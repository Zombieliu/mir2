use std::collections::{BTreeMap, BTreeSet};

use mir2_protocol::Point;
use serde::Serialize;

use crate::runtime::map::zone_map_collision_data;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ZoneBounds {
    pub min_x: i32,
    pub max_x: i32,
    pub min_y: i32,
    pub max_y: i32,
}

impl ZoneBounds {
    pub const fn new(min_x: i32, max_x: i32, min_y: i32, max_y: i32) -> Self {
        Self {
            min_x,
            max_x,
            min_y,
            max_y,
        }
    }

    fn contains(&self, point: &Point) -> bool {
        point.x >= self.min_x
            && point.x <= self.max_x
            && point.y >= self.min_y
            && point.y <= self.max_y
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ZoneCollision {
    bounds: Option<ZoneBounds>,
    blocked_cells: BTreeSet<(i32, i32)>,
    transfer_source_cells: BTreeSet<(i32, i32)>,
    /// Door index → its cells (start closed, i.e. present in `blocked_cells`).
    doors: BTreeMap<u8, Vec<(i32, i32)>>,
    // Missing ordinary terrain is not an explicitly open arena. Omit false so
    // known-map and trusted unbounded checkpoint roots keep their original bytes.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    unavailable: bool,
}

impl ZoneCollision {
    pub fn for_map(map_file_name: &str) -> Self {
        zone_map_collision_data(map_file_name)
            .map(|data| Self {
                bounds: Some(ZoneBounds::new(
                    data.bounds.min_x,
                    data.bounds.max_x,
                    data.bounds.min_y,
                    data.bounds.max_y,
                )),
                blocked_cells: data.blocked_cells,
                transfer_source_cells: data.transfer_source_cells,
                doors: data.doors,
                unavailable: false,
            })
            .unwrap_or_else(Self::unavailable)
    }

    pub fn unbounded() -> Self {
        Self {
            bounds: None,
            blocked_cells: BTreeSet::new(),
            transfer_source_cells: BTreeSet::new(),
            doors: BTreeMap::new(),
            unavailable: false,
        }
    }

    fn unavailable() -> Self {
        Self {
            unavailable: true,
            ..Self::unbounded()
        }
    }

    /// Door index → cells, for building the zone's dynamic door state.
    pub(crate) fn doors(&self) -> &BTreeMap<u8, Vec<(i32, i32)>> {
        &self.doors
    }

    /// Unblock a door's cells (door opened). No-op for unknown indices.
    pub(crate) fn open_door(&mut self, index: u8) {
        if let Some(cells) = self.doors.get(&index) {
            for cell in cells {
                self.blocked_cells.remove(cell);
            }
        }
    }

    /// Re-block a door's cells (door closed).
    pub(crate) fn close_door(&mut self, index: u8) {
        if let Some(cells) = self.doors.get(&index) {
            for cell in cells {
                self.blocked_cells.insert(*cell);
            }
        }
    }

    pub fn with_bounds(mut self, bounds: ZoneBounds) -> Self {
        self.bounds = Some(bounds);
        self
    }

    pub fn with_blocked_cells<I>(mut self, cells: I) -> Self
    where
        I: IntoIterator<Item = Point>,
    {
        self.blocked_cells
            .extend(cells.into_iter().map(|point| (point.x, point.y)));
        self
    }

    /// Register a (closed) door of the given index over `cells`.
    #[cfg(test)]
    pub(crate) fn with_door(mut self, index: u8, cells: Vec<(i32, i32)>) -> Self {
        for cell in &cells {
            self.blocked_cells.insert(*cell);
        }
        self.doors.insert(index & 0x7F, cells);
        self
    }

    pub(crate) fn is_blocked(&self, point: &Point) -> bool {
        self.unavailable
            || self
                .bounds
                .map(|bounds| !bounds.contains(point))
                .unwrap_or(false)
            || self.blocked_cells.contains(&(point.x, point.y))
    }

    pub(crate) fn is_mineable_wall(&self, point: &Point) -> bool {
        !self.unavailable
            && self.bounds.is_some_and(|bounds| bounds.contains(point))
            && self.blocked_cells.contains(&(point.x, point.y))
    }

    pub(crate) fn bounds(&self) -> Option<ZoneBounds> {
        self.bounds
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn transfer_source_cells_for_test(&self) -> &BTreeSet<(i32, i32)> {
        &self.transfer_source_cells
    }

    pub(crate) fn is_player_movement_blocked(&self, point: &Point) -> bool {
        if self.unavailable {
            return true;
        }

        // A direct-movement transfer source is always steppable by a player:
        // stepping onto it immediately fires the map transfer, so it must bypass
        // both the region-bounds and static-collision checks. The full original
        // Crystal collision for some maps (e.g. Bichon map "0") is not always
        // available at runtime, which leaves the embedded region fragment too
        // small to contain entrance doorways such as the Library source at
        // (322, 247) on the northern edge.
        if self.transfer_source_cells.contains(&(point.x, point.y)) {
            return false;
        }

        if self
            .bounds
            .map(|bounds| !bounds.contains(point))
            .unwrap_or(false)
        {
            return true;
        }

        self.blocked_cells.contains(&(point.x, point.y))
    }
}

#[cfg(test)]
mod unavailable_collision_tests {
    use super::*;

    const MISSING: &str = "missing-collision-regression-20261010";

    #[test]
    fn missing_terrain_blocks_transfer_exception_and_door_changes() {
        let point = Point { x: 10, y: 10 };
        let mut collision = ZoneCollision::for_map(MISSING)
            .with_bounds(ZoneBounds::new(0, 20, 0, 20))
            .with_door(7, vec![(10, 10)]);
        collision.transfer_source_cells.insert((10, 10));
        assert!(collision.is_blocked(&point));
        assert!(collision.is_player_movement_blocked(&point));
        assert!(!collision.is_mineable_wall(&point));
        collision.open_door(7);
        assert!(collision.is_blocked(&point));
        assert!(collision.is_player_movement_blocked(&point));
        collision.close_door(7);
        assert!(collision.is_player_movement_blocked(&point));
        assert!(!collision.is_mineable_wall(&point));
    }

    #[test]
    fn missing_terrain_clone_retains_closed_authority() {
        let collision = ZoneCollision::for_map(MISSING).clone();
        for point in [
            Point { x: 0, y: 0 },
            Point { x: -1, y: -1 },
            Point {
                x: i32::MAX,
                y: i32::MIN,
            },
        ] {
            assert!(collision.is_blocked(&point));
            assert!(collision.is_player_movement_blocked(&point));
            assert!(!collision.is_mineable_wall(&point));
        }
    }

    #[test]
    fn explicit_unbounded_keeps_original_serialized_checkpoint_shape() {
        let collision = ZoneCollision::unbounded();
        assert_eq!(
            serde_json::to_string(&collision).unwrap(),
            r#"{"bounds":null,"blocked_cells":[],"transfer_source_cells":[],"doors":{}}"#
        );
        assert!(!collision.is_player_movement_blocked(&Point { x: -1, y: -1 }));
    }

    #[test]
    fn missing_terrain_cannot_share_the_explicit_unbounded_state_root_input() {
        let missing = serde_json::to_value(ZoneCollision::for_map(MISSING)).unwrap();
        let explicit = serde_json::to_value(ZoneCollision::unbounded()).unwrap();
        assert_eq!(
            missing.get("unavailable"),
            Some(&serde_json::Value::Bool(true))
        );
        assert_ne!(missing, explicit);
        assert!(serde_json::to_value(ZoneCollision::for_map("0"))
            .unwrap()
            .get("unavailable")
            .is_none());
    }
}
