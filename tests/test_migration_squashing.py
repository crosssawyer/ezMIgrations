"""
Test suite for migration squashing functionality.
Tests the ability to squash multiple migrations and create a new squashed migration.
"""
import pytest
import tempfile
import shutil
from pathlib import Path
from typing import List
import os

from main import MigrationManager, MigrationParser, SqlExtractor, StoredProcedure

# Get the base directory for test fixtures
FIXTURES_DIR = Path(__file__).parent / "fixtures"
MIGRATIONS_DIR = Path(__file__).parent.parent / "migrations"


class TestMigrationParser:
    """Test the MigrationParser class."""

    def test_extract_up_down_methods_success(self):
        """Test successful extraction of Up and Down methods."""
        parser = MigrationParser()
        
        migration_content = """
        public partial class TestMigration : Migration
        {
            protected override void Up(MigrationBuilder migrationBuilder)
            {
                migrationBuilder.CreateTable(name: "Test");
            }

            protected override void Down(MigrationBuilder migrationBuilder)
            {
                migrationBuilder.DropTable(name: "Test");
            }
        }
        """
        
        result = parser.extract_up_down_methods(migration_content)
        assert result is not None
        up_content, down_content = result
        assert "CreateTable" in up_content
        assert "DropTable" in down_content

    def test_extract_up_down_methods_missing_up(self):
        """Test extraction fails when Up method is missing."""
        parser = MigrationParser()
        
        migration_content = """
        public partial class TestMigration : Migration
        {
            protected override void Down(MigrationBuilder migrationBuilder)
            {
                migrationBuilder.DropTable(name: "Test");
            }
        }
        """
        
        result = parser.extract_up_down_methods(migration_content)
        assert result is None

    def test_extract_up_down_methods_missing_down(self):
        """Test extraction fails when Down method is missing."""
        parser = MigrationParser()
        
        migration_content = """
        public partial class TestMigration : Migration
        {
            protected override void Up(MigrationBuilder migrationBuilder)
            {
                migrationBuilder.CreateTable(name: "Test");
            }
        }
        """
        
        result = parser.extract_up_down_methods(migration_content)
        assert result is None


