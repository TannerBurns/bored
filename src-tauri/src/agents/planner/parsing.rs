//! Parsing utilities for planner output.

use crate::db::ProjectPlan;

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
pub fn extract_json_code_block(text: &str) -> Option<String> {
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
        // Backward compat: old plans that don't include branchName
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
}
