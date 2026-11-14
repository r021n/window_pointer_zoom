//! Win32 Fullscreen Magnification API adapter, DPI/monitor context, and console ctrl guard.
//! This is the only module containing `unsafe` Win32 FFI calls for magnification & display.

use std::marker::PhantomData;
use std::mem::size_of;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};

use windows::core::{Result as WinResult, BOOL};
use windows::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, HANDLE, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::Security::{GetTokenInformation, TokenUIAccess, TOKEN_QUERY};
use windows::Win32::System::Console::{
    SetConsoleCtrlHandler, CTRL_BREAK_EVENT, CTRL_CLOSE_EVENT, CTRL_C_EVENT,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, SetThreadDpiAwarenessContext,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Magnification::{
    MagInitialize, MagSetFullscreenTransform, MagSetInputTransform, MagUninitialize,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetSystemMetrics, SM_CXSCREEN, SM_CXVIRTUALSCREEN, SM_CYSCREEN,
    SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

use crate::engine::viewport_math::{
    compute_input_rects, sanitize_scale, DesktopRect, PointF, RectI32, StepParams,
    ViewportAnimator, ViewportTransform,
};
use crate::state::{
    InputMappingState, InputMappingStatus, SharedEngine, StatusChangeCallback, FRAME_INTERVAL,
};

/// Enables per-monitor v2 DPI awareness for the process (best-effort).
///
/// Intended to be called before Tauri/WebView2 initialization and harmless if already configured
/// by a manifest or earlier call.
pub fn enable_per_monitor_dpi_awareness() {
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
}

/// Query the actual process token; elevation or a manifest alone does not prove UIAccess.
fn query_ui_access() -> WinResult<bool> {
    struct TokenHandle(HANDLE);
    impl Drop for TokenHandle {
        fn drop(&mut self) {
            let _ = unsafe { CloseHandle(self.0) };
        }
    }
    let mut handle = HANDLE::default();
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut handle) }?;
    let token = TokenHandle(handle);
    let mut enabled = 0u32;
    let mut returned = 0u32;
    unsafe {
        GetTokenInformation(
            token.0,
            TokenUIAccess,
            Some((&mut enabled as *mut u32).cast()),
            size_of::<u32>() as u32,
            &mut returned,
        )
    }?;
    Ok(enabled != 0)
}

/// Keep cleanup obligations separate from health: a failed update may leave old mapping enabled.
#[derive(Debug, Default)]
struct InputMappingTracker {
    status: InputMappingStatus,
    may_be_enabled: bool,
    access_denied: bool,
}

impl InputMappingTracker {
    fn new(token_result: WinResult<bool>) -> Self {
        let mut tracker = Self::default();
        tracker.status.state = InputMappingState::Disabled;
        match token_result {
            Ok(enabled) => tracker.status.ui_access = Some(enabled),
            Err(err) => tracker.status.ui_access_error_code = Some(err.code().0),
        }
        tracker
    }

    fn record_update(&mut self, result: &WinResult<()>) {
        match result {
            Ok(()) => {
                self.may_be_enabled = true;
                self.access_denied = false;
                self.status.state = InputMappingState::Active;
                self.status.error_code = None;
            }
            Err(err) => {
                self.access_denied = err.code() == ERROR_ACCESS_DENIED.to_hresult();
                self.record_error(err, self.access_denied);
            }
        }
    }

    fn record_error(&mut self, error: &windows::core::Error, denied: bool) {
        self.status.state = if denied {
            InputMappingState::Denied
        } else {
            InputMappingState::Error
        };
        self.status.error_code = Some(error.code().0);
    }

    fn set_denied(&mut self) {
        self.status.state = InputMappingState::Denied;
        self.status.error_code = Some(ERROR_ACCESS_DENIED.to_hresult().0);
    }

    fn record_disabled(&mut self) {
        self.may_be_enabled = false;
        if self.access_denied {
            self.set_denied();
        } else {
            self.status.state = InputMappingState::Disabled;
            self.status.error_code = None;
        }
    }
}

fn to_win_rect(rect: RectI32) -> RECT {
    RECT {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    }
}

