use axum::{extract::State, Json};

use super::error::ApiResult;
use super::state::AppState;

pub async fn health() -> &'static str {
    "ok"
}

pub async fn health_detailed(State(state): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    let board_count = state.db.get_boards().map(|b| b.len()).unwrap_or(0);

    Ok(Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "database": "connected",
        "boardCount": board_count
    })))
}
