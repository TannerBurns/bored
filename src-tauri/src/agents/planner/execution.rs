//! Plan execution logic - creates epics and tickets from approved plans.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tokio::sync::broadcast;

use crate::api::state::LiveEvent;
use crate::db::{
    CreateTask, CreateTicket, Database, Priority, ProjectPlan, Spec, SpecVersion,
    SpecVersionStatus, WorkflowType,
};

use super::config::{PlannerConfig, PlannerError, PlannerResult};
use super::dependencies::topological_sort_epics;

/// Context for creating an epic and its child tickets
struct EpicCreationContext<'a> {
    plan_epic: &'a crate::db::PlanEpic,
    epic_title_to_id: &'a HashMap<String, String>,
    board_id: &'a str,
    column_id: &'a str,
    project_id: &'a str,
    version_id: &'a str,
    model: Option<String>,
}

/// Executes an approved plan by creating epics and tickets in the database.
pub struct PlanExecutor {
    db: Arc<Database>,
    config: PlannerConfig,
    event_tx: Option<broadcast::Sender<LiveEvent>>,
}

impl PlanExecutor {
    pub fn new(
        db: Arc<Database>,
        config: PlannerConfig,
        event_tx: Option<broadcast::Sender<LiveEvent>>,
    ) -> Self {
        Self { db, config, event_tx }
    }

