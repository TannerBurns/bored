//! Epic lifecycle orchestration
//!
//! Handles automatic advancement of epic children and epic state management.
//! Also handles cross-epic dependencies (depends_on_epic_id).

use std::sync::Arc;
use crate::db::{Database, DbError, Ticket, AuthorType, CreateComment, ScratchpadStatus, UpdateTicket};
use super::TicketState;

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
/// 1. Check if this epic has a dependency (depends_on_epic_id)
/// 2. If dependency exists and is not Done, block this epic
/// 3. Otherwise, move its first pending child to Ready
pub fn on_epic_moved_to_ready(
    db: &Arc<Database>,
    epic: &Ticket,
) -> Result<EpicAdvancement, DbError> {
    if !epic.is_epic {
        return Ok(EpicAdvancement::NoAction);
    }

    // Check if this epic has a dependency
    if let Some(ref dependency_id) = epic.depends_on_epic_id {
        // Check if the dependency epic is in Done
        let dependency = db.get_ticket(dependency_id)?;
        let dep_column = db.get_columns(&dependency.board_id)?
            .into_iter()
            .find(|c| c.id == dependency.column_id);
        
        // Determine if dependency is complete
        // If column lookup fails, treat as incomplete (fail-safe: block the epic)
        let dependency_complete = match dep_column {
            Some(ref col) => col.name == "Done",
            None => {
                tracing::warn!(
                    "Epic {}: could not find column {} for dependency {}, treating as incomplete",
                    epic.id, dependency.column_id, dependency_id
                );
                false
            }
        };
        
        if !dependency_complete {
            // Dependency not complete - try to move epic to Backlog, but always block
            // regardless of whether the column lookup succeeds
            if let Some(backlog) = db.find_column_by_name(&epic.board_id, "Backlog")? {
                db.move_ticket(&epic.id, &backlog.id)?;
                
                // Add system comment
                db.create_comment(&CreateComment {
                    ticket_id: epic.id.clone(),
                    author_type: AuthorType::System,
                    body_md: format!(
                        "Epic blocked: depends on \"{}\" which is not yet complete. Moved back to Backlog.",
                        dependency.title
                    ),
                    metadata: None,
                })?;
                
                tracing::info!(
                    "Epic {} blocked by dependency {}, moved to Backlog",
                    epic.id, dependency_id
                );
            } else {
                tracing::warn!(
                    "Epic {} blocked by dependency {} but could not find Backlog column to move it",
                    epic.id, dependency_id
                );
            }
            
            // Always return BlockedByDependency when dependency is incomplete,
            // regardless of whether we could move the epic to Backlog
            return Ok(EpicAdvancement::BlockedByDependency { 
                dependency_id: dependency_id.clone() 
            });
        }
    }

    // If this is a consolidation epic, populate its ticket descriptions with branch info
    if epic.is_consolidation_epic() {
        populate_consolidation_tickets(db, epic)?;
    }

    // Get the next pending child (first child in Backlog)
    if let Some(child) = db.get_next_pending_child(&epic.id)? {
        // Find the Ready column for this board
        if let Some(ready_column) = db.find_column_by_name(&epic.board_id, "Ready")? {
            db.move_ticket(&child.id, &ready_column.id)?;
            
            tracing::info!(
                "Epic {}: advanced child {} to Ready",
                epic.id, child.id
            );
            
            return Ok(EpicAdvancement::ChildAdvanced { child_id: child.id });
        }
    }

    Ok(EpicAdvancement::NoAction)
}

