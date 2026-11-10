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
    let mut packets = Vec::new();
    for step in 1..=4 {
        tick_fishing_at(
            world,
            &mut packets,
            cast_at + Duration::from_millis(step * 300),
        );
    }
    assert_eq!(
        updates(&packets),
        [
            (true, 25, false),
            (true, 50, false),
            (true, 75, false),
            (true, 100, true)
        ]
    );
    assert_eq!(
        packets
            .iter()
            .filter(|packet| matches!(packet, ServerPacket::DuraChanged { .. }))
            .count(),
        1
    );
    world.resource::<FishingResource>().bite_expires_at.unwrap()
}

#[test]
fn fishing_manual_autocast_orders_terminal_then_exact_new_cast_cost() {
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
    let second = fishing_cast_at(world, false, now);
    assert!(
        matches!(
            second.as_slice(),
            [
                ServerPacket::FishingUpdate {
                    fishing: false,
                    progress_percent: 100,
                    found_fish: false,
                    ..
                },
                ServerPacket::DuraChanged { .. },
                ServerPacket::DeleteItem { count: 1, .. },
                ServerPacket::DuraChanged { .. },
                ServerPacket::FishingUpdate {
                    fishing: true,
                    progress_percent: 0,
                    found_fish: false,
                    ..
                },
            ]
        ),
        "{second:?}"
    );
    let empty = fishing_cast_at(world, false, now);
    assert_eq!(empty.len(), 1);
    assert_eq!(updates(&empty), [(false, 100, false)]);
    assert!(!world.resource::<FishingResource>().fishing);
}

#[test]
fn fishing_bite_expires_after_three_seconds_without_a_repeat_or_reward() {
    let mut session = fixture(2);
    let world = session.app.world_mut();
    let started = Instant::now();
    let deadline = bite(world, started);
    assert_eq!(
        deadline,
        started + Duration::from_millis(1200) + CRYSTAL_FISHING_BITE_WINDOW
    );
    let mut packets = Vec::new();
    tick_fishing_at(world, &mut packets, deadline);
    assert_eq!(updates(&packets), [(true, 100, true)]);
    packets.clear();
    tick_fishing_at(world, &mut packets, deadline + Duration::from_millis(1));
    assert_eq!(packets.len(), 1);
    assert_eq!(updates(&packets), [(true, 100, false)]);
    packets.clear();
    tick_fishing_at(world, &mut packets, deadline + Duration::from_millis(300));
    assert_eq!(packets.len(), 1);
    assert_eq!(updates(&packets), [(false, 100, false)]);
    assert!(world
        .resource::<FishingResource>()
        .bite_expires_at
        .is_none());
}

#[test]
fn fishing_late_reel_checks_expiry_before_next_maintenance_tick() {
    let mut session = fixture(2);
    let world = session.app.world_mut();
    let deadline = bite(world, Instant::now());
    world.resource_mut::<FishingResource>().chance_percent = 100;
    let late = fishing_cast_at(world, false, deadline + Duration::from_millis(1));
    assert_eq!(late.len(), 1, "late reel generated loot or wear: {late:?}");
    assert_eq!(updates(&late), [(false, 100, false)]);
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
    assert_eq!(updates(&packets), [(false, 100, false)]);
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
    assert_eq!(updates(&packets), [(false, 100, false)]);
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
