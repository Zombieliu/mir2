//! One authored Character paper-doll plan and actual painter for both hosts.
use super::{
    character_materials::CrystalCharacterWingMaterials,
    spec::{CrystalFrameSpec, CrystalRect},
};
use crate::inventory::{InventoryModel, ItemModel};
use crate::read_model::UiReadModel;
use bevy::prelude::*;

pub const CRYSTAL_CHARACTER_EQUIPMENT_SLOTS: [(u32, CrystalRect); 14] = [
    (0, CrystalRect::new(131.0, 97.0, 36.0, 32.0)),
    (1, CrystalRect::new(171.0, 97.0, 36.0, 32.0)),
    (2, CrystalRect::new(211.0, 97.0, 36.0, 32.0)),
    (13, CrystalRect::new(211.0, 152.0, 36.0, 32.0)),
    (4, CrystalRect::new(211.0, 188.0, 36.0, 32.0)),
    (3, CrystalRect::new(211.0, 224.0, 36.0, 32.0)),
    (5, CrystalRect::new(16.0, 260.0, 36.0, 32.0)),
    (6, CrystalRect::new(211.0, 260.0, 36.0, 32.0)),
    (7, CrystalRect::new(16.0, 296.0, 36.0, 32.0)),
    (8, CrystalRect::new(211.0, 296.0, 36.0, 32.0)),
    (9, CrystalRect::new(16.0, 332.0, 36.0, 32.0)),
    (11, CrystalRect::new(56.0, 332.0, 36.0, 32.0)),
    (10, CrystalRect::new(96.0, 332.0, 36.0, 32.0)),
    (12, CrystalRect::new(136.0, 332.0, 36.0, 32.0)),
];
const CRYSTAL_MALE_HAIR_RECTS: [CrystalRect; 9] = [
    CrystalRect::new(131.0, 173.0, 16.0, 14.0),
    CrystalRect::new(127.0, 170.0, 20.0, 33.0),
    CrystalRect::new(127.0, 174.0, 24.0, 16.0),
    CrystalRect::new(118.0, 157.0, 36.0, 37.0),
    CrystalRect::new(118.0, 157.0, 36.0, 37.0),
    CrystalRect::new(118.0, 157.0, 36.0, 37.0),
    CrystalRect::new(128.0, 173.0, 20.0, 23.0),
    CrystalRect::new(128.0, 173.0, 20.0, 23.0),
    CrystalRect::new(128.0, 173.0, 20.0, 22.0),
];
const CRYSTAL_ASSASSIN_MALE_HAIR_RECTS: [CrystalRect; 9] = [
    CrystalRect::new(125.0, 147.0, 16.0, 21.0),
    CrystalRect::new(120.0, 146.0, 28.0, 31.0),
    CrystalRect::new(118.0, 150.0, 28.0, 26.0),
    CrystalRect::new(104.0, 126.0, 44.0, 46.0),
    CrystalRect::new(104.0, 126.0, 44.0, 46.0),
    CrystalRect::new(104.0, 126.0, 44.0, 46.0),
    CrystalRect::new(123.0, 149.0, 20.0, 26.0),
    CrystalRect::new(123.0, 149.0, 20.0, 26.0),
    CrystalRect::new(123.0, 149.0, 20.0, 26.0),
];
const CRYSTAL_FEMALE_HAIR_RECTS: [CrystalRect; 9] = [
    CrystalRect::new(126.0, 171.0, 24.0, 25.0),
    CrystalRect::new(128.0, 171.0, 20.0, 24.0),
    CrystalRect::new(116.0, 160.0, 40.0, 38.0),
    CrystalRect::new(126.0, 161.0, 28.0, 29.0),
    CrystalRect::new(126.0, 161.0, 28.0, 29.0),
    CrystalRect::new(126.0, 161.0, 28.0, 29.0),
    CrystalRect::new(116.0, 167.0, 44.0, 31.0),
    CrystalRect::new(116.0, 167.0, 44.0, 31.0),
    CrystalRect::new(118.0, 168.0, 40.0, 30.0),
];
const CRYSTAL_ASSASSIN_FEMALE_HAIR_RECTS: [CrystalRect; 9] = [
    CrystalRect::new(122.0, 156.0, 24.0, 24.0),
    CrystalRect::new(125.0, 155.0, 20.0, 23.0),
    CrystalRect::new(122.0, 149.0, 24.0, 32.0),
    CrystalRect::new(122.0, 139.0, 32.0, 37.0),
    CrystalRect::new(122.0, 139.0, 32.0, 37.0),
    CrystalRect::new(122.0, 139.0, 32.0, 37.0),
    CrystalRect::new(114.0, 149.0, 40.0, 33.0),
    CrystalRect::new(114.0, 149.0, 40.0, 33.0),
    CrystalRect::new(114.0, 149.0, 40.0, 33.0),
];
pub(crate) fn crystal_character_gender_offset(gender: Option<&str>) -> Option<u16> {
    let gender = gender?.trim();
    if gender.eq_ignore_ascii_case("male") {
        Some(0)
    } else if gender.eq_ignore_ascii_case("female") {
        Some(1)
    } else {
        None
    }
}

