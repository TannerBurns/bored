use super::{
    create_child_ticket, create_epic_ticket, create_epic_with_dependency, create_test_db,
};
use crate::db::models::{CreateTicket, Priority, WorkflowType};
use crate::db::DbError;

#[test]
fn get_epic_children_returns_ordered_children() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");

    // Create children - order_in_epic is assigned automatically
    let child1 = create_child_ticket(&db, &board.id, &backlog.id, &epic.id, "Child 1");
    let child2 = create_child_ticket(&db, &board.id, &backlog.id, &epic.id, "Child 2");
    let child3 = create_child_ticket(&db, &board.id, &backlog.id, &epic.id, "Child 3");

    let children = db.get_epic_children(&epic.id).unwrap();

    assert_eq!(children.len(), 3);
    assert_eq!(children[0].id, child1.id);
    assert_eq!(children[1].id, child2.id);
    assert_eq!(children[2].id, child3.id);

    // Verify order_in_epic values
    assert_eq!(children[0].order_in_epic, Some(0));
    assert_eq!(children[1].order_in_epic, Some(1));
    assert_eq!(children[2].order_in_epic, Some(2));
}

#[test]
fn get_epic_children_returns_empty_for_no_children() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");

    let children = db.get_epic_children(&epic.id).unwrap();
    assert!(children.is_empty());
}

#[test]
fn get_next_pending_child_returns_first_in_backlog() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
    let ready = columns.iter().find(|c| c.name == "Ready").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &ready.id, "Epic");

    // First child in Ready (not pending), second in Backlog
    let _child1 = create_child_ticket(&db, &board.id, &ready.id, &epic.id, "Child 1");
    let child2 = create_child_ticket(&db, &board.id, &backlog.id, &epic.id, "Child 2");

    let next = db.get_next_pending_child(&epic.id).unwrap();
    assert!(next.is_some());
    assert_eq!(next.unwrap().id, child2.id);
}

#[test]
fn get_next_pending_child_returns_none_when_no_backlog() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
    let done = columns.iter().find(|c| c.name == "Done").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &ready.id, "Epic");

    // All children done or in progress
    let _child1 = create_child_ticket(&db, &board.id, &done.id, &epic.id, "Child 1");
    let _child2 = create_child_ticket(&db, &board.id, &ready.id, &epic.id, "Child 2");

    let next = db.get_next_pending_child(&epic.id).unwrap();
    assert!(next.is_none());
}

#[test]
fn get_epic_progress_counts_columns_correctly() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
    let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
    let done = columns.iter().find(|c| c.name == "Done").unwrap();
    let blocked = columns.iter().find(|c| c.name == "Blocked").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &ready.id, "Epic");

    // Create children in various columns
    create_child_ticket(&db, &board.id, &backlog.id, &epic.id, "Backlog 1");
    create_child_ticket(&db, &board.id, &backlog.id, &epic.id, "Backlog 2");
    create_child_ticket(&db, &board.id, &ready.id, &epic.id, "Ready 1");
    create_child_ticket(&db, &board.id, &done.id, &epic.id, "Done 1");
    create_child_ticket(&db, &board.id, &blocked.id, &epic.id, "Blocked 1");

    let progress = db.get_epic_progress(&epic.id).unwrap();

    assert_eq!(progress.total, 5);
    assert_eq!(progress.backlog, 2);
    assert_eq!(progress.ready, 1);
    assert_eq!(progress.done, 1);
    assert_eq!(progress.blocked, 1);
    assert_eq!(progress.in_progress, 0);
    assert_eq!(progress.review, 0);
}

#[test]
fn get_epic_progress_returns_zeros_for_no_children() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");

    let progress = db.get_epic_progress(&epic.id).unwrap();

    assert_eq!(progress.total, 0);
    assert_eq!(progress.backlog, 0);
    assert_eq!(progress.done, 0);
}

#[test]
fn add_ticket_to_epic_success() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");

    // Create a standalone ticket (not a child)
    let ticket = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: backlog.id.clone(),
            title: "Standalone".to_string(),
            description_md: "Not a child yet".to_string(),
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

    assert!(ticket.epic_id.is_none());

    // Add to epic
    db.add_ticket_to_epic(&epic.id, &ticket.id).unwrap();

    // Verify
    let updated = db.get_ticket(&ticket.id).unwrap();
    assert_eq!(updated.epic_id, Some(epic.id.clone()));
    assert_eq!(updated.order_in_epic, Some(0));
}

#[test]
fn add_ticket_to_epic_fails_if_not_epic() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    // Create a non-epic ticket
    let not_epic = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: backlog.id.clone(),
            title: "Not Epic".to_string(),
            description_md: "Regular ticket".to_string(),
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

    let ticket = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: backlog.id.clone(),
            title: "Ticket".to_string(),
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

    let result = db.add_ticket_to_epic(&not_epic.id, &ticket.id);
    assert!(matches!(result, Err(DbError::Validation(_))));
}

