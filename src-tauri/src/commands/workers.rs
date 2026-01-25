//! Tauri commands for worker management.

use std::path::PathBuf;
use std::sync::Arc;
use once_cell::sync::Lazy;
use tauri::State;

use crate::agents::worker::{WorkerConfig, WorkerManager, WorkerStatus};
use crate::agents::validation::{ValidationResult, validate_worker_environment};
use crate::agents::{AgentKind, cursor, claude};
use crate::db::Database;

pub static WORKER_MANAGER: Lazy<WorkerManager> = Lazy::new(WorkerManager::new);

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartWorkerRequest {
    pub agent_type: String,
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartWorkerResponse {
    pub worker_id: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerQueueStatus {
    pub ready_count: usize,
    pub in_progress_count: usize,
    pub worker_count: usize,
}

#[tauri::command]
pub async fn start_worker(
    agent_type: String,
    project_id: Option<String>,
    db: State<'_, Arc<Database>>,
) -> Result<StartWorkerResponse, String> {
    tracing::info!(
        "Starting worker: agent_type={}, project_id={:?}",
        agent_type,
        project_id
    );

    let agent_kind = match agent_type.as_str() {
        "cursor" => AgentKind::Cursor,
        "claude" => AgentKind::Claude,
        _ => return Err(format!("Invalid agent type: {}", agent_type)),
    };

    let api_url = std::env::var("AGENT_KANBAN_API_URL").unwrap_or_else(|_| {
        format!(
            "http://127.0.0.1:{}",
            std::env::var("AGENT_KANBAN_API_PORT").unwrap_or_else(|_| "7432".to_string())
        )
    });
    let api_token = std::env::var("AGENT_KANBAN_API_TOKEN")
        .unwrap_or_else(|_| "default-token".to_string());

    let config = WorkerConfig {
        agent_type: agent_kind,
        project_id,
        api_url,
        api_token,
        ..Default::default()
    };

    let worker_id = WORKER_MANAGER.start_worker(config, db.inner().clone());

    tracing::info!("Worker started: {}", worker_id);

    Ok(StartWorkerResponse { worker_id })
}

#[tauri::command]
pub async fn stop_worker(worker_id: String) -> Result<bool, String> {
    tracing::info!("Stopping worker: {}", worker_id);
    let stopped = WORKER_MANAGER.stop_worker(&worker_id);
    if stopped {
        tracing::info!("Worker stopped: {}", worker_id);
    } else {
        tracing::warn!("Worker not found: {}", worker_id);
    }
    Ok(stopped)
}

#[tauri::command]
pub async fn stop_all_workers() -> Result<(), String> {
    tracing::info!("Stopping all workers");
    WORKER_MANAGER.stop_all().await;
    tracing::info!("All workers stopped");
    Ok(())
}

#[tauri::command]
pub async fn get_workers() -> Result<Vec<WorkerStatus>, String> {
    Ok(WORKER_MANAGER.get_all_status())
}

#[tauri::command]
pub async fn get_worker_queue_status(
    db: State<'_, Arc<Database>>,
) -> Result<WorkerQueueStatus, String> {
    let boards = db.get_boards().map_err(|e| e.to_string())?;
    
    let mut ready_count = 0;
    let mut in_progress_count = 0;

    for board in &boards {
        let columns = db.get_columns(&board.id).map_err(|e| e.to_string())?;

        if let Some(ready_col) = columns.iter().find(|c| c.name == "Ready") {
            let tickets = db
                .get_tickets(&board.id, Some(&ready_col.id))
                .map_err(|e| e.to_string())?;
            ready_count += tickets
                .iter()
                .filter(|t| {
                    t.lock_expires_at
                        .is_none_or(|exp| exp <= chrono::Utc::now())
                })
                .count();
        }

        if let Some(ip_col) = columns.iter().find(|c| c.name == "In Progress") {
            in_progress_count += db
                .get_tickets(&board.id, Some(&ip_col.id))
                .map_err(|e| e.to_string())?
                .len();
        }
    }

    Ok(WorkerQueueStatus {
        ready_count,
        in_progress_count,
        worker_count: WORKER_MANAGER.worker_count(),
    })
}

#[tauri::command]
pub async fn validate_worker(
    agent_type: String,
    repo_path: String,
) -> Result<ValidationResult, String> {
    let agent_kind = match agent_type.as_str() {
        "cursor" => AgentKind::Cursor,
        "claude" => AgentKind::Claude,
        _ => return Err(format!("Invalid agent type: {}", agent_type)),
    };

    let api_url = std::env::var("AGENT_KANBAN_API_URL").ok();
    let result = validate_worker_environment(
        agent_kind,
        &PathBuf::from(&repo_path),
        api_url.as_deref(),
    );

    Ok(result)
}

#[tauri::command]
pub async fn get_commands_path(app: tauri::AppHandle) -> Result<Option<String>, String> {
    if let Some(path) = cursor::get_bundled_commands_path_with_app(&app) {
        return Ok(Some(path.to_string_lossy().to_string()));
    }
    Ok(None)
}

#[tauri::command]
pub async fn get_available_commands(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    if let Some(path) = cursor::get_bundled_commands_path_with_app(&app) {
        return Ok(cursor::get_available_commands(&path));
    }
    Ok(vec![])
}

#[tauri::command]
pub async fn install_commands_to_project(
    app: tauri::AppHandle,
    agent_type: String,
    repo_path: String,
) -> Result<Vec<String>, String> {
    let commands_source = cursor::get_bundled_commands_path_with_app(&app)
        .ok_or_else(|| "Command templates not found".to_string())?;

    let repo = PathBuf::from(&repo_path);

    let installed = match agent_type.as_str() {
        "cursor" => cursor::install_commands(&repo, &commands_source),
        "claude" => claude::install_commands(&repo, &commands_source),
        _ => return Err(format!("Invalid agent type: {}", agent_type)),
    };

    installed.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn install_commands_to_user(
    app: tauri::AppHandle,
    agent_type: String,
) -> Result<Vec<String>, String> {
    let commands_source = cursor::get_bundled_commands_path_with_app(&app)
        .ok_or_else(|| "Command templates not found".to_string())?;

    let installed = match agent_type.as_str() {
        "cursor" => cursor::install_user_commands(&commands_source),
        "claude" => claude::install_user_commands(&commands_source),
        _ => return Err(format!("Invalid agent type: {}", agent_type)),
    };

    installed.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn check_commands_installed(
    agent_type: String,
    repo_path: String,
) -> Result<bool, String> {
    let repo = PathBuf::from(&repo_path);

    // Check both user-level and project-level commands
    let installed = match agent_type.as_str() {
        "cursor" => cursor::check_user_commands_installed() || cursor::check_project_commands_installed(&repo),
        "claude" => claude::check_user_commands_installed() || claude::check_project_commands_installed(&repo),
        _ => return Err(format!("Invalid agent type: {}", agent_type)),
    };

    Ok(installed)
}

#[tauri::command]
pub async fn check_user_commands_installed(agent_type: String) -> Result<bool, String> {
    let installed = match agent_type.as_str() {
        "cursor" => cursor::check_user_commands_installed(),
        "claude" => claude::check_user_commands_installed(),
        _ => return Err(format!("Invalid agent type: {}", agent_type)),
    };

    Ok(installed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_worker_request_deserializes() {
        let json = r#"{"agentType":"cursor","projectId":"p1"}"#;
        let req: StartWorkerRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.agent_type, "cursor");
        assert_eq!(req.project_id, Some("p1".to_string()));
    }

    #[test]
    fn start_worker_request_optional_project() {
        let json = r#"{"agentType":"claude"}"#;
        let req: StartWorkerRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.agent_type, "claude");
        assert!(req.project_id.is_none());
    }

    #[test]
    fn start_worker_response_serializes() {
        let resp = StartWorkerResponse {
            worker_id: "w123".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("workerId"));
        assert!(json.contains("w123"));
    }

    #[test]
    fn worker_queue_status_serializes() {
        let status = WorkerQueueStatus {
            ready_count: 5,
            in_progress_count: 2,
            worker_count: 1,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"readyCount\":5"));
        assert!(json.contains("\"inProgressCount\":2"));
        assert!(json.contains("\"workerCount\":1"));
    }
}
