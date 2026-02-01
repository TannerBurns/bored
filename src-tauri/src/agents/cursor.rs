use super::AgentRunConfig;
use std::path::{Path, PathBuf};
use std::process::Command;

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

    if let Some(ref model) = config.model {
        args.push("--model".to_string());
        args.push(model.clone());
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

pub fn generate_hooks_json(hook_script_path: &str) -> serde_json::Value {
    generate_hooks_json_with_api(hook_script_path, None, None, None)
}

/// Configuration for generating hooks.json
#[derive(Debug, Clone, Default)]
pub struct HooksConfig<'a> {
    pub hook_script_path: &'a str,
    pub api_url: Option<&'a str>,
    pub api_token: Option<&'a str>,
    pub run_id: Option<&'a str>,
}

pub fn generate_hooks_json_with_api(
    hook_script_path: &str,
    api_url: Option<&str>,
    api_token: Option<&str>,
    run_id: Option<&str>,
) -> serde_json::Value {
    generate_hooks_json_with_config(HooksConfig {
        hook_script_path,
        api_url,
        api_token,
        run_id,
    })
}

pub fn generate_hooks_json_with_config(config: HooksConfig) -> serde_json::Value {
    // Build environment variable exports for shell command
    // NOTE: We do NOT export AGENT_KANBAN_API_TOKEN here - the hook script reads it
    // from a persisted file at runtime. This avoids issues with Cursor caching
    // stale tokens in hooks.json.
    let mut env_exports = String::new();
    
    if let Some(url) = config.api_url {
        env_exports.push_str(&format!("export AGENT_KANBAN_API_URL=\"{}\"; ", url));
    }
    // API token is intentionally NOT set here - script reads from file
    // This ensures hooks work even when Cursor caches old hooks.json
    if let Some(run_id) = config.run_id {
        env_exports.push_str(&format!("export AGENT_KANBAN_RUN_ID=\"{}\"; ", run_id));
    }
    
    // Create hook command wrapped in sh -c to ensure environment variables are set
    // Cursor executes commands directly, so we need an explicit shell
    // Use double quotes for the script path inside (handles spaces)
    let make_hook = |event: &str| {
        // Escape any double quotes in the script path
        let escaped_script = config.hook_script_path.replace("\"", "\\\"");
        let shell_command = format!("{}node \"{}\" {}", env_exports, escaped_script, event);
        // Wrap in sh -c with the command in single quotes (shell_command uses double quotes internally)
        let command = format!("/bin/sh -c '{}'", shell_command);
        // Each hook is an array of command objects (Cursor 1.7+ format)
        serde_json::json!([{
            "command": command
        }])
    };

    // Cursor hooks.json v1 format
    serde_json::json!({
        "version": 1,
        "hooks": {
            "beforeShellExecution": make_hook("beforeShellExecution"),
            "beforeReadFile": make_hook("beforeReadFile"),
            "beforeMCPExecution": make_hook("beforeMCPExecution"),
            "afterFileEdit": make_hook("afterFileEdit"),
            "stop": make_hook("stop")
        }
    })
}

#[allow(dead_code)]
pub fn generate_hooks_config(api_url: &str, hook_script_path: &str) -> serde_json::Value {
    // Updated to use Cursor 1.7+ hooks.json v1 format
    // Wrap in sh -c to ensure shell interpretation of environment variables
    // Use double quotes inside to avoid quoting issues
    let escaped_script = hook_script_path.replace("\"", "\\\"");
    let make_hook = |event: &str| {
        let shell_command = format!("export AGENT_KANBAN_API_URL=\"{}\"; node \"{}\" {}", api_url, escaped_script, event);
        let command = format!("/bin/sh -c '{}'", shell_command);
        serde_json::json!([{ "command": command }])
    };
    
    serde_json::json!({
        "version": 1,
        "hooks": {
            "beforeShellExecution": make_hook("beforeShellExecution"),
            "afterFileEdit": make_hook("afterFileEdit"),
            "stop": make_hook("stop")
        }
    })
}

pub fn install_hooks(
    repo_path: &Path,
    hook_script_path: &str,
    api_url: Option<&str>,
    api_token: Option<&str>,
) -> std::io::Result<()> {
    install_hooks_with_run_id(repo_path, hook_script_path, api_url, api_token, None)
}

