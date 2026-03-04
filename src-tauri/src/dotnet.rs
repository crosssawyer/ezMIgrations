use std::env;
use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub struct DotnetEf;

struct RunningEfProcess {
    child: Child,
    operation: String,
    canceled: bool,
}

fn running_process() -> &'static Mutex<Option<RunningEfProcess>> {
    static RUNNING_PROCESS: OnceLock<Mutex<Option<RunningEfProcess>>> = OnceLock::new();
    RUNNING_PROCESS.get_or_init(|| Mutex::new(None))
}

#[derive(Debug)]
pub struct CommandResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    /// The dotnet ef command that was executed (for diagnostics).
    pub command_display: String,
}

impl CommandResult {
    /// Return the most useful error text: stderr if non-empty, otherwise stdout.
    /// Appends the executed command for easier debugging.
    pub fn error_output(&self) -> String {
        let body = if self.stderr.trim().is_empty() {
            &self.stdout
        } else {
            &self.stderr
        };
        format!("{}\n\nExecuted: {}", body.trim(), self.command_display)
    }
}

impl DotnetEf {
    /// Build the Command and return a human-readable representation of it.
    fn build_ef_command(project_path: &str, args: &[&str], startup_project: &str) -> (Command, String) {
        let project = Path::new(project_path);

        // Derive the solution root (parent of the project directory) and run from there
        // using relative paths, e.g.: dotnet ef migrations remove --project cmms-data --startup-project cmms-api
        let solution_dir = project.parent();

        let mut cmd = Command::new("dotnet");

        // Hide the console window on Windows so it doesn't flash over the app.
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);

        // macOS GUI apps inherit a minimal PATH that doesn't include dotnet.
        // Enrich PATH with common dotnet install locations so the command resolves.
        if let Ok(current_path) = env::var("PATH") {
            let home = env::var("HOME").unwrap_or_default();
            let extra_paths = [
                format!("{}/.dotnet/tools", home),
                format!("{}/.dotnet", home),
                "/usr/local/share/dotnet".to_string(),
                "/usr/local/bin".to_string(),
                "/opt/homebrew/bin".to_string(),
            ];
            let enriched = extra_paths
                .iter()
                .chain(std::iter::once(&current_path))
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(":");
            cmd.env("PATH", enriched);
        }

        let mut display_parts: Vec<String> = vec!["dotnet".into(), "ef".into()];
        cmd.arg("ef");
        cmd.args(args);
        display_parts.extend(args.iter().map(|a| a.to_string()));

        if let Some(sol_dir) = solution_dir.filter(|p| p.as_os_str().len() > 0 && p.exists()) {
            cmd.current_dir(sol_dir);

            // Use path relative to solution root for --project
            if let Ok(rel) = project.strip_prefix(sol_dir) {
                cmd.arg("--project").arg(rel);
                display_parts.push("--project".into());
                display_parts.push(rel.to_string_lossy().to_string());
            } else {
                cmd.arg("--project").arg(project_path);
                display_parts.push("--project".into());
                display_parts.push(project_path.to_string());
            }

            // Use path relative to solution root for --startup-project
            if !startup_project.is_empty() {
                let sp = Path::new(startup_project);
                if let Ok(rel) = sp.strip_prefix(sol_dir) {
                    cmd.arg("--startup-project").arg(rel);
                    display_parts.push("--startup-project".into());
                    display_parts.push(rel.to_string_lossy().to_string());
                } else {
                    cmd.arg("--startup-project").arg(startup_project);
                    display_parts.push("--startup-project".into());
                    display_parts.push(startup_project.to_string());
                }
            }
        } else {
            cmd.arg("--project").arg(project_path);
            display_parts.push("--project".into());
            display_parts.push(project_path.to_string());
            if !startup_project.is_empty() {
                cmd.arg("--startup-project").arg(startup_project);
                display_parts.push("--startup-project".into());
                display_parts.push(startup_project.to_string());
            }
        }

