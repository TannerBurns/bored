//! Tauri commands for task queue management

use std::sync::Arc;
use tauri::State;
use crate::db::Database;
use crate::db::models::{Task, CreateTask, TaskType};
use crate::db::tasks::TaskCounts;

/// Columns that should trigger a move back to Ready when a new task is added
const COMPLETED_COLUMNS: &[&str] = &["Done", "Review"];

/// Move ticket back to Ready if it's in a completed column (Done/Review)
/// This allows workers to pick up the ticket again for the new task
fn move_to_ready_if_completed(db: &Database, ticket_id: &str) -> Result<(), String> {
    // Get the ticket to find its current column and board
    let ticket = db.get_ticket(ticket_id)
        .map_err(|e| e.to_string())?;
    
    // Get all columns for the board
    let columns = db.get_columns(&ticket.board_id)
        .map_err(|e| e.to_string())?;
    
    // Find the current column
    let current_column = columns.iter()
        .find(|c| c.id == ticket.column_id);
    
    // Check if ticket is in a completed column
    let is_completed = current_column
        .map(|c| COMPLETED_COLUMNS.iter().any(|&name| c.name.eq_ignore_ascii_case(name)))
        .unwrap_or(false);
    
    if is_completed {
        // Find the Ready column (where workers pick up tickets)
        if let Some(ready) = columns.iter().find(|c| c.name.eq_ignore_ascii_case("Ready")) {
            tracing::info!(
                "Moving ticket {} from {} back to Ready for new task",
                ticket_id,
                current_column.map(|c| c.name.as_str()).unwrap_or("unknown")
            );
            db.move_ticket(ticket_id, &ready.id)
                .map_err(|e| e.to_string())?;
        }
    }
    
    Ok(())
}

/// Get all tasks for a ticket
#[tauri::command]
pub fn get_tasks(
    db: State<'_, Arc<Database>>,
    ticket_id: String,
) -> Result<Vec<Task>, String> {
    db.get_tasks_for_ticket(&ticket_id)
        .map_err(|e| e.to_string())
}

/// Get a specific task by ID
#[tauri::command]
pub fn get_task(
    db: State<'_, Arc<Database>>,
    task_id: String,
) -> Result<Task, String> {
    db.get_task(&task_id)
        .map_err(|e| e.to_string())
}

/// Create a new custom task for a ticket
#[tauri::command]
pub fn create_task(
    db: State<'_, Arc<Database>>,
    ticket_id: String,
    title: Option<String>,
    content: Option<String>,
) -> Result<Task, String> {
    let task = db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::Custom,
            title,
            content,
        })
        .map_err(|e| e.to_string())?;
    
    // Move ticket back to Ready if it was in Done/Review
    move_to_ready_if_completed(&db, &ticket_id)?;
    
    Ok(task)
}

/// Add a preset task to a ticket
#[tauri::command]
pub fn add_preset_task(
    db: State<'_, Arc<Database>>,
    ticket_id: String,
    preset_type: String,
) -> Result<Task, String> {
    let task_type = match preset_type.as_str() {
        "sync_with_main" => TaskType::SyncWithMain,
        "add_tests" => TaskType::AddTests,
        "review_polish" => TaskType::ReviewPolish,
        "fix_lint" => TaskType::FixLint,
        _ => return Err(format!("Unknown preset type: {}", preset_type)),
    };
    
    // Use the display name as the title
    let title = task_type.display_name().to_string();
    
    let task = db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type,
            title: Some(title),
            content: None, // Preset tasks use their template content
        })
        .map_err(|e| e.to_string())?;
    
    // Move ticket back to Ready if it was in Done/Review
    move_to_ready_if_completed(&db, &ticket_id)?;
    
    Ok(task)
}

/// Delete a task
#[tauri::command]
pub fn delete_task(
    db: State<'_, Arc<Database>>,
    task_id: String,
) -> Result<(), String> {
    db.delete_task(&task_id)
        .map_err(|e| e.to_string())
}

/// Get the next pending task for a ticket
#[tauri::command]
pub fn get_next_pending_task(
    db: State<'_, Arc<Database>>,
    ticket_id: String,
) -> Result<Option<Task>, String> {
    db.get_next_pending_task(&ticket_id)
        .map_err(|e| e.to_string())
}

/// Check if a ticket has any pending tasks
#[tauri::command]
pub fn has_pending_tasks(
    db: State<'_, Arc<Database>>,
    ticket_id: String,
) -> Result<bool, String> {
    db.has_pending_tasks(&ticket_id)
        .map_err(|e| e.to_string())
}

/// Get task counts by status for a ticket
#[tauri::command]
pub fn get_task_counts(
    db: State<'_, Arc<Database>>,
    ticket_id: String,
) -> Result<TaskCounts, String> {
    db.get_task_counts(&ticket_id)
        .map_err(|e| e.to_string())
}

/// Update a task's title or content
#[tauri::command]
pub fn update_task(
    db: State<'_, Arc<Database>>,
    task_id: String,
    title: Option<String>,
    content: Option<String>,
) -> Result<Task, String> {
    use crate::db::models::UpdateTask;
    
    db.update_task(&task_id, &UpdateTask {
            title,
            content,
            status: None,
            run_id: None,
        })
        .map_err(|e| e.to_string())
}

/// Reset a failed or completed task back to pending
/// 
/// This allows the task to be picked up by a worker again.
#[tauri::command]
pub fn reset_task(
    db: State<'_, Arc<Database>>,
    task_id: String,
) -> Result<Task, String> {
    let task = db.reset_task(&task_id)
        .map_err(|e| e.to_string())?;
    
    // Move ticket back to Ready if it was in Done/Review
    move_to_ready_if_completed(&db, &task.ticket_id)?;
    
    Ok(task)
}

/// Get all available preset task types
#[tauri::command]
pub fn get_preset_types() -> Vec<PresetTaskInfo> {
    vec![
        PresetTaskInfo {
            type_name: "sync_with_main".to_string(),
            display_name: "Sync with Main".to_string(),
            description: "Merge the latest changes from main branch".to_string(),
        },
        PresetTaskInfo {
            type_name: "add_tests".to_string(),
            display_name: "Add Tests".to_string(),
            description: "Add test coverage for recent changes".to_string(),
        },
        PresetTaskInfo {
            type_name: "review_polish".to_string(),
            display_name: "Review & Polish".to_string(),
            description: "Review code quality and apply best practices".to_string(),
        },
        PresetTaskInfo {
            type_name: "fix_lint".to_string(),
            display_name: "Fix Lint Errors".to_string(),
            description: "Fix all linting and type checking errors".to_string(),
        },
    ]
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetTaskInfo {
    pub type_name: String,
    pub display_name: String,
    pub description: String,
}
