//! Read-only `ezmigrations://` resource URIs surfaced over MCP.
//!
//! Each helper returns a JSON or plain-text payload built from `AppState` plus
//! a fresh git read where relevant. None of these mutate state, so they're
//! safe to fire concurrently with mutating tools (the underlying mutexes
//! protect the data they touch).

use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;

use crate::dotnet::DotnetEf;
use crate::git::GitService;
use crate::parser::MigrationParser;
use crate::state::{AppState, Migration, ProjectConfig};

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
/// from EF, same path as `list_migrations`).
pub async fn read_migrations(state: Arc<AppState>) -> Result<String, String> {
    let config = require_project(&state)?;
    let migrations = list_migrations_inner(config).await?;
    *state.migrations.lock().unwrap() = migrations.clone();
    serde_json::to_string_pretty(&migrations).map_err(|e| e.to_string())
}

/// `ezmigrations://migrations/{name}/sql` — JSON of parsed SQL for one
/// migration (custom_sql_up/down + Up/Down bodies).
pub async fn read_migration_sql(state: Arc<AppState>, name: &str) -> Result<String, String> {
    let config = require_project(&state)?;
    let migration_name = name.to_string();

    let info = tokio::task::spawn_blocking(move || {
        let file_path = MigrationParser::get_migration_file(&config.project_path, &migration_name)
            .ok_or_else(|| {
                format!(
                    "Migration file not found for '{}' in project '{}'",
                    migration_name, config.project_path
                )
            })?;
        let parsed = MigrationParser::parse_file(&file_path)?;
        Ok::<_, String>(serde_json::json!({
            "name": parsed.file_name,
            "up_body": parsed.up_body,
            "down_body": parsed.down_body,
            "custom_sql_up": parsed.sql_strings_up(),
            "custom_sql_down": parsed.sql_strings_down(),
        }))
    })
    .await
    .map_err(|e| e.to_string())??;

    serde_json::to_string_pretty(&info).map_err(|e| e.to_string())
}

/// `ezmigrations://branches` — JSON of all git branches (local + remote),
/// excluding the current one (mirrors `list_git_branches`).
pub async fn read_branches(state: Arc<AppState>) -> Result<String, String> {
    let project_path = require_project(&state)?.project_path;

    let branches = tokio::task::spawn_blocking(move || {
        let current = GitService::get_current_branch(&project_path).unwrap_or_default();
        let branches = GitService::list_branches(&project_path)?;
        Ok::<_, String>(
            branches
                .into_iter()
                .filter(|(name, _)| name != &current)
                .map(|(name, is_remote)| {
                    serde_json::json!({ "name": name, "isRemote": is_remote })
                })
                .collect::<Vec<_>>(),
        )
    })
    .await
    .map_err(|e| e.to_string())??;

    serde_json::to_string_pretty(&branches).map_err(|e| e.to_string())
}

/// `ezmigrations://branches/current` — plain string of the current branch.
pub async fn read_current_branch(state: Arc<AppState>) -> Result<String, String> {
    let project_path = require_project(&state)?.project_path;
    let branch = tokio::task::spawn_blocking(move || GitService::get_current_branch(&project_path))
        .await
        .map_err(|e| e.to_string())??;
    *state.current_branch.lock().unwrap() = branch.clone();
    Ok(branch)
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
    let operation_in_progress = state
        .op_cancel
        .load(std::sync::atomic::Ordering::SeqCst);
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

pub fn require_project(state: &AppState) -> Result<ProjectConfig, String> {
    state
        .config
        .lock()
        .unwrap()
        .as_ref()
        .cloned()
        .ok_or_else(|| "No project configured".to_string())
}

/// Shared migration list builder used by both the tool and the resource so
/// they stay in lock-step. Mirrors `commands::list_migrations` minus the
/// Tauri `State` extraction.
pub async fn list_migrations_inner(config: ProjectConfig) -> Result<Vec<Migration>, String> {
    tokio::task::spawn_blocking(move || {
        let ef_migrations = DotnetEf::list_migrations(
            &config.project_path,
            &config.db_context,
            &config.startup_project,
        )?;

        let all_files =
            MigrationParser::find_migration_files(&config.project_path).unwrap_or_default();

        let mut migrations: Vec<Migration> = Vec::new();
        for (name, applied) in &ef_migrations {
            let file_path = all_files.iter().find(|f| {
                f.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.contains(name) || name.contains(s))
                    .unwrap_or(false)
            });

            let (has_custom_sql, custom_sql_up, custom_sql_down) = if let Some(fp) = file_path {
                match MigrationParser::parse_file(fp) {
                    Ok(parsed) => (
                        parsed.has_custom_sql,
                        parsed.sql_strings_up(),
                        parsed.sql_strings_down(),
                    ),
                    Err(_) => (false, Vec::new(), Vec::new()),
                }
            } else {
                (false, Vec::new(), Vec::new())
            };

            migrations.push(Migration {
                id: name.clone(),
                name: name.clone(),
                applied: *applied,
                has_custom_sql,
                custom_sql_up,
                custom_sql_down,
                file_path: file_path.map(|p| p.to_string_lossy().to_string()),
            });
        }
        Ok(migrations)
    })
    .await
    .map_err(|e| e.to_string())?
}
