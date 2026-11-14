//! Single source of truth for zoom bounds, panning configuration, and thread-safe engine state.

use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::engine::spawn_engine;
pub use crate::engine::viewport_math::PanningMode;
use crate::engine::viewport_math::{
    MAX_EDGE_DEADZONE_RATIO, MIN_EDGE_DEADZONE_RATIO, ZOOM_SNAP_EPSILON,
};
use crate::shortcuts::ShortcutConfig;

pub const UNMAGNIFIED_SCALE: f32 = 1.0;
pub const MIN_SCALE_ENVELOPE: f32 = 1.0;
pub const MAX_SCALE_ENVELOPE: f32 = 10.0;
pub const DEFAULT_MIN_ZOOM_SCALE: f32 = 1.0;
pub const DEFAULT_TOGGLE_SCALE: f32 = 2.0;
pub const DEFAULT_MAX_ZOOM_SCALE: f32 = 5.0;
pub const DEFAULT_ZOOM_STEP: f32 = 0.25;
pub const DEFAULT_ZOOM_ALPHA: f32 = 0.15;
pub const DEFAULT_PAN_ALPHA: f32 = 0.25;
pub const DEFAULT_EDGE_DEADZONE: f32 = 0.15;
pub const FRAME_INTERVAL: Duration = Duration::from_millis(16);

const MIN_ZOOM_STEP: f32 = 0.05;
const MAX_ZOOM_STEP: f32 = 2.0;
const MIN_ALPHA: f32 = 0.01;
const MAX_ALPHA: f32 = 1.0;

fn sanitize_clamped(value: f32, min: f32, max: f32, default: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        default
    }
}

/// Allowed zoom scale range and default toggle target exposed to the UI.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ZoomLimits {
    pub min_scale: f32,
    pub default_scale: f32,
    pub max_scale: f32,
}

impl Default for ZoomLimits {
    fn default() -> Self {
        Self {
            min_scale: DEFAULT_MIN_ZOOM_SCALE,
            default_scale: DEFAULT_TOGGLE_SCALE,
            max_scale: DEFAULT_MAX_ZOOM_SCALE,
        }
    }
}

impl ZoomLimits {
    /// Enforces finite bounds and `1.0 <= min_scale <= default_scale <= max_scale <= 10.0`.
    pub fn sanitized(self) -> Self {
        let mut min_scale = sanitize_clamped(
            self.min_scale,
            MIN_SCALE_ENVELOPE,
            MAX_SCALE_ENVELOPE,
            DEFAULT_MIN_ZOOM_SCALE,
        );
        let mut max_scale = sanitize_clamped(
            self.max_scale,
            MIN_SCALE_ENVELOPE,
            MAX_SCALE_ENVELOPE,
            DEFAULT_MAX_ZOOM_SCALE,
        );
        if min_scale > max_scale {
            std::mem::swap(&mut min_scale, &mut max_scale);
        }
        let default_raw = if self.default_scale.is_finite() {
            self.default_scale
        } else {
            DEFAULT_TOGGLE_SCALE
        };
        let default_scale = default_raw.clamp(min_scale, max_scale);
        Self {
            min_scale,
            default_scale,
            max_scale,
        }
    }
}

/// User-configurable zoom and cursor-panning settings.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PanningConfig {
    pub mode: PanningMode,
    pub zoom_step: f32,
    pub zoom_alpha: f32,
    pub pan_alpha: f32,
    pub edge_deadzone: f32,
}

impl Default for PanningConfig {
    fn default() -> Self {
        Self {
            mode: PanningMode::default(),
            zoom_step: DEFAULT_ZOOM_STEP,
            zoom_alpha: DEFAULT_ZOOM_ALPHA,
            pan_alpha: DEFAULT_PAN_ALPHA,
            edge_deadzone: DEFAULT_EDGE_DEADZONE,
        }
    }
}

