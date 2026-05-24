//! Headless ezMigrations MCP server.
//!
//! Starts the in-process MCP server without launching the Tauri GUI. Agents
//! (or scripts) can invoke this binary directly to drive ezMigrations on
//! machines where the GUI isn't installed or isn't running. The MCP surface
//! is identical to what the desktop app exposes — same 23 tools, same 8
//! resources, same `op_mutex` serialisation — because the bin reuses
//! `ez_migrations_lib::mcp::start_mcp_server`.
//!
//! Refuses to start if another ezMigrations process (GUI or headless) is
//! already serving MCP on this machine, so we never write a port file the
//! other server is currently advertising.

use std::process;
use std::sync::Arc;

use ez_migrations_lib::mcp;
use ez_migrations_lib::state::AppState;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("ezmigrations-mcp: fatal: {e}");
        process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Collision check: if the port file already exists and points at a live
    // process, refuse to start. A second server on the same machine would
    // race the first on the port file and confuse agents that read it
    // mid-write. A stale file (process died without cleanup) is fine — we
    // log and let `start_mcp_server` overwrite it.
    if let Some(pid) = mcp::read_port_file_pid() {
        if pid_alive(pid) && pid != std::process::id() {
            let path = mcp::port_file_path();
            return Err(format!(
                "ezMigrations is already running (PID {pid}, port file {path}). \
                 Stop it before starting the headless server.",
                pid = pid,
                path = path.display(),
            )
            .into());
        } else {
            eprintln!(
                "ezmigrations-mcp: stale port file at {}, taking over",
                mcp::port_file_path().display()
            );
        }
    }

    // `AppState::default()` is fully Tauri-independent — every field is a
    // `Mutex<Default>` / `Arc<AsyncMutex<...>>` — so we can build it without
    // an `AppHandle`. The trade-off: config that the GUI persists via
    // `AppHandle::path()` isn't loaded here; the running session starts with
    // an empty project list and `set_project` / `save_project` calls live in
    // memory only. That's the same persistence story documented in the
    // server `instructions` debrief.
    let state = Arc::new(AppState::default());

    let port = mcp::start_mcp_server(state).await?;

    eprintln!(
        "ezmigrations-mcp listening on http://127.0.0.1:{}/mcp (headless)",
        port
    );

    // Block until ctrl-c or the parent process tears us down. We don't bother
    // unlinking the port file on shutdown: the next launch detects a stale
    // file and overwrites it. Simpler than a panic-safe cleanup path.
    tokio::signal::ctrl_c().await?;
    eprintln!("ezmigrations-mcp: received ctrl-c, shutting down");
    Ok(())
}

/// Best-effort "is this PID still running" check using only what's in the
/// standard library — no `sysinfo`, `nix`, `libc`, or `windows` crate. On the
/// hot path this runs exactly once per startup, so spawning a child process
/// for the probe is fine.
fn pid_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // `kill -0 <pid>` is the POSIX idiom: signal 0 doesn't deliver
        // anything, it just exercises permission/existence checks. Exit
        // status 0 means the process exists; non-zero means it doesn't (or
        // we lack permission, which on a single-user box is functionally
        // the same as "treat the slot as taken").
        std::process::Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        // `tasklist /FI "PID eq <pid>"` prints "INFO: No tasks are running
        // which match the specified criteria." when nothing matches. We grep
        // the output rather than parse it because the table format isn't
        // stable across Windows locales — the "No tasks" sentinel is.
        let out = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid)])
            .output();
        match out {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                !stdout.contains("No tasks")
            }
            Err(_) => false,
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        // Unknown target: assume alive so we err on the side of refusing to
        // start. The user can delete the port file manually if they're sure.
        let _ = pid;
        true
    }
}
