use crate::dotnet::DotnetEf;
use crate::git::GitService;
use crate::parser::MigrationParser;
use crate::state::{AppConfig, AppState, Migration, ProjectConfig, SavedProject};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::mpsc::channel;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};

// ─── Project Commands ───────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct ProjectInfo {
    pub id: Option<String>,
    pub path: String,
    pub db_context: String,
    pub startup_project: String,
    pub branch: String,
    pub stable_migration: Option<String>,
}

fn config_file_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("project_config.json"))
}

fn app_config_file_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("app_config.json"))
}

fn generate_id() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}

fn save_app_config(app: &AppHandle, app_config: &AppConfig) -> Result<(), String> {
    let config_path = app_config_file_path(app).ok_or("Could not resolve app data dir")?;
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(app_config).map_err(|e| e.to_string())?;
    std::fs::write(&config_path, json).map_err(|e| e.to_string())?;
    Ok(())
}

fn derive_project_name(project_path: &str) -> String {
    Path::new(project_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "My Project".to_string())
}

fn migrate_legacy_config(app: &AppHandle, state: &AppState) -> Option<AppConfig> {
    let app_config_path = app_config_file_path(app)?;
    if app_config_path.exists() {
        return None; // already migrated
    }

    let legacy_path = config_file_path(app)?;
    if !legacy_path.exists() {
        return None;
    }

    let json = std::fs::read_to_string(&legacy_path).ok()?;
    let legacy: ProjectConfig = serde_json::from_str(&json).ok()?;

    if !Path::new(&legacy.project_path).exists() {
        return None;
    }

    let id = generate_id();
    let saved = SavedProject {
        id: id.clone(),
        name: derive_project_name(&legacy.project_path),
        project_path: legacy.project_path.clone(),
        db_context: legacy.db_context.clone(),
        startup_project: legacy.startup_project.clone(),
        stable_migration: None,
    };

    let app_config = AppConfig {
        projects: vec![saved],
        active_project_id: Some(id),
    };

    // Persist + load into state
    let _ = save_app_config(app, &app_config);
    *state.config.lock().unwrap() = Some(legacy);
    *state.app_config.lock().unwrap() = app_config.clone();

    Some(app_config)
}

#[tauri::command]
pub async fn set_project(
    app: AppHandle,
    state: State<'_, AppState>,
    project_path: String,
    db_context: String,
    startup_project: String,
) -> Result<ProjectInfo, String> {
    if !Path::new(&project_path).exists() {
        return Err(format!("Path does not exist: {}", project_path));
    }

    let pp = project_path.clone();
    let branch = tokio::task::spawn_blocking(move || {
        GitService::get_current_branch(&pp).unwrap_or_default()
    })
    .await
    .map_err(|e| e.to_string())?;

    let config = ProjectConfig {
        project_path: project_path.clone(),
        db_context: db_context.clone(),
        startup_project: startup_project.clone(),
    };

    // Upsert into app_config
    let (project_id, stable_migration) = {
        let mut ac = state.app_config.lock().unwrap();
        // Find existing by path or create new
        let (id, stable) = if let Some(existing) = ac.projects.iter_mut().find(|p| p.project_path == project_path) {
            existing.db_context = db_context.clone();
            existing.startup_project = startup_project.clone();
            (existing.id.clone(), existing.stable_migration.clone())
        } else {
            let id = generate_id();
            ac.projects.push(SavedProject {
                id: id.clone(),
                name: derive_project_name(&project_path),
                project_path: project_path.clone(),
                db_context: db_context.clone(),
                startup_project: startup_project.clone(),
                stable_migration: None,
            });
            (id, None)
        };
        ac.active_project_id = Some(id.clone());
        let _ = save_app_config(&app, &ac);
        (id, stable)
    };

    *state.config.lock().unwrap() = Some(config.clone());
    *state.current_branch.lock().unwrap() = branch.clone();

    Ok(ProjectInfo {
        id: Some(project_id),
        path: config.project_path,
        db_context: config.db_context,
        startup_project: config.startup_project,
        branch,
        stable_migration,
    })
}

