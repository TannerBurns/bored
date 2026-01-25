use crate::db::models::{Priority, Ticket};
use super::AgentKind;

fn slugify(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .take(50)
        .collect()
}

pub fn generate_ticket_prompt(ticket: &Ticket) -> String {
    generate_ticket_prompt_with_workflow(ticket, None)
}

/// Generate a ticket prompt with optional workflow instructions for the given agent type
pub fn generate_ticket_prompt_with_workflow(ticket: &Ticket, agent_kind: Option<AgentKind>) -> String {
    let mut prompt = String::new();

    prompt.push_str(&format!("# Task: {}\n\n", ticket.title));

    if !ticket.description_md.is_empty() {
        prompt.push_str("## Description\n\n");
        prompt.push_str(&ticket.description_md);
        prompt.push_str("\n\n");
    }

    let priority_context = match ticket.priority {
        Priority::Urgent => {
            "This is an URGENT task. Please prioritize speed while maintaining quality."
        }
        Priority::High => "This is a high-priority task.",
        Priority::Medium => "",
        Priority::Low => "This is a low-priority task. Take time to ensure quality.",
    };

    if !priority_context.is_empty() {
        prompt.push_str(&format!("{}\n\n", priority_context));
    }

    if !ticket.labels.is_empty() {
        prompt.push_str("## Labels\n\n");
        for label in &ticket.labels {
            prompt.push_str(&format!("- {}\n", label));
        }
        prompt.push('\n');
    }

    if let Some(kind) = agent_kind {
        let branch_name = format!("ticket/{}/{}", &ticket.id[..8.min(ticket.id.len())], slugify(&ticket.title));
        prompt.push_str("## Workflow\n\n");
        prompt.push_str(&format!("1. Create a branch: `{}`\n", branch_name));
        prompt.push_str("2. Create a plan before implementing\n");
        prompt.push_str("3. After implementation, run this QA sequence:\n\n");
        
        match kind {
            AgentKind::Cursor => {
                prompt.push_str("   - `/deslop` - Remove AI-generated code patterns\n");
                prompt.push_str("   - `/cleanup` - Fix lint/type errors\n");
                prompt.push_str("   - `/unit-tests` - Add test coverage for your changes\n");
                prompt.push_str("   - `/cleanup` - Fix any test-related issues\n");
                prompt.push_str("   - `/review-changes` - Apply best practices\n");
                prompt.push_str("   - `/cleanup` - Final lint pass\n");
                prompt.push_str("   - `/review-changes` - Second review pass\n");
                prompt.push_str("   - `/add-and-commit` - Stage and commit with detailed message\n");
            }
            AgentKind::Claude => {
                prompt.push_str("   Read and follow each command file in order:\n");
                prompt.push_str("   - `.claude/commands/deslop.md` - Remove AI-generated code patterns\n");
                prompt.push_str("   - `.claude/commands/cleanup.md` - Fix lint/type errors\n");
                prompt.push_str("   - `.claude/commands/unit-tests.md` - Add test coverage\n");
                prompt.push_str("   - `.claude/commands/cleanup.md` - Fix test-related issues\n");
                prompt.push_str("   - `.claude/commands/review-changes.md` - Apply best practices\n");
                prompt.push_str("   - `.claude/commands/cleanup.md` - Final lint pass\n");
                prompt.push_str("   - `.claude/commands/review-changes.md` - Second review pass\n");
                prompt.push_str("   - `.claude/commands/add-and-commit.md` - Stage and commit\n");
            }
        }
        prompt.push('\n');
    } else {
        prompt.push_str("## Instructions\n\n");
        prompt.push_str("1. Carefully read and understand the task requirements\n");
        prompt.push_str("2. Implement the requested changes\n");
        prompt.push_str("3. Test your changes where appropriate\n");
        prompt.push_str("4. Commit your changes with a descriptive message\n");
    }

    prompt
}

#[allow(dead_code)]
pub fn generate_custom_prompt(ticket: &Ticket, template: &str) -> String {
    let mut result = template.to_string();
    result = result.replace("{{title}}", &ticket.title);
    result = result.replace("{{description}}", &ticket.description_md);
    result = result.replace("{{priority}}", ticket.priority.as_str());
    result = result.replace("{{labels}}", &ticket.labels.join(", "));
    result
}

