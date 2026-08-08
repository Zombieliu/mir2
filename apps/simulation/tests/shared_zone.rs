use std::collections::BTreeSet;

use mir2_protocol::{
    ChatItem, ChatType, ClientBuff, ClientPacket, MirClass, MirDirection, MirGender, MirGridType,
    MonsterInfo, NpcInfo, ObjectAttackInfo, ObjectDiedInfo, ObjectEffectInfo, ObjectGoldInfo,
    ObjectHealthInfo, ObjectItemInfo, ObjectManaInfo, ObjectMovement, ObjectPlayerInfo,
    ObjectRangeAttackInfo, ObjectRevivedInfo, ObjectSpellInfo, ObjectStruckInfo, Point,
    ServerPacket, Spell, UserItemStat, UserLocation,
};
use mir2_simulation::{
    GroundDropLootSnapshot, GroundDropSnapshot, SessionId, SimulationConfig, SimulationSession,
    WorldEntityKind, ZoneCollision, ZoneCommand, ZoneJoin, ZoneKey, ZoneMonsterDefense,
    ZoneMonsterSpawn, ZoneOutbound, ZonePlayerCombatStats, ZoneRuntime,
};

fn session(value: &str) -> SessionId {
    SessionId::new(value)
}

fn join(session_id: &str, object_id: u32, name: &str, x: i32, y: i32) -> ZoneJoin {
    ZoneJoin {
        session_id: session(session_id),
        account_id: format!("{session_id}-account"),
        character_index: object_id as i32,
        object_id,
        name: name.to_string(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 7,
        hp: 60,
        max_hp: 60,
        mp: 100,
        map_file_name: "0".to_string(),
        position: Point { x, y },
        direction: MirDirection::Down,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }
}

fn join_with_profile(
    session_id: &str,
    object_id: u32,
    name: &str,
    x: i32,
    y: i32,
    configure: impl FnOnce(&mut mir2_simulation::ZoneChatProfile),
) -> ZoneJoin {
    let mut join = join(session_id, object_id, name, x, y);
    configure(&mut join.chat_profile);
    join
}

fn zone() -> ZoneRuntime {
    ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded())
}

fn monster_spawn_packet(object_id: u32, master_object_id: u32, x: i32, y: i32) -> ServerPacket {
    ServerPacket::ObjectMonster {
        info: MonsterInfo {
            object_id,
            name: "Shinsu".to_string(),
            name_colour_argb: -1,
            location: Point { x, y },
            image: 33,
            direction: MirDirection::Down,
            effect: 0,
            ai: 6,
            light: 0,
            dead: false,
            skeleton: false,
            poison: 0,
            hidden: false,
            shock_time: 0,
            binding_shot_center: false,
            extra: false,
            extra_byte: 0,
            master_object_id,
            rarity: 0,
            buffs: Vec::new(),
        },
    }
}

fn hero_spawn_packet(object_id: u32, owner_name: &str, x: i32, y: i32) -> ServerPacket {
    ServerPacket::ObjectHero {
        info: ObjectPlayerInfo {
            object_id,
            name: format!("{owner_name}Hero"),
            guild_name: String::new(),
            guild_rank_name: String::new(),
            name_colour_argb: -1,
            class: MirClass::Taoist,
            gender: MirGender::Female,
            level: 7,
            location: Point { x, y },
            direction: MirDirection::Left,
            hair: 0,
            light: 0,
            weapon: -1,
            weapon_effect: 0,
            armour: -1,
            poison: 0,
            dead: false,
            hidden: false,
            effect: 0,
            wing_effect: 0,
            extra: false,
            mount_type: -1,
            riding_mount: false,
            fishing: false,
            transform_type: 0,
            element_orb_effect: 0,
            element_orb_level: 0,
            element_orb_max: 0,
            buffs: Vec::new(),
            level_effects: 0,
        },
        owner_name: owner_name.to_string(),
    }
}

fn npc_spawn_packet(object_id: u32, x: i32, y: i32) -> ServerPacket {
    ServerPacket::ObjectNpc {
        info: NpcInfo {
            object_id,
            name: "Village Guide".to_string(),
            name_colour_argb: -1,
            image: 12,
            colour_argb: -1,
            location: Point { x, y },
            direction: MirDirection::Down,
            quest_ids: vec![1, 2],
        },
    }
}

fn gold_drop(
    object_id: u32,
    x: i32,
    y: i32,
    owner_object_id: Option<u32>,
    ownership_remaining_ticks: Option<u64>,
) -> GroundDropSnapshot {
    GroundDropSnapshot {
        object_id,
        name: "Gold".to_string(),
        name_colour_argb: -1,
        icon: 0,
        x,
        y,
        quantity: 1,
        source_monster: "Deer".to_string(),
        owner_object_id,
        ownership_remaining_ticks,
        loot: GroundDropLootSnapshot::Gold { amount: 25 },
    }
}

fn native_monster_spawn(object_id: u32, x: i32, y: i32) -> ZoneMonsterSpawn {
    ZoneMonsterSpawn {
        object_id,
        name: "Field Wasp".to_string(),
        name_colour_argb: -1,
        image: 900,
        ai: 0,
        level: 4,
        max_hp: 20,
        hp: 20,
        experience: 6,
        friendly_guild: None,
        position: Point { x, y },
        direction: MirDirection::Down,
        defense: Default::default(),
        drops: vec![GroundDropSnapshot {
            object_id: 9200,
            name: "Wasp Gold".to_string(),
            name_colour_argb: -1,
            icon: 0,
            x: 0,
            y: 0,
            quantity: 8,
            source_monster: String::new(),
            owner_object_id: None,
            ownership_remaining_ticks: None,
            loot: GroundDropLootSnapshot::Gold { amount: 8 },
        }],
    }
}

fn native_neutral_monster_spawn(
    object_id: u32,
    name: &str,
    ai: u8,
    x: i32,
    y: i32,
) -> ZoneMonsterSpawn {
    let mut spawn = native_monster_spawn(object_id, x, y);
    spawn.name = name.to_string();
    spawn.ai = ai;
    spawn
}

/// A join carrying an authoritative combat stat block so the zone — not the
/// caller — rolls/validates the player's damage.
fn join_with_combat_stats(
    session_id: &str,
    object_id: u32,
    name: &str,
    x: i32,
    y: i32,
    combat_stats: ZonePlayerCombatStats,
) -> ZoneJoin {
    let mut join = join(session_id, object_id, name, x, y);
    join.combat_stats = combat_stats;
    join
}

/// A high-HP native monster with explicit authoritative defensive stats, used to
/// exercise zone-side hit/miss + armour resolution without the monster dying in
/// a single blow.
fn native_monster_spawn_with_defense(
    object_id: u32,
    x: i32,
    y: i32,
    max_hp: i32,
    defense: ZoneMonsterDefense,
) -> ZoneMonsterSpawn {
    let mut spawn = native_monster_spawn(object_id, x, y);
    spawn.max_hp = max_hp;
    spawn.hp = max_hp;
    spawn.defense = defense;
    spawn
}

/// Extract the first `DamageIndicator` damage value addressed at `object_id`.
fn damage_indicator_for(outbounds: &[ZoneOutbound], object_id: u32) -> Option<i32> {
    outbounds.iter().find_map(|outbound| {
        let packets = match outbound {
            ZoneOutbound::ToSession { packets, .. }
            | ZoneOutbound::ToMany { packets, .. }
            | ZoneOutbound::ToAll { packets } => packets,
            _ => return None,
        };
        packets.iter().find_map(|packet| match packet {
            ServerPacket::DamageIndicator {
                damage,
                object_id: target,
                ..
            } if *target == object_id => Some(*damage),
            _ => None,
        })
    })
}

fn packets_for(outbounds: &[ZoneOutbound], session_id: &SessionId) -> Vec<ServerPacket> {
    outbounds
        .iter()
        .flat_map(|outbound| match outbound {
            ZoneOutbound::ToSession {
                session_id: target,
                packets,
            } if target == session_id => packets.clone(),
            ZoneOutbound::ToMany {
                session_ids,
                packets,
            } if session_ids.iter().any(|target| target == session_id) => packets.clone(),
            ZoneOutbound::ToAll { packets } => packets.clone(),
            _ => Vec::new(),
        })
        .collect()
}

fn has_packet(
    outbounds: &[ZoneOutbound],
    session_id: &SessionId,
    predicate: impl Fn(&ServerPacket) -> bool,
) -> bool {
    packets_for(outbounds, session_id).iter().any(predicate)
}

fn has_shout_consume(
    outbounds: &[ZoneOutbound],
    session_id: &SessionId,
    expected_map_shout: bool,
    expected_server_shout: bool,
) -> bool {
    outbounds.iter().any(|outbound| {
        matches!(
            outbound,
            ZoneOutbound::ConsumeShoutPermission {
                session_id: target,
                map_shout,
                server_shout,
            } if target == session_id
                && *map_shout == expected_map_shout
                && *server_shout == expected_server_shout
        )
    })
}

#[test]
fn two_players_join_same_zone_see_each_other() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    let outbounds = zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));

    assert!(has_packet(&outbounds, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectPlayer { info } if info.name == "Blade"
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectPlayer { info } if info.name == "Scout"
    )));
}

#[test]
fn player_a_walks_player_b_receives_object_walk() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));

    zone.handle(ZoneCommand::Walk {
        session_id: first.clone(),
        direction: MirDirection::Right,
        seq: 1,
        now_ms: 0,
    });
    let outbounds = zone.tick(0);

    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectWalk { movement }
            if movement.object_id == 101 && movement.position == (Point { x: 331, y: 270 })
    )));
    assert!(has_packet(&outbounds, &first, |packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == (Point { x: 331, y: 270 })
    )));
}

#[test]
fn player_a_runs_player_b_receives_object_run() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 336, 270)));

    zone.handle(ZoneCommand::Walk {
        session_id: first.clone(),
        direction: MirDirection::Right,
        seq: 1,
        now_ms: 0,
    });
    zone.tick(0);
    zone.handle(ZoneCommand::Run {
        session_id: first,
        direction: MirDirection::Right,
        seq: 2,
        now_ms: 0,
    });
    let outbounds = zone.tick(600);

    assert!(
        has_packet(&outbounds, &second, |packet| matches!(
            packet,
            ServerPacket::ObjectRun { movement }
                if movement.object_id == 101 && movement.position == (Point { x: 333, y: 270 })
        )),
        "expected delayed run packets: {outbounds:?}"
    );
}

#[test]
fn mounted_player_run_moves_three_tiles_and_broadcasts_object_run() {
    let mut zone = zone();
    let rider = session("rider");
    let observer = session("observer");
    zone.handle(ZoneCommand::Join(join("rider", 201, "Rider", 330, 270)));
    zone.handle(ZoneCommand::Join(join(
        "observer", 202, "Observer", 338, 270,
    )));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: rider.clone(),
        owner_local_object_id: 2001,
        packets: vec![ServerPacket::MountUpdate {
            object_id: 2001,
            mount_type: 3,
            riding_mount: true,
        }],
        now_ms: 0,
    });

    zone.handle(ZoneCommand::Walk {
        session_id: rider.clone(),
        direction: MirDirection::Right,
        seq: 1,
        now_ms: 0,
    });
    zone.tick(0);
    zone.handle(ZoneCommand::Run {
        session_id: rider.clone(),
        direction: MirDirection::Right,
        seq: 2,
        now_ms: 0,
    });
    let outbounds = zone.tick(600);

    assert_eq!(zone.player_position(&rider), Some(Point { x: 334, y: 270 }));
    assert!(has_packet(&outbounds, &observer, |packet| matches!(
        packet,
        ServerPacket::ObjectRun { movement }
            if movement.object_id == 201 && movement.position == (Point { x: 334, y: 270 })
    )));
}

#[test]
fn swift_feet_run_moves_three_tiles_unless_sneaking() {
    let mut zone = zone();
    let runner = session("runner");
    zone.handle(ZoneCommand::Join(join("runner", 201, "Runner", 330, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: runner.clone(),
        owner_local_object_id: 2001,
        packets: vec![ServerPacket::AddBuff {
            buff: ClientBuff {
                buff_type: 4,
                visible: true,
                object_id: 2001,
                expire_time: 5_000,
                infinite: false,
                paused: false,
                stats: Vec::new(),
                values: Vec::new(),
            },
        }],
        now_ms: 0,
    });

    zone.handle(ZoneCommand::Walk {
        session_id: runner.clone(),
        direction: MirDirection::Right,
        seq: 1,
        now_ms: 0,
    });
    zone.tick(0);
    zone.handle(ZoneCommand::Run {
        session_id: runner.clone(),
        direction: MirDirection::Right,
        seq: 2,
        now_ms: 0,
    });
    zone.tick(600);
    assert_eq!(
        zone.player_position(&runner),
        Some(Point { x: 334, y: 270 })
    );

    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: runner.clone(),
        owner_local_object_id: 2001,
        packets: vec![ServerPacket::ObjectSneaking {
            object_id: 2001,
            sneaking_active: true,
        }],
        now_ms: 600,
    });
    zone.handle(ZoneCommand::Run {
        session_id: runner.clone(),
        direction: MirDirection::Right,
        seq: 3,
        now_ms: 600,
    });
    zone.tick(1_200);
    assert_eq!(
        zone.player_position(&runner),
        Some(Point { x: 336, y: 270 })
    );
}

#[test]
fn paused_swift_feet_does_not_extend_run_to_three_tiles() {
    let mut zone = zone();
    let runner = session("runner");
    zone.handle(ZoneCommand::Join(join("runner", 201, "Runner", 330, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: runner.clone(),
        owner_local_object_id: 2001,
        packets: vec![
            ServerPacket::AddBuff {
                buff: ClientBuff {
                    buff_type: 4,
                    visible: true,
                    object_id: 2001,
                    expire_time: 5_000,
                    infinite: false,
                    paused: false,
                    stats: Vec::new(),
                    values: Vec::new(),
                },
            },
            ServerPacket::PauseBuff {
                buff_type: 4,
                object_id: 2001,
                paused: true,
            },
        ],
        now_ms: 0,
    });

    zone.handle(ZoneCommand::Walk {
        session_id: runner.clone(),
        direction: MirDirection::Right,
        seq: 1,
        now_ms: 0,
    });
    zone.tick(0);
    zone.handle(ZoneCommand::Run {
        session_id: runner.clone(),
        direction: MirDirection::Right,
        seq: 2,
        now_ms: 0,
    });
    zone.tick(600);

    assert_eq!(
        zone.player_position(&runner),
        Some(Point { x: 333, y: 270 })
    );
}

#[test]
fn mounted_run_checks_third_tile_before_moving() {
    let collision = ZoneCollision::unbounded().with_blocked_cells([Point { x: 334, y: 270 }]);
    let mut zone = ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), collision);
    let rider = session("rider");
    zone.handle(ZoneCommand::Join(join("rider", 201, "Rider", 330, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: rider.clone(),
        owner_local_object_id: 2001,
        packets: vec![ServerPacket::MountUpdate {
            object_id: 2001,
            mount_type: 3,
            riding_mount: true,
        }],
        now_ms: 0,
    });
    zone.handle(ZoneCommand::Walk {
        session_id: rider.clone(),
        direction: MirDirection::Right,
        seq: 1,
        now_ms: 0,
    });
    zone.tick(0);
    zone.handle(ZoneCommand::Run {
        session_id: rider.clone(),
        direction: MirDirection::Right,
        seq: 2,
        now_ms: 0,
    });
    let outbounds = zone.tick(600);

    assert_eq!(zone.player_position(&rider), Some(Point { x: 331, y: 270 }));
    assert!(has_packet(&outbounds, &rider, |packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == (Point { x: 331, y: 270 })
    )));
}

#[test]
fn run_from_standstill_degrades_to_walk() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 336, 270)));

    zone.handle(ZoneCommand::Run {
        session_id: first.clone(),
        direction: MirDirection::Right,
        seq: 1,
        now_ms: 0,
    });
    let outbounds = zone.tick(0);

    assert_eq!(zone.player_position(&first), Some(Point { x: 331, y: 270 }));
    assert!(has_packet(&outbounds, &first, |packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == (Point { x: 331, y: 270 })
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectWalk { movement }
            if movement.object_id == 101 && movement.position == (Point { x: 331, y: 270 })
    )));
    assert!(!has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectRun { .. }
    )));
}

#[test]
fn run_step_expires_after_crystal_celltime_grace() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 336, 270)));

    zone.handle(ZoneCommand::Walk {
        session_id: first.clone(),
        direction: MirDirection::Right,
        seq: 1,
        now_ms: 0,
    });
    zone.tick(0);
    let mut outbounds = zone.handle(ZoneCommand::Run {
        session_id: first.clone(),
        direction: MirDirection::Right,
        seq: 2,
        now_ms: 1_500,
    });
    outbounds.extend(zone.tick(1_500));

    assert_eq!(zone.player_position(&first), Some(Point { x: 332, y: 270 }));
    assert!(has_packet(&outbounds, &first, |packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == (Point { x: 332, y: 270 })
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectWalk { movement }
            if movement.object_id == 101 && movement.position == (Point { x: 332, y: 270 })
    )));
    assert!(!has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectRun { .. }
    )));
}

#[test]
fn mounted_player_walk_uses_the_same_crystal_server_delay() {
    // Crystal applies the same 600ms MoveDelay to mounted and unmounted
    // movement. The mount's travel advantage is its three-tile Run.
    let mut zone = zone();
    let rider = session("rider");
    let walker = session("walker");
    zone.handle(ZoneCommand::Join(join("rider", 201, "Rider", 320, 250)));
    zone.handle(ZoneCommand::Join(join("walker", 202, "Walker", 360, 250)));

    // Put the rider on a mount via the same MountUpdate path a real client uses.
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: rider.clone(),
        owner_local_object_id: 2001,
        packets: vec![ServerPacket::MountUpdate {
            object_id: 2001,
            mount_type: 3,
            riding_mount: true,
        }],
        now_ms: 0,
    });

    // First step for both, at the same instant.
    for who in [&rider, &walker] {
        zone.handle(ZoneCommand::Walk {
            session_id: who.clone(),
            direction: MirDirection::Right,
            seq: 1,
            now_ms: 0,
        });
    }
    zone.tick(0);
    assert_eq!(zone.player_position(&rider), Some(Point { x: 321, y: 250 }));
    assert_eq!(
        zone.player_position(&walker),
        Some(Point { x: 361, y: 250 })
    );

    // Queue both second steps early (now=50), outside the 300ms input buffer of
    // either ready time, so neither is consumed on arrival — both stay queued
    // until a tick reaches their cadence.
    for who in [&rider, &walker] {
        zone.handle(ZoneCommand::Walk {
            session_id: who.clone(),
            direction: MirDirection::Right,
            seq: 2,
            now_ms: 50,
        });
    }
    assert_eq!(
        zone.player_position(&rider),
        Some(Point { x: 321, y: 250 }),
        "queued step must not consume before the shared cadence elapses"
    );

    // Neither player may move at 400ms.
    zone.tick(400);
    assert_eq!(
        zone.player_position(&rider),
        Some(Point { x: 321, y: 250 }),
        "mounted movement must retain Crystal's 600ms server delay"
    );
    assert_eq!(
        zone.player_position(&walker),
        Some(Point { x: 361, y: 250 }),
        "unmounted player should still be on cooldown at 400ms"
    );

    // Both players advance once the common 600ms delay elapses.
    zone.tick(600);
    assert_eq!(zone.player_position(&rider), Some(Point { x: 322, y: 250 }));
    assert_eq!(
        zone.player_position(&walker),
        Some(Point { x: 362, y: 250 }),
        "unmounted player should step once its 600ms cadence elapses"
    );
}

#[test]
fn run_received_inside_grace_survives_late_zone_tick() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 336, 270)));

    zone.handle(ZoneCommand::Walk {
        session_id: first.clone(),
        direction: MirDirection::Right,
        seq: 1,
        now_ms: 0,
    });
    zone.tick(0);
    zone.handle(ZoneCommand::Run {
        session_id: first.clone(),
        direction: MirDirection::Right,
        seq: 2,
        now_ms: 600,
    });
    let outbounds = zone.tick(1_500);

    assert_eq!(zone.player_position(&first), Some(Point { x: 333, y: 270 }));
    assert!(has_packet(&outbounds, &first, |packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == (Point { x: 333, y: 270 })
    )));
    assert!(
        has_packet(&outbounds, &second, |packet| matches!(
            packet,
            ServerPacket::ObjectRun { movement }
                if movement.object_id == 101 && movement.position == (Point { x: 333, y: 270 })
        )),
        "run input accepted during Crystal grace should not be downgraded by late tick consumption: {outbounds:?}"
    );
}

#[test]
fn ready_pending_run_is_consumed_before_followup_direction_replaces_it() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 336, 270)));

    zone.handle(ZoneCommand::Walk {
        session_id: first.clone(),
        direction: MirDirection::Right,
        seq: 1,
        now_ms: 0,
    });
    zone.tick(0);
    let run_outbounds = zone.handle(ZoneCommand::Run {
        session_id: first.clone(),
        direction: MirDirection::Right,
        seq: 2,
        now_ms: 400,
    });
    assert!(
        has_packet(&run_outbounds, &second, |packet| matches!(
            packet,
            ServerPacket::ObjectRun { movement }
                if movement.object_id == 101 && movement.position == (Point { x: 333, y: 270 })
        )),
        "buffered run should be consumed as soon as it arrives near the ready edge: {run_outbounds:?}"
    );

    let reverse_outbounds = zone.handle(ZoneCommand::Walk {
        session_id: first.clone(),
        direction: MirDirection::Left,
        seq: 3,
        now_ms: 700,
    });

    assert_eq!(zone.player_position(&first), Some(Point { x: 332, y: 270 }));
    assert!(
        has_packet(&reverse_outbounds, &first, |packet| matches!(
            packet,
            ServerPacket::UserLocation { location }
                if location.position == (Point { x: 332, y: 270 })
                    && location.direction == MirDirection::Left
        )),
        "follow-up reverse walk should also be acknowledged from the same buffered chain: {reverse_outbounds:?}"
    );

    let left_outbounds = zone.tick(1_000);
    assert_eq!(zone.player_position(&first), Some(Point { x: 332, y: 270 }));
    assert!(
        !has_packet(&left_outbounds, &first, |packet| matches!(
            packet,
            ServerPacket::UserLocation { .. }
        )),
        "buffered chain should not leave a delayed correction packet behind: {left_outbounds:?}"
    );
}

#[test]
fn buffered_walk_run_reverse_chain_returns_immediate_location_acks() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 336, 270)));

    zone.handle(ZoneCommand::Walk {
        session_id: first.clone(),
        direction: MirDirection::Right,
        seq: 1,
        now_ms: 0,
    });
    zone.tick(0);

    let run_outbounds = zone.handle(ZoneCommand::Run {
        session_id: first.clone(),
        direction: MirDirection::Right,
        seq: 2,
        now_ms: 400,
    });
    assert_eq!(zone.player_position(&first), Some(Point { x: 333, y: 270 }));
    assert!(
        has_packet(&run_outbounds, &first, |packet| matches!(
            packet,
            ServerPacket::UserLocation { location }
                if location.position == (Point { x: 333, y: 270 })
        )),
        "buffered run should acknowledge immediately instead of waiting for a socket tick: {run_outbounds:?}"
    );
    assert!(has_packet(&run_outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectRun { movement }
            if movement.object_id == 101 && movement.position == (Point { x: 333, y: 270 })
    )));

    let reverse_outbounds = zone.handle(ZoneCommand::Walk {
        session_id: first.clone(),
        direction: MirDirection::Left,
        seq: 3,
        now_ms: 700,
    });
    assert_eq!(zone.player_position(&first), Some(Point { x: 332, y: 270 }));
    assert!(
        has_packet(&reverse_outbounds, &first, |packet| matches!(
            packet,
            ServerPacket::UserLocation { location }
                if location.position == (Point { x: 332, y: 270 })
                    && location.direction == MirDirection::Left
        )),
        "buffered reverse walk should acknowledge immediately after a run: {reverse_outbounds:?}"
    );
    assert!(has_packet(&reverse_outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectWalk { movement }
            if movement.object_id == 101
                && movement.position == (Point { x: 332, y: 270 })
                && movement.direction == MirDirection::Left
    )));
}

#[test]
fn continuous_run_extends_run_grace_after_successful_run() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 340, 270)));

    zone.handle(ZoneCommand::Walk {
        session_id: first.clone(),
        direction: MirDirection::Right,
        seq: 1,
        now_ms: 0,
    });
    let first_outbounds = zone.tick(0);
    assert!(has_packet(&first_outbounds, &first, |packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == (Point { x: 331, y: 270 })
    )));

    zone.handle(ZoneCommand::Run {
        session_id: first.clone(),
        direction: MirDirection::Right,
        seq: 2,
        now_ms: 0,
    });
    let second_outbounds = zone.tick(600);
    assert!(has_packet(&second_outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectRun { movement }
            if movement.object_id == 101 && movement.position == (Point { x: 333, y: 270 })
    )));

    zone.handle(ZoneCommand::Run {
        session_id: first,
        direction: MirDirection::Right,
        seq: 3,
        now_ms: 0,
    });
    let third_outbounds = zone.tick(1_200);

    assert!(
        has_packet(&third_outbounds, &second, |packet| matches!(
            packet,
            ServerPacket::ObjectRun { movement }
                if movement.object_id == 101 && movement.position == (Point { x: 335, y: 270 })
        )),
        "successful run should refresh the next run window instead of degrading back to walk: {third_outbounds:?}"
    );
}

#[test]
fn player_a_turns_player_b_receives_object_turn() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));

    zone.handle(ZoneCommand::Turn {
        session_id: first,
        direction: MirDirection::Left,
        now_ms: 0,
    });
    let outbounds = zone.tick(0);

    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectTurn { movement }
            if movement.object_id == 101 && movement.direction == MirDirection::Left
    )));
}

