use super::{create_test_db, setup_board_with_ready_ticket, temp_dir_path};
use crate::db::models::{CreateProject, CreateTicket, Priority, WorkflowType};
use chrono::{Duration, Utc};

#[test]
fn reserve_next_ticket_returns_ready_ticket() {
    let db = create_test_db();
    let (_board_id, _ready_column_id, ticket) = setup_board_with_ready_ticket(&db);

    let expires = Utc::now() + Duration::minutes(30);
    let reserved = db
        .reserve_next_ticket(None, "cursor", "run-1", expires)
        .unwrap();

    assert!(reserved.is_some());
    let reserved_ticket = reserved.unwrap();
    assert_eq!(reserved_ticket.id, ticket.id);
    assert_eq!(reserved_ticket.locked_by_run_id, Some("run-1".to_string()));
}

#[test]
fn reserve_next_ticket_returns_none_when_no_ready_tickets() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    // Create ticket in Backlog, not Ready
    db.create_ticket(&CreateTicket {
        board_id: board.id.clone(),
        column_id: backlog.id.clone(),
        title: "Backlog Ticket".to_string(),
        description_md: "".to_string(),
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
    .unwrap();

    let expires = Utc::now() + Duration::minutes(30);
    let reserved = db
        .reserve_next_ticket(None, "cursor", "run-1", expires)
        .unwrap();

    assert!(reserved.is_none());
}

#[test]
fn reserve_next_ticket_skips_locked_ticket() {
    let db = create_test_db();
    let (_board_id, _ready_column_id, ticket) = setup_board_with_ready_ticket(&db);

    // Lock the ticket
    let expires = Utc::now() + Duration::minutes(30);
    db.lock_ticket(&ticket.id, "existing-run", expires).unwrap();

    // Try to reserve - should return None since the only ticket is locked
    let reserved = db
        .reserve_next_ticket(None, "cursor", "new-run", expires)
        .unwrap();
    assert!(reserved.is_none());
}

#[test]
fn reserve_next_ticket_takes_expired_lock() {
    let db = create_test_db();
    let (_board_id, _ready_column_id, ticket) = setup_board_with_ready_ticket(&db);

    // Lock the ticket with an expired time
    let expired = Utc::now() - Duration::minutes(5);
    db.lock_ticket(&ticket.id, "old-run", expired).unwrap();

    // Try to reserve - should succeed since the lock is expired
    let new_expires = Utc::now() + Duration::minutes(30);
    let reserved = db
        .reserve_next_ticket(None, "cursor", "new-run", new_expires)
        .unwrap();

    assert!(reserved.is_some());
    let reserved_ticket = reserved.unwrap();
    assert_eq!(
        reserved_ticket.locked_by_run_id,
        Some("new-run".to_string())
    );
}

#[test]
fn reserve_next_ticket_respects_project_filter() {
    let db = create_test_db();
    let project = db
        .create_project(&CreateProject {
            name: "Test Project".to_string(),
            path: temp_dir_path(),
            requires_git: true,
        })
        .unwrap();

    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let ready = columns.iter().find(|c| c.name == "Ready").unwrap();

    // Create ticket for specific project
    db.create_ticket(&CreateTicket {
        board_id: board.id.clone(),
        column_id: ready.id.clone(),
        title: "Project Ticket".to_string(),
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

    let expires = Utc::now() + Duration::minutes(30);

    // Filter for different project should not find ticket
    let other_result = db
        .reserve_next_ticket(Some("other-project"), "cursor", "run-1", expires)
        .unwrap();
    assert!(other_result.is_none());

    // Filter for correct project should find ticket
    let correct_result = db
        .reserve_next_ticket(Some(&project.id), "cursor", "run-2", expires)
        .unwrap();
    assert!(correct_result.is_some());
}

#[test]
fn reserve_next_ticket_prioritizes_by_priority_and_age() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let ready = columns.iter().find(|c| c.name == "Ready").unwrap();

    // Create low priority ticket first
    let _low = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: ready.id.clone(),
            title: "Low Priority".to_string(),
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

    // Create urgent ticket second
    let urgent = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: ready.id.clone(),
            title: "Urgent".to_string(),
            description_md: "".to_string(),
            priority: Priority::Urgent,
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

    let expires = Utc::now() + Duration::minutes(30);
    let reserved = db
        .reserve_next_ticket(None, "cursor", "run-1", expires)
        .unwrap();

    // Should get the urgent ticket even though low priority was created first
    assert!(reserved.is_some());
    assert_eq!(reserved.unwrap().id, urgent.id);
}

#[test]
fn reserve_next_ticket_skips_epic_tickets() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let ready = columns.iter().find(|c| c.name == "Ready").unwrap();

    // Create an epic ticket in Ready - should NOT be picked up
    db.create_ticket(&CreateTicket {
        board_id: board.id.clone(),
        column_id: ready.id.clone(),
        title: "Epic Ticket".to_string(),
        description_md: "This is an epic".to_string(),
        priority: Priority::High,
        labels: vec![],
        project_id: None,
        workflow_type: WorkflowType::default(),
        model: None,
        branch_name: None,
        is_epic: true, // This makes it an epic
        epic_id: None,
        depends_on_epic_id: None,
        depends_on_epic_ids: vec![],
        spec_version_id: None,
    })
    .unwrap();

    let expires = Utc::now() + Duration::minutes(30);

    // Worker should NOT pick up the epic
    let result = db
        .reserve_next_ticket(None, "cursor", "run-1", expires)
        .unwrap();
    assert!(
        result.is_none(),
        "Epic ticket should not be picked up by workers"
    );
}

#[test]
fn reserve_next_ticket_picks_child_ticket_not_epic() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let ready = columns.iter().find(|c| c.name == "Ready").unwrap();

    // Create an epic ticket in Ready
    let epic = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: ready.id.clone(),
            title: "Epic Ticket".to_string(),
            description_md: "This is an epic".to_string(),
            priority: Priority::High,
            labels: vec![],
            project_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: true,
            epic_id: None,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .unwrap();

    // Create a child ticket in Ready - this SHOULD be picked up
    let child = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: ready.id.clone(),
            title: "Child Ticket".to_string(),
            description_md: "Child of epic".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: Some(epic.id.clone()),
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .unwrap();

    let expires = Utc::now() + Duration::minutes(30);

    // Worker should pick up the child, not the epic
    let result = db
        .reserve_next_ticket(None, "cursor", "run-1", expires)
        .unwrap();
    assert!(result.is_some(), "Child ticket should be picked up");
    assert_eq!(
        result.unwrap().id,
        child.id,
        "Should pick up child ticket, not epic"
    );
}

