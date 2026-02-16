# ezMIgrations
A python helper to make squashing migrations in EF much easier.


## Features

- Captures custom sql in migrations. (Done)
- Captures stored procedures (In Progress)
    - Latest Up is kept (In Progress)
    - Oldest Down is Kept (In Progress)
- Runs ef update back to last release migration (In Progress)
- Creates new ef migration file and pastes in CustomSql (In Progress)


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