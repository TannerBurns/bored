//! Tauri commands for task queue management

use crate::db::models::{CreateTask, Task, TaskType};
use crate::db::tasks::TaskCounts;
use crate::db::Database;
use std::sync::Arc;
use tauri::{AppHandle, State};

/// Columns that should trigger a move back to Ready when a new task is added
const COMPLETED_COLUMNS: &[&str] = &["Done", "Review"];

/// Move ticket back to Ready if it's in a completed column (Done/Review)
/// This allows workers to pick up the ticket again for the new task
pub(crate) fn move_to_ready_if_completed(
    db: &Database,
    ticket_id: &str,
    app_handle: &AppHandle,
) -> Result<(), String> {
    let ticket = db.get_ticket(ticket_id).map_err(|e| e.to_string())?;

    let columns = db
        .get_columns(&ticket.board_id)
        .map_err(|e| e.to_string())?;

    let current_column = columns.iter().find(|c| c.id == ticket.column_id);

    let is_completed = current_column
        .map(|c| {
            COMPLETED_COLUMNS
                .iter()
                .any(|&name| c.name.eq_ignore_ascii_case(name))
        })
        .unwrap_or(false);

    if is_completed {
        if let Some(ready) = columns
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case("Ready"))
        {
            tracing::info!(
                "Moving ticket {} from {} back to Ready for new task",
                ticket_id,
                current_column.map(|c| c.name.as_str()).unwrap_or("unknown")
            );
            db.move_ticket(ticket_id, &ready.id)
                .map_err(|e| e.to_string())?;
            crate::tray::refresh_tray(app_handle);
        }
    }

    Ok(())
}

/// Get all tasks for a ticket
#[tauri::command]
pub fn get_tasks(db: State<'_, Arc<Database>>, ticket_id: String) -> Result<Vec<Task>, String> {
    db.get_tasks_for_ticket(&ticket_id)
        .map_err(|e| e.to_string())
}

/// Create a new custom task for a ticket
#[tauri::command]
pub fn create_task(
    db: State<'_, Arc<Database>>,
    app_handle: AppHandle,
    ticket_id: String,
    title: Option<String>,
    content: Option<String>,
) -> Result<Task, String> {
    let task = db
        .create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::Custom,
            title,
            content,
        })
        .map_err(|e| e.to_string())?;

    if let Err(e) = move_to_ready_if_completed(&db, &ticket_id, &app_handle) {
        tracing::warn!(
            "Failed to move ticket {} back to Ready after creating task {}: {}",
            ticket_id,
            task.id,
            e
        );
    }

    Ok(task)
}

/// Add a command-based task to a ticket (built-in or custom catalog command)
#[tauri::command]
pub fn add_command_task(
    db: State<'_, Arc<Database>>,
    app_handle: AppHandle,
    ticket_id: String,
    command_id: String,
    display_name: Option<String>,
) -> Result<Task, String> {
    let task_type = TaskType::Command(command_id);
    let title = display_name.unwrap_or_else(|| task_type.display_name());

    let task = db
        .create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type,
            title: Some(title),
            content: None,
        })
        .map_err(|e| e.to_string())?;

    if let Err(e) = move_to_ready_if_completed(&db, &ticket_id, &app_handle) {
        tracing::warn!(
            "Failed to move ticket {} back to Ready after adding command task {}: {}",
            ticket_id,
            task.id,
            e
        );
    }

    Ok(task)
}

/// Delete a task
#[tauri::command]
pub fn delete_task(db: State<'_, Arc<Database>>, task_id: String) -> Result<(), String> {
    db.delete_task(&task_id).map_err(|e| e.to_string())
}

/// Get task counts by status for a ticket
#[tauri::command]
pub fn get_task_counts(
    db: State<'_, Arc<Database>>,
    ticket_id: String,
) -> Result<TaskCounts, String> {
    db.get_task_counts(&ticket_id).map_err(|e| e.to_string())
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

    db.update_task(
        &task_id,
        &UpdateTask {
            title,
            content,
            status: None,
            run_id: None,
        },
    )
    .map_err(|e| e.to_string())
}

/// Reset a failed or completed task back to pending
///
/// This allows the task to be picked up by a worker again.
#[tauri::command]
pub fn reset_task(
    db: State<'_, Arc<Database>>,
    app_handle: AppHandle,
    task_id: String,
) -> Result<Task, String> {
    let task = db.reset_task(&task_id).map_err(|e| e.to_string())?;

    if let Err(e) = move_to_ready_if_completed(&db, &task.ticket_id, &app_handle) {
        tracing::warn!(
            "Failed to move ticket {} back to Ready after resetting task {}: {}",
            task.ticket_id,
            task_id,
            e
        );
    }

    Ok(task)
}

