//! Worker module for continuous, automated ticket processing.
//!
//! Workers are automated agents that poll for tickets in the "Ready" column
//! and process them using the same execution path as manual runs.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use tauri::AppHandle;

use super::AgentKind;
use super::runner::{self, RunnerConfig};
use super::worktree;
use crate::db::{Database, AgentType, CreateRun, RunStatus, Ticket};

#[derive(Debug, Clone)]
pub struct WorkerConfig {
    pub agent_type: AgentKind,
    pub project_id: Option<String>,
    pub api_url: String,
    pub api_token: String,
    pub poll_interval_secs: u64,
    pub heartbeat_interval_secs: u64,
    pub lock_duration_mins: i64,
    pub agent_timeout_secs: u64,
    pub hook_script_path: Option<String>,
    pub app_handle: Option<AppHandle>,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            agent_type: AgentKind::Cursor,
            project_id: None,
            api_url: "http://127.0.0.1:7432".to_string(),
            api_token: "default-token".to_string(),
            poll_interval_secs: 10,
            heartbeat_interval_secs: 60,
            lock_duration_mins: 30,
            agent_timeout_secs: 3600, // 1 hour
            hook_script_path: None,
            app_handle: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerStatus {
    pub id: String,
    pub agent_type: String,
    pub project_id: Option<String>,
    pub status: WorkerState,
    pub current_ticket_id: Option<String>,
    pub current_run_id: Option<String>,
    pub tickets_processed: u32,
    pub started_at: DateTime<Utc>,
    pub last_poll_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkerState {
    Idle,
    Running,
    Stopped,
}

pub struct Worker {
    pub id: String,
    config: WorkerConfig,
    db: Arc<Database>,
    running: Arc<AtomicBool>,
    status: Arc<std::sync::Mutex<WorkerStatus>>,
    cancel_handles: runner::CancelHandlesMap,
}

impl Worker {
    pub fn new(id: String, config: WorkerConfig, db: Arc<Database>) -> Self {
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
            cancel_handles: runner::create_cancel_handles(),
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
        let lock_expires = chrono::Utc::now() + chrono::Duration::minutes(self.config.lock_duration_mins);

        // Try to reserve the next available ticket
        let Some(ticket) = self.db.reserve_next_ticket(
            self.config.project_id.as_deref(),
            self.config.agent_type,
            &run_id,
            lock_expires,
        )? else {
            return Ok(false);
        };

        tracing::info!("Worker {} reserved ticket: {}", self.id, ticket.id);

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
                self.id, ticket.id, existing_branch
            );
            
            match worktree::create_worktree_with_existing_branch(&repo_path_buf, existing_branch, &run_id, None) {
                Ok(info) => {
                    tracing::info!(
                        "Worker {} created worktree at {} using branch {}",
                        self.id, info.path.display(), info.branch_name
                    );
                    Some(info)
                }
                Err(e) => {
                    tracing::error!(
                        "Worker {} failed to create worktree for ticket {}: {}. CRITICAL: Cannot proceed without worktree.",
                        self.id, ticket.id, e
                    );
                    self.db.unlock_ticket(&ticket.id)?;
                    return Err(format!("Failed to create worktree: {}", e).into());
                }
            }
        } else {
            // First run - no branch yet
            // Create worktree with a temporary branch name
            // The orchestrator will generate an AI branch name and switch to it
            let temp_branch = format!("agent-work/{}/{}", 
                &ticket.id[..8.min(ticket.id.len())],
                &run_id[..8.min(run_id.len())]
            );
            
            tracing::info!(
                "Worker {} ticket {} has no branch yet, creating worktree with temp branch: {}",
                self.id, ticket.id, temp_branch
            );
            
            match worktree::create_worktree(&worktree::WorktreeConfig {
                repo_path: repo_path_buf.clone(),
                branch_name: temp_branch.clone(),
                run_id: run_id.clone(),
                base_dir: None,
            }) {
                Ok(info) => {
                    tracing::info!(
                        "Worker {} created worktree at {} with temp branch {}",
                        self.id, info.path.display(), info.branch_name
                    );
                    Some(info)
                }
                Err(e) => {
                    tracing::error!(
                        "Worker {} failed to create worktree for ticket {}: {}. CRITICAL: Cannot proceed without worktree.",
                        self.id, ticket.id, e
                    );
                    self.db.unlock_ticket(&ticket.id)?;
                    return Err(format!("Failed to create worktree: {}", e).into());
                }
            }
        };
        
        // Worktree is now always created - unwrap is safe here
        let worktree = worktree_info.expect("Worktree should always be created");
        let working_path = worktree.path.clone();

        // Create the run in the database
        let run = match self.db.create_run(&CreateRun {
            ticket_id: ticket.id.clone(),
            agent_type: match self.config.agent_type {
                AgentKind::Cursor => AgentType::Cursor,
                AgentKind::Claude => AgentType::Claude,
            },
            repo_path: working_path.to_string_lossy().to_string(),
            parent_run_id: None,
            stage: None,
        }) {
            Ok(run) => run,
            Err(e) => {
                let _ = self.db.unlock_ticket(&ticket.id);
                let _ = worktree::remove_worktree(&worktree.path, &worktree.repo_path);
                return Err(e.into());
            }
        };

        // Re-lock with actual run ID
        if let Err(e) = self.db.lock_ticket(&ticket.id, &run.id, lock_expires) {
            let _ = self.db.update_run_status(
                &run.id,
                RunStatus::Error,
                None,
                Some("Failed to re-lock ticket with actual run ID"),
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
        let heartbeat_handle = self.start_heartbeat(&ticket.id, &run.id);

        // Get the next pending task for this ticket
        let task = match self.db.get_next_pending_task(&ticket.id) {
            Ok(Some(t)) => {
                tracing::info!("Worker {} found pending task {} for ticket {}", self.id, t.id, ticket.id);
                Some(t)
            }
            Ok(None) => {
                tracing::warn!("Worker {} found no pending tasks for ticket {}, skipping", self.id, ticket.id);
                self.db.unlock_ticket(&ticket.id)?;
                let _ = worktree::remove_worktree(&worktree.path, &worktree.repo_path);
                return Ok(false);
            }
            Err(e) => {
                tracing::warn!("Worker {} failed to get tasks for ticket {}: {}", self.id, ticket.id, e);
                None // Fall back to legacy ticket-based workflow
            }
        };
        
        // Mark task as in progress
        if let Some(ref t) = task {
            if let Err(e) = self.db.start_task(&t.id, &run.id) {
                tracing::warn!("Failed to mark task {} as in_progress: {}", t.id, e);
            }
        }

        // Execute the agent using the shared runner
        // This gives us the exact same behavior as manual runs:
        // - Multi-stage workflow with proper stage tracking
        // - Real-time log streaming
        // - Branch comments
        // - Agent summary comments
        // - Ticket movement
        
        // Determine branch setup based on whether ticket already has a branch:
        // - First run (no branch): Use None so orchestrator generates AI branch name
        // - Subsequent runs (has branch): Use the existing branch name
        let (worktree_branch, branch_already_created) = if ticket.branch_name.is_some() {
            // Subsequent run - use existing branch (worktree attached to it)
            (ticket.branch_name.clone(), true)
        } else {
            // First run - let orchestrator generate AI branch name
            // The worktree has a temp branch, but we pass None so orchestrator generates a good name
            (None, false)
        };
        
        let runner_config = RunnerConfig {
            db: self.db.clone(),
            window: None, // Workers don't have a window
            app_handle: self.config.app_handle.clone(), // Use app_handle for global event emission
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
            timeout_secs: self.config.agent_timeout_secs,
        };

        let result = runner::execute_agent_run(runner_config).await;
        
        // Stop heartbeat
        heartbeat_handle.abort();

        // Log result and update task status
        match &result {
            Ok(r) => {
                tracing::info!(
                    "Worker {} completed run {} with status {:?} in {:.1}s",
                    self.id, run.id, r.status, r.duration_secs
                );
                
                // Mark task as completed or failed based on result
                if let Some(ref t) = task {
                    let task_result = match r.status {
                        RunStatus::Finished => self.db.complete_task(&t.id),
                        RunStatus::Error | RunStatus::Aborted => self.db.fail_task(&t.id),
                        _ => Ok(t.clone()),
                    };
                    if let Err(e) = task_result {
                        tracing::warn!("Failed to update task {} status: {}", t.id, e);
                    }
                }
            }
            Err(e) => {
                tracing::error!("Worker {} run {} failed: {}", self.id, run.id, e);
                
                // Mark task as failed
                if let Some(ref t) = task {
                    if let Err(fail_err) = self.db.fail_task(&t.id) {
                        tracing::warn!("Failed to mark task {} as failed: {}", t.id, fail_err);
                    }
                }
            }
        }

        // Unlock the ticket
        self.db.unlock_ticket(&ticket.id)?;
        
        // Clean up worktree
        if let Err(e) = worktree::remove_worktree(&worktree.path, &worktree.repo_path) {
            tracing::warn!("Failed to remove worktree {}: {}", worktree.path.display(), e);
        } else {
            tracing::info!("Worker {} removed worktree at {}", self.id, worktree.path.display());
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

    fn get_repo_path(&self, ticket: &Ticket) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
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

    fn start_heartbeat(&self, ticket_id: &str, run_id: &str) -> tokio::task::JoinHandle<()> {
        let db = self.db.clone();
        let ticket_id = ticket_id.to_string();
        let run_id = run_id.to_string();
        let interval_secs = self.config.heartbeat_interval_secs;
        let lock_mins = self.config.lock_duration_mins;
        let running = self.running.clone();

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));

            while running.load(Ordering::Relaxed) {
                ticker.tick().await;

                let new_expires = chrono::Utc::now() + chrono::Duration::minutes(lock_mins);

                if let Err(e) = db.extend_lock(&ticket_id, &run_id, new_expires) {
                    tracing::error!("Heartbeat failed for ticket {}: {}", ticket_id, e);
                    break;
                }
            }
        })
    }
}

