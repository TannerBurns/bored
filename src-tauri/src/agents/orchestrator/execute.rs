//! Main workflow execution logic for the orchestrator.

use super::config::WorkflowMode;
use super::WorkflowOrchestrator;
use crate::agents::plan_validation::{
    generate_clarification_message, validate_plan_for_clarification, PlanValidationConfig,
};
use crate::agents::prompt::{
    generate_command_prompt, generate_implement_prompt, generate_plan_prompt,
    generate_task_implement_prompt, generate_task_plan_prompt, generate_task_prompt,
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
        }
    }

    /// Execute the static multi-stage workflow pipeline.
    async fn execute_multi_stage(&self) -> Result<(), String> {
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
    async fn execute_auto_pilot(&self) -> Result<(), String> {
        self.handle_branch_creation().await?;

        let plan = self.run_plan_stage().await?;

        let impl_result = self.run_implement_stage_capturing(&plan).await?;

        if self.is_cancelled() {
            return Err("Workflow cancelled".to_string());
        }

        let selections = self.run_command_selection_stage(&plan, &impl_result).await?;

        if let Err(e) = self.db.merge_run_metadata(
            &self.parent_run_id,
            &serde_json::json!({ "auto_pilot_selections": selections }),
        ) {
            tracing::warn!("Failed to persist auto-pilot selections: {}", e);
        }

        if selections.is_empty() {
            tracing::info!("Auto-pilot: agent selected no commands, proceeding to commit");
        } else {
            tracing::info!(
                "Auto-pilot: agent selected {} commands: {:?}",
                selections.len(),
                selections.iter().map(|s| &s.command).collect::<Vec<_>>()
            );
        }

        let custom_dir = self.custom_commands_dir();
        for selection in &selections {
            if self.is_cancelled() {
                return Err("Workflow cancelled".to_string());
            }
            self.run_stage_with_model(
                &selection.command,
                &generate_command_prompt(
                    &selection.command,
                    custom_dir.as_deref(),
                ),
                &selection.model,
            )
            .await?;
        }

        self.run_commit_stage().await?;

        self.run_detour_sync_if_needed().await?;
        self.finish_workflow("Auto-pilot");
        Ok(())
    }

    fn finish_workflow(&self, mode_label: &str) {
        self.move_ticket_to_column("Review");
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

    /// Run the plan stage and return the extracted plan.
    async fn run_plan_stage(&self) -> Result<String, String> {
        if self.should_skip_stage("plan") {
            tracing::info!("Skipping plan stage (resuming from later stage)");
            return Ok(self.get_saved_plan().unwrap_or_else(|| {
                tracing::warn!(
                    "No saved plan found - implementation will proceed without plan context"
                );
                String::new()
            }));
        }

        if self.is_cancelled() {
            return Err("Workflow cancelled".to_string());
        }

        let plan_prompt = if let Some(ref task) = self.task {
            if matches!(task.task_type, TaskType::Command(_)) {
                tracing::info!(
                    "Skipping plan stage for command task type: {:?}",
                    task.task_type
                );
                String::new()
            } else {
                generate_task_plan_prompt(task, &self.ticket)
            }
        } else {
            generate_plan_prompt(&self.ticket)
        };

        if plan_prompt.is_empty() {
            return Ok(String::new());
        }

        let plan_result = self.run_stage("plan", &plan_prompt).await?;
        let raw_output = plan_result.captured_stdout.unwrap_or_default();
        let plan = self.extract_text(&raw_output);

        tracing::info!(
            "Plan extraction: raw={} chars, extracted={} chars ({}% reduction)",
            raw_output.len(),
            plan.len(),
            if raw_output.is_empty() {
                0
            } else {
                100 - (plan.len() * 100 / raw_output.len())
            }
        );

        self.validate_and_process_plan(&plan).await?;

        Ok(plan)
    }

    /// Validate the plan and handle clarification if needed.
    async fn validate_and_process_plan(&self, plan: &str) -> Result<(), String> {
        if plan.is_empty() || self.should_skip_stage("plan") {
            return Ok(());
        }

        self.add_plan_comment(plan);

        tracing::info!(
            "Running plan clarification validation for ticket {}",
            self.ticket.id
        );

        let validation_config = PlanValidationConfig {
            db: self.db.clone(),
            parent_run_id: self.parent_run_id.clone(),
            ticket_id: self.ticket.id.clone(),
            repo_path: self.repo_path.clone(),
            model: Some(self.get_stage_model("plan")),
            agent_id: self.agent_id.clone(),
            provider: self.provider.clone(),
            agent_config: self.agent_config.clone(),
            timeout_secs: self.stage_timeout_secs,
        };

        let validation_result = validate_plan_for_clarification(&validation_config, plan).await;

        match validation_result {
            Ok(result) if result.needs_clarification => {
                tracing::info!(
                    "Plan requires clarification for ticket {}: {}",
                    self.ticket.id,
                    result.reason
                );

                let clarification_message =
                    generate_clarification_message(&validation_config, plan)
                        .await
                        .unwrap_or_else(|e| {
                            tracing::warn!("Failed to generate clarification message: {}", e);
                            format!("Clarification needed: {}", result.reason)
                        });

                self.add_clarification_comment(&clarification_message);
                self.move_ticket_to_column("Blocked");

                return Err(format!(
                    "Plan requires user clarification: {}",
                    result.reason
                ));
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

        Ok(())
    }

    /// Run the implement stage.
    async fn run_implement_stage(&self, plan: &str) -> Result<(), String> {
        self.run_implement_stage_capturing(plan).await?;
        Ok(())
    }

    /// Run the implement stage and return the extracted output text.
    async fn run_implement_stage_capturing(&self, plan: &str) -> Result<String, String> {
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

        let implement_prompt = if let Some(ref task) = self.task {
            if matches!(task.task_type, TaskType::Command(_)) {
                let custom_dir = self.custom_commands_dir();
                generate_task_prompt(task, &self.ticket, custom_dir.as_deref())
            } else {
                generate_task_implement_prompt(task, &self.ticket, plan)
            }
        } else {
            generate_implement_prompt(&self.ticket, plan)
        };

        let impl_result = self.run_stage("implement", &implement_prompt).await?;
        let raw_output = impl_result.captured_stdout.unwrap_or_default();
        let text = self.extract_text(&raw_output);
        Ok(text)
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
            self.run_stage(
                cmd,
                &generate_command_prompt(cmd, custom_dir.as_deref()),
            )
            .await?;
        }
        Ok(())
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
        self.run_stage(
            cmd,
            &generate_command_prompt(cmd, custom_dir.as_deref()),
        )
        .await?;
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
