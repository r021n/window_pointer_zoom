//! Coordinates native pointer controls, persisted preferences, and UI status.

use std::sync::{Arc, Mutex, MutexGuard};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, Wry};
use tauri_plugin_store::{Store, StoreExt};

use crate::engine::click_overlay::{ClickEventSink, ClickOverlayState};
use crate::engine::fullscreen_engine::AppliedZoomTransform;
use crate::engine::pointer_size::{PointerAction, PointerSettings, PointerSize, PointerStatus};
use crate::SETTINGS_STORE_PATH;

pub(crate) const POINTER_SETTINGS_KEY: &str = "pointerSettings";
const POINTER_STATUS_EVENT: &str = "pointer-status-changed";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PointerSnapshot {
    #[serde(flatten)]
    status: PointerStatus,
    revision: u64,
    click_effect_available: bool,
}

pub(crate) struct PointerControls {
    sizing: PointerSize,
    overlay: ClickOverlayState,
    revision: Mutex<u64>,
}

impl PointerControls {
    pub(crate) fn new() -> Self {
        Self {
            sizing: PointerSize::default(),
            overlay: ClickOverlayState::default(),
            revision: Mutex::new(0),
        }
    }

    pub(crate) fn hydrate(&self, value: Option<Value>) -> Result<(), String> {
        if let Some(value) = value {
            let settings =
                serde_json::from_value::<PointerSettings>(value).map_err(|err| err.to_string())?;
            self.sizing.update(settings)?;
        }
        Ok(())
    }

    pub(crate) fn start_overlay(
        &self,
        transform: Arc<AppliedZoomTransform>,
    ) -> Result<ClickEventSink, String> {
        self.overlay.start(
            self.sizing.status().settings.click_effect_enabled,
            transform,
        )
    }

    pub(crate) fn snapshot(&self) -> PointerSnapshot {
        let revision = self.lock_revision();
        self.make_snapshot(self.sizing.status(), *revision)
    }

    pub(crate) fn update_settings(
        &self,
        app: &AppHandle,
        settings: PointerSettings,
    ) -> Result<PointerSnapshot, String> {
        let mut revision = self.lock_revision();
        let settings = settings.validated()?;
        let previous = self.sizing.status();
        if previous.settings == settings {
            return Ok(self.make_snapshot(previous, *revision));
        }
        if settings.click_effect_enabled
            && !previous.settings.click_effect_enabled
            && !self.overlay.is_available()
        {
            return Err("The click effect could not be initialized.".to_string());
        }

        let store = app
            .store(SETTINGS_STORE_PATH)
            .map_err(|err| err.to_string())?;
        let previous_value = store.get(POINTER_SETTINGS_KEY);
        let next_value = serde_json::to_value(settings).map_err(|err| err.to_string())?;
        store.set(POINTER_SETTINGS_KEY, next_value);
        let result = store
            .save()
            .map_err(|err| format!("Failed to save pointer settings: {err}"))
            .and_then(|()| self.sizing.update(settings));

        match result {
            Ok(status) => {
                self.overlay
                    .set_enabled(status.settings.click_effect_enabled);
                *revision += 1;
                Ok(self.make_snapshot(status, *revision))
            }
            Err(err) => match restore_saved_settings(&store, previous_value) {
                Ok(()) => Err(err),
                Err(restore_err) => Err(format!("{err}. {restore_err}")),
            },
        }
    }

    pub(crate) fn apply(&self, action: PointerAction) -> Result<PointerSnapshot, String> {
        let mut revision = self.lock_revision();
        let result = self.sizing.apply(action);
        *revision += 1;
        result.map(|status| self.make_snapshot(status, *revision))
    }

    pub(crate) fn shutdown(&self) {
        let _revision = self.lock_revision();
        if let Err(err) = self.sizing.shutdown() {
            eprintln!("Warning: failed to restore the Windows pointer size: {err}");
        }
        self.overlay.shutdown();
    }

    fn lock_revision(&self) -> MutexGuard<'_, u64> {
        self.revision.lock().unwrap_or_else(|err| err.into_inner())
    }

    fn make_snapshot(&self, status: PointerStatus, revision: u64) -> PointerSnapshot {
        PointerSnapshot {
            status,
            revision,
            click_effect_available: self.overlay.is_available(),
        }
    }
}

fn restore_saved_settings(store: &Store<Wry>, previous: Option<Value>) -> Result<(), String> {
    if let Some(value) = previous {
        store.set(POINTER_SETTINGS_KEY, value);
    } else {
        store.delete(POINTER_SETTINGS_KEY);
    }
    store
        .save()
        .map_err(|err| format!("Failed to restore saved pointer settings: {err}"))
}

pub(crate) fn dispatch_shortcut(app: &AppHandle, action: PointerAction) {
    let controls = app.state::<PointerControls>();
    if !controls.sizing.status().settings.enabled {
        return;
    }
    let snapshot = controls.apply(action).unwrap_or_else(|err| {
        eprintln!("Failed to apply pointer action {action:?}: {err}");
        controls.snapshot()
    });
    let _ = app.emit(POINTER_STATUS_EVENT, snapshot);
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn hydrates_partial_preferences_and_exposes_a_flat_camel_case_snapshot() {
        let controls = PointerControls::new();
        controls
            .hydrate(Some(json!({ "clickEffectEnabled": true })))
            .unwrap();

        let snapshot = serde_json::to_value(controls.snapshot()).unwrap();
        assert_eq!(snapshot["settings"]["clickEffectEnabled"], true);
        assert_eq!(snapshot["settings"]["enabled"], false);
        assert_eq!(
            snapshot["settings"]["step"],
            snapshot["constraints"]["defaultStep"]
        );
        assert!(snapshot.get("baselineSize").is_some());
        assert!(snapshot.get("currentSize").is_some());
        assert!(snapshot.get("error").is_some());
        assert_eq!(snapshot["revision"], 0);
        assert_eq!(snapshot["clickEffectAvailable"], false);
    }
}
