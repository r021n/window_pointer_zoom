//! Thin Tauri IPC handlers for zoom, shortcuts, and pointer controls.

use tauri::{AppHandle, State};

use crate::emit_zoom_status;
use crate::engine::pointer_size::{PointerAction, PointerSettings};
use crate::pointer_controls::{PointerControls, PointerSnapshot};
use crate::shortcuts::{self, ShortcutConfig, ShortcutManager};
use crate::state::{
    config_constraints, AppState, EngineSettingsSnapshot, PanningConfig, ZoomLimits, ZoomStatus,
};

#[tauri::command]
pub fn get_zoom_status(state: State<'_, AppState>) -> ZoomStatus {
    state.status()
}

#[tauri::command]
pub fn get_engine_settings(
    state: State<'_, AppState>,
    shortcut_mgr: State<'_, ShortcutManager>,
) -> EngineSettingsSnapshot {
    EngineSettingsSnapshot {
        status: state.status(),
        shortcuts: shortcut_mgr.config(),
        constraints: config_constraints(),
    }
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
    app: AppHandle,
    config: PanningConfig,
    state: State<'_, AppState>,
) -> Result<ZoomStatus, String> {
    let status = state.update_panning_config(config)?;
    emit_zoom_status(&app, status);
    Ok(status)
}

#[tauri::command]
pub fn update_zoom_limits(
    app: AppHandle,
    limits: ZoomLimits,
    state: State<'_, AppState>,
) -> Result<ZoomStatus, String> {
    let status = state.update_zoom_limits(limits)?;
    emit_zoom_status(&app, status);
    Ok(status)
}

#[tauri::command]
pub fn update_shortcuts(
    app: AppHandle,
    config: ShortcutConfig,
    shortcut_mgr: State<'_, ShortcutManager>,
) -> Result<ShortcutConfig, String> {
    shortcuts::update_shortcuts(&app, &shortcut_mgr, config)
}

#[tauri::command]
pub fn set_shortcuts_paused(
    app: AppHandle,
    paused: bool,
    shortcut_mgr: State<'_, ShortcutManager>,
) -> Result<(), String> {
    shortcuts::set_shortcuts_paused(&app, &shortcut_mgr, paused)
}

#[tauri::command]
pub(crate) fn get_pointer_settings(controls: State<'_, PointerControls>) -> PointerSnapshot {
    controls.snapshot()
}

#[tauri::command]
pub(crate) fn update_pointer_settings(
    app: AppHandle,
    settings: PointerSettings,
    controls: State<'_, PointerControls>,
) -> Result<PointerSnapshot, String> {
    controls.update_settings(&app, settings)
}

#[tauri::command]
pub(crate) fn apply_default_pointer_size(
    controls: State<'_, PointerControls>,
) -> Result<PointerSnapshot, String> {
    controls.apply(PointerAction::ApplyDefault)
}

#[tauri::command]
pub(crate) fn reset_pointer_size(
    controls: State<'_, PointerControls>,
) -> Result<PointerSnapshot, String> {
    controls.apply(PointerAction::Reset)
}
