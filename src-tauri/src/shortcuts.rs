//! Dynamic global keyboard shortcut configuration, pause control, and O(1) dispatch.

use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{plugin::TauriPlugin, AppHandle, Manager, Wry};
use tauri_plugin_global_shortcut::{
    Builder, GlobalShortcut, GlobalShortcutExt, Shortcut, ShortcutState,
};

use crate::engine::pointer_size::PointerAction;
use crate::{dispatch_zoom_action, pointer_controls, state::ZoomAction};

pub const DEFAULT_TOGGLE_SHORTCUT: &str = "Ctrl+Alt+Z";
pub const DEFAULT_ZOOM_IN_SHORTCUT: &str = "Ctrl+Alt+Up";
pub const DEFAULT_ZOOM_OUT_SHORTCUT: &str = "Ctrl+Alt+Down";
pub const DEFAULT_RESET_SHORTCUT: &str = "Ctrl+Alt+0";

/// User-configurable global keyboard shortcut bindings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShortcutConfig {
    pub toggle: String,
    pub zoom_in: String,
    pub zoom_out: String,
    pub reset: String,
    pub pointer_size_up: Option<String>,
    pub pointer_size_down: Option<String>,
    pub pointer_size_reset: Option<String>,
}

impl Default for ShortcutConfig {
    fn default() -> Self {
        Self {
            toggle: DEFAULT_TOGGLE_SHORTCUT.to_string(),
            zoom_in: DEFAULT_ZOOM_IN_SHORTCUT.to_string(),
            zoom_out: DEFAULT_ZOOM_OUT_SHORTCUT.to_string(),
            reset: DEFAULT_RESET_SHORTCUT.to_string(),
            pointer_size_up: None,
            pointer_size_down: None,
            pointer_size_reset: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShortcutAction {
    Zoom(ZoomAction),
    Pointer(PointerAction),
}

type ShortcutBinding<'a> = (ShortcutAction, Shortcut, &'a str);

impl ShortcutConfig {
    fn normalized(mut self) -> Self {
        for accelerator in [
            Some(&mut self.toggle),
            Some(&mut self.zoom_in),
            Some(&mut self.zoom_out),
            Some(&mut self.reset),
            self.pointer_size_up.as_mut(),
            self.pointer_size_down.as_mut(),
            self.pointer_size_reset.as_mut(),
        ]
        .into_iter()
        .flatten()
        {
            *accelerator = accelerator.trim().to_string();
        }
        self
    }

    fn entries(&self) -> impl Iterator<Item = (ShortcutAction, &str)> {
        use ShortcutAction::{Pointer, Zoom};
        [
            (Zoom(ZoomAction::Toggle), Some(self.toggle.as_str())),
            (Zoom(ZoomAction::ZoomIn), Some(self.zoom_in.as_str())),
            (Zoom(ZoomAction::ZoomOut), Some(self.zoom_out.as_str())),
            (Zoom(ZoomAction::Reset), Some(self.reset.as_str())),
            (
                Pointer(PointerAction::Increase),
                self.pointer_size_up.as_deref(),
            ),
            (
                Pointer(PointerAction::Decrease),
                self.pointer_size_down.as_deref(),
            ),
            (
                Pointer(PointerAction::Reset),
                self.pointer_size_reset.as_deref(),
            ),
        ]
        .into_iter()
        .filter_map(|(action, raw)| raw.map(|accelerator| (action, accelerator)))
    }

    /// Parses assigned bindings and rejects invalid or duplicate accelerators.
    fn parse_bindings(&self) -> Result<Vec<ShortcutBinding<'_>>, String> {
        let mut seen_ids = HashSet::new();
        let mut parsed = Vec::new();

        for (action, raw) in self.entries() {
            if raw.is_empty() {
                return Err(format!("Shortcut for {action:?} cannot be empty"));
            }
            let shortcut = Shortcut::from_str(raw)
                .map_err(|err| format!("Invalid shortcut '{raw}' for {action:?}: {err}"))?;
            if !seen_ids.insert(shortcut.id()) {
                return Err(format!(
                    "Duplicate shortcut '{raw}' is already assigned to another action"
                ));
            }
            parsed.push((action, shortcut, raw));
        }

        Ok(parsed)
    }
}

fn build_lookup_map(bindings: &[ShortcutBinding<'_>]) -> HashMap<u32, ShortcutAction> {
    bindings
        .iter()
        .map(|(action, shortcut, _)| (shortcut.id(), *action))
        .collect()
}

trait ShortcutRegistrar {
    fn register(&self, shortcut: Shortcut) -> Result<(), String>;
    fn unregister(&self, shortcut: Shortcut) -> Result<(), String>;
}

struct TauriShortcutRegistrar<'a>(&'a GlobalShortcut<Wry>);

impl ShortcutRegistrar for TauriShortcutRegistrar<'_> {
    fn register(&self, shortcut: Shortcut) -> Result<(), String> {
        self.0.register(shortcut).map_err(|err| err.to_string())
    }