#[test]
fn player_action_packets_are_rewritten_and_broadcast_to_observers() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));

    let outbounds = zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![
            ServerPacket::ObjectAttack {
                info: ObjectAttackInfo {
                    object_id: 1001,
                    location: Point { x: 1, y: 1 },
                    direction: MirDirection::Right,
                    spell: Spell::None as u8,
                    level: 0,
                    attack_type: 0,
                },
            },
            ServerPacket::ObjectStruck {
                info: ObjectStruckInfo {
                    object_id: 3002,
                    attacker_id: 1001,
                    location: Point { x: 331, y: 270 },
                    direction: MirDirection::Left,
                },
            },
            ServerPacket::ObjectStruck {
                info: ObjectStruckInfo {
                    object_id: 1001,
                    attacker_id: 3002,
                    location: Point { x: 330, y: 270 },
                    direction: MirDirection::Down,
                },
            },
            ServerPacket::ObjectHealth {
                info: ObjectHealthInfo {
                    object_id: 3002,
                    percent: 80,
                    expire: 0,
                },
            },
            ServerPacket::ObjectHealth {
                info: ObjectHealthInfo {
                    object_id: 1001,
                    percent: 90,
                    expire: 0,
                },
            },
            ServerPacket::ObjectDied {
                info: ObjectDiedInfo {
                    object_id: 1001,
                    location: Point { x: 1, y: 1 },
                    direction: MirDirection::Left,
                    kind: 0,
                },
            },
        ],
        now_ms: 0,
    });

    assert!(!has_packet(&outbounds, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { .. }
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info }
            if info.object_id == 101
                && info.location == (Point { x: 330, y: 270 })
                && info.direction == MirDirection::Right
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectStruck { info }
            if info.object_id == 3002 && info.attacker_id == 101
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectStruck { info }
            if info.object_id == 101 && info.attacker_id == 3002
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info } if info.object_id == 3002 && info.percent == 80
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info } if info.object_id == 101 && info.percent == 90
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectDied { info }
            if info.object_id == 101
                && info.location == (Point { x: 330, y: 270 })
                && info.direction == MirDirection::Down
    )));
}

#[test]
fn player_magic_and_projectiles_use_zone_object_id_for_observers() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));

    let outbounds = zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![
            ServerPacket::ObjectRangeAttack {
                info: ObjectRangeAttackInfo {
                    object_id: 1001,
                    location: Point { x: 1, y: 1 },
                    direction: MirDirection::Right,
                    target_id: 1001,
                    target: Point { x: 334, y: 270 },
                    attack_type: 0,
                    spell: Spell::None as u8,
                    level: 0,
                },
            },
            ServerPacket::ObjectMagic {
                object_id: 1001,
                location: Point { x: 1, y: 1 },
                direction: MirDirection::Right,
                spell: Spell::FireBall,
                target_id: 1001,
                target: Point { x: 334, y: 270 },
                cast: true,
                level: 1,
                self_broadcast: false,
                secondary_target_ids: vec![1001, 3002],
            },
            ServerPacket::ObjectProjectile {
                spell: Spell::FireBall,
                source_id: 1001,
                destination_id: 1001,
            },
        ],
        now_ms: 0,
    });

    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectRangeAttack { info }
            if info.object_id == 101
                && info.target_id == 101
                && info.location == (Point { x: 330, y: 270 })
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectMagic {
            object_id,
            location,
            spell,
            target_id,
            secondary_target_ids,
            ..
        } if *object_id == 101 && location == &(Point { x: 330, y: 270 })
            && *spell == Spell::FireBall
            && *target_id == 101
            && secondary_target_ids == &vec![101, 3002]
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectProjectile {
            source_id,
            destination_id,
            ..
        } if *source_id == 101 && *destination_id == 101
    )));
}

#[test]
fn player_movement_skill_packets_use_zone_object_id_for_observers() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));

    let outbounds = zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![
            ServerPacket::ObjectBackStep {
                movement: ObjectMovement {
                    object_id: 1001,
                    position: Point { x: 329, y: 270 },
                    direction: MirDirection::Left,
                },
                distance: 1,
            },
            ServerPacket::ObjectDash {
                object_id: 1001,
                location: Point { x: 332, y: 270 },
                direction: MirDirection::Right,
            },
            ServerPacket::ObjectDashFail {
                object_id: 1001,
                location: Point { x: 330, y: 270 },
                direction: MirDirection::Right,
            },
            ServerPacket::ObjectDashAttack {
                object_id: 1001,
                location: Point { x: 331, y: 270 },
                direction: MirDirection::Right,
                distance: 1,
            },
        ],
        now_ms: 0,
    });

    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectBackStep { movement, distance }
            if movement.object_id == 101
                && movement.position == (Point { x: 329, y: 270 })
                && movement.direction == MirDirection::Left
                && *distance == 1
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectDash {
            object_id,
            location,
            direction
        } if *object_id == 101
            && location == &(Point { x: 332, y: 270 })
            && *direction == MirDirection::Right
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectDashFail {
            object_id,
            location,
            direction
        } if *object_id == 101
            && location == &(Point { x: 330, y: 270 })
            && *direction == MirDirection::Right
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectDashAttack {
            object_id,
            location,
            direction,
            distance
        } if *object_id == 101
            && location == &(Point { x: 331, y: 270 })
            && *direction == MirDirection::Right
            && *distance == 1
    )));
}

#[test]
fn zone_applies_player_movement_skill_transform_and_save() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 336, 270)));

    let outbounds = zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectDash {
            object_id: 1001,
            location: Point { x: 332, y: 270 },
            direction: MirDirection::Right,
        }],
        now_ms: 0,
    });

    assert_eq!(zone.player_position(&first), Some(Point { x: 332, y: 270 }));
    assert_eq!(zone.player_direction(&first), Some(MirDirection::Right));
    assert!(outbounds.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::SaveTransform {
            session_id,
            position,
            direction
        } if session_id == &first
            && position == &(Point { x: 332, y: 270 })
            && *direction == MirDirection::Right
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectDash {
            object_id,
            location,
            direction
        } if *object_id == 101
            && location == &(Point { x: 332, y: 270 })
            && *direction == MirDirection::Right
    )));
}

#[test]
fn zone_rejects_player_movement_skill_transform_into_occupied_tile() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 331, 270)));

    let outbounds = zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectDash {
            object_id: 1001,
            location: Point { x: 331, y: 270 },
            direction: MirDirection::Right,
        }],
        now_ms: 0,
    });

    assert_eq!(zone.player_position(&first), Some(Point { x: 330, y: 270 }));
    assert!(has_packet(&outbounds, &first, |packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == (Point { x: 330, y: 270 })
                && location.direction == MirDirection::Right
    )));
    assert!(outbounds.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::SaveTransform {
            session_id,
            position,
            direction
        } if session_id == &first
            && position == &(Point { x: 330, y: 270 })
            && *direction == MirDirection::Right
    )));
    assert!(!has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectDash { .. }
    )));
}

#[test]
fn zone_applies_user_location_action_transform_before_observer_effect() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 336, 270)));

    let outbounds = zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![
            ServerPacket::UserLocation {
                location: UserLocation {
                    position: Point { x: 334, y: 271 },
                    direction: MirDirection::UpRight,
                },
            },
            ServerPacket::ObjectEffect {
                info: ObjectEffectInfo {
                    object_id: 1001,
                    effect: 24,
                    effect_type: 0,
                    delay_time: 0,
                    time: 0,
                },
            },
        ],
        now_ms: 0,
    });

    assert_eq!(zone.player_position(&first), Some(Point { x: 334, y: 271 }));
    assert_eq!(zone.player_direction(&first), Some(MirDirection::UpRight));
    assert!(outbounds.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::SaveTransform {
            session_id,
            position,
            direction
        } if session_id == &first
            && position == &(Point { x: 334, y: 271 })
            && *direction == MirDirection::UpRight
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectEffect { info }
            if info.object_id == 101 && info.effect == 24
    )));
}

#[test]
fn player_special_skill_state_packets_use_zone_object_id_for_observers() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));

    let outbounds = zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![
            ServerPacket::SetConcentration {
                object_id: 1001,
                enabled: true,
                interrupted: false,
            },
            ServerPacket::SetElemental {
                object_id: 1001,
                enabled: true,
                casted: true,
                value: 3,
                element_type: 2,
                exp_last: 11,
            },
            ServerPacket::SetBindingShot {
                object_id: 1001,
                enabled: true,
                value: 900,
            },
            ServerPacket::RemoveDelayedExplosion { object_id: 1001 },
            ServerPacket::ObjectSneaking {
                object_id: 1001,
                sneaking_active: true,
            },
            ServerPacket::ObjectLevelEffects {
                object_id: 1001,
                level_effects: 4,
            },
            ServerPacket::ObjectPoisoned {
                object_id: 1001,
                poison: 3,
            },
        ],
        now_ms: 0,
    });

    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::SetConcentration {
            object_id,
            enabled,
            interrupted
        } if *object_id == 101 && *enabled && !*interrupted
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::SetElemental {
            object_id,
            enabled,
            casted,
            value,
            element_type,
            exp_last
        } if *object_id == 101
            && *enabled
            && *casted
            && *value == 3
            && *element_type == 2
            && *exp_last == 11
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::SetBindingShot {
            object_id,
            enabled,
            value
        } if *object_id == 101 && *enabled && *value == 900
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::RemoveDelayedExplosion { object_id } if *object_id == 101
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectSneaking {
            object_id,
            sneaking_active
        } if *object_id == 101 && *sneaking_active
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectLevelEffects {
            object_id,
            level_effects
        } if *object_id == 101 && *level_effects == 4
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectPoisoned { object_id, poison }
            if *object_id == 101 && *poison == 3
    )));
}

#[test]
fn player_late_status_packets_use_zone_object_id_for_observers() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));

    let outbounds = zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![
            ServerPacket::PlayerUpdate {
                object_id: 1001,
                light: 5,
                weapon: 12,
                weapon_effect: 2,
                armour: 44,
                wing_effect: 1,
            },
            ServerPacket::DamageIndicator {
                damage: 17,
                damage_type: 2,
                object_id: 1001,
            },
            ServerPacket::ObjectColourChanged {
                object_id: 1001,
                name_colour_argb: -255,
            },
            ServerPacket::ObjectGuildNameChanged {
                object_id: 1001,
                guild_name: "Codex".to_string(),
            },
            ServerPacket::ObjectLeveled { object_id: 1001 },
            ServerPacket::ObjectName {
                object_id: 1001,
                name: "ScoutRenamed".to_string(),
            },
            ServerPacket::MagicDelay {
                object_id: 1001,
                spell: Spell::FireBall,
                delay: 1500,
            },
            ServerPacket::PauseBuff {
                buff_type: 7,
                object_id: 1001,
                paused: true,
            },
            ServerPacket::MountUpdate {
                object_id: 1001,
                mount_type: 3,
                riding_mount: true,
            },
            ServerPacket::FishingUpdate {
                object_id: 1001,
                fishing: true,
                progress_percent: 40,
                chance_percent: 12,
                fishing_point: Point { x: 331, y: 272 },
                found_fish: false,
            },
            ServerPacket::TransformUpdate {
                object_id: 1001,
                transform_type: 6,
            },
            ServerPacket::ObjectTeleportOut {
                object_id: 1001,
                effect_type: 4,
            },
            ServerPacket::ObjectTeleportIn {
                object_id: 1001,
                effect_type: 5,
            },
            ServerPacket::ObjectDeco {
                object_id: 1001,
                location: Point { x: 331, y: 270 },
                image: 88,
            },
        ],
        now_ms: 0,
    });

    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::PlayerUpdate {
            object_id,
            light,
            weapon,
            armour,
            ..
        } if *object_id == 101 && *light == 5 && *weapon == 12 && *armour == 44
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::DamageIndicator {
            object_id,
            damage,
            damage_type
        } if *object_id == 101 && *damage == 17 && *damage_type == 2
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectColourChanged {
            object_id,
            name_colour_argb
        } if *object_id == 101 && *name_colour_argb == -255
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectGuildNameChanged {
            object_id,
            guild_name
        } if *object_id == 101 && guild_name == "Codex"
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectLeveled { object_id } if *object_id == 101
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectName { object_id, name }
            if *object_id == 101 && name == "ScoutRenamed"
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::MagicDelay {
            object_id,
            spell,
            delay
        } if *object_id == 101 && *spell == Spell::FireBall && *delay == 1500
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::PauseBuff {
            object_id,
            buff_type,
            paused
        } if *object_id == 101 && *buff_type == 7 && *paused
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::MountUpdate {
            object_id,
            mount_type,
            riding_mount
        } if *object_id == 101 && *mount_type == 3 && *riding_mount
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::FishingUpdate {
            object_id,
            fishing,
            progress_percent,
            chance_percent,
            fishing_point,
            found_fish
        } if *object_id == 101
            && *fishing
            && *progress_percent == 40
            && *chance_percent == 12
            && fishing_point == &(Point { x: 331, y: 272 })
            && !*found_fish
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::TransformUpdate {
            object_id,
            transform_type
        } if *object_id == 101 && *transform_type == 6
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectTeleportOut {
            object_id,
            effect_type
        } if *object_id == 101 && *effect_type == 4
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectTeleportIn {
            object_id,
            effect_type
        } if *object_id == 101 && *effect_type == 5
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectDeco {
            object_id,
            location,
            image
        } if *object_id == 101 && location == &(Point { x: 331, y: 270 }) && *image == 88
    )));
}

#[test]
fn player_buff_mana_and_effect_packets_use_zone_object_id_for_observers() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));

    let outbounds = zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![
            ServerPacket::ObjectMana {
                info: ObjectManaInfo {
                    object_id: 1001,
                    percent: 72,
                },
            },
            ServerPacket::AddBuff {
                buff: ClientBuff {
                    buff_type: 7,
                    visible: true,
                    object_id: 1001,
                    expire_time: 1234,
                    infinite: false,
                    paused: false,
                    stats: Vec::new(),
                    values: vec![5],
                },
            },
            ServerPacket::RemoveBuff {
                buff_type: 7,
                object_id: 1001,
            },
            ServerPacket::ObjectEffect {
                info: ObjectEffectInfo {
                    object_id: 1001,
                    effect: 3,
                    effect_type: 9,
                    delay_time: 10,
                    time: 20,
                },
            },
            ServerPacket::ObjectSpell {
                info: ObjectSpellInfo {
                    object_id: 1001,
                    location: Point { x: 1, y: 1 },
                    spell: Spell::FireWall,
                    direction: MirDirection::Left,
                    param: true,
                },
            },
        ],
        now_ms: 0,
    });

    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectMana { info } if info.object_id == 101 && info.percent == 72
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::AddBuff { buff }
            if buff.object_id == 101 && buff.buff_type == 7 && buff.values.as_slice() == [5]
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::RemoveBuff { object_id, buff_type }
            if *object_id == 101 && *buff_type == 7
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectEffect { info }
            if info.object_id == 101 && info.effect == 3 && info.effect_type == 9
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectSpell { info }
            if info.object_id == 101
                && info.location == (Point { x: 330, y: 270 })
                && info.direction == MirDirection::Down
                && info.spell == Spell::FireWall
    )));
}

#[test]
fn player_drop_spawn_packets_are_broadcast_to_observers() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));

    let outbounds = zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![
            ServerPacket::ObjectGold {
                info: ObjectGoldInfo {
                    object_id: 8001,
                    gold: 25,
                    location: Point { x: 331, y: 270 },
                },
            },
            ServerPacket::ObjectItem {
                info: ObjectItemInfo {
                    object_id: 8002,
                    name: "Bronze Helmet".to_string(),
                    name_colour_argb: -1,
                    location: Point { x: 330, y: 271 },
                    image: 42,
                    grade: 0,
                },
            },
        ],
        now_ms: 0,
    });

    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectGold { info }
            if info.object_id == 8001
                && info.gold == 25
                && info.location == (Point { x: 331, y: 270 })
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectItem { info }
            if info.object_id == 8002
                && info.name == "Bronze Helmet"
                && info.location == (Point { x: 330, y: 271 })
                && info.image == 42
    )));
}

#[test]
fn player_intelligent_creature_pickup_is_broadcast_to_observers() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));

    let outbounds = zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![
            ServerPacket::IntelligentCreaturePickup { object_id: 8001 },
            ServerPacket::GainedGold { gold: 25 },
        ],
        now_ms: 0,
    });

    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::IntelligentCreaturePickup { object_id } if *object_id == 8001
    )));
    assert!(!has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::GainedGold { .. }
    )));
}

#[test]
fn player_spawned_monster_packets_rebase_master_for_observers() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));

    let outbounds = zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectMonster {
            info: MonsterInfo {
                object_id: 5001,
                name: "Shinsu".to_string(),
                name_colour_argb: -1,
                location: Point { x: 331, y: 270 },
                image: 33,
                direction: MirDirection::Down,
                effect: 0,
                ai: 6,
                light: 0,
                dead: false,
                skeleton: false,
                poison: 0,
                hidden: false,
                shock_time: 0,
                binding_shot_center: false,
                extra: false,
                extra_byte: 0,
                master_object_id: 1001,
                rarity: 0,
                buffs: vec![7],
            },
        }],
        now_ms: 0,
    });

    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info }
            if info.object_id == 5001
                && info.name == "Shinsu"
                && info.master_object_id == 101
                && info.buffs == vec![7]
    )));
}

#[test]
fn player_spawned_hero_and_npc_packets_are_broadcast_to_observers() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));

    let outbounds = zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![
            ServerPacket::ObjectHero {
                info: ObjectPlayerInfo {
                    object_id: 6001,
                    name: "ScoutHero".to_string(),
                    guild_name: String::new(),
                    guild_rank_name: String::new(),
                    name_colour_argb: -1,
                    class: MirClass::Taoist,
                    gender: MirGender::Female,
                    level: 7,
                    location: Point { x: 331, y: 271 },
                    direction: MirDirection::Left,
                    hair: 0,
                    light: 0,
                    weapon: -1,
                    weapon_effect: 0,
                    armour: -1,
                    poison: 0,
                    dead: false,
                    hidden: false,
                    effect: 0,
                    wing_effect: 0,
                    extra: false,
                    mount_type: -1,
                    riding_mount: false,
                    fishing: false,
                    transform_type: 0,
                    element_orb_effect: 0,
                    element_orb_level: 0,
                    element_orb_max: 0,
                    buffs: vec![3],
                    level_effects: 0,
                },
                owner_name: "Scout".to_string(),
            },
            ServerPacket::ObjectNpc {
                info: NpcInfo {
                    object_id: 7001,
                    name: "Village Guide".to_string(),
                    name_colour_argb: -1,
                    image: 12,
                    colour_argb: -1,
                    location: Point { x: 329, y: 270 },
                    direction: MirDirection::Down,
                    quest_ids: vec![1, 2],
                },
            },
        ],
        now_ms: 0,
    });

    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectHero { info, owner_name }
            if info.object_id == 6001
                && info.name == "ScoutHero"
                && info.buffs == vec![3]
                && owner_name == "Scout"
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectNpc { info }
            if info.object_id == 7001
                && info.name == "Village Guide"
                && info.quest_ids == vec![1, 2]
    )));
}

#[test]
fn later_joiner_receives_retained_zone_object_spawns() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![
            monster_spawn_packet(5001, 1001, 331, 270),
            ServerPacket::ObjectNpc {
                info: NpcInfo {
                    object_id: 7001,
                    name: "Village Guide".to_string(),
                    name_colour_argb: -1,
                    image: 12,
                    colour_argb: -1,
                    location: Point { x: 329, y: 270 },
                    direction: MirDirection::Down,
                    quest_ids: vec![1, 2],
                },
            },
            ServerPacket::ObjectGold {
                info: ObjectGoldInfo {
                    object_id: 8001,
                    gold: 25,
                    location: Point { x: 332, y: 270 },
                },
            },
        ],
        now_ms: 0,
    });

    let second = session("second");
    let outbounds = zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));

    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info }
            if info.object_id == 5001
                && info.master_object_id == 101
                && info.location == (Point { x: 331, y: 270 })
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectNpc { info }
            if info.object_id == 7001 && info.quest_ids == vec![1, 2]
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectGold { info }
            if info.object_id == 8001
                && info.gold == 25
                && info.location == (Point { x: 332, y: 270 })
    )));
}

#[test]
fn retained_zone_object_spawn_uses_object_aoi_not_actor_aoi() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 368, 270)));

    let outbounds = zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![monster_spawn_packet(5001, 1001, 352, 270)],
        now_ms: 0,
    });

    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info }
            if info.object_id == 5001
                && info.location == (Point { x: 352, y: 270 })
    )));
    assert!(!has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectPlayer { info } if info.object_id == 101
    )));
}

#[test]
fn retained_zone_object_remove_uses_object_visibility_not_actor_aoi() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 368, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![monster_spawn_packet(5001, 1001, 352, 270)],
        now_ms: 0,
    });

    let outbounds = zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectRemove { object_id: 5001 }],
        now_ms: 100,
    });

    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectRemove { object_id } if *object_id == 5001
    )));
}

#[test]
fn retained_zone_object_pickup_reaches_local_object_observer_without_visible_set() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectGold {
            info: ObjectGoldInfo {
                object_id: 8001,
                gold: 25,
                location: Point { x: 331, y: 270 },
            },
        }],
        now_ms: 0,
    });

    let outbounds = zone.handle(ZoneCommand::BroadcastPackets {
        session_id: second.clone(),
        owner_local_object_id: 1002,
        packets: vec![ServerPacket::IntelligentCreaturePickup { object_id: 8001 }],
        now_ms: 100,
    });

    assert!(has_packet(&outbounds, &first, |packet| matches!(
        packet,
        ServerPacket::IntelligentCreaturePickup { object_id } if *object_id == 8001
    )));
}

#[test]
fn retained_zone_object_state_updates_for_late_joiner() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![monster_spawn_packet(5001, 1001, 331, 270)],
        now_ms: 0,
    });
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![
            ServerPacket::ObjectWalk {
                movement: ObjectMovement {
                    object_id: 5001,
                    position: Point { x: 332, y: 270 },
                    direction: MirDirection::Right,
                },
            },
            ServerPacket::ObjectPoisoned {
                object_id: 5001,
                poison: 4,
            },
            ServerPacket::AddBuff {
                buff: ClientBuff {
                    buff_type: 9,
                    visible: true,
                    object_id: 5001,
                    expire_time: 10_000,
                    infinite: false,
                    paused: false,
                    stats: Vec::new(),
                    values: Vec::new(),
                },
            },
            ServerPacket::ObjectDied {
                info: ObjectDiedInfo {
                    object_id: 5001,
                    location: Point { x: 333, y: 271 },
                    direction: MirDirection::Left,
                    kind: 0,
                },
            },
        ],
        now_ms: 100,
    });

    let second = session("second");
    let outbounds = zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));

    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info }
            if info.object_id == 5001
                && info.location == (Point { x: 333, y: 271 })
                && info.direction == MirDirection::Left
                && info.dead
                && info.poison == 4
                && info.buffs == vec![9]
    )));
}

#[test]
fn retained_zone_object_health_updates_for_late_joiner() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![monster_spawn_packet(5001, 1001, 331, 270)],
        now_ms: 0,
    });
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectHealth {
            info: ObjectHealthInfo {
                object_id: 5001,
                percent: 64,
                expire: 90,
            },
        }],
        now_ms: 100,
    });
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectHealth {
            info: ObjectHealthInfo {
                object_id: 5001,
                percent: 42,
                expire: 70,
            },
        }],
        now_ms: 200,
    });

    let second = session("second");
    let outbounds = zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info } if info.object_id == 5001 && !info.dead
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info }
            if info.object_id == 5001 && info.percent == 42 && info.expire == 70
    )));
}

#[test]
fn retained_zone_object_health_updates_when_entering_object_aoi() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 348, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![monster_spawn_packet(5001, 1001, 331, 270)],
        now_ms: 0,
    });
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectHealth {
            info: ObjectHealthInfo {
                object_id: 5001,
                percent: 58,
                expire: 80,
            },
        }],
        now_ms: 100,
    });

    zone.handle(ZoneCommand::Walk {
        session_id: second.clone(),
        direction: MirDirection::Left,
        seq: 1,
        now_ms: 0,
    });
    let outbounds = zone.tick(0);

    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info } if info.object_id == 5001
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info }
            if info.object_id == 5001 && info.percent == 58 && info.expire == 80
    )));
}

#[test]
fn retained_zone_object_mana_updates_for_late_joiner() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![hero_spawn_packet(6001, "Scout", 331, 270)],
        now_ms: 0,
    });
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectMana {
            info: ObjectManaInfo {
                object_id: 6001,
                percent: 73,
            },
        }],
        now_ms: 100,
    });
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectMana {
            info: ObjectManaInfo {
                object_id: 6001,
                percent: 41,
            },
        }],
        now_ms: 200,
    });

    let second = session("second");
    let outbounds = zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectHero { info, owner_name }
            if info.object_id == 6001 && owner_name == "Scout"
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectMana { info } if info.object_id == 6001 && info.percent == 41
    )));
}

#[test]
fn retained_zone_object_zero_health_marks_dead_for_late_joiner() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![monster_spawn_packet(5001, 1001, 331, 270)],
        now_ms: 0,
    });
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectHealth {
            info: ObjectHealthInfo {
                object_id: 5001,
                percent: 0,
                expire: 0,
            },
        }],
        now_ms: 100,
    });

    let second = session("second");
    let outbounds = zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));

    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info } if info.object_id == 5001 && info.dead
    )));
}

#[test]
fn stale_retained_zone_object_spawn_after_death_stays_dead() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![monster_spawn_packet(5001, 1001, 331, 270)],
        now_ms: 0,
    });
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectDied {
            info: ObjectDiedInfo {
                object_id: 5001,
                location: Point { x: 333, y: 271 },
                direction: MirDirection::Left,
                kind: 0,
            },
        }],
        now_ms: 100,
    });

    let stale_outbounds = zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![monster_spawn_packet(5001, 1001, 331, 270)],
        now_ms: 200,
    });

    assert!(has_packet(&stale_outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info }
            if info.object_id == 5001
                && info.dead
                && info.location == (Point { x: 333, y: 271 })
                && info.direction == MirDirection::Left
    )));

    let third = session("third");
    let join_outbounds = zone.handle(ZoneCommand::Join(join("third", 103, "Late", 332, 270)));
    assert!(has_packet(&join_outbounds, &third, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info }
            if info.object_id == 5001
                && info.dead
                && info.location == (Point { x: 333, y: 271 })
                && info.direction == MirDirection::Left
    )));
}