/// RAII wrapper around Windows fullscreen magnification and synchronized input mapping.
///
/// Uses `PhantomData<*mut ()>` (`!Send`, `!Sync`) to enforce Win32 thread affinity:
/// initialization, transform updates, and uninitialization occur on the owning thread.
pub struct FullscreenMagnifier {
    input_transform: InputMappingTracker,
    _not_send_sync: PhantomData<*mut ()>,
}

impl FullscreenMagnifier {
    /// Ensures per-monitor v2 DPI awareness on both the process and current worker thread,
    /// then initializes the Magnification API.
    pub fn new() -> WinResult<Self> {
        enable_per_monitor_dpi_awareness();
        let _ = unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        unsafe { MagInitialize() }.ok()?;
        Ok(Self {
            input_transform: InputMappingTracker::new(query_ui_access()),
            _not_send_sync: PhantomData,
        })
    }

    fn disable_input_transform(&mut self) -> WinResult<()> {
        if !self.input_transform.may_be_enabled {
            self.input_transform.record_disabled();
            return Ok(());
        }
        let dummy = RECT {
            left: 0,
            top: 0,
            right: 1,
            bottom: 1,
        };
        if let Err(err) = unsafe { MagSetInputTransform(false, &dummy, &dummy) } {
            self.input_transform.record_error(&err, false);
            eprintln!("[zoom-engine] failed to disable input transform: {err}");
            return Err(err);
        }
        self.input_transform.record_disabled();
        Ok(())
    }

    /// Applies the fullscreen magnification transform to DWM and keeps `MagSetInputTransform`
    /// synchronized when `UIAccess` permits it.
    pub fn apply(
        &mut self,
        transform: ViewportTransform,
        virtual_bounds: DesktopRect,
    ) -> WinResult<()> {
        let safe_scale = sanitize_scale(transform.scale);
        unsafe { MagSetFullscreenTransform(safe_scale, transform.x, transform.y) }.ok()?;

        if self.input_transform.access_denied {
            self.input_transform.set_denied();
            return Ok(());
        }

        let applied = ViewportTransform {
            scale: safe_scale,
            x: transform.x,
            y: transform.y,
        };
        if let Some((src, dst)) = compute_input_rects(applied, virtual_bounds) {
            let src_rect = to_win_rect(src);
            let dst_rect = to_win_rect(dst);
            let result = unsafe { MagSetInputTransform(true, &src_rect, &dst_rect) };
            self.input_transform.record_update(&result);
            match result {
                Ok(()) => {}
                Err(err) if err.code() == ERROR_ACCESS_DENIED.to_hresult() => {
                    eprintln!(
                        "[zoom-engine] MagSetInputTransform denied (UIAccess required); continuing with visual zoom only"
                    );
                }
                Err(err) => {
                    eprintln!("[zoom-engine] failed to update input transform: {err}");
                }
            }
        } else {
            self.disable_input_transform()?;
        }

        Ok(())
    }

    /// Disables active input mapping (if enabled) and resets fullscreen magnification to `1.0x` at `(0, 0)`.
    pub fn reset(&mut self) -> WinResult<()> {
        let input_result = self.disable_input_transform();
        // Always restore the screen even if input cleanup failed.
        unsafe { MagSetFullscreenTransform(1.0, 0, 0) }.ok()?;
        input_result
    }
}

impl Drop for FullscreenMagnifier {
    fn drop(&mut self) {
        let _ = self.reset();
        let _ = unsafe { MagUninitialize() };
    }
}

#[derive(Debug, Clone, Copy)]
struct DisplayContext {
    cursor: Option<PointF>,
    active_monitor: DesktopRect,
    virtual_desktop: DesktopRect,
}

fn query_display_context() -> DisplayContext {
    let mut point = POINT::default();
    let cursor = if unsafe { GetCursorPos(&mut point) }.is_ok() {
        Some(PointF {
            x: point.x as f32,
            y: point.y as f32,
        })
    } else {
        None
    };

    let hmonitor = unsafe { MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let active_monitor =
        if !hmonitor.is_invalid() && unsafe { GetMonitorInfoW(hmonitor, &mut info) }.as_bool() {
            let rc = info.rcMonitor;
            DesktopRect {
                left: rc.left as f32,
                top: rc.top as f32,
                width: (rc.right - rc.left).max(0) as f32,
                height: (rc.bottom - rc.top).max(0) as f32,
            }
        } else {
            DesktopRect::from_size(
                unsafe { GetSystemMetrics(SM_CXSCREEN) }.max(0) as f32,
                unsafe { GetSystemMetrics(SM_CYSCREEN) }.max(0) as f32,
            )
        };

    let virtual_desktop = DesktopRect {
        left: unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) } as f32,
        top: unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) } as f32,
        width: unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) }.max(0) as f32,
        height: unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) }.max(0) as f32,
    };

    DisplayContext {
        cursor,
        active_monitor,
        virtual_desktop,
    }
}

