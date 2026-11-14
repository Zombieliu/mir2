//! Bounded return geometry over this same retained Zone's current blockers.
//! Never acquire a Zone/ledger lock; normal movement still validates occupancy.
use crate::companion_supply::{LifeScope, SupplyBounds, SupplyPoint};
use mir2_protocol::Point;
use mir2_simulation::{ZoneCollision, ZoneMovementOccupancy};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

const MAX_MAPS: usize = 16;
const MAX_CELLS: usize = 41 * 51;
const UNREACHABLE: u16 = u16::MAX;

#[derive(Default)]
pub(super) struct HomewardCache {
    maps: Mutex<VecDeque<Arc<HomewardMap>>>,
    live_maps: Mutex<VecDeque<Arc<LiveHomewardMap>>>,
}

struct LiveHomewardMap {
    terrain: Arc<HomewardMap>,
    map: HomewardMap,
}

impl HomewardCache {
    #[cfg(test)]
    pub(super) fn allows(&self, scope: &LifeScope, collision: &ZoneCollision, origin: &Point, path: &[Point]) -> bool {
        self.static_map(scope, collision).is_some_and(|map| map.allows_path(origin, path))
    }

    pub(super) fn allows_live(&self, scope: &LifeScope, collision: &ZoneCollision, occupancy: &ZoneMovementOccupancy<'_>, origin: &Point, path: &[Point]) -> bool {
        if self.live_maps.is_poisoned() { return false; }
        let Some(terrain) = self.static_map(scope, collision) else { return false; };
        let mut walkable = terrain.walkable.clone();
        let bounds = terrain.bounds;
        for point in occupancy.dynamic_blocking_cells(mir2_simulation::ZoneBounds::new(bounds.min_x, bounds.max_x, bounds.min_y, bounds.max_y)) {
            // The source is the actual retained actor position, not a client
            // coordinate. Overlapping actors may leave it; the final movement
            // predicate checks destinations, not the source.
            if point != *origin {
                if let Some(index) = terrain.index(&point) { walkable[index] = false; }
            }
        }
        if walkable == terrain.walkable { return terrain.allows_path(origin, path); }
        let cached = {
            let Ok(maps) = self.live_maps.lock() else { return false; };
            maps.iter().find(|entry| Arc::ptr_eq(&entry.terrain, &terrain) && entry.map.walkable == walkable).cloned()
        };
        let live = match cached {
            Some(map) => map,
            None => {
                let map = Arc::new(LiveHomewardMap { map: terrain.with_walkability(walkable), terrain });
                let Ok(mut maps) = self.live_maps.lock() else { return false; };
                if maps.len() >= MAX_MAPS { maps.pop_front(); }
                maps.push_back(map.clone());
                map
            }
        };
        // The same immutable current-occupancy graph judges every run tile.
        live.map.allows_path(origin, path)
    }

    fn static_map(&self, scope: &LifeScope, collision: &ZoneCollision) -> Option<Arc<HomewardMap>> {
        let token = collision.static_path_cache_token();
        let map = {
            let Ok(maps) = self.maps.lock() else { return None; };
            maps.iter().find(|map| map.matches(scope, &token)).cloned()
        };
        let map = match map {
            Some(map) => map,
            None => {
                let map = HomewardMap::build(scope, collision, token)?;
                let map = Arc::new(map);
                let Ok(mut maps) = self.maps.lock() else { return None; };
                if maps.len() >= MAX_MAPS { maps.pop_front(); }
                maps.push_back(map.clone());
                map
            }
        };
        Some(map)
    }
}

struct HomewardMap {
    map_file_name: String,
    bounds: SupplyBounds,
    home: SupplyPoint,
    collision_token: Arc<()>,
    width: usize,
    walkable: Vec<bool>,
    distances: Vec<u16>,
}

impl HomewardMap {
    fn matches(&self, scope: &LifeScope, token: &Arc<()>) -> bool {
        self.map_file_name == scope.map_file_name
            && self.bounds == scope.bounds && self.home == scope.home
            && Arc::ptr_eq(&self.collision_token, token)
    }