#[test]
fn remove_ticket_from_epic_success() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");
    let child = create_child_ticket(&db, &board.id, &backlog.id, &epic.id, "Child");

    assert!(child.epic_id.is_some());

    db.remove_ticket_from_epic(&child.id).unwrap();

    let updated = db.get_ticket(&child.id).unwrap();
    assert!(updated.epic_id.is_none());
    assert!(updated.order_in_epic.is_none());
}

#[test]
fn reorder_epic_children_success() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");
    let child1 = create_child_ticket(&db, &board.id, &backlog.id, &epic.id, "Child 1");
    let child2 = create_child_ticket(&db, &board.id, &backlog.id, &epic.id, "Child 2");
    let child3 = create_child_ticket(&db, &board.id, &backlog.id, &epic.id, "Child 3");

    // Reorder: 3, 1, 2
    db.reorder_epic_children(
        &epic.id,
        &[child3.id.clone(), child1.id.clone(), child2.id.clone()],
    )
    .unwrap();

    let children = db.get_epic_children(&epic.id).unwrap();
    assert_eq!(children[0].id, child3.id);
    assert_eq!(children[1].id, child1.id);
    assert_eq!(children[2].id, child2.id);
}

#[test]
fn get_previous_epic_sibling_returns_none_for_first_child() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");
    let child1 = create_child_ticket(&db, &board.id, &backlog.id, &epic.id, "Child 1");
    let _child2 = create_child_ticket(&db, &board.id, &backlog.id, &epic.id, "Child 2");

    let prev = db.get_previous_epic_sibling(&child1.id).unwrap();
    assert!(prev.is_none());
}

#[test]
fn get_previous_epic_sibling_returns_previous_child() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");
    let child1 = create_child_ticket(&db, &board.id, &backlog.id, &epic.id, "Child 1");
    let child2 = create_child_ticket(&db, &board.id, &backlog.id, &epic.id, "Child 2");

    let prev = db.get_previous_epic_sibling(&child2.id).unwrap();
    assert!(prev.is_some());
    assert_eq!(prev.unwrap().id, child1.id);
}

#[test]
fn get_previous_epic_sibling_returns_none_for_non_child() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    // Create a standalone ticket (not a child of any epic)
    let ticket = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: backlog.id.clone(),
            title: "Standalone".to_string(),
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

    let prev = db.get_previous_epic_sibling(&ticket.id).unwrap();
    assert!(prev.is_none());
}

#[test]
fn are_all_epic_children_done_true_when_all_done() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
    let done = columns.iter().find(|c| c.name == "Done").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");
    create_child_ticket(&db, &board.id, &done.id, &epic.id, "Done Child 1");
    create_child_ticket(&db, &board.id, &done.id, &epic.id, "Done Child 2");

    assert!(db.are_all_epic_children_done(&epic.id).unwrap());
}

#[test]
fn are_all_epic_children_done_false_when_some_not_done() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
    let done = columns.iter().find(|c| c.name == "Done").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");
    create_child_ticket(&db, &board.id, &done.id, &epic.id, "Done Child");
    create_child_ticket(&db, &board.id, &backlog.id, &epic.id, "Backlog Child");

    assert!(!db.are_all_epic_children_done(&epic.id).unwrap());
}

#[test]
fn are_all_epic_children_done_false_when_no_children() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");

    // No children = false (need at least one child to be "all done")
    assert!(!db.are_all_epic_children_done(&epic.id).unwrap());
}

#[test]
fn create_ticket_with_epic_id_sets_order() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");

    let child1 = create_child_ticket(&db, &board.id, &backlog.id, &epic.id, "Child 1");
    let child2 = create_child_ticket(&db, &board.id, &backlog.id, &epic.id, "Child 2");

    assert_eq!(child1.order_in_epic, Some(0));
    assert_eq!(child2.order_in_epic, Some(1));
}

#[test]
fn get_epics_depending_on_finds_dependents() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    // Create the base epic
    let base_epic = create_epic_ticket(&db, &board.id, &backlog.id, "Base Epic");

    // Create two epics that depend on the base
    let dep1 = create_epic_with_dependency(&db, &board.id, &backlog.id, &base_epic.id);
    let dep2 = create_epic_with_dependency(&db, &board.id, &backlog.id, &base_epic.id);

    // Find dependents
    let dependents = db.get_epics_depending_on(&base_epic.id).unwrap();

    assert_eq!(dependents.len(), 2);
    let dep_ids: Vec<_> = dependents.iter().map(|t| t.id.as_str()).collect();
    assert!(dep_ids.contains(&dep1.id.as_str()));
    assert!(dep_ids.contains(&dep2.id.as_str()));
}

#[test]
fn get_epics_depending_on_returns_empty_when_none() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Lonely Epic");

    let dependents = db.get_epics_depending_on(&epic.id).unwrap();
    assert!(dependents.is_empty());
}

