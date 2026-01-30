#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use tauri::Manager;

use agent_kanban::{api, commands, db, logging};
use agent_kanban::commands::runs::RunningAgents;

fn setup_hook_scripts(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_data_dir = app
        .path_resolver()
        .app_data_dir()
        .ok_or("Failed to get app data directory")?;
    
    let scripts_dir = app_data_dir.join("scripts");
    std::fs::create_dir_all(&scripts_dir)?;

    // Copy Cursor hook script
    copy_hook_script(app, "cursor-hook.js", &scripts_dir)?;
    
    // Copy Claude hook script
    copy_hook_script(app, "claude-hook.js", &scripts_dir)?;
    
    // Copy unified hook script (hook bridge)
    copy_hook_script(app, "agent-kanban-hook.js", &scripts_dir)?;

    Ok(())
}

fn copy_hook_script(
    app: &tauri::App,
    script_name: &str,
    scripts_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let resource_name = format!("scripts/{}", script_name);
    
    if let Some(resource_path) = app.path_resolver().resolve_resource(&resource_name) {
        let target_path = scripts_dir.join(script_name);
        
        if resource_path.exists() {
            let should_copy = if target_path.exists() {
                let resource_modified = std::fs::metadata(&resource_path)?.modified()?;
                let target_modified = std::fs::metadata(&target_path)?.modified()?;
                resource_modified > target_modified
            } else {
                true
            };

            if should_copy {
                std::fs::copy(&resource_path, &target_path)?;
                tracing::info!("Copied {} to {:?}", script_name, target_path);
                
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = std::fs::metadata(&target_path)?.permissions();
                    perms.set_mode(0o755);
                    std::fs::set_permissions(&target_path, perms)?;
                }
            }
        } else {
            tracing::warn!("Hook script resource not found at {:?}", resource_path);
        }
    } else {
        tracing::warn!("Could not resolve hook script resource path for {}", script_name);
    }

    Ok(())
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app
                .path_resolver()
                .app_data_dir()
                .expect("Failed to get app data directory");

            if let Err(e) = logging::init_logging(app_data_dir.clone()) {
                eprintln!("Failed to initialize logging: {}", e);
            }

            tracing::info!("Agent Kanban starting up...");
            tracing::info!("App data directory: {:?}", app_data_dir);

            if let Err(e) = setup_hook_scripts(app) {
                tracing::warn!("Failed to setup hook scripts: {}", e);
            }

            let db_path = app_data_dir.join("agent-kanban.db");
            let database = Arc::new(db::Database::open(db_path).expect("Failed to open database"));

            // Cleanup orphaned tasks from interrupted runs
            // This handles cases where the app crashed or was killed while a run was in progress
            match database.cleanup_orphaned_in_progress_tasks() {
                Ok(count) if count > 0 => {
                    tracing::info!("Startup cleanup: reset {} orphaned in-progress task(s)", count);
                }
                Err(e) => {
                    tracing::warn!("Startup cleanup failed: {}", e);
                }
                _ => {}
            }

            app.manage(database.clone());
            app.manage(RunningAgents::new());

            // Configure API server with persistent token
            // Try to read existing token from file, or generate a new one
            let token_path = app_data_dir.join("api_token");
            let api_token = if token_path.exists() {
                match std::fs::read_to_string(&token_path) {
                    Ok(token) if !token.trim().is_empty() => {
                        tracing::info!("Using existing API token from {}", token_path.display());
                        token.trim().to_string()
                    }
                    _ => {
                        tracing::info!("Generating new API token (existing file was empty or unreadable)");
                        let token = api::generate_token();
                        std::fs::write(&token_path, &token).expect("Failed to write API token");
                        token
                    }
                }
            } else {
                tracing::info!("Generating new API token (no existing token found)");
                let token = api::generate_token();
                std::fs::write(&token_path, &token).expect("Failed to write API token");
                token
            };
            
            let api_config = api::ApiConfig {
                token: api_token,
                ..Default::default()
            };
            
            let port_path = app_data_dir.join("api_port");
            std::fs::write(&port_path, api_config.port.to_string())
                .expect("Failed to write API port");

            // Make config available via environment for child processes
            std::env::set_var("AGENT_KANBAN_API_TOKEN", &api_config.token);
            std::env::set_var("AGENT_KANBAN_API_PORT", api_config.port.to_string());
            std::env::set_var("AGENT_KANBAN_API_URL", format!("http://127.0.0.1:{}", api_config.port));

            // Start API server
            let db_for_api = database.clone();
            tauri::async_runtime::spawn(async move {
                match api::start_server(db_for_api, api_config).await {
                    Ok(handle) => {
                        tracing::info!("API server started at {}", handle.addr);
                        // Keep handle alive - server runs until app exits
                        std::mem::forget(handle);
                    }
                    Err(e) => {
                        tracing::error!("Failed to start API server: {}", e);
                    }
                }
            });

            // Start spool processor for handling offline events
            let db_for_spool = database.clone();
            let spool_dir = api::get_default_spool_dir();
            tauri::async_runtime::spawn(async move {
                api::start_spool_processor(db_for_spool, spool_dir).await;
            });

            tracing::info!("Agent Kanban initialized successfully");

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_boards,
            commands::get_columns,
            commands::create_board,
            commands::update_board,
            commands::delete_board,
            commands::get_tickets,
            commands::create_ticket,
            commands::move_ticket,
            commands::update_ticket,
            commands::delete_ticket,
            commands::get_comments,
            commands::add_comment,
            commands::update_comment,
            // Epic management
            commands::get_epic_children,
            commands::get_epic_progress,
            commands::add_ticket_to_epic,
            commands::remove_ticket_from_epic,
            commands::reorder_epic_children,
            commands::runs::start_agent_run,
            commands::runs::get_agent_runs,
            commands::runs::get_recent_runs,
            commands::runs::get_agent_run,
            commands::runs::cancel_agent_run,
            commands::runs::cleanup_stale_runs,
            commands::runs::get_run_events,
            commands::get_projects,
            commands::get_project,
            commands::create_project,
            commands::update_project,
            commands::delete_project,
            commands::set_board_project,
            commands::set_ticket_project,
            commands::check_ticket_readiness,
            commands::update_project_hooks,
            commands::browse_for_directory,
            commands::check_git_status,
            commands::init_git_repo,
            commands::create_project_folder,
            // Cursor integration
            commands::get_cursor_status,
            commands::install_cursor_hooks_global,
            commands::install_cursor_hooks_project,
            commands::get_cursor_hooks_config,
            commands::check_project_hooks_installed,
            commands::get_hook_script_path_cmd,
            // Claude Code integration
            commands::get_claude_status,
            commands::install_claude_hooks_user,
            commands::install_claude_hooks_project,
            commands::install_claude_hooks_local,
            commands::get_claude_hooks_config,
            commands::check_claude_available,
            commands::check_claude_project_hooks_installed,
            commands::get_claude_hook_script_path,
            // Worker management
            commands::workers::start_worker,
            commands::workers::stop_worker,
            commands::workers::stop_all_workers,
            commands::workers::get_workers,
            commands::workers::get_worker_queue_status,
            // Worker validation and commands
            commands::workers::validate_worker,
            commands::workers::get_commands_path,
            commands::workers::get_available_commands,
            commands::workers::install_commands_to_project,
            commands::workers::install_commands_to_user,
            commands::workers::check_commands_installed,
            commands::workers::check_user_commands_installed,
            // API configuration
            commands::get_api_config,
            // Task queue management
            commands::tasks::get_tasks,
            commands::tasks::get_task,
            commands::tasks::create_task,
            commands::tasks::add_preset_task,
            commands::tasks::delete_task,
            commands::tasks::get_next_pending_task,
            commands::tasks::has_pending_tasks,
            commands::tasks::get_task_counts,
            commands::tasks::update_task,
            commands::tasks::get_preset_types,
            commands::tasks::reset_task,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
