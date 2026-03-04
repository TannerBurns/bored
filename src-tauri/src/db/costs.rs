use crate::agents::cost::{AggregatedCost, RunCostData};
use crate::db::{Database, DbError};

/// Parse cost data from a metadata JSON string, returning None if absent or malformed.
fn parse_cost_from_metadata(json_str: &str) -> Option<RunCostData> {
    let metadata: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let cost_value = metadata.get("cost")?;
    serde_json::from_value(cost_value.clone()).ok()
}

/// Aggregate cost data from an iterator of metadata JSON strings.
fn aggregate_metadata_rows(rows: impl Iterator<Item = String>) -> AggregatedCost {
    let mut aggregated = AggregatedCost::default();
    for json_str in rows {
        if let Some(cost) = parse_cost_from_metadata(&json_str) {
            aggregated.add(&cost);
        }
    }
    aggregated
}

impl Database {
    /// Get cost data for a single run from its metadata.
    pub fn get_run_cost(&self, run_id: &str) -> Result<Option<RunCostData>, DbError> {
        self.with_conn(|conn| {
            let metadata_json: Option<String> = conn
                .query_row(
                    "SELECT metadata_json FROM agent_runs WHERE id = ?",
                    [run_id],
                    |row| row.get(0),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        DbError::NotFound(format!("Run {}", run_id))
                    }
                    other => DbError::Sqlite(other),
                })?;

            Ok(metadata_json.and_then(|s| parse_cost_from_metadata(&s)))
        })
    }

    /// Get aggregated cost for a ticket across all its runs.
    ///
    /// For multi-stage workflows the cost lives on each sub-run.  Parent runs
    /// that *have* sub-runs are excluded to prevent double-counting.
    pub fn get_ticket_cost(&self, ticket_id: &str) -> Result<AggregatedCost, DbError> {
        self.aggregate_cost_by_query(
            r#"SELECT r.metadata_json FROM agent_runs r
               WHERE r.ticket_id = ? AND r.metadata_json IS NOT NULL
               AND (
                   r.parent_run_id IS NOT NULL
                   OR NOT EXISTS (
                       SELECT 1 FROM agent_runs sr WHERE sr.parent_run_id = r.id
                   )
               )"#,
            ticket_id,
        )
    }

    /// Get aggregated cost for an entire board.
    pub fn get_board_cost_summary(&self, board_id: &str) -> Result<AggregatedCost, DbError> {
        self.aggregate_cost_by_query(
            r#"SELECT r.metadata_json FROM agent_runs r
               JOIN tickets t ON r.ticket_id = t.id
               WHERE t.board_id = ? AND r.metadata_json IS NOT NULL
               AND (
                   r.parent_run_id IS NOT NULL
                   OR NOT EXISTS (
                       SELECT 1 FROM agent_runs sr WHERE sr.parent_run_id = r.id
                   )
               )"#,
            board_id,
        )
    }

    /// Get aggregated cost for all tickets belonging to a spec version.
    pub fn get_spec_version_cost(&self, version_id: &str) -> Result<AggregatedCost, DbError> {
        self.aggregate_cost_by_query(
            r#"SELECT r.metadata_json FROM agent_runs r
               JOIN tickets t ON r.ticket_id = t.id
               WHERE t.spec_version_id = ? AND r.metadata_json IS NOT NULL
               AND (
                   r.parent_run_id IS NOT NULL
                   OR NOT EXISTS (
                       SELECT 1 FROM agent_runs sr WHERE sr.parent_run_id = r.id
                   )
               )"#,
            version_id,
        )
    }

    /// Get aggregated cost for a chat across all its chat_runs.
    pub fn get_chat_cost(&self, chat_id: &str) -> Result<AggregatedCost, DbError> {
        self.aggregate_cost_by_query(
            r#"SELECT metadata_json FROM chat_runs
               WHERE chat_id = ? AND metadata_json IS NOT NULL"#,
            chat_id,
        )
    }

    /// Backfill cost data for completed runs that are missing it.
    /// Returns the number of runs that were backfilled.
    ///
    /// Parent runs that have sub-runs (multi-stage workflows) are skipped
    /// because their cost is captured on each sub-run.
    pub fn backfill_run_costs(
        &self,
        registry: &crate::agents::registry::AgentRegistry,
    ) -> Result<u32, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT r.id, r.agent_type, r.metadata_json, r.started_at, r.ended_at,
                          (SELECT GROUP_CONCAT(e.payload_json, char(10))
                           FROM agent_events e
                           WHERE e.run_id = r.id AND e.event_type LIKE '%log_stdout%'
                           ORDER BY e.created_at ASC) as log_events,
                          t.model
                   FROM agent_runs r
                   JOIN tickets t ON r.ticket_id = t.id
                   WHERE r.status IN ('finished', 'error')
                   AND (r.metadata_json IS NULL
                        OR r.metadata_json NOT LIKE '%"cost":%')
                   AND (
                       r.parent_run_id IS NOT NULL
                       OR NOT EXISTS (
                           SELECT 1 FROM agent_runs sr WHERE sr.parent_run_id = r.id
                       )
                   )"#,
            )?;

            let mut updates: Vec<(String, serde_json::Value)> = Vec::new();

            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            })?;

            for row in rows {
                let (run_id, agent_type, metadata_json, started_at, ended_at, log_concat, ticket_model) = row?;

                let duration_secs = compute_duration_secs(started_at.as_deref(), ended_at.as_deref());

                let stdout = log_concat.unwrap_or_default();
                let reconstructed = reconstruct_stdout_from_events(conn, &run_id);
                let full_stdout = if reconstructed.len() > stdout.len() {
                    reconstructed
                } else {
                    stdout
                };

                if full_stdout.is_empty() && duration_secs <= 0.0 {
                    continue;
                }

                let parsed_metadata = metadata_json
                    .as_deref()
                    .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());

                // Prefer the stage_model stored at execution time (captures per-stage
                // overrides from workflow settings). Fall back to ticket model, then
                // the global default.
                let stored_stage_model: Option<String> = parsed_metadata
                    .as_ref()
                    .and_then(|m| m.get("stage_model"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let model = stored_stage_model
                    .as_deref()
                    .or(ticket_model.as_deref())
                    .unwrap_or(crate::agents::models::DEFAULT_STAGE_MODEL);

                let stored_agent_config: Option<std::collections::HashMap<String, serde_json::Value>> =
                    parsed_metadata
                        .as_ref()
                        .and_then(|m| m.get("agent_config"))
                        .and_then(|v| serde_json::from_value(v.clone()).ok());

                let cost_data = if let Some(provider) = registry.get(&agent_type) {
                    if let Some(ref agent_config) = stored_agent_config {
                        crate::agents::provider::extract_cost_with_overrides(
                            &*provider,
                            &full_stdout,
                            model,
                            agent_config,
                            duration_secs,
                        )
                    } else {
                        provider.extract_cost(&full_stdout, model, duration_secs)
                    }
                } else {
                    let output_chars = full_stdout.len();
                    if output_chars > 0 || duration_secs > 0.0 {
                        Some(crate::agents::cost::estimate_cost(model, output_chars, duration_secs))
                    } else {
                        None
                    }
                };

                if let Some(cost) = cost_data {
                    let mut metadata = parsed_metadata
                        .unwrap_or_else(|| serde_json::json!({}));

                    metadata["cost"] = serde_json::to_value(&cost).unwrap_or_default();
                    if duration_secs > 0.0 && metadata.get("duration_secs").is_none() {
                        metadata["duration_secs"] = serde_json::json!(duration_secs);
                    }

                    updates.push((run_id, metadata));
                }
            }

            let backfilled = updates.len() as u32;
            for (run_id, metadata) in updates {
                let metadata_str =
                    serde_json::to_string(&metadata).unwrap_or_else(|_| "{}".to_string());
                conn.execute(
                    "UPDATE agent_runs SET metadata_json = ? WHERE id = ?",
                    rusqlite::params![metadata_str, run_id],
                )?;
            }

            Ok(backfilled)
        })
    }

    /// Shared aggregation: run a query that returns metadata_json rows and aggregate cost.
    fn aggregate_cost_by_query(&self, sql: &str, param: &str) -> Result<AggregatedCost, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt.query_map([param], |row| row.get::<_, String>(0))?;
            Ok(aggregate_metadata_rows(rows.flatten()))
        })
    }
}

