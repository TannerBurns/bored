//! Per-agent settings storage with optional file persistence.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::State;

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

    /// Returns the config map for a given agent, or empty if none stored.
    pub fn agent_config_for(&self, agent_id: &str) -> HashMap<String, serde_json::Value> {
        let guard = self.agents.lock().expect("agent settings mutex poisoned");
        guard
            .get(agent_id)
            .map(|e| e.config.clone())
            .unwrap_or_default()
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
