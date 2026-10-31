use super::super::item_grants::tests::{inventory_image, trusted_session};
use super::*;
use crate::SimulationSession;
use std::fs;

fn setup(session: &mut SimulationSession) -> Point {
    let template = crystal_item_by_name("PickAxe").unwrap();
    let mut carrier =
        super::super::items::embedded_item_state_from_template(&template, ItemContainer::Bag1, 0);
    carrier.unique_id = 800_001;
    carrier.durability_current = Some(5000);
    carrier.durability_max = Some(5000);
    let pickaxe =
        super::super::equipment::equipment_state_from_item_state(&carrier, EquipmentSlot::Weapon);
    let world = session.app.world_mut();
    let mut inventory = world.resource_mut::<InventoryResource>();
    let weapon = inventory
        .equipment_items
        .iter_mut()
        .find(|item| item.slot == EquipmentSlot::Weapon)
        .unwrap();
    // A new template requires its complete exact carrier, never a key rewrite
    // over the old starter weapon's captured template index.
    *weapon = pickaxe;
    assert_eq!(
        super::super::equipment::user_item_from_equipment_state(weapon)
            .unwrap()
            .unique_id,
        800_001
    );
    drop(inventory);
    let player = player_entity(world).unwrap();
    let target = offset_point(
        &entity_position(world, player).unwrap(),
        MirDirection::Right,
        1,
    );
    let mut resource = world.resource_mut::<MiningResource>();
    let index = resource.mine_sets.len();
    resource.mine_sets.push(MineSet {
        spot_regen_rate_minutes: 5,
        max_stones: 80,
        hit_rate: 100,
        drop_rate: 100,
        total_slots: 1,
        drops: vec![MineDrop {
            item_name: "GoldOre",
            min_slot: 0,
            max_slot: 1,
            min_dura: 3,
            max_dura: 16,
            bonus_chance: 20,
            max_bonus_dura: 10,
        }],
    });
    resource.spots.insert(
        (target.x, target.y),
        MineSpot {
            mine_set_index: index,
            stones_left: 10,
            last_regen_tick: 0,
        },
    );
    target
}

#[test]
fn guaranteed_mine_uses_shared_uid_and_keeps_ore_purity_and_original_swing_cost() {
    let (mut session, allocator, _) = trusted_session("mine-good", 5_000_000);
    let target = setup(&mut session);
    let tool_uid = equipped_pickaxe(session.app.world()).unwrap().unique_id;
    assert_ne!(
        tool_uid,
        super::super::equipment::equipment_slot_unique_id(EquipmentSlot::Weapon).unwrap()
    );
    let packets = try_mine(session.app.world_mut(), MirDirection::Right).unwrap();
    let item = packets
        .iter()
        .find_map(|packet| {
            if let ServerPacket::GainedItem { item } = packet {
                Some(item)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(item.unique_id, 5_000_001);
    assert!(item.current_dura >= 3000 && item.current_dura <= 25_000);
    assert_eq!(allocator.issued_through().unwrap(), item.unique_id);
    assert_eq!(
        session.app.world().resource::<MiningResource>().spots[&(target.x, target.y)].stones_left,
        9
    );
    let worn = session
        .app
        .world()
        .resource::<InventoryResource>()
        .equipment_items
        .iter()
        .find(|item| item.slot == EquipmentSlot::Weapon)
        .unwrap();
    assert!((5..=19).contains(&(5000 - worn.durability_current)));
    assert!(packets.iter().any(
        |packet| matches!(packet, ServerPacket::DuraChanged { unique_id, current_dura }
        if *unique_id == tool_uid && *current_dura == worn.durability_current)
    ));
}

#[test]
fn failed_mine_issuance_keeps_stones_pickaxe_and_inventory() {
    let (mut session, allocator, path) = trusted_session("mine-fail", 5_000_000);
    let target = setup(&mut session);
    // Prove the equipment and complete source pass the same preflight before
    // corrupting only the real File issuer. A bad carrier must not mask this
    // failure injection before the allocator is contacted.
    assert_eq!(
        equipped_pickaxe(session.app.world()).unwrap().unique_id,
        800_001
    );
    super::super::item_uid_issuance::refresh_world_history_floor(session.app.world()).unwrap();
    assert_eq!(allocator.issued_through().unwrap(), 5_000_000);
    let before = inventory_image(&session);
    fs::write(&path, "broken").unwrap();
    let packets = try_mine(session.app.world_mut(), MirDirection::Right).unwrap();
    assert_eq!(inventory_image(&session), before);
    assert_eq!(
        session.app.world().resource::<MiningResource>().spots[&(target.x, target.y)].stones_left,
        10
    );
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::Chat { .. })));
    assert!(!packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::GainedItem { .. }
            | ServerPacket::DuraChanged { .. }
            | ServerPacket::MapEffect { .. }
    )));
    assert_eq!(fs::read(&path).unwrap(), b"broken");
    assert!(allocator.issued_through().is_err());
}

