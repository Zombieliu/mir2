//! NPCDropDialog source geometry with ordinary bag drag selection.
//! The equipment picker remains an explicit adapter until equipment drag is wired.
use super::*;

const CONFIRM: CrystalRect = CrystalRect::new(114.0, 62.0, 48.0, 25.0);
const ITEM: CrystalRect = CrystalRect::new(38.0, 72.0, 36.0, 32.0);
// NPCDropPanel_Click forwards this wider source rectangle to ItemCell_Click.
const ITEM_CLICK_TARGET: CrystalRect = CrystalRect::new(20.0, 55.0, 75.0, 75.0);
// `OverlayShop` owns the Crystal NPCDropDialog's source-relative frame at
// (0, 224).  Keep hit testing beside the source ItemCell geometry instead of
// teaching the generic inventory drag code about service-specific layout.
const NPC_DROP_PANEL_TOP: f32 = 224.0;
const LOW_GOLD_NOTICE: &str = "Not enough gold.";

pub(super) fn service_action(shop: &ShopModel) -> Option<(&'static str, OverlayButton)> {
    if shop.allows_special_repair() {
        Some(("Special repair", OverlayButton::ShopSRepair))
    } else if shop.allows_repair() {
        Some(("Repair", OverlayButton::ShopRepair))
    } else if shop.allows_sell() {
        Some(("Sale", OverlayButton::ShopSell))
    } else {
        None
    }
}

pub(super) fn drag_target_at_cursor(
    shop: &ShopModel,
    state: &NativePlayerUiState,
    cursor: Vec2,
) -> bool {
    state.npc_shop_open()
        && service_action(shop).is_some()
        && CrystalRect::new(
            ITEM_CLICK_TARGET.left,
            NPC_DROP_PANEL_TOP + ITEM_CLICK_TARGET.top,
            ITEM_CLICK_TARGET.width,
            ITEM_CLICK_TARGET.height,
        )
        .contains(cursor.x, cursor.y)
}

/// Crystal's `NPCDropDialog.ItemCell_Click` takes the currently selected
/// inventory cell, locks it as TargetItem, and invokes Confirm only while
/// Hold is enabled. A drag has already captured the source identity, so
/// reject a replacement at the same bag slot before changing selection.
pub(super) fn select_bag_dragged_for_service(
    shop: &mut ShopModel,
    inventory: &InventoryModel,
    state: &mut NativePlayerUiState,
    source_slot: u32,
    unique_id: u64,
    intents: &mut NativePlayerUiIntentQueue,
    pending: &mut PendingOperations,
) -> bool {
    if !state.npc_shop_open()
        || service_action(shop).is_none()
        || !inventory.items.iter().any(|item| {
            item.container == 0
                && item.slot == source_slot
                && item_unique_id(item) == Some(unique_id)
        })
    {
        return false;
    }

    let dragged_count = inventory
        .items
        .iter()
        .find(|item| item.container == 0 && item.slot == source_slot)
        .map(|item| item.quantity.min(u32::from(u16::MAX)) as u16);
    if shop.allows_sell() {
        shop.selected_bag_slot_for_sell = Some(source_slot);
        state.shop_service_drag_count = dragged_count;
        state.shop_service_drag_unique_id = Some(unique_id);
    } else if shop.allows_repair() || shop.allows_special_repair() {
        shop.selected_bag_slot_for_repair = Some(source_slot);
        state.shop_service_drag_count = None;
        state.shop_service_drag_unique_id = Some(unique_id);
        state.shop_repair_container = 0;
        state.shop_repair_slot = Some(source_slot);
    } else {
        return false;
    }

    if state.npc_service_hold == Some(shop.service_mode) {
        let _ = submit_selection(shop, inventory, state, intents, pending);
    }
    true
}