#[allow(dead_code)]
pub fn generate_system_prompt(api_url: &str, ticket_id: &str, run_id: &str) -> String {
    format!(
        r#"You are an AI coding agent working on a task from a Kanban board.

## Task Context
- Ticket ID: {ticket_id}
- Run ID: {run_id}
- API URL: {api_url}

## Guidelines
1. Focus on completing the task as described
2. Make incremental changes and test frequently
3. Write clear commit messages
4. If you encounter blockers, document them clearly

## Communication
Your actions are being tracked via hooks. The board will be automatically updated as you work.
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
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
            agent_pref: None,
        }
    }

    #[test]
    fn slugify_simple_title() {
        assert_eq!(slugify("Hello World"), "hello-world");
    }

    #[test]
    fn slugify_special_characters() {
        assert_eq!(slugify("Add user@auth feature!"), "add-user-auth-feature");
    }

    #[test]
    fn slugify_multiple_spaces() {
        assert_eq!(slugify("Fix   multiple   spaces"), "fix-multiple-spaces");
    }

    #[test]
    fn slugify_long_title_truncates() {
        let long_title = "A".repeat(100);
        let result = slugify(&long_title);
        assert!(result.len() <= 50);
    }

    #[test]
    fn generate_ticket_prompt_includes_title() {
        let ticket = create_test_ticket();
        let prompt = generate_ticket_prompt(&ticket);
        assert!(prompt.contains("# Task: Test Ticket"));
    }

    #[test]
    fn generate_ticket_prompt_includes_description() {
        let ticket = create_test_ticket();
        let prompt = generate_ticket_prompt(&ticket);
        assert!(prompt.contains("This is a test description."));
    }

    #[test]
    fn generate_ticket_prompt_includes_labels() {
        let ticket = create_test_ticket();
        let prompt = generate_ticket_prompt(&ticket);
        assert!(prompt.contains("- bug"));
        assert!(prompt.contains("- urgent"));
    }

    #[test]
    fn generate_ticket_prompt_includes_instructions() {
        let ticket = create_test_ticket();
        let prompt = generate_ticket_prompt(&ticket);
        assert!(prompt.contains("## Instructions"));
        assert!(prompt.contains("Commit your changes"));
    }

    #[test]
    fn generate_ticket_prompt_urgent_priority() {
        let mut ticket = create_test_ticket();
        ticket.priority = Priority::Urgent;
        let prompt = generate_ticket_prompt(&ticket);
        assert!(prompt.contains("URGENT"));
    }

    #[test]
    fn generate_ticket_prompt_high_priority() {
        let mut ticket = create_test_ticket();
        ticket.priority = Priority::High;
        let prompt = generate_ticket_prompt(&ticket);
        assert!(prompt.contains("high-priority"));
    }

    #[test]
    fn generate_ticket_prompt_empty_description() {
        let mut ticket = create_test_ticket();
        ticket.description_md = String::new();
        let prompt = generate_ticket_prompt(&ticket);
        // Should not have description section
        assert!(!prompt.contains("## Description"));
    }

    #[test]
    fn generate_ticket_prompt_empty_labels() {
        let mut ticket = create_test_ticket();
        ticket.labels = vec![];
        let prompt = generate_ticket_prompt(&ticket);
        // Should not have labels section
        assert!(!prompt.contains("## Labels"));
    }

    #[test]
    fn generate_custom_prompt_replaces_placeholders() {
        let ticket = create_test_ticket();
        let template = "Title: {{title}}, Priority: {{priority}}";
        let result = generate_custom_prompt(&ticket, template);
        assert_eq!(result, "Title: Test Ticket, Priority: medium");
    }

    #[test]
    fn generate_system_prompt_includes_context() {
        let prompt = generate_system_prompt("http://localhost:7432", "ticket-1", "run-1");
        assert!(prompt.contains("ticket-1"));
        assert!(prompt.contains("run-1"));
        assert!(prompt.contains("http://localhost:7432"));
    }

    #[test]
    fn generate_ticket_prompt_low_priority() {
        let mut ticket = create_test_ticket();
        ticket.priority = Priority::Low;
        let prompt = generate_ticket_prompt(&ticket);
        assert!(prompt.contains("low-priority"));
    }

    #[test]
    fn generate_ticket_prompt_medium_priority_no_context() {
        let mut ticket = create_test_ticket();
        ticket.priority = Priority::Medium;
        let prompt = generate_ticket_prompt(&ticket);
        // Medium priority should not add any priority context
        assert!(!prompt.contains("URGENT"));
        assert!(!prompt.contains("high-priority"));
        assert!(!prompt.contains("low-priority"));
    }

    #[test]
    fn generate_custom_prompt_with_labels() {
        let ticket = create_test_ticket();
        let template = "Labels: {{labels}}";
        let result = generate_custom_prompt(&ticket, template);
        assert_eq!(result, "Labels: bug, urgent");
    }

    #[test]
    fn generate_ticket_prompt_with_workflow_cursor() {
        let ticket = create_test_ticket();
        let prompt = generate_ticket_prompt_with_workflow(&ticket, Some(AgentKind::Cursor));
        assert!(prompt.contains("## Workflow"));
        assert!(prompt.contains("Create a branch:"));
        assert!(prompt.contains("/deslop"));
        assert!(prompt.contains("/cleanup"));
        assert!(prompt.contains("/unit-tests"));
        assert!(prompt.contains("/review-changes"));
        assert!(prompt.contains("/add-and-commit"));
    }

    #[test]
    fn generate_ticket_prompt_with_workflow_claude() {
        let ticket = create_test_ticket();
        let prompt = generate_ticket_prompt_with_workflow(&ticket, Some(AgentKind::Claude));
        assert!(prompt.contains("## Workflow"));
        assert!(prompt.contains("Create a branch:"));
        assert!(prompt.contains(".claude/commands/deslop.md"));
        assert!(prompt.contains(".claude/commands/cleanup.md"));
        assert!(prompt.contains(".claude/commands/unit-tests.md"));
        assert!(prompt.contains(".claude/commands/review-changes.md"));
        assert!(prompt.contains(".claude/commands/add-and-commit.md"));
    }

    #[test]
    fn generate_ticket_prompt_with_workflow_none_uses_basic_instructions() {
        let ticket = create_test_ticket();
        let prompt = generate_ticket_prompt_with_workflow(&ticket, None);
        assert!(prompt.contains("## Instructions"));
        assert!(!prompt.contains("## Workflow"));
    }

    #[test]
    fn generate_ticket_prompt_with_workflow_includes_branch_name() {
        let mut ticket = create_test_ticket();
        ticket.id = "abc12345-full-id".to_string();
        ticket.title = "Add User Authentication".to_string();
        let prompt = generate_ticket_prompt_with_workflow(&ticket, Some(AgentKind::Cursor));
        assert!(prompt.contains("ticket/abc12345/add-user-authentication"));
    }
}