#[test]
fn get_ready_ticket_diagnostics_counts_various_states() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    // Create various tickets in Ready column
    // 1. Normal ticket (eligible)
    db.create_ticket(&CreateTicket {
        board_id: board.id.clone(),
        column_id: ready.id.clone(),
        title: "Eligible".to_string(),
        description_md: "".to_string(),
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
    .unwrap();

    // 2. Epic in Ready (not eligible for workers)
    db.create_ticket(&CreateTicket {
        board_id: board.id.clone(),
        column_id: ready.id.clone(),
        title: "Epic".to_string(),
        description_md: "".to_string(),
        priority: Priority::Medium,
        labels: vec![],
        project_id: None,
        workflow_type: WorkflowType::default(),
        model: None,
        branch_name: None,
        is_epic: true,
        epic_id: None,
        depends_on_epic_id: None,
        depends_on_epic_ids: vec![],
        spec_version_id: None,
    })
    .unwrap();

    // 3. Paused ticket in Ready
    let paused = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: ready.id.clone(),
            title: "Paused".to_string(),
            description_md: "".to_string(),
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
        .unwrap();
    db.pause_ticket(&paused.id, "paused", "run-1").unwrap();

    // 4. Locked ticket in Ready
    let locked = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: ready.id.clone(),
            title: "Locked".to_string(),
            description_md: "".to_string(),
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
        .unwrap();
    let expires = Utc::now() + Duration::minutes(30);
    db.lock_ticket(&locked.id, "run-2", expires).unwrap();

    // Create a ticket in Backlog (not in Ready)
    db.create_ticket(&CreateTicket {
        board_id: board.id.clone(),
        column_id: backlog.id.clone(),
        title: "In Backlog".to_string(),
        description_md: "".to_string(),
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
    .unwrap();

    // Get diagnostics for Cursor agent
    let diag = db
        .get_ready_ticket_diagnostics(None, "cursor")
        .unwrap();

    assert_eq!(diag.total_ready, 4); // 4 tickets in Ready column
    assert_eq!(diag.paused, 1); // 1 paused ticket
    assert_eq!(diag.locked, 1); // 1 locked ticket
    assert_eq!(diag.epics, 1); // 1 epic
    assert_eq!(diag.eligible, 1); // Only 1 eligible for Cursor
}

#[test]
fn get_ready_ticket_diagnostics_with_project_filter() {
    let db = create_test_db();
    // Create unique temp directories for each project
    let temp1 = std::env::temp_dir().join(format!("test-project1-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp1).unwrap();
    let temp2 = std::env::temp_dir().join(format!("test-project2-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp2).unwrap();

    let project1 = db
        .create_project(&CreateProject {
            name: "My Project".to_string(),
            path: temp1.to_string_lossy().to_string(),
            requires_git: false,
        })
        .unwrap();

    let project2 = db
        .create_project(&CreateProject {
            name: "Other Project".to_string(),
            path: temp2.to_string_lossy().to_string(),
            requires_git: false,
        })
        .unwrap();

    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let ready = columns.iter().find(|c| c.name == "Ready").unwrap();

    // Create ticket for specific project
    db.create_ticket(&CreateTicket {
        board_id: board.id.clone(),
        column_id: ready.id.clone(),
        title: "Project Ticket".to_string(),
        description_md: "".to_string(),
        priority: Priority::Medium,
        labels: vec![],
        project_id: Some(project1.id.clone()),
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

    // Create ticket for different project (not NULL - since NULL != ? returns NULL, not true)
    db.create_ticket(&CreateTicket {
        board_id: board.id.clone(),
        column_id: ready.id.clone(),
        title: "Other Project Ticket".to_string(),
        description_md: "".to_string(),
        priority: Priority::Medium,
        labels: vec![],
        project_id: Some(project2.id.clone()),
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

    // Diagnostics with project filter
    let diag = db
        .get_ready_ticket_diagnostics(Some(&project1.id), "cursor")
        .unwrap();

    assert_eq!(diag.total_ready, 2); // 2 total in Ready
    assert_eq!(diag.wrong_project, 1); // 1 has different project (project2)
    assert_eq!(diag.eligible, 1); // Only 1 eligible for project1
}
