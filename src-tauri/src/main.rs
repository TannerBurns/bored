#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::Path;
use std::sync::Arc;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

use bored::agents::claude::provider::ClaudeProvider;
use bored::agents::codex::provider::CodexProvider;
use bored::agents::cursor::provider::CursorProvider;
use bored::agents::registry::AgentRegistry;
use bored::agents::validation_agent::AppProcessManager;
use bored::commands::AgentSettingsManager;
use bored::commands::runs::RunningAgents;
use bored::commands::workflow_settings::WorkflowSettingsState;
use bored::commands::ApiConnState;
use bored::{api, commands, db, logging, tray};

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

/// Migrate data from the old `com.agent-kanban.app` data directory to the new one.
///
/// Tauri derives app_data_dir from the `identifier` in tauri.conf.json.
/// When we renamed the identifier from `com.agent-kanban.app` to `com.bored.app`,
/// the app started looking in a new (empty) directory. This function copies the
/// old database and config files so existing users don't lose their data.
fn migrate_from_old_app_dir(new_data_dir: &Path) {
    let old_data_dir = new_data_dir
        .parent()
        .expect("app data dir must have a parent")
        .join("com.agent-kanban.app");

    let old_db = old_data_dir.join("agent-kanban.db");
    let new_db = new_data_dir.join("bored.db");

    if !old_db.exists() {
        return;
    }

    // Only migrate if the new database is missing or tiny (fresh schema, no real data).
    let new_db_is_fresh = match std::fs::metadata(&new_db) {
        Ok(m) => m.len() < 100_000, // a fresh schema-only DB is well under 100KB
        Err(_) => true,
    };
    if !new_db_is_fresh {
        return;
    }

    eprintln!(
        "Migrating data from old app directory {:?} -> {:?}",
        old_data_dir, new_data_dir
    );

    // Copy the database files (main, WAL, SHM)
    let db_files = [
        ("agent-kanban.db", "bored.db"),
        ("agent-kanban.db-wal", "bored.db-wal"),
        ("agent-kanban.db-shm", "bored.db-shm"),
    ];
    for (old_name, new_name) in &db_files {
        let src = old_data_dir.join(old_name);
        let dst = new_data_dir.join(new_name);
        if src.exists() {
            if let Err(e) = std::fs::copy(&src, &dst) {
                eprintln!("Failed to copy {:?} -> {:?}: {}", src, dst, e);
                return;
            }
        }
    }

    // Copy config/settings files (non-destructive: skip if destination already exists)
    let config_files = [
        "claude_api_settings.json",
        "codex_api_settings.json",
    ];
    for name in &config_files {
        let src = old_data_dir.join(name);
        let dst = new_data_dir.join(name);
        if src.exists() && !dst.exists() {
            let _ = std::fs::copy(&src, &dst);
        }
    }

    // Copy custom commands directory if present
    let old_cmds = old_data_dir.join("custom-commands");
    let new_cmds = new_data_dir.join("custom-commands");
    if old_cmds.is_dir() && !new_cmds.exists() {
        if let Err(e) = copy_dir_recursive(&old_cmds, &new_cmds) {
            eprintln!("Failed to copy custom-commands: {}", e);
        }
    }

    eprintln!("Migration from old app directory complete.");
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else {
            std::fs::copy(entry.path(), dest_path)?;
        }
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
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");

            if let Err(e) = logging::init_logging(app_data_dir.clone()) {
                eprintln!("Failed to initialize logging: {}", e);
            }

            tracing::info!("Bored starting up...");
            tracing::info!("App data directory: {:?}", app_data_dir);

            // Resolve bundled command templates from Tauri resources so that
            // production builds can find them (CARGO_MANIFEST_DIR doesn't exist
            // outside the dev source tree).
            match app
                .path()
                .resolve("scripts/commands", tauri::path::BaseDirectory::Resource)
            {
                Ok(resource_commands) if resource_commands.exists() => {
                    tracing::info!(
                        "Bundled commands resource path: {:?}",
                        resource_commands
                    );
                    bored::agents::command_templates::init_resource_commands_path(
                        resource_commands,
                    );
                }
                Ok(resource_commands) => {
                    tracing::warn!(
                        "Bundled commands resource path resolved but does not exist: {:?}",
                        resource_commands
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to resolve bundled commands resource path: {}",
                        e
                    );
                }
            }

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

            // Build the agent registry with all known providers
            let mut agent_registry = AgentRegistry::new();
            agent_registry.register(Arc::new(ClaudeProvider::new()));
            agent_registry.register(Arc::new(CursorProvider::new()));
            agent_registry.register(Arc::new(CodexProvider::new()));

            // One-time migration: copy data from the old "com.agent-kanban.app" directory
            // if it exists and the new database hasn't been populated yet.
            migrate_from_old_app_dir(&app_data_dir);

            let db_path = app_data_dir.join("bored.db");
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

            app.manage(agent_registry);

            app.manage(database.clone());
            app.manage(RunningAgents::new());
            app.manage(AppProcessManager::new());
            app.manage(tray::NotificationsEnabled(
                std::sync::atomic::AtomicBool::new(true),
            ));

            let agent_settings = AgentSettingsManager::new();
            let claude_settings_path = app_data_dir.join("claude_api_settings.json");
            agent_settings.register_agent_settings_path("claude", claude_settings_path);
            let codex_settings_path = app_data_dir.join("codex_api_settings.json");
            agent_settings.register_agent_settings_path("codex", codex_settings_path);
            app.manage(agent_settings);

            // Workflow settings (synced from frontend, read by workers at task time)
            app.manage(WorkflowSettingsState::new());

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

            let api_url = format!("http://127.0.0.1:{}", api_config.port);

            // Create shared event channel for SSE broadcasting
            let event_tx = api::create_event_channel();

            // Manage shared state for commands that need API/event access
            app.manage(event_tx.clone());
            app.manage(ApiConnState { url: api_url, port: api_config.port, token: api_token });

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

            if let Err(e) = tray::setup_tray(app) {
                tracing::error!("Failed to setup system tray: {}", e);
            }

            tracing::info!("Bored initialized successfully");

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
            commands::get_ticket,
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
            commands::runs::get_recent_runs_with_context,
            commands::runs::get_agent_run,
            commands::runs::cancel_agent_run,
            commands::runs::cleanup_stale_runs,
            commands::runs::get_run_events,
            commands::runs::get_implementation_todos,
            commands::runs::get_ticket_cost,
            commands::runs::backfill_run_costs,
            commands::get_projects,
            commands::get_project,
            commands::create_project,
            commands::update_project,
            commands::delete_project,
            commands::set_board_project,
            commands::set_ticket_project,
            commands::check_ticket_readiness,
            commands::browse_for_directory,
            commands::check_git_status,
            commands::init_git_repo,
            commands::create_project_folder,
            // Unified agent integration
            commands::agents::get_agent_status,
            commands::agents::check_agent_available,
            // Agent settings (generic API)
            commands::agent_settings::get_agent_settings,
            commands::agent_settings::set_agent_settings,
            // Workflow settings sync
            commands::workflow_settings::sync_agent_configs,
            // Worker management
            commands::workers::start_worker,
            commands::workers::stop_worker,
            commands::workers::stop_all_workers,
            commands::workers::get_workers,
            commands::workers::get_worker_queue_status,
            // Worker validation and commands
            commands::workers::get_commands_path,
            commands::workers::get_available_commands,
            commands::workers::read_command_content,
            commands::workers::save_custom_command,
            commands::workers::delete_custom_command,
            // API configuration
            commands::get_api_config,
            commands::set_notifications_enabled,
            // Agent registry
            commands::get_available_agents,
            commands::agents::list_cursor_models,
            // Task queue management
            commands::tasks::get_tasks,
            commands::tasks::create_task,
            commands::tasks::add_command_task,
            commands::tasks::delete_task,
            commands::tasks::get_task_counts,
            commands::tasks::update_task,
            commands::tasks::reset_task,
            // Spec / Planner commands
            commands::specs::create_spec,
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
            commands::specs::pause_spec_work,
            commands::specs::resume_spec_work,
            commands::specs::halt_spec_work,
            commands::specs::reset_plan_execution,
            commands::specs::get_spec_eta,
            commands::specs::get_spec_cost,
            commands::specs::get_version_progress,
            // Spec version commands
            commands::specs::get_spec_versions,
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
            commands::tickets::resolve_clarification,
            // Release notes
            commands::release_notes::get_release_notes,
            commands::release_notes::get_all_release_notes,
            // Validation commands
            commands::validation::create_validation_session,
            commands::validation::get_validation_session,
            commands::validation::get_validation_sessions,
            commands::validation::delete_validation_session,
            commands::validation::get_validation_messages,
            commands::validation::send_validation_message,
            commands::validation::stop_validation_app,
            commands::validation::get_validation_app_status,
            commands::validation::create_fix_tasks,
            // Next steps commands (push, PR, diff, open)
            commands::next_steps::push_branch,
            commands::next_steps::create_pull_request,
            commands::next_steps::get_branch_diff,
            commands::next_steps::get_branch_diff_files,
            // Dashboard commands
            commands::dashboard::get_dashboard_summary,
            commands::dashboard::get_dashboard_trends,
            commands::dashboard::get_model_breakdown,
            commands::dashboard::get_agent_breakdown,
            commands::dashboard::backfill_git_stats,
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
