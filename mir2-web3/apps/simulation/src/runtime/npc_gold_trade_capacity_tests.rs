//! Ordinary Gold Trade capacity is exercised through the real in-memory session.
//! Expected carriers are built independently; the production gain planner is not an oracle.
use super::*;
use super::super::components::Position;
use super::super::equipment::{equipment_state_from_item_state, user_item_from_equipment_state};
use super::super::items::{embedded_item_state_from_template, try_item_state_from_user_item,
    try_user_item_from_item_state};
use crate::{EquipmentSlot, ItemContainer, SimulationConfig, SimulationSession, VisibleNpcRecord};
use mir2_protocol::{ClientPacket, MirDirection, Point};

fn trade_session() -> (SimulationSession, u64) {
    let mut config = SimulationConfig::default();
    config.visible_npcs.push(VisibleNpcRecord {
        object_id: 4990, name: "Wicked Trader".into(), image: 5, colour_argb: -1,
        position: Point { x: 331, y: 271 }, direction: MirDirection::Left,
        quest_ids: vec![], script_key: Some("BichonProvince/NaturalCave/WickedTrader".into()),
    });
    let mut session = SimulationSession::new(config);
    let login = session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(), password: "demo".into(),
    });
    assert!(login.iter().any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    session.interact(4990);
    let goods = session.select_npc_dialog_target("@BuySell");
    let catalog_uid = goods.iter().find_map(|p| match p {
        ServerPacket::NPCGoods { list, .. } => list.iter()
            .find(|i| i.item_index == 658 && i.count == 1).map(|i| i.unique_id),
        _ => None,
    }).expect("actual Wicked Trader HP Drug catalogue");
    session.app.world_mut().resource_mut::<PlayerRuntimeResource>().gold = 100_000;
    (session, catalog_uid)
}

// A fresh expected delivery, not copied from the catalogue and not obtained from the planner.
fn fresh_wire(template: &CrystalItemTemplate, uid: u64, count: u16) -> UserItem {
    UserItem {
        unique_id: uid, item_index: template.item_index, current_dura: template.durability,
        max_dura: template.durability, count, soul_bound_id: -1, identified: false,
        cursed: false, slots: vec![None; usize::from(template.slots)], gem_count: 0,
        added_stats: vec![], awake_type: 0, awake_values: vec![], refined_value: 0,
        refine_added: 0, refine_success_chance: 0, wedding_ring: -1, expire_info: None,
        rental_information: None, is_shop_item: false, sealed_info: None, gm_made: false,
    }
}

fn carrier(template: &CrystalItemTemplate, container: ItemContainer, slot: u8,
    wire: &UserItem) -> ItemState {
    try_item_state_from_user_item(embedded_item_state_from_template(template, container, slot), wire)
        .expect("independent complete carrier fixture")
}

fn fresh_at(index: i32, container: ItemContainer, slot: u8, uid: u64, count: u16) -> ItemState {
    let template = crystal_item_by_index(index).unwrap();
    carrier(&template, container, slot, &fresh_wire(&template, uid, count))
}

fn fill_carried(session: &mut SimulationSession) {
    let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
    inventory.inventory_capacity = 86; // 80 bag cells plus six belt cells.
    inventory.inventory_items = (0u8..80).map(|index| {
        let (container, slot) = if index < 40 { (ItemContainer::Bag1, index) }
            else { (ItemContainer::Bag2, index - 40) };
        fresh_at(595, container, slot, 10_000 + u64::from(index), 1)
    }).collect();
    inventory.belt_items = (0u8..6).map(|slot| {
        // Potions belong in 0..4; amulets/poisons in 4..6.
        let index = if slot < 4 { 658 } else { 712 };
        let template = crystal_item_by_index(index).unwrap();
        fresh_at(index, ItemContainer::Belt, slot, 20_000 + u64::from(slot), template.stack_size)
    }).collect();
    inventory.equipment_items.clear();
    inventory.storage_items.clear();
    inventory.reserved_item_unique_ids.clear();
}

fn buy(session: &mut SimulationSession, uid: u64, count: u16) -> Vec<ServerPacket> {
    session.handle_packet(ClientPacket::BuyItem { item_index: uid, count, panel_type: 0 })
}

