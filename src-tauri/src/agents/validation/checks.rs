//! Individual validation check functions.

use std::path::Path;

use super::types::ValidationCheck;
use crate::agents::provider::AgentProvider;

pub fn check_cli_available(provider: &dyn AgentProvider) -> ValidationCheck {
    let name = provider.display_name();
    if provider.is_available() {
        ValidationCheck::pass("cli_available", &format!("{} CLI is available", name))
    } else {
        ValidationCheck::fail(
            "cli_available",
            &format!("{} CLI is not installed or not in PATH", name),
            None,
        )
    }
}

pub fn check_hooks_configured(provider: &dyn AgentProvider, repo_path: &Path) -> ValidationCheck {
    let global_installed = provider.check_hooks_installed_global();
    let project_installed = provider.check_hooks_installed_project(repo_path);

    if global_installed || project_installed {
        let location = if project_installed {
            "project"
        } else {
            "global"
        };
        ValidationCheck::pass(
            "hooks_configured",
            &format!("Hooks are configured ({})", location),
        )
    } else {
        ValidationCheck::fail(
            "hooks_configured",
            "Hooks are not configured",
            Some("install_hooks"),
        )
    }
}

pub fn check_commands_installed(provider: &dyn AgentProvider, repo_path: &Path) -> ValidationCheck {
    let user_installed = provider.check_commands_installed_user();
    let project_installed = provider.check_commands_installed_project(repo_path);

    if user_installed || project_installed {
        let location = if user_installed { "user" } else { "project" };
        ValidationCheck::pass(
            "commands_installed",
            &format!("Command templates are installed ({})", location),
        )
    } else {
        ValidationCheck::fail(
            "commands_installed",
            "Command templates are not installed",
            Some("install_commands"),
        )
    }
}

pub fn check_git_repository(repo_path: &Path) -> ValidationCheck {
    let git_dir = repo_path.join(".git");

    if git_dir.exists() && git_dir.is_dir() {
        ValidationCheck::pass("git_repository", "Valid git repository")
    } else {
        ValidationCheck::fail("git_repository", "Not a git repository", None)
    }
}

pub fn check_api_url_configured(api_url: &str) -> ValidationCheck {
    if api_url.starts_with("http://") || api_url.starts_with("https://") {
        ValidationCheck::pass(
            "api_url_configured",
            &format!("API URL configured: {}", api_url),
        )
    } else {
        ValidationCheck::fail(
            "api_url_configured",
            &format!(
                "Invalid API URL format (must start with http:// or https://): {}",
                api_url
            ),
            None,
        )
    }
}

