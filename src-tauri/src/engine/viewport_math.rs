//! Pure viewport geometry and smooth interpolation math.
//! Zero Win32 or OS dependencies.

use serde::{Deserialize, Serialize};

pub const ZOOM_SNAP_EPSILON: f32 = 0.001;
const MAX_EDGE_DEADZONE_RATIO: f32 = 0.45;

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

impl ViewportTransform {
    pub const IDENTITY: Self = Self {
        scale: 1.0,
        x: 0,
        y: 0,
    };
}

/// Cursor-to-viewport tracking behavior while magnified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PanningMode {
    #[default]
    CenterFollow,
    EdgePanning,
}

/// Parameters controlling a single animation step of `ViewportAnimator`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepParams {
    pub target_scale: f32,
    pub zoom_alpha: f32,
    pub pan_alpha: f32,
    pub mode: PanningMode,
    pub edge_deadzone: f32,
}

fn is_valid_screen(screen: ScreenSize) -> bool {
    screen.width.is_finite()
        && screen.height.is_finite()
        && screen.width > 0.0
        && screen.height > 0.0
}

/// Sanitizes a zoom scale so non-finite or `< 1.0` values fall back safely to `1.0`.
pub fn sanitize_scale(scale: f32) -> f32 {
    if scale.is_finite() {
        scale.max(1.0)
    } else {
        1.0
    }
}

fn view_size(screen: ScreenSize, scale: f32) -> (f32, f32) {
    let safe_scale = sanitize_scale(scale);
    (screen.width / safe_scale, screen.height / safe_scale)
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
    let (view_w, view_h) = view_size(screen, scale);
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
    let (view_w, view_h) = view_size(screen, safe_scale);
    clamp_offset(
        PointF {
            x: cursor.x - (view_w * 0.5),
            y: cursor.y - (view_h * 0.5),
        },
        screen,
        safe_scale,
    )
}

fn edge_pan_axis(current_offset: f32, cursor: f32, view_len: f32, ratio: f32) -> f32 {
    let margin = view_len * ratio;
    let min_cursor = current_offset + margin;
    let max_cursor = current_offset + view_len - margin;
    if cursor < min_cursor {
        cursor - margin
    } else if cursor > max_cursor {
        cursor - (view_len - margin)
    } else {
        current_offset
    }
}

