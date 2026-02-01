//! Shared agent runner logic for both direct runs and worker-initiated runs.
//! 
//! This module provides a unified execution path for agent runs, ensuring
//! consistent behavior regardless of whether a run is triggered manually
//! or by an automated worker.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Window};

use crate::db::{Database, RunStatus, Ticket};
use crate::db::models::Task;
use super::{AgentKind, ClaudeApiConfig};
use super::spawner::CancelHandle;
use super::orchestrator::{WorkflowOrchestrator, OrchestratorConfig};
use super::claude as claude_hooks;
use super::cursor as cursor_hooks;

/// Re-export CancelHandlesMap for use by the worker
pub type CancelHandlesMap = Arc<std::sync::Mutex<HashMap<String, CancelHandle>>>;

/// Configuration for running an agent
pub struct RunnerConfig {
    pub db: Arc<Database>,
    pub window: Option<Window>,
    pub app_handle: Option<AppHandle>,
    pub ticket: Ticket,
    /// The task being executed. If None, falls back to legacy ticket-based workflow.
    pub task: Option<Task>,
    pub run_id: String,
    pub repo_path: PathBuf,
    pub agent_kind: AgentKind,
    pub api_url: String,
    pub api_token: String,
    pub hook_script_path: Option<String>,
    pub cancel_handles: CancelHandlesMap,
    pub worktree_branch: Option<String>,
    /// Whether the branch was already created (e.g., via worktree creation).
    pub branch_already_created: bool,
    /// Whether the worktree branch is a temporary name that should be renamed to an AI-generated name.
    pub is_temp_branch: bool,
    pub timeout_secs: u64,
    /// Claude API configuration (auth token, api key, base url, model override)
    pub claude_api_config: Option<ClaudeApiConfig>,
}

/// Result of an agent run execution
pub struct RunnerResult {
    pub status: RunStatus,
    pub exit_code: Option<i32>,
    pub summary: Option<String>,
    pub duration_secs: f64,
}

/// Log event for frontend emission
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentLogEvent {
    pub run_id: String,
    pub stream: String,
    pub content: String,
    pub timestamp: String,
}

/// Complete event for frontend emission
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCompleteEvent {
    pub run_id: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub duration_secs: f64,
}

/// Error event for frontend emission
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentErrorEvent {
    pub run_id: String,
    pub error: String,
}

/// Update project hooks with run-specific configuration
fn update_project_hooks_for_run(
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
        AgentKind::Cursor => {
            cursor_hooks::install_hooks_with_run_id(
                repo_path,
                hook_script_path,
                Some(api_url),
                Some(api_token),
                Some(run_id),
            )
            .map_err(|e| format!("Failed to update Cursor hooks.json: {}", e))
        }
        AgentKind::Claude => {
            claude_hooks::install_local_hooks_with_run_id(
                repo_path,
                hook_script_path,
                Some(api_url),
                Some(api_token),
                Some(run_id),
            )
            .map_err(|e| format!("Failed to update Claude settings.local.json: {}", e))
        }
    }
}