#[test]
fn get_dependency_base_branch_returns_last_done_child_branch() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
    let done = columns.iter().find(|c| c.name == "Done").unwrap();

    // Create the dependency epic
    let dep_epic = create_epic_ticket(&db, &board.id, &done.id, "Dependency Epic");

    // Create children with branches, put them in Done
    let child1 = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: done.id.clone(),
            title: "Child 1".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            workspace_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: Some("feat/child-1".to_string()),
            is_epic: false,
            epic_id: Some(dep_epic.id.clone()),
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .unwrap();

    let _child2 = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: done.id.clone(),
            title: "Child 2".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            workspace_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: Some("feat/child-2".to_string()),
            is_epic: false,
            epic_id: Some(dep_epic.id.clone()),
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .unwrap();

    // Create the dependent epic
    let dependent = create_epic_with_dependency(&db, &board.id, &backlog.id, &dep_epic.id);

    // Get the base branch for the dependent epic
    let branch = db.get_dependency_base_branch(&dependent.id).unwrap();

    // Should return the last (highest order) child's branch
    assert!(branch.is_some());
    // Child 2 has order 1 (highest), so its branch should be returned
    assert_eq!(branch.unwrap(), "feat/child-2");

    // Clean up - also verify child1 was created correctly
    assert_eq!(child1.order_in_epic, Some(0));
}

#[test]
fn get_dependency_base_branch_returns_none_when_no_dependency() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    // Create epic without dependency
    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "No Dependency");

    let branch = db.get_dependency_base_branch(&epic.id).unwrap();
    assert!(branch.is_none());
}

#[test]
fn get_dependency_base_branch_returns_none_when_no_done_children() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    // Create the dependency epic with no children in Done
    let dep_epic = create_epic_ticket(&db, &board.id, &backlog.id, "Dep Epic");
    create_child_ticket(&db, &board.id, &backlog.id, &dep_epic.id, "Backlog Child");

    // Create the dependent epic
    let dependent = create_epic_with_dependency(&db, &board.id, &backlog.id, &dep_epic.id);

    let branch = db.get_dependency_base_branch(&dependent.id).unwrap();
    // No Done children, so no branch
    assert!(branch.is_none());
}

#[test]
fn get_epic_final_branch_returns_last_done_child_branch() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
    let done = columns.iter().find(|c| c.name == "Done").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "My Epic");

    // Create children with branches in Done
    db.create_ticket(&CreateTicket {
        board_id: board.id.clone(),
        column_id: done.id.clone(),
        title: "Child 1".to_string(),
        description_md: "".to_string(),
        priority: Priority::Medium,
        labels: vec![],
        project_id: None,
        workspace_id: None,
        workflow_type: WorkflowType::default(),
        model: None,
        branch_name: Some("feat/child-1".to_string()),
        is_epic: false,
        epic_id: Some(epic.id.clone()),
        depends_on_epic_id: None,
        depends_on_epic_ids: vec![],
        spec_version_id: None,
    })
    .unwrap();

    db.create_ticket(&CreateTicket {
        board_id: board.id.clone(),
        column_id: done.id.clone(),
        title: "Child 2".to_string(),
        description_md: "".to_string(),
        priority: Priority::Medium,
        labels: vec![],
        project_id: None,
        workspace_id: None,
        workflow_type: WorkflowType::default(),
        model: None,
        branch_name: Some("feat/child-2".to_string()),
        is_epic: false,
        epic_id: Some(epic.id.clone()),
        depends_on_epic_id: None,
        depends_on_epic_ids: vec![],
        spec_version_id: None,
    })
    .unwrap();

    let branch = db.get_epic_final_branch(&epic.id).unwrap();
    assert!(branch.is_some());
    // Should return the last (highest order) child's branch
    assert_eq!(branch.unwrap(), "feat/child-2");
}

#[test]
fn get_epic_final_branch_returns_none_when_no_done_children() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "My Epic");

    // Create children only in Backlog (not Done)
    create_child_ticket(&db, &board.id, &backlog.id, &epic.id, "Backlog Child");

    let branch = db.get_epic_final_branch(&epic.id).unwrap();
    assert!(branch.is_none());
}

#[test]
fn get_epic_final_branch_returns_none_when_no_children() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Empty Epic");

    let branch = db.get_epic_final_branch(&epic.id).unwrap();
    assert!(branch.is_none());
}

