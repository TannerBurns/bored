use crate::db::models::{Priority, Ticket};

pub fn generate_ticket_prompt(ticket: &Ticket) -> String {
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

    prompt.push_str("## Instructions\n\n");
    prompt.push_str("1. Carefully read and understand the task requirements\n");
    prompt.push_str("2. Implement the requested changes\n");
    prompt.push_str("3. Test your changes where appropriate\n");
    prompt.push_str("4. Commit your changes with a descriptive message\n");

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
}
