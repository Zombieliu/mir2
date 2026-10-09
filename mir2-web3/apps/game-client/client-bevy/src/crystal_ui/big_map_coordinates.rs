//! Big Map mouse coordinates use the same displayed-image projection as navigation.
//! Keeping their label separate from the map render key avoids rebuilding every
//! NPC, button and image whenever the pointer crosses a tile.

use super::{
    render_bigmap, BigMapUiState, NativePlayerUiState, OverlayBigMap, CRYSTAL_BIGMAP_PANEL_RECT,
};
use crate::big_map::{BigMapImageGeometry, BigMapModel, BigMapView};
use crate::crystal_ui::metrics::CrystalStageTransform;
use crate::native_shell::{NativeShellModel, NativeShellScreen};
use crate::read_model::UiReadModel;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

#[derive(Component)]
pub(super) struct CoordinateLabel;

#[derive(Default)]
pub(super) struct RenderCache {
    key: Option<RenderKey>,
}

struct RenderKey {
    root: Entity,
    model: BigMapModel,
    renderer: BigMapUiState,
    fallback_title: Option<String>,
    has_assets: bool,
}

impl RenderCache {
    pub(super) fn reset(&mut self) {
        self.key = None;
    }
}

pub(super) fn fill_panel(
    commands: &mut Commands,
    query: &mut Query<(Entity, &mut Node), With<OverlayBigMap>>,
    visible: bool,
    model: &BigMapModel,
    renderer: &BigMapUiState,
    ui: &UiReadModel,
    assets: Option<&AssetServer>,
    cache: &mut RenderCache,
) {
    let Some((root, mut node)) = query.iter_mut().next() else {
        cache.reset();
        return;
    };
    let was_visible = node.display != Display::None;
    let display = if visible {
        Display::Flex
    } else {
        Display::None
    };
    if node.display != display {
        node.display = display;
    }
    if !visible {
        if cache.key.take().is_some() || was_visible {
            commands.entity(root).despawn_children();
        }
        return;
    }
    if cache.key.as_ref().is_some_and(|key| {
        key.root == root
            && key.model == *model
            && key.renderer == *renderer
            && key.fallback_title == ui.player.map_name
            && key.has_assets == assets.is_some()
    }) {
        return;
    }
    commands.entity(root).despawn_children();
    commands.entity(root).with_children(|parent| {
        render_bigmap(parent, assets, model, renderer, ui);
    });
    cache.key = Some(RenderKey {
        root,
        model: model.clone(),
        renderer: renderer.clone(),
        fallback_title: ui.player.map_name.clone(),
        has_assets: assets.is_some(),
    });
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct CoordinateScene {
    epoch: u64,
    current_map: Option<i32>,
    active_map: i32,
    image: u32,
    width: i32,
    height: i32,
}

impl CoordinateScene {
    fn for_model(model: &BigMapModel) -> Option<Self> {
        if model.view == BigMapView::WorldMap {
            return None;
        }
        let map = model.active_map()?;
        if map.info.width <= 0 || map.info.height <= 0 {
            return None;
        }
        BigMapImageGeometry::for_model(model)?;
        Some(Self {
            epoch: model.reset_epoch,
            current_map: model.current_map_index,
            active_map: map.map_index,
            image: map.info.big_map_image_index()?,
            width: map.info.width,
            height: map.info.height,
        })
    }
}

#[derive(Default)]
pub(super) struct PointerCoordinates {
    scene: Option<CoordinateScene>,
    tile: Option<(i32, i32)>,
    label: String,
}

impl PointerCoordinates {
    fn set_tile(&mut self, tile: Option<(i32, i32)>) {
        if self.tile != tile {
            self.tile = tile;
            self.label = tile
                .map(|(x, y)| format!("[ {x}, {y} ]"))
                .unwrap_or_default();
        }
    }
}

fn tile_under_cursor(window: &Window, model: &BigMapModel) -> Option<(i32, i32)> {
    let cursor = window.cursor_position()?;
    let transform =
        CrystalStageTransform::fit(window.resolution.width(), window.resolution.height());
    if !transform.contains_physical_point(cursor.x, cursor.y) {
        return None;
    }
    let logical = transform.physical_to_logical(cursor.x, cursor.y);
    let geometry = BigMapImageGeometry::for_model(model)?;
    let image_point = geometry.image_point((
        logical.0 - CRYSTAL_BIGMAP_PANEL_RECT.left,
        logical.1 - CRYSTAL_BIGMAP_PANEL_RECT.top,
    ))?;
    let map = model.active_map()?;
    geometry.tile_at(image_point, map.info.width, map.info.height)
}

pub(super) fn update(
    shell: Res<NativeShellModel>,
    ui: Res<NativePlayerUiState>,
    model: Res<BigMapModel>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut pointer: Local<PointerCoordinates>,
    mut labels: Query<&mut Text, With<CoordinateLabel>>,
) {
    let window = windows.single().ok();
    let scene = (shell.screen == NativeShellScreen::InGame
        && ui.bigmap_open()
        && window.is_some_and(|window| window.focused))
    .then(|| CoordinateScene::for_model(&model))
    .flatten();
    if pointer.scene != scene {
        pointer.scene = scene;
        pointer.set_tile(None);
    }
    if scene.is_some() {
        if let Some(tile) = window.and_then(|window| tile_under_cursor(window, &model)) {
            pointer.set_tile(Some(tile));
        }
        // Crystal keeps the last valid mouse coordinate on MouseLeave. The
        // scene key above prevents that value from leaking across maps or sessions.
    }
    for mut text in &mut labels {
        if text.0 != pointer.label {
            text.0.clone_from(&pointer.label);
        }
    }
}

#[cfg(test)]
#[path = "big_map_coordinates_tests.rs"]
mod tests;
