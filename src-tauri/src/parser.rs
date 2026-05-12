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
