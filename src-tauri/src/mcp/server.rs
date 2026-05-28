//! `EzMigrationsServer` — the MCP server backing ezMigrations.
//!
//! Mounted on Axum by [`build_axum_router`], this exposes every Tauri command
//! in the desktop app as an MCP tool plus a handful of read-only resources for
//! state inspection. State is shared with the GUI via `Arc<AppState>`, and
//! every tool delegates to [`crate::ops`] — the same orchestration the Tauri
//! commands run — so there is exactly one implementation of each workflow.
//!
//! Tools translate MCP's request/response shapes and nothing more: parse the
//! argument struct, call `ops::*`, wrap the result. Progress goes to
//! [`NoopPhaseSink`] (MCP has no event channel); persistence goes through the
//! injected [`ConfigStore`]. EF-mutating ops serialize against the GUI via
//! `state.op_mutex`, which `ops` acquires internally.

use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    AnnotateAble, CallToolResult, Content, Implementation, ListResourceTemplatesResult,
    ListResourcesResult, PaginatedRequestParams, ProtocolVersion, RawResource, RawResourceTemplate,
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

use crate::ops::{self, ConfigStore, NoopPhaseSink};
use crate::mcp::resources::{self, uri};
use crate::state::{AppState, Preferences};

/// Agent-facing debrief surfaced via `ServerCapabilities::instructions`. Single
/// source of truth in `docs/agent-debrief.md` so it can also be pasted into an
/// agent's system prompt before the MCP connection exists.
const INSTRUCTIONS: &str = include_str!("../../../docs/agent-debrief.md");

/// Path the MCP service is nested at; the port file advertises
/// `http://127.0.0.1:<port>/mcp` as its `url`.
pub const MCP_ROUTE: &str = "/mcp";

