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

You are validating implementation changes for a ticket. You have full shell access (this process can start apps, run curl, run tests, inspect logs).

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
2. If the app needs to be running to test: do NOT run it yourself. Instead output a fenced JSON block with the command for the system to start:
   ```json
   {{ "start_app": {{ "command": "npm run dev", "port": 5173 }} }}
   ```
   Use "command" for the exact shell command (e.g. "npm run dev", "cargo run", "yarn start"). Use "port" only if you know the app listens on a specific port (optional).
3. Wait for confirmation that the app is running before giving testing steps or running curl/tests against it.
4. Once the app is running, provide clear testing instructions and report what works, what's broken, and what looks suspicious.

Respond in clear markdown. You may output a JSON block with structure like:
```json
{{ "validation_complete": true/false, "observations": [...], "issues": [...] }}
```
but always include a human-readable summary as well.

Begin by reviewing the diff. If an app should be started, output the start_app JSON block first; the system will start it and then ask you for testing instructions."#,
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

You are validating implementation changes. You have full shell access.

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
Respond to the user's latest message. If you need the application to be started and have not yet output start_app, output a fenced JSON block:
```json
{{ "start_app": {{ "command": "npm run dev", "port": 5173 }} }}
```
Do not run the app yourself; the system will start it and then ask you for testing instructions. Use markdown and optional JSON for structure."#,
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
