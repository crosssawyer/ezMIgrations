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
