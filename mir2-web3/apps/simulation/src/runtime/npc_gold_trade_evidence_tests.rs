//! Read-only capacity evidence uses real owned carriers and the actual snapshot builder.
use super::*;
use super::super::equipment::{equipment_state_from_item_state, seed_equipment_items};
use crate::config::WorldSnapshot;
use crate::SimulationSession;

fn empty_roster() -> InventoryResource {
    let mut inventory = InventoryResource::new(BASE_STORAGE_SLOTS);
    inventory.inventory_capacity = crate::config::CRYSTAL_MAX_INVENTORY_CAPACITY;
    inventory
}

// Independent delivery fixture: never obtain expected metadata or capacity from the planner.
fn fresh_wire(index: i32, uid: u64, count: u16) -> UserItem {
    let template = crystal_item_by_index(index).unwrap();
    UserItem {
        unique_id: uid, item_index: index, current_dura: template.durability,
        max_dura: template.durability, count, soul_bound_id: -1, identified: false,
        cursed: false, slots: vec![None; usize::from(template.slots)], gem_count: 0,
        added_stats: vec![], awake_type: 0, awake_values: vec![], refined_value: 0,
        refine_added: 0, refine_success_chance: 0, wedding_ring: -1, expire_info: None,
        rental_information: None, is_shop_item: false, sealed_info: None, gm_made: false,
    }
}

fn carrier(wire: &UserItem, container: ItemContainer, slot: u8) -> ItemState {
    let template = crystal_item_by_index(wire.item_index).unwrap();
    try_item_state_from_user_item(embedded_item_state_from_template(&template, container, slot), wire)
        .expect("independent complete carrier")
}

fn fresh_at(index: i32, uid: u64, count: u16, container: ItemContainer, slot: u8) -> ItemState {
    carrier(&fresh_wire(index, uid, count), container, slot)
}

fn assert_invalid(inventory: &InventoryResource) {
    let evidence = npc_gold_trade_capacity_evidence(inventory);
    assert!(!evidence.roster_valid);
    assert!(evidence.fresh_compatible_unique_ids.is_empty());
}

#[test]
fn modern_roots_are_listed_independently_of_current_slack() {
    let mut inventory = empty_roster();
    inventory.belt_items.push(fresh_at(658, 103, 20, ItemContainer::Belt, 0));
    inventory.inventory_items.push(fresh_at(658, 101, 19, ItemContainer::Bag1, 8));
    inventory.inventory_items.push(fresh_at(659, 102, 3, ItemContainer::Bag2, 39));
    let evidence = npc_gold_trade_capacity_evidence(&inventory);
    assert!(evidence.roster_valid);
    assert_eq!(evidence.fresh_compatible_unique_ids, vec![101, 102, 103]);
    assert_eq!(inventory.belt_items[0].quantity, 20, "membership is identity, not available room");
}

#[test]
fn full_roster_without_any_gain_capacity_still_reports_valid_identity() {
    let mut inventory = empty_roster();
    for logical in 0u8..80 {
        let (container, slot) = if logical < 40 { (ItemContainer::Bag1, logical) }
            else { (ItemContainer::Bag2, logical - 40) };
        inventory.inventory_items.push(fresh_at(595, 1_000 + u64::from(logical), 1, container, slot));
    }
    for slot in 0u8..6 {
        let index = if slot < 4 { 658 } else { 712 };
        let count = crystal_item_by_index(index).unwrap().stack_size.max(1);
        inventory.belt_items.push(fresh_at(index, 2_000 + u64::from(slot), count, ItemContainer::Belt, slot));
    }
    let evidence = npc_gold_trade_capacity_evidence(&inventory);
    assert!(evidence.roster_valid);
    assert_eq!(evidence.fresh_compatible_unique_ids.len(), 86);
    assert!(evidence.fresh_compatible_unique_ids.contains(&1_079));
    assert!(evidence.fresh_compatible_unique_ids.contains(&2_005));
}

#[test]
fn legacy_cross_grid_roots_stay_occupied_but_unlisted() {
    let mut inventory = empty_roster();
    inventory.inventory_items = seed_inventory_items();
    inventory.belt_items = seed_belt_items();
    inventory.storage_items = seed_storage_items();
    inventory.equipment_items = seed_equipment_items();
    let before = format!("{inventory:?}");
    let evidence = npc_gold_trade_capacity_evidence(&inventory);
    assert!(evidence.roster_valid, "default legacy aliases remain grid-scoped");
    assert!(evidence.fresh_compatible_unique_ids.is_empty());
    assert_eq!(format!("{inventory:?}"), before);
}

#[test]
fn sidecarless_canonical_root_is_not_merge_evidence() {
    let mut inventory = empty_roster();
    let mut item = fresh_at(658, 110, 19, ItemContainer::Bag1, 0);
    item.user_item_metadata = None;
    inventory.inventory_items.push(item);
    let evidence = npc_gold_trade_capacity_evidence(&inventory);
    assert!(evidence.roster_valid);
    assert!(evidence.fresh_compatible_unique_ids.is_empty());
}

