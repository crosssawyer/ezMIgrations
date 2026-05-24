<!--
Agent Debrief — ezMigrations

This file is the same content the MCP server serves to clients via
`ServerCapabilities::instructions` on connect. It's mirrored here so you can
paste it into an agent's system prompt before the agent has discovered or
connected to the MCP server — handy when bootstrapping a session.

Copy the markdown below this comment block into your agent's context.
-->

ezMigrations MCP server
=======================

ezMigrations is a Tauri v2 desktop app that wraps `dotnet ef` to manage
Entity Framework Core migrations for git-versioned .NET projects. It is
branch-aware (rolling the database back to the latest common migration when
you switch branches), parses migration `.cs` files so it can preserve every
`migrationBuilder.Sql(...)` call across squashes, and watches both the
project's git HEAD and migrations folder so the UI stays in sync.

This MCP endpoint exposes the same surface the desktop UI consumes: all 23
backend commands as tools, plus 8 read-only resources for inspecting current
state without mutating anything.

Discovering & Starting ezMigrations
-----------------------------------
If you are reading this, you are already connected to a running ezMigrations
MCP server. Two scenarios exist on a fresh machine:

1. **ezMigrations GUI is running.** The GUI starts the MCP server automatically.
   Look for the port file at the user's platform app-data directory under
   `ezmigrations/mcp-port.json` (Windows: `%APPDATA%`, macOS:
   `~/Library/Application Support`, Linux: `$XDG_DATA_HOME` or
   `~/.local/share`). The file contains the JSON keys `port`, `pid`,
   `started_at_unix_ms`, `transport`, and `url`. Use the `url` field to
   connect.

2. **GUI is not running.** Two options:
   a. Ask the user to launch the GUI.
   b. Start the headless server yourself by invoking the `ezmigrations-mcp`
      binary (no arguments). It writes the same port file but does not
      launch a window. The headless server refuses to start if the GUI is
      already running on the same machine — it checks the existing port
      file's PID for liveness and aborts on a hit. It runs until it receives
      ctrl-c or its parent process exits.

The `ezmigrations-mcp` binary is built from the same `src-tauri` crate as
the desktop app; it lives next to the GUI binary in
`src-tauri/target/release/` after a build. It is not yet bundled into the
platform installers, so on a fresh end-user machine the GUI is the only
launch path until you build the headless binary explicitly.

If neither the GUI nor the `ezmigrations-mcp` binary is reachable on the
host, ezMigrations is not available; ask the user to install or build it.

The headless server is functionally identical to the GUI's server for the
MCP surface — the same 23 tools, same 8 resources, same `op_mutex`
serialization. The only difference is that the headless server has no
Tauri event bus, so file watcher events (`branch-changed`,
`migrations-changed`) are not emitted to any client in this mode. The
watcher tools (`start_branch_watcher`, `start_migration_watcher`) still
return an informational message.

Audience
--------
.NET developers driving EF Core migrations in projects that live in git. The
agent's job is usually to inspect state (`list_migrations`, `list_git_branches`),
help author or roll back migrations, or run a managed branch switch.

Capabilities
------------
The same capabilities are available whether you are talking to the GUI's
in-process server or the `ezmigrations-mcp` headless binary — the underlying
tools, resources, and state machine are identical.

- Inspect: current project, migration list with applied/pending status,
  parsed Up/Down bodies and extracted SQL, current branch, all git branches,
  saved-project list, preferences.
- Project mgmt: pick a saved project, register a new project path /
  DbContext / startup project, delete a saved project, pin a "stable
  migration" used as the safe rollback target.
- Migration ops: add a migration, remove the last one (with optional
  `--force`), apply the database forward to a target (`update_database`
  with `target=""` for latest or `"0"` to revert all), generate a SQL
  script between two migrations, get the parsed SQL for one migration.
- Squash with custom SQL preservation: extracts every `migrationBuilder.Sql`
  call in the squash range, dedupes (newest wins for Up, oldest wins for
  Down), then re-injects after EF scaffolds the new squashed migration.
