# Test Fixtures for Migration Squashing

This directory contains real migration files used for testing the migration squashing functionality.

## Migration Files

The fixtures simulate typical migration scenarios:

### 1. `20240101000000_migration1_create_procedure.cs`
- **Purpose**: Initial creation of the `GetProducts` stored procedure
- **Up Method**: Creates `GetProducts` procedure that selects all columns from Products table
- **Down Method**: Drops the `GetProducts` procedure

### 2. `20240102000000_migration2_alter_procedure.cs`
- **Purpose**: Updates the `GetProducts` stored procedure with filtering logic
- **Up Method**: Alters `GetProducts` to select only Id, Name, Price where Price > 0
- **Down Method**: Reverts `GetProducts` back to the original version (select all)

### 3. `20240103000000_migration3_create_second_procedure.cs`
- **Purpose**: Creates a new `UpdateProductPrice` stored procedure
- **Up Method**: Creates `UpdateProductPrice` procedure to update product prices
- **Down Method**: Drops the `UpdateProductPrice` procedure

### 4. `20240104000000_rejectTypeMigration.cs`
- **Purpose**: Real-world migration with table creation, indexes, foreign keys, and INSERT statement
- **Up Method**: Creates RejectType table with schema, adds columns, indexes, foreign keys, and inserts seed data
- **Down Method**: Drops foreign keys, indexes, table, and columns
- **Testing Focus**: Validates that regular SQL (INSERT, CREATE TABLE, etc.) is correctly ignored during procedure squashing

## Expected Squash Results

When these migrations are squashed, the system should:

1. **For `GetProducts` procedure**:
   - Use the latest Up method (ALTER from migration2)
   - Use the oldest Down method (DROP from migration1)

2. **For `UpdateProductPrice` procedure**:
   - Use the Up method (CREATE from migration3)
   - Use the Down method (DROP from migration3)

## File Naming Convention

Migration files are prefixed with timestamps (`YYYYMMDDHHMMSS_`) to ensure they are processed in chronological order. This matches the standard Entity Framework Core migration naming convention.

