//! Configuration types and constants for the workflow orchestrator.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Window};

use super::CancelHandlesMap;
use crate::agents::provider::AgentProvider;
use crate::agents::{AgentKind, ClaudeApiConfig};
use crate::commands::runs::StageConfig;
use crate::commands::workflow_settings::WorkflowSettings;
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
    /// Agent provider for agent-agnostic dispatch. When set, the orchestrator
    /// delegates text extraction, cost parsing, and hook installation to the provider.
    pub provider: Option<Arc<dyn AgentProvider>>,
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
    /// Claude API configuration (auth token, api key, base url)
    pub claude_api_config: Option<ClaudeApiConfig>,
    /// Stage to resume from (when resuming a paused ticket).
    /// If set, stages before this one will be skipped.
    pub resume_from_stage: Option<String>,
    /// The previous run ID (when resuming a paused ticket).
    /// Used to retrieve stage outputs from the run that was paused.
    pub previous_run_id: Option<String>,
    /// Shared workflow settings (stage configs, timeouts, retries).
    pub workflow_settings: Arc<Mutex<WorkflowSettings>>,
    /// Fallback stage configs used when workflow_settings hasn't been synced yet.
    pub stage_configs: HashMap<String, StageConfig>,
    pub code_review_max_iterations: usize,
    pub stage_timeout_secs: u64,
    pub stage_max_retries: u32,
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