fn carried_snapshot(session: &SimulationSession) -> serde_json::Value {
    let i = session.app.world().resource::<InventoryResource>();
    let player = session.app.world().resource::<PlayerRuntimeResource>();
    // Includes unrelated carriers, reservations and warehouse metadata, not only bag quantities.
    serde_json::json!({
        "gold": player.gold,
        "reserved": i.reserved_item_unique_ids, "capacity": i.inventory_capacity,
        "bag": i.inventory_items, "belt": i.belt_items, "equipment": i.equipment_items,
        "storage": i.storage_items, "storageSize": i.storage_size,
        "expanded": i.has_expanded_storage,
        "expiry": i.expanded_storage_expiry_time_binary_datetime,
        "expiryNotice": i.expanded_storage_expiry_notice_pending,
        "unlocked": i.storage_unlocked, "sent": i.storage_sent,
        "hasPassword": i.storage_has_password,
        "passwordLastSet": i.storage_password_last_set_binary_datetime,
    })
}

fn only_delivery(packets: &[ServerPacket], quantity: u16, gold: u32) -> UserItem {
    // Session finalization may also emit unrelated quest/AOI packets.
    let economic: Vec<_> = packets.iter().filter(|p| matches!(p,
        ServerPacket::LoseGold { .. } | ServerPacket::GainedItem { .. })).collect();
    assert_eq!(economic.len(), 2, "ordinary Trade emits one debit and one purchase delta");
    assert!(matches!(economic[0], ServerPacket::LoseGold { gold: actual } if *actual == gold));
    let ServerPacket::GainedItem { item } = economic[1] else { panic!("purchase delta"); };
    assert_eq!(item.count, quantity, "GainedItem is incoming quantity, not merged total");
    item.clone()
}

fn assert_rejected(session: &mut SimulationSession, uid: u64, count: u16) {
    let before = carried_snapshot(session);
    assert!(buy(session, uid, count).is_empty());
    assert_eq!(carried_snapshot(session), before, "no partial merge or wallet debit");
}

#[test]
fn npc_gold_trade_full_bag_and_belt_merge_preserves_exact_existing_carrier() {
    let (mut session, uid) = trade_session();
    fill_carried(&mut session);
    let template = crystal_item_by_index(658).unwrap();
    let mut expected = fresh_wire(&template, 30_001, 18);
    session.app.world_mut().resource_mut::<InventoryResource>().inventory_items[8] =
        carrier(&template, ItemContainer::Bag1, 8, &expected);
    let before = session.app.world().resource::<InventoryResource>().clone();
    let delta = only_delivery(&buy(&mut session, uid, 2), 2, 160);
    assert_ne!(delta.unique_id, uid);
    assert_ne!(delta.unique_id, expected.unique_id);
    expected.count = 20;
    let after = session.app.world().resource::<InventoryResource>();
    assert_eq!(after.inventory_items.len(), 80);
    assert_eq!(after.belt_items.len(), 6);
    assert_eq!(try_user_item_from_item_state(&after.inventory_items[8]).unwrap(), expected);
    let mut unchanged = after.inventory_items.clone();
    unchanged[8] = before.inventory_items[8].clone();
    assert_eq!(serde_json::to_value(unchanged).unwrap(), serde_json::to_value(before.inventory_items).unwrap());
    assert_eq!(serde_json::to_value(&after.belt_items).unwrap(), serde_json::to_value(&before.belt_items).unwrap());
    assert_eq!(session.app.world().resource::<PlayerRuntimeResource>().gold, 99_840);
}

