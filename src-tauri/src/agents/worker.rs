//! Worker module for continuous, automated ticket processing.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{interval, sleep};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

use super::{AgentKind, AgentRunConfig, RunOutcome};
use super::spawner::{self, CancelHandle};
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

    pub fn get_status(&self) -> WorkerStatus {
        self.status.lock().expect("status mutex poisoned").clone()
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

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

        let Some(ticket) = self.db.reserve_next_ticket(
            self.config.project_id.as_deref(),
            self.config.agent_type,
            &run_id,
            lock_expires,
        )? else {
            return Ok(false);
        };

        tracing::info!("Worker {} reserved ticket: {}", self.id, ticket.id);

        let repo_path = match self.get_repo_path(&ticket) {
            Ok(path) => path,
            Err(e) => {
                self.db.unlock_ticket(&ticket.id)?;
                return Err(e);
            }
        };

        let project_id = ticket.project_id.as_ref()
            .or(self.config.project_id.as_ref())
            .cloned();
        
        if let Some(ref pid) = project_id {
            if !self.db.acquire_repo_lock(pid, &run_id, lock_expires)? {
                tracing::debug!(
                    "Worker {} could not acquire repo lock for project {}, skipping ticket {}",
                    self.id, pid, ticket.id
                );
                self.db.unlock_ticket(&ticket.id)?;
                return Ok(false);
            }
        }

        let run = self.db.create_run(&CreateRun {
            ticket_id: ticket.id.clone(),
            agent_type: match self.config.agent_type {
                AgentKind::Cursor => AgentType::Cursor,
                AgentKind::Claude => AgentType::Claude,
            },
            repo_path: repo_path.clone(),
        })?;

        // Re-lock with actual run ID (atomic reservation used temporary ID)
        self.db.lock_ticket(&ticket.id, &run.id, lock_expires)?;

        if let Ok(columns) = self.db.get_columns(&ticket.board_id) {
            if let Some(in_progress) = columns.iter().find(|c| c.name == "In Progress") {
                let _ = self.db.move_ticket(&ticket.id, &in_progress.id);
            }
        }

        self.db.update_run_status(&run.id, RunStatus::Running, None, None)?;

        {
            let mut status = self.status.lock().expect("status mutex poisoned");
            status.status = WorkerState::Running;
            status.current_ticket_id = Some(ticket.id.clone());
            status.current_run_id = Some(run.id.clone());
        }

        let heartbeat_handle = self.start_heartbeat(&ticket.id, &run.id, project_id.as_deref());
        let prompt = super::prompt::generate_ticket_prompt_with_workflow(&ticket, Some(self.config.agent_type));
        let result = self.run_agent(&run.id, &repo_path, &prompt).await;
        heartbeat_handle.abort();

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
            }
            Err(e) => {
                self.db.update_run_status(
                    &run.id,
                    RunStatus::Error,
                    None,
                    Some(&format!("Error: {}", e)),
                )?;

                if let Ok(columns) = self.db.get_columns(&ticket.board_id) {
                    if let Some(blocked) = columns.iter().find(|c| c.name == "Blocked") {
                        let _ = self.db.move_ticket(&ticket.id, &blocked.id);
                    }
                }

                tracing::error!("Worker {} run {} failed: {}", self.id, run.id, e);
            }
        }

        self.db.unlock_ticket(&ticket.id)?;
        if let Some(ref pid) = project_id {
            if let Err(e) = self.db.release_repo_lock(pid, &run.id) {
                tracing::warn!("Failed to release repo lock for project {}: {}", pid, e);
            }
        }

        {
            let mut status = self.status.lock().expect("status mutex poisoned");
            status.status = WorkerState::Idle;
            status.current_ticket_id = None;
            status.current_run_id = None;
            status.tickets_processed += 1;
        }

        *self.cancel_handle.lock().expect("cancel mutex poisoned") = None;

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

    fn start_heartbeat(&self, ticket_id: &str, run_id: &str, project_id: Option<&str>) -> tokio::task::JoinHandle<()> {
        let db = self.db.clone();
        let ticket_id = ticket_id.to_string();
        let run_id = run_id.to_string();
        let project_id = project_id.map(|s| s.to_string());
        let interval_secs = self.config.heartbeat_interval_secs;
        let lock_mins = self.config.lock_duration_mins;
        let running = self.running.clone();

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(interval_secs));

            while running.load(Ordering::Relaxed) {
                ticker.tick().await;

                let new_expires = chrono::Utc::now() + chrono::Duration::minutes(lock_mins);
                match db.extend_lock(&ticket_id, &run_id, new_expires) {
                    Ok(()) => {}
                    Err(e) => {
                        tracing::error!("Heartbeat failed for ticket {}: {}", ticket_id, e);
                        break;
                    }
                }
                if let Some(ref pid) = project_id {
                    if let Err(e) = db.extend_repo_lock(pid, &run_id, new_expires) {
                        tracing::warn!("Failed to extend repo lock for project {}: {}", pid, e);
                    }
                }
            }
        })
    }

    async fn run_agent(
        &self,
        run_id: &str,
        repo_path: &str,
        prompt: &str,
    ) -> Result<super::AgentRunResult, Box<dyn std::error::Error + Send + Sync>> {
        let config = AgentRunConfig {
            kind: self.config.agent_type,
            ticket_id: String::new(),
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
        let workers = self.workers.lock().expect("workers mutex poisoned");
        for worker in workers.iter() {
            if worker.id == worker_id {
                worker.stop();
                return true;
            }
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