#[tauri::command]
pub async fn get_project(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<ProjectInfo>, String> {
    // If config is already in memory, return it
    {
        let config = state.config.lock().unwrap();
        if config.is_some() {
            let branch = state.current_branch.lock().unwrap().clone();
            let ac = state.app_config.lock().unwrap();
            let active_id = ac.active_project_id.clone();
            let stable_migration = active_id.as_ref()
                .and_then(|id| ac.projects.iter().find(|p| &p.id == id))
                .and_then(|p| p.stable_migration.clone());
            return Ok(config.as_ref().map(|c| ProjectInfo {
                id: active_id,
                path: c.project_path.clone(),
                db_context: c.db_context.clone(),
                startup_project: c.startup_project.clone(),
                branch,
                stable_migration,
            }));
        }
    }

    // Try legacy migration first
    let legacy_info = if migrate_legacy_config(&app, &state).is_some() {
        let guard = state.config.lock().unwrap();
        guard.as_ref().map(|c| {
            let path = c.project_path.clone();
            let active_id = state.app_config.lock().unwrap().active_project_id.clone();
            (path, active_id)
        })
    } else {
        None
    };

    if let Some((branch_path, active_id)) = legacy_info {
        let branch = tokio::task::spawn_blocking(move || {
            GitService::get_current_branch(&branch_path).unwrap_or_default()
        })
        .await
        .map_err(|e| e.to_string())?;

        *state.current_branch.lock().unwrap() = branch.clone();

        let config = state.config.lock().unwrap();
        return Ok(config.as_ref().map(|c| ProjectInfo {
            id: active_id.clone(),
            path: c.project_path.clone(),
            db_context: c.db_context.clone(),
            startup_project: c.startup_project.clone(),
            branch,
            stable_migration: None, // legacy projects don't have stable migration
        }));
    }

    // Try loading from app_config.json
    let app_config_path = app_config_file_path(&app);
    if let Some(ref p) = app_config_path {
        if p.exists() {
            if let Ok(json) = std::fs::read_to_string(p) {
                if let Ok(ac) = serde_json::from_str::<AppConfig>(&json) {
                    *state.app_config.lock().unwrap() = ac.clone();

                    if let Some(ref active_id) = ac.active_project_id {
                        if let Some(proj) = ac.projects.iter().find(|p| &p.id == active_id) {
                            if Path::new(&proj.project_path).exists() {
                                let config = ProjectConfig {
                                    project_path: proj.project_path.clone(),
                                    db_context: proj.db_context.clone(),
                                    startup_project: proj.startup_project.clone(),
                                };

                                let pp = config.project_path.clone();
                                let branch = tokio::task::spawn_blocking(move || {
                                    GitService::get_current_branch(&pp).unwrap_or_default()
                                })
                                .await
                                .map_err(|e| e.to_string())?;

                                *state.config.lock().unwrap() = Some(config.clone());
                                *state.current_branch.lock().unwrap() = branch.clone();

                                return Ok(Some(ProjectInfo {
                                    id: Some(active_id.clone()),
                                    path: config.project_path,
                                    db_context: config.db_context,
                                    startup_project: config.startup_project,
                                    branch,
                                    stable_migration: proj.stable_migration.clone(),
                                }));
                            }
                        }
                    }
                }
            }
        }
    }

    // Fall back to legacy project_config.json
    let config_path = match config_file_path(&app) {
        Some(p) if p.exists() => p,
        _ => return Ok(None),
    };

    let json = match std::fs::read_to_string(&config_path) {
        Ok(j) => j,
        Err(_) => return Ok(None),
    };

    let config: ProjectConfig = match serde_json::from_str(&json) {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };

    if !Path::new(&config.project_path).exists() {
        return Ok(None);
    }

    let pp = config.project_path.clone();
    let branch = tokio::task::spawn_blocking(move || {
        GitService::get_current_branch(&pp).unwrap_or_default()
    })
    .await
    .map_err(|e| e.to_string())?;

    *state.config.lock().unwrap() = Some(config.clone());
    *state.current_branch.lock().unwrap() = branch.clone();

    Ok(Some(ProjectInfo {
        id: None,
        path: config.project_path,
        db_context: config.db_context,
        startup_project: config.startup_project,
        branch,
        stable_migration: None,
    }))
}

// ─── Saved Project Commands ─────────────────────────────────────────

#[tauri::command]
pub async fn get_saved_projects(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<SavedProject>, String> {
    // Ensure app_config is loaded
    {
        let ac = state.app_config.lock().unwrap();
        if !ac.projects.is_empty() {
            return Ok(ac.projects.clone());
        }
    }

    // Try loading from disk
    if let Some(p) = app_config_file_path(&app) {
        if p.exists() {
            if let Ok(json) = std::fs::read_to_string(&p) {
                if let Ok(ac) = serde_json::from_str::<AppConfig>(&json) {
                    let projects = ac.projects.clone();
                    *state.app_config.lock().unwrap() = ac;
                    return Ok(projects);
                }
            }
        }
    }

    // Try legacy migration
    if let Some(ac) = migrate_legacy_config(&app, &state) {
        return Ok(ac.projects);
    }

    Ok(vec![])
}

#[tauri::command]
pub async fn save_project(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    path: String,
    db_context: String,
    startup_project: String,
) -> Result<SavedProject, String> {
    if !Path::new(&path).exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    let id = generate_id();
    let saved = SavedProject {
        id: id.clone(),
        name,
        project_path: path,
        db_context,
        startup_project,
        stable_migration: None,
    };

    {
        let mut ac = state.app_config.lock().unwrap();
        ac.projects.push(saved.clone());
        save_app_config(&app, &ac)?;
    }

    Ok(saved)
}

#[tauri::command]
pub async fn update_saved_project(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    name: String,
    path: String,
    db_context: String,
    startup_project: String,
) -> Result<SavedProject, String> {
    let mut ac = state.app_config.lock().unwrap();
    let proj = ac.projects.iter_mut().find(|p| p.id == id)
        .ok_or_else(|| format!("Project not found: {}", id))?;

    proj.name = name;
    proj.project_path = path;
    proj.db_context = db_context;
    proj.startup_project = startup_project;

    let updated = proj.clone();
    save_app_config(&app, &ac)?;

    // If this is the active project, update the in-memory config too
    if ac.active_project_id.as_ref() == Some(&id) {
        *state.config.lock().unwrap() = Some(ProjectConfig {
            project_path: updated.project_path.clone(),
            db_context: updated.db_context.clone(),
            startup_project: updated.startup_project.clone(),
        });
    }

    Ok(updated)
}

#[tauri::command]
pub async fn delete_saved_project(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let mut ac = state.app_config.lock().unwrap();
    ac.projects.retain(|p| p.id != id);

    if ac.active_project_id.as_ref() == Some(&id) {
        ac.active_project_id = None;
        *state.config.lock().unwrap() = None;
    }

    save_app_config(&app, &ac)?;
    Ok(())
}

#[tauri::command]
pub async fn switch_project(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<ProjectInfo, String> {
    let project = {
        let mut ac = state.app_config.lock().unwrap();
        let proj = ac.projects.iter().find(|p| p.id == id)
            .ok_or_else(|| format!("Project not found: {}", id))?
            .clone();
        ac.active_project_id = Some(id.clone());
        save_app_config(&app, &ac)?;
        proj
    };

    if !Path::new(&project.project_path).exists() {
        return Err(format!("Path does not exist: {}", project.project_path));
    }

    let config = ProjectConfig {
        project_path: project.project_path.clone(),
        db_context: project.db_context.clone(),
        startup_project: project.startup_project.clone(),
    };

    let pp = config.project_path.clone();
    let branch = tokio::task::spawn_blocking(move || {
        GitService::get_current_branch(&pp).unwrap_or_default()
    })
    .await
    .map_err(|e| e.to_string())?;

    *state.config.lock().unwrap() = Some(config);
    *state.current_branch.lock().unwrap() = branch.clone();
    *state.watching.lock().unwrap() = false;
    *state.watching_migrations.lock().unwrap() = false;

    Ok(ProjectInfo {
        id: Some(project.id),
        path: project.project_path,
        db_context: project.db_context,
        startup_project: project.startup_project,
        branch,
        stable_migration: project.stable_migration,
    })
}

#[tauri::command]
pub async fn set_stable_migration(
    app: AppHandle,
    state: State<'_, AppState>,
    migration_name: Option<String>,
) -> Result<(), String> {
    let mut ac = state.app_config.lock().unwrap();
    let active_id = ac.active_project_id.clone()
        .ok_or("No active project")?;
    let proj = ac.projects.iter_mut().find(|p| p.id == active_id)
        .ok_or("Active project not found")?;
    proj.stable_migration = migration_name;
    save_app_config(&app, &ac)?;
    Ok(())
}

// ─── Migration Commands ─────────────────────────────────────────────

#[tauri::command]
pub async fn list_migrations(state: State<'_, AppState>) -> Result<Vec<Migration>, String> {
    let config = {
        let guard = state.config.lock().unwrap();
        guard.as_ref().ok_or("No project configured")?.clone()
    };

    let migrations = tokio::task::spawn_blocking(move || {
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

        Ok::<Vec<Migration>, String>(migrations)
    })
    .await
    .map_err(|e| e.to_string())??;

    *state.migrations.lock().unwrap() = migrations.clone();
    Ok(migrations)
}

#[tauri::command]
pub async fn add_migration(state: State<'_, AppState>, name: String) -> Result<String, String> {
    let config = {
        let guard = state.config.lock().unwrap();
        guard.as_ref().ok_or("No project configured")?.clone()
    };

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

    if result.success {
        Ok(format!("Migration created successfully"))
    } else {
        Err(format!("Failed to create migration: {}", result.error_output()))
    }
}

#[tauri::command]
pub async fn remove_migration(state: State<'_, AppState>, force: bool) -> Result<String, String> {
    let config = {
        let guard = state.config.lock().unwrap();
        guard.as_ref().ok_or("No project configured")?.clone()
    };

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

    if result.success {
        Ok("Last migration removed successfully".to_string())
    } else {
        Err(format!("Failed to remove migration: {}", result.error_output()))
    }
}

#[tauri::command]
pub async fn update_database(
    state: State<'_, AppState>,
    target: String,
) -> Result<String, String> {
    let config = {
        let guard = state.config.lock().unwrap();
        guard.as_ref().ok_or("No project configured")?.clone()
    };

    let target_clone = target.clone();
    let result = tokio::task::spawn_blocking(move || {
        DotnetEf::update_database(
            &config.project_path,
            &target_clone,
            &config.db_context,
            &config.startup_project,
        )
    })
    .await
    .map_err(|e| e.to_string())??;

    if result.success {
        if target.is_empty() {
            Ok("Database updated to latest migration".to_string())
        } else {
            Ok(format!("Database updated to migration: {}", target))
        }
    } else {
        Err(format!("Failed to update database: {}", result.error_output()))
    }
}

#[tauri::command]
pub async fn get_migration_sql(
    state: State<'_, AppState>,
    migration_name: String,
) -> Result<MigrationSqlInfo, String> {
    let config = {
        let guard = state.config.lock().unwrap();
        guard.as_ref().ok_or("No project configured")?.clone()
    };

    tokio::task::spawn_blocking(move || {
        let file_path = MigrationParser::get_migration_file(&config.project_path, &migration_name)
            .ok_or_else(|| {
                format!(
                    "Migration file not found for '{}' in project '{}'",
                    migration_name, config.project_path
                )
            })?;

        let parsed = MigrationParser::parse_file(&file_path)?;

        Ok(MigrationSqlInfo {
            name: parsed.file_name,
            up_body: parsed.up_body,
            down_body: parsed.down_body,
            custom_sql_up: parsed.custom_sql_up,
            custom_sql_down: parsed.custom_sql_down,
        })
    })
    .await
    .map_err(|e| e.to_string())?
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
pub async fn squash_migrations(
    state: State<'_, AppState>,
    from_migration: String,
    to_migration: String,
    new_name: String,
) -> Result<String, String> {
    let config = {
        let guard = state.config.lock().unwrap();
        guard.as_ref().ok_or("No project configured")?.clone()
    };
    let migrations = state.migrations.lock().unwrap().clone();

    let result = tokio::task::spawn_blocking(move || {
        // 1. Collect all custom SQL from migrations in the range
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
                update_result.error_output()
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
                    result.error_output()
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
                add_result.error_output()
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
                final_update.error_output()
            ));
        }

        Ok(format!(
            "Squashed {} migrations into '{}'. Custom SQL preserved: {} statements.",
            migrations_to_remove.len(),
            new_name,
            all_custom_sql.len()
        ))
    })
    .await
    .map_err(|e| e.to_string())??;

    Ok(result)
}

// ─── Script Command ─────────────────────────────────────────────────

#[tauri::command]
pub async fn generate_script(
    state: State<'_, AppState>,
    from: String,
    to: String,
) -> Result<String, String> {
    let config = {
        let guard = state.config.lock().unwrap();
        guard.as_ref().ok_or("No project configured")?.clone()
    };

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
        Err(format!("Failed to generate script: {}", result.error_output()))
    }
}

