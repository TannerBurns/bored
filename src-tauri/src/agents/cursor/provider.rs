//! Cursor `AgentProvider` implementation.

use std::path::Path;

use crate::agents::cli_utils::{self, is_dangerous_command};
use crate::agents::command_templates;
use crate::agents::cost::{self, RunCostData};
use crate::agents::provider::{
    AgentProvider, AgentRunConfig, HookAction, NormalizedHookEvent, StopEventResult,
};

use super::command;
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
        cli_utils::is_cli_available("cursor")
    }

    fn get_version(&self) -> Option<String> {
        cli_utils::get_cli_version("cursor")
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

    fn normalize_hook_event(
        &self,
        raw_event_type: &str,
        raw_payload: &serde_json::Value,
    ) -> NormalizedHookEvent {
        let event_type = match raw_event_type {
            "beforeShellExecution" => "command_requested",
            "afterShellExecution" => "command_executed",
            "beforeReadFile" => "file_read",
            "afterFileEdit" => "file_edited",
            "beforeMCPExecution" => "command_requested",
            "stop" => "run_stopped",
            "beforeSubmitPrompt" => "prompt_submitted",
            other => other,
        };

        let structured = match raw_event_type {
            "beforeShellExecution" | "afterShellExecution" => serde_json::json!({
                "command": raw_payload.get("command").and_then(|v| v.as_str()).unwrap_or(""),
                "workingDirectory": raw_payload.get("cwd").and_then(|v| v.as_str()),
            }),
            "afterFileEdit" => serde_json::json!({
                "filePath": raw_payload.get("path").and_then(|v| v.as_str()).unwrap_or(""),
                "oldContent": raw_payload.get("oldContent")
                    .and_then(|v| v.as_str())
                    .map(|s| &s[..s.len().min(500)]),
                "newContent": raw_payload.get("newContent")
                    .and_then(|v| v.as_str())
                    .map(|s| &s[..s.len().min(500)]),
            }),
            "beforeReadFile" => serde_json::json!({
                "filePath": raw_payload.get("path").and_then(|v| v.as_str()).unwrap_or(""),
            }),
            "stop" => serde_json::json!({
                "status": raw_payload.get("status").and_then(|v| v.as_str()).unwrap_or(""),
                "reason": raw_payload.get("reason").and_then(|v| v.as_str()),
            }),
            _ => raw_payload.clone(),
        };

        NormalizedHookEvent {
            event_type: event_type.to_string(),
            structured,
        }
    }

    fn hook_action(
        &self,
        raw_event_type: &str,
        raw_payload: &serde_json::Value,
        _ticket_id: Option<&str>,
        _run_id: Option<&str>,
    ) -> HookAction {
        if raw_event_type == "beforeShellExecution" {
            let command = raw_payload
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if is_dangerous_command(command) {
                return HookAction::Deny {
                    reason: "Blocked by Agent Kanban for safety".to_string(),
                };
            }
        }
        HookAction::Allow
    }

    fn normalize_stop_event(
        &self,
        raw_payload: &serde_json::Value,
    ) -> StopEventResult {
        let status_str = raw_payload
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match status_str {
            "error" => StopEventResult {
                status: "error".to_string(),
                exit_code: 1,
                summary: "Stopped: error".to_string(),
            },
            "aborted" => StopEventResult {
                status: "aborted".to_string(),
                exit_code: 1,
                summary: "Stopped: aborted".to_string(),
            },
            _ => StopEventResult {
                status: "finished".to_string(),
                exit_code: 0,
                summary: "Completed successfully.".to_string(),
            },
        }
    }

    fn hook_script_name(&self) -> &str {
        "cursor-hook.js"
    }

    fn map_model_name(&self, model: &str) -> String {
        crate::agents::models::map_model_name(model)
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
