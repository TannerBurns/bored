//! Pure parsing helpers for extracting structured blocks from validation agent responses.

use crate::agents::json_extraction::parse_all_json_blocks;
use crate::db::models::FixTask;

/// Parsed start_app block from agent response
pub(super) struct StartAppBlock {
    pub command: String,
    pub port: Option<i32>,
}

/// Parsed run_command block from agent response
pub(super) struct RunCommandBlock {
    pub command: String,
}

/// Parsed create_fix_tasks block from agent response
pub(super) struct CreateFixTasksBlock {
    pub tasks: Vec<FixTask>,
}

pub(super) fn parse_start_app_from_response(response_text: &str) -> Option<StartAppBlock> {
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

pub(super) fn parse_run_command_from_response(response_text: &str) -> Option<RunCommandBlock> {
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

pub(super) fn parse_create_fix_tasks_from_response(
    response_text: &str,
) -> Option<CreateFixTasksBlock> {
    for v in parse_all_json_blocks(response_text) {
        if let Some(task_obj) = v.get("create_fix_task").and_then(|s| s.as_object()) {
            return Some(CreateFixTasksBlock {
                tasks: vec![parse_fix_task_from_json_obj(task_obj)],
            });
        }
        if let Some(cft) = v.get("create_fix_tasks").and_then(|s| s.as_object()) {
            if let Some(tasks_arr) = cft.get("tasks").and_then(|t| t.as_array()) {
                let tasks: Vec<FixTask> = tasks_arr
                    .iter()
                    .filter_map(|tv| tv.as_object().map(parse_fix_task_from_json_obj))
                    .collect();
                if !tasks.is_empty() {
                    return Some(CreateFixTasksBlock { tasks });
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_start_app_from_response ---

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

    // --- parse_run_command_from_response ---

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

    // --- parse_fix_task_from_json_obj ---

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

    // --- parse_create_fix_tasks_from_response ---

    #[test]
    fn fix_tasks_singular_form() {
        let text = r#"```json
{ "create_fix_task": { "title": "Fix bug", "description": "It crashes" } }
```"#;
        let block = parse_create_fix_tasks_from_response(text).unwrap();
        assert_eq!(block.tasks.len(), 1);
        assert_eq!(block.tasks[0].title, "Fix bug");
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
    fn fix_tasks_from_bare_json() {
        let text = r#"I found a bug.
{ "create_fix_task": { "title": "Bare fix", "description": "Found inline" } }
Please fix it."#;
        let block = parse_create_fix_tasks_from_response(text).unwrap();
        assert_eq!(block.tasks.len(), 1);
        assert_eq!(block.tasks[0].title, "Bare fix");
    }
}