#[test]
fn contradictory_pickaxe_metadata_rejects_before_wear_payout_or_uid_issuance() {
    let (mut session, allocator, _) = trusted_session("mine-bad-carrier", 5_000_000);
    let target = setup(&mut session);
    {
        let world = session.app.world_mut();
        let mut inventory = world.resource_mut::<InventoryResource>();
        let weapon = inventory
            .equipment_items
            .iter_mut()
            .find(|item| item.slot == EquipmentSlot::Weapon)
            .unwrap();
        weapon.user_item_metadata.as_mut().unwrap().item_index =
            Some(crystal_item_by_name("BronzeHelmet").unwrap().item_index);
    }
    let before = inventory_image(&session);
    let floor = allocator.issued_through().unwrap();
    assert!(equipped_pickaxe(session.app.world()).is_none());
    assert!(try_mine(session.app.world_mut(), MirDirection::Right).is_none());
    assert_eq!(inventory_image(&session), before);
    assert_eq!(
        session.app.world().resource::<MiningResource>().spots[&(target.x, target.y)].stones_left,
        10
    );
    assert_eq!(allocator.issued_through().unwrap(), floor);
}

#[test]
fn legacy_pickaxe_without_exact_uid_keeps_equipment_slot_wear_reference() {
    let mut session = SimulationSession::new(crate::SimulationConfig::default());
    assert!(session
        .handle_packet(mir2_protocol::ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    session.handle_packet(mir2_protocol::ClientPacket::StartGame { character_index: 0 });
    assert!(session.active_identity().is_some());
    let target = setup(&mut session);
    {
        let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
        let weapon = inventory
            .equipment_items
            .iter_mut()
            .find(|item| item.slot == EquipmentSlot::Weapon)
            .unwrap();
        weapon.user_item_unique_id = None;
        weapon.user_item_metadata = None;
    }
    let legacy_uid =
        super::super::equipment::equipment_slot_unique_id(EquipmentSlot::Weapon).unwrap();
    assert_eq!(
        equipped_pickaxe(session.app.world()).unwrap().unique_id,
        legacy_uid
    );
    let packets = try_mine(session.app.world_mut(), MirDirection::Right).unwrap();
    let worn = session
        .app
        .world()
        .resource::<InventoryResource>()
        .equipment_items
        .iter()
        .find(|item| item.slot == EquipmentSlot::Weapon)
        .unwrap();
    assert!((5..=19).contains(&(5000 - worn.durability_current)));
    assert!(packets.iter().any(
        |packet| matches!(packet, ServerPacket::DuraChanged { unique_id, current_dura }
        if *unique_id == legacy_uid && *current_dura == worn.durability_current)
    ));
    assert_eq!(
        session.app.world().resource::<MiningResource>().spots[&(target.x, target.y)].stones_left,
        9
    );
}
