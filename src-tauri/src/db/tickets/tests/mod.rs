mod crud_tests;
mod epics_tests;
mod lock_tests;
mod reserve_tests;
mod state_tests;

use crate::db::models::{CreateTicket, Priority, Ticket, WorkflowType};
use crate::db::Database;

pub fn create_test_db() -> Database {
    Database::open_in_memory().unwrap()
}

pub fn temp_dir_path() -> String {
    std::env::temp_dir().to_string_lossy().to_string()
}

pub fn setup_board_with_ready_ticket(db: &Database) -> (String, String, Ticket) {
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let ready_column = columns.iter().find(|c| c.name == "Ready").unwrap();

    let ticket = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: ready_column.id.clone(),
            title: "Test Ticket".to_string(),
            description_md: "Description".to_string(),
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

    (board.id, ready_column.id.clone(), ticket)
}

pub fn create_epic_ticket(db: &Database, board_id: &str, column_id: &str, title: &str) -> Ticket {
    db.create_ticket(&CreateTicket {
        board_id: board_id.to_string(),
        column_id: column_id.to_string(),
        title: title.to_string(),
        description_md: "Epic".to_string(),
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
    .unwrap()
}

pub fn create_child_ticket(
    db: &Database,
    board_id: &str,
    column_id: &str,
    epic_id: &str,
    title: &str,
) -> Ticket {
    db.create_ticket(&CreateTicket {
        board_id: board_id.to_string(),
        column_id: column_id.to_string(),
        title: title.to_string(),
        description_md: "Child".to_string(),
        priority: Priority::Medium,
        labels: vec![],
        project_id: None,
        workflow_type: WorkflowType::default(),
        model: None,
        branch_name: None,
        is_epic: false,
        epic_id: Some(epic_id.to_string()),
        depends_on_epic_id: None,
        depends_on_epic_ids: vec![],
        spec_version_id: None,
    })
    .unwrap()
}

pub fn create_epic_with_dependency(
    db: &Database,
    board_id: &str,
    column_id: &str,
    depends_on: &str,
) -> Ticket {
    db.create_ticket(&CreateTicket {
        board_id: board_id.to_string(),
        column_id: column_id.to_string(),
        title: "Dependent Epic".to_string(),
        description_md: "".to_string(),
        priority: Priority::Medium,
        labels: vec![],
        project_id: None,
        workflow_type: WorkflowType::default(),
        model: None,
        branch_name: None,
        is_epic: true,
        epic_id: None,
        depends_on_epic_id: Some(depends_on.to_string()),
        depends_on_epic_ids: vec![depends_on.to_string()],
        spec_version_id: None,
    })
    .unwrap()
}
