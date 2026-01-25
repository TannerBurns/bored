use crate::db::{Database, DbError, parse_datetime};
use crate::db::models::{
    Project, CreateProject, UpdateProject, AgentPref, ReadinessCheck,
};

impl Database {
    pub fn create_project(&self, input: &CreateProject) -> Result<Project, DbError> {
        let path = std::path::Path::new(&input.path);
        if !path.exists() {
            return Err(DbError::Validation(format!(
                "Path does not exist: {}",
                input.path
            )));
        }
        if !path.is_dir() {
            return Err(DbError::Validation(format!(
                "Path is not a directory: {}",
                input.path
            )));
        }

        let canonical_path = path
            .canonicalize()
            .map_err(|e| DbError::Validation(format!("Invalid path: {}", e)))?
            .to_string_lossy()
            .to_string();

        self.with_conn(|conn| {
            let project_id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now();

            conn.execute(
                r#"INSERT INTO projects 
                   (id, name, path, preferred_agent, requires_git, created_at, updated_at)
                   VALUES (?, ?, ?, ?, ?, ?, ?)"#,
                rusqlite::params![
                    project_id,
                    input.name,
                    canonical_path,
                    input.preferred_agent.as_ref().map(|p| p.as_str()),
                    input.requires_git as i32,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )?;

            Ok(Project {
                id: project_id,
                name: input.name.clone(),
                path: canonical_path,
                cursor_hooks_installed: false,
                claude_hooks_installed: false,
                preferred_agent: input.preferred_agent.clone(),
                allow_shell_commands: true,
                allow_file_writes: true,
                blocked_patterns: vec![],
                settings: serde_json::json!({}),
                requires_git: input.requires_git,
                created_at: now,
                updated_at: now,
            })
        })
    }

    pub fn get_projects(&self) -> Result<Vec<Project>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT id, name, path, cursor_hooks_installed, claude_hooks_installed,
                          preferred_agent, allow_shell_commands, allow_file_writes,
                          blocked_patterns_json, settings_json, created_at, updated_at,
                          requires_git
                   FROM projects ORDER BY name"#,
            )?;

