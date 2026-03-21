//! Cursor `AgentProvider` implementation.

use crate::agents::cli_utils;
use crate::agents::cost::{self, RunCostData};
use crate::agents::provider::{AgentProvider, AgentRunConfig};

use super::command;
use super::models;

#[derive(Debug)]
pub struct CursorProvider {
    /// Dynamically discovered (id, label) pairs from `cursor agent --list-models`.
    models: Vec<(String, String)>,
}

impl CursorProvider {
    pub fn new() -> Self {
        Self {
            models: Vec::new(),
        }
    }

    /// Create a provider pre-populated with discovered models.
    pub fn with_models(models: Vec<(String, String)>) -> Self {
        Self { models }
    }

    /// Discover available models by running `cursor agent --list-models`.
    /// On failure (CLI not installed, timeout, etc.) logs a warning and
    /// leaves the model list empty.
    pub fn discover_models(&mut self) {
        match models::list_models() {
            Ok(list) => {
                self.models = list
                    .models
                    .into_iter()
                    .map(|m| (m.id, m.label))
                    .collect();
                tracing::info!(
                    "Discovered {} Cursor models",
                    self.models.len(),
                );
            }
            Err(e) => {
                tracing::warn!("Could not discover Cursor models: {e}");
            }
        }
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

    fn extract_session_id(&self, output: &str) -> Option<String> {
        crate::agents::claude::provider::extract_session_id_from_stream_json(output)
    }

    fn available_models(&self) -> Vec<(&str, &str)> {
        self.models
            .iter()
            .map(|(id, label)| (id.as_str(), label.as_str()))
            .collect()
    }

}
