use std::collections::BTreeSet;

use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneCollision, ZoneCommand, ZoneJoin, ZoneKey,
    ZoneMonsterSpawn, ZoneOutbound, ZoneRuntime,
};

const PLAYER: Point = Point { x: 20, y: 20 };

fn zone(blocked: Vec<Point>) -> ZoneRuntime {
    let mut zone = ZoneRuntime::new_with_collision(
        ZoneKey::for_map("monster-movement-fixture"),
        ZoneCollision::unbounded().with_blocked_cells(blocked),
    );
    zone.handle(ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new("owner"),
        account_id: "owner-account".into(),
        character_index: 1,
        object_id: 100,
        name: "Owner".into(),
        class: MirClass::Taoist,
        gender: MirGender::Male,
        level: 7,
        hp: 1_000_000,
        max_hp: 1_000_000,
        mp: 100,
        map_file_name: "monster-movement-fixture".into(),
        position: PLAYER,
        direction: MirDirection::Down,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }));
    zone
}

fn spawn(zone: &mut ZoneRuntime, object_id: u32, x: i32, y: i32) {
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("owner"),
        monster: ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id,
            name: "Movement Fixture".into(),
            name_colour_argb: -1,
            image: 900,
            ai: 0,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: 4,
            max_hp: 20,
            hp: 20,
            experience: 6,
            move_speed_ms: 600,
            attack_speed_ms: 1_200,
            friendly_guild: None,
            position: Point { x, y },
            direction: MirDirection::Down,
            defense: Default::default(),
            respawn: None,
            drops: Vec::new(),
        },
        now_ms: 0,
    });
}

fn position(zone: &ZoneRuntime, object_id: u32) -> Point {
    zone.native_monster_snapshots()
        .into_iter()
        .find(|monster| monster.object_id == object_id)
        .expect("spawned monster")
        .position
}

fn distance(left: &Point, right: &Point) -> i32 {
    (left.x - right.x).abs().max((left.y - right.y).abs())
}

fn owner_packets(outbounds: &[ZoneOutbound]) -> impl Iterator<Item = &ServerPacket> {
    outbounds.iter().flat_map(|outbound| match outbound {
        ZoneOutbound::ToSession {
            session_id,
            packets,
        } if session_id == &SessionId::new("owner") => packets.as_slice(),
        ZoneOutbound::ToMany {
            session_ids,
            packets,
        } if session_ids.contains(&SessionId::new("owner")) => packets.as_slice(),
        ZoneOutbound::ToAll { packets } => packets.as_slice(),
        _ => &[],
    })
}

#[test]
fn blocked_chase_uses_actual_detour_direction_and_preserves_move_cooldown() {
    let mut zone = zone(vec![Point { x: 15, y: 20 }]);
    spawn(&mut zone, 9_100, 14, 20);

    let out = zone.tick(0);
    let first_position = Point { x: 15, y: 21 };
    assert_eq!(position(&zone, 9_100), first_position);
    assert!(owner_packets(&out).any(|packet| matches!(
        packet,
        ServerPacket::ObjectWalk { movement }
            if movement.object_id == 9_100
                && movement.position == first_position
                && movement.direction == MirDirection::DownRight
    )));

    for now_ms in [1, 300, 599] {
        let out = zone.tick(now_ms);
        assert_eq!(position(&zone, 9_100), first_position);
        assert!(!owner_packets(&out).any(|packet| matches!(
            packet,
            ServerPacket::ObjectWalk { movement } if movement.object_id == 9_100
        )));
    }
    zone.tick(600);
    assert_ne!(position(&zone, 9_100), first_position);
}

