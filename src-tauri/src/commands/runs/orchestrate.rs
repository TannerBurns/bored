//! Background workflow execution logic.
//!
//! Extracted from `start_agent_run` to keep the command handler focused on
//! setup/validation while this module owns the long-running async workflow.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::{Emitter, Manager, Window};

use crate::agents::orchestrator::{OrchestratorConfig, WorkflowOrchestrator};
use crate::agents::spawner::CancelHandle;
use crate::agents::worktree::{self, WorktreeInfo};
use crate::agents::AgentProvider;
use crate::db::models::{RunStatus, Task};
use crate::db::{Database, Ticket};

use super::branch::start_heartbeat;
use super::{AgentCompleteEvent, AgentErrorEvent, StageConfig};

/// All context needed by the background workflow task.
///
/// Collected in `start_agent_run` before spawning, then moved into the
/// async task to avoid capturing dozens of individual variables.
pub(super) struct WorkflowTaskContext {
    pub db: Arc<Database>,
    pub window: Window,
    pub run_id: String,
    pub ticket_id: String,
    pub ticket: Ticket,
    pub worktree_info: Option<WorktreeInfo>,
    pub main_repo_path: PathBuf,
    pub branch_name: String,
    pub agent_id: String,
    pub provider: Arc<dyn AgentProvider>,
    pub api_url: String,
    pub api_token: String,
    pub cancel_handles: Arc<Mutex<HashMap<String, CancelHandle>>>,
    pub agent_config: HashMap<String, serde_json::Value>,
    pub workflow_settings: Arc<Mutex<crate::commands::workflow_settings::PerAgentSettings>>,
    pub stage_configs: HashMap<String, StageConfig>,
    pub code_review_max_iterations: usize,
    pub stage_timeout_secs: u64,
    pub stage_max_retries: u32,
}

/// Execute the multi-stage workflow in the background.
///
/// Handles heartbeat, orchestrator creation, task lifecycle (start/complete/fail),
/// result handling, ticket unlock, and worktree cleanup.
pub(super) async fn execute_workflow_task(ctx: WorkflowTaskContext) {
    let WorkflowTaskContext {
        db,
        window,
        run_id,
        ticket_id,
        ticket,
        worktree_info,
        main_repo_path,
        branch_name,
        agent_id,
        provider,
        api_url,
        api_token,
        cancel_handles,
        agent_config,
        workflow_settings,
        stage_configs,
        code_review_max_iterations,
        stage_timeout_secs,
        stage_max_retries,
    } = ctx;

    if let Err(e) = db.update_run_status(&run_id, RunStatus::Running, None, None) {
        tracing::error!("Failed to update run status: {}", e);
    }

    let running_flag = Arc::new(AtomicBool::new(true));
    let heartbeat_handle = start_heartbeat(
        db.clone(), ticket_id.clone(), run_id.clone(), running_flag.clone(),
    );

    let cancel_handles_for_cleanup = cancel_handles.clone();
    let orchestrator_working_path = worktree_info
        .as_ref()
        .map(|w| w.path.clone())
        .unwrap_or_else(|| main_repo_path.clone());

    let worktree_branch = Some(branch_name);
    let branch_already_created = worktree_info.is_some() || ticket.branch_name.is_some();

    tracing::debug!(
        "Orchestrator config: worktree_branch={:?}, branch_already_created={}",
        worktree_branch, branch_already_created
    );

    let task = db.get_next_pending_task(&ticket.id).ok().flatten();

    // Must succeed before continuing — complete_task/fail_task require 'in_progress' status
    if let Some(ref t) = task {
        if let Err(e) = db.start_task(&t.id, &run_id) {
            tracing::error!(
                "Failed to mark task {} as in_progress: {}. Aborting run to prevent stuck task.",
                t.id, e
            );
            let _ = db.update_run_status(
                &run_id, RunStatus::Error, None,
                Some(&format!("Failed to start task: {}", e)),
            );
            let _ = db.unlock_ticket(&ticket_id);
            if let Some(ref wt) = worktree_info {
                let _ = worktree::remove_worktree(&wt.path, &main_repo_path);
            }
            let _ = window.emit("agent-error", &AgentErrorEvent {
                run_id: run_id.clone(),
                error: format!("Failed to start task: {}", e),
            });
            return;
        }
    }

    let resume_from_stage = ticket.paused_at_stage.clone();
    let previous_run_id = ticket.paused_run_id.clone();
    if let Some(ref stage) = resume_from_stage {
        tracing::info!(
            "Resuming ticket {} from stage '{}' (previous run: {:?})",
            ticket.id, stage, previous_run_id
        );
    }

    if ticket.paused_run_id.is_some() {
        if let Err(e) = db.clear_ticket_pause(&ticket.id) {
            tracing::warn!("Failed to clear ticket pause state: {}", e);
        }
    }

    let orchestrator = WorkflowOrchestrator::new(OrchestratorConfig {
        db: db.clone(),
        window: Some(window.clone()),
        app_handle: Some(window.app_handle().clone()),
        parent_run_id: run_id.clone(),
        ticket: ticket.clone(),
        task: task.clone(),
        repo_path: orchestrator_working_path,
        agent_id,
        provider,
        api_url,
        api_token,
        cancel_handles,
        worktree_branch,
        branch_already_created,
        is_temp_branch: false,
        agent_config,
        resume_from_stage,
        previous_run_id,
        workflow_settings,
        stage_configs,
        code_review_max_iterations,
        stage_timeout_secs,
        stage_max_retries,
    });

    tracing::info!("Starting workflow for run {}", run_id);
    let start_time = std::time::Instant::now();
    let result = orchestrator.execute().await;
    let duration_secs = start_time.elapsed().as_secs_f64();
    tracing::info!("Workflow for run {} completed in {:.1}s (ok={})", run_id, duration_secs, result.is_ok());

    running_flag.store(false, Ordering::SeqCst);
    heartbeat_handle.abort();

    {
        let mut handles = cancel_handles_for_cleanup
            .lock()
            .expect("cancel handles mutex poisoned");
        handles.remove(&run_id);
    }

    handle_workflow_result(&db, &window, &run_id, &ticket_id, &task, result, duration_secs);

    if let Err(e) = db.unlock_ticket(&ticket_id) {
        tracing::error!("Failed to unlock ticket {}: {}", ticket_id, e);
    }

    if let Some(ref wt) = worktree_info {
        if let Err(e) = worktree::remove_worktree(&wt.path, &main_repo_path) {
            tracing::error!("Failed to remove worktree {}: {}", wt.path.display(), e);
        }
    }
}

