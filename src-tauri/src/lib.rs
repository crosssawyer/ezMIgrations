mod commands;
mod dotnet;
mod git;
mod parser;
mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
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
            commands::start_branch_watcher,
            commands::get_saved_projects,
            commands::save_project,
            commands::update_saved_project,
            commands::delete_saved_project,
            commands::switch_project,
            commands::set_stable_migration,
            commands::start_migration_watcher,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