impl PanningConfig {
    /// Returns a copy with all numeric parameters finite and clamped to safe ranges.
    pub fn sanitized(self) -> Self {
        Self {
            mode: self.mode,
            zoom_step: sanitize_clamped(
                self.zoom_step,
                MIN_ZOOM_STEP,
                MAX_ZOOM_STEP,
                DEFAULT_ZOOM_STEP,
            ),
            zoom_alpha: sanitize_clamped(self.zoom_alpha, MIN_ALPHA, MAX_ALPHA, DEFAULT_ZOOM_ALPHA),
            pan_alpha: sanitize_clamped(self.pan_alpha, MIN_ALPHA, MAX_ALPHA, DEFAULT_PAN_ALPHA),
            edge_deadzone: sanitize_clamped(
                self.edge_deadzone,
                MIN_EDGE_DEADZONE_RATIO,
                MAX_EDGE_DEADZONE_RATIO,
                DEFAULT_EDGE_DEADZONE,
            ),
        }
    }
}

/// Persisted `"engineSettings"` payload stored in `settings.json`.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PersistedEngineSettings {
    pub limits: ZoomLimits,
    pub panning: PanningConfig,
}

/// Numeric slider bounds and step increments shared with the frontend.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RangeBounds {
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

/// Canonical limits and default presets exposed so the UI never duplicates backend bounds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigConstraints {
    pub scale_envelope: RangeBounds,
    pub zoom_step: RangeBounds,
    pub alpha: RangeBounds,
    pub edge_deadzone: RangeBounds,
    pub default_limits: ZoomLimits,
    pub default_panning: PanningConfig,
    pub default_shortcuts: ShortcutConfig,
}

/// Returns the canonical constraints and defaults for engine and shortcut configuration.
pub fn config_constraints() -> ConfigConstraints {
    ConfigConstraints {
        scale_envelope: RangeBounds {
            min: MIN_SCALE_ENVELOPE,
            max: MAX_SCALE_ENVELOPE,
            step: 0.1,
        },
        zoom_step: RangeBounds {
            min: MIN_ZOOM_STEP,
            max: MAX_ZOOM_STEP,
            step: 0.05,
        },
        alpha: RangeBounds {
            min: MIN_ALPHA,
            max: MAX_ALPHA,
            step: 0.01,
        },
        edge_deadzone: RangeBounds {
            min: MIN_EDGE_DEADZONE_RATIO,
            max: MAX_EDGE_DEADZONE_RATIO,
            step: 0.01,
        },
        default_limits: ZoomLimits::default(),
        default_panning: PanningConfig::default(),
        default_shortcuts: ShortcutConfig::default(),
    }
}

/// Snapshot of current zoom engine state returned by IPC commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputMappingState {
    #[default]
    Pending,
    Active,
    Denied,
    Error,
    Disabled,
}

/// Runtime health, independent from whether visual magnification succeeded.
/// Error codes are signed HRESULT values; a missing token result is unknown, not false.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputMappingStatus {
    pub state: InputMappingState,
    pub ui_access: Option<bool>,
    pub error_code: Option<i32>,
    pub ui_access_error_code: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomStatus {
    pub input_mapping: InputMappingStatus,
    pub active: bool,
    pub target_scale: f32,
    pub current_scale: f32,
    pub config: PanningConfig,
    pub limits: ZoomLimits,
}

/// Combined hydration snapshot fetched once by the frontend on mount.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineSettingsSnapshot {
    pub status: ZoomStatus,
    pub shortcuts: ShortcutConfig,
    pub constraints: ConfigConstraints,
}

/// High-level zoom action shared across shortcuts, tray menu, and wheel triggers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomAction {
    Toggle,
    ZoomIn,
    ZoomOut,
    Reset,
}

#[derive(Debug, Clone, Copy)]
struct EngineControl {
    input_mapping: InputMappingStatus,
    target_scale: f32,
    current_scale: f32,
    last_toggle_scale: f32,
    is_animating: bool,
    config: PanningConfig,
    limits: ZoomLimits,
    shutdown: bool,
}

impl Default for EngineControl {
    fn default() -> Self {
        let limits = ZoomLimits::default();
        Self {
            input_mapping: InputMappingStatus::default(),
            target_scale: UNMAGNIFIED_SCALE,
            current_scale: UNMAGNIFIED_SCALE,
            last_toggle_scale: limits.default_scale,
            is_animating: false,
            config: PanningConfig::default(),
            limits,
            shutdown: false,
        }
    }
}

