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
    for _ in 0..2 {
        let packets = fishing_cast_at(world, false, now);
        assert_eq!(packets.len(), 1);
        assert_eq!(updates(&packets), [(false, 0, false)]);
    }
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
    assert!(
        matches!(
            packets.as_slice(),
            [
                ServerPacket::FishingUpdate {
                    fishing: true,
                    progress_percent: 103,
                    ..
                },
                ServerPacket::FishingUpdate { fishing: false, .. },
                ServerPacket::DuraChanged { .. },
                ServerPacket::DeleteItem { count: 1, .. },
                ServerPacket::DuraChanged { .. },
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
    assert_eq!(fishing.chance_percent, 48);
    assert_eq!(
        fishing.next_step_at,
        Some(exhausted_at + Duration::from_millis(1100))
    );
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
        assert_eq!(packets.len(), if attempt == 31 { 2 } else { 1 });
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
    let late = fishing_cast_at(world, false, deadline + Duration::from_millis(1));
    assert_eq!(late.len(), 1, "late reel generated loot or wear: {late:?}");
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
