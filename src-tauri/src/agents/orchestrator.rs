//! Multi-stage workflow orchestrator for chaining Claude CLI calls

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager, Window};

use crate::db::{Database, AgentType, CreateRun, RunStatus, Ticket, NormalizedEvent, EventType, AgentEventPayload, CreateComment, AuthorType};
use crate::db::models::{Task, TaskType};
use super::{AgentKind, AgentRunConfig, AgentRunResult, LogCallback, LogLine, LogStream, RunOutcome, extract_text_from_stream_json};
use super::prompt::{generate_branch_name_generation_prompt, parse_branch_name_from_output, generate_plan_prompt, generate_implement_prompt, generate_command_prompt, generate_task_plan_prompt, generate_task_implement_prompt, generate_task_prompt};
use super::spawner::{run_agent_with_capture, CancelHandle};
use super::claude as claude_hooks;
use super::cursor as cursor_hooks;
use super::plan_validation::{validate_plan_for_clarification, generate_clarification_message, PlanValidationConfig};

/// Type alias for the shared cancel handles map
pub type CancelHandlesMap = Arc<Mutex<HashMap<String, CancelHandle>>>;

/// Configuration for creating a WorkflowOrchestrator
pub struct OrchestratorConfig {
    pub db: Arc<Database>,
    pub window: Option<Window>,
    pub app_handle: Option<AppHandle>,
    pub parent_run_id: String,
    pub ticket: Ticket,
    /// The task being executed. If None, falls back to legacy ticket-based workflow.
    pub task: Option<Task>,
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
}

/// The stages in a multi-stage workflow
pub const MULTI_STAGE_WORKFLOW: &[&str] = &[
    "branch",
    "plan", 
    "implement",
    "deslop",
    "cleanup",
    "unit-tests",
    "cleanup",
    "review-changes",
    "cleanup",
    "review-changes",
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
}