/// Shared by Confirm and Hold selection events; never called by rendering.
fn selection_intent(
    shop: &ShopModel,
    inventory: &InventoryModel,
    state: &NativePlayerUiState,
) -> Option<NativePlayerUiIntent> {
    if !state.npc_shop_open() {
        return None;
    }
    let item = selected_item(shop, inventory, state)?;
    let unique_id = item_unique_id(item)?;
    if shop.allows_special_repair()
        && repair_selection_enabled(state, inventory)
        && repair_quote(shop, item).is_some()
    {
        Some(NativePlayerUiIntent::SRepairItem { unique_id })
    } else if shop.allows_repair()
        && repair_selection_enabled(state, inventory)
        && repair_quote(shop, item).is_some()
    {
        Some(NativePlayerUiIntent::RepairItem { unique_id })
    } else if shop.allows_sell() && shop_sell_enabled(inventory, shop.selected_bag_slot_for_sell) {
        let count = state
            .shop_service_drag_count
            .unwrap_or_else(|| shop_quantity_clamped(state.shop_quantity))
            .min(item.quantity.min(u32::from(u16::MAX)) as u16);
        Some(NativePlayerUiIntent::SellItem {
            unique_id,
            count,
        })
    } else {
        None
    }
}

fn repair_quote(
    shop: &ShopModel,
    item: &ItemModel,
) -> Option<crate::crystal_ui::npc_item_quote::NpcRepairQuote> {
    let rate = shop.repair_rate?;
    crate::crystal_ui::npc_item_quote::crystal_npc_repair_quote(
        item,
        rate,
        shop.allows_special_repair(),
    )
}

pub(super) fn submit_selection(
    shop: &mut ShopModel,
    inventory: &InventoryModel,
    state: &mut NativePlayerUiState,
    intents: &mut NativePlayerUiIntentQueue,
    pending: &mut PendingOperations,
) -> bool {
    if !dragged_service_selection_is_current(shop, inventory, state) {
        clear_dragged_service_selection(shop, state);
        return false;
    }
    let Some(intent) = selection_intent(shop, inventory, state) else {
        return false;
    };
    if matches!(
        intent,
        NativePlayerUiIntent::RepairItem { .. } | NativePlayerUiIntent::SRepairItem { .. }
    ) && !repair_is_affordable(shop, inventory, state)
    {
        // Crystal leaves TargetItem selected and emits LowGold through System
        // chat. Keep the native selection/pending queue in the same state.
        state.npc_service_notice = Some(LOW_GOLD_NOTICE.to_owned());
        state.npc_service_notice_mode = Some(shop.service_mode);
        return false;
    }
    if !intents.push_pending_intent(pending, intent) {
        return false;
    }
    // Crystal clears TargetItem immediately after enqueue. Retain the target
    // when local pending/queue backpressure rejects the operation.
    shop.selected_bag_slot_for_sell = None;
    shop.selected_bag_slot_for_repair = None;
    state.shop_service_drag_count = None;
    state.shop_service_drag_unique_id = None;
    state.shop_repair_slot = None;
    state.shop_repair_container = 0;
    state.npc_service_notice = None;
    state.npc_service_notice_mode = None;
    true
}

/// `NPCDropDialog.Confirm` compares `GameScene.Gold` to the untruncated
/// float (`RepairPrice * multiplier * NPCRate`). The server then truncates the
/// same value for its deduction, so quote `total_price` must not drive this
/// client-side gate.
fn repair_is_affordable(
    shop: &ShopModel,
    inventory: &InventoryModel,
    state: &NativePlayerUiState,
) -> bool {
    selected_item(shop, inventory, state)
        .and_then(|item| repair_quote(shop, item))
        .is_some_and(|quote| inventory.gold as f32 >= quote.displayed_total)
}

