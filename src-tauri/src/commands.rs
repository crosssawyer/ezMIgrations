use crate::dotnet::DotnetEf;
use crate::git::GitService;
use crate::parser::MigrationParser;
use crate::state::{AppState, Migration, ProjectConfig};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::mpsc::channel;
use std::thread;
use tauri::{AppHandle, Emitter, Manager, State};

// ─── Project Commands ───────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct ProjectInfo {
    pub path: String,
    pub db_context: String,
    pub startup_project: String,
    pub branch: String,
}

#[tauri::command]
pub fn set_project(
    state: State<'_, AppState>,
    project_path: String,
    db_context: String,
    startup_project: String,
) -> Result<ProjectInfo, String> {
    // Validate path exists
    if !Path::new(&project_path).exists() {
        return Err(format!("Path does not exist: {}", project_path));
    }

    let branch = GitService::get_current_branch(&project_path).unwrap_or_default();

    let config = ProjectConfig {
        project_path: project_path.clone(),
        db_context,
        startup_project,
    };

    *state.config.lock().unwrap() = Some(config.clone());
    *state.current_branch.lock().unwrap() = branch.clone();

    Ok(ProjectInfo {
        path: config.project_path,
        db_context: config.db_context,
        startup_project: config.startup_project,
        branch,
    })
}

#[tauri::command]
pub fn get_project(state: State<'_, AppState>) -> Result<Option<ProjectInfo>, String> {
    let config = state.config.lock().unwrap();
    let branch = state.current_branch.lock().unwrap().clone();
    Ok(config.as_ref().map(|c| ProjectInfo {
        path: c.project_path.clone(),
        db_context: c.db_context.clone(),
        startup_project: c.startup_project.clone(),
        branch,
    }))
}

// ─── Migration Commands ─────────────────────────────────────────────

