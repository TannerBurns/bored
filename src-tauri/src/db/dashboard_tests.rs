use super::dashboard::*;
use crate::db::models::{CreateRun, CreateTicket, Priority, RunStatus, WorkflowType};
use crate::db::Database;

fn create_test_db() -> Database {
    Database::open_in_memory().unwrap()
}

fn make_cost_metadata(
    input: u64,
    output: u64,
    cache_read: u64,
    total_usd: f64,
    model_usage: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "cost": {
            "inputTokens": input,
            "outputTokens": output,
            "cacheReadTokens": cache_read,
            "cacheCreationTokens": 0,
            "totalCostUsd": total_usd,
            "isEstimated": false,
            "modelUsage": model_usage,
        }
    })
}

fn create_ticket(db: &Database, board_id: &str, column_id: &str) -> crate::db::models::Ticket {
    db.create_ticket(&CreateTicket {
        board_id: board_id.to_string(),
        column_id: column_id.to_string(),
        title: "Ticket".to_string(),
        description_md: "".to_string(),
        priority: Priority::Low,
        labels: vec![],
        project_id: None,
        workspace_id: None,
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
}

fn create_finished_run(
    db: &Database,
    ticket_id: &str,
    agent_type: &str,
    parent_run_id: Option<String>,
) -> crate::db::models::AgentRun {
    let run = db
        .create_run(&CreateRun {
            ticket_id: ticket_id.to_string(),
            agent_type: agent_type.to_string(),
            repo_path: "/tmp".to_string(),
            parent_run_id,
            stage: None,
            ..Default::default()
        })
        .unwrap();
    db.update_run_status(&run.id, RunStatus::Finished, Some(0), None)
        .unwrap();
    run
}

// ── parse_cost ─────────────────────────────────────────────

#[test]
fn parse_cost_valid_json() {
    let json = r#"{"cost":{"inputTokens":100,"outputTokens":50,"cacheReadTokens":10,"cacheCreationTokens":5,"totalCostUsd":0.01,"isEstimated":false,"modelUsage":{}}}"#;
    let cost = parse_cost(json).unwrap();
    assert_eq!(cost.input_tokens, 100);
    assert_eq!(cost.output_tokens, 50);
    assert_eq!(cost.cache_read_tokens, 10);
    assert!((cost.total_cost_usd - 0.01).abs() < f64::EPSILON);
}

#[test]
fn parse_cost_invalid_json_returns_none() {
    assert!(parse_cost("not json").is_none());
}

#[test]
fn parse_cost_missing_cost_key_returns_none() {
    assert!(parse_cost(r#"{"other":"data"}"#).is_none());
}

#[test]
fn parse_cost_malformed_cost_value_returns_none() {
    assert!(parse_cost(r#"{"cost":"not an object"}"#).is_none());
}

// ── time_filter_clause ─────────────────────────────────────

#[test]
fn time_filter_clause_with_days() {
    let clause = time_filter_clause(Some(30), "t.updated_at");
    assert!(clause.contains("t.updated_at"));
    assert!(clause.contains("30 days"));
}

#[test]
fn time_filter_clause_without_days() {
    let clause = time_filter_clause(None, "t.updated_at");
    assert!(clause.is_empty());
}

// ── get_dashboard_summary ──────────────────────────────────

#[test]
fn summary_empty_db() {
    let db = create_test_db();
    let summary = db.get_dashboard_summary(None).unwrap();
    assert_eq!(summary.tickets_completed, 0);
    assert_eq!(summary.tasks_completed, 0);
    assert_eq!(summary.total_runs, 0);
    assert_eq!(summary.successful_runs, 0);
    assert_eq!(summary.success_rate, 0.0);
    assert_eq!(summary.total_cost_usd, 0.0);
    assert_eq!(summary.total_commits, 0);
}

#[test]
fn summary_counts_completed_tickets() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let done_col = columns.iter().find(|c| c.name == "Done").unwrap();
    let backlog_col = columns.iter().find(|c| c.name == "Backlog").unwrap();

    create_ticket(&db, &board.id, &done_col.id);
    create_ticket(&db, &board.id, &done_col.id);
    create_ticket(&db, &board.id, &backlog_col.id);

    let summary = db.get_dashboard_summary(None).unwrap();
    assert_eq!(summary.tickets_completed, 2);
}

#[test]
fn summary_counts_runs_and_success_rate() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let ticket = create_ticket(&db, &board.id, &columns[0].id);

    create_finished_run(&db, &ticket.id, "cursor", None);
    create_finished_run(&db, &ticket.id, "cursor", None);

    let error_run = db
        .create_run(&CreateRun {
            ticket_id: ticket.id.clone(),
            agent_type: "cursor".to_string(),
            repo_path: "/tmp".to_string(),
            parent_run_id: None,
            stage: None,
            ..Default::default()
        })
        .unwrap();
    db.update_run_status(&error_run.id, RunStatus::Error, Some(1), None)
        .unwrap();

    let summary = db.get_dashboard_summary(None).unwrap();
    assert_eq!(summary.total_runs, 3);
    assert_eq!(summary.successful_runs, 2);
    assert!((summary.success_rate - 2.0 / 3.0).abs() < 0.01);
}

#[test]
fn summary_excludes_parent_runs_with_sub_runs() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let ticket = create_ticket(&db, &board.id, &columns[0].id);

    let parent = create_finished_run(&db, &ticket.id, "cursor", None);
    create_finished_run(&db, &ticket.id, "cursor", Some(parent.id));

    let summary = db.get_dashboard_summary(None).unwrap();
    assert_eq!(summary.total_runs, 1);
}

#[test]
fn summary_aggregates_cost_from_metadata() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let ticket = create_ticket(&db, &board.id, &columns[0].id);

    let run = create_finished_run(&db, &ticket.id, "cursor", None);
    db.set_run_metadata(
        &run.id,
        &make_cost_metadata(100, 50, 10, 0.05, serde_json::json!({})),
    )
    .unwrap();

    let summary = db.get_dashboard_summary(None).unwrap();
    assert!((summary.total_cost_usd - 0.05).abs() < f64::EPSILON);
    assert_eq!(summary.total_input_tokens, 100);
    assert_eq!(summary.total_output_tokens, 50);
    assert_eq!(summary.total_cache_read_tokens, 10);
}

#[test]
fn summary_includes_git_stats() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let ticket = create_ticket(&db, &board.id, &columns[0].id);

    let stats = crate::db::git_stats::TicketGitStats {
        id: String::new(),
        ticket_id: ticket.id.clone(),
        commits: 5,
        prs_created: 2,
        lines_added: 100,
        lines_removed: 30,
        files_changed: 8,
        collected_at: chrono::Utc::now().to_rfc3339(),
    };
    db.upsert_git_stats(&ticket.id, &stats).unwrap();

    let summary = db.get_dashboard_summary(None).unwrap();
    assert_eq!(summary.total_commits, 5);
    assert_eq!(summary.total_prs, 2);
    assert_eq!(summary.total_lines_added, 100);
    assert_eq!(summary.total_lines_removed, 30);
}

// ── get_dashboard_trends ───────────────────────────────────

#[test]
fn trends_returns_sorted_date_points() {
    let db = create_test_db();
    let trends = db.get_dashboard_trends(7, 0).unwrap();
    assert_eq!(trends.len(), 8);
    for w in trends.windows(2) {
        assert!(w[0].date <= w[1].date, "dates should be sorted ascending");
    }
}

#[test]
fn trends_populates_run_counts() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let ticket = create_ticket(&db, &board.id, &columns[0].id);

    create_finished_run(&db, &ticket.id, "cursor", None);

    let trends = db.get_dashboard_trends(7, 0).unwrap();
    let today = trends.last().unwrap();
    assert!(today.runs >= 1);
}

