//! Tauri-independent orchestration layer.
//!
//! Every EF/git workflow lives here exactly once. Both frontends — the Tauri
//! command handlers in [`crate::commands`] and the MCP tools in
//! [`crate::mcp::server`] — are thin adapters that translate their transport's
//! request/response types and delegate the real work to these functions.
//!
//! The two abstractions that let one implementation serve both frontends:
//!
//! * [`PhaseSink`] — where progress updates go. The GUI wires it to Tauri
//!   `operation-phase` events; headless MCP uses [`NoopPhaseSink`].
//! * [`ConfigStore`] — where the saved-project list is persisted. The GUI
//!   writes to its bundle-scoped app-data dir via an `AppHandle`; the headless
//!   binary uses [`FileConfigStore`]. Neither frontend can "forget" to
//!   persist, because the mutating ops call `store.save` themselves.
//!
//! EF-mutating ops also acquire `state.op_mutex` internally, so no caller can
//! forget to serialize against a concurrent mutation from the other frontend.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::git::GitService;
use crate::state::{AppConfig, AppState, ProjectConfig};

// ─── Frontend-facing abstractions ───────────────────────────────────

/// Sink for multi-step progress updates. Implementations decide where phase
/// messages go; the orchestration code just calls `emit`.
pub trait PhaseSink: Send + 'static {
    fn emit(&self, phase: &'static str, message: String);
}

/// Progress sink that discards everything — used by headless callers (and the
/// MCP server) that have no event channel.
#[derive(Clone, Copy)]
pub struct NoopPhaseSink;

impl PhaseSink for NoopPhaseSink {
    fn emit(&self, _phase: &'static str, _message: String) {}
}

/// Persistence backend for the saved-project list. Mutating project/config ops
/// call `save` after updating the in-memory `AppConfig`, so persistence can
/// never drift from in-memory state.
pub trait ConfigStore: Send + Sync {
    fn save(&self, config: &AppConfig) -> Result<(), String>;
}

/// File-backed [`ConfigStore`] for the headless binary. Persists to the
/// platform data dir under `ezmigrations/app_config.json`. This is a *separate*
/// file from the GUI's bundle-scoped config — headless sessions get their own
/// durable config, independent of the desktop app.
pub struct FileConfigStore {
    path: PathBuf,
}

impl FileConfigStore {
    pub fn app_data() -> Self {
        let base = dirs::data_dir().unwrap_or_else(std::env::temp_dir);
        Self {
            path: base.join("ezmigrations").join("app_config.json"),
        }
    }

    /// Load a previously persisted config, or `None` if absent/unreadable.
    pub fn load(&self) -> Option<AppConfig> {
        let body = std::fs::read_to_string(&self.path).ok()?;
        serde_json::from_str(&body).ok()
    }
}

impl ConfigStore for FileConfigStore {
    fn save(&self, config: &AppConfig) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, json).map_err(|e| e.to_string())
    }
}

// ─── Shared result types ────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct ProjectInfo {
    pub id: Option<String>,
    pub path: String,
    pub db_context: String,
    pub startup_project: String,
    pub branch: String,
    pub stable_migration: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    pub name: String,
    pub is_remote: bool,
}

#[derive(Serialize)]
pub struct BranchSwitchResult {
    pub old_branch: String,
    pub new_branch: String,
    pub common_migration: Option<String>,
    pub rollback_target: Option<String>,
    pub rollback_performed: bool,
    pub target_migration_count: usize,
}

#[derive(Serialize)]
pub struct MigrationSqlInfo {
    pub name: String,
    pub up_body: String,
    pub down_body: String,
    pub custom_sql_up: Vec<String>,
    pub custom_sql_down: Vec<String>,
}

// ─── Shared helpers ─────────────────────────────────────────────────

/// Detect common EF Core misconfiguration and return a friendlier message.
pub fn enrich_ef_error(raw: &str) -> String {
    if raw.contains("doesn't match your migrations assembly") {
        return format!(
            "Project mismatch: your \"Migrations Project\" may be pointing to the startup/API \
             project instead of the project that contains your DbContext and migrations. \
             Check your project configuration and swap them if needed.\n\n{}",
            raw
        );
    }
    raw.to_string()
}

/// Migration name from a `*.cs` path, skipping `*.Designer.cs` and snapshots.
pub fn migration_name_from_path(path: &Path) -> Option<String> {
    let file_name = path.file_name()?.to_str()?;
    if !file_name.ends_with(".cs")
        || file_name.ends_with(".Designer.cs")
        || file_name.contains("ModelSnapshot")
    {
        return None;
    }
    path.file_stem()
        .and_then(|n| n.to_str())
        .map(ToString::to_string)
}

pub fn migration_name_from_git_path(path: &str) -> Option<String> {
    migration_name_from_path(Path::new(path))
}

pub fn migration_names_from_files(files: Vec<PathBuf>) -> Vec<String> {
    files
        .iter()
        .filter_map(|path| migration_name_from_path(path))
        .collect()
}

/// The newest migration in `current` that also exists in `target`.
pub fn latest_common_migration(current: &[String], target: &[String]) -> Option<String> {
    let target_names: HashSet<&str> = target.iter().map(String::as_str).collect();
    current
        .iter()
        .rev()
        .find(|name| target_names.contains(name.as_str()))
        .cloned()
}

/// A git-pathspec-relative form of `path`, validated to live inside `repo_root`.
pub fn path_relative_to_repo(repo_root: &str, path: &Path) -> Result<String, String> {
    let root = std::fs::canonicalize(repo_root)
        .map_err(|e| format!("Failed to resolve git root '{}': {}", repo_root, e))?;
    let path = std::fs::canonicalize(path)
        .map_err(|e| format!("Failed to resolve path '{}': {}", path.display(), e))?;
    let relative = path.strip_prefix(&root).map_err(|_| {
        format!(
            "Migrations directory '{}' is not inside git repository '{}'",
            path.display(),
            root.display()
        )
    })?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

pub fn ensure_path_exists(path: &str) -> Result<(), String> {
    if Path::new(path).exists() {
        Ok(())
    } else {
        Err(format!("Path does not exist: {}", path))
    }
}

pub fn derive_project_name(project_path: &str) -> String {
    Path::new(project_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "My Project".to_string())
}

/// Monotonic-ish id derived from the wall clock (ms since epoch).
pub fn generate_id() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}

/// The active project's config, or an error if none is loaded.
pub fn require_project(state: &AppState) -> Result<ProjectConfig, String> {
    state
        .config
        .lock()
        .unwrap()
        .as_ref()
        .cloned()
        .ok_or_else(|| "No project configured".to_string())
}

pub(crate) fn check_cancel(cancel: &Arc<AtomicBool>) -> Result<(), String> {
    if cancel.load(Ordering::SeqCst) {
        Err("Canceled by user.".to_string())
    } else {
        Ok(())
    }
}

/// Resolve the active branch off the tokio worker pool.
pub(crate) async fn current_branch(project_path: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        GitService::get_current_branch(&project_path).unwrap_or_default()
    })
    .await
    .map_err(|e| e.to_string())
}

// ─── Submodules ─────────────────────────────────────────────────────
//
// Operations are grouped by domain; the abstractions, shared result types,
// and helpers above are used by all three. Re-exported flat so callers keep
// using `ops::<fn>` regardless of which file a given operation lives in.

mod mutations;
mod projects;
mod reads;

pub use mutations::*;
pub use projects::*;
pub use reads::*;
