//! Epic lifecycle orchestration
//!
//! Handles automatic advancement of epic children and epic state management.
//! Also handles cross-epic dependencies (depends_on_epic_ids).

use super::consolidation::{inject_merge_dependencies_ticket, populate_consolidation_tickets};
use super::TicketState;
use crate::db::{AuthorType, CreateComment, Database, DbError, Ticket};
use std::sync::Arc;

/// Result of epic advancement
#[derive(Debug)]
pub enum EpicAdvancement {
    /// Next child was moved to Ready
    ChildAdvanced { child_id: String },
    /// All children complete, epic moved to Done
    EpicComplete,
    /// Dependent epics were moved to Ready after this epic completed
    DependentsAdvanced { epic_ids: Vec<String> },
    /// Epic cannot start because dependency is not complete
    BlockedByDependency { dependency_id: String },
    /// No action needed
    NoAction,
}

/// Handle epic advancement when moved to Ready.
///
/// When an epic is moved to Ready:
/// 1. Check ALL dependencies (depends_on_epic_ids) are Done
/// 2. If any dependency is not Done, block this epic back to Backlog
/// 3. Guard against advancing a new child when one is already active
/// 4. Otherwise, move its first pending child to Ready
pub fn on_epic_moved_to_ready(
    db: &Arc<Database>,
    epic: &Ticket,
) -> Result<EpicAdvancement, DbError> {
    if !epic.is_epic {
        return Ok(EpicAdvancement::NoAction);
    }

    // Check ALL dependencies, not just the primary one.
    // For multi-dependency epics this prevents starting work when only some
    // dependencies are complete while others are still blocked/in-progress.
    if !epic.depends_on_epic_ids.is_empty() {
        let (all_complete, incomplete_title) = db.are_all_dependencies_complete(epic)?;

        if !all_complete {
            // At least one dependency is not Done -- block the epic back to Backlog
            let display_title = incomplete_title.unwrap_or_else(|| "unknown".to_string());

            if let Some(backlog) = db.find_column_by_name(&epic.board_id, "Backlog")? {
                db.move_ticket(&epic.id, &backlog.id)?;

                db.create_comment(&CreateComment {
                    ticket_id: epic.id.clone(),
                    author_type: AuthorType::System,
                    body_md: format!(
                        "Epic blocked: depends on \"{}\" which is not yet complete. Moved back to Backlog.",
                        display_title
                    ),
                    metadata: None,
                })?;

                tracing::info!(
                    "Epic {} blocked by incomplete dependency \"{}\", moved to Backlog",
                    epic.id,
                    display_title
                );
            } else {
                tracing::warn!(
                    "Epic {} blocked by incomplete dependency but could not find Backlog column",
                    epic.id,
                );
            }

            // Return the first incomplete dependency id for callers that need it
            let blocking_dep_id = epic
                .depends_on_epic_ids
                .iter()
                .find(|dep_id| {
                    // Re-check which one is incomplete (cheap -- small list)
                    db.get_ticket(dep_id)
                        .ok()
                        .and_then(|dep| {
                            db.get_columns(&dep.board_id)
                                .ok()
                                .and_then(|cols| cols.into_iter().find(|c| c.id == dep.column_id))
                                .map(|col| col.name != "Done")
                        })
                        .unwrap_or(true)
                })
                .cloned()
                .unwrap_or_default();

            return Ok(EpicAdvancement::BlockedByDependency {
                dependency_id: blocking_dep_id,
            });
        }
    }

    // Inject or repair merge ticket for multi-dependency epics
    if epic.depends_on_epic_ids.len() > 1 {
        match db.get_merge_dependencies_ticket(&epic.id)? {
            Some(merge_ticket) => {
                // Ticket exists - check if it's properly formed (order_in_epic == 0)
                if merge_ticket.order_in_epic != Some(0) {
                    tracing::warn!(
                        "Epic {}: merge-dependencies ticket {} has order {:?}, repairing to 0",
                        epic.id,
                        merge_ticket.id,
                        merge_ticket.order_in_epic
                    );
                    // Propagate error - if repair fails, we must not continue because
                    // get_next_pending_child would return the wrong ticket, skipping
                    // the merge-dependencies step
                    db.set_ticket_order_in_epic(&merge_ticket.id, 0)?;
                }
            }
            None => {
                // No merge ticket exists - inject one
                if let Err(e) = inject_merge_dependencies_ticket(db, epic) {
                    tracing::warn!(
                        "Epic {}: failed to inject merge dependencies ticket: {}",
                        epic.id,
                        e
                    );
                }
            }
        }
    }

    // If this is a consolidation epic, populate its ticket descriptions with branch info
    if epic.is_consolidation_epic() {
        populate_consolidation_tickets(db, epic)?;
    }

    // Guard: do not advance a second child when one is already active
    // (in Ready, In Progress, Review, or Blocked). This prevents the
    // sequential execution model from being broken when an epic is
    // manually dragged back to Ready.
    if db.has_active_epic_child(&epic.id)? {
        tracing::info!(
            "Epic {}: already has an active child, skipping advancement",
            epic.id
        );
        return Ok(EpicAdvancement::NoAction);
    }

    // Get the next pending child (first child in Backlog)
    if let Some(child) = db.get_next_pending_child(&epic.id)? {
        // Find the Ready column for this board
        if let Some(ready_column) = db.find_column_by_name(&epic.board_id, "Ready")? {
            db.move_ticket(&child.id, &ready_column.id)?;

            tracing::info!("Epic {}: advanced child {} to Ready", epic.id, child.id);

            return Ok(EpicAdvancement::ChildAdvanced { child_id: child.id });
        }
    }

    Ok(EpicAdvancement::NoAction)
}

