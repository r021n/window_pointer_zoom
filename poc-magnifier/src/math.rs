//! Pure viewport geometry and smooth interpolation math.
//! Zero Win32 or OS dependencies.

const ZOOM_SNAP_EPSILON: f32 = 0.001;

/// Physical pixel dimensions of the target screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenSize {
    pub width: f32,
    pub height: f32,
}

/// 2D point in unmagnified desktop coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointF {
    pub x: f32,
    pub y: f32,
}

impl PointF {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
}

/// Fullscreen transform parameters expected by `MagSetFullscreenTransform`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportTransform {
    pub scale: f32,
    pub x: i32,
    pub y: i32,
}

fn is_valid_screen(screen: ScreenSize) -> bool {
    screen.width.is_finite()
        && screen.height.is_finite()
        && screen.width > 0.0
        && screen.height > 0.0
}

fn sanitize_scale(scale: f32) -> f32 {
    if scale.is_finite() {
        scale.max(1.0)
    } else {
        1.0
    }
}

/// Linearly interpolates between `current` and `target` with `alpha` clamped to `[0.0, 1.0]`.
pub fn lerp(current: f32, target: f32, alpha: f32) -> f32 {
    let t = if alpha.is_finite() {
        alpha.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if !current.is_finite() {
        return if target.is_finite() { target } else { 0.0 };
    }
    if !target.is_finite() {
        return current;
    }
    current + (target - current) * t
}

/// Clamps a single-axis viewport offset to `[0.0, max(0.0, screen_len - view_len)]`.
pub fn clamp_axis(target: f32, screen_len: f32, view_len: f32) -> f32 {
    if !target.is_finite()
        || !screen_len.is_finite()
        || !view_len.is_finite()
        || screen_len <= 0.0
        || view_len <= 0.0
    {
        return 0.0;
    }
    let max_offset = (screen_len - view_len).max(0.0);
    target.clamp(0.0, max_offset)
}

fn clamp_offset(offset: PointF, screen: ScreenSize, scale: f32) -> PointF {
    if !is_valid_screen(screen) {
        return PointF::ZERO;
    }
    let safe_scale = sanitize_scale(scale);
    let view_w = screen.width / safe_scale;
    let view_h = screen.height / safe_scale;
    PointF {
        x: clamp_axis(offset.x, screen.width, view_w),
        y: clamp_axis(offset.y, screen.height, view_h),
    }
}

/// Computes the clamped top-left viewport offset that centers the magnified view on `cursor`.
pub fn compute_target_offset(cursor: PointF, screen: ScreenSize, scale: f32) -> PointF {
    if !is_valid_screen(screen) {
        return PointF::ZERO;
    }
    let safe_scale = sanitize_scale(scale);
    let view_w = screen.width / safe_scale;
    let view_h = screen.height / safe_scale;
    clamp_offset(
        PointF {
            x: cursor.x - (view_w * 0.5),
            y: cursor.y - (view_h * 0.5),
        },
        screen,
        safe_scale,
    )
}

/// Stateful smooth animator for fullscreen zoom scale and viewport offset.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportAnimator {
    current_scale: f32,
    current_offset: PointF,
}

impl Default for ViewportAnimator {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewportAnimator {
    /// Creates an animator initialized at unmagnified `1.0x` and `(0.0, 0.0)`.
    pub fn new() -> Self {
        Self {
            current_scale: 1.0,
            current_offset: PointF::ZERO,
        }
    }