/// Execute an agent run with the given configuration.
/// 
/// This is the main entry point for running agents - used by both
/// the Tauri command (`start_agent_run`) and the worker system.
/// 
/// Handles:
/// - Multi-stage vs basic workflow detection
/// - Log streaming and persistence
/// - Branch comment detection
/// - Agent summary extraction and comment creation
/// - Ticket movement between columns
pub async fn execute_agent_run(config: RunnerConfig) -> Result<RunnerResult, String> {
    let start_time = std::time::Instant::now();
    
    tracing::info!(
        "execute_agent_run: ticket={}, run_id={}, workflow_type={:?}, agent={:?}",
        config.ticket.id,
        config.run_id,
        config.ticket.workflow_type,
        config.agent_kind
    );
    
    // Update project hooks with run configuration
    if let Some(ref hook_path) = config.hook_script_path {
        if let Err(e) = update_project_hooks_for_run(
            &config.repo_path,
            hook_path,
            &config.api_url,
            &config.api_token,
            &config.run_id,
            config.agent_kind,
        ) {
            tracing::warn!("Failed to update project hooks: {}", e);
            // Continue anyway - hooks might already be configured
        }
    }
    
    // Update run status to running
    config.db.update_run_status(&config.run_id, RunStatus::Running, None, None)
        .map_err(|e| format!("Failed to update run status: {}", e))?;
    
    // All tickets use multi-stage workflow now (WorkflowType is always MultiStage)
    // The orchestrator handles the full workflow with proper stage tracking
    let result = execute_multi_stage_workflow(&config).await;
    
    let duration_secs = start_time.elapsed().as_secs_f64();
    
    match result {
        Ok(()) => {
            tracing::info!("Agent run {} completed successfully in {:.1}s", config.run_id, duration_secs);
            
            config.db.update_run_status(
                &config.run_id,
                RunStatus::Finished,
                Some(0),
                Some("Workflow completed successfully"),
            ).map_err(|e| format!("Failed to update run status: {}", e))?;
            
            // Emit completion event if we have a window
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
            
            Ok(RunnerResult {
                status: RunStatus::Finished,
                exit_code: Some(0),
                summary: Some("Workflow completed successfully".to_string()),
                duration_secs,
            })
        }
        Err(e) => {
            tracing::error!("Agent run {} failed: {}", config.run_id, e);
            
            config.db.update_run_status(
                &config.run_id,
                RunStatus::Error,
                None,
                Some(&format!("Workflow failed: {}", e)),
            ).map_err(|db_err| format!("Failed to update run status: {}", db_err))?;
            
            // Move ticket to Blocked on error
            move_ticket_to_column(&config.db, &config.ticket, "Blocked", config.window.as_ref());
            
            // Emit error event if we have a window
            if let Some(ref window) = config.window {
                let event = AgentErrorEvent {
                    run_id: config.run_id.clone(),
                    error: e.clone(),
                };
                if let Err(emit_err) = window.emit("agent-error", &event) {
                    tracing::error!("Failed to emit agent-error event: {}", emit_err);
                }
            }
            
            Ok(RunnerResult {
                status: RunStatus::Error,
                exit_code: None,
                summary: Some(format!("Workflow failed: {}", e)),
                duration_secs,
            })
        }
    }
}

/// Execute a multi-stage workflow using the orchestrator
async fn execute_multi_stage_workflow(config: &RunnerConfig) -> Result<(), String> {
    tracing::info!("Starting multi-stage workflow for run {}", config.run_id);
    
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
    });
    
    orchestrator.execute().await
}

/// Move a ticket to a column by name
fn move_ticket_to_column(db: &Database, ticket: &Ticket, column_name: &str, window: Option<&Window>) {
    match db.find_column_by_name(&ticket.board_id, column_name) {
        Ok(Some(column)) => {
            if let Err(e) = db.move_ticket(&ticket.id, &column.id) {
                tracing::error!("Failed to move ticket {} to '{}': {}", ticket.id, column_name, e);
            } else {
                tracing::info!("Moved ticket {} to column '{}'", ticket.id, column_name);
                if let Some(window) = window {
                    let _ = window.emit("ticket-moved", serde_json::json!({
                        "ticketId": ticket.id,
                        "columnName": column_name,
                        "columnId": column.id,
                    }));
                }
            }
        }
        Ok(None) => {
            tracing::warn!("Column '{}' not found for board {}", column_name, ticket.board_id);
        }
        Err(e) => {
            tracing::error!("Error finding column '{}': {}", column_name, e);
        }
    }
}

/// Create a default cancel handles map
pub fn create_cancel_handles() -> CancelHandlesMap {
    Arc::new(Mutex::new(HashMap::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_log_event_serializes() {
        let event = AgentLogEvent {
            run_id: "run-1".to_string(),
            stream: "stdout".to_string(),
            content: "Hello".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("runId"));
        assert!(json.contains("stdout"));
    }

    #[test]
    fn agent_complete_event_serializes() {
        let event = AgentCompleteEvent {
            run_id: "run-1".to_string(),
            status: "success".to_string(),
            exit_code: Some(0),
            duration_secs: 123.45,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("durationSecs"));
        assert!(json.contains("exitCode"));
    }

    #[test]
    fn agent_error_event_serializes() {
        let event = AgentErrorEvent {
            run_id: "run-1".to_string(),
            error: "Something went wrong".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("runId"));
        assert!(json.contains("error"));
    }
}
