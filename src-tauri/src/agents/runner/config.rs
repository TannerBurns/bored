//! Configuration types for the agent runner.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Window};

use crate::agents::provider::AgentProvider;
use crate::agents::spawner::CancelHandle;
use crate::commands::workflow_settings::WorkflowSettings;
use crate::db::models::Task;
use crate::db::{Database, RunStatus, Ticket};

pub type CancelHandlesMap = Arc<Mutex<HashMap<String, CancelHandle>>>;

pub struct RunnerConfig {
    pub db: Arc<Database>,
    pub window: Option<Window>,
    pub app_handle: Option<AppHandle>,
    pub ticket: Ticket,
    /// The task being executed. If None, falls back to legacy ticket-based workflow.
    pub task: Option<Task>,
    pub run_id: String,
    pub repo_path: PathBuf,
    /// Agent ID string (e.g. "cursor", "claude").
    pub agent_id: String,
    /// Agent provider for agent-agnostic dispatch.
    pub provider: Arc<dyn AgentProvider>,
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
    /// Agent-specific configuration map (auth tokens, API keys, etc.)
    pub agent_config: HashMap<String, serde_json::Value>,
    /// Maximum iterations for the code review loop (default: 3)
    pub code_review_max_iterations: usize,
    /// Timeout per workflow stage in seconds (default: 1800 = 30 min)
    pub stage_timeout_secs: u64,
    /// Maximum retries per stage (default: 2)
    pub stage_max_retries: u32,
    /// Stage to resume from (when resuming a paused ticket).
    /// If set, stages before this one will be skipped.
    pub resume_from_stage: Option<String>,
    /// The previous run ID (when resuming a paused ticket).
    /// Used to retrieve stage outputs from the run that was paused.
    pub previous_run_id: Option<String>,
    /// Per-stage configuration (enabled/disabled + model selection).
    /// Keys are user-facing stage names (e.g., "plan", "codeReview").
    pub stage_configs: HashMap<String, crate::commands::runs::StageConfig>,
    /// Shared workflow settings, passed through to the orchestrator.
    pub workflow_settings: Option<Arc<Mutex<WorkflowSettings>>>,
}

pub struct RunnerResult {
    pub status: RunStatus,
    pub exit_code: Option<i32>,
    pub summary: Option<String>,
    pub duration_secs: f64,
}

pub fn create_cancel_handles() -> CancelHandlesMap {
    Arc::new(Mutex::new(HashMap::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn create_cancel_handles_returns_empty_map() {
        let handles = create_cancel_handles();
        let map = handles.lock().unwrap();
        assert!(map.is_empty());
    }

    #[test]
    fn cancel_handles_map_is_thread_safe() {
        let handles = create_cancel_handles();
        let handles_clone = handles.clone();

        {
            let mut map = handles.lock().unwrap();
            let cancelled = Arc::new(AtomicBool::new(false));
            map.insert("test-run".to_string(), CancelHandle::new(cancelled));
        }

        {
            let map = handles_clone.lock().unwrap();
            assert!(map.contains_key("test-run"));
        }
    }
}
