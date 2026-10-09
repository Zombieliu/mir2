//! The actual shared shop painter is measured with Bevy UI layout.
//! These tests create no Window, renderer, socket, or purchase acknowledgement.
use super::*;
use crate::npc_shop_ui::{NpcShopUiContext, NpcShopUiState, NpcShopUiStamp, NpcShopUiAction, NpcShopUiView};
use crate::npc_gold_buy_attempt::NpcGoldBuyFeedback;
use crate::inventory::{InventoryModel, CrystalItemInfoModel, CrystalItemTooltipSourceModel,
    CrystalUserItemModel, CrystalItemStatModel};
use crate::shop::{NpcShopServiceMode, ShopGood};
use bevy::camera::{ComputedCameraValues, RenderTargetInfo, Viewport};
use bevy::ui::{UiGlobalTransform, CalculatedClip};
use crate::crystal_ui::widget::CrystalItemHint;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
struct HostAction(NpcShopUiAction);
#[derive(Resource)]
struct Fixture {
    shop: ShopModel, player: PlayerStats, view: NpcShopUiView,
    options: ShopPaintOptions, camera: Entity, assets_available: bool, clip_height: Option<f32>,
}
fn source_good(id: u64) -> ShopGood {
    ShopGood { unique_id: id, name: format!("Potion {id}"), price: 1, count: 1,
        stock: -1, panel_type: 0, purchase_rate: Some(1.5), requires_gold_buy_plan: true,
        tooltip_source: Some(CrystalItemTooltipSourceModel {
            info: CrystalItemInfoModel { item_index: 658, name: "Potion".into(), price: 1,
                stack_size: 99, item_type: 13, ..Default::default() },
            user_item: Some(CrystalUserItemModel { unique_id: id, item_index: 658,
                count: 1, is_shop_item: true, ..Default::default() }), ..Default::default()
        }), ..Default::default() }
}
fn shop() -> ShopModel {
    ShopModel { goods: (0..10).map(source_good).collect(), service_mode: NpcShopServiceMode::Buy,
        supports_buy: true, ..Default::default() }
}
fn context() -> NpcShopUiContext {
    NpcShopUiContext { source_revision: Some(7), open: true, input_enabled: true, show_buy: true }
}
fn view(shop: &ShopModel, inventory: &InventoryModel, feedback: NpcGoldBuyFeedback, pages: usize) -> NpcShopUiView {
    let mut state = NpcShopUiState::default(); let context = context();
    state.reconcile(context, shop, inventory);
    state.apply(NpcShopUiAction::Select(0), context, shop, inventory, feedback);
    for _ in 0..pages { state.apply(NpcShopUiAction::PageDown, context, shop, inventory, feedback); }
    state.view(context, shop, inventory, feedback)
}
fn spawn_fixture(mut commands: Commands, assets: Res<AssetServer>, fixture: Res<Fixture>) {
    commands.spawn((Node { width: Val::Px(1600.0), height: Val::Px(fixture.clip_height.unwrap_or(1200.0)),
        overflow: if fixture.clip_height.is_some() { Overflow::clip() } else { Overflow::visible() }, ..default() },
        UiTargetCamera(fixture.camera))).with_children(|parent| {
        paint_npc_gold_shop(parent, fixture.assets_available.then_some(assets.as_ref()),
            &fixture.shop, &fixture.player, &fixture.view, &fixture.options, HostAction);
    });
}
fn painted(shop: ShopModel, view: NpcShopUiView, scale: f32, assets_available: bool) -> App {
    painted_with_clip(shop, view, scale, assets_available, None)
}
fn painted_with_clip(shop: ShopModel, view: NpcShopUiView, scale: f32, assets_available: bool, clip_height: Option<f32>) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default(), bevy::input::InputPlugin,
        bevy::image::ImagePlugin::default(), bevy::transform::TransformPlugin,
        bevy::camera::visibility::VisibilityPlugin, bevy::text::TextPlugin, bevy::ui::UiPlugin));
    app.init_asset::<Image>().init_asset::<Font>().init_asset::<bevy::image::TextureAtlasLayout>()
        .init_asset::<bevy::mesh::Mesh>().init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();
    let font = Font::from_bytes(std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/original-ui/fonts/NotoSansCJKsc-Regular.otf")).expect("packaged UI font"));
    let font = app.world_mut().resource_mut::<Assets<Font>>().add(font);
    let size = UVec2::new(1600, 1200);
    let camera = app.world_mut().spawn((Camera2d, Camera { computed: ComputedCameraValues {
        target_info: Some(RenderTargetInfo { physical_size: size, scale_factor: 1.0 }), ..default()
    }, viewport: Some(Viewport { physical_size: size, ..default() }), ..default() })).id();
    app.insert_resource(Fixture { shop, player: PlayerStats::default(), view,
        options: ShopPaintOptions { font: Some(TextFont { font: font.into(), font_size: FontSize::Px(10.0),
            ..default() }), scale, ..Default::default() }, camera, assets_available, clip_height });
    app.add_systems(Startup, spawn_fixture);
    for _ in 0..4 { app.update(); } app
}
fn rect(node: &ComputedNode, transform: &UiGlobalTransform) -> CrystalRect {
    let size = node.size() * node.inverse_scale_factor;
    let center = transform.affine().translation * node.inverse_scale_factor;
    CrystalRect::new(center.x - size.x * 0.5, center.y - size.y * 0.5, size.x, size.y)
}
fn near(actual: f32, expected: f32) { assert!((actual - expected).abs() < 0.1, "{actual} != {expected}"); }
fn feedback() -> NpcGoldBuyFeedback { NpcGoldBuyFeedback { can_reserve: true, ..Default::default() } }
fn panel_count(app: &mut App) -> usize { let world = app.world_mut(); world.query::<&ShopPaintPanel>().iter(world).count() }

