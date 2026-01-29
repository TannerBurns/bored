use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;
use crate::db::{Database, DbError, parse_datetime};
use crate::db::models::{Ticket, CreateTicket, UpdateTicket, Priority, AgentPref, WorkflowType, CreateTask, TaskType};
use crate::agents::AgentKind;

impl Database {
    pub fn get_ticket(&self, ticket_id: &str) -> Result<Ticket, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, column_id, title, description_md, priority, 
                          labels_json, created_at, updated_at, locked_by_run_id, 
                          lock_expires_at, project_id, agent_pref, workflow_type, model, branch_name,
                          is_epic, epic_id, order_in_epic
                   FROM tickets WHERE id = ?"#
            )?;
            
            stmt.query_row([ticket_id], Self::map_ticket_row)
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        DbError::NotFound(format!("Ticket {}", ticket_id))
                    }
                    other => DbError::Sqlite(other),
                })
        })
    }

    pub fn update_ticket(&self, ticket_id: &str, updates: &UpdateTicket) -> Result<Ticket, DbError> {
        self.with_conn(|conn| {
            // First get the existing ticket
            let existing = {
                let mut stmt = conn.prepare(
                    r#"SELECT id, board_id, column_id, title, description_md, priority, 
                              labels_json, created_at, updated_at, locked_by_run_id, 
                              lock_expires_at, project_id, agent_pref, workflow_type, model, branch_name,
                              is_epic, epic_id, order_in_epic
                       FROM tickets WHERE id = ?"#
                )?;
                stmt.query_row([ticket_id], Self::map_ticket_row)
                    .map_err(|e| match e {
                        rusqlite::Error::QueryReturnedNoRows => {
                            DbError::NotFound(format!("Ticket {}", ticket_id))
                        }
                        other => DbError::Sqlite(other),
                    })?
            };

            let now = chrono::Utc::now();
            let title = updates.title.as_ref().unwrap_or(&existing.title);
            let description_md = updates.description_md.as_ref().unwrap_or(&existing.description_md);
            let priority = updates.priority.as_ref().unwrap_or(&existing.priority);
            let labels = updates.labels.as_ref().unwrap_or(&existing.labels);
            // Handle project_id: None means keep existing, Some("") means clear, Some(id) means set
            let project_id = match &updates.project_id {
                Some(id) if id.is_empty() => None, // Empty string means clear the project
                Some(id) => Some(id.as_str()),
                None => existing.project_id.as_deref(), // Keep existing
            };
            let agent_pref = updates.agent_pref.as_ref().or(existing.agent_pref.as_ref());
            let workflow_type = updates.workflow_type.as_ref().unwrap_or(&existing.workflow_type);
            // Handle model: None means keep existing, Some("") means clear, Some(value) means set
            let model = match &updates.model {
                Some(m) if m.is_empty() => None, // Empty string means clear the model
                Some(m) => Some(m.as_str()),
                None => existing.model.as_deref(), // Keep existing
            };
            // Handle branch_name: None means keep existing, Some("") means clear, Some(value) means set
            let branch_name = match &updates.branch_name {
                Some(b) if b.is_empty() => None, // Empty string means clear the branch
                Some(b) => Some(b.as_str()),
                None => existing.branch_name.as_deref(), // Keep existing
            };
            // Handle column_id: None means keep existing, Some(id) means set
            let column_id = updates.column_id.as_ref().unwrap_or(&existing.column_id);
            // Handle is_epic: None means keep existing, Some(value) means set
            let is_epic = updates.is_epic.unwrap_or(existing.is_epic);
            // Handle epic_id: None means keep existing, Some("") means clear, Some(id) means set
            let epic_id = match &updates.epic_id {
                Some(id) if id.is_empty() => None,
                Some(id) => Some(id.as_str()),
                None => existing.epic_id.as_deref(),
            };
            // Handle order_in_epic: None means keep existing, Some(value) means set
            let order_in_epic = updates.order_in_epic.or(existing.order_in_epic);

            let labels_json = serde_json::to_string(labels).unwrap_or_else(|_| "[]".to_string());

            conn.execute(
                r#"UPDATE tickets 
                   SET title = ?, description_md = ?, priority = ?, labels_json = ?,
                       project_id = ?, agent_pref = ?, workflow_type = ?, model = ?, branch_name = ?, 
                       column_id = ?, is_epic = ?, epic_id = ?, order_in_epic = ?, updated_at = ?
                   WHERE id = ?"#,
                rusqlite::params![
                    title,
                    description_md,
                    priority.as_str(),
                    labels_json,
                    project_id,
                    agent_pref.map(|p| p.as_str()),
                    workflow_type.as_str(),
                    model,
                    branch_name,
                    column_id,
                    is_epic,
                    epic_id,
                    order_in_epic,
                    now.to_rfc3339(),
                    ticket_id,
                ],
            )?;

            // Re-query within the same connection to avoid deadlock
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, column_id, title, description_md, priority, 
                          labels_json, created_at, updated_at, locked_by_run_id, 
                          lock_expires_at, project_id, agent_pref, workflow_type, model, branch_name,
                          is_epic, epic_id, order_in_epic
                   FROM tickets WHERE id = ?"#
            )?;
            stmt.query_row([ticket_id], Self::map_ticket_row)
                .map_err(DbError::Sqlite)
        })
    }

    pub fn delete_ticket(&self, ticket_id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let affected = conn.execute(
                "DELETE FROM tickets WHERE id = ?",
                [ticket_id],
            )?;
            
            if affected == 0 {
                return Err(DbError::NotFound(format!("Ticket {}", ticket_id)));
            }
            Ok(())
        })
    }

    /// Attempt to lock a ticket for an agent run.
    /// 
    /// This method uses atomic locking semantics: it only acquires the lock if:
    /// - The ticket is not currently locked (locked_by_run_id IS NULL), OR
    /// - The existing lock has expired (lock_expires_at < now)
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
            
            // Atomically acquire lock only if not held by another run
            let affected = conn.execute(
                r#"UPDATE tickets 
                   SET locked_by_run_id = ?, lock_expires_at = ?, updated_at = ?
                   WHERE id = ? 
                     AND (locked_by_run_id IS NULL OR lock_expires_at < ?)"#,
                rusqlite::params![
                    run_id,
                    expires_at.to_rfc3339(),
                    now_str,
                    ticket_id,
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
                    rusqlite::params![
                        new_run_id,
                        now.to_rfc3339(),
                        ticket_id,
                        old_run_id,
                    ],
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
        agent_type: AgentKind,
        run_id: &str,
        lock_expires_at: DateTime<Utc>,
    ) -> Result<Option<Ticket>, DbError> {
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            let now = Utc::now();
            let now_str = now.to_rfc3339();
            let expires_str = lock_expires_at.to_rfc3339();
            
            let agent_type_str = agent_type.as_str();
            
            // Subquery finds next ticket; outer WHERE double-checks lock status for atomicity
            // NOTE: Epics are excluded (is_epic = 0) because workers should process child tickets,
            // not the epic container itself. The epic orchestrates its children through lifecycle hooks.
            let affected = tx.execute(
                r#"UPDATE tickets 
                   SET locked_by_run_id = ?1, lock_expires_at = ?2, updated_at = ?3
                   WHERE id = (
                       SELECT t.id FROM tickets t
                       JOIN columns c ON t.column_id = c.id
                       WHERE c.name = 'Ready'
                         AND t.is_epic = 0
                         AND (t.locked_by_run_id IS NULL OR t.lock_expires_at < ?3)
                         AND (?4 IS NULL OR t.project_id = ?4)
                         AND (
                             t.agent_pref IS NULL 
                             OR t.agent_pref = 'any' 
                             OR t.agent_pref = ?5
                         )
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
                rusqlite::params![run_id, expires_str, now_str, project_filter, agent_type_str],
            )?;
            
            if affected == 0 {
                tx.commit()?;
                return Ok(None);
            }
            
            let ticket = tx.query_row(
                r#"SELECT id, board_id, column_id, title, description_md, priority, 
                          labels_json, created_at, updated_at, locked_by_run_id, 
                          lock_expires_at, project_id, agent_pref, workflow_type, model, branch_name,
                          is_epic, epic_id, order_in_epic
                   FROM tickets WHERE locked_by_run_id = ?1
                   LIMIT 1"#,
                [run_id],
                Self::map_ticket_row,
            )?;
            
            tx.commit()?;
            Ok(Some(ticket))
        })
    }

    pub fn create_ticket(&self, ticket: &CreateTicket) -> Result<Ticket, DbError> {
        // If this is a child of an epic, calculate the order_in_epic
        let order_in_epic = if let Some(ref epic_id) = ticket.epic_id {
            // Get the current max order for children of this epic
            self.with_conn(|conn| {
                let max_order: Option<i32> = conn.query_row(
                    "SELECT MAX(order_in_epic) FROM tickets WHERE epic_id = ?",
                    [epic_id],
                    |row| row.get(0),
                ).unwrap_or(None);
                Ok::<_, DbError>(Some(max_order.unwrap_or(-1) + 1))
            })?
        } else {
            None
        };

        let created_ticket = self.with_conn(|conn| {
            let ticket_id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now();
            let labels_json = serde_json::to_string(&ticket.labels).unwrap_or_else(|_| "[]".to_string());
            
            conn.execute(
                r#"INSERT INTO tickets 
                   (id, board_id, column_id, title, description_md, priority, labels_json, 
                    created_at, updated_at, project_id, agent_pref, workflow_type, model, branch_name,
                    is_epic, epic_id, order_in_epic)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
                rusqlite::params![
                    ticket_id,
                    ticket.board_id,
                    ticket.column_id,
                    ticket.title,
                    ticket.description_md,
                    ticket.priority.as_str(),
                    labels_json,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                    ticket.project_id,
                    ticket.agent_pref.as_ref().map(|p| p.as_str()),
                    ticket.workflow_type.as_str(),
                    ticket.model,
                    ticket.branch_name,
                    ticket.is_epic,
                    ticket.epic_id,
                    order_in_epic,
                ],
            )?;

            Ok(Ticket {
                id: ticket_id,
                board_id: ticket.board_id.clone(),
                column_id: ticket.column_id.clone(),
                title: ticket.title.clone(),
                description_md: ticket.description_md.clone(),
                priority: ticket.priority.clone(),
                labels: ticket.labels.clone(),
                created_at: now,
                updated_at: now,
                locked_by_run_id: None,
                lock_expires_at: None,
                project_id: ticket.project_id.clone(),
                agent_pref: ticket.agent_pref.clone(),
                workflow_type: ticket.workflow_type.clone(),
                model: ticket.model.clone(),
                branch_name: ticket.branch_name.clone(),
                is_epic: ticket.is_epic,
                epic_id: ticket.epic_id.clone(),
                order_in_epic,
            })
        })?;
        
        // Auto-create Task 1 from the ticket description
        // This is the initial task that defines the work to be done
        // CRITICAL: Every ticket MUST have at least one task. Workers expect this invariant.
        // If task creation fails, we must delete the ticket and return an error to maintain consistency.
        //
        // UTF-8 handling: chars().count() counts Unicode code points (not bytes), which is
        // consistent with SQLite's length() function used in the V8 migration. Both correctly
        // handle multi-byte UTF-8 characters like emoji. Extended grapheme clusters (e.g., 
        // emoji with skin tone modifiers) are counted as multiple code points by both.
        let task_title = if created_ticket.title.chars().count() > 50 {
            format!("{}...", created_ticket.title.chars().take(47).collect::<String>())
        } else {
            created_ticket.title.clone()
        };
        
        if let Err(e) = self.create_task(&CreateTask {
            ticket_id: created_ticket.id.clone(),
            task_type: TaskType::Custom,
            title: Some(task_title),
            content: if created_ticket.description_md.is_empty() {
                None
            } else {
                Some(created_ticket.description_md.clone())
            },
        }) {
            // Task creation failed - delete the ticket to maintain invariant
            tracing::error!(
                "Failed to create initial task for ticket {}: {}. Deleting ticket to maintain invariant.",
                created_ticket.id, e
            );
            if let Err(delete_err) = self.delete_ticket(&created_ticket.id) {
                tracing::error!("Failed to delete ticket {} after task creation failure: {}", created_ticket.id, delete_err);
            }
            return Err(DbError::Validation(format!(
                "Failed to create initial task for ticket: {}. Ticket creation aborted.",
                e
            )));
        }
        
        Ok(created_ticket)
    }

    pub fn get_tickets(&self, board_id: &str, column_id: Option<&str>) -> Result<Vec<Ticket>, DbError> {
        self.with_conn(|conn| {
            let sql = match column_id {
                Some(_) => {
                    "SELECT id, board_id, column_id, title, description_md, priority, 
                            labels_json, created_at, updated_at, locked_by_run_id, 
                            lock_expires_at, project_id, agent_pref, workflow_type, model, branch_name,
                            is_epic, epic_id, order_in_epic
                     FROM tickets WHERE board_id = ? AND column_id = ? ORDER BY created_at"
                }
                None => {
                    "SELECT id, board_id, column_id, title, description_md, priority, 
                            labels_json, created_at, updated_at, locked_by_run_id, 
                            lock_expires_at, project_id, agent_pref, workflow_type, model, branch_name,
                            is_epic, epic_id, order_in_epic
                     FROM tickets WHERE board_id = ? ORDER BY created_at"
                }
            };

            let mut stmt = conn.prepare(sql)?;
            
            let rows = match column_id {
                Some(col_id) => stmt.query_map(rusqlite::params![board_id, col_id], Self::map_ticket_row)?,
                None => stmt.query_map([board_id], Self::map_ticket_row)?,
            };

            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

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

    pub fn set_ticket_project(&self, ticket_id: &str, project_id: Option<&str>) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "UPDATE tickets SET project_id = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![project_id, now, ticket_id],
            )?;
            Ok(())
        })
    }

    fn map_ticket_row(row: &rusqlite::Row) -> rusqlite::Result<Ticket> {
        let labels_json: String = row.get(6)?;
        let labels: Vec<String> = serde_json::from_str(&labels_json).unwrap_or_default();
        
        let priority_str: String = row.get(5)?;
        let priority = Priority::parse(&priority_str).unwrap_or(Priority::Medium);
        
        let agent_pref_str: Option<String> = row.get(12)?;
        let agent_pref = agent_pref_str.and_then(|s| AgentPref::parse(&s));
        
        let workflow_type_str: String = row.get::<_, Option<String>>(13)?.unwrap_or_else(|| "basic".to_string());
        let workflow_type = WorkflowType::parse(&workflow_type_str).unwrap_or_default();
        
        let model: Option<String> = row.get(14)?;
        let branch_name: Option<String> = row.get(15)?;
        
        // Epic fields (columns 16, 17, 18)
        let is_epic: bool = row.get::<_, i32>(16).unwrap_or(0) != 0;
        let epic_id: Option<String> = row.get(17)?;
        let order_in_epic: Option<i32> = row.get(18)?;

        Ok(Ticket {
            id: row.get(0)?,
            board_id: row.get(1)?,
            column_id: row.get(2)?,
            title: row.get(3)?,
            description_md: row.get(4)?,
            priority,
            labels,
            created_at: parse_datetime(row.get(7)?),
            updated_at: parse_datetime(row.get(8)?),
            locked_by_run_id: row.get(9)?,
            lock_expires_at: row.get::<_, Option<String>>(10)?.map(parse_datetime),
            project_id: row.get(11)?,
            agent_pref,
            workflow_type,
            model,
            branch_name,
            is_epic,
            epic_id,
            order_in_epic,
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

    // ===== Epic Operations =====

    /// Get all children of an epic, ordered by order_in_epic
    pub fn get_epic_children(&self, epic_id: &str) -> Result<Vec<Ticket>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, column_id, title, description_md, priority, 
                          labels_json, created_at, updated_at, locked_by_run_id, 
                          lock_expires_at, project_id, agent_pref, workflow_type, model, branch_name,
                          is_epic, epic_id, order_in_epic
                   FROM tickets WHERE epic_id = ?
                   ORDER BY order_in_epic ASC, created_at ASC"#
            )?;
            
            let rows = stmt.query_map([epic_id], Self::map_ticket_row)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    /// Get the next pending child ticket for an epic (first child in Backlog)
    pub fn get_next_pending_child(&self, epic_id: &str) -> Result<Option<Ticket>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT t.id, t.board_id, t.column_id, t.title, t.description_md, t.priority, 
                          t.labels_json, t.created_at, t.updated_at, t.locked_by_run_id, 
                          t.lock_expires_at, t.project_id, t.agent_pref, t.workflow_type, t.model, t.branch_name,
                          t.is_epic, t.epic_id, t.order_in_epic
                   FROM tickets t
                   JOIN columns c ON t.column_id = c.id
                   WHERE t.epic_id = ? AND c.name = 'Backlog'
                   ORDER BY t.order_in_epic ASC, t.created_at ASC
                   LIMIT 1"#
            )?;
            
            stmt.query_row([epic_id], Self::map_ticket_row)
                .optional()
                .map_err(DbError::from)
        })
    }

    /// Get progress stats for an epic's children
    pub fn get_epic_progress(&self, epic_id: &str) -> Result<crate::db::models::EpicProgress, DbError> {
        use crate::db::models::EpicProgress;
        
        self.with_conn(|conn| {
            let mut progress = EpicProgress::default();
            
            let mut stmt = conn.prepare(
                r#"SELECT c.name, COUNT(*) as cnt
                   FROM tickets t
                   JOIN columns c ON t.column_id = c.id
                   WHERE t.epic_id = ?
                   GROUP BY c.name"#
            )?;
            
            let rows = stmt.query_map([epic_id], |row| {
                let name: String = row.get(0)?;
                let count: i32 = row.get(1)?;
                Ok((name, count))
            })?;
            
            for row in rows {
                let (name, count) = row?;
                progress.total += count;
                match name.as_str() {
                    "Backlog" => progress.backlog = count,
                    "Ready" => progress.ready = count,
                    "In Progress" => progress.in_progress = count,
                    "Blocked" => progress.blocked = count,
                    "Review" => progress.review = count,
                    "Done" => progress.done = count,
                    _ => {} // Unknown column
                }
            }
            
            Ok(progress)
        })
    }

    /// Add an existing ticket to an epic as a child
    pub fn add_ticket_to_epic(&self, epic_id: &str, ticket_id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            // Verify epic exists and is actually an epic
            let is_epic: bool = conn.query_row(
                "SELECT is_epic FROM tickets WHERE id = ?",
                [epic_id],
                |row| row.get::<_, i32>(0),
            ).map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(format!("Epic {} not found", epic_id)),
                other => DbError::Sqlite(other),
            })? != 0;
            
            if !is_epic {
                return Err(DbError::Validation(format!("Ticket {} is not an epic", epic_id)));
            }
            
            // Get current max order
            let max_order: Option<i32> = conn.query_row(
                "SELECT MAX(order_in_epic) FROM tickets WHERE epic_id = ?",
                [epic_id],
                |row| row.get(0),
            ).unwrap_or(None);
            
            let order = max_order.unwrap_or(-1) + 1;
            let now = chrono::Utc::now().to_rfc3339();
            
            let affected = conn.execute(
                "UPDATE tickets SET epic_id = ?, order_in_epic = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![epic_id, order, now, ticket_id],
            )?;
            
            if affected == 0 {
                return Err(DbError::NotFound(format!("Ticket {} not found", ticket_id)));
            }
            
            Ok(())
        })
    }

    /// Remove a ticket from its parent epic
    pub fn remove_ticket_from_epic(&self, ticket_id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            
            conn.execute(
                "UPDATE tickets SET epic_id = NULL, order_in_epic = NULL, updated_at = ? WHERE id = ?",
                rusqlite::params![now, ticket_id],
            )?;
            
            Ok(())
        })
    }

    /// Reorder children within an epic
    /// child_ids should be the list of ticket IDs in the desired order
    pub fn reorder_epic_children(&self, epic_id: &str, child_ids: &[String]) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            
            for (index, ticket_id) in child_ids.iter().enumerate() {
                conn.execute(
                    "UPDATE tickets SET order_in_epic = ?, updated_at = ? WHERE id = ? AND epic_id = ?",
                    rusqlite::params![index as i32, now, ticket_id, epic_id],
                )?;
            }
            
            Ok(())
        })
    }

    /// Check if all children of an epic are in Done column
    pub fn are_all_epic_children_done(&self, epic_id: &str) -> Result<bool, DbError> {
        self.with_conn(|conn| {
            // Count children not in Done
            let not_done: i32 = conn.query_row(
                r#"SELECT COUNT(*) FROM tickets t
                   JOIN columns c ON t.column_id = c.id
                   WHERE t.epic_id = ? AND c.name != 'Done'"#,
                [epic_id],
                |row| row.get(0),
            )?;
            
            // Also check there's at least one child
            let total: i32 = conn.query_row(
                "SELECT COUNT(*) FROM tickets WHERE epic_id = ?",
                [epic_id],
                |row| row.get(0),
            )?;
            
            Ok(total > 0 && not_done == 0)
        })
    }

    /// Check if any child of an epic is blocked
    pub fn has_blocked_child(&self, epic_id: &str) -> Result<bool, DbError> {
        self.with_conn(|conn| {
            let blocked: i32 = conn.query_row(
                r#"SELECT COUNT(*) FROM tickets t
                   JOIN columns c ON t.column_id = c.id
                   WHERE t.epic_id = ? AND c.name = 'Blocked'"#,
                [epic_id],
                |row| row.get(0),
            )?;
            
            Ok(blocked > 0)
        })
    }

    /// Get the previous sibling of a child ticket in an epic (for chain branching)
    /// Returns the ticket that is one position before this ticket in the epic's order
    pub fn get_previous_epic_sibling(&self, ticket_id: &str) -> Result<Option<Ticket>, DbError> {
        self.with_conn(|conn| {
            // First, get this ticket's epic_id and order_in_epic
            let ticket_info: Option<(String, i32)> = conn.query_row(
                "SELECT epic_id, order_in_epic FROM tickets WHERE id = ? AND epic_id IS NOT NULL",
                [ticket_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            ).optional()?;
            
            let (epic_id, order) = match ticket_info {
                Some((eid, ord)) => (eid, ord),
                None => return Ok(None), // Not a child of an epic
            };
            
            if order == 0 {
                return Ok(None); // First child, no previous sibling
            }
            
            // Get the previous sibling (order_in_epic = order - 1)
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, column_id, title, description_md, priority,
                          labels_json, created_at, updated_at, locked_by_run_id, 
                          lock_expires_at, project_id, agent_pref, workflow_type, model, branch_name,
                          is_epic, epic_id, order_in_epic
                   FROM tickets WHERE epic_id = ? AND order_in_epic = ?"#
            )?;
            
            stmt.query_row(rusqlite::params![epic_id, order - 1], Self::map_ticket_row)
                .optional()
                .map_err(DbError::from)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::CreateProject;

    fn create_test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn temp_dir_path() -> String {
        std::env::temp_dir().to_string_lossy().to_string()
    }
    
    fn setup_board_with_ready_ticket(db: &Database) -> (String, String, Ticket) {
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let ready_column = columns.iter().find(|c| c.name == "Ready").unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: ready_column.id.clone(),
            title: "Test Ticket".to_string(),
            description_md: "Description".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        (board.id, ready_column.id.clone(), ticket)
    }

    #[test]
    fn create_ticket_with_all_fields() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Test Ticket".to_string(),
            description_md: "Description".to_string(),
            priority: Priority::High,
            labels: vec!["bug".to_string()],
            project_id: None,
            agent_pref: Some(AgentPref::Cursor),
            workflow_type: WorkflowType::MultiStage,
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        assert_eq!(ticket.title, "Test Ticket");
        assert_eq!(ticket.priority, Priority::High);
        assert_eq!(ticket.labels, vec!["bug"]);
        assert_eq!(ticket.agent_pref, Some(AgentPref::Cursor));
        assert_eq!(ticket.workflow_type, WorkflowType::MultiStage);
    }

    #[test]
    fn get_tickets_for_board() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Ticket 1".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        let tickets = db.get_tickets(&board.id, None).unwrap();
        assert_eq!(tickets.len(), 1);
    }

    #[test]
    fn move_ticket_to_column() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Ticket".to_string(),
            description_md: "".to_string(),
            priority: Priority::Low,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        db.move_ticket(&ticket.id, &columns[1].id).unwrap();
        
        let tickets = db.get_tickets(&board.id, Some(&columns[1].id)).unwrap();
        assert_eq!(tickets.len(), 1);
        assert_eq!(tickets[0].id, ticket.id);
    }

    #[test]
    fn set_ticket_project() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let project = db.create_project(&CreateProject {
            name: "Proj".to_string(),
            path: temp_dir_path(),
            preferred_agent: None,
            requires_git: true,
        }).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Ticket".to_string(),
            description_md: "".to_string(),
            priority: Priority::Low,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        db.set_ticket_project(&ticket.id, Some(&project.id)).unwrap();
        
        let tickets = db.get_tickets(&board.id, None).unwrap();
        assert_eq!(tickets[0].project_id, Some(project.id));
    }

    #[test]
    fn get_ticket_by_id() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let created = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "My Ticket".to_string(),
            description_md: "Description".to_string(),
            priority: Priority::High,
            labels: vec!["test".to_string()],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        let fetched = db.get_ticket(&created.id).unwrap();
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.title, "My Ticket");
        assert_eq!(fetched.priority, Priority::High);
    }

    #[test]
    fn get_ticket_not_found() {
        let db = create_test_db();
        let result = db.get_ticket("nonexistent");
        assert!(matches!(result, Err(DbError::NotFound(_))));
    }

    #[test]
    fn update_ticket_partial() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Original".to_string(),
            description_md: "Desc".to_string(),
            priority: Priority::Low,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        let updated = db.update_ticket(&ticket.id, &UpdateTicket {
            title: Some("Updated Title".to_string()),
            description_md: None,
            priority: Some(Priority::Urgent),
            labels: None,
            project_id: None,
            agent_pref: None,
            workflow_type: None,
            model: None,
            branch_name: None,
            column_id: None,
            is_epic: None,
            epic_id: None,
            order_in_epic: None,
        }).unwrap();
        
        assert_eq!(updated.title, "Updated Title");
        assert_eq!(updated.description_md, "Desc");
        assert_eq!(updated.priority, Priority::Urgent);
    }

    #[test]
    fn update_ticket_not_found() {
        let db = create_test_db();
        let result = db.update_ticket("nonexistent", &UpdateTicket {
            title: Some("New".to_string()),
            column_id: None,
            description_md: None,
            priority: None,
            labels: None,
            project_id: None,
            agent_pref: None,
            workflow_type: None,
            model: None,
            branch_name: None,
            is_epic: None,
            epic_id: None,
            order_in_epic: None,
        });
        assert!(matches!(result, Err(DbError::NotFound(_))));
    }

    #[test]
    fn update_ticket_clears_project_with_empty_string() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let project = db.create_project(&CreateProject {
            name: "Test Project".to_string(),
            path: temp_dir_path(),
            preferred_agent: None,
            requires_git: true,
        }).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Ticket".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: Some(project.id.clone()),
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        assert_eq!(ticket.project_id, Some(project.id.clone()));
        
        let updated = db.update_ticket(&ticket.id, &UpdateTicket {
            title: None,
            description_md: None,
            priority: None,
            labels: None,
            project_id: Some(String::new()), // Empty string clears project
            agent_pref: None,
            workflow_type: None,
            model: None,
            branch_name: None,
            column_id: None,
            is_epic: None,
            epic_id: None,
            order_in_epic: None,
        }).unwrap();
        
        assert_eq!(updated.project_id, None);
    }

    #[test]
    fn update_ticket_keeps_project_when_none() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let project = db.create_project(&CreateProject {
            name: "Test Project".to_string(),
            path: temp_dir_path(),
            preferred_agent: None,
            requires_git: true,
        }).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Ticket".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: Some(project.id.clone()),
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        let updated = db.update_ticket(&ticket.id, &UpdateTicket {
            title: Some("Updated Title".to_string()),
            description_md: None,
            priority: None,
            labels: None,
            project_id: None, // None means keep existing
            agent_pref: None,
            workflow_type: None,
            model: None,
            branch_name: None,
            column_id: None,
            is_epic: None,
            epic_id: None,
            order_in_epic: None,
        }).unwrap();
        
        assert_eq!(updated.project_id, Some(project.id));
        assert_eq!(updated.title, "Updated Title");
    }

    #[test]
    fn delete_ticket_success() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "ToDelete".to_string(),
            description_md: "".to_string(),
            priority: Priority::Low,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        db.delete_ticket(&ticket.id).unwrap();
        
        let result = db.get_ticket(&ticket.id);
        assert!(matches!(result, Err(DbError::NotFound(_))));
    }

    #[test]
    fn delete_ticket_not_found() {
        let db = create_test_db();
        let result = db.delete_ticket("nonexistent");
        assert!(matches!(result, Err(DbError::NotFound(_))));
    }

    #[test]
    fn lock_and_unlock_ticket() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Lockable".to_string(),
            description_md: "".to_string(),
            priority: Priority::Low,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        let expires = chrono::Utc::now() + chrono::Duration::minutes(30);
        db.lock_ticket(&ticket.id, "run-123", expires).unwrap();
        
        let locked = db.get_ticket(&ticket.id).unwrap();
        assert_eq!(locked.locked_by_run_id, Some("run-123".to_string()));
        assert!(locked.lock_expires_at.is_some());
        
        db.unlock_ticket(&ticket.id).unwrap();
        
        let unlocked = db.get_ticket(&ticket.id).unwrap();
        assert!(unlocked.locked_by_run_id.is_none());
        assert!(unlocked.lock_expires_at.is_none());
    }

    #[test]
    fn extend_lock_success() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Extendable".to_string(),
            description_md: "".to_string(),
            priority: Priority::Low,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        let initial_expires = chrono::Utc::now() + chrono::Duration::minutes(30);
        db.lock_ticket(&ticket.id, "run-456", initial_expires).unwrap();
        
        let new_expires = chrono::Utc::now() + chrono::Duration::minutes(60);
        db.extend_lock(&ticket.id, "run-456", new_expires).unwrap();
        
        let extended = db.get_ticket(&ticket.id).unwrap();
        assert!(extended.lock_expires_at.unwrap() > initial_expires);
    }

    #[test]
    fn extend_lock_wrong_run() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Locked".to_string(),
            description_md: "".to_string(),
            priority: Priority::Low,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        let expires = chrono::Utc::now() + chrono::Duration::minutes(30);
        db.lock_ticket(&ticket.id, "run-correct", expires).unwrap();
        
        let result = db.extend_lock(&ticket.id, "run-wrong", expires);
        assert!(matches!(result, Err(DbError::NotFound(_))));
    }

    #[test]
    fn lock_ticket_fails_when_already_locked() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Ticket".to_string(),
            description_md: "".to_string(),
            priority: Priority::Low,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        let expires = chrono::Utc::now() + chrono::Duration::minutes(30);
        
        // First lock should succeed
        db.lock_ticket(&ticket.id, "run-1", expires).unwrap();
        
        // Second lock attempt should fail (ticket is already locked with valid lock)
        let result = db.lock_ticket(&ticket.id, "run-2", expires);
        assert!(matches!(result, Err(DbError::Validation(_))));
        
        // Original lock should still be in place
        let locked = db.get_ticket(&ticket.id).unwrap();
        assert_eq!(locked.locked_by_run_id, Some("run-1".to_string()));
    }

    #[test]
    fn lock_ticket_succeeds_when_lock_expired() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Ticket".to_string(),
            description_md: "".to_string(),
            priority: Priority::Low,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        // Lock with an already-expired timestamp
        let expired = chrono::Utc::now() - chrono::Duration::minutes(5);
        db.lock_ticket(&ticket.id, "run-1", expired).unwrap();
        
        // Second lock should succeed because the first lock has expired
        let new_expires = chrono::Utc::now() + chrono::Duration::minutes(30);
        db.lock_ticket(&ticket.id, "run-2", new_expires).unwrap();
        
        // New lock should be in place
        let locked = db.get_ticket(&ticket.id).unwrap();
        assert_eq!(locked.locked_by_run_id, Some("run-2".to_string()));
    }

    #[test]
    fn lock_ticket_not_found() {
        let db = create_test_db();
        let expires = chrono::Utc::now() + chrono::Duration::minutes(30);
        let result = db.lock_ticket("nonexistent", "run-1", expires);
        assert!(matches!(result, Err(DbError::NotFound(_))));
    }

    #[test]
    fn release_lock_correct_run() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Ticket".to_string(),
            description_md: "".to_string(),
            priority: Priority::Low,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        let expires = chrono::Utc::now() + chrono::Duration::minutes(30);
        db.lock_ticket(&ticket.id, "run-123", expires).unwrap();
        
        let locked = db.get_ticket(&ticket.id).unwrap();
        assert_eq!(locked.locked_by_run_id, Some("run-123".to_string()));
        
        db.release_lock(&ticket.id, "run-123").unwrap();
        
        let released = db.get_ticket(&ticket.id).unwrap();
        assert!(released.locked_by_run_id.is_none());
        assert!(released.lock_expires_at.is_none());
    }

    #[test]
    fn release_lock_wrong_run_no_effect() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Ticket".to_string(),
            description_md: "".to_string(),
            priority: Priority::Low,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        let expires = chrono::Utc::now() + chrono::Duration::minutes(30);
        db.lock_ticket(&ticket.id, "run-correct", expires).unwrap();
        
        // Try to release with wrong run_id - should have no effect
        db.release_lock(&ticket.id, "run-wrong").unwrap();
        
        let still_locked = db.get_ticket(&ticket.id).unwrap();
        assert_eq!(still_locked.locked_by_run_id, Some("run-correct".to_string()));
    }
    
    #[test]
    fn update_ticket_lock_owner_success() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Ticket".to_string(),
            description_md: "".to_string(),
            priority: Priority::Low,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        let expires = chrono::Utc::now() + chrono::Duration::minutes(30);
        db.lock_ticket(&ticket.id, "temp-run-id", expires).unwrap();
        
        // Update lock owner to new run ID
        let new_expires = chrono::Utc::now() + chrono::Duration::minutes(60);
        db.update_ticket_lock_owner(&ticket.id, "temp-run-id", "actual-run-id", Some(new_expires)).unwrap();
        
        let updated = db.get_ticket(&ticket.id).unwrap();
        assert_eq!(updated.locked_by_run_id, Some("actual-run-id".to_string()));
    }
    
    #[test]
    fn update_ticket_lock_owner_wrong_owner_fails() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Ticket".to_string(),
            description_md: "".to_string(),
            priority: Priority::Low,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        let expires = chrono::Utc::now() + chrono::Duration::minutes(30);
        db.lock_ticket(&ticket.id, "run-1", expires).unwrap();
        
        // Try to update from wrong owner - should fail
        let result = db.update_ticket_lock_owner(&ticket.id, "wrong-run-id", "new-run-id", None);
        assert!(result.is_err());
        
        // Original lock should still be in place
        let still_locked = db.get_ticket(&ticket.id).unwrap();
        assert_eq!(still_locked.locked_by_run_id, Some("run-1".to_string()));
    }

    // Tests for atomic reservation
    
    #[test]
    fn reserve_next_ticket_returns_ready_ticket() {
        let db = create_test_db();
        let (_board_id, _ready_column_id, ticket) = setup_board_with_ready_ticket(&db);
        
        let expires = Utc::now() + chrono::Duration::minutes(30);
        let reserved = db.reserve_next_ticket(None, AgentKind::Cursor, "run-1", expires).unwrap();
        
        assert!(reserved.is_some());
        let reserved_ticket = reserved.unwrap();
        assert_eq!(reserved_ticket.id, ticket.id);
        assert_eq!(reserved_ticket.locked_by_run_id, Some("run-1".to_string()));
    }
    
    #[test]
    fn reserve_next_ticket_returns_none_when_no_ready_tickets() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
        
        // Create ticket in Backlog, not Ready
        db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: backlog.id.clone(),
            title: "Backlog Ticket".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        let expires = Utc::now() + chrono::Duration::minutes(30);
        let reserved = db.reserve_next_ticket(None, AgentKind::Cursor, "run-1", expires).unwrap();
        
        assert!(reserved.is_none());
    }
    
    #[test]
    fn reserve_next_ticket_skips_locked_ticket() {
        let db = create_test_db();
        let (_board_id, _ready_column_id, ticket) = setup_board_with_ready_ticket(&db);
        
        // Lock the ticket
        let expires = Utc::now() + chrono::Duration::minutes(30);
        db.lock_ticket(&ticket.id, "existing-run", expires).unwrap();
        
        // Try to reserve - should return None since the only ticket is locked
        let reserved = db.reserve_next_ticket(None, AgentKind::Cursor, "new-run", expires).unwrap();
        assert!(reserved.is_none());
    }
    
    #[test]
    fn reserve_next_ticket_takes_expired_lock() {
        let db = create_test_db();
        let (_board_id, _ready_column_id, ticket) = setup_board_with_ready_ticket(&db);
        
        // Lock the ticket with an expired time
        let expired = Utc::now() - chrono::Duration::minutes(5);
        db.lock_ticket(&ticket.id, "old-run", expired).unwrap();
        
        // Try to reserve - should succeed since the lock is expired
        let new_expires = Utc::now() + chrono::Duration::minutes(30);
        let reserved = db.reserve_next_ticket(None, AgentKind::Cursor, "new-run", new_expires).unwrap();
        
        assert!(reserved.is_some());
        let reserved_ticket = reserved.unwrap();
        assert_eq!(reserved_ticket.locked_by_run_id, Some("new-run".to_string()));
    }
    
    #[test]
    fn reserve_next_ticket_respects_agent_pref_cursor() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
        
        // Create a ticket that prefers Claude
        db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: ready.id.clone(),
            title: "Claude Only".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            agent_pref: Some(AgentPref::Claude),
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        let expires = Utc::now() + chrono::Duration::minutes(30);
        
        // Cursor worker should not get this ticket
        let cursor_result = db.reserve_next_ticket(None, AgentKind::Cursor, "cursor-run", expires).unwrap();
        assert!(cursor_result.is_none());
        
        // Claude worker should get this ticket
        let claude_result = db.reserve_next_ticket(None, AgentKind::Claude, "claude-run", expires).unwrap();
        assert!(claude_result.is_some());
    }
    
    #[test]
    fn reserve_next_ticket_respects_project_filter() {
        let db = create_test_db();
        let project = db.create_project(&CreateProject {
            name: "Test Project".to_string(),
            path: temp_dir_path(),
            preferred_agent: None,
            requires_git: true,
        }).unwrap();
        
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
        
        // Create ticket for specific project
        db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: ready.id.clone(),
            title: "Project Ticket".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: Some(project.id.clone()),
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        let expires = Utc::now() + chrono::Duration::minutes(30);
        
        // Filter for different project should not find ticket
        let other_result = db.reserve_next_ticket(Some("other-project"), AgentKind::Cursor, "run-1", expires).unwrap();
        assert!(other_result.is_none());
        
        // Filter for correct project should find ticket
        let correct_result = db.reserve_next_ticket(Some(&project.id), AgentKind::Cursor, "run-2", expires).unwrap();
        assert!(correct_result.is_some());
    }
    
    #[test]
    fn reserve_next_ticket_prioritizes_by_priority_and_age() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
        
        // Create low priority ticket first
        let _low = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: ready.id.clone(),
            title: "Low Priority".to_string(),
            description_md: "".to_string(),
            priority: Priority::Low,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        // Create urgent ticket second
        let urgent = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: ready.id.clone(),
            title: "Urgent".to_string(),
            description_md: "".to_string(),
            priority: Priority::Urgent,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        let expires = Utc::now() + chrono::Duration::minutes(30);
        let reserved = db.reserve_next_ticket(None, AgentKind::Cursor, "run-1", expires).unwrap();
        
        // Should get the urgent ticket even though low priority was created first
        assert!(reserved.is_some());
        assert_eq!(reserved.unwrap().id, urgent.id);
    }
    
    #[test]
    fn reserve_next_ticket_respects_agent_pref_claude() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
        
        // Create a ticket that prefers Cursor
        db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: ready.id.clone(),
            title: "Cursor Only".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            agent_pref: Some(AgentPref::Cursor),
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        let expires = Utc::now() + chrono::Duration::minutes(30);
        
        // Claude worker should not get this ticket
        let claude_result = db.reserve_next_ticket(None, AgentKind::Claude, "claude-run", expires).unwrap();
        assert!(claude_result.is_none());
        
        // Cursor worker should get this ticket
        let cursor_result = db.reserve_next_ticket(None, AgentKind::Cursor, "cursor-run", expires).unwrap();
        assert!(cursor_result.is_some());
    }
    
    #[test]
    fn reserve_next_ticket_any_pref_works_for_both_agents() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
        
        // Create ticket with 'any' preference
        let ticket1 = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: ready.id.clone(),
            title: "Any Agent".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            agent_pref: Some(AgentPref::Any),
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        let expires = Utc::now() + chrono::Duration::minutes(30);
        
        // Cursor worker should get the ticket
        let cursor_result = db.reserve_next_ticket(None, AgentKind::Cursor, "cursor-run", expires).unwrap();
        assert!(cursor_result.is_some());
        assert_eq!(cursor_result.unwrap().id, ticket1.id);
        
        // Unlock and try with Claude
        db.unlock_ticket(&ticket1.id).unwrap();
        
        let claude_result = db.reserve_next_ticket(None, AgentKind::Claude, "claude-run", expires).unwrap();
        assert!(claude_result.is_some());
        assert_eq!(claude_result.unwrap().id, ticket1.id);
    }
    
    #[test]
    fn reserve_next_ticket_null_pref_works_for_both_agents() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
        
        // Create ticket with no agent preference (NULL)
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: ready.id.clone(),
            title: "No Preference".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        let expires = Utc::now() + chrono::Duration::minutes(30);
        
        // Both agents should be able to claim it
        let cursor_result = db.reserve_next_ticket(None, AgentKind::Cursor, "cursor-run", expires).unwrap();
        assert!(cursor_result.is_some());
        
        db.unlock_ticket(&ticket.id).unwrap();
        
        let claude_result = db.reserve_next_ticket(None, AgentKind::Claude, "claude-run", expires).unwrap();
        assert!(claude_result.is_some());
    }
    
    #[test]
    fn reserve_next_ticket_skips_epic_tickets() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
        
        // Create an epic ticket in Ready - should NOT be picked up
        db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: ready.id.clone(),
            title: "Epic Ticket".to_string(),
            description_md: "This is an epic".to_string(),
            priority: Priority::High,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: true,  // This makes it an epic
            epic_id: None,
        }).unwrap();
        
        let expires = Utc::now() + chrono::Duration::minutes(30);
        
        // Worker should NOT pick up the epic
        let result = db.reserve_next_ticket(None, AgentKind::Cursor, "run-1", expires).unwrap();
        assert!(result.is_none(), "Epic ticket should not be picked up by workers");
    }
    
    #[test]
    fn reserve_next_ticket_picks_child_ticket_not_epic() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
        
        // Create an epic ticket in Ready
        let epic = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: ready.id.clone(),
            title: "Epic Ticket".to_string(),
            description_md: "This is an epic".to_string(),
            priority: Priority::High,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: true,
            epic_id: None,
        }).unwrap();
        
        // Create a child ticket in Ready - this SHOULD be picked up
        let child = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: ready.id.clone(),
            title: "Child Ticket".to_string(),
            description_md: "Child of epic".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: Some(epic.id.clone()),
        }).unwrap();
        
        let expires = Utc::now() + chrono::Duration::minutes(30);
        
        // Worker should pick up the child, not the epic
        let result = db.reserve_next_ticket(None, AgentKind::Cursor, "run-1", expires).unwrap();
        assert!(result.is_some(), "Child ticket should be picked up");
        assert_eq!(result.unwrap().id, child.id, "Should pick up child ticket, not epic");
    }

    #[test]
    fn set_ticket_branch_success() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Test Ticket".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        assert!(ticket.branch_name.is_none());
        
        db.set_ticket_branch(&ticket.id, "feat/abc123/add-feature").unwrap();
        
        let updated = db.get_ticket(&ticket.id).unwrap();
        assert_eq!(updated.branch_name, Some("feat/abc123/add-feature".to_string()));
    }

    #[test]
    fn set_ticket_branch_not_found() {
        let db = create_test_db();
        let result = db.set_ticket_branch("nonexistent-id", "some-branch");
        assert!(matches!(result, Err(DbError::NotFound(_))));
    }

    #[test]
    fn set_ticket_branch_updates_timestamp() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Test Ticket".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        let before = ticket.updated_at;
        
        // Small delay to ensure timestamp differs
        std::thread::sleep(std::time::Duration::from_millis(10));
        
        db.set_ticket_branch(&ticket.id, "fix/123/bug-fix").unwrap();
        
        let updated = db.get_ticket(&ticket.id).unwrap();
        assert!(updated.updated_at >= before);
    }

    #[test]
    fn create_ticket_with_branch_name() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Test Ticket".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: Some("feat/preset/my-branch".to_string()),
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        assert_eq!(ticket.branch_name, Some("feat/preset/my-branch".to_string()));
        
        // Verify it persists
        let fetched = db.get_ticket(&ticket.id).unwrap();
        assert_eq!(fetched.branch_name, Some("feat/preset/my-branch".to_string()));
    }

    #[test]
    fn create_ticket_auto_creates_initial_task() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "My Feature Request".to_string(),
            description_md: "Implement this feature".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        // Verify Task 1 was automatically created
        let tasks = db.get_tasks_for_ticket(&ticket.id).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].order_index, 0);
        assert_eq!(tasks[0].title, Some("My Feature Request".to_string()));
        assert_eq!(tasks[0].content, Some("Implement this feature".to_string()));
    }

    #[test]
    fn create_ticket_truncates_long_title_for_task() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let long_title = "A".repeat(60); // 60 chars, should be truncated to 50
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: long_title.clone(),
            description_md: "Description".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        let tasks = db.get_tasks_for_ticket(&ticket.id).unwrap();
        assert_eq!(tasks.len(), 1);
        // Title should be truncated with "..."
        let task_title = tasks[0].title.as_ref().unwrap();
        assert!(task_title.chars().count() <= 50); // Check character count, not byte count
        assert!(task_title.ends_with("..."));
    }

    #[test]
    fn create_ticket_truncates_utf8_title_safely() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        // Title with multi-byte UTF-8 characters (emoji are 4 bytes each)
        // This would panic with byte-based slicing if byte 47 lands mid-character
        let emoji_title = "🎉".repeat(60); // 60 emoji = 240 bytes, 60 characters
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: emoji_title.clone(),
            description_md: "Description".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        let tasks = db.get_tasks_for_ticket(&ticket.id).unwrap();
        assert_eq!(tasks.len(), 1);
        let task_title = tasks[0].title.as_ref().unwrap();
        // Should be 47 emoji + "..." = 50 characters
        assert_eq!(task_title.chars().count(), 50);
        assert!(task_title.ends_with("..."));
        // Verify we got exactly 47 emoji (not corrupted by bad slicing)
        assert_eq!(task_title.chars().filter(|&c| c == '🎉').count(), 47);
    }

    #[test]
    fn create_ticket_empty_description_creates_task_with_no_content() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Quick Task".to_string(),
            description_md: "".to_string(), // Empty description
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
        }).unwrap();
        
        let tasks = db.get_tasks_for_ticket(&ticket.id).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].content, None); // No content since description was empty
    }
}
