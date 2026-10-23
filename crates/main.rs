//! Process entry point.
//!
//! Responsibilities, in order: install logging, build the app state once, wire
//! commands, restore window preferences, run. Anything more here would belong
//! in `state` or `commands`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;
mod window;

use tauri::Manager;

fn main() {
    init_tracing();

    tauri::Builder::default()
        .manage(state::AppState::bootstrap())
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::save_settings,
            commands::apply_provider_preset,
            commands::send_message,
            commands::cancel_generation,
            commands::clear_history,
            commands::list_skills,
            commands::save_skill,
            commands::delete_skill,
            commands::recent_history,
            commands::set_always_on_top,
        ])
        .setup(|app| {
            if let Some(main) = app.get_webview_window("main") {
                let state = app.state::<state::AppState>();
                let settings = state.snapshot();
                window::restore(&main, &settings);
            }
            tracing::info!("yura {} ready", env!("CARGO_PKG_VERSION"));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("yura failed to start");
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_from_env("YURA_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).with_target(false).init();
}
