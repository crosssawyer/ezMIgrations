# MCP Server Integration

ezMigrations runs an MCP (Model Context Protocol) server on a loopback HTTP
port while the desktop app is open, letting AI agents drive the same
workflows the GUI exposes. The server shares state with the GUI, so an
MCP-triggered `add_migration` lands in the same in-memory cache the GUI
reads — open the app and you see the new migration appear in the table.
A standalone `ezmigrations-mcp` binary runs the same server without the
GUI for headless contexts; see [Headless mode](#headless-mode).

## How it works

- The server starts automatically when ezMigrations launches and stops with
  the app.
- Transport: Streamable HTTP (the `2025-03-26` MCP transport).
- Bind: `127.0.0.1:<random-port>`. Loopback-only; never exposed on the LAN.
- Route: the MCP service is mounted at `/mcp`. The full base URL is
  `http://127.0.0.1:<port>/mcp`.
- A port file at the platform's app-data directory advertises the live port
  so clients can find it without a config dance.

### Port file

The exact path follows `dirs::data_dir()` for the running user, joined
with `ezmigrations/mcp-port.json`:

| Platform | Path |
| --- | --- |
| Windows | `%APPDATA%\ezmigrations\mcp-port.json` |
| macOS   | `~/Library/Application Support/ezmigrations/mcp-port.json` |
| Linux   | `$XDG_DATA_HOME/ezmigrations/mcp-port.json` (or `~/.local/share/ezmigrations/mcp-port.json`) |

If `dirs::data_dir()` returns `None` (rare — CI sandboxes, broken
`$HOME`), the file falls back to the OS temp directory.

Contents:

```json
{
  "port": 51843,
  "pid": 12345,
  "started_at_unix_ms": 1763908131000,
  "transport": "http",
  "url": "http://127.0.0.1:51843/mcp"
}
```

The file is rewritten on every app launch.

## Headless mode

A standalone `ezmigrations-mcp` binary builds from the same `src-tauri`
crate as the GUI. Invoking it starts the same MCP server without launching
the desktop window. Useful for CI runs, automated agents, or machines where
the GUI isn't running.

The headless server uses the same port file as the GUI and refuses to start
if the GUI is already running on the same machine — it reads the existing
port file's `pid` field and checks process liveness before binding. Stale
files from crashed sessions are cleared automatically on the next start.

Example launch:

```text
$ ezmigrations-mcp
ezmigrations-mcp listening on http://127.0.0.1:51234/mcp (headless)
```

The binary blocks until ctrl-c or its parent process exits. No flags are
required for typical use.

Behavioural differences from the GUI-hosted server:

- **Empty starting state.** The headless process can't read
  `app_config.json` (it has no `AppHandle::path()`), so the saved-project
  list starts empty. Use `set_project` to point at a project for the
  session, or have the user run the GUI once to seed the config.
- **No event bus.** The Tauri IPC channel doesn't exist, so
  `start_branch_watcher` and `start_migration_watcher` report status
  without delivering events to MCP clients.
- **No GUI flush.** Configuration mutations stay in memory for the life of
  the process and are dropped on exit.

Build it with `cargo build --release --bin ezmigrations-mcp` (or any
`cargo build` that walks the whole workspace) — the artefact lands under
`src-tauri/target/release/`. It is not yet wired into Tauri's installer
bundle, so on a fresh end-user machine you'll need to distribute it
out-of-band or build from source.

### Authentication

None in v1. The server binds to `127.0.0.1` only — anything that can read
the port file from your user's app-data directory could already invoke the
GUI. If you're running on a multi-user host, gate the port file with file
permissions or add a bearer-token layer to the Axum router.

## Connecting an agent

The general flow:

1. Read the port file and parse the `url` field.
2. Connect a streamable-HTTP MCP client to that URL.
3. The server's `initialize` response includes a multi-paragraph
   `instructions` field describing the tool surface and state model. Most
   agents fold those instructions into their system prompt automatically.
   The same content can be pasted into an agent's system prompt before
   connection — see [agent-debrief.md](./agent-debrief.md).

### Claude Code

Claude Code reads MCP servers from `claude_desktop_config.json` (or the
project-local `.mcp.json`). With a fixed-port helper script:

```jsonc
{
  "mcpServers": {
    "ezmigrations": {
      "type": "http",
      "url": "http://127.0.0.1:51843/mcp"
    }
  }
}
```

If you want the port discovered dynamically, run a shell wrapper that reads
the port file before launching the client:

```bash
# Linux (XDG_DATA_HOME or ~/.local/share)
URL=$(jq -r .url "${XDG_DATA_HOME:-$HOME/.local/share}/ezmigrations/mcp-port.json")
claude mcp call ezmigrations list_migrations
```

```bash
# macOS
URL=$(jq -r .url "$HOME/Library/Application Support/ezmigrations/mcp-port.json")
claude mcp call ezmigrations list_migrations
```

```powershell
# Windows
$url = (Get-Content "$env:APPDATA\ezmigrations\mcp-port.json" | ConvertFrom-Json).url
```

### Other clients

Any MCP client that speaks the Streamable HTTP transport works — point it
at the `url` from the port file. The session manager is local-only, so
each client gets its own session id; the underlying `AppState` is shared,
so concurrent sessions see the same project, migrations, branch, and
preferences.

## Capabilities

- **23 tools** — one per Tauri command. Argument names match the GUI's
  command bindings so configs are reusable.
- **8 resources** (7 fixed + 1 templated) — JSON / text snapshots of
  current state, free of side effects.
- **0 prompts** in v1 — placeholder for guided workflows in a later
  release.

## Tool reference

| Tool | Purpose | Key inputs | Mutates? |
| --- | --- | --- | --- |
| `set_project` | Make a path the active migrations project | `project_path`, `db_context`, `startup_project` | App state |
| `get_project` | Active project details | — | No |
| `get_saved_projects` | List saved projects | — | No |
| `save_project` | Save a new project entry | `name`, `path`, `db_context`, `startup_project` | App state |
| `update_saved_project` | Edit saved project metadata | `id`, `name`, `path`, `db_context`, `startup_project` | App state |
| `delete_saved_project` | Delete a saved project | `id` | App state |
| `switch_project` | Make a saved entry active | `id` | App state |
| `set_stable_migration` | Pin/clear the stable rollback target | `migration_name` (nullable) | App state |
| `get_preferences` | Read preferences | — | No |
| `set_preferences` | Replace preferences | `preferences` | App state |
| `list_migrations` | EF migration list, applied + custom-SQL flags | — | No |
| `add_migration` | `dotnet ef migrations add` | `name` | DB schema / files |
| `remove_migration` | `dotnet ef migrations remove [--force]` | `force` | DB schema / files |
| `update_database` | `dotnet ef database update [target]` | `target` (`""`=latest, `"0"`=revert all) | DB |
| `get_migration_sql` | Parsed Up/Down + custom SQL | `migration_name` | No |
| `squash_migrations` | Squash a range, preserving custom SQL | `from_migration`, `to_migration`, `new_name` | DB + files |
| `generate_script` | `dotnet ef migrations script` | `from`, `to` | No |
| `cancel_running_operation` | Abort the in-flight EF op | — | Process control |
| `get_current_branch` | Current git branch | — | No |
| `list_git_branches` | All branches except the current | — | No |
| `switch_branch_with_migrations` | Managed branch switch | `target_branch` | DB + git working tree |
| `start_branch_watcher` | (Reports that the watcher is owned by the GUI) | — | No |
| `start_migration_watcher` | (Reports that the watcher is owned by the GUI) | — | No |

The 5 mutating EF tools (`add_migration`, `remove_migration`,
`update_database`, `squash_migrations`, `switch_branch_with_migrations`)
serialize through an in-process mutex. A second call from any client (or
the GUI) waits until the first finishes.

## Resource reference

| URI | Returns |
| --- | --- |
| `ezmigrations://project/current` | JSON: active `ProjectInfo` or `null` |
| `ezmigrations://projects` | JSON: `SavedProject[]` |
| `ezmigrations://preferences` | JSON: `Preferences` |
| `ezmigrations://migrations` | JSON: `Migration[]` (refreshes from EF) |
| `ezmigrations://migrations/{name}/sql` | JSON: parsed Up/Down + custom SQL (templated) |
| `ezmigrations://branches` | JSON: `BranchInfo[]` (local + remote, excluding current) |
| `ezmigrations://branches/current` | Plain text: current branch name |
| `ezmigrations://app/status` | JSON: `{ project_loaded, current_branch, operation_in_progress, watchers_active }` |

The `migrations/{name}/sql` template is advertised via
`list_resource_templates`; the other 7 appear in `list_resources`.

## Troubleshooting

- **Port file missing** — neither the desktop app nor the `ezmigrations-mcp`
  headless binary is running, or one of them crashed before the MCP server
  finished binding. See [Headless mode](#headless-mode) to start a server
  without the GUI.
- **`No project configured` errors** — most tools require an active
  project. Call `get_project` first; if it returns `null`, either call
  `set_project` with a path or pick one from `get_saved_projects` then
  `switch_project`.
- **Tool hangs / takes minutes** — `dotnet ef` builds the startup project
  before every command, which can take 30s+ on a cold cache. Call
  `cancel_running_operation` to abort.
- **`Another operation is already running`** — the GUI or another MCP
  session is mid-`dotnet ef`. Wait, or cancel the other op.
- **Custom SQL lost after squash** — shouldn't happen; if it does, file
  a bug with the parsed `migrations/{name}/sql` resource output for the
  source migrations attached.
- **Watcher tools say "owned by desktop window"** — the file watchers
  emit events on Tauri's IPC channel, which doesn't route to MCP clients
  in v1. Under the headless `ezmigrations-mcp` binary there is no IPC
  channel at all, so the watchers report status only. Resources still
  reflect on-disk changes when re-read in either mode.
