use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct TerminalContext<'a> {
    pub repo_path: &'a str,
    pub project_path: &'a str,
    pub mcp_url: &'a str,
    pub port_file_path: &'a str,
}

pub fn open_terminal(kind: &str, ctx: &TerminalContext<'_>) -> Result<String, String> {
    let kind = normalize_kind(kind);

    #[cfg(target_os = "macos")]
    {
        return open_macos_terminal(kind, ctx);
    }

    #[cfg(target_os = "windows")]
    {
        return open_windows_terminal(kind, ctx);
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        return open_linux_terminal(kind, ctx);
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", unix)))]
    {
        let _ = (kind, ctx);
        Err("Opening a terminal is not supported on this platform.".to_string())
    }
}

fn normalize_kind(kind: &str) -> &str {
    match kind.trim() {
        "" => "system",
        other => other,
    }
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn write_bootstrap(extension: &str, body: &str) -> Result<PathBuf, String> {
    let path = std::env::temp_dir().join(format!(
        "ezmigrations-mcp-{}-{}.{}",
        std::process::id(),
        now_unix_ms(),
        extension
    ));
    fs::write(&path, body).map_err(|e| format!("Failed to write terminal bootstrap: {e}"))?;
    Ok(path)
}

#[cfg(any(unix, test))]
fn sh_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(any(unix, test))]
fn shell_bootstrap(ctx: &TerminalContext<'_>) -> String {
    format!(
        r#"#!/usr/bin/env bash
cd {repo} || exit 1
export EZMIGRATIONS_MCP_URL={url}
export EZMIGRATIONS_MCP_PORT_FILE={port_file}
export EZMIGRATIONS_PROJECT_PATH={project}

claude_status="Claude Code not found on PATH; MCP URL exported for manual setup."
if command -v claude >/dev/null 2>&1; then
  if claude mcp add --scope local --transport http ezmigrations "$EZMIGRATIONS_MCP_URL" >/dev/null 2>&1; then
    claude_status="Claude MCP: ezmigrations registered for this repo"
  else
    check_name="ezmigrations-check-$$"
    if claude mcp add --scope local --transport http "$check_name" "$EZMIGRATIONS_MCP_URL" >/dev/null 2>&1; then
      claude mcp remove --scope local "$check_name" >/dev/null 2>&1 || true
      claude mcp remove --scope local ezmigrations >/dev/null 2>&1 || true
      if claude mcp add --scope local --transport http ezmigrations "$EZMIGRATIONS_MCP_URL" >/dev/null 2>&1; then
        claude_status="Claude MCP: ezmigrations registered for this repo"
      else
        claude_status="Claude MCP: replacement failed after validation; run: claude mcp add --scope local --transport http ezmigrations $EZMIGRATIONS_MCP_URL"
      fi
    else
      claude_status="Claude MCP: registration failed; existing ezmigrations entry left unchanged"
    fi
  fi
fi

clear
printf '\033[1;32mezMigrations MCP ready\033[0m\n'
printf '%s\n' "$claude_status"
printf 'MCP URL: %s\n' "$EZMIGRATIONS_MCP_URL"
printf 'Project: %s\n' "$EZMIGRATIONS_PROJECT_PATH"
printf 'Repo: %s\n\n' "$PWD"
exec "${{SHELL:-/bin/bash}}" -l
"#,
        repo = sh_quote(ctx.repo_path),
        url = sh_quote(ctx.mcp_url),
        port_file = sh_quote(ctx.port_file_path),
        project = sh_quote(ctx.project_path),
    )
}

#[cfg(unix)]
fn shell_bootstrap_path(ctx: &TerminalContext<'_>) -> Result<PathBuf, String> {
    write_bootstrap("sh", &shell_bootstrap(ctx))
}

#[cfg(target_os = "macos")]
fn apple_string(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    )
}

