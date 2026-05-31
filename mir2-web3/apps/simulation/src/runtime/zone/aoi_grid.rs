//! Spatial grid for area-of-interest (AOI) candidate lookup.
//!
//! The zone visibility diff previously scanned *every* player on every move and
//! tick (`O(N)` per mover, `O(N^2)` per tick). For dense scenes — a capital city
//! or a Sabuk-style siege — that quadratic cost is the dominant scaling wall.
//!
//! `AoiGrid` buckets members into coarse cells sized to the AOI range. Because a
//! cell is at least as wide/tall as the AOI rectangle, any two mutually-visible
//! points are guaranteed to fall in the same or an adjacent cell. Scanning the
//! 3x3 neighborhood of a point therefore returns a **superset** of every member
//! within AOI range; callers then apply the exact `points_visible` test to that
//! small candidate set. The visible result is identical to a full scan, but the
//! work is bounded by *local density* rather than total population.
//!
//! The grid is generic over the member key so it can be unit-tested with plain
//! integer ids and reused for both players and (later) zone objects.

use std::collections::{BTreeMap, BTreeSet};

use mir2_protocol::Point;

/// Coarse spatial bucket keyed by member id `K`.
///
/// Invariant: a member appears in exactly one cell, and `member_cell` always
/// agrees with `cells`. `cell_w`/`cell_h` must each be >= the corresponding AOI
/// range for the 3x3-neighborhood superset guarantee to hold.
#[derive(Debug, Clone)]
pub(crate) struct AoiGrid<K: Ord + Clone> {
    cell_w: i32,
    cell_h: i32,
    cells: BTreeMap<(i32, i32), BTreeSet<K>>,
    member_cell: BTreeMap<K, (i32, i32)>,
}

impl<K: Ord + Clone> AoiGrid<K> {
    /// Create a grid whose cells are `cell_w` x `cell_h` tiles. Both dimensions
    /// must be >= 1 (and, for the superset guarantee, >= the AOI range).
    pub(crate) fn new(cell_w: i32, cell_h: i32) -> Self {
        Self {
            cell_w: cell_w.max(1),
            cell_h: cell_h.max(1),
            cells: BTreeMap::new(),
            member_cell: BTreeMap::new(),
        }
    }

    fn cell_of(&self, point: &Point) -> (i32, i32) {
        (point.x.div_euclid(self.cell_w), point.y.div_euclid(self.cell_h))
    }

    /// Insert (or relocate) `id` at `point`.
    pub(crate) fn insert(&mut self, id: K, point: &Point) {
        let cell = self.cell_of(point);
        if let Some(previous) = self.member_cell.get(&id) {
            if *previous == cell {
                return;
            }
            self.detach(&id, *previous);
        }
        self.cells.entry(cell).or_default().insert(id.clone());
        self.member_cell.insert(id, cell);
    }

    /// Move an existing `id` to `point`. Falls back to insert if not present.
    pub(crate) fn moved(&mut self, id: &K, point: &Point) {
        let new_cell = self.cell_of(point);
        match self.member_cell.get(id) {
            Some(old_cell) if *old_cell == new_cell => {}
            Some(old_cell) => {
                let old_cell = *old_cell;
                self.detach(id, old_cell);
                self.cells.entry(new_cell).or_default().insert(id.clone());
                self.member_cell.insert(id.clone(), new_cell);
            }
            None => {
                self.cells.entry(new_cell).or_default().insert(id.clone());
                self.member_cell.insert(id.clone(), new_cell);
            }
        }
    }

    /// Remove `id` from the grid. No-op if absent.
    pub(crate) fn remove(&mut self, id: &K) {
        if let Some(cell) = self.member_cell.remove(id) {
            self.detach(id, cell);
        }
    }

    fn detach(&mut self, id: &K, cell: (i32, i32)) {
        if let Some(members) = self.cells.get_mut(&cell) {
            members.remove(id);
            if members.is_empty() {
                self.cells.remove(&cell);
            }
        }
    }

