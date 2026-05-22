# Architecture

This document describes how ezMigrations is structured after the v1.1.0
React + TanStack Query rewrite. Audience: a new contributor (or the author,
six months from now) trying to find the file that owns a given feature.

## Overview

ezMigrations is a Tauri v2 desktop app:

- The Rust backend (`src-tauri/src/`) owns all I/O: it reads/writes the
  config file in the OS app-data dir, watches the project on disk, and
  shells out to `dotnet ef` and `git`.
- The React frontend (`src/`) talks to Rust exclusively through
  `invoke("command_name", args)` (see `src/lib/tauri.js`).
- TanStack Query owns *server* state (anything that came from Rust:
  migrations, branches, project config, preferences). The Query cache is
  the single source of truth for that data and is invalidated either by
  mutations or by backend file-watcher events.
- A React Context (`src/lib/ui-store.jsx`) owns *UI* state — which dialog
  is open, what's selected, what's checked, the search query, the
  operation overlay. There is no Zustand in this codebase.
- Long-running backend operations stream progress over a Tauri event
  (`operation-phase`) which feeds `OperationOverlay.jsx`.

```
 ┌────────────────────────────────────────────────────────────────┐
 │  React (src/)                                                  │
 │  ┌───────────────────────┐    ┌──────────────────────────────┐ │
 │  │ TanStack Query cache  │◄──►│ Components (Header, MainView,│ │
 │  │ keys: project,        │    │  MigrationsTable, dialogs/…) │ │
 │  │  migrations, branches │    └──────────────┬───────────────┘ │
 │  └─────────▲─────────────┘                   │                 │
 │            │                                 ▼                 │
 │            │                  ┌─────────────────────────────┐  │
 │            │                  │ UIProvider (React Context)  │  │
 │            │                  │ overlay, dialog, checked, … │  │
 │            │                  └─────────────────────────────┘  │
 │            │   queries.js  ▲           ▲                       │
 │            │   mutations.js│           │ openDialog/setOverlay │
 │            ▼               │           │                       │
 │  ┌───────────────────────────────────────────────────────────┐ │
 │  │ src/lib/tauri.js     invoke(cmd, args)   listen(event)    │ │
 │  └────────────────────┬───────────────────┬──────────────────┘ │
 └─────────────────────  │  ───────────────  │  ──────────────────┘
                         │ Tauri IPC         │ events
 ┌─────────────────────  ▼  ───────────────  ▼  ──────────────────┐
 │ Rust (src-tauri/src/)                                          │
 │  commands.rs  ── invoke_handler entrypoints                    │
 │      │                                                         │
 │      ├─► dotnet.rs  ──► process::command("dotnet")  ──► ef CLI │
 │      ├─► git.rs     ──► process::command("git")     ──► git    │
 │      ├─► parser.rs  ──► reads/writes *.cs migration files      │
 │      └─► state.rs   ──► Mutex<AppConfig>, Mutex<Migrations>, … │
 │                                                                │
 │  File watchers (notify crate) emit:                            │
 │    "migrations-changed" — when *.cs in Migrations/ changes     │
 │    "branch-changed"     — when .git/HEAD changes               │
 │    "operation-phase"    — per-step progress for long ops       │
 └────────────────────────────────────────────────────────────────┘
```

## Backend (Rust / Tauri)

### `src-tauri/src/main.rs`
Tiny entrypoint — calls `ez_migrations_lib::run()`. The
`windows_subsystem = "windows"` attribute hides the console window in
release builds.

### `src-tauri/src/lib.rs`
Builds the Tauri app, registers `tauri_plugin_dialog`, attaches the
default `AppState`, and lists every command exposed to the frontend
(`lib.rs:15-39`). This is the canonical list of the IPC surface — 23
commands at the moment.

### `src-tauri/src/state.rs`
The shared `AppState` (`state.rs:60-71`) that every command receives via
`State<'_, AppState>`. It's a bag of `Mutex`es holding:

- `config: Mutex<Option<ProjectConfig>>` — the currently selected
  project's path / DbContext / startup project.
- `app_config: Mutex<AppConfig>` — the on-disk config: list of saved
  projects, active project id, and `Preferences`. Persisted as
  `app_config.json` in the OS app-data dir.
