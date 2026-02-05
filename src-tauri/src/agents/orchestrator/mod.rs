//! Multi-stage workflow orchestrator for chaining Claude CLI calls.
//!
//! This module is split into focused submodules:
//! - `config`: Configuration types and constants
//! - `code_review`: Code review parsing functions
//! - `comments`: Comment management methods
//! - `ticket`: Ticket movement and lifecycle
//! - `stages`: Stage execution and retry logic
//! - `branch`: Branch creation and management
//! - `execute`: Main workflow execution

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Window};

use super::claude as claude_hooks;
use super::cursor as cursor_hooks;
use super::spawner::CancelHandle;
use super::{AgentKind, ClaudeApiConfig};
use crate::db::models::Task;
use crate::db::{Database, Ticket};

// Submodules
mod branch;
mod code_review;
mod comments;
mod config;
mod execute;
mod stages;
#[cfg(test)]
mod tests;
mod ticket;

// Public re-exports
pub use code_review::{extract_issues_section, parse_code_review_issues};
pub use config::{OrchestratorConfig, StageEvent, MULTI_STAGE_WORKFLOW};

/// Type alias for the shared cancel handles map
pub type CancelHandlesMap = Arc<Mutex<HashMap<String, CancelHandle>>>;

/// Orchestrates a multi-stage workflow for a ticket
pub struct WorkflowOrchestrator {
    db: Arc<Database>,
    window: Option<Window>,
    app_handle: Option<AppHandle>,
    parent_run_id: String,
    ticket: Ticket,
    /// The task being executed. If None, falls back to legacy ticket-based workflow.
    task: Option<Task>,
    repo_path: PathBuf,
    agent_kind: AgentKind,
    api_url: String,
    api_token: String,
    hook_script_path: Option<String>,
    /// Shared map of cancel handles for running agents
    cancel_handles: CancelHandlesMap,
    /// Flag to indicate if the workflow has been cancelled
    cancelled: Arc<AtomicBool>,
    /// The branch name to use (if already known). If None, orchestrator will generate one.
    worktree_branch: Option<String>,
    /// Whether the branch was already created (e.g., via worktree creation).
    /// If false but worktree_branch is Some, orchestrator will create the branch.
    branch_already_created: bool,
    /// Whether the worktree branch is a temporary name that should be renamed to an AI-generated name.
    is_temp_branch: bool,
    /// Claude API configuration (auth token, api key, base url, model override)
    claude_api_config: Option<ClaudeApiConfig>,
    /// Maximum iterations for the code review loop
    code_review_max_iterations: usize,
    /// Timeout per workflow stage in seconds
    stage_timeout_secs: u64,
    /// Maximum retries per stage
    stage_max_retries: u32,
    /// Stage to resume from (when resuming a paused ticket).
    /// If set, stages before this one will be skipped.
    resume_from_stage: Option<String>,
    /// Cached stage outputs from the previous run (loaded on resume)
    previous_stage_outputs: std::collections::HashMap<String, String>,
}

impl WorkflowOrchestrator {
    pub fn new(config: OrchestratorConfig) -> Self {
        // If resuming, load stage outputs from:
        // 1. The previous run (if we created a new run that links to the old one)
        // 2. OR the current run's existing sub-runs (if we're reusing the same run ID)
        let previous_stage_outputs = if config.resume_from_stage.is_some() {
            // First, try to load from a separate previous run if specified
            let mut outputs = if let Some(ref prev_run_id) = config.previous_run_id {
                match config.db.get_completed_stage_outputs(prev_run_id) {
                    Ok(outputs) => {
                        tracing::info!(
                            "Loaded {} stage outputs from previous run {}",
                            outputs.len(),
                            prev_run_id
                        );
                        outputs
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load previous stage outputs: {}", e);
                        std::collections::HashMap::new()
                    }
                }
            } else {
                std::collections::HashMap::new()
            };

            // Also check the current run's sub-runs (for when we reuse the same run ID)
            if outputs.is_empty() {
                match config.db.get_completed_stage_outputs(&config.parent_run_id) {
                    Ok(current_outputs) => {
                        if !current_outputs.is_empty() {
                            tracing::info!(
                                "Loaded {} stage outputs from current run {} (reusing paused run)",
                                current_outputs.len(),
                                config.parent_run_id
                            );
                            outputs = current_outputs;
                        }
                    }
                    Err(e) => {
                        tracing::debug!("No stage outputs from current run: {}", e);
                    }
                }
            }

            for (stage, output) in &outputs {
                tracing::debug!("  - {}: {} chars", stage, output.len());
            }
            outputs
        } else {
            std::collections::HashMap::new()
        };

        Self {
            db: config.db,
            window: config.window,
            app_handle: config.app_handle,
            parent_run_id: config.parent_run_id,
            ticket: config.ticket,
            task: config.task,
            repo_path: config.repo_path,
            agent_kind: config.agent_kind,
            api_url: config.api_url,
            api_token: config.api_token,
            hook_script_path: config.hook_script_path,
            cancel_handles: config.cancel_handles,
            cancelled: Arc::new(AtomicBool::new(false)),
            worktree_branch: config.worktree_branch,
            branch_already_created: config.branch_already_created,
            is_temp_branch: config.is_temp_branch,
            claude_api_config: config.claude_api_config,
            code_review_max_iterations: config.code_review_max_iterations,
            stage_timeout_secs: config.stage_timeout_secs,
            stage_max_retries: config.stage_max_retries,
            resume_from_stage: config.resume_from_stage,
            previous_stage_outputs,
        }
    }

