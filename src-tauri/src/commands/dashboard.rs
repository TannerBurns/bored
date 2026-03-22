use std::sync::Arc;
use tauri::State;

use crate::db::dashboard::{
    AgentBreakdownEntry, DashboardSummary, DashboardTrendPoint, ModelBreakdownEntry,
};
use crate::db::Database;

#[tauri::command]
pub async fn get_dashboard_summary(
    days: Option<i32>,
    db: State<'_, Arc<Database>>,
) -> Result<DashboardSummary, String> {
    db.get_dashboard_summary(days).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_dashboard_trends(
    days: i32,
    utc_offset_minutes: Option<i32>,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<DashboardTrendPoint>, String> {
    db.get_dashboard_trends(days, utc_offset_minutes.unwrap_or(0))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_model_breakdown(
    days: Option<i32>,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<ModelBreakdownEntry>, String> {
    db.get_model_breakdown(days).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_agent_breakdown(
    days: Option<i32>,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<AgentBreakdownEntry>, String> {
    db.get_agent_breakdown(days).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn backfill_git_stats(
    db: State<'_, Arc<Database>>,
) -> Result<u32, String> {
    db.backfill_git_stats().map_err(|e| e.to_string())
}