#[cfg(target_os = "macos")]
fn run_osascript(lines: &[String]) -> Result<(), String> {
    let mut cmd = Command::new("osascript");
    for line in lines {
        cmd.arg("-e").arg(line);
    }
    let status = cmd
        .status()
        .map_err(|e| format!("Failed to run osascript: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("Terminal could not be opened.".to_string())
    }
}

#[cfg(target_os = "macos")]
fn open_macos_terminal(kind: &str, ctx: &TerminalContext<'_>) -> Result<String, String> {
    let script = shell_bootstrap_path(ctx)?;
    let command = format!("clear; /bin/bash {}", sh_quote(&script.to_string_lossy()));

    match kind {
        "system" | "terminal" => {
            run_osascript(&[
                "tell application \"Terminal\"".to_string(),
                format!("do script {}", apple_string(&command)),
                "activate".to_string(),
                "end tell".to_string(),
            ])?;
            Ok("Terminal".to_string())
        }
        "iterm" | "iterm2" => {
            let command = apple_string(&command);
            let try_iterm2 = run_osascript(&[
                "tell application \"iTerm2\"".to_string(),
                "activate".to_string(),
                format!("create window with default profile command {}", command),
                "end tell".to_string(),
            ]);
            if try_iterm2.is_err() {
                run_osascript(&[
                    "tell application \"iTerm\"".to_string(),
                    "activate".to_string(),
                    format!("create window with default profile command {}", command),
                    "end tell".to_string(),
                ])?;
            }
            Ok("iTerm2".to_string())
        }
        other => Err(format!("Unknown terminal option: {other}")),
    }
}

#[cfg(target_os = "windows")]
fn ps_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(target_os = "windows")]
fn cmd_value(value: &str) -> String {
    value
        .replace('%', "%%")
        .replace('^', "^^")
        .replace('&', "^&")
        .replace('|', "^|")
        .replace('<', "^<")
        .replace('>', "^>")
        .replace('"', "\"\"")
}

#[cfg(target_os = "windows")]
fn powershell_bootstrap(ctx: &TerminalContext<'_>) -> String {
    format!(
        r#"$env:EZMIGRATIONS_MCP_URL = {url}
$env:EZMIGRATIONS_MCP_PORT_FILE = {port_file}
$env:EZMIGRATIONS_PROJECT_PATH = {project}
Set-Location -LiteralPath {repo}

$claudeStatus = 'Claude Code not found on PATH; MCP URL exported for manual setup.'
if (Get-Command claude -ErrorAction SilentlyContinue) {{
  claude mcp add --scope local --transport http ezmigrations $env:EZMIGRATIONS_MCP_URL *> $null
  if ($LASTEXITCODE -eq 0) {{
    $claudeStatus = 'Claude MCP: ezmigrations registered for this repo'
  }} else {{
    $checkName = 'ezmigrations-check-' + $PID
    claude mcp add --scope local --transport http $checkName $env:EZMIGRATIONS_MCP_URL *> $null
    if ($LASTEXITCODE -eq 0) {{
      claude mcp remove --scope local $checkName *> $null
      claude mcp remove --scope local ezmigrations *> $null
      claude mcp add --scope local --transport http ezmigrations $env:EZMIGRATIONS_MCP_URL *> $null
      if ($LASTEXITCODE -eq 0) {{
        $claudeStatus = 'Claude MCP: ezmigrations registered for this repo'
      }} else {{
        $claudeStatus = 'Claude MCP: replacement failed after validation; run: claude mcp add --scope local --transport http ezmigrations ' + $env:EZMIGRATIONS_MCP_URL
      }}
    }} else {{
      $claudeStatus = 'Claude MCP: registration failed; existing ezmigrations entry left unchanged'
    }}
  }}
}}

Clear-Host
Write-Host 'ezMigrations MCP ready' -ForegroundColor Green
Write-Host $claudeStatus
Write-Host ('MCP URL: ' + $env:EZMIGRATIONS_MCP_URL)
Write-Host ('Project: ' + $env:EZMIGRATIONS_PROJECT_PATH)
Write-Host ('Repo: ' + (Get-Location))
"#,
        url = ps_quote(ctx.mcp_url),
        port_file = ps_quote(ctx.port_file_path),
        project = ps_quote(ctx.project_path),
        repo = ps_quote(ctx.repo_path),
    )
}

