//! Pure fixture provenance, followed by actual packet/shared-custody handlers.
//! The helper below does not perform a normal PostgreSQL conversion, debit a
//! fee, mint permission, or certify a shared Zone/multiplayer deployment.
use crate::config::{GroundDropLootSnapshot, ItemContainer, SimulationConfig, VisibleNpcRecord};
use crate::runtime::items::hunting_item_binding::{
    belongs_to_current_owner, has_bound, has_economic_provenance, HuntingItemBinding,
};
use crate::runtime::items::hunting_item_conversion::{convert_bound_root, ConversionIdentity};
use crate::runtime::items::{
    crystal_item_template_for_item_key, embedded_item_state_from_template,
    try_item_state_from_user_item, try_user_item_from_item_state, ItemState,
};
use crate::runtime::resources::{
    HeroInventoryResource, InventoryResource, NpcStateResource, PlayerRuntimeResource,
    RuntimeConfigResource, SessionResource, Stage5SystemsResource,
};
use crate::runtime::session::SimulationSession;
use mir2_protocol::{ClientPacket, MirDirection, Point, ServerPacket};
use serde_json::{json, Value};

const REALM: &str = "converted-outflow-fixture";
const SELLER: u32 = 4_990;
const SELLER_POSITION: Point = Point { x: 350, y: 300 };

fn started(account_id: &str, name: &str, incarnation: &str) -> SimulationSession {
    let mut config = SimulationConfig::default();
    // This distant fixture NPC avoids the ordinary starter NPC cells. Its real
    // catalog script provides the ordinary @BuySell/SellItem implementation.
    config.visible_npcs.push(VisibleNpcRecord {
        object_id: SELLER,
        name: "Wicked Trader".into(),
        image: 5,
        colour_argb: -1,
        position: SELLER_POSITION,
        direction: MirDirection::Left,
        quest_ids: vec![],
        script_key: Some("BichonProvince/NaturalCave/WickedTrader".into()),
    });
    {
        let mut store = config.account_store.lock().unwrap();
        let mut record = store.accounts.get("demo").unwrap().clone();
        for character in &mut record.characters {
            if character.index == 0 {
                character.name = name.into();
            }
        }
        record.saves.get_mut(&0).unwrap().character.name = name.into();
        store.accounts.insert(account_id.into(), record);
    }
    let mut session = SimulationSession::new(config);
    let login = session.handle_packet(ClientPacket::Login {
        account_id: account_id.into(),
        password: "demo".into(),
    });
    assert!(
        login
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })),
        "{login:?}"
    );
    let start = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(
        start
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })),
        "{start:?}"
    );
    // Bind the synthetic test identity only after normal in-memory entry. A
    // configured PG source remains necessary for an actual economic conversion.
    session
        .app
        .world_mut()
        .resource_mut::<RuntimeConfigResource>()
        .config
        .hunting_economy_realm = Some(REALM.into());
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .hunting_character_incarnation = Some(incarnation.into());
    session.force_authoritative_player_transform(Point { x: 350, y: 301 }, MirDirection::Up);
    install(&mut session, Vec::new());
    session
}

fn sender() -> SimulationSession {
    started(
        "converted-sender",
        "Converted Sender",
        "pg-character-v1:11111111-1111-4111-8111-111111111111",
    )
}

fn receiver() -> SimulationSession {
    started(
        "converted-receiver",
        "Converted Receiver",
        "pg-character-v1:22222222-2222-4222-8222-222222222222",
    )
}

fn install(session: &mut SimulationSession, items: Vec<ItemState>) {
    let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
    inventory.inventory_items = items;
    inventory.belt_items.clear();
    inventory.storage_items.clear();
    inventory.equipment_items.clear();
    inventory.reserved_item_unique_ids.clear();
}

fn legacy_item(key: &str, unique_id: u64) -> ItemState {
    let mut item = embedded_item_state_from_template(
        &crystal_item_template_for_item_key(key).unwrap(),
        ItemContainer::Bag1,
        8,
    );
    item.unique_id = unique_id;
    item.quantity = 1;
    item
}

fn converted_fixture(session: &SimulationSession, mut item: ItemState) -> ItemState {
    item.economic_binding = Some(
        HuntingItemBinding::for_current_owner(
            session.app.world(),
            "quest-once-v1:2110001:fixture-reward",
        )
        .unwrap(),
    );
    let before = serde_json::to_value(&item).unwrap();
    let operation = "1".repeat(64);
    let quote = "2".repeat(64);
    let valuation_digest = "3".repeat(64);
    let lot = "4".repeat(64);
    let converted = convert_bound_root(
        session.app.world(),
        &item,
        ConversionIdentity {
            operation_id: &operation,
            quote_id: &quote,
            valuation_version: "fixture-material-valuation-v1",
            valuation_digest: &valuation_digest,
            lot_id: &lot,
        },
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&item).unwrap(),
        before,
        "fixture helper never changes its Bound input"
    );
    assert!(!has_bound(&converted));
    assert!(has_economic_provenance(&converted));
    assert!(converted.economic_conversion.is_some());
    converted
}

