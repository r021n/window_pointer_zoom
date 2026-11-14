//! Pure viewport geometry and smooth interpolation math.
//! Zero Win32 or OS dependencies.

use serde::{Deserialize, Serialize};

pub const ZOOM_SNAP_EPSILON: f32 = 0.001;
pub const MIN_EDGE_DEADZONE_RATIO: f32 = 0.05;
pub const MAX_EDGE_DEADZONE_RATIO: f32 = 0.45;

/// Physical pixel bounds of a display monitor or virtual desktop.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DesktopRect {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

impl DesktopRect {
    /// Creates a zero-origin `DesktopRect` from width and height.
    pub const fn from_size(width: f32, height: f32) -> Self {
        Self {
            left: 0.0,
            top: 0.0,
            width,
            height,
        }
    }
}

/// Integer pixel rectangle matching Win32 `RECT` bounds (`right > left`, `bottom > top`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RectI32 {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
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

fn is_valid_rect(rect: DesktopRect) -> bool {
    rect.left.is_finite()
        && rect.top.is_finite()
        && rect.width.is_finite()
        && rect.height.is_finite()
        && rect.width > 0.0
        && rect.height > 0.0
}

/// Sanitizes a zoom scale so non-finite or `< 1.0` values fall back safely to `1.0`.
pub fn sanitize_scale(scale: f32) -> f32 {
    if scale.is_finite() {
        scale.max(1.0)
    } else {
        1.0
    }
}

