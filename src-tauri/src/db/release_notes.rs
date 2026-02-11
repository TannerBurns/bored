//! Release notes database operations.
//!
//! Release notes are stored in `src-tauri/release-notes.json` and embedded
//! into the binary at compile time via `include_str!`. On startup they are
//! seeded into SQLite with INSERT OR IGNORE so each version is inserted once.
//!
//! ## Adding release notes for a new version
//!
//! 1. Run `scripts/draft-release-notes.sh` to auto-generate a draft entry
//!    from the git log since the last tag.
//! 2. Review and edit the draft — commit messages don't always make good
//!    user-facing notes.
//! 3. Prepend the entry to `src-tauri/release-notes.json` (newest first).
//! 4. Commit, tag, and push.

use super::{Database, DbError};

/// Raw JSON embedded at compile time from the release-notes file.
const RELEASE_NOTES_JSON: &str = include_str!("../../release-notes.json");

/// Shape of each entry in release-notes.json (deserialization only).
#[derive(serde::Deserialize)]
struct FileReleaseNote {
    version: String,
    published_at: String,
    summary: Option<String>,
    notes: Vec<ReleaseNoteCategory>,
}

/// Serializable release note returned to the frontend.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseNote {
    pub version: String,
    pub published_at: String,
    pub summary: Option<String>,
    pub notes: Vec<ReleaseNoteCategory>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReleaseNoteCategory {
    pub category: String,
    pub items: Vec<String>,
}

impl Database {
    /// Seed release notes from the embedded JSON into the database (idempotent).
    /// Called on every startup so new versions are populated automatically.
    pub fn seed_release_notes(&self) -> Result<(), DbError> {
        let entries: Vec<FileReleaseNote> =
            serde_json::from_str(RELEASE_NOTES_JSON).map_err(|e| {
                DbError::Validation(format!("Failed to parse release-notes.json: {}", e))
            })?;

        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "INSERT OR IGNORE INTO release_notes (version, published_at, summary, notes_json) VALUES (?1, ?2, ?3, ?4)",
            )?;

            for entry in &entries {
                let notes_json = serde_json::to_string(&entry.notes).unwrap_or_else(|_| "[]".to_string());
                stmt.execute(rusqlite::params![
                    entry.version,
                    entry.published_at,
                    entry.summary,
                    notes_json,
                ])?;
            }

            Ok(())
        })
    }

    /// Get release notes for a specific version.
    pub fn get_release_notes(&self, version: &str) -> Result<Option<ReleaseNote>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT version, published_at, summary, notes_json FROM release_notes WHERE version = ?1",
            )?;

            let result = stmt
                .query_row(rusqlite::params![version], |row| {
                    let version: String = row.get(0)?;
                    let published_at: String = row.get(1)?;
                    let summary: Option<String> = row.get(2)?;
                    let notes_json: String = row.get(3)?;
                    Ok((version, published_at, summary, notes_json))
                })
                .ok();

            match result {
                Some((version, published_at, summary, notes_json)) => {
                    let notes: Vec<ReleaseNoteCategory> =
                        serde_json::from_str(&notes_json).unwrap_or_default();
                    Ok(Some(ReleaseNote {
                        version,
                        published_at,
                        summary,
                        notes,
                    }))
                }
                None => Ok(None),
            }
        })
    }

    /// Get all release notes, ordered by most recent first.
    pub fn get_all_release_notes(&self) -> Result<Vec<ReleaseNote>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT version, published_at, summary, notes_json FROM release_notes ORDER BY published_at DESC, version DESC",
            )?;

            let rows = stmt
                .query_map([], |row| {
                    let version: String = row.get(0)?;
                    let published_at: String = row.get(1)?;
                    let summary: Option<String> = row.get(2)?;
                    let notes_json: String = row.get(3)?;
                    Ok((version, published_at, summary, notes_json))
                })?
                .collect::<Result<Vec<_>, _>>()?;

            let notes = rows
                .into_iter()
                .map(|(version, published_at, summary, notes_json)| {
                    let notes: Vec<ReleaseNoteCategory> =
                        serde_json::from_str(&notes_json).unwrap_or_default();
                    ReleaseNote {
                        version,
                        published_at,
                        summary,
                        notes,
                    }
                })
                .collect();

            Ok(notes)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn embedded_json_parses_successfully() {
        let entries: Vec<FileReleaseNote> =
            serde_json::from_str(RELEASE_NOTES_JSON).expect("release-notes.json should parse");
        assert!(!entries.is_empty(), "release-notes.json should have at least one entry");

        for entry in &entries {
            assert!(!entry.version.is_empty());
            assert!(!entry.published_at.is_empty());
            assert!(!entry.notes.is_empty());
        }
    }

    #[test]
    fn seed_release_notes_is_idempotent() {
        let db = create_test_db();
        // seed_release_notes is already called in open_in_memory
        // Call it again to verify idempotency
        db.seed_release_notes().unwrap();

        let entries: Vec<FileReleaseNote> =
            serde_json::from_str(RELEASE_NOTES_JSON).unwrap();
        let all = db.get_all_release_notes().unwrap();
        assert_eq!(all.len(), entries.len());
    }

    #[test]
    fn get_release_notes_returns_known_version() {
        let db = create_test_db();
        let note = db.get_release_notes("0.1.0-beta.11").unwrap();
        assert!(note.is_some());

        let note = note.unwrap();
        assert_eq!(note.version, "0.1.0-beta.11");
        assert_eq!(note.published_at, "2026-02-10");
        assert!(note.summary.is_some());
        assert!(!note.notes.is_empty());

        let categories: Vec<&str> = note.notes.iter().map(|c| c.category.as_str()).collect();
        assert!(categories.contains(&"New Features"));
        assert!(categories.contains(&"Improvements"));
        assert!(categories.contains(&"Bug Fixes"));
    }

    #[test]
    fn get_release_notes_returns_none_for_unknown_version() {
        let db = create_test_db();
        let note = db.get_release_notes("99.99.99").unwrap();
        assert!(note.is_none());
    }

    #[test]
    fn get_all_release_notes_returns_all() {
        let db = create_test_db();
        let all = db.get_all_release_notes().unwrap();
        assert!(!all.is_empty());

        for note in &all {
            assert!(!note.version.is_empty());
            assert!(!note.published_at.is_empty());
            assert!(!note.notes.is_empty());
        }
    }
}
