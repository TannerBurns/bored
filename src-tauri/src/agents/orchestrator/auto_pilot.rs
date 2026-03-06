//! Auto-pilot command selection: prompt generation and response parsing.
//!
//! After plan + implement, the agent is asked which commands should be run
//! and with which models. It returns a JSON array of `{command, model}` pairs.

use std::path::Path;

use super::WorkflowOrchestrator;
use crate::agents::command_templates;

/// A single command+model pair selected by the agent.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

/// Returns sorted, deduplicated command IDs (without `.md` extension)
/// from the custom and bundled command directories.
fn discover_available_commands(
    custom_commands_dir: Option<&Path>,
) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut commands = Vec::new();

    if let Some(custom_dir) = custom_commands_dir {
        for filename in command_templates::discover_commands(custom_dir) {
            let id = filename.trim_end_matches(".md").to_string();
            if !EXCLUDED_COMMANDS.contains(&id.as_str()) && seen.insert(id.clone()) {
                commands.push(id);
            }
        }
    }

    if let Some(bundled_dir) = command_templates::get_bundled_commands_path() {
        for filename in command_templates::discover_commands(&bundled_dir) {
            let id = filename.trim_end_matches(".md").to_string();
            if !EXCLUDED_COMMANDS.contains(&id.as_str()) && seen.insert(id.clone()) {
                commands.push(id);
            }
        }
    }

    commands.sort();
    commands
}

impl WorkflowOrchestrator {
    /// Run the command-selection stage: ask the agent which commands to run.
    pub(super) async fn run_command_selection_stage(
        &self,
        plan: &str,
        impl_summary: &str,
    ) -> Result<Vec<CommandSelection>, String> {
        let custom_dir = self.custom_commands_dir();
        let available = discover_available_commands(custom_dir.as_deref());

        let (effective_title, effective_description, ticket_context) = match &self.task {
            Some(task) => {
                let title = task.title.as_deref().unwrap_or(&self.ticket.title);
                let desc = task.content.as_deref().unwrap_or(&self.ticket.description_md);
                let context = if task.content.as_deref() != Some(&self.ticket.description_md)
                    && !self.ticket.description_md.is_empty()
                {
                    Some(self.ticket.description_md.as_str())
                } else {
                    None
                };
                (title, desc, context)
            }
            None => (
                self.ticket.title.as_str(),
                self.ticket.description_md.as_str(),
                None,
            ),
        };

        let provider_models = self.provider.available_models();

        if provider_models.is_empty() {
            tracing::warn!(
                "Provider '{}' returned no available models; \
                 auto-pilot will use '{}' for all selected commands",
                self.provider.id(),
                self.auto_pilot_model,
            );
        }

        let prompt = generate_command_selection_prompt(
            effective_title,
            effective_description,
            ticket_context,
            plan,
            impl_summary,
            &available,
            &provider_models,
        );

        let result = self
            .run_stage_with_model(
                "command-selection",
                &prompt,
                &self.auto_pilot_model,
            )
            .await;

        match result {
            Ok(run_result) => {
                let raw = run_result.captured_stdout.unwrap_or_default();
                let text = self.extract_text(&raw);
                let selections = parse_command_selection_response(&text, &available);

                tracing::info!(
                    "Command selection: raw={} chars, extracted={} chars, {} commands selected: {:?}",
                    raw.len(),
                    text.len(),
                    selections.len(),
                    selections.iter().map(|s| &s.command).collect::<Vec<_>>(),
                );
                tracing::debug!("Command selection extracted text: {}", truncate(&text, 500));

                Ok(selections)
            }
            Err(e) => {
                tracing::error!("Command selection stage failed: {}", e);
                self.emit_stage_event("command-selection", "error", None, None);
                Ok(Vec::new())
            }
        }
    }
}

