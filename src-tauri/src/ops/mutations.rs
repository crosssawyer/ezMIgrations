//! EF-mutating orchestration (each serialized via `op_mutex`) plus the
//! managed branch switch. Progress flows to the caller's `PhaseSink`.

use std::path::Path;

use super::*;
use crate::dotnet::DotnetEf;
use crate::git::GitService;
use crate::parser::{KeepStrategy, MigrationParser, SqlStatement};
use crate::state::AppState;

// ─── EF-mutating operations (serialized via op_mutex) ────────────────

pub async fn add_migration(
    state: &AppState,
    name: String,
    sink: impl PhaseSink,
) -> Result<String, String> {
    let config = require_project(state)?;
    let _guard = state.op_mutex.lock().await;
    state.reset_op_cancel();

    sink.emit("creating", format!("Creating migration {name}…"));

    let result = tokio::task::spawn_blocking(move || {
        DotnetEf::add_migration(
            &config.project_path,
            &name,
            &config.db_context,
            &config.startup_project,
        )
    })
    .await
    .map_err(|e| e.to_string())??;

    state.reset_op_cancel();
    if result.success {
        Ok("Migration created successfully".to_string())
    } else {
        Err(enrich_ef_error(&format!(
            "Failed to create migration: {}",
            result.error_output()
        )))
    }
}

pub async fn remove_migration(
    state: &AppState,
    force: bool,
    sink: impl PhaseSink,
) -> Result<String, String> {
    let config = require_project(state)?;
    let _guard = state.op_mutex.lock().await;
    state.reset_op_cancel();

    sink.emit(
        "removing",
        if force {
            "Removing migration (force)…".to_string()
        } else {
            "Removing last migration…".to_string()
        },
    );

    let result = tokio::task::spawn_blocking(move || {
        DotnetEf::remove_migration(
            &config.project_path,
            &config.db_context,
            &config.startup_project,
            force,
        )
    })
    .await
    .map_err(|e| e.to_string())??;

    state.reset_op_cancel();
    if result.success {
        Ok("Last migration removed successfully".to_string())
    } else {
        Err(enrich_ef_error(&format!(
            "Failed to remove migration: {}",
            result.error_output()
        )))
    }
}

pub async fn update_database(
    state: &AppState,
    target: String,
    sink: impl PhaseSink,
) -> Result<String, String> {
    let config = require_project(state)?;
    let _guard = state.op_mutex.lock().await;
    state.reset_op_cancel();

    let label = if target.is_empty() {
        "latest".to_string()
    } else if target == "0" {
        "base".to_string()
    } else {
        target.clone()
    };
    sink.emit("applying", format!("Updating database to {label}…"));

    let target_for_run = target.clone();
    let result = tokio::task::spawn_blocking(move || {
        DotnetEf::update_database(
            &config.project_path,
            &target_for_run,
            &config.db_context,
            &config.startup_project,
        )
    })
    .await
    .map_err(|e| e.to_string())??;

    state.reset_op_cancel();
    if result.success {
        if target.is_empty() {
            Ok("Database updated to latest migration".to_string())
        } else {
            Ok(format!("Database updated to migration: {}", target))
        }
    } else {
        Err(enrich_ef_error(&format!(
            "Failed to update database: {}",
            result.error_output()
        )))
    }
}

