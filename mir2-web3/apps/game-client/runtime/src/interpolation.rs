//! Snapshot interpolation for smooth entity motion.
//!
//! Entities in the Bevy runtime used to snap to the grid position carried in
//! the latest `WorldSnapshot`.  When snapshots arrive at ~100 ms intervals the
//! result is a visible stutter every frame where a new snapshot is applied.
//!
//! This module adds a small ring-buffer (`SnapshotBuffer`) that remembers the
//! last two stamped snapshots together with the Bevy-local time at which they
//! were received.  A dedicated system (`interpolate_entities`) then positions
//! every entity at its smoothly-interpolated world position for
//!
//!   `render_time = local_now – INTERP_DELAY`
//!
//! keeping entities ~one snapshot interval behind real-time in exchange for
//! perfectly smooth 60 fps motion.
//!
//! # Fallback / gating
//!
//! * If the buffer holds fewer than two snapshots the system does nothing and
//!   the existing `sync_entities` snap path runs as before.
//! * If both snapshots have the same timestamp (degenerate) alpha clamps to
//!   `1.0` and the entity jumps to the newer position.
//! * Entities that appear only in the newest snapshot snap in immediately
//!   (same as the existing path).
//! * Entities that are absent from the newest snapshot are removed by
//!   `sync_entities` exactly as before.

use std::collections::HashMap;

use bevy::prelude::*;

/// How far behind real-time (in seconds) the render position trails the latest
/// snapshot.  One snapshot interval (≈ 100 ms) is the sweet-spot: the render
/// point is always bracketed by two buffered snapshots so motion is smooth.
pub const INTERP_DELAY_SECS: f64 = 0.100;

/// A single entity's (x, y) grid position captured from a snapshot.
#[derive(Debug, Clone, Copy)]
pub struct EntityPos {
    pub x: i32,
    pub y: i32,
}

/// One buffered snapshot: the grid positions keyed by object_id, plus the
/// Bevy-local time (in seconds since startup) at which it was received.
#[derive(Debug, Clone)]
pub struct BufferedSnapshot {
    /// Wall-clock receipt time in Bevy's elapsed seconds.
    pub receipt_secs: f64,
    /// Grid positions of every entity present in the snapshot.
    pub positions: HashMap<String, EntityPos>,
}

/// A ring buffer holding the last two received snapshots.
///
/// `prev` is the older snapshot, `next` is the newer one.  When a third
/// snapshot arrives `prev` is replaced by what was `next`.
#[derive(Resource, Default)]
pub struct SnapshotBuffer {
    pub prev: Option<BufferedSnapshot>,
    pub next: Option<BufferedSnapshot>,
}

impl SnapshotBuffer {
    /// Push a new snapshot into the buffer, evicting the oldest slot.
    pub fn push(&mut self, snapshot: BufferedSnapshot) {
        let old_next = self.next.take();
        self.prev = old_next;
        self.next = Some(snapshot);
    }

    /// Returns `true` when at least two snapshots are available and
    /// interpolation can proceed.
    pub fn ready(&self) -> bool {
        self.prev.is_some() && self.next.is_some()
    }

    /// The receipt time of the newest buffered snapshot (seconds).
    #[allow(dead_code)]
    pub fn newest_receipt_secs(&self) -> Option<f64> {
        self.next.as_ref().map(|s| s.receipt_secs)
    }
}

// ---------------------------------------------------------------------------
// Pure math helpers — no Bevy dependencies, fully unit-testable.
// ---------------------------------------------------------------------------

/// Linearly interpolate between two world-space translations.
///
/// `t = 0.0` → `a`; `t = 1.0` → `b`.  `t` is **not** clamped here so the
/// caller can decide whether to pre-clamp.
#[inline]
pub fn lerp_entity_pos(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a + (b - a) * t
}

/// Compute the interpolation alpha for a render time bracketed by two snapshot
/// receipt times.
///
/// Returns a value in `[0.0, 1.0]`:
/// * `0.0` when `render_t ≤ prev_t` (clamped)
/// * `1.0` when `render_t ≥ next_t` (clamped), including the degenerate case
///   `prev_t == next_t`
/// * a proportional fraction otherwise
#[inline]
pub fn interpolation_alpha(prev_t: f64, next_t: f64, render_t: f64) -> f32 {
    let span = next_t - prev_t;
    if span <= 0.0 {
        // Degenerate: equal or inverted timestamps — snap to newest.
        return 1.0;
    }
    let alpha = (render_t - prev_t) / span;
    alpha.clamp(0.0, 1.0) as f32
}

