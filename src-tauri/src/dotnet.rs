use std::path::Path;
use std::process::Command;

pub struct DotnetEf;

#[derive(Debug)]
pub struct CommandResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

impl CommandResult {
    /// Return the most useful error text: stderr if non-empty, otherwise stdout.
    pub fn error_output(&self) -> &str {
        if self.stderr.trim().is_empty() {
            &self.stdout
        } else {
            &self.stderr
        }
    }
}

impl DotnetEf {
    fn run_ef(
        project_path: &str,
        args: &[&str],
        startup_project: &str,
    ) -> Result<CommandResult, String> {
        let project = Path::new(project_path);

        // Derive the solution root (parent of the project directory) and run from there
        // using relative paths, e.g.: dotnet ef migrations remove --project cmms-data --startup-project cmms-api
        let solution_dir = project.parent();

        let mut cmd = Command::new("dotnet");
        cmd.arg("ef");
        cmd.args(args);

        if let Some(sol_dir) = solution_dir.filter(|p| p.as_os_str().len() > 0 && p.exists()) {
            cmd.current_dir(sol_dir);

            // Use path relative to solution root for --project
            if let Ok(rel) = project.strip_prefix(sol_dir) {
                cmd.arg("--project").arg(rel);
            } else {
                cmd.arg("--project").arg(project_path);
            }

            // Use path relative to solution root for --startup-project
            if !startup_project.is_empty() {
                let sp = Path::new(startup_project);
                if let Ok(rel) = sp.strip_prefix(sol_dir) {
                    cmd.arg("--startup-project").arg(rel);
                } else {
                    cmd.arg("--startup-project").arg(startup_project);
                }
            }
        } else {
            cmd.arg("--project").arg(project_path);
            if !startup_project.is_empty() {
                cmd.arg("--startup-project").arg(startup_project);
            }
        }

        cmd.output()
            .map(|output| CommandResult {
                success: output.status.success(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            })
            .map_err(|e| format!("Failed to execute dotnet ef: {}", e))
    }

    /// List all migrations and their applied status.
    /// Uses `dotnet ef migrations list` which marks applied ones.
    pub fn list_migrations(
        project_path: &str,
        db_context: &str,
        startup_project: &str,
    ) -> Result<Vec<(String, bool)>, String> {
        let mut args = vec!["migrations", "list"];
        if !db_context.is_empty() {
            args.push("--context");
            args.push(db_context);
        }

        let result = Self::run_ef(project_path, &args, startup_project)?;

        if !result.success {
            return Err(format!(
                "dotnet ef migrations list failed: {}",
                result.stderr
            ));
        }

        let mut migrations = Vec::new();
        for line in result.stdout.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Skip EF Core noise: warnings, errors, info, and known preamble lines
            if trimmed.starts_with("Build")
                || trimmed.starts_with("Done")
                || trimmed.starts_with("The following")
                || trimmed.starts_with("Using")
                || trimmed.starts_with("Finding")
                || trimmed.starts_with("warn:")
                || trimmed.starts_with("info:")
                || trimmed.starts_with("error:")
                || trimmed.starts_with("fail:")
                || trimmed.starts_with("An error")
                || trimmed.starts_with("No store type")
                || trimmed.contains("Microsoft.EntityFrameworkCore")
                || trimmed.contains("provider:")
                || trimmed.contains("silently truncated")
                || trimmed.contains("HasColumnType")
                || trimmed.contains("HasPrecision")
                || trimmed.contains("HasConversion")
                || trimmed.contains("NUMERIC_ROUNDABORT")
                || trimmed.contains("network-related or instance-specific")
            {
                continue;
            }

            // EF Core migration names always start with a numeric timestamp (e.g. "20230101120000_InitialCreate")
            let name_part = trimmed.replace("(Pending)", "");
            let name_part = name_part.trim();
            if name_part.is_empty() || !name_part.starts_with(|c: char| c.is_ascii_digit()) {
                continue;
            }

            // Applied migrations are listed normally, pending ones have "(Pending)" suffix
            if trimmed.contains("(Pending)") {
                let name = trimmed.replace("(Pending)", "").trim().to_string();
                if !name.is_empty() {
                    migrations.push((name, false));
                }
            } else {
                migrations.push((trimmed.to_string(), true));
            }
        }

        Ok(migrations)
    }

    /// Add a new migration.
    pub fn add_migration(
        project_path: &str,
        name: &str,
        db_context: &str,
        startup_project: &str,
    ) -> Result<CommandResult, String> {
        let mut args = vec!["migrations", "add", name];
        if !db_context.is_empty() {
            args.push("--context");
            args.push(db_context);
        }
        Self::run_ef(project_path, &args, startup_project)
    }

    /// Remove the last migration.
    pub fn remove_migration(
        project_path: &str,
        db_context: &str,
        startup_project: &str,
        force: bool,
    ) -> Result<CommandResult, String> {
        let mut args = vec!["migrations", "remove"];
        if !db_context.is_empty() {
            args.push("--context");
            args.push(db_context);
        }
        if force {
            args.push("--force");
        }
        Self::run_ef(project_path, &args, startup_project)
    }

    /// Update the database to a specific migration (or latest if target is empty).
    pub fn update_database(
        project_path: &str,
        target: &str,
        db_context: &str,
        startup_project: &str,
    ) -> Result<CommandResult, String> {
        let mut args = vec!["database", "update"];
        if !target.is_empty() {
            args.push(target);
        }
        if !db_context.is_empty() {
            args.push("--context");
            args.push(db_context);
        }
        Self::run_ef(project_path, &args, startup_project)
    }

    /// Update the database without rebuilding — uses the already-compiled assembly.
    /// Useful for reverting after a branch switch when the old .cs files are gone
    /// but bin/obj still has the compiled Down() methods.
    pub fn update_database_no_build(
        project_path: &str,
        target: &str,
        db_context: &str,
        startup_project: &str,
    ) -> Result<CommandResult, String> {
        let mut args = vec!["database", "update"];
        if !target.is_empty() {
            args.push(target);
        }
        if !db_context.is_empty() {
            args.push("--context");
            args.push(db_context);
        }
        args.push("--no-build");
        Self::run_ef(project_path, &args, startup_project)
    }

    /// Generate SQL script between two migrations.
    pub fn script_migration(
        project_path: &str,
        from: &str,
        to: &str,
        db_context: &str,
        startup_project: &str,
    ) -> Result<CommandResult, String> {
        let mut args = vec!["migrations", "script"];
        if !from.is_empty() {
            args.push(from);
        }
        if !to.is_empty() {
            args.push(to);
        }
        if !db_context.is_empty() {
            args.push("--context");
            args.push(db_context);
        }
        Self::run_ef(project_path, &args, startup_project)
    }
}
