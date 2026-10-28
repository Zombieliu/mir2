//! Crystal-authored character-select presentation for the native shell.
//!
//! The module mirrors `Crystal/Client/MirScenes/SelectScene.cs` at the fixed
//! 1024x768 logical stage. It consumes the authoritative `NativeShellModel` and
//! emits typed UI actions; it never creates characters or starts a game itself.

use bevy::prelude::*;
use bevy::text::LineBreak;
use bevy::ui::{widget::NodeImageMode, Node, PositionType, Val};
use chrono::{DateTime, Local, Utc};

use crate::native_i18n;
use crate::native_shell::{CharacterSummary, NativeShellModel};

use super::assets::{frame_asset_path, CrystalButtonAssetSet};
use super::overlays::CrystalAdditiveUiMaterial;
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

/// Fixed materials keep the Wizard's additive frames resident across animation
/// ticks and name-field redraws. Creating a new material each tick can expose a
/// frame before the GPU has prepared its effect layer.
#[derive(Resource)]
pub(crate) struct CrystalPreviewMaterials {
    male_wizard: [Handle<CrystalAdditiveUiMaterial>; spec::PREVIEW_FRAME_COUNT],
    female_wizard: [Handle<CrystalAdditiveUiMaterial>; spec::PREVIEW_FRAME_COUNT],
}

impl CrystalPreviewMaterials {
    fn get(&self, frame_set_base: u16, frame: usize) -> Option<&Handle<CrystalAdditiveUiMaterial>> {
        match frame_set_base {
            600 => self.male_wizard.get(frame),
            880 => self.female_wizard.get(frame),
            _ => None,
        }
    }
}

pub(crate) fn load_character_preview_materials(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let load_set = |base| {
        std::array::from_fn(|frame| {
            asset_server.add(CrystalAdditiveUiMaterial {
                image: asset_server.load(preview_frame_asset_path(base, frame)),
            })
        })
    };
    commands.insert_resource(CrystalPreviewMaterials {
        male_wizard: load_set(600),
        female_wizard: load_set(880),
    });
}

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
    anchor: (f32, f32),
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

/// Keep the control's clock when name/focus edits rebuild its surrounding UI.
/// Changing class, gender or the create/select anchor starts a fresh sequence.
#[derive(Clone)]
pub(crate) struct PreviewAnimationState {
    frame_set_base: u16,
    anchor: (f32, f32),
    frame: usize,
    timer: Timer,
}

impl PreviewAnimationState {
    fn applies_to(&self, preview: &CrystalCharacterPreview) -> bool {
        self.frame_set_base == preview.frame_set_base && self.anchor == preview.anchor
    }

    fn restore(&self, preview: &mut CrystalCharacterPreview) {
        if self.applies_to(preview) {
            preview.frame = self.frame;
            preview.animation = Some(self.timer.clone());
        }
    }
}

impl CrystalCharacterPreview {
    fn new(
        asset_server: &AssetServer,
        frame_set_base: u16,
        anchor: (f32, f32),
        drives_clock: bool,
    ) -> Self {
        Self {
            frame_set_base,
            anchor,
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
    if native_i18n::active() {
        spawn_text(
            parent,
            &native_i18n::tr("Select Character"),
            CrystalRect::new(312.0, 16.0, 400.0, 34.0),
            22.0,
            WHITE,
            Justify::Center,
        );
    } else {
        spawn_frame(parent, asset_server, spec::TITLE);
    }
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
            &native_i18n::tr("Last Online:"),
            spec::LAST_ACCESS_LABEL,
            CRYSTAL_DEFAULT_FONT_SIZE_PX,
            WHITE,
            Justify::Left,
        );
        spawn_vertical_centered_text(
            parent,
            &native_i18n::tr(&format_last_access(character.last_access_binary_datetime)),
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
            &native_i18n::tr(&notice.message),
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
        if native_i18n::active() {
            // The source slot art contains English labels. Keep the same hit
            // target while rendering labels separately from opaque player names.
            contents.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(if selected {
                    Color::srgb(0.20, 0.16, 0.07)
                } else {
                    Color::srgb(0.055, 0.045, 0.03)
                }),
                BorderColor::all(Color::srgb(0.65, 0.5, 0.22)),
            ));
            if let Some(character) = character {
                spawn_relative_text(
                    contents,
                    &character.name,
                    CrystalRect::new(12.0, 6.0, 264.0, 20.0),
                    14.0,
                    WHITE,
                );
                spawn_relative_text(
                    contents,
                    &native_i18n::tr(&character.class_name),
                    CrystalRect::new(12.0, 31.0, 174.0, 20.0),
                    13.0,
                    WHITE,
                );
                let level = native_i18n::key("shell.level", "Lv. {level}")
                    .replace("{level}", &character.level.to_string());
                spawn_relative_text(
                    contents,
                    &level,
                    CrystalRect::new(194.0, 31.0, 80.0, 20.0),
                    13.0,
                    WHITE,
                );
            } else {
                spawn_relative_text(
                    contents,
                    &native_i18n::tr("Empty Slot"),
                    CrystalRect::new(12.0, 16.0, 264.0, 24.0),
                    14.0,
                    WHITE,
                );
            }
            return;
        }
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
    spawn_character_preview_at(
        parent,
        asset_server,
        &character.class_name,
        &character.gender_name,
        spec::PREVIEW_ANCHOR,
    );
}

