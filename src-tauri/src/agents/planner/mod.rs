//! Planner agent for exploring codebases and generating work plans.
//!
//! The planner agent works in phases:
//! 1. **Exploration**: Uses an AI agent to analyze the codebase structure
//! 2. **Planning**: Generates a structured work plan with epics and tickets
//! 3. **Execution**: Creates the epics and tickets in the database with proper dependencies
//!
//! This module is split into focused submodules:
//! - `config`: Configuration types and error definitions
//! - `prompts`: Prompt templates for exploration and planning
//! - `parsing`: JSON parsing utilities for agent output
//! - `dependencies`: Topological sorting and execution phase calculation
//! - `markdown`: Plan markdown generation for display

use std::sync::Arc;
use tokio::sync::broadcast;

use crate::api::state::LiveEvent;
use crate::db::{
    AgentPref, CreateTicket, Database, Exploration, Priority, ProjectPlan, Spec, SpecVersion,
    SpecVersionStatus, WorkflowType,
};

use super::spawner;
use super::{extract_agent_text, AgentRunConfig};

// Submodules
mod config;
mod dependencies;
mod markdown;
mod parsing;
mod prompts;

// Public re-exports
pub use config::{PlannerConfig, PlannerConfigWithEvents, PlannerError, PlannerResult};
pub use dependencies::{calculate_execution_phases, topological_sort_epics};
pub use markdown::generate_plan_markdown;
pub use parsing::{extract_json_code_block, parse_project_plan};
pub use prompts::{format_plan_overview, generate_exploration_prompt, generate_planning_prompt};

/// The planner agent
pub struct PlannerAgent {
    db: Arc<Database>,
    config: PlannerConfig,
    event_tx: Option<broadcast::Sender<LiveEvent>>,
}

impl PlannerAgent {
    pub fn new(db: Arc<Database>, config: PlannerConfig) -> Self {
        Self {
            db,
            config,
            event_tx: None,
        }
    }

    pub fn with_events(
        db: Arc<Database>,
        config: PlannerConfig,
        event_tx: broadcast::Sender<LiveEvent>,
    ) -> Self {
        Self {
            db,
            config,
            event_tx: Some(event_tx),
        }
    }

