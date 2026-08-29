//! Crystal-authored in-game HUD for the Windows-native host.
//!
//! This module is deliberately presentation-only.  It reads the shared UI and
//! inventory models and exposes typed button components for a later host to
//! consume; it never sends commands, changes gameplay state, or infers data
//! that is not present in the read models.

use bevy::prelude::*;
use bevy::text::LineBreak;
use bevy::ui::{widget::NodeImageMode, Display, Node, PositionType, Val};

use crate::inventory::{item_icon_path, InventoryModel, ItemModel};
use crate::map::MapModel;
use crate::native_shell::{NativeShellModel, NativeShellScreen};
use crate::read_model::UiReadModel;

use super::assets::CrystalButtonAssetSet;
use super::spec::{hud as spec, CrystalFrameSpec, CrystalRect};
use super::typography::{crystal_text_font, CRYSTAL_DEFAULT_FONT_SIZE_PX};
use super::widget::spawn_crystal_image_button;

const WHITE: Color = Color::WHITE;
pub(crate) const HUD_Z_INDEX: i32 = 950;

/// Fixed source dimensions of Crystal's `Prguse/4` orb texture.
pub const ORB_WIDTH: f32 = 104.0;
pub const ORB_HEIGHT: f32 = 80.0;
pub const ORB_HALF_WIDTH: f32 = 50.0;
pub const ORB_HP_ONLY_WIDTH: f32 = 100.0;
pub const ORB_HP_SOURCE_LEFT: f32 = 0.0;
pub const ORB_MP_SOURCE_LEFT: f32 = 51.0;
pub const ORB_TOP: f32 = 646.0;

/// The two source halves used by Crystal's bottom-clipped HP/MP orb.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrbSide {
    Hp,
    Mp,
}

/// A source-space rectangle expressed in image pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HudSourceRect {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

impl HudSourceRect {
    pub const fn new(left: f32, top: f32, width: f32, height: f32) -> Self {
        Self {
            left,
            top,
            width,
            height,
        }
    }
}

/// Source and destination rectangles for one clipped orb half.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbClipGeometry {
    pub source: HudSourceRect,
    pub destination: CrystalRect,
}

/// Return the exact 50-pixel source half used by Crystal for an orb side.
pub const fn orb_source_rect(side: OrbSide, height: f32) -> HudSourceRect {
    let left = match side {
        OrbSide::Hp => ORB_HP_SOURCE_LEFT,
        OrbSide::Mp => ORB_MP_SOURCE_LEFT,
    };
    HudSourceRect::new(left, ORB_HEIGHT - height, ORB_HALF_WIDTH, height)
}

/// Calculate a bottom-anchored Crystal orb clip using the authoritative ratio.
///
/// Crystal truncates the calculated height to an integer pixel and draws the
/// selected source rectangle at the same bottom-aligned screen position.
pub fn orb_clip_geometry(ratio: f32, side: OrbSide) -> OrbClipGeometry {
    let height = (ORB_HEIGHT * ratio.clamp(0.0, 1.0)).floor();
    let left = match side {
        OrbSide::Hp => ORB_HP_SOURCE_LEFT,
        OrbSide::Mp => ORB_MP_SOURCE_LEFT,
    };
    OrbClipGeometry {
        source: orb_source_rect(side, height),
        destination: CrystalRect::new(left, ORB_TOP + ORB_HEIGHT - height, ORB_HALF_WIDTH, height),
    }
}

/// Crystal uses `Prguse/6` as one full-width red orb for Warriors below level
/// 26. It is clipped from the bottom exactly like the split HP/MP texture.
pub fn hp_only_orb_clip_geometry(ratio: f32) -> OrbClipGeometry {
    let height = (ORB_HEIGHT * ratio.clamp(0.0, 1.0)).floor();
    OrbClipGeometry {
        source: HudSourceRect::new(0.0, ORB_HEIGHT - height, ORB_HP_ONLY_WIDTH, height),
        destination: CrystalRect::new(
            0.0,
            ORB_TOP + ORB_HEIGHT - height,
            ORB_HP_ONLY_WIDTH,
            height,
        ),
    }
}

/// Typed actions attached to native Crystal HUD controls.
///
/// The HUD only attaches these values to buttons.  A platform host may later
/// translate them into UI intents; this module intentionally has no consumer
/// that mutates a shell, inventory, or gameplay resource.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrystalHudAction {
    Character,
    Inventory,
    Skill,
    Quest,
    Option,
    Menu,
    GameShop,
    Mail,
    BigMap,
    MinimapToggle,
    /// An authoritative use request for a currently populated belt slot.
    BeltUse(u8),
}

/// Root marker for the native-only Crystal HUD.
#[derive(Component, Debug)]
pub struct CrystalHudRoot;

#[derive(Component, Debug)]
pub struct CrystalHudHpOrb;

#[derive(Component, Debug)]
pub struct CrystalHudMpOrb;

#[derive(Component, Debug)]
pub struct CrystalHudName;

#[derive(Component, Debug)]
pub struct CrystalHudLevel;

#[derive(Component, Debug)]
pub struct CrystalHudGold;

#[derive(Component, Debug)]
pub struct CrystalHudHpText;

#[derive(Component, Debug)]
pub struct CrystalHudMpText;

/// Crystal's alternate HP/MP presentation when `Settings.HPView` is disabled.
/// The original client keeps the orbs, but replaces compact labels with two
/// stacked raw-value labels.
#[derive(Component, Debug)]
pub struct CrystalHudHpAlternateTopText;

#[derive(Component, Debug)]
pub struct CrystalHudHpAlternateBottomText;

#[derive(Component, Debug)]
pub struct CrystalHudExperienceBar;

#[derive(Component, Debug)]
pub struct CrystalHudExperienceText;

#[derive(Component, Debug)]
pub struct CrystalHudWeightBar;

#[derive(Component, Debug)]
pub struct CrystalHudWeightText;

#[derive(Component, Debug)]
pub struct CrystalHudSpaceText;

#[derive(Component, Debug)]
pub struct CrystalHudMapTitle;

#[derive(Component, Debug)]
pub struct CrystalHudMapCoordinate;

