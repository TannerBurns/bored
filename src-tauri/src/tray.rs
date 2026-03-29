use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::image::Image;
use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager};

use crate::db::Database;

const TRAY_ID: &str = "main";
const RECENT_VISIBLE: u32 = 3;
const MORE_VISIBLE: u32 = 5;
const TRAY_ICON_BYTES: &[u8] = include_bytes!("../icons/tray-icon.png");

pub struct NotificationsEnabled(pub AtomicBool);

fn truncate_title(title: &str, max: usize) -> String {
    if title.chars().count() > max {
        format!("{}...", title.chars().take(max - 3).collect::<String>())
    } else {
        title.to_string()
    }
}

pub fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    setup_tray_inner(app.handle())
}

/// Variant that accepts `AppHandle` so it can be called from a background
/// task after setup() returns (the `&App` reference is only available
/// inside the setup closure).
pub fn setup_tray_deferred(handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    setup_tray_inner(handle)
}

fn setup_tray_inner(handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let db = handle.state::<Arc<Database>>();
    let menu = build_tray_menu(handle, &db)?;
    let icon = Image::from_bytes(TRAY_ICON_BYTES)?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .icon_as_template(true)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(handle_menu_event)
        .tooltip("Bored")
        .build(handle)?;

    Ok(())
}

pub fn are_notifications_enabled(app_handle: &AppHandle) -> bool {
    app_handle
        .try_state::<NotificationsEnabled>()
        .map(|s| s.0.load(Ordering::Relaxed))
        .unwrap_or(true)
}

fn build_tray_menu(
    handle: &AppHandle,
    db: &Database,
) -> Result<tauri::menu::Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    let tickets = db
        .get_recent_tickets_with_columns(RECENT_VISIBLE + MORE_VISIBLE)
        .unwrap_or_default();

    let menu = MenuBuilder::new(handle);

    let header = MenuItemBuilder::with_id("header", "Recent Tickets")
        .enabled(false)
        .build(handle)?;

    let menu = menu.item(&header);

    let (visible, more) = if tickets.len() > RECENT_VISIBLE as usize {
        tickets.split_at(RECENT_VISIBLE as usize)
    } else {
        (tickets.as_slice(), &[] as &[(_, _)])
    };

    let menu = if visible.is_empty() {
        let empty = MenuItemBuilder::with_id("no-tickets", "  No tickets yet")
            .enabled(false)
            .build(handle)?;
        menu.item(&empty)
    } else {
        let mut m = menu;
        for (ticket, col_name) in visible {
            let label = format!(
                "{} — {}",
                truncate_title(&ticket.title, 40),
                col_name,
            );
            let item =
                MenuItemBuilder::with_id(format!("ticket:{}", ticket.id), &label).build(handle)?;
            m = m.item(&item);
        }
        m
    };

    let menu = if !more.is_empty() {
        let mut sub = SubmenuBuilder::with_id(handle, "more-tickets", "More Tickets...");
        for (ticket, col_name) in more {
            let label = format!(
                "{} — {}",
                truncate_title(&ticket.title, 40),
                col_name,
            );
            let item =
                MenuItemBuilder::with_id(format!("ticket:{}", ticket.id), &label).build(handle)?;
            sub = sub.item(&item);
        }
        menu.item(&sub.build()?)
    } else {
        menu
    };

    let sep1 = PredefinedMenuItem::separator(handle)?;
    let open = MenuItemBuilder::with_id("open", "Open Bored").build(handle)?;
    let settings = MenuItemBuilder::with_id("settings", "Open Settings").build(handle)?;
    let sep2 = PredefinedMenuItem::separator(handle)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit Bored").build(handle)?;

    let menu = menu
        .item(&sep1)
        .item(&open)
        .item(&settings)
        .item(&sep2)
        .item(&quit);

    Ok(menu.build()?)
}

fn handle_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    let id = event.id().as_ref();

    match id {
        "open" => focus_main_window(app),
        "settings" => {
            focus_main_window(app);
            if let Err(e) = app.emit("navigate-to-settings", ()) {
                tracing::warn!("Failed to emit navigate-to-settings: {}", e);
            }
        }
        "quit" => {
            app.exit(0);
        }
        _ if id.starts_with("ticket:") => {
            let ticket_id = &id["ticket:".len()..];
            focus_main_window(app);
            if let Err(e) = app.emit("open-ticket", ticket_id.to_string()) {
                tracing::warn!("Failed to emit open-ticket: {}", e);
            }
        }
        _ => {}
    }
}

fn focus_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.set_focus();
        let _ = window.show();
    }
}

/// Rebuild the tray menu with fresh ticket data.
/// Call after ticket status changes to keep the tray current.
pub fn refresh_tray(app_handle: &AppHandle) {
    let db = app_handle.state::<Arc<Database>>();

    match build_tray_menu(app_handle, &db) {
        Ok(menu) => {
            if let Some(tray) = app_handle.tray_by_id(TRAY_ID) {
                if let Err(e) = tray.set_menu(Some(menu)) {
                    tracing::warn!("Failed to update tray menu: {}", e);
                }
            }
        }
        Err(e) => {
            tracing::warn!("Failed to build tray menu: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod truncate_title_tests {
        use super::*;

        #[test]
        fn short_title_unchanged() {
            assert_eq!(truncate_title("Hello", 40), "Hello");
        }

        #[test]
        fn exact_boundary_unchanged() {
            let title = "A".repeat(40);
            assert_eq!(truncate_title(&title, 40), title);
        }

        #[test]
        fn long_title_truncated_with_ellipsis() {
            let title = "A".repeat(50);
            let result = truncate_title(&title, 40);
            assert_eq!(result.chars().count(), 40);
            assert!(result.ends_with("..."));
            assert_eq!(&result[..result.len() - 3], "A".repeat(37).as_str());
        }

        #[test]
        fn empty_string() {
            assert_eq!(truncate_title("", 40), "");
        }

        #[test]
        fn unicode_emoji_truncation() {
            let title = "🎉".repeat(50);
            let result = truncate_title(&title, 10);
            assert_eq!(result.chars().count(), 10);
            assert!(result.ends_with("..."));
            assert_eq!(result.chars().filter(|&c| c == '🎉').count(), 7);
        }

        #[test]
        fn one_over_boundary() {
            let title = "A".repeat(41);
            let result = truncate_title(&title, 40);
            assert_eq!(result.chars().count(), 40);
            assert!(result.ends_with("..."));
        }
    }
}