/// Computes the clamped top-left viewport offset using a desktop-space deadzone around the
/// current viewport. Holds `current_offset` while `cursor` stays inside
/// `[current_offset + margin, current_offset + view_len - margin]`, and pans minimally when
/// crossing an edge. Non-finite cursor coordinates fall back to clamped `current_offset`.
pub fn compute_edge_pan_offset(
    current_offset: PointF,
    cursor: PointF,
    screen: ScreenSize,
    scale: f32,
    deadzone_ratio: f32,
) -> PointF {
    if !is_valid_screen(screen) {
        return PointF::ZERO;
    }
    if !cursor.x.is_finite() || !cursor.y.is_finite() {
        return clamp_offset(current_offset, screen, scale);
    }

    let safe_scale = sanitize_scale(scale);
    let base_offset = clamp_offset(current_offset, screen, safe_scale);
    let (view_w, view_h) = view_size(screen, safe_scale);
    let ratio = if deadzone_ratio.is_finite() {
        deadzone_ratio.clamp(0.0, MAX_EDGE_DEADZONE_RATIO)
    } else {
        0.0
    };

    let desired = PointF {
        x: edge_pan_axis(base_offset.x, cursor.x, view_w, ratio),
        y: edge_pan_axis(base_offset.y, cursor.y, view_h, ratio),
    };
    clamp_offset(desired, screen, safe_scale)
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

    /// Returns `true` when `target_scale <= 1.0` and the animator has settled at `1.0x` and `(0, 0)`.
    pub fn is_settled_at_identity(&self, target_scale: f32) -> bool {
        sanitize_scale(target_scale) <= 1.0
            && (self.current_scale - 1.0).abs() < ZOOM_SNAP_EPSILON
            && self.current_offset == PointF::ZERO
    }

    /// Advances scale and offset interpolation by one tick and returns a bounded transform.
    pub fn step(
        &mut self,
        cursor: PointF,
        screen: ScreenSize,
        params: StepParams,
    ) -> ViewportTransform {
        if !is_valid_screen(screen) {
            self.current_scale = 1.0;
            self.current_offset = PointF::ZERO;
            return ViewportTransform::IDENTITY;
        }

        let safe_target_scale = sanitize_scale(params.target_scale);
        let next_scale = sanitize_scale(lerp(
            self.current_scale,
            safe_target_scale,
            params.zoom_alpha,
        ));
        self.current_scale = if (next_scale - safe_target_scale).abs() < ZOOM_SNAP_EPSILON {
            safe_target_scale
        } else {
            next_scale
        };

        let desired = match params.mode {
            PanningMode::CenterFollow => compute_target_offset(cursor, screen, self.current_scale),
            PanningMode::EdgePanning => compute_edge_pan_offset(
                self.current_offset,
                cursor,
                screen,
                self.current_scale,
                params.edge_deadzone,
            ),
        };
        let next_offset = PointF {
            x: lerp(self.current_offset.x, desired.x, params.pan_alpha),
            y: lerp(self.current_offset.y, desired.y, params.pan_alpha),
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

    fn center_params(target_scale: f32, zoom_alpha: f32, pan_alpha: f32) -> StepParams {
        StepParams {
            target_scale,
            zoom_alpha,
            pan_alpha,
            mode: PanningMode::CenterFollow,
            edge_deadzone: 0.15,
        }
    }

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
            let transform = animator.step(
                PointF { x: 500.0, y: 500.0 },
                screen,
                center_params(2.0, 0.5, 0.5),
            );
            assert_eq!(transform, ViewportTransform::IDENTITY);
        }

        // Non-finite cursor / alpha values never produce NaN or out-of-bounds offsets
        let mut animator = ViewportAnimator::new();
        let t = animator.step(
            PointF {
                x: f32::NAN,
                y: f32::INFINITY,
            },
            SCREEN_1080P,
            center_params(f32::NAN, f32::NAN, f32::INFINITY),
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
        let mut last = ViewportTransform::IDENTITY;
        for _ in 0..120 {
            last = animator.step(corner, SCREEN_1080P, center_params(2.0, 0.2, 0.25));
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
            last = animator.step(corner, SCREEN_1080P, center_params(1.0, 0.2, 0.05));
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
        assert_eq!(last, ViewportTransform::IDENTITY);
        assert!(animator.is_settled_at_identity(1.0));
    }

    #[test]
    fn edge_pan_holds_offset_inside_deadzone() {
        // At 2.0x on 1080p: view is 960x540, offset=(480, 270).
        // With deadzone=0.20: margin=(192, 108), deadzone X=[672, 1248], Y=[378, 702].
        let current = PointF { x: 480.0, y: 270.0 };
        for cursor in [
            PointF { x: 960.0, y: 540.0 },
            PointF { x: 672.0, y: 378.0 },
            PointF {
                x: 1248.0,
                y: 702.0,
            },
        ] {
            let next = compute_edge_pan_offset(current, cursor, SCREEN_1080P, 2.0, 0.20);
            assert_eq!(next, current);
        }
    }

    #[test]
    fn edge_pan_shifts_and_clamps_outside_deadzone() {
        let current = PointF { x: 480.0, y: 270.0 };
        // Cursor left/above deadzone: shifts just enough to keep cursor on margin.
        let shifted_left = compute_edge_pan_offset(
            current,
            PointF { x: 600.0, y: 350.0 },
            SCREEN_1080P,
            2.0,
            0.20,
        );
        assert_eq!(shifted_left, PointF { x: 408.0, y: 242.0 });

        // Cursor right/below deadzone: shifts right/down.
        let shifted_right = compute_edge_pan_offset(
            current,
            PointF {
                x: 1300.0,
                y: 750.0,
            },
            SCREEN_1080P,
            2.0,
            0.20,
        );
        assert_eq!(shifted_right, PointF { x: 532.0, y: 318.0 });

        // Beyond screen edges: clamped to [0, 960] x [0, 540].
        assert_eq!(
            compute_edge_pan_offset(current, PointF { x: 0.0, y: 0.0 }, SCREEN_1080P, 2.0, 0.20),
            PointF::ZERO
        );
        assert_eq!(
            compute_edge_pan_offset(
                current,
                PointF {
                    x: 1920.0,
                    y: 1080.0,
                },
                SCREEN_1080P,
                2.0,
                0.20,
            ),
            PointF { x: 960.0, y: 540.0 }
        );
    }

    #[test]
    fn edge_pan_non_finite_cursor_preserves_clamped_offset() {
        let current = PointF { x: 480.0, y: 270.0 };
        assert_eq!(
            compute_edge_pan_offset(
                current,
                PointF {
                    x: f32::NAN,
                    y: 540.0,
                },
                SCREEN_1080P,
                2.0,
                0.20,
            ),
            current
        );
        assert_eq!(
            compute_edge_pan_offset(
                PointF {
                    x: 1500.0,
                    y: -20.0,
                },
                PointF {
                    x: 500.0,
                    y: f32::INFINITY,
                },
                SCREEN_1080P,
                2.0,
                0.20,
            ),
            PointF { x: 960.0, y: 0.0 }
        );
    }

    #[test]
    fn animator_edge_panning_holds_deadzone_and_settles_to_identity() {
        let mut animator = ViewportAnimator::new();
        assert!(animator.is_settled_at_identity(1.0));

        // Zoom to 2.0x with cursor at bottom-right so viewport settles near (960, 540).
        let edge_params = StepParams {
            target_scale: 2.0,
            zoom_alpha: 1.0,
            pan_alpha: 1.0,
            mode: PanningMode::EdgePanning,
            edge_deadzone: 0.20,
        };
        let t1 = animator.step(
            PointF {
                x: 1920.0,
                y: 1080.0,
            },
            SCREEN_1080P,
            edge_params,
        );
        assert_eq!(
            t1,
            ViewportTransform {
                scale: 2.0,
                x: 960,
                y: 540,
            }
        );
        assert!(!animator.is_settled_at_identity(2.0));

        // Moving cursor inside [960+192, 960+960-192] = [1152, 1728] keeps offset stationary.
        let t2 = animator.step(
            PointF {
                x: 1400.0,
                y: 800.0,
            },
            SCREEN_1080P,
            edge_params,
        );
        assert_eq!(t2, t1);

        // Zoom out to 1.0x -> settles to identity.
        let reset_params = StepParams {
            target_scale: 1.0,
            zoom_alpha: 0.25,
            pan_alpha: 0.25,
            mode: PanningMode::EdgePanning,
            edge_deadzone: 0.20,
        };
        for _ in 0..120 {
            animator.step(
                PointF {
                    x: 1400.0,
                    y: 800.0,
                },
                SCREEN_1080P,
                reset_params,
            );
        }
        assert!(animator.is_settled_at_identity(1.0));
        assert!(!animator.is_settled_at_identity(2.0));
    }
}
