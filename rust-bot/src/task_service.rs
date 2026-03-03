use crate::models::{
    ContextSnapshot, ModelStrategy, ProjectSettings, RunMode, RunTaskOptions, TaskSnapshot,
    TaskStatus,
};
use crate::runner::{OpenCodeRunner, RunnerRequest, RunnerResult};
use crate::util::{context_key, normalize_project_name, normalize_thinking, now_ms, task_key};
use anyhow::{anyhow, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;

const RETRYABLE_PATTERNS: [&str; 7] = [
    r"rate\s*limit",
    r"quota",
    r"insufficient",
    r"insufficient[_\s-]*credit",
    r"billing",
    r"too\s*many\s*requests",
    r"429",
];

#[derive(Debug, Clone)]
struct TaskRecord {
    snapshot: TaskSnapshot,
    cancel_token: Option<CancellationToken>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ContextRecord {
    key: String,
    active_project: String,
    projects: HashMap<String, ProjectSettings>,
    version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedState {
    version: u8,
    tasks: HashMap<String, TaskSnapshot>,
    contexts: HashMap<String, ContextRecord>,
}

#[derive(Debug)]
struct Inner {
    tasks: HashMap<String, TaskRecord>,
    contexts: HashMap<String, ContextRecord>,
}

#[derive(Clone)]
pub struct TaskService {
    inner: Arc<Mutex<Inner>>,
    sandbox_root: PathBuf,
    history_path: PathBuf,
    strategy: ModelStrategy,
    runner: OpenCodeRunner,
    max_output_chars: usize,
    default_agent: String,
    default_project: String,
    default_thinking: Option<String>,
}

impl TaskService {
    pub async fn new(
        sandbox_root: PathBuf,
        history_path: PathBuf,
        strategy: ModelStrategy,
        runner: OpenCodeRunner,
        default_agent: String,
        default_project: String,
        default_thinking: Option<String>,
        max_output_chars: usize,
    ) -> Self {
        let mut this = Self {
            inner: Arc::new(Mutex::new(Inner {
                tasks: HashMap::new(),
                contexts: HashMap::new(),
            })),
            sandbox_root,
            history_path,
            strategy,
            runner,
            max_output_chars,
            default_agent,
            default_project,
            default_thinking,
        };
        let _ = this.load_persisted_state().await;
        this
    }

    pub fn strategy(&self) -> ModelStrategy {
        self.strategy.clone()
    }

    pub async fn get_context(&self, chat_id: i64, thread_id: Option<i32>) -> ContextSnapshot {
        let key = context_key(chat_id, thread_id);
        let mut inner = self.inner.lock().await;
        let ctx = self.ensure_context_record(&mut inner, &key);
        self.to_context_snapshot(ctx)
    }

    pub async fn get_active_project(
        &self,
        chat_id: i64,
        thread_id: Option<i32>,
    ) -> ProjectSettings {
        let key = context_key(chat_id, thread_id);
        let mut inner = self.inner.lock().await;
        let ctx = self.ensure_context_record(&mut inner, &key);
        ctx.projects
            .get(&ctx.active_project)
            .cloned()
            .expect("active project exists")
    }

    pub async fn create_project(
        &self,
        chat_id: i64,
        thread_id: Option<i32>,
        name: &str,
    ) -> Result<ContextSnapshot> {
        let key = context_key(chat_id, thread_id);
        let project = normalize_project_name(name)?;
        {
            let mut inner = self.inner.lock().await;
            let now = now_ms();
            let ctx = self.ensure_context_record(&mut inner, &key);
            if ctx.projects.contains_key(&project) {
                return Err(anyhow!("Project '{}' already exists.", project));
            }
            ctx.projects
                .insert(project.clone(), self.new_project(&key, &project, now));
            ctx.active_project = project;
            ctx.version += 1;
        }
        self.persist().await;
        Ok(self.get_context(chat_id, thread_id).await)
    }

    pub async fn use_project(
        &self,
        chat_id: i64,
        thread_id: Option<i32>,
        name: &str,
    ) -> Result<ContextSnapshot> {
        let key = context_key(chat_id, thread_id);
        let project = normalize_project_name(name)?;
        {
            let mut inner = self.inner.lock().await;
            let ctx = self.ensure_context_record(&mut inner, &key);
            if !ctx.projects.contains_key(&project) {
                return Err(anyhow!(
                    "Project '{}' does not exist. Use /projects to list available projects.",
                    project
                ));
            }
            ctx.active_project = project;
            ctx.version += 1;
        }
        self.persist().await;
        Ok(self.get_context(chat_id, thread_id).await)
    }

    pub async fn delete_project(
        &self,
        chat_id: i64,
        thread_id: Option<i32>,
        name: &str,
    ) -> Result<(ContextSnapshot, ProjectSettings)> {
        let key = context_key(chat_id, thread_id);
        let project = normalize_project_name(name)?;

        let deleted;
        let removed_task_key = task_key(&key, &project);
        {
            let mut inner = self.inner.lock().await;
            if self
                .find_running_task_for_project(&inner, &key, &project)
                .is_some()
            {
                return Err(anyhow!(
                    "Project '{}' has a running task. Cancel it before deleting the project.",
                    project
                ));
            }

            let ctx = self.ensure_context_record(&mut inner, &key);
            if ctx.projects.len() <= 1 {
                return Err(anyhow!("Cannot delete the last project."));
            }
            let Some(target) = ctx.projects.get(&project).cloned() else {
                return Err(anyhow!(
                    "Project '{}' does not exist. Use /projects to list available projects.",
                    project
                ));
            };
            deleted = target;
            ctx.projects.remove(&project);
            if ctx.active_project == project {
                let mut names = ctx.projects.keys().cloned().collect::<Vec<_>>();
                names.sort();
                ctx.active_project = names.first().cloned().expect("at least one project");
            }
            ctx.version += 1;

            inner.tasks.remove(&removed_task_key);
        }

        self.persist().await;
        Ok((self.get_context(chat_id, thread_id).await, deleted))
    }

    pub async fn set_agent(
        &self,
        chat_id: i64,
        thread_id: Option<i32>,
        value: &str,
    ) -> Result<ProjectSettings> {
        self.update_active_project(chat_id, thread_id, |project| {
            let value = value.trim();
            if value.is_empty() {
                return Err(anyhow!("Agent name cannot be empty."));
            }
            project.agent = value.to_string();
            Ok(())
        })
        .await
    }

    pub async fn set_model(
        &self,
        chat_id: i64,
        thread_id: Option<i32>,
        value: &str,
    ) -> Result<ProjectSettings> {
        self.update_active_project(chat_id, thread_id, |project| {
            let value = value.trim();
            if value.is_empty() {
                return Err(anyhow!("Model cannot be empty."));
            }
            project.model = value.to_string();
            Ok(())
        })
        .await
    }

    pub async fn set_thinking(
        &self,
        chat_id: i64,
        thread_id: Option<i32>,
        value: Option<&str>,
    ) -> Result<ProjectSettings> {
        self.update_active_project(chat_id, thread_id, |project| {
            project.thinking = normalize_thinking(value);
            Ok(())
        })
        .await
    }

    pub async fn get_snapshot(
        &self,
        chat_id: i64,
        thread_id: Option<i32>,
        project: Option<&str>,
    ) -> Result<TaskSnapshot> {
        let key = context_key(chat_id, thread_id);
        let mut inner = self.inner.lock().await;
        let ctx = self.ensure_context_record(&mut inner, &key).clone();
        let project_name = match project {
            Some(v) => normalize_project_name(v)?,
            None => ctx.active_project,
        };
        let project_settings = ctx
            .projects
            .get(&project_name)
            .cloned()
            .ok_or_else(|| anyhow!("Project '{}' does not exist.", project_name))?;
        let key = task_key(&key, &project_name);
        let snapshot = inner
            .tasks
            .get(&key)
            .map(|v| v.snapshot.clone())
            .unwrap_or_else(|| self.new_idle_record(&ctx.key, &project_settings));
        Ok(snapshot)
    }

    pub async fn get_running_snapshot(
        &self,
        chat_id: i64,
        thread_id: Option<i32>,
    ) -> Option<TaskSnapshot> {
        let key = context_key(chat_id, thread_id);
        let inner = self.inner.lock().await;
        self.find_running_task(&inner, &key)
            .map(|task| task.snapshot.clone())
    }

    pub async fn cancel(
        &self,
        chat_id: i64,
        thread_id: Option<i32>,
        project: Option<&str>,
    ) -> Result<TaskSnapshot> {
        let key = context_key(chat_id, thread_id);
        let mut snapshot = None;
        {
            let mut inner = self.inner.lock().await;
            let target = if let Some(project_name) = project {
                let name = normalize_project_name(project_name)?;
                inner.tasks.get_mut(&task_key(&key, &name))
            } else {
                let running_key = inner
                    .tasks
                    .iter()
                    .find(|(_, t)| {
                        t.snapshot.context_key == key && t.snapshot.status == TaskStatus::Running
                    })
                    .map(|(k, _)| k.clone());
                running_key.and_then(|k| inner.tasks.get_mut(&k))
            };

            let Some(record) = target else {
                return Err(anyhow!("No running task to cancel."));
            };
            if record.snapshot.status != TaskStatus::Running {
                return Err(anyhow!("No running task to cancel."));
            }

            if let Some(token) = record.cancel_token.take() {
                token.cancel();
            }
            record.snapshot.status = TaskStatus::Cancelled;
            record.snapshot.finished_at = Some(now_ms());
            record.snapshot.version += 1;
            snapshot = Some(record.snapshot.clone());
        }
        self.persist().await;
        Ok(snapshot.expect("snapshot exists"))
    }

    pub async fn run(
        &self,
        chat_id: i64,
        thread_id: Option<i32>,
        command: String,
        options: RunTaskOptions,
    ) -> Result<TaskSnapshot> {
        let ctx_key = context_key(chat_id, thread_id);
        let RunTaskOptions {
            project: opt_project,
            model: opt_model,
            agent: opt_agent,
            mode: opt_mode,
            thinking: opt_thinking,
        } = options;

        let (rec_key, project_work_dir, first_snapshot);
        {
            let mut inner = self.inner.lock().await;
            if let Some(running) = self.find_running_task(&inner, &ctx_key) {
                return Err(anyhow!(
                    "A task is already running for project '{}'.",
                    running.snapshot.project
                ));
            }

            let ctx = self.ensure_context_record(&mut inner, &ctx_key).clone();
            let selected_project = match opt_project {
                Some(ref v) => normalize_project_name(v)?,
                None => ctx.active_project,
            };
            let project = ctx
                .projects
                .get(&selected_project)
                .cloned()
                .ok_or_else(|| anyhow!("Project '{}' does not exist.", selected_project))?;

            let model = opt_model.unwrap_or_else(|| project.model.clone());
            let agent = opt_agent.unwrap_or_else(|| project.agent.clone());
            let mode = opt_mode.unwrap_or_else(|| RunMode::from_agent(&agent));
            let thinking = opt_thinking
                .unwrap_or_else(|| project.thinking.clone())
                .and_then(|v| normalize_thinking(Some(&v)));

            rec_key = task_key(&ctx_key, &project.name);
            project_work_dir = PathBuf::from(project.work_dir.clone());
            let token = CancellationToken::new();

            let mut snapshot = self.new_idle_record(&ctx_key, &project);
            snapshot.command = Some(command.clone());
            snapshot.status = TaskStatus::Running;
            snapshot.started_at = Some(now_ms());
            snapshot.output = format!("Project: {}\nTask: {}\n", project.name, command);
            snapshot.version = 1;
            snapshot.model = model;
            snapshot.agent = agent;
            snapshot.mode = mode;
            snapshot.thinking = thinking;
            snapshot.session_id = project.session_id.clone();

            inner.tasks.insert(
                rec_key.clone(),
                TaskRecord {
                    snapshot: snapshot.clone(),
                    cancel_token: Some(token.clone()),
                },
            );
            first_snapshot = snapshot;
        }

        let _ = fs::create_dir_all(&project_work_dir).await;
        self.persist().await;

        let this = self.clone();
        tokio::spawn(async move {
            let _ = this.execute_with_fallback(&rec_key, project_work_dir).await;
        });

        Ok(first_snapshot)
    }

    async fn execute_with_fallback(&self, rec_key: &str, work_dir: PathBuf) -> Result<()> {
        self.append_output(
            rec_key,
            &format_header(self.get_task_snapshot(rec_key).await?),
        )
        .await?;

        let primary_model = self.get_task_snapshot(rec_key).await?.model.clone();
        let primary = self
            .run_single_attempt(rec_key, work_dir.clone(), primary_model.clone())
            .await?;

        if self.is_cancelled(rec_key).await? {
            self.finish_cancelled(rec_key, &primary).await?;
            return Ok(());
        }

        if is_success(&primary) {
            self.finish_success(
                rec_key,
                &primary,
                &format!("Completed with model {}.", primary_model),
            )
            .await?;
            return Ok(());
        }

        let can_fallback =
            self.strategy.fallback_model != primary_model && is_retryable_failure(&primary);
        if !can_fallback {
            self.finish_failed(
                rec_key,
                &primary,
                &format!("Failed with model {}.", primary_model),
            )
            .await?;
            return Ok(());
        }

        self.append_output(
            rec_key,
            &format!(
                "\nPrimary model failed with quota/rate-limit style error. Retrying once with {}.\n",
                self.strategy.fallback_model
            ),
        )
        .await?;
        self.set_fallback_used(rec_key).await?;

        let fallback = self
            .run_single_attempt(rec_key, work_dir, self.strategy.fallback_model.clone())
            .await?;

        if self.is_cancelled(rec_key).await? {
            self.finish_cancelled(rec_key, &fallback).await?;
        } else if is_success(&fallback) {
            self.finish_success(
                rec_key,
                &fallback,
                &format!(
                    "Completed via fallback model {}.",
                    self.strategy.fallback_model
                ),
            )
            .await?;
        } else {
            self.finish_failed(
                rec_key,
                &fallback,
                &format!("Fallback model {} failed.", self.strategy.fallback_model),
            )
            .await?;
        }
        Ok(())
    }

    async fn run_single_attempt(
        &self,
        rec_key: &str,
        work_dir: PathBuf,
        model: String,
    ) -> Result<RunnerResult> {
        self.mark_attempt(rec_key, &model).await?;

        let (request, token) = {
            let inner = self.inner.lock().await;
            let rec = inner
                .tasks
                .get(rec_key)
                .ok_or_else(|| anyhow!("Task not found."))?;
            (
                RunnerRequest {
                    task: rec.snapshot.command.clone().unwrap_or_default(),
                    model: model.clone(),
                    work_dir: work_dir.clone(),
                    agent: Some(rec.snapshot.agent.clone()),
                    thinking: rec.snapshot.thinking.clone(),
                    session_id: rec.snapshot.session_id.clone(),
                },
                rec.cancel_token
                    .clone()
                    .ok_or_else(|| anyhow!("Task cancel token missing."))?,
            )
        };

        let (tx, mut rx) = mpsc::unbounded_channel();
        let runner = self.runner.clone();
        let task = tokio::spawn(async move { runner.execute(request, token, tx).await });

        while let Some(chunk) = rx.recv().await {
            let _ = self.append_output(rec_key, &chunk).await;
        }

        let result = task.await.map_err(|e| anyhow!(e.to_string()))?;
        if let Some(session) = result.session_id.clone() {
            self.set_session(rec_key, &session).await?;
        }
        Ok(result)
    }

    async fn set_session(&self, rec_key: &str, session_id: &str) -> Result<()> {
        let mut inner = self.inner.lock().await;
        let (ctx_key, project_name) = {
            let rec = inner
                .tasks
                .get_mut(rec_key)
                .ok_or_else(|| anyhow!("Task not found."))?;
            rec.snapshot.session_id = Some(session_id.to_string());
            rec.snapshot.version += 1;
            (
                rec.snapshot.context_key.clone(),
                rec.snapshot.project.clone(),
            )
        };

        if let Some(ctx) = inner.contexts.get_mut(&ctx_key) {
            if let Some(project) = ctx.projects.get_mut(&project_name) {
                if project.session_id.as_deref() != Some(session_id) {
                    project.session_id = Some(session_id.to_string());
                    project.updated_at = now_ms();
                    ctx.version += 1;
                }
            }
        }

        drop(inner);
        self.persist().await;
        Ok(())
    }

    async fn finish_success(
        &self,
        rec_key: &str,
        result: &RunnerResult,
        footer: &str,
    ) -> Result<()> {
        self.finish(rec_key, TaskStatus::Success, result, footer)
            .await
    }

    async fn finish_failed(
        &self,
        rec_key: &str,
        result: &RunnerResult,
        footer: &str,
    ) -> Result<()> {
        self.finish(rec_key, TaskStatus::Failed, result, footer)
            .await
    }

    async fn finish_cancelled(&self, rec_key: &str, result: &RunnerResult) -> Result<()> {
        self.finish(rec_key, TaskStatus::Cancelled, result, "Task cancelled.")
            .await
    }

    async fn finish(
        &self,
        rec_key: &str,
        status: TaskStatus,
        result: &RunnerResult,
        footer: &str,
    ) -> Result<()> {
        {
            let mut inner = self.inner.lock().await;
            let rec = inner
                .tasks
                .get_mut(rec_key)
                .ok_or_else(|| anyhow!("Task not found."))?;
            rec.snapshot.status = status;
            rec.snapshot.finished_at = Some(now_ms());
            rec.snapshot.exit_code = result.exit_code;
            rec.snapshot.signal = result.signal.clone();
            rec.cancel_token = None;
            rec.snapshot.output.push('\n');
            rec.snapshot.output.push_str(footer);
            rec.snapshot.output.push('\n');
            rec.snapshot.version += 1;
        }
        self.persist().await;
        Ok(())
    }

    async fn mark_attempt(&self, rec_key: &str, model: &str) -> Result<()> {
        {
            let mut inner = self.inner.lock().await;
            let rec = inner
                .tasks
                .get_mut(rec_key)
                .ok_or_else(|| anyhow!("Task not found."))?;
            rec.snapshot.attempted_models.push(model.to_string());
            rec.snapshot.last_run_model = Some(model.to_string());
            rec.snapshot
                .output
                .push_str(&format!("\n--- Running with model: {} ---\n", model));
            rec.snapshot.version += 1;
        }
        self.persist().await;
        Ok(())
    }

    async fn set_fallback_used(&self, rec_key: &str) -> Result<()> {
        let mut inner = self.inner.lock().await;
        let rec = inner
            .tasks
            .get_mut(rec_key)
            .ok_or_else(|| anyhow!("Task not found."))?;
        rec.snapshot.fallback_used = true;
        rec.snapshot.version += 1;
        drop(inner);
        self.persist().await;
        Ok(())
    }

    async fn is_cancelled(&self, rec_key: &str) -> Result<bool> {
        let inner = self.inner.lock().await;
        let rec = inner
            .tasks
            .get(rec_key)
            .ok_or_else(|| anyhow!("Task not found."))?;
        Ok(rec.snapshot.status == TaskStatus::Cancelled)
    }

    async fn append_output(&self, rec_key: &str, chunk: &str) -> Result<()> {
        let mut inner = self.inner.lock().await;
        let rec = inner
            .tasks
            .get_mut(rec_key)
            .ok_or_else(|| anyhow!("Task not found."))?;
        rec.snapshot.output.push_str(chunk);
        if self.max_output_chars > 0 && rec.snapshot.output.len() > self.max_output_chars {
            let len = rec.snapshot.output.len();
            rec.snapshot.output = rec.snapshot.output[(len - self.max_output_chars)..].to_string();
        }
        rec.snapshot.version += 1;
        Ok(())
    }

    async fn get_task_snapshot(&self, rec_key: &str) -> Result<TaskSnapshot> {
        let inner = self.inner.lock().await;
        inner
            .tasks
            .get(rec_key)
            .map(|r| r.snapshot.clone())
            .ok_or_else(|| anyhow!("Task not found."))
    }

    async fn update_active_project<F>(
        &self,
        chat_id: i64,
        thread_id: Option<i32>,
        updater: F,
    ) -> Result<ProjectSettings>
    where
        F: FnOnce(&mut ProjectSettings) -> Result<()>,
    {
        let key = context_key(chat_id, thread_id);
        let project = {
            let mut inner = self.inner.lock().await;
            let ctx = self.ensure_context_record(&mut inner, &key);
            let name = ctx.active_project.clone();
            let project_clone = {
                let project = ctx
                    .projects
                    .get_mut(&name)
                    .ok_or_else(|| anyhow!("Active project is missing."))?;
                updater(project)?;
                project.updated_at = now_ms();
                project.clone()
            };
            ctx.version += 1;
            project_clone
        };
        self.persist().await;
        Ok(project)
    }

    fn new_project(&self, ctx_key: &str, name: &str, now: i64) -> ProjectSettings {
        ProjectSettings {
            name: name.to_string(),
            work_dir: self
                .sandbox_root
                .join(ctx_key.replace(':', "-"))
                .join(name)
                .display()
                .to_string(),
            agent: self.default_agent.clone(),
            model: self.strategy.default_model.clone(),
            thinking: self.default_thinking.clone(),
            session_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn new_idle_record(&self, ctx_key: &str, project: &ProjectSettings) -> TaskSnapshot {
        TaskSnapshot {
            key: task_key(ctx_key, &project.name),
            context_key: ctx_key.to_string(),
            project: project.name.clone(),
            command: None,
            status: TaskStatus::Idle,
            started_at: None,
            finished_at: None,
            exit_code: None,
            signal: None,
            output: String::new(),
            attempted_models: vec![],
            last_run_model: None,
            fallback_used: false,
            model: project.model.clone(),
            agent: project.agent.clone(),
            mode: RunMode::from_agent(&project.agent),
            thinking: project.thinking.clone(),
            session_id: project.session_id.clone(),
            version: 0,
        }
    }

    fn ensure_context_record<'a>(&self, inner: &'a mut Inner, key: &str) -> &'a mut ContextRecord {
        if !inner.contexts.contains_key(key) {
            let now = now_ms();
            let default_project = normalize_project_name(&self.default_project)
                .unwrap_or_else(|_| "main".to_string());
            let mut projects = HashMap::new();
            projects.insert(
                default_project.clone(),
                self.new_project(key, &default_project, now),
            );
            inner.contexts.insert(
                key.to_string(),
                ContextRecord {
                    key: key.to_string(),
                    active_project: default_project,
                    projects,
                    version: 0,
                },
            );
        }
        inner.contexts.get_mut(key).expect("context exists")
    }

    fn find_running_task<'a>(&self, inner: &'a Inner, ctx_key: &str) -> Option<&'a TaskRecord> {
        inner
            .tasks
            .values()
            .find(|t| t.snapshot.context_key == ctx_key && t.snapshot.status == TaskStatus::Running)
    }

    fn find_running_task_for_project<'a>(
        &self,
        inner: &'a Inner,
        ctx_key: &str,
        project: &str,
    ) -> Option<&'a TaskRecord> {
        inner.tasks.get(&task_key(ctx_key, project)).and_then(|t| {
            if t.snapshot.status == TaskStatus::Running {
                Some(t)
            } else {
                None
            }
        })
    }

    fn to_context_snapshot(&self, rec: &ContextRecord) -> ContextSnapshot {
        let mut projects = rec.projects.values().cloned().collect::<Vec<_>>();
        projects.sort_by(|a, b| a.name.cmp(&b.name));
        ContextSnapshot {
            key: rec.key.clone(),
            active_project: rec.active_project.clone(),
            projects,
            version: rec.version,
        }
    }

    async fn load_persisted_state(&mut self) -> Result<()> {
        if !self.history_path.exists() {
            return Ok(());
        }
        let raw = fs::read_to_string(&self.history_path).await?;
        if raw.trim().is_empty() {
            return Ok(());
        }

        let parsed: PersistedState = serde_json::from_str(&raw)?;
        let mut inner = self.inner.lock().await;
        inner.contexts = parsed.contexts;
        inner.tasks = parsed
            .tasks
            .into_iter()
            .map(|(k, mut task)| {
                if task.status == TaskStatus::Running {
                    task.status = TaskStatus::Failed;
                    task.finished_at = Some(now_ms());
                    task.output.push_str("\nTask interrupted by bot restart.\n");
                    task.version += 1;
                }
                (
                    k,
                    TaskRecord {
                        snapshot: task,
                        cancel_token: None,
                    },
                )
            })
            .collect();
        Ok(())
    }

    async fn persist(&self) {
        let payload = {
            let inner = self.inner.lock().await;
            let tasks = inner
                .tasks
                .iter()
                .map(|(k, v)| (k.clone(), v.snapshot.clone()))
                .collect::<HashMap<_, _>>();
            PersistedState {
                version: 2,
                tasks,
                contexts: inner.contexts.clone(),
            }
        };

        let _ =
            fs::create_dir_all(self.history_path.parent().unwrap_or_else(|| Path::new("."))).await;
        if let Ok(text) = serde_json::to_string_pretty(&payload) {
            let _ = fs::write(&self.history_path, format!("{}\n", text)).await;
        }
    }
}

fn format_header(snapshot: TaskSnapshot) -> String {
    format!(
        "Agent: {}; Mode: {:?}; Model: {}; Thinking: {}; Session: {}\nModel strategy: {} -> {}\n",
        snapshot.agent,
        snapshot.mode,
        snapshot.model,
        snapshot.thinking.clone().unwrap_or_else(|| "-".to_string()),
        snapshot
            .session_id
            .clone()
            .unwrap_or_else(|| "new".to_string()),
        snapshot.model,
        std::env::var("FALLBACK_MODEL").unwrap_or_else(|_| "GLM-5".to_string())
    )
}

fn is_retryable_failure(result: &RunnerResult) -> bool {
    let haystack = format!(
        "{}\n{}",
        result.output,
        result.error_message.clone().unwrap_or_default()
    );
    RETRYABLE_PATTERNS.iter().any(|pat| {
        Regex::new(pat)
            .map(|re| re.is_match(&haystack))
            .unwrap_or(false)
    })
}

fn is_success(result: &RunnerResult) -> bool {
    result.exit_code == Some(0)
}
