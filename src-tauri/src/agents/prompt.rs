use crate::db::models::{Priority, Ticket, Task, TaskType};
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
    generate_ticket_prompt_full(ticket, agent_kind, true)
}

/// Generate a ticket prompt with full control over workflow options
pub fn generate_ticket_prompt_full(ticket: &Ticket, agent_kind: Option<AgentKind>, requires_git: bool) -> String {
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
        prompt.push_str("## Workflow\n\n");
        
        let mut step = 1;
        
        // Only include git branch step if git is required
        if requires_git {
            // Use char-based iteration to safely handle multi-byte UTF-8 characters
            let id_prefix: String = ticket.id.chars().take(8).collect();
            let branch_name = format!("ticket/{}/{}", id_prefix, slugify(&ticket.title));
            prompt.push_str(&format!("{}. Create a branch: `{}`\n", step, branch_name));
            step += 1;
        }
        
        prompt.push_str(&format!("{}. Create a plan before implementing\n", step));
        step += 1;
        prompt.push_str(&format!("{}. After implementation, run this QA sequence:\n\n", step));
        
        match kind {
            AgentKind::Cursor => {
                prompt.push_str("   - `/deslop` - Remove AI-generated code patterns\n");
                prompt.push_str("   - `/cleanup` - Fix lint/type errors\n");
                prompt.push_str("   - `/unit-tests` - Add test coverage for your changes\n");
                prompt.push_str("   - `/cleanup` - Fix any test-related issues\n");
                prompt.push_str("   - `/review-changes` - Apply best practices\n");
                prompt.push_str("   - `/cleanup` - Final lint pass\n");
                prompt.push_str("   - `/review-changes` - Second review pass\n");
                if requires_git {
                    prompt.push_str("   - `/add-and-commit` - Stage and commit with detailed message\n");
                }
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
                if requires_git {
                    prompt.push_str("   - `.claude/commands/add-and-commit.md` - Stage and commit\n");
                }
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

// ===== Multi-Stage Workflow Prompt Generators =====

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
"#.to_string()
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
    
    prompt.push_str(r#"## Instructions

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
"#);
    
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
    
    prompt.push_str(r#"## Instructions

Execute the implementation plan above. For each step:
1. Make the necessary code changes
2. Verify the changes compile/pass type checking
3. Move to the next step

Focus on implementing the plan. Do NOT:
- Run the full QA sequence (that comes in the next stages)
- Commit changes (that comes later)
- Add tests (that comes in a separate stage)

Just implement the core functionality as described in the plan.
"#);
    
    prompt
}

/// Generate a prompt for a QA command stage (deslop, cleanup, unit-tests, etc.)
pub fn generate_command_prompt(command: &str, repo_path: &std::path::Path) -> String {
    // Try to read the command file content
    let cursor_cmd_path = repo_path.join(".cursor/rules").join(format!("{}.md", command));
    let claude_cmd_path = repo_path.join(".claude/commands").join(format!("{}.md", command));
    
    let cmd_content = std::fs::read_to_string(&cursor_cmd_path)
        .or_else(|_| std::fs::read_to_string(&claude_cmd_path))
        .ok();
    
    if let Some(content) = cmd_content {
        format!(
            r#"Execute the following command: /{command}

## Command Instructions

{content}

Execute these instructions carefully. When complete, report what was done.
"#
        )
    } else {
        // Fallback prompts if command file not found
        match command {
            "deslop" => r#"Execute the /deslop command:

Remove AI-generated code patterns:
- Unnecessary comments explaining obvious code
- Overly verbose or redundant code
- Placeholder TODOs that should be resolved
- Defensive code that's not actually needed

Focus on making the code clean and production-ready.
"#.to_string(),
            
            "cleanup" => r#"Execute the /cleanup command:

Fix all linting and type errors:
1. Run the linter and fix any issues
2. Run type checking and fix any errors
3. Ensure all imports are correct
4. Fix any formatting issues

Report any issues that couldn't be automatically fixed.
"#.to_string(),
            
            "unit-tests" => r#"Execute the /unit-tests command:

Add test coverage for the recent changes:
1. Identify the new or modified code
2. Create unit tests covering the main functionality
3. Test edge cases and error conditions
4. Ensure tests pass

Focus on meaningful tests that verify behavior, not just coverage.
"#.to_string(),
            
            "review-changes" => r#"Execute the /review-changes command:

Review all recent changes:
1. Check for code quality issues
2. Verify the implementation matches requirements
3. Look for potential bugs or edge cases
4. Ensure consistent style and patterns

Make any necessary improvements.
"#.to_string(),
            
            "add-and-commit" => r#"Execute the /add-and-commit command:

Stage and commit all changes:
1. Review what will be committed
2. Stage all relevant files
3. Create a detailed commit message describing:
   - What was changed
   - Why it was changed
   - Any notable implementation decisions

Use conventional commit format if the project uses it.
"#.to_string(),
            
            _ => format!(
                r#"Execute the /{command} command:

Follow the project's conventions for this command.
If a command file exists at .cursor/rules/{command}.md or .claude/commands/{command}.md, follow those instructions.
"#
            ),
        }
    }
}

// ===== Task-Based Prompt Generators =====

/// Generate a prompt for executing a task
/// This is the main entry point for task-based prompt generation
pub fn generate_task_prompt(task: &Task, ticket: &Ticket, repo_path: &std::path::Path) -> String {
    match task.task_type {
        TaskType::Custom => generate_custom_task_prompt(task, ticket),
        TaskType::SyncWithMain => generate_preset_task_prompt("sync-with-main", repo_path),
        TaskType::AddTests => generate_preset_task_prompt("add-tests", repo_path),
        TaskType::ReviewPolish => generate_preset_task_prompt("review-polish", repo_path),
        TaskType::FixLint => generate_preset_task_prompt("fix-lint", repo_path),
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
    if task.content.as_deref() != Some(&ticket.description_md) && !ticket.description_md.is_empty() {
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
    
    prompt.push_str(r#"## Instructions

1. Analyze the task requirements
2. Create a plan before implementing
3. Implement the required changes
4. Verify the changes compile/pass type checking
5. Run the project's test suite if applicable

Focus on completing this specific task. Additional QA stages will follow.
"#);
    
    prompt
}

/// Generate a prompt for a preset task type by reading the command file
fn generate_preset_task_prompt(preset_name: &str, repo_path: &std::path::Path) -> String {
    // Try to read from various locations
    let locations = [
        repo_path.join(".cursor/rules").join(format!("{}.md", preset_name)),
        repo_path.join(".claude/commands").join(format!("{}.md", preset_name)),
        // Fallback to our bundled command files
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("scripts/commands")
            .join(format!("{}.md", preset_name)),
    ];
    
    for path in &locations {
        if let Ok(content) = std::fs::read_to_string(path) {
            return format!(
                "# Preset Task: {}\n\n{}\n\nExecute these instructions carefully. When complete, report what was done.\n",
                preset_name,
                content
            );
        }
    }
    
    // Fallback prompts if no command file found
    get_fallback_preset_prompt(preset_name)
}

/// Get a fallback prompt for a preset if the command file is not found
fn get_fallback_preset_prompt(preset_name: &str) -> String {
    match preset_name {
        "sync-with-main" => r#"# Sync with Main

Merge the latest changes from the main branch into this feature branch.

## Instructions

1. Fetch latest from origin: `git fetch origin main`
2. Merge main into current branch: `git merge origin/main`
3. Resolve any conflicts carefully
4. Run linter and type checker after resolving
5. Commit the merge
6. Push the changes
"#.to_string(),
        
        "add-tests" => r#"# Add Tests

Add comprehensive test coverage for the recent changes.

## Instructions

1. Identify what changed: `git diff main...HEAD`
2. Add unit tests for new functions
3. Test happy paths, edge cases, and error conditions
4. Ensure all tests pass
5. Follow existing test patterns in the codebase
"#.to_string(),
        
        "review-polish" => r#"# Review and Polish

Review all recent changes for code quality and best practices.

## Instructions

1. Review the diff from main
2. Check code readability and naming
3. Check error handling and logging
4. Check for security concerns
5. Remove unused code and fix formatting
6. Add documentation where helpful
"#.to_string(),
        
        "fix-lint" => r#"# Fix Lint Errors

Fix all linting and type checking errors.

## Instructions

1. Run the linter (eslint, clippy, etc.)
2. Run the type checker
3. Fix all errors
4. Verify fixes by re-running checks
5. Run tests to ensure fixes didn't break anything
"#.to_string(),
        
        _ => format!(
            r#"# Task: {}

Execute the {} task. Follow any project conventions for this task type.
"#,
            preset_name, preset_name
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
    
    prompt.push_str(r#"## Instructions

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
"#);
    
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
    
    prompt.push_str(r#"## Instructions

Execute the implementation plan above. For each step:
1. Make the necessary code changes
2. Verify the changes compile/pass type checking
3. Move to the next step

Focus on implementing the plan. Do NOT:
- Run the full QA sequence (that comes in the next stages)
- Commit changes (that comes later)
- Add tests (that's a separate task)

Just implement the core functionality as described in the plan.
"#);
    
    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_ticket() -> Ticket {
        use crate::db::models::WorkflowType;
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
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
            order_in_epic: None,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            scratchpad_id: None,
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

    #[test]
    fn generate_ticket_prompt_with_workflow_handles_multibyte_utf8_id() {
        // Test that multi-byte UTF-8 characters in ticket ID don't cause panic
        let mut ticket = create_test_ticket();
        // 🎉 is 4 bytes, so this ID has multi-byte chars that could cause issues with byte slicing
        ticket.id = "🎉🚀ab12".to_string();
        ticket.title = "Test Feature".to_string();
        let prompt = generate_ticket_prompt_with_workflow(&ticket, Some(AgentKind::Cursor));
        // Should contain the full 4 chars (2 emoji + "ab") since we take up to 8 chars
        assert!(prompt.contains("ticket/🎉🚀ab12/test-feature"));
    }

    #[test]
    fn generate_ticket_prompt_with_workflow_handles_short_id() {
        let mut ticket = create_test_ticket();
        ticket.id = "abc".to_string();
        ticket.title = "Short ID Test".to_string();
        let prompt = generate_ticket_prompt_with_workflow(&ticket, Some(AgentKind::Cursor));
        assert!(prompt.contains("ticket/abc/short-id-test"));
    }

    #[test]
    fn generate_ticket_prompt_with_workflow_handles_mixed_utf8_id() {
        let mut ticket = create_test_ticket();
        // Mix of ASCII and multi-byte chars to test boundary handling
        ticket.id = "a🎉bcdefgh".to_string();
        ticket.title = "Mixed Test".to_string();
        let prompt = generate_ticket_prompt_with_workflow(&ticket, Some(AgentKind::Cursor));
        // Takes first 8 chars: a, 🎉, b, c, d, e, f, g
        assert!(prompt.contains("ticket/a🎉bcdefg/mixed-test"));
    }
    
    #[test]
    fn generate_ticket_prompt_full_without_git_cursor() {
        let ticket = create_test_ticket();
        let prompt = generate_ticket_prompt_full(&ticket, Some(AgentKind::Cursor), false);
        
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
        let prompt = generate_ticket_prompt_full(&ticket, Some(AgentKind::Claude), false);
        
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
        let prompt = generate_ticket_prompt_full(&ticket, Some(AgentKind::Cursor), true);
        
        // Should have all workflow steps including git
        assert!(prompt.contains("Create a branch:"));
        assert!(prompt.contains("/add-and-commit"));
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
        assert_eq!(result, Some("feat/2f8c058c/add-frontend-themes".to_string()));
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
