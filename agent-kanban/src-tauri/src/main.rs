#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use tauri::Manager;

use agent_kanban::{api, commands, db, logging};
use agent_kanban::commands::runs::RunningAgents;

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

            let db_path = app_data_dir.join("agent-kanban.db");
            let database = Arc::new(db::Database::open(db_path).expect("Failed to open database"));

            app.manage(database.clone());

            // Manage running agents state
            app.manage(RunningAgents::new());

            // Configure API server
            let api_config = api::ApiConfig::default();
            
            // Write token and port for hook scripts to read
            let token_path = app_data_dir.join("api_token");
            std::fs::write(&token_path, &api_config.token)
                .expect("Failed to write API token");
            
            let port_path = app_data_dir.join("api_port");
            std::fs::write(&port_path, api_config.port.to_string())
                .expect("Failed to write API port");

            // Make config available via environment for child processes
            std::env::set_var("AGENT_KANBAN_API_TOKEN", &api_config.token);
            std::env::set_var("AGENT_KANBAN_API_PORT", api_config.port.to_string());
            std::env::set_var("AGENT_KANBAN_API_URL", format!("http://127.0.0.1:{}", api_config.port));

            // Start API server
            let db_for_api = database.clone();
            tauri::async_runtime::spawn(async move {
                match api::start_server(db_for_api, api_config).await {
                    Ok(handle) => {
                        tracing::info!("API server started at {}", handle.addr);
                        // Keep handle alive - server runs until app exits
                        std::mem::forget(handle);
                    }
                    Err(e) => {
                        tracing::error!("Failed to start API server: {}", e);
                    }
                }
            });

            tracing::info!("Agent Kanban initialized successfully");

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_boards,
            commands::create_board,
            commands::get_tickets,
            commands::create_ticket,
            commands::move_ticket,
            commands::runs::start_agent_run,
            commands::runs::get_agent_runs,
            commands::runs::get_agent_run,
            commands::runs::cancel_agent_run,
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
