//! Shared ordinary Gold NPC goods tree. Hosts own the Gate and transport;
//! this module only paints planner output and emits stamped local controls.
use bevy::prelude::*;
use bevy::text::{Justify, LineBreak, TextLayout};
use bevy::ui::FocusPolicy;
use super::assets::CrystalButtonAssetSet;
use super::item_image::{original_item_image_bundle, OriginalItemPaintScale};
use super::item_tooltip::{crystal_item_tooltip_document_from_source_with_options, CrystalItemTooltipOptions};
use super::spec::{CrystalButtonSpec, CrystalRect};
use super::widget::{spawn_crystal_image_button, CrystalItemHint};
use crate::npc_gold_buy_attempt::NpcGoldBuyAttemptPhase;
use crate::npc_shop_buy::NpcGoldBuyBlockReason;
use crate::npc_shop_ui::{ordinary_gold_surface, NpcShopUiAction, NpcShopUiStamp, NpcShopUiView, NPC_SHOP_VISIBLE_ROWS};
use crate::read_model::PlayerStats;
use crate::shop::{ShopGood, ShopModel};

pub const SHOP_PANEL_WIDTH: f32 = 244.0;
pub const SHOP_PANEL_HEIGHT: f32 = 334.0;
pub const SHOP_REQUIRED_SKINS: &[&str] = &[
    "original-ui/Prguse/1000.png",
    "original-ui/Prguse2/360.png", "original-ui/Prguse2/361.png", "original-ui/Prguse2/362.png",
    "original-ui/Prguse2/197.png", "original-ui/Prguse2/198.png", "original-ui/Prguse2/199.png",
    "original-ui/Prguse2/207.png", "original-ui/Prguse2/208.png", "original-ui/Prguse2/209.png",
    "original-ui/Prguse2/205.png",
    "original-ui/Title/312.png", "original-ui/Title/313.png", "original-ui/Title/314.png",
    "original-ui/Prguse/550.png",
];

#[derive(Component, Debug)]
pub struct ShopPaintPanel;
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShopPaintControl { pub action: NpcShopUiAction, pub enabled: bool }
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShopPaintGoodCell { pub unique_id: u64, pub selected: bool }
#[derive(Component, Debug)]
pub struct ShopPaintGoodName;
#[derive(Component, Debug)]
pub struct ShopPaintGoodPrice;
#[derive(Component, Debug)]
pub struct ShopPaintGoodCount;
#[derive(Component, Debug)]
pub struct ShopPaintGoodIcon;
#[derive(Component, Debug)]
pub struct ShopPaintGoodSelectionDivider;
#[derive(Component, Debug)]
pub struct ShopPaintGoodNewIcon;
#[derive(Component, Debug)]
pub struct ShopPaintFeedback;

