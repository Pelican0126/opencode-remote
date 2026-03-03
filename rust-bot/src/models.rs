use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Idle,
    Running,
    Success,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RunMode {
    Build,
    Plan,
}

impl RunMode {
    pub fn from_agent(agent: &str) -> Self {
        if agent.trim().eq_ignore_ascii_case("plan") {
            Self::Plan
        } else {
            Self::Build
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSnapshot {
    pub key: String,
    pub context_key: String,
    pub project: String,
    pub command: Option<String>,
    pub status: TaskStatus,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub exit_code: Option<i32>,
    pub signal: Option<String>,
    pub output: String,
    pub attempted_models: Vec<String>,
    pub last_run_model: Option<String>,
    pub fallback_used: bool,
    pub model: String,
    pub agent: String,
    pub mode: RunMode,
    pub thinking: Option<String>,
    pub session_id: Option<String>,
    pub version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSettings {
    pub name: String,
    pub work_dir: String,
    pub agent: String,
    pub model: String,
    pub thinking: Option<String>,
    pub session_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSnapshot {
    pub key: String,
    pub active_project: String,
    pub projects: Vec<ProjectSettings>,
    pub version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStrategy {
    pub default_model: String,
    pub fallback_model: String,
}

#[derive(Debug, Clone)]
pub struct RunTaskOptions {
    pub project: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub mode: Option<RunMode>,
    pub thinking: Option<Option<String>>,
}

impl Default for RunTaskOptions {
    fn default() -> Self {
        Self {
            project: None,
            model: None,
            agent: None,
            mode: None,
            thinking: None,
        }
    }
}
