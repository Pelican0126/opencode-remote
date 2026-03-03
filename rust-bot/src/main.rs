mod models;
mod runner;
mod runtime_env;
mod task_service;
mod util;

use anyhow::Result;
use models::{ModelStrategy, RunMode, RunTaskOptions, TaskSnapshot, TaskStatus};
use runner::{parse_agent_list, parse_model_list, OpenCodeRunner};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use task_service::TaskService;
use teloxide::prelude::*;
use teloxide::types::{
    BotCommand, InlineKeyboardButton, InlineKeyboardMarkup, MessageId, ParseMode,
};
use teloxide::utils::command::BotCommands;
use tokio::sync::Mutex;
use tracing::{error, info};
use util::{fit_telegram_text, strip_ansi};

const UPDATE_INTERVAL_MS: u64 = 3000;
const MAX_TELEGRAM_TEXT: usize = 3900;
const MAX_LIST_ITEMS: usize = 40;
const PROJECT_PAGE_SIZE: usize = 6;
const MODEL_PAGE_SIZE: usize = 8;
const PENDING_TTL_MS: i64 = 10 * 60 * 1000;
const OUTPUT_STREAM_CHUNK: usize = 2800;

#[derive(Clone, Debug)]
enum PendingInputKind {
    NewProject,
    RunTask,
    SetModel,
    SetAgent,
    SetThinking,
}

#[derive(Clone, Debug)]
struct PendingInputState {
    kind: PendingInputKind,
    mode_override: Option<RunMode>,
    created_at: i64,
}

#[derive(Clone, Debug)]
struct ModelCacheEntry {
    models: Vec<String>,
    updated_at: i64,
}

#[derive(Clone)]
struct App {
    task_service: TaskService,
    runner: OpenCodeRunner,
    pending: Arc<Mutex<HashMap<String, PendingInputState>>>,
    model_cache: Arc<Mutex<HashMap<String, ModelCacheEntry>>>,
}

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Commands")]
enum Command {
    #[command(description = "show help")]
    Help,
    #[command(description = "show help")]
    Start,
    #[command(description = "open control menu")]
    Menu,
    #[command(description = "pong")]
    Ping,
    #[command(description = "run task")]
    Run(String),
    #[command(description = "show status")]
    Status,
    #[command(description = "cancel task")]
    Cancel,
    #[command(description = "interrupt task")]
    Interrupt,
    #[command(description = "create project")]
    New(String),
    #[command(description = "list projects")]
    Projects,
    #[command(description = "switch project")]
    Use(String),
    #[command(description = "delete current project")]
    Del(String),
    #[command(description = "agent list|set")]
    Agent(String),
    #[command(description = "model list|set")]
    Model(String),
    #[command(description = "thinking set|off")]
    Thinking(String),
    #[command(description = "mode plan|build")]
    Mode(String),
}

fn action(name: &str) -> String {
    name.to_string()
}

fn pending_key(msg: &Message) -> String {
    let chat_id = msg.chat.id.0;
    let user_id = msg.from().map(|u| u.id.0).unwrap_or(0);
    format!("{}:{}:{}", chat_id, "main", user_id)
}

fn pending_key_raw(chat_id: i64, user_id: u64) -> String {
    format!("{}:{}:{}", chat_id, "main", user_id)
}

fn monitor_key(chat_id: i64, project: &str) -> String {
    format!("{}|{}", chat_id, project)
}

fn clean_output_line(line: &str) -> String {
    let mut cleaned = strip_ansi(line).replace('\r', "");
    cleaned = cleaned.trim().to_string();
    if cleaned.starts_with('%') {
        cleaned = cleaned.trim_start_matches('%').trim().to_string();
    }
    cleaned
}

fn is_noise_output_line(line: &str) -> bool {
    if line.is_empty() {
        return true;
    }
    let line = line.to_lowercase();
    [
        "project:",
        "task:",
        "agent:",
        "model strategy:",
        "--- running with model:",
        "completed with model",
        "completed via fallback model",
        "failed with model",
        "task cancelled",
    ]
    .iter()
    .any(|pat| line.starts_with(pat))
}

fn normalized_output(snapshot: &TaskSnapshot) -> String {
    snapshot
        .output
        .lines()
        .map(clean_output_line)
        .filter(|line| !is_noise_output_line(line))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn split_output_chunks(text: &str, max: usize) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut chunks = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        if line.len() > max {
            if !current.is_empty() {
                chunks.push(current.clone());
                current.clear();
            }
            let mut i = 0;
            while i < line.len() {
                let end = (i + max).min(line.len());
                chunks.push(line[i..end].to_string());
                i = end;
            }
            continue;
        }

        let next = if current.is_empty() {
            line.to_string()
        } else {
            format!("{}\n{}", current, line)
        };
        if next.len() > max {
            chunks.push(current.clone());
            current = line.to_string();
        } else {
            current = next;
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

fn completion_text(snapshot: &TaskSnapshot) -> String {
    let model = snapshot
        .last_run_model
        .clone()
        .unwrap_or_else(|| snapshot.model.clone());
    match snapshot.status {
        TaskStatus::Success => format!("Completed with model {}.", model),
        TaskStatus::Failed => format!("Failed with model {}.", model),
        TaskStatus::Cancelled => "Task cancelled.".to_string(),
        TaskStatus::Running => format!("Running with model {}...", model),
        TaskStatus::Idle => "Idle.".to_string(),
    }
}

fn task_to_text(snapshot: &TaskSnapshot) -> String {
    let model = snapshot
        .last_run_model
        .clone()
        .unwrap_or_else(|| snapshot.model.clone());
    let header = [
        format!("Status: {:?}", snapshot.status),
        format!("Project: {}", snapshot.project),
        format!("Agent: {}", snapshot.agent),
        format!("Mode: {:?}", snapshot.mode),
        format!("Model: {}", model),
    ]
    .join("\n");

    let body = if snapshot.status == TaskStatus::Running {
        "Streaming output...".to_string()
    } else {
        completion_text(snapshot)
    };
    fit_telegram_text(
        &format!(
            "{}\n\n<pre>{}</pre>",
            header,
            html_escape::encode_text(&body)
        ),
        MAX_TELEGRAM_TEXT,
    )
}

fn project_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("New Project", action("p:new")),
            InlineKeyboardButton::callback("Switch Project", action("p:switch:0")),
        ],
        vec![
            InlineKeyboardButton::callback("Project List", action("p:list")),
            InlineKeyboardButton::callback("Delete This Project", action("p:delthis")),
        ],
        vec![InlineKeyboardButton::callback("Back", action("m:home"))],
    ])
}

fn main_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("Projects", action("m:projects")),
            InlineKeyboardButton::callback("Run", action("m:run")),
        ],
        vec![
            InlineKeyboardButton::callback("Model", action("m:model")),
            InlineKeyboardButton::callback("Agent", action("m:agent")),
        ],
        vec![
            InlineKeyboardButton::callback("Thinking", action("m:thinking")),
            InlineKeyboardButton::callback("Mode", action("m:mode")),
        ],
        vec![
            InlineKeyboardButton::callback("Status", action("m:status")),
            InlineKeyboardButton::callback("Cancel Task", action("m:cancel")),
        ],
    ])
}

fn run_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "Run Task",
            action("r:input"),
        )],
        vec![
            InlineKeyboardButton::callback("Run Plan", action("r:input:plan")),
            InlineKeyboardButton::callback("Run Build", action("r:input:build")),
        ],
        vec![InlineKeyboardButton::callback("Back", action("m:home"))],
    ])
}

