use crate::db::models::Ticket;
use crate::db::{Database, DbError};
use rusqlite::OptionalExtension;

/// Info about the first incomplete dependency found by
/// [`Database::are_all_dependencies_complete`].
#[derive(Debug, Clone)]
pub struct IncompleteDependency {
    /// The ticket ID of the incomplete dependency epic.
    pub id: String,
    /// The title of the incomplete dependency epic.
    pub title: String,
}

impl Database {
    /// Get all children of an epic, ordered by order_in_epic
    ///
    /// Note: Uses COALESCE to sort NULL order_in_epic values last (after all numeric values).
    /// This handles legacy tickets that may have NULL order_in_epic.
    pub fn get_epic_children(&self, epic_id: &str) -> Result<Vec<Ticket>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, column_id, title, description_md, priority, 
                          labels_json, created_at, updated_at, locked_by_run_id, 
                          lock_expires_at, project_id, workflow_type, model, branch_name,
                          is_epic, epic_id, order_in_epic, depends_on_epic_id, depends_on_epic_ids_json, spec_version_id,
                          paused_at, paused_at_stage, paused_run_id
                   FROM tickets WHERE epic_id = ?
                   ORDER BY COALESCE(order_in_epic, 2147483647) ASC, created_at ASC"#,
            )?;

            let rows = stmt.query_map([epic_id], Self::map_ticket_row)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    /// Get the next pending child ticket for an epic (first child in Backlog)
    ///
    /// Note: Uses COALESCE to sort NULL order_in_epic values last (after all numeric values).
    /// This ensures injected tickets at order 0 (like merge-dependencies) are returned
    /// before legacy tickets with NULL order.
    pub fn get_next_pending_child(&self, epic_id: &str) -> Result<Option<Ticket>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT t.id, t.board_id, t.column_id, t.title, t.description_md, t.priority, 
                          t.labels_json, t.created_at, t.updated_at, t.locked_by_run_id, 
                          t.lock_expires_at, t.project_id, t.workflow_type, t.model, t.branch_name,
                          t.is_epic, t.epic_id, t.order_in_epic, t.depends_on_epic_id, t.depends_on_epic_ids_json, t.spec_version_id,
                          t.paused_at, t.paused_at_stage, t.paused_run_id
                   FROM tickets t
                   JOIN columns c ON t.column_id = c.id
                   WHERE t.epic_id = ? AND c.name = 'Backlog'
                   ORDER BY COALESCE(t.order_in_epic, 2147483647) ASC, t.created_at ASC
                   LIMIT 1"#,
            )?;

            stmt.query_row([epic_id], Self::map_ticket_row)
                .optional()
                .map_err(DbError::from)
        })
    }

    /// Get progress stats for an epic's children
    pub fn get_epic_progress(
        &self,
        epic_id: &str,
    ) -> Result<crate::db::models::EpicProgress, DbError> {
        use crate::db::models::EpicProgress;

        self.with_conn(|conn| {
            let mut progress = EpicProgress::default();

            let mut stmt = conn.prepare(
                r#"SELECT c.name, COUNT(*) as cnt
                   FROM tickets t
                   JOIN columns c ON t.column_id = c.id
                   WHERE t.epic_id = ?
                   GROUP BY c.name"#,
            )?;

            let rows = stmt.query_map([epic_id], |row| {
                let name: String = row.get(0)?;
                let count: i32 = row.get(1)?;
                Ok((name, count))
            })?;

            for row in rows {
                let (name, count) = row?;
                progress.total += count;
                match name.as_str() {
                    "Backlog" => progress.backlog = count,
                    "Ready" => progress.ready = count,
                    "In Progress" => progress.in_progress = count,
                    "Blocked" => progress.blocked = count,
                    "Review" => progress.review = count,
                    "Done" => progress.done = count,
                    _ => {} // Unknown column
                }
            }

            Ok(progress)
        })
    }

    /// Add an existing ticket to an epic as a child
    pub fn add_ticket_to_epic(&self, epic_id: &str, ticket_id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            // Verify epic exists and is actually an epic
            let is_epic: bool = conn
                .query_row(
                    "SELECT is_epic FROM tickets WHERE id = ?",
                    [epic_id],
                    |row| row.get::<_, i32>(0),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        DbError::NotFound(format!("Epic {} not found", epic_id))
                    }
                    other => DbError::Sqlite(other),
                })?
                != 0;

            if !is_epic {
                return Err(DbError::Validation(format!(
                    "Ticket {} is not an epic",
                    epic_id
                )));
            }

            // Get current max order
            let max_order: Option<i32> = conn
                .query_row(
                    "SELECT MAX(order_in_epic) FROM tickets WHERE epic_id = ?",
                    [epic_id],
                    |row| row.get(0),
                )
                .unwrap_or(None);

            let order = max_order.unwrap_or(-1) + 1;
            let now = chrono::Utc::now().to_rfc3339();

            let affected = conn.execute(
                "UPDATE tickets SET epic_id = ?, order_in_epic = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![epic_id, order, now, ticket_id],
            )?;

            if affected == 0 {
                return Err(DbError::NotFound(format!("Ticket {} not found", ticket_id)));
            }

            Ok(())
        })
    }

    /// Remove a ticket from its parent epic
    pub fn remove_ticket_from_epic(&self, ticket_id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();

            conn.execute(
                "UPDATE tickets SET epic_id = NULL, order_in_epic = NULL, updated_at = ? WHERE id = ?",
                rusqlite::params![now, ticket_id],
            )?;

            Ok(())
        })
    }

    /// Reorder children within an epic
    /// child_ids should be the list of ticket IDs in the desired order
    pub fn reorder_epic_children(
        &self,
        epic_id: &str,
        child_ids: &[String],
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();

            for (index, ticket_id) in child_ids.iter().enumerate() {
                conn.execute(
                    "UPDATE tickets SET order_in_epic = ?, updated_at = ? WHERE id = ? AND epic_id = ?",
                    rusqlite::params![index as i32, now, ticket_id, epic_id],
                )?;
            }

            Ok(())
        })
    }

    /// Get all epics that depend on the given epic.
    ///
    /// Checks both `depends_on_epic_id` (primary/legacy) and `depends_on_epic_ids_json`
    /// (full list for multi-dependency epics). This ensures that when ANY dependency
    /// completes, all dependent epics are discovered -- not just those whose primary
    /// dependency matches.
    pub fn get_epics_depending_on(&self, epic_id: &str) -> Result<Vec<Ticket>, DbError> {
        self.with_conn(|conn| {
            // Match on primary dependency OR anywhere in the JSON array.
            // The JSON pattern uses '"<id>"' to avoid partial-id false positives.
            let pattern = format!("%\"{}\"%", epic_id);
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, column_id, title, description_md, priority, 
                          labels_json, created_at, updated_at, locked_by_run_id, 
                          lock_expires_at, project_id, workflow_type, model, branch_name,
                          is_epic, epic_id, order_in_epic, depends_on_epic_id, depends_on_epic_ids_json, spec_version_id,
                          paused_at, paused_at_stage, paused_run_id
                   FROM tickets
                   WHERE (depends_on_epic_id = ?1 OR depends_on_epic_ids_json LIKE ?2)
                     AND is_epic = 1"#,
            )?;

            let rows = stmt.query_map(rusqlite::params![epic_id, pattern], Self::map_ticket_row)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    /// Check whether an epic has any child ticket that is currently active
    /// (i.e. not in Backlog and not in Done).
    ///
    /// This is used to prevent advancing a second child to Ready when one is
    /// already being worked on.
    pub fn has_active_epic_child(&self, epic_id: &str) -> Result<bool, DbError> {
        self.with_conn(|conn| {
            let active: i32 = conn.query_row(
                r#"SELECT COUNT(*) FROM tickets t
                   JOIN columns c ON t.column_id = c.id
                   WHERE t.epic_id = ? AND c.name NOT IN ('Backlog', 'Done')"#,
                [epic_id],
                |row| row.get(0),
            )?;
            Ok(active > 0)
        })
    }

    /// Check whether ALL of an epic's dependencies are in the Done column.
    ///
    /// Uses `depends_on_epic_ids` when available, but falls back to the
    /// legacy `depends_on_epic_id` field for older tickets that were created
    /// before multi-dependency support (where `depends_on_epic_ids_json` is
    /// NULL and the vector is therefore empty).
    ///
    /// Returns `Ok(None)` when every dependency is complete (or the epic has
    /// no dependencies).  Returns `Ok(Some(info))` with the first incomplete
    /// dependency's ID and title when at least one dependency is not Done.
    pub fn are_all_dependencies_complete(
        &self,
        epic: &Ticket,
    ) -> Result<Option<IncompleteDependency>, DbError> {
        let effective_deps: Vec<&str> = if !epic.depends_on_epic_ids.is_empty() {
            epic.depends_on_epic_ids.iter().map(|s| s.as_str()).collect()
        } else if let Some(ref dep_id) = epic.depends_on_epic_id {
            vec![dep_id.as_str()]
        } else {
            return Ok(None); // No dependencies at all
        };

        for dep_id in &effective_deps {
            let dep = self.get_ticket(dep_id)?;
            let columns = self.get_columns(&dep.board_id)?;
            let dep_column = columns.into_iter().find(|c| c.id == dep.column_id);

            let is_done = match dep_column {
                Some(ref col) => col.name == "Done",
                None => {
                    tracing::warn!(
                        "Epic {}: could not find column {} for dependency {}, treating as incomplete",
                        epic.id, dep.column_id, dep_id
                    );
                    false
                }
            };

            if !is_done {
                return Ok(Some(IncompleteDependency {
                    id: dep.id,
                    title: dep.title,
                }));
            }
        }

        Ok(None)
    }

    /// Check if all children of an epic are in Done column
    pub fn are_all_epic_children_done(&self, epic_id: &str) -> Result<bool, DbError> {
        self.with_conn(|conn| {
            // Count children not in Done
            let not_done: i32 = conn.query_row(
                r#"SELECT COUNT(*) FROM tickets t
                   JOIN columns c ON t.column_id = c.id
                   WHERE t.epic_id = ? AND c.name != 'Done'"#,
                [epic_id],
                |row| row.get(0),
            )?;

            // Also check there's at least one child
            let total: i32 = conn.query_row(
                "SELECT COUNT(*) FROM tickets WHERE epic_id = ?",
                [epic_id],
                |row| row.get(0),
            )?;

            Ok(total > 0 && not_done == 0)
        })
    }

    /// Get the last child ticket with a branch name from the dependency epic.
    /// Used for cross-epic branching: when an epic depends on another, its first
    /// child should branch from the last child of the dependency epic.
    pub fn get_dependency_base_branch(&self, epic_id: &str) -> Result<Option<String>, DbError> {
        self.with_conn(|conn| {
            // First get the dependency epic id
            let depends_on: Option<String> = conn
                .query_row(
                    "SELECT depends_on_epic_id FROM tickets WHERE id = ?",
                    [epic_id],
                    |row| row.get(0),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        DbError::NotFound(format!("Epic {}", epic_id))
                    }
                    other => DbError::Sqlite(other),
                })?;

            let Some(dependency_id) = depends_on else {
                return Ok(None); // No dependency
            };

            // Get the last child of the dependency epic that has a branch_name
            // Order by order_in_epic DESC to get the last child first
            let branch: Option<String> = conn
                .query_row(
                    r#"SELECT t.branch_name FROM tickets t
                   JOIN columns c ON t.column_id = c.id
                   WHERE t.epic_id = ? AND t.branch_name IS NOT NULL AND c.name = 'Done'
                   ORDER BY t.order_in_epic DESC
                   LIMIT 1"#,
                    [&dependency_id],
                    |row| row.get(0),
                )
                .ok();

            Ok(branch)
        })
    }

    /// Get the final branch of an epic (the last completed child's branch name).
    /// This is used for consolidation epics to know which branch to merge from.
    pub fn get_epic_final_branch(&self, epic_id: &str) -> Result<Option<String>, DbError> {
        self.with_conn(|conn| {
            // Get the last child of the epic that has a branch_name and is in Done
            // Order by order_in_epic DESC to get the last child first
            let branch: Option<String> = conn
                .query_row(
                    r#"SELECT t.branch_name FROM tickets t
                   JOIN columns c ON t.column_id = c.id
                   WHERE t.epic_id = ? AND t.branch_name IS NOT NULL AND c.name = 'Done'
                   ORDER BY t.order_in_epic DESC
                   LIMIT 1"#,
                    [epic_id],
                    |row| row.get(0),
                )
                .ok();

            Ok(branch)
        })
    }

    /// Get all epics for a spec with their final branches (for consolidation).
    /// Returns a list of (epic_id, epic_title, final_branch) tuples.
    pub fn get_spec_epics_with_branches(
        &self,
        spec_version_id: &str,
    ) -> Result<Vec<(String, String, Option<String>)>, DbError> {
        // First get all non-consolidation epics for this spec
        let epics = self.get_spec_version_epics(spec_version_id)?;

        let mut result = Vec::new();
        for epic in epics {
            // Skip consolidation epics
            if epic.is_consolidation_epic() {
                continue;
            }
            let branch = self.get_epic_final_branch(&epic.id)?;
            result.push((epic.id, epic.title, branch));
        }

        Ok(result)
    }

    /// Get the final branches from ALL dependency epics.
    /// Returns: Vec<(epic_id, epic_title, branch_name)>
    pub fn get_all_dependency_branches(
        &self,
        epic_id: &str,
    ) -> Result<Vec<(String, String, String)>, DbError> {
        let deps_with_titles: Vec<(String, String)> = self.with_conn(|conn| {
            let deps_json: Option<String> = conn
                .query_row(
                    "SELECT depends_on_epic_ids_json FROM tickets WHERE id = ?",
                    [epic_id],
                    |row| row.get(0),
                )
                .ok()
                .flatten();

            let dep_ids: Vec<String> = deps_json
                .and_then(|j| serde_json::from_str(&j).ok())
                .unwrap_or_default();

            let mut result = Vec::new();
            for dep_id in dep_ids {
                let title = conn
                    .query_row(
                        "SELECT title FROM tickets WHERE id = ?",
                        [&dep_id],
                        |row| row.get::<_, String>(0),
                    )
                    .unwrap_or_else(|_| dep_id.clone());
                result.push((dep_id, title));
            }
            Ok(result)
        })?;

        // Fetch branches outside with_conn to avoid deadlock
        let mut result = Vec::new();
        for (dep_id, title) in deps_with_titles {
            if let Ok(Some(branch)) = self.get_epic_final_branch(&dep_id) {
                result.push((dep_id, title, branch));
            }
        }
        Ok(result)
    }

    /// Shift all children's order_in_epic by a given amount (to make room for injected tickets)
    ///
    /// This function also handles legacy tickets with NULL order_in_epic values.
    /// In SQLite, NULL + N = NULL, so we first assign NULL-order tickets a value
    /// at the end of the order (max_order + 1 + shift), then shift all non-NULL values.
    pub fn shift_epic_children_order(&self, epic_id: &str, shift: i32) -> Result<(), DbError> {
        self.with_conn(|conn| {
            // First, get the current max order to assign to NULL-order tickets
            let max_order: Option<i32> = conn
                .query_row(
                    "SELECT MAX(order_in_epic) FROM tickets WHERE epic_id = ?",
                    [epic_id],
                    |row| row.get(0),
                )
                .unwrap_or(None);

            // Assign NULL-order tickets a position at the end (after the shift)
            // They get max_order + 1 + shift so they remain after all shifted tickets
            let null_order_value = max_order.unwrap_or(-1) + 1 + shift;
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "UPDATE tickets SET order_in_epic = ?, updated_at = ? WHERE epic_id = ? AND order_in_epic IS NULL",
                rusqlite::params![null_order_value, now, epic_id],
            )?;

            // Then shift all tickets (now all have non-NULL orders)
            conn.execute(
                "UPDATE tickets SET order_in_epic = order_in_epic + ? WHERE epic_id = ? AND order_in_epic != ?",
                rusqlite::params![shift, epic_id, null_order_value],
            )?;
            Ok(())
        })
    }

    /// Set the order_in_epic for a specific ticket
    pub fn set_ticket_order_in_epic(&self, ticket_id: &str, order: i32) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "UPDATE tickets SET order_in_epic = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![order, now, ticket_id],
            )?;
            Ok(())
        })
    }

    /// Check if an epic already has a merge-dependencies ticket injected
    pub fn has_merge_dependencies_ticket(&self, epic_id: &str) -> Result<bool, DbError> {
        Ok(self.get_merge_dependencies_ticket(epic_id)?.is_some())
    }

    /// Get the merge-dependencies ticket for an epic if one exists.
    /// Returns the ticket regardless of its order_in_epic value.
    pub fn get_merge_dependencies_ticket(&self, epic_id: &str) -> Result<Option<Ticket>, DbError> {
        self.with_conn(|conn| {
            // Match exact JSON element boundaries to avoid false positives from labels
            // like "merge-dependencies-v2". The label in JSON appears as either:
            // - "merge-dependencies"] (last element)
            // - "merge-dependencies", (followed by another element)
            let ticket_id: Option<String> = conn
                .query_row(
                    r#"SELECT id FROM tickets 
                       WHERE epic_id = ? AND (
                           labels_json LIKE '%"merge-dependencies"]%' OR
                           labels_json LIKE '%"merge-dependencies",%'
                       )"#,
                    [epic_id],
                    |row| row.get(0),
                )
                .ok();

            match ticket_id {
                Some(id) => {
                    // Note: We can't call get_ticket here as it would cause deadlock
                    // with the current connection. Return just the ID wrapped in a marker.
                    // The caller should fetch the full ticket outside with_conn.
                    Ok(Some(id))
                }
                None => Ok(None),
            }
        })
        .and_then(|maybe_id: Option<String>| {
            match maybe_id {
                Some(id) => self.get_ticket(&id).map(Some),
                None => Ok(None),
            }
        })
    }

    /// Get the previous sibling of a child ticket in an epic (for chain branching)
    /// Returns the ticket that is one position before this ticket in the epic's order
    pub fn get_previous_epic_sibling(&self, ticket_id: &str) -> Result<Option<Ticket>, DbError> {
        self.with_conn(|conn| {
            // First, get this ticket's epic_id and order_in_epic
            let ticket_info: Option<(String, i32)> = conn
                .query_row(
                    "SELECT epic_id, order_in_epic FROM tickets WHERE id = ? AND epic_id IS NOT NULL",
                    [ticket_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;

            let (epic_id, order) = match ticket_info {
                Some((eid, ord)) => (eid, ord),
                None => return Ok(None), // Not a child of an epic
            };

            if order == 0 {
                return Ok(None); // First child, no previous sibling
            }

            // Get the previous sibling (order_in_epic = order - 1)
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, column_id, title, description_md, priority,
                          labels_json, created_at, updated_at, locked_by_run_id, 
                          lock_expires_at, project_id, workflow_type, model, branch_name,
                          is_epic, epic_id, order_in_epic, depends_on_epic_id, depends_on_epic_ids_json, spec_version_id,
                          paused_at, paused_at_stage, paused_run_id
                   FROM tickets WHERE epic_id = ? AND order_in_epic = ?"#,
            )?;

            stmt.query_row(rusqlite::params![epic_id, order - 1], Self::map_ticket_row)
                .optional()
                .map_err(DbError::from)
        })
    }
}
