//! `EzMigrationsServer` — the MCP server backing ezMigrations.
//!
//! Mounted on Axum by `build_axum_router`, this exposes every Tauri command in
//! the desktop app as an MCP tool plus a handful of read-only resources for
//! state inspection. State is shared with the GUI via `Arc<AppState>` so an
//! MCP-driven mutation lands in the same in-memory caches the GUI reads.
//!
//! The tool bodies deliberately duplicate the orchestration in
//! `commands.rs`. We can't reuse those functions directly because they
//! depend on `tauri::State` and `tauri::AppHandle`, and we don't want to
//! refactor `commands.rs` while SWE-A is editing it. The 5 mutating tools
//! also acquire `state.op_mutex` to serialize against the GUI.

use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, Content, Implementation, ListResourceTemplatesResult, ListResourcesResult,
    PaginatedRequestParams, ProtocolVersion, RawResource, RawResourceTemplate,
    ReadResourceRequestParams, ReadResourceResult, ResourceContents, ServerCapabilities,
    ServerInfo,
};
use rmcp::service::RequestContext;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use rmcp::{
    handler::server::router::tool::ToolRouter, schemars, tool, tool_handler, tool_router,
    ErrorData as McpError, RoleServer, ServerHandler,
};
use serde::Deserialize;
use serde_json::json;

use crate::commands::reset_watchers;
use crate::dotnet::DotnetEf;
use crate::git::GitService;
use crate::mcp::instructions::INSTRUCTIONS;
use crate::mcp::resources::{self, uri};
use crate::parser::{KeepStrategy, MigrationParser, SqlStatement};
use crate::state::{AppState, Preferences, ProjectConfig, SavedProject};

/// Schema-bearing mirror of `state::Preferences` so the `set_preferences`
/// tool can publish an input schema without touching SWE-A's `state.rs`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PreferencesArg {
    /// Show a dialog when the file-watcher detects an external branch
    /// change. Mirrors `state::Preferences::notify_on_branch_change`.
    pub notify_on_branch_change: bool,
}

impl From<PreferencesArg> for Preferences {
    fn from(arg: PreferencesArg) -> Self {
        Self {
            notify_on_branch_change: arg.notify_on_branch_change,
        }
    }
}

/// Path the MCP service is nested at. SWE-A's port file should advertise
/// `http://127.0.0.1:<port>/mcp` as the `url` field.
pub const MCP_ROUTE: &str = "/mcp";