/// Handle the orchestrator result -- update DB status, manage tasks, emit events.
fn handle_workflow_result(
    db: &Database,
    window: &Window,
    run_id: &str,
    ticket_id: &str,
    task: &Option<Task>,
    result: Result<(), String>,
    duration_secs: f64,
) {
    match result {
        Ok(()) => {
            if let Err(e) = db.update_run_status(
                run_id, RunStatus::Finished, Some(0),
                Some("Multi-stage workflow completed successfully"),
            ) {
                tracing::error!("Failed to update run {} status to Finished: {}", run_id, e);
            }

            if let Some(ref t) = task {
                if let Err(e) = db.complete_task(&t.id) {
                    tracing::warn!("Failed to mark task {} as completed: {}", t.id, e);
                }
            }

            if task.is_some() {
                match db.has_pending_tasks(ticket_id) {
                    Ok(true) => {
                        if let Err(e) = crate::commands::tasks::move_to_ready_if_completed(db, ticket_id) {
                            tracing::warn!("Failed to move ticket {} back to Ready: {}", ticket_id, e);
                        }
                    }
                    Ok(false) => {}
                    Err(e) => {
                        tracing::warn!("Failed to check pending tasks for ticket {}: {}", ticket_id, e);
                    }
                }
            }

            let event = AgentCompleteEvent {
                run_id: run_id.to_string(),
                status: "finished".to_string(),
                exit_code: Some(0),
                duration_secs,
            };
            if let Err(e) = window.emit("agent-complete", &event) {
                tracing::error!("Failed to emit agent-complete event: {}", e);
            }
        }
        Err(e) => {
            let was_cancelled_or_paused = e.contains("cancelled")
                || e.contains("Cancelled")
                || e.contains("paused")
                || e.contains("Paused");

            if !was_cancelled_or_paused {
                tracing::error!("Workflow failed for run {}: {}", run_id, e);
                if let Err(db_err) = db.update_run_status(
                    run_id, RunStatus::Error, None,
                    Some(&format!("Multi-stage workflow failed: {}", e)),
                ) {
                    tracing::error!("Failed to update run {} status to Error: {}", run_id, db_err);
                }

                if let Some(ref t) = task {
                    if let Err(fail_err) = db.fail_task(&t.id) {
                        tracing::warn!("Failed to mark task {} as failed: {}", t.id, fail_err);
                    }
                }
            }

            let event = AgentErrorEvent {
                run_id: run_id.to_string(),
                error: e,
            };
            if let Err(emit_err) = window.emit("agent-error", &event) {
                tracing::error!("Failed to emit agent-error event: {}", emit_err);
            }
        }
    }
}
