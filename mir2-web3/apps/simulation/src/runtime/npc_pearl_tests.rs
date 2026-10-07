use super::super::resources::Stage5SystemsResource;
use super::*;
use crate::{SimulationConfig, SimulationSession, VisibleNpcRecord};
use mir2_protocol::{ClientPacket, MirDirection, Point};

fn session() -> SimulationSession {
    let mut config = SimulationConfig::default();
    config.visible_npcs.push(VisibleNpcRecord {
        object_id: 4990,
        name: "Wicked Trader".into(),
        image: 5,
        colour_argb: -1,
        position: Point { x: 331, y: 271 },
        direction: MirDirection::Left,
        quest_ids: vec![],
        script_key: Some("BichonProvince/NaturalCave/WickedTrader".into()),
    });
    let mut session = SimulationSession::new(config);
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    session
}

#[test]
fn pearl_npc_purchase_uses_only_pearl_wallet_and_rejects_insufficient_funds() {
    let mut s = session();
    s.interact(4990);
    let goods = s.select_npc_dialog_target("@BuySell");
    let uid = goods
        .iter()
        .find_map(|p| match p {
            ServerPacket::NPCGoods { list, .. } => list
                .iter()
                .find(|i| i.item_index == 658)
                .map(|i| i.unique_id),
            _ => None,
        })
        .unwrap();
    // The fixture offers a real source trade catalog through the PearlBuy service.
    s.app
        .world_mut()
        .resource_mut::<NpcStateResource>()
        .active_npc_service
        .as_mut()
        .unwrap()
        .label_key = "PEARLBUY".into();
    let gold = s.app.world().resource::<PlayerRuntimeResource>().gold;
    let before = s
        .app
        .world()
        .resource::<InventoryResource>()
        .inventory_items
        .clone();
    let buy = ClientPacket::BuyItem {
        item_index: uid,
        count: 2,
        panel_type: 0,
    };
    assert!(s.handle_packet(buy.clone()).is_empty());
    assert_eq!(
        serde_json::to_value(
            &s.app
                .world()
                .resource::<InventoryResource>()
                .inventory_items
        )
        .unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    s.app
        .world_mut()
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .intelligent_creature_pearls = 240;
    let packets = s.handle_packet(buy);
    assert!(packets.iter().any(|p| matches!(p, ServerPacket::GainedItem { item } if item.item_index == 658 && item.count == 2)));
    assert!(!packets
        .iter()
        .any(|p| matches!(p, ServerPacket::LoseGold { .. })));
    assert_eq!(s.app.world().resource::<PlayerRuntimeResource>().gold, gold);
    assert_eq!(
        s.app
            .world()
            .resource::<Stage5SystemsResource>()
            .stage5_systems
            .intelligent_creature_pearls,
        80
    );
    s.handle_packet(ClientPacket::LogOut);
    s.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert_eq!(s.app.world().resource::<Stage5SystemsResource>().stage5_systems.intelligent_creature_pearls, 80);
    assert_eq!(s.app.world().resource::<PlayerRuntimeResource>().gold, gold);
}

#[test]
fn pearl_npc_packet_and_script_wallet_bounds_match_source() {
    let mut s = session();
    let script = crystal_npc_script_by_key("BichonProvince/NaturalCave/WickedTrader").unwrap();
    let packets = crystal_npc_service_packets_for_label(Some(&script), "[@PEARLBUY]").unwrap();
    assert!(
        matches!(&packets[0], ServerPacket::NPCPearlGoods { list, panel_type: 0, .. } if !list.is_empty())
    );
    let mut state = super::super::npc_script::CrystalNpcExecutionState::default();
    for (line, expected) in [
        ("GIVEPEARLS 4294967295", i32::MAX),
        ("GIVEPEARLS -1", i32::MAX),
        ("TAKEPEARLS 4294967295", 0),
        ("GIVEPEARLS 17", 17),
        ("TAKEPEARLS 3", 14),
    ] {
        super::super::npc_script::execute_crystal_npc_action_line(
            s.app.world_mut(),
            line,
            &mut vec![],
            &mut state,
        );
        assert_eq!(
            s.app
                .world()
                .resource::<Stage5SystemsResource>()
                .stage5_systems
                .intelligent_creature_pearls,
            expected
        );
    }
}

// Fill real canonical carried cells without inventing a capacity or a belt type.
fn pearl_full_carried_fixture(s: &mut SimulationSession) {
    use super::super::items::{crystal_belt_slot_range_for_item_key, embedded_item_state_from_template};
    let bag_template = crystal_item_by_index(658).unwrap();
    assert!(bag_template.stack_size > 2);
    let inventory = &mut *s.app.world_mut().resource_mut::<InventoryResource>();
    inventory.inventory_capacity = 46;
    inventory.inventory_items.clear();
    inventory.belt_items.clear();
    for slot in 0..40u8 {
        let mut item = embedded_item_state_from_template(&bag_template, ItemContainer::Bag1, slot);
        item.unique_id = 1_000_000 + u64::from(slot);
        item.quantity = if slot == 8 { 1 } else { u32::from(bag_template.stack_size) };
        inventory.inventory_items.push(item);
    }
    let templates = mir2_game_data::crystal_item_manifest();
    for slot in 0..6u8 {
        let template = templates.items.iter().find(|template| {
            crystal_belt_slot_range_for_item_key(&crystal_item_key_for_template(template))
                .is_some_and(|(start, end)| (start..end).contains(&slot))
        }).unwrap();
        let mut item = embedded_item_state_from_template(template, ItemContainer::Belt, slot);
        item.unique_id = 2_000_000 + u64::from(slot);
        item.quantity = u32::from(template.stack_size.max(1));
        inventory.belt_items.push(item);
    }
}

fn pearl_trade_fixture(s: &mut SimulationSession) -> UserItem {
    s.interact(4990);
    let goods = s.select_npc_dialog_target("@BuySell");
    let item = goods.iter().find_map(|packet| match packet {
        ServerPacket::NPCGoods { list, .. } => list.iter().find(|item| item.item_index == 658).cloned(),
        _ => None,
    }).unwrap();
    s.app.world_mut().resource_mut::<NpcStateResource>()
        .active_npc_service.as_mut().unwrap().label_key = "PEARLBUY".into();
    s.app.world_mut().resource_mut::<Stage5SystemsResource>()
        .stage5_systems.intelligent_creature_pearls = 240;
    item
}

fn pearl_carried_value(s: &SimulationSession) -> serde_json::Value {
    let inventory = s.app.world().resource::<InventoryResource>();
    serde_json::to_value((&inventory.inventory_items, &inventory.belt_items)).unwrap()
}

#[test]
fn pearl_full_bag_and_belt_stack_slack_cannot_debit_or_overlap() {
    let mut s = session();
    let source = pearl_trade_fixture(&mut s);
    pearl_full_carried_fixture(&mut s);
    let template = crystal_item_by_index(source.item_index).unwrap();
    assert!(can_gain_item_quantity(s.app.world().resource::<InventoryResource>(),
        ItemContainer::Bag1, &crystal_item_key_for_template(&template), 2));
    let before = pearl_carried_value(&s);
    let gold = s.app.world().resource::<PlayerRuntimeResource>().gold;
    assert!(s.handle_packet(ClientPacket::BuyItem {
        item_index: source.unique_id, count: 2, panel_type: 0,
    }).is_empty());
    assert_eq!(pearl_carried_value(&s), before);
    assert_eq!(s.app.world().resource::<PlayerRuntimeResource>().gold, gold);
    assert_eq!(s.app.world().resource::<Stage5SystemsResource>()
        .stage5_systems.intelligent_creature_pearls, 240);
}

#[test]
fn pearl_single_legal_free_slot_delivers_legacy_metadata_and_raw_catalog_uid() {
    let mut s = session();
    let source = pearl_trade_fixture(&mut s);
    pearl_full_carried_fixture(&mut s);
    s.app.world_mut().resource_mut::<InventoryResource>()
        .inventory_items.retain(|item| item.slot != 39);
    let before = pearl_carried_value(&s);
    let gold = s.app.world().resource::<PlayerRuntimeResource>().gold;
    // A template index is not the raw catalog selector.
    assert!(s.handle_packet(ClientPacket::BuyItem {
        item_index: u64::try_from(source.item_index).unwrap(), count: 2, panel_type: 0,
    }).is_empty());
    assert_eq!(pearl_carried_value(&s), before);
    let packets = s.handle_packet(ClientPacket::BuyItem {
        item_index: source.unique_id, count: 2, panel_type: 0,
    });
    let incoming = packets.iter().find_map(|packet| match packet {
        ServerPacket::GainedItem { item } => Some(item), _ => None,
    }).unwrap();
    assert_eq!((incoming.item_index, incoming.count, incoming.current_dura, incoming.max_dura),
        (source.item_index, 2, source.current_dura, source.max_dura));
    assert!(!packets.iter().any(|packet| matches!(packet, ServerPacket::LoseGold { .. })));
    assert_eq!(s.app.world().resource::<PlayerRuntimeResource>().gold, gold);
    assert_eq!(s.app.world().resource::<Stage5SystemsResource>()
        .stage5_systems.intelligent_creature_pearls, 80);
    let inventory = s.app.world().resource::<InventoryResource>();
    let delivered = inventory.inventory_items.iter().find(|item| item.slot == 39).unwrap();
    assert_eq!(delivered.unique_id, incoming.unique_id);
    assert_eq!(delivered.container, ItemContainer::Bag1);
    assert_eq!(delivered.quantity, 2);
    assert_eq!(delivered.durability_current, Some(source.current_dura));
    assert_eq!(delivered.durability_max, Some(source.max_dura));
    assert_eq!(delivered.identified, None);
    assert!(delivered.user_item_metadata.is_none());
    assert!(delivered.socketed.is_empty());
    let retained: Vec<_> = inventory.inventory_items.iter().filter(|item| item.slot != 39).cloned().collect();
    assert_eq!(serde_json::to_value((retained, &inventory.belt_items)).unwrap(), before);
}

#[test]
fn pearl_used_fallback_failed_delivery_retains_stock_and_wallet() {
    let mut s = session();
    let mut source = pearl_trade_fixture(&mut s);
    source.unique_id = 9_000_000_001;
    let script_key = s.app.world().resource::<NpcStateResource>()
        .active_npc_service.as_ref().unwrap().script_key.clone();
    push_crystal_npc_used_good_item(&mut s.app.world_mut().resource_mut::<NpcStateResource>(),
        &script_key, source.clone());
    let service = s.app.world().resource::<NpcStateResource>().active_npc_service.clone().unwrap();
    assert!(matches!(crystal_npc_service_item_for_purchase(s.app.world(), &service, source.unique_id),
        Some(CrystalNpcPurchaseItem { source: CrystalNpcPurchaseSource::Used, .. })));
    pearl_full_carried_fixture(&mut s);
    let before = pearl_carried_value(&s);
    let stock = s.app.world().resource::<NpcStateResource>().npc_used_goods_items.clone();
    let gold = s.app.world().resource::<PlayerRuntimeResource>().gold;
    assert!(s.handle_packet(ClientPacket::BuyItem {
        item_index: source.unique_id, count: 2, panel_type: 0,
    }).is_empty());
    assert_eq!(pearl_carried_value(&s), before);
    assert_eq!(s.app.world().resource::<NpcStateResource>().npc_used_goods_items, stock);
    assert_eq!(s.app.world().resource::<PlayerRuntimeResource>().gold, gold);
    assert_eq!(s.app.world().resource::<Stage5SystemsResource>()
        .stage5_systems.intelligent_creature_pearls, 240);
}

#[test]
fn pearl_used_fallback_commits_delivery_wallet_and_selected_stock_together() {
    let mut s = session();
    let mut source = pearl_trade_fixture(&mut s);
    source.unique_id = 9_000_000_002;
    source.count = 2;
    source.current_dura = 1;
    source.max_dura = 2;
    let mut retained = source.clone();
    retained.unique_id += 1;
    let script_key = s.app.world().resource::<NpcStateResource>()
        .active_npc_service.as_ref().unwrap().script_key.clone();
    {
        let mut npc = s.app.world_mut().resource_mut::<NpcStateResource>();
        push_crystal_npc_used_good_item(&mut npc, &script_key, source.clone());
        push_crystal_npc_used_good_item(&mut npc, &script_key, retained.clone());
    }
    pearl_full_carried_fixture(&mut s);
    s.app.world_mut().resource_mut::<InventoryResource>()
        .inventory_items.retain(|item| item.slot != 39);
    let gold = s.app.world().resource::<PlayerRuntimeResource>().gold;
    let packets = s.handle_packet(ClientPacket::BuyItem {
        item_index: source.unique_id, count: 2, panel_type: 0,
    });
    assert!(matches!(&packets[0], ServerPacket::GainedItem { item }
        if (item.item_index, item.count, item.current_dura, item.max_dura)
            == (source.item_index, 2, 1, 2)));
    assert!(matches!(&packets[1], ServerPacket::NPCGoods { list, panel_type: 0, .. }
        if list.iter().any(|item| item.unique_id == retained.unique_id)
            && !list.iter().any(|item| item.unique_id == source.unique_id)));
    assert_eq!(crystal_npc_used_goods_for_script(s.app.world(), &script_key), vec![retained]);
    assert_eq!(s.app.world().resource::<Stage5SystemsResource>()
        .stage5_systems.intelligent_creature_pearls, 80);
    assert_eq!(s.app.world().resource::<PlayerRuntimeResource>().gold, gold);
    let inventory = s.app.world().resource::<InventoryResource>();
    let delivered = inventory.inventory_items.iter().find(|item| item.slot == 39).unwrap();
    assert_eq!((delivered.quantity, delivered.durability_current, delivered.durability_max),
        (2, Some(1), Some(2)));
    assert_eq!(inventory.inventory_items.len(), 40);
    assert_eq!(inventory.belt_items.len(), 6);
}