/// Squash `[from_migration, to_migration]` into one new migration, preserving
/// every custom `migrationBuilder.Sql(...)` call. Reverts → removes → scaffolds
/// → re-injects SQL → reapplies. Refreshes the migration list internally so it
/// doesn't depend on a prior `list_migrations` call.
pub async fn squash_migrations(
    state: &AppState,
    from_migration: String,
    to_migration: String,
    new_name: String,
    sink: impl PhaseSink,
) -> Result<String, String> {
    let config = require_project(state)?;
    // Refresh so we always have current file_path metadata for the range.
    let migrations = list_migrations(state).await?;

    let _guard = state.op_mutex.lock().await;
    state.reset_op_cancel();
    let cancel = state.op_cancel.clone();

    // Numbered step prefix. Bump TOTAL_STEPS if you add/remove a numbered step
    // (currently: 1 revert, 2 remove, 3 create, 4 apply — scan and inject are
    // shown without numbers because they're effectively instantaneous).
    const TOTAL_STEPS: usize = 4;

    let result = tokio::task::spawn_blocking(move || {
        let step_prefix = |n: usize| format!("Step {n}/{TOTAL_STEPS}");

        sink.emit(
            "scanning",
            format!("Scanning migrations from {from_migration} to {to_migration}…"),
        );

        // 1. Collect custom SQL across the range (with ordering metadata).
        let mut in_range = false;
        let mut all_custom_sql_up: Vec<SqlStatement> = Vec::new();
        let mut all_custom_sql_down: Vec<SqlStatement> = Vec::new();
        let mut migrations_to_remove: Vec<String> = Vec::new();

        for m in &migrations {
            if m.name == from_migration {
                in_range = true;
            }
            if in_range {
                if let Some(ref fp) = m.file_path {
                    if let Ok(parsed) = MigrationParser::parse_file(Path::new(fp)) {
                        all_custom_sql_up.extend(parsed.custom_sql_up);
                        all_custom_sql_down.extend(parsed.custom_sql_down);
                    }
                }
                migrations_to_remove.push(m.name.clone());
            }
            if m.name == to_migration {
                break;
            }
        }

        let all_custom_sql_up =
            MigrationParser::deduplicate_sql(all_custom_sql_up, KeepStrategy::Last);
        let all_custom_sql_down =
            MigrationParser::deduplicate_sql(all_custom_sql_down, KeepStrategy::First);

        if migrations_to_remove.is_empty() {
            return Err("No migrations found in the specified range".to_string());
        }
        let total = migrations_to_remove.len();

        // 2. Revert the database to the migration before the range.
        let before_migration = migrations
            .iter()
            .take_while(|m| m.name != from_migration)
            .last()
            .map(|m| m.name.clone())
            .unwrap_or_else(|| "0".to_string());

        check_cancel(&cancel)?;
        let revert_label = if before_migration == "0" {
            "base".to_string()
        } else {
            before_migration.clone()
        };
        sink.emit(
            "reverting",
            format!("{} — Reverting database to {revert_label}…", step_prefix(1)),
        );
        let update_result = DotnetEf::update_database(
            &config.project_path,
            &before_migration,
            &config.db_context,
            &config.startup_project,
        )?;
        if !update_result.success {
            return Err(format!(
                "Failed to revert database for squash: {}",
                update_result.error_output()
            ));
        }

        // 3. Remove migrations in reverse order.
        for (idx, name) in migrations_to_remove.iter().rev().enumerate() {
            check_cancel(&cancel)?;
            sink.emit(
                "removing",
                format!(
                    "{} — Removing migration {}/{}: {}…",
                    step_prefix(2),
                    idx + 1,
                    total,
                    name
                ),
            );
            let result = DotnetEf::remove_migration(
                &config.project_path,
                &config.db_context,
                &config.startup_project,
                true,
            )?;
            if !result.success {
                return Err(format!(
                    "Failed to remove migration during squash: {}",
                    result.error_output()
                ));
            }
        }

        // 4. Create the new squashed migration.
        check_cancel(&cancel)?;
        sink.emit(
            "creating",
            format!("{} — Creating squashed migration {new_name}…", step_prefix(3)),
        );
        let add_result = DotnetEf::add_migration(
            &config.project_path,
            &new_name,
            &config.db_context,
            &config.startup_project,
        )?;
        if !add_result.success {
            return Err(format!(
                "Failed to create squashed migration: {}",
                add_result.error_output()
            ));
        }

        // 5. Re-inject captured custom SQL (Up and Down).
        if !all_custom_sql_up.is_empty() || !all_custom_sql_down.is_empty() {
            sink.emit(
                "injecting",
                format!(
                    "Preserving custom SQL ({} Up, {} Down)…",
                    all_custom_sql_up.len(),
                    all_custom_sql_down.len()
                ),
            );
        }
        if let Some(new_file) = MigrationParser::get_migration_file(&config.project_path, &new_name)
        {
            if !all_custom_sql_up.is_empty() {
                MigrationParser::inject_custom_sql(&new_file, "Up", &all_custom_sql_up)?;
            }
            if !all_custom_sql_down.is_empty() {
                MigrationParser::inject_custom_sql(&new_file, "Down", &all_custom_sql_down)?;
            }
        }

        // 6. Apply the new squashed migration.
        check_cancel(&cancel)?;
        sink.emit(
            "applying",
            format!("{} — Applying squashed migration {new_name}…", step_prefix(4)),
        );
        let final_update = DotnetEf::update_database(
            &config.project_path,
            "",
            &config.db_context,
            &config.startup_project,
        )?;
        if !final_update.success {
            return Err(format!(
                "Squash created but failed to apply: {}",
                final_update.error_output()
            ));
        }

        Ok(format!(
            "Squashed {} migrations into '{}'. Custom SQL preserved: {} Up, {} Down.",
            total,
            new_name,
            all_custom_sql_up.len(),
            all_custom_sql_down.len()
        ))
    })
    .await
    .map_err(|e| e.to_string())?;

    state.reset_op_cancel();
    result.map_err(|e| enrich_ef_error(&e))
}

