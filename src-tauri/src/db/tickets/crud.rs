use crate::db::models::{CreateTask, CreateTicket, TaskType, Ticket, UpdateTicket};
use crate::db::{Database, DbError};

impl Database {
    pub fn get_ticket(&self, ticket_id: &str) -> Result<Ticket, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, column_id, title, description_md, priority, 
                          labels_json, created_at, updated_at, locked_by_run_id, 
                          lock_expires_at, project_id, workflow_type, model, branch_name,
                          is_epic, epic_id, order_in_epic, depends_on_epic_id, depends_on_epic_ids_json, spec_version_id,
                          paused_at, paused_at_stage, paused_run_id
                   FROM tickets WHERE id = ?"#,
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

    pub fn update_ticket(
        &self,
        ticket_id: &str,
        updates: &UpdateTicket,
    ) -> Result<Ticket, DbError> {
        self.with_conn(|conn| {
            // First get the existing ticket
            let existing = {
                let mut stmt = conn.prepare(
                    r#"SELECT id, board_id, column_id, title, description_md, priority, 
                              labels_json, created_at, updated_at, locked_by_run_id, 
                              lock_expires_at, project_id, workflow_type, model, branch_name,
                              is_epic, epic_id, order_in_epic, depends_on_epic_id, depends_on_epic_ids_json, spec_version_id,
                          paused_at, paused_at_stage, paused_run_id
                       FROM tickets WHERE id = ?"#,
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
            let description_md = updates
                .description_md
                .as_ref()
                .unwrap_or(&existing.description_md);
            let priority = updates.priority.as_ref().unwrap_or(&existing.priority);
            let labels = updates.labels.as_ref().unwrap_or(&existing.labels);
            // Handle project_id: None means keep existing, Some("") means clear, Some(id) means set
            let project_id = match &updates.project_id {
                Some(id) if id.is_empty() => None, // Empty string means clear the project
                Some(id) => Some(id.as_str()),
                None => existing.project_id.as_deref(), // Keep existing
            };
            let workflow_type = updates
                .workflow_type
                .as_ref()
                .unwrap_or(&existing.workflow_type);
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
            // Handle depends_on_epic_id: None means keep existing, Some("") means clear, Some(id) means set
            let depends_on_epic_id = match &updates.depends_on_epic_id {
                Some(id) if id.is_empty() => None,
                Some(id) => Some(id.as_str()),
                None => existing.depends_on_epic_id.as_deref(),
            };
            // Handle spec_version_id: None means keep existing, Some("") means clear, Some(id) means set
            let spec_version_id = match &updates.spec_version_id {
                Some(id) if id.is_empty() => None,
                Some(id) => Some(id.as_str()),
                None => existing.spec_version_id.as_deref(),
            };
            // Handle depends_on_epic_ids: empty Vec means keep existing, non-empty means set
            let depends_on_epic_ids = if updates.depends_on_epic_ids.is_empty() {
                existing.depends_on_epic_ids.clone()
            } else {
                updates.depends_on_epic_ids.clone()
            };

            let labels_json =
                serde_json::to_string(labels).unwrap_or_else(|_| "[]".to_string());
            let depends_on_epic_ids_json = if depends_on_epic_ids.is_empty() {
                None
            } else {
                Some(
                    serde_json::to_string(&depends_on_epic_ids)
                        .unwrap_or_else(|_| "[]".to_string()),
                )
            };

            conn.execute(
                r#"UPDATE tickets 
                   SET title = ?, description_md = ?, priority = ?, labels_json = ?,
                       project_id = ?, workflow_type = ?, model = ?, branch_name = ?, 
                       column_id = ?, is_epic = ?, epic_id = ?, order_in_epic = ?, 
                       depends_on_epic_id = ?, depends_on_epic_ids_json = ?, spec_version_id = ?, updated_at = ?
                   WHERE id = ?"#,
                rusqlite::params![
                    title,
                    description_md,
                    priority.as_str(),
                    labels_json,
                    project_id,
                    workflow_type.as_str(),
                    model,
                    branch_name,
                    column_id,
                    is_epic,
                    epic_id,
                    order_in_epic,
                    depends_on_epic_id,
                    depends_on_epic_ids_json,
                    spec_version_id,
                    now.to_rfc3339(),
                    ticket_id,
                ],
            )?;

            // Re-query within the same connection to avoid deadlock
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, column_id, title, description_md, priority, 
                          labels_json, created_at, updated_at, locked_by_run_id, 
                          lock_expires_at, project_id, workflow_type, model, branch_name,
                          is_epic, epic_id, order_in_epic, depends_on_epic_id, depends_on_epic_ids_json, spec_version_id,
                          paused_at, paused_at_stage, paused_run_id
                   FROM tickets WHERE id = ?"#,
            )?;
            stmt.query_row([ticket_id], Self::map_ticket_row)
                .map_err(DbError::Sqlite)
        })
    }

    pub fn delete_ticket(&self, ticket_id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let affected = conn.execute("DELETE FROM tickets WHERE id = ?", [ticket_id])?;

            if affected == 0 {
                return Err(DbError::NotFound(format!("Ticket {}", ticket_id)));
            }
            Ok(())
        })
    }

    pub fn create_ticket(&self, ticket: &CreateTicket) -> Result<Ticket, DbError> {
        // If this is a child of an epic, calculate the order_in_epic
        let order_in_epic = if let Some(ref epic_id) = ticket.epic_id {
            // Get the current max order for children of this epic
            self.with_conn(|conn| {
                let max_order: Option<i32> = conn
                    .query_row(
                        "SELECT MAX(order_in_epic) FROM tickets WHERE epic_id = ?",
                        [epic_id],
                        |row| row.get(0),
                    )
                    .unwrap_or(None);
                Ok::<_, DbError>(Some(max_order.unwrap_or(-1) + 1))
            })?
        } else {
            None
        };

        let created_ticket = self.with_conn(|conn| {
            let ticket_id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now();
            let labels_json =
                serde_json::to_string(&ticket.labels).unwrap_or_else(|_| "[]".to_string());
            let depends_on_epic_ids_json = if ticket.depends_on_epic_ids.is_empty() {
                None
            } else {
                Some(
                    serde_json::to_string(&ticket.depends_on_epic_ids)
                        .unwrap_or_else(|_| "[]".to_string()),
                )
            };

            conn.execute(
                r#"INSERT INTO tickets 
                   (id, board_id, column_id, title, description_md, priority, labels_json, 
                    created_at, updated_at, project_id, workflow_type, model, branch_name,
                    is_epic, epic_id, order_in_epic, depends_on_epic_id, depends_on_epic_ids_json, spec_version_id,
                    paused_at, paused_at_stage, paused_run_id)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, NULL)"#,
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
                    ticket.workflow_type.as_str(),
                    ticket.model,
                    ticket.branch_name,
                    ticket.is_epic,
                    ticket.epic_id,
                    order_in_epic,
                    ticket.depends_on_epic_id,
                    depends_on_epic_ids_json,
                    ticket.spec_version_id,
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
                workflow_type: ticket.workflow_type.clone(),
                model: ticket.model.clone(),
                branch_name: ticket.branch_name.clone(),
                is_epic: ticket.is_epic,
                epic_id: ticket.epic_id.clone(),
                order_in_epic,
                depends_on_epic_id: ticket.depends_on_epic_id.clone(),
                depends_on_epic_ids: ticket.depends_on_epic_ids.clone(),
                spec_version_id: ticket.spec_version_id.clone(),
                paused_at: None,
                paused_at_stage: None,
                paused_run_id: None,
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
            format!(
                "{}...",
                created_ticket.title.chars().take(47).collect::<String>()
            )
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
                tracing::error!(
                    "Failed to delete ticket {} after task creation failure: {}",
                    created_ticket.id,
                    delete_err
                );
            }
            return Err(DbError::Validation(format!(
                "Failed to create initial task for ticket: {}. Ticket creation aborted.",
                e
            )));
        }

        Ok(created_ticket)
    }

    /// Get most recently updated tickets across all boards, with their column name.
    pub fn get_recent_tickets_with_columns(
        &self,
        limit: u32,
    ) -> Result<Vec<(Ticket, String)>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT t.id, t.board_id, t.column_id, t.title, t.description_md, t.priority,
                          t.labels_json, t.created_at, t.updated_at, t.locked_by_run_id,
                          t.lock_expires_at, t.project_id, t.workflow_type, t.model, t.branch_name,
                          t.is_epic, t.epic_id, t.order_in_epic, t.depends_on_epic_id,
                          t.depends_on_epic_ids_json, t.spec_version_id,
                          t.paused_at, t.paused_at_stage, t.paused_run_id,
                          c.name
                   FROM tickets t
                   JOIN columns c ON t.column_id = c.id
                   ORDER BY t.updated_at DESC
                   LIMIT ?"#,
            )?;

            let rows = stmt.query_map([limit], |row| {
                let ticket = Self::map_ticket_row(row)?;
                let column_name: String = row.get(24)?;
                Ok((ticket, column_name))
            })?;

            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    pub fn get_tickets(
        &self,
        board_id: &str,
        column_id: Option<&str>,
    ) -> Result<Vec<Ticket>, DbError> {
        self.with_conn(|conn| {
            let sql = match column_id {
                Some(_) => {
                    "SELECT id, board_id, column_id, title, description_md, priority, 
                            labels_json, created_at, updated_at, locked_by_run_id, 
                            lock_expires_at, project_id, workflow_type, model, branch_name,
                            is_epic, epic_id, order_in_epic, depends_on_epic_id, depends_on_epic_ids_json, spec_version_id,
                          paused_at, paused_at_stage, paused_run_id
                     FROM tickets WHERE board_id = ? AND column_id = ? ORDER BY created_at"
                }
                None => {
                    "SELECT id, board_id, column_id, title, description_md, priority, 
                            labels_json, created_at, updated_at, locked_by_run_id, 
                            lock_expires_at, project_id, workflow_type, model, branch_name,
                            is_epic, epic_id, order_in_epic, depends_on_epic_id, depends_on_epic_ids_json, spec_version_id,
                          paused_at, paused_at_stage, paused_run_id
                     FROM tickets WHERE board_id = ? ORDER BY created_at"
                }
            };

            let mut stmt = conn.prepare(sql)?;

            let rows = match column_id {
                Some(col_id) => {
                    stmt.query_map(rusqlite::params![board_id, col_id], Self::map_ticket_row)?
                }
                None => stmt.query_map([board_id], Self::map_ticket_row)?,
            };

            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }
}
