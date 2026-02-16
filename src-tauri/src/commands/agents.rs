//! Unified agent commands — generic Tauri commands that dispatch through the
//! `AgentRegistry` instead of hard-coding agent-specific logic.

use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

use crate::agents::provider::AgentProvider;
use crate::agents::registry::AgentRegistry;

/// Unified status response for any agent.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatus {
    pub is_available: bool,
    pub version: Option<String>,
    pub global_hooks_installed: bool,
    pub hook_script_path: Option<String>,
}

const DEFAULT_API_URL: &str = "http://127.0.0.1:7432";

/// Look up a provider by ID, returning a user-facing error if not found.
fn resolve_provider(
    registry: &AgentRegistry,
    agent_id: &str,
) -> Result<Arc<dyn AgentProvider>, String> {
    registry
        .get(agent_id)
        .ok_or_else(|| format!("Unknown agent: {}", agent_id))
}

fn resolve_api(
    api_url: Option<String>,
    api_token: Option<String>,
) -> (String, Option<String>) {
    let url = api_url
        .or_else(|| std::env::var("AGENT_KANBAN_API_URL").ok())
        .unwrap_or_else(|| DEFAULT_API_URL.to_string());
    let token = api_token.or_else(|| std::env::var("AGENT_KANBAN_API_TOKEN").ok());
    (url, token)
}