impl EngineControl {
    fn to_status(self) -> ZoomStatus {
        ZoomStatus {
            input_mapping: self.input_mapping,
            active: self.target_scale > UNMAGNIFIED_SCALE || self.is_animating,
            target_scale: self.target_scale,
            current_scale: self.current_scale,
            config: self.config,
            limits: self.limits,
        }
    }

    fn set_target_scale(&mut self, next_scale: f32) {
        let clamped = if !next_scale.is_finite() || next_scale <= UNMAGNIFIED_SCALE {
            UNMAGNIFIED_SCALE
        } else {
            next_scale.clamp(self.limits.min_scale, self.limits.max_scale)
        };
        self.target_scale = clamped;
        if clamped > UNMAGNIFIED_SCALE && self.input_mapping.state == InputMappingState::Disabled {
            self.input_mapping.state = InputMappingState::Pending;
        }
        if (clamped - self.current_scale).abs() >= ZOOM_SNAP_EPSILON {
            self.is_animating = true;
        }
    }
}

/// Shared synchronization primitive coordinating commands and the background zoom worker.
pub struct SharedEngine {
    control: Mutex<EngineControl>,
    wake: Condvar,
}

/// Callback invoked when `input_mapping` transitions in `SharedEngine`.
pub type StatusChangeCallback = Arc<dyn Fn(ZoomStatus) + Send + Sync>;

impl Default for SharedEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedEngine {
    pub fn new() -> Self {
        Self {
            control: Mutex::new(EngineControl::default()),
            wake: Condvar::new(),
        }
    }

    fn lock_control(&self) -> MutexGuard<'_, EngineControl> {
        self.control
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Called by the worker after initialization and every transform attempt.
    /// Returns the updated `ZoomStatus` only when `input_mapping` changed.
    pub fn report_input_mapping(&self, status: InputMappingStatus) -> Option<ZoomStatus> {
        let mut guard = self.lock_control();
        if guard.input_mapping == status {
            return None;
        }
        guard.input_mapping = status;
        Some(guard.to_status())
    }