/// Deliver exactly one pending local NPC service rejection through Crystal's
/// System-chat route. Retaining it until `ChatModel` exists makes lightweight
/// UI tests and startup ordering safe without rendering side effects.
pub(super) fn drain_service_notice(
    mut state: ResMut<NativePlayerUiState>,
    mut chat: Option<ResMut<crate::chat::ChatModel>>,
) {
    let Some(chat) = chat.as_deref_mut() else {
        return;
    };
    if let Some(text) = state.npc_service_notice.take() {
        chat.push(crate::chat::ChatLine {
            text,
            channel: "system".to_owned(),
        });
    }
}

fn selected_item<'a>(
    shop: &ShopModel,
    inventory: &'a InventoryModel,
    state: &NativePlayerUiState,
) -> Option<&'a ItemModel> {
    if shop.allows_repair() || shop.allows_special_repair() {
        selected_repair_item(state, inventory)
            .filter(|item| item.container == 0 || item.container == 2)
            .filter(|item| {
                state
                    .shop_service_drag_unique_id
                    .is_none_or(|unique_id| item_unique_id(item) == Some(unique_id))
            })
    } else if shop.allows_sell() {
        inventory
            .items
            .iter()
            .find(|item| item.container == 0 && Some(item.slot) == shop.selected_bag_slot_for_sell)
            .filter(|item| {
                state
                    .shop_service_drag_unique_id
                    .is_none_or(|unique_id| item_unique_id(item) == Some(unique_id))
            })
    } else {
        None
    }
}

fn dragged_service_selection_is_current(
    shop: &ShopModel,
    inventory: &InventoryModel,
    state: &NativePlayerUiState,
) -> bool {
    let Some(unique_id) = state.shop_service_drag_unique_id else {
        return true;
    };
    let (container, slot) = if shop.allows_repair() || shop.allows_special_repair() {
        (state.shop_repair_container, state.shop_repair_slot)
    } else if shop.allows_sell() {
        (0, shop.selected_bag_slot_for_sell)
    } else {
        return false;
    };
    inventory.items.iter().any(|item| {
        item.container == container
            && Some(item.slot) == slot
            && item_unique_id(item) == Some(unique_id)
    })
}

fn clear_dragged_service_selection(shop: &mut ShopModel, state: &mut NativePlayerUiState) {
    shop.selected_bag_slot_for_sell = None;
    shop.selected_bag_slot_for_repair = None;
    state.shop_service_drag_count = None;
    state.shop_service_drag_unique_id = None;
    state.shop_repair_container = 0;
    state.shop_repair_slot = None;
}

