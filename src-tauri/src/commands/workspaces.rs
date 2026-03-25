use std::sync::Arc;
use tauri::State;

use crate::db::models::Workspace;
use crate::db::Database;

#[tauri::command]
pub async fn get_workspaces(db: State<'_, Arc<Database>>) -> Result<Vec<Workspace>, String> {
    db.get_workspaces().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_workspace(
    workspace_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Option<Workspace>, String> {
    db.get_workspace(&workspace_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_workspace(
    name: String,
    project_ids: Vec<String>,
    db: State<'_, Arc<Database>>,
) -> Result<Workspace, String> {
    let workspace = db.create_workspace(&name).map_err(|e| e.to_string())?;

    for (i, project_id) in project_ids.iter().enumerate() {
        db.add_project_to_workspace(&workspace.id, project_id, i as i32)
            .map_err(|e| e.to_string())?;
    }

    db.get_workspace(&workspace.id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Failed to retrieve created workspace".to_string())
}

#[tauri::command]
pub async fn update_workspace(
    workspace_id: String,
    name: String,
    db: State<'_, Arc<Database>>,
) -> Result<Workspace, String> {
    db.update_workspace(&workspace_id, &name)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_workspace(
    workspace_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    db.delete_workspace(&workspace_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_project_to_workspace(
    workspace_id: String,
    project_id: String,
    position: i32,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    db.add_project_to_workspace(&workspace_id, &project_id, position)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_project_from_workspace(
    workspace_id: String,
    project_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    db.remove_project_from_workspace(&workspace_id, &project_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_workspace_projects(
    workspace_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<crate::db::models::Project>, String> {
    db.get_workspace_projects(&workspace_id)
        .map_err(|e| e.to_string())
}
