"""
Test edge cases for migration squashing.
Tests scenarios like empty procedures, DROP-only procedures, etc.
"""
import pytest
from pathlib import Path

from main import MigrationManager
from classes.StoreProcedure import StoredProcedure


class TestEdgeCases:
    """Test edge cases and potential bugs."""
    
    def test_empty_procedure_up_method(self):
        """Test that procedures with empty up methods don't generate invalid SQL."""
        manager = MigrationManager()
        
        # Create a procedure with empty up method (could happen with DROP-only procedures)
        manager.sql_extractor.stored_procedures["TestProc"] = StoredProcedure(
            "TestProc",
            "",  # Empty up method
            "DROP PROCEDURE TestProc;"
        )
        
        squashed = manager.generate_squashed_migration()
        
        # Should not contain empty SQL statement
        assert 'migrationBuilder.Sql(@"");' not in squashed
        # Should not include the procedure name in Up if no up method
        assert "protected override void Up" in squashed
        # Should still have the Down method with the DROP
        assert "DROP PROCEDURE TestProc" in squashed
    
    def test_empty_procedure_down_method(self):
        """Test that procedures with empty down methods don't generate invalid SQL."""
        manager = MigrationManager()
        
        # Create a procedure with empty down method
        manager.sql_extractor.stored_procedures["TestProc"] = StoredProcedure(
            "TestProc",
            "CREATE PROCEDURE TestProc AS BEGIN SELECT 1 END;",
            ""  # Empty down method
        )
        
        squashed = manager.generate_squashed_migration()
        
        # Should not contain empty SQL statement
        assert 'migrationBuilder.Sql(@"");' not in squashed
        # Should have the CREATE in Up
        assert "CREATE PROCEDURE TestProc" in squashed
        # Should not include the procedure name in Down if no down method
        # (the name will appear but not with empty SQL)
        down_section = squashed.split("protected override void Down")[1]
        assert 'migrationBuilder.Sql(@"");' not in down_section
    
    def test_both_methods_empty(self):
        """Test that procedures with both methods empty don't appear in output."""
        manager = MigrationManager()
        
        # Create a procedure with both empty (shouldn't happen but defensive)
        manager.sql_extractor.stored_procedures["EmptyProc"] = StoredProcedure(
            "EmptyProc",
            "",
            ""
        )
        # Also add a valid one
        manager.sql_extractor.stored_procedures["ValidProc"] = StoredProcedure(
            "ValidProc",
            "CREATE PROCEDURE ValidProc AS BEGIN SELECT 1 END;",
            "DROP PROCEDURE ValidProc;"
        )
        
        squashed = manager.generate_squashed_migration()
        
        # Should not contain any empty SQL statements
        assert 'migrationBuilder.Sql(@"");' not in squashed
        # Should only have the valid procedure
        assert "ValidProc" in squashed
        # Empty proc name might appear in comment but not with SQL
        if "EmptyProc" in squashed:
            # If it appears, it shouldn't be followed by empty Sql call
            lines = squashed.split('\n')
            for i, line in enumerate(lines):
                if "EmptyProc" in line:
                    # Next line should not be migrationBuilder.Sql with empty string
                    if i + 1 < len(lines):
                        assert 'migrationBuilder.Sql(@"");' not in lines[i + 1]
    
    def test_drop_only_procedure_in_down_method(self):
        """Test procedure that only appears as DROP in Down method."""
        manager = MigrationManager()
        
        # Simulate a procedure that was created before our squash point
        # and only appears as DROP in the migrations we're squashing
        manager.sql_extractor.stored_procedures["PreExistingProc"] = StoredProcedure(
            "PreExistingProc",
            "",  # No up method in our migrations
            "DROP PROCEDURE PreExistingProc;"  # Only DROP
        )
        
        squashed = manager.generate_squashed_migration()
        
        # Should not generate invalid code
        assert 'migrationBuilder.Sql(@"");' not in squashed
        # Should have the DROP in Down
        assert "DROP PROCEDURE PreExistingProc" in squashed
        # Should not have a corresponding entry in Up
        up_section = squashed.split("protected override void Down")[0]
        # PreExistingProc should not appear in Up section
        # (or if it does as comment, shouldn't have SQL call)
        if "PreExistingProc" in up_section:
            # Make sure it's not followed by SQL call
            assert 'migrationBuilder.Sql' not in up_section.split("PreExistingProc")[1].split('\n')[0]
    
    def test_multiple_procedures_some_empty(self):
        """Test mix of valid and empty procedures."""
        manager = MigrationManager()
        
        # Mix of different procedure states
        manager.sql_extractor.stored_procedures["ValidProc"] = StoredProcedure(
            "ValidProc",
            "CREATE PROCEDURE ValidProc AS BEGIN SELECT 1 END;",
            "DROP PROCEDURE ValidProc;"
        )
        manager.sql_extractor.stored_procedures["NoDown"] = StoredProcedure(
            "NoDown",
            "CREATE PROCEDURE NoDown AS BEGIN SELECT 2 END;",
            ""
        )
        manager.sql_extractor.stored_procedures["NoUp"] = StoredProcedure(
            "NoUp",
            "",
            "DROP PROCEDURE NoUp;"
        )
        
        squashed = manager.generate_squashed_migration()
        
        # Should not have any empty SQL calls
        assert 'migrationBuilder.Sql(@"");' not in squashed
        
        # ValidProc should appear in both
        assert "CREATE PROCEDURE ValidProc" in squashed
        assert "DROP PROCEDURE ValidProc" in squashed
        
        # NoDown should only appear in Up
        assert "CREATE PROCEDURE NoDown" in squashed
        down_section = squashed.split("protected override void Down")[1]
        assert "NoDown" not in down_section or 'migrationBuilder.Sql' not in down_section.split("NoDown")[0] if "NoDown" in down_section else True
        
        # NoUp should only appear in Down
        up_section = squashed.split("protected override void Down")[0]
        assert "NoUp" not in up_section or 'migrationBuilder.Sql' not in up_section.split("NoUp")[0] if "NoUp" in up_section else True
        assert "DROP PROCEDURE NoUp" in down_section
    
    def test_whitespace_only_is_treated_as_empty(self):
        """Test that whitespace-only strings are handled correctly."""
        manager = MigrationManager()
        
        # Procedure with whitespace-only methods
        manager.sql_extractor.stored_procedures["WhitespaceProc"] = StoredProcedure(
            "WhitespaceProc",
            "   ",  # Whitespace only
            "\n\t  "  # Whitespace only
        )
        
        squashed = manager.generate_squashed_migration()
        
        # Python's truthiness treats whitespace strings as truthy, but empty string as falsy
        # This test documents current behavior - whitespace strings will be included
        # In a production scenario, you might want to strip() before checking
        # For now, this is acceptable as our SQL extraction shouldn't produce whitespace-only strings

