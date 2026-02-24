//! Worker environment validation for agents.

use std::path::Path;

use crate::agents::provider::AgentProvider;

mod checks;
mod types;

pub use types::{ValidationCheck, ValidationResult};

pub fn validate_worker_environment(
    provider: &dyn AgentProvider,
    repo_path: &Path,
    api_url: Option<&str>,
) -> ValidationResult {
    validate_worker_environment_with_options(provider, repo_path, api_url, true)
}

/// Validate worker environment with configurable git requirement.
pub fn validate_worker_environment_with_options(
    provider: &dyn AgentProvider,
    repo_path: &Path,
    api_url: Option<&str>,
    requires_git: bool,
) -> ValidationResult {
    let mut all_checks = Vec::new();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    let cli_check = checks::check_cli_available(provider);
    if !cli_check.passed {
        errors.push(cli_check.message.clone());
    }
    all_checks.push(cli_check);

    if requires_git {
        let git_check = checks::check_git_repository(repo_path);
        if !git_check.passed {
            errors.push(git_check.message.clone());
        }
        all_checks.push(git_check);
    } else {
        all_checks.push(ValidationCheck::pass(
            "git_repository",
            "Git not required for this project",
        ));
    }

    if let Some(url) = api_url {
        let api_check = checks::check_api_url_configured(url);
        if !api_check.passed {
            errors.push(api_check.message.clone());
        }
        all_checks.push(api_check);
    }

    if requires_git {
        let clean_check = checks::check_git_clean_state(repo_path);
        if clean_check.is_warning {
            warnings.push(clean_check.message.clone());
        }
        all_checks.push(clean_check);
    }

    ValidationResult {
        valid: errors.is_empty(),
        checks: all_checks,
        errors,
        warnings,
    }
}

pub fn is_environment_valid(provider: &dyn AgentProvider, repo_path: &Path) -> bool {
    let result = validate_worker_environment(provider, repo_path, None);
    result.valid
}

/// Check if environment is valid with configurable git requirement.
pub fn is_environment_valid_with_options(
    provider: &dyn AgentProvider,
    repo_path: &Path,
    requires_git: bool,
) -> bool {
    let result =
        validate_worker_environment_with_options(provider, repo_path, None, requires_git);
    result.valid
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::cursor::provider::CursorProvider;
    use std::path::PathBuf;

    #[test]
    fn validate_worker_environment_returns_result() {
        let temp_dir =
            std::env::temp_dir().join(format!("validation_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let provider = CursorProvider::new();
        let result = validate_worker_environment(&provider, &temp_dir, None);

        assert!(!result.checks.is_empty());
        assert!(!result.valid);
        assert!(!result.errors.is_empty());

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn is_environment_valid_returns_bool() {
        let temp_dir = PathBuf::from("/nonexistent/path");
        let provider = CursorProvider::new();
        assert!(!is_environment_valid(&provider, &temp_dir));
    }

    #[test]
    fn validate_with_requires_git_false_skips_git_check() {
        let temp_dir =
            std::env::temp_dir().join(format!("validation_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let provider = CursorProvider::new();
        let result = validate_worker_environment_with_options(
            &provider,
            &temp_dir,
            None,
            false,
        );

        let git_check = result.checks.iter().find(|c| c.name == "git_repository");
        assert!(git_check.is_some());
        assert!(git_check.unwrap().passed);
        assert!(git_check.unwrap().message.contains("not required"));

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn validate_with_requires_git_true_fails_on_missing_git() {
        let temp_dir =
            std::env::temp_dir().join(format!("validation_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let provider = CursorProvider::new();
        let result = validate_worker_environment_with_options(
            &provider,
            &temp_dir,
            None,
            true,
        );

        let git_check = result.checks.iter().find(|c| c.name == "git_repository");
        assert!(git_check.is_some());
        assert!(!git_check.unwrap().passed);

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn validation_does_not_include_hooks_check() {
        let temp_dir =
            std::env::temp_dir().join(format!("validation_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let provider = CursorProvider::new();
        let result = validate_worker_environment(&provider, &temp_dir, None);

        let hooks_check = result.checks.iter().find(|c| c.name == "hooks_configured");
        assert!(
            hooks_check.is_none(),
            "Validation should not include a hooks_configured check after hooks removal"
        );

        let check_names: Vec<&str> = result.checks.iter().map(|c| c.name.as_str()).collect();
        assert!(check_names.contains(&"cli_available"));
        assert!(check_names.contains(&"git_repository"));

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn validation_does_not_include_commands_installed_check() {
        let temp_dir =
            std::env::temp_dir().join(format!("validation_no_cmds_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let provider = CursorProvider::new();
        let result = validate_worker_environment(&provider, &temp_dir, None);

        let commands_check = result.checks.iter().find(|c| c.name == "commands_installed");
        assert!(
            commands_check.is_none(),
            "Validation should not include commands_installed check — commands are now app-internal"
        );

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn validation_check_names_are_correct_set() {
        let temp_dir =
            std::env::temp_dir().join(format!("validation_names_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let provider = CursorProvider::new();
        let result = validate_worker_environment(&provider, &temp_dir, None);
        let check_names: Vec<&str> = result.checks.iter().map(|c| c.name.as_str()).collect();

        assert!(check_names.contains(&"cli_available"));
        assert!(check_names.contains(&"git_repository"));
        assert!(check_names.contains(&"git_clean_state"));
        assert!(!check_names.contains(&"commands_installed"));
        assert!(!check_names.contains(&"hooks_configured"));

        std::fs::remove_dir_all(&temp_dir).ok();
    }
}
