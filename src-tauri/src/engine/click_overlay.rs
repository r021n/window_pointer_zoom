//! A click-through Win32 overlay whose worker only renders during a click animation.

use std::mem::size_of;
use std::ptr::{self, NonNull};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use windows::core::{w, Error, Result as WinResult, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, COLORREF, HANDLE, HWND, POINT, SIZE, WAIT_FAILED};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, MonitorFromPoint, SelectObject,
    AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS,
    HBITMAP, HDC, HGDIOBJ, HMONITOR, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::Threading::{CreateEventW, SetEvent, INFINITE};
use windows::Win32::UI::HiDpi::{
    GetDpiForWindow, SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, DispatchMessageW, MsgWaitForMultipleObjectsEx, PeekMessageW,
    SetWindowPos, UpdateLayeredWindow, MSG, MWMO_INPUTAVAILABLE, PM_REMOVE, QS_ALLINPUT,
    SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW,
    ULW_ALPHA, WM_QUIT, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_EX_TRANSPARENT, WS_POPUP,
};

use super::fullscreen_engine::AppliedZoomTransform;

const CLICK_QUEUE_CAPACITY: usize = 16;
const FRAME_INTERVAL: Duration = Duration::from_millis(16);
const PRESS_DURATION: Duration = Duration::from_millis(90);
const RELEASE_DURATION: Duration = Duration::from_millis(270);
const SURFACE_DIPS: f32 = 72.0;
const MAX_SURFACE_PX: usize = 192;

#[derive(Clone, Copy)]
pub(super) enum ClickButton {
    Left,
    Right,
}

#[derive(Clone, Copy)]
pub(super) enum ClickPhase {
    Press,
    Release,
}

#[derive(Clone, Copy)]
struct Click {
    button: ClickButton,
    phase: ClickPhase,
    point: POINT,
    started: Instant,
}

impl Click {
    fn frame(self, now: Instant) -> Option<RingFrame> {
        let duration = match self.phase {
            ClickPhase::Press => PRESS_DURATION,
            ClickPhase::Release => RELEASE_DURATION,
        };
        let elapsed = now.saturating_duration_since(self.started);
        if elapsed >= duration {
            return None;
        }

        let progress = elapsed.as_secs_f32() / duration.as_secs_f32();
        let (radius_dips, opacity) = match self.phase {
            ClickPhase::Press => (13.0 - 4.0 * progress, 0.86 * (1.0 - 0.5 * progress)),
            ClickPhase::Release => {
                let remaining = (1.0 - progress).powi(2);
                (9.0 + 19.0 * (1.0 - remaining), 0.91 * remaining)
            }
        };
        let color = match self.button {
            ClickButton::Left => [80, 221, 255],
            ClickButton::Right => [255, 160, 85],
        };
        Some(RingFrame {
            point: self.point,
            radius_dips,
            opacity,
            color,
        })
    }
}

struct RingFrame {
    point: POINT,
    radius_dips: f32,
    opacity: f32,
    color: [u8; 3],
}

struct QueuedClick {
    generation: usize,
    click: Click,
}

struct WakeEvent(HANDLE);

// A process-local Win32 event can be signaled and waited on concurrently. The shared
// OverlayControl keeps its handle alive until all producers and the worker release it.
unsafe impl Send for WakeEvent {}
unsafe impl Sync for WakeEvent {}

impl Drop for WakeEvent {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

struct OverlayControl {
    // The low bit is the enabled flag. Changing it invalidates previously queued input.
    generation: AtomicUsize,
    stopped: AtomicBool,
    wake: WakeEvent,
}

impl OverlayControl {
    fn notify(&self) {
        let _ = unsafe { SetEvent(self.wake.0) };
    }
}

pub struct ClickEventSink {
    tx: SyncSender<QueuedClick>,
    control: Arc<OverlayControl>,
}

impl ClickEventSink {
    pub(super) fn send(&self, button: ClickButton, phase: ClickPhase, point: POINT) {
        let generation = self.control.generation.load(Ordering::Acquire);
        if generation & 1 == 0 || self.control.stopped.load(Ordering::Acquire) {
            return;
        }

        let event = QueuedClick {
            generation,
            click: Click {
                button,
                phase,
                point,
                started: Instant::now(),
            },
        };
        if self.tx.try_send(event).is_ok() {
            self.control.notify();
        }
    }
}

struct OverlayWorker {
    control: Arc<OverlayControl>,
    thread: JoinHandle<()>,
}

#[derive(Default)]
pub struct ClickOverlayState {
    worker: Mutex<Option<OverlayWorker>>,
}

impl ClickOverlayState {
    pub fn start(
        &self,
        enabled: bool,
        applied_zoom: Arc<AppliedZoomTransform>,
    ) -> Result<ClickEventSink, String> {
        let mut worker = self.worker.lock().unwrap_or_else(|err| err.into_inner());
        if worker.is_some() {
            return Err("Click overlay is already running".into());
        }

        let wake = unsafe { CreateEventW(None, false, false, PCWSTR::null()) }
            .map_err(|err| format!("Failed to create click overlay event: {err}"))?;
        let control = Arc::new(OverlayControl {
            generation: AtomicUsize::new(usize::from(enabled)),
            stopped: AtomicBool::new(false),
            wake: WakeEvent(wake),
        });
        let worker_control = Arc::clone(&control);
        let (tx, rx) = mpsc::sync_channel(CLICK_QUEUE_CAPACITY);
        let thread = thread::Builder::new()
            .name("click-overlay".into())
            .spawn(move || run_overlay(&worker_control, &rx, &applied_zoom))
            .map_err(|err| format!("Failed to start click overlay thread: {err}"))?;

        *worker = Some(OverlayWorker {
            control: Arc::clone(&control),
            thread,
        });
        Ok(ClickEventSink { tx, control })
    }

