use super::AgentRunConfig;
use std::path::PathBuf;
use std::process::Command;

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

#[derive(Debug, Clone, Default)]
pub struct CursorSettings {
    pub executable_path: Option<String>,
    pub extra_flags: Vec<String>,
    pub yolo_mode: bool,
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

    let mut args = vec!["agent".to_string(), "-p".to_string(), config.prompt.clone()];
    args.push("--output-format".to_string());
    args.push("text".to_string());

    if settings.yolo_mode {
        args.push("--yolo".to_string());
    }
    args.extend(settings.extra_flags.clone());

    (command, args)
}

pub fn generate_hooks_json(hook_script_path: &str) -> serde_json::Value {
    serde_json::json!({
        "hooks": {
            "beforeShellExecution": {
                "command": hook_script_path,
                "args": ["beforeShellExecution"]
            },
            "beforeReadFile": {
                "command": hook_script_path,
                "args": ["beforeReadFile"]
            },
            "beforeMCPExecution": {
                "command": hook_script_path,
                "args": ["beforeMCPExecution"]
            },
            "afterFileEdit": {
                "command": hook_script_path,
                "args": ["afterFileEdit"]
            },
            "stop": {
                "command": hook_script_path,
                "args": ["stop"]
            }
        }
    })
}

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

pub fn install_hooks(repo_path: &PathBuf, hook_script_path: &str) -> std::io::Result<()> {
    let cursor_dir = repo_path.join(".cursor");
    std::fs::create_dir_all(&cursor_dir)?;

    let hooks_json = generate_hooks_json(hook_script_path);
    let hooks_path = cursor_dir.join("hooks.json");
    
    std::fs::write(
        hooks_path,
        serde_json::to_string_pretty(&hooks_json).unwrap(),
    )?;

    Ok(())
}

pub fn global_hooks_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".cursor").join("hooks.json"))
}

pub fn install_global_hooks(hook_script_path: &str) -> std::io::Result<()> {
    if let Some(hooks_path) = global_hooks_path() {
        if let Some(parent) = hooks_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let hooks_json = generate_hooks_json(hook_script_path);
        std::fs::write(
            hooks_path,
            serde_json::to_string_pretty(&hooks_json).unwrap(),
        )?;
    }
    Ok(())
}

pub fn is_cursor_available() -> bool {
    Command::new("cursor")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn get_cursor_version() -> Option<String> {
    Command::new("cursor")
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
}

pub fn check_global_hooks_installed() -> bool {
    global_hooks_path()
        .map(|p| p.exists())
        .unwrap_or(false)
}

pub fn check_project_hooks_installed(repo_path: &PathBuf) -> bool {
    repo_path.join(".cursor").join("hooks.json").exists()
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
    fn generate_hooks_json_has_all_hooks() {
        let config = generate_hooks_json("/path/to/hook.js");
        let hooks = config.get("hooks").unwrap();
        assert!(hooks.get("beforeShellExecution").is_some());
        assert!(hooks.get("beforeReadFile").is_some());
        assert!(hooks.get("beforeMCPExecution").is_some());
        assert!(hooks.get("afterFileEdit").is_some());
        assert!(hooks.get("stop").is_some());
    }

    #[test]
    fn generate_hooks_json_uses_correct_script_path() {
        let script_path = "/custom/path/hook.js";
        let config = generate_hooks_json(script_path);
        let hooks = config.get("hooks").unwrap();
        let shell_hook = hooks.get("beforeShellExecution").unwrap();
        assert_eq!(shell_hook.get("command").unwrap().as_str().unwrap(), script_path);
    }

    #[test]
    fn install_hooks_creates_directory_and_file() {
        let temp_dir = std::env::temp_dir().join(format!("cursor_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        let result = install_hooks(&temp_dir, "/path/to/hook.js");
        assert!(result.is_ok());
        
        let hooks_path = temp_dir.join(".cursor").join("hooks.json");
        assert!(hooks_path.exists());
        
        // Cleanup
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn check_project_hooks_installed_returns_false_when_missing() {
        let temp_dir = std::env::temp_dir().join(format!("cursor_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        assert!(!check_project_hooks_installed(&temp_dir));
        
        // Cleanup
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn check_project_hooks_installed_returns_true_when_present() {
        let temp_dir = std::env::temp_dir().join(format!("cursor_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        install_hooks(&temp_dir, "/path/to/hook.js").unwrap();
        assert!(check_project_hooks_installed(&temp_dir));
        
        // Cleanup
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn global_hooks_path_returns_some() {
        // Should return Some on systems with a home directory
        let path = global_hooks_path();
        // This test might fail in unusual environments without home dir
        if dirs::home_dir().is_some() {
            assert!(path.is_some());
            assert!(path.unwrap().to_string_lossy().contains(".cursor"));
        }
    }
}