/// Handle child ticket completion.
///
/// When a child ticket moves to Done, check if there are more children
/// to process. If yes, move the next child to Ready. If no, move the epic to Done.
pub fn on_child_completed(db: &Arc<Database>, child: &Ticket) -> Result<EpicAdvancement, DbError> {
    let Some(epic_id) = &child.epic_id else {
        return Ok(EpicAdvancement::NoAction);
    };

    let epic = db.get_ticket(epic_id)?;

    // Check if all children are done
    if db.are_all_epic_children_done(&epic.id)? {
        // Move epic to Done
        if let Some(done_column) = db.find_column_by_name(&epic.board_id, "Done")? {
            db.move_ticket(&epic.id, &done_column.id)?;

            // Add system comment
            db.create_comment(&CreateComment {
                ticket_id: epic.id.clone(),
                author_type: AuthorType::System,
                body_md: "All child tickets completed. Epic moved to Done.".to_string(),
                metadata: None,
            })?;

            tracing::info!("Epic {} completed - all children done", epic.id);

            // Check for dependent epics that can now be moved to Ready
            let advanced = advance_dependent_epics(db, &epic)?;

            // Check if this epic belongs to a spec and if all spec epics are done
            check_spec_completion(db, &epic)?;

            if !advanced.is_empty() {
                return Ok(EpicAdvancement::DependentsAdvanced { epic_ids: advanced });
            }

            return Ok(EpicAdvancement::EpicComplete);
        }
    } else {
        // Get the next pending child
        if let Some(next_child) = db.get_next_pending_child(&epic.id)? {
            if let Some(ready_column) = db.find_column_by_name(&epic.board_id, "Ready")? {
                db.move_ticket(&next_child.id, &ready_column.id)?;

                tracing::info!(
                    "Epic {}: advanced next child {} to Ready after {} completed",
                    epic.id,
                    next_child.id,
                    child.id
                );

                return Ok(EpicAdvancement::ChildAdvanced {
                    child_id: next_child.id,
                });
            }
        }
    }

    Ok(EpicAdvancement::NoAction)
}

/// When an epic completes, check for other epics that depend on it
/// and move them to Ready if they're in Backlog and ALL of their
/// dependencies are now complete.
///
/// For multi-dependency epics this means we only advance the dependent
/// once every dependency has reached Done -- not when just one of them
/// completes.
pub fn advance_dependent_epics(
    db: &Arc<Database>,
    completed_epic: &Ticket,
) -> Result<Vec<String>, DbError> {
    let mut advanced = Vec::new();

    // Find all epics that depend on this one (checks both primary and
    // the full depends_on_epic_ids_json list).
    let dependents = db.get_epics_depending_on(&completed_epic.id)?;

    for dependent in dependents {
        // Only advance epics that are still in Backlog
        let columns = db.get_columns(&dependent.board_id)?;
        let current_column = columns.iter().find(|c| c.id == dependent.column_id);

        if let Some(col) = current_column {
            if col.name != "Backlog" {
                continue;
            }
        } else {
            continue;
        }

        // For multi-dependency epics, verify ALL dependencies are Done
        // before advancing. If any other dependency is still incomplete
        // the epic will be re-evaluated when that dependency completes.
        let (all_deps_done, _) = db.are_all_dependencies_complete(&dependent)?;
        if !all_deps_done {
            tracing::info!(
                "Epic {} still has incomplete dependencies, not advancing yet",
                dependent.id
            );
            continue;
        }

        // All dependencies satisfied -- move to Ready
        if let Some(ready_column) = db.find_column_by_name(&dependent.board_id, "Ready")? {
            db.move_ticket(&dependent.id, &ready_column.id)?;

            // Add system comment
            db.create_comment(&CreateComment {
                ticket_id: dependent.id.clone(),
                author_type: AuthorType::System,
                body_md: format!(
                    "Dependency \"{}\" completed. Epic moved to Ready.",
                    completed_epic.title
                ),
                metadata: None,
            })?;

            tracing::info!(
                "Epic {} moved to Ready after dependency {} completed",
                dependent.id,
                completed_epic.id
            );

            advanced.push(dependent.id.clone());

            // Also trigger on_epic_moved_to_ready to advance its first child
            let _ = on_epic_moved_to_ready(db, &dependent);
        }
    }

    Ok(advanced)
}