    /// Blocks until the worker should run a frame (`Some`) or exit (`None`).
    pub fn wait_for_active_tick(&self) -> Option<(f32, PanningConfig)> {
        let mut guard = self.lock_control();
        loop {
            if guard.shutdown {
                return None;
            }
            if guard.target_scale > UNMAGNIFIED_SCALE || guard.is_animating {
                return Some((guard.target_scale, guard.config));
            }
            guard = self
                .wake
                .wait(guard)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
    }

    /// Marks the engine settled at `1.0x` unless a new zoom target arrived during reset.
    /// Returns `true` if shutdown was requested.
    pub fn finish_settled_and_check_shutdown(&self) -> bool {
        let mut guard = self.lock_control();
        guard.current_scale = UNMAGNIFIED_SCALE;
        guard.is_animating = guard.target_scale > UNMAGNIFIED_SCALE;
        guard.shutdown
    }

    /// Records the current animated scale and waits up to `interval` for the next tick or wakeup.
    /// Returns `true` if shutdown was requested before or during the wait.
    pub fn report_frame_and_wait(&self, current_scale: f32, interval: Duration) -> bool {
        let mut guard = self.lock_control();
        guard.current_scale = current_scale;
        guard.is_animating = true;
        if guard.shutdown {
            return true;
        }
        let (guard, _) = self
            .wake
            .wait_timeout(guard, interval)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.shutdown
    }

    fn clear_shutdown(&self) {
        let mut guard = self.lock_control();
        guard.shutdown = false;
    }

    fn request_shutdown(&self) {
        let mut guard = self.lock_control();
        guard.shutdown = true;
        self.wake.notify_one();
    }
}

/// Tauri-managed controller owning shared engine state and the worker thread lifecycle.
pub struct AppState {
    shared: Arc<SharedEngine>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            shared: Arc::new(SharedEngine::new()),
            worker: Mutex::new(None),
        }
    }

    /// Spawns the background zoom engine thread if it is not already running.
    pub fn start(&self, on_status_change: Option<StatusChangeCallback>) -> Result<(), String> {
        let mut worker_guard = self
            .worker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if worker_guard.is_some() {
            return Ok(());
        }

        self.shared.clear_shutdown();
        let handle = spawn_engine(Arc::clone(&self.shared), on_status_change)?;
        *worker_guard = Some(handle);
        Ok(())
    }

    /// Returns a snapshot of the current zoom status.
    pub fn status(&self) -> ZoomStatus {
        self.shared.lock_control().to_status()
    }

    /// Increases the target zoom scale by `config.zoom_step`, clamping into `[limits.min_scale, limits.max_scale]`.
    pub fn zoom_in(&self) -> Result<ZoomStatus, String> {
        Ok(self.update_target(|c| {
            let base = if c.target_scale <= UNMAGNIFIED_SCALE {
                UNMAGNIFIED_SCALE
            } else {
                c.target_scale
            };
            (base + c.config.zoom_step).clamp(c.limits.min_scale, c.limits.max_scale)
        }))
    }

    /// Decreases the target zoom scale by `config.zoom_step`, returning to `1.0` when stepping below `limits.min_scale`.
    pub fn zoom_out(&self) -> Result<ZoomStatus, String> {
        Ok(self.update_target(|c| {
            if c.target_scale <= UNMAGNIFIED_SCALE {
                return UNMAGNIFIED_SCALE;
            }
            let next = c.target_scale - c.config.zoom_step;
            if next < c.limits.min_scale - ZOOM_SNAP_EPSILON {
                UNMAGNIFIED_SCALE
            } else {
                next.clamp(c.limits.min_scale, c.limits.max_scale)
            }
        }))
    }

    /// Smoothly resets target zoom scale back to `UNMAGNIFIED_SCALE` (`1.0`).
    pub fn reset_zoom(&self) -> Result<ZoomStatus, String> {
        Ok(self.update_target(|_| UNMAGNIFIED_SCALE))
    }

    /// Toggles between `UNMAGNIFIED_SCALE` (`1.0`) and `last_toggle_scale` within active limits.
    pub fn toggle_zoom(&self) -> Result<ZoomStatus, String> {
        Ok(self.update_target(|c| {
            if c.target_scale > UNMAGNIFIED_SCALE {
                c.last_toggle_scale = c.target_scale.clamp(c.limits.min_scale, c.limits.max_scale);
                UNMAGNIFIED_SCALE
            } else if c.last_toggle_scale > UNMAGNIFIED_SCALE {
                c.last_toggle_scale
                    .clamp(c.limits.min_scale, c.limits.max_scale)
            } else {
                c.limits
                    .default_scale
                    .clamp(c.limits.min_scale, c.limits.max_scale)
            }
        }))
    }

    /// Dispatches a high-level `ZoomAction` to the corresponding state mutation.
    pub fn apply_action(&self, action: ZoomAction) -> Result<ZoomStatus, String> {
        match action {
            ZoomAction::Toggle => self.toggle_zoom(),
            ZoomAction::ZoomIn => self.zoom_in(),
            ZoomAction::ZoomOut => self.zoom_out(),
            ZoomAction::Reset => self.reset_zoom(),
        }
    }

    /// Sets the target zoom scale explicitly (`<= 1.0` resets to `1.0`; `> 1.0` clamps to active limits).
    pub fn set_zoom_level(&self, scale: f32) -> Result<ZoomStatus, String> {
        if !scale.is_finite() {
            return Err(format!("Zoom scale must be a finite number, got {scale}"));
        }
        Ok(self.update_target(|_| scale))
    }

    /// Validates, sanitizes, and stores a new `PanningConfig`.
    pub fn update_panning_config(&self, config: PanningConfig) -> Result<ZoomStatus, String> {
        let sanitized = config.sanitized();
        let mut guard = self.shared.lock_control();
        if guard.shutdown {
            return Ok(guard.to_status());
        }
        guard.config = sanitized;
        if guard.target_scale > UNMAGNIFIED_SCALE || guard.is_animating {
            self.shared.wake.notify_one();
        }
        Ok(guard.to_status())
    }

    /// Sanitizes and updates `ZoomLimits`, re-clamping active target and toggle memory as needed.
    pub fn update_zoom_limits(&self, limits: ZoomLimits) -> Result<ZoomStatus, String> {
        let sanitized = limits.sanitized();
        let mut guard = self.shared.lock_control();
        if guard.shutdown {
            return Ok(guard.to_status());
        }
        let default_changed =
            (sanitized.default_scale - guard.limits.default_scale).abs() > ZOOM_SNAP_EPSILON;
        guard.limits = sanitized;
        if default_changed {
            guard.last_toggle_scale = sanitized.default_scale;
        } else {
            guard.last_toggle_scale = guard
                .last_toggle_scale
                .clamp(sanitized.min_scale, sanitized.max_scale);
        }

        if guard.target_scale > UNMAGNIFIED_SCALE {
            let prev_target = guard.target_scale;
            let reclamped = prev_target.clamp(sanitized.min_scale, sanitized.max_scale);
            if (reclamped - prev_target).abs() > ZOOM_SNAP_EPSILON {
                guard.set_target_scale(reclamped);
                self.shared.wake.notify_one();
            } else {
                guard.target_scale = reclamped;
            }
        }
        Ok(guard.to_status())
    }

    /// Signals the worker thread to stop and joins it. Safe to call multiple times.
    pub fn shutdown(&self) {
        self.shared.request_shutdown();
        let handle = self
            .worker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(handle) = handle {
            let _ = handle.join();
        }
    }

    fn update_target<F>(&self, next_fn: F) -> ZoomStatus
    where
        F: FnOnce(&mut EngineControl) -> f32,
    {
        let mut guard = self.shared.lock_control();
        if guard.shutdown {
            return guard.to_status();
        }
        let next = next_fn(&mut guard);
        guard.set_target_scale(next);
        if guard.target_scale > UNMAGNIFIED_SCALE || guard.is_animating {
            self.shared.wake.notify_one();
        }
        guard.to_status()
    }
}

