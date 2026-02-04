//! Event types for frontend emission during agent runs.

/// Log event for frontend emission
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentLogEvent {
    pub run_id: String,
    pub stream: String,
    pub content: String,
    pub timestamp: String,
}

/// Complete event for frontend emission
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCompleteEvent {
    pub run_id: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub duration_secs: f64,
}

/// Error event for frontend emission
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentErrorEvent {
    pub run_id: String,
    pub error: String,
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn agent_error_event_serializes() {
        let event = AgentErrorEvent {
            run_id: "run-1".to_string(),
            error: "Something went wrong".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("runId"));
        assert!(json.contains("error"));
    }
}
