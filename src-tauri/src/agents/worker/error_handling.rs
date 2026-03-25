//! Error handling for worker operations.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

use crate::agents::provider::AgentProvider;
use crate::db::{AuthorType, CreateComment, Database, Ticket};
use crate::lifecycle::epic::on_child_blocked;

use super::super::diagnostic;
use super::super::worktree::WorktreeError;

/// Context for handling worktree failures.
pub struct WorktreeFailureContext<'a> {
    pub db: Arc<Database>,
    pub app_handle: Option<AppHandle>,
    pub ticket: &'a Ticket,
    pub repo_path: &'a Path,
    pub error: &'a WorktreeError,
    pub provider: Arc<dyn AgentProvider>,
    pub agent_config: HashMap<String, serde_json::Value>,
    pub worker_id: &'a str,
    pub diagnostic_model: Option<String>,
}

/// Spawns a diagnostic agent and moves ticket to Blocked.
///
/// Returns `true` if the ticket was successfully moved to the Blocked column.
/// When this returns `false`, the caller must take alternative action (e.g. keep
/// the ticket locked) to prevent infinite re-queuing.
pub async fn handle_worktree_failure(ctx: WorktreeFailureContext<'_>) -> bool {
    let WorktreeFailureContext {
        db,
        app_handle,
        ticket,
        repo_path,
        error,
        provider,
        agent_config,
        worker_id,
        diagnostic_model,
    } = ctx;
    tracing::info!(
        "Worker {} handling worktree failure for ticket {}: {:?}",
        worker_id,
        ticket.id,
        error.diagnostic_type()
    );

    let mut context = diagnostic::classify_worktree_error(error);
    context.repo_path = repo_path.to_path_buf();
    context.additional_context = Some(format!(
        "Branch: {}, Ticket: {}",
        ticket.branch_name.as_deref().unwrap_or("(new)"),
        ticket.title
    ));

    // Post error comment before the move to prevent stale clarification banner.
    let _ = db.create_comment(&CreateComment {
        ticket_id: ticket.id.clone(),
        author_type: AuthorType::System,
        body_md: format!(
            "## Blocked: {:?}\n\nDiagnosing issue...",
            error.diagnostic_type()
        ),
        metadata: Some(serde_json::json!({ "type": "diagnostic" })),
    });

    let ticket_blocked = move_ticket_to_blocked(&db, &app_handle, ticket, worker_id);

    let db_clone = db.clone();
    let ticket_id = ticket.id.clone();
    let model = Some(diagnostic_model.unwrap_or_else(|| crate::agents::models::DEFAULT_DIAGNOSTIC_MODEL.to_string()));
    let context_clone = context.clone();
    let worker_id = worker_id.to_string();

    tokio::spawn(async move {
        tracing::info!(
            "Worker {} spawning diagnostic agent for ticket {}",
            worker_id,
            ticket_id
        );

        match diagnostic::run_diagnostic_agent(
            db_clone.clone(),
            app_handle,
            &ticket_id,
            context_clone.clone(),
            model,
            provider,
            agent_config,
        )
        .await
        {
            Ok(()) => {
                tracing::info!(
                    "Worker {} diagnostic agent completed for ticket {}",
                    worker_id,
                    ticket_id
                );
            }
            Err(e) => {
                tracing::warn!(
                    "Worker {} diagnostic agent failed for ticket {}: {}. Adding fallback comment.",
                    worker_id, ticket_id, e
                );

                // Fall back to a static comment with basic troubleshooting steps
                let fallback_comment = diagnostic::create_fallback_diagnostic_comment(&context_clone);

                if let Err(comment_err) = db_clone.create_comment(&CreateComment {
                    ticket_id: ticket_id.clone(),
                    author_type: AuthorType::System,
                    body_md: fallback_comment,
                    metadata: None,
                }) {
                    tracing::error!(
                        "Worker {} failed to create fallback diagnostic comment for ticket {}: {}",
                        worker_id, ticket_id, comment_err
                    );
                }
            }
        }
    });

    ticket_blocked
}

