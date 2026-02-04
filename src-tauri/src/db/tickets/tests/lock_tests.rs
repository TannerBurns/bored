use super::create_test_db;
use crate::db::models::{CreateTicket, Priority, WorkflowType};
use crate::db::DbError;

#[test]
fn lock_and_unlock_ticket() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();

    let ticket = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Lockable".to_string(),
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

    let expires = chrono::Utc::now() + chrono::Duration::minutes(30);
    db.lock_ticket(&ticket.id, "run-123", expires).unwrap();

    let locked = db.get_ticket(&ticket.id).unwrap();
    assert_eq!(locked.locked_by_run_id, Some("run-123".to_string()));
    assert!(locked.lock_expires_at.is_some());

    db.unlock_ticket(&ticket.id).unwrap();

    let unlocked = db.get_ticket(&ticket.id).unwrap();
    assert!(unlocked.locked_by_run_id.is_none());
    assert!(unlocked.lock_expires_at.is_none());
}

#[test]
fn extend_lock_success() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();

    let ticket = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Extendable".to_string(),
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

    let initial_expires = chrono::Utc::now() + chrono::Duration::minutes(30);
    db.lock_ticket(&ticket.id, "run-456", initial_expires)
        .unwrap();

    let new_expires = chrono::Utc::now() + chrono::Duration::minutes(60);
    db.extend_lock(&ticket.id, "run-456", new_expires).unwrap();

    let extended = db.get_ticket(&ticket.id).unwrap();
    assert!(extended.lock_expires_at.unwrap() > initial_expires);
}

#[test]
fn extend_lock_wrong_run() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();

    let ticket = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Locked".to_string(),
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

    let expires = chrono::Utc::now() + chrono::Duration::minutes(30);
    db.lock_ticket(&ticket.id, "run-correct", expires).unwrap();

    let result = db.extend_lock(&ticket.id, "run-wrong", expires);
    assert!(matches!(result, Err(DbError::NotFound(_))));
}

#[test]
fn lock_ticket_fails_when_already_locked() {
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

    let expires = chrono::Utc::now() + chrono::Duration::minutes(30);

    // First lock should succeed
    db.lock_ticket(&ticket.id, "run-1", expires).unwrap();

    // Second lock attempt should fail (ticket is already locked with valid lock)
    let result = db.lock_ticket(&ticket.id, "run-2", expires);
    assert!(matches!(result, Err(DbError::Validation(_))));

    // Original lock should still be in place
    let locked = db.get_ticket(&ticket.id).unwrap();
    assert_eq!(locked.locked_by_run_id, Some("run-1".to_string()));
}

#[test]
fn lock_ticket_succeeds_when_same_run_reacquires() {
    // This test verifies that a paused run can re-acquire its own lock when resuming.
    // When a ticket is paused, the lock is preserved (not unlocked). When the run
    // resumes, it needs to re-lock the ticket with a new expiration time.
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

    let expires = chrono::Utc::now() + chrono::Duration::minutes(30);

    // First lock should succeed
    db.lock_ticket(&ticket.id, "run-1", expires).unwrap();

    // Same run re-acquiring lock should succeed (simulates resume after pause)
    let new_expires = chrono::Utc::now() + chrono::Duration::minutes(60);
    db.lock_ticket(&ticket.id, "run-1", new_expires).unwrap();

    // Lock should be updated with new expiration
    let locked = db.get_ticket(&ticket.id).unwrap();
    assert_eq!(locked.locked_by_run_id, Some("run-1".to_string()));
    // Verify expiration was updated (new_expires > original expires)
    assert!(locked.lock_expires_at.unwrap() > expires);
}

#[test]
fn lock_ticket_succeeds_when_lock_expired() {
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

    // Lock with an already-expired timestamp
    let expired = chrono::Utc::now() - chrono::Duration::minutes(5);
    db.lock_ticket(&ticket.id, "run-1", expired).unwrap();

    // Second lock should succeed because the first lock has expired
    let new_expires = chrono::Utc::now() + chrono::Duration::minutes(30);
    db.lock_ticket(&ticket.id, "run-2", new_expires).unwrap();

    // New lock should be in place
    let locked = db.get_ticket(&ticket.id).unwrap();
    assert_eq!(locked.locked_by_run_id, Some("run-2".to_string()));
}

#[test]
fn lock_ticket_not_found() {
    let db = create_test_db();
    let expires = chrono::Utc::now() + chrono::Duration::minutes(30);
    let result = db.lock_ticket("nonexistent", "run-1", expires);
    assert!(matches!(result, Err(DbError::NotFound(_))));
}

#[test]
fn release_lock_correct_run() {
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

    let expires = chrono::Utc::now() + chrono::Duration::minutes(30);
    db.lock_ticket(&ticket.id, "run-123", expires).unwrap();

    let locked = db.get_ticket(&ticket.id).unwrap();
    assert_eq!(locked.locked_by_run_id, Some("run-123".to_string()));

    db.release_lock(&ticket.id, "run-123").unwrap();

    let released = db.get_ticket(&ticket.id).unwrap();
    assert!(released.locked_by_run_id.is_none());
    assert!(released.lock_expires_at.is_none());
}

#[test]
fn release_lock_wrong_run_no_effect() {
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

    let expires = chrono::Utc::now() + chrono::Duration::minutes(30);
    db.lock_ticket(&ticket.id, "run-correct", expires).unwrap();

    // Try to release with wrong run_id - should have no effect
    db.release_lock(&ticket.id, "run-wrong").unwrap();

    let still_locked = db.get_ticket(&ticket.id).unwrap();
    assert_eq!(
        still_locked.locked_by_run_id,
        Some("run-correct".to_string())
    );
}

#[test]
fn update_ticket_lock_owner_success() {
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

    let expires = chrono::Utc::now() + chrono::Duration::minutes(30);
    db.lock_ticket(&ticket.id, "temp-run-id", expires).unwrap();

    // Update lock owner to new run ID
    let new_expires = chrono::Utc::now() + chrono::Duration::minutes(60);
    db.update_ticket_lock_owner(
        &ticket.id,
        "temp-run-id",
        "actual-run-id",
        Some(new_expires),
    )
    .unwrap();

    let updated = db.get_ticket(&ticket.id).unwrap();
    assert_eq!(updated.locked_by_run_id, Some("actual-run-id".to_string()));
}

#[test]
fn update_ticket_lock_owner_wrong_owner_fails() {
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

    let expires = chrono::Utc::now() + chrono::Duration::minutes(30);
    db.lock_ticket(&ticket.id, "run-1", expires).unwrap();

    // Try to update from wrong owner - should fail
    let result = db.update_ticket_lock_owner(&ticket.id, "wrong-run-id", "new-run-id", None);
    assert!(result.is_err());

    // Original lock should still be in place
    let still_locked = db.get_ticket(&ticket.id).unwrap();
    assert_eq!(still_locked.locked_by_run_id, Some("run-1".to_string()));
}
