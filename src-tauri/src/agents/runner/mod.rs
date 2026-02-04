//! Shared agent runner logic for both direct runs and worker-initiated runs.

mod config;
mod events;
mod workflow;

pub use config::{create_cancel_handles, CancelHandlesMap, RunnerConfig, RunnerResult};
pub use events::{AgentCompleteEvent, AgentErrorEvent, AgentLogEvent};

use crate::db::RunStatus;

/// Execute an agent run with the given configuration.
pub async fn execute_agent_run(config: RunnerConfig) -> Result<RunnerResult, String> {
    let start_time = std::time::Instant::now();

    tracing::info!(
        "execute_agent_run: ticket={}, run_id={}, workflow_type={:?}, agent={:?}",
        config.ticket.id,
        config.run_id,
        config.ticket.workflow_type,
        config.agent_kind
    );

    if let Some(ref hook_path) = config.hook_script_path {
        if let Err(e) = workflow::update_project_hooks_for_run(
            &config.repo_path,
            hook_path,
            &config.api_url,
            &config.api_token,
            &config.run_id,
            config.agent_kind,
        ) {
            tracing::warn!("Failed to update project hooks: {}", e);
        }
    }

    config
        .db
        .update_run_status(&config.run_id, RunStatus::Running, None, None)
        .map_err(|e| format!("Failed to update run status: {}", e))?;

    let result = workflow::execute_multi_stage_workflow(&config).await;
    let duration_secs = start_time.elapsed().as_secs_f64();

    match result {
        Ok(()) => workflow::handle_workflow_success(&config, duration_secs),
        Err(e) => workflow::handle_workflow_error(&config, e, duration_secs),
    }
}
