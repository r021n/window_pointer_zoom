//! Tauri application builder, managed state lifecycle, and IPC command registration.

mod commands;
pub mod engine;
mod shortcuts;
pub mod state;
mod tray;
mod window;

use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_store::StoreExt;

use crate::engine::fullscreen_engine::{
    enable_per_monitor_dpi_awareness, install_console_ctrl_handler,
};
use crate::engine::wheel_hook::{spawn_wheel_hook, WheelHookState};
use crate::shortcuts::{ShortcutConfig, ShortcutManager};
use crate::state::{AppState, PersistedEngineSettings, ZoomAction, ZoomStatus};

const ZOOM_STATUS_EVENT: &str = "zoom-status-changed";
const SETTINGS_STORE_PATH: &str = "settings.json";
const ENGINE_SETTINGS_KEY: &str = "engineSettings";
const SHORTCUTS_KEY: &str = "shortcuts";

/// Broadcasts an updated `ZoomStatus` snapshot to all frontend listeners.
pub(crate) fn emit_zoom_status(app: &AppHandle, status: ZoomStatus) {
    let _ = app.emit(ZOOM_STATUS_EVENT, status);
}

/// Applies a zoom action through `AppState` and notifies the frontend of the updated status.
pub(crate) fn dispatch_zoom_action(app: &AppHandle, action: ZoomAction) {
    match app.state::<AppState>().apply_action(action) {
        Ok(status) => emit_zoom_status(app, status),
        Err(err) => {
            eprintln!("Failed to apply zoom action {action:?}: {err}");
        }
    }
}

/// Hydrates `AppState` and `ShortcutManager` from raw JSON store values, ignoring invalid entries.
pub(crate) fn hydrate_settings_from_values(
    engine_val: Option<Value>,
    shortcuts_val: Option<Value>,
    app_state: &AppState,
    shortcut_mgr: &ShortcutManager,
) {
    if let Some(val) = engine_val {
        match serde_json::from_value::<PersistedEngineSettings>(val) {
            Ok(persisted) => {
                if let Err(err) = app_state.update_zoom_limits(persisted.limits) {
                    eprintln!("Warning: failed to hydrate zoom limits: {err}");
                }
                if let Err(err) = app_state.update_panning_config(persisted.panning) {
                    eprintln!("Warning: failed to hydrate panning config: {err}");
                }
            }
            Err(err) => {
                eprintln!("Warning: ignoring invalid '{ENGINE_SETTINGS_KEY}' in store: {err}");
            }
        }
    }

    if let Some(val) = shortcuts_val {
        match serde_json::from_value::<ShortcutConfig>(val) {
            Ok(shortcuts_cfg) => {
                if let Err(err) = shortcut_mgr.hydrate(shortcuts_cfg) {
                    eprintln!("Warning: ignoring invalid '{SHORTCUTS_KEY}' in store: {err}");
                }
            }
            Err(err) => {
                eprintln!("Warning: ignoring invalid '{SHORTCUTS_KEY}' in store: {err}");
            }
        }
    }
}

fn hydrate_from_store(app: &AppHandle) {
    match app.store(SETTINGS_STORE_PATH) {
        Ok(store) => {
            hydrate_settings_from_values(
                store.get(ENGINE_SETTINGS_KEY),
                store.get(SHORTCUTS_KEY),
                &app.state::<AppState>(),
                &app.state::<ShortcutManager>(),
            );
        }
        Err(err) => {
            eprintln!("Warning: failed to open '{SETTINGS_STORE_PATH}': {err}");
        }
    }
}

