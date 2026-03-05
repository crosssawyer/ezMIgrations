# Changelog

All notable changes to ezMigrations are documented here.

## [0.5.0] - 2026-03-05

### Added
- **Out-of-sync detection** — detects "foreign" migrations applied from another branch (Applied after Pending), shows a red warning banner with count and source branch name
- **Foreign migration highlighting** — foreign rows get a red left border and "Foreign" label in the migration list
- **One-click revert** — "Revert Foreign" button reverts the database to the last clean migration before the foreign ones
- **Dismiss sync warning** — dismiss button hides the banner for the session; reappears on next branch change
- **Preferences panel** — new Preferences section in Settings with a toggle to disable branch-change notification prompts
- **Persisted preferences** — preferences saved to `app_config.json` via new `get_preferences` / `set_preferences` backend commands

## [0.4.0] - 2026-03-04

### Performance
- Guard `refreshMigrations()` against concurrent calls — duplicate `dotnet ef` processes are now coalesced into one (`bfb56f8`)
- Add cancellation tokens to file watcher threads so project switches cleanly stop old watchers instead of leaking them (`bfb56f8`)
- Cache migration file list once per refresh instead of scanning the directory once per migration (`bfb56f8`)
- Filter branch watcher to only react to `.git/HEAD` changes, ignoring all other `.git/` file writes (`bfb56f8`)
- Increase migration watcher debounce to 3s and filter out `.Designer.cs` and `ModelSnapshot` files (`bfb56f8`)

## [0.3.2] - 2026-03-04

### Fixed
- Hide console windows on Windows when spawning `dotnet ef` and `git` subprocesses via `CREATE_NO_WINDOW` flag (`f7da948`)
- Extract shared `process::command()` helper so all subprocesses get the flag (`f7da948`)

## [0.3.1] - 2026-03-04

### Fixed
- Enrich `PATH` on macOS so the GUI app can find `dotnet` — prepends `~/.dotnet/tools`, `/usr/local/share/dotnet`, `/opt/homebrew/bin`, etc. before spawning commands (`668aca2`)

## [0.3.0] - 2026-03-03

### Added
- **Keyboard shortcuts**: `Ctrl+N` (new migration), `Ctrl+R` (refresh), `Ctrl+F` (focus search), `Escape` (close panels/clear search) (`13cbf93`)
- **Migration search/filter**: real-time filter input in the toolbar with case-insensitive matching on migration name (`13cbf93`)
- **Hotkeys help popup**: press `?` or click the `?` button in the header to see all shortcuts (`13cbf93`)
- Distinct "No migrations match your filter" empty state when search yields no results (`13cbf93`)
- Button title tooltips show shortcut hints (`Ctrl+N`, `Ctrl+R`) (`13cbf93`)

## [0.2.0] - 2026-03-03

### Added
- Rewrite from Python CLI to Tauri v2 desktop app (`940c656`)
- Async `dotnet ef` commands with cancellable operations (`dd95c4f`)
- Settings page with saved projects, project switching (`de22fe3`, `462ac5b`)
- Branch watcher with auto-revert to stable migration on branch switch
- Migration file watcher with auto-refresh on `.cs` changes
- Squash migrations with custom SQL preservation
- Drift warning banner when pending migrations detected
- Health indicator, pending badge, status bar
- Cross-platform release pipeline via GitHub Actions (`282c1dc`, `c025663`)

### Fixed
- Enable Tauri bundling, improve EF project config UX (`316108e`)
- Use `tauri-action` for release asset uploads (`5189c1d`)
- Handle existing tag in release workflow (`03f5d27`)

## [0.1.0] - 2026-02-28

### Added
- Initial Python CLI tool for EF Core migration management
- Up/Down method extraction and stored procedure pipeline
