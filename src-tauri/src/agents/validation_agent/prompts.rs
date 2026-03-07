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
4. When you need to stop a running app, output a `stop_app` block. Do NOT try to kill the process yourself via `run_command`:
   ```json
   {{ "stop_app": {{}} }}
   ```
5. Wait for confirmation that the app is running before giving testing steps. The system will tell you the exact path to the app log file.
6. Once the app is running, you can read the log file (path provided in the confirmation message) to check for errors, warnings, or stack traces.
7. Once the app is running, provide clear testing instructions and report what works, what's broken, and what looks suspicious.
8. When the user reports a bug or issue, you MUST immediately output a `create_fix_task` JSON block. Do NOT ask for confirmation. Do NOT attempt to fix the issue yourself. Output exactly ONE task per response, written as a spec with requirements. The description should use markdown with sections for Problem, Requirements, and Acceptance Criteria:
   ```json
   {{ "create_fix_task": {{ "title": "Fix the broken login form", "description": "Problem: ... Requirements: ... Acceptance Criteria: ..." }} }}
   ```
   The system will automatically create this task on the ticket and a worker agent will fix it. The system will wait for the fix to complete and notify the user.

Begin by reviewing the diff. If the app needs setup commands first (e.g. install dependencies), output a `run_command` block. Then output `start_app` to start the application."#,
        ticket_title,
        ticket_description,
        criteria_section,
        truncate_diff(branch_diff, 120_000)
    )
}

/// Build a lightweight prompt for session resumption (the session already has
/// full context from the initial turn). Only includes new messages the session
/// hasn't seen yet.
pub fn build_resumption_prompt(new_messages: &[ValidationMessage]) -> String {
    let mut prompt = String::new();
    for msg in new_messages {
        let role = match msg.role {
            ValidationMessageRole::User => "User",
            ValidationMessageRole::Assistant => "Assistant",
            ValidationMessageRole::System => "System",
        };
        prompt.push_str(&format!("{}: {}\n\n", role, msg.content));
    }
    prompt.push_str("Respond to the latest message above.");
    prompt
}