/// Build the Axum router that hosts the MCP service. SWE-A's bootstrap calls
/// this with the shared `Arc<AppState>` and binds the returned router to a
/// random loopback port.
pub fn build_axum_router(state: Arc<AppState>) -> axum::Router {
    let service = StreamableHttpService::new(
        move || Ok(EzMigrationsServer::new(state.clone())),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    axum::Router::new().nest_service(MCP_ROUTE, service)
}

// ─── Tool input schemas ─────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetProjectArgs {
    /// Filesystem path of the EF migrations project (the `.csproj` or its
    /// containing directory).
    pub project_path: String,
    /// Name of the `DbContext` class. Empty string for EF's default.
    pub db_context: String,
    /// Path of the startup project that hosts the design-time DI. Empty
    /// string to let ezMigrations auto-detect from sibling `.csproj` files.
    pub startup_project: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SaveProjectArgs {
    /// Display name shown in the project switcher.
    pub name: String,
    /// Filesystem path of the migrations project.
    pub path: String,
    pub db_context: String,
    pub startup_project: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UpdateSavedProjectArgs {
    /// Saved-project id (millisecond timestamp string from `save_project`).
    pub id: String,
    pub name: String,
    pub path: String,
    pub db_context: String,
    pub startup_project: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ProjectIdArgs {
    pub id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct StableMigrationArgs {
    /// Migration name to pin, or `null` to clear the pin.
    pub migration_name: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetPreferencesArgs {
    pub preferences: PreferencesArg,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MigrationNameArgs {
    pub migration_name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddMigrationArgs {
    /// Migration name (e.g. `AddUsersTable`). EF prepends a timestamp.
    pub name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveMigrationArgs {
    /// Pass `--force` to EF (drops the migration's tables/columns if it was
    /// already applied).
    pub force: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UpdateDatabaseArgs {
    /// Empty string = update to latest. `"0"` = revert all. Otherwise a
    /// specific migration name.
    pub target: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SquashArgs {
    pub from_migration: String,
    pub to_migration: String,
    /// Name for the new squashed migration.
    pub new_name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GenerateScriptArgs {
    /// Start migration (empty string = from the beginning).
    pub from: String,
    /// End migration (empty string = to the latest).
    pub to: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SwitchBranchArgs {
    pub target_branch: String,
}

// ─── Server struct ─────────────────────────────────────────────────

#[derive(Clone)]
pub struct EzMigrationsServer {
    state: Arc<AppState>,
    tool_router: ToolRouter<EzMigrationsServer>,
}

#[tool_router]
impl EzMigrationsServer {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            tool_router: Self::tool_router(),
        }
    }

    // ─── Project / Active Config ────────────────────────────────────

    #[tool(
        description = "Register a project as the active migrations project and load its current branch. Replaces or adds it in the saved-projects list."
    )]
    async fn set_project(
        &self,
        Parameters(args): Parameters<SetProjectArgs>,
    ) -> Result<CallToolResult, McpError> {
        let SetProjectArgs {
            project_path,
            db_context,
            startup_project,
        } = args;

        ensure_path_exists(&project_path).map_err(tool_err)?;

        let pp = project_path.clone();
        let branch = tokio::task::spawn_blocking(move || {
            GitService::get_current_branch(&pp).unwrap_or_default()
        })
        .await
        .map_err(|e| tool_err(e.to_string()))?;

        let config = ProjectConfig {
            project_path: project_path.clone(),
            db_context: db_context.clone(),
            startup_project: startup_project.clone(),
        };

        let (project_id, stable_migration) = {
            let mut ac = self.state.app_config.lock().unwrap();
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
            (id, stable)
        };

        *self.state.config.lock().unwrap() = Some(config.clone());
        *self.state.current_branch.lock().unwrap() = branch.clone();
        reset_watchers(&self.state);

        json_result(&json!({
            "id": project_id,
            "path": config.project_path,
            "db_context": config.db_context,
            "startup_project": config.startup_project,
            "branch": branch,
            "stable_migration": stable_migration,
            "note": "Project saved in-memory; persisted to app_config.json by the GUI."
        }))
    }

    #[tool(
        description = "Return the currently active project (path, DbContext, startup, branch, stable-migration pin). Returns null when no project is loaded."
    )]
    async fn get_project(&self) -> Result<CallToolResult, McpError> {
        let payload = resources::read_project_current(&self.state).map_err(tool_err)?;
        Ok(CallToolResult::success(vec![Content::text(payload)]))
    }

    // ─── Saved Projects CRUD ────────────────────────────────────────

    #[tool(description = "List all saved projects.")]
    async fn get_saved_projects(&self) -> Result<CallToolResult, McpError> {
        let payload = resources::read_projects(&self.state).map_err(tool_err)?;
        Ok(CallToolResult::success(vec![Content::text(payload)]))
    }

    #[tool(
        description = "Save a new project entry without making it active. Use `switch_project` to activate it."
    )]
    async fn save_project(
        &self,
        Parameters(args): Parameters<SaveProjectArgs>,
    ) -> Result<CallToolResult, McpError> {
        ensure_path_exists(&args.path).map_err(tool_err)?;

        let saved = SavedProject {
            id: generate_id(),
            name: args.name,
            project_path: args.path,
            db_context: args.db_context,
            startup_project: args.startup_project,
            stable_migration: None,
        };

        {
            let mut ac = self.state.app_config.lock().unwrap();
            ac.projects.push(saved.clone());
        }

        json_result(&saved)
    }

    #[tool(description = "Update the metadata of a saved project by id.")]
    async fn update_saved_project(
        &self,
        Parameters(args): Parameters<UpdateSavedProjectArgs>,
    ) -> Result<CallToolResult, McpError> {
        ensure_path_exists(&args.path).map_err(tool_err)?;

        let updated = {
            let mut ac = self.state.app_config.lock().unwrap();
            let proj = ac
                .projects
                .iter_mut()
                .find(|p| p.id == args.id)
                .ok_or_else(|| tool_err(format!("Project not found: {}", args.id)))?;
            proj.name = args.name;
            proj.project_path = args.path;
            proj.db_context = args.db_context;
            proj.startup_project = args.startup_project;
            let updated = proj.clone();

            if ac.active_project_id.as_ref() == Some(&args.id) {
                *self.state.config.lock().unwrap() = Some(ProjectConfig {
                    project_path: updated.project_path.clone(),
                    db_context: updated.db_context.clone(),
                    startup_project: updated.startup_project.clone(),
                });
            }
            updated
        };

        json_result(&updated)
    }

    #[tool(
        description = "Delete a saved project by id. If it's the active project, the in-memory active config is cleared."
    )]
    async fn delete_saved_project(
        &self,
        Parameters(args): Parameters<ProjectIdArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut ac = self.state.app_config.lock().unwrap();
        ac.projects.retain(|p| p.id != args.id);
        if ac.active_project_id.as_ref() == Some(&args.id) {
            ac.active_project_id = None;
            *self.state.config.lock().unwrap() = None;
            *self.state.current_branch.lock().unwrap() = String::new();
            self.state.migrations.lock().unwrap().clear();
        }
        Ok(CallToolResult::success(vec![Content::text("ok")]))
    }

    #[tool(description = "Switch the active project to the saved entry with the given id.")]
    async fn switch_project(
        &self,
        Parameters(args): Parameters<ProjectIdArgs>,
    ) -> Result<CallToolResult, McpError> {
        let project = {
            let ac = self.state.app_config.lock().unwrap();
            ac.projects
                .iter()
                .find(|p| p.id == args.id)
                .ok_or_else(|| tool_err(format!("Project not found: {}", args.id)))?
                .clone()
        };

        ensure_path_exists(&project.project_path).map_err(tool_err)?;

        {
            let mut ac = self.state.app_config.lock().unwrap();
            ac.active_project_id = Some(args.id.clone());
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
        .map_err(|e| tool_err(e.to_string()))?;

        *self.state.config.lock().unwrap() = Some(config);
        *self.state.current_branch.lock().unwrap() = branch.clone();
        reset_watchers(&self.state);

        json_result(&json!({
            "id": project.id,
            "path": project.project_path,
            "db_context": project.db_context,
            "startup_project": project.startup_project,
            "branch": branch,
            "stable_migration": project.stable_migration,
        }))
    }

    #[tool(
        description = "Pin a migration as the stable rollback point for the active project. Pass `migration_name: null` to clear the pin."
    )]
    async fn set_stable_migration(
        &self,
        Parameters(args): Parameters<StableMigrationArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut ac = self.state.app_config.lock().unwrap();
        let active_id = ac
            .active_project_id
            .clone()
            .ok_or_else(|| tool_err("No active project"))?;
        let proj = ac
            .projects
            .iter_mut()
            .find(|p| p.id == active_id)
            .ok_or_else(|| tool_err("Active project not found"))?;
        proj.stable_migration = args.migration_name;
        Ok(CallToolResult::success(vec![Content::text("ok")]))
    }

    // ─── Preferences ────────────────────────────────────────────────

    #[tool(description = "Return current user preferences.")]
    async fn get_preferences(&self) -> Result<CallToolResult, McpError> {
        let prefs = self.state.app_config.lock().unwrap().preferences.clone();
        json_result(&prefs)
    }

    #[tool(description = "Replace user preferences.")]
    async fn set_preferences(
        &self,
        Parameters(args): Parameters<SetPreferencesArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.state.app_config.lock().unwrap().preferences = args.preferences.into();
        Ok(CallToolResult::success(vec![Content::text("ok")]))
    }

    // ─── Migration Ops ──────────────────────────────────────────────

    #[tool(
        description = "List EF migrations for the active project, marking each as `applied` or pending and noting whether it contains custom SQL."
    )]
    async fn list_migrations(&self) -> Result<CallToolResult, McpError> {
        let config = require_project(&self.state)?;
        let migrations = resources::list_migrations_inner(config)
            .await
            .map_err(|e| tool_err(enrich_ef_error(&e)))?;
        *self.state.migrations.lock().unwrap() = migrations.clone();
        json_result(&migrations)
    }

    #[tool(
        description = "Create a new EF migration. Mutating: runs `dotnet ef migrations add`. Serializes with other EF mutations via an internal mutex."
    )]
    async fn add_migration(
        &self,
        Parameters(args): Parameters<AddMigrationArgs>,
    ) -> Result<CallToolResult, McpError> {
        let _guard = self.state.op_mutex.lock().await;
        reset_op_cancel(&self.state);
        let config = require_project(&self.state)?;
        let name = args.name.clone();

        let result = tokio::task::spawn_blocking(move || {
            DotnetEf::add_migration(
                &config.project_path,
                &name,
                &config.db_context,
                &config.startup_project,
            )
        })
        .await
        .map_err(|e| tool_err(e.to_string()))?
        .map_err(|e| tool_err(enrich_ef_error(&e)))?;

        if result.success {
            Ok(CallToolResult::success(vec![Content::text(
                "Migration created successfully",
            )]))
        } else {
            Err(tool_err(enrich_ef_error(&format!(
                "Failed to create migration: {}",
                result.error_output()
            ))))
        }
    }

    #[tool(
        description = "Remove the last EF migration. Pass `force: true` to also drop the migration's schema if it was already applied. Mutating; serialized."
    )]
    async fn remove_migration(
        &self,
        Parameters(args): Parameters<RemoveMigrationArgs>,
    ) -> Result<CallToolResult, McpError> {
        let _guard = self.state.op_mutex.lock().await;
        reset_op_cancel(&self.state);
        let config = require_project(&self.state)?;

        let result = tokio::task::spawn_blocking(move || {
            DotnetEf::remove_migration(
                &config.project_path,
                &config.db_context,
                &config.startup_project,
                args.force,
            )
        })
        .await
        .map_err(|e| tool_err(e.to_string()))?
        .map_err(|e| tool_err(enrich_ef_error(&e)))?;

        if result.success {
            Ok(CallToolResult::success(vec![Content::text(
                "Last migration removed successfully",
            )]))
        } else {
            Err(tool_err(enrich_ef_error(&format!(
                "Failed to remove migration: {}",
                result.error_output()
            ))))
        }
    }

    #[tool(
        description = "Apply the database to a target migration. `target=\"\"` updates to latest; `target=\"0\"` reverts all; otherwise a specific migration name. Mutating; serialized."
    )]
    async fn update_database(
        &self,
        Parameters(args): Parameters<UpdateDatabaseArgs>,
    ) -> Result<CallToolResult, McpError> {
        let _guard = self.state.op_mutex.lock().await;
        reset_op_cancel(&self.state);
        let config = require_project(&self.state)?;
        let target = args.target.clone();

        let result = tokio::task::spawn_blocking(move || {
            DotnetEf::update_database(
                &config.project_path,
                &target,
                &config.db_context,
                &config.startup_project,
            )
        })
        .await
        .map_err(|e| tool_err(e.to_string()))?
        .map_err(|e| tool_err(enrich_ef_error(&e)))?;

        if result.success {
            let msg = if args.target.is_empty() {
                "Database updated to latest migration".to_string()
            } else {
                format!("Database updated to migration: {}", args.target)
            };
            Ok(CallToolResult::success(vec![Content::text(msg)]))
        } else {
            Err(tool_err(enrich_ef_error(&format!(
                "Failed to update database: {}",
                result.error_output()
            ))))
        }
    }

    #[tool(
        description = "Return parsed Up/Down bodies plus any extracted `migrationBuilder.Sql(...)` calls for a single migration. Read-only."
    )]
    async fn get_migration_sql(
        &self,
        Parameters(args): Parameters<MigrationNameArgs>,
    ) -> Result<CallToolResult, McpError> {
        let payload = resources::read_migration_sql(self.state.clone(), &args.migration_name)
            .await
            .map_err(tool_err)?;
        Ok(CallToolResult::success(vec![Content::text(payload)]))
    }

    #[tool(
        description = "Squash a contiguous range of migrations into a new one, preserving every `migrationBuilder.Sql(...)` call (newest version wins for Up, original for Down). Mutating; serialized. Reverts → removes → scaffolds → reapplies."
    )]
    async fn squash_migrations(
        &self,
        Parameters(args): Parameters<SquashArgs>,
    ) -> Result<CallToolResult, McpError> {
        let _guard = self.state.op_mutex.lock().await;
        reset_op_cancel(&self.state);
        let config = require_project(&self.state)?;

        // Use the most recent listing — refresh so we have file_path metadata
        // even if `list_migrations` hasn't been called recently.
        let migrations = resources::list_migrations_inner(config.clone())
            .await
            .map_err(|e| tool_err(enrich_ef_error(&e)))?;
        *self.state.migrations.lock().unwrap() = migrations.clone();

        let cancel = self.state.op_cancel.clone();
        let SquashArgs {
            from_migration,
            to_migration,
            new_name,
        } = args;

        let result = tokio::task::spawn_blocking(move || -> Result<String, String> {
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

            let before_migration = migrations
                .iter()
                .take_while(|m| m.name != from_migration)
                .last()
                .map(|m| m.name.clone())
                .unwrap_or_else(|| "0".to_string());

            check_cancel(&cancel)?;
            let revert = DotnetEf::update_database(
                &config.project_path,
                &before_migration,
                &config.db_context,
                &config.startup_project,
            )?;
            if !revert.success {
                return Err(format!(
                    "Failed to revert database for squash: {}",
                    revert.error_output()
                ));
            }

            for _name in migrations_to_remove.iter().rev() {
                check_cancel(&cancel)?;
                let removed = DotnetEf::remove_migration(
                    &config.project_path,
                    &config.db_context,
                    &config.startup_project,
                    true,
                )?;
                if !removed.success {
                    return Err(format!(
                        "Failed to remove migration during squash: {}",
                        removed.error_output()
                    ));
                }
            }

            check_cancel(&cancel)?;
            let added = DotnetEf::add_migration(
                &config.project_path,
                &new_name,
                &config.db_context,
                &config.startup_project,
            )?;
            if !added.success {
                return Err(format!(
                    "Failed to create squashed migration: {}",
                    added.error_output()
                ));
            }

            if let Some(new_file) =
                MigrationParser::get_migration_file(&config.project_path, &new_name)
            {
                if !all_custom_sql_up.is_empty() {
                    MigrationParser::inject_custom_sql(&new_file, "Up", &all_custom_sql_up)?;
                }
                if !all_custom_sql_down.is_empty() {
                    MigrationParser::inject_custom_sql(&new_file, "Down", &all_custom_sql_down)?;
                }
            }

            check_cancel(&cancel)?;
            let applied = DotnetEf::update_database(
                &config.project_path,
                "",
                &config.db_context,
                &config.startup_project,
            )?;
            if !applied.success {
                return Err(format!(
                    "Squash created but failed to apply: {}",
                    applied.error_output()
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
        .map_err(|e| tool_err(e.to_string()))?;

        reset_op_cancel(&self.state);
        match result {
            Ok(msg) => Ok(CallToolResult::success(vec![Content::text(msg)])),
            Err(e) => Err(tool_err(enrich_ef_error(&e))),
        }
    }

    #[tool(
        description = "Generate a SQL script between two migrations using `dotnet ef migrations script`. Read-only; returns the script text."
    )]
    async fn generate_script(
        &self,
        Parameters(args): Parameters<GenerateScriptArgs>,
    ) -> Result<CallToolResult, McpError> {
        let config = require_project(&self.state)?;

        let result = tokio::task::spawn_blocking(move || {
            DotnetEf::script_migration(
                &config.project_path,
                &args.from,
                &args.to,
                &config.db_context,
                &config.startup_project,
            )
        })
        .await
        .map_err(|e| tool_err(e.to_string()))?
        .map_err(|e| tool_err(enrich_ef_error(&e)))?;

        if result.success {
            Ok(CallToolResult::success(vec![Content::text(result.stdout)]))
        } else {
            Err(tool_err(format!(
                "Failed to generate script: {}",
                result.error_output()
            )))
        }
    }

    // ─── Process Control ────────────────────────────────────────────

    #[tool(
        description = "Cancel any currently running EF operation (kills the `dotnet ef` child if one is running and signals multi-step orchestration to bail at the next phase)."
    )]
    async fn cancel_running_operation(&self) -> Result<CallToolResult, McpError> {
        self.state
            .op_cancel
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let killed = tokio::task::spawn_blocking(DotnetEf::cancel_running_operation)
            .await
            .map_err(|e| tool_err(e.to_string()))?
            .map_err(tool_err)?;
        let msg = match killed {
            Some(op) => format!("Cancel requested for '{}'", op),
            None => "Cancel requested.".to_string(),
        };
        Ok(CallToolResult::success(vec![Content::text(msg)]))
    }

    // ─── Git / Branch ───────────────────────────────────────────────

    #[tool(description = "Return the current git branch of the active project.")]
    async fn get_current_branch(&self) -> Result<CallToolResult, McpError> {
        let branch = resources::read_current_branch(self.state.clone())
            .await
            .map_err(tool_err)?;
        Ok(CallToolResult::success(vec![Content::text(branch)]))
    }

    #[tool(
        description = "List local and remote git branches in the active project's repo (excluding the current branch)."
    )]
    async fn list_git_branches(&self) -> Result<CallToolResult, McpError> {
        let payload = resources::read_branches(self.state.clone())
            .await
            .map_err(tool_err)?;
        Ok(CallToolResult::success(vec![Content::text(payload)]))
    }

    #[tool(
        description = "Managed branch switch: roll the database back to the latest migration that exists on both branches, `git checkout` the target, then apply the new branch's migrations. Refuses to run on a dirty working tree. Mutating; serialized."
    )]
    async fn switch_branch_with_migrations(
        &self,
        Parameters(args): Parameters<SwitchBranchArgs>,
    ) -> Result<CallToolResult, McpError> {
        let _guard = self.state.op_mutex.lock().await;
        reset_op_cancel(&self.state);
        let config = require_project(&self.state)?;
        let project_path_for_error = config.project_path.clone();
        let cancel = self.state.op_cancel.clone();
        let target_branch = args.target_branch;

        let result = tokio::task::spawn_blocking(move || -> Result<_, String> {
            let target_branch = target_branch.trim().to_string();
            if target_branch.is_empty() {
                return Err("Choose a branch to switch to".to_string());
            }

            let old_branch = GitService::get_current_branch(&config.project_path)?;
            if old_branch == target_branch {
                return Ok(json!({
                    "old_branch": old_branch.clone(),
                    "new_branch": old_branch,
                    "common_migration": null,
                    "rollback_target": null,
                    "rollback_performed": false,
                    "target_migration_count": 0,
                }));
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

            let target_files =
                GitService::list_files_at_ref(&repo_root, &target_branch, &migrations_pathspec)?;
            let target_migrations: Vec<String> = target_files
                .iter()
                .filter_map(|p| migration_name_from_path(Path::new(p)))
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
            let ef_migrations = DotnetEf::list_migrations(
                &config.project_path,
                &config.db_context,
                &config.startup_project,
            )?;
            let ef_migration_names: Vec<String> =
                ef_migrations.iter().map(|(name, _)| name.clone()).collect();
            let current_migrations = if ef_migration_names.is_empty() {
                current_files
            } else {
                ef_migration_names
            };

            let common_migration =
                latest_common_migration(&current_migrations, &target_migrations);
            let common_index = common_migration
                .as_ref()
                .and_then(|name| current_migrations.iter().position(|m| m == name));
            let latest_applied_index =
                ef_migrations.iter().rposition(|(_, applied)| *applied);

            if common_migration.is_none() && latest_applied_index.is_some() {
                return Err(format!(
                    "No common migration found between '{}' and '{}'. Refusing to revert all applied migrations automatically.",
                    old_branch, target_branch
                ));
            }

            let rollback_target = match (latest_applied_index, common_index) {
                (Some(applied), Some(common)) if applied > common => common_migration.clone(),
                _ => None,
            };

            let mut rollback_performed = false;
            if let Some(ref target) = rollback_target {
                check_cancel(&cancel)?;
                let rollback = DotnetEf::update_database(
                    &config.project_path,
                    target,
                    &config.db_context,
                    &config.startup_project,
                )?;
                if !rollback.success {
                    return Err(format!(
                        "Failed to roll back before switching branches: {}",
                        rollback.error_output()
                    ));
                }
                rollback_performed = true;
            }

            check_cancel(&cancel)?;
            GitService::switch_branch(&config.project_path, &target_branch)?;
            let new_branch = GitService::get_current_branch(&config.project_path)
                .unwrap_or_else(|_| target_branch.clone());

            check_cancel(&cancel)?;
            let update = DotnetEf::update_database(
                &config.project_path,
                "",
                &config.db_context,
                &config.startup_project,
            )?;
            if !update.success {
                return Err(format!(
                    "Switched to '{}', but failed to update the database: {}",
                    new_branch,
                    update.error_output()
                ));
            }

            Ok(json!({
                "old_branch": old_branch,
                "new_branch": new_branch,
                "common_migration": common_migration,
                "rollback_target": rollback_target,
                "rollback_performed": rollback_performed,
                "target_migration_count": target_migrations.len(),
            }))
        })
        .await
        .map_err(|e| tool_err(e.to_string()))?;

        reset_op_cancel(&self.state);
        match result {
            Ok(payload) => {
                if let Some(new_branch) =
                    payload.get("new_branch").and_then(|v| v.as_str())
                {
                    *self.state.current_branch.lock().unwrap() = new_branch.to_string();
                    self.state.migrations.lock().unwrap().clear();
                }
                json_result(&payload)
            }
            Err(err) => {
                if let Ok(branch) = GitService::get_current_branch(&project_path_for_error) {
                    *self.state.current_branch.lock().unwrap() = branch;
                }
                Err(tool_err(enrich_ef_error(&err)))
            }
        }
    }

    // ─── File Watchers ──────────────────────────────────────────────

    #[tool(
        description = "Start the .git/HEAD watcher for the active project. Idempotent. Watcher events are delivered to the desktop window, not to MCP clients."
    )]
    async fn start_branch_watcher(&self) -> Result<CallToolResult, McpError> {
        let _config = require_project(&self.state)?;
        Ok(CallToolResult::success(vec![Content::text(
            "Branch watcher is owned by the desktop window. Open the ezMigrations app to receive `branch-changed` events; MCP clients don't observe them yet.",
        )]))
    }

    #[tool(
        description = "Start the migrations-folder watcher for the active project. Idempotent. Watcher events are delivered to the desktop window only."
    )]
    async fn start_migration_watcher(&self) -> Result<CallToolResult, McpError> {
        let _config = require_project(&self.state)?;
        Ok(CallToolResult::success(vec![Content::text(
            "Migration watcher is owned by the desktop window. Open the ezMigrations app to receive `migrations-changed` events; MCP clients don't observe them yet.",
        )]))
    }
}

