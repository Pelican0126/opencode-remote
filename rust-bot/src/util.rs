use anyhow::{anyhow, Result};

pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn context_key(chat_id: i64, thread_id: Option<i32>) -> String {
    format!(
        "{}:{}",
        chat_id,
        thread_id.map_or_else(|| "main".to_string(), |v| v.to_string())
    )
}

pub fn task_key(ctx_key: &str, project: &str) -> String {
    format!("{}|{}", ctx_key, project)
}

pub fn normalize_project_name(raw: &str) -> Result<String> {
    let normalized = raw
        .trim()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
        .replace(['\\', '/', ':', '*', '?', '"', '<', '>', '|'], "-");

    let mut squashed = normalized;
    while squashed.contains("--") {
        squashed = squashed.replace("--", "-");
    }
    let squashed = squashed.trim_matches('-').to_string();
    if squashed.is_empty() {
        return Err(anyhow!("Project name cannot be empty."));
    }
    if squashed.len() > 64 {
        return Err(anyhow!("Project name is too long (max 64 chars)."));
    }
    Ok(squashed)
}

pub fn normalize_thinking(value: Option<&str>) -> Option<String> {
    let value = value.unwrap_or("").trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

pub fn fit_telegram_text(input: &str, max: usize) -> String {
    if input.chars().count() <= max {
        return input.to_string();
    }
    input.chars().skip(input.chars().count() - max).collect()
}

pub fn strip_ansi(input: &str) -> String {
    // Covers the output format used by opencode.
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b {
            i += 1;
            if i < bytes.len() && bytes[i] == b'[' {
                i += 1;
                while i < bytes.len() && !bytes[i].is_ascii_alphabetic() {
                    i += 1;
                }
                if i < bytes.len() {
                    i += 1;
                }
            }
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}
