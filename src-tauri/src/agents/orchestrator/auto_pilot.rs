//! Auto-pilot command selection: prompt generation and response parsing.
//!
//! After plan + implement, the agent is asked which commands should be run
//! and with which models. It returns a JSON array of `{command, model}` pairs.

use super::WorkflowOrchestrator;
use crate::agents::models::MODEL_ENTRIES;

/// A single command+model pair selected by the agent.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CommandSelection {
    pub command: String,
    pub model: String,
}

/// Commands excluded from the auto-pilot selection list because they are
/// handled separately by the orchestrator.
const EXCLUDED_COMMANDS: &[&str] = &[
    "add-and-commit",
    "code-review-fix",
];

/// Available command descriptions presented to the agent.
const AVAILABLE_COMMANDS: &[(&str, &str)] = &[
    ("code-review", "Iterative review loop to find and fix issues"),
    ("cleanup", "Run linters, fix build warnings, and clean up code"),
    ("unit-tests", "Generate and run unit tests for the changes"),
    ("review-changes", "Senior code review for correctness and security"),
    ("deslop", "Remove AI-generated slop and improve code taste"),
    ("add-tests", "Add comprehensive tests for the changes"),
    ("fix-lint", "Fix linting errors and warnings"),
    ("sync-with-main", "Sync the working branch with the main branch"),
    ("review-polish", "Final review and polishing pass"),
    ("patch-security", "Security review and fix pass scoped to branch diff"),
    ("api-contract-check", "Verify and fix public contract consistency across call sites"),
    ("observability-pass", "Align logs, metrics, and tracing with repo standards"),
    ("integration-test", "Add minimal integration tests for boundary-spanning changes"),
    ("doc-sync", "Update or create documentation from branch changes"),
];

impl WorkflowOrchestrator {
    /// Run the command-selection stage: ask the agent which commands to run.
    pub(super) async fn run_command_selection_stage(
        &self,
        plan: &str,
        impl_summary: &str,
    ) -> Result<Vec<CommandSelection>, String> {
        let prompt = generate_command_selection_prompt(
            &self.ticket.title,
            &self.ticket.description_md,
            plan,
            impl_summary,
        );

        let result = self
            .run_stage_with_model(
                "command-selection",
                &prompt,
                &self.get_stage_model("implement"),
            )
            .await;

        match result {
            Ok(run_result) => {
                let raw = run_result.captured_stdout.unwrap_or_default();
                let text = self.extract_text(&raw);
                let selections = parse_command_selection_response(&text);
                Ok(selections)
            }
            Err(e) => {
                tracing::warn!(
                    "Command selection stage failed, falling back to empty selection: {}",
                    e
                );
                Ok(Vec::new())
            }
        }
    }
}

fn generate_command_selection_prompt(
    ticket_title: &str,
    ticket_description: &str,
    plan: &str,
    impl_summary: &str,
) -> String {
    let mut commands_list = String::new();
    for (id, desc) in AVAILABLE_COMMANDS {
        commands_list.push_str(&format!("- `{}`: {}\n", id, desc));
    }

    let mut models_list = String::new();
    for &(friendly, _, _) in MODEL_ENTRIES {
        models_list.push_str(&format!("- `{}`\n", friendly));
    }

    let plan_section = if plan.is_empty() {
        String::new()
    } else {
        format!("## Plan\n\n{}\n\n", truncate(plan, 4000))
    };

    let impl_section = if impl_summary.is_empty() {
        String::new()
    } else {
        format!(
            "## Implementation Summary\n\n{}\n\n",
            truncate(impl_summary, 4000)
        )
    };

    format!(
        r#"You are a workflow orchestrator deciding which quality assurance commands should run after an implementation is complete.

## Ticket

**Title:** {ticket_title}
**Description:** {ticket_description}

{plan_section}{impl_section}## Available Commands

{commands_list}
## Available Models

{models_list}
## Instructions

Based on the ticket, plan, and implementation above, decide which commands should be run and in what order to ensure the implementation is high quality. For each command, choose an appropriate model.

Guidelines:
- Use more capable models (opus) for complex review tasks, cheaper models (sonnet) for mechanical tasks like linting or formatting.
- You do NOT need to use all commands. Only select commands that are relevant to the changes.
- If the implementation is simple or you believe no additional QA is needed, return an empty array.
- Do NOT include `add-and-commit` — it always runs automatically at the end.
- Order matters: put review/fix commands before final checks.

Respond with ONLY a JSON array (no other text). Example:

```json
[
  {{"command": "cleanup", "model": "sonnet-4.6"}},
  {{"command": "code-review", "model": "opus-4.6"}},
  {{"command": "deslop", "model": "sonnet-4.5"}}
]
```

If no commands are needed, respond with:

```json
[]
```
"#
    )
}

/// Parse the agent's response to extract the command selection list.
pub fn parse_command_selection_response(response: &str) -> Vec<CommandSelection> {
    crate::agents::json_extraction::parse_json_response::<Vec<CommandSelection>>(response)
        .map(filter_valid_selections)
        .unwrap_or_else(|| {
            tracing::warn!("Could not parse command selection from agent response");
            Vec::new()
        })
}

fn filter_valid_selections(selections: Vec<CommandSelection>) -> Vec<CommandSelection> {
    let valid_commands: std::collections::HashSet<&str> =
        AVAILABLE_COMMANDS.iter().map(|(id, _)| *id).collect();

    selections
        .into_iter()
        .filter(|s| {
            if EXCLUDED_COMMANDS.contains(&s.command.as_str()) {
                tracing::warn!(
                    "Auto-pilot: filtering out excluded command '{}'",
                    s.command
                );
                return false;
            }
            if !valid_commands.contains(s.command.as_str()) {
                tracing::warn!(
                    "Auto-pilot: filtering out unknown command '{}'",
                    s.command
                );
                return false;
            }
            true
        })
        .collect()
}

