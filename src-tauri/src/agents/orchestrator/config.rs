//! Configuration types and constants for the workflow orchestrator.

use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Window};

use super::CancelHandlesMap;
use crate::agents::{AgentKind, ClaudeApiConfig};
use crate::db::Database;

/// Configuration for creating a WorkflowOrchestrator
pub struct OrchestratorConfig {
    pub db: Arc<Database>,
    pub window: Option<Window>,
    pub app_handle: Option<AppHandle>,
    pub parent_run_id: String,
    pub ticket: crate::db::Ticket,
    /// The task being executed. If None, falls back to legacy ticket-based workflow.
    pub task: Option<crate::db::models::Task>,
    pub repo_path: PathBuf,
    pub agent_kind: AgentKind,
    pub api_url: String,
    pub api_token: String,
    pub hook_script_path: Option<String>,
    pub cancel_handles: CancelHandlesMap,
    /// The branch name to use (if already known). If None, orchestrator will generate one.
    pub worktree_branch: Option<String>,
    /// Whether the branch was already created (e.g., via worktree creation).
    /// If false but worktree_branch is Some, orchestrator will create the branch.
    pub branch_already_created: bool,
    /// Whether the worktree branch is a temporary name that should be renamed to an AI-generated name.
    pub is_temp_branch: bool,
    /// Claude API configuration (auth token, api key, base url, model override)
    pub claude_api_config: Option<ClaudeApiConfig>,
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
}

/// The stages in a multi-stage workflow.
/// The code-review loop runs dynamically after implement (not listed here).
pub const MULTI_STAGE_WORKFLOW: &[&str] = &[
    "branch",
    "plan",
    "implement",
    "deslop",
    "cleanup",
    "unit-tests",
    "cleanup-post-tests",
    "review-changes",
    "cleanup-post-review",
    "review-changes-final",
    "add-and-commit",
];

/// Event payload for stage updates
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StageEvent {
    pub parent_run_id: String,
    pub stage: String,
    pub status: String,
    pub sub_run_id: Option<String>,
    pub duration_secs: Option<f64>,
}
