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
            &format!("{}..{}", default_branch, branch),
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
                       commits = MAX(ticket_git_stats.commits, ?3),
                       prs_created = MAX(ticket_git_stats.prs_created, ?4),
                       lines_added = MAX(ticket_git_stats.lines_added, ?5),
                       lines_removed = MAX(ticket_git_stats.lines_removed, ?6),
                       files_changed = MAX(ticket_git_stats.files_changed, ?7),
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

    /// Refresh git stats for all tickets that have a branch_name.
    /// Returns the number of tickets updated.
    pub fn backfill_git_stats(&self) -> Result<u32, DbError> {
        let single_project_tickets: Vec<(String, String, Option<String>)> = self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT t.id, t.branch_name, p.path
                   FROM tickets t
                   JOIN projects p ON t.project_id = p.id
                   WHERE t.branch_name IS NOT NULL"#,
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
        for (ticket_id, branch_name, project_path) in single_project_tickets {
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

            if !branch_ref_exists(&working_dir, &branch_name) {
                continue;
            }

            let stats = collect_git_stats_for_ticket(&working_dir, &branch_name, &default_branch);
            if self.upsert_git_stats(&ticket_id, &stats).is_ok() {
                count += 1;
            }
        }

        let workspace_tickets: Vec<(String, String, String)> = self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"SELECT t.id, t.branch_name, p.path
                   FROM tickets t
                   JOIN workspace_projects wp ON t.workspace_id = wp.workspace_id
                   JOIN projects p ON wp.project_id = p.id
                   WHERE t.branch_name IS NOT NULL
                     AND t.workspace_id IS NOT NULL
                     AND t.project_id IS NULL
                   ORDER BY t.id, wp.position"#,
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?;
            Ok(rows.flatten().collect())
        })?;

        let mut current_ticket_id: Option<String> = None;
        let mut total = TicketGitStats::default();

        for (ticket_id, branch_name, project_path) in &workspace_tickets {
            if current_ticket_id.as_deref() != Some(ticket_id) {
                if let Some(ref prev_id) = current_ticket_id {
                    total.collected_at = chrono::Utc::now().to_rfc3339();
                    if self.upsert_git_stats(prev_id, &total).is_ok() {
                        count += 1;
                    }
                }
                current_ticket_id = Some(ticket_id.clone());
                total = TicketGitStats::default();
            }

            let working_dir = find_worktree_or_project(project_path, branch_name);
            let default_branch = match crate::commands::next_steps::get_default_branch(&working_dir) {
                Ok(b) => b,
                Err(_) => continue,
            };

            if !branch_ref_exists(&working_dir, branch_name) {
                continue;
            }

            let s = collect_git_stats_for_ticket(&working_dir, branch_name, &default_branch);
            total.commits += s.commits;
            total.lines_added += s.lines_added;
            total.lines_removed += s.lines_removed;
            total.files_changed += s.files_changed;
        }

        if let Some(ref prev_id) = current_ticket_id {
            total.collected_at = chrono::Utc::now().to_rfc3339();
            if self.upsert_git_stats(prev_id, &total).is_ok() {
                count += 1;
            }
        }
        Ok(count)
    }
}

