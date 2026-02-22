//! Integration tests for WorkflowOrchestrator construction and helper logic.
//!
//! These tests exercise the orchestrator's mode derivation, resume logic,
//! stage skip/enable checks, and model resolution using a real in-memory
//! database and `None` Tauri window/app_handle (events silently no-op).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use super::config::{WorkflowMode, DEFAULT_STAGE_ORDER};
use super::{CancelHandlesMap, OrchestratorConfig, WorkflowOrchestrator};
use crate::agents::provider::AgentProvider;
use crate::commands::runs::StageConfig;
use crate::commands::workflow_settings::{PerAgentSettings, WorkflowSettings};
use crate::db::models::{CreateTicket, Priority, WorkflowType};
use crate::db::{CreateRun, Database, RunStatus, Ticket};

#[derive(Debug)]
struct StubProvider;

impl AgentProvider for StubProvider {
    fn id(&self) -> &str { "stub" }
    fn display_name(&self) -> &str { "Stub" }
    fn build_command(&self, _: &crate::agents::AgentRunConfig) -> (String, Vec<String>) {
        ("echo".into(), vec!["ok".into()])
    }
    fn build_env_vars(&self, _: &crate::agents::AgentRunConfig) -> Vec<(String, String)> { vec![] }
    fn extract_text(&self, o: &str) -> String { o.into() }
    fn extract_cost(&self, _: &str, _: &str, _: f64) -> Option<crate::agents::cost::RunCostData> { None }
    fn is_available(&self) -> bool { true }
    fn get_version(&self) -> Option<String> { Some("1.0".into()) }
    fn config_dir_name(&self) -> &str { ".stub" }
    fn command_instructions_subdir(&self) -> &str { "commands" }
    fn format_command_reference(&self, c: &str) -> String { format!("/{c}") }
}

fn create_test_db() -> Arc<Database> {
    Arc::new(Database::open_in_memory().unwrap())
}

fn seed_ticket(db: &Database) -> Ticket {
    let board = db.create_board("Test Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    db.create_ticket(&CreateTicket {
        board_id: board.id,
        column_id: columns[0].id.clone(),
        title: "Test Ticket".to_string(),
        description_md: "Do the thing".to_string(),
        priority: Priority::Medium,
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
}

fn seed_parent_run(db: &Database, ticket_id: &str) -> String {
    let run = db
        .create_run(&CreateRun {
            ticket_id: ticket_id.to_string(),
            agent_type: "stub".to_string(),
            repo_path: "/tmp/test".to_string(),
            parent_run_id: None,
            stage: None,
            resumed_from_run_id: None,
        })
        .unwrap();
    run.id
}

fn make_workflow_settings(auto_pilot: bool, synced: bool) -> Arc<Mutex<PerAgentSettings>> {
    let mut map = HashMap::new();
    let default_stages: HashMap<String, StageConfig> = [
        ("branchGen", "sonnet-4.6"),
        ("plan", "opus-4.6"),
        ("implement", "opus-4.6"),
        ("code-review", "opus-4.6"),
        ("cleanup", "sonnet-4.6"),
        ("commit", "sonnet-4.6"),
    ]
    .into_iter()
    .map(|(k, m)| {
        (
            k.to_string(),
            StageConfig {
                enabled: true,
                model: m.to_string(),
            },
        )
    })
    .collect();

    map.insert(
        "stub".to_string(),
        WorkflowSettings {
            auto_pilot_enabled: auto_pilot,
            stage_configs: default_stages,
            code_review_max_iterations: 3,
            stage_timeout_hours: 1,
            stage_max_retries: 2,
            diagnostic_model: "sonnet-4.6".to_string(),
            stage_order: Some(
                DEFAULT_STAGE_ORDER.iter().map(|s| s.to_string()).collect(),
            ),
            synced,
        },
    );
    Arc::new(Mutex::new(map))
}

fn make_config(
    db: Arc<Database>,
    ticket: Ticket,
    parent_run_id: String,
    settings: Arc<Mutex<PerAgentSettings>>,
) -> OrchestratorConfig {
    OrchestratorConfig {
        db,
        window: None,
        app_handle: None,
        parent_run_id,
        ticket,
        task: None,
        repo_path: PathBuf::from("/tmp/test"),
        agent_id: "stub".to_string(),
        provider: Arc::new(StubProvider),
        api_url: "https://api.test".to_string(),
        api_token: "test-token".to_string(),
        cancel_handles: Arc::new(Mutex::new(HashMap::new())),
        worktree_branch: Some("test-branch".to_string()),
        branch_already_created: true,
        is_temp_branch: false,
        agent_config: HashMap::new(),
        resume_from_stage: None,
        previous_run_id: None,
        workflow_settings: settings,
        stage_configs: HashMap::new(),
        code_review_max_iterations: 3,
        stage_timeout_secs: 3600,
        stage_max_retries: 2,
    }
}

// -- WorkflowMode derivation tests --

#[test]
fn new_multi_stage_mode_when_auto_pilot_disabled() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);
    let orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));
    assert_eq!(orch.workflow_mode, WorkflowMode::MultiStage);
}

