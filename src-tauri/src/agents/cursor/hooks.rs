//! Cursor hooks configuration and installation.

use std::path::Path;

use super::settings::global_hooks_path;

/// Configuration for generating hooks.json
#[derive(Debug, Clone, Default)]
pub struct HooksConfig<'a> {
    pub hook_script_path: &'a str,
    pub api_url: Option<&'a str>,
    pub api_token: Option<&'a str>,
    pub run_id: Option<&'a str>,
}

pub fn generate_hooks_json(hook_script_path: &str) -> serde_json::Value {
    generate_hooks_json_with_api(hook_script_path, None, None, None)
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
        let shell_command = format!(
            "export AGENT_KANBAN_API_URL=\"{}\"; node \"{}\" {}",
            api_url, escaped_script, event
        );
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

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::settings::check_project_hooks_installed;

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
        let shell_hook_array = hooks
            .get("beforeShellExecution")
            .unwrap()
            .as_array()
            .unwrap();
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
    fn check_project_hooks_installed_returns_true_when_present() {
        let temp_dir = std::env::temp_dir().join(format!("cursor_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        install_hooks(&temp_dir, "/path/to/hook.js", None, None).unwrap();
        assert!(check_project_hooks_installed(&temp_dir));

        // Cleanup
        std::fs::remove_dir_all(&temp_dir).ok();
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
    fn generate_hooks_json_with_api_includes_env_in_command() {
        let config = generate_hooks_json_with_api(
            "/path/to/hook.js",
            Some("http://localhost:7432"),
            None,
            None,
        );
        let hooks = config.get("hooks").unwrap();
        let shell_hook_array = hooks
            .get("beforeShellExecution")
            .unwrap()
            .as_array()
            .unwrap();
        let command = shell_hook_array[0]
            .get("command")
            .unwrap()
            .as_str()
            .unwrap();

        // Env vars should be embedded in the shell command with export and double quotes
        assert!(command.contains("export AGENT_KANBAN_API_URL=\"http://localhost:7432\""));
        // Command should be wrapped in /bin/sh -c
        assert!(command.starts_with("/bin/sh -c '"));
    }

    #[test]
    fn generate_hooks_json_with_api_none_has_no_env_in_command() {
        let config = generate_hooks_json_with_api("/path/to/hook.js", None, None, None);
        let hooks = config.get("hooks").unwrap();
        let shell_hook_array = hooks
            .get("beforeShellExecution")
            .unwrap()
            .as_array()
            .unwrap();
        let command = shell_hook_array[0]
            .get("command")
            .unwrap()
            .as_str()
            .unwrap();

        // Should not contain env var prefix
        assert!(!command.contains("AGENT_KANBAN_API_URL="));
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
        let shell_hook_array = hooks
            .get("beforeShellExecution")
            .unwrap()
            .as_array()
            .unwrap();
        let command = shell_hook_array[0]
            .get("command")
            .unwrap()
            .as_str()
            .unwrap();

        assert!(command.contains("export AGENT_KANBAN_RUN_ID=\"run-12345\""));
        assert!(command.contains("export AGENT_KANBAN_API_URL=\"http://localhost:7432\""));
        // Token is NOT set in command - script reads from file at runtime
        assert!(!command.contains("AGENT_KANBAN_API_TOKEN"));
    }
}
