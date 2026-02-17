//! Cursor `AgentProvider` implementation.

use std::path::Path;

use crate::agents::cost::{self, RunCostData};
use crate::agents::provider::{AgentProvider, AgentRunConfig};

use super::availability;
use super::command;
use super::commands;
use super::hooks;
use super::settings;

#[derive(Debug)]
pub struct CursorProvider;

impl CursorProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CursorProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentProvider for CursorProvider {
    fn id(&self) -> &str {
        "cursor"
    }

    fn display_name(&self) -> &str {
        "Cursor"
    }

    fn build_command(&self, config: &AgentRunConfig) -> (String, Vec<String>) {
        let mut config = config.clone();
        config.model = config.model.map(|m| self.map_model_name(&m));
        command::build_command_from_provider_config(&config)
    }

    fn build_env_vars(&self, _config: &AgentRunConfig) -> Vec<(String, String)> {
        Vec::new()
    }

    fn extract_text(&self, output: &str) -> String {
        output.to_string()
    }

    fn extract_cost(
        &self,
        stdout: &str,
        model: &str,
        duration_secs: f64,
    ) -> Option<RunCostData> {
        let output_chars = stdout.len();
        if output_chars > 0 || duration_secs > 0.0 {
            Some(cost::estimate_cost(model, output_chars, duration_secs))
        } else {
            None
        }
    }

    fn is_available(&self) -> bool {
        availability::is_cursor_available()
    }

    fn get_version(&self) -> Option<String> {
        availability::get_cursor_version()
    }

    fn config_dir_name(&self) -> &str {
        ".cursor"
    }

    fn command_instructions_subdir(&self) -> &str {
        "rules"
    }

    fn format_command_reference(&self, command: &str) -> String {
        format!("/{}", command)
    }

    fn install_hooks_for_run(
        &self,
        repo_path: &Path,
        hook_script_path: &str,
        api_url: Option<&str>,
        api_token: Option<&str>,
        run_id: Option<&str>,
    ) -> Result<(), String> {
        hooks::install_hooks_with_run_id(repo_path, hook_script_path, api_url, api_token, run_id)
            .map_err(|e| format!("Failed to update Cursor hooks.json: {}", e))
    }

    fn check_hooks_installed_global(&self) -> bool {
        settings::check_global_hooks_installed()
    }

    fn check_hooks_installed_project(&self, repo_path: &Path) -> bool {
        settings::check_project_hooks_installed(repo_path)
    }

    fn install_hooks_global(
        &self,
        hook_script_path: &str,
        api_url: Option<&str>,
        api_token: Option<&str>,
    ) -> Result<(), String> {
        hooks::install_global_hooks(hook_script_path, api_url, api_token)
            .map_err(|e| e.to_string())
    }

    fn install_hooks_project(
        &self,
        repo_path: &Path,
        hook_script_path: &str,
        api_url: Option<&str>,
        api_token: Option<&str>,
    ) -> Result<(), String> {
        hooks::install_hooks(repo_path, hook_script_path, api_url, api_token)
            .map_err(|e| e.to_string())
    }

    fn generate_hooks_config_json(&self, hook_script_path: &str) -> Result<String, String> {
        let config = hooks::generate_hooks_json(hook_script_path);
        serde_json::to_string_pretty(&config).map_err(|e| e.to_string())
    }

    fn hook_script_name(&self) -> &str {
        "cursor-hook.js"
    }

    fn map_model_name(&self, model: &str) -> String {
        match model {
            "opus-4.6" => "claude-opus-4-6".to_string(),
            "opus-4.5" => "claude-opus-4-5".to_string(),
            "sonnet-4.5" => "claude-sonnet-4-5".to_string(),
            other => other.to_string(),
        }
    }

    fn check_commands_installed_project(&self, repo_path: &Path) -> bool {
        commands::check_project_commands_installed(repo_path)
    }

    fn check_commands_installed_user(&self) -> bool {
        commands::check_user_commands_installed()
    }

    fn install_commands_to_project(
        &self,
        repo_path: &Path,
        commands_source: &Path,
    ) -> Result<Vec<String>, String> {
        commands::install_commands(repo_path, commands_source)
            .map_err(|e| format!("Failed to install Cursor commands: {}", e))
    }