#[derive(Debug, Clone)]
pub struct ShopPaintLabels {
    pub no_goods: String, pub gold: String, pub total: String,
    pub waiting: String, pub entered: String, pub flushed: String,
    pub unknown: String, pub definitely_unsent: String, pub blocked: String,
}
impl Default for ShopPaintLabels {
    fn default() -> Self {
        Self {
            no_goods: "No goods".into(), gold: "Gold".into(), total: "Total".into(),
            waiting: "Purchase waiting to send".into(),
            entered: "Purchase sending; result unconfirmed".into(),
            flushed: "Purchase sent; result unconfirmed".into(),
            unknown: "Purchase result unknown; automatic retry disabled".into(),
            definitely_unsent: "Purchase not sent; choose Buy to retry".into(),
            blocked: "Cannot buy this item".into(),
        }
    }
}
#[derive(Clone)]
pub struct ShopPaintOptions {
    /// Fixed pixel font. None keeps the Native Crystal Arial 10px face.
    pub font: Option<TextFont>, pub scale: f32, pub labels: ShopPaintLabels,
}
impl Default for ShopPaintOptions {
    fn default() -> Self { Self { font: None, scale: 1.0, labels: ShopPaintLabels::default() } }
}
fn scaled(rect: CrystalRect, scale: f32) -> CrystalRect {
    CrystalRect::new(rect.left * scale, rect.top * scale, rect.width * scale, rect.height * scale)
}
fn node(rect: CrystalRect) -> Node {
    Node { position_type: PositionType::Absolute, left: Val::Px(rect.left), top: Val::Px(rect.top),
        width: Val::Px(rect.width), height: Val::Px(rect.height), ..default() }
}
fn label(parent: &mut ChildSpawnerCommands, text: String, rect: CrystalRect, font: &TextFont, color: Color) {
    parent.spawn((Node { overflow: Overflow::clip(), ..node(rect) }, FocusPolicy::Pass))
        .with_children(|parent| { parent.spawn((Text::new(text), font.clone(), TextColor(color),
            TextLayout::new(Justify::Left, LineBreak::NoWrap), FocusPolicy::Pass)); });
}
#[allow(clippy::too_many_arguments)]
fn image_control<B: Bundle>(parent: &mut ChildSpawnerCommands, assets: &AssetServer,
    library: &'static str, indices: [u16; 3], rect: CrystalRect, action: NpcShopUiAction,
    enabled: bool, stamp: NpcShopUiStamp, map: &mut impl FnMut(NpcShopUiAction) -> B) {
    let spec = CrystalButtonSpec::new(library, indices[0], indices[1], indices[2], rect, rect.width, rect.height);
    let control = ShopPaintControl { action, enabled };
    if enabled {
        spawn_crystal_image_button(parent, assets, spec, CrystalButtonAssetSet::from_spec(spec),
            (control, stamp, action, map(action)), false, true);
    } else {
        spawn_crystal_image_button(parent, assets, spec, CrystalButtonAssetSet::from_spec(spec),
            (control, stamp), false, false);
    }
}
#[allow(clippy::too_many_arguments)]
fn text_control<B: Bundle>(parent: &mut ChildSpawnerCommands, text: &str, rect: CrystalRect,
    font: &TextFont, action: NpcShopUiAction, enabled: bool, stamp: NpcShopUiStamp,
    map: &mut impl FnMut(NpcShopUiAction) -> B) {
    let mut entity = parent.spawn((ShopPaintControl { action, enabled }, stamp,
        Node { justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..node(rect) },
        BackgroundColor(if enabled { Color::srgba(0.12, 0.08, 0.04, 0.60) }
            else { Color::srgba(0.25, 0.20, 0.12, 0.28) }), FocusPolicy::Block));
    if enabled { entity.insert((Button, action, map(action))); }
    entity.with_children(|parent| { parent.spawn((Text::new(text), font.clone(),
        TextColor(if enabled { Color::WHITE } else { Color::srgba(0.8, 0.8, 0.8, 0.5) }),
        TextLayout::new(Justify::Center, LineBreak::NoWrap), FocusPolicy::Pass)); });
}

/// Original MirGoodsCell new-item marker rule, independent of purchase policy.
pub fn shop_good_new_icon_visible(good: &ShopGood, goods: &[ShopGood]) -> bool {
    let Some(source) = good.tooltip_source.as_ref() else { return false; };
    let Some(item) = source.user_item.as_ref() else { return false; };
    let mut matching = 0usize;
    let mut has_non_shop_item = false;
    for candidate in goods {
        let Some(candidate_source) = candidate.tooltip_source.as_ref() else { continue; };
        if candidate_source.info.item_index != source.info.item_index { continue; }
        matching += 1;
        has_non_shop_item |= candidate_source.user_item.as_ref().is_some_and(|candidate| !candidate.is_shop_item);
    }
    !item.is_shop_item || (matching > 1 && has_non_shop_item)
}