#[derive(Component, Debug)]
pub struct CrystalHudMinimap;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrystalHudBeltKey {
    pub slot: u8,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrystalHudBeltItem {
    pub slot: u8,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrystalHudBeltIcon {
    pub slot: u8,
}

/// Stable slot hit target. The `Button` component is synchronized after each
/// inventory snapshot, so startup with an empty belt remains operable later.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrystalHudBeltHitTarget {
    pub slot: u8,
}

/// Belt geometry follows `Prguse/1932` and keeps six fixed 40-pixel slots.
pub const BELT_FRAME: CrystalRect = CrystalRect::new(230.0, 618.0, 240.0, 38.0);
pub const BELT_SLOT_STEP: f32 = 35.0;
pub const BELT_SLOT_COUNT: u8 = 6;

/// Exact source-relative positions from Crystal's `MainDialog`.
pub const HP_TEXT_RECT: CrystalRect = CrystalRect::new(0.0, 673.0, 100.0, 14.0);
pub const MP_TEXT_RECT: CrystalRect = CrystalRect::new(0.0, 688.0, 100.0, 14.0);
pub const LEVEL_RECT: CrystalRect = CrystalRect::new(5.0, 724.0, 30.0, 14.0);
pub const NAME_RECT: CrystalRect = CrystalRect::new(6.0, 736.0, 90.0, 16.0);
pub const GOLD_RECT: CrystalRect = CrystalRect::new(919.0, 735.0, 99.0, 13.0);
pub const EXPERIENCE_TEXT_RECT: CrystalRect = CrystalRect::new(491.0, 749.0, 40.0, 12.0);
pub const MAP_TITLE_RECT: CrystalRect = CrystalRect::new(900.0, 2.0, 120.0, 18.0);
pub const MAP_COORDINATE_RECT: CrystalRect = CrystalRect::new(944.0, 131.0, 56.0, 18.0);
pub const ALTERNATE_TOP_RECT: CrystalRect = CrystalRect::new(9.0, 666.0, 85.0, 30.0);
pub const ALTERNATE_BOTTOM_RECT: CrystalRect = CrystalRect::new(9.0, 696.0, 85.0, 30.0);

pub struct Mir2CrystalHudPlugin;

impl Plugin for Mir2CrystalHudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiReadModel>()
            .init_resource::<InventoryModel>()
            .init_resource::<MapModel>()
            .init_resource::<NativeShellModel>()
            .add_systems(Startup, spawn_crystal_hud)
            .add_systems(
                Update,
                update_hud_visibility.run_if(resource_changed::<NativeShellModel>),
            )
            .add_systems(
                Update,
                update_hud_read_model.run_if(resource_changed::<UiReadModel>),
            )
            .add_systems(
                Update,
                update_hud_hp_alternate_text.run_if(resource_changed::<UiReadModel>),
            )
            .add_systems(Update, update_hud_option_presentation)
            .add_systems(
                Update,
                sync_belt_hit_targets.run_if(resource_changed::<InventoryModel>),
            )
            .add_systems(
                Update,
                update_hud_inventory.run_if(resource_changed::<InventoryModel>),
            )
            .add_systems(
                Update,
                update_hud_map_model.run_if(resource_changed::<MapModel>),
            )
            .add_systems(Update, update_hud_minimap_visibility)
            .add_plugins(super::overlays::Mir2CrystalOverlayPlugin);
    }
}

fn spawn_crystal_hud(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    shell: Res<NativeShellModel>,
    ui_model: Res<UiReadModel>,
    inventory: Res<InventoryModel>,
    map_model: Res<MapModel>,
) {
    let display = if shell.screen == NativeShellScreen::InGame {
        Display::Flex
    } else {
        Display::None
    };
    let hp_only = crystal_hp_only(&ui_model);

    commands
        .spawn((
            CrystalHudRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(1024.0),
                height: Val::Px(768.0),
                display,
                ..default()
            },
            GlobalZIndex(HUD_Z_INDEX),
        ))
        .with_children(|root| {
            spawn_frame(root, &asset_server, spec::MAIN);

            spawn_orb_half(
                root,
                &asset_server,
                OrbSide::Hp,
                ui_model.player.normalized_hp(),
                hp_only,
            );
            spawn_orb_half(
                root,
                &asset_server,
                OrbSide::Mp,
                ui_model.player.normalized_mp(),
                hp_only,
            );
            spawn_horizontal_bar(
                root,
                &asset_server,
                CrystalHudExperienceBar,
                "Prguse",
                spec::EXPERIENCE_BAR.index,
                spec::EXPERIENCE_BAR.rect,
                ui_model.player.normalized_experience(),
                3.0,
            );
            let (weight_library, weight_index) =
                weight_bar_asset(ui_model.player.normalized_weight());
            spawn_horizontal_bar(
                root,
                &asset_server,
                CrystalHudWeightBar,
                weight_library,
                weight_index,
                spec::WEIGHT_BAR.rect,
                ui_model.player.normalized_weight(),
                2.0,
            );

            spawn_text(
                root,
                CrystalHudHpText,
                &format!("HP {}", ui_model.player.hp_label().replacen(" / ", "/", 1)),
                HP_TEXT_RECT,
                CRYSTAL_DEFAULT_FONT_SIZE_PX,
                WHITE,
                Justify::Center,
            );
            spawn_text(
                root,
                CrystalHudMpText,
                &compact_mp_label(&ui_model),
                MP_TEXT_RECT,
                CRYSTAL_DEFAULT_FONT_SIZE_PX,
                WHITE,
                Justify::Center,
            );
            spawn_text(
                root,
                CrystalHudHpAlternateTopText,
                &hp_view_alternate_top(&ui_model),
                ALTERNATE_TOP_RECT,
                CRYSTAL_DEFAULT_FONT_SIZE_PX,
                WHITE,
                Justify::Center,
            );
            spawn_text(
                root,
                CrystalHudHpAlternateBottomText,
                &hp_view_alternate_bottom(&ui_model),
                ALTERNATE_BOTTOM_RECT,
                CRYSTAL_DEFAULT_FONT_SIZE_PX,
                WHITE,
                Justify::Center,
            );
            spawn_text(
                root,
                CrystalHudLevel,
                &ui_model.player.level.to_string(),
                LEVEL_RECT,
                CRYSTAL_DEFAULT_FONT_SIZE_PX,
                WHITE,
                Justify::Left,
            );
            spawn_vertical_centered_text(
                root,
                CrystalHudName,
                ui_model.player.name.as_deref().unwrap_or(""),
                NAME_RECT,
                CRYSTAL_DEFAULT_FONT_SIZE_PX,
                WHITE,
                Justify::Center,
            );
            spawn_vertical_centered_text(
                root,
                CrystalHudGold,
                &format_gold(ui_model.player.gold),
                GOLD_RECT,
                CRYSTAL_DEFAULT_FONT_SIZE_PX,
                WHITE,
                Justify::Left,
            );
            spawn_text(
                root,
                CrystalHudExperienceText,
                &ui_model.player.experience_percent_label(),
                EXPERIENCE_TEXT_RECT,
                CRYSTAL_DEFAULT_FONT_SIZE_PX,
                WHITE,
                Justify::Left,
            );
            spawn_text(
                root,
                CrystalHudWeightText,
                &ui_model.player.available_weight().to_string(),
                spec::WEIGHT_LABEL,
                CRYSTAL_DEFAULT_FONT_SIZE_PX,
                WHITE,
                Justify::Left,
            );
            spawn_text(
                root,
                CrystalHudSpaceText,
                &free_inventory_slots(&inventory).to_string(),
                spec::SPACE_LABEL,
                CRYSTAL_DEFAULT_FONT_SIZE_PX,
                WHITE,
                Justify::Left,
            );

            spawn_frame_at(root, &asset_server, "Prguse", 1932, BELT_FRAME);
            spawn_frame_tinted(
                root,
                &asset_server,
                "Prguse",
                1933,
                BELT_FRAME,
                Color::srgba(1.0, 1.0, 1.0, 0.5),
            );
            for slot in 0..BELT_SLOT_COUNT {
                spawn_belt_slot(root, &asset_server, &inventory, slot);
            }

            spawn_minimap_frame(root, &asset_server);
            spawn_vertical_centered_text(
                root,
                CrystalHudMapTitle,
                ui_model.player.map_name.as_deref().unwrap_or(""),
                MAP_TITLE_RECT,
                CRYSTAL_DEFAULT_FONT_SIZE_PX,
                WHITE,
                Justify::Center,
            );
            spawn_vertical_centered_text(
                root,
                CrystalHudMapCoordinate,
                &format!("{}, {}", map_model.center_x, map_model.center_y),
                MAP_COORDINATE_RECT,
                CRYSTAL_DEFAULT_FONT_SIZE_PX,
                WHITE,
                Justify::Center,
            );

            spawn_hud_buttons(root, &asset_server);
        });
}

