use super::{TicketState, LifecycleOutcome, TransitionPermission, can_transition};
use crate::db::{Database, DbError};

pub struct TransitionExecutor<'a> {
    db: &'a Database,
}

impl<'a> TransitionExecutor<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn move_ticket(
        &self,
        ticket_id: &str,
        target_state: TicketState,
        is_system: bool,
    ) -> Result<TransitionResult, DbError> {
        let ticket = self.get_ticket(ticket_id)?;
        let current_state = TicketState::from_column_name(&ticket.column_name)
            .ok_or_else(|| DbError::Validation("Unknown column state".into()))?;
        
        let is_locked = ticket.locked_by_run_id.is_some();

        match can_transition(current_state, target_state, is_locked, is_system) {
            TransitionPermission::Allowed => {
                self.execute_transition(ticket_id, &ticket.board_id, target_state)?;
                Ok(TransitionResult::Success {
                    from: current_state,
                    to: target_state,
                })
            }
            TransitionPermission::RequiresUnlock => {
                Ok(TransitionResult::RequiresUnlock {
                    from: current_state,
                    to: target_state,
                })
            }
            TransitionPermission::Denied(reason) => {
                Ok(TransitionResult::Denied {
                    from: current_state,
                    to: target_state,
                    reason,
                })
            }
        }
    }

    pub fn handle_run_completion(
        &self,
        ticket_id: &str,
        outcome: LifecycleOutcome,
    ) -> Result<TransitionResult, DbError> {
        self.move_ticket(ticket_id, outcome.target_state(), true)
    }

    fn get_ticket(&self, ticket_id: &str) -> Result<TicketInfo, DbError> {
        self.db.with_conn(|conn| {
            conn.query_row(
                r#"SELECT t.board_id, c.name as column_name, t.locked_by_run_id
                   FROM tickets t
                   JOIN columns c ON t.column_id = c.id
                   WHERE t.id = ?"#,
                [ticket_id],
                |row| {
                    Ok(TicketInfo {
                        board_id: row.get(0)?,
                        column_name: row.get(1)?,
                        locked_by_run_id: row.get(2)?,
                    })
                },
            ).map_err(|_| DbError::NotFound(format!("Ticket {} not found", ticket_id)))
        })
    }

    fn execute_transition(
        &self,
        ticket_id: &str,
        board_id: &str,
        target_state: TicketState,
    ) -> Result<(), DbError> {
        self.db.with_conn(|conn| {
            let target_column_id: String = conn.query_row(
                "SELECT id FROM columns WHERE board_id = ? AND name = ?",
                rusqlite::params![board_id, target_state.to_column_name()],
                |row| row.get(0),
            ).map_err(|_| DbError::NotFound(format!(
                "Column {} not found in board {}",
                target_state.to_column_name(),
                board_id
            )))?;

            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "UPDATE tickets SET column_id = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![target_column_id, now, ticket_id],
            )?;

            Ok(())
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionResult {
    Success { from: TicketState, to: TicketState },
    RequiresUnlock { from: TicketState, to: TicketState },
    Denied { from: TicketState, to: TicketState, reason: String },
}

impl TransitionResult {
    pub fn is_success(&self) -> bool {
        matches!(self, TransitionResult::Success { .. })
    }
}

struct TicketInfo {
    board_id: String,
    column_name: String,
    locked_by_run_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transition_result_is_success() {
        let success = TransitionResult::Success {
            from: TicketState::Ready,
            to: TicketState::InProgress,
        };
        assert!(success.is_success());

        let denied = TransitionResult::Denied {
            from: TicketState::Backlog,
            to: TicketState::InProgress,
            reason: "Not allowed".to_string(),
        };
        assert!(!denied.is_success());

        let requires_unlock = TransitionResult::RequiresUnlock {
            from: TicketState::InProgress,
            to: TicketState::Ready,
        };
        assert!(!requires_unlock.is_success());
    }
}
