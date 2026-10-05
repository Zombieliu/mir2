//! Actual Crystal warehouse Bevy tree shared by Native and portable hosts.
//! Hosts own visibility, gestures, requests and receipts; this painter owns
//! the original art, controls, item hints and measured hit cells.

use bevy::prelude::*;
use bevy::text::{Justify, LineBreak, TextLayout};
use bevy::ui::FocusPolicy;

use super::assets::CrystalButtonAssetSet;
use super::bag_paint::paint_stack_count;
use super::item_image::{original_item_image_bundle, OriginalItemPaintScale};
use super::item_tooltip::crystal_item_tooltip_document;
use super::spec::{CrystalButtonSpec, CrystalRect};
use super::widget::{spawn_crystal_image_button, CrystalItemHint};
use crate::inventory::ItemModel;
use crate::read_model::PlayerStats;
use crate::storage::{StorageItemSelection, StorageModel};

pub const STORAGE_PANEL_WIDTH: f32 = 388.0;
pub const STORAGE_PANEL_HEIGHT: f32 = 346.0;
/// Static frame/control art. Concrete item icon paths come from live ItemModel data.
pub const STORAGE_REQUIRED_SKINS: &[&str] = &[
    "original-ui/Prguse/586.png",
    "original-ui/Title/0.png",
    "original-ui/Title/743.png",
    "original-ui/Title/744.png",
    "original-ui/Title/745.png",
    "original-ui/Title/746.png",
    "original-ui/Prguse2/360.png",
    "original-ui/Prguse2/361.png",
    "original-ui/Prguse2/362.png",
    "original-ui/Prguse/2443.png",
    "original-ui/Title/483.png",
    "original-ui/Title/484.png",
    "original-ui/Title/485.png",
    "original-ui/Title/113.png",
    "original-ui/Title/114.png",
    "original-ui/Title/115.png",
];

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoragePaintAction {
    Close,
    Page(usize),
    Cell { slot: u32, unique_id: Option<u64> },
    Password,
    Rent,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoragePaintCell {
    pub slot: u32,
    pub unique_id: Option<u64>,
    pub enabled: bool,
}

#[derive(Component, Debug)]
pub struct StoragePaintPanel;

#[derive(Component, Debug)]
pub struct StoragePaintGrid;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoragePaintBlocker {
    Password,
    Rental,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoragePaintSelection {
    pub slot: u32,
    pub unique_id: u64,
}

#[derive(Debug, Clone)]
pub struct StoragePaintLabels {
    pub rental_locked: String,
    pub expiry_prefix: String,
    pub password_locked: String,
}

impl Default for StoragePaintLabels {
    fn default() -> Self {
        Self {
            rental_locked: "Expanded Storage Locked".to_owned(),
            expiry_prefix: "Expanded Storage Expires On".to_owned(),
            password_locked: "Storage Locked".to_owned(),
        }
    }
}

#[derive(Clone)]
pub struct StoragePaintOptions {
    pub page: usize,
    pub selection: Option<StorageItemSelection>,
    /// Fixed pixel font; None preserves the original Native Arial 10px font.
    pub font: Option<TextFont>,
    pub labels: StoragePaintLabels,
}

impl Default for StoragePaintOptions {
    fn default() -> Self {
        Self {
            page: 0,
            selection: None,
            font: None,
            labels: StoragePaintLabels::default(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StoragePaintLayout {
    pub scale: f32,
}

impl Default for StoragePaintLayout {
    fn default() -> Self {
        Self { scale: 1.0 }
    }
}

impl StoragePaintLayout {
    fn rect(self, rect: CrystalRect) -> CrystalRect {
        CrystalRect::new(
            rect.left * self.scale,
            rect.top * self.scale,
            rect.width * self.scale,
            rect.height * self.scale,
        )
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

fn image_button<B: Bundle>(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    library: &'static str,
    indices: [u16; 3],
    rect: CrystalRect,
    action: StoragePaintAction,
    host: B,
) {
    let spec = CrystalButtonSpec::new(
        library,
        indices[0],
        indices[1],
        indices[2],
        rect,
        rect.width,
        rect.height,
    );
    spawn_crystal_image_button(
        parent,
        assets,
        spec,
        CrystalButtonAssetSet::from_spec(spec),
        (action, host),
        false,
        true,
    );
}

fn label(
    parent: &mut ChildSpawnerCommands,
    text: String,
    rect: CrystalRect,
    font: &TextFont,
    color: Color,
    centered: bool,
) {
    let mut node = absolute_node(rect);
    node.overflow = Overflow::clip();
    node.align_items = AlignItems::Center;
    node.justify_content = if centered {
        JustifyContent::Center
    } else {
        JustifyContent::FlexStart
    };
    parent.spawn((node, FocusPolicy::Pass)).with_children(|parent| {
        parent.spawn((
            Text::new(text),
            font.clone(),
            TextColor(color),
            TextLayout::new(
                if centered { Justify::Center } else { Justify::Left },
                LineBreak::NoWrap,
            ),
            FocusPolicy::Pass,
        ));
    });
}

#[allow(clippy::too_many_arguments)]
fn item_cell<B: Bundle>(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    item: &ItemModel,
    rect: CrystalRect,
    marker: StoragePaintCell,
    action: B,
    player: &PlayerStats,
    font: &TextFont,
    scale: f32,
) {
    let mut entity = parent.spawn((
        marker,
        absolute_node(rect),
        BackgroundColor(Color::NONE),
        Button,
        FocusPolicy::Block,
        CrystalItemHint(crystal_item_tooltip_document(item, player)),
    ));
    if marker.enabled {
        entity.insert(action);
    }
    entity.with_children(|cell| {
        if let Some(index) = item.user_item_image_index() {
            let (image_marker, node, mut image) = original_item_image_bundle(
                assets,
                Some(index),
                rect.width as i32,
                rect.height as i32,
            );
            image.color = if marker.enabled {
                Color::WHITE
            } else {
                Color::srgba(0.412, 0.412, 0.412, 0.8)
            };
            cell.spawn((
                image_marker, node, image, OriginalItemPaintScale(scale), FocusPolicy::Pass,
            ));
        }
        let count = item.crystal_stack_label();
        if !count.is_empty() {
            paint_stack_count(cell, &count, rect.width, rect.height, font);
        }
    });
}

/// Paint original Native geometry; hosts map controls into their existing intent queue.
pub fn paint_storage<B: Bundle>(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&AssetServer>,
    storage: &StorageModel,
    player: &PlayerStats,
    options: &StoragePaintOptions,
    map: impl FnMut(StoragePaintAction) -> B,
) {
    paint_storage_with_layout(
        parent, assets, storage, player, options, StoragePaintLayout::default(), map,
    );
}

/// Paint the same controls at a host-selected scale. Missing assets or an invalid
/// scale/pixel font do not produce a panel that could be mistaken for readiness.
pub fn paint_storage_with_layout<B: Bundle>(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&AssetServer>,
    storage: &StorageModel,
    player: &PlayerStats,
    options: &StoragePaintOptions,
    layout: StoragePaintLayout,
    mut map: impl FnMut(StoragePaintAction) -> B,
) {
    let Some(assets) = assets else {
        return;
    };
    if !layout.scale.is_finite() || layout.scale <= 0.0 {
        return;
    }
    let page = storage.page(options.page);
    let mut font = options.font.clone()
        .unwrap_or_else(|| super::typography::crystal_text_font(10.0));
    // Hosts supply a fixed pixel font so text and measured touch cells scale together.
    let FontSize::Px(font_size) = font.font_size else {
        return;
    };
    font.font_size = FontSize::Px(font_size * layout.scale);
    let panel = layout.rect(CrystalRect::new(0.0, 0.0, STORAGE_PANEL_WIDTH, STORAGE_PANEL_HEIGHT));
    parent.spawn((
        StoragePaintPanel,
        absolute_node(panel),
        FocusPolicy::Block,
        ImageNode { image: assets.load("original-ui/Prguse/586.png"), ..default() },
    )).with_children(|parent| {
        parent.spawn((
            absolute_node(layout.rect(CrystalRect::new(18.0, 8.0, 71.0, 15.0))),
            FocusPolicy::Pass,
            ImageNode { image: assets.load("original-ui/Title/0.png"), ..default() },
        ));
        image_button(parent, assets, "Title",
            if page.page == 0 { [743, 743, 744] } else { [744, 744, 744] },
            layout.rect(CrystalRect::new(8.0, 36.0, 72.0, 20.0)),
            StoragePaintAction::Page(0), map(StoragePaintAction::Page(0)));
        image_button(parent, assets, "Title",
            if page.page == 1 { [745, 745, 746] } else { [746, 746, 746] },
            layout.rect(CrystalRect::new(80.0, 36.0, 72.0, 20.0)),
            StoragePaintAction::Page(1), map(StoragePaintAction::Page(1)));
        image_button(parent, assets, "Prguse2", [360, 361, 362],
            layout.rect(CrystalRect::new(363.0, 3.0, 24.0, 21.0)),
            StoragePaintAction::Close, map(StoragePaintAction::Close));

        let mut grid_node = absolute_node(panel);
        if page.rental_locked {
            grid_node.display = Display::None;
        }
        parent.spawn((StoragePaintGrid, grid_node, FocusPolicy::Pass)).with_children(|grid| {
            for (offset, slot) in page.slots.iter().enumerate() {
                let rect = layout.rect(CrystalRect::new(
                    9.0 + (offset % 10) as f32 * 37.0,
                    60.0 + (offset / 10) as f32 * 33.0,
                    36.0, 32.0,
                ));
                let enabled = !page.locked && !page.rental_locked && !slot.locked
                    && (slot.item.is_none() || slot.unique_id.is_some());
                let marker = StoragePaintCell { slot: slot.slot, unique_id: slot.unique_id, enabled };
                let action = StoragePaintAction::Cell { slot: slot.slot, unique_id: slot.unique_id };
                if let Some(item) = slot.item {
                    if enabled {
                        item_cell(grid, assets, item, rect, marker, (action, map(action)), player, &font, layout.scale);
                    } else {
                        item_cell(grid, assets, item, rect, marker, (), player, &font, layout.scale);
                    }
                    if let Some(unique_id) = slot.unique_id.filter(|unique_id| {
                        options.selection == Some(StorageItemSelection { slot: slot.slot, unique_id: *unique_id })
                    }) {
                        grid.spawn((
                            StoragePaintSelection { slot: slot.slot, unique_id },
                            Node { overflow: Overflow::clip(), ..absolute_node(rect) },
                            Text::new("▶"), font.clone(),
                            TextColor(Color::srgb(0.94, 0.78, 0.28)),
                            TextLayout::new(Justify::Left, LineBreak::NoWrap), FocusPolicy::Pass,
                        ));
                    }
                } else {
                    let mut cell = grid.spawn((
                        marker, absolute_node(rect), FocusPolicy::Block,
                        BackgroundColor(if slot.locked {
                            Color::srgba(0.20, 0.16, 0.11, 0.70)
                        } else {
                            Color::srgba(0.06, 0.04, 0.02, 0.35)
                        }),
                    ));
                    if enabled {
                        cell.insert((Button, action, map(action)));
                    }
                }
            }
        });

        if page.rental_locked {
            parent.spawn((
                StoragePaintBlocker::Rental, Button, FocusPolicy::Block,
                absolute_node(layout.rect(CrystalRect::new(8.0, 59.0, 372.0, 265.0))),
                ImageNode { image: assets.load("original-ui/Prguse/2443.png"), ..default() },
                ZIndex(1),
            ));
            label(parent, options.labels.rental_locked.clone(),
                layout.rect(CrystalRect::new(40.0, 322.0, 300.0, 16.0)),
                &font, Color::srgb(0.95, 0.20, 0.20), true);
        } else if page.locked {
            parent.spawn((
                StoragePaintBlocker::Password, Button, FocusPolicy::Block,
                absolute_node(layout.rect(CrystalRect::new(8.0, 59.0, 372.0, 265.0))),
                BackgroundColor(Color::srgba(0.06, 0.04, 0.02, 0.90)), ZIndex(1),
            )).with_children(|cover| {
                label(cover, options.labels.password_locked.clone(),
                    layout.rect(CrystalRect::new(0.0, 120.0, 372.0, 24.0)),
                    &font, Color::srgb(0.95, 0.92, 0.82), true);
            });
        }
        if page.page == 1 {
            image_button(parent, assets, "Title", [483, 484, 485],
                layout.rect(CrystalRect::new(283.0, 33.0, 48.0, 25.0)),
                StoragePaintAction::Rent, map(StoragePaintAction::Rent));
            if !page.rental_locked {
                let text = storage_expiry_label(page.expiry)
                    .map(|expiry| format!("{}{expiry}", options.labels.expiry_prefix))
                    .unwrap_or_else(|| options.labels.expiry_prefix.clone());
                label(parent, text, layout.rect(CrystalRect::new(40.0, 322.0, 300.0, 16.0)),
                    &font, Color::srgb(0.95, 0.92, 0.82), true);
            }
        }
        image_button(parent, assets, "Title", [113, 114, 115],
            layout.rect(CrystalRect::new(328.0, 33.0, 48.0, 25.0)),
            StoragePaintAction::Password, map(StoragePaintAction::Password));
    });
}

/// Safely decodes the .NET DateTime binary value carried by ResizeStorage.
/// Invalid/zero values retain the source label without exposing raw ticks.
pub fn storage_expiry_label(binary_datetime: i64) -> Option<String> {
    const DOTNET_TICKS_MASK: u64 = 0x3fff_ffff_ffff_ffff;
    const DOTNET_KIND_MASK: u64 = 0xc000_0000_0000_0000;
    const DOTNET_KIND_LOCAL: u64 = 0x8000_0000_0000_0000;
    const DOTNET_UNIX_EPOCH_TICKS: i128 = 621_355_968_000_000_000;
    const DOTNET_MAX_TICKS: i128 = 3_155_378_975_999_999_999;
    const DOTNET_TICKS_PER_SECOND: i128 = 10_000_000;
    if binary_datetime == 0 {
        return None;
    }
    let bits = binary_datetime as u64;
    let ticks = i128::from(bits & DOTNET_TICKS_MASK);
    if ticks > DOTNET_MAX_TICKS {
        return None;
    }
    let unix_ticks = ticks - DOTNET_UNIX_EPOCH_TICKS;
    let seconds = unix_ticks.div_euclid(DOTNET_TICKS_PER_SECOND);
    let nanos = unix_ticks.rem_euclid(DOTNET_TICKS_PER_SECOND).saturating_mul(100);
    let (Ok(seconds), Ok(nanos)) = (i64::try_from(seconds), u32::try_from(nanos)) else {
        return None;
    };
    let utc = chrono::DateTime::<chrono::Utc>::from_timestamp(seconds, nanos)?;
    Some(if bits & DOTNET_KIND_MASK == DOTNET_KIND_LOCAL {
        utc.with_timezone(&chrono::Local)
            .format("%Y/%m/%d %H:%M:%S")
            .to_string()
    } else {
        utc.format("%Y/%m/%d %H:%M:%S").to_string()
    })
}

#[cfg(test)]
#[path = "storage_paint_tests.rs"]
mod tests;