#[test]
fn get_spec_epics_with_branches_returns_epics_and_branches() {
    use crate::db::models::{CreateProject, CreateSpec, CreateSpecVersion};

    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let done = columns.iter().find(|c| c.name == "Done").unwrap();

    // Create project first (required for spec)
    let project = db
        .create_project(&CreateProject {
            name: "Test Project".to_string(),
            path: std::env::temp_dir().to_string_lossy().to_string(),
            requires_git: false,
        })
        .unwrap();

    // Create a spec and version
    let spec = db
        .create_spec(&CreateSpec {
            board_id: board.id.clone(),
            target_board_id: Some(board.id.clone()),
            project_id: project.id.clone(),
            name: "Test Spec".to_string(),
            user_input: "Test input".to_string(),
            model: None,
            settings: serde_json::json!({}),
        })
        .unwrap();

    let version = db
        .create_spec_version(&CreateSpecVersion {
            spec_id: spec.id.clone(),
        })
        .unwrap();

    // Create epic linked to spec version
    let epic = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: done.id.clone(),
            title: "Feature Epic".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            workspace_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: true,
            epic_id: None,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: Some(version.id.clone()),
        })
        .unwrap();

    // Create child with branch in Done
    db.create_ticket(&CreateTicket {
        board_id: board.id.clone(),
        column_id: done.id.clone(),
        title: "Child".to_string(),
        description_md: "".to_string(),
        priority: Priority::Medium,
        labels: vec![],
        project_id: None,
        workspace_id: None,
        workflow_type: WorkflowType::default(),
        model: None,
        branch_name: Some("feat/epic-branch".to_string()),
        is_epic: false,
        epic_id: Some(epic.id.clone()),
        depends_on_epic_id: None,
        depends_on_epic_ids: vec![],
        spec_version_id: None,
    })
    .unwrap();

    let epics_with_branches = db.get_spec_epics_with_branches(&version.id).unwrap();

    assert_eq!(epics_with_branches.len(), 1);
    let (id, title, branch) = &epics_with_branches[0];
    assert_eq!(id, &epic.id);
    assert_eq!(title, "Feature Epic");
    assert_eq!(branch, &Some("feat/epic-branch".to_string()));
}

#[test]
fn get_spec_epics_with_branches_excludes_consolidation_epics() {
    use crate::db::models::{CreateProject, CreateSpec, CreateSpecVersion};

    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let done = columns.iter().find(|c| c.name == "Done").unwrap();

    // Create project first (required for spec)
    let project = db
        .create_project(&CreateProject {
            name: "Test Project".to_string(),
            path: std::env::temp_dir().to_string_lossy().to_string(),
            requires_git: false,
        })
        .unwrap();

    // Create a spec and version
    let spec = db
        .create_spec(&CreateSpec {
            board_id: board.id.clone(),
            target_board_id: Some(board.id.clone()),
            project_id: project.id.clone(),
            name: "Test Spec".to_string(),
            user_input: "Test input".to_string(),
            model: None,
            settings: serde_json::json!({}),
        })
        .unwrap();

    let version = db
        .create_spec_version(&CreateSpecVersion {
            spec_id: spec.id.clone(),
        })
        .unwrap();

    // Create a regular epic
    let regular_epic = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: done.id.clone(),
            title: "Regular Epic".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            workspace_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: true,
            epic_id: None,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: Some(version.id.clone()),
        })
        .unwrap();

    // Create a consolidation epic (title starts with "Consolidate")
    db.create_ticket(&CreateTicket {
        board_id: board.id.clone(),
        column_id: done.id.clone(),
        title: "Consolidate branches".to_string(),
        description_md: "".to_string(),
        priority: Priority::Medium,
        labels: vec![],
        project_id: None,
        workspace_id: None,
        workflow_type: WorkflowType::default(),
        model: None,
        branch_name: None,
        is_epic: true,
        epic_id: None,
        depends_on_epic_id: None,
        depends_on_epic_ids: vec![],
        spec_version_id: Some(version.id.clone()),
    })
    .unwrap();

    let epics_with_branches = db.get_spec_epics_with_branches(&version.id).unwrap();

    // Should only include the regular epic, not the consolidation epic
    assert_eq!(epics_with_branches.len(), 1);
    assert_eq!(epics_with_branches[0].0, regular_epic.id);
}

#[test]
fn get_spec_epics_with_branches_returns_empty_for_no_epics() {
    use crate::db::models::{CreateProject, CreateSpec, CreateSpecVersion};

    let db = create_test_db();
    let board = db.create_board("Board").unwrap();

    // Create project first (required for spec)
    let project = db
        .create_project(&CreateProject {
            name: "Test Project".to_string(),
            path: std::env::temp_dir().to_string_lossy().to_string(),
            requires_git: false,
        })
        .unwrap();

    // Create a spec and version with no epics
    let spec = db
        .create_spec(&CreateSpec {
            board_id: board.id.clone(),
            target_board_id: Some(board.id.clone()),
            project_id: project.id.clone(),
            name: "Empty Spec".to_string(),
            user_input: "Test input".to_string(),
            model: None,
            settings: serde_json::json!({}),
        })
        .unwrap();

    let version = db
        .create_spec_version(&CreateSpecVersion {
            spec_id: spec.id.clone(),
        })
        .unwrap();

    let epics_with_branches = db.get_spec_epics_with_branches(&version.id).unwrap();
    assert!(epics_with_branches.is_empty());
}

#[test]
fn shift_epic_children_order_shifts_all_children() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");

    // Create children - they'll get order 0, 1, 2
    let child1 = create_child_ticket(&db, &board.id, &backlog.id, &epic.id, "Child 1");
    let child2 = create_child_ticket(&db, &board.id, &backlog.id, &epic.id, "Child 2");
    let child3 = create_child_ticket(&db, &board.id, &backlog.id, &epic.id, "Child 3");

    assert_eq!(child1.order_in_epic, Some(0));
    assert_eq!(child2.order_in_epic, Some(1));
    assert_eq!(child3.order_in_epic, Some(2));

    // Shift all by 1
    db.shift_epic_children_order(&epic.id, 1).unwrap();

    // Verify orders are now 1, 2, 3
    let updated1 = db.get_ticket(&child1.id).unwrap();
    let updated2 = db.get_ticket(&child2.id).unwrap();
    let updated3 = db.get_ticket(&child3.id).unwrap();

    assert_eq!(updated1.order_in_epic, Some(1));
    assert_eq!(updated2.order_in_epic, Some(2));
    assert_eq!(updated3.order_in_epic, Some(3));
}

