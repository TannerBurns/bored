//! Task queue database operations

use crate::db::{Database, DbError, parse_datetime};
use crate::db::models::{Task, CreateTask, UpdateTask, TaskType, TaskStatus};

impl Database {
    /// Create a new task for a ticket
    pub fn create_task(&self, task: &CreateTask) -> Result<Task, DbError> {
        self.with_conn(|conn| {
            let task_id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now();
            
            // Get the next order_index for this ticket
            let next_index: i32 = conn.query_row(
                "SELECT COALESCE(MAX(order_index), -1) + 1 FROM tasks WHERE ticket_id = ?",
                [&task.ticket_id],
                |row| row.get(0),
            ).unwrap_or(0);
            
            conn.execute(
                r#"INSERT INTO tasks 
                   (id, ticket_id, order_index, task_type, title, content, status, created_at)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
                rusqlite::params![
                    task_id,
                    task.ticket_id,
                    next_index,
                    task.task_type.as_str(),
                    task.title,
                    task.content,
                    TaskStatus::Pending.as_str(),
                    now.to_rfc3339(),
                ],
            )?;

            Ok(Task {
                id: task_id,
                ticket_id: task.ticket_id.clone(),
                order_index: next_index,
                task_type: task.task_type.clone(),
                title: task.title.clone(),
                content: task.content.clone(),
                status: TaskStatus::Pending,
                run_id: None,
                created_at: now,
                started_at: None,
                completed_at: None,
            })
        })
    }

