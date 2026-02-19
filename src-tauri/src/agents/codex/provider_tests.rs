use std::collections::HashMap;

use super::provider::*;
use crate::agents::provider::AgentProvider;

#[test]
fn provider_id_is_codex() {
    let p = CodexProvider::new();
    assert_eq!(p.id(), "codex");
}

#[test]
fn provider_display_name() {
    let p = CodexProvider::new();
    assert_eq!(p.display_name(), "Codex");
}

#[test]
fn provider_brand_color() {
    let p = CodexProvider::new();
    assert_eq!(p.brand_color(), Some("#0ea5e9"));
}

#[test]
fn provider_config_dir() {
    let p = CodexProvider::new();
    assert_eq!(p.config_dir_name(), ".codex");
}

#[test]
fn provider_model_passthrough() {
    let p = CodexProvider::new();
    assert_eq!(p.map_model_name("gpt-5.3-codex"), "gpt-5.3-codex");
}

#[test]
fn provider_available_models_not_empty() {
    let p = CodexProvider::new();
    let models = p.available_models();
    assert!(!models.is_empty());
    assert!(models.iter().any(|(id, _)| *id == "gpt-5.3-codex"));
}

#[test]
fn extract_text_from_item_completed() {
    let p = CodexProvider::new();
    let output = r#"{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"hello world"}}
{"type":"turn.completed","usage":{"input_tokens":100,"cached_input_tokens":50,"output_tokens":10}}"#;
    let text = p.extract_text(output);
    assert_eq!(text, "hello world");
}

#[test]
fn extract_text_ignores_reasoning_items() {
    let p = CodexProvider::new();
    let output = r#"{"type":"item.completed","item":{"id":"item_1","type":"reasoning","text":"thinking..."}}
{"type":"item.completed","item":{"id":"item_2","type":"agent_message","text":"actual response"}}"#;
    let text = p.extract_text(output);
    assert_eq!(text, "actual response");
}

#[test]
fn extract_text_multiple_messages() {
    let p = CodexProvider::new();
    let output = r#"{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"part one"}}
{"type":"item.completed","item":{"id":"item_2","type":"agent_message","text":"part two"}}"#;
    let text = p.extract_text(output);
    assert_eq!(text, "part one\npart two");
}

#[test]
fn extract_text_no_items_returns_raw() {
    let p = CodexProvider::new();
    let output = "raw text output";
    let text = p.extract_text(output);
    assert_eq!(text, "raw text output");
}

#[test]
fn extract_text_falls_back_to_command_output_when_no_agent_message() {
    let p = CodexProvider::new();
    let output = r#"{"type":"item.completed","item":{"id":"item_1","type":"command_execution","command":"echo hello","aggregated_output":"hello\n","exit_code":0,"status":"completed"}}
{"type":"turn.completed","usage":{"input_tokens":100,"cached_input_tokens":50,"output_tokens":10}}"#;
    let text = p.extract_text(output);
    assert_eq!(text, "hello\n");
}

#[test]
fn extract_text_prefers_agent_message_over_command_output() {
    let p = CodexProvider::new();
    let output = r#"{"type":"item.completed","item":{"id":"item_1","type":"command_execution","command":"ls","aggregated_output":"file.txt\n","exit_code":0,"status":"completed"}}
{"type":"item.completed","item":{"id":"item_2","type":"agent_message","text":"The answer is 42"}}"#;
    let text = p.extract_text(output);
    assert_eq!(text, "The answer is 42");
}

#[test]
fn extract_text_skips_empty_command_output() {
    let p = CodexProvider::new();
    let output = r#"{"type":"item.completed","item":{"id":"item_1","type":"command_execution","command":"true","aggregated_output":"","exit_code":0,"status":"completed"}}"#;
    let text = p.extract_text(output);
    assert_eq!(text, output, "should fall through to raw output when command output is empty");
}

#[test]
fn extract_text_multiple_command_outputs_joined() {
    let p = CodexProvider::new();
    let output = r#"{"type":"item.completed","item":{"id":"item_1","type":"command_execution","command":"echo a","aggregated_output":"aaa\n","exit_code":0,"status":"completed"}}
{"type":"item.completed","item":{"id":"item_2","type":"command_execution","command":"echo b","aggregated_output":"bbb\n","exit_code":0,"status":"completed"}}"#;
    let text = p.extract_text(output);
    assert_eq!(text, "aaa\n\nbbb\n");
}

#[test]
fn extract_text_skips_non_item_completed_events() {
    let p = CodexProvider::new();
    let output = r#"{"type":"thread.started","thread_id":"abc"}
{"type":"turn.started"}
{"type":"item.started","item":{"id":"item_1","type":"command_execution","command":"ls"}}
{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"result"}}
{"type":"turn.completed","usage":{"input_tokens":10,"output_tokens":5}}"#;
    let text = p.extract_text(output);
    assert_eq!(text, "result");
}

