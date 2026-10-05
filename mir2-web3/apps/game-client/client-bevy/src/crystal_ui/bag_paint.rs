//! The Crystal inventory panel's Bevy nodes, shared by native and portable hosts.
//! Hosts supply intent bundles and native-only cell decisions; this module owns
//! the original frame, tabs, footer, grid, icons, stack labels, and hit cells.

use bevy::prelude::*;
use bevy::text::{Justify, LineBreak, TextLayout};

use super::assets::CrystalButtonAssetSet;
use super::item_image::{original_item_image_bundle, spawn_original_item_image_tinted, OriginalItemPaintScale};
use super::item_tooltip::{
    crystal_item_tooltip_document_with_options, CrystalItemTooltipOptions, CrystalStackSplitHint,
};
use super::panel_layouts::{
    INVENTORY_CELL_SIZE, INVENTORY_DELETE_BUTTON_ORIGIN, INVENTORY_DELETE_BUTTON_SIZE,
    INVENTORY_FREE_SLOT_LABEL_ORIGIN, INVENTORY_FREE_SLOT_LABEL_SIZE, INVENTORY_GOLD_LABEL_ORIGIN,
    INVENTORY_GOLD_LABEL_SIZE, INVENTORY_GRID_ORIGIN, INVENTORY_GRID_STEP, INVENTORY_PAGE_COLUMNS,
    INVENTORY_PAGE_SIZE, INVENTORY_PANEL_SIZE, INVENTORY_WEIGHT_BAR_ORIGIN,
    INVENTORY_WEIGHT_BAR_SIZE,
};
use super::spec::{CrystalButtonSpec, CrystalRect};
use super::widget::{spawn_crystal_image_button, CrystalItemHint};
use crate::inventory::{InventoryModel, ItemModel};
use crate::read_model::PlayerStats;

/// Static Crystal art used by this panel. Item icons are authoritative,
/// dynamic `original-ui/Items/{index}.png` paths and cannot be enumerated here.
pub const PORTABLE_BAG_REQUIRED_SKINS: &[&str] = &[
    "original-ui/Title/196.png",
    "original-ui/Title/197.png",
    "original-ui/Title/737.png",
    "original-ui/Title/169.png",
    "original-ui/Title/168.png",
    "original-ui/Title/738.png",
    "original-ui/Title/198.png",
    "original-ui/Title/739.png",
    "original-ui/Prguse2/360.png",
    "original-ui/Prguse2/361.png",
    "original-ui/Prguse2/362.png",
    "original-ui/Prguse2/366.png",
    "original-ui/Prguse2/367.png",
    "original-ui/Prguse2/368.png",
    "original-ui/Prguse/24.png",
    "original-ui/UI_32bit/471.png",
    "original-ui/UI_32bit/470.png",
];

/// A stable intent and identity on every enabled bag control. The host adds
/// its own action component; it must not add a second Button or Interaction.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BagPaintAction {
    SelectPage(u8),
    Close,
    ToggleDelete,
    InspectCell {
        container: u8,
        slot: u32,
        unique_id: Option<u64>,
    },
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BagPaintCell {
    pub container: u8,
    pub slot: u32,
    pub unique_id: Option<u64>,
}

#[derive(Component, Debug)]
pub struct BagPaintGridViewport;

/// Values not present in a host's authoritative read model remain absent.
/// Native passes its existing weight/free-slot calculations and Arial font.
#[derive(Clone)]
pub struct BagPaintOptions {
    pub page: u8,
    pub delete_mode: bool,
    pub weight_ratio: Option<f32>,
    pub free_slots: Option<u32>,
    pub font: TextFont,
    pub stack_split_hint: CrystalStackSplitHint,
}

/// Layout-only presentation profile. The original entry point always uses
/// desktop geometry; the portable host opts into touch-sized real Nodes.
#[derive(Debug, Clone, Copy)]
pub struct BagPaintLayout {
    pub scale: f32,
    pub touch: bool,
}

impl Default for BagPaintLayout {
    fn default() -> Self { Self { scale: 1.0, touch: false } }
}

impl BagPaintLayout {
    fn rect(self, rect: CrystalRect) -> CrystalRect {
        CrystalRect::new(rect.left * self.scale, rect.top * self.scale,
            rect.width * self.scale, rect.height * self.scale)
    }
}

#[derive(Clone, Copy)]
pub struct BagPaintCellInfo<'a> {
    pub container: u8,
    pub slot: u32,
    pub local_slot: usize,
    pub rect: CrystalRect,
    pub item: Option<&'a ItemModel>,
}

