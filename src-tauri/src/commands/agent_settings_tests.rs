//! Tests for agent_settings module.

use super::agent_settings::*;
use std::collections::HashMap;

// ── ClaudeApiSettings serde tests ───────────────────────────────────

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
        model_override: Some("claude-opus-4-6".to_string()),
        ..Default::default()
    };
    let json = serde_json::to_string(&settings).unwrap();
    assert!(json.contains("authToken"));
    assert!(json.contains("apiKey"));
    assert!(json.contains("baseUrl"));
    assert!(json.contains("modelOverride"));
}

#[test]
fn claude_api_settings_serializes_cli_option_fields() {
    let settings = ClaudeApiSettings {
        thinking_enabled: Some(true),
        extended_context_enabled: Some(false),
        chrome_enabled: Some(true),
        ..Default::default()
    };
    let json = serde_json::to_string(&settings).unwrap();
    assert!(json.contains("thinkingEnabled"));
    assert!(json.contains("extendedContextEnabled"));
    assert!(json.contains("chromeEnabled"));
}

#[test]
fn claude_api_settings_deserializes_cli_option_fields() {
    let json = r#"{"thinkingEnabled":false,"extendedContextEnabled":true,"chromeEnabled":true}"#;
    let settings: ClaudeApiSettings = serde_json::from_str(json).unwrap();
    assert_eq!(settings.thinking_enabled, Some(false));
    assert_eq!(settings.extended_context_enabled, Some(true));
    assert_eq!(settings.chrome_enabled, Some(true));
}

#[test]
fn claude_api_settings_backward_compat_old_json_without_cli_options() {
    let json =
        r#"{"authToken":"tok","apiKey":"key","baseUrl":"https://x","modelOverride":"model"}"#;
    let settings: ClaudeApiSettings = serde_json::from_str(json).unwrap();
    assert!(settings.thinking_enabled.is_none());
    assert!(settings.extended_context_enabled.is_none());
    assert!(settings.chrome_enabled.is_none());
    assert_eq!(settings.auth_token, Some("tok".to_string()));
}

#[test]
fn claude_api_settings_cli_options_default_to_none() {
    let settings = ClaudeApiSettings::default();
    assert!(settings.thinking_enabled.is_none());
    assert!(settings.extended_context_enabled.is_none());
    assert!(settings.chrome_enabled.is_none());
}

#[test]
fn claude_api_settings_deserializes_from_camel_case() {
    let json =
        r#"{"authToken":"tok","apiKey":"key","baseUrl":"https://x","modelOverride":"model"}"#;
    let settings: ClaudeApiSettings = serde_json::from_str(json).unwrap();
    assert_eq!(settings.auth_token, Some("tok".to_string()));
    assert_eq!(settings.api_key, Some("key".to_string()));
    assert_eq!(settings.base_url, Some("https://x".to_string()));
    assert_eq!(settings.model_override, Some("model".to_string()));
}

// ── claude_settings_from_config helper ──────────────────────────────

#[test]
fn config_helper_empty_map_returns_all_none() {
    let config = HashMap::new();
    let settings = claude_settings_from_config(&config);
    assert!(settings.auth_token.is_none());
    assert!(settings.api_key.is_none());
    assert!(settings.base_url.is_none());
    assert!(settings.model_override.is_none());
    assert!(settings.thinking_enabled.is_none());
    assert!(settings.extended_context_enabled.is_none());
    assert!(settings.chrome_enabled.is_none());
}

#[test]
fn config_helper_reconstructs_all_fields() {
    let mut config = HashMap::new();
    config.insert("auth_token".to_string(), serde_json::json!("tok"));
    config.insert("api_key".to_string(), serde_json::json!("key"));
    config.insert("base_url".to_string(), serde_json::json!("https://x"));
    config.insert("model_override".to_string(), serde_json::json!("model"));
    config.insert("thinking_enabled".to_string(), serde_json::json!(true));
    config.insert("extended_context_enabled".to_string(), serde_json::json!(false));
    config.insert("chrome_enabled".to_string(), serde_json::json!(true));

    let settings = claude_settings_from_config(&config);
    assert_eq!(settings.auth_token, Some("tok".to_string()));
    assert_eq!(settings.api_key, Some("key".to_string()));
    assert_eq!(settings.base_url, Some("https://x".to_string()));
    assert_eq!(settings.model_override, Some("model".to_string()));
    assert_eq!(settings.thinking_enabled, Some(true));
    assert_eq!(settings.extended_context_enabled, Some(false));
    assert_eq!(settings.chrome_enabled, Some(true));
}

