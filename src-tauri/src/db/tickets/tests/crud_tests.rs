//! Tests for CRUD operations on tickets.

use super::{create_test_db, temp_dir_path};
use crate::db::models::{CreateProject, CreateTicket, Priority, UpdateTicket, WorkflowType};
use crate::db::DbError;

#[test]
fn create_ticket_with_all_fields() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();

    let ticket = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Test Ticket".to_string(),
            description_md: "Description".to_string(),
            priority: Priority::High,
            labels: vec!["bug".to_string()],
            project_id: None,
            workflow_type: WorkflowType::MultiStage,
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .unwrap();

    assert_eq!(ticket.title, "Test Ticket");
    assert_eq!(ticket.priority, Priority::High);
    assert_eq!(ticket.labels, vec!["bug"]);
    assert_eq!(ticket.workflow_type, WorkflowType::MultiStage);
}

#[test]
fn get_tickets_for_board() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();

    db.create_ticket(&CreateTicket {
        board_id: board.id.clone(),
        column_id: columns[0].id.clone(),
        title: "Ticket 1".to_string(),
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

    let tickets = db.get_tickets(&board.id, None).unwrap();
    assert_eq!(tickets.len(), 1);
}

#[test]
fn move_ticket_to_column() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();

    let ticket = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Ticket".to_string(),
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

    db.move_ticket(&ticket.id, &columns[1].id).unwrap();

    let tickets = db.get_tickets(&board.id, Some(&columns[1].id)).unwrap();
    assert_eq!(tickets.len(), 1);
    assert_eq!(tickets[0].id, ticket.id);
}

#[test]
fn set_ticket_project() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();

    let project = db
        .create_project(&CreateProject {
            name: "Proj".to_string(),
            path: temp_dir_path(),
            requires_git: true,
        })
        .unwrap();

    let ticket = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Ticket".to_string(),
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

    db.set_ticket_project(&ticket.id, Some(&project.id))
        .unwrap();

    let tickets = db.get_tickets(&board.id, None).unwrap();
    assert_eq!(tickets[0].project_id, Some(project.id));
}

#[test]
fn get_ticket_by_id() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();

    let created = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "My Ticket".to_string(),
            description_md: "Description".to_string(),
            priority: Priority::High,
            labels: vec!["test".to_string()],
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

    let fetched = db.get_ticket(&created.id).unwrap();
    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.title, "My Ticket");
    assert_eq!(fetched.priority, Priority::High);
}

#[test]
fn get_ticket_not_found() {
    let db = create_test_db();
    let result = db.get_ticket("nonexistent");
    assert!(matches!(result, Err(DbError::NotFound(_))));
}

#[test]
fn update_ticket_partial() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();

    let ticket = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Original".to_string(),
            description_md: "Desc".to_string(),
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

    let updated = db
        .update_ticket(
            &ticket.id,
            &UpdateTicket {
                title: Some("Updated Title".to_string()),
                description_md: None,
                priority: Some(Priority::Urgent),
                labels: None,
                project_id: None,
                workflow_type: None,
                model: None,
                branch_name: None,
                column_id: None,
                is_epic: None,
                epic_id: None,
                order_in_epic: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: None,
            },
        )
        .unwrap();

    assert_eq!(updated.title, "Updated Title");
    assert_eq!(updated.description_md, "Desc");
    assert_eq!(updated.priority, Priority::Urgent);
}

#[test]
fn update_ticket_not_found() {
    let db = create_test_db();
    let result = db.update_ticket(
        "nonexistent",
        &UpdateTicket {
            title: Some("New".to_string()),
            column_id: None,
            description_md: None,
            priority: None,
            labels: None,
            project_id: None,
            workflow_type: None,
            model: None,
            branch_name: None,
            is_epic: None,
            epic_id: None,
            order_in_epic: None,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        },
    );
    assert!(matches!(result, Err(DbError::NotFound(_))));
}

