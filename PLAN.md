# ezMigrations - Tauri + .NET Migrations Manager

## Overview
A fast, lightweight Tauri desktop app for managing Entity Framework Core migrations. Rust backend for speed, minimal frontend, direct integration with `dotnet ef` CLI and `git`.

## Architecture

### Tech Stack
- **Backend**: Rust (Tauri v2)
- **Frontend**: Vanilla HTML/CSS/JS (no framework - keep it lightweight)
- **CLI Integration**: Shells out to `dotnet ef` and `git` commands
- **State**: SQLite via `rusqlite` for tracking migration state per project

### Core Data Model

```
Project {
  id, path, name, db_context, startup_project
}

Migration {
  id, project_id, name, timestamp, applied (bool),
  up_sql (custom SQL extracted), down_sql (custom SQL extracted),
  file_path
}

BranchSnapshot {
  id, project_id, branch_name, last_applied_migration
}
```

## Features & Implementation Plan

### Phase 1: Project Scaffolding
1. Initialize Tauri v2 project (Rust + vanilla JS frontend)
2. Set up SQLite database with rusqlite
3. Basic window with project selector UI
4. Tauri commands for project CRUD

### Phase 2: Migration Discovery & Tracking
1. **Scan migrations folder** - Parse `Migrations/` directory for `*.cs` files (exclude `*.Designer.cs`, `*ModelSnapshot.cs`)
2. **Parse migration files** - Extract `Up()` and `Down()` methods via regex in Rust
3. **Extract custom SQL** - Find `migrationBuilder.Sql(...)` calls, capture the SQL strings
4. **Track applied state** - Run `dotnet ef migrations list` and parse output to know which are applied vs pending
5. **Display in UI** - Table showing migration name, status (applied/pending), has custom SQL indicator

### Phase 3: Create & Delete Migrations
1. **Create migration** - Text input for name → runs `dotnet ef migrations add <name>` → refreshes list
2. **Delete migration** - Select migration → runs `dotnet ef migrations remove` (only last) or deletes files for specific ones → refreshes list
3. **Apply migration** - Select target → runs `dotnet ef database update <target>`
4. **Rollback** - Select target → runs `dotnet ef database update <previous_migration>`

### Phase 4: Squash Migrations
1. Select range of migrations to squash
2. Extract all custom SQL from selected migrations (preserve order)
3. Rollback to migration before range
4. Delete migration files in range
5. Create single new migration
6. Inject consolidated custom SQL into the new migration's `Up()` and corresponding `Down()`
7. Apply the squashed migration

### Phase 5: Git Branch Awareness
1. **Watch for branch switches** - Use `git` commands or file watcher on `.git/HEAD`
2. **Snapshot on switch** - Before switching, record current branch + last applied migration in `BranchSnapshot`
3. **Restore on return** - When switching to a branch, check if a snapshot exists → run `dotnet ef database update <snapshot_migration>` to get DB in sync
4. **Notify user** - Show what migrations will be applied/rolled back

### Phase 6: Custom SQL Viewer
1. Side panel showing extracted custom SQL per migration
2. Syntax-highlighted SQL display
3. Copy to clipboard
4. Export all custom SQL to a `.sql` file

## File Structure

```
ezMigrations/
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── src/
│   │   ├── main.rs              # Tauri entry point
│   │   ├── lib.rs               # Module declarations
│   │   ├── db.rs                # SQLite setup & queries
│   │   ├── commands/
│   │   │   ├── mod.rs
│   │   │   ├── project.rs       # Project CRUD commands
│   │   │   ├── migration.rs     # Migration operations
│   │   │   ├── squash.rs        # Squash logic
│   │   │   └── git.rs           # Git branch tracking
│   │   ├── parser.rs            # C# migration file parser
│   │   ├── dotnet.rs            # dotnet ef CLI wrapper
│   │   └── git_watcher.rs       # Git HEAD file watcher
├── src/
│   ├── index.html               # Main UI
│   ├── styles.css               # Minimal dark theme
│   └── main.js                  # Frontend logic + Tauri invoke calls
├── .gitignore
└── README.md
```

## UI Design
- Dark theme, single window
- Left sidebar: project list
- Main area: migration table with status indicators
- Top bar: create migration input, refresh, branch indicator
- Right panel (toggle): custom SQL viewer
- Bottom bar: command output / logs
- Modal dialogs for squash wizard and confirmations

## Key Rust Dependencies
- `tauri` v2
- `rusqlite` - SQLite
- `regex` - Parsing C# migration files
- `serde` / `serde_json` - Serialization
- `notify` - File system watcher (for git HEAD changes)
