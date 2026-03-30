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
use crate::db::{CreateRun, Database, Exploration, ProjectPlan, RunStatus, Spec, SpecVersion, SpecVersionStatus};

use super::spawner;
use super::AgentRunConfig;

// Submodules
mod config;
mod dependencies;
mod execution;
mod markdown;
mod parsing;
mod prompts;

// Public re-exports
pub use config::{PlannerConfig, PlannerError, PlannerResult};
pub use dependencies::{calculate_execution_phases, topological_sort_epics};
pub use markdown::generate_plan_markdown;
pub use parsing::parse_project_plan;
pub use prompts::{format_plan_overview, generate_exploration_prompt, generate_planning_prompt};

/// The planner agent
pub struct PlannerAgent {
    db: Arc<Database>,
    config: PlannerConfig,
    event_tx: Option<broadcast::Sender<LiveEvent>>,
    parent_run_id: Option<String>,
}

impl PlannerAgent {
    pub fn new(db: Arc<Database>, config: PlannerConfig) -> Self {
        Self {
            db,
            config,
            event_tx: None,
            parent_run_id: None,
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
            parent_run_id: None,
        }
    }

    /// Broadcast an event if we have an event sender
    fn broadcast(&self, event: LiveEvent) {
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(event);
        }
    }

    /// Create a parent run record for this planner invocation.
    fn create_parent_run(&mut self) -> Option<String> {
        match self.db.create_run(&CreateRun {
            ticket_id: self.config.spec_id.clone(),
            agent_type: self.config.agent_id.clone(),
            repo_path: self.config.repo_path.to_string_lossy().to_string(),
            parent_run_id: None,
            stage: Some("planner".to_string()),
            ..Default::default()
        }) {
            Ok(run) => {
                let _ = self.db.update_run_status(&run.id, RunStatus::Running, None, None);
                self.parent_run_id = Some(run.id.clone());
                Some(run.id)
            }
            Err(e) => {
                tracing::warn!("Failed to create planner parent run: {}", e);
                None
            }
        }
    }

    /// Finalize the parent run with the given status.
    fn finalize_parent_run(&self, status: RunStatus, exit_code: Option<i32>, summary: Option<&str>) {
        if let Some(ref run_id) = self.parent_run_id {
            let _ = self.db.update_run_status(run_id, status, exit_code, summary);
        }
    }

    /// Run the full planner workflow: explore -> plan -> (optionally) execute
    pub async fn run(&mut self) -> Result<PlannerResult, PlannerError> {
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

        self.create_parent_run();

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
                    self.finalize_parent_run(RunStatus::Error, None, Some(&e.to_string()));
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
                self.finalize_parent_run(RunStatus::Error, None, Some(&e.to_string()));
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
            let result = self.execute_plan().await;
            match &result {
                Ok(_) => self.finalize_parent_run(RunStatus::Finished, Some(0), None),
                Err(e) => self.finalize_parent_run(RunStatus::Error, None, Some(&e.to_string())),
            }
            return result;
        }

        self.finalize_parent_run(RunStatus::Finished, Some(0), None);

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
        &mut self,
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

        self.create_parent_run();

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
                self.finalize_parent_run(RunStatus::Error, None, Some(&e.to_string()));
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
            let result = self.execute_plan().await;
            match &result {
                Ok(_) => self.finalize_parent_run(RunStatus::Finished, Some(0), None),
                Err(e) => self.finalize_parent_run(RunStatus::Error, None, Some(&e.to_string())),
            }
            return result;
        }

        self.finalize_parent_run(RunStatus::Finished, Some(0), None);

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
        let run_id = format!("planner-{}", uuid::Uuid::new_v4());

        let sub_run = self.db.create_run(&CreateRun {
            ticket_id: spec.id.clone(),
            agent_type: self.config.agent_id.clone(),
            repo_path: self.config.repo_path.to_string_lossy().to_string(),
            parent_run_id: self.parent_run_id.clone(),
            stage: Some(phase.to_string()),
            ..Default::default()
        });
        let sub_run_id = sub_run.as_ref().ok().map(|r| r.id.clone());
        if let Some(ref id) = sub_run_id {
            let _ = self.db.update_run_status(id, RunStatus::Running, None, None);
        }

        let config = AgentRunConfig {
            agent_id: self.config.agent_id.clone(),
            ticket_id: spec.id.clone(),
            run_id,
            repo_path: self.config.repo_path.clone(),
            prompt: prompt.to_string(),
            timeout_secs: Some(self.config.timeout_secs),
            model: self.config.model.clone(),
            agent_config: self.config.agent_config.clone(),
            session_id: None,
            workspace_file: None,
            workspace_paths: vec![],
            debug_mode: false,
            allow_protected_branch: true,
        };

        tracing::info!(
            "Running {} agent for spec {} (phase: {}, attempt {}/{})",
            self.config.agent_id,
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

        let provider = self.config.provider.clone();
        let provider_for_cost = self.config.provider.clone();
        let agent_config_for_cost = self.config.agent_config.clone();
        let model_for_cost = self.config.model.clone();
        let start_time = std::time::Instant::now();

        let spawn_result = tokio::task::spawn_blocking(move || {
            spawner::run_agent_via_provider_with_cancel(&*provider, &config, log_callback, None)
        })
        .await;

        let result = match spawn_result {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                let msg = e.to_string();
                if let Some(ref id) = sub_run_id {
                    let _ = self.db.update_run_status(id, RunStatus::Error, None, Some(&msg));
                }
                return Err(PlannerError::ExplorationFailed(msg));
            }
            Err(e) => {
                let msg = format!("Task join error: {}", e);
                if let Some(ref id) = sub_run_id {
                    let _ = self.db.update_run_status(id, RunStatus::Error, None, Some(&msg));
                }
                return Err(PlannerError::ExplorationFailed(msg));
            }
        };

        if let Some(ref id) = sub_run_id {
            let duration_secs = start_time.elapsed().as_secs_f64();
            let exit_code = result.exit_code;
            let status = if result.status == super::RunOutcome::Success {
                RunStatus::Finished
            } else {
                RunStatus::Error
            };
            let _ = self.db.update_run_status(id, status, exit_code, result.summary.as_deref());

            let stage_model = model_for_cost.as_deref().unwrap_or("unknown");
            let stdout = result.captured_stdout.as_deref().unwrap_or("");
            let cost_data = crate::agents::provider::extract_cost_with_overrides(
                &*provider_for_cost,
                stdout,
                stage_model,
                &agent_config_for_cost,
                duration_secs,
            );
            let mut metadata = serde_json::json!({
                "duration_secs": duration_secs,
                "stage_model": stage_model,
            });
            if let Some(ref cost) = cost_data {
                metadata["cost"] = serde_json::to_value(cost).unwrap_or_default();
            }
            if let Err(e) = self.db.set_run_metadata(id, &metadata) {
                tracing::warn!("Failed to save planner sub-run metadata: {}", e);
            }
        }

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

        // Extract text from agent output using the provider's parser
        let response = self.config.provider.extract_text(&output);

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

        // Extract text from agent output using the provider's parser
        let text = self.config.provider.extract_text(&output);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::claude::provider::ClaudeProvider;
    use std::path::PathBuf;

    fn make_config(spec_id: &str) -> PlannerConfig {
        PlannerConfig {
            spec_id: spec_id.to_string(),
            max_explorations: 1,
            auto_approve: false,
            model: Some("test-model".to_string()),
            agent_id: "claude".to_string(),
            provider: Arc::new(ClaudeProvider::new()),
            repo_path: PathBuf::from("/tmp"),
            agent_config: std::collections::HashMap::new(),
            timeout_secs: 60,
            max_retries: 0,
        }
    }

    #[test]
    fn create_parent_run_inserts_run_record() {
        let db = Arc::new(crate::db::Database::open_in_memory().unwrap());
        let config = make_config("spec-test-123");
        let mut agent = PlannerAgent::new(db.clone(), config);

        let run_id = agent.create_parent_run();
        assert!(run_id.is_some());
        assert!(agent.parent_run_id.is_some());
        assert_eq!(run_id, agent.parent_run_id);

        let run = db.get_run(run_id.as_ref().unwrap()).unwrap();
        assert_eq!(run.ticket_id, "spec-test-123");
        assert_eq!(run.agent_type, "claude");
        assert_eq!(run.status, RunStatus::Running);
        assert_eq!(run.stage, Some("planner".to_string()));
        assert!(run.parent_run_id.is_none());
    }

    #[test]
    fn finalize_parent_run_updates_status() {
        let db = Arc::new(crate::db::Database::open_in_memory().unwrap());
        let config = make_config("spec-abc");
        let mut agent = PlannerAgent::new(db.clone(), config);

        agent.create_parent_run();
        let run_id = agent.parent_run_id.clone().unwrap();

        agent.finalize_parent_run(RunStatus::Finished, Some(0), None);

        let run = db.get_run(&run_id).unwrap();
        assert_eq!(run.status, RunStatus::Finished);
        assert_eq!(run.exit_code, Some(0));
    }

    #[test]
    fn finalize_parent_run_stores_error_summary() {
        let db = Arc::new(crate::db::Database::open_in_memory().unwrap());
        let config = make_config("spec-err");
        let mut agent = PlannerAgent::new(db.clone(), config);

        agent.create_parent_run();
        let run_id = agent.parent_run_id.clone().unwrap();

        agent.finalize_parent_run(RunStatus::Error, None, Some("exploration timed out"));

        let run = db.get_run(&run_id).unwrap();
        assert_eq!(run.status, RunStatus::Error);
        assert_eq!(run.summary_md, Some("exploration timed out".to_string()));
    }

    #[test]
    fn finalize_parent_run_noop_without_parent_run() {
        let db = Arc::new(crate::db::Database::open_in_memory().unwrap());
        let config = make_config("spec-none");
        let agent = PlannerAgent::new(db, config);

        assert!(agent.parent_run_id.is_none());
        agent.finalize_parent_run(RunStatus::Error, None, Some("should not panic"));
    }

    #[test]
    fn new_initializes_without_parent_run_id() {
        let db = Arc::new(crate::db::Database::open_in_memory().unwrap());
        let agent = PlannerAgent::new(db, make_config("x"));
        assert!(agent.parent_run_id.is_none());
    }

    #[test]
    fn with_events_initializes_without_parent_run_id() {
        let db = Arc::new(crate::db::Database::open_in_memory().unwrap());
        let (tx, _) = broadcast::channel(1);
        let agent = PlannerAgent::with_events(db, make_config("x"), tx);
        assert!(agent.parent_run_id.is_none());
    }
}