#[allow(clippy::too_many_arguments)]
fn good_cell<B: Bundle>(parent: &mut ChildSpawnerCommands, assets: &AssetServer, good: &ShopGood,
    rect: CrystalRect, selected: bool, enabled: bool, stamp: NpcShopUiStamp,
    shop: &ShopModel, player: &PlayerStats, font: &TextFont, scale: f32,
    map: &mut impl FnMut(NpcShopUiAction) -> B) {
    let action = NpcShopUiAction::Select(good.unique_id);
    let mut entity = parent.spawn((ShopPaintGoodCell { unique_id: good.unique_id, selected },
        ShopPaintControl { action, enabled }, stamp, node(rect), FocusPolicy::Block,
        BackgroundColor(Color::NONE), Outline::new(Val::Px(scale), Val::Px(0.0),
            if selected { Color::srgb(0.0, 1.0, 0.0) } else { Color::NONE })));
    if enabled { entity.insert((Button, action, map(action))); }
    // Disabled goods still retain the complete source-authored hint.
    if let Some(document) = crystal_item_tooltip_document_from_source_with_options(
        &good.name, good.icon, u32::from(good.count), good.tooltip_source.as_ref(), player,
        CrystalItemTooltipOptions { hide_added_stats: shop.hide_added_stats, ..Default::default() }) {
        entity.insert((CrystalItemHint(document), Interaction::None));
    }
    entity.with_children(|row| {
        if let Some(index) = good.user_item_image_index() {
            row.spawn((ShopPaintGoodIcon, original_item_image_bundle(assets, Some(index), 40, 32),
                OriginalItemPaintScale(scale), FocusPolicy::Pass));
        }
        row.spawn((ShopPaintGoodName, node(scaled(CrystalRect::new(44.0, 0.0, 159.0, 14.0), scale)),
            Text::new(crate::player_text::name(&good.name)), font.clone(), TextColor(Color::WHITE),
            TextLayout::new(Justify::Left, LineBreak::NoWrap), FocusPolicy::Pass));
        row.spawn((ShopPaintGoodPrice, node(scaled(CrystalRect::new(44.0, 14.0, 159.0, 18.0), scale)),
            Text::new(crate::player_text::text(&good.price_label())), font.clone(), TextColor(Color::WHITE),
            TextLayout::new(Justify::Left, LineBreak::NoWrap), FocusPolicy::Pass));
        if good.count > 1 {
            row.spawn((ShopPaintGoodCount, node(scaled(CrystalRect::new(23.0, 17.0, 17.0, 15.0), scale)),
                Text::new(good.count.to_string()), font.clone(), TextColor(Color::srgb(1.0, 1.0, 0.0)),
                TextLayout::new(Justify::Left, LineBreak::NoWrap), FocusPolicy::Pass));
        }
        if selected { row.spawn((ShopPaintGoodSelectionDivider,
            node(scaled(CrystalRect::new(40.0, 0.0, 1.0, 32.0), scale)),
            BackgroundColor(Color::srgb(0.0, 1.0, 0.0)), FocusPolicy::Pass)); }
        if shop_good_new_icon_visible(good, &shop.goods) {
            row.spawn((ShopPaintGoodNewIcon, node(scaled(CrystalRect::new(190.0, 5.0, 12.0, 9.0), scale)),
                ImageNode { image: assets.load("original-ui/Prguse/550.png"), ..default() }, FocusPolicy::Pass));
        }
    });
}
fn feedback_text(view: &NpcShopUiView, labels: &ShopPaintLabels) -> String {
    if let Some(phase) = view.feedback.phase {
        return match phase {
            NpcGoldBuyAttemptPhase::Queued | NpcGoldBuyAttemptPhase::Bound => &labels.waiting,
            NpcGoldBuyAttemptPhase::Entered => &labels.entered,
            NpcGoldBuyAttemptPhase::Flushed => &labels.flushed,
            NpcGoldBuyAttemptPhase::Unknown => &labels.unknown,
            NpcGoldBuyAttemptPhase::DefinitelyUnsent => &labels.definitely_unsent,
        }.clone();
    }
    match view.plan.block_reason {
        Some(NpcGoldBuyBlockReason::NoSelection) => "Select an item".into(),
        Some(NpcGoldBuyBlockReason::InsufficientGold) => "Not enough gold".into(),
        Some(NpcGoldBuyBlockReason::InventoryFull) => "No empty inventory cell".into(),
        Some(NpcGoldBuyBlockReason::InvalidQuantity) => "Choose a valid quantity".into(),
        Some(_) => labels.blocked.clone(),
        None => String::new(),
    }
}

