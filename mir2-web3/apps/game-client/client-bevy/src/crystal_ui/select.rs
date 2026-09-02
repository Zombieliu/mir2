//! Crystal-authored character-select presentation for the native shell.
//!
//! The module mirrors `Crystal/Client/MirScenes/SelectScene.cs` at the fixed
//! 1024x768 logical stage. It consumes the authoritative `NativeShellModel` and
//! emits typed UI actions; it never creates characters or starts a game itself.

use bevy::prelude::*;
use bevy::text::LineBreak;
use bevy::ui::{widget::NodeImageMode, Node, PositionType, Val};
use chrono::{DateTime, Local, Utc};

use crate::native_shell::{CharacterSummary, NativeShellModel};

use super::assets::{frame_asset_path, CrystalButtonAssetSet};
use super::preview_data::{preview_frames, preview_overlay_frames, PreviewFrame};
use super::spec::{character_select as spec, CrystalFrameSpec, CrystalRect};
use super::typography::{crystal_text_font, CRYSTAL_DEFAULT_FONT_SIZE_PX};
use super::widget::spawn_crystal_image_button;

const WHITE: Color = Color::WHITE;
const ERROR: Color = Color::srgb(1.0, 0.35, 0.28);
const DOTNET_TICKS_MASK: u64 = 0x3fff_ffff_ffff_ffff;
const DOTNET_KIND_MASK: u64 = 0xc000_0000_0000_0000;
const DOTNET_KIND_UTC: u64 = 0x4000_0000_0000_0000;
const DOTNET_KIND_LOCAL: u64 = 0x8000_0000_0000_0000;
const DOTNET_UNIX_EPOCH_TICKS: i128 = 621_355_968_000_000_000;
const DOTNET_TICKS_PER_SECOND: i128 = 10_000_000;

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
    /// Only the base layer owns the clock. Optional Crystal overlays follow
    /// the same committed frame so a slower-loading weapon/effect layer cannot
    /// drift one frame behind the body.
    animation: Option<Timer>,
    /// Strong handles keep all 16 Crystal frames resident for the lifetime of
    /// the preview. Loading a fresh handle only at each tick allowed the prior
    /// frame to unload and exposed blank frames as a continuous flicker.
    frame_images: Vec<Handle<Image>>,
}

impl CrystalCharacterPreview {
    fn new(asset_server: &AssetServer, frame_set_base: u16, drives_clock: bool) -> Self {
        Self {
            frame_set_base,
            frame: 0,
            animation: drives_clock.then(|| {
                Timer::from_seconds(spec::PREVIEW_FRAME_DELAY_SECONDS, TimerMode::Repeating)
            }),
            frame_images: (0..spec::PREVIEW_FRAME_COUNT)
                .map(|frame| asset_server.load(preview_frame_asset_path(frame_set_base, frame)))
                .collect(),
        }
    }
}