#[test]
fn extract_text_item_completed_without_item_field_skipped() {
    let p = CodexProvider::new();
    let output = r#"{"type":"item.completed"}
{"type":"item.completed","item":{"id":"item_2","type":"agent_message","text":"ok"}}"#;
    let text = p.extract_text(output);
    assert_eq!(text, "ok");
}

#[test]
fn extract_cost_from_turn_completed() {
    let p = CodexProvider::new();
    let output = r#"{"type":"turn.completed","usage":{"input_tokens":7448,"cached_input_tokens":6528,"output_tokens":74}}"#;
    let cost = p.extract_cost(output, "gpt-5.3-codex", 10.0);
    assert!(cost.is_some());
    let cost = cost.unwrap();
    assert_eq!(cost.input_tokens, 7448);
    assert_eq!(cost.output_tokens, 74);
    assert_eq!(cost.cache_read_tokens, 6528);
}

#[test]
fn extract_cost_multiple_turns_accumulates() {
    let p = CodexProvider::new();
    let output = r#"{"type":"turn.completed","usage":{"input_tokens":100,"cached_input_tokens":50,"output_tokens":10}}
{"type":"turn.completed","usage":{"input_tokens":200,"cached_input_tokens":100,"output_tokens":20}}"#;
    let cost = p.extract_cost(output, "gpt-5.3-codex", 10.0);
    assert!(cost.is_some());
    let cost = cost.unwrap();
    assert_eq!(cost.input_tokens, 300);
    assert_eq!(cost.output_tokens, 30);
    assert_eq!(cost.cache_read_tokens, 150);
}

#[test]
fn extract_cost_no_usage_falls_back_to_estimation() {
    let p = CodexProvider::new();
    let output = "some output without json";
    let cost = p.extract_cost(output, "gpt-5.3-codex", 10.0);
    assert!(cost.is_some());
}

#[test]
fn extract_cost_empty_output_returns_none() {
    let p = CodexProvider::new();
    let cost = p.extract_cost("", "gpt-5.3-codex", 0.0);
    assert!(cost.is_none());
}

// ── CodexApiConfig tests ───────────────────────────────────────────

#[test]
fn api_config_empty_map_returns_all_none() {
    let map = HashMap::new();
    let config = CodexApiConfig::from_agent_config(&map);
    assert!(config.oss_enabled.is_none());
    assert!(config.local_provider.is_none());
    assert!(config.model_override.is_none());
}

#[test]
fn api_config_reads_snake_case_keys() {
    let mut map = HashMap::new();
    map.insert("oss_enabled".into(), serde_json::json!(true));
    map.insert("local_provider".into(), serde_json::json!("ollama"));
    map.insert("model_override".into(), serde_json::json!("llama3.2"));

    let config = CodexApiConfig::from_agent_config(&map);
    assert_eq!(config.oss_enabled, Some(true));
    assert_eq!(config.local_provider.as_deref(), Some("ollama"));
    assert_eq!(config.model_override.as_deref(), Some("llama3.2"));
}

#[test]
fn api_config_reads_camel_case_keys() {
    let mut map = HashMap::new();
    map.insert("ossEnabled".into(), serde_json::json!(false));
    map.insert("localProvider".into(), serde_json::json!("lmstudio"));
    map.insert("modelOverride".into(), serde_json::json!("codestral"));

    let config = CodexApiConfig::from_agent_config(&map);
    assert_eq!(config.oss_enabled, Some(false));
    assert_eq!(config.local_provider.as_deref(), Some("lmstudio"));
    assert_eq!(config.model_override.as_deref(), Some("codestral"));
}

#[test]
fn api_config_snake_case_takes_precedence() {
    let mut map = HashMap::new();
    map.insert("oss_enabled".into(), serde_json::json!(true));
    map.insert("ossEnabled".into(), serde_json::json!(false));
    map.insert("local_provider".into(), serde_json::json!("ollama"));
    map.insert("localProvider".into(), serde_json::json!("lmstudio"));

    let config = CodexApiConfig::from_agent_config(&map);
    assert_eq!(config.oss_enabled, Some(true));
    assert_eq!(config.local_provider.as_deref(), Some("ollama"));
}

#[test]
fn api_config_wrong_type_returns_none() {
    let mut map = HashMap::new();
    map.insert("oss_enabled".into(), serde_json::json!("not-a-bool"));
    map.insert("local_provider".into(), serde_json::json!(42));
    map.insert("model_override".into(), serde_json::json!(true));

    let config = CodexApiConfig::from_agent_config(&map);
    assert!(config.oss_enabled.is_none());
    assert!(config.local_provider.is_none());
    assert!(config.model_override.is_none());
}

#[test]
fn api_config_default_trait() {
    let config = CodexApiConfig::default();
    assert!(config.oss_enabled.is_none());
    assert!(config.local_provider.is_none());
    assert!(config.model_override.is_none());
}