#[test]
fn stale_retained_zone_object_movement_and_vitals_after_death_are_suppressed() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![monster_spawn_packet(5001, 1001, 331, 270)],
        now_ms: 0,
    });
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectDied {
            info: ObjectDiedInfo {
                object_id: 5001,
                location: Point { x: 331, y: 270 },
                direction: MirDirection::Down,
                kind: 0,
            },
        }],
        now_ms: 100,
    });

    let stale_outbounds = zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![
            ServerPacket::ObjectWalk {
                movement: ObjectMovement {
                    object_id: 5001,
                    position: Point { x: 335, y: 272 },
                    direction: MirDirection::Right,
                },
            },
            ServerPacket::ObjectHealth {
                info: ObjectHealthInfo {
                    object_id: 5001,
                    percent: 80,
                    expire: 30,
                },
            },
            ServerPacket::ObjectMana {
                info: ObjectManaInfo {
                    object_id: 5001,
                    percent: 90,
                },
            },
        ],
        now_ms: 200,
    });

    assert!(!has_packet(&stale_outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectWalk { movement } if movement.object_id == 5001
    )));
    assert!(!has_packet(&stale_outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info } if info.object_id == 5001 && info.percent == 80
    )));
    assert!(!has_packet(&stale_outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectMana { info } if info.object_id == 5001
    )));

    let third = session("third");
    let join_outbounds = zone.handle(ZoneCommand::Join(join("third", 103, "Late", 332, 270)));
    assert!(has_packet(&join_outbounds, &third, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info }
            if info.object_id == 5001
                && info.dead
                && info.location == (Point { x: 331, y: 270 })
                && info.direction == MirDirection::Down
    )));
    assert!(!has_packet(&join_outbounds, &third, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info } if info.object_id == 5001 && info.percent == 80
    )));
}

#[test]
fn retained_zone_object_health_does_not_increase_from_stale_runtime_packet() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![monster_spawn_packet(5001, 1001, 331, 270)],
        now_ms: 0,
    });
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectHealth {
            info: ObjectHealthInfo {
                object_id: 5001,
                percent: 42,
                expire: 70,
            },
        }],
        now_ms: 100,
    });
    let stale_outbounds = zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectHealth {
            info: ObjectHealthInfo {
                object_id: 5001,
                percent: 77,
                expire: 90,
            },
        }],
        now_ms: 200,
    });
    assert!(!has_packet(&stale_outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info } if info.object_id == 5001 && info.percent == 77
    )));

    let lower_outbounds = zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectHealth {
            info: ObjectHealthInfo {
                object_id: 5001,
                percent: 21,
                expire: 50,
            },
        }],
        now_ms: 300,
    });
    assert!(has_packet(&lower_outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info } if info.object_id == 5001 && info.percent == 21
    )));

    let third = session("third");
    let join_outbounds = zone.handle(ZoneCommand::Join(join("third", 103, "Late", 332, 270)));
    assert!(has_packet(&join_outbounds, &third, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info }
            if info.object_id == 5001 && info.percent == 21 && info.expire == 50
    )));
}

#[test]
fn zero_health_before_retained_spawn_marks_late_spawn_dead() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectHealth {
            info: ObjectHealthInfo {
                object_id: 5001,
                percent: 0,
                expire: 0,
            },
        }],
        now_ms: 100,
    });
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![monster_spawn_packet(5001, 1001, 331, 270)],
        now_ms: 200,
    });

    let second = session("second");
    let outbounds = zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info }
            if info.object_id == 5001
                && info.dead
                && info.location == (Point { x: 331, y: 270 })
    )));
}

#[test]
fn retained_zone_object_revive_updates_existing_dead_object() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![monster_spawn_packet(5001, 1001, 331, 270)],
        now_ms: 0,
    });
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectDied {
            info: ObjectDiedInfo {
                object_id: 5001,
                location: Point { x: 331, y: 270 },
                direction: MirDirection::Down,
                kind: 0,
            },
        }],
        now_ms: 100,
    });
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectRevived {
            info: ObjectRevivedInfo {
                object_id: 5001,
                effect: true,
            },
        }],
        now_ms: 200,
    });

    let second = session("second");
    let outbounds = zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info } if info.object_id == 5001 && !info.dead
    )));
}

#[test]
fn retained_zone_object_revive_before_spawn_suppresses_stale_dead_spawn() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectRevived {
            info: ObjectRevivedInfo {
                object_id: 5001,
                effect: true,
            },
        }],
        now_ms: 100,
    });
    let mut stale_dead_spawn = monster_spawn_packet(5001, 1001, 331, 270);
    if let ServerPacket::ObjectMonster { info } = &mut stale_dead_spawn {
        info.dead = true;
    }
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![stale_dead_spawn],
        now_ms: 200,
    });

    let second = session("second");
    let outbounds = zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info } if info.object_id == 5001 && !info.dead
    )));
}

#[test]
fn retained_zone_object_buffs_expire_for_observers_and_late_joiners() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![monster_spawn_packet(5001, 1001, 331, 270)],
        now_ms: 0,
    });
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::AddBuff {
            buff: ClientBuff {
                buff_type: 9,
                visible: true,
                object_id: 5001,
                expire_time: 250,
                infinite: false,
                paused: false,
                stats: Vec::new(),
                values: Vec::new(),
            },
        }],
        now_ms: 1_000,
    });

    let early_outbounds = zone.tick(1_249);
    assert!(!has_packet(&early_outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::RemoveBuff { object_id, buff_type }
            if *object_id == 5001 && *buff_type == 9
    )));

    let expired_outbounds = zone.tick(1_250);
    assert!(has_packet(&expired_outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::RemoveBuff { object_id, buff_type }
            if *object_id == 5001 && *buff_type == 9
    )));

    let third = session("third");
    let join_outbounds = zone.handle(ZoneCommand::Join(join("third", 103, "Late", 332, 270)));
    assert!(has_packet(&join_outbounds, &third, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info }
            if info.object_id == 5001 && info.buffs.is_empty()
    )));
}

#[test]
fn retained_zone_object_buff_payloads_are_replayed_for_late_joiner() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![monster_spawn_packet(5001, 1001, 331, 270)],
        now_ms: 0,
    });
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::AddBuff {
            buff: ClientBuff {
                buff_type: 9,
                visible: true,
                object_id: 5001,
                expire_time: 2_500,
                infinite: false,
                paused: false,
                stats: vec![UserItemStat { stat: 11, value: 4 }],
                values: vec![7, 8],
            },
        }],
        now_ms: 100,
    });
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::PauseBuff {
            object_id: 5001,
            buff_type: 9,
            paused: true,
        }],
        now_ms: 200,
    });

    let second = session("second");
    let outbounds = zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info }
            if info.object_id == 5001 && info.buffs == vec![9]
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::AddBuff { buff }
            if buff.object_id == 5001
                && buff.buff_type == 9
                && buff.visible
                && buff.paused
                && buff.stats.as_slice() == [UserItemStat { stat: 11, value: 4 }]
                && buff.values.as_slice() == [7, 8]
    )));
}

#[test]
fn retained_zone_object_remove_suppresses_late_join_spawn() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectGold {
            info: ObjectGoldInfo {
                object_id: 8001,
                gold: 25,
                location: Point { x: 332, y: 270 },
            },
        }],
        now_ms: 0,
    });
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectRemove { object_id: 8001 }],
        now_ms: 100,
    });

    let second = session("second");
    let outbounds = zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));

    assert!(!has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectGold { info } if info.object_id == 8001
    )));
}

#[test]
fn stale_retained_zone_object_spawn_after_remove_is_suppressed() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectGold {
            info: ObjectGoldInfo {
                object_id: 8001,
                gold: 25,
                location: Point { x: 332, y: 270 },
            },
        }],
        now_ms: 0,
    });
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectRemove { object_id: 8001 }],
        now_ms: 100,
    });

    let stale_outbounds = zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectGold {
            info: ObjectGoldInfo {
                object_id: 8001,
                gold: 25,
                location: Point { x: 332, y: 270 },
            },
        }],
        now_ms: 200,
    });

    assert!(!has_packet(&stale_outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectGold { info } if info.object_id == 8001
    )));

    let third = session("third");
    let join_outbounds = zone.handle(ZoneCommand::Join(join("third", 103, "Late", 332, 270)));
    assert!(!has_packet(&join_outbounds, &third, |packet| matches!(
        packet,
        ServerPacket::ObjectGold { info } if info.object_id == 8001
    )));
}

#[test]
fn retained_zone_drops_expire_on_zone_tick() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectGold {
            info: ObjectGoldInfo {
                object_id: 8001,
                gold: 25,
                location: Point { x: 332, y: 270 },
            },
        }],
        now_ms: 1_000,
    });

    let early_outbounds = zone.tick(540_999);
    assert!(!has_packet(&early_outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectRemove { object_id } if *object_id == 8001
    )));

    let expired_outbounds = zone.tick(541_000);
    assert!(has_packet(&expired_outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectRemove { object_id } if *object_id == 8001
    )));

    let third = session("third");
    let join_outbounds = zone.handle(ZoneCommand::Join(join("third", 103, "Late", 332, 270)));
    assert!(!has_packet(&join_outbounds, &third, |packet| matches!(
        packet,
        ServerPacket::ObjectGold { info } if info.object_id == 8001
    )));
}

#[test]
fn owner_leave_removes_owned_retained_zone_objects() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![monster_spawn_packet(5001, 1001, 331, 270)],
        now_ms: 0,
    });

    let leave_outbounds = zone.handle(ZoneCommand::Leave {
        session_id: first.clone(),
    });

    assert!(has_packet(&leave_outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectRemove { object_id } if *object_id == 101
    )));
    assert!(has_packet(&leave_outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectRemove { object_id } if *object_id == 5001
    )));

    let third = session("third");
    let join_outbounds = zone.handle(ZoneCommand::Join(join("third", 103, "Late", 332, 270)));
    assert!(!has_packet(&join_outbounds, &third, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info } if info.object_id == 5001
    )));
}

#[test]
fn player_movement_diffs_retained_zone_object_visibility() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![monster_spawn_packet(5001, 1001, 331, 270)],
        now_ms: 0,
    });

    let join_outbounds = zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 348, 270)));
    assert!(!has_packet(&join_outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info } if info.object_id == 5001
    )));

    zone.handle(ZoneCommand::Walk {
        session_id: second.clone(),
        direction: MirDirection::Left,
        seq: 1,
        now_ms: 0,
    });
    let enter_outbounds = zone.tick(0);
    assert!(has_packet(&enter_outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info } if info.object_id == 5001
    )));

    zone.handle(ZoneCommand::Walk {
        session_id: second.clone(),
        direction: MirDirection::Right,
        seq: 2,
        now_ms: 0,
    });
    let leave_outbounds = zone.tick(600);
    assert!(has_packet(&leave_outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectRemove { object_id } if *object_id == 5001
    )));
}

#[test]
fn player_harvest_packets_use_zone_object_id_for_observers() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));

    let outbounds = zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![
            ServerPacket::ObjectHarvest {
                movement: ObjectMovement {
                    object_id: 1001,
                    position: Point { x: 1, y: 1 },
                    direction: MirDirection::Left,
                },
            },
            ServerPacket::ObjectHarvested {
                movement: ObjectMovement {
                    object_id: 1001,
                    position: Point { x: 2, y: 2 },
                    direction: MirDirection::Right,
                },
            },
        ],
        now_ms: 0,
    });

    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectHarvest { movement }
            if movement.object_id == 101
                && movement.position == (Point { x: 330, y: 270 })
                && movement.direction == MirDirection::Down
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectHarvested { movement }
            if movement.object_id == 101
                && movement.position == (Point { x: 330, y: 270 })
                && movement.direction == MirDirection::Down
    )));

    let third = session("third");
    let join_outbounds = zone.handle(ZoneCommand::Join(join("third", 103, "Late", 332, 270)));
    assert!(has_packet(&join_outbounds, &third, |packet| matches!(
        packet,
        ServerPacket::ObjectPlayer { info } if info.object_id == 101 && !info.dead
    )));
}

#[test]
fn retained_zone_object_harvested_is_canonical_for_late_joiners() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![monster_spawn_packet(5001, 1001, 331, 270)],
        now_ms: 0,
    });

    let harvested = ServerPacket::ObjectHarvested {
        movement: ObjectMovement {
            object_id: 5001,
            position: Point { x: 331, y: 270 },
            direction: MirDirection::Left,
        },
    };
    let harvest_outbounds = zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![harvested.clone()],
        now_ms: 100,
    });
    assert!(has_packet(&harvest_outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectHarvested { movement }
            if movement.object_id == 5001
                && movement.position == (Point { x: 331, y: 270 })
                && movement.direction == MirDirection::Left
    )));

    let duplicate_outbounds = zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![harvested],
        now_ms: 200,
    });
    assert!(!has_packet(
        &duplicate_outbounds,
        &second,
        |packet| matches!(
            packet,
            ServerPacket::ObjectHarvested { movement } if movement.object_id == 5001
        )
    ));

    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![monster_spawn_packet(5001, 1001, 329, 269)],
        now_ms: 300,
    });
    let third = session("third");
    let join_outbounds = zone.handle(ZoneCommand::Join(join("third", 103, "Late", 332, 270)));
    assert!(has_packet(&join_outbounds, &third, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info }
            if info.object_id == 5001
                && info.dead
                && info.location == (Point { x: 331, y: 270 })
                && info.direction == MirDirection::Left
    )));
}

#[test]
fn retained_zone_object_harvested_before_spawn_keeps_late_spawn_dead() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectHarvested {
            movement: ObjectMovement {
                object_id: 5001,
                position: Point { x: 331, y: 270 },
                direction: MirDirection::Right,
            },
        }],
        now_ms: 100,
    });
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![monster_spawn_packet(5001, 1001, 333, 272)],
        now_ms: 200,
    });

    let second = session("second");
    let outbounds = zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info }
            if info.object_id == 5001
                && info.dead
                && info.location == (Point { x: 331, y: 270 })
                && info.direction == MirDirection::Right
    )));
}

#[test]
fn later_joiner_receives_rebased_player_buffs_in_object_player() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::AddBuff {
            buff: ClientBuff {
                buff_type: 7,
                visible: true,
                object_id: 1001,
                expire_time: 1234,
                infinite: false,
                paused: false,
                stats: Vec::new(),
                values: vec![5],
            },
        }],
        now_ms: 0,
    });

    let second = session("second");
    let outbounds = zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));

    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectPlayer { info }
            if info.object_id == 101 && info.buffs == vec![7]
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::AddBuff { buff }
            if buff.object_id == 101 && buff.buff_type == 7 && buff.values.as_slice() == [5]
    )));
}

#[test]
fn zone_expires_player_buffs_for_observers_and_late_joiners() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::AddBuff {
            buff: ClientBuff {
                buff_type: 7,
                visible: true,
                object_id: 1001,
                expire_time: 250,
                infinite: false,
                paused: false,
                stats: Vec::new(),
                values: vec![5],
            },
        }],
        now_ms: 1_000,
    });

    let early_outbounds = zone.tick(1_249);
    assert!(!has_packet(&early_outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::RemoveBuff { object_id, buff_type }
            if *object_id == 101 && *buff_type == 7
    )));

    let expired_outbounds = zone.tick(1_250);
    assert!(has_packet(&expired_outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::RemoveBuff { object_id, buff_type }
            if *object_id == 101 && *buff_type == 7
    )));

    let third = session("third");
    let join_outbounds = zone.handle(ZoneCommand::Join(join("third", 103, "Late", 334, 270)));
    let third_packets = packets_for(&join_outbounds, &third);
    assert!(third_packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectPlayer { info } if info.object_id == 101 && info.buffs.is_empty()
    )));
    assert!(!third_packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::AddBuff { buff } if buff.object_id == 101 && buff.buff_type == 7
    )));
}

#[test]
fn later_joiners_receive_player_hidden_dead_and_effect_state() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![
            ServerPacket::ObjectHidden {
                object_id: 1001,
                hidden: true,
            },
            ServerPacket::ObjectEffect {
                info: ObjectEffectInfo {
                    object_id: 1001,
                    effect: 12,
                    effect_type: 2,
                    delay_time: 0,
                    time: 500,
                },
            },
            ServerPacket::ObjectDied {
                info: ObjectDiedInfo {
                    object_id: 1001,
                    location: Point { x: 330, y: 270 },
                    direction: MirDirection::Left,
                    kind: 0,
                },
            },
        ],
        now_ms: 1_000,
    });

    let second = session("second");
    let hidden_outbounds = zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    assert!(has_packet(&hidden_outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectPlayer { info }
            if info.object_id == 101 && info.hidden && info.dead && info.effect == 12
    )));

    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![
            ServerPacket::ObjectRevived {
                info: ObjectRevivedInfo {
                    object_id: 1001,
                    effect: true,
                },
            },
            ServerPacket::ObjectHidden {
                object_id: 1001,
                hidden: false,
            },
            ServerPacket::ObjectEffect {
                info: ObjectEffectInfo {
                    object_id: 1001,
                    effect: 0,
                    effect_type: 0,
                    delay_time: 0,
                    time: 0,
                },
            },
        ],
        now_ms: 1_500,
    });

    let third = session("third");
    let visible_outbounds = zone.handle(ZoneCommand::Join(join("third", 103, "Late", 334, 270)));
    assert!(has_packet(&visible_outbounds, &third, |packet| matches!(
        packet,
        ServerPacket::ObjectPlayer { info }
            if info.object_id == 101 && !info.hidden && !info.dead && info.effect == 0
    )));
}

#[test]
fn later_joiners_receive_retained_player_visual_status() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first,
        owner_local_object_id: 1001,
        packets: vec![
            ServerPacket::PlayerUpdate {
                object_id: 1001,
                light: 4,
                weapon: 12,
                weapon_effect: 3,
                armour: 44,
                wing_effect: 2,
            },
            ServerPacket::ObjectColourChanged {
                object_id: 1001,
                name_colour_argb: -255,
            },
            ServerPacket::ObjectGuildNameChanged {
                object_id: 1001,
                guild_name: "Codex".to_string(),
            },
            ServerPacket::ObjectName {
                object_id: 1001,
                name: "ScoutRenamed".to_string(),
            },
            ServerPacket::ObjectPoisoned {
                object_id: 1001,
                poison: 3,
            },
            ServerPacket::ObjectLevelEffects {
                object_id: 1001,
                level_effects: 9,
            },
            ServerPacket::MountUpdate {
                object_id: 1001,
                mount_type: 5,
                riding_mount: true,
            },
            ServerPacket::FishingUpdate {
                object_id: 1001,
                fishing: true,
                progress_percent: 20,
                chance_percent: 5,
                fishing_point: Point { x: 331, y: 271 },
                found_fish: true,
            },
            ServerPacket::TransformUpdate {
                object_id: 1001,
                transform_type: 6,
            },
        ],
        now_ms: 0,
    });

    let second = session("second");
    let outbounds = zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));

    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectPlayer { info }
            if info.object_id == 101
                && info.name == "ScoutRenamed"
                && info.guild_name == "Codex"
                && info.name_colour_argb == -255
                && info.light == 4
                && info.weapon == 12
                && info.weapon_effect == 3
                && info.armour == 44
                && info.poison == 3
                && info.wing_effect == 2
                && info.mount_type == 5
                && info.riding_mount
                && info.fishing
                && info.transform_type == 6
                && info.level_effects == 9
    )));
}

#[test]
fn player_a_leaves_player_b_receives_object_remove() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));

    let outbounds = zone.handle(ZoneCommand::Leave { session_id: first });

    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectRemove { object_id } if *object_id == 101
    )));
}

#[test]
fn two_players_cannot_occupy_same_tile() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 331, 270)));

    zone.handle(ZoneCommand::Walk {
        session_id: first.clone(),
        direction: MirDirection::Right,
        seq: 1,
        now_ms: 0,
    });
    let outbounds = zone.tick(0);

    assert_eq!(zone.player_position(&first), Some(Point { x: 330, y: 270 }));
    assert!(has_packet(&outbounds, &first, |packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == (Point { x: 330, y: 270 })
                && location.direction == MirDirection::Down
    )));
    assert!(!has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectWalk { .. }
    )));
}

#[test]
fn retained_zone_npc_blocks_player_walk() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![npc_spawn_packet(7001, 331, 270)],
        now_ms: 0,
    });

    zone.handle(ZoneCommand::Walk {
        session_id: first.clone(),
        direction: MirDirection::Right,
        seq: 1,
        now_ms: 0,
    });
    let outbounds = zone.tick(0);

    assert_eq!(zone.player_position(&first), Some(Point { x: 330, y: 270 }));
    assert!(has_packet(&outbounds, &first, |packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == (Point { x: 330, y: 270 })
                && location.direction == MirDirection::Down
    )));
}

#[test]
fn living_native_monster_blocks_player_walk_into_its_tile() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, 331, 270),
        now_ms: 0,
    });

    zone.handle(ZoneCommand::Walk {
        session_id: first.clone(),
        direction: MirDirection::Right,
        seq: 1,
        now_ms: 0,
    });
    let outbounds = zone.tick(0);

    assert_eq!(zone.player_position(&first), Some(Point { x: 330, y: 270 }));
    assert!(has_packet(&outbounds, &first, |packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == (Point { x: 330, y: 270 })
                && location.direction == MirDirection::Down
    )));
    assert!(!has_packet(&outbounds, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectWalk { movement } if movement.object_id == 101
    )));
}

#[test]
fn retained_dead_monster_does_not_block_player_walk() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![monster_spawn_packet(5001, 1001, 331, 270)],
        now_ms: 0,
    });
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectDied {
            info: ObjectDiedInfo {
                object_id: 5001,
                location: Point { x: 331, y: 270 },
                direction: MirDirection::Down,
                kind: 0,
            },
        }],
        now_ms: 100,
    });

    zone.handle(ZoneCommand::Walk {
        session_id: first.clone(),
        direction: MirDirection::Right,
        seq: 1,
        now_ms: 0,
    });
    let outbounds = zone.tick(0);

    assert_eq!(zone.player_position(&first), Some(Point { x: 331, y: 270 }));
    assert!(has_packet(&outbounds, &first, |packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == (Point { x: 331, y: 270 })
                && location.direction == MirDirection::Right
    )));
}

#[test]
fn retained_object_remove_clears_blocking_occupancy() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![npc_spawn_packet(7001, 331, 270)],
        now_ms: 0,
    });
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::ObjectRemove { object_id: 7001 }],
        now_ms: 100,
    });

    zone.handle(ZoneCommand::Walk {
        session_id: first.clone(),
        direction: MirDirection::Right,
        seq: 1,
        now_ms: 0,
    });
    let outbounds = zone.tick(0);

    assert_eq!(zone.player_position(&first), Some(Point { x: 331, y: 270 }));
    assert!(has_packet(&outbounds, &first, |packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == (Point { x: 331, y: 270 })
    )));
}

#[test]
fn shared_object_action_uses_zone_object_aoi_not_same_map() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    let far = session("far");

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::Join(join("far", 103, "Far", 380, 270)));
    zone.handle(ZoneCommand::SyncSharedObjects {
        session_id: first.clone(),
        packets: vec![monster_spawn_packet(9001, 0, 331, 270)],
        now_ms: 0,
    });

    let outbounds = zone.handle(ZoneCommand::BroadcastSharedObjectPackets {
        session_id: first,
        local_self_object_id: Some(1),
        packets: vec![ServerPacket::ObjectAttack {
            info: ObjectAttackInfo {
                object_id: 9001,
                location: Point { x: 331, y: 270 },
                direction: MirDirection::Left,
                spell: 0,
                level: 0,
                attack_type: 0,
            },
        }],
        now_ms: 10,
    });

    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info }
            if info.object_id == 9001 && info.location == (Point { x: 331, y: 270 })
    )));
    assert!(!has_packet(&outbounds, &far, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info } if info.object_id == 9001
    )));
}

#[test]
fn shared_object_strike_rebases_local_self_results_to_zone_player() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::SyncSharedObjects {
        session_id: first.clone(),
        packets: vec![monster_spawn_packet(9001, 0, 331, 270)],
        now_ms: 0,
    });

    let outbounds = zone.handle(ZoneCommand::BroadcastSharedObjectPackets {
        session_id: first,
        local_self_object_id: Some(1),
        packets: vec![
            ServerPacket::ObjectStruck {
                info: ObjectStruckInfo {
                    object_id: 1,
                    attacker_id: 9001,
                    location: Point { x: 330, y: 270 },
                    direction: MirDirection::Down,
                },
            },
            ServerPacket::ObjectHealth {
                info: ObjectHealthInfo {
                    object_id: 1,
                    percent: 80,
                    expire: 0,
                },
            },
        ],
        now_ms: 10,
    });

    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectStruck { info }
            if info.object_id == 101 && info.attacker_id == 9001
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info }
            if info.object_id == 101 && info.percent == 80
    )));
    assert!(!has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info } if info.object_id == 1
    )));
}

#[test]
fn shared_object_health_updates_retained_state_for_late_joiner() {
    let mut zone = zone();
    let first = session("first");

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::SyncSharedObjects {
        session_id: first.clone(),
        packets: vec![monster_spawn_packet(9001, 0, 331, 270)],
        now_ms: 0,
    });
    zone.handle(ZoneCommand::BroadcastSharedObjectPackets {
        session_id: first,
        local_self_object_id: Some(1),
        packets: vec![ServerPacket::ObjectHealth {
            info: ObjectHealthInfo {
                object_id: 9001,
                percent: 35,
                expire: 40,
            },
        }],
        now_ms: 10,
    });

    let late = session("late");
    let outbounds = zone.handle(ZoneCommand::Join(join("late", 103, "Late", 332, 271)));

    assert!(has_packet(&outbounds, &late, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info } if info.object_id == 9001
    )));
    assert!(has_packet(&outbounds, &late, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info }
            if info.object_id == 9001 && info.percent == 35 && info.expire == 40
    )));
}

