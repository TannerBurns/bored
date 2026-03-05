use std::sync::Arc;

use tauri::State;

use crate::agents;
use crate::db::Database;

/// Get aggregated cost for a ticket across all its runs.
#[tauri::command]
pub async fn get_ticket_cost(
    ticket_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<agents::AggregatedCost, String> {
    db.get_ticket_cost(&ticket_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn backfill_run_costs(
    db: State<'_, Arc<Database>>,
    registry: State<'_, Arc<crate::agents::registry::AgentRegistry>>,
) -> Result<u32, String> {
    let count = db.backfill_run_costs(&registry).map_err(|e| e.to_string())?;
    if count > 0 {
        tracing::debug!("Backfilled cost data for {} runs", count);
    }
    Ok(count)
}
