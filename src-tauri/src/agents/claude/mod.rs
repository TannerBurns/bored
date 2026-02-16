//! Claude CLI integration: commands, hooks, and availability checking.

// Submodules
mod availability;
mod command;
#[cfg(test)]
mod command_tests;
mod commands;
mod hooks;
pub mod provider;
#[cfg(test)]
mod provider_tests;
mod settings;

// Public re-exports
pub use availability::{get_claude_version, is_claude_available};
pub use command::ClaudeSettings;
pub use commands::{
    check_project_commands_installed, check_user_commands_installed, get_available_commands,
    get_bundled_commands_path, get_bundled_commands_path_with_app, install_commands,
    install_user_commands, COMMAND_TEMPLATES,
};
pub use hooks::{
    generate_hooks_config, generate_hooks_settings, generate_hooks_settings_with_api,
    generate_hooks_settings_with_config, install_local_hooks, install_local_hooks_with_run_id,
    install_project_hooks, install_user_hooks, HooksConfig,
};
pub use settings::{
    check_global_hooks_installed, check_project_hooks_installed, local_settings_path,
    project_settings_path, user_settings_path,
};

/// Shell-escape a string for safe use in shell commands.
pub(crate) fn shell_escape(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '/' || c == '.')
    {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_escape_simple_path() {
        assert_eq!(shell_escape("/path/to/hook.js"), "/path/to/hook.js");
    }

    #[test]
    fn shell_escape_empty_string() {
        assert_eq!(shell_escape(""), "''");
    }

    #[test]
    fn shell_escape_path_with_spaces() {
        assert_eq!(shell_escape("/my path/to/hook.js"), "'/my path/to/hook.js'");
    }

    #[test]
    fn shell_escape_path_with_single_quote() {
        assert_eq!(
            shell_escape("/path/it's/hook.js"),
            "'/path/it'\\''s/hook.js'"
        );
    }

    #[test]
    fn shell_escape_special_characters() {
        assert_eq!(shell_escape("value with $var"), "'value with $var'");
        assert_eq!(shell_escape("value;rm -rf /"), "'value;rm -rf /'");
    }

    #[test]
    fn shell_escape_alphanumeric_unchanged() {
        assert_eq!(shell_escape("simple_value-123"), "simple_value-123");
    }

    #[test]
    fn shell_escape_url_with_colon() {
        assert_eq!(
            shell_escape("http://localhost:7432"),
            "'http://localhost:7432'"
        );
    }
}