    pub fn is_available(&self) -> bool {
        self.worker
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .as_ref()
            .is_some_and(|worker| !worker.thread.is_finished())
    }

    pub fn set_enabled(&self, enabled: bool) {
        let worker = self.worker.lock().unwrap_or_else(|err| err.into_inner());
        if let Some(worker) = worker.as_ref() {
            let generation = worker.control.generation.load(Ordering::Relaxed);
            if (generation & 1 != 0) != enabled {
                worker
                    .control
                    .generation
                    .store(generation.wrapping_add(1), Ordering::Release);
                worker.control.notify();
            }
        }
    }

    pub fn shutdown(&self) {
        let worker = self
            .worker
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .take();
        if let Some(worker) = worker {
            worker.control.stopped.store(true, Ordering::Release);
            worker.control.notify();
            let _ = worker.thread.join();
        }
    }
}

impl Drop for ClickOverlayState {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn run_overlay(control: &OverlayControl, rx: &Receiver<QueuedClick>, zoom: &AppliedZoomTransform) {
    let _ = unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    let mut generation = control.generation.load(Ordering::Acquire);
    let mut effect: Option<Click> = None;
    let mut surface: Option<OverlaySurface> = None;
    let mut next_frame: Option<Instant> = None;
    let handles = [control.wake.0];
    let mut message = MSG::default();

    while !control.stopped.load(Ordering::Acquire) {
        let latest = control.generation.load(Ordering::Acquire);
        if latest != generation {
            generation = latest;
            effect = None;
            next_frame = None;
            surface = None;
        }

        // A burst of mouse input cannot monopolize the worker or grow its queue.
        for queued in rx.try_iter().take(CLICK_QUEUE_CAPACITY) {
            if queued.generation == generation {
                effect = Some(queued.click);
            }
        }

        let now = Instant::now();
        if next_frame.is_none_or(|deadline| now >= deadline) {
            if let Some(frame) = effect.and_then(|click| click.frame(now)) {
                let overlay = match surface.as_mut() {
                    Some(overlay) => overlay,
                    None => match OverlaySurface::new(frame.point) {
                        Ok(overlay) => surface.insert(overlay),
                        Err(err) => {
                            eprintln!("[click-overlay] failed to initialize renderer: {err}");
                            return;
                        }
                    },
                };
                if let Err(err) = overlay.present(&frame, zoom.scale()) {
                    eprintln!("[click-overlay] failed to render click effect: {err}");
                    effect = None;
                    next_frame = None;
                    surface = None;
                } else {
                    next_frame = Some(now + FRAME_INTERVAL);
                }
            } else {
                effect = None;
                next_frame = None;
                if let Some(overlay) = surface.as_mut() {
                    if let Err(err) = overlay.set_visible(false) {
                        eprintln!("[click-overlay] failed to hide overlay: {err}");
                        surface = None;
                    }
                }
            }
        }

        while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
            if message.message == WM_QUIT {
                return;
            }
            unsafe { DispatchMessageW(&message) };
        }