#[test]
fn zone_native_monster_combat_kill_and_drop_are_authoritative() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    let spawn = zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, 331, 270),
        now_ms: 0,
    });

    assert!(has_packet(&spawn, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info }
            if info.object_id == 9100 && info.name == "Field Wasp" && !info.dead
    )));
    assert!(has_packet(&spawn, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info } if info.object_id == 9100
    )));

    let attack_launch = zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: first.clone(),
        object_id: 9100,
        direction: MirDirection::Right,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 7,
        now_ms: 10,
    });

    for target in [&first, &second] {
        assert!(has_packet(&attack_launch, target, |packet| matches!(
            packet,
            ServerPacket::ObjectAttack { info }
                if info.object_id == 101 && info.direction == MirDirection::Right
        )));
        assert!(!has_packet(&attack_launch, target, |packet| matches!(
            packet,
            ServerPacket::ObjectStruck { .. } | ServerPacket::ObjectHealth { .. }
        )));
    }

    let struck = zone.tick(10);
    for target in [&first, &second] {
        assert!(has_packet(&struck, target, |packet| matches!(
            packet,
            ServerPacket::ObjectStruck { info }
                if info.object_id == 9100 && info.attacker_id == 101
        )));
        assert!(has_packet(&struck, target, |packet| matches!(
            packet,
            ServerPacket::DamageIndicator { damage, object_id, .. }
                if *object_id == 9100 && *damage == 7
        )));
        assert!(has_packet(&struck, target, |packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info }
                if info.object_id == 9100 && info.percent == 65
        )));
    }
    assert!(!struck
        .iter()
        .any(|outbound| matches!(outbound, ZoneOutbound::MonsterKillAward { .. })));

    let kill_launch = zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: first.clone(),
        object_id: 9100,
        direction: MirDirection::Right,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 99,
        now_ms: 620,
    });

    for target in [&first, &second] {
        assert!(has_packet(&kill_launch, target, |packet| matches!(
            packet,
            ServerPacket::ObjectAttack { info }
                if info.object_id == 101 && info.direction == MirDirection::Right
        )));
        assert!(!has_packet(&kill_launch, target, |packet| matches!(
            packet,
            ServerPacket::ObjectDied { .. } | ServerPacket::ObjectGold { .. }
        )));
    }

    let killed = zone.tick(620);
    for target in [&first, &second] {
        assert!(has_packet(&killed, target, |packet| matches!(
            packet,
            ServerPacket::ObjectDied { info }
                if info.object_id == 9100 && info.location == (Point { x: 331, y: 270 })
        )));
        assert!(has_packet(&killed, target, |packet| matches!(
            packet,
            ServerPacket::ObjectGold { info }
                if info.object_id == 9200 && info.gold == 8 && info.location == (Point { x: 331, y: 270 })
        )));
    }
    assert!(!has_packet(&killed, &first, |packet| matches!(
        packet,
        ServerPacket::GainExperience { .. }
    )));
    assert!(killed.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::MonsterKillAward { session_id, award }
            if session_id == &first
                && award.monster_object_id == 9100
                && award.monster_name == "Field Wasp"
                && award.experience == 6
                && award.drops.iter().any(|drop| drop.object_id == 9200
                    && drop.owner_object_id == Some(101)
                    && drop.ownership_remaining_ticks == Some(100))
    )));
    assert!(zone.has_ground_drop(9200));

    let blocked = zone.handle(ZoneCommand::ClaimGroundDrop {
        session_id: second.clone(),
        object_id: Some(9200),
        target: Point { x: 331, y: 270 },
        group_members: Vec::new(),
        now_ms: 625,
    });
    assert!(has_packet(&blocked, &second, |packet| matches!(
        packet,
        ServerPacket::Chat { message, .. } if message == "server.CannotPickupNotOwner"
    )));

    let claimed = zone.handle(ZoneCommand::ClaimGroundDrop {
        session_id: first.clone(),
        object_id: Some(9200),
        target: Point { x: 331, y: 270 },
        group_members: Vec::new(),
        now_ms: 630,
    });
    assert!(claimed.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::GroundDropClaimed { session_id, drop }
            if session_id == &first && drop.object_id == 9200
    )));
}

#[test]
fn zone_native_player_range_attack_damages_monster_authoritatively() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_neutral_monster_spawn(9100, "Royal_Guard", 1, 335, 270),
        now_ms: 0,
    });

    let launch = zone.handle(ZoneCommand::PlayerRangeAttackObject {
        session_id: first.clone(),
        object_id: 9100,
        direction: MirDirection::Right,
        target: Point { x: 335, y: 270 },
        spell: Spell::Focus,
        level: 3,
        attack_type: 0,
        damage: 8,
        now_ms: 10,
    });

    assert!(has_packet(&launch, &first, |packet| matches!(
        packet,
        ServerPacket::RangeAttack {
            target_id,
            target,
            spell
        } if *target_id == 9100
            && target == &(Point { x: 335, y: 270 })
            && *spell == Spell::Focus
    )));
    for target in [&first, &second] {
        assert!(has_packet(&launch, target, |packet| matches!(
            packet,
            ServerPacket::ObjectRangeAttack { info }
                if info.object_id == 101
                    && info.target_id == 9100
                    && info.target == (Point { x: 335, y: 270 })
                    && info.spell == Spell::Focus as u8
                    && info.level == 3
        )));
        assert!(!has_packet(&launch, target, |packet| matches!(
            packet,
            ServerPacket::ObjectStruck { .. } | ServerPacket::ObjectHealth { .. }
        )));
    }

    let struck = zone.tick(10);
    for target in [&first, &second] {
        assert!(has_packet(&struck, target, |packet| matches!(
            packet,
            ServerPacket::ObjectStruck { info }
                if info.object_id == 9100 && info.attacker_id == 101
        )));
        assert!(has_packet(&struck, target, |packet| matches!(
            packet,
            ServerPacket::DamageIndicator { object_id, damage, .. }
                if *object_id == 9100 && *damage == 8
        )));
        assert!(has_packet(&struck, target, |packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info }
                if info.object_id == 9100 && info.percent == 60
        )));
    }
}

#[test]
fn zone_native_player_range_attack_respects_attack_action_window() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_neutral_monster_spawn(9100, "Royal_Guard", 1, 335, 270),
        now_ms: 0,
    });

    let first_launch = zone.handle(ZoneCommand::PlayerRangeAttackObject {
        session_id: first.clone(),
        object_id: 9100,
        direction: MirDirection::Right,
        target: Point { x: 335, y: 270 },
        spell: Spell::Focus,
        level: 1,
        attack_type: 0,
        damage: 1,
        now_ms: 10,
    });
    assert!(has_packet(&first_launch, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectRangeAttack { info }
            if info.object_id == 101 && info.target_id == 9100
    )));
    let _ = zone.tick(10);

    let early = zone.handle(ZoneCommand::PlayerRangeAttackObject {
        session_id: first.clone(),
        object_id: 9100,
        direction: MirDirection::Right,
        target: Point { x: 335, y: 270 },
        spell: Spell::Focus,
        level: 1,
        attack_type: 0,
        damage: 1,
        now_ms: 100,
    });
    assert!(has_packet(&early, &first, |packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == (Point { x: 330, y: 270 })
    )));
    for target in [&first, &second] {
        assert!(!has_packet(&early, target, |packet| matches!(
            packet,
            ServerPacket::RangeAttack { .. } | ServerPacket::ObjectRangeAttack { .. }
        )));
    }
    let early_tick = zone.tick(100);
    for target in [&first, &second] {
        assert!(!has_packet(&early_tick, target, |packet| matches!(
            packet,
            ServerPacket::ObjectStruck { .. }
        )));
    }

    let ready = zone.handle(ZoneCommand::PlayerRangeAttackObject {
        session_id: first.clone(),
        object_id: 9100,
        direction: MirDirection::Right,
        target: Point { x: 335, y: 270 },
        spell: Spell::Focus,
        level: 1,
        attack_type: 0,
        damage: 1,
        now_ms: 610,
    });
    assert!(has_packet(&ready, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectRangeAttack { info }
            if info.object_id == 101 && info.target_id == 9100
    )));
}

#[test]
fn zone_native_player_magic_damages_monster_and_projects_authoritatively() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_neutral_monster_spawn(9100, "Royal_Guard", 1, 334, 270),
        now_ms: 0,
    });

    let launch = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::FireBall,
        direction: MirDirection::Right,
        target: Point { x: 334, y: 270 },
        cast: true,
        level: 2,
        damage: 9,
        mp_cost: 7,
        cooldown_ms: 500,
        now_ms: 20,
    });

    assert!(has_packet(&launch, &first, |packet| matches!(
        packet,
        ServerPacket::Magic {
            spell,
            target_id,
            target,
            cast,
            level,
            ..
        } if *spell == Spell::FireBall
            && *target_id == 9100
            && target == &(Point { x: 334, y: 270 })
            && *cast
            && *level == 2
    )));
    for target in [&first, &second] {
        assert!(has_packet(&launch, target, |packet| matches!(
            packet,
            ServerPacket::ObjectMagic {
                object_id,
                spell,
                target_id,
                target,
                cast,
                level,
                ..
            } if *object_id == 101
                && *spell == Spell::FireBall
                && *target_id == 9100
                && target == &(Point { x: 334, y: 270 })
                && *cast
                && *level == 2
        )));
        assert!(has_packet(&launch, target, |packet| matches!(
            packet,
            ServerPacket::ObjectProjectile {
                spell,
                source_id,
                destination_id
            } if *spell == Spell::FireBall && *source_id == 101 && *destination_id == 9100
        )));
        assert!(!has_packet(&launch, target, |packet| matches!(
            packet,
            ServerPacket::ObjectStruck { .. } | ServerPacket::ObjectHealth { .. }
        )));
    }

    let struck = zone.tick(20);
    for target in [&first, &second] {
        assert!(has_packet(&struck, target, |packet| matches!(
            packet,
            ServerPacket::ObjectStruck { info }
                if info.object_id == 9100 && info.attacker_id == 101
        )));
        assert!(has_packet(&struck, target, |packet| matches!(
            packet,
            ServerPacket::DamageIndicator { object_id, damage, .. }
                if *object_id == 9100 && *damage == 9
        )));
        assert!(has_packet(&struck, target, |packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info }
                if info.object_id == 9100 && info.percent == 55
        )));
    }
}

#[test]
fn zone_native_player_magic_subtracts_monster_magic_armour() {
    // A monster with authoritative MAC must mitigate incoming attack-magic with
    // Random(MinMAC,MaxMAC) — mirroring how the physical path subtracts AC. Here
    // FireBall deals 9, the monster's MAC range is a flat 3, so the zone applies
    // 9 - 3 = 6 against a 20-HP monster -> 70% health, not the raw 9 (= 55%).
    let mut zone = zone();
    let first = session("first");
    let attacker = first.clone();

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn_with_defense(
            9100,
            334,
            270,
            20,
            ZoneMonsterDefense {
                min_mac: 3,
                max_mac: 3,
                ..Default::default()
            },
        ),
        now_ms: 0,
    });

    zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::FireBall,
        direction: MirDirection::Right,
        target: Point { x: 334, y: 270 },
        cast: true,
        level: 2,
        damage: 9,
        mp_cost: 7,
        cooldown_ms: 500,
        now_ms: 20,
    });
    let struck = zone.tick(20);

    // 9 (spell) - 3 (rolled MAC) = 6 damage, leaving 14/20 HP = 70%.
    assert_eq!(damage_indicator_for(&struck, 9100), Some(6));
    assert!(has_packet(&struck, &attacker, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info }
            if info.object_id == 9100 && info.percent == 70
    )));
}

#[test]
fn zone_native_player_area_magic_damages_secondary_monsters_authoritatively() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, 334, 270),
        now_ms: 0,
    });
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9101, 335, 270),
        now_ms: 0,
    });
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9102, 337, 270),
        now_ms: 0,
    });

    let launch = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::FireBang,
        direction: MirDirection::Right,
        target: Point { x: 334, y: 270 },
        cast: true,
        level: 2,
        damage: 4,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });

    assert!(has_packet(&launch, &first, |packet| matches!(
        packet,
        ServerPacket::Magic {
            spell,
            target_id,
            secondary_target_ids,
            ..
        } if *spell == Spell::FireBang
            && *target_id == 9100
            && secondary_target_ids == &vec![9101]
    )));
    for target in [&first, &second] {
        assert!(has_packet(&launch, target, |packet| matches!(
            packet,
            ServerPacket::ObjectMagic {
                spell,
                target_id,
                secondary_target_ids,
                ..
            } if *spell == Spell::FireBang
                && *target_id == 9100
                && secondary_target_ids == &vec![9101]
        )));
    }

    let resolved = zone.tick(20);
    for target in [&first, &second] {
        assert!(has_packet(&resolved, target, |packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info }
                if info.object_id == 9100 && info.percent == 80
        )));
        assert!(has_packet(&resolved, target, |packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info }
                if info.object_id == 9101 && info.percent == 80
        )));
        assert!(!has_packet(&resolved, target, |packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info } if info.object_id == 9102
        )));
    }
}

#[test]
fn zone_native_player_meteor_shower_damages_primary_and_secondary_monsters() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    let target = Point { x: 334, y: 270 };

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, target.x, target.y),
        now_ms: 0,
    });
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9101, target.x + 1, target.y),
        now_ms: 0,
    });
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9102, target.x, target.y + 2),
        now_ms: 0,
    });
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9103, target.x + 5, target.y),
        now_ms: 0,
    });

    let launch = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::MeteorShower,
        direction: MirDirection::Right,
        target: target.clone(),
        cast: true,
        level: 2,
        damage: 8,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });

    assert!(has_packet(&launch, &first, |packet| matches!(
        packet,
        ServerPacket::Magic {
            spell,
            target_id,
            secondary_target_ids,
            ..
        } if *spell == Spell::MeteorShower
            && *target_id == 9100
            && secondary_target_ids == &vec![9101, 9102]
    )));
    for target_session in [&first, &second] {
        assert!(has_packet(&launch, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectMagic {
                spell,
                target_id,
                secondary_target_ids,
                ..
            } if *spell == Spell::MeteorShower
                && *target_id == 9100
                && secondary_target_ids == &vec![9101, 9102]
        )));
    }

    let resolved = zone.tick(20);
    for target_session in [&first, &second] {
        assert!(has_packet(&resolved, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info }
                if info.object_id == 9100 && info.percent == 60
        )));
        assert!(has_packet(&resolved, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info }
                if info.object_id == 9101 && info.percent == 80
        )));
        assert!(has_packet(&resolved, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info }
                if info.object_id == 9102 && info.percent == 80
        )));
        assert!(!has_packet(&resolved, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info } if info.object_id == 9103
        )));
    }
}

#[test]
fn zone_native_player_fire_bounce_chains_projectiles_and_damage() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    let target = Point { x: 334, y: 270 };

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 324, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 326, 270)));
    for (object_id, x) in [(9100, 334), (9101, 336), (9102, 338), (9103, 344)] {
        zone.handle(ZoneCommand::SpawnMonster {
            session_id: first.clone(),
            monster: native_monster_spawn(object_id, x, 270),
            now_ms: 0,
        });
    }

    let launch = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::FireBounce,
        direction: MirDirection::Right,
        target: target.clone(),
        cast: true,
        level: 2,
        damage: 8,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });

    for target_session in [&first, &second] {
        assert!(has_packet(&launch, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectProjectile {
                spell,
                source_id,
                destination_id
            } if *spell == Spell::FireBounce
                && *source_id == 101
                && *destination_id == 9100
        )));
    }

    let primary = zone.tick(20);
    for target_session in [&first, &second] {
        assert!(has_packet(&primary, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info }
                if info.object_id == 9100 && info.percent == 60
        )));
    }

    let second_bounce = zone.tick(120);
    for target_session in [&first, &second] {
        assert!(has_packet(
            &second_bounce,
            target_session,
            |packet| matches!(
                packet,
                ServerPacket::ObjectProjectile {
                    spell,
                    source_id,
                    destination_id
                } if *spell == Spell::FireBounce
                    && *source_id == 9100
                    && *destination_id == 9101
            )
        ));
        assert!(has_packet(
            &second_bounce,
            target_session,
            |packet| matches!(
                packet,
                ServerPacket::ObjectHealth { info }
                    if info.object_id == 9101 && info.percent == 60
            )
        ));
    }

    let third_bounce = zone.tick(220);
    for target_session in [&first, &second] {
        assert!(has_packet(
            &third_bounce,
            target_session,
            |packet| matches!(
                packet,
                ServerPacket::ObjectProjectile {
                    spell,
                    source_id,
                    destination_id
                } if *spell == Spell::FireBounce
                    && *source_id == 9101
                    && *destination_id == 9102
            )
        ));
        assert!(has_packet(
            &third_bounce,
            target_session,
            |packet| matches!(
                packet,
                ServerPacket::ObjectHealth { info }
                    if info.object_id == 9102 && info.percent == 60
            )
        ));
        assert!(!has_packet(
            &third_bounce,
            target_session,
            |packet| matches!(
                packet,
                ServerPacket::ObjectHealth { info } if info.object_id == 9103
            )
        ));
    }
}

#[test]
fn zone_native_player_firewall_spawns_ground_spell_and_ticks_damage() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    let target = Point { x: 334, y: 270 };

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, target.x, target.y),
        now_ms: 0,
    });
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9101, 337, 270),
        now_ms: 0,
    });

    let launch = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::FireWall,
        direction: MirDirection::Right,
        target: target.clone(),
        cast: true,
        level: 2,
        damage: 4,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });

    for target_session in [&first, &second] {
        assert!(has_packet(&launch, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectMagic {
                spell,
                target_id,
                target: packet_target,
                cast,
                ..
            } if *spell == Spell::FireWall
                && *target_id == 9100
                && packet_target == &target
                && *cast
        )));
        assert!(!has_packet(&launch, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectSpell { .. } | ServerPacket::ObjectHealth { .. }
        )));
    }

    let early = zone.tick(519);
    for target_session in [&first, &second] {
        assert!(!has_packet(&early, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectSpell { .. } | ServerPacket::ObjectHealth { .. }
        )));
    }

    let tick = zone.tick(520);
    for target_session in [&first, &second] {
        let spell_locations = packets_for(&tick, target_session)
            .into_iter()
            .filter_map(|packet| match packet {
                ServerPacket::ObjectSpell { info } if info.spell == Spell::FireWall => {
                    Some(info.location)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(spell_locations.len(), 5);
        assert!(spell_locations.contains(&target));
        assert!(spell_locations.contains(&Point { x: 334, y: 269 }));
        assert!(spell_locations.contains(&Point { x: 335, y: 270 }));
        assert!(spell_locations.contains(&Point { x: 334, y: 271 }));
        assert!(spell_locations.contains(&Point { x: 333, y: 270 }));
        assert!(has_packet(&tick, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectStruck { info }
                if info.object_id == 9100 && info.attacker_id == 101
        )));
        assert!(has_packet(&tick, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info }
                if info.object_id == 9100 && info.percent == 80
        )));
        assert!(!has_packet(&tick, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info } if info.object_id == 9101
        )));
    }
}

#[test]
fn zone_native_player_firewall_subtracts_monster_magic_armour() {
    // Ground/AoE attack spells mitigate their direct hit with monster MAC, just
    // like the single-target cast. FireWall ticks 4 damage; the monster's flat
    // MAC 2 leaves 4 - 2 = 2 per tick, so a 20-HP monster drops to 18/20 = 90%
    // on the first tick (vs 80% with the raw, un-mitigated 4).
    let mut zone = zone();
    let first = session("first");
    let target = Point { x: 334, y: 270 };

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn_with_defense(
            9100,
            target.x,
            target.y,
            20,
            ZoneMonsterDefense {
                min_mac: 2,
                max_mac: 2,
                ..Default::default()
            },
        ),
        now_ms: 0,
    });

    zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::FireWall,
        direction: MirDirection::Right,
        target: target.clone(),
        cast: true,
        level: 2,
        damage: 4,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });

    zone.tick(519);
    let tick = zone.tick(520);
    assert_eq!(damage_indicator_for(&tick, 9100), Some(2));
    assert!(has_packet(&tick, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info }
            if info.object_id == 9100 && info.percent == 90
    )));
}

#[test]
fn zone_native_player_firewall_accepts_targetless_ground_cast() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    let target = Point { x: 334, y: 270 };

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));

    let launch = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 0,
        spell: Spell::FireWall,
        direction: MirDirection::Right,
        target: target.clone(),
        cast: true,
        level: 2,
        damage: 4,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });

    assert!(has_packet(&launch, &first, |packet| matches!(
        packet,
        ServerPacket::Magic {
            spell,
            target_id,
            target: packet_target,
            ..
        } if *spell == Spell::FireWall && *target_id == 0 && packet_target == &target
    )));
    for target_session in [&first, &second] {
        assert!(has_packet(&launch, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectMagic {
                spell,
                target_id,
                target: packet_target,
                ..
            } if *spell == Spell::FireWall && *target_id == 0 && packet_target == &target
        )));
    }

    let spell_tick = zone.tick(520);
    for target_session in [&first, &second] {
        assert!(has_packet(&spell_tick, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectSpell { info }
                if info.spell == Spell::FireWall && info.location == target
        )));
    }
}

#[test]
fn zone_native_player_blizzard_family_spawns_ground_spell_and_ticks_damage() {
    for spell in [Spell::Blizzard, Spell::MeteorStrike] {
        let mut zone = zone();
        let first = session("first");
        let second = session("second");
        let target = Point { x: 334, y: 270 };

        zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 324, 270)));
        zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 326, 270)));
        zone.handle(ZoneCommand::SpawnMonster {
            session_id: first.clone(),
            monster: native_monster_spawn(9100, target.x, target.y),
            now_ms: 0,
        });
        zone.handle(ZoneCommand::SpawnMonster {
            session_id: first.clone(),
            monster: native_monster_spawn(9101, target.x + 2, target.y + 2),
            now_ms: 0,
        });
        zone.handle(ZoneCommand::SpawnMonster {
            session_id: first.clone(),
            monster: native_monster_spawn(9102, target.x + 3, target.y),
            now_ms: 0,
        });

        let launch = zone.handle(ZoneCommand::PlayerCastMagic {
            session_id: first.clone(),
            object_id: 9100,
            spell,
            direction: MirDirection::Right,
            target: target.clone(),
            cast: true,
            level: 2,
            damage: 4,
            mp_cost: 0,
            cooldown_ms: 500,
            now_ms: 20,
        });

        for target_session in [&first, &second] {
            assert!(has_packet(&launch, target_session, |packet| matches!(
                packet,
                ServerPacket::ObjectMagic {
                    spell: packet_spell,
                    target_id,
                    target: packet_target,
                    cast,
                    ..
                } if *packet_spell == spell
                    && *target_id == 9100
                    && packet_target == &target
                    && *cast
            )));
            assert!(!has_packet(&launch, target_session, |packet| matches!(
                packet,
                ServerPacket::ObjectSpell { .. } | ServerPacket::ObjectHealth { .. }
            )));
        }

        let spell_spawn = zone.tick(520);
        for target_session in [&first, &second] {
            let spell_packets = packets_for(&spell_spawn, target_session)
                .into_iter()
                .filter_map(|packet| match packet {
                    ServerPacket::ObjectSpell { info } if info.spell == spell => Some(info),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(spell_packets.len(), 25);
            assert!(spell_packets
                .iter()
                .any(|info| info.location == target && info.param));
            assert!(!has_packet(
                &spell_spawn,
                target_session,
                |packet| matches!(packet, ServerPacket::ObjectHealth { .. })
            ));
        }

        let early_damage = zone.tick(1_319);
        for target_session in [&first, &second] {
            assert!(!has_packet(
                &early_damage,
                target_session,
                |packet| matches!(packet, ServerPacket::ObjectHealth { .. })
            ));
        }

        let damage_tick = zone.tick(1_320);
        for target_session in [&first, &second] {
            assert!(has_packet(&damage_tick, target_session, |packet| matches!(
                packet,
                ServerPacket::ObjectHealth { info }
                    if info.object_id == 9100 && info.percent == 80
            )));
            assert!(has_packet(&damage_tick, target_session, |packet| matches!(
                packet,
                ServerPacket::ObjectHealth { info }
                    if info.object_id == 9101 && info.percent == 80
            )));
            assert!(!has_packet(
                &damage_tick,
                target_session,
                |packet| matches!(
                    packet,
                    ServerPacket::ObjectHealth { info } if info.object_id == 9102
                )
            ));
        }
    }
}

#[test]
fn zone_native_player_poison_cloud_spawns_ground_spell_and_poisons_monsters() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    let target = Point { x: 334, y: 270 };

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 324, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 326, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, target.x, target.y),
        now_ms: 0,
    });
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9101, target.x + 2, target.y),
        now_ms: 0,
    });

    let launch = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::PoisonCloud,
        direction: MirDirection::Right,
        target: target.clone(),
        cast: true,
        level: 3,
        damage: 6,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });

    for target_session in [&first, &second] {
        assert!(has_packet(&launch, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectMagic {
                spell,
                target_id,
                target: packet_target,
                cast,
                ..
            } if *spell == Spell::PoisonCloud
                && *target_id == 9100
                && packet_target == &target
                && *cast
        )));
        assert!(!has_packet(&launch, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectSpell { .. } | ServerPacket::ObjectHealth { .. }
        )));
    }

    let tick = zone.tick(520);
    for target_session in [&first, &second] {
        assert!(has_packet(&tick, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectSpell { info }
                if info.spell == Spell::PoisonCloud && info.location == target
        )));
        assert!(has_packet(&tick, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info }
                if info.object_id == 9100 && info.percent == 70
        )));
        assert!(has_packet(&tick, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectPoisoned {
                object_id: 9100,
                poison: 1
            }
        )));
        assert!(!has_packet(&tick, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info } if info.object_id == 9101
        )));
    }
}

#[test]
fn zone_native_player_trap_spawns_object_and_roots_lower_level_monster() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    let target = Point { x: 334, y: 270 };

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, target.x, target.y),
        now_ms: 0,
    });

    let launch = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::Trap,
        direction: MirDirection::Right,
        target: target.clone(),
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });

    for target_session in [&first, &second] {
        assert!(has_packet(&launch, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectMagic {
                spell,
                target_id,
                target: packet_target,
                cast,
                ..
            } if *spell == Spell::Trap
                && *target_id == 9100
                && packet_target == &target
                && *cast
        )));
        assert!(!has_packet(&launch, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectSpell { .. } | ServerPacket::ObjectWalk { .. }
        )));
    }

    let spell_tick = zone.tick(520);
    for target_session in [&first, &second] {
        assert!(has_packet(&spell_tick, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectSpell { info }
                if info.spell == Spell::Trap
                    && info.location == target
                    && info.direction == MirDirection::Right
                    && info.param
        )));
        assert!(!has_packet(&spell_tick, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectWalk { movement } if movement.object_id == 9100
        )));
    }

    let rooted_tick = zone.tick(1_200);
    for target_session in [&first, &second] {
        assert!(!has_packet(
            &rooted_tick,
            target_session,
            |packet| matches!(
                packet,
                ServerPacket::ObjectWalk { movement } if movement.object_id == 9100
            )
        ));
    }
}

