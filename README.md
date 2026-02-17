# ezMigrations

A fast, lightweight desktop app for managing .NET Entity Framework Core migrations. Built with Tauri (Rust) and vanilla JS.

## Features

- **Migration tracking** - See all migrations with applied/pending status at a glance
- **Custom SQL capture** - Automatically extracts `migrationBuilder.Sql()` calls from migration files
- **Squash migrations** - Consolidate multiple migrations into one while preserving custom SQL
- **Git branch awareness** - Watches for branch changes and prompts to update the database accordingly
- **Create / delete migrations** - One-click migration management through `dotnet ef`
- **Update database** - Apply up to any migration or update to latest
- **SQL script generation** - Generate SQL scripts between migration points

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
    state.rs         App state models
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

1. Builds a standalone executable on each platform using PyInstaller:
   - `ezmigrations-linux`
   - `ezmigrations-windows.exe`
   - `ezmigrations-macos`
2. Creates a GitHub Release tagged with the version
3. Attaches all three executables to the release
4. Auto-generates release notes from commit history

### Downloading a release

Go to the [Releases](../../releases) page and download the executable for your platform. No Python installation required — the executables are self-contained.
