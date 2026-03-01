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
    fn available_models(&self) -> Vec<(&str, &str)> {
        vec![
            ("claude-opus-4-6", "Claude Opus 4.6"),
            ("claude-sonnet-4-5", "Claude Sonnet 4.5"),
        ]
    }
}

#[derive(Debug)]
struct CodexStubProvider;

impl AgentProvider for CodexStubProvider {
    fn id(&self) -> &str { "codex" }
    fn display_name(&self) -> &str { "Codex" }
    fn build_command(&self, _: &crate::agents::AgentRunConfig) -> (String, Vec<String>) {
        ("echo".into(), vec!["ok".into()])
    }
    fn build_env_vars(&self, _: &crate::agents::AgentRunConfig) -> Vec<(String, String)> { vec![] }
    fn extract_text(&self, o: &str) -> String { o.into() }
    fn extract_cost(&self, _: &str, _: &str, _: f64) -> Option<crate::agents::cost::RunCostData> { None }
    fn is_available(&self) -> bool { true }
    fn get_version(&self) -> Option<String> { Some("1.0".into()) }
    fn config_dir_name(&self) -> &str { ".codex" }
    fn command_instructions_subdir(&self) -> &str { "commands" }
    fn format_command_reference(&self, c: &str) -> String { format!("/{c}") }
    fn available_models(&self) -> Vec<(&str, &str)> {
        vec![
            ("gpt-5.3-codex", "GPT-5.3 Codex"),
            ("gpt-5.2-codex", "GPT-5.2 Codex"),
        ]
    }
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
    let stage = crate::agents::models::DEFAULT_STAGE_MODEL;
    let diag = crate::agents::models::DEFAULT_DIAGNOSTIC_MODEL;
    let default_stages: HashMap<String, StageConfig> = [
        ("branchGen", diag),
        ("plan", stage),
        ("implement", stage),
        ("code-review", stage),
        ("cleanup", diag),
        ("commit", diag),
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
            auto_pilot_model: crate::agents::models::DEFAULT_STAGE_MODEL.to_string(),
            stage_configs: default_stages,
            code_review_max_iterations: 3,
            stage_timeout_hours: 1,
            stage_max_retries: 2,
            diagnostic_model: crate::agents::models::DEFAULT_DIAGNOSTIC_MODEL.to_string(),
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
        cancel_handles: Arc::new(Mutex::new(HashMap::new())),
        worktree_branch: Some("test-branch".to_string()),
        branch_already_created: true,
        is_temp_branch: false,
        target_branch: None,
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
                model: crate::agents::models::DEFAULT_STAGE_MODEL.to_string(),
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

    assert_eq!(orch.get_stage_model("plan"), crate::agents::models::DEFAULT_STAGE_MODEL);
    assert_eq!(orch.get_stage_model("branch-gen"), crate::agents::models::DEFAULT_DIAGNOSTIC_MODEL);
    assert_eq!(orch.get_stage_model("add-and-commit"), crate::agents::models::DEFAULT_DIAGNOSTIC_MODEL);
}

#[test]
fn get_stage_model_maps_code_review_fix_to_code_review() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);
    let orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));

    assert_eq!(orch.get_stage_model("code-review"), crate::agents::models::DEFAULT_STAGE_MODEL);
    assert_eq!(orch.get_stage_model("code-review-fix"), crate::agents::models::DEFAULT_STAGE_MODEL);
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

// -- auto-pilot command selection integration tests --

#[tokio::test]
async fn command_selection_parses_json_array() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(true, true);

    let json = r#"[{"command":"cleanup","model":"sonnet-4.6"},{"command":"code-review","model":"opus-4.6"}]"#;
    let mut orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));
    orch.set_stage_runner(Arc::new(MockStageRunner::always_succeed(json)));

    let selections = orch.run_command_selection_stage("plan text", "impl text").await.unwrap();
    assert_eq!(selections.len(), 2);
    assert_eq!(selections[0].command, "cleanup");
    assert_eq!(selections[1].command, "code-review");
}

