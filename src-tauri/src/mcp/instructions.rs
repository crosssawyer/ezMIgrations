//! Agent-facing context returned in the MCP initialize response.
//!
//! The callable MCP surface is declared in `server.rs` with `#[tool]`
//! handlers and in `resources.rs` with resource handlers. This text is only
//! orientation for clients after they connect; it is not a substitute for
//! tool schemas or resource discovery.

pub(super) const INSTRUCTIONS: &str = r#"ezMigrations exposes the same EF Core migration workflows as the desktop UI.

Use MCP tool discovery and resource discovery as the source of truth for
available actions, argument schemas, and read-only state. The tool descriptions
describe each operation; resources under the `ezmigrations://` scheme provide
JSON snapshots when inspection is enough.

Important operating rules:
- An active project is required for migration, branch, script, and watcher
  operations. Call `get_project` or read `ezmigrations://project/current`
  first; if no project is active, use `set_project` or `switch_project`.
- EF-mutating operations are single-flight. `add_migration`,
  `remove_migration`, `update_database`, `squash_migrations`, and
  `switch_branch_with_migrations` serialize through one in-process mutex shared
  with the desktop app.
- `cancel_running_operation` asks the current EF child process to stop and
  sets a cooperative cancellation flag checked between multi-step phases.
- GUI-hosted MCP shares the desktop app state and persists saved-project and
  preference mutations through the app config store. Headless MCP persists its
  own config through a file-backed store in the platform data directory.
- Watcher tools are best-effort. In GUI mode they affect the desktop window's
  event flow; MCP clients do not currently receive watcher callbacks.

Confirm with the user before destructive or production-adjacent operations:
- `update_database` applies or reverts migrations against the configured
  connection string.
- `remove_migration` with `force = true` can drop database objects when the
  migration was already applied.
- `squash_migrations` reverts, removes, scaffolds, and reapplies migrations in
  the requested range.
- `switch_branch_with_migrations` runs git checkout plus database rollback and
  update logic, and refuses dirty working trees.
- `delete_saved_project` removes saved metadata only; it does not delete files.
"#;
