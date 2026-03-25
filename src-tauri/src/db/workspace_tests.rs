use crate::db::models::CreateProject;
use crate::db::Database;

fn create_test_db() -> Database {
    Database::open_in_memory().unwrap()
}

fn temp_dir_path() -> String {
    std::env::temp_dir().to_string_lossy().to_string()
}

#[test]
fn create_workspace_and_get() {
    let db = create_test_db();
    let ws = db.create_workspace("My Workspace").unwrap();
    assert_eq!(ws.name, "My Workspace");
    assert!(ws.project_ids.is_empty());

    let fetched = db.get_workspace(&ws.id).unwrap().expect("workspace should exist");
    assert_eq!(fetched.id, ws.id);
    assert_eq!(fetched.name, "My Workspace");
    assert!(fetched.project_ids.is_empty());
}

#[test]
fn get_workspace_returns_none_for_missing() {
    let db = create_test_db();
    let result = db.get_workspace("nonexistent").unwrap();
    assert!(result.is_none());
}

#[test]
fn get_workspaces_lists_all() {
    let db = create_test_db();
    db.create_workspace("WS A").unwrap();
    db.create_workspace("WS B").unwrap();

    let all = db.get_workspaces().unwrap();
    assert_eq!(all.len(), 2);
}

#[test]
fn update_workspace_renames() {
    let db = create_test_db();
    let ws = db.create_workspace("Old Name").unwrap();
    let updated = db.update_workspace(&ws.id, "New Name").unwrap();
    assert_eq!(updated.name, "New Name");

    let fetched = db.get_workspace(&ws.id).unwrap().unwrap();
    assert_eq!(fetched.name, "New Name");
}

#[test]
fn update_workspace_not_found() {
    let db = create_test_db();
    let result = db.update_workspace("nonexistent", "Name");
    assert!(result.is_err());
}

#[test]
fn delete_workspace_success() {
    let db = create_test_db();
    let ws = db.create_workspace("To Delete").unwrap();
    db.delete_workspace(&ws.id).unwrap();
    assert!(db.get_workspace(&ws.id).unwrap().is_none());
}

#[test]
fn delete_workspace_not_found() {
    let db = create_test_db();
    let result = db.delete_workspace("nonexistent");
    assert!(result.is_err());
}

#[test]
fn add_project_to_workspace_and_get() {
    let db = create_test_db();
    let ws = db.create_workspace("WS").unwrap();
    let dir_a = std::env::temp_dir().join(format!("ws_test_a_{}", uuid::Uuid::new_v4()));
    let dir_b = std::env::temp_dir().join(format!("ws_test_b_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir_a).unwrap();
    std::fs::create_dir_all(&dir_b).unwrap();

    let p1 = db
        .create_project(&CreateProject {
            name: "Project A".to_string(),
            path: dir_a.to_string_lossy().to_string(),
            requires_git: true,
        })
        .unwrap();
    let p2 = db
        .create_project(&CreateProject {
            name: "Project B".to_string(),
            path: dir_b.to_string_lossy().to_string(),
            requires_git: false,
        })
        .unwrap();

    db.add_project_to_workspace(&ws.id, &p2.id, 1).unwrap();
    db.add_project_to_workspace(&ws.id, &p1.id, 0).unwrap();

    let fetched = db.get_workspace(&ws.id).unwrap().unwrap();
    assert_eq!(fetched.project_ids.len(), 2);
    assert_eq!(fetched.project_ids[0], p1.id, "position 0 should be p1");
    assert_eq!(fetched.project_ids[1], p2.id, "position 1 should be p2");
}

#[test]
fn get_workspace_projects_returns_full_project_data() {
    let db = create_test_db();
    let ws = db.create_workspace("WS").unwrap();
    let proj = db
        .create_project(&CreateProject {
            name: "Full Project".to_string(),
            path: temp_dir_path(),
            requires_git: true,
        })
        .unwrap();

    db.add_project_to_workspace(&ws.id, &proj.id, 0).unwrap();

    let projects = db.get_workspace_projects(&ws.id).unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].id, proj.id);
    assert_eq!(projects[0].name, "Full Project");
    assert!(projects[0].requires_git);
}

#[test]
fn get_workspace_projects_empty_for_wrong_id() {
    let db = create_test_db();
    let projects = db.get_workspace_projects("nonexistent").unwrap();
    assert!(projects.is_empty());
}

#[test]
fn remove_project_from_workspace() {
    let db = create_test_db();
    let ws = db.create_workspace("WS").unwrap();
    let proj = db
        .create_project(&CreateProject {
            name: "Proj".to_string(),
            path: temp_dir_path(),
            requires_git: true,
        })
        .unwrap();

    db.add_project_to_workspace(&ws.id, &proj.id, 0).unwrap();
    assert_eq!(db.get_workspace_projects(&ws.id).unwrap().len(), 1);

    db.remove_project_from_workspace(&ws.id, &proj.id).unwrap();
    assert!(db.get_workspace_projects(&ws.id).unwrap().is_empty());
}

#[test]
fn add_project_to_workspace_replace_semantics() {
    let db = create_test_db();
    let ws = db.create_workspace("WS").unwrap();
    let proj = db
        .create_project(&CreateProject {
            name: "Proj".to_string(),
            path: temp_dir_path(),
            requires_git: true,
        })
        .unwrap();

    db.add_project_to_workspace(&ws.id, &proj.id, 0).unwrap();
    db.add_project_to_workspace(&ws.id, &proj.id, 5).unwrap();

    let fetched = db.get_workspace(&ws.id).unwrap().unwrap();
    assert_eq!(fetched.project_ids.len(), 1, "should not duplicate");
}
