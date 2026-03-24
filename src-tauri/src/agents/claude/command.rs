//! Claude CLI command building.

use crate::agents::provider::AgentRunConfig;

/// Map short/display model names to full Claude CLI identifiers.
///
/// The config layer may store short names like `opus-4.6` while the
/// Claude CLI requires full identifiers like `claude-opus-4-6`.
/// Names already in CLI format pass through unchanged.
pub fn normalize_model_for_cli(model: &str) -> String {
    match model {
        "opus-4.6" => "claude-opus-4-6".to_string(),
        "opus-4.5" => "claude-opus-4-5".to_string(),
        "sonnet-4.6" => "claude-sonnet-4-6".to_string(),
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

    let api_config = ClaudeApiConfig::from_agent_config(&config.agent_config);

    let effective_model = api_config
        .model_override
        .as_ref()
        .filter(|s| !s.is_empty())
        .cloned()
        .or_else(|| config.model.clone());

    if let Some(ref model) = effective_model {
        args.push("--model".to_string());
        args.push(normalize_model_for_cli(model));
    }
    let thinking = api_config.thinking_enabled.unwrap_or(true);
    let extended_context = api_config.extended_context_enabled.unwrap_or(false);
    let chrome = api_config.chrome_enabled.unwrap_or(false);
    push_cli_option_flags_raw(&mut args, thinking, extended_context, chrome);

    if let Some(ref tools) = api_config.allowed_tools {
        args.push("--tools".to_string());
        args.push(tools.clone());
    }

    if let Some(ref sid) = config.session_id {
        args.push("--resume".to_string());
        args.push(sid.clone());
    }

    for wp in &config.workspace_paths {
        if wp != &config.repo_path {
            args.push("--add-dir".to_string());
            args.push(wp.to_string_lossy().to_string());
        }
    }

    args.push("-p".to_string());
    args.push(config.prompt.clone());

    (command, args)
}