#[tokio::test]
async fn command_selection_parses_code_fenced_json() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(true, true);

    let json = "Here are the commands:\n\n```json\n[{\"command\":\"deslop\",\"model\":\"sonnet-4.5\"}]\n```\n";
    let mut orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));
    orch.set_stage_runner(Arc::new(MockStageRunner::always_succeed(json)));

    let selections = orch.run_command_selection_stage("", "").await.unwrap();
    assert_eq!(selections.len(), 1);
    assert_eq!(selections[0].command, "deslop");
}

#[tokio::test]
async fn command_selection_handles_prose_with_brackets() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(true, true);

    let json = "Based on [the analysis] of the changes:\n[{\"command\":\"unit-tests\",\"model\":\"opus-4.5\"}]";
    let mut orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));
    orch.set_stage_runner(Arc::new(MockStageRunner::always_succeed(json)));

    let selections = orch.run_command_selection_stage("", "").await.unwrap();
    assert_eq!(selections.len(), 1);
    assert_eq!(selections[0].command, "unit-tests");
}

#[tokio::test]
async fn command_selection_handles_result_text_appended() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(true, true);

    let json = r#"[{"command":"cleanup","model":"sonnet-4.6"}]I selected cleanup for QA."#;
    let mut orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));
    orch.set_stage_runner(Arc::new(MockStageRunner::always_succeed(json)));

    let selections = orch.run_command_selection_stage("", "").await.unwrap();
    assert_eq!(selections.len(), 1);
    assert_eq!(selections[0].command, "cleanup");
}

#[tokio::test]
async fn command_selection_returns_empty_on_stage_failure() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(true, true);

    let mut orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));
    orch.set_stage_runner(Arc::new(MockStageRunner::new(10, "")));

    let selections = orch.run_command_selection_stage("", "").await.unwrap();
    assert!(selections.is_empty());
}

#[tokio::test]
async fn command_selection_filters_excluded_commands() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(true, true);

    let json = r#"[{"command":"add-and-commit","model":"sonnet-4.6"},{"command":"cleanup","model":"sonnet-4.6"}]"#;
    let mut orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));
    orch.set_stage_runner(Arc::new(MockStageRunner::always_succeed(json)));

    let selections = orch.run_command_selection_stage("", "").await.unwrap();
    assert_eq!(selections.len(), 1);
    assert_eq!(selections[0].command, "cleanup");
}

#[tokio::test]
async fn command_selection_empty_stdout_returns_empty() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(true, true);

    let mut orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));
    orch.set_stage_runner(Arc::new(MockStageRunner::always_succeed("")));

    let selections = orch.run_command_selection_stage("", "").await.unwrap();
    assert!(selections.is_empty());
}

#[tokio::test]
async fn command_selection_parses_empty_array() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(true, true);

    let mut orch = WorkflowOrchestrator::new(make_config(db, ticket, run_id, settings));
    orch.set_stage_runner(Arc::new(MockStageRunner::always_succeed("[]")));

    let selections = orch.run_command_selection_stage("", "").await.unwrap();
    assert!(selections.is_empty());
}

#[tokio::test]
async fn command_selection_with_stream_json_format() {
    use crate::agents::claude::provider::extract_text_from_stream_json;

    let stream = concat!(
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"[{\"command\":\"code-review\",\"model\":\"opus-4.6\"}]"}}}"#,
        "\n",
        r#"{"type":"result","result":"Selected code-review.","subtype":"success"}"#,
    );
    let text = extract_text_from_stream_json(stream).unwrap();
    let selections = super::auto_pilot::parse_command_selection_response(
        &text,
        &["code-review".to_string(), "cleanup".to_string()],
    );
    assert_eq!(selections.len(), 1);
    assert_eq!(selections[0].command, "code-review");
    assert_eq!(selections[0].model, "opus-4.6");
}

