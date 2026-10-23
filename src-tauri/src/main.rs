//! Process entry point.
//!
//! Responsibilities, in order: install logging, build the app state once, wire
//! commands, restore window preferences, run. Anything more here would belong
//! in `state` or `commands`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod presence;
mod state;
mod tools_cmd;
mod window;

use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;

fn main() {
    init_tracing();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(state::AppState::bootstrap())
        .manage(tools_cmd::Approvals::default())
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::save_settings,
            commands::apply_provider_preset,
            commands::send_message,
            commands::cancel_generation,
            commands::clear_history,
            commands::legacy_clear_history,
            commands::list_chats,
            commands::new_chat,
            commands::load_chat,
            commands::rename_chat,
            commands::pin_chat,
            commands::delete_chat,
            commands::list_skills,
            commands::save_skill,
            commands::delete_skill,
            commands::recent_history,
            commands::set_always_on_top,
            tools_cmd::run_tool,
            tools_cmd::respond_approval,
            tools_cmd::library_list,
            tools_cmd::library_read,
            tools_cmd::library_write,
            tools_cmd::agenda_get,
            tools_cmd::agenda_add_timer,
            tools_cmd::agenda_add_task,
            tools_cmd::agenda_complete,
            tools_cmd::pick_workspace,
            tools_cmd::ingest_attachment,
            tools_cmd::open_data_dir,
            tools_cmd::apply_general,
            tools_cmd::resource_status,
            tools_cmd::download_resource,
            tools_cmd::remove_resource,
        ])
        .setup(|app| {
            if let Some(main) = app.get_webview_window("main") {
                let state = app.state::<state::AppState>();
                let settings = state.snapshot();
                window::restore(&main, &settings);
            }

            let state = app.state::<state::AppState>();
            let settings = state.snapshot();
            let handle = app.handle().clone();
            presence::init(&handle, settings.general.tray);
            if settings.general.autostart {
                if let Err(error) = presence::apply_autostart(&handle, true) {
                    tracing::warn!(%error, "could not enable start-on-login");
                }
            }

            tracing::info!("yura {} ready", env!("CARGO_PKG_VERSION"));
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let settings = window.app_handle().state::<state::AppState>().snapshot();
                if settings.general.tray && settings.general.close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("yura failed to start");
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_from_env("YURA_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).with_target(false).init();
}
