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
use super::AgentKind;
use crate::db::{AgentRun, AgentType, CreateRun, Database, RunStatus, Ticket};

// Submodules
mod config;
mod error_handling;
mod heartbeat;
mod manager;

// Public re-exports
pub use config::{WorkerConfig, WorkerState, WorkerStatus};
pub use manager::WorkerManager;

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
            agent_type: config.agent_type.as_str().to_string(),
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
            self.config.agent_type,
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
            self.config.agent_type,
            &run_id,
            lock_expires,
        )?
        else {
            // Log diagnostics to help debug why no tickets are being picked up
            if let Ok(diag) = self.db.get_ready_ticket_diagnostics(
                self.config.project_id.as_deref(),
                self.config.agent_type,
            ) {
                if diag.total_ready > 0 {
                    tracing::debug!(
                        "Worker {} found no eligible tickets: {} total in Ready (paused={}, locked={}, epics={}, wrong_project={}, wrong_agent={}, eligible={})",
                        self.id,
                        diag.total_ready,
                        diag.paused,
                        diag.locked,
                        diag.epics,
                        diag.wrong_project,
                        diag.wrong_agent_pref,
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

        // Create a worktree for isolated execution - ALWAYS use worktrees
        // This ensures agent work never affects the user's main repo/terminal
        let repo_path_buf = std::path::PathBuf::from(&repo_path);

        let worktree_info = if let Some(ref existing_branch) = ticket.branch_name {
            // Ticket has a branch - create worktree to reuse it
            tracing::info!(
                "Worker {} found existing branch for ticket {}: {}, creating worktree",
                self.id,
                ticket.id,
                existing_branch
            );

            match worktree::create_worktree_with_existing_branch(
                &repo_path_buf,
                existing_branch,
                &run_id,
                None,
            ) {
                Ok(info) => {
                    tracing::info!(
                        "Worker {} created worktree at {} using branch {}",
                        self.id,
                        info.path.display(),
                        info.branch_name
                    );
                    Some(info)
                }
                Err(e) => {
                    tracing::error!(
                        "Worker {} failed to create worktree for ticket {}: {}. CRITICAL: Cannot proceed without worktree.",
                        self.id, ticket.id, e
                    );

                    // Handle worktree failure with diagnostics
                    error_handling::handle_worktree_failure(
                        self.db.clone(),
                        self.config.app_handle.clone(),
                        &ticket,
                        &repo_path_buf,
                        &e,
                        &self.config.api_url,
                        &self.config.api_token,
                        self.config.agent_type,
                        self.config.claude_api_config.clone(),
                        &self.id,
                    )
                    .await;
                    self.db.unlock_ticket(&ticket.id)?;
                    return Err(format!("Failed to create worktree: {}", e).into());
                }
            }
        } else {
            // First run - no branch yet
            // Create worktree with a temporary branch name
            // The orchestrator will generate an AI branch name and switch to it
            let temp_branch = format!(
                "agent-work/{}/{}",
                &ticket.id[..8.min(ticket.id.len())],
                &run_id[..8.min(run_id.len())]
            );

            // For epic child tickets, check if previous sibling has a branch
            // to implement chain branching (each child branches from previous child)
            let base_branch = self.get_base_branch_for_ticket(&ticket);

            tracing::info!(
                "Worker {} ticket {} has no branch yet, creating worktree with temp branch: {}{}",
                self.id,
                ticket.id,
                temp_branch,
                base_branch
                    .as_ref()
                    .map_or(String::new(), |b| format!(" (based on {})", b))
            );

            match worktree::create_worktree(&worktree::WorktreeConfig {
                repo_path: repo_path_buf.clone(),
                branch_name: temp_branch.clone(),
                run_id: run_id.clone(),
                base_dir: None,
                base_branch,
            }) {
                Ok(mut info) => {
                    tracing::info!(
                        "Worker {} created worktree at {} with temp branch {}",
                        self.id,
                        info.path.display(),
                        info.branch_name
                    );
                    // Mark this as a temp branch so downstream knows a branch was created
                    // but it's not the ticket's permanent branch yet
                    info.is_temp_branch = true;
                    Some(info)
                }
                Err(e) => {
                    tracing::error!(
                        "Worker {} failed to create worktree for ticket {}: {}. CRITICAL: Cannot proceed without worktree.",
                        self.id, ticket.id, e
                    );

                    // Handle worktree failure with diagnostics
                    error_handling::handle_worktree_failure(
                        self.db.clone(),
                        self.config.app_handle.clone(),
                        &ticket,
                        &repo_path_buf,
                        &e,
                        &self.config.api_url,
                        &self.config.api_token,
                        self.config.agent_type,
                        self.config.claude_api_config.clone(),
                        &self.id,
                    )
                    .await;
                    self.db.unlock_ticket(&ticket.id)?;
                    return Err(format!("Failed to create worktree: {}", e).into());
                }
            }
        };

        // Worktree is now always created - unwrap is safe here
        let worktree = worktree_info.expect("Worktree should always be created");
        let working_path = worktree.path.clone();

        // Create or resume run
        let run = self
            .create_or_resume_run(&ticket, &run_id, &working_path, &worktree)
            .await?;

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

        // Build runner config
        let (worktree_branch, branch_already_created, is_temp_branch) =
            if ticket.branch_name.is_some() {
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

        let runner_config = RunnerConfig {
            db: self.db.clone(),
            window: None,
            app_handle: self.config.app_handle.clone(),
            ticket: ticket.clone(),
            task: task.clone(),
            run_id: run.id.clone(),
            repo_path: working_path.clone(),
            agent_kind: self.config.agent_type,
            api_url: self.config.api_url.clone(),
            api_token: self.config.api_token.clone(),
            hook_script_path: self.config.hook_script_path.clone(),
            cancel_handles: self.cancel_handles.clone(),
            worktree_branch,
            branch_already_created,
            is_temp_branch,
            timeout_secs: self.config.agent_timeout_secs,
            claude_api_config: self.config.claude_api_config.clone(),
            code_review_max_iterations: self.config.code_review_max_iterations,
            stage_timeout_secs: self.config.stage_timeout_secs,
            stage_max_retries: self.config.stage_max_retries,
            resume_from_stage,
            previous_run_id,
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
        self.handle_run_result(&result, &run.id, task.as_ref()).await;

        // Unlock the ticket
        self.db.unlock_ticket(&ticket.id)?;

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

        if let Some(ref project_id) = self.config.project_id {
            if let Ok(Some(project)) = self.db.get_project(project_id) {
                return Ok(project.path);
            }
        }

        Err("No project configured for ticket".into())
    }

    fn get_base_branch_for_ticket(&self, ticket: &Ticket) -> Option<String> {
        if ticket.epic_id.is_none() {
            return None;
        }

        // For epic child tickets, check if previous sibling has a branch
        match self.db.get_previous_epic_sibling(&ticket.id) {
            Ok(Some(prev_sibling)) => {
                if let Some(ref branch) = prev_sibling.branch_name {
                    tracing::info!(
                        "Worker {} using chain branching: basing {} on previous sibling's branch {}",
                        self.id, ticket.id, branch
                    );
                    Some(branch.clone())
                } else {
                    tracing::info!(
                        "Worker {} previous sibling {} has no branch yet, using default branch",
                        self.id, prev_sibling.id
                    );
                    None
                }
            }
            Ok(None) => {
                // First child in epic - check for cross-epic dependency branching
                if let Some(ref epic_id) = ticket.epic_id {
                    match self.db.get_dependency_base_branch(epic_id) {
                        Ok(Some(ref branch)) => {
                            tracing::info!(
                                "Worker {} using cross-epic branching: basing {} on dependency epic's last child branch {}",
                                self.id, ticket.id, branch
                            );
                            Some(branch.clone())
                        }
                        Ok(None) => {
                            tracing::info!(
                                "Worker {} ticket {} is first child in epic with no dependency, using default branch",
                                self.id, ticket.id
                            );
                            None
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Worker {} failed to get dependency base branch for epic {}: {}, using default branch",
                                self.id, epic_id, e
                            );
                            None
                        }
                    }
                } else {
                    tracing::info!(
                        "Worker {} ticket {} is first child in epic, using default branch",
                        self.id,
                        ticket.id
                    );
                    None
                }
            }
            Err(e) => {
                tracing::warn!(
                    "Worker {} failed to get previous sibling for {}: {}, using default branch",
                    self.id, ticket.id, e
                );
                None
            }
        }
    }

    async fn create_or_resume_run(
        &self,
        ticket: &Ticket,
        _run_id: &str,
        working_path: &std::path::Path,
        worktree: &worktree::WorktreeInfo,
    ) -> Result<AgentRun, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(ref paused_run_id) = ticket.paused_run_id {
            // Reuse the existing paused run
            tracing::info!(
                "Worker {} resuming paused run {} for ticket {}",
                self.id,
                paused_run_id,
                ticket.id
            );

            match self.db.get_run(paused_run_id) {
                Ok(mut existing_run) => {
                    // Update the run status back to running
                    if let Err(e) = self.db.update_run_status(
                        paused_run_id,
                        RunStatus::Running,
                        None,
                        None,
                    ) {
                        tracing::error!("Failed to update paused run status to running: {}", e);
                        let _ = self.db.unlock_ticket(&ticket.id);
                        let _ = worktree::remove_worktree(&worktree.path, &worktree.repo_path);
                        return Err(e.into());
                    }

                    // Remove the old cancelled handle so the orchestrator doesn't
                    // immediately detect cancellation
                    {
                        let mut handles = self
                            .cancel_handles
                            .lock()
                            .expect("cancel handles mutex poisoned");
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
                    self.create_new_run(ticket, working_path, worktree)
                }
            }
        } else {
            self.create_new_run(ticket, working_path, worktree)
        }
    }

    fn create_new_run(
        &self,
        ticket: &Ticket,
        working_path: &std::path::Path,
        worktree: &worktree::WorktreeInfo,
    ) -> Result<AgentRun, Box<dyn std::error::Error + Send + Sync>> {
        match self.db.create_run(&CreateRun {
            ticket_id: ticket.id.clone(),
            agent_type: match self.config.agent_type {
                AgentKind::Cursor => AgentType::Cursor,
                AgentKind::Claude => AgentType::Claude,
            },
            repo_path: working_path.to_string_lossy().to_string(),
            parent_run_id: None,
            stage: None,
            ..Default::default()
        }) {
            Ok(run) => Ok(run),
            Err(e) => {
                let _ = self.db.unlock_ticket(&ticket.id);
                let _ = worktree::remove_worktree(&worktree.path, &worktree.repo_path);
                Err(e.into())
            }
        }
    }

    async fn handle_run_result(
        &self,
        result: &Result<runner::RunnerResult, String>,
        run_id: &str,
        task: Option<&crate::db::models::Task>,
    ) {
        match result {
            Ok(r) => {
                tracing::info!(
                    "Worker {} completed run {} with status {:?} in {:.1}s",
                    self.id,
                    run_id,
                    r.status,
                    r.duration_secs
                );

                if let Some(t) = task {
                    let task_result = match r.status {
                        RunStatus::Finished => self.db.complete_task(&t.id),
                        RunStatus::Error => self.db.fail_task(&t.id),
                        RunStatus::Aborted => {
                            tracing::info!(
                                "Skipping task update for aborted run - task already reset"
                            );
                            Ok(t.clone())
                        }
                        RunStatus::Paused => {
                            tracing::info!(
                                "Skipping task update for paused run - task already reset"
                            );
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
                    tracing::info!("Worker {} run {} was cancelled/paused", self.id, run_id);
                } else {
                    tracing::error!("Worker {} run {} failed: {}", self.id, run_id, e);

                    if let Some(t) = task {
                        if let Err(fail_err) = self.db.fail_task(&t.id) {
                            tracing::warn!("Failed to mark task {} as failed: {}", t.id, fail_err);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_new_initializes_correctly() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let config = WorkerConfig {
            agent_type: AgentKind::Claude,
            project_id: Some("proj-1".to_string()),
            ..Default::default()
        };

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
        let worker = Worker::new("w1".to_string(), WorkerConfig::default(), db, None);

        worker.stop();

        assert!(!worker.is_running());
        assert_eq!(worker.get_status().status, WorkerState::Stopped);
    }
}