#[tokio::test]
async fn command_selection_with_stream_json_multiple_deltas() {
    use crate::agents::claude::provider::extract_text_from_stream_json;

    let stream = concat!(
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"["}}}"#,
        "\n",
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"{\"command\":\"cleanup\",\"model\":\"sonnet-4.6\"}"}}}"#,
        "\n",
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"]"}}}"#,
        "\n",
        r#"{"type":"result","result":"Done.","subtype":"success"}"#,
    );
    let text = extract_text_from_stream_json(stream).unwrap();
    assert_eq!(text, r#"[{"command":"cleanup","model":"sonnet-4.6"}]"#);

    let selections = super::auto_pilot::parse_command_selection_response(
        &text,
        &["cleanup".to_string()],
    );
    assert_eq!(selections.len(), 1);
    assert_eq!(selections[0].command, "cleanup");
}

// -- Prompt content integration tests --
// These verify that run_command_selection_stage builds a prompt that is
// truly dynamic: commands come from the bundled catalog, and models come
// from the provider.

/// A mock runner that captures the prompt sent to the stage.
struct PromptCapturingRunner {
    captured_prompt: std::sync::Mutex<Option<String>>,
}

impl PromptCapturingRunner {
    fn new() -> Self {
        Self {
            captured_prompt: std::sync::Mutex::new(None),
        }
    }

    fn prompt(&self) -> String {
        self.captured_prompt.lock().unwrap().clone().unwrap()
    }
}

impl super::StageRunner for PromptCapturingRunner {
    fn run(
        &self,
        _provider: &dyn AgentProvider,
        config: &crate::agents::AgentRunConfig,
        _on_log: Option<Arc<crate::agents::LogCallback>>,
        _on_spawn: Option<crate::agents::spawner::OnSpawnCallback>,
    ) -> Result<crate::agents::AgentRunResult, crate::agents::spawner::SpawnError> {
        *self.captured_prompt.lock().unwrap() = Some(config.prompt.clone());
        Ok(crate::agents::AgentRunResult {
            run_id: config.run_id.clone(),
            exit_code: Some(0),
            status: crate::agents::RunOutcome::Success,
            summary: None,
            duration_secs: 0.1,
            captured_stdout: Some("[]".to_string()),
        })
    }
}