impl Drop for AppState {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_mapping_health_reaches_ipc_and_target_changes_cannot_mask_failure() {
        let state = AppState::new();
        assert_eq!(
            state.status().input_mapping.state,
            InputMappingState::Pending
        );
        let denied = InputMappingStatus {
            state: InputMappingState::Denied,
            ui_access: Some(false),
            error_code: Some(0x80070005u32 as i32),
            ui_access_error_code: None,
        };
        state.shared.report_input_mapping(denied);
        assert_eq!(state.set_zoom_level(2.5).unwrap().input_mapping, denied);
        assert_eq!(state.reset_zoom().unwrap().input_mapping, denied);
        let serialized = serde_json::to_value(state.status()).unwrap();
        assert_eq!(serialized["inputMapping"]["state"], "denied");
        assert_eq!(serialized["inputMapping"]["uiAccess"], false);
        assert_eq!(serialized["inputMapping"]["errorCode"], -2147024891i32);

        state.shared.report_input_mapping(InputMappingStatus {
            state: InputMappingState::Disabled,
            ..denied
        });
        assert_eq!(
            state.set_zoom_level(2.0).unwrap().input_mapping.state,
            InputMappingState::Pending
        );
    }

    #[test]
    fn report_input_mapping_returns_some_only_on_change() {
        let shared = SharedEngine::new();
        assert_eq!(
            shared.report_input_mapping(InputMappingStatus::default()),
            None
        );

        let disabled = InputMappingStatus {
            state: InputMappingState::Disabled,
            ui_access: Some(true),
            error_code: None,
            ui_access_error_code: None,
        };
        let first = shared
            .report_input_mapping(disabled)
            .expect("Pending -> Disabled should emit");
        assert_eq!(first.input_mapping, disabled);
        assert_eq!(shared.report_input_mapping(disabled), None);

        let active = InputMappingStatus {
            state: InputMappingState::Active,
            ..disabled
        };
        let second = shared
            .report_input_mapping(active)
            .expect("Disabled -> Active should emit");
        assert_eq!(second.input_mapping, active);
        assert_eq!(shared.report_input_mapping(active), None);
    }

