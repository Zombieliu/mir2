//! Production provenance activation against the actual transport source fence.
//! The existing test-only world-owner capability seeds a trusted connected
//! owner; this is not socket authentication or a purchase acknowledgement test.
use super::*;
use bevy::prelude::*;
use mir2_client_bevy::crystal_ui::overlays::{
    NativePlayerUiIntent, NativePlayerUiIntentQueue, NativePlayerUiState,
};
use mir2_client_bevy::inventory::InventoryModel;
use mir2_client_bevy::read_model::UiReadModel;
use mir2_client_bevy::shop::{NpcShopServiceMode, NpcShopServiceSignal, ShopModel};

fn activation_app(change: usize) -> (App, NativeCommandFence) {
    let (sender, _receiver) = command_channel(8);
    let fence = sender.ownership_fence().unwrap();
    let stamp = fence.test_world_ready(7, 0);
    let commands = crate::input::GatewayCommands::new(sender);
    assert!(commands.activate_world_stamp(stamp));
    let raw = mir2_client_bevy::inventory::CrystalUserItemModel {
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
    let source = mir2_bevy_runtime::npc_purchase_economy::owner_shop_catalog(&owner).unwrap();
    let inventory = InventoryModel {
        gold: 500_000,
        ..Default::default()
    };
    let staged = fence.stage_npc_gold_buy_inventory(&serde_json::to_value(&inventory).unwrap());
    fence.finish_npc_gold_buy_inventory(staged, true);
    let staged = fence.stage_npc_gold_buy_complete_catalog(&serde_json::to_value(&source).unwrap());
    fence.finish_npc_gold_buy_catalog(staged, true);
    assert!(fence.npc_gold_buy_source().is_some());

    let mut actual = source;
    actual.apply_service_signal(NpcShopServiceSignal {
        mode: NpcShopServiceMode::Sell,
        repair_rate: None,
    });
    assert_eq!(actual.service_mode, NpcShopServiceMode::Sell);
    match change {
        1 => actual.supports_buy = false,
        2 => actual.goods[0].price += 1,
        3 => actual.goods[0].unique_id += 1,
        _ => {}
    }
    let mut ui = NativePlayerUiState::default();
    ui.toggle_npc_shop();
    ui.npc_shop_buy_tab = true;
    let mut read = UiReadModel::default();
    read.player.hp = 10;
    read.player.max_hp = 10;
    let mut app = App::new();
    app.insert_resource(commands)
        .insert_resource(ui)
        .insert_resource(read)
        .insert_resource(actual)
        .insert_resource(inventory)
        .init_resource::<NativePlayerUiIntentQueue>()
        .init_resource::<crate::gameplay_bridge::NativeWorldProducerStamp>()
        .add_systems(
            Update,
            crate::gameplay_bridge::activate_native_command_provenance,
        );
    (app, fence)
}

#[test]
fn native_npc_buysell_real_provenance_activation_allows_one_strict_purchase_intent() {
    let (mut app, _) = activation_app(0);
    app.update();
    let gate = app
        .world()
        .resource::<NativePlayerUiIntentQueue>()
        .npc_gold_buy_gate();
    assert!(gate.ui_authority_revision().is_some());
    assert!(gate.feedback().can_reserve);
    let mut shop = app.world().resource::<ShopModel>().clone();
    shop.selected_id = Some(71);
    let inventory = app.world().resource::<InventoryModel>().clone();
    let mut queue = app.world_mut().resource_mut::<NativePlayerUiIntentQueue>();
    assert!(queue.push_npc_gold_buy(&shop, &inventory, 1));
    assert!(!queue.push_npc_gold_buy(&shop, &inventory, 1));
    let intents = queue.drain_intents();
    assert_eq!(intents.len(), 1);
    assert!(matches!(
        &intents[0],
        NativePlayerUiIntent::NpcGoldBuy {
            item_index: 71,
            count: 1,
            ..
        }
    ));
}

#[test]
fn native_npc_buysell_real_provenance_rejects_changed_caps_price_uid_and_owner() {
    for change in 1..=3 {
        let (mut app, _) = activation_app(change);
        app.update();
        let gate = app
            .world()
            .resource::<NativePlayerUiIntentQueue>()
            .npc_gold_buy_gate();
        assert!(
            gate.ui_authority_revision().is_none(),
            "changed source {change}"
        );
        assert!(!gate.feedback().can_reserve);
    }
    let (mut app, fence) = activation_app(0);
    app.update();
    let gate = app
        .world()
        .resource::<NativePlayerUiIntentQueue>()
        .npc_gold_buy_gate();
    assert!(gate.feedback().can_reserve);
    fence.test_owner_change(8, 0);
    app.update();
    assert!(!gate.feedback().can_reserve);
    let mut shop = app.world().resource::<ShopModel>().clone();
    shop.selected_id = Some(71);
    let inventory = app.world().resource::<InventoryModel>().clone();
    assert!(!app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .push_npc_gold_buy(&shop, &inventory, 1));
}
