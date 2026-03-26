//! Worker module for continuous, automated ticket processing.
//!
//! Workers are automated agents that poll for tickets in the "Ready" column
//! and process them using the same execution path as manual runs.
//!
//! This module is split into focused submodules:
//! - `config`: Configuration types and status definitions
//! - `heartbeat`: Lock extension management
//! - `error_handling`: Worktree failure handling and ticket blocking
//! - `manager`: Worker lifecycle management

use chrono::Utc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use super::runner::{self, CancelHandlesMap, RunnerConfig};
use super::worktree;
use crate::db::{AuthorType, CreateComment, Database, RunStatus, Ticket};

// Submodules
pub(crate) mod branching;
mod config;
pub(crate) mod error_handling;
mod heartbeat;
mod manager;
mod run;
mod worktree_setup;

// Public re-exports
pub use config::{WorkerConfig, WorkerState, WorkerStatus};
pub use manager::WorkerManager;
pub use worktree_setup::{WorkspaceWorktreeError, WorkspaceWorktreeSet, create_worktrees_for_workspace};

pub struct Worker {
    pub id: String,
    config: WorkerConfig,
    db: Arc<Database>,
    running: Arc<AtomicBool>,
    status: Arc<std::sync::Mutex<WorkerStatus>>,
    cancel_handles: CancelHandlesMap,
}

impl Worker {
    pub fn new(
        id: String,
        config: WorkerConfig,
        db: Arc<Database>,
        cancel_handles: Option<CancelHandlesMap>,
    ) -> Self {
        let status = WorkerStatus {
            id: id.clone(),
            agent_type: config.agent_id.clone(),
            project_id: config.project_id.clone(),
            status: WorkerState::Idle,
            current_ticket_id: None,
            current_run_id: None,
            tickets_processed: 0,
            started_at: Utc::now(),
            last_poll_at: None,
        };

        Self {
            id,
            config,
            db,
            running: Arc::new(AtomicBool::new(false)),
            status: Arc::new(std::sync::Mutex::new(status)),
            cancel_handles: cancel_handles.unwrap_or_else(runner::create_cancel_handles),
        }
    }