#[test]
fn captured_and_reserved_root_conflicts_block_the_whole_roster() {
    for owner in 0..4 {
        let mut inventory = empty_roster();
        inventory.inventory_items.push(fresh_at(658, 120, 19, ItemContainer::Bag1, 0));
        match owner {
            0 => { inventory.reserved_item_unique_ids.insert(120); }
            1 => inventory.storage_items.push(fresh_at(658, 120, 1, ItemContainer::Storage, 0)),
            2 => inventory.belt_items.push(fresh_at(658, 120, 1, ItemContainer::Belt, 0)),
            _ => inventory.equipment_items.push(equipment_state_from_item_state(
                &fresh_at(595, 120, 1, ItemContainer::Bag1, 3), EquipmentSlot::Helmet,
            )),
        }
        assert_invalid(&inventory);
    }
}

#[test]
fn quest_root_and_nested_conflicts_block_without_quest_capacity() {
    for nested in [false, true] {
        for conflict in [false, true] {
            let mut inventory = empty_roster();
            inventory.inventory_items.push(fresh_at(658, 130, 19, ItemContainer::Bag1, 0));
            let id = if conflict { 130 } else { 131 };
            let quest = if nested {
                let mut wire = fresh_wire(771, 132, 1);
                assert_eq!(wire.slots.len(), 5);
                wire.slots[0] = Some(fresh_wire(658, id, 1));
                carrier(&wire, ItemContainer::Quest, 0)
            } else {
                fresh_at(595, id, 1, ItemContainer::Quest, 0)
            };
            inventory.inventory_items.push(quest);
            let evidence = npc_gold_trade_capacity_evidence(&inventory);
            assert_eq!(evidence.roster_valid, !conflict);
            assert_eq!(evidence.fresh_compatible_unique_ids,
                if conflict { vec![] } else { vec![130] });
        }
    }
}

#[test]
fn invalid_carried_cells_and_own_quantities_publish_false() {
    for invalid in 0..7 {
        let mut inventory = empty_roster();
        inventory.inventory_items.push(fresh_at(658, 140, 19, ItemContainer::Bag1, 0));
        match invalid {
            0 => inventory.inventory_items[0].quantity = 0,
            1 => inventory.inventory_items[0].quantity = 21,
            2 => inventory.inventory_items[0].slot = 40,
            3 => { inventory.inventory_items[0].container = ItemContainer::Bag2;
                inventory.inventory_capacity = 46; }
            4 => inventory.inventory_items.push(fresh_at(658, 141, 1, ItemContainer::Bag1, 0)),
            5 => inventory.belt_items.push(fresh_at(658, 142, 1, ItemContainer::Belt, 4)),
            _ => inventory.inventory_capacity = 47,
        }
        assert_invalid(&inventory);
    }
}

#[test]
fn metadata_durability_and_socket_differences_are_not_fresh_compatible() {
    for difference in 0..8 {
        let mut inventory = empty_roster();
        let mut wire = if difference >= 6 { fresh_wire(771, 150, 1) }
            else { fresh_wire(658, 150, 19) };
        match difference {
            0 => wire.identified = true,
            1 => wire.cursed = true,
            2 => wire.gm_made = true,
            3 => wire.is_shop_item = true,
            4 => wire.added_stats.push(UserItemStat { stat: 12, value: 1 }),
            5 => { wire.current_dura = 1; wire.max_dura = 1; }
            6 => wire.slots[0] = Some(fresh_wire(658, 151, 1)),
            _ => { let mut child = fresh_wire(658, 152, 1); child.identified = true;
                wire.slots[0] = Some(child); }
        }
        inventory.inventory_items.push(carrier(&wire, ItemContainer::Bag1, 0));
        let evidence = npc_gold_trade_capacity_evidence(&inventory);
        assert!(evidence.roster_valid, "valid metadata remains an occupied carrier: {difference}");
        assert!(evidence.fresh_compatible_unique_ids.is_empty(), "metadata difference {difference}");
    }
}

#[test]
fn functional_carrier_differences_are_not_erased_by_equal_user_item() {
    for difference in 0..8 {
        let mut inventory = empty_roster();
        let mut item = fresh_at(658, 160, 19, ItemContainer::Bag1, 0);
        let before_wire = try_user_item_from_item_state(&item).unwrap();
        match difference {
            0 => item.name.push('!'),
            1 => item.description.push('!'),
            2 => item.icon = item.icon.saturating_add(1),
            3 => item.weight = item.weight.saturating_add(1),
            4 => item.attack += 1,
            5 => item.defence += 1,
            6 => item.heal_hp += 1,
            _ => item.heal_mp += 1,
        }
        assert_eq!(try_user_item_from_item_state(&item).unwrap(), before_wire,
            "functional difference is absent from protocol UserItem: {difference}");
        inventory.inventory_items.push(item);
        let evidence = npc_gold_trade_capacity_evidence(&inventory);
        assert!(evidence.roster_valid);
        assert!(evidence.fresh_compatible_unique_ids.is_empty(), "functional difference {difference}");
    }
}