        let timeout = next_frame.map_or(INFINITE, |deadline| {
            deadline
                .saturating_duration_since(Instant::now())
                .as_millis()
                .max(1) as u32
        });
        if unsafe {
            MsgWaitForMultipleObjectsEx(Some(&handles), timeout, QS_ALLINPUT, MWMO_INPUTAVAILABLE)
        } == WAIT_FAILED
        {
            eprintln!("[click-overlay] wait failed: {}", Error::from_thread());
            return;
        }
    }
}

struct OverlayWindow(HWND);

impl Drop for OverlayWindow {
    fn drop(&mut self) {
        let _ = unsafe { DestroyWindow(self.0) };
    }
}

struct OverlaySurface {
    window: OverlayWindow,
    bitmap: LayeredBitmap,
    monitor: HMONITOR,
    visible: bool,
}

impl OverlaySurface {
    fn new(origin: POINT) -> WinResult<Self> {
        let window = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED
                    | WS_EX_TRANSPARENT
                    | WS_EX_TOPMOST
                    | WS_EX_TOOLWINDOW
                    | WS_EX_NOACTIVATE,
                w!("STATIC"),
                PCWSTR::null(),
                WS_POPUP,
                origin.x,
                origin.y,
                1,
                1,
                None,
                None,
                None,
                None,
            )
        }?;
        let window = OverlayWindow(window);
        Ok(Self {
            window,
            bitmap: LayeredBitmap::new()?,
            monitor: unsafe { MonitorFromPoint(origin, MONITOR_DEFAULTTONEAREST) },
            visible: false,
        })
    }

    fn present(&mut self, frame: &RingFrame, scale: f32) -> WinResult<()> {
        self.update_layer(frame, scale)?;
        self.set_visible(true)
    }

    fn update_layer(&mut self, frame: &RingFrame, scale: f32) -> WinResult<()> {
        // The hook supplies a desktop point. DWM magnifies both the native cursor and this
        // window, keeping the ring centered without applying the viewport offset twice.
        let monitor = unsafe { MonitorFromPoint(frame.point, MONITOR_DEFAULTTONEAREST) };
        if monitor != self.monitor {
            self.set_visible(false)?;
            unsafe {
                SetWindowPos(
                    self.window.0,
                    None,
                    frame.point.x,
                    frame.point.y,
                    1,
                    1,
                    SWP_NOACTIVATE | SWP_NOZORDER,
                )
            }?;
            self.monitor = monitor;
        }

        let dpi = unsafe { GetDpiForWindow(self.window.0) };
        let side = surface_side(dpi, scale);
        self.bitmap.draw(frame, side);

        let size = SIZE {
            cx: side as i32,
            cy: side as i32,
        };
        let position = POINT {
            x: frame.point.x.saturating_sub(size.cx / 2),
            y: frame.point.y.saturating_sub(size.cy / 2),
        };
        let source = POINT::default();
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            SourceConstantAlpha: u8::MAX,
            AlphaFormat: AC_SRC_ALPHA as u8,
            ..Default::default()
        };
        unsafe {
            UpdateLayeredWindow(
                self.window.0,
                None,
                Some(&position),
                Some(&size),
                Some(self.bitmap.dc),
                Some(&source),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            )
        }
    }

    fn set_visible(&mut self, visible: bool) -> WinResult<()> {
        if self.visible != visible {
            let visibility = if visible {
                SWP_SHOWWINDOW
            } else {
                SWP_HIDEWINDOW
            };
            unsafe {
                SetWindowPos(
                    self.window.0,
                    None,
                    0,
                    0,
                    0,
                    0,
                    visibility | SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOMOVE | SWP_NOSIZE,
                )
            }?;
            self.visible = visible;
        }
        Ok(())
    }
}

fn surface_side(dpi: u32, scale: f32) -> usize {
    let dpi_ratio = if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 };
    (SURFACE_DIPS * dpi_ratio / scale)
        .round()
        .clamp(8.0, MAX_SURFACE_PX as f32) as usize
}

struct LayeredBitmap {
    dc: HDC,
    bitmap: HBITMAP,
    previous_bitmap: HGDIOBJ,
    pixels: NonNull<SurfacePixels>,
}

type SurfacePixels = [[u32; MAX_SURFACE_PX]; MAX_SURFACE_PX];

impl LayeredBitmap {
    fn new() -> WinResult<Self> {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: MAX_SURFACE_PX as i32,
                biHeight: -(MAX_SURFACE_PX as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = ptr::null_mut();
        let bitmap = unsafe { CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0) }?;
        let Some(pixels) = NonNull::<SurfacePixels>::new(bits.cast()) else {
            let _ = unsafe { DeleteObject(bitmap.into()) };
            return Err(Error::from_thread());
        };
        let dc = unsafe { CreateCompatibleDC(None) };
        if dc.is_invalid() {
            let error = Error::from_thread();
            let _ = unsafe { DeleteObject(bitmap.into()) };
            return Err(error);
        }
        let previous_bitmap = unsafe { SelectObject(dc, bitmap.into()) };
        if previous_bitmap.is_invalid() {
            let error = Error::from_thread();
            let _ = unsafe { DeleteDC(dc) };
            let _ = unsafe { DeleteObject(bitmap.into()) };
            return Err(error);
        }

        unsafe { pixels.as_ptr().write_bytes(0, 1) };
        Ok(Self {
            dc,
            bitmap,
            previous_bitmap,
            pixels,
        })
    }