fn branch_ref_exists(working_dir: &str, branch: &str) -> bool {
    Command::new("git")
        .args(["rev-parse", "--verify", &format!("refs/heads/{}", branch)])
        .current_dir(working_dir)
        .stderr(std::process::Stdio::null())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Find the worktree path for a branch, falling back to the project path.
fn find_worktree_or_project(project_path: &str, branch: &str) -> String {
    crate::commands::next_steps::resolve_working_dir_for_project(project_path, branch)
        .unwrap_or_else(|_| project_path.to_string())
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
            workspace_id: None,
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
    fn upsert_preserves_pr_count_when_stats_pass_zero() {
        let db = create_test_db();
        let ticket = create_ticket(&db);

        db.upsert_git_stats(&ticket.id, &make_stats(1, 0, 10, 5, 2))
            .unwrap();
        db.increment_pr_count(&ticket.id).unwrap();
        db.increment_pr_count(&ticket.id).unwrap();

        db.upsert_git_stats(&ticket.id, &make_stats(3, 0, 30, 10, 5))
            .unwrap();

        let (commits, prs): (i64, i64) = db
            .with_conn(|conn| {
                conn.query_row(
                    "SELECT commits, prs_created FROM ticket_git_stats WHERE ticket_id = ?",
                    [&ticket.id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(crate::db::DbError::Sqlite)
            })
            .unwrap();

        assert_eq!(commits, 3, "commits should be updated");
        assert_eq!(prs, 2, "prs_created should be preserved, not reset to 0");
    }

    #[test]
    fn upsert_zero_stats_does_not_overwrite_existing_data() {
        let db = create_test_db();
        let ticket = create_ticket(&db);

        db.upsert_git_stats(&ticket.id, &make_stats(5, 0, 100, 40, 8))
            .unwrap();
        db.increment_pr_count(&ticket.id).unwrap();

        // Simulate what happens when a deleted branch produces all-zero stats.
        // The backfill caller now skips deleted branches, but the upsert itself
        // should also guard against zeroing out real data.
        db.upsert_git_stats(&ticket.id, &make_stats(0, 0, 0, 0, 0))
            .unwrap();

        let row: (i64, i64, i64, i64, i64) = db
            .with_conn(|conn| {
                conn.query_row(
                    "SELECT commits, prs_created, lines_added, lines_removed, files_changed FROM ticket_git_stats WHERE ticket_id = ?",
                    [&ticket.id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
                ).map_err(crate::db::DbError::Sqlite)
            })
            .unwrap();

        assert_eq!(row, (5, 1, 100, 40, 8), "zero upsert must not erase existing stats");
    }

    #[test]
    fn upsert_higher_stats_still_updates() {
        let db = create_test_db();
        let ticket = create_ticket(&db);

        db.upsert_git_stats(&ticket.id, &make_stats(2, 0, 20, 5, 3))
            .unwrap();
        db.upsert_git_stats(&ticket.id, &make_stats(7, 0, 80, 25, 10))
            .unwrap();

        let row: (i64, i64, i64, i64) = db
            .with_conn(|conn| {
                conn.query_row(
                    "SELECT commits, lines_added, lines_removed, files_changed FROM ticket_git_stats WHERE ticket_id = ?",
                    [&ticket.id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                ).map_err(crate::db::DbError::Sqlite)
            })
            .unwrap();

        assert_eq!(row, (7, 80, 25, 10), "higher values should be accepted");
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

    // --- find_worktree_or_project ---

    fn init_temp_repo() -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().to_str().unwrap().to_string();

        Command::new("git")
            .args(["init"])
            .current_dir(&path)
            .output()
            .expect("git init");
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(&path)
            .output()
            .expect("git config email");
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&path)
            .output()
            .expect("git config name");

        std::fs::write(dir.path().join("README.md"), "# init").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(&path)
            .output()
            .expect("git add");
        Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(&path)
            .output()
            .expect("git commit");

        (dir, path)
    }

    #[test]
    fn find_worktree_no_match_returns_project_path() {
        let (_dir, path) = init_temp_repo();
        let result = find_worktree_or_project(&path, "feat/nonexistent");
        assert_eq!(result, path);
    }

    #[test]
    fn find_worktree_with_matching_worktree() {
        let (_dir, path) = init_temp_repo();

        Command::new("git")
            .args(["branch", "feat/wt-match"])
            .current_dir(&path)
            .output()
            .expect("create branch");

        let wt_dir = tempfile::tempdir().expect("wt dir");
        let wt_path = wt_dir.path().to_str().unwrap().to_string();
        std::fs::remove_dir(&wt_path).ok();

        Command::new("git")
            .args(["worktree", "add", &wt_path, "feat/wt-match"])
            .current_dir(&path)
            .output()
            .expect("git worktree add");

        let result = find_worktree_or_project(&path, "feat/wt-match");
        let canon = |p: &str| std::fs::canonicalize(p).unwrap_or_else(|_| std::path::PathBuf::from(p));
        assert_eq!(canon(&result), canon(&wt_path));
    }

    #[test]
    fn find_worktree_stale_reference_falls_back() {
        let (_dir, path) = init_temp_repo();

        Command::new("git")
            .args(["branch", "feat/stale-wt"])
            .current_dir(&path)
            .output()
            .expect("create branch");

        let wt_dir = tempfile::tempdir().expect("wt dir");
        let wt_path = wt_dir.path().to_str().unwrap().to_string();
        std::fs::remove_dir(&wt_path).ok();

        Command::new("git")
            .args(["worktree", "add", &wt_path, "feat/stale-wt"])
            .current_dir(&path)
            .output()
            .expect("git worktree add");

        // Remove directory to make it stale
        std::fs::remove_dir_all(&wt_path).expect("remove wt dir");

        let result = find_worktree_or_project(&path, "feat/stale-wt");
        assert_eq!(result, path, "stale worktree should fall back to project path");
    }

    #[test]
    fn find_worktree_invalid_path_returns_project_path() {
        let result = find_worktree_or_project("/nonexistent/path/xyz", "main");
        assert_eq!(result, "/nonexistent/path/xyz");
    }

    // --- branch_ref_exists ---

    #[test]
    fn branch_ref_exists_returns_true_for_existing_branch() {
        let (_dir, path) = init_temp_repo();
        let branch_output = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&path)
            .output()
            .expect("git branch");
        let branch = String::from_utf8_lossy(&branch_output.stdout).trim().to_string();

        assert!(branch_ref_exists(&path, &branch));
    }

    #[test]
    fn branch_ref_exists_returns_false_for_missing_branch() {
        let (_dir, path) = init_temp_repo();
        assert!(!branch_ref_exists(&path, "feat/does-not-exist"));
    }
}