- `migrations: Mutex<Vec<Migration>>` — most recent EF listing,
  re-read on every `list_migrations` call. Used by squash so the
  command doesn't have to re-parse files.
- `current_branch: Mutex<String>` — last-known git branch.
- `watching` / `watching_migrations: Mutex<bool>` — flags so the
  watcher commands don't spawn duplicate threads.
- `watcher_cancel: Mutex<Arc<AtomicBool>>` — cancel token shared with
  background watcher threads; replaced on project change to stop the
  old watchers without joining them.
- `op_cancel: Arc<AtomicBool>` — set by `cancel_running_operation` so
  multi-step orchestration (squash, branch switch) can bail between
  EF invocations even when no child process is alive to kill.

`Migration`, `SavedProject`, `Preferences`, `ProjectConfig`, `AppConfig`
all live here as plain serde-derived structs that cross the IPC
boundary.

### `src-tauri/src/commands.rs`
Every `#[tauri::command]` lives here. Notable helpers:

- `PhasedOp` / `PhaseEmitter` (`commands.rs:76-135`) — RAII guard for
  multi-step operations. Constructed at the top of a command, cloned
  into the `spawn_blocking` closure, calls `emit("operation-phase", …)`
  between steps and exposes `check_canceled()` so squash and branch
  switch poll the cancel flag between EF calls. `Drop` clears
  `op_cancel` so the next operation starts clean.
- `enrich_ef_error` (`commands.rs:18-28`) — recognizes the
  "doesn't match your migrations assembly" mistake and prepends a
  human-readable explanation before passing the error on.
- `migrate_legacy_config` (`commands.rs:236-276`) — one-shot importer
  from the old single-project `project_config.json` to the v1.1 multi-
  project `app_config.json`.
- `start_branch_watcher` (`commands.rs:1469-1582`) — spawns an OS
  thread holding a `notify::RecommendedWatcher` over the parent of
  `.git/HEAD`. Debounced 2s, with a 500ms settle delay so git
  finishes writing before we read. Compares the new branch name to
  the last one seen and emits `branch-changed` only on a real change.
- `start_migration_watcher` (`commands.rs:1591-1684`) — same pattern
  on the migrations directory, recursive, filtered to `*.cs` files
  that aren't `.Designer.cs` or `ModelSnapshot`. Emits a payload-less
  `migrations-changed` event.
- `switch_branch_with_migrations` (`commands.rs:1236-1447`) — the
  managed-branch-switch flow. Lists migrations on the target branch
  via `git ls-tree`, intersects with current ones, computes a
  `rollback_target` (most-recent common migration), then sequences
  `dotnet ef database update <rollback_target>` → `git checkout` →
  `dotnet ef database update` (latest). Refuses to run if there is
  no common migration and any migration is currently applied — the
  resulting "revert everything" would be too destructive to be safe.
- `squash_migrations` (`commands.rs:956-1154`) — the four-step squash
  pipeline (revert → remove range → add new → apply), with custom
  SQL extracted by `parser.rs` and re-injected into the new
  migration file before re-apply.

### `src-tauri/src/dotnet.rs`
Wraps `dotnet ef`. Notable pieces:

- `build_ef_command` (`dotnet.rs:130-196`) — the heart of the wrapper.
  Runs `dotnet ef` from the *solution* root (the parent of the
  migrations project) and passes `--project`/`--startup-project` as
  paths relative to that root. This is what makes the displayed
  command match what a user would type.
- `auto_detect_startup_project` (`dotnet.rs:47-91`) — when the user
  didn't configure a startup project, walks sibling directories for a
  `.csproj` containing `Microsoft.NET.Sdk.Web`, falling back to any
  sibling csproj. EF needs the host project to load the DbContext's
  DI registrations at design time.
- `enrich_path` (`dotnet.rs:18-41`) — macOS only. GUI apps inherit a
  trimmed PATH; we prepend `~/.dotnet/tools`, `/usr/local/share/dotnet`,
  etc. Compiled out on Windows because Windows uses `;` as the
  separator and prepending Unix-style entries corrupts it.
- `run_ef_cancellable` (`dotnet.rs:310-417`) — spawn the child,
  stash it in a process-global `OnceLock<Mutex<Option<…>>>`, poll
  `try_wait()` every 120ms. `cancel_running_operation` reaches into
  the same Mutex and calls `child.kill()`. Used for everything that
  can take more than a moment (`add`, `remove`, `database update`).