#[test]
fn npc_gold_trade_multiple_belt_and_bag_stacks_absorb_exact_purchase_delta() {
    let (mut session, uid) = trade_session();
    fill_carried(&mut session);
    {
        let mut i = session.app.world_mut().resource_mut::<InventoryResource>();
        i.belt_items[0] = fresh_at(658, ItemContainer::Belt, 0, 30_010, 18);
        i.inventory_items[8] = fresh_at(658, ItemContainer::Bag1, 8, 30_011, 18);
    }
    let before = session.app.world().resource::<InventoryResource>().clone();
    only_delivery(&buy(&mut session, uid, 4), 4, 320);
    let i = session.app.world().resource::<InventoryResource>();
    for (actual, original) in [(&i.belt_items[0], &before.belt_items[0]),
        (&i.inventory_items[8], &before.inventory_items[8])] {
        let mut expected = try_user_item_from_item_state(original).unwrap();
        expected.count = 20;
        assert_eq!(try_user_item_from_item_state(actual).unwrap(), expected);
    }
    assert_eq!((i.inventory_items.len(), i.belt_items.len()), (80, 6));
    assert_eq!(session.app.world().resource::<PlayerRuntimeResource>().gold, 99_680);
}

#[test]
fn npc_gold_trade_partial_multiple_stack_absorption_without_remainder_cell_rolls_back() {
    let (mut session, uid) = trade_session();
    fill_carried(&mut session);
    {
        let mut i = session.app.world_mut().resource_mut::<InventoryResource>();
        i.belt_items[0] = fresh_at(658, ItemContainer::Belt, 0, 30_020, 19);
        i.inventory_items[8] = fresh_at(658, ItemContainer::Bag1, 8, 30_021, 19);
    }
    assert_rejected(&mut session, uid, 3);
    // Exactly the available two units are a positive control in the same session.
    only_delivery(&buy(&mut session, uid, 2), 2, 160);
    let i = session.app.world().resource::<InventoryResource>();
    assert_eq!(i.belt_items[0].quantity, 20);
    assert_eq!(i.inventory_items[8].quantity, 20);
}

#[test]
fn npc_gold_trade_same_key_different_metadata_never_merges_or_debits() {
    for variant in 0..4 {
        let (mut session, uid) = trade_session();
        fill_carried(&mut session);
        let template = crystal_item_by_index(658).unwrap();
        let mut wire = fresh_wire(&template, 30_030, 18);
        match variant {
            0 => wire.identified = true,
            1 => wire.cursed = true,
            2 => wire.gm_made = true,
            _ => wire.added_stats.push(UserItemStat { stat: 12, value: 1 }),
        }
        session.app.world_mut().resource_mut::<InventoryResource>().inventory_items[8] =
            carrier(&template, ItemContainer::Bag1, 8, &wire);
        assert_rejected(&mut session, uid, 2);
    }
}

#[test]
fn npc_gold_trade_potion_capacity_uses_only_its_legal_belt_partition() {
    let (mut session, uid) = trade_session();
    fill_carried(&mut session);
    session.app.world_mut().resource_mut::<InventoryResource>().belt_items.retain(|i| i.slot < 4);
    assert_rejected(&mut session, uid, 2); // Free amulet cells 4 and 5 are not potion capacity.
    session.app.world_mut().resource_mut::<InventoryResource>().belt_items.retain(|i| i.slot != 0);
    let delta = only_delivery(&buy(&mut session, uid, 2), 2, 160);
    let i = session.app.world().resource::<InventoryResource>();
    let added = i.belt_items.iter().find(|i| i.slot == 0).unwrap();
    assert_eq!(try_user_item_from_item_state(added).unwrap(), delta);
    assert_eq!(i.belt_items.len(), 4);
    assert_eq!(i.inventory_items.len(), 80);
}

#[test]
fn npc_gold_trade_last_legal_bag2_cell_receives_fresh_item_without_duplicate_grid() {
    let (mut session, uid) = trade_session();
    fill_carried(&mut session);
    session.app.world_mut().resource_mut::<InventoryResource>().inventory_items
        .retain(|i| !(i.container == ItemContainer::Bag2 && i.slot == 39));
    let delta = only_delivery(&buy(&mut session, uid, 2), 2, 160);
    let i = session.app.world().resource::<InventoryResource>();
    assert_eq!(i.inventory_items.len(), 80);
    let last: Vec<_> = i.inventory_items.iter()
        .filter(|i| i.container == ItemContainer::Bag2 && i.slot == 39).collect();
    assert_eq!(last.len(), 1);
    assert_eq!(try_user_item_from_item_state(last[0]).unwrap(), delta);
}

