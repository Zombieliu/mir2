//! Snapshot buffering and interpolation math shared by every client host.

use std::collections::HashMap;

/// One snapshot interval behind the newest authoritative state.
pub const INTERP_DELAY_SECS: f64 = 0.100;

/// A single entity's authoritative grid position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityPos {
    pub x: i32,
    pub y: i32,
}

/// Grid positions received together at one local receipt time.
#[derive(Debug, Clone, PartialEq)]
pub struct BufferedSnapshot {
    pub receipt_secs: f64,
    pub positions: HashMap<String, EntityPos>,
}

/// The newest two authoritative snapshots used to smooth presentation.
#[derive(Debug, Default)]
pub struct SnapshotBuffer {
    prev: Option<BufferedSnapshot>,
    next: Option<BufferedSnapshot>,
}

impl SnapshotBuffer {
    pub fn push(&mut self, snapshot: BufferedSnapshot) {
        self.prev = self.next.take();
        self.next = Some(snapshot);
    }

    pub fn ready(&self) -> bool {
        self.bracket().is_some()
    }

    pub fn bracket(&self) -> Option<(&BufferedSnapshot, &BufferedSnapshot)> {
        Some((self.prev.as_ref()?, self.next.as_ref()?))
    }

    pub fn newest_receipt_secs(&self) -> Option<f64> {
        self.next.as_ref().map(|snapshot| snapshot.receipt_secs)
    }
}

/// Interpolate a three-component presentation position without depending on a
/// renderer's vector type.
#[inline]
pub fn lerp_position3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Return a clamped interpolation fraction for two receipt timestamps.
#[inline]
pub fn interpolation_alpha(prev_t: f64, next_t: f64, render_t: f64) -> f32 {
    let span = next_t - prev_t;
    if span <= 0.0 {
        return 1.0;
    }

    ((render_t - prev_t) / span).clamp(0.0, 1.0) as f32
}

/// Convert a Mir grid position into renderer-neutral world coordinates.
#[inline]
pub fn grid_to_world(x: i32, y: i32, z: f32, tile_size: f32) -> [f32; 3] {
    [x as f32 * tile_size, -(y as f32 * tile_size), z]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(receipt_secs: f64) -> BufferedSnapshot {
        BufferedSnapshot {
            receipt_secs,
            positions: HashMap::new(),
        }
    }

    #[test]
    fn keeps_only_the_newest_two_snapshots() {
        let mut buffer = SnapshotBuffer::default();
        buffer.push(snapshot(0.0));
        buffer.push(snapshot(0.1));
        buffer.push(snapshot(0.2));

        let (prev, next) = buffer.bracket().expect("two snapshots");
        assert_eq!(prev.receipt_secs, 0.1);
        assert_eq!(next.receipt_secs, 0.2);
        assert_eq!(buffer.newest_receipt_secs(), Some(0.2));
    }

    #[test]
    fn readiness_requires_a_complete_bracket() {
        let mut buffer = SnapshotBuffer::default();
        assert!(!buffer.ready());
        buffer.push(snapshot(0.0));
        assert!(!buffer.ready());
        buffer.push(snapshot(0.1));
        assert!(buffer.ready());
    }

    #[test]
    fn interpolation_clamps_and_handles_degenerate_time() {
        assert_eq!(interpolation_alpha(1.0, 2.0, 0.5), 0.0);
        assert_eq!(interpolation_alpha(1.0, 2.0, 3.0), 1.0);
        assert!((interpolation_alpha(1.0, 3.0, 2.0) - 0.5).abs() < f32::EPSILON);
        assert_eq!(interpolation_alpha(2.0, 2.0, 2.0), 1.0);
        assert_eq!(interpolation_alpha(3.0, 1.0, 2.0), 1.0);
    }

    #[test]
    fn position_math_is_renderer_neutral() {
        assert_eq!(
            lerp_position3([0.0, 0.0, 1.0], [100.0, 200.0, 1.0], 0.5),
            [50.0, 100.0, 1.0]
        );
        assert_eq!(grid_to_world(2, 3, 1.0, 32.0), [64.0, -96.0, 1.0]);
    }
}