/// Check if all epics for a spec version are complete
/// If so, update the spec version status to Completed
fn check_spec_completion(db: &Arc<Database>, completed_epic: &Ticket) -> Result<(), DbError> {
    // Only check if epic belongs to a spec version
    let Some(ref spec_version_id) = completed_epic.spec_version_id else {
        return Ok(());
    };

    check_spec_completion_by_id(db, spec_version_id)
}

/// Check if all epics for a spec version are complete by version ID
/// If so, update the spec version status to Completed
/// This is public so it can be called from start_spec_work and other places
pub fn check_spec_completion_by_id(db: &Arc<Database>, spec_version_id: &str) -> Result<(), DbError> {
    use crate::db::SpecVersionStatus;
    
    // Check if all spec version epics are done
    if db.are_all_spec_version_epics_done(spec_version_id)? {
        // Get spec version to check current status
        let version = db.get_spec_version(spec_version_id)?;

        // Update if currently in a status that indicates work was in progress
        // This handles edge cases like:
        // - Working: normal completion
        // - Paused: work was paused but all epics completed
        // - Halted: work was halted but all epics completed
        // - Executed: work never started but epics were moved to Done manually
        match version.status {
            SpecVersionStatus::Working
            | SpecVersionStatus::Paused
            | SpecVersionStatus::Halted
            | SpecVersionStatus::Executed => {
                db.set_spec_version_status(spec_version_id, SpecVersionStatus::Completed)?;

                tracing::info!(
                    "Spec version {} completed (from status '{}') - all {} epics done",
                    spec_version_id,
                    version.status.as_str(),
                    db.get_spec_version_epics(spec_version_id)?.len()
                );
            }
            _ => {
                // Already completed, failed, or in an earlier state - don't change
            }
        }
    }

    Ok(())
}