// ─── ServerHandler ──────────────────────────────────────────────────

#[tool_handler]
impl ServerHandler for EzMigrationsServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_server_info(Implementation::from_build_env())
        .with_protocol_version(ProtocolVersion::V_2024_11_05)
        .with_instructions(INSTRUCTIONS.to_string())
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let resources = vec![
            RawResource::new(uri::PROJECT_CURRENT, "Active project")
                .no_annotation(),
            RawResource::new(uri::PROJECTS, "Saved projects").no_annotation(),
            RawResource::new(uri::PREFERENCES, "Preferences").no_annotation(),
            RawResource::new(uri::MIGRATIONS, "Migrations").no_annotation(),
            RawResource::new(uri::BRANCHES, "Branches").no_annotation(),
            RawResource::new(uri::BRANCHES_CURRENT, "Current branch")
                .no_annotation(),
            RawResource::new(uri::APP_STATUS, "App status").no_annotation(),
        ];
        Ok(ListResourcesResult {
            resources,
            next_cursor: None,
            meta: None,
        })
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        let template =
            RawResourceTemplate::new(uri::MIGRATION_SQL_TEMPLATE, "Migration SQL")
                .with_description(
                    "Parsed Up/Down bodies and extracted migrationBuilder.Sql calls for one migration."
                        .to_string(),
                )
                .with_mime_type("application/json".to_string())
                .no_annotation();
        Ok(ListResourceTemplatesResult {
            resource_templates: vec![template],
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let uri_str = request.uri;

        let result: Result<String, String> = match uri_str.as_str() {
            uri::PROJECT_CURRENT => resources::read_project_current(&self.state),
            uri::PROJECTS => resources::read_projects(&self.state),
            uri::PREFERENCES => resources::read_preferences(&self.state),
            uri::MIGRATIONS => resources::read_migrations(self.state.clone()).await,
            uri::BRANCHES => resources::read_branches(self.state.clone()).await,
            uri::BRANCHES_CURRENT => resources::read_current_branch(self.state.clone()).await,
            uri::APP_STATUS => resources::read_app_status(&self.state),
            other => {
                if let Some(name) = resources::parse_migration_sql_uri(other) {
                    resources::read_migration_sql(self.state.clone(), name).await
                } else {
                    return Err(McpError::resource_not_found(
                        "resource_not_found",
                        Some(json!({ "uri": other })),
                    ));
                }
            }
        };

        let body = result.map_err(|e| {
            McpError::internal_error(
                e,
                Some(json!({ "uri": uri_str })),
            )
        })?;

        Ok(ReadResourceResult::new(vec![ResourceContents::text(
            body, uri_str,
        )]))
    }
}

