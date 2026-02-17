//! Agent-agnostic settings manager with per-agent config storage.
//!
//! Each agent can have its own persisted settings (e.g. Claude has API keys,
//! model overrides, etc.). The `AgentSettingsManager` stores them all and
//! provides a uniform `agent_config_for(id)` interface used by the rest of
//! the codebase.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::State;

/// Claude API settings for overriding default API configuration.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeApiSettings {
    /// ANTHROPIC_AUTH_TOKEN - OAuth token for Claude Code
    pub auth_token: Option<String>,
    /// ANTHROPIC_API_KEY - API key for direct API access
    pub api_key: Option<String>,
    /// ANTHROPIC_BASE_URL - Custom API base URL
    pub base_url: Option<String>,
    /// Model override - bypasses normal model mapping, uses value directly for --model
    pub model_override: Option<String>,
    /// Enable extended thinking (--settings). Defaults to true when None.
    pub thinking_enabled: Option<bool>,
    /// Enable 1M token extended context (--betas). Defaults to false when None.
    /// Only works with API key users.
    pub extended_context_enabled: Option<bool>,
    /// Enable browser automation via Chrome (--chrome). Defaults to false when None.
    pub chrome_enabled: Option<bool>,
}

/// Reconstruct typed `ClaudeApiSettings` from a generic config map.
pub(crate) fn claude_settings_from_config(config: &HashMap<String, serde_json::Value>) -> ClaudeApiSettings {
    ClaudeApiSettings {
        auth_token: config.get("auth_token").and_then(|v| v.as_str()).map(|s| s.to_string()),
        api_key: config.get("api_key").and_then(|v| v.as_str()).map(|s| s.to_string()),
        base_url: config.get("base_url").and_then(|v| v.as_str()).map(|s| s.to_string()),
        model_override: config.get("model_override").and_then(|v| v.as_str()).map(|s| s.to_string()),
        thinking_enabled: config.get("thinking_enabled").and_then(|v| v.as_bool()),
        extended_context_enabled: config.get("extended_context_enabled").and_then(|v| v.as_bool()),
        chrome_enabled: config.get("chrome_enabled").and_then(|v| v.as_bool()),
    }
}

/// Settings for a single agent: opaque config map plus optional persistence path.
struct AgentSettingsEntry {
    config: HashMap<String, serde_json::Value>,
    persistence_path: Option<PathBuf>,
}

/// Per-agent settings storage with optional file persistence.
pub struct AgentSettingsManager {
    agents: Arc<Mutex<HashMap<String, AgentSettingsEntry>>>,
}