pub(crate) fn crystal_character_page_index(gender: Option<&str>) -> u16 {
    // A missing legacy gender keeps the structural page usable, while the
    // actual appearance layers below remain fail-closed.
    340 + crystal_character_gender_offset(gender).unwrap_or_default()
}

pub(crate) fn crystal_character_hair_frame(
    class_name: Option<&str>,
    gender: Option<&str>,
    hair: Option<u8>,
) -> Option<CrystalFrameSpec> {
    let gender_offset = crystal_character_gender_offset(gender)?;
    let hair = usize::from(hair?);
    if hair >= CRYSTAL_MALE_HAIR_RECTS.len() {
        return None;
    }
    let assassin = class_name
        .map(str::trim)
        .is_some_and(|value| value.eq_ignore_ascii_case("assassin"));
    let (base, mut rect) = match (gender_offset, assassin) {
        (0, false) => (441, CRYSTAL_MALE_HAIR_RECTS[hair]),
        (0, true) => (461, CRYSTAL_ASSASSIN_MALE_HAIR_RECTS[hair]),
        (1, false) => (481, CRYSTAL_FEMALE_HAIR_RECTS[hair]),
        (1, true) => (501, CRYSTAL_ASSASSIN_FEMALE_HAIR_RECTS[hair]),
        _ => return None,
    };
    // CharacterDialog.cs applies these offsets on top of the source frame's
    // intrinsic `useOffset=true` x/y for Assassin hair only.
    if assassin {
        rect.left += if gender_offset == 0 { 6.0 } else { 4.0 };
        rect.top += if gender_offset == 0 { 25.0 } else { 18.0 };
    }
    Some(CrystalFrameSpec::new("Prguse", base + hair as u16, rect))
}

pub(crate) fn crystal_character_state_item_frame(item: &ItemModel) -> Option<CrystalFrameSpec> {
    if item.state_image == 0 || item.state_image_width == 0 || item.state_image_height == 0 {
        return None;
    }
    Some(CrystalFrameSpec::new(
        "StateItem",
        item.state_image,
        CrystalRect::new(
            item.state_image_x as f32,
            item.state_image_y as f32,
            item.state_image_width as f32,
            item.state_image_height as f32,
        ),
    ))
}