#[test]
fn set_ticket_order_in_epic_updates_order() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");
    let child = create_child_ticket(&db, &board.id, &backlog.id, &epic.id, "Child");

    assert_eq!(child.order_in_epic, Some(0));

    // Set to a different order
    db.set_ticket_order_in_epic(&child.id, 5).unwrap();

    let updated = db.get_ticket(&child.id).unwrap();
    assert_eq!(updated.order_in_epic, Some(5));
}

#[test]
fn has_merge_dependencies_ticket_returns_false_when_none() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");

    // No merge-dependencies ticket exists
    assert!(!db.has_merge_dependencies_ticket(&epic.id).unwrap());
}

#[test]
fn has_merge_dependencies_ticket_returns_true_when_exists() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");

    // Create a child with merge-dependencies label
    db.create_ticket(&CreateTicket {
        board_id: board.id.clone(),
        column_id: backlog.id.clone(),
        title: "Merge Dependencies".to_string(),
        description_md: "".to_string(),
        priority: Priority::High,
        labels: vec![
            "auto-generated".to_string(),
            "merge-dependencies".to_string(),
        ],
        project_id: None,
        workspace_id: None,
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

    // Should now find the merge-dependencies ticket
    assert!(db.has_merge_dependencies_ticket(&epic.id).unwrap());
}

#[test]
fn has_merge_dependencies_ticket_ignores_similar_labels() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");

    // Create a child with a similar but NOT exact label
    db.create_ticket(&CreateTicket {
        board_id: board.id.clone(),
        column_id: backlog.id.clone(),
        title: "Not a merge deps ticket".to_string(),
        description_md: "".to_string(),
        priority: Priority::Medium,
        labels: vec![
            "merge-dependencies-v2".to_string(), // Similar but not exact
        ],
        project_id: None,
        workspace_id: None,
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

    // Should NOT find a merge-dependencies ticket (similar label doesn't count)
    assert!(
        !db.has_merge_dependencies_ticket(&epic.id).unwrap(),
        "Similar labels like 'merge-dependencies-v2' should not match"
    );

    // Now add the exact label
    db.create_ticket(&CreateTicket {
        board_id: board.id.clone(),
        column_id: backlog.id.clone(),
        title: "Real merge deps".to_string(),
        description_md: "".to_string(),
        priority: Priority::High,
        labels: vec!["merge-dependencies".to_string()],
        project_id: None,
        workspace_id: None,
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

    // Now it should find the exact match
    assert!(db.has_merge_dependencies_ticket(&epic.id).unwrap());
}

#[test]
fn get_merge_dependencies_ticket_returns_ticket_when_exists() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");

    // No ticket yet
    assert!(db.get_merge_dependencies_ticket(&epic.id).unwrap().is_none());

    // Create merge-dependencies ticket
    let merge_ticket = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: backlog.id.clone(),
            title: "Merge Dependencies".to_string(),
            description_md: "Merge all the things".to_string(),
            priority: Priority::High,
            labels: vec![
                "auto-generated".to_string(),
                "merge-dependencies".to_string(),
            ],
            project_id: None,
            workspace_id: None,
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

    // Should now return the ticket
    let result = db.get_merge_dependencies_ticket(&epic.id).unwrap();
    assert!(result.is_some());
    let ticket = result.unwrap();
    assert_eq!(ticket.id, merge_ticket.id);
    assert!(ticket.labels.contains(&"merge-dependencies".to_string()));
}

#[test]
fn get_merge_dependencies_ticket_returns_ticket_regardless_of_order() {
    // This test verifies that get_merge_dependencies_ticket returns the ticket
    // even if its order_in_epic is wrong (e.g., due to a partial injection failure).
    // This is important for the repair logic.
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");

    // Create merge-dependencies ticket
    let merge_ticket = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: backlog.id.clone(),
            title: "Merge Dependencies".to_string(),
            description_md: "".to_string(),
            priority: Priority::High,
            labels: vec!["merge-dependencies".to_string()],
            project_id: None,
            workspace_id: None,
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

    // Set to a wrong order (simulating partial failure)
    db.set_ticket_order_in_epic(&merge_ticket.id, 99).unwrap();

    // Should still return the ticket even with wrong order
    let result = db.get_merge_dependencies_ticket(&epic.id).unwrap();
    assert!(result.is_some());
    let ticket = result.unwrap();
    assert_eq!(ticket.id, merge_ticket.id);
    assert_eq!(ticket.order_in_epic, Some(99)); // Order is wrong but ticket is returned
}

#[test]
fn get_all_dependency_branches_returns_branches_from_all_deps() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
    let done = columns.iter().find(|c| c.name == "Done").unwrap();

    // Create two dependency epics with children that have branches
    let dep_epic1 = create_epic_ticket(&db, &board.id, &done.id, "Dep Epic 1");
    let dep_epic2 = create_epic_ticket(&db, &board.id, &done.id, "Dep Epic 2");

    // Add completed children with branches to each dependency epic
    db.create_ticket(&CreateTicket {
        board_id: board.id.clone(),
        column_id: done.id.clone(),
        title: "Child of Epic 1".to_string(),
        description_md: "".to_string(),
        priority: Priority::Medium,
        labels: vec![],
        project_id: None,
        workspace_id: None,
        workflow_type: WorkflowType::default(),
        model: None,
        branch_name: Some("feat/epic1-branch".to_string()),
        is_epic: false,
        epic_id: Some(dep_epic1.id.clone()),
        depends_on_epic_id: None,
        depends_on_epic_ids: vec![],
        spec_version_id: None,
    })
    .unwrap();

    db.create_ticket(&CreateTicket {
        board_id: board.id.clone(),
        column_id: done.id.clone(),
        title: "Child of Epic 2".to_string(),
        description_md: "".to_string(),
        priority: Priority::Medium,
        labels: vec![],
        project_id: None,
        workspace_id: None,
        workflow_type: WorkflowType::default(),
        model: None,
        branch_name: Some("feat/epic2-branch".to_string()),
        is_epic: false,
        epic_id: Some(dep_epic2.id.clone()),
        depends_on_epic_id: None,
        depends_on_epic_ids: vec![],
        spec_version_id: None,
    })
    .unwrap();

    // Create an epic that depends on BOTH dependency epics
    let multi_dep_epic = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: backlog.id.clone(),
            title: "Multi-Dep Epic".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            workspace_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: true,
            epic_id: None,
            depends_on_epic_id: Some(dep_epic1.id.clone()), // Primary
            depends_on_epic_ids: vec![dep_epic1.id.clone(), dep_epic2.id.clone()], // All
            spec_version_id: None,
        })
        .unwrap();

    // Get all dependency branches
    let branches = db.get_all_dependency_branches(&multi_dep_epic.id).unwrap();

    assert_eq!(branches.len(), 2);
    
    // Verify both branches are returned
    let branch_names: Vec<_> = branches.iter().map(|(_, _, b)| b.as_str()).collect();
    assert!(branch_names.contains(&"feat/epic1-branch"));
    assert!(branch_names.contains(&"feat/epic2-branch"));
}

