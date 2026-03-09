//! Prompt building for validation agent (ticket + diff + chat).

use crate::db::models::{ValidationMessage, ValidationMessageRole};

/// Build the initial review prompt (first message in session).
pub fn build_initial_prompt(
    ticket_title: &str,
    ticket_description: &str,
    branch_diff: &str,
    acceptance_criteria: Option<&str>,
    user_message: &str,
) -> String {
    let criteria_section = acceptance_criteria
        .filter(|s| !s.is_empty())
        .map(|s| format!("\n## Acceptance Criteria\n{}\n", s))
        .unwrap_or_default();

    format!(
        r#"# Review Session

You are a review assistant for a ticket that has implementation changes on a branch. Your role is to help the user review, refine, and improve the work that has been done. You can review code, identify issues, run commands, start/stop the application, and create new tasks for a worker agent to fix.

CRITICAL RULES:
- You MUST NOT attempt to fix code, write code, or edit files yourself.
- You MUST NOT use tools to modify the codebase in any way.
- Your role is to review, analyze, and create tasks. A separate worker agent handles all code changes.

## Ticket
**Title:** {}
**Description:**
{}

{}
## Branch diff (vs main)
```
{}
```

## Available tools

You have the following tools available. Use them as needed based on what the user asks for — you do NOT need to use all of them.

### Run a shell command
Execute a command in the project directory to explore files, run tests, check logs, install dependencies, etc.
```json
{{ "run_command": {{ "command": "ls -la" }} }}
```

### Start the application
Launch the application as a background process. Use "port" only if you know the specific port (optional). The system will manage the process and stream logs. Do NOT start the app via `run_command`.
```json
{{ "start_app": {{ "command": "npm run dev", "port": 3000 }} }}
```

### Stop the application
Stop a previously started application. Do NOT try to kill processes via `run_command`.
```json
{{ "stop_app": {{}} }}
```

### Create fix tasks
When you identify issues, improvements, or bugs, create tasks for a worker agent to fix. Write each task as a spec with a clear problem statement, requirements, and acceptance criteria. The description should use markdown with sections for Problem, Requirements, and Acceptance Criteria. You may create one task at a time:
```json
{{ "create_fix_task": {{ "title": "Fix the issue", "description": "Problem: ... Requirements: ... Acceptance Criteria: ..." }} }}
```
Or multiple tasks at once:
```json
{{ "create_fix_tasks": {{ "tasks": [{{ "title": "First task", "description": "..." }}, {{ "title": "Second task", "description": "..." }}] }} }}
```
The system will automatically create these tasks on the ticket and a worker agent will pick them up. Do NOT ask for confirmation before creating tasks — if something needs fixing, create the task immediately.

## User's request
{}

Respond to the user's request above. Use the ticket context and diff to inform your response. Only use the tools that are relevant to what the user is asking for."#,
        ticket_title,
        ticket_description,
        criteria_section,
        truncate_diff(branch_diff, 120_000),
        user_message
    )
}

/// Build a lightweight prompt for session resumption. The session already has
/// full context from the initial turn.
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
        r#"# Review Session (continued)

You are a review assistant for a ticket with implementation changes. You review, analyze, and create tasks. You MUST NOT fix code, write code, or edit files yourself. A separate worker agent handles all code changes.

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
## Available tools

Use any of these as needed based on the conversation:

- **Run a command:** `{{ "run_command": {{ "command": "..." }} }}`
- **Start the app:** `{{ "start_app": {{ "command": "...", "port": 3000 }} }}` (port is optional; do NOT start via run_command)
- **Stop the app:** `{{ "stop_app": {{}} }}` (do NOT kill via run_command)
- **Create a fix task:** `{{ "create_fix_task": {{ "title": "...", "description": "..." }} }}`
- **Create multiple fix tasks:** `{{ "create_fix_tasks": {{ "tasks": [{{ "title": "...", "description": "..." }}, ...] }} }}`

When creating fix tasks, write them as specs with Problem, Requirements, and Acceptance Criteria sections. Do NOT ask for confirmation — create tasks immediately when issues are identified.

Respond to the user's latest message."#,
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
        let prompt = build_initial_prompt("My Title", "My Description", "diff here", None, "Review the diff");
        assert!(prompt.contains("My Title"));
        assert!(prompt.contains("My Description"));
        assert!(prompt.contains("diff here"));
    }

    #[test]
    fn initial_prompt_includes_user_message() {
        let prompt = build_initial_prompt("T", "D", "diff", None, "Start the app and test");
        assert!(prompt.contains("Start the app and test"));
        assert!(prompt.contains("User's request"));
    }

    #[test]
    fn initial_prompt_includes_acceptance_criteria() {
        let prompt = build_initial_prompt("T", "D", "diff", Some("Must pass all tests"), "review");
        assert!(prompt.contains("## Acceptance Criteria"));
        assert!(prompt.contains("Must pass all tests"));
    }

    #[test]
    fn initial_prompt_omits_criteria_when_none() {
        let prompt = build_initial_prompt("T", "D", "diff", None, "review");
        assert!(!prompt.contains("## Acceptance Criteria"));
    }

    #[test]
    fn initial_prompt_omits_criteria_when_empty() {
        let prompt = build_initial_prompt("T", "D", "diff", Some(""), "review");
        assert!(!prompt.contains("## Acceptance Criteria"));
    }

    #[test]
    fn initial_prompt_presents_tools_as_available() {
        let prompt = build_initial_prompt("T", "D", "diff", None, "What changed?");
        assert!(prompt.contains("Available tools"));
        assert!(prompt.contains("run_command"));
        assert!(prompt.contains("start_app"));
        assert!(prompt.contains("create_fix_task"));
        assert!(prompt.contains("create_fix_tasks"));
    }

    #[test]
    fn initial_prompt_empty_user_message() {
        let prompt = build_initial_prompt("T", "D", "diff", None, "");
        assert!(prompt.contains("User's request"));
        assert!(prompt.contains("Available tools"));
    }

    #[test]
    fn initial_prompt_uses_review_session_header() {
        let prompt = build_initial_prompt("T", "D", "diff", None, "review");
        assert!(prompt.contains("# Review Session"));
        assert!(!prompt.contains("Validation Session"));
    }

    #[test]
    fn initial_prompt_is_not_prescriptive() {
        let prompt = build_initial_prompt("T", "D", "diff", None, "review");
        assert!(!prompt.contains("Begin by reviewing the diff. Then explore"));
        assert!(!prompt.contains("Your task\n1."));
    }

    #[test]
    fn conversation_prompt_uses_review_header() {
        let messages = vec![ValidationMessage {
            id: "1".into(),
            session_id: "s".into(),
            role: ValidationMessageRole::User,
            content: "hi".into(),
            metadata: None,
            created_at: Utc::now(),
        }];
        let prompt = build_conversation_prompt("T", "D", "diff", None, &messages);
        assert!(prompt.contains("# Review Session (continued)"));
        assert!(!prompt.contains("Validation Session"));
    }

    #[test]
    fn conversation_prompt_mentions_plural_fix_tasks() {
        let messages = vec![ValidationMessage {
            id: "1".into(),
            session_id: "s".into(),
            role: ValidationMessageRole::User,
            content: "hi".into(),
            metadata: None,
            created_at: Utc::now(),
        }];
        let prompt = build_conversation_prompt("T", "D", "diff", None, &messages);
        assert!(prompt.contains("create_fix_tasks"));
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