pub(super) fn render(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&AssetServer>,
    shop: &ShopModel,
    inventory: &InventoryModel,
    state: &NativePlayerUiState,
    player: &crate::read_model::PlayerStats,
) {
    let service = service_action(shop);
    let repair = shop.allows_repair() || shop.allows_special_repair();
    let selected = selected_item(shop, inventory, state);
    let repair_quote = repair.then(|| selected.and_then(|item| repair_quote(shop, item))).flatten();
    let enabled = service.is_some()
        && selected.is_some_and(|item| item_unique_id(item).is_some())
        && if repair {
            repair_selection_enabled(state, inventory) && repair_quote.is_some()
        } else {
            shop_sell_enabled(inventory, shop.selected_bag_slot_for_sell)
        };
    if let Some(assets) = assets {
        // BeforeDraw changes the constructor's Prguse/392 to Prguse2/351.
        spawn_overlay_frame(parent, assets, "original-ui/Prguse2/351.png", 176.0, 147.0);
        if let Some((_, action)) = service {
            spawn_overlay_crystal_button_enabled(
                parent, assets, "Title", 290, 291, 292, CONFIRM, action, enabled,
            );
        }
        let held = state.npc_service_hold == Some(shop.service_mode);
        spawn_overlay_crystal_button_enabled(
            parent,
            assets,
            "Title",
            if held { 295 } else { 293 },
            294,
            295,
            CrystalRect::new(114.0, 36.0, 48.0, 25.0),
            OverlayButton::ShopToggleHold,
            service.is_some(),
        );
    } else if let Some((title, action)) = service {
        overlay_absolute_button(parent, title, CONFIRM, action, enabled);
        overlay_absolute_button(
            parent,
            "Hold",
            CrystalRect::new(114.0, 36.0, 48.0, 25.0),
            OverlayButton::ShopToggleHold,
            true,
        );
    }
    let title = service.map_or("Unavailable", |(title, _)| title);
    let info = if repair {
        repair_quote.map_or_else(
            || title.to_owned(),
            |quote| format!("{title}: {} gold", quote.displayed_total),
        )
    } else if shop.allows_sell() {
        selected
            .and_then(|item| {
                item.sell_value.checked_mul(
                    item.quantity
                        .min(u32::from(
                            state
                                .shop_service_drag_count
                                .unwrap_or_else(|| shop_quantity_clamped(state.shop_quantity)),
                        )),
                )
            })
            .map_or_else(
                || title.to_owned(),
                |price| format!("{title}: {price} gold"),
            )
    } else {
        title.to_owned()
    };
    overlay_text_at(
        parent,
        &info,
        CrystalRect::new(30.0, 10.0, 140.0, 20.0),
        10.0,
        TEXT,
    );
    if let Some(item) = selected {
        if let Some(assets) = assets {
            parent
                .spawn(Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(ITEM.left),
                    top: Val::Px(ITEM.top),
                    width: Val::Px(ITEM.width),
                    height: Val::Px(ITEM.height),
                    ..default()
                })
                .with_children(|cell| {
                    cell.spawn(original_item_image_bundle(
                        assets,
                        item.user_item_image_index(),
                        ITEM.width as i32,
                        ITEM.height as i32,
                    ));
                });
        } else {
            overlay_text_at(parent, &item.name, ITEM, 10.0, TEXT);
        }
    }

    // NPCDropDialog.Show keeps Crystal's ordinary InventoryDialog visible,
    // so bag selection now comes from that real panel rather than this former
    // adjacent picker. Equipment remains an explicit adapter until its drag
    // source is wired into the service target.
    if repair {
        parent.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(180.0),
                top: Val::Px(0.0),
                width: Val::Px(296.0),
                height: Val::Px(326.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.03, 0.035, 0.96)),
        ));
        overlay_text_at(
            parent,
            "Equipment",
            CrystalRect::new(184.0, 235.0, 160.0, 18.0),
            11.0,
            GOLD,
        );
        for item in inventory
            .items_in(2)
            .into_iter()
            .filter(|item| item.slot < 14)
        {
            picker_item(
                parent,
                assets,
                item,
                CrystalRect::new(
                    184.0 + (item.slot % 8) as f32 * 36.0,
                    255.0 + (item.slot / 8) as f32 * 34.0,
                    36.0,
                    32.0,
                ),
                OverlayButton::SelectEquipForRepair(item.slot),
                true,
                selected,
                player,
            );
        }
    } else if shop.allows_sell() {
        let selected_count = state
            .shop_service_drag_count
            .unwrap_or_else(|| shop_quantity_clamped(state.shop_quantity));
        let drag_selected = state.shop_service_drag_count.is_some();
        overlay_absolute_button(
            parent,
            "−",
            CrystalRect::new(20.0, 153.0, 28.0, 22.0),
            OverlayButton::ShopQuantityDec,
            !drag_selected && state.shop_quantity > SHOP_QUANTITY_MIN,
        );
        overlay_text_at(
            parent,
            &format!("x{selected_count}"),
            CrystalRect::new(50.0, 155.0, 50.0, 18.0),
            11.0,
            TEXT,
        );
        overlay_absolute_button(
            parent,
            "+",
            CrystalRect::new(105.0, 153.0, 28.0, 22.0),
            OverlayButton::ShopQuantityInc,
            !drag_selected && state.shop_quantity < SHOP_QUANTITY_MAX,
        );
    }
    if shop.allows_buy() && shop.allows_sell() {
        overlay_absolute_button(
            parent,
            "Buy",
            CrystalRect::new(20.0, 182.0, 60.0, 22.0),
            OverlayButton::ShopShowBuy,
            true,
        );
    }
    overlay_absolute_button(
        parent,
        "Close",
        CrystalRect::new(90.0, 182.0, 60.0, 22.0),
        OverlayButton::ShopCancel,
        true,
    );
    if let Some(rate) = shop.repair_rate.filter(|_| repair) {
        overlay_text_at(
            parent,
            &format!("Repair rate x{rate:.2}"),
            CrystalRect::new(20.0, 212.0, 150.0, 18.0),
            10.0,
            TEXT,
        );
    }
    if repair && repair_quote.is_none() {
        overlay_text_at(
            parent,
            "Quote unavailable",
            CrystalRect::new(20.0, 235.0, 150.0, 18.0),
            10.0,
            TEXT,
        );
    }
}

