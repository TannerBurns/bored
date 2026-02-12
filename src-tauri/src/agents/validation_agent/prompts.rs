//! Prompt building for validation agent (ticket + diff + chat).

use crate::db::models::{ValidationMessage, ValidationMessageRole};

/// Build the initial validation prompt (first message in session)
pub fn build_initial_prompt(
    ticket_title: &str,
    ticket_description: &str,
    branch_diff: &str,
    acceptance_criteria: Option<&str>,
) -> String {
    let criteria_section = acceptance_criteria
        .filter(|s| !s.is_empty())
        .map(|s| format!("\n## Acceptance Criteria\n{}\n", s))
        .unwrap_or_default();

    format!(
        r#"# Validation Session

You are a validation agent. Your ONLY role is to help the user validate implementation changes for a ticket. You review code, start the app, provide testing instructions, and create fix tasks when issues are found.

CRITICAL RULES:
- You MUST NOT attempt to fix code, write code, edit files, or run commands to fix issues.
- You MUST NOT use tools to modify the codebase in any way.
- Your role is ONLY to validate and report. A separate worker agent will do the fixing.

## Ticket
**Title:** {}
**Description:**
{}

{}
## Branch diff (vs main)
```
{}
```

## Your task
1. Review the diff and ticket description.
2. If the app needs setup before starting (installing dependencies, building, running migrations, etc.), output a `run_command` block. The system will run it and show you the output. You can chain multiple commands this way:
   ```json
   {{ "run_command": {{ "command": "npm install" }} }}
   ```
3. When the app is ready to start, output a `start_app` block. Do NOT run the app yourself:
   ```json
   {{ "start_app": {{ "command": "npm run dev", "port": 5173 }} }}
   ```
   Use "port" only if you know the app listens on a specific port (optional). If the app fails to start, you will see the error output and can issue more `run_command` or `start_app` blocks to fix it.
4. Wait for confirmation that the app is running before giving testing steps.
5. Once the app is running, application logs (stdout and stderr) are written to `.validation-app.log` in the project directory. You can read this file to check for errors.
6. Once the app is running, provide clear testing instructions and report what works, what's broken, and what looks suspicious.
7. When the user reports a bug or issue, you MUST immediately output a `create_fix_task` JSON block. Do NOT ask for confirmation. Do NOT attempt to fix the issue yourself. Output exactly ONE task per response, written as a spec with requirements. The description should use markdown with sections for Problem, Requirements, and Acceptance Criteria:
   ```json
   {{ "create_fix_task": {{ "title": "Fix the broken login form", "description": "Problem: ... Requirements: ... Acceptance Criteria: ..." }} }}
   ```
   The system will automatically create this task on the ticket and a worker agent will fix it. After the fix is complete the system will restart the app for re-validation.

Begin by reviewing the diff. If the app needs setup commands first (e.g. install dependencies), output a `run_command` block. Then output `start_app` to start the application."#,
        ticket_title,
        ticket_description,
        criteria_section,
        truncate_diff(branch_diff, 120_000)
    )
}

/// Build a prompt for continuing the validation conversation
pub fn build_conversation_prompt(
    ticket_title: &str,
    ticket_description: &str,
    branch_diff: &str,
    acceptance_criteria: Option<&str>,
    messages: &[ValidationMessage],
) -> String {
    let criteria_section = acceptance_criteria
        .filter(|s| !s.is_empty())
        .map(|s| format!("\n## Acceptance Criteria\n{}\n", s))
        .unwrap_or_default();

    let mut history = String::new();
    for msg in messages {
        let role = match msg.role {
            ValidationMessageRole::User => "User",
            ValidationMessageRole::Assistant => "Assistant",
            ValidationMessageRole::System => "System",
        };
        history.push_str(&format!("\n{}: {}\n", role, msg.content));
    }

    format!(
        r#"# Validation Session (continued)

You are a validation agent. Your ONLY role is to validate and report. You MUST NOT attempt to fix code, write code, edit files, or run commands to fix issues. A separate worker agent handles fixes.

## Ticket
**Title:** {}
**Description:**
{}

{}
## Branch diff (vs main)
```
{}
```

## Conversation so far
{}
## Your task
Respond to the user's latest message.

If you need to run a setup command (install deps, build, migrate, etc.), output:
```json
{{ "run_command": {{ "command": "npm install" }} }}
```
The system will run it and show you the output so you can decide what to do next.

If you need the application to be started, output:
```json
{{ "start_app": {{ "command": "npm run dev", "port": 5173 }} }}
```
Do not run the app yourself. If the app fails to start, you will see the error output and can issue more `run_command` or `start_app` blocks.

When the user reports a bug or issue, you MUST immediately output exactly ONE `create_fix_task` JSON block written as a spec with requirements. Do NOT ask for confirmation. Do NOT attempt to fix the issue yourself.
```json
{{ "create_fix_task": {{ "title": "Fix the issue", "description": "Problem: ... Requirements: ... Acceptance Criteria: ..." }} }}
```
The system will create this task automatically and a worker agent will fix it."#,
        ticket_title,
        ticket_description,
        criteria_section,
        truncate_diff(branch_diff, 80_000),
        history
    )
}

fn truncate_diff(diff: &str, max_bytes: usize) -> &str {
    if diff.len() <= max_bytes {
        return diff;
    }
    let mut end = max_bytes;
    while end > 0 && !diff.is_char_boundary(end) {
        end -= 1;
    }
    &diff[..end]
}
