//! Workflow orchestrator for chaining agent CLI calls.
//!
//! Supports two modes:
//! - **Multi-stage**: static pipeline configured by the user in settings
//! - **Auto-pilot**: agent dynamically decides which commands to run after implementation
//!
//! This module is split into focused submodules:
//! - `config`: Configuration types, constants, and `WorkflowMode` enum
//! - `auto_pilot`: Command-selection prompt and response parsing for auto-pilot mode
//! - `code_review`: Code review parsing functions
//! - `comments`: Comment management methods
//! - `ticket`: Ticket movement and lifecycle
//! - `stages`: Stage execution and retry logic
//! - `branch`: Branch creation and management
//! - `execute`: Main workflow execution and mode dispatch

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use tauri::{AppHandle, Emitter, Window};

use super::provider::AgentProvider;
use super::spawner::CancelHandle;
use crate::commands::runs::StageConfig;
use crate::db::models::Task;
use crate::db::{Database, Ticket};

// Submodules
mod auto_pilot;
mod branch;
mod clarification;
mod code_review;
mod comments;
mod config;
mod execute;
mod impl_todos;
mod stages;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod integration_tests;
mod ticket;

// Public re-exports
pub use code_review::{extract_issues_section, parse_code_review_issues, parse_structured_review};
pub use config::{
    CodeReviewIterationEvent, ImplementationProgress, ImplementationTodo, OrchestratorConfig,
    StageEvent, TodoItemStatus, TodoStatus, WorkflowMode,
};

/// Type alias for the shared cancel handles map
pub type CancelHandlesMap = Arc<Mutex<HashMap<String, CancelHandle>>>;

/// Abstracts the agent spawn call so tests can inject a mock.
pub(super) trait StageRunner: Send + Sync {
    fn run(
        &self,
        provider: &dyn AgentProvider,
        config: &super::AgentRunConfig,
        on_log: Option<std::sync::Arc<super::LogCallback>>,
        on_spawn: Option<super::spawner::OnSpawnCallback>,
    ) -> Result<super::AgentRunResult, super::spawner::SpawnError>;
}

struct DefaultStageRunner;

impl StageRunner for DefaultStageRunner {
    fn run(
        &self,
        provider: &dyn AgentProvider,
        config: &super::AgentRunConfig,
        on_log: Option<std::sync::Arc<super::LogCallback>>,
        on_spawn: Option<super::spawner::OnSpawnCallback>,
    ) -> Result<super::AgentRunResult, super::spawner::SpawnError> {
        super::spawner::run_agent_via_provider_with_cancel(provider, config, on_log, on_spawn)
    }
}

/// Orchestrates a workflow for a ticket (either static multi-stage or agent-driven auto-pilot).
pub struct WorkflowOrchestrator {
    db: Arc<Database>,
    window: Option<Window>,
    app_handle: Option<AppHandle>,
    parent_run_id: String,
    ticket: Ticket,
    /// The task being executed. If None, falls back to legacy ticket-based workflow.
    /// Wrapped in `RwLock` so auto-clarification can refresh the content after
    /// an `UpdateTask` resolution without requiring `&mut self`.
    task: RwLock<Option<Task>>,
    repo_path: PathBuf,
    workspace_file: Option<PathBuf>,
    workspace_paths: Vec<PathBuf>,
    /// Agent ID string (e.g. "cursor", "claude").
    agent_id: String,
    /// Agent provider for agent-agnostic dispatch (text extraction, cost).
    provider: Arc<dyn AgentProvider>,
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
    /// When the agent is working on a detour branch (because the ticket's branch is already
    /// checked out by the user), this holds the original branch name to merge back into.
    target_branch: Option<String>,
    /// Agent-specific configuration map (auth tokens, API keys, etc.)
    agent_config: HashMap<String, serde_json::Value>,
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
    /// Per-stage configuration (enabled/disabled + model selection)
    stage_configs: std::collections::HashMap<String, StageConfig>,
    /// Full stage ordering (frontend keys like "code-review", "cleanup", etc.)
    stage_order: Vec<String>,
    /// Full execution order (backend stage names), built once from `stage_order` for resume logic.
    full_execution_order: Vec<String>,
    workflow_mode: config::WorkflowMode,
    auto_pilot_model: String,
    auto_pilot_enabled_models: Vec<String>,
    auto_pilot_required_commands: Vec<crate::commands::workflow_settings::AutoPilotRequiredCommand>,
    auto_complete_tickets: bool,
    auto_clarification: bool,
    auto_code_review_on_complete: bool,
    debug_mode: bool,
    stage_runner: Arc<dyn StageRunner>,
    /// In-memory storage for implementation todos (populated by plan decomposition)
    implementation_todos: RwLock<Vec<config::ImplementationTodo>>,
    /// Session ID threaded across all workflow stages for conversational continuity.
    workflow_session_id: RwLock<Option<String>>,
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