        (cmd, display_parts.join(" "))
    }

    fn run_ef(
        project_path: &str,
        args: &[&str],
        startup_project: &str,
    ) -> Result<CommandResult, String> {
        let (mut cmd, command_display) = Self::build_ef_command(project_path, args, startup_project);
        cmd.output()
            .map(|output| CommandResult {
                success: output.status.success(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                command_display: command_display.clone(),
            })
            .map_err(|e| format!("Failed to execute dotnet ef: {}", e))
    }

    fn run_ef_cancellable(
        project_path: &str,
        args: &[&str],
        startup_project: &str,
        operation: &str,
    ) -> Result<CommandResult, String> {
        let (mut cmd, command_display) = Self::build_ef_command(project_path, args, startup_project);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to execute dotnet ef: {}", e))?;

        let mut child_stdout = child
            .stdout
            .take()
            .ok_or("Failed to capture dotnet ef stdout")?;
        let mut child_stderr = child
            .stderr
            .take()
            .ok_or("Failed to capture dotnet ef stderr")?;

        let stdout_reader = thread::spawn(move || {
            let mut buffer = Vec::new();
            let _ = child_stdout.read_to_end(&mut buffer);
            buffer
        });
        let stderr_reader = thread::spawn(move || {
            let mut buffer = Vec::new();
            let _ = child_stderr.read_to_end(&mut buffer);
            buffer
        });

        {
            let mut guard = running_process()
                .lock()
                .map_err(|_| "Failed to lock running operation state".to_string())?;
            if guard.is_some() {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err("Another operation is already running".to_string());
            }

            *guard = Some(RunningEfProcess {
                child,
                operation: operation.to_string(),
                canceled: false,
            });
        }

        let exit_status = loop {
            let maybe_status = {
                let mut guard = running_process()
                    .lock()
                    .map_err(|_| "Failed to lock running operation state".to_string())?;
                let running = guard
                    .as_mut()
                    .ok_or("Running operation disappeared unexpectedly")?;
                running
                    .child
                    .try_wait()
                    .map_err(|e| format!("Failed while waiting for dotnet ef: {}", e))?
            };

            if let Some(status) = maybe_status {
                break status;
            }

            thread::sleep(Duration::from_millis(120));
        };

        let canceled = {
            let mut guard = running_process()
                .lock()
                .map_err(|_| "Failed to lock running operation state".to_string())?;
            if let Some(mut running) = guard.take() {
                let _ = running.child.wait();
                running.canceled
            } else {
                false
            }
        };

        let stdout = String::from_utf8_lossy(&stdout_reader.join().unwrap_or_default()).to_string();
        let mut stderr =
            String::from_utf8_lossy(&stderr_reader.join().unwrap_or_default()).to_string();

        if canceled {
            if !stderr.trim().is_empty() {
                stderr.push('\n');
            }
            stderr.push_str("Operation canceled by user.");
        }

        Ok(CommandResult {
            success: exit_status.success() && !canceled,
            stdout,
            stderr,
            command_display,
        })
    }

    pub fn cancel_running_operation() -> Result<String, String> {
        let mut guard = running_process()
            .lock()
            .map_err(|_| "Failed to lock running operation state".to_string())?;
        let running = guard
            .as_mut()
            .ok_or("No cancelable operation is currently running")?;

        running.canceled = true;
        running
            .child
            .kill()
            .map_err(|e| format!("Failed to cancel '{}': {}", running.operation, e))?;

        Ok(format!("Cancel requested for '{}'", running.operation))
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
        Self::run_ef_cancellable(project_path, &args, startup_project, "add migration")
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
        Self::run_ef_cancellable(project_path, &args, startup_project, "remove migration")
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
        Self::run_ef_cancellable(project_path, &args, startup_project, "update database")
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
        Self::run_ef_cancellable(project_path, &args, startup_project, "update database")
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
