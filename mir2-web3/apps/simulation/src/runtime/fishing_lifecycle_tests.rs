use super::*;
use crate::runtime::components::PlayerVitals;
use crate::runtime::item_grants::tests::{plain, trusted_session};
use crate::SimulationSession;
use mir2_protocol::{ClientPacket, MirDirection};

const SHORE: Point = Point { x: 342, y: 244 };
const WATER: Point = Point { x: 339, y: 247 };

fn fixture(bait_count: u32) -> SimulationSession {
    let (mut session, _, _) = trusted_session("fishing-lifecycle", 6_000_000);
    let grants = [
        plain("BlueFishingRod", 1, ItemContainer::Bag1),
        plain("FishingHook", 1, ItemContainer::Bag1),
        plain("FishingFloat", 1, ItemContainer::Bag1),
        plain("FishBait", bait_count, ItemContainer::Bag1),
        plain("FishDetector", 1, ItemContainer::Bag1),
        plain("FishingReel", 1, ItemContainer::Bag1),
    ];
    let world = session.app.world_mut();
    {
        let mut config = world.resource_mut::<RuntimeConfigResource>();
        config.config.crystal_fishing_attempts = std::num::NonZeroU16::new(30).unwrap();
        config.config.crystal_fishing_delay_ms = 0;
    }
    let items = publish_world_grants(world, &grants).unwrap();
    let item = |name: &str| items.iter().find(|item| item.name == name).unwrap();
    let rod = item("BlueFishingRod");
    let mut rod_user_item = user_item_from_item_state(rod);
    for (slot, name) in [
        (FISHING_SLOT_HOOK, "FishingHook"),
        (FISHING_SLOT_FLOAT, "FishingFloat"),
        (FISHING_SLOT_BAIT, "FishBait"),
        (FISHING_SLOT_FINDER, "FishDetector"),
        (FISHING_SLOT_REEL, "FishingReel"),
    ] {
        rod_user_item.slots[slot] = Some(user_item_from_item_state(item(name)));
    }
    let rod = try_item_state_from_user_item(rod.clone(), &rod_user_item).unwrap();
    let mut inventory = world.resource_mut::<InventoryResource>();
    inventory.inventory_items.clear();
    inventory
        .equipment_items
        .retain(|item| item.slot != EquipmentSlot::Weapon);
    inventory
        .equipment_items
        .push(equipment_state_from_item_state(&rod, EquipmentSlot::Weapon));
    drop(inventory);
    session.force_authoritative_player_transform(SHORE, MirDirection::DownLeft);
    session
}

fn updates(packets: &[ServerPacket]) -> Vec<(bool, i32, bool)> {
    packets
        .iter()
        .filter_map(|packet| match packet {
            ServerPacket::FishingUpdate {
                fishing,
                progress_percent,
                found_fish,
                ..
            } => Some((*fishing, *progress_percent, *found_fish)),
            _ => None,
        })
        .collect()
}

fn bite(world: &mut World, cast_at: Instant) -> Instant {
    assert_eq!(
        updates(&fishing_cast_at(world, true, cast_at)),
        [(true, 0, false)]
    );
    world.resource_mut::<FishingResource>().nibble_chance = 100;
    let packets = step(world, cast_at + crate::CRYSTAL_FISHING_CAST_DELAY);
    assert_eq!(updates(&packets), [(true, 3, true)]);
    assert_eq!(
        packets
            .iter()
            .filter(|packet| matches!(packet, ServerPacket::DuraChanged { .. }))
            .count(),
        1
    );
    world.resource::<FishingResource>().bite_expires_at.unwrap()
}

fn step(world: &mut World, at: Instant) -> Vec<ServerPacket> {
    let mut packets = Vec::new();
    tick_fishing_at(world, &mut packets, at);
    packets
}

fn equipped_rod(world: &World) -> UserItem {
    equipped_fishing_slot_carrier(world).unwrap().2
}

