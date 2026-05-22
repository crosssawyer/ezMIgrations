use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

// ─── Static Regexes (compiled once) ────────────────────────────────

fn re_operations() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"migrationBuilder\s*\.\s*(\w+)\s*\(").unwrap())
}

fn re_sql_simple() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"migrationBuilder\s*\.\s*Sql\s*\(\s*@?"((?:[^"\\]|\\.|"")*?)"\s*\)"#)
            .unwrap()
    })
}

fn re_sql_object() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(?:CREATE\s+(?:OR\s+ALTER\s+)?|ALTER\s+)(PROCEDURE|VIEW|FUNCTION|TRIGGER)\s+(\[?[\w.]+\]?(?:\.\[?[\w.]+\]?)*)")
            .unwrap()
    })
}

// ─── Types ─────────────────────────────────────────────────────────

/// A custom SQL statement with ordering metadata for correct injection during squash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlStatement {
    pub sql: String,
    /// Position of this SQL call among all migrationBuilder calls in the method.
    pub operation_index: usize,
    /// Total number of migrationBuilder calls in the method.
    pub total_operations: usize,
}

/// Controls which version to keep when multiple SQL statements target the same object.
pub enum KeepStrategy {
    /// Keep the last version (for Up — newest stored proc wins).
    Last,
    /// Keep the first version (for Down — reverts to pre-squash state).
    First,
}

/// Represents extracted data from a single migration file.
#[derive(Debug, Clone)]
pub struct ParsedMigration {
    pub file_name: String,
    pub up_body: String,
    pub down_body: String,
    pub custom_sql_up: Vec<SqlStatement>,
    pub custom_sql_down: Vec<SqlStatement>,
    pub has_custom_sql: bool,
}

impl ParsedMigration {
    pub fn sql_strings_up(&self) -> Vec<String> {
        self.custom_sql_up.iter().map(|s| s.sql.clone()).collect()
    }

    pub fn sql_strings_down(&self) -> Vec<String> {
        self.custom_sql_down.iter().map(|s| s.sql.clone()).collect()
    }
}

// ─── Parser ────────────────────────────────────────────────────────

pub struct MigrationParser;

