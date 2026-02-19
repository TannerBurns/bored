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

/// Default ordering of optional stages (using frontend stage keys).
/// The orchestrator expands these into backend execution stage names.
pub const DEFAULT_OPTIONAL_STAGE_ORDER: &[&str] = &[
    "codeReview",
    "cleanup",
    "unitTests",
    "finalReview",
    "deslop",
];

/// Expand a frontend stage key into its backend execution stage names.
pub fn expand_stage_key(key: &str) -> &'static [&'static str] {
    match key {
        "codeReview" => &["code-review"],
        "cleanup" => &["cleanup"],
        "unitTests" => &["unit-tests", "cleanup-post-tests"],
        "finalReview" => &["review-changes", "cleanup-post-review", "review-changes-final"],
        "deslop" => &["deslop"],
        "commit" => &["add-and-commit"],
        _ => &[],
    }
}

/// Build the full execution-order list of backend stage names from a frontend stage order.
/// The input may contain all 9 frontend keys (including required ones like branchGen, plan,
/// implement, commit); required keys are filtered out since they occupy fixed positions.
pub fn build_full_stage_order(optional_order: &[String]) -> Vec<&'static str> {
    let mut order: Vec<&'static str> = vec![
        "branch-gen", "branch", "plan", "plan-validation", "implement",
    ];
    for key in optional_order {
        if matches!(key.as_str(), "branchGen" | "plan" | "implement" | "commit") {
            continue;
        }
        if key == "codeReview" {
            order.push("code-review");
            order.push("code-review-fix");
        } else {
            order.extend_from_slice(expand_stage_key(key));
        }
    }
    order.push("add-and-commit");
    order
}

/// The stages in a multi-stage workflow (default order for backward compat).
/// The code-review loop runs dynamically after implement (not listed here).
pub const MULTI_STAGE_WORKFLOW: &[&str] = &[
    "branch",
    "plan",
    "implement",
    "cleanup",
    "unit-tests",
    "cleanup-post-tests",
    "review-changes",
    "cleanup-post-review",
    "review-changes-final",
    "deslop",
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