pub fn check_git_clean_state(repo_path: &Path) -> ValidationCheck {
    use std::process::Command;

    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_path)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.trim().is_empty() {
                ValidationCheck::pass("git_clean_state", "Git working tree is clean")
            } else {
                ValidationCheck::warning(
                    "git_clean_state",
                    "Git working tree has uncommitted changes",
                    None,
                )
            }
        }
        _ => ValidationCheck::warning("git_clean_state", "Could not check git status", None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::cost::RunCostData;
    use crate::agents::provider::{AgentProvider, AgentRunConfig};
    use std::path::{Path, PathBuf};

    /// Configurable stub for testing check functions that take `&dyn AgentProvider`.
    #[derive(Debug)]
    struct CheckStub {
        available: bool,
        global_hooks: bool,
        project_hooks: bool,
        user_commands: bool,
        project_commands: bool,
    }

    impl Default for CheckStub {
        fn default() -> Self {
            Self {
                available: false,
                global_hooks: false,
                project_hooks: false,
                user_commands: false,
                project_commands: false,
            }
        }
    }

    impl AgentProvider for CheckStub {
        fn id(&self) -> &str { "stub" }
        fn display_name(&self) -> &str { "Stub Agent" }
        fn build_command(&self, _: &AgentRunConfig) -> (String, Vec<String>) { ("stub".into(), vec![]) }
        fn build_env_vars(&self, _: &AgentRunConfig) -> Vec<(String, String)> { vec![] }
        fn extract_text(&self, o: &str) -> String { o.to_string() }
        fn extract_cost(&self, _: &str, _: &str, _: f64) -> Option<RunCostData> { None }
        fn is_available(&self) -> bool { self.available }
        fn get_version(&self) -> Option<String> { None }
        fn config_dir_name(&self) -> &str { ".stub" }
        fn command_instructions_subdir(&self) -> &str { "commands" }
        fn format_command_reference(&self, c: &str) -> String { format!("/{c}") }
        fn install_hooks_for_run(&self, _: &Path, _: &str, _: Option<&str>, _: Option<&str>, _: Option<&str>) -> Result<(), String> { Ok(()) }
        fn check_hooks_installed_global(&self) -> bool { self.global_hooks }
        fn check_hooks_installed_project(&self, _: &Path) -> bool { self.project_hooks }
        fn check_commands_installed_user(&self) -> bool { self.user_commands }
        fn check_commands_installed_project(&self, _: &Path) -> bool { self.project_commands }
    }

    // ── check_cli_available ──────────────────────────────────────────

    #[test]
    fn cli_available_passes_when_provider_is_available() {
        let stub = CheckStub { available: true, ..Default::default() };
        let check = check_cli_available(&stub);
        assert!(check.passed);
        assert_eq!(check.name, "cli_available");
        assert!(check.message.contains("Stub Agent"));
    }

    #[test]
    fn cli_available_fails_when_provider_is_unavailable() {
        let stub = CheckStub { available: false, ..Default::default() };
        let check = check_cli_available(&stub);
        assert!(!check.passed);
        assert!(check.message.contains("not installed"));
    }

    // ── check_hooks_configured ───────────────────────────────────────

    #[test]
    fn hooks_configured_passes_with_global_hooks() {
        let stub = CheckStub { global_hooks: true, ..Default::default() };
        let check = check_hooks_configured(&stub, Path::new("/tmp"));
        assert!(check.passed);
        assert!(check.message.contains("global"));
    }

    #[test]
    fn hooks_configured_passes_with_project_hooks() {
        let stub = CheckStub { project_hooks: true, ..Default::default() };
        let check = check_hooks_configured(&stub, Path::new("/tmp"));
        assert!(check.passed);
        assert!(check.message.contains("project"));
    }

    #[test]
    fn hooks_configured_prefers_project_over_global() {
        let stub = CheckStub { global_hooks: true, project_hooks: true, ..Default::default() };
        let check = check_hooks_configured(&stub, Path::new("/tmp"));
        assert!(check.passed);
        assert!(check.message.contains("project"));
    }

    #[test]
    fn hooks_configured_fails_when_neither_installed() {
        let stub = CheckStub::default();
        let check = check_hooks_configured(&stub, Path::new("/tmp"));
        assert!(!check.passed);
        assert_eq!(check.fix_action.as_deref(), Some("install_hooks"));
    }

    // ── check_commands_installed ──────────────────────────────────────

    #[test]
    fn commands_installed_passes_with_user_commands() {
        let stub = CheckStub { user_commands: true, ..Default::default() };
        let check = check_commands_installed(&stub, Path::new("/tmp"));
        assert!(check.passed);
        assert!(check.message.contains("user"));
    }

    #[test]
    fn commands_installed_passes_with_project_commands() {
        let stub = CheckStub { project_commands: true, ..Default::default() };
        let check = check_commands_installed(&stub, Path::new("/tmp"));
        assert!(check.passed);
        assert!(check.message.contains("project"));
    }

    #[test]
    fn commands_installed_prefers_user_over_project() {
        let stub = CheckStub { user_commands: true, project_commands: true, ..Default::default() };
        let check = check_commands_installed(&stub, Path::new("/tmp"));
        assert!(check.passed);
        assert!(check.message.contains("user"));
    }

    #[test]
    fn commands_installed_fails_when_neither_installed() {
        let stub = CheckStub::default();
        let check = check_commands_installed(&stub, Path::new("/tmp"));
        assert!(!check.passed);
        assert_eq!(check.fix_action.as_deref(), Some("install_commands"));
    }

    // ── existing tests ───────────────────────────────────────────────

    #[test]
    fn check_git_repository_detects_git_dir() {
        let temp_dir =
            std::env::temp_dir().join(format!("validation_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let check = check_git_repository(&temp_dir);
        assert!(!check.passed);

        std::fs::create_dir_all(temp_dir.join(".git")).unwrap();
        let check = check_git_repository(&temp_dir);
        assert!(check.passed);

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn check_api_url_configured_validates_url_format() {
        let check = check_api_url_configured("http://localhost:7432");
        assert!(check.passed);
        assert_eq!(check.name, "api_url_configured");

        let check = check_api_url_configured("https://api.example.com");
        assert!(check.passed);

        let check = check_api_url_configured("invalid-url");
        assert!(!check.passed);
        assert!(check.message.contains("Invalid API URL format"));
    }

    #[test]
    fn check_git_clean_state_in_non_git_dir() {
        let temp_dir =
            std::env::temp_dir().join(format!("validation_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let check = check_git_clean_state(&temp_dir);
        assert!(check.passed);
        assert!(check.is_warning);
        assert!(check.message.contains("Could not check"));

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn is_environment_valid_returns_bool() {
        use crate::agents::cursor::provider::CursorProvider;
        let temp_dir = PathBuf::from("/nonexistent/path");
        let provider = CursorProvider::new();
        assert!(!super::super::is_environment_valid(&provider, &temp_dir));
    }
}