#[test]
fn zone_native_player_trap_hexagon_roots_area_and_spawns_ring_objects() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    let center = Point { x: 334, y: 270 };

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, center.x, center.y),
        now_ms: 0,
    });
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9101, center.x, center.y + 1),
        now_ms: 0,
    });

    let launch = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::TrapHexagon,
        direction: MirDirection::Right,
        target: center.clone(),
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });

    for target_session in [&first, &second] {
        assert!(has_packet(&launch, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectMagic {
                spell,
                target_id,
                target: packet_target,
                cast,
                ..
            } if *spell == Spell::TrapHexagon
                && *target_id == 9100
                && packet_target == &center
                && *cast
        )));
        assert!(!has_packet(&launch, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectSpell { .. } | ServerPacket::ObjectWalk { .. }
        )));
    }

    let spell_tick = zone.tick(520);
    for target_session in [&first, &second] {
        let ring_locations = packets_for(&spell_tick, target_session)
            .into_iter()
            .filter_map(|packet| match packet {
                ServerPacket::ObjectSpell { info } if info.spell == Spell::TrapHexagon => {
                    Some(info.location)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let expected_locations = [
            Point { x: 335, y: 268 },
            Point { x: 333, y: 268 },
            Point { x: 336, y: 269 },
            Point { x: 336, y: 271 },
            Point { x: 335, y: 272 },
            Point { x: 333, y: 272 },
            Point { x: 332, y: 269 },
            Point { x: 332, y: 271 },
        ];
        assert_eq!(ring_locations.len(), 8);
        assert!(expected_locations
            .iter()
            .all(|location| ring_locations.contains(location)));
    }

    let rooted_tick = zone.tick(1_200);
    for target_session in [&first, &second] {
        assert!(!has_packet(
            &rooted_tick,
            target_session,
            |packet| matches!(
                packet,
                ServerPacket::ObjectWalk { movement }
                    if movement.object_id == 9100 || movement.object_id == 9101
            )
        ));
    }
}

#[test]
fn zone_native_player_explosive_trap_spawns_front_row_and_detonates_once() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    let origin = Point { x: 333, y: 267 };
    let front = Point { x: 334, y: 267 };
    let upper_side = Point { x: 334, y: 266 };
    let lower_side = Point { x: 334, y: 268 };

    zone.handle(ZoneCommand::Join(join(
        "first", 101, "Scout", origin.x, origin.y,
    )));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 267)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, front.x, front.y),
        now_ms: 0,
    });
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9101, upper_side.x, upper_side.y),
        now_ms: 0,
    });
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9102, lower_side.x + 2, lower_side.y),
        now_ms: 0,
    });

    let launch = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 0,
        spell: Spell::ExplosiveTrap,
        direction: MirDirection::Right,
        target: front.clone(),
        cast: true,
        level: 2,
        damage: 4,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });

    for target_session in [&first, &second] {
        assert!(has_packet(&launch, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectMagic {
                spell,
                target_id: 0,
                target: packet_target,
                cast,
                ..
            } if *spell == Spell::ExplosiveTrap
                && packet_target == &front
                && *cast
        )));
        assert!(!has_packet(&launch, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectSpell { .. } | ServerPacket::ObjectHealth { .. }
        )));
    }

    let tick = zone.tick(520);
    for target_session in [&first, &second] {
        let trap_locations = packets_for(&tick, target_session)
            .into_iter()
            .filter_map(|packet| match packet {
                ServerPacket::ObjectSpell { info } if info.spell == Spell::ExplosiveTrap => {
                    Some((info.location, info.direction, info.param))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(trap_locations.len(), 3);
        assert!(trap_locations.contains(&(front.clone(), MirDirection::Right, false)));
        assert!(trap_locations.contains(&(upper_side.clone(), MirDirection::Right, false)));
        assert!(trap_locations.contains(&(lower_side.clone(), MirDirection::Right, false)));
        assert!(has_packet(&tick, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info }
                if info.object_id == 9100 && info.percent == 80
        )));
        assert!(has_packet(&tick, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info }
                if info.object_id == 9101 && info.percent == 80
        )));
        assert!(!has_packet(&tick, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info } if info.object_id == 9102
        )));
    }

    let later_tick = zone.tick(1_020);
    for target_session in [&first, &second] {
        assert!(!has_packet(&later_tick, target_session, |packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info }
                if info.object_id == 9100 || info.object_id == 9101
        )));
    }
}

#[test]
fn zone_native_player_poison_shot_ticks_green_damage_and_awards_kill() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_neutral_monster_spawn(9100, "Royal_Guard", 1, 334, 270),
        now_ms: 0,
    });

    let launch = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::PoisonShot,
        direction: MirDirection::Right,
        target: Point { x: 334, y: 270 },
        cast: true,
        level: 2,
        damage: 5,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });

    for target in [&first, &second] {
        assert!(has_packet(&launch, target, |packet| matches!(
            packet,
            ServerPacket::ObjectPoisoned { object_id, poison }
                if *object_id == 9100 && *poison == 1
        )));
    }

    let direct_hit = zone.tick(20);
    for target in [&first, &second] {
        assert!(has_packet(&direct_hit, target, |packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info }
                if info.object_id == 9100 && info.percent == 75
        )));
    }

    let poison_tick = zone.tick(2_020);
    for target in [&first, &second] {
        assert!(has_packet(&poison_tick, target, |packet| matches!(
            packet,
            ServerPacket::DamageIndicator { object_id, damage, .. }
                if *object_id == 9100 && *damage == 3
        )));
        assert!(has_packet(&poison_tick, target, |packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info }
                if info.object_id == 9100 && info.percent == 60
        )));
        assert!(!has_packet(&poison_tick, target, |packet| matches!(
            packet,
            ServerPacket::ObjectStruck { .. }
        )));
    }

    for now_ms in [4_020, 6_020, 8_020] {
        zone.tick(now_ms);
    }
    let killed = zone.tick(10_020);
    for target in [&first, &second] {
        assert!(has_packet(&killed, target, |packet| matches!(
            packet,
            ServerPacket::ObjectDied { info }
                if info.object_id == 9100
        )));
    }
    assert!(killed.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::MonsterKillAward { session_id, award }
            if session_id == &first
                && award.monster_object_id == 9100
                && award.drops.iter().any(|drop| drop.object_id == 9200)
    )));
    assert!(zone.has_ground_drop(9200));
}

#[test]
fn zone_native_player_poison_shot_applies_visible_arrow_buff() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, 334, 270),
        now_ms: 0,
    });

    let launch = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::PoisonShot,
        direction: MirDirection::Right,
        target: Point { x: 334, y: 270 },
        cast: true,
        level: 2,
        damage: 5,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });

    for target in [&first, &second] {
        assert!(has_packet(&launch, target, |packet| matches!(
            packet,
            ServerPacket::AddBuff { buff }
                if buff.object_id == 101
                    && buff.buff_type == 17
                    && buff.visible
                    && buff.stats.is_empty()
                    && buff.expire_time >= 15_000
        )));
    }

    let third = session("third");
    let late_join = zone.handle(ZoneCommand::Join(join("third", 103, "Late", 332, 271)));
    assert!(has_packet(&late_join, &third, |packet| matches!(
        packet,
        ServerPacket::ObjectPlayer { info }
            if info.object_id == 101 && info.buffs.contains(&17)
    )));
    assert!(has_packet(&late_join, &third, |packet| matches!(
        packet,
        ServerPacket::AddBuff { buff }
            if buff.object_id == 101 && buff.buff_type == 17
    )));
}

#[test]
fn zone_native_player_cripple_shot_consumes_poison_buff_and_spreads_green_poison() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::AddBuff {
            buff: ClientBuff {
                buff_type: 17,
                visible: true,
                object_id: 1001,
                expire_time: 20_000,
                infinite: false,
                paused: false,
                stats: Vec::new(),
                values: Vec::new(),
            },
        }],
        now_ms: 0,
    });
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, 334, 270),
        now_ms: 0,
    });
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9101, 335, 270),
        now_ms: 0,
    });
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9102, 338, 270),
        now_ms: 0,
    });

    let launch = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::CrippleShot,
        direction: MirDirection::Right,
        target: Point { x: 334, y: 270 },
        cast: true,
        level: 2,
        damage: 5,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });

    for target in [&first, &second] {
        assert!(has_packet(&launch, target, |packet| matches!(
            packet,
            ServerPacket::RemoveBuff { object_id, buff_type }
                if *object_id == 101 && *buff_type == 17
        )));
        assert!(has_packet(&launch, target, |packet| matches!(
            packet,
            ServerPacket::ObjectPoisoned { object_id, poison }
                if *object_id == 9100 && *poison == 1
        )));
        assert!(has_packet(&launch, target, |packet| matches!(
            packet,
            ServerPacket::ObjectPoisoned { object_id, poison }
                if *object_id == 9101 && *poison == 1
        )));
        assert!(!has_packet(&launch, target, |packet| matches!(
            packet,
            ServerPacket::ObjectPoisoned { object_id, .. } if *object_id == 9102
        )));
    }

    let poison_tick = zone.tick(2_020);
    for target in [&first, &second] {
        assert!(has_packet(&poison_tick, target, |packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info } if info.object_id == 9100
        )));
        assert!(has_packet(&poison_tick, target, |packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info } if info.object_id == 9101
        )));
    }
}

#[test]
fn zone_native_player_vampire_shot_heals_owner_through_zone_authority() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    let mut first_join = join("first", 101, "Scout", 330, 270);
    first_join.hp = 20;
    first_join.max_hp = 60;

    zone.handle(ZoneCommand::Join(first_join));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, 334, 270),
        now_ms: 0,
    });

    let launch = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::VampireShot,
        direction: MirDirection::Right,
        target: Point { x: 334, y: 270 },
        cast: true,
        level: 3,
        damage: 8,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });

    for target in [&first, &second] {
        assert!(has_packet(&launch, target, |packet| matches!(
            packet,
            ServerPacket::AddBuff { buff }
                if buff.object_id == 101 && buff.buff_type == 16 && buff.visible
        )));
    }

    let resolved = zone.tick(20);
    for target in [&first, &second] {
        assert!(has_packet(&resolved, target, |packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info }
                if info.object_id == 101 && info.percent == 46
        )));
    }
    assert!(resolved.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::PlayerHealed { session_id, amount }
            if session_id == &first && *amount == 8
    )));
}

#[test]
fn zone_native_player_cripple_shot_consumes_vampire_buff_and_heals_owner() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    let mut first_join = join("first", 101, "Scout", 330, 270);
    first_join.hp = 20;
    first_join.max_hp = 60;

    zone.handle(ZoneCommand::Join(first_join));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::AddBuff {
            buff: ClientBuff {
                buff_type: 16,
                visible: true,
                object_id: 1001,
                expire_time: 20_000,
                infinite: false,
                paused: false,
                stats: Vec::new(),
                values: Vec::new(),
            },
        }],
        now_ms: 0,
    });
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, 334, 270),
        now_ms: 0,
    });

    let launch = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::CrippleShot,
        direction: MirDirection::Right,
        target: Point { x: 334, y: 270 },
        cast: true,
        level: 3,
        damage: 8,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });

    for target in [&first, &second] {
        assert!(has_packet(&launch, target, |packet| matches!(
            packet,
            ServerPacket::RemoveBuff { object_id, buff_type }
                if *object_id == 101 && *buff_type == 16
        )));
    }

    let resolved = zone.tick(20);
    for target in [&first, &second] {
        assert!(has_packet(&resolved, target, |packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info }
                if info.object_id == 101 && info.percent == 46
        )));
    }
    assert!(resolved.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::PlayerHealed { session_id, amount }
            if session_id == &first && *amount == 8
    )));
}

#[test]
fn zone_native_player_magic_spends_mana_and_enforces_cooldown() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    let mut first_join = join("first", 101, "Scout", 330, 270);
    first_join.mp = 10;

    zone.handle(ZoneCommand::Join(first_join));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_neutral_monster_spawn(9100, "Royal_Guard", 1, 334, 270),
        now_ms: 0,
    });

    let launch = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::FireBall,
        direction: MirDirection::Right,
        target: Point { x: 334, y: 270 },
        cast: true,
        level: 1,
        damage: 1,
        mp_cost: 4,
        cooldown_ms: 500,
        now_ms: 20,
    });
    for target in [&first, &second] {
        assert!(has_packet(&launch, target, |packet| matches!(
            packet,
            ServerPacket::ObjectMana { info }
                if info.object_id == 101 && info.percent == 6
        )));
    }
    let _ = zone.tick(20);

    let cooldown_reject = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::FireBall,
        direction: MirDirection::Right,
        target: Point { x: 334, y: 270 },
        cast: true,
        level: 1,
        damage: 1,
        mp_cost: 4,
        cooldown_ms: 500,
        now_ms: 100,
    });
    assert!(has_packet(&cooldown_reject, &first, |packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == (Point { x: 330, y: 270 })
    )));
    for target in [&first, &second] {
        assert!(!has_packet(&cooldown_reject, target, |packet| matches!(
            packet,
            ServerPacket::Magic { .. }
                | ServerPacket::ObjectMagic { .. }
                | ServerPacket::ObjectMana { .. }
        )));
    }
    let cooldown_tick = zone.tick(100);
    for target in [&first, &second] {
        assert!(!has_packet(&cooldown_tick, target, |packet| matches!(
            packet,
            ServerPacket::ObjectStruck { .. }
        )));
    }

    let second_launch = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::FireBall,
        direction: MirDirection::Right,
        target: Point { x: 334, y: 270 },
        cast: true,
        level: 1,
        damage: 1,
        mp_cost: 4,
        cooldown_ms: 500,
        now_ms: 600,
    });
    assert!(has_packet(&second_launch, &first, |packet| matches!(
        packet,
        ServerPacket::Magic { spell, target_id, .. }
            if *spell == Spell::FireBall && *target_id == 9100
    )));
    for target in [&first, &second] {
        assert!(has_packet(&second_launch, target, |packet| matches!(
            packet,
            ServerPacket::ObjectMana { info }
                if info.object_id == 101 && info.percent == 2
        )));
    }
    let _ = zone.tick(600);

    let mp_reject = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::FireBall,
        direction: MirDirection::Right,
        target: Point { x: 334, y: 270 },
        cast: true,
        level: 1,
        damage: 1,
        mp_cost: 3,
        cooldown_ms: 500,
        now_ms: 1200,
    });
    assert!(has_packet(&mp_reject, &first, |packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == (Point { x: 330, y: 270 })
    )));
    for target in [&first, &second] {
        assert!(!has_packet(&mp_reject, target, |packet| matches!(
            packet,
            ServerPacket::Magic { .. }
                | ServerPacket::ObjectMagic { .. }
                | ServerPacket::ObjectMana { .. }
        )));
    }
}

#[test]
fn zone_native_player_magic_respects_spell_action_window_across_spells() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_neutral_monster_spawn(9100, "Royal_Guard", 1, 334, 270),
        now_ms: 0,
    });

    let first_launch = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::FireBall,
        direction: MirDirection::Right,
        target: Point { x: 334, y: 270 },
        cast: true,
        level: 1,
        damage: 1,
        mp_cost: 1,
        cooldown_ms: 1,
        now_ms: 20,
    });
    assert!(has_packet(&first_launch, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectMagic {
            object_id,
            spell,
            target_id,
            ..
        } if *object_id == 101 && *spell == Spell::FireBall && *target_id == 9100
    )));
    let _ = zone.tick(20);

    let early = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::ThunderBolt,
        direction: MirDirection::Right,
        target: Point { x: 334, y: 270 },
        cast: true,
        level: 1,
        damage: 1,
        mp_cost: 1,
        cooldown_ms: 1,
        now_ms: 100,
    });
    assert!(has_packet(&early, &first, |packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == (Point { x: 330, y: 270 })
    )));
    for target in [&first, &second] {
        assert!(!has_packet(&early, target, |packet| matches!(
            packet,
            ServerPacket::Magic { .. }
                | ServerPacket::ObjectMagic { .. }
                | ServerPacket::ObjectMana { .. }
        )));
    }
    let early_tick = zone.tick(100);
    for target in [&first, &second] {
        assert!(!has_packet(&early_tick, target, |packet| matches!(
            packet,
            ServerPacket::ObjectStruck { .. }
        )));
    }

    let ready = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::ThunderBolt,
        direction: MirDirection::Right,
        target: Point { x: 334, y: 270 },
        cast: true,
        level: 1,
        damage: 1,
        mp_cost: 1,
        cooldown_ms: 1,
        now_ms: 320,
    });
    assert!(has_packet(&ready, &first, |packet| matches!(
        packet,
        ServerPacket::Magic { spell, target_id, .. }
            if *spell == Spell::ThunderBolt && *target_id == 9100
    )));
    assert!(has_packet(&ready, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectMagic {
            object_id,
            spell,
            target_id,
            ..
        } if *object_id == 101 && *spell == Spell::ThunderBolt && *target_id == 9100
    )));
}

#[test]
fn zone_native_player_magic_control_stops_monster_ai_until_expiry() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, 334, 270),
        now_ms: 0,
    });

    let launch = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::Entrapment,
        direction: MirDirection::Right,
        target: Point { x: 334, y: 270 },
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });

    assert!(has_packet(&launch, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectMagic {
            object_id,
            spell,
            target_id,
            ..
        } if *object_id == 101 && *spell == Spell::Entrapment && *target_id == 9100
    )));
    assert!(has_packet(&launch, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectEffect { info }
            if info.object_id == 9100 && info.effect == 9
    )));
    assert!(has_packet(&launch, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectPoisoned { object_id, poison }
            if *object_id == 9100 && *poison == 256
    )));

    let impact_tick = zone.tick(20);
    assert!(!has_packet(&impact_tick, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectStruck { .. }
            | ServerPacket::DamageIndicator { .. }
            | ServerPacket::ObjectWalk { .. }
            | ServerPacket::ObjectAttack { .. }
    )));

    let controlled_tick = zone.tick(600);
    assert!(!has_packet(&controlled_tick, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectWalk { .. } | ServerPacket::ObjectAttack { .. }
    )));

    let released_tick = zone.tick(3_020);
    assert!(has_packet(&released_tick, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectPoisoned { object_id, poison }
            if *object_id == 9100 && *poison == 0
    )));
    assert!(has_packet(&released_tick, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectWalk { movement }
            if movement.object_id == 9100
                && movement.position == (Point { x: 333, y: 270 })
                && movement.direction == MirDirection::Left
    )));
}

#[test]
fn zone_native_player_buff_stats_authoritatively_modify_damage_until_expiry() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::AddBuff {
            buff: ClientBuff {
                buff_type: 5,
                visible: true,
                object_id: 1001,
                expire_time: 200,
                infinite: false,
                paused: false,
                stats: vec![UserItemStat { stat: 5, value: 4 }],
                values: Vec::new(),
            },
        }],
        now_ms: 0,
    });
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, 331, 270),
        now_ms: 0,
    });

    zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: first.clone(),
        object_id: 9100,
        direction: MirDirection::Right,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 10,
        now_ms: 10,
    });
    let buffed_hit = zone.tick(10);
    assert!(has_packet(&buffed_hit, &first, |packet| matches!(
        packet,
        ServerPacket::DamageIndicator { object_id, damage, .. }
            if *object_id == 9100 && *damage == 14
    )));

    let expired = zone.tick(200);
    assert!(!has_packet(&expired, &first, |packet| matches!(
        packet,
        ServerPacket::RemoveBuff { object_id, buff_type }
            if *object_id == 101 && *buff_type == 5
    )));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9101, 332, 270),
        now_ms: 610,
    });
    zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: first.clone(),
        object_id: 9101,
        direction: MirDirection::Right,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 10,
        now_ms: 620,
    });
    let unbuffed_hit = zone.tick(620);
    assert!(has_packet(&unbuffed_hit, &first, |packet| matches!(
        packet,
        ServerPacket::DamageIndicator { object_id, damage, .. }
            if *object_id == 9101 && *damage == 10
    )));
}

#[test]
fn zone_native_player_magic_shield_adds_zone_buff_and_mitigates_hits() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));

    let cast = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 0,
        spell: Spell::MagicShield,
        direction: MirDirection::Right,
        target: Point { x: 330, y: 270 },
        cast: true,
        level: 2,
        damage: 25,
        mp_cost: 10,
        cooldown_ms: 5_000,
        now_ms: 10,
    });

    assert!(has_packet(&cast, &first, |packet| matches!(
        packet,
        ServerPacket::Magic {
            spell,
            target_id,
            ..
        } if *spell == Spell::MagicShield && *target_id == 0
    )));
    assert!(has_packet(&cast, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectMagic {
            spell,
            target_id,
            object_id,
            ..
        } if *spell == Spell::MagicShield && *target_id == 0 && *object_id == 101
    )));
    assert!(has_packet(&cast, &first, |packet| matches!(
        packet,
        ServerPacket::AddBuff { buff }
            if buff.object_id == 101
                && buff.buff_type == 24
                && buff.stats.iter().any(|stat| stat.stat == 124 && stat.value == 40)
    )));
    assert!(has_packet(&cast, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectEffect { info } if info.object_id == 101 && info.effect == 6
    )));

    let third = session("third");
    let join_packets = zone.handle(ZoneCommand::Join(join("third", 103, "Late", 334, 270)));
    assert!(has_packet(&join_packets, &third, |packet| matches!(
        packet,
        ServerPacket::ObjectPlayer { info }
            if info.object_id == 101 && info.buffs == vec![24]
    )));
    assert!(has_packet(&join_packets, &third, |packet| matches!(
        packet,
        ServerPacket::AddBuff { buff }
            if buff.object_id == 101
                && buff.buff_type == 24
                && buff.stats.iter().any(|stat| stat.stat == 124 && stat.value == 40)
    )));

    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, 331, 270),
        now_ms: 20,
    });
    assert!(has_packet(&zone.tick(20), &first, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info } if info.object_id == 9100
    )));
    // The magic shield applies a 40% damage-reduction buff (stat 124). Against
    // the monster's authoritative melee damage of 7 it does not fully block but
    // mitigates the hit to 7 * 60% = 4 (previously the fixed-1 placeholder
    // rounded to 0, which read as a full block).
    let mitigated_hit = zone.tick(620);
    assert!(has_packet(&mitigated_hit, &first, |packet| matches!(
        packet,
        ServerPacket::DamageIndicator { object_id, damage, .. }
            if *object_id == 101 && *damage == 4
    )));
    assert!(mitigated_hit.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::PlayerDamaged { session_id, damage }
            if session_id == &first && *damage == 4
    )));
}

#[test]
fn zone_native_player_healing_self_schedules_zone_heal() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    let mut healer = join("first", 101, "Sage", 330, 270);
    healer.class = MirClass::Taoist;
    healer.hp = 20;
    healer.max_hp = 60;
    zone.handle(ZoneCommand::Join(healer));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));

    let cast = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 101,
        spell: Spell::Healing,
        direction: MirDirection::Right,
        target: Point { x: 330, y: 270 },
        cast: true,
        level: 2,
        damage: 12,
        mp_cost: 5,
        cooldown_ms: 1_000,
        now_ms: 10,
    });

    assert!(has_packet(&cast, &first, |packet| matches!(
        packet,
        ServerPacket::Magic {
            spell,
            target_id,
            ..
        } if *spell == Spell::Healing && *target_id == 101
    )));
    assert!(has_packet(&cast, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectMagic {
            spell,
            target_id,
            object_id,
            ..
        } if *spell == Spell::Healing && *target_id == 101 && *object_id == 101
    )));
    assert!(has_packet(&cast, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectEffect { info } if info.object_id == 101 && info.effect == 3
    )));

    let healed = zone.tick(510);
    assert!(has_packet(&healed, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info } if info.object_id == 101 && info.percent > 33
    )));
    assert!(healed.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::PlayerHealed { session_id, amount } if session_id == &first && *amount > 0
    )));
}

#[test]
fn zone_native_player_mass_healing_schedules_area_zone_heal() {
    let mut zone = zone();
    let first = session("first");
    let mut healer = join("first", 101, "Sage", 330, 270);
    healer.class = MirClass::Taoist;
    healer.hp = 10;
    healer.max_hp = 60;
    zone.handle(ZoneCommand::Join(healer));
    let second = session("second");
    let mut ally = join("second", 102, "Blade", 332, 270);
    ally.hp = 20;
    ally.max_hp = 80;
    zone.handle(ZoneCommand::Join(ally));

    let cast = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 101,
        spell: Spell::MassHealing,
        direction: MirDirection::Down,
        target: Point { x: 331, y: 270 },
        cast: true,
        level: 2,
        damage: 18,
        mp_cost: 8,
        cooldown_ms: 1_000,
        now_ms: 10,
    });

    assert!(has_packet(&cast, &first, |packet| matches!(
        packet,
        ServerPacket::Magic {
            spell,
            target_id,
            level,
            ..
        } if *spell == Spell::MassHealing && *target_id == 101 && *level == 2
    )));
    assert!(!has_packet(&cast, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { .. }
    )));

    let healed = zone.tick(510);
    assert!(has_packet(&healed, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info } if info.object_id == 101 && info.percent > 16
    )));
    assert!(has_packet(&healed, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info } if info.object_id == 102 && info.percent > 25
    )));
    assert!(healed.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::PlayerHealed { session_id, amount } if session_id == &first && *amount >= 18
    )));
    assert!(healed.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::PlayerHealed { session_id, amount } if session_id == &second && *amount >= 18
    )));
}

#[test]
fn zone_native_player_healing_circle_spawns_spell_and_heals_in_zone() {
    let mut zone = zone();
    let first = session("first");
    let mut healer = join("first", 101, "Sage", 330, 270);
    healer.class = MirClass::Taoist;
    healer.hp = 10;
    healer.max_hp = 60;
    zone.handle(ZoneCommand::Join(healer));
    let second = session("second");
    let mut ally = join("second", 102, "Blade", 331, 270);
    ally.hp = 20;
    ally.max_hp = 80;
    zone.handle(ZoneCommand::Join(ally));

    let cast = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 101,
        spell: Spell::HealingCircle,
        direction: MirDirection::Down,
        target: Point { x: 330, y: 270 },
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 12,
        cooldown_ms: 1_000,
        now_ms: 20,
    });

    assert!(has_packet(&cast, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectMagic {
            spell,
            target_id,
            ..
        } if *spell == Spell::HealingCircle && *target_id == 101
    )));
    let delayed = zone.tick(1_720);
    assert!(has_packet(&delayed, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectSpell { info }
            if info.spell == Spell::HealingCircle
                && info.object_id == 70_121
                && info.location == (Point { x: 330, y: 270 })
    )));
    assert!(has_packet(&delayed, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info } if info.object_id == 101 && info.percent > 16
    )));
    assert!(has_packet(&delayed, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info } if info.object_id == 102 && info.percent > 25
    )));
    assert!(delayed.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::PlayerHealed { session_id, amount } if session_id == &first && *amount == 25
    )));
    assert!(delayed.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::PlayerHealed { session_id, amount } if session_id == &second && *amount == 25
    )));
}