fn compute_duration_secs(started_at: Option<&str>, ended_at: Option<&str>) -> f64 {
    let (start, end) = match (started_at, ended_at) {
        (Some(s), Some(e)) => (s, e),
        _ => return 0.0,
    };

    let parse = |ts: &str| {
        chrono::DateTime::parse_from_rfc3339(ts).or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S")
                .map(|ndt| ndt.and_utc().fixed_offset())
        })
    };

    match (parse(start), parse(end)) {
        (Ok(s), Ok(e)) => (e - s).num_seconds() as f64,
        _ => 0.0,
    }
}

fn reconstruct_stdout_from_events(conn: &rusqlite::Connection, run_id: &str) -> String {
    let mut stmt = match conn.prepare(
        r#"SELECT payload_json FROM agent_events
           WHERE run_id = ? AND event_type LIKE '%log_stdout%'
           ORDER BY created_at ASC"#,
    ) {
        Ok(s) => s,
        Err(_) => return String::new(),
    };

    let mut lines = Vec::new();
    if let Ok(rows) = stmt.query_map([run_id], |row| row.get::<_, String>(0)) {
        for row in rows.flatten() {
            if let Ok(payload) = serde_json::from_str::<serde_json::Value>(&row) {
                if let Some(raw) = payload.get("raw").and_then(|r| r.as_str()) {
                    lines.push(raw.to_string());
                    continue;
                }
            }
            lines.push(row);
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::{CreateRun, CreateTicket, Priority, WorkflowType};

    fn create_test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn create_ticket_and_run(db: &Database) -> (String, String) {
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let ticket = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: columns[0].id.clone(),
                title: "Cost Ticket".to_string(),
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
        (ticket.id, run.id)
    }

    fn cost_metadata(input_tokens: u64, cost_usd: f64, estimated: bool) -> serde_json::Value {
        serde_json::json!({
            "cost": {
                "inputTokens": input_tokens, "outputTokens": 0, "totalCostUsd": cost_usd,
                "cacheReadTokens": 0, "cacheCreationTokens": 0, "isEstimated": estimated,
                "modelUsage": {}
            }
        })
    }

    #[test]
    fn parse_cost_from_valid_metadata() {
        let json = r#"{"cost":{"inputTokens":100,"outputTokens":50,"cacheReadTokens":0,"cacheCreationTokens":0,"totalCostUsd":0.01,"isEstimated":false,"modelUsage":{}},"duration_secs":5.0}"#;
        let cost = parse_cost_from_metadata(json).unwrap();
        assert_eq!(cost.input_tokens, 100);
    }

    #[test]
    fn parse_cost_returns_none_for_no_cost_key() {
        assert!(parse_cost_from_metadata(r#"{"duration_secs":5.0}"#).is_none());
    }

    #[test]
    fn parse_cost_returns_none_for_invalid_json() {
        assert!(parse_cost_from_metadata("not json").is_none());
    }

    #[test]
    fn aggregate_empty_iterator() {
        let agg = aggregate_metadata_rows(std::iter::empty());
        assert_eq!(agg.run_count, 0);
    }

    #[test]
    fn aggregate_skips_invalid_rows() {
        let rows = vec![
            "not json".to_string(),
            r#"{"no_cost":true}"#.to_string(),
            r#"{"cost":{"inputTokens":100,"outputTokens":50,"cacheReadTokens":0,"cacheCreationTokens":0,"totalCostUsd":0.01,"isEstimated":false,"modelUsage":{}}}"#.to_string(),
        ];
        let agg = aggregate_metadata_rows(rows.into_iter());
        assert_eq!(agg.run_count, 1);
    }

    #[test]
    fn compute_duration_secs_rfc3339() {
        let d = compute_duration_secs(
            Some("2025-01-01T00:00:00+00:00"),
            Some("2025-01-01T00:01:00+00:00"),
        );
        assert!((d - 60.0).abs() < 0.1);
    }

    #[test]
    fn compute_duration_secs_naive_format() {
        let d = compute_duration_secs(Some("2025-01-01 00:00:00"), Some("2025-01-01 00:00:30"));
        assert!((d - 30.0).abs() < 0.1);
    }

    #[test]
    fn compute_duration_secs_missing_timestamps() {
        assert_eq!(compute_duration_secs(None, Some("2025-01-01T00:00:00+00:00")), 0.0);
        assert_eq!(compute_duration_secs(Some("2025-01-01T00:00:00+00:00"), None), 0.0);
        assert_eq!(compute_duration_secs(None, None), 0.0);
    }

    #[test]
    fn get_run_cost_returns_none_when_no_metadata() {
        let db = create_test_db();
        let (_, run_id) = create_ticket_and_run(&db);
        assert!(db.get_run_cost(&run_id).unwrap().is_none());
    }

    #[test]
    fn get_run_cost_returns_none_when_metadata_has_no_cost() {
        let db = create_test_db();
        let (_, run_id) = create_ticket_and_run(&db);
        db.set_run_metadata(&run_id, &serde_json::json!({"duration_secs": 10.0}))
            .unwrap();
        assert!(db.get_run_cost(&run_id).unwrap().is_none());
    }

    #[test]
    fn get_run_cost_returns_cost_from_metadata() {
        let db = create_test_db();
        let (_, run_id) = create_ticket_and_run(&db);
        db.set_run_metadata(&run_id, &cost_metadata(500, 0.05, false)).unwrap();
        let cost = db.get_run_cost(&run_id).unwrap().unwrap();
        assert_eq!(cost.input_tokens, 500);
        assert!((cost.total_cost_usd - 0.05).abs() < 0.001);
    }

    #[test]
    fn get_run_cost_not_found() {
        let db = create_test_db();
        assert!(matches!(db.get_run_cost("nonexistent"), Err(DbError::NotFound(_))));
    }

    #[test]
    fn get_ticket_cost_empty_when_no_runs() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let ticket = db
            .create_ticket(&CreateTicket {
                board_id: board.id,
                column_id: columns[0].id.clone(),
                title: "T".to_string(),
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
        let agg = db.get_ticket_cost(&ticket.id).unwrap();
        assert_eq!(agg.run_count, 0);
    }

    #[test]
    fn get_ticket_cost_aggregates_multiple_runs() {
        let db = create_test_db();
        let (ticket_id, run1_id) = create_ticket_and_run(&db);
        let run2 = db
            .create_run(&CreateRun {
                ticket_id: ticket_id.clone(),
                agent_type: "claude".to_string(),
                repo_path: "/tmp".to_string(),
                ..Default::default()
            })
            .unwrap();

        db.set_run_metadata(&run1_id, &cost_metadata(100, 0.01, false)).unwrap();
        db.set_run_metadata(&run2.id, &cost_metadata(200, 0.02, true)).unwrap();

        let agg = db.get_ticket_cost(&ticket_id).unwrap();
        assert_eq!(agg.run_count, 2);
        assert_eq!(agg.estimated_count, 1);
        assert_eq!(agg.total_input_tokens, 300);
        assert!((agg.total_cost_usd - 0.03).abs() < 0.001);
    }

    #[test]
    fn get_ticket_cost_skips_runs_without_cost() {
        let db = create_test_db();
        let (ticket_id, run1_id) = create_ticket_and_run(&db);
        db.set_run_metadata(&run1_id, &cost_metadata(100, 0.01, false)).unwrap();
        db.create_run(&CreateRun {
            ticket_id: ticket_id.clone(),
                agent_type: "cursor".to_string(),
            repo_path: "/tmp".to_string(),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(db.get_ticket_cost(&ticket_id).unwrap().run_count, 1);
    }

    #[test]
    fn get_ticket_cost_excludes_multi_stage_parent() {
        let db = create_test_db();
        let (ticket_id, parent_id) = create_ticket_and_run(&db);

        // Give the parent run cost metadata (simulates backfill on a parent)
        db.set_run_metadata(&parent_id, &cost_metadata(500, 0.10, true))
            .unwrap();

        // Create two sub-runs with their own cost data
        let sub1 = db
            .create_run(&CreateRun {
                ticket_id: ticket_id.clone(),
                agent_type: "claude".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: Some(parent_id.clone()),
                stage: Some("plan".to_string()),
                ..Default::default()
            })
            .unwrap();
        let sub2 = db
            .create_run(&CreateRun {
                ticket_id: ticket_id.clone(),
                agent_type: "claude".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: Some(parent_id.clone()),
                stage: Some("implement".to_string()),
                ..Default::default()
            })
            .unwrap();
        db.set_run_metadata(&sub1.id, &cost_metadata(100, 0.03, false))
            .unwrap();
        db.set_run_metadata(&sub2.id, &cost_metadata(200, 0.05, false))
            .unwrap();

        let agg = db.get_ticket_cost(&ticket_id).unwrap();

        // Should only count the two sub-runs, NOT the parent
        assert_eq!(
            agg.run_count, 2,
            "Parent run with sub-runs must be excluded"
        );
        assert!(
            (agg.total_cost_usd - 0.08).abs() < 0.001,
            "Total should be sub1 + sub2 = 0.08, got {}",
            agg.total_cost_usd
        );
    }

    #[test]
    fn get_ticket_cost_includes_single_stage_parent() {
        let db = create_test_db();
        let (ticket_id, run_id) = create_ticket_and_run(&db);

        // Single-stage run (no sub-runs) should be included
        db.set_run_metadata(&run_id, &cost_metadata(100, 0.03, false))
            .unwrap();

        let agg = db.get_ticket_cost(&ticket_id).unwrap();
        assert_eq!(agg.run_count, 1);
        assert!((agg.total_cost_usd - 0.03).abs() < 0.001);
    }

    #[test]
    fn get_ticket_cost_mixed_single_and_multi_stage() {
        let db = create_test_db();
        let (ticket_id, single_run_id) = create_ticket_and_run(&db);

        // Single-stage run with cost
        db.set_run_metadata(&single_run_id, &cost_metadata(100, 0.02, false))
            .unwrap();

        // Multi-stage parent (should be excluded)
        let parent = db
            .create_run(&CreateRun {
                ticket_id: ticket_id.clone(),
                agent_type: "claude".to_string(),
                repo_path: "/tmp".to_string(),
                ..Default::default()
            })
            .unwrap();
        db.set_run_metadata(&parent.id, &cost_metadata(500, 0.50, true))
            .unwrap();

        // Sub-runs (should be included)
        let sub1 = db
            .create_run(&CreateRun {
                ticket_id: ticket_id.clone(),
                agent_type: "claude".to_string(),
                repo_path: "/tmp".to_string(),
                parent_run_id: Some(parent.id.clone()),
                stage: Some("plan".to_string()),
                ..Default::default()
            })
            .unwrap();
        db.set_run_metadata(&sub1.id, &cost_metadata(200, 0.04, false))
            .unwrap();

        let agg = db.get_ticket_cost(&ticket_id).unwrap();

        // single_run + sub1 = 2 runs, parent excluded
        assert_eq!(agg.run_count, 2);
        assert!(
            (agg.total_cost_usd - 0.06).abs() < 0.001,
            "Total should be single(0.02) + sub1(0.04) = 0.06, got {}",
            agg.total_cost_usd
        );
    }

    #[test]
    fn get_board_cost_summary_aggregates_across_tickets() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();

        let make_ticket = |title: &str| {
            db.create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: columns[0].id.clone(),
                title: title.to_string(),
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
            .unwrap()
        };

        let t1 = make_ticket("T1");
        let t2 = make_ticket("T2");

        let r1 = db.create_run(&CreateRun { ticket_id: t1.id, agent_type: "claude".to_string(), repo_path: "/tmp".to_string(), ..Default::default() }).unwrap();
        let r2 = db.create_run(&CreateRun { ticket_id: t2.id, agent_type: "claude".to_string(), repo_path: "/tmp".to_string(), ..Default::default() }).unwrap();

        db.set_run_metadata(&r1.id, &cost_metadata(100, 0.01, false)).unwrap();
        db.set_run_metadata(&r2.id, &cost_metadata(200, 0.02, false)).unwrap();

        let agg = db.get_board_cost_summary(&board.id).unwrap();
        assert_eq!(agg.run_count, 2);
        assert_eq!(agg.total_input_tokens, 300);
    }

    // ── backfill_run_costs ───────────────────────────────────────────

    mod backfill_tests {
        use super::*;
        use crate::agents::cost::RunCostData;
        use crate::agents::provider::{AgentProvider, AgentRunConfig};
        use crate::agents::registry::AgentRegistry;
        use crate::db::models::{
            AgentEventPayload, EventType, NormalizedEvent, RunStatus,
        };
        use std::sync::Arc;

        /// Stub provider that returns fixed cost data from `extract_cost`.
        /// Its `id()` matches the DB agent_type string so the
        /// registry dispatch in `backfill_run_costs` can find it.
        #[derive(Debug)]
        struct CostStubProvider {
            agent_id: &'static str,
        }

        impl AgentProvider for CostStubProvider {
            fn id(&self) -> &str {
                self.agent_id
            }
            fn display_name(&self) -> &str {
                self.agent_id
            }
            fn build_command(&self, _: &AgentRunConfig) -> (String, Vec<String>) {
                (self.agent_id.to_string(), vec![])
            }
            fn build_env_vars(&self, _: &AgentRunConfig) -> Vec<(String, String)> {
                vec![]
            }
            fn extract_text(&self, output: &str) -> String {
                output.to_string()
            }
            fn extract_cost(
                &self,
                _stdout: &str,
                _model: &str,
                _duration: f64,
            ) -> Option<RunCostData> {
                Some(RunCostData {
                    input_tokens: 200,
                    output_tokens: 100,
                    cache_read_tokens: 0,
                    cache_creation_tokens: 0,
                    total_cost_usd: 0.03,
                    model_usage: Default::default(),
                    is_estimated: false,
                })
            }
            fn is_available(&self) -> bool {
                true
            }
            fn get_version(&self) -> Option<String> {
                None
            }
            fn config_dir_name(&self) -> &str {
                ".stub"
            }
            fn command_instructions_subdir(&self) -> &str {
                "commands"
            }
            fn format_command_reference(&self, cmd: &str) -> String {
                format!("/{}", cmd)
            }
        }

        fn make_registry() -> AgentRegistry {
            let mut reg = AgentRegistry::new();
            reg.register(Arc::new(CostStubProvider { agent_id: "claude" }));
            reg.register(Arc::new(CostStubProvider { agent_id: "cursor" }));
            reg
        }

        fn create_finished_run_without_cost(db: &Database) -> (String, String) {
            let board = db.create_board("Board").unwrap();
            let columns = db.get_columns(&board.id).unwrap();
            let ticket = db
                .create_ticket(&CreateTicket {
                    board_id: board.id.clone(),
                    column_id: columns[0].id.clone(),
                    title: "Backfill Ticket".to_string(),
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

            // Mark the run as finished so it qualifies for backfilling
            db.update_run_status(&run.id, RunStatus::Finished, Some(0), None)
                .unwrap();

            (ticket.id, run.id)
        }

        #[test]
        fn backfill_populates_cost_for_finished_run() {
            let db = create_test_db();
            let registry = make_registry();
            let (ticket_id, run_id) = create_finished_run_without_cost(&db);

            // Add a stdout log event so the backfill has something to extract from
            db.create_event(&NormalizedEvent {
                run_id: run_id.clone(),
                ticket_id: ticket_id.clone(),
                agent_type: "claude".to_string(),
                event_type: EventType::Custom("log_stdout".to_string()),
                payload: AgentEventPayload {
                    raw: Some("some agent output".to_string()),
                    structured: None,
                },
                timestamp: chrono::Utc::now(),
            })
            .unwrap();

            // Before backfill: no cost metadata
            assert!(db.get_run_cost(&run_id).unwrap().is_none());

            let count = db.backfill_run_costs(&registry).unwrap();
            assert_eq!(count, 1);

            // After backfill: cost metadata is present
            let cost = db.get_run_cost(&run_id).unwrap().unwrap();
            assert_eq!(cost.input_tokens, 200);
            assert_eq!(cost.output_tokens, 100);
            assert!((cost.total_cost_usd - 0.03).abs() < 0.001);
            assert!(!cost.is_estimated);
        }

        #[test]
        fn backfill_skips_runs_already_with_cost() {
            let db = create_test_db();
            let registry = make_registry();
            let (_, run_id) = create_finished_run_without_cost(&db);

            // Pre-populate cost metadata
            db.set_run_metadata(&run_id, &cost_metadata(500, 0.05, false))
                .unwrap();

            let count = db.backfill_run_costs(&registry).unwrap();
            assert_eq!(count, 0);
        }

        #[test]
        fn backfill_skips_parent_runs_with_sub_runs() {
            let db = create_test_db();
            let registry = make_registry();
            let board = db.create_board("Board").unwrap();
            let columns = db.get_columns(&board.id).unwrap();
            let ticket = db
                .create_ticket(&CreateTicket {
                    board_id: board.id.clone(),
                    column_id: columns[0].id.clone(),
                    title: "Parent Ticket".to_string(),
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

            // Create parent run
            let parent_run = db
                .create_run(&CreateRun {
                    ticket_id: ticket.id.clone(),
                    agent_type: "claude".to_string(),
                    repo_path: "/tmp".to_string(),
                    ..Default::default()
                })
                .unwrap();
            db.update_run_status(&parent_run.id, RunStatus::Finished, Some(0), None)
                .unwrap();

            // Create a sub-run under the parent
            let sub_run = db
                .create_run(&CreateRun {
                    ticket_id: ticket.id.clone(),
                    agent_type: "claude".to_string(),
                    repo_path: "/tmp".to_string(),
                    parent_run_id: Some(parent_run.id.clone()),
                    stage: Some("plan".to_string()),
                    ..Default::default()
                })
                .unwrap();
            db.update_run_status(&sub_run.id, RunStatus::Finished, Some(0), None)
                .unwrap();

            // Add log events to both
            for rid in &[&parent_run.id, &sub_run.id] {
                db.create_event(&NormalizedEvent {
                    run_id: rid.to_string(),
                    ticket_id: ticket.id.clone(),
                    agent_type: "claude".to_string(),
                    event_type: EventType::Custom("log_stdout".to_string()),
                    payload: AgentEventPayload {
                        raw: Some("output".to_string()),
                        structured: None,
                    },
                    timestamp: chrono::Utc::now(),
                })
                .unwrap();
            }

            let count = db.backfill_run_costs(&registry).unwrap();
            // Only the sub-run should be backfilled; parent is skipped
            assert_eq!(count, 1);

            // Verify the sub-run got cost data, parent did not
            assert!(db.get_run_cost(&sub_run.id).unwrap().is_some());
            assert!(db.get_run_cost(&parent_run.id).unwrap().is_none());
        }

        #[test]
        fn backfill_returns_zero_when_nothing_to_do() {
            let db = create_test_db();
            let registry = make_registry();
            let count = db.backfill_run_costs(&registry).unwrap();
            assert_eq!(count, 0);
        }

        /// Provider that keys model_usage by a hardcoded API model name
        /// (simulating Claude Code) and supports model overrides via
        /// `effective_cost_model` / `is_local_override`.
        #[derive(Debug)]
        struct OverrideAwareProvider;

        impl AgentProvider for OverrideAwareProvider {
            fn id(&self) -> &str { "claude" }
            fn display_name(&self) -> &str { "claude" }
            fn build_command(&self, _: &AgentRunConfig) -> (String, Vec<String>) {
                ("claude".to_string(), vec![])
            }
            fn build_env_vars(&self, _: &AgentRunConfig) -> Vec<(String, String)> { vec![] }
            fn extract_text(&self, o: &str) -> String { o.to_string() }
            fn extract_cost(&self, _stdout: &str, _model: &str, _dur: f64) -> Option<RunCostData> {
                let mut usage = std::collections::HashMap::new();
                usage.insert("claude-opus-4-6".to_string(), crate::agents::cost::ModelCostData {
                    input_tokens: 200,
                    output_tokens: 100,
                    cost_usd: 0.03,
                    ..Default::default()
                });
                Some(RunCostData {
                    input_tokens: 200,
                    output_tokens: 100,
                    total_cost_usd: 0.03,
                    model_usage: usage,
                    is_estimated: false,
                    ..Default::default()
                })
            }
            fn is_available(&self) -> bool { true }
            fn get_version(&self) -> Option<String> { None }
            fn config_dir_name(&self) -> &str { ".claude" }
            fn command_instructions_subdir(&self) -> &str { "commands" }
            fn format_command_reference(&self, c: &str) -> String { format!("/{c}") }

            fn is_local_override(&self, agent_config: &std::collections::HashMap<String, serde_json::Value>) -> bool {
                agent_config.get("useLocalProvider")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            }

            fn effective_cost_model(&self, stage: &str, agent_config: &std::collections::HashMap<String, serde_json::Value>) -> String {
                agent_config.get("modelOverride")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| stage.to_string())
            }
        }

        #[test]
        fn backfill_uses_model_override_from_stored_agent_config() {
            let db = create_test_db();
            let mut reg = AgentRegistry::new();
            reg.register(Arc::new(OverrideAwareProvider));

            let (ticket_id, run_id) = create_finished_run_without_cost(&db);

            // Store metadata with agent_config (model override) but no cost
            db.set_run_metadata(&run_id, &serde_json::json!({
                "duration_secs": 10.0,
                "agent_config": {
                    "useLocalProvider": true,
                    "modelOverride": "llama3.2"
                }
            })).unwrap();

            db.create_event(&NormalizedEvent {
                run_id: run_id.clone(),
                ticket_id: ticket_id.clone(),
                agent_type: "claude".to_string(),
                event_type: EventType::Custom("log_stdout".to_string()),
                payload: AgentEventPayload {
                    raw: Some("some output".to_string()),
                    structured: None,
                },
                timestamp: chrono::Utc::now(),
            }).unwrap();

            let count = db.backfill_run_costs(&reg).unwrap();
            assert_eq!(count, 1);

            let cost = db.get_run_cost(&run_id).unwrap().unwrap();

            // Tokens should be re-keyed to the override model name
            assert!(
                cost.model_usage.contains_key("llama3.2"),
                "should track under override model; got keys: {:?}",
                cost.model_usage.keys().collect::<Vec<_>>()
            );
            assert!(
                !cost.model_usage.contains_key("opus-4.6")
                    && !cost.model_usage.contains_key("claude-opus-4-6"),
                "API/stage model key should be gone"
            );

            // Local override should zero out costs
            assert_eq!(cost.total_cost_usd, 0.0, "local override should zero cost");
            assert_eq!(cost.model_usage["llama3.2"].cost_usd, 0.0);
            assert_eq!(cost.model_usage["llama3.2"].input_tokens, 200);
        }

        #[test]
        fn backfill_without_agent_config_uses_ticket_model() {
            let db = create_test_db();
            let mut reg = AgentRegistry::new();
            reg.register(Arc::new(OverrideAwareProvider));

            let (ticket_id, run_id) = create_finished_run_without_cost(&db);

            db.create_event(&NormalizedEvent {
                run_id: run_id.clone(),
                ticket_id: ticket_id.clone(),
                agent_type: "claude".to_string(),
                event_type: EventType::Custom("log_stdout".to_string()),
                payload: AgentEventPayload {
                    raw: Some("some output".to_string()),
                    structured: None,
                },
                timestamp: chrono::Utc::now(),
            }).unwrap();

            // No metadata with agent_config — backfill falls back to direct extract_cost
            let count = db.backfill_run_costs(&reg).unwrap();
            assert_eq!(count, 1);

            let cost = db.get_run_cost(&run_id).unwrap().unwrap();
            // Without agent_config, the API model key from extract_cost is used as-is
            assert!(
                cost.model_usage.contains_key("claude-opus-4-6"),
                "without override, API model key should be preserved"
            );
            assert_eq!(cost.total_cost_usd, 0.03, "no local override → cost preserved");
        }
    }
}
