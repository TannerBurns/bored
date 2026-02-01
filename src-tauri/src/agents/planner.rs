//! Planner agent for exploring codebases and generating work plans.
//!
//! The planner agent works in phases:
//! 1. **Exploration**: Uses an AI agent to analyze the codebase structure
//! 2. **Planning**: Generates a structured work plan with epics and tickets
//! 3. **Execution**: Creates the epics and tickets in the database with proper dependencies

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::api::state::LiveEvent;
use crate::db::{
    AgentPref, CreateTicket, Database, Exploration, PlanEpic, Priority, ProjectPlan, Scratchpad,
    ScratchpadStatus, WorkflowType,
};

#[cfg(test)]
use crate::db::PlanTicket;

use super::planner_prompts;
use super::spawner;
use super::{extract_agent_text, AgentKind, AgentRunConfig, ClaudeApiConfig};

/// Configuration for the planner agent
#[derive(Debug, Clone)]
pub struct PlannerConfig {
    pub scratchpad_id: String,
    pub max_explorations: usize,
    pub auto_approve: bool,
    pub model: Option<String>,
    pub agent_kind: AgentKind,
    pub repo_path: PathBuf,
    pub api_url: String,
    pub api_token: String,
    /// Claude API configuration (auth token, api key, base url, model override)
    pub claude_api_config: Option<ClaudeApiConfig>,
}

/// Extended config with event broadcasting
pub struct PlannerConfigWithEvents {
    pub config: PlannerConfig,
    pub event_tx: Option<broadcast::Sender<LiveEvent>>,
}

/// Result of a planner execution
#[derive(Debug)]
pub struct PlannerResult {
    pub scratchpad_id: String,
    pub status: ScratchpadStatus,
    pub epic_ids: Vec<String>,
    pub ticket_ids: Vec<String>,
}

