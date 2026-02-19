//! Tests for agent_settings module.

use super::agent_settings::*;
use std::collections::HashMap;

use crate::agents::claude::provider::ClaudeApiConfig;

// ── ClaudeApiConfig from_agent_config tests ─────────────────────────

#[test]
fn claude_config_empty_map_returns_all_none() {
    let config = HashMap::new();
    let api = ClaudeApiConfig::from_agent_config(&config);
    assert!(api.use_local_provider.is_none());
    assert!(api.auth_token.is_none());
    assert!(api.api_key.is_none());
    assert!(api.base_url.is_none());
    assert!(api.model_override.is_none());
    assert!(api.thinking_enabled.is_none());
    assert!(api.extended_context_enabled.is_none());
    assert!(api.chrome_enabled.is_none());
}

#[test]
fn claude_config_reads_snake_case_keys() {
    let mut config = HashMap::new();
    config.insert("use_local_provider".to_string(), serde_json::json!(true));
    config.insert("auth_token".to_string(), serde_json::json!("tok"));
    config.insert("api_key".to_string(), serde_json::json!("key"));
    config.insert("base_url".to_string(), serde_json::json!("https://x"));
    config.insert("model_override".to_string(), serde_json::json!("model"));
    config.insert("thinking_enabled".to_string(), serde_json::json!(true));
    config.insert("extended_context_enabled".to_string(), serde_json::json!(false));
    config.insert("chrome_enabled".to_string(), serde_json::json!(true));

    let api = ClaudeApiConfig::from_agent_config(&config);
    assert_eq!(api.use_local_provider, Some(true));
    assert_eq!(api.auth_token.as_deref(), Some("tok"));
    assert_eq!(api.api_key.as_deref(), Some("key"));
    assert_eq!(api.base_url.as_deref(), Some("https://x"));
    assert_eq!(api.model_override.as_deref(), Some("model"));
    assert_eq!(api.thinking_enabled, Some(true));
    assert_eq!(api.extended_context_enabled, Some(false));
    assert_eq!(api.chrome_enabled, Some(true));
}

#[test]
fn claude_config_reads_camel_case_keys() {
    let mut config = HashMap::new();
    config.insert("useLocalProvider".to_string(), serde_json::json!(false));
    config.insert("authToken".to_string(), serde_json::json!("tok"));
    config.insert("apiKey".to_string(), serde_json::json!("key"));
    config.insert("baseUrl".to_string(), serde_json::json!("https://x"));
    config.insert("modelOverride".to_string(), serde_json::json!("model"));
    config.insert("thinkingEnabled".to_string(), serde_json::json!(false));
    config.insert("extendedContextEnabled".to_string(), serde_json::json!(true));
    config.insert("chromeEnabled".to_string(), serde_json::json!(true));

    let api = ClaudeApiConfig::from_agent_config(&config);
    assert_eq!(api.use_local_provider, Some(false));
    assert_eq!(api.auth_token.as_deref(), Some("tok"));
    assert_eq!(api.api_key.as_deref(), Some("key"));
    assert_eq!(api.base_url.as_deref(), Some("https://x"));
    assert_eq!(api.model_override.as_deref(), Some("model"));
    assert_eq!(api.thinking_enabled, Some(false));
    assert_eq!(api.extended_context_enabled, Some(true));
    assert_eq!(api.chrome_enabled, Some(true));
}

#[test]
fn claude_config_snake_case_takes_precedence_over_camel_case() {
    let mut config = HashMap::new();
    config.insert("auth_token".to_string(), serde_json::json!("snake-wins"));
    config.insert("authToken".to_string(), serde_json::json!("camel-loses"));
    config.insert("thinking_enabled".to_string(), serde_json::json!(false));
    config.insert("thinkingEnabled".to_string(), serde_json::json!(true));

    let api = ClaudeApiConfig::from_agent_config(&config);
    assert_eq!(api.auth_token.as_deref(), Some("snake-wins"));
    assert_eq!(api.thinking_enabled, Some(false));
}

#[test]
fn claude_config_falls_back_to_camel_when_snake_missing() {
    let mut config = HashMap::new();
    config.insert("authToken".to_string(), serde_json::json!("camel-ok"));

    let api = ClaudeApiConfig::from_agent_config(&config);
    assert_eq!(api.auth_token.as_deref(), Some("camel-ok"));
    assert!(api.api_key.is_none());
}