pub struct WorkerManager {
    workers: std::sync::Mutex<Vec<Arc<Worker>>>,
    handles: std::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>,
}

impl WorkerManager {
    pub fn new() -> Self {
        Self {
            workers: std::sync::Mutex::new(Vec::new()),
            handles: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn start_worker(&self, config: WorkerConfig, db: Arc<Database>) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let worker = Arc::new(Worker::new(id.clone(), config, db));
        let worker_clone = worker.clone();

        let handle = tokio::spawn(async move {
            worker_clone.run().await;
        });

        self.workers.lock().expect("workers mutex poisoned").push(worker);
        self.handles.lock().expect("handles mutex poisoned").push(handle);

        id
    }

    pub fn stop_worker(&self, worker_id: &str) -> bool {
        let mut workers = self.workers.lock().expect("workers mutex poisoned");
        let mut handles = self.handles.lock().expect("handles mutex poisoned");
        
        let index = workers.iter().position(|w| w.id == worker_id);
        
        if let Some(idx) = index {
            workers[idx].stop();
            workers.remove(idx);
            if idx < handles.len() {
                let handle = handles.remove(idx);
                handle.abort();
            }
            return true;
        }
        false
    }

    pub async fn stop_all(&self) {
        {
            let workers = self.workers.lock().expect("workers mutex poisoned");
            for worker in workers.iter() {
                worker.stop();
            }
        }

        let handles: Vec<_> = self.handles.lock().expect("handles mutex poisoned").drain(..).collect();
        for handle in handles {
            let _ = handle.await;
        }

        self.workers.lock().expect("workers mutex poisoned").clear();
    }

    pub fn get_all_status(&self) -> Vec<WorkerStatus> {
        self.workers
            .lock()
            .expect("workers mutex poisoned")
            .iter()
            .map(|w| w.get_status())
            .collect()
    }

    pub fn worker_count(&self) -> usize {
        self.workers.lock().expect("workers mutex poisoned").len()
    }
}

impl Default for WorkerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_config_default() {
        let config = WorkerConfig::default();
        assert_eq!(config.poll_interval_secs, 10);
        assert_eq!(config.heartbeat_interval_secs, 60);
        assert_eq!(config.lock_duration_mins, 30);
        assert_eq!(config.agent_timeout_secs, 3600);
    }

