//! Bevy adapter for platform-neutral snapshot interpolation.

use bevy::prelude::*;
use mir2_client_core::interpolation as core;

pub use core::{BufferedSnapshot, EntityPos, INTERP_DELAY_SECS};

/// Bevy resource wrapper around the renderer-neutral snapshot buffer.
#[derive(Resource, Default)]
pub struct SnapshotBuffer(core::SnapshotBuffer);

impl SnapshotBuffer {
    pub fn push(&mut self, snapshot: BufferedSnapshot) {
        self.0.push(snapshot);
    }

    pub fn ready(&self) -> bool {
        self.0.ready()
    }

    pub fn bracket(&self) -> Option<(&BufferedSnapshot, &BufferedSnapshot)> {
        self.0.bracket()
    }

    #[allow(dead_code)]
    pub fn newest_receipt_secs(&self) -> Option<f64> {
        self.0.newest_receipt_secs()
    }
}

#[inline]
pub fn lerp_entity_pos(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    Vec3::from_array(core::lerp_position3(a.to_array(), b.to_array(), t))
}

#[inline]
pub fn interpolation_alpha(prev_t: f64, next_t: f64, render_t: f64) -> f32 {
    core::interpolation_alpha(prev_t, next_t, render_t)
}

#[allow(dead_code)]
#[inline]
pub fn grid_to_world(x: i32, y: i32, z: f32) -> Vec3 {
    Vec3::from_array(core::grid_to_world(x, y, z, 32.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_preserves_bevy_vector_coordinates() {
        assert_eq!(
            lerp_entity_pos(Vec3::ZERO, Vec3::new(64.0, -96.0, 1.0), 0.5),
            Vec3::new(32.0, -48.0, 0.5)
        );
        assert_eq!(grid_to_world(2, 3, 1.0), Vec3::new(64.0, -96.0, 1.0));
    }
}
