use super::*;
use crate::config::SimulationConfig;
use crate::runtime::items::{embedded_item_state_from_template, use_item};
use crate::runtime::resources::SessionResource;
use crate::runtime::session::SimulationSession;

fn session() -> SimulationSession {
    let config = SimulationConfig::default();
    let selected_character = config.default_character.clone();
    let mut session = SimulationSession::new(config);
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .selected_character = Some(selected_character);
    let mut resources = session.app.world_mut().resource_mut::<InventoryResource>();
    resources.inventory_items.clear();
    resources.storage_items.clear();
    resources.equipment_items.clear();
    drop(resources);
    session
}

fn item(name: &str, unique_id: u64, slot: u8) -> ItemState {
    let template = mir2_game_data::crystal_item_by_name(name).expect("real Crystal item");
    let mut item = embedded_item_state_from_template(&template, ItemContainer::Bag1, slot);
    item.unique_id = unique_id;
    item
}

fn use_inventory_item(session: &mut SimulationSession, unique_id: u64) -> Vec<ServerPacket> {
    use_item(
        session.app.world_mut(),
        "",
        Some((unique_id, MirGridType::Inventory)),
    )
}

fn assert_use_succeeded(packets: &[ServerPacket], expected_uid: u64) {
    assert!(
        packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::UseItem {
                unique_id,
                grid: MirGridType::Inventory,
                success: true,
            } if *unique_id == expected_uid
        )),
        "expected successful use acknowledgement, got {packets:?}"
    );
}

fn worn_uid(resources: &InventoryResource, slot: EquipmentSlot) -> Option<u64> {
    resources
        .equipment_items
        .iter()
        .find(|item| item.slot == slot)
        .and_then(|item| item.user_item_unique_id)
}

#[test]
fn use_item_replaces_occupied_weapon_and_returns_previous_instance_to_source_cell() {
    let mut session = session();
    let mut incoming = item("WoodenSword", 91_001, 7);
    incoming.added_attack = 3;
    incoming.durability_current = Some(1);
    let mut previous = item("WoodenSword", 91_000, 12);
    previous.added_defence = 2;
    let previous = equipment_state_from_item_state(&previous, EquipmentSlot::Weapon);
    {
        let mut resources = session.app.world_mut().resource_mut::<InventoryResource>();
        resources.inventory_items.push(incoming);
        resources.equipment_items.push(previous);
    }

    assert_use_succeeded(&use_inventory_item(&mut session, 91_001), 91_001);
    let resources = session.app.world().resource::<InventoryResource>();
    assert_eq!(worn_uid(resources, EquipmentSlot::Weapon), Some(91_001));
    assert_eq!(resources.equipment_items[0].added_attack, 3);
    assert_eq!(resources.equipment_items[0].durability_current, 1);
    assert_eq!(resources.inventory_items.len(), 1);
    let returned = &resources.inventory_items[0];
    assert_eq!(returned.unique_id, 91_000);
    assert_eq!(returned.container, ItemContainer::Bag1);
    assert_eq!(returned.slot, 7);
    assert_eq!(returned.added_defence, 2);
}

#[test]
fn use_item_wears_two_ring_instances_then_replaces_left_without_losing_right() {
    let mut session = session();
    {
        let mut resources = session.app.world_mut().resource_mut::<InventoryResource>();
        resources.inventory_items.extend([
            item("CopperRing", 92_001, 7),
            item("CopperRing", 92_002, 8),
            item("CopperRing", 92_003, 9),
        ]);
    }

    assert_use_succeeded(&use_inventory_item(&mut session, 92_001), 92_001);
    assert_use_succeeded(&use_inventory_item(&mut session, 92_002), 92_002);
    {
        let resources = session.app.world().resource::<InventoryResource>();
        assert_eq!(worn_uid(resources, EquipmentSlot::RingRight), Some(92_001));
        assert_eq!(worn_uid(resources, EquipmentSlot::RingLeft), Some(92_002));
        assert_eq!(resources.equipment_items.len(), 2);
    }
    assert_use_succeeded(&use_inventory_item(&mut session, 92_003), 92_003);
    let resources = session.app.world().resource::<InventoryResource>();
    assert_eq!(worn_uid(resources, EquipmentSlot::RingRight), Some(92_001));
    assert_eq!(worn_uid(resources, EquipmentSlot::RingLeft), Some(92_003));
    assert_eq!(resources.equipment_items.len(), 2);
    assert_eq!(resources.inventory_items.len(), 1);
    assert_eq!(resources.inventory_items[0].unique_id, 92_002);
    assert_eq!(resources.inventory_items[0].slot, 9);
}

#[test]
fn use_item_fills_empty_right_ring_when_left_ring_is_cursed() {
    let mut session = session();
    let mut previous = item("CopperRing", 93_000, 7);
    previous.cursed = true;
    {
        let mut resources = session.app.world_mut().resource_mut::<InventoryResource>();
        resources
            .inventory_items
            .push(item("CopperRing", 93_001, 8));
        resources
            .equipment_items
            .push(equipment_state_from_item_state(
                &previous,
                EquipmentSlot::RingLeft,
            ));
    }

    assert_use_succeeded(&use_inventory_item(&mut session, 93_001), 93_001);
    let resources = session.app.world().resource::<InventoryResource>();
    assert_eq!(worn_uid(resources, EquipmentSlot::RingLeft), Some(93_000));
    assert_eq!(worn_uid(resources, EquipmentSlot::RingRight), Some(93_001));
}

#[test]
fn explicit_ring_target_replaces_selected_slot_even_if_other_slot_is_empty() {
    let mut session = session();
    {
        let mut resources = session.app.world_mut().resource_mut::<InventoryResource>();
        resources
            .inventory_items
            .push(item("CopperRing", 94_001, 8));
        resources
            .equipment_items
            .push(equipment_state_from_item_state(
                &item("CopperRing", 94_000, 7),
                EquipmentSlot::RingRight,
            ));
    }
    let right = equipment_slot_index(EquipmentSlot::RingRight).unwrap() as i32;
    let packets = equip_item_impl(
        session.app.world_mut(),
        MirGridType::Inventory,
        94_001,
        right,
    );
    assert!(packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::EquipItem { to, success: true, .. } if *to == right
    )));
    let resources = session.app.world().resource::<InventoryResource>();
    assert_eq!(worn_uid(resources, EquipmentSlot::RingRight), Some(94_001));
    assert_eq!(worn_uid(resources, EquipmentSlot::RingLeft), None);
    assert_eq!(resources.inventory_items[0].unique_id, 94_000);
    assert_eq!(resources.inventory_items[0].slot, 8);
}
