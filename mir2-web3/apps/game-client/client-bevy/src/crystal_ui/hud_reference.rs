//! ReferenceLayoutV1 presentation with optional code-owned offline artwork.
//! Item pixels remain the verified R16 images; this module performs no I/O.
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use super::hud::{
    belt_item_label, belt_slot_item, free_inventory_slots, CrystalBeltControlAction,
    CrystalBeltPresentation, CrystalHudAction, CrystalHudBeltFrame, CrystalHudBeltHitTarget,
    CrystalHudBeltIcon, CrystalHudBeltItem, CrystalHudBeltKey, CrystalHudSpaceText,
};
use super::hud_layout::{HudLayoutSpec, ReferenceHudArtwork, ResolvedHudLayout};
use super::item_image::original_item_image_bundle;
use super::shared_hud::{absolute_node, hud_font, hud_hint};
use super::spec::CrystalRect;
use super::typography::CRYSTAL_DEFAULT_FONT_SIZE_PX;
use super::widget::CrystalHint;
use crate::inventory::{InventoryModel, ItemModel};
use crate::read_model::UiReadModel;

const BLACK: Color = Color::srgb(0.028, 0.033, 0.043);
const GOLD: Color = Color::srgb(0.66, 0.50, 0.24);
const CYAN: Color = Color::srgb(0.18, 0.76, 0.85);
const RED: Color = Color::srgb(0.64, 0.07, 0.10);
const BLUE: Color = Color::srgb(0.06, 0.24, 0.64);

#[derive(Component)]
pub(crate) struct ReferenceButton;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FillKind {
    Hp,
    Mp,
    Experience,
    Weight,
    OrbStrip,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct ReferenceFill {
    pub(crate) kind: FillKind,
    pub(crate) full: CrystalRect,
    pub(crate) strip_y: f32,
    pub(crate) diameter: f32,
}

#[derive(Component)]
pub(crate) struct MpBackdrop;

#[derive(Component, Clone, Copy)]
pub(crate) enum BeltPart {
    Frame,
    Slot(u8),
    Key(u8),
    Count(u8),
    Rotate,
    Close,
}

pub(crate) fn panel(
    parent: &mut ChildSpawnerCommands,
    rect: CrystalRect,
    art: Option<&ReferenceHudArtwork>,
) {
    let mut node = absolute_node(rect);
    if let Some(art) = art {
        parent.spawn((
            node,
            super::hud_art::frame_image(art, if rect.height >= 64.0 { 0.12 } else { 0.075 }, 1.0),
            FocusPolicy::Pass,
        ));
    } else {
        node.border = UiRect::all(Val::Px(1.));
        parent.spawn((
            node,
            BackgroundColor(BLACK),
            BorderColor::all(GOLD),
            FocusPolicy::Pass,
        ));
    }
}

fn text(parent: &mut ChildSpawnerCommands, value: impl Into<String>, rect: CrystalRect) {
    let mut node = absolute_node(rect);
    node.overflow = Overflow::clip();
    parent.spawn((
        node,
        Text::new(value),
        hud_font(None, CRYSTAL_DEFAULT_FONT_SIZE_PX),
        TextColor(Color::WHITE),
        FocusPolicy::Pass,
    ));
}

fn button<B: Bundle>(
    parent: &mut ChildSpawnerCommands,
    rect: CrystalRect,
    label: &str,
    hint: &str,
    marker: B,
    art: Option<&ReferenceHudArtwork>,
    icon: Option<CrystalHudAction>,
) {
    let mut node = absolute_node(rect);
    node.border = UiRect::all(Val::Px(1.));
    node.align_items = AlignItems::Center;
    node.justify_content = JustifyContent::Center;
    node.overflow = Overflow::clip();
    let mut entity = parent.spawn((
        node,
        Button,
        Interaction::None,
        ReferenceButton,
        BackgroundColor(BLACK),
        BorderColor::all(GOLD),
        CrystalHint::new(hint),
    ));
    entity.insert(marker);
    if let Some(art) = art {
        entity.insert(super::hud_art::frame_image(art, 0.06, 1.0));
    }
    entity.with_children(|p| {
        let value = crate::native_i18n::tr(label);
        if let Some(action) = icon.filter(|_| art.is_some()) {
            super::hud_art::action_icon(p, action, rect.width);
            p.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(2.0),
                    top: Val::Px(rect.height - 14.0),
                    width: Val::Px(rect.width - 4.0),
                    height: Val::Px(13.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
                Text::new(&value),
                hud_font(None, if value.chars().count() > 7 { 8.0 } else { 10.0 }),
                TextLayout::new(Justify::Center, bevy::text::LineBreak::NoWrap),
                TextColor(Color::srgb(0.92, 0.9, 0.82)),
                FocusPolicy::Pass,
            ));
        } else {
            p.spawn((
                Text::new(value),
                hud_font(None, CRYSTAL_DEFAULT_FONT_SIZE_PX),
                TextColor(Color::WHITE),
                FocusPolicy::Pass,
            ));
        }
    });
}

