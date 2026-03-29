//! Ticket prompt generation functions.

use super::utils::slugify;
use crate::agents::provider::AgentProvider;
use crate::db::models::{Priority, Ticket};

/// Generate workspace context section for multi-project tickets.
/// Callers should omit this for single-project (non-workspace) tickets.
pub fn generate_workspace_context(
    workspace_name: &str,
    projects: &[(String, String)], // (name, path) pairs
) -> String {
    let mut ctx = String::new();
    ctx.push_str(&format!("## Workspace: {}\n\n", workspace_name));
    ctx.push_str("This ticket spans multiple projects:\n");
    for (name, path) in projects {
        ctx.push_str(&format!("- **{}** ({})\n", name, path));
    }
    ctx.push_str("\nYou have access to all projects simultaneously. Make coordinated changes across projects as needed.\n\n");
    ctx
}

pub fn generate_ticket_prompt(ticket: &Ticket) -> String {
    generate_ticket_prompt_with_workflow(ticket, None, None)
}

/// Generate a ticket prompt with optional workflow instructions for the given agent provider
pub fn generate_ticket_prompt_with_workflow(
    ticket: &Ticket,
    provider: Option<&dyn AgentProvider>,
    workspace: Option<(&str, &[(String, String)])>,
) -> String {
    generate_ticket_prompt_full(ticket, provider, true, workspace)
}