/// Move a ticket to the Blocked column.
///
/// Returns `true` if the ticket was successfully moved, `false` otherwise.
pub fn move_ticket_to_blocked(
    db: &Arc<Database>,
    app_handle: &Option<AppHandle>,
    ticket: &Ticket,
    worker_id: &str,
) -> bool {
    match db.find_column_by_name(&ticket.board_id, "Blocked") {
        Ok(Some(column)) => {
            if let Err(e) = db.move_ticket(&ticket.id, &column.id) {
                tracing::error!(
                    "Worker {} failed to move ticket {} to Blocked: {}",
                    worker_id,
                    ticket.id,
                    e
                );
                false
            } else {
                tracing::info!(
                    "Worker {} moved ticket {} to Blocked column",
                    worker_id,
                    ticket.id
                );

                if let Some(ref app_handle) = app_handle {
                    let _ = app_handle.emit(
                        "ticket-moved",
                        serde_json::json!({
                            "ticketId": ticket.id,
                            "columnName": "Blocked",
                            "columnId": column.id,
                        }),
                    );
                }

                if ticket.epic_id.is_some() {
                    if let Err(e) = on_child_blocked(db, ticket) {
                        tracing::warn!(
                            "Worker {} failed to block parent epic for ticket {}: {}",
                            worker_id,
                            ticket.id,
                            e
                        );
                    }
                }

                true
            }
        }
        Ok(None) => {
            tracing::warn!(
                "Worker {} could not find Blocked column for board {}",
                worker_id,
                ticket.board_id
            );
            false
        }
        Err(e) => {
            tracing::error!("Worker {} error finding Blocked column: {}", worker_id, e);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{CreateTicket, Priority, WorkflowType};

    fn setup_db_with_ticket() -> (Arc<Database>, Ticket) {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let ready_col = columns.iter().find(|c| c.name == "Ready").unwrap();

        let ticket = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: ready_col.id.clone(),
                title: "Test ticket".to_string(),
                description_md: "desc".to_string(),
                priority: Priority::Medium,
                labels: vec![],
                project_id: None,
                workspace_id: None,
                workflow_type: WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: None,
            })
            .unwrap();

        (db, ticket)
    }

    #[test]
    fn move_ticket_to_blocked_returns_true_on_success() {
        let (db, ticket) = setup_db_with_ticket();

        let result = move_ticket_to_blocked(&db, &None, &ticket, "test-worker");
        assert!(result);

        let updated = db.get_ticket(&ticket.id).unwrap();
        let columns = db.get_columns(&ticket.board_id).unwrap();
        let blocked_col = columns.iter().find(|c| c.name == "Blocked").unwrap();
        assert_eq!(updated.column_id, blocked_col.id);
    }

    #[test]
    fn move_ticket_to_blocked_returns_false_when_no_blocked_column() {
        let (db, mut ticket) = setup_db_with_ticket();

        // Point ticket at a non-existent board so find_column_by_name returns None
        ticket.board_id = "nonexistent-board-id".to_string();

        let result = move_ticket_to_blocked(&db, &None, &ticket, "test-worker");
        assert!(!result);
    }

    #[test]
    fn move_ticket_to_blocked_returns_false_when_move_fails() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let board = db.create_board("Test Board").unwrap();

        let fake_ticket = Ticket {
            id: "nonexistent-ticket-id".to_string(),
            board_id: board.id.clone(),
            column_id: "col-1".to_string(),
            title: "Fake".to_string(),
            description_md: String::new(),
            priority: Priority::Medium,
            labels: vec![],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            locked_by_run_id: None,
            lock_expires_at: None,
            project_id: None,
            workspace_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
            order_in_epic: None,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
            paused_at: None,
            paused_at_stage: None,
            paused_run_id: None,
        };

        // Blocked column exists but move_ticket returns NotFound for the
        // nonexistent ticket, so move_ticket_to_blocked must return false.
        let result = move_ticket_to_blocked(&db, &None, &fake_ticket, "test-worker");
        assert!(!result);
    }

    #[test]
    fn move_ticket_to_blocked_preserves_ticket_in_ready_on_failure() {
        let (db, mut ticket) = setup_db_with_ticket();

        let original_column_id = ticket.column_id.clone();
        ticket.board_id = "nonexistent-board-id".to_string();

        let result = move_ticket_to_blocked(&db, &None, &ticket, "test-worker");
        assert!(!result);

        let updated = db.get_ticket(&ticket.id).unwrap();
        assert_eq!(updated.column_id, original_column_id);
    }
}