#[test]
fn monsters_route_around_a_wall_with_deterministic_shared_state() {
    let blocked = (18..=22).map(|y| Point { x: 16, y }).collect::<Vec<_>>();
    let mut replay = zone(blocked.clone());
    let mut zone = zone(blocked.clone());
    spawn(&mut zone, 9_100, 14, 20);
    spawn(&mut replay, 9_100, 14, 20);

    // Separate runtimes intentionally mint distinct admission namespaces.
    // Verify that this is their only identity difference before comparing
    // deterministic gameplay under each runtime's own authority.
    let owner = SessionId::new("owner");
    let primary_owner = zone.owner_health_cursor(&owner).unwrap().online_owner;
    let replay_owner = replay.owner_health_cursor(&owner).unwrap().online_owner;
    let mut primary_identity: serde_json::Value = serde_json::from_str(&primary_owner).unwrap();
    let mut replay_identity: serde_json::Value = serde_json::from_str(&replay_owner).unwrap();
    let primary_namespace = primary_identity
        .as_object_mut()
        .unwrap()
        .remove("namespace")
        .unwrap();
    let replay_namespace = replay_identity
        .as_object_mut()
        .unwrap()
        .remove("namespace")
        .unwrap();
    assert_ne!(primary_namespace, replay_namespace);
    assert_eq!(primary_identity, replay_identity);

    let mut previous = position(&zone, 9_100);
    for now_ms in (0..12).map(|tick| tick * 2_000) {
        let out = zone.tick(now_ms);
        for event in &out {
            if let ZoneOutbound::OwnerHealthChanged { change } = event {
                assert_eq!(change.cursor.online_owner, primary_owner);
            }
        }
        let replay_out = replay.tick(now_ms);
        // Normalize only a comparison clone, after proving every receipt is
        // stamped by its actual runtime. The nonce must never cross authority.
        let mut comparable_replay = replay_out.clone();
        for event in &mut comparable_replay {
            if let ZoneOutbound::OwnerHealthChanged { change } = event {
                assert_eq!(change.cursor.online_owner, replay_owner);
                change.cursor.online_owner = primary_owner.clone();
            }
        }
        assert_eq!(out, comparable_replay, "detour replay at {now_ms}");
        let current = position(&zone, 9_100);
        assert!(!blocked.contains(&current), "monster entered wall");
        assert!(distance(&previous, &current) <= 1, "monster skipped a tile");
        assert_ne!(
            current, PLAYER,
            "monster entered the player's occupied tile"
        );
        assert_eq!(current, position(&replay, 9_100));
        previous = current;
    }
    assert_eq!(
        distance(&previous, &PLAYER),
        1,
        "monster should reach attack range"
    );
}

#[test]
fn monster_escapes_l_shaped_blockers_without_repeating_two_tiles() {
    let blocked = vec![
        Point { x: 16, y: 21 },
        Point { x: 17, y: 20 },
        Point { x: 17, y: 21 },
    ];
    let mut zone = zone(blocked.clone());
    spawn(&mut zone, 9_100, 14, 20);

    let mut visited_north_side = false;
    let mut previous = position(&zone, 9_100);
    for now_ms in (0..12).map(|tick| tick * 2_000) {
        zone.tick(now_ms);
        let current = position(&zone, 9_100);
        assert!(!blocked.contains(&current));
        assert!(distance(&previous, &current) <= 1);
        assert_ne!(current, PLAYER);
        visited_north_side |= current.y < 20;
        previous = current;
    }
    assert!(
        visited_north_side,
        "reachable north-side detour was ignored"
    );
    assert_eq!(distance(&previous, &PLAYER), 1, "chase remained in a loop");
}

#[test]
fn monster_keeps_retreat_detour_out_of_a_half_enclosed_pocket() {
    let blocked = vec![
        Point { x: 15, y: 19 },
        Point { x: 15, y: 20 },
        Point { x: 15, y: 21 },
        Point { x: 14, y: 19 },
        Point { x: 14, y: 21 },
    ];
    let mut zone = zone(blocked.clone());
    spawn(&mut zone, 9_100, 14, 20);

    let mut retreated = false;
    let mut previous = position(&zone, 9_100);
    for now_ms in (0..12).map(|tick| tick * 2_000) {
        zone.tick(now_ms);
        let current = position(&zone, 9_100);
        assert!(!blocked.contains(&current));
        assert!(distance(&previous, &current) <= 1);
        assert_ne!(current, PLAYER);
        retreated |= current.x < 14;
        previous = current;
    }
    assert!(
        retreated,
        "monster must first retreat through the west opening"
    );
    assert_eq!(distance(&previous, &PLAYER), 1, "retreat detour was undone");
}

