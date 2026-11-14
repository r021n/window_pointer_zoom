//! Single source of truth for zoom bounds, panning configuration, and thread-safe engine state.

use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::engine::spawn_engine;
pub use crate::engine::viewport_math::PanningMode;
use crate::engine::viewport_math::ZOOM_SNAP_EPSILON;

pub const MIN_ZOOM_SCALE: f32 = 1.0;
pub const MAX_ZOOM_SCALE: f32 = 5.0;
pub const DEFAULT_ZOOM_STEP: f32 = 0.25;
pub const DEFAULT_ZOOM_ALPHA: f32 = 0.15;
pub const DEFAULT_PAN_ALPHA: f32 = 0.25;
pub const DEFAULT_EDGE_DEADZONE: f32 = 0.15;
pub const FRAME_INTERVAL: Duration = Duration::from_millis(16);

const MIN_ZOOM_STEP: f32 = 0.05;
const MAX_ZOOM_STEP: f32 = 2.0;
const MIN_ALPHA: f32 = 0.01;
const MAX_ALPHA: f32 = 1.0;
const MIN_EDGE_DEADZONE: f32 = 0.05;
const MAX_EDGE_DEADZONE: f32 = 0.45;

fn sanitize_clamped(value: f32, min: f32, max: f32, default: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        default
    }
}

/// Allowed zoom scale range exposed to the UI.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomLimits {
    pub min_scale: f32,
    pub max_scale: f32,
}

impl Default for ZoomLimits {
    fn default() -> Self {
        Self {
            min_scale: MIN_ZOOM_SCALE,
            max_scale: MAX_ZOOM_SCALE,
        }
    }
}

/// User-configurable zoom and cursor-panning settings.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
                MIN_EDGE_DEADZONE,
                MAX_EDGE_DEADZONE,
                DEFAULT_EDGE_DEADZONE,
            ),
        }
    }
}

/// Snapshot of current zoom engine state returned by IPC commands.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomStatus {
    pub active: bool,
    pub target_scale: f32,
    pub current_scale: f32,
    pub config: PanningConfig,
    pub limits: ZoomLimits,
}

#[derive(Debug, Clone, Copy)]
struct EngineControl {
    target_scale: f32,
    current_scale: f32,
    is_animating: bool,
    config: PanningConfig,
    shutdown: bool,
}

impl Default for EngineControl {
    fn default() -> Self {
        Self {
            target_scale: MIN_ZOOM_SCALE,
            current_scale: MIN_ZOOM_SCALE,
            is_animating: false,
            config: PanningConfig::default(),
            shutdown: false,
        }
    }
}

impl EngineControl {
    fn to_status(self) -> ZoomStatus {
        ZoomStatus {
            active: self.target_scale > MIN_ZOOM_SCALE || self.is_animating,
            target_scale: self.target_scale,
            current_scale: self.current_scale,
            config: self.config,
            limits: ZoomLimits::default(),
        }
    }

    fn set_target_scale(&mut self, next_scale: f32) {
        let clamped = next_scale.clamp(MIN_ZOOM_SCALE, MAX_ZOOM_SCALE);
        self.target_scale = clamped;
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

    /// Blocks until the worker should run a frame (`Some`) or exit (`None`).
    pub fn wait_for_active_tick(&self) -> Option<(f32, PanningConfig)> {
        let mut guard = self.lock_control();
        loop {
            if guard.shutdown {
                return None;
            }
            if guard.target_scale > MIN_ZOOM_SCALE || guard.is_animating {
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
        guard.current_scale = MIN_ZOOM_SCALE;
        guard.is_animating = guard.target_scale > MIN_ZOOM_SCALE;
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
    pub fn start(&self) -> Result<(), String> {
        let mut worker_guard = self
            .worker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if worker_guard.is_some() {
            return Ok(());
        }

        self.shared.clear_shutdown();
        let handle = spawn_engine(Arc::clone(&self.shared))?;
        *worker_guard = Some(handle);
        Ok(())
    }

    /// Returns a snapshot of the current zoom status.
    pub fn status(&self) -> ZoomStatus {
        self.shared.lock_control().to_status()
    }

    /// Increases the target zoom scale by `config.zoom_step`, clamped to `[MIN_ZOOM_SCALE, MAX_ZOOM_SCALE]`.
    pub fn zoom_in(&self) -> Result<ZoomStatus, String> {
        Ok(self.update_target(|c| c.target_scale + c.config.zoom_step))
    }

    /// Decreases the target zoom scale by `config.zoom_step`, clamped to `[MIN_ZOOM_SCALE, MAX_ZOOM_SCALE]`.
    pub fn zoom_out(&self) -> Result<ZoomStatus, String> {
        Ok(self.update_target(|c| c.target_scale - c.config.zoom_step))
    }

    /// Smoothly resets target zoom scale back to `MIN_ZOOM_SCALE` (`1.0`).
    pub fn reset_zoom(&self) -> Result<ZoomStatus, String> {
        Ok(self.update_target(|_| MIN_ZOOM_SCALE))
    }

    /// Sets the target zoom scale explicitly, rejecting non-finite inputs and clamping finite values.
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
        guard.config = sanitized;
        if guard.target_scale > MIN_ZOOM_SCALE || guard.is_animating {
            self.shared.wake.notify_one();
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
        F: FnOnce(&EngineControl) -> f32,
    {
        let mut guard = self.shared.lock_control();
        let next = next_fn(&guard);
        guard.set_target_scale(next);
        if guard.target_scale > MIN_ZOOM_SCALE || guard.is_animating {
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
    fn zoom_bounds_clamping_and_step_commands() {
        let state = AppState::new();
        let initial = state.status();
        assert!(!initial.active);
        assert_eq!(initial.target_scale, MIN_ZOOM_SCALE);
        assert_eq!(initial.current_scale, MIN_ZOOM_SCALE);
        assert_eq!(
            initial.limits,
            ZoomLimits {
                min_scale: MIN_ZOOM_SCALE,
                max_scale: MAX_ZOOM_SCALE,
            }
        );

        let after_in = state.zoom_in().expect("zoom_in succeeds");
        assert!(after_in.active);
        assert_eq!(after_in.target_scale, 1.25);

        let clamped_high = state.set_zoom_level(99.0).expect("finite scale clamps");
        assert_eq!(clamped_high.target_scale, MAX_ZOOM_SCALE);

        let after_step_over_max = state.zoom_in().expect("zoom_in clamps at max");
        assert_eq!(after_step_over_max.target_scale, MAX_ZOOM_SCALE);

        let clamped_low = state.set_zoom_level(-3.0).expect("finite scale clamps");
        assert_eq!(clamped_low.target_scale, MIN_ZOOM_SCALE);

        let after_out_at_min = state.zoom_out().expect("zoom_out clamps at min");
        assert_eq!(after_out_at_min.target_scale, MIN_ZOOM_SCALE);
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
                edge_deadzone: MAX_EDGE_DEADZONE,
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
        assert_eq!(during_reset.target_scale, MIN_ZOOM_SCALE);
        assert_eq!(during_reset.current_scale, 2.0);
        assert!(during_reset.active);

        // Once the worker reports settling at identity, active transitions to false.
        assert!(!state.shared.finish_settled_and_check_shutdown());
        let settled = state.status();
        assert!(!settled.active);
        assert_eq!(settled.current_scale, MIN_ZOOM_SCALE);
        assert_eq!(settled.target_scale, MIN_ZOOM_SCALE);
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
}
