//! Agent provider trait — the core abstraction for agent-agnostic execution.
//!
//! Every agent backend implements `AgentProvider` so that the orchestrator,
//! spawner, and runner can work with any agent without hard-coding
//! agent-specific logic.

use std::collections::HashMap;
use std::path::PathBuf;

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
    pub model: Option<String>,
    /// Opaque agent-specific configuration.
    /// Each provider knows its own keys (e.g. Claude uses "auth_token", "api_key", etc.).
    pub agent_config: HashMap<String, serde_json::Value>,
    /// Session identifier from a previous run, used to resume an agent session
    /// so that context is preserved across sequential invocations (e.g. implementation todos).
    pub session_id: Option<String>,
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

    /// Build agent-specific environment variables for the spawned process.
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

    /// Extract the session/thread identifier from raw agent output so that
    /// subsequent invocations can resume the same session (via `AgentRunConfig::session_id`).
    fn extract_session_id(&self, _output: &str) -> Option<String> {
        None
    }

    /// Brand color hex for UI display (e.g. "#da7756").
    fn brand_color(&self) -> Option<&str> {
        None
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
}

/// Extract cost with local-provider-aware model resolution and zero-cost handling.
///
/// Composes [`AgentProvider::effective_cost_model`], [`AgentProvider::extract_cost`],
/// and [`AgentProvider::is_local_override`] into a single call. All cost extraction
/// call sites should use this to ensure consistent handling of local overrides.
///
/// When a model override is active (effective model differs from stage model),
/// all `model_usage` entries are collapsed into a single entry keyed by the
/// override model name so that token tracking attributes usage to the model
/// the user actually configured.
pub fn extract_cost_with_overrides(
    provider: &dyn AgentProvider,
    stdout: &str,
    stage_model: &str,
    agent_config: &HashMap<String, serde_json::Value>,
    duration_secs: f64,
) -> Option<RunCostData> {
    let effective_model = provider.effective_cost_model(stage_model, agent_config);
    let mut cost = provider.extract_cost(stdout, &effective_model, duration_secs)?;

    let normalized_effective = super::cost::normalize_model_name(&effective_model);
    let normalized_stage = super::cost::normalize_model_name(stage_model);
    if normalized_effective != normalized_stage && !cost.model_usage.is_empty() {
        let mut merged = super::cost::ModelCostData::default();
        for data in cost.model_usage.values() {
            merged.input_tokens += data.input_tokens;
            merged.output_tokens += data.output_tokens;
            merged.cache_read_tokens += data.cache_read_tokens;
            merged.cache_creation_tokens += data.cache_creation_tokens;
            merged.cost_usd += data.cost_usd;
        }
        cost.model_usage.clear();
        cost.model_usage.insert(normalized_effective, merged);
    }

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
    fn default_extract_session_id_returns_none() {
        let stub = StubProvider;
        assert!(
            stub.extract_session_id(r#"{"type":"system","session_id":"abc"}"#).is_none(),
            "default trait impl should return None regardless of input"
        );
        assert!(stub.extract_session_id("").is_none());
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

    // ── re-keying tests (API returns different model names than override) ──

    /// Provider that always keys model_usage under a hardcoded API model name,
    /// simulating Claude Code where the API response determines the key
    /// regardless of the `model` parameter.
    #[derive(Debug)]
    struct ApiKeyedProvider {
        api_model_key: String,
        local_override: bool,
        override_model: Option<String>,
    }

    impl AgentProvider for ApiKeyedProvider {
        fn id(&self) -> &str { "api-keyed" }
        fn display_name(&self) -> &str { "ApiKeyed" }
        fn build_command(&self, _: &AgentRunConfig) -> (String, Vec<String>) {
            ("fake".into(), vec![])
        }
        fn build_env_vars(&self, _: &AgentRunConfig) -> Vec<(String, String)> { vec![] }
        fn extract_text(&self, o: &str) -> String { o.into() }
        fn extract_cost(&self, stdout: &str, _model: &str, _dur: f64) -> Option<RunCostData> {
            if stdout.is_empty() { return None; }
            let mut usage = HashMap::new();
            usage.insert(self.api_model_key.clone(), crate::agents::cost::ModelCostData {
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
    fn rekey_model_usage_when_api_returns_different_model_name() {
        let p = ApiKeyedProvider {
            api_model_key: "claude-opus-4-6".into(),
            local_override: true,
            override_model: Some("llama3.2".into()),
        };
        let cost = extract_cost_with_overrides(&p, "output", "opus-4.6", &HashMap::new(), 5.0).unwrap();
        assert!(
            cost.model_usage.contains_key("llama3.2"),
            "should re-key to override model; got keys: {:?}", cost.model_usage.keys().collect::<Vec<_>>()
        );
        assert!(!cost.model_usage.contains_key("opus-4.6"), "stage model key should be gone");
        assert!(!cost.model_usage.contains_key("claude-opus-4-6"), "API model key should be gone");
        assert_eq!(cost.model_usage["llama3.2"].input_tokens, 100);
        assert_eq!(cost.model_usage["llama3.2"].output_tokens, 50);
    }

    #[test]
    fn rekey_merges_multiple_api_model_entries() {
        let p = ApiKeyedProvider {
            api_model_key: "ignored".into(),
            local_override: false,
            override_model: Some("my-custom".into()),
        };
        // Manually build a provider that returns two model entries
        let mut usage = HashMap::new();
        usage.insert("model-a".to_string(), crate::agents::cost::ModelCostData {
            input_tokens: 60, output_tokens: 30, cost_usd: 0.01, ..Default::default()
        });
        usage.insert("model-b".to_string(), crate::agents::cost::ModelCostData {
            input_tokens: 40, output_tokens: 20, cost_usd: 0.02, ..Default::default()
        });
        let mut cost_data = RunCostData {
            input_tokens: 100, output_tokens: 50, total_cost_usd: 0.03,
            model_usage: usage, ..Default::default()
        };

        // Simulate what extract_cost_with_overrides does after extract_cost
        let effective = p.effective_cost_model("opus-4.6", &HashMap::new());
        let normalized_effective = crate::agents::cost::normalize_model_name(&effective);
        let normalized_stage = crate::agents::cost::normalize_model_name("opus-4.6");
        assert_ne!(normalized_effective, normalized_stage);

        let mut merged = crate::agents::cost::ModelCostData::default();
        for data in cost_data.model_usage.values() {
            merged.input_tokens += data.input_tokens;
            merged.output_tokens += data.output_tokens;
            merged.cost_usd += data.cost_usd;
        }
        cost_data.model_usage.clear();
        cost_data.model_usage.insert(normalized_effective.clone(), merged);

        assert_eq!(cost_data.model_usage.len(), 1);
        assert_eq!(cost_data.model_usage[&normalized_effective].input_tokens, 100);
        assert_eq!(cost_data.model_usage[&normalized_effective].output_tokens, 50);
        assert!((cost_data.model_usage[&normalized_effective].cost_usd - 0.03).abs() < 0.001);
    }

    #[test]
    fn no_rekey_when_effective_matches_stage() {
        let p = ApiKeyedProvider {
            api_model_key: "opus-4.6".into(),
            local_override: false,
            override_model: None,
        };
        let cost = extract_cost_with_overrides(&p, "output", "opus-4.6", &HashMap::new(), 5.0).unwrap();
        assert!(cost.model_usage.contains_key("opus-4.6"), "original key should be preserved");
        assert_eq!(cost.model_usage.len(), 1);
    }
}