pub(crate) fn action_button(
    parent: &mut ChildSpawnerCommands,
    action: CrystalHudAction,
    rect: CrystalRect,
    art: Option<&ReferenceHudArtwork>,
) {
    let label = match action {
        CrystalHudAction::Character => "Character",
        CrystalHudAction::Inventory => "Bag",
        CrystalHudAction::Skill => "Skill",
        CrystalHudAction::Quest => "Quest",
        CrystalHudAction::Option => "Option",
        CrystalHudAction::Menu => "Menu",
        CrystalHudAction::GameShop => "Shop",
        _ => return,
    };
    button(
        parent,
        rect,
        label,
        hud_hint(action).unwrap_or(label),
        action,
        art,
        (action != CrystalHudAction::GameShop).then_some(action),
    );
}

fn fill(parent: &mut ChildSpawnerCommands, kind: FillKind, full: CrystalRect, color: Color) {
    parent.spawn((
        absolute_node(full),
        ReferenceFill {
            kind,
            full,
            strip_y: 0.,
            diameter: 0.,
        },
        BackgroundColor(color),
        FocusPolicy::Pass,
    ));
}

pub(crate) fn spawn_status(
    parent: &mut ChildSpawnerCommands,
    _model: &UiReadModel,
    layout: ResolvedHudLayout,
    art: Option<&ReferenceHudArtwork>,
) {
    panel(parent, layout.life_info, art);
    let device = layout.health_orb;
    let orb = if art.is_some() {
        let diameter = device.width * 0.65;
        let inset = (device.width - diameter) * 0.5;
        CrystalRect::new(device.left + inset, device.top + inset, diameter, diameter)
    } else {
        device
    };
    // The live fill is still clipped by exact circle chords. The optional
    // bezel surrounds its aperture; it never substitutes a static HP value.
    let radius = orb.width * 0.5;
    for row in 0..orb.height as u32 {
        let dy = row as f32 + 0.5 - radius;
        let half = (radius * radius - dy * dy).max(0.).sqrt();
        let outer = CrystalRect::new(
            orb.left + radius - half,
            orb.top + row as f32,
            half * 2.,
            1.,
        );
        parent.spawn((
            absolute_node(outer),
            BackgroundColor(GOLD),
            FocusPolicy::Pass,
        ));
        let inner_radius = (radius - 2.).max(0.);
        if dy.abs() < inner_radius {
            let half = (inner_radius * inner_radius - dy * dy).sqrt();
            let inner = CrystalRect::new(
                orb.left + radius - half,
                orb.top + row as f32,
                half * 2.,
                1.,
            );
            parent.spawn((
                absolute_node(inner),
                BackgroundColor(BLACK),
                FocusPolicy::Pass,
            ));
            // Shade the live HP geometry; all segments retain the same row
            // threshold and authoritative HP ratio, including zero and full.
            let segments = if art.is_some() { 5 } else { 1 };
            for segment in 0..segments {
                let width = inner.width / segments as f32;
                let part = CrystalRect::new(
                    inner.left + segment as f32 * width,
                    inner.top,
                    width,
                    inner.height,
                );
                let color = if art.is_some() {
                    let x = (part.left + width * 0.5 - (orb.left + radius)) / radius;
                    let y = dy / radius;
                    let light = 1.0 - row as f32 / orb.height;
                    let shade = 0.72 + 0.28 * (1.0 - x * x).max(0.0).sqrt();
                    let gloss =
                        0.5 * (-((x + 0.32).powi(2) / 0.055 + (y + 0.42).powi(2) / 0.10)).exp();
                    Color::srgb(
                        ((0.38 + 0.4 * light) * shade + gloss).min(1.0),
                        (0.025 + 0.09 * light + gloss * 0.72).min(1.0),
                        (0.04 + 0.08 * light + gloss * 0.62).min(1.0),
                    )
                } else {
                    RED
                };
                parent.spawn((
                    absolute_node(part),
                    ReferenceFill {
                        kind: FillKind::OrbStrip,
                        full: part,
                        strip_y: row as f32 + 0.5,
                        diameter: orb.height,
                    },
                    BackgroundColor(color),
                    FocusPolicy::Pass,
                ));
            }
        }
    }
    if let Some(art) = art {
        parent.spawn((
            absolute_node(device),
            ImageNode {
                image: art.orb_bezel.clone(),
                ..default()
            },
            FocusPolicy::Pass,
        ));
    }
    text(
        parent,
        "HP",
        CrystalRect::new(device.left + 34., device.top + 39., 28., 16.),
    );
    for (kind, rect, color) in [
        (FillKind::Hp, layout.hp_bar.expect("Reference HP bar"), RED),
        (FillKind::Mp, layout.mp_bar.expect("Reference MP bar"), BLUE),
        (FillKind::Experience, layout.experience_bar, GOLD),
        (FillKind::Weight, layout.weight_bar, CYAN),
    ] {
        if matches!(kind, FillKind::Mp) {
            let mut entity = parent.spawn((
                absolute_node(rect),
                MpBackdrop,
                BackgroundColor(BLACK),
                FocusPolicy::Pass,
            ));
            if let Some(art) = art {
                entity.insert(super::hud_art::frame_image(art, 0.012, 1.0));
            }
        } else {
            panel(parent, rect, if rect.height >= 12.0 { art } else { None });
        }
        let glass = art.is_some() && matches!(kind, FillKind::Hp | FillKind::Mp);
        let ink = if glass {
            CrystalRect::new(
                rect.left + 2.0,
                rect.top + 2.0,
                rect.width - 4.0,
                rect.height - 4.0,
            )
        } else {
            rect
        };
        fill(parent, kind, ink, color);
        if glass {
            fill(
                parent,
                kind,
                CrystalRect::new(ink.left, ink.top, ink.width, 2.0),
                if matches!(kind, FillKind::Hp) {
                    Color::srgb(0.95, 0.29, 0.25)
                } else {
                    Color::srgb(0.22, 0.68, 0.98)
                },
            );
        }
    }
}