            let projects = stmt
                .query_map([], |row| {
                    let blocked_json: String = row.get(8)?;
                    let settings_json: String = row.get(9)?;
                    let pref_str: Option<String> = row.get(5)?;

                    Ok(Project {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        path: row.get(2)?,
                        cursor_hooks_installed: row.get::<_, i32>(3)? != 0,
                        claude_hooks_installed: row.get::<_, i32>(4)? != 0,
                        preferred_agent: pref_str.and_then(|s| AgentPref::parse(&s)),
                        allow_shell_commands: row.get::<_, i32>(6)? != 0,
                        allow_file_writes: row.get::<_, i32>(7)? != 0,
                        blocked_patterns: serde_json::from_str(&blocked_json).unwrap_or_default(),
                        settings: serde_json::from_str(&settings_json).unwrap_or(serde_json::json!({})),
                        requires_git: row.get::<_, i32>(12).unwrap_or(1) != 0,
                        created_at: parse_datetime(row.get(10)?),
                        updated_at: parse_datetime(row.get(11)?),
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;

            Ok(projects)
        })
    }

    pub fn get_project(&self, project_id: &str) -> Result<Option<Project>, DbError> {
        self.get_projects().map(|projects| {
            projects.into_iter().find(|p| p.id == project_id)
        })
    }

    pub fn get_project_by_path(&self, path: &str) -> Result<Option<Project>, DbError> {
        let canonical = std::path::Path::new(path)
            .canonicalize()
            .ok()
            .map(|p| p.to_string_lossy().to_string());

        self.get_projects().map(|projects| {
            projects.into_iter().find(|p| {
                Some(&p.path) == canonical.as_ref()
            })
        })
    }

    pub fn update_project(&self, project_id: &str, input: &UpdateProject) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();

            if let Some(ref name) = input.name {
                conn.execute(
                    "UPDATE projects SET name = ?, updated_at = ? WHERE id = ?",
                    rusqlite::params![name, now, project_id],
                )?;
            }

            if let Some(ref pref) = input.preferred_agent {
                conn.execute(
                    "UPDATE projects SET preferred_agent = ?, updated_at = ? WHERE id = ?",
                    rusqlite::params![pref.as_str(), now, project_id],
                )?;
            }

            if let Some(allow) = input.allow_shell_commands {
                conn.execute(
                    "UPDATE projects SET allow_shell_commands = ?, updated_at = ? WHERE id = ?",
                    rusqlite::params![allow as i32, now, project_id],
                )?;
            }

            if let Some(allow) = input.allow_file_writes {
                conn.execute(
                    "UPDATE projects SET allow_file_writes = ?, updated_at = ? WHERE id = ?",
                    rusqlite::params![allow as i32, now, project_id],
                )?;
            }

            if let Some(ref patterns) = input.blocked_patterns {
                let json = serde_json::to_string(patterns).unwrap_or_else(|_| "[]".to_string());
                conn.execute(
                    "UPDATE projects SET blocked_patterns_json = ?, updated_at = ? WHERE id = ?",
                    rusqlite::params![json, now, project_id],
                )?;
            }

            if let Some(requires_git) = input.requires_git {
                conn.execute(
                    "UPDATE projects SET requires_git = ?, updated_at = ? WHERE id = ?",
                    rusqlite::params![requires_git as i32, now, project_id],
                )?;
            }

            Ok(())
        })
    }

    pub fn update_project_hooks(
        &self,
        project_id: &str,
        cursor_installed: Option<bool>,
        claude_installed: Option<bool>,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();

            if let Some(installed) = cursor_installed {
                conn.execute(
                    "UPDATE projects SET cursor_hooks_installed = ?, updated_at = ? WHERE id = ?",
                    rusqlite::params![installed as i32, now, project_id],
                )?;
            }

            if let Some(installed) = claude_installed {
                conn.execute(
                    "UPDATE projects SET claude_hooks_installed = ?, updated_at = ? WHERE id = ?",
                    rusqlite::params![installed as i32, now, project_id],
                )?;
            }

            Ok(())
        })
    }

    pub fn delete_project(&self, project_id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let board_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM boards WHERE default_project_id = ?",
                [project_id],
                |row| row.get(0),
            )?;

            if board_count > 0 {
                return Err(DbError::Validation(format!(
                    "Cannot delete project: {} board(s) use it as default",
                    board_count
                )));
            }

            conn.execute("DELETE FROM projects WHERE id = ?", [project_id])?;
            Ok(())
        })
    }

    pub fn set_board_project(
        &self,
        board_id: &str,
        project_id: Option<&str>,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "UPDATE boards SET default_project_id = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![project_id, now, board_id],
            )?;
            Ok(())
        })
    }

    pub fn can_move_to_ready(&self, ticket_id: &str) -> Result<ReadinessCheck, DbError> {
        self.with_conn(|conn| {
            let result: Result<(Option<String>, String), _> = conn.query_row(
                "SELECT project_id, board_id FROM tickets WHERE id = ?",
                [ticket_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            );

            let (ticket_project_id, board_id) = result
                .map_err(|_| DbError::NotFound(format!("Ticket {} not found", ticket_id)))?;

            let board_project_id: Option<String> = conn.query_row(
                "SELECT default_project_id FROM boards WHERE id = ?",
                [&board_id],
                |row| row.get(0),
            ).ok().flatten();

            let effective_project_id = ticket_project_id.or(board_project_id);

            match effective_project_id {
                Some(pid) => {
                    let path: Option<String> = conn.query_row(
                        "SELECT path FROM projects WHERE id = ?",
                        [&pid],
                        |row| row.get(0),
                    ).ok();

                    if let Some(p) = path {
                        if std::path::Path::new(&p).exists() {
                            Ok(ReadinessCheck::Ready { project_id: pid })
                        } else {
                            Ok(ReadinessCheck::ProjectPathMissing { path: p })
                        }
                    } else {
                        Ok(ReadinessCheck::ProjectNotFound(None))
                    }
                }
                None => Ok(ReadinessCheck::NoProject(None)),
            }
        })
    }

    pub fn resolve_project_for_ticket(&self, ticket_id: &str) -> Result<Option<Project>, DbError> {
        match self.can_move_to_ready(ticket_id)? {
            ReadinessCheck::Ready { project_id } => self.get_project(&project_id),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::db::models::{CreateTicket, Priority};

    fn create_test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn temp_dir_path() -> String {
        std::env::temp_dir().to_string_lossy().to_string()
    }

    #[test]
    fn create_project_validates_path_exists() {
        let db = create_test_db();
        
        let result = db.create_project(&CreateProject {
            name: "Bad".to_string(),
            path: "/nonexistent/path/12345".to_string(),
            preferred_agent: None,
            requires_git: true,
        });
        
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("does not exist"));
    }

    #[test]
    fn create_project_validates_path_is_directory() {
        let db = create_test_db();
        
        let file_path = std::env::current_exe().unwrap();
        let result = db.create_project(&CreateProject {
            name: "Bad".to_string(),
            path: file_path.to_string_lossy().to_string(),
            preferred_agent: None,
            requires_git: true,
        });
        
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("not a directory"));
    }

    #[test]
    fn get_project_by_path() {
        let db = create_test_db();
        let temp = temp_dir_path();
        
        let project = db.create_project(&CreateProject {
            name: "Test".to_string(),
            path: temp.clone(),
            preferred_agent: None,
            requires_git: true,
        }).unwrap();
        
        let found = db.get_project_by_path(&temp).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, project.id);
        
        let not_found = db.get_project_by_path("/some/other/path").unwrap();
        assert!(not_found.is_none());
    }

    #[test]
    fn update_project_hooks() {
        let db = create_test_db();
        
        let project = db.create_project(&CreateProject {
            name: "Test".to_string(),
            path: temp_dir_path(),
            preferred_agent: None,
            requires_git: true,
        }).unwrap();
        
        assert!(!project.cursor_hooks_installed);
        assert!(!project.claude_hooks_installed);
        
        db.update_project_hooks(&project.id, Some(true), None).unwrap();
        let updated = db.get_project(&project.id).unwrap().unwrap();
        assert!(updated.cursor_hooks_installed);
        assert!(!updated.claude_hooks_installed);
        
        db.update_project_hooks(&project.id, None, Some(true)).unwrap();
        let updated = db.get_project(&project.id).unwrap().unwrap();
        assert!(updated.cursor_hooks_installed);
        assert!(updated.claude_hooks_installed);
    }

    #[test]
    fn delete_project_fails_if_board_uses_it() {
        let db = create_test_db();
        
        let project = db.create_project(&CreateProject {
            name: "Test".to_string(),
            path: temp_dir_path(),
            preferred_agent: None,
            requires_git: true,
        }).unwrap();
        
        let board = db.create_board("Board").unwrap();
        db.set_board_project(&board.id, Some(&project.id)).unwrap();
        
        let result = db.delete_project(&project.id);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("board"));
    }

    #[test]
    fn set_board_project() {
        let db = create_test_db();
        
        let project = db.create_project(&CreateProject {
            name: "Test".to_string(),
            path: temp_dir_path(),
            preferred_agent: None,
            requires_git: true,
        }).unwrap();
        
        let board = db.create_board("Board").unwrap();
        assert!(board.default_project_id.is_none());
        
        db.set_board_project(&board.id, Some(&project.id)).unwrap();
        
        let updated = db.get_board(&board.id).unwrap().unwrap();
        assert_eq!(updated.default_project_id, Some(project.id.clone()));
        
        db.set_board_project(&board.id, None).unwrap();
        let cleared = db.get_board(&board.id).unwrap().unwrap();
        assert!(cleared.default_project_id.is_none());
    }

    #[test]
    fn update_project_blocked_patterns() {
        let db = create_test_db();
        
        let project = db.create_project(&CreateProject {
            name: "Test".to_string(),
            path: temp_dir_path(),
            preferred_agent: None,
            requires_git: true,
        }).unwrap();
        
        assert!(project.blocked_patterns.is_empty());
        
        db.update_project(&project.id, &UpdateProject {
            name: None,
            preferred_agent: None,
            allow_shell_commands: None,
            allow_file_writes: None,
            blocked_patterns: Some(vec!["*.log".to_string(), "node_modules".to_string()]),
            requires_git: None,
        }).unwrap();
        
        let updated = db.get_project(&project.id).unwrap().unwrap();
        assert_eq!(updated.blocked_patterns, vec!["*.log", "node_modules"]);
    }

    #[test]
    fn can_move_to_ready_with_ticket_project() {
        let db = create_test_db();
        
        let project = db.create_project(&CreateProject {
            name: "Proj".to_string(),
            path: temp_dir_path(),
            preferred_agent: None,
            requires_git: true,
        }).unwrap();
        
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Ticket".to_string(),
            description_md: "".to_string(),
            priority: Priority::Low,
            labels: vec![],
            project_id: Some(project.id.clone()),
            agent_pref: None,
        }).unwrap();
        
        let check = db.can_move_to_ready(&ticket.id).unwrap();
        match check {
            ReadinessCheck::Ready { project_id } => assert_eq!(project_id, project.id),
            other => panic!("Expected Ready, got {:?}", other),
        }
    }

    #[test]
    fn can_move_to_ready_uses_board_default() {
        let db = create_test_db();
        
        let project = db.create_project(&CreateProject {
            name: "Proj".to_string(),
            path: temp_dir_path(),
            preferred_agent: None,
            requires_git: true,
        }).unwrap();
        
        let board = db.create_board("Board").unwrap();
        db.set_board_project(&board.id, Some(&project.id)).unwrap();
        
        let columns = db.get_columns(&board.id).unwrap();
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Ticket".to_string(),
            description_md: "".to_string(),
            priority: Priority::Low,
            labels: vec![],
            project_id: None,
            agent_pref: None,
        }).unwrap();
        
        let check = db.can_move_to_ready(&ticket.id).unwrap();
        match check {
            ReadinessCheck::Ready { project_id } => assert_eq!(project_id, project.id),
            other => panic!("Expected Ready, got {:?}", other),
        }
    }

    #[test]
    fn can_move_to_ready_returns_no_project() {
        let db = create_test_db();
        
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Ticket".to_string(),
            description_md: "".to_string(),
            priority: Priority::Low,
            labels: vec![],
            project_id: None,
            agent_pref: None,
        }).unwrap();
        
        let check = db.can_move_to_ready(&ticket.id).unwrap();
        assert!(matches!(check, ReadinessCheck::NoProject(_)));
    }
    
    #[test]
    fn create_project_with_requires_git_false() {
        let db = create_test_db();
        
        let project = db.create_project(&CreateProject {
            name: "No Git Project".to_string(),
            path: temp_dir_path(),
            preferred_agent: None,
            requires_git: false,
        }).unwrap();
        
        assert!(!project.requires_git);
        
        // Verify it persists
        let fetched = db.get_project(&project.id).unwrap().unwrap();
        assert!(!fetched.requires_git);
    }
    
    #[test]
    fn update_project_requires_git() {
        let db = create_test_db();
        
        let project = db.create_project(&CreateProject {
            name: "Test".to_string(),
            path: temp_dir_path(),
            preferred_agent: None,
            requires_git: true,
        }).unwrap();
        
        assert!(project.requires_git);
        
        // Update to not require git
        db.update_project(&project.id, &UpdateProject {
            name: None,
            preferred_agent: None,
            allow_shell_commands: None,
            allow_file_writes: None,
            blocked_patterns: None,
            requires_git: Some(false),
        }).unwrap();
        
        let updated = db.get_project(&project.id).unwrap().unwrap();
        assert!(!updated.requires_git);
    }
}