#[test]
fn shop_paint_actual_computed_rows_controls_and_stamps_match_original_geometry() {
    let shop = shop(); let inventory = InventoryModel { gold: 100, ..Default::default() };
    let view = view(&shop, &inventory, feedback(), 0); let stamp = view.stamp.unwrap();
    let mut app = painted(shop, view, 1.0, true); let world = app.world_mut();
    let panel = world.query_filtered::<(&ComputedNode, &UiGlobalTransform), With<ShopPaintPanel>>()
        .single(world).map(|(node, transform)| rect(node, transform)).unwrap();
    near(panel.width, 244.0); near(panel.height, 334.0);
    let mut rows: Vec<_> = world.query::<(&ShopPaintGoodCell, &ComputedNode, &UiGlobalTransform,
        &ShopPaintControl, &NpcShopUiStamp, &HostAction, &Button, &NpcShopUiAction)>()
        .iter(world).map(|(cell, node, transform, control, current, host, _, action)| {
            assert_eq!(*current, stamp); assert!(control.enabled);
            assert_eq!(control.action, NpcShopUiAction::Select(cell.unique_id));
            assert_eq!(host.0, control.action); assert_eq!(*action, control.action);
            (*cell, rect(node, transform))
        }).collect();
    rows.sort_by_key(|row| row.0.unique_id); assert_eq!(rows.len(), 8);
    for (index, (cell, bounds)) in rows.iter().enumerate() {
        assert_eq!(cell.unique_id, index as u64); assert_eq!(cell.selected, index == 0);
        near(bounds.left - panel.left, 10.0); near(bounds.top - panel.top, 34.0 + index as f32 * 33.0);
        near(bounds.width, 205.0); near(bounds.height, 32.0);
        let (x, y) = bounds.center(); assert_eq!(rows.iter().filter(|row| row.1.contains(x, y)).count(), 1);
        assert!(panel.contains(x, y));
    }
    let controls: Vec<_> = world.query::<(&ShopPaintControl, &NpcShopUiStamp, Option<&Button>,
        Option<&HostAction>, Option<&NpcShopUiAction>)>().iter(world).collect();
    for (control, current, button, host, action) in controls {
        assert_eq!(*current, stamp);
        assert_eq!(button.is_some(), control.enabled); assert_eq!(host.is_some(), control.enabled);
        assert_eq!(action.is_some(), control.enabled);
    }
}

#[test]
fn shop_paint_buy_disabled_by_wallet_or_attempt_retains_control_without_click_action() {
    for (gold, attempt) in [(0, feedback()), (100, NpcGoldBuyFeedback { pending: true,
        phase: Some(crate::npc_gold_buy_attempt::NpcGoldBuyAttemptPhase::Unknown), ..Default::default() })] {
        let shop = shop(); let inventory = InventoryModel { gold, ..Default::default() };
        let view = view(&shop, &inventory, attempt, 0); assert!(!view.can_buy);
        let mut app = painted(shop, view, 1.0, true); let world = app.world_mut();
        let rows: Vec<_> = world.query::<(&ShopPaintControl, &NpcShopUiStamp, Option<&Button>,
            Option<&HostAction>, Option<&NpcShopUiAction>, &ComputedNode)>().iter(world)
            .filter(|row| row.0.action == NpcShopUiAction::Buy).collect();
        assert_eq!(rows.len(), 1); let (control, _, button, host, action, node) = rows[0];
        assert!(!control.enabled); assert!(button.is_none() && host.is_none() && action.is_none());
        assert!(node.size().x > 0.0 && node.size().y > 0.0);
        assert_eq!(world.query::<&CrystalItemHint>().iter(world).count(), 8);
    }
}

