use crate::commands::runs::*;

#[test]
fn running_agents_new_is_empty() {
    let ra = RunningAgents::new();
    assert!(ra.handles.lock().unwrap().is_empty());
}

#[test]
fn agent_log_event_serializes() {
    let event = AgentLogEvent {
        run_id: "run-1".to_string(),
        stream: "stdout".to_string(),
        content: "Hello".to_string(),
        timestamp: "2024-01-01T00:00:00Z".to_string(),
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("runId"));
    assert!(json.contains("stdout"));
}

#[test]
fn agent_complete_event_serializes() {
    let event = AgentCompleteEvent {
        run_id: "run-1".to_string(),
        status: "success".to_string(),
        exit_code: Some(0),
        duration_secs: 123.45,
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("durationSecs"));
    assert!(json.contains("exitCode"));
}

#[test]
fn running_agents_default_same_as_new() {
    let default = RunningAgents::default();
    let new = RunningAgents::new();
    assert!(default.handles.lock().unwrap().is_empty());
    assert!(new.handles.lock().unwrap().is_empty());
}

#[test]
fn agent_error_event_serializes() {
    let event = AgentErrorEvent {
        run_id: "run-1".to_string(),
        error: "Something went wrong".to_string(),
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("runId"));
    assert!(json.contains("error"));
    assert!(json.contains("Something went wrong"));
}

#[test]
fn start_run_input_deserializes() {
    let json = r#"{"ticketId":"t1","agentType":"cursor","repoPath":"/tmp"}"#;
    let input: StartRunInput = serde_json::from_str(json).unwrap();
    assert_eq!(input.ticket_id, "t1");
    assert_eq!(input.agent_type, "cursor");
    assert_eq!(input.repo_path, "/tmp");
}

#[test]
fn agent_complete_event_null_exit_code() {
    let event = AgentCompleteEvent {
        run_id: "run-1".to_string(),
        status: "timeout".to_string(),
        exit_code: None,
        duration_secs: 3600.0,
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"exitCode\":null"));
}

#[test]
fn stage_config_serializes_camel_case() {
    let config = StageConfig {
        enabled: true,
        model: "opus-4.6".to_string(),
    };
    let json = serde_json::to_string(&config).unwrap();
    assert_eq!(json, r#"{"enabled":true,"model":"opus-4.6"}"#);
}

#[test]
fn stage_config_deserializes_camel_case() {
    let json = r#"{"enabled":false,"model":"sonnet-4.5"}"#;
    let config: StageConfig = serde_json::from_str(json).unwrap();
    assert!(!config.enabled);
    assert_eq!(config.model, "sonnet-4.5");
}

#[test]
fn start_run_input_deserializes_with_stage_configs() {
    let json = r#"{
        "ticketId":"t1",
        "agentType":"cursor",
        "repoPath":"/tmp",
        "stageConfigs":{
            "plan":{"enabled":true,"model":"opus-4.6"},
            "codeReview":{"enabled":false,"model":"sonnet-4.5"}
        }
    }"#;
    let input: StartRunInput = serde_json::from_str(json).unwrap();
    let configs = input.stage_configs.unwrap();
    assert_eq!(configs.len(), 2);
    assert!(configs["plan"].enabled);
    assert_eq!(configs["plan"].model, "opus-4.6");
    assert!(!configs["codeReview"].enabled);
}

#[test]
fn start_run_input_deserializes_without_stage_configs() {
    let json = r#"{"ticketId":"t1","agentType":"cursor","repoPath":"/tmp"}"#;
    let input: StartRunInput = serde_json::from_str(json).unwrap();
    assert!(input.stage_configs.is_none());
}