    /// Advances scale and offset interpolation by one tick and returns a bounded transform.
    pub fn step(
        &mut self,
        cursor: PointF,
        screen: ScreenSize,
        target_scale: f32,
        zoom_alpha: f32,
        pan_alpha: f32,
    ) -> ViewportTransform {
        if !is_valid_screen(screen) {
            self.current_scale = 1.0;
            self.current_offset = PointF::ZERO;
            return ViewportTransform {
                scale: 1.0,
                x: 0,
                y: 0,
            };
        }

        let safe_target_scale = sanitize_scale(target_scale);
        let next_scale = sanitize_scale(lerp(self.current_scale, safe_target_scale, zoom_alpha));
        self.current_scale = if (next_scale - safe_target_scale).abs() < ZOOM_SNAP_EPSILON {
            safe_target_scale
        } else {
            next_scale
        };

        let desired = compute_target_offset(cursor, screen, self.current_scale);
        let next_offset = PointF {
            x: lerp(self.current_offset.x, desired.x, pan_alpha),
            y: lerp(self.current_offset.y, desired.y, pan_alpha),
        };

        // Re-clamp against `current_scale` so zoom-out never pushes the viewport past screen edges.
        self.current_offset = clamp_offset(next_offset, screen, self.current_scale);

        ViewportTransform {
            scale: self.current_scale,
            x: self.current_offset.x.round() as i32,
            y: self.current_offset.y.round() as i32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN_1080P: ScreenSize = ScreenSize {
        width: 1920.0,
        height: 1080.0,
    };

    #[test]
    fn scale_one_always_yields_zero_offset() {
        let corners = [
            PointF { x: 0.0, y: 0.0 },
            PointF { x: 960.0, y: 540.0 },
            PointF {
                x: 1920.0,
                y: 1080.0,
            },
        ];
        for cursor in corners {
            let offset = compute_target_offset(cursor, SCREEN_1080P, 1.0);
            assert_eq!(offset, PointF::ZERO);
        }
    }

    #[test]
    fn two_x_centering_on_1080p() {
        let offset = compute_target_offset(PointF { x: 960.0, y: 540.0 }, SCREEN_1080P, 2.0);
        assert_eq!(offset, PointF { x: 480.0, y: 270.0 });
    }

    #[test]
    fn clamping_at_top_left_and_bottom_right_edges() {
        let top_left = compute_target_offset(PointF { x: 0.0, y: 0.0 }, SCREEN_1080P, 2.0);
        assert_eq!(top_left, PointF { x: 0.0, y: 0.0 });

        let beyond_top_left =
            compute_target_offset(PointF { x: -50.0, y: -25.0 }, SCREEN_1080P, 2.0);
        assert_eq!(beyond_top_left, PointF { x: 0.0, y: 0.0 });

        let bottom_right = compute_target_offset(
            PointF {
                x: 1920.0,
                y: 1080.0,
            },
            SCREEN_1080P,
            2.0,
        );
        assert_eq!(bottom_right, PointF { x: 960.0, y: 540.0 });

        let beyond_bottom_right = compute_target_offset(
            PointF {
                x: 2500.0,
                y: 1500.0,
            },
            SCREEN_1080P,
            2.0,
        );
        assert_eq!(beyond_bottom_right, PointF { x: 960.0, y: 540.0 });
    }

    #[test]
    fn invalid_and_edge_inputs_stay_safe() {
        // Scale < 1.0 is clamped to 1.0
        assert_eq!(
            compute_target_offset(PointF { x: 960.0, y: 540.0 }, SCREEN_1080P, 0.5),
            PointF::ZERO
        );
        assert_eq!(
            compute_target_offset(PointF { x: 960.0, y: 540.0 }, SCREEN_1080P, 0.0),
            PointF::ZERO
        );
        assert_eq!(
            compute_target_offset(PointF { x: 960.0, y: 540.0 }, SCREEN_1080P, -2.0),
            PointF::ZERO
        );
        assert_eq!(
            compute_target_offset(PointF { x: 960.0, y: 540.0 }, SCREEN_1080P, f32::NAN),
            PointF::ZERO
        );
        assert_eq!(
            compute_target_offset(PointF { x: 960.0, y: 540.0 }, SCREEN_1080P, f32::INFINITY),
            PointF::ZERO
        );

        // Zero, negative, and non-finite screen dimensions return zero offset
        let bad_screens = [
            ScreenSize {
                width: 0.0,
                height: 1080.0,
            },
            ScreenSize {
                width: 1920.0,
                height: 0.0,
            },
            ScreenSize {
                width: -1920.0,
                height: 1080.0,
            },
            ScreenSize {
                width: f32::NAN,
                height: 1080.0,
            },
            ScreenSize {
                width: 1920.0,
                height: f32::INFINITY,
            },
        ];
        for screen in bad_screens {
            assert_eq!(
                compute_target_offset(PointF { x: 500.0, y: 500.0 }, screen, 2.0),
                PointF::ZERO
            );
            let mut animator = ViewportAnimator::new();
            let transform = animator.step(PointF { x: 500.0, y: 500.0 }, screen, 2.0, 0.5, 0.5);
            assert_eq!(
                transform,
                ViewportTransform {
                    scale: 1.0,
                    x: 0,
                    y: 0,
                }
            );
        }

        // Non-finite cursor / alpha values never produce NaN or out-of-bounds offsets
        let mut animator = ViewportAnimator::new();
        let t = animator.step(
            PointF {
                x: f32::NAN,
                y: f32::INFINITY,
            },
            SCREEN_1080P,
            f32::NAN,
            f32::NAN,
            f32::INFINITY,
        );
        assert!(t.scale >= 1.0 && t.scale.is_finite());
        assert_eq!(t.x, 0);
        assert_eq!(t.y, 0);
    }

    #[test]
    fn lerp_converges_monotonically_and_clamps_alpha() {
        assert_eq!(lerp(10.0, 20.0, 0.0), 10.0);
        assert_eq!(lerp(10.0, 20.0, 1.0), 20.0);
        assert_eq!(lerp(10.0, 20.0, -0.5), 10.0);
        assert_eq!(lerp(10.0, 20.0, 2.0), 20.0);
        assert_eq!(lerp(10.0, 20.0, f32::NAN), 10.0);

        let mut value = 0.0;
        for _ in 0..120 {
            value = lerp(value, 480.0, 0.15);
        }
        assert!((value - 480.0).abs() < 0.01);
    }

    #[test]
    fn animator_converges_and_smooth_zoom_out_clamps_and_snaps_to_one() {
        let mut animator = ViewportAnimator::new();
        let corner = PointF {
            x: 1920.0,
            y: 1080.0,
        };

        // Converge to 2.0x at bottom-right corner -> (960, 540)
        let mut last = ViewportTransform {
            scale: 1.0,
            x: 0,
            y: 0,
        };
        for _ in 0..120 {
            last = animator.step(corner, SCREEN_1080P, 2.0, 0.2, 0.25);
            let max_x = (SCREEN_1080P.width - SCREEN_1080P.width / last.scale).round() as i32;
            let max_y = (SCREEN_1080P.height - SCREEN_1080P.height / last.scale).round() as i32;
            assert!(last.x >= 0 && last.x <= max_x);
            assert!(last.y >= 0 && last.y <= max_y);
        }
        assert_eq!(last.scale, 2.0);
        assert_eq!(last.x, 960);
        assert_eq!(last.y, 540);

        // Smooth zoom-out with slow pan_alpha: final safety clamp must keep offset within bounds
        // of the shrinking scale at every single step, and scale must snap cleanly to 1.0.
        for _ in 0..120 {
            last = animator.step(corner, SCREEN_1080P, 1.0, 0.2, 0.05);
            let max_x = (SCREEN_1080P.width - SCREEN_1080P.width / last.scale).ceil() as i32;
            let max_y = (SCREEN_1080P.height - SCREEN_1080P.height / last.scale).ceil() as i32;
            assert!(
                last.x >= 0 && last.x <= max_x,
                "x={} exceeded max_x={} at scale={}",
                last.x,
                max_x,
                last.scale
            );
            assert!(
                last.y >= 0 && last.y <= max_y,
                "y={} exceeded max_y={} at scale={}",
                last.y,
                max_y,
                last.scale
            );
        }
        assert_eq!(
            last,
            ViewportTransform {
                scale: 1.0,
                x: 0,
                y: 0,
            }
        );
    }
}
