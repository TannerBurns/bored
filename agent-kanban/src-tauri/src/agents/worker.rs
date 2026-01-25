//! Worker module for continuous, automated ticket processing
//!
//! Workers are background processes that:
//! 1. Poll for Ready tickets in the queue
//! 2. Reserve and lock tickets before processing
//! 3. Spawn agents to work on tickets
//! 4. Send heartbeats to maintain locks during execution
//! 5. Finalize runs and release locks on completion

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{interval, sleep};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

use super::{AgentKind, AgentRunConfig, RunOutcome};
use super::spawner::{self, CancelHandle};
use crate::db::{Database, AgentType, CreateRun, RunStatus, Ticket};

/// Configuration for a worker
#[derive(Debug, Clone)]
pub struct WorkerConfig {
    /// Which agent type to use
    pub agent_type: AgentKind,
    /// Optional repo path filter (only process tickets for this repo)
    pub project_id: Option<String>,
    /// API URL for hooks to call back
    pub api_url: String,
    /// API authentication token
    pub api_token: String,
    /// How often to poll for new tickets (seconds)
    pub poll_interval_secs: u64,
    /// How often to send heartbeats (seconds)
    pub heartbeat_interval_secs: u64,
    /// Lock duration to request (minutes)
    pub lock_duration_mins: i64,
    /// Timeout for agent execution (seconds)
    pub agent_timeout_secs: u64,
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
        }
    }
}

/// Status of a worker
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

/// A worker that continuously processes tickets
pub struct Worker {
    pub id: String,
    config: WorkerConfig,
    db: Arc<Database>,
    running: Arc<AtomicBool>,
    status: Arc<std::sync::Mutex<WorkerStatus>>,
    cancel_handle: Arc<std::sync::Mutex<Option<CancelHandle>>>,
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
            cancel_handle: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Get current worker status
    pub fn get_status(&self) -> WorkerStatus {
        self.status.lock().expect("status mutex poisoned").clone()
    }

    /// Check if the worker is currently running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Stop the worker
    pub fn stop(&self) {
        tracing::info!("Stopping worker {}", self.id);
        self.running.store(false, Ordering::Relaxed);

        // Cancel any running agent
        if let Some(handle) = self.cancel_handle.lock().expect("cancel mutex poisoned").take() {
            handle.cancel();
        }

        let mut status = self.status.lock().expect("status mutex poisoned");
        status.status = WorkerState::Stopped;
    }

    /// Start the worker loop
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
                Ok(true) => {
                    // Successfully processed a ticket, check for more immediately
                    tracing::debug!("Worker {} processed ticket, checking for more", self.id);
                }
                Ok(false) => {
                    // No tickets available, wait before polling again
                    sleep(Duration::from_secs(self.config.poll_interval_secs)).await;
                }
                Err(e) => {
                    tracing::error!("Worker {} error: {}", self.id, e);
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }

        tracing::info!("Worker {} stopped", self.id);
    }

    /// Process the next available ticket
    async fn process_next(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        // Update last poll time
        {
            let mut status = self.status.lock().expect("status mutex poisoned");
            status.last_poll_at = Some(Utc::now());
        }

        // Find next available ticket
        let ticket = self.find_next_ticket()?;

        let Some(ticket) = ticket else {
            return Ok(false);
        };

        tracing::info!(
            "Worker {} processing ticket: {} - {}",
            self.id,
            ticket.id,
            ticket.title
        );

        // Get project path for the ticket
        let repo_path = self.get_repo_path(&ticket)?;

        // Create run
        let run = self.db.create_run(&CreateRun {
            ticket_id: ticket.id.clone(),
            agent_type: match self.config.agent_type {
                AgentKind::Cursor => AgentType::Cursor,
                AgentKind::Claude => AgentType::Claude,
            },
            repo_path: repo_path.clone(),
        })?;

        // Reserve the ticket
        let lock_expires = chrono::Utc::now()
            + chrono::Duration::minutes(self.config.lock_duration_mins);
        self.db.lock_ticket(&ticket.id, &run.id, lock_expires)?;

        // Move to In Progress column if available
        if let Ok(columns) = self.db.get_columns(&ticket.board_id) {
            if let Some(in_progress) = columns.iter().find(|c| c.name == "In Progress") {
                let _ = self.db.move_ticket(&ticket.id, &in_progress.id);
            }
        }

        // Update status to running
        self.db.update_run_status(&run.id, RunStatus::Running, None, None)?;

        {
            let mut status = self.status.lock().expect("status mutex poisoned");
            status.status = WorkerState::Running;
            status.current_ticket_id = Some(ticket.id.clone());
            status.current_run_id = Some(run.id.clone());
        }

        // Start heartbeat task
        let heartbeat_handle = self.start_heartbeat(&ticket.id, &run.id);

        // Generate prompt and run the agent
        let prompt = super::prompt::generate_ticket_prompt(&ticket);
        let result = self.run_agent(&run.id, &repo_path, &prompt).await;

        // Stop heartbeat
        heartbeat_handle.abort();

        // Finalize run
        match result {
            Ok(agent_result) => {
                let status = match agent_result.status {
                    RunOutcome::Success => RunStatus::Finished,
                    RunOutcome::Error => RunStatus::Error,
                    RunOutcome::Timeout => RunStatus::Error,
                    RunOutcome::Cancelled => RunStatus::Aborted,
                };

                self.db.update_run_status(
                    &run.id,
                    status.clone(),
                    agent_result.exit_code,
                    agent_result.summary.as_deref(),
                )?;

                // Move ticket to appropriate column based on outcome
                if let Ok(columns) = self.db.get_columns(&ticket.board_id) {
                    let target_column = match status {
                        RunStatus::Finished => columns.iter().find(|c| c.name == "Review"),
                        RunStatus::Error | RunStatus::Aborted => {
                            columns.iter().find(|c| c.name == "Blocked")
                        }
                        _ => None,
                    };

                    if let Some(col) = target_column {
                        let _ = self.db.move_ticket(&ticket.id, &col.id);
                    }
                }

                tracing::info!(
                    "Worker {} completed run {} with status {:?}",
                    self.id,
                    run.id,
                    agent_result.status
                );
            }
            Err(e) => {
                self.db.update_run_status(
                    &run.id,
                    RunStatus::Error,
                    None,
                    Some(&format!("Error: {}", e)),
                )?;

                // Move ticket to Blocked
                if let Ok(columns) = self.db.get_columns(&ticket.board_id) {
                    if let Some(blocked) = columns.iter().find(|c| c.name == "Blocked") {
                        let _ = self.db.move_ticket(&ticket.id, &blocked.id);
                    }
                }

                tracing::error!("Worker {} run {} failed: {}", self.id, run.id, e);
            }
        }

        // Release lock
        self.db.unlock_ticket(&ticket.id)?;

        // Update status
        {
            let mut status = self.status.lock().expect("status mutex poisoned");
            status.status = WorkerState::Idle;
            status.current_ticket_id = None;
            status.current_run_id = None;
            status.tickets_processed += 1;
        }

        // Clear cancel handle
        *self.cancel_handle.lock().expect("cancel mutex poisoned") = None;

        Ok(true)
    }

    /// Find the next available ticket to process
    fn find_next_ticket(&self) -> Result<Option<Ticket>, crate::db::DbError> {
        let boards = self.db.get_boards()?;

        for board in boards {
            let columns = self.db.get_columns(&board.id)?;

            let ready_column = match columns.iter().find(|c| c.name == "Ready") {
                Some(c) => c,
                None => continue,
            };

            let tickets = self.db.get_tickets(&board.id, Some(&ready_column.id))?;

            for ticket in tickets {
                // Skip locked tickets
                if let Some(ref lock_expires) = ticket.lock_expires_at {
                    if *lock_expires > Utc::now() {
                        continue;
                    }
                }

                // Filter by project if configured
                if let Some(ref filter_project_id) = self.config.project_id {
                    if ticket.project_id.as_ref() != Some(filter_project_id) {
                        continue;
                    }
                }

                // Check agent preference
                if let Some(ref pref) = ticket.agent_pref {
                    use crate::db::AgentPref;
                    match (pref, self.config.agent_type) {
                        (AgentPref::Cursor, AgentKind::Claude) => continue,
                        (AgentPref::Claude, AgentKind::Cursor) => continue,
                        _ => {}
                    }
                }

                return Ok(Some(ticket));
            }
        }

        Ok(None)
    }

    /// Get the repo path for a ticket
    fn get_repo_path(&self, ticket: &Ticket) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Try to get from ticket's project
        if let Some(ref project_id) = ticket.project_id {
            if let Ok(Some(project)) = self.db.get_project(project_id) {
                return Ok(project.path);
            }
        }

        // Try worker's configured project
        if let Some(ref project_id) = self.config.project_id {
            if let Ok(Some(project)) = self.db.get_project(project_id) {
                return Ok(project.path);
            }
        }

        Err("No project configured for ticket".into())
    }

    /// Start a background heartbeat task
    fn start_heartbeat(&self, ticket_id: &str, run_id: &str) -> tokio::task::JoinHandle<()> {
        let db = self.db.clone();
        let ticket_id = ticket_id.to_string();
        let run_id = run_id.to_string();
        let interval_secs = self.config.heartbeat_interval_secs;
        let lock_mins = self.config.lock_duration_mins;
        let running = self.running.clone();

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(interval_secs));

            while running.load(Ordering::Relaxed) {
                ticker.tick().await;

                let new_expires = chrono::Utc::now() + chrono::Duration::minutes(lock_mins);
                match db.extend_lock(&ticket_id, &run_id, new_expires) {
                    Ok(()) => {
                        tracing::debug!(
                            "Heartbeat: lock on ticket {} extended to {}",
                            ticket_id,
                            new_expires
                        );
                    }
                    Err(e) => {
                        tracing::error!("Heartbeat failed for ticket {}: {}", ticket_id, e);
                        break;
                    }
                }
            }
        })
    }

    /// Run the agent for the given configuration
    async fn run_agent(
        &self,
        run_id: &str,
        repo_path: &str,
        prompt: &str,
    ) -> Result<super::AgentRunResult, Box<dyn std::error::Error + Send + Sync>> {
        let config = AgentRunConfig {
            kind: self.config.agent_type,
            ticket_id: String::new(), // Not used directly
            run_id: run_id.to_string(),
            repo_path: std::path::PathBuf::from(repo_path),
            prompt: prompt.to_string(),
            timeout_secs: Some(self.config.agent_timeout_secs),
            api_url: self.config.api_url.clone(),
            api_token: self.config.api_token.clone(),
        };

        let cancel_handle_storage = self.cancel_handle.clone();
        let on_spawn: spawner::OnSpawnCallback = Box::new(move |handle| {
            *cancel_handle_storage.lock().expect("cancel mutex poisoned") = Some(handle);
        });

        let result = tokio::task::spawn_blocking(move || {
            spawner::run_agent_with_cancel_callback(config, None, Some(on_spawn))
        })
        .await??;

        Ok(result)
    }
}

/// Manager for multiple workers
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

    /// Start a new worker
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

    /// Stop a specific worker
    pub fn stop_worker(&self, worker_id: &str) -> bool {
        let workers = self.workers.lock().expect("workers mutex poisoned");
        for worker in workers.iter() {
            if worker.id == worker_id {
                worker.stop();
                return true;
            }
        }
        false
    }

    /// Stop all workers
    pub async fn stop_all(&self) {
        {
            let workers = self.workers.lock().expect("workers mutex poisoned");
            for worker in workers.iter() {
                worker.stop();
            }
        }

        // Wait for all handles to complete
        let handles: Vec<_> = self.handles.lock().expect("handles mutex poisoned").drain(..).collect();
        for handle in handles {
            let _ = handle.await;
        }

        self.workers.lock().expect("workers mutex poisoned").clear();
    }

    /// Get status of all workers
    pub fn get_all_status(&self) -> Vec<WorkerStatus> {
        self.workers
            .lock()
            .expect("workers mutex poisoned")
            .iter()
            .map(|w| w.get_status())
            .collect()
    }

    /// Get worker count
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
}