        let per_agent = config
            .workflow_settings
            .lock()
            .expect("workflow settings mutex poisoned");

        let agent_ws = per_agent.get(&config.agent_id);
        let (mut stage_configs, mut code_review_max_iterations, mut stage_timeout_secs, mut stage_max_retries, mut stage_order, auto_pilot_enabled, auto_pilot_model, auto_pilot_enabled_models, auto_pilot_required_commands, auto_complete_tickets, auto_clarification, auto_code_review_on_complete, debug_mode) =
            if let Some(ws) = agent_ws.filter(|ws| ws.synced) {
                let order = ws.stage_order.clone().unwrap_or_else(|| {
                    config::DEFAULT_STAGE_ORDER.iter().map(|s| s.to_string()).collect()
                });
                (
                    ws.stage_configs.clone(),
                    ws.code_review_max_iterations,
                    ws.stage_timeout_hours as u64 * 3600,
                    ws.stage_max_retries,
                    order,
                    ws.auto_pilot_enabled,
                    ws.auto_pilot_model.clone(),
                    ws.auto_pilot_enabled_models.clone(),
                    ws.auto_pilot_required_commands.clone(),
                    ws.auto_complete_tickets,
                    ws.auto_clarification,
                    ws.auto_code_review_on_complete,
                    ws.debug_mode,
                )
            } else {
                tracing::warn!("WorkflowSettings not yet synced for agent '{}', using config fallback", config.agent_id);
                (
                    config.stage_configs,
                    config.code_review_max_iterations,
                    config.stage_timeout_secs,
                    config.stage_max_retries,
                    config::DEFAULT_STAGE_ORDER.iter().map(|s| s.to_string()).collect(),
                    false,
                    crate::agents::models::DEFAULT_STAGE_MODEL.to_string(),
                    Vec::new(),
                    Vec::new(),
                    false,
                    false,
                    config.auto_code_review_on_complete,
                    config.debug_mode,
                )
            };

        // Extract code review agent settings while per_agent guard is still alive
        let cr_agent_settings = agent_ws.filter(|ws| ws.synced).map(|ws| {
            (
                ws.code_review_agent_max_iterations,
                ws.code_review_agent_timeout_minutes,
                ws.code_review_agent_max_retries,
                ws.code_review_agent_model.clone(),
            )
        });

        let is_code_review_task = config
            .task
            .as_ref()
            .is_some_and(|t| matches!(t.task_type, crate::db::models::TaskType::CodeReview));

        let workflow_mode = match config.workflow_mode_override.as_deref() {
            Some("code_review_only") => config::WorkflowMode::CodeReviewOnly,
            _ if is_code_review_task => config::WorkflowMode::CodeReviewOnly,
            _ if auto_pilot_enabled => config::WorkflowMode::AutoPilot,
            _ => config::WorkflowMode::MultiStage,
        };

        drop(per_agent);