#[test]
fn update_ticket_clears_project_with_empty_string() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let project = db
        .create_project(&CreateProject {
            name: "Test Project".to_string(),
            path: temp_dir_path(),
            requires_git: true,
        })
        .unwrap();

    let ticket = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Ticket".to_string(),
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

    assert_eq!(ticket.project_id, Some(project.id.clone()));

    let updated = db
        .update_ticket(
            &ticket.id,
            &UpdateTicket {
                title: None,
                description_md: None,
                priority: None,
                labels: None,
                project_id: Some(String::new()), // Empty string clears project
                workflow_type: None,
                model: None,
                branch_name: None,
                column_id: None,
                is_epic: None,
                epic_id: None,
                order_in_epic: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: None,
            },
        )
        .unwrap();

    assert_eq!(updated.project_id, None);
}

#[test]
fn update_ticket_keeps_project_when_none() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let project = db
        .create_project(&CreateProject {
            name: "Test Project".to_string(),
            path: temp_dir_path(),
            requires_git: true,
        })
        .unwrap();

    let ticket = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Ticket".to_string(),
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

    let updated = db
        .update_ticket(
            &ticket.id,
            &UpdateTicket {
                title: Some("Updated Title".to_string()),
                description_md: None,
                priority: None,
                labels: None,
                project_id: None, // None means keep existing
                workflow_type: None,
                model: None,
                branch_name: None,
                column_id: None,
                is_epic: None,
                epic_id: None,
                order_in_epic: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: None,
            },
        )
        .unwrap();

    assert_eq!(updated.project_id, Some(project.id));
    assert_eq!(updated.title, "Updated Title");
}

#[test]
fn delete_ticket_success() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();

    let ticket = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "ToDelete".to_string(),
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

    db.delete_ticket(&ticket.id).unwrap();

    let result = db.get_ticket(&ticket.id);
    assert!(matches!(result, Err(DbError::NotFound(_))));
}

#[test]
fn delete_ticket_not_found() {
    let db = create_test_db();
    let result = db.delete_ticket("nonexistent");
    assert!(matches!(result, Err(DbError::NotFound(_))));
}

#[test]
fn set_ticket_branch_success() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();

    let ticket = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Test Ticket".to_string(),
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

    assert!(ticket.branch_name.is_none());

    db.set_ticket_branch(&ticket.id, "feat/abc123/add-feature")
        .unwrap();

    let updated = db.get_ticket(&ticket.id).unwrap();
    assert_eq!(
        updated.branch_name,
        Some("feat/abc123/add-feature".to_string())
    );
}

#[test]
fn set_ticket_branch_not_found() {
    let db = create_test_db();
    let result = db.set_ticket_branch("nonexistent-id", "some-branch");
    assert!(matches!(result, Err(DbError::NotFound(_))));
}

#[test]
fn set_ticket_branch_updates_timestamp() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();

    let ticket = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Test Ticket".to_string(),
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

    let before = ticket.updated_at;

    // Small delay to ensure timestamp differs
    std::thread::sleep(std::time::Duration::from_millis(10));

    db.set_ticket_branch(&ticket.id, "fix/123/bug-fix").unwrap();

    let updated = db.get_ticket(&ticket.id).unwrap();
    assert!(updated.updated_at >= before);
}

#[test]
fn create_ticket_with_branch_name() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();

    let ticket = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Test Ticket".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: Some("feat/preset/my-branch".to_string()),
            is_epic: false,
            epic_id: None,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .unwrap();

    assert_eq!(
        ticket.branch_name,
        Some("feat/preset/my-branch".to_string())
    );

    // Verify it persists
    let fetched = db.get_ticket(&ticket.id).unwrap();
    assert_eq!(
        fetched.branch_name,
        Some("feat/preset/my-branch".to_string())
    );
}

#[test]
fn create_ticket_does_not_auto_create_tasks() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();

    let ticket = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "My Feature Request".to_string(),
            description_md: "Implement this feature".to_string(),
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

    let tasks = db.get_tasks_for_ticket(&ticket.id).unwrap();
    assert_eq!(tasks.len(), 0, "Tickets should be created without tasks");
    assert_eq!(ticket.description_md, "Implement this feature");
}

#[test]
fn create_ticket_empty_description_creates_no_tasks() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();

    let ticket = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Quick Task".to_string(),
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

    let tasks = db.get_tasks_for_ticket(&ticket.id).unwrap();
    assert_eq!(tasks.len(), 0);
    assert_eq!(ticket.description_md, "");
}
