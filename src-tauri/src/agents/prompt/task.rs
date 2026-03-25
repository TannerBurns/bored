//! Task-based prompt generation.

use std::path::Path;

use crate::db::models::{Priority, Task, TaskType, Ticket};

/// Generate workspace context for task prompts.
pub fn generate_workspace_task_context(
    workspace_name: &str,
    projects: &[(String, String)],
) -> String {
    let mut ctx = String::new();
    ctx.push_str(&format!("## Workspace: {}\n\n", workspace_name));
    ctx.push_str("This task is part of a multi-project workspace:\n");
    for (name, path) in projects {
        ctx.push_str(&format!("- **{}** ({})\n", name, path));
    }
    ctx.push_str("\nCoordinate changes across projects as needed.\n\n");
    ctx
}

/// Generate a prompt for executing a task
/// This is the main entry point for task-based prompt generation
pub fn generate_task_prompt(
    task: &Task,
    ticket: &Ticket,
    custom_commands_dir: Option<&Path>,
    workspace: Option<(&str, &[(String, String)])>,
) -> String {
    match &task.task_type {
        TaskType::Custom => generate_custom_task_prompt(task, ticket, workspace),
        TaskType::Command(id) => {
            let mut body = generate_command_task_prompt(id, custom_commands_dir);
            if let Some((name, projects)) = workspace {
                let prefix = generate_workspace_task_context(name, projects);
                body = format!("{prefix}{body}");
            }
            body
        }
    }
}

/// Generate a prompt for a custom task
fn generate_custom_task_prompt(
    task: &Task,
    ticket: &Ticket,
    workspace: Option<(&str, &[(String, String)])>,
) -> String {
    let mut prompt = String::new();

    prompt.push_str(&format!("# Task: {}\n\n", ticket.title));

    if let Some((name, projects)) = workspace {
        prompt.push_str(&generate_workspace_task_context(name, projects));
    }

    if !ticket.description_md.is_empty() {
        prompt.push_str("## Ticket Context\n\n");
        prompt.push_str(&ticket.description_md);
        prompt.push_str("\n\n");
    }

    if let Some(ref content) = task.content {
        if !content.is_empty() {
            prompt.push_str("## Task Instructions\n\n");
            prompt.push_str(content);
            prompt.push_str("\n\n");
        }
    }

    let priority_context = match ticket.priority {
        Priority::Urgent => "This is an URGENT task. Prioritize a minimal viable solution.",
        Priority::High => "This is a high-priority task.",
        Priority::Medium => "",
        Priority::Low => "This is a low-priority task. Take time for quality.",
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
2. Create a plan before implementing
3. Implement the required changes
4. Verify the changes compile/pass type checking
5. Run the project's test suite if applicable

Focus on completing this specific task. Additional QA stages will follow.
"#,
    );

    prompt
}

/// Generate a prompt for a command-based task by reading the command file.
fn generate_command_task_prompt(
    command_id: &str,
    custom_commands_dir: Option<&Path>,
) -> String {
    let locations =
        super::workflow::build_command_search_paths(command_id, custom_commands_dir);

    for path in &locations {
        if let Ok(content) = std::fs::read_to_string(path) {
            return format!(
                "# Command Task: {}\n\n{}\n\nExecute these instructions carefully. When complete, report what was done.\n",
                command_id,
                content
            );
        }
    }

    get_fallback_command_prompt(command_id)
}

/// Get a fallback prompt for a command task when no `.md` file is found
fn get_fallback_command_prompt(command_id: &str) -> String {
    match command_id {
        "sync-with-main" => r#"# Sync with Main

Merge the latest changes from the main branch into this feature branch.

## Instructions

1. Fetch latest from origin: `git fetch origin main`
2. Merge main into current branch: `git merge origin/main`
3. Resolve any conflicts carefully
4. Run linter and type checker after resolving
5. Commit the merge
6. Push the changes
"#
        .to_string(),

        "add-tests" => r#"# Add Tests

Add comprehensive test coverage for the recent changes.

## Instructions

1. Identify what changed: `git diff main...HEAD`
2. Add unit tests for new functions
3. Test happy paths, edge cases, and error conditions
4. Ensure all tests pass
5. Follow existing test patterns in the codebase
"#
        .to_string(),

        "review-polish" => r#"# Review and Polish

Review all recent changes for code quality and best practices.

## Instructions

1. Review the diff from main
2. Check code readability and naming
3. Check error handling and logging
4. Check for security concerns
5. Remove unused code and fix formatting
6. Add documentation where helpful
"#
        .to_string(),

        "fix-lint" => r#"# Fix Lint Errors

Fix all linting and type checking errors.

## Instructions

1. Run the linter (eslint, clippy, etc.)
2. Run the type checker
3. Fix all errors
4. Verify fixes by re-running checks
5. Run tests to ensure fixes didn't break anything
"#
        .to_string(),

        _ => format!(
            r#"# Task: {}

Execute the {} task. Follow any project conventions for this task type.
"#,
            command_id, command_id
        ),
    }
}