#[test]
fn config_helper_ignores_wrong_types() {
    let mut config = HashMap::new();
    config.insert("auth_token".to_string(), serde_json::json!(42));
    config.insert("thinking_enabled".to_string(), serde_json::json!("yes"));

    let settings = claude_settings_from_config(&config);
    assert!(settings.auth_token.is_none());
    assert!(settings.thinking_enabled.is_none());
}

// ── AgentSettingsManager tests ──────────────────────────────────────

#[test]
fn manager_default_returns_empty_config() {
    let manager = AgentSettingsManager::new();
    let config = manager.agent_config_for("claude");
    assert!(config.is_empty());
}

#[test]
fn manager_set_and_get_claude_settings() {
    let manager = AgentSettingsManager::new();
    manager.set_claude_settings_memory_only(ClaudeApiSettings {
        auth_token: Some("test-token".to_string()),
        api_key: None,
        base_url: Some("https://custom.api".to_string()),
        model_override: None,
        ..Default::default()
    });

    let loaded = manager.get_claude_settings();
    assert_eq!(loaded.auth_token, Some("test-token".to_string()));
    assert!(loaded.api_key.is_none());
    assert_eq!(loaded.base_url, Some("https://custom.api".to_string()));
}

#[test]
fn manager_agent_config_for_claude_returns_populated_map() {
    let manager = AgentSettingsManager::new();
    manager.set_claude_settings_memory_only(ClaudeApiSettings {
        auth_token: Some("tok".to_string()),
        thinking_enabled: Some(true),
        ..Default::default()
    });
    let config = manager.agent_config_for("claude");
    assert_eq!(
        config.get("auth_token").and_then(|v| v.as_str()),
        Some("tok")
    );
    assert_eq!(
        config.get("thinking_enabled").and_then(|v| v.as_bool()),
        Some(true)
    );
}

#[test]
fn manager_persist_without_path_succeeds_silently() {
    let manager = AgentSettingsManager::new();
    let mut config = HashMap::new();
    config.insert("key".to_string(), serde_json::json!("value"));
    let result = manager.set_agent_config_and_persist("no-path-agent", config);
    assert!(result.is_ok());
    assert_eq!(
        manager.agent_config_for("no-path-agent").get("key").and_then(|v| v.as_str()),
        Some("value"),
    );
}

#[test]
fn manager_persist_to_bad_path_returns_error() {
    let manager = AgentSettingsManager::new();
    manager.register_agent_settings_path(
        "bad-path",
        std::path::PathBuf::from("/nonexistent_dir_12345/settings.json"),
    );
    let mut config = HashMap::new();
    config.insert("key".to_string(), serde_json::json!("val"));
    let result = manager.set_agent_config_and_persist("bad-path", config);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Failed to save"));
}

#[test]
fn manager_agent_config_for_non_claude_returns_empty() {
    let manager = AgentSettingsManager::new();
    manager.set_claude_settings_memory_only(ClaudeApiSettings {
        auth_token: Some("tok".to_string()),
        ..Default::default()
    });
    let config = manager.agent_config_for("cursor");
    assert!(config.is_empty());
}

#[test]
fn manager_agent_config_for_unknown_agent_returns_empty() {
    let manager = AgentSettingsManager::new();
    let config = manager.agent_config_for("some-new-agent");
    assert!(config.is_empty());
}

#[test]
fn manager_generic_set_and_get() {
    let manager = AgentSettingsManager::new();
    let mut config = HashMap::new();
    config.insert("api_key".to_string(), serde_json::json!("my-key"));
    config.insert("option_a".to_string(), serde_json::json!(true));

    manager.set_agent_config("new-agent", config);

    let loaded = manager.agent_config_for("new-agent");
    assert_eq!(loaded.get("api_key").and_then(|v| v.as_str()), Some("my-key"));
    assert_eq!(loaded.get("option_a").and_then(|v| v.as_bool()), Some(true));
}

