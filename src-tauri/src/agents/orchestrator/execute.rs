//! Main workflow execution logic for the orchestrator.

use super::auto_pilot;
use super::config::{TodoItemStatus, WorkflowMode};
use super::WorkflowOrchestrator;
use crate::agents::prompt::{
    generate_command_prompt, generate_implement_prompt, generate_plan_prompt,
    generate_task_implement_prompt, generate_task_plan_prompt, generate_task_prompt,
    generate_todo_implement_prompt,
};
use crate::db::models::TaskType;

impl WorkflowOrchestrator {
    /// Execute the workflow, dispatching based on workflow mode.
    pub async fn execute(&self) -> Result<(), String> {
        self.log_workflow_start();
        self.move_ticket_to_column("In Progress");

        match self.workflow_mode {
            WorkflowMode::AutoPilot => self.execute_auto_pilot().await,
            WorkflowMode::MultiStage => self.execute_multi_stage().await,
            WorkflowMode::CodeReviewOnly => self.execute_code_review_only().await,
        }
    }

    /// Execute the static multi-stage workflow pipeline.
    async fn execute_multi_stage(&self) -> Result<(), String> {
        if self.resume_from_stage.is_some() {
            self.restore_workflow_session_id();
        }

        let mut plan = String::new();

        for stage_key in &self.stage_order {
            match stage_key.as_str() {
                "branchGen" => {
                    self.handle_branch_creation().await?;
                }
                "plan" => {
                    plan = self.run_plan_stage().await?;
                }
                "implement" => {
                    self.run_implement_stage(&plan).await?;
                }
                "commit" => {
                    self.run_commit_stage().await?;
                }
                "code-review" => {
                    if self.should_skip_stage("code-review-fix") {
                        tracing::info!("Skipping code-review loop (resuming from later stage)");
                    } else if !self.is_stage_enabled("code-review") {
                        tracing::info!("Skipping code-review loop (disabled in workflow settings)");
                    } else if self.is_cancelled() {
                        return Err("Workflow cancelled".to_string());
                    } else {
                        self.run_code_review_loop().await?;
                    }
                }
                cmd => {
                    self.run_command_stage(cmd).await?;
                }
            }
        }

        self.run_detour_sync_if_needed().await?;
        self.finish_workflow("Multi-stage");
        Ok(())
    }

    /// Execute the auto-pilot workflow where the agent decides which commands to run.
    ///
    /// Execution order:
    /// 1. Branch creation, plan, implement
    /// 2. Pre-commands (required commands with phase=before)
    /// 3. LLM command selection (forced commands excluded from available list)
    /// 4. Agent-selected commands
    /// 5. Post-commands (required commands with phase=after)
    /// 6. Commit
    async fn execute_auto_pilot(&self) -> Result<(), String> {
        if self.resume_from_stage.is_some() {
            self.restore_workflow_session_id();
        }

        self.handle_branch_creation().await?;

        let plan = self.run_plan_stage().await?;

        let impl_result = self.run_implement_stage_capturing(&plan).await?;

        if self.is_cancelled() {
            return Err("Workflow cancelled".to_string());
        }

        let (pre_commands, post_commands) = auto_pilot::split_required_commands(
            &self.auto_pilot_required_commands,
            &self.stage_configs,
            &self.auto_pilot_model,
        );

        let forced_ids: Vec<&str> = self
            .auto_pilot_required_commands
            .iter()
            .map(|r| r.command.as_str())
            .collect();

        let custom_dir = self.custom_commands_dir();

        if !pre_commands.is_empty() {
            tracing::info!(
                "Auto-pilot: running {} pre-commands: {:?}",
                pre_commands.len(),
                pre_commands.iter().map(|s| &s.command).collect::<Vec<_>>(),
            );
            for selection in &pre_commands {
                if self.is_cancelled() {
                    return Err("Workflow cancelled".to_string());
                }
                self.run_auto_pilot_command(selection, custom_dir.as_deref())
                    .await?;
            }
        }

        if self.is_cancelled() {
            return Err("Workflow cancelled".to_string());
        }

        let selections = self
            .run_command_selection_stage_excluding(&plan, &impl_result, &forced_ids)
            .await?;

        let all_selections: Vec<auto_pilot::CommandSelection> = pre_commands
            .iter()
            .chain(selections.iter())
            .chain(post_commands.iter())
            .cloned()
            .collect();

        if let Err(e) = self.db.merge_run_metadata(
            &self.parent_run_id,
            &serde_json::json!({ "auto_pilot_selections": all_selections }),
        ) {
            tracing::warn!("Failed to persist auto-pilot selections: {}", e);
        }

        if selections.is_empty() {
            tracing::info!("Auto-pilot: agent selected no commands");
        } else {
            tracing::info!(
                "Auto-pilot: agent selected {} commands: {:?}",
                selections.len(),
                selections.iter().map(|s| &s.command).collect::<Vec<_>>()
            );
        }

        for selection in &selections {
            if self.is_cancelled() {
                return Err("Workflow cancelled".to_string());
            }
            self.run_auto_pilot_command(selection, custom_dir.as_deref())
                .await?;
        }

        if !post_commands.is_empty() {
            tracing::info!(
                "Auto-pilot: running {} post-commands: {:?}",
                post_commands.len(),
                post_commands.iter().map(|s| &s.command).collect::<Vec<_>>(),
            );
            for selection in &post_commands {
                if self.is_cancelled() {
                    return Err("Workflow cancelled".to_string());
                }
                self.run_auto_pilot_command(selection, custom_dir.as_deref())
                    .await?;
            }
        }

        self.run_commit_stage().await?;

        self.run_detour_sync_if_needed().await?;
        self.finish_workflow("Auto-pilot");
        Ok(())
    }

