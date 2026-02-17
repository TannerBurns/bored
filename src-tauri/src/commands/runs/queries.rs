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
    tracing::debug!("Getting agent runs for ticket: {}", ticket_id);
    db.get_runs(&ticket_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_recent_runs(
    limit: Option<u32>,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<AgentRun>, String> {
    let limit = limit.unwrap_or(50);
    tracing::debug!("Getting recent {} agent runs", limit);
    db.get_recent_runs(limit).map_err(|e| e.to_string())
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
    tracing::debug!("Getting recent {} agent runs with context", limit);
    db.get_recent_runs_with_context(limit).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_agent_run(
    run_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<AgentRun, String> {
    tracing::debug!("Getting agent run: {}", run_id);
    db.get_run(&run_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_run_events(
    run_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<crate::db::AgentEvent>, String> {
    tracing::debug!("Getting events for run: {}", run_id);
    db.get_events(&run_id).map_err(|e| e.to_string())
}
