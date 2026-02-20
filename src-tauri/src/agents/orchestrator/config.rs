//! Configuration types and constants for the workflow orchestrator.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Window};

use super::CancelHandlesMap;
use crate::agents::provider::AgentProvider;
use crate::commands::runs::StageConfig;
use crate::commands::workflow_settings::PerAgentSettings;
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
    /// Agent ID string (e.g. "cursor", "claude").
    pub agent_id: String,
    /// Agent provider for agent-agnostic dispatch.
    pub provider: Arc<dyn AgentProvider>,
    pub api_url: String,
    pub api_token: String,
    pub cancel_handles: CancelHandlesMap,
    /// The branch name to use (if already known). If None, orchestrator will generate one.
    pub worktree_branch: Option<String>,
    /// Whether the branch was already created (e.g., via worktree creation).
    /// If false but worktree_branch is Some, orchestrator will create the branch.
    pub branch_already_created: bool,
    /// Whether the worktree branch is a temporary name that should be renamed to an AI-generated name.
    pub is_temp_branch: bool,
    /// Agent-specific configuration map (auth tokens, API keys, etc.)
    pub agent_config: HashMap<String, serde_json::Value>,
    /// Stage to resume from (when resuming a paused ticket).
    /// If set, stages before this one will be skipped.
    pub resume_from_stage: Option<String>,
    /// The previous run ID (when resuming a paused ticket).
    /// Used to retrieve stage outputs from the run that was paused.
    pub previous_run_id: Option<String>,
    /// Shared per-agent workflow settings (stage configs, timeouts, retries).
    pub workflow_settings: Arc<Mutex<PerAgentSettings>>,
    /// Fallback stage configs used when workflow_settings hasn't been synced yet.
    pub stage_configs: HashMap<String, StageConfig>,
    pub code_review_max_iterations: usize,
    pub stage_timeout_secs: u64,
    pub stage_max_retries: u32,
}

/// Expand a frontend stage key into its backend execution stage names.
/// Required stages and code-review have special mappings; all other commands
/// map 1:1 (the command ID is the backend stage name).
pub fn expand_stage_key(key: &str) -> Vec<String> {
    match key {
        "branchGen" => vec!["branch-gen".to_string(), "branch".to_string()],
        "plan" => vec!["plan".to_string(), "plan-validation".to_string()],
        "implement" => vec!["implement".to_string()],
        "code-review" => vec!["code-review".to_string(), "code-review-fix".to_string()],
        "commit" => vec!["add-and-commit".to_string()],
        other => vec![other.to_string()],
    }
}

/// Build the full execution-order list of backend stage names from a frontend stage order.
/// The frontend sends the complete stage order (required + commands).
pub fn build_full_stage_order(stage_order: &[String]) -> Vec<String> {
    let mut order: Vec<String> = Vec::new();
    for key in stage_order {
        order.extend(expand_stage_key(key));
    }
    order
}

/// The stages in a multi-stage workflow (default order for backward compat).
pub const MULTI_STAGE_WORKFLOW: &[&str] = &[
    "branch",
    "plan",
    "implement",
    "code-review",
    "code-review-fix",
    "cleanup",
    "unit-tests",
    "review-changes",
    "deslop",
    "add-and-commit",
];

/// Default stage order using the new catalog-based keys.
pub const DEFAULT_STAGE_ORDER: &[&str] = &[
    "branchGen", "plan", "implement",
    "code-review", "cleanup", "unit-tests", "review-changes", "deslop",
    "commit",
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
