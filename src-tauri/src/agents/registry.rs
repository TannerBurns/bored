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
    use std::path::Path;

    /// Minimal stub provider for testing the registry.
    #[derive(Debug)]
    struct StubProvider {
        name: String,
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
            false
        }
        fn get_version(&self) -> Option<String> {
            None
        }
        fn install_hooks_for_run(
            &self,
            _repo_path: &Path,
            _hook_script_path: &str,
            _api_url: Option<&str>,
            _api_token: Option<&str>,
            _run_id: Option<&str>,
        ) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn register_and_get() {
        let mut registry = AgentRegistry::new();
        registry.register(Arc::new(StubProvider {
            name: "test-agent".to_string(),
        }));

        assert!(registry.get("test-agent").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn list_ids_returns_all_registered() {
        let mut registry = AgentRegistry::new();
        registry.register(Arc::new(StubProvider {
            name: "a".to_string(),
        }));
        registry.register(Arc::new(StubProvider {
            name: "b".to_string(),
        }));

        let mut ids = registry.list_ids();
        ids.sort();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn register_overwrites_existing() {
        let mut registry = AgentRegistry::new();
        registry.register(Arc::new(StubProvider {
            name: "same".to_string(),
        }));
        registry.register(Arc::new(StubProvider {
            name: "same".to_string(),
        }));

        assert_eq!(registry.list_ids().len(), 1);
    }

    #[test]
    fn default_is_empty() {
        let registry = AgentRegistry::default();
        assert!(registry.list_ids().is_empty());
    }
}
