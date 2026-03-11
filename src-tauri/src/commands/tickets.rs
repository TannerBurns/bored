use serde::Deserialize;
use std::sync::Arc;
use tauri::{AppHandle, State};

use crate::agents::plan_validation::{rewrite_task_with_clarification, PlanValidationConfig};
use crate::agents::registry::AgentRegistry;
use crate::commands::agent_settings::AgentSettingsManager;
use crate::db::{
    AuthorType, Comment, CreateComment, CreateTicket, Database, EpicProgress, Priority, RunStatus,
    Ticket, UpdateTicket, WorkflowType,
};
use crate::db::models::{TaskStatus, UpdateTask};

/// Reject moving a non-epic ticket to the Ready column when it has no tasks.
fn require_tasks_for_ready(
    db: &Database,
    ticket: &Ticket,
    target_column_name: &str,
) -> Result<(), String> {
    if !ticket.is_epic && target_column_name.eq_ignore_ascii_case("Ready") {
        let tasks = db
            .get_tasks_for_ticket(&ticket.id)
            .map_err(|e| e.to_string())?;
        if tasks.is_empty() {
            return Err("Cannot move to Ready: ticket has no tasks".to_string());
        }
    }
    Ok(())
}

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

    require_tasks_for_ready(&db, &ticket, target_column_name)?;

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

    if is_column_changing {
        if let Some(ref new_col) = updates.column_id {
            let columns = db
                .get_columns(&ticket.board_id)
                .map_err(|e| e.to_string())?;
            let target_name = columns
                .iter()
                .find(|c| &c.id == new_col)
                .map(|c| c.name.as_str())
                .unwrap_or("");
            require_tasks_for_ready(&db, &ticket, target_name)?;
        }
    }

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
    db.get_epic_children(&epic_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_epic_progress(
    epic_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<EpicProgress, String> {
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

/// Resolve a clarification by rewriting the task spec with an LLM, then move to Ready.
#[tauri::command]
pub async fn resolve_clarification(
    ticket_id: String,
    user_response: String,
    agent_type: Option<String>,
    db: State<'_, Arc<Database>>,
    agent_settings: State<'_, AgentSettingsManager>,
    registry: State<'_, Arc<AgentRegistry>>,
) -> Result<Ticket, String> {
    tracing::info!(
        "Resolving clarification for ticket {} (response={} chars)",
        ticket_id,
        user_response.len()
    );

    let ticket = db.get_ticket(&ticket_id).map_err(|e| e.to_string())?;

    let project_id = ticket
        .project_id
        .as_ref()
        .ok_or_else(|| "Ticket has no project assigned".to_string())?;
    let project = db
        .get_project(project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Project '{}' not found", project_id))?;

    let comments = db.get_comments(&ticket_id).map_err(|e| e.to_string())?;
    let clarification_comment = comments
        .iter()
        .filter(|c| c.author_type != AuthorType::User)
        .filter(|c| {
            c.metadata
                .as_ref()
                .and_then(|m| m.get("type"))
                .and_then(|v| v.as_str())
                == Some("clarification")
        })
        .next_back()
        .ok_or_else(|| "No clarification comment found for this ticket".to_string())?;

    let clarification_questions = extract_clarification_body(&clarification_comment.body_md);

    let blocked_task_id = clarification_comment
        .metadata
        .as_ref()
        .and_then(|m| m.get("task_id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let blocked_task = blocked_task_id
        .as_ref()
        .and_then(|id| db.get_task(id).ok());

    // When a task exists, rewrite its content. For the legacy taskless workflow
    // (task_id is null in metadata), fall back to the ticket description.
    let original_spec = blocked_task
        .as_ref()
        .and_then(|t| t.content.clone())
        .unwrap_or_else(|| ticket.description_md.clone());

    let agent_id = agent_type.unwrap_or_else(|| registry.default_agent_id());
    let provider = registry
        .get(&agent_id)
        .ok_or_else(|| format!("Unknown agent: {}", agent_id))?;
    let agent_config = agent_settings.agent_config_for(&agent_id);

    let parent_run = db
        .create_run(&crate::db::CreateRun {
            ticket_id: ticket_id.clone(),
            agent_type: agent_id.clone(),
            repo_path: project.path.clone(),
            parent_run_id: None,
            stage: Some("clarification-rewrite".to_string()),
            ..Default::default()
        })
        .map_err(|e| format!("Failed to create parent run: {}", e))?;

    let config = PlanValidationConfig {
        db: db.inner().clone(),
        parent_run_id: parent_run.id.clone(),
        ticket_id: ticket_id.clone(),
        repo_path: std::path::PathBuf::from(&project.path),
        model: ticket.model.clone(),
        agent_id,
        provider,
        agent_config,
        timeout_secs: 120,
    };

    let _ = db.update_run_status(&parent_run.id, RunStatus::Running, None, None);

    let rewrite_result = rewrite_task_with_clarification(
        &config,
        &original_spec,
        &clarification_questions,
        &user_response,
    )
    .await;

    let rewritten_spec = match rewrite_result {
        Ok(spec) => {
            let _ = db.update_run_status(&parent_run.id, RunStatus::Finished, Some(0), None);
            spec
        }
        Err(e) => {
            let _ = db.update_run_status(
                &parent_run.id,
                RunStatus::Error,
                None,
                Some(&e.to_string()),
            );
            return Err(format!("Failed to rewrite spec: {}", e));
        }
    };

    if let Some(ref task_id) = blocked_task_id {
        db.update_task(
            task_id,
            &UpdateTask {
                content: Some(rewritten_spec.clone()),
                title: None,
                status: None,
                run_id: None,
            },
        )
        .map_err(|e| format!("Failed to update task content: {}", e))?;
        tracing::info!("Updated task {} content with rewritten spec", task_id);
    } else {
        db.update_ticket(
            &ticket_id,
            &UpdateTicket {
                description_md: Some(rewritten_spec.clone()),
                ..Default::default()
            },
        )
        .map_err(|e| format!("Failed to update ticket description: {}", e))?;
        tracing::info!(
            "Updated ticket {} description with rewritten spec (legacy taskless workflow)",
            ticket_id
        );
    }

    // Reset failed tasks to pending so they can be retried after clarification
    if let Ok(tasks) = db.get_tasks_for_ticket(&ticket_id) {
        for task in tasks.iter().filter(|t| t.status == TaskStatus::Failed) {
            if let Err(e) = db.update_task(
                &task.id,
                &UpdateTask {
                    title: None,
                    content: None,
                    status: Some(TaskStatus::Pending),
                    run_id: None,
                },
            ) {
                tracing::warn!("Failed to reset failed task {}: {}", task.id, e);
            }
        }
    }

    // Move ticket to Ready
    let columns = db
        .get_columns(&ticket.board_id)
        .map_err(|e| e.to_string())?;
    let ready_column = columns
        .iter()
        .find(|c| c.name == "Ready")
        .ok_or_else(|| "Ready column not found".to_string())?;

    db.move_ticket(&ticket_id, &ready_column.id)
        .map_err(|e| e.to_string())?;

    tracing::info!(
        "Clarification resolved for ticket {} — moved to Ready",
        ticket_id
    );

    db.get_ticket(&ticket_id).map_err(|e| e.to_string())
}

/// Extract the clarification body from the full comment markdown.
/// The format is: "## Clarification Needed\n\n{body}\n\n---\n*footer*"
fn extract_clarification_body(body_md: &str) -> String {
    let header_end = body_md.find("\n\n");
    let footer_start = body_md.rfind("\n\n---\n");
    if let (Some(h), Some(f)) = (header_end, footer_start) {
        if f > h {
            return body_md[h + 2..f].trim().to_string();
        }
    }
    body_md.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    mod extract_clarification_body_tests {
        use super::*;

        #[test]
        fn standard_format() {
            let body =
                "## Clarification Needed\n\nWhat framework should we use?\n\n---\n*Update the ticket.*";
            assert_eq!(
                extract_clarification_body(body),
                "What framework should we use?"
            );
        }

        #[test]
        fn multiline_body() {
            let body = "## Clarification Needed\n\n1. Which DB?\n2. What auth?\n\n---\n*footer*";
            assert_eq!(
                extract_clarification_body(body),
                "1. Which DB?\n2. What auth?"
            );
        }

        #[test]
        fn no_structure_returns_full_text() {
            let body = "plain text with no header or footer";
            assert_eq!(extract_clarification_body(body), body);
        }

        #[test]
        fn empty_string() {
            assert_eq!(extract_clarification_body(""), "");
        }

        #[test]
        fn header_only_no_footer() {
            let body = "## Header\n\nSome body text without a footer";
            assert_eq!(extract_clarification_body(body), body);
        }

        #[test]
        fn footer_before_header_returns_full_text() {
            let body = "\n\n---\n*footer*\n\nsome text after";
            assert_eq!(extract_clarification_body(body), body);
        }

        #[test]
        fn body_with_extra_whitespace_is_trimmed() {
            let body =
                "## Header\n\n   spaced content   \n\n---\n*footer*";
            assert_eq!(
                extract_clarification_body(body),
                "spaced content"
            );
        }

        #[test]
        fn body_with_multiple_paragraphs() {
            let body = "## Clarification Needed\n\nParagraph one.\n\nParagraph two.\n\n---\n*footer*";
            assert_eq!(
                extract_clarification_body(body),
                "Paragraph one.\n\nParagraph two."
            );
        }

        #[test]
        fn body_with_markdown_formatting() {
            let body = "## Clarification\n\n- **Option A**: React\n- **Option B**: Vue\n\n---\n*Edit the ticket.*";
            assert_eq!(
                extract_clarification_body(body),
                "- **Option A**: React\n- **Option B**: Vue"
            );
        }
    }
}