    /// Broadcast an event if we have an event sender
    fn broadcast(&self, event: LiveEvent) {
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(event);
        }
    }

    /// Run the full planner workflow: explore -> plan -> (optionally) execute
    pub async fn run(&self) -> Result<PlannerResult, PlannerError> {
        // Get spec and its latest version
        let spec = self
            .db
            .get_spec(&self.config.spec_id)
            .map_err(|e| PlannerError::Database(e.to_string()))?;

        let version = self
            .db
            .get_latest_spec_version(&self.config.spec_id)
            .map_err(|e| PlannerError::Database(e.to_string()))?
            .ok_or_else(|| PlannerError::SpecNotFound("No version found".to_string()))?;

        tracing::info!(
            "Starting planner for spec {} version {}: {:?}",
            spec.id,
            version.id,
            version.status
        );

        // Run exploration and planning with error recovery
        match self.run_explore_and_plan(&spec, &version).await {
            Ok(exploration_result) => {
                // Generate plan using exploration context
                if let Err(e) = self.generate_plan(&spec, &version, &exploration_result).await {
                    // Set status to failed so UI stops showing spinner
                    tracing::error!("Plan generation failed, setting status to failed: {}", e);
                    let _ = self
                        .db
                        .set_spec_version_status(&version.id, SpecVersionStatus::Failed);
                    self.broadcast(LiveEvent::SpecUpdated {
                        spec_id: spec.id.clone(),
                    });
                    return Err(e);
                }
            }
            Err(e) => {
                // Set status to failed so UI stops showing spinner
                tracing::error!("Exploration failed, setting status to failed: {}", e);
                let _ = self
                    .db
                    .set_spec_version_status(&version.id, SpecVersionStatus::Failed);
                self.broadcast(LiveEvent::SpecUpdated {
                    spec_id: spec.id.clone(),
                });
                return Err(e);
            }
        }

        // Check if auto-approve is enabled
        if self.config.auto_approve {
            self.db
                .set_spec_version_status(&version.id, SpecVersionStatus::Approved)
                .map_err(|e| PlannerError::Database(e.to_string()))?;

            self.broadcast(LiveEvent::PlanApproved {
                spec_id: spec.id.clone(),
            });

            // Execute the plan
            return self.execute_plan().await;
        }

        // Return awaiting approval
        Ok(PlannerResult {
            spec_id: spec.id,
            version_id: version.id,
            status: SpecVersionStatus::AwaitingApproval,
            epic_ids: vec![],
            ticket_ids: vec![],
        })
    }

    /// Run plan generation only, skipping exploration (for use after conversational spec discovery)
    /// The exploration_context should contain the technical notes gathered during conversation
    pub async fn run_plan_only(
        &self,
        exploration_context: &str,
    ) -> Result<PlannerResult, PlannerError> {
        // Get spec and its latest version
        let spec = self
            .db
            .get_spec(&self.config.spec_id)
            .map_err(|e| PlannerError::Database(e.to_string()))?;

        let version = self
            .db
            .get_latest_spec_version(&self.config.spec_id)
            .map_err(|e| PlannerError::Database(e.to_string()))?
            .ok_or_else(|| PlannerError::SpecNotFound("No version found".to_string()))?;

        tracing::info!(
            "Running plan-only for spec {} version {} (skipping exploration)",
            spec.id,
            version.id
        );

        // Generate plan using provided exploration context
        match self
            .generate_plan(&spec, &version, exploration_context)
            .await
        {
            Ok(()) => {}
            Err(e) => {
                tracing::error!("Plan generation failed, setting status to failed: {}", e);
                let _ = self
                    .db
                    .set_spec_version_status(&version.id, SpecVersionStatus::Failed);
                self.broadcast(LiveEvent::SpecUpdated {
                    spec_id: spec.id.clone(),
                });
                return Err(e);
            }
        }

        // Check if auto-approve is enabled
        if self.config.auto_approve {
            self.db
                .set_spec_version_status(&version.id, SpecVersionStatus::Approved)
                .map_err(|e| PlannerError::Database(e.to_string()))?;

            self.broadcast(LiveEvent::PlanApproved {
                spec_id: spec.id.clone(),
            });

            // Execute the plan
            return self.execute_plan().await;
        }

        // Return awaiting approval
        Ok(PlannerResult {
            spec_id: spec.id,
            version_id: version.id,
            status: SpecVersionStatus::AwaitingApproval,
            epic_ids: vec![],
            ticket_ids: vec![],
        })
    }

    /// Run the exploration phase, returning the exploration result
    async fn run_explore_and_plan(
        &self,
        spec: &Spec,
        version: &SpecVersion,
    ) -> Result<String, PlannerError> {
        self.run_exploration(spec, version).await
    }

    /// Run an agent with the given prompt (with retry support)
    async fn run_agent(
        &self,
        prompt: &str,
        spec: &Spec,
        phase: &str,
    ) -> Result<String, PlannerError> {
        let max_attempts = self.config.max_retries + 1;
        let mut last_error = String::new();

        for attempt in 1..=max_attempts {
            if attempt > 1 {
                let backoff_secs = 3 * attempt as u64;
                tracing::warn!(
                    "Planner {} retry {}/{} after {}s backoff",
                    phase,
                    attempt,
                    max_attempts,
                    backoff_secs
                );
                tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)).await;
            }

            match self
                .run_agent_attempt(prompt, spec, phase, attempt, max_attempts)
                .await
            {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = e.to_string();
                    if attempt < max_attempts {
                        tracing::warn!(
                            "Planner {} failed (attempt {}/{}): {}",
                            phase,
                            attempt,
                            max_attempts,
                            e
                        );
                        continue;
                    }
                }
            }
        }

        Err(PlannerError::ExplorationFailed(format!(
            "{} (after {} attempts)",
            last_error, max_attempts
        )))
    }

    /// Run a single attempt of an agent call
    async fn run_agent_attempt(
        &self,
        prompt: &str,
        spec: &Spec,
        phase: &str,
        attempt: u32,
        max_attempts: u32,
    ) -> Result<String, PlannerError> {
        let config = AgentRunConfig {
            kind: self.config.agent_kind,
            ticket_id: spec.id.clone(),
            run_id: format!("planner-{}", uuid::Uuid::new_v4()),
            repo_path: self.config.repo_path.clone(),
            prompt: prompt.to_string(),
            timeout_secs: Some(self.config.timeout_secs),
            api_url: self.config.api_url.clone(),
            api_token: self.config.api_token.clone(),
            model: self.config.model.clone(),
            claude_api_config: self.config.claude_api_config.clone(),
        };

        tracing::info!(
            "Running {} agent for spec {} (phase: {}, attempt {}/{})",
            self.config.agent_kind.as_str(),
            spec.id,
            phase,
            attempt,
            max_attempts
        );

        // Create a log callback that broadcasts log entries in real-time
        let log_callback: Option<Arc<super::LogCallback>> = if let Some(ref tx) = self.event_tx {
            let tx_clone = tx.clone();
            let spec_id = spec.id.clone();
            let phase_str = phase.to_string();

            Some(Arc::new(Box::new(move |line: super::LogLine| {
                let level = match line.stream {
                    super::LogStream::Stdout => "output",
                    super::LogStream::Stderr => "error",
                };

                let _ = tx_clone.send(LiveEvent::PlannerLogEntry {
                    spec_id: spec_id.clone(),
                    phase: phase_str.clone(),
                    level: level.to_string(),
                    message: line.content,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                });
            })))
        } else {
            None
        };

        // Run the agent in a blocking task to avoid blocking the async runtime
        // This allows SSE events to be processed while the agent is running
        let result = tokio::task::spawn_blocking(move || {
            spawner::run_agent_with_cancel_callback(config, log_callback, None)
        })
        .await
        .map_err(|e| PlannerError::ExplorationFailed(format!("Task join error: {}", e)))?
        .map_err(|e| PlannerError::ExplorationFailed(e.to_string()))?;

        if result.status != super::RunOutcome::Success {
            return Err(PlannerError::ExplorationFailed(format!(
                "Agent exited with status {:?}: {}",
                result.status,
                result.summary.unwrap_or_default()
            )));
        }

        Ok(result.captured_stdout.unwrap_or_default())
    }

    /// Run the exploration phase
    async fn run_exploration(
        &self,
        spec: &Spec,
        version: &SpecVersion,
    ) -> Result<String, PlannerError> {
        // Update status to exploring
        self.db
            .set_spec_version_status(&version.id, SpecVersionStatus::Exploring)
            .map_err(|e| PlannerError::Database(e.to_string()))?;

        self.broadcast(LiveEvent::SpecUpdated {
            spec_id: spec.id.clone(),
        });

        tracing::info!(
            "Starting exploration phase for spec {} version {} (max {} queries)",
            spec.id,
            version.id,
            self.config.max_explorations
        );

        self.broadcast(LiveEvent::ExplorationProgress {
            spec_id: spec.id.clone(),
            query: "Starting codebase exploration...".to_string(),
            status: "running".to_string(),
        });

        // Generate exploration prompt
        let prompt = generate_exploration_prompt(&spec.user_input, 1);

        // Run the agent
        let output = self.run_agent(&prompt, spec, "exploration").await?;

        // Extract text from agent output (handles Claude stream-json format)
        let response = extract_agent_text(&output);

        // Store the exploration result
        let exploration = Exploration {
            query: format!("Analyzing codebase for: {}", spec.user_input),
            response: response.clone(),
            timestamp: chrono::Utc::now(),
        };

        self.db
            .append_spec_version_exploration(&version.id, &exploration)
            .map_err(|e| PlannerError::Database(e.to_string()))?;

        self.broadcast(LiveEvent::ExplorationProgress {
            spec_id: spec.id.clone(),
            query: exploration.query.clone(),
            status: "completed".to_string(),
        });

        tracing::info!(
            "Exploration completed for spec {} version {}, response length: {} chars",
            spec.id,
            version.id,
            response.len()
        );

        Ok(response)
    }

    /// Generate a structured plan based on exploration results
    async fn generate_plan(
        &self,
        spec: &Spec,
        version: &SpecVersion,
        exploration_context: &str,
    ) -> Result<(), PlannerError> {
        // Update status to planning
        self.db
            .set_spec_version_status(&version.id, SpecVersionStatus::Planning)
            .map_err(|e| PlannerError::Database(e.to_string()))?;

        self.broadcast(LiveEvent::SpecUpdated {
            spec_id: spec.id.clone(),
        });

        tracing::info!(
            "Generating plan for spec {} version {}",
            spec.id,
            version.id
        );

        // Generate planning prompt
        let prompt = generate_planning_prompt(&spec.user_input, exploration_context);

        // Run the agent to generate plan
        let output = self
            .run_agent(&prompt, spec, "planning")
            .await
            .map_err(|e| PlannerError::PlanGenerationFailed(e.to_string()))?;

        // Extract text from agent output
        let text = extract_agent_text(&output);

        // Parse the JSON plan from output
        let plan: ProjectPlan =
            parse_project_plan(&text).map_err(PlannerError::PlanGenerationFailed)?;

        tracing::info!(
            "Plan parsed successfully: {} epics, {} total tickets",
            plan.epics.len(),
            plan.epics.iter().map(|e| e.tickets.len()).sum::<usize>()
        );

        // Generate markdown for display
        let markdown = generate_plan_markdown(&plan);
        let plan_json = serde_json::to_value(&plan)?;

        // Save the plan
        self.db
            .set_spec_version_plan(&version.id, &markdown, Some(&plan_json))
            .map_err(|e| PlannerError::Database(e.to_string()))?;

        // Update status to awaiting approval
        self.db
            .set_spec_version_status(&version.id, SpecVersionStatus::AwaitingApproval)
            .map_err(|e| PlannerError::Database(e.to_string()))?;

        self.broadcast(LiveEvent::PlanGenerated {
            spec_id: spec.id.clone(),
        });

        self.broadcast(LiveEvent::SpecUpdated {
            spec_id: spec.id.clone(),
        });

        Ok(())
    }

    /// Execute an approved plan by creating epics and tickets
    pub async fn execute_plan(&self) -> Result<PlannerResult, PlannerError> {
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
        match self.execute_plan_inner(&spec, &version).await {
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

    /// Inner implementation of execute_plan for error recovery
    async fn execute_plan_inner(
        &self,
        spec: &Spec,
        version: &SpecVersion,
    ) -> Result<PlannerResult, PlannerError> {
        // Get the plan JSON from the version
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

        let mut epic_ids = Vec::new();
        let mut ticket_ids = Vec::new();

        // First pass: topologically sort epics so dependencies are created before dependents
        // Build title -> index map and validate all dependencies exist
        let epic_titles: std::collections::HashSet<_> =
            plan.epics.iter().map(|e| e.title.clone()).collect();

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

        let mut epic_title_to_id = std::collections::HashMap::new();

        // Convert spec's agent_pref string to AgentPref enum
        let agent_pref = spec.agent_pref.as_ref().and_then(|s| AgentPref::parse(s));

        // Create epics and their child tickets in dependency order
        for plan_epic in sorted_epics {
            // Resolve dependencies - use the first dependency for the database FK (for execution logic)
            // Store all dependencies in the JSON array for display purposes
            // All dependencies are guaranteed to exist since we sorted topologically
            let depends_on_epic_id = plan_epic
                .depends_on
                .first()
                .and_then(|dep_title| epic_title_to_id.get(dep_title).cloned());

            // Build list of all dependency IDs for storage
            let depends_on_epic_ids: Vec<String> = plan_epic
                .depends_on
                .iter()
                .filter_map(|dep_title| epic_title_to_id.get(dep_title).cloned())
                .collect();

            // Create the epic
            let epic = self
                .db
                .create_ticket(&CreateTicket {
                    board_id: target_board_id.clone(),
                    column_id: backlog_column.id.clone(),
                    title: plan_epic.title.clone(),
                    description_md: plan_epic.description.clone(),
                    priority: Priority::Medium,
                    labels: vec!["plan-generated".to_string()],
                    project_id: Some(spec.project_id.clone()),
                    agent_pref: agent_pref.clone(),
                    workflow_type: WorkflowType::MultiStage,
                    model: spec.model.clone(),
                    branch_name: None,
                    is_epic: true,
                    epic_id: None,
                    depends_on_epic_id,
                    depends_on_epic_ids,
                    spec_version_id: Some(version.id.clone()),
                })
                .map_err(|e| PlannerError::Database(e.to_string()))?;

            epic_title_to_id.insert(plan_epic.title.clone(), epic.id.clone());
            epic_ids.push(epic.id.clone());

            // Create child tickets
            for plan_ticket in &plan_epic.tickets {
                let mut description = plan_ticket.description.clone();

                // Add acceptance criteria if present
                if let Some(ref criteria) = plan_ticket.acceptance_criteria {
                    description.push_str("\n\n## Acceptance Criteria\n");
                    for c in criteria {
                        description.push_str(&format!("- [ ] {}\n", c));
                    }
                }

                let ticket = self
                    .db
                    .create_ticket(&CreateTicket {
                        board_id: target_board_id.clone(),
                        column_id: backlog_column.id.clone(),
                        title: plan_ticket.title.clone(),
                        description_md: description,
                        priority: Priority::Medium,
                        labels: vec!["plan-generated".to_string()],
                        project_id: Some(spec.project_id.clone()),
                        agent_pref: agent_pref.clone(),
                        workflow_type: WorkflowType::MultiStage,
                        model: spec.model.clone(),
                        branch_name: None,
                        is_epic: false,
                        epic_id: Some(epic.id.clone()),
                        depends_on_epic_id: None,
                        depends_on_epic_ids: vec![],
                        spec_version_id: Some(version.id.clone()),
                    })
                    .map_err(|e| PlannerError::Database(e.to_string()))?;

                ticket_ids.push(ticket.id);
            }
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
            "Plan execution completed for spec {} version {}: {} epics, {} tickets created. Ready to start work.",
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
}
