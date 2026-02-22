use serde::Deserialize;
use std::sync::Arc;
use tauri::{AppHandle, State};

use crate::db::{
    AuthorType, Comment, CreateComment, CreateTicket, Database, EpicProgress, Priority,
    Ticket, UpdateTicket, WorkflowType,
};
use crate::db::models::{TaskStatus, TaskType, UpdateTask};

/// Input struct for creating tickets via Tauri command.
/// Allows setting is_epic and epic_id at creation time.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTicketInput {
    pub board_id: String,
    pub column_id: String,
    pub title: String,
    pub description_md: String,
    pub priority: Priority,
    pub labels: Vec<String>,
    pub project_id: Option<String>,
    #[serde(default)]
    pub workflow_type: Option<WorkflowType>,
    pub model: Option<String>,
    /// Optional pre-defined branch name (if not provided, will be AI-generated on first run)
    pub branch_name: Option<String>,
    /// Whether to create this ticket as an epic
    #[serde(default)]
    pub is_epic: bool,
    /// The parent epic ID (when creating a child ticket)
    pub epic_id: Option<String>,
}

/// Input struct for updating tickets via Tauri command.
/// Excludes is_epic, epic_id, and order_in_epic fields to prevent clients from
/// directly modifying epic relationships. Use dedicated epic commands instead:
/// - add_ticket_to_epic
/// - remove_ticket_from_epic
/// - reorder_epic_children
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTicketInput {
    pub title: Option<String>,
    pub description_md: Option<String>,
    pub priority: Option<Priority>,
    pub labels: Option<Vec<String>>,
    pub project_id: Option<String>,
    pub workflow_type: Option<WorkflowType>,
    pub model: Option<String>,
    pub branch_name: Option<String>,
    pub column_id: Option<String>,
}