#[test]
fn new_auto_pilot_mode_when_enabled() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(true, true);
    let orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));
    assert_eq!(orch.workflow_mode, WorkflowMode::AutoPilot);
}

#[test]
fn new_falls_back_to_config_when_not_synced() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(true, false);
    let orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));
    // Not synced -> uses OrchestratorConfig fallback (auto_pilot not set there)
    assert_eq!(orch.workflow_mode, WorkflowMode::MultiStage);
}

#[test]
fn new_stores_workflow_mode_in_run_metadata() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(true, true);
    let _orch = WorkflowOrchestrator::new(make_config(db.clone(), ticket, run_id.clone(), settings));

    let run = db.get_run(&run_id).unwrap();
    let meta = run.metadata.unwrap_or(serde_json::json!({}));
    assert_eq!(meta["workflow_mode"], "auto_pilot");
}

#[test]
fn new_stores_multi_stage_mode_in_metadata() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);
    let _orch = WorkflowOrchestrator::new(make_config(db.clone(), ticket, run_id.clone(), settings));

    let run = db.get_run(&run_id).unwrap();
    let meta = run.metadata.unwrap_or(serde_json::json!({}));
    assert_eq!(meta["workflow_mode"], "multi_stage");
}

// -- Stage order tests --

#[test]
fn new_builds_execution_order_from_synced_settings() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);
    let orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));

    assert_eq!(orch.full_execution_order[0], "branch-gen");
    assert_eq!(orch.full_execution_order[1], "branch");
    assert_eq!(orch.full_execution_order[2], "plan");
    assert!(orch.full_execution_order.contains(&"add-and-commit".to_string()));
}

#[test]
fn new_uses_default_stage_order_when_not_synced() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, false);
    let orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));

    // Default stage order starts with branchGen -> branch-gen, branch
    assert_eq!(orch.stage_order[0], "branchGen");
    assert_eq!(orch.full_execution_order[0], "branch-gen");
}

// -- should_skip_stage tests --

#[test]
fn should_skip_stage_false_when_no_resume() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);
    let orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));

    assert!(!orch.should_skip_stage("plan"));
    assert!(!orch.should_skip_stage("implement"));
}

#[test]
fn should_skip_stage_skips_before_resume_point() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);

    let mut config = make_config(db, ticket, run_id, settings);
    config.resume_from_stage = Some("implement".to_string());
    let orch = WorkflowOrchestrator::new(config);

    assert!(orch.should_skip_stage("branch-gen"));
    assert!(orch.should_skip_stage("branch"));
    assert!(orch.should_skip_stage("plan"));
    assert!(orch.should_skip_stage("plan-validation"));
    assert!(!orch.should_skip_stage("implement"));
    assert!(!orch.should_skip_stage("cleanup"));
}

