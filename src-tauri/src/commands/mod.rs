pub mod boards;
pub mod claude;
pub mod cursor;
pub mod projects;
pub mod runs;
pub mod specs;
pub mod tasks;
pub mod tickets;
pub mod workers;

pub use boards::*;
pub use claude::*;
pub use cursor::*;
pub use projects::*;
pub use runs::{
    cancel_agent_run, get_agent_run, get_agent_runs, get_recent_runs, get_run_events,
    start_agent_run,
};
pub use specs::{
    append_spec_exploration, approve_plan, create_spec, delete_spec, execute_plan, get_spec,
    get_spec_eta, get_spec_progress, get_spec_tickets, get_specs, halt_spec_work, pause_spec_work,
    resume_spec_work, set_spec_plan, set_spec_status, start_planner, start_spec_work, update_spec,
};
pub use tasks::{
    add_preset_task, create_task, delete_task, get_next_pending_task, get_preset_types, get_task,
    get_task_counts, get_tasks, has_pending_tasks, reset_task, update_task,
};
pub use tickets::*;
pub use workers::{
    check_commands_installed, check_user_commands_installed, get_available_commands,
    get_commands_path, get_worker_queue_status, get_workers, install_commands_to_project,
    install_commands_to_user, start_worker, stop_all_workers, stop_worker, validate_worker,
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
    let port_str = std::env::var("AGENT_KANBAN_API_PORT").unwrap_or_else(|_| "7432".to_string());
    let port: u16 = port_str.parse().unwrap_or(7432);

    let url = std::env::var("AGENT_KANBAN_API_URL")
        .unwrap_or_else(|_| format!("http://127.0.0.1:{}", port));

    let token = std::env::var("AGENT_KANBAN_API_TOKEN").unwrap_or_default();

    Ok(ApiConfigResponse { url, port, token })
}
