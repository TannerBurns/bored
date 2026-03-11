//! Agent registry — a central lookup table for agent providers.
//!
//! The registry is populated at startup and made available as Tauri managed
//! state so that any command or subsystem can resolve an agent by ID.

use std::collections::HashMap;
use std::sync::Arc;

use super::provider::AgentProvider;

/// Central registry that maps agent IDs to their provider implementations.
pub struct AgentRegistry {
    providers: HashMap<String, Arc<dyn AgentProvider>>,
}

impl AgentRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// Register a provider. Overwrites any existing provider with the same ID.
    pub fn register(&mut self, provider: Arc<dyn AgentProvider>) {
        self.providers.insert(provider.id().to_string(), provider);
    }

    /// Look up a provider by its ID (e.g. "cursor", "claude").
    pub fn get(&self, id: &str) -> Option<Arc<dyn AgentProvider>> {
        self.providers.get(id).cloned()
    }

    /// Return an iterator over all registered provider IDs.
    pub fn list_ids(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }

    /// Return all registered providers.
    pub fn providers(&self) -> Vec<Arc<dyn AgentProvider>> {
        self.providers.values().cloned().collect()
    }

    /// Return the ID of the first available agent, or the first registered
    /// agent if none are available. Returns an empty string only when the
    /// registry is completely empty.
    pub fn default_agent_id(&self) -> String {
        self.providers
            .values()
            .find(|p| p.is_available())
            .or_else(|| self.providers.values().next())
            .map(|p| p.id().to_string())
            .unwrap_or_default()
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::cost::RunCostData;
    use crate::agents::provider::AgentRunConfig;

    /// Minimal stub provider for testing the registry.
    #[derive(Debug)]
    struct StubProvider {
        name: String,
        available: bool,
    }

    impl StubProvider {
        fn unavailable(name: &str) -> Self {
            Self { name: name.to_string(), available: false }
        }
        fn available(name: &str) -> Self {
            Self { name: name.to_string(), available: true }
        }
    }

    impl AgentProvider for StubProvider {
        fn id(&self) -> &str {
            &self.name
        }
        fn display_name(&self) -> &str {
            &self.name
        }
        fn build_command(&self, _config: &AgentRunConfig) -> (String, Vec<String>) {
            (self.name.clone(), vec![])
        }
        fn build_env_vars(&self, _config: &AgentRunConfig) -> Vec<(String, String)> {
            vec![]
        }
        fn extract_text(&self, output: &str) -> String {
            output.to_string()
        }
        fn extract_cost(&self, _: &str, _: &str, _: f64) -> Option<RunCostData> {
            None
        }
        fn is_available(&self) -> bool {
            self.available
        }
        fn get_version(&self) -> Option<String> {
            None
        }
        fn config_dir_name(&self) -> &str {
            ".stub"
        }
        fn command_instructions_subdir(&self) -> &str {
            "commands"
        }
        fn format_command_reference(&self, command: &str) -> String {
            format!("/{}", command)
        }
        fn extract_session_id(&self, _output: &str) -> Option<String> {
            None
        }
    }

    #[test]
    fn register_and_get() {
        let mut registry = AgentRegistry::new();
        registry.register(Arc::new(StubProvider::unavailable("test-agent")));

        assert!(registry.get("test-agent").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn list_ids_returns_all_registered() {
        let mut registry = AgentRegistry::new();
        registry.register(Arc::new(StubProvider::unavailable("a")));
        registry.register(Arc::new(StubProvider::unavailable("b")));

        let mut ids = registry.list_ids();
        ids.sort();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn register_overwrites_existing() {
        let mut registry = AgentRegistry::new();
        registry.register(Arc::new(StubProvider::unavailable("same")));
        registry.register(Arc::new(StubProvider::unavailable("same")));

        assert_eq!(registry.list_ids().len(), 1);
    }

    #[test]
    fn default_is_empty() {
        let registry = AgentRegistry::default();
        assert!(registry.list_ids().is_empty());
    }

    mod default_agent_id_tests {
        use super::*;

        #[test]
        fn empty_registry_returns_empty_string() {
            let registry = AgentRegistry::new();
            assert_eq!(registry.default_agent_id(), "");
        }

        #[test]
        fn returns_available_agent() {
            let mut registry = AgentRegistry::new();
            registry.register(Arc::new(StubProvider::available("online")));
            assert_eq!(registry.default_agent_id(), "online");
        }

        #[test]
        fn prefers_available_over_unavailable() {
            let mut registry = AgentRegistry::new();
            registry.register(Arc::new(StubProvider::unavailable("offline")));
            registry.register(Arc::new(StubProvider::available("online")));
            assert_eq!(registry.default_agent_id(), "online");
        }

        #[test]
        fn falls_back_to_first_registered_when_none_available() {
            let mut registry = AgentRegistry::new();
            registry.register(Arc::new(StubProvider::unavailable("offline-a")));
            registry.register(Arc::new(StubProvider::unavailable("offline-b")));
            let id = registry.default_agent_id();
            assert!(!id.is_empty(), "should return a registered agent, not empty");
        }

        #[test]
        fn does_not_hardcode_claude_fallback() {
            let mut registry = AgentRegistry::new();
            registry.register(Arc::new(StubProvider::unavailable("some-new-agent")));
            let id = registry.default_agent_id();
            assert_ne!(id, "claude", "should not hardcode claude as fallback");
            assert_eq!(id, "some-new-agent");
        }
    }
}