    /// Candidate members in the 3x3 cell neighborhood of `point`. This is a
    /// **superset** of all members within AOI range; the caller must still apply
    /// the exact `points_visible` test. Bounded by local density, not total size.
    pub(crate) fn neighbors(&self, point: &Point) -> Vec<K> {
        let (cx, cy) = self.cell_of(point);
        let mut out = Vec::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(members) = self.cells.get(&(cx + dx, cy + dy)) {
                    out.extend(members.iter().cloned());
                }
            }
        }
        out
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.member_cell.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::zone::aoi::{AOI_X_RANGE, AOI_Y_RANGE, points_visible};

    fn p(x: i32, y: i32) -> Point {
        Point { x, y }
    }

    // Deterministic pseudo-random spread so the superset proof is reproducible.
    fn scatter(seed: u64, count: usize, span: i32) -> Vec<(u32, Point)> {
        let mut state = seed;
        let mut out = Vec::new();
        for id in 0..count as u32 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let x = (state >> 33) as i32 % span - span / 2;
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let y = (state >> 33) as i32 % span - span / 2;
            out.push((id, p(x, y)));
        }
        out
    }

    /// The core correctness contract: for a grid sized to the AOI range, the 3x3
    /// neighborhood must contain EVERY member within AOI range of the query — i.e.
    /// it is a true superset, so swapping a full scan for grid+filter cannot drop
    /// any visible entity.
    #[test]
    fn neighborhood_is_a_superset_of_everything_visible() {
        let mut grid = AoiGrid::new(AOI_X_RANGE, AOI_Y_RANGE);
        let members = scatter(0x1234_5678, 400, 600);
        for (id, point) in &members {
            grid.insert(*id, point);
        }

        for (_, query) in &members {
            let candidates: BTreeSet<u32> = grid.neighbors(query).into_iter().collect();
            // Brute-force ground truth: every member actually within AOI range.
            for (id, point) in &members {
                if points_visible(query, point) {
                    assert!(
                        candidates.contains(id),
                        "visible member {id} at {point:?} missing from neighborhood of {query:?}",
                    );
                }
            }
        }
    }

    #[test]
    fn moved_updates_buckets_and_preserves_superset() {
        let mut grid = AoiGrid::new(AOI_X_RANGE, AOI_Y_RANGE);
        grid.insert(1u32, &p(0, 0));
        grid.insert(2u32, &p(500, 500));
        // Initially 2 is nowhere near 1.
        assert!(!grid.neighbors(&p(0, 0)).contains(&2));
        // Move 2 next to 1; it must now be a neighbor, and only appear once.
        grid.moved(&2, &p(1, 1));
        let near = grid.neighbors(&p(0, 0));
        assert!(near.contains(&2));
        assert_eq!(near.iter().filter(|id| **id == 2).count(), 1);
        assert_eq!(grid.len(), 2);
    }

    #[test]
    fn remove_clears_member_and_empty_cell() {
        let mut grid = AoiGrid::new(AOI_X_RANGE, AOI_Y_RANGE);
        grid.insert(7u32, &p(10, 10));
        grid.remove(&7);
        assert_eq!(grid.len(), 0);
        assert!(grid.neighbors(&p(10, 10)).is_empty());
        // Removing again is a no-op.
        grid.remove(&7);
        assert_eq!(grid.len(), 0);
    }

    #[test]
    fn handles_negative_coordinates() {
        let mut grid = AoiGrid::new(AOI_X_RANGE, AOI_Y_RANGE);
        grid.insert(1u32, &p(-5, -5));
        grid.insert(2u32, &p(-3, -4));
        assert!(grid.neighbors(&p(-5, -5)).contains(&2));
    }

    #[test]
    fn insert_twice_relocates_without_duplicating() {
        let mut grid = AoiGrid::new(AOI_X_RANGE, AOI_Y_RANGE);
        grid.insert(1u32, &p(0, 0));
        grid.insert(1u32, &p(300, 300)); // same id, new position
        assert_eq!(grid.len(), 1);
        assert!(grid.neighbors(&p(0, 0)).is_empty());
        assert!(grid.neighbors(&p(300, 300)).contains(&1));
    }
}
