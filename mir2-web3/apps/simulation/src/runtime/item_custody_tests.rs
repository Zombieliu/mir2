use super::*;

pub(super) fn activate_refine_service(session: &mut SimulationSession, label: &str) {
    activate_storage_service(session);
    session
        .app
        .world_mut()
        .resource_mut::<NpcStateResource>()
        .active_npc_service
        .as_mut()
        .unwrap()
        .label_key = label.into();
}

#[test]
fn legacy_uid_repair_cannot_choose_a_custodied_slot_alias() {
    let mut session = SimulationSession::new(SimulationConfig::default());
    login_demo_account_for_persistence_test(&mut session);
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let mut resources = session.app.world_mut().resource_mut::<InventoryResource>();
    resources.reserved_item_unique_ids.insert(1);
    resources.equipment_items[0].user_item_unique_id = Some(700_020);
    let mut item = resources.inventory_items[0].clone();
    item.container = ItemContainer::Bag1;
    item.slot = 1;
    item.unique_id = 700_020;
    item.user_item_metadata = None;
    resources.inventory_items = vec![item];
    super::super::super::inventory::normalize_inventory_unique_ids(&mut resources);
    assert_ne!(resources.inventory_items[0].unique_id, 1);
    assert_ne!(resources.inventory_items[0].unique_id, 700_020);
}

#[test]
fn custody_accepts_existing_protocol_zero_uid() {
    let template =
        super::super::super::items::crystal_item_template_for_item_key("dagger").unwrap();
    let mut item = super::super::super::items::embedded_item_state_from_template(
        &template,
        ItemContainer::Bag1,
        0,
    );
    item.unique_id = 0;
    let encoded = super::super::super::item_custody::encode(&item).unwrap();
    let returned = super::super::super::item_custody::decode(&item.key, &encoded).unwrap();
    assert_eq!(returned.unique_id, 0);
    assert!(returned.user_item_metadata.is_some());
    let mut inventory = InventoryResource::new(80);
    inventory.reserved_item_unique_ids.insert(0);
    let (_, changed) =
        super::super::super::item_custody::plan_return(&inventory, &returned, Some(1), false)
            .unwrap();
    let wire = super::super::super::items::try_user_item_from_item_state(&changed[0]).unwrap();
    assert_eq!(
        wire.unique_id, 0,
        "legacy zero must keep its wire identity across slots"
    );
}

#[test]
fn custody_reserved_max_uid_allocation_wraps_without_reusing_held_ids() {
    let mut resources = InventoryResource::new(80);
    resources
        .reserved_item_unique_ids
        .extend([0, 1, 2, u64::MAX]);
    let allocated =
        super::super::super::inventory::allocate_item_unique_id(&resources, ItemContainer::Bag1, 1);
    assert_eq!(allocated, 3);
    let template =
        super::super::super::items::crystal_item_template_for_item_key("dagger").unwrap();
    let mut item = super::super::super::items::embedded_item_state_from_template(
        &template,
        ItemContainer::Bag1,
        1,
    );
    super::super::super::inventory::normalize_fresh_item_tree_unique_ids(
        &resources,
        &mut item,
        &[],
    );
    assert_eq!(item.unique_id, 3);
}

#[test]
fn storage_split_does_not_reuse_custodied_slot_uid() {
    let mut session = SimulationSession::new(SimulationConfig::default());
    login_demo_account_for_persistence_test(&mut session);
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    activate_storage_service(&mut session);
    {
        let mut resources = session.app.world_mut().resource_mut::<InventoryResource>();
        let mut source = resources.storage_items[0].clone();
        source.slot = 0;
        source.unique_id = 700_010;
        source.quantity = 10;
        resources.storage_items = vec![source];
        resources.reserved_item_unique_ids.insert(1);
    }
    let packets = session.handle_packet(ClientPacket::SplitItem {
        grid: MirGridType::Storage,
        unique_id: 700_010,
        count: 2,
    });
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::SplitItem1 { success: true, .. })));
    let resources = session.app.world().resource::<InventoryResource>();
    assert_eq!(
        resources
            .storage_items
            .iter()
            .map(|item| item.quantity)
            .sum::<u32>(),
        10
    );
    assert!(resources
        .storage_items
        .iter()
        .all(|item| item.unique_id != 1));
}

#[test]
fn legacy_equipment_remove_keeps_item_when_destination_uid_is_held() {
    let mut session = SimulationSession::new(SimulationConfig::default());
    login_demo_account_for_persistence_test(&mut session);
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let reference = {
        let mut resources = session.app.world_mut().resource_mut::<InventoryResource>();
        resources
            .inventory_items
            .retain(|item| !(item.container == ItemContainer::Bag1 && item.slot == 1));
        let equipment = &mut resources.equipment_items[0];
        equipment.user_item_unique_id = None;
        let reference = super::super::super::equipment::user_item_from_equipment_state(equipment)
            .unwrap()
            .unique_id;
        resources.reserved_item_unique_ids.insert(1);
        reference
    };
    let before = session.world_snapshot().equipment_items;
    let packets = session.handle_packet(ClientPacket::RemoveItem {
        grid: MirGridType::Inventory,
        unique_id: reference,
        to: 1,
    });
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::RemoveItem { success: false, .. })));
    assert_eq!(session.world_snapshot().equipment_items, before);
}
