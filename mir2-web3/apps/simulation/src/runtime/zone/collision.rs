use std::collections::BTreeSet;

use mir2_protocol::Point;

use crate::runtime::map::zone_map_collision_data;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneCollision {
    bounds: Option<ZoneBounds>,
    blocked_cells: BTreeSet<(i32, i32)>,
    transfer_source_cells: BTreeSet<(i32, i32)>,
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
            })
            .unwrap_or_else(Self::unbounded)
    }

    pub fn unbounded() -> Self {
        Self {
            bounds: None,
            blocked_cells: BTreeSet::new(),
            transfer_source_cells: BTreeSet::new(),
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

    pub(crate) fn is_blocked(&self, point: &Point) -> bool {
        self.bounds
            .map(|bounds| !bounds.contains(point))
            .unwrap_or(false)
            || self.blocked_cells.contains(&(point.x, point.y))
    }

    pub(crate) fn is_player_movement_blocked(&self, point: &Point) -> bool {
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