fn durability_receipts(packets: &[ServerPacket]) -> Vec<(u64, u16)> {
    packets
        .iter()
        .filter_map(|packet| match packet {
            ServerPacket::DuraChanged {
                unique_id,
                current_dura,
            } => Some((*unique_id, *current_dura)),
            _ => None,
        })
        .collect()
}

#[test]
fn fishing_manual_reel_cancels_recast_without_spending_a_second_bait() {
    let mut session = fixture(2);
    let world = session.app.world_mut();
    let now = Instant::now();
    fishing_change_autocast_impl(world, true);
    assert_eq!(
        updates(&fishing_cast_at(world, true, now)),
        [(true, 0, false)]
    );
    assert!(
        fishing_cast_at(world, true, now).is_empty(),
        "duplicate cast spent again"
    );
    let hook = equipped_rod(world).slots[FISHING_SLOT_HOOK]
        .as_ref()
        .unwrap()
        .unique_id;
    let packets = fishing_cast_at(world, false, now);
    assert_eq!(packets.len(), 2);
    assert_eq!(durability_receipts(&packets), [(hook, 7_998)]);
    assert_eq!(updates(&packets), [(false, 0, false)]);
    let repeated = fishing_cast_at(world, false, now);
    assert_eq!(repeated.len(), 1);
    assert_eq!(updates(&repeated), [(false, 0, false)]);
    let fishing = world.resource::<FishingResource>();
    assert!(!fishing.fishing && fishing.auto_cast && fishing.next_step_at.is_none());
    assert_eq!(
        fishing.slot_items[FISHING_SLOT_BAIT]
            .as_ref()
            .unwrap()
            .quantity,
        1
    );
    assert_eq!(fishing.chance_counter, 0);
}

#[test]
fn fishing_source_progress_uses_cast_delay_attempts_and_exhaustion_order() {
    let mut session = fixture(2);
    let world = session.app.world_mut();
    world
        .resource_mut::<RuntimeConfigResource>()
        .config
        .crystal_fishing_delay_ms = 350;
    fishing_change_autocast_impl(world, true);
    let rod = equipped_rod(world);
    let hook = rod.slots[FISHING_SLOT_HOOK].as_ref().unwrap().unique_id;
    let finder = rod.slots[FISHING_SLOT_FINDER].as_ref().unwrap().unique_id;
    let cast_at = Instant::now();
    assert_eq!(
        updates(&fishing_cast_at(world, true, cast_at)),
        [(true, 0, false)]
    );
    let first_at = cast_at + Duration::from_millis(1100);
    world.resource_mut::<FishingResource>().nibble_chance = -1;
    for attempt in 1..=30 {
        let at = first_at + CRYSTAL_FISHING_STEP * (attempt - 1);
        assert!(step(world, at - Duration::from_millis(1)).is_empty());
        assert_eq!(
            updates(&step(world, at)),
            [(true, (attempt * 100 / 30) as i32, false)]
        );
        assert!(
            step(world, at).is_empty(),
            "duplicate timer advanced attempt {attempt}"
        );
    }
    assert_eq!(world.resource::<FishingResource>().chance_counter, 0);
    let exhausted_at = first_at + CRYSTAL_FISHING_STEP * 30;
    let packets = step(world, exhausted_at);
    assert_eq!(
        updates(&packets),
        [(true, 103, false), (false, 103, false), (true, 0, false)]
    );
    assert_eq!(
        durability_receipts(&packets),
        [
            (hook, 7_998),
            (rod.unique_id, 9_998),
            (hook, 7_997),
            (finder, 4_999)
        ]
    );
    assert!(
        matches!(
            packets.as_slice(),
            [
                ServerPacket::FishingUpdate {
                    fishing: true,
                    progress_percent: 103,
                    ..
                },
                ServerPacket::DuraChanged { .. },
                ServerPacket::FishingUpdate { fishing: false, .. },
                ServerPacket::DuraChanged { .. },
                ServerPacket::DuraChanged { .. },
                ServerPacket::DuraChanged { .. },
                ServerPacket::DeleteItem { count: 1, .. },
                ServerPacket::FishingUpdate {
                    fishing: true,
                    progress_percent: 0,
                    ..
                },
            ]
        ),
        "{packets:?}"
    );
    let fishing = world.resource::<FishingResource>();
    assert_eq!(fishing.attempts, 0);
    assert_eq!(fishing.chance_counter, 1);
    assert!((48..=50).contains(&fishing.chance_percent));
    assert_eq!(
        fishing.next_step_at,
        Some(exhausted_at + Duration::from_millis(1100))
    );
    let reel = fishing_cast_at(world, false, exhausted_at);
    assert_eq!(durability_receipts(&reel), [(hook, 7_996), (finder, 4_998)]);
    assert_eq!(updates(&reel), [(false, 0, false)]);
}