    pub fn get_status(&self) -> WorkerStatus {
        self.status.lock().expect("status mutex poisoned").clone()
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn stop(&self) {
        tracing::info!("Stopping worker {}", self.id);
        self.running.store(false, Ordering::Relaxed);

        // Cancel any running agent by cancelling all handles
        let handles = self.cancel_handles.lock().expect("cancel mutex poisoned");
        for (run_id, handle) in handles.iter() {
            tracing::info!("Cancelling run {} for worker {}", run_id, self.id);
            handle.cancel();
        }

        let mut status = self.status.lock().expect("status mutex poisoned");
        status.status = WorkerState::Stopped;
    }

    pub async fn run(&self) {
        self.running.store(true, Ordering::Relaxed);

        {
            let mut status = self.status.lock().expect("status mutex poisoned");
            status.status = WorkerState::Idle;
            status.started_at = Utc::now();
        }

        tracing::info!(
            "Worker {} started: {:?} agent, project filter: {:?}",
            self.id,
            self.config.agent_id,
            self.config.project_id
        );

        while self.running.load(Ordering::Relaxed) {
            match self.process_next().await {
                Ok(true) => {}
                Ok(false) => sleep(Duration::from_secs(self.config.poll_interval_secs)).await,
                Err(e) => {
                    tracing::error!("Worker {} error: {}", self.id, e);
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }

        tracing::info!("Worker {} stopped", self.id);
    }

    async fn process_next(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        {
            let mut status = self.status.lock().expect("status mutex poisoned");
            status.last_poll_at = Some(Utc::now());
        }

        let run_id = uuid::Uuid::new_v4().to_string();
        let lock_expires =
            chrono::Utc::now() + chrono::Duration::minutes(self.config.lock_duration_mins);

        // Try to reserve the next available ticket
        let Some(ticket) = self.db.reserve_next_ticket(
            self.config.project_id.as_deref(),
            &self.config.agent_id,
            &run_id,
            lock_expires,
        )?
        else {
            // Log diagnostics to help debug why no tickets are being picked up
            if let Ok(diag) = self.db.get_ready_ticket_diagnostics(
                self.config.project_id.as_deref(),
                &self.config.agent_id,
            ) {
                if diag.total_ready > 0 {
                    tracing::debug!(
                        "Worker {} found no eligible tickets: {} total in Ready (paused={}, locked={}, epics={}, wrong_project={}, eligible={})",
                        self.id,
                        diag.total_ready,
                        diag.paused,
                        diag.locked,
                        diag.epics,
                        diag.wrong_project,
                        diag.eligible
                    );
                }
            }
            return Ok(false);
        };

        tracing::info!("Worker {} reserved ticket: {}", self.id, ticket.id);

        // Check if the ticket's parent spec version is paused or halted
        if let Some(ref spec_version_id) = ticket.spec_version_id {
            match self.db.get_spec_version(spec_version_id) {
                Ok(version) => {
                    use crate::db::SpecVersionStatus;
                    match version.status {
                        SpecVersionStatus::Paused | SpecVersionStatus::Halted => {
                            tracing::info!(
                                "Worker {} skipping ticket {} because spec version {} is {:?}",
                                self.id,
                                ticket.id,
                                spec_version_id,
                                version.status
                            );
                            self.db.unlock_ticket(&ticket.id)?;
                            return Ok(false);
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Worker {} failed to check spec version {} status: {}, unlocking ticket to be safe",
                        self.id,
                        spec_version_id,
                        e
                    );
                    self.db.unlock_ticket(&ticket.id)?;
                    return Ok(false);
                }
            }
        }

        // Get the repo path for this ticket
        let repo_path = match self.get_repo_path(&ticket) {
            Ok(path) => path,
            Err(e) => {
                self.db.unlock_ticket(&ticket.id)?;
                return Err(e);
            }
        };

        let diagnostic_model = self.config.workflow_settings.as_ref().and_then(|ws| {
            let per_agent = ws.lock().expect("workflow settings mutex poisoned");
            per_agent.get(&self.config.agent_id).map(|s| s.diagnostic_model.clone())
        });

        let (worktree, extra_worktrees, ws_file, ws_paths) =
            if let Some(ref workspace_id) = ticket.workspace_id {
                match worktree_setup::create_worktrees_for_workspace(
                    self.db.clone(),
                    workspace_id,
                    &ticket,
                    &run_id,
                    &self.id,
                    self.config.app_handle.clone(),
                    self.config.provider.clone(),
                    self.config.agent_config.clone(),
                    diagnostic_model,
                )
                .await
                {
                    Ok(ws_set) => {
                        let workspace_file = ws_set.workspace_file;
                        let mut worktrees = ws_set.worktrees;
                        let primary = worktrees.remove(0);
                        let paths: Vec<std::path::PathBuf> =
                            std::iter::once(primary.path.clone())
                                .chain(worktrees.iter().map(|wt| wt.path.clone()))
                                .collect();
                        tracing::info!(
                            "Worker {} created {} workspace worktrees for ticket {}",
                            self.id,
                            1 + worktrees.len(),
                            ticket.id
                        );
                        (primary, worktrees, Some(workspace_file), paths)
                    }
                    Err(e) => {
                        tracing::error!(
                            "Worker {} failed to create workspace worktrees for ticket {}: {}",
                            self.id,
                            ticket.id,
                            e.message
                        );
                        if e.ticket_blocked {
                            self.db.unlock_ticket(&ticket.id)?;
                        } else {
                            tracing::warn!(
                                "Worker {} could not move ticket {} to Blocked, keeping lock active ({} min expiry)",
                                self.id,
                                ticket.id,
                                self.config.lock_duration_mins,
                            );
                        }
                        return Err(e.message.into());
                    }
                }
            } else {
                let repo_path_buf = std::path::PathBuf::from(&repo_path);
                match worktree_setup::create_worktree_for_ticket(
                    worktree_setup::WorktreeSetupContext {
                        db: self.db.clone(),
                        ticket: &ticket,
                        run_id: &run_id,
                        repo_path: repo_path_buf,
                        worker_id: &self.id,
                        app_handle: self.config.app_handle.clone(),
                        provider: self.config.provider.clone(),
                        agent_config: self.config.agent_config.clone(),
                        diagnostic_model,
                    },
                )
                .await
                {
                    worktree_setup::WorktreeSetupResult::Success(info) => {
                        (info, vec![], None, vec![])
                    }
                    worktree_setup::WorktreeSetupResult::Failed {
                        message,
                        ticket_blocked,
                    } => {
                        if ticket_blocked {
                            self.db.unlock_ticket(&ticket.id)?;
                        } else {
                            tracing::warn!(
                                "Worker {} could not move ticket {} to Blocked, keeping lock active ({} min expiry)",
                                self.id,
                                ticket.id,
                                self.config.lock_duration_mins,
                            );
                        }
                        return Err(message.into());
                    }
                }
            };
        let working_path = worktree.path.clone();

        // Create or resume run
        let run = run::create_or_resume_run(
            &self.db,
            &self.config,
            &self.cancel_handles,
            &ticket,
            &working_path,
            &worktree,
            &self.id,
        )?;

        // Transfer lock ownership from temporary run_id to actual run ID
        if let Err(e) =
            self.db
                .update_ticket_lock_owner(&ticket.id, &run_id, &run.id, Some(lock_expires))
        {
            tracing::error!(
                "Worker {} failed to transfer lock from {} to {}: {}",
                self.id,
                run_id,
                run.id,
                e
            );
            let _ = self.db.update_run_status(
                &run.id,
                RunStatus::Error,
                None,
                Some("Failed to transfer ticket lock to actual run ID"),
            );
            let _ = self.db.unlock_ticket(&ticket.id);
            let _ = worktree::remove_worktree(&worktree.path, &worktree.repo_path);
            return Err(e.into());
        }

        // Update worker status
        {
            let mut status = self.status.lock().expect("status mutex poisoned");
            status.status = WorkerState::Running;
            status.current_ticket_id = Some(ticket.id.clone());
            status.current_run_id = Some(run.id.clone());
        }

        // Start heartbeat to keep the lock alive
        let heartbeat_handle = heartbeat::start_heartbeat(
            self.db.clone(),
            ticket.id.clone(),
            run.id.clone(),
            self.config.heartbeat_interval_secs,
            self.config.lock_duration_mins,
            self.running.clone(),
        );

        // Get the next pending task for this ticket
        let task = match self.db.get_next_pending_task(&ticket.id) {
            Ok(Some(t)) => {
                tracing::info!(
                    "Worker {} found pending task {} for ticket {}",
                    self.id,
                    t.id,
                    ticket.id
                );
                Some(t)
            }
            Ok(None) => {
                tracing::warn!(
                    "Worker {} found no pending tasks for ticket {}, skipping",
                    self.id,
                    ticket.id
                );

                // Update run status to Finished (no work to do is not an error)
                if let Err(e) = self.db.update_run_status(
                    &run.id,
                    RunStatus::Finished,
                    None,
                    Some("No pending tasks for ticket"),
                ) {
                    tracing::error!(
                        "Worker {} failed to update run {} status to Finished: {}",
                        self.id,
                        run.id,
                        e
                    );
                    let _ = self.db.update_run_status(
                        &run.id,
                        RunStatus::Error,
                        None,
                        Some(&format!("Failed to update status: {}", e)),
                    );
                }

                // Now safe to stop heartbeat since status is no longer Running
                heartbeat_handle.abort();

                // Clean up
                self.db.unlock_ticket(&ticket.id)?;
                let _ = worktree::remove_worktree(&worktree.path, &worktree.repo_path);

                // Reset worker status
                {
                    let mut status = self.status.lock().expect("status mutex poisoned");
                    status.status = WorkerState::Idle;
                    status.current_ticket_id = None;
                    status.current_run_id = None;
                }

                return Ok(false);
            }
            Err(e) => {
                tracing::warn!(
                    "Worker {} failed to get tasks for ticket {}: {}",
                    self.id,
                    ticket.id,
                    e
                );
                None // Fall back to legacy ticket-based workflow
            }
        };

        // Mark task as in progress
        if let Some(ref t) = task {
            if let Err(e) = self.db.start_task(&t.id, &run.id) {
                tracing::error!(
                    "Worker {} failed to mark task {} as in_progress: {}. Aborting run to prevent stuck task.",
                    self.id, t.id, e
                );
                let _ = self.db.update_run_status(
                    &run.id,
                    RunStatus::Error,
                    None,
                    Some(&format!("Failed to start task: {}", e)),
                );
                self.db.unlock_ticket(&ticket.id)?;
                let _ = worktree::remove_worktree(&worktree.path, &worktree.repo_path);
                return Err(format!("Failed to start task {}: {}", t.id, e).into());
            }
        }

        // Prefer worktree.branch_name over ticket.branch_name — on detour
        // branches the two differ and the orchestrator needs the actual checkout.
        let (worktree_branch, branch_already_created, is_temp_branch) =
            if worktree.target_branch.is_some() {
                (Some(worktree.branch_name.clone()), true, false)
            } else if ticket.branch_name.is_some() {
                (ticket.branch_name.clone(), true, false)
            } else {
                (
                    Some(worktree.branch_name.clone()),
                    true,
                    worktree.is_temp_branch,
                )
            };

        let resume_from_stage = ticket.paused_at_stage.clone();
        if let Some(ref stage) = resume_from_stage {
            tracing::info!(
                "Worker {} resuming ticket {} from stage '{}'",
                self.id,
                ticket.id,
                stage
            );
        }

        let previous_run_id = ticket.paused_run_id.clone();

        // Pass the shared workflow settings reference directly to the runner.
        // The orchestrator will read from it at workflow-start time — this is
        // the single source of truth for stage configs, models, timeouts, etc.
        let resolved = self.config.resolve_workflow_settings();
        tracing::info!(
            "Worker {} passing shared workflow settings to runner ({} stage configs currently)",
            self.id,
            resolved.stage_configs.len(),
        );

        let runner_config = RunnerConfig {
            db: self.db.clone(),
            window: None,
            app_handle: self.config.app_handle.clone(),
            ticket: ticket.clone(),
            task: task.clone(),
            run_id: run.id.clone(),
            repo_path: working_path.clone(),
            workspace_file: ws_file.clone(),
            workspace_paths: ws_paths,
            agent_id: self.config.agent_id.clone(),
            provider: self.config.provider.clone(),
            cancel_handles: self.cancel_handles.clone(),
            worktree_branch,
            branch_already_created,
            is_temp_branch,
            target_branch: worktree.target_branch.clone(),
            timeout_secs: self.config.agent_timeout_secs,
            agent_config: self.config.agent_config.clone(),
            code_review_max_iterations: resolved.code_review_max_iterations,
            stage_timeout_secs: resolved.stage_timeout_secs,
            stage_max_retries: resolved.stage_max_retries,
            resume_from_stage,
            previous_run_id,
            stage_configs: resolved.stage_configs,
            // THE source of truth — orchestrator reads directly from this
            workflow_settings: self.config.workflow_settings.clone(),
        };

        // Clear pause state now that we've captured the resume info
        if ticket.paused_run_id.is_some() {
            if let Err(e) = self.db.clear_ticket_pause(&ticket.id) {
                tracing::warn!("Failed to clear ticket pause state: {}", e);
            } else {
                tracing::info!(
                    "Cleared pause state for ticket {} after capturing resume info",
                    ticket.id
                );
            }
        }

        let result = runner::execute_agent_run(runner_config).await;

        // Stop heartbeat
        heartbeat_handle.abort();

        // Log result and update task status
        run::handle_run_result(&self.db, &result, &run.id, task.as_ref(), &self.id);

        // Unlock the ticket
        self.db.unlock_ticket(&ticket.id)?;

        // Merge detour branch back into target if this was a detour worktree
        let mut detour_merged = false;
        if let (Some(ref target), Some(ref fork_point)) =
            (&worktree.target_branch, &worktree.detour_fork_point)
        {
            match worktree::merge_detour_into_target(
                &worktree.repo_path,
                &worktree.branch_name,
                target,
                fork_point,
            ) {
                Ok(worktree::DetourMergeResult::Merged { ref new_head }) => {
                    tracing::info!(
                        "Worker {} merged detour {} into {} (HEAD: {})",
                        self.id,
                        worktree.branch_name,
                        target,
                        new_head
                    );
                    detour_merged = true;
                }
                Ok(worktree::DetourMergeResult::MergedWorkingTreeDirty { ref new_head }) => {
                    tracing::info!(
                        "Worker {} merged detour {} into {} via update-ref (HEAD: {}, working tree not updated — dirty)",
                        self.id,
                        worktree.branch_name,
                        target,
                        new_head
                    );
                    detour_merged = true;
                    post_detour_dirty_worktree_comment(&self.db, &ticket.id, target);
                }
                Ok(worktree::DetourMergeResult::MergedWorkingTreeStale { ref new_head }) => {
                    tracing::info!(
                        "Worker {} merged detour {} into {} via update-ref (HEAD: {}, working tree stale — ff-merge failed)",
                        self.id,
                        worktree.branch_name,
                        target,
                        new_head
                    );
                    detour_merged = true;
                    post_detour_stale_worktree_comment(&self.db, &ticket.id, target);
                }
                Ok(worktree::DetourMergeResult::NothingToMerge) => {
                    tracing::info!(
                        "Worker {} detour {} had no new commits",
                        self.id,
                        worktree.branch_name
                    );
                    detour_merged = true;
                }
                Ok(worktree::DetourMergeResult::Diverged { .. }) => {
                    tracing::warn!(
                        "Worker {} target {} diverged from detour {}; leaving detour for manual merge",
                        self.id,
                        target,
                        worktree.branch_name
                    );
                    post_detour_recovery_comment(
                        &self.db, &ticket.id, &worktree.branch_name, target,
                        "The target branch has diverged since the agent started working.",
                    );
                }
                Err(e) => {
                    tracing::error!(
                        "Worker {} failed to merge detour {}: {}",
                        self.id,
                        worktree.branch_name,
                        e
                    );
                    post_detour_recovery_comment(
                        &self.db, &ticket.id, &worktree.branch_name, target,
                        &format!("Merge failed: {}", e),
                    );
                }
            }
        }

        // Clean up worktree
        if let Err(e) = worktree::remove_worktree(&worktree.path, &worktree.repo_path) {
            tracing::warn!(
                "Failed to remove worktree {}: {}",
                worktree.path.display(),
                e
            );
        } else {
            tracing::info!(
                "Worker {} removed worktree at {}",
                self.id,
                worktree.path.display()
            );
        }

        // Only delete the detour branch if the merge succeeded; preserve it for manual merge otherwise
        if detour_merged {
            worktree::delete_branch(&worktree.repo_path, &worktree.branch_name);
        }

        for extra_wt in &extra_worktrees {
            if let (Some(ref target), Some(ref fork_point)) =
                (&extra_wt.target_branch, &extra_wt.detour_fork_point)
            {
                match worktree::merge_detour_into_target(
                    &extra_wt.repo_path,
                    &extra_wt.branch_name,
                    target,
                    fork_point,
                ) {
                    Ok(
                        worktree::DetourMergeResult::Merged { .. }
                        | worktree::DetourMergeResult::MergedWorkingTreeDirty { .. }
                        | worktree::DetourMergeResult::MergedWorkingTreeStale { .. }
                        | worktree::DetourMergeResult::NothingToMerge,
                    ) => {
                        worktree::delete_branch(&extra_wt.repo_path, &extra_wt.branch_name);
                    }
                    Ok(worktree::DetourMergeResult::Diverged { .. }) | Err(_) => {
                        tracing::warn!(
                            "Worker {} could not merge workspace detour {} into {} for repo {}",
                            self.id,
                            extra_wt.branch_name,
                            target,
                            extra_wt.repo_path.display()
                        );
                    }
                }
            }
            if let Err(e) = worktree::remove_worktree(&extra_wt.path, &extra_wt.repo_path) {
                tracing::warn!(
                    "Failed to remove workspace worktree {}: {}",
                    extra_wt.path.display(),
                    e
                );
            }
        }
        if let Some(ref ws_file_path) = ws_file {
            let _ = std::fs::remove_file(ws_file_path);
        }

        // Update worker status
        {
            let mut status = self.status.lock().expect("status mutex poisoned");
            status.status = WorkerState::Idle;
            status.current_ticket_id = None;
            status.current_run_id = None;
            status.tickets_processed += 1;
        }

        Ok(true)
    }

    fn get_repo_path(
        &self,
        ticket: &Ticket,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(ref project_id) = ticket.project_id {
            if let Ok(Some(project)) = self.db.get_project(project_id) {
                return Ok(project.path);
            }
        }

        if let Some(ref workspace_id) = ticket.workspace_id {
            let projects = self
                .db
                .get_workspace_projects(workspace_id)
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                    format!("Failed to get workspace projects: {}", e).into()
                })?;
            if let Some(first) = projects.first() {
                return Ok(first.path.clone());
            }
            return Err("Workspace has no projects".into());
        }

        if let Some(ref project_id) = self.config.project_id {
            if let Ok(Some(project)) = self.db.get_project(project_id) {
                return Ok(project.path);
            }
        }

        Err("No project configured for ticket".into())
    }

}

/// Post a system comment on the ticket notifying the user that the detour
/// branch could not be merged automatically and providing recovery steps.
fn post_detour_recovery_comment(
    db: &Database,
    ticket_id: &str,
    detour_branch: &str,
    target_branch: &str,
    reason: &str,
) {
    let body = format!(
        "## Detour Branch Needs Manual Merge\n\n\
         {reason}\n\n\
         The agent's work is preserved on branch `{detour_branch}`.\n\n\
         ### Recovery steps\n\
         ```bash\n\
         git checkout {target_branch}\n\
         git merge {detour_branch}\n\
         # resolve any conflicts, then:\n\
         git branch -d {detour_branch}\n\
         ```"
    );

    if let Err(e) = db.create_comment(&CreateComment {
        ticket_id: ticket_id.to_string(),
        author_type: AuthorType::System,
        body_md: body,
        metadata: Some(serde_json::json!({ "type": "detour-merge-failed" })),
    }) {
        tracing::error!(
            "Failed to post detour recovery comment for ticket {}: {}",
            ticket_id, e
        );
    }
}

/// Post a system comment when the detour merged via update-ref but the user's
/// working tree wasn't updated because they have uncommitted changes.
fn post_detour_dirty_worktree_comment(db: &Database, ticket_id: &str, target_branch: &str) {
    let body = format!(
        "## Working Tree Out of Sync\n\n\
         The agent's work has been merged into `{target_branch}`, but your working tree \
         was not updated because you have uncommitted changes.\n\n\
         To see the agent's changes, run:\n\
         ```bash\n\
         git stash           # save your changes\n\
         git reset --hard HEAD\n\
         git stash pop        # re-apply your changes\n\
         ```"
    );

    if let Err(e) = db.create_comment(&CreateComment {
        ticket_id: ticket_id.to_string(),
        author_type: AuthorType::System,
        body_md: body,
        metadata: Some(serde_json::json!({ "type": "detour-working-tree-dirty" })),
    }) {
        tracing::error!(
            "Failed to post dirty-worktree comment for ticket {}: {}",
            ticket_id, e
        );
    }
}

/// Post a system comment when the detour merged via update-ref but the working
/// tree wasn't updated because merge --ff-only failed (the tree itself is clean).
fn post_detour_stale_worktree_comment(db: &Database, ticket_id: &str, target_branch: &str) {
    let body = format!(
        "## Working Tree Out of Sync\n\n\
         The agent's work has been merged into `{target_branch}`, but your working tree \
         could not be updated automatically.\n\n\
         To see the agent's changes, run:\n\
         ```bash\n\
         git reset --hard HEAD\n\
         ```"
    );

    if let Err(e) = db.create_comment(&CreateComment {
        ticket_id: ticket_id.to_string(),
        author_type: AuthorType::System,
        body_md: body,
        metadata: Some(serde_json::json!({ "type": "detour-working-tree-stale" })),
    }) {
        tracing::error!(
            "Failed to post stale-worktree comment for ticket {}: {}",
            ticket_id, e
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::claude::provider::ClaudeProvider;
    use crate::db::{CreateTicket, Priority, WorkflowType};
    use std::collections::HashMap;

    fn create_test_worker_config() -> WorkerConfig {
        WorkerConfig {
            agent_id: "claude".to_string(),
            provider: Arc::new(ClaudeProvider::new()),
            project_id: None,
            poll_interval_secs: 10,
            heartbeat_interval_secs: 30,
            lock_duration_mins: 30,
            agent_timeout_secs: 300,
            app_handle: None,
            agent_config: HashMap::new(),
            code_review_max_iterations: 3,
            stage_timeout_secs: 1800,
            stage_max_retries: 2,
            workflow_settings: None,
        }
    }

    #[test]
    fn worker_new_initializes_correctly() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let mut config = create_test_worker_config();
        config.project_id = Some("proj-1".to_string());

        let worker = Worker::new("test-worker".to_string(), config, db, None);

        assert_eq!(worker.id, "test-worker");
        assert!(!worker.is_running());

        let status = worker.get_status();
        assert_eq!(status.id, "test-worker");
        assert_eq!(status.agent_type, "claude");
        assert_eq!(status.project_id, Some("proj-1".to_string()));
        assert_eq!(status.status, WorkerState::Idle);
        assert_eq!(status.tickets_processed, 0);
        assert!(status.current_ticket_id.is_none());
        assert!(status.current_run_id.is_none());
    }

    #[test]
    fn worker_stop_sets_state() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let config = create_test_worker_config();
        let worker = Worker::new("w1".to_string(), config, db, None);

        worker.stop();

        assert!(!worker.is_running());
        assert_eq!(worker.get_status().status, WorkerState::Stopped);
    }

    #[test]
    fn worktree_setup_failed_blocked_unlocks_ticket() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let ready_col = columns.iter().find(|c| c.name == "Ready").unwrap();

        let ticket = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: ready_col.id.clone(),
                title: "Test".to_string(),
                description_md: String::new(),
                priority: Priority::Medium,
                labels: vec![],
                project_id: None,
                workspace_id: None,
                workflow_type: WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: None,
            })
            .unwrap();

