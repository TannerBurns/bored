use std::path::PathBuf;
use tauri::AppHandle;

use crate::agents::cursor;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorStatus {
    pub is_available: bool,
    pub version: Option<String>,
    pub global_hooks_installed: bool,
    pub hook_script_path: Option<String>,
}

#[tauri::command]
pub async fn get_cursor_status(app: AppHandle) -> Result<CursorStatus, String> {
    let hook_script_path = get_hook_script_path(&app);
    
    Ok(CursorStatus {
        is_available: cursor::is_cursor_available(),
        version: cursor::get_cursor_version(),
        global_hooks_installed: cursor::check_global_hooks_installed(),
        hook_script_path,
    })
}

const DEFAULT_API_URL: &str = "http://127.0.0.1:7432";

#[tauri::command]
pub async fn install_cursor_hooks_global(
    hook_script_path: String,
    api_url: Option<String>,
    api_token: Option<String>,
) -> Result<(), String> {
    let url = api_url.as_deref().unwrap_or(DEFAULT_API_URL);
    cursor::install_global_hooks(&hook_script_path, Some(url), api_token.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn install_cursor_hooks_project(
    hook_script_path: String,
    project_path: String,
    api_url: Option<String>,
    api_token: Option<String>,
) -> Result<(), String> {
    let url = api_url.as_deref().unwrap_or(DEFAULT_API_URL);
    cursor::install_hooks(&PathBuf::from(project_path), &hook_script_path, Some(url), api_token.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_cursor_hooks_config(
    hook_script_path: String,
) -> Result<String, String> {
    let config = cursor::generate_hooks_json(&hook_script_path);
    serde_json::to_string_pretty(&config)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn check_project_hooks_installed(
    project_path: String,
) -> Result<bool, String> {
    Ok(cursor::check_project_hooks_installed(&PathBuf::from(project_path)))
}

#[tauri::command]
pub async fn get_hook_script_path_cmd(app: AppHandle) -> Result<Option<String>, String> {
    Ok(get_hook_script_path(&app))
}

fn get_hook_script_path(app: &AppHandle) -> Option<String> {
    app.path_resolver()
        .app_data_dir()
        .map(|dir| dir.join("scripts").join("cursor-hook.js"))
        .map(|p| p.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_status_serializes_correctly() {
        let status = CursorStatus {
            is_available: true,
            version: Some("0.43.0".to_string()),
            global_hooks_installed: false,
            hook_script_path: Some("/path/to/hook.js".to_string()),
        };
        
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("isAvailable"));
        assert!(json.contains("globalHooksInstalled"));
        assert!(json.contains("hookScriptPath"));
    }

    #[test]
    fn cursor_status_serializes_with_none_values() {
        let status = CursorStatus {
            is_available: false,
            version: None,
            global_hooks_installed: false,
            hook_script_path: None,
        };
        
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"isAvailable\":false"));
        assert!(json.contains("\"version\":null"));
        assert!(json.contains("\"hookScriptPath\":null"));
    }

    #[test]
    fn cursor_status_deserializes_json_values() {
        let status = CursorStatus {
            is_available: true,
            version: Some("1.0.0".to_string()),
            global_hooks_installed: true,
            hook_script_path: Some("/usr/local/bin/hook.js".to_string()),
        };
        
        let json = serde_json::to_string(&status).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        
        assert_eq!(parsed["isAvailable"], true);
        assert_eq!(parsed["version"], "1.0.0");
        assert_eq!(parsed["globalHooksInstalled"], true);
        assert_eq!(parsed["hookScriptPath"], "/usr/local/bin/hook.js");
    }

    #[test]
    fn cursor_status_debug_impl() {
        let status = CursorStatus {
            is_available: true,
            version: Some("0.43.0".to_string()),
            global_hooks_installed: false,
            hook_script_path: None,
        };
        
        let debug = format!("{:?}", status);
        assert!(debug.contains("CursorStatus"));
        assert!(debug.contains("is_available: true"));
    }

    #[test]
    fn cursor_status_clone() {
        let status = CursorStatus {
            is_available: true,
            version: Some("0.43.0".to_string()),
            global_hooks_installed: true,
            hook_script_path: Some("/path".to_string()),
        };
        
        let cloned = status.clone();
        assert_eq!(cloned.is_available, status.is_available);
        assert_eq!(cloned.version, status.version);
        assert_eq!(cloned.global_hooks_installed, status.global_hooks_installed);
        assert_eq!(cloned.hook_script_path, status.hook_script_path);
    }
}
