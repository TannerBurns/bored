//! Fix-task lifecycle helpers for validation sessions.
//!
//! Handles creating fix tasks from agent responses, polling for their
//! completion, and posting summary messages back to the chat.

use std::sync::Arc;
use tokio::sync::broadcast;

use super::validation_parsing::parse_create_fix_tasks_from_response;
use crate::api::state::LiveEvent;
use crate::db::models::{
    CreateValidationMessage, TaskStatus, ValidationMessage, ValidationMessageRole,
    ValidationSessionStatus,
};
use crate::db::Database;

/// Process any create_fix_task blocks found in an agent response.
/// Creates tasks in the DB, updates session status, moves ticket to Ready, and emits events.
/// Returns the IDs of any tasks that were created.
pub(super) fn process_fix_tasks_in_response(
    response_text: &str,
    db: &Arc<Database>,
    session_id: &str,
    ticket_id: &str,
    ticket_title: &str,
    ticket_board_id: &str,
    event_tx: &broadcast::Sender<LiveEvent>,
) -> Vec<String> {
    let fix_block = match parse_create_fix_tasks_from_response(response_text) {
        Some(block) => block,
        None => return Vec::new(),
    };

    let mut task_ids = Vec::new();
    for fix_task in &fix_block.tasks {
        let mut description = fix_task.description.clone();
        if let Some(ref criteria) = fix_task.acceptance_criteria {
            description.push_str("\n\n## Acceptance Criteria\n");
            for criterion in criteria {
                description.push_str(&format!("- {}\n", criterion));
            }
        }
        if let Ok(task) = db.create_task(&crate::db::models::CreateTask {
            ticket_id: ticket_id.to_string(),
            task_type: crate::db::models::TaskType::Custom,
            title: Some(fix_task.title.clone()),
            content: Some(description),
        }) {
            task_ids.push(task.id);
        }
    }

    if !task_ids.is_empty() {
        let _ = db.update_validation_session_status(
            session_id,
            &ValidationSessionStatus::Failed,
        );
        let task_summary = fix_block
            .tasks
            .iter()
            .map(|t| format!("- {}", t.title))
            .collect::<Vec<_>>()
            .join("\n");
        let _ = db.create_validation_message(&CreateValidationMessage {
            session_id: session_id.to_string(),
            role: ValidationMessageRole::System,
            content: format!(
                "Fix tasks created for ticket **{}**:\n{}\n\nA worker agent will pick these up. You'll be notified when the work completes.",
                ticket_title, task_summary
            ),
            metadata: Some(serde_json::json!({
                "type": "fix_tasks_created",
                "task_ids": task_ids,
            })),
        });
        if let Ok(columns) = db.get_columns(ticket_board_id) {
            if let Some(ready_col) = columns.iter().find(|c| c.name == "Ready") {
                let _ = db.move_ticket(ticket_id, &ready_col.id);
            }
        }
        let _ = event_tx.send(LiveEvent::ValidationFixTasksCreated {
            session_id: session_id.to_string(),
            ticket_id: ticket_id.to_string(),
            task_count: task_ids.len(),
        });
    }

    task_ids
}

/// Poll the DB until all tasks in `task_ids` reach a terminal state (completed/failed),
/// emitting progress log events so the UI keeps showing the thinking indicator.
pub(super) async fn wait_for_fix_tasks(
    task_ids: &[String],
    db: &Arc<Database>,
    event_tx: &broadcast::Sender<LiveEvent>,
    session_id: &str,
) {
    if task_ids.is_empty() {
        return;
    }

    const POLL_INTERVAL_SECS: u64 = 5;
    const MAX_WAIT_SECS: u64 = 30 * 60;
    let timeout = std::time::Duration::from_secs(MAX_WAIT_SECS);
    let start = std::time::Instant::now();

    let _ = event_tx.send(LiveEvent::ValidationLogEntry {
        session_id: session_id.to_string(),
        stream: "stdout".to_string(),
        message: "Waiting for worker agent to complete fix tasks...".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    });

    loop {
        let mut completed = 0usize;
        let mut failed = 0usize;
        let mut still_pending = 0usize;

        for id in task_ids {
            match db.get_task(id) {
                Ok(task) => match task.status {
                    TaskStatus::Completed => completed += 1,
                    TaskStatus::Failed => failed += 1,
                    _ => still_pending += 1,
                },
                Err(_) => completed += 1,
            }
        }

        if still_pending == 0 {
            break;
        }

        if start.elapsed() > timeout {
            tracing::warn!(
                "Timed out waiting for fix tasks in session {} ({} still pending)",
                session_id, still_pending
            );
            break;
        }

        let _ = event_tx.send(LiveEvent::ValidationLogEntry {
            session_id: session_id.to_string(),
            stream: "stdout".to_string(),
            message: format!(
                "Fix tasks: {} completed, {} failed, {} in progress",
                completed, failed, still_pending
            ),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });

        tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
    }
}

