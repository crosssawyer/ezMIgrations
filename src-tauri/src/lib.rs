mod commands;
mod dotnet;
mod git;
// `mcp`, `ops`, and `state` are `pub` so the `ezmigrations-mcp` binary in
// `src/bin/` (which sees the crate as an external dependency named
// `ez_migrations_lib`) can reach `start_mcp_server`, the headless
// `ops::FileConfigStore`, and `AppState`. The remaining modules stay private —
// only the Tauri command surface needs them.
pub mod mcp;
pub mod ops;
mod parser;
mod process;
pub mod state;
mod terminal;

use std::sync::Arc;

use commands::TauriConfigStore;
use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // AppState is wrapped in `Arc` so the MCP server and Tauri commands can
    // share one logical instance. Commands resolve it as
    // `State<'_, Arc<AppState>>`; field access still works through Arc's
    // `Deref` impl.
    let state = Arc::new(AppState::default());
    let mcp_state = state.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                let state = window.state::<Arc<AppState>>();
                mcp::cleanup_managed_mcp_server(state.inner().as_ref());
            }
        })
        .setup(move |app| {
            // The MCP server shares the GUI's persistence: tool calls that
            // mutate the saved-project list write to the same bundle-scoped
            // app-data file the Tauri commands use, so GUI and agent stay in
            // sync. Hence the AppHandle-backed store here rather than a no-op.
            let store: Arc<dyn ops::ConfigStore> =
                Arc::new(TauriConfigStore::new(app.handle().clone()));

            // Spawn the MCP server on Tauri's async runtime so it doesn't
            // block startup. Bind failures are logged but non-fatal to the
            // GUI — the user can still drive the app manually.
            tauri::async_runtime::spawn(async move {
                match mcp::start_managed_mcp_server(mcp_state, store).await {
                    Ok(status) => {
                        if let Some(url) = status.url {
                            eprintln!("MCP server listening on {}", url);
                        } else {
                            eprintln!("MCP server status is stopped after startup");
                        }
                    }
                    Err(e) => {
                        eprintln!("MCP server failed to start: {}", e);
                    }
                }
            });
            Ok(())
        })
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::set_project,
            commands::get_project,
            commands::list_migrations,
            commands::add_migration,
            commands::remove_migration,
            commands::update_database,
            commands::cancel_running_operation,
            commands::get_migration_sql,
            commands::squash_migrations,
            commands::generate_script,
            commands::get_current_branch,
            commands::list_git_branches,
            commands::fetch_remote,
            commands::switch_branch_with_migrations,
            commands::start_branch_watcher,
            commands::get_saved_projects,
            commands::save_project,
            commands::update_saved_project,
            commands::delete_saved_project,
            commands::switch_project,
            commands::set_stable_migration,
            commands::start_migration_watcher,
            commands::get_preferences,
            commands::set_preferences,
            commands::get_mcp_status,
            commands::start_mcp_server,
            commands::stop_mcp_server,
            commands::open_mcp_terminal,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
