//! Tauri application builder, managed state lifecycle, and IPC command registration.

mod commands;
pub mod engine;
pub mod state;

use tauri::Manager;

use crate::state::AppState;

/// Configures and runs the Tauri desktop application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .setup(|app| app.state::<AppState>().start().map_err(Into::into))
        .invoke_handler(tauri::generate_handler![
            commands::get_zoom_status,
            commands::zoom_in,
            commands::zoom_out,
            commands::reset_zoom,
            commands::set_zoom_level,
            commands::update_panning_config,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.state::<AppState>().shutdown();
            }
        });
}