    /// Execute the code-review-only workflow: iteratively runs code-review + code-review-fix
    /// on the existing branch until no issues are found or cancelled.
    async fn execute_code_review_only(&self) -> Result<(), String> {
        if self.resume_from_stage.is_some() {
            self.restore_workflow_session_id();
        }

        self.handle_branch_creation().await?;

        if self.is_cancelled() {
            return Err("Workflow cancelled".to_string());
        }

        self.run_code_review_loop().await?;

        self.run_commit_stage().await?;

        self.run_detour_sync_if_needed().await?;
        self.finish_workflow("Code-review-only");
        Ok(())
    }

    /// Run a single auto-pilot command, dispatching composite commands
    /// (like code-review) to their iterative loop.
    async fn run_auto_pilot_command(
        &self,
        selection: &auto_pilot::CommandSelection,
        custom_dir: Option<&std::path::Path>,
    ) -> Result<(), String> {
        if selection.command == "code-review" {
            self.run_code_review_loop_with_model(&selection.model)
                .await
        } else {
            let base_prompt = generate_command_prompt(&selection.command, custom_dir);
            let prompt = self.append_workspace_context_to_prompt(&base_prompt);
            self.run_stage_with_model(
                &selection.command,
                &prompt,
                &selection.model,
            )
            .await
            .map(|_| ())
        }
    }

    pub(super) fn finish_workflow(&self, mode_label: &str) {
        let has_task = self.get_task().is_some();
        let has_pending = has_task
            && self
                .db
                .has_pending_tasks(&self.ticket.id)
                .unwrap_or(false);

        let should_auto_code_review = self.auto_code_review_on_complete
            && has_task
            && !has_pending
            && self.workflow_mode != WorkflowMode::CodeReviewOnly;

        if should_auto_code_review {
            match self.db.create_task(&crate::db::models::CreateTask {
                ticket_id: self.ticket.id.clone(),
                task_type: TaskType::CodeReview,
                title: Some("Code Review".to_string()),
                content: None,
            }) {
                Ok(task) => {
                    tracing::info!(
                        "Auto code review: created CodeReview task {} for ticket {} (last task completed)",
                        task.id,
                        self.ticket.id,
                    );
                    self.move_ticket_to_column("Ready");
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to create auto code review task for ticket {}: {}",
                        self.ticket.id,
                        e,
                    );
                    self.move_ticket_to_column("Review");
                }
            }
        } else if has_pending {
            tracing::info!(
                "Ticket {} has more pending tasks, moving back to Ready",
                self.ticket.id
            );
            self.move_ticket_to_column("Ready");
        } else if self.auto_complete_tickets {
            tracing::info!(
                "Auto-complete enabled: moving ticket {} directly to Done",
                self.ticket.id
            );
            self.move_ticket_to_column("Done");
        } else {
            self.move_ticket_to_column("Review");
        }