#[test]
fn zone_native_player_summon_skeleton_spawns_owned_friendly_summon_after_delay() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    let mut taoist = join("first", 101, "Sage", 330, 270);
    taoist.class = MirClass::Taoist;
    zone.handle(ZoneCommand::Join(taoist));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));

    let cast = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 0,
        spell: Spell::SummonSkeleton,
        direction: MirDirection::Right,
        target: Point { x: 331, y: 270 },
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 7,
        cooldown_ms: 1_000,
        now_ms: 10,
    });

    assert!(has_packet(&cast, &first, |packet| matches!(
        packet,
        ServerPacket::Magic {
            spell,
            target_id,
            target,
            ..
        } if *spell == Spell::SummonSkeleton
            && *target_id == 0
            && *target == (Point { x: 331, y: 270 })
    )));
    assert!(has_packet(&cast, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectMagic {
            object_id,
            spell,
            target_id,
            ..
        } if *object_id == 101 && *spell == Spell::SummonSkeleton && *target_id == 0
    )));
    assert!(!has_packet(&cast, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { .. }
    )));

    let early = zone.tick(509);
    assert!(!has_packet(&early, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { .. }
    )));

    let spawned = zone.tick(510);
    let summon_object_id = packets_for(&spawned, &first)
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectMonster { info }
                if info.name == "BoneFamiliar"
                    && info.master_object_id == 101
                    && info.extra
                    && info.location == (Point { x: 331, y: 270 }) =>
            {
                Some(info.object_id)
            }
            _ => None,
        })
        .expect("delayed Zone summon should spawn an owned BoneFamiliar");
    assert!(has_packet(&spawned, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info }
            if info.object_id == summon_object_id
                && info.name == "BoneFamiliar"
                && info.master_object_id == 101
                && info.extra
    )));

    let third = session("third");
    let late_join = zone.handle(ZoneCommand::Join(join("third", 103, "Watcher", 333, 270)));
    assert!(has_packet(&late_join, &third, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info }
            if info.object_id == summon_object_id
                && info.name == "BoneFamiliar"
                && info.master_object_id == 101
                && info.extra
    )));

    let quiet = zone.tick(1_200);
    assert!(!has_packet(&quiet, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info } if info.object_id == summon_object_id
    )));
    assert!(!has_packet(&quiet, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectWalk { movement } if movement.object_id == summon_object_id
    )));
}

#[test]
fn zone_native_player_summon_skeleton_recalls_existing_owned_summon_without_respawn() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    let mut taoist = join("first", 101, "Sage", 330, 270);
    taoist.class = MirClass::Taoist;
    zone.handle(ZoneCommand::Join(taoist));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 337, 270)));

    zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 0,
        spell: Spell::SummonSkeleton,
        direction: MirDirection::Right,
        target: Point { x: 331, y: 270 },
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 7,
        cooldown_ms: 1_000,
        now_ms: 10,
    });
    let spawned = zone.tick(510);
    let summon_object_id = packets_for(&spawned, &first)
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectMonster { info }
                if info.name == "BoneFamiliar" && info.master_object_id == 101 =>
            {
                Some(info.object_id)
            }
            _ => None,
        })
        .expect("first summon should materialize before recall");

    zone.handle(ZoneCommand::SyncPlayerTransform {
        session_id: first.clone(),
        position: Point { x: 334, y: 270 },
        direction: MirDirection::Right,
    });
    let recall = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 0,
        spell: Spell::SummonSkeleton,
        direction: MirDirection::Right,
        target: Point { x: 335, y: 270 },
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 7,
        cooldown_ms: 1_000,
        now_ms: 1_200,
    });

    assert!(has_packet(&recall, &first, |packet| matches!(
        packet,
        ServerPacket::Magic {
            spell: Spell::SummonSkeleton,
            target_id: 0,
            target,
            ..
        } if *target == (Point { x: 335, y: 270 })
    )));
    assert!(!has_packet(&recall, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { .. }
    )));
    assert!(has_packet(&recall, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectWalk { movement }
            if movement.object_id == summon_object_id
                && movement.position == (Point { x: 334, y: 270 })
    )));
    assert!(has_packet(&recall, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectWalk { movement }
            if movement.object_id == summon_object_id
                && movement.position == (Point { x: 334, y: 270 })
    )));

    let later = zone.tick(2_000);
    assert!(!has_packet(&later, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info } if info.name == "BoneFamiliar"
    )));
}

#[test]
fn zone_native_summon_attacks_hostile_monster_for_owner_without_hitting_players() {
    let mut zone = zone();
    let first = session("first");
    let mut taoist = join("first", 101, "Sage", 330, 270);
    taoist.class = MirClass::Taoist;
    zone.handle(ZoneCommand::Join(taoist));

    zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 0,
        spell: Spell::SummonSkeleton,
        direction: MirDirection::Right,
        target: Point { x: 331, y: 270 },
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 7,
        cooldown_ms: 1_000,
        now_ms: 10,
    });
    let spawned = zone.tick(510);
    let summon_object_id = packets_for(&spawned, &first)
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectMonster { info }
                if info.name == "BoneFamiliar" && info.master_object_id == 101 =>
            {
                Some(info.object_id)
            }
            _ => None,
        })
        .expect("summon should spawn before pet combat");

    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, 332, 270),
        now_ms: 520,
    });

    let attack = zone.tick(1_110);
    assert!(has_packet(&attack, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info }
            if info.object_id == summon_object_id
                && info.location == (Point { x: 331, y: 270 })
                && info.direction == MirDirection::Right
    )));
    assert!(!attack
        .iter()
        .any(|outbound| matches!(outbound, ZoneOutbound::PlayerDamaged { .. })));

    let struck = zone.tick(1_710);
    assert!(has_packet(&struck, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectStruck { info }
            if info.object_id == 9100 && info.attacker_id == summon_object_id
    )));
    assert!(has_packet(&struck, &first, |packet| matches!(
        packet,
        ServerPacket::DamageIndicator {
            damage, object_id, ..
        } if *object_id == 9100 && *damage == 1
    )));
    assert!(has_packet(&struck, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info }
            if info.object_id == 9100 && info.percent == 95
    )));
    assert!(!struck
        .iter()
        .any(|outbound| matches!(outbound, ZoneOutbound::PlayerDamaged { .. })));
    assert!(!has_packet(&struck, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectStruck { info } if info.object_id == 101
    )));
}

#[test]
fn zone_native_holy_deva_uses_ranged_summon_attack_against_hostile_monster() {
    let mut zone = zone();
    let first = session("first");
    let mut taoist = join("first", 101, "Sage", 330, 270);
    taoist.class = MirClass::Taoist;
    zone.handle(ZoneCommand::Join(taoist));

    zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 0,
        spell: Spell::SummonHolyDeva,
        direction: MirDirection::Right,
        target: Point { x: 331, y: 270 },
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 9,
        cooldown_ms: 1_000,
        now_ms: 10,
    });
    let early = zone.tick(1_509);
    assert!(!has_packet(&early, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info } if info.name == "HolyDeva"
    )));

    let spawned = zone.tick(1_510);
    let summon_object_id = packets_for(&spawned, &first)
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectMonster { info }
                if info.name == "HolyDeva"
                    && info.master_object_id == 101
                    && info.extra
                    && info.location == (Point { x: 331, y: 270 }) =>
            {
                Some(info.object_id)
            }
            _ => None,
        })
        .expect("HolyDeva should spawn after its Crystal summon delay");

    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(2_000_000, 337, 270),
        now_ms: 1_520,
    });

    let attack = zone.tick(2_110);
    assert!(has_packet(&attack, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectRangeAttack { info }
            if info.object_id == summon_object_id
                && info.target_id == 2_000_000
                && info.target == (Point { x: 337, y: 270 })
    )));
    assert!(!has_packet(&attack, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectWalk { movement } if movement.object_id == summon_object_id
    )));
    assert!(!attack
        .iter()
        .any(|outbound| matches!(outbound, ZoneOutbound::PlayerDamaged { .. })));

    let struck = zone.tick(2_610);
    assert!(has_packet(&struck, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectStruck { info }
            if info.object_id == 2_000_000 && info.attacker_id == summon_object_id
    )));
    assert!(has_packet(&struck, &first, |packet| matches!(
        packet,
        ServerPacket::DamageIndicator {
            damage, object_id, ..
        } if *object_id == 2_000_000 && *damage > 1
    )));
    assert!(has_packet(&struck, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info }
            if info.object_id == 2_000_000 && info.percent < 100
    )));
    assert!(!struck
        .iter()
        .any(|outbound| matches!(outbound, ZoneOutbound::PlayerDamaged { .. })));
}

#[test]
fn zone_native_summon_shinsu_spawns_owned_pet_and_attacks_hostile_monster() {
    let mut zone = zone();
    let first = session("first");
    let mut taoist = join("first", 101, "Sage", 330, 270);
    taoist.class = MirClass::Taoist;
    zone.handle(ZoneCommand::Join(taoist));

    zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 0,
        spell: Spell::SummonShinsu,
        direction: MirDirection::Right,
        target: Point { x: 331, y: 270 },
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 12,
        cooldown_ms: 1_000,
        now_ms: 10,
    });
    let early = zone.tick(509);
    assert!(!has_packet(&early, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info } if info.name == "Shinsu"
    )));

    let spawned = zone.tick(510);
    let summon_object_id = packets_for(&spawned, &first)
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectMonster { info }
                if info.name == "Shinsu"
                    && info.master_object_id == 101
                    && info.extra
                    && info.location == (Point { x: 331, y: 270 }) =>
            {
                Some(info.object_id)
            }
            _ => None,
        })
        .expect("SummonShinsu should spawn an owned Shinsu after delay");

    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, 332, 270),
        now_ms: 520,
    });

    let attack = zone.tick(1_110);
    assert!(has_packet(&attack, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info }
            if info.object_id == summon_object_id
                && info.location == (Point { x: 331, y: 270 })
                && info.direction == MirDirection::Right
    )));
    let struck = zone.tick(1_710);
    assert!(has_packet(&struck, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectStruck { info }
            if info.object_id == 9100 && info.attacker_id == summon_object_id
    )));
    assert!(!struck
        .iter()
        .any(|outbound| matches!(outbound, ZoneOutbound::PlayerDamaged { .. })));
}

#[test]
fn zone_native_pet_enhancer_buffs_owned_summon_and_increases_damage() {
    let mut zone = zone();
    let first = session("first");
    let mut taoist = join("first", 101, "Sage", 330, 270);
    taoist.class = MirClass::Taoist;
    zone.handle(ZoneCommand::Join(taoist));

    zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 0,
        spell: Spell::SummonShinsu,
        direction: MirDirection::Right,
        target: Point { x: 331, y: 270 },
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1,
        now_ms: 10,
    });
    let spawned = zone.tick(510);
    let summon_object_id = packets_for(&spawned, &first)
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectMonster { info }
                if info.name == "Shinsu"
                    && info.master_object_id == 101
                    && info.extra
                    && info.location == (Point { x: 331, y: 270 }) =>
            {
                Some(info.object_id)
            }
            _ => None,
        })
        .expect("SummonShinsu should spawn before PetEnhancer");

    let enhanced = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: summon_object_id,
        spell: Spell::PetEnhancer,
        direction: MirDirection::Right,
        target: Point { x: 331, y: 270 },
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1,
        now_ms: 600,
    });
    assert!(has_packet(&enhanced, &first, |packet| matches!(
        packet,
        ServerPacket::Magic {
            spell: Spell::PetEnhancer,
            target_id,
            ..
        } if *target_id == summon_object_id
    )));
    assert!(has_packet(&enhanced, &first, |packet| matches!(
        packet,
        ServerPacket::AddBuff { buff }
            if buff.object_id == summon_object_id
                && buff.buff_type == 22
                && buff.visible
                && buff.stats.iter().any(|stat| stat.stat == 4 && stat.value > 0)
                && buff.stats.iter().any(|stat| stat.stat == 1 && stat.value > 0)
    )));

    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, 332, 270),
        now_ms: 620,
    });

    let attack = zone.tick(1_110);
    assert!(has_packet(&attack, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info }
            if info.object_id == summon_object_id && info.direction == MirDirection::Right
    )));
    let struck = zone.tick(1_710);
    assert!(has_packet(&struck, &first, |packet| matches!(
        packet,
        ServerPacket::DamageIndicator {
            damage, object_id, ..
        } if *object_id == 9100 && *damage > 1
    )));
    assert!(!struck
        .iter()
        .any(|outbound| matches!(outbound, ZoneOutbound::PlayerDamaged { .. })));
}

#[test]
fn zone_native_archer_summon_vampire_spawns_at_target_and_recalls_existing_pet() {
    let mut zone = zone();
    let first = session("first");
    let mut archer = join("first", 101, "Robin", 330, 270);
    archer.class = MirClass::Archer;
    zone.handle(ZoneCommand::Join(archer));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, 334, 270),
        now_ms: 0,
    });

    let cast = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::SummonVampire,
        direction: MirDirection::Right,
        target: Point { x: 334, y: 270 },
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1_000,
        now_ms: 10,
    });

    assert!(has_packet(&cast, &first, |packet| matches!(
        packet,
        ServerPacket::Magic {
            spell: Spell::SummonVampire,
            target_id: 9100,
            target,
            ..
        } if *target == (Point { x: 334, y: 270 })
    )));
    assert!(!has_packet(&cast, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info } if info.name == "VampireSpider"
    )));

    let spawned = zone.tick(1_210);
    let summon_object_id = packets_for(&spawned, &first)
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectMonster { info }
                if info.name == "VampireSpider" && info.master_object_id == 101 && info.extra =>
            {
                assert!(
                    (info.location.x - 334)
                        .abs()
                        .max((info.location.y - 270).abs())
                        <= 1,
                    "VampireSpider should materialize beside the target point"
                );
                Some(info.object_id)
            }
            _ => None,
        })
        .expect("SummonVampire should spawn an owned VampireSpider after projectile delay");

    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9101, 337, 270),
        now_ms: 1_220,
    });
    let recall = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9101,
        spell: Spell::SummonVampire,
        direction: MirDirection::Right,
        target: Point { x: 337, y: 270 },
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1_000,
        now_ms: 2_000,
    });

    assert!(!has_packet(&recall, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info } if info.name == "VampireSpider"
    )));
    let recall_position = packets_for(&recall, &first)
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectWalk { movement } if movement.object_id == summon_object_id => {
                Some(movement.position)
            }
            _ => None,
        })
        .expect("recasting SummonVampire should recall the existing pet");
    assert!(
        (recall_position.x - 337)
            .abs()
            .max((recall_position.y - 270).abs())
            <= 1,
        "Archer summon recall should move the retained pet beside the new target"
    );
}

#[test]
fn zone_native_vampire_spider_hit_bleeds_target_and_heals_owner() {
    let mut zone = zone();
    let first = session("first");
    let mut archer = join("first", 101, "Robin", 330, 270);
    archer.class = MirClass::Archer;
    archer.hp = 30;
    zone.handle(ZoneCommand::Join(archer));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, 334, 270),
        now_ms: 0,
    });

    zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::SummonVampire,
        direction: MirDirection::Right,
        target: Point { x: 334, y: 270 },
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1_000,
        now_ms: 10,
    });
    let spawned = zone.tick(1_210);
    let summon_object_id = packets_for(&spawned, &first)
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectMonster { info } if info.name == "VampireSpider" => {
                Some(info.object_id)
            }
            _ => None,
        })
        .expect("SummonVampire should spawn before resolving its melee hit");

    assert!(has_packet(&spawned, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info } if info.object_id == summon_object_id
    )));
    let struck = zone.tick(1_810);
    assert!(has_packet(&struck, &first, |packet| matches!(
        packet,
        ServerPacket::DamageIndicator {
            object_id, damage, ..
        } if *object_id == 9100 && *damage > 0
    )));
    assert!(has_packet(&struck, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectEffect { info }
            if info.object_id == 9100 && info.effect == 18 && info.effect_type == 0
    )));
    assert!(struck.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::PlayerHealed { session_id, amount }
            if session_id == &first && *amount > 0
    )));
    assert!(!struck
        .iter()
        .any(|outbound| matches!(outbound, ZoneOutbound::PlayerDamaged { .. })));
}

#[test]
fn zone_native_vampire_spider_explodes_on_expiry_and_vampires_nearby_target() {
    let mut zone = zone();
    let first = session("first");
    let mut archer = join("first", 101, "Robin", 330, 270);
    archer.class = MirClass::Archer;
    archer.hp = 30;
    zone.handle(ZoneCommand::Join(archer));

    zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 0,
        spell: Spell::SummonVampire,
        direction: MirDirection::Right,
        target: Point { x: 334, y: 270 },
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1_000,
        now_ms: 10,
    });
    let spawned = zone.tick(1_210);
    let (summon_object_id, summon_position) = packets_for(&spawned, &first)
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectMonster { info } if info.name == "VampireSpider" => {
                Some((info.object_id, info.location))
            }
            _ => None,
        })
        .expect("SummonVampire should spawn on the ground target");
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, summon_position.x + 1, summon_position.y),
        now_ms: 19_000,
    });

    let exploded = zone.tick(19_210);
    assert!(has_packet(&exploded, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectDied { info } if info.object_id == summon_object_id
    )));
    assert!(has_packet(&exploded, &first, |packet| matches!(
        packet,
        ServerPacket::DamageIndicator {
            object_id, damage, ..
        } if *object_id == 9100 && *damage > 0
    )));
    assert!(has_packet(&exploded, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectEffect { info }
            if info.object_id == 9100 && info.effect == 18 && info.effect_type == 0
    )));
    assert!(exploded.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::PlayerHealed { session_id, amount }
            if session_id == &first && *amount > 0
    )));
    assert!(!exploded
        .iter()
        .any(|outbound| matches!(outbound, ZoneOutbound::PlayerDamaged { .. })));
}

#[test]
fn zone_native_archer_summon_toad_uses_ranged_pet_attack() {
    let mut zone = zone();
    let first = session("first");
    let mut archer = join("first", 101, "Robin", 330, 270);
    archer.class = MirClass::Archer;
    zone.handle(ZoneCommand::Join(archer));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, 338, 270),
        now_ms: 0,
    });

    zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::SummonToad,
        direction: MirDirection::Right,
        target: Point { x: 338, y: 270 },
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1_000,
        now_ms: 10,
    });

    let spawned_and_attack = zone.tick(1_410);
    let summon_object_id = packets_for(&spawned_and_attack, &first)
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectMonster { info }
                if info.name == "SpittingToad" && info.master_object_id == 101 && info.extra =>
            {
                Some(info.object_id)
            }
            _ => None,
        })
        .expect("SummonToad should spawn an owned SpittingToad after projectile delay");

    assert!(has_packet(&spawned_and_attack, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectRangeAttack { info }
            if info.object_id == summon_object_id && info.target_id == 9100
    )));
    assert!(!has_packet(&spawned_and_attack, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectWalk { movement } if movement.object_id == summon_object_id
    )));
}

#[test]
fn zone_native_archer_summon_snakes_spawns_static_totem_profile() {
    let mut zone = zone();
    let first = session("first");
    let mut archer = join("first", 101, "Robin", 330, 270);
    archer.class = MirClass::Archer;
    zone.handle(ZoneCommand::Join(archer));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, 336, 270),
        now_ms: 0,
    });

    zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::SummonSnakes,
        direction: MirDirection::Right,
        target: Point { x: 336, y: 270 },
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1_000,
        now_ms: 10,
    });

    let spawned = zone.tick(1_310);
    let totem_object_id = packets_for(&spawned, &first)
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectMonster { info }
                if info.name == "SnakeTotem" && info.master_object_id == 101 && info.extra =>
            {
                Some(info.object_id)
            }
            _ => None,
        })
        .expect("SummonSnakes should create an owned SnakeTotem");
    assert!(!has_packet(&spawned, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectWalk { movement } if movement.object_id == totem_object_id
    )));
    assert!(has_packet(&spawned, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info } if info.object_id == totem_object_id
    )));
    let snake_object_id = packets_for(&spawned, &first)
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectMonster { info }
                if info.name == "CharmedSnake"
                    && info.master_object_id == totem_object_id
                    && info.extra =>
            {
                Some(info.object_id)
            }
            _ => None,
        })
        .expect("SnakeTotem should spawn a CharmedSnake minion");

    let attack = zone.tick(1_910);
    assert!(has_packet(&attack, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info } if info.object_id == snake_object_id
    )));
    let struck = zone.tick(2_510);
    assert!(has_packet(&struck, &first, |packet| matches!(
    packet,
    ServerPacket::ObjectStruck { info }
        if info.object_id == 9100 && info.attacker_id == snake_object_id
    )));
}

#[test]
fn zone_native_charmed_snake_hit_applies_paralysis_poison() {
    let mut zone = zone();
    let first = session("first");
    let mut archer = join("first", 101, "Robin", 330, 270);
    archer.class = MirClass::Archer;
    zone.handle(ZoneCommand::Join(archer));
    let mut target = native_monster_spawn(9100, 336, 270);
    target.max_hp = 500;
    target.hp = 500;
    target.drops.clear();
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: target,
        now_ms: 0,
    });

    zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::SummonSnakes,
        direction: MirDirection::Right,
        target: Point { x: 336, y: 270 },
        cast: true,
        level: 9,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1_000,
        now_ms: 10,
    });

    let spawned = zone.tick(1_310);
    let snake_object_id = packets_for(&spawned, &first)
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectMonster { info } if info.name == "CharmedSnake" => {
                Some(info.object_id)
            }
            _ => None,
        })
        .expect("SnakeTotem should spawn a CharmedSnake minion");

    let attack = zone.tick(1_910);
    assert!(has_packet(&attack, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info } if info.object_id == snake_object_id
    )));
    let struck = zone.tick(2_510);
    assert!(has_packet(&struck, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectStruck { info }
            if info.object_id == 9100 && info.attacker_id == snake_object_id
    )));
    assert!(has_packet(&struck, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectPoisoned { object_id, poison }
            if *object_id == 9100 && (*poison & 256) != 0
    )));
}

#[test]
fn zone_native_snake_totem_caps_minions_and_respawns_after_minion_expiry() {
    let mut zone = zone();
    let first = session("first");
    let mut archer = join("first", 101, "Robin", 330, 270);
    archer.class = MirClass::Archer;
    zone.handle(ZoneCommand::Join(archer));
    let mut target = native_monster_spawn(9100, 336, 270);
    target.max_hp = 500;
    target.hp = 500;
    target.drops.clear();
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: target,
        now_ms: 0,
    });

    zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::SummonSnakes,
        direction: MirDirection::Right,
        target: Point { x: 336, y: 270 },
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1_000,
        now_ms: 10,
    });

    let mut snake_ids = BTreeSet::new();
    for now_ms in [1_310, 1_910, 2_510, 3_110] {
        let outbounds = zone.tick(now_ms);
        for packet in packets_for(&outbounds, &first) {
            if let ServerPacket::ObjectMonster { info } = packet {
                if info.name == "CharmedSnake" {
                    snake_ids.insert(info.object_id);
                }
            }
        }
    }
    assert_eq!(
        snake_ids.len(),
        3,
        "level 2 SnakeTotem should cap active CharmedSnake minions at PetLevel + 1"
    );

    let first_snake_id = *snake_ids
        .iter()
        .next()
        .expect("SnakeTotem should have spawned a first minion");
    let expired = zone.tick(15_310);
    assert!(has_packet(&expired, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectDied { info } if info.object_id == first_snake_id
    )));
    let replacement_id = packets_for(&expired, &first)
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectMonster { info }
                if info.name == "CharmedSnake" && !snake_ids.contains(&info.object_id) =>
            {
                Some(info.object_id)
            }
            _ => None,
        })
        .expect("SnakeTotem should refresh the swarm after an expired minion dies");
    assert_ne!(replacement_id, first_snake_id);
}

#[test]
fn zone_native_snake_totem_self_destruct_kills_owned_minions() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    let mut archer = join("first", 101, "Robin", 330, 270);
    archer.class = MirClass::Archer;
    zone.handle(ZoneCommand::Join(archer));
    zone.handle(ZoneCommand::Join(join("second", 102, "Watcher", 350, 270)));
    let mut target = native_monster_spawn(9100, 336, 270);
    target.max_hp = 500;
    target.hp = 500;
    target.drops.clear();
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: target,
        now_ms: 0,
    });

    zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 9100,
        spell: Spell::SummonSnakes,
        direction: MirDirection::Right,
        target: Point { x: 336, y: 270 },
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1_000,
        now_ms: 10,
    });
    let spawned = zone.tick(1_310);
    let totem_object_id = packets_for(&spawned, &first)
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectMonster { info } if info.name == "SnakeTotem" => {
                Some(info.object_id)
            }
            _ => None,
        })
        .expect("SummonSnakes should spawn a SnakeTotem");
    let snake_object_id = packets_for(&spawned, &first)
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectMonster { info } if info.name == "CharmedSnake" => {
                Some(info.object_id)
            }
            _ => None,
        })
        .expect("SnakeTotem should spawn a CharmedSnake minion");

    zone.handle(ZoneCommand::SyncPlayerTransform {
        session_id: first.clone(),
        position: Point { x: 400, y: 270 },
        direction: MirDirection::Right,
    });
    let died = zone.tick(1_910);
    assert!(has_packet(&died, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectDied { info } if info.object_id == totem_object_id
    )));
    assert!(has_packet(&died, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectDied { info } if info.object_id == snake_object_id
    )));
    assert!(!died
        .iter()
        .any(|outbound| matches!(outbound, ZoneOutbound::PlayerDamaged { .. })));
}

#[test]
fn zone_native_archer_stonetrap_spawns_static_trap_and_expires() {
    let mut zone = zone();
    let first = session("first");
    let mut archer = join("first", 101, "Robin", 330, 270);
    archer.class = MirClass::Archer;
    zone.handle(ZoneCommand::Join(archer));

    let cast = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 0,
        spell: Spell::Stonetrap,
        direction: MirDirection::Right,
        target: Point { x: 332, y: 270 },
        cast: true,
        level: 0,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1_000,
        now_ms: 10,
    });
    assert!(has_packet(&cast, &first, |packet| matches!(
        packet,
        ServerPacket::Magic {
            spell: Spell::Stonetrap,
            target_id: 0,
            ..
        }
    )));

    let spawned = zone.tick(1_110);
    let trap_object_id = packets_for(&spawned, &first)
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectMonster { info }
                if info.name == "StoneTrap"
                    && info.master_object_id == 101
                    && info.extra
                    && info.location == (Point { x: 332, y: 270 }) =>
            {
                Some(info.object_id)
            }
            _ => None,
        })
        .expect("Stonetrap should create an owned static trap object");
    assert!(!has_packet(&spawned, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info } if info.object_id == trap_object_id
    )));

    let expired = zone.tick(11_111);
    assert!(has_packet(&expired, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectRemove { object_id } if *object_id == trap_object_id
    )));
}

