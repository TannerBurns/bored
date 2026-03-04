mod branch;
mod cost_commands;
mod orchestrate;
mod queries;
#[cfg(test)]
mod tests;

// Re-export all public items from submodules
pub use cost_commands::*;
pub use queries::*;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{Manager, State, Window};

use crate::agents::spawner::CancelHandle;
use crate::agents::worker::error_handling::{self, WorktreeFailureContext};
use crate::agents::AgentRegistry;
use crate::commands::agent_settings::AgentSettingsManager;
use crate::db::models::{CreateRun, RunStatus};
use crate::db::Database;

use branch::{setup_worktree_and_branch, WorktreeBranchSetup};

/// Shared state for tracking running agents
pub struct RunningAgents {
    pub handles: Arc<Mutex<HashMap<String, CancelHandle>>>,
}

impl RunningAgents {
    pub fn new() -> Self {
        Self {
            handles: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for RunningAgents {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageConfig {
    pub enabled: bool,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRunInput {
    pub ticket_id: String,
    pub agent_type: String,
    pub repo_path: String,
    pub code_review_max_iterations: Option<usize>,
    pub stage_timeout_hours: Option<u32>,
    pub stage_max_retries: Option<u32>,
    pub stage_configs: Option<HashMap<String, StageConfig>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentLogEvent {
    pub run_id: String,
    pub stream: String,
    pub content: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCompleteEvent {
    pub run_id: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub duration_secs: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentErrorEvent {
    pub run_id: String,
    pub error: String,
}

#[tauri::command]
pub async fn start_agent_run(
    window: Window,
    input: StartRunInput,
    db: State<'_, Arc<Database>>,
    running_agents: State<'_, RunningAgents>,
    agent_settings: State<'_, AgentSettingsManager>,
    workflow_settings_state: State<'_, crate::commands::workflow_settings::WorkflowSettingsState>,
    registry: State<'_, Arc<AgentRegistry>>,
) -> Result<String, String> {
    let StartRunInput {
        ticket_id,
        agent_type,
        repo_path,
        code_review_max_iterations,
        stage_timeout_hours,
        stage_max_retries,
        stage_configs,
    } = input;

    tracing::info!("=== START_AGENT_RUN CALLED ===");
    tracing::info!(
        "Agent type: {}, Ticket ID: {}, Repo path: {}",
        agent_type,
        ticket_id,
        repo_path
    );

    let agent_id = agent_type.clone();
    let provider = registry
        .get(&agent_id)
        .ok_or_else(|| format!("Unknown agent type: {}", agent_id))?;
    let db_agent_type = agent_id.clone();

    let agent_config = agent_settings.agent_config_for(&agent_id);

    let ticket = db
        .get_ticket(&ticket_id)
        .map_err(|e| format!("Failed to get ticket: {}", e))?;

    // If resuming from a paused run, reuse the same run ID for continuity
    let run = if let Some(ref paused_run_id) = ticket.paused_run_id {
        tracing::info!(
            "Resuming paused run {} for ticket {}",
            paused_run_id,
            ticket_id
        );

        // Get the existing run and update its status back to running
        let existing_run = db
            .get_run(paused_run_id)
            .map_err(|e| format!("Failed to get paused run: {}", e))?;

        db.update_run_status(paused_run_id, RunStatus::Running, None, None)
            .map_err(|e| format!("Failed to update paused run status: {}", e))?;

        // CRITICAL: Remove the old cancelled handle so the orchestrator doesn't
        // immediately detect cancellation when checking is_cancelled()
        {
            let mut handles = running_agents
                .handles
                .lock()
                .expect("running agents mutex poisoned");
            if handles.remove(paused_run_id).is_some() {
                tracing::info!(
                    "Removed old cancelled handle for run {} to allow resume",
                    paused_run_id
                );
            }
        }

        existing_run
    } else {
        db.create_run(&CreateRun {
            ticket_id: ticket_id.clone(),
            agent_type: db_agent_type,
            repo_path: repo_path.clone(),
            parent_run_id: None,
            stage: None,
            ..Default::default()
        })
        .map_err(|e| format!("Failed to create run: {}", e))?
    };

    let run_id = run.id.clone();

    // Lock the ticket with a 30-minute expiration (same as worker default)
    // This ensures the cleanup service can release stale locks
    let lock_expires_at = chrono::Utc::now() + chrono::Duration::minutes(30);
    db.lock_ticket(&ticket_id, &run_id, lock_expires_at)
        .map_err(|e| format!("Failed to lock ticket: {}", e))?;
    tracing::info!(
        "Locked ticket {} with run {} until {}",
        ticket_id,
        run_id,
        lock_expires_at
    );

    // Create git worktree for isolated execution (same approach as worker path)
    let (worktree_info, branch_name) = match setup_worktree_and_branch(WorktreeBranchSetup {
        ticket: &ticket,
        run_id: &run_id,
        repo_path: &repo_path,
        db: db.inner(),
    }).await {
        Ok(result) => result,
        Err(worktree_err) => {
            let err_msg = format!("Worktree setup failed: {}", worktree_err);

            if let Err(e) = db.update_run_status(
                &run_id, RunStatus::Error, None, Some(&err_msg),
            ) {
                tracing::error!("Failed to mark run {} as Error: {}", run_id, e);
            }

            let repo_path_buf = std::path::PathBuf::from(&repo_path);
            let diagnostic_model = {
                let ws = workflow_settings_state.shared();
                let per_agent = ws.lock().expect("workflow settings mutex poisoned");
                per_agent.get(&agent_id).map(|s| s.diagnostic_model.clone())
            };

            let ticket_blocked = error_handling::handle_worktree_failure(WorktreeFailureContext {
                db: db.inner().clone(),
                app_handle: Some(window.app_handle().clone()),
                ticket: &ticket,
                repo_path: &repo_path_buf,
                error: &worktree_err,
                provider,
                agent_config,
                worker_id: "direct-run",
                diagnostic_model,
            })
            .await;

            if ticket_blocked {
                if let Err(e) = db.unlock_ticket(&ticket_id) {
                    tracing::error!("Failed to unlock ticket {} after worktree failure: {}", ticket_id, e);
                }
            } else {
                tracing::warn!(
                    "Could not move ticket {} to Blocked column; keeping lock active to prevent re-queuing",
                    ticket_id,
                );
            }

            return Err(err_msg);
        }
    };

    let working_path = worktree_info.path.clone();

    tracing::info!(
        "Agent will work in: {} (worktree branch: {})",
        working_path.display(),
        worktree_info.branch_name
    );

    // All tickets now use multi-stage workflow
    tracing::info!(
        "Workflow type: {:?}, run_id: {}, working_path: {}",
        ticket.workflow_type,
        run_id,
        working_path.display()
    );

    let db_clone = db.inner().clone();
    let run_id_for_task = run_id.clone();
    let ticket_id_for_task = ticket_id.clone();
    let window_clone = window.clone();

    // Store original repo path for worktree cleanup
    let main_repo_path = std::path::PathBuf::from(&repo_path);

    // Clone the Arc<Mutex<HashMap>> so we can move it into the async task
    let running_agents_handles = running_agents.handles.clone();

    // The orchestrator reads workflow settings (stage configs, models,
    // timeouts, retries) directly from the shared WorkflowSettingsState at
    // construction time — that is the single source of truth.
    // We pass the Arc reference so the orchestrator can lock and read it.
    let shared_workflow_settings = workflow_settings_state.shared();

    // Build workflow context and spawn background task
    let ctx = orchestrate::WorkflowTaskContext {
        db: db_clone,
        window: window_clone,
        run_id: run_id_for_task,
        ticket_id: ticket_id_for_task,
        ticket,
        worktree_info,
        main_repo_path,
        branch_name,
        agent_id,
        provider,
        cancel_handles: running_agents_handles,
        agent_config,
        workflow_settings: shared_workflow_settings,
        stage_configs: stage_configs.unwrap_or_default(),
        code_review_max_iterations: code_review_max_iterations.unwrap_or(3),
        stage_timeout_secs: stage_timeout_hours.map(|h| h as u64 * 3600).unwrap_or(3600),
        stage_max_retries: stage_max_retries.unwrap_or(2),
    };

    tauri::async_runtime::spawn(orchestrate::execute_workflow_task(ctx));

    Ok(run_id)
}

#[tauri::command]
pub async fn cancel_agent_run(
    run_id: String,
    is_pause: Option<bool>,
    db: State<'_, Arc<Database>>,
    running_agents: State<'_, RunningAgents>,
) -> Result<(), String> {
    let is_pause = is_pause.unwrap_or(false);
    tracing::info!(
        "{} agent run: {}",
        if is_pause { "Pausing" } else { "Cancelling" },
        run_id
    );

    // Try to cancel via handle
    let handle_found = {
        let handles = running_agents
            .handles
            .lock()
            .expect("running agents mutex poisoned");

        if let Some(handle) = handles.get(&run_id) {
            let was_already_cancelled = handle.is_cancelled();
            handle.cancel();
            tracing::info!(
                "Cancel handle found for run {}, cancelled (was_already_cancelled: {})",
                run_id,
                was_already_cancelled
            );
            true
        } else {
            // Log available handles for debugging
            let available_handles: Vec<_> = handles.keys().collect();
            tracing::warn!(
                "No cancel handle found for run {}. Available handles: {:?}",
                run_id,
                available_handles
            );
            false
        }
    };

    if !handle_found {
        tracing::warn!(
            "Run {} may have already finished or not started yet. \
            Updating DB status anyway.",
            run_id
        );
    }

    // Update the status in the database - use Paused for pause, Aborted for cancel
    let (status, message) = if is_pause {
        (RunStatus::Paused, "Paused by user")
    } else {
        (RunStatus::Aborted, "Cancelled by user")
    };

    db.update_run_status(&run_id, status, None, Some(message))
        .map_err(|e| e.to_string())?;

    // Reset any in-progress task back to pending so it can be retried
    match db.reset_tasks_for_run(&run_id) {
        Ok(count) => {
            if count > 0 {
                tracing::info!(
                    "Reset {} task(s) to pending for {} run {}",
                    count,
                    if is_pause { "paused" } else { "cancelled" },
                    run_id
                );
            }
        }
        Err(e) => {
            tracing::warn!(
                "Failed to reset tasks for {} run {}: {}",
                if is_pause { "paused" } else { "cancelled" },
                run_id,
                e
            );
        }
    }

    // Also unlock any ticket that was locked by this run (but NOT when pausing)
    // When pausing, we keep the lock so the resume flow can find the paused run
    if !is_pause {
        if let Ok(run) = db.get_run(&run_id) {
            if let Err(e) = db.unlock_ticket(&run.ticket_id) {
                tracing::warn!("Failed to unlock ticket after cancel: {}", e);
            }
        }
    }

    Ok(())
}
