//! Claude CLI command building.

use crate::agents::provider::AgentRunConfig;

/// Default model used when none is explicitly specified
const DEFAULT_MODEL: &str = "opus-4.6";

/// Map normalized model name to Claude Code format
fn map_model_for_claude(model: &str) -> String {
    match model {
        "opus-4.6" => "claude-opus-4-6".to_string(),
        "opus-4.5" => "claude-opus-4-5".to_string(),
        "sonnet-4.5" => "claude-sonnet-4-5".to_string(),
        other => other.to_string(),
    }
}

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

    let model = config.model.as_deref().unwrap_or(DEFAULT_MODEL);
    args.push("--model".to_string());
    args.push(map_model_for_claude(model));

    let api_config = ClaudeApiConfig::from_agent_config(&config.agent_config);
    let thinking = api_config.thinking_enabled.unwrap_or(true);
    let extended_context = api_config.extended_context_enabled.unwrap_or(false);
    let chrome = api_config.chrome_enabled.unwrap_or(false);
    push_cli_option_flags_raw(&mut args, thinking, extended_context, chrome);

    args.push("-p".to_string());
    args.push(config.prompt.clone());

    (command, args)
}

#[derive(Debug, Clone, Default)]
pub struct ClaudeSettings {
    pub executable_path: Option<String>,
    pub system_prompt: Option<String>,
    pub system_prompt_file: Option<String>,
    pub extra_flags: Vec<String>,
    pub permission_mode: Option<String>,
}
