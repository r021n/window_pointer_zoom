//! Global `Ctrl + Alt + Mouse Wheel` hook (`WH_MOUSE_LL`) and delta accumulator.
//! All `unsafe` Win32 hook FFI lives in this module inside `engine/`.

use std::sync::mpsc::{self, SyncSender};
use std::sync::{Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use tauri::AppHandle;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL, VK_MENU};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, PeekMessageW, PostThreadMessageW, SetWindowsHookExW,
    UnhookWindowsHookEx, HHOOK, MSG, MSLLHOOKSTRUCT, PM_NOREMOVE, WH_MOUSE_LL, WM_MOUSEWHEEL,
    WM_QUIT,
};

use crate::dispatch_zoom_action;
use crate::state::ZoomAction;

const WHEEL_DELTA_STEP: i32 = 120;
const ACCUMULATOR_TIMEOUT: Duration = Duration::from_millis(300);
const WHEEL_CHANNEL_CAPACITY: usize = 64;

enum WheelMsg {
    Delta(i16),
    Shutdown,
}

static WHEEL_TX: OnceLock<SyncSender<WheelMsg>> = OnceLock::new();

/// Accumulates raw `WM_MOUSEWHEEL` deltas (including sub-`120` precision touchpad increments)
/// into discrete `ZoomIn` / `ZoomOut` actions.
#[derive(Debug, Default)]
pub struct WheelDeltaAccumulator {
    accumulated: i32,
    last_event: Option<Instant>,
}

impl WheelDeltaAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds a wheel delta at `now` and returns any zoom actions triggered.
    pub fn push(&mut self, delta: i16, now: Instant) -> Vec<ZoomAction> {
        if delta == 0 {
            return Vec::new();
        }

        if let Some(last) = self.last_event {
            if now.saturating_duration_since(last) > ACCUMULATOR_TIMEOUT {
                self.accumulated = 0;
            }
        }

        let delta_i32 = i32::from(delta);
        if self.accumulated != 0 && self.accumulated.signum() != delta_i32.signum() {
            self.accumulated = 0;
        }

        self.accumulated += delta_i32;
        self.last_event = Some(now);

        let mut actions = Vec::new();
        while self.accumulated >= WHEEL_DELTA_STEP {
            actions.push(ZoomAction::ZoomIn);
            self.accumulated -= WHEEL_DELTA_STEP;
        }
        while self.accumulated <= -WHEEL_DELTA_STEP {
            actions.push(ZoomAction::ZoomOut);
            self.accumulated += WHEEL_DELTA_STEP;
        }
        actions
    }
}

struct MouseHookGuard(HHOOK);

impl Drop for MouseHookGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = UnhookWindowsHookEx(self.0);
        }
    }
}

unsafe extern "system" fn low_level_mouse_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 && w_param.0 as u32 == WM_MOUSEWHEEL && l_param.0 != 0 {
        let ctrl_held = GetAsyncKeyState(i32::from(VK_CONTROL.0)) < 0;
        let alt_held = GetAsyncKeyState(i32::from(VK_MENU.0)) < 0;
        if ctrl_held && alt_held {
            let msll = &*(l_param.0 as *const MSLLHOOKSTRUCT);
            let delta = ((msll.mouseData >> 16) as u16) as i16;
            if delta != 0 {
                if let Some(tx) = WHEEL_TX.get() {
                    let _ = tx.try_send(WheelMsg::Delta(delta));
                }
                return LRESULT(1);
            }
        }
    }
    CallNextHookEx(None, n_code, w_param, l_param)
}

/// Active thread and channel handles for the running mouse wheel hook.
pub struct WheelHookHandle {
    hook_thread_id: u32,
    msg_tx: SyncSender<WheelMsg>,
    hook_thread: Option<JoinHandle<()>>,
    dispatch_thread: Option<JoinHandle<()>>,
}

