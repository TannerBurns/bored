//! Consolidation and merge logic for epics.
//!
//! Handles:
//! - Populating consolidation epic tickets with merge instructions
//! - Topological ordering of epics by dependencies
//! - Injecting merge-dependencies tickets for multi-dependency epics

use crate::db::{
    AuthorType, CreateComment, CreateTicket, Database, DbError, Priority, Ticket, UpdateTicket,
    WorkflowType,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

/// Populate consolidation epic ticket descriptions with branch merge instructions.
/// Called when a consolidation epic moves to Ready (all dependencies complete).
pub(super) fn populate_consolidation_tickets(
    db: &Arc<Database>,
    epic: &Ticket,
) -> Result<(), DbError> {
    let Some(ref spec_version_id) = epic.spec_version_id else {
        tracing::warn!(
            "Consolidation epic {} has no spec_version_id, cannot populate branch info",
            epic.id
        );
        return Ok(());
    };

    let epics_with_branches = db.get_spec_epics_with_branches(spec_version_id)?;

    if epics_with_branches.is_empty() {
        tracing::warn!(
            "Consolidation epic {}: no epics with branches found in spec version {}",
            epic.id,
            spec_version_id
        );
        return Ok(());
    }

    let ordered_epics = order_epics_by_dependencies(&epics_with_branches, db)?;

    let (valid_branches, missing_branches): (Vec<_>, Vec<_>) = ordered_epics
        .iter()
        .partition(|(_, _, branch)| branch.is_some());

    let description = build_consolidation_description(spec_version_id, &valid_branches, &missing_branches, &ordered_epics);

    let children = db.get_epic_children(&epic.id)?;
    for child in children {
        // Skip merge-dependencies tickets - they have their own specialized description
        // from build_merge_description that should not be overwritten
        if child.labels.contains(&"merge-dependencies".to_string()) {
            tracing::debug!(
                "Consolidation epic {}: skipping merge-dependencies ticket {} (preserving specialized description)",
                epic.id,
                child.id
            );
            continue;
        }

        db.update_ticket(
            &child.id,
            &UpdateTicket {
                description_md: Some(description.clone()),
                ..Default::default()
            },
        )?;

        tracing::info!(
            "Consolidation epic {}: updated child {} with branch merge instructions",
            epic.id,
            child.id
        );
    }

    let missing_warning = if !missing_branches.is_empty() {
        format!(
            "\n\n⚠️ {} epic(s) missing branches: {}",
            missing_branches.len(),
            missing_branches
                .iter()
                .map(|(_, title, _)| title.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    } else {
        String::new()
    };

    db.create_comment(&CreateComment {
        ticket_id: epic.id.clone(),
        author_type: AuthorType::System,
        body_md: format!(
            "Consolidation epic ready. {} epic branches to merge:\n{}{}",
            valid_branches.len(),
            valid_branches
                .iter()
                .filter_map(|(_, title, branch)| branch
                    .as_ref()
                    .map(|b| format!("- {} → `{}`", title, b)))
                .collect::<Vec<_>>()
                .join("\n"),
            missing_warning
        ),
        metadata: None,
    })?;

    Ok(())
}

fn build_consolidation_description(
    spec_version_id: &str,
    valid_branches: &[&(String, String, Option<String>)],
    missing_branches: &[&(String, String, Option<String>)],
    ordered_epics: &[(String, String, Option<String>)],
) -> String {
    let mut merge_steps = Vec::new();
    merge_steps.push("## Branch Consolidation Task\n\n".to_string());
    merge_steps.push(
        "Create a consolidation branch and merge all epic work sequentially.\n\n".to_string(),
    );

    if !missing_branches.is_empty() {
        merge_steps.push("### ⚠️ MISSING BRANCHES\n\n".to_string());
        merge_steps.push(
            "The following epics have no final branch. Investigate before proceeding:\n\n"
                .to_string(),
        );
        for (epic_id, title, _) in missing_branches {
            merge_steps.push(format!("- **{}** (ID: `{}`)\n", title, epic_id));
        }
        merge_steps.push("\nPossible causes:\n".to_string());
        merge_steps.push("- Epic completed without creating branches\n".to_string());
        merge_steps.push("- Child tickets were moved to Done manually\n".to_string());
        merge_steps.push("- Branch was deleted before consolidation\n\n".to_string());
    }

    merge_steps.push("### Steps:\n\n".to_string());
    merge_steps.push(format!(
        "1. Create new branch from main: `spec-version/{}/consolidated`\n\n",
        spec_version_id
    ));

    let mut step = 2;
    for (_, epic_title, branch) in valid_branches {
        if let Some(branch_name) = branch {
            merge_steps.push(format!(
                "{}. Merge `{}` (from Epic: {})\n",
                step, branch_name, epic_title
            ));
            merge_steps.push(format!("   - `git merge --no-ff {}`\n", branch_name));
            merge_steps.push("   - Resolve any conflicts\n".to_string());
            merge_steps.push("   - Run tests: verify passing\n".to_string());
            merge_steps.push(format!(
                "   - Commit with message: 'Merge {} into consolidated'\n\n",
                epic_title
            ));
            step += 1;
        }
    }

    merge_steps.push(format!(
        "{}. Run full test suite and verify all tests pass\n",
        step
    ));
    merge_steps.push(format!("{}. Push the consolidated branch\n\n", step + 1));

    merge_steps.push("### Conflict Resolution Strategy:\n\n".to_string());
    merge_steps.push("- For each conflict, examine both versions carefully\n".to_string());
    merge_steps.push("- Prefer the most complete/recent implementation\n".to_string());
    merge_steps.push(
        "- Check for semantic conflicts (code that compiles but has bugs)\n".to_string(),
    );
    merge_steps.push("- Run tests after each merge to catch issues early\n".to_string());
    merge_steps
        .push("- Document significant decisions in merge commit messages\n\n".to_string());

    merge_steps.push("### Epics to Merge (Dependency Order):\n\n".to_string());
    for (_, epic_title, branch) in ordered_epics {
        let branch_info = branch
            .as_ref()
            .map(|b| format!(" → `{}`", b))
            .unwrap_or_else(|| " ⚠️ NO BRANCH".to_string());
        merge_steps.push(format!("- {}{}\n", epic_title, branch_info));
    }

    merge_steps.join("")
}

/// Topological sort: epics with no dependencies first.
fn order_epics_by_dependencies(
    epics: &[(String, String, Option<String>)],
    db: &Arc<Database>,
) -> Result<Vec<(String, String, Option<String>)>, DbError> {
    let mut deps_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut all_ids: HashSet<String> = HashSet::new();

    for (epic_id, _, _) in epics {
        all_ids.insert(epic_id.clone());
        if let Ok(ticket) = db.get_ticket(epic_id) {
            deps_map.insert(epic_id.clone(), ticket.depends_on_epic_ids.clone());
        } else {
            deps_map.insert(epic_id.clone(), vec![]);
        }
    }

    let mut in_degree: HashMap<String, usize> = HashMap::new();
    for (id, deps) in &deps_map {
        let count = deps.iter().filter(|d| all_ids.contains(*d)).count();
        in_degree.insert(id.clone(), count);
    }

    let mut queue: VecDeque<String> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(id, _)| id.clone())
        .collect();

    let mut sorted = Vec::new();
    while let Some(id) = queue.pop_front() {
        sorted.push(id.clone());

        for (other_id, other_deps) in &deps_map {
            if other_deps.contains(&id) {
                if let Some(count) = in_degree.get_mut(other_id) {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        queue.push_back(other_id.clone());
                    }
                }
            }
        }
    }

    if sorted.len() != all_ids.len() {
        tracing::warn!(
            "Dependency cycle detected in epics, using original order. Sorted {} of {}",
            sorted.len(),
            all_ids.len()
        );
        return Ok(epics.to_vec());
    }

    let id_to_epic: HashMap<_, _> = epics.iter().map(|e| (e.0.clone(), e.clone())).collect();
    Ok(sorted
        .into_iter()
        .filter_map(|id| id_to_epic.get(&id).cloned())
        .collect())
}

/// Inject a "Merge Dependencies" ticket as the first child of an epic with multiple dependencies.
/// This ticket instructs the agent to merge all dependency branches before starting the epic's work.
pub(super) fn inject_merge_dependencies_ticket(
    db: &Arc<Database>,
    epic: &Ticket,
) -> Result<String, DbError> {
    // === PHASE 1: Validation and precondition checks (read-only) ===
    // All preconditions must be validated BEFORE any mutations to avoid leaving
    // the database in an inconsistent state if later operations fail.
    // The caller swallows errors, so partial mutations would be permanent.

    let dependency_branches = db.get_all_dependency_branches(&epic.id)?;

    if dependency_branches.is_empty() {
        return Err(DbError::Validation(
            "No dependency branches available to merge".to_string(),
        ));
    }

    let backlog_column = db
        .find_column_by_name(&epic.board_id, "Backlog")?
        .ok_or_else(|| DbError::NotFound("Backlog column not found".to_string()))?;

    // === PHASE 2: Pure computation (no side effects) ===
    let description = build_merge_description(epic, &dependency_branches);

    // === PHASE 3: Mutations in safe order ===
    // Create the ticket FIRST, before shifting children. This ensures:
    // - If create_ticket fails: no side effects (children unmodified)
    // - If shift fails after create: ticket exists (has_merge_dependencies_ticket returns true)
    //   so retries are blocked, preventing unbounded order growth
    // - If set_order fails: same as above, ticket exists at wrong position but retries blocked
    //
    // The old order (shift -> create -> set_order) was dangerous because if create_ticket
    // failed after shift succeeded, children would be permanently shifted with no ticket,
    // and retries would shift them again.

    let merge_ticket = db.create_ticket(&CreateTicket {
        board_id: epic.board_id.clone(),
        column_id: backlog_column.id.clone(),
        title: format!("Merge dependency branches for {}", epic.title),
        description_md: description,
        priority: Priority::High,
        labels: vec![
            "auto-generated".to_string(),
            "merge-dependencies".to_string(),
        ],
        project_id: epic.project_id.clone(),
        workspace_id: epic.workspace_id.clone(),
        workflow_type: WorkflowType::MultiStage,
        model: epic.model.clone(),
        branch_name: None,
        is_epic: false,
        epic_id: Some(epic.id.clone()),
        depends_on_epic_id: None,
        depends_on_epic_ids: vec![],
        spec_version_id: epic.spec_version_id.clone(),
    })?;

    // Shift ALL children (including the just-created merge ticket) by 1
    db.shift_epic_children_order(&epic.id, 1)?;

    // Then set the merge ticket's order to 0, making it first
    db.set_ticket_order_in_epic(&merge_ticket.id, 0)?;

    db.create_comment(&CreateComment {
        ticket_id: epic.id.clone(),
        author_type: AuthorType::System,
        body_md: format!(
            "Auto-generated merge ticket to combine {} dependency branches before epic work begins:\n{}",
            dependency_branches.len(),
            dependency_branches
                .iter()
                .map(|(_, title, branch)| format!("- {} → `{}`", title, branch))
                .collect::<Vec<_>>()
                .join("\n")
        ),
        metadata: None,
    })?;

    tracing::info!(
        "Epic {}: injected merge-dependencies ticket {} with {} branches",
        epic.id,
        merge_ticket.id,
        dependency_branches.len()
    );

    Ok(merge_ticket.id)
}

fn build_merge_description(epic: &Ticket, dependency_branches: &[(String, String, String)]) -> String {
    let mut description = String::from("## Merge Dependency Branches\n\n");
    description.push_str(
        "Before starting this epic's work, merge all dependency branches into a unified base.\n\n",
    );
    description.push_str("### Steps:\n\n");
    description.push_str(&format!(
        "1. Create base branch: `git checkout -b epic/{}/base main`\n\n",
        epic.id
    ));

    for (i, (_, title, branch)) in dependency_branches.iter().enumerate() {
        description.push_str(&format!(
            "{}. Merge `{}` (from Epic: {})\n",
            i + 2,
            branch,
            title
        ));
        description.push_str(&format!("   - `git merge --no-ff {}`\n", branch));
        description.push_str("   - If conflicts: analyze both sides, resolve, run tests\n");
        description.push_str("   - Commit the merge\n\n");
    }

    description.push_str(&format!(
        "{}. Verify all tests pass\n",
        dependency_branches.len() + 2
    ));
    description.push_str(&format!(
        "{}. Push the base branch\n\n",
        dependency_branches.len() + 3
    ));

    description.push_str("### Conflict Resolution Strategy:\n\n");
    description.push_str("- For each conflict, examine both versions carefully\n");
    description.push_str("- Prefer the most complete/recent implementation\n");
    description.push_str("- Ensure merged code compiles and tests pass\n");
    description.push_str("- Document any significant merge decisions in commit message\n\n");

    description.push_str("### If Merge Fails:\n\n");
    description.push_str("- `git merge --abort` to reset\n");
    description.push_str("- Document which branches conflict and why\n");
    description.push_str("- Move ticket to Blocked with explanation\n");

    description
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{CreateProject, CreateSpec, CreateSpecVersion, CreateTicket};

    #[test]
    fn test_order_epics_by_dependencies_empty() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let result = order_epics_by_dependencies(&[], &db).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_order_epics_by_dependencies_preserves_order_without_db_tickets() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let epics = vec![
            ("a".to_string(), "Epic A".to_string(), Some("branch-a".to_string())),
            ("b".to_string(), "Epic B".to_string(), Some("branch-b".to_string())),
        ];
        let result = order_epics_by_dependencies(&epics, &db).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_populate_consolidation_tickets_skips_merge_dependencies_ticket() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let backlog = columns.iter().find(|c| c.name == "Backlog").unwrap();
        let done = columns.iter().find(|c| c.name == "Done").unwrap();

        // Create a project first (required for spec)
        // Use /tmp which exists on most systems
        let project = db
            .create_project(&CreateProject {
                name: "Test Project".to_string(),
                path: "/tmp".to_string(),
                requires_git: false,
            })
            .unwrap();

        // Create a spec and version for the consolidation epic
        let spec = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: None,
                project_id: project.id.clone(),
                name: "Test Spec".to_string(),
                user_input: "Test input".to_string(),
                model: None,
                settings: serde_json::Value::Null,
            })
            .unwrap();
        let version = db
            .create_spec_version(&CreateSpecVersion {
                spec_id: spec.id.clone(),
            })
            .unwrap();

        // Create a dependency epic with a completed child that has a branch
        let dep_epic = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: done.id.clone(),
                title: "Dependency Epic".to_string(),
                description_md: "".to_string(),
                priority: Priority::Medium,
                labels: vec!["consolidation".to_string()],
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

        db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: done.id.clone(),
            title: "Dep Child".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            workspace_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: Some("feat/dep-branch".to_string()),
            is_epic: false,
            epic_id: Some(dep_epic.id.clone()),
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .unwrap();

        // Create a consolidation epic
        let consolidation_epic = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: backlog.id.clone(),
                title: "Consolidation Epic".to_string(),
                description_md: "".to_string(),
                priority: Priority::Medium,
                labels: vec!["consolidation".to_string()],
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

        // Create a merge-dependencies ticket with specialized description
        let merge_description = "## Merge Dependency Branches\n\nSpecialized instructions here.";
        let merge_ticket = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: backlog.id.clone(),
                title: "Merge Dependencies".to_string(),
                description_md: merge_description.to_string(),
                priority: Priority::High,
                labels: vec![
                    "auto-generated".to_string(),
                    "merge-dependencies".to_string(),
                ],
                project_id: None,
                workspace_id: None,
                workflow_type: WorkflowType::MultiStage,
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: Some(consolidation_epic.id.clone()),
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: None,
            })
            .unwrap();

        // Create a regular child ticket
        let regular_child = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: backlog.id.clone(),
                title: "Regular Child".to_string(),
                description_md: "Original description".to_string(),
                priority: Priority::Medium,
                labels: vec![],
                project_id: None,
                workspace_id: None,
                workflow_type: WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: Some(consolidation_epic.id.clone()),
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: None,
            })
            .unwrap();

        // Call populate_consolidation_tickets
        populate_consolidation_tickets(&db, &consolidation_epic).unwrap();

        // Verify merge-dependencies ticket's description is PRESERVED
        let updated_merge = db.get_ticket(&merge_ticket.id).unwrap();
        assert_eq!(
            updated_merge.description_md, merge_description,
            "Merge-dependencies ticket description should NOT be overwritten"
        );

        // Verify regular child's description WAS updated
        let updated_regular = db.get_ticket(&regular_child.id).unwrap();
        assert!(
            updated_regular.description_md.contains("Branch Consolidation Task"),
            "Regular child description should be updated with consolidation instructions"
        );
        assert_ne!(
            updated_regular.description_md, "Original description",
            "Regular child description should have been changed"
        );
    }
}