// ── get_model_breakdown ────────────────────────────────────

#[test]
fn model_breakdown_groups_by_model() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let ticket = create_ticket(&db, &board.id, &columns[0].id);

    let run = create_finished_run(&db, &ticket.id, "cursor", None);
    db.set_run_metadata(
        &run.id,
        &make_cost_metadata(
            200,
            100,
            0,
            0.10,
            serde_json::json!({
                "claude-sonnet": {
                    "inputTokens": 200,
                    "outputTokens": 100,
                    "cacheReadTokens": 0,
                    "cacheCreationTokens": 0,
                    "costUsd": 0.10,
                }
            }),
        ),
    )
    .unwrap();

    let breakdown = db.get_model_breakdown(None).unwrap();
    assert_eq!(breakdown.len(), 1);
    assert_eq!(breakdown[0].model, "claude-sonnet");
    assert_eq!(breakdown[0].input_tokens, 200);
    assert_eq!(breakdown[0].output_tokens, 100);
}

#[test]
fn model_breakdown_falls_back_to_unknown() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let ticket = create_ticket(&db, &board.id, &columns[0].id);

    let run = create_finished_run(&db, &ticket.id, "cursor", None);
    db.set_run_metadata(
        &run.id,
        &make_cost_metadata(100, 50, 0, 0.05, serde_json::json!({})),
    )
    .unwrap();

    let breakdown = db.get_model_breakdown(None).unwrap();
    assert_eq!(breakdown.len(), 1);
    assert_eq!(breakdown[0].model, "unknown");
}