fn hook_script_path_for(app: &AppHandle, script_name: &str) -> Option<String> {
    if script_name.is_empty() {
        return None;
    }
    app.path()
        .app_data_dir()
        .ok()
        .map(|dir| dir.join("scripts").join(script_name))
        .map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn get_agent_status(
    agent_id: String,
    app: AppHandle,
    registry: State<'_, AgentRegistry>,
) -> Result<AgentStatus, String> {
    let provider = resolve_provider(&registry, &agent_id)?;
    let hook_script_path = hook_script_path_for(&app, provider.hook_script_name());

    Ok(AgentStatus {
        is_available: provider.is_available(),
        version: provider.get_version(),
        global_hooks_installed: provider.check_hooks_installed_global(),
        hook_script_path,
    })
}

#[tauri::command]
pub async fn install_agent_hooks_global(
    agent_id: String,
    hook_script_path: String,
    api_url: Option<String>,
    api_token: Option<String>,
    registry: State<'_, AgentRegistry>,
) -> Result<(), String> {
    let provider = resolve_provider(&registry, &agent_id)?;
    let (url, token) = resolve_api(api_url, api_token);
    provider.install_hooks_global(&hook_script_path, Some(&url), token.as_deref())
}

#[tauri::command]
pub async fn install_agent_hooks_project(
    agent_id: String,
    hook_script_path: String,
    project_path: String,
    api_url: Option<String>,
    api_token: Option<String>,
    registry: State<'_, AgentRegistry>,
) -> Result<(), String> {
    let provider = resolve_provider(&registry, &agent_id)?;
    let (url, token) = resolve_api(api_url, api_token);
    provider.install_hooks_project(
        &PathBuf::from(project_path),
        &hook_script_path,
        Some(&url),
        token.as_deref(),
    )
}

#[tauri::command]
pub async fn get_agent_hooks_config(
    agent_id: String,
    hook_script_path: String,
    registry: State<'_, AgentRegistry>,
) -> Result<String, String> {
    let provider = resolve_provider(&registry, &agent_id)?;
    provider.generate_hooks_config_json(&hook_script_path)
}

#[tauri::command]
pub async fn check_agent_available(
    agent_id: String,
    registry: State<'_, AgentRegistry>,
) -> Result<bool, String> {
    let provider = resolve_provider(&registry, &agent_id)?;
    Ok(provider.is_available())
}

#[tauri::command]
pub async fn check_agent_project_hooks_installed(
    agent_id: String,
    project_path: String,
    registry: State<'_, AgentRegistry>,
) -> Result<bool, String> {
    let provider = resolve_provider(&registry, &agent_id)?;
    Ok(provider.check_hooks_installed_project(&PathBuf::from(project_path)))
}

#[tauri::command]
pub async fn get_agent_hook_script_path(
    agent_id: String,
    app: AppHandle,
    registry: State<'_, AgentRegistry>,
) -> Result<Option<String>, String> {
    let provider = resolve_provider(&registry, &agent_id)?;
    Ok(hook_script_path_for(&app, provider.hook_script_name()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::cost::RunCostData;
    use crate::agents::provider::AgentRunConfig;
    use std::path::Path;

    /// Stub provider for testing command dispatch logic.
    #[derive(Debug)]
    struct StubProvider {
        name: String,
        available: bool,
        version: Option<String>,
        hook_script: String,
    }

    impl StubProvider {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                available: true,
                version: Some("1.0.0".to_string()),
                hook_script: "stub-hook.js".to_string(),
            }
        }

        fn unavailable(name: &str) -> Self {
            Self {
                name: name.to_string(),
                available: false,
                version: None,
                hook_script: "".to_string(),
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
        fn install_hooks_for_run(
            &self,
            _: &Path,
            _: &str,
            _: Option<&str>,
            _: Option<&str>,
            _: Option<&str>,
        ) -> Result<(), String> {
            Ok(())
        }
        fn hook_script_name(&self) -> &str {
            &self.hook_script
        }
    }

    fn registry_with_stubs() -> AgentRegistry {
        let mut registry = AgentRegistry::new();
        registry.register(Arc::new(StubProvider::new("test-agent")));
        registry.register(Arc::new(StubProvider::unavailable("offline-agent")));
        registry
    }

    // ── resolve_provider ─────────────────────────────────────────────

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

    // ── resolve_api ──────────────────────────────────────────────────

    #[test]
    fn resolve_api_uses_explicit_url_over_env() {
        let (url, _) = resolve_api(Some("http://custom:9999".to_string()), None);
        assert_eq!(url, "http://custom:9999");
    }

    #[test]
    fn resolve_api_defaults_when_no_url_or_env() {
        // Temporarily unset the env var; explicit param = None forces fallback
        let saved = std::env::var("AGENT_KANBAN_API_URL").ok();
        std::env::remove_var("AGENT_KANBAN_API_URL");
        let (url, _) = resolve_api(None, None);
        // Restore
        if let Some(v) = saved {
            std::env::set_var("AGENT_KANBAN_API_URL", v);
        }
        assert_eq!(url, DEFAULT_API_URL);
    }

    #[test]
    fn resolve_api_passes_token_through() {
        let (_, token) = resolve_api(None, Some("my-secret".to_string()));
        assert_eq!(token, Some("my-secret".to_string()));
    }

    #[test]
    fn resolve_api_returns_none_token_when_omitted() {
        let saved = std::env::var("AGENT_KANBAN_API_TOKEN").ok();
        std::env::remove_var("AGENT_KANBAN_API_TOKEN");
        let (_, token) = resolve_api(None, None);
        if let Some(v) = saved {
            std::env::set_var("AGENT_KANBAN_API_TOKEN", v);
        }
        assert!(token.is_none());
    }

    // ── Provider dispatch via resolve_provider (simulates command logic) ──

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

    #[test]
    fn dispatch_hook_script_name_returns_expected() {
        let registry = registry_with_stubs();
        let provider = resolve_provider(&registry, "test-agent").unwrap();
        assert_eq!(provider.hook_script_name(), "stub-hook.js");
    }

    #[test]
    fn dispatch_hook_script_name_empty_for_offline_agent() {
        let registry = registry_with_stubs();
        let provider = resolve_provider(&registry, "offline-agent").unwrap();
        assert_eq!(provider.hook_script_name(), "");
    }

    #[test]
    fn dispatch_check_hooks_installed_global_defaults_false() {
        let registry = registry_with_stubs();
        let provider = resolve_provider(&registry, "test-agent").unwrap();
        assert!(!provider.check_hooks_installed_global());
    }

    #[test]
    fn dispatch_check_hooks_installed_project_defaults_false() {
        let registry = registry_with_stubs();
        let provider = resolve_provider(&registry, "test-agent").unwrap();
        assert!(!provider.check_hooks_installed_project(Path::new("/tmp")));
    }

    #[test]
    fn dispatch_generate_hooks_config_json_returns_default() {
        let registry = registry_with_stubs();
        let provider = resolve_provider(&registry, "test-agent").unwrap();
        let json = provider.generate_hooks_config_json("/path/to/hook.js").unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn dispatch_install_hooks_global_returns_default_error() {
        let registry = registry_with_stubs();
        let provider = resolve_provider(&registry, "test-agent").unwrap();
        let result = provider.install_hooks_global("/path", Some("http://localhost"), None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not supported"));
    }

    #[test]
    fn dispatch_install_hooks_project_returns_default_error() {
        let registry = registry_with_stubs();
        let provider = resolve_provider(&registry, "test-agent").unwrap();
        let result = provider.install_hooks_project(
            Path::new("/tmp"),
            "/path",
            Some("http://localhost"),
            None,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not supported"));
    }
}
