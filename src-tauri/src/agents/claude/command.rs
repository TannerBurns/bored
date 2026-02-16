//! Claude CLI command building.

use super::super::AgentRunConfig;
use crate::agents::provider::AgentRunConfig as ProviderAgentRunConfig;

/// Default model used when none is explicitly specified
const DEFAULT_MODEL: &str = "opus-4.6";

/// Map normalized model name to Claude Code format
/// e.g., "sonnet-4.5" -> "claude-sonnet-4-5"
fn map_model_for_claude(model: &str) -> String {
    match model {
        "opus-4.6" => "claude-opus-4-6".to_string(),
        "opus-4.5" => "claude-opus-4-5".to_string(),
        "sonnet-4.5" => "claude-sonnet-4-5".to_string(),
        other => other.to_string(),
    }
}

/// Push conditional CLI flags based on ClaudeApiConfig settings (legacy path).
fn push_cli_option_flags(args: &mut Vec<String>, config: &AgentRunConfig) {
    let api_config = config.claude_api_config.as_ref();
    let thinking = api_config.and_then(|c| c.thinking_enabled).unwrap_or(true);
    let extended_context = api_config
        .and_then(|c| c.extended_context_enabled)
        .unwrap_or(false);
    let chrome = api_config.and_then(|c| c.chrome_enabled).unwrap_or(false);

    push_cli_option_flags_raw(args, thinking, extended_context, chrome);
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

/// Build command from the legacy `AgentRunConfig` (still used by existing callers).
pub fn build_command(config: &AgentRunConfig) -> (String, Vec<String>) {
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

    push_cli_option_flags(&mut args, config);

    args.push("-p".to_string());
    args.push(config.prompt.clone());

    (command, args)
}

/// Build command from the provider-based `AgentRunConfig`.
pub fn build_command_from_provider_config(config: &ProviderAgentRunConfig) -> (String, Vec<String>) {
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

#[allow(dead_code)]
pub fn build_command_with_settings(
    config: &AgentRunConfig,
    settings: &ClaudeSettings,
) -> (String, Vec<String>) {
    let command = settings
        .executable_path
        .clone()
        .unwrap_or_else(|| "claude".to_string());

    let mut args = vec![];

    if let Some(ref prompt) = settings.system_prompt {
        args.push("--append-system-prompt".to_string());
        args.push(prompt.clone());
    } else if let Some(ref file) = settings.system_prompt_file {
        args.push("--system-prompt-file".to_string());
        args.push(file.clone());
    }

    if let Some(ref mode) = settings.permission_mode {
        args.push("--permission-mode".to_string());
        args.push(mode.clone());
    }

    push_cli_option_flags(&mut args, config);

    args.push("-p".to_string());
    args.push(config.prompt.clone());
    args.extend(settings.extra_flags.clone());

    (command, args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_config() -> AgentRunConfig {
        AgentRunConfig {
            kind: super::super::super::AgentKind::Claude,
            ticket_id: "test-ticket".to_string(),
            run_id: "test-run".to_string(),
            repo_path: PathBuf::from("/tmp/test"),
            prompt: "Test prompt".to_string(),
            timeout_secs: Some(300),
            api_url: "http://localhost:7432".to_string(),
            api_token: "token".to_string(),
            model: None,
            claude_api_config: None,
            agent_config: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn build_command_returns_claude() {
        let config = create_test_config();
        let (cmd, _) = build_command(&config);
        assert_eq!(cmd, "claude");
    }

    #[test]
    fn build_command_includes_prompt() {
        let config = create_test_config();
        let (_, args) = build_command(&config);
        assert!(args.contains(&"-p".to_string()));
        assert!(args.contains(&"Test prompt".to_string()));
    }

    #[test]
    fn build_command_prompt_immediately_follows_p_flag() {
        let config = create_test_config();
        let (_, args) = build_command(&config);

        let p_index = args
            .iter()
            .position(|a| a == "-p")
            .expect("-p flag must be present");
        let prompt_index = args
            .iter()
            .position(|a| a == "Test prompt")
            .expect("prompt must be present");

        assert_eq!(
            prompt_index,
            p_index + 1,
            "-p must be immediately followed by the prompt"
        );
    }

    #[test]
    fn build_command_includes_model_when_specified() {
        let mut config = create_test_config();
        config.model = Some("sonnet-4.5".to_string());
        let (_, args) = build_command(&config);
        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"claude-sonnet-4-5".to_string()));
        assert_eq!(args.last(), Some(&"Test prompt".to_string()));
    }

    #[test]
    fn build_command_maps_model_names_correctly() {
        let test_cases = [
            ("opus-4.6", "claude-opus-4-6"),
            ("opus-4.5", "claude-opus-4-5"),
            ("sonnet-4.5", "claude-sonnet-4-5"),
            ("unknown-model", "unknown-model"),
        ];

        for (input, expected) in test_cases {
            let mut config = create_test_config();
            config.model = Some(input.to_string());
            let (_, args) = build_command(&config);
            assert!(
                args.contains(&expected.to_string()),
                "Expected {} to be mapped to {}",
                input,
                expected
            );
        }
    }

    #[test]
    fn build_command_defaults_to_opus_4_6_when_none() {
        let config = create_test_config();
        let (_, args) = build_command(&config);
        assert!(args.contains(&"--model".to_string()));
        assert!(
            args.contains(&"claude-opus-4-6".to_string()),
            "Should default to claude-opus-4-6 when no model specified"
        );
    }

    #[test]
    fn build_command_ignores_model_override() {
        // model_override on ClaudeApiConfig should NOT affect model selection.
        // Only config.model (from workflow settings) matters.
        let mut config = create_test_config();
        config.model = Some("sonnet-4.5".to_string());
        config.claude_api_config = Some(super::super::super::ClaudeApiConfig {
            model_override: Some("custom-model-override".to_string()),
            ..Default::default()
        });
        let (_, args) = build_command(&config);
        assert!(
            args.contains(&"claude-sonnet-4-5".to_string()),
            "Per-stage model from workflow settings should always be used"
        );
        assert!(
            !args.contains(&"custom-model-override".to_string()),
            "model_override should never override workflow settings"
        );
    }

    #[test]
    fn build_command_excludes_betas_by_default() {
        let config = create_test_config();
        let (_, args) = build_command(&config);
        assert!(
            !args.contains(&"--betas".to_string()),
            "Extended context should be off by default"
        );
    }

    #[test]
    fn build_command_includes_betas_when_enabled() {
        let mut config = create_test_config();
        config.claude_api_config = Some(super::super::super::ClaudeApiConfig {
            extended_context_enabled: Some(true),
            ..Default::default()
        });
        let (_, args) = build_command(&config);
        let beta_index = args
            .iter()
            .position(|a| a == "--betas")
            .expect("--betas flag must be present when enabled");
        assert_eq!(
            args[beta_index + 1], "context-1m-2025-08-07",
            "--betas must be followed by context-1m-2025-08-07"
        );
    }

    #[test]
    fn build_command_includes_thinking_by_default() {
        let config = create_test_config();
        let (_, args) = build_command(&config);
        assert!(
            args.contains(&"--settings".to_string()),
            "Thinking should be enabled by default"
        );
        assert!(
            args.contains(&r#"{"alwaysThinkingEnabled": true}"#.to_string()),
            "Thinking settings value should be present"
        );
    }

    #[test]
    fn build_command_excludes_thinking_when_disabled() {
        let mut config = create_test_config();
        config.claude_api_config = Some(super::super::super::ClaudeApiConfig {
            thinking_enabled: Some(false),
            ..Default::default()
        });
        let (_, args) = build_command(&config);
        assert!(
            !args.contains(&"--settings".to_string()),
            "Thinking flag should not be present when disabled"
        );
    }

    #[test]
    fn build_command_excludes_chrome_by_default() {
        let config = create_test_config();
        let (_, args) = build_command(&config);
        assert!(
            !args.contains(&"--chrome".to_string()),
            "Chrome should be off by default"
        );
    }

    #[test]
    fn build_command_includes_chrome_when_enabled() {
        let mut config = create_test_config();
        config.claude_api_config = Some(super::super::super::ClaudeApiConfig {
            chrome_enabled: Some(true),
            ..Default::default()
        });
        let (_, args) = build_command(&config);
        assert!(
            args.contains(&"--chrome".to_string()),
            "Chrome flag should be present when enabled"
        );
    }

    #[test]
    fn build_command_all_cli_options_enabled() {
        let mut config = create_test_config();
        config.claude_api_config = Some(super::super::super::ClaudeApiConfig {
            thinking_enabled: Some(true),
            extended_context_enabled: Some(true),
            chrome_enabled: Some(true),
            ..Default::default()
        });
        let (_, args) = build_command(&config);
        assert!(args.contains(&"--settings".to_string()));
        assert!(args.contains(&"--betas".to_string()));
        assert!(args.contains(&"--chrome".to_string()));
    }

    #[test]
    fn build_command_all_cli_options_disabled() {
        let mut config = create_test_config();
        config.claude_api_config = Some(super::super::super::ClaudeApiConfig {
            thinking_enabled: Some(false),
            extended_context_enabled: Some(false),
            chrome_enabled: Some(false),
            ..Default::default()
        });
        let (_, args) = build_command(&config);
        assert!(!args.contains(&"--settings".to_string()));
        assert!(!args.contains(&"--betas".to_string()));
        assert!(!args.contains(&"--chrome".to_string()));
    }

    #[test]
    fn default_settings() {
        let settings = ClaudeSettings::default();
        assert!(settings.executable_path.is_none());
        assert!(settings.system_prompt.is_none());
        assert!(settings.permission_mode.is_none());
    }

    #[test]
    fn build_with_system_prompt() {
        let config = create_test_config();
        let settings = ClaudeSettings {
            system_prompt: Some("Be helpful".to_string()),
            ..Default::default()
        };
        let (_, args) = build_command_with_settings(&config, &settings);
        assert!(args.contains(&"--append-system-prompt".to_string()));
        assert!(args.contains(&"Be helpful".to_string()));
    }

    #[test]
    fn build_with_permission_mode() {
        let config = create_test_config();
        let settings = ClaudeSettings {
            permission_mode: Some("ask".to_string()),
            ..Default::default()
        };
        let (_, args) = build_command_with_settings(&config, &settings);
        assert!(args.contains(&"--permission-mode".to_string()));
        assert!(args.contains(&"ask".to_string()));
    }

    #[test]
    fn build_with_custom_executable() {
        let config = create_test_config();
        let settings = ClaudeSettings {
            executable_path: Some("/usr/local/bin/claude".to_string()),
            ..Default::default()
        };
        let (cmd, _) = build_command_with_settings(&config, &settings);
        assert_eq!(cmd, "/usr/local/bin/claude");
    }

    // --- build_command_with_settings CLI option tests ---

    #[test]
    fn build_with_settings_includes_thinking_by_default() {
        let config = create_test_config();
        let settings = ClaudeSettings::default();
        let (_, args) = build_command_with_settings(&config, &settings);
        assert!(
            args.contains(&"--settings".to_string()),
            "Thinking should be on by default in build_command_with_settings"
        );
    }

    #[test]
    fn build_with_settings_excludes_betas_by_default() {
        let config = create_test_config();
        let settings = ClaudeSettings::default();
        let (_, args) = build_command_with_settings(&config, &settings);
        assert!(
            !args.contains(&"--betas".to_string()),
            "Extended context should be off by default in build_command_with_settings"
        );
    }

    #[test]
    fn build_with_settings_excludes_chrome_by_default() {
        let config = create_test_config();
        let settings = ClaudeSettings::default();
        let (_, args) = build_command_with_settings(&config, &settings);
        assert!(
            !args.contains(&"--chrome".to_string()),
            "Chrome should be off by default in build_command_with_settings"
        );
    }

    #[test]
    fn build_with_settings_respects_cli_options() {
        let mut config = create_test_config();
        config.claude_api_config = Some(super::super::super::ClaudeApiConfig {
            thinking_enabled: Some(false),
            extended_context_enabled: Some(true),
            chrome_enabled: Some(true),
            ..Default::default()
        });
        let settings = ClaudeSettings::default();
        let (_, args) = build_command_with_settings(&config, &settings);
        assert!(!args.contains(&"--settings".to_string()));
        assert!(args.contains(&"--betas".to_string()));
        assert!(args.contains(&"--chrome".to_string()));
    }

    #[test]
    fn build_command_explicit_true_same_as_none_for_thinking() {
        // explicit Some(true) and None (default) should both include thinking
        let config_none = create_test_config(); // claude_api_config is None
        let (_, args_none) = build_command(&config_none);

        let mut config_explicit = create_test_config();
        config_explicit.claude_api_config = Some(super::super::super::ClaudeApiConfig {
            thinking_enabled: Some(true),
            ..Default::default()
        });
        let (_, args_explicit) = build_command(&config_explicit);

        assert!(args_none.contains(&"--settings".to_string()));
        assert!(args_explicit.contains(&"--settings".to_string()));
    }

    #[test]
    fn build_command_prompt_is_last_with_cli_options() {
        let mut config = create_test_config();
        config.claude_api_config = Some(super::super::super::ClaudeApiConfig {
            thinking_enabled: Some(true),
            extended_context_enabled: Some(true),
            chrome_enabled: Some(true),
            ..Default::default()
        });
        let (_, args) = build_command(&config);
        assert_eq!(
            args.last(),
            Some(&"Test prompt".to_string()),
            "Prompt must be the last argument even with all CLI options enabled"
        );
    }
}
