//! Error handling for worker operations.

use std::sync::Arc;
use tauri::{AppHandle, Emitter};

use crate::db::{AuthorType, CreateComment, Database, Ticket};
use crate::lifecycle::epic::on_child_blocked;

use super::super::diagnostic;
use super::super::worktree::WorktreeError;
use super::super::{AgentKind, ClaudeApiConfig};

/// Spawns a diagnostic agent and moves ticket to Blocked.
pub async fn handle_worktree_failure(
    db: Arc<Database>,
    app_handle: Option<AppHandle>,
    ticket: &Ticket,
    repo_path: &std::path::Path,
    error: &WorktreeError,
    api_url: &str,
    api_token: &str,
    agent_kind: AgentKind,
    claude_api_config: Option<ClaudeApiConfig>,
    worker_id: &str,
) {
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

    move_ticket_to_blocked(&db, &app_handle, ticket, &worker_id);

    let db_clone = db.clone();
    let ticket_id = ticket.id.clone();
    let ticket_model = ticket.model.clone();
    let api_url = api_url.to_string();
    let api_token = api_token.to_string();
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
            &api_url,
            &api_token,
            ticket_model,
            agent_kind,
            claude_api_config,
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
}

/// Move a ticket to the Blocked column
pub fn move_ticket_to_blocked(
    db: &Arc<Database>,
    app_handle: &Option<AppHandle>,
    ticket: &Ticket,
    worker_id: &str,
) {
    match db.find_column_by_name(&ticket.board_id, "Blocked") {
        Ok(Some(column)) => {
            if let Err(e) = db.move_ticket(&ticket.id, &column.id) {
                tracing::error!(
                    "Worker {} failed to move ticket {} to Blocked: {}",
                    worker_id,
                    ticket.id,
                    e
                );
            } else {
                tracing::info!(
                    "Worker {} moved ticket {} to Blocked column",
                    worker_id,
                    ticket.id
                );

                // Emit event if we have an app handle
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

                // Epic lifecycle: if this ticket is a child, block the parent epic
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
            }
        }
        Ok(None) => {
            tracing::warn!(
                "Worker {} could not find Blocked column for board {}",
                worker_id,
                ticket.board_id
            );
        }
        Err(e) => {
            tracing::error!("Worker {} error finding Blocked column: {}", worker_id, e);
        }
    }
}
