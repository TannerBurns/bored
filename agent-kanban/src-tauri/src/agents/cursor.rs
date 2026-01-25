use super::AgentRunConfig;

pub fn build_command(config: &AgentRunConfig) -> (String, Vec<String>) {
    let command = "cursor".to_string();
    let args = vec![
        "agent".to_string(),
        "-p".to_string(),
        config.prompt.clone(),
        "--output-format".to_string(),
        "text".to_string(),
    ];
    (command, args)
}

/// Cursor-specific settings
#[derive(Debug, Clone)]
pub struct CursorSettings {
    /// Path to cursor executable (if not in PATH)
    pub executable_path: Option<String>,

    /// Additional CLI flags
    pub extra_flags: Vec<String>,

    /// Whether to use yolo mode (no confirmations)
    pub yolo_mode: bool,
}

impl Default for CursorSettings {
    fn default() -> Self {
        Self {
            executable_path: None,
            extra_flags: vec![],
            yolo_mode: false,
        }
    }
}

/// Build command with custom settings
#[allow(dead_code)]
pub fn build_command_with_settings(
    config: &AgentRunConfig,
    settings: &CursorSettings,
) -> (String, Vec<String>) {
    let command = settings
        .executable_path
        .clone()
        .unwrap_or_else(|| "cursor".to_string());

    let mut args = vec!["agent".to_string(), "-p".to_string(), config.prompt.clone()];
    args.push("--output-format".to_string());
    args.push("text".to_string());

    if settings.yolo_mode {
        args.push("--yolo".to_string());
    }
    args.extend(settings.extra_flags.clone());

    (command, args)
}

/// Generate the hooks.json content for Cursor
#[allow(dead_code)]
pub fn generate_hooks_config(api_url: &str, hook_script_path: &str) -> serde_json::Value {
    serde_json::json!({
        "hooks": {
            "beforeShellExecution": {
                "command": hook_script_path,
                "args": ["beforeShellExecution"],
                "env": {
                    "AGENT_KANBAN_API_URL": api_url
                }
            },
            "afterFileEdit": {
                "command": hook_script_path,
                "args": ["afterFileEdit"],
                "env": {
                    "AGENT_KANBAN_API_URL": api_url
                }
            },
            "stop": {
                "command": hook_script_path,
                "args": ["stop"],
                "env": {
                    "AGENT_KANBAN_API_URL": api_url
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_config() -> AgentRunConfig {
        AgentRunConfig {
            kind: super::super::AgentKind::Cursor,
            ticket_id: "test-ticket".to_string(),
            run_id: "test-run".to_string(),
            repo_path: PathBuf::from("/tmp/test"),
            prompt: "Test prompt".to_string(),
            timeout_secs: Some(300),
            api_url: "http://localhost:7432".to_string(),
            api_token: "token".to_string(),
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
        assert!(args.contains(&"-p".to_string()));
        assert!(args.contains(&"Test prompt".to_string()));
    }

    #[test]
    fn default_settings_no_yolo() {
        let settings = CursorSettings::default();
        assert!(!settings.yolo_mode);
        assert!(settings.executable_path.is_none());
        assert!(settings.extra_flags.is_empty());
    }

    #[test]
    fn build_with_yolo_mode() {
        let config = create_test_config();
        let settings = CursorSettings {
            yolo_mode: true,
            ..Default::default()
        };
        let (_, args) = build_command_with_settings(&config, &settings);
        assert!(args.contains(&"--yolo".to_string()));
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
    fn generate_hooks_config_structure() {
        let config = generate_hooks_config("http://localhost:7432", "/path/to/hook.sh");
        assert!(config.get("hooks").is_some());
        let hooks = config.get("hooks").unwrap();
        assert!(hooks.get("beforeShellExecution").is_some());
        assert!(hooks.get("afterFileEdit").is_some());
        assert!(hooks.get("stop").is_some());
    }
}
