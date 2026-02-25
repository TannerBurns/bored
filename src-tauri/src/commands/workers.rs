//! Tauri commands for worker management.

use once_cell::sync::Lazy;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::State;

use crate::agents::command_templates::discover_commands;
use crate::agents::worker::{WorkerConfig, WorkerManager, WorkerStatus};
use crate::agents::AgentRegistry;
use crate::commands::agent_settings::AgentSettingsManager;
use crate::commands::runs::RunningAgents;
use crate::commands::workflow_settings::WorkflowSettingsState;
use crate::db::Database;

pub static WORKER_MANAGER: Lazy<WorkerManager> = Lazy::new(WorkerManager::new);

fn get_custom_commands_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?
        .join("custom-commands");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create custom commands dir: {}", e))?;
    Ok(dir)
}

/// Reject filenames containing path separators or `..` components so that
/// callers cannot traverse outside the intended directory.
fn safe_join(dir: &Path, filename: &str) -> Result<PathBuf, String> {
    if Path::new(filename).file_name() != Some(std::ffi::OsStr::new(filename)) {
        return Err(format!(
            "Invalid filename '{}': path traversal detected",
            filename
        ));
    }
    Ok(dir.join(filename))
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartWorkerRequest {
    pub agent_type: String,
    pub project_id: Option<String>,
    pub code_review_max_iterations: Option<usize>,
    pub stage_timeout_hours: Option<u32>,
    pub stage_max_retries: Option<u32>,
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
    app: tauri::AppHandle,
    input: StartWorkerRequest,
    db: State<'_, Arc<Database>>,
    agent_settings: State<'_, AgentSettingsManager>,
    running_agents: State<'_, RunningAgents>,
    workflow_settings_state: State<'_, WorkflowSettingsState>,
    registry: State<'_, AgentRegistry>,
) -> Result<StartWorkerResponse, String> {
    let StartWorkerRequest {
        agent_type,
        project_id,
        code_review_max_iterations,
        stage_timeout_hours,
        stage_max_retries,
    } = input;

    tracing::info!(
        "Starting worker: agent_type={}, project_id={:?}",
        agent_type,
        project_id
    );

    let agent_id = agent_type.clone();
    let provider = registry
        .get(&agent_id)
        .ok_or_else(|| format!("Unknown agent type: {}", agent_id))?;

    let agent_config = agent_settings.agent_config_for(&agent_id);

    let workflow_settings = Some(workflow_settings_state.shared());

    let config = WorkerConfig {
        agent_id,
        provider,
        project_id,
        poll_interval_secs: 10,
        heartbeat_interval_secs: 60,
        lock_duration_mins: 30,
        agent_timeout_secs: 3600,
        app_handle: Some(app.clone()),
        agent_config,
        code_review_max_iterations: code_review_max_iterations.unwrap_or(3),
        stage_timeout_secs: stage_timeout_hours.map(|h| h as u64 * 3600).unwrap_or(3600),
        stage_max_retries: stage_max_retries.unwrap_or(2),
        workflow_settings,
    };

    let cancel_handles = Some(running_agents.handles.clone());
    let worker_id = WORKER_MANAGER.start_worker(config, db.inner().clone(), cancel_handles);

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
    let now = chrono::Utc::now();

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
                .filter(|t| t.lock_expires_at.is_none_or(|exp| exp <= now))
                .count();
        }

        let all_tickets = db.get_tickets(&board.id, None).map_err(|e| e.to_string())?;
        in_progress_count += all_tickets
            .iter()
            .filter(|t| {
                t.locked_by_run_id.is_some() && t.lock_expires_at.is_some_and(|exp| exp > now)
            })
            .count();
    }

    Ok(WorkerQueueStatus {
        ready_count,
        in_progress_count,
        worker_count: WORKER_MANAGER.worker_count(),
    })
}

#[tauri::command]
pub async fn get_commands_path(
    _app: tauri::AppHandle,
) -> Result<Option<String>, String> {
    use crate::agents::command_templates;
    Ok(command_templates::get_bundled_commands_path().map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn get_available_commands(
    app: tauri::AppHandle,
) -> Result<Vec<String>, String> {
    use crate::agents::command_templates;
    let mut all_commands = std::collections::HashSet::new();

    // Bundled commands
    if let Some(bundled) = command_templates::get_bundled_commands_path() {
        for name in discover_commands(&bundled) {
            all_commands.insert(name);
        }
    }

    // Custom commands
    if let Ok(custom_dir) = get_custom_commands_dir(&app) {
        for name in discover_commands(&custom_dir) {
            all_commands.insert(name);
        }
    }

    let mut result: Vec<String> = all_commands.into_iter().collect();
    result.sort();
    Ok(result)
}

#[tauri::command]
pub async fn read_command_content(
    app: tauri::AppHandle,
    filename: String,
) -> Result<String, String> {
    if let Ok(custom_dir) = get_custom_commands_dir(&app) {
        let file_path = safe_join(&custom_dir, &filename)?;
        if file_path.exists() {
            return std::fs::read_to_string(&file_path)
                .map_err(|e| format!("Failed to read command file: {}", e));
        }
    }

    use crate::agents::command_templates;
    if let Some(bundled) = command_templates::get_bundled_commands_path() {
        let file_path = safe_join(&bundled, &filename)?;
        if file_path.exists() {
            return std::fs::read_to_string(&file_path)
                .map_err(|e| format!("Failed to read command file: {}", e));
        }
    }

    Err(format!("Command file not found: {}", filename))
}

#[tauri::command]
pub async fn save_custom_command(
    app: tauri::AppHandle,
    _id: String,
    filename: String,
    content: String,
) -> Result<(), String> {
    let custom_dir = get_custom_commands_dir(&app)?;
    let file_path = safe_join(&custom_dir, &filename)?;
    std::fs::write(&file_path, &content)
        .map_err(|e| format!("Failed to save command file: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn delete_custom_command(
    app: tauri::AppHandle,
    filename: String,
) -> Result<(), String> {
    let custom_dir = get_custom_commands_dir(&app)?;
    let file_path = safe_join(&custom_dir, &filename)?;
    if file_path.exists() {
        std::fs::remove_file(&file_path)
            .map_err(|e| format!("Failed to delete command file: {}", e))?;
    }
    Ok(())
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

    #[test]
    fn safe_join_allows_plain_filenames() {
        let dir = PathBuf::from("/tmp/commands");
        assert_eq!(
            safe_join(&dir, "cleanup.md").unwrap(),
            dir.join("cleanup.md")
        );
    }

    #[test]
    fn safe_join_rejects_parent_traversal() {
        let dir = PathBuf::from("/tmp/commands");
        assert!(safe_join(&dir, "../../../.bashrc").is_err());
        assert!(safe_join(&dir, "..").is_err());
        assert!(safe_join(&dir, "../secret.md").is_err());
    }

    #[test]
    fn safe_join_rejects_subdirectory_paths() {
        let dir = PathBuf::from("/tmp/commands");
        assert!(safe_join(&dir, "subdir/file.md").is_err());
    }

    #[test]
    fn safe_join_rejects_empty_and_absolute() {
        let dir = PathBuf::from("/tmp/commands");
        assert!(safe_join(&dir, "").is_err());
        assert!(safe_join(&dir, "/etc/passwd").is_err());
    }
}
