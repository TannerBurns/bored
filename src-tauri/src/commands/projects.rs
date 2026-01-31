use std::path::Path;
use std::sync::Arc;
use tauri::State;

use crate::agents::worktree::{is_git_repo, repo_has_commits, create_initial_commit};
use crate::db::{CreateProject, Database, Project, ReadinessCheck, UpdateProject};

#[tauri::command]
pub async fn get_projects(db: State<'_, Arc<Database>>) -> Result<Vec<Project>, String> {
    db.get_projects().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_project(
    project_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Option<Project>, String> {
    db.get_project(&project_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_project(
    input: CreateProject,
    db: State<'_, Arc<Database>>,
) -> Result<Project, String> {
    db.create_project(&input).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_project(
    project_id: String,
    input: UpdateProject,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    db.update_project(&project_id, &input)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_project(
    project_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    db.delete_project(&project_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_board_project(
    board_id: String,
    project_id: Option<String>,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    db.set_board_project(&board_id, project_id.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_ticket_project(
    ticket_id: String,
    project_id: Option<String>,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    db.set_ticket_project(&ticket_id, project_id.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn check_ticket_readiness(
    ticket_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<ReadinessCheck, String> {
    db.can_move_to_ready(&ticket_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_project_hooks(
    project_id: String,
    cursor_installed: Option<bool>,
    claude_installed: Option<bool>,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    db.update_project_hooks(&project_id, cursor_installed, claude_installed)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn browse_for_directory() -> Result<Option<String>, String> {
    use tauri::api::dialog::blocking::FileDialogBuilder;

    let path = FileDialogBuilder::new()
        .set_title("Select Project Directory")
        .pick_folder();

    Ok(path.map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn check_git_status(path: String) -> Result<bool, String> {
    Ok(is_git_repo(Path::new(&path)))
}

#[tauri::command]
pub async fn init_git_repo(path: String) -> Result<(), String> {
    use std::process::Command;

    let repo_path = Path::new(&path);

    let output = Command::new("git")
        .args(["init"])
        .current_dir(&path)
        .output()
        .map_err(|e| format!("Failed to execute git init: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to initialize git repository: {}", stderr.trim()));
    }

    // Create an initial commit so the repository is ready for worktree operations.
    // This prevents the "unborn branch" error when agents try to create worktrees.
    if !repo_has_commits(repo_path) {
        create_initial_commit(repo_path)
            .map_err(|e| format!("Git repository initialized but failed to create initial commit: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
pub async fn create_project_folder(parent_path: String, name: String) -> Result<String, String> {
    let parent = Path::new(&parent_path);

    if !parent.exists() {
        return Err(format!("Parent directory does not exist: {}", parent_path));
    }

    if !parent.is_dir() {
        return Err(format!("Parent path is not a directory: {}", parent_path));
    }

    if name.is_empty() {
        return Err("Project name cannot be empty".to_string());
    }

    if name.contains('/') || name.contains('\\') {
        return Err("Project name cannot contain path separators".to_string());
    }

    let full_path = parent.join(&name);

    if full_path.exists() {
        return Err(format!("Directory already exists: {}", full_path.display()));
    }

    std::fs::create_dir(&full_path)
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    Ok(full_path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_readiness_check_serialization() {
        let ready = ReadinessCheck::Ready {
            project_id: "proj-1".to_string(),
        };
        let json = serde_json::to_string(&ready).unwrap();
        assert!(json.contains("ready"));
        assert!(json.contains("proj-1"));

        let no_project = ReadinessCheck::NoProject(None);
        let json = serde_json::to_string(&no_project).unwrap();
        assert_eq!(json, r#"{"noProject":null}"#);
    }

    #[tokio::test]
    async fn create_project_folder_parent_not_exists() {
        let result = create_project_folder(
            "/nonexistent/path/that/does/not/exist".to_string(),
            "my-project".to_string(),
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    #[tokio::test]
    async fn create_project_folder_parent_is_file() {
        let temp_dir = std::env::temp_dir().join(format!("test_file_{}", uuid::Uuid::new_v4()));
        std::fs::write(&temp_dir, "test").unwrap();

        let result = create_project_folder(
            temp_dir.to_string_lossy().to_string(),
            "my-project".to_string(),
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not a directory"));

        std::fs::remove_file(&temp_dir).ok();
    }

    #[tokio::test]
    async fn create_project_folder_empty_name() {
        let temp_dir = std::env::temp_dir();

        let result = create_project_folder(
            temp_dir.to_string_lossy().to_string(),
            "".to_string(),
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot be empty"));
    }

    #[tokio::test]
    async fn create_project_folder_name_with_forward_slash() {
        let temp_dir = std::env::temp_dir();

        let result = create_project_folder(
            temp_dir.to_string_lossy().to_string(),
            "my/project".to_string(),
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("path separators"));
    }

    #[tokio::test]
    async fn create_project_folder_name_with_backslash() {
        let temp_dir = std::env::temp_dir();

        let result = create_project_folder(
            temp_dir.to_string_lossy().to_string(),
            "my\\project".to_string(),
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("path separators"));
    }

    #[tokio::test]
    async fn create_project_folder_already_exists() {
        let temp_dir = std::env::temp_dir();
        let existing_name = format!("existing_dir_{}", uuid::Uuid::new_v4());
        let existing_path = temp_dir.join(&existing_name);
        std::fs::create_dir(&existing_path).unwrap();

        let result = create_project_folder(
            temp_dir.to_string_lossy().to_string(),
            existing_name.clone(),
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already exists"));

        std::fs::remove_dir(&existing_path).ok();
    }

    #[tokio::test]
    async fn create_project_folder_success() {
        let temp_dir = std::env::temp_dir();
        let project_name = format!("new_project_{}", uuid::Uuid::new_v4());

        let result = create_project_folder(
            temp_dir.to_string_lossy().to_string(),
            project_name.clone(),
        )
        .await;

        assert!(result.is_ok());
        let created_path = result.unwrap();
        assert!(created_path.contains(&project_name));
        assert!(Path::new(&created_path).exists());
        assert!(Path::new(&created_path).is_dir());

        std::fs::remove_dir(&created_path).ok();
    }

    #[tokio::test]
    async fn check_git_status_on_non_git_dir() {
        let temp_dir = std::env::temp_dir().join(format!("non_git_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&temp_dir).unwrap();

        let result = check_git_status(temp_dir.to_string_lossy().to_string()).await;

        assert!(result.is_ok());
        assert!(!result.unwrap());

        std::fs::remove_dir(&temp_dir).ok();
    }

    #[tokio::test]
    async fn check_git_status_on_git_repo() {
        let temp_dir = std::env::temp_dir().join(format!("git_repo_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&temp_dir).unwrap();

        std::process::Command::new("git")
            .args(["init"])
            .current_dir(&temp_dir)
            .output()
            .unwrap();

        let result = check_git_status(temp_dir.to_string_lossy().to_string()).await;

        assert!(result.is_ok());
        assert!(result.unwrap());

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn init_git_repo_success() {
        let temp_dir = std::env::temp_dir().join(format!("init_git_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&temp_dir).unwrap();

        let result = init_git_repo(temp_dir.to_string_lossy().to_string()).await;

        assert!(result.is_ok());
        assert!(temp_dir.join(".git").exists());
        
        // Verify that an initial commit was also created (prevents unborn branch issues)
        assert!(repo_has_commits(&temp_dir), "init_git_repo should create an initial commit");

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn init_git_repo_nonexistent_path() {
        let result = init_git_repo("/nonexistent/path/for/git/init".to_string()).await;

        assert!(result.is_err());
    }
}
