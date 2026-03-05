//! Parsing utilities for planner output.

use crate::agents::json_extraction;
use crate::db::ProjectPlan;

/// Parse a ProjectPlan from agent output.
/// Handles cases where the JSON is embedded in other text.
pub fn parse_project_plan(output: &str) -> Result<ProjectPlan, String> {
    json_extraction::parse_json_response::<ProjectPlan>(output)
        .ok_or_else(|| "No valid JSON found in planner output".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let text = r#"{"overview":"Test","epics":[{"title":"Epic 1","description":"Desc","dependsOn":null,"tickets":[]}]}"#;

        let result = parse_project_plan(text);
        assert!(result.is_ok());
        let plan = result.unwrap();
        assert!(plan.epics[0].depends_on.is_empty());
    }

    #[test]
    fn test_parse_project_plan_with_old_format_string() {
        let text = r#"{"overview":"Test","epics":[{"title":"Epic 1","description":"Desc","dependsOn":"Other Epic","tickets":[]}]}"#;

        let result = parse_project_plan(text);
        assert!(result.is_ok());
        let plan = result.unwrap();
        assert_eq!(plan.epics[0].depends_on, vec!["Other Epic".to_string()]);
    }

    #[test]
    fn test_parse_project_plan_with_new_format_array() {
        let text = r#"{"overview":"Test","epics":[{"title":"Epic 1","description":"Desc","dependsOn":["A", "B"],"tickets":[]}]}"#;

        let result = parse_project_plan(text);
        assert!(result.is_ok());
        let plan = result.unwrap();
        assert_eq!(
            plan.epics[0].depends_on,
            vec!["A".to_string(), "B".to_string()]
        );
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
    fn test_parse_plan_ticket_with_branch_name() {
        let text = r#"{"overview":"Test","epics":[{
            "title":"Epic 1","description":"Desc","dependsOn":[],
            "tickets":[{
                "title":"Ticket 1",
                "description":"Do the thing",
                "acceptanceCriteria":["Done"],
                "branchName":"feat/epic-1/do-the-thing"
            }]
        }]}"#;

        let plan = parse_project_plan(text).unwrap();
        let ticket = &plan.epics[0].tickets[0];
        assert_eq!(
            ticket.branch_name,
            Some("feat/epic-1/do-the-thing".to_string())
        );
    }

    #[test]
    fn test_parse_plan_ticket_without_branch_name() {
        let text = r#"{"overview":"Test","epics":[{
            "title":"Epic 1","description":"Desc","dependsOn":[],
            "tickets":[{
                "title":"Ticket 1",
                "description":"Do the thing",
                "acceptanceCriteria":["Done"]
            }]
        }]}"#;

        let plan = parse_project_plan(text).unwrap();
        let ticket = &plan.epics[0].tickets[0];
        assert_eq!(ticket.branch_name, None);
    }

    #[test]
    fn test_parse_plan_ticket_with_null_branch_name() {
        let text = r#"{"overview":"Test","epics":[{
            "title":"Epic 1","description":"Desc","dependsOn":[],
            "tickets":[{
                "title":"Ticket 1",
                "description":"Do the thing",
                "branchName":null
            }]
        }]}"#;

        let plan = parse_project_plan(text).unwrap();
        assert_eq!(plan.epics[0].tickets[0].branch_name, None);
    }

    #[test]
    fn test_parse_plan_ticket_with_tasks() {
        let text = r#"{"overview":"Test","epics":[{
            "title":"Epic 1","description":"Desc","dependsOn":[],
            "tickets":[{
                "title":"Ticket 1",
                "description":"Context",
                "acceptanceCriteria":["Done"],
                "branchName":"feat/epic-1/ticket-1",
                "tasks":[
                    {"title":"Task A","content":"Do step A"},
                    {"title":"Task B","content":"Do step B"}
                ]
            }]
        }]}"#;

        let plan = parse_project_plan(text).unwrap();
        let ticket = &plan.epics[0].tickets[0];
        let tasks = ticket.tasks.as_ref().unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].title, "Task A");
        assert_eq!(tasks[0].content.as_deref(), Some("Do step A"));
        assert_eq!(tasks[1].title, "Task B");
    }

    #[test]
    fn test_parse_plan_ticket_without_tasks_defaults_to_none() {
        let text = r#"{"overview":"Test","epics":[{
            "title":"Epic 1","description":"Desc","dependsOn":[],
            "tickets":[{
                "title":"Ticket 1",
                "description":"Context",
                "acceptanceCriteria":["Done"]
            }]
        }]}"#;

        let plan = parse_project_plan(text).unwrap();
        assert!(plan.epics[0].tickets[0].tasks.is_none());
    }
}