impl WheelHookHandle {
    /// Signals both wheel threads to exit and joins them idempotently.
    pub fn shutdown(&mut self) {
        if self.hook_thread.is_some() {
            unsafe {
                let _ = PostThreadMessageW(self.hook_thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
        let _ = self.msg_tx.send(WheelMsg::Shutdown);

        if let Some(handle) = self.hook_thread.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.dispatch_thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for WheelHookHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Tauri-managed wrapper owning the optional `WheelHookHandle`.
#[derive(Default)]
pub struct WheelHookState {
    handle: Mutex<Option<WheelHookHandle>>,
}

impl WheelHookState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&self, handle: WheelHookHandle) {
        let mut guard = self
            .handle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(mut previous) = guard.take() {
            previous.shutdown();
        }
        *guard = Some(handle);
    }

    pub fn shutdown(&self) {
        let handle = self
            .handle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(mut handle) = handle {
            handle.shutdown();
        }
    }
}

impl Drop for WheelHookState {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Spawns the `"wheel-dispatch"` and `"wheel-hook"` threads and installs `WH_MOUSE_LL`.
pub fn spawn_wheel_hook(app: AppHandle) -> Result<WheelHookHandle, String> {
    let (msg_tx, msg_rx) = mpsc::sync_channel::<WheelMsg>(WHEEL_CHANNEL_CAPACITY);
    WHEEL_TX
        .set(msg_tx.clone())
        .map_err(|_| "Wheel hook sender already initialized".to_string())?;

    let dispatch_thread = thread::Builder::new()
        .name("wheel-dispatch".to_string())
        .spawn(move || {
            let mut accumulator = WheelDeltaAccumulator::new();
            while let Ok(msg) = msg_rx.recv() {
                match msg {
                    WheelMsg::Delta(delta) => {
                        for action in accumulator.push(delta, Instant::now()) {
                            dispatch_zoom_action(&app, action);
                        }
                    }
                    WheelMsg::Shutdown => break,
                }
            }
        })
        .map_err(|err| format!("Failed to spawn wheel-dispatch thread: {err}"))?;

    let (init_tx, init_rx) = mpsc::sync_channel::<Result<u32, String>>(1);
    let hook_thread = match thread::Builder::new()
        .name("wheel-hook".to_string())
        .spawn(move || {
            let mut msg = MSG::default();
            // Force Win32 message queue creation before reporting thread id for WM_QUIT.
            unsafe {
                let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
            }

            let hook = match unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(low_level_mouse_proc), None, 0) } {
                Ok(hook) => hook,
                Err(err) => {
                    let _ = init_tx.send(Err(format!("Failed to install mouse wheel hook: {err}")));
                    return;
                }
            };
            let _guard = MouseHookGuard(hook);
            let thread_id = unsafe { GetCurrentThreadId() };
            let _ = init_tx.send(Ok(thread_id));

            while unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0 {}
        }) {
        Ok(handle) => handle,
        Err(err) => {
            let _ = msg_tx.send(WheelMsg::Shutdown);
            let _ = dispatch_thread.join();
            return Err(format!("Failed to spawn wheel-hook thread: {err}"));
        }
    };

    let hook_thread_id = match init_rx.recv() {
        Ok(Ok(id)) => id,
        Ok(Err(err)) => {
            let _ = msg_tx.send(WheelMsg::Shutdown);
            let _ = hook_thread.join();
            let _ = dispatch_thread.join();
            return Err(err);
        }
        Err(_) => {
            let _ = msg_tx.send(WheelMsg::Shutdown);
            let _ = hook_thread.join();
            let _ = dispatch_thread.join();
            return Err("Wheel hook thread exited before initialization completed".to_string());
        }
    };

    Ok(WheelHookHandle {
        hook_thread_id,
        msg_tx,
        hook_thread: Some(hook_thread),
        dispatch_thread: Some(dispatch_thread),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notched_wheel_triggers_single_step_immediately() {
        let mut acc = WheelDeltaAccumulator::new();
        let t0 = Instant::now();

        assert_eq!(acc.push(120, t0), vec![ZoomAction::ZoomIn]);
        assert_eq!(acc.push(-120, t0), vec![ZoomAction::ZoomOut]);
        assert_eq!(acc.push(240, t0), vec![ZoomAction::ZoomIn, ZoomAction::ZoomIn]);
    }

    #[test]
    fn fractional_deltas_accumulate_across_events() {
        let mut acc = WheelDeltaAccumulator::new();
        let t0 = Instant::now();

        assert!(acc.push(30, t0).is_empty());
        assert!(acc.push(60, t0 + Duration::from_millis(20)).is_empty());
        assert_eq!(
            acc.push(30, t0 + Duration::from_millis(40)),
            vec![ZoomAction::ZoomIn]
        );

        assert!(acc.push(-60, t0 + Duration::from_millis(60)).is_empty());
        assert_eq!(
            acc.push(-60, t0 + Duration::from_millis(80)),
            vec![ZoomAction::ZoomOut]
        );
    }

    #[test]
    fn direction_reversal_resets_remainder() {
        let mut acc = WheelDeltaAccumulator::new();
        let t0 = Instant::now();

        assert!(acc.push(90, t0).is_empty());
        assert!(acc.push(-60, t0 + Duration::from_millis(10)).is_empty());
        assert_eq!(
            acc.push(-60, t0 + Duration::from_millis(20)),
            vec![ZoomAction::ZoomOut]
        );
    }

    #[test]
    fn zero_delta_is_noop() {
        let mut acc = WheelDeltaAccumulator::new();
        let t0 = Instant::now();

        assert!(acc.push(60, t0).is_empty());
        assert!(acc.push(0, t0 + Duration::from_millis(500)).is_empty());
        assert_eq!(
            acc.push(60, t0 + Duration::from_millis(50)),
            vec![ZoomAction::ZoomIn]
        );
    }

    #[test]
    fn timeout_resets_remainder() {
        let mut acc = WheelDeltaAccumulator::new();
        let t0 = Instant::now();

        assert!(acc.push(90, t0).is_empty());
        assert!(acc.push(60, t0 + Duration::from_millis(301)).is_empty());
        assert_eq!(
            acc.push(60, t0 + Duration::from_millis(350)),
            vec![ZoomAction::ZoomIn]
        );
    }
}
