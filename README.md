# ezMigrations

A desktop app for managing .NET Entity Framework Core migrations.

ezMigrations wraps `dotnet ef` in a focused UI that understands your git branches, surfaces drift the moment it appears, and preserves the things `dotnet ef migrations` quietly throws away when you squash. Built with Tauri v2 and React so it ships as a small native binary on Windows, macOS, and Linux.

> Screenshots: _coming soon_

## Features

### Branch awareness

- **Managed branch switching** — pick a branch, and the app diffs migrations against your current HEAD, rolls the database back to the latest common migration, checks out the branch, and brings the database forward in one guided flow.
- **Foreign migration detection** — when migrations applied in another branch show up as `Applied` after a `Pending`, ezMigrations flags them with a red border, a source-branch label, and a one-click "Revert Foreign" button.
- **Branch-change prompts** — a file watcher on `.git/HEAD` notices when you switch branches outside the app and offers to reconcile the database. Configurable in Preferences.
- **Stable migration pin** — mark any migration as the safe rollback point used by automated branch switches.
- **Remote branches in the picker** — local and remote branches are listed together in the switch dialog, with an in-dialog **Fetch** button to pull newly pushed branches without leaving the app. The dialog is fully keyboard-drivable.

### Migration management

- **Custom SQL preservation on squash** — the parser extracts every `migrationBuilder.Sql(...)` call from your migration files, deduplicates by content, and reinjects them into the squashed migration in the right order (newest wins for `Up`, original wins for `Down`). Squashing in raw `dotnet ef` loses these; ezMigrations does not.
- **Create, remove, and apply migrations** — all the usual `dotnet ef` operations from the toolbar, with a working cancel button on every long-running command.
- **Update database to any point** — apply forward, roll back, or jump to a specific migration.
- **Generate SQL scripts** — produce a script between any two migration points.
- **SQL detail panel** — inspect a migration's Up, Down, and custom SQL in a resizable panel, with an **All** tab and copy-all button for the combined custom SQL.
- **Drift detection** — banner appears when pending migrations are detected, and the app re-checks state every time the window regains focus.

### Productivity

- **Structured EF error dialog** — `dotnet ef` failures are parsed into a dialog with a plain-English cause, a suggested fix, and copy-to-clipboard for the raw output. Common misconfigurations (wrong startup project, mismatched migrations assembly) get specific guidance.
- **Auto-detected startup project** — ezMigrations reads your solution and picks the right startup project for design-time `DbContext` discovery automatically.
- **Per-step operation feedback** — long-running flows like managed branch switching show a phase-by-phase overlay so you know exactly which step is running.
- **Multi-project support** — save multiple EF projects and switch between them from the settings panel.
- **Search, filter, and shortcuts** — `Ctrl+N` new, `Ctrl+R` refresh, `Ctrl+F` filter, `Esc` to clear, `?` for the full hotkey list. The migrations table is fully keyboard-navigable with arrow keys.
- **Toast notifications and preferences** — non-intrusive feedback for mutations and copy actions; toggle branch-change prompts in Preferences.

## Install

Download the latest installer for your platform from the [Releases](../../releases) page.

| Platform | Installer |
| --- | --- |
| Windows | `.msi` (and `.exe` NSIS) |
| macOS | `.dmg` |
| Linux | `.deb` and `.AppImage` |

### Prerequisites

ezMigrations drives the EF Core CLI, so you need the .NET SDK and the `dotnet-ef` tool installed on your machine:

```bash
# .NET SDK: https://dotnet.microsoft.com/download
dotnet tool install --global dotnet-ef
```

On macOS, the app will look for `dotnet` in the usual locations (`~/.dotnet/tools`, `/usr/local/share/dotnet`, `/opt/homebrew/bin`) even when launched from Finder.

## Quick start

1. Open ezMigrations.
2. Click **Add project**, point it at your migrations project (`.csproj`), and optionally set a startup project and DbContext.
3. The migration list loads and the app starts watching the project's git repository and migrations folder for changes.

From here, new migrations, branch switches, and database updates run through the toolbar. Press `?` for the full hotkey list.

## Development

```bash
# Install frontend dependencies
npm install

# Run in development (starts Vite + Tauri together)
npm run tauri dev

# Build a production binary for your platform
npm run tauri build
```

You'll need [Node.js](https://nodejs.org/) and [Rust](https://rustup.rs/) for local development, plus the platform-specific Tauri prerequisites listed in [Tauri's setup docs](https://tauri.app/start/prerequisites/).

## Architecture

The Rust backend (`src-tauri/`) handles all interaction with `dotnet ef`, git, and the file system, exposing Tauri commands consumed by a React + TanStack Query frontend (`src/`). UI components follow the shadcn/ui pattern and the app uses code splitting so dialogs and settings panels load lazily.

For a deeper walkthrough of the modules, command surface, state model, and frontend data flow, see [docs/architecture.md](docs/architecture.md).

## Releasing

Releases are built by a GitHub Actions workflow that produces installers for Linux, Windows, and macOS and attaches them to a GitHub Release. The pipeline triggers on any tag matching `v*`, or via manual workflow dispatch.

For the full release procedure, version-bumping checklist, and troubleshooting notes, see [docs/releasing.md](docs/releasing.md).
