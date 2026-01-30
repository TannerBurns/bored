//! Database operations for scratchpads (planner agent)

use crate::db::{Database, DbError, parse_datetime};
use crate::db::models::{Scratchpad, CreateScratchpad, UpdateScratchpad, ScratchpadStatus, Exploration};

impl Database {
    pub fn create_scratchpad(&self, input: &CreateScratchpad) -> Result<Scratchpad, DbError> {
        self.with_conn(|conn| {
            let id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now();
            let settings_json = serde_json::to_string(&input.settings).unwrap_or_else(|_| "{}".to_string());
            
            conn.execute(
                r#"INSERT INTO scratchpads 
                   (id, board_id, project_id, name, user_input, status, agent_pref, model, settings_json, created_at, updated_at)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
                rusqlite::params![
                    id,
                    input.board_id,
                    input.project_id,
                    input.name,
                    input.user_input,
                    ScratchpadStatus::Draft.as_str(),
                    input.agent_pref,
                    input.model,
                    settings_json,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )?;

            Ok(Scratchpad {
                id,
                board_id: input.board_id.clone(),
                project_id: input.project_id.clone(),
                name: input.name.clone(),
                user_input: input.user_input.clone(),
                status: ScratchpadStatus::Draft,
                agent_pref: input.agent_pref.clone(),
                model: input.model.clone(),
                exploration_log: vec![],
                plan_markdown: None,
                plan_json: None,
                settings: input.settings.clone(),
                created_at: now,
                updated_at: now,
            })
        })
    }

    pub fn get_scratchpad(&self, id: &str) -> Result<Scratchpad, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, project_id, name, user_input, status, agent_pref, model,
                          exploration_log, plan_markdown, plan_json, settings_json, created_at, updated_at
                   FROM scratchpads WHERE id = ?"#
            )?;
            