fn generate_command_selection_prompt(
    ticket_title: &str,
    ticket_description: &str,
    ticket_context: Option<&str>,
    plan: &str,
    impl_summary: &str,
    available_commands: &[String],
    available_models: &[(&str, &str)],
) -> String {
    let mut commands_list = String::new();
    for id in available_commands {
        commands_list.push_str(&format!("- `{}`\n", id));
    }

    let mut models_list = String::new();
    for &(id, label) in available_models {
        models_list.push_str(&format!("- `{}` ({})\n", id, label));
    }

    let example_models = pick_example_models(available_models);

    let ticket_context_section = match ticket_context {
        Some(ctx) if !ctx.is_empty() => {
            format!("## Original Ticket Context\n\n{}\n\n", ctx)
        }
        _ => String::new(),
    };

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

    let (models_section, model_guidance, examples, model_constraint, footer_example) =
        if let Some((capable, efficient)) = example_models {
            (
                format!("## Available Models\n\n{}\n", models_list),
                format!(
                    r#"**Model selection:**
- Use more capable models (e.g., `{capable}`) for tasks requiring deep reasoning: code review, security review, complex test generation.
- Use cheaper/faster models (e.g., `{efficient}`) for mechanical tasks: linting, cleanup, formatting, simple doc updates.
- ONLY use model names from the Available Models list above."#
                ),
                format!(
                    r#"## Example Workflows

**Quick bug fix** — ticket says "Fix null pointer crash in user lookup":
```json
[
  {{"command": "cleanup", "model": "{efficient}"}},
  {{"command": "unit-tests", "model": "{efficient}"}}
]
```

**Standard feature** — ticket says "Add email notification preferences to settings":
```json
[
  {{"command": "code-review", "model": "{capable}"}},
  {{"command": "cleanup", "model": "{efficient}"}},
  {{"command": "unit-tests", "model": "{capable}"}},
  {{"command": "deslop", "model": "{efficient}"}}
]
```

**Comprehensive / production-ready** — ticket says "Implement OAuth2 login flow — needs to be thorough and production-ready":
```json
[
  {{"command": "code-review", "model": "{capable}"}},
  {{"command": "patch-security", "model": "{capable}"}},
  {{"command": "unit-tests", "model": "{capable}"}},
  {{"command": "integration-test", "model": "{capable}"}},
  {{"command": "cleanup", "model": "{efficient}"}},
  {{"command": "deslop", "model": "{efficient}"}},
  {{"command": "review-changes", "model": "{capable}"}},
  {{"command": "doc-sync", "model": "{efficient}"}}
]
```

**Trivial change** — ticket says "Fix typo in README":
```json
[]
```"#
                ),
                "- ONLY use model names from the Available Models list.\n".to_string(),
                format!(
                    r#"[{{"command": "code-review", "model": "{capable}"}}, {{"command": "cleanup", "model": "{efficient}"}}]"#
                ),
            )
        } else {
            (
                String::new(),
                String::new(),
                r#"## Example Workflows

**Quick bug fix** — ticket says "Fix null pointer crash in user lookup":
```json
[
  {"command": "cleanup", "model": "auto"},
  {"command": "unit-tests", "model": "auto"}
]
```

**Standard feature** — ticket says "Add email notification preferences to settings":
```json
[
  {"command": "code-review", "model": "auto"},
  {"command": "cleanup", "model": "auto"},
  {"command": "unit-tests", "model": "auto"},
  {"command": "deslop", "model": "auto"}
]
```

**Trivial change** — ticket says "Fix typo in README":
```json
[]
```"#
                .to_string(),
                String::new(),
                r#"[{"command": "code-review", "model": "auto"}, {"command": "cleanup", "model": "auto"}]"#
                    .to_string(),
            )
        };

    format!(
        r#"You are a workflow orchestrator deciding which quality assurance commands should run after an implementation is complete. Your job is to read the ticket, plan, and implementation, then build a tailored QA workflow.

## Ticket

**Title:** {ticket_title}
**Description:** {ticket_description}

{ticket_context_section}{plan_section}{impl_section}## Available Commands

{commands_list}
{models_section}## How to Reason About Workflow Selection

**Pay close attention to the user's intent in the ticket title and description.** The user's language signals how thorough the workflow should be:

- Words like "comprehensive", "production-ready", "thorough", "bulletproof", or "full review" mean the user wants a deep QA pipeline with multiple passes.
- Words like "quick", "hotfix", "small fix", "typo", "minor", or "just do X" mean the user wants minimal overhead — only run commands that are truly necessary.
- If the description mentions specific concerns (e.g., "make sure tests pass", "check for security issues", "update the docs"), target those areas directly.
- If there's no strong signal, use your judgment based on the scope and risk of the implementation.

**Match workflow depth to implementation scope:**
- A one-line config change needs little or no QA.
- A new feature touching multiple files needs review, tests, and cleanup.
- Changes to authentication, payments, or data handling need security review.
- Public API changes need contract checks and documentation.

{model_guidance}

{examples}

## Instructions

Based on the ticket, plan, and implementation, select which commands to run and in what order. Follow these rules:

- Do NOT include `add-and-commit` — it always runs automatically at the end.
- Only select commands that are relevant to the changes.
{model_constraint}- Order matters: put fix/review commands before final polish and documentation.
- You may return an empty array `[]` if no QA commands are needed.

IMPORTANT: Your response must contain ONLY a valid JSON array of objects with "command" and "model" keys — no prose, no markdown, no explanation. Example format:

{footer_example}
"#
    )
}

