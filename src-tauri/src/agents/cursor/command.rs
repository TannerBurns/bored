//! Cursor CLI command building.

use crate::agents::provider::AgentRunConfig;

pub fn build_command_from_provider_config(config: &AgentRunConfig) -> (String, Vec<String>) {
    let command = "cursor".to_string();

    let workspace_path = config
        .workspace_file
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| config.repo_path.to_string_lossy().to_string());

    let mut args = vec![
        "agent".to_string(),
        "--print".to_string(),
        "--force".to_string(),
        "--approve-mcps".to_string(),
        "--output-format".to_string(),
        "stream-json".to_string(),
        "--workspace".to_string(),
        workspace_path,
    ];

    if let Some(ref model) = config.model {
        args.push("--model".to_string());
        args.push(model.clone());
    }

    if let Some(ref sid) = config.session_id {
        args.push("--resume".to_string());
        args.push(sid.clone());
    }

    args.push(config.prompt.clone());
    (command, args)
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
            model: None,
            agent_config: std::collections::HashMap::new(),
            session_id: None,
            workspace_file: None,
            workspace_paths: vec![],
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
        assert!(args.contains(&"stream-json".to_string()));
    }

    #[test]
    fn build_command_includes_model_when_specified() {
        let mut config = create_test_config();
        config.model = Some("claude-sonnet-4-5".to_string());
        let (_, args) = build_command_from_provider_config(&config);
        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"claude-sonnet-4-5".to_string()));
    }

    #[test]
    fn build_command_omits_model_when_none() {
        let config = create_test_config();
        let (_, args) = build_command_from_provider_config(&config);
        assert!(!args.contains(&"--model".to_string()));
    }

    #[test]
    fn provider_build_prompt_is_last() {
        let config = create_test_config();
        let (_, args) = build_command_from_provider_config(&config);
        assert_eq!(args.last(), Some(&"Test prompt".to_string()));
    }

    #[test]
    fn build_command_includes_resume_when_session_set() {
        let mut config = create_test_config();
        config.session_id = Some("chat-abc".to_string());
        let (_, args) = build_command_from_provider_config(&config);
        let idx = args.iter().position(|a| a == "--resume").expect("--resume must be present");
        assert_eq!(args[idx + 1], "chat-abc");
        assert_eq!(args.last(), Some(&"Test prompt".to_string()));
    }

    #[test]
    fn build_command_omits_resume_when_no_session() {
        let config = create_test_config();
        let (_, args) = build_command_from_provider_config(&config);
        assert!(!args.contains(&"--resume".to_string()));
    }
}