#[test]
fn npc_gold_trade_illegal_cells_cannot_supply_capacity_even_with_other_empty_cells() {
    for variant in 0..3 {
        let (mut session, uid) = trade_session();
        fill_carried(&mut session);
        let mut i = session.app.world_mut().resource_mut::<InventoryResource>();
        i.inventory_items.retain(|i| !(i.container == ItemContainer::Bag1 && i.slot == 9));
        match variant {
            0 => i.inventory_items[0].slot = 40,
            1 => i.inventory_capacity = 46, // Populated Bag2 is outside this real capacity.
            _ => {
                i.belt_items.retain(|item| item.slot != 4);
                i.belt_items[0].slot = 4; // Unique cell, but wrong potion belt partition.
            }
        }
        drop(i);
        assert_rejected(&mut session, uid, 2);
    }
}

#[test]
fn npc_gold_trade_duplicate_cell_rejects_entire_staged_plan() {
    let (mut session, uid) = trade_session();
    fill_carried(&mut session);
    let mut i = session.app.world_mut().resource_mut::<InventoryResource>();
    i.inventory_items.retain(|i| !(i.container == ItemContainer::Bag1 && i.slot == 9));
    i.inventory_items.push(fresh_at(658, ItemContainer::Bag1, 8, 30_080, 18));
    drop(i);
    assert_rejected(&mut session, uid, 2);
}

#[test]
fn npc_gold_trade_duplicate_carried_uid_rejects_even_when_physical_cell_is_free() {
    let (mut session, uid) = trade_session();
    fill_carried(&mut session);
    let mut i = session.app.world_mut().resource_mut::<InventoryResource>();
    i.inventory_items.retain(|i| !(i.container == ItemContainer::Bag1 && i.slot == 9));
    let duplicate = i.inventory_items[0].unique_id;
    i.inventory_items[1] = fresh_at(595, ItemContainer::Bag1, 1, duplicate, 1);
    drop(i);
    assert_rejected(&mut session, uid, 2);
}

#[test]
fn npc_gold_trade_fresh_delivery_uses_full_canonical_metadata_and_excludes_catalog_uid() {
    let (mut session, uid) = trade_session();
    let reserved = [uid, 1, 40_001].into_iter().collect();
    {
        let mut i = session.app.world_mut().resource_mut::<InventoryResource>();
        i.inventory_items.clear(); i.belt_items.clear(); i.equipment_items.clear();
        i.storage_items.clear(); i.reserved_item_unique_ids = reserved;
    }
    let before_reserved = session.app.world().resource::<InventoryResource>().reserved_item_unique_ids.clone();
    let delta = only_delivery(&buy(&mut session, uid, 2), 2, 160);
    let template = crystal_item_by_index(658).unwrap();
    assert!(!template.stats.is_empty(), "base HP stat fixture is nonempty");
    assert_ne!(delta.unique_id, 0);
    assert_ne!(delta.unique_id, uid);
    assert!(!before_reserved.contains(&delta.unique_id));
    assert_eq!(delta, fresh_wire(&template, delta.unique_id, 2));
    let i = session.app.world().resource::<InventoryResource>();
    assert_eq!(i.reserved_item_unique_ids, before_reserved);
    let carried: Vec<_> = i.inventory_items.iter().chain(i.belt_items.iter()).collect();
    assert_eq!(carried.len(), 1);
    assert_eq!(try_user_item_from_item_state(carried[0]).unwrap(), delta);
    assert_eq!(carried[0].added_attack, 0);
    assert_eq!(carried[0].added_defence, 0);
    assert!(carried[0].added_stats.is_empty(), "template base stats are not bonuses");
}

