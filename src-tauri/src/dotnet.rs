use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use crate::process::command;

/// GUI apps inherit a minimal/stale PATH from the OS shell, which often misses
/// the directories where `dotnet` and `dotnet-ef` live. We prepend the usual
/// install locations so spawned commands can find them. The path separator and
/// candidate directories differ per-platform.
#[cfg(not(target_os = "windows"))]
fn compute_enriched_path(current_path: &str, home: &str) -> String {
    let extra_paths = [
        format!("{}/.dotnet/tools", home),
        format!("{}/.dotnet", home),
        "/usr/local/share/dotnet".to_string(),
        "/usr/local/bin".to_string(),
        "/opt/homebrew/bin".to_string(),
    ];
    extra_paths
        .iter()
        .map(|s| s.as_str())
        .chain(std::iter::once(current_path))
        .collect::<Vec<_>>()
        .join(":")
}

#[cfg(target_os = "windows")]
fn compute_enriched_path(current_path: &str, user_profile: &str, program_files: &str) -> String {
    let extra_paths = [
        format!("{}\\.dotnet\\tools", user_profile),
        format!("{}\\dotnet", program_files),
    ];
    extra_paths
        .iter()
        .map(|s| s.as_str())
        .chain(std::iter::once(current_path))
        .collect::<Vec<_>>()
        .join(";")
}

#[cfg(not(target_os = "windows"))]
fn enrich_path(cmd: &mut Command) {
    let Ok(current_path) = env::var("PATH") else {
        return;
    };
    let home = env::var("HOME").unwrap_or_default();
    cmd.env("PATH", compute_enriched_path(&current_path, &home));
}

#[cfg(target_os = "windows")]
fn enrich_path(cmd: &mut Command) {
    let Ok(current_path) = env::var("PATH") else {
        return;
    };
    let user_profile = env::var("USERPROFILE").unwrap_or_default();
    let program_files =
        env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".to_string());
    cmd.env(
        "PATH",
        compute_enriched_path(&current_path, &user_profile, &program_files),
    );
}