pub(crate) fn spawn_belt(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    inventory: &InventoryModel,
    layout: ResolvedHudLayout,
    art: Option<&ReferenceHudArtwork>,
) {
    let mut node = absolute_node(layout.belt.frame(false));
    node.border = UiRect::all(Val::Px(1.));
    let mut frame = parent.spawn((
        node,
        CrystalHudBeltFrame,
        BeltPart::Frame,
        BackgroundColor(BLACK),
        BorderColor::all(GOLD),
        FocusPolicy::Pass,
    ));
    if let Some(art) = art {
        // The same corner-preserving component supports both resolved belt
        // orientations; sync_belt changes its rect without stretching a plate.
        frame.insert(super::hud_art::frame_image(art, 0.12, 1.0));
    }
    for slot in 0..6 {
        let rect = layout.belt.slot(slot, false);
        let mut node = absolute_node(rect);
        node.border = UiRect::all(Val::Px(1.));
        let mut slot_entity = parent.spawn((
            node,
            Button,
            Interaction::None,
            ReferenceButton,
            BeltPart::Slot(slot),
            CrystalHudBeltHitTarget { slot },
            CrystalHudAction::BeltUse(slot),
            BackgroundColor(BLACK),
            BorderColor::all(GOLD),
        ));
        if let Some(art) = art {
            slot_entity.insert(super::hud_art::frame_image(art, 0.045, 1.0));
        }
        slot_entity.with_children(|p| {
            p.spawn((
                CrystalHudBeltIcon { slot },
                original_item_image_bundle(
                    assets,
                    belt_slot_item(inventory, slot).and_then(ItemModel::user_item_image_index),
                    rect.width as i32,
                    rect.height as i32,
                ),
            ));
        });
        for (part, rect, value, count) in [
            (
                BeltPart::Key(slot),
                layout.belt.key(slot, false),
                (slot + 1).to_string(),
                false,
            ),
            (
                BeltPart::Count(slot),
                layout.belt.count(slot, false),
                belt_item_label(inventory, slot),
                true,
            ),
        ] {
            let mut e = parent.spawn((
                absolute_node(rect),
                part,
                Text::new(value),
                hud_font(None, CRYSTAL_DEFAULT_FONT_SIZE_PX),
                TextColor(Color::WHITE),
                FocusPolicy::Pass,
            ));
            if count {
                e.insert((
                    CrystalHudBeltItem { slot },
                    TextLayout::new(Justify::Right, bevy::text::LineBreak::NoWrap),
                ));
            } else {
                e.insert(CrystalHudBeltKey { slot });
            }
        }
    }
    button(
        parent,
        layout.belt.rotate(false),
        "R",
        "Rotate",
        (CrystalBeltControlAction::Rotate, BeltPart::Rotate),
        art,
        None,
    );
    button(
        parent,
        layout.belt.close(false),
        "X",
        "Close (Z)",
        (CrystalBeltControlAction::Close, BeltPart::Close),
        art,
        None,
    );
    parent.spawn((
        absolute_node(layout.labels.space),
        CrystalHudSpaceText,
        Text::new(format!("Free slots {}", free_inventory_slots(inventory))),
        hud_font(None, CRYSTAL_DEFAULT_FONT_SIZE_PX),
        TextColor(Color::WHITE),
        FocusPolicy::Pass,
    ));
}

