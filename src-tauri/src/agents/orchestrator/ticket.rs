//! Ticket movement and lifecycle management for the workflow orchestrator.

use super::WorkflowOrchestrator;
use crate::lifecycle::epic::{on_child_blocked, on_child_completed};

impl WorkflowOrchestrator {
    /// Move the ticket to a column by name (best effort - logs warning if column not found)
    pub(super) fn move_ticket_to_column(&self, column_name: &str) {
        tracing::info!(
            "Attempting to move ticket {} to column '{}' on board {}",
            self.ticket.id,
            column_name,
            self.ticket.board_id
        );

        match self
            .db
            .find_column_by_name(&self.ticket.board_id, column_name)
        {
            Ok(Some(column)) => {
                tracing::info!(
                    "Found column '{}' with id {} for board {}",
                    column_name,
                    column.id,
                    self.ticket.board_id
                );
                if let Err(e) = self.db.move_ticket(&self.ticket.id, &column.id) {
                    tracing::error!(
                        "Failed to move ticket {} to '{}': {}",
                        self.ticket.id,
                        column_name,
                        e
                    );
                } else {
                    tracing::info!(
                        "Successfully moved ticket {} to column '{}'",
                        self.ticket.id,
                        column_name
                    );
                    // Emit event for frontend to update
                    if let Err(e) = self.emit_event(
                        "ticket-moved",
                        &serde_json::json!({
                            "ticketId": self.ticket.id,
                            "columnName": column_name,
                            "columnId": column.id,
                        }),
                    ) {
                        tracing::warn!("Failed to emit ticket-moved event: {}", e);
                    } else {
                        tracing::info!("Emitted ticket-moved event for ticket {}", self.ticket.id);
                    }

                    // Epic lifecycle hooks: when a child ticket moves to Done or Blocked,
                    // trigger epic advancement or blocking
                    if self.ticket.epic_id.is_some() {
                        match column_name {
                            "Done" => {
                                // Child completed - try to advance epic
                                if let Err(e) = on_child_completed(&self.db, &self.ticket) {
                                    tracing::warn!("Epic advancement failed: {}", e);
                                }
                            }
                            "Blocked" => {
                                // Child blocked - block parent epic
                                if let Err(e) = on_child_blocked(&self.db, &self.ticket) {
                                    tracing::warn!("Epic blocking failed: {}", e);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            Ok(None) => {
                tracing::error!(
                    "Column '{}' not found for board {}. Looking up available columns...",
                    column_name,
                    self.ticket.board_id
                );
                // Log available columns for debugging
                if let Ok(columns) = self.db.get_columns(&self.ticket.board_id) {
                    let column_names: Vec<_> = columns.iter().map(|c| c.name.as_str()).collect();
                    tracing::error!("Available columns on board: {:?}", column_names);
                }
            }
            Err(e) => {
                tracing::error!("Error finding column '{}': {}", column_name, e);
            }
        }
    }
}