/// Convert a grid position to a Bevy world translation.
///
/// Mirrors `tile_to_world` in `lib.rs` but lives here so the interpolation
/// path can call it without coupling to the entity-sync code.
#[allow(dead_code)]
#[inline]
pub fn grid_to_world(x: i32, y: i32, z: f32) -> Vec3 {
    const TILE_SIZE: f32 = 32.0;
    Vec3::new(x as f32 * TILE_SIZE, -(y as f32 * TILE_SIZE), z)
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- lerp_entity_pos ---

    #[test]
    fn lerp_t0_returns_a() {
        let a = Vec3::new(0.0, 0.0, 1.0);
        let b = Vec3::new(100.0, 200.0, 1.0);
        let result = lerp_entity_pos(a, b, 0.0);
        assert_eq!(result, a);
    }

    #[test]
    fn lerp_t1_returns_b() {
        let a = Vec3::new(0.0, 0.0, 1.0);
        let b = Vec3::new(100.0, 200.0, 1.0);
        let result = lerp_entity_pos(a, b, 1.0);
        assert_eq!(result, b);
    }

    #[test]
    fn lerp_midpoint() {
        let a = Vec3::new(0.0, 0.0, 1.0);
        let b = Vec3::new(100.0, 200.0, 1.0);
        let result = lerp_entity_pos(a, b, 0.5);
        assert!((result.x - 50.0).abs() < 1e-5);
        assert!((result.y - 100.0).abs() < 1e-5);
    }

    // --- interpolation_alpha ---

    #[test]
    fn alpha_render_before_prev_clamps_to_zero() {
        let alpha = interpolation_alpha(1.0, 2.0, 0.5);
        assert_eq!(alpha, 0.0);
    }

    #[test]
    fn alpha_render_after_next_clamps_to_one() {
        let alpha = interpolation_alpha(1.0, 2.0, 3.0);
        assert_eq!(alpha, 1.0);
    }

    #[test]
    fn alpha_midpoint() {
        let alpha = interpolation_alpha(1.0, 3.0, 2.0);
        assert!((alpha - 0.5).abs() < 1e-6);
    }

    #[test]
    fn alpha_quarter_point() {
        let alpha = interpolation_alpha(0.0, 4.0, 1.0);
        assert!((alpha - 0.25).abs() < 1e-6);
    }

    #[test]
    fn alpha_equal_timestamps_degenerate_returns_one() {
        // Both snapshots at the same time → snap to newest.
        let alpha = interpolation_alpha(2.0, 2.0, 2.0);
        assert_eq!(alpha, 1.0);
    }

    #[test]
    fn alpha_inverted_timestamps_returns_one() {
        // next < prev (clock went backwards) → snap to newest.
        let alpha = interpolation_alpha(3.0, 1.0, 2.0);
        assert_eq!(alpha, 1.0);
    }

    // --- SnapshotBuffer ---

    fn make_buf_snap(receipt_secs: f64) -> BufferedSnapshot {
        BufferedSnapshot {
            receipt_secs,
            positions: HashMap::new(),
        }
    }

    #[test]
    fn buffer_not_ready_when_empty() {
        let buf = SnapshotBuffer::default();
        assert!(!buf.ready());
    }

    #[test]
    fn buffer_not_ready_with_one_snapshot() {
        let mut buf = SnapshotBuffer::default();
        buf.push(make_buf_snap(0.0));
        assert!(!buf.ready());
    }

    #[test]
    fn buffer_ready_with_two_snapshots() {
        let mut buf = SnapshotBuffer::default();
        buf.push(make_buf_snap(0.0));
        buf.push(make_buf_snap(0.1));
        assert!(buf.ready());
    }

    #[test]
    fn buffer_evicts_oldest_on_third_push() {
        let mut buf = SnapshotBuffer::default();
        buf.push(make_buf_snap(0.0));
        buf.push(make_buf_snap(0.1));
        buf.push(make_buf_snap(0.2));
        // prev should now be the second snapshot (0.1), next the third (0.2).
        assert!((buf.prev.as_ref().unwrap().receipt_secs - 0.1).abs() < 1e-9);
        assert!((buf.next.as_ref().unwrap().receipt_secs - 0.2).abs() < 1e-9);
    }

    // --- grid_to_world ---

    #[test]
    fn grid_to_world_origin() {
        let v = grid_to_world(0, 0, 1.0);
        assert_eq!(v, Vec3::new(0.0, 0.0, 1.0));
    }

    #[test]
    fn grid_to_world_positive_x_negative_y_in_world() {
        // grid y increases downward; world y increases upward.
        let v = grid_to_world(2, 3, 1.0);
        assert!((v.x - 64.0).abs() < 1e-5); // 2 * 32
        assert!((v.y + 96.0).abs() < 1e-5); // -(3 * 32)
    }
}