fn picker_item(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&AssetServer>,
    item: &ItemModel,
    rect: CrystalRect,
    action: OverlayButton,
    enabled: bool,
    selected: Option<&ItemModel>,
    player: &crate::read_model::PlayerStats,
) {
    let is_selected = selected
        .is_some_and(|selected| selected.container == item.container && selected.slot == item.slot);
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            ..default()
        },
        BackgroundColor(if is_selected {
            Color::srgba(0.2, 0.6, 0.2, 0.45)
        } else {
            Color::srgba(0.0, 0.0, 0.0, 0.55)
        }),
    ));
    if let Some(assets) = assets {
        overlay_absolute_item_button(parent, assets, item, rect, action, enabled, player);
    } else {
        overlay_absolute_button(
            parent,
            &short_name(&item.name, &item.key),
            rect,
            action,
            enabled,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{
        CrystalItemInfoModel, CrystalItemStatModel, CrystalItemTooltipSourceModel,
        CrystalUserItemModel,
    };

    fn repairable_item(unique_id: u64, container: u8, slot: u32) -> ItemModel {
        ItemModel {
            unique_id: Some(unique_id),
            container,
            slot,
            quantity: 1,
            durability_current: Some(500),
            durability_max: Some(1000),
            tooltip_source: Some(CrystalItemTooltipSourceModel {
                info: CrystalItemInfoModel {
                    item_index: 77,
                    price: 1000,
                    durability: 1000,
                    ..Default::default()
                },
                user_item: Some(CrystalUserItemModel {
                    unique_id,
                    item_index: 77,
                    current_dura: 500,
                    max_dura: 1000,
                    count: 1,
                    added_stats: vec![CrystalItemStatModel { stat: 5, value: 5 }],
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn hold_selection_emits_once_and_idle_updates_do_not_resubmit() {
        let mut app = App::new();
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<MailComposeUi>()
            .init_resource::<NativePlayerUiIntentQueue>()
            .init_resource::<PendingOperations>()
            .init_resource::<NativeUiIntentQueue>()
            .init_resource::<crate::chat::ChatModel>()
            .init_resource::<InventoryModel>()
            .init_resource::<MailModel>()
            .init_resource::<MapModel>()
            .init_resource::<ShopModel>()
            .init_resource::<StorageModel>()
            .init_resource::<crate::social::SocialModel>()
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..default()
            });
        super::super::tests::init_overlay_button_test_resources(&mut app);
        app.add_systems(
            Update,
            (process_overlay_buttons, drain_service_notice).chain(),
        );
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .toggle_npc_shop();
        app.world_mut()
            .resource_mut::<ShopModel>()
            .apply_service_signal(NpcShopServiceSignal {
                mode: NpcShopServiceMode::SpecialRepair,
                repair_rate: Some(2.0),
            });
        {
            let mut inventory = app.world_mut().resource_mut::<InventoryModel>();
            // Crystal's special quote is 188 * 3 * 2 for this fixture.
            inventory.gold = 1128;
            inventory.items.push(repairable_item(42, 2, 13));
        }
        fn press(app: &mut App, action: OverlayButton) {
            let button = app
                .world_mut()
                .spawn((Button, action, Interaction::Pressed))
                .id();
            app.update();
            app.world_mut().despawn(button);
        }
        press(&mut app, OverlayButton::ShopToggleHold);
        assert!(app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents()
            .is_empty());
        press(&mut app, OverlayButton::SelectEquipForRepair(13));
        let intents = app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents();
        assert_eq!(intents.len(), 1);
        assert!(matches!(
            intents[0],
            NativePlayerUiIntent::SRepairItem { unique_id: 42 }
        ));
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .shop_repair_slot,
            None
        );
        app.update();
        app.update();
        assert!(app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents()
            .is_empty());
        press(&mut app, OverlayButton::ShopCancel);
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .npc_service_hold,
            None
        );
    }

    #[test]
    fn enqueue_clears_target_only_on_success_and_sale_count_never_exceeds_stack() {
        let inventory = InventoryModel {
            items: vec![ItemModel {
                unique_id: Some(77),
                quantity: 3,
                slot: 45,
                container: 0,
                ..default()
            }],
            ..default()
        };
        let mut state = NativePlayerUiState::default();
        state.toggle_npc_shop();
        state.shop_quantity = 99;
        let mut shop = ShopModel {
            service_mode: NpcShopServiceMode::Sell,
            selected_bag_slot_for_sell: Some(45),
            ..default()
        };
        let mut intents = NativePlayerUiIntentQueue::default();
        let mut pending = PendingOperations::default();
        assert!(submit_selection(
            &mut shop,
            &inventory,
            &mut state,
            &mut intents,
            &mut pending
        ));
        assert!(matches!(
            intents.drain_intents()[0],
            NativePlayerUiIntent::SellItem {
                unique_id: 77,
                count: 3
            }
        ));
        assert_eq!(shop.selected_bag_slot_for_sell, None);
        shop.selected_bag_slot_for_sell = Some(45);
        assert!(!submit_selection(
            &mut shop,
            &inventory,
            &mut state,
            &mut intents,
            &mut pending
        ));
        assert_eq!(shop.selected_bag_slot_for_sell, Some(45));
        assert!(intents.drain_intents().is_empty());
    }

    #[test]
    fn repair_requires_the_untruncated_crystal_gold_threshold_and_keeps_target() {
        for (mode, exact_gold) in [
            (NpcShopServiceMode::Repair, 188_u32),
            (NpcShopServiceMode::SpecialRepair, 564_u32),
        ] {
            let mut inventory = InventoryModel {
                gold: exact_gold - 1,
                items: vec![repairable_item(42, 0, 4)],
                ..default()
            };
            let mut state = NativePlayerUiState::default();
            state.toggle_npc_shop();
            state.shop_repair_container = 0;
            state.shop_repair_slot = Some(4);
            let mut shop = ShopModel {
                service_mode: mode,
                repair_rate: Some(1.0),
                selected_bag_slot_for_repair: Some(4),
                ..default()
            };
            let mut intents = NativePlayerUiIntentQueue::default();
            let mut pending = PendingOperations::default();

            assert!(!submit_selection(
                &mut shop,
                &inventory,
                &mut state,
                &mut intents,
                &mut pending,
            ));
            assert_eq!(state.npc_service_notice.as_deref(), Some(LOW_GOLD_NOTICE));
            assert_eq!(state.shop_repair_slot, Some(4));
            assert_eq!(shop.selected_bag_slot_for_repair, Some(4));
            assert!(intents.drain_intents().is_empty());
            assert!(pending.is_empty());

            inventory.gold = exact_gold;
            assert!(submit_selection(
                &mut shop,
                &inventory,
                &mut state,
                &mut intents,
                &mut pending,
            ));
            assert!(matches!(
                intents.drain_intents().as_slice(),
                [NativePlayerUiIntent::RepairItem { unique_id: 42 }]
                    | [NativePlayerUiIntent::SRepairItem { unique_id: 42 }]
            ));
        }
    }

    #[test]
    fn fractional_quote_rejects_truncated_server_cost_and_accepts_ceil_gold() {
        let mut inventory = InventoryModel {
            gold: 18,
            items: vec![repairable_item(42, 0, 4)],
            ..default()
        };
        let mut state = NativePlayerUiState::default();
        state.toggle_npc_shop();
        state.shop_repair_container = 0;
        state.shop_repair_slot = Some(4);
        let mut shop = ShopModel {
            service_mode: NpcShopServiceMode::Repair,
            repair_rate: Some(0.1),
            selected_bag_slot_for_repair: Some(4),
            ..default()
        };
        let mut intents = NativePlayerUiIntentQueue::default();
        let mut pending = PendingOperations::default();

        assert!(!submit_selection(
            &mut shop,
            &inventory,
            &mut state,
            &mut intents,
            &mut pending,
        ));
        assert_eq!(state.npc_service_notice.as_deref(), Some(LOW_GOLD_NOTICE));
        assert!(pending.is_empty());
        inventory.gold = 19;
        assert!(submit_selection(
            &mut shop,
            &inventory,
            &mut state,
            &mut intents,
            &mut pending,
        ));
    }

    #[test]
    fn hold_low_gold_reports_once_without_pending_or_losing_selection() {
        let mut app = App::new();
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<MailComposeUi>()
            .init_resource::<NativePlayerUiIntentQueue>()
            .init_resource::<PendingOperations>()
            .init_resource::<NativeUiIntentQueue>()
            .init_resource::<crate::chat::ChatModel>()
            .init_resource::<InventoryModel>()
            .init_resource::<MailModel>()
            .init_resource::<MapModel>()
            .init_resource::<ShopModel>()
            .init_resource::<StorageModel>()
            .init_resource::<crate::social::SocialModel>()
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..default()
            });
        super::super::tests::init_overlay_button_test_resources(&mut app);
        app.add_systems(
            Update,
            (process_overlay_buttons, drain_service_notice).chain(),
        );
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .toggle_npc_shop();
        app.world_mut()
            .resource_mut::<ShopModel>()
            .apply_service_signal(NpcShopServiceSignal {
                mode: NpcShopServiceMode::SpecialRepair,
                repair_rate: Some(1.0),
            });
        app.world_mut()
            .resource_mut::<InventoryModel>()
            .items
            .push(repairable_item(42, 2, 13));
        fn press(app: &mut App, action: OverlayButton) {
            let button = app
                .world_mut()
                .spawn((Button, action, Interaction::Pressed))
                .id();
            app.update();
            app.world_mut().despawn(button);
        }

        press(&mut app, OverlayButton::ShopToggleHold);
        press(&mut app, OverlayButton::SelectEquipForRepair(13));
        assert!(app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents()
            .is_empty());
        assert!(app.world().resource::<PendingOperations>().is_empty());
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .shop_repair_slot,
            Some(13)
        );
        assert_eq!(
            app.world().resource::<crate::chat::ChatModel>().lines,
            vec![crate::chat::ChatLine {
                text: LOW_GOLD_NOTICE.to_owned(),
                channel: "system".to_owned(),
            }]
        );
        app.update();
        assert_eq!(app.world().resource::<crate::chat::ChatModel>().lines.len(), 1);
    }

    #[test]
    fn rendered_service_exposes_last_bag_and_equipment_slots_and_original_confirm() {
        let mut app = super::super::tests::overlay_render_test_app();
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .toggle_npc_shop();
        *app.world_mut().resource_mut::<ShopModel>() = ShopModel {
            service_mode: NpcShopServiceMode::Repair,
            repair_rate: Some(1.0),
            ..Default::default()
        };
        app.world_mut().resource_mut::<InventoryModel>().items = vec![
            repairable_item(10, 0, 45),
            repairable_item(20, 2, 13),
        ];
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .shop_repair_slot = Some(45);
        app.update();
        let (controls, labels) = {
            let world = app.world_mut();
            let controls: Vec<_> = world
                .query::<(&OverlayButton, &Node)>()
                .iter(world)
                .map(|(action, node)| (*action, node.clone()))
                .collect();
            let labels: Vec<_> = world
                .query::<&Text>()
                .iter(world)
                .map(|text| text.0.clone())
                .collect();
            (controls, labels)
        };
        assert!(!controls
            .iter()
            .any(|(action, _)| *action == OverlayButton::SelectBagForRepair(45)));
        assert!(controls
            .iter()
            .any(|(action, _)| *action == OverlayButton::SelectEquipForRepair(13)));
        let (_, confirm) = controls
            .iter()
            .find(|(action, _)| *action == OverlayButton::ShopRepair)
            .unwrap();
        assert_eq!(
            (confirm.left, confirm.top, confirm.width, confirm.height),
            (Val::Px(114.0), Val::Px(62.0), Val::Px(48.0), Val::Px(25.0))
        );
        assert!(labels.iter().any(|text| text == "Repair: 188 gold"));

        app.world_mut().resource_mut::<InventoryModel>().items[0].tooltip_source = None;
        {
            let world = app.world();
            let shop = world.resource::<ShopModel>();
            let inventory = world.resource::<InventoryModel>();
            let state = world.resource::<NativePlayerUiState>();
            let selected = selected_item(shop, inventory, state).unwrap();
            assert!(repair_quote(shop, selected).is_none());
            assert!(selection_intent(shop, inventory, state).is_none());
        }
        app.update();
        let world = app.world_mut();
        let labels: Vec<_> = world.query::<&Text>().iter(world).map(|text| text.0.clone()).collect();
        assert!(labels.iter().any(|text| text == "Quote unavailable"));
    }

    #[test]
    fn service_actions_are_authoritative_and_never_default_to_sell() {
        for (mode, expected) in [
            (NpcShopServiceMode::Closed, None),
            (NpcShopServiceMode::Buy, None),
            (NpcShopServiceMode::Sell, Some(OverlayButton::ShopSell)),
            (NpcShopServiceMode::Repair, Some(OverlayButton::ShopRepair)),
            (
                NpcShopServiceMode::SpecialRepair,
                Some(OverlayButton::ShopSRepair),
            ),
        ] {
            let shop = ShopModel {
                service_mode: mode,
                ..default()
            };
            assert_eq!(service_action(&shop).map(|(_, action)| action), expected);
        }
    }

    #[test]
    fn repair_selection_keeps_equipment_identity_and_sale_uses_only_bag() {
        let inventory = InventoryModel {
            items: vec![
                ItemModel {
                    unique_id: Some(10),
                    container: 0,
                    slot: 45,
                    ..default()
                },
                ItemModel {
                    unique_id: Some(20),
                    container: 2,
                    slot: 13,
                    ..default()
                },
            ],
            ..default()
        };
        let mut state = NativePlayerUiState::default();
        state.shop_repair_container = 2;
        state.shop_repair_slot = Some(13);
        let mut shop = ShopModel {
            service_mode: NpcShopServiceMode::SpecialRepair,
            selected_bag_slot_for_sell: Some(45),
            ..default()
        };
        assert_eq!(
            selected_item(&shop, &inventory, &state).unwrap().unique_id,
            Some(20)
        );
        shop.service_mode = NpcShopServiceMode::Sell;
        assert_eq!(
            selected_item(&shop, &inventory, &state).unwrap().unique_id,
            Some(10)
        );
        shop.selected_bag_slot_for_sell = Some(13);
        assert!(selected_item(&shop, &inventory, &state).is_none());
        shop.service_mode = NpcShopServiceMode::Closed;
        assert!(selected_item(&shop, &inventory, &state).is_none());
    }
}
