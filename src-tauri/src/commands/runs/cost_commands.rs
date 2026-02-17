use std::sync::Arc;

use tauri::State;

use crate::agents;
use crate::db::Database;

/// Get cost data for a single run.
#[tauri::command]
pub async fn get_run_cost(
    run_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Option<agents::RunCostData>, String> {
    tracing::debug!("Getting cost for run: {}", run_id);
    db.get_run_cost(&run_id).map_err(|e| e.to_string())
}

/// Get aggregated cost for a ticket across all its runs.
#[tauri::command]
pub async fn get_ticket_cost(
    ticket_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<agents::AggregatedCost, String> {
    tracing::debug!("Getting cost for ticket: {}", ticket_id);
    db.get_ticket_cost(&ticket_id).map_err(|e| e.to_string())
}

/// Get aggregated cost summary for an entire board.
#[tauri::command]
pub async fn get_board_cost_summary(
    board_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<agents::AggregatedCost, String> {
    tracing::debug!("Getting cost summary for board: {}", board_id);
    db.get_board_cost_summary(&board_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn backfill_run_costs(
    db: State<'_, Arc<Database>>,
    registry: State<'_, crate::agents::registry::AgentRegistry>,
) -> Result<u32, String> {
    let count = db.backfill_run_costs(&registry).map_err(|e| e.to_string())?;
    if count > 0 {
        tracing::debug!("Backfilled cost data for {} runs", count);
    }
    Ok(count)
}