/// Handle child ticket blocked.
///
/// When a child ticket moves to Blocked, move the parent epic to Blocked as well.
pub fn on_child_blocked(db: &Arc<Database>, child: &Ticket) -> Result<(), DbError> {
    let Some(epic_id) = &child.epic_id else {
        return Ok(());
    };

    let epic = db.get_ticket(epic_id)?;

    // Get current epic state
    let epic_column = db
        .get_columns(&epic.board_id)?
        .into_iter()
        .find(|c| c.id == epic.column_id);

    if let Some(col) = epic_column {
        let current_state = TicketState::from_column_name(&col.name);

        // Only block epic if it's not already blocked or done
        if current_state != Some(TicketState::Blocked) && current_state != Some(TicketState::Done) {
            if let Some(blocked_column) = db.find_column_by_name(&epic.board_id, "Blocked")? {
                db.move_ticket(&epic.id, &blocked_column.id)?;

                // Add system comment explaining why
                db.create_comment(&CreateComment {
                    ticket_id: epic.id.clone(),
                    author_type: AuthorType::System,
                    body_md: format!("Epic blocked: child ticket \"{}\" is blocked.", child.title),
                    metadata: None,
                })?;

                tracing::info!(
                    "Epic {} blocked due to child {} being blocked",
                    epic.id,
                    child.id
                );
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{CreateTicket, Priority, WorkflowType};

    fn create_test_db() -> Arc<Database> {
        Arc::new(Database::open_in_memory().unwrap())
    }

    fn create_test_epic(db: &Database, board_id: &str, column_id: &str) -> Ticket {
        db.create_ticket(&CreateTicket {
            board_id: board_id.to_string(),
            column_id: column_id.to_string(),
            title: "Test Epic".to_string(),
            description_md: "Epic description".to_string(),
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

    fn create_test_child(
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
            description_md: "Child description".to_string(),
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

    #[test]
    fn test_epic_advances_first_child_on_ready() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();

        // Create epic in Ready
        let epic = create_test_epic(&db, &board.id, &ready.id);

        // Create children in Backlog
        let child1 = create_test_child(&db, &board.id, &backlog.id, &epic.id, "Child 1");
        let _child2 = create_test_child(&db, &board.id, &backlog.id, &epic.id, "Child 2");

        // Trigger epic advancement
        let result = on_epic_moved_to_ready(&db, &epic).unwrap();

        match result {
            EpicAdvancement::ChildAdvanced { child_id } => {
                assert_eq!(child_id, child1.id);
                // Verify child1 is now in Ready
                let updated_child = db.get_ticket(&child1.id).unwrap();
                assert_eq!(updated_child.column_id, ready.id);
            }
            _ => panic!("Expected ChildAdvanced"),
        }
    }

    #[test]
    fn test_epic_completes_when_all_children_done() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
        let done = columns.iter().find(|c| c.name == "Done").unwrap();

        // Create epic in Ready
        let epic = create_test_epic(&db, &board.id, &ready.id);

        // Create single child in Done
        let child = create_test_child(&db, &board.id, &done.id, &epic.id, "Only Child");

        // Trigger child completion handling
        let result = on_child_completed(&db, &child).unwrap();

        match result {
            EpicAdvancement::EpicComplete => {
                // Verify epic is now in Done
                let updated_epic = db.get_ticket(&epic.id).unwrap();
                assert_eq!(updated_epic.column_id, done.id);
            }
            _ => panic!("Expected EpicComplete"),
        }
    }

    #[test]
    fn test_epic_advances_next_child_after_completion() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
        let done = columns.iter().find(|c| c.name == "Done").unwrap();

        // Create epic in Ready
        let epic = create_test_epic(&db, &board.id, &ready.id);

        // Create children: one done, one in backlog
        let child1 = create_test_child(&db, &board.id, &done.id, &epic.id, "Child 1");
        let child2 = create_test_child(&db, &board.id, &backlog.id, &epic.id, "Child 2");

        // Trigger child1 completion handling
        let result = on_child_completed(&db, &child1).unwrap();

        match result {
            EpicAdvancement::ChildAdvanced { child_id } => {
                assert_eq!(child_id, child2.id);
                // Verify child2 is now in Ready
                let updated_child2 = db.get_ticket(&child2.id).unwrap();
                assert_eq!(updated_child2.column_id, ready.id);
            }
            _ => panic!("Expected ChildAdvanced"),
        }
    }

    #[test]
    fn test_epic_blocks_when_child_blocked() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
        let blocked = columns.iter().find(|c| c.name == "Blocked").unwrap();

        // Create epic in Ready
        let epic = create_test_epic(&db, &board.id, &ready.id);

        // Create child in Blocked
        let child = create_test_child(&db, &board.id, &blocked.id, &epic.id, "Blocked Child");

        // Trigger child blocked handling
        on_child_blocked(&db, &child).unwrap();

        // Verify epic is now in Blocked
        let updated_epic = db.get_ticket(&epic.id).unwrap();
        assert_eq!(updated_epic.column_id, blocked.id);
    }

    fn create_epic_with_dependency(
        db: &Database,
        board_id: &str,
        column_id: &str,
        depends_on: &str,
    ) -> Ticket {
        db.create_ticket(&CreateTicket {
            board_id: board_id.to_string(),
            column_id: column_id.to_string(),
            title: "Dependent Epic".to_string(),
            description_md: "Epic with dependency".to_string(),
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

    #[test]
    fn test_epic_blocked_when_dependency_not_complete() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();

        // Create first epic (the dependency) in Ready (not Done)
        let dependency_epic = create_test_epic(&db, &board.id, &ready.id);

        // Create second epic that depends on the first, try to move to Ready
        let dependent_epic =
            create_epic_with_dependency(&db, &board.id, &ready.id, &dependency_epic.id);

        // Create a child for the dependent epic (in Backlog)
        create_test_child(&db, &board.id, &backlog.id, &dependent_epic.id, "Child");

        // Trigger epic advancement - should be blocked because dependency not in Done
        let result = on_epic_moved_to_ready(&db, &dependent_epic).unwrap();

        match result {
            EpicAdvancement::BlockedByDependency { dependency_id } => {
                assert_eq!(dependency_id, dependency_epic.id);
                // Verify dependent epic was moved back to Backlog
                let updated = db.get_ticket(&dependent_epic.id).unwrap();
                assert_eq!(updated.column_id, backlog.id);
            }
            _ => panic!("Expected BlockedByDependency, got {:?}", result),
        }
    }

    #[test]
    fn test_epic_proceeds_when_dependency_complete() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
        let done = columns.iter().find(|c| c.name == "Done").unwrap();

        // Create first epic (the dependency) in Done
        let dependency_epic = create_test_epic(&db, &board.id, &done.id);

        // Create second epic that depends on the first, in Ready
        let dependent_epic =
            create_epic_with_dependency(&db, &board.id, &ready.id, &dependency_epic.id);

        // Create a child for the dependent epic (in Backlog)
        let child = create_test_child(&db, &board.id, &backlog.id, &dependent_epic.id, "Child");

        // Trigger epic advancement - should proceed since dependency is Done
        let result = on_epic_moved_to_ready(&db, &dependent_epic).unwrap();

        match result {
            EpicAdvancement::ChildAdvanced { child_id } => {
                assert_eq!(child_id, child.id);
                // Verify child was moved to Ready
                let updated_child = db.get_ticket(&child.id).unwrap();
                assert_eq!(updated_child.column_id, ready.id);
            }
            _ => panic!("Expected ChildAdvanced, got {:?}", result),
        }
    }

    #[test]
    fn test_advance_dependent_epics_moves_to_ready() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
        let done = columns.iter().find(|c| c.name == "Done").unwrap();

        // Create first epic that will complete
        let first_epic = create_test_epic(&db, &board.id, &done.id);

        // Create second epic that depends on the first, in Backlog
        let dependent_epic =
            create_epic_with_dependency(&db, &board.id, &backlog.id, &first_epic.id);

        // Trigger advance_dependent_epics
        let advanced = advance_dependent_epics(&db, &first_epic).unwrap();

        // Should have advanced the dependent epic
        assert_eq!(advanced.len(), 1);
        assert_eq!(advanced[0], dependent_epic.id);

        // Verify dependent epic is now in Ready
        let updated = db.get_ticket(&dependent_epic.id).unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
        assert_eq!(updated.column_id, ready.id);
    }

    #[test]
    fn test_advance_dependent_epics_ignores_non_backlog() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
        let done = columns.iter().find(|c| c.name == "Done").unwrap();

        // Create first epic that completes
        let first_epic = create_test_epic(&db, &board.id, &done.id);

        // Create second epic already in Ready (not Backlog)
        let dependent_epic = create_epic_with_dependency(&db, &board.id, &ready.id, &first_epic.id);

        // Trigger advance_dependent_epics
        let advanced = advance_dependent_epics(&db, &first_epic).unwrap();

        // Should NOT have advanced (already in Ready)
        assert!(advanced.is_empty());

        // Verify dependent epic is still in Ready
        let updated = db.get_ticket(&dependent_epic.id).unwrap();
        assert_eq!(updated.column_id, ready.id);
    }

    fn create_epic_with_multi_dependencies(
        db: &Database,
        board_id: &str,
        column_id: &str,
        dep_ids: Vec<String>,
    ) -> Ticket {
        db.create_ticket(&CreateTicket {
            board_id: board_id.to_string(),
            column_id: column_id.to_string(),
            title: "Multi-Dep Epic".to_string(),
            description_md: "Epic with multiple dependencies".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
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
    fn test_multi_dependency_epic_injects_merge_ticket() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
        let done = columns.iter().find(|c| c.name == "Done").unwrap();

        // Create two dependency epics in Done with children that have branches
        let dep1 = create_test_epic(&db, &board.id, &done.id);
        let dep2 = create_test_epic(&db, &board.id, &done.id);

        // Add completed children with branches to each
        db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: done.id.clone(),
            title: "Dep1 Child".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: Some("feat/dep1-branch".to_string()),
            is_epic: false,
            epic_id: Some(dep1.id.clone()),
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .unwrap();

        db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: done.id.clone(),
            title: "Dep2 Child".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: Some("feat/dep2-branch".to_string()),
            is_epic: false,
            epic_id: Some(dep2.id.clone()),
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .unwrap();

        // Create epic with multiple dependencies in Ready
        let multi_dep = create_epic_with_multi_dependencies(
            &db,
            &board.id,
            &ready.id,
            vec![dep1.id.clone(), dep2.id.clone()],
        );

        // Add an existing child
        let _existing_child =
            create_test_child(&db, &board.id, &backlog.id, &multi_dep.id, "Existing Child");

        // Trigger on_epic_moved_to_ready
        let _ = on_epic_moved_to_ready(&db, &multi_dep);

        // Verify merge-dependencies ticket was injected
        assert!(db.has_merge_dependencies_ticket(&multi_dep.id).unwrap());

        // Get children and verify merge ticket is first
        let children = db.get_epic_children(&multi_dep.id).unwrap();
        assert!(children.len() >= 2);

        let merge_ticket = &children[0];
        assert!(merge_ticket
            .labels
            .contains(&"merge-dependencies".to_string()));
        assert_eq!(merge_ticket.order_in_epic, Some(0));
        assert!(merge_ticket
            .title
            .contains("Merge dependency branches"));
    }

    #[test]
    fn test_multi_dependency_epic_idempotent_merge_ticket() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
        let done = columns.iter().find(|c| c.name == "Done").unwrap();

        // Create two dependency epics in Done with children that have branches
        let dep1 = create_test_epic(&db, &board.id, &done.id);
        let dep2 = create_test_epic(&db, &board.id, &done.id);

        db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: done.id.clone(),
            title: "Child".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: Some("feat/branch".to_string()),
            is_epic: false,
            epic_id: Some(dep1.id.clone()),
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .unwrap();

        db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: done.id.clone(),
            title: "Child2".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: Some("feat/branch2".to_string()),
            is_epic: false,
            epic_id: Some(dep2.id.clone()),
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .unwrap();

        let multi_dep = create_epic_with_multi_dependencies(
            &db,
            &board.id,
            &ready.id,
            vec![dep1.id.clone(), dep2.id.clone()],
        );

        // Call twice - should only inject once
        let _ = on_epic_moved_to_ready(&db, &multi_dep);
        let _ = on_epic_moved_to_ready(&db, &multi_dep);

        // Count merge-dependencies tickets
        let children = db.get_epic_children(&multi_dep.id).unwrap();
        let merge_count = children
            .iter()
            .filter(|c| c.labels.contains(&"merge-dependencies".to_string()))
            .count();

        assert_eq!(merge_count, 1, "Should only have one merge ticket");
    }

    #[test]
    fn test_single_dependency_epic_no_merge_ticket() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
        let done = columns.iter().find(|c| c.name == "Done").unwrap();

        // Create single dependency epic
        let dep = create_test_epic(&db, &board.id, &done.id);

        // Create epic with single dependency (should NOT inject merge ticket)
        let single_dep = create_epic_with_dependency(&db, &board.id, &ready.id, &dep.id);

        // Add a child
        create_test_child(&db, &board.id, &backlog.id, &single_dep.id, "Child");

        // Trigger on_epic_moved_to_ready
        let _ = on_epic_moved_to_ready(&db, &single_dep);

        // Should NOT have merge-dependencies ticket
        assert!(!db.has_merge_dependencies_ticket(&single_dep.id).unwrap());
    }

    #[test]
    fn test_malformed_merge_ticket_is_repaired() {
        // This test verifies that if a merge-dependencies ticket exists with the wrong
        // order_in_epic (e.g., due to a partial failure during injection), it gets repaired
        // when on_epic_moved_to_ready is called again.
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
        let done = columns.iter().find(|c| c.name == "Done").unwrap();

        // Create two dependency epics in Done with children that have branches
        let dep1 = create_test_epic(&db, &board.id, &done.id);
        let dep2 = create_test_epic(&db, &board.id, &done.id);

        db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: done.id.clone(),
            title: "Dep1 Child".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: Some("feat/dep1-branch".to_string()),
            is_epic: false,
            epic_id: Some(dep1.id.clone()),
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .unwrap();

        db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: done.id.clone(),
            title: "Dep2 Child".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: Some("feat/dep2-branch".to_string()),
            is_epic: false,
            epic_id: Some(dep2.id.clone()),
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .unwrap();

        // Create epic with multiple dependencies in Ready
        let multi_dep = create_epic_with_multi_dependencies(
            &db,
            &board.id,
            &ready.id,
            vec![dep1.id.clone(), dep2.id.clone()],
        );

        // Manually create a MALFORMED merge-dependencies ticket (wrong order)
        let malformed_merge = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: backlog.id.clone(),
                title: "Merge dependency branches".to_string(),
                description_md: "".to_string(),
                priority: Priority::High,
                labels: vec![
                    "auto-generated".to_string(),
                    "merge-dependencies".to_string(),
                ],
                project_id: None,
                workflow_type: WorkflowType::MultiStage,
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: Some(multi_dep.id.clone()),
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: None,
            })
            .unwrap();

        // Set it to a WRONG order (simulating partial failure)
        db.set_ticket_order_in_epic(&malformed_merge.id, 5).unwrap();

        // Verify it has wrong order
        let ticket_before = db.get_ticket(&malformed_merge.id).unwrap();
        assert_eq!(
            ticket_before.order_in_epic,
            Some(5),
            "Ticket should have wrong order before repair"
        );

        // Call on_epic_moved_to_ready - should repair the malformed ticket
        let _ = on_epic_moved_to_ready(&db, &multi_dep);

        // Verify the ticket now has correct order
        let ticket_after = db.get_ticket(&malformed_merge.id).unwrap();
        assert_eq!(
            ticket_after.order_in_epic,
            Some(0),
            "Malformed merge-dependencies ticket should be repaired to order 0"
        );

        // Verify only one merge-dependencies ticket exists (no duplicate created)
        let children = db.get_epic_children(&multi_dep.id).unwrap();
        let merge_count = children
            .iter()
            .filter(|c| c.labels.contains(&"merge-dependencies".to_string()))
            .count();
        assert_eq!(
            merge_count, 1,
            "Should still have exactly one merge ticket after repair"
        );
    }

    // ======================================================================
    // Bug-fix tests: multi-dependency checking
    // ======================================================================

    #[test]
    fn test_multi_dep_epic_blocked_when_non_primary_dep_incomplete() {
        // When an epic depends on [A, B] and only A is Done (B is still in
        // Ready), moving the dependent to Ready should block it back to
        // Backlog.
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
        let done = columns.iter().find(|c| c.name == "Done").unwrap();

        // Dep A is Done, dep B is still in Ready (incomplete)
        let dep_a = create_test_epic(&db, &board.id, &done.id);
        let dep_b = create_test_epic(&db, &board.id, &ready.id);

        // Create the multi-dep epic in Ready
        let multi_dep = create_epic_with_multi_dependencies(
            &db,
            &board.id,
            &ready.id,
            vec![dep_a.id.clone(), dep_b.id.clone()],
        );

        // Add a child so there's something to advance
        create_test_child(&db, &board.id, &backlog.id, &multi_dep.id, "Child");

        let result = on_epic_moved_to_ready(&db, &multi_dep).unwrap();

        match result {
            EpicAdvancement::BlockedByDependency { dependency_id } => {
                // Should report dep_b as the blocking dependency
                assert_eq!(dependency_id, dep_b.id);
                // Epic should be moved back to Backlog
                let updated = db.get_ticket(&multi_dep.id).unwrap();
                assert_eq!(updated.column_id, backlog.id);
            }
            _ => panic!(
                "Expected BlockedByDependency, got {:?}",
                result
            ),
        }
    }

    #[test]
    fn test_multi_dep_epic_proceeds_when_all_deps_done() {
        // When ALL dependencies are Done, the multi-dep epic should proceed.
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
        let done = columns.iter().find(|c| c.name == "Done").unwrap();

        let dep_a = create_test_epic(&db, &board.id, &done.id);
        let dep_b = create_test_epic(&db, &board.id, &done.id);

        let multi_dep = create_epic_with_multi_dependencies(
            &db,
            &board.id,
            &ready.id,
            vec![dep_a.id.clone(), dep_b.id.clone()],
        );

        let child = create_test_child(&db, &board.id, &backlog.id, &multi_dep.id, "Child");

        let result = on_epic_moved_to_ready(&db, &multi_dep).unwrap();

        match result {
            EpicAdvancement::ChildAdvanced { child_id } => {
                assert_eq!(child_id, child.id);
            }
            _ => panic!("Expected ChildAdvanced, got {:?}", result),
        }
    }

    #[test]
    fn test_advance_dependent_waits_for_all_deps() {
        // advance_dependent_epics should NOT advance an epic when only one
        // of its multiple dependencies has completed.
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
        let done = columns.iter().find(|c| c.name == "Done").unwrap();

        // A is Done, B is still in Ready
        let dep_a = create_test_epic(&db, &board.id, &done.id);
        let dep_b = create_test_epic(&db, &board.id, &ready.id);

        // Epic C depends on [A, B]
        let epic_c = create_epic_with_multi_dependencies(
            &db,
            &board.id,
            &backlog.id,
            vec![dep_a.id.clone(), dep_b.id.clone()],
        );

        // A completes -- try to advance dependents
        let advanced = advance_dependent_epics(&db, &dep_a).unwrap();

        // Should NOT have advanced because B is not Done
        assert!(
            advanced.is_empty(),
            "Should not advance when B is incomplete"
        );

        // Epic C should still be in Backlog
        let updated_c = db.get_ticket(&epic_c.id).unwrap();
        assert_eq!(updated_c.column_id, backlog.id);
    }

    #[test]
    fn test_advance_dependent_proceeds_when_all_deps_done() {
        // advance_dependent_epics should advance once the last dependency
        // completes and all are Done.
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
        let done = columns.iter().find(|c| c.name == "Done").unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();

        let dep_a = create_test_epic(&db, &board.id, &done.id);
        let dep_b = create_test_epic(&db, &board.id, &done.id);

        let epic_c = create_epic_with_multi_dependencies(
            &db,
            &board.id,
            &backlog.id,
            vec![dep_a.id.clone(), dep_b.id.clone()],
        );

        // B completes (A was already done) -- both deps are now Done
        let advanced = advance_dependent_epics(&db, &dep_b).unwrap();

        assert_eq!(advanced.len(), 1);
        assert_eq!(advanced[0], epic_c.id);

        let updated_c = db.get_ticket(&epic_c.id).unwrap();
        assert_eq!(updated_c.column_id, ready.id);
    }

    #[test]
    fn test_non_primary_dep_completion_finds_dependent() {
        // When the non-primary dependency completes, get_epics_depending_on
        // should still find the dependent epic via depends_on_epic_ids_json.
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
        let done = columns.iter().find(|c| c.name == "Done").unwrap();

        let dep_a = create_test_epic(&db, &board.id, &done.id);
        let dep_b = create_test_epic(&db, &board.id, &done.id);

        // depends_on_epic_id = dep_a (primary), depends_on_epic_ids = [dep_a, dep_b]
        let _epic_c = create_epic_with_multi_dependencies(
            &db,
            &board.id,
            &backlog.id,
            vec![dep_a.id.clone(), dep_b.id.clone()],
        );

        // Querying by dep_b (non-primary) should find the dependent
        let dependents = db.get_epics_depending_on(&dep_b.id).unwrap();
        assert_eq!(dependents.len(), 1, "Should find dependent via depends_on_epic_ids_json");
    }

    // ======================================================================
    // Bug-fix tests: active child guard
    // ======================================================================

    #[test]
    fn test_epic_does_not_advance_second_child_when_one_active() {
        // When an epic is dragged back to Ready while a child is already
        // in Ready / In Progress / Review, no additional child should be
        // moved to Ready.
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
        let in_progress = columns.iter().find(|c| c.name == "In Progress").unwrap();

        let epic = create_test_epic(&db, &board.id, &ready.id);

        // Child 1 is already in progress
        let _child1 = create_test_child(&db, &board.id, &in_progress.id, &epic.id, "Child 1");
        // Child 2 is still waiting
        let child2 = create_test_child(&db, &board.id, &backlog.id, &epic.id, "Child 2");

        let result = on_epic_moved_to_ready(&db, &epic).unwrap();

        // Should NOT advance child2 because child1 is active
        match result {
            EpicAdvancement::NoAction => {
                // Verify child2 is still in Backlog
                let updated = db.get_ticket(&child2.id).unwrap();
                assert_eq!(updated.column_id, backlog.id);
            }
            _ => panic!(
                "Expected NoAction (active child guard), got {:?}",
                result
            ),
        }
    }

    #[test]
    fn test_epic_does_not_advance_when_child_in_review() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
        let review = columns.iter().find(|c| c.name == "Review").unwrap();

        let epic = create_test_epic(&db, &board.id, &ready.id);

        let _child1 = create_test_child(&db, &board.id, &review.id, &epic.id, "Child 1");
        let child2 = create_test_child(&db, &board.id, &backlog.id, &epic.id, "Child 2");

        let result = on_epic_moved_to_ready(&db, &epic).unwrap();

        match result {
            EpicAdvancement::NoAction => {
                let updated = db.get_ticket(&child2.id).unwrap();
                assert_eq!(updated.column_id, backlog.id);
            }
            _ => panic!("Expected NoAction, got {:?}", result),
        }
    }

    #[test]
    fn test_epic_does_not_advance_when_child_blocked() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
        let blocked = columns.iter().find(|c| c.name == "Blocked").unwrap();

        let epic = create_test_epic(&db, &board.id, &ready.id);

        let _child1 = create_test_child(&db, &board.id, &blocked.id, &epic.id, "Blocked Child");
        let child2 = create_test_child(&db, &board.id, &backlog.id, &epic.id, "Child 2");

        let result = on_epic_moved_to_ready(&db, &epic).unwrap();

        match result {
            EpicAdvancement::NoAction => {
                let updated = db.get_ticket(&child2.id).unwrap();
                assert_eq!(updated.column_id, backlog.id);
            }
            _ => panic!("Expected NoAction, got {:?}", result),
        }
    }

    #[test]
    fn test_epic_advances_child_when_all_inactive() {
        // When all existing children are in Backlog or Done, advancing
        // should work normally.
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();
        let done = columns.iter().find(|c| c.name == "Done").unwrap();

        let epic = create_test_epic(&db, &board.id, &ready.id);

        // Child 1 already done, child 2 still in backlog
        let _child1 = create_test_child(&db, &board.id, &done.id, &epic.id, "Child 1");
        let child2 = create_test_child(&db, &board.id, &backlog.id, &epic.id, "Child 2");

        let result = on_epic_moved_to_ready(&db, &epic).unwrap();

        match result {
            EpicAdvancement::ChildAdvanced { child_id } => {
                assert_eq!(child_id, child2.id);
                let updated = db.get_ticket(&child2.id).unwrap();
                assert_eq!(updated.column_id, ready.id);
            }
            _ => panic!("Expected ChildAdvanced, got {:?}", result),
        }
    }
}
