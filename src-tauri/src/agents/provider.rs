//! Agent provider trait — the core abstraction for agent-agnostic execution.
//!
//! Every agent backend implements `AgentProvider` so that the orchestrator,
//! spawner, and runner can work with any agent without hard-coding
//! agent-specific logic.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::cost::RunCostData;

// ── Hook event types ────────────────────────────────────────────────

/// Normalized hook event with a common event type and structured payload.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedHookEvent {
    /// Common event type (e.g. "command_requested", "file_edited", "run_stopped").
    pub event_type: String,
    /// Structured payload with agent-specific details extracted into common fields.
    pub structured: serde_json::Value,
}

/// Action the hook script should take after posting an event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum HookAction {
    Allow,
    Deny { reason: String },
    InjectContext { context: String },
    NoAction,
}

/// Result of normalizing a stop/session-end event into common run status fields.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StopEventResult {
    /// Common status: "finished", "error", or "aborted".
    pub status: String,
    pub exit_code: i32,
    pub summary: String,
}

/// Configuration for running an agent — agent-agnostic.
#[derive(Debug, Clone)]
pub struct AgentRunConfig {
    /// Which agent to use (e.g. "cursor", "claude")
    pub agent_id: String,
    pub ticket_id: String,
    pub run_id: String,
    pub repo_path: PathBuf,
    pub prompt: String,
    pub timeout_secs: Option<u64>,
    pub api_url: String,
    pub api_token: String,
    pub model: Option<String>,
    /// Opaque agent-specific configuration.
    /// Each provider knows its own keys (e.g. Claude uses "auth_token", "api_key", etc.).
    pub agent_config: HashMap<String, serde_json::Value>,
}

/// The trait every agent implementation must satisfy.
///
/// Methods are synchronous where CLI access is blocking (availability, version),
/// and take shared references to allow concurrent use behind `Arc`.
pub trait AgentProvider: Send + Sync + std::fmt::Debug {
    /// Unique lowercase identifier (e.g. "cursor", "claude").
    fn id(&self) -> &str;

    /// Human-readable display name (e.g. "Cursor", "Claude Code").
    fn display_name(&self) -> &str;

    // ── Command building ────────────────────────────────────────────

    /// Build the CLI command and argument list for a run.
    fn build_command(&self, config: &AgentRunConfig) -> (String, Vec<String>);

    /// Build agent-specific environment variables.
    ///
    /// Base environment variables (AGENT_KANBAN_*) are added by the spawner;
    /// this method only needs to return additional, agent-specific variables.
    fn build_env_vars(&self, config: &AgentRunConfig) -> Vec<(String, String)>;

    // ── Output parsing ──────────────────────────────────────────────

    /// Extract the meaningful text from raw agent output.
    fn extract_text(&self, output: &str) -> String;

    /// Extract cost/token data from agent output, or estimate it.
    fn extract_cost(
        &self,
        stdout: &str,
        model: &str,
        duration_secs: f64,
    ) -> Option<RunCostData>;

    // ── Availability ────────────────────────────────────────────────

    /// Check whether this agent's CLI is available on the system.
    fn is_available(&self) -> bool;

    /// Get the agent CLI version string, if available.
    fn get_version(&self) -> Option<String>;

    // ── Directory configuration ─────────────────────────────────────

    /// The dot-directory name this agent uses (e.g. ".cursor", ".claude").
    fn config_dir_name(&self) -> &str;

    /// Subdirectory under `config_dir_name()` where command instruction files
    /// live in a repo. For Cursor this is "rules", for Claude this is "commands".
    fn command_instructions_subdir(&self) -> &str;

    /// Format a command reference as this agent expects it in a prompt.
    /// e.g. Cursor returns "/deslop", Claude returns ".claude/commands/deslop.md".
    fn format_command_reference(&self, command: &str) -> String;

    // ── Hooks ───────────────────────────────────────────────────────