impl MigrationParser {
    pub fn find_migration_files(project_path: &str) -> Result<Vec<PathBuf>, String> {
        let migrations_dir = Self::find_migrations_dir(project_path)?;
        let mut files: Vec<PathBuf> = Vec::new();

        let entries = fs::read_dir(&migrations_dir)
            .map_err(|e| format!("Cannot read migrations directory: {}", e))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(".cs")
                    && !name.ends_with(".Designer.cs")
                    && !name.contains("ModelSnapshot")
                {
                    files.push(path);
                }
            }
        }

        files.sort();
        Ok(files)
    }

    pub fn find_migrations_dir(project_path: &str) -> Result<PathBuf, String> {
        let base = Path::new(project_path);

        let base = if base.is_file() {
            base.parent().unwrap_or(base)
        } else {
            base
        };

        let candidates = [
            base.join("Migrations"),
            base.join("Data").join("Migrations"),
        ];

        for candidate in &candidates {
            if candidate.exists() && candidate.is_dir() {
                return Ok(candidate.clone());
            }
        }

        if let Some(found) = Self::walk_for_migrations(base, 5) {
            return Ok(found);
        }

        // Search sibling directories (common in multi-project solutions
        // where migrations live in a separate project like cmms-data/)
        if let Some(parent) = base.parent() {
            if let Some(found) = Self::walk_for_migrations(parent, 3) {
                return Ok(found);
            }
        }

        Err(format!(
            "No Migrations directory found in {}",
            project_path
        ))
    }

    fn walk_for_migrations(dir: &Path, depth: u32) -> Option<PathBuf> {
        if depth == 0 {
            return None;
        }
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if path.file_name().and_then(|n| n.to_str()) == Some("Migrations") {
                        return Some(path);
                    }
                    if let Some(found) = Self::walk_for_migrations(&path, depth - 1) {
                        return Some(found);
                    }
                }
            }
        }
        None
    }

    pub fn parse_file(file_path: &Path) -> Result<ParsedMigration, String> {
        let content = fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read {}: {}", file_path.display(), e))?;

        let file_name = file_path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let up_body = Self::extract_method_body(&content, "Up");
        let down_body = Self::extract_method_body(&content, "Down");

        let custom_sql_up = Self::extract_custom_sql(&up_body);
        let custom_sql_down = Self::extract_custom_sql(&down_body);
        let has_custom_sql = !custom_sql_up.is_empty() || !custom_sql_down.is_empty();

        Ok(ParsedMigration {
            file_name,
            up_body,
            down_body,
            custom_sql_up,
            custom_sql_down,
            has_custom_sql,
        })
    }

    fn extract_method_body(content: &str, method_name: &str) -> String {
        let pattern = format!(
            r"protected\s+override\s+void\s+{}\s*\(",
            regex::escape(method_name)
        );
        let re = Regex::new(&pattern).unwrap();

        if let Some(m) = re.find(content) {
            let after_signature = &content[m.end()..];
            if let Some(brace_start) = after_signature.find('{') {
                let body_start = m.end() + brace_start;
                if let Some(body_end) = Self::find_matching_brace(content, body_start) {
                    return content[body_start + 1..body_end].trim().to_string();
                }
            }
        }

        String::new()
    }

    fn find_matching_brace(content: &str, start: usize) -> Option<usize> {
        let bytes = content.as_bytes();
        let mut depth = 0;
        let mut in_string = false;
        let mut in_verbatim = false;
        let mut prev_char = 0u8;
        let mut skip_next = false;

        for i in start..bytes.len() {
            if skip_next {
                skip_next = false;
                prev_char = bytes[i];
                continue;
            }

            let ch = bytes[i];

            if ch == b'"' && !in_verbatim {
                if prev_char == b'@' {
                    in_verbatim = true;
                    in_string = true;
                } else if prev_char != b'\\' {
                    in_string = !in_string;
                }
            } else if in_verbatim && ch == b'"' {
                if i + 1 < bytes.len() && bytes[i + 1] == b'"' {
                    skip_next = true;
                } else {
                    in_verbatim = false;
                    in_string = false;
                }
            }

            if !in_string {
                if ch == b'{' {
                    depth += 1;
                } else if ch == b'}' {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
            }

            prev_char = ch;
        }

        None
    }

    pub fn extract_custom_sql(method_body: &str) -> Vec<SqlStatement> {
        let operations: Vec<(usize, String)> = re_operations()
            .captures_iter(method_body)
            .map(|cap| {
                (
                    cap.get(0).unwrap().start(),
                    cap.get(1).unwrap().as_str().to_string(),
                )
            })
            .collect();

        let total_operations = operations.len();

        let sql_op_indices: HashMap<usize, usize> = operations
            .iter()
            .enumerate()
            .filter(|(_, (_, name))| name == "Sql")
            .map(|(idx, (offset, _))| (*offset, idx))
            .collect();

        let mut sqls = Vec::new();

        for cap in re_sql_simple().captures_iter(method_body) {
            if let Some(sql) = cap.get(1) {
                let sql_text = Self::unescape_sql(sql.as_str());

                let call_offset = cap.get(0).unwrap().start();
                let operation_index = sql_op_indices
                    .get(&call_offset)
                    .copied()
                    .unwrap_or(total_operations);

                sqls.push(SqlStatement {
                    sql: sql_text,
                    operation_index,
                    total_operations,
                });
            }
        }

        sqls
    }

    /// Normalize C# verbatim string escaping to plain text.
    fn unescape_sql(raw: &str) -> String {
        raw.replace("\"\"", "\"")
            .replace("\\n", "\n")
            .replace("\\r", "")
            .replace("\\t", "\t")
            .trim()
            .to_string()
    }

    fn extract_sql_object_name(sql: &str) -> Option<String> {
        re_sql_object().captures(sql).map(|cap| {
            let obj_type = cap.get(1).unwrap().as_str().to_uppercase();
            let obj_name = cap
                .get(2)
                .unwrap()
                .as_str()
                .replace('[', "")
                .replace(']', "")
                .to_uppercase();
            format!("{}.{}", obj_type, obj_name)
        })
    }

    /// Deduplicate SQL statements targeting the same database object.
    /// `KeepStrategy::Last` for Up (newest wins), `KeepStrategy::First` for Down (revert to pre-squash).
    pub fn deduplicate_sql(
        statements: Vec<SqlStatement>,
        strategy: KeepStrategy,
    ) -> Vec<SqlStatement> {
        // Extract object names once per statement to avoid redundant regex calls
        let obj_names: Vec<Option<String>> = statements
            .iter()
            .map(|stmt| Self::extract_sql_object_name(&stmt.sql))
            .collect();

        let mut keep_idx: HashMap<String, usize> = HashMap::new();
        for (i, name) in obj_names.iter().enumerate() {
            if let Some(obj_name) = name {
                match strategy {
                    KeepStrategy::First => {
                        keep_idx.entry(obj_name.clone()).or_insert(i);
                    }
                    KeepStrategy::Last => {
                        keep_idx.insert(obj_name.clone(), i);
                    }
                }
            }
        }

        statements
            .into_iter()
            .enumerate()
            .filter(|(i, _)| match &obj_names[*i] {
                Some(obj_name) => keep_idx.get(obj_name) == Some(i),
                None => true,
            })
            .map(|(_, stmt)| stmt)
            .collect()
    }

    /// Inject custom SQL into a migration method (Up or Down).
    /// SQL is placed after all existing schema operations (before the closing brace).
    pub fn inject_custom_sql(
        file_path: &Path,
        method_name: &str,
        sql_statements: &[SqlStatement],
    ) -> Result<(), String> {
        let content = fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read {}: {}", file_path.display(), e))?;

        let pattern = format!(
            r"protected\s+override\s+void\s+{}\s*\(",
            regex::escape(method_name)
        );
        let re = Regex::new(&pattern).unwrap();

        if let Some(m) = re.find(&content) {
            let after_sig = &content[m.end()..];
            let brace_rel = after_sig.find('{').ok_or_else(|| {
                format!("Could not find opening brace of {} method", method_name)
            })?;
            let brace_abs = m.end() + brace_rel;
            let close_abs = Self::find_matching_brace(&content, brace_abs).ok_or_else(|| {
                format!("Could not find closing brace of {} method", method_name)
            })?;

            let mut injected_sql = String::new();
            for sql in sql_statements {
                let escaped = sql.sql.replace('"', "\"\"");
                injected_sql.push_str(&format!(
                    "\n            migrationBuilder.Sql(@\"{}\");",
                    escaped
                ));
            }
            injected_sql.push('\n');

            let new_content = format!(
                "{}{}{}",
                &content[..close_abs],
                injected_sql,
                &content[close_abs..]
            );

            fs::write(file_path, new_content)
                .map_err(|e| format!("Failed to write {}: {}", file_path.display(), e))?;

            Ok(())
        } else {
            Err(format!(
                "Could not find {} method in migration file",
                method_name
            ))
        }
    }

    pub fn get_migration_file(project_path: &str, migration_name: &str) -> Option<PathBuf> {
        if let Ok(files) = Self::find_migration_files(project_path) {
            for file in files {
                if let Some(name) = file.file_stem().and_then(|n| n.to_str()) {
                    if name.contains(migration_name) || migration_name.contains(name) {
                        return Some(file);
                    }
                }
            }
        }
        None
    }
}

