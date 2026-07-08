//! Read-only orchestration: migration listing, SQL, scripts, branch reads,
//! and operation cancellation. None of these mutate persisted config.

use super::*;
use crate::dotnet::DotnetEf;
use crate::git::GitService;
use crate::parser::MigrationParser;
use crate::state::{AppState, Migration};

// ─── Read operations ────────────────────────────────────────────────

/// EF migration list for the active project, annotated with applied/custom-SQL
/// flags. Refreshes `state.migrations` so later reads see the latest snapshot.
pub async fn list_migrations(state: &AppState) -> Result<Vec<Migration>, String> {
    let config = require_project(state)?;

    let migrations = tokio::task::spawn_blocking(move || {
        let ef_migrations = DotnetEf::list_migrations(
            &config.project_path,
            &config.db_context,
            &config.startup_project,
        )
        .map_err(|e| enrich_ef_error(&e))?;

        // Cache all migration files once instead of scanning per migration.
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
        Ok::<Vec<Migration>, String>(migrations)
    })
    .await
    .map_err(|e| e.to_string())??;

    *state.migrations.lock().unwrap() = migrations.clone();
    Ok(migrations)
}

/// Parsed Up/Down bodies + extracted custom SQL for one migration.
pub async fn get_migration_sql(state: &AppState, name: String) -> Result<MigrationSqlInfo, String> {
    let config = require_project(state)?;

    tokio::task::spawn_blocking(move || {
        let file_path = MigrationParser::get_migration_file(&config.project_path, &name)
            .ok_or_else(|| {
                format!(
                    "Migration file not found for '{}' in project '{}'",
                    name, config.project_path
                )
            })?;
        let parsed = MigrationParser::parse_file(&file_path)?;
        let custom_sql_up = parsed.sql_strings_up();
        let custom_sql_down = parsed.sql_strings_down();
        Ok(MigrationSqlInfo {
            name: parsed.file_name,
            up_body: parsed.up_body,
            down_body: parsed.down_body,
            custom_sql_up,
            custom_sql_down,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// SQL script between two migrations (empty bounds = full range).
pub async fn generate_script(state: &AppState, from: String, to: String) -> Result<String, String> {
    let config = require_project(state)?;

    let result = tokio::task::spawn_blocking(move || {
        DotnetEf::script_migration(
            &config.project_path,
            &from,
            &to,
            &config.db_context,
            &config.startup_project,
        )
    })
    .await
    .map_err(|e| e.to_string())??;

    if result.success {
        Ok(result.stdout)
    } else {
        Err(enrich_ef_error(&format!(
            "Failed to generate script: {}",
            result.error_output()
        )))
    }
}

/// Local + remote branches except the current one.
pub async fn list_git_branches(state: &AppState) -> Result<Vec<BranchInfo>, String> {
    let project_path = require_project(state)?.project_path;

    tokio::task::spawn_blocking(move || {
        let current = GitService::get_current_branch(&project_path).unwrap_or_default();
        let branches = GitService::list_branches(&project_path)?;
        Ok(branches
            .into_iter()
            .filter(|(name, _)| name != &current)
            .map(|(name, is_remote)| BranchInfo { name, is_remote })
            .collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Fetch and prune all configured remotes for the active project repository.
pub async fn fetch_remote(state: &AppState) -> Result<(), String> {
    let project_path = require_project(state)?.project_path;

    tokio::task::spawn_blocking(move || GitService::fetch(&project_path))
        .await
        .map_err(|e| e.to_string())?
}

/// Request cancellation of the in-flight operation and kill the `dotnet ef`
/// child if one is running. Multi-step orchestration bails at its next phase.
pub async fn cancel_running_operation(state: &AppState) -> Result<String, String> {
    state.request_cancel();
    let killed = tokio::task::spawn_blocking(DotnetEf::cancel_running_operation)
        .await
        .map_err(|e| e.to_string())??;
    Ok(match killed {
        Some(op) => format!("Cancel requested for '{}'", op),
        None => "Cancel requested.".to_string(),
    })
}

/// Current branch of the active project; also refreshes `state.current_branch`.
pub async fn get_current_branch(state: &AppState) -> Result<String, String> {
    let project_path = require_project(state)?.project_path;
    let branch = tokio::task::spawn_blocking(move || GitService::get_current_branch(&project_path))
        .await
        .map_err(|e| e.to_string())??;
    *state.current_branch.lock().unwrap() = branch.clone();
    Ok(branch)
}
