//! Branch name generation and parsing.

use super::utils::slugify;
use crate::db::models::Ticket;

/// Generate a prompt for the branch creation stage
pub fn generate_branch_prompt(ticket: &Ticket) -> String {
    let id_prefix: String = ticket.id.chars().take(8).collect();
    let branch_name = format!("ticket/{}/{}", id_prefix, slugify(&ticket.title));

    format!(
        r#"Create a new git branch for this task.

## Task
Create and switch to a new branch: `{branch_name}`

## Instructions
1. Check if you're on a clean working tree (stash changes if needed)
2. Switch to the main branch (or master if main doesn't exist)
3. Pull the latest changes from origin to ensure you have the most up-to-date code: `git pull origin main`
4. Create and switch to the new branch from the updated main branch
5. Push the branch to origin with -u flag

IMPORTANT: You must pull the latest changes from main before creating the branch. This ensures the codebase is up-to-date and avoids drift from other recent changes. The planning and implementation stages will use this code, so it must be current.

Do NOT start implementing any code changes. Just create the branch.
"#
    )
}

/// Generate a prompt to ask the agent what branch was created
pub fn generate_get_branch_name_prompt() -> String {
    r#"What is the exact name of the git branch you just created?

Reply with ONLY the branch name on a single line, nothing else.
For example: ticket/abc12345/add-feature
"#
    .to_string()
}

/// Generate a prompt for the agent to create a meaningful branch name
/// based on ticket title, description, and labels.
pub fn generate_branch_name_generation_prompt(ticket: &Ticket) -> String {
    let id_prefix: String = ticket.id.chars().take(8).collect();
    let labels_str = if ticket.labels.is_empty() {
        "None".to_string()
    } else {
        ticket.labels.join(", ")
    };

    format!(
        r#"Analyze this ticket and generate a git branch name.

## Ticket Information

**Title**: {title}

**Description**:
{description}

**Labels**: {labels}

## Branch Naming Rules

1. **Extract external ticket IDs**: Look for patterns like JIRA-123, GH-123, ISSUE-123, or similar ticket/issue references in the title or description. If found, include it in the branch name.

2. **Determine the type prefix** based on the nature of the work:
   - `feat/` - New features or functionality
   - `fix/` - Bug fixes
   - `chore/` - Maintenance tasks, dependency updates, config changes
   - `refactor/` - Code restructuring without changing behavior
   - `docs/` - Documentation only changes
   - `test/` - Adding or updating tests

3. **Create a concise slug** from the main task (2-5 words, lowercase, hyphen-separated)

4. **Format**: `<type>/<ticket-id>/<slug>` or `<type>/{id_prefix}/<slug>` if no external ID found

## Output Format

Respond with ONLY a JSON object on a single line, nothing else:
{{"branch_name": "<your-generated-branch-name>"}}

## Examples

- If description mentions "JIRA-456" and it's a bug fix: `{{"branch_name": "fix/JIRA-456/user-login-error"}}`
- If no external ID and it's a new feature: `{{"branch_name": "feat/{id_prefix}/add-dark-mode"}}`
- For a refactoring task: `{{"branch_name": "refactor/{id_prefix}/extract-auth-service"}}`
"#,
        title = ticket.title,
        description = if ticket.description_md.is_empty() {
            "No description provided".to_string()
        } else {
            ticket.description_md.clone()
        },
        labels = labels_str,
        id_prefix = id_prefix,
    )
}

/// Parse the branch name from agent output (expects JSON format)
pub fn parse_branch_name_from_output(output: &str) -> Option<String> {
    // Try to find JSON in the output
    let trimmed = output.trim();

    // Try parsing as JSON directly
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(branch) = json.get("branch_name").and_then(|v| v.as_str()) {
            return Some(branch.to_string());
        }
    }

    // Try to find the FIRST complete JSON object in the output
    // This handles cases where multiple JSON objects are concatenated together
    if let Some(start) = trimmed.find('{') {
        // Find matching closing brace by counting braces
        let chars: Vec<char> = trimmed[start..].chars().collect();
        let mut depth = 0;
        let mut end_offset = None;

        for (i, ch) in chars.iter().enumerate() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end_offset = Some(i);
                        break;
                    }
                }
                _ => {}
            }
        }

        if let Some(end) = end_offset {
            let json_str: String = chars[..=end].iter().collect();
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&json_str) {
                if let Some(branch) = json.get("branch_name").and_then(|v| v.as_str()) {
                    return Some(branch.to_string());
                }
            }
        }
    }

    // Fallback: if it looks like a valid branch name, use the first line
    let first_line = trimmed.lines().next().unwrap_or("");
    if first_line.contains('/') && !first_line.contains(' ') && first_line.len() < 100 {
        return Some(first_line.to_string());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::{Priority, WorkflowType};
    use chrono::Utc;

    fn create_test_ticket() -> Ticket {
        Ticket {
            id: "ticket-1".to_string(),
            board_id: "board-1".to_string(),
            column_id: "col-1".to_string(),
            title: "Test Ticket".to_string(),
            description_md: "This is a test description.".to_string(),
            priority: Priority::Medium,
            labels: vec!["bug".to_string(), "urgent".to_string()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            locked_by_run_id: None,
            lock_expires_at: None,
            project_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
            order_in_epic: None,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
            paused_at: None,
            paused_at_stage: None,
            paused_run_id: None,
        }
    }

    #[test]
    fn parse_branch_name_from_simple_json() {
        let output = r#"{"branch_name": "feat/abc123/add-feature"}"#;
        let result = parse_branch_name_from_output(output);
        assert_eq!(result, Some("feat/abc123/add-feature".to_string()));
    }

    #[test]
    fn parse_branch_name_from_duplicated_json() {
        // This simulates the output from Claude stream-json where the text appears twice
        let output = r#"{"branch_name": "feat/2f8c058c/add-frontend-themes"}{"branch_name": "feat/2f8c058c/add-frontend-themes"}"#;
        let result = parse_branch_name_from_output(output);
        assert_eq!(
            result,
            Some("feat/2f8c058c/add-frontend-themes".to_string())
        );
    }

    #[test]
    fn parse_branch_name_with_surrounding_text() {
        let output = r#"Here is the branch name: {"branch_name": "fix/123/bug-fix"} That's all!"#;
        let result = parse_branch_name_from_output(output);
        assert_eq!(result, Some("fix/123/bug-fix".to_string()));
    }

    #[test]
    fn parse_branch_name_fallback_to_first_line() {
        // If no valid JSON, fall back to first line if it looks like a branch name
        let output = "feat/abc/some-branch";
        let result = parse_branch_name_from_output(output);
        assert_eq!(result, Some("feat/abc/some-branch".to_string()));
    }

    #[test]
    fn parse_branch_name_returns_none_for_invalid() {
        let output = "This is just some text without a branch name";
        let result = parse_branch_name_from_output(output);
        assert_eq!(result, None);
    }

    #[test]
    fn generate_branch_name_generation_prompt_includes_ticket_info() {
        let ticket = create_test_ticket();
        let prompt = generate_branch_name_generation_prompt(&ticket);

        assert!(prompt.contains(&ticket.title));
        assert!(prompt.contains(&ticket.description_md));
        assert!(prompt.contains("branch_name"));
        assert!(prompt.contains("feat/"));
        assert!(prompt.contains("fix/"));
    }

    #[test]
    fn generate_branch_name_generation_prompt_with_labels() {
        let mut ticket = create_test_ticket();
        ticket.labels = vec!["bug".to_string(), "urgent".to_string()];
        let prompt = generate_branch_name_generation_prompt(&ticket);

        assert!(prompt.contains("bug"));
        assert!(prompt.contains("urgent"));
    }

    #[test]
    fn generate_branch_name_generation_prompt_with_empty_description() {
        let mut ticket = create_test_ticket();
        ticket.description_md = "".to_string();
        let prompt = generate_branch_name_generation_prompt(&ticket);

        assert!(prompt.contains("No description provided"));
    }

    #[test]
    fn generate_branch_name_generation_prompt_includes_id_prefix() {
        let mut ticket = create_test_ticket();
        ticket.id = "abcd1234efgh5678".to_string();
        let prompt = generate_branch_name_generation_prompt(&ticket);

        // Should include first 8 chars of ticket ID
        assert!(prompt.contains("abcd1234"));
    }
}