        if workflow_mode == config::WorkflowMode::CodeReviewOnly {
            stage_order = vec!["code-review".to_string(), "commit".to_string()];
            if let Some((cr_max, cr_timeout, cr_retries, cr_model)) = cr_agent_settings {
                code_review_max_iterations = if cr_max == 0 { usize::MAX } else { cr_max };
                stage_timeout_secs = cr_timeout as u64 * 60;
                stage_max_retries = cr_retries;
                stage_configs.insert("code-review".to_string(), StageConfig {
                    enabled: true,
                    model: cr_model,
                });
            } else {
                code_review_max_iterations = usize::MAX;
            }
        }

        let full_execution_order = config::build_full_stage_order(&stage_order);

        let resume_from_stage = config.resume_from_stage.map(|s| {
            match config::normalize_legacy_stage_name(&s) {
                Some(normalized) => {
                    tracing::warn!(
                        "Normalized legacy resume stage '{}' → '{}' (pre-catalog-refactor migration)",
                        s, normalized
                    );
                    normalized.to_string()
                }
                None => s,
            }
        });

        tracing::info!(
            "WorkflowOrchestrator mode: {:?} for agent '{}'",
            workflow_mode, config.agent_id,
        );

        let mode_str = match workflow_mode {
            config::WorkflowMode::AutoPilot => "auto_pilot",
            config::WorkflowMode::MultiStage => "multi_stage",
            config::WorkflowMode::CodeReviewOnly => "code_review_only",
        };
        if let Err(e) = config.db.set_run_metadata(
            &config.parent_run_id,
            &serde_json::json!({ "workflow_mode": mode_str }),
        ) {
            tracing::warn!("Failed to set workflow_mode metadata on parent run: {}", e);
        }

