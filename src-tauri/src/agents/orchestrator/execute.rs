//! Main workflow execution logic for the orchestrator.

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
    /// Execute the full multi-stage workflow
    pub async fn execute(&self) -> Result<(), String> {
        self.log_workflow_start();

        // Move ticket to "In Progress" when workflow starts
        self.move_ticket_to_column("In Progress");

        // Handle branch creation
        self.handle_branch_creation().await?;

        // Stage 1: Plan
        let plan = self.run_plan_stage().await?;

        // Stage 2: Implement
        self.run_implement_stage(&plan).await?;

        // Code review loop
        if self.should_skip_stage("code-review") {
            tracing::info!("Skipping code-review loop (resuming from later stage)");
        } else if !self.is_stage_enabled("code-review") {
            tracing::info!("Skipping code-review loop (disabled in workflow settings)");
        } else {
            self.run_code_review_loop().await?;
        }

        // Move ticket to "Review" when entering QA phase
        self.move_ticket_to_column("Review");

        // Stage 3+: QA Commands
        self.run_qa_stages().await?;

        // Move ticket to "Done" when workflow completes successfully
        self.move_ticket_to_column("Done");

        // Add workflow completion summary comment
        self.add_workflow_summary_comment();

        tracing::info!(
            "Multi-stage workflow completed for ticket {}",
            self.ticket.id
        );
        Ok(())
    }

    /// Log the workflow start, handling resumption if applicable.
    fn log_workflow_start(&self) {
        if let Some(ref resume_stage) = self.resume_from_stage {
            tracing::info!(
                "Resuming multi-stage workflow for ticket {} from stage '{}'",
                self.ticket.id,
                resume_stage
            );
            // Clear the paused_at_stage from the database now that we're resuming
            if let Err(e) = self.db.clear_ticket_pause(&self.ticket.id) {
                tracing::warn!("Failed to clear ticket pause state: {}", e);
                // Continue anyway - the important thing is we have the resume stage
            }
        } else {
            tracing::info!(
                "Starting multi-stage workflow for ticket {}",
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
        // Skip if we're resuming past the plan stage, but retrieve the saved plan
        if self.should_skip_stage("plan") {
            tracing::info!("Skipping plan stage (resuming from later stage)");
            // Try to retrieve the saved plan from previous comments for use in implementation
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

        // Use task-based prompts if we have a task, otherwise fall back to ticket-based
        let plan_prompt = if let Some(ref task) = self.task {
            // For preset tasks, we skip the plan stage and go directly to execution
            if task.task_type != TaskType::Custom {
                // Skip plan for preset tasks - they have their own instructions
                tracing::info!(
                    "Skipping plan stage for preset task type: {:?}",
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
        // Extract only the text content from stream-json output.
        // The raw captured_stdout contains all tool calls, file reads, grep results, etc.
        // which can be 100K+ tokens. We only need the final plan text.
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

        // Run plan validation
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
            api_url: self.api_url.clone(),
            api_token: self.api_token.clone(),
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
        // Skip if we're resuming past the implement stage
        if self.should_skip_stage("implement") {
            tracing::info!("Skipping implement stage (resuming from later stage)");
            return Ok(());
        }

        if self.is_cancelled() {
            return Err("Workflow cancelled".to_string());
        }

        let implement_prompt = if let Some(ref task) = self.task {
            // For preset tasks, use the preset-specific prompt
            if task.task_type != TaskType::Custom {
                generate_task_prompt(task, &self.ticket, &self.repo_path)
            } else {
                generate_task_implement_prompt(task, &self.ticket, plan)
            }
        } else {
            generate_implement_prompt(&self.ticket, plan)
        };

        let _impl_result = self.run_stage("implement", &implement_prompt).await?;

        Ok(())
    }

    /// Run the QA command stages.
    async fn run_qa_stages(&self) -> Result<(), String> {
        let qa_commands = &[
            "deslop",
            "cleanup",
            "unit-tests",
            "cleanup-post-tests",
            "review-changes",
            "cleanup-post-review",
            "review-changes-final",
            "add-and-commit",
        ];

        for cmd in qa_commands {
            // Skip commands that come before the resume point
            if self.should_skip_stage(cmd) {
                tracing::info!("Skipping '{}' stage (resuming from later stage)", cmd);
                continue;
            }

            // Skip commands that are disabled in workflow settings
            if !self.is_stage_enabled(cmd) {
                tracing::info!("Skipping '{}' stage (disabled in workflow settings)", cmd);
                continue;
            }

            if self.is_cancelled() {
                return Err("Workflow cancelled".to_string());
            }
            self.run_stage(cmd, &generate_command_prompt(cmd, &self.repo_path))
                .await?;
        }

        Ok(())
    }
}