#[test]
fn conflicting_raw_template_identity_is_not_a_compatible_row() {
    let mut inventory = empty_roster();
    let mut item = fresh_at(658, 170, 19, ItemContainer::Bag1, 0);
    item.user_item_metadata.as_mut().unwrap().item_index = Some(659);
    inventory.inventory_items.push(item);
    assert_invalid(&inventory);
}

#[test]
fn evidence_never_mutates_inventory_or_allocates_live_identity() {
    let mut inventory = empty_roster();
    inventory.inventory_items.push(fresh_at(658, 180, 19, ItemContainer::Bag1, 0));
    inventory.storage_items.push(fresh_at(595, 181, 1, ItemContainer::Storage, 0));
    inventory.reserved_item_unique_ids.insert(182);
    inventory.storage_sent = true;
    inventory.storage_unlocked = false;
    let before = format!("{inventory:?}");
    for _ in 0..3 {
        assert_eq!(npc_gold_trade_capacity_evidence(&inventory).fresh_compatible_unique_ids, vec![180]);
        assert_eq!(format!("{inventory:?}"), before);
    }
}

#[test]
fn actual_snapshot_serializes_capacity_and_accepts_missing_or_null_legacy_field() {
    let mut session = SimulationSession::new(SimulationConfig::default());
    let mut inventory = empty_roster();
    inventory.inventory_items.push(fresh_at(658, 190, 19, ItemContainer::Bag1, 0));
    *session.app.world_mut().resource_mut::<InventoryResource>() = inventory;
    let snapshot = session.world_snapshot();
    let value = serde_json::to_value(&snapshot).unwrap();
    assert_eq!(serde_json::to_value(snapshot.client_view()).unwrap()["npcGoldTradeCapacity"],
        value["npcGoldTradeCapacity"]);
    assert_eq!(value["npcGoldTradeCapacity"], serde_json::json!({
        "rosterValid": true, "freshCompatibleUniqueIds": [190],
    }));
    assert!(!value.as_object().unwrap().contains_key("npc_gold_trade_capacity"));
    let roundtrip: WorldSnapshot = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(roundtrip.npc_gold_trade_capacity, snapshot.npc_gold_trade_capacity);
    let mut old = value.clone();
    old.as_object_mut().unwrap().remove("npcGoldTradeCapacity");
    assert!(serde_json::from_value::<WorldSnapshot>(old).unwrap().npc_gold_trade_capacity.is_none());
    let mut null = value;
    null["npcGoldTradeCapacity"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<WorldSnapshot>(null).unwrap().npc_gold_trade_capacity.is_none());
    session.app.world_mut().resource_mut::<InventoryResource>().reserved_item_unique_ids.insert(190);
    assert_eq!(serde_json::to_value(session.world_snapshot()).unwrap()["npcGoldTradeCapacity"],
        serde_json::json!({"rosterValid":false,"freshCompatibleUniqueIds":[]}));
}

#[test]
fn packed_capacity_deserialization_is_strict_bounded_and_lossless() {
    for invalid in [
        "{}",
        r#"{"rosterValid":true}"#,
        r#"{"freshCompatibleUniqueIds":[]}"#,
        r#"{"rosterValid":1,"freshCompatibleUniqueIds":[]}"#,
        r#"{"rosterValid":true,"freshCompatibleUniqueIds":[],"extra":0}"#,
        r#"{"rosterValid":true,"rosterValid":false,"freshCompatibleUniqueIds":[]}"#,
        r#"{"rosterValid":true,"freshCompatibleUniqueIds":[],"freshCompatibleUniqueIds":[]}"#,
        r#"{"rosterValid":true,"freshCompatibleUniqueIds":[1,1]}"#,
        r#"{"rosterValid":false,"freshCompatibleUniqueIds":[1]}"#,
        r#"{"rosterValid":true,"freshCompatibleUniqueIds":[-1]}"#,
        r#"{"rosterValid":true,"freshCompatibleUniqueIds":[18446744073709551616]}"#,
    ] {
        assert!(serde_json::from_str::<NpcGoldTradeCapacity>(invalid).is_err(), "{invalid}");
    }
    let too_many = serde_json::json!({"rosterValid":true,"freshCompatibleUniqueIds":(0u64..87).collect::<Vec<_>>()});
    assert!(serde_json::from_value::<NpcGoldTradeCapacity>(too_many).is_err());
    let boundary = serde_json::json!({"rosterValid":true,"freshCompatibleUniqueIds":(0u64..86).collect::<Vec<_>>()});
    assert_eq!(serde_json::from_value::<NpcGoldTradeCapacity>(boundary).unwrap().fresh_compatible_unique_ids.len(), 86);
    let full = r#"{"rosterValid":true,"freshCompatibleUniqueIds":[0,18446744073709551615]}"#;
    let decoded: NpcGoldTradeCapacity = serde_json::from_str(full).unwrap();
    assert_eq!(decoded.fresh_compatible_unique_ids, vec![0, u64::MAX]);
    assert_eq!(serde_json::to_string(&decoded).unwrap(), full);
    let false_roster: NpcGoldTradeCapacity = serde_json::from_str(
        r#"{"rosterValid":false,"freshCompatibleUniqueIds":[]}"#,
    ).unwrap();
    assert!(!false_roster.roster_valid);
}