    fn draw(&mut self, frame: &RingFrame, side: usize) {
        let pixels = unsafe { self.pixels.as_mut() };
        let resolution = side as f32 / SURFACE_DIPS;
        let center = (side / 2) as f32;
        let radius = frame.radius_dips * resolution;
        let half_width = (1.25 * resolution).max(0.5);
        let [red, green, blue] = frame.color.map(u32::from);

        for (y, row) in pixels.iter_mut().take(side).enumerate() {
            let dy = y as f32 + 0.5 - center;
            for (x, pixel) in row.iter_mut().take(side).enumerate() {
                let dx = x as f32 + 0.5 - center;
                let distance = (dx * dx + dy * dy).sqrt();
                let coverage = (half_width + 0.5 - (distance - radius).abs()).clamp(0.0, 1.0);
                let alpha = (coverage * frame.opacity * 255.0).round() as u32;
                *pixel = u32::from_be_bytes([
                    alpha as u8,
                    (red * alpha / 255) as u8,
                    (green * alpha / 255) as u8,
                    (blue * alpha / 255) as u8,
                ]);
            }
        }
    }
}

impl Drop for LayeredBitmap {
    fn drop(&mut self) {
        unsafe {
            let _ = SelectObject(self.dc, self.previous_bitmap);
            let _ = DeleteDC(self.dc);
            let _ = DeleteObject(self.bitmap.into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongW, GetWindowRect, IsWindowVisible, GWL_EXSTYLE,
    };

    #[test]
    fn press_expires_without_release_and_release_starts_a_new_pulse() {
        let now = Instant::now();
        let click = Click {
            button: ClickButton::Left,
            phase: ClickPhase::Press,
            point: POINT { x: -400, y: -200 },
            started: now,
        };
        assert!(
            click.frame(now).unwrap().radius_dips
                > click.frame(now + PRESS_DURATION / 2).unwrap().radius_dips
        );
        assert!(click.frame(now + PRESS_DURATION).is_none());

        let release = Click {
            button: ClickButton::Right,
            phase: ClickPhase::Release,
            started: now + PRESS_DURATION,
            ..click
        };
        let initial = release.frame(release.started).unwrap();
        let later = release
            .frame(release.started + RELEASE_DURATION / 2)
            .unwrap();
        assert!(later.radius_dips > initial.radius_dips);
        assert!(later.opacity < initial.opacity);
        assert_eq!(later.point.x, -400);
        assert_eq!(later.point.y, -200);
        assert!(release.frame(release.started + RELEASE_DURATION).is_none());
    }

    #[test]
    fn surface_size_accounts_for_display_dpi_and_applied_magnification() {
        assert_eq!(surface_side(0, 1.0), 72);
        assert_eq!(surface_side(144, 1.0), 108);
        assert_eq!(surface_side(144, 2.0), 54);
        assert_eq!(surface_side(192, 2.0), 72);
        assert_eq!(surface_side(384, 1.0), MAX_SURFACE_PX);
    }

    #[test]
    fn native_layer_accepts_negative_coordinates_without_showing_or_activating() {
        let _ = unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        let origin = POINT { x: -431, y: -183 };
        let mut overlay = OverlaySurface::new(origin).unwrap();
        let frame = RingFrame {
            point: origin,
            radius_dips: 13.0,
            opacity: 0.85,
            color: [80, 221, 255],
        };

        overlay.update_layer(&frame, 2.0).unwrap();

        let expected_styles = WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE;
        let styles = unsafe { GetWindowLongW(overlay.window.0, GWL_EXSTYLE) } as u32;
        let mut bounds = RECT::default();
        unsafe { GetWindowRect(overlay.window.0, &mut bounds) }.unwrap();
        assert_eq!(styles & expected_styles.0, expected_styles.0);
        assert!(!unsafe { IsWindowVisible(overlay.window.0) }.as_bool());
        assert_eq!(bounds.left + (bounds.right - bounds.left) / 2, origin.x);
        assert_eq!(bounds.top + (bounds.bottom - bounds.top) / 2, origin.y);
    }
}