pub fn install_hooks_with_run_id(
    repo_path: &Path,
    hook_script_path: &str,
    api_url: Option<&str>,
    api_token: Option<&str>,
    run_id: Option<&str>,
) -> std::io::Result<()> {
    let cursor_dir = repo_path.join(".cursor");
    std::fs::create_dir_all(&cursor_dir)?;

    let hooks_json = generate_hooks_json_with_api(hook_script_path, api_url, api_token, run_id);
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

pub fn install_global_hooks(
    hook_script_path: &str,
    api_url: Option<&str>,
    api_token: Option<&str>,
) -> std::io::Result<()> {
    install_global_hooks_with_run_id(hook_script_path, api_url, api_token, None)
}

pub fn install_global_hooks_with_run_id(
    hook_script_path: &str,
    api_url: Option<&str>,
    api_token: Option<&str>,
    run_id: Option<&str>,
) -> std::io::Result<()> {
    let hooks_path = global_hooks_path().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not determine home directory for global hooks installation",
        )
    })?;

    if let Some(parent) = hooks_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let hooks_json = generate_hooks_json_with_api(hook_script_path, api_url, api_token, run_id);
    std::fs::write(
        hooks_path,
        serde_json::to_string_pretty(&hooks_json).unwrap(),
    )?;

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

pub fn check_project_hooks_installed(repo_path: &Path) -> bool {
    repo_path.join(".cursor").join("hooks.json").exists()
}

pub const COMMAND_TEMPLATES: &[&str] = &[
    "add-and-commit.md",
    "cleanup.md",
    "deslop.md",
    "review-changes.md",
    "unit-tests.md",
];

pub fn check_project_commands_installed(repo_path: &Path) -> bool {
    let commands_dir = repo_path.join(".cursor").join("commands");
    if !commands_dir.exists() {
        return false;
    }
    
    COMMAND_TEMPLATES.iter().all(|name| commands_dir.join(name).exists())
}

/// Get the user-level commands directory (~/.cursor/commands/)
pub fn user_commands_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".cursor").join("commands"))
}

/// Check if commands are installed at the user level (~/.cursor/commands/)
pub fn check_user_commands_installed() -> bool {
    user_commands_path()
        .map(|p| {
            if !p.exists() {
                return false;
            }
            COMMAND_TEMPLATES.iter().all(|name| p.join(name).exists())
        })
        .unwrap_or(false)
}

/// Get the bundled commands path, checking development path first.
/// This version doesn't have access to Tauri's resource resolver.
pub fn get_bundled_commands_path() -> Option<PathBuf> {
    // Check development path (only works in dev builds)
    let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts").join("commands");
    if dev_path.exists() {
        return Some(dev_path);
    }
    None
}

/// Get the bundled commands path with Tauri resource resolver fallback.
/// In production builds, uses Tauri's resource API to locate bundled commands.
pub fn get_bundled_commands_path_with_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Option<PathBuf> {
    // First, check development path
    if let Some(path) = get_bundled_commands_path() {
        return Some(path);
    }
    
    // In production, resolve via Tauri's resource API
    // The commands are bundled under scripts/commands/
    app.path_resolver()
        .resolve_resource("scripts/commands")
        .filter(|p| p.exists())
}

pub fn install_commands(
    repo_path: &Path,
    commands_source: &Path,
) -> std::io::Result<Vec<String>> {
    let commands_dir = repo_path.join(".cursor").join("commands");
    std::fs::create_dir_all(&commands_dir)?;
    
    let mut installed = Vec::new();
    
    for name in COMMAND_TEMPLATES {
        let source = commands_source.join(name);
        let dest = commands_dir.join(name);
        
        if source.exists() {
            std::fs::copy(&source, &dest)?;
            installed.push(name.to_string());
        }
    }
    
    Ok(installed)
}

