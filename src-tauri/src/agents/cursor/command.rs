//! Cursor CLI command building.

use super::super::AgentRunConfig;
use crate::agents::provider::AgentRunConfig as ProviderAgentRunConfig;

/// Default model used when none is explicitly specified
const DEFAULT_MODEL: &str = "opus-4.6";

/// Map normalized model name to Claude format for Cursor
/// e.g., "sonnet-4.5" -> "claude-sonnet-4-5"
fn map_model_for_cursor(model: &str) -> String {
    match model {
        "opus-4.6" => "claude-opus-4-6".to_string(),
        "opus-4.5" => "claude-opus-4-5".to_string(),
        "sonnet-4.5" => "claude-sonnet-4-5".to_string(),
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

    let model = config.model.as_deref().unwrap_or(DEFAULT_MODEL);
    args.push("--model".to_string());
    args.push(map_model_for_cursor(model));

    args.push(config.prompt.clone());
    (command, args)
}

/// Build command from the provider-based `AgentRunConfig`.
pub fn build_command_from_provider_config(config: &ProviderAgentRunConfig) -> (String, Vec<String>) {
    let command = "cursor".to_string();
    let mut args = vec![
        "agent".to_string(),
        "--print".to_string(),
        "--force".to_string(),
        "--approve-mcps".to_string(),
        "--output-format".to_string(),
        "text".to_string(),
        "--workspace".to_string(),
        config.repo_path.to_string_lossy().to_string(),
    ];

    let model = config.model.as_deref().unwrap_or(DEFAULT_MODEL);
    args.push("--model".to_string());
    args.push(map_model_for_cursor(model));

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
            agent_config: std::collections::HashMap::new(),
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

    // ── build_command_from_provider_config tests ────────────────────

    fn create_provider_config() -> ProviderAgentRunConfig {
        ProviderAgentRunConfig {
            agent_id: "cursor".to_string(),
            ticket_id: "t".to_string(),
            run_id: "r".to_string(),
            repo_path: PathBuf::from("/tmp/test"),
            prompt: "Test prompt".to_string(),
            timeout_secs: None,
            api_url: "http://localhost:7432".to_string(),
            api_token: "tok".to_string(),
            model: None,
            agent_config: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn provider_build_returns_cursor_command() {
        let (cmd, args) = build_command_from_provider_config(&create_provider_config());
        assert_eq!(cmd, "cursor");
        assert_eq!(args[0], "agent");
    }

    #[test]
    fn provider_build_includes_headless_flags() {
        let (_, args) = build_command_from_provider_config(&create_provider_config());
        assert!(args.contains(&"--print".to_string()));
        assert!(args.contains(&"--force".to_string()));
        assert!(args.contains(&"--approve-mcps".to_string()));
        assert!(args.contains(&"--output-format".to_string()));
        assert!(args.contains(&"text".to_string()));
    }

    #[test]
    fn provider_build_includes_workspace() {
        let (_, args) = build_command_from_provider_config(&create_provider_config());
        assert!(args.contains(&"--workspace".to_string()));
        assert!(args.contains(&"/tmp/test".to_string()));
    }

    #[test]
    fn provider_build_defaults_to_opus_model() {
        let (_, args) = build_command_from_provider_config(&create_provider_config());
        assert!(args.contains(&"claude-opus-4-6".to_string()));
    }

    #[test]
    fn provider_build_maps_model_name() {
        let mut config = create_provider_config();
        config.model = Some("sonnet-4.5".to_string());
        let (_, args) = build_command_from_provider_config(&config);
        assert!(args.contains(&"claude-sonnet-4-5".to_string()));
    }

    #[test]
    fn provider_build_prompt_is_last() {
        let (_, args) = build_command_from_provider_config(&create_provider_config());
        assert_eq!(args.last(), Some(&"Test prompt".to_string()));
    }
}
