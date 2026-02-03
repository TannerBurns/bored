//! Database operations for specs (planning agent)

use crate::db::models::{
    CreateSpec, Exploration, Spec, SpecEpicStatus, SpecProgress, SpecStatus, SpecTicketStatus,
    UpdateSpec,
};
use crate::db::{parse_datetime, Database, DbError};

impl Database {
    pub fn create_spec(&self, input: &CreateSpec) -> Result<Spec, DbError> {
        self.with_conn(|conn| {
            let id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now();
            let settings_json = serde_json::to_string(&input.settings).unwrap_or_else(|_| "{}".to_string());
            
            conn.execute(
                r#"INSERT INTO specs 
                   (id, board_id, target_board_id, project_id, name, user_input, status, agent_pref, model, settings_json, created_at, updated_at)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
                rusqlite::params![
                    id,
                    input.board_id,
                    input.target_board_id,
                    input.project_id,
                    input.name,
                    input.user_input,
                    SpecStatus::Draft.as_str(),
                    input.agent_pref,
                    input.model,
                    settings_json,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )?;

            Ok(Spec {
                id,
                board_id: input.board_id.clone(),
                target_board_id: input.target_board_id.clone(),
                project_id: input.project_id.clone(),
                name: input.name.clone(),
                user_input: input.user_input.clone(),
                status: SpecStatus::Draft,
                agent_pref: input.agent_pref.clone(),
                model: input.model.clone(),
                exploration_log: vec![],
                plan_markdown: None,
                plan_json: None,
                settings: input.settings.clone(),
                work_started_at: None,
                created_at: now,
                updated_at: now,
            })
        })
    }

    pub fn get_spec(&self, id: &str) -> Result<Spec, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, target_board_id, project_id, name, user_input, status, agent_pref, model,
                          exploration_log, plan_markdown, plan_json, settings_json, work_started_at, created_at, updated_at
                   FROM specs WHERE id = ?"#
            )?;
            