/// Pick a "capable" and "efficient" model from the available list for prompt examples.
/// The first model is assumed to be the most capable, the last the most efficient.
/// Returns `None` when the list is empty — callers must omit model-specific
/// guidance rather than injecting a bogus placeholder name.
fn pick_example_models<'a>(models: &[(&'a str, &'a str)]) -> Option<(&'a str, &'a str)> {
    match models.len() {
        0 => None,
        1 => Some((models[0].0, models[0].0)),
        _ => Some((models[0].0, models[models.len() - 1].0)),
    }
}

/// Parse the agent's response to extract the command selection list.
pub fn parse_command_selection_response(
    response: &str,
    available_commands: &[String],
) -> Vec<CommandSelection> {
    match crate::agents::json_extraction::parse_json_response::<Vec<CommandSelection>>(response) {
        Some(raw) => filter_valid_selections(raw, available_commands),
        None => {
            tracing::warn!(
                "Could not parse command selection JSON from agent response ({} chars). Response: {}",
                response.len(),
                truncate(response, 1000),
            );
            Vec::new()
        }
    }
}

fn filter_valid_selections(
    selections: Vec<CommandSelection>,
    available_commands: &[String],
) -> Vec<CommandSelection> {
    let valid: std::collections::HashSet<&str> =
        available_commands.iter().map(|s| s.as_str()).collect();

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
            if !valid.contains(s.command.as_str()) {
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

    fn test_commands() -> Vec<String> {
        vec![
            "cleanup", "code-review", "deslop", "unit-tests", "review-changes",
            "add-tests", "fix-lint", "sync-with-main", "review-polish",
            "patch-security", "api-contract-check", "observability-pass",
            "integration-test", "doc-sync",
        ]
        .into_iter()
        .map(String::from)
        .collect()
    }

    fn claude_models() -> Vec<(&'static str, &'static str)> {
        vec![
            ("claude-opus-4-6", "Claude Opus 4.6"),
            ("claude-opus-4-5", "Claude Opus 4.5"),
            ("claude-sonnet-4-6", "Claude Sonnet 4.6"),
            ("claude-sonnet-4-5", "Claude Sonnet 4.5"),
        ]
    }

    fn codex_models() -> Vec<(&'static str, &'static str)> {
        vec![
            ("gpt-5.4", "GPT-5.4"),
            ("gpt-5.3-codex", "GPT-5.3 Codex"),
            ("gpt-5.2-codex", "GPT-5.2 Codex"),
        ]
    }

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
        let result = parse_command_selection_response(response, &test_commands());
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].command, "cleanup");
        assert_eq!(result[0].model, "sonnet-4.6");
        assert_eq!(result[1].command, "code-review");
        assert_eq!(result[1].model, "opus-4.6");
    }

    #[test]
    fn parse_raw_json_array() {
        let response = r#"[{"command": "deslop", "model": "sonnet-4.5"}]"#;
        let result = parse_command_selection_response(response, &test_commands());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].command, "deslop");
    }

    #[test]
    fn parse_empty_array() {
        let response = "```json\n[]\n```";
        let result = parse_command_selection_response(response, &test_commands());
        assert!(result.is_empty());
    }

    #[test]
    fn filters_excluded_commands() {
        let response =
            r#"[{"command": "add-and-commit", "model": "sonnet-4.6"}, {"command": "cleanup", "model": "sonnet-4.6"}]"#;
        let result = parse_command_selection_response(response, &test_commands());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].command, "cleanup");
    }

    #[test]
    fn filters_unknown_commands() {
        let response =
            r#"[{"command": "nonexistent", "model": "opus-4.6"}, {"command": "deslop", "model": "opus-4.5"}]"#;
        let result = parse_command_selection_response(response, &test_commands());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].command, "deslop");
    }

    #[test]
    fn parse_unparseable_returns_empty() {
        let result = parse_command_selection_response("I don't know what to do.", &test_commands());
        assert!(result.is_empty());
    }

    #[test]
    fn parse_json_with_surrounding_text() {
        let response = r#"Based on the implementation, I recommend:

[{"command": "unit-tests", "model": "opus-4.5"}, {"command": "review-changes", "model": "opus-4.5"}]

These will ensure quality."#;
        let result = parse_command_selection_response(response, &test_commands());
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].command, "unit-tests");
        assert_eq!(result[1].command, "review-changes");
    }

    #[test]
    fn filters_code_review_fix_command() {
        let response =
            r#"[{"command": "code-review-fix", "model": "opus-4.6"}, {"command": "cleanup", "model": "sonnet-4.6"}]"#;
        let result = parse_command_selection_response(response, &test_commands());
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
        let result = parse_command_selection_response(response, &test_commands());
        assert_eq!(result.len(), 9);
    }

    #[test]
    fn accepts_custom_commands_when_in_available_list() {
        let mut cmds = test_commands();
        cmds.push("my-custom-lint".to_string());
        let response = r#"[{"command": "my-custom-lint", "model": "sonnet-4.6"}]"#;
        let result = parse_command_selection_response(response, &cmds);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].command, "my-custom-lint");
    }

    #[test]
    fn rejects_custom_commands_not_in_available_list() {
        let response = r#"[{"command": "my-custom-lint", "model": "sonnet-4.6"}]"#;
        let result = parse_command_selection_response(response, &test_commands());
        assert!(result.is_empty());
    }

    // ── truncate ──────────────────────────────────────────────

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

    // ── generate_command_selection_prompt ───────────────────────

    #[test]
    fn prompt_includes_ticket_info() {
        let cmds = test_commands();
        let prompt =
            generate_command_selection_prompt("Fix the bug", "There is a null pointer", None, "", "", &cmds, &claude_models());
        assert!(prompt.contains("Fix the bug"));
        assert!(prompt.contains("There is a null pointer"));
    }

    #[test]
    fn prompt_includes_available_commands() {
        let cmds = test_commands();
        let prompt = generate_command_selection_prompt("T", "D", None, "", "", &cmds, &claude_models());
        assert!(prompt.contains("- `cleanup`"));
        assert!(prompt.contains("- `code-review`"));
        assert!(prompt.contains("- `deslop`"));
        assert!(prompt.contains("- `unit-tests`"));
    }

    #[test]
    fn prompt_includes_custom_commands() {
        let cmds = vec!["cleanup".to_string(), "my-custom-deploy".to_string()];
        let prompt = generate_command_selection_prompt("T", "D", None, "", "", &cmds, &claude_models());
        assert!(prompt.contains("- `cleanup`"));
        assert!(prompt.contains("- `my-custom-deploy`"));
    }

    #[test]
    fn prompt_includes_example_workflows() {
        let cmds = test_commands();
        let prompt = generate_command_selection_prompt("T", "D", None, "", "", &cmds, &claude_models());
        assert!(prompt.contains("Quick bug fix"));
        assert!(prompt.contains("Standard feature"));
        assert!(prompt.contains("Comprehensive / production-ready"));
        assert!(prompt.contains("Trivial change"));
    }

    #[test]
    fn prompt_includes_user_intent_guidance() {
        let cmds = test_commands();
        let prompt = generate_command_selection_prompt("T", "D", None, "", "", &cmds, &claude_models());
        assert!(prompt.contains("Pay close attention to the user's intent"));
        assert!(prompt.contains("comprehensive"));
        assert!(prompt.contains("quick"));
        assert!(prompt.contains("hotfix"));
    }

    #[test]
    fn prompt_includes_provider_models() {
        let cmds = test_commands();
        let prompt = generate_command_selection_prompt("T", "D", None, "", "", &cmds, &claude_models());
        assert!(prompt.contains("`claude-opus-4-6` (Claude Opus 4.6)"));
        assert!(prompt.contains("`claude-sonnet-4-5` (Claude Sonnet 4.5)"));
    }

    #[test]
    fn prompt_uses_codex_models_when_provided() {
        let cmds = test_commands();
        let prompt = generate_command_selection_prompt("T", "D", None, "", "", &cmds, &codex_models());
        assert!(prompt.contains("`gpt-5.4` (GPT-5.4)"));
        assert!(prompt.contains("`gpt-5.3-codex` (GPT-5.3 Codex)"));
        assert!(prompt.contains("`gpt-5.2-codex` (GPT-5.2 Codex)"));
        assert!(!prompt.contains("opus"), "Codex prompt should not mention opus models");
        assert!(prompt.contains(r#""model": "gpt-5.4""#), "examples should use codex models");
        assert!(prompt.contains(r#""model": "gpt-5.2-codex""#), "examples should use codex models");
    }

    #[test]
    fn prompt_examples_use_provider_model_names() {
        let cmds = test_commands();

        let claude_prompt = generate_command_selection_prompt("T", "D", None, "", "", &cmds, &claude_models());
        assert!(claude_prompt.contains(r#""model": "claude-opus-4-6""#), "Claude examples should use claude-opus-4-6");
        assert!(claude_prompt.contains(r#""model": "claude-sonnet-4-5""#), "Claude examples should use claude-sonnet-4-5");

        let codex_prompt = generate_command_selection_prompt("T", "D", None, "", "", &cmds, &codex_models());
        assert!(codex_prompt.contains(r#""model": "gpt-5.4""#), "Codex examples should use gpt-5.4");
        assert!(codex_prompt.contains(r#""model": "gpt-5.2-codex""#), "Codex examples should use gpt-5.2-codex");
    }

    #[test]
    fn prompt_omits_empty_plan_and_impl() {
        let cmds = test_commands();
        let prompt = generate_command_selection_prompt("T", "D", None, "", "", &cmds, &claude_models());
        assert!(!prompt.contains("## Plan"));
        assert!(!prompt.contains("## Implementation Summary"));
    }

    #[test]
    fn prompt_includes_plan_and_impl_when_provided() {
        let cmds = test_commands();
        let prompt = generate_command_selection_prompt(
            "T", "D", None, "Step 1: do X", "Changed file Y", &cmds, &claude_models(),
        );
        assert!(prompt.contains("## Plan"));
        assert!(prompt.contains("Step 1: do X"));
        assert!(prompt.contains("## Implementation Summary"));
        assert!(prompt.contains("Changed file Y"));
    }

    #[test]
    fn prompt_uses_task_title_and_content() {
        let cmds = test_commands();
        let prompt = generate_command_selection_prompt(
            "Add integration tests",
            "Write integration tests for the OAuth2 flow",
            Some("Implement OAuth2 login with Google and GitHub providers"),
            "",
            "",
            &cmds,
            &claude_models(),
        );
        assert!(prompt.contains("Add integration tests"));
        assert!(prompt.contains("Write integration tests for the OAuth2 flow"));
        assert!(prompt.contains("## Original Ticket Context"));
        assert!(prompt.contains("Implement OAuth2 login with Google and GitHub providers"));
    }

    #[test]
    fn prompt_omits_ticket_context_when_none() {
        let cmds = test_commands();
        let prompt = generate_command_selection_prompt(
            "Fix the bug", "There is a null pointer", None, "", "", &cmds, &claude_models(),
        );
        assert!(!prompt.contains("## Original Ticket Context"));
    }

    #[test]
    fn prompt_omits_ticket_context_when_empty() {
        let cmds = test_commands();
        let prompt = generate_command_selection_prompt(
            "Fix the bug", "There is a null pointer", Some(""), "", "", &cmds, &claude_models(),
        );
        assert!(!prompt.contains("## Original Ticket Context"));
    }

    #[test]
    fn prompt_uses_important_instruction_not_empty_code_fence() {
        let cmds = test_commands();
        let prompt = generate_command_selection_prompt("T", "D", None, "", "", &cmds, &claude_models());
        assert!(
            prompt.contains("IMPORTANT"),
            "prompt must contain the IMPORTANT instruction"
        );
        assert!(
            prompt.contains(r#""command""#) && prompt.contains(r#""model""#),
            "prompt must show the expected JSON keys in the instruction"
        );

        let instructions_section = prompt.split("## Instructions").last().unwrap();
        assert!(
            !instructions_section.contains("```json\n[]\n```"),
            "instructions must not contain empty array code fence template"
        );
    }

    #[test]
    fn prompt_only_model_constraint() {
        let cmds = test_commands();
        let prompt = generate_command_selection_prompt("T", "D", None, "", "", &cmds, &claude_models());
        assert!(prompt.contains("ONLY use model names from the Available Models list"));
    }

    #[test]
    fn prompt_with_empty_models_uses_auto_not_default() {
        let cmds = test_commands();
        let prompt = generate_command_selection_prompt("T", "D", None, "", "", &cmds, &[]);
        assert!(
            !prompt.contains("\"default\""),
            "prompt must not contain 'default' as a model name"
        );
        assert!(
            !prompt.contains("## Available Models"),
            "prompt should omit the Available Models section when empty"
        );
        assert!(
            !prompt.contains("ONLY use model names from the Available Models list"),
            "prompt should not include model constraint when no models are listed"
        );
        assert!(
            prompt.contains(r#""model": "auto""#),
            "empty-models prompt should use 'auto' as the placeholder model"
        );
        assert!(
            prompt.contains("## Example Workflows"),
            "prompt should still include example workflows"
        );
    }

    // ── pick_example_models ─────────────────────────────────────

    #[test]
    fn pick_models_empty_returns_none() {
        assert!(pick_example_models(&[]).is_none());
    }

    #[test]
    fn pick_models_single_uses_same_for_both() {
        let models = vec![("only-model", "Only Model")];
        let (c, e) = pick_example_models(&models).unwrap();
        assert_eq!(c, "only-model");
        assert_eq!(e, "only-model");
    }

    #[test]
    fn pick_models_multiple_picks_first_and_last() {
        let (c, e) = pick_example_models(&claude_models()).unwrap();
        assert_eq!(c, "claude-opus-4-6");
        assert_eq!(e, "claude-sonnet-4-5");
    }

    #[test]
    fn pick_models_codex() {
        let (c, e) = pick_example_models(&codex_models()).unwrap();
        assert_eq!(c, "gpt-5.4");
        assert_eq!(e, "gpt-5.2-codex");
    }

    // ── discover_available_commands ──────────────────────────────

    #[test]
    fn discover_finds_bundled_commands() {
        let commands = discover_available_commands(None);
        assert!(commands.contains(&"cleanup".to_string()));
        assert!(commands.contains(&"code-review".to_string()));
        assert!(commands.contains(&"deslop".to_string()));
        assert!(!commands.contains(&"add-and-commit".to_string()));
        assert!(!commands.contains(&"code-review-fix".to_string()));
    }

    #[test]
    fn discover_excludes_orchestrator_internal_commands() {
        let commands = discover_available_commands(None);
        for excluded in EXCLUDED_COMMANDS {
            assert!(
                !commands.contains(&excluded.to_string()),
                "'{}' should be excluded from discovered commands",
                excluded
            );
        }
    }

    #[test]
    fn discover_returns_sorted_output() {
        let commands = discover_available_commands(None);
        let mut sorted = commands.clone();
        sorted.sort();
        assert_eq!(commands, sorted);
    }

    #[test]
    fn discover_picks_up_custom_commands() {
        let temp = std::env::temp_dir().join(format!("auto_pilot_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp).unwrap();
        std::fs::write(temp.join("my-custom-deploy.md"), "# deploy").unwrap();

        let commands = discover_available_commands(Some(&temp));
        assert!(
            commands.contains(&"my-custom-deploy".to_string()),
            "Custom command should be discovered"
        );
        assert!(commands.contains(&"cleanup".to_string()));

        std::fs::remove_dir_all(&temp).ok();
    }

    #[test]
    fn discover_deduplicates_across_sources() {
        let temp = std::env::temp_dir().join(format!("auto_pilot_dedup_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp).unwrap();
        std::fs::write(temp.join("cleanup.md"), "# custom cleanup").unwrap();

        let commands = discover_available_commands(Some(&temp));
        let cleanup_count = commands.iter().filter(|c| c.as_str() == "cleanup").count();
        assert_eq!(cleanup_count, 1, "Duplicate commands should be deduplicated");

        std::fs::remove_dir_all(&temp).ok();
    }

    #[test]
    fn filter_with_empty_available_list_rejects_everything() {
        let response = r#"[{"command": "cleanup", "model": "sonnet-4.6"}]"#;
        let result = parse_command_selection_response(response, &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn command_selection_serializes_to_json() {
        let selection = CommandSelection {
            command: "cleanup".to_string(),
            model: "sonnet-4.6".to_string(),
        };
        let json = serde_json::to_value(&selection).unwrap();
        assert_eq!(json["command"], "cleanup");
        assert_eq!(json["model"], "sonnet-4.6");

        let roundtripped: CommandSelection = serde_json::from_value(json).unwrap();
        assert_eq!(roundtripped.command, "cleanup");
        assert_eq!(roundtripped.model, "sonnet-4.6");
    }

    #[test]
    fn command_selection_vec_serializes_for_metadata() {
        let selections = vec![
            CommandSelection { command: "cleanup".to_string(), model: "sonnet-4.6".to_string() },
            CommandSelection { command: "unit-tests".to_string(), model: "opus-4.6".to_string() },
        ];
        let metadata = serde_json::json!({ "auto_pilot_selections": selections });
        let arr = metadata["auto_pilot_selections"].as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["command"], "cleanup");
        assert_eq!(arr[1]["command"], "unit-tests");
    }

    #[test]
    fn filter_mixed_valid_excluded_unknown() {
        let cmds = vec!["cleanup".to_string(), "deslop".to_string()];
        let response = r#"[
            {"command": "cleanup", "model": "sonnet-4.6"},
            {"command": "add-and-commit", "model": "sonnet-4.6"},
            {"command": "nonexistent", "model": "opus-4.6"},
            {"command": "deslop", "model": "sonnet-4.5"}
        ]"#;
        let result = parse_command_selection_response(response, &cmds);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].command, "cleanup");
        assert_eq!(result[1].command, "deslop");
    }

    #[test]
    fn discover_with_empty_custom_dir_returns_bundled_only() {
        let temp = std::env::temp_dir().join(format!("auto_pilot_empty_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp).unwrap();

        let commands = discover_available_commands(Some(&temp));
        assert!(commands.contains(&"cleanup".to_string()), "Bundled commands should be present");
        assert!(commands.contains(&"deslop".to_string()), "Bundled commands should be present");
        assert!(!commands.contains(&"add-and-commit".to_string()), "Excluded should still be excluded");

        std::fs::remove_dir_all(&temp).ok();
    }

    #[test]
    fn discover_ignores_non_md_files_in_custom_dir() {
        let temp = std::env::temp_dir().join(format!("auto_pilot_nonmd_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp).unwrap();
        std::fs::write(temp.join("readme.txt"), "not a command").unwrap();
        std::fs::write(temp.join("config.json"), "{}").unwrap();
        std::fs::write(temp.join("real-cmd.md"), "# a command").unwrap();

        let commands = discover_available_commands(Some(&temp));
        assert!(commands.contains(&"real-cmd".to_string()));
        assert!(!commands.iter().any(|c| c.contains("readme")));
        assert!(!commands.iter().any(|c| c.contains("config")));

        std::fs::remove_dir_all(&temp).ok();
    }

    #[test]
    fn discover_with_nonexistent_custom_dir_returns_bundled() {
        let commands = discover_available_commands(Some(Path::new("/nonexistent/path")));
        assert!(commands.contains(&"cleanup".to_string()));
        assert!(!commands.contains(&"add-and-commit".to_string()));
    }

    #[test]
    fn discover_none_excludes_code_review_fix() {
        let commands = discover_available_commands(None);
        assert!(
            !commands.contains(&"code-review-fix".to_string()),
            "code-review-fix should be excluded"
        );
    }

}
