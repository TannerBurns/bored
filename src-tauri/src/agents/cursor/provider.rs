//! Cursor `AgentProvider` implementation.

use crate::agents::cli_utils;
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
        command::build_command_from_provider_config(config)
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

}
