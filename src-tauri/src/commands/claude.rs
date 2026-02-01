use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State};

use crate::agents::claude;

/// Claude API settings for overriding default API configuration
/// These settings are injected as environment variables when spawning Claude agents
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
}

/// Internal state containing both the settings and optional persistence path
struct ClaudeApiSettingsInner {
    settings: ClaudeApiSettings,
    persistence_path: Option<PathBuf>,
}

/// Managed state wrapper for ClaudeApiSettings with optional file persistence
pub struct ClaudeApiSettingsState(Arc<Mutex<ClaudeApiSettingsInner>>);

impl ClaudeApiSettingsState {
    /// Create a new state without persistence (for testing)
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(ClaudeApiSettingsInner {
            settings: ClaudeApiSettings::default(),
            persistence_path: None,
        })))
    }
    
    /// Create a new state with file persistence.
    /// Settings are loaded from the file if it exists.
    pub fn new_with_path(path: PathBuf) -> Self {
        let settings = if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    match serde_json::from_str(&content) {
                        Ok(s) => {
                            tracing::info!("Loaded Claude API settings from {}", path.display());
                            s
                        }
                        Err(e) => {
                            tracing::warn!("Failed to parse Claude API settings from {}: {}", path.display(), e);
                            ClaudeApiSettings::default()
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to read Claude API settings from {}: {}", path.display(), e);
                    ClaudeApiSettings::default()
                }
            }
        } else {
            tracing::debug!("No Claude API settings file at {}, using defaults", path.display());
            ClaudeApiSettings::default()
        };
        
        Self(Arc::new(Mutex::new(ClaudeApiSettingsInner {
            settings,
            persistence_path: Some(path),
        })))
    }
    
    pub fn get(&self) -> ClaudeApiSettings {
        self.0.lock().expect("claude api settings mutex poisoned").settings.clone()
    }
    
    /// Set settings in memory. Does not persist to disk.
    /// Use `set_and_persist` if disk persistence is required.
    pub fn set(&self, settings: ClaudeApiSettings) {
        let mut guard = self.0.lock().expect("claude api settings mutex poisoned");
        guard.settings = settings;
    }
    
    /// Set settings and persist to disk.
    /// Returns an error if persistence fails (settings are still updated in memory).
    pub fn set_and_persist(&self, settings: ClaudeApiSettings) -> Result<(), String> {
        let mut guard = self.0.lock().expect("claude api settings mutex poisoned");
        guard.settings = settings.clone();
        
        // Persist to file if we have a path
        if let Some(ref path) = guard.persistence_path {
            let json = serde_json::to_string_pretty(&settings)
                .map_err(|e| format!("Failed to serialize Claude API settings: {}", e))?;
            
            std::fs::write(path, json)
                .map_err(|e| format!("Failed to save Claude API settings to {}: {}", path.display(), e))?;
            
            tracing::debug!("Saved Claude API settings to {}", path.display());
        }
        
        Ok(())
    }
}

impl Default for ClaudeApiSettingsState {
    fn default() -> Self {
        Self::new()
    }
}

#[tauri::command]
pub async fn get_claude_api_settings(
    state: State<'_, ClaudeApiSettingsState>,
) -> Result<ClaudeApiSettings, String> {
    Ok(state.get())
}