/// Error type for planner operations
#[derive(Debug, thiserror::Error)]
pub enum PlannerError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Scratchpad not found: {0}")]
    ScratchpadNotFound(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Exploration failed: {0}")]
    ExplorationFailed(String),

    #[error("Plan generation failed: {0}")]
    PlanGenerationFailed(String),

    #[error("Plan execution failed: {0}")]
    ExecutionFailed(String),
    
    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),
}

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
        // Get scratchpad
        let scratchpad = self
            .db
            .get_scratchpad(&self.config.scratchpad_id)
            .map_err(|e| PlannerError::Database(e.to_string()))?;

        tracing::info!(
            "Starting planner for scratchpad {}: {:?}",
            scratchpad.id,
            scratchpad.status
        );

        // Run exploration and planning with error recovery
        match self.run_explore_and_plan(&scratchpad).await {
            Ok(exploration_result) => {
                // Generate plan using exploration context
                if let Err(e) = self.generate_plan(&scratchpad, &exploration_result).await {
                    // Set status to failed so UI stops showing spinner
                    tracing::error!("Plan generation failed, setting status to failed: {}", e);
                    let _ = self.db.set_scratchpad_status(&scratchpad.id, ScratchpadStatus::Failed);
                    self.broadcast(LiveEvent::ScratchpadUpdated {
                        scratchpad_id: scratchpad.id.clone(),
                    });
                    return Err(e);
                }
            }
            Err(e) => {
                // Set status to failed so UI stops showing spinner
                tracing::error!("Exploration failed, setting status to failed: {}", e);
                let _ = self.db.set_scratchpad_status(&scratchpad.id, ScratchpadStatus::Failed);
                self.broadcast(LiveEvent::ScratchpadUpdated {
                    scratchpad_id: scratchpad.id.clone(),
                });
                return Err(e);
            }
        }

        // Check if auto-approve is enabled
        if self.config.auto_approve {
            self.db
                .set_scratchpad_status(&scratchpad.id, ScratchpadStatus::Approved)
                .map_err(|e| PlannerError::Database(e.to_string()))?;

            self.broadcast(LiveEvent::PlanApproved {
                scratchpad_id: scratchpad.id.clone(),
            });

            // Execute the plan
            return self.execute_plan().await;
        }

        // Return awaiting approval
        Ok(PlannerResult {
            scratchpad_id: scratchpad.id,
            status: ScratchpadStatus::AwaitingApproval,
            epic_ids: vec![],
            ticket_ids: vec![],
        })
    }

    /// Run the exploration phase, returning the exploration result
    async fn run_explore_and_plan(&self, scratchpad: &Scratchpad) -> Result<String, PlannerError> {
        self.run_exploration(scratchpad).await
    }

    /// Run an agent with the given prompt
    async fn run_agent(&self, prompt: &str, scratchpad: &Scratchpad, phase: &str) -> Result<String, PlannerError> {
        let config = AgentRunConfig {
            kind: self.config.agent_kind,
            ticket_id: scratchpad.id.clone(),
            run_id: format!("planner-{}", uuid::Uuid::new_v4()),
            repo_path: self.config.repo_path.clone(),
            prompt: prompt.to_string(),
            timeout_secs: Some(300), // 5 min timeout for exploration/planning
            api_url: self.config.api_url.clone(),
            api_token: self.config.api_token.clone(),
            model: self.config.model.clone(),
            claude_api_config: self.config.claude_api_config.clone(),
        };

        tracing::info!(
            "Running {} agent for scratchpad {} (phase: {})",
            self.config.agent_kind.as_str(),
            scratchpad.id,
            phase
        );

        // Create a log callback that broadcasts log entries in real-time
        let log_callback: Option<Arc<super::LogCallback>> = if let Some(ref tx) = self.event_tx {
            let tx_clone = tx.clone();
            let scratchpad_id = scratchpad.id.clone();
            let phase_str = phase.to_string();
            
            Some(Arc::new(Box::new(move |line: super::LogLine| {
                let level = match line.stream {
                    super::LogStream::Stdout => "output",
                    super::LogStream::Stderr => "error",
                };
                
                let _ = tx_clone.send(LiveEvent::PlannerLogEntry {
                    scratchpad_id: scratchpad_id.clone(),
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
    async fn run_exploration(&self, scratchpad: &Scratchpad) -> Result<String, PlannerError> {
        // Update status to exploring
        self.db
            .set_scratchpad_status(&scratchpad.id, ScratchpadStatus::Exploring)
            .map_err(|e| PlannerError::Database(e.to_string()))?;

        self.broadcast(LiveEvent::ScratchpadUpdated {
            scratchpad_id: scratchpad.id.clone(),
        });

        tracing::info!(
            "Starting exploration phase for scratchpad {} (max {} queries)",
            scratchpad.id,
            self.config.max_explorations
        );

        self.broadcast(LiveEvent::ExplorationProgress {
            scratchpad_id: scratchpad.id.clone(),
            query: "Starting codebase exploration...".to_string(),
            status: "running".to_string(),
        });

        // Generate exploration prompt
        let prompt = planner_prompts::generate_exploration_prompt(&scratchpad.user_input, 1);

        // Run the agent
        let output = self.run_agent(&prompt, scratchpad, "exploration").await?;

        // Extract text from agent output (handles Claude stream-json format)
        let response = extract_agent_text(&output);

        // Store the exploration result
        let exploration = Exploration {
            query: format!("Analyzing codebase for: {}", scratchpad.user_input),
            response: response.clone(),
            timestamp: chrono::Utc::now(),
        };

        self.db
            .append_exploration(&scratchpad.id, &exploration)
            .map_err(|e| PlannerError::Database(e.to_string()))?;

        self.broadcast(LiveEvent::ExplorationProgress {
            scratchpad_id: scratchpad.id.clone(),
            query: exploration.query.clone(),
            status: "completed".to_string(),
        });

        tracing::info!(
            "Exploration completed for scratchpad {}, response length: {} chars",
            scratchpad.id,
            response.len()
        );

        Ok(response)
    }

    /// Generate a structured plan based on exploration results
    async fn generate_plan(
        &self,
        scratchpad: &Scratchpad,
        exploration_context: &str,
    ) -> Result<(), PlannerError> {
        // Update status to planning
        self.db
            .set_scratchpad_status(&scratchpad.id, ScratchpadStatus::Planning)
            .map_err(|e| PlannerError::Database(e.to_string()))?;

        self.broadcast(LiveEvent::ScratchpadUpdated {
            scratchpad_id: scratchpad.id.clone(),
        });

        tracing::info!("Generating plan for scratchpad {}", scratchpad.id);

        // Generate planning prompt
        let prompt =
            planner_prompts::generate_planning_prompt(&scratchpad.user_input, exploration_context);

        // Run the agent to generate plan
        let output = self.run_agent(&prompt, scratchpad, "planning").await
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
            .set_scratchpad_plan(&scratchpad.id, &markdown, Some(&plan_json))
            .map_err(|e| PlannerError::Database(e.to_string()))?;

        // Update status to awaiting approval
        self.db
            .set_scratchpad_status(&scratchpad.id, ScratchpadStatus::AwaitingApproval)
            .map_err(|e| PlannerError::Database(e.to_string()))?;

        self.broadcast(LiveEvent::PlanGenerated {
            scratchpad_id: scratchpad.id.clone(),
        });

        self.broadcast(LiveEvent::ScratchpadUpdated {
            scratchpad_id: scratchpad.id.clone(),
        });

        Ok(())
    }

    /// Execute an approved plan by creating epics and tickets
    pub async fn execute_plan(&self) -> Result<PlannerResult, PlannerError> {
        let scratchpad = self
            .db
            .get_scratchpad(&self.config.scratchpad_id)
            .map_err(|e| PlannerError::Database(e.to_string()))?;

        // Verify status is approved (or stuck in executing from a previous failed attempt)
        if scratchpad.status != ScratchpadStatus::Approved && scratchpad.status != ScratchpadStatus::Executing {
            return Err(PlannerError::InvalidState(format!(
                "Cannot execute plan: status is {:?}, expected Approved",
                scratchpad.status
            )));
        }

        // Update status to executing (if not already)
        if scratchpad.status != ScratchpadStatus::Executing {
            self.db
                .set_scratchpad_status(&scratchpad.id, ScratchpadStatus::Executing)
                .map_err(|e| PlannerError::Database(e.to_string()))?;
        }

        self.broadcast(LiveEvent::PlanExecutionStarted {
            scratchpad_id: scratchpad.id.clone(),
        });

        tracing::info!("Executing plan for scratchpad {}", scratchpad.id);
        
        // Execute the plan creation with error recovery
        match self.execute_plan_inner(&scratchpad).await {
            Ok(result) => Ok(result),
            Err(e) => {
                // Reset status to approved so user can retry
                tracing::error!("Plan execution failed, resetting status to approved: {}", e);
                let _ = self.db.set_scratchpad_status(&scratchpad.id, ScratchpadStatus::Approved);
                self.broadcast(LiveEvent::ScratchpadUpdated {
                    scratchpad_id: scratchpad.id.clone(),
                });
                Err(e)
            }
        }
    }
    
    /// Inner implementation of execute_plan for error recovery
    async fn execute_plan_inner(&self, scratchpad: &Scratchpad) -> Result<PlannerResult, PlannerError> {
        // Get the plan JSON
        let plan_json = scratchpad
            .plan_json
            .clone()
            .ok_or_else(|| PlannerError::InvalidState("No plan JSON found".to_string()))?;

        let plan: ProjectPlan = serde_json::from_value(plan_json)
            .map_err(|e| PlannerError::ExecutionFailed(format!("Failed to parse plan: {}", e)))?;

        // Use target_board_id if set, otherwise fall back to board_id
        let target_board_id = scratchpad.target_board_id.as_ref()
            .unwrap_or(&scratchpad.board_id);
        
        // Get target board's backlog column for creating tickets
        let columns = self
            .db
            .get_columns(target_board_id)
            .map_err(|e| PlannerError::Database(e.to_string()))?;

        let backlog_column = columns
            .iter()
            .find(|c| c.name == "Backlog")
            .ok_or_else(|| PlannerError::ExecutionFailed("Backlog column not found on target board".to_string()))?;

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

        // Convert scratchpad's agent_pref string to AgentPref enum
        let agent_pref = scratchpad
            .agent_pref
            .as_ref()
            .and_then(|s| AgentPref::parse(s));

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
                    project_id: Some(scratchpad.project_id.clone()),
                    agent_pref: agent_pref.clone(),
                    workflow_type: WorkflowType::MultiStage,
                    model: scratchpad.model.clone(),
                    branch_name: None,
                    is_epic: true,
                    epic_id: None,
                    depends_on_epic_id,
                    depends_on_epic_ids,
                    scratchpad_id: Some(scratchpad.id.clone()),
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
                        project_id: Some(scratchpad.project_id.clone()),
                        agent_pref: agent_pref.clone(),
                        workflow_type: WorkflowType::MultiStage,
                        model: scratchpad.model.clone(),
                        branch_name: None,
                        is_epic: false,
                        epic_id: Some(epic.id.clone()),
                        depends_on_epic_id: None,
                        depends_on_epic_ids: vec![],
                        scratchpad_id: Some(scratchpad.id.clone()),
                    })
                    .map_err(|e| PlannerError::Database(e.to_string()))?;

                ticket_ids.push(ticket.id);
            }
        }

        // Update status to executed (ready to start work)
        self.db
            .set_scratchpad_status(&scratchpad.id, ScratchpadStatus::Executed)
            .map_err(|e| PlannerError::Database(e.to_string()))?;

        self.broadcast(LiveEvent::PlanExecutionCompleted {
            scratchpad_id: scratchpad.id.clone(),
            epic_ids: epic_ids.clone(),
        });

        tracing::info!(
            "Plan execution completed: {} epics, {} tickets created. Ready to start work.",
            epic_ids.len(),
            ticket_ids.len()
        );

        Ok(PlannerResult {
            scratchpad_id: scratchpad.id.clone(),
            status: ScratchpadStatus::Executed,
            epic_ids,
            ticket_ids,
        })
    }
}

/// Parse a ProjectPlan from agent output.
/// Handles cases where the JSON is embedded in other text.
pub fn parse_project_plan(output: &str) -> Result<ProjectPlan, String> {
    let trimmed = output.trim();
    
    // Try direct parse first
    if let Ok(plan) = serde_json::from_str::<ProjectPlan>(trimmed) {
        return Ok(plan);
    }

    // Try to find JSON code block (```json ... ```)
    if let Some(json_str) = extract_json_code_block(trimmed) {
        if let Ok(plan) = serde_json::from_str::<ProjectPlan>(&json_str) {
            return Ok(plan);
        }
    }

    // Find JSON object in text (handles preamble/postamble)
    let start = trimmed.find('{').ok_or("No JSON object found in output")?;
    let end = trimmed.rfind('}').ok_or("No closing brace found")?;

    if end > start {
        let json_str = &trimmed[start..=end];
        serde_json::from_str(json_str).map_err(|e| format!("JSON parse error: {}", e))
    } else {
        Err("Invalid JSON structure".to_string())
    }
}

/// Extract JSON from a markdown code block if present
fn extract_json_code_block(text: &str) -> Option<String> {
    // Look for ```json ... ``` pattern
    let start_pattern = "```json";
    let end_pattern = "```";
    
    if let Some(start_idx) = text.find(start_pattern) {
        let content_start = start_idx + start_pattern.len();
        if let Some(end_idx) = text[content_start..].find(end_pattern) {
            let json_content = &text[content_start..content_start + end_idx];
            return Some(json_content.trim().to_string());
        }
    }
    
    // Also try plain ``` blocks that contain JSON
    if let Some(start_idx) = text.find("```\n{") {
        let content_start = start_idx + 4; // Skip "```\n"
        if let Some(end_idx) = text[content_start..].find("\n```") {
            let json_content = &text[content_start..content_start + end_idx];
            return Some(json_content.trim().to_string());
        }
    }
    
    None
}

/// Topologically sort epics so dependencies come before dependents.
/// Returns an error if there's a cycle in the dependency graph.
fn topological_sort_epics(epics: &[PlanEpic]) -> Result<Vec<&PlanEpic>, String> {
    use std::collections::{HashMap, HashSet};

    // Build title -> epic reference map
    let title_to_epic: HashMap<&str, &PlanEpic> =
        epics.iter().map(|e| (e.title.as_str(), e)).collect();

    // Track visited and in-current-path for cycle detection
    let mut visited: HashSet<&str> = HashSet::new();
    let mut in_path: HashSet<&str> = HashSet::new();
    let mut result: Vec<&PlanEpic> = Vec::new();

    fn visit<'a>(
        title: &'a str,
        title_to_epic: &HashMap<&str, &'a PlanEpic>,
        visited: &mut HashSet<&'a str>,
        in_path: &mut HashSet<&'a str>,
        result: &mut Vec<&'a PlanEpic>,
    ) -> Result<(), String> {
        if in_path.contains(title) {
            return Err(format!(
                "Circular dependency detected involving epic '{}'",
                title
            ));
        }

        if visited.contains(title) {
            return Ok(());
        }

        in_path.insert(title);

        if let Some(epic) = title_to_epic.get(title) {
            // Visit all dependencies first
            for dep_title in &epic.depends_on {
                visit(dep_title, title_to_epic, visited, in_path, result)?;
            }

            visited.insert(title);
            in_path.remove(title);
            result.push(epic);
        }

        Ok(())
    }

    // Visit all epics
    for epic in epics {
        visit(
            &epic.title,
            &title_to_epic,
            &mut visited,
            &mut in_path,
            &mut result,
        )?;
    }

    Ok(result)
}

/// Calculate execution phases based on dependencies.
/// Returns a vector of phases, where each phase contains epics that can run in parallel.
fn calculate_execution_phases(epics: &[PlanEpic]) -> Vec<Vec<&PlanEpic>> {
    use std::collections::HashMap;

    let title_to_epic: HashMap<&str, &PlanEpic> =
        epics.iter().map(|e| (e.title.as_str(), e)).collect();

    let mut levels: HashMap<&str, usize> = HashMap::new();

    fn get_level<'a>(
        epic: &'a PlanEpic,
        title_to_epic: &HashMap<&str, &'a PlanEpic>,
        levels: &mut HashMap<&'a str, usize>,
    ) -> usize {
        if let Some(&level) = levels.get(epic.title.as_str()) {
            return level;
        }

        if epic.depends_on.is_empty() {
            levels.insert(&epic.title, 0);
            return 0;
        }

        let mut max_dep_level = 0;
        for dep_title in &epic.depends_on {
            if let Some(dep_epic) = title_to_epic.get(dep_title.as_str()) {
                let dep_level = get_level(dep_epic, title_to_epic, levels);
                max_dep_level = max_dep_level.max(dep_level + 1);
            }
        }
        levels.insert(&epic.title, max_dep_level);
        max_dep_level
    }

    // Calculate levels for all epics
    for epic in epics {
        get_level(epic, &title_to_epic, &mut levels);
    }

    // Group by level
    let max_level = levels.values().copied().max().unwrap_or(0);
    let mut phases: Vec<Vec<&PlanEpic>> = vec![vec![]; max_level + 1];

    for epic in epics {
        let level = levels.get(epic.title.as_str()).copied().unwrap_or(0);
        phases[level].push(epic);
    }

    phases
}

/// Generate a markdown representation of the plan
fn generate_plan_markdown(plan: &ProjectPlan) -> String {
    let mut md = String::new();

    md.push_str("# Work Plan\n\n");
    md.push_str(&plan.overview);
    md.push_str("\n\n---\n\n");

    // Generate execution flow summary
    md.push_str("## Execution Flow\n\n");
    let phases = calculate_execution_phases(&plan.epics);
    let root_count = phases.first().map(|p| p.len()).unwrap_or(0);
    let total_epics = plan.epics.len();

    if root_count == 1 {
        md.push_str(&format!(
            "✓ **Sequential execution:** 1 root epic, {} phases total\n\n",
            phases.len()
        ));
    } else if root_count == total_epics {
        md.push_str(&format!(
            "⚠ **All {} epics are root** (no dependencies) - all can run in parallel\n\n",
            root_count
        ));
    } else {
        md.push_str(&format!(
            "{} root epic{} (can start immediately), {} phases total\n\n",
            root_count,
            if root_count != 1 { "s" } else { "" },
            phases.len()
        ));
    }

    for (phase_idx, phase_epics) in phases.iter().enumerate() {
        let epic_titles: Vec<&str> = phase_epics.iter().map(|e| e.title.as_str()).collect();
        let parallel_note = if phase_epics.len() > 1 {
            " *(parallel)*"
        } else {
            ""
        };
        md.push_str(&format!(
            "- **Phase {}:** {}{}\n",
            phase_idx + 1,
            epic_titles.join(", "),
            parallel_note
        ));
    }
    md.push_str("\n---\n\n");

    for (i, epic) in plan.epics.iter().enumerate() {
        md.push_str(&format!("## Epic {}: {}\n\n", i + 1, epic.title));
        md.push_str(&epic.description);
        md.push('\n');

        if !epic.depends_on.is_empty() {
            if epic.depends_on.len() == 1 {
                md.push_str(&format!("\n**Depends on:** {}\n", epic.depends_on[0]));
            } else {
                md.push_str(&format!("\n**Depends on:** {}\n", epic.depends_on.join(", ")));
            }
        }

        md.push_str("\n### Tickets\n\n");

        for (j, ticket) in epic.tickets.iter().enumerate() {
            md.push_str(&format!("#### {}.{} {}\n\n", i + 1, j + 1, ticket.title));
            md.push_str(&ticket.description);
            md.push('\n');

            if let Some(ref criteria) = ticket.acceptance_criteria {
                md.push_str("\n**Acceptance Criteria:**\n");
                for c in criteria {
                    md.push_str(&format!("- {}\n", c));
                }
            }
            md.push('\n');
        }
        md.push('\n');
    }

    md
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_plan_markdown() {
        let plan = ProjectPlan {
            overview: "Test plan overview".to_string(),
            epics: vec![PlanEpic {
                title: "Epic 1".to_string(),
                description: "Description 1".to_string(),
                depends_on: vec![],
                tickets: vec![PlanTicket {
                    title: "Ticket 1".to_string(),
                    description: "Ticket description".to_string(),
                    acceptance_criteria: Some(vec!["Criteria 1".to_string()]),
                }],
            }],
        };

        let md = generate_plan_markdown(&plan);

        assert!(md.contains("# Work Plan"));
        assert!(md.contains("Test plan overview"));
        assert!(md.contains("Epic 1: Epic 1"));
        assert!(md.contains("1.1 Ticket 1"));
        assert!(md.contains("Criteria 1"));
    }

    #[test]
    fn test_topological_sort_no_dependencies() {
        let epics = vec![
            PlanEpic {
                title: "A".to_string(),
                description: "".to_string(),
                depends_on: vec![],
                tickets: vec![],
            },
            PlanEpic {
                title: "B".to_string(),
                description: "".to_string(),
                depends_on: vec![],
                tickets: vec![],
            },
        ];

        let sorted = topological_sort_epics(&epics).unwrap();
        assert_eq!(sorted.len(), 2);
    }

    #[test]
    fn test_topological_sort_with_dependencies() {
        // B depends on A, so A should come first
        let epics = vec![
            PlanEpic {
                title: "B".to_string(),
                description: "".to_string(),
                depends_on: vec!["A".to_string()],
                tickets: vec![],
            },
            PlanEpic {
                title: "A".to_string(),
                description: "".to_string(),
                depends_on: vec![],
                tickets: vec![],
            },
        ];

        let sorted = topological_sort_epics(&epics).unwrap();
        assert_eq!(sorted.len(), 2);
        assert_eq!(sorted[0].title, "A");
        assert_eq!(sorted[1].title, "B");
    }

    #[test]
    fn test_topological_sort_forward_reference_works() {
        // This is the bug case: C depends on D, but D appears after C in the list
        // The topological sort should handle this correctly
        let epics = vec![
            PlanEpic {
                title: "A".to_string(),
                description: "".to_string(),
                depends_on: vec![],
                tickets: vec![],
            },
            PlanEpic {
                title: "C".to_string(),
                description: "".to_string(),
                depends_on: vec!["D".to_string()],
                tickets: vec![],
            },
            PlanEpic {
                title: "B".to_string(),
                description: "".to_string(),
                depends_on: vec!["A".to_string()],
                tickets: vec![],
            },
            PlanEpic {
                title: "D".to_string(),
                description: "".to_string(),
                depends_on: vec![],
                tickets: vec![],
            },
        ];

        let sorted = topological_sort_epics(&epics).unwrap();
        assert_eq!(sorted.len(), 4);

        // Build a position map
        let positions: std::collections::HashMap<_, _> = sorted
            .iter()
            .enumerate()
            .map(|(i, e)| (e.title.as_str(), i))
            .collect();

        // A should come before B
        assert!(positions["A"] < positions["B"]);
        // D should come before C
        assert!(positions["D"] < positions["C"]);
    }

    #[test]
    fn test_topological_sort_detects_cycle() {
        // A -> B -> A (cycle)
        let epics = vec![
            PlanEpic {
                title: "A".to_string(),
                description: "".to_string(),
                depends_on: vec!["B".to_string()],
                tickets: vec![],
            },
            PlanEpic {
                title: "B".to_string(),
                description: "".to_string(),
                depends_on: vec!["A".to_string()],
                tickets: vec![],
            },
        ];

        let result = topological_sort_epics(&epics);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Circular dependency"));
    }

    #[test]
    fn test_topological_sort_chain() {
        // C -> B -> A (chain)
        let epics = vec![
            PlanEpic {
                title: "C".to_string(),
                description: "".to_string(),
                depends_on: vec!["B".to_string()],
                tickets: vec![],
            },
            PlanEpic {
                title: "B".to_string(),
                description: "".to_string(),
                depends_on: vec!["A".to_string()],
                tickets: vec![],
            },
            PlanEpic {
                title: "A".to_string(),
                description: "".to_string(),
                depends_on: vec![],
                tickets: vec![],
            },
        ];

        let sorted = topological_sort_epics(&epics).unwrap();
        assert_eq!(sorted.len(), 3);
        assert_eq!(sorted[0].title, "A");
        assert_eq!(sorted[1].title, "B");
        assert_eq!(sorted[2].title, "C");
    }

    #[test]
    fn test_topological_sort_multiple_dependencies() {
        // C depends on both A and B
        let epics = vec![
            PlanEpic {
                title: "C".to_string(),
                description: "".to_string(),
                depends_on: vec!["A".to_string(), "B".to_string()],
                tickets: vec![],
            },
            PlanEpic {
                title: "A".to_string(),
                description: "".to_string(),
                depends_on: vec![],
                tickets: vec![],
            },
            PlanEpic {
                title: "B".to_string(),
                description: "".to_string(),
                depends_on: vec![],
                tickets: vec![],
            },
        ];

        let sorted = topological_sort_epics(&epics).unwrap();
        assert_eq!(sorted.len(), 3);

        // Build a position map
        let positions: std::collections::HashMap<_, _> = sorted
            .iter()
            .enumerate()
            .map(|(i, e)| (e.title.as_str(), i))
            .collect();

        // Both A and B should come before C
        assert!(positions["A"] < positions["C"]);
        assert!(positions["B"] < positions["C"]);
    }

    #[test]
    fn test_parse_project_plan_direct_json() {
        let json = r#"{"overview":"Test","epics":[]}"#;
        let result = parse_project_plan(json);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().overview, "Test");
    }

    #[test]
    fn test_parse_project_plan_with_preamble() {
        let text = r#"Here's the plan:

{"overview":"Test plan","epics":[{"title":"Epic 1","description":"Desc","dependsOn":[],"tickets":[]}]}

That's the plan!"#;
        
        let result = parse_project_plan(text);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().overview, "Test plan");
    }

    #[test]
    fn test_parse_project_plan_with_old_format_null() {
        // Test backward compatibility with old format (null for dependsOn)
        let text = r#"{"overview":"Test","epics":[{"title":"Epic 1","description":"Desc","dependsOn":null,"tickets":[]}]}"#;
        
        let result = parse_project_plan(text);
        assert!(result.is_ok());
        let plan = result.unwrap();
        assert!(plan.epics[0].depends_on.is_empty());
    }

    #[test]
    fn test_parse_project_plan_with_old_format_string() {
        // Test backward compatibility with old format (string for dependsOn)
        let text = r#"{"overview":"Test","epics":[{"title":"Epic 1","description":"Desc","dependsOn":"Other Epic","tickets":[]}]}"#;
        
        let result = parse_project_plan(text);
        assert!(result.is_ok());
        let plan = result.unwrap();
        assert_eq!(plan.epics[0].depends_on, vec!["Other Epic".to_string()]);
    }

    #[test]
    fn test_parse_project_plan_with_new_format_array() {
        // Test new format (array for dependsOn)
        let text = r#"{"overview":"Test","epics":[{"title":"Epic 1","description":"Desc","dependsOn":["A", "B"],"tickets":[]}]}"#;
        
        let result = parse_project_plan(text);
        assert!(result.is_ok());
        let plan = result.unwrap();
        assert_eq!(plan.epics[0].depends_on, vec!["A".to_string(), "B".to_string()]);
    }

    #[test]
    fn test_parse_project_plan_code_block() {
        let text = r#"Here's the JSON:

```json
{"overview":"Code block plan","epics":[]}
```

Done!"#;
        
        let result = parse_project_plan(text);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().overview, "Code block plan");
    }

    #[test]
    fn test_parse_project_plan_no_json() {
        let text = "This has no JSON at all";
        let result = parse_project_plan(text);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_json_code_block() {
        let text = "prefix\n```json\n{\"key\":\"value\"}\n```\nsuffix";
        let result = extract_json_code_block(text);
        assert_eq!(result, Some("{\"key\":\"value\"}".to_string()));
    }

    #[test]
    fn test_extract_json_code_block_plain() {
        let text = "prefix\n```\n{\"key\":\"value\"}\n```\nsuffix";
        let result = extract_json_code_block(text);
        assert_eq!(result, Some("{\"key\":\"value\"}".to_string()));
    }

    #[test]
    fn test_extract_json_code_block_none() {
        let text = "no code block here";
        let result = extract_json_code_block(text);
        assert_eq!(result, None);
    }
}