    /// Get a task by ID
    pub fn get_task(&self, task_id: &str) -> Result<Task, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, ticket_id, order_index, task_type, title, content, 
                          status, run_id, created_at, started_at, completed_at
                   FROM tasks WHERE id = ?"#
            )?;
            
            stmt.query_row([task_id], Self::map_task_row)
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        DbError::NotFound(format!("Task {}", task_id))
                    }
                    other => DbError::Sqlite(other),
                })
        })
    }

    /// Get all tasks for a ticket, ordered by order_index
    pub fn get_tasks_for_ticket(&self, ticket_id: &str) -> Result<Vec<Task>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, ticket_id, order_index, task_type, title, content, 
                          status, run_id, created_at, started_at, completed_at
                   FROM tasks WHERE ticket_id = ? ORDER BY order_index"#
            )?;
            
            let rows = stmt.query_map([ticket_id], Self::map_task_row)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    /// Get the next pending task for a ticket
    pub fn get_next_pending_task(&self, ticket_id: &str) -> Result<Option<Task>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, ticket_id, order_index, task_type, title, content, 
                          status, run_id, created_at, started_at, completed_at
                   FROM tasks 
                   WHERE ticket_id = ? AND status = 'pending'
                   ORDER BY order_index
                   LIMIT 1"#
            )?;
            
            let result = stmt.query_row([ticket_id], Self::map_task_row);
            match result {
                Ok(task) => Ok(Some(task)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(DbError::Sqlite(e)),
            }
        })
    }

    /// Update a task
    pub fn update_task(&self, task_id: &str, updates: &UpdateTask) -> Result<Task, DbError> {
        self.with_conn(|conn| {
            // First get the existing task
            let existing = {
                let mut stmt = conn.prepare(
                    r#"SELECT id, ticket_id, order_index, task_type, title, content, 
                              status, run_id, created_at, started_at, completed_at
                       FROM tasks WHERE id = ?"#
                )?;
                stmt.query_row([task_id], Self::map_task_row)
                    .map_err(|e| match e {
                        rusqlite::Error::QueryReturnedNoRows => {
                            DbError::NotFound(format!("Task {}", task_id))
                        }
                        other => DbError::Sqlite(other),
                    })?
            };

            let title = updates.title.as_ref().or(existing.title.as_ref());
            let content = updates.content.as_ref().or(existing.content.as_ref());
            let status = updates.status.as_ref().unwrap_or(&existing.status);
            let run_id = updates.run_id.as_ref().or(existing.run_id.as_ref());

            conn.execute(
                r#"UPDATE tasks 
                   SET title = ?, content = ?, status = ?, run_id = ?
                   WHERE id = ?"#,
                rusqlite::params![
                    title,
                    content,
                    status.as_str(),
                    run_id,
                    task_id,
                ],
            )?;

            // Re-query within the same connection
            let mut stmt = conn.prepare(
                r#"SELECT id, ticket_id, order_index, task_type, title, content, 
                          status, run_id, created_at, started_at, completed_at
                   FROM tasks WHERE id = ?"#
            )?;
            stmt.query_row([task_id], Self::map_task_row)
                .map_err(DbError::Sqlite)
        })
    }

    /// Mark a task as in progress
    pub fn start_task(&self, task_id: &str, run_id: &str) -> Result<Task, DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            
            let affected = conn.execute(
                r#"UPDATE tasks 
                   SET status = 'in_progress', run_id = ?, started_at = ?
                   WHERE id = ? AND status = 'pending'"#,
                rusqlite::params![run_id, now, task_id],
            )?;
            
            if affected == 0 {
                return Err(DbError::Validation(
                    format!("Task {} is not pending", task_id)
                ));
            }

            // Re-query
            let mut stmt = conn.prepare(
                r#"SELECT id, ticket_id, order_index, task_type, title, content, 
                          status, run_id, created_at, started_at, completed_at
                   FROM tasks WHERE id = ?"#
            )?;
            stmt.query_row([task_id], Self::map_task_row)
                .map_err(DbError::Sqlite)
        })
    }

    /// Mark a task as completed
    pub fn complete_task(&self, task_id: &str) -> Result<Task, DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            
            let affected = conn.execute(
                r#"UPDATE tasks 
                   SET status = 'completed', completed_at = ?
                   WHERE id = ? AND status = 'in_progress'"#,
                rusqlite::params![now, task_id],
            )?;
            
            if affected == 0 {
                return Err(DbError::Validation(
                    format!("Task {} is not in progress", task_id)
                ));
            }

            // Re-query
            let mut stmt = conn.prepare(
                r#"SELECT id, ticket_id, order_index, task_type, title, content, 
                          status, run_id, created_at, started_at, completed_at
                   FROM tasks WHERE id = ?"#
            )?;
            stmt.query_row([task_id], Self::map_task_row)
                .map_err(DbError::Sqlite)
        })
    }

    /// Mark a task as failed
    pub fn fail_task(&self, task_id: &str) -> Result<Task, DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            
            let affected = conn.execute(
                r#"UPDATE tasks 
                   SET status = 'failed', completed_at = ?
                   WHERE id = ? AND status = 'in_progress'"#,
                rusqlite::params![now, task_id],
            )?;
            
            if affected == 0 {
                return Err(DbError::Validation(
                    format!("Task {} is not in progress", task_id)
                ));
            }

            // Re-query
            let mut stmt = conn.prepare(
                r#"SELECT id, ticket_id, order_index, task_type, title, content, 
                          status, run_id, created_at, started_at, completed_at
                   FROM tasks WHERE id = ?"#
            )?;
            stmt.query_row([task_id], Self::map_task_row)
                .map_err(DbError::Sqlite)
        })
    }

    /// Delete a task
    pub fn delete_task(&self, task_id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let affected = conn.execute(
                "DELETE FROM tasks WHERE id = ?",
                [task_id],
            )?;
            
            if affected == 0 {
                return Err(DbError::NotFound(format!("Task {}", task_id)));
            }
            Ok(())
        })
    }

    /// Check if a ticket has any pending tasks
    pub fn has_pending_tasks(&self, ticket_id: &str) -> Result<bool, DbError> {
        self.with_conn(|conn| {
            let count: i32 = conn.query_row(
                "SELECT COUNT(*) FROM tasks WHERE ticket_id = ? AND status = 'pending'",
                [ticket_id],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        })
    }

    /// Get count of tasks by status for a ticket
    pub fn get_task_counts(&self, ticket_id: &str) -> Result<TaskCounts, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT status, COUNT(*) FROM tasks 
                   WHERE ticket_id = ? GROUP BY status"#
            )?;
            
            let mut counts = TaskCounts::default();
            let mut rows = stmt.query([ticket_id])?;
            
            while let Some(row) = rows.next()? {
                let status: String = row.get(0)?;
                let count: i32 = row.get(1)?;
                match status.as_str() {
                    "pending" => counts.pending = count,
                    "in_progress" => counts.in_progress = count,
                    "completed" => counts.completed = count,
                    "failed" => counts.failed = count,
                    _ => {}
                }
            }
            
            Ok(counts)
        })
    }

    fn map_task_row(row: &rusqlite::Row) -> rusqlite::Result<Task> {
        let task_type_str: String = row.get(3)?;
        let task_type = TaskType::parse(&task_type_str).unwrap_or_default();
        
        let status_str: String = row.get(6)?;
        let status = TaskStatus::parse(&status_str).unwrap_or_default();

        Ok(Task {
            id: row.get(0)?,
            ticket_id: row.get(1)?,
            order_index: row.get(2)?,
            task_type,
            title: row.get(4)?,
            content: row.get(5)?,
            status,
            run_id: row.get(7)?,
            created_at: parse_datetime(row.get(8)?),
            started_at: row.get::<_, Option<String>>(9)?.map(parse_datetime),
            completed_at: row.get::<_, Option<String>>(10)?.map(parse_datetime),
        })
    }
}

