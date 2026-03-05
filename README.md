# ezMigrations

A fast, lightweight desktop app for managing .NET Entity Framework Core migrations. Built with Tauri (Rust) and vanilla JS.

## Features

- **Migration tracking** — See all migrations with applied/pending status at a glance
- **Custom SQL capture** — Automatically extracts `migrationBuilder.Sql()` calls from migration files
- **Squash migrations** — Consolidate multiple migrations into one while preserving custom SQL
- **Git branch awareness** — Watches for branch changes and prompts to update the database accordingly
- **Out-of-sync detection** — Detects foreign migrations left over from other branches, highlights them in the list, and offers a one-click revert
- **Stable migration** — Pin a migration as a safe rollback point for branch switches
- **Create / delete migrations** — One-click migration management through `dotnet ef`
- **Update database** — Apply up to any migration or update to latest
- **SQL script generation** — Generate SQL scripts between migration points
- **Multi-project support** — Save and switch between multiple EF projects from the settings panel
- **Preferences** — Configure notifications (e.g. disable branch-change prompts)
- **Keyboard shortcuts** — `Ctrl+N` new, `Ctrl+R` refresh, `Ctrl+F` filter, `?` help

## Prerequisites

- [.NET SDK](https://dotnet.microsoft.com/download) with `dotnet-ef` tool installed
- [Node.js](https://nodejs.org/) (for frontend build)
- [Rust](https://rustup.rs/) (for Tauri backend)

```bash
# Install the EF Core CLI tool if you haven't
dotnet tool install --global dotnet-ef
```

## Development

```bash
# Install frontend dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

## Architecture

```
src-tauri/           Rust backend (Tauri v2)
  src/
    commands.rs      Tauri command handlers
    dotnet.rs        dotnet ef CLI wrapper
    git.rs           Git operations + branch detection
    parser.rs        C# migration file parser
    process.rs       Cross-platform subprocess helper
    state.rs         App state & config models
src/                 Frontend (vanilla JS)
  main.js           App logic + UI
  style.css         Dark theme styling
index.html           Entry point
```

## Releasing

The project uses a GitHub Actions workflow to build standalone executables for Linux, Windows, and macOS and publish them as a GitHub Release.

### Option 1: Tag push (recommended)

Create and push a version tag from your local machine:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The tag must start with `v` (e.g. `v0.1.0`, `v1.2.3`).

### Option 2: Manual dispatch

1. Go to the repository on GitHub
2. Navigate to **Actions** > **Release**
3. Click **Run workflow**
4. Enter the version number (e.g. `0.2.0` — no `v` prefix needed)
5. Click **Run workflow**

This will create the tag automatically and trigger the full release.

### What the pipeline does

1. Builds native installers on each platform using Tauri:
   - **Linux** — `.deb` and `.AppImage`
   - **Windows** — `.msi` and `.exe` (NSIS)
   - **macOS** — `.dmg`
2. Creates a GitHub Release tagged with the version
3. Attaches all installers to the release
4. Auto-generates release notes from commit history

### Downloading a release

Go to the [Releases](../../releases) page and download the installer for your platform.
