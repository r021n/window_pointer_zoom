//! Win32 Fullscreen Magnification API adapter and dedicated worker thread.
//! This is the only module containing `unsafe` Win32 FFI calls.

use std::marker::PhantomData;
use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};

use windows::core::Result as WinResult;
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Magnification::{
    MagInitialize, MagSetFullscreenTransform, MagUninitialize,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN,
};

use crate::engine::viewport_math::{
    sanitize_scale, PointF, ScreenSize, StepParams, ViewportAnimator, ViewportTransform,
};
use crate::state::{SharedEngine, FRAME_INTERVAL};

/// RAII wrapper around Windows fullscreen magnification.
///
/// Uses `PhantomData<*mut ()>` (`!Send`, `!Sync`) to enforce Win32 thread affinity:
/// initialization, transform updates, and uninitialization occur on the owning thread.
pub struct FullscreenMagnifier {
    _not_send_sync: PhantomData<*mut ()>,
}

impl FullscreenMagnifier {
    /// Enables per-monitor DPI awareness (best-effort) and initializes the Magnification API.
    pub fn new() -> WinResult<Self> {
        // Best-effort: fails harmlessly if DPI awareness was already set by a manifest/caller.
        let _ =
            unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        unsafe { MagInitialize() }.ok()?;
        Ok(Self {
            _not_send_sync: PhantomData,
        })
    }

    /// Applies the given fullscreen magnification transform to DWM.
    pub fn apply(&self, transform: ViewportTransform) -> WinResult<()> {
        let safe_scale = sanitize_scale(transform.scale);
        unsafe { MagSetFullscreenTransform(safe_scale, transform.x, transform.y) }.ok()
    }

    /// Resets fullscreen magnification back to normal `1.0x` at `(0, 0)`.
    pub fn reset(&self) -> WinResult<()> {
        self.apply(ViewportTransform::IDENTITY)
    }
}

impl Drop for FullscreenMagnifier {
    fn drop(&mut self) {
        let _ = self.reset();
        let _ = unsafe { MagUninitialize() };
    }
}

/// Returns the primary monitor dimensions in physical pixels.
pub fn primary_screen_size() -> ScreenSize {
    let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    ScreenSize {
        width: width.max(0) as f32,
        height: height.max(0) as f32,
    }
}

/// Queries the current cursor position in desktop coordinates.
pub fn cursor_position() -> WinResult<PointF> {
    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point) }?;
    Ok(PointF {
        x: point.x as f32,
        y: point.y as f32,
    })
}

/// Spawns the dedicated `"zoom-engine"` thread and waits for `MagInitialize()` to succeed.
pub fn spawn_engine(shared: Arc<SharedEngine>) -> Result<JoinHandle<()>, String> {
    let (init_tx, init_rx) = mpsc::sync_channel::<Result<(), String>>(1);

    let handle = thread::Builder::new()
        .name("zoom-engine".to_string())
        .spawn(move || {
            let magnifier = match FullscreenMagnifier::new() {
                Ok(magnifier) => {
                    let _ = init_tx.send(Ok(()));
                    magnifier
                }
                Err(err) => {
                    let _ = init_tx.send(Err(format!(
                        "Failed to initialize Windows Magnification API: {err}"
                    )));
                    return;
                }
            };

            run_engine_loop(&shared, &magnifier);
        })
        .map_err(|err| format!("Failed to spawn zoom engine thread: {err}"))?;

    match init_rx.recv() {
        Ok(Ok(())) => Ok(handle),
        Ok(Err(err)) => {
            let _ = handle.join();
            Err(err)
        }
        Err(_) => {
            let _ = handle.join();
            Err("Zoom engine thread terminated before completing initialization".to_string())
        }
    }
}

fn run_engine_loop(shared: &SharedEngine, magnifier: &FullscreenMagnifier) {
    let mut animator = ViewportAnimator::new();
    let mut last_cursor = cursor_position().unwrap_or(PointF::ZERO);

    while let Some((target_scale, config)) = shared.wait_for_active_tick() {
        let screen = primary_screen_size();
        if let Ok(pos) = cursor_position() {
            last_cursor = pos;
        }

        let params = StepParams {
            target_scale,
            zoom_alpha: config.zoom_alpha,
            pan_alpha: config.pan_alpha,
            mode: config.mode,
            edge_deadzone: config.edge_deadzone,
        };
        let transform = animator.step(last_cursor, screen, params);

        if animator.is_settled_at_identity(target_scale) {
            match magnifier.reset() {
                Ok(()) => {
                    if shared.finish_settled_and_check_shutdown() {
                        break;
                    }
                }
                Err(err) => {
                    eprintln!("[zoom-engine] failed to reset magnifier: {err}");
                    if shared.report_frame_and_wait(transform.scale, FRAME_INTERVAL) {
                        break;
                    }
                }
            }
        } else {
            if let Err(err) = magnifier.apply(transform) {
                eprintln!("[zoom-engine] failed to apply fullscreen transform: {err}");
            }
            if shared.report_frame_and_wait(transform.scale, FRAME_INTERVAL) {
                break;
            }
        }
    }
}