#[test]
fn zone_native_stonetrap_draws_hostile_monster_aggro_without_player_damage() {
    let mut zone = zone();
    let first = session("first");
    let mut archer = join("first", 101, "Robin", 330, 270);
    archer.class = MirClass::Archer;
    zone.handle(ZoneCommand::Join(archer));

    zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: first.clone(),
        object_id: 0,
        spell: Spell::Stonetrap,
        direction: MirDirection::Right,
        target: Point { x: 332, y: 270 },
        cast: true,
        level: 0,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1_000,
        now_ms: 10,
    });
    let spawned = zone.tick(1_110);
    let trap_object_id = packets_for(&spawned, &first)
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectMonster { info } if info.name == "StoneTrap" => {
                Some(info.object_id)
            }
            _ => None,
        })
        .expect("StoneTrap should exist before aggro test");

    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, 334, 270),
        now_ms: 1_120,
    });

    let walked = zone.tick(1_720);
    assert!(has_packet(&walked, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectWalk { movement }
            if movement.object_id == 9100
                && movement.position == (Point { x: 333, y: 270 })
    )));
    let attacked = zone.tick(2_320);
    assert!(has_packet(&attacked, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info }
            if info.object_id == 9100 && info.direction == MirDirection::Left
    )));
    assert!(!attacked
        .iter()
        .any(|outbound| matches!(outbound, ZoneOutbound::PlayerDamaged { .. })));
    let later = zone.tick(2_920);
    assert!(!has_packet(&later, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectStruck { info } if info.object_id == trap_object_id
    )));
}

#[test]
fn zone_native_player_range_attack_rejects_invalid_target() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, 350, 270),
        now_ms: 0,
    });

    let rejected = zone.handle(ZoneCommand::PlayerRangeAttackObject {
        session_id: first.clone(),
        object_id: 9100,
        direction: MirDirection::Right,
        target: Point { x: 335, y: 270 },
        spell: Spell::Focus,
        level: 3,
        attack_type: 0,
        damage: 8,
        now_ms: 10,
    });

    assert!(has_packet(&rejected, &first, |packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == (Point { x: 330, y: 270 })
    )));
    for target in [&first, &second] {
        assert!(!has_packet(&rejected, target, |packet| matches!(
            packet,
            ServerPacket::ObjectRangeAttack { .. }
                | ServerPacket::ObjectStruck { .. }
                | ServerPacket::ObjectHealth { .. }
        )));
    }

    let later = zone.tick(10);
    for target in [&first, &second] {
        assert!(!has_packet(&later, target, |packet| matches!(
            packet,
            ServerPacket::ObjectStruck { .. } | ServerPacket::ObjectHealth { .. }
        )));
    }
}

#[test]
fn zone_native_monster_tick_walks_toward_visible_player() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, 333, 270),
        now_ms: 0,
    });

    let outbounds = zone.tick(0);

    assert!(has_packet(&outbounds, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectWalk { movement }
            if movement.object_id == 9100
                && movement.position == (Point { x: 332, y: 270 })
                && movement.direction == MirDirection::Left
    )));
}

#[test]
fn zone_native_neutral_guards_do_not_follow_player() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_neutral_monster_spawn(9100, "Royal_Guard", 6, 333, 270),
        now_ms: 0,
    });
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_neutral_monster_spawn(9101, "Royal_Archer", 57, 327, 270),
        now_ms: 0,
    });

    let outbounds = zone.tick(0);

    assert!(!has_packet(&outbounds, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectWalk { movement }
            if movement.object_id == 9100 || movement.object_id == 9101
    )));
    assert!(!has_packet(&outbounds, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info }
            if info.object_id == 9100 || info.object_id == 9101
    )));
}

#[test]
fn zone_native_monster_tick_attacks_adjacent_player_with_delayed_hit() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, 331, 270),
        now_ms: 0,
    });

    let launch = zone.tick(0);

    assert!(has_packet(&launch, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info }
            if info.object_id == 9100
                && info.location == (Point { x: 331, y: 270 })
                && info.direction == MirDirection::Left
    )));
    assert!(!has_packet(&launch, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectStruck { .. } | ServerPacket::DamageIndicator { .. }
    )));

    let hit = zone.tick(600);

    assert!(has_packet(&hit, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectStruck { info }
            if info.object_id == 101 && info.attacker_id == 9100
    )));
    // Field Wasp has no Crystal combat-manifest entry, so the zone rolls the
    // authoritative fallback melee damage of 7 (previously a fixed placeholder
    // of 1). Player 60 -> 53 HP = 88%.
    assert!(has_packet(&hit, &first, |packet| matches!(
        packet,
        ServerPacket::DamageIndicator { object_id, damage, .. }
            if *object_id == 101 && *damage == 7
    )));
    assert!(has_packet(&hit, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info }
            if info.object_id == 101 && info.percent == 88
    )));
    assert!(hit.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::PlayerDamaged { session_id, damage }
            if session_id == &first && *damage == 7
    )));
}

#[test]
fn conquest_archer_guard_ignores_defender_guild_and_attacks_enemy_guild() {
    let mut zone = zone();
    let defender = session("defender");
    let attacker = session("attacker");
    zone.handle(ZoneCommand::Join(join_with_profile(
        "defender",
        101,
        "WolfBlade",
        330,
        270,
        |profile| profile.guild_name = Some("Wolves".to_string()),
    )));
    zone.handle(ZoneCommand::Join(join_with_profile(
        "attacker",
        102,
        "TigerBlade",
        332,
        270,
        |profile| profile.guild_name = Some("Tigers".to_string()),
    )));
    let mut guard = native_monster_spawn(9100, 331, 270);
    guard.name = "ArcherGuard3".to_string();
    guard.ai = 80;
    guard.friendly_guild = Some("Wolves".to_string());
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: defender.clone(),
        monster: guard,
        now_ms: 0,
    });

    let mut outbounds = zone.tick(0);
    for now_ms in [300, 600, 900, 1_200, 1_800, 2_400] {
        outbounds.extend(zone.tick(now_ms));
    }

    assert!(!outbounds.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::PlayerDamaged { session_id, .. } if session_id == &defender
    )));
    assert!(outbounds.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::PlayerDamaged { session_id, damage }
            if session_id == &attacker && *damage > 0
    )));
}

#[test]
fn zone_native_monster_paralysis_blocks_movement_until_status_expires() {
    let mut zone = zone();
    let first = session("first");
    let mut spawn = native_monster_spawn(9004, 331, 270);
    spawn.name = "CaveMaggot".to_string();
    spawn.ai = 7;

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: spawn,
        now_ms: 0,
    });

    assert!(has_packet(&zone.tick(0), &first, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info } if info.object_id == 9004
    )));
    let hit = zone.tick(600);
    assert!(has_packet(&hit, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectPoisoned { object_id, poison }
            if *object_id == 101 && *poison == 256
    )));

    zone.handle(ZoneCommand::Walk {
        session_id: first.clone(),
        direction: MirDirection::Left,
        seq: 1,
        now_ms: 0,
    });
    let blocked = zone.tick(700);
    assert_eq!(zone.player_position(&first), Some(Point { x: 330, y: 270 }));
    assert!(has_packet(&blocked, &first, |packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == (Point { x: 330, y: 270 })
    )));

    zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: first.clone(),
        object_id: 9004,
        direction: MirDirection::Right,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 99,
        now_ms: 800,
    });
    zone.tick(800);

    let expired = zone.tick(5_700);
    assert!(has_packet(&expired, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectPoisoned { object_id, poison }
            if *object_id == 101 && *poison == 0
    )));
    zone.handle(ZoneCommand::Walk {
        session_id: first.clone(),
        direction: MirDirection::Left,
        seq: 2,
        now_ms: 0,
    });
    let moved = zone.tick(5_800);
    assert_eq!(zone.player_position(&first), Some(Point { x: 329, y: 270 }));
    assert!(has_packet(&moved, &first, |packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == (Point { x: 329, y: 270 })
    )));
}

#[test]
fn zone_native_monster_green_poison_does_not_block_movement() {
    let mut zone = zone();
    let first = session("first");
    let mut spawn = native_monster_spawn(9001, 331, 270);
    spawn.name = "ToxicGhoul".to_string();
    spawn.ai = 28;

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: spawn,
        now_ms: 0,
    });

    assert!(has_packet(&zone.tick(0), &first, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info } if info.object_id == 9001
    )));
    let hit = zone.tick(600);
    assert!(has_packet(&hit, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectPoisoned { object_id, poison }
            if *object_id == 101 && *poison == 1
    )));

    zone.handle(ZoneCommand::Walk {
        session_id: first.clone(),
        direction: MirDirection::Left,
        seq: 1,
        now_ms: 0,
    });
    let moved = zone.tick(700);
    assert_eq!(zone.player_position(&first), Some(Point { x: 329, y: 270 }));
    assert!(has_packet(&moved, &first, |packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == (Point { x: 329, y: 270 })
    )));
}

#[test]
fn zone_native_ranged_monster_attacks_without_chasing_when_target_not_adjacent() {
    let mut zone = zone();
    let first = session("first");
    let mut spawn = native_monster_spawn(9100, 333, 270);
    spawn.name = "OmaMage".to_string();
    spawn.ai = 19;

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: spawn,
        now_ms: 0,
    });

    let launch = zone.tick(0);

    assert!(has_packet(&launch, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectRangeAttack { info }
            if info.object_id == 9100
                && info.location == (Point { x: 333, y: 270 })
                && info.target_id == 101
                && info.target == (Point { x: 330, y: 270 })
                && info.direction == MirDirection::Left
    )));
    assert!(!has_packet(&launch, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectWalk { movement } if movement.object_id == 9100
    )));
    assert!(!has_packet(&launch, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectStruck { .. } | ServerPacket::DamageIndicator { .. }
    )));

    let hit = zone.tick(600);

    assert!(has_packet(&hit, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectStruck { info }
            if info.object_id == 101 && info.attacker_id == 9100
    )));
    assert!(has_packet(&hit, &first, |packet| matches!(
        packet,
        ServerPacket::DamageIndicator { object_id, damage, .. }
            if *object_id == 101 && *damage > 0
    )));
    assert!(hit.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::PlayerDamaged { session_id, damage }
            if session_id == &first && *damage > 0
    )));
}

#[test]
fn zone_native_player_defence_buff_mitigates_monster_damage_until_expiry() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 1001,
        packets: vec![ServerPacket::AddBuff {
            buff: ClientBuff {
                buff_type: 7,
                visible: true,
                object_id: 1001,
                expire_time: 600,
                infinite: false,
                paused: false,
                // MaxAC high enough to fully absorb the monster's authoritative
                // melee damage while the buff is active.
                stats: vec![UserItemStat { stat: 1, value: 99 }],
                values: Vec::new(),
            },
        }],
        now_ms: 0,
    });
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, 331, 270),
        now_ms: 0,
    });

    assert!(has_packet(&zone.tick(0), &first, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info } if info.object_id == 9100
    )));
    let mitigated_hit = zone.tick(600);
    assert!(!has_packet(&mitigated_hit, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectStruck { .. }
            | ServerPacket::DamageIndicator { .. }
            | ServerPacket::ObjectHealth { .. }
    )));
    assert!(!mitigated_hit.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::PlayerDamaged { session_id, .. } if session_id == &first
    )));

    assert!(has_packet(&zone.tick(1_200), &first, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info } if info.object_id == 9100
    )));
    // Once the defence buff expires the monster's full authoritative melee
    // damage (Field Wasp fallback = 7) lands.
    let unmitigated_hit = zone.tick(1_800);
    assert!(has_packet(&unmitigated_hit, &first, |packet| matches!(
        packet,
        ServerPacket::DamageIndicator { object_id, damage, .. }
            if *object_id == 101 && *damage == 7
    )));
    assert!(unmitigated_hit.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::PlayerDamaged { session_id, damage }
            if session_id == &first && *damage == 7
    )));
}

#[test]
fn zone_ground_drop_claim_blocks_non_owner_until_owner_window_expires() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 330, 270)));
    zone.handle(ZoneCommand::SyncGroundDrops {
        session_id: first.clone(),
        drops: vec![gold_drop(8001, 330, 270, Some(101), Some(1))],
        now_ms: 0,
    });

    let blocked = zone.handle(ZoneCommand::ClaimGroundDrop {
        session_id: second.clone(),
        object_id: Some(8001),
        target: Point { x: 330, y: 270 },
        group_members: Vec::new(),
        now_ms: 0,
    });
    assert!(has_packet(&blocked, &second, |packet| matches!(
        packet,
        ServerPacket::Chat { message, .. } if message == "server.CannotPickupNotOwner"
    )));

    let allowed = zone.handle(ZoneCommand::ClaimGroundDrop {
        session_id: second.clone(),
        object_id: Some(8001),
        target: Point { x: 330, y: 270 },
        group_members: Vec::new(),
        now_ms: 301,
    });
    assert!(allowed.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::GroundDropClaimed { session_id, drop }
            if session_id == &second && drop.object_id == 8001
    )));
}

#[test]
fn zone_ground_drop_object_id_claim_allows_adjacent_player_tile_only() {
    let mut zone = zone();
    let first = session("first");

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::SyncGroundDrops {
        session_id: first.clone(),
        drops: vec![gold_drop(8001, 331, 270, None, None)],
        now_ms: 0,
    });

    let tile_only = zone.handle(ZoneCommand::ClaimGroundDrop {
        session_id: first.clone(),
        object_id: None,
        target: Point { x: 330, y: 270 },
        group_members: Vec::new(),
        now_ms: 0,
    });
    assert!(!tile_only
        .iter()
        .any(|outbound| matches!(outbound, ZoneOutbound::GroundDropClaimed { .. })));
    assert!(zone.has_ground_drop(8001));

    let object_claim = zone.handle(ZoneCommand::ClaimGroundDrop {
        session_id: first.clone(),
        object_id: Some(8001),
        target: Point { x: 330, y: 270 },
        group_members: Vec::new(),
        now_ms: 1,
    });
    assert!(object_claim.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::GroundDropClaimed { session_id, drop }
            if session_id == &first && drop.object_id == 8001
    )));
    assert!(!zone.has_ground_drop(8001));
}

#[test]
fn zone_ground_drop_claim_commit_removes_for_late_joiners() {
    let mut zone = zone();
    let first = session("first");

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::SyncGroundDrops {
        session_id: first.clone(),
        drops: vec![gold_drop(8001, 330, 270, Some(101), Some(10))],
        now_ms: 0,
    });
    zone.handle(ZoneCommand::ClaimGroundDrop {
        session_id: first.clone(),
        object_id: Some(8001),
        target: Point { x: 330, y: 270 },
        group_members: Vec::new(),
        now_ms: 0,
    });
    zone.handle(ZoneCommand::CommitGroundDropClaim {
        session_id: first,
        object_id: 8001,
    });

    let late = session("late");
    let outbounds = zone.handle(ZoneCommand::Join(join("late", 103, "Late", 330, 270)));
    assert!(!has_packet(&outbounds, &late, |packet| matches!(
        packet,
        ServerPacket::ObjectGold { info } if info.object_id == 8001
    )));
}

#[test]
fn zone_ground_drop_claim_cancel_restores_visibility() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 330, 270)));
    zone.handle(ZoneCommand::SyncGroundDrops {
        session_id: first.clone(),
        drops: vec![gold_drop(8001, 330, 270, Some(101), Some(10))],
        now_ms: 0,
    });
    zone.handle(ZoneCommand::ClaimGroundDrop {
        session_id: first.clone(),
        object_id: Some(8001),
        target: Point { x: 330, y: 270 },
        group_members: Vec::new(),
        now_ms: 0,
    });

    let restored = zone.handle(ZoneCommand::CancelGroundDropClaim {
        session_id: first,
        object_id: 8001,
        now_ms: 10,
    });
    assert!(has_packet(&restored, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectGold { info } if info.object_id == 8001 && info.gold == 25
    )));
}

#[test]
fn zone_ground_drop_nearest_claim_filters_range_and_allowed_ids() {
    let mut zone = zone();
    let first = session("first");

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::SyncGroundDrops {
        session_id: first.clone(),
        drops: vec![
            gold_drop(8001, 333, 270, None, None),
            gold_drop(8002, 331, 270, None, None),
            gold_drop(8003, 336, 270, None, None),
        ],
        now_ms: 0,
    });

    let outbounds = zone.handle(ZoneCommand::ClaimNearestGroundDrop {
        session_id: first.clone(),
        origin: Point { x: 330, y: 270 },
        max_range: 4,
        allowed_object_ids: BTreeSet::from([8001, 8003]),
        group_members: Vec::new(),
        now_ms: 0,
    });

    assert!(outbounds.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::GroundDropClaimed { session_id, drop }
            if session_id == &first && drop.object_id == 8001
    )));
    assert!(!zone.has_ground_drop(8001));
    assert!(zone.has_ground_drop(8002));
    assert!(zone.has_ground_drop(8003));
}

#[test]
fn run_checks_intermediate_tile() {
    let collision = ZoneCollision::unbounded().with_blocked_cells([Point { x: 331, y: 269 }]);
    let mut zone = ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), collision);
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 336, 269)));

    zone.handle(ZoneCommand::Walk {
        session_id: first.clone(),
        direction: MirDirection::Up,
        seq: 1,
        now_ms: 0,
    });
    zone.tick(0);
    zone.handle(ZoneCommand::Run {
        session_id: first.clone(),
        direction: MirDirection::Right,
        seq: 2,
        now_ms: 0,
    });
    let outbounds = zone.tick(600);

    assert_eq!(zone.player_position(&first), Some(Point { x: 330, y: 269 }));
    assert!(has_packet(&outbounds, &first, |packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == (Point { x: 330, y: 269 })
    )));
    assert!(!has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectRun { .. }
    )));
}

#[test]
fn high_frequency_input_keeps_latest_crystal_movement_intent() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));

    zone.handle(ZoneCommand::Walk {
        session_id: first.clone(),
        direction: MirDirection::Up,
        seq: 1,
        now_ms: 0,
    });
    zone.handle(ZoneCommand::Walk {
        session_id: first.clone(),
        direction: MirDirection::Right,
        seq: 2,
        now_ms: 0,
    });
    let outbounds = zone.tick(0);
    let replay = zone.tick(600);

    assert_eq!(zone.player_position(&first), Some(Point { x: 331, y: 270 }));
    assert!(has_packet(&outbounds, &first, |packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == (Point { x: 331, y: 270 })
    )));
    assert!(replay.is_empty());
}

#[test]
fn movement_input_after_ready_acknowledges_without_waiting_for_world_tick() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));

    zone.handle(ZoneCommand::Walk {
        session_id: first.clone(),
        direction: MirDirection::Right,
        seq: 1,
        now_ms: 0,
    });
    let first_tick = zone.tick(0);
    assert!(has_packet(&first_tick, &first, |packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == (Point { x: 331, y: 270 })
    )));

    let late_walk = zone.handle(ZoneCommand::Walk {
        session_id: first.clone(),
        direction: MirDirection::Right,
        seq: 2,
        now_ms: 1_000,
    });

    assert_eq!(zone.player_position(&first), Some(Point { x: 332, y: 270 }));
    assert!(
        has_packet(&late_walk, &first, |packet| matches!(
            packet,
            ServerPacket::UserLocation { location }
                if location.position == (Point { x: 332, y: 270 })
                    && location.direction == MirDirection::Right
        )),
        "movement that arrives after the ready time should ACK in the command response, not wait for a later world tick: {late_walk:?}"
    );

    let replay = zone.tick(1_000);
    assert!(
        !has_packet(&replay, &first, |packet| matches!(
            packet,
            ServerPacket::UserLocation { .. }
        )),
        "late-ready movement should not leave a delayed duplicate ACK: {replay:?}"
    );
}

#[test]
fn stale_movement_intent_is_not_replayed() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));

    zone.handle(ZoneCommand::Walk {
        session_id: first.clone(),
        direction: MirDirection::Right,
        seq: 2,
        now_ms: 0,
    });
    zone.handle(ZoneCommand::Walk {
        session_id: first.clone(),
        direction: MirDirection::Left,
        seq: 1,
        now_ms: 0,
    });
    zone.tick(0);
    let replay = zone.tick(600);

    assert_eq!(zone.player_position(&first), Some(Point { x: 331, y: 270 }));
    assert!(replay.is_empty());
}

#[test]
fn zone_movement_emits_save_transform() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));

    zone.handle(ZoneCommand::Walk {
        session_id: first.clone(),
        direction: MirDirection::Right,
        seq: 1,
        now_ms: 0,
    });
    let outbounds = zone.tick(0);

    assert!(outbounds.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::SaveTransform {
            session_id,
            position,
            direction,
        } if session_id == &first
            && position == &(Point { x: 331, y: 270 })
            && *direction == MirDirection::Right
    )));
}

#[test]
fn zone_whisper_routes_sender_and_target_only() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    let third = session("third");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::Join(join("third", 103, "Jina", 333, 270)));

    let outbounds = zone.handle(ZoneCommand::Chat {
        session_id: first.clone(),
        message: "/Blade secret".to_string(),
        linked_items: Vec::new(),
        linked_user_items: Vec::new(),
        now_ms: 0,
    });

    assert!(has_packet(&outbounds, &first, |packet| matches!(
        packet,
        ServerPacket::Chat {
            chat_type: ChatType::WhisperOut,
            message,
        } if message.contains("secret")
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::Chat {
            chat_type: ChatType::WhisperIn,
            message,
        } if message.contains("Scout=>")
    )));
    assert!(!has_packet(&outbounds, &third, |packet| matches!(
        packet,
        ServerPacket::Chat { .. } | ServerPacket::ObjectChat { .. }
    )));
}

#[test]
fn zone_group_chat_routes_declared_members() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    let third = session("third");
    zone.handle(ZoneCommand::Join(join_with_profile(
        "first",
        101,
        "Scout",
        330,
        270,
        |profile| profile.group_members = vec!["Blade".to_string()],
    )));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    zone.handle(ZoneCommand::Join(join("third", 103, "Jina", 333, 270)));

    let outbounds = zone.handle(ZoneCommand::Chat {
        session_id: first.clone(),
        message: "!!group hello".to_string(),
        linked_items: Vec::new(),
        linked_user_items: Vec::new(),
        now_ms: 0,
    });

    assert!(has_packet(&outbounds, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectChat {
            chat_type: ChatType::Group,
            text,
            ..
        } if text.contains("group hello")
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectChat {
            chat_type: ChatType::Group,
            ..
        }
    )));
    assert!(!has_packet(&outbounds, &third, |packet| matches!(
        packet,
        ServerPacket::ObjectChat {
            chat_type: ChatType::Group,
            ..
        }
    )));
}

#[test]
fn zone_guild_chat_routes_same_guild() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    let third = session("third");
    zone.handle(ZoneCommand::Join(join_with_profile(
        "first",
        101,
        "Scout",
        330,
        270,
        |profile| profile.guild_name = Some("Bichon".to_string()),
    )));
    zone.handle(ZoneCommand::Join(join_with_profile(
        "second",
        102,
        "Blade",
        332,
        270,
        |profile| profile.guild_name = Some("bichon".to_string()),
    )));
    zone.handle(ZoneCommand::Join(join_with_profile(
        "third",
        103,
        "Jina",
        333,
        270,
        |profile| profile.guild_name = Some("Other".to_string()),
    )));

    let outbounds = zone.handle(ZoneCommand::Chat {
        session_id: first.clone(),
        message: "!~guild hello".to_string(),
        linked_items: Vec::new(),
        linked_user_items: Vec::new(),
        now_ms: 0,
    });

    assert!(has_packet(&outbounds, &first, |packet| matches!(
        packet,
        ServerPacket::Chat {
            chat_type: ChatType::Guild,
            message,
        } if message.contains("guild hello")
    )));
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::Chat {
            chat_type: ChatType::Guild,
            ..
        }
    )));
    assert!(!has_packet(&outbounds, &third, |packet| matches!(
        packet,
        ServerPacket::Chat {
            chat_type: ChatType::Guild,
            ..
        }
    )));
}

#[test]
fn zone_shout_level_gate_and_linked_item_text_match_crystal_surface() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));

    let rejected = zone.handle(ZoneCommand::Chat {
        session_id: first.clone(),
        message: "!too soon".to_string(),
        linked_items: Vec::new(),
        linked_user_items: Vec::new(),
        now_ms: 0,
    });
    assert!(has_packet(&rejected, &first, |packet| matches!(
        packet,
        ServerPacket::Chat {
            chat_type: ChatType::System,
            message,
        } if message.contains("level 8")
    )));

    let mut shout_zone =
        ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded());
    shout_zone.handle(ZoneCommand::Join(join_with_profile(
        "first",
        101,
        "Scout",
        330,
        270,
        |profile| profile.free_map_shout = true,
    )));
    let shouted = shout_zone.handle(ZoneCommand::Chat {
        session_id: first.clone(),
        message: "!check <bronze ring>".to_string(),
        linked_items: vec![ChatItem {
            unique_id: 77,
            title: "Bronze Ring".to_string(),
            grid: MirGridType::Inventory,
        }],
        linked_user_items: Vec::new(),
        now_ms: 0,
    });
    assert!(has_packet(&shouted, &first, |packet| matches!(
        packet,
        ServerPacket::Chat {
            chat_type: ChatType::Shout2,
            message,
        } if message.contains("<Bronze Ring/77>")
    )));
    assert!(has_shout_consume(&shouted, &first, true, false));
}

#[test]
fn zone_shout_applies_crystal_cooldown_and_position_macro() {
    let mut zone = zone();
    let first = session("first");
    let mut first_join = join("first", 101, "Scout", 330, 270);
    first_join.level = 8;
    zone.handle(ZoneCommand::Join(first_join));

    let shouted = zone.handle(ZoneCommand::Chat {
        session_id: first.clone(),
        message: "!at $pos".to_string(),
        linked_items: Vec::new(),
        linked_user_items: Vec::new(),
        now_ms: 1_000,
    });
    assert!(has_packet(&shouted, &first, |packet| matches!(
        packet,
        ServerPacket::Chat {
            chat_type: ChatType::Shout,
            message,
        } if message.contains("330, 270")
    )));

    let blocked = zone.handle(ZoneCommand::Chat {
        session_id: first.clone(),
        message: "!again".to_string(),
        linked_items: Vec::new(),
        linked_user_items: Vec::new(),
        now_ms: 2_000,
    });
    assert!(has_packet(&blocked, &first, |packet| matches!(
        packet,
        ServerPacket::Chat {
            chat_type: ChatType::System,
            message,
        } if message.contains("another 9 seconds")
    )));
}