#[tauri::command]
pub fn list_migrations(state: State<'_, AppState>) -> Result<Vec<Migration>, String> {
    let config = state.config.lock().unwrap();
    let config = config.as_ref().ok_or("No project configured")?;

    let ef_migrations = DotnetEf::list_migrations(
        &config.project_path,
        &config.db_context,
        &config.startup_project,
    )?;

    let mut migrations: Vec<Migration> = Vec::new();

    for (name, applied) in &ef_migrations {
        let file_path =
            MigrationParser::get_migration_file(&config.project_path, name);

        let (has_custom_sql, custom_sql_up, custom_sql_down) = if let Some(ref fp) = file_path {
            match MigrationParser::parse_file(fp) {
                Ok(parsed) => (
                    parsed.has_custom_sql,
                    parsed.custom_sql_up,
                    parsed.custom_sql_down,
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

    *state.migrations.lock().unwrap() = migrations.clone();
    Ok(migrations)
}

#[tauri::command]
pub fn add_migration(state: State<'_, AppState>, name: String) -> Result<String, String> {
    let config = state.config.lock().unwrap();
    let config = config.as_ref().ok_or("No project configured")?;

    let result = DotnetEf::add_migration(
        &config.project_path,
        &name,
        &config.db_context,
        &config.startup_project,
    )?;

    if result.success {
        Ok(format!("Migration '{}' created successfully", name))
    } else {
        Err(format!("Failed to create migration: {}", result.stderr))
    }
}

#[tauri::command]
pub fn remove_migration(state: State<'_, AppState>, force: bool) -> Result<String, String> {
    let config = state.config.lock().unwrap();
    let config = config.as_ref().ok_or("No project configured")?;

    let result = DotnetEf::remove_migration(
        &config.project_path,
        &config.db_context,
        &config.startup_project,
        force,
    )?;

    if result.success {
        Ok("Last migration removed successfully".to_string())
    } else {
        Err(format!("Failed to remove migration: {}", result.stderr))
    }
}

#[tauri::command]
pub fn update_database(state: State<'_, AppState>, target: String) -> Result<String, String> {
    let config = state.config.lock().unwrap();
    let config = config.as_ref().ok_or("No project configured")?;

    let result = DotnetEf::update_database(
        &config.project_path,
        &target,
        &config.db_context,
        &config.startup_project,
    )?;

    if result.success {
        if target.is_empty() {
            Ok("Database updated to latest migration".to_string())
        } else {
            Ok(format!("Database updated to migration: {}", target))
        }
    } else {
        Err(format!("Failed to update database: {}", result.stderr))
    }
}

#[tauri::command]
pub fn get_migration_sql(
    state: State<'_, AppState>,
    migration_name: String,
) -> Result<MigrationSqlInfo, String> {
    let config = state.config.lock().unwrap();
    let config = config.as_ref().ok_or("No project configured")?;

    let file_path = MigrationParser::get_migration_file(&config.project_path, &migration_name)
        .ok_or("Migration file not found")?;

    let parsed = MigrationParser::parse_file(&file_path)?;

    Ok(MigrationSqlInfo {
        name: parsed.file_name,
        up_body: parsed.up_body,
        down_body: parsed.down_body,
        custom_sql_up: parsed.custom_sql_up,
        custom_sql_down: parsed.custom_sql_down,
    })
}

#[derive(Serialize)]
pub struct MigrationSqlInfo {
    pub name: String,
    pub up_body: String,
    pub down_body: String,
    pub custom_sql_up: Vec<String>,
    pub custom_sql_down: Vec<String>,
}

// ─── Squash Command ─────────────────────────────────────────────────

#[tauri::command]
pub fn squash_migrations(
    state: State<'_, AppState>,
    from_migration: String,
    to_migration: String,
    new_name: String,
) -> Result<String, String> {
    let config = state.config.lock().unwrap();
    let config = config.as_ref().ok_or("No project configured")?;

    // 1. Collect all custom SQL from migrations in the range
    let migrations = state.migrations.lock().unwrap().clone();

    let mut in_range = false;
    let mut all_custom_sql: Vec<String> = Vec::new();
    let mut migrations_to_remove: Vec<String> = Vec::new();

    for m in &migrations {
        if m.name == from_migration {
            in_range = true;
        }
        if in_range {
            all_custom_sql.extend(m.custom_sql_up.clone());
            migrations_to_remove.push(m.name.clone());
        }
        if m.name == to_migration {
            break;
        }
    }

    if migrations_to_remove.is_empty() {
        return Err("No migrations found in the specified range".to_string());
    }

    // 2. Update database back to the migration before the range
    let before_migration = migrations
        .iter()
        .take_while(|m| m.name != from_migration)
        .last()
        .map(|m| m.name.clone())
        .unwrap_or_else(|| "0".to_string());

    let update_result = DotnetEf::update_database(
        &config.project_path,
        &before_migration,
        &config.db_context,
        &config.startup_project,
    )?;

    if !update_result.success {
        return Err(format!(
            "Failed to revert database for squash: {}",
            update_result.stderr
        ));
    }

    // 3. Remove migrations in reverse order
    for _ in migrations_to_remove.iter().rev() {
        let result = DotnetEf::remove_migration(
            &config.project_path,
            &config.db_context,
            &config.startup_project,
            true,
        )?;

        if !result.success {
            return Err(format!(
                "Failed to remove migration during squash: {}",
                result.stderr
            ));
        }
    }

    // 4. Create new squashed migration
    let add_result = DotnetEf::add_migration(
        &config.project_path,
        &new_name,
        &config.db_context,
        &config.startup_project,
    )?;

    if !add_result.success {
        return Err(format!(
            "Failed to create squashed migration: {}",
            add_result.stderr
        ));
    }

    // 5. Inject captured custom SQL into the new migration
    if !all_custom_sql.is_empty() {
        if let Some(new_file) =
            MigrationParser::get_migration_file(&config.project_path, &new_name)
        {
            MigrationParser::inject_custom_sql(&new_file, &all_custom_sql)?;
        }
    }

    // 6. Apply the new squashed migration
    let final_update = DotnetEf::update_database(
        &config.project_path,
        "",
        &config.db_context,
        &config.startup_project,
    )?;

    if !final_update.success {
        return Err(format!(
            "Squash created but failed to apply: {}",
            final_update.stderr
        ));
    }

    Ok(format!(
        "Squashed {} migrations into '{}'. Custom SQL preserved: {} statements.",
        migrations_to_remove.len(),
        new_name,
        all_custom_sql.len()
    ))
}

// ─── Script Command ─────────────────────────────────────────────────

#[tauri::command]
pub fn generate_script(
    state: State<'_, AppState>,
    from: String,
    to: String,
) -> Result<String, String> {
    let config = state.config.lock().unwrap();
    let config = config.as_ref().ok_or("No project configured")?;

    let result = DotnetEf::script_migration(
        &config.project_path,
        &from,
        &to,
        &config.db_context,
        &config.startup_project,
    )?;

    if result.success {
        Ok(result.stdout)
    } else {
        Err(format!("Failed to generate script: {}", result.stderr))
    }
}

// ─── Git / Branch Commands ──────────────────────────────────────────

#[tauri::command]
pub fn get_current_branch(state: State<'_, AppState>) -> Result<String, String> {
    let config = state.config.lock().unwrap();
    let config = config.as_ref().ok_or("No project configured")?;

    let branch = GitService::get_current_branch(&config.project_path)?;
    *state.current_branch.lock().unwrap() = branch.clone();
    Ok(branch)
}

#[tauri::command]
pub fn start_branch_watcher(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let config = state.config.lock().unwrap();
    let config = config.as_ref().ok_or("No project configured")?;

    let head_path = GitService::get_head_path(&config.project_path)
        .ok_or("Could not find .git/HEAD")?;

    // Check if already watching
    {
        let watching = state.watching.lock().unwrap();
        if *watching {
            return Ok("Already watching for branch changes".to_string());
        }
    }

    *state.watching.lock().unwrap() = true;

    let project_path = config.project_path.clone();
    let head_path_clone = head_path.clone();

    thread::spawn(move || {
        let (tx, rx) = channel();
        let mut watcher = match RecommendedWatcher::new(tx, Config::default()) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("Failed to create watcher: {}", e);
                return;
            }
        };

        let parent = Path::new(&head_path_clone).parent().unwrap();
        if let Err(e) = watcher.watch(parent, RecursiveMode::NonRecursive) {
            eprintln!("Failed to watch .git directory: {}", e);
            return;
        }

        let mut last_branch = GitService::get_current_branch(&project_path).unwrap_or_default();

        loop {
            match rx.recv() {
                Ok(_event) => {
                    // Small delay to let git finish writing
                    thread::sleep(std::time::Duration::from_millis(500));

                    if let Ok(new_branch) = GitService::get_current_branch(&project_path) {
                        if new_branch != last_branch {
                            let old = last_branch.clone();
                            last_branch = new_branch.clone();

                            let _ = app.emit(
                                "branch-changed",
                                BranchChangeEvent {
                                    old_branch: old,
                                    new_branch,
                                },
                            );
                        }
                    }
                }
                Err(_) => break,
            }
        }
    });

    Ok(format!("Watching for branch changes: {}", head_path))
}

#[derive(Clone, Serialize)]
struct BranchChangeEvent {
    old_branch: String,
    new_branch: String,
}