/// Spawns Crystal's offset-aware animated character preview at a control
/// anchor. `MirAnimatedControl.UseOffSet` treats the source location as an
/// anchor; the individual `ChrSel` frame offset determines its top-left.
pub fn spawn_character_preview_at(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    class_name: &str,
    gender_name: &str,
    anchor: (f32, f32),
) {
    let base = preview_base_index(class_name, gender_name);
    for (frame_set_base, frame, drives_clock) in preview_layer_specs(base) {
        spawn_preview_layer(
            parent,
            asset_server,
            frame_set_base,
            frame,
            anchor,
            drives_clock,
        );
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
    anchor: (f32, f32),
    drives_clock: bool,
) {
    let rect = preview_rect_at(anchor, frame);
    let preview = CrystalCharacterPreview::new(asset_server, frame_set_base, anchor, drives_clock);
    let first_frame = preview.frame_images[0].clone();
    let mut layer = parent.spawn((preview, absolute_node(rect)));
    if drives_clock {
        layer.insert(ImageNode {
            image: first_frame,
            ..default()
        });
    }
    // Wizard overlays are attached as MaterialNodes before UI extraction by
    // animate_character_previews. They must never use ordinary ImageNode
    // alpha blending: opaque dark glow pixels would cover the weapon itself.
}

pub(crate) fn animate_character_previews(
    mut commands: Commands,
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    materials: Res<CrystalPreviewMaterials>,
    mut retained: bevy::prelude::Local<Option<PreviewAnimationState>>,
    mut previews: Query<(
        Entity,
        &mut CrystalCharacterPreview,
        &mut Node,
        Option<&mut ImageNode>,
        Option<&mut MaterialNode<CrystalAdditiveUiMaterial>>,
    )>,
) {
    let mut layers = previews.iter_mut().collect::<Vec<_>>();
    let Some(driver_index) = layers
        .iter()
        .position(|(_, preview, _, _, _)| preview.animation.is_some())
    else {
        *retained = None;
        return;
    };

    let (finished, current_frame) = {
        let (_, preview, _, _, _) = &mut layers[driver_index];
        if let Some(previous) = retained.as_ref() {
            previous.restore(preview);
        }
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
        let all_layers_ready = layers.iter().all(|(_, preview, _, _, _)| {
            asset_server.is_loaded_with_dependencies(preview.frame_images[next_frame].id())
                && materials
                    .get(preview.frame_set_base, next_frame)
                    .is_none_or(|material| asset_server.is_loaded_with_dependencies(material.id()))
        });
        if all_layers_ready {
            for (_, preview, _, _, _) in &mut layers {
                preview.frame = next_frame;
            }
        }
    }

    let committed_frame = layers[driver_index].1.frame;
    *retained = Some(PreviewAnimationState {
        frame_set_base: layers[driver_index].1.frame_set_base,
        anchor: layers[driver_index].1.anchor,
        frame: committed_frame,
        timer: layers[driver_index].1.animation.clone().unwrap(),
    });
    for (entity, mut preview, mut node, image, material_node) in layers {
        preview.frame = committed_frame;
        if let Some(mut image) = image {
            if image.image != preview.frame_images[committed_frame] {
                image.image = preview.frame_images[committed_frame].clone();
            }
        }
        if let Some(handle) = materials.get(preview.frame_set_base, preview.frame) {
            if let Some(mut material_node) = material_node {
                if material_node.0 != *handle {
                    material_node.0 = handle.clone();
                }
            } else {
                commands.entity(entity).insert(MaterialNode(handle.clone()));
            }
        }
        let frame = frame_for_set(preview.frame_set_base, preview.frame);
        let rect = preview_rect_at(preview.anchor, frame);
        node.left = Val::Px(rect.left);
        node.top = Val::Px(rect.top);
        node.width = Val::Px(rect.width);
        node.height = Val::Px(rect.height);
    }
}

#[cfg(test)]
pub(crate) fn preview_render_state_for_tests(world: &mut World) -> Vec<serde_json::Value> {
    let mut query = world.query::<(
        &CrystalCharacterPreview,
        &Node,
        Option<&ImageNode>,
        Option<&MaterialNode<CrystalAdditiveUiMaterial>>,
    )>();
    let asset_server = world.resource::<AssetServer>();
    let Some(materials) = world.get_resource::<CrystalPreviewMaterials>() else {
        assert!(
            query.iter(world).next().is_none(),
            "preview entities require their material cache"
        );
        return Vec::new();
    };
    let pixels = |value| match value {
        Val::Px(value) => value,
        _ => panic!("preview bounds must use Crystal pixel coordinates"),
    };
    let mut layers: Vec<_> = query
        .iter(world)
        .map(|(preview, node, image, material)| {
            let ready = preview.frame_images.iter().enumerate().all(|(frame, image)| {
                asset_server.is_loaded_with_dependencies(image.id())
                    && materials.get(preview.frame_set_base, frame).is_none_or(|handle| {
                        asset_server.is_loaded_with_dependencies(handle.id())
                    })
            });
            serde_json::json!({
                "base": preview.frame_set_base,
                "frame": preview.frame,
                "anchor": [preview.anchor.0, preview.anchor.1],
                "rect": [pixels(node.left), pixels(node.top), pixels(node.width), pixels(node.height)],
                "drivesClock": preview.animation.is_some(),
                "imageNode": image.is_some(),
                "additiveNode": material.is_some(),
                "allImagesReady": ready,
            })
        })
        .collect();
    layers.sort_by_key(|layer| layer["base"].as_u64());
    layers
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

fn preview_rect_at(anchor: (f32, f32), frame: PreviewFrame) -> CrystalRect {
    CrystalRect::new(
        anchor.0 + frame.x,
        anchor.1 + frame.y,
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
            preview_rect_at(spec::PREVIEW_ANCHOR, warrior),
            CrystalRect::new(177.0, 270.0, 196.0, 302.0)
        );
        assert_eq!(
            preview_rect_at((338.0, 404.0), warrior),
            CrystalRect::new(255.0, 254.0, 196.0, 302.0)
        );
        let wizard_overlay = preview_overlay_frames(40).unwrap().1[0];
        assert_eq!(
            preview_rect_at(spec::PREVIEW_ANCHOR, wizard_overlay),
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
    fn create_and_select_share_all_ten_source_body_sequences() {
        for (class, male, female) in [
            ("Warrior", 20, 300),
            ("Wizard", 40, 320),
            ("Taoist", 60, 340),
            ("Assassin", 80, 360),
            ("Archer", 100, 140),
        ] {
            for (gender, expected) in [("Male", male), ("Female", female)] {
                let base = preview_base_index(class, gender);
                assert_eq!(base, expected);
                let layers = preview_layer_specs(base);
                assert_eq!(layers.len(), if class == "Wizard" { 2 } else { 1 });
                assert_eq!(layers.iter().filter(|(_, _, driver)| *driver).count(), 1);
                assert_eq!(preview_frames(base).unwrap().len(), 16);
                for frame in 0..16 {
                    assert_eq!(
                        frame_for_set(base, frame),
                        preview_frames(base).unwrap()[frame]
                    );
                }
            }
        }
    }

    #[test]
    fn redraw_preserves_frame_and_partial_clock_but_choice_change_restarts() {
        let mut timer = Timer::from_seconds(0.25, TimerMode::Repeating);
        timer.tick(std::time::Duration::from_millis(110));
        let previous = PreviewAnimationState {
            frame_set_base: 40,
            anchor: (338.0, 404.0),
            frame: 7,
            timer,
        };
        let make_preview = |base, anchor| CrystalCharacterPreview {
            frame_set_base: base,
            anchor,
            frame: 0,
            animation: Some(Timer::from_seconds(0.25, TimerMode::Repeating)),
            frame_images: Vec::new(),
        };
        let mut redrawn = make_preview(40, (338.0, 404.0));
        previous.restore(&mut redrawn);
        assert_eq!(redrawn.frame, 7);
        assert_eq!(
            redrawn.animation.as_ref().unwrap().elapsed().as_millis(),
            110
        );
        assert!(redrawn
            .animation
            .as_mut()
            .unwrap()
            .tick(std::time::Duration::from_millis(140))
            .just_finished());
        for (base, anchor) in [
            (320, (338.0, 404.0)),
            (20, (338.0, 404.0)),
            (40, spec::PREVIEW_ANCHOR),
        ] {
            let mut changed = make_preview(base, anchor);
            previous.restore(&mut changed);
            assert_eq!(changed.frame, 0);
            assert!(changed.animation.unwrap().elapsed().is_zero());
        }
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