#[test]
fn get_all_dependency_branches_returns_empty_when_no_deps() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "No Deps Epic");

    let branches = db.get_all_dependency_branches(&epic.id).unwrap();
    assert!(branches.is_empty());
}

#[test]
fn shift_epic_children_order_handles_null_order_values() {
    // This test verifies that shift_epic_children_order also fixes legacy tickets
    // that have NULL order_in_epic values, which would otherwise sort before order 0
    // in SQLite's default ascending sort (NULL sorts first).
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");

    // Create a child ticket normally (will get order 0)
    let normal_child = create_child_ticket(&db, &board.id, &backlog.id, &epic.id, "Normal Child");
    assert_eq!(normal_child.order_in_epic, Some(0));

    // Create a "legacy" ticket without an epic, then manually set its epic_id
    // via raw SQL to simulate a legacy ticket with NULL order_in_epic
    let legacy_ticket = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: backlog.id.clone(),
            title: "Legacy Child".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            workspace_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None, // No epic initially
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .unwrap();

    // Now manually add it to the epic with NULL order_in_epic (simulating legacy data)
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE tickets SET epic_id = ? WHERE id = ?",
            rusqlite::params![epic.id, legacy_ticket.id],
        )?;
        Ok::<_, crate::db::DbError>(())
    })
    .unwrap();

    // Verify the legacy ticket has NULL order_in_epic
    let legacy = db.get_ticket(&legacy_ticket.id).unwrap();
    assert!(
        legacy.order_in_epic.is_none(),
        "Legacy ticket should have NULL order_in_epic"
    );

    // Now shift all children by 1 (simulating merge-dependencies injection)
    db.shift_epic_children_order(&epic.id, 1).unwrap();

    // After the fix, the legacy ticket should have been assigned an order value
    // (shifted from NULL to a real value), not left as NULL
    let updated_legacy = db.get_ticket(&legacy_ticket.id).unwrap();
    let updated_normal = db.get_ticket(&normal_child.id).unwrap();

    // The normal child should have been shifted from 0 to 1
    assert_eq!(updated_normal.order_in_epic, Some(1));

    // The legacy ticket should now have an order value, not NULL
    // After the fix, NULL values get assigned max_order + shift (so they sort last)
    assert!(
        updated_legacy.order_in_epic.is_some(),
        "Legacy ticket with NULL order should be assigned a value during shift"
    );
}

