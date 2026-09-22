//! Native BigMap image input and bounded full-map routes (no teleport intents).
use super::*;
use mir2_client_bevy::big_map::{BigMapImageGeometry, BigMapModel, BigMapView};
use mir2_client_bevy::chat::{ChatLine, ChatModel};
use mir2_client_bevy::crystal_ui::overlays::CRYSTAL_BIGMAP_PANEL_RECT;

const SEARCH_BUDGET: usize = 250_000;

#[derive(Clone, Copy, Debug)]
pub(super) struct HuntArea {
    pub center: (i32, i32),
    pub radius: i32,
}

#[derive(Debug)]
pub(super) struct MapRoute {
    pub map_file: String,
    pub map_index: i32,
    pub origin: (i32, i32),
    pub destination: (i32, i32),
    pub steps: Vec<(i32, i32)>,
    pub hunt_area: Option<HuntArea>,
}

pub(super) fn image_position(window: &Window, model: &BigMapModel) -> Option<(f32, f32)> {
    let cursor = window.cursor_position()?;
    let transform = mir2_client_bevy::crystal_ui::metrics::CrystalStageTransform::fit(
        window.resolution.width(),
        window.resolution.height(),
    );
    let (x, y) = transform.physical_to_logical(cursor.x, cursor.y);
    BigMapImageGeometry::for_model(model)?.image_point((
        x - CRYSTAL_BIGMAP_PANEL_RECT.left,
        y - CRYSTAL_BIGMAP_PANEL_RECT.top,
    ))
}

/// The Big Map panel is the only overlay whose close and control presses may
/// preserve an already-running map route. A press outside this rectangle is a
/// normal world action and must retain the usual cancellation behavior.
pub(super) fn cursor_over_panel(window: &Window) -> bool {
    let Some(cursor) = window.cursor_position() else {
        return false;
    };
    let transform = mir2_client_bevy::crystal_ui::metrics::CrystalStageTransform::fit(
        window.resolution.width(),
        window.resolution.height(),
    );
    if !transform.contains_physical_point(cursor.x, cursor.y) {
        return false;
    }
    let (x, y) = transform.physical_to_logical(cursor.x, cursor.y);
    CRYSTAL_BIGMAP_PANEL_RECT.contains(x, y)
}

pub(super) fn destination(
    model: &BigMapModel,
    point: (f32, f32),
) -> Result<(i32, i32), &'static str> {
    if model.view == BigMapView::WorldMap || model.active_map_index != model.current_map_index {
        return Err("只能在当前地图内自动寻路。");
    }
    let map = model.active_map().ok_or("地图信息尚未加载。");
    let map = map?;
    let geometry = BigMapImageGeometry::for_model(model).ok_or("地图图片尺寸尚未加载。");
    geometry?
        .tile_at(point, map.info.width, map.info.height)
        .ok_or("请选择有效地图图像内的位置。")
}

pub(super) fn feedback(chat: Option<&mut ChatModel>, message: &str) {
    if let Some(chat) = chat {
        chat.push(ChatLine {
            text: message.to_owned(),
            channel: "system".to_owned(),
        });
    }
}

pub(super) fn ui_blocks_except_map(ui: &NativePlayerUiState) -> bool {
    let mut guard = ui.clone();
    if guard.bigmap_open() {
        guard.core.panel = mir2_ui_core::state::UiPanel::None;
    }
    guard.blocks_world_click()
}

// Eight-way Crystal pathing, bounded by actual map edges rather than the
// short NewMove click radius. The search budget bounds memory/latency on a
// pathological maze; exceeding it is reported instead of publishing a route.
fn search(
    width: i32,
    height: i32,
    origin: (i32, i32),
    destination: (i32, i32),
    blocked: impl FnMut((i32, i32), (i32, i32)) -> bool,
) -> Result<Vec<(i32, i32)>, &'static str> {
    search_region(width, height, origin, destination, 0, blocked)
}

