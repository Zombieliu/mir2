//! Crystal-authored character-select presentation for the native shell.
//!
//! The module mirrors `Crystal/Client/MirScenes/SelectScene.cs` at the fixed
//! 1024x768 logical stage. It consumes the authoritative `NativeShellModel` and
//! emits typed UI actions; it never creates characters or starts a game itself.

use bevy::prelude::*;
use bevy::ui::{widget::NodeImageMode, Node, PositionType, Val};

use crate::native_shell::{CharacterSummary, NativeShellModel};

use super::assets::{frame_asset_path, CrystalButtonAssetSet};
use super::preview_data::{preview_frames, preview_overlay_frames, PreviewFrame};
use super::spec::{character_select as spec, CrystalFrameSpec, CrystalRect};
use super::widget::spawn_crystal_image_button;

const WHITE: Color = Color::srgb(0.94, 0.94, 0.94);
const MUTED_GOLD: Color = Color::srgb(0.81, 0.73, 0.58);
const ERROR: Color = Color::srgb(1.0, 0.35, 0.28);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrystalSelectAction {
    SelectCharacter(i32),
    Start,
    NewCharacter,
    DeleteCharacter,
    Credits,
    Exit,
}

#[derive(Component, Debug)]
pub struct CrystalCharacterPreview {
    frame_set_base: u16,
    frame: usize,
    animation: Timer,
}

impl CrystalCharacterPreview {
    fn new(frame_set_base: u16) -> Self {
        Self {
            frame_set_base,
            frame: 0,
            animation: Timer::from_seconds(spec::PREVIEW_FRAME_DELAY_SECONDS, TimerMode::Repeating),
        }
    }
}

pub fn class_index(class_name: &str) -> u16 {
    if class_name.eq_ignore_ascii_case("Wizard") {
        1
    } else if class_name.eq_ignore_ascii_case("Taoist") {
        2
    } else if class_name.eq_ignore_ascii_case("Assassin") {
        3
    } else if class_name.eq_ignore_ascii_case("Archer") {
        4
    } else {
        0
    }
}

pub fn preview_base_index(class_name: &str, gender_name: &str) -> u16 {
    let female = gender_name.eq_ignore_ascii_case("Female");
    match (class_index(class_name), female) {
        (0, false) => 20,
        (0, true) => 300,
        (1, false) => 40,
        (1, true) => 320,
        (2, false) => 60,
        (2, true) => 340,
        (3, false) => 80,
        (3, true) => 360,
        (4, false) => 100,
        (4, true) => 140,
        _ => 20,
    }
}

pub fn slot_frame_index(character: &CharacterSummary, selected: bool) -> u16 {
    spec::occupied_slot_index(class_index(&character.class_name), selected)
}

pub fn spawn_character_select_screen(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    model: &NativeShellModel,
) {
    spawn_frame(parent, asset_server, spec::BACKGROUND);
    spawn_frame(parent, asset_server, spec::TITLE);
    spawn_text(
        parent,
        "Legend of Mir 2",
        spec::SERVER_LABEL,
        13.0,
        WHITE,
        Justify::Center,
    );

    let selected = model.selected_character_index.and_then(|index| {
        model
            .characters
            .iter()
            .find(|character| character.index == index)
    });

    if let Some(character) = selected {
        spawn_character_preview(parent, asset_server, character);
        spawn_text(
            parent,
            "Last Online:",
            spec::LAST_ACCESS_LABEL,
            9.0,
            MUTED_GOLD,
            Justify::Left,
        );
        // The current WebSocket roster does not yet expose Crystal's binary
        // LastAccess value. `Never` is the exact source fallback for zero/min.
        spawn_text(
            parent,
            "Never",
            spec::LAST_ACCESS_VALUE,
            9.0,
            WHITE,
            Justify::Left,
        );
    }

    for slot in 0..4 {
        spawn_character_slot(parent, asset_server, model, slot);
    }

    let has_selection = selected.is_some();
    spawn_crystal_image_button(
        parent,
        asset_server,
        spec::START,
        CrystalButtonAssetSet::from_spec(spec::START),
        CrystalSelectAction::Start,
        false,
        has_selection,
    );
    spawn_crystal_image_button(
        parent,
        asset_server,
        spec::NEW_CHARACTER,
        CrystalButtonAssetSet::from_spec(spec::NEW_CHARACTER),
        CrystalSelectAction::NewCharacter,
        false,
        model.characters.len() < 4,
    );
    spawn_crystal_image_button(
        parent,
        asset_server,
        spec::DELETE_CHARACTER,
        CrystalButtonAssetSet::from_spec(spec::DELETE_CHARACTER),
        CrystalSelectAction::DeleteCharacter,
        false,
        has_selection,
    );
    spawn_crystal_image_button(
        parent,
        asset_server,
        spec::CREDITS,
        CrystalButtonAssetSet::from_spec(spec::CREDITS),
        CrystalSelectAction::Credits,
        false,
        true,
    );
    spawn_crystal_image_button(
        parent,
        asset_server,
        spec::EXIT,
        CrystalButtonAssetSet::from_spec(spec::EXIT),
        CrystalSelectAction::Exit,
        false,
        true,
    );

    if let Some(notice) = &model.notice {
        spawn_text(
            parent,
            &notice.message,
            CrystalRect::new(262.0, 678.0, 500.0, 22.0),
            12.0,
            ERROR,
            Justify::Center,
        );
    }
}

