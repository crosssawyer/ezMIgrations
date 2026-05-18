use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project_path: String,
    pub db_context: String,
    pub startup_project: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct SavedProject {
    pub id: String,
    pub name: String,
    pub project_path: String,
    pub db_context: String,
    pub startup_project: String,
    pub stable_migration: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preferences {
    #[serde(default = "default_true")]
    pub notify_on_branch_change: bool,
}

fn default_true() -> bool {
    true
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            notify_on_branch_change: true,
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub projects: Vec<SavedProject>,
    pub active_project_id: Option<String>,
    #[serde(default)]
    pub preferences: Preferences,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Migration {
    pub id: String,
    pub name: String,
    pub applied: bool,
    pub has_custom_sql: bool,
    pub custom_sql_up: Vec<String>,
    pub custom_sql_down: Vec<String>,
    pub file_path: Option<String>,
}

#[derive(Default)]
pub struct AppState {
    pub config: Mutex<Option<ProjectConfig>>,
    pub app_config: Mutex<AppConfig>,
    pub migrations: Mutex<Vec<Migration>>,
    pub current_branch: Mutex<String>,
    pub watching: Mutex<bool>,
    pub watching_migrations: Mutex<bool>,
    pub watcher_cancel: Mutex<Arc<AtomicBool>>,
    /// Set by `cancel_running_operation` so multi-step operations (e.g. branch
    /// switch) can bail between phases when there's no EF child process to kill.
    pub op_cancel: Arc<AtomicBool>,
}
