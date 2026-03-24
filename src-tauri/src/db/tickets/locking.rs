// Agent type parameter kept for API compatibility but unused in SQL query
use crate::db::models::Ticket;
use crate::db::{Database, DbError};
use chrono::{DateTime, Utc};

use super::ReadyTicketDiagnostics;

impl Database {
    /// Attempt to lock a ticket for an agent run.
    ///
    /// This method uses atomic locking semantics: it only acquires the lock if:
    /// - The ticket is not currently locked (locked_by_run_id IS NULL), OR
    /// - The same run already holds the lock (locked_by_run_id = run_id), OR
    /// - The existing lock has expired (lock_expires_at < now)
    ///
    /// The second condition allows a paused run to re-acquire its lock when resuming,
    /// since the lock is preserved during pause to maintain exclusive access.
    ///
    /// Returns Ok(()) if the lock was acquired, Err(LockConflict) if another run
    /// holds a valid lock, or Err(NotFound) if the ticket doesn't exist.
    pub fn lock_ticket(
        &self,
        ticket_id: &str,
        run_id: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now();
            let now_str = now.to_rfc3339();

            // Atomically acquire lock only if not held by another run.
            // Also allow the same run to re-acquire its own lock (for resume after pause).
            let affected = conn.execute(
                r#"UPDATE tickets 
                   SET locked_by_run_id = ?, lock_expires_at = ?, updated_at = ?
                   WHERE id = ? 
                     AND (locked_by_run_id IS NULL OR locked_by_run_id = ? OR lock_expires_at < ?)"#,
                rusqlite::params![
                    run_id,
                    expires_at.to_rfc3339(),
                    now_str,
                    ticket_id,
                    run_id,
                    now_str,
                ],
            )?;

            if affected == 0 {
                // Check if ticket exists to give appropriate error
                let exists: bool = conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM tickets WHERE id = ?)",
                    [ticket_id],
                    |row| row.get(0),
                )?;

                if !exists {
                    return Err(DbError::NotFound(format!("Ticket {}", ticket_id)));
                }

