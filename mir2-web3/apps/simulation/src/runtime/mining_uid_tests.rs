use super::*;
use super::super::item_grants::tests::{trusted_session, inventory_image};
use crate::SimulationSession;
use std::fs;

fn setup(session: &mut SimulationSession) -> Point {
    let template = crystal_item_by_name("PickAxe").unwrap();
    let world = session.app.world_mut();
    let mut inventory = world.resource_mut::<InventoryResource>();
    let weapon = inventory.equipment_items.iter_mut()
        .find(|item| item.slot == EquipmentSlot::Weapon).unwrap();
    weapon.key = crystal_item_key_for_template(&template);
    weapon.durability_current = 5000;
    weapon.durability_max = 5000;
    drop(inventory);
    let player = player_entity(world).unwrap();
    let target = offset_point(&entity_position(world, player).unwrap(), MirDirection::Right, 1);
    let mut resource = world.resource_mut::<MiningResource>();
    let index = resource.mine_sets.len();
    resource.mine_sets.push(MineSet { spot_regen_rate_minutes: 5, max_stones: 80, hit_rate: 100,
        drop_rate: 100, total_slots: 1, drops: vec![MineDrop { item_name: "GoldOre", min_slot: 0,
            max_slot: 1, min_dura: 3, max_dura: 16, bonus_chance: 20, max_bonus_dura: 10 }] });
    resource.spots.insert((target.x, target.y), MineSpot { mine_set_index: index, stones_left: 10, last_regen_tick: 0 });
    target
}

#[test]
fn guaranteed_mine_uses_shared_uid_and_keeps_ore_purity_and_original_swing_cost() {
    let (mut session, allocator, _) = trusted_session("mine-good", 5_000_000);
    let target = setup(&mut session);
    let packets = try_mine(session.app.world_mut(), MirDirection::Right).unwrap();
    let item = packets.iter().find_map(|packet| if let ServerPacket::GainedItem { item } = packet { Some(item) } else { None }).unwrap();
    assert_eq!(item.unique_id, 5_000_001);
    assert!(item.current_dura >= 3000 && item.current_dura <= 25_000);
    assert_eq!(allocator.issued_through().unwrap(), item.unique_id);
    assert_eq!(session.app.world().resource::<MiningResource>().spots[&(target.x, target.y)].stones_left, 9);
    assert!(packets.iter().any(|packet| matches!(packet, ServerPacket::DuraChanged { .. })));
}

#[test]
fn failed_mine_issuance_keeps_stones_pickaxe_and_inventory() {
    let (mut session, _, path) = trusted_session("mine-fail", 5_000_000);
    let target = setup(&mut session);
    let before = inventory_image(&session);
    fs::write(path, "broken").unwrap();
    let packets = try_mine(session.app.world_mut(), MirDirection::Right).unwrap();
    assert_eq!(inventory_image(&session), before);
    assert_eq!(session.app.world().resource::<MiningResource>().spots[&(target.x, target.y)].stones_left, 10);
    assert!(packets.iter().any(|packet| matches!(packet, ServerPacket::Chat { .. })));
    assert!(!packets.iter().any(|packet| matches!(packet, ServerPacket::GainedItem { .. } | ServerPacket::DuraChanged { .. } | ServerPacket::MapEffect { .. })));
}