- `enrich_with_build_diagnostics` (`dotnet.rs:222-253`) — when EF
  reports "Build failed. Use dotnet build to see the errors.", we
  follow up with a real `dotnet build --nologo -clp:ErrorsOnly`
  against the startup project and append the actual compile errors
  to the EF stderr so the error dialog shows something useful.
- `list_migrations` parsing (`dotnet.rs:440-531`) — the EF CLI is
  noisy and sometimes exits 0 even when it can't load the DbContext.
  We surface lines like "Unable to resolve service" / "No DbContext
  was found" as real errors before parsing, then strip warnings,
  info lines, banners, and EF logging prefixes from the output.

### `src-tauri/src/git.rs`
Thin wrapper over `git` invocations. `GitService::run_git` (`git.rs:9-22`)
is the shared shell-out helper. Notable methods: `list_branches`
(`git.rs:49-84`, uses `git for-each-ref` to separate locals from
remotes), `switch_branch` (`git.rs:108-130`, falls back to
`checkout --track` for remote-only refs), `list_files_at_ref`
(`git.rs:132-156`, uses `git ls-tree -r` to enumerate migration files
on the target branch without checking it out — the magic that powers
managed switch's safety check), and `get_head_path` for the branch
watcher.

### `src-tauri/src/parser.rs`
Parses and rewrites EF Core migration `.cs` files. The Rust regex
crate doesn't support look-around, so brace matching is done by
`find_matching_brace` (`parser.rs:205-253`), which respects C# verbatim
strings (`@"…""…"`) when counting `{`/`}`. The interesting public
methods:

- `find_migration_files` / `find_migrations_dir` — discovers the
  Migrations folder, with a few fallbacks (`./Migrations`,
  `./Data/Migrations`, then a depth-limited walk, then a sibling-dir
  walk for multi-project solutions).
- `parse_file` — returns `ParsedMigration` with `up_body`, `down_body`,
  and ordered `SqlStatement` lists. Each `SqlStatement` carries
  `operation_index` so squash can preserve the relative ordering of
  `migrationBuilder.Sql(...)` calls against schema operations.
- `deduplicate_sql` (`parser.rs:324-357`) — when multiple migrations
  in the squash range touch the same stored procedure or view, keep
  the last version for `Up` and the first version for `Down` (so
  Down still reverts to the pre-squash state).
- `inject_custom_sql` (`parser.rs:361-412`) — reopens the freshly-
  scaffolded squashed migration, finds the matching `}` of `Up`/`Down`
  via the brace matcher, and inserts `migrationBuilder.Sql(@"…");`
  lines before the closing brace.

### `src-tauri/src/process.rs`
17 lines. Single helper, `command(program)`, that returns a
`std::process::Command` with `CREATE_NO_WINDOW` set on Windows so
child processes don't flash a console. Every shell-out in
`dotnet.rs` and `git.rs` goes through this.

## Frontend (React + TanStack Query + shadcn/ui)

### Why the rewrite

The previous frontend was vanilla JS plus a hand-rolled "state +
DOM-patching" loop. By the time the app supported a branch watcher,
nested dialogs, an operation overlay with cancel, multiple saved
projects, and out-of-sync detection, the loop spent more time
re-reading the DOM than rendering. The TanStack rewrite buys three
things:

1. **Caching + invalidation.** `useMigrations()` returns the same data
   to every consumer, refetches on demand, and dedupes concurrent
   callers. Mutations declare what they invalidate.
2. **Server-event integration.** The backend's file watchers emit
   `migrations-changed` / `branch-changed`; `App.jsx` listens once
   and calls `qc.invalidateQueries(...)`. Every component using the
   relevant key updates automatically.
3. **Lazy code splitting.** Dialogs and the settings sheet are
   `React.lazy`-loaded, so the initial bundle is just the table view.

### Layer cake

The frontend is a small stack of single-purpose modules:

- `src/lib/tauri.js` (10 lines) — the only place `window.__TAURI__` is
  touched. Exposes `invoke`, `listen`, and `openFolderDialog`.
- `src/lib/queries.js` (70 lines) — `queryKeys` (one object, every
  key in the app) plus a `useX()` hook per query. Seven keys total:
  `project`, `migrations`, `migrationSql(name)`, `branches`,
  `currentBranch`, `savedProjects`, `preferences`.
- `src/lib/mutations.js` (~290 lines) — every write goes here. Each
  mutation invalidates the queries it touches, calls a toast on
  success, and routes failures either to `errToast` or, for EF
  failures, through `useEfErrorHandler`. Long-running ones go
  through `useOperationMutation` which raises the overlay.
- `src/lib/ui-store.jsx` — React Context with `useState` slots for
  overlay, dialog, selection, checked set, search, branch-change
  state. `useUI()` returns the whole bag.
- Components consume `useX()` queries and `useY()` mutations
  directly. They never call `invoke` themselves.

#### End-to-end example: the migrations list

1. Rust: `commands::list_migrations` (`commands.rs:703`) holds the
   `migrations` mutex, calls `DotnetEf::list_migrations`, then
   walks the cached migration files and parses each for custom SQL.
   Returns `Vec<Migration>`.
2. IPC wrapper: `src/lib/tauri.js:3` — `invoke("list_migrations")`.
3. Query: `useMigrations` in `src/lib/queries.js:22-29`. Key is
   `["migrations"]`, staleTime 5s.
4. Consumer: `MigrationsTable` (`src/components/migrations/MigrationsTable.jsx:172`)
   via `MainView` (`src/components/MainView.jsx:13`). Also
   `Header` (`Header.jsx:29`, for the pending-count badge),
   `MigrationsBanners` (for out-of-sync detection), and `SquashDialog`
   (to look up which rows are checked).
5. Mutation that invalidates it: `useAddMigration` in
   `src/lib/mutations.js:144`. On success: `toast.success(msg)` +
   `invalidateMigrations(qc)` (mutations.js:10-11) which fires
   `qc.invalidateQueries({ queryKey: queryKeys.migrations })`.
6. *Also* invalidated by the file watcher: `App.jsx:35-40` listens for
   `migrations-changed` and calls the same invalidator. So if a
   teammate's `dotnet ef migrations add` finishes in another
   terminal, the UI updates.

### UI state vs server state

In the TanStack cache (re-fetchable, may go stale):

- The current project (`useProject`)
- The migration list (`useMigrations`)
- One migration's parsed SQL (`useMigrationSql(name)`)
- Git branches (`useBranches`)
- Current branch name (`useCurrentBranch`)
- Saved projects (`useSavedProjects`)
- User preferences (`usePreferences`)

In `ui-store.jsx` (ephemeral, lives only in memory):

- `overlay` — `{ operation, message, cancelable } | null` for the
  full-screen operation overlay
- `dialog` — `{ type, props } | null`, single-dialog stack
- `hotkeysOpen`, `settingsOpen` — booleans for the help dialog and
  the right-side settings sheet
- `selectedMigrationId` — which row's detail panel is open
- `checked` — `Set<string>` of migration ids selected for squash
- `searchQuery` — filter text driving the TanStack Table global filter
- `previousBranch`, `syncDismissed` — used by `MigrationsBanners`
  to label the foreign-migration banner with the branch the user
  just left

Rule of thumb: if Rust could regenerate it, it goes in the cache; if
it only exists because of a user click, it goes in `ui-store`.

### Dialog system

A single `DialogRoot.jsx` (44 lines) renders at most one dialog at
a time:

- `useUI()` exposes `dialog` (`{ type, props } | null`) plus
  `openDialog(type, props)` / `closeDialog()`.
- `DialogRoot.jsx:8-32` registers each dialog as a `React.lazy(...)`
  import: `newMigration`, `squash`, `forceRemove`, `addProject`,
  `editProject`, `changeProject`, `switchBranch`, `branchChanged`,
  `migrationError`.
- The whole tree is wrapped in `<React.Suspense fallback={null}>` in
  `App.jsx:137-141`, so the first `openDialog` triggers the chunk
  fetch.

`ProjectDialog` is reused for both `addProject` and `editProject` —
`DialogRoot.jsx:18-19` aliases both to the same lazy import; the
dialog reads `mode` from its props to decide which mutation to fire.

### Toast system

`src/lib/toast.js` is a thin sugar layer over `sonner`. `toast(msg)`
is itself callable (default duration), with `.success`, `.error`,
`.warning`, `.info`, `.loading`, `.promise`, `.dismiss` attached.
The `errToast(prefix)` factory (`toast.js:14`) returns a mutation-
`onError` handler that prepends a context string:
`toast.error("Failed to save project: <err>")`. The `<Toaster />`
provider is mounted once in `main.jsx:23`.

### EF error handling

When `dotnet ef` fails, the raw stderr is multiline log noise. The
frontend turns that into a structured dialog:

1. `parseEfError` (`src/lib/parse-ef-error.js:42`) tries to extract
   `failedMigration`, `failedDirection` (applying vs reverting),
   `sqlError` (from a `Microsoft.Data.SqlClient.SqlException` or a
   `fail: Microsoft.EntityFrameworkCore.Database.Command` block), and
   the offending `statement` (from a `Failed executing DbCommand`
   block). Returns `null` if the output isn't recognizably EF.
2. `useEfErrorHandler` (`src/lib/ef-error-handler.js:16`) is a hook
   that returns `(err, messages) => …`. If `parseEfError` returned
   something, it calls `ui.openDialog("migrationError", {...})`;
   otherwise it falls back to `toast.error`. It also picks `rollback`
   vs `apply` copy from the `messages` argument by looking for
   `/roll back|reverting/` in the raw error.
3. `MigrationErrorDialog.jsx` renders the structured fields with
   a "Copy full log" button.

The mutations that need this flow are `useUpdateDatabase`,
`useRemoveMigration`, `useSquashMigrations`, and `useSwitchBranch`
(see `src/lib/mutations.js`).

### Operation overlay (per-step progress + cancel)

`OperationOverlay.jsx` is a full-screen Suspense-style overlay shown
whenever `ui.overlay` is non-null. The interesting bit is that the
overlay is *driven by backend events* once the operation starts:

1. The mutation factory `useOperationMutation` (`mutations.js:35-69`)
   calls `setOverlay({ operation, message, cancelable: true })`
   *before* invoking, so the user sees an initial message
   instantly ("Preparing to switch to main…").
2. Rust's `PhaseEmitter` fires `operation-phase` events with
   per-step messages ("Step 2/4 — Removing migration 3/12…").
3. `App.jsx:54-63` listens for `operation-phase` and merges the new
   `message` into `ui.overlay` so the user sees real progress.
4. The Cancel button calls `useCancelOperation` (`mutations.js:289`)
   → Rust `cancel_running_operation` → `op_cancel` flag set +
   any running EF child killed.
5. If the resulting error message contains "Canceled by user",
   `useOperationMutation` short-circuits the error pipeline and
   shows a neutral toast ("Operation canceled.") instead of a
   failure toast (`mutations.js:60-65`).

## Feature → Code map

| Feature | Backend | Frontend |
|---------|---------|----------|
| Initial project setup (path + DbContext + startup) | `commands::set_project` (`commands.rs:278`) | `SetupView.jsx`, `useSetProject` (`mutations.js:71`) |
| Saved-projects list + switch | `commands::{get_saved_projects, save_project, update_saved_project, delete_saved_project, switch_project}` (`commands.rs:488-660`) | `SettingsSheet.jsx`, `ProjectDialog.jsx`, `useSavedProjects` / `useSwitchProject` etc. |
| Migration list + status | `commands::list_migrations` (`commands.rs:703`) + `DotnetEf::list_migrations` (`dotnet.rs:440`) | `useMigrations` (`queries.js:22`), `MigrationsTable.jsx` |
| New migration | `commands::add_migration` (`commands.rs:765`) | `NewMigrationDialog.jsx`, `useAddMigration` (`mutations.js:144`) |
| Remove last / force remove | `commands::remove_migration` (`commands.rs:802`) | `ForceRemoveDialog.jsx`, `useRemoveLastOrForce` (`row-actions.js:14`), `useRemoveMigration` (`mutations.js:210`) |
| Update database (full or to migration) | `commands::update_database` (`commands.rs:844`) | `MigrationsToolbar` "Update DB", row Play action via `useApplyTo` (`row-actions.js:5`), `useUpdateDatabase` (`mutations.js:180`) |
| Cancel running op | `commands::cancel_running_operation` (`commands.rs:893`) | `OperationOverlay.jsx` Cancel button, `useCancelOperation` (`mutations.js:289`) |
| Stable migration pin | `commands::set_stable_migration` (`commands.rs:663`) | Pin icon in row actions, `useSetStable` (`mutations.js:231`) |
| Migration detail (Up/Down + custom SQL) | `commands::get_migration_sql` (`commands.rs:910`) + `MigrationParser::parse_file` (`parser.rs:158`) | `DetailPanel.jsx`, `useMigrationSql` (`queries.js:31`) |
| Squash with custom SQL preservation | `commands::squash_migrations` (`commands.rs:956`) + `MigrationParser::{deduplicate_sql, inject_custom_sql}` (`parser.rs:324`, `:361`) | `SquashDialog.jsx`, `useSquashMigrations` (`mutations.js:158`) |
| Generate SQL script | `commands::generate_script` (`commands.rs:1159`) + `DotnetEf::script_migration` (`dotnet.rs:585`) | (no current consumer in `src/`) |
| Branch list | `commands::list_git_branches` (`commands.rs:1211`) + `GitService::list_branches` (`git.rs:49`) | `useBranches` (`queries.js:40`), `SwitchBranchDialog.jsx` |
| Managed branch switch (rollback → checkout → update) | `commands::switch_branch_with_migrations` (`commands.rs:1236`) | `SwitchBranchDialog.jsx`, `useSwitchBranch` (`mutations.js:245`) |
| External branch change detection | `commands::start_branch_watcher` (`commands.rs:1469`), emits `branch-changed` | `App.jsx:41-53` listener → `BranchChangedDialog.jsx` |
| External migration file change detection | `commands::start_migration_watcher` (`commands.rs:1592`), emits `migrations-changed` | `App.jsx:35-40` listener → `qc.invalidateQueries(migrations)` |
| Foreign / out-of-sync detection | (frontend only) | `detect-sync.js` + `MigrationsBanners.jsx` + "Foreign" row badge in `MigrationsTable.jsx:84` |
| Per-step progress UI | `PhaseEmitter` (`commands.rs:76`), emits `operation-phase` | `App.jsx:54-63` + `OperationOverlay.jsx`, `useOperationMutation` (`mutations.js:35`) |
| EF error dialog | `enrich_ef_error` (`commands.rs:18`) + `enrich_with_build_diagnostics` (`dotnet.rs:222`) | `parse-ef-error.js`, `ef-error-handler.js`, `MigrationErrorDialog.jsx` |
| Preferences (notify on branch change) | `commands::{get,set}_preferences` (`commands.rs:683-698`) | `SettingsSheet.jsx`, `useSetPreferences` (`mutations.js:280`) |
| Keyboard shortcuts (⌘N, ⌘R, ⌘F, Esc, ?) | — | `App.jsx:84-119` global keydown listener, `HotkeysDialog.jsx` |
| Refresh-on-visible | — | `useRefreshOnVisible` (`src/lib/hooks.js:12`), wired in `App.jsx:79` |
| Migration filter / search | — | `MigrationsToolbar.jsx` Search input → `ui.searchQuery` → TanStack Table `globalFilter` in `MigrationsTable.jsx:186-194` |
| Folder picker | `tauri-plugin-dialog` | `openFolderDialog` (`tauri.js:6`), `FolderInput.jsx` |

## Data flow examples

### 1. User clicks "New migration"

1. `MigrationsToolbar.jsx:18` — click handler calls
   `ui.openDialog("newMigration")`.
2. `ui-store.jsx:22` sets `dialog = { type: "newMigration", props: {} }`.
3. `DialogRoot.jsx:34-43` reads `dialog`, lazy-loads
   `NewMigrationDialog.jsx`.
4. User types `AddUsersTable`, presses Enter. Form submit
   (`NewMigrationDialog.jsx:21`) calls
   `add.mutateAsync({ name: "AddUsersTable" })`.
5. `useAddMigration` is `useOperationMutation` with
   `operation: "add_migration"`. Before the IPC call,
   `setOverlay({ operation: "add_migration", message: "Creating
   migration AddUsersTable…", cancelable: true })` (`mutations.js:46`).
   `OperationOverlay` renders.
6. `invoke("add_migration", { name: "AddUsersTable" })` →
   `commands::add_migration` (`commands.rs:765`). A `PhasedOp` is
   created; it emits `operation-phase` with phase `creating` and the
   "Creating migration AddUsersTable…" message. The frontend listener
   in `App.jsx:54-63` overwrites `ui.overlay.message`.
7. `DotnetEf::add_migration` (`dotnet.rs:534`) shells out via
   `run_ef_cancellable`. Meanwhile the migration `.cs` file appears
   on disk.
8. The migration watcher (`commands.rs:1592`) sees the new file and
   emits `migrations-changed`. The listener in `App.jsx:35-40`
   calls `qc.invalidateQueries({ queryKey: queryKeys.migrations })`.
9. The EF call returns success. `useAddMigration`'s `onSuccess`
   clears the overlay, fires `toast.success(msg)`, and invalidates
   `migrations` again for good measure. The dialog's `await`
   resumes and calls `onClose()`.
10. `MigrationsTable` refetches `useMigrations`, sees the new row.

### 2. User switches git branch externally (e.g. in a terminal)

1. The branch watcher thread (started by `App.jsx:75`'s
   `invoke("start_branch_watcher")`) was already running, watching
   `.git/HEAD`.
2. The user runs `git checkout feature/foo` in a terminal. The
   watcher fires (`commands.rs:1530-1571`), debounces 2s, re-reads
   `HEAD`, sees the branch changed, and emits `branch-changed` with
   `{ old_branch, new_branch, reverted_to_stable: false }`.
3. `App.jsx:41-53` receives it, sets `ui.previousBranch = old_branch`
   (used later by the out-of-sync banner), updates the query cache
   with the new branch name (`qc.setQueryData(queryKeys.currentBranch,
   new_branch)`), invalidates `migrations`, and — if the user hasn't
   disabled it in preferences — opens the `branchChanged` dialog.
4. `BranchChangedDialog.jsx` offers "Update to latest" (which calls
   `useUpdateDatabase` against the new branch) or "Not now".
5. Independently, `useMigrations` refetches. If the previously-applied
   migrations are *not* in the new branch's migrations folder, they
   show up as applied-but-not-listed; `detect-sync.js` flags them,
   `MigrationsBanners.jsx` shows the red "Out-of-sync" banner with a
   "Revert Foreign" action that calls `useUpdateDatabase` with the
   correct rollback target.

### 3. `dotnet ef database update` fails on a bad migration

1. User clicks "Update DB" in `MigrationsToolbar.jsx:31`.
2. `useUpdateDatabase` raises the overlay, calls
   `invoke("update_database", { target: "" })`.
3. `commands::update_database` (`commands.rs:844`) emits
   `operation-phase` with "Updating database to latest…", then
   `DotnetEf::update_database` shells out. The migration's `Up()`
   throws `SqlException: There is already an object named 'Users'`.
4. `dotnet ef` exits non-zero. Rust returns
   `Err(enrich_ef_error(format!("Failed to update database: {}", …)))`.
5. `useOperationMutation.onError` (`mutations.js:57`) clears the
   overlay and delegates to the mutation's own `onError` —
   `handleEfError(err, { apply: { title: "Migration failed", … } })`
   (`mutations.js:194-207`).
6. `useEfErrorHandler` (`ef-error-handler.js:16`) calls
   `parseEfError(raw)`. `parse-ef-error.js` matches
   `Applying migration 'XXX'.` to set `failedMigration` +
   `failedDirection: "applying"`, finds the `SqlException` block for
   `sqlError`, and the `Failed executing DbCommand` block for the
   offending `statement`. It returns the structured object.
7. `ui.openDialog("migrationError", { title, context, error: parsed })`.
   `MigrationErrorDialog.jsx` renders the structured fields with the
   "Copy full log" button. The DB stays at the last successful
   migration.

## Build and dev

- `npm install` once.
- `npm run tauri dev` — Vite serves the frontend with HMR; Tauri
  spawns the desktop window pointed at the dev server. The
  `package.json` `scripts` block only defines `dev`, `build`,
  `preview`, and `tauri`; everything Tauri-related goes through
  `npm run tauri <cmd>`.
- `npm run tauri build` — full release build (Vite frontend bundle
  → Cargo `--release` → platform installer).
- `__APP_VERSION__` is a Vite-defined global rendered in
  `Header.jsx:48`.
- See `docs/releasing.md` for the signed/notarized release pipeline.