impl AgentSettingsManager {
    pub fn new() -> Self {
        Self {
            agents: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a manager pre-loaded with Claude settings from the given path.
    pub fn new_with_claude_settings(claude_settings_path: PathBuf) -> Self {
        let manager = Self::new();
        manager.load_claude_settings(claude_settings_path);
        manager
    }

    /// Returns the config map for a given agent, or empty if none stored.
    pub fn agent_config_for(&self, agent_id: &str) -> HashMap<String, serde_json::Value> {
        let guard = self.agents.lock().expect("agent settings mutex poisoned");
        guard
            .get(agent_id)
            .map(|e| e.config.clone())
            .unwrap_or_default()
    }

    pub fn set_agent_config(&self, agent_id: &str, config: HashMap<String, serde_json::Value>) {
        let mut guard = self.agents.lock().expect("agent settings mutex poisoned");
        let entry = guard.entry(agent_id.to_string()).or_insert_with(|| {
            AgentSettingsEntry {
                config: HashMap::new(),
                persistence_path: None,
            }
        });
        entry.config = config;
    }

    pub fn set_agent_config_and_persist(
        &self,
        agent_id: &str,
        config: HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        let mut guard = self.agents.lock().expect("agent settings mutex poisoned");
        let entry = guard.entry(agent_id.to_string()).or_insert_with(|| {
            AgentSettingsEntry {
                config: HashMap::new(),
                persistence_path: None,
            }
        });
        entry.config = config;

        if let Some(ref path) = entry.persistence_path {
            let json = serde_json::to_string_pretty(&entry.config)
                .map_err(|e| format!("Failed to serialize {} settings: {}", agent_id, e))?;
            std::fs::write(path, json).map_err(|e| {
                format!(
                    "Failed to save {} settings to {}: {}",
                    agent_id,
                    path.display(),
                    e
                )
            })?;
            tracing::debug!("Saved {} settings to {}", agent_id, path.display());
        }

        Ok(())
    }

    /// Register a persistence path for an agent, loading from disk if present.
    pub fn register_agent_settings_path(&self, agent_id: &str, path: PathBuf) {
        let config = if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(c) => {
                        tracing::info!("Loaded {} settings from {}", agent_id, path.display());
                        c
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to parse {} settings from {}: {}",
                            agent_id,
                            path.display(),
                            e
                        );
                        HashMap::new()
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        "Failed to read {} settings from {}: {}",
                        agent_id,
                        path.display(),
                        e
                    );
                    HashMap::new()
                }
            }
        } else {
            tracing::debug!("No {} settings file at {}, using defaults", agent_id, path.display());
            HashMap::new()
        };

        let mut guard = self.agents.lock().expect("agent settings mutex poisoned");
        guard.insert(
            agent_id.to_string(),
            AgentSettingsEntry {
                config,
                persistence_path: Some(path),
            },
        );
    }

    pub fn shared(&self) -> SharedAgentSettings {
        SharedAgentSettings(self.agents.clone())
    }

    /// Load Claude settings from disk into the registry.
    fn load_claude_settings(&self, path: PathBuf) {
        let settings: ClaudeApiSettings = if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(s) => {
                        tracing::info!("Loaded Claude API settings from {}", path.display());
                        s
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to parse Claude API settings from {}: {}",
                            path.display(),
                            e
                        );
                        ClaudeApiSettings::default()
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        "Failed to read Claude API settings from {}: {}",
                        path.display(),
                        e
                    );
                    ClaudeApiSettings::default()
                }
            }
        } else {
            tracing::debug!(
                "No Claude API settings file at {}, using defaults",
                path.display()
            );
            ClaudeApiSettings::default()
        };

        let config = crate::agents::claude::provider::ClaudeApiConfig::from(settings)
            .to_agent_config();

        let mut guard = self.agents.lock().expect("agent settings mutex poisoned");
        guard.insert(
            "claude".to_string(),
            AgentSettingsEntry {
                config,
                persistence_path: Some(path),
            },
        );
    }

    pub fn get_claude_settings(&self) -> ClaudeApiSettings {
        claude_settings_from_config(&self.agent_config_for("claude"))
    }

    /// Persist Claude settings to disk in the native camelCase serde format.
    pub fn set_claude_settings(&self, settings: ClaudeApiSettings) -> Result<(), String> {
        let config = crate::agents::claude::provider::ClaudeApiConfig::from(settings.clone())
            .to_agent_config();

        let mut guard = self.agents.lock().expect("agent settings mutex poisoned");
        let entry = guard.entry("claude".to_string()).or_insert_with(|| AgentSettingsEntry {
            config: HashMap::new(),
            persistence_path: None,
        });
        entry.config = config;

        if let Some(ref path) = entry.persistence_path {
            let json = serde_json::to_string_pretty(&settings)
                .map_err(|e| format!("Failed to serialize Claude API settings: {}", e))?;
            std::fs::write(path, json).map_err(|e| {
                format!("Failed to save Claude API settings to {}: {}", path.display(), e)
            })?;
            tracing::debug!("Saved Claude API settings to {}", path.display());
        }

        Ok(())
    }

    pub fn set_claude_settings_memory_only(&self, settings: ClaudeApiSettings) {
        let config = crate::agents::claude::provider::ClaudeApiConfig::from(settings)
            .to_agent_config();
        self.set_agent_config("claude", config);
    }
}

impl Default for AgentSettingsManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Cloneable handle for reading agent settings from background tasks.
#[derive(Clone)]
pub struct SharedAgentSettings(Arc<Mutex<HashMap<String, AgentSettingsEntry>>>);

impl SharedAgentSettings {
    pub fn agent_config_for(&self, agent_id: &str) -> HashMap<String, serde_json::Value> {
        let guard = self.0.lock().expect("agent settings mutex poisoned");
        guard
            .get(agent_id)
            .map(|e| e.config.clone())
            .unwrap_or_default()
    }

    pub fn get_claude_settings(&self) -> ClaudeApiSettings {
        claude_settings_from_config(&self.agent_config_for("claude"))
    }
}

impl std::fmt::Debug for SharedAgentSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedAgentSettings").finish()
    }
}

#[tauri::command]
pub async fn get_agent_settings(
    agent_id: String,
    state: State<'_, AgentSettingsManager>,
) -> Result<HashMap<String, serde_json::Value>, String> {
    Ok(state.agent_config_for(&agent_id))
}

#[tauri::command]
pub async fn set_agent_settings(
    agent_id: String,
    settings: HashMap<String, serde_json::Value>,
    state: State<'_, AgentSettingsManager>,
) -> Result<(), String> {
    state.set_agent_config_and_persist(&agent_id, settings)?;
    tracing::info!("Updated {} agent settings", agent_id);
    Ok(())
}

#[tauri::command]
pub async fn get_claude_api_settings(
    state: State<'_, AgentSettingsManager>,
) -> Result<ClaudeApiSettings, String> {
    Ok(state.get_claude_settings())
}

#[tauri::command]
pub async fn set_claude_api_settings(
    settings: ClaudeApiSettings,
    state: State<'_, AgentSettingsManager>,
) -> Result<(), String> {
    state.set_claude_settings(settings)?;
    tracing::info!("Updated Claude API settings");
    Ok(())
}

