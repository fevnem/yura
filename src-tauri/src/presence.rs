//! Presence: the parts of her that keep working when the window is closed.
//!
//! A reminder that only exists while the app is on screen is not a reminder. So
//! timers are armed as detached tokio tasks that survive chat switching, the
//! tray keeps her reachable, and startup is a setting rather than an installer
//! checkbox — the user can change their mind without reinstalling.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use yura_tools::Timer;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Show a desktop notification, quietly doing nothing if the platform says no.
pub fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app
        .notification()
        .builder()
        .title(title)
        .body(body)
        .show();
}

/// Arm a timer: sleep until it is due, then tell both the desktop and the UI.
///
/// The task owns nothing borrowed, so it outlives the request that created it.
pub fn arm(app: &AppHandle, timer: &Timer) {
    let app = app.clone();
    let timer = timer.clone();

    tauri::async_runtime::spawn(async move {
        let due = timer.due.max(now_ms());
        let wait = Duration::from_millis(due.saturating_sub(now_ms()));
        tokio::time::sleep(wait).await;

        notify(&app, "Yura", &timer.label);
        let _ = app.emit("reminder:due", &timer);

        // Keep the file honest: a fired timer is not pending.
        if let Ok(dir) = yura_store::data_dir() {
            let path = dir.join("agenda.json");
            let mut agenda = yura_tools::Agenda::load(&path);
            if let Some(entry) = agenda.timers.iter_mut().find(|t| t.id == timer.id) {
                entry.fired = true;
            }
            agenda.prune();
            let _ = agenda.save(&path);
        }
    });
}

/// Build the tray icon and its two-item menu.
pub fn init(app: &AppHandle, visible: bool) {
    let show = MenuItem::with_id(app, "show", "Open Yura", true, None::<&str>);
    let fresh = MenuItem::with_id(app, "new", "New conversation", true, None::<&str>);
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>);

    let menu = match (show, fresh, quit) {
        (Ok(show), Ok(fresh), Ok(quit)) => Menu::with_items(app, &[&show, &fresh, &quit]),
        _ => return,
    };

    let mut builder = TrayIconBuilder::with_id("yura")
        .tooltip("Yura")
        .menu(&match menu {
            Ok(menu) => menu,
            Err(_) => return,
        })
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => reveal(app),
            "new" => {
                reveal(app);
                let _ = app.emit("tray:new-chat", ());
            }
            "quit" => {
                if let Some(state) = app.try_state::<crate::state::AppState>() {
                    let settings = state.snapshot();
                    let _ = state.settings_store.save(&settings);
                }
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                reveal(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }

    if let Ok(tray) = builder.build(app) {
        let _ = tray.set_visible(visible);
    }
}

pub fn reveal(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Turn start-on-login on or off. Errors are surfaced, not swallowed.
pub fn apply_autostart(app: &AppHandle, on: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    if on {
        manager.enable().map_err(|e| e.to_string())
    } else {
        manager.disable().map_err(|e| e.to_string())
    }
}
