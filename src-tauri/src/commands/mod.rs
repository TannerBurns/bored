pub mod agents;
pub mod agent_settings;
#[cfg(test)]
mod agent_settings_tests;
pub mod boards;
pub mod chat;
pub mod dashboard;
mod diff_parser;
pub mod next_steps;
pub mod projects;
pub mod release_notes;
pub mod runs;
pub mod specs;
pub mod tasks;
pub mod tickets;
pub mod workers;
pub mod workflow_settings;

pub use agents::{check_agent_available, get_agent_status, AgentStatus};
pub use boards::*;
pub use agent_settings::{AgentSettingsManager, SharedAgentSettings};
pub use projects::*;
pub use runs::{
    backfill_run_costs, cancel_agent_run, get_agent_run, get_agent_runs,
    get_implementation_todos, get_recent_runs_with_context, get_run_events,
    get_ticket_cost, start_agent_run,
};
pub use specs::{
    append_spec_exploration, approve_plan, create_spec, delete_spec, execute_plan,
    get_spec_cost, get_spec_eta, get_spec_tickets,
    get_version_progress, halt_spec_work, pause_spec_work, reset_plan_execution,
    resume_spec_work, set_spec_plan, set_spec_status, start_planner, start_spec_work, update_spec,
};
pub use tasks::{
    add_command_task, create_task, delete_task,
    get_task_counts, get_tasks, reset_task, update_task,
};
pub use tickets::*;
pub use workers::{
    delete_custom_command, get_available_commands, get_commands_path,
    get_worker_queue_status, get_workers, read_command_content, save_custom_command,
    start_worker, stop_all_workers, stop_worker,
};

use tauri::State;

/// API connection state shared across Tauri commands via managed state.
/// Bundled as a struct because Tauri keys managed state by type —
/// two bare `String` values would shadow each other.
#[derive(Debug, Clone)]
pub struct ApiConnState {
    pub url: String,
    pub port: u16,
    pub token: String,
}

/// API configuration returned to the frontend
#[derive(Debug, Clone, serde::Serialize)]
pub struct ApiConfigResponse {
    pub url: String,
    pub port: u16,
    pub token: String,
}

/// A model option for a specific agent, returned to the frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentModelOption {
    pub value: String,
    pub label: String,
}

/// Information about a registered agent, returned to the frontend.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInfo {
    pub id: String,
    pub display_name: String,
    pub is_available: bool,
    pub version: Option<String>,
    pub brand_color: Option<String>,
    pub available_models: Vec<AgentModelOption>,
}

/// Return the list of all registered agents with their availability status.
#[tauri::command]
pub fn get_available_agents(
    registry: tauri::State<'_, std::sync::Arc<crate::agents::registry::AgentRegistry>>,
) -> Vec<AgentInfo> {
    registry
        .providers()
        .iter()
        .map(|p| AgentInfo {
            id: p.id().to_string(),
            display_name: p.display_name().to_string(),
            is_available: p.is_available(),
            version: p.get_version(),
            brand_color: p.brand_color().map(|s| s.to_string()),
            available_models: p
                .available_models()
                .into_iter()
                .map(|(v, l)| AgentModelOption {
                    value: v.to_string(),
                    label: l.to_string(),
                })
                .collect(),
        })
        .collect()
}

#[tauri::command]
pub fn set_notifications_enabled(
    enabled: bool,
    state: tauri::State<'_, crate::tray::NotificationsEnabled>,
) {
    state
        .0
        .store(enabled, std::sync::atomic::Ordering::Relaxed);
    tracing::info!("Notifications {}", if enabled { "enabled" } else { "disabled" });
}

/// Get the current API configuration (port, URL, token)
#[tauri::command]
pub fn get_api_config(api_conn: State<'_, ApiConnState>) -> Result<ApiConfigResponse, String> {
    Ok(ApiConfigResponse {
        url: api_conn.url.clone(),
        port: api_conn.port,
        token: api_conn.token.clone(),
    })
}