fn assert_full_item_except_location(actual: &ItemState, expected: &ItemState) {
    let mut relocated = expected.clone();
    relocated.container = actual.container;
    relocated.slot = actual.slot;
    assert_eq!(
        serde_json::to_value(actual).unwrap(),
        serde_json::to_value(relocated).unwrap(),
        "private provenance and full instance metadata survive the custody channel"
    );
}

fn carried(session: &SimulationSession, unique_id: u64) -> ItemState {
    let inventory = session.app.world().resource::<InventoryResource>();
    inventory
        .inventory_items
        .iter()
        .chain(&inventory.belt_items)
        .find(|item| item.unique_id == unique_id)
        .unwrap()
        .clone()
}

fn census(session: &SimulationSession) -> Value {
    let drops = session.world_snapshot().ground_drops;
    let world = session.app.world();
    let inventory = world.resource::<InventoryResource>();
    let hero = world.resource::<HeroInventoryResource>();
    let npc = world.resource::<NpcStateResource>();
    let config = &world.resource::<RuntimeConfigResource>().config;
    json!({
        "items": inventory.inventory_items, "belt": inventory.belt_items,
        "storage": inventory.storage_items, "equipment": inventory.equipment_items,
        "reserved": inventory.reserved_item_unique_ids, "capacity": inventory.inventory_capacity,
        "heroItems": hero.items, "heroEquipment": hero.equipment,
        "gold": world.resource::<PlayerRuntimeResource>().gold,
        "credit": world.resource::<PlayerRuntimeResource>().credit,
        "buyback": npc.npc_buy_back_items, "used": npc.npc_used_goods_items,
        "systems": world.resource::<Stage5SystemsResource>().stage5_systems,
        "store": &*config.account_store.lock().unwrap(),
        "drops": drops,
    })
}

#[test]
fn hunting_converted_shared_trade_offer_delivery_preserves_provenance_across_accounts() {
    let mut source = sender();
    let mut target = receiver();
    let item = converted_fixture(&source, legacy_item("dagger", 93_001));
    assert!(
        belongs_to_current_owner(target.app.world(), &item),
        "same realm Converted is not Bound to its original account"
    );
    install(&mut source, vec![item.clone()]);
    source.trade_request("Converted Receiver");
    target.trade_request("Converted Sender");
    let deposit = source.handle_packet(ClientPacket::DepositTradeItem { from: 8, to: 0 });
    assert!(
        deposit
            .iter()
            .any(|packet| matches!(packet, ServerPacket::DepositTradeItem { success: true, .. })),
        "{deposit:?}"
    );
    let (_, outgoing) = source.shared_trade_confirm();
    let outgoing = outgoing.expect("real confirmation produces the complete outgoing offer");
    let (_, empty_return) = target.shared_trade_confirm();
    let empty_return = empty_return.expect("real reciprocal empty offer");
    assert_eq!(outgoing.account_id, "converted-sender");
    assert_eq!(outgoing.items.len(), 1);
    let encoded: ItemState = serde_json::from_str(&outgoing.items[0].item_state_json).unwrap();
    assert_full_item_except_location(&encoded, &item);
    assert!(
        !source
            .app
            .world()
            .resource::<InventoryResource>()
            .inventory_items
            .iter()
            .any(|item| item.unique_id == 93_001),
        "confirmation moves the outgoing item into real trade escrow"
    );
    let delivered = target.apply_shared_trade_delivery(&outgoing);
    assert!(
        delivered
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedItem { .. })),
        "{delivered:?}"
    );
    let _ = source.apply_shared_trade_delivery(&empty_return);
    assert_full_item_except_location(&carried(&target, 93_001), &item);
    assert!(!has_bound(&carried(&target, 93_001)));
    assert!(!source.has_active_shared_trade_state() && !target.has_active_shared_trade_state());
    let after = census(&target);
    assert!(
        target.apply_shared_trade_delivery(&outgoing).is_empty(),
        "retired escrow cannot deliver the same offer twice"
    );
    assert_eq!(census(&target), after);
}