fn spawn_frame(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    frame: CrystalFrameSpec,
) {
    parent.spawn((
        absolute_node(frame.rect),
        ImageNode {
            image: asset_server.load(frame_asset_path(frame)),
            image_mode: NodeImageMode::Stretch,
            ..default()
        },
    ));
}

fn spawn_character_slot(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    model: &NativeShellModel,
    slot: usize,
) {
    let top = spec::SLOT_TOPS[slot];
    let character = model.characters.get(slot);
    let selected = character
        .map(|character| Some(character.index) == model.selected_character_index)
        .unwrap_or(false);
    let rect = CrystalRect::new(
        spec::SLOT_LEFT,
        top,
        spec::SLOT_WIDTH,
        if character.is_some() {
            spec::OCCUPIED_SLOT_HEIGHT
        } else {
            spec::EMPTY_SLOT_HEIGHT
        },
    );

    let mut slot_entity = parent.spawn((absolute_node(rect),));
    if let Some(character) = character {
        slot_entity.insert((
            Button,
            CrystalSelectAction::SelectCharacter(character.index),
        ));
    }

    slot_entity.with_children(|contents| {
        let image_path = character.map_or_else(
            || format!("original-ui/Prguse/{}.png", spec::EMPTY_SLOT_INDEX),
            |character| {
                format!(
                    "original-ui/Title/{}.png",
                    slot_frame_index(character, selected)
                )
            },
        );
        contents.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(rect.width),
                height: Val::Px(rect.height),
                ..default()
            },
            ImageNode {
                image: asset_server.load(image_path),
                image_mode: NodeImageMode::Stretch,
                ..default()
            },
        ));

        if let Some(character) = character {
            spawn_relative_text(contents, &character.name, spec::SLOT_NAME, 15.0, WHITE);
            spawn_relative_text(
                contents,
                &character.level.to_string(),
                spec::SLOT_LEVEL,
                12.0,
                WHITE,
            );
            spawn_relative_text(
                contents,
                &character.class_name,
                spec::SLOT_CLASS,
                12.0,
                MUTED_GOLD,
            );
        }
    });
}

fn spawn_character_preview(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    character: &CharacterSummary,
) {
    let base = preview_base_index(&character.class_name, &character.gender_name);
    let Some(frames) = preview_frames(base) else {
        return;
    };
    spawn_preview_layer(parent, asset_server, base, frames[0]);

    if let Some((overlay_base, overlay_frames)) = preview_overlay_frames(base) {
        spawn_preview_layer(parent, asset_server, overlay_base, overlay_frames[0]);
    }
}

fn spawn_preview_layer(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    frame_set_base: u16,
    frame: PreviewFrame,
) {
    let rect = preview_rect(frame);
    parent.spawn((
        CrystalCharacterPreview::new(frame_set_base),
        absolute_node(rect),
        ImageNode {
            image: asset_server.load(format!("original-ui/ChrSel/{frame_set_base}.png")),
            ..default()
        },
    ));
}