#[test]
fn should_skip_stage_unknown_resume_stage_skips_core() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);

    let mut config = make_config(db, ticket, run_id, settings);
    config.resume_from_stage = Some("nonexistent-stage".to_string());
    let orch = WorkflowOrchestrator::new(config);

    // Unknown resume stage -> core stages (up to implement) are skipped
    assert!(orch.should_skip_stage("branch-gen"));
    assert!(orch.should_skip_stage("plan"));
    assert!(orch.should_skip_stage("implement"));
    // Post-implement stages run
    assert!(!orch.should_skip_stage("cleanup"));
}

// -- is_stage_enabled tests --

#[test]
fn is_stage_enabled_defaults_to_true() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);
    let orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));

    assert!(orch.is_stage_enabled("plan"));
    assert!(orch.is_stage_enabled("implement"));
    assert!(orch.is_stage_enabled("add-and-commit"));
}

#[test]
fn is_stage_enabled_returns_false_for_disabled() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);

    let settings = make_workflow_settings(false, true);
    {
        let mut map = settings.lock().unwrap();
        let ws = map.get_mut("stub").unwrap();
        ws.stage_configs.insert(
            "code-review".to_string(),
            StageConfig {
                enabled: false,
                model: "opus-4.6".to_string(),
            },
        );
    }

    let orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));
    assert!(!orch.is_stage_enabled("code-review"));
    assert!(!orch.is_stage_enabled("code-review-fix"));
}

#[test]
fn is_stage_enabled_unknown_stage_defaults_true() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);
    let orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));

    assert!(orch.is_stage_enabled("unknown-custom-stage"));
}

// -- get_stage_model tests --

#[test]
fn get_stage_model_returns_configured_model() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);
    let orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));

    assert_eq!(orch.get_stage_model("plan"), "opus-4.6");
    assert_eq!(orch.get_stage_model("branch-gen"), "sonnet-4.6");
    assert_eq!(orch.get_stage_model("add-and-commit"), "sonnet-4.6");
}

#[test]
fn get_stage_model_maps_code_review_fix_to_code_review() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);
    let orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));

    assert_eq!(orch.get_stage_model("code-review"), "opus-4.6");
    assert_eq!(orch.get_stage_model("code-review-fix"), "opus-4.6");
}

#[test]
fn get_stage_model_returns_default_for_unknown() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);
    let orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));

    let model = orch.get_stage_model("nonexistent-stage");
    assert_eq!(model, crate::agents::models::DEFAULT_STAGE_MODEL);
}

// -- cancellation tests --

#[test]
fn is_cancelled_false_initially() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);
    let orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));

    assert!(!orch.is_cancelled());
}

#[test]
fn is_cancelled_true_after_flag_set() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);
    let orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));

    orch.cancelled.store(true, Ordering::Relaxed);
    assert!(orch.is_cancelled());
}

#[test]
fn is_cancelled_true_when_cancel_handle_cancelled() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);
    let cancel_handles: CancelHandlesMap = Arc::new(Mutex::new(HashMap::new()));

    let mut config = make_config(db, ticket, run_id.clone(), settings);
    config.cancel_handles = cancel_handles.clone();
    let orch = WorkflowOrchestrator::new(config);

    let handle = crate::agents::spawner::CancelHandle::new(Arc::new(std::sync::atomic::AtomicBool::new(false)));
    {
        let mut handles = cancel_handles.lock().unwrap();
        handles.insert(run_id, handle.clone());
    }
    handle.cancel();

    assert!(orch.is_cancelled());
}

// -- resume with previous stage outputs --

