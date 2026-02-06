//! Claude CLI command building.

use super::super::AgentRunConfig;

/// Default model used when none is explicitly specified
const DEFAULT_MODEL: &str = "opus-4.6";

/// Map normalized model name to Claude Code format
/// e.g., "opus-4.5" -> "claude-opus-4-5"
fn map_model_for_claude(model: &str) -> String {
    match model {
        "opus-4.6" => "claude-opus-4-6".to_string(),
        "opus-4.5" => "claude-opus-4-5".to_string(),
        "sonnet-4.5" => "claude-sonnet-4-5".to_string(),
        "haiku-4.5" => "claude-haiku-4-5".to_string(),
        other => other.to_string(),
    }
}

pub fn build_command(config: &AgentRunConfig) -> (String, Vec<String>) {
    let command = "claude".to_string();
    let mut args = vec![
        "--output-format".to_string(),
        "stream-json".to_string(),
        "--verbose".to_string(),
        "--dangerously-skip-permissions".to_string(),
    ];

    let model_to_use = config
        .claude_api_config
        .as_ref()
        .and_then(|c| c.model_override.as_ref())
        .filter(|s| !s.is_empty())
        .cloned();

    if let Some(model) = model_to_use {
        args.push("--model".to_string());
        args.push(model);
    } else {
        let model = config
            .model
            .as_deref()
            .unwrap_or(DEFAULT_MODEL);
        args.push("--model".to_string());
        args.push(map_model_for_claude(model));
    }

    args.push("--settings".to_string());
    args.push(r#"{"alwaysThinkingEnabled": true}"#.to_string());

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

    args.push("--settings".to_string());
    args.push(r#"{"alwaysThinkingEnabled": true}"#.to_string());

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
        config.model = Some("opus-4.5".to_string());
        let (_, args) = build_command(&config);
        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"claude-opus-4-5".to_string()));
        assert_eq!(args.last(), Some(&"Test prompt".to_string()));
    }

    #[test]
    fn build_command_maps_model_names_correctly() {
        let test_cases = [
            ("opus-4.6", "claude-opus-4-6"),
            ("opus-4.5", "claude-opus-4-5"),
            ("sonnet-4.5", "claude-sonnet-4-5"),
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
}