#[derive(Debug, Clone, Copy)]
pub struct BagCellPolicy {
    pub show_item: bool,
    pub enabled: bool,
    pub muted: bool,
}

impl BagCellPolicy {
    pub const fn new(show_item: bool, enabled: bool, muted: bool) -> Self {
        Self {
            show_item,
            enabled,
            muted,
        }
    }
}

pub fn bag_cell_rect(local_slot: usize) -> Option<CrystalRect> {
    (local_slot < INVENTORY_PAGE_SIZE).then(|| {
        CrystalRect::new(
            (local_slot % INVENTORY_PAGE_COLUMNS) as f32 * INVENTORY_GRID_STEP.x as f32,
            (local_slot / INVENTORY_PAGE_COLUMNS) as f32 * INVENTORY_GRID_STEP.y as f32,
            INVENTORY_CELL_SIZE.width as f32,
            INVENTORY_CELL_SIZE.height as f32,
        )
    })
}

pub fn inventory_second_tab_index(inventory: &InventoryModel, active: bool) -> u16 {
    if !inventory.second_bag_unlocked() {
        169
    } else if active {
        168
    } else {
        738
    }
}

pub fn format_crystal_gold(gold: u32) -> String {
    let digits = gold.to_string();
    let mut output = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            output.push(',');
        }
        output.push(digit);
    }
    output
}

pub fn inventory_weight_bar_asset(ratio: f32) -> (&'static str, u16) {
    if ratio <= 0.5 {
        ("Prguse", 24)
    } else if ratio <= 0.75 {
        ("UI_32bit", 471)
    } else {
        ("UI_32bit", 470)
    }
}

pub fn inventory_weight_bar_width(ratio: f32) -> f32 {
    ((INVENTORY_WEIGHT_BAR_SIZE.width as f32 - 3.0) * ratio.clamp(0.0, 1.0)).floor()
}

fn image_button<A: Bundle>(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    library: &'static str,
    normal: u16,
    hover: u16,
    pressed: u16,
    rect: CrystalRect,
    action: BagPaintAction,
    host_action: A,
) {
    let spec = CrystalButtonSpec::new(
        library,
        normal,
        hover,
        pressed,
        rect,
        rect.width,
        rect.height,
    );
    spawn_crystal_image_button(
        parent,
        asset_server,
        spec,
        CrystalButtonAssetSet::from_spec(spec),
        (action, host_action),
        false,
        true,
    );
}

fn text_at(parent: &mut ChildSpawnerCommands, text: String, rect: CrystalRect, font: &TextFont) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            overflow: Overflow::clip(),
            ..default()
        },
        Text::new(text),
        font.clone(),
        TextColor(Color::srgb(0.95, 0.92, 0.82)),
        TextLayout::new(Justify::Left, LineBreak::NoWrap),
    ));
}

fn paint_weight_bar(parent: &mut ChildSpawnerCommands, asset_server: &AssetServer, ratio: f32, layout: BagPaintLayout) {
    let width = inventory_weight_bar_width(ratio);
    if width <= 0.0 {
        return;
    }
    let (library, index) = inventory_weight_bar_asset(ratio);
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(INVENTORY_WEIGHT_BAR_ORIGIN.x as f32 * layout.scale),
            top: Val::Px(INVENTORY_WEIGHT_BAR_ORIGIN.y as f32 * layout.scale),
            width: Val::Px(width * layout.scale),
            height: Val::Px(INVENTORY_WEIGHT_BAR_SIZE.height as f32 * layout.scale),
            overflow: Overflow::clip(),
            ..default()
        },
        ImageNode {
            image: asset_server.load(format!("original-ui/{library}/{index}.png")),
            rect: Some(bevy::math::Rect {
                min: Vec2::ZERO,
                max: Vec2::new(width, INVENTORY_WEIGHT_BAR_SIZE.height as f32),
            }),
            ..default()
        },
    ));
}

/// Paint one original Crystal item cell. Other native item surfaces reuse this
/// same icon/tooltip/count implementation with `marker=()`.
#[allow(clippy::too_many_arguments)]
pub fn paint_original_item_cell<A: Bundle, M: Bundle>(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    item: &ItemModel,
    rect: CrystalRect,
    action: A,
    marker: M,
    enabled: bool,
    muted: bool,
    player: &PlayerStats,
    count_font: &TextFont,
) {
    paint_original_item_cell_with_hint(
        parent, asset_server, item, rect, action, marker, enabled, muted, player,
        count_font, CrystalStackSplitHint::default(), 1.0,
    );
}