#[test]
fn model_breakdown_sorted_by_cost_desc() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let ticket = create_ticket(&db, &board.id, &columns[0].id);

    for (model, cost) in [("cheap-model", 0.01), ("expensive-model", 0.50)] {
        let run = create_finished_run(&db, &ticket.id, "cursor", None);
        db.set_run_metadata(
            &run.id,
            &make_cost_metadata(
                100,
                50,
                0,
                cost,
                serde_json::json!({
                    (model): {
                        "inputTokens": 100,
                        "outputTokens": 50,
                        "cacheReadTokens": 0,
                        "cacheCreationTokens": 0,
                        "costUsd": cost,
                    }
                }),
            ),
        )
        .unwrap();
    }

    let breakdown = db.get_model_breakdown(None).unwrap();
    assert!(breakdown.len() >= 2);
    assert!(
        breakdown[0].cost_usd >= breakdown[1].cost_usd,
        "should be sorted by cost descending"
    );
}

// ── get_agent_breakdown ────────────────────────────────────

#[test]
fn agent_breakdown_groups_by_agent_type() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let ticket = create_ticket(&db, &board.id, &columns[0].id);

    create_finished_run(&db, &ticket.id, "cursor", None);
    create_finished_run(&db, &ticket.id, "cursor", None);
    create_finished_run(&db, &ticket.id, "claude", None);

    let breakdown = db.get_agent_breakdown(None).unwrap();
    assert_eq!(breakdown.len(), 2);

    let cursor = breakdown.iter().find(|e| e.agent_type == "cursor").unwrap();
    assert_eq!(cursor.run_count, 2);
    assert_eq!(cursor.success_count, 2);

    let claude = breakdown.iter().find(|e| e.agent_type == "claude").unwrap();
    assert_eq!(claude.run_count, 1);
}

#[test]
fn agent_breakdown_tracks_success_vs_error() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let ticket = create_ticket(&db, &board.id, &columns[0].id);

    create_finished_run(&db, &ticket.id, "cursor", None);

    let error_run = db
        .create_run(&CreateRun {
            ticket_id: ticket.id.clone(),
            agent_type: "cursor".to_string(),
            repo_path: "/tmp".to_string(),
            parent_run_id: None,
            stage: None,
            ..Default::default()
        })
        .unwrap();
    db.update_run_status(&error_run.id, RunStatus::Error, Some(1), None)
        .unwrap();

    let breakdown = db.get_agent_breakdown(None).unwrap();
    let cursor = breakdown.iter().find(|e| e.agent_type == "cursor").unwrap();
    assert_eq!(cursor.run_count, 2);
    assert_eq!(cursor.success_count, 1);
}

