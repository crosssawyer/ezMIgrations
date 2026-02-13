mod commands;
mod dotnet;
mod git;
mod parser;
mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::set_project,
            commands::get_project,
            commands::list_migrations,
            commands::add_migration,
            commands::remove_migration,
            commands::update_database,
            commands::get_migration_sql,
            commands::squash_migrations,
            commands::generate_script,
            commands::get_current_branch,
            commands::start_branch_watcher,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
