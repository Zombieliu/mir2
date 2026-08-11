//! Renderer-neutral motion-window math.

/// An entity's current presentation step between authoritative grid cells.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionWindow {
    pub from_x: i32,
    pub from_y: i32,
    pub to_x: i32,
    pub to_y: i32,
    pub started_ms: f64,
    pub expires_ms: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionOffset {
    pub x: f32,
    pub y: f32,
}

/// Compute the shrinking sub-tile offset for an in-progress movement step.
pub fn compute_motion_offset(
    window: &MotionWindow,
    now_ms: f64,
    cell_width: f32,
    cell_height: f32,
) -> MotionOffset {
    let span = window.expires_ms - window.started_ms;
    let remaining = if span <= 0.0 {
        0.0
    } else {
        (1.0 - (now_ms - window.started_ms) / span).clamp(0.0, 1.0)
    } as f32;

    MotionOffset {
        x: (window.from_x - window.to_x) as f32 * cell_width * remaining,
        y: (window.from_y - window.to_y) as f32 * cell_height * remaining,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn horizontal_step() -> MotionWindow {
        MotionWindow {
            from_x: 0,
            from_y: 0,
            to_x: 1,
            to_y: 0,
            started_ms: 0.0,
            expires_ms: 600.0,
        }
    }

    #[test]
    fn offset_shrinks_from_source_to_destination() {
        let step = horizontal_step();
        assert_eq!(
            compute_motion_offset(&step, 0.0, 32.0, 32.0),
            MotionOffset { x: -32.0, y: 0.0 }
        );
        assert_eq!(
            compute_motion_offset(&step, 300.0, 32.0, 32.0),
            MotionOffset { x: -16.0, y: 0.0 }
        );
        assert_eq!(
            compute_motion_offset(&step, 600.0, 32.0, 32.0),
            MotionOffset { x: 0.0, y: 0.0 }
        );
    }

    #[test]
    fn time_and_degenerate_windows_are_clamped() {
        let mut step = horizontal_step();
        assert_eq!(compute_motion_offset(&step, -1.0, 32.0, 32.0).x, -32.0);
        assert_eq!(compute_motion_offset(&step, 900.0, 32.0, 32.0).x, 0.0);
        step.expires_ms = step.started_ms;
        assert_eq!(compute_motion_offset(&step, 0.0, 32.0, 32.0).x, 0.0);
    }
}
