//! Pure parsing helpers for extracting structured blocks from validation agent responses.
//! Shared by both the validation commands and the chat review mode runner.

use crate::agents::json_extraction::{find_balanced_from_offset, parse_all_json_blocks};
use crate::db::models::FixTask;

pub(crate) struct StartAppBlock {
    pub command: String,
    pub port: Option<i32>,
}

pub(crate) struct RunCommandBlock {
    pub command: String,
}

pub(crate) struct CreateFixTasksBlock {
    pub tasks: Vec<FixTask>,
}

pub(crate) fn parse_start_app_from_response(response_text: &str) -> Option<StartAppBlock> {
    for v in parse_all_json_blocks(response_text) {
        if let Some(start_app) = v.get("start_app").and_then(|s| s.as_object()) {
            if let Some(command) = start_app.get("command").and_then(|c| c.as_str()) {
                let port = start_app.get("port").and_then(|p| p.as_i64()).map(|p| p as i32);
                return Some(StartAppBlock {
                    command: command.to_string(),
                    port,
                });
            }
        }
    }
    None
}

pub(crate) fn parse_stop_app_from_response(response_text: &str) -> bool {
    for v in parse_all_json_blocks(response_text) {
        if v.get("stop_app").is_some() {
            return true;
        }
    }
    false
}

pub(crate) fn parse_run_command_from_response(response_text: &str) -> Option<RunCommandBlock> {
    for v in parse_all_json_blocks(response_text) {
        if let Some(rc) = v.get("run_command").and_then(|s| s.as_object()) {
            if let Some(command) = rc.get("command").and_then(|c| c.as_str()) {
                return Some(RunCommandBlock {
                    command: command.to_string(),
                });
            }
        }
    }
    None
}

fn parse_fix_task_from_json_obj(obj: &serde_json::Map<String, serde_json::Value>) -> FixTask {
    let title = obj.get("title").and_then(|t| t.as_str()).unwrap_or("Fix task");
    let description = obj.get("description").and_then(|d| d.as_str()).unwrap_or("");
    let acceptance_criteria = obj
        .get("acceptance_criteria")
        .or_else(|| obj.get("acceptanceCriteria"))
        .and_then(|ac| ac.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });
    FixTask {
        title: title.to_string(),
        description: description.to_string(),
        acceptance_criteria,
    }
}