pub(crate) fn sync_status(
    model: Res<UiReadModel>,
    layout: Res<HudLayoutSpec>,
    mut fills: Query<(&ReferenceFill, &mut Node, &mut BackgroundColor)>,
    mut mp_backdrops: Query<&mut Node, (With<MpBackdrop>, Without<ReferenceFill>)>,
) {
    if !layout.resolve().is_reference() {
        return;
    }
    let hp_only = super::hud_orb::crystal_hp_only(&model);
    for (fill, mut node, mut color) in &mut fills {
        let ratio = match fill.kind {
            FillKind::Hp | FillKind::OrbStrip => model.player.normalized_hp(),
            FillKind::Mp => model.player.normalized_mp(),
            FillKind::Experience => model.player.normalized_experience(),
            FillKind::Weight => model.player.normalized_weight(),
        };
        let visible = if matches!(fill.kind, FillKind::OrbStrip) {
            fill.strip_y >= fill.diameter * (1. - ratio)
        } else {
            !(matches!(fill.kind, FillKind::Mp) && hp_only)
        };
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
        if !matches!(fill.kind, FillKind::OrbStrip) {
            node.width = Val::Px((fill.full.width * ratio).floor());
        }
        if matches!(fill.kind, FillKind::Weight) {
            color.0 = match super::hud_bar::weight_bar_asset(ratio) {
                ("Prguse", _) => CYAN,
                (_, 473) => GOLD,
                _ => RED,
            };
        }
    }
    for mut node in &mut mp_backdrops {
        node.display = if hp_only {
            Display::None
        } else {
            Display::Flex
        };
    }
}

pub(crate) fn sync_belt(
    layout: Res<HudLayoutSpec>,
    belt: Res<CrystalBeltPresentation>,
    mut nodes: Query<(&BeltPart, &mut Node)>,
) {
    let geometry = layout.resolve();
    if !geometry.is_reference() {
        return;
    }
    for (part, mut node) in &mut nodes {
        let r = match *part {
            BeltPart::Frame => geometry.belt.frame(belt.vertical),
            BeltPart::Slot(s) => geometry.belt.slot(s, belt.vertical),
            BeltPart::Key(s) => geometry.belt.key(s, belt.vertical),
            BeltPart::Count(s) => geometry.belt.count(s, belt.vertical),
            BeltPart::Rotate => geometry.belt.rotate(belt.vertical),
            BeltPart::Close => geometry.belt.close(belt.vertical),
        };
        node.left = Val::Px(r.left);
        node.top = Val::Px(r.top);
        node.width = Val::Px(r.width);
        node.height = Val::Px(r.height);
        node.display = if belt.visible {
            Display::Flex
        } else {
            Display::None
        };
    }
}

pub(crate) fn sync_buttons(
    mut buttons: Query<
        (
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
            Option<&mut ImageNode>,
        ),
        (With<ReferenceButton>, Changed<Interaction>),
    >,
) {
    for (interaction, mut background, mut border, image) in &mut buttons {
        let (bg, edge) = match interaction {
            Interaction::None => (BLACK, GOLD),
            Interaction::Hovered => (Color::srgb(0.055, 0.095, 0.11), CYAN),
            Interaction::Pressed => (
                Color::srgb(0.18, 0.13, 0.055),
                Color::srgb(0.95, 0.74, 0.33),
            ),
        };
        background.0 = bg;
        *border = BorderColor::all(edge);
        if let Some(mut image) = image {
            image.color = match interaction {
                Interaction::None => Color::WHITE,
                Interaction::Hovered => Color::srgb(0.78, 1.0, 1.0),
                Interaction::Pressed => Color::srgb(1.0, 0.78, 0.45),
            };
        }
    }
}