const MAX_CONVERSATION_MESSAGES: usize = 20;

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

    let recent_messages = if messages.len() > MAX_CONVERSATION_MESSAGES {
        &messages[messages.len() - MAX_CONVERSATION_MESSAGES..]
    } else {
        messages
    };

    let mut history = String::new();
    if messages.len() > MAX_CONVERSATION_MESSAGES {
        history.push_str(&format!(
            "\n[{} earlier messages omitted]\n",
            messages.len() - MAX_CONVERSATION_MESSAGES
        ));
    }
    for msg in recent_messages {
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

If you need to stop the running app, output:
```json
{{ "stop_app": {{}} }}
```
Do NOT try to kill the process yourself via `run_command`.

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::{ValidationMessage, ValidationMessageRole};
    use chrono::Utc;

    #[test]
    fn initial_prompt_contains_ticket_fields() {
        let prompt = build_initial_prompt("My Title", "My Description", "diff here", None);
        assert!(prompt.contains("My Title"));
        assert!(prompt.contains("My Description"));
        assert!(prompt.contains("diff here"));
    }

    #[test]
    fn initial_prompt_includes_acceptance_criteria() {
        let prompt = build_initial_prompt("T", "D", "diff", Some("Must pass all tests"));
        assert!(prompt.contains("## Acceptance Criteria"));
        assert!(prompt.contains("Must pass all tests"));
    }

    #[test]
    fn initial_prompt_omits_criteria_when_none() {
        let prompt = build_initial_prompt("T", "D", "diff", None);
        assert!(!prompt.contains("## Acceptance Criteria"));
    }

    #[test]
    fn initial_prompt_omits_criteria_when_empty() {
        let prompt = build_initial_prompt("T", "D", "diff", Some(""));
        assert!(!prompt.contains("## Acceptance Criteria"));
    }

    #[test]
    fn conversation_prompt_includes_history() {
        let messages = vec![
            ValidationMessage {
                id: "1".into(),
                session_id: "s".into(),
                role: ValidationMessageRole::User,
                content: "Start the app".into(),
                metadata: None,
                created_at: Utc::now(),
            },
            ValidationMessage {
                id: "2".into(),
                session_id: "s".into(),
                role: ValidationMessageRole::Assistant,
                content: "Starting now".into(),
                metadata: None,
                created_at: Utc::now(),
            },
        ];
        let prompt =
            build_conversation_prompt("Title", "Desc", "diff", None, &messages);
        assert!(prompt.contains("User: Start the app"));
        assert!(prompt.contains("Assistant: Starting now"));
    }

    #[test]
    fn conversation_prompt_maps_system_role() {
        let messages = vec![ValidationMessage {
            id: "1".into(),
            session_id: "s".into(),
            role: ValidationMessageRole::System,
            content: "App started".into(),
            metadata: None,
            created_at: Utc::now(),
        }];
        let prompt =
            build_conversation_prompt("T", "D", "diff", None, &messages);
        assert!(prompt.contains("System: App started"));
    }

    // --- build_resumption_prompt ---

    #[test]
    fn resumption_prompt_formats_new_messages() {
        let messages = vec![
            ValidationMessage {
                id: "1".into(),
                session_id: "s".into(),
                role: ValidationMessageRole::System,
                content: "Ran `npm install` (exit 0, success)".into(),
                metadata: None,
                created_at: Utc::now(),
            },
            ValidationMessage {
                id: "2".into(),
                session_id: "s".into(),
                role: ValidationMessageRole::User,
                content: "Command finished".into(),
                metadata: None,
                created_at: Utc::now(),
            },
        ];
        let prompt = build_resumption_prompt(&messages);
        assert!(prompt.contains("System: Ran `npm install`"));
        assert!(prompt.contains("User: Command finished"));
        assert!(prompt.contains("Respond to the latest message above."));
        assert!(!prompt.contains("Branch diff"));
        assert!(!prompt.contains("Ticket"));
    }

    #[test]
    fn resumption_prompt_empty_messages() {
        let prompt = build_resumption_prompt(&[]);
        assert!(prompt.contains("Respond to the latest message above."));
    }

    // --- conversation_prompt truncation ---

    #[test]
    fn conversation_prompt_truncates_old_messages() {
        let mut messages = Vec::new();
        for i in 0..30 {
            messages.push(ValidationMessage {
                id: format!("{}", i),
                session_id: "s".into(),
                role: if i % 2 == 0 {
                    ValidationMessageRole::User
                } else {
                    ValidationMessageRole::Assistant
                },
                content: format!("message-{}", i),
                metadata: None,
                created_at: Utc::now(),
            });
        }
        let prompt = build_conversation_prompt("T", "D", "diff", None, &messages);
        assert!(prompt.contains("[10 earlier messages omitted]"));
        assert!(!prompt.contains("message-0"));
        assert!(!prompt.contains("message-9"));
        assert!(prompt.contains("message-10"));
        assert!(prompt.contains("message-29"));
    }

    #[test]
    fn conversation_prompt_no_truncation_under_limit() {
        let messages = vec![ValidationMessage {
            id: "1".into(),
            session_id: "s".into(),
            role: ValidationMessageRole::User,
            content: "hello".into(),
            metadata: None,
            created_at: Utc::now(),
        }];
        let prompt = build_conversation_prompt("T", "D", "diff", None, &messages);
        assert!(!prompt.contains("earlier messages omitted"));
        assert!(prompt.contains("User: hello"));
    }

    // --- truncate_diff ---

    #[test]
    fn truncate_diff_within_limit() {
        assert_eq!(truncate_diff("hello", 10), "hello");
    }

    #[test]
    fn truncate_diff_at_limit() {
        assert_eq!(truncate_diff("hello", 5), "hello");
    }

    #[test]
    fn truncate_diff_over_limit() {
        assert_eq!(truncate_diff("hello world", 5), "hello");
    }

    #[test]
    fn truncate_diff_respects_char_boundary() {
        // 'é' is 2 bytes; cutting at 1 must back up to 0
        assert_eq!(truncate_diff("é", 1), "");
        // "aé" is 3 bytes; cutting at 2 must yield "a"
        assert_eq!(truncate_diff("aé", 2), "a");
    }

    #[test]
    fn truncate_diff_empty() {
        assert_eq!(truncate_diff("", 100), "");
    }
}