    #[test]
    fn zoom_bounds_clamping_and_step_commands() {
        let state = AppState::new();
        let initial = state.status();
        assert!(!initial.active);
        assert_eq!(initial.target_scale, UNMAGNIFIED_SCALE);
        assert_eq!(initial.current_scale, UNMAGNIFIED_SCALE);
        assert_eq!(
            initial.limits,
            ZoomLimits {
                min_scale: DEFAULT_MIN_ZOOM_SCALE,
                default_scale: DEFAULT_TOGGLE_SCALE,
                max_scale: DEFAULT_MAX_ZOOM_SCALE,
            }
        );

        let after_in = state.zoom_in().expect("zoom_in succeeds");
        assert!(after_in.active);
        assert_eq!(after_in.target_scale, 1.25);

        let clamped_high = state.set_zoom_level(99.0).expect("finite scale clamps");
        assert_eq!(clamped_high.target_scale, DEFAULT_MAX_ZOOM_SCALE);

        let after_step_over_max = state.zoom_in().expect("zoom_in clamps at max");
        assert_eq!(after_step_over_max.target_scale, DEFAULT_MAX_ZOOM_SCALE);

        let clamped_low = state.set_zoom_level(-3.0).expect("finite scale clamps");
        assert_eq!(clamped_low.target_scale, UNMAGNIFIED_SCALE);

        let after_out_at_min = state.zoom_out().expect("zoom_out clamps at min");
        assert_eq!(after_out_at_min.target_scale, UNMAGNIFIED_SCALE);
    }

    #[test]
    fn zoom_limits_sanitized_enforces_ordering_and_envelope() {
        let inverted = ZoomLimits {
            min_scale: 8.0,
            default_scale: 1.5,
            max_scale: 3.0,
        };
        assert_eq!(
            inverted.sanitized(),
            ZoomLimits {
                min_scale: 3.0,
                default_scale: 3.0,
                max_scale: 8.0,
            }
        );

        let out_of_envelope = ZoomLimits {
            min_scale: 0.2,
            default_scale: 99.0,
            max_scale: 42.0,
        };
        assert_eq!(
            out_of_envelope.sanitized(),
            ZoomLimits {
                min_scale: MIN_SCALE_ENVELOPE,
                default_scale: MAX_SCALE_ENVELOPE,
                max_scale: MAX_SCALE_ENVELOPE,
            }
        );

        let non_finite = ZoomLimits {
            min_scale: f32::NAN,
            default_scale: f32::INFINITY,
            max_scale: f32::NEG_INFINITY,
        };
        assert_eq!(
            non_finite.sanitized(),
            ZoomLimits {
                min_scale: DEFAULT_MIN_ZOOM_SCALE,
                default_scale: DEFAULT_TOGGLE_SCALE,
                max_scale: DEFAULT_MAX_ZOOM_SCALE,
            }
        );
    }

    #[test]
    fn elevated_min_scale_handles_step_set_reset_toggle_and_reclamp() {
        let state = AppState::new();
        let limits = ZoomLimits {
            min_scale: 1.5,
            default_scale: 2.5,
            max_scale: 4.0,
        };
        let updated = state
            .update_zoom_limits(limits)
            .expect("update_zoom_limits succeeds");
        assert_eq!(updated.limits, limits);
        assert_eq!(updated.target_scale, UNMAGNIFIED_SCALE);

        // zoom_out when unmagnified stays at 1.0 (never zooms IN to min_scale).
        let out_when_idle = state.zoom_out().expect("zoom_out succeeds");
        assert_eq!(out_when_idle.target_scale, UNMAGNIFIED_SCALE);

        // zoom_in from 1.0 jumps into [min_scale, max_scale].
        let in_from_idle = state.zoom_in().expect("zoom_in succeeds");
        assert_eq!(in_from_idle.target_scale, 1.5);

        // zoom_out from min_scale steps cleanly back to 1.0.
        let back_to_idle = state.zoom_out().expect("zoom_out succeeds");
        assert_eq!(back_to_idle.target_scale, UNMAGNIFIED_SCALE);

        // toggle_zoom uses updated default_scale (2.5), then back to 1.0.
        let toggled_on = state.toggle_zoom().expect("toggle on succeeds");
        assert_eq!(toggled_on.target_scale, 2.5);
        let toggled_off = state.toggle_zoom().expect("toggle off succeeds");
        assert_eq!(toggled_off.target_scale, UNMAGNIFIED_SCALE);

        // set_zoom_level clamps > 1.0 into [1.5, 4.0] and <= 1.0 to 1.0.
        assert_eq!(
            state
                .set_zoom_level(1.2)
                .expect("set_zoom_level")
                .target_scale,
            1.5
        );
        assert_eq!(
            state
                .set_zoom_level(6.0)
                .expect("set_zoom_level")
                .target_scale,
            4.0
        );
        assert_eq!(
            state
                .set_zoom_level(1.0)
                .expect("set_zoom_level")
                .target_scale,
            UNMAGNIFIED_SCALE
        );

        // Updating limits while magnified re-clamps active target_scale.
        state.set_zoom_level(3.5).expect("zoom to 3.5");
        let reclamped = state
            .update_zoom_limits(ZoomLimits {
                min_scale: 1.5,
                default_scale: 2.0,
                max_scale: 3.0,
            })
            .expect("reclamp succeeds");
        assert_eq!(reclamped.target_scale, 3.0);

        // reset_zoom always goes to 1.0 even when min_scale > 1.0.
        let reset = state.reset_zoom().expect("reset succeeds");
        assert_eq!(reset.target_scale, UNMAGNIFIED_SCALE);
    }

