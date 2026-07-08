use crate::git::GitService;
use crate::ops::{self, BranchInfo, BranchSwitchResult, MigrationSqlInfo, PhaseSink, ProjectInfo};
use crate::parser::MigrationParser;
use crate::state::{AppConfig, AppState, Migration, Preferences, ProjectConfig, SavedProject};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use std::path::Path;
use std::sync::mpsc::channel;
use std::sync::{atomic::AtomicBool, Arc};
use std::thread;
use tauri::{AppHandle, Emitter, Manager, State};

// ─── Tauri frontend adapters ────────────────────────────────────────

/// `operation-phase` event payload consumed by the desktop frontend.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OperationPhaseEvent {
    operation: &'static str,
    phase: &'static str,
    message: String,
}

/// [`PhaseSink`] that forwards progress to the Tauri window as
/// `operation-phase` events tagged with the originating operation name.
#[derive(Clone)]
struct TauriPhaseSink {
    app: AppHandle,
    operation: &'static str,
}

impl TauriPhaseSink {
    fn new(app: &AppHandle, operation: &'static str) -> Self {
        Self {
            app: app.clone(),
            operation,
        }
    }
}

impl PhaseSink for TauriPhaseSink {
    fn emit(&self, phase: &'static str, message: String) {
        let _ = self.app.emit(
            "operation-phase",
            OperationPhaseEvent {
                operation: self.operation,
                phase,
                message,
            },
        );
    }
}

/// [`ConfigStore`](crate::ops::ConfigStore) backed by the Tauri app-data dir,
/// so saved-project mutations persist to `app_config.json`.
pub struct TauriConfigStore {
    app: AppHandle,
}

impl TauriConfigStore {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl crate::ops::ConfigStore for TauriConfigStore {
    fn save(&self, config: &AppConfig) -> Result<(), String> {
        save_app_config(&self.app, config)
    }
}

// ─── Helpers ────────────────────────────────────────────────────────

fn config_file_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|d| d.join("project_config.json"))
}

fn app_config_file_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|d| d.join("app_config.json"))
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

pub(crate) fn reset_watchers(state: &AppState) {
    {
        let mut cancel = state.watcher_cancel.lock().unwrap();
        cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        *cancel = Arc::new(AtomicBool::new(false));
    }
    *state.watching.lock().unwrap() = false;
    *state.watching_migrations.lock().unwrap() = false;
}

fn clear_branch_watcher_if_current(state: &AppState, cancel_token: &Arc<AtomicBool>) {
    let current = state.watcher_cancel.lock().unwrap().clone();
    if Arc::ptr_eq(&current, cancel_token) {
        *state.watching.lock().unwrap() = false;
    }
}