#[test]
fn claude_config_ignores_wrong_types() {
    let mut config = HashMap::new();
    config.insert("use_local_provider".to_string(), serde_json::json!("yes"));
    config.insert("auth_token".to_string(), serde_json::json!(42));
    config.insert("thinking_enabled".to_string(), serde_json::json!("yes"));

    let api = ClaudeApiConfig::from_agent_config(&config);
    assert!(api.use_local_provider.is_none());
    assert!(api.auth_token.is_none());
    assert!(api.thinking_enabled.is_none());
}

#[test]
fn claude_config_to_agent_config_roundtrips() {
    let original = ClaudeApiConfig {
        use_local_provider: Some(true),
        auth_token: Some("tok".to_string()),
        api_key: Some("key".to_string()),
        base_url: None,
        model_override: Some("model".to_string()),
        thinking_enabled: Some(true),
        extended_context_enabled: Some(false),
        chrome_enabled: None,
    };
    let map = original.to_agent_config();
    let recovered = ClaudeApiConfig::from_agent_config(&map);
    assert_eq!(recovered.use_local_provider, original.use_local_provider);
    assert_eq!(recovered.auth_token, original.auth_token);
    assert_eq!(recovered.api_key, original.api_key);
    assert_eq!(recovered.base_url, original.base_url);
    assert_eq!(recovered.model_override, original.model_override);
    assert_eq!(recovered.thinking_enabled, original.thinking_enabled);
    assert_eq!(recovered.extended_context_enabled, original.extended_context_enabled);
    assert_eq!(recovered.chrome_enabled, original.chrome_enabled);
}

// ── AgentSettingsManager tests ──────────────────────────────────────

#[test]
fn manager_default_returns_empty_config() {
    let manager = AgentSettingsManager::new();
    let config = manager.agent_config_for("claude");
    assert!(config.is_empty());
}

#[test]
fn manager_set_and_get_agent_settings() {
    let manager = AgentSettingsManager::new();
    let config = ClaudeApiConfig {
        auth_token: Some("test-token".to_string()),
        base_url: Some("https://custom.api".to_string()),
        ..Default::default()
    }.to_agent_config();
    manager.set_agent_config("claude", config);

    let loaded = manager.agent_config_for("claude");
    let api = ClaudeApiConfig::from_agent_config(&loaded);
    assert_eq!(api.auth_token.as_deref(), Some("test-token"));
    assert!(api.api_key.is_none());
    assert_eq!(api.base_url.as_deref(), Some("https://custom.api"));
}

