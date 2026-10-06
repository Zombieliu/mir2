//! Lightweight WebAssembly adapter for shared map route search.
//!
//! `edges` has one byte per cell. Bits 0 through 7 mean that a step from that
//! cell is allowed in the Native compass order N, NE, E, SE, S, SW, W, NW.
//! A returned path is advisory; it never sends or authorizes movement.

use mir2_client_core::map_route::{search, SearchError};
use wasm_bindgen::prelude::*;

const MAX_MAP_CELLS: usize = 16 * 1024 * 1024;

#[wasm_bindgen(js_name = getMir2MapRouteVersion)]
pub fn get_mir2_map_route_version() -> u32 {
    1
}

/// Returns `[status, x1, y1, ...]`: 0 success, 1 invalid/outside map,
/// 2 bounded search exhausted, 3 unreachable. A same-tile route is `[0]`.
#[wasm_bindgen(js_name = getMir2MapRoutePlan)]
pub fn get_mir2_map_route_plan(
    width: i32,
    height: i32,
    origin_x: i32,
    origin_y: i32,
    goal_x: i32,
    goal_y: i32,
    edges: &[u8],
) -> Vec<i32> {
    let (Ok(width_usize), Ok(height_usize)) =
        (usize::try_from(width), usize::try_from(height)) else {
            return vec![status(SearchError::OutsideMap)];
        };
    if width_usize == 0 || height_usize == 0 {
        return vec![status(SearchError::OutsideMap)];
    }
    let Some(cell_count) = width_usize.checked_mul(height_usize) else {
        return vec![status(SearchError::OutsideMap)];
    };
    if cell_count > MAX_MAP_CELLS || cell_count != edges.len() {
        return vec![status(SearchError::OutsideMap)];
    }

    let width = width as usize;
    let blocked = |from: (i32, i32), to: (i32, i32)| {
        let dx = to.0 - from.0;
        let dy = to.1 - from.1;
        let direction = match (dx, dy) {
            (0, -1) => 0, (1, -1) => 1, (1, 0) => 2, (1, 1) => 3,
            (0, 1) => 4, (-1, 1) => 5, (-1, 0) => 6, (-1, -1) => 7,
            _ => return true,
        };
        let index = (from.1 as usize) * width + (from.0 as usize);
        edges[index] & (1 << direction) == 0
    };
    match search(
        width as i32,
        height,
        (origin_x, origin_y),
        (goal_x, goal_y),
        blocked,
    ) {
        Ok(points) => {
            let mut result = Vec::with_capacity(1 + points.len() * 2);
            result.push(0);
            for (x, y) in points {
                result.push(x);
                result.push(y);
            }
            result
        }
        Err(error) => vec![status(error)],
    }
}

fn status(error: SearchError) -> i32 {
    match error {
        SearchError::OutsideMap => 1,
        SearchError::BudgetExceeded => 2,
        SearchError::Unreachable => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_edges(width: usize, height: usize) -> Vec<u8> {
        vec![u8::MAX; width * height]
    }

    #[test]
    fn bridge_keeps_native_direction_order_and_path_points() {
        assert_eq!(get_mir2_map_route_version(), 1);
        let result = get_mir2_map_route_plan(3, 3, 1, 2, 1, 0, &open_edges(3, 3));
        assert_eq!(result, vec![0, 1, 1, 1, 0]);
    }

    #[test]
    fn bridge_rejects_bad_dimensions_cell_counts_and_coordinates() {
        assert_eq!(get_mir2_map_route_plan(0, 3, 0, 0, 0, 0, &[]), vec![1]);
        assert_eq!(get_mir2_map_route_plan(2, 2, 0, 0, 1, 1, &[u8::MAX; 3]), vec![1]);
        assert_eq!(get_mir2_map_route_plan(2, 2, -1, 0, 1, 1, &[u8::MAX; 4]), vec![1]);
        assert_eq!(get_mir2_map_route_plan(i32::MAX, i32::MAX, 0, 0, 1, 1, &[]), vec![1]);
        assert_eq!(get_mir2_map_route_plan(4097, 4097, 0, 0, 1, 1, &[]), vec![1]);
    }

    #[test]
    fn bridge_distinguishes_unreachable_from_outside_map() {
        assert_eq!(get_mir2_map_route_plan(2, 2, 0, 0, 1, 1, &[0; 4]), vec![3]);
        assert_eq!(get_mir2_map_route_plan(2, 2, 0, 0, 2, 1, &[u8::MAX; 4]), vec![1]);
    }

    #[test]
    fn bridge_enforces_the_fixed_expansion_budget() {
        let width = 512;
        let height = 512;
        let mut edges = open_edges(width, height);
        let goal = (width - 1, height - 1);
        edges[goal.1 * width + goal.0] = 0;
        for (dx, dy, bit) in [
            (0, -1, 4), (1, -1, 5), (1, 0, 6), (1, 1, 7),
            (0, 1, 0), (-1, 1, 1), (-1, 0, 2), (-1, -1, 3),
        ] {
            let x = goal.0 as isize + dx;
            let y = goal.1 as isize + dy;
            if x < 0 || y < 0 || x >= width as isize || y >= height as isize {
                continue;
            }
            let (x, y) = (x as usize, y as usize);
            edges[y * width + x] &= !(1 << bit);
        }
        assert_eq!(get_mir2_map_route_plan(width as i32, height as i32, 0, 0,
            goal.0 as i32, goal.1 as i32, &edges), vec![2]);
    }
}