                // Ticket exists but has a valid lock held by another run
                return Err(DbError::Validation(format!(
                    "Ticket {} is already locked by another run",
                    ticket_id
                )));
            }
            Ok(())
        })
    }

    pub fn unlock_ticket(&self, ticket_id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            conn.execute(
                r#"UPDATE tickets 
                   SET locked_by_run_id = NULL, lock_expires_at = NULL, updated_at = ?
                   WHERE id = ?"#,
                rusqlite::params![chrono::Utc::now().to_rfc3339(), ticket_id],
            )?;
            Ok(())
        })
    }

    pub fn extend_lock(
        &self,
        ticket_id: &str,
        run_id: &str,
        new_expires_at: DateTime<Utc>,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let affected = conn.execute(
                r#"UPDATE tickets 
                   SET lock_expires_at = ?, updated_at = ?
                   WHERE id = ? AND locked_by_run_id = ?"#,
                rusqlite::params![
                    new_expires_at.to_rfc3339(),
                    chrono::Utc::now().to_rfc3339(),
                    ticket_id,
                    run_id,
                ],
            )?;

            if affected == 0 {
                return Err(DbError::NotFound("Lock not found or expired".to_string()));
            }
            Ok(())
        })
    }

    /// Update the run_id that owns a ticket lock.
    /// Used when a temporary run_id is replaced with the actual run ID after creation.
    /// Only updates if the lock is currently held by old_run_id.
    pub fn update_ticket_lock_owner(
        &self,
        ticket_id: &str,
        old_run_id: &str,
        new_run_id: &str,
        new_expires_at: Option<DateTime<Utc>>,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now();
            let affected = if let Some(expires) = new_expires_at {
                conn.execute(
                    r#"UPDATE tickets 
                       SET locked_by_run_id = ?, lock_expires_at = ?, updated_at = ?
                       WHERE id = ? AND locked_by_run_id = ?"#,
                    rusqlite::params![
                        new_run_id,
                        expires.to_rfc3339(),
                        now.to_rfc3339(),
                        ticket_id,
                        old_run_id,
                    ],
                )?
            } else {
                conn.execute(
                    r#"UPDATE tickets 
                       SET locked_by_run_id = ?, updated_at = ?
                       WHERE id = ? AND locked_by_run_id = ?"#,
                    rusqlite::params![new_run_id, now.to_rfc3339(), ticket_id, old_run_id,],
                )?
            };

            if affected == 0 {
                return Err(DbError::NotFound(format!(
                    "Ticket lock not found or not owned by run {}",
                    old_run_id
                )));
            }
            Ok(())
        })
    }

    /// Atomically reserve the next available ticket from the Ready column.
    ///
    /// This method uses a single UPDATE...WHERE statement to atomically find and lock
    /// a ticket, preventing race conditions where multiple workers might grab the same ticket.
    ///
    /// Returns Some(ticket) if a ticket was reserved, None if no tickets are available.
    pub fn reserve_next_ticket(
        &self,
        project_filter: Option<&str>,
        _agent_id: &str,
        run_id: &str,
        lock_expires_at: DateTime<Utc>,
    ) -> Result<Option<Ticket>, DbError> {
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            let now = Utc::now();
            let now_str = now.to_rfc3339();
            let expires_str = lock_expires_at.to_rfc3339();

            // Excludes epics (workers process children) and paused tickets
            let affected = tx.execute(
                r#"UPDATE tickets 
                   SET locked_by_run_id = ?1, lock_expires_at = ?2, updated_at = ?3
                   WHERE id = (
                       SELECT t.id FROM tickets t
                       JOIN columns c ON t.column_id = c.id
                       WHERE c.name = 'Ready'
                         AND t.is_epic = 0
                         AND t.paused_at IS NULL
                         AND (t.locked_by_run_id IS NULL OR t.lock_expires_at < ?3)
                         AND (?4 IS NULL OR t.project_id = ?4)
                       ORDER BY 
                         CASE t.priority 
                           WHEN 'urgent' THEN 0 
                           WHEN 'high' THEN 1 
                           WHEN 'medium' THEN 2 
                           WHEN 'low' THEN 3 
                         END,
                         t.created_at ASC
                       LIMIT 1
                   )
                   AND (locked_by_run_id IS NULL OR lock_expires_at < ?3)"#,
                rusqlite::params![run_id, expires_str, now_str, project_filter],
            )?;

            if affected == 0 {
                tx.commit()?;
                return Ok(None);
            }

            let ticket = tx.query_row(
                r#"SELECT id, board_id, column_id, title, description_md, priority, 
                          labels_json, created_at, updated_at, locked_by_run_id, 
                          lock_expires_at, project_id, workspace_id, workflow_type, model, branch_name,
                          is_epic, epic_id, order_in_epic, depends_on_epic_id, depends_on_epic_ids_json, spec_version_id,
                          paused_at, paused_at_stage, paused_run_id
                   FROM tickets WHERE locked_by_run_id = ?1
                   LIMIT 1"#,
                [run_id],
                Self::map_ticket_row,
            )?;

            tx.commit()?;
            Ok(Some(ticket))
        })
    }

    /// Get diagnostic info about why tickets might not be reserved.
    /// Returns counts of tickets in various states for debugging.
    pub fn get_ready_ticket_diagnostics(
        &self,
        project_filter: Option<&str>,
        _agent_id: &str,
    ) -> Result<ReadyTicketDiagnostics, DbError> {
        self.with_conn(|conn| {
            let now_str = Utc::now().to_rfc3339();

            // Count tickets in Ready column
            let total_ready: i64 = conn
                .query_row(
                    r#"SELECT COUNT(*) FROM tickets t
                   JOIN columns c ON t.column_id = c.id
                   WHERE c.name = 'Ready'"#,
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            // Count paused tickets in Ready
            let paused: i64 = conn
                .query_row(
                    r#"SELECT COUNT(*) FROM tickets t
                   JOIN columns c ON t.column_id = c.id
                   WHERE c.name = 'Ready' AND t.paused_at IS NOT NULL"#,
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            // Count locked tickets in Ready
            let locked: i64 = conn
                .query_row(
                    r#"SELECT COUNT(*) FROM tickets t
                   JOIN columns c ON t.column_id = c.id
                   WHERE c.name = 'Ready' 
                     AND t.locked_by_run_id IS NOT NULL 
                     AND t.lock_expires_at >= ?"#,
                    [&now_str],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            // Count epics in Ready
            let epics: i64 = conn
                .query_row(
                    r#"SELECT COUNT(*) FROM tickets t
                   JOIN columns c ON t.column_id = c.id
                   WHERE c.name = 'Ready' AND t.is_epic = 1"#,
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            // Count with wrong project
            let wrong_project: i64 = if let Some(proj_id) = project_filter {
                conn.query_row(
                    r#"SELECT COUNT(*) FROM tickets t
                       JOIN columns c ON t.column_id = c.id
                       WHERE c.name = 'Ready' 
                         AND t.is_epic = 0 
                         AND t.paused_at IS NULL
                         AND (t.locked_by_run_id IS NULL OR t.lock_expires_at < ?)
                         AND t.project_id != ?"#,
                    rusqlite::params![&now_str, proj_id],
                    |row| row.get(0),
                )
                .unwrap_or(0)
            } else {
                0
            };

            // Count eligible (what reserve_next_ticket would match)
            let eligible: i64 = conn
                .query_row(
                    r#"SELECT COUNT(*) FROM tickets t
                   JOIN columns c ON t.column_id = c.id
                   WHERE c.name = 'Ready'
                     AND t.is_epic = 0
                     AND t.paused_at IS NULL
                     AND (t.locked_by_run_id IS NULL OR t.lock_expires_at < ?)
                     AND (? IS NULL OR t.project_id = ?)"#,
                    rusqlite::params![&now_str, project_filter, project_filter],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            Ok(ReadyTicketDiagnostics {
                total_ready,
                paused,
                locked,
                epics,
                wrong_project,
                eligible,
            })
        })
    }
}
