//! Retrieval uses normalized bag indexes and releases only the offered instance.
use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::{ItemContainer, SimulationConfig, SimulationSession};
use serde_json::{json, Value};

fn started() -> SimulationSession {
    let mut session = SimulationSession::new(SimulationConfig::default());
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        })
        .iter()
        .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(session.active_identity().is_some());
    session
}

fn offer(session: &mut SimulationSession) -> u8 {
    session.trade_request("Trader");
    session.handle_packet(ClientPacket::TradeReply {
        accept_invite: true,
    });
    let snapshot = session.world_snapshot();
    let item = snapshot
        .inventory_items
        .iter()
        .find(|i| i.key == "red-potion" && i.container == ItemContainer::Bag1)
        .unwrap();
    let source = item.slot;
    assert!(session
        .handle_packet(ClientPacket::DepositTradeItem {
            from: source.into(),
            to: 3,
        })
        .iter()
        .any(|p| matches!(p, ServerPacket::DepositTradeItem { success: true, .. })));
    source
}

fn retrieve(session: &mut SimulationSession, from: i32, to: i32, success: bool) {
    let packets = session.handle_packet(ClientPacket::RetrieveTradeItem { from, to });
    assert!(
        packets.contains(&ServerPacket::RetrieveTradeItem { from, to, success }),
        "{packets:?}"
    );
}

#[test]
fn retrieve_moves_exact_instance_to_requested_bag_and_survives_reload() {
    for destination in [35, 75] {
        let mut session = started();
        let source = offer(&mut session);
        let before = session.world_snapshot();
        let mut expected = before.inventory_items.clone();
        let item = expected
            .iter_mut()
            .find(|i| i.container == ItemContainer::Bag1 && i.slot == source)
            .unwrap();
        item.container = if destination < 40 {
            ItemContainer::Bag1
        } else {
            ItemContainer::Bag2
        };
        item.slot = if destination < 40 {
            destination
        } else {
            destination - 40
        };
        retrieve(&mut session, 3, destination.into(), true);
        let after = session.world_snapshot();
        assert_eq!(
            after.inventory_items, expected,
            "metadata, quantity and UID must be unchanged"
        );
        let trade = after.stage5_systems.trade.unwrap();
        assert!(
            trade.offered_slots.is_empty()
                && trade.offered_unique_ids.is_empty()
                && trade.offered_items.is_empty()
        );
        let checkpoint = session.active_character_checkpoint().unwrap();
        let mut restored = started();
        restored
            .restore_active_character_checkpoint(&checkpoint)
            .unwrap();
        assert_eq!(restored.world_snapshot().inventory_items, expected);
    }
}

#[test]
fn retrieving_to_original_retained_slot_is_supported() {
    let mut session = started();
    let source = offer(&mut session);
    let before = session.world_snapshot().inventory_items;
    retrieve(&mut session, 3, source.into(), true);
    assert_eq!(session.world_snapshot().inventory_items, before);
    assert!(session
        .world_snapshot()
        .stage5_systems
        .trade
        .unwrap()
        .offered_slots
        .is_empty());
}

#[test]
fn invalid_or_occupied_destination_preserves_inventory_and_offer() {
    let mut session = started();
    let source = offer(&mut session);
    let before = session.world_snapshot();
    let occupied = before
        .inventory_items
        .iter()
        .find(|i| i.container == ItemContainer::Bag1 && i.slot != source)
        .unwrap()
        .slot;
    for destination in [-1, 80, 86, 256, occupied.into()] {
        retrieve(&mut session, 3, destination, false);
        let after = session.world_snapshot();
        assert_eq!(after.inventory_items, before.inventory_items);
        assert_eq!(after.stage5_systems.trade, before.stage5_systems.trade);
    }
    for from in [-1, 10, 256] {
        retrieve(&mut session, from, 35, false);
        assert_eq!(
            session.world_snapshot().stage5_systems.trade,
            before.stage5_systems.trade
        );
    }
}

#[test]
fn stale_or_missing_offered_identity_cannot_release_replacement_item() {
    for stale in [true, false] {
        let mut session = started();
        offer(&mut session);
        let mut checkpoint = session.active_character_checkpoint().unwrap();
        let mut systems: Value =
            serde_json::from_str(checkpoint.stage5_systems_json.as_ref().unwrap()).unwrap();
        if stale {
            systems["trade"]["offeredUniqueIds"]["3"] = json!(u64::MAX);
        } else {
            systems["trade"]["offeredUniqueIds"] = json!({});
        }
        checkpoint.stage5_systems_json = Some(systems.to_string());
        session
            .restore_active_character_checkpoint(&checkpoint)
            .unwrap();
        let before = session.world_snapshot();
        retrieve(&mut session, 3, 35, false);
        let after = session.world_snapshot();
        assert_eq!(after.inventory_items, before.inventory_items);
        assert_eq!(after.stage5_systems.trade, before.stage5_systems.trade);
    }
}

#[test]
fn locked_and_prepared_trade_reject_retrieval_without_mutation() {
    for prepared in [false, true] {
        let mut session = started();
        offer(&mut session);
        if prepared {
            assert!(session.shared_trade_confirm().1.is_some());
        } else {
            session.handle_packet(ClientPacket::TradeConfirm { locked: true });
        }
        let before = session.world_snapshot();
        retrieve(&mut session, 3, 35, false);
        let after = session.world_snapshot();
        assert_eq!(after.inventory_items, before.inventory_items);
        assert_eq!(after.stage5_systems.trade, before.stage5_systems.trade);
    }
}