fn model_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "Select Model",
            action("mdl:pick:0"),
        )],
        vec![
            InlineKeyboardButton::callback("List Models", action("mdl:list")),
            InlineKeyboardButton::callback("Set Model", action("mdl:set")),
        ],
        vec![InlineKeyboardButton::callback("Back", action("m:home"))],
    ])
}

fn agent_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("List Agents", action("agt:list")),
            InlineKeyboardButton::callback("Set Agent", action("agt:set")),
        ],
        vec![InlineKeyboardButton::callback("Back", action("m:home"))],
    ])
}

fn thinking_menu(current: Option<&str>) -> InlineKeyboardMarkup {
    let current = current.unwrap_or("off").to_lowercase();
    let label = |name: &str, v: &str| {
        if current == v {
            format!("{} ✓", name)
        } else {
            name.to_string()
        }
    };

    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            label("Off", "off"),
            action("th:off"),
        )],
        vec![
            InlineKeyboardButton::callback(label("Minimal", "minimal"), action("th:set:minimal")),
            InlineKeyboardButton::callback(label("Low", "low"), action("th:set:low")),
        ],
        vec![
            InlineKeyboardButton::callback(label("Medium", "medium"), action("th:set:medium")),
            InlineKeyboardButton::callback(label("High", "high"), action("th:set:high")),
        ],
        vec![
            InlineKeyboardButton::callback(label("Max", "max"), action("th:set:max")),
            InlineKeyboardButton::callback(label("XHigh", "xhigh"), action("th:set:xhigh")),
        ],
        vec![InlineKeyboardButton::callback("Back", action("m:home"))],
    ])
}

fn mode_menu(current: RunMode) -> InlineKeyboardMarkup {
    let build = if current == RunMode::Build {
        "Build ✓"
    } else {
        "Build"
    };
    let plan = if current == RunMode::Plan {
        "Plan ✓"
    } else {
        "Plan"
    };
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback(build, action("mode:build")),
            InlineKeyboardButton::callback(plan, action("mode:plan")),
        ],
        vec![InlineKeyboardButton::callback("Back", action("m:home"))],
    ])
}

fn project_switch_menu(projects: &[String], active: &str, page: usize) -> InlineKeyboardMarkup {
    let total_pages = projects.len().max(1).div_ceil(PROJECT_PAGE_SIZE);
    let page = page.min(total_pages.saturating_sub(1));
    let start = page * PROJECT_PAGE_SIZE;
    let mut rows = projects
        .iter()
        .enumerate()
        .skip(start)
        .take(PROJECT_PAGE_SIZE)
        .map(|(idx, p)| {
            let label = if p == active {
                format!("✓ {}", p)
            } else {
                p.clone()
            };
            vec![InlineKeyboardButton::callback(
                label,
                format!("p:use:{}:{}", idx, page),
            )]
        })
        .collect::<Vec<_>>();

    if total_pages > 1 {
        let mut nav = Vec::new();
        if page > 0 {
            nav.push(InlineKeyboardButton::callback(
                "Prev",
                format!("p:switch:{}", page - 1),
            ));
        }
        if page + 1 < total_pages {
            nav.push(InlineKeyboardButton::callback(
                "Next",
                format!("p:switch:{}", page + 1),
            ));
        }
        if !nav.is_empty() {
            rows.push(nav);
        }
    }

    rows.push(vec![
        InlineKeyboardButton::callback("New Project", action("p:new")),
        InlineKeyboardButton::callback("Back", action("m:projects")),
    ]);
    InlineKeyboardMarkup::new(rows)
}

fn project_delete_menu(projects: &[String], active: &str, page: usize) -> InlineKeyboardMarkup {
    let total_pages = projects.len().max(1).div_ceil(PROJECT_PAGE_SIZE);
    let page = page.min(total_pages.saturating_sub(1));
    let start = page * PROJECT_PAGE_SIZE;
    let mut rows = projects
        .iter()
        .enumerate()
        .skip(start)
        .take(PROJECT_PAGE_SIZE)
        .map(|(idx, p)| {
            let label = if p == active {
                format!("* {}", p)
            } else {
                p.clone()
            };
            vec![InlineKeyboardButton::callback(
                label,
                format!("p:delpick:{}:{}", idx, page),
            )]
        })
        .collect::<Vec<_>>();

    if total_pages > 1 {
        let mut nav = Vec::new();
        if page > 0 {
            nav.push(InlineKeyboardButton::callback(
                "Prev",
                format!("p:delete:{}", page - 1),
            ));
        }
        if page + 1 < total_pages {
            nav.push(InlineKeyboardButton::callback(
                "Next",
                format!("p:delete:{}", page + 1),
            ));
        }
        if !nav.is_empty() {
            rows.push(nav);
        }
    }
    rows.push(vec![InlineKeyboardButton::callback(
        "Back",
        action("m:projects"),
    )]);
    InlineKeyboardMarkup::new(rows)
}

fn project_delete_confirm_menu(index: usize, page: usize) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            "Confirm Delete",
            format!("p:delconfirm:{}:{}", index, page),
        ),
        InlineKeyboardButton::callback("Cancel", format!("p:delcancel:{}", page)),
    ]])
}

async fn send_menu_or_edit(
    bot: &Bot,
    q: Option<&CallbackQuery>,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    text: String,
    markup: InlineKeyboardMarkup,
) -> Result<()> {
    if let (Some(_), Some(mid)) = (q, message_id) {
        let _ = bot
            .edit_message_text(chat_id, mid, text)
            .reply_markup(markup)
            .await;
        return Ok(());
    }
    bot.send_message(chat_id, text).reply_markup(markup).await?;
    Ok(())
}

async fn set_pending(
    app: &App,
    msg: &Message,
    kind: PendingInputKind,
    mode_override: Option<RunMode>,
) {
    let mut map = app.pending.lock().await;
    map.insert(
        pending_key(msg),
        PendingInputState {
            kind,
            mode_override,
            created_at: util::now_ms(),
        },
    );
}

async fn set_pending_raw(
    app: &App,
    chat_id: i64,
    user_id: u64,
    kind: PendingInputKind,
    mode_override: Option<RunMode>,
) {
    let mut map = app.pending.lock().await;
    map.insert(
        pending_key_raw(chat_id, user_id),
        PendingInputState {
            kind,
            mode_override,
            created_at: util::now_ms(),
        },
    );
}

async fn take_pending(app: &App, msg: &Message) -> Option<PendingInputState> {
    let mut map = app.pending.lock().await;
    let key = pending_key(msg);
    let value = map.get(&key).cloned();
    if let Some(v) = &value {
        if util::now_ms() - v.created_at > PENDING_TTL_MS {
            map.remove(&key);
            return None;
        }
    }
    value
}

fn command_args(input: &str) -> Vec<&str> {
    input.split_whitespace().collect()
}

fn parse_run_payload(raw: &str) -> (String, Option<RunMode>) {
    let text = raw.trim();
    let lower = text.to_lowercase();
    if lower.starts_with("plan ") {
        (text[5..].trim().to_string(), Some(RunMode::Plan))
    } else if lower == "plan" {
        (String::new(), Some(RunMode::Plan))
    } else if lower.starts_with("build ") || lower.starts_with("bulid ") {
        let idx = text.find(' ').unwrap_or(text.len());
        (text[idx..].trim().to_string(), Some(RunMode::Build))
    } else if lower == "build" || lower == "bulid" {
        (String::new(), Some(RunMode::Build))
    } else {
        (text.to_string(), None)
    }
}

