use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

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
    // Host cache identity only. Arc<()> value equality stays true, so this
    // neither changes terrain equality nor any checkpoint bytes. Holding a
    // token prevents pointer reuse while a derived path map is cached.
    #[serde(skip)]
    static_path_revision: Arc<()>,
}

impl ZoneCollision {
    pub fn for_map(map_file_name: &str) -> Self {
        zone_map_collision_data(map_file_name)
            .map(Self::from_map_data)
            .unwrap_or_else(Self::unavailable)
    }

    /// Exact activated Crystal-world terrain, without a global mode change or
    /// the starter-slice fallback. Only available to isolated host fixtures.
    #[cfg(any(test, feature = "test-support"))]
    pub fn for_full_world_test(map_file_name: &str) -> Self {
        crate::runtime::map::zone_full_world_collision_for_test(map_file_name)
            .map(Self::from_map_data)
            .unwrap_or_else(Self::unavailable)
    }

    fn from_map_data(data: crate::runtime::map::ZoneMapCollisionData) -> Self {
        Self {
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
            static_path_revision: Arc::new(()),
        }
    }

    pub fn unbounded() -> Self {
        Self {
            bounds: None,
            blocked_cells: BTreeSet::new(),
            transfer_source_cells: BTreeSet::new(),
            doors: BTreeMap::new(),
            unavailable: false,
            static_path_revision: Arc::new(()),
        }
    }

    fn unavailable() -> Self {
        Self {
            unavailable: true,
            ..Self::unbounded()
        }
    }

    pub(crate) fn is_available(&self) -> bool {
        !self.unavailable
    }

    /// Stable across clones/forks; replaced by an actual collision edit.
    /// Process-local and never a client capability or serialized identifier.
    pub fn static_path_cache_token(&self) -> Arc<()> {
        self.static_path_revision.clone()
    }

    /// Ordinary static terrain for a bounded return path. Unlike a player's
    /// movement exception, a transfer source is never a safe return waypoint.
    pub fn static_path_cell_walkable(&self, point: &Point) -> bool {
        !self.is_ground_drop_blocked(point)
    }

    /// Door index → cells, for building the zone's dynamic door state.
    pub(crate) fn doors(&self) -> &BTreeMap<u8, Vec<(i32, i32)>> {
        &self.doors
    }

    /// Unblock a door's cells (door opened). No-op for unknown indices.
    pub(crate) fn open_door(&mut self, index: u8) {
        let mut changed = false;
        if let Some(cells) = self.doors.get(&index) {
            for cell in cells {
                changed |= self.blocked_cells.remove(cell);
            }
        }
        if changed { self.static_path_revision = Arc::new(()); }
    }

    /// Re-block a door's cells (door closed).
    pub(crate) fn close_door(&mut self, index: u8) {
        let mut changed = false;
        if let Some(cells) = self.doors.get(&index) {
            for cell in cells {
                changed |= self.blocked_cells.insert(*cell);
            }
        }
        if changed { self.static_path_revision = Arc::new(()); }
    }

    pub fn with_bounds(mut self, bounds: ZoneBounds) -> Self {
        if self.bounds != Some(bounds) { self.static_path_revision = Arc::new(()); }
        self.bounds = Some(bounds);
        self
    }

    pub fn with_blocked_cells<I>(mut self, cells: I) -> Self
    where
        I: IntoIterator<Item = Point>,
    {
        let before = self.blocked_cells.len();
        self.blocked_cells
            .extend(cells.into_iter().map(|point| (point.x, point.y)));
        if self.blocked_cells.len() != before { self.static_path_revision = Arc::new(()); }
        self
    }

