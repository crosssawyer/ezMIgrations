use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;

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
    /// Held by every EF-mutating command (and the MCP tool wrappers) for the
    /// duration of the operation, so concurrent calls from the GUI and the MCP
    /// server can't stomp on each other. Async-aware so async tool handlers
    /// don't block a tokio worker thread while they wait.
    pub op_mutex: Arc<AsyncMutex<()>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_config_roundtrips_through_json() {
        let original = ProjectConfig {
            project_path: "/tmp/proj".to_string(),
            db_context: "AppDb".to_string(),
            startup_project: "/tmp/api".to_string(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let back: ProjectConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.project_path, original.project_path);
        assert_eq!(back.db_context, original.db_context);
        assert_eq!(back.startup_project, original.startup_project);
    }

    #[test]
    fn saved_project_roundtrips_with_stable_migration() {
        let original = SavedProject {
            id: "1".to_string(),
            name: "App".to_string(),
            project_path: "/p".to_string(),
            db_context: "Ctx".to_string(),
            startup_project: "/s".to_string(),
            stable_migration: Some("20240101_Init".to_string()),
        };
        let json = serde_json::to_string(&original).unwrap();
        let back: SavedProject = serde_json::from_str(&json).unwrap();
        assert_eq!(back.stable_migration.as_deref(), Some("20240101_Init"));
        assert_eq!(back.name, "App");
    }

    #[test]
    fn saved_project_roundtrips_without_stable_migration() {
        let original = SavedProject {
            stable_migration: None,
            ..SavedProject::default()
        };
        let json = serde_json::to_string(&original).unwrap();
        let back: SavedProject = serde_json::from_str(&json).unwrap();
        assert!(back.stable_migration.is_none());
    }

    #[test]
    fn preferences_default_notifies_on_branch_change() {
        let prefs = Preferences::default();
        assert!(prefs.notify_on_branch_change);
    }

    #[test]
    fn preferences_default_applied_when_field_missing_in_json() {
        // Forward-compat: older configs may not have the field at all.
        let json = "{}";
        let prefs: Preferences = serde_json::from_str(json).unwrap();
        assert!(prefs.notify_on_branch_change);
    }

    #[test]
    fn preferences_respect_explicit_false() {
        let json = r#"{"notify_on_branch_change": false}"#;
        let prefs: Preferences = serde_json::from_str(json).unwrap();
        assert!(!prefs.notify_on_branch_change);
    }

    #[test]
    fn app_config_roundtrips_full_state() {
        let original = AppConfig {
            projects: vec![SavedProject {
                id: "abc".to_string(),
                name: "Test".to_string(),
                project_path: "/x".to_string(),
                db_context: "C".to_string(),
                startup_project: "/y".to_string(),
                stable_migration: Some("M".to_string()),
            }],
            active_project_id: Some("abc".to_string()),
            preferences: Preferences {
                notify_on_branch_change: false,
            },
        };
        let json = serde_json::to_string(&original).unwrap();
        let back: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.projects.len(), 1);
        assert_eq!(back.projects[0].id, "abc");
        assert_eq!(back.active_project_id.as_deref(), Some("abc"));
        assert!(!back.preferences.notify_on_branch_change);
    }

    #[test]
    fn app_config_loads_legacy_json_without_preferences() {
        // Legacy app_config.json from before the preferences field was added.
        let json = r#"{
            "projects": [],
            "active_project_id": null
        }"#;
        let cfg: AppConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.projects.is_empty());
        assert!(cfg.active_project_id.is_none());
        // Should fall back to Preferences::default()
        assert!(cfg.preferences.notify_on_branch_change);
    }

    #[test]
    fn migration_roundtrips_through_json() {
        let original = Migration {
            id: "M1".to_string(),
            name: "M1".to_string(),
            applied: true,
            has_custom_sql: true,
            custom_sql_up: vec!["SELECT 1".to_string()],
            custom_sql_down: vec!["DROP".to_string()],
            file_path: Some("/p/M1.cs".to_string()),
        };
        let json = serde_json::to_string(&original).unwrap();
        let back: Migration = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "M1");
        assert!(back.applied);
        assert!(back.has_custom_sql);
        assert_eq!(back.custom_sql_up, vec!["SELECT 1".to_string()]);
        assert_eq!(back.file_path.as_deref(), Some("/p/M1.cs"));
    }
}
