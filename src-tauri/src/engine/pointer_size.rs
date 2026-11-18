//! Session-scoped Windows cursor sizing. This does not modify the user's profile.

use std::ffi::c_void;
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};
use windows::Win32::UI::HiDpi::{SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_SYSTEM_AWARE};
use windows::Win32::UI::WindowsAndMessaging::{
    SystemParametersInfoW, SYSTEM_PARAMETERS_INFO_ACTION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
};

// Windows uses this pair for its accessibility cursor scale. These actions are not
// in the public SystemParametersInfo table. The setter takes the size by value.
const SPI_GET_POINTER_SIZE: SYSTEM_PARAMETERS_INFO_ACTION = SYSTEM_PARAMETERS_INFO_ACTION(0x2028);
const SPI_SET_POINTER_SIZE: SYSTEM_PARAMETERS_INFO_ACTION = SYSTEM_PARAMETERS_INFO_ACTION(0x2029);

const CONSTRAINTS: PointerConstraints = PointerConstraints {
    min_size: 32,
    max_size: 128,
    min_step: 1,
    max_step: 32,
    default_size: 64,
    default_step: 16,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PointerConstraints {
    pub min_size: u32,
    pub max_size: u32,
    pub min_step: u32,
    pub max_step: u32,
    pub default_size: u32,
    pub default_step: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PointerSettings {
    pub enabled: bool,
    pub default_size: u32,
    pub step: u32,
    pub click_effect_enabled: bool,
}

impl Default for PointerSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            default_size: CONSTRAINTS.default_size,
            step: CONSTRAINTS.default_step,
            click_effect_enabled: false,
        }
    }
}