#[test]
fn zone_one_shot_map_shout_is_consumed_before_next_profile_sync() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join_with_profile(
        "first",
        101,
        "Scout",
        330,
        270,
        |profile| profile.free_map_shout = true,
    )));

    let first_shout = zone.handle(ZoneCommand::Chat {
        session_id: first.clone(),
        message: "!scroll shout".to_string(),
        linked_items: Vec::new(),
        linked_user_items: Vec::new(),
        now_ms: 1_000,
    });
    assert!(has_shout_consume(&first_shout, &first, true, false));

    let second_shout = zone.handle(ZoneCommand::Chat {
        session_id: first.clone(),
        message: "!should need level".to_string(),
        linked_items: Vec::new(),
        linked_user_items: Vec::new(),
        now_ms: 11_000,
    });
    assert!(has_packet(&second_shout, &first, |packet| matches!(
        packet,
        ServerPacket::Chat {
            chat_type: ChatType::System,
            message,
        } if message.contains("level 8")
    )));
}

#[test]
fn call_npc_packet_opens_runtime_npc_dialog() {
    let mut session = SimulationSession::new(SimulationConfig::default());
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    session.transfer_map("crystal:0:327:271");

    let packets = session.handle_packet(ClientPacket::CallNpc {
        object_id: 36,
        key: "@Main".to_string(),
    });

    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectChat { object_id: 36, .. })),
        "CallNpc packets: {packets:?}; npcs: {:?}; dialog: {:?}",
        session
            .world_snapshot()
            .entities
            .into_iter()
            .filter(|entity| entity.kind == WorldEntityKind::Npc)
            .map(|entity| (entity.object_id, entity.name, entity.x, entity.y))
            .collect::<Vec<_>>(),
        session.world_snapshot().active_npc_dialog
    );
    assert_eq!(
        session
            .world_snapshot()
            .active_npc_dialog
            .as_ref()
            .map(|dialog| dialog.npc_object_id),
        Some(36)
    );
}

#[test]
fn session_applies_shared_monster_snapshot_to_local_runtime() {
    let mut session = SimulationSession::new(SimulationConfig::default());
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let mut shared_monster = session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.kind == WorldEntityKind::Monster && entity.max_hp.is_some())
        .expect("starter scene should expose a monster");
    shared_monster.hp = Some(1);
    shared_monster.dead = false;

    assert!(session.apply_shared_entity_snapshot(&shared_monster));
    let after_hp = session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.object_id == shared_monster.object_id)
        .and_then(|entity| entity.hp);
    assert_eq!(after_hp, Some(1));

    shared_monster.hp = Some(0);
    shared_monster.dead = true;
    assert!(session.apply_shared_entity_snapshot(&shared_monster));
    let after_dead = session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.object_id == shared_monster.object_id)
        .expect("shared monster should remain visible as dead snapshot");
    assert_eq!(after_dead.hp, Some(0));
    assert!(after_dead.dead);
}

#[test]
fn each_joined_player_has_unique_object_id() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 101, "Blade", 332, 270)));

    assert_ne!(
        zone.player_object_id(&first),
        zone.player_object_id(&second),
        "zone must not expose duplicate player object ids"
    );
}

#[test]
fn zone_movement_writes_back_authoritative_transform_to_session() {
    let mut session_runtime = SimulationSession::new(SimulationConfig::default());
    session_runtime.handle_packet(ClientPacket::Login {
        account_id: "demo".to_string(),
        password: "demo".to_string(),
    });
    session_runtime.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let join = session_runtime
        .active_zone_join_snapshot("first")
        .expect("started session should produce a zone join snapshot");
    let session_id = join.session_id.clone();
    let mut zone = ZoneRuntime::new_with_collision(
        ZoneKey::for_map(join.map_file_name.clone()),
        ZoneCollision::unbounded(),
    );
    zone.handle(ZoneCommand::Join(join));

    zone.handle(ZoneCommand::Walk {
        session_id: session_id.clone(),
        direction: MirDirection::Right,
        seq: 1,
        now_ms: 0,
    });
    let outbounds = zone.tick(0);
    let (position, direction) = outbounds
        .iter()
        .find_map(|outbound| match outbound {
            ZoneOutbound::SaveTransform {
                session_id: saved_session_id,
                position,
                direction,
            } if saved_session_id == &session_id => Some((position.clone(), *direction)),
            _ => None,
        })
        .expect("successful zone movement should emit SaveTransform");

    session_runtime.force_authoritative_player_transform(position.clone(), direction);
    let snapshot = session_runtime.world_snapshot();
    let self_player = snapshot
        .entities
        .iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .expect("session snapshot should still include self player");

    assert_eq!((self_player.x, self_player.y), (position.x, position.y));
    assert_eq!(self_player.direction, direction);
}

#[test]
fn zone_resolves_player_attack_damage_from_authoritative_stats() {
    // The zone must roll damage from the attacker's authoritative stat block and
    // subtract the monster's armour itself — ignoring whatever scalar the gateway
    // pre-rolled in the personal session (here a deliberately absurd 999).
    let mut zone = zone();
    let attacker = session("first");
    zone.handle(ZoneCommand::Join(join_with_combat_stats(
        "first",
        101,
        "Scout",
        330,
        270,
        ZonePlayerCombatStats {
            min_dc: 15,
            max_dc: 15,
            accuracy: 10_000,
            ..Default::default()
        },
    )));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: attacker.clone(),
        monster: native_monster_spawn_with_defense(
            9100,
            331,
            270,
            100,
            ZoneMonsterDefense {
                agility: 0,
                min_ac: 5,
                max_ac: 5,
                ..Default::default()
            },
        ),
        now_ms: 0,
    });

    zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: attacker.clone(),
        object_id: 9100,
        direction: MirDirection::Right,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 999,
        now_ms: 10,
    });
    let struck = zone.tick(10);

    // 15 (MaxDC roll) - 5 (armour) = 10, not the trusted 999 scalar.
    assert_eq!(damage_indicator_for(&struck, 9100), Some(10));
    assert!(has_packet(&struck, &attacker, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info } if info.object_id == 9100 && info.percent == 90
    )));
}

#[test]
fn zone_resolves_player_critical_hit_from_authoritative_stats() {
    // With CriticalRate 100 (weight 5 -> always crits) the zone amplifies the
    // rolled DC by `Floor(damage * (CriticalDamage / CriticalDamageWeight) * 10)`
    // (weight 50, so CriticalDamage 10 == +200%, a tripled blow) before
    // subtracting armour — the Crystal crit (HumanObject.cs:7156-7161,
    // MonsterObject.cs:2594-2599), owned by the zone rather than the session.
    let mut zone = zone();
    let attacker = session("first");
    zone.handle(ZoneCommand::Join(join_with_combat_stats(
        "first",
        101,
        "Scout",
        330,
        270,
        ZonePlayerCombatStats {
            min_dc: 15,
            max_dc: 15,
            accuracy: 10_000,
            critical_rate: 100,
            critical_damage: 10,
            ..Default::default()
        },
    )));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: attacker.clone(),
        monster: native_monster_spawn_with_defense(
            9100,
            331,
            270,
            100,
            ZoneMonsterDefense {
                agility: 0,
                min_ac: 5,
                max_ac: 5,
                ..Default::default()
            },
        ),
        now_ms: 0,
    });

    zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: attacker.clone(),
        object_id: 9100,
        direction: MirDirection::Right,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 999,
        now_ms: 10,
    });
    let struck = zone.tick(10);

    // base 15 -> crit triples to 45 (15 + Floor(15*2.0)), minus 5 armour = 40
    // (vs 10 without crit).
    assert_eq!(damage_indicator_for(&struck, 9100), Some(40));
}

#[test]
fn zone_player_attack_armour_can_fully_block_authoritative_damage() {
    // When armour meets or exceeds the rolled damage the hit lands but deals 0 —
    // a Crystal "block" — and the monster keeps full HP.
    let mut zone = zone();
    let attacker = session("first");
    zone.handle(ZoneCommand::Join(join_with_combat_stats(
        "first",
        101,
        "Scout",
        330,
        270,
        ZonePlayerCombatStats {
            min_dc: 10,
            max_dc: 10,
            accuracy: 10_000,
            ..Default::default()
        },
    )));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: attacker.clone(),
        monster: native_monster_spawn_with_defense(
            9100,
            331,
            270,
            100,
            ZoneMonsterDefense {
                agility: 0,
                min_ac: 50,
                max_ac: 50,
                ..Default::default()
            },
        ),
        now_ms: 0,
    });

    zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: attacker.clone(),
        object_id: 9100,
        direction: MirDirection::Right,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 999,
        now_ms: 10,
    });
    let struck = zone.tick(10);

    // Struck animation lands, damage is fully absorbed, HP unchanged.
    assert!(has_packet(&struck, &attacker, |packet| matches!(
        packet,
        ServerPacket::ObjectStruck { info } if info.object_id == 9100
    )));
    assert_eq!(damage_indicator_for(&struck, 9100), Some(0));
    assert!(has_packet(&struck, &attacker, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info } if info.object_id == 9100 && info.percent == 100
    )));
}

#[test]
fn zone_player_attack_luck_biases_authoritative_damage_toward_max() {
    // Crystal `MapObject.GetAttackPower(MinDC, MaxDC)` is Luck-biased: positive
    // Luck can force the MaxDC end. The zone's authoritative physical attack must
    // honor Luck just like the per-session path, so a lucky attacker lands MaxDC
    // far more often than a Luck-less one across the same swings.
    fn rolled_damages(luck: i32) -> Vec<i32> {
        let mut zone = zone();
        let attacker = session("first");
        zone.handle(ZoneCommand::Join(join_with_combat_stats(
            "first",
            101,
            "Scout",
            330,
            270,
            ZonePlayerCombatStats {
                min_dc: 5,
                max_dc: 25,
                accuracy: 10_000, // never miss, so every swing yields a damage roll
                luck,
                ..Default::default()
            },
        )));
        zone.handle(ZoneCommand::SpawnMonster {
            session_id: attacker.clone(),
            monster: native_monster_spawn_with_defense(
                9100,
                331,
                270,
                1_000_000, // huge HP so it survives every swing
                ZoneMonsterDefense {
                    agility: 0,
                    min_ac: 0,
                    max_ac: 0, // no armour, so the indicator is the raw rolled DC
                    ..Default::default()
                },
            ),
            now_ms: 0,
        });

        // Attacks are gated by a 600ms action window; space them 1s apart so each
        // lands and rolls at a distinct deterministic tick.
        let mut damages = Vec::new();
        for i in 0..32u64 {
            let now_ms = 1_000 + i * 1_000;
            zone.handle(ZoneCommand::PlayerAttackObject {
                session_id: attacker.clone(),
                object_id: 9100,
                direction: MirDirection::Right,
                spell: Spell::None as u8,
                level: 0,
                attack_type: 0,
                damage: 999,
                now_ms,
            });
            let struck = zone.tick(now_ms);
            if let Some(damage) = damage_indicator_for(&struck, 9100) {
                damages.push(damage);
            }
        }
        damages
    }

    let plain = rolled_damages(0);
    let lucky = rolled_damages(9); // MaxLuck is 10, so 9 forces max ~90% of swings.

    assert!(!plain.is_empty() && !lucky.is_empty(), "swings should land");
    for &d in plain.iter().chain(lucky.iter()) {
        assert!((5..=25).contains(&d), "rolled {d} outside [5, 25]");
    }

    let max_rolls = |rolls: &[i32]| rolls.iter().filter(|&&d| d == 25).count();
    assert!(
        max_rolls(&lucky) > max_rolls(&plain),
        "Luck 9 should land MaxDC (25) more often (lucky={}, plain={})",
        max_rolls(&lucky),
        max_rolls(&plain)
    );
    let sum = |rolls: &[i32]| rolls.iter().map(|&d| i64::from(d)).sum::<i64>();
    assert!(
        sum(&lucky) > sum(&plain),
        "Luck should raise average zone melee damage (lucky={}, plain={})",
        sum(&lucky),
        sum(&plain)
    );
}

#[test]
fn zone_player_attack_misses_evasive_monster_authoritatively() {
    // A player with zero accuracy against a hugely evasive monster fails the
    // accuracy-vs-agility check: only the swing animation is broadcast — no
    // struck/health/damage reaction, and the monster keeps full HP.
    let mut zone = zone();
    let attacker = session("first");
    zone.handle(ZoneCommand::Join(join_with_combat_stats(
        "first",
        101,
        "Scout",
        330,
        270,
        ZonePlayerCombatStats {
            min_dc: 30,
            max_dc: 30,
            accuracy: 0,
            ..Default::default()
        },
    )));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: attacker.clone(),
        monster: native_monster_spawn_with_defense(
            9100,
            331,
            270,
            100,
            ZoneMonsterDefense {
                agility: 1_000_000,
                ..Default::default()
            },
        ),
        now_ms: 0,
    });

    let launch = zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: attacker.clone(),
        object_id: 9100,
        direction: MirDirection::Right,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 999,
        now_ms: 10,
    });
    // Swing animation still broadcasts.
    assert!(has_packet(&launch, &attacker, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info } if info.object_id == 101
    )));

    let resolved = zone.tick(10);
    assert!(!has_packet(&resolved, &attacker, |packet| matches!(
        packet,
        ServerPacket::ObjectStruck { .. } | ServerPacket::DamageIndicator { .. }
    )));
    assert_eq!(damage_indicator_for(&resolved, 9100), None);
}

#[test]
fn zone_authoritative_attack_is_deterministic_across_runs() {
    // Two independently constructed zones with identical inputs must resolve the
    // same damage roll — the zone RNG is a pure function of (tick, ids, stats).
    fn run() -> Option<i32> {
        let mut zone = zone();
        let attacker = session("first");
        zone.handle(ZoneCommand::Join(join_with_combat_stats(
            "first",
            101,
            "Scout",
            330,
            270,
            ZonePlayerCombatStats {
                min_dc: 10,
                max_dc: 40,
                accuracy: 10_000,
                ..Default::default()
            },
        )));
        zone.handle(ZoneCommand::SpawnMonster {
            session_id: attacker.clone(),
            monster: native_monster_spawn_with_defense(
                9100,
                331,
                270,
                500,
                ZoneMonsterDefense::default(),
            ),
            now_ms: 0,
        });
        zone.handle(ZoneCommand::PlayerAttackObject {
            session_id: attacker.clone(),
            object_id: 9100,
            direction: MirDirection::Right,
            spell: Spell::None as u8,
            level: 0,
            attack_type: 0,
            damage: 999,
            now_ms: 10,
        });
        damage_indicator_for(&zone.tick(10), 9100)
    }

    let first = run();
    let second = run();
    assert_eq!(first, second);
    let damage = first.expect("attack should land against a non-evasive monster");
    assert!(
        (10..=40).contains(&damage),
        "rolled damage {damage} must be within [MinDC, MaxDC]"
    );
}

#[test]
fn zone_authoritative_damage_shared_hp_consistent_across_attackers() {
    // Two attackers with authoritative stat blocks hit the same shared monster.
    // Both observers must always agree on the single zone-owned HP value.
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    let stats = ZonePlayerCombatStats {
        min_dc: 12,
        max_dc: 12,
        accuracy: 10_000,
        ..Default::default()
    };
    zone.handle(ZoneCommand::Join(join_with_combat_stats(
        "first", 101, "Scout", 330, 270, stats,
    )));
    zone.handle(ZoneCommand::Join(join_with_combat_stats(
        "second", 102, "Blade", 332, 270, stats,
    )));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn_with_defense(
            9100,
            331,
            270,
            100,
            ZoneMonsterDefense {
                min_ac: 2,
                max_ac: 2,
                ..Default::default()
            },
        ),
        now_ms: 0,
    });

    // First attacker: 12 - 2 = 10 damage -> 90 HP.
    zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: first.clone(),
        object_id: 9100,
        direction: MirDirection::Right,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 0,
        now_ms: 10,
    });
    let after_first = zone.tick(10);
    for observer in [&first, &second] {
        assert!(has_packet(&after_first, observer, |packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info } if info.object_id == 9100 && info.percent == 90
        )));
    }

    // Second attacker hits the SAME shared HP -> 80 HP for both observers.
    zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: second.clone(),
        object_id: 9100,
        direction: MirDirection::Left,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 0,
        now_ms: 30,
    });
    let after_second = zone.tick(30);
    for observer in [&first, &second] {
        assert!(has_packet(&after_second, observer, |packet| matches!(
            packet,
            ServerPacket::ObjectHealth { info } if info.object_id == 9100 && info.percent == 80
        )));
    }
}

#[test]
fn zone_update_player_combat_stats_promotes_to_authoritative_damage() {
    // Before a stat block is supplied the zone trusts the gateway scalar (legacy
    // path). After UpdatePlayerCombatStats the zone rolls damage itself.
    let mut zone = zone();
    let attacker = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: attacker.clone(),
        monster: native_monster_spawn_with_defense(
            9100,
            331,
            270,
            500,
            ZoneMonsterDefense::default(),
        ),
        now_ms: 0,
    });

    // Legacy: trusted scalar of 7 applies verbatim.
    zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: attacker.clone(),
        object_id: 9100,
        direction: MirDirection::Right,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 7,
        now_ms: 10,
    });
    assert_eq!(damage_indicator_for(&zone.tick(10), 9100), Some(7));

    // Promote to authoritative stats and confirm the trusted scalar is ignored.
    zone.handle(ZoneCommand::UpdatePlayerCombatStats {
        session_id: attacker.clone(),
        stats: ZonePlayerCombatStats {
            min_dc: 30,
            max_dc: 30,
            accuracy: 10_000,
            ..Default::default()
        },
    });
    zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: attacker.clone(),
        object_id: 9100,
        direction: MirDirection::Right,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 999,
        now_ms: 620,
    });
    assert_eq!(damage_indicator_for(&zone.tick(620), 9100), Some(30));
}

#[test]
fn zone_native_monster_melee_damage_is_data_driven_from_crystal_stats() {
    // A monster present in the Crystal combat manifest deals its authoritative
    // melee damage (CaveMaggot DC = 6..=8), not the old fixed placeholder.
    let mut zone = zone();
    let first = session("first");
    let mut spawn = native_monster_spawn(9300, 331, 270);
    spawn.name = "CaveMaggot".to_string();
    spawn.ai = 7;
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: spawn,
        now_ms: 0,
    });

    // Tick once to launch the adjacent melee, then again past the hit delay.
    assert!(has_packet(&zone.tick(0), &first, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info } if info.object_id == 9300
    )));
    let hit = zone.tick(600);
    assert!(hit.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::PlayerDamaged { session_id, damage }
            if session_id == &first && (6..=8).contains(damage)
    )));
}

#[test]
fn zone_player_base_armour_mitigates_incoming_monster_melee() {
    // A player carrying authoritative AC takes reduced melee damage from a
    // monster: Field Wasp melee 7 minus AC 5 = 2.
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join_with_combat_stats(
        "first",
        101,
        "Scout",
        330,
        270,
        ZonePlayerCombatStats {
            min_ac: 5,
            max_ac: 5,
            ..Default::default()
        },
    )));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, 331, 270),
        now_ms: 0,
    });

    assert!(has_packet(&zone.tick(0), &first, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info } if info.object_id == 9100
    )));
    let hit = zone.tick(600);
    assert!(has_packet(&hit, &first, |packet| matches!(
        packet,
        ServerPacket::DamageIndicator { object_id, damage, .. }
            if *object_id == 101 && *damage == 2
    )));
}

#[test]
fn zone_starter_armour_can_fully_block_scarecrow_damage() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join_with_combat_stats(
        "first",
        101,
        "Scout",
        330,
        270,
        ZonePlayerCombatStats {
            min_ac: 2,
            max_ac: 2,
            ..Default::default()
        },
    )));
    let mut scarecrow = native_monster_spawn(9301, 331, 270);
    scarecrow.name = "Scarecrow".to_string();
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: scarecrow,
        now_ms: 0,
    });

    assert!(has_packet(&zone.tick(0), &first, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info } if info.object_id == 9301
    )));
    let hit = zone.tick(600);
    assert!(!hit.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::PlayerDamaged { session_id, .. } if session_id == &first
    )));
}

#[test]
fn zone_player_physical_armour_does_not_reduce_incoming_magic_damage() {
    // Magic monster damage is mitigated by MAC, not AC. A player with huge AC
    // but zero MAC takes the full magic hit (Field Wasp fallback magic = 7),
    // proving the zone applies the correct armour channel per damage type.
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join_with_combat_stats(
        "first",
        101,
        "Scout",
        330,
        270,
        ZonePlayerCombatStats {
            min_ac: 100,
            max_ac: 100,
            min_mac: 0,
            max_mac: 0,
            ..Default::default()
        },
    )));
    // ai 19 prefers a ranged magic attack from distance 3 (no melee closing).
    let mut caster = native_monster_spawn(9100, 333, 270);
    caster.ai = 19;
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: caster,
        now_ms: 0,
    });

    assert!(has_packet(&zone.tick(0), &first, |packet| matches!(
        packet,
        ServerPacket::ObjectRangeAttack { info } if info.object_id == 9100
    )));
    let hit = zone.tick(600);
    assert!(has_packet(&hit, &first, |packet| matches!(
        packet,
        ServerPacket::DamageIndicator { object_id, damage, .. }
            if *object_id == 101 && *damage == 7
    )));
}

#[test]
fn zone_recomputes_authoritative_magic_damage_ignoring_supplied_scalar() {
    // With an authoritative stat block the zone recomputes the spell's damage
    // from its Crystal formula and ignores the gateway-supplied scalar; without
    // one it trusts the supplied value (legacy).
    fn fireball_damage(stats: ZonePlayerCombatStats) -> i32 {
        let mut zone = zone();
        let first = session("first");
        zone.handle(ZoneCommand::Join(join_with_combat_stats(
            "first", 101, "Scout", 330, 270, stats,
        )));
        zone.handle(ZoneCommand::SpawnMonster {
            session_id: first.clone(),
            monster: native_monster_spawn_with_defense(
                9100,
                334,
                270,
                1000,
                ZoneMonsterDefense::default(),
            ),
            now_ms: 0,
        });
        zone.handle(ZoneCommand::PlayerCastMagic {
            session_id: first.clone(),
            object_id: 9100,
            spell: Spell::FireBall,
            direction: MirDirection::Right,
            target: Point { x: 334, y: 270 },
            cast: true,
            level: 2,
            damage: 1, // deliberately wrong; the authoritative path must ignore it
            mp_cost: 7,
            cooldown_ms: 500,
            now_ms: 20,
        });
        damage_indicator_for(&zone.tick(20), 9100).expect("fireball should strike the monster")
    }

    // Legacy: no stat block -> the supplied scalar of 1 is applied verbatim.
    assert_eq!(fireball_damage(ZonePlayerCombatStats::default()), 1);

    // Authoritative: the zone recomputes FireBall damage from the player's base,
    // far exceeding the bogus supplied 1.
    let recomputed = fireball_damage(ZonePlayerCombatStats {
        min_dc: 50,
        max_dc: 50,
        ..Default::default()
    });
    assert!(
        recomputed > 1,
        "zone must recompute magic damage from stats, got {recomputed}"
    );
}

// --- AOI grid (L1 scaling) regression coverage ---
// These lock the behavior of the spatial-grid visibility path: the grid must
// never drop a visible peer (it returns a superset that the exact visibility
// test filters) and must still emit appear/remove as players cross the AOI
// boundary. Crystal's object data range is symmetric at 16 tiles.

#[test]
fn aoi_grid_players_far_apart_do_not_see_each_other() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");

    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    // 60 tiles away on X - far outside the 16-tile AOI, several grid cells over.
    let outbounds = zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 390, 270)));

    assert!(
        !has_packet(&outbounds, &first, |packet| matches!(
            packet,
            ServerPacket::ObjectPlayer { info } if info.name == "Blade"
        )),
        "a player joining far outside AOI range must not appear"
    );
    assert!(!has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectPlayer { info } if info.name == "Scout"
    )));
}

#[test]
fn aoi_grid_walking_into_range_triggers_appearance() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");

    // Start just outside AOI X-range (17 > 16) so they are mutually invisible,
    // then walk the second player one tile left into range. This crosses toward
    // (and possibly across) a grid-cell boundary, exercising the
    // neighbors-union-visible candidate set in diff_visibility_for.
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    let join_out = zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 347, 270)));
    assert!(
        !has_packet(&join_out, &first, |packet| matches!(
            packet,
            ServerPacket::ObjectPlayer { info } if info.object_id == 102
        )),
        "precondition: peers start out of AOI range"
    );

    let mut now = 0u64;
    let mut saw_appearance = false;
    // Walk left repeatedly; within a few steps 102 enters 101's AOI range and
    // 101 must receive an ObjectPlayer for 102.
    for _ in 0..6 {
        zone.handle(ZoneCommand::Walk {
            session_id: second.clone(),
            direction: MirDirection::Left,
            seq: 1,
            now_ms: now,
        });
        let outbounds = zone.tick(now);
        if has_packet(
            &outbounds,
            &first,
            |packet| matches!(packet, ServerPacket::ObjectPlayer { info } if info.object_id == 102),
        ) {
            saw_appearance = true;
            break;
        }
        now += 700; // > ZONE_WALK_DELAY_MS so each walk commits
    }

    assert!(
        saw_appearance,
        "second player walking into AOI range must appear to the first via the grid path"
    );
}

#[test]
fn aoi_grid_relocates_moving_shared_monster_before_visibility_diff() {
    let mut zone = zone();
    let source = session("source");
    let observer = session("observer");
    zone.handle(ZoneCommand::Join(join("source", 101, "Source", 0, 0)));
    zone.handle(ZoneCommand::Join(join("observer", 102, "Observer", 1, 0)));

    let spawned = zone.handle(ZoneCommand::SyncSharedObjects {
        session_id: source.clone(),
        packets: vec![monster_spawn_packet(9_001, 0, 2, 0)],
        now_ms: 0,
    });
    assert!(has_packet(&spawned, &observer, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info } if info.object_id == 9_001
    )));

    let moved = zone.handle(ZoneCommand::BroadcastSharedObjectPackets {
        session_id: source,
        local_self_object_id: None,
        packets: vec![ServerPacket::ObjectWalk {
            movement: ObjectMovement {
                object_id: 9_001,
                position: Point { x: 100, y: 0 },
                direction: MirDirection::Right,
            },
        }],
        now_ms: 100,
    });
    assert!(has_packet(&moved, &observer, |packet| matches!(
        packet,
        ServerPacket::ObjectRemove { object_id } if *object_id == 9_001
    )));

    let approached = zone.handle(ZoneCommand::SyncPlayerTransform {
        session_id: observer.clone(),
        position: Point { x: 99, y: 0 },
        direction: MirDirection::Right,
    });
    assert!(
        has_packet(&approached, &observer, |packet| matches!(
            packet,
            ServerPacket::ObjectMonster { info }
                if info.object_id == 9_001 && info.location == Point { x: 100, y: 0 }
        )),
        "a moved monster must be discoverable from its new AOI grid cell"
    );
}
