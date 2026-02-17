//! Claude CLI command building.

use crate::agents::provider::AgentRunConfig;

/// Push conditional CLI flags from raw booleans.
fn push_cli_option_flags_raw(
    args: &mut Vec<String>,
    thinking: bool,
    extended_context: bool,
    chrome: bool,
) {
    if thinking {
        args.push("--settings".to_string());
        args.push(r#"{"alwaysThinkingEnabled": true}"#.to_string());
    }

    if extended_context {
        args.push("--betas".to_string());
        args.push("context-1m-2025-08-07".to_string());
    }

    if chrome {
        args.push("--chrome".to_string());
    }
}

/// Build command from the provider-based `AgentRunConfig`.
pub fn build_command_from_provider_config(config: &AgentRunConfig) -> (String, Vec<String>) {
    use super::provider::ClaudeApiConfig;

    let command = "claude".to_string();
    let mut args = vec![
        "--output-format".to_string(),
        "stream-json".to_string(),
        "--verbose".to_string(),
        "--dangerously-skip-permissions".to_string(),
    ];

    if let Some(ref model) = config.model {
        args.push("--model".to_string());
        args.push(model.clone());
    }

    let api_config = ClaudeApiConfig::from_agent_config(&config.agent_config);
    let thinking = api_config.thinking_enabled.unwrap_or(true);
    let extended_context = api_config.extended_context_enabled.unwrap_or(false);
    let chrome = api_config.chrome_enabled.unwrap_or(false);
    push_cli_option_flags_raw(&mut args, thinking, extended_context, chrome);

    args.push("-p".to_string());
    args.push(config.prompt.clone());

    (command, args)
}