        self.add_workflow_summary_comment();
        tracing::info!(
            "{} workflow completed for ticket {}",
            mode_label,
            self.ticket.id
        );
    }

    /// Log the workflow start, handling resumption if applicable.
    fn log_workflow_start(&self) {
        let mode_label = match self.workflow_mode {
            WorkflowMode::AutoPilot => "auto-pilot",
            WorkflowMode::MultiStage => "multi-stage",
            WorkflowMode::CodeReviewOnly => "code-review-only",
        };
        if let Some(ref resume_stage) = self.resume_from_stage {
            tracing::info!(
                "Resuming {} workflow for ticket {} from stage '{}'",
                mode_label,
                self.ticket.id,
                resume_stage
            );
            if let Err(e) = self.db.clear_ticket_pause(&self.ticket.id) {
                tracing::warn!("Failed to clear ticket pause state: {}", e);
            }
        } else {
            tracing::info!(
                "Starting {} workflow for ticket {}",
                mode_label,
                self.ticket.id
            );
        }

        tracing::info!(
            "Workflow stage_configs: {} entries",
            self.stage_configs.len(),
        );
    }

    /// Workspace display name and (project name, path) pairs for multi-repo prompts.
    fn workspace_prompt_owned(&self) -> Option<(String, Vec<(String, String)>)> {
        let workspace_id = self.ticket.workspace_id.as_ref()?;
        let ws = self.db.get_workspace(workspace_id).ok().flatten()?;
        let projects = self.db.get_workspace_projects(workspace_id).ok()?;
        if projects.is_empty() {
            return None;
        }
        Some((
            ws.name,
            projects.into_iter().map(|p| (p.name, p.path)).collect(),
        ))
    }

    /// Run the plan stage and return the extracted plan.
    async fn run_plan_stage(&self) -> Result<String, String> {
        let plan = if self.should_skip_stage("plan") {
            tracing::info!("Skipping plan generation (resuming from later stage)");
            self.get_saved_plan().unwrap_or_else(|| {
                tracing::warn!(
                    "No saved plan found - implementation will proceed without plan context"
                );
                String::new()
            })
        } else {
            let mut plan = self.generate_plan_text().await?;

            const MAX_PLAN_REGENERATIONS: usize = 1;
            for attempt in 0..=MAX_PLAN_REGENERATIONS {
                let needs_regeneration = self.validate_and_process_plan(&plan).await?;
                if !needs_regeneration {
                    break;
                }
                if attempt == MAX_PLAN_REGENERATIONS {
                    tracing::warn!(
                        "Reached max plan regeneration attempts ({}), proceeding with current plan",
                        MAX_PLAN_REGENERATIONS,
                    );
                    break;
                }
                tracing::info!(
                    "Regenerating plan after auto-clarification updated task content (attempt {})",
                    attempt + 1,
                );
                plan = self.generate_plan_text().await?;
            }

            if !plan.is_empty() {
                self.add_plan_comment(&plan);
            }

            plan
        };

        if !plan.is_empty() && !self.should_skip_stage("plan-decompose") {
            if self.is_cancelled() {
                return Err("Workflow cancelled".to_string());
            }
            self.decompose_plan_into_todos(&plan).await;
        } else if self.should_skip_stage("plan-decompose") {
            self.load_todos_from_metadata();
        }

        Ok(plan)
    }

    /// Generate a plan by running the plan stage against the current task/ticket.
    async fn generate_plan_text(&self) -> Result<String, String> {
        if self.is_cancelled() {
            return Err("Workflow cancelled".to_string());
        }

        let workspace_owned = self.workspace_prompt_owned();
        let workspace_arg = workspace_owned
            .as_ref()
            .map(|(name, pairs)| (name.as_str(), pairs.as_slice()));

        let current_task = self.get_task();
        let plan_prompt = if let Some(ref task) = current_task {
            if matches!(task.task_type, TaskType::Command(_)) {
                tracing::info!(
                    "Skipping plan stage for command task type: {:?}",
                    task.task_type
                );
                String::new()
            } else {
                generate_task_plan_prompt(task, &self.ticket, workspace_arg)
            }
        } else {
            generate_plan_prompt(&self.ticket, workspace_arg)
        };

        if plan_prompt.is_empty() {
            return Ok(String::new());
        }

        let plan_result = self.run_stage("plan", &plan_prompt).await?;
        let raw_output = plan_result.captured_stdout.unwrap_or_default();
        let extracted = self.extract_text(&raw_output);

        tracing::info!(
            "Plan extraction: raw={} chars, extracted={} chars ({}% reduction)",
            raw_output.len(),
            extracted.len(),
            if raw_output.is_empty() {
                0
            } else {
                100 - (extracted.len() * 100 / raw_output.len())
            }
        );

        Ok(extracted)
    }

    /// Run the implement stage.
    async fn run_implement_stage(&self, plan: &str) -> Result<(), String> {
        self.run_implement_stage_capturing(plan).await?;
        Ok(())
    }

    /// Run the implement stage and return the extracted output text.
    pub(super) async fn run_implement_stage_capturing(&self, plan: &str) -> Result<String, String> {
        if self.should_skip_stage("implement") {
            tracing::info!("Skipping implement stage (resuming from later stage)");
            return Ok(self.get_previous_stage_output("implement").unwrap_or_else(|| {
                tracing::warn!(
                    "No saved implement output found - command selection will proceed without implementation context"
                );
                String::new()
            }));
        }

        if self.is_cancelled() {
            return Err("Workflow cancelled".to_string());
        }

        let workspace_owned = self.workspace_prompt_owned();
        let workspace_arg = workspace_owned
            .as_ref()
            .map(|(name, pairs)| (name.as_str(), pairs.as_slice()));

        let todos = self.get_implementation_todos();

        if todos.is_empty() {
            let current_task = self.get_task();
            let implement_prompt = if let Some(ref task) = current_task {
                if matches!(task.task_type, TaskType::Command(_)) {
                    let custom_dir = self.custom_commands_dir();
                    generate_task_prompt(
                        task,
                        &self.ticket,
                        custom_dir.as_deref(),
                        workspace_arg,
                    )
                } else {
                    generate_task_implement_prompt(task, &self.ticket, plan, workspace_arg)
                }
            } else {
                generate_implement_prompt(&self.ticket, plan, workspace_arg)
            };

            let impl_result = self.run_stage("implement", &implement_prompt).await?;
            let raw_output = impl_result.captured_stdout.unwrap_or_default();
            let text = self.extract_text(&raw_output);
            return Ok(text);
        }

        tracing::info!(
            "Running todo-based implementation: {} todos",
            todos.len()
        );

        let total = todos.len();

        let saved_statuses = self.load_todo_statuses_vec();
        let mut completed_count = saved_statuses
            .iter()
            .filter(|s| **s == TodoItemStatus::Completed)
            .count();

        let mut combined_output = if completed_count > 0 {
            tracing::info!(
                "Seeding combined output with {} previously completed todo(s)",
                completed_count
            );
            self.get_previous_stage_output("implement")
                .unwrap_or_default()
        } else {
            String::new()
        };

        // Emit initial progress so the frontend shows todos immediately,
        // including previously completed ones on resume.
        self.emit_implementation_progress(completed_count, total, "");

        for (idx, todo) in todos.iter().enumerate() {
            if let Some(status) = saved_statuses.get(idx) {
                match status {
                    TodoItemStatus::Completed => {
                        tracing::info!(
                            "Skipping todo {}/{} (already completed): {}",
                            idx + 1,
                            total,
                            todo.title
                        );
                        continue;
                    }
                    TodoItemStatus::Failed => {
                        tracing::warn!(
                            "Retrying previously failed todo {}/{}: {}",
                            idx + 1,
                            total,
                            todo.title
                        );
                        self.mark_todo_status(idx, TodoItemStatus::Pending);
                    }
                    TodoItemStatus::InProgress => {
                        tracing::warn!(
                            "Retrying interrupted todo {}/{} (was still InProgress): {}",
                            idx + 1,
                            total,
                            todo.title
                        );
                        self.mark_todo_status(idx, TodoItemStatus::Pending);
                    }
                    TodoItemStatus::Pending => {}
                }
            }

            if self.is_cancelled() {
                return Err("Workflow cancelled".to_string());
            }

            tracing::info!(
                "Implementing todo {}/{}: {}",
                idx + 1,
                total,
                todo.title
            );

            self.mark_todo_status(idx, TodoItemStatus::InProgress);
            self.emit_implementation_progress(completed_count, total, &todo.title);

            let prompt = generate_todo_implement_prompt(
                &self.ticket,
                plan,
                &todo.title,
                &todo.description,
                idx,
                total,
                workspace_arg,
            );

            match self.run_stage("implement", &prompt).await {
                Ok(result) => {
                    let raw_output = result.captured_stdout.unwrap_or_default();
                    let text = self.extract_text(&raw_output);
                    if !combined_output.is_empty() {
                        combined_output.push_str("\n\n");
                    }
                    combined_output.push_str(&text);
                    if self.mark_todo_status(idx, TodoItemStatus::Completed) {
                        completed_count += 1;
                    }
                    self.emit_implementation_progress(completed_count, total, "");
                }
                Err(e) => {
                    self.mark_todo_status(idx, TodoItemStatus::Failed);
                    self.emit_implementation_progress(completed_count, total, &todo.title);
                    return Err(e);
                }
            }
        }

        Ok(combined_output)
    }

    async fn run_commit_stage(&self) -> Result<(), String> {
        let cmd = "add-and-commit";
        if self.should_skip_stage(cmd) {
            tracing::info!("Skipping '{}' stage (resuming from later stage)", cmd);
        } else if !self.is_stage_enabled(cmd) {
            tracing::info!("Skipping '{}' stage (disabled in workflow settings)", cmd);
        } else if self.is_cancelled() {
            return Err("Workflow cancelled".to_string());
        } else {
            let custom_dir = self.custom_commands_dir();
            let base_prompt = generate_command_prompt(cmd, custom_dir.as_deref());
            let prompt = self.append_workspace_context_to_prompt(&base_prompt);
            self.run_stage(cmd, &prompt).await?;
        }

        self.commit_secondary_workspace_worktrees();

        Ok(())
    }

    /// For workspace tickets, commit any uncommitted changes in secondary project
    /// worktrees. The agent's add-and-commit only commits in the primary worktree
    /// (its CWD), but stages may have written changes to secondary projects via
    /// --add-dir paths.
    fn commit_secondary_workspace_worktrees(&self) {
        if self.ticket.workspace_id.is_none() {
            return;
        }

        let dirs = match crate::commands::next_steps::get_ticket_working_dirs(
            &self.db,
            &self.ticket.id,
        ) {
            Ok(dirs) if dirs.len() > 1 => dirs,
            _ => return,
        };

        let primary = self.repo_path.to_string_lossy().to_string();
        let commit_msg = format!("chore: {}", self.ticket.title);

        for (_, project_name, working_dir, _) in &dirs {
            if *working_dir == primary {
                continue;
            }

            if !crate::commands::next_steps::has_uncommitted_changes(working_dir) {
                continue;
            }

            tracing::info!(
                "Committing uncommitted changes in workspace project '{}' ({})",
                project_name,
                working_dir
            );

            if let Err(e) = crate::commands::next_steps::commit_all_changes(working_dir, &commit_msg) {
                tracing::error!(
                    "Failed to commit changes in workspace project '{}': {}",
                    project_name,
                    e
                );
            }
        }
    }

    async fn run_command_stage(&self, cmd: &str) -> Result<(), String> {
        if self.should_skip_stage(cmd) {
            tracing::info!("Skipping '{}' stage (resuming from later stage)", cmd);
            return Ok(());
        }
        if !self.is_stage_enabled(cmd) {
            tracing::info!("Skipping '{}' stage (disabled in workflow settings)", cmd);
            return Ok(());
        }
        if self.is_cancelled() {
            return Err("Workflow cancelled".to_string());
        }
        let custom_dir = self.custom_commands_dir();
        let base_prompt = generate_command_prompt(cmd, custom_dir.as_deref());
        let prompt = self.append_workspace_context_to_prompt(&base_prompt);
        self.run_stage(cmd, &prompt).await?;
        Ok(())
    }

    /// If working on a detour branch, merge the target branch to incorporate any
    /// changes the user may have pushed while the agent was working.
    async fn run_detour_sync_if_needed(&self) -> Result<(), String> {
        let target = match &self.target_branch {
            Some(t) => t.clone(),
            None => return Ok(()),
        };

        if self.is_cancelled() {
            return Err("Workflow cancelled".to_string());
        }

        tracing::info!(
            "Running detour-sync: merging '{}' into current branch",
            target
        );

        let prompt = format!(
            r#"Merge the branch `{target}` into the current branch to synchronize changes.

## Instructions
1. Run `git merge {target}`
2. If there are merge conflicts, resolve them carefully:
   - Examine each conflicting file
   - Choose the correct resolution based on the intent of both sets of changes
   - Stage resolved files with `git add`
   - Complete the merge with `git commit`
3. If the merge completes cleanly (no conflicts), you're done.
4. If the branch is already up to date, no action is needed.

Do NOT make any other code changes. Only perform the merge."#
        );

        self.run_stage("detour-sync", &prompt).await?;
        Ok(())
    }
}