/// Install command templates to the user-level directory (~/.cursor/commands/)
pub fn install_user_commands(commands_source: &Path) -> std::io::Result<Vec<String>> {
    let commands_dir = user_commands_path().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not determine home directory",
        )
    })?;
    
    std::fs::create_dir_all(&commands_dir)?;
    
    let mut installed = Vec::new();
    
    for name in COMMAND_TEMPLATES {
        let source = commands_source.join(name);
        let dest = commands_dir.join(name);
        
        if source.exists() {
            std::fs::copy(&source, &dest)?;
            installed.push(name.to_string());
        }
    }
    
    Ok(installed)
}

pub fn get_available_commands(commands_source: &Path) -> Vec<String> {
    COMMAND_TEMPLATES
        .iter()
        .filter(|name| commands_source.join(name).exists())
        .map(|s| s.to_string())
        .collect()
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
    fn generate_hooks_config_structure() {
        let config = generate_hooks_config("http://localhost:7432", "/path/to/hook.sh");
        // Should have version 1
        assert_eq!(config.get("version").unwrap().as_i64().unwrap(), 1);
        assert!(config.get("hooks").is_some());
        let hooks = config.get("hooks").unwrap();
        // Each hook should be an array
        assert!(hooks.get("beforeShellExecution").unwrap().is_array());
        assert!(hooks.get("afterFileEdit").unwrap().is_array());
        assert!(hooks.get("stop").unwrap().is_array());
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
        // Cursor uses the normalized format directly
        assert!(args.contains(&"sonnet-4.5".to_string()));
    }

    #[test]
    fn build_command_omits_model_when_none() {
        let config = create_test_config();
        let (_, args) = build_command(&config);
        assert!(!args.contains(&"--model".to_string()));
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
        // Each hook is an array of command objects
        let shell_hook_array = hooks.get("beforeShellExecution").unwrap().as_array().unwrap();
        let shell_hook = &shell_hook_array[0];
        let command = shell_hook.get("command").unwrap().as_str().unwrap();
        // Command should contain the script path
        assert!(command.contains(script_path));
    }

    #[test]
    fn install_hooks_creates_directory_and_file() {
        let temp_dir = std::env::temp_dir().join(format!("cursor_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        let result = install_hooks(&temp_dir, "/path/to/hook.js", None, None);
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
        
        install_hooks(&temp_dir, "/path/to/hook.js", None, None).unwrap();
        assert!(check_project_hooks_installed(&temp_dir));
        
        // Cleanup
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn global_hooks_path_returns_some() {
        let path = global_hooks_path();
        if dirs::home_dir().is_some() {
            assert!(path.is_some());
            assert!(path.unwrap().to_string_lossy().contains(".cursor"));
        }
    }

    #[test]
    fn generate_hooks_json_each_hook_has_correct_event_in_command() {
        let config = generate_hooks_json("/path/to/hook.js");
        let hooks = config.get("hooks").unwrap();
        
        let expected_events = vec![
            "beforeShellExecution",
            "beforeReadFile",
            "beforeMCPExecution",
            "afterFileEdit",
            "stop",
        ];

        for event in expected_events {
            let hook_array = hooks.get(event).unwrap().as_array().unwrap();
            assert_eq!(hook_array.len(), 1, "Hook {} should have exactly one command", event);
            let command = hook_array[0].get("command").unwrap().as_str().unwrap();
            // Command is wrapped in sh -c, so event name is at end of inner command before closing quote
            let expected_ending = format!("{}'", event);
            assert!(command.ends_with(&expected_ending), "Command should end with event name before closing quote: {} (got: {})", event, command);
        }
    }

    #[test]
    fn install_hooks_writes_valid_json() {
        let temp_dir = std::env::temp_dir().join(format!("cursor_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        install_hooks(&temp_dir, "/path/to/hook.js", None, None).unwrap();
        
        let hooks_path = temp_dir.join(".cursor").join("hooks.json");
        let content = std::fs::read_to_string(&hooks_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        
        assert!(parsed.get("hooks").is_some());
        assert!(parsed["hooks"].get("beforeShellExecution").is_some());
        
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn install_hooks_creates_nested_cursor_directory() {
        let temp_dir = std::env::temp_dir().join(format!("cursor_test_{}", uuid::Uuid::new_v4()));
        // Don't create temp_dir - install_hooks should handle missing .cursor dir
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        let cursor_dir = temp_dir.join(".cursor");
        assert!(!cursor_dir.exists());
        
        install_hooks(&temp_dir, "/path/to/hook.js", None, None).unwrap();
        
        assert!(cursor_dir.exists());
        assert!(cursor_dir.is_dir());
        
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn command_templates_list_has_all_commands() {
        assert_eq!(COMMAND_TEMPLATES.len(), 5);
        assert!(COMMAND_TEMPLATES.contains(&"add-and-commit.md"));
        assert!(COMMAND_TEMPLATES.contains(&"cleanup.md"));
        assert!(COMMAND_TEMPLATES.contains(&"deslop.md"));
        assert!(COMMAND_TEMPLATES.contains(&"review-changes.md"));
        assert!(COMMAND_TEMPLATES.contains(&"unit-tests.md"));
    }

    #[test]
    fn check_project_commands_installed_returns_false_when_missing() {
        let temp_dir = std::env::temp_dir().join(format!("cursor_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        assert!(!check_project_commands_installed(&temp_dir));
        
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn install_commands_creates_directory_and_files() {
        let temp_dir = std::env::temp_dir().join(format!("cursor_test_{}", uuid::Uuid::new_v4()));
        let source_dir = temp_dir.join("source");
        std::fs::create_dir_all(&source_dir).unwrap();
        
        // Create source command files
        for name in COMMAND_TEMPLATES {
            std::fs::write(source_dir.join(name), format!("# {}", name)).unwrap();
        }
        
        let project_dir = temp_dir.join("project");
        std::fs::create_dir_all(&project_dir).unwrap();
        
        let installed = install_commands(&project_dir, &source_dir).unwrap();
        assert_eq!(installed.len(), 5);
        
        // Verify files exist
        let commands_dir = project_dir.join(".cursor").join("commands");
        for name in COMMAND_TEMPLATES {
            assert!(commands_dir.join(name).exists());
        }
        
        assert!(check_project_commands_installed(&project_dir));
        
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn get_available_commands_returns_existing_files() {
        let temp_dir = std::env::temp_dir().join(format!("cursor_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        // Create only some command files
        std::fs::write(temp_dir.join("cleanup.md"), "# cleanup").unwrap();
        std::fs::write(temp_dir.join("deslop.md"), "# deslop").unwrap();
        
        let available = get_available_commands(&temp_dir);
        assert_eq!(available.len(), 2);
        assert!(available.contains(&"cleanup.md".to_string()));
        assert!(available.contains(&"deslop.md".to_string()));
        
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn check_global_hooks_installed_returns_false_when_no_file() {
        // This test checks behavior - global hooks path exists but file doesn't
        // We can't easily test this without mocking, but we verify the function runs
        let result = check_global_hooks_installed();
        // Result depends on actual system state, just verify it doesn't panic
        let _ = result;
    }

    #[test]
    fn is_cursor_available_returns_bool() {
        let _result = is_cursor_available();
    }

    #[test]
    fn get_cursor_version_returns_option() {
        // Verify function runs without panic
        let result = get_cursor_version();
        // If cursor is available, version should be non-empty
        if let Some(version) = result {
            assert!(!version.is_empty());
        }
    }

    #[test]
    fn generate_hooks_json_with_api_includes_env_in_command() {
        let config = generate_hooks_json_with_api("/path/to/hook.js", Some("http://localhost:7432"), None, None);
        let hooks = config.get("hooks").unwrap();
        let shell_hook_array = hooks.get("beforeShellExecution").unwrap().as_array().unwrap();
        let command = shell_hook_array[0].get("command").unwrap().as_str().unwrap();
        
        // Env vars should be embedded in the shell command with export and double quotes
        assert!(command.contains("export AGENT_KANBAN_API_URL=\"http://localhost:7432\""));
        // Command should be wrapped in /bin/sh -c
        assert!(command.starts_with("/bin/sh -c '"));
    }

    #[test]
    fn generate_hooks_json_with_api_none_has_no_env_in_command() {
        let config = generate_hooks_json_with_api("/path/to/hook.js", None, None, None);
        let hooks = config.get("hooks").unwrap();
        let shell_hook_array = hooks.get("beforeShellExecution").unwrap().as_array().unwrap();
        let command = shell_hook_array[0].get("command").unwrap().as_str().unwrap();
        
        // Should not contain env var prefix
        assert!(!command.contains("AGENT_KANBAN_API_URL="));
    }

    #[test]
    fn install_hooks_with_api_url_includes_env_in_command() {
        let temp_dir = std::env::temp_dir().join(format!("cursor_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        install_hooks(&temp_dir, "/path/to/hook.js", Some("http://localhost:7432"), None).unwrap();
        
        let hooks_path = temp_dir.join(".cursor").join("hooks.json");
        let content = std::fs::read_to_string(&hooks_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        
        let hook_array = parsed["hooks"]["beforeShellExecution"].as_array().unwrap();
        let command = hook_array[0]["command"].as_str().unwrap();
        assert!(command.contains("export AGENT_KANBAN_API_URL=\"http://localhost:7432\""));
        
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn generate_hooks_json_with_api_and_token_includes_both_in_command() {
        let config = generate_hooks_json_with_api(
            "/path/to/hook.js",
            Some("http://localhost:7432"),
            Some("test-token-123"),
            None,
        );
        let hooks = config.get("hooks").unwrap();
        let shell_hook_array = hooks.get("beforeShellExecution").unwrap().as_array().unwrap();
        let command = shell_hook_array[0].get("command").unwrap().as_str().unwrap();
        
        // API URL should be set, but API token is no longer embedded (read from file at runtime)
        assert!(command.contains("export AGENT_KANBAN_API_URL=\"http://localhost:7432\""));
        // Token is intentionally NOT set in the command - script reads it from file
        assert!(!command.contains("AGENT_KANBAN_API_TOKEN"));
    }

    #[test]
    fn generate_hooks_json_with_token_only_does_not_embed_token() {
        let config = generate_hooks_json_with_api("/path/to/hook.js", None, Some("test-token-456"), None);
        let hooks = config.get("hooks").unwrap();
        let shell_hook_array = hooks.get("beforeShellExecution").unwrap().as_array().unwrap();
        let command = shell_hook_array[0].get("command").unwrap().as_str().unwrap();
        
        // Neither should be set - token is read from file at runtime
        assert!(!command.contains("AGENT_KANBAN_API_URL="));
        assert!(!command.contains("AGENT_KANBAN_API_TOKEN"));
    }

    #[test]
    fn generate_hooks_json_with_run_id_includes_run_id_in_command() {
        let config = generate_hooks_json_with_api(
            "/path/to/hook.js",
            Some("http://localhost:7432"),
            Some("test-token"),
            Some("run-12345"),
        );
        let hooks = config.get("hooks").unwrap();
        let shell_hook_array = hooks.get("beforeShellExecution").unwrap().as_array().unwrap();
        let command = shell_hook_array[0].get("command").unwrap().as_str().unwrap();
        
        assert!(command.contains("export AGENT_KANBAN_RUN_ID=\"run-12345\""));
        assert!(command.contains("export AGENT_KANBAN_API_URL=\"http://localhost:7432\""));
        // Token is NOT set in command - script reads from file at runtime
        assert!(!command.contains("AGENT_KANBAN_API_TOKEN"));
    }

    #[test]
    fn install_hooks_with_api_url_and_token_includes_url_not_token() {
        let temp_dir = std::env::temp_dir().join(format!("cursor_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        install_hooks(
            &temp_dir,
            "/path/to/hook.js",
            Some("http://localhost:7432"),
            Some("my-secret-token"),
        )
        .unwrap();
        
        let hooks_path = temp_dir.join(".cursor").join("hooks.json");
        let content = std::fs::read_to_string(&hooks_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        
        let hook_array = parsed["hooks"]["beforeShellExecution"].as_array().unwrap();
        let command = hook_array[0]["command"].as_str().unwrap();
        // URL is set, but token is NOT (script reads from file at runtime)
        assert!(command.contains("export AGENT_KANBAN_API_URL=\"http://localhost:7432\""));
        assert!(!command.contains("AGENT_KANBAN_API_TOKEN"));
        
        std::fs::remove_dir_all(&temp_dir).ok();
    }
}
