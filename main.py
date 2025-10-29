#!/usr/bin/env python3
"""
ezMIgrations - Entity Framework Core Migration Squashing Tool

This tool helps squash multiple EF Core migrations into a single migration
while preserving custom stored procedures.
"""
import re
import subprocess
import sys
from pathlib import Path
from typing import List, Optional, Tuple, Dict
from classes.StoreProcedure import StoredProcedure

try:
    from config_manager import ConfigManager
    from migration_squasher import MigrationSquasher
    HAS_SQUASHER = True
except ImportError:
    HAS_SQUASHER = False


class MigrationParser:
    def __init__(self) -> None:
        self.up_method_pattern = re.compile(
            r"protected override void Up\([^)]*\)\s*\{(.*?)\}", re.DOTALL
        )
        self.down_method_pattern = re.compile(
            r"protected override void Down\([^)]*\)\s*\{(.*?)\}", re.DOTALL
        )

    def extract_up_down_methods(self, file_content: str) -> Optional[Tuple[str, str]]:
        up_method = self.up_method_pattern.search(file_content)
        down_method = self.down_method_pattern.search(file_content)

        if not up_method or not down_method:
            return None

        return up_method.group(1), down_method.group(1)


class SqlExtractor:
    def __init__(self) -> None:
        self.sql_pattern = re.compile(
            r'migrationBuilder\.Sql\(\s*(@?"(?:[^"\\]|\\.|"(?=[^)]))+"(?:\s*\+\s*"?[^"]*")*)\s*\)',
            re.DOTALL,
        )
        self.stored_procedures: Dict[str, StoredProcedure] = {}

    def is_stored_procedure(self, sql_content: str) -> bool:
        return "ALTER PROCEDURE" in sql_content or "CREATE PROCEDURE" in sql_content

    def extract_sql_from_content(self, content: str) -> List[str]:
        return self.sql_pattern.findall(content)

    def process_custom_sql(self, up_content: str, down_content: str) -> None:
        up_sql_matches = self.extract_sql_from_content(up_content)
        down_sql_matches = self.extract_sql_from_content(down_content)

        for sql in up_sql_matches:
            if self.is_stored_procedure(sql):
                self._handle_stored_procedure(sql, is_up_method=True)

        for sql in down_sql_matches:
            # Handle both stored procedures (ALTER/CREATE) and DROP PROCEDURE statements
            if self.is_stored_procedure(sql):
                self._handle_stored_procedure(sql, is_up_method=False)
            elif "DROP PROCEDURE" in sql.upper():
                self._handle_stored_procedure(sql, is_up_method=False)

    def _handle_stored_procedure(self, sql_content: str, is_up_method: bool) -> None:
        # Extract procedure name from either CREATE/ALTER or DROP statement
        proc_name = self._extract_procedure_name(sql_content)
        if not proc_name:
            proc_name = self._extract_procedure_name_from_drop(sql_content)
        
        if proc_name:
            if proc_name not in self.stored_procedures:
                self.stored_procedures[proc_name] = StoredProcedure(proc_name, "", "")

            if is_up_method:
                self.stored_procedures[proc_name].update_up_method(sql_content)
            else:
                self.stored_procedures[proc_name].update_down_method(sql_content)

    def _extract_procedure_name(self, sql_content: str) -> Optional[str]:
        pattern = r"(?:CREATE|ALTER)\s+PROCEDURE\s+(\w+)"
        match = re.search(pattern, sql_content, re.IGNORECASE)
        return match.group(1) if match else None
    
    def _extract_procedure_name_from_drop(self, sql_content: str) -> Optional[str]:
        """Extract procedure name from DROP PROCEDURE statement."""
        pattern = r"DROP\s+PROCEDURE\s+(\w+)"
        match = re.search(pattern, sql_content, re.IGNORECASE)
        return match.group(1) if match else None


class MigrationManager:
    def __init__(self) -> None:
        self.parser = MigrationParser()
        self.sql_extractor = SqlExtractor()

    def get_migrations_directory(self) -> Path:
        while True:
            user_input = input("Please enter the path to the migrations folder: ")
            path = Path(user_input)
            if path.is_dir():
                return path
            print("Invalid path. Please try again.")

    def get_migration_files(self, directory: Path) -> List[Path]:
        migration_files = [
            p
            for p in directory.iterdir()
            if p.is_file() and p.suffix == ".cs" and not p.name.endswith(".Designer.cs")
        ]

        if not migration_files:
            raise ValueError("No migration files found in the specified directory.")

        # Sort migration files by name to ensure chronological processing
        return sorted(migration_files, key=lambda p: p.name)

    def process_migration_files(self, migration_files: List[Path]) -> None:
        for file_path in migration_files:
            with file_path.open("r", encoding="utf-8") as f:
                content = f.read()
                up_down_methods = self.parser.extract_up_down_methods(content)

                if up_down_methods:
                    up_content, down_content = up_down_methods
                    self.sql_extractor.process_custom_sql(up_content, down_content)

    def run_ef_commands(self, target_migration: Optional[str] = None) -> None:
        commands = [
            "dotnet ef database update",
            "dotnet ef migrations add SquashedMigration",
        ]

        for command in commands:
            try:
                subprocess.run(command.split(), check=True)
                print(f"Successfully executed: {command}")
            except subprocess.CalledProcessError as e:
                print(f"Error executing {command}: {e}")

    def generate_squashed_migration(self) -> str:
        migration_content = "// Squashed Migration\n"
        migration_content += (
            "protected override void Up(MigrationBuilder migrationBuilder)\n{\n"
        )

        for proc in self.sql_extractor.stored_procedures.values():
            if proc.latest_up_method:
                migration_content += f"    // {proc.name}\n"
                migration_content += (
                    f'    migrationBuilder.Sql(@"{proc.latest_up_method}");\n\n'
                )

        migration_content += "}\n\n"
        migration_content += (
            "protected override void Down(MigrationBuilder migrationBuilder)\n{\n"
        )

        for proc in self.sql_extractor.stored_procedures.values():
            if proc.oldest_down_method:
                migration_content += f"    // {proc.name}\n"
                migration_content += (
                    f'    migrationBuilder.Sql(@"{proc.oldest_down_method}");\n\n'
                )

        migration_content += "}"
        return migration_content


