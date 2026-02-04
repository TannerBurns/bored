use crate::db::models::Ticket;
use crate::db::{Database, DbError};

impl Database {
    pub fn move_ticket(&self, ticket_id: &str, column_id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now();
            let affected = conn.execute(
                "UPDATE tickets SET column_id = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![column_id, now.to_rfc3339(), ticket_id],
            )?;

            if affected == 0 {
                return Err(DbError::NotFound(format!("Ticket {} not found", ticket_id)));
            }
            Ok(())
        })
    }

    pub fn set_ticket_project(
        &self,
        ticket_id: &str,
        project_id: Option<&str>,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "UPDATE tickets SET project_id = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![project_id, now, ticket_id],
            )?;
            Ok(())
        })
    }

    /// Set the branch name for a ticket (used after agent generates branch name)
    pub fn set_ticket_branch(&self, ticket_id: &str, branch_name: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            let affected = conn.execute(
                "UPDATE tickets SET branch_name = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![branch_name, now, ticket_id],
            )?;

            if affected == 0 {
                return Err(DbError::NotFound(format!("Ticket {} not found", ticket_id)));
            }
            Ok(())
        })
    }

    /// Pause a ticket's execution - saves current stage and run ID for later resume
    pub fn pause_ticket(&self, ticket_id: &str, stage: &str, run_id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now();
            let affected = conn.execute(
                "UPDATE tickets SET paused_at = ?, paused_at_stage = ?, paused_run_id = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![
                    now.to_rfc3339(),
                    stage,
                    run_id,
                    now.to_rfc3339(),
                    ticket_id
                ],
            )?;

            if affected == 0 {
                return Err(DbError::NotFound(format!("Ticket {} not found", ticket_id)));
            }
            Ok(())
        })
    }

    /// Resume a paused ticket - clears pause state but preserves stage info for orchestrator
    /// Returns the stage the ticket was paused at
    pub fn resume_ticket(&self, ticket_id: &str) -> Result<Option<String>, DbError> {
        self.with_conn(|conn| {
            // First get the paused_at_stage
            let stage: Option<String> = conn
                .query_row(
                    "SELECT paused_at_stage FROM tickets WHERE id = ?",
                    [ticket_id],
                    |row| row.get(0),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        DbError::NotFound(format!("Ticket {} not found", ticket_id))
                    }
                    other => DbError::Sqlite(other),
                })?;

            let now = chrono::Utc::now().to_rfc3339();
            // Preserve paused_at_stage and paused_run_id for orchestrator
            conn.execute(
                "UPDATE tickets SET paused_at = NULL, updated_at = ? WHERE id = ?",
                rusqlite::params![now, ticket_id],
            )?;

            Ok(stage)
        })
    }

    /// Clear pause state from a ticket without returning the stage
    pub fn clear_ticket_pause(&self, ticket_id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            let affected = conn.execute(
                "UPDATE tickets SET paused_at = NULL, paused_at_stage = NULL, paused_run_id = NULL, updated_at = ? WHERE id = ?",
                rusqlite::params![now, ticket_id],
            )?;

            if affected == 0 {
                return Err(DbError::NotFound(format!("Ticket {} not found", ticket_id)));
            }
            Ok(())
        })
    }

    /// Get all paused tickets for a spec
    pub fn get_paused_tickets(&self, spec_version_id: &str) -> Result<Vec<Ticket>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, column_id, title, description_md, priority, 
                          labels_json, created_at, updated_at, locked_by_run_id, 
                          lock_expires_at, project_id, workflow_type, model, branch_name,
                          is_epic, epic_id, order_in_epic, depends_on_epic_id, depends_on_epic_ids_json, spec_version_id,
                          paused_at, paused_at_stage, paused_run_id
                   FROM tickets 
                   WHERE spec_version_id = ? AND paused_at IS NOT NULL
                   ORDER BY paused_at ASC"#,
            )?;

            let rows = stmt.query_map([spec_version_id], Self::map_ticket_row)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    /// Check if a ticket is currently paused
    pub fn is_ticket_paused(&self, ticket_id: &str) -> Result<bool, DbError> {
        self.with_conn(|conn| {
            let paused_at: Option<String> = conn
                .query_row(
                    "SELECT paused_at FROM tickets WHERE id = ?",
                    [ticket_id],
                    |row| row.get(0),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        DbError::NotFound(format!("Ticket {} not found", ticket_id))
                    }
                    other => DbError::Sqlite(other),
                })?;

            Ok(paused_at.is_some())
        })
    }
}
