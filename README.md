# ezMIgrations

A Python helper tool to make squashing Entity Framework migrations much easier by automatically extracting and consolidating custom SQL and stored procedures.

## Features

✅ **Migration Parsing**

- Extracts Up and Down methods from EF migration files
- Handles complex migration structures with proper regex patterns

✅ **Custom SQL Extraction**

- Identifies and extracts custom SQL statements from migrations
- Supports both simple SQL and complex multi-line statements
- Preserves SQL formatting and string concatenation

✅ **Stored Procedure Management**

- Automatically detects stored procedures in migrations
- Tracks the latest Up method and oldest Down method for each procedure
- Consolidates multiple procedure updates into a single migration

✅ **EF Command Integration**

- Runs `dotnet ef database update` commands
- Generates new squashed migration files
- Integrates with existing EF workflow

## Installation

1. Clone the repository:

```bash
git clone https://github.com/yourusername/ezMIgrations.git
cd ezMIgrations
```

2. Set up a virtual environment:

```bash
python -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate
```

3. Install dependencies:

```bash
pip install -r requirements.txt
```

## Usage

1. Run the tool:

```bash
python main.py
```

2. Enter the path to your migrations folder when prompted

3. The tool will:
   - Scan all migration files
   - Extract custom SQL and stored procedures
   - Generate a consolidated migration file
   - Optionally run EF commands

## Architecture

The tool is built with a clean OOP structure:

- **`MigrationParser`**: Extracts Up/Down methods from migration files
- **`SqlExtractor`**: Handles custom SQL detection and stored procedure management
- **`MigrationManager`**: Orchestrates the entire migration squashing process
- **`StoredProcedure`**: Represents stored procedure data with update tracking

## Example

Given migrations with custom SQL like:

```csharp
migrationBuilder.Sql(@"
    CREATE PROCEDURE GetProducts
    AS
    BEGIN
        SELECT * FROM Products
    END;
");
```

The tool will extract this SQL and include it in the final squashed migration, maintaining proper Up/Down method structure.

## Requirements

- Python 3.8+
- .NET Core SDK (for EF commands)
- Entity Framework Core project
