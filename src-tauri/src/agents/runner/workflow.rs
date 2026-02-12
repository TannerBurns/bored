//! Workflow execution logic for agent runs.

use tauri::{Emitter, Window};

use super::config::RunnerConfig;
use super::events::{AgentCompleteEvent, AgentErrorEvent};
use crate::agents::claude as claude_hooks;
use crate::agents::cursor as cursor_hooks;
use crate::agents::orchestrator::{OrchestratorConfig, WorkflowOrchestrator};
use crate::agents::AgentKind;
use crate::db::{Database, RunStatus, Ticket};

pub(super) fn update_project_hooks_for_run(
    repo_path: &std::path::Path,
    hook_script_path: &str,
    api_url: &str,
    api_token: &str,
    run_id: &str,
    agent_kind: AgentKind,
) -> Result<(), String> {
    tracing::debug!(
        "Updating project hooks: run_id={}, api_url={}, token_prefix={}...",
        run_id,
        api_url,
        &api_token.chars().take(8).collect::<String>()
    );

    match agent_kind {
        AgentKind::Cursor => cursor_hooks::install_hooks_with_run_id(
            repo_path,
            hook_script_path,
            Some(api_url),
            Some(api_token),
            Some(run_id),
        )
        .map_err(|e| format!("Failed to update Cursor hooks.json: {}", e)),
        AgentKind::Claude => claude_hooks::install_local_hooks_with_run_id(
            repo_path,
            hook_script_path,
            Some(api_url),
            Some(api_token),
            Some(run_id),
        )
        .map_err(|e| format!("Failed to update Claude settings.local.json: {}", e)),
    }
}

pub(super) async fn execute_multi_stage_workflow(config: &RunnerConfig) -> Result<(), String> {
    tracing::info!("Starting multi-stage workflow for run {}", config.run_id);

    let workflow_settings = config
        .workflow_settings
        .clone()
        .unwrap_or_else(|| {
            tracing::warn!("No shared WorkflowSettings on RunnerConfig — using empty defaults");
            std::sync::Arc::new(std::sync::Mutex::new(
                crate::commands::workflow_settings::WorkflowSettings::default(),
            ))
        });

    let orchestrator = WorkflowOrchestrator::new(OrchestratorConfig {
        db: config.db.clone(),
        window: config.window.clone(),
        app_handle: config.app_handle.clone(),
        parent_run_id: config.run_id.clone(),
        ticket: config.ticket.clone(),
        task: config.task.clone(),
        repo_path: config.repo_path.clone(),
        agent_kind: config.agent_kind,
        api_url: config.api_url.clone(),
        api_token: config.api_token.clone(),
        hook_script_path: config.hook_script_path.clone(),
        cancel_handles: config.cancel_handles.clone(),
        worktree_branch: config.worktree_branch.clone(),
        branch_already_created: config.branch_already_created,
        is_temp_branch: config.is_temp_branch,
        claude_api_config: config.claude_api_config.clone(),
        resume_from_stage: config.resume_from_stage.clone(),
        previous_run_id: config.previous_run_id.clone(),
        workflow_settings,
        stage_configs: config.stage_configs.clone(),
        code_review_max_iterations: config.code_review_max_iterations,
        stage_timeout_secs: config.stage_timeout_secs,
        stage_max_retries: config.stage_max_retries,
    });

    orchestrator.execute().await
}

pub(super) fn move_ticket_to_column(
    db: &Database,
    ticket: &Ticket,
    column_name: &str,
    window: Option<&Window>,
) {
    match db.find_column_by_name(&ticket.board_id, column_name) {
        Ok(Some(column)) => {
            if let Err(e) = db.move_ticket(&ticket.id, &column.id) {
                tracing::error!(
                    "Failed to move ticket {} to '{}': {}",
                    ticket.id,
                    column_name,
                    e
                );
            } else {
                tracing::info!("Moved ticket {} to column '{}'", ticket.id, column_name);
                if let Some(window) = window {
                    let _ = window.emit(
                        "ticket-moved",
                        serde_json::json!({
                            "ticketId": ticket.id,
                            "columnName": column_name,
                            "columnId": column.id,
                        }),
                    );
                }
            }
        }
        Ok(None) => {
            tracing::warn!(
                "Column '{}' not found for board {}",
                column_name,
                ticket.board_id
            );
        }
        Err(e) => {
            tracing::error!("Error finding column '{}': {}", column_name, e);
        }
    }
}

pub(super) fn handle_workflow_success(
    config: &RunnerConfig,
    duration_secs: f64,
) -> Result<super::config::RunnerResult, String> {
    tracing::info!(
        "Agent run {} completed successfully in {:.1}s",
        config.run_id,
        duration_secs
    );

    config
        .db
        .update_run_status(
            &config.run_id,
            RunStatus::Finished,
            Some(0),
            Some("Workflow completed successfully"),
        )
        .map_err(|e| format!("Failed to update run status: {}", e))?;

    if let Some(ref window) = config.window {
        let event = AgentCompleteEvent {
            run_id: config.run_id.clone(),
            status: "finished".to_string(),
            exit_code: Some(0),
            duration_secs,
        };
        if let Err(e) = window.emit("agent-complete", &event) {
            tracing::error!("Failed to emit agent-complete event: {}", e);
        }
    }

    Ok(super::config::RunnerResult {
        status: RunStatus::Finished,
        exit_code: Some(0),
        summary: Some("Workflow completed successfully".to_string()),
        duration_secs,
    })
}

pub(super) fn handle_workflow_error(
    config: &RunnerConfig,
    error: String,
    duration_secs: f64,
) -> Result<super::config::RunnerResult, String> {
    tracing::error!("Agent run {} failed: {}", config.run_id, error);

    config
        .db
        .update_run_status(
            &config.run_id,
            RunStatus::Error,
            None,
            Some(&format!("Workflow failed: {}", error)),
        )
        .map_err(|db_err| format!("Failed to update run status: {}", db_err))?;

    move_ticket_to_column(
        &config.db,
        &config.ticket,
        "Blocked",
        config.window.as_ref(),
    );

    if let Some(ref window) = config.window {
        let event = AgentErrorEvent {
            run_id: config.run_id.clone(),
            error: error.clone(),
        };
        if let Err(emit_err) = window.emit("agent-error", &event) {
            tracing::error!("Failed to emit agent-error event: {}", emit_err);
        }
    }

    Ok(super::config::RunnerResult {
        status: RunStatus::Error,
        exit_code: None,
        summary: Some(format!("Workflow failed: {}", error)),
        duration_secs,
    })
}