// ─── Helpers ────────────────────────────────────────────────────────

fn tool_err(msg: impl Into<String>) -> McpError {
    McpError::internal_error(msg.into(), None)
}

fn json_result<T: serde::Serialize>(value: &T) -> Result<CallToolResult, McpError> {
    let text = serde_json::to_string_pretty(value).map_err(|e| tool_err(e.to_string()))?;
    Ok(CallToolResult::success(vec![Content::text(text)]))
}

fn require_project(state: &AppState) -> Result<ProjectConfig, McpError> {
    resources::require_project(state).map_err(tool_err)
}

fn reset_op_cancel(state: &AppState) {
    state
        .op_cancel
        .store(false, std::sync::atomic::Ordering::SeqCst);
}

fn check_cancel(cancel: &std::sync::Arc<std::sync::atomic::AtomicBool>) -> Result<(), String> {
    if cancel.load(std::sync::atomic::Ordering::SeqCst) {
        Err("Canceled by user.".to_string())
    } else {
        Ok(())
    }
}

fn ensure_path_exists(path: &str) -> Result<(), String> {
    if Path::new(path).exists() {
        Ok(())
    } else {
        Err(format!("Path does not exist: {}", path))
    }
}

fn derive_project_name(project_path: &str) -> String {
    Path::new(project_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "My Project".to_string())
}

fn generate_id() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}

fn migration_name_from_path(path: &Path) -> Option<String> {
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

fn latest_common_migration(current: &[String], target: &[String]) -> Option<String> {
    let target_names: std::collections::HashSet<&str> =
        target.iter().map(String::as_str).collect();
    current
        .iter()
        .rev()
        .find(|name| target_names.contains(name.as_str()))
        .cloned()
}

fn path_relative_to_repo(repo_root: &str, path: &Path) -> Result<String, String> {
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

fn enrich_ef_error(raw: &str) -> String {
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