fn spawn_frame(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    frame: CrystalFrameSpec,
) {
    spawn_frame_at(parent, asset_server, frame.library, frame.index, frame.rect);
}

fn spawn_minimap_frame(parent: &mut ChildSpawnerCommands, asset_server: &AssetServer) {
    let frame = spec::MINIMAP;
    parent.spawn((
        CrystalHudMinimap,
        absolute_node(frame.rect),
        ImageNode {
            image: asset_server.load(format!("original-ui/{}/{}.png", frame.library, frame.index)),
            image_mode: NodeImageMode::Stretch,
            ..default()
        },
    ));
}

fn spawn_frame_at(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    library: &str,
    index: u16,
    rect: CrystalRect,
) {
    parent.spawn((
        absolute_node(rect),
        ImageNode {
            image: asset_server.load(format!("original-ui/{library}/{index}.png")),
            image_mode: NodeImageMode::Stretch,
            ..default()
        },
    ));
}

fn spawn_frame_tinted(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    library: &str,
    index: u16,
    rect: CrystalRect,
    color: Color,
) {
    parent.spawn((
        absolute_node(rect),
        ImageNode {
            image: asset_server.load(format!("original-ui/{library}/{index}.png")),
            color,
            image_mode: NodeImageMode::Stretch,
            ..default()
        },
    ));
}

