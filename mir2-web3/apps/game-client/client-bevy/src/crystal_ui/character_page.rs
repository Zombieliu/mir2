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

fn crystal_character_cosmetic_armour_frame(ui: &UiReadModel) -> Option<CrystalFrameSpec> {
    let image = ui.player.cosmetic_armour_image?;
    let female = match crystal_character_gender_offset(ui.player.gender.as_deref())? {
        0 => false,
        1 => true,
        _ => return None,
    };
    let class = ui.player.class_name.as_deref()?.trim();
    let classic = ["warrior", "wizard", "taoist"]
        .iter()
        .any(|known| class.eq_ignore_ascii_case(known));
    let alternate = ["assassin", "archer"]
        .iter()
        .any(|known| class.eq_ignore_ascii_case(known));
    if !classic && !alternate {
        return None;
    }
    // Exact intrinsic geometry from original-ui/StateItem/meta.json. These four
    // frames are the bounded atelier art catalogue, not equipment or stat data.
    let rect = match (image, female) {
        (60, false) => CrystalRect::new(92.0, 194.0, 80.0, 128.0),
        (80, true) => CrystalRect::new(93.0, 195.0, 80.0, 127.0),
        (595, false) if classic => CrystalRect::new(90.0, 180.0, 84.0, 145.0),
        (605, true) if classic => CrystalRect::new(94.0, 178.0, 80.0, 145.0),
        _ => return None,
    };
    Some(CrystalFrameSpec::new("StateItem", image as u16, rect))
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

    // Keep the real armour's wing rule, weapon and helmet-or-hair ordering.
    // A wardrobe image overrides only the painted body, including when no real
    // armour is worn; it never creates an equipment cell or its attributes.
    if equipped(1).is_some() {
        if let Some(frame) =
            crystal_character_wing_frame(ui.player.wing_effect, ui.player.gender.as_deref())
        {
            layers.push(CrystalCharacterLayer::draw_blend(frame));
        }
    }
    if let Some(frame) = crystal_character_cosmetic_armour_frame(ui)
        .or_else(|| equipped(1).and_then(crystal_character_state_item_frame))
    {
        layers.push(CrystalCharacterLayer::alpha(frame));
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

#[cfg(test)]
mod cosmetic_tests {
    use super::*;

    fn real_armour() -> ItemModel {
        ItemModel {
            key: "real-armour".into(),
            name: "Real armour".into(),
            quantity: 1,
            container: 2,
            slot: 1,
            unique_id: Some(712),
            state_image: 61,
            state_image_width: 80,
            state_image_height: 128,
            durability_current: Some(321),
            durability_max: Some(4000),
            ..default()
        }
    }

    #[test]
    fn cosmetic_paper_doll_covers_supported_classes_genders_without_equipment_mutation() {
        for class in ["warrior", "wizard", "taoist", "assassin", "archer"] {
            for (gender, traveler, gala) in [("male", 60, 595), ("female", 80, 605)] {
                for image in [traveler, gala] {
                    let mut inventory = InventoryModel::default();
                    inventory.items = vec![real_armour()];
                    let before = serde_json::to_value(&inventory).unwrap();
                    let mut ui = UiReadModel::default();
                    ui.player.class_name = Some(class.into());
                    ui.player.gender = Some(gender.into());
                    ui.player.cosmetic_armour_image = Some(image);
                    let layers = crystal_character_paper_doll_layers(&inventory, &ui);
                    let expected = if image == gala && matches!(class, "assassin" | "archer") {
                        61
                    } else {
                        image as u16
                    };
                    assert_eq!(layers.len(), 1);
                    assert_eq!(layers[0].frame.library, "StateItem");
                    assert_eq!(layers[0].frame.index, expected);
                    assert_eq!(serde_json::to_value(&inventory).unwrap(), before);
                }
            }
        }
    }

    #[test]
    fn cosmetic_paper_doll_without_armour_and_clear_use_current_real_armour() {
        let mut inventory = InventoryModel::default();
        inventory.items.clear();
        let mut ui = UiReadModel::default();
        ui.player.class_name = Some("Warrior".into());
        ui.player.gender = Some("Male".into());
        ui.player.cosmetic_armour_image = Some(595);
        ui.player.wing_effect = Some(1);
        let layers = crystal_character_paper_doll_layers(&inventory, &ui);
        assert_eq!(layers.len(), 1, "fashion cannot invent real-armour wings");
        assert_eq!(layers[0].frame.index, 595);
        inventory.items.push(real_armour());
        ui.player.wing_effect = None;
        assert_eq!(
            crystal_character_paper_doll_layers(&inventory, &ui)[0]
                .frame
                .index,
            595
        );
        inventory.items[0].state_image = 740;
        assert_eq!(
            crystal_character_paper_doll_layers(&inventory, &ui)[0]
                .frame
                .index,
            595
        );
        ui.player.cosmetic_armour_image = None;
        assert_eq!(
            crystal_character_paper_doll_layers(&inventory, &ui)[0]
                .frame
                .index,
            740
        );
        inventory.items.clear();
        assert!(crystal_character_paper_doll_layers(&inventory, &ui).is_empty());
    }

    #[test]
    fn cosmetic_paper_doll_refuses_unknown_wrong_gender_or_missing_identity_art() {
        let mut inventory = InventoryModel::default();
        inventory.items = vec![real_armour()];
        let mut ui = UiReadModel::default();
        ui.player.class_name = Some("Warrior".into());
        ui.player.gender = Some("Male".into());
        for image in [0, 80, 605, 65_536, u32::MAX] {
            ui.player.cosmetic_armour_image = Some(image);
            assert_eq!(
                crystal_character_paper_doll_layers(&inventory, &ui)[0]
                    .frame
                    .index,
                61
            );
        }
        ui.player.cosmetic_armour_image = Some(60);
        ui.player.class_name = None;
        assert_eq!(
            crystal_character_paper_doll_layers(&inventory, &ui)[0]
                .frame
                .index,
            61
        );
        ui.player.class_name = Some("Unknown".into());
        assert_eq!(
            crystal_character_paper_doll_layers(&inventory, &ui)[0]
                .frame
                .index,
            61
        );
        ui.player.class_name = Some("Warrior".into());
        ui.player.gender = None;
        assert_eq!(
            crystal_character_paper_doll_layers(&inventory, &ui)[0]
                .frame
                .index,
            61
        );
    }
}