// A hunting destination is the imported square spawn area, not a monster's
// occupied tile. One A* search retains the existing total expansion budget.
fn search_region(
    width: i32,
    height: i32,
    origin: (i32, i32),
    destination: (i32, i32),
    radius: i32,
    mut blocked: impl FnMut((i32, i32), (i32, i32)) -> bool,
) -> Result<Vec<(i32, i32)>, &'static str> {
    let inside = |p: (i32, i32)| p.0 >= 0 && p.1 >= 0 && p.0 < width && p.1 < height;
    if !inside(origin) || !inside(destination) || radius < 0 || radius > width.max(height) {
        return Err("目标不在当前地图范围内。");
    }
    let distance = |point| chebyshev_distance(point, destination).saturating_sub(radius).max(0);
    if distance(origin) == 0 {
        return Ok(Vec::new());
    }
    let mut open = BinaryHeap::new();
    let mut costs = HashMap::from([(origin, 0)]);
    let mut previous = HashMap::new();
    open.push(Reverse((
        distance(origin),
        0,
        0_u64,
        origin,
    )));
    let mut expanded = 0;
    let mut sequence = 0_u64;
    while let Some(Reverse((_, negative_cost, _, current))) = open.pop() {
        let cost = -negative_cost;
        if costs.get(&current) != Some(&cost) {
            continue;
        }
        if distance(current) == 0 {
            let mut steps = Vec::new();
            let mut p = current;
            while p != origin {
                steps.push(p);
                p = previous[&p];
            }
            steps.reverse();
            return Ok(steps);
        }
        expanded += 1;
        if expanded > SEARCH_BUDGET {
            return Err("寻路范围过于复杂，请选择更近的目标。");
        }
        let preferred = movement_direction_toward(Some(destination), current).unwrap();
        for rotation in [0, 1, -1, 2, -2, 3, -3, 4] {
            let (dx, dy) = direction_to_delta(rotate_direction(preferred, rotation).unwrap());
            let next = (current.0 + dx, current.1 + dy);
            let next_cost = cost + 1;
            if !inside(next)
                || blocked(current, next)
                || costs.get(&next).is_some_and(|old| *old <= next_cost)
            {
                continue;
            }
            costs.insert(next, next_cost);
            previous.insert(next, current);
            // Prefer deeper nodes for equal estimates, avoiding a broad
            // plateau scan on large open maps. Stable direction order keeps
            // unobstructed routes straight instead of zigzagging on ties.
            sequence += 1;
            open.push(Reverse((
                next_cost + distance(next),
                -next_cost,
                sequence,
                next,
            )));
        }
    }
    Err("无法找到通往目标的路径。")
}

pub(super) fn plan(
    movement: &WorldPointerMovementState,
    entities: &EntityModelSet,
    presentation: &NativeEntityPresentation,
    self_id: &str,
    map_file: &str,
    origin: (i32, i32),
    destination: (i32, i32),
) -> Result<Vec<(i32, i32)>, &'static str> {
    let map = crate::map_parser::load_map(map_file).ok_or("当前地图的寻路数据尚未加载。");
    let map = map?;
    if map.cell_blocks_movement(destination.0, destination.1) {
        return Err("目标位置有障碍，无法到达。");
    }
    if entity_blocks_movement(entities, Some(presentation), self_id, destination) {
        return Err("入口当前被实体占用，请稍后重试。");
    }
    search(
        i32::from(map.width),
        i32::from(map.height),
        origin,
        destination,
        |from, to| {
            map.cell_blocks_movement(to.0, to.1)
                || auto_path_step_blocked(
                    movement,
                    entities,
                    Some(presentation),
                    self_id,
                    None,
                    from,
                    to,
                )
        },
    )
}

