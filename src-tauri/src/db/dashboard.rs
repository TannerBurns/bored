use crate::agents::cost::RunCostData;
use crate::db::{Database, DbError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSummary {
    pub tickets_completed: i64,
    pub tasks_completed: i64,
    pub total_runs: i64,
    pub successful_runs: i64,
    pub success_rate: f64,
    pub avg_run_duration_secs: f64,
    pub total_cost_usd: f64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub total_commits: i64,
    pub total_prs: i64,
    pub total_lines_added: i64,
    pub total_lines_removed: i64,
    pub avg_cycle_time_hours: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DashboardTrendPoint {
    pub date: String,
    pub tickets_completed: i64,
    pub tasks_completed: i64,
    pub cost_usd: f64,
    pub tokens_used: u64,
    pub runs: i64,
    pub commits: i64,
    pub lines_added: i64,
    pub lines_removed: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelBreakdownEntry {
    pub model: String,
    pub cost_usd: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub run_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AgentBreakdownEntry {
    pub agent_type: String,
    pub run_count: i64,
    pub success_count: i64,
    pub avg_duration_secs: f64,
}

/// SQL fragment that excludes parent runs which have sub-runs, preventing double-counting.
/// Leaf runs (with parent_run_id) are always included; top-level runs are included only
/// when they have no children.
const EXCLUDE_PARENT_RUNS_FILTER: &str = r#"AND (
    r.parent_run_id IS NOT NULL
    OR NOT EXISTS (
        SELECT 1 FROM agent_runs sr WHERE sr.parent_run_id = r.id
    )
)"#;

pub(crate) fn parse_cost(json_str: &str) -> Option<RunCostData> {
    let metadata: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let cost_value = metadata.get("cost")?;
    serde_json::from_value(cost_value.clone()).ok()
}

/// Prefer per-model cost sums when available; fall back to `total_cost_usd`
/// for runs that pre-date per-model tracking.
fn effective_cost_usd(cost: &RunCostData) -> f64 {
    if cost.model_usage.is_empty() {
        cost.total_cost_usd
    } else {
        cost.model_usage.values().map(|d| d.cost_usd).sum()
    }
}

pub(crate) fn time_filter_clause(days: Option<i32>, column: &str) -> String {
    match days {
        Some(d) => format!(
            "AND {} >= datetime('now', '-{} days')",
            column, d
        ),
        None => String::new(),
    }
}

/// SQLite time modifier that shifts UTC timestamps to the user's local time
/// for date bucketing. `offset_minutes` is positive east of UTC (e.g. +60 for
/// CET, -480 for PST) -- the same convention as negated JS `getTimezoneOffset`.
fn local_date_modifier(offset_minutes: i32) -> String {
    format!("{:+} minutes", offset_minutes)
}

impl Database {
    /// Get summary stats for the dashboard, optionally filtered to the last N days.
    pub fn get_dashboard_summary(
        &self,
        days: Option<i32>,
    ) -> Result<DashboardSummary, DbError> {
        self.with_conn(|conn| {
            let time_filter = time_filter_clause(days, "t.updated_at");

            let tickets_completed: i64 = conn
                .query_row(
                    &format!(
                        r#"SELECT COUNT(*) FROM tickets t
                           JOIN columns c ON t.column_id = c.id
                           WHERE c.name = 'Done' {}"#,
                        time_filter
                    ),
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            let task_time_filter = time_filter_clause(days, "completed_at");
            let tasks_completed: i64 = conn
                .query_row(
                    &format!(
                        "SELECT COUNT(*) FROM tasks WHERE status = 'completed' {}",
                        task_time_filter
                    ),
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            let run_time_filter = time_filter_clause(days, "r.started_at");

            let (total_runs, successful_runs, avg_duration): (i64, i64, f64) = conn
                .query_row(
                    &format!(
                        r#"SELECT
                            COUNT(*),
                            SUM(CASE WHEN r.status = 'finished' THEN 1 ELSE 0 END),
                            COALESCE(AVG(
                                COALESCE(
                                    json_extract(r.metadata_json, '$.duration_secs'),
                                    CASE WHEN r.ended_at IS NOT NULL AND r.started_at IS NOT NULL
                                    THEN (julianday(r.ended_at) - julianday(r.started_at)) * 86400
                                    END
                                )
                            ), 0)
                           FROM agent_runs r
                           WHERE r.status IN ('finished', 'error', 'aborted')
                           {}
                           {}"#,
                        EXCLUDE_PARENT_RUNS_FILTER, run_time_filter
                    ),
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap_or((0, 0, 0.0));

            let success_rate = if total_runs > 0 {
                successful_runs as f64 / total_runs as f64
            } else {
                0.0
            };

            let mut total_cost_usd = 0.0f64;
            let mut total_input_tokens = 0u64;
            let mut total_output_tokens = 0u64;
            let mut total_cache_read_tokens = 0u64;

            {
                let mut stmt = conn.prepare(&format!(
                    r#"SELECT r.metadata_json FROM agent_runs r
                       WHERE r.metadata_json IS NOT NULL
                       {}
                       {}"#,
                    EXCLUDE_PARENT_RUNS_FILTER, run_time_filter
                ))?;
                let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
                for row in rows.flatten() {
                    if let Some(cost) = parse_cost(&row) {
                        total_cost_usd += effective_cost_usd(&cost);
                        total_input_tokens += cost.input_tokens;
                        total_output_tokens += cost.output_tokens;
                        total_cache_read_tokens += cost.cache_read_tokens;
                    }
                }
            }

            // Include chat_runs costs
            {
                let chat_time_filter = time_filter_clause(days, "cr.created_at");
                let mut stmt = conn.prepare(&format!(
                    r#"SELECT cr.metadata_json FROM chat_runs cr
                       WHERE cr.metadata_json IS NOT NULL
                       {}"#,
                    chat_time_filter
                ))?;
                let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
                for row in rows.flatten() {
                    if let Some(cost) = parse_cost(&row) {
                        total_cost_usd += effective_cost_usd(&cost);
                        total_input_tokens += cost.input_tokens;
                        total_output_tokens += cost.output_tokens;
                        total_cache_read_tokens += cost.cache_read_tokens;
                    }
                }
            }

            let git_time_filter = time_filter_clause(days, "g.collected_at");
            let (total_commits, total_prs, total_lines_added, total_lines_removed): (
                i64,
                i64,
                i64,
                i64,
            ) = conn
                .query_row(
                    &format!(
                        r#"SELECT
                            COALESCE(SUM(g.commits), 0),
                            COALESCE(SUM(g.prs_created), 0),
                            COALESCE(SUM(g.lines_added), 0),
                            COALESCE(SUM(g.lines_removed), 0)
                           FROM ticket_git_stats g
                           WHERE 1=1 {}"#,
                        git_time_filter
                    ),
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .unwrap_or((0, 0, 0, 0));

            let avg_cycle_time_hours: f64 = conn
                .query_row(
                    &format!(
                        r#"SELECT COALESCE(AVG(
                            (julianday(t.updated_at) - julianday(t.created_at)) * 24
                        ), 0)
                        FROM tickets t
                        JOIN columns c ON t.column_id = c.id
                        WHERE c.name = 'Done'
                        AND t.updated_at > t.created_at
                        {}"#,
                        time_filter
                    ),
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0.0);

            Ok(DashboardSummary {
                tickets_completed,
                tasks_completed,
                total_runs,
                successful_runs,
                success_rate,
                avg_run_duration_secs: avg_duration,
                total_cost_usd,
                total_input_tokens,
                total_output_tokens,
                total_cache_read_tokens,
                total_commits,
                total_prs,
                total_lines_added,
                total_lines_removed,
                avg_cycle_time_hours,
            })
        })
    }

    /// Get time-series trend data bucketed by day in the user's local timezone.
    /// `utc_offset_minutes` shifts UTC timestamps before extracting the date
    /// (positive = east of UTC, e.g. -480 for PST, +60 for CET).
    pub fn get_dashboard_trends(
        &self,
        days: i32,
        utc_offset_minutes: i32,
    ) -> Result<Vec<DashboardTrendPoint>, DbError> {
        self.with_conn(|conn| {
            let tz_mod = local_date_modifier(utc_offset_minutes);
            let mut date_map: HashMap<String, DashboardTrendPoint> = HashMap::new();

            for i in 0..=days {
                let date: String = conn
                    .query_row(
                        "SELECT date('now', ?, ? || ' days')",
                        [&tz_mod, &format!("-{}", i)],
                        |row| row.get(0),
                    )
                    .unwrap_or_default();
                if !date.is_empty() {
                    date_map.insert(
                        date.clone(),
                        DashboardTrendPoint {
                            date,
                            ..Default::default()
                        },
                    );
                }
            }

            // Tickets completed per day (local date)
            {
                let mut stmt = conn.prepare(&format!(
                    r#"SELECT date(t.updated_at, '{tz}') as d, COUNT(*)
                       FROM tickets t
                       JOIN columns c ON t.column_id = c.id
                       WHERE c.name = 'Done'
                       AND t.updated_at >= datetime('now', '-{days} days')
                       GROUP BY d"#,
                    tz = tz_mod,
                    days = days,
                ))?;
                let rows = stmt.query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                })?;
                for row in rows.flatten() {
                    if let Some(point) = date_map.get_mut(&row.0) {
                        point.tickets_completed = row.1;
                    }
                }
            }

            // Tasks completed per day (local date)
            {
                let mut stmt = conn.prepare(&format!(
                    r#"SELECT date(completed_at, '{tz}') as d, COUNT(*)
                       FROM tasks
                       WHERE status = 'completed'
                       AND completed_at >= datetime('now', '-{days} days')
                       GROUP BY d"#,
                    tz = tz_mod,
                    days = days,
                ))?;
                let rows = stmt.query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                })?;
                for row in rows.flatten() {
                    if let Some(point) = date_map.get_mut(&row.0) {
                        point.tasks_completed = row.1;
                    }
                }
            }

            // Runs per day (local date)
            {
                let mut stmt = conn.prepare(&format!(
                    r#"SELECT date(r.started_at, '{tz}') as d, COUNT(*)
                       FROM agent_runs r
                       WHERE r.started_at >= datetime('now', '-{days} days')
                       {exclude}
                       GROUP BY d"#,
                    tz = tz_mod,
                    days = days,
                    exclude = EXCLUDE_PARENT_RUNS_FILTER,
                ))?;
                let rows = stmt.query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                })?;
                for row in rows.flatten() {
                    if let Some(point) = date_map.get_mut(&row.0) {
                        point.runs = row.1;
                    }
                }
            }

            // Cost and tokens per day (agent_runs, local date)
            {
                let mut stmt = conn.prepare(&format!(
                    r#"SELECT date(r.started_at, '{tz}') as d, r.metadata_json
                       FROM agent_runs r
                       WHERE r.metadata_json IS NOT NULL
                       AND r.started_at >= datetime('now', '-{days} days')
                       {exclude}"#,
                    tz = tz_mod,
                    days = days,
                    exclude = EXCLUDE_PARENT_RUNS_FILTER,
                ))?;
                let rows = stmt.query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?;
                for row in rows.flatten() {
                    if let Some(cost) = parse_cost(&row.1) {
                        if let Some(point) = date_map.get_mut(&row.0) {
                            point.cost_usd += effective_cost_usd(&cost);
                            point.tokens_used +=
                                cost.input_tokens + cost.output_tokens + cost.cache_read_tokens;
                        }
                    }
                }
            }

            // Cost and tokens per day (chat_runs, local date)
            {
                let mut stmt = conn.prepare(&format!(
                    r#"SELECT date(cr.created_at, '{tz}') as d, cr.metadata_json
                       FROM chat_runs cr
                       WHERE cr.metadata_json IS NOT NULL
                       AND cr.created_at >= datetime('now', '-{days} days')"#,
                    tz = tz_mod,
                    days = days,
                ))?;
                let rows = stmt.query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?;
                for row in rows.flatten() {
                    if let Some(cost) = parse_cost(&row.1) {
                        if let Some(point) = date_map.get_mut(&row.0) {
                            point.cost_usd += effective_cost_usd(&cost);
                            point.tokens_used +=
                                cost.input_tokens + cost.output_tokens + cost.cache_read_tokens;
                        }
                    }
                }
            }

            let mut points: Vec<DashboardTrendPoint> = date_map.into_values().collect();
            points.sort_by(|a, b| a.date.cmp(&b.date));
            Ok(points)
        })
    }

    /// Get per-model cost and token breakdown.
    pub fn get_model_breakdown(
        &self,
        days: Option<i32>,
    ) -> Result<Vec<ModelBreakdownEntry>, DbError> {
        self.with_conn(|conn| {
            let time_filter = time_filter_clause(days, "r.started_at");
            let mut model_map: HashMap<String, ModelBreakdownEntry> = HashMap::new();

            let agent_sql = format!(
                r#"SELECT r.metadata_json FROM agent_runs r
                   WHERE r.metadata_json IS NOT NULL
                   {}
                   {}"#,
                EXCLUDE_PARENT_RUNS_FILTER, time_filter
            );
            let chat_time_filter = time_filter_clause(days, "cr.created_at");
            let chat_sql = format!(
                r#"SELECT cr.metadata_json FROM chat_runs cr
                   WHERE cr.metadata_json IS NOT NULL
                   {}"#,
                chat_time_filter
            );

            for sql in [&agent_sql, &chat_sql] {
                let mut stmt = conn.prepare(sql)?;
                let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;

                for row in rows.flatten() {
                    if let Some(cost) = parse_cost(&row) {
                        if cost.model_usage.is_empty() {
                            let entry = model_map
                                .entry("unknown".to_string())
                                .or_insert_with(|| ModelBreakdownEntry {
                                    model: "unknown".to_string(),
                                    ..Default::default()
                                });
                            entry.cost_usd += cost.total_cost_usd;
                            entry.input_tokens += cost.input_tokens;
                            entry.output_tokens += cost.output_tokens;
                            entry.run_count += 1;
                        } else {
                            let dominant_model = cost
                                .model_usage
                                .iter()
                                .max_by(|a, b| {
                                    a.1.cost_usd
                                        .partial_cmp(&b.1.cost_usd)
                                        .unwrap_or(std::cmp::Ordering::Equal)
                                })
                                .map(|(name, _)| name.clone());

                            for (model_name, model_data) in &cost.model_usage {
                                let entry = model_map
                                    .entry(model_name.clone())
                                    .or_insert_with(|| ModelBreakdownEntry {
                                        model: model_name.clone(),
                                        ..Default::default()
                                    });
                                entry.cost_usd += model_data.cost_usd;
                                entry.input_tokens += model_data.input_tokens;
                                entry.output_tokens += model_data.output_tokens;
                                if dominant_model.as_ref() == Some(model_name) {
                                    entry.run_count += 1;
                                }
                            }
                        }
                    }
                }
            }

            let mut entries: Vec<ModelBreakdownEntry> = model_map.into_values().collect();
            entries.sort_by(|a, b| {
                b.cost_usd
                    .partial_cmp(&a.cost_usd)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            Ok(entries)
        })
    }

    /// Get per-agent-type run breakdown.
    pub fn get_agent_breakdown(
        &self,
        days: Option<i32>,
    ) -> Result<Vec<AgentBreakdownEntry>, DbError> {
        self.with_conn(|conn| {
            let time_filter = time_filter_clause(days, "r.started_at");
            let mut stmt = conn.prepare(&format!(
                r#"SELECT
                    r.agent_type,
                    COUNT(*) as run_count,
                    SUM(CASE WHEN r.status = 'finished' THEN 1 ELSE 0 END) as success_count,
                    COALESCE(AVG(
                        COALESCE(
                            json_extract(r.metadata_json, '$.duration_secs'),
                            CASE WHEN r.ended_at IS NOT NULL AND r.started_at IS NOT NULL
                            THEN (julianday(r.ended_at) - julianday(r.started_at)) * 86400
                            END
                        )
                    ), 0) as avg_duration
                   FROM agent_runs r
                   WHERE r.status IN ('finished', 'error', 'aborted')
                   {}
                   {}
                   GROUP BY r.agent_type
                   ORDER BY run_count DESC"#,
                EXCLUDE_PARENT_RUNS_FILTER, time_filter
            ))?;
            let rows = stmt.query_map([], |row| {
                Ok(AgentBreakdownEntry {
                    agent_type: row.get(0)?,
                    run_count: row.get(1)?,
                    success_count: row.get(2)?,
                    avg_duration_secs: row.get(3)?,
                })
            })?;
            Ok(rows.flatten().collect())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::{CreateRun, CreateTicket, Priority, RunStatus, WorkflowType};

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
                title: "Dashboard Ticket".to_string(),
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
        db.update_run_status(&run.id, RunStatus::Finished, Some(0), None)
            .unwrap();
        (ticket.id, run.id)
    }

    #[test]
    fn summary_cost_uses_total_when_model_usage_empty() {
        let db = create_test_db();
        let (_, run_id) = create_ticket_and_run(&db);

        db.set_run_metadata(
            &run_id,
            &serde_json::json!({
                "cost": {
                    "inputTokens": 100,
                    "outputTokens": 50,
                    "cacheReadTokens": 0,
                    "cacheCreationTokens": 0,
                    "totalCostUsd": 0.05,
                    "isEstimated": false,
                    "modelUsage": {}
                }
            }),
        )
        .unwrap();

        let summary = db.get_dashboard_summary(None).unwrap();
        assert!(
            (summary.total_cost_usd - 0.05).abs() < 0.001,
            "empty model_usage → should use totalCostUsd; got {}",
            summary.total_cost_usd
        );
        assert_eq!(summary.total_input_tokens, 100);
        assert_eq!(summary.total_output_tokens, 50);
    }

    #[test]
    fn summary_cost_sums_model_usage_when_present() {
        let db = create_test_db();
        let (_, run_id) = create_ticket_and_run(&db);

        db.set_run_metadata(
            &run_id,
            &serde_json::json!({
                "cost": {
                    "inputTokens": 100,
                    "outputTokens": 50,
                    "cacheReadTokens": 0,
                    "cacheCreationTokens": 0,
                    "totalCostUsd": 0.10,
                    "isEstimated": false,
                    "modelUsage": {
                        "opus-4.6": { "inputTokens": 80, "outputTokens": 40, "costUsd": 0.08, "cacheReadTokens": 0, "cacheCreationTokens": 0 },
                        "sonnet-4.5": { "inputTokens": 20, "outputTokens": 10, "costUsd": 0.02, "cacheReadTokens": 0, "cacheCreationTokens": 0 }
                    }
                }
            }),
        )
        .unwrap();

        let summary = db.get_dashboard_summary(None).unwrap();
        assert!(
            (summary.total_cost_usd - 0.10).abs() < 0.001,
            "non-empty model_usage → should sum model costs (0.08+0.02=0.10); got {}",
            summary.total_cost_usd
        );
    }

    #[test]
    fn summary_cost_mixed_runs_with_and_without_model_usage() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let ticket = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
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

        let r1 = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "claude".to_string(),
                repo_path: "/tmp".to_string(),
                ..Default::default()
            })
            .unwrap();
        db.update_run_status(&r1.id, RunStatus::Finished, Some(0), None)
            .unwrap();
        db.set_run_metadata(
            &r1.id,
            &serde_json::json!({
                "cost": {
                    "inputTokens": 100, "outputTokens": 50,
                    "cacheReadTokens": 0, "cacheCreationTokens": 0,
                    "totalCostUsd": 0.03, "isEstimated": false,
                    "modelUsage": {}
                }
            }),
        )
        .unwrap();

        let r2 = db
            .create_run(&CreateRun {
                ticket_id: ticket.id.clone(),
                agent_type: "claude".to_string(),
                repo_path: "/tmp".to_string(),
                ..Default::default()
            })
            .unwrap();
        db.update_run_status(&r2.id, RunStatus::Finished, Some(0), None)
            .unwrap();
        db.set_run_metadata(
            &r2.id,
            &serde_json::json!({
                "cost": {
                    "inputTokens": 200, "outputTokens": 100,
                    "cacheReadTokens": 0, "cacheCreationTokens": 0,
                    "totalCostUsd": 0.07,
                    "isEstimated": false,
                    "modelUsage": {
                        "opus-4.6": { "inputTokens": 200, "outputTokens": 100, "costUsd": 0.07, "cacheReadTokens": 0, "cacheCreationTokens": 0 }
                    }
                }
            }),
        )
        .unwrap();

        let summary = db.get_dashboard_summary(None).unwrap();
        assert!(
            (summary.total_cost_usd - 0.10).abs() < 0.001,
            "mixed: 0.03 (empty model_usage) + 0.07 (from model_usage) = 0.10; got {}",
            summary.total_cost_usd
        );
        assert_eq!(summary.total_input_tokens, 300);
        assert_eq!(summary.total_output_tokens, 150);
    }

    #[test]
    fn summary_empty_db_returns_zero_costs() {
        let db = create_test_db();
        let summary = db.get_dashboard_summary(None).unwrap();
        assert_eq!(summary.total_cost_usd, 0.0);
        assert_eq!(summary.total_input_tokens, 0);
        assert_eq!(summary.total_output_tokens, 0);
        assert_eq!(summary.total_runs, 0);
    }
}