/// Configures and runs the Tauri desktop application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    enable_per_monitor_dpi_awareness();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            window::show_main_window(app);
        }))
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(shortcuts::plugin())
        .manage(AppState::new())
        .manage(WheelHookState::new())
        .manage(ShortcutManager::new())
        .on_window_event(window::handle_window_event)
        .setup(|app| {
            hydrate_from_store(app.handle());

            if !std::env::args().any(|arg| arg == "--minimized") {
                window::show_main_window(app.handle());
            }

            let status_handle = app.handle().clone();
            app.state::<AppState>()
                .start(Some(Arc::new(move |status| {
                    emit_zoom_status(&status_handle, status);
                })))
                .map_err(|err| -> Box<dyn std::error::Error> { err.into() })?;
            tray::setup_tray(app.handle())?;
            shortcuts::register_shortcuts(app.handle());
            let hook_handle = spawn_wheel_hook(app.handle().clone())
                .map_err(|err| -> Box<dyn std::error::Error> { err.into() })?;
            app.state::<WheelHookState>().set(hook_handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_zoom_status,
            commands::get_engine_settings,
            commands::zoom_in,
            commands::zoom_out,
            commands::reset_zoom,
            commands::set_zoom_level,
            commands::update_panning_config,
            commands::update_zoom_limits,
            commands::update_shortcuts,
            commands::set_shortcuts_paused,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    let exit_handle = app.handle().clone();
    let close_handle = app.handle().clone();
    let _console_ctrl_guard = install_console_ctrl_handler(
        Arc::new(move || {
            exit_handle.exit(0);
        }),
        Arc::new(move || {
            close_handle.state::<AppState>().shutdown();
        }),
    );

    app.run(|app, event| {
        if let tauri::RunEvent::Exit = event {
            shortcuts::unregister_all(app);
            app.state::<WheelHookState>().shutdown();
            app.state::<AppState>().shutdown();
        }
    });
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::engine::viewport_math::{PanningMode, MAX_EDGE_DEADZONE_RATIO};
    use crate::state::{config_constraints, EngineSettingsSnapshot, PanningConfig, ZoomLimits};

    #[test]
    fn hydrate_settings_from_values_applies_and_sanitizes_camelcase_payload() {
        let app_state = AppState::new();
        let shortcut_mgr = ShortcutManager::new();

        let engine_val = json!({
            "limits": {
                "minScale": 1.5,
                "defaultScale": 2.75,
                "maxScale": 6.0
            },
            "panning": {
                "mode": "edge_panning",
                "zoomStep": 0.5,
                "zoomAlpha": 0.2,
                "panAlpha": 0.35,
                "edgeDeadzone": 0.99
            }
        });
        let shortcuts_val = json!({
            "toggle": "Ctrl+Shift+Z",
            "zoomIn": "Ctrl+Shift+Up",
            "zoomOut": "Ctrl+Shift+Down",
            "reset": "Ctrl+Shift+0"
        });

        hydrate_settings_from_values(
            Some(engine_val),
            Some(shortcuts_val),
            &app_state,
            &shortcut_mgr,
        );

        let status = app_state.status();
        assert_eq!(
            status.limits,
            ZoomLimits {
                min_scale: 1.5,
                default_scale: 2.75,
                max_scale: 6.0,
            }
        );
        assert_eq!(
            status.config,
            PanningConfig {
                mode: PanningMode::EdgePanning,
                zoom_step: 0.5,
                zoom_alpha: 0.2,
                pan_alpha: 0.35,
                edge_deadzone: MAX_EDGE_DEADZONE_RATIO,
            }
        );
        assert_eq!(
            shortcut_mgr.config(),
            ShortcutConfig {
                toggle: "Ctrl+Shift+Z".to_string(),
                zoom_in: "Ctrl+Shift+Up".to_string(),
                zoom_out: "Ctrl+Shift+Down".to_string(),
                reset: "Ctrl+Shift+0".to_string(),
            }
        );

        // Toggle uses hydrated default_scale = 2.75
        let toggled = app_state.toggle_zoom().expect("toggle succeeds");
        assert_eq!(toggled.target_scale, 2.75);
    }

    #[test]
    fn hydrate_settings_ignores_corrupt_or_duplicate_entries() {
        let app_state = AppState::new();
        let shortcut_mgr = ShortcutManager::new();

        hydrate_settings_from_values(
            Some(json!("not-an-object")),
            Some(json!({
                "toggle": "Ctrl+Alt+Z",
                "zoomIn": "Ctrl+Alt+Z",
                "zoomOut": "Ctrl+Alt+Down",
                "reset": "Ctrl+Alt+0"
            })),
            &app_state,
            &shortcut_mgr,
        );

        assert_eq!(app_state.status().limits, ZoomLimits::default());
        assert_eq!(app_state.status().config, PanningConfig::default());
        assert_eq!(shortcut_mgr.config(), ShortcutConfig::default());
    }

    #[test]
    fn engine_settings_snapshot_serializes_expected_camelcase_contract() {
        let app_state = AppState::new();
        let shortcut_mgr = ShortcutManager::new();
        let snapshot = EngineSettingsSnapshot {
            status: app_state.status(),
            shortcuts: shortcut_mgr.config(),
            constraints: config_constraints(),
        };

        let serialized = serde_json::to_value(&snapshot).expect("serialize snapshot");
        assert!(serialized.get("status").is_some());
        assert!(serialized.get("shortcuts").is_some());
        let constraints = serialized.get("constraints").expect("constraints present");
        assert!(constraints.get("scaleEnvelope").is_some());
        assert!(constraints.get("zoomStep").is_some());
        assert!(constraints.get("alpha").is_some());
        assert!(constraints.get("edgeDeadzone").is_some());
        assert!(constraints.get("defaultLimits").is_some());
        assert!(constraints.get("defaultPanning").is_some());
        assert!(constraints.get("defaultShortcuts").is_some());

        let limits = serialized["status"]["limits"]
            .as_object()
            .expect("limits obj");
        assert!(limits.contains_key("minScale"));
        assert!(limits.contains_key("defaultScale"));
        assert!(limits.contains_key("maxScale"));

        let roundtrip: EngineSettingsSnapshot =
            serde_json::from_value(serialized).expect("deserialize snapshot");
        assert_eq!(roundtrip, snapshot);
    }
}