def main() -> None:
    """Main entry point for the application."""
    import argparse
    
    parser = argparse.ArgumentParser(
        description="ezMIgrations - EF Core Migration Squashing Tool",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Analyze migrations without making changes
  python main.py --analyze

  # Squash migrations after a specific migration
  python main.py --squash --target InitialMigration --migrations-dir ./Migrations

  # Dry run to see what would happen
  python main.py --squash --target InitialMigration --dry-run

  # Use custom config file
  python main.py --squash --target InitialMigration --config custom_config.yaml
        """
    )
    
    parser.add_argument(
        "--analyze",
        action="store_true",
        help="Analyze migrations and show stored procedures (no changes)"
    )
    
    parser.add_argument(
        "--squash",
        action="store_true",
        help="Perform migration squashing workflow"
    )
    
    parser.add_argument(
        "--target",
        type=str,
        help="Target migration to roll back to (required for --squash)"
    )
    
    parser.add_argument(
        "--migrations-dir",
        type=str,
        help="Path to migrations directory (will prompt if not provided)"
    )
    
    parser.add_argument(
        "--config",
        type=str,
        default="config.yaml",
        help="Path to configuration file (default: config.yaml)"
    )
    
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Show what would happen without making changes"
    )
    
    parser.add_argument(
        "--name",
        type=str,
        help="Name for the squashed migration (overrides config)"
    )
    
    args = parser.parse_args()
    
    # Validate arguments
    if args.squash and not args.target:
        parser.error("--squash requires --target to be specified")
    
    if not args.analyze and not args.squash:
        parser.error("Must specify either --analyze or --squash")
    
    try:
        # Get migrations directory
        if args.migrations_dir:
            migrations_dir = Path(args.migrations_dir)
        else:
            migrations_dir = get_migrations_directory()
        
        if not migrations_dir.is_dir():
            print(f"Error: Directory not found: {migrations_dir}")
            sys.exit(1)
        
        # Analyze mode - just show stored procedures
        if args.analyze:
            analyze_migrations(migrations_dir)
            return
        
        # Squash mode - full workflow
        if args.squash:
            if not HAS_SQUASHER:
                print("Error: Missing dependencies for squashing workflow")
                print("Please ensure config_manager.py and migration_squasher.py are available")
                sys.exit(1)
            
            # Load configuration
            config_path = Path(args.config)
            config = ConfigManager(config_path if config_path.exists() else None)
            
            # Override with CLI arguments
            if args.dry_run:
                config.config["options"]["dry_run"] = True
            
            # Perform squashing
            squasher = MigrationSquasher(config, migrations_dir)
            success = squasher.squash_migrations(
                target_migration=args.target,
                squashed_name=args.name
            )
            
            sys.exit(0 if success else 1)
    
    except KeyboardInterrupt:
        print("\n\nOperation cancelled by user.")
        sys.exit(1)
    except Exception as e:
        print(f"Error: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)


def get_migrations_directory() -> Path:
    """Prompt user for migrations directory."""
    while True:
        user_input = input("Please enter the path to the migrations folder: ")
        path = Path(user_input)
        if path.is_dir():
            return path
        print("Invalid path. Please try again.")


def analyze_migrations(migrations_dir: Path) -> None:
    """Analyze migrations and display stored procedures."""
    print(f"\n{'='*60}")
    print(f"Migration Analysis")
    print(f"{'='*60}")
    print(f"Migrations directory: {migrations_dir}\n")
    
    manager = MigrationManager()
    migration_files = manager.get_migration_files(migrations_dir)
    
    print(f"Found {len(migration_files)} migration files:")
    for mig in migration_files:
        print(f"  - {mig.name}")
    print()
    
    manager.process_migration_files(migration_files)
    
    print(f"Found {len(manager.sql_extractor.stored_procedures)} stored procedures:\n")
    
    if manager.sql_extractor.stored_procedures:
        for name, proc in manager.sql_extractor.stored_procedures.items():
            print(f"Procedure: {name}")
            print(f"  Latest Up: {proc.latest_up_method[:80]}...")
            print(f"  Oldest Down: {proc.oldest_down_method[:80]}...")
            print()
        
        print("\nGenerated squashed migration content:")
        print("="*60)
        squashed_migration = manager.generate_squashed_migration()
        print(squashed_migration)
    else:
        print("No stored procedures found in migrations.")
    
    print(f"\n{'='*60}")


if __name__ == "__main__":
    main()
