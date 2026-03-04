//! Unified agent commands — generic Tauri commands that dispatch through the
//! `AgentRegistry` instead of hard-coding agent-specific logic.

use std::sync::Arc;
use tauri::State;

use crate::agents::provider::AgentProvider;
use crate::agents::registry::AgentRegistry;

/// Unified status response for any agent.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatus {
    pub is_available: bool,
    pub version: Option<String>,
}

/// Look up a provider by ID, returning a user-facing error if not found.
fn resolve_provider(
    registry: &AgentRegistry,
    agent_id: &str,
) -> Result<Arc<dyn AgentProvider>, String> {
    registry
        .get(agent_id)
        .ok_or_else(|| format!("Unknown agent: {}", agent_id))
}

#[tauri::command]
pub async fn get_agent_status(
    agent_id: String,
    registry: State<'_, Arc<AgentRegistry>>,
) -> Result<AgentStatus, String> {
    let provider = resolve_provider(&registry, &agent_id)?;

    Ok(AgentStatus {
        is_available: provider.is_available(),
        version: provider.get_version(),
    })
}

#[tauri::command]
pub async fn check_agent_available(
    agent_id: String,
    registry: State<'_, Arc<AgentRegistry>>,
) -> Result<bool, String> {
    let provider = resolve_provider(&registry, &agent_id)?;
    Ok(provider.is_available())
}

/// Fetch the model list from `cursor agent --list-models`.
#[tauri::command]
pub async fn list_cursor_models() -> Result<crate::agents::cursor::models::CursorModelList, String>
{
    tokio::task::spawn_blocking(crate::agents::cursor::models::list_models)
        .await
        .map_err(|e| format!("Task join error: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::cost::RunCostData;
    use crate::agents::provider::AgentRunConfig;

    /// Stub provider for testing command dispatch logic.
    #[derive(Debug)]
    struct StubProvider {
        name: String,
        available: bool,
        version: Option<String>,
    }

    impl StubProvider {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                available: true,
                version: Some("1.0.0".to_string()),
            }
        }

        fn unavailable(name: &str) -> Self {
            Self {
                name: name.to_string(),
                available: false,
                version: None,
            }
        }
    }

    impl AgentProvider for StubProvider {
        fn id(&self) -> &str {
            &self.name
        }
        fn display_name(&self) -> &str {
            &self.name
        }
        fn build_command(&self, _: &AgentRunConfig) -> (String, Vec<String>) {
            (self.name.clone(), vec![])
        }
        fn build_env_vars(&self, _: &AgentRunConfig) -> Vec<(String, String)> {
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
            self.version.clone()
        }
        fn config_dir_name(&self) -> &str {
            ".stub"
        }
        fn command_instructions_subdir(&self) -> &str {
            "commands"
        }
        fn format_command_reference(&self, cmd: &str) -> String {
            format!("/{}", cmd)
        }
    }

    fn registry_with_stubs() -> AgentRegistry {
        let mut registry = AgentRegistry::new();
        registry.register(Arc::new(StubProvider::new("test-agent")));
        registry.register(Arc::new(StubProvider::unavailable("offline-agent")));
        registry
    }

    #[test]
    fn resolve_provider_returns_known_agent() {
        let registry = registry_with_stubs();
        let result = resolve_provider(&registry, "test-agent");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id(), "test-agent");
    }

    #[test]
    fn resolve_provider_errors_on_unknown_agent() {
        let registry = registry_with_stubs();
        let result = resolve_provider(&registry, "nonexistent");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Unknown agent: nonexistent");
    }

    #[test]
    fn resolve_provider_empty_registry_always_errors() {
        let registry = AgentRegistry::new();
        assert!(resolve_provider(&registry, "anything").is_err());
    }

    #[test]
    fn dispatch_is_available_returns_true_for_available_agent() {
        let registry = registry_with_stubs();
        let provider = resolve_provider(&registry, "test-agent").unwrap();
        assert!(provider.is_available());
    }

    #[test]
    fn dispatch_is_available_returns_false_for_offline_agent() {
        let registry = registry_with_stubs();
        let provider = resolve_provider(&registry, "offline-agent").unwrap();
        assert!(!provider.is_available());
    }

    #[test]
    fn dispatch_get_version_returns_value_for_available_agent() {
        let registry = registry_with_stubs();
        let provider = resolve_provider(&registry, "test-agent").unwrap();
        assert_eq!(provider.get_version(), Some("1.0.0".to_string()));
    }

    #[test]
    fn dispatch_get_version_returns_none_for_offline_agent() {
        let registry = registry_with_stubs();
        let provider = resolve_provider(&registry, "offline-agent").unwrap();
        assert!(provider.get_version().is_none());
    }
}
