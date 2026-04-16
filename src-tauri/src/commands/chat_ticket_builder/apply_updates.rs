use std::sync::Arc;

use tokio::sync::broadcast;

use crate::agents::chat::{ticket_is_in_done_column, TicketBuilderUpdate};
use crate::api::state::LiveEvent;
use crate::db::models::{CreateTask, UpdateTicket};
use crate::db::Database;

pub(crate) fn apply_ticket_updates(
    db: &Arc<Database>,
    event_tx: &broadcast::Sender<LiveEvent>,
    updates: &[TicketBuilderUpdate],
    updated_ids: &mut Vec<String>,
    summary_lines: &mut Vec<String>,
) -> Result<(), String> {
    for update in updates {
        let existing = db
            .get_ticket(&update.ticket_id)
            .map_err(|e| e.to_string())?;

        let columns = db
            .get_columns(&existing.board_id)
            .map_err(|e| e.to_string())?;
        if ticket_is_in_done_column(&existing.column_id, &columns) {
            tracing::warn!(
                ticket_id = %update.ticket_id,
                "Skipping ticket builder update: ticket is in Done column"
            );
            summary_lines.push(format!(
                "- Skipped update for completed ticket \"{}\" (id: {}) — tickets in Done are read-only",
                existing.title, update.ticket_id
            ));
            continue;
        }

        let priority = update
            .priority
            .as_deref()
            .and_then(crate::db::models::Priority::parse);

        let epic_change = update.epic_id.clone();

        let update_data = UpdateTicket {
            title: update.title.clone(),
            description_md: update.description.clone(),
            priority,
            column_id: None,
            labels: None,
            project_id: None,
            workspace_id: None,
            workflow_type: None,
            model: None,
            branch_name: None,
            is_epic: None,
            epic_id: None,
            order_in_epic: None,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        };

        db.update_ticket(&update.ticket_id, &update_data)
            .map_err(|e| e.to_string())?;

        if let Some(ref tasks) = update.tasks {
            db.delete_tasks_for_ticket(&update.ticket_id)
                .map_err(|e| e.to_string())?;
            for task in tasks {
                db.create_task(&CreateTask {
                    ticket_id: update.ticket_id.clone(),
                    task_type: Default::default(),
                    title: Some(task.title.clone()),
                    content: task.content.clone(),
                })
                .map_err(|e| e.to_string())?;
            }
        }

        if let Some(ref eid) = epic_change {
            if eid.is_empty() {
                db.remove_ticket_from_epic(&update.ticket_id)
                    .map_err(|e| e.to_string())?;
            } else {
                db.add_ticket_to_epic(eid, &update.ticket_id)
                    .map_err(|e| e.to_string())?;
            }
        }

        let _ = event_tx.send(LiveEvent::TicketUpdated {
            ticket_id: update.ticket_id.clone(),
        });

        let display_title = update.title.as_deref().unwrap_or(&existing.title);
        let mut line = format!("- Updated \"{}\" (id: {})", display_title, update.ticket_id);
        if let Some(ref eid) = epic_change {
            if eid.is_empty() {
                line.push_str(" — removed from epic");
            } else {
                line.push_str(&format!(" — assigned to epic {}", eid));
            }
        }
        summary_lines.push(line);

        updated_ids.push(update.ticket_id.clone());
    }
    Ok(())
}