#[tauri::command]
pub async fn get_tickets(
    board_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<Ticket>, String> {
    tracing::debug!("Getting tickets for board: {}", board_id);
    db.get_tickets(&board_id, None).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_ticket(ticket_id: String, db: State<'_, Arc<Database>>) -> Result<Ticket, String> {
    db.get_ticket(&ticket_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_ticket(
    ticket: CreateTicketInput,
    db: State<'_, Arc<Database>>,
) -> Result<Ticket, String> {
    tracing::info!(
        "Creating ticket: {} (epic: {})",
        ticket.title,
        ticket.is_epic
    );
    let create = CreateTicket {
        board_id: ticket.board_id,
        column_id: ticket.column_id,
        title: ticket.title,
        description_md: ticket.description_md,
        priority: ticket.priority,
        labels: ticket.labels,
        project_id: ticket.project_id,
        workflow_type: ticket.workflow_type.unwrap_or_default(),
        model: ticket.model,
        branch_name: ticket.branch_name,
        is_epic: ticket.is_epic,
        epic_id: ticket.epic_id,
        depends_on_epic_id: None,
        depends_on_epic_ids: vec![],
        spec_version_id: None,
    };
    db.create_ticket(&create).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn move_ticket(
    ticket_id: String,
    column_id: String,
    app_handle: AppHandle,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    tracing::info!("Moving ticket {} to column {}", ticket_id, column_id);

    // Get the ticket before moving to check if it's an epic
    let ticket = db.get_ticket(&ticket_id).map_err(|e| e.to_string())?;

    // Get the target column name
    let columns = db
        .get_columns(&ticket.board_id)
        .map_err(|e| e.to_string())?;
    let target_column = columns.iter().find(|c| c.id == column_id);
    let target_column_name = target_column.map(|c| c.name.as_str()).unwrap_or("");

    // Perform the move
    db.move_ticket(&ticket_id, &column_id)
        .map_err(|e| e.to_string())?;

    // Refresh ticket after move for lifecycle hooks
    let updated_ticket = db.get_ticket(&ticket_id).map_err(|e| e.to_string())?;

    // Epic lifecycle: when an epic is moved to Ready, advance its first child
    if ticket.is_epic && target_column_name.eq_ignore_ascii_case("Ready") {
        if let Err(e) = crate::lifecycle::epic::on_epic_moved_to_ready(&db, &updated_ticket) {
            tracing::warn!("Failed to advance epic children: {}", e);
        }
    }

    // Handle ticket moved to Done - trigger lifecycle hooks
    if target_column_name.eq_ignore_ascii_case("Done") {
        let db_arc = db.inner().clone();
        // If this is a child ticket (has epic_id), trigger child completion
        if updated_ticket.epic_id.is_some() {
            if let Err(e) = crate::lifecycle::epic::on_child_completed(&db_arc, &updated_ticket) {
                tracing::warn!("Failed to handle child completion: {}", e);
            }
        }
        // If this is an epic with a spec, check for spec completion
        else if updated_ticket.is_epic && updated_ticket.spec_version_id.is_some() {
            if let Err(e) = crate::lifecycle::epic::check_spec_completion_by_id(
                &db_arc,
                updated_ticket.spec_version_id.as_ref().unwrap(),
            ) {
                tracing::warn!("Failed to check spec completion: {}", e);
            }
        }
    }

    // Handle ticket moved to Blocked - trigger epic blocking
    if target_column_name.eq_ignore_ascii_case("Blocked") && updated_ticket.epic_id.is_some() {
        let db_arc = db.inner().clone();
        if let Err(e) = crate::lifecycle::epic::on_child_blocked(&db_arc, &updated_ticket) {
            tracing::warn!("Failed to handle child blocked: {}", e);
        }
    }

    crate::tray::refresh_tray(&app_handle);

    Ok(())
}

#[tauri::command]
pub async fn update_ticket(
    ticket_id: String,
    updates: UpdateTicketInput,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    tracing::info!("Updating ticket: {}", ticket_id);

    // Get the ticket before updating to check for column changes and epic status
    let ticket = db.get_ticket(&ticket_id).map_err(|e| e.to_string())?;
    let old_column_id = ticket.column_id.clone();
    let is_column_changing = updates
        .column_id
        .as_ref()
        .map(|new_col| new_col != &old_column_id)
        .unwrap_or(false);

    let new_description = updates.description_md.clone();

    // Convert to UpdateTicket, explicitly setting epic fields to None to prevent
    // clients from modifying epic relationships through this command.
    // Use dedicated epic commands (add_ticket_to_epic, remove_ticket_from_epic,
    // reorder_epic_children) to manage epic associations.
    let update = UpdateTicket {
        title: updates.title,
        description_md: updates.description_md,
        priority: updates.priority,
        labels: updates.labels,
        project_id: updates.project_id,
        workflow_type: updates.workflow_type,
        model: updates.model,
        branch_name: updates.branch_name,
        column_id: updates.column_id.clone(),
        is_epic: None,
        epic_id: None,
        order_in_epic: None,
        depends_on_epic_id: None,
        depends_on_epic_ids: vec![],
        spec_version_id: None,
    };
    db.update_ticket(&ticket_id, &update)
        .map(|_| ())
        .map_err(|e| e.to_string())?;

    // Keep the initial task's content in sync with the ticket description so
    // clarification edits propagate; also reset failed tasks to pending.
    if let Some(ref new_description) = new_description {
        if let Ok(tasks) = db.get_tasks_for_ticket(&ticket_id) {
            if let Some(initial_task) = tasks.iter().find(|t| {
                t.order_index == 0
                    && t.task_type == TaskType::Custom
                    && (t.status == TaskStatus::Pending || t.status == TaskStatus::Failed)
            }) {
                let new_status = if initial_task.status == TaskStatus::Failed {
                    Some(TaskStatus::Pending)
                } else {
                    None
                };
                if let Err(e) = db.update_task(
                    &initial_task.id,
                    &UpdateTask {
                        title: None,
                        content: Some(new_description.clone()),
                        status: new_status,
                        run_id: None,
                    },
                ) {
                    tracing::warn!(
                        "Failed to sync description to initial task for ticket {}: {}",
                        ticket_id,
                        e
                    );
                } else {
                    tracing::info!(
                        "Synced description to initial task for ticket {}",
                        ticket_id,
                    );
                }
            }
        }
    }

    // Epic lifecycle hooks for column changes
    if is_column_changing {
        if let Some(new_column_id) = updates.column_id {
            // Get the target column name
            let columns = db
                .get_columns(&ticket.board_id)
                .map_err(|e| e.to_string())?;
            let target_column = columns.iter().find(|c| c.id == new_column_id);
            let target_column_name = target_column.map(|c| c.name.as_str()).unwrap_or("");

            // Refresh ticket after update for lifecycle hooks
            let updated_ticket = db.get_ticket(&ticket_id).map_err(|e| e.to_string())?;
            let db_arc = db.inner().clone();

            // Epic moved to Ready: advance its first child
            if ticket.is_epic && target_column_name.eq_ignore_ascii_case("Ready") {
                if let Err(e) = crate::lifecycle::epic::on_epic_moved_to_ready(&db, &updated_ticket)
                {
                    tracing::warn!("Failed to advance epic children on update: {}", e);
                }
            }

            // Ticket moved to Done: trigger child completion or check spec completion
            if target_column_name.eq_ignore_ascii_case("Done") {
                if updated_ticket.epic_id.is_some() {
                    if let Err(e) =
                        crate::lifecycle::epic::on_child_completed(&db_arc, &updated_ticket)
                    {
                        tracing::warn!("Failed to handle child completion on update: {}", e);
                    }
                } else if updated_ticket.is_epic && updated_ticket.spec_version_id.is_some() {
                    if let Err(e) = crate::lifecycle::epic::check_spec_completion_by_id(
                        &db_arc,
                        updated_ticket.spec_version_id.as_ref().unwrap(),
                    ) {
                        tracing::warn!("Failed to check spec completion on update: {}", e);
                    }
                }
            }

            // Ticket moved to Blocked: trigger epic blocking
            if target_column_name.eq_ignore_ascii_case("Blocked")
                && updated_ticket.epic_id.is_some()
            {
                if let Err(e) = crate::lifecycle::epic::on_child_blocked(&db_arc, &updated_ticket) {
                    tracing::warn!("Failed to handle child blocked on update: {}", e);
                }
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn delete_ticket(
    ticket_id: String,
    app_handle: AppHandle,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    tracing::info!("Deleting ticket: {}", ticket_id);
    db.delete_ticket(&ticket_id).map_err(|e| e.to_string())?;
    crate::tray::refresh_tray(&app_handle);
    Ok(())
}

#[tauri::command]
pub async fn get_comments(
    ticket_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<Comment>, String> {
    tracing::debug!("Getting comments for ticket: {}", ticket_id);
    db.get_comments(&ticket_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_comment(
    ticket_id: String,
    body: String,
    author_type: String,
    db: State<'_, Arc<Database>>,
) -> Result<Comment, String> {
    tracing::info!("Adding comment to ticket: {}", ticket_id);
    let author = match author_type.as_str() {
        "user" => AuthorType::User,
        "system" => AuthorType::System,
        _ => AuthorType::Agent,
    };
    let create = CreateComment {
        ticket_id,
        author_type: author,
        body_md: body,
        metadata: None,
    };
    db.create_comment(&create).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_comment(
    comment_id: String,
    body: String,
    db: State<'_, Arc<Database>>,
) -> Result<Comment, String> {
    tracing::info!("Updating comment: {}", comment_id);
    db.update_comment(&comment_id, &body)
        .map_err(|e| e.to_string())
}

// ===== Epic Commands =====

#[tauri::command]
pub async fn get_epic_children(
    epic_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<Ticket>, String> {
    tracing::debug!("Getting children for epic: {}", epic_id);
    db.get_epic_children(&epic_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_epic_progress(
    epic_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<EpicProgress, String> {
    tracing::debug!("Getting progress for epic: {}", epic_id);
    db.get_epic_progress(&epic_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_ticket_to_epic(
    epic_id: String,
    ticket_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    tracing::info!("Adding ticket {} to epic {}", ticket_id, epic_id);
    db.add_ticket_to_epic(&epic_id, &ticket_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_ticket_from_epic(
    ticket_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    tracing::info!("Removing ticket {} from epic", ticket_id);
    db.remove_ticket_from_epic(&ticket_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reorder_epic_children(
    epic_id: String,
    child_ids: Vec<String>,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    tracing::info!("Reordering children for epic {}: {:?}", epic_id, child_ids);
    db.reorder_epic_children(&epic_id, &child_ids)
        .map_err(|e| e.to_string())
}

/// Pause a ticket's execution - saves current stage and run ID for later resume
#[tauri::command]
pub async fn pause_ticket(
    ticket_id: String,
    stage: String,
    run_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    tracing::info!(
        "Pausing ticket {} at stage {} (run {})",
        ticket_id,
        stage,
        run_id
    );
    db.pause_ticket(&ticket_id, &stage, &run_id)
        .map_err(|e| e.to_string())
}

/// Resume a paused ticket - moves to Ready and returns the stage to resume from
#[tauri::command]
pub async fn resume_ticket(
    ticket_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Option<String>, String> {
    tracing::info!("Resuming ticket {}", ticket_id);

    // Get the ticket to find its board
    let ticket = db.get_ticket(&ticket_id).map_err(|e| e.to_string())?;

    // Find the Ready column for this board
    let columns = db
        .get_columns(&ticket.board_id)
        .map_err(|e| e.to_string())?;
    let ready_column = columns
        .iter()
        .find(|c| c.name == "Ready")
        .ok_or_else(|| "Ready column not found".to_string())?;

    // Resume the ticket (clears paused_at)
    let stage = db.resume_ticket(&ticket_id).map_err(|e| e.to_string())?;

    // Move ticket to Ready so workers can pick it up
    db.move_ticket(&ticket_id, &ready_column.id)
        .map_err(|e| e.to_string())?;
    tracing::info!(
        "Moved ticket {} to Ready column for worker pickup",
        ticket_id
    );

    Ok(stage)
}