/// Managed branch switch: roll the DB back to the latest migration common to
/// both branches, `git checkout` the target, then apply the target's
/// migrations. Refuses to run on a dirty working tree. Serialized via op_mutex.
///
/// On success, updates `state.current_branch` and clears the cached migration
/// list. On failure, re-reads the branch so cached state reflects reality
/// (the checkout may or may not have happened).
pub async fn switch_branch_with_migrations(
    state: &AppState,
    target_branch: String,
    sink: impl PhaseSink,
) -> Result<BranchSwitchResult, String> {
    let config = require_project(state)?;
    let _guard = state.op_mutex.lock().await;
    state.reset_op_cancel();

    let project_path_for_error = config.project_path.clone();
    let cancel = state.op_cancel.clone();

    let result = tokio::task::spawn_blocking(move || {
        let target_branch = target_branch.trim().to_string();
        if target_branch.is_empty() {
            return Err("Choose a branch to switch to".to_string());
        }

        sink.emit(
            "preparing",
            format!("Preparing to switch to {target_branch}…"),
        );

        let old_branch = GitService::get_current_branch(&config.project_path)?;
        if old_branch == target_branch {
            return Ok(BranchSwitchResult {
                old_branch: old_branch.clone(),
                new_branch: old_branch,
                common_migration: None,
                rollback_target: None,
                rollback_performed: false,
                target_migration_count: 0,
            });
        }

        if !GitService::ref_exists(&config.project_path, &target_branch)? {
            return Err(format!("Git branch not found: {}", target_branch));
        }
        if !GitService::is_working_tree_clean(&config.project_path)? {
            return Err(
                "Working tree has uncommitted changes. Commit, stash, or discard them before using automatic branch switch."
                    .to_string(),
            );
        }
        check_cancel(&cancel)?;

        let repo_root = GitService::get_repo_root(&config.project_path)?;
        let migrations_dir = MigrationParser::find_migrations_dir(&config.project_path)?;
        let migrations_pathspec = path_relative_to_repo(&repo_root, &migrations_dir)?;

        sink.emit(
            "reading-target",
            format!("Reading migrations on {target_branch}…"),
        );
        let target_files =
            GitService::list_files_at_ref(&repo_root, &target_branch, &migrations_pathspec)?;
        let target_migrations: Vec<String> = target_files
            .iter()
            .filter_map(|path| migration_name_from_path(Path::new(path)))
            .collect();

        let current_files = MigrationParser::find_migration_files(&config.project_path)
            .map(|files| {
                files
                    .iter()
                    .filter_map(|p| migration_name_from_path(p))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        check_cancel(&cancel)?;
        sink.emit(
            "listing-applied",
            "Listing applied migrations in the database…".to_string(),
        );
        let ef_migrations = DotnetEf::list_migrations(
            &config.project_path,
            &config.db_context,
            &config.startup_project,
        )
        .map_err(|e| enrich_ef_error(&e))?;

        let ef_migration_names: Vec<String> =
            ef_migrations.iter().map(|(name, _)| name.clone()).collect();
        let current_migrations = if ef_migration_names.is_empty() {
            current_files
        } else {
            ef_migration_names
        };

        let common_migration = latest_common_migration(&current_migrations, &target_migrations);
        let common_index = common_migration
            .as_ref()
            .and_then(|name| current_migrations.iter().position(|m| m == name));
        let latest_applied_index = ef_migrations.iter().rposition(|(_, applied)| *applied);

        if common_migration.is_none() && latest_applied_index.is_some() {
            let preview = |list: &[String]| -> String {
                if list.is_empty() {
                    "(empty)".to_string()
                } else {
                    let shown: Vec<String> = list.iter().take(5).cloned().collect();
                    let suffix = if list.len() > 5 {
                        format!(", … (+{} more)", list.len() - 5)
                    } else {
                        String::new()
                    };
                    format!("[{}{}]", shown.join(", "), suffix)
                }
            };
            return Err(format!(
                "No common migration found between '{}' and '{}'. Refusing to revert all applied migrations automatically — this is almost always caused by mismatched migration filenames or a different migrations folder path on the target branch.\n\nCurrent branch migrations ({}): {}\nTarget branch migrations ({}): {}\nMigrations pathspec: {}",
                old_branch,
                target_branch,
                current_migrations.len(),
                preview(&current_migrations),
                target_migrations.len(),
                preview(&target_migrations),
                migrations_pathspec,
            ));
        }

        let rollback_target = match (latest_applied_index, common_index) {
            (Some(applied), Some(common)) if applied > common => common_migration.clone(),
            _ => None,
        };

        let mut rollback_performed = false;
        if let Some(ref target) = rollback_target {
            check_cancel(&cancel)?;
            let label = if target == "0" { "base" } else { target.as_str() };
            sink.emit("rolling-back", format!("Rolling database back to {label}…"));
            let rollback = DotnetEf::update_database(
                &config.project_path,
                target,
                &config.db_context,
                &config.startup_project,
            )?;
            if !rollback.success {
                return Err(enrich_ef_error(&format!(
                    "Failed to roll back before switching branches: {}",
                    rollback.error_output()
                )));
            }
            rollback_performed = true;
        }

        check_cancel(&cancel)?;
        sink.emit(
            "switching-git",
            format!("Switching git to {target_branch}…"),
        );
        GitService::switch_branch(&config.project_path, &target_branch)?;
        let new_branch = GitService::get_current_branch(&config.project_path)
            .unwrap_or_else(|_| target_branch.clone());

        check_cancel(&cancel)?;
        let pending = match (common_index, target_migrations.len()) {
            (Some(common), total) if total > common + 1 => total - common - 1,
            (None, total) => total,
            _ => 0,
        };
        let apply_msg = if pending > 0 {
            format!(
                "Applying {} pending migration{} on {}…",
                pending,
                if pending == 1 { "" } else { "s" },
                new_branch
            )
        } else {
            format!("Updating database on {}…", new_branch)
        };
        sink.emit("applying", apply_msg);
        let update = DotnetEf::update_database(
            &config.project_path,
            "",
            &config.db_context,
            &config.startup_project,
        )?;
        if !update.success {
            return Err(enrich_ef_error(&format!(
                "Switched to '{}', but failed to update the database: {}",
                new_branch,
                update.error_output()
            )));
        }

        Ok(BranchSwitchResult {
            old_branch,
            new_branch,
            common_migration,
            rollback_target,
            rollback_performed,
            target_migration_count: target_migrations.len(),
        })
    })
    .await
    .map_err(|e| e.to_string())?;

    state.reset_op_cancel();
    match result {
        Ok(switch) => {
            *state.current_branch.lock().unwrap() = switch.new_branch.clone();
            state.migrations.lock().unwrap().clear();
            Ok(switch)
        }
        Err(err) => {
            if let Ok(branch) = GitService::get_current_branch(&project_path_for_error) {
                *state.current_branch.lock().unwrap() = branch;
            }
            Err(err)
        }
    }
}