#[test]
fn fishing_hook_and_retry_finder_delete_on_wear_after_zero() {
    let mut session = fixture(2);
    let world = session.app.world_mut();
    let rod = equipped_rod(world);
    let hook = rod.slots[FISHING_SLOT_HOOK].as_ref().unwrap().unique_id;
    let finder = rod.slots[FISHING_SLOT_FINDER].as_ref().unwrap().unique_id;
    sync_fishing_slots_from_equipment(world);
    {
        let mut fishing = world.resource_mut::<FishingResource>();
        fishing.chance_counter = 1;
        for slot in [FISHING_SLOT_HOOK, FISHING_SLOT_FINDER] {
            fishing.slot_items[slot]
                .as_mut()
                .unwrap()
                .durability_current = Some(1);
        }
    }
    assert!(persist_fishing_slots_to_equipment(world));
    let now = Instant::now();
    let cast = fishing_cast_at(world, true, now);
    assert_eq!(
        durability_receipts(&cast),
        [(rod.unique_id, 9_999), (hook, 0), (finder, 0)]
    );
    let zero = equipped_rod(world);
    assert_eq!(
        zero.slots[FISHING_SLOT_HOOK].as_ref().unwrap().current_dura,
        0
    );
    assert_eq!(
        zero.slots[FISHING_SLOT_FINDER]
            .as_ref()
            .unwrap()
            .current_dura,
        0
    );
    assert!(fishing_cast_at(world, true, now).is_empty());
    let reel = fishing_cast_at(world, false, now);
    assert_eq!(
        reel,
        [
            ServerPacket::DeleteItem {
                unique_id: hook,
                count: 1
            },
            ServerPacket::DeleteItem {
                unique_id: finder,
                count: 1
            },
            fishing_update_packet(world),
        ],
    );
    let finished = equipped_rod(world);
    assert!(finished.slots[FISHING_SLOT_HOOK].is_none());
    assert!(finished.slots[FISHING_SLOT_FINDER].is_none());
    assert_eq!(finished.slots[FISHING_SLOT_BAIT].as_ref().unwrap().count, 1);
    assert_eq!(finished.current_dura, 9_999);
    assert_eq!(
        fishing_cast_at(world, false, now),
        [fishing_update_packet(world)]
    );
}

#[test]
fn fishing_source_wears_tackle_before_reporting_missing_bait() {
    let mut session = fixture(1);
    let world = session.app.world_mut();
    sync_fishing_slots_from_equipment(world);
    {
        let mut fishing = world.resource_mut::<FishingResource>();
        fishing.chance_counter = 1;
        fishing.slot_items[FISHING_SLOT_BAIT] = None;
    }
    assert!(persist_fishing_slots_to_equipment(world));
    let rod = equipped_rod(world);
    let hook = rod.slots[FISHING_SLOT_HOOK].as_ref().unwrap().unique_id;
    let finder = rod.slots[FISHING_SLOT_FINDER].as_ref().unwrap().unique_id;
    let packets = fishing_cast_at(world, true, Instant::now());
    assert_eq!(
        durability_receipts(&packets),
        [(rod.unique_id, 9_999), (hook, 7_999), (finder, 4_999)]
    );
    assert!(matches!(packets.last(), Some(ServerPacket::Chat { .. })));
    assert!(updates(&packets).is_empty());
    assert!(!world.resource::<FishingResource>().fishing);
}