impl PointerSettings {
    pub fn validated(self) -> Result<Self, String> {
        if !(CONSTRAINTS.min_size..=CONSTRAINTS.max_size).contains(&self.default_size) {
            return Err(format!(
                "Default pointer size must be between {} and {} px",
                CONSTRAINTS.min_size, CONSTRAINTS.max_size
            ));
        }
        if !(CONSTRAINTS.min_step..=CONSTRAINTS.max_step).contains(&self.step) {
            return Err(format!(
                "Pointer size step must be between {} and {} px",
                CONSTRAINTS.min_step, CONSTRAINTS.max_step
            ));
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerAction {
    Increase,
    Decrease,
    ApplyDefault,
    Reset,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PointerStatus {
    pub settings: PointerSettings,
    pub current_size: Option<u32>,
    pub baseline_size: Option<u32>,
    pub constraints: PointerConstraints,
    pub error: Option<String>,
}

trait CursorSizeApi {
    fn read(&self) -> Result<u32, String>;
    fn write(&self, size: u32) -> Result<(), String>;
}

struct WindowsCursor;

impl CursorSizeApi for WindowsCursor {
    fn read(&self) -> Result<u32, String> {
        let mut size = 0u32;
        cursor_size_parameter(SPI_GET_POINTER_SIZE, (&mut size as *mut u32).cast())
            .map_err(|err| format!("Could not read the Windows pointer size: {err}"))?;
        if size == 0 {
            return Err("Windows did not report a valid pointer size".into());
        }
        Ok(size)
    }

    fn write(&self, size: u32) -> Result<(), String> {
        cursor_size_parameter(SPI_SET_POINTER_SIZE, size as usize as *mut c_void)
            .map_err(|err| format!("Could not set the Windows pointer size to {size} px: {err}"))
    }
}

fn cursor_size_parameter(
    action: SYSTEM_PARAMETERS_INFO_ACTION,
    parameter: *mut c_void,
) -> windows::core::Result<()> {
    // SystemParametersInfoW is not DPI-aware. Keep this context change local to the call.
    let previous = unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_SYSTEM_AWARE) };
    if previous.0.is_null() {
        return Err(windows::core::Error::from_thread());
    }
    // Zero flags keep the override in the current Windows session without updating the profile.
    let result = unsafe {
        SystemParametersInfoW(
            action,
            0,
            Some(parameter),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    unsafe { SetThreadDpiAwarenessContext(previous) };
    result
}

#[derive(Clone, Copy)]
struct SizeOverride {
    baseline: u32,
    applied: u32,
}

struct PointerState {
    status: PointerStatus,
    size_override: Option<SizeOverride>,
    shutting_down: bool,
}

impl PointerState {
    fn new(api: &impl CursorSizeApi) -> Self {
        let mut state = Self {
            status: PointerStatus {
                settings: PointerSettings::default(),
                current_size: None,
                baseline_size: None,
                constraints: CONSTRAINTS,
                error: None,
            },
            size_override: None,
            shutting_down: false,
        };
        state.status.error = state.refresh(api).err();
        state
    }

    fn refresh(&mut self, api: &impl CursorSizeApi) -> Result<u32, String> {
        let current = api.read();
        self.status.current_size = current.as_ref().ok().copied();
        if let Ok(current) = current {
            // A change made outside the app becomes the new baseline.
            if self
                .size_override
                .is_some_and(|last| last.applied != current)
            {
                self.size_override = None;
            }
            self.status.baseline_size =
                Some(self.size_override.map_or(current, |last| last.baseline));
        }
        current
    }

    fn apply(&mut self, action: PointerAction, api: &impl CursorSizeApi) -> Result<(), String> {
        if self.shutting_down {
            return Err("Pointer sizing is shutting down".into());
        }
        if !self.status.settings.enabled {
            return Err("Pointer sizing is disabled".into());
        }
        let current = self.refresh(api)?;
        let settings = self.status.settings;
        let next = match action {
            PointerAction::Increase => current
                .saturating_add(settings.step)
                .clamp(CONSTRAINTS.min_size, CONSTRAINTS.max_size)
                .max(current),
            PointerAction::Decrease => current
                .saturating_sub(settings.step)
                .clamp(CONSTRAINTS.min_size, CONSTRAINTS.max_size)
                .min(current),
            PointerAction::ApplyDefault => settings.default_size,
            PointerAction::Reset => self.size_override.map_or(current, |last| last.baseline),
        };
        self.set_size(current, next, api)
    }

    fn set_size(
        &mut self,
        current: u32,
        next: u32,
        api: &impl CursorSizeApi,
    ) -> Result<(), String> {
        if next == current {
            return Ok(());
        }
        api.write(next)?;
        let baseline = self.size_override.map_or(current, |last| last.baseline);
        self.size_override = (next != baseline).then_some(SizeOverride {
            baseline,
            applied: next,
        });
        self.status.current_size = Some(next);
        Ok(())
    }

    fn restore(&mut self, api: &impl CursorSizeApi) -> Result<(), String> {
        if self.size_override.is_some() {
            let current = self.refresh(api)?;
            if let Some(last) = self.size_override {
                self.set_size(current, last.baseline, api)?;
            }
        }
        Ok(())
    }

    fn update(
        &mut self,
        settings: PointerSettings,
        api: &impl CursorSizeApi,
    ) -> Result<PointerStatus, String> {
        if self.shutting_down {
            return Err("Pointer sizing is shutting down".into());
        }
        let settings = settings.validated()?;
        if !settings.enabled {
            self.restore(api)?;
        }
        self.status.settings = settings;
        self.status.error = self.refresh(api).err();
        Ok(self.status.clone())
    }

    fn shutdown(&mut self, api: &impl CursorSizeApi) -> Result<(), String> {
        self.shutting_down = true;
        self.restore(api)
    }
}

/// Manages sizing and restoration under one lock, including cleanup from the console thread.
pub struct PointerSize(Mutex<PointerState>);

impl PointerSize {
    fn state(&self) -> MutexGuard<'_, PointerState> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn status(&self) -> PointerStatus {
        self.state().status.clone()
    }

    pub fn apply(&self, action: PointerAction) -> Result<PointerStatus, String> {
        let mut state = self.state();
        let result = state.apply(action, &WindowsCursor);
        state.status.error = result.as_ref().err().cloned();
        result.map(|()| state.status.clone())
    }

    pub fn update(&self, settings: PointerSettings) -> Result<PointerStatus, String> {
        let mut state = self.state();
        let result = state.update(settings, &WindowsCursor);
        if let Err(ref err) = result {
            state.status.error = Some(err.clone());
        }
        result
    }

    pub fn shutdown(&self) -> Result<(), String> {
        let mut state = self.state();
        let result = state.shutdown(&WindowsCursor);
        state.status.error = result.as_ref().err().cloned();
        result
    }
}

impl Default for PointerSize {
    fn default() -> Self {
        Self(Mutex::new(PointerState::new(&WindowsCursor)))
    }
}

impl Drop for PointerSize {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    struct FakeCursor {
        size: Cell<u32>,
        fail_writes: Cell<bool>,
    }

    impl FakeCursor {
        fn new(size: u32) -> Self {
            Self {
                size: Cell::new(size),
                fail_writes: Cell::new(false),
            }
        }
    }

    impl CursorSizeApi for FakeCursor {
        fn read(&self) -> Result<u32, String> {
            Ok(self.size.get())
        }

        fn write(&self, size: u32) -> Result<(), String> {
            if self.fail_writes.get() {
                return Err("cannot set pointer size".into());
            }
            self.size.set(size);
            Ok(())
        }
    }

    fn enabled_state(api: &FakeCursor) -> PointerState {
        let mut state = PointerState::new(api);
        state
            .update(
                PointerSettings {
                    enabled: true,
                    ..PointerSettings::default()
                },
                api,
            )
            .unwrap();
        state
    }

    #[test]
    fn applies_steps_and_reset_without_overwriting_the_starting_size_on_enable() {
        let api = FakeCursor::new(120);
        let mut state = enabled_state(&api);
        assert_eq!(api.size.get(), 120);

        state.apply(PointerAction::Increase, &api).unwrap();
        assert_eq!(state.status.current_size, Some(128));
        state.apply(PointerAction::Decrease, &api).unwrap();
        assert_eq!(state.status.current_size, Some(112));
        state.apply(PointerAction::ApplyDefault, &api).unwrap();
        assert_eq!(state.status.current_size, Some(64));
        state.apply(PointerAction::Reset, &api).unwrap();
        assert_eq!(api.size.get(), 120);
    }

    #[test]
    fn disabling_restores_a_baseline_above_the_app_limit() {
        let api = FakeCursor::new(160);
        let mut state = enabled_state(&api);
        state.apply(PointerAction::Increase, &api).unwrap();
        assert_eq!(api.size.get(), 160);
        state.apply(PointerAction::ApplyDefault, &api).unwrap();

        let disabled = state.update(PointerSettings::default(), &api).unwrap();
        assert_eq!(api.size.get(), 160);
        assert_eq!(disabled.current_size, Some(160));
        assert_eq!(disabled.baseline_size, Some(160));
        assert!(state.apply(PointerAction::Decrease, &api).is_err());
    }

    #[test]
    fn external_windows_size_change_takes_precedence_over_restore() {
        let api = FakeCursor::new(64);
        let mut state = enabled_state(&api);
        state.apply(PointerAction::Increase, &api).unwrap();
        api.size.set(96);

        state.shutdown(&api).unwrap();
        assert_eq!(api.size.get(), 96);
        assert_eq!(state.status.baseline_size, Some(96));
    }

    #[test]
    fn shutdown_rejects_late_shortcuts_and_retries_a_failed_restore() {
        let api = FakeCursor::new(64);
        let mut state = enabled_state(&api);
        state.apply(PointerAction::Increase, &api).unwrap();
        api.fail_writes.set(true);
        assert!(state.shutdown(&api).is_err());

        api.fail_writes.set(false);
        assert!(state.apply(PointerAction::Increase, &api).is_err());
        assert_eq!(api.size.get(), 80);
        state.shutdown(&api).unwrap();
        state.shutdown(&api).unwrap();
        assert_eq!(api.size.get(), 64);
    }
}
