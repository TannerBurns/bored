use std::sync::Arc;
use tauri::State;

use crate::db::{CreateProject, Database, Project, ReadinessCheck, UpdateProject};

#[tauri::command]
pub async fn get_projects(db: State<'_, Arc<Database>>) -> Result<Vec<Project>, String> {
    db.get_projects().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_project(
    project_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Option<Project>, String> {
    db.get_project(&project_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_project(
    input: CreateProject,
    db: State<'_, Arc<Database>>,
) -> Result<Project, String> {
    db.create_project(&input).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_project(
    project_id: String,
    input: UpdateProject,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    db.update_project(&project_id, &input)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_project(
    project_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    db.delete_project(&project_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_board_project(
    board_id: String,
    project_id: Option<String>,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    db.set_board_project(&board_id, project_id.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_ticket_project(
    ticket_id: String,
    project_id: Option<String>,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    db.set_ticket_project(&ticket_id, project_id.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn check_ticket_readiness(
    ticket_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<ReadinessCheck, String> {
    db.can_move_to_ready(&ticket_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_project_hooks(
    project_id: String,
    cursor_installed: Option<bool>,
    claude_installed: Option<bool>,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    db.update_project_hooks(&project_id, cursor_installed, claude_installed)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn browse_for_directory() -> Result<Option<String>, String> {
    use tauri::api::dialog::blocking::FileDialogBuilder;

    let path = FileDialogBuilder::new()
        .set_title("Select Project Directory")
        .pick_folder();

    Ok(path.map(|p| p.to_string_lossy().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_readiness_check_serialization() {
        let ready = ReadinessCheck::Ready {
            project_id: "proj-1".to_string(),
        };
        let json = serde_json::to_string(&ready).unwrap();
        assert!(json.contains("ready"));
        assert!(json.contains("proj-1"));

        let no_project = ReadinessCheck::NoProject(None);
        let json = serde_json::to_string(&no_project).unwrap();
        assert_eq!(json, r#"{"noProject":null}"#);
    }
}