    /// Install project-level hooks for a specific run.
    ///
    /// Called by the orchestrator/runner before each stage so the hook script
    /// can report events back to the API with the correct run ID.
    fn install_hooks_for_run(
        &self,
        repo_path: &Path,
        hook_script_path: &str,
        api_url: Option<&str>,
        api_token: Option<&str>,
        run_id: Option<&str>,
    ) -> Result<(), String>;

    /// Check whether hooks are installed at the global/user level.
    fn check_hooks_installed_global(&self) -> bool {
        false
    }

    /// Check whether hooks are installed for a specific project.
    fn check_hooks_installed_project(&self, _repo_path: &Path) -> bool {
        false
    }

    /// Install hooks at the global/user level (e.g. ~/.cursor, ~/Library/...).
    fn install_hooks_global(
        &self,
        _hook_script_path: &str,
        _api_url: Option<&str>,
        _api_token: Option<&str>,
    ) -> Result<(), String> {
        Err("Global hook installation not supported by this agent".to_string())
    }

    /// Install hooks for a specific project directory.
    fn install_hooks_project(
        &self,
        _repo_path: &Path,
        _hook_script_path: &str,
        _api_url: Option<&str>,
        _api_token: Option<&str>,
    ) -> Result<(), String> {
        Err("Project hook installation not supported by this agent".to_string())
    }

    /// Generate the hooks configuration as a JSON string.
    fn generate_hooks_config_json(&self, _hook_script_path: &str) -> Result<String, String> {
        Ok("{}".to_string())
    }

    /// The filename of this agent's hook script (e.g. "cursor-hook.js"),
    /// or empty if the agent doesn't use one.
    fn hook_script_name(&self) -> &str {
        ""
    }

    // ── Hook event normalization ─────────────────────────────────────

    /// Normalize a raw hook event into the common event schema.
    fn normalize_hook_event(
        &self,
        raw_event_type: &str,
        raw_payload: &serde_json::Value,
    ) -> NormalizedHookEvent {
        NormalizedHookEvent {
            event_type: raw_event_type.to_lowercase(),
            structured: raw_payload.clone(),
        }
    }

    /// Decide what action to take for a hook event.
    fn hook_action(
        &self,
        _raw_event_type: &str,
        _raw_payload: &serde_json::Value,
        _ticket_id: Option<&str>,
        _run_id: Option<&str>,
    ) -> HookAction {
        HookAction::Allow
    }

    /// Normalize a stop/session-end event into common run status fields.
    fn normalize_stop_event(
        &self,
        _raw_payload: &serde_json::Value,
    ) -> StopEventResult {
        StopEventResult {
            status: "finished".to_string(),
            exit_code: 0,
            summary: "Completed successfully.".to_string(),
        }
    }

    /// Brand color hex for UI display (e.g. "#da7756").
    fn brand_color(&self) -> Option<&str> {
        None
    }

    /// Map a friendly model name to the CLI format this agent expects.
    /// E.g. Claude maps "opus-4.6" -> "claude-opus-4-6".
    fn map_model_name(&self, model: &str) -> String {
        model.to_string()
    }

    // ── Commands checking and installation ───────────────────────────

    /// Check whether command templates are installed in a project.
    fn check_commands_installed_project(&self, _repo_path: &Path) -> bool {
        false
    }

    /// Check whether command templates are installed at the user level.
    fn check_commands_installed_user(&self) -> bool {
        false
    }

    /// Install command templates into a project directory.
    fn install_commands_to_project(
        &self,
        _repo_path: &Path,
        _commands_source: &Path,
    ) -> Result<Vec<String>, String> {
        Ok(vec![])
    }

    /// Install command templates to the user-level directory.
    fn install_commands_to_user(
        &self,
        _commands_source: &Path,
    ) -> Result<Vec<String>, String> {
        Ok(vec![])
    }

