use std::sync::Arc;

use tokio::sync::broadcast;

use crate::agents::chat::{TicketBuilderOutput, TicketBuilderTicket};
use crate::api::state::LiveEvent;
use crate::db::models::{Chat, ChatMessageRole, CreateTask, CreateTicket, WorkflowType};
use crate::db::Database;

use super::apply_updates::apply_ticket_updates;

pub(crate) fn apply_ticket_builder_output(
    db: &Arc<Database>,
    event_tx: &broadcast::Sender<LiveEvent>,
    chat_id: &str,
    chat: &Chat,
    board_id: &str,
    backlog_column_id: &str,
    output: TicketBuilderOutput,
) -> Result<Vec<String>, String> {
    let mut created_ticket_ids: Vec<String> = Vec::new();
    let mut updated_ticket_ids: Vec<String> = Vec::new();
    let mut summary_lines: Vec<String> = Vec::new();

    let create_child_tickets = |db: &Arc<Database>,
                                tickets_data: &[TicketBuilderTicket],
                                board_id: &str,
                                column_id: &str,
                                epic_id: Option<&str>,
                                project_id: &Option<String>,
                                workspace_id: &Option<String>,
                                event_tx: &broadcast::Sender<LiveEvent>,
                                summary_lines: &mut Vec<String>,
                                indent: &str|
     -> Result<Vec<String>, String> {
        let mut ids = Vec::new();
        for ticket_data in tickets_data {
            if let Some(ref existing_id) = ticket_data.id {
                let Some(parent_epic_id) = epic_id else {
                    return Err(
                        "Ticket entries with \"id\" must be nested under an epic".to_string(),
                    );
                };
                let existing = db
                    .get_ticket(existing_id)
                    .map_err(|e| e.to_string())?;
                if existing.is_epic {
                    return Err(format!(
                        "Cannot attach epic {} as a child of another epic",
                        existing_id
                    ));
                }
                if existing.board_id != board_id {
                    return Err(format!(
                        "Ticket {} is on a different board than this chat",
                        existing_id
                    ));
                }
                db.add_ticket_to_epic(parent_epic_id, existing_id)
                    .map_err(|e| e.to_string())?;

                let _ = event_tx.send(LiveEvent::TicketUpdated {
                    ticket_id: existing_id.clone(),
                });

                let label = if ticket_data.title.trim().is_empty() {
                    existing.title.clone()
                } else {
                    ticket_data.title.clone()
                };
                summary_lines.push(format!(
                    "{}- Linked existing \"{}\" (id: {})",
                    indent, label, existing_id
                ));
                ids.push(existing_id.clone());
                continue;
            }

            if ticket_data.title.trim().is_empty() || ticket_data.description.trim().is_empty() {
                return Err(
                    "Each new ticket must include non-empty title and description".to_string(),
                );
            }

            let priority = ticket_data.resolved_priority();
            let ticket = db
                .create_ticket(&CreateTicket {
                    board_id: board_id.to_string(),
                    column_id: column_id.to_string(),
                    title: ticket_data.title.clone(),
                    description_md: ticket_data.description.clone(),
                    priority,
                    labels: vec![],
                    project_id: project_id.clone(),
                    workspace_id: workspace_id.clone(),
                    workflow_type: WorkflowType::default(),
                    model: None,
                    branch_name: None,
                    is_epic: false,
                    epic_id: epic_id.map(|s| s.to_string()),
                    depends_on_epic_id: None,
                    depends_on_epic_ids: vec![],
                    spec_version_id: None,
                })
                .map_err(|e| e.to_string())?;

            if let Some(ref tasks) = ticket_data.tasks {
                for task in tasks {
                    db.create_task(&CreateTask {
                        ticket_id: ticket.id.clone(),
                        task_type: Default::default(),
                        title: Some(task.title.clone()),
                        content: task.content.clone(),
                    })
                    .map_err(|e| e.to_string())?;
                }
            }

            let _ = event_tx.send(LiveEvent::TicketCreated {
                ticket_id: ticket.id.clone(),
                board_id: board_id.to_string(),
            });

            summary_lines.push(format!(
                "{}- \"{}\" (id: {})",
                indent, ticket_data.title, ticket.id
            ));

            ids.push(ticket.id);
        }
        Ok(ids)
    };

    if !output.tickets.is_empty() {
        let standalone_ids = create_child_tickets(
            db,
            &output.tickets,
            board_id,
            backlog_column_id,
            None,
            &chat.project_id,
            &chat.workspace_id,
            event_tx,
            &mut summary_lines,
            "",
        )?;
        created_ticket_ids.extend(standalone_ids);
    }

    for epic_data in &output.epics {
        let epic_id = if let Some(ref existing_id) = epic_data.id {
            let existing = db.get_ticket(existing_id).map_err(|e| e.to_string())?;
            if !existing.is_epic {
                return Err(format!("Ticket {} is not an epic", existing_id));
            }
            summary_lines.push(format!(
                "- Added to epic \"{}\" (id: {})",
                existing.title, existing_id
            ));
            existing_id.clone()
        } else {
            if epic_data.name.trim().is_empty() {
                return Err("New epic entries must include a non-empty name".to_string());
            }
            let epic_desc = epic_data.description.clone().unwrap_or_default();
            let epic_ticket = db
                .create_ticket(&CreateTicket {
                    board_id: board_id.to_string(),
                    column_id: backlog_column_id.to_string(),
                    title: epic_data.name.clone(),
                    description_md: epic_desc,
                    priority: crate::db::models::Priority::Medium,
                    labels: vec![],
                    project_id: chat.project_id.clone(),
                    workspace_id: chat.workspace_id.clone(),
                    workflow_type: WorkflowType::default(),
                    model: None,
                    branch_name: None,
                    is_epic: true,
                    epic_id: None,
                    depends_on_epic_id: None,
                    depends_on_epic_ids: vec![],
                    spec_version_id: None,
                })
                .map_err(|e| e.to_string())?;

            let _ = event_tx.send(LiveEvent::TicketCreated {
                ticket_id: epic_ticket.id.clone(),
                board_id: board_id.to_string(),
            });

            summary_lines.push(format!(
                "- Epic \"{}\" (id: {})",
                epic_data.name, epic_ticket.id
            ));

            created_ticket_ids.push(epic_ticket.id.clone());
            epic_ticket.id
        };

        let child_ids = create_child_tickets(
            db,
            &epic_data.tickets,
            board_id,
            backlog_column_id,
            Some(&epic_id),
            &chat.project_id,
            &chat.workspace_id,
            event_tx,
            &mut summary_lines,
            "  ",
        )?;
        created_ticket_ids.extend(child_ids);
    }

    apply_ticket_updates(
        db,
        event_tx,
        &output.updates,
        &mut updated_ticket_ids,
        &mut summary_lines,
    )?;

    let mut header_parts: Vec<String> = Vec::new();
    if !created_ticket_ids.is_empty() {
        header_parts.push(format!("Created {} ticket(s)", created_ticket_ids.len()));
    }
    if !updated_ticket_ids.is_empty() {
        header_parts.push(format!("updated {} ticket(s)", updated_ticket_ids.len()));
    }
    let header = if header_parts.is_empty() {
        "No changes applied".to_string()
    } else {
        let mut h = header_parts.join(" and ");
        h.push(':');
        h
    };

    let summary = if summary_lines.is_empty() {
        header.trim_end_matches(':').to_string()
    } else {
        format!("{}\n{}", header, summary_lines.join("\n"))
    };

    let mut all_ids = created_ticket_ids;
    all_ids.extend(updated_ticket_ids.iter().cloned());

    if let Ok(sys_msg) = db.create_chat_message(
        chat_id,
        ChatMessageRole::System,
        &summary,
        Some(&serde_json::json!({
            "type": "tickets_created",
            "ticketIds": &all_ids,
            "updatedTicketIds": &updated_ticket_ids,
        })),
    ) {
        let _ = event_tx.send(LiveEvent::ChatMessageAdded {
            chat_id: chat_id.to_string(),
            message_id: sys_msg.id,
            role: "system".to_string(),
        });
    }

    Ok(all_ids)
}
