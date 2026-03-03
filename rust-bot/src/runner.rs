use crate::runtime_env::build_isolated_opencode_env;
use crate::util::strip_ansi;
use anyhow::Result;
use regex::Regex;
use std::env;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct RunnerRequest {
    pub task: String,
    pub model: String,
    pub work_dir: PathBuf,
    pub agent: Option<String>,
    pub thinking: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RunnerResult {
    pub exit_code: Option<i32>,
    pub signal: Option<String>,
    pub output: String,
    pub error_message: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OpenCodeRunner {
    bin: String,
    base_args: Vec<String>,
}

impl OpenCodeRunner {
    pub fn new() -> Self {
        let fallback_bin = if cfg!(windows) {
            "opencode.cmd"
        } else {
            "opencode"
        };
        let bin = env::var("OPENCODE_BIN").unwrap_or_else(|_| fallback_bin.to_string());
        let base_args = env::var("OPENCODE_EXTRA_ARGS")
            .unwrap_or_default()
            .split_whitespace()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        Self { bin, base_args }
    }

    pub async fn execute(
        &self,
        request: RunnerRequest,
        token: CancellationToken,
        out_tx: mpsc::UnboundedSender<String>,
    ) -> RunnerResult {
        let mut args = self.base_args.clone();
        args.extend([
            "run".to_string(),
            "--model".to_string(),
            request.model.clone(),
        ]);
        if let Some(agent) = request.agent.clone().filter(|v| !v.trim().is_empty()) {
            args.extend(["--agent".to_string(), agent]);
        }
        if let Some(thinking) = request.thinking.clone().filter(|v| !v.trim().is_empty()) {
            args.extend(["--variant".to_string(), thinking]);
        }
        if let Some(session) = request.session_id.clone().filter(|v| !v.trim().is_empty()) {
            args.extend(["--session".to_string(), session]);
        }
        args.push(request.task.clone());

        let envs = build_isolated_opencode_env(&request.work_dir);
        let mut cmd = Command::new(&self.bin);
        cmd.args(args)
            .current_dir(&request.work_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        for (k, v) in envs {
            cmd.env(k, v);
        }

        let child = cmd.spawn();
        let mut child = match child {
            Ok(v) => v,
            Err(err) => {
                let message = format!("Runner spawn error: {}\n", err);
                let _ = out_tx.send(message.clone());
                return RunnerResult {
                    exit_code: Some(1),
                    signal: None,
                    output: message,
                    error_message: Some(err.to_string()),
                    session_id: request.session_id,
                };
            }
        };

        let mut output = String::new();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut stderr = BufReader::new(child.stderr.take().unwrap());

        let mut stdout_buf = vec![0_u8; 4096];
        let mut stderr_buf = vec![0_u8; 4096];

        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    let _ = child.kill().await;
                    let _ = out_tx.send("\nTask cancelled.\n".to_string());
                    output.push_str("\nTask cancelled.\n");
                    break;
                }
                read = stdout.read(&mut stdout_buf) => {
                    if let Ok(n) = read {
                        if n > 0 {
                            let text = String::from_utf8_lossy(&stdout_buf[..n]).to_string();
                            output.push_str(&text);
                            let _ = out_tx.send(text);
                        }
                    }
                }
                read = stderr.read(&mut stderr_buf) => {
                    if let Ok(n) = read {
                        if n > 0 {
                            let text = String::from_utf8_lossy(&stderr_buf[..n]).to_string();
                            output.push_str(&text);
                            let _ = out_tx.send(text);
                        }
                    }
                }
                status = child.wait() => {
                    let status = status.ok();
                    let exit_code = status.and_then(|s| s.code());
                    let signal = None;
                    let session_id = self.get_latest_session_id(&request.work_dir).await.ok().flatten().or(request.session_id);
                    return RunnerResult {
                        exit_code,
                        signal,
                        output,
                        error_message: None,
                        session_id,
                    };
                }
            }
        }

        let session_id = self
            .get_latest_session_id(&request.work_dir)
            .await
            .ok()
            .flatten()
            .or(request.session_id);
        RunnerResult {
            exit_code: None,
            signal: None,
            output,
            error_message: None,
            session_id,
        }
    }

    pub async fn run_simple(
        &self,
        args: &[&str],
        work_dir: &PathBuf,
    ) -> Result<(bool, String, String)> {
        let envs = build_isolated_opencode_env(work_dir);
        let mut cmd = Command::new(&self.bin);
        cmd.args(args)
            .current_dir(work_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in envs {
            cmd.env(k, v);
        }

        let out = cmd.output().await?;
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        Ok((out.status.success(), stdout, stderr))
    }

    async fn get_latest_session_id(&self, work_dir: &PathBuf) -> Result<Option<String>> {
        let (ok, stdout, _) = self
            .run_simple(
                &["session", "list", "--format", "json", "-n", "30"],
                work_dir,
            )
            .await?;
        if !ok {
            return Ok(None);
        }

        let entries: serde_json::Value = serde_json::from_str(&stdout)?;
        let Some(arr) = entries.as_array() else {
            return Ok(None);
        };

        let target = work_dir.to_string_lossy().replace('\\', "/").to_lowercase();
        for item in arr {
            let id = item.get("id").and_then(|v| v.as_str());
            let dir = item.get("directory").and_then(|v| v.as_str());
            if let (Some(id), Some(dir)) = (id, dir) {
                if dir.replace('\\', "/").to_lowercase() == target {
                    return Ok(Some(id.to_string()));
                }
            }
        }
        Ok(None)
    }
}

pub fn parse_model_list(output: &str) -> Vec<String> {
    let cleaned = strip_ansi(output);
    let re = Regex::new(r"^[a-z0-9._-]+/[a-z0-9._-]+$").expect("regex");
    cleaned
        .lines()
        .map(str::trim)
        .filter(|line| re.is_match(line))
        .map(ToString::to_string)
        .collect()
}

pub fn parse_agent_list(output: &str) -> Vec<String> {
    let cleaned = strip_ansi(output);
    let re = Regex::new(r"^([a-z0-9-]+)\s+\((primary|subagent)\)$").expect("regex");
    cleaned
        .lines()
        .filter_map(|line| {
            re.captures(line.trim())
                .map(|caps| format!("{} ({})", &caps[1], &caps[2]))
        })
        .collect()
}
