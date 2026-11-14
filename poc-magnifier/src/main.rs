//! Fase 1 PoC runner for fullscreen pointer-centered desktop zoom.

mod magnifier;
mod math;

use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use magnifier::FullscreenMagnifier;
use math::ViewportAnimator;

const FRAME_INTERVAL: Duration = Duration::from_millis(16);
const ZOOM_IN_DURATION: Duration = Duration::from_secs(5);
const ZOOM_OUT_DURATION: Duration = Duration::from_millis(1500);
const TARGET_ZOOM_SCALE: f32 = 2.0;
const ZOOM_ALPHA: f32 = 0.15;
const PAN_ALPHA: f32 = 0.25;

fn main() -> Result<(), Box<dyn Error>> {
    let running = Arc::new(AtomicBool::new(true));
    let stop_flag = Arc::clone(&running);

    // Ctrl+C runs on a separate OS thread; only flip the atomic flag so `Drop`
    // executes on the owning main thread where `MagInitialize` was called.
    ctrlc::set_handler(move || {
        stop_flag.store(false, Ordering::Relaxed);
    })?;

    let magnifier = FullscreenMagnifier::new()?;
    let mut animator = ViewportAnimator::new();

    println!(
        "PoC magnifier active: zooming to {TARGET_ZOOM_SCALE:.1}x for 5s (press Ctrl+C to stop)..."
    );

    let start = Instant::now();
    let total_duration = ZOOM_IN_DURATION + ZOOM_OUT_DURATION;
    let mut zooming_out_logged = false;

    while running.load(Ordering::Relaxed) {
        let elapsed = start.elapsed();
        if elapsed >= total_duration {
            break;
        }

        let target_scale = if elapsed < ZOOM_IN_DURATION {
            TARGET_ZOOM_SCALE
        } else {
            if !zooming_out_logged {
                println!("Zooming out smoothly to 1.0x...");
                zooming_out_logged = true;
            }
            1.0
        };

        let screen = magnifier::primary_screen_size();
        if let Ok(cursor) = magnifier::cursor_position() {
            let transform = animator.step(cursor, screen, target_scale, ZOOM_ALPHA, PAN_ALPHA);
            if let Err(err) = magnifier.apply(transform) {
                eprintln!("Failed to apply magnification transform: {err}");
                break;
            }
        }

        std::thread::sleep(FRAME_INTERVAL);
    }

    println!("Restoring normal 1.0x view...");
    Ok(())
}