fn truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_json_code_block() {
        let response = r#"Here are the commands:

```json
[
  {"command": "cleanup", "model": "sonnet-4.6"},
  {"command": "code-review", "model": "opus-4.6"}
]
```
"#;
        let result = parse_command_selection_response(response);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].command, "cleanup");
        assert_eq!(result[0].model, "sonnet-4.6");
        assert_eq!(result[1].command, "code-review");
        assert_eq!(result[1].model, "opus-4.6");
    }

    #[test]
    fn parse_raw_json_array() {
        let response = r#"[{"command": "deslop", "model": "sonnet-4.5"}]"#;
        let result = parse_command_selection_response(response);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].command, "deslop");
    }

    #[test]
    fn parse_empty_array() {
        let response = "```json\n[]\n```";
        let result = parse_command_selection_response(response);
        assert!(result.is_empty());
    }

    #[test]
    fn filters_excluded_commands() {
        let response =
            r#"[{"command": "add-and-commit", "model": "sonnet-4.6"}, {"command": "cleanup", "model": "sonnet-4.6"}]"#;
        let result = parse_command_selection_response(response);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].command, "cleanup");
    }

    #[test]
    fn filters_unknown_commands() {
        let response =
            r#"[{"command": "nonexistent", "model": "opus-4.6"}, {"command": "deslop", "model": "opus-4.5"}]"#;
        let result = parse_command_selection_response(response);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].command, "deslop");
    }

    #[test]
    fn parse_unparseable_returns_empty() {
        let result = parse_command_selection_response("I don't know what to do.");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_json_with_surrounding_text() {
        let response = r#"Based on the implementation, I recommend:

[{"command": "unit-tests", "model": "opus-4.5"}, {"command": "review-changes", "model": "opus-4.5"}]

These will ensure quality."#;
        let result = parse_command_selection_response(response);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].command, "unit-tests");
        assert_eq!(result[1].command, "review-changes");
    }

    #[test]
    fn filters_code_review_fix_command() {
        let response =
            r#"[{"command": "code-review-fix", "model": "opus-4.6"}, {"command": "cleanup", "model": "sonnet-4.6"}]"#;
        let result = parse_command_selection_response(response);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].command, "cleanup");
    }

    #[test]
    fn parse_all_valid_commands() {
        let response = r#"[
            {"command": "code-review", "model": "opus-4.6"},
            {"command": "cleanup", "model": "sonnet-4.6"},
            {"command": "unit-tests", "model": "opus-4.5"},
            {"command": "review-changes", "model": "opus-4.5"},
            {"command": "deslop", "model": "sonnet-4.5"},
            {"command": "add-tests", "model": "opus-4.5"},
            {"command": "fix-lint", "model": "sonnet-4.6"},
            {"command": "patch-security", "model": "opus-4.6"},
            {"command": "doc-sync", "model": "sonnet-4.5"}
        ]"#;
        let result = parse_command_selection_response(response);
        assert_eq!(result.len(), 9);
    }

    // -- truncate --

    #[test]
    fn truncate_short_string() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_at_limit() {
        assert_eq!(truncate("abcde", 5), "abcde");
    }

    #[test]
    fn truncate_longer_than_limit() {
        assert_eq!(truncate("abcdefgh", 3), "abc");
    }

    #[test]
    fn truncate_empty() {
        assert_eq!(truncate("", 5), "");
    }

    #[test]
    fn truncate_unicode() {
        // Each emoji is one char but multiple bytes
        let s = "🎉🎊🎈🎁";
        assert_eq!(truncate(s, 2), "🎉🎊");
    }

    // -- generate_command_selection_prompt --

    #[test]
    fn prompt_includes_ticket_info() {
        let prompt =
            generate_command_selection_prompt("Fix the bug", "There is a null pointer", "", "");
        assert!(prompt.contains("Fix the bug"));
        assert!(prompt.contains("There is a null pointer"));
    }

    #[test]
    fn prompt_includes_available_commands() {
        let prompt = generate_command_selection_prompt("T", "D", "", "");
        assert!(prompt.contains("- `cleanup`:"));
        assert!(prompt.contains("- `code-review`:"));
        assert!(prompt.contains("- `deslop`:"));
        assert!(prompt.contains("- `unit-tests`:"));
        // Excluded commands should not appear in the available commands list
        assert!(!prompt.contains("- `add-and-commit`:"));
        assert!(!prompt.contains("- `code-review-fix`:"));
    }

    #[test]
    fn prompt_includes_models() {
        let prompt = generate_command_selection_prompt("T", "D", "", "");
        assert!(prompt.contains("`opus-4.6`"));
        assert!(prompt.contains("`sonnet-4.6`"));
    }

    #[test]
    fn prompt_omits_empty_plan_and_impl() {
        let prompt = generate_command_selection_prompt("T", "D", "", "");
        assert!(!prompt.contains("## Plan"));
        assert!(!prompt.contains("## Implementation Summary"));
    }

    #[test]
    fn prompt_includes_plan_and_impl_when_provided() {
        let prompt = generate_command_selection_prompt(
            "T",
            "D",
            "Step 1: do X",
            "Changed file Y",
        );
        assert!(prompt.contains("## Plan"));
        assert!(prompt.contains("Step 1: do X"));
        assert!(prompt.contains("## Implementation Summary"));
        assert!(prompt.contains("Changed file Y"));
    }
}