    fn broadcast(&self, event: LiveEvent) {
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(event);
        }
    }

    /// Execute an approved plan by creating epics and tickets
    pub async fn execute(&self) -> Result<PlannerResult, PlannerError> {
        let spec = self
            .db
            .get_spec(&self.config.spec_id)
            .map_err(|e| PlannerError::Database(e.to_string()))?;

        let version = self
            .db
            .get_latest_spec_version(&self.config.spec_id)
            .map_err(|e| PlannerError::Database(e.to_string()))?
            .ok_or_else(|| PlannerError::SpecNotFound("No version found".to_string()))?;

        // Verify status is approved (or stuck in executing from a previous failed attempt)
        if version.status != SpecVersionStatus::Approved
            && version.status != SpecVersionStatus::Executing
        {
            return Err(PlannerError::InvalidState(format!(
                "Cannot execute plan: version status is {:?}, expected Approved",
                version.status
            )));
        }

        // Update status to executing (if not already)
        if version.status != SpecVersionStatus::Executing {
            self.db
                .set_spec_version_status(&version.id, SpecVersionStatus::Executing)
                .map_err(|e| PlannerError::Database(e.to_string()))?;
        }

        self.broadcast(LiveEvent::PlanExecutionStarted {
            spec_id: spec.id.clone(),
        });

        tracing::info!(
            "Executing plan for spec {} version {}",
            spec.id,
            version.id
        );

        // Execute the plan creation with error recovery
        match self.execute_inner(&spec, &version).await {
            Ok(result) => Ok(result),
            Err(e) => {
                // Reset status to approved so user can retry
                tracing::error!("Plan execution failed, resetting status to approved: {}", e);
                let _ = self
                    .db
                    .set_spec_version_status(&version.id, SpecVersionStatus::Approved);
                self.broadcast(LiveEvent::SpecUpdated {
                    spec_id: spec.id.clone(),
                });
                Err(e)
            }
        }
    }

    /// Inner implementation of execute for error recovery
    async fn execute_inner(
        &self,
        spec: &Spec,
        version: &SpecVersion,
    ) -> Result<PlannerResult, PlannerError> {
        let plan_json = version
            .plan_json
            .clone()
            .ok_or_else(|| PlannerError::InvalidState("No plan JSON found".to_string()))?;

        let plan: ProjectPlan = serde_json::from_value(plan_json)
            .map_err(|e| PlannerError::ExecutionFailed(format!("Failed to parse plan: {}", e)))?;

        // Use target_board_id if set, otherwise fall back to board_id
        let target_board_id = spec.target_board_id.as_ref().unwrap_or(&spec.board_id);

        // Get target board's backlog column for creating tickets
        let columns = self
            .db
            .get_columns(target_board_id)
            .map_err(|e| PlannerError::Database(e.to_string()))?;

        let backlog_column = columns
            .iter()
            .find(|c| c.name == "Backlog")
            .ok_or_else(|| {
                PlannerError::ExecutionFailed(
                    "Backlog column not found on target board".to_string(),
                )
            })?;

        // Validate dependencies exist
        let epic_titles: HashSet<_> = plan.epics.iter().map(|e| e.title.clone()).collect();
        for plan_epic in &plan.epics {
            for dep_title in &plan_epic.depends_on {
                if !epic_titles.contains(dep_title) {
                    return Err(PlannerError::ExecutionFailed(format!(
                        "Epic '{}' depends on '{}' which does not exist in the plan",
                        plan_epic.title, dep_title
                    )));
                }
            }
        }

        // Topologically sort: dependencies before dependents
        let sorted_epics =
            topological_sort_epics(&plan.epics).map_err(PlannerError::ExecutionFailed)?;

        let mut epic_title_to_id: HashMap<String, String> = HashMap::new();
        let mut epic_ids = Vec::new();
        let mut ticket_ids = Vec::new();

        // Create epics and their child tickets in dependency order
        for plan_epic in sorted_epics {
            let (epic_id, child_ticket_ids) = self.create_epic_with_tickets(EpicCreationContext {
                plan_epic,
                epic_title_to_id: &epic_title_to_id,
                board_id: target_board_id,
                column_id: &backlog_column.id,
                project_id: &spec.project_id,
                version_id: &version.id,
                model: spec.model.clone(),
            })?;

            epic_title_to_id.insert(plan_epic.title.clone(), epic_id.clone());
            epic_ids.push(epic_id);
            ticket_ids.extend(child_ticket_ids);
        }

        // Update status to executed (ready to start work)
        self.db
            .set_spec_version_status(&version.id, SpecVersionStatus::Executed)
            .map_err(|e| PlannerError::Database(e.to_string()))?;

        self.broadcast(LiveEvent::PlanExecutionCompleted {
            spec_id: spec.id.clone(),
            epic_ids: epic_ids.clone(),
        });

        tracing::info!(
            "Plan execution completed for spec {} version {}: {} epics, {} tickets created",
            spec.id,
            version.id,
            epic_ids.len(),
            ticket_ids.len()
        );

        Ok(PlannerResult {
            spec_id: spec.id.clone(),
            version_id: version.id.clone(),
            status: SpecVersionStatus::Executed,
            epic_ids,
            ticket_ids,
        })
    }

    fn create_epic_with_tickets(
        &self,
        ctx: EpicCreationContext<'_>,
    ) -> Result<(String, Vec<String>), PlannerError> {
        let EpicCreationContext {
            plan_epic,
            epic_title_to_id,
            board_id,
            column_id,
            project_id,
            version_id,
            model,
        } = ctx;

        // Resolve dependencies
        let depends_on_epic_id = plan_epic
            .depends_on
            .first()
            .and_then(|dep_title| epic_title_to_id.get(dep_title).cloned());

        let depends_on_epic_ids: Vec<String> = plan_epic
            .depends_on
            .iter()
            .filter_map(|dep_title| epic_title_to_id.get(dep_title).cloned())
            .collect();

        // Create the epic
        let epic = self
            .db
            .create_ticket(&CreateTicket {
                board_id: board_id.to_string(),
                column_id: column_id.to_string(),
                title: plan_epic.title.clone(),
                description_md: plan_epic.description.clone(),
                priority: Priority::Medium,
                labels: vec!["plan-generated".to_string()],
                project_id: Some(project_id.to_string()),
                workspace_id: None,
                workflow_type: WorkflowType::MultiStage,
                model: model.clone(),
                branch_name: None,
                is_epic: true,
                epic_id: None,
                depends_on_epic_id,
                depends_on_epic_ids,
                spec_version_id: Some(version_id.to_string()),
            })
            .map_err(|e| PlannerError::Database(e.to_string()))?;

        // Create child tickets
        let mut ticket_ids = Vec::new();
        for plan_ticket in &plan_epic.tickets {
            let mut description = plan_ticket.description.clone();

            if let Some(ref criteria) = plan_ticket.acceptance_criteria {
                description.push_str("\n\n## Acceptance Criteria\n");
                for c in criteria {
                    description.push_str(&format!("- [ ] {}\n", c));
                }
            }

            let ticket = self
                .db
                .create_ticket(&CreateTicket {
                    board_id: board_id.to_string(),
                    column_id: column_id.to_string(),
                    title: plan_ticket.title.clone(),
                    description_md: description.clone(),
                    priority: Priority::Medium,
                    labels: vec!["plan-generated".to_string()],
                    project_id: Some(project_id.to_string()),
                    workspace_id: None,
                    workflow_type: WorkflowType::MultiStage,
                    model: model.clone(),
                    branch_name: plan_ticket.branch_name.clone(),
                    is_epic: false,
                    epic_id: Some(epic.id.clone()),
                    depends_on_epic_id: None,
                    depends_on_epic_ids: vec![],
                    spec_version_id: Some(version_id.to_string()),
                })
                .map_err(|e| PlannerError::Database(e.to_string()))?;

            if let Some(tasks) = plan_ticket.tasks.as_ref().filter(|t| !t.is_empty()) {
                for task in tasks {
                    self.db
                        .create_task(&CreateTask {
                            ticket_id: ticket.id.clone(),
                            task_type: Default::default(),
                            title: Some(task.title.clone()),
                            content: task.content.clone(),
                        })
                        .map_err(|e| PlannerError::Database(e.to_string()))?;
                }
            } else {
                tracing::warn!(
                    "Ticket '{}' has no tasks in plan, creating fallback task from description",
                    plan_ticket.title,
                );
                self.db
                    .create_task(&CreateTask {
                        ticket_id: ticket.id.clone(),
                        task_type: Default::default(),
                        title: Some(plan_ticket.title.clone()),
                        content: Some(description.clone()),
                    })
                    .map_err(|e| PlannerError::Database(e.to_string()))?;
            }

            ticket_ids.push(ticket.id);
        }

        Ok((epic.id, ticket_ids))
    }
}
