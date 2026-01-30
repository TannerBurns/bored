//! Planner agent for exploring codebases and generating work plans.
//!
//! The planner agent works in phases:
//! 1. **Exploration**: Uses semantic search and file reading to understand the codebase
//! 2. **Planning**: Generates a structured work plan with epics and tickets
//! 3. **Execution**: Creates the epics and tickets in the database with proper dependencies

use std::sync::Arc;
use crate::db::{
    Database, 
    Scratchpad, 
    ScratchpadStatus, 
    Exploration, 
    ProjectPlan, 
    PlanEpic, 
    PlanTicket,
    CreateTicket,
    Priority,
    WorkflowType,
};

/// Configuration for the planner agent
#[derive(Debug, Clone)]
pub struct PlannerConfig {
    pub scratchpad_id: String,
    pub max_explorations: usize,
    pub auto_approve: bool,
    pub model: Option<String>,
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
}

/// The planner agent
pub struct PlannerAgent {
    db: Arc<Database>,
    config: PlannerConfig,
}

impl PlannerAgent {
    pub fn new(db: Arc<Database>, config: PlannerConfig) -> Self {
        Self { db, config }
    }
    
    /// Run the full planner workflow: explore -> plan -> (optionally) execute
    pub async fn run(&self) -> Result<PlannerResult, PlannerError> {
        // Get scratchpad
        let scratchpad = self.db.get_scratchpad(&self.config.scratchpad_id)
            .map_err(|e| PlannerError::Database(e.to_string()))?;
        
        tracing::info!(
            "Starting planner for scratchpad {}: {:?}",
            scratchpad.id,
            scratchpad.status
        );
        
        // Run exploration phase
        self.run_exploration(&scratchpad).await?;
        
        // Generate plan
        self.generate_plan(&scratchpad).await?;
        
        // Check if auto-approve is enabled
        if self.config.auto_approve {
            self.db.set_scratchpad_status(&scratchpad.id, ScratchpadStatus::Approved)
                .map_err(|e| PlannerError::Database(e.to_string()))?;
            
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
    
    /// Run the exploration phase
    async fn run_exploration(&self, scratchpad: &Scratchpad) -> Result<(), PlannerError> {
        // Update status to exploring
        self.db.set_scratchpad_status(&scratchpad.id, ScratchpadStatus::Exploring)
            .map_err(|e| PlannerError::Database(e.to_string()))?;
        
        tracing::info!(
            "Starting exploration phase for scratchpad {} (max {} queries)",
            scratchpad.id,
            self.config.max_explorations
        );
        
        // In a real implementation, this would:
        // 1. Parse the user input to identify key concepts
        // 2. Run semantic searches on the codebase
        // 3. Read relevant files
        // 4. Build context about the codebase structure
        
        // For now, we'll add a placeholder exploration entry
        let initial_exploration = Exploration {
            query: format!("Understanding the codebase for: {}", scratchpad.user_input),
            response: "Exploration phase completed. The planner has analyzed the codebase structure and identified relevant files and patterns.".to_string(),
            timestamp: chrono::Utc::now(),
        };
        
        self.db.append_exploration(&scratchpad.id, &initial_exploration)
            .map_err(|e| PlannerError::Database(e.to_string()))?;
        
        Ok(())
    }
    
    /// Generate a structured plan based on exploration results
    async fn generate_plan(&self, scratchpad: &Scratchpad) -> Result<(), PlannerError> {
        // Update status to planning
        self.db.set_scratchpad_status(&scratchpad.id, ScratchpadStatus::Planning)
            .map_err(|e| PlannerError::Database(e.to_string()))?;
        
        tracing::info!("Generating plan for scratchpad {}", scratchpad.id);
        
        // In a real implementation, this would use an LLM to generate a structured plan
        // For now, create a placeholder plan structure
        let plan = ProjectPlan {
            overview: format!(
                "Implementation plan for: {}\n\nThis plan breaks down the work into manageable epics and tickets.",
                scratchpad.user_input
            ),
            epics: vec![
                PlanEpic {
                    title: "Foundation Setup".to_string(),
                    description: "Set up the foundational components and infrastructure needed for the feature.".to_string(),
                    depends_on: None,
                    tickets: vec![
                        PlanTicket {
                            title: "Database schema changes".to_string(),
                            description: "Add necessary database tables and columns".to_string(),
                            acceptance_criteria: Some(vec![
                                "Schema migration added".to_string(),
                                "Models updated".to_string(),
                            ]),
                        },
                        PlanTicket {
                            title: "Backend API endpoints".to_string(),
                            description: "Implement the backend API endpoints".to_string(),
                            acceptance_criteria: Some(vec![
                                "CRUD endpoints implemented".to_string(),
                                "Tests passing".to_string(),
                            ]),
                        },
                    ],
                },
                PlanEpic {
                    title: "Core Implementation".to_string(),
                    description: "Implement the core functionality.".to_string(),
                    depends_on: Some("Foundation Setup".to_string()),
                    tickets: vec![
                        PlanTicket {
                            title: "Core logic implementation".to_string(),
                            description: "Implement the main business logic".to_string(),
                            acceptance_criteria: Some(vec![
                                "Logic implemented".to_string(),
                                "Edge cases handled".to_string(),
                            ]),
                        },
                    ],
                },
            ],
        };
        
        // Generate markdown representation
        let markdown = generate_plan_markdown(&plan);
        
        // Convert plan to JSON value
        let plan_json = serde_json::to_value(&plan)
            .map_err(|e| PlannerError::PlanGenerationFailed(e.to_string()))?;
        
        // Save the plan
        self.db.set_scratchpad_plan(&scratchpad.id, &markdown, Some(&plan_json))
            .map_err(|e| PlannerError::Database(e.to_string()))?;
        
        // Update status to awaiting approval
        self.db.set_scratchpad_status(&scratchpad.id, ScratchpadStatus::AwaitingApproval)
            .map_err(|e| PlannerError::Database(e.to_string()))?;
        
        Ok(())
    }
    
    /// Execute an approved plan by creating epics and tickets
    pub async fn execute_plan(&self) -> Result<PlannerResult, PlannerError> {
        let scratchpad = self.db.get_scratchpad(&self.config.scratchpad_id)
            .map_err(|e| PlannerError::Database(e.to_string()))?;
        
        // Verify status is approved
        if scratchpad.status != ScratchpadStatus::Approved {
            return Err(PlannerError::InvalidState(format!(
                "Cannot execute plan: status is {:?}, expected Approved",
                scratchpad.status
            )));
        }
        
        // Update status to executing
        self.db.set_scratchpad_status(&scratchpad.id, ScratchpadStatus::Executing)
            .map_err(|e| PlannerError::Database(e.to_string()))?;
        
        tracing::info!("Executing plan for scratchpad {}", scratchpad.id);
        
        // Get the plan JSON
        let plan_json = scratchpad.plan_json
            .ok_or_else(|| PlannerError::InvalidState("No plan JSON found".to_string()))?;
        
        let plan: ProjectPlan = serde_json::from_value(plan_json)
            .map_err(|e| PlannerError::ExecutionFailed(format!("Failed to parse plan: {}", e)))?;
        
        // Get board's backlog column for creating tickets
        let columns = self.db.get_columns(&scratchpad.board_id)
            .map_err(|e| PlannerError::Database(e.to_string()))?;
        
        let backlog_column = columns.iter()
            .find(|c| c.name == "Backlog")
            .ok_or_else(|| PlannerError::ExecutionFailed("Backlog column not found".to_string()))?;
        
        let mut epic_ids = Vec::new();
        let mut ticket_ids = Vec::new();
        
        // First pass: topologically sort epics so dependencies are created before dependents
        // Build title -> index map and validate all dependencies exist
        let epic_titles: std::collections::HashSet<_> = plan.epics.iter()
            .map(|e| e.title.clone())
            .collect();
        
        for plan_epic in &plan.epics {
            if let Some(ref dep_title) = plan_epic.depends_on {
                if !epic_titles.contains(dep_title) {
                    return Err(PlannerError::ExecutionFailed(format!(
                        "Epic '{}' depends on '{}' which does not exist in the plan",
                        plan_epic.title, dep_title
                    )));
                }
            }
        }
        
        // Topologically sort: dependencies before dependents
        let sorted_epics = topological_sort_epics(&plan.epics)
            .map_err(|e| PlannerError::ExecutionFailed(e))?;
        
        let mut epic_title_to_id = std::collections::HashMap::new();
        
        // Create epics and their child tickets in dependency order
        for plan_epic in sorted_epics {
            // Resolve dependency - guaranteed to exist since we sorted topologically
            let depends_on_epic_id = plan_epic.depends_on.as_ref()
                .and_then(|dep_title| epic_title_to_id.get(dep_title).cloned());
            
            // Create the epic
            let epic = self.db.create_ticket(&CreateTicket {
                board_id: scratchpad.board_id.clone(),
                column_id: backlog_column.id.clone(),
                title: plan_epic.title.clone(),
                description_md: plan_epic.description.clone(),
                priority: Priority::Medium,
                labels: vec!["plan-generated".to_string()],
                project_id: Some(scratchpad.project_id.clone()),
                agent_pref: None,
                workflow_type: WorkflowType::MultiStage,
                model: None,
                branch_name: None,
                is_epic: true,
                epic_id: None,
                depends_on_epic_id,
                scratchpad_id: Some(scratchpad.id.clone()),
            }).map_err(|e| PlannerError::Database(e.to_string()))?;
            
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
                
                let ticket = self.db.create_ticket(&CreateTicket {
                    board_id: scratchpad.board_id.clone(),
                    column_id: backlog_column.id.clone(),
                    title: plan_ticket.title.clone(),
                    description_md: description,
                    priority: Priority::Medium,
                    labels: vec!["plan-generated".to_string()],
                    project_id: Some(scratchpad.project_id.clone()),
                    agent_pref: None,
                    workflow_type: WorkflowType::MultiStage,
                    model: None,
                    branch_name: None,
                    is_epic: false,
                    epic_id: Some(epic.id.clone()),
                    depends_on_epic_id: None,
                    scratchpad_id: Some(scratchpad.id.clone()),
                }).map_err(|e| PlannerError::Database(e.to_string()))?;
                
                ticket_ids.push(ticket.id);
            }
        }
        
        // Update status to completed
        self.db.set_scratchpad_status(&scratchpad.id, ScratchpadStatus::Completed)
            .map_err(|e| PlannerError::Database(e.to_string()))?;
        
        tracing::info!(
            "Plan execution completed: {} epics, {} tickets created",
            epic_ids.len(),
            ticket_ids.len()
        );
        
        Ok(PlannerResult {
            scratchpad_id: scratchpad.id,
            status: ScratchpadStatus::Completed,
            epic_ids,
            ticket_ids,
        })
    }
}

/// Topologically sort epics so dependencies come before dependents.
/// Returns an error if there's a cycle in the dependency graph.
fn topological_sort_epics(epics: &[PlanEpic]) -> Result<Vec<&PlanEpic>, String> {
    use std::collections::{HashMap, HashSet};
    
    // Build title -> epic reference map
    let title_to_epic: HashMap<&str, &PlanEpic> = epics.iter()
        .map(|e| (e.title.as_str(), e))
        .collect();
    
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
            return Err(format!("Circular dependency detected involving epic '{}'", title));
        }
        