// ── avg_run_duration from metadata ────────────────────────

#[test]
fn summary_avg_duration_prefers_metadata_duration_secs() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let ticket = create_ticket(&db, &board.id, &columns[0].id);

    let run = create_finished_run(&db, &ticket.id, "cursor", None);
    db.set_run_metadata(
        &run.id,
        &serde_json::json!({ "duration_secs": 42.5 }),
    )
    .unwrap();

    let summary = db.get_dashboard_summary(None).unwrap();
    assert!(
        (summary.avg_run_duration_secs - 42.5).abs() < 0.01,
        "should use duration_secs from metadata; got {}",
        summary.avg_run_duration_secs
    );
}

#[test]
fn summary_avg_duration_falls_back_to_timestamps_without_metadata() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let ticket = create_ticket(&db, &board.id, &columns[0].id);

    create_finished_run(&db, &ticket.id, "cursor", None);

    let summary = db.get_dashboard_summary(None).unwrap();
    assert!(
        summary.avg_run_duration_secs >= 0.0,
        "should fall back to timestamp-based duration; got {}",
        summary.avg_run_duration_secs
    );
}

#[test]
fn summary_avg_duration_mixed_metadata_and_timestamps() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let ticket = create_ticket(&db, &board.id, &columns[0].id);

    let run_with_meta = create_finished_run(&db, &ticket.id, "cursor", None);
    db.set_run_metadata(
        &run_with_meta.id,
        &serde_json::json!({ "duration_secs": 100.0 }),
    )
    .unwrap();

    create_finished_run(&db, &ticket.id, "cursor", None);

    let summary = db.get_dashboard_summary(None).unwrap();
    assert!(
        summary.avg_run_duration_secs > 10.0,
        "mixed avg should reflect the metadata run; got {}",
        summary.avg_run_duration_secs
    );
}

#[test]
fn agent_breakdown_avg_duration_prefers_metadata_duration_secs() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let ticket = create_ticket(&db, &board.id, &columns[0].id);

    let run = create_finished_run(&db, &ticket.id, "claude", None);
    db.set_run_metadata(
        &run.id,
        &serde_json::json!({ "duration_secs": 60.0 }),
    )
    .unwrap();

    let breakdown = db.get_agent_breakdown(None).unwrap();
    let claude = breakdown.iter().find(|e| e.agent_type == "claude").unwrap();
    assert!(
        (claude.avg_duration_secs - 60.0).abs() < 0.01,
        "agent breakdown should use metadata duration; got {}",
        claude.avg_duration_secs
    );
}

#[test]
fn agent_breakdown_avg_duration_falls_back_without_metadata() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let ticket = create_ticket(&db, &board.id, &columns[0].id);

    create_finished_run(&db, &ticket.id, "claude", None);

    let breakdown = db.get_agent_breakdown(None).unwrap();
    let claude = breakdown.iter().find(|e| e.agent_type == "claude").unwrap();
    assert!(
        claude.avg_duration_secs >= 0.0,
        "should fall back to timestamp duration; got {}",
        claude.avg_duration_secs
    );
}

#[test]
fn summary_avg_duration_with_cost_and_duration_metadata() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let ticket = create_ticket(&db, &board.id, &columns[0].id);

    let run = create_finished_run(&db, &ticket.id, "cursor", None);
    let mut metadata = make_cost_metadata(100, 50, 0, 0.05, serde_json::json!({}));
    metadata["duration_secs"] = serde_json::json!(85.3);
    db.set_run_metadata(&run.id, &metadata).unwrap();

    let summary = db.get_dashboard_summary(None).unwrap();
    assert!(
        (summary.avg_run_duration_secs - 85.3).abs() < 0.01,
        "should use duration_secs even when cost metadata is present; got {}",
        summary.avg_run_duration_secs
    );
    assert!(
        (summary.total_cost_usd - 0.05).abs() < 0.001,
        "cost should still be captured correctly; got {}",
        summary.total_cost_usd
    );
}