/// After fix tasks finish, post a system message summarizing the outcome
/// and asking the user about next steps (without auto-starting the app).
pub(super) fn post_fix_tasks_completion_message(
    task_ids: &[String],
    db: &Arc<Database>,
    event_tx: &broadcast::Sender<LiveEvent>,
    session_id: &str,
) -> Option<ValidationMessage> {
    if task_ids.is_empty() {
        return None;
    }

    let mut completed = 0usize;
    let mut failed = 0usize;
    let mut still_running = 0usize;
    for id in task_ids {
        if let Ok(task) = db.get_task(id) {
            match task.status {
                TaskStatus::Completed => completed += 1,
                TaskStatus::Failed => failed += 1,
                _ => still_running += 1,
            }
        }
    }

    let content = if still_running > 0 {
        format!(
            "Timed out waiting for fix tasks: {} completed, {} failed, {} still in progress.",
            completed, failed, still_running
        )
    } else if failed == 0 && completed > 0 {
        format!(
            "All {} fix task(s) completed successfully. Would you like to re-validate the changes, or is there anything else to check?",
            completed
        )
    } else if completed == 0 && failed > 0 {
        format!(
            "All {} fix task(s) failed. Would you like to review what went wrong and try again?",
            failed
        )
    } else {
        format!(
            "{} fix task(s) completed and {} failed. Would you like to review the results?",
            completed, failed
        )
    };

    let msg = db
        .create_validation_message(&CreateValidationMessage {
            session_id: session_id.to_string(),
            role: ValidationMessageRole::System,
            content,
            metadata: Some(serde_json::json!({ "type": "fix_tasks_completed" })),
        })
        .ok()?;

    let _ = event_tx.send(LiveEvent::ValidationMessageAdded {
        session_id: session_id.to_string(),
        message_id: msg.id.clone(),
        role: "system".to_string(),
    });

    Some(msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::{
        CreateTask, CreateTicket, CreateValidationSession, Priority, TaskType, WorkflowType,
    };

    fn setup_validation_fixture() -> (Arc<Database>, String, String, String) {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let ticket = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: columns[0].id.clone(),
                title: "Test Ticket".to_string(),
                description_md: "Test description".to_string(),
                priority: Priority::Medium,
                labels: vec![],
                project_id: None,
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
        let session = db
            .create_validation_session(&CreateValidationSession {
                ticket_id: ticket.id.clone(),
                project_id: None,
                agent_type: None,
            })
            .unwrap();
        (db, board.id, ticket.id, session.id)
    }

    // --- process_fix_tasks_in_response ---

    #[test]
    fn process_fix_tasks_returns_created_task_ids() {
        let (db, board_id, ticket_id, session_id) = setup_validation_fixture();
        let (event_tx, _rx) = broadcast::channel(16);

        let response = "Found issues:\n```json\n\
            { \"create_fix_task\": { \"title\": \"Fix login\", \"description\": \"Login broken\" } }\n\
            ```";

        let ids = process_fix_tasks_in_response(
            response, &db, &session_id, &ticket_id,
            "Test Ticket", &board_id, &event_tx,
        );

        assert_eq!(ids.len(), 1);
        let task = db.get_task(&ids[0]).unwrap();
        assert_eq!(task.title, Some("Fix login".to_string()));
        assert_eq!(task.status, TaskStatus::Pending);
    }

    #[test]
    fn process_fix_tasks_returns_empty_when_no_blocks() {
        let (db, board_id, ticket_id, session_id) = setup_validation_fixture();
        let (event_tx, _rx) = broadcast::channel(16);

        let ids = process_fix_tasks_in_response(
            "Looks good, no issues found.",
            &db, &session_id, &ticket_id,
            "Test Ticket", &board_id, &event_tx,
        );

        assert!(ids.is_empty());
    }

    #[test]
    fn process_fix_tasks_returns_multiple_ids_for_plural_form() {
        let (db, board_id, ticket_id, session_id) = setup_validation_fixture();
        let (event_tx, _rx) = broadcast::channel(16);

        let response = "Multiple issues:\n```json\n\
            { \"create_fix_tasks\": { \"tasks\": [\n\
                { \"title\": \"Fix A\", \"description\": \"A\" },\n\
                { \"title\": \"Fix B\", \"description\": \"B\" }\n\
            ] } }\n```";

        let ids = process_fix_tasks_in_response(
            response, &db, &session_id, &ticket_id,
            "Test Ticket", &board_id, &event_tx,
        );

        assert_eq!(ids.len(), 2);
        assert_eq!(db.get_task(&ids[0]).unwrap().title, Some("Fix A".to_string()));
        assert_eq!(db.get_task(&ids[1]).unwrap().title, Some("Fix B".to_string()));
    }

    #[test]
    fn process_fix_tasks_sets_session_status_to_failed() {
        let (db, board_id, ticket_id, session_id) = setup_validation_fixture();
        let (event_tx, _rx) = broadcast::channel(16);

        let response = "```json\n\
            { \"create_fix_task\": { \"title\": \"Fix it\", \"description\": \"broken\" } }\n```";

        let ids = process_fix_tasks_in_response(
            response, &db, &session_id, &ticket_id,
            "Test Ticket", &board_id, &event_tx,
        );

        assert!(!ids.is_empty());
        let session = db.get_validation_session(&session_id).unwrap();
        assert_eq!(session.status, ValidationSessionStatus::Failed);
    }

    // --- post_fix_tasks_completion_message ---

    #[test]
    fn completion_message_returns_none_for_empty_ids() {
        let (db, _, _, session_id) = setup_validation_fixture();
        let (event_tx, _rx) = broadcast::channel(16);

        assert!(post_fix_tasks_completion_message(&[], &db, &event_tx, &session_id).is_none());
    }

    #[test]
    fn completion_message_all_completed() {
        let (db, _, ticket_id, session_id) = setup_validation_fixture();
        let (event_tx, _rx) = broadcast::channel(16);

        let t1 = db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::Custom,
            title: Some("Fix 1".to_string()),
            content: None,
        }).unwrap();
        db.start_task(&t1.id, "run-1").unwrap();
        db.complete_task(&t1.id).unwrap();

        let t2 = db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::Custom,
            title: Some("Fix 2".to_string()),
            content: None,
        }).unwrap();
        db.start_task(&t2.id, "run-2").unwrap();
        db.complete_task(&t2.id).unwrap();

        let msg = post_fix_tasks_completion_message(
            &[t1.id, t2.id], &db, &event_tx, &session_id,
        ).unwrap();

        assert!(msg.content.contains("All 2 fix task(s) completed successfully"));
        assert_eq!(msg.role, ValidationMessageRole::System);
    }

    #[test]
    fn completion_message_all_failed() {
        let (db, _, ticket_id, session_id) = setup_validation_fixture();
        let (event_tx, _rx) = broadcast::channel(16);

        let t1 = db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::Custom,
            title: Some("Fix 1".to_string()),
            content: None,
        }).unwrap();
        db.start_task(&t1.id, "run-1").unwrap();
        db.fail_task(&t1.id).unwrap();

        let msg = post_fix_tasks_completion_message(
            &[t1.id], &db, &event_tx, &session_id,
        ).unwrap();

        assert!(msg.content.contains("All 1 fix task(s) failed"));
    }

    #[test]
    fn completion_message_mixed_results() {
        let (db, _, ticket_id, session_id) = setup_validation_fixture();
        let (event_tx, _rx) = broadcast::channel(16);

        let t1 = db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::Custom,
            title: Some("Fix 1".to_string()),
            content: None,
        }).unwrap();
        db.start_task(&t1.id, "run-1").unwrap();
        db.complete_task(&t1.id).unwrap();

        let t2 = db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::Custom,
            title: Some("Fix 2".to_string()),
            content: None,
        }).unwrap();
        db.start_task(&t2.id, "run-2").unwrap();
        db.fail_task(&t2.id).unwrap();

        let msg = post_fix_tasks_completion_message(
            &[t1.id, t2.id], &db, &event_tx, &session_id,
        ).unwrap();

        assert!(msg.content.contains("1 fix task(s) completed and 1 failed"));
    }

    #[test]
    fn completion_message_timeout_with_pending_tasks() {
        let (db, _, ticket_id, session_id) = setup_validation_fixture();
        let (event_tx, _rx) = broadcast::channel(16);

        let t1 = db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::Custom,
            title: Some("Fix 1".to_string()),
            content: None,
        }).unwrap();
        db.start_task(&t1.id, "run-1").unwrap();
        db.complete_task(&t1.id).unwrap();

        // t2 is still pending -- simulates a timeout scenario
        let t2 = db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::Custom,
            title: Some("Fix 2".to_string()),
            content: None,
        }).unwrap();

        let msg = post_fix_tasks_completion_message(
            &[t1.id, t2.id], &db, &event_tx, &session_id,
        ).unwrap();

        assert!(msg.content.contains("Timed out"));
        assert!(msg.content.contains("1 still in progress"));
    }

    #[test]
    fn completion_message_has_correct_metadata_type() {
        let (db, _, ticket_id, session_id) = setup_validation_fixture();
        let (event_tx, _rx) = broadcast::channel(16);

        let t1 = db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::Custom,
            title: Some("Fix".to_string()),
            content: None,
        }).unwrap();
        db.start_task(&t1.id, "run-1").unwrap();
        db.complete_task(&t1.id).unwrap();

        let msg = post_fix_tasks_completion_message(
            &[t1.id], &db, &event_tx, &session_id,
        ).unwrap();

        let meta = msg.metadata.unwrap();
        assert_eq!(meta.get("type").and_then(|v| v.as_str()), Some("fix_tasks_completed"));
    }

    // --- wait_for_fix_tasks ---

    #[tokio::test]
    async fn wait_for_fix_tasks_empty_returns_immediately() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let (event_tx, _rx) = broadcast::channel(16);

        wait_for_fix_tasks(&[], &db, &event_tx, "session-1").await;
    }

    #[tokio::test]
    async fn wait_for_fix_tasks_exits_when_all_completed() {
        tokio::time::pause();
        let (db, _, ticket_id, _) = setup_validation_fixture();
        let (event_tx, _rx) = broadcast::channel(16);

        let t1 = db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::Custom,
            title: Some("Fix".to_string()),
            content: None,
        }).unwrap();
        db.start_task(&t1.id, "run-1").unwrap();
        db.complete_task(&t1.id).unwrap();

        wait_for_fix_tasks(&[t1.id], &db, &event_tx, "session-1").await;
    }

    #[tokio::test]
    async fn wait_for_fix_tasks_exits_when_all_failed() {
        tokio::time::pause();
        let (db, _, ticket_id, _) = setup_validation_fixture();
        let (event_tx, _rx) = broadcast::channel(16);

        let t1 = db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::Custom,
            title: Some("Fix".to_string()),
            content: None,
        }).unwrap();
        db.start_task(&t1.id, "run-1").unwrap();
        db.fail_task(&t1.id).unwrap();

        wait_for_fix_tasks(&[t1.id], &db, &event_tx, "session-1").await;
    }

    #[tokio::test]
    async fn wait_for_fix_tasks_exits_on_mix_of_completed_and_failed() {
        tokio::time::pause();
        let (db, _, ticket_id, _) = setup_validation_fixture();
        let (event_tx, _rx) = broadcast::channel(16);

        let t1 = db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::Custom,
            title: Some("Fix 1".to_string()),
            content: None,
        }).unwrap();
        db.start_task(&t1.id, "run-1").unwrap();
        db.complete_task(&t1.id).unwrap();

        let t2 = db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::Custom,
            title: Some("Fix 2".to_string()),
            content: None,
        }).unwrap();
        db.start_task(&t2.id, "run-2").unwrap();
        db.fail_task(&t2.id).unwrap();

        wait_for_fix_tasks(&[t1.id, t2.id], &db, &event_tx, "session-1").await;
    }

    #[tokio::test]
    async fn wait_for_fix_tasks_treats_missing_task_as_complete() {
        tokio::time::pause();
        let db = Arc::new(Database::open_in_memory().unwrap());
        let (event_tx, _rx) = broadcast::channel(16);

        wait_for_fix_tasks(
            &["nonexistent-id".to_string()], &db, &event_tx, "session-1",
        ).await;
    }
}