impl WorkflowOrchestrator {
    pub fn new(config: OrchestratorConfig) -> Self {
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
        }
    }
    
    /// Emit an event to the frontend, using window if available, otherwise app_handle
    fn emit_event<S: serde::Serialize + Clone>(&self, event_name: &str, payload: &S) -> Result<(), String> {
        if let Some(ref window) = self.window {
            window.emit(event_name, payload)
                .map_err(|e| format!("Failed to emit {} via window: {}", event_name, e))
        } else if let Some(ref app_handle) = self.app_handle {
            app_handle.emit_all(event_name, payload)
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
            AgentKind::Cursor => {
                cursor_hooks::install_hooks_with_run_id(
                    &self.repo_path,
                    hook_script_path,
                    Some(&self.api_url),
                    Some(&self.api_token),
                    Some(&self.parent_run_id),
                )
                .map_err(|e| format!("Failed to update Cursor hooks: {}", e))
            }
            AgentKind::Claude => {
                claude_hooks::install_local_hooks_with_run_id(
                    &self.repo_path,
                    hook_script_path,
                    Some(&self.api_url),
                    Some(&self.api_token),
                    Some(&self.parent_run_id),
                )
                .map_err(|e| format!("Failed to update Claude hooks: {}", e))
            }
        }
    }

    /// Execute the full multi-stage workflow
    pub async fn execute(&self) -> Result<(), String> {
        tracing::info!("Starting multi-stage workflow for ticket {}", self.ticket.id);
        
        // Move ticket to "In Progress" when workflow starts
        self.move_ticket_to_column("In Progress");
        
        // Handle branch creation based on whether we already have a branch name
        // and whether it was already created (e.g., via worktree)
        if let Some(ref branch_name) = self.worktree_branch {
            if self.is_temp_branch {
                // Temp branch exists but needs to be renamed to an AI-generated name
                tracing::info!("Temp branch '{}' exists, generating AI name and renaming...", branch_name);
                
                if self.is_cancelled() {
                    return Err("Workflow cancelled".to_string());
                }
                
                let branch_gen_result = self.run_stage("branch-gen", &generate_branch_name_generation_prompt(&self.ticket)).await?;
                
                // Try to parse the generated branch name
                let generated_branch = branch_gen_result.captured_stdout
                    .as_ref()
                    .and_then(|output| {
                        let text_content = extract_text_from_stream_json(output)
                            .unwrap_or_else(|| output.clone());
                        tracing::debug!("Branch-gen output (extracted): {}", text_content);
                        parse_branch_name_from_output(&text_content)
                    });
                
                // Use generated name or fall back to deterministic
                let new_branch_name = if let Some(ref name) = generated_branch {
                    tracing::info!("Agent generated branch name: {}", name);
                    name.clone()
                } else {
                    let fallback = super::worktree::generate_branch_name(&self.ticket.id, &self.ticket.title);
                    tracing::warn!("Could not parse generated branch name, using fallback: {}", fallback);
                    fallback
                };
                
                // Rename the temp branch to the new name BEFORE updating the database.
                // This ensures the database only records the new branch name after the git
                // rename succeeds. If we updated the database first and the rename failed,
                // the database would have the new name while git still has the old name,
                // causing subsequent runs to fail when they try to use the recorded branch.
                if self.is_cancelled() {
                    return Err("Workflow cancelled".to_string());
                }
                
                let rename_prompt = format!(
                    r#"Rename the current git branch to a better name.

## Task
Rename the current branch from `{}` to `{}`

## Instructions
1. You should already be on the branch `{}`
2. Rename the current branch: `git branch -m {}`
3. Push the renamed branch to origin: `git push -u origin {}`
4. Delete the old branch from origin (if it was pushed): `git push origin --delete {}` (ignore errors if it doesn't exist remotely)

Do NOT start implementing any code changes. Just rename the branch.
"#,
                    branch_name, new_branch_name,
                    branch_name, new_branch_name, new_branch_name, branch_name
                );
                
                let _rename_result = self.run_stage("branch", &rename_prompt).await?;
                
                // Now that the git rename succeeded, store the NEW branch name on ticket
                if let Err(e) = self.db.set_ticket_branch(&self.ticket.id, &new_branch_name) {
                    tracing::warn!("Failed to store branch name on ticket: {}", e);
                } else {
                    tracing::info!("Stored branch name '{}' on ticket {}", new_branch_name, self.ticket.id);
                    let _ = self.emit_event("ticket-branch-updated", &serde_json::json!({
                        "ticketId": self.ticket.id,
                        "branchName": new_branch_name,
                    }));
                }
            } else {
                // We have a permanent branch name already
                tracing::info!("Using pre-determined branch name: {}", branch_name);
                
                // Store branch name on ticket if not already set
                if self.ticket.branch_name.is_none() {
                    if let Err(e) = self.db.set_ticket_branch(&self.ticket.id, branch_name) {
                        tracing::warn!("Failed to store branch name on ticket: {}", e);
                    } else {
                        tracing::info!("Stored branch name '{}' on ticket {}", branch_name, self.ticket.id);
                    }
                }
                
                // If branch wasn't already created (e.g., worktree creation failed),
                // we need to create it now
                if !self.branch_already_created {
                    tracing::info!("Branch '{}' not yet created, creating now...", branch_name);
                    
                    if self.is_cancelled() {
                        return Err("Workflow cancelled".to_string());
                    }
                    
                    let branch_prompt = format!(
                        r#"Create a new git branch for this task.

## Task
Create and switch to a new branch: `{}`

## Instructions
1. Check if you're on a clean working tree (stash changes if needed)
2. Switch to the main branch (or master if main doesn't exist)
3. Pull the latest changes from origin: `git pull origin main`
4. Create and switch to the new branch from main
5. Push the branch to origin with -u flag

Do NOT start implementing any code changes. Just create the branch.
"#,
                        branch_name
                    );
                    
                    let _branch_result = self.run_stage("branch", &branch_prompt).await?;
                }
            }
        } else {
            // No branch name yet - generate and create a branch
            // (This path is kept for backwards compatibility but shouldn't normally be hit)
            tracing::info!("No branch name provided, generating and creating new branch...");
            
            if self.is_cancelled() {
                return Err("Workflow cancelled".to_string());
            }
            
            let branch_gen_result = self.run_stage("branch-gen", &generate_branch_name_generation_prompt(&self.ticket)).await?;
            
            // Try to parse the generated branch name
            // For Claude, we need to extract text from stream-json format first
            let generated_branch = branch_gen_result.captured_stdout
                .as_ref()
                .and_then(|output| {
                    // Try extracting text from stream-json (Claude format)
                    let text_content = extract_text_from_stream_json(output)
                        .unwrap_or_else(|| output.clone());
                    
                    tracing::debug!("Branch-gen output (extracted): {}", text_content);
                    parse_branch_name_from_output(&text_content)
                });
            
            // Use generated name or fall back to deterministic
            let branch_to_create = if let Some(ref name) = generated_branch {
                tracing::info!("Agent generated branch name: {}", name);
                name.clone()
            } else {
                // Fallback to deterministic naming
                let fallback = super::worktree::generate_branch_name(&self.ticket.id, &self.ticket.title);
                tracing::warn!("Could not parse generated branch name, using fallback: {}", fallback);
                fallback
            };
            
            // Store branch name on ticket BEFORE creating the branch
            // This allows the UI to show the branch immediately
            if let Err(e) = self.db.set_ticket_branch(&self.ticket.id, &branch_to_create) {
                tracing::warn!("Failed to store branch name on ticket: {}", e);
            } else {
                tracing::info!("Stored branch name '{}' on ticket {}", branch_to_create, self.ticket.id);
                // Emit event for frontend to update the ticket display
                let _ = self.emit_event("ticket-branch-updated", &serde_json::json!({
                    "ticketId": self.ticket.id,
                    "branchName": branch_to_create,
                }));
            }
            
            // Now have the agent create the branch with that name
            if self.is_cancelled() {
                return Err("Workflow cancelled".to_string());
            }
            
            let branch_prompt = format!(
                r#"Create a new git branch for this task.

## Task
Create and switch to a new branch: `{}`

## Instructions
1. Check if you're on a clean working tree (stash changes if needed)
2. Switch to the main branch (or master if main doesn't exist)
3. Pull the latest changes from origin: `git pull origin main`
4. Create and switch to the new branch from main
5. Push the branch to origin with -u flag

Do NOT start implementing any code changes. Just create the branch.
"#,
                branch_to_create
            );
            
            let _branch_result = self.run_stage("branch", &branch_prompt).await?;
        }
        
        // Stage 1: Plan
        if self.is_cancelled() {
            return Err("Workflow cancelled".to_string());
        }
        
        // Use task-based prompts if we have a task, otherwise fall back to ticket-based
        let plan_prompt = if let Some(ref task) = self.task {
            // For preset tasks, we skip the plan stage and go directly to execution
            if task.task_type != TaskType::Custom {
                // Skip plan for preset tasks - they have their own instructions
                tracing::info!("Skipping plan stage for preset task type: {:?}", task.task_type);
                String::new()
            } else {
                generate_task_plan_prompt(task, &self.ticket)
            }
        } else {
            generate_plan_prompt(&self.ticket)
        };
        
        let plan = if !plan_prompt.is_empty() {
            let plan_result = self.run_stage("plan", &plan_prompt).await?;
            // Extract only the text content from stream-json output.
            // The raw captured_stdout contains all tool calls, file reads, grep results, etc.
            // which can be 100K+ tokens. We only need the final plan text.
            let raw_output = plan_result.captured_stdout.unwrap_or_default();
            let extracted = extract_text_from_stream_json(&raw_output)
                .unwrap_or_else(|| raw_output.clone());
            
            tracing::info!(
                "Plan extraction: raw={} chars, extracted={} chars ({}% reduction)",
                raw_output.len(),
                extracted.len(),
                if raw_output.is_empty() { 0 } else { 100 - (extracted.len() * 100 / raw_output.len()) }
            );
            
            extracted
        } else {
            String::new()
        };
        
        if !plan.is_empty() {
            self.add_plan_comment(&plan);
            
            tracing::info!("Running plan clarification validation for ticket {}", self.ticket.id);
            
            let validation_config = PlanValidationConfig {
                db: self.db.clone(),
                parent_run_id: self.parent_run_id.clone(),
                ticket_id: self.ticket.id.clone(),
                repo_path: self.repo_path.clone(),
                api_url: self.api_url.clone(),
                api_token: self.api_token.clone(),
                model: self.ticket.model.clone(),
                agent_kind: self.agent_kind,
            };
            
            let validation_result = validate_plan_for_clarification(&validation_config, &plan).await;
            
            match validation_result {
                Ok(result) if result.needs_clarification => {
                    tracing::info!(
                        "Plan requires clarification for ticket {}: {}",
                        self.ticket.id,
                        result.reason
                    );
                    
                    let clarification_message = generate_clarification_message(&validation_config, &plan)
                        .await
                        .unwrap_or_else(|e| {
                            tracing::warn!("Failed to generate clarification message: {}", e);
                            format!("Clarification needed: {}", result.reason)
                        });
                    
                    self.add_clarification_comment(&clarification_message);
                    self.move_ticket_to_column("Blocked");
                    
                    return Err(format!("Plan requires user clarification: {}", result.reason));
                }
                Ok(result) => {
                    tracing::info!(
                        "Plan validation passed for ticket {}: {}",
                        self.ticket.id,
                        result.reason
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "Plan validation failed for ticket {}, proceeding anyway: {}",
                        self.ticket.id,
                        e
                    );
                }
            }
        }
        
        // Stage 2: Implement
        if self.is_cancelled() {
            return Err("Workflow cancelled".to_string());
        }
        
        let implement_prompt = if let Some(ref task) = self.task {
            // For preset tasks, use the preset-specific prompt
            if task.task_type != TaskType::Custom {
                generate_task_prompt(task, &self.ticket, &self.repo_path)
            } else {
                generate_task_implement_prompt(task, &self.ticket, &plan)
            }
        } else {
            generate_implement_prompt(&self.ticket, &plan)
        };
        
        let _impl_result = self.run_stage("implement", &implement_prompt).await?;
        
        // Move ticket to "Review" when entering QA phase
        self.move_ticket_to_column("Review");
        
        // Stage 3+: QA Commands
        let qa_commands = &["deslop", "cleanup", "unit-tests", "cleanup", 
                           "review-changes", "cleanup", "review-changes", "add-and-commit"];
        
        for cmd in qa_commands {
            if self.is_cancelled() {
                return Err("Workflow cancelled".to_string());
            }
            self.run_stage(cmd, &generate_command_prompt(cmd, &self.repo_path)).await?;
        }
        
        // Move ticket to "Done" when workflow completes successfully
        self.move_ticket_to_column("Done");
        
        // Add workflow completion summary comment
        self.add_workflow_summary_comment();
        
        tracing::info!("Multi-stage workflow completed for ticket {}", self.ticket.id);
        Ok(())
    }
    
    /// Add a completion summary comment for the workflow
    fn add_workflow_summary_comment(&self) {
        let comment_text = format!(
            "## Workflow Complete\n\nMulti-stage workflow completed successfully for ticket **{}**.\n\n\
            Stages completed: branch, plan, implement, deslop, cleanup, unit-tests, review-changes, add-and-commit",
            self.ticket.title
        );
        let create_comment = CreateComment {
            ticket_id: self.ticket.id.clone(),
            author_type: AuthorType::Agent,
            body_md: comment_text.clone(),
            metadata: Some(serde_json::json!({
                "type": "workflow_complete",
                "parent_run_id": self.parent_run_id,
            })),
        };
        if let Err(e) = self.db.create_comment(&create_comment) {
            tracing::warn!("Failed to add workflow summary comment: {}", e);
        } else {
            tracing::info!("Added workflow summary comment for ticket {}", self.ticket.id);
            let _ = self.emit_event("ticket-comment-added", &serde_json::json!({
                "ticketId": self.ticket.id,
                "comment": comment_text,
            }));
        }
    }
    
    /// Add a comment with the extracted plan for visibility and debugging
    fn add_plan_comment(&self, plan: &str) {
        let comment_text = format!(
            "## Implementation Plan\n\n{}\n\n---\n*This plan was extracted from the planning stage and will guide the implementation.*",
            plan.trim()
        );
        let create_comment = CreateComment {
            ticket_id: self.ticket.id.clone(),
            author_type: AuthorType::Agent,
            body_md: comment_text.clone(),
            metadata: Some(serde_json::json!({
                "type": "plan",
                "parent_run_id": self.parent_run_id,
            })),
        };
        if let Err(e) = self.db.create_comment(&create_comment) {
            tracing::warn!("Failed to add plan comment: {}", e);
        } else {
            tracing::info!(
                "Added plan comment for ticket {} ({} chars)", 
                self.ticket.id,
                plan.len()
            );
            let _ = self.emit_event("ticket-comment-added", &serde_json::json!({
                "ticketId": self.ticket.id,
                "comment": comment_text,
            }));
        }
    }
    
    /// Add a clarification request comment when the plan needs user input
    fn add_clarification_comment(&self, message: &str) {
        let comment_text = format!(
            "## Clarification Needed\n\n{}\n\n---\n*Please update the ticket description with the requested information and move this ticket back to Ready to continue.*",
            message.trim()
        );
        let create_comment = CreateComment {
            ticket_id: self.ticket.id.clone(),
            author_type: AuthorType::Agent,
            body_md: comment_text.clone(),
            metadata: Some(serde_json::json!({
                "type": "clarification",
                "parent_run_id": self.parent_run_id,
            })),
        };
        if let Err(e) = self.db.create_comment(&create_comment) {
            tracing::warn!("Failed to add clarification comment: {}", e);
        } else {
            tracing::info!(
                "Added clarification comment for ticket {} ({} chars)", 
                self.ticket.id,
                message.len()
            );
            let _ = self.emit_event("ticket-comment-added", &serde_json::json!({
                "ticketId": self.ticket.id,
                "comment": comment_text,
            }));
        }
    }
    
    /// Move the ticket to a column by name (best effort - logs warning if column not found)
    fn move_ticket_to_column(&self, column_name: &str) {
        tracing::info!("Attempting to move ticket {} to column '{}' on board {}", 
            self.ticket.id, column_name, self.ticket.board_id);
        
        match self.db.find_column_by_name(&self.ticket.board_id, column_name) {
            Ok(Some(column)) => {
                tracing::info!("Found column '{}' with id {} for board {}", 
                    column_name, column.id, self.ticket.board_id);
                if let Err(e) = self.db.move_ticket(&self.ticket.id, &column.id) {
                    tracing::error!("Failed to move ticket {} to '{}': {}", self.ticket.id, column_name, e);
                } else {
                    tracing::info!("Successfully moved ticket {} to column '{}'", self.ticket.id, column_name);
                    // Emit event for frontend to update
                    if let Err(e) = self.emit_event("ticket-moved", &serde_json::json!({
                        "ticketId": self.ticket.id,
                        "columnName": column_name,
                        "columnId": column.id,
                    })) {
                        tracing::warn!("Failed to emit ticket-moved event: {}", e);
                    } else {
                        tracing::info!("Emitted ticket-moved event for ticket {}", self.ticket.id);
                    }
                }
            }
            Ok(None) => {
                tracing::error!("Column '{}' not found for board {}. Looking up available columns...", 
                    column_name, self.ticket.board_id);
                // Log available columns for debugging
                if let Ok(columns) = self.db.get_columns(&self.ticket.board_id) {
                    let column_names: Vec<_> = columns.iter().map(|c| c.name.as_str()).collect();
                    tracing::error!("Available columns on board: {:?}", column_names);
                }
            }
            Err(e) => {
                tracing::error!("Error finding column '{}': {}", column_name, e);
            }
        }
    }

    /// Run a single stage of the workflow
    async fn run_stage(
        &self,
        stage: &str,
        prompt: &str,
    ) -> Result<AgentRunResult, String> {
        tracing::info!("Starting stage '{}' for parent run {}", stage, self.parent_run_id);
        
        // Emit stage started event
        self.emit_stage_event(stage, "running", None, None);
        
        // Create sub-run in database
        let sub_run = self.db.create_run(&CreateRun {
            ticket_id: self.ticket.id.clone(),
            agent_type: match self.agent_kind {
                AgentKind::Cursor => AgentType::Cursor,
                AgentKind::Claude => AgentType::Claude,
            },
            repo_path: self.repo_path.to_string_lossy().to_string(),
            parent_run_id: Some(self.parent_run_id.clone()),
            stage: Some(stage.to_string()),
        }).map_err(|e| format!("Failed to create sub-run: {}", e))?;
        
        // Update project hooks with parent run configuration
        // This ensures the hook script has the correct run_id for API calls
        // We use the PARENT run_id so events are grouped under the main workflow run
        if let Err(e) = self.update_hooks_for_run() {
            tracing::warn!("Failed to update hooks for stage '{}': {}", stage, e);
            // Continue anyway - hooks might work with existing configuration
        }
        
        // Update sub-run status to running
        self.db.update_run_status(&sub_run.id, RunStatus::Running, None, None)
            .map_err(|e| format!("Failed to update sub-run status: {}", e))?;
        
        // Build agent config
        let config = AgentRunConfig {
            kind: self.agent_kind,
            ticket_id: self.ticket.id.clone(),
            run_id: sub_run.id.clone(),
            repo_path: self.repo_path.clone(),
            prompt: prompt.to_string(),
            timeout_secs: Some(1800), // 30 minutes per stage
            api_url: self.api_url.clone(),
            api_token: self.api_token.clone(),
            model: self.ticket.model.clone(),
        };
        
        // Create log callback
        // Use PARENT run ID for both database storage and frontend events
        // This ensures events can be retrieved using ticket.lockedByRunId
        let db_for_logs = self.db.clone();
        let window_for_logs = self.window.clone();
        let app_handle_for_logs = self.app_handle.clone();
        let parent_run_id_for_logs = self.parent_run_id.clone();
        let ticket_id_for_logs = self.ticket.id.clone();
        let db_agent_type = match self.agent_kind {
            AgentKind::Cursor => AgentType::Cursor,
            AgentKind::Claude => AgentType::Claude,
        };
        let stage_for_logs = stage.to_string();
        
        let on_log: Arc<LogCallback> = Arc::new(Box::new(move |log: LogLine| {
            let stream_name = match log.stream {
                LogStream::Stdout => "stdout",
                LogStream::Stderr => "stderr",
            };
            tracing::debug!("LOG [{}:{}]: [{}] - {} chars", 
                stage_for_logs,
                parent_run_id_for_logs,
                stream_name,
                log.content.len()
            );
            
            // Store log to database with PARENT run ID
            // This allows frontend to retrieve events using ticket.lockedByRunId
            let normalized_event = NormalizedEvent {
                run_id: parent_run_id_for_logs.clone(),
                ticket_id: ticket_id_for_logs.clone(),
                agent_type: db_agent_type,
                event_type: EventType::Custom(format!("log_{}", stream_name)),
                payload: AgentEventPayload {
                    raw: Some(log.content.clone()),
                    structured: None,
                },
                timestamp: log.timestamp,
            };
            if let Err(e) = db_for_logs.create_event(&normalized_event) {
                tracing::error!("Failed to persist log event: {}", e);
            }
            
            // Emit to frontend for real-time display
            // Use window if available (direct agent runs), otherwise use app_handle (worker runs)
            #[derive(serde::Serialize, Clone)]
            #[serde(rename_all = "camelCase")]
            struct AgentLogEvent {
                run_id: String,
                stream: String,
                content: String,
                timestamp: String,
            }
            let event = AgentLogEvent {
                run_id: parent_run_id_for_logs.clone(),
                stream: stream_name.to_string(),
                content: log.content,
                timestamp: log.timestamp.to_rfc3339(),
            };
            
            if let Some(ref window) = window_for_logs {
                if let Err(e) = window.emit("agent-log", &event) {
                    tracing::error!("Failed to emit agent-log event via window: {}", e);
                }
            } else if let Some(ref app_handle) = app_handle_for_logs {
                if let Err(e) = app_handle.emit_all("agent-log", &event) {
                    tracing::error!("Failed to emit agent-log event via app_handle: {}", e);
                }
            }
        }));
        
        // Set up cancel handle registration
        let cancel_handles = self.cancel_handles.clone();
        let sub_run_id_for_spawn = sub_run.id.clone();
        let sub_run_id_for_cleanup = sub_run.id.clone();
        // Also register the parent run ID so cancelling the parent works
        let parent_run_id = self.parent_run_id.clone();
        let cancelled = self.cancelled.clone();
        
        let on_spawn: super::spawner::OnSpawnCallback = Box::new(move |cancel_handle| {
            tracing::info!("Sub-run {} spawned for parent {}", sub_run_id_for_spawn, parent_run_id);
            let mut handles = cancel_handles.lock().expect("cancel handles mutex poisoned");
            
            // Check if the previous handle for parent run was cancelled (cancellation between stages)
            // If so, immediately cancel the new handle too to propagate the cancellation
            if let Some(prev_handle) = handles.get(&parent_run_id) {
                if prev_handle.is_cancelled() {
                    tracing::info!(
                        "Previous handle for parent {} was cancelled, propagating to new sub-run {}",
                        parent_run_id, sub_run_id_for_spawn
                    );
                    cancel_handle.cancel();
                }
            }
            
            // Register under both the sub-run ID and the parent run ID
            handles.insert(sub_run_id_for_spawn.clone(), cancel_handle.clone());
            handles.insert(parent_run_id.clone(), cancel_handle);
        });
        
        // Run the agent with capture
        let start_time = std::time::Instant::now();
        let result = tokio::task::spawn_blocking(move || {
            run_agent_with_capture(config, Some(on_log), Some(on_spawn))
        }).await
            .map_err(|e| format!("Stage task failed: {}", e))?
            .map_err(|e| format!("Stage execution failed: {}", e))?;
        
        // Clean up cancel handles
        {
            let mut handles = self.cancel_handles.lock().expect("cancel handles mutex poisoned");
            handles.remove(&sub_run_id_for_cleanup);
            // Don't remove the parent run ID handle yet - it will be updated with the next sub-run's handle
        }
        
        // Check if we were cancelled during execution
        if result.status == RunOutcome::Cancelled {
            cancelled.store(true, Ordering::Relaxed);
        }
        
        let duration_secs = start_time.elapsed().as_secs_f64();
        
        // Update sub-run status
        let status = match result.status {
            RunOutcome::Success => RunStatus::Finished,
            RunOutcome::Error => RunStatus::Error,
            RunOutcome::Timeout => RunStatus::Error,
            RunOutcome::Cancelled => RunStatus::Aborted,
        };
        
        self.db.update_run_status(
            &sub_run.id,
            status.clone(),
            result.exit_code,
            result.summary.as_deref(),
        ).map_err(|e| format!("Failed to update sub-run status: {}", e))?;
        
        // Emit stage completed event
        self.emit_stage_event(
            stage, 
            status.as_str(), 
            Some(sub_run.id.clone()),
            Some(duration_secs),
        );
        
        // Check if stage failed
        if result.status != RunOutcome::Success {
            return Err(format!("Stage '{}' failed with status {:?}", stage, result.status));
        }
        
        tracing::info!("Stage '{}' completed in {:.1}s", stage, duration_secs);
        Ok(result)
    }
    
    /// Emit a stage event to the frontend
    fn emit_stage_event(&self, stage: &str, status: &str, sub_run_id: Option<String>, duration_secs: Option<f64>) {
        let event = StageEvent {
            parent_run_id: self.parent_run_id.clone(),
            stage: stage.to_string(),
            status: status.to_string(),
            sub_run_id,
            duration_secs,
        };
        if let Err(e) = self.emit_event("agent-stage-update", &event) {
            tracing::warn!("Failed to emit stage event: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_event_serializes() {
        let event = StageEvent {
            parent_run_id: "run-1".to_string(),
            stage: "plan".to_string(),
            status: "running".to_string(),
            sub_run_id: None,
            duration_secs: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("parentRunId"));
        assert!(json.contains("plan"));
    }

    #[test]
    fn multi_stage_workflow_has_expected_stages() {
        assert!(MULTI_STAGE_WORKFLOW.contains(&"branch"));
        assert!(MULTI_STAGE_WORKFLOW.contains(&"plan"));
        assert!(MULTI_STAGE_WORKFLOW.contains(&"implement"));
        assert!(MULTI_STAGE_WORKFLOW.contains(&"add-and-commit"));
    }
}
