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

use crate::state::AppState;

/// Bind a loopback HTTP listener on a random port, spawn the rmcp/axum server
/// onto the current tokio runtime, and return the chosen port. The serve
/// future runs detached for the lifetime of the process; we don't expose a
/// shutdown handle because the server lives and dies with the Tauri app.
pub async fn start_mcp_server(
    state: Arc<AppState>,
) -> Result<u16, Box<dyn std::error::Error + Send + Sync>> {
    // Bind first so we know the real port before we advertise it.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();

    let router = server::build_axum_router(state);

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
/// Returns `None` for "no file", "unreadable", or "PID field missing /
/// malformed". The caller can use a fresh `None` from a missing file the same
/// way as a stale-and-unrecoverable file: take the slot.
///
/// We hand-parse rather than depending on `serde_json` for this lookup so
/// other binaries can use it without paying the JSON-deserialisation cost
/// for what is effectively a five-line file under our own control.
pub fn read_port_file_pid() -> Option<u32> {
    let body = fs::read_to_string(port_file_path()).ok()?;
    // Look for `"pid": <digits>` — tolerates whitespace variations and the
    // hand-formatted layout `write_port_file` produces. If the format ever
    // grows nested objects we'll switch to a real parser.
    let key_pos = body.find("\"pid\"")?;
    let after_key = &body[key_pos + "\"pid\"".len()..];
    let colon = after_key.find(':')?;
    let after_colon = after_key[colon + 1..].trim_start();
    let digits: String = after_colon.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// Write the discovery JSON. We hand-roll the timestamp via `SystemTime` so
/// we don't have to pull in `chrono` just for an ISO-8601 string.
fn write_port_file(port: u16) -> std::io::Result<()> {
    let path = port_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let started_at_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    // We intentionally hand-format rather than going through `serde_json` so
    // a future reader can eyeball the file without firing up jq. The values
    // are all primitives we control, so escaping isn't a concern here.
    let body = format!(
        "{{\n  \"port\": {port},\n  \"pid\": {pid},\n  \"started_at_unix_ms\": {started_at_unix_ms},\n  \"transport\": \"http\",\n  \"url\": \"http://127.0.0.1:{port}/mcp\"\n}}\n",
        port = port,
        pid = std::process::id(),
        started_at_unix_ms = started_at_unix_ms,
    );

    fs::write(&path, body)
}