// ─── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn sample_migration(up: &str, down: &str) -> String {
        format!(
            r#"using Microsoft.EntityFrameworkCore.Migrations;

namespace MyApp.Migrations
{{
    public partial class TestMigration : Migration
    {{
        protected override void Up(MigrationBuilder migrationBuilder)
        {{
{up}
        }}

        protected override void Down(MigrationBuilder migrationBuilder)
        {{
{down}
        }}
    }}
}}
"#
        )
    }

    fn write_temp_migration(content: &str) -> (TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("20240101000000_Test.cs");
        fs::write(&path, content).unwrap();
        (dir, path)
    }

    // ─── extract_method_body ─────────────────────────────────────

    #[test]
    fn extracts_up_method_body() {
        let content = sample_migration("            // up body marker", "            // down body marker");
        let body = MigrationParser::extract_method_body(&content, "Up");
        assert!(body.contains("up body marker"));
        assert!(!body.contains("down body marker"));
    }

    #[test]
    fn extracts_down_method_body() {
        let content = sample_migration("            // up body marker", "            // down body marker");
        let body = MigrationParser::extract_method_body(&content, "Down");
        assert!(body.contains("down body marker"));
        assert!(!body.contains("up body marker"));
    }

    #[test]
    fn returns_empty_string_when_method_missing() {
        let content = "public class Foo { void Bar() {} }";
        let body = MigrationParser::extract_method_body(content, "Up");
        assert_eq!(body, "");
    }

    #[test]
    fn handles_nested_braces_in_method_body() {
        let content = sample_migration(
            r#"            if (true) { /* inner */ }
            migrationBuilder.Sql("SELECT 1");"#,
            "",
        );
        let body = MigrationParser::extract_method_body(&content, "Up");
        assert!(body.contains("SELECT 1"));
        assert!(body.contains("inner"));
    }

    #[test]
    fn handles_string_literals_containing_braces() {
        let content = sample_migration(
            r#"            migrationBuilder.Sql("CREATE PROC x AS BEGIN SELECT 1 END");"#,
            "",
        );
        let body = MigrationParser::extract_method_body(&content, "Up");
        // The closing brace of method must come after the SQL string, not be confused
        // with a brace inside the string literal.
        assert!(body.contains("CREATE PROC"));
    }

    // ─── extract_custom_sql ──────────────────────────────────────

    #[test]
    fn extracts_simple_sql_string() {
        let body = r#"migrationBuilder.Sql("SELECT 1");"#;
        let sqls = MigrationParser::extract_custom_sql(body);
        assert_eq!(sqls.len(), 1);
        assert_eq!(sqls[0].sql, "SELECT 1");
    }

    #[test]
    fn extracts_verbatim_string() {
        let body = r#"migrationBuilder.Sql(@"SELECT ""foo""");"#;
        let sqls = MigrationParser::extract_custom_sql(body);
        assert_eq!(sqls.len(), 1);
        // verbatim "" -> single quote in unescape_sql
        assert_eq!(sqls[0].sql, r#"SELECT "foo""#);
    }

    #[test]
    fn extracts_multiple_sql_statements() {
        let body = r#"
            migrationBuilder.Sql("CREATE PROCEDURE p1 AS SELECT 1");
            migrationBuilder.CreateTable("X");
            migrationBuilder.Sql("CREATE PROCEDURE p2 AS SELECT 2");
        "#;
        let sqls = MigrationParser::extract_custom_sql(body);
        assert_eq!(sqls.len(), 2);
        assert!(sqls[0].sql.contains("p1"));
        assert!(sqls[1].sql.contains("p2"));
    }

    #[test]
    fn records_operation_indices_among_all_calls() {
        let body = r#"
            migrationBuilder.CreateTable("T1");
            migrationBuilder.Sql("S1");
            migrationBuilder.AddColumn("c");
            migrationBuilder.Sql("S2");
        "#;
        let sqls = MigrationParser::extract_custom_sql(body);
        assert_eq!(sqls.len(), 2);
        assert_eq!(sqls[0].operation_index, 1);
        assert_eq!(sqls[1].operation_index, 3);
        assert_eq!(sqls[0].total_operations, 4);
        assert_eq!(sqls[1].total_operations, 4);
    }

    #[test]
    fn returns_empty_when_no_sql_calls() {
        let body = r#"
            migrationBuilder.CreateTable("Foo");
            migrationBuilder.DropTable("Bar");
        "#;
        let sqls = MigrationParser::extract_custom_sql(body);
        assert!(sqls.is_empty());
    }

    #[test]
    fn handles_escape_sequences_in_normal_string() {
        let body = r#"migrationBuilder.Sql("line1\nline2\ttab");"#;
        let sqls = MigrationParser::extract_custom_sql(body);
        assert_eq!(sqls.len(), 1);
        assert_eq!(sqls[0].sql, "line1\nline2\ttab");
    }

    #[test]
    fn extracts_sql_from_parsed_file() {
        let content = sample_migration(
            r#"            migrationBuilder.Sql("CREATE PROCEDURE foo AS SELECT 1");"#,
            r#"            migrationBuilder.Sql("DROP PROCEDURE foo");"#,
        );
        let (_dir, path) = write_temp_migration(&content);
        let parsed = MigrationParser::parse_file(&path).unwrap();
        assert!(parsed.has_custom_sql);
        assert_eq!(parsed.custom_sql_up.len(), 1);
        assert_eq!(parsed.custom_sql_down.len(), 1);
        assert!(parsed.custom_sql_up[0].sql.contains("CREATE PROCEDURE foo"));
        assert!(parsed.custom_sql_down[0].sql.contains("DROP PROCEDURE foo"));
    }

    #[test]
    fn parsed_file_with_only_up_sql() {
        let content = sample_migration(
            r#"            migrationBuilder.Sql("SELECT 1");"#,
            r#"            migrationBuilder.DropTable("X");"#,
        );
        let (_dir, path) = write_temp_migration(&content);
        let parsed = MigrationParser::parse_file(&path).unwrap();
        assert!(parsed.has_custom_sql);
        assert_eq!(parsed.custom_sql_up.len(), 1);
        assert!(parsed.custom_sql_down.is_empty());
    }

    #[test]
    fn parsed_file_with_no_custom_sql() {
        let content = sample_migration(
            r#"            migrationBuilder.CreateTable("T");"#,
            r#"            migrationBuilder.DropTable("T");"#,
        );
        let (_dir, path) = write_temp_migration(&content);
        let parsed = MigrationParser::parse_file(&path).unwrap();
        assert!(!parsed.has_custom_sql);
        assert!(parsed.custom_sql_up.is_empty());
        assert!(parsed.custom_sql_down.is_empty());
    }

    #[test]
    fn parse_file_fails_on_missing_path() {
        let result = MigrationParser::parse_file(Path::new("/nonexistent/path/file.cs"));
        assert!(result.is_err());
    }

    // ─── deduplicate_sql ─────────────────────────────────────────

    fn stmt(sql: &str) -> SqlStatement {
        SqlStatement {
            sql: sql.to_string(),
            operation_index: 0,
            total_operations: 0,
        }
    }

    #[test]
    fn dedupe_last_keeps_newest_version() {
        let stmts = vec![
            stmt("CREATE PROCEDURE [dbo].[GetUsers] AS SELECT 1"),
            stmt("CREATE OR ALTER PROCEDURE [dbo].[GetUsers] AS SELECT 2"),
        ];
        let result = MigrationParser::deduplicate_sql(stmts, KeepStrategy::Last);
        assert_eq!(result.len(), 1);
        assert!(result[0].sql.contains("SELECT 2"));
    }

    #[test]
    fn dedupe_first_keeps_oldest_version() {
        let stmts = vec![
            stmt("CREATE PROCEDURE [dbo].[GetUsers] AS SELECT 1"),
            stmt("CREATE OR ALTER PROCEDURE [dbo].[GetUsers] AS SELECT 2"),
        ];
        let result = MigrationParser::deduplicate_sql(stmts, KeepStrategy::First);
        assert_eq!(result.len(), 1);
        assert!(result[0].sql.contains("SELECT 1"));
    }

    #[test]
    fn dedupe_treats_different_objects_independently() {
        let stmts = vec![
            stmt("CREATE PROCEDURE [dbo].[A] AS SELECT 1"),
            stmt("CREATE PROCEDURE [dbo].[B] AS SELECT 2"),
            stmt("CREATE OR ALTER PROCEDURE [dbo].[A] AS SELECT 3"),
        ];
        let result = MigrationParser::deduplicate_sql(stmts, KeepStrategy::Last);
        assert_eq!(result.len(), 2);
        // A should be the v3 variant, B should remain
        let a = result.iter().find(|s| s.sql.contains("[A]")).unwrap();
        assert!(a.sql.contains("SELECT 3"));
        assert!(result.iter().any(|s| s.sql.contains("[B]")));
    }

    #[test]
    fn dedupe_preserves_statements_with_no_recognized_object() {
        let stmts = vec![
            stmt("INSERT INTO Foo VALUES (1)"),
            stmt("UPDATE Bar SET x = 1"),
        ];
        let result = MigrationParser::deduplicate_sql(stmts, KeepStrategy::Last);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn dedupe_handles_views_and_functions() {
        let stmts = vec![
            stmt("CREATE VIEW dbo.MyView AS SELECT 1"),
            stmt("CREATE OR ALTER VIEW dbo.MyView AS SELECT 2"),
            stmt("CREATE FUNCTION dbo.MyFunc() RETURNS INT AS BEGIN RETURN 1 END"),
        ];
        let result = MigrationParser::deduplicate_sql(stmts, KeepStrategy::Last);
        assert_eq!(result.len(), 2);
        let view = result.iter().find(|s| s.sql.contains("VIEW")).unwrap();
        assert!(view.sql.contains("SELECT 2"));
    }

    // ─── inject_custom_sql ───────────────────────────────────────

    #[test]
    fn injects_sql_into_up_method() {
        let content = sample_migration(
            r#"            migrationBuilder.CreateTable("X");"#,
            r#"            migrationBuilder.DropTable("X");"#,
        );
        let (_dir, path) = write_temp_migration(&content);

        let sqls = vec![stmt("CREATE PROCEDURE p AS SELECT 1")];
        MigrationParser::inject_custom_sql(&path, "Up", &sqls).unwrap();

        let written = fs::read_to_string(&path).unwrap();
        assert!(written.contains(r#"migrationBuilder.Sql(@"CREATE PROCEDURE p AS SELECT 1");"#));

        // Re-parse to confirm the migration is still well-formed and SQL is in Up only
        let parsed = MigrationParser::parse_file(&path).unwrap();
        assert_eq!(parsed.custom_sql_up.len(), 1);
        assert!(parsed.custom_sql_down.is_empty());
    }

    #[test]
    fn injects_sql_into_down_method() {
        let content = sample_migration(
            r#"            migrationBuilder.CreateTable("X");"#,
            r#"            migrationBuilder.DropTable("X");"#,
        );
        let (_dir, path) = write_temp_migration(&content);

        let sqls = vec![stmt("DROP PROCEDURE p")];
        MigrationParser::inject_custom_sql(&path, "Down", &sqls).unwrap();

        let parsed = MigrationParser::parse_file(&path).unwrap();
        assert!(parsed.custom_sql_up.is_empty());
        assert_eq!(parsed.custom_sql_down.len(), 1);
        assert!(parsed.custom_sql_down[0].sql.contains("DROP PROCEDURE p"));
    }

    #[test]
    fn injects_sql_with_embedded_quotes_using_verbatim_escape() {
        let content = sample_migration("", "");
        let (_dir, path) = write_temp_migration(&content);

        let sqls = vec![stmt(r#"PRINT "hello""#)];
        MigrationParser::inject_custom_sql(&path, "Up", &sqls).unwrap();

        // After roundtrip parse, the unescaped form should match the original
        let parsed = MigrationParser::parse_file(&path).unwrap();
        assert_eq!(parsed.custom_sql_up.len(), 1);
        assert_eq!(parsed.custom_sql_up[0].sql, r#"PRINT "hello""#);
    }

    #[test]
    fn inject_fails_when_method_not_found() {
        let content = "public class Foo { void Bar() {} }";
        let (_dir, path) = write_temp_migration(content);

        let sqls = vec![stmt("SELECT 1")];
        let result = MigrationParser::inject_custom_sql(&path, "Up", &sqls);
        assert!(result.is_err());
    }

    // ─── find_migrations_dir ────────────────────────────────────

    #[test]
    fn finds_migrations_dir_at_root() {
        let dir = tempfile::tempdir().unwrap();
        let migrations = dir.path().join("Migrations");
        fs::create_dir(&migrations).unwrap();

        let found = MigrationParser::find_migrations_dir(dir.path().to_str().unwrap()).unwrap();
        assert_eq!(found, migrations);
    }

    #[test]
    fn finds_migrations_dir_under_data_subdir() {
        let dir = tempfile::tempdir().unwrap();
        let migrations = dir.path().join("Data").join("Migrations");
        fs::create_dir_all(&migrations).unwrap();

        let found = MigrationParser::find_migrations_dir(dir.path().to_str().unwrap()).unwrap();
        assert_eq!(found, migrations);
    }

    #[test]
    fn errors_when_no_migrations_dir_found() {
        let dir = tempfile::tempdir().unwrap();
        let result = MigrationParser::find_migrations_dir(dir.path().to_str().unwrap());
        assert!(result.is_err());
    }

    // ─── find_migration_files ────────────────────────────────────

    #[test]
    fn finds_migration_files_and_skips_designer_and_snapshots() {
        let dir = tempfile::tempdir().unwrap();
        let migrations = dir.path().join("Migrations");
        fs::create_dir(&migrations).unwrap();
        fs::write(migrations.join("20240101_A.cs"), "").unwrap();
        fs::write(migrations.join("20240101_A.Designer.cs"), "").unwrap();
        fs::write(migrations.join("20240202_B.cs"), "").unwrap();
        fs::write(migrations.join("MyContextModelSnapshot.cs"), "").unwrap();
        fs::write(migrations.join("notes.txt"), "").unwrap();

        let files = MigrationParser::find_migration_files(dir.path().to_str().unwrap()).unwrap();
        let names: Vec<String> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();

        assert_eq!(names, vec!["20240101_A.cs", "20240202_B.cs"]);
    }

    // ─── sql_strings helpers ─────────────────────────────────────

    #[test]
    fn sql_strings_up_returns_just_strings() {
        let parsed = ParsedMigration {
            file_name: "x".to_string(),
            up_body: String::new(),
            down_body: String::new(),
            custom_sql_up: vec![stmt("A"), stmt("B")],
            custom_sql_down: vec![],
            has_custom_sql: true,
        };
        assert_eq!(parsed.sql_strings_up(), vec!["A".to_string(), "B".to_string()]);
        assert!(parsed.sql_strings_down().is_empty());
    }
}
