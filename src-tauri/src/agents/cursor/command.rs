//! Cursor CLI command building.

use super::super::AgentRunConfig;

/// Map normalized model name to Claude format for Cursor
/// e.g., "opus-4.5" -> "claude-opus-4-5"
fn map_model_for_cursor(model: &str) -> String {
    match model {
        "opus-4.6" => "claude-opus-4-6".to_string(),
        "opus-4.5" => "claude-opus-4-5".to_string(),
        "sonnet-4.5" => "claude-sonnet-4-5".to_string(),
        "sonnet-4" => "claude-sonnet-4".to_string(),
        "haiku-4.5" => "claude-haiku-4-5".to_string(),
        other => other.to_string(),
    }
}

pub fn build_command(config: &AgentRunConfig) -> (String, Vec<String>) {
    let command = "cursor".to_string();
    let mut args = vec![
        "agent".to_string(),
        "--print".to_string(),
        "--force".to_string(),
        "--approve-mcps".to_string(),
        "--output-format".to_string(),
        "text".to_string(),
        // Explicitly set workspace so Cursor finds .cursor/hooks.json
        "--workspace".to_string(),
        config.repo_path.to_string_lossy().to_string(),
    ];

    // Prioritize model_override from Claude API settings, then fall back to config.model
    let model_to_use = config
        .claude_api_config
        .as_ref()
        .and_then(|c| c.model_override.as_ref())
        .filter(|s| !s.is_empty())
        .cloned();

    if let Some(model) = model_to_use {
        args.push("--model".to_string());
        args.push(model);
    } else if let Some(ref model) = config.model {
        args.push("--model".to_string());
        args.push(map_model_for_cursor(model));
    }

    args.push(config.prompt.clone());
    (command, args)
}

#[derive(Debug, Clone, Default)]
pub struct CursorSettings {
    pub executable_path: Option<String>,
    pub extra_flags: Vec<String>,
}

