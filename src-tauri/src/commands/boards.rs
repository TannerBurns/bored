use std::sync::Arc;
use tauri::State;

use crate::db::{Board, Column, Database};

#[tauri::command]
pub async fn get_boards(db: State<'_, Arc<Database>>) -> Result<Vec<Board>, String> {
    tracing::info!("Getting all boards");
    db.get_boards().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_columns(
    board_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<Column>, String> {
    tracing::info!("Getting columns for board: {}", board_id);
    db.get_columns(&board_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_board(
    name: String,
    db: State<'_, Arc<Database>>,
) -> Result<Board, String> {
    tracing::info!("Creating board: {}", name);
    db.create_board(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_board(
    board_id: String,
    name: String,
    db: State<'_, Arc<Database>>,
) -> Result<Board, String> {
    tracing::info!("Updating board {}: name={}", board_id, name);
    db.update_board(&board_id, &name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_board(
    board_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    tracing::info!("Deleting board: {}", board_id);
    db.delete_board(&board_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn factory_reset(
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    tracing::warn!("Factory reset requested");
    db.factory_reset().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn repair_scratchpads_table(
    db: State<'_, Arc<Database>>,
) -> Result<String, String> {
    tracing::warn!("Repairing scratchpads table CHECK constraint");
    db.repair_scratchpads_constraint().map_err(|e| e.to_string())
}