#[test]
fn get_next_pending_child_returns_order_zero_before_null() {
    // This test verifies that get_next_pending_child returns a ticket at order 0
    // before a legacy ticket with NULL order_in_epic.
    // In SQLite, NULL sorts first in ASC order by default, so we need NULLS LAST.
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");

    // Create a "legacy" ticket without order, directly via SQL
    let legacy_ticket = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: backlog.id.clone(),
            title: "Legacy Child".to_string(),
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

    // Manually add to epic with NULL order
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE tickets SET epic_id = ? WHERE id = ?",
            rusqlite::params![epic.id, legacy_ticket.id],
        )?;
        Ok::<_, crate::db::DbError>(())
    })
    .unwrap();

    // Create merge-dependencies ticket at order 0
    let merge_ticket = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: backlog.id.clone(),
            title: "Merge Dependencies".to_string(),
            description_md: "".to_string(),
            priority: Priority::High,
            labels: vec!["merge-dependencies".to_string()],
            project_id: None,
            workspace_id: None,
            workflow_type: WorkflowType::MultiStage,
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: Some(epic.id.clone()),
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .unwrap();

    // Set merge ticket to order 0 explicitly
    db.set_ticket_order_in_epic(&merge_ticket.id, 0).unwrap();

    // get_next_pending_child should return the merge ticket (order 0), not the legacy ticket (NULL)
    let next = db.get_next_pending_child(&epic.id).unwrap();
    assert!(next.is_some(), "Should find a pending child");
    let next_ticket = next.unwrap();
    assert_eq!(
        next_ticket.id, merge_ticket.id,
        "Should return merge ticket at order 0, not legacy ticket with NULL order"
    );
}

#[test]
fn get_epic_children_orders_null_last() {
    // Verify that get_epic_children orders NULL order_in_epic values AFTER numeric values
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");

    // Create children with explicit orders
    let child_0 = create_child_ticket(&db, &board.id, &backlog.id, &epic.id, "Child 0");
    let child_1 = create_child_ticket(&db, &board.id, &backlog.id, &epic.id, "Child 1");

    // Create a legacy ticket with NULL order
    let legacy = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: backlog.id.clone(),
            title: "Legacy".to_string(),
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

    db.with_conn(|conn| {
        conn.execute(
            "UPDATE tickets SET epic_id = ? WHERE id = ?",
            rusqlite::params![epic.id, legacy.id],
        )?;
        Ok::<_, crate::db::DbError>(())
    })
    .unwrap();

    let children = db.get_epic_children(&epic.id).unwrap();
    assert_eq!(children.len(), 3);

    // Ordered children should come first
    assert_eq!(children[0].id, child_0.id);
    assert_eq!(children[1].id, child_1.id);
    // Legacy ticket with NULL order should be last
    assert_eq!(children[2].id, legacy.id);
}

// ======================================================================
// has_active_epic_child
// ======================================================================

#[test]
fn has_active_child_false_when_no_children() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");
    assert!(!db.has_active_epic_child(&epic.id).unwrap());
}

#[test]
fn has_active_child_false_when_all_backlog() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");
    create_child_ticket(&db, &board.id, &backlog.id, &epic.id, "C1");
    create_child_ticket(&db, &board.id, &backlog.id, &epic.id, "C2");
    assert!(!db.has_active_epic_child(&epic.id).unwrap());
}

#[test]
fn has_active_child_false_when_all_done() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
    let done = columns.iter().find(|c| c.name == "Done").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");
    create_child_ticket(&db, &board.id, &done.id, &epic.id, "C1");
    assert!(!db.has_active_epic_child(&epic.id).unwrap());
}

#[test]
fn has_active_child_true_when_child_in_ready() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
    let ready = columns.iter().find(|c| c.name == "Ready").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");
    create_child_ticket(&db, &board.id, &ready.id, &epic.id, "C1");
    assert!(db.has_active_epic_child(&epic.id).unwrap());
}

#[test]
fn has_active_child_true_when_child_in_progress() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
    let in_progress = columns.iter().find(|c| c.name == "In Progress").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");
    create_child_ticket(&db, &board.id, &in_progress.id, &epic.id, "C1");
    assert!(db.has_active_epic_child(&epic.id).unwrap());
}

// ======================================================================
// are_all_dependencies_complete
// ======================================================================

fn create_epic_with_multi_deps(
    db: &crate::db::Database,
    board_id: &str,
    column_id: &str,
    dep_ids: Vec<String>,
) -> crate::db::models::Ticket {
    db.create_ticket(&CreateTicket {
        board_id: board_id.to_string(),
        column_id: column_id.to_string(),
        title: "Multi-Dep Epic".to_string(),
        description_md: "".to_string(),
        priority: Priority::Medium,
        labels: vec![],
        project_id: None,
        workspace_id: None,
        workflow_type: WorkflowType::default(),
        model: None,
        branch_name: None,
        is_epic: true,
        epic_id: None,
        depends_on_epic_id: dep_ids.first().cloned(),
        depends_on_epic_ids: dep_ids,
        spec_version_id: None,
    })
    .unwrap()
}