/// Original eight-row shop geometry, suitable for both Native and portable hosts.
/// Readiness/input ownership is the host's responsibility; no missing source,
/// assets or invalid pixel layout can produce an apparently ready new tree.
pub fn paint_npc_gold_shop<B: Bundle>(parent: &mut ChildSpawnerCommands, assets: Option<&AssetServer>,
    shop: &ShopModel, player: &PlayerStats, view: &NpcShopUiView, options: &ShopPaintOptions,
    mut map: impl FnMut(NpcShopUiAction) -> B) {
    let (Some(assets), Some(stamp)) = (assets, view.stamp) else { return; };
    let scale = options.scale;
    if !scale.is_finite() || scale <= 0.0 || !ordinary_gold_surface(shop, true)
        || view.start_index > shop.goods.len().saturating_sub(NPC_SHOP_VISIBLE_ROWS) { return; }
    let mut font = options.font.clone().unwrap_or_else(|| super::typography::crystal_text_font(10.0));
    let FontSize::Px(font_size) = font.font_size else { return; };
    if !font_size.is_finite() || font_size <= 0.0 || !(font_size * scale).is_finite()
        || font_size * scale <= 0.0 || !(584.0 * scale).is_finite()
        || SHOP_PANEL_HEIGHT * scale <= 0.0 { return; }
    font.font_size = FontSize::Px(font_size * scale);
    let mut row_font = font.clone(); row_font.font_size = FontSize::Px(font_size * scale * 0.9);
    parent.spawn((ShopPaintPanel, stamp, node(scaled(CrystalRect::new(0.0, 0.0, SHOP_PANEL_WIDTH, SHOP_PANEL_HEIGHT), scale)),
        FocusPolicy::Block, ImageNode { image: assets.load("original-ui/Prguse/1000.png"), ..default() }))
        .with_children(|parent| {
            image_control(parent, assets, "Prguse2", [360,361,362],
                scaled(CrystalRect::new(217.0,3.0,24.0,21.0), scale), NpcShopUiAction::Close, view.can_select, stamp, &mut map);
            image_control(parent, assets, "Prguse2", [197,198,199],
                scaled(CrystalRect::new(219.0,35.0,12.0,12.0), scale), NpcShopUiAction::PageUp, view.can_page_up, stamp, &mut map);
            image_control(parent, assets, "Prguse2", [207,208,209],
                scaled(CrystalRect::new(219.0,284.0,12.0,12.0), scale), NpcShopUiAction::PageDown, view.can_page_down, stamp, &mut map);
            let max_start = shop.goods.len().saturating_sub(NPC_SHOP_VISIBLE_ROWS);
            let scroll_top = if max_start == 0 { 49.0 } else { 49.0 + 217.0 * view.start_index as f32 / max_start as f32 };
            parent.spawn((node(scaled(CrystalRect::new(219.0,scroll_top,12.0,18.0), scale)),
                ImageNode { image: assets.load("original-ui/Prguse2/205.png"), ..default() }, FocusPolicy::Pass));
            image_control(parent, assets, "Title", [312,313,314],
                scaled(CrystalRect::new(77.0,304.0,80.0,25.0), scale), NpcShopUiAction::Buy, view.can_buy, stamp, &mut map);
            for (row, good) in shop.goods.iter().skip(view.start_index).take(NPC_SHOP_VISIBLE_ROWS).enumerate() {
                good_cell(parent, assets, good, scaled(CrystalRect::new(10.0,34.0 + 33.0 * row as f32,205.0,32.0), scale),
                    view.selected_id == Some(good.unique_id), view.can_select, stamp, shop, player, &row_font, scale, &mut map);
            }
            let gold = view.plan.total_gold.map(|total| format!("{} {} / {} {}", options.labels.gold, view.gold, options.labels.total, total))
                .unwrap_or_else(|| format!("{} {}", options.labels.gold, view.gold));
            label(parent, crate::player_text::text(&gold), scaled(CrystalRect::new(10.0,8.0,205.0,16.0), scale), &font, Color::srgb(1.0,0.84,0.0));
            text_control(parent, "−", scaled(CrystalRect::new(12.0,304.0,20.0,22.0), scale), &font,
                NpcShopUiAction::QuantityDec, view.can_dec, stamp, &mut map);
            label(parent, format!("x{}", view.quantity), scaled(CrystalRect::new(34.0,306.0,36.0,18.0), scale), &font, Color::WHITE);
            text_control(parent, "+", scaled(CrystalRect::new(58.0,304.0,18.0,22.0), scale), &font,
                NpcShopUiAction::QuantityInc, view.can_inc, stamp, &mut map);
            if view.show_sell { text_control(parent, "Sell", scaled(CrystalRect::new(162.0,304.0,52.0,22.0), scale), &font,
                NpcShopUiAction::HandoffSell, view.can_select, stamp, &mut map); }
            // This notice uses the original host's right-hand free space. It
            // does not enlarge or move the source frame and its hit controls.
            parent.spawn((ShopPaintFeedback, node(scaled(CrystalRect::new(254.0,34.0,330.0,80.0), scale)), FocusPolicy::Pass))
                .with_children(|notice| { label(notice, crate::player_text::text(&feedback_text(view, &options.labels)),
                    scaled(CrystalRect::new(0.0,0.0,330.0,80.0), scale), &font, Color::WHITE); });
        });
}

#[cfg(test)]
#[path = "shop_paint_tests.rs"]
mod tests;