    fn unregister(&self, shortcut: Shortcut) -> Result<(), String> {
        self.0.unregister(shortcut).map_err(|err| err.to_string())
    }
}

struct ShortcutInner {
    config: ShortcutConfig,
    bindings_by_id: HashMap<u32, ShortcutAction>,
    paused: bool,
}

impl Default for ShortcutInner {
    fn default() -> Self {
        let config = ShortcutConfig::default();
        let bindings = config
            .parse_bindings()
            .expect("default shortcut config must be valid");
        let bindings_by_id = build_lookup_map(&bindings);
        Self {
            config,
            bindings_by_id,
            paused: false,
        }
    }
}

/// Thread-safe manager owning the active global shortcut config, O(1) dispatch table, and pause state.
pub struct ShortcutManager {
    inner: Mutex<ShortcutInner>,
}

impl Default for ShortcutManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ShortcutManager {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(ShortcutInner::default()),
        }
    }

    fn lock_inner(&self) -> std::sync::MutexGuard<'_, ShortcutInner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Returns a snapshot of the active shortcut configuration.
    pub fn config(&self) -> ShortcutConfig {
        self.lock_inner().config.clone()
    }

    /// Validates and stores `config` before OS registration (used during startup hydration).
    pub fn hydrate(&self, config: ShortcutConfig) -> Result<(), String> {
        let normalized = config.normalized();
        let bindings = normalized.parse_bindings()?;
        let mut inner = self.lock_inner();
        inner.bindings_by_id = build_lookup_map(&bindings);
        inner.config = normalized;
        Ok(())
    }

    /// Resolves a pressed shortcut ID, returning `None` when paused or unbound.
    fn lookup_action(&self, shortcut_id: u32) -> Option<ShortcutAction> {
        let inner = self.lock_inner();
        if inner.paused {
            return None;
        }
        inner.bindings_by_id.get(&shortcut_id).copied()
    }

    fn set_paused_with<R: ShortcutRegistrar>(
        &self,
        registrar: &R,
        paused: bool,
    ) -> Result<(), String> {
        let mut inner = self.lock_inner();
        if inner.paused == paused {
            return Ok(());
        }

        let bindings = inner.config.parse_bindings()?;
        if paused {
            for (_, shortcut, _) in &bindings {
                let _ = registrar.unregister(*shortcut);
            }
            inner.paused = true;
            Ok(())
        } else {
            let mut registered = Vec::with_capacity(bindings.len());
            for (action, shortcut, raw) in &bindings {
                if let Err(err) = registrar.register(*shortcut) {
                    for s in registered {
                        let _ = registrar.unregister(s);
                    }
                    return Err(format!(
                        "Failed to re-register shortcut '{raw}' for {action:?}: {err}"
                    ));
                }
                registered.push(*shortcut);
            }
            inner.paused = false;
            Ok(())
        }
    }

    fn update_with<R: ShortcutRegistrar>(
        &self,
        registrar: &R,
        next_config: ShortcutConfig,
    ) -> Result<ShortcutConfig, String> {
        let normalized = next_config.normalized();
        let next_bindings = normalized.parse_bindings()?;

        let mut inner = self.lock_inner();
        if inner.config == normalized {
            return Ok(inner.config.clone());
        }

        let prev_bindings = inner.config.parse_bindings()?;
        let was_paused = inner.paused;

        if !was_paused {
            for (_, prev_shortcut, _) in &prev_bindings {
                let _ = registrar.unregister(*prev_shortcut);
            }
        }

        let mut registered = Vec::with_capacity(next_bindings.len());
        for (action, shortcut, raw) in &next_bindings {
            if let Err(err) = registrar.register(*shortcut) {
                for s in registered {
                    let _ = registrar.unregister(s);
                }
                if !was_paused {
                    for (_, prev_shortcut, _) in &prev_bindings {
                        let _ = registrar.register(*prev_shortcut);
                    }
                }
                return Err(format!(
                    "Failed to register shortcut '{raw}' for {action:?}: {err}"
                ));
            }
            registered.push(*shortcut);
        }

        // Test-registration succeeded while paused; unregister again until unpaused.
        if was_paused {
            for s in registered {
                let _ = registrar.unregister(s);
            }
        }

        inner.bindings_by_id = build_lookup_map(&next_bindings);
        inner.config = normalized.clone();
        Ok(normalized)
    }
}

