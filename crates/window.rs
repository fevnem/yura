use tauri::{LogicalSize, WebviewWindow};
use yura_core::Settings;

/// Apply the window-related settings. Called once at startup and again
/// whenever the settings panel saves, so pinning takes effect immediately.
pub fn restore(window: &WebviewWindow, settings: &Settings) {
    if let Err(e) = window.set_always_on_top(settings.ui.always_on_top) {
        tracing::warn!(error = %e, "could not set always-on-top");
    }

    // The window is frameless: give the content exactly the size requested in
    // the config on first run, then let the user resize freely.
    if let Ok(size) = window.inner_size() {
        if size.width == 0 || size.height == 0 {
            let _ = window.set_size(LogicalSize::new(1180.0, 780.0));
        }
    }
}
