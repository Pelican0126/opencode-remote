use serde_json::json;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn ensure_dir(path: &Path) {
    let _ = fs::create_dir_all(path);
}

fn resolve_dashscope_base_url(envs: &HashMap<String, String>) -> String {
    if let Some(url) = envs.get("DASHSCOPE_BASE_URL") {
        let v = url.trim();
        if !v.is_empty() {
            return v.to_string();
        }
    }

    let key = envs
        .get("DASHSCOPE_API_KEY")
        .map(|v| v.trim().to_string())
        .unwrap_or_default();
    if key.starts_with("sk-sp-") {
        "https://coding.dashscope.aliyuncs.com/v1".to_string()
    } else {
        "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string()
    }
}

fn seed_auth_file(data_dir: &Path, host_auth_file: Option<&str>) {
    let Some(source) = host_auth_file else {
        return;
    };
    let source = source.trim();
    if source.is_empty() {
        return;
    }
    let src = PathBuf::from(source);
    if !src.exists() {
        return;
    }

    let auth_dir = data_dir.join("opencode");
    let target = auth_dir.join("auth.json");
    ensure_dir(&auth_dir);
    let _ = fs::copy(src, target);
}

fn build_inline_config(dashscope_base_url: &str) -> String {
    json!({
      "$schema": "https://opencode.ai/config.json",
      "provider": {
        "kimi": {
          "npm": "@ai-sdk/openai-compatible",
          "name": "Kimi (custom)",
          "options": {
            "baseURL": "https://api.moonshot.ai/v1",
            "apiKey": "{env:KIMI_API_KEY}"
          },
          "models": {
            "moonshot-v1-128k": {"name": "Moonshot v1 128k"},
            "moonshot-v1-32k": {"name": "Moonshot v1 32k"},
            "moonshot-v1-8k": {"name": "Moonshot v1 8k"}
          }
        },
        "bailian-coding": {
          "npm": "@ai-sdk/openai-compatible",
          "name": "Alibaba Bailian (custom)",
          "options": {
            "baseURL": dashscope_base_url,
            "apiKey": "{env:DASHSCOPE_API_KEY}"
          },
          "models": {
            "glm-5": {"name": "GLM-5"}
          }
        }
      }
    })
    .to_string()
}

pub fn build_isolated_opencode_env(work_dir: &Path) -> HashMap<String, String> {
    let root = work_dir.join(".opencode-runtime");
    let config_dir = root.join("config");
    let state_dir = root.join("state");
    let data_dir = root.join("data");
    let home_dir = root.join("home");

    ensure_dir(&root);
    ensure_dir(&config_dir);
    ensure_dir(&state_dir);
    ensure_dir(&data_dir);
    ensure_dir(&home_dir);

    let mut envs: HashMap<String, String> = env::vars().collect();
    let host_auth = envs.get("OPENCODE_HOST_AUTH_FILE").cloned();
    seed_auth_file(&data_dir, host_auth.as_deref());

    let keys: Vec<String> = envs
        .keys()
        .filter(|k| k.starts_with("OPENCODE"))
        .cloned()
        .collect();
    for key in keys {
        envs.remove(&key);
    }
    envs.remove("AGENT");

    envs.insert(
        "OPENCODE_CONFIG_DIR".to_string(),
        config_dir.display().to_string(),
    );
    envs.insert("OPENCODE_CLIENT".to_string(), "cli".to_string());
    envs.insert(
        "XDG_CONFIG_HOME".to_string(),
        config_dir.display().to_string(),
    );
    envs.insert(
        "XDG_STATE_HOME".to_string(),
        state_dir.display().to_string(),
    );
    envs.insert("XDG_DATA_HOME".to_string(), data_dir.display().to_string());
    envs.insert("HOME".to_string(), home_dir.display().to_string());
    envs.insert("USERPROFILE".to_string(), home_dir.display().to_string());
    envs.insert("APPDATA".to_string(), config_dir.display().to_string());
    envs.insert("LOCALAPPDATA".to_string(), data_dir.display().to_string());

    let base_url = resolve_dashscope_base_url(&envs);
    envs.insert(
        "OPENCODE_CONFIG_CONTENT".to_string(),
        build_inline_config(&base_url),
    );

    envs
}