#[test]
fn npc_gold_trade_funds_count_panel_catalog_and_service_range_guards_preserve_state() {
    let (mut session, uid) = trade_session();
    let template = crystal_item_by_index(658).unwrap();
    for packet in [
        ClientPacket::BuyItem { item_index: uid, count: 0, panel_type: 0 },
        ClientPacket::BuyItem { item_index: uid, count: template.stack_size + 1, panel_type: 0 },
        ClientPacket::BuyItem { item_index: uid, count: 2, panel_type: 1 },
        ClientPacket::BuyItem { item_index: u64::MAX, count: 2, panel_type: 0 },
    ] {
        let before = carried_snapshot(&session);
        assert!(session.handle_packet(packet).is_empty());
        assert_eq!(carried_snapshot(&session), before);
    }
    session.app.world_mut().resource_mut::<PlayerRuntimeResource>().gold = 159;
    assert_rejected(&mut session, uid, 2);
    session.app.world_mut().resource_mut::<PlayerRuntimeResource>().gold = 100_000;
    let player = player_entity(session.app.world()).unwrap();
    session.app.world_mut().entity_mut(player).insert(Position(Point { x: 0, y: 0 }));
    let before = carried_snapshot(&session);
    let packets = buy(&mut session, uid, 2);
    assert!(packets.iter().all(|p| !matches!(p,
        ServerPacket::LoseGold { .. } | ServerPacket::GainedItem { .. })),
        "out-of-range service cannot debit or deliver; AOI removal is unrelated");
    assert_eq!(carried_snapshot(&session), before);
}

#[test]
fn npc_gold_trade_invalid_carrier_or_external_identity_overlap_never_partially_commits() {
    for quantity in [0, 21] {
        let (mut session, uid) = trade_session();
        fill_carried(&mut session);
        let mut i = session.app.world_mut().resource_mut::<InventoryResource>();
        i.inventory_items[8] = fresh_at(658, ItemContainer::Bag1, 8, 30_120, 18);
        // Corrupt another carried row after constructing a valid typed carrier.
        i.belt_items[1].quantity = quantity;
        drop(i);
        assert_rejected(&mut session, uid, 2);
    }

    for external in 0..3 {
        let (mut session, uid) = trade_session();
        fill_carried(&mut session);
        let collision = 30_121;
        let mut i = session.app.world_mut().resource_mut::<InventoryResource>();
        i.inventory_items[8] = fresh_at(658, ItemContainer::Bag1, 8, collision, 18);
        match external {
            0 => { i.reserved_item_unique_ids.insert(collision); }
            1 => { i.storage_items.push(fresh_at(658, ItemContainer::Storage, 0, collision, 1)); }
            _ => {
                let equipment = equipment_state_from_item_state(
                    &fresh_at(595, ItemContainer::Bag1, 0, collision, 1), EquipmentSlot::Helmet);
                assert_eq!(user_item_from_equipment_state(&equipment).unwrap().unique_id, collision);
                i.equipment_items.push(equipment);
            }
        }
        drop(i);
        assert_rejected(&mut session, uid, 2);
    }
}

#[test]
fn npc_gold_trade_quest_root_and_nested_uid_ambiguity_reject_without_using_quest_capacity() {
    for nested in [false, true] {
        for conflict in [false, true] {
            let (mut session, catalog_uid) = trade_session();
            fill_carried(&mut session);
            let merge_uid = 30_120;
            let quest_uid = if conflict { merge_uid } else { 30_121 };
            let quest = if nested {
                // Shape 7 has the canonical five-slot mount carrier. Its nested
                // UserItem identity must also stay outside the carried UID roster.
                let template = crystal_item_by_index(771).unwrap();
                let mut wire = fresh_wire(&template, 30_199, 1);
                assert_eq!(wire.slots.len(), 5);
                wire.slots[0] = Some(fresh_wire(&crystal_item_by_index(658).unwrap(), quest_uid, 1));
                wire.identified = true;
                wire.gm_made = true;
                carrier(&template, ItemContainer::Quest, 0, &wire)
            } else {
                let template = crystal_item_by_index(595).unwrap();
                let mut wire = fresh_wire(&template, quest_uid, 1);
                wire.identified = true;
                wire.added_stats.push(UserItemStat { stat: 1, value: 3 });
                carrier(&template, ItemContainer::Quest, 0, &wire)
            };
            let quest_wire = try_user_item_from_item_state(&quest).unwrap();
            let quest_bytes = serde_json::to_vec(&quest).unwrap();
            {
                let mut i = session.app.world_mut().resource_mut::<InventoryResource>();
                i.inventory_items[8] = fresh_at(658, ItemContainer::Bag1, 8, merge_uid, 18);
                i.inventory_items.push(quest);
            }
            if conflict {
                assert_rejected(&mut session, catalog_uid, 2);
            } else {
                only_delivery(&buy(&mut session, catalog_uid, 2), 2, 160);
                let i = session.app.world().resource::<InventoryResource>();
                let merged = i.inventory_items.iter().find(|item|
                    item.container == ItemContainer::Bag1 && item.slot == 8).unwrap();
                assert_eq!(try_user_item_from_item_state(merged).unwrap(),
                    fresh_wire(&crystal_item_by_index(658).unwrap(), merge_uid, 20));
                assert_eq!(i.inventory_items.iter().filter(|item|
                    matches!(item.container, ItemContainer::Bag1 | ItemContainer::Bag2)).count(), 80);
                assert_eq!(session.app.world().resource::<PlayerRuntimeResource>().gold, 99_840);
            }
            let i = session.app.world().resource::<InventoryResource>();
            let quests: Vec<_> = i.inventory_items.iter().filter(|item|
                item.container == ItemContainer::Quest).collect();
            assert_eq!(quests.len(), 1);
            assert_eq!(serde_json::to_vec(quests[0]).unwrap(), quest_bytes,
                "Quest root/nested metadata is preserved in both rejection and successful merge");
            assert_eq!(try_user_item_from_item_state(quests[0]).unwrap(), quest_wire);
        }
    }
}

