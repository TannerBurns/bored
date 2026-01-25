//! Tauri commands for project management

use std::sync::Arc;
use tauri::State;

use crate::db::{CreateProject, Database, Project, ReadinessCheck, UpdateProject};

/// Get all projects
#[tauri::command]
pub async fn get_projects(db: State<'_, Arc<Database>>) -> Result<Vec<Project>, String> {
    tracing::info!("Getting all projects");
    db.get_projects().map_err(|e| e.to_string())
}

/// Get a project by ID
#[tauri::command]
pub async fn get_project(
    project_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Option<Project>, String> {
    tracing::info!("Getting project: {}", project_id);
    db.get_project(&project_id).map_err(|e| e.to_string())
}

/// Create a new project
#[tauri::command]
pub async fn create_project(
    input: CreateProject,
    db: State<'_, Arc<Database>>,
) -> Result<Project, String> {
    tracing::info!("Creating project: {} at {}", input.name, input.path);
    db.create_project(&input).map_err(|e| e.to_string())
}

/// Update a project
#[tauri::command]
pub async fn update_project(
    project_id: String,
    input: UpdateProject,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    tracing::info!("Updating project: {}", project_id);
    db.update_project(&project_id, &input)
        .map_err(|e| e.to_string())
}

/// Delete a project
#[tauri::command]
pub async fn delete_project(
    project_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    tracing::info!("Deleting project: {}", project_id);
    db.delete_project(&project_id).map_err(|e| e.to_string())
}

/// Set a board's default project
#[tauri::command]
pub async fn set_board_project(
    board_id: String,
    project_id: Option<String>,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    tracing::info!(
        "Setting board {} default project to {:?}",
        board_id,
        project_id
    );
    db.set_board_project(&board_id, project_id.as_deref())
        .map_err(|e| e.to_string())
}

/// Set a ticket's project override
#[tauri::command]
pub async fn set_ticket_project(
    ticket_id: String,
    project_id: Option<String>,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    tracing::info!(
        "Setting ticket {} project to {:?}",
        ticket_id,
        project_id
    );
    db.set_ticket_project(&ticket_id, project_id.as_deref())
        .map_err(|e| e.to_string())
}

/// Check if a ticket can be moved to Ready
#[tauri::command]
pub async fn check_ticket_readiness(
    ticket_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<ReadinessCheck, String> {
    tracing::info!("Checking readiness for ticket: {}", ticket_id);
    db.can_move_to_ready(&ticket_id).map_err(|e| e.to_string())
}

/// Update project hook installation status
#[tauri::command]
pub async fn update_project_hooks(
    project_id: String,
    cursor_installed: Option<bool>,
    claude_installed: Option<bool>,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    tracing::info!(
        "Updating hooks for project {}: cursor={:?}, claude={:?}",
        project_id,
        cursor_installed,
        claude_installed
    );
    db.update_project_hooks(&project_id, cursor_installed, claude_installed)
        .map_err(|e| e.to_string())
}

/// Browse for a directory (opens native file picker)
#[tauri::command]
pub async fn browse_for_directory() -> Result<Option<String>, String> {
    use tauri::api::dialog::blocking::FileDialogBuilder;

    tracing::info!("Opening directory picker");

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
        assert!(json.contains("projectId"));

        let no_project = ReadinessCheck::NoProject;
        let json = serde_json::to_string(&no_project).unwrap();
        assert!(json.contains("noProject"));
    }
}