fn view_size(rect: DesktopRect, scale: f32) -> (f32, f32) {
    let safe_scale = sanitize_scale(scale);
    (rect.width / safe_scale, rect.height / safe_scale)
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

fn to_visible_origin(offset: PointF, monitor: DesktopRect, scale: f32) -> PointF {
    let safe_scale = sanitize_scale(scale);
    PointF {
        x: offset.x + monitor.left / safe_scale,
        y: offset.y + monitor.top / safe_scale,
    }
}

fn to_transform_offset(visible_origin: PointF, monitor: DesktopRect, scale: f32) -> PointF {
    let safe_scale = sanitize_scale(scale);
    PointF {
        x: visible_origin.x - monitor.left / safe_scale,
        y: visible_origin.y - monitor.top / safe_scale,
    }
}

fn clamp_axis(target: f32, origin: f32, monitor_len: f32, view_len: f32) -> f32 {
    if !origin.is_finite()
        || !monitor_len.is_finite()
        || !view_len.is_finite()
        || monitor_len <= 0.0
    {
        return 0.0;
    }
    if !target.is_finite() || view_len <= 0.0 {
        return origin;
    }
    let max_offset = (monitor_len - view_len).max(0.0);
    target.clamp(origin, origin + max_offset)
}

fn clamp_visible_origin(visible_origin: PointF, monitor: DesktopRect, scale: f32) -> PointF {
    if !is_valid_rect(monitor) {
        return PointF::ZERO;
    }
    let (view_w, view_h) = view_size(monitor, scale);
    PointF {
        x: clamp_axis(visible_origin.x, monitor.left, monitor.width, view_w),
        y: clamp_axis(visible_origin.y, monitor.top, monitor.height, view_h),
    }
}

fn clamp_transform_offset(offset: PointF, monitor: DesktopRect, scale: f32) -> PointF {
    if !is_valid_rect(monitor) {
        return PointF::ZERO;
    }
    let safe_scale = sanitize_scale(scale);
    let clamped_visible = clamp_visible_origin(
        to_visible_origin(offset, monitor, safe_scale),
        monitor,
        safe_scale,
    );
    to_transform_offset(clamped_visible, monitor, safe_scale)
}

/// Computes the clamped fullscreen transform offset that centers the magnified view on `cursor`
/// within `monitor`.
pub fn compute_target_offset(cursor: PointF, monitor: DesktopRect, scale: f32) -> PointF {
    if !is_valid_rect(monitor) {
        return PointF::ZERO;
    }
    let safe_scale = sanitize_scale(scale);
    if safe_scale <= 1.0 {
        return PointF::ZERO;
    }
    let (view_w, view_h) = view_size(monitor, safe_scale);
    let desired_visible = PointF {
        x: cursor.x - (view_w * 0.5),
        y: cursor.y - (view_h * 0.5),
    };
    let clamped_visible = clamp_visible_origin(desired_visible, monitor, safe_scale);
    to_transform_offset(clamped_visible, monitor, safe_scale)
}

fn edge_pan_axis(current_visible: f32, cursor: f32, view_len: f32, ratio: f32) -> f32 {
    let margin = view_len * ratio;
    let min_cursor = current_visible + margin;
    let max_cursor = current_visible + view_len - margin;
    if cursor < min_cursor {
        cursor - margin
    } else if cursor > max_cursor {
        cursor - (view_len - margin)
    } else {
        current_visible
    }
}

/// Computes the clamped fullscreen transform offset using a desktop-space deadzone around the
/// current visible viewport on `monitor`. Holds `current_offset` while `cursor` stays inside the
/// margin, and pans minimally when crossing an edge. Non-finite cursor coordinates fall back to
/// clamped `current_offset`.
pub fn compute_edge_pan_offset(
    current_offset: PointF,
    cursor: PointF,
    monitor: DesktopRect,
    scale: f32,
    deadzone_ratio: f32,
) -> PointF {
    if !is_valid_rect(monitor) {
        return PointF::ZERO;
    }
    let safe_scale = sanitize_scale(scale);
    if safe_scale <= 1.0 {
        return PointF::ZERO;
    }
    if !cursor.x.is_finite() || !cursor.y.is_finite() {
        return clamp_transform_offset(current_offset, monitor, safe_scale);
    }

    let base_visible = clamp_visible_origin(
        to_visible_origin(current_offset, monitor, safe_scale),
        monitor,
        safe_scale,
    );
    let (view_w, view_h) = view_size(monitor, safe_scale);
    let ratio = if deadzone_ratio.is_finite() {
        deadzone_ratio.clamp(0.0, MAX_EDGE_DEADZONE_RATIO)
    } else {
        0.0
    };

    let desired_visible = PointF {
        x: edge_pan_axis(base_visible.x, cursor.x, view_w, ratio),
        y: edge_pan_axis(base_visible.y, cursor.y, view_h, ratio),
    };
    let clamped_visible = clamp_visible_origin(desired_visible, monitor, safe_scale);
    to_transform_offset(clamped_visible, monitor, safe_scale)
}

/// Computes `(rect_source, rect_dest)` for `MagSetInputTransform` from an applied fullscreen
/// transform and virtual desktop bounds. Returns `None` when unmagnified or bounds are invalid.
pub fn compute_input_rects(
    transform: ViewportTransform,
    virtual_bounds: DesktopRect,
) -> Option<(RectI32, RectI32)> {
    if !transform.scale.is_finite()
        || transform.scale <= 1.0 + ZOOM_SNAP_EPSILON
        || !is_valid_rect(virtual_bounds)
    {
        return None;
    }

    let scale = transform.scale;
    let vx = virtual_bounds.left;
    let vy = virtual_bounds.top;
    let vx_end = vx + virtual_bounds.width;
    let vy_end = vy + virtual_bounds.height;

    let dest_left = vx.round() as i32;
    let dest_top = vy.round() as i32;
    let dest_right = (vx_end.round() as i32).max(dest_left.saturating_add(1));
    let dest_bottom = (vy_end.round() as i32).max(dest_top.saturating_add(1));

    let x = transform.x as f32;
    let y = transform.y as f32;
    let src_left = (x + vx / scale).round() as i32;
    let src_top = (y + vy / scale).round() as i32;
    let src_right = (x + vx_end / scale).round() as i32;
    let src_bottom = (y + vy_end / scale).round() as i32;

    Some((
        RectI32 {
            left: src_left,
            top: src_top,
            right: src_right.max(src_left.saturating_add(1)),
            bottom: src_bottom.max(src_top.saturating_add(1)),
        },
        RectI32 {
            left: dest_left,
            top: dest_top,
            right: dest_right,
            bottom: dest_bottom,
        },
    ))
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
        monitor: DesktopRect,
        params: StepParams,
    ) -> ViewportTransform {
        if !is_valid_rect(monitor) {
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
        if safe_target_scale <= 1.0 + ZOOM_SNAP_EPSILON
            && (self.current_scale - 1.0).abs() < ZOOM_SNAP_EPSILON
        {
            self.current_scale = 1.0;
        }

        if self.current_scale <= 1.0 + ZOOM_SNAP_EPSILON {
            self.current_offset = PointF::ZERO;
            return ViewportTransform {
                scale: self.current_scale,
                x: 0,
                y: 0,
            };
        }

        let desired = match params.mode {
            PanningMode::CenterFollow => compute_target_offset(cursor, monitor, self.current_scale),
            PanningMode::EdgePanning => compute_edge_pan_offset(
                self.current_offset,
                cursor,
                monitor,
                self.current_scale,
                params.edge_deadzone,
            ),
        };
        let next_offset = PointF {
            x: lerp(self.current_offset.x, desired.x, params.pan_alpha),
            y: lerp(self.current_offset.y, desired.y, params.pan_alpha),
        };

        // Re-clamp against `current_scale` so zoom-out never pushes the viewport past monitor edges.
        self.current_offset = clamp_transform_offset(next_offset, monitor, self.current_scale);

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

    const SCREEN_1080P: DesktopRect = DesktopRect::from_size(1920.0, 1080.0);
    const NEGATIVE_MONITOR: DesktopRect = DesktopRect {
        left: -1920.0,
        top: -200.0,
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
        assert_eq!(
            compute_target_offset(
                PointF {
                    x: -960.0,
                    y: 340.0
                },
                NEGATIVE_MONITOR,
                1.0
            ),
            PointF::ZERO
        );
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
    fn negative_origin_monitor_centers_and_clamps_in_transform_space() {
        // NEGATIVE_MONITOR = [-1920, -200, 0, 880], at 2.0x:
        // vis_x in [-1920, -960] -> transform x = vis_x - (-1920/2) = vis_x + 960 in [-960, 0]
        // vis_y in [-200, 340]   -> transform y = vis_y - (-200/2) = vis_y + 100 in [-100, 440]
        assert_eq!(
            compute_target_offset(
                PointF {
                    x: -960.0,
                    y: 340.0
                },
                NEGATIVE_MONITOR,
                2.0
            ),
            PointF {
                x: -480.0,
                y: 170.0
            }
        );
        assert_eq!(
            compute_target_offset(
                PointF {
                    x: -1920.0,
                    y: -200.0
                },
                NEGATIVE_MONITOR,
                2.0
            ),
            PointF {
                x: -960.0,
                y: -100.0
            }
        );
        assert_eq!(
            compute_target_offset(PointF { x: 0.0, y: 880.0 }, NEGATIVE_MONITOR, 2.0),
            PointF { x: 0.0, y: 440.0 }
        );

        // Edge panning on negative-origin monitor: deadzone stays in visible desktop coordinates.
        let center = PointF {
            x: -480.0,
            y: 170.0,
        };
        // visible=[ -1440.. -480, 70..610 ], margin=(192, 108) -> deadzone=[-1248..-672, 178..502]
        assert_eq!(
            compute_edge_pan_offset(
                center,
                PointF {
                    x: -960.0,
                    y: 340.0
                },
                NEGATIVE_MONITOR,
                2.0,
                0.20,
            ),
            center
        );
        // Cursor left of deadzone at -1300 -> vis_x = -1300 - 192 = -1492 -> x = -532.
        assert_eq!(
            compute_edge_pan_offset(
                center,
                PointF {
                    x: -1300.0,
                    y: 340.0
                },
                NEGATIVE_MONITOR,
                2.0,
                0.20,
            ),
            PointF {
                x: -532.0,
                y: 170.0
            }
        );
    }

    #[test]
    fn two_monitor_border_continuity_across_shared_edge() {
        // M1 (1080p) right edge R1 == 0 == M2 (4K) left edge L2.
        let left_monitor = DesktopRect {
            left: -1920.0,
            top: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let right_4k = DesktopRect::from_size(3840.0, 2160.0);
        let seam_cursor = PointF { x: 0.0, y: 540.0 };

        let on_left = compute_target_offset(seam_cursor, left_monitor, 2.0);
        let on_right = compute_target_offset(seam_cursor, right_4k, 2.0);
        assert_eq!(on_left.x, on_right.x);
        assert_eq!(on_left.x, 0.0);

        // M1 [0, 1920] and M2 [1920, 4480] sharing R1 == L2 == 1920 at 2.5x.
        let m1 = DesktopRect::from_size(1920.0, 1080.0);
        let m2 = DesktopRect {
            left: 1920.0,
            top: 0.0,
            width: 2560.0,
            height: 1440.0,
        };
        let border_cursor = PointF {
            x: 1920.0,
            y: 540.0,
        };
        let m1_offset = compute_target_offset(border_cursor, m1, 2.5);
        let m2_offset = compute_target_offset(border_cursor, m2, 2.5);
        assert!((m1_offset.x - m2_offset.x).abs() < 1e-4);

        // Animator crossing seam maintains identical transform offset without jump.
        let mut animator = ViewportAnimator::new();
        let params = center_params(2.0, 1.0, 1.0);
        let t1 = animator.step(
            PointF {
                x: 1920.0,
                y: 540.0,
            },
            m1,
            params,
        );
        let t2 = animator.step(
            PointF {
                x: 1920.0,
                y: 540.0,
            },
            m2,
            params,
        );
        assert_eq!(t1.x, t2.x);
    }

    #[test]
    fn scale_one_snap_on_negative_origin_monitor_resets_offset_to_zero() {
        // Non-power-of-two negative origin to catch any float rounding residual in L / scale.
        let odd_neg_monitor = DesktopRect {
            left: -1366.0,
            top: -768.0,
            width: 1366.0,
            height: 768.0,
        };
        let mut animator = ViewportAnimator::new();
        for _ in 0..120 {
            animator.step(
                PointF {
                    x: -1000.0,
                    y: -500.0,
                },
                odd_neg_monitor,
                center_params(2.75, 0.25, 0.25),
            );
        }
        assert!(!animator.is_settled_at_identity(2.75));

        let mut last = ViewportTransform::IDENTITY;
        for _ in 0..180 {
            last = animator.step(
                PointF {
                    x: -1000.0,
                    y: -500.0,
                },
                odd_neg_monitor,
                center_params(1.0, 0.2, 0.05),
            );
        }
        assert_eq!(last, ViewportTransform::IDENTITY);
        assert!(animator.is_settled_at_identity(1.0));
    }

    #[test]
    fn compute_input_rects_handles_valid_and_edge_cases() {
        let single = DesktopRect::from_size(1920.0, 1080.0);
        assert_eq!(
            compute_input_rects(ViewportTransform::IDENTITY, single),
            None
        );
        assert_eq!(
            compute_input_rects(
                ViewportTransform {
                    scale: 1.0 + ZOOM_SNAP_EPSILON * 0.5,
                    x: 10,
                    y: 20,
                },
                single,
            ),
            None
        );
        assert_eq!(
            compute_input_rects(
                ViewportTransform {
                    scale: f32::NAN,
                    x: 0,
                    y: 0,
                },
                single,
            ),
            None
        );

        let bad_bounds = [
            DesktopRect::from_size(0.0, 1080.0),
            DesktopRect::from_size(1920.0, 0.0),
            DesktopRect::from_size(-1920.0, 1080.0),
            DesktopRect {
                left: f32::NAN,
                top: 0.0,
                width: 1920.0,
                height: 1080.0,
            },
            DesktopRect::from_size(f32::INFINITY, 1080.0),
        ];
        let zoom2x = ViewportTransform {
            scale: 2.0,
            x: 480,
            y: 270,
        };
        for bounds in bad_bounds {
            assert_eq!(compute_input_rects(zoom2x, bounds), None);
        }

        // Single-monitor 2.0x centered
        assert_eq!(
            compute_input_rects(zoom2x, single),
            Some((
                RectI32 {
                    left: 480,
                    top: 270,
                    right: 1440,
                    bottom: 810,
                },
                RectI32 {
                    left: 0,
                    top: 0,
                    right: 1920,
                    bottom: 1080,
                },
            ))
        );

        // Multi-monitor virtual desktop with negative left/top
        let virtual_dual = DesktopRect {
            left: -1920.0,
            top: -100.0,
            width: 3840.0,
            height: 1180.0,
        };
        let dual_transform = ViewportTransform {
            scale: 2.0,
            x: -480,
            y: 220,
        };
        assert_eq!(
            compute_input_rects(dual_transform, virtual_dual),
            Some((
                RectI32 {
                    left: -1440,
                    top: 170,
                    right: 480,
                    bottom: 760,
                },
                RectI32 {
                    left: -1920,
                    top: -100,
                    right: 1920,
                    bottom: 1080,
                },
            ))
        );

        // Tiny virtual bounds / high scale still guarantees right > left and bottom > top
        let tiny = DesktopRect::from_size(1.0, 1.0);
        let (src, dst) = compute_input_rects(
            ViewportTransform {
                scale: 10.0,
                x: 0,
                y: 0,
            },
            tiny,
        )
        .expect("tiny positive virtual bounds produce valid rects");
        assert!(src.right > src.left && src.bottom > src.top);
        assert!(dst.right > dst.left && dst.bottom > dst.top);
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

        // Zero, negative, and non-finite monitor dimensions return zero offset
        let bad_monitors = [
            DesktopRect::from_size(0.0, 1080.0),
            DesktopRect::from_size(1920.0, 0.0),
            DesktopRect::from_size(-1920.0, 1080.0),
            DesktopRect::from_size(f32::NAN, 1080.0),
            DesktopRect::from_size(1920.0, f32::INFINITY),
            DesktopRect {
                left: f32::NAN,
                top: 0.0,
                width: 1920.0,
                height: 1080.0,
            },
        ];
        for monitor in bad_monitors {
            assert_eq!(
                compute_target_offset(PointF { x: 500.0, y: 500.0 }, monitor, 2.0),
                PointF::ZERO
            );
            let mut animator = ViewportAnimator::new();
            let transform = animator.step(
                PointF { x: 500.0, y: 500.0 },
                monitor,
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