struct ConsoleCtrlCallbacks {
    on_exit: Arc<dyn Fn() + Send + Sync>,
    on_close: Arc<dyn Fn() + Send + Sync>,
}

static CONSOLE_CTRL_CALLBACKS: Mutex<Option<ConsoleCtrlCallbacks>> = Mutex::new(None);

unsafe extern "system" fn console_ctrl_handler(ctrl_type: u32) -> BOOL {
    let callback = {
        let guard = CONSOLE_CTRL_CALLBACKS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.as_ref().and_then(|cbs| match ctrl_type {
            CTRL_C_EVENT | CTRL_BREAK_EVENT => Some(Arc::clone(&cbs.on_exit)),
            CTRL_CLOSE_EVENT => Some(Arc::clone(&cbs.on_close)),
            _ => None,
        })
    };

    if let Some(cb) = callback {
        cb();
        BOOL(1)
    } else {
        BOOL(0)
    }
}

/// RAII guard that unregisters the console control handler and clears callbacks on drop.
pub struct ConsoleCtrlGuard {
    _private: (),
}

impl Drop for ConsoleCtrlGuard {
    fn drop(&mut self) {
        let _ = unsafe { SetConsoleCtrlHandler(Some(console_ctrl_handler), false) };
        let mut guard = CONSOLE_CTRL_CALLBACKS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = None;
    }
}

/// Installs a best-effort Windows console control handler for graceful cleanup.
///
/// `on_exit` is invoked for `CTRL_C_EVENT` and `CTRL_BREAK_EVENT`, while `on_close` is invoked
/// synchronously for `CTRL_CLOSE_EVENT`. Returns `None` when no console is attached or
/// registration fails.
pub fn install_console_ctrl_handler(
    on_exit: Arc<dyn Fn() + Send + Sync>,
    on_close: Arc<dyn Fn() + Send + Sync>,
) -> Option<ConsoleCtrlGuard> {
    {
        let mut guard = CONSOLE_CTRL_CALLBACKS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = Some(ConsoleCtrlCallbacks { on_exit, on_close });
    }

    if unsafe { SetConsoleCtrlHandler(Some(console_ctrl_handler), true) }.is_ok() {
        Some(ConsoleCtrlGuard { _private: () })
    } else {
        let mut guard = CONSOLE_CTRL_CALLBACKS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = None;
        None
    }
}

fn report_input_mapping_change(
    shared: &SharedEngine,
    on_status_change: Option<&StatusChangeCallback>,
    status: InputMappingStatus,
) {
    if let Some(zoom_status) = shared.report_input_mapping(status) {
        if let Some(callback) = on_status_change {
            callback(zoom_status);
        }
    }
}