/// Build the Axum router hosting the MCP service. The caller binds the returned
/// router to a loopback port and supplies the shared state plus the persistence
/// backend for saved-project mutations.
pub fn build_axum_router(state: Arc<AppState>, store: Arc<dyn ConfigStore>) -> axum::Router {
    let service = StreamableHttpService::new(
        move || Ok(EzMigrationsServer::new(state.clone(), store.clone())),
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
    pub preferences: Preferences,
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
    store: Arc<dyn ConfigStore>,
    tool_router: ToolRouter<EzMigrationsServer>,
}

#[tool_router]
impl EzMigrationsServer {
    pub fn new(state: Arc<AppState>, store: Arc<dyn ConfigStore>) -> Self {
        Self {
            state,
            store,
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
        let info = ops::set_project(
            &self.state,
            self.store.as_ref(),
            args.project_path,
            args.db_context,
            args.startup_project,
        )
        .await
        .map_err(tool_err)?;
        json_result(&info)
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
        let saved = ops::save_project(
            &self.state,
            self.store.as_ref(),
            args.name,
            args.path,
            args.db_context,
            args.startup_project,
        )
        .map_err(tool_err)?;
        json_result(&saved)
    }

    #[tool(description = "Update the metadata of a saved project by id.")]
    async fn update_saved_project(
        &self,
        Parameters(args): Parameters<UpdateSavedProjectArgs>,
    ) -> Result<CallToolResult, McpError> {
        let updated = ops::update_saved_project(
            &self.state,
            self.store.as_ref(),
            args.id,
            args.name,
            args.path,
            args.db_context,
            args.startup_project,
        )
        .map_err(tool_err)?;
        json_result(&updated)
    }

    #[tool(
        description = "Delete a saved project by id. If it's the active project, the in-memory active config is cleared."
    )]
    async fn delete_saved_project(
        &self,
        Parameters(args): Parameters<ProjectIdArgs>,
    ) -> Result<CallToolResult, McpError> {
        ops::delete_saved_project(&self.state, self.store.as_ref(), args.id).map_err(tool_err)?;
        Ok(CallToolResult::success(vec![Content::text("ok")]))
    }

    #[tool(description = "Switch the active project to the saved entry with the given id.")]
    async fn switch_project(
        &self,
        Parameters(args): Parameters<ProjectIdArgs>,
    ) -> Result<CallToolResult, McpError> {
        let info = ops::switch_project(&self.state, self.store.as_ref(), args.id)
            .await
            .map_err(tool_err)?;
        json_result(&info)
    }

    #[tool(
        description = "Pin a migration as the stable rollback point for the active project. Pass `migration_name: null` to clear the pin."
    )]
    async fn set_stable_migration(
        &self,
        Parameters(args): Parameters<StableMigrationArgs>,
    ) -> Result<CallToolResult, McpError> {
        ops::set_stable_migration(&self.state, self.store.as_ref(), args.migration_name)
            .map_err(tool_err)?;
        Ok(CallToolResult::success(vec![Content::text("ok")]))
    }

    // ─── Preferences ────────────────────────────────────────────────

    #[tool(description = "Return current user preferences.")]
    async fn get_preferences(&self) -> Result<CallToolResult, McpError> {
        let payload = resources::read_preferences(&self.state).map_err(tool_err)?;
        Ok(CallToolResult::success(vec![Content::text(payload)]))
    }

    #[tool(description = "Replace user preferences.")]
    async fn set_preferences(
        &self,
        Parameters(args): Parameters<SetPreferencesArgs>,
    ) -> Result<CallToolResult, McpError> {
        ops::set_preferences(&self.state, self.store.as_ref(), args.preferences)
            .map_err(tool_err)?;
        Ok(CallToolResult::success(vec![Content::text("ok")]))
    }

    // ─── Migration Ops ──────────────────────────────────────────────

    #[tool(
        description = "List EF migrations for the active project, marking each as `applied` or pending and noting whether it contains custom SQL."
    )]
    async fn list_migrations(&self) -> Result<CallToolResult, McpError> {
        let migrations = ops::list_migrations(&self.state).await.map_err(tool_err)?;
        json_result(&migrations)
    }

    #[tool(
        description = "Create a new EF migration. Mutating: runs `dotnet ef migrations add`. Serializes with other EF mutations via an internal mutex."
    )]
    async fn add_migration(
        &self,
        Parameters(args): Parameters<AddMigrationArgs>,
    ) -> Result<CallToolResult, McpError> {
        let msg = ops::add_migration(&self.state, args.name, NoopPhaseSink)
            .await
            .map_err(tool_err)?;
        Ok(CallToolResult::success(vec![Content::text(msg)]))
    }

    #[tool(
        description = "Remove the last EF migration. Pass `force: true` to also drop the migration's schema if it was already applied. Mutating; serialized."
    )]
    async fn remove_migration(
        &self,
        Parameters(args): Parameters<RemoveMigrationArgs>,
    ) -> Result<CallToolResult, McpError> {
        let msg = ops::remove_migration(&self.state, args.force, NoopPhaseSink)
            .await
            .map_err(tool_err)?;
        Ok(CallToolResult::success(vec![Content::text(msg)]))
    }

    #[tool(
        description = "Apply the database to a target migration. `target=\"\"` updates to latest; `target=\"0\"` reverts all; otherwise a specific migration name. Mutating; serialized."
    )]
    async fn update_database(
        &self,
        Parameters(args): Parameters<UpdateDatabaseArgs>,
    ) -> Result<CallToolResult, McpError> {
        let msg = ops::update_database(&self.state, args.target, NoopPhaseSink)
            .await
            .map_err(tool_err)?;
        Ok(CallToolResult::success(vec![Content::text(msg)]))
    }

    #[tool(
        description = "Return parsed Up/Down bodies plus any extracted `migrationBuilder.Sql(...)` calls for a single migration. Read-only."
    )]
    async fn get_migration_sql(
        &self,
        Parameters(args): Parameters<MigrationNameArgs>,
    ) -> Result<CallToolResult, McpError> {
        let info = ops::get_migration_sql(&self.state, args.migration_name)
            .await
            .map_err(tool_err)?;
        json_result(&info)
    }

    #[tool(
        description = "Squash a contiguous range of migrations into a new one, preserving every `migrationBuilder.Sql(...)` call (newest version wins for Up, original for Down). Mutating; serialized. Reverts → removes → scaffolds → reapplies."
    )]
    async fn squash_migrations(
        &self,
        Parameters(args): Parameters<SquashArgs>,
    ) -> Result<CallToolResult, McpError> {
        let msg = ops::squash_migrations(
            &self.state,
            args.from_migration,
            args.to_migration,
            args.new_name,
            NoopPhaseSink,
        )
        .await
        .map_err(tool_err)?;
        Ok(CallToolResult::success(vec![Content::text(msg)]))
    }

    #[tool(
        description = "Generate a SQL script between two migrations using `dotnet ef migrations script`. Read-only; returns the script text."
    )]
    async fn generate_script(
        &self,
        Parameters(args): Parameters<GenerateScriptArgs>,
    ) -> Result<CallToolResult, McpError> {
        let script = ops::generate_script(&self.state, args.from, args.to)
            .await
            .map_err(tool_err)?;
        Ok(CallToolResult::success(vec![Content::text(script)]))
    }

    // ─── Process Control ────────────────────────────────────────────

    #[tool(
        description = "Cancel any currently running EF operation (kills the `dotnet ef` child if one is running and signals multi-step orchestration to bail at the next phase)."
    )]
    async fn cancel_running_operation(&self) -> Result<CallToolResult, McpError> {
        let msg = ops::cancel_running_operation(&self.state)
            .await
            .map_err(tool_err)?;
        Ok(CallToolResult::success(vec![Content::text(msg)]))
    }

    // ─── Git / Branch ───────────────────────────────────────────────

    #[tool(description = "Return the current git branch of the active project.")]
    async fn get_current_branch(&self) -> Result<CallToolResult, McpError> {
        let branch = ops::get_current_branch(&self.state).await.map_err(tool_err)?;
        Ok(CallToolResult::success(vec![Content::text(branch)]))
    }

    #[tool(
        description = "List local and remote git branches in the active project's repo (excluding the current branch)."
    )]
    async fn list_git_branches(&self) -> Result<CallToolResult, McpError> {
        let branches = ops::list_git_branches(&self.state).await.map_err(tool_err)?;
        json_result(&branches)
    }

    #[tool(
        description = "Managed branch switch: roll the database back to the latest migration that exists on both branches, `git checkout` the target, then apply the new branch's migrations. Refuses to run on a dirty working tree. Mutating; serialized."
    )]
    async fn switch_branch_with_migrations(
        &self,
        Parameters(args): Parameters<SwitchBranchArgs>,
    ) -> Result<CallToolResult, McpError> {
        let result =
            ops::switch_branch_with_migrations(&self.state, args.target_branch, NoopPhaseSink)
                .await
                .map_err(tool_err)?;
        json_result(&result)
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
            RawResource::new(uri::PROJECT_CURRENT, "Active project").no_annotation(),
            RawResource::new(uri::PROJECTS, "Saved projects").no_annotation(),
            RawResource::new(uri::PREFERENCES, "Preferences").no_annotation(),
            RawResource::new(uri::MIGRATIONS, "Migrations").no_annotation(),
            RawResource::new(uri::BRANCHES, "Branches").no_annotation(),
            RawResource::new(uri::BRANCHES_CURRENT, "Current branch").no_annotation(),
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
        let template = RawResourceTemplate::new(uri::MIGRATION_SQL_TEMPLATE, "Migration SQL")
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

        let body = result.map_err(|e| McpError::internal_error(e, Some(json!({ "uri": uri_str }))))?;

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

fn require_project(state: &AppState) -> Result<crate::state::ProjectConfig, McpError> {
    ops::require_project(state).map_err(tool_err)
}
