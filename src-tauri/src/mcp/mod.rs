//! In-process MCP (Model Context Protocol) server.
//!
//! At app start we bind a loopback HTTP listener on an ephemeral port, hand
//! it to an rmcp-backed axum router (built by `server::build_axum_router`),
//! and drop a port file into the user's app-data directory so external MCP
//! clients (typically AI agents) can discover us. The router shares the same
//! `Arc<AppState>` as the Tauri command handlers, so MCP tool calls hit the
//! exact same code paths as the GUI.

mod instructions;
mod resources;
mod server;

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::state::{AppState, McpServerRuntime};

/// Discovery document written to [`port_file_path`] so external MCP clients
/// (typically AI agents) can find the loopback port without a config dance.
#[derive(Clone, Serialize, Deserialize)]
struct PortFile {
    port: u16,
    pid: u32,
    started_at_unix_ms: u64,
    transport: String,
    url: String,
}

#[derive(Clone, Serialize)]
pub struct McpServerStatus {
    pub running: bool,
    pub port: Option<u16>,
    pub pid: u32,
    pub started_at_unix_ms: Option<u64>,
    pub transport: String,
    pub url: Option<String>,
    pub port_file_path: String,
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn status_stopped() -> McpServerStatus {
    McpServerStatus {
        running: false,
        port: None,
        pid: std::process::id(),
        started_at_unix_ms: None,
        transport: "http".to_string(),
        url: None,
        port_file_path: port_file_path().to_string_lossy().to_string(),
    }
}

fn status_from_runtime(runtime: &McpServerRuntime) -> McpServerStatus {
    McpServerStatus {
        running: true,
        port: Some(runtime.port),
        pid: std::process::id(),
        started_at_unix_ms: Some(runtime.started_at_unix_ms),
        transport: "http".to_string(),
        url: Some(runtime.url.clone()),
        port_file_path: port_file_path().to_string_lossy().to_string(),
    }
}

fn status_from_parts(port: u16, started_at_unix_ms: u64) -> McpServerStatus {
    McpServerStatus {
        running: true,
        port: Some(port),
        pid: std::process::id(),
        started_at_unix_ms: Some(started_at_unix_ms),
        transport: "http".to_string(),
        url: Some(format!("http://127.0.0.1:{port}/mcp")),
        port_file_path: port_file_path().to_string_lossy().to_string(),
    }
}

pub fn managed_mcp_status(state: &AppState) -> McpServerStatus {
    state
        .mcp_server
        .lock()
        .unwrap()
        .as_ref()
        .map(status_from_runtime)
        .unwrap_or_else(status_stopped)
}

pub async fn start_managed_mcp_server(
    state: Arc<AppState>,
    store: Arc<dyn crate::ops::ConfigStore>,
) -> Result<McpServerStatus, Box<dyn std::error::Error + Send + Sync>> {
    let _guard = state.mcp_lifecycle.lock().await;

    if let Some(runtime) = state.mcp_server.lock().unwrap().as_ref() {
        return Ok(status_from_runtime(runtime));
    }

    if let Some(owner) = live_external_port_owner() {
        return Err(format!(
            "ezMigrations MCP is already running in another process (PID {}, {}). Stop it before starting the GUI-hosted server.",
            owner.pid, owner.url
        )
        .into());
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let url = format!("http://127.0.0.1:{port}/mcp");
    let started_at_unix_ms = now_unix_ms();
    let generation = state.mcp_generation.fetch_add(1, Ordering::SeqCst) + 1;
    let router = server::build_axum_router(state.clone(), store);
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    if let Err(e) = write_port_file_with_started_at(port, started_at_unix_ms) {
        eprintln!("MCP: failed to write port file: {}", e);
    }

    {
        let mut runtime = state.mcp_server.lock().unwrap();
        *runtime = Some(McpServerRuntime {
            generation,
            port,
            url,
            started_at_unix_ms,
            shutdown: Some(shutdown_tx),
        });
    }

    let clear_state = state.clone();
    tokio::spawn(async move {
        let result = axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await;
        if let Err(e) = result {
            eprintln!("MCP server exited with error: {}", e);
        }
        clear_managed_if_generation(&clear_state, generation);
    });

    Ok(status_from_parts(port, started_at_unix_ms))
}

pub async fn stop_managed_mcp_server(state: Arc<AppState>) -> McpServerStatus {
    let _guard = state.mcp_lifecycle.lock().await;
    let runtime = state.mcp_server.lock().unwrap().take();

    if let Some(mut runtime) = runtime {
        if let Some(shutdown) = runtime.shutdown.take() {
            let _ = shutdown.send(());
        }
        let _ = remove_port_file_for_runtime(&runtime);
    }

    status_stopped()
}

pub fn cleanup_managed_mcp_server(state: &AppState) {
    let runtime = state.mcp_server.lock().unwrap().take();
    if let Some(mut runtime) = runtime {
        if let Some(shutdown) = runtime.shutdown.take() {
            let _ = shutdown.send(());
        }
        let _ = remove_port_file_for_runtime(&runtime);
    }
}

fn clear_managed_if_generation(state: &AppState, generation: u64) {
    let runtime = {
        let mut guard = state.mcp_server.lock().unwrap();
        if guard
            .as_ref()
            .map(|runtime| runtime.generation == generation)
            .unwrap_or(false)
        {
            guard.take()
        } else {
            None
        }
    };

    if let Some(runtime) = runtime {
        let _ = remove_port_file_for_runtime(&runtime);
    }
}

/// Bind a loopback HTTP listener on a random port, spawn the rmcp/axum server
/// onto the current tokio runtime, and return the chosen port. The serve
/// future runs detached for the lifetime of the process; we don't expose a
/// shutdown handle because the server lives and dies with the Tauri app.
pub async fn start_mcp_server(
    state: Arc<AppState>,
    store: Arc<dyn crate::ops::ConfigStore>,
) -> Result<u16, Box<dyn std::error::Error + Send + Sync>> {
    // Bind first so we know the real port before we advertise it.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();

    let router = server::build_axum_router(state, store);

    // Advertise the port BEFORE handing the listener to axum so an agent that
    // polls the port file the instant we log "listening" can connect on the
    // first try. A write failure is non-fatal: agents can still find us via
    // logs or, in the future, a Tauri status command.
    if let Err(e) = write_port_file(port) {
        eprintln!("MCP: failed to write port file: {}", e);
    }

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, router).await {
            eprintln!("MCP server exited with error: {}", e);
        }
    });

    Ok(port)
}