pub(super) fn plan_hunt_region(
    movement: &WorldPointerMovementState,
    entities: &EntityModelSet,
    presentation: &NativeEntityPresentation,
    self_id: &str,
    map_file: &str,
    origin: (i32, i32),
    area: HuntArea,
) -> Result<Vec<(i32, i32)>, &'static str> {
    let map = crate::map_parser::load_map(map_file).ok_or("当前地图的寻路数据尚未加载。")?;
    if chebyshev_distance(origin, area.center) <= area.radius
        && (map.cell_blocks_movement(origin.0, origin.1)
            || entity_blocks_movement(entities, Some(presentation), self_id, origin))
    {
        return Err("当前位置被占用，无法确认到达狩猎区域。");
    }
    search_region(i32::from(map.width), i32::from(map.height), origin, area.center, area.radius,
        |from, to| map.cell_blocks_movement(to.0, to.1)
            || auto_path_step_blocked(movement, entities, Some(presentation), self_id, None, from, to))
}

pub(super) fn advance(
    movement: &mut WorldPointerMovementState,
    entities: &EntityModelSet,
    presentation: &NativeEntityPresentation,
    self_id: &str,
    origin: (i32, i32),
) -> Result<Vec<(i32, i32)>, &'static str> {
    let route = movement.map_auto_path.as_ref().ok_or("自动寻路已取消。");
    let route = route?;
    let index = if origin == route.origin {
        Some(0)
    } else {
        route
            .steps
            .iter()
            .position(|point| *point == origin)
            .map(|i| i + 1)
    };
    if let Some(index) = index {
        let remaining = &route.steps[index..];
        let mut from = origin;
        let next_clear = remaining.iter().take(3).all(|to| {
            let clear = !auto_path_step_blocked(
                movement,
                entities,
                Some(presentation),
                self_id,
                Some(&route.map_file),
                from,
                *to,
            );
            from = *to;
            clear
        });
        if next_clear {
            return Ok(remaining.iter().take(3).copied().collect());
        }
    }
    let map_file = route.map_file.clone();
    let destination = route.destination;
    let steps = if let Some(area) = route.hunt_area {
        plan_hunt_region(movement, entities, presentation, self_id, &map_file, origin, area)?
    } else {
        plan(movement, entities, presentation, self_id, &map_file, origin, destination)?
    };
    let destination = steps.last().copied().unwrap_or(origin);
    let next = steps.iter().take(3).copied().collect();
    let route = movement.map_auto_path.as_mut().unwrap();
    route.origin = origin;
    route.steps = steps;
    route.destination = destination;
    movement.auto_path_destination = Some(destination);
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hunt_navigation_region_search_avoids_occupied_goal_tiles_and_reports_unreachable_areas() {
        let steps = search_region(80, 80, (10, 30), (50, 30), 4,
            |_, to| to.0 == 46 && to.1 == 30).unwrap();
        let destination = *steps.last().unwrap();
        assert!(chebyshev_distance(destination, (50, 30)) <= 4);
        assert_ne!(destination, (46, 30));
        assert!(!steps.contains(&(46, 30)));
        assert_eq!(steps.len(), 36, "shortest route to the square, not to its occupied center");
        assert!(search_region(80, 80, (50, 30), (50, 30), 4, |_, _| false).unwrap().is_empty());
        assert!(search_region(80, 80, (10, 30), (50, 30), 4,
            |_, to| chebyshev_distance(to, (50, 30)) <= 4).is_err());
        assert!(search_region(80, 80, (10, 30), (50, 30), -1, |_, _| false).is_err());
    }

    #[test]
    fn long_route_passes_gap_and_does_not_use_twenty_tile_click_limit() {
        assert_eq!(
            search(80, 80, (10, 10), (16, 10), |_, _| false).unwrap(),
            (11..=16).map(|x| (x, 10)).collect::<Vec<_>>()
        );
        let route = search(700, 700, (10, 10), (650, 10), |_, to| {
            to.0 == 300 && to.1 != 50
        })
        .unwrap();
        assert_eq!(route.last(), Some(&(650, 10)));
        assert!(route.contains(&(300, 50)));
        assert!(route.len() > 20);
        assert!(search(80, 80, (10, 10), (60, 10), |_, to| to.0 == 30).is_err());
        assert!(search(80, 80, (10, 10), (80, 10), |_, _| false).is_err());
    }
}