#[allow(dead_code)]
pub fn build_command_with_settings(
    config: &AgentRunConfig,
    settings: &CursorSettings,
) -> (String, Vec<String>) {
    let command = settings
        .executable_path
        .clone()
        .unwrap_or_else(|| "cursor".to_string());

    // Build args with proper Cursor CLI syntax
    let mut args = vec![
        "agent".to_string(),
        "--print".to_string(),
        "--force".to_string(),
        "--approve-mcps".to_string(),
        "--output-format".to_string(),
        "text".to_string(),
        // Explicitly set workspace so Cursor finds .cursor/hooks.json
        "--workspace".to_string(),
        config.repo_path.to_string_lossy().to_string(),
    ];

    // Add extra flags before the prompt
    args.extend(settings.extra_flags.clone());

    // Prompt is a positional argument at the end
    args.push(config.prompt.clone());

    (command, args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_config() -> AgentRunConfig {
        AgentRunConfig {
            kind: super::super::super::AgentKind::Cursor,
            ticket_id: "test-ticket".to_string(),
            run_id: "test-run".to_string(),
            repo_path: PathBuf::from("/tmp/test"),
            prompt: "Test prompt".to_string(),
            timeout_secs: Some(300),
            api_url: "http://localhost:7432".to_string(),
            api_token: "token".to_string(),
            model: None,
            claude_api_config: None,
        }
    }

    #[test]
    fn build_command_returns_cursor() {
        let config = create_test_config();
        let (cmd, _) = build_command(&config);
        assert_eq!(cmd, "cursor");
    }

    #[test]
    fn build_command_includes_agent_flag() {
        let config = create_test_config();
        let (_, args) = build_command(&config);
        assert_eq!(args[0], "agent");
    }

    #[test]
    fn build_command_includes_prompt() {
        let config = create_test_config();
        let (_, args) = build_command(&config);
        // Prompt is a positional argument at the end
        assert!(args.contains(&"Test prompt".to_string()));
        // Should include --print for headless mode
        assert!(args.contains(&"--print".to_string()));
        // Should include --force for tool execution
        assert!(args.contains(&"--force".to_string()));
    }

    #[test]
    fn default_settings_has_none_executable() {
        let settings = CursorSettings::default();
        assert!(settings.executable_path.is_none());
        assert!(settings.extra_flags.is_empty());
    }

    #[test]
    fn build_command_includes_headless_flags() {
        let config = create_test_config();
        let (_, args) = build_command(&config);
        // Should include flags for headless execution
        assert!(args.contains(&"--print".to_string()));
        assert!(args.contains(&"--force".to_string()));
        assert!(args.contains(&"--approve-mcps".to_string()));
    }

    #[test]
    fn build_command_includes_workspace_flag() {
        let config = create_test_config();
        let (_, args) = build_command(&config);
        // Should include --workspace flag with repo path so Cursor finds hooks.json
        assert!(args.contains(&"--workspace".to_string()));
        assert!(args.contains(&"/tmp/test".to_string()));
    }

    #[test]
    fn build_with_custom_executable() {
        let config = create_test_config();
        let settings = CursorSettings {
            executable_path: Some("/usr/local/bin/cursor".to_string()),
            ..Default::default()
        };
        let (cmd, _) = build_command_with_settings(&config, &settings);
        assert_eq!(cmd, "/usr/local/bin/cursor");
    }

    #[test]
    fn build_with_extra_flags() {
        let config = create_test_config();
        let settings = CursorSettings {
            extra_flags: vec!["--verbose".to_string(), "--no-cache".to_string()],
            ..Default::default()
        };
        let (_, args) = build_command_with_settings(&config, &settings);
        assert!(args.contains(&"--verbose".to_string()));
        assert!(args.contains(&"--no-cache".to_string()));
    }

    #[test]
    fn build_command_includes_output_format() {
        let config = create_test_config();
        let (_, args) = build_command(&config);
        assert!(args.contains(&"--output-format".to_string()));
        assert!(args.contains(&"text".to_string()));
    }

    #[test]
    fn build_command_includes_model_when_specified() {
        let mut config = create_test_config();
        config.model = Some("sonnet-4.5".to_string());
        let (_, args) = build_command(&config);
        assert!(args.contains(&"--model".to_string()));
        // Cursor maps normalized format to Claude format
        assert!(args.contains(&"claude-sonnet-4-5".to_string()));
    }

    #[test]
    fn build_command_maps_model_names_correctly() {
        let test_cases = [
            ("opus-4.6", "claude-opus-4-6"),
            ("opus-4.5", "claude-opus-4-5"),
            ("sonnet-4.5", "claude-sonnet-4-5"),
            ("sonnet-4", "claude-sonnet-4"),
            ("haiku-4.5", "claude-haiku-4-5"),
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
    fn build_command_omits_model_when_none() {
        let config = create_test_config();
        let (_, args) = build_command(&config);
        assert!(!args.contains(&"--model".to_string()));
    }

    #[test]
    fn build_command_uses_model_override_when_present() {
        let mut config = create_test_config();
        config.model = Some("sonnet-4".to_string());
        config.claude_api_config = Some(super::super::super::ClaudeApiConfig {
            model_override: Some("custom-model-override".to_string()),
            ..Default::default()
        });
        let (_, args) = build_command(&config);
        assert!(args.contains(&"--model".to_string()));
        // model_override should take priority over config.model
        assert!(
            args.contains(&"custom-model-override".to_string()),
            "model_override should be used instead of config.model"
        );
        assert!(
            !args.contains(&"claude-sonnet-4".to_string()),
            "config.model should not be used when model_override is set"
        );
    }

    #[test]
    fn build_command_ignores_empty_model_override() {
        let mut config = create_test_config();
        config.model = Some("sonnet-4".to_string());
        config.claude_api_config = Some(super::super::super::ClaudeApiConfig {
            model_override: Some("".to_string()),
            ..Default::default()
        });
        let (_, args) = build_command(&config);
        assert!(args.contains(&"--model".to_string()));
        // Empty model_override should fall back to config.model
        assert!(
            args.contains(&"claude-sonnet-4".to_string()),
            "Should fall back to config.model when model_override is empty"
        );
    }

    #[test]
    fn build_command_falls_back_to_config_model_without_override() {
        let mut config = create_test_config();
        config.model = Some("opus-4.5".to_string());
        config.claude_api_config = Some(super::super::super::ClaudeApiConfig {
            model_override: None,
            ..Default::default()
        });
        let (_, args) = build_command(&config);
        assert!(args.contains(&"--model".to_string()));
        assert!(
            args.contains(&"claude-opus-4-5".to_string()),
            "Should fall back to config.model when model_override is None"
        );
    }
}