/// Builds the global shortcut plugin with a `Pressed`-only, pause-aware O(1) dispatch handler.
pub fn plugin() -> TauriPlugin<Wry> {
    Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            match app.state::<ShortcutManager>().lookup_action(shortcut.id()) {
                Some(ShortcutAction::Zoom(action)) => dispatch_zoom_action(app, action),
                Some(ShortcutAction::Pointer(action)) => {
                    pointer_controls::dispatch_shortcut(app, action)
                }
                None => {}
            }
        })
        .build()
}

/// Registers all currently configured global shortcuts, logging warnings without aborting startup.
pub fn register_shortcuts(app: &AppHandle) {
    let config = app.state::<ShortcutManager>().config();
    let global_shortcut = app.global_shortcut();
    for (_, accel) in config.entries() {
        if let Err(err) = global_shortcut.register(accel) {
            eprintln!("Warning: failed to register global shortcut '{accel}': {err}");
        }
    }
}

/// Best-effort unregistration of all global shortcuts on application shutdown.
pub fn unregister_all(app: &AppHandle) {
    if let Err(err) = app.global_shortcut().unregister_all() {
        eprintln!("Warning: failed to unregister global shortcuts on exit: {err}");
    }
}

/// Unregisters active shortcuts when `paused = true`, or re-registers them when `paused = false`.
pub fn set_shortcuts_paused(
    app: &AppHandle,
    mgr: &ShortcutManager,
    paused: bool,
) -> Result<(), String> {
    let global_shortcut = app.global_shortcut();
    let registrar = TauriShortcutRegistrar(global_shortcut);
    mgr.set_paused_with(&registrar, paused)
}

