use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

/// Represents extracted data from a single migration file.
#[derive(Debug, Clone)]
pub struct ParsedMigration {
    pub file_name: String,
    pub up_body: String,
    pub down_body: String,
    pub custom_sql_up: Vec<String>,
    pub custom_sql_down: Vec<String>,
    pub has_custom_sql: bool,
}

pub struct MigrationParser;

impl MigrationParser {
    /// Find all migration .cs files in the Migrations directory (excludes .Designer.cs and snapshot).
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

    /// Locate the Migrations directory within a project.
    fn find_migrations_dir(project_path: &str) -> Result<PathBuf, String> {
        let base = Path::new(project_path);

        // Check common locations
        let candidates = [
            base.join("Migrations"),
            base.join("Data").join("Migrations"),
        ];

        for candidate in &candidates {
            if candidate.exists() && candidate.is_dir() {
                return Ok(candidate.clone());
            }
        }

        // Walk directory tree to find any Migrations folder
        if let Some(found) = Self::walk_for_migrations(base, 3) {
            return Ok(found);
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

    /// Parse a migration .cs file and extract Up/Down methods and custom SQL.
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

    /// Extract the body of an Up() or Down() method using brace matching.
    fn extract_method_body(content: &str, method_name: &str) -> String {
        // Match "protected override void Up(" or "protected override void Down("
        let pattern = format!(
            r"protected\s+override\s+void\s+{}\s*\(",
            regex::escape(method_name)
        );
        let re = Regex::new(&pattern).unwrap();

        if let Some(m) = re.find(content) {
            let after_signature = &content[m.end()..];
            // Find the opening brace after the method signature
            if let Some(brace_start) = after_signature.find('{') {
                let body_start = m.end() + brace_start;
                if let Some(body_end) = Self::find_matching_brace(content, body_start) {
                    return content[body_start + 1..body_end].trim().to_string();
                }
            }
        }

        String::new()
    }

    /// Find the matching closing brace for an opening brace at `start`.
    fn find_matching_brace(content: &str, start: usize) -> Option<usize> {
        let bytes = content.as_bytes();
        let mut depth = 0;
        let mut in_string = false;
        let mut in_verbatim = false;
        let mut prev_char = 0u8;

        for i in start..bytes.len() {
            let ch = bytes[i];

            // Handle C# string literals (simplified)
            if ch == b'"' && !in_verbatim {
                if prev_char == b'@' {
                    in_verbatim = true;
                    in_string = true;
                } else if prev_char != b'\\' {
                    in_string = !in_string;
                }
            } else if in_verbatim && ch == b'"' {
                // In verbatim strings, "" is an escaped quote
                if i + 1 < bytes.len() && bytes[i + 1] == b'"' {
                    // skip next
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

    /// Extract all migrationBuilder.Sql(...) calls from a method body.
    pub fn extract_custom_sql(method_body: &str) -> Vec<String> {
        let re = Regex::new(
            r#"migrationBuilder\s*\.\s*Sql\s*\(\s*@?"((?:[^"\\]|\\.|"")*?)"\s*\)"#,
        )
        .unwrap();

        let mut sqls = Vec::new();
        for cap in re.captures_iter(method_body) {
            if let Some(sql) = cap.get(1) {
                let sql_text = sql
                    .as_str()
                    .replace("\"\"", "\"") // Unescape C# verbatim string quotes
                    .replace("\\n", "\n")
                    .replace("\\r", "")
                    .replace("\\t", "\t");
                sqls.push(sql_text);
            }
        }

        // Also capture multi-line Sql() calls with string concatenation or raw strings
        let re_multiline = Regex::new(
            r#"migrationBuilder\s*\.\s*Sql\s*\(\s*\$?@"([\s\S]*?)(?:(?<!")"(?!"))\s*\)"#,
        );

        if let Ok(re_ml) = re_multiline {
            for cap in re_ml.captures_iter(method_body) {
                if let Some(sql) = cap.get(1) {
                    let sql_text = sql.as_str().replace("\"\"", "\"").trim().to_string();
                    if !sqls.contains(&sql_text) && !sql_text.is_empty() {
                        sqls.push(sql_text);
                    }
                }
            }
        }

        sqls
    }

    /// Inject custom SQL statements into a migration file's Up method.
    pub fn inject_custom_sql(file_path: &Path, sql_statements: &[String]) -> Result<(), String> {
        let content = fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read {}: {}", file_path.display(), e))?;

        let up_pattern =
            Regex::new(r"(protected\s+override\s+void\s+Up\s*\([^)]*\)\s*\{)").unwrap();

        if let Some(m) = up_pattern.find(&content) {
            let insert_pos = m.end();
            let mut injected_sql = String::new();

            for sql in sql_statements {
                let escaped = sql.replace('"', "\"\"");
                injected_sql.push_str(&format!(
                    "\n            migrationBuilder.Sql(@\"{}\");",
                    escaped
                ));
            }

            let new_content = format!(
                "{}{}{}",
                &content[..insert_pos],
                injected_sql,
                &content[insert_pos..]
            );

            fs::write(file_path, new_content)
                .map_err(|e| format!("Failed to write {}: {}", file_path.display(), e))?;

            Ok(())
        } else {
            Err("Could not find Up method in migration file".to_string())
        }
    }

    /// Get migration file path from a migration name/id.
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
