//! Actual native queue/ingest with the complete same-turn server catalog.
//! No socket, fake purchase acknowledgement, or actor permission is created.
use super::*;
use mir2_client_bevy::inventory::{CrystalUserItemModel, InventoryModel};
use mir2_client_bevy::npc_gold_buy_attempt::{
    npc_gold_buy_model_authority, NpcGoldBuyConnectionEpoch, NpcGoldBuyGate,
};
use mir2_client_bevy::npc_shop_ui::{NpcShopUiAction, NpcShopUiContext, NpcShopUiState};
use mir2_client_bevy::shop::{NpcShopServiceMode, NpcShopServiceSignal, ShopModel};

fn complete_buysell_catalog() -> ShopModel {
    let raw = CrystalUserItemModel {
        unique_id: 71,
        item_index: 658,
        count: 1,
        is_shop_item: true,
        ..Default::default()
    };
    let owner = serde_json::json!({
        "playerObjectId":7,
        "entities":[{"objectId":7,"kind":"selfPlayer","level":22,"class":"Warrior"}],
        "nativeNpcShop":{
            "npcObjectId":4990,"scriptKey":"BichonProvince/NaturalCave/WickedTrader",
            "service":"BUYSELL","packetType":"NPCGoods","list":[raw],
            "rate":1.0,"panelType":0,"hideAddedStats":false
        }
    });
    crate::npc_purchase_economy::owner_shop_catalog(&owner).unwrap()
}

#[test]
fn native_npc_buysell_packet_pair_and_complete_owner_ingest_produce_usable_same_source() {
    let _queue = native_ingest::native_queue_test_guard();
    let mut app = ingest_app();
    let source = complete_buysell_catalog();
    let inventory = InventoryModel {
        gold: 500_000,
        ..Default::default()
    };
    assert!(native_ingest::push_native_inventory_model(
        serde_json::to_string(&inventory).unwrap()
    ));
    assert!(native_ingest::push_native_shop_model(
        serde_json::to_string(&source).unwrap()
    ));
    for mode in [NpcShopServiceMode::Buy, NpcShopServiceMode::Sell] {
        assert!(native_ingest::push_native_npc_shop_service(
            serde_json::to_string(&NpcShopServiceSignal {
                mode,
                repair_rate: None
            })
            .unwrap()
        ));
    }
    app.update();
    // The complete owner arrives after NPCGoods/NPCSell, as real periodic snapshots do.
    assert!(native_ingest::push_native_shop_model(
        serde_json::to_string(&source).unwrap()
    ));
    app.update();
    let actual = app.world().resource::<ShopModel>();
    assert_eq!(source.service_mode, NpcShopServiceMode::Buy);
    assert_eq!(actual.service_mode, NpcShopServiceMode::Sell);
    assert!(actual.allows_buy() && actual.allows_sell());
    let actual_inventory = app.world().resource::<InventoryModel>();
    let source_key = npc_gold_buy_model_authority(&source, &inventory).unwrap();
    assert_eq!(
        npc_gold_buy_model_authority(actual, actual_inventory),
        Some(source_key.clone())
    );

    let gate = NpcGoldBuyGate::default();
    assert!(gate.observe_connection(NpcGoldBuyConnectionEpoch {
        run: 1,
        connection: 1
    }));
    assert!(gate.observe("same-complete-owner", 1, &source_key, true));
    let context = NpcShopUiContext {
        source_revision: gate.ui_authority_revision(),
        open: true,
        input_enabled: true,
        show_buy: true,
    };
    let mut presentation = NpcShopUiState::default();
    presentation.reconcile(context, actual, actual_inventory);
    assert!(presentation
        .view(context, actual, actual_inventory, gate.feedback())
        .stamp
        .is_some());
    presentation.apply(
        NpcShopUiAction::Select(71),
        context,
        actual,
        actual_inventory,
        gate.feedback(),
    );
    let view = presentation.view(context, actual, actual_inventory, gate.feedback());
    assert!(
        view.can_select && view.can_buy,
        "the valid received pharmacy must be usable, not just visible"
    );
    assert_eq!(view.plan.command.as_ref().unwrap().item_index, 71);
    let mut selected = actual.clone();
    selected.selected_id = presentation.selected_id;
    assert!(gate.reserve(&selected, actual_inventory, 1).is_some());
}
