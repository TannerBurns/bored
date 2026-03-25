//! Database operations for specs (planning agent)

use crate::db::models::{CreateSpec, CreateSpecVersion, Spec, SpecWithVersion, UpdateSpec};
use crate::db::{parse_datetime, Database, DbError};

impl Database {
    /// Create a new spec (also creates the first version)
    pub fn create_spec(&self, input: &CreateSpec) -> Result<Spec, DbError> {
        self.with_conn(|conn| {
            let id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now();
            let settings_json =
                serde_json::to_string(&input.settings).unwrap_or_else(|_| "{}".to_string());

            conn.execute(
                r#"INSERT INTO specs 
                   (id, board_id, target_board_id, project_id, name, user_input, model, settings_json, created_at, updated_at)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
                rusqlite::params![
                    id,
                    input.board_id,
                    input.target_board_id,
                    input.project_id,
                    input.name,
                    input.user_input,
                    input.model,
                    settings_json,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )?;

            let spec = Spec {
                id: id.clone(),
                board_id: input.board_id.clone(),
                target_board_id: input.target_board_id.clone(),
                project_id: input.project_id.clone(),
                name: input.name.clone(),
                user_input: input.user_input.clone(),
                model: input.model.clone(),
                settings: input.settings.clone(),
                created_at: now,
                updated_at: now,
            };

            // Also create the first version
            let version_id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                r#"INSERT INTO spec_versions 
                   (id, spec_id, version_number, status, exploration_log, created_at, updated_at)
                   VALUES (?, ?, ?, ?, ?, ?, ?)"#,
                rusqlite::params![
                    version_id,
                    id,
                    1,
                    "conversing",
                    "[]",
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )?;

            Ok(spec)
        })
    }

    /// Get a spec by ID
    pub fn get_spec(&self, id: &str) -> Result<Spec, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, target_board_id, project_id, name, user_input, 
                          model, settings_json, created_at, updated_at
                   FROM specs WHERE id = ?"#,
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

    /// Get a spec with its latest version
    pub fn get_spec_with_version(&self, id: &str) -> Result<SpecWithVersion, DbError> {
        let spec = self.get_spec(id)?;
        let latest_version = self.get_latest_spec_version(id)?;
        let version_count = self.get_spec_version_count(id)?;

        Ok(SpecWithVersion {
            spec,
            latest_version,
            version_count,
        })
    }

    /// Get specs for a board
    pub fn get_specs(&self, board_id: &str) -> Result<Vec<Spec>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, target_board_id, project_id, name, user_input, 
                          model, settings_json, created_at, updated_at
                   FROM specs WHERE board_id = ?
                   ORDER BY created_at DESC"#,
            )?;