#[test]
fn new_loads_stage_outputs_from_previous_run() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let prev_run_id = seed_parent_run(&db, &ticket.id);

    let sub_run = db
        .create_run(&CreateRun {
            ticket_id: ticket.id.clone(),
            agent_type: "stub".to_string(),
            repo_path: "/tmp/test".to_string(),
            parent_run_id: Some(prev_run_id.clone()),
            stage: Some("plan".to_string()),
            resumed_from_run_id: None,
        })
        .unwrap();
    db.update_run_status(&sub_run.id, RunStatus::Finished, Some(0), None)
        .unwrap();
    db.set_run_metadata(
        &sub_run.id,
        &serde_json::json!({ "stage_output": "The plan output" }),
    )
    .unwrap();

    let new_run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);

    let mut config = make_config(db, ticket, new_run_id, settings);
    config.resume_from_stage = Some("implement".to_string());
    config.previous_run_id = Some(prev_run_id);
    let orch = WorkflowOrchestrator::new(config);

    assert!(
        !orch.previous_stage_outputs.is_empty(),
        "Should have loaded stage outputs from previous run"
    );
}

#[test]
fn new_empty_outputs_when_not_resuming() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);
    let orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));

    assert!(orch.previous_stage_outputs.is_empty());
}

// -- emit_event no-ops without window --

#[test]
fn emit_event_succeeds_without_window() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);
    let orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));

    let result = orch.emit_event("test-event", &serde_json::json!({"ok": true}));
    assert!(result.is_ok());
}

#[test]
fn emit_stage_event_succeeds_without_window() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);
    let orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));

    orch.emit_stage_event("plan", "running", None, None);
    orch.emit_stage_event("plan", "finished", Some("sub-1".into()), Some(5.0));
}

// -- stage timeout and retry config --

#[test]
fn new_reads_timeout_from_synced_settings() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);

    let settings = make_workflow_settings(false, true);
    {
        let mut map = settings.lock().unwrap();
        let ws = map.get_mut("stub").unwrap();
        ws.stage_timeout_hours = 4;
    }
    let orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));

    assert_eq!(orch.stage_timeout_secs, 4 * 3600);
}

#[test]
fn new_reads_max_retries_from_synced_settings() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);

    let settings = make_workflow_settings(false, true);
    {
        let mut map = settings.lock().unwrap();
        let ws = map.get_mut("stub").unwrap();
        ws.stage_max_retries = 5;
    }
    let orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));

    assert_eq!(orch.stage_max_retries, 5);
}

#[test]
fn new_reads_code_review_iterations_from_synced_settings() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);

    let settings = make_workflow_settings(false, true);
    {
        let mut map = settings.lock().unwrap();
        let ws = map.get_mut("stub").unwrap();
        ws.code_review_max_iterations = 7;
    }
    let orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));

    assert_eq!(orch.code_review_max_iterations, 7);
}

// -- legacy stage name normalization on resume --

#[test]
fn resume_from_legacy_stage_normalizes() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);

    let mut config = make_config(db, ticket, run_id, settings);
    config.resume_from_stage = Some("cleanup-post-tests".to_string());
    let orch = WorkflowOrchestrator::new(config);

    assert_eq!(
        orch.resume_from_stage.as_deref(),
        Some("unit-tests"),
    );
}

#[test]
fn resume_from_current_stage_name_unchanged() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);

    let mut config = make_config(db, ticket, run_id, settings);
    config.resume_from_stage = Some("cleanup".to_string());
    let orch = WorkflowOrchestrator::new(config);

    assert_eq!(orch.resume_from_stage.as_deref(), Some("cleanup"));
}

// -- StageRunner trait tests --

use std::sync::atomic::AtomicU32;

/// A mock stage runner that returns configurable results.
struct MockStageRunner {
    /// How many times `run` has been called (shared so the test can inspect it).
    call_count: Arc<AtomicU32>,
    /// Number of failures to produce before a success.
    failures_before_success: u32,
    /// The stdout to return on success.
    success_stdout: String,
}

impl MockStageRunner {
    fn new(failures: u32, stdout: &str) -> Self {
        Self {
            call_count: Arc::new(AtomicU32::new(0)),
            failures_before_success: failures,
            success_stdout: stdout.to_string(),
        }
    }

    fn always_succeed(stdout: &str) -> Self {
        Self::new(0, stdout)
    }
}