#[tauri::command]
pub async fn set_claude_api_settings(
    settings: ClaudeApiSettings,
    state: State<'_, ClaudeApiSettingsState>,
) -> Result<(), String> {
    state.set_and_persist(settings)?;
    tracing::info!("Updated Claude API settings");
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeStatus {
    pub is_available: bool,
    pub version: Option<String>,
    pub user_hooks_installed: bool,
    pub hook_script_path: Option<String>,
}

#[tauri::command]
pub async fn get_claude_status(app: AppHandle) -> Result<ClaudeStatus, String> {
    let hook_script_path = get_hook_script_path(&app);
    
    Ok(ClaudeStatus {
        is_available: claude::is_claude_available(),
        version: claude::get_claude_version(),
        user_hooks_installed: claude::check_global_hooks_installed(),
        hook_script_path,
    })
}

const DEFAULT_API_URL: &str = "http://127.0.0.1:7432";

/// Get the API token, preferring the provided value, falling back to env var
fn get_api_token(provided: Option<String>) -> Option<String> {
    provided.or_else(|| std::env::var("AGENT_KANBAN_API_TOKEN").ok())
}

/// Get the API URL, preferring the provided value, falling back to env var
fn get_api_url(provided: Option<String>) -> String {
    provided
        .or_else(|| std::env::var("AGENT_KANBAN_API_URL").ok())
        .unwrap_or_else(|| DEFAULT_API_URL.to_string())
}

#[tauri::command]
pub async fn install_claude_hooks_user(
    hook_script_path: String,
    api_url: Option<String>,
    api_token: Option<String>,
) -> Result<(), String> {
    let url = get_api_url(api_url);
    let token = get_api_token(api_token);
    claude::install_user_hooks(&hook_script_path, Some(&url), token.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn install_claude_hooks_project(
    hook_script_path: String,
    project_path: String,
    api_url: Option<String>,
    api_token: Option<String>,
) -> Result<(), String> {
    let url = get_api_url(api_url);
    let token = get_api_token(api_token);
    claude::install_project_hooks(
        &PathBuf::from(project_path),
        &hook_script_path,
        Some(&url),
        token.as_deref(),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn install_claude_hooks_local(
    hook_script_path: String,
    project_path: String,
    api_url: Option<String>,
    api_token: Option<String>,
) -> Result<(), String> {
    let url = get_api_url(api_url);
    let token = get_api_token(api_token);
    claude::install_local_hooks(
        &PathBuf::from(project_path),
        &hook_script_path,
        Some(&url),
        token.as_deref(),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_claude_hooks_config(hook_script_path: String) -> Result<String, String> {
    let config = claude::generate_hooks_settings(&hook_script_path);
    serde_json::to_string_pretty(&config).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn check_claude_available() -> bool {
    claude::is_claude_available()
}

#[tauri::command]
pub async fn check_claude_project_hooks_installed(project_path: String) -> Result<bool, String> {
    Ok(claude::check_project_hooks_installed(&PathBuf::from(
        project_path,
    )))
}

#[tauri::command]
pub async fn get_claude_hook_script_path(app: AppHandle) -> Result<Option<String>, String> {
    Ok(get_hook_script_path(&app))
}

fn get_hook_script_path(app: &AppHandle) -> Option<String> {
    app.path_resolver()
        .app_data_dir()
        .map(|dir| dir.join("scripts").join("claude-hook.js"))
        .map(|p| p.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_api_settings_default() {
        let settings = ClaudeApiSettings::default();
        assert!(settings.auth_token.is_none());
        assert!(settings.api_key.is_none());
        assert!(settings.base_url.is_none());
        assert!(settings.model_override.is_none());
    }

    #[test]
    fn claude_api_settings_serializes_camel_case() {
        let settings = ClaudeApiSettings {
            auth_token: Some("token123".to_string()),
            api_key: Some("key456".to_string()),
            base_url: Some("https://api.example.com".to_string()),
            model_override: Some("claude-opus-4-5".to_string()),
        };
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("authToken"));
        assert!(json.contains("apiKey"));
        assert!(json.contains("baseUrl"));
        assert!(json.contains("modelOverride"));
    }

    #[test]
    fn claude_api_settings_deserializes_from_camel_case() {
        let json = r#"{"authToken":"tok","apiKey":"key","baseUrl":"https://x","modelOverride":"model"}"#;
        let settings: ClaudeApiSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.auth_token, Some("tok".to_string()));
        assert_eq!(settings.api_key, Some("key".to_string()));
        assert_eq!(settings.base_url, Some("https://x".to_string()));
        assert_eq!(settings.model_override, Some("model".to_string()));
    }

    #[test]
    fn claude_api_settings_state_get_set() {
        let state = ClaudeApiSettingsState::new();
        
        // Initially empty
        let initial = state.get();
        assert!(initial.auth_token.is_none());
        
        // Set new values
        state.set(ClaudeApiSettings {
            auth_token: Some("test-token".to_string()),
            api_key: None,
            base_url: Some("https://custom.api".to_string()),
            model_override: None,
        });
        
        // Verify update
        let updated = state.get();
        assert_eq!(updated.auth_token, Some("test-token".to_string()));
        assert!(updated.api_key.is_none());
        assert_eq!(updated.base_url, Some("https://custom.api".to_string()));
    }

    #[test]
    fn claude_api_settings_state_default() {
        let state = ClaudeApiSettingsState::default();
        let settings = state.get();
        assert!(settings.auth_token.is_none());
    }

    #[test]
    fn claude_api_settings_state_with_path_loads_existing() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join(format!("test_claude_settings_{}.json", std::process::id()));
        
        // Write settings to file
        let settings = ClaudeApiSettings {
            auth_token: Some("persisted-token".to_string()),
            api_key: Some("persisted-key".to_string()),
            base_url: None,
            model_override: Some("custom-model".to_string()),
        };
        std::fs::write(&path, serde_json::to_string(&settings).unwrap()).unwrap();
        
        // Load from file
        let state = ClaudeApiSettingsState::new_with_path(path.clone());
        let loaded = state.get();
        
        assert_eq!(loaded.auth_token, Some("persisted-token".to_string()));
        assert_eq!(loaded.api_key, Some("persisted-key".to_string()));
        assert!(loaded.base_url.is_none());
        assert_eq!(loaded.model_override, Some("custom-model".to_string()));
        
        // Cleanup
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn claude_api_settings_state_with_path_saves_on_set_and_persist() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join(format!("test_claude_settings_save_{}.json", std::process::id()));
        
        // Ensure file doesn't exist
        let _ = std::fs::remove_file(&path);
        
        let state = ClaudeApiSettingsState::new_with_path(path.clone());
        
        // Set new values with persistence
        let result = state.set_and_persist(ClaudeApiSettings {
            auth_token: Some("new-token".to_string()),
            api_key: None,
            base_url: Some("https://api.test.com".to_string()),
            model_override: None,
        });
        
        assert!(result.is_ok());
        
        // Verify file was written
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        let saved: ClaudeApiSettings = serde_json::from_str(&content).unwrap();
        
        assert_eq!(saved.auth_token, Some("new-token".to_string()));
        assert!(saved.api_key.is_none());
        assert_eq!(saved.base_url, Some("https://api.test.com".to_string()));
        
        // Cleanup
        let _ = std::fs::remove_file(&path);
    }
    
    #[test]
    fn claude_api_settings_state_set_does_not_persist() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join(format!("test_claude_settings_no_persist_{}.json", std::process::id()));
        
        // Ensure file doesn't exist
        let _ = std::fs::remove_file(&path);
        
        let state = ClaudeApiSettingsState::new_with_path(path.clone());
        
        // Use set() which should NOT persist
        state.set(ClaudeApiSettings {
            auth_token: Some("memory-only".to_string()),
            api_key: None,
            base_url: None,
            model_override: None,
        });
        
        // Settings should be in memory
        assert_eq!(state.get().auth_token, Some("memory-only".to_string()));
        
        // But file should NOT exist
        assert!(!path.exists());
    }
    
    #[test]
    fn claude_api_settings_state_set_and_persist_returns_error_on_write_failure() {
        // Use a path that doesn't exist and can't be created
        let path = std::path::PathBuf::from("/nonexistent_dir_12345/settings.json");
        
        let state = ClaudeApiSettingsState::new_with_path(path);
        
        let result = state.set_and_persist(ClaudeApiSettings {
            auth_token: Some("test".to_string()),
            api_key: None,
            base_url: None,
            model_override: None,
        });
        
        // Should return an error
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Failed to save Claude API settings"));
        
        // But settings should still be updated in memory
        assert_eq!(state.get().auth_token, Some("test".to_string()));
    }

    #[test]
    fn claude_api_settings_state_with_path_handles_missing_file() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join(format!("test_claude_settings_missing_{}.json", std::process::id()));
        
        // Ensure file doesn't exist
        let _ = std::fs::remove_file(&path);
        
        // Should not panic, should use defaults
        let state = ClaudeApiSettingsState::new_with_path(path.clone());
        let settings = state.get();
        
        assert!(settings.auth_token.is_none());
        assert!(settings.api_key.is_none());
        assert!(settings.base_url.is_none());
        assert!(settings.model_override.is_none());
    }

    #[test]
    fn claude_api_settings_state_with_path_handles_invalid_json() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join(format!("test_claude_settings_invalid_{}.json", std::process::id()));
        
        // Write invalid JSON
        std::fs::write(&path, "not valid json").unwrap();
        
        // Should not panic, should use defaults
        let state = ClaudeApiSettingsState::new_with_path(path.clone());
        let settings = state.get();
        
        assert!(settings.auth_token.is_none());
        
        // Cleanup
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn claude_status_serializes_correctly() {
        let status = ClaudeStatus {
            is_available: true,
            version: Some("1.0.0".to_string()),
            user_hooks_installed: false,
            hook_script_path: Some("/path/to/hook.js".to_string()),
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("isAvailable"));
        assert!(json.contains("userHooksInstalled"));
        assert!(json.contains("hookScriptPath"));
    }

    #[test]
    fn claude_status_serializes_with_none_values() {
        let status = ClaudeStatus {
            is_available: false,
            version: None,
            user_hooks_installed: false,
            hook_script_path: None,
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"isAvailable\":false"));
        assert!(json.contains("\"version\":null"));
        assert!(json.contains("\"hookScriptPath\":null"));
    }

    #[test]
    fn claude_status_deserializes_json_values() {
        let status = ClaudeStatus {
            is_available: true,
            version: Some("1.0.0".to_string()),
            user_hooks_installed: true,
            hook_script_path: Some("/usr/local/bin/hook.js".to_string()),
        };

        let json = serde_json::to_string(&status).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["isAvailable"], true);
        assert_eq!(parsed["version"], "1.0.0");
        assert_eq!(parsed["userHooksInstalled"], true);
        assert_eq!(parsed["hookScriptPath"], "/usr/local/bin/hook.js");
    }

    #[test]
    fn claude_status_debug_impl() {
        let status = ClaudeStatus {
            is_available: true,
            version: Some("1.0.0".to_string()),
            user_hooks_installed: false,
            hook_script_path: None,
        };

        let debug = format!("{:?}", status);
        assert!(debug.contains("ClaudeStatus"));
        assert!(debug.contains("is_available: true"));
    }

    #[test]
    fn claude_status_clone() {
        let status = ClaudeStatus {
            is_available: true,
            version: Some("1.0.0".to_string()),
            user_hooks_installed: true,
            hook_script_path: Some("/path".to_string()),
        };

        let cloned = status.clone();
        assert_eq!(cloned.is_available, status.is_available);
        assert_eq!(cloned.version, status.version);
        assert_eq!(cloned.user_hooks_installed, status.user_hooks_installed);
        assert_eq!(cloned.hook_script_path, status.hook_script_path);
    }

    #[test]
    fn get_api_token_uses_provided_value() {
        let result = get_api_token(Some("my-token".to_string()));
        assert_eq!(result, Some("my-token".to_string()));
    }

    #[test]
    fn get_api_token_returns_none_when_not_provided() {
        std::env::remove_var("AGENT_KANBAN_API_TOKEN");
        let result = get_api_token(None);
        assert!(result.is_none());
    }

    #[test]
    fn get_api_url_uses_provided_value() {
        let result = get_api_url(Some("http://custom:8080".to_string()));
        assert_eq!(result, "http://custom:8080");
    }

    #[test]
    fn get_api_url_uses_default_when_not_provided() {
        std::env::remove_var("AGENT_KANBAN_API_URL");
        let result = get_api_url(None);
        assert_eq!(result, DEFAULT_API_URL);
    }
}
