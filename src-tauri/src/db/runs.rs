use crate::agents::cost::{AggregatedCost, RunCostData};
use crate::db::models::{AgentRun, AgentRunWithContext, CreateRun, RunStatus};
use crate::db::{parse_datetime, Database, DbError};

const AGENT_RUN_COLUMNS: &str =
    "id, ticket_id, agent_type, repo_path, status, started_at, ended_at, exit_code, summary_md, metadata_json, parent_run_id, stage, resumed_from_run_id";

fn agent_run_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentRun> {
    let status_str: String = row.get(4)?;
    let metadata_json: Option<String> = row.get(9)?;

    Ok(AgentRun {
        id: row.get(0)?,
        ticket_id: row.get(1)?,
        agent_type: row.get(2)?,
        repo_path: row.get(3)?,
        status: RunStatus::parse(&status_str).unwrap_or(RunStatus::Error),
        started_at: parse_datetime(row.get(5)?),
        ended_at: row.get::<_, Option<String>>(6)?.map(parse_datetime),
        exit_code: row.get(7)?,
        summary_md: row.get(8)?,
        metadata: metadata_json.and_then(|s| serde_json::from_str(&s).ok()),
        parent_run_id: row.get(10)?,
        stage: row.get(11)?,
        resumed_from_run_id: row.get(12)?,
    })
}

