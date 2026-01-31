use crate::db::{Database, DbError, parse_datetime};
use crate::db::models::{AgentRun, CreateRun, AgentType, RunStatus};

impl Database {
    pub fn get_run(&self, run_id: &str) -> Result<AgentRun, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, ticket_id, agent_type, repo_path, status, 
                          started_at, ended_at, exit_code, summary_md, metadata_json,
                          parent_run_id, stage
                   FROM agent_runs WHERE id = ?"#
            )?;
            
            stmt.query_row([run_id], |row| {
                let agent_type_str: String = row.get(2)?;
                let status_str: String = row.get(4)?;
                let metadata_json: Option<String> = row.get(9)?;
                
                Ok(AgentRun {
                    id: row.get(0)?,
                    ticket_id: row.get(1)?,
                    agent_type: match agent_type_str.as_str() {
                        "cursor" => AgentType::Cursor,
                        _ => AgentType::Claude,
                    },
                    repo_path: row.get(3)?,
                    status: RunStatus::parse(&status_str).unwrap_or(RunStatus::Error),
                    started_at: parse_datetime(row.get(5)?),
                    ended_at: row.get::<_, Option<String>>(6)?.map(parse_datetime),
                    exit_code: row.get(7)?,
                    summary_md: row.get(8)?,
                    metadata: metadata_json.and_then(|s| serde_json::from_str(&s).ok()),
                    parent_run_id: row.get(10)?,
                    stage: row.get(11)?,
                })
            }).map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    DbError::NotFound(format!("Run {}", run_id))
                }
                other => DbError::Sqlite(other),
            })
        })
    }

    pub fn create_run(&self, run: &CreateRun) -> Result<AgentRun, DbError> {
        self.with_conn(|conn| {
            let run_id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now();
            
            conn.execute(
                r#"INSERT INTO agent_runs 
                   (id, ticket_id, agent_type, repo_path, status, started_at, parent_run_id, stage)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
                rusqlite::params![
                    run_id,
                    run.ticket_id,
                    run.agent_type.as_str(),
                    run.repo_path,
                    RunStatus::Queued.as_str(),
                    now.to_rfc3339(),
                    run.parent_run_id,
                    run.stage,
                ],
            )?;

            Ok(AgentRun {
                id: run_id,
                ticket_id: run.ticket_id.clone(),
                agent_type: run.agent_type,
                repo_path: run.repo_path.clone(),
                status: RunStatus::Queued,
                started_at: now,
                ended_at: None,
                exit_code: None,
                summary_md: None,
                metadata: None,
                parent_run_id: run.parent_run_id.clone(),
                stage: run.stage.clone(),
            })
        })
    }

    pub fn update_run_status(
        &self,
        run_id: &str,
        status: RunStatus,
        exit_code: Option<i32>,
        summary_md: Option<&str>,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now();
            let ended_at = if matches!(status, RunStatus::Finished | RunStatus::Error | RunStatus::Aborted) {
                Some(now.to_rfc3339())
            } else {
                None
            };
            
            conn.execute(
                "UPDATE agent_runs SET status = ?, ended_at = ?, exit_code = ?, summary_md = ? WHERE id = ?",
                rusqlite::params![status.as_str(), ended_at, exit_code, summary_md, run_id],
            )?;
            Ok(())
        })
    }

    pub fn get_runs(&self, ticket_id: &str) -> Result<Vec<AgentRun>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, ticket_id, agent_type, repo_path, status, 
                          started_at, ended_at, exit_code, summary_md, metadata_json,
                          parent_run_id, stage
                   FROM agent_runs WHERE ticket_id = ? ORDER BY started_at DESC"#
            )?;
            
            let runs = stmt.query_map([ticket_id], |row| {
                let agent_type_str: String = row.get(2)?;
                let status_str: String = row.get(4)?;
                let metadata_json: Option<String> = row.get(9)?;
                
                Ok(AgentRun {
                    id: row.get(0)?,
                    ticket_id: row.get(1)?,
                    agent_type: match agent_type_str.as_str() {
                        "cursor" => AgentType::Cursor,
                        _ => AgentType::Claude,
                    },
                    repo_path: row.get(3)?,
                    status: RunStatus::parse(&status_str).unwrap_or(RunStatus::Error),
                    started_at: parse_datetime(row.get(5)?),
                    ended_at: row.get::<_, Option<String>>(6)?.map(parse_datetime),
                    exit_code: row.get(7)?,
                    summary_md: row.get(8)?,
                    metadata: metadata_json.and_then(|s| serde_json::from_str(&s).ok()),
                    parent_run_id: row.get(10)?,
                    stage: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
            
            Ok(runs)
        })
    }

    /// Get recent runs across all tickets (for the runs view)
    pub fn get_recent_runs(&self, limit: u32) -> Result<Vec<AgentRun>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, ticket_id, agent_type, repo_path, status, 
                          started_at, ended_at, exit_code, summary_md, metadata_json,
                          parent_run_id, stage
                   FROM agent_runs 
                   WHERE parent_run_id IS NULL
                   ORDER BY started_at DESC 
                   LIMIT ?"#
            )?;
            
            let runs = stmt.query_map([limit], |row| {
                let agent_type_str: String = row.get(2)?;
                let status_str: String = row.get(4)?;
                let metadata_json: Option<String> = row.get(9)?;
                
                Ok(AgentRun {
                    id: row.get(0)?,
                    ticket_id: row.get(1)?,
                    agent_type: match agent_type_str.as_str() {
                        "cursor" => AgentType::Cursor,
                        _ => AgentType::Claude,
                    },
                    repo_path: row.get(3)?,
                    status: RunStatus::parse(&status_str).unwrap_or(RunStatus::Error),
                    started_at: parse_datetime(row.get(5)?),
                    ended_at: row.get::<_, Option<String>>(6)?.map(parse_datetime),
                    exit_code: row.get(7)?,
                    summary_md: row.get(8)?,
                    metadata: metadata_json.and_then(|s| serde_json::from_str(&s).ok()),
                    parent_run_id: row.get(10)?,
                    stage: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
            
            Ok(runs)
        })
    }

    /// Get all sub-runs for a parent run
    pub fn get_sub_runs(&self, parent_run_id: &str) -> Result<Vec<AgentRun>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, ticket_id, agent_type, repo_path, status, 
                          started_at, ended_at, exit_code, summary_md, metadata_json,
                          parent_run_id, stage
                   FROM agent_runs WHERE parent_run_id = ? ORDER BY started_at ASC"#
            )?;
            
            let runs = stmt.query_map([parent_run_id], |row| {
                let agent_type_str: String = row.get(2)?;
                let status_str: String = row.get(4)?;
                let metadata_json: Option<String> = row.get(9)?;
                
                Ok(AgentRun {
                    id: row.get(0)?,
                    ticket_id: row.get(1)?,
                    agent_type: match agent_type_str.as_str() {
                        "cursor" => AgentType::Cursor,
                        _ => AgentType::Claude,
                    },
                    repo_path: row.get(3)?,
                    status: RunStatus::parse(&status_str).unwrap_or(RunStatus::Error),
                    started_at: parse_datetime(row.get(5)?),
                    ended_at: row.get::<_, Option<String>>(6)?.map(parse_datetime),
                    exit_code: row.get(7)?,
                    summary_md: row.get(8)?,
                    metadata: metadata_json.and_then(|s| serde_json::from_str(&s).ok()),
                    parent_run_id: row.get(10)?,
                    stage: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
            
            Ok(runs)
        })
    }
    
    /// Clean up stale runs that are stuck in "running" or "queued" status.
    /// This is useful for runs that crashed or were interrupted without proper cleanup.
    /// Returns the number of runs that were marked as aborted.
    pub fn cleanup_stale_running_status(&self) -> Result<u32, DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now();
            
            // Mark all "running" or "queued" runs as aborted
            let count = conn.execute(
                r#"UPDATE agent_runs 
                   SET status = ?, ended_at = ?, summary_md = COALESCE(summary_md, 'Run was stale - marked as aborted during cleanup')
                   WHERE status IN (?, ?)"#,
                rusqlite::params![
                    RunStatus::Aborted.as_str(),
                    now.to_rfc3339(),
                    RunStatus::Running.as_str(),
                    RunStatus::Queued.as_str(),
                ],
            )?;
            
            Ok(count as u32)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::{CreateTicket, Priority, WorkflowType};

    fn create_test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn create_and_get_runs() {
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
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            scratchpad_id: None,
        }).unwrap();
        
        let run = db.create_run(&CreateRun {
            ticket_id: ticket.id.clone(),
            agent_type: AgentType::Cursor,
            repo_path: "/tmp".to_string(),
            parent_run_id: None,
            stage: None,
        }).unwrap();
        
        assert_eq!(run.status, RunStatus::Queued);
        assert_eq!(run.agent_type, AgentType::Cursor);
        
        let runs = db.get_runs(&ticket.id).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].id, run.id);
    }

    #[test]
    fn update_run_status() {
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
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            scratchpad_id: None,
        }).unwrap();
        
        let run = db.create_run(&CreateRun {
            ticket_id: ticket.id.clone(),
            agent_type: AgentType::Claude,
            repo_path: "/tmp".to_string(),
            parent_run_id: None,
            stage: None,
        }).unwrap();
        
        db.update_run_status(&run.id, RunStatus::Finished, Some(0), Some("Done")).unwrap();
        
        let runs = db.get_runs(&ticket.id).unwrap();
        assert_eq!(runs[0].status, RunStatus::Finished);
        assert_eq!(runs[0].exit_code, Some(0));
        assert_eq!(runs[0].summary_md, Some("Done".to_string()));
        assert!(runs[0].ended_at.is_some());
    }

    #[test]
    fn get_run_by_id() {
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
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            scratchpad_id: None,
        }).unwrap();
        
        let created = db.create_run(&CreateRun {
            ticket_id: ticket.id.clone(),
            agent_type: AgentType::Cursor,
            repo_path: "/tmp/repo".to_string(),
            parent_run_id: None,
            stage: None,
        }).unwrap();
        
        let fetched = db.get_run(&created.id).unwrap();
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.ticket_id, ticket.id);
        assert_eq!(fetched.agent_type, AgentType::Cursor);
        assert_eq!(fetched.repo_path, "/tmp/repo");
        assert_eq!(fetched.status, RunStatus::Queued);
    }

    #[test]
    fn get_run_not_found() {
        let db = create_test_db();
        let result = db.get_run("nonexistent-run-id");
        assert!(matches!(result, Err(DbError::NotFound(_))));
    }

    #[test]
    fn update_and_get_run_artifacts() {
        use crate::db::RunArtifacts;
        
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
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            scratchpad_id: None,
        }).unwrap();
        
        let run = db.create_run(&CreateRun {
            ticket_id: ticket.id.clone(),
            agent_type: AgentType::Cursor,
            repo_path: "/tmp/repo".to_string(),
            parent_run_id: None,
            stage: None,
        }).unwrap();
        
        let artifacts = RunArtifacts {
            commit_hash: Some("abc123".to_string()),
            files_changed: vec!["src/main.rs".to_string(), "Cargo.toml".to_string()],
            diff_path: Some("/tmp/diff.patch".to_string()),
            transcript_path: None,
            log_path: Some("/tmp/log.txt".to_string()),
        };
        
        db.update_run_artifacts(&run.id, &artifacts).unwrap();
        
        let fetched = db.get_run_artifacts(&run.id).unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.commit_hash, Some("abc123".to_string()));
        assert_eq!(fetched.files_changed.len(), 2);
        assert_eq!(fetched.diff_path, Some("/tmp/diff.patch".to_string()));
        assert!(fetched.transcript_path.is_none());
        assert_eq!(fetched.log_path, Some("/tmp/log.txt".to_string()));
    }

    #[test]
    fn get_run_artifacts_none_when_not_set() {
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
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            scratchpad_id: None,
        }).unwrap();
        
        let run = db.create_run(&CreateRun {
            ticket_id: ticket.id.clone(),
            agent_type: AgentType::Claude,
            repo_path: "/tmp".to_string(),
            parent_run_id: None,
            stage: None,
        }).unwrap();
        
        let fetched = db.get_run_artifacts(&run.id).unwrap();
        assert!(fetched.is_none());
    }

    #[test]
    fn run_artifacts_serialization() {
        use crate::db::RunArtifacts;
        
        let artifacts = RunArtifacts {
            commit_hash: Some("def456".to_string()),
            files_changed: vec!["file.txt".to_string()],
            diff_path: None,
            transcript_path: Some("/path/to/transcript".to_string()),
            log_path: None,
        };
        
        let json = serde_json::to_string(&artifacts).unwrap();
        assert!(json.contains("commitHash"));
        assert!(json.contains("filesChanged"));
        
        let parsed: RunArtifacts = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.commit_hash, artifacts.commit_hash);
        assert_eq!(parsed.files_changed, artifacts.files_changed);
    }
}