fn preview_frame_asset_path(frame_set_base: u16, frame: usize) -> String {
    format!("original-ui/ChrSel/{}.png", frame_set_base + frame as u16)
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
    spawn_vertical_centered_text(
        parent,
        "Legend of Mir 2",
        spec::SERVER_LABEL,
        CRYSTAL_DEFAULT_FONT_SIZE_PX,
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
        spawn_vertical_centered_text(
            parent,
            "Last Online:",
            spec::LAST_ACCESS_LABEL,
            CRYSTAL_DEFAULT_FONT_SIZE_PX,
            WHITE,
            Justify::Left,
        );
        spawn_vertical_centered_text(
            parent,
            &format_last_access(character.last_access_binary_datetime),
            spec::LAST_ACCESS_VALUE,
            CRYSTAL_DEFAULT_FONT_SIZE_PX,
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
            spawn_relative_text(
                contents,
                &character.name,
                spec::SLOT_NAME,
                CRYSTAL_DEFAULT_FONT_SIZE_PX,
                WHITE,
            );
            spawn_relative_text(
                contents,
                &character.level.to_string(),
                spec::SLOT_LEVEL,
                CRYSTAL_DEFAULT_FONT_SIZE_PX,
                WHITE,
            );
            spawn_relative_text(
                contents,
                &character.class_name,
                spec::SLOT_CLASS,
                CRYSTAL_DEFAULT_FONT_SIZE_PX,
                WHITE,
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
    for (frame_set_base, frame, drives_clock) in preview_layer_specs(base) {
        spawn_preview_layer(parent, asset_server, frame_set_base, frame, drives_clock);
    }
}

fn preview_layer_specs(base: u16) -> Vec<(u16, PreviewFrame, bool)> {
    let Some(frames) = preview_frames(base) else {
        return Vec::new();
    };
    let mut layers = vec![(base, frames[0], true)];
    if let Some((overlay_base, overlay_frames)) = preview_overlay_frames(base) {
        layers.push((overlay_base, overlay_frames[0], false));
    }
    layers
}

fn spawn_preview_layer(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    frame_set_base: u16,
    frame: PreviewFrame,
    drives_clock: bool,
) {
    let rect = preview_rect(frame);
    let preview = CrystalCharacterPreview::new(asset_server, frame_set_base, drives_clock);
    let first_frame = preview.frame_images[0].clone();
    parent.spawn((
        preview,
        absolute_node(rect),
        ImageNode {
            image: first_frame,
            ..default()
        },
    ));
}

pub fn animate_character_previews(
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    mut previews: Query<(&mut CrystalCharacterPreview, &mut Node, &mut ImageNode)>,
) {
    let mut layers = previews.iter_mut().collect::<Vec<_>>();
    let Some(driver_index) = layers
        .iter()
        .position(|(preview, _, _)| preview.animation.is_some())
    else {
        return;
    };

    let (finished, current_frame) = {
        let (preview, _, _) = &mut layers[driver_index];
        let finished = preview
            .animation
            .as_mut()
            .expect("preview clock driver should own a timer")
            .tick(time.delta())
            .times_finished_this_tick();
        (finished, preview.frame)
    };

    if finished > 0 {
        let next_frame = (current_frame + finished as usize) % spec::PREVIEW_FRAME_COUNT;
        let all_layers_ready = layers.iter().all(|(preview, _, _)| {
            asset_server.is_loaded_with_dependencies(preview.frame_images[next_frame].id())
        });
        if all_layers_ready {
            for (preview, _, image) in &mut layers {
                preview.frame = next_frame;
                image.image = preview.frame_images[next_frame].clone();
            }
        }
    }

    for (preview, mut node, _) in layers {
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

fn format_last_access(binary_datetime: i64) -> String {
    if binary_datetime == 0 {
        return "Never".to_string();
    }

    let bits = binary_datetime as u64;
    let kind = bits & DOTNET_KIND_MASK;
    let ticks = i128::from(bits & DOTNET_TICKS_MASK);
    let unix_ticks = ticks - DOTNET_UNIX_EPOCH_TICKS;
    let seconds = unix_ticks.div_euclid(DOTNET_TICKS_PER_SECOND);
    let nanos = unix_ticks
        .rem_euclid(DOTNET_TICKS_PER_SECOND)
        .saturating_mul(100);
    let Ok(seconds) = i64::try_from(seconds) else {
        return "Never".to_string();
    };
    let Ok(nanos) = u32::try_from(nanos) else {
        return "Never".to_string();
    };
    let Some(utc) = DateTime::<Utc>::from_timestamp(seconds, nanos) else {
        return "Never".to_string();
    };

    match kind {
        DOTNET_KIND_LOCAL => utc
            .with_timezone(&Local)
            .format("%Y/%m/%d %H:%M:%S")
            .to_string(),
        DOTNET_KIND_UTC => utc.format("%Y/%m/%d %H:%M:%S").to_string(),
        _ => utc.naive_utc().format("%Y/%m/%d %H:%M:%S").to_string(),
    }
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
    let mut node = absolute_node(rect);
    node.overflow = Overflow::clip();
    parent.spawn((
        node,
        Text::new(value.to_owned()),
        crystal_text_font(font_size),
        TextColor(color),
        TextLayout::new(justify, LineBreak::NoWrap),
        TextShadow {
            offset: Vec2::splat(1.0),
            color: Color::BLACK,
        },
    ));
}

fn spawn_vertical_centered_text(
    parent: &mut ChildSpawnerCommands,
    value: &str,
    rect: CrystalRect,
    font_size: f32,
    color: Color,
    justify: Justify,
) {
    let mut container = vertical_centered_text_container(rect, justify);
    container.overflow = Overflow::clip();
    parent.spawn((container,)).with_children(|text_root| {
        text_root.spawn((
            Node::default(),
            Text::new(value.to_owned()),
            crystal_text_font(font_size),
            TextColor(color),
            TextLayout::new(Justify::Left, LineBreak::NoWrap),
            TextShadow {
                offset: Vec2::splat(1.0),
                color: Color::BLACK,
            },
        ));
    });
}

fn vertical_centered_text_container(rect: CrystalRect, justify: Justify) -> Node {
    let mut node = absolute_node(rect);
    node.align_items = AlignItems::Center;
    node.justify_content = match justify {
        Justify::Center => JustifyContent::Center,
        Justify::Right | Justify::End => JustifyContent::FlexEnd,
        Justify::Justified | Justify::Left | Justify::Start => JustifyContent::FlexStart,
    };
    node
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

    #[test]
    fn preview_frame_paths_cover_the_resident_crystal_animation_set() {
        let paths = (0..spec::PREVIEW_FRAME_COUNT)
            .map(|frame| preview_frame_asset_path(20, frame))
            .collect::<Vec<_>>();
        assert_eq!(paths.len(), 16);
        assert_eq!(
            paths.first().map(String::as_str),
            Some("original-ui/ChrSel/20.png")
        );
        assert_eq!(
            paths.last().map(String::as_str),
            Some("original-ui/ChrSel/35.png")
        );
    }

    #[test]
    fn layered_crystal_previews_have_one_shared_clock_driver() {
        let layers = preview_layer_specs(40);
        assert_eq!(layers.len(), 2, "male Wizard has body and weapon layers");
        assert_eq!(
            layers.iter().filter(|(_, _, drives)| *drives).count(),
            1,
            "layered previews must advance atomically from one clock"
        );
        assert_eq!(layers[0].0, 40);
        assert_eq!(layers[1].0, 600);
    }

    #[test]
    fn crystal_last_access_formats_binary_datetime_and_preserves_never() {
        assert_eq!(format_last_access(0), "Never");
        assert_eq!(
            format_last_access(621_355_968_000_000_000),
            "1970/01/01 00:00:00"
        );
        assert_eq!(
            format_last_access((621_355_968_000_000_000_u64 | DOTNET_KIND_UTC) as i64),
            "1970/01/01 00:00:00"
        );
    }

    #[test]
    fn crystal_vertical_centered_text_uses_the_source_control_alignment() {
        let left = vertical_centered_text_container(spec::LAST_ACCESS_VALUE, Justify::Left);
        assert_eq!(left.align_items, AlignItems::Center);
        assert_eq!(left.justify_content, JustifyContent::FlexStart);
        assert_eq!(left.height, Val::Px(21.0));

        let centered = vertical_centered_text_container(spec::SERVER_LABEL, Justify::Center);
        assert_eq!(centered.align_items, AlignItems::Center);
        assert_eq!(centered.justify_content, JustifyContent::Center);
    }
}