// ─── Git / Branch Commands ──────────────────────────────────────────

#[tauri::command]
pub async fn get_current_branch(state: State<'_, AppState>) -> Result<String, String> {
    let project_path = {
        let guard = state.config.lock().unwrap();
        guard.as_ref().ok_or("No project configured")?.project_path.clone()
    };

    let branch = tokio::task::spawn_blocking(move || {
        GitService::get_current_branch(&project_path)
    })
    .await
    .map_err(|e| e.to_string())??;

    *state.current_branch.lock().unwrap() = branch.clone();
    Ok(branch)
}

#[tauri::command]
pub async fn start_branch_watcher(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let config = {
        let guard = state.config.lock().unwrap();
        guard.as_ref().ok_or("No project configured")?.clone()
    };

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
    let db_context = config.db_context.clone();
    let startup_project = config.startup_project.clone();
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

#[tauri::command]
pub async fn start_migration_watcher(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let config = {
        let guard = state.config.lock().unwrap();
        guard.as_ref().ok_or("No project configured")?.clone()
    };

    let migrations_dir = MigrationParser::find_migrations_dir(&config.project_path)?;

    // Check if already watching
    {
        let watching = state.watching_migrations.lock().unwrap();
        if *watching {
            return Ok("Already watching for migration changes".to_string());
        }
    }

    *state.watching_migrations.lock().unwrap() = true;

    let migrations_dir_str = migrations_dir.to_string_lossy().to_string();

    thread::spawn(move || {
        let (tx, rx) = channel();
        let mut watcher = match RecommendedWatcher::new(tx, Config::default()) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("Failed to create migration watcher: {}", e);
                return;
            }
        };

        if let Err(e) = watcher.watch(Path::new(&migrations_dir_str), RecursiveMode::Recursive) {
            eprintln!("Failed to watch migrations directory: {}", e);
            return;
        }

        let mut last_emit = std::time::Instant::now();

        loop {
            match rx.recv() {
                Ok(Ok(event)) => {
                    // Only react to .cs file changes
                    let has_cs = event.paths.iter().any(|p| {
                        p.extension().and_then(|e| e.to_str()) == Some("cs")
                    });
                    if !has_cs {
                        continue;
                    }

                    // Debounce: skip if less than 1 second since last emit
                    let now = std::time::Instant::now();
                    if now.duration_since(last_emit) < std::time::Duration::from_secs(1) {
                        continue;
                    }
                    last_emit = now;

                    // Small delay to let file operations finish
                    thread::sleep(std::time::Duration::from_millis(500));

                    let _ = app.emit("migrations-changed", ());
                }
                Ok(Err(e)) => {
                    eprintln!("Migration watcher error: {}", e);
                }
                Err(_) => break,
            }
        }
    });

    Ok(format!(
        "Watching for migration changes: {}",
        migrations_dir.display()
    ))
}
