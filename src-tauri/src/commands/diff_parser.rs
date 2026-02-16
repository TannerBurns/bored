//! Pure diff-parsing logic: converts unified-diff text into structured per-file diffs.

/// Per-file diff for the file-by-file diff viewer
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub path: String,
    /// "modified", "added", "deleted", "renamed"
    pub status: String,
    pub additions: usize,
    pub deletions: usize,
    pub hunks: Vec<DiffHunk>,
}

/// A hunk in a unified diff (e.g. @@ -1,5 +1,7 @@)
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

/// A single line in a hunk
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    /// "add", "delete", "context"
    pub line_type: String,
    pub content: String,
    pub old_line_num: Option<usize>,
    pub new_line_num: Option<usize>,
}

/// Parse unified diff output into per-file structured diffs.
pub fn parse_unified_diff(diff: &str) -> Vec<FileDiff> {
    let mut files = Vec::new();
    let diff_splits = diff.split("\ndiff --git ");
    for (i, block) in diff_splits.enumerate() {
        let block = block.trim_start();
        let block = if i == 0 && !block.starts_with("diff --git ") {
            continue;
        } else if i == 0 {
            block.strip_prefix("diff --git ").unwrap_or(block)
        } else {
            block
        };
        let mut lines = block.lines();
        let first = match lines.next() {
            Some(l) => l,
            None => continue,
        };
        // First line: "a/path b/path" or "a/path b/path\n"
        let path = first
            .strip_prefix("a/")
            .and_then(|s| s.split(" b/").next())
            .unwrap_or(first)
            .to_string();
        if path.is_empty() {
            continue;
        }
        let (mut additions, mut deletions) = (0usize, 0usize);
        let mut hunks = Vec::new();
        let mut in_header = true;
        let mut header_status: Option<&str> = None;
        let mut current_hunk_header = String::new();
        let mut current_hunk_lines: Vec<DiffLine> = Vec::new();
        let mut old_line = 0usize;
        let mut new_line = 0usize;

        for line in lines {
            if line.starts_with("@@ ") {
                if !current_hunk_header.is_empty() {
                    hunks.push(DiffHunk {
                        header: current_hunk_header.clone(),
                        lines: current_hunk_lines.clone(),
                    });
                }
                current_hunk_header = line.to_string();
                current_hunk_lines.clear();
                if let Some(rest) = line.strip_prefix("@@ ") {
                    // Use split_once instead of strip_suffix so that
                    // function context after the closing @@ is ignored
                    // (e.g. "@@ -10,3 +10,4 @@ fn main()").
                    if let Some((nums, _)) = rest.split_once(" @@") {
                        let parts: Vec<&str> = nums.split(' ').collect();
                        if let Some(old_part) = parts.first() {
                            old_line = old_part.split(',').next().and_then(|s| s.trim_start_matches('-').parse().ok()).unwrap_or(1);
                        }
                        if let Some(new_part) = parts.get(1) {
                            new_line = new_part.split(',').next().and_then(|s| s.trim_start_matches('+').parse().ok()).unwrap_or(1);
                        }
                    }
                }
                in_header = false;
                continue;
            }
            if in_header {
                if line.starts_with("new file mode") {
                    header_status = Some("added");
                } else if line.starts_with("deleted file mode") {
                    header_status = Some("deleted");
                } else if line.starts_with("rename from") || line.starts_with("rename to") {
                    header_status = Some("renamed");
                }
                continue;
            }
            let (line_type, content) = if let Some(rest) = line.get(1..) {
                match line.chars().next() {
                    Some('+') => {
                        additions += 1;
                        (Some("add"), rest.to_string())
                    }
                    Some('-') => {
                        deletions += 1;
                        (Some("delete"), rest.to_string())
                    }
                    Some(' ') => (Some("context"), rest.to_string()),
                    _ => (None, line.to_string()),
                }
            } else {
                (Some("context"), String::new())
            };
            if let Some(lt) = line_type {
                let (old_num, new_num) = match lt {
                    "add" => (None, Some(new_line)),
                    "delete" => (Some(old_line), None),
                    _ => (Some(old_line), Some(new_line)),
                };
                current_hunk_lines.push(DiffLine {
                    line_type: lt.to_string(),
                    content,
                    old_line_num: old_num,
                    new_line_num: new_num,
                });
                match lt {
                    "add" => new_line = new_line.saturating_add(1),
                    "delete" => old_line = old_line.saturating_add(1),
                    _ => {
                        old_line = old_line.saturating_add(1);
                        new_line = new_line.saturating_add(1);
                    }
                }
            }
        }
        if !current_hunk_header.is_empty() {
            hunks.push(DiffHunk {
                header: current_hunk_header,
                lines: current_hunk_lines,
            });
        }
        let status = if let Some(s) = header_status {
            s
        } else if additions > 0 && deletions == 0 && hunks.iter().all(|h| h.lines.iter().all(|l| l.line_type != "delete")) {
            "added"
        } else if deletions > 0 && additions == 0 && hunks.iter().all(|h| h.lines.iter().all(|l| l.line_type != "add")) {
            "deleted"
        } else {
            "modified"
        };
        files.push(FileDiff {
            path,
            status: status.to_string(),
            additions,
            deletions,
            hunks,
        });
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_diff() {
        assert!(parse_unified_diff("").is_empty());
    }

    #[test]
    fn parse_single_modified_file() {
        let diff = "\
diff --git a/src/main.rs b/src/main.rs
index abc..def 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,3 @@
 fn main() {
-    let x = 1;
+    let x = 2;
 }
";
        let files = parse_unified_diff(diff);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "src/main.rs");
        assert_eq!(files[0].status, "modified");
        assert_eq!(files[0].additions, 1);
        assert_eq!(files[0].deletions, 1);
        assert_eq!(files[0].hunks.len(), 1);
        assert!(files[0].hunks[0].header.starts_with("@@ "));
    }

    #[test]
    fn parse_new_file() {
        let diff = "\
diff --git a/new.txt b/new.txt
new file mode 100644
index 0000000..abc1234
--- /dev/null
+++ b/new.txt
@@ -0,0 +1,2 @@
+line one
+line two
";
        let files = parse_unified_diff(diff);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "new.txt");
        assert_eq!(files[0].status, "added");
        assert_eq!(files[0].additions, 2);
        assert_eq!(files[0].deletions, 0);
    }

    #[test]
    fn parse_deleted_file() {
        let diff = "\
diff --git a/old.txt b/old.txt
deleted file mode 100644
index abc1234..0000000
--- a/old.txt
+++ /dev/null
@@ -1,2 +0,0 @@
-line one
-line two
";
        let files = parse_unified_diff(diff);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "old.txt");
        assert_eq!(files[0].status, "deleted");
        assert_eq!(files[0].additions, 0);
        assert_eq!(files[0].deletions, 2);
    }

    #[test]
    fn parse_multiple_files() {
        let diff = "\
diff --git a/a.rs b/a.rs
index abc..def 100644
--- a/a.rs
+++ b/a.rs
@@ -1,1 +1,2 @@
 existing
+added

diff --git a/b.rs b/b.rs
index abc..def 100644
--- a/b.rs
+++ b/b.rs
@@ -1,2 +1,1 @@
 keep
-removed
";
        let files = parse_unified_diff(diff);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "a.rs");
        assert_eq!(files[0].additions, 1);
        assert_eq!(files[1].path, "b.rs");
        assert_eq!(files[1].deletions, 1);
    }

    #[test]
    fn parse_line_numbers_tracked_correctly() {
        let diff = "\
diff --git a/f.rs b/f.rs
index abc..def 100644
--- a/f.rs
+++ b/f.rs
@@ -10,3 +10,4 @@
 context
+added
-removed
 context2
";
        let files = parse_unified_diff(diff);
        assert_eq!(files.len(), 1);
        let lines = &files[0].hunks[0].lines;

        // "context" at old=10, new=10
        assert_eq!(lines[0].line_type, "context");
        assert_eq!(lines[0].old_line_num, Some(10));
        assert_eq!(lines[0].new_line_num, Some(10));

        // "added" at new=11 (no old)
        assert_eq!(lines[1].line_type, "add");
        assert_eq!(lines[1].old_line_num, None);
        assert_eq!(lines[1].new_line_num, Some(11));

        // "removed" at old=11 (no new)
        assert_eq!(lines[2].line_type, "delete");
        assert_eq!(lines[2].old_line_num, Some(11));
        assert_eq!(lines[2].new_line_num, None);
    }

    #[test]
    fn parse_multiple_hunks() {
        let diff = "\
diff --git a/f.rs b/f.rs
index abc..def 100644
--- a/f.rs
+++ b/f.rs
@@ -1,2 +1,2 @@
-old1
+new1
 same
@@ -10,2 +10,2 @@
-old2
+new2
 same2
";
        let files = parse_unified_diff(diff);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].hunks.len(), 2);
        assert_eq!(files[0].additions, 2);
        assert_eq!(files[0].deletions, 2);
    }

    #[test]
    fn parse_hunk_header_with_function_context() {
        let diff = "\
diff --git a/f.rs b/f.rs
index abc..def 100644
--- a/f.rs
+++ b/f.rs
@@ -10,3 +10,4 @@ fn main() {
 context
+added
-removed
 context2
";
        let files = parse_unified_diff(diff);
        assert_eq!(files.len(), 1);
        let lines = &files[0].hunks[0].lines;

        assert_eq!(lines[0].line_type, "context");
        assert_eq!(lines[0].old_line_num, Some(10));
        assert_eq!(lines[0].new_line_num, Some(10));

        assert_eq!(lines[1].line_type, "add");
        assert_eq!(lines[1].old_line_num, None);
        assert_eq!(lines[1].new_line_num, Some(11));

        assert_eq!(lines[2].line_type, "delete");
        assert_eq!(lines[2].old_line_num, Some(11));
        assert_eq!(lines[2].new_line_num, None);
    }

    #[test]
    fn parse_renamed_file() {
        let diff = "\
diff --git a/old_name.rs b/new_name.rs
similarity index 95%
rename from old_name.rs
rename to new_name.rs
index abc..def 100644
--- a/old_name.rs
+++ b/new_name.rs
@@ -1,1 +1,1 @@
-old
+new
";
        let files = parse_unified_diff(diff);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].status, "renamed");
    }
}
