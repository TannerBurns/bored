use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{State, Window};

use crate::agents::{self, AgentKind, AgentRunConfig, LogLine, RunOutcome};
use crate::agents::spawner::CancelHandle;
use crate::db::models::{AgentRun, AgentType, CreateRun, RunStatus};
use crate::db::Database;

/// Shared state for tracking running agents
pub struct RunningAgents {
    pub handles: Arc<Mutex<HashMap<String, CancelHandle>>>,
}

impl RunningAgents {
    pub fn new() -> Self {
        Self {
            handles: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for RunningAgents {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRunInput {
    pub ticket_id: String,
    pub agent_type: String,
    pub repo_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentLogEvent {
    pub run_id: String,
    pub stream: String,
    pub content: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCompleteEvent {
    pub run_id: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub duration_secs: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentErrorEvent {
    pub run_id: String,
    pub error: String,
}

#[tauri::command]
pub async fn start_agent_run(
    window: Window,
    ticket_id: String,
    agent_type: String,
    repo_path: String,
    db: State<'_, Arc<Database>>,
    running_agents: State<'_, RunningAgents>,
) -> Result<String, String> {
    tracing::info!("Starting {} agent run for ticket: {}", agent_type, ticket_id);

    let (db_agent_type, agent_kind) = match agent_type.as_str() {
        "cursor" => (AgentType::Cursor, AgentKind::Cursor),
        "claude" => (AgentType::Claude, AgentKind::Claude),
        _ => return Err(format!("Invalid agent type: {}", agent_type)),
    };

    let ticket = db
        .get_ticket(&ticket_id)
        .map_err(|e| format!("Failed to get ticket: {}", e))?;

    let run = db
        .create_run(&CreateRun {
            ticket_id: ticket_id.clone(),
            agent_type: db_agent_type,
            repo_path: repo_path.clone(),
        })
        .map_err(|e| format!("Failed to create run: {}", e))?;

    let run_id = run.id.clone();

    let api_url = std::env::var("AGENT_KANBAN_API_URL")
        .unwrap_or_else(|_| format!("http://127.0.0.1:{}", 
            std::env::var("AGENT_KANBAN_API_PORT").unwrap_or_else(|_| "7432".to_string())
        ));
    let api_token = std::env::var("AGENT_KANBAN_API_TOKEN")
        .unwrap_or_else(|_| "default-token".to_string());

    let prompt = agents::prompt::generate_ticket_prompt(&ticket);

    let config = AgentRunConfig {
        kind: agent_kind,
        ticket_id: ticket_id.clone(),
        run_id: run_id.clone(),
        repo_path: std::path::PathBuf::from(&repo_path),
        prompt,
        timeout_secs: Some(3600), // 1 hour default
        api_url,
        api_token,
    };

    let db_clone = db.inner().clone();
    let run_id_for_task = run_id.clone();
    let run_id_for_cleanup = run_id.clone();
    let window_clone = window.clone();

    // Clone the Arc<Mutex<HashMap>> so we can move it into the async task
    let running_agents_handles = running_agents.handles.clone();

    tauri::async_runtime::spawn(async move {
        if let Err(e) = db_clone.update_run_status(&run_id_for_task, RunStatus::Running, None, None)
        {
            tracing::error!("Failed to update run status: {}", e);
        }

        let window_for_logs = window_clone.clone();
        let run_id_for_logs = run_id_for_task.clone();
        let on_log: Arc<agents::LogCallback> = Arc::new(Box::new(move |log: LogLine| {
            let event = AgentLogEvent {
                run_id: run_id_for_logs.clone(),
                stream: match log.stream {
                    agents::LogStream::Stdout => "stdout".to_string(),
                    agents::LogStream::Stderr => "stderr".to_string(),
                },
                content: log.content,
                timestamp: log.timestamp.to_rfc3339(),
            };
            let _ = window_for_logs.emit("agent-log", event);
        }));

        let config_clone = config.clone();
        let on_log_clone = on_log.clone();

        // Clone handles for use in the spawn callback
        let handles_for_spawn = running_agents_handles.clone();
        let run_id_for_spawn = run_id_for_task.clone();

        // Create the callback to store the cancel handle when the process spawns
        let on_spawn: agents::spawner::OnSpawnCallback = Box::new(move |cancel_handle| {
            handles_for_spawn
                .lock()
                .expect("running agents mutex poisoned")
                .insert(run_id_for_spawn.clone(), cancel_handle);
        });

        let result = tokio::task::spawn_blocking(move || {
            agents::spawner::run_agent_with_cancel_callback(
                config_clone,
                Some(on_log_clone),
                Some(on_spawn),
            )
        })
        .await;

        // Clean up the cancel handle now that the run is complete
        running_agents_handles
            .lock()
            .expect("running agents mutex poisoned")
            .remove(&run_id_for_cleanup);

        match result {
            Ok(Ok(agent_result)) => {
                let status = match agent_result.status {
                    RunOutcome::Success => RunStatus::Finished,
                    RunOutcome::Error => RunStatus::Error,
                    RunOutcome::Timeout => RunStatus::Error,
                    RunOutcome::Cancelled => RunStatus::Aborted,
                };
                let status_str = status.as_str().to_string();

                if let Err(e) = db_clone.update_run_status(
                    &agent_result.run_id,
                    status,
                    agent_result.exit_code,
                    agent_result.summary.as_deref(),
                ) {
                    tracing::error!("Failed to update run status: {}", e);
                }

                let event = AgentCompleteEvent {
                    run_id: agent_result.run_id,
                    status: status_str,
                    exit_code: agent_result.exit_code,
                    duration_secs: agent_result.duration_secs,
                };
                let _ = window_clone.emit("agent-complete", event);
            }
            Ok(Err(e)) => {
                tracing::error!("Agent run failed: {}", e);
                if let Err(db_err) = db_clone.update_run_status(
                    &run_id_for_task,
                    RunStatus::Error,
                    None,
                    Some(&e.to_string()),
                ) {
                    tracing::error!("Failed to update run status: {}", db_err);
                }

                let event = AgentErrorEvent {
                    run_id: run_id_for_task,
                    error: e.to_string(),
                };
                let _ = window_clone.emit("agent-error", event);
            }
            Err(e) => {
                tracing::error!("Task join error: {}", e);
                if let Err(db_err) = db_clone.update_run_status(
                    &run_id_for_task,
                    RunStatus::Error,
                    None,
                    Some(&e.to_string()),
                ) {
                    tracing::error!("Failed to update run status: {}", db_err);
                }

                let event = AgentErrorEvent {
                    run_id: run_id_for_task,
                    error: e.to_string(),
                };
                let _ = window_clone.emit("agent-error", event);
            }
        }
    });

    Ok(run_id)
}

#[tauri::command]
pub async fn cancel_agent_run(
    run_id: String,
    db: State<'_, Arc<Database>>,
    running_agents: State<'_, RunningAgents>,
) -> Result<(), String> {
    tracing::info!("Cancelling agent run: {}", run_id);

    // Try to cancel via handle
    if let Some(handle) = running_agents
        .handles
        .lock()
        .expect("running agents mutex poisoned")
        .get(&run_id)
    {
        handle.cancel();
    }

    // Update the status in the database
    db.update_run_status(&run_id, RunStatus::Aborted, None, Some("Cancelled by user"))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_agent_runs(
    ticket_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<AgentRun>, String> {
    tracing::info!("Getting agent runs for ticket: {}", ticket_id);
    db.get_runs(&ticket_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_agent_run(run_id: String, db: State<'_, Arc<Database>>) -> Result<AgentRun, String> {
    tracing::info!("Getting agent run: {}", run_id);
    db.get_run(&run_id).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

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

}
