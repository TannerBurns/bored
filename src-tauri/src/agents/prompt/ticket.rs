//! Ticket prompt generation functions.

use super::utils::slugify;
use crate::agents::provider::AgentProvider;
use crate::db::models::{Priority, Ticket};

pub fn generate_ticket_prompt(ticket: &Ticket) -> String {
    generate_ticket_prompt_with_workflow(ticket, None)
}

/// Generate a ticket prompt with optional workflow instructions for the given agent provider
pub fn generate_ticket_prompt_with_workflow(
    ticket: &Ticket,
    provider: Option<&dyn AgentProvider>,
) -> String {
    generate_ticket_prompt_full(ticket, provider, true)
}

/// Generate a ticket prompt with full control over workflow options
pub fn generate_ticket_prompt_full(
    ticket: &Ticket,
    provider: Option<&dyn AgentProvider>,
    requires_git: bool,
) -> String {
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

    if let Some(p) = provider {
        prompt.push_str("## Workflow\n\n");

        let mut step = 1;

        if requires_git {
            let id_prefix: String = ticket.id.chars().take(8).collect();
            let branch_name = format!("ticket/{}/{}", id_prefix, slugify(&ticket.title));
            prompt.push_str(&format!("{}. Create a branch: `{}`\n", step, branch_name));
            step += 1;
        }

        prompt.push_str(&format!("{}. Create a plan before implementing\n", step));
        step += 1;
        prompt.push_str(&format!(
            "{}. After implementation, run this QA sequence:\n\n",
            step
        ));

        let descriptions = [
            ("deslop", "Remove AI-generated code patterns"),
            ("cleanup", "Fix lint/type errors"),
            ("unit-tests", "Add test coverage for your changes"),
            ("cleanup", "Fix any test-related issues"),
            ("review-changes", "Apply best practices"),
            ("cleanup", "Final lint pass"),
            ("review-changes", "Second review pass"),
        ];

        for (command, description) in &descriptions {
            let reference = p.format_command_reference(command);
            prompt.push_str(&format!("   - `{}` - {}\n", reference, description));
        }

        if requires_git {
            let commit_ref = p.format_command_reference("add-and-commit");
            prompt.push_str(&format!(
                "   - `{}` - Stage and commit with detailed message\n",
                commit_ref
            ));
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

pub fn generate_custom_prompt(ticket: &Ticket, template: &str) -> String {
    let mut result = template.to_string();
    result = result.replace("{{title}}", &ticket.title);
    result = result.replace("{{description}}", &ticket.description_md);
    result = result.replace("{{priority}}", ticket.priority.as_str());
    result = result.replace("{{labels}}", &ticket.labels.join(", "));
    result
}

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
3. Write commit messages in Conventional Commits (commitizen) format: `<type>(<scope>): <description>`
4. If you encounter blockers, document them clearly

## Communication
The board will be automatically updated as you work.
"#
    )
}

/// Generate a prompt for the planning stage
pub fn generate_plan_prompt(ticket: &Ticket) -> String {
    let mut prompt = String::new();

    prompt.push_str("Create an implementation plan for this task.\n\n");
    prompt.push_str(&format!("# Task: {}\n\n", ticket.title));

    if !ticket.description_md.is_empty() {
        prompt.push_str("## Description\n\n");
        prompt.push_str(&ticket.description_md);
        prompt.push_str("\n\n");
    }

    let priority_context = match ticket.priority {
        Priority::Urgent => "This is an URGENT task. Prioritize a minimal viable solution.",
        Priority::High => "This is a high-priority task.",
        Priority::Medium => "",
        Priority::Low => "This is a low-priority task. Plan thoroughly.",
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

    prompt.push_str(
        r#"## Instructions

1. Analyze the task requirements
2. Identify the files that need to be modified or created
3. Break down the implementation into numbered steps
4. Consider edge cases and potential issues
5. Output a clear, actionable plan

Format your plan as:
```
## Implementation Plan

### Files to Modify
- file1.rs - reason
- file2.ts - reason

### Steps
1. Step description
2. Step description
...

### Testing Strategy
- How to verify the implementation works
```

Do NOT implement any code. Just create the plan.
"#,
    );

    prompt
}

/// Generate a prompt for the implementation stage
pub fn generate_implement_prompt(ticket: &Ticket, plan: &str) -> String {
    let mut prompt = String::new();

    prompt.push_str(&format!("# Task: {}\n\n", ticket.title));

    if !ticket.description_md.is_empty() {
        prompt.push_str("## Description\n\n");
        prompt.push_str(&ticket.description_md);
        prompt.push_str("\n\n");
    }

    prompt.push_str("## Implementation Plan\n\n");
    prompt.push_str(plan);
    prompt.push_str("\n\n");

    prompt.push_str(
        r#"## Instructions

Execute the implementation plan above. For each step:
1. Make the necessary code changes
2. Verify the changes compile/pass type checking
3. Move to the next step

Focus on implementing the plan. Do NOT:
- Run the full QA sequence (that comes in the next stages)
- Commit changes (that comes later)
- Add tests (that comes in a separate stage)

Just implement the core functionality as described in the plan.
"#,
    );

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::claude::provider::ClaudeProvider;
    use crate::agents::cursor::provider::CursorProvider;
    use crate::db::models::WorkflowType;
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
        let cursor = CursorProvider::new();
        let prompt = generate_ticket_prompt_with_workflow(&ticket, Some(&cursor));
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
        let claude = ClaudeProvider::new();
        let prompt = generate_ticket_prompt_with_workflow(&ticket, Some(&claude));
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
        let cursor = CursorProvider::new();
        let prompt = generate_ticket_prompt_with_workflow(&ticket, Some(&cursor));
        assert!(prompt.contains("ticket/abc12345/add-user-authentication"));
    }

    #[test]
    fn generate_ticket_prompt_with_workflow_handles_multibyte_utf8_id() {
        let mut ticket = create_test_ticket();
        ticket.id = "🎉🚀ab12".to_string();
        ticket.title = "Test Feature".to_string();
        let cursor = CursorProvider::new();
        let prompt = generate_ticket_prompt_with_workflow(&ticket, Some(&cursor));
        assert!(prompt.contains("ticket/🎉🚀ab12/test-feature"));
    }

    #[test]
    fn generate_ticket_prompt_with_workflow_handles_short_id() {
        let mut ticket = create_test_ticket();
        ticket.id = "abc".to_string();
        ticket.title = "Short ID Test".to_string();
        let cursor = CursorProvider::new();
        let prompt = generate_ticket_prompt_with_workflow(&ticket, Some(&cursor));
        assert!(prompt.contains("ticket/abc/short-id-test"));
    }

    #[test]
    fn generate_ticket_prompt_with_workflow_handles_mixed_utf8_id() {
        let mut ticket = create_test_ticket();
        ticket.id = "a🎉bcdefgh".to_string();
        ticket.title = "Mixed Test".to_string();
        let cursor = CursorProvider::new();
        let prompt = generate_ticket_prompt_with_workflow(&ticket, Some(&cursor));
        assert!(prompt.contains("ticket/a🎉bcdefg/mixed-test"));
    }

    #[test]
    fn generate_ticket_prompt_full_without_git_cursor() {
        let ticket = create_test_ticket();
        let cursor = CursorProvider::new();
        let prompt = generate_ticket_prompt_full(&ticket, Some(&cursor), false);

        // Should have workflow section
        assert!(prompt.contains("## Workflow"));

        // Should NOT have git-related steps
        assert!(!prompt.contains("Create a branch:"));
        assert!(!prompt.contains("/add-and-commit"));

        // Should still have non-git workflow steps
        assert!(prompt.contains("/deslop"));
        assert!(prompt.contains("/cleanup"));
        assert!(prompt.contains("/unit-tests"));
        assert!(prompt.contains("/review-changes"));
    }

    #[test]
    fn generate_ticket_prompt_full_without_git_claude() {
        let ticket = create_test_ticket();
        let claude = ClaudeProvider::new();
        let prompt = generate_ticket_prompt_full(&ticket, Some(&claude), false);

        // Should have workflow section
        assert!(prompt.contains("## Workflow"));

        // Should NOT have git-related steps
        assert!(!prompt.contains("Create a branch:"));
        assert!(!prompt.contains("add-and-commit.md"));

        // Should still have non-git workflow steps
        assert!(prompt.contains("deslop.md"));
        assert!(prompt.contains("cleanup.md"));
        assert!(prompt.contains("unit-tests.md"));
    }

    #[test]
    fn generate_ticket_prompt_full_with_git_includes_all_steps() {
        let ticket = create_test_ticket();
        let cursor = CursorProvider::new();
        let prompt = generate_ticket_prompt_full(&ticket, Some(&cursor), true);

        // Should have all workflow steps including git
        assert!(prompt.contains("Create a branch:"));
        assert!(prompt.contains("/add-and-commit"));
    }
}