pub(crate) fn parse_create_fix_tasks_from_response(
    response_text: &str,
) -> Option<CreateFixTasksBlock> {
    let mut all_tasks: Vec<FixTask> = Vec::new();

    let blocks = parse_all_json_blocks(response_text);
    tracing::debug!(
        "parse_all_json_blocks returned {} block(s) from {} chars",
        blocks.len(),
        response_text.len()
    );
    for v in blocks {
        extract_fix_tasks_from_value(&v, &mut all_tasks);
    }

    if !all_tasks.is_empty() {
        tracing::debug!(
            "Primary path: extracted {} task(s)",
            all_tasks.len()
        );
        return Some(CreateFixTasksBlock { tasks: all_tasks });
    }

    // Fallback: search for the key directly in the raw text. This handles
    // cases where extract_all_json_code_blocks is confused by triple-backtick
    // sequences inside JSON string values (e.g. markdown code blocks in task
    // descriptions), causing parse_all_json_blocks to miss the block entirely.
    for key in &["\"create_fix_tasks\"", "\"create_fix_task\""] {
        if let Some(key_pos) = response_text.find(key) {
            tracing::debug!("Fallback: found key {} at pos {}", key, key_pos);
            if let Some(brace_pos) = response_text[..key_pos].rfind('{') {
                // Strategy 1: balanced brace extraction (handles most cases).
                if let Some(balanced) =
                    find_balanced_from_offset(response_text, brace_pos, '{', '}')
                {
                    tracing::debug!(
                        "Fallback balanced: extracted {} chars",
                        balanced.len()
                    );
                    if let Ok(v) =
                        serde_json::from_str::<serde_json::Value>(&balanced)
                    {
                        extract_fix_tasks_from_value(&v, &mut all_tasks);
                        if !all_tasks.is_empty() {
                            tracing::debug!(
                                "Fallback balanced path: extracted {} task(s)",
                                all_tasks.len()
                            );
                            return Some(CreateFixTasksBlock { tasks: all_tasks });
                        }
                    }
                }

                // Strategy 2: if balanced extraction fails (e.g. model emits
                // unescaped quotes inside description strings that confuse the
                // string-aware brace matcher), try parsing from the opening
                // brace to the end of the response. The JSON block is typically
                // the last thing in the response, so trimming trailing
                // whitespace and trying serde directly often works even when
                // the brace matcher can't find the boundary.
                let tail = response_text[brace_pos..].trim();
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(tail) {
                    tracing::debug!(
                        "Fallback tail-parse: parsed {} chars directly",
                        tail.len()
                    );
                    extract_fix_tasks_from_value(&v, &mut all_tasks);
                    if !all_tasks.is_empty() {
                        tracing::debug!(
                            "Fallback tail-parse path: extracted {} task(s)",
                            all_tasks.len()
                        );
                        return Some(CreateFixTasksBlock { tasks: all_tasks });
                    }
                }

                // Strategy 3: find the last `}` in the text and try parsing
                // from the opening brace to that position. Handles cases where
                // there is trailing text after the JSON block.
                if let Some(last_brace) = response_text.rfind('}') {
                    if last_brace > brace_pos {
                        let substr = &response_text[brace_pos..=last_brace];
                        if let Ok(v) =
                            serde_json::from_str::<serde_json::Value>(substr)
                        {
                            tracing::debug!(
                                "Fallback last-brace: parsed {}..={} ({} chars)",
                                brace_pos,
                                last_brace,
                                substr.len()
                            );
                            extract_fix_tasks_from_value(&v, &mut all_tasks);
                            if !all_tasks.is_empty() {
                                tracing::debug!(
                                    "Fallback last-brace path: extracted {} task(s)",
                                    all_tasks.len()
                                );
                                return Some(CreateFixTasksBlock {
                                    tasks: all_tasks,
                                });
                            }
                        }
                    }
                }

                tracing::debug!(
                    "Fallback: all strategies failed for key {} at brace_pos {}",
                    key,
                    brace_pos
                );
            }
        }
    }

    None
}

