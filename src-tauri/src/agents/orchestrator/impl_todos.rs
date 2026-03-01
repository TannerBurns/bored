//! Implementation todo management: decomposition, storage, status tracking, and progress events.

use super::config::{
    self, ImplementationProgress, ImplementationTodo, TodoItemStatus, TodoStatus,
};
use super::WorkflowOrchestrator;
use crate::agents::prompt::generate_plan_decomposition_prompt;

impl WorkflowOrchestrator {
    /// Decompose a plan into implementation todos via agent call.
    pub(super) async fn decompose_plan_into_todos(&self, plan: &str) {
        tracing::info!("Decomposing plan into implementation todos");

        let prompt = generate_plan_decomposition_prompt(plan);

        match self.run_stage("plan-decompose", &prompt).await {
            Ok(result) => {
                let raw_output = result.captured_stdout.unwrap_or_default();
                let text = self.extract_text(&raw_output);
                let todos = config::parse_implementation_todos(&text);

                if todos.len() <= 1 {
                    tracing::info!(
                        "Plan decomposition returned {} todo(s), skipping todo-based implementation",
                        todos.len()
                    );
                    return;
                }

                tracing::info!(
                    "Plan decomposed into {} todos: {:?}",
                    todos.len(),
                    todos.iter().map(|t| &t.title).collect::<Vec<_>>()
                );

                self.store_implementation_todos(&todos);
            }
            Err(e) => {
                tracing::warn!(
                    "Plan decomposition failed, falling back to single implement: {}",
                    e
                );
            }
        }
    }

    fn store_implementation_todos(&self, todos: &[ImplementationTodo]) {
        if let Ok(mut stored) = self.implementation_todos.write() {
            *stored = todos.to_vec();
        }

        let todo_statuses: Vec<TodoStatus> = todos
            .iter()
            .map(|t| TodoStatus {
                title: t.title.clone(),
                description: t.description.clone(),
                status: TodoItemStatus::Pending,
            })
            .collect();

        if let Err(e) = self.db.merge_run_metadata(
            &self.parent_run_id,
            &serde_json::json!({ "implementation_todos": todo_statuses }),
        ) {
            tracing::warn!("Failed to persist implementation todos: {}", e);
        }
    }

    pub(super) fn get_implementation_todos(&self) -> Vec<ImplementationTodo> {
        self.implementation_todos
            .read()
            .map(|todos| todos.clone())
            .unwrap_or_default()
    }

    /// Load implementation todos from run metadata (for resume scenarios).
    /// Checks the current parent run first, then falls back to the run it resumed from.
    pub(super) fn load_todos_from_metadata(&self) {
        let current_run = match self.db.get_run(&self.parent_run_id) {
            Ok(run) => run,
            Err(_) => return,
        };

        let todos = Self::extract_todos_from_metadata(&current_run.metadata).or_else(|| {
            let prev_id = current_run.resumed_from_run_id.as_ref()?;
            let prev_run = self.db.get_run(prev_id).ok()?;
            Self::extract_todos_from_metadata(&prev_run.metadata)
        });

        if let Some(todos) = todos {
            tracing::info!(
                "Loaded {} implementation todos from run metadata",
                todos.len()
            );
            if let Ok(mut stored) = self.implementation_todos.write() {
                *stored = todos;
            }
        }
    }

    fn extract_todos_from_metadata(
        metadata: &Option<serde_json::Value>,
    ) -> Option<Vec<ImplementationTodo>> {
        let meta = metadata.as_ref()?;
        let raw_todos = meta.get("implementation_todos")?;
        let todo_statuses =
            serde_json::from_value::<Vec<TodoStatus>>(raw_todos.clone()).ok()?;
        let todos: Vec<ImplementationTodo> = todo_statuses
            .iter()
            .map(|ts| ImplementationTodo {
                title: ts.title.clone(),
                description: ts.description.clone(),
            })
            .collect();
        if todos.is_empty() {
            None
        } else {
            Some(todos)
        }
    }

    pub(super) fn mark_todo_status(&self, index: usize, status: TodoItemStatus) {
        let mut todo_statuses = match self.load_todo_statuses() {
            Some(s) => s,
            None => return,
        };

        if let Some(todo) = todo_statuses.get_mut(index) {
            todo.status = status;
        }

        if let Err(e) = self.db.merge_run_metadata(
            &self.parent_run_id,
            &serde_json::json!({ "implementation_todos": todo_statuses }),
        ) {
            tracing::warn!("Failed to update todo status: {}", e);
        }
    }

    pub(super) fn emit_implementation_progress(
        &self,
        completed: usize,
        total: usize,
        current_title: &str,
    ) {
        let todos = self.load_todo_statuses().unwrap_or_default();

        let progress = ImplementationProgress {
            completed,
            total,
            current_todo_title: current_title.to_string(),
            todos,
        };

        self.emit_stage_event_with_progress("implement", "running", None, None, Some(progress));
    }

    pub(super) fn load_todo_statuses_vec(&self) -> Vec<TodoItemStatus> {
        self.load_todo_statuses()
            .map(|statuses| statuses.into_iter().map(|s| s.status).collect())
            .unwrap_or_default()
    }

    fn load_todo_statuses(&self) -> Option<Vec<TodoStatus>> {
        let run = self.db.get_run(&self.parent_run_id).ok()?;
        let meta = run.metadata?;
        let raw = meta.get("implementation_todos")?;
        serde_json::from_value::<Vec<TodoStatus>>(raw.clone()).ok()
    }
}