#[test]
fn npc_gold_trade_default_legacy_grid_aliases_preserve_roster_and_accept_fresh_delivery() {
    let (mut session, catalog_uid) = trade_session();
    let inventory = session.app.world().resource::<InventoryResource>();
    for rows in [&inventory.inventory_items, &inventory.belt_items, &inventory.storage_items] {
        assert!(rows.iter().all(|item| item.user_item_metadata.is_none()));
        assert!(rows.iter().any(|item| item.unique_id == 0));
        assert!(rows.iter().any(|item| item.unique_id == 1));
    }
    assert!(inventory.equipment_items.iter().all(|item| item.user_item_unique_id.is_none()));
    assert_eq!(inventory.inventory_capacity, 86);
    let before = carried_snapshot(&session);
    let delta = only_delivery(&buy(&mut session, catalog_uid, 2), 2, 160);
    assert_ne!(delta.unique_id, 0);
    assert_ne!(delta.unique_id, catalog_uid);
    assert_eq!(delta, fresh_wire(&crystal_item_by_index(658).unwrap(), delta.unique_id, 2));
    let mut after = carried_snapshot(&session);
    let mut delivered_rows = 0;
    for grid in ["bag", "belt"] {
        after[grid].as_array_mut().unwrap().retain(|row| {
            let delivered = row["unique_id"].as_u64() == Some(delta.unique_id);
            if delivered { delivered_rows += 1; }
            !delivered
        });
    }
    assert_eq!(delivered_rows, 1, "fresh delivery is a new row, never a legacy merge");
    assert_eq!(after["gold"].as_u64().unwrap(), before["gold"].as_u64().unwrap() - 160);
    after["gold"] = before["gold"].clone();
    assert_eq!(after, before, "all original legacy IDs and complete metadata arrays remain unchanged");
}

#[test]
fn npc_gold_trade_full_roster_legacy_lookalike_stack_supplies_no_merge_slack() {
    let (mut session, catalog_uid) = trade_session();
    fill_carried(&mut session);
    let template = crystal_item_by_index(658).unwrap();
    let mut legacy = fresh_at(658, ItemContainer::Bag1, 8, 30_150, 18);
    legacy.user_item_metadata = None;
    assert_eq!(try_user_item_from_item_state(&legacy).unwrap(), fresh_wire(&template, 30_150, 18));
    let legacy_bytes = serde_json::to_vec(&legacy).unwrap();
    session.app.world_mut().resource_mut::<InventoryResource>().inventory_items[8] = legacy;
    assert_rejected(&mut session, catalog_uid, 2);
    // A real free cell admits the same purchase without using the legacy slack.
    session.app.world_mut().resource_mut::<InventoryResource>().inventory_items.retain(|item|
        !(item.container == ItemContainer::Bag1 && item.slot == 9));
    let delta = only_delivery(&buy(&mut session, catalog_uid, 2), 2, 160);
    let inventory = session.app.world().resource::<InventoryResource>();
    let legacy = inventory.inventory_items.iter().find(|item|
        item.container == ItemContainer::Bag1 && item.slot == 8).unwrap();
    assert_eq!(serde_json::to_vec(legacy).unwrap(), legacy_bytes);
    assert_ne!(delta.unique_id, legacy.unique_id);
    let delivered = inventory.inventory_items.iter().find(|item|
        item.container == ItemContainer::Bag1 && item.slot == 9).unwrap();
    assert_eq!(try_user_item_from_item_state(delivered).unwrap(), delta);
}

