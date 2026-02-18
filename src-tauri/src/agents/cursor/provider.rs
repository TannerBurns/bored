//! Cursor `AgentProvider` implementation.

use std::path::Path;

use crate::agents::cli_utils;
use crate::agents::command_templates;
use crate::agents::cost::{self, RunCostData};
use crate::agents::provider::{AgentProvider, AgentRunConfig};

use super::command;

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
        let thinking = config
            .agent_config
            .get("thinking_enabled")
            .or_else(|| config.agent_config.get("thinkingEnabled"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let mut config = config.clone();
        config.model = config.model.map(|m| {
            let base = self.map_model_name(&m);
            if thinking {
                format!("{}-thinking", base)
            } else {
                base
            }
        });
        command::build_command_from_provider_config(&config)
    }

    fn build_env_vars(&self, _config: &AgentRunConfig) -> Vec<(String, String)> {
        Vec::new()
    }

    fn extract_text(&self, output: &str) -> String {
        crate::agents::claude::provider::extract_text_from_stream_json(output)
            .unwrap_or_else(|| output.to_string())
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
        cli_utils::is_cli_available("cursor")
    }

    fn get_version(&self) -> Option<String> {
        cli_utils::get_cli_version("cursor")
    }

    fn config_dir_name(&self) -> &str {
        ".cursor"
    }

    fn command_instructions_subdir(&self) -> &str {
        "commands"
    }

    fn format_command_reference(&self, command: &str) -> String {
        format!("/{}", command)
    }

    fn available_models(&self) -> Vec<(&str, &str)> {
        vec![
            ("opus-4.6", "Opus 4.6"),
            ("opus-4.5", "Opus 4.5"),
            ("sonnet-4.6", "Sonnet 4.6"),
            ("sonnet-4.5", "Sonnet 4.5"),
            ("gpt-5.3-codex", "GPT-5.3 Codex"),
            ("gpt-5.2-codex", "GPT-5.2 Codex"),
        ]
    }

    fn check_commands_installed_project(&self, repo_path: &Path) -> bool {
        command_templates::check_project_commands_installed(repo_path, self.config_dir_name())
    }

    fn check_commands_installed_user(&self) -> bool {
        command_templates::check_user_commands_installed(self.config_dir_name())
    }

    fn install_commands_to_project(
        &self,
        repo_path: &Path,
        commands_source: &Path,
    ) -> Result<Vec<String>, String> {
        command_templates::install_commands(repo_path, self.config_dir_name(), commands_source)
            .map_err(|e| format!("Failed to install {} commands: {}", self.display_name(), e))
    }

    fn install_commands_to_user(
        &self,
        commands_source: &Path,
    ) -> Result<Vec<String>, String> {
        command_templates::install_user_commands(self.config_dir_name(), commands_source)
            .map_err(|e| format!("Failed to install {} user commands: {}", self.display_name(), e))
    }
}