fn spawn_orb_half(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    side: OrbSide,
    ratio: f32,
    hp_only: bool,
) {
    let geometry = if side == OrbSide::Hp && hp_only {
        hp_only_orb_clip_geometry(ratio)
    } else {
        orb_clip_geometry(ratio, side)
    };
    match side {
        OrbSide::Hp => {
            parent.spawn((
                CrystalHudHpOrb,
                absolute_node(geometry.destination),
                ImageNode {
                    image: asset_server.load(if hp_only {
                        "original-ui/Prguse/6.png"
                    } else {
                        "original-ui/Prguse/4.png"
                    }),
                    rect: Some(to_bevy_rect(geometry.source)),
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                },
            ));
        }
        OrbSide::Mp => {
            let mut node = absolute_node(geometry.destination);
            node.display = if hp_only {
                Display::None
            } else {
                Display::Flex
            };
            parent.spawn((
                CrystalHudMpOrb,
                node,
                ImageNode {
                    image: asset_server.load("original-ui/Prguse/4.png"),
                    rect: Some(to_bevy_rect(geometry.source)),
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                },
            ));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_horizontal_bar<T: Component>(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    marker: T,
    library: &str,
    index: u16,
    frame: CrystalRect,
    ratio: f32,
    source_inset: f32,
) {
    let clipped = horizontal_bar_rect(frame, ratio, source_inset);
    let display = if clipped.width > 0.0 {
        Display::Flex
    } else {
        Display::None
    };
    let mut node = absolute_node(clipped);
    node.display = display;
    parent.spawn((
        marker,
        node,
        ImageNode {
            image: asset_server.load(format!("original-ui/{library}/{index}.png")),
            rect: Some(to_bevy_rect(HudSourceRect::new(
                0.0,
                0.0,
                clipped.width,
                frame.height,
            ))),
            image_mode: NodeImageMode::Stretch,
            ..default()
        },
    ));
}

pub fn horizontal_bar_rect(frame: CrystalRect, ratio: f32, source_inset: f32) -> CrystalRect {
    CrystalRect::new(
        frame.left,
        frame.top,
        ((frame.width - source_inset).max(0.0) * ratio.clamp(0.0, 1.0)).floor(),
        frame.height,
    )
}

fn weight_bar_asset(ratio: f32) -> (&'static str, u16) {
    if ratio <= 0.50 {
        ("Prguse", 76)
    } else if ratio <= 0.75 {
        ("UI_32bit", 473)
    } else {
        ("UI_32bit", 472)
    }
}

fn spawn_hud_buttons(parent: &mut ChildSpawnerCommands, asset_server: &AssetServer) {
    spawn_hud_button(
        parent,
        asset_server,
        spec::CHARACTER,
        CrystalHudAction::Character,
    );
    spawn_hud_button(
        parent,
        asset_server,
        spec::INVENTORY,
        CrystalHudAction::Inventory,
    );
    spawn_hud_button(parent, asset_server, spec::SKILL, CrystalHudAction::Skill);
    spawn_hud_button(parent, asset_server, spec::QUEST, CrystalHudAction::Quest);
    spawn_hud_button(parent, asset_server, spec::OPTION, CrystalHudAction::Option);
    spawn_hud_button(parent, asset_server, spec::MENU, CrystalHudAction::Menu);
    spawn_hud_button(
        parent,
        asset_server,
        spec::GAME_SHOP,
        CrystalHudAction::GameShop,
    );
    spawn_hud_button(parent, asset_server, spec::MAIL, CrystalHudAction::Mail);
    spawn_hud_button(
        parent,
        asset_server,
        spec::BIG_MAP,
        CrystalHudAction::BigMap,
    );
    spawn_hud_button(
        parent,
        asset_server,
        spec::MINIMAP_TOGGLE,
        CrystalHudAction::MinimapToggle,
    );
    spawn_frame(parent, asset_server, spec::LIGHT_SETTING);
}

fn spawn_hud_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    button: super::spec::CrystalButtonSpec,
    action: CrystalHudAction,
) {
    spawn_crystal_image_button(
        parent,
        asset_server,
        button,
        CrystalButtonAssetSet::from_spec(button),
        action,
        false,
        true,
    );
}

fn spawn_belt_slot(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    inventory: &InventoryModel,
    slot: u8,
) {
    let slot_rect = spec::belt_slot(slot as usize);
    let item = belt_slot_item(inventory, slot);
    // A stale or legacy stack without its server instance id may be displayed,
    // but cannot issue an ambiguous use command.
    let mut hit_target = parent.spawn((
        {
            let mut node =
                absolute_node(CrystalRect::new(slot_rect.left, slot_rect.top, 32.0, 32.0));
            node.align_items = AlignItems::Center;
            node.justify_content = JustifyContent::Center;
            node
        },
        CrystalHudBeltHitTarget { slot },
        CrystalHudAction::BeltUse(slot),
    ));
    hit_target.with_children(|button| {
        let path = item.and_then(|item| item_icon_path(item.icon));
        button.spawn((
            CrystalHudBeltIcon { slot },
            Node {
                display: if path.is_some() {
                    Display::Flex
                } else {
                    Display::None
                },
                ..default()
            },
            ImageNode {
                image: path.map(|path| asset_server.load(path)).unwrap_or_default(),
                image_mode: NodeImageMode::Auto,
                ..default()
            },
        ));
    });
    spawn_text(
        parent,
        CrystalHudBeltKey { slot },
        &(slot + 1).to_string(),
        belt_key_rect(slot),
        CRYSTAL_DEFAULT_FONT_SIZE_PX,
        WHITE,
        Justify::Left,
    );
    spawn_unoutlined_text(
        parent,
        CrystalHudBeltItem { slot },
        &belt_item_label(inventory, slot),
        belt_count_rect(slot),
        CRYSTAL_DEFAULT_FONT_SIZE_PX,
        Color::srgb_u8(255, 255, 0),
        Justify::Right,
    );
}

fn spawn_text<T: Component>(
    parent: &mut ChildSpawnerCommands,
    marker: T,
    value: &str,
    rect: CrystalRect,
    font_size: f32,
    color: Color,
    justify: Justify,
) {
    spawn_text_entity(
        parent,
        marker,
        value,
        text_absolute_node(rect),
        font_size,
        color,
        justify,
        true,
    );
}

fn spawn_vertical_centered_text<T: Component>(
    parent: &mut ChildSpawnerCommands,
    marker: T,
    value: &str,
    rect: CrystalRect,
    font_size: f32,
    color: Color,
    justify: Justify,
) {
    let mut container = text_absolute_node(rect);
    container.align_items = AlignItems::Center;
    parent.spawn(container).with_children(|text_root| {
        spawn_text_entity(
            text_root,
            marker,
            value,
            Node {
                width: Val::Percent(100.0),
                ..default()
            },
            font_size,
            color,
            justify,
            true,
        );
    });
}

fn spawn_unoutlined_text<T: Component>(
    parent: &mut ChildSpawnerCommands,
    marker: T,
    value: &str,
    rect: CrystalRect,
    font_size: f32,
    color: Color,
    justify: Justify,
) {
    spawn_text_entity(
        parent,
        marker,
        value,
        text_absolute_node(rect),
        font_size,
        color,
        justify,
        false,
    );
}

#[allow(clippy::too_many_arguments)]
fn spawn_text_entity<T: Component>(
    parent: &mut ChildSpawnerCommands,
    marker: T,
    value: &str,
    node: Node,
    font_size: f32,
    color: Color,
    justify: Justify,
    outlined: bool,
) {
    let mut entity = parent.spawn((
        marker,
        node,
        Text::new(value.to_owned()),
        crystal_text_font(font_size),
        TextColor(color),
        TextLayout::new(justify, LineBreak::NoWrap),
    ));
    if outlined {
        entity.insert(TextShadow {
            offset: Vec2::splat(1.0),
            color: Color::BLACK,
        });
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

fn text_absolute_node(rect: CrystalRect) -> Node {
    let mut node = absolute_node(rect);
    node.overflow = Overflow::clip();
    node
}

fn to_bevy_rect(rect: HudSourceRect) -> bevy::math::Rect {
    bevy::math::Rect {
        min: Vec2::new(rect.left, rect.top),
        max: Vec2::new(rect.left + rect.width, rect.top + rect.height),
    }
}

fn update_hud_visibility(
    shell: Res<NativeShellModel>,
    mut roots: Query<&mut Node, With<CrystalHudRoot>>,
) {
    let Ok(mut root) = roots.single_mut() else {
        return;
    };
    root.display = if shell.screen == NativeShellScreen::InGame {
        Display::Flex
    } else {
        Display::None
    };
}

fn update_hud_read_model(
    model: Res<UiReadModel>,
    asset_server: Res<AssetServer>,
    mut image_queries: ParamSet<(
        Query<(&mut Node, &mut ImageNode), With<CrystalHudHpOrb>>,
        Query<(&mut Node, &mut ImageNode), With<CrystalHudMpOrb>>,
        Query<(&mut Node, &mut ImageNode), With<CrystalHudExperienceBar>>,
        Query<(&mut Node, &mut ImageNode), With<CrystalHudWeightBar>>,
    )>,
    mut text_queries: ParamSet<(
        Query<&mut Text, With<CrystalHudHpText>>,
        Query<&mut Text, With<CrystalHudMpText>>,
        Query<&mut Text, With<CrystalHudName>>,
        Query<&mut Text, With<CrystalHudLevel>>,
        Query<&mut Text, With<CrystalHudGold>>,
        Query<&mut Text, With<CrystalHudMapTitle>>,
        Query<&mut Text, With<CrystalHudExperienceText>>,
        Query<&mut Text, With<CrystalHudWeightText>>,
    )>,
) {
    let hp_only = crystal_hp_only(&model);
    sync_orb_half(
        &mut image_queries.p0(),
        &asset_server,
        OrbSide::Hp,
        model.player.normalized_hp(),
        hp_only,
    );
    sync_orb_half(
        &mut image_queries.p1(),
        &asset_server,
        OrbSide::Mp,
        model.player.normalized_mp(),
        hp_only,
    );
    sync_horizontal_bar(
        &mut image_queries.p2(),
        spec::EXPERIENCE_BAR.rect,
        model.player.normalized_experience(),
        3.0,
    );
    sync_weight_bar(
        &mut image_queries.p3(),
        &asset_server,
        model.player.normalized_weight(),
    );

    let hp = format!("HP {}", model.player.hp_label().replacen(" / ", "/", 1));
    let mp = compact_mp_label(&model);
    set_text(&mut text_queries.p0(), hp);
    set_text(&mut text_queries.p1(), mp);
    set_text(
        &mut text_queries.p2(),
        model.player.name.as_deref().unwrap_or("").to_owned(),
    );
    set_text(&mut text_queries.p3(), model.player.level.to_string());
    set_text(&mut text_queries.p4(), format_gold(model.player.gold));
    set_text(
        &mut text_queries.p5(),
        model.player.map_name.as_deref().unwrap_or("").to_owned(),
    );
    set_text(
        &mut text_queries.p6(),
        model.player.experience_percent_label(),
    );
    set_text(
        &mut text_queries.p7(),
        model.player.available_weight().to_string(),
    );
}

/// Keep alternate HP/MP labels in a separate system because Bevy's `ParamSet`
/// supports eight queries. It shares the same change gate as the primary HUD
/// update and still reads only authoritative player values.
fn update_hud_hp_alternate_text(
    model: Res<UiReadModel>,
    mut texts: ParamSet<(
        Query<&mut Text, With<CrystalHudHpAlternateTopText>>,
        Query<&mut Text, With<CrystalHudHpAlternateBottomText>>,
    )>,
) {
    set_text(&mut texts.p0(), hp_view_alternate_top(&model));
    set_text(&mut texts.p1(), hp_view_alternate_bottom(&model));
}

/// `MainDialog` switches from compact HP/MP labels to two raw-value rows when
/// `Settings.HPView` is off. Keeping this pure makes the alternate rendering
/// independently testable from the Bevy node wiring.
pub fn hp_view_alternate_top(model: &UiReadModel) -> String {
    if crystal_hp_only(model) {
        format!("{}\n--", model.player.hp)
    } else {
        format!(
            " {}    {} \n---------------",
            model.player.hp, model.player.mp
        )
    }
}

pub fn hp_view_alternate_bottom(model: &UiReadModel) -> String {
    if crystal_hp_only(model) {
        model.player.max_hp.to_string()
    } else {
        format!(" {}    {} ", model.player.max_hp, model.player.max_mp)
    }
}

pub fn crystal_hp_only(model: &UiReadModel) -> bool {
    model.player.level < 26
        && model
            .player
            .class_name
            .as_deref()
            .is_some_and(|class_name| class_name.eq_ignore_ascii_case("Warrior"))
}

pub fn compact_mp_label(model: &UiReadModel) -> String {
    if crystal_hp_only(model) {
        String::new()
    } else {
        format!("MP {} ", model.player.mp_label().replacen(" / ", "/", 1))
    }
}

/// Apply the presentation choices this HUD owns. The state is read directly
/// from `NativePlayerUiState.core.options`; this system never copies options
/// into a local source of truth and never changes authoritative player state.
/// Re-enabling a mode exposes the latest render nodes again.
fn update_hud_option_presentation(
    shell: Res<NativeShellModel>,
    model: Res<UiReadModel>,
    state: Option<Res<crate::crystal_ui::overlays::NativePlayerUiState>>,
    mut nodes: ParamSet<(
        Query<&mut Node, With<CrystalHudHpText>>,
        Query<&mut Node, With<CrystalHudMpText>>,
        Query<&mut Node, With<CrystalHudHpAlternateTopText>>,
        Query<&mut Node, With<CrystalHudHpAlternateBottomText>>,
    )>,
) {
    let in_game = shell.screen == NativeShellScreen::InGame;
    let hp_view = state
        .as_deref()
        .map(|state| state.core.options.hp_view)
        .unwrap_or(true);
    let compact_hp_display = if in_game && hp_view {
        Display::Flex
    } else {
        Display::None
    };
    let compact_mp_display = if in_game && hp_view && !crystal_hp_only(&model) {
        Display::Flex
    } else {
        Display::None
    };
    let alternate_display = if in_game && !hp_view {
        Display::Flex
    } else {
        Display::None
    };
    for mut node in nodes.p0().iter_mut() {
        node.display = compact_hp_display;
    }
    for mut node in nodes.p1().iter_mut() {
        node.display = compact_mp_display;
    }
    for mut node in nodes.p2().iter_mut() {
        node.display = alternate_display;
    }
    for mut node in nodes.p3().iter_mut() {
        node.display = alternate_display;
    }
}

fn sync_orb_half<T>(
    query: &mut Query<(&mut Node, &mut ImageNode), With<T>>,
    asset_server: &AssetServer,
    side: OrbSide,
    ratio: f32,
    hp_only: bool,
) where
    T: Component,
{
    let geometry = if side == OrbSide::Hp && hp_only {
        hp_only_orb_clip_geometry(ratio)
    } else {
        orb_clip_geometry(ratio, side)
    };
    for (mut node, mut image) in query.iter_mut() {
        node.display = if side == OrbSide::Mp && hp_only {
            Display::None
        } else {
            Display::Flex
        };
        node.left = Val::Px(geometry.destination.left);
        node.top = Val::Px(geometry.destination.top);
        node.width = Val::Px(geometry.destination.width);
        node.height = Val::Px(geometry.destination.height);
        image.image = asset_server.load(if side == OrbSide::Hp && hp_only {
            "original-ui/Prguse/6.png"
        } else {
            "original-ui/Prguse/4.png"
        });
        image.rect = Some(to_bevy_rect(geometry.source));
    }
}

fn sync_horizontal_bar<T>(
    query: &mut Query<(&mut Node, &mut ImageNode), With<T>>,
    frame: CrystalRect,
    ratio: f32,
    source_inset: f32,
) where
    T: Component,
{
    let clipped = horizontal_bar_rect(frame, ratio, source_inset);
    for (mut node, mut image) in query.iter_mut() {
        node.display = if clipped.width > 0.0 {
            Display::Flex
        } else {
            Display::None
        };
        node.left = Val::Px(clipped.left);
        node.top = Val::Px(clipped.top);
        node.width = Val::Px(clipped.width);
        node.height = Val::Px(clipped.height);
        image.rect = Some(to_bevy_rect(HudSourceRect::new(
            0.0,
            0.0,
            clipped.width,
            clipped.height,
        )));
    }
}

fn sync_weight_bar<T>(
    query: &mut Query<(&mut Node, &mut ImageNode), With<T>>,
    asset_server: &AssetServer,
    ratio: f32,
) where
    T: Component,
{
    sync_horizontal_bar(query, spec::WEIGHT_BAR.rect, ratio, 2.0);
    let (library, index) = weight_bar_asset(ratio);
    for (_, mut image) in query.iter_mut() {
        image.image = asset_server.load(format!("original-ui/{library}/{index}.png"));
    }
}

fn update_hud_inventory(
    inventory: Res<InventoryModel>,
    asset_server: Res<AssetServer>,
    mut text_queries: ParamSet<(
        Query<(&CrystalHudBeltItem, &mut Text)>,
        Query<&mut Text, With<CrystalHudSpaceText>>,
    )>,
    mut icons: Query<(&CrystalHudBeltIcon, &mut ImageNode, &mut Node)>,
) {
    for (marker, mut text) in &mut text_queries.p0() {
        text.0 = belt_item_label(&inventory, marker.slot);
    }
    set_text(
        &mut text_queries.p1(),
        free_inventory_slots(&inventory).to_string(),
    );
    for (marker, mut image, mut node) in &mut icons {
        if let Some(path) =
            belt_slot_item(&inventory, marker.slot).and_then(|item| item_icon_path(item.icon))
        {
            image.image = asset_server.load(path);
            node.display = Display::Flex;
        } else {
            node.display = Display::None;
        }
    }
}

/// Add/remove the actual UI hit component from the live authoritative belt
/// model. Dropping `Button` also removes the stale interaction state.
fn sync_belt_hit_targets(
    mut commands: Commands,
    inventory: Res<InventoryModel>,
    targets: Query<(Entity, &CrystalHudBeltHitTarget, Option<&Button>)>,
) {
    for (entity, marker, button) in &targets {
        let enabled =
            belt_slot_item(&inventory, marker.slot).is_some_and(|item| item.unique_id.is_some());
        match (enabled, button.is_some()) {
            (true, false) => {
                commands.entity(entity).insert((Button, Interaction::None));
            }
            (false, true) => {
                commands.entity(entity).remove::<Button>();
                commands.entity(entity).remove::<Interaction>();
            }
            _ => {}
        }
    }
}

fn update_hud_map_model(
    map_model: Res<MapModel>,
    mut coordinates: Query<&mut Text, With<CrystalHudMapCoordinate>>,
) {
    set_text(
        &mut coordinates,
        format!("{}, {}", map_model.center_x, map_model.center_y),
    );
}

fn update_hud_minimap_visibility(
    shell: Res<NativeShellModel>,
    state: Option<Res<crate::crystal_ui::overlays::NativePlayerUiState>>,
    mut node_queries: ParamSet<(
        Query<&mut Node, With<CrystalHudMinimap>>,
        Query<&mut Node, With<CrystalHudMapTitle>>,
        Query<&mut Node, With<CrystalHudMapCoordinate>>,
    )>,
) {
    let in_game = shell.screen == NativeShellScreen::InGame;
    let visible = in_game
        && state
            .as_deref()
            .map(|s| s.minimap_visible())
            .unwrap_or(true);
    let display = if visible {
        Display::Flex
    } else {
        Display::None
    };
    for mut node in node_queries.p0().iter_mut() {
        node.display = display;
    }
    for mut node in node_queries.p1().iter_mut() {
        node.display = display;
    }
    for mut node in node_queries.p2().iter_mut() {
        node.display = display;
    }
}

fn set_text<T>(texts: &mut Query<&mut Text, With<T>>, value: String)
where
    T: Component,
{
    for mut text in texts.iter_mut() {
        text.0 = value.clone();
    }
}

fn format_gold(gold: u32) -> String {
    let digits = gold.to_string();
    let mut output = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            output.push(',');
        }
        output.push(digit);
    }
    output
}

/// Return the belt item at a fixed Crystal slot, ignoring bag/equipment items.
pub fn belt_slot_item<'a>(model: &'a InventoryModel, slot: u8) -> Option<&'a ItemModel> {
    model
        .items
        .iter()
        .find(|item| item.container == 1 && item.slot == slot as u32)
}

