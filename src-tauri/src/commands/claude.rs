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

/// Managed state wrapper for ClaudeApiSettings
pub struct ClaudeApiSettingsState(pub Arc<Mutex<ClaudeApiSettings>>);

impl ClaudeApiSettingsState {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(ClaudeApiSettings::default())))
    }
    
    pub fn get(&self) -> ClaudeApiSettings {
        self.0.lock().expect("claude api settings mutex poisoned").clone()
    }
    
    pub fn set(&self, settings: ClaudeApiSettings) {
        let mut guard = self.0.lock().expect("claude api settings mutex poisoned");
        *guard = settings;
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
    state.set(settings);
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