#[cfg(target_os = "windows")]
fn cmd_bootstrap(ctx: &TerminalContext<'_>) -> String {
    format!(
        r#"@echo off
setlocal EnableExtensions EnableDelayedExpansion
cd /d "{repo}"
set "EZMIGRATIONS_MCP_URL={url}"
set "EZMIGRATIONS_MCP_PORT_FILE={port_file}"
set "EZMIGRATIONS_PROJECT_PATH={project}"

set "CLAUDE_STATUS=Claude Code not found on PATH; MCP URL exported for manual setup."
where claude >nul 2>nul
if errorlevel 1 (
  rem Claude Code is not installed or not on PATH.
) else (
  claude mcp add --scope local --transport http ezmigrations "%EZMIGRATIONS_MCP_URL%" >nul 2>nul
  if errorlevel 1 (
    set "CHECK_NAME=ezmigrations-check-%RANDOM%"
    claude mcp add --scope local --transport http "!CHECK_NAME!" "%EZMIGRATIONS_MCP_URL%" >nul 2>nul
    if errorlevel 1 (
      set "CLAUDE_STATUS=Claude MCP: registration failed; existing ezmigrations entry left unchanged"
    ) else (
      claude mcp remove --scope local "!CHECK_NAME!" >nul 2>nul
      claude mcp remove --scope local ezmigrations >nul 2>nul
      claude mcp add --scope local --transport http ezmigrations "%EZMIGRATIONS_MCP_URL%" >nul 2>nul
      if errorlevel 1 (
        set "CLAUDE_STATUS=Claude MCP: replacement failed after validation; run: claude mcp add --scope local --transport http ezmigrations %EZMIGRATIONS_MCP_URL%"
      ) else (
        set "CLAUDE_STATUS=Claude MCP: ezmigrations registered for this repo"
      )
    )
  ) else (
    set "CLAUDE_STATUS=Claude MCP: ezmigrations registered for this repo"
  )
)

cls
echo ezMigrations MCP ready
echo !CLAUDE_STATUS!
echo MCP URL: %EZMIGRATIONS_MCP_URL%
echo Project: %EZMIGRATIONS_PROJECT_PATH%
echo Repo: %CD%
"#,
        repo = cmd_value(ctx.repo_path),
        url = cmd_value(ctx.mcp_url),
        port_file = cmd_value(ctx.port_file_path),
        project = cmd_value(ctx.project_path),
    )
}