#[test]
fn shop_paint_paging_uses_controller_window_without_filtering_catalog_uids() {
    let shop = shop(); let inventory = InventoryModel { gold: 100, ..Default::default() };
    let view = view(&shop, &inventory, feedback(), 2); assert_eq!(view.start_index, 2);
    let stamp = view.stamp.unwrap(); let mut app = painted(shop, view, 1.0, true); let world = app.world_mut();
    let mut ids: Vec<_> = world.query::<(&ShopPaintGoodCell, &NpcShopUiStamp)>().iter(world)
        .map(|(cell, current)| { assert_eq!(*current, stamp); assert!(!cell.selected); cell.unique_id }).collect();
    ids.sort(); assert_eq!(ids, (2..10).collect::<Vec<_>>());
    assert_eq!(world.query::<&HostAction>().iter(world).filter(|action| action.0 == NpcShopUiAction::PageDown).count(), 0);
    assert_eq!(world.query::<&HostAction>().iter(world).filter(|action| action.0 == NpcShopUiAction::PageUp).count(), 1);
}

#[test]
fn shop_paint_offpage_special_catalog_entry_prevents_any_partial_common_tree() {
    for special in 0..3 {
        let mut shop = shop();
        match special { 0 => shop.goods[9].use_pearls = true, 1 => shop.goods[9].stock = 1,
            _ => shop.goods[9].panel_type = 1 }
        let inventory = InventoryModel { gold: 100, ..Default::default() };
        let view = view(&shop, &inventory, feedback(), 0); assert!(view.stamp.is_none());
        let mut app = painted(shop, view, 1.0, true); assert_eq!(panel_count(&mut app), 0);
        let world = app.world_mut(); assert_eq!(world.query::<&ShopPaintGoodCell>().iter(world).count(), 0);
        assert_eq!(world.query::<&HostAction>().iter(world).count(), 0);
    }
}

#[test]
fn shop_paint_missing_raw_marked_good_stays_common_but_cannot_buy() {
    let mut shop = shop(); shop.goods[0].tooltip_source = None; shop.goods[0].purchase_rate = None;
    let inventory = InventoryModel { gold: 100, ..Default::default() };
    let view = view(&shop, &inventory, feedback(), 0); assert!(view.stamp.is_some()); assert!(!view.can_buy);
    let mut app = painted(shop, view, 1.0, true); assert_eq!(panel_count(&mut app), 1);
    let world = app.world_mut();
    assert_eq!(world.query::<&HostAction>().iter(world).filter(|action| action.0 == NpcShopUiAction::Buy).count(), 0);
}

#[test]
fn shop_paint_scale_and_font_apply_to_actual_nodes_without_redefining_original_geometry() {
    let shop = shop(); let inventory = InventoryModel { gold: 100, ..Default::default() };
    let view = view(&shop, &inventory, feedback(), 0); let scale = 1.5;
    let mut app = painted(shop, view, scale, true); let world = app.world_mut();
    let (node, transform) = world.query_filtered::<(&ComputedNode, &UiGlobalTransform), With<ShopPaintPanel>>()
        .single(world).unwrap(); let bounds = rect(node, transform);
    near(bounds.width, 244.0 * scale); near(bounds.height, 334.0 * scale);
    for (node, transform) in world.query_filtered::<(&ComputedNode, &UiGlobalTransform), With<ShopPaintGoodCell>>().iter(world) {
        let bounds = rect(node, transform);
        // Bevy defaults to rounding physical pixels; this fixture uses scale factor 1.
        near(node.unrounded_size().x * node.inverse_scale_factor, 205.0 * scale);
        near(bounds.width, (205.0 * scale).round()); near(bounds.height, 32.0 * scale);
    }
    let expected = world.resource::<Fixture>().options.font.as_ref().unwrap().font.clone();
    for (font, name, price, count) in world.query::<(&TextFont, Option<&ShopPaintGoodName>, Option<&ShopPaintGoodPrice>, Option<&ShopPaintGoodCount>)>().iter(world) {
        assert_eq!(font.font, expected);
        let original_size = if name.is_some() || price.is_some() || count.is_some() { 9.0 } else { 10.0 };
        assert_eq!(font.font_size, FontSize::Px(original_size * scale));
    }
}