/// Counts of tasks by status for a ticket
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCounts {
    pub pending: i32,
    pub in_progress: i32,
    pub completed: i32,
    pub failed: i32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::{CreateTicket, Priority, WorkflowType};

    fn create_test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn setup_ticket(db: &Database) -> String {
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id,
            column_id: columns[0].id.clone(),
            title: "Test Ticket".to_string(),
            description_md: "Test description".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
        }).unwrap();
        ticket.id
    }

    #[test]
    fn create_task_success() {
        let db = create_test_db();
        let ticket_id = setup_ticket(&db);
        
        // Note: ticket creation auto-creates Task 1, so we add Task 2 here
        let task = db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::Custom,
            title: Some("Task 2".to_string()),
            content: Some("Do something".to_string()),
        }).unwrap();
        
        assert_eq!(task.ticket_id, ticket_id);
        assert_eq!(task.order_index, 1); // 1 because Task 0 was auto-created
        assert_eq!(task.task_type, TaskType::Custom);
        assert_eq!(task.title, Some("Task 2".to_string()));
        assert_eq!(task.status, TaskStatus::Pending);
    }

    #[test]
    fn create_multiple_tasks_increments_order() {
        let db = create_test_db();
        let ticket_id = setup_ticket(&db);
        
        // Note: ticket creation auto-creates Task 0, so these will be 1, 2, 3
        let task1 = db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::Custom,
            title: Some("Task 2".to_string()),
            content: None,
        }).unwrap();
        
        let task2 = db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::SyncWithMain,
            title: None,
            content: None,
        }).unwrap();
        
        let task3 = db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::AddTests,
            title: None,
            content: None,
        }).unwrap();
        
        assert_eq!(task1.order_index, 1); // Starts at 1 because 0 was auto-created
        assert_eq!(task2.order_index, 2);
        assert_eq!(task3.order_index, 3);
    }

    #[test]
    fn get_tasks_for_ticket_ordered() {
        let db = create_test_db();
        let ticket_id = setup_ticket(&db);
        
        // Note: ticket creation auto-creates Task 0
        db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::Custom,
            title: Some("Second".to_string()),
            content: None,
        }).unwrap();
        
        db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::Custom,
            title: Some("Third".to_string()),
            content: None,
        }).unwrap();
        
        let tasks = db.get_tasks_for_ticket(&ticket_id).unwrap();
        
        assert_eq!(tasks.len(), 3); // 1 auto-created + 2 manual
        assert_eq!(tasks[0].title, Some("Test Ticket".to_string())); // Auto-created from title
        assert_eq!(tasks[1].title, Some("Second".to_string()));
        assert_eq!(tasks[2].title, Some("Third".to_string()));
    }

    #[test]
    fn get_next_pending_task() {
        let db = create_test_db();
        let ticket_id = setup_ticket(&db);
        
        // Note: ticket creation auto-creates Task 0, so that should be the first pending task
        let tasks = db.get_tasks_for_ticket(&ticket_id).unwrap();
        assert!(!tasks.is_empty());
        let auto_task_id = tasks[0].id.clone();
        
        db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::Custom,
            title: Some("Task 2".to_string()),
            content: None,
        }).unwrap();
        
        let next = db.get_next_pending_task(&ticket_id).unwrap();
        assert!(next.is_some());
        // Should be the auto-created task (order_index 0)
        assert_eq!(next.unwrap().id, auto_task_id);
    }

    #[test]
    fn start_and_complete_task() {
        let db = create_test_db();
        let ticket_id = setup_ticket(&db);
        
        let task = db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::Custom,
            title: Some("Task".to_string()),
            content: None,
        }).unwrap();
        
        // Start the task
        let started = db.start_task(&task.id, "run-123").unwrap();
        assert_eq!(started.status, TaskStatus::InProgress);
        assert_eq!(started.run_id, Some("run-123".to_string()));
        assert!(started.started_at.is_some());
        
        // Complete the task
        let completed = db.complete_task(&task.id).unwrap();
        assert_eq!(completed.status, TaskStatus::Completed);
        assert!(completed.completed_at.is_some());
    }

    #[test]
    fn fail_task() {
        let db = create_test_db();
        let ticket_id = setup_ticket(&db);
        
        let task = db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::Custom,
            title: Some("Task".to_string()),
            content: None,
        }).unwrap();
        
        db.start_task(&task.id, "run-123").unwrap();
        
        let failed = db.fail_task(&task.id).unwrap();
        assert_eq!(failed.status, TaskStatus::Failed);
        assert!(failed.completed_at.is_some());
    }

    #[test]
    fn has_pending_tasks() {
        let db = create_test_db();
        let ticket_id = setup_ticket(&db);
        
        // Ticket creation auto-creates Task 0, so there's already a pending task
        assert!(db.has_pending_tasks(&ticket_id).unwrap());
        
        // Complete the auto-created task
        let tasks = db.get_tasks_for_ticket(&ticket_id).unwrap();
        db.start_task(&tasks[0].id, "run-1").unwrap();
        db.complete_task(&tasks[0].id).unwrap();
        
        // Now there should be no pending tasks
        assert!(!db.has_pending_tasks(&ticket_id).unwrap());
        
        // Add a new pending task
        db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::Custom,
            title: None,
            content: None,
        }).unwrap();
        
        // Should have pending task again
        assert!(db.has_pending_tasks(&ticket_id).unwrap());
    }

    #[test]
    fn get_task_counts() {
        let db = create_test_db();
        let ticket_id = setup_ticket(&db);
        
        // Ticket creation auto-creates Task 0
        let auto_tasks = db.get_tasks_for_ticket(&ticket_id).unwrap();
        let auto_task = &auto_tasks[0];
        
        // Create additional tasks
        let task2 = db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::Custom,
            title: None,
            content: None,
        }).unwrap();
        
        db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::Custom,
            title: None,
            content: None,
        }).unwrap();
        
        // Complete auto-created task
        db.start_task(&auto_task.id, "run-1").unwrap();
        db.complete_task(&auto_task.id).unwrap();
        
        // Start task2
        db.start_task(&task2.id, "run-2").unwrap();
        
        let counts = db.get_task_counts(&ticket_id).unwrap();
        assert_eq!(counts.pending, 1);  // The 3rd task we created
        assert_eq!(counts.in_progress, 1);  // task2
        assert_eq!(counts.completed, 1);  // auto-created task
        assert_eq!(counts.failed, 0);
    }

    #[test]
    fn delete_task() {
        let db = create_test_db();
        let ticket_id = setup_ticket(&db);
        
        let task = db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::Custom,
            title: None,
            content: None,
        }).unwrap();
        
        db.delete_task(&task.id).unwrap();
        
        let result = db.get_task(&task.id);
        assert!(matches!(result, Err(DbError::NotFound(_))));
    }

    #[test]
    fn task_type_parse_roundtrip() {
        for task_type in [
            TaskType::Custom,
            TaskType::SyncWithMain,
            TaskType::AddTests,
            TaskType::ReviewPolish,
            TaskType::FixLint,
        ] {
            let s = task_type.as_str();
            let parsed = TaskType::parse(s);
            assert_eq!(parsed, Some(task_type));
        }
    }

    #[test]
    fn task_status_parse_roundtrip() {
        for status in [
            TaskStatus::Pending,
            TaskStatus::InProgress,
            TaskStatus::Completed,
            TaskStatus::Failed,
        ] {
            let s = status.as_str();
            let parsed = TaskStatus::parse(s);
            assert_eq!(parsed, Some(status));
        }
    }

    #[test]
    fn task_type_display_name() {
        assert_eq!(TaskType::Custom.display_name(), "Custom Task");
        assert_eq!(TaskType::SyncWithMain.display_name(), "Sync with Main");
        assert_eq!(TaskType::AddTests.display_name(), "Add Tests");
        assert_eq!(TaskType::ReviewPolish.display_name(), "Review & Polish");
        assert_eq!(TaskType::FixLint.display_name(), "Fix Lint Errors");
    }

    #[test]
    fn task_type_default() {
        assert_eq!(TaskType::default(), TaskType::Custom);
    }

    #[test]
    fn task_status_default() {
        assert_eq!(TaskStatus::default(), TaskStatus::Pending);
    }

    #[test]
    fn get_task_by_id() {
        let db = create_test_db();
        let ticket_id = setup_ticket(&db);
        
        let task = db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::AddTests,
            title: Some("Test Task".to_string()),
            content: Some("Test content".to_string()),
        }).unwrap();
        
        let fetched = db.get_task(&task.id).unwrap();
        assert_eq!(fetched.id, task.id);
        assert_eq!(fetched.title, Some("Test Task".to_string()));
        assert_eq!(fetched.content, Some("Test content".to_string()));
        assert_eq!(fetched.task_type, TaskType::AddTests);
    }

    #[test]
    fn get_task_not_found() {
        let db = create_test_db();
        let result = db.get_task("nonexistent-id");
        assert!(matches!(result, Err(DbError::NotFound(_))));
    }

    #[test]
    fn update_task_title_and_content() {
        let db = create_test_db();
        let ticket_id = setup_ticket(&db);
        
        let task = db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::Custom,
            title: Some("Original".to_string()),
            content: Some("Original content".to_string()),
        }).unwrap();
        
        let updated = db.update_task(&task.id, &UpdateTask {
            title: Some("Updated".to_string()),
            content: Some("Updated content".to_string()),
            status: None,
            run_id: None,
        }).unwrap();
        
        assert_eq!(updated.title, Some("Updated".to_string()));
        assert_eq!(updated.content, Some("Updated content".to_string()));
    }

    #[test]
    fn update_task_partial() {
        let db = create_test_db();
        let ticket_id = setup_ticket(&db);
        
        let task = db.create_task(&CreateTask {
            ticket_id: ticket_id.clone(),
            task_type: TaskType::Custom,
            title: Some("Original".to_string()),
            content: Some("Original content".to_string()),
        }).unwrap();
        
        // Update only title, content should be preserved
        let updated = db.update_task(&task.id, &UpdateTask {
            title: Some("New Title".to_string()),
            content: None,
            status: None,
            run_id: None,
        }).unwrap();
        
        assert_eq!(updated.title, Some("New Title".to_string()));
        assert_eq!(updated.content, Some("Original content".to_string()));
    }

    #[test]
    fn task_type_parse_invalid() {
        assert_eq!(TaskType::parse("invalid"), None);
        assert_eq!(TaskType::parse(""), None);
    }

    #[test]
    fn task_status_parse_invalid() {
        assert_eq!(TaskStatus::parse("invalid"), None);
        assert_eq!(TaskStatus::parse(""), None);
    }
}
