//! Branch selection logic for epic chain branching.
//!
//! Handles determining the base branch for tickets based on:
//! - Previous sibling branches (chain branching within an epic)
//! - Cross-epic dependency branches
//! - Merge-dependencies tickets (which branch from main)
//!
//! Epic chain branching (sibling + cross-epic) applies only to tickets linked to a spec
//! version (`spec_version_id`). Other epic children use the default base (e.g. main).

use std::sync::Arc;

use crate::db::{Database, Ticket};

#[cfg(test)]
use crate::db::models::{CreateProject, CreateSpec, UpdateTicket};
#[cfg(test)]
use crate::db::{CreateTicket, Priority, WorkflowType};

fn is_merge_dependencies_ticket(ticket: &Ticket) -> bool {
    // Only check the label, not order_in_epic. During injection, merge-dependencies
    // tickets may temporarily have non-zero order before repair logic fixes it.
    // The label is the definitive marker for these tickets.
    ticket.labels.contains(&"merge-dependencies".to_string())
}

/// Determine the base branch for a ticket based on epic chain branching rules.
/// Merge-dependencies tickets branch from main. Other epic children chain from siblings
/// or dependency epics only when `spec_version_id` is set; otherwise the default base is used.
pub(crate) fn get_base_branch_for_ticket(
    db: &Arc<Database>,
    ticket: &Ticket,
    caller_id: &str,
) -> Option<String> {
    ticket.epic_id.as_ref()?;

    if is_merge_dependencies_ticket(ticket) {
        tracing::info!(
            "{} ticket {} is a merge-dependencies ticket, using default branch",
            caller_id,
            ticket.id
        );
        return None;
    }

    if ticket.spec_version_id.is_none() {
        tracing::info!(
            "{} ticket {} has no spec_version_id; skipping epic chain branching (default base)",
            caller_id,
            ticket.id
        );
        return None;
    }

    match db.get_previous_epic_sibling(&ticket.id) {
        Ok(Some(prev_sibling)) => {
            if let Some(ref branch) = prev_sibling.branch_name {
                tracing::info!(
                    "{} using chain branching: basing {} on previous sibling's branch {}",
                    caller_id, ticket.id, branch
                );
                Some(branch.clone())
            } else {
                tracing::info!(
                    "{} previous sibling {} has no branch yet, using default branch",
                    caller_id, prev_sibling.id
                );
                None
            }
        }
        Ok(None) => {
            get_cross_epic_base_branch(db, ticket, caller_id)
        }
        Err(e) => {
            tracing::warn!(
                "{} failed to get previous sibling for {}: {}, using default branch",
                caller_id, ticket.id, e
            );
            None
        }
    }
}