/// Spawns the dedicated `"zoom-engine"` thread and waits for `MagInitialize()` to succeed.
pub fn spawn_engine(
    shared: Arc<SharedEngine>,
    on_status_change: Option<StatusChangeCallback>,
) -> Result<JoinHandle<()>, String> {
    let (init_tx, init_rx) = mpsc::sync_channel::<Result<(), String>>(1);

    let handle = thread::Builder::new()
        .name("zoom-engine".to_string())
        .spawn(move || {
            let mut magnifier = match FullscreenMagnifier::new() {
                Ok(magnifier) => {
                    report_input_mapping_change(
                        &shared,
                        on_status_change.as_ref(),
                        magnifier.input_transform.status,
                    );
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

            run_engine_loop(&shared, on_status_change.as_ref(), &mut magnifier);
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

fn run_engine_loop(
    shared: &SharedEngine,
    on_status_change: Option<&StatusChangeCallback>,
    magnifier: &mut FullscreenMagnifier,
) {
    let mut animator = ViewportAnimator::new();
    let mut last_cursor = query_display_context().cursor.unwrap_or(PointF::ZERO);

    while let Some((target_scale, config)) = shared.wait_for_active_tick() {
        let display = query_display_context();
        if let Some(pos) = display.cursor {
            last_cursor = pos;
        }

        let params = StepParams {
            target_scale,
            zoom_alpha: config.zoom_alpha,
            pan_alpha: config.pan_alpha,
            mode: config.mode,
            edge_deadzone: config.edge_deadzone,
        };
        let transform = animator.step(last_cursor, display.active_monitor, params);

        if animator.is_settled_at_identity(target_scale) {
            let result = magnifier.reset();
            report_input_mapping_change(shared, on_status_change, magnifier.input_transform.status);
            match result {
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
            if let Err(err) = magnifier.apply(transform, display.virtual_desktop) {
                eprintln!("[zoom-engine] failed to apply fullscreen transform: {err}");
            }
            report_input_mapping_change(shared, on_status_change, magnifier.input_transform.status);
            if shared.report_frame_and_wait(transform.scale, FRAME_INTERVAL) {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::core::{Error, HRESULT};

    #[test]
    fn failed_update_invalidates_active_health_but_preserves_cleanup_obligation() {
        let mut tracker = InputMappingTracker::new(Ok(true));
        tracker.record_update(&Ok(()));
        assert_eq!(tracker.status.state, InputMappingState::Active);
        assert!(tracker.may_be_enabled);

        tracker.record_update(&Err(Error::from_hresult(HRESULT(0x80004005u32 as i32))));
        assert_eq!(tracker.status.state, InputMappingState::Error);
        assert_eq!(tracker.status.error_code, Some(0x80004005u32 as i32));
        assert!(tracker.may_be_enabled);
        assert!(!tracker.access_denied);

        tracker.record_update(&Ok(()));
        assert_eq!(tracker.status.state, InputMappingState::Active);
        assert_eq!(tracker.status.error_code, None);
        tracker.record_disabled();
        assert_eq!(tracker.status.state, InputMappingState::Disabled);
        assert!(!tracker.may_be_enabled);
    }

    #[test]
    fn access_denied_survives_reset_without_claiming_input_is_active() {
        let denied_hresult = ERROR_ACCESS_DENIED.to_hresult().0;
        let mut tracker = InputMappingTracker::new(Ok(false));
        tracker.record_update(&Err(Error::from_hresult(ERROR_ACCESS_DENIED.to_hresult())));
        assert_eq!(tracker.status.state, InputMappingState::Denied);
        assert_eq!(tracker.status.ui_access, Some(false));
        assert_eq!(tracker.status.error_code, Some(denied_hresult));
        assert!(tracker.access_denied);
        assert!(!tracker.may_be_enabled);

        tracker.record_disabled();
        assert_eq!(tracker.status.state, InputMappingState::Denied);
        assert_eq!(tracker.status.error_code, Some(denied_hresult));
        assert!(tracker.access_denied);
        assert!(!tracker.may_be_enabled);

        // Even when token query failed (ui_access == None), Denied and error_code persist across reset.
        let mut unknown_token_tracker =
            InputMappingTracker::new(Err(Error::from_hresult(HRESULT(0x80004005u32 as i32))));
        unknown_token_tracker
            .record_update(&Err(Error::from_hresult(ERROR_ACCESS_DENIED.to_hresult())));
        unknown_token_tracker.record_disabled();
        assert_eq!(unknown_token_tracker.status.ui_access, None);
        assert_eq!(
            unknown_token_tracker.status.state,
            InputMappingState::Denied
        );
        assert_eq!(
            unknown_token_tracker.status.error_code,
            Some(denied_hresult)
        );
    }

    #[test]
    fn failed_token_query_stays_unknown_and_retains_its_error_after_mapping_succeeds() {
        let mut tracker =
            InputMappingTracker::new(Err(Error::from_hresult(ERROR_ACCESS_DENIED.to_hresult())));
        tracker.record_update(&Ok(()));
        assert_eq!(tracker.status.ui_access, None);
        assert_eq!(
            tracker.status.ui_access_error_code,
            Some(ERROR_ACCESS_DENIED.to_hresult().0)
        );
        assert_eq!(tracker.status.state, InputMappingState::Active);
        assert_eq!(tracker.status.error_code, None);
    }
}