#[test]
fn retrieval_obeys_current_expansion_capacity() {
    let mut session = started();
    offer(&mut session);
    let mut checkpoint = session.active_character_checkpoint().unwrap();
    checkpoint.inventory_capacity = 54; // Six belt cells, 48 normalized bag cells.
    session
        .restore_active_character_checkpoint(&checkpoint)
        .unwrap();
    assert_eq!(session.world_snapshot().inventory_capacity, 54);
    let before = session.world_snapshot();
    retrieve(&mut session, 3, 48, false);
    assert_eq!(
        session.world_snapshot().inventory_items,
        before.inventory_items
    );
    assert_eq!(
        session.world_snapshot().stage5_systems.trade,
        before.stage5_systems.trade
    );
    retrieve(&mut session, 3, 47, true);
    assert!(session
        .world_snapshot()
        .inventory_items
        .iter()
        .any(|item| item.container == ItemContainer::Bag2
            && item.slot == 7
            && item.key == "red-potion"));
}

#[test]
fn explicit_legacy_zero_uid_stays_zero_on_wire_after_move_and_reload() {
    let mut session = started();
    let mut checkpoint = session.active_character_checkpoint().unwrap();
    let raw = checkpoint
        .inventory_items_json
        .iter_mut()
        .find(|raw| {
            let item: Value = serde_json::from_str(raw).unwrap();
            item["key"] == "red-potion"
        })
        .unwrap();
    let mut legacy: Value = serde_json::from_str(raw).unwrap();
    legacy["unique_id"] = json!(0);
    legacy["slot"] = json!(0);
    legacy.as_object_mut().unwrap().remove("user_item_metadata");
    *raw = legacy.to_string();
    session
        .restore_active_character_checkpoint(&checkpoint)
        .unwrap();
    assert_eq!(offer(&mut session), 0);
    retrieve(&mut session, 3, 35, true);
    let checkpoint = session.active_character_checkpoint().unwrap();
    let saved: Value = serde_json::from_str(
        checkpoint
            .inventory_items_json
            .iter()
            .find(|raw| {
                let item: Value = serde_json::from_str(raw).unwrap();
                item["key"] == "red-potion"
            })
            .unwrap(),
    )
    .unwrap();
    assert_eq!(saved["unique_id"], json!(0));
    assert_eq!(saved["slot"], json!(35));
    assert!(saved["user_item_metadata"].is_object());
    let mut restored = started();
    restored
        .restore_active_character_checkpoint(&checkpoint)
        .unwrap();
    for session in [&mut session, &mut restored] {
        let packets = session.handle_packet(ClientPacket::DepositTradeItem { from: 35, to: 3 });
        let item = packets
            .iter()
            .find_map(|packet| match packet {
                ServerPacket::TradeItem { trade_items } => trade_items[3].as_ref(),
                _ => None,
            })
            .expect("deposit emits the retrieved instance on the wire");
        assert_eq!(item.unique_id, 0);
        assert_eq!(item.count, 5);
    }
}

#[test]
fn explicit_metadata_and_quantity_are_unchanged_by_retrieval() {
    let mut session = started();
    let mut checkpoint = session.active_character_checkpoint().unwrap();
    let raw = checkpoint
        .inventory_items_json
        .iter_mut()
        .find(|raw| {
            let item: Value = serde_json::from_str(raw).unwrap();
            item["key"] == "red-potion"
        })
        .unwrap();
    let mut item: Value = serde_json::from_str(raw).unwrap();
    item["unique_id"] = json!(987654321);
    item["quantity"] = json!(7);
    item["user_item_metadata"] = json!({
        "item_index": 658, "awake_type": 1, "awake_values": [2, 3],
        "refined_value": 2, "refine_added": 4, "refine_success_chance": 12,
        "wedding_ring": -1, "gm_made": true
    });
    *raw = item.to_string();
    session
        .restore_active_character_checkpoint(&checkpoint)
        .unwrap();
    offer(&mut session);
    let before = session.active_character_checkpoint().unwrap();
    let mut expected: Value = serde_json::from_str(
        before
            .inventory_items_json
            .iter()
            .find(|raw| {
                let item: Value = serde_json::from_str(raw).unwrap();
                item["key"] == "red-potion"
            })
            .unwrap(),
    )
    .unwrap();
    expected["slot"] = json!(35);
    retrieve(&mut session, 3, 35, true);
    let after = session.active_character_checkpoint().unwrap();
    let actual: Value = serde_json::from_str(
        after
            .inventory_items_json
            .iter()
            .find(|raw| {
                let item: Value = serde_json::from_str(raw).unwrap();
                item["key"] == "red-potion"
            })
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        actual, expected,
        "only destination slot may change in exact saved state"
    );
    let mut restored = started();
    restored
        .restore_active_character_checkpoint(&after)
        .unwrap();
    assert_eq!(
        restored.world_snapshot().inventory_items,
        session.world_snapshot().inventory_items
    );
}
