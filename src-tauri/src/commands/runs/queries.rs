use std::sync::Arc;

use tauri::State;

use crate::db::models::{AgentRun, AgentRunWithContext};
use crate::db::Database;

/// Clean up runs stuck in "Running" status (e.g. after a crash).
#[tauri::command]
pub async fn cleanup_stale_runs(db: State<'_, Arc<Database>>) -> Result<u32, String> {
    let count = db
        .cleanup_stale_running_status()
        .map_err(|e| format!("Failed to cleanup stale runs: {}", e))?;

    if count > 0 {
        tracing::info!("Cleaned up {} stale runs", count);
    }
    Ok(count)
}

#[tauri::command]
pub async fn get_agent_runs(
    ticket_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<AgentRun>, String> {
    db.get_runs(&ticket_id).map_err(|e| e.to_string())
}

/// Get recent runs with full context (board, project, ticket info).
/// This is the preferred method for the runs list view as it eliminates
/// client-side lookups and works across all boards.
#[tauri::command]
pub async fn get_recent_runs_with_context(
    limit: Option<u32>,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<AgentRunWithContext>, String> {
    let limit = limit.unwrap_or(50);
    db.get_recent_runs_with_context(limit).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_agent_run(
    run_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<AgentRun, String> {
    db.get_run(&run_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_run_events(
    run_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<crate::db::AgentEvent>, String> {
    db.get_events(&run_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_implementation_todos(
    run_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<crate::agents::orchestrator::TodoStatus>, String> {
    let run = db.get_run(&run_id).map_err(|e| e.to_string())?;

    let todos = run
        .metadata
        .and_then(|meta| meta.get("implementation_todos").cloned())
        .and_then(|raw| serde_json::from_value::<Vec<crate::agents::orchestrator::TodoStatus>>(raw).ok())
        .unwrap_or_default();

    Ok(todos)
}