#[cfg(target_os = "windows")]
fn open_windows_terminal(kind: &str, ctx: &TerminalContext<'_>) -> Result<String, String> {
    let ps_script = write_bootstrap("ps1", &powershell_bootstrap(ctx))?;

    match kind {
        "system" | "windows_terminal" | "wt" => {
            let mut cmd = Command::new("wt");
            cmd.arg("-d")
                .arg(ctx.repo_path)
                .arg("powershell.exe")
                .arg("-NoExit")
                .arg("-ExecutionPolicy")
                .arg("Bypass")
                .arg("-File")
                .arg(&ps_script);
            if cmd.spawn().is_ok() {
                return Ok("Windows Terminal".to_string());
            }
            open_windows_terminal("powershell", ctx)
        }
        "powershell" => {
            Command::new("powershell.exe")
                .current_dir(ctx.repo_path)
                .arg("-NoExit")
                .arg("-ExecutionPolicy")
                .arg("Bypass")
                .arg("-File")
                .arg(ps_script)
                .spawn()
                .map_err(|e| format!("Failed to open PowerShell: {e}"))?;
            Ok("PowerShell".to_string())
        }
        "cmd" => {
            let cmd_script = write_bootstrap("cmd", &cmd_bootstrap(ctx))?;
            Command::new("cmd.exe")
                .current_dir(ctx.repo_path)
                .arg("/K")
                .arg(cmd_script)
                .spawn()
                .map_err(|e| format!("Failed to open Command Prompt: {e}"))?;
            Ok("Command Prompt".to_string())
        }
        other => Err(format!("Unknown terminal option: {other}")),
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn spawn_linux_terminal(
    program: &str,
    args: &[&str],
    ctx: &TerminalContext<'_>,
) -> Result<(), String> {
    Command::new(program)
        .current_dir(ctx.repo_path)
        .args(args)
        .spawn()
        .map_err(|e| format!("Failed to open {program}: {e}"))?;
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_linux_terminal(kind: &str, ctx: &TerminalContext<'_>) -> Result<String, String> {
    let script = shell_bootstrap_path(ctx)?;
    let script = script.to_string_lossy().to_string();

    match kind {
        "system" => {
            if let Ok(term) = std::env::var("TERMINAL") {
                if spawn_linux_terminal(&term, &["-e", "bash", &script], ctx).is_ok() {
                    return Ok(term);
                }
            }
            for (label, program, args) in [
                (
                    "GNOME Terminal",
                    "gnome-terminal",
                    vec!["--working-directory", ctx.repo_path, "--", "bash", &script],
                ),
                (
                    "Konsole",
                    "konsole",
                    vec!["--workdir", ctx.repo_path, "-e", "bash", &script],
                ),
                ("xterm", "xterm", vec!["-e", "bash", &script]),
            ] {
                if spawn_linux_terminal(program, &args, ctx).is_ok() {
                    return Ok(label.to_string());
                }
            }
            Err("No supported terminal emulator was found.".to_string())
        }
        "gnome" | "gnome_terminal" => {
            spawn_linux_terminal(
                "gnome-terminal",
                &["--working-directory", ctx.repo_path, "--", "bash", &script],
                ctx,
            )?;
            Ok("GNOME Terminal".to_string())
        }
        "konsole" => {
            spawn_linux_terminal(
                "konsole",
                &["--workdir", ctx.repo_path, "-e", "bash", &script],
                ctx,
            )?;
            Ok("Konsole".to_string())
        }
        "xterm" => {
            spawn_linux_terminal("xterm", &["-e", "bash", &script], ctx)?;
            Ok("xterm".to_string())
        }
        other => Err(format!("Unknown terminal option: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> TerminalContext<'static> {
        TerminalContext {
            repo_path: "/tmp/repo with spaces",
            project_path: "/tmp/repo with spaces/App.Data",
            mcp_url: "http://127.0.0.1:12345/mcp",
            port_file_path: "/tmp/ezmigrations/mcp-port.json",
        }
    }

    #[test]
    fn shell_bootstrap_uses_real_shell_parameter_expansion() {
        let script = shell_bootstrap(&ctx());
        assert!(script.contains(r#"exec "${SHELL:-/bin/bash}" -l"#));
        assert!(!script.contains("${{SHELL"));
    }

    #[test]
    fn shell_bootstrap_quotes_paths_with_spaces() {
        let script = shell_bootstrap(&ctx());
        assert!(script.contains("cd '/tmp/repo with spaces' || exit 1"));
        assert!(
            script.contains("export EZMIGRATIONS_PROJECT_PATH='/tmp/repo with spaces/App.Data'")
        );
    }

    #[test]
    fn shell_bootstrap_validates_before_replacing_existing_claude_entry() {
        let script = shell_bootstrap(&ctx());
        let first_add = script
            .find(r#"claude mcp add --scope local --transport http ezmigrations "$EZMIGRATIONS_MCP_URL""#)
            .expect("direct add should be attempted first");
        let first_remove = script
            .find("claude mcp remove --scope local ezmigrations >/dev/null 2>&1")
            .expect("existing entry may be removed after validation");
        assert!(first_add < first_remove);
        assert!(script.contains(r#"check_name="ezmigrations-check-$$""#));
    }
}