/// Validates, atomically registers (with rollback on OS failure), and stores new shortcuts.
pub fn update_shortcuts(
    app: &AppHandle,
    mgr: &ShortcutManager,
    config: ShortcutConfig,
) -> Result<ShortcutConfig, String> {
    let global_shortcut = app.global_shortcut();
    let registrar = TauriShortcutRegistrar(global_shortcut);
    mgr.update_with(&registrar, config)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    struct MockRegistrar {
        active: RefCell<HashSet<u32>>,
        fail_on: HashSet<u32>,
    }

    impl MockRegistrar {
        fn new() -> Self {
            Self {
                active: RefCell::new(HashSet::new()),
                fail_on: HashSet::new(),
            }
        }

        fn failing_on(combos: &[&str]) -> Self {
            let fail_on = combos
                .iter()
                .map(|c| Shortcut::from_str(c).expect("valid test shortcut").id())
                .collect();
            Self {
                active: RefCell::new(HashSet::new()),
                fail_on,
            }
        }

        fn seed(&self, config: &ShortcutConfig) {
            let mut active = self.active.borrow_mut();
            for (_, shortcut, _) in config.parse_bindings().expect("valid config") {
                active.insert(shortcut.id());
            }
        }

        fn is_active(&self, accel: &str) -> bool {
            let id = Shortcut::from_str(accel).expect("valid shortcut").id();
            self.active.borrow().contains(&id)
        }

        fn active_count(&self) -> usize {
            self.active.borrow().len()
        }
    }

    impl ShortcutRegistrar for MockRegistrar {
        fn register(&self, shortcut: Shortcut) -> Result<(), String> {
            if self.fail_on.contains(&shortcut.id()) {
                return Err("OS hotkey already in use".to_string());
            }
            self.active.borrow_mut().insert(shortcut.id());
            Ok(())
        }

        fn unregister(&self, shortcut: Shortcut) -> Result<(), String> {
            self.active.borrow_mut().remove(&shortcut.id());
            Ok(())
        }
    }

    #[test]
    fn default_shortcuts_parse_uniquely_and_lookup_o1() {
        let mgr = ShortcutManager::new();
        let config = mgr.config();
        let bindings = config.parse_bindings().expect("defaults parse");
        for (action, shortcut, _) in bindings {
            assert_eq!(mgr.lookup_action(shortcut.id()), Some(action));
        }

        let unbound = Shortcut::from_str("Ctrl+Alt+Shift+F12").expect("valid test shortcut");
        assert_eq!(mgr.lookup_action(unbound.id()), None);
    }

    #[test]
    fn rejects_invalid_empty_and_duplicate_shortcuts() {
        let invalid = ShortcutConfig {
            toggle: "NotARealKeyCombo".to_string(),
            ..ShortcutConfig::default()
        };
        assert!(invalid.parse_bindings().is_err());

        let empty = ShortcutConfig {
            reset: "   ".to_string(),
            ..ShortcutConfig::default()
        };
        assert!(empty.normalized().parse_bindings().is_err());

        // Case-insensitive duplicate detection via Shortcut::id()
        let duplicate = ShortcutConfig {
            toggle: "Ctrl+Alt+Z".to_string(),
            reset: "ctrl+alt+z".to_string(),
            ..ShortcutConfig::default()
        };
        assert!(duplicate.parse_bindings().is_err());
    }

    #[test]
    fn pointer_bindings_share_validation_and_registration_with_zoom_bindings() {
        let mgr = ShortcutManager::new();
        let registrar = MockRegistrar::failing_on(&["Ctrl+Alt+F10"]);
        registrar.seed(&mgr.config());
        let config = ShortcutConfig {
            pointer_size_up: Some("Ctrl+Alt+F9".to_string()),
            ..ShortcutConfig::default()
        };
        mgr.update_with(&registrar, config.clone()).unwrap();
        let id = Shortcut::from_str("Ctrl+Alt+F9").unwrap().id();
        assert_eq!(
            mgr.lookup_action(id),
            Some(ShortcutAction::Pointer(PointerAction::Increase))
        );

        let duplicate = ShortcutConfig {
            pointer_size_down: Some("Alt+Ctrl+Z".to_string()),
            ..config.clone()
        };
        assert!(duplicate.parse_bindings().is_err());
        let conflict = ShortcutConfig {
            pointer_size_down: Some("Ctrl+Alt+F10".to_string()),
            ..config.clone()
        };
        assert!(mgr.update_with(&registrar, conflict).is_err());
        assert_eq!(mgr.config(), config);
        assert!(registrar.is_active("Ctrl+Alt+F9"));
        assert!(registrar.is_active(DEFAULT_TOGGLE_SHORTCUT));

        mgr.update_with(&registrar, ShortcutConfig::default())
            .unwrap();
        assert_eq!(mgr.lookup_action(id), None);
        assert_eq!(registrar.active_count(), 4);
    }

    #[test]
    fn pause_unregisters_gates_lookup_and_resume_reregisters() {
        let mgr = ShortcutManager::new();
        let registrar = MockRegistrar::new();
        registrar.seed(&mgr.config());
        assert_eq!(registrar.active_count(), 4);

        let toggle_id = Shortcut::from_str(DEFAULT_TOGGLE_SHORTCUT)
            .expect("valid")
            .id();
        assert_eq!(
            mgr.lookup_action(toggle_id),
            Some(ShortcutAction::Zoom(ZoomAction::Toggle))
        );

        mgr.set_paused_with(&registrar, true)
            .expect("pause succeeds");
        assert_eq!(registrar.active_count(), 0);
        assert_eq!(mgr.lookup_action(toggle_id), None);

        mgr.set_paused_with(&registrar, false)
            .expect("unpause succeeds");
        assert_eq!(registrar.active_count(), 4);
        assert!(registrar.is_active(DEFAULT_TOGGLE_SHORTCUT));
        assert_eq!(
            mgr.lookup_action(toggle_id),
            Some(ShortcutAction::Zoom(ZoomAction::Toggle))
        );
    }

    #[test]
    fn update_shortcuts_rolls_back_atomically_on_os_failure() {
        let mgr = ShortcutManager::new();
        let registrar = MockRegistrar::failing_on(&["Ctrl+Shift+Down"]);
        registrar.seed(&mgr.config());

        let next = ShortcutConfig {
            toggle: "Ctrl+Shift+Z".to_string(),
            zoom_in: "Ctrl+Shift+Up".to_string(),
            zoom_out: "Ctrl+Shift+Down".to_string(),
            reset: "Ctrl+Shift+0".to_string(),
            ..ShortcutConfig::default()
        };

        let err = mgr
            .update_with(&registrar, next)
            .expect_err("should fail on third binding");
        assert!(err.contains("Ctrl+Shift+Down"));

        // Previous defaults must be restored and partial registrations cleaned up.
        assert_eq!(mgr.config(), ShortcutConfig::default());
        assert_eq!(registrar.active_count(), 4);
        assert!(registrar.is_active(DEFAULT_TOGGLE_SHORTCUT));
        assert!(registrar.is_active(DEFAULT_ZOOM_IN_SHORTCUT));
        assert!(registrar.is_active(DEFAULT_ZOOM_OUT_SHORTCUT));
        assert!(registrar.is_active(DEFAULT_RESET_SHORTCUT));
        assert!(!registrar.is_active("Ctrl+Shift+Z"));
        assert!(!registrar.is_active("Ctrl+Shift+Up"));
    }

    #[test]
    fn update_shortcuts_while_paused_validates_and_stays_unregistered() {
        let mgr = ShortcutManager::new();
        let registrar = MockRegistrar::failing_on(&["Ctrl+Shift+0"]);
        registrar.seed(&mgr.config());

        mgr.set_paused_with(&registrar, true)
            .expect("pause succeeds");
        assert_eq!(registrar.active_count(), 0);

        // Failing update while paused must leave nothing registered and config unchanged.
        let bad_next = ShortcutConfig {
            toggle: "Ctrl+Shift+Z".to_string(),
            zoom_in: "Ctrl+Shift+Up".to_string(),
            zoom_out: "Ctrl+Shift+Down".to_string(),
            reset: "Ctrl+Shift+0".to_string(),
            ..ShortcutConfig::default()
        };
        assert!(mgr.update_with(&registrar, bad_next).is_err());
        assert_eq!(registrar.active_count(), 0);
        assert_eq!(mgr.config(), ShortcutConfig::default());

        // Valid update while paused succeeds, stays unregistered, and activates on unpause.
        let good_next = ShortcutConfig {
            toggle: "Ctrl+Shift+Z".to_string(),
            zoom_in: "Ctrl+Shift+Up".to_string(),
            zoom_out: "Ctrl+Shift+Down".to_string(),
            reset: "Ctrl+Shift+9".to_string(),
            ..ShortcutConfig::default()
        };
        let saved = mgr
            .update_with(&registrar, good_next.clone())
            .expect("valid update while paused succeeds");
        assert_eq!(saved, good_next);
        assert_eq!(registrar.active_count(), 0);

        mgr.set_paused_with(&registrar, false)
            .expect("unpause registers updated shortcuts");
        assert_eq!(registrar.active_count(), 4);
        assert!(registrar.is_active("Ctrl+Shift+Z"));
        assert!(registrar.is_active("Ctrl+Shift+9"));
    }
}
