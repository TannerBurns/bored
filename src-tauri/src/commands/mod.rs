pub mod boards;
pub mod claude;
pub mod cursor;
pub mod projects;
pub mod runs;
pub mod tasks;
pub mod tickets;
pub mod workers;

pub use boards::*;
pub use claude::*;
pub use cursor::*;
pub use projects::*;
pub use runs::{start_agent_run, get_agent_runs, get_recent_runs, get_agent_run, get_run_events, cancel_agent_run};
pub use tasks::{
    get_tasks, get_task, create_task, add_preset_task, delete_task,
    get_next_pending_task, has_pending_tasks, get_task_counts, update_task, get_preset_types,
};
pub use tickets::*;
pub use workers::{
    start_worker, stop_worker, stop_all_workers, get_workers, get_worker_queue_status,
    validate_worker, get_commands_path, get_available_commands, install_commands_to_project,
    install_commands_to_user, check_commands_installed, check_user_commands_installed,
};

/// API configuration returned to the frontend
#[derive(Debug, Clone, serde::Serialize)]
pub struct ApiConfigResponse {
    pub url: String,
    pub port: u16,
    pub token: String,
}

/// Get the current API configuration (port, URL, token)
#[tauri::command]
pub fn get_api_config() -> Result<ApiConfigResponse, String> {
    let port_str = std::env::var("AGENT_KANBAN_API_PORT")
        .unwrap_or_else(|_| "7432".to_string());
    let port: u16 = port_str.parse().unwrap_or(7432);
    
    let url = std::env::var("AGENT_KANBAN_API_URL")
        .unwrap_or_else(|_| format!("http://127.0.0.1:{}", port));
    
    let token = std::env::var("AGENT_KANBAN_API_TOKEN")
        .unwrap_or_default();
    
    Ok(ApiConfigResponse { url, port, token })
}
