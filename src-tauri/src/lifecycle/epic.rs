//! Epic lifecycle orchestration
//!
//! Handles automatic advancement of epic children and epic state management.

use std::sync::Arc;
use crate::db::{Database, DbError, Ticket, AuthorType, CreateComment};
use super::TicketState;

/// Result of epic advancement
#[derive(Debug)]
pub enum EpicAdvancement {
    /// Next child was moved to Ready
    ChildAdvanced { child_id: String },
    /// All children complete, epic moved to Done
    EpicComplete,
    /// No action needed
    NoAction,
}

/// Handle epic advancement when moved to Ready.
/// 
/// When an epic is moved to Ready, move its first pending child to Ready.
pub fn on_epic_moved_to_ready(
    db: &Arc<Database>,
    epic: &Ticket,
) -> Result<EpicAdvancement, DbError> {
    if !epic.is_epic {
        return Ok(EpicAdvancement::NoAction);
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
}
