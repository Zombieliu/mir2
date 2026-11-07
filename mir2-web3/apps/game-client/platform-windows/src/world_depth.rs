//! Crystal DrawObjects draws a complete map row, then that row's objects.
//!
//! Keep coordinates relative to the camera: multiplying absolute map rows into
//! large f32 values loses the one-unit body/hair/weapon layer differences.

pub(crate) const MAP_DEPTH_GAIN: f32 = 10.0;
const ROW_STRIDE: i64 = 12_000;
const OBJECT_PASS: i64 = 6_000;
const COLUMN_STRIDE: i64 = 20;
// Native viewports are bounded to 128 cells plus six guard cells per side;
// object lookahead reaches at most 70 rows. This leaves both full row passes
// disjoint, including the widest supported viewport and malformed outliers.
const RELATIVE_LIMIT: i64 = 128;

fn row_column(center_x: i64, center_y: i64, x: i64, y: i64) -> i64 {
    let row = y.saturating_sub(center_y).clamp(-RELATIVE_LIMIT, RELATIVE_LIMIT);
    let column = x.saturating_sub(center_x).clamp(-RELATIVE_LIMIT, RELATIVE_LIMIT);
    (row + RELATIVE_LIMIT) * ROW_STRIDE + (column + RELATIVE_LIMIT) * COLUMN_STRIDE
}

/// Raw entity-layer depth, before the renderer's /100000 world-Z conversion.
pub(crate) fn object_depth(center_x: i64, center_y: i64, x: i64, y: i64) -> f32 {
    (row_column(center_x, center_y, x, y) + OBJECT_PASS) as f32
}

/// Same units as object_depth. MapRenderState divides this by MAP_DEPTH_GAIN;
/// the renderer applies that gain when binding map layers.
pub(crate) fn map_depth(center_x: i64, center_y: i64, x: i64, y: i64, layer: f32) -> f32 {
    row_column(center_x, center_y, x, y) as f32 + layer * MAP_DEPTH_GAIN
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_map_row_precedes_objects_and_next_row_still_occludes() {
        // Include the screenshot's Bichon coordinates and the u16 map edge.
        for (cx, cy) in [(289, 611), (65_400, 65_400)] {
            for map_dx in [-70, -1, 0, 1, 70] {
                for actor_dx in [-70, -1, 0, 1, 70] {
                    let front = map_depth(cx, cy, cx + map_dx, cy, 1.0);
                    let actor = object_depth(cx, cy, cx + actor_dx, cy);
                    let next_front = map_depth(cx, cy, cx - 70, cy + 1, -0.5);
                    assert!(front < actor + 1.0, "all map columns precede items/actors");
                    assert!(actor + 7.0 < next_front, "later rows still occlude actors");
                }
            }
        }
    }

    #[test]
    fn camera_translation_and_large_maps_preserve_composite_precision() {
        let base = object_depth(289, 611, 290, 610);
        assert_eq!(base, object_depth(65_400, 65_400, 65_401, 65_399));
        assert!(base + 1.0 < base + 2.0); // item before mount
        assert!(base + 4.0 < base + 5.0); // rear weapon before body
        assert!(base + 5.0 < base + 6.0); // body before hair
        assert!(base + 6.0 < base + 7.0); // hair before front weapon
        assert!(base + 7.0 < object_depth(289, 611, 291, 610) + 1.0);
        // Keep the existing opt-in native body occlusion probe identifiable.
        assert_eq!((base + 5.0).rem_euclid(10.0), 5.0);
    }
}