    /// Register a (closed) door of the given index over `cells`.
    #[cfg(test)]
    pub(crate) fn with_door(mut self, index: u8, cells: Vec<(i32, i32)>) -> Self {
        for cell in &cells {
            self.blocked_cells.insert(*cell);
        }
        self.doors.insert(index & 0x7F, cells);
        self.static_path_revision = Arc::new(());
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

    pub(crate) fn is_ground_drop_blocked(&self, point: &Point) -> bool {
        self.is_blocked(point) || self.transfer_source_cells.contains(&(point.x, point.y))
    }

    #[cfg(test)]
    pub(crate) fn with_transfer_source_cells_for_test(
        mut self,
        cells: impl IntoIterator<Item = Point>,
    ) -> Self {
        let before = self.transfer_source_cells.len();
        self.transfer_source_cells
            .extend(cells.into_iter().map(|p| (p.x, p.y)));
        if self.transfer_source_cells.len() != before { self.static_path_revision = Arc::new(()); }
        self
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
mod ground_drop_tests {
    use super::*;

    #[test]
    fn ground_drop_collision_excludes_transfer_sources_and_walls() {
        let mut collision = ZoneCollision::unbounded()
            .with_bounds(ZoneBounds::new(0, 5, 0, 5))
            .with_blocked_cells([Point { x: 1, y: 1 }]);
        collision.transfer_source_cells.insert((2, 2));
        assert!(collision.is_ground_drop_blocked(&Point { x: 1, y: 1 }));
        assert!(collision.is_ground_drop_blocked(&Point { x: 2, y: 2 }));
        assert!(collision.is_ground_drop_blocked(&Point { x: 6, y: 2 }));
        assert!(!collision.is_ground_drop_blocked(&Point { x: 3, y: 3 }));
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

#[cfg(test)]
mod static_path_cache_tests {
    use super::*;

    #[test]
    fn static_path_cache_full_world_fixture_loads_real_bichon_and_never_falls_back_for_unknown_map() {
        let real = ZoneCollision::for_full_world_test("0");
        assert!(real.static_path_cell_walkable(&Point { x: 288, y: 616 }));
        assert!(real.static_path_cell_walkable(&Point { x: 290, y: 631 }));
        let missing = ZoneCollision::for_full_world_test("missing-full-world-homeward-fixture");
        assert!(!missing.static_path_cell_walkable(&Point { x: 288, y: 616 }));
    }

    #[test]
    fn static_path_cache_token_is_shared_by_forks_but_never_changes_equality_or_serialization() {
        let original = ZoneCollision::unbounded();
        let fork = original.clone();
        let independent = ZoneCollision::unbounded();
        assert!(Arc::ptr_eq(&original.static_path_cache_token(), &fork.static_path_cache_token()));
        assert!(!Arc::ptr_eq(&original.static_path_cache_token(), &independent.static_path_cache_token()));
        assert_eq!(original, independent);
        assert_eq!(serde_json::to_vec(&original).unwrap(), serde_json::to_vec(&independent).unwrap());
        assert_eq!(serde_json::to_string(&original).unwrap(), r#"{"bounds":null,"blocked_cells":[],"transfer_source_cells":[],"doors":{}}"#);
    }

    #[test]
    fn static_path_cache_token_changes_on_real_bounds_blocked_or_transfer_mutation() {
        let original = ZoneCollision::unbounded();
        let bounded = original.clone().with_bounds(ZoneBounds::new(0, 20, 0, 20));
        assert!(!Arc::ptr_eq(&original.static_path_cache_token(), &bounded.static_path_cache_token()));
        let same = bounded.clone().with_bounds(ZoneBounds::new(0, 20, 0, 20));
        assert!(Arc::ptr_eq(&bounded.static_path_cache_token(), &same.static_path_cache_token()));
        let blocked = bounded.clone().with_blocked_cells([Point { x: 10, y: 10 }]);
        assert!(!Arc::ptr_eq(&bounded.static_path_cache_token(), &blocked.static_path_cache_token()));
        assert!(!blocked.static_path_cell_walkable(&Point { x: 10, y: 10 }));
        let transfer = bounded.clone().with_transfer_source_cells_for_test([Point { x: 10, y: 10 }]);
        assert!(!Arc::ptr_eq(&bounded.static_path_cache_token(), &transfer.static_path_cache_token()));
        assert!(!transfer.static_path_cell_walkable(&Point { x: 10, y: 10 }));
        assert!(!transfer.is_player_movement_blocked(&Point { x: 10, y: 10 }), "return geometry must not reuse a map-exit movement exception");
    }

    #[test]
    fn static_path_cache_token_changes_when_a_door_opens_or_closes_without_affecting_old_fork() {
        let mut collision = ZoneCollision::unbounded().with_door(7, vec![(10, 10)]);
        let old_fork = collision.clone();
        let closed = collision.static_path_cache_token();
        collision.open_door(7);
        let open = collision.static_path_cache_token();
        assert!(!Arc::ptr_eq(&closed, &open));
        assert!(collision.static_path_cell_walkable(&Point { x: 10, y: 10 }));
        assert!(!old_fork.static_path_cell_walkable(&Point { x: 10, y: 10 }));
        collision.open_door(7);
        assert!(Arc::ptr_eq(&open, &collision.static_path_cache_token()), "a no-op cannot evict a valid map");
        collision.close_door(7);
        assert!(!Arc::ptr_eq(&open, &collision.static_path_cache_token()));
        assert_eq!(collision, old_fork);
        assert_eq!(serde_json::to_vec(&collision).unwrap(), serde_json::to_vec(&old_fork).unwrap());
    }
}