#[test]
fn owned_summon_and_monster_pet_target_both_detour_around_blockers() {
    let mut blocked = (18..=22).map(|y| Point { x: 22, y }).collect::<Vec<_>>();
    blocked.push(Point { x: 25, y: 20 });
    let mut zone = zone(blocked.clone());
    zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("owner"),
        object_id: 0,
        spell: Spell::SummonSkeleton,
        direction: MirDirection::Right,
        target: Point { x: 21, y: 20 },
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1_000,
        now_ms: 10,
    });
    let birth = zone.tick(510);
    let pet = zone
        .native_monster_snapshots()
        .into_iter()
        .find(|monster| !monster.hostile_to_player)
        .expect("real owned skeleton summon");
    assert_eq!(pet.position, Point { x: 21, y: 20 });
    assert!(owner_packets(&birth).any(|packet| matches!(packet,
        ServerPacket::ObjectMonster { info }
            if info.object_id == pet.object_id && info.master_object_id == 100)));

    // The pet is nearer than its owner, so AI0 uses the typed entity-target
    // pursuit path. Its direct left step is blocked by a static map cell.
    spawn(&mut zone, 9_100, 26, 20);
    zone.tick(511);
    assert_eq!(position(&zone, 9_100), Point { x: 25, y: 19 });

    // Establish an actual accepted owner attack in Focus mode. Merely spawning
    // a nearby monster does not authorize a pet attack, and an idle summon
    // already beside its owner legitimately has no reason to move.
    let owner = SessionId::new("owner");
    let mut profile = zone.player_chat_profile(&owner).unwrap();
    profile.pet_mode = 4;
    zone.handle(ZoneCommand::UpdateChatProfile {
        session_id: owner.clone(),
        profile,
    });
    zone.handle(ZoneCommand::SyncPlayerTransform {
        session_id: owner.clone(),
        position: Point { x: 25, y: 18 },
        direction: MirDirection::Down,
    });
    // Join/render state grants no combat authority. Mirror the owner's real
    // class/life admission through the public trusted Zone command first.
    zone.handle(ZoneCommand::sync_player_combat_state(
        owner.clone(),
        MirClass::Taoist,
        false,
        false,
        false,
        false,
        false,
        false,
    ));
    let attack = zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: owner.clone(),
        object_id: 9_100,
        direction: MirDirection::Down,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 1,
        now_ms: 1_500,
    });
    assert!(
        owner_packets(&attack).any(|packet| matches!(packet,
        ServerPacket::ObjectAttack { info } if info.object_id == 100)),
        "focused owner attack must be admitted: {attack:?}"
    );
    zone.handle(ZoneCommand::SyncPlayerTransform {
        session_id: owner,
        position: PLAYER,
        direction: MirDirection::Down,
    });

    // Both pursuit paths remain collision/occupancy bounded. Check actual pet
    // walks without requiring a particular valid rotation around the wall.
    let mut previous = pet.position.clone();
    let mut pet_walked = false;
    for now_ms in [1_501, 2_101, 3_101] {
        let out = zone.tick(now_ms);
        let pet_position = position(&zone, pet.object_id);
        assert!(!blocked.contains(&pet_position));
        assert_ne!(pet_position, PLAYER);
        assert!(
            distance(&previous, &pet_position) <= 1,
            "pet skipped a tile"
        );
        pet_walked |= owner_packets(&out).any(|packet| {
            matches!(packet,
            ServerPacket::ObjectWalk { movement } if movement.object_id == pet.object_id)
        });
        if let Some(target) = zone
            .native_monster_snapshots()
            .iter()
            .find(|m| m.object_id == 9_100 && !m.dead)
        {
            assert_ne!(
                pet_position, target.position,
                "pet entered its target's occupied tile"
            );
        }
        previous = pet_position;
    }
    assert!(
        pet_walked,
        "a focused summon must actually pursue around its wall"
    );
    assert_ne!(position(&zone, pet.object_id), pet.position);
}

#[test]
fn monsters_detour_around_each_other_to_fill_the_players_eight_adjacent_tiles() {
    let mut zone = zone(Vec::new());
    for offset in 0..8 {
        spawn(&mut zone, 9_100 + offset, 14, 17 + offset as i32);
    }
    assert_eq!(zone.native_monster_count(), 8);

    for now_ms in (0..20).map(|tick| tick * 2_000) {
        zone.tick(now_ms);
        let monsters = zone.native_monster_snapshots();
        let occupied = monsters
            .iter()
            .map(|monster| (monster.position.x, monster.position.y))
            .collect::<BTreeSet<_>>();
        assert_eq!(occupied.len(), 8, "monsters overlapped at {now_ms}");
        assert!(!occupied.contains(&(PLAYER.x, PLAYER.y)));
    }
    assert!(zone
        .native_monster_snapshots()
        .iter()
        .all(|monster| distance(&monster.position, &PLAYER) == 1));
}

#[test]
fn fully_blocked_monster_stays_in_place_without_crossing_collision() {
    let origin = Point { x: 14, y: 20 };
    let blocked = (-1..=1)
        .flat_map(|dx| (-1..=1).map(move |dy| (dx, dy)))
        .filter(|&(dx, dy)| dx != 0 || dy != 0)
        .map(|(dx, dy)| Point {
            x: origin.x + dx,
            y: origin.y + dy,
        })
        .collect();
    let mut zone = zone(blocked);
    spawn(&mut zone, 9_100, origin.x, origin.y);

    for now_ms in [0, 599, 600, 1_200, 2_400] {
        let out = zone.tick(now_ms);
        assert_eq!(position(&zone, 9_100), origin);
        assert!(!owner_packets(&out).any(|packet| matches!(
            packet,
            ServerPacket::ObjectWalk { movement } if movement.object_id == 9_100
        )));
    }
}
