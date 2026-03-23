//! Fix-task lifecycle helpers for review mode.

use std::sync::Arc;

use tokio::sync::broadcast;

use crate::agents::validation_agent::parsing::parse_create_fix_tasks_from_response;
use crate::api::state::LiveEvent;
use crate::db::models::{
    ChatMessage, ChatMessageRole, CreateTask, TaskStatus, TaskType,
};
use crate::db::Database;

/// Create fix tasks from parsed response blocks, save a chat system message,
/// move the ticket to Ready, and return the task IDs.
pub(super) fn process_fix_tasks_for_chat(
    response_text: &str,
    db: &Arc<Database>,
    chat_id: &str,
    ticket_id: &str,
    ticket_title: &str,
    board_id: &str,
    event_tx: &broadcast::Sender<LiveEvent>,
) -> Vec<String> {
    let fix_block = match parse_create_fix_tasks_from_response(response_text) {
        Some(block) => block,
        None => {
            tracing::debug!(
                "No create_fix_tasks block found in response ({} chars)",
                response_text.len()
            );
            return Vec::new();
        }
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
        if let Ok(task) = db.create_task(&CreateTask {
            ticket_id: ticket_id.to_string(),
            task_type: TaskType::Custom,
            title: Some(fix_task.title.clone()),
            content: Some(description),
        }) {
            task_ids.push(task.id);
        }
    }

    if !task_ids.is_empty() {
        let task_summary = fix_block
            .tasks
            .iter()
            .map(|t| format!("- {}", t.title))
            .collect::<Vec<_>>()
            .join("\n");
        if let Ok(msg) = db.create_chat_message(
            chat_id,
            ChatMessageRole::System,
            &format!(
                "Fix tasks created for ticket **{}**:\n{}\n\nA worker agent will pick these up. You'll be notified when the work completes.",
                ticket_title, task_summary
            ),
            Some(&serde_json::json!({
                "type": "fix_tasks_created",
                "task_ids": task_ids,
            })),
        ) {
            let _ = event_tx.send(LiveEvent::ChatMessageAdded {
                chat_id: chat_id.to_string(),
                message_id: msg.id,
                role: "system".to_string(),
            });
        }
        if let Ok(columns) = db.get_columns(board_id) {
            if let Some(ready_col) = columns.iter().find(|c| c.name == "Ready") {
                let _ = db.move_ticket(ticket_id, &ready_col.id);
            }
        }
    }

    task_ids
}

pub(super) async fn wait_for_fix_tasks_chat(
    task_ids: &[String],
    db: &Arc<Database>,
    event_tx: &broadcast::Sender<LiveEvent>,
    chat_id: &str,
) {
    if task_ids.is_empty() {
        return;
    }

    const POLL_INTERVAL_SECS: u64 = 5;
    const MAX_WAIT_SECS: u64 = 30 * 60;
    let timeout = std::time::Duration::from_secs(MAX_WAIT_SECS);
    let start = std::time::Instant::now();

    let _ = event_tx.send(LiveEvent::ChatLogEntry {
        chat_id: chat_id.to_string(),
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
                "Timed out waiting for fix tasks in chat {} ({} still pending)",
                chat_id,
                still_pending
            );
            break;
        }

        let _ = event_tx.send(LiveEvent::ChatLogEntry {
            chat_id: chat_id.to_string(),
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

pub(super) fn post_fix_tasks_completion_chat(
    task_ids: &[String],
    db: &Arc<Database>,
    event_tx: &broadcast::Sender<LiveEvent>,
    chat_id: &str,
) -> Option<ChatMessage> {
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
        .create_chat_message(
            chat_id,
            ChatMessageRole::System,
            &content,
            Some(&serde_json::json!({ "type": "fix_tasks_completed" })),
        )
        .ok()?;

    let _ = event_tx.send(LiveEvent::ChatMessageAdded {
        chat_id: chat_id.to_string(),
        message_id: msg.id.clone(),
        role: "system".to_string(),
    });

    Some(msg)
}