#[test]
fn fishing_last_rod_point_uses_the_pre_wear_cast_stat() {
    let mut session = fixture(2);
    let world = session.app.world_mut();
    let rod_id = equipped_rod(world).unique_id;
    world
        .resource_mut::<InventoryResource>()
        .equipment_items
        .iter_mut()
        .find(|item| item.slot == EquipmentSlot::Weapon)
        .unwrap()
        .durability_current = 1;
    let now = Instant::now();
    let packets = fishing_cast_at(world, true, now);
    assert_eq!(
        durability_receipts(&packets),
        [
            (rod_id, 0),
            (
                equipped_rod(world).slots[FISHING_SLOT_HOOK]
                    .as_ref()
                    .unwrap()
                    .unique_id,
                7_999
            )
        ]
    );
    assert!(matches!(
        packets.last(),
        Some(ServerPacket::FishingUpdate {
            fishing: true,
            chance_percent: 38,
            ..
        })
    ));
    let interrupted = step(world, now + crate::CRYSTAL_FISHING_CAST_DELAY);
    assert_eq!(updates(&interrupted), [(false, 0, false)]);
    assert_eq!(interrupted.len(), 1);
}

#[test]
fn fishing_bite_expires_after_three_seconds_without_a_second_nibble() {
    let mut session = fixture(2);
    let world = session.app.world_mut();
    let started = Instant::now();
    let deadline = bite(world, started);
    assert_eq!(
        deadline,
        started + crate::CRYSTAL_FISHING_CAST_DELAY + CRYSTAL_FISHING_BITE_WINDOW
    );
    let first_at = started + crate::CRYSTAL_FISHING_CAST_DELAY;
    for attempt in 2..=31 {
        let at = first_at + CRYSTAL_FISHING_STEP * (attempt - 1);
        let packets = step(world, at);
        let progress = (attempt * 100 / 30) as i32;
        assert_eq!(updates(&packets)[0], (true, progress, at <= deadline));
        assert_eq!(packets.len(), if attempt == 31 { 3 } else { 1 });
    }
    let fishing = world.resource::<FishingResource>();
    assert!(!fishing.fishing && fishing.bite_expires_at.is_none());
    assert_eq!(fishing.chance_counter, 1);
}

#[test]
fn fishing_reel_chance_is_independent_of_the_autocast_switch() {
    for auto_cast in [false, true] {
        let mut session = fixture(2);
        let world = session.app.world_mut();
        fishing_change_autocast_impl(world, auto_cast);
        let started = Instant::now();
        bite(world, started);
        // Stock reels have no MaxMAC. Give the active cast the same rate a
        // real template with MaxMAC 100 would produce.
        assert_eq!(world.resource::<FishingResource>().auto_reel_chance, 0);
        {
            let mut fishing = world.resource_mut::<FishingResource>();
            fishing.auto_reel_chance = 100;
            fishing.chance_percent = 100;
        }
        let next_at = started + crate::CRYSTAL_FISHING_CAST_DELAY + CRYSTAL_FISHING_STEP;
        let packets = step(world, next_at);
        let mut expected = vec![(false, 6, false)];
        if auto_cast {
            expected.extend([(true, 0, false), (true, 0, false)]);
        }
        assert_eq!(updates(&packets), expected);
        assert_eq!(
            world.resource::<FishingResource>().next_step_at,
            auto_cast.then_some(next_at + CRYSTAL_FISHING_STEP)
        );
    }
}