#[test]
fn manager_agent_config_populated_map() {
    let manager = AgentSettingsManager::new();
    let config = ClaudeApiConfig {
        auth_token: Some("tok".to_string()),
        thinking_enabled: Some(true),
        ..Default::default()
    }.to_agent_config();
    manager.set_agent_config("claude", config);

    let loaded = manager.agent_config_for("claude");
    assert_eq!(loaded.get("auth_token").and_then(|v| v.as_str()), Some("tok"));
    assert_eq!(loaded.get("thinking_enabled").and_then(|v| v.as_bool()), Some(true));
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
fn manager_agent_config_for_different_agent_returns_empty() {
    let manager = AgentSettingsManager::new();
    let config = ClaudeApiConfig {
        auth_token: Some("tok".to_string()),
        ..Default::default()
    }.to_agent_config();
    manager.set_agent_config("claude", config);

    let cursor_config = manager.agent_config_for("cursor");
    assert!(cursor_config.is_empty());
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
fn manager_loads_legacy_camel_case_file() {
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join(format!(
        "test_claude_settings_{}.json",
        std::process::id()
    ));

    // Write settings in legacy camelCase format
    let legacy_json = serde_json::json!({
        "authToken": "persisted-token",
        "apiKey": "persisted-key",
        "modelOverride": "custom-model"
    });
    std::fs::write(&path, serde_json::to_string(&legacy_json).unwrap()).unwrap();

    let manager = AgentSettingsManager::new();
    manager.register_agent_settings_path("claude", path.clone());

    // ClaudeApiConfig handles both key formats
    let loaded = manager.agent_config_for("claude");
    let api = ClaudeApiConfig::from_agent_config(&loaded);
    assert_eq!(api.auth_token.as_deref(), Some("persisted-token"));
    assert_eq!(api.api_key.as_deref(), Some("persisted-key"));
    assert!(api.base_url.is_none());
    assert_eq!(api.model_override.as_deref(), Some("custom-model"));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn manager_shared_reads_fresh_settings() {
    let manager = AgentSettingsManager::new();
    let shared = manager.shared();

    let config = ClaudeApiConfig {
        auth_token: Some("fresh".to_string()),
        ..Default::default()
    }.to_agent_config();
    manager.set_agent_config("claude", config);

    let loaded = shared.agent_config_for("claude");
    let api = ClaudeApiConfig::from_agent_config(&loaded);
    assert_eq!(api.auth_token.as_deref(), Some("fresh"));
}

#[test]
fn manager_handles_missing_file() {
    let path = std::env::temp_dir().join(format!(
        "test_agent_settings_missing_{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);

    let manager = AgentSettingsManager::new();
    manager.register_agent_settings_path("claude", path);
    let loaded = manager.agent_config_for("claude");
    assert!(loaded.is_empty());
}

#[test]
fn manager_save_load_roundtrip() {
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join(format!(
        "test_roundtrip_{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);

    let manager = AgentSettingsManager::new();
    manager.register_agent_settings_path("test-agent", path.clone());

    let config = ClaudeApiConfig {
        auth_token: Some("tok-original".to_string()),
        api_key: Some("key-original".to_string()),
        thinking_enabled: Some(true),
        ..Default::default()
    }.to_agent_config();
    manager.set_agent_config_and_persist("test-agent", config).unwrap();

    // Reload from disk in a fresh manager
    let manager2 = AgentSettingsManager::new();
    manager2.register_agent_settings_path("test-agent", path.clone());
    let reloaded = manager2.agent_config_for("test-agent");
    let api = ClaudeApiConfig::from_agent_config(&reloaded);
    assert_eq!(api.auth_token.as_deref(), Some("tok-original"));
    assert_eq!(api.api_key.as_deref(), Some("key-original"));
    assert_eq!(api.thinking_enabled, Some(true));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn manager_multiple_agents_independent() {
    let manager = AgentSettingsManager::new();

    let mut claude_config = HashMap::new();
    claude_config.insert("auth_token".to_string(), serde_json::json!("claude-tok"));
    manager.set_agent_config("claude", claude_config);

    let mut cursor_config = HashMap::new();
    cursor_config.insert("custom_key".to_string(), serde_json::json!("cursor-val"));
    manager.set_agent_config("cursor", cursor_config);

    let claude = manager.agent_config_for("claude");
    let cursor = manager.agent_config_for("cursor");
    assert_eq!(claude.get("auth_token").and_then(|v| v.as_str()), Some("claude-tok"));
    assert!(!claude.contains_key("custom_key"));
    assert_eq!(cursor.get("custom_key").and_then(|v| v.as_str()), Some("cursor-val"));
    assert!(!cursor.contains_key("auth_token"));
}

#[test]
fn manager_set_agent_config_replaces_entire_map() {
    let manager = AgentSettingsManager::new();

    let mut config1 = HashMap::new();
    config1.insert("key_a".to_string(), serde_json::json!("val_a"));
    config1.insert("key_b".to_string(), serde_json::json!("val_b"));
    manager.set_agent_config("agent", config1);

    let mut config2 = HashMap::new();
    config2.insert("key_c".to_string(), serde_json::json!("val_c"));
    manager.set_agent_config("agent", config2);

    let loaded = manager.agent_config_for("agent");
    assert!(!loaded.contains_key("key_a"), "old keys should be replaced");
    assert_eq!(loaded.get("key_c").and_then(|v| v.as_str()), Some("val_c"));
}

#[test]
fn manager_handles_invalid_json() {
    let path = std::env::temp_dir().join(format!(
        "test_agent_settings_invalid_{}.json",
        std::process::id()
    ));
    std::fs::write(&path, "not valid json").unwrap();

    let manager = AgentSettingsManager::new();
    manager.register_agent_settings_path("claude", path.clone());
    let loaded = manager.agent_config_for("claude");
    assert!(loaded.is_empty());

    let _ = std::fs::remove_file(&path);
}
