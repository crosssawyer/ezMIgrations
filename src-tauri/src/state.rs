use serde::{Deserialize, Serialize};
use std::sync::Mutex;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project_path: String,
    pub db_context: String,
    pub startup_project: String,
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

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct BranchInfo {
    pub name: String,
    pub last_migration: Option<String>,
}

#[derive(Default)]
pub struct AppState {
    pub config: Mutex<Option<ProjectConfig>>,
    pub migrations: Mutex<Vec<Migration>>,
    pub current_branch: Mutex<String>,
    pub watching: Mutex<bool>,
}