/// Scan sibling directories of the migrations project for a candidate startup
/// `.csproj`. EF's design-time DbContext factory usually lives in the API/host
/// project, not the data project, so when the user didn't configure one we
/// look for `Microsoft.NET.Sdk.Web` first and fall back to any sibling csproj.
fn auto_detect_startup_project(project_path: &str) -> Option<String> {
    let project = Path::new(project_path);
    let project_dir: PathBuf = if project.is_file() {
        project.parent()?.to_path_buf()
    } else {
        project.to_path_buf()
    };
    let solution_dir = project_dir.parent()?;

    let mut candidates: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(solution_dir).ok()?.flatten() {
        let dir = entry.path();
        if !dir.is_dir() || dir == project_dir {
            continue;
        }
        let Ok(sub_entries) = fs::read_dir(&dir) else {
            continue;
        };
        for sub in sub_entries.flatten() {
            let p = sub.path();
            if p.extension()
                .and_then(|e| e.to_str())
                .map(|s| s.eq_ignore_ascii_case("csproj"))
                .unwrap_or(false)
            {
                candidates.push(p);
            }
        }
    }

    for candidate in &candidates {
        if let Ok(content) = fs::read_to_string(candidate) {
            if content.contains("Microsoft.NET.Sdk.Web") {
                return candidate
                    .parent()
                    .map(|p| p.to_string_lossy().into_owned());
            }
        }
    }

    candidates
        .first()
        .and_then(|c| c.parent())
        .map(|p| p.to_string_lossy().into_owned())
}

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

        // Fall back to a sibling project (typically the ASP.NET host) when the
        // user didn't configure a startup project. Without it, EF often can't
        // resolve the DbContext's DI options and silently returns no output.
        let effective_startup: String = if startup_project.is_empty() {
            auto_detect_startup_project(project_path).unwrap_or_default()
        } else {
            startup_project.to_string()
        };

        let mut cmd = command("dotnet");
        enrich_path(&mut cmd);
        // Force English so our stdout parser isn't defeated by localized
        // .NET output on non-English Windows installs.
        cmd.env("DOTNET_CLI_UI_LANGUAGE", "en");

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
            if !effective_startup.is_empty() {
                let sp = Path::new(&effective_startup);
                if let Ok(rel) = sp.strip_prefix(sol_dir) {
                    cmd.arg("--startup-project").arg(rel);
                    display_parts.push("--startup-project".into());
                    display_parts.push(rel.to_string_lossy().to_string());
                } else {
                    cmd.arg("--startup-project").arg(&effective_startup);
                    display_parts.push("--startup-project".into());
                    display_parts.push(effective_startup.clone());
                }
            }
        } else {
            cmd.arg("--project").arg(project_path);
            display_parts.push("--project".into());
            display_parts.push(project_path.to_string());
            if !effective_startup.is_empty() {
                cmd.arg("--startup-project").arg(&effective_startup);
                display_parts.push("--startup-project".into());
                display_parts.push(effective_startup.clone());
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
            .map(|output| {
                let mut result = CommandResult {
                    success: output.status.success(),
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    command_display: command_display.clone(),
                };
                Self::enrich_with_build_diagnostics(&mut result, project_path, startup_project);
                result
            })
            .map_err(|e| format!("Failed to execute dotnet ef: {}", e))
    }

    /// When `dotnet ef` reports "Build failed. Use dotnet build to see the errors.",
    /// run a follow-up `dotnet build` against the same projects and append the
    /// extracted error lines so callers see the actual compile errors instead of
    /// the unhelpful EF message.
    fn enrich_with_build_diagnostics(
        result: &mut CommandResult,
        project_path: &str,
        startup_project: &str,
    ) {
        if result.success {
            return;
        }
        let combined = format!("{}\n{}", result.stdout, result.stderr);
        if !combined.contains("Build failed") {
            return;
        }
        // Prefer the startup project (what EF actually compiles to load DbContext);
        // fall back to the migrations project if no startup project is configured.
        let build_target = if !startup_project.is_empty() {
            startup_project
        } else {
            project_path
        };
        let build_output = Self::run_dotnet_build(build_target);
        if build_output.trim().is_empty() {
            return;
        }
        let separator = "\n\n--- dotnet build errors ---\n";
        if !result.stderr.trim().is_empty() {
            result.stderr.push_str(separator);
            result.stderr.push_str(&build_output);
        } else {
            result.stdout.push_str(separator);
            result.stdout.push_str(&build_output);
        }
    }

    /// Run `dotnet build` against a project and return only the diagnostic lines
    /// (errors and the final FAILED summary). Always returns even on success so
    /// the caller gets *something* useful when EF claims a build failed.
    fn run_dotnet_build(project_path: &str) -> String {
        let project = Path::new(project_path);
        let (cwd, target): (Option<&Path>, String) = if let Some(parent) = project.parent() {
            if parent.as_os_str().is_empty() || !parent.exists() {
                (None, project_path.to_string())
            } else if let Ok(rel) = project.strip_prefix(parent) {
                (Some(parent), rel.to_string_lossy().to_string())
            } else {
                (Some(parent), project_path.to_string())
            }
        } else {
            (None, project_path.to_string())
        };

        let mut cmd = command("dotnet");
        enrich_path(&mut cmd);
        cmd.arg("build").arg(&target).arg("--nologo");
        // -clp:ErrorsOnly limits the console logger to error-level messages.
        cmd.arg("-clp:ErrorsOnly");
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }

        let output = match cmd.output() {
            Ok(o) => o,
            Err(e) => return format!("(Failed to run dotnet build for diagnostics: {})", e),
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // -clp:ErrorsOnly trims most noise, but we still filter empties and the
        // usage banner from `dotnet build --help`-style fallthroughs.
        let mut lines: Vec<&str> = stdout
            .lines()
            .chain(stderr.lines())
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
        // Cap output so a very chatty build can't swamp the error dialog.
        const MAX_LINES: usize = 40;
        let truncated = lines.len() > MAX_LINES;
        if truncated {
            lines.truncate(MAX_LINES);
        }
        let mut joined = lines.join("\n");
        if truncated {
            joined.push_str("\n… (output truncated)");
        }
        joined
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

        let mut result = CommandResult {
            success: exit_status.success() && !canceled,
            stdout,
            stderr,
            command_display,
        };
        if !canceled {
            Self::enrich_with_build_diagnostics(&mut result, project_path, startup_project);
        }
        Ok(result)
    }

    /// Try to kill the running EF child, if any. Returns the operation name
    /// that was killed, or None when nothing was running.
    pub fn cancel_running_operation() -> Result<Option<String>, String> {
        let mut guard = running_process()
            .lock()
            .map_err(|_| "Failed to lock running operation state".to_string())?;
        let Some(running) = guard.as_mut() else {
            return Ok(None);
        };

        running.canceled = true;
        running
            .child
            .kill()
            .map_err(|e| format!("Failed to cancel '{}': {}", running.operation, e))?;

        Ok(Some(running.operation.clone()))
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

        // EF sometimes exits 0 even when it couldn't actually load the DbContext —
        // it just prints the error to stdout and lists no migrations. Detect the
        // common signatures and surface them so the user sees a real error instead
        // of an empty migrations table.
        for line in result.stdout.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("Unable to create")
                || trimmed.contains("Unable to resolve service")
                || trimmed.contains("No DbContext was found")
                || trimmed.contains("More than one DbContext was found")
            {
                return Err(format!(
                    "EF couldn't load the DbContext at design time. \
                     This usually means the startup project is missing or wrong, \
                     or its configuration (connection string, DI registrations) \
                     isn't available at design time.\n\n{}",
                    trimmed
                ));
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn result(stdout: &str, stderr: &str) -> CommandResult {
        CommandResult {
            success: false,
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
            command_display: "dotnet ef migrations list".to_string(),
        }
    }

    #[test]
    fn error_output_prefers_stderr_when_present() {
        let r = result("stdout text", "stderr text");
        let out = r.error_output();
        assert!(out.contains("stderr text"));
        assert!(!out.contains("stdout text"));
    }

    #[test]
    fn error_output_falls_back_to_stdout_when_stderr_empty() {
        let r = result("stdout text", "");
        let out = r.error_output();
        assert!(out.contains("stdout text"));
    }

    #[test]
    fn error_output_falls_back_to_stdout_when_stderr_whitespace_only() {
        let r = result("the real error", "   \n\t  ");
        let out = r.error_output();
        assert!(out.contains("the real error"));
    }

    #[test]
    fn error_output_appends_command_display() {
        let r = result("err", "");
        let out = r.error_output();
        assert!(out.contains("Executed: dotnet ef migrations list"));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn enriched_path_prepends_extra_dirs_in_order() {
        let enriched = compute_enriched_path("/usr/bin:/bin", "/Users/alice");
        let parts: Vec<&str> = enriched.split(':').collect();
        assert_eq!(parts[0], "/Users/alice/.dotnet/tools");
        assert_eq!(parts[1], "/Users/alice/.dotnet");
        assert_eq!(parts[2], "/usr/local/share/dotnet");
        assert_eq!(parts[3], "/usr/local/bin");
        assert_eq!(parts[4], "/opt/homebrew/bin");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn enriched_path_preserves_existing_path_at_end() {
        let enriched = compute_enriched_path("/usr/bin:/bin", "/Users/alice");
        assert!(enriched.ends_with(":/usr/bin:/bin"));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn enriched_path_joins_with_colons_not_semicolons() {
        // Regression: forcing colon-joined PATH onto Windows breaks subprocess
        // env (it splits on ";"). Helper is only compiled on non-windows, but
        // assert the separator anyway so the contract is locked in.
        let enriched = compute_enriched_path("/usr/bin", "/home/x");
        assert!(!enriched.contains(';'));
        assert!(enriched.contains(':'));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn enriched_path_handles_empty_home() {
        let enriched = compute_enriched_path("/usr/bin", "");
        assert!(enriched.starts_with("/.dotnet/tools"));
        assert!(enriched.ends_with(":/usr/bin"));
    }
}