            stmt.query_row([id], Self::map_scratchpad_row)
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        DbError::NotFound(format!("Scratchpad {}", id))
                    }
                    other => DbError::Sqlite(other),
                })
        })
    }

    pub fn get_scratchpads(&self, board_id: &str) -> Result<Vec<Scratchpad>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, project_id, name, user_input, status, agent_pref, model,
                          exploration_log, plan_markdown, plan_json, settings_json, created_at, updated_at
                   FROM scratchpads WHERE board_id = ?
                   ORDER BY created_at DESC"#
            )?;
            
            let rows = stmt.query_map([board_id], Self::map_scratchpad_row)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    pub fn update_scratchpad(&self, id: &str, updates: &UpdateScratchpad) -> Result<Scratchpad, DbError> {
        self.with_conn(|conn| {
            // First get existing
            let existing = {
                let mut stmt = conn.prepare(
                    r#"SELECT id, board_id, project_id, name, user_input, status, agent_pref, model,
                              exploration_log, plan_markdown, plan_json, settings_json, created_at, updated_at
                       FROM scratchpads WHERE id = ?"#
                )?;
                stmt.query_row([id], Self::map_scratchpad_row)
                    .map_err(|e| match e {
                        rusqlite::Error::QueryReturnedNoRows => {
                            DbError::NotFound(format!("Scratchpad {}", id))
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
                r#"UPDATE scratchpads 
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
                r#"SELECT id, board_id, project_id, name, user_input, status, agent_pref, model,
                          exploration_log, plan_markdown, plan_json, settings_json, created_at, updated_at
                   FROM scratchpads WHERE id = ?"#
            )?;
            stmt.query_row([id], Self::map_scratchpad_row)
                .map_err(DbError::Sqlite)
        })
    }

    pub fn delete_scratchpad(&self, id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let affected = conn.execute(
                "DELETE FROM scratchpads WHERE id = ?",
                [id],
            )?;
            
            if affected == 0 {
                return Err(DbError::NotFound(format!("Scratchpad {}", id)));
            }
            Ok(())
        })
    }

    /// Append an exploration entry to a scratchpad's log
    pub fn append_exploration(&self, id: &str, exploration: &Exploration) -> Result<(), DbError> {
        self.with_conn(|conn| {
            // Get existing log
            let existing_log: Option<String> = conn.query_row(
                "SELECT exploration_log FROM scratchpads WHERE id = ?",
                [id],
                |row| row.get(0),
            ).map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(format!("Scratchpad {}", id)),
                other => DbError::Sqlite(other),
            })?;

            let mut log: Vec<Exploration> = existing_log
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
            
            log.push(exploration.clone());
            
            let log_json = serde_json::to_string(&log).unwrap_or_else(|_| "[]".to_string());
            let now = chrono::Utc::now().to_rfc3339();
            
            conn.execute(
                "UPDATE scratchpads SET exploration_log = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![log_json, now, id],
            )?;
            
            Ok(())
        })
    }

    /// Update the status of a scratchpad
    pub fn set_scratchpad_status(&self, id: &str, status: ScratchpadStatus) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            let affected = conn.execute(
                "UPDATE scratchpads SET status = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![status.as_str(), now, id],
            )?;
            
            if affected == 0 {
                return Err(DbError::NotFound(format!("Scratchpad {}", id)));
            }
            Ok(())
        })
    }

    /// Set the generated plan for a scratchpad
    pub fn set_scratchpad_plan(&self, id: &str, markdown: &str, json: Option<&serde_json::Value>) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            let json_str = json.map(|v| serde_json::to_string(v).unwrap_or_else(|_| "null".to_string()));
            
            let affected = conn.execute(
                "UPDATE scratchpads SET plan_markdown = ?, plan_json = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![markdown, json_str, now, id],
            )?;
            
            if affected == 0 {
                return Err(DbError::NotFound(format!("Scratchpad {}", id)));
            }
            Ok(())
        })
    }

    /// Get all tickets created from a scratchpad
    pub fn get_scratchpad_tickets(&self, scratchpad_id: &str) -> Result<Vec<crate::db::models::Ticket>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, column_id, title, description_md, priority, 
                          labels_json, created_at, updated_at, locked_by_run_id, 
                          lock_expires_at, project_id, agent_pref, workflow_type, model, branch_name,
                          is_epic, epic_id, order_in_epic, depends_on_epic_id, scratchpad_id
                   FROM tickets WHERE scratchpad_id = ?
                   ORDER BY created_at ASC"#
            )?;
            
            let rows = stmt.query_map([scratchpad_id], Self::map_ticket_row_v10)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    fn map_scratchpad_row(row: &rusqlite::Row) -> rusqlite::Result<Scratchpad> {
        // Column order: id, board_id, project_id, name, user_input, status, agent_pref, model,
        //               exploration_log, plan_markdown, plan_json, settings_json, created_at, updated_at
        let status_str: String = row.get(5)?;
        let status = ScratchpadStatus::parse(&status_str).unwrap_or_default();
        
        let exploration_log_str: Option<String> = row.get(8)?;
        let exploration_log: Vec<Exploration> = exploration_log_str
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        
        let plan_json_str: Option<String> = row.get(10)?;
        let plan_json = plan_json_str.and_then(|s| serde_json::from_str(&s).ok());
        
        let settings_str: String = row.get::<_, Option<String>>(11)?.unwrap_or_else(|| "{}".to_string());
        let settings = serde_json::from_str(&settings_str).unwrap_or_else(|_| serde_json::json!({}));

        Ok(Scratchpad {
            id: row.get(0)?,
            board_id: row.get(1)?,
            project_id: row.get(2)?,
            name: row.get(3)?,
            user_input: row.get(4)?,
            status,
            agent_pref: row.get(6)?,
            model: row.get(7)?,
            exploration_log,
            plan_markdown: row.get(9)?,
            plan_json,
            settings,
            created_at: parse_datetime(row.get(12)?),
            updated_at: parse_datetime(row.get(13)?),
        })
    }

    // Temporary helper to map ticket rows with new columns (v10)
    // This will be consolidated with map_ticket_row once tickets.rs is updated
    fn map_ticket_row_v10(row: &rusqlite::Row) -> rusqlite::Result<crate::db::models::Ticket> {
        use crate::db::models::{Ticket, Priority, AgentPref, WorkflowType};
        
        let labels_json: String = row.get(6)?;
        let labels: Vec<String> = serde_json::from_str(&labels_json).unwrap_or_default();
        
        let priority_str: String = row.get(5)?;
        let priority = Priority::parse(&priority_str).unwrap_or(Priority::Medium);
        
        let agent_pref_str: Option<String> = row.get(12)?;
        let agent_pref = agent_pref_str.and_then(|s| AgentPref::parse(&s));
        
        let workflow_type_str: String = row.get::<_, Option<String>>(13)?.unwrap_or_else(|| "basic".to_string());
        let workflow_type = WorkflowType::parse(&workflow_type_str).unwrap_or_default();
        
        let model: Option<String> = row.get(14)?;
        let branch_name: Option<String> = row.get(15)?;
        
        let is_epic: bool = row.get::<_, i32>(16).unwrap_or(0) != 0;
        let epic_id: Option<String> = row.get(17)?;
        let order_in_epic: Option<i32> = row.get(18)?;
        let depends_on_epic_id: Option<String> = row.get(19)?;
        let scratchpad_id: Option<String> = row.get(20)?;

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
            scratchpad_id,
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
        }).unwrap()
    }

    #[test]
    fn create_and_get_scratchpad() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let project = create_test_project(&db);
        
        let scratchpad = db.create_scratchpad(&CreateScratchpad {
            board_id: board.id.clone(),
            project_id: project.id.clone(),
            name: "Feature Plan".to_string(),
            user_input: "I want to add a new authentication system".to_string(),
            agent_pref: Some("claude".to_string()),
            model: Some("opus".to_string()),
            settings: serde_json::json!({}),
        }).unwrap();
        
        assert_eq!(scratchpad.name, "Feature Plan");
        assert_eq!(scratchpad.project_id, project.id);
        assert_eq!(scratchpad.agent_pref, Some("claude".to_string()));
        assert_eq!(scratchpad.model, Some("opus".to_string()));
        assert_eq!(scratchpad.status, ScratchpadStatus::Draft);
        assert!(scratchpad.exploration_log.is_empty());
        
        let fetched = db.get_scratchpad(&scratchpad.id).unwrap();
        assert_eq!(fetched.id, scratchpad.id);
        assert_eq!(fetched.user_input, "I want to add a new authentication system");
        assert_eq!(fetched.project_id, project.id);
    }

    #[test]
    fn get_scratchpads_for_board() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let project = create_test_project(&db);
        
        db.create_scratchpad(&CreateScratchpad {
            board_id: board.id.clone(),
            project_id: project.id.clone(),
            name: "Plan 1".to_string(),
            user_input: "Input 1".to_string(),
            agent_pref: None,
            model: None,
            settings: serde_json::json!({}),
        }).unwrap();
        
        db.create_scratchpad(&CreateScratchpad {
            board_id: board.id.clone(),
            project_id: project.id.clone(),
            name: "Plan 2".to_string(),
            user_input: "Input 2".to_string(),
            agent_pref: None,
            model: None,
            settings: serde_json::json!({}),
        }).unwrap();
        
        let scratchpads = db.get_scratchpads(&board.id).unwrap();
        assert_eq!(scratchpads.len(), 2);
    }

    #[test]
    fn update_scratchpad() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let project = create_test_project(&db);
        
        let scratchpad = db.create_scratchpad(&CreateScratchpad {
            board_id: board.id.clone(),
            project_id: project.id.clone(),
            name: "Original".to_string(),
            user_input: "Original input".to_string(),
            agent_pref: None,
            model: None,
            settings: serde_json::json!({}),
        }).unwrap();
        
        let updated = db.update_scratchpad(&scratchpad.id, &UpdateScratchpad {
            name: Some("Updated".to_string()),
            user_input: None,
            status: Some(ScratchpadStatus::Exploring),
            agent_pref: Some("cursor".to_string()),
            model: Some("sonnet".to_string()),
            exploration_log: None,
            plan_markdown: None,
            plan_json: None,
            settings: None,
        }).unwrap();
        
        assert_eq!(updated.name, "Updated");
        assert_eq!(updated.user_input, "Original input");
        assert_eq!(updated.status, ScratchpadStatus::Exploring);
        assert_eq!(updated.agent_pref, Some("cursor".to_string()));
        assert_eq!(updated.model, Some("sonnet".to_string()));
    }

    #[test]
    fn append_exploration() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let project = create_test_project(&db);
        
        let scratchpad = db.create_scratchpad(&CreateScratchpad {
            board_id: board.id.clone(),
            project_id: project.id.clone(),
            name: "Plan".to_string(),
            user_input: "Input".to_string(),
            agent_pref: None,
            model: None,
            settings: serde_json::json!({}),
        }).unwrap();
        
        let exploration = Exploration {
            query: "How does auth work?".to_string(),
            response: "Auth uses JWT tokens...".to_string(),
            timestamp: chrono::Utc::now(),
        };
        
        db.append_exploration(&scratchpad.id, &exploration).unwrap();
        
        let fetched = db.get_scratchpad(&scratchpad.id).unwrap();
        assert_eq!(fetched.exploration_log.len(), 1);
        assert_eq!(fetched.exploration_log[0].query, "How does auth work?");
    }

    #[test]
    fn set_scratchpad_status() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let project = create_test_project(&db);
        
        let scratchpad = db.create_scratchpad(&CreateScratchpad {
            board_id: board.id.clone(),
            project_id: project.id.clone(),
            name: "Plan".to_string(),
            user_input: "Input".to_string(),
            agent_pref: None,
            model: None,
            settings: serde_json::json!({}),
        }).unwrap();
        
        assert_eq!(scratchpad.status, ScratchpadStatus::Draft);
        
        db.set_scratchpad_status(&scratchpad.id, ScratchpadStatus::Completed).unwrap();
        
        let fetched = db.get_scratchpad(&scratchpad.id).unwrap();
        assert_eq!(fetched.status, ScratchpadStatus::Completed);
    }

    #[test]
    fn set_scratchpad_plan() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let project = create_test_project(&db);
        
        let scratchpad = db.create_scratchpad(&CreateScratchpad {
            board_id: board.id.clone(),
            project_id: project.id.clone(),
            name: "Plan".to_string(),
            user_input: "Input".to_string(),
            agent_pref: None,
            model: None,
            settings: serde_json::json!({}),
        }).unwrap();
        
        let plan_json = serde_json::json!({
            "overview": "Test plan",
            "epics": []
        });
        
        db.set_scratchpad_plan(&scratchpad.id, "# Test Plan\n\nOverview...", Some(&plan_json)).unwrap();
        
        let fetched = db.get_scratchpad(&scratchpad.id).unwrap();
        assert!(fetched.plan_markdown.is_some());
        assert!(fetched.plan_json.is_some());
        assert_eq!(fetched.plan_json.unwrap()["overview"], "Test plan");
    }

    #[test]
    fn delete_scratchpad() {
        let db = create_test_db();
        let board = db.create_board("Test Board").unwrap();
        let project = create_test_project(&db);
        
        let scratchpad = db.create_scratchpad(&CreateScratchpad {
            board_id: board.id.clone(),
            project_id: project.id.clone(),
            name: "Plan".to_string(),
            user_input: "Input".to_string(),
            agent_pref: None,
            model: None,
            settings: serde_json::json!({}),
        }).unwrap();
        
        db.delete_scratchpad(&scratchpad.id).unwrap();
        
        let result = db.get_scratchpad(&scratchpad.id);
        assert!(matches!(result, Err(DbError::NotFound(_))));
    }

    #[test]
    fn delete_scratchpad_not_found() {
        let db = create_test_db();
        let result = db.delete_scratchpad("nonexistent");
        assert!(matches!(result, Err(DbError::NotFound(_))));
    }

    #[test]
    fn scratchpad_status_roundtrip() {
        for status in [
            ScratchpadStatus::Draft,
            ScratchpadStatus::Exploring,
            ScratchpadStatus::Planning,
            ScratchpadStatus::AwaitingApproval,
            ScratchpadStatus::Approved,
            ScratchpadStatus::Executing,
            ScratchpadStatus::Completed,
            ScratchpadStatus::Failed,
        ] {
            assert_eq!(ScratchpadStatus::parse(status.as_str()), Some(status));
        }
    }
}
