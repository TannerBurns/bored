//! Database operations for spec versions

use rusqlite::OptionalExtension;

use crate::db::models::{
    CreateSpecVersion, Exploration, SpecVersion, SpecVersionStatus, UpdateSpecVersion,
};
use crate::db::{parse_datetime, Database, DbError};

impl Database {
    /// Create a new version for a spec
    pub fn create_spec_version(&self, input: &CreateSpecVersion) -> Result<SpecVersion, DbError> {
        self.with_conn(|conn| {
            let id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now();

            // Get the next version number for this spec
            let version_number: i32 = conn
                .query_row(
                    "SELECT COALESCE(MAX(version_number), 0) + 1 FROM spec_versions WHERE spec_id = ?",
                    [&input.spec_id],
                    |row| row.get(0),
                )
                .unwrap_or(1);

            conn.execute(
                r#"INSERT INTO spec_versions 
                   (id, spec_id, version_number, status, exploration_log, created_at, updated_at)
                   VALUES (?, ?, ?, ?, ?, ?, ?)"#,
                rusqlite::params![
                    id,
                    input.spec_id,
                    version_number,
                    SpecVersionStatus::Conversing.as_str(),
                    "[]",
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )?;

            Ok(SpecVersion {
                id,
                spec_id: input.spec_id.clone(),
                version_number,
                status: SpecVersionStatus::Conversing,
                exploration_log: vec![],
                plan_markdown: None,
                plan_json: None,
                work_started_at: None,
                created_at: now,
                updated_at: now,
            })
        })
    }

    /// Get a spec version by ID
    pub fn get_spec_version(&self, id: &str) -> Result<SpecVersion, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, spec_id, version_number, status, exploration_log, 
                          plan_markdown, plan_json, work_started_at, created_at, updated_at
                   FROM spec_versions WHERE id = ?"#,
            )?;

