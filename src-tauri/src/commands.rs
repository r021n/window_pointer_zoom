//! Thin Tauri IPC command handlers delegating to `AppState`.

use tauri::State;

use crate::state::{AppState, PanningConfig, ZoomStatus};

#[tauri::command]
pub fn get_zoom_status(state: State<'_, AppState>) -> ZoomStatus {
    state.status()
}

#[tauri::command]
pub fn zoom_in(state: State<'_, AppState>) -> Result<ZoomStatus, String> {
    state.zoom_in()
}

#[tauri::command]
pub fn zoom_out(state: State<'_, AppState>) -> Result<ZoomStatus, String> {
    state.zoom_out()
}

#[tauri::command]
pub fn reset_zoom(state: State<'_, AppState>) -> Result<ZoomStatus, String> {
    state.reset_zoom()
}

#[tauri::command]
pub fn set_zoom_level(scale: f32, state: State<'_, AppState>) -> Result<ZoomStatus, String> {
    state.set_zoom_level(scale)
}

#[tauri::command]
pub fn update_panning_config(
    config: PanningConfig,
    state: State<'_, AppState>,
) -> Result<ZoomStatus, String> {
    state.update_panning_config(config)
}
