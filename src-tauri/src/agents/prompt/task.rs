//! Task-based prompt generation.

use std::path::Path;

use crate::agents::provider::AgentProvider;
use crate::db::models::{Priority, Task, TaskType, Ticket};

/// Generate a prompt for executing a task
/// This is the main entry point for task-based prompt generation
pub fn generate_task_prompt(
    task: &Task,
    ticket: &Ticket,
    repo_path: &Path,
    providers: &[&dyn AgentProvider],
) -> String {
    match &task.task_type {
        TaskType::Custom => generate_custom_task_prompt(task, ticket),
        TaskType::Command(id) => generate_command_task_prompt(id, repo_path, providers),
    }
}

/// Generate a prompt for a custom task
fn generate_custom_task_prompt(task: &Task, ticket: &Ticket) -> String {
    let mut prompt = String::new();

    prompt.push_str(&format!("# Task: {}\n\n", ticket.title));

    // Include the task-specific content if available
    if let Some(ref content) = task.content {
        if !content.is_empty() {
            prompt.push_str("## Task Instructions\n\n");
            prompt.push_str(content);
            prompt.push_str("\n\n");
        }
    }

    // Include ticket context if different from task content
    if task.content.as_deref() != Some(&ticket.description_md) && !ticket.description_md.is_empty()
    {
        prompt.push_str("## Original Ticket Context\n\n");
        prompt.push_str(&ticket.description_md);
        prompt.push_str("\n\n");
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
///
/// Searches provider-specific repo-level directories first (e.g.
/// `<repo>/.cursor/rules/<command>.md`), then bundled command files,
/// then falls back to hardcoded prompts.
fn generate_command_task_prompt(
    command_id: &str,
    repo_path: &Path,
    providers: &[&dyn AgentProvider],
) -> String {
    let locations =
        super::workflow::build_command_search_paths(command_id, repo_path, providers);

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
pub fn generate_task_plan_prompt(task: &Task, ticket: &Ticket) -> String {
    let mut prompt = String::new();

    prompt.push_str("Create an implementation plan for this task.\n\n");
    prompt.push_str(&format!("# Task: {}\n\n", ticket.title));

    // Use task content if available, otherwise use ticket description
    let content = task.content.as_deref().unwrap_or(&ticket.description_md);
    if !content.is_empty() {
        prompt.push_str("## Requirements\n\n");
        prompt.push_str(content);
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
pub fn generate_task_implement_prompt(task: &Task, ticket: &Ticket, plan: &str) -> String {
    let mut prompt = String::new();

    prompt.push_str(&format!("# Task: {}\n\n", ticket.title));

    // Use task content if available
    let content = task.content.as_deref().unwrap_or(&ticket.description_md);
    if !content.is_empty() {
        prompt.push_str("## Requirements\n\n");
        prompt.push_str(content);
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
    fn generate_task_prompt_custom_includes_content() {
        let ticket = create_test_ticket();
        let task = create_test_task(TaskType::Custom);
        let prompt = generate_task_prompt(&task, &ticket, Path::new("/tmp"), &[]);

        assert!(prompt.contains(&ticket.title));
        assert!(prompt.contains("Custom task content"));
    }

    #[test]
    fn generate_task_prompt_command_type_returns_fallback() {
        let ticket = create_test_ticket();
        let mut task = create_test_task(TaskType::Command("sync-with-main".to_string()));
        task.content = None;
        let prompt = generate_task_prompt(&task, &ticket, Path::new("/nonexistent"), &[]);

        assert!(prompt.contains("Sync with Main"));
        assert!(prompt.contains("git fetch"));
    }

    #[test]
    fn generate_custom_task_prompt_includes_priority_context() {
        let mut ticket = create_test_ticket();
        ticket.priority = Priority::Urgent;
        let task = create_test_task(TaskType::Custom);
        let prompt = generate_task_prompt(&task, &ticket, Path::new("/tmp"), &[]);

        assert!(prompt.contains("URGENT"));
    }

    #[test]
    fn generate_custom_task_prompt_includes_labels() {
        let ticket = create_test_ticket();
        let task = create_test_task(TaskType::Custom);
        let prompt = generate_task_prompt(&task, &ticket, Path::new("/tmp"), &[]);

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
    fn generate_task_plan_prompt_includes_requirements() {
        let ticket = create_test_ticket();
        let task = create_test_task(TaskType::Custom);
        let prompt = generate_task_plan_prompt(&task, &ticket);

        assert!(prompt.contains("Create an implementation plan"));
        assert!(prompt.contains(&ticket.title));
        assert!(prompt.contains("Custom task content"));
    }

    #[test]
    fn generate_task_plan_prompt_uses_ticket_description_as_fallback() {
        let ticket = create_test_ticket();
        let mut task = create_test_task(TaskType::Custom);
        task.content = None;
        let prompt = generate_task_plan_prompt(&task, &ticket);

        assert!(prompt.contains(&ticket.description_md));
    }

    #[test]
    fn generate_task_implement_prompt_includes_plan() {
        let ticket = create_test_ticket();
        let task = create_test_task(TaskType::Custom);
        let plan = "Step 1: Do this\nStep 2: Do that";
        let prompt = generate_task_implement_prompt(&task, &ticket, plan);

        assert!(prompt.contains(&ticket.title));
        assert!(prompt.contains(plan));
        assert!(prompt.contains("Execute the implementation plan"));
    }

    #[test]
    fn generate_preset_prompt_finds_provider_specific_file() {
        use crate::agents::cost::RunCostData;
        use crate::agents::provider::{AgentProvider, AgentRunConfig};

        #[derive(Debug)]
        struct TestProvider;
        impl AgentProvider for TestProvider {
            fn id(&self) -> &str { "test" }
            fn display_name(&self) -> &str { "Test" }
            fn build_command(&self, _: &AgentRunConfig) -> (String, Vec<String>) { ("test".into(), vec![]) }
            fn build_env_vars(&self, _: &AgentRunConfig) -> Vec<(String, String)> { vec![] }
            fn extract_text(&self, o: &str) -> String { o.to_string() }
            fn extract_cost(&self, _: &str, _: &str, _: f64) -> Option<RunCostData> { None }
            fn is_available(&self) -> bool { false }
            fn get_version(&self) -> Option<String> { None }
            fn config_dir_name(&self) -> &str { ".test-agent" }
            fn command_instructions_subdir(&self) -> &str { "commands" }
            fn format_command_reference(&self, c: &str) -> String { format!("/{c}") }
        }

        // Create a temp dir with a provider-specific command file
        let tmp = std::env::temp_dir().join(format!("task_test_{}", uuid::Uuid::new_v4()));
        let cmd_dir = tmp.join(".test-agent").join("commands");
        std::fs::create_dir_all(&cmd_dir).unwrap();
        std::fs::write(cmd_dir.join("sync-with-main.md"), "# Custom sync instructions\nDo the custom sync.").unwrap();

        let ticket = create_test_ticket();
        let mut task = create_test_task(TaskType::Command("sync-with-main".to_string()));
        task.content = None;
        let provider = TestProvider;
        let providers: &[&dyn AgentProvider] = &[&provider];

        let prompt = generate_task_prompt(&task, &ticket, &tmp, providers);

        // Should find the provider-specific file, not the fallback
        assert!(prompt.contains("Custom sync instructions"), "Should find provider-specific command file");
        assert!(!prompt.contains("git fetch"), "Should NOT fall through to hardcoded fallback");

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn generate_command_prompt_falls_back_without_provider_file() {
        let ticket = create_test_ticket();
        let mut task = create_test_task(TaskType::Command("fix-lint".to_string()));
        task.content = None;
        let prompt = generate_task_prompt(&task, &ticket, Path::new("/nonexistent"), &[]);

        assert!(prompt.contains("Fix Lint") || prompt.contains("fix-lint"));
    }

    #[test]
    fn generate_command_prompt_unknown_command_uses_generic_fallback() {
        let ticket = create_test_ticket();
        let mut task = create_test_task(TaskType::Command("my-custom-cmd".to_string()));
        task.content = None;
        let prompt = generate_task_prompt(&task, &ticket, Path::new("/nonexistent"), &[]);

        assert!(prompt.contains("my-custom-cmd"));
    }
}