    #[test]
    fn set_zoom_level_rejects_non_finite_values() {
        let state = AppState::new();
        state.set_zoom_level(2.5).expect("valid scale sets target");

        assert!(state.set_zoom_level(f32::NAN).is_err());
        assert!(state.set_zoom_level(f32::INFINITY).is_err());
        assert!(state.set_zoom_level(f32::NEG_INFINITY).is_err());

        // Previous valid target remains intact after rejected calls.
        assert_eq!(state.status().target_scale, 2.5);
    }

    #[test]
    fn panning_config_sanitized_clamps_and_replaces_non_finite() {
        let dirty = PanningConfig {
            mode: PanningMode::EdgePanning,
            zoom_step: 10.0,
            zoom_alpha: -1.0,
            pan_alpha: f32::NAN,
            edge_deadzone: 0.9,
        };
        assert_eq!(
            dirty.sanitized(),
            PanningConfig {
                mode: PanningMode::EdgePanning,
                zoom_step: MAX_ZOOM_STEP,
                zoom_alpha: MIN_ALPHA,
                pan_alpha: DEFAULT_PAN_ALPHA,
                edge_deadzone: MAX_EDGE_DEADZONE_RATIO,
            }
        );

        let non_finite = PanningConfig {
            mode: PanningMode::CenterFollow,
            zoom_step: f32::INFINITY,
            zoom_alpha: f32::NEG_INFINITY,
            pan_alpha: 5.0,
            edge_deadzone: f32::NAN,
        };
        assert_eq!(
            non_finite.sanitized(),
            PanningConfig {
                mode: PanningMode::CenterFollow,
                zoom_step: DEFAULT_ZOOM_STEP,
                zoom_alpha: DEFAULT_ZOOM_ALPHA,
                pan_alpha: MAX_ALPHA,
                edge_deadzone: DEFAULT_EDGE_DEADZONE,
            }
        );
    }

    #[test]
    fn zoom_status_remains_active_during_zoom_out_animation() {
        let state = AppState::new();

        // Zoom to 2.0x and simulate worker advancing current_scale to 2.0x.
        let s1 = state.set_zoom_level(2.0).expect("set_zoom_level succeeds");
        assert!(s1.active);
        assert!(!state.shared.report_frame_and_wait(2.0, Duration::ZERO));

        // Reset zoom while current_scale is still 2.0x: target becomes 1.0x, active stays true.
        let during_reset = state.reset_zoom().expect("reset_zoom succeeds");
        assert_eq!(during_reset.target_scale, UNMAGNIFIED_SCALE);
        assert_eq!(during_reset.current_scale, 2.0);
        assert!(during_reset.active);

        // Once the worker reports settling at identity, active transitions to false.
        assert!(!state.shared.finish_settled_and_check_shutdown());
        let settled = state.status();
        assert!(!settled.active);
        assert_eq!(settled.current_scale, UNMAGNIFIED_SCALE);
        assert_eq!(settled.target_scale, UNMAGNIFIED_SCALE);
    }

    #[test]
    fn shutdown_is_idempotent_and_wakes_worker() {
        let state = AppState::new();
        let shared = Arc::clone(&state.shared);
        let handle = std::thread::spawn(move || {
            while shared.wait_for_active_tick().is_some() {
                if shared.report_frame_and_wait(1.0, FRAME_INTERVAL) {
                    break;
                }
            }
        });
        *state.worker.lock().expect("lock worker") = Some(handle);

        state.shutdown();
        state.shutdown();
        assert!(state.worker.lock().expect("lock worker").is_none());
    }

