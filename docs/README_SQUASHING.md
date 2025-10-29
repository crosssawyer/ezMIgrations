# Migration Squashing Workflow

## Overview

ezMIgrations now supports a complete migration squashing workflow that:

1. Rolls back the database to a target migration
2. Captures custom stored procedures from migrations
3. Deletes old migration files (with backup)
4. Generates a new squashed migration
5. Injects the captured stored procedures into the new migration

## Installation

```bash
pip install -r requirements.txt
```

## Configuration

Create or edit `config.yaml`:

```yaml
# Entity Framework Core settings
ef_core:
  add_command: "dotnet ef migrations add {migration_name}"
  update_command: "dotnet ef database update {migration_name}"
  remove_command: "dotnet ef migrations remove"

# Squashed migration settings
squashed_migration:
  name: "SquashedMigration"
  backup_migrations: true
  backup_directory: "../migrations_backup"

# Processing options
options:
  confirm_deletion: true
  rollback_database: true
  dry_run: false
```

## Usage

### Analyze Mode (Safe - No Changes)

Analyze your migrations to see what stored procedures would be captured:

```bash
python main.py --analyze --migrations-dir ./Migrations
```

### Dry Run Mode (Preview Changes)

See exactly what would happen without making any changes:

```bash
python main.py --squash --target InitialMigration --migrations-dir ./Migrations --dry-run
```

### Full Squashing Workflow

Squash all migrations after a target migration:

```bash
python main.py --squash --target InitialMigration --migrations-dir ./Migrations
```

### Custom Migration Name

Override the squashed migration name:

```bash
python main.py --squash --target InitialMigration --name MySquashedMigration --migrations-dir ./Migrations
```

### Custom Config File

Use a different configuration file:

```bash
python main.py --squash --target InitialMigration --config custom_config.yaml --migrations-dir ./Migrations
```

## Workflow Steps

When you run the squashing workflow, it performs these steps:

### 1. **Analysis**

- Scans migrations directory
- Identifies migrations after the target
- Extracts stored procedures from each migration

### 2. **Confirmation**

- Shows which migrations will be squashed
- Prompts for user confirmation (unless `confirm_deletion: false`)

### 3. **Database Rollback**

- Rolls back database to target migration using EF Core
- Command: `dotnet ef database update {target_migration}`

### 4. **Backup**

- Creates timestamped backup of migration files
- Includes both `.cs` and `.Designer.cs` files
- Backup location: `../migrations_backup/backup_YYYYMMDD_HHMMSS/`

### 5. **Deletion**

- Removes migration files after the target
- Deletes both `.cs` and `.Designer.cs` files

### 6. **Generation**

- Creates new migration using EF Core
- Command: `dotnet ef migrations add {squashed_name}`

### 7. **Injection**

- Parses the newly generated migration
- Injects captured stored procedures into Up method
- Injects rollback procedures into Down method

## Example Workflow

```bash
# 1. First, analyze to see what will be captured
python main.py --analyze --migrations-dir ./Migrations

# Output:
# Found 5 migration files:
#   - 20240101_InitialMigration.cs
#   - 20240102_AddProducts.cs
#   - 20240103_CreateGetProductsProc.cs
#   - 20240104_AlterGetProductsProc.cs
#   - 20240105_CreateUpdateProductsProc.cs
#
# Found 2 stored procedures:
#   - GetProducts
#   - UpdateProducts

# 2. Do a dry run to see what would happen
python main.py --squash --target InitialMigration --dry-run --migrations-dir ./Migrations

# Output:
# Found 4 migrations to squash:
#   - 20240102_AddProducts.cs
#   - 20240103_CreateGetProductsProc.cs
#   - 20240104_AlterGetProductsProc.cs
#   - 20240105_CreateUpdateProductsProc.cs
#
# DRY RUN MODE - No changes will be made
# Would perform the following actions:
#   1. Roll back database to 'InitialMigration'
#   2. Backup migrations to '../migrations_backup'
#   3. Delete 4 migration files
#   4. Generate new migration 'SquashedMigration'
#   5. Inject 2 stored procedures

# 3. If everything looks good, run the actual squash
python main.py --squash --target InitialMigration --migrations-dir ./Migrations

# Confirm when prompted:
# Proceed with squashing 4 migrations? (yes/no): yes
```

