fn fishing_equipment_session() -> SimulationSession {
    let mut session = authenticated_demo_fishing_session();
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .selected_character
        .as_mut()
        .unwrap()
        .level = 30;
    equip_crystal_item(&mut session, "BlueFishingRod", EquipmentSlot::Weapon);
    {
        let rod = session
            .app
            .world()
            .resource::<InventoryResource>()
            .equipment_items
            .iter()
            .find(|i| i.slot == EquipmentSlot::Weapon)
            .unwrap()
            .clone();
        let mut wire = crate::runtime::equipment::user_item_from_equipment_state(&rod).unwrap();
        wire.unique_id = 88000;
        let carrier =
            crate::runtime::equipment::item_state_from_equipment_state(rod, ItemContainer::Bag1, 0);
        let carrier = crate::runtime::items::try_item_state_from_user_item(carrier, &wire).unwrap();
        let rod = crate::runtime::equipment::equipment_state_from_item_state(
            &carrier,
            EquipmentSlot::Weapon,
        );
        *session
            .app
            .world_mut()
            .resource_mut::<InventoryResource>()
            .equipment_items
            .iter_mut()
            .find(|i| i.slot == EquipmentSlot::Weapon)
            .unwrap() = rod;
    }
    for (n, name) in [
        "FishingHook",
        "FishingFloat",
        "FishBait",
        "FishDetector",
        "FishingReel",
    ]
    .iter()
    .enumerate()
    {
        let mut item = fishing_slot_item_state(
            name,
            25 + n as u8,
            88100 + n as u64,
            if n == 2 { 3 } else { 1 },
            Some(20),
        );
        item.container = ItemContainer::Bag1;
        session
            .app
            .world_mut()
            .resource_mut::<InventoryResource>()
            .inventory_items
            .push(item);
    }
    assert_eq!(fishing_rod(&session).slots.len(), 5);
    session
}

fn attach(session: &mut SimulationSession, slot: i32, uid: u64, rod: u64, expected: bool) {
    let packets = session.handle_packet(ClientPacket::EquipSlotItem {
        grid: MirGridType::Inventory,
        unique_id: uid,
        to: slot,
        grid_to: MirGridType::Fishing,
        to_unique_id: rod,
    });
    assert!(
        packets.iter().any(
            |p| matches!(p, ServerPacket::EquipSlotItem {success, ..} if *success == expected)
        ),
        "{packets:?}"
    );
}

fn fishing_rod(session: &SimulationSession) -> mir2_protocol::UserItem {
    crate::runtime::equipment::user_item_from_equipment_state(
        session
            .app
            .world()
            .resource::<InventoryResource>()
            .equipment_items
            .iter()
            .find(|i| i.slot == EquipmentSlot::Weapon)
            .unwrap(),
    )
    .unwrap()
}

#[test]
fn fishing_equipment_five_slots_preserve_items_and_reload() {
    let mut session = fishing_equipment_session();
    for slot in 0..5 {
        attach(&mut session, slot, 88100 + slot as u64, 88000, true);
    }
    let rod = fishing_rod(&session);
    assert_eq!(rod.slots.len(), 5);
    for (slot, item) in rod.slots.iter().enumerate() {
        assert_eq!(item.as_ref().unwrap().unique_id, 88100 + slot as u64);
    }
    assert_eq!(rod.slots[2].as_ref().unwrap().count, 3);
    let checkpoint = session.active_character_checkpoint().unwrap();
    session
        .restore_active_character_checkpoint(&checkpoint)
        .unwrap();
    assert_eq!(fishing_rod(&session), rod);
    let packets = session.handle_packet(ClientPacket::RemoveSlotItem {
        grid: MirGridType::Fishing,
        grid_to: MirGridType::Inventory,
        unique_id: 88102,
        to: 35,
        from_unique_id: 88000,
    });
    assert!(
        packets
            .iter()
            .any(|p| matches!(p, ServerPacket::RemoveSlotItem { success: true, .. })),
        "{packets:?}"
    );
    assert!(fishing_rod(&session).slots[2].is_none());
    assert_eq!(
        session
            .world_snapshot()
            .inventory_items
            .iter()
            .find(|i| i.unique_id == 88102)
            .unwrap()
            .quantity,
        3
    );
}