#[test]
fn fishing_zero_reel_can_retry_a_miss_until_its_next_successful_wear() {
    let mut session = fixture(4);
    let world = session.app.world_mut();
    sync_fishing_slots_from_equipment(world);
    world.resource_mut::<FishingResource>().slot_items[FISHING_SLOT_REEL]
        .as_mut()
        .unwrap()
        .durability_current = Some(1);
    assert!(persist_fishing_slots_to_equipment(world));
    let reel = equipped_rod(world).slots[FISHING_SLOT_REEL]
        .as_ref()
        .unwrap()
        .unique_id;
    fishing_change_autocast_impl(world, true);
    let start = Instant::now();
    bite(world, start);
    world.resource_mut::<FishingResource>().chance_percent = 100;
    let mut first = Vec::new();
    assert!(!finish_fishing_cast(
        world,
        start + crate::CRYSTAL_FISHING_CAST_DELAY,
        3,
        true,
        &mut first
    ));
    assert!(durability_receipts(&first).contains(&(reel, 0)));
    assert_eq!(updates(&first), [(false, 3, false)]);
    assert!(world.resource::<FishingResource>().auto_cast);
    assert_eq!(
        equipped_rod(world).slots[FISHING_SLOT_REEL]
            .as_ref()
            .unwrap()
            .current_dura,
        0
    );

    world
        .resource_mut::<RuntimeConfigResource>()
        .config
        .crystal_fishing_attempts = std::num::NonZeroU16::new(1).unwrap();
    let second = start + Duration::from_secs(10);
    assert_eq!(
        updates(&fishing_cast_at(world, true, second)),
        [(true, 0, false)]
    );
    world.resource_mut::<FishingResource>().nibble_chance = -1;
    let first_step = second + crate::CRYSTAL_FISHING_CAST_DELAY;
    assert_eq!(updates(&step(world, first_step)), [(true, 100, false)]);
    assert_eq!(
        updates(&step(world, first_step + CRYSTAL_FISHING_STEP)),
        [(true, 200, false), (false, 200, false), (true, 0, false)]
    );
    let third_step = world.resource::<FishingResource>().next_step_at.unwrap();
    world.resource_mut::<FishingResource>().nibble_chance = 100;
    assert_eq!(updates(&step(world, third_step)), [(true, 100, true)]);
    world.resource_mut::<FishingResource>().chance_percent = 100;
    let mut last = Vec::new();
    assert!(!finish_fishing_cast(
        world, third_step, 100, true, &mut last
    ));
    assert_eq!(updates(&last), [(false, 100, false)]);
    assert_eq!(
        last.iter()
            .filter(|packet| matches!(packet,
                ServerPacket::DeleteItem { unique_id, count: 1 } if *unique_id == reel
            ))
            .count(),
        1
    );
    assert!(equipped_rod(world).slots[FISHING_SLOT_REEL].is_none());
    assert!(fishing_change_autocast_impl(world, false).is_empty());
    assert!(!world.resource::<FishingResource>().auto_cast);
}

#[test]
fn fishing_disabling_without_a_rod_retires_the_existing_preference() {
    let mut session = fixture(2);
    let world = session.app.world_mut();
    fishing_change_autocast_impl(world, true);
    assert!(world.resource::<FishingResource>().auto_cast);
    world
        .resource_mut::<InventoryResource>()
        .equipment_items
        .retain(|item| item.slot != EquipmentSlot::Weapon);
    assert!(fishing_change_autocast_impl(world, false).is_empty());
    assert!(fishing_change_autocast_impl(world, true).is_empty());
    assert!(!world.resource::<FishingResource>().auto_cast);
}

#[test]
fn fishing_delayed_host_wake_advances_one_attempt_without_advancing_world_time() {
    let mut session = fixture(2);
    let cast_at = Instant::now() - Duration::from_secs(2);
    let world = session.app.world_mut();
    assert_eq!(
        updates(&fishing_cast_at(world, true, cast_at)),
        [(true, 0, false)]
    );
    world.resource_mut::<FishingResource>().nibble_chance = -1;
    world
        .resource_mut::<RuntimeConfigResource>()
        .config
        .crystal_fishing_attempts = std::num::NonZeroU16::new(4).unwrap();
    let world_tick = session.current_tick();
    assert_eq!(updates(&session.tick_fishing()), [(true, 25, false)]);
    assert!(session.tick_fishing().is_empty());
    assert_eq!(session.current_tick(), world_tick);
    assert!(
        crate::validate_production_player_command(true, &crate::WorldCommand::TickFishing).is_err()
    );
}