class TestSqlExtractor:
    """Test the SqlExtractor class."""

    def test_extract_sql_from_content(self):
        """Test SQL extraction from migration content."""
        extractor = SqlExtractor()
        
        content = """
        migrationBuilder.Sql(@"CREATE TABLE Test (Id INT);");
        migrationBuilder.Sql(@"DROP TABLE Test;");
        """
        
        sql_matches = extractor.extract_sql_from_content(content)
        assert len(sql_matches) == 2
        assert "CREATE TABLE" in sql_matches[0]
        assert "DROP TABLE" in sql_matches[1]

    def test_is_stored_procedure_create(self):
        """Test detection of CREATE PROCEDURE."""
        extractor = SqlExtractor()
        
        sql = "CREATE PROCEDURE GetProducts AS BEGIN SELECT * FROM Products END;"
        assert extractor.is_stored_procedure(sql) is True

    def test_is_stored_procedure_alter(self):
        """Test detection of ALTER PROCEDURE."""
        extractor = SqlExtractor()
        
        sql = "ALTER PROCEDURE GetProducts AS BEGIN SELECT * FROM Products END;"
        assert extractor.is_stored_procedure(sql) is True

    def test_is_stored_procedure_not_procedure(self):
        """Test detection of non-procedure SQL."""
        extractor = SqlExtractor()
        
        sql = "CREATE TABLE Test (Id INT);"
        assert extractor.is_stored_procedure(sql) is False

    def test_extract_procedure_name(self):
        """Test extraction of procedure name."""
        extractor = SqlExtractor()
        
        sql = "CREATE PROCEDURE GetProducts AS BEGIN SELECT * FROM Products END;"
        name = extractor._extract_procedure_name(sql)
        assert name == "GetProducts"

    def test_extract_procedure_name_alter(self):
        """Test extraction of procedure name from ALTER statement."""
        extractor = SqlExtractor()
        
        sql = "ALTER PROCEDURE UpdateProducts AS BEGIN UPDATE Products SET Name = 'Test' END;"
        name = extractor._extract_procedure_name(sql)
        assert name == "UpdateProducts"

    def test_process_custom_sql_single_procedure(self):
        """Test processing single stored procedure."""
        extractor = SqlExtractor()
        
        up_content = 'migrationBuilder.Sql(@"CREATE PROCEDURE GetProducts AS BEGIN SELECT * FROM Products END;");'
        down_content = 'migrationBuilder.Sql(@"DROP PROCEDURE GetProducts;");'
        
        extractor.process_custom_sql(up_content, down_content)
        
        assert len(extractor.stored_procedures) == 1
        assert "GetProducts" in extractor.stored_procedures
        proc = extractor.stored_procedures["GetProducts"]
        assert "CREATE PROCEDURE" in proc.latest_up_method
        assert "DROP PROCEDURE" in proc.oldest_down_method

    def test_process_custom_sql_multiple_procedures(self):
        """Test processing multiple stored procedures."""
        extractor = SqlExtractor()
        
        up_content = """
        migrationBuilder.Sql(@"CREATE PROCEDURE GetProducts AS BEGIN SELECT * FROM Products END;");
        migrationBuilder.Sql(@"CREATE PROCEDURE UpdateProducts AS BEGIN UPDATE Products SET Name = 'Test' END;");
        """
        down_content = """
        migrationBuilder.Sql(@"DROP PROCEDURE GetProducts;");
        migrationBuilder.Sql(@"DROP PROCEDURE UpdateProducts;");
        """
        
        extractor.process_custom_sql(up_content, down_content)
        
        assert len(extractor.stored_procedures) == 2
        assert "GetProducts" in extractor.stored_procedures
        assert "UpdateProducts" in extractor.stored_procedures

    def test_process_custom_sql_procedure_evolution(self):
        """Test tracking procedure evolution across multiple migrations."""
        extractor = SqlExtractor()
        
        # First migration: CREATE
        up_content1 = 'migrationBuilder.Sql(@"CREATE PROCEDURE GetProducts AS BEGIN SELECT * FROM Products END;");'
        down_content1 = 'migrationBuilder.Sql(@"DROP PROCEDURE GetProducts;");'
        extractor.process_custom_sql(up_content1, down_content1)
        
        # Second migration: ALTER (should update latest_up_method)
        up_content2 = 'migrationBuilder.Sql(@"ALTER PROCEDURE GetProducts AS BEGIN SELECT Id, Name FROM Products END;");'
        down_content2 = 'migrationBuilder.Sql(@"ALTER PROCEDURE GetProducts AS BEGIN SELECT * FROM Products END;");'
        extractor.process_custom_sql(up_content2, down_content2)
        
        assert len(extractor.stored_procedures) == 1
        proc = extractor.stored_procedures["GetProducts"]
        # Latest up should be the ALTER version
        assert "ALTER PROCEDURE" in proc.latest_up_method
        assert "SELECT Id, Name" in proc.latest_up_method
        # Oldest down should be the original DROP
        assert "DROP PROCEDURE" in proc.oldest_down_method


class TestStoredProcedure:
    """Test the StoredProcedure class."""

    def test_update_up_method(self):
        """Test updating up method."""
        proc = StoredProcedure("TestProc", "CREATE PROCEDURE TestProc", "DROP PROCEDURE TestProc")
        proc.update_up_method("ALTER PROCEDURE TestProc")
        assert proc.latest_up_method == "ALTER PROCEDURE TestProc"

    def test_update_down_method(self):
        """Test updating down method preserves first value."""
        proc = StoredProcedure("TestProc", "CREATE PROCEDURE TestProc", "DROP PROCEDURE TestProc")
        # Should preserve the original value when updating
        original_down = proc.oldest_down_method
        proc.update_down_method("ALTER PROCEDURE TestProc")
        # Should still have the original value (oldest is preserved)
        assert proc.oldest_down_method == original_down


