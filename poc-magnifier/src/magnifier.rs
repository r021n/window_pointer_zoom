//! Win32 Fullscreen Magnification API adapter and RAII guard.
//! This is the only module containing `unsafe` Win32 FFI calls.

use std::marker::PhantomData;

use windows::core::Result;
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

use crate::math::{PointF, ScreenSize, ViewportTransform};

/// RAII wrapper around Windows fullscreen magnification.
///
/// Uses `PhantomData<*mut ()>` (`!Send`, `!Sync`) to enforce Win32 thread affinity:
/// initialization, transform updates, and uninitialization occur on the owning thread.
pub struct FullscreenMagnifier {
    _not_send_sync: PhantomData<*mut ()>,
}

impl FullscreenMagnifier {
    /// Enables per-monitor DPI awareness (best-effort) and initializes the Magnification API.
    pub fn new() -> Result<Self> {
        // Best-effort: fails harmlessly if DPI awareness was already set by a manifest/caller.
        let _ =
            unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        unsafe { MagInitialize() }.ok()?;
        Ok(Self {
            _not_send_sync: PhantomData,
        })
    }

    /// Applies the given fullscreen magnification transform to DWM.
    pub fn apply(&self, transform: ViewportTransform) -> Result<()> {
        let safe_scale = if transform.scale.is_finite() {
            transform.scale.max(1.0)
        } else {
            1.0
        };
        unsafe { MagSetFullscreenTransform(safe_scale, transform.x, transform.y) }.ok()
    }

    /// Resets fullscreen magnification back to normal `1.0x` at `(0, 0)`.
    pub fn reset(&self) -> Result<()> {
        self.apply(ViewportTransform {
            scale: 1.0,
            x: 0,
            y: 0,
        })
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
pub fn cursor_position() -> Result<PointF> {
    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point) }?;
    Ok(PointF {
        x: point.x as f32,
        y: point.y as f32,
    })
}