            let rows = stmt.query_map([board_id], Self::map_spec_row)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    /// Get specs for a board with their latest versions
    pub fn get_specs_with_versions(&self, board_id: &str) -> Result<Vec<SpecWithVersion>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT s.id, s.board_id, s.target_board_id, s.project_id, s.name, s.user_input,
                          s.model, s.settings_json, s.created_at, s.updated_at,
                          sv.id, sv.spec_id, sv.version_number, sv.status, sv.exploration_log,
                          sv.plan_markdown, sv.plan_json, sv.work_started_at, sv.created_at, sv.updated_at,
                          (SELECT COUNT(*) FROM spec_versions WHERE spec_id = s.id) as version_count
                   FROM specs s
                   LEFT JOIN spec_versions sv ON sv.spec_id = s.id
                     AND sv.version_number = (SELECT MAX(version_number) FROM spec_versions WHERE spec_id = s.id)
                   WHERE s.board_id = ?
                   ORDER BY s.created_at DESC"#,
            )?;

            let rows = stmt.query_map([board_id], |row| {
                let spec = Self::map_spec_row(row)?;
                let latest_version = if row.get::<_, Option<String>>(10)?.is_some() {
                    Some(Self::map_spec_version_row_offset(row, 10)?)
                } else {
                    None
                };
                let version_count: i32 = row.get(20)?;
                Ok(SpecWithVersion { spec, latest_version, version_count })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    /// Get all specs across all boards
    pub fn get_all_specs(&self) -> Result<Vec<Spec>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, target_board_id, project_id, name, user_input, 
                          model, settings_json, created_at, updated_at
                   FROM specs
                   ORDER BY created_at DESC"#,
            )?;

            let rows = stmt.query_map([], Self::map_spec_row)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    /// Get all specs with their latest versions
    pub fn get_all_specs_with_versions(&self) -> Result<Vec<SpecWithVersion>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT s.id, s.board_id, s.target_board_id, s.project_id, s.name, s.user_input,
                          s.model, s.settings_json, s.created_at, s.updated_at,
                          sv.id, sv.spec_id, sv.version_number, sv.status, sv.exploration_log,
                          sv.plan_markdown, sv.plan_json, sv.work_started_at, sv.created_at, sv.updated_at,
                          (SELECT COUNT(*) FROM spec_versions WHERE spec_id = s.id) as version_count
                   FROM specs s
                   LEFT JOIN spec_versions sv ON sv.spec_id = s.id
                     AND sv.version_number = (SELECT MAX(version_number) FROM spec_versions WHERE spec_id = s.id)
                   ORDER BY s.created_at DESC"#,
            )?;

            let rows = stmt.query_map([], |row| {
                let spec = Self::map_spec_row(row)?;
                let latest_version = if row.get::<_, Option<String>>(10)?.is_some() {
                    Some(Self::map_spec_version_row_offset(row, 10)?)
                } else {
                    None
                };
                let version_count: i32 = row.get(20)?;
                Ok(SpecWithVersion { spec, latest_version, version_count })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    /// Update a spec (non-versioned fields only)
    pub fn update_spec(&self, id: &str, updates: &UpdateSpec) -> Result<Spec, DbError> {
        self.with_conn(|conn| {
            // First get existing
            let existing = {
                let mut stmt = conn.prepare(
                    r#"SELECT id, board_id, target_board_id, project_id, name, user_input, 
                              model, settings_json, created_at, updated_at
                       FROM specs WHERE id = ?"#,
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
            let model = updates.model.as_ref().or(existing.model.as_ref());
            let settings = updates.settings.as_ref().unwrap_or(&existing.settings);

            let settings_json =
                serde_json::to_string(settings).unwrap_or_else(|_| "{}".to_string());

            conn.execute(
                r#"UPDATE specs 
                   SET name = ?, user_input = ?, model = ?, settings_json = ?, updated_at = ?
                   WHERE id = ?"#,
                rusqlite::params![name, user_input, model, settings_json, now.to_rfc3339(), id],
            )?;

            // Re-query to return updated
            let mut stmt = conn.prepare(
                r#"SELECT id, board_id, target_board_id, project_id, name, user_input, 
                          model, settings_json, created_at, updated_at
                   FROM specs WHERE id = ?"#,
            )?;
            stmt.query_row([id], Self::map_spec_row)
                .map_err(DbError::Sqlite)
        })
    }

    /// Delete a spec (cascades to versions and conversation messages)
    pub fn delete_spec(&self, id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let affected = conn.execute("DELETE FROM specs WHERE id = ?", [id])?;

            if affected == 0 {
                return Err(DbError::NotFound(format!("Spec {}", id)));
            }
            Ok(())
        })
    }

    /// Delete a spec and all tickets created from any of its versions (cascade delete)
    /// Returns the number of tickets deleted
    pub fn delete_spec_with_tickets(&self, id: &str) -> Result<usize, DbError> {
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;

            let mut version_stmt = tx.prepare("SELECT id FROM spec_versions WHERE spec_id = ?")?;
            let version_ids: Vec<String> = version_stmt
                .query_map([id], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?;
            drop(version_stmt);

            let mut total_ticket_count = 0;

            for version_id in &version_ids {
                let mut stmt =
                    tx.prepare("SELECT id FROM tickets WHERE spec_version_id = ?")?;
                let ticket_ids: Vec<String> = stmt
                    .query_map([version_id], |row| row.get(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                drop(stmt);

                total_ticket_count += ticket_ids.len();

                for ticket_id in &ticket_ids {
                    tx.execute("DELETE FROM comments WHERE ticket_id = ?", [ticket_id])?;
                    tx.execute("DELETE FROM tasks WHERE ticket_id = ?", [ticket_id])?;
                    tx.execute("DELETE FROM agent_events WHERE ticket_id = ?", [ticket_id])?;
                    tx.execute("DELETE FROM agent_runs WHERE ticket_id = ?", [ticket_id])?;
                }

                tx.execute("DELETE FROM tickets WHERE spec_version_id = ?", [version_id])?;
            }

            let affected = tx.execute("DELETE FROM specs WHERE id = ?", [id])?;

            if affected == 0 {
                return Err(DbError::NotFound(format!("Spec {}", id)));
            }

            tx.commit()?;
            Ok(total_ticket_count)
        })
    }

    /// Create a new version for a spec (used when user returns to add more context)
    pub fn create_new_spec_version(&self, spec_id: &str) -> Result<crate::db::models::SpecVersion, DbError> {
        // Verify spec exists
        let _ = self.get_spec(spec_id)?;

        self.create_spec_version(&CreateSpecVersion {
            spec_id: spec_id.to_string(),
        })
    }

    /// Delete all tickets associated with a spec version
    /// Returns the number of tickets deleted
    pub fn delete_spec_version_tickets(&self, version_id: &str) -> Result<usize, DbError> {
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;

            let mut stmt = tx.prepare("SELECT id FROM tickets WHERE spec_version_id = ?")?;
            let ticket_ids: Vec<String> = stmt
                .query_map([version_id], |row| row.get(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            drop(stmt);

            let count = ticket_ids.len();

            for ticket_id in &ticket_ids {
                tx.execute("DELETE FROM comments WHERE ticket_id = ?", [ticket_id])?;
                tx.execute("DELETE FROM tasks WHERE ticket_id = ?", [ticket_id])?;
                tx.execute("DELETE FROM agent_events WHERE ticket_id = ?", [ticket_id])?;
                tx.execute("DELETE FROM agent_runs WHERE ticket_id = ?", [ticket_id])?;
            }

            tx.execute("DELETE FROM tickets WHERE spec_version_id = ?", [version_id])?;

            tx.commit()?;
            Ok(count)
        })
    }

    fn map_spec_row(row: &rusqlite::Row) -> rusqlite::Result<Spec> {
        // Column order: 0-id, 1-board_id, 2-target_board_id, 3-project_id, 4-name, 5-user_input,
        //               6-model, 7-settings_json, 8-created_at, 9-updated_at
        let settings_str: String = row
            .get::<_, Option<String>>(7)?
            .unwrap_or_else(|| "{}".to_string());
        let settings =
            serde_json::from_str(&settings_str).unwrap_or_else(|_| serde_json::json!({}));

        Ok(Spec {
            id: row.get(0)?,
            board_id: row.get(1)?,
            target_board_id: row.get(2)?,
            project_id: row.get(3)?,
            name: row.get(4)?,
            user_input: row.get(5)?,
            model: row.get(6)?,
            settings,
            created_at: parse_datetime(row.get(8)?),
            updated_at: parse_datetime(row.get(9)?),
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
                model: Some("opus".to_string()),
                settings: serde_json::json!({}),
            })
            .unwrap();

        assert_eq!(spec.name, "Feature Plan");
        assert_eq!(spec.project_id, project.id);
        assert_eq!(spec.model, Some("opus".to_string()));

        let fetched = db.get_spec(&spec.id).unwrap();
        assert_eq!(fetched.id, spec.id);
        assert_eq!(
            fetched.user_input,
            "I want to add a new authentication system"
        );
        assert_eq!(fetched.project_id, project.id);

        // Verify version was created
        let version = db.get_latest_spec_version(&spec.id).unwrap().unwrap();
        assert_eq!(version.version_number, 1);
        assert_eq!(
            version.status,
            crate::db::models::SpecVersionStatus::Conversing
        );
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
                    model: Some("sonnet".to_string()),
                    settings: None,
                },
            )
            .unwrap();

        assert_eq!(updated.name, "Updated");
        assert_eq!(updated.user_input, "Original input");
        assert_eq!(updated.model, Some("sonnet".to_string()));
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
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();

        db.delete_spec(&spec.id).unwrap();

        let result = db.get_spec(&spec.id);
        assert!(matches!(result, Err(DbError::NotFound(_))));

        // Version should also be deleted
        let version = db.get_latest_spec_version(&spec.id).unwrap();
        assert!(version.is_none());
    }

    #[test]
    fn delete_spec_not_found() {
        let db = create_test_db();
        let result = db.delete_spec("nonexistent");
        assert!(matches!(result, Err(DbError::NotFound(_))));
    }

    #[test]
    fn get_spec_with_version() {
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
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();

        let with_version = db.get_spec_with_version(&spec.id).unwrap();
        assert_eq!(with_version.spec.id, spec.id);
        assert!(with_version.latest_version.is_some());
        assert_eq!(with_version.version_count, 1);
    }

    #[test]
    fn create_new_spec_version() {
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
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();

        // Create a second version
        let v2 = db.create_new_spec_version(&spec.id).unwrap();
        assert_eq!(v2.version_number, 2);

        let with_version = db.get_spec_with_version(&spec.id).unwrap();
        assert_eq!(with_version.version_count, 2);
        assert_eq!(
            with_version.latest_version.unwrap().version_number,
            2
        );
    }

    #[test]
    fn get_specs_with_versions_join_query() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let project = create_test_project(&db);

        let spec1 = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: Some(board.id.clone()),
                project_id: project.id.clone(),
                name: "Spec 1".to_string(),
                user_input: "Input 1".to_string(),
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();

        let spec2 = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: Some(board.id.clone()),
                project_id: project.id.clone(),
                name: "Spec 2".to_string(),
                user_input: "Input 2".to_string(),
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();

        db.create_new_spec_version(&spec1.id).unwrap();

        let results = db.get_specs_with_versions(&board.id).unwrap();
        assert_eq!(results.len(), 2);

        let s1 = results.iter().find(|r| r.spec.id == spec1.id).unwrap();
        assert_eq!(s1.version_count, 2);
        assert_eq!(s1.latest_version.as_ref().unwrap().version_number, 2);

        let s2 = results.iter().find(|r| r.spec.id == spec2.id).unwrap();
        assert_eq!(s2.version_count, 1);
        assert_eq!(s2.latest_version.as_ref().unwrap().version_number, 1);
    }

    #[test]
    fn get_all_specs_with_versions_join_query() {
        let db = create_test_db();
        let board1 = db.create_board("Board 1").unwrap();
        let board2 = db.create_board("Board 2").unwrap();
        let project = create_test_project(&db);

        db.create_spec(&CreateSpec {
            board_id: board1.id.clone(),
            target_board_id: Some(board1.id.clone()),
            project_id: project.id.clone(),
            name: "Spec A".to_string(),
            user_input: "Input A".to_string(),
            model: None,
            settings: serde_json::json!({}),
        })
        .unwrap();

        db.create_spec(&CreateSpec {
            board_id: board2.id.clone(),
            target_board_id: Some(board2.id.clone()),
            project_id: project.id.clone(),
            name: "Spec B".to_string(),
            user_input: "Input B".to_string(),
            model: None,
            settings: serde_json::json!({}),
        })
        .unwrap();

        let results = db.get_all_specs_with_versions().unwrap();
        assert_eq!(results.len(), 2);
        for r in &results {
            assert!(r.latest_version.is_some());
            assert_eq!(r.version_count, 1);
        }
    }

    #[test]
    fn get_specs_with_versions_empty_board() {
        let db = create_test_db();
        let board = db.create_board("Empty Board").unwrap();

        let results = db.get_specs_with_versions(&board.id).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn delete_spec_with_tickets_uses_transaction() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let project = create_test_project(&db);

        let spec = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: Some(board.id.clone()),
                project_id: project.id.clone(),
                name: "To Delete".to_string(),
                user_input: "Will be deleted".to_string(),
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();

        let version = db.get_latest_spec_version(&spec.id).unwrap().unwrap();

        let columns = db.get_columns(&board.id).unwrap();
        let col_id = columns[0].id.clone();

        use crate::db::{CreateTicket, Priority, WorkflowType};
        use crate::db::models::CreateTask;
        let ticket = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: col_id.clone(),
                title: "Ticket from spec".to_string(),
                description_md: "desc".to_string(),
                priority: Priority::Medium,
                labels: vec![],
                project_id: Some(project.id.clone()),
                workspace_id: None,
                workflow_type: WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: Some(version.id.clone()),
            })
            .unwrap();

        db.create_task(&CreateTask {
            ticket_id: ticket.id.clone(),
            task_type: Default::default(),
            title: Some("Test task".to_string()),
            content: None,
        }).unwrap();

        let deleted_count = db.delete_spec_with_tickets(&spec.id).unwrap();
        assert_eq!(deleted_count, 1);

        assert!(matches!(db.get_spec(&spec.id), Err(DbError::NotFound(_))));
        assert!(db.get_latest_spec_version(&spec.id).unwrap().is_none());

        let tickets = db.get_tickets(&board.id, None).unwrap();
        assert!(tickets.iter().all(|t| t.id != ticket.id));
    }

    #[test]
    fn delete_spec_with_tickets_not_found() {
        let db = create_test_db();
        let result = db.delete_spec_with_tickets("nonexistent");
        assert!(matches!(result, Err(DbError::NotFound(_))));
    }

    #[test]
    fn delete_spec_version_tickets_uses_transaction() {
        let db = create_test_db();
        let board = db.create_board("Board").unwrap();
        let project = create_test_project(&db);

        let spec = db
            .create_spec(&CreateSpec {
                board_id: board.id.clone(),
                target_board_id: Some(board.id.clone()),
                project_id: project.id.clone(),
                name: "Spec".to_string(),
                user_input: "Input".to_string(),
                model: None,
                settings: serde_json::json!({}),
            })
            .unwrap();

        let version = db.get_latest_spec_version(&spec.id).unwrap().unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let col_id = columns[0].id.clone();

        use crate::db::{CreateTicket, Priority, WorkflowType};
        for i in 0..3 {
            db.create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: col_id.clone(),
                title: format!("Ticket {}", i),
                description_md: "desc".to_string(),
                priority: Priority::Medium,
                labels: vec![],
                project_id: Some(project.id.clone()),
                workspace_id: None,
                workflow_type: WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: Some(version.id.clone()),
            })
            .unwrap();
        }

        let deleted = db.delete_spec_version_tickets(&version.id).unwrap();
        assert_eq!(deleted, 3);

        let remaining = db.get_tickets(&board.id, None).unwrap();
        assert!(remaining.is_empty());

        assert!(db.get_spec(&spec.id).is_ok());
    }
}