            stmt.query_row([id], Self::map_spec_row)
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        DbError::NotFound(format!("Spec {}", id))
                    }
                    other => DbError::Sqlite(other),
                })
        })
    }

    pub fn get_specs(&self, board_id: &str) -> Result<Vec<Spec>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, target_board_id, project_id, name, user_input, status, agent_pref, model,
                          exploration_log, plan_markdown, plan_json, settings_json, work_started_at, created_at, updated_at
                   FROM specs WHERE board_id = ?
                   ORDER BY created_at DESC"#
            )?;
            
            let rows = stmt.query_map([board_id], Self::map_spec_row)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    /// Get all specs across all boards
    pub fn get_all_specs(&self) -> Result<Vec<Spec>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, target_board_id, project_id, name, user_input, status, agent_pref, model,
                          exploration_log, plan_markdown, plan_json, settings_json, work_started_at, created_at, updated_at
                   FROM specs
                   ORDER BY created_at DESC"#
            )?;
            
            let rows = stmt.query_map([], Self::map_spec_row)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    pub fn update_spec(&self, id: &str, updates: &UpdateSpec) -> Result<Spec, DbError> {
        self.with_conn(|conn| {
            // First get existing
            let existing = {
                let mut stmt = conn.prepare(
                    r#"SELECT id, board_id, target_board_id, project_id, name, user_input, status, agent_pref, model,
                              exploration_log, plan_markdown, plan_json, settings_json, work_started_at, created_at, updated_at
                       FROM specs WHERE id = ?"#
                )?;
                stmt.query_row([id], Self::map_spec_row)
                    .map_err(|e| match e {
                        rusqlite::Error::QueryReturnedNoRows => {
                            DbError::NotFound(format!("Spec {}", id))
                        }
                        other => DbError::Sqlite(other),
                    })?
            };

            let now = chrono::Utc::now();
            let name = updates.name.as_ref().unwrap_or(&existing.name);
            let user_input = updates.user_input.as_ref().unwrap_or(&existing.user_input);
            let status = updates.status.as_ref().unwrap_or(&existing.status);
            let agent_pref = updates.agent_pref.as_ref().or(existing.agent_pref.as_ref());
            let model = updates.model.as_ref().or(existing.model.as_ref());
            let exploration_log = updates.exploration_log.as_ref().unwrap_or(&existing.exploration_log);
            let plan_markdown = updates.plan_markdown.as_ref().or(existing.plan_markdown.as_ref());
            let plan_json = updates.plan_json.as_ref().or(existing.plan_json.as_ref());
            let settings = updates.settings.as_ref().unwrap_or(&existing.settings);

            let exploration_json = serde_json::to_string(exploration_log).unwrap_or_else(|_| "[]".to_string());
            let settings_json = serde_json::to_string(settings).unwrap_or_else(|_| "{}".to_string());
            let plan_json_str = plan_json.map(|v| serde_json::to_string(v).unwrap_or_else(|_| "null".to_string()));

            conn.execute(
                r#"UPDATE specs 
                   SET name = ?, user_input = ?, status = ?, agent_pref = ?, model = ?,
                       exploration_log = ?, plan_markdown = ?, plan_json = ?, settings_json = ?, updated_at = ?
                   WHERE id = ?"#,
                rusqlite::params![
                    name,
                    user_input,
                    status.as_str(),
                    agent_pref,
                    model,
                    exploration_json,
                    plan_markdown,
                    plan_json_str,
                    settings_json,
                    now.to_rfc3339(),
                    id,
                ],
            )?;

            // Re-query to return updated
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, target_board_id, project_id, name, user_input, status, agent_pref, model,
                          exploration_log, plan_markdown, plan_json, settings_json, work_started_at, created_at, updated_at
                   FROM specs WHERE id = ?"#
            )?;
            stmt.query_row([id], Self::map_spec_row)
                .map_err(DbError::Sqlite)
        })
    }

    pub fn delete_spec(&self, id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let affected = conn.execute("DELETE FROM specs WHERE id = ?", [id])?;

            if affected == 0 {
                return Err(DbError::NotFound(format!("Spec {}", id)));
            }
            Ok(())
        })
    }

    /// Delete a spec and all tickets created from it (cascade delete)
    /// Returns the number of tickets deleted
    pub fn delete_spec_with_tickets(&self, id: &str) -> Result<usize, DbError> {
        self.with_conn(|conn| {
            // First, get all ticket IDs associated with this spec
            let mut stmt = conn.prepare("SELECT id FROM tickets WHERE spec_id = ?")?;
            let ticket_ids: Vec<String> = stmt
                .query_map([id], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?;

            let ticket_count = ticket_ids.len();

            // Delete all related data for these tickets
            for ticket_id in &ticket_ids {
                // Delete comments
                conn.execute("DELETE FROM comments WHERE ticket_id = ?", [ticket_id])?;
                // Delete tasks
                conn.execute("DELETE FROM tasks WHERE ticket_id = ?", [ticket_id])?;
                // Delete events
                conn.execute("DELETE FROM events WHERE ticket_id = ?", [ticket_id])?;
                // Delete runs
                conn.execute("DELETE FROM runs WHERE ticket_id = ?", [ticket_id])?;
            }

            // Delete all tickets with this spec_id
            conn.execute("DELETE FROM tickets WHERE spec_id = ?", [id])?;

            // Delete the spec itself
            let affected = conn.execute("DELETE FROM specs WHERE id = ?", [id])?;

            if affected == 0 {
                return Err(DbError::NotFound(format!("Spec {}", id)));
            }

            Ok(ticket_count)
        })
    }

    /// Append an exploration entry to a spec's log
    pub fn append_spec_exploration(
        &self,
        id: &str,
        exploration: &Exploration,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            // Get existing log
            let existing_log: Option<String> = conn
                .query_row(
                    "SELECT exploration_log FROM specs WHERE id = ?",
                    [id],
                    |row| row.get(0),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        DbError::NotFound(format!("Spec {}", id))
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
                "UPDATE specs SET exploration_log = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![log_json, now, id],
            )?;

            Ok(())
        })
    }

    /// Update the status of a spec
    pub fn set_spec_status(&self, id: &str, status: SpecStatus) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            let affected = conn.execute(
                "UPDATE specs SET status = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![status.as_str(), now, id],
            )?;

            if affected == 0 {
                return Err(DbError::NotFound(format!("Spec {}", id)));
            }
            Ok(())
        })
    }

    /// Set the generated plan for a spec
    pub fn set_spec_plan(
        &self,
        id: &str,
        markdown: &str,
        json: Option<&serde_json::Value>,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            let json_str =
                json.map(|v| serde_json::to_string(v).unwrap_or_else(|_| "null".to_string()));

            let affected = conn.execute(
                "UPDATE specs SET plan_markdown = ?, plan_json = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![markdown, json_str, now, id],
            )?;

            if affected == 0 {
                return Err(DbError::NotFound(format!("Spec {}", id)));
            }
            Ok(())
        })
    }

    /// Pause work on a spec - sets status to Paused
    pub fn pause_spec_work(&self, id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();

            // Check current status - can only pause if Working
            let current_status: String = conn
                .query_row("SELECT status FROM specs WHERE id = ?", [id], |row| {
                    row.get(0)
                })
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        DbError::NotFound(format!("Spec {}", id))
                    }
                    other => DbError::Sqlite(other),
                })?;

            if current_status != "working" {
                return Err(DbError::Validation(format!(
                    "Cannot pause: spec is in '{}' status, must be 'working'",
                    current_status
                )));
            }

            conn.execute(
                "UPDATE specs SET status = 'paused', updated_at = ? WHERE id = ?",
                rusqlite::params![now, id],
            )?;

            Ok(())
        })
    }

    /// Resume work on a paused spec - sets status back to Working and clears ticket pause states
    pub fn resume_spec_work(&self, id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();

            // Check current status - can only resume if Paused
            let current_status: String = conn
                .query_row("SELECT status FROM specs WHERE id = ?", [id], |row| {
                    row.get(0)
                })
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        DbError::NotFound(format!("Spec {}", id))
                    }
                    other => DbError::Sqlite(other),
                })?;

            if current_status != "paused" {
                return Err(DbError::Validation(format!(
                    "Cannot resume: spec is in '{}' status, must be 'paused'",
                    current_status
                )));
            }

            // Clear paused_at but preserve stage/run info for resume
            conn.execute(
                "UPDATE tickets SET paused_at = NULL, updated_at = ? WHERE spec_id = ?",
                rusqlite::params![now, id],
            )?;

            conn.execute(
                "UPDATE specs SET status = 'working', updated_at = ? WHERE id = ?",
                rusqlite::params![now, id],
            )?;

            Ok(())
        })
    }

    /// Halt work on a spec - sets status to Halted and clears paused tickets
    pub fn halt_spec_work(&self, id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            
            // Check current status - can only halt if Working or Paused
            let current_status: String = conn.query_row(
                "SELECT status FROM specs WHERE id = ?",
                [id],
                |row| row.get(0),
            ).map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(format!("Spec {}", id)),
                other => DbError::Sqlite(other),
            })?;
            
            if current_status != "working" && current_status != "paused" {
                return Err(DbError::Validation(format!(
                    "Cannot halt: spec is in '{}' status, must be 'working' or 'paused'",
                    current_status
                )));
            }
            
            // Clear pause state from all tickets in this spec
            conn.execute(
                "UPDATE tickets SET paused_at = NULL, paused_at_stage = NULL, paused_run_id = NULL, updated_at = ? WHERE spec_id = ?",
                rusqlite::params![now, id],
            )?;
            
            // Set spec to halted
            conn.execute(
                "UPDATE specs SET status = 'halted', updated_at = ? WHERE id = ?",
                rusqlite::params![now, id],
            )?;
            
            Ok(())
        })
    }

    /// Start work on a spec - sets status to Working and records work_started_at
    pub fn start_spec_work(&self, id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            
            // Check current status - can only start if Executed or Halted
            let current_status: String = conn.query_row(
                "SELECT status FROM specs WHERE id = ?",
                [id],
                |row| row.get(0),
            ).map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(format!("Spec {}", id)),
                other => DbError::Sqlite(other),
            })?;
            
            // Allow executed, halted, or completed (command handler validates completed is valid edge case)
            if current_status != "executed" && current_status != "halted" && current_status != "completed" {
                return Err(DbError::Validation(format!(
                    "Cannot start work: spec is in '{}' status, must be 'executed', 'halted', or 'completed'",
                    current_status
                )));
            }
            
            // Only set work_started_at if not already set (preserves original timestamp on restart after halt)
            conn.execute(
                "UPDATE specs SET status = 'working', work_started_at = COALESCE(work_started_at, ?), updated_at = ? WHERE id = ?",
                rusqlite::params![now, now, id],
            )?;
            
            Ok(())
        })
    }

    /// Get all tickets created from a spec
    pub fn get_spec_tickets(
        &self,
        spec_id: &str,
    ) -> Result<Vec<crate::db::models::Ticket>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, column_id, title, description_md, priority, 
                          labels_json, created_at, updated_at, locked_by_run_id, 
                          lock_expires_at, project_id, agent_pref, workflow_type, model, branch_name,
                          is_epic, epic_id, order_in_epic, depends_on_epic_id, depends_on_epic_ids_json, spec_id,
                          paused_at, paused_at_stage, paused_run_id
                   FROM tickets WHERE spec_id = ?
                   ORDER BY created_at ASC"#
            )?;
            
            let rows = stmt.query_map([spec_id], Self::map_ticket_row_v15)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    /// Get all epics created from a spec
    pub fn get_spec_epics(&self, spec_id: &str) -> Result<Vec<crate::db::models::Ticket>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, column_id, title, description_md, priority, 
                          labels_json, created_at, updated_at, locked_by_run_id, 
                          lock_expires_at, project_id, agent_pref, workflow_type, model, branch_name,
                          is_epic, epic_id, order_in_epic, depends_on_epic_id, depends_on_epic_ids_json, spec_id,
                          paused_at, paused_at_stage, paused_run_id
                   FROM tickets WHERE spec_id = ? AND is_epic = 1
                   ORDER BY created_at ASC"#
            )?;
            
            let rows = stmt.query_map([spec_id], Self::map_ticket_row_v15)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    /// Get root epics (no dependencies) for a spec
    pub fn get_spec_root_epics(
        &self,
        spec_id: &str,
    ) -> Result<Vec<crate::db::models::Ticket>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, column_id, title, description_md, priority, 
                          labels_json, created_at, updated_at, locked_by_run_id, 
                          lock_expires_at, project_id, agent_pref, workflow_type, model, branch_name,
                          is_epic, epic_id, order_in_epic, depends_on_epic_id, depends_on_epic_ids_json, spec_id,
                          paused_at, paused_at_stage, paused_run_id
                   FROM tickets 
                   WHERE spec_id = ? AND is_epic = 1 AND depends_on_epic_id IS NULL
                   ORDER BY created_at ASC"#
            )?;
            
            let rows = stmt.query_map([spec_id], Self::map_ticket_row_v15)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    /// Check if all epics for a spec are complete (in Done column)
    pub fn are_all_spec_epics_done(&self, spec_id: &str) -> Result<bool, DbError> {
        self.with_conn(|conn| {
            // First check if there are any epics for this spec
            let epic_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM tickets WHERE spec_id = ? AND is_epic = 1",
                [spec_id],
                |row| row.get(0),
            )?;

            if epic_count == 0 {
                return Ok(false); // No epics means not complete
            }

            // Check how many are in the Done column
            let done_count: i64 = conn.query_row(
                r#"SELECT COUNT(*) FROM tickets t
                   JOIN columns c ON t.column_id = c.id
                   WHERE t.spec_id = ? AND t.is_epic = 1 AND c.name = 'Done'"#,
                [spec_id],
                |row| row.get(0),
            )?;

            Ok(done_count == epic_count)
        })
    }

    /// Get progress stats for a spec's epics
    pub fn get_spec_progress(&self, spec_id: &str) -> Result<SpecProgress, DbError> {
        self.with_conn(|conn| {
            // First, get all epics with their dependency info (using JSON array for multiple deps)
            let mut epic_stmt = conn.prepare(
                r#"SELECT t.id, t.title, c.name as column_name, t.depends_on_epic_ids_json
                   FROM tickets t
                   JOIN columns c ON t.column_id = c.id
                   WHERE t.spec_id = ? AND t.is_epic = 1
                   ORDER BY t.created_at ASC"#,
            )?;

            let epic_rows = epic_stmt.query_map([spec_id], |row| {
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
                "SELECT COUNT(*) FROM tickets WHERE spec_id = ?",
                [spec_id],
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

    fn map_spec_row(row: &rusqlite::Row) -> rusqlite::Result<Spec> {
        // Column order: 0-id, 1-board_id, 2-target_board_id, 3-project_id, 4-name, 5-user_input,
        //               6-status, 7-agent_pref, 8-model, 9-exploration_log, 10-plan_markdown,
        //               11-plan_json, 12-settings_json, 13-work_started_at, 14-created_at, 15-updated_at
        let status_str: String = row.get(6)?;
        let status = SpecStatus::parse(&status_str).unwrap_or_default();

        let exploration_log_str: Option<String> = row.get(9)?;
        let exploration_log: Vec<Exploration> = exploration_log_str
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let plan_json_str: Option<String> = row.get(11)?;
        let plan_json = plan_json_str.and_then(|s| serde_json::from_str(&s).ok());

        let settings_str: String = row
            .get::<_, Option<String>>(12)?
            .unwrap_or_else(|| "{}".to_string());
        let settings =
            serde_json::from_str(&settings_str).unwrap_or_else(|_| serde_json::json!({}));

        let work_started_at: Option<String> = row.get(13)?;

        Ok(Spec {
            id: row.get(0)?,
            board_id: row.get(1)?,
            target_board_id: row.get(2)?,
            project_id: row.get(3)?,
            name: row.get(4)?,
            user_input: row.get(5)?,
            status,
            agent_pref: row.get(7)?,
            model: row.get(8)?,
            exploration_log,
            plan_markdown: row.get(10)?,
            plan_json,
            settings,
            work_started_at: work_started_at.map(parse_datetime),
            created_at: parse_datetime(row.get(14)?),
            updated_at: parse_datetime(row.get(15)?),
        })
    }

    // Helper to map ticket rows with spec_id column (v15)
    fn map_ticket_row_v15(row: &rusqlite::Row) -> rusqlite::Result<crate::db::models::Ticket> {
        use crate::db::models::{AgentPref, Priority, Ticket, WorkflowType};

        let labels_json: String = row.get(6)?;
        let labels: Vec<String> = serde_json::from_str(&labels_json).unwrap_or_default();

        let priority_str: String = row.get(5)?;
        let priority = Priority::parse(&priority_str).unwrap_or(Priority::Medium);

        let agent_pref_str: Option<String> = row.get(12)?;
        let agent_pref = agent_pref_str.and_then(|s| AgentPref::parse(&s));

        let workflow_type_str: String = row
            .get::<_, Option<String>>(13)?
            .unwrap_or_else(|| "basic".to_string());
        let workflow_type = WorkflowType::parse(&workflow_type_str).unwrap_or_default();

        let model: Option<String> = row.get(14)?;
        let branch_name: Option<String> = row.get(15)?;

        let is_epic: bool = row.get::<_, i32>(16).unwrap_or(0) != 0;
        let epic_id: Option<String> = row.get(17)?;
        let order_in_epic: Option<i32> = row.get(18)?;
        let depends_on_epic_id: Option<String> = row.get(19)?;
        let depends_on_epic_ids_json: Option<String> = row.get(20)?;
        let depends_on_epic_ids: Vec<String> = depends_on_epic_ids_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let spec_id: Option<String> = row.get(21)?;

        // Pause fields (columns 22, 23, 24)
        let paused_at: Option<String> = row.get(22)?;
        let paused_at_stage: Option<String> = row.get(23)?;
        let paused_run_id: Option<String> = row.get(24)?;

        Ok(Ticket {
            id: row.get(0)?,
            board_id: row.get(1)?,
            column_id: row.get(2)?,
            title: row.get(3)?,
            description_md: row.get(4)?,
            priority,
            labels,
            created_at: parse_datetime(row.get(7)?),
            updated_at: parse_datetime(row.get(8)?),
            locked_by_run_id: row.get(9)?,
            lock_expires_at: row.get::<_, Option<String>>(10)?.map(parse_datetime),
            project_id: row.get(11)?,
            agent_pref,
            workflow_type,
            model,
            branch_name,
            is_epic,
            epic_id,
            order_in_epic,
            depends_on_epic_id,
            depends_on_epic_ids,
            spec_id,
            paused_at: paused_at.map(parse_datetime),
            paused_at_stage,
            paused_run_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            preferred_agent: None,
            requires_git: false,
        })
        .unwrap()
    }

    #[test]
    fn create_and_get_spec() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let project = create_test_project(&db);

        let spec = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: Some(board.id.clone()),
                project_id: project.id.clone(),
                name: "Feature Plan".to_string(),
                user_input: "I want to add a new authentication system".to_string(),
                agent_pref: Some("claude".to_string()),
                model: Some("opus".to_string()),
                settings: serde_json::json!({}),
            })
            .unwrap();

        assert_eq!(spec.name, "Feature Plan");
        assert_eq!(spec.project_id, project.id);
        assert_eq!(spec.agent_pref, Some("claude".to_string()));
        assert_eq!(spec.model, Some("opus".to_string()));
        assert_eq!(spec.status, SpecStatus::Draft);
        assert!(spec.exploration_log.is_empty());

        let fetched = db.get_spec(&spec.id).unwrap();
        assert_eq!(fetched.id, spec.id);
        assert_eq!(
            fetched.user_input,
            "I want to add a new authentication system"
        );
        assert_eq!(fetched.project_id, project.id);
    }

    #[test]
    fn get_specs_for_board() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let project = create_test_project(&db);

        db.create_spec(&CreateSpec {
            board_id: board.id.clone(),
            target_board_id: Some(board.id.clone()),
            project_id: project.id.clone(),
            name: "Plan 1".to_string(),
            user_input: "Input 1".to_string(),
            agent_pref: None,
            model: None,
            settings: serde_json::json!({}),
        })
        .unwrap();

        db.create_spec(&CreateSpec {
            board_id: board.id.clone(),
            target_board_id: Some(board.id.clone()),
            project_id: project.id.clone(),
            name: "Plan 2".to_string(),
            user_input: "Input 2".to_string(),
            agent_pref: None,
            model: None,
            settings: serde_json::json!({}),
        })
        .unwrap();

        let specs = db.get_specs(&board.id).unwrap();
        assert_eq!(specs.len(), 2);
    }

    #[test]
    fn update_spec() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let project = create_test_project(&db);

        let spec = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: Some(board.id.clone()),
                project_id: project.id.clone(),
                name: "Original".to_string(),
                user_input: "Original input".to_string(),
                agent_pref: None,
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();

        let updated = db
            .update_spec(
                &spec.id,
                &UpdateSpec {
                    name: Some("Updated".to_string()),
                    user_input: None,
                    status: Some(SpecStatus::Exploring),
                    agent_pref: Some("cursor".to_string()),
                    model: Some("sonnet".to_string()),
                    exploration_log: None,
                    plan_markdown: None,
                    plan_json: None,
                    settings: None,
                },
            )
            .unwrap();

        assert_eq!(updated.name, "Updated");
        assert_eq!(updated.user_input, "Original input");
        assert_eq!(updated.status, SpecStatus::Exploring);
        assert_eq!(updated.agent_pref, Some("cursor".to_string()));
        assert_eq!(updated.model, Some("sonnet".to_string()));
    }

    #[test]
    fn append_spec_exploration() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let project = create_test_project(&db);

        let spec = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: Some(board.id.clone()),
                project_id: project.id.clone(),
                name: "Plan".to_string(),
                user_input: "Input".to_string(),
                agent_pref: None,
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();

        let exploration = Exploration {
            query: "How does auth work?".to_string(),
            response: "Auth uses JWT tokens...".to_string(),
            timestamp: chrono::Utc::now(),
        };

        db.append_spec_exploration(&spec.id, &exploration).unwrap();

        let fetched = db.get_spec(&spec.id).unwrap();
        assert_eq!(fetched.exploration_log.len(), 1);
        assert_eq!(fetched.exploration_log[0].query, "How does auth work?");
    }

    #[test]
    fn set_spec_status() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let project = create_test_project(&db);

        let spec = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: Some(board.id.clone()),
                project_id: project.id.clone(),
                name: "Plan".to_string(),
                user_input: "Input".to_string(),
                agent_pref: None,
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();

        assert_eq!(spec.status, SpecStatus::Draft);

        db.set_spec_status(&spec.id, SpecStatus::Completed).unwrap();

        let fetched = db.get_spec(&spec.id).unwrap();
        assert_eq!(fetched.status, SpecStatus::Completed);
    }

    #[test]
    fn set_spec_plan() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let project = create_test_project(&db);

        let spec = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: Some(board.id.clone()),
                project_id: project.id.clone(),
                name: "Plan".to_string(),
                user_input: "Input".to_string(),
                agent_pref: None,
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();

        let plan_json = serde_json::json!({
            "overview": "Test plan",
            "epics": []
        });

        db.set_spec_plan(&spec.id, "# Test Plan\n\nOverview...", Some(&plan_json))
            .unwrap();

        let fetched = db.get_spec(&spec.id).unwrap();
        assert!(fetched.plan_markdown.is_some());
        assert!(fetched.plan_json.is_some());
        assert_eq!(fetched.plan_json.unwrap()["overview"], "Test plan");
    }

    #[test]
    fn delete_spec() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let project = create_test_project(&db);

        let spec = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: Some(board.id.clone()),
                project_id: project.id.clone(),
                name: "Plan".to_string(),
                user_input: "Input".to_string(),
                agent_pref: None,
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();

        db.delete_spec(&spec.id).unwrap();

        let result = db.get_spec(&spec.id);
        assert!(matches!(result, Err(DbError::NotFound(_))));
    }

    #[test]
    fn delete_spec_not_found() {
        let db = create_test_db();
        let result = db.delete_spec("nonexistent");
        assert!(matches!(result, Err(DbError::NotFound(_))));
    }

    #[test]
    fn spec_status_roundtrip() {
        for status in [
            SpecStatus::Draft,
            SpecStatus::Exploring,
            SpecStatus::Planning,
            SpecStatus::AwaitingApproval,
            SpecStatus::Approved,
            SpecStatus::Executing,
            SpecStatus::Executed,
            SpecStatus::Working,
            SpecStatus::Paused,
            SpecStatus::Halted,
            SpecStatus::Completed,
            SpecStatus::Failed,
        ] {
            assert_eq!(SpecStatus::parse(status.as_str()), Some(status));
        }
    }

    // ===== Pause/Resume/Halt Tests =====

    fn create_working_spec(db: &Database) -> crate::db::models::Spec {
        let board = db.create_board("Test Board").unwrap();
        let project = create_test_project(db);

        let spec = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: Some(board.id.clone()),
                project_id: project.id.clone(),
                name: "Work Plan".to_string(),
                user_input: "Test".to_string(),
                agent_pref: None,
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();

        // Move through the workflow to 'executed' then 'working'
        db.set_spec_status(&spec.id, SpecStatus::Executed).unwrap();
        db.start_spec_work(&spec.id).unwrap();

        db.get_spec(&spec.id).unwrap()
    }

    #[test]
    fn pause_spec_work_success() {
        let db = create_test_db();
        let spec = create_working_spec(&db);

        assert_eq!(spec.status, SpecStatus::Working);

        db.pause_spec_work(&spec.id).unwrap();

        let paused = db.get_spec(&spec.id).unwrap();
        assert_eq!(paused.status, SpecStatus::Paused);
    }

    #[test]
    fn pause_spec_work_fails_if_not_working() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let project = create_test_project(&db);

        let spec = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: Some(board.id.clone()),
                project_id: project.id.clone(),
                name: "Plan".to_string(),
                user_input: "Test".to_string(),
                agent_pref: None,
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();

        // Status is Draft, not Working
        let result = db.pause_spec_work(&spec.id);
        assert!(matches!(result, Err(DbError::Validation(_))));
    }

    #[test]
    fn pause_spec_work_not_found() {
        let db = create_test_db();
        let result = db.pause_spec_work("nonexistent");
        assert!(matches!(result, Err(DbError::NotFound(_))));
    }

    #[test]
    fn resume_spec_work_success() {
        let db = create_test_db();
        let spec = create_working_spec(&db);

        db.pause_spec_work(&spec.id).unwrap();

        let paused = db.get_spec(&spec.id).unwrap();
        assert_eq!(paused.status, SpecStatus::Paused);

        db.resume_spec_work(&spec.id).unwrap();

        let resumed = db.get_spec(&spec.id).unwrap();
        assert_eq!(resumed.status, SpecStatus::Working);
    }

    #[test]
    fn resume_spec_work_fails_if_not_paused() {
        let db = create_test_db();
        let spec = create_working_spec(&db);

        // Status is Working, not Paused
        let result = db.resume_spec_work(&spec.id);
        assert!(matches!(result, Err(DbError::Validation(_))));
    }

    #[test]
    fn resume_spec_work_not_found() {
        let db = create_test_db();
        let result = db.resume_spec_work("nonexistent");
        assert!(matches!(result, Err(DbError::NotFound(_))));
    }

    #[test]
    fn resume_spec_work_clears_ticket_pause_state() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let project = create_test_project(&db);
        let columns = db.get_columns(&board.id).unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();

        let spec = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: Some(board.id.clone()),
                project_id: project.id.clone(),
                name: "Plan".to_string(),
                user_input: "Test".to_string(),
                agent_pref: None,
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();

        // Create a ticket linked to the spec
        let ticket = db
            .create_ticket(&crate::db::models::CreateTicket {
                board_id: board.id.clone(),
                column_id: ready.id.clone(),
                title: "T1".to_string(),
                description_md: "".to_string(),
                priority: crate::db::models::Priority::Medium,
                labels: vec![],
                project_id: None,
                agent_pref: None,
                workflow_type: crate::db::models::WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_id: Some(spec.id.clone()),
            })
            .unwrap();

        // Set to working, pause ticket, then pause spec
        db.set_spec_status(&spec.id, SpecStatus::Working).unwrap();
        db.pause_ticket(&ticket.id, "impl", "run-1").unwrap();

        assert!(db.is_ticket_paused(&ticket.id).unwrap());

        db.pause_spec_work(&spec.id).unwrap();

        // Resume the spec - ticket pause state should be cleared
        db.resume_spec_work(&spec.id).unwrap();

        // Verify spec is working again
        let resumed = db.get_spec(&spec.id).unwrap();
        assert_eq!(resumed.status, SpecStatus::Working);

        // Verify ticket pause state is cleared so workers can pick it up
        assert!(!db.is_ticket_paused(&ticket.id).unwrap());
    }

    #[test]
    fn resume_spec_work_preserves_paused_at_stage() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let project = create_test_project(&db);
        let columns = db.get_columns(&board.id).unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();

        let spec = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: Some(board.id.clone()),
                project_id: project.id.clone(),
                name: "Plan".to_string(),
                user_input: "Test".to_string(),
                agent_pref: None,
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();

        // Create a ticket linked to the spec
        let ticket = db
            .create_ticket(&crate::db::models::CreateTicket {
                board_id: board.id.clone(),
                column_id: ready.id.clone(),
                title: "T1".to_string(),
                description_md: "".to_string(),
                priority: crate::db::models::Priority::Medium,
                labels: vec![],
                project_id: None,
                agent_pref: None,
                workflow_type: crate::db::models::WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_id: Some(spec.id.clone()),
            })
            .unwrap();

        // Set to working, pause ticket at "implement" stage, then pause spec
        db.set_spec_status(&spec.id, SpecStatus::Working).unwrap();
        db.pause_ticket(&ticket.id, "implement", "run-123").unwrap();

        let paused = db.get_ticket(&ticket.id).unwrap();
        assert_eq!(paused.paused_at_stage, Some("implement".to_string()));

        db.pause_spec_work(&spec.id).unwrap();

        // Resume the spec
        db.resume_spec_work(&spec.id).unwrap();

        // Verify ticket is no longer paused (paused_at cleared)
        assert!(!db.is_ticket_paused(&ticket.id).unwrap());

        // Both paused_at_stage and paused_run_id should be preserved so worker can resume
        // the same run from the same stage
        let resumed_ticket = db.get_ticket(&ticket.id).unwrap();
        assert_eq!(
            resumed_ticket.paused_at_stage,
            Some("implement".to_string())
        );
        // paused_run_id is preserved so the same run can be reused for continuity
        assert_eq!(resumed_ticket.paused_run_id, Some("run-123".to_string()));
    }

    #[test]
    fn resume_spec_work_preserves_paused_run_id() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let project = create_test_project(&db);
        let columns = db.get_columns(&board.id).unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();

        let spec = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: Some(board.id.clone()),
                project_id: project.id.clone(),
                name: "Plan".to_string(),
                user_input: "Test".to_string(),
                agent_pref: None,
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();

        // Create a ticket linked to the spec
        let ticket = db
            .create_ticket(&crate::db::models::CreateTicket {
                board_id: board.id.clone(),
                column_id: ready.id.clone(),
                title: "T1".to_string(),
                description_md: "".to_string(),
                priority: crate::db::models::Priority::Medium,
                labels: vec![],
                project_id: None,
                agent_pref: None,
                workflow_type: crate::db::models::WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_id: Some(spec.id.clone()),
            })
            .unwrap();

        // Set to working and pause the ticket with a run ID
        db.set_spec_status(&spec.id, SpecStatus::Working).unwrap();
        db.pause_ticket(&ticket.id, "review", "run-xyz-123")
            .unwrap();

        // Verify the run ID was saved
        let paused_ticket = db.get_ticket(&ticket.id).unwrap();
        assert_eq!(paused_ticket.paused_run_id, Some("run-xyz-123".to_string()));

        db.pause_spec_work(&spec.id).unwrap();
        db.resume_spec_work(&spec.id).unwrap();

        // paused_run_id should be preserved so the same run can be reused for continuity
        // paused_at_stage should be preserved so worker knows where to resume
        let resumed_ticket = db.get_ticket(&ticket.id).unwrap();
        assert_eq!(
            resumed_ticket.paused_run_id,
            Some("run-xyz-123".to_string())
        );
        assert_eq!(resumed_ticket.paused_at_stage, Some("review".to_string()));
    }

    #[test]
    fn halt_spec_work_from_working() {
        let db = create_test_db();
        let spec = create_working_spec(&db);

        db.halt_spec_work(&spec.id).unwrap();

        let halted = db.get_spec(&spec.id).unwrap();
        assert_eq!(halted.status, SpecStatus::Halted);
    }

    #[test]
    fn halt_spec_work_from_paused() {
        let db = create_test_db();
        let spec = create_working_spec(&db);

        db.pause_spec_work(&spec.id).unwrap();
        db.halt_spec_work(&spec.id).unwrap();

        let halted = db.get_spec(&spec.id).unwrap();
        assert_eq!(halted.status, SpecStatus::Halted);
    }

    #[test]
    fn halt_spec_work_clears_ticket_pause_state() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let project = create_test_project(&db);
        let columns = db.get_columns(&board.id).unwrap();
        let ready = columns.iter().find(|c| c.name == "Ready").unwrap();

        let spec = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: Some(board.id.clone()),
                project_id: project.id.clone(),
                name: "Plan".to_string(),
                user_input: "Test".to_string(),
                agent_pref: None,
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();

        // Create a ticket linked to the spec
        let ticket = db
            .create_ticket(&crate::db::models::CreateTicket {
                board_id: board.id.clone(),
                column_id: ready.id.clone(),
                title: "T1".to_string(),
                description_md: "".to_string(),
                priority: crate::db::models::Priority::Medium,
                labels: vec![],
                project_id: None,
                agent_pref: None,
                workflow_type: crate::db::models::WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_id: Some(spec.id.clone()),
            })
            .unwrap();

        // Set to working and pause the ticket
        db.set_spec_status(&spec.id, SpecStatus::Working).unwrap();
        db.pause_ticket(&ticket.id, "impl", "run-1").unwrap();

        assert!(db.is_ticket_paused(&ticket.id).unwrap());

        // Halt the spec
        db.halt_spec_work(&spec.id).unwrap();

        // Ticket pause state should be cleared
        assert!(!db.is_ticket_paused(&ticket.id).unwrap());
    }

    #[test]
    fn halt_spec_work_fails_if_wrong_status() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let project = create_test_project(&db);

        let spec = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: Some(board.id.clone()),
                project_id: project.id.clone(),
                name: "Plan".to_string(),
                user_input: "Test".to_string(),
                agent_pref: None,
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();

        // Status is Draft
        let result = db.halt_spec_work(&spec.id);
        assert!(matches!(result, Err(DbError::Validation(_))));
    }

    #[test]
    fn halt_spec_work_not_found() {
        let db = create_test_db();
        let result = db.halt_spec_work("nonexistent");
        assert!(matches!(result, Err(DbError::NotFound(_))));
    }

    #[test]
    fn start_spec_work_from_executed() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let project = create_test_project(&db);

        let spec = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: Some(board.id.clone()),
                project_id: project.id.clone(),
                name: "Plan".to_string(),
                user_input: "Test".to_string(),
                agent_pref: None,
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();

        db.set_spec_status(&spec.id, SpecStatus::Executed).unwrap();

        db.start_spec_work(&spec.id).unwrap();

        let working = db.get_spec(&spec.id).unwrap();
        assert_eq!(working.status, SpecStatus::Working);
        assert!(working.work_started_at.is_some());
    }

    #[test]
    fn start_spec_work_from_halted() {
        let db = create_test_db();
        let spec = create_working_spec(&db);

        db.halt_spec_work(&spec.id).unwrap();

        let halted = db.get_spec(&spec.id).unwrap();
        assert_eq!(halted.status, SpecStatus::Halted);

        db.start_spec_work(&spec.id).unwrap();

        let restarted = db.get_spec(&spec.id).unwrap();
        assert_eq!(restarted.status, SpecStatus::Working);
    }

    #[test]
    fn start_spec_work_fails_if_wrong_status() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let project = create_test_project(&db);

        let spec = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: Some(board.id.clone()),
                project_id: project.id.clone(),
                name: "Plan".to_string(),
                user_input: "Test".to_string(),
                agent_pref: None,
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();

        // Status is Draft
        let result = db.start_spec_work(&spec.id);
        assert!(matches!(result, Err(DbError::Validation(_))));
    }

    #[test]
    fn start_spec_work_not_found() {
        let db = create_test_db();
        let result = db.start_spec_work("nonexistent");
        assert!(matches!(result, Err(DbError::NotFound(_))));
    }

    #[test]
    fn work_started_at_is_set_on_start() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let project = create_test_project(&db);

        let spec = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: Some(board.id.clone()),
                project_id: project.id.clone(),
                name: "Plan".to_string(),
                user_input: "Test".to_string(),
                agent_pref: None,
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();

        assert!(spec.work_started_at.is_none());

        db.set_spec_status(&spec.id, SpecStatus::Executed).unwrap();
        db.start_spec_work(&spec.id).unwrap();

        let started = db.get_spec(&spec.id).unwrap();
        assert!(started.work_started_at.is_some());
    }
}