    #[test]
    fn worker_state_serializes() {
        assert_eq!(serde_json::to_string(&WorkerState::Idle).unwrap(), "\"idle\"");
        assert_eq!(serde_json::to_string(&WorkerState::Running).unwrap(), "\"running\"");
        assert_eq!(serde_json::to_string(&WorkerState::Stopped).unwrap(), "\"stopped\"");
    }

    #[test]
    fn worker_manager_new_is_empty() {
        let manager = WorkerManager::new();
        assert_eq!(manager.worker_count(), 0);
        assert!(manager.get_all_status().is_empty());
    }

    #[test]
    fn worker_status_serializes() {
        let status = WorkerStatus {
            id: "w1".to_string(),
            agent_type: "cursor".to_string(),
            project_id: None,
            status: WorkerState::Idle,
            current_ticket_id: None,
            current_run_id: None,
            tickets_processed: 5,
            started_at: Utc::now(),
            last_poll_at: None,
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"ticketsProcessed\":5"));
        assert!(json.contains("\"status\":\"idle\""));
    }

    #[test]
    fn worker_state_deserializes() {
        assert_eq!(
            serde_json::from_str::<WorkerState>("\"idle\"").unwrap(),
            WorkerState::Idle
        );
        assert_eq!(
            serde_json::from_str::<WorkerState>("\"running\"").unwrap(),
            WorkerState::Running
        );
        assert_eq!(
            serde_json::from_str::<WorkerState>("\"stopped\"").unwrap(),
            WorkerState::Stopped
        );
    }

    #[test]
    fn worker_new_initializes_correctly() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let config = WorkerConfig {
            agent_type: AgentKind::Claude,
            project_id: Some("proj-1".to_string()),
            ..Default::default()
        };

        let worker = Worker::new("test-worker".to_string(), config, db);

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
        let worker = Worker::new("w1".to_string(), WorkerConfig::default(), db);

        worker.stop();

        assert!(!worker.is_running());
        assert_eq!(worker.get_status().status, WorkerState::Stopped);
    }

    #[test]
    fn worker_manager_stop_unknown_returns_false() {
        let manager = WorkerManager::new();
        assert!(!manager.stop_worker("nonexistent-id"));
    }

    #[test]
    fn worker_manager_default_is_new() {
        let manager = WorkerManager::default();
        assert_eq!(manager.worker_count(), 0);
    }

    #[test]
    fn worker_config_with_custom_values() {
        let config = WorkerConfig {
            agent_type: AgentKind::Claude,
            project_id: Some("my-project".to_string()),
            api_url: "http://localhost:8080".to_string(),
            api_token: "secret".to_string(),
            poll_interval_secs: 30,
            heartbeat_interval_secs: 120,
            lock_duration_mins: 60,
            agent_timeout_secs: 7200,
            hook_script_path: Some("/path/to/hook.js".to_string()),
            app_handle: None,
        };

        assert_eq!(config.poll_interval_secs, 30);
        assert_eq!(config.heartbeat_interval_secs, 120);
        assert_eq!(config.lock_duration_mins, 60);
        assert_eq!(config.agent_timeout_secs, 7200);
        assert_eq!(config.api_url, "http://localhost:8080");
    }

    #[test]
    fn worker_status_with_all_fields() {
        let now = Utc::now();
        let status = WorkerStatus {
            id: "w1".to_string(),
            agent_type: "cursor".to_string(),
            project_id: Some("proj".to_string()),
            status: WorkerState::Running,
            current_ticket_id: Some("t1".to_string()),
            current_run_id: Some("r1".to_string()),
            tickets_processed: 10,
            started_at: now,
            last_poll_at: Some(now),
        };

        let json = serde_json::to_string(&status).unwrap();
        let deserialized: WorkerStatus = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, "w1");
        assert_eq!(deserialized.current_ticket_id, Some("t1".to_string()));
        assert_eq!(deserialized.current_run_id, Some("r1".to_string()));
        assert_eq!(deserialized.status, WorkerState::Running);
    }
}