    /// Get the path to bundled command templates (dev builds).
    fn get_bundled_commands_path(&self) -> Option<PathBuf> {
        let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("scripts")
            .join("commands");
        if dev_path.exists() {
            Some(dev_path)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── HookAction serialization ──────────────────────────────────
    // The JS hook script parses these JSON shapes directly. These tests
    // act as a contract: if serde attributes change, the tests break
    // before the JS does.

    #[test]
    fn hook_action_allow_serializes_correctly() {
        let json = serde_json::to_value(&HookAction::Allow).unwrap();
        assert_eq!(json["action"], "allow");
    }

    #[test]
    fn hook_action_deny_serializes_with_reason() {
        let action = HookAction::Deny {
            reason: "too dangerous".to_string(),
        };
        let json = serde_json::to_value(&action).unwrap();
        assert_eq!(json["action"], "deny");
        assert_eq!(json["reason"], "too dangerous");
    }

    #[test]
    fn hook_action_inject_context_serializes_with_context() {
        let action = HookAction::InjectContext {
            context: "ticket info".to_string(),
        };
        let json = serde_json::to_value(&action).unwrap();
        assert_eq!(json["action"], "inject_context");
        assert_eq!(json["context"], "ticket info");
    }

    #[test]
    fn hook_action_no_action_serializes_correctly() {
        let json = serde_json::to_value(&HookAction::NoAction).unwrap();
        assert_eq!(json["action"], "no_action");
    }

    #[test]
    fn hook_action_roundtrips_through_serde() {
        let cases = vec![
            HookAction::Allow,
            HookAction::Deny { reason: "bad".to_string() },
            HookAction::InjectContext { context: "ctx".to_string() },
            HookAction::NoAction,
        ];
        for action in cases {
            let json = serde_json::to_string(&action).unwrap();
            let restored: HookAction = serde_json::from_str(&json).unwrap();
            assert_eq!(restored, action);
        }
    }

    // ── StopEventResult serialization ─────────────────────────────

    #[test]
    fn stop_event_result_serializes_camel_case() {
        let result = StopEventResult {
            status: "error".to_string(),
            exit_code: 1,
            summary: "Failed".to_string(),
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["status"], "error");
        assert_eq!(json["exitCode"], 1);
        assert_eq!(json["summary"], "Failed");
        // Verify camelCase (not snake_case)
        assert!(json.get("exit_code").is_none());
    }

    // ── Default trait impls ───────────────────────────────────────

    #[test]
    fn default_normalize_hook_event_lowercases_event_type() {
        let stub = StubProvider;
        let payload = serde_json::json!({"key": "value"});
        let result = stub.normalize_hook_event("SomeEvent", &payload);
        assert_eq!(result.event_type, "someevent");
        assert_eq!(result.structured, payload);
    }

    #[test]
    fn default_hook_action_returns_allow() {
        let stub = StubProvider;
        let action = stub.hook_action("any", &serde_json::json!({}), None, None);
        assert_eq!(action, HookAction::Allow);
    }

    #[test]
    fn default_normalize_stop_event_returns_finished() {
        let stub = StubProvider;
        let result = stub.normalize_stop_event(&serde_json::json!({}));
        assert_eq!(result.status, "finished");
        assert_eq!(result.exit_code, 0);
    }

    #[derive(Debug)]
    struct StubProvider;

    impl AgentProvider for StubProvider {
        fn id(&self) -> &str { "stub" }
        fn display_name(&self) -> &str { "Stub" }
        fn build_command(&self, _: &AgentRunConfig) -> (String, Vec<String>) {
            ("stub".into(), vec![])
        }
        fn build_env_vars(&self, _: &AgentRunConfig) -> Vec<(String, String)> { vec![] }
        fn extract_text(&self, o: &str) -> String { o.into() }
        fn extract_cost(&self, _: &str, _: &str, _: f64) -> Option<crate::agents::cost::RunCostData> { None }
        fn is_available(&self) -> bool { false }
        fn get_version(&self) -> Option<String> { None }
        fn config_dir_name(&self) -> &str { ".stub" }
        fn command_instructions_subdir(&self) -> &str { "commands" }
        fn format_command_reference(&self, c: &str) -> String { format!("/{c}") }
        fn install_hooks_for_run(&self, _: &Path, _: &str, _: Option<&str>, _: Option<&str>, _: Option<&str>) -> Result<(), String> { Ok(()) }
    }
}
