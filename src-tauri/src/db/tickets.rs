use chrono::{DateTime, Utc};
use crate::db::{Database, DbError, parse_datetime};
use crate::db::models::{Ticket, CreateTicket, UpdateTicket, Priority, AgentPref, WorkflowType, CreateTask, TaskType};
use crate::agents::AgentKind;

impl Database {
    pub fn get_ticket(&self, ticket_id: &str) -> Result<Ticket, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, column_id, title, description_md, priority, 
                          labels_json, created_at, updated_at, locked_by_run_id, 
                          lock_expires_at, project_id, agent_pref, workflow_type, model, branch_name
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
                              lock_expires_at, project_id, agent_pref, workflow_type, model, branch_name
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

            let labels_json = serde_json::to_string(labels).unwrap_or_else(|_| "[]".to_string());

            conn.execute(
                r#"UPDATE tickets 
                   SET title = ?, description_md = ?, priority = ?, labels_json = ?,
                       project_id = ?, agent_pref = ?, workflow_type = ?, model = ?, branch_name = ?, updated_at = ?
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
                    now.to_rfc3339(),
                    ticket_id,
                ],
            )?;

            // Re-query within the same connection to avoid deadlock
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, column_id, title, description_md, priority, 
                          labels_json, created_at, updated_at, locked_by_run_id, 
                          lock_expires_at, project_id, agent_pref, workflow_type, model, branch_name
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

    pub fn lock_ticket(
        &self,
        ticket_id: &str,
        run_id: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let affected = conn.execute(
                r#"UPDATE tickets 
                   SET locked_by_run_id = ?, lock_expires_at = ?, updated_at = ?
                   WHERE id = ?"#,
                rusqlite::params![
                    run_id,
                    expires_at.to_rfc3339(),
                    chrono::Utc::now().to_rfc3339(),
                    ticket_id,
                ],
            )?;
            
            if affected == 0 {
                return Err(DbError::NotFound(format!("Ticket {}", ticket_id)));
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
            let affected = tx.execute(
                r#"UPDATE tickets 
                   SET locked_by_run_id = ?1, lock_expires_at = ?2, updated_at = ?3
                   WHERE id = (
                       SELECT t.id FROM tickets t
                       JOIN columns c ON t.column_id = c.id
                       WHERE c.name = 'Ready'
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
                          lock_expires_at, project_id, agent_pref, workflow_type, model, branch_name
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
        let created_ticket = self.with_conn(|conn| {
            let ticket_id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now();
            let labels_json = serde_json::to_string(&ticket.labels).unwrap_or_else(|_| "[]".to_string());
            
            conn.execute(
                r#"INSERT INTO tickets 
                   (id, board_id, column_id, title, description_md, priority, labels_json, 
                    created_at, updated_at, project_id, agent_pref, workflow_type, model, branch_name)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
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
            })
        })?;
        
        // Auto-create Task 1 from the ticket description
        // This is the initial task that defines the work to be done
        let task_title = if created_ticket.title.len() > 50 {
            format!("{}...", &created_ticket.title[..47])
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
            tracing::warn!("Failed to create initial task for ticket {}: {}", created_ticket.id, e);
        }
        
        Ok(created_ticket)
    }

    pub fn get_tickets(&self, board_id: &str, column_id: Option<&str>) -> Result<Vec<Ticket>, DbError> {
        self.with_conn(|conn| {
            let sql = match column_id {
                Some(_) => {
                    "SELECT id, board_id, column_id, title, description_md, priority, 
                            labels_json, created_at, updated_at, locked_by_run_id, 
                            lock_expires_at, project_id, agent_pref, workflow_type, model, branch_name
                     FROM tickets WHERE board_id = ? AND column_id = ? ORDER BY created_at"
                }
                None => {
                    "SELECT id, board_id, column_id, title, description_md, priority, 
                            labels_json, created_at, updated_at, locked_by_run_id, 
                            lock_expires_at, project_id, agent_pref, workflow_type, model, branch_name
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
            description_md: None,
            priority: None,
            labels: None,
            project_id: None,
            agent_pref: None,
            workflow_type: None,
            model: None,
            branch_name: None,
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
        }).unwrap();
        
        let expires = chrono::Utc::now() + chrono::Duration::minutes(30);
        db.lock_ticket(&ticket.id, "run-correct", expires).unwrap();
        
        let result = db.extend_lock(&ticket.id, "run-wrong", expires);
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
        }).unwrap();
        
        let expires = chrono::Utc::now() + chrono::Duration::minutes(30);
        db.lock_ticket(&ticket.id, "run-correct", expires).unwrap();
        
        // Try to release with wrong run_id - should have no effect
        db.release_lock(&ticket.id, "run-wrong").unwrap();
        
        let still_locked = db.get_ticket(&ticket.id).unwrap();
        assert_eq!(still_locked.locked_by_run_id, Some("run-correct".to_string()));
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
        }).unwrap();
        
        let tasks = db.get_tasks_for_ticket(&ticket.id).unwrap();
        assert_eq!(tasks.len(), 1);
        // Title should be truncated with "..."
        let task_title = tasks[0].title.as_ref().unwrap();
        assert!(task_title.len() <= 50);
        assert!(task_title.ends_with("..."));
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
        }).unwrap();
        
        let tasks = db.get_tasks_for_ticket(&ticket.id).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].content, None); // No content since description was empty
    }
}
