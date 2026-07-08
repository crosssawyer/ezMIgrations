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
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::state::AppState;

/// Discovery document written to [`port_file_path`] so external MCP clients
/// (typically AI agents) can find the loopback port without a config dance.
#[derive(Serialize, Deserialize)]
struct PortFile {
    port: u16,
    pid: u32,
    started_at_unix_ms: u64,
    transport: String,
    url: String,
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

/// Read the PID from an existing port file, if one exists and is parseable.
///
/// Returns `None` for "no file", "unreadable", or malformed JSON. The caller
/// can use a fresh `None` from a missing file the same way as a
/// stale-and-unrecoverable file: take the slot.
pub fn read_port_file_pid() -> Option<u32> {
    let body = fs::read_to_string(port_file_path()).ok()?;
    serde_json::from_str::<PortFile>(&body).ok().map(|p| p.pid)
}

/// Write the discovery JSON. The timestamp is a raw unix-ms value so we don't
/// have to pull in `chrono` just for an ISO-8601 string.
fn write_port_file(port: u16) -> std::io::Result<()> {
    let path = port_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let doc = PortFile {
        port,
        pid: std::process::id(),
        started_at_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
        transport: "http".to_string(),
        url: format!("http://127.0.0.1:{port}/mcp"),
    };

    let body = serde_json::to_string_pretty(&doc)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(&path, body)
}