- Managed branch switch: lists migrations on the target branch via
  `git ls-tree`, computes the latest common migration, rolls the DB back
  to it, checks out the branch, applies the new branch's migrations.
- File watchers: start the .git/HEAD watcher and the Migrations/ watcher.
  In the GUI-hosted server, watcher events fire on the desktop window's
  IPC; MCP clients don't observe them yet. In the headless server, no
  event bus is attached, so the watchers report status only.

Persistence
-----------
MCP tools that mutate configuration — `set_project`, `save_project`,
`update_saved_project`, `delete_saved_project`, `switch_project`,
`set_stable_migration`, and `set_preferences` — update the running app's
**in-memory state only**. The MCP server has no access to `AppHandle::path()`
so it cannot flush `app_config.json` itself. When the GUI is the host, it
writes the full config on its next mutation, so any in-memory change is
persisted as soon as the user (or a subsequent GUI-driven action) interacts
with the app. **If the app is quit before any GUI mutation occurs,
in-memory-only changes will be lost.** Under the headless `ezmigrations-mcp`
binary there is no GUI to flush the config at all — every session starts
with an empty saved-project list and ends without writing anything back.
To guarantee persistence without a GUI action, ask the user to perform any
config-touching operation in the desktop window (e.g., toggling a preference).

State assumptions
-----------------
- **Active project required.** `list_migrations`, `add_migration`,
  `update_database`, `squash_migrations`, `switch_branch_with_migrations`,
  `get_current_branch`, `list_git_branches`, `start_branch_watcher`,
  `start_migration_watcher`, `get_migration_sql`, `generate_script`,
  and `remove_migration` all require an active project. Call `get_project`
  first; if it returns `null`, either call `set_project` with a path or
  pick one from `get_saved_projects` and call `switch_project`.
- **Single-flight EF mutations.** `add_migration`, `remove_migration`,
  `update_database`, `squash_migrations`, and
  `switch_branch_with_migrations` serialize through an in-process mutex.
  Concurrent calls (including from the desktop GUI) wait their turn.
- **Cancellation.** `cancel_running_operation` aborts the currently
  running `dotnet ef` child and sets a flag that multi-step orchestration
  (squash, branch switch) polls between phases. Safe to call any time.
- **Watchers are best-effort.** `start_branch_watcher` and
  `start_migration_watcher` need an active project; they're idempotent.
  Under the GUI-hosted server they emit Tauri events to the desktop window
  only; under the headless `ezmigrations-mcp` server there is no event bus
  so they report status without delivering callbacks.

Destructive operations
----------------------
Confirm with the user before invoking on production-adjacent projects:
- `update_database` — runs `dotnet ef database update`, applies or
  reverts migrations against the configured connection string.
- `remove_migration` with `force = true` — `dotnet ef migrations remove
  --force` will drop the corresponding tables/columns if the migration
  has been applied.
- `squash_migrations` — reverts the database to before the squash range,
  removes every migration in the range, scaffolds a new squashed
  migration, then reapplies. Writes to the migrations folder.
- `switch_branch_with_migrations` — `git checkout` plus a database
  update. Refuses to run with a dirty working tree.
- `delete_saved_project` — only deletes the saved metadata, not the
  on-disk project.

Read-only safe tools
--------------------
`get_project`, `get_saved_projects`, `get_preferences`,
`list_migrations`, `get_migration_sql`, `generate_script`,
`get_current_branch`, `list_git_branches`. These never invoke `dotnet ef`
in mutating mode and never touch git working state.

Resources
---------
Eight read-only URIs under the `ezmigrations://` scheme give you a JSON
snapshot of state without firing a tool:
`project/current`, `projects`, `preferences`, `migrations`,
`migrations/{name}/sql` (templated), `branches`, `branches/current`,
`app/status`. Prefer resources over tools when you only need a peek.