pub(crate) fn crystal_character_wing_frame(
    wing_effect: Option<u8>,
    gender: Option<&str>,
) -> Option<CrystalFrameSpec> {
    let gender_offset = crystal_character_gender_offset(gender)?;
    let wing_offset = match wing_effect? {
        1 => 2,
        2 => 4,
        _ => return None,
    };
    let index = 1200 + wing_offset + gender_offset;
    // Exact `Prguse2` intrinsic rectangles (`useOffset=true`) exported from
    // the same Crystal library consumed by CharacterDialog.cs.
    let rect = match index {
        1202 => CrystalRect::new(64.0, 138.0, 148.0, 139.0),
        1203 => CrystalRect::new(64.0, 145.0, 148.0, 144.0),
        1204 => CrystalRect::new(55.0, 140.0, 156.0, 185.0),
        1205 => CrystalRect::new(56.0, 144.0, 156.0, 185.0),
        _ => return None,
    };
    Some(CrystalFrameSpec::new("Prguse2", index, rect))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CrystalCharacterBlend {
    Alpha,
    DrawBlend,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CrystalCharacterLayer {
    pub(crate) frame: CrystalFrameSpec,
    pub(crate) blend: CrystalCharacterBlend,
}

impl CrystalCharacterLayer {
    const fn alpha(frame: CrystalFrameSpec) -> Self {
        Self {
            frame,
            blend: CrystalCharacterBlend::Alpha,
        }
    }

    const fn draw_blend(frame: CrystalFrameSpec) -> Self {
        Self {
            frame,
            blend: CrystalCharacterBlend::DrawBlend,
        }
    }
}

pub(crate) fn spawn_character_layer(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    wing_materials: Option<&CrystalCharacterWingMaterials>,
    layer: CrystalCharacterLayer,
) {
    match layer.blend {
        CrystalCharacterBlend::Alpha => {
            spawn_character_alpha(
                parent,
                asset_server,
                layer.frame.asset_path(),
                layer.frame.rect,
            );
        }
        CrystalCharacterBlend::DrawBlend => {
            let Some(material) =
                wing_materials.and_then(|materials| materials.get(layer.frame.index))
            else {
                return;
            };
            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(layer.frame.rect.left),
                    top: Val::Px(layer.frame.rect.top),
                    width: Val::Px(layer.frame.rect.width),
                    height: Val::Px(layer.frame.rect.height),
                    ..default()
                },
                MaterialNode(material.clone()),
            ));
        }
    }
}

pub(crate) fn crystal_character_paper_doll_layers(
    inventory: &InventoryModel,
    ui: &UiReadModel,
) -> Vec<CrystalCharacterLayer> {
    let equipped = |slot| {
        inventory
            .items
            .iter()
            .find(|item| item.container == 2 && item.slot == slot)
    };
    let mut layers = Vec::with_capacity(4);

    // Exact CharacterPage.AfterDraw condition and order: a wing is legal only
    // while an armour item exists, then armour, weapon, and helmet-or-hair.
    if let Some(armour) = equipped(1) {
        if let Some(frame) =
            crystal_character_wing_frame(ui.player.wing_effect, ui.player.gender.as_deref())
        {
            layers.push(CrystalCharacterLayer::draw_blend(frame));
        }
        if let Some(frame) = crystal_character_state_item_frame(armour) {
            layers.push(CrystalCharacterLayer::alpha(frame));
        }
    }
    if let Some(frame) = equipped(0).and_then(crystal_character_state_item_frame) {
        layers.push(CrystalCharacterLayer::alpha(frame));
    }
    if let Some(helmet) = equipped(2) {
        if let Some(frame) = crystal_character_state_item_frame(helmet) {
            layers.push(CrystalCharacterLayer::alpha(frame));
        }
    } else if let Some(frame) = crystal_character_hair_frame(
        ui.player.class_name.as_deref(),
        ui.player.gender.as_deref(),
        ui.player.hair,
    ) {
        layers.push(CrystalCharacterLayer::alpha(frame));
    }
    layers
}

pub fn render_character_paper_doll(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    wing_materials: Option<&CrystalCharacterWingMaterials>,
    inventory: &InventoryModel,
    ui: &UiReadModel,
) {
    for layer in crystal_character_paper_doll_layers(inventory, ui) {
        spawn_character_layer(parent, asset_server, wing_materials, layer);
    }
}

fn spawn_character_alpha(
    parent: &mut ChildSpawnerCommands,
    server: &AssetServer,
    path: String,
    rect: CrystalRect,
) {
    parent.spawn((
        super::shared_hud::absolute_node(rect),
        bevy::ui::FocusPolicy::Pass,
        ImageNode {
            image: server.load(path),
            image_mode: bevy::ui::widget::NodeImageMode::Stretch,
            ..default()
        },
    ));
}

#[cfg(test)]
#[path = "character_page_tests.rs"]
mod tests;
