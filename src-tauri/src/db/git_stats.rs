use crate::db::{Database, DbError};
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TicketGitStats {
    pub id: String,
    pub ticket_id: String,
    pub commits: i64,
    pub prs_created: i64,
    pub lines_added: i64,
    pub lines_removed: i64,
    pub files_changed: i64,
    pub collected_at: String,
}

/// Collect git stats for a ticket branch by running git commands against the repo.
pub fn collect_git_stats_for_ticket(
    working_dir: &str,
    branch: &str,
    default_branch: &str,
) -> TicketGitStats {
    let commits = count_commits(working_dir, branch, default_branch);
    let (lines_added, lines_removed, files_changed) =
        count_diff_stats(working_dir, branch, default_branch);

    TicketGitStats {
        id: String::new(),
        ticket_id: String::new(),
        commits,
        prs_created: 0,
        lines_added,
        lines_removed,
        files_changed,
        collected_at: chrono::Utc::now().to_rfc3339(),
    }
}

fn count_commits(working_dir: &str, branch: &str, default_branch: &str) -> i64 {
    let output = Command::new("git")
        .args([
            "rev-list",
            "--count",
            &format!("{}..{}", default_branch, branch),
        ])
        .current_dir(working_dir)
        .output();

    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .trim()
            .parse::<i64>()
            .unwrap_or(0),
        _ => 0,
    }
}

fn count_diff_stats(
    working_dir: &str,
    branch: &str,
    default_branch: &str,
) -> (i64, i64, i64) {
    let output = Command::new("git")
        .args([
            "diff",
            "--numstat",
            &format!("{}...{}", default_branch, branch),
        ])
        .current_dir(working_dir)
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            let mut added: i64 = 0;
            let mut removed: i64 = 0;
            let mut files: i64 = 0;

            for line in text.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 2 {
                    // Binary files show "-" for added/removed
                    added += parts[0].parse::<i64>().unwrap_or(0);
                    removed += parts[1].parse::<i64>().unwrap_or(0);
                    files += 1;
                }
            }
            (added, removed, files)
        }
        _ => (0, 0, 0),
    }
}

impl Database {
    /// Upsert git stats for a ticket.
    pub fn upsert_git_stats(
        &self,
        ticket_id: &str,
        stats: &TicketGitStats,
    ) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                r#"INSERT INTO ticket_git_stats (id, ticket_id, commits, prs_created, lines_added, lines_removed, files_changed, collected_at)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                   ON CONFLICT(ticket_id) DO UPDATE SET
                       commits = ?3,
                       prs_created = ?4,
                       lines_added = ?5,
                       lines_removed = ?6,
                       files_changed = ?7,
                       collected_at = ?8"#,
                rusqlite::params![
                    id,
                    ticket_id,
                    stats.commits,
                    stats.prs_created,
                    stats.lines_added,
                    stats.lines_removed,
                    stats.files_changed,
                    stats.collected_at,
                ],
            )?;
            Ok(())
        })
    }

    /// Increment the PR count for a ticket.
    pub fn increment_pr_count(&self, ticket_id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let affected = conn.execute(
                "UPDATE ticket_git_stats SET prs_created = prs_created + 1 WHERE ticket_id = ?",
                [ticket_id],
            )?;
            if affected == 0 {
                let id = uuid::Uuid::new_v4().to_string();
                let now = chrono::Utc::now().to_rfc3339();
                conn.execute(
                    r#"INSERT INTO ticket_git_stats (id, ticket_id, prs_created, collected_at)
                       VALUES (?1, ?2, 1, ?3)"#,
                    rusqlite::params![id, ticket_id, now],
                )?;
            }
            Ok(())
        })
    }

    /// Backfill git stats for all tickets that have a branch_name.
    /// Returns the number of tickets backfilled.
    pub fn backfill_git_stats(&self) -> Result<u32, DbError> {
        let tickets: Vec<(String, String, Option<String>)> = self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT t.id, t.branch_name, p.path
                   FROM tickets t
                   JOIN projects p ON t.project_id = p.id
                   WHERE t.branch_name IS NOT NULL
                   AND NOT EXISTS (
                       SELECT 1 FROM ticket_git_stats g WHERE g.ticket_id = t.id
                   )"#,
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })?;
            Ok(rows.flatten().collect())
        })?;

        let mut count = 0u32;
        for (ticket_id, branch_name, project_path) in tickets {
            let project_path = match project_path {
                Some(p) => p,
                None => continue,
            };

            let working_dir = find_worktree_or_project(&project_path, &branch_name);
            let default_branch = match crate::commands::next_steps::get_default_branch(&working_dir)
            {
                Ok(b) => b,
                Err(_) => continue,
            };

            let stats = collect_git_stats_for_ticket(&working_dir, &branch_name, &default_branch);
            if self.upsert_git_stats(&ticket_id, &stats).is_ok() {
                count += 1;
            }
        }

        Ok(count)
    }
}

