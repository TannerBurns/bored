use crate::db::models::{Project, Workspace};
use crate::db::{parse_datetime, Database, DbError};

impl Database {
    pub fn create_workspace(&self, name: &str) -> Result<Workspace, DbError> {
        self.with_conn(|conn| {
            let id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now();

            conn.execute(
                "INSERT INTO workspaces (id, name, created_at, updated_at) VALUES (?, ?, ?, ?)",
                rusqlite::params![id, name, now.to_rfc3339(), now.to_rfc3339()],
            )?;

            Ok(Workspace {
                id,
                name: name.to_string(),
                project_ids: vec![],
                created_at: now,
                updated_at: now,
            })
        })
    }

    pub fn get_workspace(&self, workspace_id: &str) -> Result<Option<Workspace>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, created_at, updated_at FROM workspaces WHERE id = ?",
            )?;

            let workspace = stmt
                .query_row([workspace_id], |row| {
                    Ok(Workspace {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        project_ids: vec![],
                        created_at: parse_datetime(row.get(2)?),
                        updated_at: parse_datetime(row.get(3)?),
                    })
                })
                .optional()?;

            if let Some(mut ws) = workspace {
                let mut pstmt = conn.prepare(
                    "SELECT project_id FROM workspace_projects WHERE workspace_id = ? ORDER BY position",
                )?;
                ws.project_ids = pstmt
                    .query_map([workspace_id], |row| row.get(0))?
                    .collect::<Result<Vec<String>, _>>()?;
                Ok(Some(ws))
            } else {
                Ok(None)
            }
        })
    }

    pub fn get_workspaces(&self) -> Result<Vec<Workspace>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, created_at, updated_at FROM workspaces ORDER BY created_at DESC",
            )?;

            let workspaces: Vec<Workspace> = stmt
                .query_map([], |row| {
                    Ok(Workspace {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        project_ids: vec![],
                        created_at: parse_datetime(row.get(2)?),
                        updated_at: parse_datetime(row.get(3)?),
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;

            let mut pstmt = conn.prepare(
                "SELECT workspace_id, project_id FROM workspace_projects ORDER BY position",
            )?;
            let pairs: Vec<(String, String)> = pstmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<Result<Vec<_>, _>>()?;

            let mut result = workspaces;
            for ws in &mut result {
                ws.project_ids = pairs
                    .iter()
                    .filter(|(wid, _)| wid == &ws.id)
                    .map(|(_, pid)| pid.clone())
                    .collect();
            }

            Ok(result)
        })
    }

    pub fn update_workspace(&self, workspace_id: &str, name: &str) -> Result<Workspace, DbError> {
        self.with_conn(|conn| {
            let now = chrono::Utc::now();
            let affected = conn.execute(
                "UPDATE workspaces SET name = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![name, now.to_rfc3339(), workspace_id],
            )?;

            if affected == 0 {
                return Err(DbError::NotFound(format!("Workspace {}", workspace_id)));
            }

            Ok(())
        })?;

        self.get_workspace(workspace_id)?
            .ok_or_else(|| DbError::NotFound(format!("Workspace {}", workspace_id)))
    }

    pub fn delete_workspace(&self, workspace_id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let affected =
                conn.execute("DELETE FROM workspaces WHERE id = ?", [workspace_id])?;
            if affected == 0 {
                return Err(DbError::NotFound(format!("Workspace {}", workspace_id)));
            }
            Ok(())
        })
    }

    pub fn add_project_to_workspace(
        &self,
        workspace_id: &str,
        project_id: &str,
        position: i32,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO workspace_projects (workspace_id, project_id, position) VALUES (?, ?, ?)",
                rusqlite::params![workspace_id, project_id, position],
            )?;
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "UPDATE workspaces SET updated_at = ? WHERE id = ?",
                rusqlite::params![now, workspace_id],
            )?;
            Ok(())
        })
    }

    pub fn remove_project_from_workspace(
        &self,
        workspace_id: &str,
        project_id: &str,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            conn.execute(
                "DELETE FROM workspace_projects WHERE workspace_id = ? AND project_id = ?",
                rusqlite::params![workspace_id, project_id],
            )?;
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "UPDATE workspaces SET updated_at = ? WHERE id = ?",
                rusqlite::params![now, workspace_id],
            )?;
            Ok(())
        })
    }

    pub fn get_workspace_projects(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<Project>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT p.id, p.name, p.path,
                          p.allow_shell_commands, p.allow_file_writes,
                          p.blocked_patterns_json, p.settings_json, p.created_at, p.updated_at,
                          p.requires_git
                   FROM projects p
                   JOIN workspace_projects wp ON p.id = wp.project_id
                   WHERE wp.workspace_id = ?
                   ORDER BY wp.position"#,
            )?;

            let projects = stmt
                .query_map([workspace_id], |row| {
                    let blocked_json: String = row.get(5)?;
                    let settings_json: String = row.get(6)?;

                    Ok(Project {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        path: row.get(2)?,
                        allow_shell_commands: row.get::<_, i32>(3)? != 0,
                        allow_file_writes: row.get::<_, i32>(4)? != 0,
                        blocked_patterns: serde_json::from_str(&blocked_json).unwrap_or_default(),
                        settings: serde_json::from_str(&settings_json)
                            .unwrap_or(serde_json::json!({})),
                        requires_git: row.get::<_, i32>(9).unwrap_or(1) != 0,
                        created_at: parse_datetime(row.get(7)?),
                        updated_at: parse_datetime(row.get(8)?),
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;

            Ok(projects)
        })
    }
}

use rusqlite::OptionalExtension;