/// Generate a planning prompt for a task
pub fn generate_task_plan_prompt(
    task: &Task,
    ticket: &Ticket,
    workspace: Option<(&str, &[(String, String)])>,
) -> String {
    let mut prompt = String::new();

    prompt.push_str("Create an implementation plan for this task.\n\n");
    prompt.push_str(&format!("# Task: {}\n\n", ticket.title));

    if let Some((name, projects)) = workspace {
        prompt.push_str(&generate_workspace_task_context(name, projects));
    }

    if !ticket.description_md.is_empty() {
        prompt.push_str("## Ticket Context\n\n");
        prompt.push_str(&ticket.description_md);
        prompt.push_str("\n\n");
    }

    if let Some(ref content) = task.content {
        if !content.is_empty() {
            prompt.push_str("## Task Requirements\n\n");
            prompt.push_str(content);
            prompt.push_str("\n\n");
        }
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

/// Generate an implementation prompt for a task with a plan
pub fn generate_task_implement_prompt(
    task: &Task,
    ticket: &Ticket,
    plan: &str,
    workspace: Option<(&str, &[(String, String)])>,
) -> String {
    let mut prompt = String::new();

    prompt.push_str(&format!("# Task: {}\n\n", ticket.title));

    if let Some((name, projects)) = workspace {
        prompt.push_str(&generate_workspace_task_context(name, projects));
    }

    if !ticket.description_md.is_empty() {
        prompt.push_str("## Ticket Context\n\n");
        prompt.push_str(&ticket.description_md);
        prompt.push_str("\n\n");
    }

    if let Some(ref content) = task.content {
        if !content.is_empty() {
            prompt.push_str("## Task Requirements\n\n");
            prompt.push_str(content);
            prompt.push_str("\n\n");
        }
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
- Add tests (that's a separate task)

Just implement the core functionality as described in the plan.
"#,
    );

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::{Priority, TaskStatus, WorkflowType};
    use chrono::Utc;

    fn create_test_ticket() -> Ticket {
        Ticket {
            id: "ticket-1".to_string(),
            board_id: "board-1".to_string(),
            column_id: "col-1".to_string(),
            title: "Test Ticket".to_string(),
            description_md: "This is a test description.".to_string(),
            priority: Priority::Medium,
            labels: vec!["bug".to_string()],
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

    fn create_test_task(task_type: TaskType) -> Task {
        Task {
            id: "task-1".to_string(),
            ticket_id: "ticket-1".to_string(),
            task_type,
            status: TaskStatus::Pending,
            content: Some("Custom task content".to_string()),
            title: None,
            order_index: 0,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            run_id: None,
        }
    }

    #[test]
    fn generate_workspace_task_context_lists_projects() {
        let projects = vec![("p1".to_string(), "/p1".to_string())];
        let s = generate_workspace_task_context("W", &projects);
        assert!(s.contains("## Workspace: W"));
        assert!(s.contains("multi-project workspace"));
        assert!(s.contains("**p1** (/p1)"));
    }

    #[test]
    fn generate_custom_task_prompt_includes_workspace_before_ticket_context() {
        let ticket = create_test_ticket();
        let task = create_test_task(TaskType::Custom);
        let projs = [("svc".to_string(), "/svc".to_string())];
        let prompt = generate_custom_task_prompt(&task, &ticket, Some(("WS", &projs)));
        let ws = prompt.find("## Workspace: WS").unwrap();
        let ctx = prompt.find("## Ticket Context").unwrap();
        assert!(ws < ctx);
    }

    #[test]
    fn generate_task_prompt_command_prepends_workspace() {
        let ticket = create_test_ticket();
        let mut task = create_test_task(TaskType::Command("sync-with-main".to_string()));
        task.content = None;
        let projs = [("x".to_string(), "/x".to_string())];
        let prompt = generate_task_prompt(
            &task,
            &ticket,
            None,
            Some(("WS", &projs)),
        );
        assert!(prompt.starts_with("## Workspace: WS"));
        assert!(prompt.contains("Sync with Main"));
    }

    #[test]
    fn generate_task_prompt_custom_includes_content_and_ticket_context() {
        let ticket = create_test_ticket();
        let task = create_test_task(TaskType::Custom);
        let prompt = generate_task_prompt(&task, &ticket, None, None);

        assert!(prompt.contains(&ticket.title));
        assert!(prompt.contains("Custom task content"));
        assert!(prompt.contains("Ticket Context"));
        assert!(prompt.contains(&ticket.description_md));
    }

    #[test]
    fn generate_task_prompt_command_type_returns_fallback() {
        let ticket = create_test_ticket();
        let mut task = create_test_task(TaskType::Command("sync-with-main".to_string()));
        task.content = None;
        let prompt = generate_task_prompt(&task, &ticket, None, None);

        assert!(prompt.contains("Sync with Main"));
        assert!(prompt.contains("git fetch"));
    }

    #[test]
    fn generate_custom_task_prompt_includes_priority_context() {
        let mut ticket = create_test_ticket();
        ticket.priority = Priority::Urgent;
        let task = create_test_task(TaskType::Custom);
        let prompt = generate_task_prompt(&task, &ticket, None, None);

        assert!(prompt.contains("URGENT"));
    }

    #[test]
    fn generate_custom_task_prompt_includes_labels() {
        let ticket = create_test_ticket();
        let task = create_test_task(TaskType::Custom);
        let prompt = generate_task_prompt(&task, &ticket, None, None);

        assert!(prompt.contains("bug"));
    }

    #[test]
    fn get_fallback_command_prompt_add_tests() {
        let prompt = get_fallback_command_prompt("add-tests");
        assert!(prompt.contains("Add Tests"));
        assert!(prompt.contains("test coverage"));
    }

    #[test]
    fn get_fallback_command_prompt_review_polish() {
        let prompt = get_fallback_command_prompt("review-polish");
        assert!(prompt.contains("Review and Polish"));
    }

    #[test]
    fn get_fallback_command_prompt_fix_lint() {
        let prompt = get_fallback_command_prompt("fix-lint");
        assert!(prompt.contains("Fix Lint"));
        assert!(prompt.contains("linter"));
    }

    #[test]
    fn get_fallback_command_prompt_unknown_returns_generic() {
        let prompt = get_fallback_command_prompt("unknown-task");
        assert!(prompt.contains("unknown-task"));
    }

    #[test]
    fn generate_task_plan_prompt_includes_context_and_requirements() {
        let ticket = create_test_ticket();
        let task = create_test_task(TaskType::Custom);
        let prompt = generate_task_plan_prompt(&task, &ticket, None);

        assert!(prompt.contains("Create an implementation plan"));
        assert!(prompt.contains(&ticket.title));
        assert!(prompt.contains("Ticket Context"));
        assert!(prompt.contains(&ticket.description_md));
        assert!(prompt.contains("Task Requirements"));
        assert!(prompt.contains("Custom task content"));
    }

    #[test]
    fn generate_task_plan_prompt_always_includes_ticket_description() {
        let ticket = create_test_ticket();
        let mut task = create_test_task(TaskType::Custom);
        task.content = None;
        let prompt = generate_task_plan_prompt(&task, &ticket, None);

        assert!(prompt.contains("Ticket Context"));
        assert!(prompt.contains(&ticket.description_md));
        assert!(!prompt.contains("Task Requirements"));
    }

    #[test]
    fn generate_task_implement_prompt_includes_context_and_plan() {
        let ticket = create_test_ticket();
        let task = create_test_task(TaskType::Custom);
        let plan = "Step 1: Do this\nStep 2: Do that";
        let prompt = generate_task_implement_prompt(&task, &ticket, plan, None);

        assert!(prompt.contains(&ticket.title));
        assert!(prompt.contains("Ticket Context"));
        assert!(prompt.contains(&ticket.description_md));
        assert!(prompt.contains("Task Requirements"));
        assert!(prompt.contains("Custom task content"));
        assert!(prompt.contains(plan));
        assert!(prompt.contains("Execute the implementation plan"));
    }

    #[test]
    fn generate_command_prompt_reads_custom_dir_first() {
        let tmp = std::env::temp_dir().join(format!("task_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("sync-with-main.md"), "# Custom sync instructions\nDo the custom sync.").unwrap();

        let ticket = create_test_ticket();
        let mut task = create_test_task(TaskType::Command("sync-with-main".to_string()));
        task.content = None;

        let prompt = generate_task_prompt(&task, &ticket, Some(&tmp), None);

        assert!(prompt.contains("Custom sync instructions"), "Should find custom command file");
        assert!(!prompt.contains("git fetch"), "Should NOT fall through to hardcoded fallback");

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn generate_command_prompt_falls_back_without_custom_file() {
        let ticket = create_test_ticket();
        let mut task = create_test_task(TaskType::Command("fix-lint".to_string()));
        task.content = None;
        let prompt = generate_task_prompt(&task, &ticket, None, None);

        assert!(prompt.contains("Fix Lint") || prompt.contains("fix-lint"));
    }

    #[test]
    fn generate_command_prompt_unknown_command_uses_generic_fallback() {
        let ticket = create_test_ticket();
        let mut task = create_test_task(TaskType::Command("my-custom-cmd".to_string()));
        task.content = None;
        let prompt = generate_task_prompt(&task, &ticket, None, None);

        assert!(prompt.contains("my-custom-cmd"));
    }

    #[test]
    fn custom_dir_missing_command_falls_through_to_bundled() {
        let tmp = std::env::temp_dir().join(format!("task_fallthrough_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("other-cmd.md"), "# other").unwrap();

        let ticket = create_test_ticket();
        let mut task = create_test_task(TaskType::Command("fix-lint".to_string()));
        task.content = None;

        let prompt = generate_task_prompt(&task, &ticket, Some(&tmp), None);

        assert!(
            prompt.contains("Fix Lint") || prompt.contains("fix-lint"),
            "Should fall through to bundled/fallback when custom dir lacks the command"
        );

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn none_custom_dir_reads_bundled_command_file() {
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        if manifest_dir.join("scripts/commands/code-review.md").exists() {
            let ticket = create_test_ticket();
            let mut task = create_test_task(TaskType::Command("code-review".to_string()));
            task.content = None;

            let prompt = generate_task_prompt(&task, &ticket, None, None);

            assert!(
                prompt.contains("Command Task: code-review"),
                "Should read bundled file content"
            );
            assert!(
                prompt.contains("Execute these instructions carefully"),
                "Should include the instruction footer"
            );
        }
    }

    #[test]
    fn custom_task_ignores_custom_commands_dir() {
        let tmp = std::env::temp_dir().join(format!("task_custom_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap();

        let ticket = create_test_ticket();
        let task = create_test_task(TaskType::Custom);
        let prompt = generate_task_prompt(&task, &ticket, Some(&tmp), None);

        assert!(prompt.contains("Custom task content"));
        assert!(prompt.contains(&ticket.title));

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn custom_task_empty_description_omits_ticket_context() {
        let mut ticket = create_test_ticket();
        ticket.description_md = String::new();
        let task = create_test_task(TaskType::Custom);
        let prompt = generate_custom_task_prompt(&task, &ticket, None);

        assert!(!prompt.contains("Ticket Context"));
        assert!(prompt.contains("Task Instructions"));
        assert!(prompt.contains("Custom task content"));
    }

    #[test]
    fn custom_task_no_content_only_shows_ticket_context() {
        let ticket = create_test_ticket();
        let mut task = create_test_task(TaskType::Custom);
        task.content = None;
        let prompt = generate_custom_task_prompt(&task, &ticket, None);

        assert!(prompt.contains("Ticket Context"));
        assert!(prompt.contains(&ticket.description_md));
        assert!(!prompt.contains("Task Instructions"));
    }

    #[test]
    fn custom_task_ticket_context_appears_before_task_instructions() {
        let ticket = create_test_ticket();
        let task = create_test_task(TaskType::Custom);
        let prompt = generate_custom_task_prompt(&task, &ticket, None);

        let ctx_pos = prompt.find("## Ticket Context").unwrap();
        let instr_pos = prompt.find("## Task Instructions").unwrap();
        assert!(ctx_pos < instr_pos, "Ticket Context must appear before Task Instructions");
    }

    #[test]
    fn plan_prompt_empty_description_omits_ticket_context() {
        let mut ticket = create_test_ticket();
        ticket.description_md = String::new();
        let task = create_test_task(TaskType::Custom);
        let prompt = generate_task_plan_prompt(&task, &ticket, None);

        assert!(!prompt.contains("Ticket Context"));
        assert!(prompt.contains("Task Requirements"));
        assert!(prompt.contains("Custom task content"));
    }

    #[test]
    fn plan_prompt_ticket_context_appears_before_task_requirements() {
        let ticket = create_test_ticket();
        let task = create_test_task(TaskType::Custom);
        let prompt = generate_task_plan_prompt(&task, &ticket, None);

        let ctx_pos = prompt.find("## Ticket Context").unwrap();
        let req_pos = prompt.find("## Task Requirements").unwrap();
        assert!(ctx_pos < req_pos, "Ticket Context must appear before Task Requirements");
    }

    #[test]
    fn implement_prompt_no_task_content_only_shows_ticket_context() {
        let ticket = create_test_ticket();
        let mut task = create_test_task(TaskType::Custom);
        task.content = None;
        let plan = "Step 1: Do this";
        let prompt = generate_task_implement_prompt(&task, &ticket, plan, None);

        assert!(prompt.contains("Ticket Context"));
        assert!(prompt.contains(&ticket.description_md));
        assert!(!prompt.contains("Task Requirements"));
        assert!(prompt.contains("Implementation Plan"));
        assert!(prompt.contains(plan));
    }

    #[test]
    fn implement_prompt_empty_description_omits_ticket_context() {
        let mut ticket = create_test_ticket();
        ticket.description_md = String::new();
        let task = create_test_task(TaskType::Custom);
        let plan = "Step 1: Do this";
        let prompt = generate_task_implement_prompt(&task, &ticket, plan, None);

        assert!(!prompt.contains("Ticket Context"));
        assert!(prompt.contains("Task Requirements"));
        assert!(prompt.contains("Custom task content"));
    }

    #[test]
    fn implement_prompt_ticket_context_appears_before_task_requirements() {
        let ticket = create_test_ticket();
        let task = create_test_task(TaskType::Custom);
        let plan = "Step 1: Do this";
        let prompt = generate_task_implement_prompt(&task, &ticket, plan, None);

        let ctx_pos = prompt.find("## Ticket Context").unwrap();
        let req_pos = prompt.find("## Task Requirements").unwrap();
        assert!(ctx_pos < req_pos, "Ticket Context must appear before Task Requirements");
    }

    #[test]
    fn generate_task_plan_prompt_with_workspace_inserts_after_title() {
        let ticket = create_test_ticket();
        let task = create_test_task(TaskType::Custom);
        let projs = [("api".to_string(), "/api".to_string())];
        let prompt = generate_task_plan_prompt(&task, &ticket, Some(("WS", &projs)));

        let ws_pos = prompt.find("## Workspace: WS").unwrap();
        let title_pos = prompt.find("# Task:").unwrap();
        let ctx_pos = prompt.find("## Ticket Context").unwrap();
        assert!(title_pos < ws_pos);
        assert!(ws_pos < ctx_pos);
        assert!(prompt.contains("multi-project workspace"));
        assert!(prompt.contains("**api** (/api)"));
    }

    #[test]
    fn generate_task_implement_prompt_with_workspace_inserts_after_title() {
        let ticket = create_test_ticket();
        let task = create_test_task(TaskType::Custom);
        let plan = "Step 1: Foo";
        let projs = [("web".to_string(), "/web".to_string())];
        let prompt = generate_task_implement_prompt(&task, &ticket, plan, Some(("WS", &projs)));

        let title_pos = prompt.find("# Task:").unwrap();
        let ws_pos = prompt.find("## Workspace: WS").unwrap();
        let ctx_pos = prompt.find("## Ticket Context").unwrap();
        assert!(title_pos < ws_pos);
        assert!(ws_pos < ctx_pos);
        assert!(prompt.contains("**web** (/web)"));
        assert!(prompt.contains(plan));
    }
}