fn make_config_with_provider(
    db: Arc<Database>,
    ticket: Ticket,
    parent_run_id: String,
    settings: Arc<Mutex<PerAgentSettings>>,
    provider: Arc<dyn AgentProvider>,
    agent_id: &str,
) -> OrchestratorConfig {
    OrchestratorConfig {
        db,
        window: None,
        app_handle: None,
        parent_run_id,
        ticket,
        task: None,
        repo_path: PathBuf::from("/tmp/test"),
        agent_id: agent_id.to_string(),
        provider,
        cancel_handles: Arc::new(Mutex::new(HashMap::new())),
        worktree_branch: Some("test-branch".to_string()),
        branch_already_created: true,
        is_temp_branch: false,
        target_branch: None,
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

fn make_workflow_settings_for_agent(agent_id: &str) -> Arc<Mutex<PerAgentSettings>> {
    let mut map = HashMap::new();
    let stage = crate::agents::models::DEFAULT_STAGE_MODEL;
    let diag = crate::agents::models::DEFAULT_DIAGNOSTIC_MODEL;
    let default_stages: HashMap<String, StageConfig> = [
        ("branchGen", diag),
        ("plan", stage),
        ("implement", stage),
        ("commit", diag),
    ]
    .into_iter()
    .map(|(k, m)| (k.to_string(), StageConfig { enabled: true, model: m.to_string() }))
    .collect();

    map.insert(
        agent_id.to_string(),
        WorkflowSettings {
            auto_pilot_enabled: true,
            auto_pilot_model: crate::agents::models::DEFAULT_STAGE_MODEL.to_string(),
            stage_configs: default_stages,
            code_review_max_iterations: 3,
            stage_timeout_hours: 1,
            stage_max_retries: 0,
            diagnostic_model: crate::agents::models::DEFAULT_DIAGNOSTIC_MODEL.to_string(),
            stage_order: Some(DEFAULT_STAGE_ORDER.iter().map(|s| s.to_string()).collect()),
            synced: true,
        },
    );
    Arc::new(Mutex::new(map))
}

#[tokio::test]
async fn command_selection_prompt_contains_provider_models_claude() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings_for_agent("stub");

    let runner = Arc::new(PromptCapturingRunner::new());
    let mut orch = WorkflowOrchestrator::new(make_config_with_provider(
        db, ticket, run_id, settings, Arc::new(StubProvider), "stub",
    ));
    orch.set_stage_runner(runner.clone());

    let _ = orch.run_command_selection_stage("the plan", "the impl").await;

    let prompt = runner.prompt();

    // Provider models (StubProvider returns claude-opus-4-6 and claude-sonnet-4-5)
    assert!(
        prompt.contains("`claude-opus-4-6` (Claude Opus 4.6)"),
        "Prompt should list the provider's first model with label"
    );
    assert!(
        prompt.contains("`claude-sonnet-4-5` (Claude Sonnet 4.5)"),
        "Prompt should list the provider's second model with label"
    );

    // Examples should use the provider's models
    assert!(
        prompt.contains(r#""model": "claude-opus-4-6""#),
        "Examples should use the capable model from the provider"
    );
    assert!(
        prompt.contains(r#""model": "claude-sonnet-4-5""#),
        "Examples should use the efficient model from the provider"
    );

    // Bundled commands should appear
    assert!(prompt.contains("- `cleanup`"), "Prompt should list bundled commands");
    assert!(prompt.contains("- `code-review`"), "Prompt should list bundled commands");
    assert!(prompt.contains("- `deslop`"), "Prompt should list bundled commands");

    // Excluded commands must NOT appear
    assert!(!prompt.contains("- `add-and-commit`"), "Excluded commands must not appear");

    // Ticket context
    assert!(prompt.contains("Test Ticket"), "Prompt should contain ticket title");
    assert!(prompt.contains("Do the thing"), "Prompt should contain ticket description");

    // Plan and impl summary
    assert!(prompt.contains("the plan"), "Prompt should contain plan text");
    assert!(prompt.contains("the impl"), "Prompt should contain implementation summary");

    // Model constraint instruction
    assert!(
        prompt.contains("ONLY use model names from the Available Models list"),
        "Prompt should tell the agent to only use listed models"
    );
}

#[tokio::test]
async fn command_selection_prompt_uses_codex_models_not_claude() {
    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings_for_agent("codex");

    let runner = Arc::new(PromptCapturingRunner::new());
    let mut orch = WorkflowOrchestrator::new(make_config_with_provider(
        db, ticket, run_id, settings, Arc::new(CodexStubProvider), "codex",
    ));
    orch.set_stage_runner(runner.clone());

    let _ = orch.run_command_selection_stage("", "").await;

    let prompt = runner.prompt();

    // Should contain Codex models
    assert!(
        prompt.contains("`gpt-5.3-codex` (GPT-5.3 Codex)"),
        "Codex prompt should list gpt-5.3-codex"
    );
    assert!(
        prompt.contains("`gpt-5.2-codex` (GPT-5.2 Codex)"),
        "Codex prompt should list gpt-5.2-codex"
    );

    // Examples should use Codex models
    assert!(
        prompt.contains(r#""model": "gpt-5.3-codex""#),
        "Codex examples should use gpt-5.3-codex as capable model"
    );
    assert!(
        prompt.contains(r#""model": "gpt-5.2-codex""#),
        "Codex examples should use gpt-5.2-codex as efficient model"
    );

    // Should NOT contain Claude model names anywhere
    assert!(
        !prompt.contains("opus"),
        "Codex prompt must not mention Claude 'opus' models"
    );
    assert!(
        !prompt.contains("sonnet"),
        "Codex prompt must not mention Claude 'sonnet' models"
    );

    // Bundled commands should still be listed
    assert!(prompt.contains("- `cleanup`"), "Commands should still be present");
    assert!(prompt.contains("- `unit-tests`"), "Commands should still be present");
}

// -- Real CLI output round-trip tests --
// These use actual captured output from each CLI to verify the full
// extract_text -> parse_command_selection_response pipeline produces
// usable CommandSelection results with valid model names.

fn available_commands() -> Vec<String> {
    vec![
        "cleanup", "code-review", "deslop", "unit-tests", "review-changes",
        "add-tests", "fix-lint",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

#[test]
fn real_claude_cli_output_round_trip() {
    use crate::agents::claude::provider::extract_text_from_stream_json;

    let raw = concat!(
        r#"{"type":"system","subtype":"init","cwd":"/tmp","session_id":"s1","model":"claude-sonnet-4-6"}"#, "\n",
        r#"{"type":"assistant","message":{"model":"claude-sonnet-4-6","id":"msg_1","type":"message","role":"assistant","content":[{"type":"text","text":"[{\"command\": \"review-changes\", \"model\": \"sonnet-4.5\"}, {\"command\": \"unit-tests\", \"model\": \"sonnet-4.5\"}, {\"command\": \"deslop\", \"model\": \"sonnet-4.5\"}]"}],"stop_reason":null},"session_id":"s1"}"#, "\n",
        r#"{"type":"result","subtype":"success","is_error":false,"result":"[{\"command\": \"review-changes\", \"model\": \"sonnet-4.5\"}, {\"command\": \"unit-tests\", \"model\": \"sonnet-4.5\"}, {\"command\": \"deslop\", \"model\": \"sonnet-4.5\"}]","session_id":"s1","total_cost_usd":0.013}"#,
    );

    let text = extract_text_from_stream_json(raw).unwrap();
    let cmds = available_commands();
    let selections = super::auto_pilot::parse_command_selection_response(&text, &cmds);

    assert!(!selections.is_empty(), "Claude: should parse selections from real output");
    for s in &selections {
        assert!(cmds.contains(&s.command), "Claude: command '{}' must be in available list", s.command);
    }
}

#[test]
fn real_codex_cli_output_round_trip() {
    use crate::agents::codex::provider::CodexProvider;

    let raw = concat!(
        r#"{"type":"thread.started","thread_id":"t1"}"#, "\n",
        r#"{"type":"turn.started"}"#, "\n",
        r#"{"type":"item.completed","item":{"id":"item_0","type":"reasoning","text":"thinking..."}}"#, "\n",
        r#"{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"[{\"command\":\"review-changes\",\"model\":\"gpt-5.2-codex\"},{\"command\":\"code-review\",\"model\":\"gpt-5.3-codex\"},{\"command\":\"unit-tests\",\"model\":\"gpt-5.2-codex\"},{\"command\":\"add-tests\",\"model\":\"gpt-5.3-codex\"},{\"command\":\"fix-lint\",\"model\":\"gpt-5.2-codex\"}]"}}"#, "\n",
        r#"{"type":"turn.completed","usage":{"input_tokens":7792,"cached_input_tokens":6528,"output_tokens":338}}"#,
    );

    let provider = CodexProvider::new();
    let text = provider.extract_text(raw);
    let cmds = available_commands();
    let selections = super::auto_pilot::parse_command_selection_response(&text, &cmds);

    assert!(!selections.is_empty(), "Codex: should parse selections from real output");
    for s in &selections {
        assert!(cmds.contains(&s.command), "Codex: command '{}' must be in available list", s.command);
        assert!(
            ["gpt-5.3-codex", "gpt-5.2-codex"].contains(&s.model.as_str()),
            "Codex: model '{}' must be a valid Codex model", s.model
        );
    }
}

#[test]
fn real_cursor_cli_output_round_trip() {
    use crate::agents::claude::provider::extract_text_from_stream_json;

    let raw = concat!(
        r#"{"type":"system","subtype":"init","apiKeySource":"login","cwd":"/tmp","session_id":"s2","model":"Claude 4.6 Sonnet"}"#, "\n",
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"[{\"command\": \"code-review\", \"model\": \"sonnet-4.6\"}, {\"command\": \"fix-lint\", \"model\": \"sonnet-4.5\"}, {\"command\": \"add-tests\", \"model\": \"sonnet-4.6\"}]"}]},"session_id":"s2"}"#, "\n",
        r#"{"type":"result","subtype":"success","duration_ms":2277,"is_error":false,"result":"[{\"command\": \"code-review\", \"model\": \"sonnet-4.6\"}, {\"command\": \"fix-lint\", \"model\": \"sonnet-4.5\"}, {\"command\": \"add-tests\", \"model\": \"sonnet-4.6\"}]","session_id":"s2"}"#,
    );

    let text = extract_text_from_stream_json(raw).unwrap();
    let cmds = available_commands();
    let selections = super::auto_pilot::parse_command_selection_response(&text, &cmds);

    assert!(!selections.is_empty(), "Cursor: should parse selections from real output");
    for s in &selections {
        assert!(cmds.contains(&s.command), "Cursor: command '{}' must be in available list", s.command);
    }
}

// -- Todo-based implementation resume tests --

#[tokio::test]
async fn resume_resets_in_progress_todo_to_pending_before_reexecution() {
    use super::config::{ImplementationTodo, TodoItemStatus, TodoStatus};

    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);

    let mut orch = WorkflowOrchestrator::new(make_config(db.clone(), ticket, run_id.clone(), settings));
    orch.set_stage_runner(Arc::new(MockStageRunner::always_succeed("impl output")));

    // Simulate a previous run that was interrupted mid-todo:
    // todo 0 = Completed, todo 1 = InProgress, todo 2 = Pending
    let todo_statuses = vec![
        TodoStatus {
            title: "Step 1".to_string(),
            description: "First step".to_string(),
            status: TodoItemStatus::Completed,
        },
        TodoStatus {
            title: "Step 2".to_string(),
            description: "Second step".to_string(),
            status: TodoItemStatus::InProgress,
        },
        TodoStatus {
            title: "Step 3".to_string(),
            description: "Third step".to_string(),
            status: TodoItemStatus::Pending,
        },
    ];
    db.merge_run_metadata(
        &run_id,
        &serde_json::json!({ "implementation_todos": todo_statuses }),
    )
    .unwrap();

    // Load the todos into the orchestrator's in-memory storage
    {
        let mut stored = orch.implementation_todos.write().unwrap();
        *stored = vec![
            ImplementationTodo { title: "Step 1".to_string(), description: "First step".to_string() },
            ImplementationTodo { title: "Step 2".to_string(), description: "Second step".to_string() },
            ImplementationTodo { title: "Step 3".to_string(), description: "Third step".to_string() },
        ];
    }

    let result = orch.run_implement_stage_capturing("test plan").await;
    assert!(result.is_ok());

    // Verify: the InProgress todo (index 1) was first reset to Pending,
    // then executed and marked Completed. Check the final DB state.
    let run = db.get_run(&run_id).unwrap();
    let meta = run.metadata.unwrap();
    let saved: Vec<TodoStatus> =
        serde_json::from_value(meta["implementation_todos"].clone()).unwrap();

    assert_eq!(saved[0].status, TodoItemStatus::Completed, "todo 0 should remain Completed");
    assert_eq!(saved[1].status, TodoItemStatus::Completed, "todo 1 (was InProgress) should now be Completed");
    assert_eq!(saved[2].status, TodoItemStatus::Completed, "todo 2 should be Completed");
}

#[tokio::test]
async fn resume_retries_failed_todo_instead_of_skipping() {
    use super::config::{ImplementationTodo, TodoItemStatus, TodoStatus};

    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);

    let mut orch = WorkflowOrchestrator::new(make_config(db.clone(), ticket, run_id.clone(), settings));
    orch.set_stage_runner(Arc::new(MockStageRunner::always_succeed("impl output")));

    // Simulate a previous run where todo 1 failed:
    // todo 0 = Completed, todo 1 = Failed, todo 2 = Pending
    let todo_statuses = vec![
        TodoStatus {
            title: "Step 1".to_string(),
            description: "First step".to_string(),
            status: TodoItemStatus::Completed,
        },
        TodoStatus {
            title: "Step 2".to_string(),
            description: "Second step".to_string(),
            status: TodoItemStatus::Failed,
        },
        TodoStatus {
            title: "Step 3".to_string(),
            description: "Third step".to_string(),
            status: TodoItemStatus::Pending,
        },
    ];
    db.merge_run_metadata(
        &run_id,
        &serde_json::json!({ "implementation_todos": todo_statuses }),
    )
    .unwrap();

    {
        let mut stored = orch.implementation_todos.write().unwrap();
        *stored = vec![
            ImplementationTodo { title: "Step 1".to_string(), description: "First step".to_string() },
            ImplementationTodo { title: "Step 2".to_string(), description: "Second step".to_string() },
            ImplementationTodo { title: "Step 3".to_string(), description: "Third step".to_string() },
        ];
    }

    let result = orch.run_implement_stage_capturing("test plan").await;
    assert!(result.is_ok(), "implement stage should succeed on retry");

    let run = db.get_run(&run_id).unwrap();
    let meta = run.metadata.unwrap();
    let saved: Vec<TodoStatus> =
        serde_json::from_value(meta["implementation_todos"].clone()).unwrap();

    assert_eq!(saved[0].status, TodoItemStatus::Completed, "todo 0 should remain Completed");
    assert_eq!(saved[1].status, TodoItemStatus::Completed, "todo 1 (was Failed) should now be Completed after retry");
    assert_eq!(saved[2].status, TodoItemStatus::Completed, "todo 2 should be Completed");
}

#[tokio::test]
async fn resume_combined_output_includes_previously_completed_todo_output() {
    use super::config::{ImplementationTodo, TodoItemStatus, TodoStatus};

    let db = create_test_db();
    let ticket = seed_ticket(&db);
    let run_id = seed_parent_run(&db, &ticket.id);
    let settings = make_workflow_settings(false, true);

    let mut orch = WorkflowOrchestrator::new(make_config(db.clone(), ticket, run_id.clone(), settings));
    orch.set_stage_runner(Arc::new(MockStageRunner::always_succeed("new todo output")));

    // Simulate resuming with todo 0 already completed
    let todo_statuses = vec![
        TodoStatus {
            title: "Step 1".to_string(),
            description: "First step".to_string(),
            status: TodoItemStatus::Completed,
        },
        TodoStatus {
            title: "Step 2".to_string(),
            description: "Second step".to_string(),
            status: TodoItemStatus::Pending,
        },
    ];
    db.merge_run_metadata(
        &run_id,
        &serde_json::json!({ "implementation_todos": todo_statuses }),
    )
    .unwrap();

    {
        let mut stored = orch.implementation_todos.write().unwrap();
        *stored = vec![
            ImplementationTodo { title: "Step 1".to_string(), description: "First step".to_string() },
            ImplementationTodo { title: "Step 2".to_string(), description: "Second step".to_string() },
        ];
    }

    // Seed previous_stage_outputs with the output from the already-completed todo
    orch.previous_stage_outputs
        .insert("implement".to_string(), "previous step 1 output".to_string());

    let result = orch.run_implement_stage_capturing("test plan").await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(
        output.contains("previous step 1 output"),
        "combined output should include output from previously completed todos, got: {}",
        output,
    );
    assert!(
        output.contains("new todo output"),
        "combined output should include output from newly executed todos, got: {}",
        output,
    );
}
