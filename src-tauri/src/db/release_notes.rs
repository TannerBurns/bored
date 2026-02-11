//! Release notes database operations.
//!
//! Release notes are embedded in the binary and seeded into the database
//! on startup. When creating a new release, add an entry to KNOWN_RELEASE_NOTES.

use super::{Database, DbError};

/// A release note entry to seed into the database.
struct KnownReleaseNote {
    version: &'static str,
    published_at: &'static str,
    summary: &'static str,
    notes_json: &'static str,
}

/// Embedded release notes — add a new entry here for each release.
///
/// When preparing a release:
/// 1. Review all commits since the last tag: `git log <last-tag>..HEAD --oneline`
/// 2. Categorize changes into "New Features", "Improvements", and "Bug Fixes"
/// 3. Add a new KnownReleaseNote entry below with the release version
const KNOWN_RELEASE_NOTES: &[KnownReleaseNote] = &[
    KnownReleaseNote {
        version: "0.1.0-beta.11",
        published_at: "2026-02-10",
        summary: "Workflow presets, cost tracking, spec shortcuts, and release notes",
        notes_json: r#"[
            {
                "category": "New Features",
                "items": [
                    "Per-stage AI workflow settings with presets (Comprehensive, Balanced, Quick Fix, etc.)",
                    "Agent cost tracking with per-run and per-ticket cost summaries",
                    "Backfill button in Data Settings for retroactive cost calculation",
                    "Spec progress shortcut for quick access to epic progress view",
                    "Opus 4.5 model support",
                    "\"What's New\" release notes shown on version upgrade"
                ]
            },
            {
                "category": "Improvements",
                "items": [
                    "Spec Agent settings extracted into dedicated settings tab",
                    "Themed confirmation dialogs replace native browser confirm()",
                    "Simplified spec creation (removed redundant per-spec model selector)",
                    "Versioned model identifiers for consistency across all settings",
                    "Spec list cards show project name alongside board name"
                ]
            },
            {
                "category": "Bug Fixes",
                "items": [
                    "Fixed cost backfill retry logic for tickets with new runs",
                    "Fixed CostBadge showing \"Unavailable\" for Cursor runs with estimated costs",
                    "Fixed spec generating indicator race condition causing UI flicker",
                    "Fixed output truncation boundary for multi-byte UTF-8 characters",
                    "Fixed model mapping for unversioned model values"
                ]
            }
        ]"#,
    },
];

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
    /// Seed known release notes into the database (idempotent).
    /// Called on every startup so new versions are populated automatically.
    pub fn seed_release_notes(&self) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "INSERT OR IGNORE INTO release_notes (version, published_at, summary, notes_json) VALUES (?1, ?2, ?3, ?4)",
            )?;

            for note in KNOWN_RELEASE_NOTES {
                stmt.execute(rusqlite::params![
                    note.version,
                    note.published_at,
                    note.summary,
                    note.notes_json,
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

    /// Get all release notes, ordered by version descending.
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
    fn seed_release_notes_is_idempotent() {
        let db = create_test_db();
        // seed_release_notes is already called in open_in_memory via migrate + seed
        // Call it again to verify idempotency
        db.seed_release_notes().unwrap();

        let all = db.get_all_release_notes().unwrap();
        assert_eq!(all.len(), KNOWN_RELEASE_NOTES.len());
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

        // Verify categories
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

        // Each entry should have parsed notes
        for note in &all {
            assert!(!note.version.is_empty());
            assert!(!note.published_at.is_empty());
            assert!(!note.notes.is_empty());
        }
    }
}
