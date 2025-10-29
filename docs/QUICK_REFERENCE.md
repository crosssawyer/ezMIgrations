# ezMIgrations Quick Reference

## Installation

```bash
pip install -r requirements.txt
```

## Commands

### Analyze Migrations (Safe - No Changes)

```bash
python main.py --analyze --migrations-dir ./Migrations
```

Shows what stored procedures exist in your migrations without making any changes.

### Dry Run Squashing

```bash
python main.py --squash --target YourTargetMigration --migrations-dir ./Migrations --dry-run
```

Preview exactly what would happen without making any changes.

### Actual Squashing

```bash
python main.py --squash --target YourTargetMigration --migrations-dir ./Migrations
```

Performs the complete squashing workflow.

## What Gets Squashed?

The tool captures and preserves:

- ✅ **CREATE PROCEDURE** statements
- ✅ **ALTER PROCEDURE** statements
- ✅ **DROP PROCEDURE** statements
- ✅ Procedure evolution (keeps latest Up, oldest Down)

The tool ignores (not included in squash):

- ❌ Regular SQL (INSERT, UPDATE, DELETE)
- ❌ CREATE TABLE / ALTER TABLE
- ❌ CREATE INDEX
- ❌ Triggers, Views, Functions

## Workflow

1. **Rollback** database to target migration
2. **Extract** stored procedures from migrations after target
3. **Backup** migration files (timestamped)
4. **Delete** migration files after target
5. **Generate** new migration via `dotnet ef migrations add`
6. **Inject** captured stored procedures into new migration

## Safety

- All operations can be previewed with `--dry-run`
- Automatic backups before deletion
- Confirmation prompts before destructive operations
- Aborts on any error to prevent partial state

## Configuration

Edit `config.yaml`:

```yaml
ef_core:
  add_command: "dotnet ef migrations add {migration_name}"
  update_command: "dotnet ef database update {migration_name}"

squashed_migration:
  name: "SquashedMigration"
  backup_migrations: true
  backup_directory: "../migrations_backup"

options:
  confirm_deletion: true
  rollback_database: true
  dry_run: false
```

## Common Issues

**"Target migration not found"**

- Check migration name spelling (case-sensitive)

**"Failed to roll back database"**

- Verify `dotnet ef` tools are installed
- Check database connection

**No stored procedures found**

- Use `--analyze` to verify procedures exist
- Only procedures are captured, not regular SQL

## Examples

```bash
# See what you have
python main.py --analyze --migrations-dir ./Migrations

# Preview squashing
python main.py --squash --target InitialMigration --dry-run --migrations-dir ./Migrations

# Actually squash
python main.py --squash --target InitialMigration --migrations-dir ./Migrations

# Custom migration name
python main.py --squash --target InitialMigration --name MySquash --migrations-dir ./Migrations
```

## Get Help

```bash
python main.py --help
```

See `README_SQUASHING.md` for detailed documentation.