fn clear_migration_watcher_if_current(state: &AppState, cancel_token: &Arc<AtomicBool>) {
    let current = state.watcher_cancel.lock().unwrap().clone();
    if Arc::ptr_eq(&current, cancel_token) {
        *state.watching_migrations.lock().unwrap() = false;
    }
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

    let id = ops::generate_id();
    let saved = SavedProject {
        id: id.clone(),
        name: ops::derive_project_name(&legacy.project_path),
        project_path: legacy.project_path.clone(),
        db_context: legacy.db_context.clone(),
        startup_project: legacy.startup_project.clone(),
        stable_migration: None,
    };

    let app_config = AppConfig {
        projects: vec![saved],
        active_project_id: Some(id),
        preferences: Preferences::default(),
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
    state: State<'_, Arc<AppState>>,
    project_path: String,
    db_context: String,
    startup_project: String,
) -> Result<ProjectInfo, String> {
    let store = TauriConfigStore::new(app.clone());
    let info = ops::set_project(&state, &store, project_path, db_context, startup_project).await?;
    reset_watchers(&state);
    Ok(info)
}

#[tauri::command]
pub async fn get_project(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<ProjectInfo>, String> {
    // If config is already in memory, return it
    {
        let config = state.config.lock().unwrap();
        if config.is_some() {
            let branch = state.current_branch.lock().unwrap().clone();
            let ac = state.app_config.lock().unwrap();
            let active_id = ac.active_project_id.clone();
            let stable_migration = active_id
                .as_ref()
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
    state: State<'_, Arc<AppState>>,
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
    state: State<'_, Arc<AppState>>,
    name: String,
    path: String,
    db_context: String,
    startup_project: String,
) -> Result<SavedProject, String> {
    let store = TauriConfigStore::new(app.clone());
    ops::save_project(&state, &store, name, path, db_context, startup_project)
}

#[tauri::command]
pub async fn update_saved_project(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
    name: String,
    path: String,
    db_context: String,
    startup_project: String,
) -> Result<SavedProject, String> {
    let store = TauriConfigStore::new(app.clone());
    ops::update_saved_project(&state, &store, id, name, path, db_context, startup_project)
}

#[tauri::command]
pub async fn delete_saved_project(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    let store = TauriConfigStore::new(app.clone());
    if ops::delete_saved_project(&state, &store, id)? {
        // The active project was removed — tear down its GUI watchers.
        reset_watchers(&state);
    }
    Ok(())
}

#[tauri::command]
pub async fn switch_project(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<ProjectInfo, String> {
    let store = TauriConfigStore::new(app.clone());
    let info = ops::switch_project(&state, &store, id).await?;
    reset_watchers(&state);
    Ok(info)
}

#[tauri::command]
pub async fn set_stable_migration(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    migration_name: Option<String>,
) -> Result<(), String> {
    let store = TauriConfigStore::new(app.clone());
    ops::set_stable_migration(&state, &store, migration_name)
}

// ─── Preferences Commands ───────────────────────────────────────────

#[tauri::command]
pub async fn get_preferences(state: State<'_, Arc<AppState>>) -> Result<Preferences, String> {
    let ac = state.app_config.lock().unwrap();
    Ok(ac.preferences.clone())
}

#[tauri::command]
pub async fn set_preferences(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    preferences: Preferences,
) -> Result<(), String> {
    let store = TauriConfigStore::new(app.clone());
    ops::set_preferences(&state, &store, preferences)
}

// ─── Migration Commands ─────────────────────────────────────────────

#[tauri::command]
pub async fn list_migrations(state: State<'_, Arc<AppState>>) -> Result<Vec<Migration>, String> {
    ops::list_migrations(&state).await
}

#[tauri::command]
pub async fn add_migration(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    name: String,
) -> Result<String, String> {
    ops::add_migration(&state, name, TauriPhaseSink::new(&app, "add_migration")).await
}

#[tauri::command]
pub async fn remove_migration(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    force: bool,
) -> Result<String, String> {
    ops::remove_migration(&state, force, TauriPhaseSink::new(&app, "remove_migration")).await
}

#[tauri::command]
pub async fn update_database(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    target: String,
) -> Result<String, String> {
    ops::update_database(&state, target, TauriPhaseSink::new(&app, "update_database")).await
}

#[tauri::command]
pub async fn cancel_running_operation(state: State<'_, Arc<AppState>>) -> Result<String, String> {
    ops::cancel_running_operation(&state).await
}

#[tauri::command]
pub async fn get_migration_sql(
    state: State<'_, Arc<AppState>>,
    migration_name: String,
) -> Result<MigrationSqlInfo, String> {
    ops::get_migration_sql(&state, migration_name).await
}

// ─── Squash Command ─────────────────────────────────────────────────

#[tauri::command]
pub async fn squash_migrations(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    from_migration: String,
    to_migration: String,
    new_name: String,
) -> Result<String, String> {
    ops::squash_migrations(
        &state,
        from_migration,
        to_migration,
        new_name,
        TauriPhaseSink::new(&app, "squash"),
    )
    .await
}

// ─── Script Command ─────────────────────────────────────────────────

#[tauri::command]
pub async fn generate_script(
    state: State<'_, Arc<AppState>>,
    from: String,
    to: String,
) -> Result<String, String> {
    ops::generate_script(&state, from, to).await
}

// ─── Git / Branch Commands ──────────────────────────────────────────

#[tauri::command]
pub async fn list_git_branches(state: State<'_, Arc<AppState>>) -> Result<Vec<BranchInfo>, String> {
    ops::list_git_branches(&state).await
}

#[tauri::command]
pub async fn fetch_remote(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    ops::fetch_remote(&state).await
}

#[tauri::command]
pub async fn switch_branch_with_migrations(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    target_branch: String,
) -> Result<BranchSwitchResult, String> {
    ops::switch_branch_with_migrations(
        &state,
        target_branch,
        TauriPhaseSink::new(&app, "switch_branch"),
    )
    .await
}

#[tauri::command]
pub async fn get_current_branch(state: State<'_, Arc<AppState>>) -> Result<String, String> {
    ops::get_current_branch(&state).await
}

#[tauri::command]
pub async fn start_branch_watcher(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<String, String> {
    let config = {
        let guard = state.config.lock().unwrap();
        guard.as_ref().ok_or("No project configured")?.clone()
    };

    let head_path =
        GitService::get_head_path(&config.project_path).ok_or("Could not find .git/HEAD")?;

    // Check if already watching
    {
        let watching = state.watching.lock().unwrap();
        if *watching {
            return Ok("Already watching for branch changes".to_string());
        }
    }

    let cancel_token = state.watcher_cancel.lock().unwrap().clone();
    *state.watching.lock().unwrap() = true;

    let project_path = config.project_path.clone();
    let head_path_clone = head_path.clone();

    thread::spawn(move || {
        let (tx, rx) = channel();
        let mut watcher = match RecommendedWatcher::new(tx, Config::default()) {
            Ok(w) => w,
            Err(e) => {
                clear_branch_watcher_if_current(&app.state::<Arc<AppState>>(), &cancel_token);
                eprintln!("Failed to create watcher: {}", e);
                return;
            }
        };

        let parent = Path::new(&head_path_clone).parent().unwrap();
        if let Err(e) = watcher.watch(parent, RecursiveMode::NonRecursive) {
            clear_branch_watcher_if_current(&app.state::<Arc<AppState>>(), &cancel_token);
            eprintln!("Failed to watch .git directory: {}", e);
            return;
        }

        let head_file = Path::new(&head_path_clone)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let mut last_branch = GitService::get_current_branch(&project_path).unwrap_or_default();
        let mut last_check = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(10))
            .unwrap_or_else(std::time::Instant::now);

        loop {
            // Check cancellation
            if cancel_token.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }

            match rx.recv_timeout(std::time::Duration::from_secs(2)) {
                Ok(Ok(event)) => {
                    // Only react to HEAD file changes
                    let is_head = event.paths.iter().any(|p| {
                        p.file_name()
                            .and_then(|n| n.to_str())
                            .map(|n| n == head_file)
                            .unwrap_or(false)
                    });
                    if !is_head {
                        continue;
                    }

                    // Debounce
                    let now = std::time::Instant::now();
                    if now.duration_since(last_check) < std::time::Duration::from_secs(2) {
                        continue;
                    }
                    last_check = now;

                    // Delay to let git finish writing
                    thread::sleep(std::time::Duration::from_millis(500));

                    // Drain queued events
                    while rx.try_recv().is_ok() {}

                    if let Ok(new_branch) = GitService::get_current_branch(&project_path) {
                        if new_branch != last_branch {
                            let old = last_branch.clone();
                            last_branch = new_branch.clone();

                            let _ = app.emit(
                                "branch-changed",
                                BranchChangeEvent {
                                    old_branch: old,
                                    new_branch,
                                    reverted_to_stable: false,
                                },
                            );
                        }
                    }
                }
                Ok(Err(_)) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        clear_branch_watcher_if_current(&app.state::<Arc<AppState>>(), &cancel_token);
    });

    Ok(format!("Watching for branch changes: {}", head_path))
}

#[derive(Clone, Serialize)]
struct BranchChangeEvent {
    old_branch: String,
    new_branch: String,
    reverted_to_stable: bool,
}

#[tauri::command]
pub async fn start_migration_watcher(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
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

    let cancel_token = state.watcher_cancel.lock().unwrap().clone();
    *state.watching_migrations.lock().unwrap() = true;

    let migrations_dir_str = migrations_dir.to_string_lossy().to_string();

    thread::spawn(move || {
        let (tx, rx) = channel();
        let mut watcher = match RecommendedWatcher::new(tx, Config::default()) {
            Ok(w) => w,
            Err(e) => {
                clear_migration_watcher_if_current(&app.state::<Arc<AppState>>(), &cancel_token);
                eprintln!("Failed to create migration watcher: {}", e);
                return;
            }
        };

        if let Err(e) = watcher.watch(Path::new(&migrations_dir_str), RecursiveMode::Recursive) {
            clear_migration_watcher_if_current(&app.state::<Arc<AppState>>(), &cancel_token);
            eprintln!("Failed to watch migrations directory: {}", e);
            return;
        }

        let mut last_emit = std::time::Instant::now();

        loop {
            // Check cancellation
            if cancel_token.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }

            match rx.recv_timeout(std::time::Duration::from_secs(2)) {
                Ok(Ok(event)) => {
                    // Only react to main migration .cs files (skip .Designer.cs and snapshots)
                    let has_migration_cs = event.paths.iter().any(|p| {
                        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        name.ends_with(".cs")
                            && !name.ends_with(".Designer.cs")
                            && !name.contains("ModelSnapshot")
                    });
                    if !has_migration_cs {
                        continue;
                    }

                    // Debounce: skip if less than 3 seconds since last emit
                    let now = std::time::Instant::now();
                    if now.duration_since(last_emit) < std::time::Duration::from_secs(3) {
                        continue;
                    }
                    last_emit = now;

                    // Delay to let file operations finish
                    thread::sleep(std::time::Duration::from_millis(500));

                    // Drain any events that queued during the sleep
                    while rx.try_recv().is_ok() {}

                    let _ = app.emit("migrations-changed", ());
                }
                Ok(Err(e)) => {
                    eprintln!("Migration watcher error: {}", e);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        clear_migration_watcher_if_current(&app.state::<Arc<AppState>>(), &cancel_token);
    });

    Ok(format!(
        "Watching for migration changes: {}",
        migrations_dir.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::{
        derive_project_name, enrich_ef_error, ensure_path_exists, generate_id,
        latest_common_migration, migration_name_from_git_path, migration_name_from_path,
        migration_names_from_files, path_relative_to_repo,
    };
    use std::path::PathBuf;

    // ─── enrich_ef_error ────────────────────────────────────────

    #[test]
    fn enrich_ef_error_passes_through_unrelated_errors() {
        let raw = "Something unexpected happened";
        assert_eq!(enrich_ef_error(raw), raw);
    }

    #[test]
    fn enrich_ef_error_explains_migrations_assembly_mismatch() {
        let raw = "Your target project 'Foo' doesn't match your migrations assembly 'Bar'.";
        let enriched = enrich_ef_error(raw);
        assert!(enriched.contains("Project mismatch"));
        assert!(enriched.contains("Migrations Project"));
        // Original raw error should still be appended for context
        assert!(enriched.contains(raw));
    }

    // ─── migration_name_from_path ───────────────────────────────

    #[test]
    fn migration_name_extracted_from_cs_file() {
        let name = migration_name_from_path(Path::new("/a/b/20240101_Init.cs"));
        assert_eq!(name.as_deref(), Some("20240101_Init"));
    }

    #[test]
    fn migration_name_rejects_designer_files() {
        let name = migration_name_from_path(Path::new("/a/b/20240101_Init.Designer.cs"));
        assert!(name.is_none());
    }

    #[test]
    fn migration_name_rejects_model_snapshot() {
        let name = migration_name_from_path(Path::new("/a/b/MyContextModelSnapshot.cs"));
        assert!(name.is_none());
    }

    #[test]
    fn migration_name_rejects_non_cs_files() {
        let name = migration_name_from_path(Path::new("/a/b/notes.txt"));
        assert!(name.is_none());
    }

    #[test]
    fn migration_name_handles_forward_slash_git_path() {
        let name = migration_name_from_git_path("MyProj/Migrations/20240101_Init.cs");
        assert_eq!(name.as_deref(), Some("20240101_Init"));
    }

    #[test]
    fn migration_name_from_git_path_rejects_designer() {
        let name = migration_name_from_git_path("MyProj/Migrations/20240101_Init.Designer.cs");
        assert!(name.is_none());
    }

    // ─── migration_names_from_files ─────────────────────────────

    #[test]
    fn migration_names_filters_designer_and_snapshot() {
        let files = vec![
            PathBuf::from("/p/20240101_A.cs"),
            PathBuf::from("/p/20240101_A.Designer.cs"),
            PathBuf::from("/p/20240202_B.cs"),
            PathBuf::from("/p/MyDbContextModelSnapshot.cs"),
            PathBuf::from("/p/readme.md"),
        ];
        let names = migration_names_from_files(files);
        assert_eq!(
            names,
            vec!["20240101_A".to_string(), "20240202_B".to_string()]
        );
    }

    // ─── latest_common_migration ────────────────────────────────

    #[test]
    fn latest_common_migration_returns_most_recent_shared() {
        let current = vec!["M1".to_string(), "M2".to_string(), "M3".to_string()];
        let target = vec!["M1".to_string(), "M2".to_string(), "M4".to_string()];
        let common = latest_common_migration(&current, &target);
        assert_eq!(common.as_deref(), Some("M2"));
    }

    #[test]
    fn latest_common_migration_returns_none_when_no_overlap() {
        let current = vec!["A".to_string(), "B".to_string()];
        let target = vec!["X".to_string(), "Y".to_string()];
        let common = latest_common_migration(&current, &target);
        assert!(common.is_none());
    }

    #[test]
    fn latest_common_migration_handles_identical_lists() {
        let current = vec!["M1".to_string(), "M2".to_string()];
        let target = current.clone();
        let common = latest_common_migration(&current, &target);
        assert_eq!(common.as_deref(), Some("M2"));
    }

    #[test]
    fn latest_common_migration_handles_empty_inputs() {
        assert!(latest_common_migration(&[], &["A".to_string()]).is_none());
        assert!(latest_common_migration(&["A".to_string()], &[]).is_none());
        assert!(latest_common_migration(&[], &[]).is_none());
    }

    // ─── derive_project_name ────────────────────────────────────

    #[test]
    fn derive_project_name_uses_directory_basename() {
        assert_eq!(derive_project_name("/home/user/MyApp"), "MyApp");
    }

    #[test]
    fn derive_project_name_falls_back_when_path_is_empty() {
        assert_eq!(derive_project_name(""), "My Project");
    }

    // ─── ensure_path_exists ─────────────────────────────────────

    #[test]
    fn ensure_path_exists_ok_for_real_path() {
        let dir = tempfile::tempdir().unwrap();
        let result = ensure_path_exists(dir.path().to_str().unwrap());
        assert!(result.is_ok());
    }

    #[test]
    fn ensure_path_exists_errors_for_missing_path() {
        let result = ensure_path_exists("/this/path/definitely/does/not/exist/abc123");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    // ─── generate_id ────────────────────────────────────────────

    #[test]
    fn generate_id_returns_non_empty_numeric_string() {
        let id = generate_id();
        assert!(!id.is_empty());
        assert!(id.chars().all(|c| c.is_ascii_digit()));
    }

    // ─── path_relative_to_repo ──────────────────────────────────

    #[test]
    fn path_relative_to_repo_normalizes_backslashes_to_forward_slash() {
        let repo = tempfile::tempdir().unwrap();
        let nested = repo.path().join("a").join("b");
        std::fs::create_dir_all(&nested).unwrap();

        let rel = path_relative_to_repo(repo.path().to_str().unwrap(), &nested).unwrap();
        assert!(!rel.contains('\\'));
        // On both Unix and Windows the result should be a/b
        assert_eq!(rel.replace('\\', "/"), "a/b");
    }

    #[test]
    fn path_relative_to_repo_errors_when_outside_repo() {
        let repo = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();

        let result = path_relative_to_repo(repo.path().to_str().unwrap(), outside.path());
        assert!(result.is_err());
    }
}