    fn install_commands_to_user(
        &self,
        commands_source: &Path,
    ) -> Result<Vec<String>, String> {
        commands::install_user_commands(commands_source)
            .map_err(|e| format!("Failed to install Cursor user commands: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn make_config() -> AgentRunConfig {
        AgentRunConfig {
            agent_id: "cursor".to_string(),
            ticket_id: "t".to_string(),
            run_id: "r".to_string(),
            repo_path: PathBuf::from("/tmp/test"),
            prompt: "Test".to_string(),
            timeout_secs: None,
            api_url: "http://localhost:7432".to_string(),
            api_token: "tok".to_string(),
            model: None,
            agent_config: HashMap::new(),
        }
    }

    #[test]
    fn provider_id_and_display_name() {
        let p = CursorProvider::new();
        assert_eq!(p.id(), "cursor");
        assert_eq!(p.display_name(), "Cursor");
    }

    #[test]
    fn build_command_returns_cursor() {
        let p = CursorProvider::new();
        let (cmd, args) = p.build_command(&make_config());
        assert_eq!(cmd, "cursor");
        assert!(args.contains(&"agent".to_string()));
    }

    #[test]
    fn build_env_vars_empty() {
        let p = CursorProvider::new();
        let env = p.build_env_vars(&make_config());
        assert!(env.is_empty());
    }

    #[test]
    fn extract_text_passthrough() {
        let p = CursorProvider::new();
        assert_eq!(p.extract_text("hello world"), "hello world");
    }

    #[test]
    fn extract_cost_estimates() {
        let p = CursorProvider::new();
        let cost = p.extract_cost("some output", "opus-4.6", 10.0);
        assert!(cost.is_some());
        assert!(cost.unwrap().is_estimated);
    }

    #[test]
    fn extract_cost_empty_returns_none() {
        let p = CursorProvider::new();
        let cost = p.extract_cost("", "opus-4.6", 0.0);
        assert!(cost.is_none());
    }

    // ── map_model_name ─────────────────────────────────────────────

    #[test]
    fn map_model_name_maps_known_models() {
        let p = CursorProvider::new();
        assert_eq!(p.map_model_name("opus-4.6"), "claude-opus-4-6");
        assert_eq!(p.map_model_name("opus-4.5"), "claude-opus-4-5");
        assert_eq!(p.map_model_name("sonnet-4.5"), "claude-sonnet-4-5");
    }

    #[test]
    fn map_model_name_passes_through_unknown() {
        let p = CursorProvider::new();
        assert_eq!(p.map_model_name("custom-model"), "custom-model");
        assert_eq!(p.map_model_name("claude-opus-4-6"), "claude-opus-4-6");
    }

    #[test]
    fn build_command_maps_model_name_end_to_end() {
        let p = CursorProvider::new();
        let mut config = make_config();
        config.model = Some("opus-4.6".to_string());
        let (_, args) = p.build_command(&config);
        assert!(
            args.contains(&"claude-opus-4-6".to_string()),
            "Provider build_command should map opus-4.6 -> claude-opus-4-6"
        );
    }

    #[test]
    fn build_command_no_model_omits_model_flag() {
        let p = CursorProvider::new();
        let config = make_config();
        let (_, args) = p.build_command(&config);
        assert!(!args.contains(&"--model".to_string()));
    }

    // ── New trait methods coverage ───────────────────────────────────

    #[test]
    fn config_dir_name_returns_cursor() {
        let p = CursorProvider::new();
        assert_eq!(p.config_dir_name(), ".cursor");
    }

    #[test]
    fn command_instructions_subdir_returns_rules() {
        let p = CursorProvider::new();
        assert_eq!(p.command_instructions_subdir(), "rules");
    }

    #[test]
    fn format_command_reference_returns_slash_command() {
        let p = CursorProvider::new();
        assert_eq!(p.format_command_reference("deslop"), "/deslop");
        assert_eq!(p.format_command_reference("add-and-commit"), "/add-and-commit");
    }

    #[test]
    fn check_commands_installed_project_returns_false_for_missing_dir() {
        let temp = std::env::temp_dir().join(format!("cursor_prov_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp).unwrap();
        let p = CursorProvider::new();
        assert!(!p.check_commands_installed_project(&temp));
        std::fs::remove_dir_all(&temp).ok();
    }

    #[test]
    fn check_hooks_installed_project_returns_false_for_missing_hooks() {
        let temp = std::env::temp_dir().join(format!("cursor_prov_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp).unwrap();
        let p = CursorProvider::new();
        assert!(!p.check_hooks_installed_project(&temp));
        std::fs::remove_dir_all(&temp).ok();
    }
}
