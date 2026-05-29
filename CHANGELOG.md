# Changelog

All notable changes to ezMigrations are documented here.

## [1.2.0] - 2026-05-29

### Added
- **Keyboard navigation in the migrations table** — arrow keys move a focus cursor through migration rows, with a declarative shortcut table driving update, branch menu, fetch, and switch actions (`790c810`, `1c90aa5`, `26a5f08`)
- **Keyboard-drivable branch switch dialog** — the whole switch dialog can be operated from the keyboard, with the filter input autofocused on open (`7688204`)
- **"All" tab and copy-all for custom SQL** — the detail panel adds an All tab that aggregates every custom SQL section with a one-click copy of the combined script (`d5ff1a0`)
- **Resizable detail panel** — drag to resize the migration detail panel (`76d0648`)
- **Git fetch before switching** — refresh remote branches on demand from the switch dialog so newly pushed branches show up without leaving the app (`505199a`)

### Changed
- Drive global shortcuts from a single declarative binding table behind one keydown listener instead of scattered handlers (`ac8efa9`, `86f3b92`)
- Render the SQL tabs from a single section descriptor (`d31314d`)
- Surface the raw `list_migrations` error in the empty-table area instead of silently showing "No migrations found", so failures can be copied and reported (`4247c42`)
- Auto-mark releases as pre-release when the tag contains a `-` (`80c7a91`)

### Fixed
- Stop corrupting `PATH` on Windows when invoking `dotnet ef` — enrich `PATH` with the per-user dotnet tools dirs using the `;` separator (`4247c42`)
- Stop mouse hover from hijacking the keyboard switch target in the branch dialog (`0c5c8f3`)
- Keep the branch-dialog filter focusable so the on-open autofocus lands (`7066ff0`)
- Stop overclaiming ARIA on the table keyboard cursor (`b558051`)
- Keep the SQL copy button on-screen by scrolling long lines, and fix SQL horizontal overflow (`e0bb299`, `76d0648`)
- Cap the branch-switch dialog height instead of filling the viewport (`b8d2a96`)
- Move the Fetch button out of the dialog close-X corner (`1744968`)
- Harden the git fetch path against edge cases found in review (`82c8f60`)

### Internal
- Add Rust unit tests, a vitest + React Testing Library setup, and a CI workflow; cover `enrich_path` and local-branch-name helpers (`936c6bc`, `07aaf18`)
- Rewrite the README and add architecture + releasing docs (`172f4c2`)

## [1.1.0] - 2026-05-22

### Added
- **Auto-detect startup project** — detects the EF startup project from the solution and surfaces design-time `DbContext` errors with actionable messages (`a2e2e7f`)

### Fixed
- Stop corrupting `PATH` on Windows when invoking `dotnet ef` (`1abe332`)
- Send flat args to `save_saved_project` / `update_saved_project` so saves no longer fail with a deserialization error (`77ce85c`)
- Use numeric pre-release id for the Windows MSI bundler so releases publish successfully (`acb14ac`)

## [1.0.0] - 2026-05-15

### Added
- **Frontend rewrite on React + TanStack Query + shadcn/ui** — replaces the vanilla JS UI with a component-based architecture, better state management, and a consistent design system (`a076637`)
- **Managed branch switching** — compares target-branch migrations, rolls back to the latest common migration, checks out the branch, and updates the database in one flow (`a59a135`)
- **Per-step operation feedback** — overlay surfaces progress for each step of long-running operations with a working cancel button (`b29ed1b`)
- **Structured EF Core error dialog** — parses `dotnet ef` failures into a dialog with cause, suggested fix, and copy-to-clipboard (`d28374f`)
- **Drift detection on window focus** — re-checks migration state when the app regains focus so external changes are caught immediately (`d28374f`)
- **Toast notifications** — shared toast utility surfacing success/failure for project mutations, copy actions, and other operations (`c5bb98b`, `13d2c63`)
- **Remote branch surfacing** — `SwitchBranchDialog` now lists remote branches alongside local ones with hover-select (`8bf65d6`)
- **Code splitting** — dialogs, `SettingsSheet`, and `HotkeysDialog` lazy-loaded to shrink initial bundle (`5322955`)

### Changed
- Default to a sensible minimum window size on launch (`8bf65d6`)
- Track `Cargo.lock` for reproducible Tauri builds (`abd9f5a`)
- Bump `tauri-plugin-dialog`, Tauri API/CLI, `lucide-react`; resolve rollup advisory (`97dc0d9`, `f04d254`)

### Fixed
- Clamp dialogs to viewport and wrap footer buttons on narrow widths (`200f042`, `32abaa6`)
- Rewrite `SwitchBranchDialog` layout so the footer is always visible (`8b8cb4f`)
- Add error-context prefixes to all bare `errToast()` calls (`05c3b53`)

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
