//! Pure full-map eight-way search shared by ordinary Quest navigation hosts.
//! Collision is supplied by the host; a route grants no movement authority.

use std::{cmp::Reverse, collections::{BinaryHeap, HashMap}};

pub const SEARCH_BUDGET: usize = 250_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchError { OutsideMap, BudgetExceeded, Unreachable }

impl SearchError {
    pub fn native_message(self) -> &'static str {
        match self {
            Self::OutsideMap => "目标不在当前地图范围内。",
            Self::BudgetExceeded => "寻路范围过于复杂，请选择更近的目标。",
            Self::Unreachable => "无法找到通往目标的路径。",
        }
    }
}

pub fn search(
    width: i32, height: i32, origin: (i32, i32), destination: (i32, i32),
    blocked: impl FnMut((i32, i32), (i32, i32)) -> bool,
) -> Result<Vec<(i32, i32)>, SearchError> {
    search_region(width, height, origin, destination, 0, blocked)
}

/// Search the imported square area, including any unoccupied reachable tile.
/// An already-inside origin must be checked by the host before this function.
pub fn search_region(
    width: i32, height: i32, origin: (i32, i32), destination: (i32, i32), radius: i32,
    blocked: impl FnMut((i32, i32), (i32, i32)) -> bool,
) -> Result<Vec<(i32, i32)>, SearchError> {
    search_region_with_budget(width, height, origin, destination, radius, blocked, SEARCH_BUDGET)
}

fn search_region_with_budget(
    width: i32, height: i32, origin: (i32, i32), destination: (i32, i32), radius: i32,
    mut blocked: impl FnMut((i32, i32), (i32, i32)) -> bool, budget: usize,
) -> Result<Vec<(i32, i32)>, SearchError> {
    let inside = |p: (i32, i32)| p.0 >= 0 && p.1 >= 0 && p.0 < width && p.1 < height;
    if !inside(origin) || !inside(destination) || radius < 0 || radius > width.max(height) {
        return Err(SearchError::OutsideMap);
    }
    let distance = |point: (i32, i32)| chebyshev(point, destination).saturating_sub(radius).max(0);
    if distance(origin) == 0 { return Ok(Vec::new()); }
    let mut open = BinaryHeap::new();
    let mut costs = HashMap::from([(origin, 0)]);
    let mut previous = HashMap::new();
    open.push(Reverse((distance(origin), 0, 0_u64, origin)));
    let mut expanded = 0;
    let mut sequence = 0_u64;
    while let Some(Reverse((_, negative_cost, _, current))) = open.pop() {
        let cost = -negative_cost;
        if costs.get(&current) != Some(&cost) { continue; }
        if distance(current) == 0 {
            let mut steps = Vec::new();
            let mut p = current;
            while p != origin { steps.push(p); p = previous[&p]; }
            steps.reverse();
            return Ok(steps);
        }
        expanded += 1;
        if expanded > budget { return Err(SearchError::BudgetExceeded); }
        let preferred: i32 = match ((destination.0-current.0).signum(), (destination.1-current.1).signum()) {
            (0,-1) => 0, (1,-1) => 1, (1,0) => 2, (1,1) => 3,
            (0,1) => 4, (-1,1) => 5, (-1,0) => 6, (-1,-1) => 7,
            _ => unreachable!("a goal node was already returned"),
        };
        const DELTAS: [(i32, i32); 8] = [(0,-1),(1,-1),(1,0),(1,1),(0,1),(-1,1),(-1,0),(-1,-1)];
        for rotation in [0, 1, -1, 2, -2, 3, -3, 4] {
            let (dx, dy) = DELTAS[(preferred + rotation).rem_euclid(8) as usize];
            let next = (current.0 + dx, current.1 + dy);
            let next_cost = cost + 1;
            if !inside(next) || blocked(current, next)
                || costs.get(&next).is_some_and(|old| *old <= next_cost) { continue; }
            costs.insert(next, next_cost);
            previous.insert(next, current);
            // Native tie-breaking prefers deeper nodes, then stable direction order.
            sequence += 1;
            open.push(Reverse((next_cost + distance(next), -next_cost, sequence, next)));
        }
    }
    Err(SearchError::Unreachable)
}

fn chebyshev(left: (i32, i32), right: (i32, i32)) -> i32 {
    left.0.abs_diff(right.0).max(left.1.abs_diff(right.1)) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_chooses_free_shortest_edge_instead_of_occupied_center() {
        let steps = search_region(80,80,(10,30),(50,30),4,|_,to| to==(46,30)).unwrap();
        assert_eq!(steps.len(),36);
        assert!(chebyshev(*steps.last().unwrap(),(50,30))<=4);
        assert!(!steps.contains(&(46,30)));
    }

    #[test]
    fn region_requires_reachable_tile_and_valid_center_and_radius() {
        assert_eq!(search_region(80,80,(10,30),(50,30),4,|_,to| chebyshev(to,(50,30))<=4),Err(SearchError::Unreachable));
        assert_eq!(search_region(80,80,(10,30),(80,30),4,|_,_| false),Err(SearchError::OutsideMap));
        assert_eq!(search_region(80,80,(10,30),(50,30),-1,|_,_| false),Err(SearchError::OutsideMap));
        assert!(search_region(80,80,(50,30),(50,30),4,|_,_| false).unwrap().is_empty());
    }

    #[test]
    fn exact_path_stays_straight_and_long_path_passes_only_gap() {
        assert_eq!(search(80,80,(10,10),(16,10),|_,_| false).unwrap(),(11..=16).map(|x|(x,10)).collect::<Vec<_>>());
        let steps=search(700,700,(10,10),(650,10),|_,to| to.0==300 && to.1!=50).unwrap();
        assert_eq!(steps.last(),Some(&(650,10)));
        assert!(steps.contains(&(300,50)));
        assert!(steps.len()>20);
    }

    #[test]
    fn exhausted_budget_never_returns_best_progress() {
        assert_eq!(search_region_with_budget(80,80,(10,10),(60,10),0,|_,_| false,5),Err(SearchError::BudgetExceeded));
    }

    #[test]
    fn directed_rejected_edge_can_choose_another_step() {
        let steps=search(80,80,(10,10),(16,10),|from,to| from==(10,10) && to==(11,10)).unwrap();
        assert_ne!(steps.first(),Some(&(11,10)));
        assert_eq!(steps.last(),Some(&(16,10)));
    }

    #[test]
    fn region_replan_selects_another_endpoint_with_original_radius() {
        let initial=search_region(80,80,(10,30),(50,30),4,|_,_| false).unwrap();
        let endpoint=*initial.last().unwrap();
        let replanned=search_region(80,80,(45,30),(50,30),4,|_,to| to==endpoint).unwrap();
        assert_ne!(replanned.last(),Some(&endpoint));
        assert!(chebyshev(*replanned.last().unwrap(),(50,30))<=4);
        assert_eq!(replanned.len(),1);
    }
}
