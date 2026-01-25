#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod db;
mod api;
mod agents;
mod logging;


fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path_resolver()
                .app_data_dir()
                .expect("Failed to get app data directory");

            if let Err(e) = logging::init_logging(app_data_dir.clone()) {
                eprintln!("Failed to initialize logging: {}", e);
            }

            tracing::info!("Agent Kanban starting up...");
            tracing::info!("App data directory: {:?}", app_data_dir);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_boards,
            commands::create_board,
            commands::get_tickets,
            commands::create_ticket,
            commands::move_ticket,
            commands::start_agent_run,
            commands::get_agent_runs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
