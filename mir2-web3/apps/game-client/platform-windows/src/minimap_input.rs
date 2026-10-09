//! Mini Map image clicks use the last rendered crop, then ordinary map pathing.
use super::*;
use mir2_client_bevy::big_map::BigMapModel;
use mir2_client_bevy::crystal_ui::minimap::{mini_map_contains, MiniMapViewState};
use mir2_client_bevy::map::MapModel;

fn logical_cursor(window: &Window) -> Option<bevy::prelude::Vec2> {
    let cursor = window.cursor_position()?;
    let transform = mir2_client_bevy::crystal_ui::CrystalStageTransform::fit(
        window.resolution.width(),
        window.resolution.height(),
    );
    if !window.focused || !transform.contains_physical_point(cursor.x, cursor.y) {
        return None;
    }
    let (x, y) = transform.physical_to_logical(cursor.x, cursor.y);
    Some(bevy::prelude::Vec2::new(x, y))
}

pub(super) fn contains(
    window: &Window,
    ui: &NativePlayerUiState,
    map: &MapModel,
    displayed: Option<&MiniMapViewState>,
) -> bool {
    let expanded = !ui.local_keys.camera_hidden
        && mir2_client_bevy::crystal_ui::hud::minimap_is_expanded(
            ui.minimap_visible(),
            map.mini_map_index,
            map.map_width,
            map.map_height,
        );
    // A map-change packet can clear metadata in PreUpdate while the old
    // image is still on screen. Consume that click and let identity validation
    // reject it; it must not become a click on the world under the old HUD.
    (expanded || displayed.is_some_and(|state| state.displayed.is_some()))
        && logical_cursor(window).is_some_and(mini_map_contains)
}

pub(super) fn frame_contains(window: &Window, expanded: bool) -> bool {
    let mut frame = spec::hud::MINIMAP.rect;
    if !expanded {
        frame.height = 45.0;
    }
    logical_cursor(window).is_some_and(|point| frame.contains(point.x, point.y))
}

pub(super) fn destination(
    window: &Window,
    displayed: Option<&MiniMapViewState>,
    map: &MapModel,
    identity: &BigMapModel,
) -> Result<(i32, (i32, i32)), &'static str> {
    let view = displayed
        .and_then(|state| state.displayed)
        .ok_or("小地图图片尚未加载，请稍后重试。");
    let view = view?;
    if !view.matches_map(map, identity) {
        return Err("小地图正在切换，请稍后重试。");
    }
    let tile = logical_cursor(window)
        .and_then(|point| view.tile_at(point))
        .ok_or("请选择小地图图像内的位置。");
    Ok((view.map_index, tile?))
}