fn extract_fix_tasks_from_value(v: &serde_json::Value, all_tasks: &mut Vec<FixTask>) {
    if let Some(cft) = v.get("create_fix_tasks").and_then(|s| s.as_object()) {
        if let Some(tasks_arr) = cft.get("tasks").and_then(|t| t.as_array()) {
            for tv in tasks_arr {
                if let Some(obj) = tv.as_object() {
                    all_tasks.push(parse_fix_task_from_json_obj(obj));
                }
            }
        }
    } else if let Some(cft) = v.get("create_fix_task").and_then(|s| s.as_object()) {
        all_tasks.push(parse_fix_task_from_json_obj(cft));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_app_with_port() {
        let text = r#"```json
{ "start_app": { "command": "npm run dev", "port": 5173 } }
```"#;
        let result = parse_start_app_from_response(text).unwrap();
        assert_eq!(result.command, "npm run dev");
        assert_eq!(result.port, Some(5173));
    }

    #[test]
    fn start_app_without_port() {
        let text = r#"```json
{ "start_app": { "command": "python manage.py runserver" } }
```"#;
        let result = parse_start_app_from_response(text).unwrap();
        assert_eq!(result.command, "python manage.py runserver");
        assert_eq!(result.port, None);
    }

    #[test]
    fn start_app_missing_command_returns_none() {
        let text = r#"```json
{ "start_app": { "port": 3000 } }
```"#;
        assert!(parse_start_app_from_response(text).is_none());
    }

    #[test]
    fn start_app_no_block_returns_none() {
        assert!(parse_start_app_from_response("Just text, no JSON.").is_none());
    }

    #[test]
    fn stop_app_detected() {
        let text = r#"I'll stop the app now.
```json
{ "stop_app": {} }
```"#;
        assert!(parse_stop_app_from_response(text));
    }

    #[test]
    fn stop_app_missing_returns_false() {
        assert!(!parse_stop_app_from_response("Just text, no JSON."));
    }

    #[test]
    fn run_command_extracts_command() {
        let text = r#"```json
{ "run_command": { "command": "npm install" } }
```"#;
        let result = parse_run_command_from_response(text).unwrap();
        assert_eq!(result.command, "npm install");
    }

    #[test]
    fn run_command_missing_returns_none() {
        assert!(parse_run_command_from_response("No command here.").is_none());
    }

    #[test]
    fn fix_task_full_fields() {
        let obj: serde_json::Value = serde_json::json!({
            "title": "Fix login",
            "description": "The login form is broken",
            "acceptance_criteria": ["Form submits", "Shows error on invalid"]
        });
        let task = parse_fix_task_from_json_obj(obj.as_object().unwrap());
        assert_eq!(task.title, "Fix login");
        assert_eq!(task.description, "The login form is broken");
        assert_eq!(
            task.acceptance_criteria,
            Some(vec!["Form submits".to_string(), "Shows error on invalid".to_string()])
        );
    }

    #[test]
    fn fix_task_camel_case_acceptance_criteria() {
        let obj: serde_json::Value = serde_json::json!({
            "title": "Fix it",
            "description": "desc",
            "acceptanceCriteria": ["criterion"]
        });
        let task = parse_fix_task_from_json_obj(obj.as_object().unwrap());
        assert_eq!(task.acceptance_criteria, Some(vec!["criterion".to_string()]));
    }

    #[test]
    fn fix_task_defaults_on_missing_fields() {
        let obj: serde_json::Value = serde_json::json!({});
        let task = parse_fix_task_from_json_obj(obj.as_object().unwrap());
        assert_eq!(task.title, "Fix task");
        assert_eq!(task.description, "");
        assert!(task.acceptance_criteria.is_none());
    }

    #[test]
    fn fix_tasks_plural_form() {
        let text = r#"```json
{ "create_fix_tasks": { "tasks": [
    { "title": "Fix A", "description": "desc A" },
    { "title": "Fix B", "description": "desc B" }
] } }
```"#;
        let block = parse_create_fix_tasks_from_response(text).unwrap();
        assert_eq!(block.tasks.len(), 2);
        assert_eq!(block.tasks[0].title, "Fix A");
        assert_eq!(block.tasks[1].title, "Fix B");
    }

    #[test]
    fn fix_tasks_plural_empty_array_returns_none() {
        let text = r#"```json
{ "create_fix_tasks": { "tasks": [] } }
```"#;
        assert!(parse_create_fix_tasks_from_response(text).is_none());
    }

    #[test]
    fn fix_tasks_no_block_returns_none() {
        assert!(parse_create_fix_tasks_from_response("No fix tasks here.").is_none());
    }

    #[test]
    fn fix_tasks_ignores_unrelated_json_blocks() {
        let text = r#"Let me run a command first.

```json
{ "run_command": { "command": "npm test" } }
```

I found an issue:

```json
{ "create_fix_tasks": { "tasks": [{ "title": "Fix test failure", "description": "Tests are failing" }] } }
```"#;
        let block = parse_create_fix_tasks_from_response(text).unwrap();
        assert_eq!(block.tasks.len(), 1);
        assert_eq!(block.tasks[0].title, "Fix test failure");
    }

    #[test]
    fn fix_tasks_multiple_plural_blocks() {
        let text = r#"```json
{ "create_fix_tasks": { "tasks": [
    { "title": "A1", "description": "first batch" }
] } }
```

```json
{ "create_fix_tasks": { "tasks": [
    { "title": "B1", "description": "second batch" },
    { "title": "B2", "description": "second batch" }
] } }
```"#;
        let block = parse_create_fix_tasks_from_response(text).unwrap();
        assert_eq!(block.tasks.len(), 3);
        assert_eq!(block.tasks[0].title, "A1");
        assert_eq!(block.tasks[1].title, "B1");
        assert_eq!(block.tasks[2].title, "B2");
    }

    #[test]
    fn fix_tasks_single_item_array() {
        let text = r#"```json
{ "create_fix_tasks": { "tasks": [{ "title": "Solo fix", "description": "Just one" }] } }
```"#;
        let block = parse_create_fix_tasks_from_response(text).unwrap();
        assert_eq!(block.tasks.len(), 1);
        assert_eq!(block.tasks[0].title, "Solo fix");
    }

    // ── singular create_fix_task fallback ──────────────────────

    #[test]
    fn fix_task_singular_in_code_fence() {
        let text = r#"```json
{ "create_fix_task": { "title": "Fix login", "description": "Login broken" } }
```"#;
        let block = parse_create_fix_tasks_from_response(text).unwrap();
        assert_eq!(block.tasks.len(), 1);
        assert_eq!(block.tasks[0].title, "Fix login");
        assert_eq!(block.tasks[0].description, "Login broken");
    }

    #[test]
    fn fix_task_singular_bare_json() {
        let text = r#"Found a bug.

{ "create_fix_task": { "title": "Fix crash", "description": "App crashes on start" } }

Please review."#;
        let block = parse_create_fix_tasks_from_response(text).unwrap();
        assert_eq!(block.tasks.len(), 1);
        assert_eq!(block.tasks[0].title, "Fix crash");
    }

    #[test]
    fn fix_task_singular_with_acceptance_criteria() {
        let text = r#"```json
{ "create_fix_task": { "title": "Fix it", "description": "desc", "acceptance_criteria": ["Tests pass"] } }
```"#;
        let block = parse_create_fix_tasks_from_response(text).unwrap();
        assert_eq!(block.tasks[0].acceptance_criteria, Some(vec!["Tests pass".to_string()]));
    }

    // ── direct-search fallback (triple-backticks in description) ──

    #[test]
    fn fix_tasks_bare_json_with_backticks_in_description() {
        let text = concat!(
            "Here are the issues I found:\n\n",
            "**Root Cause:** The query is wrong.\n\n",
            r#"{ "create_fix_tasks": { "tasks": [{ "title": "Fix SQL query", "description": "The query is:\n```sql\nSELECT * FROM t\n```\nFix it." }] } }"#,
        );
        let block = parse_create_fix_tasks_from_response(text).unwrap();
        assert_eq!(block.tasks.len(), 1);
        assert_eq!(block.tasks[0].title, "Fix SQL query");
        assert!(block.tasks[0].description.contains("SELECT * FROM t"));
    }

    #[test]
    fn fix_tasks_bare_json_with_multiple_code_blocks_in_description() {
        let text = concat!(
            "Analysis complete.\n\n",
            r#"{ "create_fix_tasks": { "tasks": [{ "title": "Fix tests", "description": "Problem:\n```go\nfixture[\"cwd\"] = hookDir\nfixtureData, _ = json.Marshal(fixture)\n```\n\nAlso fix:\n```makefile\ntest-coverage:\n\tDB_HOST=localhost $(GO_CMD) test\n```\n\nDone." }] } }"#,
        );
        let block = parse_create_fix_tasks_from_response(text).unwrap();
        assert_eq!(block.tasks.len(), 1);
        assert_eq!(block.tasks[0].title, "Fix tests");
        assert!(block.tasks[0].description.contains("fixture"));
        assert!(block.tasks[0].description.contains("makefile"));
    }

    #[test]
    fn fix_tasks_singular_bare_with_backticks_in_description() {
        let text = concat!(
            "Found an issue.\n\n",
            r#"{ "create_fix_task": { "title": "Fix it", "description": "See:\n```go\nfmt.Println()\n```\nDone." } }"#,
        );
        let block = parse_create_fix_tasks_from_response(text).unwrap();
        assert_eq!(block.tasks.len(), 1);
        assert_eq!(block.tasks[0].title, "Fix it");
    }

    #[test]
    fn fix_tasks_large_response_with_prose_and_bare_json() {
        let prose = "Now I have the full picture. Here are the issues I've identified:\n\n\
            **Root Cause 1: Fixture values are wrong.** The API requires valid UUIDs \
            but all fixtures use human-readable strings.\n\n\
            **Root Cause 2: Fixture cwd doesn't match.** Fixtures hardcode a path \
            but the test uses a temp dir.\n\n\
            **Root Cause 3: Test coverage includes hook tests.** The glob recursively \
            matches hook tests that need a different setup.\n\n";
        let json = r#"{ "create_fix_tasks": { "tasks": [{ "title": "Fix integration tests", "description": "Problem: Tests fail for three reasons.\n\nRequirements:\n\n### Fix 1\nUse valid UUIDs.\n\n### Fix 2\nDynamic cwd injection:\n```go\nfixture[\"cwd\"] = hookDir\nfixtureData, _ = json.Marshal(fixture)\n```\n\n### Fix 3\nExclude hook tests:\n```makefile\ntest-coverage:\n\t$(GO_CMD) test ./tests/integration\n```\n\nAcceptance Criteria:\n- All fixtures use UUIDs\n- Tests pass" }] } }"#;
        let text = format!("{}{}", prose, json);
        let block = parse_create_fix_tasks_from_response(&text).unwrap();
        assert_eq!(block.tasks.len(), 1);
        assert_eq!(block.tasks[0].title, "Fix integration tests");
        assert!(block.tasks[0].description.contains("Fix 1"));
        assert!(block.tasks[0].description.contains("Fix 3"));
    }

    #[test]
    fn fix_tasks_real_world_long_description_with_makefile_and_go_blocks() {
        // Reproduces the exact production failure: prose with quoted strings
        // like `"cwd"` and `"test-branch"`, followed by bare JSON whose
        // description contains ```makefile and ```go code blocks with escaped
        // quotes, tabs, and complex shell expressions.
        let text = concat!(
            "Now I have the full picture. Here are the issues found:\n\n",
            "**Primary bug:** All 22 fixture files set `\"cwd\": \"/tmp/test-repo\"` ",
            "but `smee_git_context()` runs `git -C \"$CWD\"` against that path. ",
            "This causes `REPO=\"\"` and `BRANCH=\"\"`, failing the workspace assertions.\n\n",
            "**Secondary issue:** `test-coverage` runs `./tests/integration/...` ",
            "which recursively includes hooks tests.\n\n",
            "Creating fix tasks:\n\n",
            r#"{ "create_fix_tasks": { "tasks": [{ "title": "Fix hook integration test fixture cwd mismatch and CI double-run", "description": "Problem: The hook integration tests fail because all 22 fixture JSON files hardcode `\"cwd\": \"/tmp/test-repo\"` but `smee_git_context()` in `_common.sh` runs `git -C \"$CWD\"` against that path.\n\nRequirements:\n\n### 1. Fix fixture cwd injection\n\nAfter `json.Unmarshal` into `fixture`, set `fixture[\"cwd\"] = hookDir`, then re-marshal.\n\n### 2. Exclude hooks from test-coverage\n\nIn the Makefile:\n```makefile\ntest-coverage:\n\tDB_HOST=localhost $(GO_CMD) test -v -tags=integration -coverprofile=coverage.out $(shell go list -tags=integration ./tests/integration/... | grep -v /hooks)\n```\n\n### 3. Verify locally\n\nRun `go vet -tags=integration ./tests/integration/hooks/...`\n\nAcceptance Criteria:\n- All 22 hook test fixtures have their `cwd` dynamically set\n- `TestHookScripts` workspace assertions pass\n- Hook tests do NOT run as part of `make test-coverage`\n- `go vet` passes" }] } }"#,
        );
        let block = parse_create_fix_tasks_from_response(text).unwrap();
        assert_eq!(block.tasks.len(), 1);
        assert_eq!(
            block.tasks[0].title,
            "Fix hook integration test fixture cwd mismatch and CI double-run"
        );
        assert!(block.tasks[0].description.contains("fixture cwd injection"));
        assert!(block.tasks[0].description.contains("test-coverage"));
        assert!(block.tasks[0].description.contains("go vet"));
    }

    #[test]
    fn fix_tasks_exact_production_output_v3() {
        // Exact reproduction of third production failure report.
        let text = "Good, the investigation reveals clear root causes. Here are the issues:\n\n\
            **Issue 1 - Fixture CWD mismatch (causes test failures):** All 22 fixture JSON files have \
            `\"cwd\": \"/tmp/test-repo\"` but `smee_git_context()` runs `git -C \"$CWD\"` using that \
            static path. The test creates a real git repo in a dynamic temp dir via `initGitRepo(t, hookDir)`, \
            but the fixture's CWD doesn't point there. So `REPO` and `BRANCH` resolve to empty strings, \
            and the assertions at `hooks_test.go:176-182` (`workspace.repo == \"test/test-repo\"`, \
            `workspace.branch == \"test-branch\"`) fail.\n\n\
            **Issue 2 - test-coverage job also runs hooks tests:** The Makefile `test-coverage` target \
            uses `./tests/integration/...` which includes `./tests/integration/hooks/...`. So hooks tests \
            run in both the `test-coverage` and `integration-hooks` CI jobs, doubling execution time and coupling.\n\n\
            **Issue 3 - Potential race with backgrounded curl:** The `disown`'d curl in `smee_post` is \
            detached from the process group, creating a timing race with the 10-second polling in `waitForEventBySession`.\n\n\
            { \"create_fix_tasks\": { \"tasks\": [{ \"title\": \"Fix hook integration test failures: CWD mismatch, CI overlap, and test reliability\", \
            \"description\": \"Problem: Hook integration tests fail.\\n\\nRequirements:\\n\\n\
            ### Fix 1: Fixture CWD must point to the temp git repo\\n\\n\
            In `hooks_test.go`, after reading the fixture JSON, overwrite the `cwd` field with `hookDir`.\\n\\n\
            **Fix:** Something like:\\n\
            ```go\\nfixture[\\\"cwd\\\"] = hookDir\\nfixtureData, _ = json.Marshal(fixture)\\n```\\n\\n\
            ### Fix 2: Exclude hooks tests from test-coverage target\\n\\n\
            **Fix:** Update the Makefile:\\n\
            ```makefile\\ntest-coverage:\\n\\tDB_HOST=localhost $(GO_CMD) test -v -tags=integration $(shell go list -tags=integration ./tests/integration/... | grep -v /hooks)\\n```\\n\\n\
            ### Fix 3: Improve test reliability\\n\\n\
            Ensure polling timeout is adequate.\\n\\n\
            Acceptance Criteria:\\n\
            - All 22 tests pass\\n\
            - make test-coverage does NOT run hooks tests\\n\
            - go vet passes\" }] } }";
        let block = parse_create_fix_tasks_from_response(text).unwrap();
        assert_eq!(block.tasks.len(), 1);
        assert_eq!(
            block.tasks[0].title,
            "Fix hook integration test failures: CWD mismatch, CI overlap, and test reliability"
        );
        assert!(block.tasks[0].description.contains("Fix 1"));
        assert!(block.tasks[0].description.contains("Fix 2"));
        assert!(block.tasks[0].description.contains("go vet"));
    }
}
