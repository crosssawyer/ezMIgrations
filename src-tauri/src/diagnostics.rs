use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager};

const LOG_FILE: &str = "diagnostics.log";
const MAX_LOG_SIZE: u64 = 1_048_576; // 1 MB — trim before this
const TRIM_TARGET: usize = 524_288;  // 512 KB — keep tail when trimming

fn write_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub fn log_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join(LOG_FILE))
}

pub struct ListMigrationsEntry<'a> {
    pub project_path: &'a str,
    pub db_context: &'a str,
    pub startup_project: &'a str,
    pub command_display: &'a str,
    pub duration_ms: u128,
    pub exit_success: bool,
    pub parsed_count: usize,
    pub migration_files_found: usize,
    pub stdout: &'a str,
    pub stderr: &'a str,
}

pub fn log_list_migrations(app: &AppHandle, entry: &ListMigrationsEntry) {
    let path = match log_path(app) {
        Some(p) => p,
        None => return,
    };
    let formatted = format_list_migrations_entry(entry);
    let _ = write_entry(&path, &formatted);
}

fn format_list_migrations_entry(entry: &ListMigrationsEntry) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "=== {} list_migrations ===\n",
        timestamp_utc()
    ));
    out.push_str(&format!("app_version: {}\n", env!("CARGO_PKG_VERSION")));
    out.push_str(&format!(
        "os: {} ({})\n",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    out.push_str(&format!("project_path: {}\n", entry.project_path));
    out.push_str(&format!("db_context: {}\n", display_or_blank(entry.db_context)));
    out.push_str(&format!(
        "startup_project: {}\n",
        display_or_blank(entry.startup_project)
    ));
    out.push_str(&format!("command: {}\n", entry.command_display));
    out.push_str(&format!("duration_ms: {}\n", entry.duration_ms));
    out.push_str(&format!("exit_success: {}\n", entry.exit_success));
    out.push_str(&format!("parsed_count: {}\n", entry.parsed_count));
    out.push_str(&format!(
        "migration_files_found: {}\n",
        entry.migration_files_found
    ));
    out.push_str("--- stdout ---\n");
    out.push_str(entry.stdout.trim_end());
    out.push_str("\n--- stderr ---\n");
    out.push_str(entry.stderr.trim_end());
    out.push_str("\n\n");
    out
}

fn display_or_blank(s: &str) -> &str {
    if s.is_empty() { "(unset)" } else { s }
}

fn write_entry(path: &Path, body: &str) -> std::io::Result<()> {
    let _guard = write_lock().lock().unwrap_or_else(|e| e.into_inner());

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Trim if oversized
    if let Ok(meta) = fs::metadata(path) {
        if meta.len() > MAX_LOG_SIZE {
            let _ = trim_log(path);
        }
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(body.as_bytes())?;
    Ok(())
}

fn trim_log(path: &Path) -> std::io::Result<()> {
    let contents = fs::read(path)?;
    let start = contents.len().saturating_sub(TRIM_TARGET);
    // Align to next newline so the first entry isn't half-cut.
    let aligned = match contents[start..].iter().position(|&b| b == b'\n') {
        Some(idx) => start + idx + 1,
        None => start,
    };
    let header = b"... (older entries trimmed) ...\n\n";
    let mut new_contents = Vec::with_capacity(header.len() + contents.len() - aligned);
    new_contents.extend_from_slice(header);
    new_contents.extend_from_slice(&contents[aligned..]);
    fs::write(path, new_contents)?;
    Ok(())
}

pub fn read_tail(app: &AppHandle, max_bytes: usize) -> String {
    let path = match log_path(app) {
        Some(p) => p,
        None => return String::new(),
    };
    let bytes = match fs::read(&path) {
        Ok(b) => b,
        Err(_) => return String::new(),
    };
    if bytes.len() <= max_bytes {
        return String::from_utf8_lossy(&bytes).to_string();
    }
    let start = bytes.len() - max_bytes;
    let aligned = match bytes[start..].iter().position(|&b| b == b'\n') {
        Some(idx) => start + idx + 1,
        None => start,
    };
    let mut s = String::from("... (older entries trimmed) ...\n");
    s.push_str(&String::from_utf8_lossy(&bytes[aligned..]));
    s
}

pub fn clear(app: &AppHandle) -> Result<(), String> {
    let path = log_path(app).ok_or("Could not resolve diagnostics path")?;
    if path.exists() {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn reveal(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let path_str = path.to_string_lossy().to_string();
        // explorer.exe returns exit code 1 even on success; spawn-and-forget.
        crate::process::command("explorer")
            .arg(format!("/select,{}", path_str))
            .spawn()
            .map_err(|e| format!("Failed to launch explorer: {}", e))?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        let path_str = path.to_string_lossy().to_string();
        crate::process::command("open")
            .args(["-R", &path_str])
            .spawn()
            .map_err(|e| format!("Failed to launch open: {}", e))?;
        Ok(())
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // xdg-open doesn't support "reveal", so open the parent directory.
        let parent = path.parent().unwrap_or(path);
        crate::process::command("xdg-open")
            .arg(parent)
            .spawn()
            .map_err(|e| format!("Failed to launch xdg-open: {}", e))?;
        Ok(())
    }
}

// ─── Timestamp helpers ──────────────────────────────────────────────

fn timestamp_utc() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_unix_secs_utc(secs)
}

fn format_unix_secs_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let tod = secs % 86_400;
    let hour = tod / 3600;
    let minute = (tod % 3600) / 60;
    let second = tod % 60;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y, m, d, hour, minute, second
    )
}

// Howard Hinnant's "civil_from_days": days since 1970-01-01 → (year, month, day).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_known_epochs() {
        assert_eq!(format_unix_secs_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_unix_secs_utc(1_700_000_000), "2023-11-14T22:13:20Z");
    }
}