pub fn animate_character_previews(
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    mut previews: Query<(&mut CrystalCharacterPreview, &mut Node, &mut ImageNode)>,
) {
    for (mut preview, mut node, mut image) in &mut previews {
        let finished = preview
            .animation
            .tick(time.delta())
            .times_finished_this_tick();
        if finished > 0 {
            preview.frame = (preview.frame + finished as usize) % spec::PREVIEW_FRAME_COUNT;
            image.image = asset_server.load(format!(
                "original-ui/ChrSel/{}.png",
                preview.frame_set_base + preview.frame as u16
            ));
        }

        let frame = frame_for_set(preview.frame_set_base, preview.frame);
        let rect = preview_rect(frame);
        node.left = Val::Px(rect.left);
        node.top = Val::Px(rect.top);
        node.width = Val::Px(rect.width);
        node.height = Val::Px(rect.height);
    }
}

fn frame_for_set(frame_set_base: u16, frame: usize) -> PreviewFrame {
    preview_frames(frame_set_base)
        .or_else(|| match frame_set_base {
            600 => preview_overlay_frames(40).map(|value| value.1),
            880 => preview_overlay_frames(320).map(|value| value.1),
            _ => None,
        })
        .expect("spawned Crystal preview frame set must have source metadata")[frame]
}

fn preview_rect(frame: PreviewFrame) -> CrystalRect {
    CrystalRect::new(
        spec::PREVIEW_ANCHOR.0 + frame.x,
        spec::PREVIEW_ANCHOR.1 + frame.y,
        frame.width,
        frame.height,
    )
}

fn absolute_node(rect: CrystalRect) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(rect.left),
        top: Val::Px(rect.top),
        width: Val::Px(rect.width),
        height: Val::Px(rect.height),
        ..default()
    }
}

fn spawn_text(
    parent: &mut ChildSpawnerCommands,
    value: &str,
    rect: CrystalRect,
    font_size: f32,
    color: Color,
    justify: Justify,
) {
    parent.spawn((
        absolute_node(rect),
        Text::new(value.to_owned()),
        TextFont {
            font_size: FontSize::Px(font_size),
            ..default()
        },
        TextColor(color),
        TextLayout::justify(justify),
        TextShadow {
            offset: Vec2::splat(1.0),
            color: Color::BLACK,
        },
    ));
}

fn spawn_relative_text(
    parent: &mut ChildSpawnerCommands,
    value: &str,
    rect: CrystalRect,
    font_size: f32,
    color: Color,
) {
    spawn_text(parent, value, rect, font_size, color, Justify::Left);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn character(class_name: &str, gender_name: &str) -> CharacterSummary {
        CharacterSummary::new(7, "VisualHero", 12, class_name, gender_name)
    }

    #[test]
    fn class_and_gender_map_to_crystal_preview_bases() {
        assert_eq!(preview_base_index("Warrior", "Male"), 20);
        assert_eq!(preview_base_index("Warrior", "Female"), 300);
        assert_eq!(preview_base_index("Wizard", "Male"), 40);
        assert_eq!(preview_base_index("Wizard", "Female"), 320);
        assert_eq!(preview_base_index("Taoist", "Male"), 60);
        assert_eq!(preview_base_index("Assassin", "Female"), 360);
        assert_eq!(preview_base_index("Archer", "Male"), 100);
        assert_eq!(preview_base_index("Archer", "Female"), 140);
    }

    #[test]
    fn slot_frame_maps_class_and_selection_state() {
        assert_eq!(slot_frame_index(&character("Warrior", "Male"), false), 660);
        assert_eq!(slot_frame_index(&character("Wizard", "Female"), true), 666);
        assert_eq!(slot_frame_index(&character("Archer", "Female"), true), 669);
    }

    #[test]
    fn first_preview_frame_applies_crystal_use_offset_anchor() {
        let warrior = preview_frames(20).unwrap()[0];
        assert_eq!(
            preview_rect(warrior),
            CrystalRect::new(177.0, 270.0, 196.0, 302.0)
        );
        let wizard_overlay = preview_overlay_frames(40).unwrap().1[0];
        assert_eq!(
            preview_rect(wizard_overlay),
            CrystalRect::new(170.0, 176.0, 164.0, 392.0)
        );
    }
}