    /// Check if a stage should be skipped due to resumption.
    /// Returns true if we're resuming and haven't reached the resume stage yet.
    fn should_skip_stage(&self, stage: &str) -> bool {
        match &self.resume_from_stage {
            None => false, // Not resuming, don't skip anything
            Some(resume_stage) => {
                // Define the stage order. Stages before the resume point are skipped.
                // This includes both the main workflow stages and the code-review stages.
                // Must match the order in TicketModal.tsx handlePauseTicket()
                let stage_order = [
                    "branch-gen",
                    "branch",
                    "plan",
                    "plan-validation",
                    "implement",
                    "code-review",
                    "code-review-fix",
                    "deslop",
                    "cleanup",
                    "unit-tests",
                    "cleanup-post-tests",
                    "review-changes",
                    "cleanup-post-review",
                    "review-changes-final",
                    "add-and-commit",
                ];

                // Find the index of the resume stage
                let resume_idx = stage_order.iter().position(|&s| s == resume_stage);
                // Find the index of the current stage
                let current_idx = stage_order.iter().position(|&s| s == stage);

                match (resume_idx, current_idx) {
                    (Some(resume), Some(current)) => current < resume,
                    _ => {
                        // Unknown stage, don't skip to be safe
                        tracing::warn!(
                            "Unknown stage for resumption check: stage={}, resume_from={}",
                            stage,
                            resume_stage
                        );
                        false
                    }
                }
            }
        }
    }

    /// Emit an event to the frontend, using window if available, otherwise app_handle
    fn emit_event<S: serde::Serialize + Clone>(
        &self,
        event_name: &str,
        payload: &S,
    ) -> Result<(), String> {
        if let Some(ref window) = self.window {
            window
                .emit(event_name, payload)
                .map_err(|e| format!("Failed to emit {} via window: {}", event_name, e))
        } else if let Some(ref app_handle) = self.app_handle {
            app_handle
                .emit(event_name, payload)
                .map_err(|e| format!("Failed to emit {} via app_handle: {}", event_name, e))
        } else {
            // No window or app_handle, just log and continue
            tracing::debug!("No window or app_handle available to emit {}", event_name);
            Ok(())
        }
    }

    /// Check if the workflow has been cancelled
    ///
    /// This checks both the orchestrator's own cancelled flag AND the cancel handle
    /// registered in the shared map. The latter is important for detecting cancellations
    /// that happened between stages (after one stage finished but before the next started).
    fn is_cancelled(&self) -> bool {
        // Check our own flag first (set when a stage returns Cancelled)
        if self.cancelled.load(Ordering::Relaxed) {
            return true;
        }

        // Also check the cancel handle in the shared map
        // This catches cancellations that happened between stages
        if let Ok(handles) = self.cancel_handles.lock() {
            if let Some(handle) = handles.get(&self.parent_run_id) {
                if handle.is_cancelled() {
                    return true;
                }
            }
        }

        false
    }

    /// Update project hooks with run configuration
    /// Uses the PARENT run_id so all events are associated with the main workflow run
    fn update_hooks_for_run(&self) -> Result<(), String> {
        let hook_script_path = match &self.hook_script_path {
            Some(p) => p,
            None => {
                tracing::warn!("No hook script path configured, skipping hook update");
                return Ok(());
            }
        };

        tracing::debug!(
            "Updating hooks for parent run {} with token (first 8 chars): {}...",
            self.parent_run_id,
            &self.api_token.chars().take(8).collect::<String>()
        );

        match self.agent_kind {
            AgentKind::Cursor => cursor_hooks::install_hooks_with_run_id(
                &self.repo_path,
                hook_script_path,
                Some(&self.api_url),
                Some(&self.api_token),
                Some(&self.parent_run_id),
            )
            .map_err(|e| format!("Failed to update Cursor hooks: {}", e)),
            AgentKind::Claude => claude_hooks::install_local_hooks_with_run_id(
                &self.repo_path,
                hook_script_path,
                Some(&self.api_url),
                Some(&self.api_token),
                Some(&self.parent_run_id),
            )
            .map_err(|e| format!("Failed to update Claude hooks: {}", e)),
        }
    }
}