#[test]
fn all_deps_complete_returns_none_when_no_dependencies() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

    let epic = create_epic_ticket(&db, &board.id, &backlog.id, "Epic");
    assert!(db.are_all_dependencies_complete(&epic).unwrap().is_none());
}

#[test]
fn all_deps_complete_returns_none_when_single_dep_done() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
    let done = columns.iter().find(|c| c.name == "Done").unwrap();

    let dep = create_epic_ticket(&db, &board.id, &done.id, "Dep");
    let epic = create_epic_with_dependency(&db, &board.id, &backlog.id, &dep.id);
    assert!(db.are_all_dependencies_complete(&epic).unwrap().is_none());
}

#[test]
fn all_deps_complete_returns_incomplete_when_single_dep_not_done() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
    let ready = columns.iter().find(|c| c.name == "Ready").unwrap();

    let dep = create_epic_ticket(&db, &board.id, &ready.id, "Dep");
    let epic = create_epic_with_dependency(&db, &board.id, &backlog.id, &dep.id);
    let result = db.are_all_dependencies_complete(&epic).unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap().id, dep.id);
}

#[test]
fn all_deps_complete_returns_none_when_all_multi_deps_done() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
    let done = columns.iter().find(|c| c.name == "Done").unwrap();

    let dep_a = create_epic_ticket(&db, &board.id, &done.id, "A");
    let dep_b = create_epic_ticket(&db, &board.id, &done.id, "B");
    let epic = create_epic_with_multi_deps(
        &db,
        &board.id,
        &backlog.id,
        vec![dep_a.id.clone(), dep_b.id.clone()],
    );
    assert!(db.are_all_dependencies_complete(&epic).unwrap().is_none());
}

#[test]
fn all_deps_complete_returns_first_incomplete_multi_dep() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
    let done = columns.iter().find(|c| c.name == "Done").unwrap();
    let ready = columns.iter().find(|c| c.name == "Ready").unwrap();

    let dep_a = create_epic_ticket(&db, &board.id, &done.id, "A");
    let dep_b = create_epic_ticket(&db, &board.id, &ready.id, "B");
    let epic = create_epic_with_multi_deps(
        &db,
        &board.id,
        &backlog.id,
        vec![dep_a.id.clone(), dep_b.id.clone()],
    );
    let result = db.are_all_dependencies_complete(&epic).unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap().id, dep_b.id);
}

#[test]
fn all_deps_complete_legacy_fallback_with_empty_depends_on_epic_ids() {
    // Simulates a ticket created before multi-dep support: only
    // depends_on_epic_id is set, depends_on_epic_ids is empty.
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
    let ready = columns.iter().find(|c| c.name == "Ready").unwrap();

    let dep = create_epic_ticket(&db, &board.id, &ready.id, "Dep");

    // Create with only depends_on_epic_id, empty depends_on_epic_ids
    let legacy = db
        .create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: backlog.id.clone(),
            title: "Legacy".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            workspace_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: true,
            epic_id: None,
            depends_on_epic_id: Some(dep.id.clone()),
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .unwrap();

    let result = db.are_all_dependencies_complete(&legacy).unwrap();
    assert!(result.is_some(), "Legacy fallback should check depends_on_epic_id");
    assert_eq!(result.unwrap().id, dep.id);
}

// ======================================================================
// get_epics_depending_on (JSON LIKE search)
// ======================================================================

#[test]
fn get_epics_depending_on_finds_via_json_field() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
    let done = columns.iter().find(|c| c.name == "Done").unwrap();

    let dep_a = create_epic_ticket(&db, &board.id, &done.id, "A");
    let dep_b = create_epic_ticket(&db, &board.id, &done.id, "B");

    // depends_on_epic_id = dep_a (primary), depends_on_epic_ids = [dep_a, dep_b]
    let dependent = create_epic_with_multi_deps(
        &db,
        &board.id,
        &backlog.id,
        vec![dep_a.id.clone(), dep_b.id.clone()],
    );

    // Search by non-primary dep_b should find via JSON LIKE
    let results = db.get_epics_depending_on(&dep_b.id).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, dependent.id);

    // Search by primary dep_a should also find it
    let results = db.get_epics_depending_on(&dep_a.id).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, dependent.id);
}

#[test]
fn get_epics_depending_on_ignores_non_epic_tickets() {
    let db = create_test_db();
    let board = db.create_board("Board").unwrap();
    let columns = db.get_columns(&board.id).unwrap();
    let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
    let done = columns.iter().find(|c| c.name == "Done").unwrap();

    let dep = create_epic_ticket(&db, &board.id, &done.id, "Dep");

    // Create a non-epic ticket with depends_on_epic_id set
    db.create_ticket(&CreateTicket {
        board_id: board.id.clone(),
        column_id: backlog.id.clone(),
        title: "Not an epic".to_string(),
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
        depends_on_epic_id: Some(dep.id.clone()),
        depends_on_epic_ids: vec![dep.id.clone()],
        spec_version_id: None,
    })
    .unwrap();

    let results = db.get_epics_depending_on(&dep.id).unwrap();
    assert!(results.is_empty(), "Should only return epics, not regular tickets");
}