async fn start_monitor(
    bot: Bot,
    app: App,
    chat_id: ChatId,
    status_message_id: MessageId,
    output_message_id: MessageId,
    project: String,
) {
    // The monitor is intentionally simple: one timer per running task.
    tokio::spawn(async move {
        let mut last_version = u64::MAX;
        let mut last_rendered = String::new();
        let _key = monitor_key(chat_id.0, &project);
        loop {
            let snapshot = match app
                .task_service
                .get_snapshot(chat_id.0, None, Some(&project))
                .await
            {
                Ok(v) => v,
                Err(_) => break,
            };
            if snapshot.version != last_version {
                let text = task_to_text(&snapshot);
                let _ = bot
                    .edit_message_text(chat_id, status_message_id, text)
                    .parse_mode(ParseMode::Html)
                    .await;

                let output = normalized_output(&snapshot);
                let chunks = if output.is_empty() {
                    if snapshot.status == TaskStatus::Running {
                        vec!["Waiting for output...\n\n▍".to_string()]
                    } else {
                        vec![completion_text(&snapshot)]
                    }
                } else {
                    let mut c = split_output_chunks(&output, OUTPUT_STREAM_CHUNK);
                    if snapshot.status == TaskStatus::Running {
                        if let Some(last) = c.last_mut() {
                            last.push_str("\n\n▍");
                        }
                    }
                    c
                };
                let rendered = format!(
                    "<pre>{}</pre>",
                    html_escape::encode_text(&chunks.join("\n\n"))
                );
                if rendered != last_rendered {
                    let _ = bot
                        .edit_message_text(chat_id, output_message_id, rendered.clone())
                        .parse_mode(ParseMode::Html)
                        .await;
                    last_rendered = rendered;
                }

                last_version = snapshot.version;
            }

            if snapshot.status != TaskStatus::Running {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(UPDATE_INTERVAL_MS)).await;
        }
    });
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    if let Err(err) = run().await {
        error!("fatal: {err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let bot_token =
        env::var("BOT_TOKEN").map_err(|_| anyhow::anyhow!("Missing BOT_TOKEN in environment."))?;
    let workspace_root =
        PathBuf::from(env::var("WORKSPACE_ROOT").unwrap_or_else(|_| "workspace".to_string()));
    let _ = tokio::fs::create_dir_all(&workspace_root).await;

    let strategy = ModelStrategy {
        default_model: env::var("DEFAULT_MODEL").unwrap_or_else(|_| "gpt-5.3-codex".to_string()),
        fallback_model: env::var("FALLBACK_MODEL").unwrap_or_else(|_| "GLM-5".to_string()),
    };

    let history_path = PathBuf::from(
        env::var("TASK_HISTORY_FILE").unwrap_or_else(|_| "data/task-history.json".to_string()),
    );
    let max_output_chars = env::var("MAX_OUTPUT_CHARS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);

    let runner = OpenCodeRunner::new();
    let service = TaskService::new(
        workspace_root,
        history_path,
        strategy,
        runner.clone(),
        env::var("DEFAULT_AGENT").unwrap_or_else(|_| "build".to_string()),
        env::var("DEFAULT_PROJECT_NAME").unwrap_or_else(|_| "main".to_string()),
        util::normalize_thinking(env::var("DEFAULT_THINKING").ok().as_deref()),
        max_output_chars,
    )
    .await;

    let app = App {
        task_service: service,
        runner,
        pending: Arc::new(Mutex::new(HashMap::new())),
        model_cache: Arc::new(Mutex::new(HashMap::new())),
    };

    let bot = Bot::new(bot_token);
    bot.set_my_commands(vec![
        BotCommand::new("start", "Show help and quick start"),
        BotCommand::new("help", "Show help and command usage"),
        BotCommand::new("menu", "Open interactive control menu"),
        BotCommand::new("ping", "Check bot responsiveness"),
        BotCommand::new("run", "Run a task (plain text also works)"),
        BotCommand::new("status", "Show current task status"),
        BotCommand::new("cancel", "Cancel running task"),
        BotCommand::new("interrupt", "Interrupt running task (alias of /cancel)"),
        BotCommand::new("new", "Create and switch project"),
        BotCommand::new("del", "Delete current project with confirmation"),
        BotCommand::new("projects", "List projects in this chat"),
        BotCommand::new("use", "Switch active project"),
        BotCommand::new("agent", "List or set active agent"),
        BotCommand::new("model", "List or set active model"),
        BotCommand::new("thinking", "Set thinking strength"),
        BotCommand::new("mode", "Set mode: plan or build"),
    ])
    .await?;

    info!("Telegram command menu registered.");

    let handler = Update::filter_message()
        .branch(
            dptree::entry()
                .filter_command::<Command>()
                .endpoint(handle_command),
        )
        .branch(Update::filter_callback_query().endpoint(handle_callback))
        .branch(Update::filter_message().endpoint(handle_plain_text));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![app])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
    Ok(())
}

async fn send_help(bot: &Bot, msg: &Message) -> Result<()> {
    bot.send_message(
        msg.chat.id,
        [
            "TG remote coding bot is ready.",
            "Type / to see all commands, or use /menu for buttons.",
            "",
            "Core:",
            "/run <task>",
            "(or just send plain text)",
            "/status",
            "/cancel",
            "/interrupt",
            "",
            "Projects:",
            "/new <name>",
            "/del this project",
            "/projects",
            "/use <name>",
            "(or use /menu -> Projects -> Delete This Project)",
            "",
            "Configuration:",
            "/agent list | /agent set <name>",
            "/model (button picker) | /model set <provider/model>",
            "/thinking set <minimal|low|medium|high|max|xhigh>",
            "/thinking off",
            "/mode (buttons) or /mode plan|build",
        ]
        .join("\n"),
    )
    .reply_markup(main_menu())
    .await?;
    Ok(())
}

// Command handlers are kept thin and push most state changes into TaskService.
async fn handle_command(bot: Bot, msg: Message, cmd: Command, app: App) -> Result<()> {
    match cmd {
        Command::Help | Command::Start => send_help(&bot, &msg).await?,
        Command::Menu => {
            bot.send_message(msg.chat.id, "Main menu\nChoose an action:")
                .reply_markup(main_menu())
                .await?;
        }
        Command::Ping => {
            bot.send_message(msg.chat.id, "pong").await?;
        }
        Command::New(name) => {
            let name = name.trim();
            if name.is_empty() {
                set_pending(&app, &msg, PendingInputKind::NewProject, None).await;
                bot.send_message(
                    msg.chat.id,
                    "Send the new project name in your next message. Type 'cancel' to abort.",
                )
                .await?;
            } else {
                let snapshot = app
                    .task_service
                    .create_project(msg.chat.id.0, None, name)
                    .await?;
                let active = app
                    .task_service
                    .get_active_project(msg.chat.id.0, None)
                    .await;
                let _ = tokio::fs::create_dir_all(&active.work_dir).await;
                bot.send_message(
                    msg.chat.id,
                    format!(
                        "Created and switched to project '{}'.\n\nProject: {}\nAgent: {}\nModel: {}\nThinking: {}\nSession: {}\nWorkdir: {}",
                        snapshot.active_project,
                        active.name,
                        active.agent,
                        active.model,
                        active.thinking.clone().unwrap_or_else(|| "-".to_string()),
                        active.session_id.clone().unwrap_or_else(|| "-".to_string()),
                        active.work_dir
                    ),
                )
                .await?;
            }
        }
        Command::Projects => {
            let snapshot = app.task_service.get_context(msg.chat.id.0, None).await;
            let mut lines = vec![
                format!("Active project: {}", snapshot.active_project),
                "".into(),
                "Projects:".into(),
            ];
            for p in snapshot.projects {
                let marker = if p.name == snapshot.active_project {
                    "*"
                } else {
                    "-"
                };
                lines.push(format!(
                    "{} {} | agent={} | model={} | thinking={} | session={}",
                    marker,
                    p.name,
                    p.agent,
                    p.model,
                    p.thinking.unwrap_or_else(|| "-".to_string()),
                    p.session_id.unwrap_or_else(|| "-".to_string())
                ));
            }
            lines.push("".into());
            lines.push("Use /new <name> to create, /use <name> to switch.".into());
            bot.send_message(
                msg.chat.id,
                fit_telegram_text(&lines.join("\n"), MAX_TELEGRAM_TEXT),
            )
            .reply_markup(InlineKeyboardMarkup::new(vec![
                vec![
                    InlineKeyboardButton::callback("Switch Project", action("p:switch:0")),
                    InlineKeyboardButton::callback("Open Menu", action("m:projects")),
                ],
                vec![InlineKeyboardButton::callback(
                    "Delete This Project",
                    action("p:delthis"),
                )],
            ]))
            .await?;
        }
        Command::Del(args) => {
            let normalized = args.trim().to_lowercase();
            if normalized.is_empty()
                || normalized == "this"
                || normalized == "this-project"
                || normalized == "this project"
            {
                let snapshot = app.task_service.get_context(msg.chat.id.0, None).await;
                let active_name = snapshot.active_project.clone();
                let idx = snapshot
                    .projects
                    .iter()
                    .position(|p| p.name == active_name)
                    .unwrap_or(0);
                let page = idx / PROJECT_PAGE_SIZE;
                let target = snapshot
                    .projects
                    .get(idx)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("No project found."))?;

                bot.send_message(
                    msg.chat.id,
                    format!(
                        "Delete project '{}' ?\nThis will remove project data and delete folder:\n{}",
                        target.name, target.work_dir
                    ),
                )
                .reply_markup(project_delete_confirm_menu(idx, page))
                .await?;
            } else {
                bot.send_message(
                    msg.chat.id,
                    "Usage: /del this project\n(also supports /del this or /del)",
                )
                .await?;
            }
        }
        Command::Use(name) => {
            let name = name.trim();
            if name.is_empty() {
                let snapshot = app.task_service.get_context(msg.chat.id.0, None).await;
                let names = snapshot
                    .projects
                    .into_iter()
                    .map(|p| p.name)
                    .collect::<Vec<_>>();
                bot.send_message(msg.chat.id, "Switch project (1/1)")
                    .reply_markup(project_switch_menu(&names, &snapshot.active_project, 0))
                    .await?;
            } else {
                app.task_service
                    .use_project(msg.chat.id.0, None, name)
                    .await?;
                let active = app
                    .task_service
                    .get_active_project(msg.chat.id.0, None)
                    .await;
                bot.send_message(
                    msg.chat.id,
                    format!("Switched to project '{}'.", active.name),
                )
                .await?;
            }
        }
        Command::Run(payload) => {
            let payload = payload.trim();
            if payload.is_empty() {
                set_pending(&app, &msg, PendingInputKind::RunTask, None).await;
                bot.send_message(
                    msg.chat.id,
                    "Send your task prompt in the next message. Type 'cancel' to abort.",
                )
                .await?;
            } else {
                let (task, mode_override) = parse_run_payload(payload);
                if task.is_empty() {
                    set_pending(&app, &msg, PendingInputKind::RunTask, mode_override).await;
                    bot.send_message(msg.chat.id, format!("Send your task prompt in the next message. Mode: {:?}. Type 'cancel' to abort.", mode_override))
                        .await?;
                } else {
                    trigger_run(&bot, &app, &msg, task, mode_override).await?;
                }
            }
        }
        Command::Status => send_status_chat(&bot, &app, msg.chat.id).await?,
        Command::Cancel | Command::Interrupt => {
            let snapshot = app.task_service.cancel(msg.chat.id.0, None, None).await?;
            bot.send_message(msg.chat.id, task_to_text(&snapshot))
                .parse_mode(ParseMode::Html)
                .await?;
        }
        Command::Agent(args) => {
            handle_agent_command(&bot, &app, msg.chat.id, Some(&msg), &args).await?
        }
        Command::Model(args) => {
            handle_model_command(&bot, &app, msg.chat.id, Some(&msg), &args).await?
        }
        Command::Thinking(args) => {
            handle_thinking_command(&bot, &app, msg.chat.id, Some(&msg), &args).await?
        }
        Command::Mode(raw) => {
            let raw = raw.trim().to_lowercase();
            if raw == "plan" {
                let updated = app
                    .task_service
                    .set_agent(msg.chat.id.0, None, "plan")
                    .await?;
                bot.send_message(
                    msg.chat.id,
                    format!(
                        "Mode switched to plan (agent={}) in project '{}'.",
                        updated.agent, updated.name
                    ),
                )
                .await?;
            } else if raw == "build" || raw == "bulid" {
                let updated = app
                    .task_service
                    .set_agent(msg.chat.id.0, None, "build")
                    .await?;
                bot.send_message(
                    msg.chat.id,
                    format!(
                        "Mode switched to build (agent={}) in project '{}'.",
                        updated.agent, updated.name
                    ),
                )
                .await?;
            } else {
                let active = app
                    .task_service
                    .get_active_project(msg.chat.id.0, None)
                    .await;
                bot.send_message(
                    msg.chat.id,
                    format!(
                        "Mode\nCurrent project: {}\nCurrent mode: {:?}",
                        active.name,
                        RunMode::from_agent(&active.agent)
                    ),
                )
                .reply_markup(mode_menu(RunMode::from_agent(&active.agent)))
                .await?;
            }
        }
    }
    Ok(())
}

async fn handle_plain_text(bot: Bot, msg: Message, app: App) -> Result<()> {
    let Some(text) = msg.text() else {
        return Ok(());
    };
    if text.starts_with('/') {
        return Ok(());
    }

    if let Some(pending) = take_pending(&app, &msg).await {
        if text.eq_ignore_ascii_case("cancel") {
            let mut map = app.pending.lock().await;
            map.remove(&pending_key(&msg));
            bot.send_message(msg.chat.id, "Pending input cancelled.")
                .await?;
            return Ok(());
        }

        match pending.kind {
            PendingInputKind::NewProject => {
                app.task_service
                    .create_project(msg.chat.id.0, None, text)
                    .await?;
                bot.send_message(msg.chat.id, "Project created.").await?;
            }
            PendingInputKind::RunTask => {
                trigger_run(&bot, &app, &msg, text.to_string(), pending.mode_override).await?;
            }
            PendingInputKind::SetModel => {
                let updated = app
                    .task_service
                    .set_model(msg.chat.id.0, None, text)
                    .await?;
                bot.send_message(
                    msg.chat.id,
                    format!(
                        "Model updated to '{}' in project '{}'.",
                        updated.model, updated.name
                    ),
                )
                .await?;
            }
            PendingInputKind::SetAgent => {
                let updated = app
                    .task_service
                    .set_agent(msg.chat.id.0, None, text)
                    .await?;
                bot.send_message(
                    msg.chat.id,
                    format!(
                        "Agent updated to '{}' in project '{}'.",
                        updated.agent, updated.name
                    ),
                )
                .await?;
            }
            PendingInputKind::SetThinking => {
                let value =
                    if text.eq_ignore_ascii_case("off") || text.eq_ignore_ascii_case("clear") {
                        None
                    } else {
                        Some(text)
                    };
                app.task_service
                    .set_thinking(msg.chat.id.0, None, value)
                    .await?;
                bot.send_message(msg.chat.id, "Thinking updated.").await?;
            }
        }
        let mut map = app.pending.lock().await;
        map.remove(&pending_key(&msg));
        return Ok(());
    }

    if text.eq_ignore_ascii_case("ping") {
        bot.send_message(msg.chat.id, "pong").await?;
        return Ok(());
    }

    trigger_run(&bot, &app, &msg, text.to_string(), None).await
}

async fn trigger_run(
    bot: &Bot,
    app: &App,
    msg: &Message,
    task: String,
    mode: Option<RunMode>,
) -> Result<()> {
    if task.trim().is_empty() {
        bot.send_message(
            msg.chat.id,
            "Usage: /run <task> or /run plan <task> or /run build <task>",
        )
        .await?;
        return Ok(());
    }

    let snapshot = app
        .task_service
        .run(
            msg.chat.id.0,
            None,
            task.trim().to_string(),
            RunTaskOptions {
                mode,
                agent: mode.map(|m| match m {
                    RunMode::Build => "build".to_string(),
                    RunMode::Plan => "plan".to_string(),
                }),
                ..Default::default()
            },
        )
        .await?;

    let status = bot
        .send_message(
            msg.chat.id,
            format!(
                "Task accepted in project '{}'. Streaming output...",
                snapshot.project
            ),
        )
        .await?;
    let output = bot
        .send_message(msg.chat.id, "<pre>Waiting for output...</pre>")
        .parse_mode(ParseMode::Html)
        .await?;
    start_monitor(
        bot.clone(),
        app.clone(),
        msg.chat.id,
        status.id,
        output.id,
        snapshot.project,
    )
    .await;
    Ok(())
}

async fn send_status_chat(bot: &Bot, app: &App, chat_id: ChatId) -> Result<()> {
    let snapshot =
        if let Some(running) = app.task_service.get_running_snapshot(chat_id.0, None).await {
            running
        } else {
            let active = app.task_service.get_active_project(chat_id.0, None).await;
            app.task_service
                .get_snapshot(chat_id.0, None, Some(&active.name))
                .await?
        };

    bot.send_message(chat_id, task_to_text(&snapshot))
        .parse_mode(ParseMode::Html)
        .await?;
    let out = normalized_output(&snapshot);
    for chunk in split_output_chunks(&out, OUTPUT_STREAM_CHUNK) {
        if chunk.trim().is_empty() {
            continue;
        }
        bot.send_message(
            chat_id,
            format!("<pre>{}</pre>", html_escape::encode_text(&chunk)),
        )
        .parse_mode(ParseMode::Html)
        .await?;
    }
    Ok(())
}

async fn handle_agent_command(
    bot: &Bot,
    app: &App,
    chat_id: ChatId,
    msg: Option<&Message>,
    args: &str,
) -> Result<()> {
    let args = command_args(args);
    if args.is_empty() {
        let active = app.task_service.get_active_project(chat_id.0, None).await;
        bot.send_message(
            chat_id,
            format!("Current agent: {} (project: {})", active.agent, active.name),
        )
        .await?;
        return Ok(());
    }
    match args[0].to_lowercase().as_str() {
        "list" => {
            let active = app.task_service.get_active_project(chat_id.0, None).await;
            let (ok, stdout, stderr) = app
                .runner
                .run_simple(&["agent", "list"], &PathBuf::from(active.work_dir))
                .await?;
            if !ok {
                bot.send_message(
                    chat_id,
                    format!(
                        "Failed to list agents.\n{}",
                        fit_telegram_text(&strip_ansi(&(stderr + &stdout)), MAX_TELEGRAM_TEXT)
                    ),
                )
                .await?;
                return Ok(());
            }
            let agents = parse_agent_list(&stdout);
            if agents.is_empty() {
                bot.send_message(
                    chat_id,
                    fit_telegram_text(&strip_ansi(&stdout), MAX_TELEGRAM_TEXT),
                )
                .await?;
            } else {
                let mut lines = vec!["Available agents:".to_string()];
                lines.extend(
                    agents
                        .iter()
                        .take(MAX_LIST_ITEMS)
                        .map(|a| format!("- {}", a)),
                );
                if agents.len() > MAX_LIST_ITEMS {
                    lines.push(format!("... and {} more", agents.len() - MAX_LIST_ITEMS));
                }
                bot.send_message(
                    chat_id,
                    fit_telegram_text(&lines.join("\n"), MAX_TELEGRAM_TEXT),
                )
                .await?;
            }
        }
        "set" => {
            let value = args[1..].join(" ");
            if value.trim().is_empty() {
                if let Some(m) = msg {
                    set_pending(app, m, PendingInputKind::SetAgent, None).await;
                }
                bot.send_message(
                    chat_id,
                    "Send the agent name in your next message. Type 'cancel' to abort.",
                )
                .await?;
            } else {
                let updated = app
                    .task_service
                    .set_agent(chat_id.0, None, value.trim())
                    .await?;
                bot.send_message(
                    chat_id,
                    format!(
                        "Agent updated to '{}' in project '{}'.\nMode now: {:?}.",
                        updated.agent,
                        updated.name,
                        RunMode::from_agent(&updated.agent)
                    ),
                )
                .await?;
            }
        }
        _ => {
            bot.send_message(chat_id, "Usage: /agent list OR /agent set <name>")
                .await?;
        }
    }
    Ok(())
}

async fn fetch_models(app: &App, chat_id: i64, force: bool) -> Result<Vec<String>> {
    let active = app.task_service.get_active_project(chat_id, None).await;
    let key = format!("{}|{}|{}", chat_id, "main", active.name);
    if !force {
        if let Some(entry) = app.model_cache.lock().await.get(&key).cloned() {
            if util::now_ms() - entry.updated_at < 5 * 60 * 1000 {
                return Ok(entry.models);
            }
        }
    }

    let (ok, stdout, stderr) = app
        .runner
        .run_simple(&["models"], &PathBuf::from(active.work_dir.clone()))
        .await?;
    if !ok {
        return Err(anyhow::anyhow!(
            "Failed to list models.\n{}",
            fit_telegram_text(&strip_ansi(&(stderr + &stdout)), MAX_TELEGRAM_TEXT)
        ));
    }
    let models = parse_model_list(&stdout);
    if models.is_empty() {
        return Err(anyhow::anyhow!("No models found."));
    }
    app.model_cache.lock().await.insert(
        key,
        ModelCacheEntry {
            models: models.clone(),
            updated_at: util::now_ms(),
        },
    );
    Ok(models)
}

fn model_picker(models: &[String], current: &str, page: usize) -> InlineKeyboardMarkup {
    let total_pages = models.len().max(1).div_ceil(MODEL_PAGE_SIZE);
    let page = page.min(total_pages.saturating_sub(1));
    let start = page * MODEL_PAGE_SIZE;
    let mut rows = models
        .iter()
        .enumerate()
        .skip(start)
        .take(MODEL_PAGE_SIZE)
        .map(|(idx, model)| {
            let label = if model == current {
                format!("✓ {}", model)
            } else {
                model.clone()
            };
            vec![InlineKeyboardButton::callback(
                label,
                format!("mdl:use:{}:{}", idx, page),
            )]
        })
        .collect::<Vec<_>>();

    if total_pages > 1 {
        let mut nav = Vec::new();
        if page > 0 {
            nav.push(InlineKeyboardButton::callback(
                "Prev",
                format!("mdl:pick:{}", page - 1),
            ));
        }
        if page + 1 < total_pages {
            nav.push(InlineKeyboardButton::callback(
                "Next",
                format!("mdl:pick:{}", page + 1),
            ));
        }
        if !nav.is_empty() {
            rows.push(nav);
        }
    }

    rows.push(vec![
        InlineKeyboardButton::callback("Refresh", action("mdl:refresh")),
        InlineKeyboardButton::callback("Back", action("m:model")),
    ]);
    InlineKeyboardMarkup::new(rows)
}

async fn handle_model_command(
    bot: &Bot,
    app: &App,
    chat_id: ChatId,
    msg: Option<&Message>,
    args: &str,
) -> Result<()> {
    let args = command_args(args);
    if args.is_empty() {
        let active = app.task_service.get_active_project(chat_id.0, None).await;
        let models = fetch_models(app, chat_id.0, false).await?;
        bot.send_message(
            chat_id,
            format!(
                "Select model (1/{})\nCurrent project: {}\nCurrent model: {}",
                models.len().max(1).div_ceil(MODEL_PAGE_SIZE),
                active.name,
                active.model
            ),
        )
        .reply_markup(model_picker(&models, &active.model, 0))
        .await?;
        return Ok(());
    }
    match args[0].to_lowercase().as_str() {
        "pick" => {
            let active = app.task_service.get_active_project(chat_id.0, None).await;
            let models = fetch_models(app, chat_id.0, true).await?;
            bot.send_message(
                chat_id,
                format!(
                    "Select model (1/{})\nCurrent project: {}\nCurrent model: {}",
                    models.len().max(1).div_ceil(MODEL_PAGE_SIZE),
                    active.name,
                    active.model
                ),
            )
            .reply_markup(model_picker(&models, &active.model, 0))
            .await?;
        }
        "list" => {
            let keyword = args[1..].join(" ").to_lowercase();
            let models = fetch_models(app, chat_id.0, true).await?;
            let models = models
                .into_iter()
                .filter(|m| keyword.is_empty() || m.to_lowercase().contains(&keyword))
                .collect::<Vec<_>>();
            if models.is_empty() {
                bot.send_message(
                    chat_id,
                    if keyword.is_empty() {
                        "No models found.".to_string()
                    } else {
                        format!("No models matching '{}'.", keyword)
                    },
                )
                .await?;
            } else {
                let mut lines = vec![if keyword.is_empty() {
                    "Available models:".to_string()
                } else {
                    format!("Models matching '{}':", keyword)
                }];
                lines.extend(
                    models
                        .iter()
                        .take(MAX_LIST_ITEMS)
                        .map(|m| format!("- {}", m)),
                );
                if models.len() > MAX_LIST_ITEMS {
                    lines.push(format!(
                        "... and {} more. Add keyword: /model list <keyword>",
                        models.len() - MAX_LIST_ITEMS
                    ));
                }
                bot.send_message(
                    chat_id,
                    fit_telegram_text(&lines.join("\n"), MAX_TELEGRAM_TEXT),
                )
                .await?;
            }
        }
        "set" => {
            let target = args[1..].join(" ");
            if target.trim().is_empty() {
                if let Some(m) = msg {
                    set_pending(app, m, PendingInputKind::SetModel, None).await;
                }
                bot.send_message(chat_id, "Send the model id in your next message (e.g. kimi/moonshot-v1-128k). Type 'cancel' to abort.")
                    .await?;
            } else {
                let updated = app
                    .task_service
                    .set_model(chat_id.0, None, target.trim())
                    .await?;
                bot.send_message(
                    chat_id,
                    format!(
                        "Model updated to '{}' in project '{}'.",
                        updated.model, updated.name
                    ),
                )
                .await?;
            }
        }
        _ => {
            bot.send_message(chat_id, "Usage: /model OR /model pick OR /model list [keyword] OR /model set <provider/model>").await?;
        }
    }
    Ok(())
}

async fn handle_thinking_command(
    bot: &Bot,
    app: &App,
    chat_id: ChatId,
    msg: Option<&Message>,
    args: &str,
) -> Result<()> {
    let args = command_args(args);
    if args.is_empty() {
        let active = app.task_service.get_active_project(chat_id.0, None).await;
        bot.send_message(
            chat_id,
            format!(
                "Current thinking: {} (project: {})",
                active.thinking.unwrap_or_else(|| "off".to_string()),
                active.name
            ),
        )
        .await?;
        return Ok(());
    }
    match args[0].to_lowercase().as_str() {
        "set" => {
            let value = args[1..].join(" ");
            if value.trim().is_empty() {
                if let Some(m) = msg {
                    set_pending(app, m, PendingInputKind::SetThinking, None).await;
                }
                bot.send_message(chat_id, "Send thinking value (minimal|low|medium|high|max|xhigh) or 'off'. Type 'cancel' to abort.")
                    .await?;
            } else {
                app.task_service
                    .set_thinking(chat_id.0, None, Some(value.trim()))
                    .await?;
                bot.send_message(chat_id, "Thinking updated.").await?;
            }
        }
        "off" | "clear" => {
            app.task_service.set_thinking(chat_id.0, None, None).await?;
            bot.send_message(chat_id, "Thinking disabled.").await?;
        }
        _ => {
            bot.send_message(chat_id, "Usage: /thinking set <variant> OR /thinking off")
                .await?;
        }
    }
    Ok(())
}

async fn handle_callback(bot: Bot, q: CallbackQuery, app: App) -> Result<()> {
    let Some(message) = q.message.clone() else {
        return Ok(());
    };
    let Some(data) = q.data.clone() else {
        return Ok(());
    };
    let chat_id = message.chat().id;
    let message_id = Some(message.id());
    let _ = bot.answer_callback_query(q.id.clone()).await;

    if data == "m:home" {
        send_menu_or_edit(
            &bot,
            Some(&q),
            chat_id,
            message_id,
            "Main menu\nChoose an action:".to_string(),
            main_menu(),
        )
        .await?;
        return Ok(());
    }
    if data == "m:projects" {
        let active = app.task_service.get_active_project(chat_id.0, None).await;
        send_menu_or_edit(
            &bot,
            Some(&q),
            chat_id,
            message_id,
            format!(
                "Projects\nCurrent project: {}\nUse buttons for create/switch/list/delete.",
                active.name
            ),
            project_menu(),
        )
        .await?;
        return Ok(());
    }
    if data == "m:run" {
        send_menu_or_edit(
            &bot,
            Some(&q),
            chat_id,
            message_id,
            "Run tasks\nChoose a mode and send your task in the next message.".to_string(),
            run_menu(),
        )
        .await?;
        return Ok(());
    }
    if data == "m:model" {
        let active = app.task_service.get_active_project(chat_id.0, None).await;
        send_menu_or_edit(
            &bot,
            Some(&q),
            chat_id,
            message_id,
            format!("Model settings\nCurrent project: {}\nCurrent model: {}\nUse buttons to pick a model.", active.name, active.model),
            model_menu(),
        )
        .await?;
        return Ok(());
    }
    if data == "m:agent" {
        let active = app.task_service.get_active_project(chat_id.0, None).await;
        send_menu_or_edit(
            &bot,
            Some(&q),
            chat_id,
            message_id,
            format!(
                "Agent settings\nCurrent project: {}\nCurrent agent: {}",
                active.name, active.agent
            ),
            agent_menu(),
        )
        .await?;
        return Ok(());
    }
    if data == "m:thinking" {
        let active = app.task_service.get_active_project(chat_id.0, None).await;
        send_menu_or_edit(
            &bot,
            Some(&q),
            chat_id,
            message_id,
            format!(
                "Thinking strength\nCurrent project: {}\nCurrent thinking: {}",
                active.name,
                active.thinking.clone().unwrap_or_else(|| "off".to_string())
            ),
            thinking_menu(active.thinking.as_deref()),
        )
        .await?;
        return Ok(());
    }
    if data == "m:mode" {
        let active = app.task_service.get_active_project(chat_id.0, None).await;
        send_menu_or_edit(
            &bot,
            Some(&q),
            chat_id,
            message_id,
            format!(
                "Mode\nCurrent project: {}\nCurrent mode: {:?}",
                active.name,
                RunMode::from_agent(&active.agent)
            ),
            mode_menu(RunMode::from_agent(&active.agent)),
        )
        .await?;
        return Ok(());
    }
    if data == "m:status" {
        send_status_chat(&bot, &app, chat_id).await?;
        return Ok(());
    }
    if data == "m:cancel" {
        match app.task_service.cancel(chat_id.0, None, None).await {
            Ok(snapshot) => {
                bot.send_message(chat_id, task_to_text(&snapshot))
                    .parse_mode(ParseMode::Html)
                    .await?;
            }
            Err(err) => {
                bot.send_message(chat_id, err.to_string()).await?;
            }
        }
        return Ok(());
    }

    if data == "p:new" {
        set_pending_raw(
            &app,
            chat_id.0,
            q.from.id.0,
            PendingInputKind::NewProject,
            None,
        )
        .await;
        bot.send_message(
            chat_id,
            "Send the new project name in your next message. Type 'cancel' to abort.",
        )
        .await?;
        return Ok(());
    }
    if data == "p:list" {
        let snapshot = app.task_service.get_context(chat_id.0, None).await;
        let mut lines = vec![
            format!("Active project: {}", snapshot.active_project),
            "".into(),
            "Projects:".into(),
        ];
        for p in snapshot.projects {
            let marker = if p.name == snapshot.active_project {
                "*"
            } else {
                "-"
            };
            lines.push(format!(
                "{} {} | agent={} | model={} | thinking={} | session={}",
                marker,
                p.name,
                p.agent,
                p.model,
                p.thinking.unwrap_or_else(|| "-".to_string()),
                p.session_id.unwrap_or_else(|| "-".to_string())
            ));
        }
        bot.send_message(
            chat_id,
            fit_telegram_text(&lines.join("\n"), MAX_TELEGRAM_TEXT),
        )
        .await?;
        return Ok(());
    }
    if data == "p:delthis" {
        let snapshot = app.task_service.get_context(chat_id.0, None).await;
        let active_name = snapshot.active_project.clone();
        let idx = snapshot
            .projects
            .iter()
            .position(|p| p.name == active_name)
            .unwrap_or(0);
        let page = idx / PROJECT_PAGE_SIZE;

        if let Some(target) = snapshot.projects.get(idx) {
            send_menu_or_edit(
                &bot,
                Some(&q),
                chat_id,
                message_id,
                format!(
                    "Delete project '{}' ?\nThis will remove project data and delete folder:\n{}",
                    target.name, target.work_dir
                ),
                project_delete_confirm_menu(idx, page),
            )
            .await?;
        }
        return Ok(());
    }
    if let Some(page_raw) = data.strip_prefix("p:switch:") {
        let page = page_raw.parse::<usize>().unwrap_or(0);
        let snapshot = app.task_service.get_context(chat_id.0, None).await;
        let names = snapshot
            .projects
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<_>>();
        send_menu_or_edit(
            &bot,
            Some(&q),
            chat_id,
            message_id,
            format!(
                "Switch project\nCurrent project: {}",
                snapshot.active_project
            ),
            project_switch_menu(&names, &snapshot.active_project, page),
        )
        .await?;
        return Ok(());
    }
    if let Some(payload) = data.strip_prefix("p:use:") {
        let parts = payload.split(':').collect::<Vec<_>>();
        if parts.len() == 2 {
            let idx = parts[0].parse::<usize>().unwrap_or(usize::MAX);
            let page = parts[1].parse::<usize>().unwrap_or(0);
            let snapshot = app.task_service.get_context(chat_id.0, None).await;
            if let Some(target) = snapshot.projects.get(idx) {
                app.task_service
                    .use_project(chat_id.0, None, &target.name)
                    .await?;
            }
            let next = app.task_service.get_context(chat_id.0, None).await;
            let names = next
                .projects
                .iter()
                .map(|p| p.name.clone())
                .collect::<Vec<_>>();
            send_menu_or_edit(
                &bot,
                Some(&q),
                chat_id,
                message_id,
                format!("Switch project\nCurrent project: {}", next.active_project),
                project_switch_menu(&names, &next.active_project, page),
            )
            .await?;
        }
        return Ok(());
    }

    if let Some(page_raw) = data.strip_prefix("p:delete:") {
        let page = page_raw.parse::<usize>().unwrap_or(0);
        let snapshot = app.task_service.get_context(chat_id.0, None).await;
        let names = snapshot
            .projects
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<_>>();
        send_menu_or_edit(
            &bot,
            Some(&q),
            chat_id,
            message_id,
            format!(
                "Delete project\nCurrent project: {}\nSelect one to continue.",
                snapshot.active_project
            ),
            project_delete_menu(&names, &snapshot.active_project, page),
        )
        .await?;
        return Ok(());
    }
    if let Some(payload) = data.strip_prefix("p:delpick:") {
        let parts = payload.split(':').collect::<Vec<_>>();
        if parts.len() == 2 {
            let idx = parts[0].parse::<usize>().unwrap_or(usize::MAX);
            let page = parts[1].parse::<usize>().unwrap_or(0);
            let snapshot = app.task_service.get_context(chat_id.0, None).await;
            if let Some(target) = snapshot.projects.get(idx) {
                send_menu_or_edit(
                    &bot,
                    Some(&q),
                    chat_id,
                    message_id,
                    format!(
                        "Delete project '{}' ?\nThis will remove project data and delete folder:\n{}",
                        target.name, target.work_dir
                    ),
                    project_delete_confirm_menu(idx, page),
                )
                .await?;
            }
        }
        return Ok(());
    }
    if let Some(payload) = data.strip_prefix("p:delconfirm:") {
        let parts = payload.split(':').collect::<Vec<_>>();
        if parts.len() == 2 {
            let idx = parts[0].parse::<usize>().unwrap_or(usize::MAX);
            let page = parts[1].parse::<usize>().unwrap_or(0);
            let snapshot = app.task_service.get_context(chat_id.0, None).await;
            if snapshot.projects.len() <= 1 {
                bot.send_message(chat_id, "Cannot delete the last project.")
                    .await?;
            } else if let Some(target) = snapshot.projects.get(idx) {
                if let Some(running) = app.task_service.get_running_snapshot(chat_id.0, None).await
                {
                    if running.project == target.name {
                        bot.send_message(
                            chat_id,
                            format!(
                                "Project '{}' has a running task. Use /cancel first.",
                                target.name
                            ),
                        )
                        .await?;
                    } else {
                        let _ = tokio::fs::remove_dir_all(&target.work_dir).await;
                        let (ctx, deleted) = app
                            .task_service
                            .delete_project(chat_id.0, None, &target.name)
                            .await?;
                        app.model_cache
                            .lock()
                            .await
                            .remove(&format!("{}|{}|{}", chat_id.0, "main", deleted.name));
                        bot.send_message(
                            chat_id,
                            format!(
                                "Deleted project '{}'.\nNew active project: {}",
                                deleted.name, ctx.active_project
                            ),
                        )
                        .await?;
                    }
                } else {
                    let _ = tokio::fs::remove_dir_all(&target.work_dir).await;
                    let (ctx, deleted) = app
                        .task_service
                        .delete_project(chat_id.0, None, &target.name)
                        .await?;
                    app.model_cache
                        .lock()
                        .await
                        .remove(&format!("{}|{}|{}", chat_id.0, "main", deleted.name));
                    bot.send_message(
                        chat_id,
                        format!(
                            "Deleted project '{}'.\nNew active project: {}",
                            deleted.name, ctx.active_project
                        ),
                    )
                    .await?;
                }
            }

            let next = app.task_service.get_context(chat_id.0, None).await;
            let names = next
                .projects
                .iter()
                .map(|p| p.name.clone())
                .collect::<Vec<_>>();
            send_menu_or_edit(
                &bot,
                Some(&q),
                chat_id,
                message_id,
                format!(
                    "Delete project\nCurrent project: {}\nSelect one to continue.",
                    next.active_project
                ),
                project_delete_menu(&names, &next.active_project, page),
            )
            .await?;
        }
        return Ok(());
    }
    if let Some(page_raw) = data.strip_prefix("p:delcancel:") {
        let page = page_raw.parse::<usize>().unwrap_or(0);
        let snapshot = app.task_service.get_context(chat_id.0, None).await;
        let names = snapshot
            .projects
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<_>>();
        send_menu_or_edit(
            &bot,
            Some(&q),
            chat_id,
            message_id,
            format!(
                "Delete project\nCurrent project: {}\nSelect one to continue.",
                snapshot.active_project
            ),
            project_delete_menu(&names, &snapshot.active_project, page),
        )
        .await?;
        return Ok(());
    }

    if data == "r:input" || data == "r:input:plan" || data == "r:input:build" {
        let mode = if data == "r:input:plan" {
            Some(RunMode::Plan)
        } else if data == "r:input:build" {
            Some(RunMode::Build)
        } else {
            None
        };
        set_pending_raw(
            &app,
            chat_id.0,
            q.from.id.0,
            PendingInputKind::RunTask,
            mode,
        )
        .await;
        bot.send_message(
            chat_id,
            format!(
                "Send your task prompt in the next message. Mode: {:?}. Type 'cancel' to abort.",
                mode
            ),
        )
        .await?;
        return Ok(());
    }

    if data == "mdl:list" {
        handle_model_command(&bot, &app, chat_id, None, "list").await?;
        return Ok(());
    }
    if data == "mdl:set" {
        set_pending_raw(
            &app,
            chat_id.0,
            q.from.id.0,
            PendingInputKind::SetModel,
            None,
        )
        .await;
        bot.send_message(chat_id, "Send the model id in your next message (e.g. kimi/moonshot-v1-128k). Type 'cancel' to abort.")
            .await?;
        return Ok(());
    }
    if data == "mdl:refresh" || data.starts_with("mdl:pick:") {
        let page = data
            .strip_prefix("mdl:pick:")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);
        let active = app.task_service.get_active_project(chat_id.0, None).await;
        let models = fetch_models(&app, chat_id.0, data == "mdl:refresh").await?;
        send_menu_or_edit(
            &bot,
            Some(&q),
            chat_id,
            message_id,
            format!(
                "Select model\nCurrent project: {}\nCurrent model: {}",
                active.name, active.model
            ),
            model_picker(&models, &active.model, page),
        )
        .await?;
        return Ok(());
    }
    if let Some(payload) = data.strip_prefix("mdl:use:") {
        let parts = payload.split(':').collect::<Vec<_>>();
        if parts.len() == 2 {
            let idx = parts[0].parse::<usize>().unwrap_or(usize::MAX);
            let page = parts[1].parse::<usize>().unwrap_or(0);
            let models = fetch_models(&app, chat_id.0, false).await?;
            if let Some(model) = models.get(idx) {
                let updated = app.task_service.set_model(chat_id.0, None, model).await?;
                bot.send_message(
                    chat_id,
                    format!(
                        "Model updated to '{}' in project '{}'.",
                        updated.model, updated.name
                    ),
                )
                .await?;
            }
            let active = app.task_service.get_active_project(chat_id.0, None).await;
            send_menu_or_edit(
                &bot,
                Some(&q),
                chat_id,
                message_id,
                format!(
                    "Select model\nCurrent project: {}\nCurrent model: {}",
                    active.name, active.model
                ),
                model_picker(&models, &active.model, page),
            )
            .await?;
        }
        return Ok(());
    }

    if data == "agt:list" {
        handle_agent_command(&bot, &app, chat_id, None, "list").await?;
        return Ok(());
    }
    if data == "agt:set" {
        set_pending_raw(
            &app,
            chat_id.0,
            q.from.id.0,
            PendingInputKind::SetAgent,
            None,
        )
        .await;
        bot.send_message(
            chat_id,
            "Send the agent name in your next message. Type 'cancel' to abort.",
        )
        .await?;
        return Ok(());
    }

    if data == "th:off" || data.starts_with("th:set:") {
        if data == "th:off" {
            app.task_service.set_thinking(chat_id.0, None, None).await?;
        } else {
            let value = data.trim_start_matches("th:set:");
            app.task_service
                .set_thinking(chat_id.0, None, Some(value))
                .await?;
        }
        let active = app.task_service.get_active_project(chat_id.0, None).await;
        send_menu_or_edit(
            &bot,
            Some(&q),
            chat_id,
            message_id,
            format!(
                "Thinking strength\nCurrent project: {}\nCurrent thinking: {}",
                active.name,
                active.thinking.clone().unwrap_or_else(|| "off".to_string())
            ),
            thinking_menu(active.thinking.as_deref()),
        )
        .await?;
        return Ok(());
    }

    if data == "mode:build" || data == "mode:plan" {
        let target = if data == "mode:plan" { "plan" } else { "build" };
        app.task_service.set_agent(chat_id.0, None, target).await?;
        let active = app.task_service.get_active_project(chat_id.0, None).await;
        send_menu_or_edit(
            &bot,
            Some(&q),
            chat_id,
            message_id,
            format!(
                "Mode\nCurrent project: {}\nCurrent mode: {:?}",
                active.name,
                RunMode::from_agent(&active.agent)
            ),
            mode_menu(RunMode::from_agent(&active.agent)),
        )
        .await?;
        return Ok(());
    }

    bot.send_message(chat_id, "Unknown action. Use /menu to reload.")
        .await?;
    Ok(())
}
