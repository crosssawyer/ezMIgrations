use serde::{Deserialize, Serialize};
use std::time::Duration;
use tiberius::{Client, Config};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

const KEYRING_SERVICE: &str = "ez-migrations";
const QUERY_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbHistoryRow {
    pub migration_id: String,
    pub product_version: String,
}

// ─── Keyring storage ────────────────────────────────────────────────

fn keyring_entry(project_id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, project_id)
        .map_err(|e| format!("Failed to access OS keyring: {}", e))
}

pub fn store_connection_string(project_id: &str, conn: &str) -> Result<(), String> {
    let entry = keyring_entry(project_id)?;
    entry
        .set_password(conn)
        .map_err(|e| format!("Failed to store connection string in keyring: {}", e))
}

pub fn load_connection_string(project_id: &str) -> Result<Option<String>, String> {
    let entry = keyring_entry(project_id)?;
    match entry.get_password() {
        Ok(s) => Ok(Some(s)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("Failed to read connection string from keyring: {}", e)),
    }
}

pub fn clear_connection_string(project_id: &str) -> Result<(), String> {
    let entry = keyring_entry(project_id)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("Failed to clear keyring entry: {}", e)),
    }
}

// ─── SQL Server access ──────────────────────────────────────────────

async fn connect(conn_string: &str) -> Result<Client<Compat<TcpStream>>, String> {
    let config = Config::from_ado_string(conn_string)
        .map_err(|e| format!("Invalid SQL Server connection string: {}", e))?;

    let tcp = TcpStream::connect(config.get_addr())
        .await
        .map_err(|e| format!("Could not reach SQL Server at {}: {}", config.get_addr(), e))?;
    tcp.set_nodelay(true)
        .map_err(|e| format!("set_nodelay failed: {}", e))?;

    Client::connect(config, tcp.compat_write())
        .await
        .map_err(|e| format!("SQL Server handshake failed: {}", e))
}

async fn test_connection(conn_string: &str) -> Result<(), String> {
    let mut client = connect(conn_string).await?;
    client
        .simple_query("SELECT 1")
        .await
        .map_err(|e| format!("Probe query failed: {}", e))?;
    Ok(())
}

/// Returns rows from `__EFMigrationsHistory`. If the table doesn't exist (e.g.
/// the DB was created but no migration has ever run), returns an empty Vec —
/// that's an expected state, not an error.
async fn fetch_history(conn_string: &str) -> Result<Vec<DbHistoryRow>, String> {
    let mut client = connect(conn_string).await?;

    // The NVARCHAR widths in the empty-result branch match EF Core's canonical
    // __EFMigrationsHistory schema (MigrationId NVARCHAR(150), ProductVersion
    // NVARCHAR(32)). Picking the widths explicitly keeps column metadata stable
    // across both branches of the IF.
    let stream = client
        .simple_query(
            "IF OBJECT_ID('dbo.__EFMigrationsHistory', 'U') IS NULL \
                 SELECT TOP 0 CAST(NULL AS NVARCHAR(150)) AS MigrationId, \
                              CAST(NULL AS NVARCHAR(32))  AS ProductVersion \
             ELSE \
                 SELECT MigrationId, ProductVersion \
                 FROM dbo.__EFMigrationsHistory \
                 ORDER BY MigrationId",
        )
        .await
        .map_err(|e| format!("Query failed: {}", e))?;

    let rows = stream
        .into_first_result()
        .await
        .map_err(|e| format!("Reading rows failed: {}", e))?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let migration_id: &str = row.get(0).ok_or(
            "__EFMigrationsHistory.MigrationId returned NULL or non-string — \
             unexpected schema for this table",
        )?;
        let product_version: &str = row.get(1).ok_or(
            "__EFMigrationsHistory.ProductVersion returned NULL or non-string — \
             unexpected schema for this table",
        )?;
        out.push(DbHistoryRow {
            migration_id: migration_id.to_string(),
            product_version: product_version.to_string(),
        });
    }
    Ok(out)
}

// ─── Tauri commands ─────────────────────────────────────────────────
//
// These let users verify against `__EFMigrationsHistory` directly, independent
// of `dotnet ef migrations list`. The connection string is stored in the OS
// keyring (not the JSON config) so passwords never hit disk in plaintext.
//
// `project_id` is taken explicitly — the project being edited in settings is
// not always the active project, so we don't infer it from AppState here.

#[tauri::command]
pub async fn set_db_connection(
    project_id: String,
    connection_string: String,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || store_connection_string(&project_id, &connection_string))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn clear_db_connection(project_id: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || clear_connection_string(&project_id))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn has_db_connection(project_id: String) -> Result<bool, String> {
    let result = tokio::task::spawn_blocking(move || load_connection_string(&project_id))
        .await
        .map_err(|e| e.to_string())??;
    Ok(result.is_some())
}

/// Probe a connection string without saving it. Frontend uses this for the
/// "Test connection" button in project settings.
#[tauri::command]
pub async fn test_db_connection(connection_string: String) -> Result<(), String> {
    tokio::time::timeout(QUERY_TIMEOUT, test_connection(&connection_string))
        .await
        .map_err(|_| format!("Connection attempt timed out after {}s", QUERY_TIMEOUT.as_secs()))?
}

#[tauri::command]
pub async fn fetch_db_history(project_id: String) -> Result<Vec<DbHistoryRow>, String> {
    let conn = tokio::task::spawn_blocking(move || load_connection_string(&project_id))
        .await
        .map_err(|e| e.to_string())??
        .ok_or_else(|| "No connection string configured for this project".to_string())?;

    tokio::time::timeout(QUERY_TIMEOUT, fetch_history(&conn))
        .await
        .map_err(|_| format!("Query timed out after {}s", QUERY_TIMEOUT.as_secs()))?
}