## Generated Migration Structure

The squashed migration will look like:

```csharp
public partial class SquashedMigration : Migration
{
    protected override void Up(MigrationBuilder migrationBuilder)
    {
        // Auto-generated schema changes from EF Core
        migrationBuilder.CreateTable(...);
        migrationBuilder.AddColumn(...);

        // Injected stored procedures
        // GetProducts
        migrationBuilder.Sql(@"
            ALTER PROCEDURE GetProducts
            AS
            BEGIN
                SELECT Id, Name, Price FROM Products WHERE Price > 0
            END;
        ");

        // UpdateProducts
        migrationBuilder.Sql(@"
            CREATE PROCEDURE UpdateProducts
                @ProductId INT,
                @NewPrice DECIMAL(18,2)
            AS
            BEGIN
                UPDATE Products SET Price = @NewPrice WHERE Id = @ProductId
            END;
        ");
    }

    protected override void Down(MigrationBuilder migrationBuilder)
    {
        // GetProducts
        migrationBuilder.Sql(@"DROP PROCEDURE GetProducts;");

        // UpdateProducts
        migrationBuilder.Sql(@"DROP PROCEDURE UpdateProducts;");

        // Auto-generated rollback from EF Core
        migrationBuilder.DropColumn(...);
        migrationBuilder.DropTable(...);
    }
}
```

## Safety Features

- **Dry Run Mode**: Preview changes without modifying anything
- **Automatic Backups**: All deleted migrations are backed up with timestamps
- **Confirmation Prompts**: Asks for confirmation before destructive operations
- **Database Rollback**: Ensures database is in the correct state before squashing
- **Error Handling**: Aborts on errors to prevent partial squashing

## Configuration Options

### Database Commands

Customize EF Core commands for different project structures:

```yaml
ef_core:
  # For projects in subdirectories
  add_command: "dotnet ef migrations add {migration_name} --project ./MyProject.Data"
  update_command: "dotnet ef database update {migration_name} --project ./MyProject.Data"

  # For different contexts
  add_command: "dotnet ef migrations add {migration_name} --context MyDbContext"
```

### Safety Options

```yaml
options:
  # Skip confirmation prompt (careful!)
  confirm_deletion: false

  # Skip database rollback (not recommended)
  rollback_database: false

  # Always do dry run
  dry_run: true
```

### Backup Options

```yaml
squashed_migration:
  # Disable backups (not recommended)
  backup_migrations: false

  # Custom backup location
  backup_directory: "../archived_migrations"
```

## Troubleshooting

### "Target migration not found"

Ensure the target migration name matches exactly (case-sensitive).

### "Failed to roll back database"

- Verify EF Core tools are installed: `dotnet ef --version`
- Check database connection string
- Ensure target migration exists in the database

### "Could not find generated migration file"

- Check EF Core command output for errors
- Verify migrations directory path
- Ensure you have write permissions

### "Failed to inject stored procedures"

- Check the generated migration file format
- Verify Up/Down methods exist in the generated file
- Report as an issue if problem persists

## Best Practices

1. **Always use --analyze first** to understand what will be captured
2. **Use --dry-run** to preview changes before executing
3. **Commit your code** before running squash (or verify backups are enabled)
4. **Test the squashed migration** on a development database first
5. **Keep backups** until you've verified the squashed migration works
6. **Document the squash** in your version control commit message

## Limitations

- Only captures stored procedures (CREATE, ALTER, DROP PROCEDURE)
- Does not capture other SQL like triggers, views, or functions
- Assumes standard EF Core migration structure
- Requires dotnet CLI and EF Core tools

## Future Enhancements

- Support for triggers and views
- Support for custom SQL functions
- Interactive migration selection
- Migration diff visualization
- Rollback capability for the squash operation
