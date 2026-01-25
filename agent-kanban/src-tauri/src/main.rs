#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agents;
mod api;
mod commands;
mod db;
mod logging;

use std::sync::Arc;
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app
                .path_resolver()
                .app_data_dir()
                .expect("Failed to get app data directory");

            if let Err(e) = logging::init_logging(app_data_dir.clone()) {
                eprintln!("Failed to initialize logging: {}", e);
            }

            tracing::info!("Agent Kanban starting up...");
            tracing::info!("App data directory: {:?}", app_data_dir);

            // Initialize database
            let db_path = app_data_dir.join("agent-kanban.db");
            let database = db::Database::open(db_path).expect("Failed to open database");

            // Make database available to commands
            app.manage(Arc::new(database));

            tracing::info!("Agent Kanban initialized successfully");

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Board commands
            commands::get_boards,
            commands::create_board,
            // Ticket commands
            commands::get_tickets,
            commands::create_ticket,
            commands::move_ticket,
            // Agent run commands
            commands::start_agent_run,
            commands::get_agent_runs,
            // Project commands
            commands::get_projects,
            commands::get_project,
            commands::create_project,
            commands::update_project,
            commands::delete_project,
            commands::set_board_project,
            commands::set_ticket_project,
            commands::check_ticket_readiness,
            commands::update_project_hooks,
            commands::browse_for_directory,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
