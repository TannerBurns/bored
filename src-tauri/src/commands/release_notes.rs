use std::sync::Arc;
use tauri::State;

use crate::db::release_notes::ReleaseNote;
use crate::db::Database;

#[tauri::command]
pub async fn get_release_notes(
    version: String,
    db: State<'_, Arc<Database>>,
) -> Result<Option<ReleaseNote>, String> {
    db.get_release_notes(&version).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_all_release_notes(
    db: State<'_, Arc<Database>>,
) -> Result<Vec<ReleaseNote>, String> {
    db.get_all_release_notes().map_err(|e| e.to_string())
}