/// Check for cross-epic dependency branching when ticket is the first child in its epic.
fn get_cross_epic_base_branch(
    db: &Arc<Database>,
    ticket: &Ticket,
    caller_id: &str,
) -> Option<String> {
    let epic_id = ticket.epic_id.as_ref()?;

    match db.get_dependency_base_branch(epic_id) {
        Ok(Some(ref branch)) => {
            tracing::info!(
                "{} using cross-epic branching: basing {} on dependency epic's last child branch {}",
                caller_id, ticket.id, branch
            );
            Some(branch.clone())
        }
        Ok(None) => {
            tracing::info!(
                "{} ticket {} is first child in epic with no dependency, using default branch",
                caller_id, ticket.id
            );
            None
        }
        Err(e) => {
            tracing::warn!(
                "{} failed to get dependency base branch for epic {}: {}, using default branch",
                caller_id, epic_id, e
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_ticket(labels: Vec<String>, order_in_epic: Option<i32>, epic_id: Option<String>) -> Ticket {
        Ticket {
            id: "t1".to_string(),
            board_id: "b1".to_string(),
            column_id: "c1".to_string(),
            title: "Test".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            locked_by_run_id: None,
            lock_expires_at: None,
            project_id: None,
            workspace_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id,
            order_in_epic,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
            paused_at: None,
            paused_at_stage: None,
            paused_run_id: None,
        }
    }

    #[test]
    fn is_merge_dependencies_ticket_true_when_label_present() {
        let ticket = make_ticket(
            vec!["merge-dependencies".to_string()],
            Some(0),
            Some("epic-1".to_string()),
        );
        assert!(is_merge_dependencies_ticket(&ticket));
    }

    #[test]
    fn is_merge_dependencies_ticket_false_without_label() {
        let ticket = make_ticket(
            vec!["other-label".to_string()],
            Some(0),
            Some("epic-1".to_string()),
        );
        assert!(!is_merge_dependencies_ticket(&ticket));
    }

    #[test]
    fn is_merge_dependencies_ticket_true_regardless_of_order() {
        // Merge-dependencies tickets should be detected by label alone,
        // regardless of order_in_epic value. This handles the race condition
        // where the ticket is processed before repair logic fixes the order.
        let ticket = make_ticket(
            vec!["merge-dependencies".to_string()],
            Some(5), // Any order value
            Some("epic-1".to_string()),
        );
        assert!(is_merge_dependencies_ticket(&ticket));
    }

    #[test]
    fn is_merge_dependencies_ticket_true_when_order_is_none() {
        // Should still detect merge-dependencies ticket even with NULL order
        let ticket = make_ticket(
            vec!["merge-dependencies".to_string()],
            None,
            Some("epic-1".to_string()),
        );
        assert!(is_merge_dependencies_ticket(&ticket));
    }

    #[test]
    fn get_base_branch_returns_none_for_non_epic_child() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let ticket = make_ticket(vec![], None, None); // No epic_id
        
        let result = get_base_branch_for_ticket(&db, &ticket, "worker-1");
        assert!(result.is_none());
    }

    #[test]
    fn get_base_branch_returns_none_for_merge_dependencies_ticket() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

        // Create an epic
        let epic = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: backlog.id.clone(),
            title: "Epic".to_string(),
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
            spec_version_id: None,
        }).unwrap();

        // Create a merge-dependencies ticket
        let merge_ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: backlog.id.clone(),
            title: "Merge Deps".to_string(),
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
        }).unwrap();

        // Set order_in_epic to 0
        db.set_ticket_order_in_epic(&merge_ticket.id, 0).unwrap();

        // Re-fetch to get updated ticket
        let updated = db.get_ticket(&merge_ticket.id).unwrap();

        let result = get_base_branch_for_ticket(&db, &updated, "worker-1");
        assert!(result.is_none(), "Merge-dependencies tickets should use default branch");
    }

    #[test]
    fn get_base_branch_skips_chain_when_not_spec_linked_despite_sibling_branch() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

        let epic = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: backlog.id.clone(),
                title: "Epic".to_string(),
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
                spec_version_id: None,
            })
            .unwrap();

        let child1 = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: backlog.id.clone(),
                title: "Child 1".to_string(),
                description_md: "".to_string(),
                priority: Priority::Medium,
                labels: vec![],
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

        let child2 = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: backlog.id.clone(),
                title: "Child 2".to_string(),
                description_md: "".to_string(),
                priority: Priority::Medium,
                labels: vec![],
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

        db.update_ticket(
            &child1.id,
            &UpdateTicket {
                branch_name: Some("feat/epic/prev".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        let child2 = db.get_ticket(&child2.id).unwrap();
        let result = get_base_branch_for_ticket(&db, &child2, "worker-1");
        assert!(
            result.is_none(),
            "non-spec epic children should not chain from sibling"
        );
    }

    #[test]
    fn get_base_branch_uses_sibling_when_spec_linked() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();

        let project_dir =
            std::env::temp_dir().join(format!("bored-branching-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&project_dir).unwrap();
        let project = db
            .create_project(&CreateProject {
                name: "P".to_string(),
                path: project_dir.to_string_lossy().into_owned(),
                requires_git: false,
            })
            .unwrap();

        let spec = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: Some(board.id.clone()),
                project_id: project.id.clone(),
                name: "S".to_string(),
                user_input: "u".to_string(),
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();

        let version = db.get_latest_spec_version(&spec.id).unwrap().unwrap();

        let epic = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: backlog.id.clone(),
                title: "Epic".to_string(),
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
                spec_version_id: None,
            })
            .unwrap();

        let child1 = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: backlog.id.clone(),
                title: "Child 1".to_string(),
                description_md: "".to_string(),
                priority: Priority::Medium,
                labels: vec![],
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

        let child2 = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: backlog.id.clone(),
                title: "Child 2".to_string(),
                description_md: "".to_string(),
                priority: Priority::Medium,
                labels: vec![],
                project_id: None,
                workspace_id: None,
                workflow_type: WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: Some(epic.id.clone()),
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: Some(version.id.clone()),
            })
            .unwrap();

        db.update_ticket(
            &child1.id,
            &UpdateTicket {
                branch_name: Some("feat/epic/prev".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        let child2 = db.get_ticket(&child2.id).unwrap();
        let result = get_base_branch_for_ticket(&db, &child2, "worker-1");
        assert_eq!(result.as_deref(), Some("feat/epic/prev"));
    }
}