/// Find the worktree path for a branch, falling back to the project path.
fn find_worktree_or_project(project_path: &str, branch: &str) -> String {
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(project_path)
        .output();

    if let Ok(o) = output {
        if o.status.success() {
            let text = String::from_utf8_lossy(&o.stdout);
            let mut current_wt = String::new();
            for line in text.lines() {
                if let Some(path) = line.strip_prefix("worktree ") {
                    current_wt = path.to_string();
                }
                if let Some(branch_ref) = line.strip_prefix("branch ") {
                    let wt_branch = branch_ref
                        .strip_prefix("refs/heads/")
                        .unwrap_or(branch_ref);
                    if wt_branch == branch {
                        return current_wt;
                    }
                }
            }
        }
    }

    project_path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::{CreateTicket, Priority, WorkflowType};

    fn create_test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn create_ticket(db: &Database) -> crate::db::models::Ticket {
        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Ticket".to_string(),
            description_md: "".to_string(),
            priority: Priority::Low,
            labels: vec![],
            project_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .unwrap()
    }

    fn make_stats(commits: i64, prs: i64, added: i64, removed: i64, files: i64) -> TicketGitStats {
        TicketGitStats {
            id: String::new(),
            ticket_id: String::new(),
            commits,
            prs_created: prs,
            lines_added: added,
            lines_removed: removed,
            files_changed: files,
            collected_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    #[test]
    fn upsert_git_stats_inserts_new_record() {
        let db = create_test_db();
        let ticket = create_ticket(&db);
        let stats = make_stats(3, 1, 50, 10, 4);

        db.upsert_git_stats(&ticket.id, &stats).unwrap();

        let row: (i64, i64, i64, i64, i64) = db
            .with_conn(|conn| {
                conn.query_row(
                    "SELECT commits, prs_created, lines_added, lines_removed, files_changed FROM ticket_git_stats WHERE ticket_id = ?",
                    [&ticket.id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
                ).map_err(crate::db::DbError::Sqlite)
            })
            .unwrap();

        assert_eq!(row, (3, 1, 50, 10, 4));
    }

    #[test]
    fn upsert_git_stats_updates_existing_record() {
        let db = create_test_db();
        let ticket = create_ticket(&db);

        db.upsert_git_stats(&ticket.id, &make_stats(1, 0, 10, 5, 2))
            .unwrap();
        db.upsert_git_stats(&ticket.id, &make_stats(5, 2, 100, 30, 8))
            .unwrap();

        let commits: i64 = db
            .with_conn(|conn| {
                conn.query_row(
                    "SELECT commits FROM ticket_git_stats WHERE ticket_id = ?",
                    [&ticket.id],
                    |row| row.get(0),
                )
                .map_err(crate::db::DbError::Sqlite)
            })
            .unwrap();

        assert_eq!(commits, 5, "should have the updated value");
    }

    #[test]
    fn increment_pr_count_on_existing_record() {
        let db = create_test_db();
        let ticket = create_ticket(&db);

        db.upsert_git_stats(&ticket.id, &make_stats(1, 0, 10, 5, 2))
            .unwrap();
        db.increment_pr_count(&ticket.id).unwrap();

        let prs: i64 = db
            .with_conn(|conn| {
                conn.query_row(
                    "SELECT prs_created FROM ticket_git_stats WHERE ticket_id = ?",
                    [&ticket.id],
                    |row| row.get(0),
                )
                .map_err(crate::db::DbError::Sqlite)
            })
            .unwrap();

        assert_eq!(prs, 1, "should be incremented from 0 to 1");
    }

    #[test]
    fn increment_pr_count_creates_new_record_when_none_exists() {
        let db = create_test_db();
        let ticket = create_ticket(&db);

        db.increment_pr_count(&ticket.id).unwrap();

        let prs: i64 = db
            .with_conn(|conn| {
                conn.query_row(
                    "SELECT prs_created FROM ticket_git_stats WHERE ticket_id = ?",
                    [&ticket.id],
                    |row| row.get(0),
                )
                .map_err(crate::db::DbError::Sqlite)
            })
            .unwrap();

        assert_eq!(prs, 1);
    }

    #[test]
    fn increment_pr_count_increments_multiple_times() {
        let db = create_test_db();
        let ticket = create_ticket(&db);

        db.increment_pr_count(&ticket.id).unwrap();
        db.increment_pr_count(&ticket.id).unwrap();
        db.increment_pr_count(&ticket.id).unwrap();

        let prs: i64 = db
            .with_conn(|conn| {
                conn.query_row(
                    "SELECT prs_created FROM ticket_git_stats WHERE ticket_id = ?",
                    [&ticket.id],
                    |row| row.get(0),
                )
                .map_err(crate::db::DbError::Sqlite)
            })
            .unwrap();

        assert_eq!(prs, 3);
    }
}