/// Populate consolidation epic ticket descriptions with branch merge instructions.
/// Called when a consolidation epic moves to Ready (all dependencies complete).
fn populate_consolidation_tickets(
    db: &Arc<Database>,
    epic: &Ticket,
) -> Result<(), DbError> {
    let Some(ref scratchpad_id) = epic.scratchpad_id else {
        tracing::warn!(
            "Consolidation epic {} has no scratchpad_id, cannot populate branch info",
            epic.id
        );
        return Ok(());
    };

    // Get all epics from the scratchpad with their final branches
    let epics_with_branches = db.get_scratchpad_epics_with_branches(scratchpad_id)?;
    
    if epics_with_branches.is_empty() {
        tracing::warn!(
            "Consolidation epic {}: no epics with branches found in scratchpad {}",
            epic.id, scratchpad_id
        );
        return Ok(());
    }

    // Build the merge instructions markdown
    let mut merge_steps = Vec::new();
    merge_steps.push("## Branch Consolidation Task\n".to_string());
    merge_steps.push("Create a consolidation branch and merge all epic work sequentially.\n".to_string());
    merge_steps.push("### Steps:\n".to_string());
    merge_steps.push(format!("1. Create new branch from main: `scratchpad/{}/consolidated`\n", scratchpad_id));
    
    let mut step = 2;
    for (epic_id, epic_title, branch) in &epics_with_branches {
        if let Some(branch_name) = branch {
            merge_steps.push(format!(
                "{}. Merge branch `{}` (from Epic: {})\n   - Resolve any merge conflicts\n",
                step, branch_name, epic_title
            ));
            step += 1;
        } else {
            tracing::warn!(
                "Consolidation epic {}: epic {} ({}) has no final branch",
                epic.id, epic_id, epic_title
            );
        }
    }
    
    merge_steps.push(format!("{}. Verify all tests pass\n", step));
    merge_steps.push(format!("{}. Push the consolidated branch\n", step + 1));
    
    merge_steps.push("\n### Epics to Merge:\n".to_string());
    for (_, epic_title, branch) in &epics_with_branches {
        let branch_info = branch.as_ref().map(|b| format!(" → `{}`", b)).unwrap_or_default();
        merge_steps.push(format!("- {}{}\n", epic_title, branch_info));
    }

    let description = merge_steps.join("");

    // Update all children of the consolidation epic with the merge instructions
    let children = db.get_epic_children(&epic.id)?;
    for child in children {
        db.update_ticket(&child.id, &UpdateTicket {
            description_md: Some(description.clone()),
            ..Default::default()
        })?;
        
        tracing::info!(
            "Consolidation epic {}: updated child {} with branch merge instructions",
            epic.id, child.id
        );
    }

    // Add system comment to the consolidation epic
    db.create_comment(&CreateComment {
        ticket_id: epic.id.clone(),
        author_type: AuthorType::System,
        body_md: format!(
            "Consolidation epic ready. {} epic branches to merge:\n{}",
            epics_with_branches.len(),
            epics_with_branches.iter()
                .filter_map(|(_, title, branch)| branch.as_ref().map(|b| format!("- {} → `{}`", title, b)))
                .collect::<Vec<_>>()
                .join("\n")
        ),
        metadata: None,
    })?;

    Ok(())
}

/// Handle child ticket completion.
/// 
/// When a child ticket moves to Done, check if there are more children
/// to process. If yes, move the next child to Ready. If no, move the epic to Done.
pub fn on_child_completed(
    db: &Arc<Database>,
    child: &Ticket,
) -> Result<EpicAdvancement, DbError> {
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
            
            // Check if this epic belongs to a scratchpad and if all scratchpad epics are done
            check_scratchpad_completion(db, &epic)?;
            
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
                    epic.id, next_child.id, child.id
                );
                
                return Ok(EpicAdvancement::ChildAdvanced { child_id: next_child.id });
            }
        }
    }

    Ok(EpicAdvancement::NoAction)
}

