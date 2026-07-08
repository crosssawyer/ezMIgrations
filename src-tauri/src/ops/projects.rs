//! Project and preferences mutations. Each updates in-memory `AppState`
//! and persists through the injected `ConfigStore`.

use super::*;
use crate::state::{AppState, Preferences, ProjectConfig, SavedProject};

// ─── Project / config mutations (persisted via ConfigStore) ──────────

/// Make `project_path` the active project: upsert it into the saved-project
/// list, activate it, load its branch, and persist. Returns the active info.
pub async fn set_project(
    state: &AppState,
    store: &dyn ConfigStore,
    project_path: String,
    db_context: String,
    startup_project: String,
) -> Result<ProjectInfo, String> {
    ensure_path_exists(&project_path)?;

    let branch = current_branch(project_path.clone()).await?;

    let config = ProjectConfig {
        project_path: project_path.clone(),
        db_context: db_context.clone(),
        startup_project: startup_project.clone(),
    };

    let (project_id, stable_migration) = {
        let mut ac = state.app_config.lock().unwrap();
        let (id, stable) = if let Some(existing) = ac
            .projects
            .iter_mut()
            .find(|p| p.project_path == project_path)
        {
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
        store.save(&ac)?;
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

/// Add a saved project without activating it.
pub fn save_project(
    state: &AppState,
    store: &dyn ConfigStore,
    name: String,
    path: String,
    db_context: String,
    startup_project: String,
) -> Result<SavedProject, String> {
    ensure_path_exists(&path)?;

    let saved = SavedProject {
        id: generate_id(),
        name,
        project_path: path,
        db_context,
        startup_project,
        stable_migration: None,
    };

    let mut ac = state.app_config.lock().unwrap();
    ac.projects.push(saved.clone());
    store.save(&ac)?;
    Ok(saved)
}

/// Edit a saved project's metadata. Keeps the active in-memory config in sync
/// when the edited project is the active one.
pub fn update_saved_project(
    state: &AppState,
    store: &dyn ConfigStore,
    id: String,
    name: String,
    path: String,
    db_context: String,
    startup_project: String,
) -> Result<SavedProject, String> {
    ensure_path_exists(&path)?;

    let mut ac = state.app_config.lock().unwrap();
    let proj = ac
        .projects
        .iter_mut()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("Project not found: {}", id))?;

    proj.name = name;
    proj.project_path = path;
    proj.db_context = db_context;
    proj.startup_project = startup_project;
    let updated = proj.clone();
    store.save(&ac)?;

    if ac.active_project_id.as_ref() == Some(&id) {
        *state.config.lock().unwrap() = Some(ProjectConfig {
            project_path: updated.project_path.clone(),
            db_context: updated.db_context.clone(),
            startup_project: updated.startup_project.clone(),
        });
    }

    Ok(updated)
}

/// Delete a saved project. Returns `true` if the deleted project was active
/// (so the caller can run any GUI-only teardown, e.g. watchers).
pub fn delete_saved_project(
    state: &AppState,
    store: &dyn ConfigStore,
    id: String,
) -> Result<bool, String> {
    let mut ac = state.app_config.lock().unwrap();
    ac.projects.retain(|p| p.id != id);

    let was_active = ac.active_project_id.as_ref() == Some(&id);
    if was_active {
        ac.active_project_id = None;
        *state.config.lock().unwrap() = None;
        *state.current_branch.lock().unwrap() = String::new();
        state.migrations.lock().unwrap().clear();
    }

    store.save(&ac)?;
    Ok(was_active)
}

/// Activate a saved project by id and load its branch.
pub async fn switch_project(
    state: &AppState,
    store: &dyn ConfigStore,
    id: String,
) -> Result<ProjectInfo, String> {
    let project = {
        let ac = state.app_config.lock().unwrap();
        ac.projects
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("Project not found: {}", id))?
            .clone()
    };

    ensure_path_exists(&project.project_path)?;

    {
        let mut ac = state.app_config.lock().unwrap();
        ac.active_project_id = Some(id.clone());
        store.save(&ac)?;
    }

    let config = ProjectConfig {
        project_path: project.project_path.clone(),
        db_context: project.db_context.clone(),
        startup_project: project.startup_project.clone(),
    };
    let branch = current_branch(config.project_path.clone()).await?;

    *state.config.lock().unwrap() = Some(config);
    *state.current_branch.lock().unwrap() = branch.clone();

    Ok(ProjectInfo {
        id: Some(project.id),
        path: project.project_path,
        db_context: project.db_context,
        startup_project: project.startup_project,
        branch,
        stable_migration: project.stable_migration,
    })
}

/// Pin (or clear, with `None`) the stable rollback migration for the active project.
pub fn set_stable_migration(
    state: &AppState,
    store: &dyn ConfigStore,
    migration_name: Option<String>,
) -> Result<(), String> {
    let mut ac = state.app_config.lock().unwrap();
    let active_id = ac.active_project_id.clone().ok_or("No active project")?;
    let proj = ac
        .projects
        .iter_mut()
        .find(|p| p.id == active_id)
        .ok_or("Active project not found")?;
    proj.stable_migration = migration_name;
    store.save(&ac)?;
    Ok(())
}

/// Replace user preferences.
pub fn set_preferences(
    state: &AppState,
    store: &dyn ConfigStore,
    preferences: Preferences,
) -> Result<(), String> {
    let mut ac = state.app_config.lock().unwrap();
    ac.preferences = preferences;
    store.save(&ac)?;
    Ok(())
}
