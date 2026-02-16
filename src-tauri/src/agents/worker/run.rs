//! Run creation and result handling for worker operations.

use std::sync::Arc;

use crate::db::{AgentRun, AgentType, CreateRun, Database, RunStatus, Ticket};

use super::super::runner::{self, CancelHandlesMap};
use super::super::worktree::{self, WorktreeInfo};
use super::WorkerConfig;

/// Create a new run or resume a paused one.
pub fn create_or_resume_run(
    db: &Arc<Database>,
    config: &WorkerConfig,
    cancel_handles: &CancelHandlesMap,
    ticket: &Ticket,
    working_path: &std::path::Path,
    worktree: &WorktreeInfo,
    worker_id: &str,
) -> Result<AgentRun, Box<dyn std::error::Error + Send + Sync>> {
    if let Some(ref paused_run_id) = ticket.paused_run_id {
        tracing::info!(
            "Worker {} resuming paused run {} for ticket {}",
            worker_id,
            paused_run_id,
            ticket.id
        );

        match db.get_run(paused_run_id) {
            Ok(mut existing_run) => {
                if let Err(e) = db.update_run_status(paused_run_id, RunStatus::Running, None, None) {
                    tracing::error!("Failed to update paused run status to running: {}", e);
                    let _ = db.unlock_ticket(&ticket.id);
                    let _ = worktree::remove_worktree(&worktree.path, &worktree.repo_path);
                    return Err(e.into());
                }

                {
                    let mut handles = cancel_handles.lock().expect("cancel handles mutex poisoned");
                    if handles.remove(paused_run_id).is_some() {
                        tracing::info!(
                            "Removed old cancelled handle for run {} to allow resume",
                            paused_run_id
                        );
                    }
                }

                existing_run.repo_path = working_path.to_string_lossy().to_string();
                existing_run.status = RunStatus::Running;
                Ok(existing_run)
            }
            Err(e) => {
                tracing::warn!(
                    "Could not find paused run {}, creating new run: {}",
                    paused_run_id,
                    e
                );
                create_new_run(db, config, ticket, working_path, worktree)
            }
        }
    } else {
        create_new_run(db, config, ticket, working_path, worktree)
    }
}

/// Create a new run in the database.
fn create_new_run(
    db: &Arc<Database>,
    config: &WorkerConfig,
    ticket: &Ticket,
    working_path: &std::path::Path,
    worktree: &WorktreeInfo,
) -> Result<AgentRun, Box<dyn std::error::Error + Send + Sync>> {
    match db.create_run(&CreateRun {
        ticket_id: ticket.id.clone(),
        agent_type: AgentType::parse_agent(&config.agent_id),
        repo_path: working_path.to_string_lossy().to_string(),
        parent_run_id: None,
        stage: None,
        ..Default::default()
    }) {
        Ok(run) => Ok(run),
        Err(e) => {
            let _ = db.unlock_ticket(&ticket.id);
            let _ = worktree::remove_worktree(&worktree.path, &worktree.repo_path);
            Err(e.into())
        }
    }
}

/// Handle the result of a run and update task status accordingly.
pub fn handle_run_result(
    db: &Arc<Database>,
    result: &Result<runner::RunnerResult, String>,
    run_id: &str,
    task: Option<&crate::db::models::Task>,
    worker_id: &str,
) {
    match result {
        Ok(r) => {
            tracing::info!(
                "Worker {} completed run {} with status {:?} in {:.1}s",
                worker_id,
                run_id,
                r.status,
                r.duration_secs
            );

            if let Some(t) = task {
                let task_result = match r.status {
                    RunStatus::Finished => db.complete_task(&t.id),
                    RunStatus::Error => db.fail_task(&t.id),
                    RunStatus::Aborted => {
                        tracing::info!("Skipping task update for aborted run - task already reset");
                        Ok(t.clone())
                    }
                    RunStatus::Paused => {
                        tracing::info!("Skipping task update for paused run - task already reset");
                        Ok(t.clone())
                    }
                    _ => Ok(t.clone()),
                };
                if let Err(e) = task_result {
                    tracing::warn!("Failed to update task {} status: {}", t.id, e);
                }
            }
        }
        Err(e) => {
            let error_str = e.to_string();
            let was_cancelled_or_paused = error_str.contains("cancelled")
                || error_str.contains("Cancelled")
                || error_str.contains("paused")
                || error_str.contains("Paused");

            if was_cancelled_or_paused {
                tracing::info!("Worker {} run {} was cancelled/paused", worker_id, run_id);
            } else {
                tracing::error!("Worker {} run {} failed: {}", worker_id, run_id, e);

                if let Some(t) = task {
                    if let Err(fail_err) = db.fail_task(&t.id) {
                        tracing::warn!("Failed to mark task {} as failed: {}", t.id, fail_err);
                    }
                }
            }
        }
    }
}