/// When an epic completes, check for other epics that depend on it
/// and move them to Ready if they're in Backlog.
pub fn advance_dependent_epics(
    db: &Arc<Database>,
    completed_epic: &Ticket,
) -> Result<Vec<String>, DbError> {
    let mut advanced = Vec::new();
    
    // Find all epics that depend on this one
    let dependents = db.get_epics_depending_on(&completed_epic.id)?;
    
    for dependent in dependents {
        // Check if it's in Backlog
        let columns = db.get_columns(&dependent.board_id)?;
        let current_column = columns.iter().find(|c| c.id == dependent.column_id);
        
        if let Some(col) = current_column {
            if col.name == "Backlog" {
                // Move to Ready
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
                        dependent.id, completed_epic.id
                    );
                    
                    advanced.push(dependent.id.clone());
                    
                    // Also trigger on_epic_moved_to_ready to advance its first child
                    let _ = on_epic_moved_to_ready(db, &dependent);
                }
            }
        }
    }
    
    Ok(advanced)
}

/// Check if all epics for a scratchpad are complete
/// If so, update the scratchpad status to Completed
fn check_scratchpad_completion(
    db: &Arc<Database>,
    completed_epic: &Ticket,
) -> Result<(), DbError> {
    // Only check if epic belongs to a scratchpad
    let Some(ref scratchpad_id) = completed_epic.scratchpad_id else {
        return Ok(());
    };
    
    // Check if all scratchpad epics are done
    if db.are_all_scratchpad_epics_done(scratchpad_id)? {
        // Get scratchpad to check current status
        let scratchpad = db.get_scratchpad(scratchpad_id)?;
        
        // Only update if currently in Working status
        if scratchpad.status == ScratchpadStatus::Working {
            db.set_scratchpad_status(scratchpad_id, ScratchpadStatus::Completed)?;
            
            tracing::info!(
                "Scratchpad {} completed - all {} epics done",
                scratchpad_id,
                db.get_scratchpad_epics(scratchpad_id)?.len()
            );
        }
    }
    
    Ok(())
}

/// Handle child ticket blocked.
/// 
/// When a child ticket moves to Blocked, move the parent epic to Blocked as well.
pub fn on_child_blocked(
    db: &Arc<Database>,
    child: &Ticket,
) -> Result<(), DbError> {
    let Some(epic_id) = &child.epic_id else {
        return Ok(());
    };

    let epic = db.get_ticket(epic_id)?;
    
    // Get current epic state
    let epic_column = db.get_columns(&epic.board_id)?
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
                    body_md: format!(
                        "Epic blocked: child ticket \"{}\" is blocked.",
                        child.title
                    ),
                    metadata: None,
                })?;
                
                tracing::info!(
                    "Epic {} blocked due to child {} being blocked",
                    epic.id, child.id
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
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: true,
            epic_id: None,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            scratchpad_id: None,
        }).unwrap()
    }

    fn create_test_child(db: &Database, board_id: &str, column_id: &str, epic_id: &str, title: &str) -> Ticket {
        db.create_ticket(&CreateTicket {
            board_id: board_id.to_string(),
            column_id: column_id.to_string(),
            title: title.to_string(),
            description_md: "Child description".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: Some(epic_id.to_string()),
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            scratchpad_id: None,
        }).unwrap()
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

    fn create_epic_with_dependency(db: &Database, board_id: &str, column_id: &str, depends_on: &str) -> Ticket {
        db.create_ticket(&CreateTicket {
            board_id: board_id.to_string(),
            column_id: column_id.to_string(),
            title: "Dependent Epic".to_string(),
            description_md: "Epic with dependency".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            agent_pref: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: true,
            epic_id: None,
            depends_on_epic_id: Some(depends_on.to_string()),
            depends_on_epic_ids: vec![depends_on.to_string()],
            scratchpad_id: None,
        }).unwrap()
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
        let dependent_epic = create_epic_with_dependency(&db, &board.id, &ready.id, &dependency_epic.id);
        
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
        let dependent_epic = create_epic_with_dependency(&db, &board.id, &ready.id, &dependency_epic.id);
        
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
        let dependent_epic = create_epic_with_dependency(&db, &board.id, &backlog.id, &first_epic.id);

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
}
