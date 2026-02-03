//! Branch selection logic for epic chain branching.
//!
//! Handles determining the base branch for tickets based on:
//! - Previous sibling branches (chain branching within an epic)
//! - Cross-epic dependency branches

use std::sync::Arc;

use crate::db::{Database, Ticket};

/// Determine the base branch for a ticket based on epic chain branching rules.
///
/// For epic child tickets, we implement chain branching where each ticket
/// branches from the previous sibling's branch (if available) rather than
/// the default branch.
pub fn get_base_branch_for_ticket(
    db: &Arc<Database>,
    ticket: &Ticket,
    worker_id: &str,
) -> Option<String> {
    ticket.epic_id.as_ref()?;

    match db.get_previous_epic_sibling(&ticket.id) {
        Ok(Some(prev_sibling)) => {
            if let Some(ref branch) = prev_sibling.branch_name {
                tracing::info!(
                    "Worker {} using chain branching: basing {} on previous sibling's branch {}",
                    worker_id, ticket.id, branch
                );
                Some(branch.clone())
            } else {
                tracing::info!(
                    "Worker {} previous sibling {} has no branch yet, using default branch",
                    worker_id, prev_sibling.id
                );
                None
            }
        }
        Ok(None) => {
            // First child in epic - check for cross-epic dependency branching
            get_cross_epic_base_branch(db, ticket, worker_id)
        }
        Err(e) => {
            tracing::warn!(
                "Worker {} failed to get previous sibling for {}: {}, using default branch",
                worker_id, ticket.id, e
            );
            None
        }
    }
}

/// Check for cross-epic dependency branching when ticket is the first child in its epic.
fn get_cross_epic_base_branch(
    db: &Arc<Database>,
    ticket: &Ticket,
    worker_id: &str,
) -> Option<String> {
    let epic_id = ticket.epic_id.as_ref()?;

    match db.get_dependency_base_branch(epic_id) {
        Ok(Some(ref branch)) => {
            tracing::info!(
                "Worker {} using cross-epic branching: basing {} on dependency epic's last child branch {}",
                worker_id, ticket.id, branch
            );
            Some(branch.clone())
        }
        Ok(None) => {
            tracing::info!(
                "Worker {} ticket {} is first child in epic with no dependency, using default branch",
                worker_id, ticket.id
            );
            None
        }
        Err(e) => {
            tracing::warn!(
                "Worker {} failed to get dependency base branch for epic {}: {}, using default branch",
                worker_id, epic_id, e
            );
            None
        }
    }
}
