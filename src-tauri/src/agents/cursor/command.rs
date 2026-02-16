//! Cursor CLI command building.

use crate::agents::provider::AgentRunConfig;

/// Default model used when none is explicitly specified
const DEFAULT_MODEL: &str = "opus-4.6";

/// Map normalized model name to Claude format for Cursor
fn map_model_for_cursor(model: &str) -> String {
    match model {
        "opus-4.6" => "claude-opus-4-6".to_string(),
        "opus-4.5" => "claude-opus-4-5".to_string(),
        "sonnet-4.5" => "claude-sonnet-4-5".to_string(),
        other => other.to_string(),
    }
}

/// Build command from the provider-based `AgentRunConfig`.
pub fn build_command_from_provider_config(config: &AgentRunConfig) -> (String, Vec<String>) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_config() -> AgentRunConfig {
        AgentRunConfig {
            agent_id: "cursor".to_string(),
            ticket_id: "test-ticket".to_string(),
            run_id: "test-run".to_string(),
            repo_path: PathBuf::from("/tmp/test"),
            prompt: "Test prompt".to_string(),
            timeout_secs: Some(300),
            api_url: "http://localhost:7432".to_string(),
            api_token: "token".to_string(),
            model: None,
            agent_config: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn build_command_returns_cursor() {
        let config = create_test_config();
        let (cmd, _) = build_command_from_provider_config(&config);
        assert_eq!(cmd, "cursor");
    }

    #[test]
    fn build_command_includes_agent_flag() {
        let config = create_test_config();
        let (_, args) = build_command_from_provider_config(&config);
        assert_eq!(args[0], "agent");
    }

    #[test]
    fn build_command_includes_prompt() {
        let config = create_test_config();
        let (_, args) = build_command_from_provider_config(&config);
        assert!(args.contains(&"Test prompt".to_string()));
        assert!(args.contains(&"--print".to_string()));
        assert!(args.contains(&"--force".to_string()));
    }

    #[test]
    fn build_command_includes_headless_flags() {
        let config = create_test_config();
        let (_, args) = build_command_from_provider_config(&config);
        assert!(args.contains(&"--print".to_string()));
        assert!(args.contains(&"--force".to_string()));
        assert!(args.contains(&"--approve-mcps".to_string()));
    }

    #[test]
    fn build_command_includes_workspace_flag() {
        let config = create_test_config();
        let (_, args) = build_command_from_provider_config(&config);
        assert!(args.contains(&"--workspace".to_string()));
        assert!(args.contains(&"/tmp/test".to_string()));
    }

    #[test]
    fn build_command_includes_output_format() {
        let config = create_test_config();
        let (_, args) = build_command_from_provider_config(&config);
        assert!(args.contains(&"--output-format".to_string()));
        assert!(args.contains(&"text".to_string()));
    }

    #[test]
    fn build_command_includes_model_when_specified() {
        let mut config = create_test_config();
        config.model = Some("sonnet-4.5".to_string());
        let (_, args) = build_command_from_provider_config(&config);
        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"claude-sonnet-4-5".to_string()));
    }

    #[test]
    fn build_command_defaults_to_opus_4_6_when_none() {
        let config = create_test_config();
        let (_, args) = build_command_from_provider_config(&config);
        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"claude-opus-4-6".to_string()));
    }

    #[test]
    fn provider_build_prompt_is_last() {
        let config = create_test_config();
        let (_, args) = build_command_from_provider_config(&config);
        assert_eq!(args.last(), Some(&"Test prompt".to_string()));
    }
}