#[test]
fn fishing_late_reel_checks_expiry_before_next_maintenance_tick() {
    let mut session = fixture(2);
    let world = session.app.world_mut();
    let deadline = bite(world, Instant::now());
    world.resource_mut::<FishingResource>().chance_percent = 100;
    let hook = equipped_rod(world).slots[FISHING_SLOT_HOOK]
        .as_ref()
        .unwrap()
        .unique_id;
    let late = fishing_cast_at(world, false, deadline + Duration::from_millis(1));
    assert_eq!(
        late.len(),
        2,
        "late reel generated more than hook wear and terminal: {late:?}"
    );
    assert_eq!(durability_receipts(&late), [(hook, 7_998)]);
    assert_eq!(updates(&late), [(false, 3, false)]);
}

#[test]
fn fishing_same_map_relocation_and_owner_logout_retire_the_cast() {
    let mut session = fixture(3);
    let world = session.app.world_mut();
    let now = Instant::now();
    fishing_change_autocast_impl(world, true);
    assert_eq!(
        updates(&fishing_cast_at(world, true, now)),
        [(true, 0, false)]
    );
    let map = world.resource::<MapRuntimeResource>().current_map.clone();
    let packets = super::super::map::relocate_player_to_map(
        world,
        map,
        Point { x: 346, y: 244 },
        MirDirection::DownLeft,
        None,
    );
    assert!(matches!(
        packets.first(),
        Some(ServerPacket::MapInformation { .. })
    ));
    assert!(packets.iter().any(|packet| matches!(packet,
        ServerPacket::FishingUpdate { fishing: false, fishing_point, found_fish: false, .. } if *fishing_point == WATER
    )));
    assert!(world.resource::<FishingResource>().auto_cast);
    session.force_authoritative_player_transform(SHORE, MirDirection::DownLeft);
    assert_eq!(
        updates(&fishing_cast_at(session.app.world_mut(), true, now)),
        [(true, 0, false)]
    );
    assert!(session
        .handle_packet(ClientPacket::LogOut)
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })));
    let fishing = session.app.world().resource::<FishingResource>();
    assert!(!fishing.fishing && !fishing.auto_cast && fishing.bite_expires_at.is_none());
    assert!(session
        .handle_packet(ClientPacket::StartGame { character_index: 0 })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    assert!(!session.app.world().resource::<FishingResource>().fishing);
}

#[test]
fn fishing_cross_map_relocation_cancels_at_the_old_cast_point() {
    let mut session = fixture(2);
    let world = session.app.world_mut();
    bite(world, Instant::now());
    let next_map = super::super::npc_script::crystal_npc_move_map_information(world, "1");
    let packets = super::super::map::relocate_player_to_map(
        world,
        next_map,
        Point { x: 244, y: 406 },
        MirDirection::Up,
        None,
    );
    assert!(
        matches!(packets.first(), Some(ServerPacket::MapInformation { info }) if info.file_name == "1")
    );
    assert_eq!(updates(&packets), [(false, 3, false)]);
    assert!(packets.iter().any(|packet| matches!(packet,
        ServerPacket::FishingUpdate { fishing_point, .. } if *fishing_point == WATER
    )));
    let fishing = world.resource::<FishingResource>();
    assert!(!fishing.fishing && fishing.bite_expires_at.is_none());
}

#[test]
fn fishing_town_revive_cancels_live_nibble_without_recast_or_reward() {
    let mut session = fixture(2);
    let world = session.app.world_mut();
    fishing_change_autocast_impl(world, true);
    bite(world, Instant::now());
    let player = player_entity(world).unwrap();
    world
        .entity_mut(player)
        .get_mut::<PlayerVitals>()
        .unwrap()
        .hp = 0;
    let packets = super::super::movement::town_revive_packets(world);
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::Revived)));
    assert_eq!(updates(&packets), [(false, 3, false)]);
    assert!(!packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::GainedItem { .. }
            | ServerPacket::GainedGold { .. }
            | ServerPacket::DuraChanged { .. }
            | ServerPacket::DeleteItem { .. }
    )));
    let fishing = world.resource::<FishingResource>();
    assert!(!fishing.fishing && fishing.auto_cast && fishing.bite_expires_at.is_none());
}