pub fn free_inventory_slots(model: &InventoryModel) -> u32 {
    let occupied = model
        .items
        .iter()
        .filter(|item| matches!(item.container, 0 | 1))
        .count() as u32;
    u32::from(model.effective_capacity()).saturating_sub(occupied)
}

/// Produce a bounded label suitable for one 40-pixel belt slot.
pub fn belt_item_label(model: &InventoryModel, slot: u8) -> String {
    let Some(item) = belt_slot_item(model, slot) else {
        return String::new();
    };
    if item.quantity > 1 {
        item.quantity.to_string()
    } else {
        String::new()
    }
}

/// Crystal `BeltDialog.Key` uses a fixed 26x14 label at `(8 + slot*35, 2)`
/// relative to the 230,618 belt frame.
pub const fn belt_key_rect(slot: u8) -> CrystalRect {
    CrystalRect::new(238.0 + BELT_SLOT_STEP * slot as f32, 620.0, 26.0, 14.0)
}

/// `MirItemCell.CountLabel` is auto-sized and bottom-right anchored. Arial 8pt
/// measures to a 14px row on the 96-DPI logical stage, so this right-justified
/// row reproduces its source anchor without the native-only `x` prefix.
pub const fn belt_count_rect(slot: u8) -> CrystalRect {
    let item = spec::belt_slot(slot as usize);
    CrystalRect::new(item.left, item.top + 18.0, item.width, 14.0)
}

