//! System tray icon, context menu, and tray interaction handlers.

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle,
};

use crate::{dispatch_zoom_action, state::ZoomAction, window::show_main_window};

const TRAY_ID: &str = "main-tray";
const MENU_RESET_ZOOM: &str = "reset_zoom";
const MENU_SHOW_SETTINGS: &str = "show_settings";
const MENU_EXIT: &str = "exit";

/// Creates and registers the application system tray icon and context menu.
pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let reset_item = MenuItem::with_id(app, MENU_RESET_ZOOM, "Reset Zoom", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, MENU_SHOW_SETTINGS, "Settings", true, None::<&str>)?;
    let exit_item = MenuItem::with_id(app, MENU_EXIT, "Exit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&reset_item, &settings_item, &exit_item])?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Window Pointer Zoom")
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_RESET_ZOOM => dispatch_zoom_action(app, ZoomAction::Reset),
            MENU_SHOW_SETTINGS => show_main_window(app),
            MENU_EXIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }

    let _ = builder.build(app)?;
    Ok(())
}
