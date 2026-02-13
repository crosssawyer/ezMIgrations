use std::path::Path;
use std::process::Command;

pub struct DotnetEf;

#[derive(Debug)]
pub struct CommandResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

impl DotnetEf {
    fn run_ef(
        project_path: &str,
        args: &[&str],
        startup_project: &str,
    ) -> Result<CommandResult, String> {
        let mut cmd = Command::new("dotnet");
        cmd.arg("ef");
        cmd.args(args);
        cmd.arg("--project").arg(project_path);

        if !startup_project.is_empty() {
            cmd.arg("--startup-project").arg(startup_project);
        }

        // Run from the parent directory if project_path is relative
        if let Some(parent) = Path::new(project_path).parent() {
            if parent.exists() {
                cmd.current_dir(parent);
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
        args.push("--no-build");

        let result = Self::run_ef(project_path, &args, startup_project);

        // If --no-build fails, retry with build
        let result = match result {
            Ok(r) if !r.success => {
                let mut args2 = vec!["migrations", "list"];
                if !db_context.is_empty() {
                    args2.push("--context");
                    args2.push(db_context);
                }
                Self::run_ef(project_path, &args2, startup_project)?
            }
            Ok(r) => r,
            Err(e) => return Err(e),
        };

        if !result.success {
            return Err(format!(
                "dotnet ef migrations list failed: {}",
                result.stderr
            ));
        }

        let mut migrations = Vec::new();
        for line in result.stdout.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("Build") || trimmed.starts_with("Done") {
                continue;
            }
            // Applied migrations are listed normally, pending ones have "(Pending)" suffix
            if trimmed.contains("(Pending)") {
                let name = trimmed.replace("(Pending)", "").trim().to_string();
                if !name.is_empty() {
                    migrations.push((name, false));
                }
            } else if !trimmed.is_empty()
                && !trimmed.starts_with("The following")
                && !trimmed.starts_with("Using")
                && !trimmed.starts_with("Finding")
            {
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
