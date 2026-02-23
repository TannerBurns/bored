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

pub(crate) fn time_filter_clause(days: Option<i32>, column: &str) -> String {
    match days {
        Some(d) => format!(
            "AND {} >= datetime('now', '-{} days')",
            column, d
        ),
        None => String::new(),
    }
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
                                CASE WHEN r.ended_at IS NOT NULL AND r.started_at IS NOT NULL
                                THEN (julianday(r.ended_at) - julianday(r.started_at)) * 86400
                                END
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
                        total_cost_usd += cost.total_cost_usd;
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

    /// Get time-series trend data bucketed by day.
    pub fn get_dashboard_trends(
        &self,
        days: i32,
    ) -> Result<Vec<DashboardTrendPoint>, DbError> {
        self.with_conn(|conn| {
            let mut date_map: HashMap<String, DashboardTrendPoint> = HashMap::new();

            for i in 0..=days {
                let date: String = conn
                    .query_row(
                        "SELECT date('now', ? || ' days')",
                        [format!("-{}", i)],
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

            // Tickets completed per day
            {
                let mut stmt = conn.prepare(&format!(
                    r#"SELECT date(t.updated_at) as d, COUNT(*)
                       FROM tickets t
                       JOIN columns c ON t.column_id = c.id
                       WHERE c.name = 'Done'
                       AND t.updated_at >= datetime('now', '-{} days')
                       GROUP BY d"#,
                    days
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

            // Tasks completed per day
            {
                let mut stmt = conn.prepare(&format!(
                    r#"SELECT date(completed_at) as d, COUNT(*)
                       FROM tasks
                       WHERE status = 'completed'
                       AND completed_at >= datetime('now', '-{} days')
                       GROUP BY d"#,
                    days
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

            // Runs per day
            {
                let mut stmt = conn.prepare(&format!(
                    r#"SELECT date(r.started_at) as d, COUNT(*)
                       FROM agent_runs r
                       WHERE r.started_at >= datetime('now', '-{} days')
                       {}
                       GROUP BY d"#,
                    days, EXCLUDE_PARENT_RUNS_FILTER
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

            // Cost and tokens per day
            {
                let mut stmt = conn.prepare(&format!(
                    r#"SELECT date(r.started_at) as d, r.metadata_json
                       FROM agent_runs r
                       WHERE r.metadata_json IS NOT NULL
                       AND r.started_at >= datetime('now', '-{} days')
                       {}"#,
                    days, EXCLUDE_PARENT_RUNS_FILTER
                ))?;
                let rows = stmt.query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?;
                for row in rows.flatten() {
                    if let Some(cost) = parse_cost(&row.1) {
                        if let Some(point) = date_map.get_mut(&row.0) {
                            point.cost_usd += cost.total_cost_usd;
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

            let mut stmt = conn.prepare(&format!(
                r#"SELECT r.metadata_json FROM agent_runs r
                   WHERE r.metadata_json IS NOT NULL
                   {}
                   {}"#,
                EXCLUDE_PARENT_RUNS_FILTER, time_filter
            ))?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;

            for row in rows.flatten() {
                if let Some(cost) = parse_cost(&row) {
                    if cost.model_usage.is_empty() {
                        // No per-model breakdown; aggregate under "unknown"
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
                            entry.run_count += 1;
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
                        CASE WHEN r.ended_at IS NOT NULL AND r.started_at IS NOT NULL
                        THEN (julianday(r.ended_at) - julianday(r.started_at)) * 86400
                        END
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