impl Database {
    pub fn get_run(&self, run_id: &str) -> Result<AgentRun, DbError> {
        self.with_conn(|conn| {
            let sql = format!("SELECT {} FROM agent_runs WHERE id = ?", AGENT_RUN_COLUMNS);
            let mut stmt = conn.prepare(&sql)?;

            stmt.query_row([run_id], agent_run_from_row)
            .map_err(|e| match e {
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
                   (id, ticket_id, agent_type, repo_path, status, started_at, parent_run_id, stage, resumed_from_run_id)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
                rusqlite::params![
                    run_id,
                    run.ticket_id,
                    run.agent_type,
                    run.repo_path,
                    RunStatus::Queued.as_str(),
                    now.to_rfc3339(),
                    run.parent_run_id,
                    run.stage,
                    run.resumed_from_run_id,
                ],
            )?;

            Ok(AgentRun {
                id: run_id,
                ticket_id: run.ticket_id.clone(),
                agent_type: run.agent_type.clone(),
                repo_path: run.repo_path.clone(),
                status: RunStatus::Queued,
                started_at: now,
                ended_at: None,
                exit_code: None,
                summary_md: None,
                metadata: None,
                parent_run_id: run.parent_run_id.clone(),
                stage: run.stage.clone(),
                resumed_from_run_id: run.resumed_from_run_id.clone(),
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

    /// Update a run's metadata (used to store stage outputs for resume)
    pub fn set_run_metadata(
        &self,
        run_id: &str,
        metadata: &serde_json::Value,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let metadata_json = serde_json::to_string(metadata)
                .map_err(|e| DbError::Validation(format!("Failed to serialize metadata: {}", e)))?;

            conn.execute(
                "UPDATE agent_runs SET metadata_json = ? WHERE id = ?",
                rusqlite::params![metadata_json, run_id],
            )?;
            Ok(())
        })
    }

    /// Merge new fields into a run's existing metadata, preserving existing keys.
    pub fn merge_run_metadata(
        &self,
        run_id: &str,
        new_fields: &serde_json::Value,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let existing: Option<String> = conn
                .query_row(
                    "SELECT metadata_json FROM agent_runs WHERE id = ?",
                    [run_id],
                    |row| row.get(0),
                )
                .ok()
                .flatten();

            let mut merged = match existing {
                Some(ref json_str) => serde_json::from_str::<serde_json::Value>(json_str)
                    .unwrap_or_else(|_| serde_json::json!({})),
                None => serde_json::json!({}),
            };

            if let (Some(base), Some(additions)) = (merged.as_object_mut(), new_fields.as_object())
            {
                for (k, v) in additions {
                    base.insert(k.clone(), v.clone());
                }
            }

            let metadata_json = serde_json::to_string(&merged)
                .map_err(|e| DbError::Validation(format!("Failed to serialize metadata: {}", e)))?;

            conn.execute(
                "UPDATE agent_runs SET metadata_json = ? WHERE id = ?",
                rusqlite::params![metadata_json, run_id],
            )?;
            Ok(())
        })
    }

    /// Get completed stage outputs from sub-runs of a parent run
    /// Returns a map of stage name -> extracted output text
    pub fn get_completed_stage_outputs(
        &self,
        parent_run_id: &str,
    ) -> Result<std::collections::HashMap<String, String>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT stage, metadata_json FROM agent_runs 
                   WHERE parent_run_id = ? AND status = 'finished' AND stage IS NOT NULL
                   ORDER BY started_at ASC"#,
            )?;

            let mut outputs = std::collections::HashMap::new();
            let rows = stmt.query_map([parent_run_id], |row| {
                let stage: String = row.get(0)?;
                let metadata_json: Option<String> = row.get(1)?;
                Ok((stage, metadata_json))
            })?;

            for row in rows {
                let (stage, metadata_json) = row?;
                if let Some(json_str) = metadata_json {
                    if let Ok(metadata) = serde_json::from_str::<serde_json::Value>(&json_str) {
                        if let Some(output) = metadata.get("stage_output").and_then(|v| v.as_str())
                        {
                            outputs
                                .entry(stage)
                                .and_modify(|existing: &mut String| {
                                    existing.push_str("\n\n");
                                    existing.push_str(output);
                                })
                                .or_insert_with(|| output.to_string());
                        }
                    }
                }
            }

            Ok(outputs)
        })
    }

    pub fn get_runs(&self, ticket_id: &str) -> Result<Vec<AgentRun>, DbError> {
        self.with_conn(|conn| {
            let sql = format!(
                "SELECT {} FROM agent_runs WHERE ticket_id = ? ORDER BY started_at DESC",
                AGENT_RUN_COLUMNS
            );
            let mut stmt = conn.prepare(&sql)?;
            let runs = stmt
                .query_map([ticket_id], agent_run_from_row)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(runs)
        })
    }

    /// Get recent runs with full context (board, project, ticket info) for the runs view.
    /// Uses LEFT JOINs so runs referencing specs (planner/brainstorm) are also returned.
    pub fn get_recent_runs_with_context(&self, limit: u32) -> Result<Vec<AgentRunWithContext>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT 
                    r.id, r.ticket_id, r.agent_type, r.repo_path, r.status, 
                    r.started_at, r.ended_at, r.exit_code, r.summary_md, r.metadata_json,
                    r.parent_run_id, r.stage, r.resumed_from_run_id,
                    COALESCE(t.title, s.name) as ticket_title,
                    COALESCE(t.board_id, s.board_id) as board_id,
                    COALESCE(b.name, sb.name) as board_name,
                    COALESCE(t.project_id, s.project_id) as project_id,
                    COALESCE(p.name, sp.name) as project_name,
                    (SELECT stage FROM agent_runs sub 
                     WHERE sub.parent_run_id = r.id AND sub.status = 'running' 
                     ORDER BY sub.started_at DESC LIMIT 1) as current_stage,
                    (SELECT COUNT(*) FROM agent_runs sub 
                     WHERE sub.parent_run_id = r.id AND sub.status = 'finished') as completed_stages,
                    (SELECT COUNT(*) FROM agent_runs sub 
                     WHERE sub.parent_run_id = r.id) as total_stages
                FROM agent_runs r
                LEFT JOIN tickets t ON r.ticket_id = t.id
                LEFT JOIN boards b ON t.board_id = b.id
                LEFT JOIN projects p ON t.project_id = p.id
                LEFT JOIN specs s ON r.ticket_id = s.id
                LEFT JOIN boards sb ON s.board_id = sb.id
                LEFT JOIN projects sp ON s.project_id = sp.id
                WHERE r.parent_run_id IS NULL
                ORDER BY r.started_at DESC
                LIMIT ?"#,
            )?;

            let mut runs = stmt
                .query_map([limit], |row| {
                    Ok(AgentRunWithContext {
                        run: agent_run_from_row(row)?,
                        ticket_title: row.get(13)?,
                        board_id: row.get(14)?,
                        board_name: row.get(15)?,
                        project_id: row.get(16)?,
                        project_name: row.get(17)?,
                        current_stage: row.get(18)?,
                        completed_stages: row.get::<_, i64>(19)? as u32,
                        total_stages: row.get::<_, i64>(20)? as u32,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;

            // For multi-stage parent runs that don't already have cost in their
            // metadata, aggregate sub-run costs so the frontend can display them.
            for run in &mut runs {
                if run.total_stages == 0 {
                    continue;
                }
                let has_cost = run
                    .run
                    .metadata
                    .as_ref()
                    .and_then(|m| m.get("cost"))
                    .is_some();
                if has_cost {
                    continue;
                }

                let agg = aggregate_sub_run_costs(conn, &run.run.id);
                if agg.run_count > 0 {
                    let metadata = run
                        .run
                        .metadata
                        .get_or_insert_with(|| serde_json::json!({}));
                    if let Some(obj) = metadata.as_object_mut() {
                        if let Ok(cost_val) = serde_json::to_value(&agg) {
                            obj.insert("cost".to_string(), cost_val);
                        }
                    }
                }
            }

            Ok(runs)
        })
    }

    /// Get the current stage of a parent run by finding the latest running or finished sub-run.
    /// Returns the stage name if found, or None if no sub-runs exist.
    pub fn get_current_run_stage(&self, parent_run_id: &str) -> Result<Option<String>, DbError> {
        self.with_conn(|conn| {
            // First try to find a running sub-run
            let running_stage: Option<String> = conn
                .query_row(
                    r#"SELECT stage FROM agent_runs 
                   WHERE parent_run_id = ? AND status = 'running' AND stage IS NOT NULL
                   ORDER BY started_at DESC LIMIT 1"#,
                    [parent_run_id],
                    |row| row.get(0),
                )
                .ok();

            if running_stage.is_some() {
                return Ok(running_stage);
            }

            // Fall back to the most recent sub-run (finished or otherwise)
            let latest_stage: Option<String> = conn
                .query_row(
                    r#"SELECT stage FROM agent_runs 
                   WHERE parent_run_id = ? AND stage IS NOT NULL
                   ORDER BY started_at DESC LIMIT 1"#,
                    [parent_run_id],
                    |row| row.get(0),
                )
                .ok();

            Ok(latest_stage)
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

/// Aggregate cost data from all sub-runs of a parent run.
fn aggregate_sub_run_costs(conn: &rusqlite::Connection, parent_run_id: &str) -> AggregatedCost {
    let mut agg = AggregatedCost::default();

    let mut stmt = match conn.prepare(
        "SELECT metadata_json FROM agent_runs WHERE parent_run_id = ? AND metadata_json IS NOT NULL",
    ) {
        Ok(s) => s,
        Err(_) => return agg,
    };

    let rows = match stmt.query_map([parent_run_id], |row| row.get::<_, String>(0)) {
        Ok(r) => r,
        Err(_) => return agg,
    };

    for json_str in rows.flatten() {
        if let Ok(metadata) = serde_json::from_str::<serde_json::Value>(&json_str) {
            if let Some(cost_value) = metadata.get("cost") {
                if let Ok(cost) = serde_json::from_value::<RunCostData>(cost_value.clone()) {
                    agg.add(&cost);
                }
            }
        }
    }

    agg
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

        let ticket = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: columns[0].id.clone(),
                title: "Ticket".to_string(),
                description_md: "".to_string(),
                priority: Priority::Low,
                labels: vec![],
                project_id: None,
                workflow_type: WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: None,
            })
            .unwrap();

        let run = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "cursor".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: None,
                stage: None,
                ..Default::default()
            })
            .unwrap();

        assert_eq!(run.status, RunStatus::Queued);
        assert_eq!(run.agent_type, "cursor");

        let runs = db.get_runs(&ticket.id).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].id, run.id);
    }

    #[test]
    fn update_run_status() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();

        let ticket = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: columns[0].id.clone(),
                title: "Ticket".to_string(),
                description_md: "".to_string(),
                priority: Priority::Low,
                labels: vec![],
                project_id: None,
                workflow_type: WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: None,
            })
            .unwrap();

        let run = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "claude".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: None,
                stage: None,
                ..Default::default()
            })
            .unwrap();

        db.update_run_status(&run.id, RunStatus::Finished, Some(0), Some("Done"))
            .unwrap();

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

        let ticket = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: columns[0].id.clone(),
                title: "Ticket".to_string(),
                description_md: "".to_string(),
                priority: Priority::Low,
                labels: vec![],
                project_id: None,
                workflow_type: WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: None,
            })
            .unwrap();

        let created = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "cursor".to_string(),
                repo_path: "/tmp/repo".to_string(),
                parent_run_id: None,
                stage: None,
                ..Default::default()
            })
            .unwrap();

        let fetched = db.get_run(&created.id).unwrap();
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.ticket_id, ticket.id);
        assert_eq!(fetched.agent_type, "cursor");
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

        let ticket = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: columns[0].id.clone(),
                title: "Ticket".to_string(),
                description_md: "".to_string(),
                priority: Priority::Low,
                labels: vec![],
                project_id: None,
                workflow_type: WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: None,
            })
            .unwrap();

        let run = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "cursor".to_string(),
                repo_path: "/tmp/repo".to_string(),
                parent_run_id: None,
                stage: None,
                ..Default::default()
            })
            .unwrap();

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

        let ticket = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: columns[0].id.clone(),
                title: "Ticket".to_string(),
                description_md: "".to_string(),
                priority: Priority::Low,
                labels: vec![],
                project_id: None,
                workflow_type: WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: None,
            })
            .unwrap();

        let run = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "claude".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: None,
                stage: None,
                ..Default::default()
            })
            .unwrap();

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

    fn temp_dir_path() -> String {
        std::env::temp_dir().to_string_lossy().to_string()
    }

    #[test]
    fn get_recent_runs_with_context_returns_empty_when_no_runs() {
        let db = create_test_db();
        let result = db.get_recent_runs_with_context(10).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn get_recent_runs_with_context_returns_run_with_ticket_and_board_info() {
        use crate::db::models::CreateProject;

        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();

        let project = db
            .create_project(&CreateProject {
                name: "Test Project".to_string(),
            path: temp_dir_path(),
            requires_git: false,
            })
            .unwrap();

        let ticket = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: columns[0].id.clone(),
                title: "Test Ticket".to_string(),
                description_md: "".to_string(),
                priority: Priority::Medium,
                labels: vec![],
                project_id: Some(project.id.clone()),
                workflow_type: WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: None,
            })
            .unwrap();

        let run = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "cursor".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: None,
                stage: None,
                ..Default::default()
            })
            .unwrap();

        let results = db.get_recent_runs_with_context(10).unwrap();
        assert_eq!(results.len(), 1);

        let result = &results[0];
        assert_eq!(result.run.id, run.id);
        assert_eq!(result.ticket_title, Some("Test Ticket".to_string()));
        assert_eq!(result.board_id, Some(board.id.clone()));
        assert_eq!(result.board_name, Some("Test Board".to_string()));
        assert_eq!(result.project_id, Some(project.id.clone()));
        assert_eq!(result.project_name, Some("Test Project".to_string()));
    }

    #[test]
    fn get_recent_runs_with_context_handles_run_without_project() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();

        let ticket = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: columns[0].id.clone(),
                title: "No Project Ticket".to_string(),
                description_md: "".to_string(),
                priority: Priority::Low,
                labels: vec![],
                project_id: None,
                workflow_type: WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: None,
            })
            .unwrap();

        db.create_run(&CreateRun {
            ticket_id: ticket.id.clone(),
            agent_type: "claude".to_string(),
            repo_path: "/tmp".to_string(),
            parent_run_id: None,
            stage: None,
            ..Default::default()
        })
        .unwrap();

        let results = db.get_recent_runs_with_context(10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].project_id, None);
        assert_eq!(results[0].project_name, None);
    }

    #[test]
    fn get_recent_runs_with_context_excludes_sub_runs() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();

        let ticket = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: columns[0].id.clone(),
                title: "Ticket".to_string(),
                description_md: "".to_string(),
                priority: Priority::Low,
                labels: vec![],
                project_id: None,
                workflow_type: WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: None,
            })
            .unwrap();

        let parent_run = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "cursor".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: None,
                stage: None,
                ..Default::default()
            })
            .unwrap();

        // Create a sub-run
        db.create_run(&CreateRun {
            ticket_id: ticket.id.clone(),
            agent_type: "cursor".to_string(),
            repo_path: "/tmp".to_string(),
            parent_run_id: Some(parent_run.id.clone()),
            stage: Some("code_review".to_string()),
            ..Default::default()
        })
        .unwrap();

        let results = db.get_recent_runs_with_context(10).unwrap();
        // Only the parent run should be returned
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].run.id, parent_run.id);
    }

    #[test]
    fn get_recent_runs_with_context_calculates_stage_counts() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();

        let ticket = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: columns[0].id.clone(),
                title: "Ticket".to_string(),
                description_md: "".to_string(),
                priority: Priority::Low,
                labels: vec![],
                project_id: None,
                workflow_type: WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: None,
            })
            .unwrap();

        let parent_run = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "cursor".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: None,
                stage: None,
                ..Default::default()
            })
            .unwrap();

        // Create 3 sub-runs, 2 finished
        let sub1 = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "cursor".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: Some(parent_run.id.clone()),
                stage: Some("build".to_string()),
                ..Default::default()
            })
            .unwrap();
        db.update_run_status(&sub1.id, RunStatus::Finished, Some(0), None)
            .unwrap();

        let sub2 = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "cursor".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: Some(parent_run.id.clone()),
                stage: Some("test".to_string()),
                ..Default::default()
            })
            .unwrap();
        db.update_run_status(&sub2.id, RunStatus::Finished, Some(0), None)
            .unwrap();

        // Third sub-run is still running
        db.create_run(&CreateRun {
            ticket_id: ticket.id.clone(),
            agent_type: "cursor".to_string(),
            repo_path: "/tmp".to_string(),
            parent_run_id: Some(parent_run.id.clone()),
            stage: Some("review".to_string()),
            ..Default::default()
        })
        .unwrap();

        let results = db.get_recent_runs_with_context(10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].total_stages, 3);
        assert_eq!(results[0].completed_stages, 2);
    }

    #[test]
    fn get_recent_runs_with_context_respects_limit() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();

        // Create 5 tickets with runs
        for i in 0..5 {
            let ticket = db
                .create_ticket(&CreateTicket {
                    board_id: board.id.clone(),
                    column_id: columns[0].id.clone(),
                    title: format!("Ticket {}", i),
                    description_md: "".to_string(),
                    priority: Priority::Low,
                    labels: vec![],
                    project_id: None,
                    workflow_type: WorkflowType::default(),
                    model: None,
                    branch_name: None,
                    is_epic: false,
                    epic_id: None,
                    depends_on_epic_id: None,
                    depends_on_epic_ids: vec![],
                    spec_version_id: None,
                })
                .unwrap();

            db.create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "cursor".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: None,
                stage: None,
                ..Default::default()
            })
            .unwrap();
        }

        let results = db.get_recent_runs_with_context(3).unwrap();
        assert_eq!(results.len(), 3);
    }

    fn create_ticket_for_board(db: &Database) -> (crate::db::models::Board, crate::db::models::Ticket) {
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let ticket = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: columns[0].id.clone(),
                title: "Ticket".to_string(),
                description_md: "".to_string(),
                priority: Priority::Low,
                labels: vec![],
                project_id: None,
                workflow_type: WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: None,
            })
            .unwrap();
        (board, ticket)
    }

    #[test]
    fn aggregate_sub_run_costs_returns_zero_when_no_sub_runs() {
        let db = create_test_db();
        let (_board, ticket) = create_ticket_for_board(&db);

        let parent = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "cursor".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: None,
                stage: None,
                ..Default::default()
            })
            .unwrap();

        db.with_conn(|conn| {
            let agg = aggregate_sub_run_costs(conn, &parent.id);
            assert_eq!(agg.run_count, 0);
            assert_eq!(agg.total_input_tokens, 0);
            assert_eq!(agg.total_output_tokens, 0);
            assert_eq!(agg.total_cost_usd, 0.0);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn aggregate_sub_run_costs_sums_multiple_sub_runs() {
        let db = create_test_db();
        let (_board, ticket) = create_ticket_for_board(&db);

        let parent = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "cursor".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: None,
                stage: None,
                ..Default::default()
            })
            .unwrap();

        let sub1 = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "cursor".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: Some(parent.id.clone()),
                stage: Some("build".to_string()),
                ..Default::default()
            })
            .unwrap();

        db.set_run_metadata(
            &sub1.id,
            &serde_json::json!({
                "cost": {
                    "inputTokens": 100,
                    "outputTokens": 50,
                    "cacheReadTokens": 10,
                    "cacheCreationTokens": 5,
                    "totalCostUsd": 0.01,
                    "isEstimated": false,
                    "modelUsage": {}
                }
            }),
        )
        .unwrap();

        let sub2 = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "cursor".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: Some(parent.id.clone()),
                stage: Some("test".to_string()),
                ..Default::default()
            })
            .unwrap();

        db.set_run_metadata(
            &sub2.id,
            &serde_json::json!({
                "cost": {
                    "inputTokens": 200,
                    "outputTokens": 75,
                    "cacheReadTokens": 20,
                    "cacheCreationTokens": 8,
                    "totalCostUsd": 0.02,
                    "isEstimated": true,
                    "modelUsage": {}
                }
            }),
        )
        .unwrap();

        db.with_conn(|conn| {
            let agg = aggregate_sub_run_costs(conn, &parent.id);
            assert_eq!(agg.run_count, 2);
            assert_eq!(agg.total_input_tokens, 300);
            assert_eq!(agg.total_output_tokens, 125);
            assert_eq!(agg.total_cache_read_tokens, 30);
            assert_eq!(agg.total_cache_creation_tokens, 13);
            assert_eq!(agg.estimated_count, 1);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn aggregate_sub_run_costs_skips_sub_runs_without_cost() {
        let db = create_test_db();
        let (_board, ticket) = create_ticket_for_board(&db);

        let parent = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "cursor".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: None,
                stage: None,
                ..Default::default()
            })
            .unwrap();

        let sub_with_cost = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "cursor".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: Some(parent.id.clone()),
                stage: Some("build".to_string()),
                ..Default::default()
            })
            .unwrap();

        db.set_run_metadata(
            &sub_with_cost.id,
            &serde_json::json!({
                "cost": {
                    "inputTokens": 500,
                    "outputTokens": 200,
                    "cacheReadTokens": 0,
                    "cacheCreationTokens": 0,
                    "totalCostUsd": 0.05,
                    "isEstimated": false,
                    "modelUsage": {}
                }
            }),
        )
        .unwrap();

        let sub_no_cost = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "cursor".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: Some(parent.id.clone()),
                stage: Some("review".to_string()),
                ..Default::default()
            })
            .unwrap();

        db.set_run_metadata(
            &sub_no_cost.id,
            &serde_json::json!({ "stage_output": "looks good" }),
        )
        .unwrap();

        // Third sub-run has no metadata at all (already the default)

        db.with_conn(|conn| {
            let agg = aggregate_sub_run_costs(conn, &parent.id);
            assert_eq!(agg.run_count, 1);
            assert_eq!(agg.total_input_tokens, 500);
            assert_eq!(agg.total_output_tokens, 200);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn recent_runs_with_context_aggregates_cost_for_multi_stage_parent() {
        let db = create_test_db();
        let (_board, ticket) = create_ticket_for_board(&db);

        let parent = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "cursor".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: None,
                stage: None,
                ..Default::default()
            })
            .unwrap();

        let sub = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "cursor".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: Some(parent.id.clone()),
                stage: Some("build".to_string()),
                ..Default::default()
            })
            .unwrap();

        db.set_run_metadata(
            &sub.id,
            &serde_json::json!({
                "cost": {
                    "inputTokens": 1000,
                    "outputTokens": 400,
                    "cacheReadTokens": 50,
                    "cacheCreationTokens": 25,
                    "totalCostUsd": 0.10,
                    "isEstimated": false,
                    "modelUsage": {}
                }
            }),
        )
        .unwrap();

        let results = db.get_recent_runs_with_context(10).unwrap();
        assert_eq!(results.len(), 1);

        let cost = results[0]
            .run
            .metadata
            .as_ref()
            .and_then(|m| m.get("cost"));
        assert!(cost.is_some(), "parent run should have aggregated cost");

        let cost_val = cost.unwrap();
        assert_eq!(cost_val.get("runCount").and_then(|v| v.as_u64()), Some(1));
        assert_eq!(
            cost_val.get("totalInputTokens").and_then(|v| v.as_u64()),
            Some(1000)
        );
    }

    #[test]
    fn recent_runs_with_context_skips_cost_aggregation_when_parent_already_has_cost() {
        let db = create_test_db();
        let (_board, ticket) = create_ticket_for_board(&db);

        let parent = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "cursor".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: None,
                stage: None,
                ..Default::default()
            })
            .unwrap();

        db.set_run_metadata(
            &parent.id,
            &serde_json::json!({
                "cost": { "preExisting": true }
            }),
        )
        .unwrap();

        let sub = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "cursor".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: Some(parent.id.clone()),
                stage: Some("build".to_string()),
                ..Default::default()
            })
            .unwrap();

        db.set_run_metadata(
            &sub.id,
            &serde_json::json!({
                "cost": {
                    "inputTokens": 999,
                    "outputTokens": 999,
                    "cacheReadTokens": 0,
                    "cacheCreationTokens": 0,
                    "totalCostUsd": 9.99,
                    "isEstimated": false,
                    "modelUsage": {}
                }
            }),
        )
        .unwrap();

        let results = db.get_recent_runs_with_context(10).unwrap();
        assert_eq!(results.len(), 1);

        let cost = results[0]
            .run
            .metadata
            .as_ref()
            .and_then(|m| m.get("cost"))
            .unwrap();
        assert_eq!(
            cost.get("preExisting").and_then(|v| v.as_bool()),
            Some(true),
            "should preserve the original cost, not overwrite with aggregated"
        );
    }

    #[test]
    fn arbitrary_agent_type_roundtrips_through_db() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();

        let ticket = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: columns[0].id.clone(),
                title: "Ticket".to_string(),
                description_md: "".to_string(),
                priority: Priority::Low,
                labels: vec![],
                project_id: None,
                workflow_type: WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: None,
            })
            .unwrap();

        let run = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "my-custom-agent".to_string(),
                repo_path: "/tmp/repo".to_string(),
                parent_run_id: None,
                stage: None,
                ..Default::default()
            })
            .unwrap();

        assert_eq!(run.agent_type, "my-custom-agent");

        let fetched = db.get_run(&run.id).unwrap();
        assert_eq!(fetched.agent_type, "my-custom-agent");

        let runs = db.get_runs(&ticket.id).unwrap();
        assert_eq!(runs[0].agent_type, "my-custom-agent");
    }

    #[test]
    fn merge_run_metadata_into_empty() {
        let db = create_test_db();
        let (_board, ticket) = create_ticket_for_board(&db);
        let run = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "claude".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: None,
                stage: None,
                ..Default::default()
            })
            .unwrap();

        db.merge_run_metadata(
            &run.id,
            &serde_json::json!({ "auto_pilot_selections": [{"command": "cleanup", "model": "sonnet-4.6"}] }),
        )
        .unwrap();

        let fetched = db.get_run(&run.id).unwrap();
        let meta = fetched.metadata.unwrap();
        let selections = meta.get("auto_pilot_selections").unwrap().as_array().unwrap();
        assert_eq!(selections.len(), 1);
        assert_eq!(selections[0]["command"], "cleanup");
    }

    #[test]
    fn merge_run_metadata_preserves_existing_keys() {
        let db = create_test_db();
        let (_board, ticket) = create_ticket_for_board(&db);
        let run = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "claude".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: None,
                stage: None,
                ..Default::default()
            })
            .unwrap();

        db.set_run_metadata(
            &run.id,
            &serde_json::json!({ "duration_secs": 42.0, "workflow_mode": "auto_pilot" }),
        )
        .unwrap();

        db.merge_run_metadata(
            &run.id,
            &serde_json::json!({ "auto_pilot_selections": [] }),
        )
        .unwrap();

        let fetched = db.get_run(&run.id).unwrap();
        let meta = fetched.metadata.unwrap();
        assert_eq!(meta.get("duration_secs").unwrap().as_f64().unwrap(), 42.0);
        assert_eq!(meta.get("workflow_mode").unwrap().as_str().unwrap(), "auto_pilot");
        assert!(meta.get("auto_pilot_selections").unwrap().as_array().unwrap().is_empty());
    }

    #[test]
    fn merge_run_metadata_overwrites_conflicting_keys() {
        let db = create_test_db();
        let (_board, ticket) = create_ticket_for_board(&db);
        let run = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "claude".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: None,
                stage: None,
                ..Default::default()
            })
            .unwrap();

        db.set_run_metadata(
            &run.id,
            &serde_json::json!({ "stage_output": "old value", "keep_me": true }),
        )
        .unwrap();

        db.merge_run_metadata(
            &run.id,
            &serde_json::json!({ "stage_output": "new value" }),
        )
        .unwrap();

        let fetched = db.get_run(&run.id).unwrap();
        let meta = fetched.metadata.unwrap();
        assert_eq!(meta.get("stage_output").unwrap().as_str().unwrap(), "new value");
        assert!(meta.get("keep_me").unwrap().as_bool().unwrap());
    }

    #[test]
    fn get_completed_stage_outputs_concatenates_duplicate_stages() {
        let db = create_test_db();
        let (_board, ticket) = create_ticket_for_board(&db);

        let parent = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "cursor".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: None,
                stage: None,
                ..Default::default()
            })
            .unwrap();

        // Create 3 sub-runs all with stage "implement" (simulating todo-based implementation)
        for (i, output) in ["output from todo 1", "output from todo 2", "output from todo 3"]
            .iter()
            .enumerate()
        {
            let sub = db
                .create_run(&CreateRun {
                    ticket_id: ticket.id.clone(),
                    agent_type: "cursor".to_string(),
                    repo_path: "/tmp".to_string(),
                    parent_run_id: Some(parent.id.clone()),
                    stage: Some("implement".to_string()),
                    ..Default::default()
                })
                .unwrap();

            db.update_run_status(&sub.id, RunStatus::Finished, Some(0), None)
                .unwrap();
            db.set_run_metadata(
                &sub.id,
                &serde_json::json!({
                    "stage_output": output,
                    "duration_secs": i as f64,
                }),
            )
            .unwrap();
        }

        let outputs = db.get_completed_stage_outputs(&parent.id).unwrap();
        let implement_output = outputs.get("implement").expect("should have implement key");
        assert!(
            implement_output.contains("output from todo 1"),
            "should contain first todo output"
        );
        assert!(
            implement_output.contains("output from todo 2"),
            "should contain second todo output"
        );
        assert!(
            implement_output.contains("output from todo 3"),
            "should contain third todo output"
        );
        assert!(
            implement_output.contains("\n\n"),
            "outputs should be separated by double newlines"
        );
    }

    #[test]
    fn get_completed_stage_outputs_single_stage_unchanged() {
        let db = create_test_db();
        let (_board, ticket) = create_ticket_for_board(&db);

        let parent = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "cursor".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: None,
                stage: None,
                ..Default::default()
            })
            .unwrap();

        let sub = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "cursor".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: Some(parent.id.clone()),
                stage: Some("implement".to_string()),
                ..Default::default()
            })
            .unwrap();

        db.update_run_status(&sub.id, RunStatus::Finished, Some(0), None)
            .unwrap();
        db.set_run_metadata(
            &sub.id,
            &serde_json::json!({ "stage_output": "single output" }),
        )
        .unwrap();

        let outputs = db.get_completed_stage_outputs(&parent.id).unwrap();
        assert_eq!(outputs.get("implement").unwrap(), "single output");
    }

    #[test]
    fn merge_run_metadata_nonexistent_run_succeeds() {
        let db = create_test_db();
        let result = db.merge_run_metadata(
            "nonexistent-run",
            &serde_json::json!({ "key": "value" }),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn create_run_allows_non_ticket_id() {
        let db = create_test_db();
        let run = db
            .create_run(&CreateRun {
                ticket_id: "spec-abc-123".to_string(),
                agent_type: "claude".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: None,
                stage: Some("brainstorm".to_string()),
                ..Default::default()
            })
            .unwrap();

        let fetched = db.get_run(&run.id).unwrap();
        assert_eq!(fetched.ticket_id, "spec-abc-123");
        assert_eq!(fetched.stage, Some("brainstorm".to_string()));
    }

    #[test]
    fn create_run_allows_spec_id_with_sub_runs() {
        let db = create_test_db();
        let parent = db
            .create_run(&CreateRun {
                ticket_id: "spec-xyz".to_string(),
                agent_type: "cursor".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: None,
                stage: Some("planner".to_string()),
                ..Default::default()
            })
            .unwrap();

        let sub = db
            .create_run(&CreateRun {
                ticket_id: "spec-xyz".to_string(),
                agent_type: "cursor".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: Some(parent.id.clone()),
                stage: Some("exploration".to_string()),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(sub.parent_run_id, Some(parent.id));
    }

    #[test]
    fn get_recent_runs_with_context_returns_spec_based_runs() {
        use crate::db::models::{CreateProject, CreateSpec};

        let db = create_test_db();
        let board = db.create_board("Spec Board").unwrap();
        let project = db
            .create_project(&CreateProject {
                name: "Spec Project".to_string(),
                path: temp_dir_path(),
                requires_git: false,
            })
            .unwrap();
        let spec = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: None,
                project_id: project.id.clone(),
                name: "My Feature Plan".to_string(),
                user_input: "Build auth".to_string(),
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();

        db.create_run(&CreateRun {
            ticket_id: spec.id.clone(),
            agent_type: "claude".to_string(),
            repo_path: "/tmp".to_string(),
            parent_run_id: None,
            stage: Some("planner".to_string()),
            ..Default::default()
        })
        .unwrap();

        let results = db.get_recent_runs_with_context(10).unwrap();
        assert_eq!(results.len(), 1);

        let r = &results[0];
        assert_eq!(r.run.ticket_id, spec.id);
        assert_eq!(r.ticket_title, Some("My Feature Plan".to_string()));
        assert_eq!(r.board_id, Some(board.id.clone()));
        assert_eq!(r.board_name, Some("Spec Board".to_string()));
        assert_eq!(r.project_id, Some(project.id.clone()));
        assert_eq!(r.project_name, Some("Spec Project".to_string()));
    }

    #[test]
    fn get_recent_runs_with_context_returns_orphan_runs_with_null_context() {
        let db = create_test_db();

        db.create_run(&CreateRun {
            ticket_id: "nonexistent-id".to_string(),
            agent_type: "claude".to_string(),
            repo_path: "/tmp".to_string(),
            parent_run_id: None,
            stage: Some("validation-chat".to_string()),
            ..Default::default()
        })
        .unwrap();

        let results = db.get_recent_runs_with_context(10).unwrap();
        assert_eq!(results.len(), 1);

        let r = &results[0];
        assert_eq!(r.ticket_title, None);
        assert_eq!(r.board_id, None);
        assert_eq!(r.board_name, None);
        assert_eq!(r.project_id, None);
        assert_eq!(r.project_name, None);
    }

    #[test]
    fn get_recent_runs_with_context_mixes_ticket_and_spec_runs() {
        use crate::db::models::{CreateProject, CreateSpec};

        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let project = db
            .create_project(&CreateProject {
                name: "Project".to_string(),
                path: temp_dir_path(),
                requires_git: false,
            })
            .unwrap();

        let ticket = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: columns[0].id.clone(),
                title: "Ticket Run".to_string(),
                description_md: "".to_string(),
                priority: Priority::Medium,
                labels: vec![],
                project_id: Some(project.id.clone()),
                workflow_type: WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: None,
            })
            .unwrap();

        let spec = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: None,
                project_id: project.id.clone(),
                name: "Spec Run".to_string(),
                user_input: "Plan something".to_string(),
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();

        // Ticket-based run
        db.create_run(&CreateRun {
            ticket_id: ticket.id.clone(),
            agent_type: "cursor".to_string(),
            repo_path: "/tmp".to_string(),
            parent_run_id: None,
            stage: None,
            ..Default::default()
        })
        .unwrap();

        // Spec-based run
        db.create_run(&CreateRun {
            ticket_id: spec.id.clone(),
            agent_type: "claude".to_string(),
            repo_path: "/tmp".to_string(),
            parent_run_id: None,
            stage: Some("brainstorm".to_string()),
            ..Default::default()
        })
        .unwrap();

        let results = db.get_recent_runs_with_context(10).unwrap();
        assert_eq!(results.len(), 2);

        let titles: Vec<_> = results
            .iter()
            .filter_map(|r| r.ticket_title.clone())
            .collect();
        assert!(titles.contains(&"Ticket Run".to_string()));
        assert!(titles.contains(&"Spec Run".to_string()));
    }

    #[test]
    fn get_recent_runs_with_context_aggregates_cost_for_spec_parent_runs() {
        use crate::db::models::{CreateProject, CreateSpec};

        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let project = db
            .create_project(&CreateProject {
                name: "Project".to_string(),
                path: temp_dir_path(),
                requires_git: false,
            })
            .unwrap();
        let spec = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: None,
                project_id: project.id.clone(),
                name: "Plan".to_string(),
                user_input: "Plan".to_string(),
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();

        let parent = db
            .create_run(&CreateRun {
                ticket_id: spec.id.clone(),
                agent_type: "claude".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: None,
                stage: Some("planner".to_string()),
                ..Default::default()
            })
            .unwrap();

        let sub = db
            .create_run(&CreateRun {
                ticket_id: spec.id.clone(),
                agent_type: "claude".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: Some(parent.id.clone()),
                stage: Some("exploration".to_string()),
                ..Default::default()
            })
            .unwrap();
        db.update_run_status(&sub.id, RunStatus::Finished, Some(0), None)
            .unwrap();
        db.set_run_metadata(
            &sub.id,
            &serde_json::json!({
                "cost": {
                    "inputTokens": 100,
                    "outputTokens": 50,
                    "cacheReadTokens": 0,
                    "cacheCreationTokens": 0,
                    "totalCostUsd": 0.005,
                    "modelUsage": {},
                    "isEstimated": false
                }
            }),
        )
        .unwrap();

        let results = db.get_recent_runs_with_context(10).unwrap();
        assert_eq!(results.len(), 1);

        let cost = results[0]
            .run
            .metadata
            .as_ref()
            .and_then(|m| m.get("cost"));
        assert!(cost.is_some(), "Parent run should have aggregated cost");
    }
}