/// Generate a ticket prompt with full control over workflow options
pub fn generate_ticket_prompt_full(
    ticket: &Ticket,
    provider: Option<&dyn AgentProvider>,
    requires_git: bool,
    workspace: Option<(&str, &[(String, String)])>,
) -> String {
    let mut prompt = String::new();

    prompt.push_str(&format!("# Task: {}\n\n", ticket.title));

    if let Some((name, projects)) = workspace {
        prompt.push_str(&generate_workspace_context(name, projects));
    }

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

/// Build a lightweight ticket-intent section for code-review prompts.
/// Includes the ticket's title, priority, labels, and description so the
/// reviewer understands what the code changes are *supposed* to accomplish,
/// without any workflow or implementation instructions.
pub fn build_code_review_ticket_context(ticket: &Ticket) -> String {
    let mut ctx = String::new();

    ctx.push_str("## Ticket Intent\n\n");
    ctx.push_str(&format!("**Title:** {}\n", ticket.title));

    let priority_label = match ticket.priority {
        Priority::Urgent => "Urgent",
        Priority::High => "High",
        Priority::Medium => "Medium",
        Priority::Low => "Low",
    };
    ctx.push_str(&format!("**Priority:** {}\n", priority_label));

    if !ticket.labels.is_empty() {
        ctx.push_str(&format!("**Labels:** {}\n", ticket.labels.join(", ")));
    }

    if !ticket.description_md.is_empty() {
        ctx.push_str("\n### Description\n\n");
        ctx.push_str(&ticket.description_md);
        ctx.push('\n');
    }

    ctx.push('\n');
    ctx
}

/// Generate a prompt for the planning stage
pub fn generate_plan_prompt(
    ticket: &Ticket,
    workspace: Option<(&str, &[(String, String)])>,
) -> String {
    let mut prompt = String::new();

    prompt.push_str("Create an implementation plan for this task.\n\n");
    prompt.push_str(&format!("# Task: {}\n\n", ticket.title));

    if let Some((name, projects)) = workspace {
        prompt.push_str(&generate_workspace_context(name, projects));
    }

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

/// Generate a prompt to decompose a plan into focused implementation todos.
pub fn generate_plan_decomposition_prompt(plan: &str) -> String {
    format!(
        r#"You are a technical project decomposer. Break the following implementation plan into small, focused implementation steps (todos).

## Implementation Plan

{plan}

## Instructions

Decompose this plan into individual implementation todos. You MUST produce at least 2 todos for any non-trivial task. The only exception is a genuinely trivial change — for example, a single-line fix, a typo correction, or renaming one symbol in one file. If in doubt, produce more todos rather than fewer.

Each todo should be:
- **Independently implementable**: Can be done without depending on later todos
- **Small-scoped**: Focused on a single logical change (e.g. one file, one feature, one component)
- **Ordered**: Listed in the order they should be implemented
- **Specific**: Include file paths, function names, and concrete approach

Guidelines for splitting:
- Separate type/interface changes from logic changes
- Separate backend changes from frontend changes
- Separate new code from refactoring existing code
- Separate test additions from implementation

Produce between 2 and 10 todos. Only return a single todo if the entire task is a one-line change.

Output a JSON array of objects with "title" and "description" fields:

```json
[
  {{
    "title": "Short descriptive title",
    "description": "Detailed breakdown including:\n- Which files to modify/create\n- What changes to make\n- Acceptance criteria"
  }}
]
```

Return ONLY the JSON array. Do not include any other text.
"#
    )
}

/// Generate a focused implement prompt for a single todo item.
pub fn generate_todo_implement_prompt(
    ticket: &Ticket,
    plan: &str,
    todo_title: &str,
    todo_description: &str,
    todo_index: usize,
    todo_total: usize,
    workspace: Option<(&str, &[(String, String)])>,
) -> String {
    let mut prompt = String::new();

    prompt.push_str(&format!("# Task: {}\n\n", ticket.title));

    if let Some((name, projects)) = workspace {
        prompt.push_str(&generate_workspace_context(name, projects));
    }

    if !ticket.description_md.is_empty() {
        prompt.push_str("## Description\n\n");
        prompt.push_str(&ticket.description_md);
        prompt.push_str("\n\n");
    }

    prompt.push_str("## Full Implementation Plan (for context)\n\n");
    prompt.push_str(plan);
    prompt.push_str("\n\n");

    prompt.push_str(&format!(
        "## Current Step ({}/{}): {}\n\n",
        todo_index + 1,
        todo_total,
        todo_title
    ));
    prompt.push_str(todo_description);
    prompt.push_str("\n\n");

    prompt.push_str(&format!(
        r#"## Instructions

You are implementing step {current} of {total} in the plan above.

Focus ONLY on this step: **{title}**

1. Make the necessary code changes for this step
2. Verify the changes compile/pass type checking
3. Do NOT work on other steps — they will be handled separately

Do NOT:
- Run the full QA sequence (that comes in later stages)
- Commit changes (that comes later)
- Add tests (that comes in a separate stage)
- Implement steps beyond the current one

Just implement this specific step as described.
"#,
        current = todo_index + 1,
        total = todo_total,
        title = todo_title,
    ));

    prompt
}

/// Generate a prompt for the implementation stage
pub fn generate_implement_prompt(
    ticket: &Ticket,
    plan: &str,
    workspace: Option<(&str, &[(String, String)])>,
) -> String {
    let mut prompt = String::new();

    prompt.push_str(&format!("# Task: {}\n\n", ticket.title));

    if let Some((name, projects)) = workspace {
        prompt.push_str(&generate_workspace_context(name, projects));
    }

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
            workspace_id: None,
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
    fn generate_workspace_context_lists_projects() {
        let projects = vec![
            ("api".to_string(), "/path/api".to_string()),
            ("web".to_string(), "/path/web".to_string()),
        ];
        let s = generate_workspace_context("My Workspace", &projects);
        assert!(s.contains("## Workspace: My Workspace"));
        assert!(s.contains("**api** (/path/api)"));
        assert!(s.contains("**web** (/path/web)"));
        assert!(s.contains("coordinated changes across projects"));
    }

    #[test]
    fn generate_ticket_prompt_full_inserts_workspace_after_title() {
        let ticket = create_test_ticket();
        let cursor = CursorProvider::new();
        let projs = [("svc".to_string(), "/svc".to_string())];
        let prompt = generate_ticket_prompt_full(
            &ticket,
            Some(&cursor),
            true,
            Some(("WS", &projs)),
        );
        let title_pos = prompt.find("# Task: Test Ticket").unwrap();
        let ws_pos = prompt.find("## Workspace: WS").unwrap();
        let desc_pos = prompt.find("## Description").unwrap();
        assert!(title_pos < ws_pos);
        assert!(ws_pos < desc_pos);
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
        let prompt = generate_ticket_prompt_with_workflow(&ticket, Some(&cursor), None);
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
        let prompt = generate_ticket_prompt_with_workflow(&ticket, Some(&claude), None);
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
        let prompt = generate_ticket_prompt_with_workflow(&ticket, None, None);
        assert!(prompt.contains("## Instructions"));
        assert!(!prompt.contains("## Workflow"));
    }

    #[test]
    fn generate_ticket_prompt_with_workflow_includes_branch_name() {
        let mut ticket = create_test_ticket();
        ticket.id = "abc12345-full-id".to_string();
        ticket.title = "Add User Authentication".to_string();
        let cursor = CursorProvider::new();
        let prompt = generate_ticket_prompt_with_workflow(&ticket, Some(&cursor), None);
        assert!(prompt.contains("ticket/abc12345/add-user-authentication"));
    }

    #[test]
    fn generate_ticket_prompt_with_workflow_handles_multibyte_utf8_id() {
        let mut ticket = create_test_ticket();
        ticket.id = "🎉🚀ab12".to_string();
        ticket.title = "Test Feature".to_string();
        let cursor = CursorProvider::new();
        let prompt = generate_ticket_prompt_with_workflow(&ticket, Some(&cursor), None);
        assert!(prompt.contains("ticket/🎉🚀ab12/test-feature"));
    }

    #[test]
    fn generate_ticket_prompt_with_workflow_handles_short_id() {
        let mut ticket = create_test_ticket();
        ticket.id = "abc".to_string();
        ticket.title = "Short ID Test".to_string();
        let cursor = CursorProvider::new();
        let prompt = generate_ticket_prompt_with_workflow(&ticket, Some(&cursor), None);
        assert!(prompt.contains("ticket/abc/short-id-test"));
    }

    #[test]
    fn generate_ticket_prompt_with_workflow_handles_mixed_utf8_id() {
        let mut ticket = create_test_ticket();
        ticket.id = "a🎉bcdefgh".to_string();
        ticket.title = "Mixed Test".to_string();
        let cursor = CursorProvider::new();
        let prompt = generate_ticket_prompt_with_workflow(&ticket, Some(&cursor), None);
        assert!(prompt.contains("ticket/a🎉bcdefg/mixed-test"));
    }

    #[test]
    fn generate_ticket_prompt_full_without_git_cursor() {
        let ticket = create_test_ticket();
        let cursor = CursorProvider::new();
        let prompt = generate_ticket_prompt_full(&ticket, Some(&cursor), false, None);

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
        let prompt = generate_ticket_prompt_full(&ticket, Some(&claude), false, None);

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
        let prompt = generate_ticket_prompt_full(&ticket, Some(&cursor), true, None);

        // Should have all workflow steps including git
        assert!(prompt.contains("Create a branch:"));
        assert!(prompt.contains("/add-and-commit"));
    }

    #[test]
    fn generate_plan_decomposition_prompt_includes_plan() {
        let plan = "## Steps\n1. Add module\n2. Write tests";
        let prompt = generate_plan_decomposition_prompt(plan);
        assert!(prompt.contains(plan));
        assert!(prompt.contains("## Implementation Plan"));
        assert!(prompt.contains("2 and 10"));
        assert!(prompt.contains("\"title\""));
        assert!(prompt.contains("\"description\""));
    }

    #[test]
    fn generate_plan_decomposition_prompt_returns_nonempty() {
        let prompt = generate_plan_decomposition_prompt("");
        assert!(!prompt.is_empty());
        assert!(prompt.contains("## Implementation Plan"));
    }

    #[test]
    fn generate_todo_implement_prompt_includes_ticket_and_step() {
        let ticket = create_test_ticket();
        let plan = "1. Modify file_a.rs\n2. Modify file_b.rs";
        let prompt = generate_todo_implement_prompt(
            &ticket, plan, "Add API endpoint", "Create GET /api/items", 0, 3, None,
        );
        assert!(prompt.contains("# Task: Test Ticket"));
        assert!(prompt.contains("## Description"));
        assert!(prompt.contains("This is a test description."));
        assert!(prompt.contains("## Full Implementation Plan"));
        assert!(prompt.contains(plan));
        assert!(prompt.contains("## Current Step (1/3): Add API endpoint"));
        assert!(prompt.contains("Create GET /api/items"));
        assert!(prompt.contains("step 1 of 3"));
        assert!(prompt.contains("**Add API endpoint**"));
    }

    #[test]
    fn generate_todo_implement_prompt_step_numbering_last() {
        let ticket = create_test_ticket();
        let prompt = generate_todo_implement_prompt(
            &ticket, "plan", "Final step", "Cleanup", 4, 5, None,
        );
        assert!(prompt.contains("## Current Step (5/5): Final step"));
        assert!(prompt.contains("step 5 of 5"));
    }

    #[test]
    fn generate_todo_implement_prompt_empty_description_omits_section() {
        let mut ticket = create_test_ticket();
        ticket.description_md = String::new();
        let prompt = generate_todo_implement_prompt(
            &ticket, "plan", "Step", "Do things", 0, 1, None,
        );
        assert!(!prompt.contains("## Description"));
        assert!(prompt.contains("# Task: Test Ticket"));
        assert!(prompt.contains("## Current Step (1/1): Step"));
    }

    #[test]
    fn generate_todo_implement_prompt_contains_scope_instructions() {
        let ticket = create_test_ticket();
        let prompt = generate_todo_implement_prompt(
            &ticket, "plan", "One thing", "desc", 0, 2, None,
        );
        assert!(prompt.contains("Focus ONLY on this step"));
        assert!(prompt.contains("Do NOT work on other steps"));
        assert!(prompt.contains("Do NOT"));
    }

    #[test]
    fn build_code_review_ticket_context_includes_title_and_priority() {
        let ticket = create_test_ticket();
        let ctx = build_code_review_ticket_context(&ticket);
        assert!(ctx.contains("## Ticket Intent"));
        assert!(ctx.contains("**Title:** Test Ticket"));
        assert!(ctx.contains("**Priority:** Medium"));
    }

    #[test]
    fn build_code_review_ticket_context_includes_labels() {
        let ticket = create_test_ticket();
        let ctx = build_code_review_ticket_context(&ticket);
        assert!(ctx.contains("**Labels:** bug, urgent"));
    }

    #[test]
    fn build_code_review_ticket_context_includes_description() {
        let ticket = create_test_ticket();
        let ctx = build_code_review_ticket_context(&ticket);
        assert!(ctx.contains("### Description"));
        assert!(ctx.contains("This is a test description."));
    }

    #[test]
    fn build_code_review_ticket_context_omits_description_when_empty() {
        let mut ticket = create_test_ticket();
        ticket.description_md = String::new();
        let ctx = build_code_review_ticket_context(&ticket);
        assert!(!ctx.contains("### Description"));
    }

    #[test]
    fn build_code_review_ticket_context_omits_labels_when_empty() {
        let mut ticket = create_test_ticket();
        ticket.labels = vec![];
        let ctx = build_code_review_ticket_context(&ticket);
        assert!(!ctx.contains("**Labels:**"));
    }

    #[test]
    fn build_code_review_ticket_context_urgent_priority() {
        let mut ticket = create_test_ticket();
        ticket.priority = Priority::Urgent;
        let ctx = build_code_review_ticket_context(&ticket);
        assert!(ctx.contains("**Priority:** Urgent"));
    }

    #[test]
    fn build_code_review_ticket_context_high_priority() {
        let mut ticket = create_test_ticket();
        ticket.priority = Priority::High;
        let ctx = build_code_review_ticket_context(&ticket);
        assert!(ctx.contains("**Priority:** High"));
    }

    #[test]
    fn build_code_review_ticket_context_low_priority() {
        let mut ticket = create_test_ticket();
        ticket.priority = Priority::Low;
        let ctx = build_code_review_ticket_context(&ticket);
        assert!(ctx.contains("**Priority:** Low"));
    }

    #[test]
    fn build_code_review_ticket_context_has_no_workflow_instructions() {
        let ticket = create_test_ticket();
        let ctx = build_code_review_ticket_context(&ticket);
        assert!(!ctx.contains("## Workflow"));
        assert!(!ctx.contains("## Instructions"));
        assert!(!ctx.contains("Implementation Plan"));
        assert!(!ctx.contains("Create a branch"));
    }

    #[test]
    fn build_code_review_ticket_context_section_ordering() {
        let ticket = create_test_ticket();
        let ctx = build_code_review_ticket_context(&ticket);
        let intent_pos = ctx.find("## Ticket Intent").unwrap();
        let title_pos = ctx.find("**Title:**").unwrap();
        let priority_pos = ctx.find("**Priority:**").unwrap();
        let labels_pos = ctx.find("**Labels:**").unwrap();
        let desc_pos = ctx.find("### Description").unwrap();
        assert!(intent_pos < title_pos);
        assert!(title_pos < priority_pos);
        assert!(priority_pos < labels_pos);
        assert!(labels_pos < desc_pos);
    }

    #[test]
    fn build_code_review_ticket_context_single_label() {
        let mut ticket = create_test_ticket();
        ticket.labels = vec!["frontend".to_string()];
        let ctx = build_code_review_ticket_context(&ticket);
        assert!(ctx.contains("**Labels:** frontend"));
        assert!(!ctx.contains(", "));
    }

    #[test]
    fn generate_plan_prompt_with_workspace_inserts_after_title() {
        let ticket = create_test_ticket();
        let projs = [("api".to_string(), "/api".to_string())];
        let prompt = generate_plan_prompt(&ticket, Some(("WS", &projs)));

        let title_pos = prompt.find("# Task:").unwrap();
        let ws_pos = prompt.find("## Workspace: WS").unwrap();
        let desc_pos = prompt.find("## Description").unwrap();
        assert!(title_pos < ws_pos);
        assert!(ws_pos < desc_pos);
        assert!(prompt.contains("spans multiple projects"));
        assert!(prompt.contains("**api** (/api)"));
    }

    #[test]
    fn generate_plan_prompt_without_workspace_has_no_workspace_section() {
        let ticket = create_test_ticket();
        let prompt = generate_plan_prompt(&ticket, None);
        assert!(!prompt.contains("## Workspace"));
        assert!(prompt.contains("# Task:"));
        assert!(prompt.contains("## Description"));
    }

    #[test]
    fn generate_implement_prompt_with_workspace_inserts_after_title() {
        let ticket = create_test_ticket();
        let projs = [("web".to_string(), "/web".to_string())];
        let prompt = generate_implement_prompt(&ticket, "do stuff", Some(("WS", &projs)));

        let title_pos = prompt.find("# Task:").unwrap();
        let ws_pos = prompt.find("## Workspace: WS").unwrap();
        let desc_pos = prompt.find("## Description").unwrap();
        assert!(title_pos < ws_pos);
        assert!(ws_pos < desc_pos);
        assert!(prompt.contains("coordinated changes"));
        assert!(prompt.contains("**web** (/web)"));
        assert!(prompt.contains("do stuff"));
    }

    #[test]
    fn generate_implement_prompt_without_workspace_has_no_workspace_section() {
        let ticket = create_test_ticket();
        let prompt = generate_implement_prompt(&ticket, "plan text", None);
        assert!(!prompt.contains("## Workspace"));
        assert!(prompt.contains("# Task:"));
        assert!(prompt.contains("plan text"));
    }

    #[test]
    fn generate_todo_implement_prompt_with_workspace_inserts_after_title() {
        let ticket = create_test_ticket();
        let projs = [
            ("svc".to_string(), "/svc".to_string()),
            ("lib".to_string(), "/lib".to_string()),
        ];
        let prompt = generate_todo_implement_prompt(
            &ticket, "plan", "Step A", "Do step A", 0, 2, Some(("WS", &projs)),
        );

        let title_pos = prompt.find("# Task:").unwrap();
        let ws_pos = prompt.find("## Workspace: WS").unwrap();
        let desc_pos = prompt.find("## Description").unwrap();
        assert!(title_pos < ws_pos);
        assert!(ws_pos < desc_pos);
        assert!(prompt.contains("**svc** (/svc)"));
        assert!(prompt.contains("**lib** (/lib)"));
        assert!(prompt.contains("## Current Step (1/2): Step A"));
    }
}