        Self {
            db: config.db,
            window: config.window,
            app_handle: config.app_handle,
            parent_run_id: config.parent_run_id,
            ticket: config.ticket,
            task: RwLock::new(config.task),
            repo_path: config.repo_path,
            workspace_file: config.workspace_file,
            workspace_paths: config.workspace_paths,
            agent_id: config.agent_id,
            provider: config.provider,
            cancel_handles: config.cancel_handles,
            cancelled: Arc::new(AtomicBool::new(false)),
            worktree_branch: config.worktree_branch,
            branch_already_created: config.branch_already_created,
            is_temp_branch: config.is_temp_branch,
            target_branch: config.target_branch,
            agent_config: config.agent_config,
            code_review_max_iterations,
            stage_timeout_secs,
            stage_max_retries,
            resume_from_stage,
            previous_stage_outputs,
            stage_configs,
            stage_order,
            full_execution_order,
            workflow_mode,
            auto_pilot_model,
            auto_pilot_enabled_models,
            auto_pilot_required_commands,
            auto_complete_tickets,
            auto_clarification,
            auto_code_review_on_complete,
            debug_mode,
            stage_runner: Arc::new(DefaultStageRunner),
            implementation_todos: RwLock::new(Vec::new()),
            workflow_session_id: RwLock::new(None),
        }
    }

    #[cfg(test)]
    pub(super) fn set_stage_runner(&mut self, runner: Arc<dyn StageRunner>) {
        self.stage_runner = runner;
    }

    /// Check if a stage should be skipped due to resumption.
    ///
    /// Normal case: both resume and current stages are in `full_execution_order`;
    /// skip the current stage if it comes before the resume point.
    ///
    /// Fallback: if the resume stage is not in the execution order (e.g. the
    /// workflow configuration changed between pause and resume, or a custom
    /// command was removed), core stages (branch through implement) are still
    /// skipped — they must have already completed for the workflow to have
    /// reached a post-implement pause point.
    fn should_skip_stage(&self, stage: &str) -> bool {
        match &self.resume_from_stage {
            None => false,
            Some(resume_stage) => {
                let resume_idx = self
                    .full_execution_order
                    .iter()
                    .position(|s| s == resume_stage.as_str());
                let current_idx = self
                    .full_execution_order
                    .iter()
                    .position(|s| s == stage);

                match (resume_idx, current_idx) {
                    (Some(resume), Some(current)) => current < resume,
                    (None, Some(current)) => {
                        // Resume stage not in current execution order (config changed
                        // between pause and resume, or legacy name not covered by
                        // normalization). Core stages are always present, so an unknown
                        // resume stage must be post-implement. Skip core stages to
                        // avoid expensive re-execution of plan/implement.
                        let implement_idx = self
                            .full_execution_order
                            .iter()
                            .position(|s| s == "implement");
                        let skip = implement_idx
                            .map(|impl_idx| current <= impl_idx)
                            .unwrap_or(false);
                        tracing::warn!(
                            "Resume stage '{}' not in execution order; {} core stage '{}'",
                            resume_stage,
                            if skip { "skipping" } else { "running" },
                            stage,
                        );
                        skip
                    }
                    _ => {
                        tracing::warn!(
                            "Stage '{}' not in execution order during resume check \
                             (resume_from='{}')",
                            stage,
                            resume_stage,
                        );
                        false
                    }
                }
            }
        }
    }

    /// Emit an event to the frontend
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
            tracing::debug!("No window or app_handle available to emit {}", event_name);
            Ok(())
        }
    }

    /// Check if the workflow has been cancelled
    fn is_cancelled(&self) -> bool {
        if self.cancelled.load(Ordering::Relaxed) {
            return true;
        }

        if let Ok(handles) = self.cancel_handles.lock() {
            if let Some(handle) = handles.get(&self.parent_run_id) {
                if handle.is_cancelled() {
                    return true;
                }
            }
        }

        false
    }

    fn stage_config_key(stage: &str) -> &str {
        match stage {
            "branch-gen" | "branch" => "branchGen",
            "plan" | "plan-validation" | "plan-decompose" => "plan",
            "implement" => "implement",
            "code-review" | "code-review-fix" => "code-review",
            "add-and-commit" => "commit",
            _ => stage,
        }
    }

    fn is_stage_enabled(&self, stage: &str) -> bool {
        let key = Self::stage_config_key(stage);
        self.stage_configs
            .get(key)
            .map(|c| c.enabled)
            .unwrap_or(true)
    }

    fn get_stage_model(&self, stage: &str) -> String {
        let key = Self::stage_config_key(stage);
        self.stage_configs
            .get(key)
            .map(|c| c.model.clone())
            .unwrap_or_else(|| crate::agents::models::DEFAULT_STAGE_MODEL.to_string())
    }

    /// Resolve the custom commands directory from the app handle.
    fn custom_commands_dir(&self) -> Option<PathBuf> {
        self.app_handle.as_ref().and_then(|handle| {
            use tauri::Manager;
            handle
                .path()
                .app_data_dir()
                .ok()
                .map(|d| d.join("custom-commands"))
                .filter(|d| d.exists())
        })
    }

    /// Return a clone of the current in-memory task.
    pub(super) fn get_task(&self) -> Option<Task> {
        self.task.read().ok().and_then(|guard| guard.clone())
    }

    /// Reload the task from the database, refreshing the in-memory copy.
    pub(super) fn refresh_task_from_db(&self) {
        let task_id = self
            .task
            .read()
            .ok()
            .and_then(|guard| guard.as_ref().map(|t| t.id.clone()));

        if let Some(id) = task_id {
            match self.db.get_task(&id) {
                Ok(fresh) => {
                    if let Ok(mut guard) = self.task.write() {
                        *guard = Some(fresh);
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to reload task {} from DB: {}", id, e);
                }
            }
        }
    }

    /// Extract text from agent output using the provider.
    pub(super) fn extract_text(&self, output: &str) -> String {
        self.provider.extract_text(output)
    }

    /// Extract cost from agent output using the provider.
    pub(super) fn extract_cost(
        &self,
        stdout: &str,
        stage_model: &str,
        duration_secs: f64,
    ) -> Option<crate::agents::cost::RunCostData> {
        crate::agents::provider::extract_cost_with_overrides(
            &*self.provider,
            stdout,
            stage_model,
            &self.agent_config,
            duration_secs,
        )
    }
}
