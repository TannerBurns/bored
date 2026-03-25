//! Tests for ticket state transitions (pause, resume, etc.).

use super::{create_test_db, setup_board_with_ready_ticket, temp_dir_path};
use crate::db::models::{CreateProject, CreateSpec, CreateTicket, Priority, WorkflowType};
use crate::db::DbError;
use chrono::{Duration, Utc};

#[test]
fn pause_ticket_sets_pause_fields() {
    let db = create_test_db();
    let (_, _, ticket) = setup_board_with_ready_ticket(&db);

    db.pause_ticket(&ticket.id, "implement", "run-123").unwrap();

    let paused = db.get_ticket(&ticket.id).unwrap();
    assert!(paused.paused_at.is_some());
    assert_eq!(paused.paused_at_stage, Some("implement".to_string()));
    assert_eq!(paused.paused_run_id, Some("run-123".to_string()));
}

#[test]
fn pause_ticket_not_found() {
    let db = create_test_db();
    let result = db.pause_ticket("nonexistent", "stage", "run");
    assert!(matches!(result, Err(DbError::NotFound(_))));
}

#[test]
fn resume_ticket_clears_pause_and_returns_stage() {
    let db = create_test_db();
    let (_, _, ticket) = setup_board_with_ready_ticket(&db);

    db.pause_ticket(&ticket.id, "review", "run-456").unwrap();

    let stage = db.resume_ticket(&ticket.id).unwrap();
    assert_eq!(stage, Some("review".to_string()));

    let resumed = db.get_ticket(&ticket.id).unwrap();
    // paused_at is cleared so workers can pick up the ticket
    assert!(resumed.paused_at.is_none());
    // paused_at_stage is preserved so worker knows which stage to resume from
    assert_eq!(resumed.paused_at_stage, Some("review".to_string()));
    // paused_run_id is preserved so we can load stage outputs from the previous run
    assert_eq!(resumed.paused_run_id, Some("run-456".to_string()));
}

#[test]
fn resume_ticket_returns_none_if_not_paused() {
    let db = create_test_db();
    let (_, _, ticket) = setup_board_with_ready_ticket(&db);

    let stage = db.resume_ticket(&ticket.id).unwrap();
    assert!(stage.is_none());
}

#[test]
fn resume_ticket_not_found() {
    let db = create_test_db();
    let result = db.resume_ticket("nonexistent");
    assert!(matches!(result, Err(DbError::NotFound(_))));
}

#[test]
fn clear_ticket_pause_success() {
    let db = create_test_db();
    let (_, _, ticket) = setup_board_with_ready_ticket(&db);

    db.pause_ticket(&ticket.id, "deslop", "run-789").unwrap();

    db.clear_ticket_pause(&ticket.id).unwrap();

    let cleared = db.get_ticket(&ticket.id).unwrap();
    assert!(cleared.paused_at.is_none());
    assert!(cleared.paused_at_stage.is_none());
    assert!(cleared.paused_run_id.is_none());
}

#[test]
fn clear_ticket_pause_not_found() {
    let db = create_test_db();
    let result = db.clear_ticket_pause("nonexistent");
    assert!(matches!(result, Err(DbError::NotFound(_))));
}

#[test]
fn is_ticket_paused_true_when_paused() {
    let db = create_test_db();
    let (_, _, ticket) = setup_board_with_ready_ticket(&db);

    assert!(!db.is_ticket_paused(&ticket.id).unwrap());

    db.pause_ticket(&ticket.id, "branch", "run").unwrap();

    assert!(db.is_ticket_paused(&ticket.id).unwrap());
}

#[test]
fn is_ticket_paused_not_found() {
    let db = create_test_db();
    let result = db.is_ticket_paused("nonexistent");
    assert!(matches!(result, Err(DbError::NotFound(_))));
}

#[test]
fn get_paused_tickets_returns_only_paused() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let project = db
        .create_project(&CreateProject {
            name: "Test".to_string(),
            path: temp_dir_path(),
            requires_git: false,
        })
        .unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let ready = columns.iter().find(|c| c.name == "Ready").unwrap();

    // Create a spec first (this also creates version 1)
    let spec = db
        .create_spec(&CreateSpec {
            board_id: board.id.clone(),
            target_board_id: Some(board.id.clone()),
            project_id: project.id.clone(),
            name: "Test".to_string(),
            user_input: "Test".to_string(),
            model: None,
            settings: serde_json::json!({}),
        })
        .unwrap();

    // Get the version ID (create_spec creates version 1)
    let version = db.get_latest_spec_version(&spec.id).unwrap().unwrap();

    // Create tickets with spec_version_id
    let t1 = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: ready.id.clone(),
            title: "T1".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
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
            spec_version_id: Some(version.id.clone()),
        })
        .unwrap();

    let t2 = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: ready.id.clone(),
            title: "T2".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
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
            spec_version_id: Some(version.id.clone()),
        })
        .unwrap();

    // Pause only t1
    db.pause_ticket(&t1.id, "impl", "run-1").unwrap();

    let paused = db.get_paused_tickets(&version.id).unwrap();
    assert_eq!(paused.len(), 1);
    assert_eq!(paused[0].id, t1.id);

    // Pause t2 as well
    db.pause_ticket(&t2.id, "review", "run-2").unwrap();

    let paused2 = db.get_paused_tickets(&version.id).unwrap();
    assert_eq!(paused2.len(), 2);
}

#[test]
fn reserve_next_ticket_skips_paused_tickets() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let ready = columns.iter().find(|c| c.name == "Ready").unwrap();

    // Create two tickets in Ready
    let t1 = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: ready.id.clone(),
            title: "Ticket 1".to_string(),
            description_md: "".to_string(),
            priority: Priority::High,
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
        .unwrap();

    let t2 = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: ready.id.clone(),
            title: "Ticket 2".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
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
        .unwrap();

    // Pause t1 (the higher priority one)
    db.pause_ticket(&t1.id, "stage", "run").unwrap();

    let expires = Utc::now() + Duration::minutes(30);
    let reserved = db
        .reserve_next_ticket(None, "cursor", "new-run", expires)
        .unwrap();

    // Should skip paused t1 and reserve t2
    assert!(reserved.is_some());
    assert_eq!(reserved.unwrap().id, t2.id);
}

#[test]
fn reserve_next_ticket_returns_none_when_all_paused() {
    let db = create_test_db();
    let (_, _, ticket) = setup_board_with_ready_ticket(&db);

    // Pause the only ticket
    db.pause_ticket(&ticket.id, "stage", "run").unwrap();

    let expires = Utc::now() + Duration::minutes(30);
    let reserved = db
        .reserve_next_ticket(None, "cursor", "run-1", expires)
        .unwrap();

    assert!(reserved.is_none());
}