#[test]
fn manager_persist_and_load() {
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join(format!(
        "test_agent_settings_{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);

    let manager = AgentSettingsManager::new();
    manager.register_agent_settings_path("test-agent", path.clone());

    let mut config = HashMap::new();
    config.insert("key".to_string(), serde_json::json!("value"));
    manager
        .set_agent_config_and_persist("test-agent", config)
        .unwrap();

    assert!(path.exists());
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("value"));

    // Load into fresh manager
    let manager2 = AgentSettingsManager::new();
    manager2.register_agent_settings_path("test-agent", path.clone());
    let loaded = manager2.agent_config_for("test-agent");
    assert_eq!(loaded.get("key").and_then(|v| v.as_str()), Some("value"));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn manager_claude_persist_and_load() {
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join(format!(
        "test_claude_settings_{}.json",
        std::process::id()
    ));

    // Write settings in the old ClaudeApiSettings format
    let settings = ClaudeApiSettings {
        auth_token: Some("persisted-token".to_string()),
        api_key: Some("persisted-key".to_string()),
        base_url: None,
        model_override: Some("custom-model".to_string()),
        ..Default::default()
    };
    std::fs::write(&path, serde_json::to_string(&settings).unwrap()).unwrap();

    let manager = AgentSettingsManager::new_with_claude_settings(path.clone());
    let loaded = manager.get_claude_settings();

    assert_eq!(loaded.auth_token, Some("persisted-token".to_string()));
    assert_eq!(loaded.api_key, Some("persisted-key".to_string()));
    assert!(loaded.base_url.is_none());
    assert_eq!(loaded.model_override, Some("custom-model".to_string()));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn manager_shared_reads_fresh_settings() {
    let manager = AgentSettingsManager::new();
    let shared = manager.shared();

    manager.set_claude_settings_memory_only(ClaudeApiSettings {
        auth_token: Some("fresh".to_string()),
        ..Default::default()
    });

    let loaded = shared.get_claude_settings();
    assert_eq!(loaded.auth_token, Some("fresh".to_string()));
}

#[test]
fn manager_handles_missing_file() {
    let path = std::env::temp_dir().join(format!(
        "test_agent_settings_missing_{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);

    let manager = AgentSettingsManager::new_with_claude_settings(path);
    let settings = manager.get_claude_settings();
    assert!(settings.auth_token.is_none());
}

#[test]
fn manager_claude_save_load_roundtrip() {
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join(format!(
        "test_claude_roundtrip_{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);

    // Write initial file in the old camelCase format (simulating existing install)
    let original = ClaudeApiSettings {
        auth_token: Some("tok-original".to_string()),
        api_key: Some("key-original".to_string()),
        thinking_enabled: Some(true),
        ..Default::default()
    };
    std::fs::write(&path, serde_json::to_string(&original).unwrap()).unwrap();

    // Load, modify, save
    let manager = AgentSettingsManager::new_with_claude_settings(path.clone());
    let mut updated = manager.get_claude_settings();
    assert_eq!(updated.auth_token, Some("tok-original".to_string()));

    updated.auth_token = Some("tok-updated".to_string());
    manager.set_claude_settings(updated).unwrap();

    // Reload from disk in a fresh manager -- must survive the roundtrip
    let manager2 = AgentSettingsManager::new_with_claude_settings(path.clone());
    let reloaded = manager2.get_claude_settings();
    assert_eq!(reloaded.auth_token, Some("tok-updated".to_string()));
    assert_eq!(reloaded.api_key, Some("key-original".to_string()));
    assert_eq!(reloaded.thinking_enabled, Some(true));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn manager_handles_invalid_json() {
    let path = std::env::temp_dir().join(format!(
        "test_agent_settings_invalid_{}.json",
        std::process::id()
    ));
    std::fs::write(&path, "not valid json").unwrap();

    let manager = AgentSettingsManager::new_with_claude_settings(path.clone());
    let settings = manager.get_claude_settings();
    assert!(settings.auth_token.is_none());

    let _ = std::fs::remove_file(&path);
}
