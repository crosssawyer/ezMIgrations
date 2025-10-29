"""
Complete migration squashing workflow.
Handles rollback, deletion, SQL capture, and injection.
"""
import re
import subprocess
import shutil
from pathlib import Path
from typing import List, Optional, Tuple, Dict
from datetime import datetime

from classes.StoreProcedure import StoredProcedure
from config_manager import ConfigManager


class MigrationSquasher:
    """Handles the complete migration squashing workflow."""
    
    def __init__(self, config: ConfigManager, migrations_dir: Path):
        """
        Initialize the migration squasher.
        
        Args:
            config: Configuration manager instance
            migrations_dir: Path to migrations directory
        """
        self.config = config
        self.migrations_dir = migrations_dir
        self.stored_procedures: Dict[str, StoredProcedure] = {}
        
        # Patterns for parsing
        self.up_pattern = re.compile(
            r"protected override void Up\([^)]*\)\s*\{(.*?)\}", re.DOTALL
        )
        self.down_pattern = re.compile(
            r"protected override void Down\([^)]*\)\s*\{(.*?)\}", re.DOTALL
        )
        self.sql_pattern = re.compile(
            r'migrationBuilder\.Sql\(\s*(@?"(?:[^"\\]|\\.|"(?=[^)]))+"(?:\s*\+\s*"?[^"]*")*)\s*\)',
            re.DOTALL,
        )
        self.namespace_pattern = re.compile(r'namespace\s+([\w\.]+)')
        self.class_pattern = re.compile(r'public partial class\s+(\w+)')
    
    def squash_migrations(
        self,
        target_migration: str,
        squashed_name: Optional[str] = None
    ) -> bool:
        """
        Complete workflow to squash migrations.
        
        Args:
            target_migration: Name of migration to roll back to
            squashed_name: Optional name for the squashed migration
            
        Returns:
            True if successful, False otherwise
        """
        squashed_name = squashed_name or self.config.squashed_migration_name
        
        print(f"\n{'='*60}")
        print(f"Migration Squashing Workflow")
        print(f"{'='*60}")
        print(f"Target migration: {target_migration}")
        print(f"Squashed migration name: {squashed_name}")
        print(f"Migrations directory: {self.migrations_dir}")
        print(f"Dry run mode: {self.config.dry_run}")
        print(f"{'='*60}\n")
        
        try:
            # Step 1: Get migrations to squash
            migrations_to_squash = self._get_migrations_after_target(target_migration)
            if not migrations_to_squash:
                print(f"No migrations found after '{target_migration}'")
                return False
            
            print(f"Found {len(migrations_to_squash)} migrations to squash:")
            for mig in migrations_to_squash:
                print(f"  - {mig.name}")
            print()
            
            # Step 2: Extract stored procedures from migrations
            print("Step 1: Extracting stored procedures...")
            self._extract_procedures_from_migrations(migrations_to_squash)
            print(f"Found {len(self.stored_procedures)} stored procedures\n")
            
            if self.config.dry_run:
                print("DRY RUN MODE - No changes will be made\n")
                print("Would perform the following actions:")
                if self.config.rollback_database:
                    print(f"  1. Roll back database to '{target_migration}'")
                if self.config.backup_migrations:
                    print(f"  2. Backup migrations to '{self.config.backup_directory}'")
                print(f"  3. Delete {len(migrations_to_squash)} migration files")
                print(f"  4. Generate new migration '{squashed_name}'")
                print(f"  5. Inject {len(self.stored_procedures)} stored procedures")
                return True
            
            # Step 3: Confirm with user
            if self.config.confirm_deletion:
                response = input(f"\nProceed with squashing {len(migrations_to_squash)} migrations? (yes/no): ")
                if response.lower() != 'yes':
                    print("Squashing cancelled.")
                    return False
            
            # Step 4: Roll back database
            if self.config.rollback_database:
                print(f"\nStep 2: Rolling back database to '{target_migration}'...")
                if not self._rollback_database(target_migration):
                    print("Failed to roll back database. Aborting.")
                    return False
                print("Database rolled back successfully\n")
            
            # Step 5: Backup migrations
            if self.config.backup_migrations:
                print("Step 3: Backing up migrations...")
                self._backup_migrations(migrations_to_squash)
                print("Migrations backed up successfully\n")
            
            # Step 6: Delete migration files
            print("Step 4: Deleting migration files...")
            self._delete_migrations(migrations_to_squash)
            print("Migration files deleted\n")
            
            # Step 7: Generate new migration
            print(f"Step 5: Generating new migration '{squashed_name}'...")
            if not self._generate_new_migration(squashed_name):
                print("Failed to generate new migration. Aborting.")
                return False
            print("New migration generated successfully\n")
            
            # Step 8: Inject stored procedures
            print("Step 6: Injecting stored procedures into new migration...")
            new_migration_file = self._find_migration_file(squashed_name)
            if not new_migration_file:
                print(f"Could not find generated migration file for '{squashed_name}'")
                return False
            
            if not self._inject_procedures_into_migration(new_migration_file):
                print("Failed to inject stored procedures")
                return False
            print("Stored procedures injected successfully\n")
            
            print(f"{'='*60}")
            print(f"Migration squashing completed successfully!")
            print(f"{'='*60}\n")
            
            return True
            
        except Exception as e:
            print(f"Error during squashing: {e}")
            return False
    
    def _get_migrations_after_target(self, target_migration: str) -> List[Path]:
        """Get all migration files after the target migration."""
        all_migrations = sorted([
            p for p in self.migrations_dir.iterdir()
            if p.is_file() and p.suffix == ".cs" and not p.name.endswith(".Designer.cs")
        ], key=lambda p: p.name)
        
        # Find target migration index
        target_idx = None
        for idx, mig in enumerate(all_migrations):
            if target_migration in mig.stem:
                target_idx = idx
                break
        
        if target_idx is None:
            raise ValueError(f"Target migration '{target_migration}' not found")
        
        # Return migrations after target
        return all_migrations[target_idx + 1:]
    
    def _extract_procedures_from_migrations(self, migration_files: List[Path]) -> None:
        """Extract stored procedures from migration files."""
        for file_path in migration_files:
            with file_path.open("r", encoding="utf-8") as f:
                content = f.read()
                up_match = self.up_pattern.search(content)
                down_match = self.down_pattern.search(content)
                
                if up_match and down_match:
                    up_content = up_match.group(1)
                    down_content = down_match.group(1)
                    self._process_custom_sql(up_content, down_content)
    
    def _process_custom_sql(self, up_content: str, down_content: str) -> None:
        """Process custom SQL from Up and Down methods."""
        # Process Up methods
        up_sql_matches = self.sql_pattern.findall(up_content)
        for sql in up_sql_matches:
            if self._is_stored_procedure(sql):
                self._handle_stored_procedure(sql, is_up_method=True)
        
        # Process Down methods
        down_sql_matches = self.sql_pattern.findall(down_content)
        for sql in down_sql_matches:
            if self._is_stored_procedure(sql) or "DROP PROCEDURE" in sql.upper():
                self._handle_stored_procedure(sql, is_up_method=False)
    
    def _is_stored_procedure(self, sql_content: str) -> bool:
        """Check if SQL content is a stored procedure."""
        return "ALTER PROCEDURE" in sql_content or "CREATE PROCEDURE" in sql_content
    
    def _handle_stored_procedure(self, sql_content: str, is_up_method: bool) -> None:
        """Handle a stored procedure extraction."""
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
        """Extract procedure name from CREATE/ALTER PROCEDURE."""
        pattern = r"(?:CREATE|ALTER)\s+PROCEDURE\s+(\w+)"
        match = re.search(pattern, sql_content, re.IGNORECASE)
        return match.group(1) if match else None
    
    def _extract_procedure_name_from_drop(self, sql_content: str) -> Optional[str]:
        """Extract procedure name from DROP PROCEDURE."""
        pattern = r"DROP\s+PROCEDURE\s+(\w+)"
        match = re.search(pattern, sql_content, re.IGNORECASE)
        return match.group(1) if match else None
    
    def _rollback_database(self, target_migration: str) -> bool:
        """Roll back database to target migration."""
        try:
            command = self.config.get_update_command(target_migration)
            result = subprocess.run(
                command.split(),
                check=True,
                capture_output=True,
                text=True
            )
            print(f"  {result.stdout}")
            return True
        except subprocess.CalledProcessError as e:
            print(f"  Error: {e.stderr}")
            return False
    
    def _backup_migrations(self, migrations: List[Path]) -> None:
        """Backup migration files before deletion."""
        backup_dir = self.migrations_dir.parent / self.config.backup_directory
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        backup_path = backup_dir / f"backup_{timestamp}"
        backup_path.mkdir(parents=True, exist_ok=True)
        
        for mig in migrations:
            shutil.copy2(mig, backup_path / mig.name)
            # Also backup Designer file if exists
            designer_file = mig.parent / f"{mig.stem}.Designer.cs"
            if designer_file.exists():
                shutil.copy2(designer_file, backup_path / designer_file.name)
        
        print(f"  Backed up to: {backup_path}")
    
    def _delete_migrations(self, migrations: List[Path]) -> None:
        """Delete migration files."""
        for mig in migrations:
            mig.unlink()
            # Also delete Designer file if exists
            designer_file = mig.parent / f"{mig.stem}.Designer.cs"
            if designer_file.exists():
                designer_file.unlink()
            print(f"  Deleted: {mig.name}")
    
    def _generate_new_migration(self, migration_name: str) -> bool:
        """Generate new migration using EF Core."""
        try:
            command = self.config.get_add_command(migration_name)
            result = subprocess.run(
                command.split(),
                check=True,
                capture_output=True,
                text=True,
                cwd=self.migrations_dir.parent
            )
            print(f"  {result.stdout}")
            return True
        except subprocess.CalledProcessError as e:
            print(f"  Error: {e.stderr}")
            return False
    
    def _find_migration_file(self, migration_name: str) -> Optional[Path]:
        """Find the migration file by name."""
        for file in self.migrations_dir.iterdir():
            if file.is_file() and migration_name in file.stem and not file.name.endswith(".Designer.cs"):
                return file
        return None
    
    def _inject_procedures_into_migration(self, migration_file: Path) -> bool:
        """Inject stored procedures into the migration file."""
        try:
            with migration_file.open("r", encoding="utf-8") as f:
                content = f.read()
            
            # Find Up and Down methods
            up_match = self.up_pattern.search(content)
            down_match = self.down_pattern.search(content)
            
            if not up_match or not down_match:
                print("  Could not find Up/Down methods in migration file")
                return False
            
            # Generate procedure SQL
            up_procedures = self._generate_procedure_sql_for_up()
            down_procedures = self._generate_procedure_sql_for_down()
            
            # Inject into Up method
            up_start, up_end = up_match.span(1)
            up_body = content[up_start:up_end]
            new_up_body = up_body.rstrip() + "\n\n" + up_procedures + "\n        "
            
            # Inject into Down method
            down_start, down_end = down_match.span(1)
            down_body = content[down_start:down_end]
            new_down_body = down_body.rstrip() + "\n\n" + down_procedures + "\n        "
            
            # Replace content
            new_content = (
                content[:up_start] + new_up_body + content[up_end:down_start] +
                new_down_body + content[down_end:]
            )
            
            # Write back
            with migration_file.open("w", encoding="utf-8") as f:
                f.write(new_content)
            
            print(f"  Injected {len(self.stored_procedures)} procedures into {migration_file.name}")
            return True
            
        except Exception as e:
            print(f"  Error injecting procedures: {e}")
            return False
    
    def _generate_procedure_sql_for_up(self) -> str:
        """Generate SQL for Up method."""
        lines = []
        for proc in self.stored_procedures.values():
            if proc.latest_up_method:
                lines.append(f"        // {proc.name}")
                lines.append(f'        migrationBuilder.Sql({proc.latest_up_method});')
                lines.append("")
        return "\n".join(lines)
    
    def _generate_procedure_sql_for_down(self) -> str:
        """Generate SQL for Down method."""
        lines = []
        for proc in self.stored_procedures.values():
            if proc.oldest_down_method:
                lines.append(f"        // {proc.name}")
                lines.append(f'        migrationBuilder.Sql({proc.oldest_down_method});')
                lines.append("")
        return "\n".join(lines)

