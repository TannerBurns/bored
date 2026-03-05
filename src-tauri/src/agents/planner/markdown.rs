//! Markdown generation for plan display.

use crate::db::ProjectPlan;

use super::dependencies::calculate_execution_phases;

/// Generate a markdown representation of the plan
pub fn generate_plan_markdown(plan: &ProjectPlan) -> String {
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
                md.push_str(&format!(
                    "\n**Depends on:** {}\n",
                    epic.depends_on.join(", ")
                ));
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

            if let Some(ref tasks) = ticket.tasks {
                md.push_str("\n**Tasks:**\n");
                for (k, task) in tasks.iter().enumerate() {
                    md.push_str(&format!("{}. {}\n", k + 1, task.title));
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
    use crate::db::{PlanEpic, PlanTicket, PlanTicketTask};

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
                    branch_name: Some("feat/epic-1/ticket-1".to_string()),
                    tasks: None,
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
    fn test_generate_plan_markdown_with_tasks() {
        let plan = ProjectPlan {
            overview: "Plan with tasks".to_string(),
            epics: vec![PlanEpic {
                title: "Epic 1".to_string(),
                description: "Description".to_string(),
                depends_on: vec![],
                tickets: vec![PlanTicket {
                    title: "Ticket 1".to_string(),
                    description: "Shared context".to_string(),
                    acceptance_criteria: Some(vec!["It works".to_string()]),
                    branch_name: Some("feat/epic-1/ticket-1".to_string()),
                    tasks: Some(vec![
                        PlanTicketTask {
                            title: "Add the store".to_string(),
                            content: Some("Create the store file".to_string()),
                        },
                        PlanTicketTask {
                            title: "Wire up the UI".to_string(),
                            content: None,
                        },
                    ]),
                }],
            }],
        };

        let md = generate_plan_markdown(&plan);

        assert!(md.contains("**Tasks:**"));
        assert!(md.contains("1. Add the store"));
        assert!(md.contains("2. Wire up the UI"));
    }

    #[test]
    fn test_generate_plan_markdown_with_dependencies() {
        let plan = ProjectPlan {
            overview: "Plan with deps".to_string(),
            epics: vec![
                PlanEpic {
                    title: "Epic A".to_string(),
                    description: "First epic".to_string(),
                    depends_on: vec![],
                    tickets: vec![],
                },
                PlanEpic {
                    title: "Epic B".to_string(),
                    description: "Second epic".to_string(),
                    depends_on: vec!["Epic A".to_string()],
                    tickets: vec![],
                },
            ],
        };

        let md = generate_plan_markdown(&plan);

        assert!(md.contains("**Depends on:** Epic A"));
        assert!(md.contains("Phase 1"));
        assert!(md.contains("Phase 2"));
    }

    #[test]
    fn test_generate_plan_markdown_multiple_dependencies() {
        let plan = ProjectPlan {
            overview: "Plan with multiple deps".to_string(),
            epics: vec![
                PlanEpic {
                    title: "Epic A".to_string(),
                    description: "First".to_string(),
                    depends_on: vec![],
                    tickets: vec![],
                },
                PlanEpic {
                    title: "Epic B".to_string(),
                    description: "Second".to_string(),
                    depends_on: vec![],
                    tickets: vec![],
                },
                PlanEpic {
                    title: "Epic C".to_string(),
                    description: "Depends on both".to_string(),
                    depends_on: vec!["Epic A".to_string(), "Epic B".to_string()],
                    tickets: vec![],
                },
            ],
        };

        let md = generate_plan_markdown(&plan);

        assert!(md.contains("**Depends on:** Epic A, Epic B"));
    }
}
