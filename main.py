import re
import subprocess
from pathlib import Path
from typing import List, Optional, Tuple, Dict
from classes.StoreProcedure import StoredProcedure


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
            if self.is_stored_procedure(sql):
                self._handle_stored_procedure(sql, is_up_method=False)

    def _handle_stored_procedure(self, sql_content: str, is_up_method: bool) -> None:
        proc_name = self._extract_procedure_name(sql_content)
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

        return migration_files

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
            migration_content += f"    // {proc.name}\n"
            migration_content += (
                f'    migrationBuilder.Sql(@"{proc.latest_up_method}");\n\n'
            )

        migration_content += "}\n\n"
        migration_content += (
            "protected override void Down(MigrationBuilder migrationBuilder)\n{\n"
        )

        for proc in self.sql_extractor.stored_procedures.values():
            migration_content += f"    // {proc.name}\n"
            migration_content += (
                f'    migrationBuilder.Sql(@"{proc.oldest_down_method}");\n\n'
            )

        migration_content += "}"
        return migration_content


def main() -> None:
    manager = MigrationManager()

    try:
        migrations_dir = manager.get_migrations_directory()
        migration_files = manager.get_migration_files(migrations_dir)

        print(f"Found {len(migration_files)} migration files to process...")
        manager.process_migration_files(migration_files)

        print(f"Found {len(manager.sql_extractor.stored_procedures)} stored procedures")

        squashed_migration = manager.generate_squashed_migration()
        print("\nGenerated squashed migration:")
        print(squashed_migration)

    except Exception as e:
        print(f"Error: {e}")


if __name__ == "__main__":
    main()
