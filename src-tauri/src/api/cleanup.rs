//! Background service for cleaning up expired ticket locks.

use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;

use crate::db::{Database, DbError, RunStatus};

pub struct CleanupConfig {
    pub check_interval_secs: u64,
}

impl Default for CleanupConfig {
    fn default() -> Self {
        Self {
            check_interval_secs: 60,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CleanupResult {
    pub released_tickets: Vec<String>,
    pub aborted_runs: Vec<String>,
    pub released_repo_locks: usize,
}

impl CleanupResult {
    pub fn is_empty(&self) -> bool {
        self.released_tickets.is_empty() && self.aborted_runs.is_empty() && self.released_repo_locks == 0
    }
}

pub fn cleanup_expired_locks(db: &Database) -> Result<CleanupResult, DbError> {
    let released_repo_locks = db.cleanup_expired_repo_locks()?;
    db.with_conn(|conn| {
        let now = chrono::Utc::now().to_rfc3339();

        let mut stmt = conn.prepare(
            r#"SELECT id, locked_by_run_id 
               FROM tickets 
               WHERE locked_by_run_id IS NOT NULL 
               AND lock_expires_at IS NOT NULL 
               AND lock_expires_at < ?"#,
        )?;

        let expired: Vec<(String, String)> = stmt
            .query_map([&now], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;

        if expired.is_empty() {
            return Ok(CleanupResult {
                released_tickets: vec![],
                aborted_runs: vec![],
                released_repo_locks,
            });
        }

        let ticket_ids: Vec<String> = expired.iter().map(|(id, _)| id.clone()).collect();
        let run_ids: Vec<String> = expired.iter().map(|(_, run_id)| run_id.clone()).collect();

        conn.execute(
            r#"UPDATE tickets 
               SET locked_by_run_id = NULL, 
                   lock_expires_at = NULL, 
                   updated_at = ?
               WHERE lock_expires_at IS NOT NULL 
               AND lock_expires_at < ?"#,
            rusqlite::params![&now, &now],
        )?;

        for run_id in &run_ids {
            conn.execute(
                r#"UPDATE agent_runs 
                   SET status = ?, 
                       ended_at = ?,
                       summary_md = COALESCE(summary_md, 'Lock expired - run may have crashed')
                   WHERE id = ? AND status IN ('queued', 'running')"#,
                rusqlite::params![RunStatus::Aborted.as_str(), &now, run_id],
            )?;
        }

        Ok(CleanupResult {
            released_tickets: ticket_ids,
            aborted_runs: run_ids,
            released_repo_locks,
        })
    })
}

pub fn start_cleanup_service(db: Arc<Database>, config: CleanupConfig) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(config.check_interval_secs));

        tracing::info!(
            "Lock cleanup service started (interval: {}s)",
            config.check_interval_secs
        );

        loop {
            ticker.tick().await;

            match cleanup_expired_locks(&db) {
                Ok(result) => {
                    if !result.is_empty() {
                        tracing::info!(
                            "Cleanup: released {} expired ticket locks, {} repo locks, aborted {} runs",
                            result.released_tickets.len(),
                            result.released_repo_locks,
                            result.aborted_runs.len()
                        );

                        for ticket_id in &result.released_tickets {
                            tracing::debug!("Released expired lock on ticket {}", ticket_id);
                        }

                        for run_id in &result.aborted_runs {
                            tracing::debug!("Marked run {} as aborted due to lock expiration", run_id);
                        }
                        
                        if result.released_repo_locks > 0 {
                            tracing::debug!("Released {} expired repo locks", result.released_repo_locks);
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Lock cleanup error: {}", e);
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{CreateRun, CreateTicket, AgentType, Priority};
    use chrono::{Duration as ChronoDuration, Utc};

    fn setup_test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn cleanup_with_no_expired_locks() {
        let db = setup_test_db();
        let result = cleanup_expired_locks(&db).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn cleanup_releases_expired_lock() {
        let db = setup_test_db();

        // Create a board and ticket
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Test Ticket".to_string(),
            description_md: "Description".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            agent_pref: None,
        }).unwrap();

        // Create a run and lock the ticket with an expired time
        let run = db.create_run(&CreateRun {
            ticket_id: ticket.id.clone(),
            agent_type: AgentType::Cursor,
            repo_path: "/tmp/test".to_string(),
        }).unwrap();

        let expired_time = Utc::now() - ChronoDuration::minutes(5);
        db.lock_ticket(&ticket.id, &run.id, expired_time).unwrap();

        // Run cleanup
        let result = cleanup_expired_locks(&db).unwrap();

        assert_eq!(result.released_tickets.len(), 1);
        assert_eq!(result.released_tickets[0], ticket.id);

        // Verify ticket is now unlocked
        let updated_ticket = db.get_ticket(&ticket.id).unwrap();
        assert!(updated_ticket.locked_by_run_id.is_none());
        assert!(updated_ticket.lock_expires_at.is_none());
    }

    #[test]
    fn cleanup_does_not_release_valid_lock() {
        let db = setup_test_db();

        // Create a board and ticket
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Test Ticket".to_string(),
            description_md: "Description".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            agent_pref: None,
        }).unwrap();

        // Create a run and lock the ticket with a future expiration
        let run = db.create_run(&CreateRun {
            ticket_id: ticket.id.clone(),
            agent_type: AgentType::Cursor,
            repo_path: "/tmp/test".to_string(),
        }).unwrap();

        let future_time = Utc::now() + ChronoDuration::minutes(30);
        db.lock_ticket(&ticket.id, &run.id, future_time).unwrap();

        // Run cleanup
        let result = cleanup_expired_locks(&db).unwrap();

        assert!(result.is_empty());

        // Verify ticket is still locked
        let updated_ticket = db.get_ticket(&ticket.id).unwrap();
        assert_eq!(updated_ticket.locked_by_run_id, Some(run.id));
    }

    #[test]
    fn cleanup_marks_run_as_aborted() {
        let db = setup_test_db();

        // Create a board and ticket
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Test Ticket".to_string(),
            description_md: "Description".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            agent_pref: None,
        }).unwrap();

        // Create a run with running status
        let run = db.create_run(&CreateRun {
            ticket_id: ticket.id.clone(),
            agent_type: AgentType::Cursor,
            repo_path: "/tmp/test".to_string(),
        }).unwrap();

        db.update_run_status(&run.id, RunStatus::Running, None, None).unwrap();

        let expired_time = Utc::now() - ChronoDuration::minutes(5);
        db.lock_ticket(&ticket.id, &run.id, expired_time).unwrap();

        // Run cleanup
        let result = cleanup_expired_locks(&db).unwrap();

        assert_eq!(result.aborted_runs.len(), 1);

        // Verify run is now aborted
        let updated_run = db.get_run(&run.id).unwrap();
        assert_eq!(updated_run.status, RunStatus::Aborted);
    }

    #[test]
    fn cleanup_result_is_empty_check() {
        let empty = CleanupResult {
            released_tickets: vec![],
            aborted_runs: vec![],
            released_repo_locks: 0,
        };
        assert!(empty.is_empty());

        let with_tickets = CleanupResult {
            released_tickets: vec!["t1".to_string()],
            aborted_runs: vec![],
            released_repo_locks: 0,
        };
        assert!(!with_tickets.is_empty());

        let with_runs = CleanupResult {
            released_tickets: vec![],
            aborted_runs: vec!["r1".to_string()],
            released_repo_locks: 0,
        };
        assert!(!with_runs.is_empty());
        
        let with_repo_locks = CleanupResult {
            released_tickets: vec![],
            aborted_runs: vec![],
            released_repo_locks: 1,
        };
        assert!(!with_repo_locks.is_empty());
    }

    #[test]
    fn cleanup_config_default() {
        let config = CleanupConfig::default();
        assert_eq!(config.check_interval_secs, 60);
    }

    #[test]
    fn cleanup_handles_multiple_expired_tickets() {
        let db = setup_test_db();

        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();

        let ticket1 = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Ticket 1".to_string(),
            description_md: "Desc".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            agent_pref: None,
        }).unwrap();

        let ticket2 = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Ticket 2".to_string(),
            description_md: "Desc".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            agent_pref: None,
        }).unwrap();

        let run1 = db.create_run(&CreateRun {
            ticket_id: ticket1.id.clone(),
            agent_type: AgentType::Cursor,
            repo_path: "/tmp/test".to_string(),
        }).unwrap();

        let run2 = db.create_run(&CreateRun {
            ticket_id: ticket2.id.clone(),
            agent_type: AgentType::Claude,
            repo_path: "/tmp/test".to_string(),
        }).unwrap();

        let expired_time = Utc::now() - ChronoDuration::minutes(5);
        db.lock_ticket(&ticket1.id, &run1.id, expired_time).unwrap();
        db.lock_ticket(&ticket2.id, &run2.id, expired_time).unwrap();

        let result = cleanup_expired_locks(&db).unwrap();

        assert_eq!(result.released_tickets.len(), 2);
        assert_eq!(result.aborted_runs.len(), 2);
    }
    
    #[test]
    fn cleanup_releases_expired_repo_locks() {
        use crate::db::models::CreateProject;
        
        let db = setup_test_db();
        
        // Create a project
        let project = db.create_project(&CreateProject {
            name: "Test Project".to_string(),
            path: std::env::temp_dir().to_string_lossy().to_string(),
            preferred_agent: None,
            requires_git: true,
        }).unwrap();
        
        // Acquire an expired repo lock
        let expired_time = Utc::now() - ChronoDuration::minutes(5);
        db.acquire_repo_lock(&project.id, "old-run", expired_time).unwrap();
        
        // Run cleanup
        let result = cleanup_expired_locks(&db).unwrap();
        
        // Should have released the repo lock
        assert_eq!(result.released_repo_locks, 1);
        
        // New acquisition should succeed
        let new_expires = Utc::now() + ChronoDuration::minutes(30);
        let acquired = db.acquire_repo_lock(&project.id, "new-run", new_expires).unwrap();
        assert!(acquired);
    }
    
    #[test]
    fn cleanup_does_not_release_valid_repo_locks() {
        use crate::db::models::CreateProject;
        
        let db = setup_test_db();
        
        // Create a project
        let project = db.create_project(&CreateProject {
            name: "Test Project".to_string(),
            path: std::env::temp_dir().to_string_lossy().to_string(),
            preferred_agent: None,
            requires_git: true,
        }).unwrap();
        
        // Acquire a valid repo lock
        let valid_time = Utc::now() + ChronoDuration::minutes(30);
        db.acquire_repo_lock(&project.id, "current-run", valid_time).unwrap();
        
        // Run cleanup
        let result = cleanup_expired_locks(&db).unwrap();
        
        // Should not have released the repo lock
        assert_eq!(result.released_repo_locks, 0);
        
        // New acquisition should fail
        let acquired = db.acquire_repo_lock(&project.id, "new-run", valid_time).unwrap();
        assert!(!acquired);
    }
}
