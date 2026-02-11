#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

use agent_kanban::commands::claude::ClaudeApiSettingsState;
use agent_kanban::commands::runs::RunningAgents;
use agent_kanban::commands::ApiConnState;
use agent_kanban::{api, commands, db, logging};

/// Check if a URL is allowed for navigation within the app
fn is_allowed_url(url: &url::Url) -> bool {
    // Allow the dev server
    if url.host_str() == Some("localhost") || url.host_str() == Some("127.0.0.1") {
        if let Some(port) = url.port() {
            // Allow Vite dev server port
            if port == 1420 {
                return true;
            }
        }
    }

    // Allow tauri custom protocol (production builds)
    if url.scheme() == "tauri" {
        return true;
    }

    // Allow about:blank and similar
    if url.scheme() == "about" {
        return true;
    }

    false
}

fn setup_hook_scripts(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;

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

    if let Ok(resource_path) = app
        .path()
        .resolve(&resource_name, tauri::path::BaseDirectory::Resource)
    {
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
        tracing::warn!(
            "Could not resolve hook script resource path for {}",
            script_name
        );
    }

    Ok(())
}

fn main() {
    // Fix PATH for bundled apps on macOS/Linux
    // When launched from Finder, apps get a minimal PATH that doesn't include
    // directories where CLI tools like `cursor` and `claude` are installed
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fix_path_env::fix().ok();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");

            if let Err(e) = logging::init_logging(app_data_dir.clone()) {
                eprintln!("Failed to initialize logging: {}", e);
            }

            tracing::info!("Agent Kanban starting up...");
            tracing::info!("App data directory: {:?}", app_data_dir);

            // Create window first to show loading screen while initialization continues
            let window_url = if cfg!(debug_assertions) {
                WebviewUrl::External("http://localhost:1420".parse().unwrap())
            } else {
                WebviewUrl::App("index.html".into())
            };

            let _main_window = WebviewWindowBuilder::new(app, "main", window_url)
                .title("Bored")
                .inner_size(1200.0, 800.0)
                .resizable(true)
                .on_navigation(|url| {
                    let allowed = is_allowed_url(url);
                    if !allowed {
                        tracing::warn!(
                            "Blocked navigation to external URL: {} - use system browser instead",
                            url
                        );
                    }
                    allowed
                })
                .build()
                .expect("Failed to create main window");

            tracing::info!("Main window created, continuing initialization...");

            if let Err(e) = setup_hook_scripts(app) {
                tracing::warn!("Failed to setup hook scripts: {}", e);
            }

            let db_path = app_data_dir.join("agent-kanban.db");
            let database = match db::Database::open(db_path.clone()) {
                Ok(db) => Arc::new(db),
                Err(e) => {
                    tracing::error!("Failed to open database at {:?}: {}", db_path, e);
                    // Provide detailed error message for common issues
                    let error_msg = match &e {
                        db::DbError::Migration(msg) => format!(
                            "Database migration failed. {}\n\nThe database file is at: {:?}\n\nYou may need to restore from a backup or delete the database to start fresh.",
                            msg, db_path
                        ),
                        db::DbError::Sqlite(sqlite_err) => format!(
                            "SQLite error: {}\n\nDatabase file: {:?}\n\nThis could indicate database corruption. Check file permissions or disk space.",
                            sqlite_err, db_path
                        ),
                        _ => format!("Database error: {}", e),
                    };
                    panic!("{}", error_msg);
                }
            };

            let db_for_cleanup = database.clone();
            tauri::async_runtime::spawn(async move {
                match db_for_cleanup.cleanup_orphaned_in_progress_tasks() {
                    Ok(count) if count > 0 => {
                        tracing::info!(
                            "Startup cleanup: reset {} orphaned in-progress task(s)",
                            count
                        );
                    }
                    Err(e) => {
                        tracing::warn!("Startup cleanup failed: {}", e);
                    }
                    _ => {}
                }
            });

            app.manage(database.clone());
            app.manage(RunningAgents::new());

            // Load Claude API settings from disk (or create fresh if not present)
            let claude_settings_path = app_data_dir.join("claude_api_settings.json");
            app.manage(ClaudeApiSettingsState::new_with_path(claude_settings_path));

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
                        tracing::info!(
                            "Generating new API token (existing file was empty or unreadable)"
                        );
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
                token: api_token.clone(),
                ..Default::default()
            };

            let port_path = app_data_dir.join("api_port");
            std::fs::write(&port_path, api_config.port.to_string())
                .expect("Failed to write API port");

            // Create shared API URL and token for Tauri commands
            let api_url = format!("http://127.0.0.1:{}", api_config.port);

            // Make config available via environment for child processes
            std::env::set_var("AGENT_KANBAN_API_TOKEN", &api_config.token);
            std::env::set_var("AGENT_KANBAN_API_PORT", api_config.port.to_string());
            std::env::set_var("AGENT_KANBAN_API_URL", &api_url);

            // Create shared event channel for SSE broadcasting
            let event_tx = api::create_event_channel();

            // Manage shared state for commands that need API/event access
            app.manage(event_tx.clone());
            app.manage(ApiConnState { url: api_url, token: api_token });

            // Start API server with shared event channel
            let db_for_api = database.clone();
            let event_tx_for_api = event_tx;
            let api_config_clone = api_config.clone();
            tauri::async_runtime::spawn(async move {
                match api::start_server_with_event_tx(
                    db_for_api,
                    api_config_clone,
                    event_tx_for_api,
                )
                .await
                {
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
            commands::factory_reset,
            commands::repair_specs_table,
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
            commands::runs::get_recent_runs_with_context,
            commands::runs::get_agent_run,
            commands::runs::cancel_agent_run,
            commands::runs::cleanup_stale_runs,
            commands::runs::get_run_events,
            commands::runs::get_run_cost,
            commands::runs::get_ticket_cost,
            commands::runs::get_board_cost_summary,
            commands::runs::backfill_run_costs,
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
            commands::get_claude_api_settings,
            commands::set_claude_api_settings,
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
            // Spec / Planner commands
            commands::specs::create_spec,
            commands::specs::get_specs,
            commands::specs::get_all_specs,
            commands::specs::get_spec,
            commands::specs::update_spec,
            commands::specs::delete_spec,
            commands::specs::delete_spec_with_tickets,
            commands::specs::set_spec_status,
            commands::specs::append_spec_exploration,
            commands::specs::set_spec_plan,
            commands::specs::approve_plan,
            commands::specs::get_spec_tickets,
            commands::specs::start_planner,
            commands::specs::execute_plan,
            commands::specs::start_spec_work,
            commands::specs::get_spec_progress,
            commands::specs::pause_spec_work,
            commands::specs::resume_spec_work,
            commands::specs::halt_spec_work,
            commands::specs::reset_plan_execution,
            commands::specs::get_spec_eta,
            commands::specs::get_spec_cost,
            commands::specs::get_spec_version_cost,
            commands::specs::get_version_progress,
            // Spec version commands
            commands::specs::get_spec_versions,
            commands::specs::get_latest_spec_version,
            commands::specs::get_spec_with_version,
            commands::specs::get_specs_with_versions,
            commands::specs::get_all_specs_with_versions,
            commands::specs::create_new_spec_version,
            // Conversation (brainstorming) commands
            commands::conversations::get_conversation_messages,
            commands::conversations::send_conversation_message,
            commands::conversations::start_conversation,
            // Ticket pause/resume commands
            commands::tickets::pause_ticket,
            commands::tickets::resume_ticket,
            commands::tickets::is_ticket_paused,
            commands::tickets::get_paused_tickets,
            // Release notes
            commands::release_notes::get_release_notes,
            commands::release_notes::get_all_release_notes,
            // Validation commands
            commands::validation::create_validation_session,
            commands::validation::get_validation_session,
            commands::validation::get_validation_sessions,
            commands::validation::update_validation_session_status,
            commands::validation::delete_validation_session,
            commands::validation::get_validation_messages,
            commands::validation::send_validation_message,
            commands::validation::create_fix_tasks,
            // Next steps commands (push, PR, diff, open)
            commands::next_steps::push_branch,
            commands::next_steps::create_pull_request,
            commands::next_steps::get_branch_diff,
            commands::next_steps::open_in_editor,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_allowed_url_allows_localhost_dev_server() {
        let url = url::Url::parse("http://localhost:1420").unwrap();
        assert!(is_allowed_url(&url));
    }

    #[test]
    fn is_allowed_url_allows_127_0_0_1_dev_server() {
        let url = url::Url::parse("http://127.0.0.1:1420").unwrap();
        assert!(is_allowed_url(&url));
    }

    #[test]
    fn is_allowed_url_blocks_localhost_wrong_port() {
        let url = url::Url::parse("http://localhost:3000").unwrap();
        assert!(!is_allowed_url(&url));
    }

    #[test]
    fn is_allowed_url_blocks_localhost_no_port() {
        let url = url::Url::parse("http://localhost/").unwrap();
        assert!(!is_allowed_url(&url));
    }

    #[test]
    fn is_allowed_url_allows_tauri_scheme() {
        let url = url::Url::parse("tauri://localhost/index.html").unwrap();
        assert!(is_allowed_url(&url));
    }

    #[test]
    fn is_allowed_url_allows_about_scheme() {
        let url = url::Url::parse("about:blank").unwrap();
        assert!(is_allowed_url(&url));
    }

    #[test]
    fn is_allowed_url_blocks_external_https() {
        let url = url::Url::parse("https://example.com").unwrap();
        assert!(!is_allowed_url(&url));
    }

    #[test]
    fn is_allowed_url_blocks_external_http() {
        let url = url::Url::parse("http://malicious.com/steal-data").unwrap();
        assert!(!is_allowed_url(&url));
    }

    #[test]
    fn is_allowed_url_blocks_external_with_path() {
        let url = url::Url::parse("https://github.com/user/repo").unwrap();
        assert!(!is_allowed_url(&url));
    }

    #[test]
    fn is_allowed_url_blocks_javascript_scheme() {
        let url = url::Url::parse("javascript:alert(1)").unwrap();
        assert!(!is_allowed_url(&url));
    }
}