        let run_id = "test-run-1";
        let lock_expires = chrono::Utc::now() + chrono::Duration::minutes(30);
        db.lock_ticket(&ticket.id, run_id, lock_expires).unwrap();

        // Simulate: worktree failed, ticket WAS moved to Blocked
        let result = worktree_setup::WorktreeSetupResult::Failed {
            message: "Branch already checked out".to_string(),
            ticket_blocked: true,
        };

        if let worktree_setup::WorktreeSetupResult::Failed { ticket_blocked, .. } = result {
            if ticket_blocked {
                db.unlock_ticket(&ticket.id).unwrap();
            }
        }

        // Ticket should be unlocked
        let updated = db.get_ticket(&ticket.id).unwrap();
        assert!(updated.locked_by_run_id.is_none());
    }

    #[test]
    fn worktree_setup_failed_not_blocked_keeps_lock() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let ready_col = columns.iter().find(|c| c.name == "Ready").unwrap();

        let ticket = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: ready_col.id.clone(),
                title: "Test".to_string(),
                description_md: String::new(),
                priority: Priority::Medium,
                labels: vec![],
                project_id: None,
                workspace_id: None,
                workflow_type: WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: None,
            })
            .unwrap();

        let run_id = "test-run-2";
        let lock_expires = chrono::Utc::now() + chrono::Duration::minutes(30);
        db.lock_ticket(&ticket.id, run_id, lock_expires).unwrap();

        // Simulate: worktree failed, ticket was NOT moved to Blocked
        let result = worktree_setup::WorktreeSetupResult::Failed {
            message: "Branch already checked out".to_string(),
            ticket_blocked: false,
        };

        if let worktree_setup::WorktreeSetupResult::Failed { ticket_blocked, .. } = result {
            if ticket_blocked {
                db.unlock_ticket(&ticket.id).unwrap();
            }
        }

        // Ticket should STILL be locked to prevent re-queuing
        let updated = db.get_ticket(&ticket.id).unwrap();
        assert_eq!(updated.locked_by_run_id.as_deref(), Some(run_id));
    }

    #[test]
    fn worktree_setup_result_failed_carries_message_and_flag() {
        let result = worktree_setup::WorktreeSetupResult::Failed {
            message: "SSH auth failed".to_string(),
            ticket_blocked: true,
        };

        match result {
            worktree_setup::WorktreeSetupResult::Failed { message, ticket_blocked } => {
                assert_eq!(message, "SSH auth failed");
                assert!(ticket_blocked);
            }
            _ => panic!("Expected Failed variant"),
        }

        let result2 = worktree_setup::WorktreeSetupResult::Failed {
            message: "No blocked column".to_string(),
            ticket_blocked: false,
        };

        match result2 {
            worktree_setup::WorktreeSetupResult::Failed { message, ticket_blocked } => {
                assert_eq!(message, "No blocked column");
                assert!(!ticket_blocked);
            }
            _ => panic!("Expected Failed variant"),
        }
    }
}