#[test]
fn hunting_converted_player_drop_shared_pickup_preserves_full_source_across_accounts() {
    let mut source = sender();
    let mut target = receiver();
    let item = converted_fixture(&source, legacy_item("dagger", 93_002));
    let source_gold = source.world_snapshot().gold;
    let target_gold = target.world_snapshot().gold;
    install(&mut source, vec![item.clone()]);
    let requested = source
        .shared_inventory_item_drop(93_002, 1, false)
        .expect("Converted is eligible for the actual player-drop transaction");
    let dropped = source.commit_shared_inventory_item_drop_transaction(&requested);
    assert!(dropped.committed, "{:?}", dropped.packets);
    assert!(dropped
        .packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::DropItem { success: true, .. })));
    assert!(source
        .app
        .world()
        .resource::<InventoryResource>()
        .inventory_items
        .is_empty());
    let drop = source.world_snapshot().ground_drops.into_iter().find(|drop| {
        matches!(&drop.loot, GroundDropLootSnapshot::InventoryItem { exact_item: Some(payload), .. }
            if payload.item.unique_id == 93_002)
    }).expect("actual drop includes its private full-instance custody");
    let GroundDropLootSnapshot::InventoryItem {
        key,
        exact_item: Some(payload),
        ..
    } = &drop.loot
    else {
        unreachable!()
    };
    assert!(
        payload.server_state.is_some(),
        "wire-only state cannot prove Converted provenance"
    );
    let retained =
        crate::runtime::drops::ground_drop_custody::decode(payload, key, drop.quantity).unwrap();
    assert_full_item_except_location(&retained, &item);
    // The shared Zone is responsible for consuming this drop once. This test
    // passes its real full-state handoff to a different personal owner, rather
    // than pretending that two independent personal Worlds form multiplayer.
    assert!(target.can_commit_shared_ground_drop_pickup(&drop));
    let picked_up = target.apply_shared_ground_drop_pickup_commit(&drop);
    assert!(picked_up.committed, "{:?}", picked_up.packets);
    assert!(picked_up
        .packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::GainedItem { .. })));
    assert_full_item_except_location(&carried(&target, 93_002), &item);
    assert!(belongs_to_current_owner(
        target.app.world(),
        &carried(&target, 93_002)
    ));
    assert_eq!(source.world_snapshot().gold, source_gold);
    assert_eq!(
        target.world_snapshot().gold,
        target_gold,
        "no legacy gold settlement in the full-state pickup path"
    );
}

fn sell_service(session: &mut SimulationSession) {
    session.interact(SELLER);
    session.select_npc_dialog_target("@BuySell");
    assert!(
        crate::runtime::npc::current_crystal_npc_service_in_range(session.app.world())
            .is_some_and(|service| crate::runtime::npc::active_crystal_sell_service(&service)),
        "real nonoverlapping fixture NPC must expose its catalog SELL service"
    );
}

#[test]
fn hunting_converted_npc_sell_rejects_root_and_hidden_child_without_any_custody_or_gold_change() {
    // First establish that this actual service accepts the unmarked control.
    let mut control = sender();
    sell_service(&mut control);
    install(&mut control, vec![legacy_item("dagger", 93_003)]);
    let sold = control.handle_packet(ClientPacket::SellItem {
        unique_id: 93_003,
        count: 1,
    });
    assert!(
        sold.iter()
            .any(|packet| matches!(packet, ServerPacket::SellItem { success: true, .. })),
        "{sold:?}"
    );

    for nested in [false, true] {
        let mut session = sender();
        sell_service(&mut session);
        let mut item = legacy_item("dagger", 93_004);
        if nested {
            let mut child = embedded_item_state_from_template(
                &mir2_game_data::crystal_item_by_name("BronzeBell").unwrap(),
                ItemContainer::Bag1,
                0,
            );
            child.unique_id = 93_005;
            let mut wire = try_user_item_from_item_state(&item).unwrap();
            wire.slots = vec![None, Some(try_user_item_from_item_state(&child).unwrap())];
            item = try_item_state_from_user_item(item, &wire).unwrap();
            let child = converted_fixture(&session, item.socketed[0].clone());
            item.socketed[0] = child;
            assert!(
                item.economic_conversion.is_none(),
                "the host is an otherwise legacy carrier"
            );
        } else {
            item = converted_fixture(&session, item);
        }
        assert!(!has_bound(&item) && has_economic_provenance(&item));
        install(&mut session, vec![item]);
        let before = census(&session);
        let rejected = session.handle_packet(ClientPacket::SellItem {
            unique_id: 93_004,
            count: 1,
        });
        assert!(
            rejected
                .iter()
                .any(|packet| matches!(packet, ServerPacket::SellItem { success: false, .. })),
            "{nested}: {rejected:?}"
        );
        assert!(!rejected
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedGold { .. })));
        assert_eq!(
            census(&session),
            before,
            "{nested}: refusal precedes debit, old gold and lossy buyback publication"
        );
    }
}