            stmt.query_row([id], Self::map_spec_version_row)
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        DbError::NotFound(format!("SpecVersion {}", id))
                    }
                    other => DbError::Sqlite(other),
                })
        })
    }

    /// Get all versions for a spec
    pub fn get_spec_versions(&self, spec_id: &str) -> Result<Vec<SpecVersion>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, spec_id, version_number, status, exploration_log, 
                          plan_markdown, plan_json, work_started_at, created_at, updated_at
                   FROM spec_versions WHERE spec_id = ?
                   ORDER BY version_number ASC"#,
            )?;

            let rows = stmt.query_map([spec_id], Self::map_spec_version_row)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    /// Get the latest version for a spec
    pub fn get_latest_spec_version(&self, spec_id: &str) -> Result<Option<SpecVersion>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, spec_id, version_number, status, exploration_log, 
                          plan_markdown, plan_json, work_started_at, created_at, updated_at
                   FROM spec_versions WHERE spec_id = ?
                   ORDER BY version_number DESC
                   LIMIT 1"#,
            )?;

            stmt.query_row([spec_id], Self::map_spec_version_row)
                .optional()
                .map_err(DbError::from)
        })
    }

    /// Get the version count for a spec
    pub fn get_spec_version_count(&self, spec_id: &str) -> Result<i32, DbError> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM spec_versions WHERE spec_id = ?",
                [spec_id],
                |row| row.get(0),
            )
            .map_err(DbError::from)
        })
    }

    /// Update a spec version
    pub fn update_spec_version(
        &self,
        id: &str,
        updates: &UpdateSpecVersion,
    ) -> Result<SpecVersion, DbError> {
        self.with_conn(|conn| {
            // First get existing
            let existing = {
                let mut stmt = conn.prepare(
                    r#"SELECT id, spec_id, version_number, status, exploration_log, 
                              plan_markdown, plan_json, work_started_at, created_at, updated_at
                       FROM spec_versions WHERE id = ?"#,
                )?;
                stmt.query_row([id], Self::map_spec_version_row)
                    .map_err(|e| match e {
                        rusqlite::Error::QueryReturnedNoRows => {
                            DbError::NotFound(format!("SpecVersion {}", id))
                        }
                        other => DbError::Sqlite(other),
                    })?
            };

            let now = chrono::Utc::now();
            let status = updates.status.as_ref().unwrap_or(&existing.status);
            let exploration_log = updates
                .exploration_log
                .as_ref()
                .unwrap_or(&existing.exploration_log);
            let plan_markdown = updates
                .plan_markdown
                .as_ref()
                .or(existing.plan_markdown.as_ref());
            let plan_json = updates.plan_json.as_ref().or(existing.plan_json.as_ref());
            let work_started_at = updates.work_started_at.or(existing.work_started_at);

            let exploration_json =
                serde_json::to_string(exploration_log).unwrap_or_else(|_| "[]".to_string());
            let plan_json_str =
                plan_json.map(|v| serde_json::to_string(v).unwrap_or_else(|_| "null".to_string()));
            let work_started_str = work_started_at.map(|dt| dt.to_rfc3339());

            conn.execute(
                r#"UPDATE spec_versions 
                   SET status = ?, exploration_log = ?, plan_markdown = ?, plan_json = ?, 
                       work_started_at = ?, updated_at = ?
                   WHERE id = ?"#,
                rusqlite::params![
                    status.as_str(),
                    exploration_json,
                    plan_markdown,
                    plan_json_str,
                    work_started_str,
                    now.to_rfc3339(),
                    id,
                ],
            )?;

            // Re-query to return updated
            let mut stmt = conn.prepare(
                r#"SELECT id, spec_id, version_number, status, exploration_log, 
                          plan_markdown, plan_json, work_started_at, created_at, updated_at
                   FROM spec_versions WHERE id = ?"#,
            )?;
            stmt.query_row([id], Self::map_spec_version_row)
                .map_err(DbError::Sqlite)
        })
    }

    /// Delete a spec version
    pub fn delete_spec_version(&self, id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let affected = conn.execute("DELETE FROM spec_versions WHERE id = ?", [id])?;

            if affected == 0 {
                return Err(DbError::NotFound(format!("SpecVersion {}", id)));
            }
            Ok(())
        })
    }

    /// Append an exploration entry to a spec version's log
    pub fn append_spec_version_exploration(
        &self,
        version_id: &str,
        exploration: &Exploration,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            // Get existing log
            let existing_log: Option<String> = conn
                .query_row(
                    "SELECT exploration_log FROM spec_versions WHERE id = ?",
                    [version_id],
                    |row| row.get(0),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        DbError::NotFound(format!("SpecVersion {}", version_id))
                    }
                    other => DbError::Sqlite(other),
                })?;

            let mut log: Vec<Exploration> = existing_log
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();

            log.push(exploration.clone());

            let log_json = serde_json::to_string(&log).unwrap_or_else(|_| "[]".to_string());
            let now = chrono::Utc::now().to_rfc3339();

            conn.execute(
                "UPDATE spec_versions SET exploration_log = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![log_json, now, version_id],
            )?;

            Ok(())
        })
    }

    /// Update the status of a spec version
    pub fn set_spec_version_status(
        &self,
        version_id: &str,
        status: SpecVersionStatus,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            let affected = conn.execute(
                "UPDATE spec_versions SET status = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![status.as_str(), now, version_id],
            )?;

            if affected == 0 {
                return Err(DbError::NotFound(format!("SpecVersion {}", version_id)));
            }
            Ok(())
        })
    }

    /// Set the generated plan for a spec version
    pub fn set_spec_version_plan(
        &self,
        version_id: &str,
        markdown: &str,
        json: Option<&serde_json::Value>,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            let json_str =
                json.map(|v| serde_json::to_string(v).unwrap_or_else(|_| "null".to_string()));

            let affected = conn.execute(
                "UPDATE spec_versions SET plan_markdown = ?, plan_json = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![markdown, json_str, now, version_id],
            )?;

            if affected == 0 {
                return Err(DbError::NotFound(format!("SpecVersion {}", version_id)));
            }
            Ok(())
        })
    }

    /// Pause work on a spec version - sets status to Paused
    pub fn pause_spec_version_work(&self, version_id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();

            // Check current status - can only pause if Working
            let current_status: String = conn
                .query_row(
                    "SELECT status FROM spec_versions WHERE id = ?",
                    [version_id],
                    |row| row.get(0),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        DbError::NotFound(format!("SpecVersion {}", version_id))
                    }
                    other => DbError::Sqlite(other),
                })?;

            if current_status != "working" {
                return Err(DbError::Validation(format!(
                    "Cannot pause: spec version is in '{}' status, must be 'working'",
                    current_status
                )));
            }

            conn.execute(
                "UPDATE spec_versions SET status = 'paused', updated_at = ? WHERE id = ?",
                rusqlite::params![now, version_id],
            )?;

            Ok(())
        })
    }

    /// Resume work on a paused spec version - sets status back to Working
    pub fn resume_spec_version_work(&self, version_id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();

            // Check current status - can only resume if Paused
            let current_status: String = conn
                .query_row(
                    "SELECT status FROM spec_versions WHERE id = ?",
                    [version_id],
                    |row| row.get(0),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        DbError::NotFound(format!("SpecVersion {}", version_id))
                    }
                    other => DbError::Sqlite(other),
                })?;

            if current_status != "paused" {
                return Err(DbError::Validation(format!(
                    "Cannot resume: spec version is in '{}' status, must be 'paused'",
                    current_status
                )));
            }

            // Clear paused_at on tickets but preserve stage/run info for resume
            conn.execute(
                "UPDATE tickets SET paused_at = NULL, updated_at = ? WHERE spec_version_id = ?",
                rusqlite::params![now, version_id],
            )?;

            conn.execute(
                "UPDATE spec_versions SET status = 'working', updated_at = ? WHERE id = ?",
                rusqlite::params![now, version_id],
            )?;

            Ok(())
        })
    }

    /// Halt work on a spec version - sets status to Halted and clears paused tickets
    pub fn halt_spec_version_work(&self, version_id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();

            // Check current status - can only halt if Working or Paused
            let current_status: String = conn
                .query_row(
                    "SELECT status FROM spec_versions WHERE id = ?",
                    [version_id],
                    |row| row.get(0),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        DbError::NotFound(format!("SpecVersion {}", version_id))
                    }
                    other => DbError::Sqlite(other),
                })?;

            if current_status != "working" && current_status != "paused" {
                return Err(DbError::Validation(format!(
                    "Cannot halt: spec version is in '{}' status, must be 'working' or 'paused'",
                    current_status
                )));
            }

            // Clear pause state from all tickets in this spec version
            conn.execute(
                "UPDATE tickets SET paused_at = NULL, paused_at_stage = NULL, paused_run_id = NULL, updated_at = ? WHERE spec_version_id = ?",
                rusqlite::params![now, version_id],
            )?;

            // Set spec version to halted
            conn.execute(
                "UPDATE spec_versions SET status = 'halted', updated_at = ? WHERE id = ?",
                rusqlite::params![now, version_id],
            )?;

            Ok(())
        })
    }

    /// Start work on a spec version - sets status to Working and records work_started_at
    pub fn start_spec_version_work(&self, version_id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();

            // Check current status - can only start if Executed or Halted
            let current_status: String = conn
                .query_row(
                    "SELECT status FROM spec_versions WHERE id = ?",
                    [version_id],
                    |row| row.get(0),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        DbError::NotFound(format!("SpecVersion {}", version_id))
                    }
                    other => DbError::Sqlite(other),
                })?;

            // Allow executed, halted, or completed
            if current_status != "executed"
                && current_status != "halted"
                && current_status != "completed"
            {
                return Err(DbError::Validation(format!(
                    "Cannot start work: spec version is in '{}' status, must be 'executed', 'halted', or 'completed'",
                    current_status
                )));
            }

            // Only set work_started_at if not already set (preserves original timestamp on restart after halt)
            conn.execute(
                "UPDATE spec_versions SET status = 'working', work_started_at = COALESCE(work_started_at, ?), updated_at = ? WHERE id = ?",
                rusqlite::params![now, now, version_id],
            )?;

            Ok(())
        })
    }

    /// Get all tickets created from a spec version
    pub fn get_spec_version_tickets(
        &self,
        version_id: &str,
    ) -> Result<Vec<crate::db::models::Ticket>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, column_id, title, description_md, priority, 
                          labels_json, created_at, updated_at, locked_by_run_id, 
                          lock_expires_at, project_id, workspace_id, workflow_type, model, branch_name,
                          is_epic, epic_id, order_in_epic, depends_on_epic_id, depends_on_epic_ids_json, spec_version_id,
                          paused_at, paused_at_stage, paused_run_id
                   FROM tickets WHERE spec_version_id = ?
                   ORDER BY created_at ASC"#,
            )?;

            let rows = stmt.query_map([version_id], Self::map_ticket_row)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    /// Get all epics created from a spec version
    pub fn get_spec_version_epics(
        &self,
        version_id: &str,
    ) -> Result<Vec<crate::db::models::Ticket>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, column_id, title, description_md, priority, 
                          labels_json, created_at, updated_at, locked_by_run_id, 
                          lock_expires_at, project_id, workspace_id, workflow_type, model, branch_name,
                          is_epic, epic_id, order_in_epic, depends_on_epic_id, depends_on_epic_ids_json, spec_version_id,
                          paused_at, paused_at_stage, paused_run_id
                   FROM tickets WHERE spec_version_id = ? AND is_epic = 1
                   ORDER BY created_at ASC"#,
            )?;

            let rows = stmt.query_map([version_id], Self::map_ticket_row)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    /// Get root epics (no dependencies) for a spec version
    pub fn get_spec_version_root_epics(
        &self,
        version_id: &str,
    ) -> Result<Vec<crate::db::models::Ticket>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, column_id, title, description_md, priority, 
                          labels_json, created_at, updated_at, locked_by_run_id, 
                          lock_expires_at, project_id, workspace_id, workflow_type, model, branch_name,
                          is_epic, epic_id, order_in_epic, depends_on_epic_id, depends_on_epic_ids_json, spec_version_id,
                          paused_at, paused_at_stage, paused_run_id
                   FROM tickets 
                   WHERE spec_version_id = ? AND is_epic = 1 AND depends_on_epic_id IS NULL
                   ORDER BY created_at ASC"#,
            )?;

            let rows = stmt.query_map([version_id], Self::map_ticket_row)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    /// Check if all epics for a spec version are complete (in Done column)
    pub fn are_all_spec_version_epics_done(&self, version_id: &str) -> Result<bool, DbError> {
        self.with_conn(|conn| {
            // First check if there are any epics for this spec version
            let epic_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM tickets WHERE spec_version_id = ? AND is_epic = 1",
                [version_id],
                |row| row.get(0),
            )?;

            if epic_count == 0 {
                return Ok(false); // No epics means not complete
            }

            // Check how many are in the Done column
            let done_count: i64 = conn.query_row(
                r#"SELECT COUNT(*) FROM tickets t
                   JOIN columns c ON t.column_id = c.id
                   WHERE t.spec_version_id = ? AND t.is_epic = 1 AND c.name = 'Done'"#,
                [version_id],
                |row| row.get(0),
            )?;

            Ok(done_count == epic_count)
        })
    }

    /// Get progress stats for a spec version's epics
    pub fn get_spec_version_progress(
        &self,
        version_id: &str,
    ) -> Result<crate::db::models::SpecProgress, DbError> {
        use crate::db::models::{SpecEpicStatus, SpecProgress, SpecTicketStatus};

        self.with_conn(|conn| {
            // First, get all epics with their dependency info
            let mut epic_stmt = conn.prepare(
                r#"SELECT t.id, t.title, c.name as column_name, t.depends_on_epic_ids_json
                   FROM tickets t
                   JOIN columns c ON t.column_id = c.id
                   WHERE t.spec_version_id = ? AND t.is_epic = 1
                   ORDER BY t.created_at ASC"#,
            )?;

            let epic_rows = epic_stmt.query_map([version_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })?;

            let epic_data: Vec<(String, String, String, Option<String>)> =
                epic_rows.collect::<Result<Vec<_>, _>>()?;

            // Build a map of epic id -> title for resolving dependency titles
            let mut epic_title_map: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();
            for (id, title, _, _) in &epic_data {
                epic_title_map.insert(id.clone(), title.clone());
            }

            // For each epic, get its child tickets
            let mut ticket_stmt = conn.prepare(
                r#"SELECT t.id, t.title, c.name as column_name
                   FROM tickets t
                   JOIN columns c ON t.column_id = c.id
                   WHERE t.epic_id = ?
                   ORDER BY t.order_in_epic ASC, t.created_at ASC"#,
            )?;

            let mut epics = Vec::new();
            for (epic_id, epic_title, epic_column, depends_on_json) in epic_data {
                let ticket_rows = ticket_stmt.query_map([&epic_id], |row| {
                    Ok(SpecTicketStatus {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        column: row.get(2)?,
                    })
                })?;

                let tickets: Vec<SpecTicketStatus> = ticket_rows.collect::<Result<Vec<_>, _>>()?;

                // Parse dependency IDs from JSON
                let depends_on_ids: Vec<String> = depends_on_json
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default();

                // Resolve dependency titles
                let depends_on_titles: Vec<String> = depends_on_ids
                    .iter()
                    .filter_map(|id| epic_title_map.get(id).cloned())
                    .collect();

                epics.push(SpecEpicStatus {
                    id: epic_id,
                    title: epic_title,
                    column: epic_column,
                    depends_on_ids,
                    depends_on_titles,
                    tickets,
                });
            }

            let total = epics.len();
            let done = epics.iter().filter(|e| e.column == "Done").count();
            let in_progress = epics
                .iter()
                .filter(|e| matches!(e.column.as_str(), "Ready" | "In Progress" | "Review"))
                .count();
            let blocked = epics.iter().filter(|e| e.column == "Blocked").count();

            // Get total count of ALL tickets (epics + child tickets)
            let total_tickets: usize = conn.query_row(
                "SELECT COUNT(*) FROM tickets WHERE spec_version_id = ?",
                [version_id],
                |row| row.get::<_, i64>(0),
            )? as usize;

            Ok(SpecProgress {
                total,
                done,
                in_progress,
                blocked,
                total_tickets,
                epics,
            })
        })
    }

    fn map_spec_version_row(row: &rusqlite::Row) -> rusqlite::Result<SpecVersion> {
        Self::map_spec_version_row_offset(row, 0)
    }

    /// Map a spec version row starting at a given column offset (for JOIN queries).
    pub(crate) fn map_spec_version_row_offset(row: &rusqlite::Row, off: usize) -> rusqlite::Result<SpecVersion> {
        let status_str: String = row.get(off + 3)?;
        let status = SpecVersionStatus::parse(&status_str).unwrap_or_default();

        let exploration_log_str: Option<String> = row.get(off + 4)?;
        let exploration_log: Vec<Exploration> = exploration_log_str
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let plan_json_str: Option<String> = row.get(off + 6)?;
        let plan_json = plan_json_str.and_then(|s| serde_json::from_str(&s).ok());

        let work_started_at: Option<String> = row.get(off + 7)?;

        Ok(SpecVersion {
            id: row.get(off)?,
            spec_id: row.get(off + 1)?,
            version_number: row.get(off + 2)?,
            status,
            exploration_log,
            plan_markdown: row.get(off + 5)?,
            plan_json,
            work_started_at: work_started_at.map(parse_datetime),
            created_at: parse_datetime(row.get(off + 8)?),
            updated_at: parse_datetime(row.get(off + 9)?),
        })
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::{CreateSpec, CreateSpecVersion};

    fn create_test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn temp_dir_path() -> String {
        std::env::temp_dir().to_string_lossy().to_string()
    }

    fn create_test_project(db: &Database) -> crate::db::models::Project {
        use crate::db::models::CreateProject;
        db.create_project(&CreateProject {
            name: "Test Project".to_string(),
            path: temp_dir_path(),
            requires_git: false,
        })
        .unwrap()
    }

    fn create_test_spec(db: &Database) -> (crate::db::models::Spec, SpecVersion) {
        let board = db.create_board("Test Board").unwrap();
        let project = create_test_project(db);

        let spec = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: Some(board.id.clone()),
                project_id: project.id.clone(),
                name: "Feature Plan".to_string(),
                user_input: "I want to add a new authentication system".to_string(),
                model: Some("opus".to_string()),
                settings: serde_json::json!({}),
            })
            .unwrap();

        let version = db.get_latest_spec_version(&spec.id).unwrap().unwrap();
        (spec, version)
    }

    #[test]
    fn create_and_get_spec_version() {
        let db = create_test_db();
        let (spec, version) = create_test_spec(&db);

        assert_eq!(version.spec_id, spec.id);
        assert_eq!(version.version_number, 1);
        assert_eq!(version.status, SpecVersionStatus::Conversing);
        assert!(version.exploration_log.is_empty());

        let fetched = db.get_spec_version(&version.id).unwrap();
        assert_eq!(fetched.id, version.id);
    }

    #[test]
    fn create_multiple_versions() {
        let db = create_test_db();
        let (spec, v1) = create_test_spec(&db);

        let v2 = db
            .create_spec_version(&CreateSpecVersion {
                spec_id: spec.id.clone(),
            })
            .unwrap();

        assert_eq!(v1.version_number, 1);
        assert_eq!(v2.version_number, 2);

        let versions = db.get_spec_versions(&spec.id).unwrap();
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].version_number, 1);
        assert_eq!(versions[1].version_number, 2);

        let latest = db.get_latest_spec_version(&spec.id).unwrap().unwrap();
        assert_eq!(latest.version_number, 2);
    }

    #[test]
    fn update_spec_version() {
        let db = create_test_db();
        let (_, version) = create_test_spec(&db);

        let updated = db
            .update_spec_version(
                &version.id,
                &UpdateSpecVersion {
                    status: Some(SpecVersionStatus::Exploring),
                    exploration_log: None,
                    plan_markdown: None,
                    plan_json: None,
                    work_started_at: None,
                },
            )
            .unwrap();

        assert_eq!(updated.status, SpecVersionStatus::Exploring);
    }

    #[test]
    fn append_spec_version_exploration() {
        let db = create_test_db();
        let (_, version) = create_test_spec(&db);

        let exploration = Exploration {
            query: "How does auth work?".to_string(),
            response: "Auth uses JWT tokens...".to_string(),
            timestamp: chrono::Utc::now(),
        };

        db.append_spec_version_exploration(&version.id, &exploration)
            .unwrap();

        let fetched = db.get_spec_version(&version.id).unwrap();
        assert_eq!(fetched.exploration_log.len(), 1);
        assert_eq!(fetched.exploration_log[0].query, "How does auth work?");
    }

    #[test]
    fn set_spec_version_status() {
        let db = create_test_db();
        let (_, version) = create_test_spec(&db);

        assert_eq!(version.status, SpecVersionStatus::Conversing);

        db.set_spec_version_status(&version.id, SpecVersionStatus::Completed)
            .unwrap();

        let fetched = db.get_spec_version(&version.id).unwrap();
        assert_eq!(fetched.status, SpecVersionStatus::Completed);
    }

    #[test]
    fn set_spec_version_plan() {
        let db = create_test_db();
        let (_, version) = create_test_spec(&db);

        let plan_json = serde_json::json!({
            "overview": "Test plan",
            "epics": []
        });

        db.set_spec_version_plan(&version.id, "# Test Plan\n\nOverview...", Some(&plan_json))
            .unwrap();

        let fetched = db.get_spec_version(&version.id).unwrap();
        assert!(fetched.plan_markdown.is_some());
        assert!(fetched.plan_json.is_some());
        assert_eq!(fetched.plan_json.unwrap()["overview"], "Test plan");
    }

    #[test]
    fn delete_spec_version() {
        let db = create_test_db();
        let (spec, v1) = create_test_spec(&db);

        // Create a second version
        let v2 = db
            .create_spec_version(&CreateSpecVersion {
                spec_id: spec.id.clone(),
            })
            .unwrap();

        // Delete v2
        db.delete_spec_version(&v2.id).unwrap();

        let versions = db.get_spec_versions(&spec.id).unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].id, v1.id);
    }

    #[test]
    fn spec_version_status_roundtrip() {
        for status in [
            SpecVersionStatus::Conversing,
            SpecVersionStatus::Exploring,
            SpecVersionStatus::Planning,
            SpecVersionStatus::AwaitingApproval,
            SpecVersionStatus::Approved,
            SpecVersionStatus::Executing,
            SpecVersionStatus::Executed,
            SpecVersionStatus::Working,
            SpecVersionStatus::Paused,
            SpecVersionStatus::Halted,
            SpecVersionStatus::Completed,
            SpecVersionStatus::Failed,
        ] {
            assert_eq!(SpecVersionStatus::parse(status.as_str()), Some(status));
        }
    }
}