    fn build(scope: &LifeScope, collision: &ZoneCollision, token: Arc<()>) -> Option<Self> {
        let bounds = scope.bounds;
        // Even a malformed host projection cannot allocate an unbounded map
        // or authorize a return outside this first finite Bichon experiment.
        if scope.map_file_name != "0" || bounds.min_x < 270 || bounds.max_x > 310
            || bounds.min_y < 595 || bounds.max_y > 645 { return None; }
        let width = usize::try_from(bounds.max_x.checked_sub(bounds.min_x)?.checked_add(1)?).ok()?;
        let height = usize::try_from(bounds.max_y.checked_sub(bounds.min_y)?.checked_add(1)?).ok()?;
        let cells = width.checked_mul(height)?;
        if width == 0 || height == 0 || cells > MAX_CELLS { return None; }
        let home = Point { x: scope.home.x, y: scope.home.y };
        if home.x < bounds.min_x || home.x > bounds.max_x || home.y < bounds.min_y || home.y > bounds.max_y
            || !collision.static_path_cell_walkable(&home) { return None; }
        let mut map = Self {
            map_file_name: scope.map_file_name.clone(),
            bounds, home: scope.home, collision_token: token, width,
            walkable: vec![false; cells], distances: vec![UNREACHABLE; cells],
        };
        for index in 0..cells {
            let p = map.point(index);
            map.walkable[index] = collision.static_path_cell_walkable(&p);
        }
        map.rebuild_distances();
        Some(map)
    }

    fn with_walkability(&self, walkable: Vec<bool>) -> Self {
        let mut map = Self {
            map_file_name: self.map_file_name.clone(), bounds: self.bounds, home: self.home,
            collision_token: self.collision_token.clone(), width: self.width,
            distances: vec![UNREACHABLE; walkable.len()], walkable,
        };
        map.rebuild_distances();
        map
    }

    fn rebuild_distances(&mut self) {
        self.distances.fill(UNREACHABLE);
        let mut queue = VecDeque::new();
        for index in 0..self.walkable.len() {
            let p = self.point(index);
            // Arrival beside the authorized home avoids stepping into an
            // owner's occupied tile. Static blockage of home itself failed above.
            if self.walkable[index] && (p.x - self.home.x).abs().max((p.y - self.home.y).abs()) <= 1 {
                self.distances[index] = 0;
                queue.push_back(index);
            }
        }
        while let Some(index) = queue.pop_front() {
            let p = self.point(index);
            let next_distance = self.distances[index] + 1;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if dx == 0 && dy == 0 { continue; }
                    let next = Point { x: p.x + dx, y: p.y + dy };
                    let Some(next_index) = self.index(&next) else { continue; };
                    if self.distances[next_index] != UNREACHABLE || !self.step_walkable(&p, &next) { continue; }
                    self.distances[next_index] = next_distance;
                    queue.push_back(next_index);
                }
            }
        }
    }

    fn index(&self, p: &Point) -> Option<usize> {
        if p.x < self.bounds.min_x || p.x > self.bounds.max_x || p.y < self.bounds.min_y || p.y > self.bounds.max_y { return None; }
        Some((p.y - self.bounds.min_y) as usize * self.width + (p.x - self.bounds.min_x) as usize)
    }
    fn point(&self, index: usize) -> Point {
        Point { x: self.bounds.min_x + (index % self.width) as i32, y: self.bounds.min_y + (index / self.width) as i32 }
    }
    fn walkable(&self, p: &Point) -> bool { self.index(p).is_some_and(|index| self.walkable[index]) }
    fn distance(&self, p: &Point) -> Option<u16> {
        self.index(p).map(|index| self.distances[index]).filter(|distance| *distance != UNREACHABLE)
    }
    fn step_walkable(&self, from: &Point, to: &Point) -> bool {
        if !self.walkable(from) || !self.walkable(to) { return false; }
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        if dx.abs() > 1 || dy.abs() > 1 { return false; }
        // Match the public client's eight-direction BFS: no cutting either
        // blocked orthogonal corner. Final live occupancy is checked by Zone.
        (dx == 0 || dy == 0) || self.walkable(&Point { x: to.x, y: from.y }) && self.walkable(&Point { x: from.x, y: to.y })
    }
    fn allows_path(&self, origin: &Point, path: &[Point]) -> bool {
        if path.is_empty() { return false; }
        let Some(mut previous_distance) = self.distance(origin) else { return false; };
        let mut previous = origin;
        for point in path {
            if !self.step_walkable(previous, point) { return false; }
            if point == previous { continue; }
            let Some(next) = self.distance(point) else { return false; };
            // Every actual tile must make shortest-path progress. Equal-cost
            // lateral wandering could otherwise form an unbounded circle.
            if next >= previous_distance { return false; }
            previous_distance = next;
            previous = point;
        }
        true
    }
}

#[cfg(test)]
#[path = "companion_homeward_tests.rs"]
mod tests;