impl super::StageRunner for MockStageRunner {
    fn run(
        &self,
        _provider: &dyn AgentProvider,
        config: &crate::agents::AgentRunConfig,
        _on_log: Option<Arc<crate::agents::LogCallback>>,
        _on_spawn: Option<crate::agents::spawner::OnSpawnCallback>,
    ) -> Result<crate::agents::AgentRunResult, crate::agents::spawner::SpawnError> {
        let n = self.call_count.fetch_add(1, Ordering::Relaxed);
        if n < self.failures_before_success {
            Ok(crate::agents::AgentRunResult {
                run_id: config.run_id.clone(),
                exit_code: Some(1),
                status: crate::agents::RunOutcome::Error,
                summary: Some("stage failed".to_string()),
                duration_secs: 0.1,
                captured_stdout: None,
            })
        } else {
            Ok(crate::agents::AgentRunResult {
                run_id: config.run_id.clone(),
                exit_code: Some(0),
                status: crate::agents::RunOutcome::Success,
                summary: None,
                duration_secs: 1.0,
                captured_stdout: Some(self.success_stdout.clone()),
            })
        }
    }
}

#[tokio::test]
async fn run_stage_succeeds_with_mock_runner() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);

    let mut orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));
    orch.set_stage_runner(Arc::new(MockStageRunner::always_succeed("plan output")));

    let result = orch.run_stage("plan", "Generate a plan").await;
    assert!(result.is_ok());
    let run_result = result.unwrap();
    assert_eq!(run_result.status, crate::agents::RunOutcome::Success);
    assert_eq!(run_result.captured_stdout.as_deref(), Some("plan output"));
}

#[tokio::test]
async fn run_stage_retries_on_failure() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);

    let mut config = make_config(db.clone(), ticket, run_id, settings);
    config.stage_max_retries = 2; // allow 2 retries (3 total attempts)
    let mut orch = WorkflowOrchestrator::new(config);

    let runner = MockStageRunner::new(2, "success after retries");
    let call_count = runner.call_count.clone();
    orch.set_stage_runner(Arc::new(runner));

    orch.stage_max_retries = 2;

    let result = orch.run_stage("plan", "Generate a plan").await;
    assert!(result.is_ok(), "Should succeed after retries");

    let count = call_count.load(Ordering::Relaxed);
    assert_eq!(count, 3, "Should have been called 3 times (2 failures + 1 success)");
}

#[tokio::test]
async fn run_stage_exhausts_retries_and_fails() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);

    let mut orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));
    orch.stage_max_retries = 1; // 1 retry = 2 total attempts
    orch.set_stage_runner(Arc::new(MockStageRunner::new(10, "never reached")));

    let result = orch.run_stage("plan", "Generate a plan").await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("after 2 attempts"), "Error: {}", err);
}

#[tokio::test]
async fn run_stage_aborts_when_cancelled() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);

    let mut orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));
    orch.set_stage_runner(Arc::new(MockStageRunner::always_succeed("ok")));

    orch.cancelled.store(true, Ordering::Relaxed);
    let result = orch.run_stage("plan", "Generate a plan").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("cancelled"));
}

#[tokio::test]
async fn run_stage_with_model_uses_override() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);

    let mut orch = WorkflowOrchestrator::new(make_config(db.clone(), ticket, run_id, settings));
    orch.set_stage_runner(Arc::new(MockStageRunner::always_succeed("ok")));

    let result = orch
        .run_stage_with_model("plan", "test prompt", "custom-model-1")
        .await;
    assert!(result.is_ok());

    let runs = db.get_runs(&orch.ticket.id).unwrap();
    let sub_run = runs.iter().find(|r| r.stage.as_deref() == Some("plan")).unwrap();
    let meta = sub_run.metadata.as_ref().unwrap();
    assert!(meta.is_object());
}

// -- extract_text delegates to provider --

#[test]
fn extract_text_delegates_to_provider() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);
    let orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));

    assert_eq!(orch.extract_text("hello world"), "hello world");
}