#[test]
fn shop_paint_missing_assets_stamp_or_invalid_scale_never_fabricates_a_panel() {
    let shop = shop(); let inventory = InventoryModel { gold: 100, ..Default::default() };
    let view = view(&shop, &inventory, feedback(), 0);
    for (assets_available, scale, no_stamp) in [(false, 1.0, false), (true, 0.0, false),
        (true, f32::NAN, false), (true, 1.0, true)] {
        let mut current = view.clone(); if no_stamp { current.stamp = None; }
        let mut app = painted(shop.clone(), current, scale, assets_available); assert_eq!(panel_count(&mut app), 0);
        let world = app.world_mut(); assert_eq!(world.query::<&ShopPaintControl>().iter(world).count(), 0);
    }
}

#[test]
fn shop_paint_rich_tooltip_is_full_and_hide_added_stats_uses_actual_document_options() {
    for hidden in [false, true] {
        let mut shop = shop(); shop.hide_added_stats = hidden;
        let raw = shop.goods[0].tooltip_source.as_mut().unwrap();
        raw.info.name = "Wooden Sword".into(); raw.info.item_type = 1; raw.info.stack_size = 1;
        raw.info.stats = vec![CrystalItemStatModel { stat: 4, value: 2 }, CrystalItemStatModel { stat: 5, value: 4 }];
        raw.user_item.as_mut().unwrap().added_stats = vec![CrystalItemStatModel { stat: 5, value: 9 }];
        let inventory = InventoryModel { gold: 100, ..Default::default() };
        let view = view(&shop, &inventory, feedback(), 0);
        let mut app = painted(shop, view, 1.0, true); let world = app.world_mut();
        let hint = world.query::<(&ShopPaintGoodCell, &CrystalItemHint)>().iter(world)
            .find(|(cell, _)| cell.unique_id == 0).map(|(_, hint)| hint).expect("actual first-row tooltip");
        assert!(hint.0.source_complete);
        assert!(hint.0.plain_text().contains(if hidden { "DC + 2~4" } else { "DC + 2~13 (+9)" }));
        assert_eq!(hint.0.plain_text().contains("(+9)"), !hidden);
    }
}

#[test]
fn shop_paint_actual_calculated_clip_excludes_lower_row_centers_without_rewriting_rows() {
    let shop = shop(); let inventory = InventoryModel { gold: 100, ..Default::default() };
    let view = view(&shop, &inventory, feedback(), 0);
    let mut app = painted_with_clip(shop, view, 1.0, true, Some(100.0)); let world = app.world_mut();
    let mut rows: Vec<_> = world.query::<(&ShopPaintGoodCell, &ComputedNode, &UiGlobalTransform, &CalculatedClip)>()
        .iter(world).map(|(cell, node, transform, clip)| (*cell, rect(node, transform), clip.clip)).collect();
    rows.sort_by_key(|row| row.0.unique_id); assert_eq!(rows.len(), 8);
    let (_, first, first_clip) = rows.first().unwrap(); let (x, y) = first.center();
    assert!(first_clip.contains(Vec2::new(x, y)), "first actual row remains visible");
    let (_, last, last_clip) = rows.last().unwrap(); let (x, y) = last.center();
    assert!(!last_clip.contains(Vec2::new(x, y)), "clipped lower row cannot be a visible center hit");
    near(last.height, 32.0); assert!(last.top > first.top);
    // This proves actual inherited clipping metadata, not a live pointer/host claim.
}

#[test]
fn shop_paint_input_disabled_goods_keep_full_hints_but_have_no_executable_controls() {
    let shop = shop(); let inventory = InventoryModel { gold: 100, ..Default::default() };
    let mut current = context(); current.input_enabled = false;
    let mut state = NpcShopUiState::default(); state.reconcile(current, &shop, &inventory);
    let view = state.view(current, &shop, &inventory, feedback());
    assert!(view.stamp.is_some()); assert!(!view.can_select && !view.can_buy);
    let mut app = painted(shop, view, 1.0, true); let world = app.world_mut();
    assert_eq!(world.query::<&ShopPaintGoodCell>().iter(world).count(), 8);
    let hints: Vec<_> = world.query::<&CrystalItemHint>().iter(world).collect();
    assert_eq!(hints.len(), 8); assert!(hints.iter().all(|hint| hint.0.source_complete));
    for (control, button, action, host) in world.query::<(&ShopPaintControl, Option<&Button>,
        Option<&NpcShopUiAction>, Option<&HostAction>)>().iter(world) {
        assert!(!control.enabled); assert!(button.is_none() && action.is_none() && host.is_none());
    }
}