    #[test]
    fn toggle_zoom_switches_between_min_and_last_scale() {
        let state = AppState::new();
        assert_eq!(state.status().target_scale, UNMAGNIFIED_SCALE);

        let zoomed = state.toggle_zoom().expect("toggle on succeeds");
        assert_eq!(zoomed.target_scale, DEFAULT_TOGGLE_SCALE);
        assert!(zoomed.active);

        let unzoomed = state.toggle_zoom().expect("toggle off succeeds");
        assert_eq!(unzoomed.target_scale, UNMAGNIFIED_SCALE);

        state.set_zoom_level(2.75).expect("custom scale succeeds");
        let saved_off = state.toggle_zoom().expect("toggle saves custom scale");
        assert_eq!(saved_off.target_scale, UNMAGNIFIED_SCALE);

        let restored = state.toggle_zoom().expect("toggle restores custom scale");
        assert_eq!(restored.target_scale, 2.75);
    }

    #[test]
    fn toggle_zoom_falls_back_to_default_if_last_scale_not_above_min() {
        let state = AppState::new();
        state.shared.lock_control().last_toggle_scale = UNMAGNIFIED_SCALE;
        let zoomed = state.toggle_zoom().expect("toggle falls back to default");
        assert_eq!(zoomed.target_scale, DEFAULT_TOGGLE_SCALE);
    }

    #[test]
    fn apply_action_dispatches_all_variants() {
        let state = AppState::new();

        let toggled = state
            .apply_action(ZoomAction::Toggle)
            .expect("toggle action succeeds");
        assert_eq!(toggled.target_scale, DEFAULT_TOGGLE_SCALE);

        let zoomed_in = state
            .apply_action(ZoomAction::ZoomIn)
            .expect("zoom_in action succeeds");
        assert_eq!(
            zoomed_in.target_scale,
            DEFAULT_TOGGLE_SCALE + DEFAULT_ZOOM_STEP
        );

        let zoomed_out = state
            .apply_action(ZoomAction::ZoomOut)
            .expect("zoom_out action succeeds");
        assert_eq!(zoomed_out.target_scale, DEFAULT_TOGGLE_SCALE);

        let reset = state
            .apply_action(ZoomAction::Reset)
            .expect("reset action succeeds");
        assert_eq!(reset.target_scale, UNMAGNIFIED_SCALE);
    }

    #[test]
    fn mutations_after_shutdown_are_ignored_and_do_not_rearm_animation() {
        let state = AppState::new();
        state.shutdown();

        let status = state.zoom_in().expect("zoom_in returns status");
        assert!(!status.active);
        assert_eq!(status.target_scale, UNMAGNIFIED_SCALE);

        let status = state.zoom_out().expect("zoom_out returns status");
        assert!(!status.active);
        assert_eq!(status.target_scale, UNMAGNIFIED_SCALE);

        let status = state
            .set_zoom_level(3.0)
            .expect("set_zoom_level returns status");
        assert!(!status.active);
        assert_eq!(status.target_scale, UNMAGNIFIED_SCALE);

        let status = state.toggle_zoom().expect("toggle_zoom returns status");
        assert!(!status.active);
        assert_eq!(status.target_scale, UNMAGNIFIED_SCALE);

        let new_panning = PanningConfig {
            mode: PanningMode::EdgePanning,
            zoom_step: 1.0,
            zoom_alpha: 0.5,
            pan_alpha: 0.5,
            edge_deadzone: 0.25,
        };
        let status = state
            .update_panning_config(new_panning)
            .expect("update_panning_config returns status");
        assert!(!status.active);
        assert_eq!(status.config, PanningConfig::default());

        let new_limits = ZoomLimits {
            min_scale: 1.5,
            default_scale: 3.0,
            max_scale: 8.0,
        };
        let status = state
            .update_zoom_limits(new_limits)
            .expect("update_zoom_limits returns status");
        assert!(!status.active);
        assert_eq!(status.limits, ZoomLimits::default());
        assert_eq!(status.target_scale, UNMAGNIFIED_SCALE);
    }
}