#[test]
fn npc_gold_trade_legacy_root_cannot_alias_exact_nested_or_reserved_ownership() {
    for external in 0..7 {
        for conflict in [false, true] {
            let (mut session, catalog_uid) = trade_session();
            fill_carried(&mut session);
            let legacy_uid = 30_150;
            let other_uid = if conflict { legacy_uid } else { 30_151 };
            let mut legacy = fresh_at(658, ItemContainer::Bag1, 8, legacy_uid, 18);
            legacy.user_item_metadata = None;
            let legacy_bytes = serde_json::to_vec(&legacy).unwrap();
            {
                let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
                inventory.inventory_items[8] = legacy;
                inventory.inventory_items.retain(|item|
                    !(item.container == ItemContainer::Bag1 && item.slot == 9));
                match external {
                    0 => inventory.belt_items[0] = fresh_at(658, ItemContainer::Belt, 0, other_uid, 20),
                    1 => {
                        let template = crystal_item_by_index(771).unwrap();
                        let mut wire = fresh_wire(&template, 30_190, 1);
                        wire.slots[0] = Some(fresh_wire(&crystal_item_by_index(658).unwrap(), other_uid, 1));
                        inventory.inventory_items.push(carrier(&template, ItemContainer::Quest, 0, &wire));
                    }
                    2 => {
                        let equipment = equipment_state_from_item_state(
                            &fresh_at(595, ItemContainer::Bag1, 0, other_uid, 1), EquipmentSlot::Helmet);
                        assert_eq!(equipment.user_item_unique_id, Some(other_uid));
                        inventory.equipment_items.push(equipment);
                    }
                    3 => inventory.inventory_items.push(fresh_at(595, ItemContainer::Quest, 0, other_uid, 1)),
                    4 => inventory.storage_items.push(fresh_at(595, ItemContainer::Storage, 0, other_uid, 1)),
                    5 => { inventory.reserved_item_unique_ids.insert(other_uid); }
                    _ => {
                        let mut same_grid = fresh_at(595, ItemContainer::Bag2, 0, other_uid, 1);
                        same_grid.user_item_metadata = None;
                        let position = inventory.inventory_items.iter().position(|item|
                            item.container == ItemContainer::Bag2 && item.slot == 0).unwrap();
                        inventory.inventory_items[position] = same_grid;
                    }
                }
            }
            if conflict {
                assert_rejected(&mut session, catalog_uid, 2);
            } else {
                let delta = only_delivery(&buy(&mut session, catalog_uid, 2), 2, 160);
                assert_ne!(delta.unique_id, legacy_uid);
                assert_ne!(delta.unique_id, other_uid);
                let inventory = session.app.world().resource::<InventoryResource>();
                let delivered = inventory.inventory_items.iter().find(|item|
                    item.container == ItemContainer::Bag1 && item.slot == 9).unwrap();
                assert_eq!(try_user_item_from_item_state(delivered).unwrap(), delta);
            }
            let inventory = session.app.world().resource::<InventoryResource>();
            let retained = inventory.inventory_items.iter().find(|item|
                item.container == ItemContainer::Bag1 && item.slot == 8).unwrap();
            assert_eq!(serde_json::to_vec(retained).unwrap(), legacy_bytes,
                "legacy carrier is unchanged on both collision rejection and valid purchase");
        }
    }
}
