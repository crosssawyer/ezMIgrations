//! Read-only `ezmigrations://` resource URIs surfaced over MCP.
//!
//! Each helper returns a JSON or plain-text payload built from `AppState` plus
//! a fresh git read where relevant. None of these mutate state, so they're
//! safe to fire concurrently with mutating tools (the underlying mutexes
//! protect the data they touch).

use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;

use crate::ops;
use crate::state::AppState;

/// All URI templates we advertise via `list_resources` / `list_resource_templates`.
pub mod uri {
    pub const PROJECT_CURRENT: &str = "ezmigrations://project/current";
    pub const MIGRATIONS: &str = "ezmigrations://migrations";
    pub const BRANCHES: &str = "ezmigrations://branches";
    pub const BRANCHES_CURRENT: &str = "ezmigrations://branches/current";
    pub const PROJECTS: &str = "ezmigrations://projects";
    pub const PREFERENCES: &str = "ezmigrations://preferences";
    pub const APP_STATUS: &str = "ezmigrations://app/status";

    /// RFC 6570 template; the `{name}` segment is the EF migration name.
    pub const MIGRATION_SQL_TEMPLATE: &str = "ezmigrations://migrations/{name}/sql";

    /// Prefix used to detect a templated migration-SQL read at dispatch time.
    pub const MIGRATION_SQL_PREFIX: &str = "ezmigrations://migrations/";
    pub const MIGRATION_SQL_SUFFIX: &str = "/sql";
}

#[derive(Serialize)]
pub struct AppStatusPayload {
    pub project_loaded: bool,
    pub current_branch: Option<String>,
    pub operation_in_progress: bool,
    pub watchers_active: WatchersStatus,
}

#[derive(Serialize)]
pub struct WatchersStatus {
    pub branch: bool,
    pub migrations: bool,
}

/// `ezmigrations://project/current` — JSON of the active project or `null`.
pub fn read_project_current(state: &AppState) -> Result<String, String> {
    let config = state.config.lock().unwrap().clone();
    let app_config = state.app_config.lock().unwrap();
    let branch = state.current_branch.lock().unwrap().clone();

    let value = config.map(|c| {
        let active_id = app_config.active_project_id.clone();
        let stable_migration = active_id
            .as_ref()
            .and_then(|id| app_config.projects.iter().find(|p| &p.id == id))
            .and_then(|p| p.stable_migration.clone());
        serde_json::json!({
            "id": active_id,
            "path": c.project_path,
            "db_context": c.db_context,
            "startup_project": c.startup_project,
            "branch": branch,
            "stable_migration": stable_migration,
        })
    });

    serde_json::to_string_pretty(&value.unwrap_or(Value::Null)).map_err(|e| e.to_string())
}

/// `ezmigrations://projects` — JSON of all saved projects.
pub fn read_projects(state: &AppState) -> Result<String, String> {
    let projects = state.app_config.lock().unwrap().projects.clone();
    serde_json::to_string_pretty(&projects).map_err(|e| e.to_string())
}

/// `ezmigrations://preferences` — JSON of user preferences.
pub fn read_preferences(state: &AppState) -> Result<String, String> {
    let prefs = state.app_config.lock().unwrap().preferences.clone();
    serde_json::to_string_pretty(&prefs).map_err(|e| e.to_string())
}

/// `ezmigrations://migrations` — JSON of the current migration list (refreshed
/// from EF, same code path as the `list_migrations` tool).
pub async fn read_migrations(state: Arc<AppState>) -> Result<String, String> {
    let migrations = ops::list_migrations(&state).await?;
    serde_json::to_string_pretty(&migrations).map_err(|e| e.to_string())
}

/// `ezmigrations://migrations/{name}/sql` — JSON of parsed SQL for one
/// migration (custom_sql_up/down + Up/Down bodies).
pub async fn read_migration_sql(state: Arc<AppState>, name: &str) -> Result<String, String> {
    let info = ops::get_migration_sql(&state, name.to_string()).await?;
    serde_json::to_string_pretty(&info).map_err(|e| e.to_string())
}

/// `ezmigrations://branches` — JSON of all git branches (local + remote),
/// excluding the current one (mirrors the `list_git_branches` tool).
pub async fn read_branches(state: Arc<AppState>) -> Result<String, String> {
    let branches = ops::list_git_branches(&state).await?;
    serde_json::to_string_pretty(&branches).map_err(|e| e.to_string())
}

/// `ezmigrations://branches/current` — plain string of the current branch.
pub async fn read_current_branch(state: Arc<AppState>) -> Result<String, String> {
    ops::get_current_branch(&state).await
}

/// `ezmigrations://app/status` — snapshot of high-level app state.
pub fn read_app_status(state: &AppState) -> Result<String, String> {
    let project_loaded = state.config.lock().unwrap().is_some();
    let current_branch = {
        let b = state.current_branch.lock().unwrap().clone();
        if b.is_empty() {
            None
        } else {
            Some(b)
        }
    };
    let operation_in_progress = state.op_cancel.load(std::sync::atomic::Ordering::SeqCst);
    let watchers_active = WatchersStatus {
        branch: *state.watching.lock().unwrap(),
        migrations: *state.watching_migrations.lock().unwrap(),
    };

    let payload = AppStatusPayload {
        project_loaded,
        current_branch,
        operation_in_progress,
        watchers_active,
    };
    serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())
}

// ─── Shared helpers ─────────────────────────────────────────────────

/// Extract `{name}` from a `ezmigrations://migrations/{name}/sql` URI.
/// Returns `None` if the URI doesn't match the template shape.
pub fn parse_migration_sql_uri(uri: &str) -> Option<&str> {
    let rest = uri.strip_prefix(uri::MIGRATION_SQL_PREFIX)?;
    let name = rest.strip_suffix(uri::MIGRATION_SQL_SUFFIX)?;
    if name.is_empty() || name.contains('/') {
        return None;
    }
    Some(name)
}
