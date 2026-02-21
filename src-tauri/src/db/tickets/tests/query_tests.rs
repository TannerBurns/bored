//! Tests for cross-board query operations (get_recent_tickets_with_columns).

use super::create_test_db;
use crate::db::models::{CreateTicket, Priority, WorkflowType};

#[test]
fn get_recent_tickets_with_columns_empty_db() {
    let db = create_test_db();
    let _ = db.create_board("Board").unwrap();
    let result = db.get_recent_tickets_with_columns(10).unwrap();
    assert!(result.is_empty());
}

#[test]
fn get_recent_tickets_with_columns_returns_column_name() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = &columns[0];

    db.create_ticket(&CreateTicket {
        board_id: board.id.clone(),
        column_id: backlog.id.clone(),
        title: "Ticket A".to_string(),
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

    let result = db.get_recent_tickets_with_columns(10).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0.title, "Ticket A");
    assert_eq!(result[0].1, backlog.name);
}

#[test]
fn get_recent_tickets_with_columns_respects_limit() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();

    for i in 0..5 {
        db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: format!("Ticket {}", i),
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
    }

    let result = db.get_recent_tickets_with_columns(3).unwrap();
    assert_eq!(result.len(), 3);

    let all = db.get_recent_tickets_with_columns(10).unwrap();
    assert_eq!(all.len(), 5);
}

#[test]
fn get_recent_tickets_with_columns_ordered_by_updated_at_desc() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();

    let t1 = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "First".to_string(),
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

    std::thread::sleep(std::time::Duration::from_millis(10));

    let t2 = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Second".to_string(),
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

    let result = db.get_recent_tickets_with_columns(10).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].0.id, t2.id);
    assert_eq!(result[1].0.id, t1.id);
}

#[test]
fn get_recent_tickets_with_columns_reflects_move() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
    let review = columns.iter().find(|c| c.name == "Review").unwrap();

    let ticket = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: backlog.id.clone(),
            title: "Moveable".to_string(),
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

    let before = db.get_recent_tickets_with_columns(10).unwrap();
    assert_eq!(before[0].1, "Backlog");

    db.move_ticket(&ticket.id, &review.id).unwrap();

    let after = db.get_recent_tickets_with_columns(10).unwrap();
    assert_eq!(after[0].0.id, ticket.id);
    assert_eq!(after[0].1, "Review");
}

#[test]
fn get_recent_tickets_with_columns_spans_multiple_boards() {
    let db = create_test_db();
    let board1 = db.create_board("Board 1").unwrap();
    let board2 = db.create_board("Board 2").unwrap();
    let cols1 = db.get_columns(&board1.id).unwrap();
    let cols2 = db.get_columns(&board2.id).unwrap();

    db.create_ticket(&CreateTicket {
        board_id: board1.id.clone(),
        column_id: cols1[0].id.clone(),
        title: "Board1 Ticket".to_string(),
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

    db.create_ticket(&CreateTicket {
        board_id: board2.id.clone(),
        column_id: cols2[0].id.clone(),
        title: "Board2 Ticket".to_string(),
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

    let result = db.get_recent_tickets_with_columns(10).unwrap();
    assert_eq!(result.len(), 2);
    let titles: Vec<&str> = result.iter().map(|(t, _)| t.title.as_str()).collect();
    assert!(titles.contains(&"Board1 Ticket"));
    assert!(titles.contains(&"Board2 Ticket"));
}
