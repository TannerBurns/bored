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
use crate::db::{Database, Exploration, ProjectPlan, Spec, SpecVersion, SpecVersionStatus};

use super::spawner;
use super::{extract_agent_text, AgentRunConfig};

// Submodules
mod config;
mod dependencies;
mod execution;
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
            agent_config: std::collections::HashMap::new(),
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
        let executor = execution::PlanExecutor::new(
            self.db.clone(),
            self.config.clone(),
            self.event_tx.clone(),
        );
        executor.execute().await
    }
}