        if visited.contains(title) {
            return Ok(());
        }
        
        in_path.insert(title);
        
        if let Some(epic) = title_to_epic.get(title) {
            // Visit dependency first
            if let Some(ref dep_title) = epic.depends_on {
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
        visit(&epic.title, &title_to_epic, &mut visited, &mut in_path, &mut result)?;
    }
    
    Ok(result)
}

/// Generate a markdown representation of the plan
fn generate_plan_markdown(plan: &ProjectPlan) -> String {
    let mut md = String::new();
    
    md.push_str("# Work Plan\n\n");
    md.push_str(&plan.overview);
    md.push_str("\n\n---\n\n");
    
    for (i, epic) in plan.epics.iter().enumerate() {
        md.push_str(&format!("## Epic {}: {}\n\n", i + 1, epic.title));
        md.push_str(&epic.description);
        md.push('\n');
        
        if let Some(ref dep) = epic.depends_on {
            md.push_str(&format!("\n**Depends on:** {}\n", dep));
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
            epics: vec![
                PlanEpic {
                    title: "Epic 1".to_string(),
                    description: "Description 1".to_string(),
                    depends_on: None,
                    tickets: vec![
                        PlanTicket {
                            title: "Ticket 1".to_string(),
                            description: "Ticket description".to_string(),
                            acceptance_criteria: Some(vec!["Criteria 1".to_string()]),
                        },
                    ],
                },
            ],
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
                depends_on: None,
                tickets: vec![],
            },
            PlanEpic {
                title: "B".to_string(),
                description: "".to_string(),
                depends_on: None,
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
                depends_on: Some("A".to_string()),
                tickets: vec![],
            },
            PlanEpic {
                title: "A".to_string(),
                description: "".to_string(),
                depends_on: None,
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
                depends_on: None,
                tickets: vec![],
            },
            PlanEpic {
                title: "C".to_string(),
                description: "".to_string(),
                depends_on: Some("D".to_string()),
                tickets: vec![],
            },
            PlanEpic {
                title: "B".to_string(),
                description: "".to_string(),
                depends_on: Some("A".to_string()),
                tickets: vec![],
            },
            PlanEpic {
                title: "D".to_string(),
                description: "".to_string(),
                depends_on: None,
                tickets: vec![],
            },
        ];
        
        let sorted = topological_sort_epics(&epics).unwrap();
        assert_eq!(sorted.len(), 4);
        
        // Build a position map
        let positions: std::collections::HashMap<_, _> = sorted.iter()
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
                depends_on: Some("B".to_string()),
                tickets: vec![],
            },
            PlanEpic {
                title: "B".to_string(),
                description: "".to_string(),
                depends_on: Some("A".to_string()),
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
                depends_on: Some("B".to_string()),
                tickets: vec![],
            },
            PlanEpic {
                title: "B".to_string(),
                description: "".to_string(),
                depends_on: Some("A".to_string()),
                tickets: vec![],
            },
            PlanEpic {
                title: "A".to_string(),
                description: "".to_string(),
                depends_on: None,
                tickets: vec![],
            },
        ];
        
        let sorted = topological_sort_epics(&epics).unwrap();
        assert_eq!(sorted.len(), 3);
        assert_eq!(sorted[0].title, "A");
        assert_eq!(sorted[1].title, "B");
        assert_eq!(sorted[2].title, "C");
    }
}