pub fn bounded_belt_label(value: &str, max_chars: usize) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() <= max_chars {
        return value.to_owned();
    }
    if max_chars == 0 {
        return String::new();
    }
    if max_chars == 1 {
        return "…".to_owned();
    }
    chars[..max_chars - 1].iter().collect::<String>() + "…"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::ItemModel;

    #[test]
    fn inventory_button_uses_exact_crystal_three_state_assets_and_geometry() {
        let assets = CrystalButtonAssetSet::from_spec(spec::INVENTORY);
        assert_eq!(assets.normal, "original-ui/Prguse/1903.png");
        assert_eq!(assets.hover, "original-ui/Prguse/1904.png");
        assert_eq!(assets.pressed, "original-ui/Prguse/1905.png");
        assert_eq!(assets.disabled, None);
        assert_eq!(
            spec::INVENTORY.rect,
            CrystalRect::new(928.0, 692.0, 20.0, 20.0)
        );
    }

    #[test]
    fn character_button_uses_exact_crystal_three_state_assets_and_geometry() {
        let assets = CrystalButtonAssetSet::from_spec(spec::CHARACTER);
        assert_eq!(assets.normal, "original-ui/Prguse/1900.png");
        assert_eq!(assets.hover, "original-ui/Prguse/1901.png");
        assert_eq!(assets.pressed, "original-ui/Prguse/1902.png");
        assert_eq!(assets.disabled, None);
        assert_eq!(
            spec::CHARACTER.rect,
            CrystalRect::new(905.0, 692.0, 20.0, 20.0)
        );
    }

    #[test]
    fn main_hud_button_matrix_uses_exact_crystal_assets_and_geometry() {
        let cases = [
            (
                spec::SKILL,
                1906,
                CrystalRect::new(951.0, 692.0, 20.0, 20.0),
            ),
            (
                spec::QUEST,
                1909,
                CrystalRect::new(974.0, 692.0, 20.0, 20.0),
            ),
            (
                spec::OPTION,
                1912,
                CrystalRect::new(997.0, 692.0, 20.0, 20.0),
            ),
            (spec::MENU, 1960, CrystalRect::new(969.0, 651.0, 40.0, 40.0)),
            (
                spec::GAME_SHOP,
                826,
                CrystalRect::new(919.0, 651.0, 40.0, 38.0),
            ),
        ];

        for (button, first_index, expected_rect) in cases {
            let assets = CrystalButtonAssetSet::from_spec(button);
            assert_eq!(
                assets.normal,
                format!("original-ui/Prguse/{first_index}.png")
            );
            assert_eq!(
                assets.hover,
                format!("original-ui/Prguse/{}.png", first_index + 1)
            );
            assert_eq!(
                assets.pressed,
                format!("original-ui/Prguse/{}.png", first_index + 2)
            );
            assert_eq!(assets.disabled, None);
            assert_eq!(button.rect, expected_rect);
        }
    }

    #[test]
    fn minimap_visibility_system_initializes_with_overlapping_node_markers() {
        let mut app = App::new();
        app.insert_resource(NativeShellModel::default());
        app.add_systems(Update, update_hud_minimap_visibility);
        app.world_mut()
            .spawn((Node::default(), CrystalHudMinimap, CrystalHudMapTitle));
        app.world_mut()
            .spawn((Node::default(), CrystalHudMapCoordinate));

        app.update();
    }

    fn item(key: &str, name: &str, container: u8, slot: u32, quantity: u32) -> ItemModel {
        ItemModel {
            unique_id: key.parse().ok(),
            key: key.to_owned(),
            name: name.to_owned(),
            quantity,
            slot,
            container,
            ..ItemModel::default()
        }
    }

    #[test]
    fn belt_hit_target_tracks_late_population_and_clear() {
        let mut app = App::new();
        app.init_resource::<InventoryModel>()
            .add_systems(Update, sync_belt_hit_targets);
        let target = app
            .world_mut()
            .spawn((
                CrystalHudBeltHitTarget { slot: 0 },
                CrystalHudAction::BeltUse(0),
            ))
            .id();

        app.update();
        assert!(!app.world().entity(target).contains::<Button>());

        app.world_mut().resource_mut::<InventoryModel>().items = vec![ItemModel {
            unique_id: Some(77),
            key: "potion".to_owned(),
            name: "Potion".to_owned(),
            quantity: 2,
            slot: 0,
            container: 1,
            icon: 7,
            ..ItemModel::default()
        }];
        app.update();
        assert!(app.world().entity(target).contains::<Button>());

        app.world_mut()
            .resource_mut::<InventoryModel>()
            .items
            .clear();
        app.update();
        assert!(!app.world().entity(target).contains::<Button>());
    }

    #[test]
    fn orb_clip_is_bottom_anchored_and_ratio_clamped() {
        let hp = orb_clip_geometry(0.5, OrbSide::Hp);
        assert_eq!(hp.source, HudSourceRect::new(0.0, 40.0, 50.0, 40.0));
        assert_eq!(hp.destination, CrystalRect::new(0.0, 686.0, 50.0, 40.0));

        let mp = orb_clip_geometry(2.0, OrbSide::Mp);
        assert_eq!(mp.source, HudSourceRect::new(51.0, 0.0, 50.0, 80.0));
        assert_eq!(mp.destination, CrystalRect::new(51.0, 646.0, 50.0, 80.0));

        let empty = orb_clip_geometry(-1.0, OrbSide::Hp);
        assert_eq!(empty.source.height, 0.0);
        assert_eq!(empty.destination.top, 726.0);
    }

    #[test]
    fn orb_source_rects_match_crystal_prguse_four_halves() {
        assert_eq!(
            orb_source_rect(OrbSide::Hp, 80.0),
            HudSourceRect::new(0.0, 0.0, 50.0, 80.0)
        );
        assert_eq!(
            orb_source_rect(OrbSide::Mp, 80.0),
            HudSourceRect::new(51.0, 0.0, 50.0, 80.0)
        );
        assert_eq!(
            orb_source_rect(OrbSide::Hp, 25.0),
            HudSourceRect::new(0.0, 55.0, 50.0, 25.0)
        );
    }

    #[test]
    fn low_level_warrior_uses_crystal_full_width_hp_orb() {
        let geometry = hp_only_orb_clip_geometry(0.5);
        assert_eq!(geometry.source, HudSourceRect::new(0.0, 40.0, 100.0, 40.0));
        assert_eq!(
            geometry.destination,
            CrystalRect::new(0.0, 686.0, 100.0, 40.0)
        );
    }

    #[test]
    fn hp_view_alternate_labels_keep_authoritative_current_and_max_values() {
        let model = UiReadModel {
            player: crate::read_model::PlayerStats {
                hp: 12,
                max_hp: 15,
                mp: 4,
                max_mp: 11,
                ..default()
            },
        };
        assert_eq!(hp_view_alternate_top(&model), " 12    4 \n---------------");
        assert_eq!(hp_view_alternate_bottom(&model), " 15    11 ");
    }

    #[test]
    fn low_level_warrior_hides_mp_and_uses_hp_only_labels() {
        let mut model = UiReadModel {
            player: crate::read_model::PlayerStats {
                hp: 12,
                max_hp: 15,
                mp: 4,
                max_mp: 11,
                level: 25,
                class_name: Some("Warrior".to_owned()),
                ..default()
            },
        };
        assert!(crystal_hp_only(&model));
        assert_eq!(compact_mp_label(&model), "");
        assert_eq!(hp_view_alternate_top(&model), "12\n--");
        assert_eq!(hp_view_alternate_bottom(&model), "15");

        model.player.level = 26;
        assert!(!crystal_hp_only(&model));
        assert_eq!(compact_mp_label(&model), "MP 4/11 ");
    }

    #[test]
    fn hp_view_ecs_gate_swaps_presentation_without_mutating_options() {
        let mut shell = NativeShellModel::default();
        shell.screen = NativeShellScreen::InGame;
        let mut player_ui = crate::crystal_ui::overlays::NativePlayerUiState::default();
        player_ui.core.options.hp_view = false;
        let mut app = App::new();
        app.insert_resource(shell)
            .insert_resource(UiReadModel::default())
            .insert_resource(player_ui)
            .add_systems(Update, update_hud_option_presentation);
        let hp = app
            .world_mut()
            .spawn((Node::default(), CrystalHudHpText))
            .id();
        let mp = app
            .world_mut()
            .spawn((Node::default(), CrystalHudMpText))
            .id();
        let alternate_top = app
            .world_mut()
            .spawn((Node::default(), CrystalHudHpAlternateTopText))
            .id();
        let alternate_bottom = app
            .world_mut()
            .spawn((Node::default(), CrystalHudHpAlternateBottomText))
            .id();

        app.update();
        assert_eq!(
            app.world().entity(hp).get::<Node>().unwrap().display,
            Display::None
        );
        assert_eq!(
            app.world().entity(mp).get::<Node>().unwrap().display,
            Display::None
        );
        assert_eq!(
            app.world()
                .entity(alternate_top)
                .get::<Node>()
                .unwrap()
                .display,
            Display::Flex
        );
        assert_eq!(
            app.world()
                .entity(alternate_bottom)
                .get::<Node>()
                .unwrap()
                .display,
            Display::Flex
        );

        app.world_mut()
            .resource_mut::<crate::crystal_ui::overlays::NativePlayerUiState>()
            .core
            .options
            .hp_view = true;
        app.update();
        assert_eq!(
            app.world().entity(hp).get::<Node>().unwrap().display,
            Display::Flex
        );
        assert_eq!(
            app.world().entity(mp).get::<Node>().unwrap().display,
            Display::Flex
        );
        assert_eq!(
            app.world()
                .entity(alternate_top)
                .get::<Node>()
                .unwrap()
                .display,
            Display::None
        );
        assert_eq!(
            app.world()
                .entity(alternate_bottom)
                .get::<Node>()
                .unwrap()
                .display,
            Display::None
        );
    }

    #[test]
    fn belt_mapping_uses_container_one_and_exact_slot() {
        let model = InventoryModel {
            gold: 7,
            items: vec![
                item("bag", "BagItem", 0, 2, 1),
                item("slot2", "Second", 1, 2, 1),
                item("slot0", "First", 1, 0, 1),
                item("equipment", "Equip", 2, 0, 1),
            ],
            ..Default::default()
        };
        assert_eq!(
            belt_slot_item(&model, 0).map(|item| item.key.as_str()),
            Some("slot0")
        );
        assert_eq!(belt_slot_item(&model, 1), None);
        assert_eq!(belt_item_label(&model, 2), "");
        assert_eq!(belt_item_label(&model, 0), "");
    }

    #[test]
    fn main_hud_text_uses_crystal_point_size_and_source_anchors() {
        assert!((CRYSTAL_DEFAULT_FONT_SIZE_PX - 10.666_667).abs() < 0.000_01);
        assert_eq!(HP_TEXT_RECT, CrystalRect::new(0.0, 673.0, 100.0, 14.0));
        assert_eq!(MP_TEXT_RECT, CrystalRect::new(0.0, 688.0, 100.0, 14.0));
        assert_eq!(
            EXPERIENCE_TEXT_RECT,
            CrystalRect::new(491.0, 749.0, 40.0, 12.0)
        );
        assert_eq!(belt_key_rect(0), CrystalRect::new(238.0, 620.0, 26.0, 14.0));
        assert_eq!(belt_key_rect(5), CrystalRect::new(413.0, 620.0, 26.0, 14.0));
        assert_eq!(
            belt_count_rect(0),
            CrystalRect::new(242.0, 639.0, 32.0, 14.0)
        );
    }

    #[test]
    fn belt_count_matches_crystal_without_native_prefix_or_durability_text() {
        let mut stacked = item("stacked", "Potion", 1, 0, 12);
        stacked.durability_current = Some(7);
        stacked.durability_max = Some(10);
        let model = InventoryModel {
            items: vec![stacked],
            ..Default::default()
        };
        assert_eq!(belt_item_label(&model, 0), "12");

        let mut single = item("single", "Sword", 1, 1, 1);
        single.durability_current = Some(7);
        single.durability_max = Some(10);
        let model = InventoryModel {
            items: vec![single],
            ..Default::default()
        };
        assert_eq!(belt_item_label(&model, 1), "");
    }

    #[test]
    fn belt_labels_are_bounded_without_splitting_unicode() {
        assert_eq!(bounded_belt_label("Potion", 5), "Poti…");
        assert_eq!(bounded_belt_label("红蓝药水", 5), "红蓝药水");
        assert_eq!(bounded_belt_label("abcdef", 1), "…");
    }

    #[test]
    fn progress_bars_follow_crystal_source_width_insets() {
        assert_eq!(
            horizontal_bar_rect(spec::EXPERIENCE_BAR.rect, 0.5, 3.0),
            CrystalRect::new(9.0, 759.0, 500.0, 8.0)
        );
        assert_eq!(
            horizontal_bar_rect(spec::WEIGHT_BAR.rect, 0.5, 2.0),
            CrystalRect::new(919.0, 719.0, 37.0, 12.0)
        );
        assert_eq!(
            horizontal_bar_rect(spec::WEIGHT_BAR.rect, -1.0, 2.0).width,
            0.0
        );
        assert_eq!(
            horizontal_bar_rect(spec::WEIGHT_BAR.rect, 2.0, 2.0).width,
            74.0
        );
        assert_eq!(weight_bar_asset(0.50), ("Prguse", 76));
        assert_eq!(weight_bar_asset(0.75), ("UI_32bit", 473));
        assert_eq!(weight_bar_asset(0.76), ("UI_32bit", 472));
    }

    #[test]
    fn free_inventory_slots_follow_crystal_inventory_array_capacity() {
        let model = InventoryModel {
            gold: 0,
            items: vec![
                item("bag0", "Bag zero", 0, 0, 1),
                item("bag1", "Bag one", 0, 1, 1),
                item("belt", "Belt", 1, 0, 1),
                item("equip", "Equipment", 2, 0, 1),
            ],
            ..Default::default()
        };
        assert_eq!(free_inventory_slots(&model), 43);

        let expanded = InventoryModel {
            capacity: 54,
            ..model
        };
        assert_eq!(free_inventory_slots(&expanded), 51);
    }

    #[test]
    fn source_positions_match_crystal_main_dialog() {
        assert_eq!(spec::MAIN.rect, CrystalRect::new(0.0, 616.0, 1024.0, 152.0));
        assert_eq!(BELT_FRAME, CrystalRect::new(230.0, 618.0, 240.0, 38.0));
        assert_eq!(HP_TEXT_RECT.top, 673.0);
        assert_eq!(MP_TEXT_RECT.top, 688.0);
        assert_eq!(GOLD_RECT.left, 919.0);
        assert_eq!(
            MAP_COORDINATE_RECT,
            CrystalRect::new(944.0, 131.0, 56.0, 18.0)
        );
    }
}
