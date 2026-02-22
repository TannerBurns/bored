//! Agent provider trait — the core abstraction for agent-agnostic execution.
//!
//! Every agent backend implements `AgentProvider` so that the orchestrator,
//! spawner, and runner can work with any agent without hard-coding
//! agent-specific logic.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::cost::RunCostData;

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
    /// Base environment variables (BORED_*) are added by the spawner;
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

    /// Brand color hex for UI display (e.g. "#da7756").
    fn brand_color(&self) -> Option<&str> {
        None
    }

    /// Map a friendly model name to the CLI format this agent expects.
    /// E.g. "opus-4.6" -> "claude-opus-4-6".
    ///
    /// The default is a passthrough. Providers that target a specific model
    /// family should override this (see [`models::map_model_name`]).
    fn map_model_name(&self, model: &str) -> String {
        model.to_string()
    }

    /// Return the list of models this agent supports as `(id, display_label)` pairs.
    /// Used by the frontend to populate model dropdowns per agent.
    fn available_models(&self) -> Vec<(&str, &str)> {
        vec![]
    }

    // ── Local provider overrides ──────────────────────────────────────

    /// Whether the agent_config indicates a local/self-hosted provider override.
    ///
    /// When true, cost tracking will record token counts but zero out USD cost
    /// since self-hosted inference has no per-token API charge.
    fn is_local_override(&self, _agent_config: &HashMap<String, serde_json::Value>) -> bool {
        false
    }

    /// Resolve the model name to use for cost/usage tracking.
    ///
    /// When a local provider override supplies a `model_override`, this returns
    /// that name so usage reports attribute tokens to the actual model rather
    /// than the workflow-settings stage model.
    fn effective_cost_model(
        &self,
        stage_model: &str,
        _agent_config: &HashMap<String, serde_json::Value>,
    ) -> String {
        stage_model.to_string()
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

/// Extract cost with local-provider-aware model resolution and zero-cost handling.
///
/// Composes [`AgentProvider::effective_cost_model`], [`AgentProvider::extract_cost`],
/// and [`AgentProvider::is_local_override`] into a single call. All cost extraction
/// call sites should use this to ensure consistent handling of local overrides.
pub fn extract_cost_with_overrides(
    provider: &dyn AgentProvider,
    stdout: &str,
    stage_model: &str,
    agent_config: &HashMap<String, serde_json::Value>,
    duration_secs: f64,
) -> Option<RunCostData> {
    let effective_model = provider.effective_cost_model(stage_model, agent_config);
    let mut cost = provider.extract_cost(stdout, &effective_model, duration_secs)?;
    if provider.is_local_override(agent_config) {
        cost.zero_out_costs();
    }
    Some(cost)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    #[test]
    fn stub_provider_has_correct_id() {
        let stub = StubProvider;
        assert_eq!(stub.id(), "stub");
    }

    #[test]
    fn default_is_local_override_returns_false() {
        let stub = StubProvider;
        let empty = HashMap::new();
        assert!(!stub.is_local_override(&empty));

        let mut with_values = HashMap::new();
        with_values.insert("base_url".to_string(), serde_json::json!("http://localhost"));
        assert!(!stub.is_local_override(&with_values));
    }

    #[test]
    fn default_effective_cost_model_passes_through_stage_model() {
        let stub = StubProvider;
        let empty = HashMap::new();
        assert_eq!(stub.effective_cost_model("opus-4.6", &empty), "opus-4.6");

        let mut with_override = HashMap::new();
        with_override.insert("model_override".to_string(), serde_json::json!("custom"));
        assert_eq!(
            stub.effective_cost_model("opus-4.6", &with_override),
            "opus-4.6",
            "default impl ignores agent_config"
        );
    }

    // ── extract_cost_with_overrides tests ──────────────────────────

    #[derive(Debug)]
    struct FakeProvider {
        local_override: bool,
        override_model: Option<String>,
    }

    impl AgentProvider for FakeProvider {
        fn id(&self) -> &str { "fake" }
        fn display_name(&self) -> &str { "Fake" }
        fn build_command(&self, _: &AgentRunConfig) -> (String, Vec<String>) {
            ("fake".into(), vec![])
        }
        fn build_env_vars(&self, _: &AgentRunConfig) -> Vec<(String, String)> { vec![] }
        fn extract_text(&self, o: &str) -> String { o.into() }
        fn extract_cost(&self, stdout: &str, model: &str, _dur: f64) -> Option<RunCostData> {
            if stdout.is_empty() { return None; }
            let mut usage = HashMap::new();
            usage.insert(model.to_string(), crate::agents::cost::ModelCostData {
                input_tokens: 100,
                output_tokens: 50,
                cost_usd: 0.03,
                ..Default::default()
            });
            Some(RunCostData {
                input_tokens: 100,
                output_tokens: 50,
                total_cost_usd: 0.03,
                model_usage: usage,
                ..Default::default()
            })
        }
        fn is_available(&self) -> bool { false }
        fn get_version(&self) -> Option<String> { None }
        fn config_dir_name(&self) -> &str { ".fake" }
        fn command_instructions_subdir(&self) -> &str { "commands" }
        fn format_command_reference(&self, c: &str) -> String { format!("/{c}") }

        fn is_local_override(&self, _: &HashMap<String, serde_json::Value>) -> bool {
            self.local_override
        }
        fn effective_cost_model(&self, stage: &str, _: &HashMap<String, serde_json::Value>) -> String {
            self.override_model.clone().unwrap_or_else(|| stage.to_string())
        }
    }

    #[test]
    fn extract_cost_with_overrides_returns_none_when_no_cost() {
        let p = FakeProvider { local_override: false, override_model: None };
        let result = extract_cost_with_overrides(&p, "", "opus-4.6", &HashMap::new(), 5.0);
        assert!(result.is_none());
    }

    #[test]
    fn extract_cost_with_overrides_passes_through_for_non_local() {
        let p = FakeProvider { local_override: false, override_model: None };
        let cost = extract_cost_with_overrides(&p, "output", "opus-4.6", &HashMap::new(), 5.0).unwrap();
        assert_eq!(cost.total_cost_usd, 0.03);
        assert_eq!(cost.input_tokens, 100);
        assert!(cost.model_usage.contains_key("opus-4.6"));
    }

    #[test]
    fn extract_cost_with_overrides_zeroes_cost_for_local() {
        let p = FakeProvider { local_override: true, override_model: None };
        let cost = extract_cost_with_overrides(&p, "output", "opus-4.6", &HashMap::new(), 5.0).unwrap();
        assert_eq!(cost.total_cost_usd, 0.0, "local override should zero cost");
        assert_eq!(cost.input_tokens, 100, "tokens should be preserved");
        assert_eq!(cost.model_usage["opus-4.6"].cost_usd, 0.0);
        assert_eq!(cost.model_usage["opus-4.6"].input_tokens, 100);
    }

    #[test]
    fn extract_cost_with_overrides_uses_effective_model() {
        let p = FakeProvider { local_override: false, override_model: Some("llama3.2".into()) };
        let cost = extract_cost_with_overrides(&p, "output", "opus-4.6", &HashMap::new(), 5.0).unwrap();
        assert!(cost.model_usage.contains_key("llama3.2"), "should use effective model name");
        assert!(!cost.model_usage.contains_key("opus-4.6"), "should not use stage model name");
    }

    #[test]
    fn extract_cost_with_overrides_local_override_with_model_override() {
        let p = FakeProvider { local_override: true, override_model: Some("my-local".into()) };
        let cost = extract_cost_with_overrides(&p, "output", "opus-4.6", &HashMap::new(), 5.0).unwrap();
        assert_eq!(cost.total_cost_usd, 0.0);
        assert!(cost.model_usage.contains_key("my-local"));
        assert_eq!(cost.model_usage["my-local"].cost_usd, 0.0);
        assert_eq!(cost.model_usage["my-local"].input_tokens, 100);
    }
}