#[allow(clippy::too_many_arguments)]
fn paint_original_item_cell_with_hint<A: Bundle, M: Bundle>(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    item: &ItemModel,
    rect: CrystalRect,
    action: A,
    marker: M,
    enabled: bool,
    muted: bool,
    player: &PlayerStats,
    count_font: &TextFont,
    stack_split_hint: CrystalStackSplitHint,
    paint_scale: f32,
) {
    let mut entity = parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            ..default()
        },
        BackgroundColor(Color::NONE),
        Button,
        CrystalItemHint(crystal_item_tooltip_document_with_options(
            item,
            player,
            CrystalItemTooltipOptions {
                stack_split_hint,
                ..Default::default()
            },
        )),
        marker,
    ));
    if enabled {
        entity.insert(action);
    }
    entity.with_children(|cell| {
        if let Some(index) = item.user_item_image_index() {
            if muted {
                if paint_scale == 1.0 {
                    spawn_original_item_image_tinted(
                        cell, asset_server, index, rect.width as i32, rect.height as i32,
                        Color::srgba(105.0 / 255.0, 105.0 / 255.0, 105.0 / 255.0, 0.8),
                    );
                } else {
                    let (marker, node, mut image) = original_item_image_bundle(
                        asset_server, Some(index), rect.width as i32, rect.height as i32);
                    image.color = Color::srgba(105.0 / 255.0, 105.0 / 255.0, 105.0 / 255.0, 0.8);
                    cell.spawn((marker, node, image, OriginalItemPaintScale(paint_scale)));
                }
            } else {
                let (marker, node, mut image) = original_item_image_bundle(
                    asset_server,
                    Some(index),
                    rect.width as i32,
                    rect.height as i32,
                );
                image.color = if enabled {
                    Color::WHITE
                } else {
                    Color::srgba(0.412, 0.412, 0.412, 0.8)
                };
                if paint_scale == 1.0 {
                    cell.spawn((marker, node, image));
                } else {
                    cell.spawn((marker, node, image, OriginalItemPaintScale(paint_scale)));
                }
            }
        }
        let count = item.crystal_stack_label();
        if !count.is_empty() {
            paint_stack_count(cell, &count, rect.width, rect.height, count_font);
        }
    });
}

pub fn paint_empty_cell<A: Bundle, M: Bundle>(
    parent: &mut ChildSpawnerCommands,
    rect: CrystalRect,
    action: A,
    marker: M,
    enabled: bool,
) {
    let mut entity = parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            ..default()
        },
        BackgroundColor(Color::NONE),
        marker,
    ));
    if enabled {
        entity.insert((Button, action));
    }
}

pub fn paint_stack_count(
    parent: &mut ChildSpawnerCommands,
    text: &str,
    cell_width: f32,
    cell_height: f32,
    font: &TextFont,
) {
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(cell_width),
                height: Val::Px(cell_height),
                align_items: AlignItems::FlexEnd,
                justify_content: JustifyContent::FlexEnd,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .with_children(|label| {
            label.spawn((
                Text::new(text.to_owned()),
                font.clone(),
                TextColor(Color::srgb(1.0, 1.0, 0.0)),
                TextLayout::new(Justify::Right, LineBreak::NoWrap),
            ));
        });
}