#[test]
fn fishing_equipment_rejects_wrong_host_type_slot_and_duplicate_without_mutation() {
    let mut session = fishing_equipment_session();
    let before = session.world_snapshot();
    for (slot, uid, rod) in [
        (0, 88100, 88001),
        (1, 88100, 88000),
        (5, 88100, 88000),
        (-1, 88100, 88000),
        (0, 99999, 88000),
    ] {
        attach(&mut session, slot, uid, rod, false);
        assert_eq!(
            session.world_snapshot().inventory_items,
            before.inventory_items
        );
        assert_eq!(
            session.world_snapshot().equipment_items,
            before.equipment_items
        );
    }
    attach(&mut session, 0, 88100, 88000, true);
    attach(&mut session, 0, 88100, 88000, false);
    let rod = fishing_rod(&session);
    for (uid, host, to) in [(88100, 88001, 35), (88104, 88000, 35), (88100, 88000, 99)] {
        let packets = session.handle_packet(ClientPacket::RemoveSlotItem {
            grid: MirGridType::Fishing,
            grid_to: MirGridType::Inventory,
            unique_id: uid,
            to,
            from_unique_id: host,
        });
        assert!(
            packets
                .iter()
                .any(|p| matches!(p, ServerPacket::RemoveSlotItem { success: false, .. })),
            "{packets:?}"
        );
        assert_eq!(fishing_rod(&session), rod);
    }
}

#[test]
fn fishing_equipment_consumption_updates_actual_rod_and_survives_reload() {
    let mut session = fishing_equipment_session();
    let no_hook = session.handle_packet(ClientPacket::FishingCast { cast_out: true });
    assert!(!no_hook
        .iter()
        .any(|p| matches!(p, ServerPacket::FishingUpdate { fishing: true, .. })));
    attach(&mut session, 0, 88100, 88000, true);
    attach(&mut session, 2, 88102, 88000, true);
    let packets = session.handle_packet(ClientPacket::FishingCast { cast_out: true });
    assert!(
        packets
            .iter()
            .any(|p| matches!(p, ServerPacket::FishingUpdate { fishing: true, .. })),
        "{packets:?}"
    );
    let rod = fishing_rod(&session);
    assert_eq!(rod.slots[2].as_ref().unwrap().count, 2);
    assert_eq!(rod.slots[0].as_ref().unwrap().current_dura, 19);
    assert!(packets.iter().any(|p| matches!(
        p,
        ServerPacket::DuraChanged {
            unique_id: 88000,
            ..
        }
    )));
    let checkpoint = session.active_character_checkpoint().unwrap();
    session
        .restore_active_character_checkpoint(&checkpoint)
        .unwrap();
    assert_eq!(fishing_rod(&session), rod);
}

#[test]
fn fishing_equipment_respects_level_and_trade_custody() {
    let mut session = fishing_equipment_session();
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .selected_character
        .as_mut()
        .unwrap()
        .level = 19;
    attach(&mut session, 0, 88100, 88000, false);
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .selected_character
        .as_mut()
        .unwrap()
        .level = 30;
    session.trade_request("Trader");
    session.handle_packet(ClientPacket::TradeReply {
        accept_invite: true,
    });
    let packets = session.handle_packet(ClientPacket::DepositTradeItem { from: 25, to: 0 });
    assert!(
        packets
            .iter()
            .any(|p| matches!(p, ServerPacket::DepositTradeItem { success: true, .. })),
        "{packets:?}"
    );
    let before = session.world_snapshot();
    attach(&mut session, 0, 88100, 88000, false);
    assert_eq!(
        session.world_snapshot().inventory_items,
        before.inventory_items
    );
    assert_eq!(
        session.world_snapshot().stage5_systems.trade,
        before.stage5_systems.trade
    );
}

#[test]
fn fishing_equipment_empty_replacement_cannot_reuse_previous_rod_attachments() {
    let mut session = fishing_equipment_session();
    attach(&mut session, 0, 88100, 88000, true);
    attach(&mut session, 2, 88102, 88000, true);
    session.handle_packet(ClientPacket::FishingCast { cast_out: true });
    equip_crystal_item(&mut session, "BlueFishingRod", EquipmentSlot::Weapon);
    let packets = session.handle_packet(ClientPacket::FishingCast { cast_out: true });
    assert!(
        !packets
            .iter()
            .any(|p| matches!(p, ServerPacket::FishingUpdate { fishing: true, .. })),
        "{packets:?}"
    );
    assert!(fishing_rod(&session).slots.iter().all(Option::is_none));
    assert!(session
        .app
        .world()
        .resource::<FishingResource>()
        .slot_items
        .iter()
        .all(Option::is_none));
}