class TestMigrationManager:
    """Test the MigrationManager class."""

    @pytest.fixture
    def temp_migrations_dir(self):
        """Create a temporary directory with test migration files."""
        temp_dir = tempfile.mkdtemp()
        migrations_dir = Path(temp_dir) / "migrations"
        migrations_dir.mkdir()
        
        yield migrations_dir
        
        shutil.rmtree(temp_dir)

    def create_migration_file(self, migrations_dir: Path, filename: str, content: str):
        """Helper to create a migration file."""
        file_path = migrations_dir / filename
        file_path.write_text(content, encoding="utf-8")
        return file_path

    def test_get_migration_files_success(self, temp_migrations_dir):
        """Test getting migration files from directory."""
        manager = MigrationManager()
        
        # Create test migration files
        self.create_migration_file(
            temp_migrations_dir,
            "20240101000000_InitialMigration.cs",
            "public partial class InitialMigration : Migration { }"
        )
        self.create_migration_file(
            temp_migrations_dir,
            "20240102000000_SecondMigration.cs",
            "public partial class SecondMigration : Migration { }"
        )
        # Create Designer file (should be ignored)
        self.create_migration_file(
            temp_migrations_dir,
            "20240101000000_InitialMigration.Designer.cs",
            "// Designer file"
        )
        
        migration_files = manager.get_migration_files(temp_migrations_dir)
        
        assert len(migration_files) == 2
        assert all(f.suffix == ".cs" for f in migration_files)
        assert all(not f.name.endswith(".Designer.cs") for f in migration_files)

    def test_get_migration_files_empty_directory(self, temp_migrations_dir):
        """Test error when no migration files found."""
        manager = MigrationManager()
        
        with pytest.raises(ValueError, match="No migration files found"):
            manager.get_migration_files(temp_migrations_dir)

    def test_process_migration_files_with_stored_procedures(self, temp_migrations_dir):
        """Test processing migrations with stored procedures."""
        manager = MigrationManager()
        
        migration1_content = """
        public partial class Migration1 : Migration
        {
            protected override void Up(MigrationBuilder migrationBuilder)
            {
                migrationBuilder.Sql(@"CREATE PROCEDURE GetProducts AS BEGIN SELECT * FROM Products END;");
            }

            protected override void Down(MigrationBuilder migrationBuilder)
            {
                migrationBuilder.Sql(@"DROP PROCEDURE GetProducts;");
            }
        }
        """
        
        migration2_content = """
        public partial class Migration2 : Migration
        {
            protected override void Up(MigrationBuilder migrationBuilder)
            {
                migrationBuilder.Sql(@"CREATE PROCEDURE UpdateProducts AS BEGIN UPDATE Products SET Name = 'Test' END;");
            }

            protected override void Down(MigrationBuilder migrationBuilder)
            {
                migrationBuilder.Sql(@"DROP PROCEDURE UpdateProducts;");
            }
        }
        """
        
        self.create_migration_file(temp_migrations_dir, "Migration1.cs", migration1_content)
        self.create_migration_file(temp_migrations_dir, "Migration2.cs", migration2_content)
        
        migration_files = manager.get_migration_files(temp_migrations_dir)
        manager.process_migration_files(migration_files)
        
        assert len(manager.sql_extractor.stored_procedures) == 2
        assert "GetProducts" in manager.sql_extractor.stored_procedures
        assert "UpdateProducts" in manager.sql_extractor.stored_procedures

    def test_generate_squashed_migration_single_procedure(self):
        """Test generating squashed migration with single procedure."""
        manager = MigrationManager()
        
        # Manually add a stored procedure
        manager.sql_extractor.stored_procedures["GetProducts"] = StoredProcedure(
            "GetProducts",
            "CREATE PROCEDURE GetProducts AS BEGIN SELECT * FROM Products END;",
            "DROP PROCEDURE GetProducts;"
        )
        
        squashed = manager.generate_squashed_migration()
        
        assert "Squashed Migration" in squashed
        assert "protected override void Up" in squashed
        assert "protected override void Down" in squashed
        assert "GetProducts" in squashed
        assert "CREATE PROCEDURE GetProducts" in squashed
        assert "DROP PROCEDURE GetProducts" in squashed

    def test_generate_squashed_migration_multiple_procedures(self):
        """Test generating squashed migration with multiple procedures."""
        manager = MigrationManager()
        
        # Manually add multiple stored procedures
        manager.sql_extractor.stored_procedures["GetProducts"] = StoredProcedure(
            "GetProducts",
            "CREATE PROCEDURE GetProducts AS BEGIN SELECT * FROM Products END;",
            "DROP PROCEDURE GetProducts;"
        )
        manager.sql_extractor.stored_procedures["UpdateProducts"] = StoredProcedure(
            "UpdateProducts",
            "CREATE PROCEDURE UpdateProducts AS BEGIN UPDATE Products SET Name = 'Test' END;",
            "DROP PROCEDURE UpdateProducts;"
        )
        
        squashed = manager.generate_squashed_migration()
        
        assert "GetProducts" in squashed
        assert "UpdateProducts" in squashed
        assert squashed.count("protected override void Up") == 1
        assert squashed.count("protected override void Down") == 1
        # Should have both procedures in Up method
        assert squashed.count("CREATE PROCEDURE GetProducts") == 1
        assert squashed.count("CREATE PROCEDURE UpdateProducts") == 1

    def test_generate_squashed_migration_procedure_evolution(self):
        """Test squashed migration preserves latest up and oldest down."""
        manager = MigrationManager()
        
        # Simulate procedure evolution: CREATE -> ALTER
        manager.sql_extractor.stored_procedures["GetProducts"] = StoredProcedure(
            "GetProducts",
            "ALTER PROCEDURE GetProducts AS BEGIN SELECT Id, Name FROM Products END;",  # Latest
            "DROP PROCEDURE GetProducts;"  # Oldest
        )
        
        squashed = manager.generate_squashed_migration()
        
        # Should use latest ALTER in Up
        assert "ALTER PROCEDURE GetProducts" in squashed
        assert "SELECT Id, Name" in squashed
        # Should use oldest DROP in Down
        assert "DROP PROCEDURE GetProducts" in squashed

    def test_full_squash_workflow_with_fixtures(self):
        """Test the complete workflow using real fixture migration files."""
        manager = MigrationManager()
        
        # Use the fixtures directory which has real migration files
        if not FIXTURES_DIR.exists():
            pytest.skip("Fixtures directory not found")
        
        # Process migrations from fixtures
        migration_files = manager.get_migration_files(FIXTURES_DIR)
        manager.process_migration_files(migration_files)
        
        # Verify stored procedures were tracked correctly
        assert len(manager.sql_extractor.stored_procedures) == 2
        assert "GetProducts" in manager.sql_extractor.stored_procedures
        assert "UpdateProductPrice" in manager.sql_extractor.stored_procedures
        
        # Verify GetProducts has latest ALTER (from migration2)
        get_products_proc = manager.sql_extractor.stored_procedures["GetProducts"]
        assert "ALTER PROCEDURE" in get_products_proc.latest_up_method
        assert "Price > 0" in get_products_proc.latest_up_method
        # Verify oldest down is the original DROP (from migration1)
        assert "DROP PROCEDURE" in get_products_proc.oldest_down_method
        
        # Generate squashed migration
        squashed = manager.generate_squashed_migration()
        
        # Verify squashed migration contains both procedures
        assert "GetProducts" in squashed
        assert "UpdateProductPrice" in squashed
        # Verify GetProducts uses latest version (ALTER from migration2)
        assert "ALTER PROCEDURE GetProducts" in squashed
        assert "Price > 0" in squashed
        # Verify GetProducts uses oldest down (DROP from migration1)
        assert "DROP PROCEDURE GetProducts" in squashed
    
    def test_full_squash_workflow(self, temp_migrations_dir):
        """Test the complete workflow: parse migrations, extract procedures, generate squashed migration."""
        manager = MigrationManager()
        
        # Create migrations with stored procedures
        migration1 = """
        public partial class Migration1 : Migration
        {
            protected override void Up(MigrationBuilder migrationBuilder)
            {
                migrationBuilder.Sql(@"CREATE PROCEDURE GetProducts AS BEGIN SELECT * FROM Products END;");
            }

            protected override void Down(MigrationBuilder migrationBuilder)
            {
                migrationBuilder.Sql(@"DROP PROCEDURE GetProducts;");
            }
        }
        """
        
        migration2 = """
        public partial class Migration2 : Migration
        {
            protected override void Up(MigrationBuilder migrationBuilder)
            {
                migrationBuilder.Sql(@"ALTER PROCEDURE GetProducts AS BEGIN SELECT Id, Name FROM Products END;");
            }

            protected override void Down(MigrationBuilder migrationBuilder)
            {
                migrationBuilder.Sql(@"ALTER PROCEDURE GetProducts AS BEGIN SELECT * FROM Products END;");
            }
        }
        """
        
        migration3 = """
        public partial class Migration3 : Migration
        {
            protected override void Up(MigrationBuilder migrationBuilder)
            {
                migrationBuilder.Sql(@"CREATE PROCEDURE DeleteProducts AS BEGIN DELETE FROM Products WHERE Id = @Id END;");
            }

            protected override void Down(MigrationBuilder migrationBuilder)
            {
                migrationBuilder.Sql(@"DROP PROCEDURE DeleteProducts;");
            }
        }
        """
        
        self.create_migration_file(temp_migrations_dir, "Migration1.cs", migration1)
        self.create_migration_file(temp_migrations_dir, "Migration2.cs", migration2)
        self.create_migration_file(temp_migrations_dir, "Migration3.cs", migration3)
        
        # Process migrations
        migration_files = manager.get_migration_files(temp_migrations_dir)
        manager.process_migration_files(migration_files)
        
        # Verify stored procedures were tracked correctly
        assert len(manager.sql_extractor.stored_procedures) == 2
        assert "GetProducts" in manager.sql_extractor.stored_procedures
        assert "DeleteProducts" in manager.sql_extractor.stored_procedures
        
        # Verify GetProducts has latest ALTER (not original CREATE)
        get_products_proc = manager.sql_extractor.stored_procedures["GetProducts"]
        assert "ALTER PROCEDURE" in get_products_proc.latest_up_method
        assert "SELECT Id, Name" in get_products_proc.latest_up_method
        # Verify oldest down is the original DROP
        assert "DROP PROCEDURE" in get_products_proc.oldest_down_method
        
        # Generate squashed migration
        squashed = manager.generate_squashed_migration()
        
        # Verify squashed migration contains both procedures
        assert "GetProducts" in squashed
        assert "DeleteProducts" in squashed
        # Verify GetProducts uses latest version (ALTER)
        assert "ALTER PROCEDURE GetProducts" in squashed
        assert "SELECT Id, Name" in squashed
        # Verify GetProducts uses oldest down (DROP)
        assert "DROP PROCEDURE GetProducts" in squashed
    
    def test_actual_migrations_folder(self):
        """Test with actual migrations from the migrations folder."""
        manager = MigrationManager()
        
        # Use the actual migrations directory
        if not MIGRATIONS_DIR.exists():
            pytest.skip("Migrations directory not found")
        
        # Process migrations
        migration_files = manager.get_migration_files(MIGRATIONS_DIR)
        manager.process_migration_files(migration_files)
        
        # The actual migrations don't have stored procedures, so this should be empty
        # This tests that non-procedure SQL is correctly ignored
        assert len(manager.sql_extractor.stored_procedures) == 0
        
        # Generate squashed migration (should be empty/minimal)
        squashed = manager.generate_squashed_migration()
        assert "protected override void Up" in squashed
        assert "protected override void Down" in squashed
    
    def test_reject_type_migration_with_insert_sql(self):
        """Test the rejectTypeMigration with INSERT statement (should ignore non-procedure SQL)."""
        manager = MigrationManager()
        
        # Use the fixtures directory with the reject type migration
        if not FIXTURES_DIR.exists():
            pytest.skip("Fixtures directory not found")
        
        reject_type_file = FIXTURES_DIR / "20240104000000_rejectTypeMigration.cs"
        if not reject_type_file.exists():
            pytest.skip("Reject type migration fixture not found")
        
        # Process just the reject type migration
        manager.process_migration_files([reject_type_file])
        
        # Should have no stored procedures (INSERT is not a procedure)
        assert len(manager.sql_extractor.stored_procedures) == 0
        
        # Verify the SQL was extracted but not treated as a procedure
        # This confirms the system correctly distinguishes between SQL and stored procedures
        squashed = manager.generate_squashed_migration()
        assert "protected override void Up" in squashed
        assert "protected override void Down" in squashed
        # Should NOT contain the INSERT statement (only procedures are included)
        assert "INSERT INTO Inventory.RejectType" not in squashed
    
    def test_mixed_migrations_procedures_and_regular_sql(self):
        """Test processing a mix of migrations with procedures and regular SQL."""
        manager = MigrationManager()
        
        if not FIXTURES_DIR.exists():
            pytest.skip("Fixtures directory not found")
        
        # Process all fixtures (includes both procedure migrations and reject type migration)
        migration_files = manager.get_migration_files(FIXTURES_DIR)
        manager.process_migration_files(migration_files)
        
        # Should only have the stored procedures, not the INSERT from reject type migration
        assert len(manager.sql_extractor.stored_procedures) == 2
        assert "GetProducts" in manager.sql_extractor.stored_procedures
        assert "UpdateProductPrice" in manager.sql_extractor.stored_procedures
        
        # Generate squashed migration
        squashed = manager.generate_squashed_migration()
        
        # Should contain the procedures
        assert "GetProducts" in squashed
        assert "UpdateProductPrice" in squashed
        
        # Should NOT contain the INSERT statement from reject type migration
        assert "INSERT INTO Inventory.RejectType" not in squashed
        assert "Coatings" not in squashed
        assert "Weld" not in squashed

    def test_squash_migration_with_non_procedure_sql(self, temp_migrations_dir):
        """Test that non-procedure SQL is ignored during squashing."""
        manager = MigrationManager()
        
        migration_content = """
        public partial class Migration1 : Migration
        {
            protected override void Up(MigrationBuilder migrationBuilder)
            {
                migrationBuilder.CreateTable(name: "Products");
                migrationBuilder.Sql(@"INSERT INTO Products VALUES (1, 'Test');");
            }

            protected override void Down(MigrationBuilder migrationBuilder)
            {
                migrationBuilder.DropTable(name: "Products");
            }
        }
        """
        
        self.create_migration_file(temp_migrations_dir, "Migration1.cs", migration_content)
        
        migration_files = manager.get_migration_files(temp_migrations_dir)
        manager.process_migration_files(migration_files)
        
        # Should have no stored procedures (INSERT is not a procedure)
        assert len(manager.sql_extractor.stored_procedures) == 0
        
        # Squashed migration should be mostly empty (just structure)
        squashed = manager.generate_squashed_migration()
        assert "protected override void Up" in squashed
        assert "protected override void Down" in squashed
        # Should not contain the INSERT statement
        assert "INSERT INTO Products" not in squashed