/// Native and portable hosts call this one node builder. `map_action` adds
/// only the host's action bundle; the painter owns Button/Interaction exactly
/// once. `cell_policy` can hide trade-offered items, mute mail-locked icons,
/// and apply native move/merge eligibility without entering the shared paint.
pub fn paint_crystal_inventory<A: Bundle>(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    inventory: &InventoryModel,
    player: &PlayerStats,
    options: &BagPaintOptions,
    map_action: impl FnMut(BagPaintAction) -> A,
    cell_policy: impl FnMut(BagPaintCellInfo<'_>) -> BagCellPolicy,
    decorate: impl FnMut(&mut ChildSpawnerCommands, BagPaintCellInfo<'_>),
) {
    paint_crystal_inventory_with_layout(parent, asset_server, inventory, player, options,
        BagPaintLayout::default(), map_action, cell_policy, decorate);
}

pub fn paint_crystal_inventory_with_layout<A: Bundle>(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    inventory: &InventoryModel,
    player: &PlayerStats,
    options: &BagPaintOptions,
    layout: BagPaintLayout,
    mut map_action: impl FnMut(BagPaintAction) -> A,
    mut cell_policy: impl FnMut(BagPaintCellInfo<'_>) -> BagCellPolicy,
    mut decorate: impl FnMut(&mut ChildSpawnerCommands, BagPaintCellInfo<'_>),
) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Px(INVENTORY_PANEL_SIZE.width as f32 * layout.scale),
            height: Val::Px(INVENTORY_PANEL_SIZE.height as f32 * layout.scale),
            ..default()
        },
        ImageNode {
            image: asset_server.load("original-ui/Title/196.png"),
            ..default()
        },
    ));
    for (page, left, active_index, idle_index) in [
        (0, 6.0, 197, 737),
        (
            1,
            76.0,
            inventory_second_tab_index(inventory, true),
            inventory_second_tab_index(inventory, false),
        ),
        (2, 146.0, 198, 739),
    ] {
        let action = BagPaintAction::SelectPage(page);
        let index = if options.page == page {
            active_index
        } else {
            idle_index
        };
        let tab = if layout.touch {
            CrystalRect::new(6.0 + page as f32 * 73.0, 2.0, 72.0, 32.0)
        } else {
            CrystalRect::new(left, 7.0, 72.0, 23.0)
        };
        image_button(
            parent,
            asset_server,
            "Title",
            index,
            index,
            index,
            layout.rect(tab),
            action,
            map_action(action),
        );
    }
    let close = BagPaintAction::Close;
    image_button(
        parent,
        asset_server,
        "Prguse2",
        360,
        361,
        362,
        layout.rect(if layout.touch { CrystalRect::new(278.0, 2.0, 32.0, 32.0) }
            else { CrystalRect::new(289.0, 3.0, 24.0, 21.0) }),
        close,
        map_action(close),
    );
    text_at(
        parent,
        format_crystal_gold(inventory.gold),
        layout.rect(CrystalRect::new(
            INVENTORY_GOLD_LABEL_ORIGIN.x as f32,
            INVENTORY_GOLD_LABEL_ORIGIN.y as f32,
            INVENTORY_GOLD_LABEL_SIZE.width as f32,
            INVENTORY_GOLD_LABEL_SIZE.height as f32,
        )),
        &options.font,
    );
    if let Some(ratio) = options.weight_ratio {
        paint_weight_bar(parent, asset_server, ratio, layout);
    }
    if let Some(free_slots) = options.free_slots {
        text_at(
            parent,
            free_slots.to_string(),
            layout.rect(CrystalRect::new(
                INVENTORY_FREE_SLOT_LABEL_ORIGIN.x as f32,
                INVENTORY_FREE_SLOT_LABEL_ORIGIN.y as f32,
                INVENTORY_FREE_SLOT_LABEL_SIZE.width as f32,
                INVENTORY_FREE_SLOT_LABEL_SIZE.height as f32,
            )),
            &options.font,
        );
    }
    let delete = BagPaintAction::ToggleDelete;
    if !layout.touch { image_button(
        parent,
        asset_server,
        "Prguse2",
        if options.delete_mode { 368 } else { 366 },
        if options.delete_mode { 368 } else { 367 },
        368,
        CrystalRect::new(
            INVENTORY_DELETE_BUTTON_ORIGIN.x as f32,
            INVENTORY_DELETE_BUTTON_ORIGIN.y as f32,
            INVENTORY_DELETE_BUTTON_SIZE.width as f32,
            INVENTORY_DELETE_BUTTON_SIZE.height as f32,
        ),
        delete,
        map_action(delete),
    ); }

    let (container, page_offset) = if options.page == 2 {
        (3, 0)
    } else {
        (0, usize::from(options.page) * INVENTORY_PAGE_SIZE)
    };
    parent
        .spawn((
            BagPaintGridViewport,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(INVENTORY_GRID_ORIGIN.x as f32 * layout.scale),
                top: Val::Px(INVENTORY_GRID_ORIGIN.y as f32 * layout.scale),
                width: Val::Px(
                    (INVENTORY_GRID_STEP.x as usize * (INVENTORY_PAGE_COLUMNS - 1)
                        + INVENTORY_CELL_SIZE.width as usize) as f32 * layout.scale,
                ),
                height: Val::Px(
                    (INVENTORY_GRID_STEP.y as usize
                        * (INVENTORY_PAGE_SIZE / INVENTORY_PAGE_COLUMNS - 1)
                        + INVENTORY_CELL_SIZE.height as usize) as f32 * layout.scale,
                ),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .with_children(|grid| {
            let page_items = inventory.items_in(container);
            for local_slot in 0..INVENTORY_PAGE_SIZE {
                let slot = (page_offset + local_slot) as u32;
                if container == 0 && slot >= u32::from(inventory.bag_slot_capacity()) {
                    continue;
                }
                let raw_item = page_items.iter().copied().find(|item| item.slot == slot);
                let rect = layout.rect(bag_cell_rect(local_slot).expect("bounded inventory page cell"));
                let raw = BagPaintCellInfo {
                    container,
                    slot,
                    local_slot,
                    rect,
                    item: raw_item,
                };
                let policy = cell_policy(raw);
                let item = if policy.show_item { raw_item } else { None };
                let info = BagPaintCellInfo { item, ..raw };
                let unique_id = item.and_then(|item| item.unique_id);
                let action = BagPaintAction::InspectCell {
                    container,
                    slot,
                    unique_id,
                };
                let marker = BagPaintCell {
                    container,
                    slot,
                    unique_id,
                };
                if let Some(item) = item {
                    paint_original_item_cell_with_hint(
                        grid,
                        asset_server,
                        item,
                        rect,
                        (action, map_action(action)),
                        marker,
                        policy.enabled,
                        policy.muted,
                        player,
                        &options.font,
                        options.stack_split_hint,
                        layout.scale,
                    );
                } else {
                    paint_empty_cell(
                        grid,
                        rect,
                        (action, map_action(action)),
                        marker,
                        policy.enabled,
                    );
                }
                decorate(grid, info);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource)]
    struct FixtureLayout(BagPaintLayout);

    fn paint_fixture(mut commands: Commands, assets: Res<AssetServer>, layout: Res<FixtureLayout>) {
        let inventory = InventoryModel {
            items: vec![ItemModel { unique_id: Some(7), icon: 1, quantity: 7,
                slot: 0, container: 0, name: "Potion".into(), ..default() }],
            ..default()
        };
        let options = BagPaintOptions { page: 0, delete_mode: false, weight_ratio: None,
            free_slots: Some(39), font: TextFont::default(),
            stack_split_hint: CrystalStackSplitHint::MoreActions };
        commands.spawn(Node::default()).with_children(|parent| {
            paint_crystal_inventory_with_layout(parent, &assets, &inventory,
                &PlayerStats::default(), &options, layout.0,
                |_| (), |_| BagCellPolicy::new(true, true, false), |_, _| {});
        });
    }

    #[test]
    fn touch_painter_uses_real_hit_nodes_and_bag_only_scaled_icon() {
        for (css_scale, expected_touch) in [(1.0, false), (640.0 / 1368.0, true),
            (844.0 / 1664.0, true)] {
            let layout = if expected_touch {
                BagPaintLayout { scale: 1.28 / css_scale, touch: true }
            } else { BagPaintLayout::default() };
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
                .init_asset::<Image>()
                .insert_resource(FixtureLayout(layout))
                .add_systems(Update, paint_fixture);
            app.update();
            let world = app.world_mut();
            let mut actions = world.query::<(&BagPaintAction, &Node)>();
            let mut cells = 0;
            let mut tabs = 0;
            let mut close = 0;
            let mut delete = 0;
            for (action, node) in actions.iter(world) {
                match action {
                    BagPaintAction::InspectCell { .. } => { cells += 1; }
                    BagPaintAction::SelectPage(_) => { tabs += 1; }
                    BagPaintAction::Close => { close += 1; }
                    BagPaintAction::ToggleDelete => { delete += 1; }
                }
                if expected_touch {
                    let (Val::Px(width), Val::Px(height)) = (node.width, node.height) else { panic!("real hit Node required"); };
                    assert!(width * css_scale >= 39.99 && height * css_scale >= 39.99,
                        "{action:?}: {}x{} CSS", width * css_scale, height * css_scale);
                }
            }
            assert_eq!((cells, tabs, close, delete), (40, 3, 1, usize::from(!expected_touch)));
            let mut scaled = world.query::<&OriginalItemPaintScale>();
            assert_eq!(scaled.iter(world).count(), usize::from(expected_touch));
            if !expected_touch {
                let mut first = world.query::<(&BagPaintCell, &Node)>();
                let node = first.iter(world).find(|(cell, _)| cell.slot == 0).unwrap().1;
                assert_eq!((node.width, node.height), (Val::Px(36.0), Val::Px(32.0)));
            }
        }
    }

    #[test]
    fn original_bag_geometry_and_page_visibility_are_stable() {
        assert_eq!(
            bag_cell_rect(0),
            Some(CrystalRect::new(0.0, 0.0, 36.0, 32.0))
        );
        assert_eq!(
            bag_cell_rect(39),
            Some(CrystalRect::new(259.0, 132.0, 36.0, 32.0))
        );
        assert_eq!(bag_cell_rect(40), None);
        let locked = InventoryModel::default();
        assert_eq!(inventory_second_tab_index(&locked, false), 169);
        let expanded = InventoryModel {
            capacity: 54,
            ..Default::default()
        };
        assert_eq!(inventory_second_tab_index(&expanded, true), 168);
    }
}
