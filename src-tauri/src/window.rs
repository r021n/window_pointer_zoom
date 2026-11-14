//! Main window visibility and hide-to-tray lifecycle helpers.

use tauri::{AppHandle, Manager, Window, WindowEvent};

/// Restores, shows, and focuses the `"main"` application window.
pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Intercepts close and minimize events on the main window to hide it to the system tray.
pub fn handle_window_event(window: &Window, event: &WindowEvent) {
    match event {
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            let _ = window.hide();
        }
        WindowEvent::Resized(_) if window.is_minimized().unwrap_or(false) => {
            let _ = window.hide();
        }
        _ => {}
    }
}
