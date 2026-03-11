//! Plan validation and auto-clarification logic for the orchestrator.

use super::WorkflowOrchestrator;
use crate::agents::plan_validation::{
    auto_resolve_clarification, generate_clarification_message, validate_plan_for_clarification,
    AutoClarificationAction, PlanValidationConfig,
};
use crate::db::{TaskStatus, UpdateTask};

impl WorkflowOrchestrator {
    /// Validate the plan and handle clarification if needed.
    pub(super) async fn validate_and_process_plan(&self, plan: &str) -> Result<(), String> {
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

                if self.auto_clarification {
                    if let Some(outcome) = self
                        .try_auto_resolve_clarification(&validation_config, plan, &result.reason)
                        .await
                    {
                        return outcome;
                    }
                }

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

    /// Attempt to auto-resolve a clarification without user input.
    ///
    /// Returns `Some(Ok(()))` if the task was updated (workflow continues),
    /// `Some(Err(...))` if the task was deleted (workflow stops for this task),
    /// or `None` if the agent could not resolve (caller falls through to blocking).
    async fn try_auto_resolve_clarification(
        &self,
        validation_config: &PlanValidationConfig,
        plan: &str,
        reason: &str,
    ) -> Option<Result<(), String>> {
        tracing::info!(
            "Auto-clarification enabled, attempting autonomous resolution for ticket {}",
            self.ticket.id,
        );

        let ticket_description = &self.ticket.description_md;
        let current_task = self.get_task();
        let task_content = current_task
            .as_ref()
            .and_then(|t| t.content.as_deref())
            .unwrap_or("");

        let completed_summaries = self.build_completed_task_summaries();

        let auto_result = auto_resolve_clarification(
            validation_config,
            plan,
            reason,
            ticket_description,
            task_content,
            &completed_summaries,
        )
        .await;

        match auto_result {
            Ok(resolution) => match resolution.action {
                AutoClarificationAction::UpdateTask { updated_content } => {
                    let current_task = self.get_task();
                    let Some(ref task) = current_task else {
                        tracing::warn!(
                            "Auto-clarification: UpdateTask but self.task is None for ticket {}",
                            self.ticket.id,
                        );
                        return None;
                    };
                    if let Err(e) = self.db.update_task(
                        &task.id,
                        &UpdateTask {
                            content: Some(updated_content),
                            title: None,
                            status: None,
                            run_id: None,
                        },
                    ) {
                        tracing::warn!(
                            "Auto-clarification: failed to update task {}: {}",
                            task.id,
                            e,
                        );
                        return None;
                    }
                    self.refresh_task_from_db();
                    self.add_auto_clarification_comment("Task updated", &resolution.reason);
                    tracing::info!(
                        "Auto-clarification resolved (update_task) for ticket {}",
                        self.ticket.id,
                    );
                    Some(Ok(()))
                }
                AutoClarificationAction::DeleteTask => {
                    let current_task = self.get_task();
                    let Some(ref task) = current_task else {
                        tracing::warn!(
                            "Auto-clarification: DeleteTask but self.task is None for ticket {}",
                            self.ticket.id,
                        );
                        return None;
                    };
                    if let Err(e) = self.db.delete_task(&task.id) {
                        tracing::warn!(
                            "Auto-clarification: failed to delete task {}: {}",
                            task.id,
                            e,
                        );
                        return None;
                    }
                    self.add_auto_clarification_comment("Task deleted", &resolution.reason);
                    self.move_ticket_to_column("Ready");
                    tracing::info!(
                        "Auto-clarification resolved (delete_task) for ticket {}, moved to Ready",
                        self.ticket.id,
                    );
                    Some(Err(format!(
                        "Task deleted by auto-clarification: {}",
                        resolution.reason,
                    )))
                }
                AutoClarificationAction::CannotResolve => {
                    tracing::info!(
                        "Auto-clarification could not resolve for ticket {}: {}",
                        self.ticket.id,
                        resolution.reason,
                    );
                    None
                }
            },
            Err(e) => {
                tracing::warn!(
                    "Auto-clarification failed for ticket {}, falling back to blocking: {}",
                    self.ticket.id,
                    e,
                );
                None
            }
        }
    }

    /// Build a summary of completed tasks for context in auto-clarification prompts.
    fn build_completed_task_summaries(&self) -> String {
        self.db
            .get_tasks_for_ticket(&self.ticket.id)
            .ok()
            .map(|tasks| {
                tasks
                    .iter()
                    .filter(|t| t.status == TaskStatus::Completed)
                    .map(|t| {
                        format!(
                            "- [{}] {}",
                            t.title.as_deref().unwrap_or("untitled"),
                            t.content
                                .as_deref()
                                .unwrap_or("")
                                .chars()
                                .take(200)
                                .collect::<String>()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default()
    }
}