/// Resolve the platform-specific path for the port advertisement file.
///
/// * Windows: `%APPDATA%\ezmigrations\mcp-port.json`
/// * macOS:   `~/Library/Application Support/ezmigrations/mcp-port.json`
/// * Linux:   `$XDG_DATA_HOME/ezmigrations/mcp-port.json` (or `~/.local/share/...`)
///
/// Falls back to the OS temp dir if `dirs::data_dir()` returns `None` (rare:
/// CI sandboxes, broken `$HOME`). Agents that share the temp dir with us
/// will still find the file.
///
/// `pub` so the headless `ezmigrations-mcp` binary (which Cargo compiles as
/// a separate crate that depends on this library) can read the same file
/// the GUI writes. The function returns nothing sensitive — just a path —
/// so widening visibility is cheap.
pub fn port_file_path() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(std::env::temp_dir);
    base.join("ezmigrations").join("mcp-port.json")
}

fn read_port_file() -> Option<PortFile> {
    let body = fs::read_to_string(port_file_path()).ok()?;
    serde_json::from_str::<PortFile>(&body).ok()
}

fn live_external_port_owner() -> Option<PortFile> {
    let owner = read_port_file()?;
    if owner.pid == std::process::id() {
        return None;
    }
    if pid_alive(owner.pid) {
        Some(owner)
    } else {
        let _ = remove_port_file();
        None
    }
}

/// Read the PID from an existing port file, if one exists and is parseable.
///
/// Returns `None` for "no file", "unreadable", or malformed JSON. The caller
/// can use a fresh `None` from a missing file the same way as a
/// stale-and-unrecoverable file: take the slot.
pub fn read_port_file_pid() -> Option<u32> {
    read_port_file().map(|p| p.pid)
}

/// Write the discovery JSON. The timestamp is a raw unix-ms value so we don't
/// have to pull in `chrono` just for an ISO-8601 string.
fn write_port_file(port: u16) -> std::io::Result<()> {
    write_port_file_with_started_at(port, now_unix_ms())
}

fn write_port_file_with_started_at(port: u16, started_at_unix_ms: u64) -> std::io::Result<()> {
    let path = port_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let doc = PortFile {
        port,
        pid: std::process::id(),
        started_at_unix_ms,
        transport: "http".to_string(),
        url: format!("http://127.0.0.1:{port}/mcp"),
    };

    let body = serde_json::to_string_pretty(&doc)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(&path, body)
}

fn remove_port_file() -> std::io::Result<()> {
    match fs::remove_file(port_file_path()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

fn remove_port_file_for_runtime(runtime: &McpServerRuntime) -> std::io::Result<()> {
    let owned = read_port_file()
        .map(|file| {
            file.pid == std::process::id()
                && file.port == runtime.port
                && file.started_at_unix_ms == runtime.started_at_unix_ms
        })
        .unwrap_or(false);

    if owned {
        remove_port_file()
    } else {
        Ok(())
    }
}

/// Best-effort "is this PID still running" check using only process tools that
/// are present on the target OS. Used before taking ownership of mcp-port.json.
pub fn pid_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        let out = Command::new("tasklist")
            .args(["/FO", "CSV", "/NH", "/FI", &format!("PID eq {}", pid)])
            .output();
        match out {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                let pid_str = pid.to_string();
                stdout.lines().any(|line| {
                    line.split(',')
                        .nth(1)
                        .map(|field| field.trim().trim_matches('"') == pid_str)
                        .unwrap_or(false)
                })
            }
            Err(_) => false,
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pid_alive_reports_current_process() {
        assert!(pid_alive(std::process::id()));
    }
}
