use std::collections::{BTreeMap, BTreeSet};

use mir2_protocol::{
    ChatItem, ChatType, ClientBuff, ClientPacket, MirClass, MirDirection, MirGender, MirGridType,
    MonsterInfo, NpcInfo, ObjectAttackInfo, ObjectDiedInfo, ObjectEffectInfo, ObjectGoldInfo,
    ObjectHealthInfo, ObjectItemInfo, ObjectManaInfo, ObjectMovement, ObjectPlayerInfo,
    ObjectRangeAttackInfo, ObjectRevivedInfo, ObjectSpellInfo, ObjectStruckInfo, Point,
    ServerPacket, Spell, UserItemStat, UserLocation,
};
use mir2_simulation::{
    GroundDropClaimTicket, GroundDropLootSnapshot, GroundDropSnapshot, SessionId, SimulationConfig,
    SimulationSession, WorldEntityDisposition, WorldEntityKind, ZoneCollision, ZoneCommand,
    ZoneJoin, ZoneKey, ZoneMapMetadata, ZoneMonsterDefense, ZoneMonsterSpawn,
    ZoneNpcTeleportConfig, ZoneNpcTeleportDestination, ZoneOutbound, ZonePlayerCombatStats,
    ZoneRuntime,
};

fn session(value: &str) -> SessionId {
    SessionId::new(value)
}

fn login_demo_account(session: &mut SimulationSession) {
    let packets = session.handle_packet(ClientPacket::Login {
        account_id: "demo".to_string(),
        password: "demo".to_string(),
    });
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })),
        "login packets: {packets:?}"
    );
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

fn join_archer(session_id: &str, object_id: u32, name: &str, x: i32, y: i32) -> ZoneJoin {
    let mut value = join(session_id, object_id, name, x, y);
    value.class = MirClass::Archer;
    value
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

fn ground_drop_claim_ticket(
    outbounds: &[ZoneOutbound],
    expected_session: &SessionId,
    object_id: u32,
) -> GroundDropClaimTicket {
    outbounds
        .iter()
        .find_map(|outbound| match outbound {
            ZoneOutbound::GroundDropClaimedWithTicket { session_id, ticket }
                if session_id == expected_session && ticket.object_id == object_id =>
            {
                Some(ticket.clone())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing claim ticket for object {object_id}: {outbounds:?}"))
}

fn sync_combat_admission(
    zone: &mut ZoneRuntime,
    session_id: &SessionId,
    class: MirClass,
    has_class_weapon: bool,
    riding_mount: bool,
    dead: bool,
    attack_blocked: bool,
    fishing: bool,
) {
    sync_combat_admission_with_mount_attack(
        zone,
        session_id,
        class,
        has_class_weapon,
        riding_mount,
        !riding_mount,
        dead,
        attack_blocked,
        fishing,
    );
}

#[allow(clippy::too_many_arguments)]
fn sync_combat_admission_with_mount_attack(
    zone: &mut ZoneRuntime,
    session_id: &SessionId,
    class: MirClass,
    has_class_weapon: bool,
    riding_mount: bool,
    mount_attack_allowed: bool,
    dead: bool,
    attack_blocked: bool,
    fishing: bool,
) {
    zone.handle(ZoneCommand::sync_player_combat_state(
        session_id.clone(),
        class,
        has_class_weapon,
        riding_mount,
        mount_attack_allowed,
        dead,
        attack_blocked,
        fishing,
    ));
}

fn admit_melee(zone: &mut ZoneRuntime, session_id: &SessionId) {
    sync_combat_admission(
        zone,
        session_id,
        MirClass::Warrior,
        false,
        false,
        false,
        false,
        false,
    );
}

fn admit_archer_range(zone: &mut ZoneRuntime, session_id: &SessionId) {
    sync_combat_admission(
        zone,
        session_id,
        MirClass::Archer,
        true,
        false,
        false,
        false,
        false,
    );
}

fn zone_player_transform(
    zone: &ZoneRuntime,
    session_id: &SessionId,
) -> Option<(Point, MirDirection)> {
    Some((
        zone.player_position(session_id)?,
        zone.player_direction(session_id)?,
    ))
}

fn enabled_npc_teleport_config(object_id: u32) -> ZoneNpcTeleportConfig {
    let map = ZoneMapMetadata {
        map_index: 1,
        file_name: "0".to_string(),
        title: "BichonProvince".to_string(),
        mini_map: 1,
        big_map: 101,
        lights: 2,
        map_dark_light: 0,
        music: 0,
        weather: 0,
    };
    ZoneNpcTeleportConfig {
        enabled: true,
        cost: 3_000,
        maps: BTreeMap::from([("0".to_string(), map)]),
        destinations: vec![ZoneNpcTeleportDestination {
            map_file_name: "0".to_string(),
            object_id,
        }],
    }
}

fn enabled_npc_teleport_zone(object_id: u32) -> ZoneRuntime {
    ZoneRuntime::new_with_collision_and_npc_teleport_config(
        ZoneKey::for_map("0"),
        ZoneCollision::unbounded(),
        enabled_npc_teleport_config(object_id),
    )
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

#[test]
fn npc_teleport_real_disabled_policy_is_silent_and_mutates_nothing() {
    let mut zone = zone();
    let owner = session("owner");
    zone.handle(ZoneCommand::Join(join("owner", 100, "Owner", 10, 10)));
    zone.handle(ZoneCommand::SyncSharedObjects {
        session_id: owner.clone(),
        packets: vec![npc_spawn_packet(900, 40, 40)],
        include_owner: false,
        now_ms: 1,
    });
    let before = zone_player_transform(&zone, &owner);

    let outbounds = zone.handle(ZoneCommand::TeleportToNpc {
        session_id: owner.clone(),
        object_id: 900,
        available_gold: 10_000,
    });

    assert!(outbounds.is_empty());
    assert_eq!(zone_player_transform(&zone, &owner), before);
}

#[test]
fn npc_teleport_enabled_fixture_commits_exact_front_and_refreshes_observers() {
    let mut zone = enabled_npc_teleport_zone(900);
    let owner = session("owner");
    let old_observer = session("old-observer");
    let new_observer = session("new-observer");
    zone.handle(ZoneCommand::Join(join("owner", 100, "Owner", 10, 10)));
    zone.handle(ZoneCommand::Join(join(
        "old-observer",
        101,
        "OldObserver",
        11,
        10,
    )));
    zone.handle(ZoneCommand::Join(join(
        "new-observer",
        102,
        "NewObserver",
        43,
        40,
    )));
    zone.handle(ZoneCommand::SyncSharedObjects {
        session_id: owner.clone(),
        packets: vec![npc_spawn_packet(900, 40, 40)],
        include_owner: false,
        now_ms: 1,
    });

    let outbounds = zone.handle(ZoneCommand::TeleportToNpc {
        session_id: owner.clone(),
        object_id: 900,
        available_gold: 3_000,
    });

    assert!(matches!(
        outbounds.first(),
        Some(ZoneOutbound::NpcTeleportCommit {
            session_id,
            gold_cost: 3_000,
            map,
        }) if session_id == &owner && map.map_index == 1
    ));
    assert!(outbounds.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::ToMany { session_ids, packets }
            if session_ids.contains(&old_observer)
                && packets == &vec![ServerPacket::ObjectRemove { object_id: 100 }]
    )));
    assert!(outbounds.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::ToSession { session_id, packets }
            if session_id == &new_observer
                && packets.iter().any(|packet| matches!(
                    packet,
                    ServerPacket::ObjectPlayer { info } if info.object_id == 100
                ))
    )));
    assert!(outbounds.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::SaveTransform { session_id, position, .. }
            if session_id == &owner && position == &Point { x: 40, y: 41 }
    )));
    assert_eq!(
        zone_player_transform(&zone, &owner),
        Some((Point { x: 40, y: 41 }, MirDirection::Down))
    );

    let replay = zone.handle(ZoneCommand::TeleportToNpc {
        session_id: owner.clone(),
        object_id: 900,
        available_gold: 9_000,
    });
    assert!(replay.is_empty(), "an exact replay must not charge twice");
    assert_eq!(
        zone_player_transform(&zone, &owner),
        Some((Point { x: 40, y: 41 }, MirDirection::Down))
    );
}

#[test]
fn npc_teleport_rollback_sync_restores_aoi_and_occupancy() {
    let mut zone = enabled_npc_teleport_zone(900);
    let owner = session("rollback-owner");
    let old_observer = session("rollback-old-observer");
    let new_observer = session("rollback-new-observer");
    zone.handle(ZoneCommand::Join(join(
        "rollback-owner",
        100,
        "Owner",
        10,
        10,
    )));
    zone.handle(ZoneCommand::Join(join(
        "rollback-old-observer",
        101,
        "OldObserver",
        11,
        10,
    )));
    zone.handle(ZoneCommand::Join(join(
        "rollback-new-observer",
        102,
        "NewObserver",
        43,
        40,
    )));
    zone.handle(ZoneCommand::SyncSharedObjects {
        session_id: owner.clone(),
        packets: vec![npc_spawn_packet(900, 40, 40)],
        include_owner: false,
        now_ms: 1,
    });

    let teleported = zone.handle(ZoneCommand::TeleportToNpc {
        session_id: owner.clone(),
        object_id: 900,
        available_gold: 9_000,
    });
    assert!(has_packet(&teleported, &old_observer, |packet| matches!(
        packet,
        ServerPacket::ObjectRemove { object_id: 100 }
    )));
    assert!(has_packet(&teleported, &new_observer, |packet| matches!(
        packet,
        ServerPacket::ObjectPlayer { info } if info.object_id == 100
    )));

    let rolled_back = zone.handle(ZoneCommand::SyncPlayerTransform {
        session_id: owner.clone(),
        position: Point { x: 10, y: 10 },
        direction: MirDirection::Down,
    });
    assert_eq!(
        zone_player_transform(&zone, &owner),
        Some((Point { x: 10, y: 10 }, MirDirection::Down))
    );
    assert!(has_packet(&rolled_back, &new_observer, |packet| matches!(
        packet,
        ServerPacket::ObjectRemove { object_id: 100 }
    )));
    assert!(has_packet(&rolled_back, &old_observer, |packet| matches!(
        packet,
        ServerPacket::ObjectPlayer { info } if info.object_id == 100
    )));

    let destination_probe = session("rollback-destination-probe");
    zone.handle(ZoneCommand::Join(join(
        "rollback-destination-probe",
        103,
        "DestinationProbe",
        40,
        41,
    )));
    assert_eq!(
        zone.player_position(&destination_probe),
        Some(Point { x: 40, y: 41 }),
        "rollback must release the temporary teleport destination"
    );

    let origin_probe = session("rollback-origin-probe");
    zone.handle(ZoneCommand::Join(join(
        "rollback-origin-probe",
        104,
        "OriginProbe",
        10,
        10,
    )));
    assert_ne!(
        zone.player_position(&origin_probe),
        Some(Point { x: 10, y: 10 }),
        "rollback must restore the owner's origin occupancy"
    );
}

#[test]
fn npc_teleport_discards_movement_intent_queued_before_commit() {
    let mut zone = enabled_npc_teleport_zone(900);
    let owner = session("teleport-movement-owner");
    zone.handle(ZoneCommand::Join(join(
        "teleport-movement-owner",
        100,
        "Owner",
        10,
        10,
    )));
    zone.handle(ZoneCommand::SyncSharedObjects {
        session_id: owner.clone(),
        packets: vec![npc_spawn_packet(900, 40, 40)],
        include_owner: false,
        now_ms: 1,
    });

    // The first step starts the movement cooldown; the second intent remains
    // queued and would move the player on a later tick unless teleport clears
    // all pre-commit movement state.
    zone.handle(ZoneCommand::Walk {
        session_id: owner.clone(),
        direction: MirDirection::Right,
        seq: 1,
        now_ms: 10,
    });
    zone.handle(ZoneCommand::Walk {
        session_id: owner.clone(),
        direction: MirDirection::Right,
        seq: 2,
        now_ms: 11,
    });

    let teleported = zone.handle(ZoneCommand::TeleportToNpc {
        session_id: owner.clone(),
        object_id: 900,
        available_gold: 9_000,
    });
    assert!(matches!(
        teleported.first(),
        Some(ZoneOutbound::NpcTeleportCommit { .. })
    ));
    assert_eq!(
        zone_player_transform(&zone, &owner),
        Some((Point { x: 40, y: 41 }, MirDirection::Right))
    );

    let after_cooldown = zone.tick(u64::MAX);
    assert_eq!(
        zone_player_transform(&zone, &owner),
        Some((Point { x: 40, y: 41 }, MirDirection::Right)),
        "a pre-teleport movement intent must never overwrite the committed destination"
    );
    assert!(!after_cooldown.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::ToSession { packets, .. } | ZoneOutbound::ToMany { packets, .. }
            if packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::ObjectWalk { .. } | ServerPacket::ObjectRun { .. }
            ))
    )));
}

#[test]
fn npc_teleport_rejections_preserve_transform_for_missing_ineligible_low_gold_and_occupied_front() {
    for (requested_object_id, available_gold, occupy_front) in [
        (901, 10_000, false),
        (900, 2_999, false),
        (900, 10_000, true),
    ] {
        let mut zone = enabled_npc_teleport_zone(900);
        let owner = session("owner");
        zone.handle(ZoneCommand::Join(join("owner", 100, "Owner", 10, 10)));
        if occupy_front {
            zone.handle(ZoneCommand::Join(join("blocker", 101, "Blocker", 40, 41)));
        }
        zone.handle(ZoneCommand::SyncSharedObjects {
            session_id: owner.clone(),
            packets: vec![npc_spawn_packet(900, 40, 40)],
            include_owner: false,
            now_ms: 1,
        });
        let before = zone_player_transform(&zone, &owner);

        let outbounds = zone.handle(ZoneCommand::TeleportToNpc {
            session_id: owner.clone(),
            object_id: requested_object_id,
            available_gold,
        });

        assert!(outbounds.is_empty());
        assert_eq!(zone_player_transform(&zone, &owner), before);
    }

    // Object 901 is present in the authoritative Zone object set, but only
    // object 900 is configured as teleport-eligible.  A rejection must not
    // emit the fee commit, move either player, release the owner's occupied
    // tile, or disturb the existing owner/observer AOI relationship.
    let mut ineligible = enabled_npc_teleport_zone(900);
    let owner = session("ineligible-owner");
    let observer = session("ineligible-observer");
    ineligible.handle(ZoneCommand::Join(join(
        "ineligible-owner",
        100,
        "Owner",
        10,
        10,
    )));
    ineligible.handle(ZoneCommand::Join(join(
        "ineligible-observer",
        101,
        "Observer",
        9,
        10,
    )));
    ineligible.handle(ZoneCommand::SyncSharedObjects {
        session_id: owner.clone(),
        packets: vec![npc_spawn_packet(900, 40, 40), npc_spawn_packet(901, 42, 40)],
        include_owner: false,
        now_ms: 1,
    });
    let owner_before = zone_player_transform(&ineligible, &owner);
    let observer_before = zone_player_transform(&ineligible, &observer);

    let rejected = ineligible.handle(ZoneCommand::TeleportToNpc {
        session_id: owner.clone(),
        object_id: 901,
        available_gold: 10_000,
    });

    assert!(rejected.is_empty());
    assert_eq!(zone_player_transform(&ineligible, &owner), owner_before);
    assert_eq!(
        zone_player_transform(&ineligible, &observer),
        observer_before
    );

    let mut blocked_by_owner = ineligible.handle(ZoneCommand::Walk {
        session_id: observer.clone(),
        direction: MirDirection::Right,
        seq: 1,
        now_ms: 2,
    });
    blocked_by_owner.extend(ineligible.tick(2));
    assert_eq!(
        zone_player_transform(&ineligible, &observer),
        observer_before
    );
    assert!(has_packet(&blocked_by_owner, &observer, |packet| matches!(
        packet,
        ServerPacket::UserLocation { location }
            if location.position == (Point { x: 9, y: 10 })
    )));
    assert!(!has_packet(&blocked_by_owner, &owner, |packet| matches!(
        packet,
        ServerPacket::ObjectWalk { .. }
    )));

    let mut aoi_probe = ineligible.handle(ZoneCommand::Turn {
        session_id: owner.clone(),
        direction: MirDirection::Left,
        now_ms: 3,
    });
    aoi_probe.extend(ineligible.tick(3));
    assert!(has_packet(&aoi_probe, &observer, |packet| matches!(
        packet,
        ServerPacket::ObjectTurn { movement }
            if movement.object_id == 100 && movement.direction == MirDirection::Left
    )));

    let mut cross_zone = ZoneRuntime::new_with_collision_and_npc_teleport_config(
        ZoneKey::for_map("0"),
        ZoneCollision::unbounded(),
        ZoneNpcTeleportConfig {
            enabled: true,
            cost: 3_000,
            maps: BTreeMap::from([(
                "0".to_string(),
                ZoneMapMetadata {
                    map_index: 1,
                    file_name: "0".to_string(),
                    title: "BichonProvince".to_string(),
                    mini_map: 1,
                    big_map: 101,
                    lights: 2,
                    map_dark_light: 0,
                    music: 0,
                    weather: 0,
                },
            )]),
            destinations: vec![ZoneNpcTeleportDestination {
                map_file_name: "other-map".to_string(),
                object_id: 900,
            }],
        },
    );
    let owner = session("cross-zone-owner");
    cross_zone.handle(ZoneCommand::Join(join(
        "cross-zone-owner",
        100,
        "Owner",
        10,
        10,
    )));
    cross_zone.handle(ZoneCommand::SyncSharedObjects {
        session_id: owner.clone(),
        packets: vec![npc_spawn_packet(900, 40, 40)],
        include_owner: false,
        now_ms: 1,
    });
    let before = zone_player_transform(&cross_zone, &owner);
    assert!(cross_zone
        .handle(ZoneCommand::TeleportToNpc {
            session_id: owner.clone(),
            object_id: 900,
            available_gold: 10_000,
        })
        .is_empty());
    assert_eq!(zone_player_transform(&cross_zone, &owner), before);

    let mut static_collision = ZoneRuntime::new_with_collision_and_npc_teleport_config(
        ZoneKey::for_map("0"),
        ZoneCollision::unbounded().with_blocked_cells([Point { x: 40, y: 41 }]),
        enabled_npc_teleport_config(900),
    );
    let owner = session("static-collision-owner");
    static_collision.handle(ZoneCommand::Join(join(
        "static-collision-owner",
        100,
        "Owner",
        10,
        10,
    )));
    static_collision.handle(ZoneCommand::SyncSharedObjects {
        session_id: owner.clone(),
        packets: vec![npc_spawn_packet(900, 40, 40)],
        include_owner: false,
        now_ms: 1,
    });
    let before = zone_player_transform(&static_collision, &owner);
    assert!(static_collision
        .handle(ZoneCommand::TeleportToNpc {
            session_id: owner.clone(),
            object_id: 900,
            available_gold: 10_000,
        })
        .is_empty());
    assert_eq!(zone_player_transform(&static_collision, &owner), before);
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

#[test]
fn synchronized_native_monsters_never_spawn_with_overlapping_hitboxes() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));

    zone.handle(ZoneCommand::SyncNativeMonsters {
        session_id: first,
        monsters: vec![
            native_monster_spawn(9_100, 336, 270),
            native_monster_spawn(9_101, 336, 270),
        ],
        now_ms: 0,
    });

    let snapshots = zone.native_monster_snapshots();
    let first_position = snapshots
        .iter()
        .find(|monster| monster.object_id == 9_100)
        .expect("first synchronized monster")
        .position
        .clone();
    let second_position = snapshots
        .iter()
        .find(|monster| monster.object_id == 9_101)
        .expect("second synchronized monster")
        .position
        .clone();
    assert_eq!(first_position, Point { x: 336, y: 270 });
    assert_ne!(second_position, first_position);
    assert!(
        (second_position.x - first_position.x)
            .abs()
            .max((second_position.y - first_position.y).abs())
            <= 1,
        "overlapping source spawns should fan out to the nearest free tile"
    );
}

#[test]
fn zone_native_monster_preserves_crystal_move_speed_in_real_milliseconds() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    let mut spawn = native_monster_spawn(9_100, 336, 270);
    spawn.name = "Scarecrow".to_string();
    spawn.ai = 0;
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: spawn,
        now_ms: 0,
    });

    let first_step = zone.tick(0);
    assert!(has_packet(&first_step, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectWalk { movement } if movement.object_id == 9_100
    )));

    for now_ms in [300, 600, 900, 1_200, 1_499] {
        assert!(
            !has_packet(&zone.tick(now_ms), &first, |packet| matches!(
                packet,
                ServerPacket::ObjectWalk { movement } if movement.object_id == 9_100
            )),
            "Scarecrow moved before its Crystal 1500ms cadence at {now_ms}ms"
        );
    }

    assert!(has_packet(&zone.tick(1_500), &first, |packet| matches!(
        packet,
        ServerPacket::ObjectWalk { movement } if movement.object_id == 9_100
    )));
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
    spawn.disposition = Some(WorldEntityDisposition::Neutral);
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

fn assert_owner_only_location_correction(
    outbounds: &[ZoneOutbound],
    owner: &SessionId,
    observer: &SessionId,
) {
    assert_eq!(outbounds.len(), 1);
    assert!(matches!(
        packets_for(outbounds, owner).as_slice(),
        [ServerPacket::UserLocation { .. }]
    ));
    assert!(packets_for(outbounds, observer).is_empty());
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
fn shared_object_update_uses_zone_object_aoi_not_same_map() {
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
        include_owner: false,
        now_ms: 0,
    });

    let outbounds = zone.handle(ZoneCommand::BroadcastSharedObjectPackets {
        session_id: first,
        local_self_object_id: Some(1),
        packets: vec![ServerPacket::ObjectHealth {
            info: ObjectHealthInfo {
                object_id: 9001,
                percent: 75,
                expire: 0,
            },
        }],
        now_ms: 10,
    });

    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info }
            if info.object_id == 9001 && info.percent == 75
    )));
    assert!(!has_packet(&outbounds, &far, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info } if info.object_id == 9001
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
        include_owner: false,
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
        include_owner: false,
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
    admit_melee(&mut zone, &first);
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
    let corpse = zone
        .native_monster_snapshots()
        .into_iter()
        .find(|monster| monster.object_id == 9100)
        .expect("the authoritative corpse must remain in the zone until cleanup");
    assert_eq!(corpse.hp, 0);
    assert!(corpse.dead);

    let mut walked_through_corpse = zone.handle(ZoneCommand::Walk {
        session_id: first.clone(),
        direction: MirDirection::Right,
        seq: 1,
        now_ms: 1_300,
    });
    walked_through_corpse.extend(zone.tick(1_300));
    assert!(
        has_packet(&walked_through_corpse, &first, |packet| matches!(
            packet,
            ServerPacket::UserLocation { location }
                if location.position == (Point { x: 331, y: 270 })
        )),
        "dead native monster must release collision: {walked_through_corpse:?}"
    );

    let late = session("late");
    let late_join = zone.handle(ZoneCommand::Join(join("late", 103, "Late", 332, 271)));
    assert!(has_packet(&late_join, &late, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info }
            if info.object_id == 9100 && info.dead
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
        ZoneOutbound::GroundDropClaimedWithTicket { session_id, ticket }
            if session_id == &first && ticket.object_id == 9200
    )));
}

#[test]
fn zone_native_monster_same_object_id_respawns_as_attackable_new_incarnation() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    admit_melee(&mut zone, &first);

    let mut initial = native_monster_spawn(9_101, 331, 270);
    initial.drops.clear();
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: initial,
        now_ms: 0,
    });
    zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: first.clone(),
        object_id: 9_101,
        direction: MirDirection::Right,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 99,
        now_ms: 10,
    });
    assert!(has_packet(&zone.tick(10), &first, |packet| matches!(
        packet,
        ServerPacket::ObjectDied { info } if info.object_id == 9_101
    )));

    let mut respawn = native_monster_spawn(9_101, 331, 270);
    respawn.drops.clear();
    let respawned = zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: respawn,
        now_ms: 1_000,
    });
    assert!(has_packet(&respawned, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectRevived { info }
            if info.object_id == 9_101
    )));
    assert!(has_packet(&respawned, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info }
            if info.object_id == 9_101 && !info.dead
    )));
    let live = zone
        .native_monster_snapshots()
        .into_iter()
        .find(|monster| monster.object_id == 9_101)
        .expect("respawned monster should remain authoritative in the zone");
    assert_eq!(live.hp, 20);
    assert!(!live.dead);

    let second_launch = zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: first.clone(),
        object_id: 9_101,
        direction: MirDirection::Right,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 99,
        now_ms: 1_000,
    });
    assert!(has_packet(&second_launch, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info } if info.object_id == 101
    )));
    assert!(has_packet(&zone.tick(1_000), &first, |packet| matches!(
        packet,
        ServerPacket::ObjectDied { info } if info.object_id == 9_101
    )));
}

#[test]
fn zone_native_harvestable_monster_cannot_respawn_before_object_harvested() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    admit_melee(&mut zone, &first);

    let initial = native_neutral_monster_spawn(9_102, "Deer", 2, 331, 270);
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: initial.clone(),
        now_ms: 0,
    });
    let launch = zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: first.clone(),
        object_id: 9_102,
        direction: MirDirection::Right,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 99,
        now_ms: 10,
    });
    assert!(has_packet(&launch, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info } if info.object_id == 101
    )));
    assert!(has_packet(&zone.tick(10), &first, |packet| matches!(
        packet,
        ServerPacket::ObjectDied { info } if info.object_id == 9_102
    )));

    let blocked = zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: initial.clone(),
        now_ms: 500,
    });
    assert!(!has_packet(&blocked, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectRevived { info } if info.object_id == 9_102
    )));
    let corpse = zone
        .native_monster_snapshots()
        .into_iter()
        .find(|monster| monster.object_id == 9_102)
        .expect("unharvested Deer corpse should remain authoritative");
    assert!(corpse.dead);
    assert_eq!(corpse.hp, 0);

    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: first.clone(),
        owner_local_object_id: 101,
        packets: vec![ServerPacket::ObjectHarvested {
            movement: ObjectMovement {
                object_id: 9_102,
                position: Point { x: 330, y: 270 },
                direction: MirDirection::Right,
            },
        }],
        now_ms: 600,
    });
    let respawned = zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: initial,
        now_ms: 700,
    });
    assert!(has_packet(&respawned, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectRevived { info } if info.object_id == 9_102
    )));
    let live = zone
        .native_monster_snapshots()
        .into_iter()
        .find(|monster| monster.object_id == 9_102)
        .expect("harvested Deer should respawn as a new incarnation");
    assert!(!live.dead);
    assert!(live.hp > 0);
}

#[test]
fn zone_neutral_harvestable_monster_accepts_only_adjacent_melee() {
    let attacker = session("melee-attacker");
    let mut melee_zone = zone();
    melee_zone.handle(ZoneCommand::Join(join(
        "melee-attacker",
        101,
        "MeleeAttacker",
        330,
        270,
    )));
    admit_melee(&mut melee_zone, &attacker);
    let deer = native_neutral_monster_spawn(9_150, "Deer", 2, 331, 270);
    melee_zone.handle(ZoneCommand::SpawnMonster {
        session_id: attacker.clone(),
        monster: deer.clone(),
        now_ms: 0,
    });

    let direct = melee_zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: attacker.clone(),
        object_id: 9_150,
        direction: MirDirection::Right,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 999,
        now_ms: 10,
    });
    assert!(has_packet(&direct, &attacker, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info } if info.object_id == 101
    )));
    assert!(damage_indicator_for(&melee_zone.tick(10), 9_150).is_some());

    let materialized_attacker = session("materialized-attacker");
    let mut materialized_zone = zone();
    materialized_zone.handle(ZoneCommand::Join(join(
        "materialized-attacker",
        201,
        "MaterializedAttacker",
        330,
        270,
    )));
    admit_melee(&mut materialized_zone, &materialized_attacker);
    let materialized = materialized_zone.handle(ZoneCommand::PlayerAttackMaterializedObject {
        session_id: materialized_attacker.clone(),
        object_id: 9_151,
        monster: Some(ZoneMonsterSpawn {
            object_id: 9_151,
            ..deer
        }),
        direction: MirDirection::Right,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 999,
        now_ms: 10,
    });
    assert!(materialized_zone.has_native_monster(9_151));
    assert!(has_packet(
        &materialized,
        &materialized_attacker,
        |packet| matches!(packet, ServerPacket::ObjectAttack { info } if info.object_id == 201)
    ));
    assert!(damage_indicator_for(&materialized_zone.tick(10), 9_151).is_some());
}

#[test]
fn zone_native_player_range_attack_damages_monster_authoritatively() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");

    zone.handle(ZoneCommand::Join(join_archer(
        "first", 101, "Scout", 330, 270,
    )));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    admit_archer_range(&mut zone, &first);
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, 335, 270),
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

    zone.handle(ZoneCommand::Join(join_archer(
        "first", 101, "Scout", 330, 270,
    )));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    admit_archer_range(&mut zone, &first);
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, 335, 270),
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
    let authoritative_target = zone
        .native_monster_snapshots()
        .into_iter()
        .find(|monster| monster.object_id == 9100)
        .expect("range target remains alive after the first one-damage hit")
        .position;

    let early = zone.handle(ZoneCommand::PlayerRangeAttackObject {
        session_id: first.clone(),
        object_id: 9100,
        direction: MirDirection::Right,
        target: authoritative_target.clone(),
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
        target: authoritative_target,
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
fn zone_range_attack_requires_trusted_admission_and_enforces_nine_tile_boundary() {
    let rejected_cases = [
        (
            "unknown",
            MirClass::Archer,
            true,
            false,
            false,
            false,
            false,
        ),
        (
            "warrior",
            MirClass::Warrior,
            true,
            false,
            false,
            false,
            true,
        ),
        (
            "no weapon",
            MirClass::Archer,
            false,
            false,
            false,
            false,
            true,
        ),
        ("mounted", MirClass::Archer, true, true, false, false, true),
        ("dead", MirClass::Archer, true, false, true, false, true),
        ("blocked", MirClass::Archer, true, false, false, true, true),
        ("fishing", MirClass::Archer, true, false, false, false, true),
    ];

    for (label, class, has_weapon, mounted, dead, blocked, sync_state) in rejected_cases {
        let mut zone = zone();
        let attacker = session("attacker");
        let mut player_join = if class == MirClass::Archer {
            join_archer("attacker", 101, "Attacker", 330, 270)
        } else {
            join("attacker", 101, "Attacker", 330, 270)
        };
        player_join.combat_stats = ZonePlayerCombatStats {
            min_dc: 4,
            max_dc: 4,
            accuracy: 10_000,
            ..Default::default()
        };
        zone.handle(ZoneCommand::Join(player_join));
        if sync_state {
            sync_combat_admission(
                &mut zone,
                &attacker,
                class,
                has_weapon,
                mounted,
                dead,
                blocked,
                label == "fishing",
            );
        }
        zone.handle(ZoneCommand::SpawnMonster {
            session_id: attacker.clone(),
            monster: native_monster_spawn(9_100, 339, 270),
            now_ms: 0,
        });
        let before = zone
            .native_monster_snapshots()
            .into_iter()
            .find(|monster| monster.object_id == 9_100)
            .expect("target")
            .hp;
        let launch = zone.handle(ZoneCommand::PlayerRangeAttackObject {
            session_id: attacker.clone(),
            object_id: 9_100,
            direction: MirDirection::Right,
            target: Point { x: 339, y: 270 },
            spell: Spell::None,
            level: 0,
            attack_type: 0,
            damage: 999,
            now_ms: 10,
        });
        assert!(
            has_packet(&launch, &attacker, |packet| matches!(
                packet,
                ServerPacket::UserLocation { .. }
            )),
            "{label} must receive owner correction"
        );
        assert!(!has_packet(&launch, &attacker, |packet| matches!(
            packet,
            ServerPacket::RangeAttack { .. } | ServerPacket::ObjectRangeAttack { .. }
        )));
        let _ = zone.tick(10);
        assert_eq!(
            zone.native_monster_snapshots()
                .into_iter()
                .find(|monster| monster.object_id == 9_100)
                .expect("target")
                .hp,
            before,
            "{label} must schedule zero damage"
        );
    }

    for (distance, allowed) in [(9, true), (10, false)] {
        let mut zone = zone();
        let attacker = session("attacker");
        let mut player_join = join_archer("attacker", 101, "Attacker", 330, 270);
        player_join.combat_stats = ZonePlayerCombatStats {
            min_dc: 4,
            max_dc: 4,
            accuracy: 10_000,
            ..Default::default()
        };
        zone.handle(ZoneCommand::Join(player_join));
        admit_archer_range(&mut zone, &attacker);
        let target = Point {
            x: 330 + distance,
            y: 270,
        };
        zone.handle(ZoneCommand::SpawnMonster {
            session_id: attacker.clone(),
            monster: native_monster_spawn(9_100, target.x, target.y),
            now_ms: 0,
        });
        let launch = zone.handle(ZoneCommand::PlayerRangeAttackObject {
            session_id: attacker.clone(),
            object_id: 9_100,
            direction: MirDirection::Right,
            target,
            spell: Spell::None,
            level: 0,
            attack_type: 0,
            damage: 999,
            now_ms: 10,
        });
        assert_eq!(
            has_packet(&launch, &attacker, |packet| matches!(
                packet,
                ServerPacket::RangeAttack { .. }
            )),
            allowed,
            "distance {distance} must follow the authoritative 9-tile boundary"
        );
        let tick = zone.tick(10);
        assert_eq!(
            damage_indicator_for(&tick, 9_100),
            allowed.then_some(4),
            "the zone must ignore the supplied damage scalar"
        );
        if !allowed {
            assert!(has_packet(&launch, &attacker, |packet| matches!(
                packet,
                ServerPacket::UserLocation { .. }
            )));
        }
    }
}

#[test]
fn zone_melee_requires_trusted_dead_and_blocked_admission() {
    let rejected_cases = [
        ("unknown", None),
        ("class mismatch", Some((MirClass::Archer, false, false))),
        ("dead", Some((MirClass::Warrior, true, false))),
        ("blocked", Some((MirClass::Warrior, false, true))),
    ];

    for (label, admission) in rejected_cases {
        let mut zone = zone();
        let attacker = session("attacker");
        zone.handle(ZoneCommand::Join(join(
            "attacker", 101, "Attacker", 330, 270,
        )));
        if let Some((class, dead, blocked)) = admission {
            sync_combat_admission(
                &mut zone, &attacker, class, false, false, dead, blocked, false,
            );
        }
        zone.handle(ZoneCommand::SpawnMonster {
            session_id: attacker.clone(),
            monster: native_monster_spawn(9_100, 331, 270),
            now_ms: 0,
        });
        let hp_before = zone
            .native_monster_snapshots()
            .into_iter()
            .find(|monster| monster.object_id == 9_100)
            .expect("target")
            .hp;
        let launch = zone.handle(ZoneCommand::PlayerAttackObject {
            session_id: attacker.clone(),
            object_id: 9_100,
            direction: MirDirection::Right,
            spell: Spell::None as u8,
            level: 0,
            attack_type: 0,
            damage: 999,
            now_ms: 10,
        });
        assert!(
            has_packet(&launch, &attacker, |packet| matches!(
                packet,
                ServerPacket::UserLocation { .. }
            )),
            "{label} must receive owner correction"
        );
        assert!(!has_packet(&launch, &attacker, |packet| matches!(
            packet,
            ServerPacket::ObjectAttack { .. }
        )));
        let _ = zone.tick(10);
        assert_eq!(
            zone.native_monster_snapshots()
                .into_iter()
                .find(|monster| monster.object_id == 9_100)
                .expect("target")
                .hp,
            hp_before,
            "{label} must schedule zero melee damage"
        );
    }

    let mut legal = zone();
    let attacker = session("attacker");
    legal.handle(ZoneCommand::Join(join(
        "attacker", 101, "Attacker", 330, 270,
    )));
    sync_combat_admission(
        &mut legal,
        &attacker,
        MirClass::Warrior,
        false,
        false,
        false,
        false,
        false,
    );
    legal.handle(ZoneCommand::SpawnMonster {
        session_id: attacker.clone(),
        monster: native_monster_spawn(9_100, 331, 270),
        now_ms: 0,
    });
    let launch = legal.handle(ZoneCommand::PlayerAttackObject {
        session_id: attacker.clone(),
        object_id: 9_100,
        direction: MirDirection::Right,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 999,
        now_ms: 10,
    });
    assert!(has_packet(&launch, &attacker, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { .. }
    )));
}

#[test]
fn materialized_melee_rejection_is_atomic_and_allowed_attack_commits() {
    let mut zone = zone();
    let attacker = session("attacker");
    zone.handle(ZoneCommand::Join(join(
        "attacker", 101, "Attacker", 330, 270,
    )));
    let spawn = native_monster_spawn(9_100, 331, 270);
    let before_root = zone.canonical_state_root().expect("state root");

    let rejected = zone.handle(ZoneCommand::PlayerAttackMaterializedObject {
        session_id: attacker.clone(),
        object_id: 9_100,
        monster: Some(spawn.clone()),
        direction: MirDirection::Right,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 999,
        now_ms: 10,
    });
    assert!(has_packet(&rejected, &attacker, |packet| matches!(
        packet,
        ServerPacket::UserLocation { .. }
    )));
    assert_eq!(zone.native_monster_count(), 0);
    assert_eq!(
        zone.canonical_state_root().expect("state root"),
        before_root
    );
    assert!(!rejected.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::ToMany { packets, .. } | ZoneOutbound::ToAll { packets }
            if packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::ObjectMonster { .. } | ServerPacket::ObjectAttack { .. }
            ))
    )));

    admit_melee(&mut zone, &attacker);
    let allowed = zone.handle(ZoneCommand::PlayerAttackMaterializedObject {
        session_id: attacker.clone(),
        object_id: 9_100,
        monster: Some(spawn),
        direction: MirDirection::Right,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 999,
        now_ms: 20,
    });
    assert!(zone.has_native_monster(9_100));
    assert!(has_packet(&allowed, &attacker, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info } if info.object_id == 101
    )));
    assert!(damage_indicator_for(&zone.tick(20), 9_100).is_some());
}

#[test]
fn zone_direct_and_materialized_melee_reject_neutral_monsters_atomically() {
    let mut zone = zone();
    let attacker = session("attacker");
    let observer = session("observer");
    zone.handle(ZoneCommand::Join(join(
        "attacker", 101, "Attacker", 330, 270,
    )));
    zone.handle(ZoneCommand::Join(join(
        "observer", 102, "Observer", 330, 271,
    )));
    admit_melee(&mut zone, &attacker);
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: attacker.clone(),
        monster: native_neutral_monster_spawn(9_100, "Royal_Guard", 6, 331, 270),
        now_ms: 0,
    });
    let direct_hp_before = zone
        .native_monster_snapshots()
        .into_iter()
        .find(|monster| monster.object_id == 9_100)
        .expect("neutral target")
        .hp;
    let direct_root_before = zone.canonical_state_root().expect("state root");

    let direct = zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: attacker.clone(),
        object_id: 9_100,
        direction: MirDirection::Right,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 999,
        now_ms: 10,
    });
    assert_eq!(direct.len(), 1);
    assert!(has_packet(&direct, &attacker, |packet| matches!(
        packet,
        ServerPacket::UserLocation { .. }
    )));
    assert!(!has_packet(&direct, &observer, |_| true));
    assert_eq!(
        zone.canonical_state_root().expect("state root"),
        direct_root_before
    );
    assert_eq!(
        zone.native_monster_snapshots()
            .into_iter()
            .find(|monster| monster.object_id == 9_100)
            .expect("neutral target")
            .hp,
        direct_hp_before
    );

    let materialized_root_before = zone.canonical_state_root().expect("state root");
    let materialized = zone.handle(ZoneCommand::PlayerAttackMaterializedObject {
        session_id: attacker.clone(),
        object_id: 9_101,
        monster: Some(native_neutral_monster_spawn(
            9_101,
            "Royal_Guard",
            6,
            329,
            270,
        )),
        direction: MirDirection::Left,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 999,
        now_ms: 10,
    });
    assert_eq!(materialized.len(), 1);
    assert!(has_packet(&materialized, &attacker, |packet| matches!(
        packet,
        ServerPacket::UserLocation { .. }
    )));
    assert!(!has_packet(&materialized, &observer, |_| true));
    assert!(!zone.has_native_monster(9_101));
    assert_eq!(zone.native_monster_count(), 1);
    assert_eq!(
        zone.canonical_state_root().expect("state root"),
        materialized_root_before
    );

    let accepted = zone.handle(ZoneCommand::PlayerAttackMaterializedObject {
        session_id: attacker.clone(),
        object_id: 9_102,
        monster: Some(native_monster_spawn(9_102, 329, 270)),
        direction: MirDirection::Left,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 999,
        now_ms: 10,
    });
    assert!(has_packet(&accepted, &attacker, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { .. }
    )));
}

#[test]
fn zone_explicit_disposition_overrides_ai0_for_direct_and_materialized_attacks() {
    let attacker = session("melee-attacker");
    let observer = session("melee-observer");
    let mut melee_zone = zone();
    melee_zone.handle(ZoneCommand::Join(join(
        "melee-attacker",
        101,
        "MeleeAttacker",
        330,
        270,
    )));
    melee_zone.handle(ZoneCommand::Join(join(
        "melee-observer",
        102,
        "MeleeObserver",
        330,
        271,
    )));
    admit_melee(&mut melee_zone, &attacker);

    let mut direct_friendly = native_monster_spawn(9_200, 331, 270);
    direct_friendly.ai = 0;
    direct_friendly.disposition = Some(WorldEntityDisposition::Friendly);
    melee_zone.handle(ZoneCommand::SpawnMonster {
        session_id: attacker.clone(),
        monster: direct_friendly,
        now_ms: 0,
    });
    let direct_root = melee_zone.canonical_state_root().expect("state root");
    let direct = melee_zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: attacker.clone(),
        object_id: 9_200,
        direction: MirDirection::Right,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 999,
        now_ms: 10,
    });
    assert_owner_only_location_correction(&direct, &attacker, &observer);
    assert_eq!(
        melee_zone.canonical_state_root().expect("state root"),
        direct_root
    );

    let mut materialized_friendly = native_monster_spawn(9_201, 329, 270);
    materialized_friendly.ai = 0;
    materialized_friendly.disposition = Some(WorldEntityDisposition::Friendly);
    let materialized_root = melee_zone.canonical_state_root().expect("state root");
    let materialized = melee_zone.handle(ZoneCommand::PlayerAttackMaterializedObject {
        session_id: attacker.clone(),
        object_id: 9_201,
        monster: Some(materialized_friendly),
        direction: MirDirection::Left,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 999,
        now_ms: 10,
    });
    assert_owner_only_location_correction(&materialized, &attacker, &observer);
    assert!(!melee_zone.has_native_monster(9_201));
    assert_eq!(
        melee_zone.canonical_state_root().expect("state root"),
        materialized_root
    );

    let hostile_ai0 = native_monster_spawn(9_202, 329, 270);
    assert_eq!(hostile_ai0.ai, 0);
    assert_eq!(
        hostile_ai0.disposition,
        Some(WorldEntityDisposition::Hostile)
    );
    let accepted_melee = melee_zone.handle(ZoneCommand::PlayerAttackMaterializedObject {
        session_id: attacker.clone(),
        object_id: 9_202,
        monster: Some(hostile_ai0),
        direction: MirDirection::Left,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 999,
        now_ms: 10,
    });
    assert!(has_packet(&accepted_melee, &attacker, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { .. }
    )));

    let archer = session("range-attacker");
    let range_observer = session("range-observer");
    let mut range_zone = zone();
    range_zone.handle(ZoneCommand::Join(join_archer(
        "range-attacker",
        201,
        "RangeAttacker",
        330,
        270,
    )));
    range_zone.handle(ZoneCommand::Join(join(
        "range-observer",
        202,
        "RangeObserver",
        330,
        271,
    )));
    admit_archer_range(&mut range_zone, &archer);

    let mut direct_friendly_range = native_monster_spawn(9_210, 339, 270);
    direct_friendly_range.ai = 0;
    direct_friendly_range.disposition = Some(WorldEntityDisposition::Friendly);
    range_zone.handle(ZoneCommand::SpawnMonster {
        session_id: archer.clone(),
        monster: direct_friendly_range,
        now_ms: 0,
    });
    let direct_range_root = range_zone.canonical_state_root().expect("state root");
    let direct_range = range_zone.handle(ZoneCommand::PlayerRangeAttackObject {
        session_id: archer.clone(),
        object_id: 9_210,
        direction: MirDirection::Right,
        target: Point { x: 339, y: 270 },
        spell: Spell::None,
        level: 0,
        attack_type: 0,
        damage: 999,
        now_ms: 10,
    });
    assert_owner_only_location_correction(&direct_range, &archer, &range_observer);
    assert_eq!(
        range_zone.canonical_state_root().expect("state root"),
        direct_range_root
    );

    let mut materialized_friendly_range = native_monster_spawn(9_211, 338, 270);
    materialized_friendly_range.ai = 0;
    materialized_friendly_range.disposition = Some(WorldEntityDisposition::Friendly);
    let materialized_range_root = range_zone.canonical_state_root().expect("state root");
    let materialized_range = range_zone.handle(ZoneCommand::PlayerRangeAttackMaterializedObject {
        session_id: archer.clone(),
        object_id: 9_211,
        monster: Some(materialized_friendly_range),
        direction: MirDirection::Right,
        target: Point { x: 338, y: 270 },
        spell: Spell::None,
        level: 0,
        attack_type: 0,
        damage: 999,
        now_ms: 10,
    });
    assert_owner_only_location_correction(&materialized_range, &archer, &range_observer);
    assert!(!range_zone.has_native_monster(9_211));
    assert_eq!(
        range_zone.canonical_state_root().expect("state root"),
        materialized_range_root
    );

    let hostile_range_ai0 = native_monster_spawn(9_212, 338, 270);
    let accepted_range = range_zone.handle(ZoneCommand::PlayerRangeAttackMaterializedObject {
        session_id: archer.clone(),
        object_id: 9_212,
        monster: Some(hostile_range_ai0),
        direction: MirDirection::Right,
        target: Point { x: 338, y: 270 },
        spell: Spell::None,
        level: 0,
        attack_type: 0,
        damage: 999,
        now_ms: 10,
    });
    assert!(has_packet(&accepted_range, &archer, |packet| matches!(
        packet,
        ServerPacket::RangeAttack { .. }
    )));
}

#[test]
fn zone_melee_enforces_fishing_and_authoritative_mount_attack_capability() {
    for (label, riding_mount, mount_type, mount_attack_allowed, fishing, accepted) in [
        ("fishing", false, -1, true, true, false),
        ("mounted without bells", true, 5, false, false, false),
        ("mounted with bells", true, 5, true, false, true),
        ("dismounted without bells", false, -1, true, false, true),
    ] {
        let mut zone = zone();
        let attacker = session("attacker");
        zone.handle(ZoneCommand::Join(join(
            "attacker", 101, "Attacker", 330, 270,
        )));
        zone.handle(ZoneCommand::BroadcastPackets {
            session_id: attacker.clone(),
            owner_local_object_id: 101,
            packets: vec![
                ServerPacket::MountUpdate {
                    object_id: 101,
                    mount_type,
                    riding_mount,
                },
                ServerPacket::FishingUpdate {
                    object_id: 101,
                    fishing,
                    progress_percent: 0,
                    chance_percent: 0,
                    fishing_point: Point { x: 330, y: 270 },
                    found_fish: false,
                },
            ],
            now_ms: 0,
        });
        sync_combat_admission_with_mount_attack(
            &mut zone,
            &attacker,
            MirClass::Warrior,
            false,
            riding_mount,
            mount_attack_allowed,
            false,
            false,
            fishing,
        );
        let before_root = zone.canonical_state_root().expect("state root");
        let outbounds = zone.handle(ZoneCommand::PlayerAttackMaterializedObject {
            session_id: attacker.clone(),
            object_id: 9_120,
            monster: Some(native_monster_spawn(9_120, 331, 270)),
            direction: MirDirection::Right,
            spell: Spell::None as u8,
            level: 0,
            attack_type: 0,
            damage: 999,
            now_ms: 10,
        });

        assert_eq!(
            has_packet(&outbounds, &attacker, |packet| matches!(
                packet,
                ServerPacket::ObjectAttack { .. }
            )),
            accepted,
            "{label}: {outbounds:?}"
        );
        if accepted {
            assert!(zone.has_native_monster(9_120));
        } else {
            assert!(has_packet(&outbounds, &attacker, |packet| matches!(
                packet,
                ServerPacket::UserLocation { .. }
            )));
            assert_eq!(zone.native_monster_count(), 0);
            assert_eq!(
                zone.canonical_state_root().expect("state root"),
                before_root,
                "{label} must not mutate Zone state"
            );
            assert!(!outbounds.iter().any(|outbound| matches!(
                outbound,
                ZoneOutbound::ToMany { packets, .. } | ZoneOutbound::ToAll { packets }
                    if packets.iter().any(|packet| matches!(
                        packet,
                        ServerPacket::ObjectMonster { .. }
                            | ServerPacket::ObjectAttack { .. }
                            | ServerPacket::DamageIndicator { .. }
                    ))
            )));
        }
    }
}

#[test]
fn materialized_range_refresh_rejects_stale_readiness_before_spawn() {
    let mut zone = zone();
    let attacker = session("archer");
    zone.handle(ZoneCommand::Join(join_archer(
        "archer", 101, "Archer", 330, 270,
    )));
    admit_archer_range(&mut zone, &attacker);
    sync_combat_admission(
        &mut zone,
        &attacker,
        MirClass::Archer,
        true,
        true,
        false,
        false,
        false,
    );
    let before_root = zone.canonical_state_root().expect("state root");
    let rejected = zone.handle(ZoneCommand::PlayerRangeAttackMaterializedObject {
        session_id: attacker.clone(),
        object_id: 9_101,
        monster: Some(native_monster_spawn(9_101, 339, 270)),
        direction: MirDirection::Right,
        target: Point { x: 339, y: 270 },
        spell: Spell::None,
        level: 0,
        attack_type: 0,
        damage: 999,
        now_ms: 10,
    });
    assert!(has_packet(&rejected, &attacker, |packet| matches!(
        packet,
        ServerPacket::UserLocation { .. }
    )));
    assert_eq!(zone.native_monster_count(), 0);
    assert_eq!(
        zone.canonical_state_root().expect("state root"),
        before_root
    );
}

#[test]
fn materialized_range_rejects_neutral_ai_atomically() {
    let mut zone = zone();
    let attacker = session("archer");
    zone.handle(ZoneCommand::Join(join_archer(
        "archer", 101, "Archer", 330, 270,
    )));
    admit_archer_range(&mut zone, &attacker);
    let before_root = zone.canonical_state_root().expect("state root");
    let rejected = zone.handle(ZoneCommand::PlayerRangeAttackMaterializedObject {
        session_id: attacker.clone(),
        object_id: 9_102,
        monster: Some(native_neutral_monster_spawn(
            9_102,
            "Royal_Guard",
            1,
            339,
            270,
        )),
        direction: MirDirection::Right,
        target: Point { x: 339, y: 270 },
        spell: Spell::None,
        level: 0,
        attack_type: 0,
        damage: 999,
        now_ms: 10,
    });

    assert_eq!(rejected.len(), 1);
    assert!(has_packet(&rejected, &attacker, |packet| matches!(
        packet,
        ServerPacket::UserLocation { .. }
    )));
    assert!(!rejected.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::ToMany { packets, .. } | ZoneOutbound::ToAll { packets }
            if packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::ObjectMonster { .. }
                    | ServerPacket::ObjectRangeAttack { .. }
                    | ServerPacket::DamageIndicator { .. }
            ))
    )));
    assert_eq!(zone.native_monster_count(), 0);
    assert_eq!(
        zone.canonical_state_root().expect("state root"),
        before_root
    );
}

#[test]
fn harvest_admission_is_trusted_and_fail_closed() {
    let mut zone = zone();
    let owner = session("owner");
    zone.handle(ZoneCommand::Join(join("owner", 101, "Owner", 330, 270)));
    assert!(!zone.player_harvest_admitted(&owner, 10));
    for (mounted, dead, blocked, fishing) in [
        (true, false, false, false),
        (false, true, false, false),
        (false, false, true, false),
        (false, false, false, true),
    ] {
        sync_combat_admission(
            &mut zone,
            &owner,
            MirClass::Warrior,
            false,
            mounted,
            dead,
            blocked,
            fishing,
        );
        assert!(!zone.player_harvest_admitted(&owner, 10));
    }
    admit_melee(&mut zone, &owner);
    assert!(zone.player_harvest_admitted(&owner, 10));
}

#[test]
fn observer_actions_require_the_authenticated_owner_actor() {
    let mut zone = zone();
    let owner = session("owner");
    let observer = session("observer");
    zone.handle(ZoneCommand::Join(join("owner", 101, "Owner", 330, 270)));
    zone.handle(ZoneCommand::Join(join(
        "observer", 102, "Observer", 331, 270,
    )));
    let spoofed = vec![
        ServerPacket::ObjectAttack {
            info: ObjectAttackInfo {
                object_id: 999,
                location: Point { x: 330, y: 270 },
                direction: MirDirection::Right,
                spell: 0,
                level: 0,
                attack_type: 0,
            },
        },
        ServerPacket::ObjectRangeAttack {
            info: ObjectRangeAttackInfo {
                object_id: 999,
                location: Point { x: 330, y: 270 },
                direction: MirDirection::Right,
                target_id: 500,
                target: Point { x: 332, y: 270 },
                attack_type: 0,
                spell: 0,
                level: 0,
            },
        },
        ServerPacket::ObjectMagic {
            object_id: 999,
            location: Point { x: 330, y: 270 },
            direction: MirDirection::Right,
            spell: Spell::FireBall,
            target_id: 500,
            target: Point { x: 332, y: 270 },
            cast: true,
            level: 1,
            self_broadcast: false,
            secondary_target_ids: Vec::new(),
        },
        ServerPacket::ObjectSpell {
            info: ObjectSpellInfo {
                object_id: 999,
                location: Point { x: 330, y: 270 },
                spell: Spell::FireBall,
                direction: MirDirection::Right,
                param: false,
            },
        },
        ServerPacket::ObjectHarvest {
            movement: ObjectMovement {
                object_id: 999,
                position: Point { x: 330, y: 270 },
                direction: MirDirection::Right,
            },
        },
    ];
    let rejected = zone.handle(ZoneCommand::BroadcastPackets {
        session_id: owner.clone(),
        owner_local_object_id: 101,
        packets: spoofed.clone(),
        now_ms: 10,
    });
    assert!(!has_packet(&rejected, &observer, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { .. }
            | ServerPacket::ObjectRangeAttack { .. }
            | ServerPacket::ObjectMagic { .. }
            | ServerPacket::ObjectSpell { .. }
            | ServerPacket::ObjectHarvest { .. }
    )));

    let identity_missing = zone.handle(ZoneCommand::BroadcastSharedObjectPackets {
        session_id: owner,
        local_self_object_id: None,
        packets: spoofed,
        now_ms: 11,
    });
    assert!(!has_packet(
        &identity_missing,
        &observer,
        |packet| matches!(
            packet,
            ServerPacket::ObjectAttack { .. }
                | ServerPacket::ObjectRangeAttack { .. }
                | ServerPacket::ObjectMagic { .. }
                | ServerPacket::ObjectSpell { .. }
                | ServerPacket::ObjectHarvest { .. }
        )
    ));
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
        assert!(
            has_packet(&resolved, target, |packet| matches!(
                packet,
                ServerPacket::ObjectHealth { info }
                    if info.object_id == 9100 && info.percent == 80
            )),
            "expected primary FireBang health update for {target:?}, got {resolved:?}"
        );
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
    assert!(
        has_packet(&released_tick, &first, |packet| matches!(
            packet,
                ServerPacket::ObjectWalk { movement }
                    if movement.object_id == 9100
                    && movement.position == (Point { x: 331, y: 270 })
                    && movement.direction == MirDirection::Left
        )),
        "expected monster AI to resume after Entrapment expiry: {released_tick:?}"
    );
}

#[test]
fn zone_native_player_buff_stats_authoritatively_modify_damage_until_expiry() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    admit_melee(&mut zone, &first);
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
    assert!(
        !has_packet(&expired, &first, |packet| matches!(
            packet,
            ServerPacket::RemoveBuff { object_id, buff_type }
                if *object_id == 101 && *buff_type == 5
        )),
        "the Zone must not duplicate the personal session's owner expiry packet: {expired:?}"
    );
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9101, 329, 270),
        now_ms: 610,
    });
    zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: first.clone(),
        object_id: 9101,
        direction: MirDirection::Left,
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
    assert!(
        has_packet(&delayed, &first, |packet| matches!(
            packet,
            ServerPacket::ObjectSpell { info }
                if info.spell == Spell::HealingCircle
                    && info.object_id >= 1_000_000
                    && info.location == (Point { x: 330, y: 270 })
        )),
        "expected HealingCircle ground spell packet: {delayed:?}"
    );
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

    zone.handle(ZoneCommand::Join(join_archer(
        "first", 101, "Scout", 330, 270,
    )));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 332, 270)));
    admit_archer_range(&mut zone, &first);
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
fn living_native_monsters_do_not_walk_onto_the_same_tile() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9101, 332, 270),
        now_ms: 0,
    });
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: first.clone(),
        monster: native_monster_spawn(9100, 333, 270),
        now_ms: 0,
    });

    let outbounds = zone.tick(0);

    assert!(!has_packet(&outbounds, &first, |packet| matches!(
        packet,
        ServerPacket::ObjectWalk { movement }
            if movement.object_id == 9100 && movement.position == (Point { x: 332, y: 270 })
    )));
    let snapshots = zone.native_monster_snapshots();
    assert_eq!(
        snapshots
            .iter()
            .map(|monster| (monster.position.x, monster.position.y))
            .collect::<BTreeSet<_>>()
            .len(),
        snapshots.len()
    );
    assert_eq!(
        snapshots
            .iter()
            .find(|monster| monster.object_id == 9100)
            .map(|monster| monster.position.clone()),
        Some(Point { x: 333, y: 270 })
    );
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
        ZoneOutbound::GroundDropClaimedWithTicket { session_id, ticket }
            if session_id == &second && ticket.object_id == 8001
    )));
}

#[test]
fn zone_ground_drop_owner_group_and_unowned_protection_follow_crystal_semantics() {
    let mut zone = zone();
    let owner = session("owner");
    let group_member = session("group-member");
    let stranger = session("stranger");

    zone.handle(ZoneCommand::Join(join("owner", 101, "Owner", 330, 270)));
    zone.handle(ZoneCommand::Join(join(
        "group-member",
        102,
        "GroupMember",
        330,
        270,
    )));
    zone.handle(ZoneCommand::Join(join(
        "stranger", 103, "Stranger", 330, 270,
    )));

    zone.handle(ZoneCommand::SyncGroundDrops {
        session_id: owner.clone(),
        drops: vec![gold_drop(8101, 330, 270, Some(101), Some(1))],
        now_ms: 0,
    });
    let owner_claim = zone.handle(ZoneCommand::ClaimGroundDrop {
        session_id: owner.clone(),
        object_id: Some(8101),
        target: Point { x: 330, y: 270 },
        group_members: Vec::new(),
        now_ms: 0,
    });
    assert!(owner_claim.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::GroundDropClaimedWithTicket { session_id, ticket }
            if session_id == &owner && ticket.object_id == 8101
    )));
    let duplicate_owner_claim = zone.handle(ZoneCommand::ClaimGroundDrop {
        session_id: owner.clone(),
        object_id: Some(8101),
        target: Point { x: 330, y: 270 },
        group_members: Vec::new(),
        now_ms: 0,
    });
    assert!(!duplicate_owner_claim
        .iter()
        .any(|outbound| matches!(outbound, ZoneOutbound::GroundDropClaimedWithTicket { .. })));

    zone.handle(ZoneCommand::SyncGroundDrops {
        session_id: owner.clone(),
        drops: vec![gold_drop(8102, 330, 270, Some(101), Some(1))],
        now_ms: 0,
    });
    let group_claim = zone.handle(ZoneCommand::ClaimGroundDrop {
        session_id: group_member.clone(),
        object_id: Some(8102),
        target: Point { x: 330, y: 270 },
        group_members: vec!["oWnEr".to_string()],
        now_ms: 0,
    });
    assert!(group_claim.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::GroundDropClaimedWithTicket { session_id, ticket }
            if session_id == &group_member && ticket.object_id == 8102
    )));

    zone.handle(ZoneCommand::SyncGroundDrops {
        session_id: owner.clone(),
        drops: vec![gold_drop(8103, 330, 270, Some(101), Some(1))],
        now_ms: 0,
    });
    let blocked = zone.handle(ZoneCommand::ClaimGroundDrop {
        session_id: stranger.clone(),
        object_id: Some(8103),
        target: Point { x: 330, y: 270 },
        group_members: Vec::new(),
        now_ms: 0,
    });
    assert!(has_packet(&blocked, &stranger, |packet| matches!(
        packet,
        ServerPacket::Chat { message, .. } if message == "server.CannotPickupNotOwner"
    )));
    assert!(zone.has_ground_drop(8103));
    let expired_claim = zone.handle(ZoneCommand::ClaimGroundDrop {
        session_id: stranger.clone(),
        object_id: Some(8103),
        target: Point { x: 330, y: 270 },
        group_members: Vec::new(),
        now_ms: 301,
    });
    assert!(expired_claim.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::GroundDropClaimedWithTicket { session_id, ticket }
            if session_id == &stranger && ticket.object_id == 8103
    )));

    zone.handle(ZoneCommand::SyncGroundDrops {
        session_id: owner,
        drops: vec![gold_drop(8104, 330, 270, None, Some(1))],
        now_ms: 0,
    });
    let unowned_claim = zone.handle(ZoneCommand::ClaimGroundDrop {
        session_id: stranger,
        object_id: Some(8104),
        target: Point { x: 330, y: 270 },
        group_members: Vec::new(),
        now_ms: 0,
    });
    assert!(unowned_claim.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::GroundDropClaimedWithTicket { ticket, .. } if ticket.object_id == 8104
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
        .any(|outbound| matches!(outbound, ZoneOutbound::GroundDropClaimedWithTicket { .. })));
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
        ZoneOutbound::GroundDropClaimedWithTicket { session_id, ticket }
            if session_id == &first && ticket.object_id == 8001
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
    let claimed = zone.handle(ZoneCommand::ClaimGroundDrop {
        session_id: first.clone(),
        object_id: Some(8001),
        target: Point { x: 330, y: 270 },
        group_members: Vec::new(),
        now_ms: 0,
    });
    let ticket = ground_drop_claim_ticket(&claimed, &first, 8001);
    zone.handle(ZoneCommand::CommitGroundDropClaimWithTicket {
        session_id: first,
        ticket,
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
    let claimed = zone.handle(ZoneCommand::ClaimGroundDrop {
        session_id: first.clone(),
        object_id: Some(8001),
        target: Point { x: 330, y: 270 },
        group_members: Vec::new(),
        now_ms: 0,
    });
    let ticket = ground_drop_claim_ticket(&claimed, &first, 8001);

    let restored = zone.handle(ZoneCommand::CancelGroundDropClaimWithTicket {
        session_id: first,
        ticket,
        now_ms: 10,
    });
    assert!(has_packet(&restored, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectGold { info } if info.object_id == 8001 && info.gold == 25
    )));
}

#[test]
fn detached_ground_drop_claim_survives_session_leave_and_restores_once() {
    // Strict Zone checkpoints reconstruct collision from the signed map
    // module. Use the canonical map collision here rather than the unbounded
    // test helper so the state-root comparison exercises a production-shaped
    // restore.
    let mut zone = ZoneRuntime::new(ZoneKey::for_map("0"));
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 330, 270)));
    zone.handle(ZoneCommand::SyncGroundDrops {
        session_id: first.clone(),
        drops: vec![gold_drop(8_021, 330, 270, Some(101), Some(10))],
        now_ms: 0,
    });
    let claimed = zone.handle(ZoneCommand::ClaimGroundDrop {
        session_id: first.clone(),
        object_id: Some(8_021),
        target: Point { x: 330, y: 270 },
        group_members: Vec::new(),
        now_ms: 1,
    });
    let ticket = ground_drop_claim_ticket(&claimed, &first, 8_021);

    assert!(zone.detach_ground_drop_claim(&first, &ticket));
    assert!(zone.pending_ground_drop_claim_tickets().is_empty());
    assert!(zone.has_detached_ground_drop_claim_ticket(&ticket));
    assert!(!zone.has_ground_drop(8_021));
    zone.handle(ZoneCommand::Leave { session_id: first });

    let checkpoint = zone.checkpoint_bytes().expect("detached Zone checkpoint");
    let mut restored =
        ZoneRuntime::restore_checkpoint(&checkpoint).expect("restore detached Zone checkpoint");
    assert!(restored.has_detached_ground_drop_claim_ticket(&ticket));
    let outbounds = restored
        .restore_detached_ground_drop_claim(&ticket, 10)
        .expect("definitive rejection restores detached claim");
    assert!(has_packet(&outbounds, &second, |packet| matches!(
        packet,
        ServerPacket::ObjectGold { info } if info.object_id == 8_021 && info.gold == 25
    )));
    assert!(restored.has_ground_drop(8_021));
    assert!(restored
        .restore_detached_ground_drop_claim(&ticket, 11)
        .is_none());
    assert_eq!(restored.ground_drop_count(), 1);
}

#[test]
fn zone_ground_drop_reclaim_uses_fresh_claim_id_and_stable_economic_key() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::SyncGroundDrops {
        session_id: first.clone(),
        drops: vec![gold_drop(8_111, 330, 270, Some(101), Some(10))],
        now_ms: 0,
    });

    let first_claim = zone.handle(ZoneCommand::ClaimGroundDrop {
        session_id: first.clone(),
        object_id: Some(8_111),
        target: Point { x: 330, y: 270 },
        group_members: Vec::new(),
        now_ms: 1,
    });
    let first_ticket = ground_drop_claim_ticket(&first_claim, &first, 8_111);
    zone.handle(ZoneCommand::CancelGroundDropClaimWithTicket {
        session_id: first.clone(),
        ticket: first_ticket.clone(),
        now_ms: 2,
    });

    let reclaimed = zone.handle(ZoneCommand::ClaimGroundDrop {
        session_id: first.clone(),
        object_id: Some(8_111),
        target: Point { x: 330, y: 270 },
        group_members: Vec::new(),
        now_ms: 3,
    });
    let reclaimed_ticket = ground_drop_claim_ticket(&reclaimed, &first, 8_111);

    assert_ne!(reclaimed_ticket.claim_id, first_ticket.claim_id);
    assert_eq!(
        reclaimed_ticket.drop_generation,
        first_ticket.drop_generation
    );
    assert_eq!(reclaimed_ticket.payload_digest, first_ticket.payload_digest);
    assert_eq!(
        reclaimed_ticket.idempotency_key,
        first_ticket.idempotency_key
    );
}

#[test]
fn zone_ground_drop_ticket_tampering_and_legacy_commands_fail_closed() {
    let mut zone = zone();
    let first = session("first");
    let second = session("second");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::Join(join("second", 102, "Blade", 330, 270)));
    zone.handle(ZoneCommand::SyncGroundDrops {
        session_id: first.clone(),
        drops: vec![gold_drop(8_101, 330, 270, Some(101), Some(10))],
        now_ms: 0,
    });
    let claimed = zone.handle(ZoneCommand::ClaimGroundDrop {
        session_id: first.clone(),
        object_id: Some(8_101),
        target: Point { x: 330, y: 270 },
        group_members: Vec::new(),
        now_ms: 0,
    });
    let ticket = ground_drop_claim_ticket(&claimed, &first, 8_101);

    assert!(zone
        .handle(ZoneCommand::CommitGroundDropClaim {
            session_id: first.clone(),
            object_id: 8_101,
        })
        .is_empty());
    assert!(zone
        .handle(ZoneCommand::CancelGroundDropClaim {
            session_id: first.clone(),
            object_id: 8_101,
            now_ms: 1,
        })
        .is_empty());
    assert!(!zone.has_ground_drop(8_101));

    let mut tampered = Vec::new();
    let mut value = ticket.clone();
    value.session_id = second.clone();
    tampered.push(value);
    let mut value = ticket.clone();
    value.claim_id += 1;
    tampered.push(value);
    let mut value = ticket.clone();
    value.drop_generation += 1;
    tampered.push(value);
    let mut value = ticket.clone();
    value.payload_digest = "0".repeat(64);
    tampered.push(value);
    let mut value = ticket.clone();
    value.idempotency_key.push_str(":forged");
    tampered.push(value);
    let mut value = ticket.clone();
    value.owner_object_id = Some(999);
    tampered.push(value);
    let mut value = ticket.clone();
    value.drop.quantity += 1;
    tampered.push(value);

    assert!(zone
        .handle(ZoneCommand::CommitGroundDropClaimWithTicket {
            session_id: second,
            ticket: ticket.clone(),
        })
        .is_empty());
    for forged in tampered {
        assert!(zone
            .handle(ZoneCommand::CommitGroundDropClaimWithTicket {
                session_id: first.clone(),
                ticket: forged.clone(),
            })
            .is_empty());
        assert!(zone
            .handle(ZoneCommand::CancelGroundDropClaimWithTicket {
                session_id: first.clone(),
                ticket: forged,
                now_ms: 2,
            })
            .is_empty());
        assert!(!zone.has_ground_drop(8_101));
    }

    zone.handle(ZoneCommand::CancelGroundDropClaimWithTicket {
        session_id: first,
        ticket,
        now_ms: 3,
    });
    assert!(zone.has_ground_drop(8_101));
}

#[test]
fn zone_ground_drop_ticket_prevents_aba_and_duplicate_followups() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::SyncGroundDrops {
        session_id: first.clone(),
        drops: vec![gold_drop(8_201, 330, 270, None, None)],
        now_ms: 0,
    });
    let first_claim = zone.handle(ZoneCommand::ClaimGroundDrop {
        session_id: first.clone(),
        object_id: Some(8_201),
        target: Point { x: 330, y: 270 },
        group_members: Vec::new(),
        now_ms: 0,
    });
    let old_ticket = ground_drop_claim_ticket(&first_claim, &first, 8_201);
    zone.handle(ZoneCommand::CancelGroundDropClaimWithTicket {
        session_id: first.clone(),
        ticket: old_ticket.clone(),
        now_ms: 1,
    });

    let mut reincarnated = gold_drop(8_201, 330, 270, None, None);
    reincarnated.loot = GroundDropLootSnapshot::Gold { amount: 26 };
    zone.handle(ZoneCommand::SyncGroundDrops {
        session_id: first.clone(),
        drops: vec![reincarnated],
        now_ms: 2,
    });
    let second_claim = zone.handle(ZoneCommand::ClaimGroundDrop {
        session_id: first.clone(),
        object_id: Some(8_201),
        target: Point { x: 330, y: 270 },
        group_members: Vec::new(),
        now_ms: 2,
    });
    let new_ticket = ground_drop_claim_ticket(&second_claim, &first, 8_201);
    assert!(new_ticket.drop_generation > old_ticket.drop_generation);
    assert!(new_ticket.claim_id > old_ticket.claim_id);
    assert_ne!(new_ticket.payload_digest, old_ticket.payload_digest);
    assert_ne!(new_ticket.idempotency_key, old_ticket.idempotency_key);

    zone.handle(ZoneCommand::CommitGroundDropClaimWithTicket {
        session_id: first.clone(),
        ticket: old_ticket.clone(),
    });
    zone.handle(ZoneCommand::CancelGroundDropClaimWithTicket {
        session_id: first.clone(),
        ticket: old_ticket,
        now_ms: 3,
    });
    assert!(!zone.has_ground_drop(8_201));

    zone.handle(ZoneCommand::CommitGroundDropClaimWithTicket {
        session_id: first.clone(),
        ticket: new_ticket.clone(),
    });
    zone.handle(ZoneCommand::CommitGroundDropClaimWithTicket {
        session_id: first.clone(),
        ticket: new_ticket.clone(),
    });
    zone.handle(ZoneCommand::CancelGroundDropClaimWithTicket {
        session_id: first,
        ticket: new_ticket,
        now_ms: 4,
    });
    assert!(!zone.has_ground_drop(8_201));
}

#[test]
fn zone_ground_drop_countdown_changes_keep_generation_but_not_claim_identity() {
    let mut zone = zone();
    let first = session("first");
    zone.handle(ZoneCommand::Join(join("first", 101, "Scout", 330, 270)));
    zone.handle(ZoneCommand::SyncGroundDrops {
        session_id: first.clone(),
        drops: vec![gold_drop(8_301, 330, 270, Some(101), Some(10))],
        now_ms: 0,
    });
    let first_claim = zone.handle(ZoneCommand::ClaimGroundDrop {
        session_id: first.clone(),
        object_id: Some(8_301),
        target: Point { x: 330, y: 270 },
        group_members: Vec::new(),
        now_ms: 0,
    });
    let first_ticket = ground_drop_claim_ticket(&first_claim, &first, 8_301);
    zone.handle(ZoneCommand::CancelGroundDropClaimWithTicket {
        session_id: first.clone(),
        ticket: first_ticket.clone(),
        now_ms: 1,
    });
    zone.handle(ZoneCommand::SyncGroundDrops {
        session_id: first.clone(),
        drops: vec![gold_drop(8_301, 330, 270, Some(101), Some(9))],
        now_ms: 2,
    });
    let second_claim = zone.handle(ZoneCommand::ClaimGroundDrop {
        session_id: first.clone(),
        object_id: Some(8_301),
        target: Point { x: 330, y: 270 },
        group_members: Vec::new(),
        now_ms: 2,
    });
    let second_ticket = ground_drop_claim_ticket(&second_claim, &first, 8_301);

    assert_eq!(second_ticket.drop_generation, first_ticket.drop_generation);
    assert_eq!(second_ticket.payload_digest, first_ticket.payload_digest);
    assert!(second_ticket.claim_id > first_ticket.claim_id);
    assert_eq!(second_ticket.idempotency_key, first_ticket.idempotency_key);
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
        ZoneOutbound::GroundDropClaimedWithTicket { session_id, ticket }
            if session_id == &first && ticket.object_id == 8001
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
    login_demo_account(&mut session);
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
    login_demo_account(&mut session);
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
fn session_materializes_missing_shared_deer_corpse_and_resets_it_on_respawn() {
    let mut session = SimulationSession::new(SimulationConfig::default());
    login_demo_account(&mut session);
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let mut shared_deer = session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.kind == WorldEntityKind::Monster && entity.max_hp.is_some())
        .expect("starter scene should expose a monster");
    shared_deer.object_id = 9_500_001;
    shared_deer.name = "Deer".to_string();
    shared_deer.ai = Some(2);
    shared_deer.max_hp = Some(25);
    shared_deer.hp = Some(0);
    shared_deer.dead = true;
    shared_deer.sprite = None;
    let corpse_position = Point {
        x: shared_deer.x,
        y: shared_deer.y,
    };
    session.force_authoritative_player_transform(
        Point {
            x: corpse_position.x + 1,
            y: corpse_position.y,
        },
        MirDirection::Left,
    );

    assert!(session.apply_shared_entity_snapshot(&shared_deer));
    let first = session.handle_packet(ClientPacket::Harvest {
        direction: MirDirection::Left,
    });
    assert!(first.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectHarvest { movement }
            if movement.position.x == corpse_position.x + 1
                && movement.position.y == corpse_position.y
    )));

    let mut harvested = first;
    for _ in 0..5 {
        harvested.extend(session.handle_packet(ClientPacket::Harvest {
            direction: MirDirection::Left,
        }));
    }
    assert!(harvested.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectHarvested { movement }
            if movement.object_id == shared_deer.object_id
    )));

    shared_deer.hp = shared_deer.max_hp.or(Some(25));
    shared_deer.dead = false;
    assert!(session.apply_shared_entity_snapshot(&shared_deer));
    shared_deer.hp = Some(0);
    shared_deer.dead = true;
    assert!(session.apply_shared_entity_snapshot(&shared_deer));
    let next_incarnation = session.handle_packet(ClientPacket::Harvest {
        direction: MirDirection::Left,
    });
    assert!(next_incarnation
        .iter()
        .any(|packet| matches!(packet, ServerPacket::ObjectHarvest { .. })));
}

#[test]
fn session_resets_shared_deer_harvest_state_on_explicit_zone_revive() {
    let mut session = SimulationSession::new(SimulationConfig::default());
    login_demo_account(&mut session);
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let mut shared_deer = session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.kind == WorldEntityKind::Monster && entity.max_hp.is_some())
        .expect("starter scene should expose a monster");
    shared_deer.object_id = 9_500_004;
    shared_deer.name = "Deer".to_string();
    shared_deer.ai = Some(2);
    shared_deer.max_hp = Some(25);
    shared_deer.hp = Some(0);
    shared_deer.dead = true;
    shared_deer.sprite = None;
    session.force_authoritative_player_transform(
        Point {
            x: shared_deer.x + 1,
            y: shared_deer.y,
        },
        MirDirection::Left,
    );

    assert!(session.apply_shared_entity_snapshot(&shared_deer));
    let mut first_incarnation = Vec::new();
    for _ in 0..6 {
        first_incarnation.extend(session.handle_packet(ClientPacket::Harvest {
            direction: MirDirection::Left,
        }));
        if first_incarnation.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectHarvested { movement }
                    if movement.object_id == shared_deer.object_id
            )
        }) {
            break;
        }
    }
    assert!(first_incarnation.iter().any(|packet| matches!(
        packet,
        ServerPacket::ObjectHarvested { movement }
            if movement.object_id == shared_deer.object_id
    )));

    session.apply_shared_monster_lifecycle_packets(&[ServerPacket::ObjectRevived {
        info: ObjectRevivedInfo {
            object_id: shared_deer.object_id,
            effect: true,
        },
    }]);
    assert!(session.apply_shared_entity_snapshot(&shared_deer));
    let next_incarnation = session.handle_packet(ClientPacket::Harvest {
        direction: MirDirection::Left,
    });
    assert!(next_incarnation
        .iter()
        .any(|packet| matches!(packet, ServerPacket::ObjectHarvest { .. })));
}

#[test]
fn session_mirrors_zone_monster_death_until_explicit_revive() {
    let mut session = SimulationSession::new(SimulationConfig::default());
    login_demo_account(&mut session);
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let monster = session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| {
            entity.kind == WorldEntityKind::Monster
                && !entity.dead
                && entity.hp.is_some_and(|hp| hp > 0)
        })
        .expect("starter scene should expose a live scheduled monster");
    let death_position = Point {
        x: monster.x,
        y: monster.y,
    };

    session.apply_shared_monster_lifecycle_packets(&[
        ServerPacket::ObjectHealth {
            info: ObjectHealthInfo {
                object_id: monster.object_id,
                percent: 0,
                expire: 0,
            },
        },
        ServerPacket::ObjectDied {
            info: ObjectDiedInfo {
                object_id: monster.object_id,
                location: death_position.clone(),
                direction: monster.direction,
                kind: 0,
            },
        },
    ]);

    let corpse = session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.object_id == monster.object_id)
        .expect("shared death should retain the private corpse");
    assert!(corpse.dead);
    assert_eq!(corpse.hp, Some(0));
    assert_eq!((corpse.x, corpse.y), (death_position.x, death_position.y));
    assert!(
        session
            .zone_monster_spawn_snapshot(monster.object_id)
            .is_none(),
        "a normal Session snapshot must not immediately respawn a Zone corpse"
    );

    session.apply_shared_monster_lifecycle_packets(&[ServerPacket::ObjectRevived {
        info: ObjectRevivedInfo {
            object_id: monster.object_id,
            effect: true,
        },
    }]);
    let revived = session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.object_id == monster.object_id)
        .expect("explicit Zone revive should retain the private mirror");
    assert!(!revived.dead);
    assert!(revived.hp.is_some_and(|hp| hp > 0));
    assert!(session
        .zone_monster_spawn_snapshot(monster.object_id)
        .is_some());
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
    admit_melee(&mut zone, &attacker);
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
    admit_melee(&mut zone, &attacker);
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
    admit_melee(&mut zone, &attacker);
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
        admit_melee(&mut zone, &attacker);
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
    admit_melee(&mut zone, &attacker);
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
        admit_melee(&mut zone, &attacker);
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
    admit_melee(&mut zone, &first);
    admit_melee(&mut zone, &second);
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
    admit_melee(&mut zone, &attacker);
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
        include_owner: false,
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

#[test]
fn level50_warrior_melee_skills_keep_crystal_range_shapes_and_hit_timing() {
    let combat_stats = ZonePlayerCombatStats {
        min_dc: 100,
        max_dc: 100,
        accuracy: 100,
        ..ZonePlayerCombatStats::default()
    };

    // Thrusting is the only classic warrior weapon skill that may acquire a
    // target two tiles ahead. At level 3 its 0.25 + 3*0.25 multiplier is 1.0.
    let mut thrusting_zone = zone();
    let warrior = session("warrior");
    let mut warrior_join =
        join_with_combat_stats("warrior", 101, "Warrior", 330, 270, combat_stats);
    warrior_join.level = 50;
    thrusting_zone.handle(ZoneCommand::Join(warrior_join));
    admit_melee(&mut thrusting_zone, &warrior);
    let mut thrust_target = native_monster_spawn(9_100, 332, 270);
    thrust_target.max_hp = 500;
    thrust_target.hp = 500;
    thrusting_zone.handle(ZoneCommand::SpawnMonster {
        session_id: warrior.clone(),
        monster: thrust_target,
        now_ms: 0,
    });
    let launch = thrusting_zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: warrior.clone(),
        object_id: 9_100,
        direction: MirDirection::Right,
        spell: Spell::Thrusting as u8,
        level: 3,
        attack_type: 0,
        damage: 1,
        now_ms: 20,
    });
    assert!(has_packet(&launch, &warrior, |packet| matches!(
        packet,
        ServerPacket::ObjectAttack { info }
            if info.spell == Spell::Thrusting as u8 && info.level == 3
    )));
    assert_eq!(
        damage_indicator_for(&thrusting_zone.tick(20), 9_100),
        Some(100)
    );

    // HalfMoon damages the forward semicircle after 300 ms, while leaving the
    // tile directly behind untouched.
    let mut half_moon_zone = zone();
    let mut warrior_join =
        join_with_combat_stats("warrior", 101, "Warrior", 330, 270, combat_stats);
    warrior_join.level = 50;
    half_moon_zone.handle(ZoneCommand::Join(warrior_join));
    admit_melee(&mut half_moon_zone, &warrior);
    for (object_id, x, y) in [
        (9_100, 331, 270),
        (9_101, 331, 269),
        (9_102, 330, 271),
        (9_103, 329, 270),
    ] {
        let mut spawn = native_monster_spawn(object_id, x, y);
        spawn.max_hp = 500;
        spawn.hp = 500;
        half_moon_zone.handle(ZoneCommand::SpawnMonster {
            session_id: warrior.clone(),
            monster: spawn,
            now_ms: 0,
        });
    }
    half_moon_zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: warrior.clone(),
        object_id: 9_100,
        direction: MirDirection::Right,
        spell: Spell::HalfMoon as u8,
        level: 3,
        attack_type: 0,
        damage: 1,
        now_ms: 20,
    });
    assert_eq!(
        damage_indicator_for(&half_moon_zone.tick(20), 9_100),
        Some(60)
    );
    let half_moon_impact = half_moon_zone.tick(320);
    assert_eq!(damage_indicator_for(&half_moon_impact, 9_101), Some(60));
    assert_eq!(damage_indicator_for(&half_moon_impact, 9_102), Some(60));
    assert_eq!(damage_indicator_for(&half_moon_impact, 9_103), None);

    // CrossHalfMoon adds the rear half of the ring.
    let mut cross_zone = zone();
    let mut warrior_join =
        join_with_combat_stats("warrior", 101, "Warrior", 330, 270, combat_stats);
    warrior_join.level = 50;
    cross_zone.handle(ZoneCommand::Join(warrior_join));
    admit_melee(&mut cross_zone, &warrior);
    for (object_id, x) in [(9_100, 331), (9_101, 329)] {
        let mut spawn = native_monster_spawn(object_id, x, 270);
        spawn.max_hp = 500;
        spawn.hp = 500;
        cross_zone.handle(ZoneCommand::SpawnMonster {
            session_id: warrior.clone(),
            monster: spawn,
            now_ms: 0,
        });
    }
    cross_zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: warrior.clone(),
        object_id: 9_100,
        direction: MirDirection::Right,
        spell: Spell::CrossHalfMoon as u8,
        level: 3,
        attack_type: 0,
        damage: 1,
        now_ms: 20,
    });
    cross_zone.tick(20);
    assert_eq!(damage_indicator_for(&cross_zone.tick(320), 9_101), Some(70));

    // TwinDrakeBlade retains the original two delayed hits and stun surface.
    let mut twin_zone = zone();
    let mut warrior_join =
        join_with_combat_stats("warrior", 101, "Warrior", 330, 270, combat_stats);
    warrior_join.level = 50;
    twin_zone.handle(ZoneCommand::Join(warrior_join));
    admit_melee(&mut twin_zone, &warrior);
    let mut twin_target = native_monster_spawn(9_100, 331, 270);
    twin_target.max_hp = 500;
    twin_target.hp = 500;
    twin_zone.handle(ZoneCommand::SpawnMonster {
        session_id: warrior.clone(),
        monster: twin_target,
        now_ms: 0,
    });
    let twin_launch = twin_zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: warrior.clone(),
        object_id: 9_100,
        direction: MirDirection::Right,
        spell: Spell::TwinDrakeBlade as u8,
        level: 3,
        attack_type: 0,
        damage: 1,
        now_ms: 20,
    });
    assert!(has_packet(&twin_launch, &warrior, |packet| matches!(
        packet,
        ServerPacket::ObjectEffect { info } if info.object_id == 9_100 && info.effect == 5
    )));
    assert!(has_packet(&twin_launch, &warrior, |packet| matches!(
        packet,
        ServerPacket::ObjectPoisoned {
            object_id: 9_100,
            poison: 16
        }
    )));
    assert_eq!(damage_indicator_for(&twin_zone.tick(319), 9_100), None);
    assert_eq!(damage_indicator_for(&twin_zone.tick(320), 9_100), Some(110));
    assert_eq!(damage_indicator_for(&twin_zone.tick(420), 9_100), Some(110));
}

#[test]
fn level50_warrior_entrapment_pulls_and_counterattack_closes_shared_combat_loop() {
    let mut entrapment_zone = zone();
    let warrior = session("warrior");
    let mut warrior_join = join("warrior", 101, "Warrior", 330, 270);
    warrior_join.level = 50;
    entrapment_zone.handle(ZoneCommand::Join(warrior_join));
    entrapment_zone.handle(ZoneCommand::SpawnMonster {
        session_id: warrior.clone(),
        monster: native_monster_spawn(9_100, 336, 270),
        now_ms: 0,
    });
    let cast = entrapment_zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: warrior.clone(),
        object_id: 9_100,
        spell: Spell::Entrapment,
        direction: MirDirection::Right,
        target: Point { x: 336, y: 270 },
        cast: true,
        level: 3,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });
    assert_eq!(
        entrapment_zone
            .native_monster_snapshots()
            .into_iter()
            .find(|monster| monster.object_id == 9_100)
            .expect("entrapped monster")
            .position,
        Point { x: 332, y: 270 }
    );
    assert_eq!(
        packets_for(&cast, &warrior)
            .into_iter()
            .filter(|packet| matches!(
                packet,
                ServerPacket::ObjectPushed {
                    object_id: 9_100,
                    ..
                }
            ))
            .count(),
        4
    );
    assert!(has_packet(&cast, &warrior, |packet| matches!(
        packet,
        ServerPacket::ObjectPoisoned {
            object_id: 9_100,
            poison: 256
        }
    )));

    let mut counter_zone = zone();
    let mut counter_join = join_with_combat_stats(
        "warrior",
        101,
        "Warrior",
        330,
        270,
        ZonePlayerCombatStats {
            min_dc: 100,
            max_dc: 100,
            accuracy: 100,
            ..ZonePlayerCombatStats::default()
        },
    );
    counter_join.level = 50;
    counter_join.hp = 1_000;
    counter_join.max_hp = 1_000;
    counter_zone.handle(ZoneCommand::Join(counter_join));
    counter_zone.handle(ZoneCommand::BroadcastPackets {
        session_id: warrior.clone(),
        owner_local_object_id: 1_001,
        packets: vec![ServerPacket::AddBuff {
            buff: ClientBuff {
                buff_type: 18,
                visible: true,
                object_id: 1_001,
                expire_time: 5_000,
                infinite: false,
                paused: false,
                stats: vec![
                    UserItemStat { stat: 0, value: 20 },
                    UserItemStat { stat: 1, value: 20 },
                    UserItemStat { stat: 2, value: 20 },
                    UserItemStat { stat: 3, value: 20 },
                ],
                values: Vec::new(),
            },
        }],
        now_ms: 0,
    });
    let mut attacker = native_monster_spawn(9_100, 331, 270);
    attacker.name = "Royal_Guard".to_string();
    attacker.max_hp = 500;
    attacker.hp = 500;
    counter_zone.handle(ZoneCommand::SpawnMonster {
        session_id: warrior.clone(),
        monster: attacker,
        now_ms: 0,
    });
    counter_zone.tick(0);
    let incoming = counter_zone.tick(600);
    assert!(has_packet(&incoming, &warrior, |packet| matches!(
        packet,
        ServerPacket::ObjectMagic {
            spell: Spell::CounterAttack,
            target_id: 9_100,
            ..
        }
    )));
    assert!(has_packet(&incoming, &warrior, |packet| matches!(
        packet,
        ServerPacket::RemoveBuff {
            object_id: 101,
            buff_type: 18
        }
    )));
    assert!(has_packet(&incoming, &warrior, |packet| matches!(
        packet,
        ServerPacket::SpellToggle {
            object_id: 101,
            spell: Spell::CounterAttack,
            can_use: false
        }
    )));
    assert_eq!(
        damage_indicator_for(&counter_zone.tick(900), 9_100),
        Some(220)
    );
}

#[test]
fn taoist_poison_item_shape_reaches_shared_zone_authority() {
    for (item_param, expected_poison) in [(1, 1), (2, 2)] {
        let mut poison_zone = zone();
        let taoist = session("taoist");
        let mut taoist_join = join("taoist", 101, "Taoist", 330, 270);
        taoist_join.class = MirClass::Taoist;
        taoist_join.level = 50;
        poison_zone.handle(ZoneCommand::Join(taoist_join));
        poison_zone.handle(ZoneCommand::SpawnMonster {
            session_id: taoist.clone(),
            monster: native_monster_spawn(9_100, 334, 270),
            now_ms: 0,
        });
        let cast = poison_zone.handle(ZoneCommand::PlayerCastMagicWithItem {
            session_id: taoist.clone(),
            object_id: 9_100,
            spell: Spell::Poisoning,
            direction: MirDirection::Right,
            target: Point { x: 334, y: 270 },
            cast: true,
            level: 3,
            damage: 12,
            mp_cost: 0,
            cooldown_ms: 500,
            item_param,
            now_ms: 20,
        });
        assert!(has_packet(&cast, &taoist, |packet| matches!(
            packet,
            ServerPacket::ObjectPoisoned { object_id: 9_100, poison }
                if *poison == expected_poison
        )));
        if expected_poison == 1 {
            assert!(damage_indicator_for(&poison_zone.tick(2_020), 9_100).is_some());
        } else {
            assert_eq!(damage_indicator_for(&poison_zone.tick(2_020), 9_100), None);
        }
    }
}

#[test]
fn level50_wizard_ground_and_direction_spell_families_hit_crystal_shapes() {
    for spell in [Spell::FireBang, Spell::IceStorm] {
        let mut spell_zone = zone();
        let wizard = session("wizard");
        let mut wizard_join = join("wizard", 101, "Wizard", 330, 270);
        wizard_join.class = MirClass::Wizard;
        wizard_join.level = 50;
        spell_zone.handle(ZoneCommand::Join(wizard_join));
        for (object_id, x, y) in [(9_100, 334, 270), (9_101, 335, 271), (9_102, 336, 270)] {
            let mut spawn = native_monster_spawn(object_id, x, y);
            spawn.max_hp = 100;
            spawn.hp = 100;
            spell_zone.handle(ZoneCommand::SpawnMonster {
                session_id: wizard.clone(),
                monster: spawn,
                now_ms: 0,
            });
        }
        let launch = spell_zone.handle(ZoneCommand::PlayerCastMagic {
            session_id: wizard.clone(),
            object_id: 0,
            spell,
            direction: MirDirection::Right,
            target: Point { x: 334, y: 270 },
            cast: true,
            level: 3,
            damage: 10,
            mp_cost: 0,
            cooldown_ms: 500,
            now_ms: 20,
        });
        assert!(has_packet(&launch, &wizard, |packet| matches!(
            packet,
            ServerPacket::ObjectMagic { spell: packet_spell, .. } if *packet_spell == spell
        )));
        let impact = spell_zone.tick(520);
        assert_eq!(damage_indicator_for(&impact, 9_100), Some(10));
        assert_eq!(damage_indicator_for(&impact, 9_101), Some(10));
        assert_eq!(damage_indicator_for(&impact, 9_102), None);
    }

    for (spell, monster_position, expected_damage) in [
        (Spell::HellFire, Point { x: 332, y: 270 }, 10),
        (Spell::Lightning, Point { x: 335, y: 270 }, 10),
        (Spell::ThunderStorm, Point { x: 331, y: 270 }, 1),
        (Spell::FlameField, Point { x: 331, y: 270 }, 10),
        (Spell::BladeAvalanche, Point { x: 333, y: 270 }, 6),
    ] {
        let mut spell_zone = zone();
        let wizard = session("wizard");
        let mut wizard_join = join("wizard", 101, "Wizard", 330, 270);
        wizard_join.class = MirClass::Wizard;
        wizard_join.level = 50;
        spell_zone.handle(ZoneCommand::Join(wizard_join));
        let mut spawn = native_monster_spawn(9_100, monster_position.x, monster_position.y);
        spawn.max_hp = 100;
        spawn.hp = 100;
        spell_zone.handle(ZoneCommand::SpawnMonster {
            session_id: wizard.clone(),
            monster: spawn,
            now_ms: 0,
        });
        let launch = spell_zone.handle(ZoneCommand::PlayerCastMagic {
            session_id: wizard.clone(),
            object_id: 0,
            spell,
            direction: MirDirection::Right,
            target: Point { x: 330, y: 270 },
            cast: true,
            level: 3,
            damage: 10,
            mp_cost: 0,
            cooldown_ms: 500,
            now_ms: 20,
        });
        assert!(has_packet(&launch, &wizard, |packet| matches!(
            packet,
            ServerPacket::ObjectMagic { spell: packet_spell, .. } if *packet_spell == spell
        )));
        assert_eq!(
            damage_indicator_for(&spell_zone.tick(520), 9_100),
            Some(expected_damage),
            "{spell:?} should retain its shared Zone hit shape"
        );
    }
}

#[test]
fn level50_wizard_relocation_clone_and_self_buffs_are_zone_authoritative() {
    for spell in [Spell::Teleport, Spell::Blink] {
        let mut relocation_zone = zone();
        let wizard = session("wizard");
        let mut wizard_join = join("wizard", 101, "Wizard", 330, 270);
        wizard_join.class = MirClass::Wizard;
        wizard_join.level = 50;
        relocation_zone.handle(ZoneCommand::Join(wizard_join));
        let destination = Point { x: 340, y: 270 };
        let cast = relocation_zone.handle(ZoneCommand::PlayerCastMagic {
            session_id: wizard.clone(),
            object_id: 0,
            spell,
            direction: MirDirection::Right,
            target: destination.clone(),
            cast: true,
            level: 3,
            damage: 0,
            mp_cost: 5,
            cooldown_ms: 500,
            now_ms: 20,
        });
        assert_eq!(
            relocation_zone.player_position(&wizard),
            Some(destination.clone())
        );
        assert!(has_packet(&cast, &wizard, |packet| matches!(
            packet,
            ServerPacket::ObjectTeleportOut { object_id: 101, .. }
        )));
        assert!(has_packet(&cast, &wizard, |packet| matches!(
            packet,
            ServerPacket::ObjectTeleportIn { object_id: 101, .. }
        )));
        assert!(cast.iter().any(|outbound| matches!(
            outbound,
            ZoneOutbound::SaveTransform { session_id, position, .. }
                if session_id == &wizard && position == &destination
        )));
    }

    let mut clone_zone = zone();
    let wizard = session("wizard");
    let mut wizard_join = join("wizard", 101, "Wizard", 330, 270);
    wizard_join.class = MirClass::Wizard;
    wizard_join.level = 50;
    clone_zone.handle(ZoneCommand::Join(wizard_join));
    clone_zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: wizard.clone(),
        object_id: 0,
        spell: Spell::Mirroring,
        direction: MirDirection::Right,
        target: Point { x: 331, y: 270 },
        cast: true,
        level: 3,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });
    let clone_spawn = clone_zone.tick(520);
    let clone = clone_zone
        .native_monster_snapshots()
        .into_iter()
        .find(|monster| monster.name == "Clone")
        .expect("Mirroring clone should enter the shared Zone");
    assert!(has_packet(&clone_spawn, &wizard, |packet| matches!(
        packet,
        ServerPacket::ObjectMonster { info } if info.name == "Clone"
    )));
    let recast = clone_zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: wizard.clone(),
        object_id: 0,
        spell: Spell::Mirroring,
        direction: MirDirection::Right,
        target: Point { x: 331, y: 270 },
        cast: true,
        level: 3,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 1_000,
    });
    assert!(has_packet(&recast, &wizard, |packet| matches!(
        packet,
        ServerPacket::ObjectDied { info } if info.object_id == clone.object_id
    )));

    for (spell, buff_type, expected_stat) in [
        (Spell::ProtectionField, 10, 1),
        (Spell::Rage, 11, 5),
        (Spell::Fury, 5, 14),
        (Spell::MagicBooster, 21, 7),
    ] {
        let mut buff_zone = zone();
        let wizard = session("wizard");
        let mut wizard_join = join_with_combat_stats(
            "wizard",
            101,
            "Wizard",
            330,
            270,
            ZonePlayerCombatStats {
                min_dc: 40,
                max_dc: 40,
                min_mc: 40,
                max_mc: 40,
                min_ac: 20,
                max_ac: 20,
                ..ZonePlayerCombatStats::default()
            },
        );
        wizard_join.class = MirClass::Wizard;
        wizard_join.level = 50;
        buff_zone.handle(ZoneCommand::Join(wizard_join));
        let cast = buff_zone.handle(ZoneCommand::PlayerCastMagic {
            session_id: wizard.clone(),
            object_id: 0,
            spell,
            direction: MirDirection::Right,
            target: Point { x: 330, y: 270 },
            cast: true,
            level: 3,
            damage: 1,
            mp_cost: 0,
            cooldown_ms: 500,
            now_ms: 20,
        });
        assert!(has_packet(&cast, &wizard, |packet| matches!(
            packet,
            ServerPacket::AddBuff { buff }
                if buff.buff_type == buff_type
                    && buff.stats.iter().any(|stat| stat.stat == expected_stat)
        )));
    }
}

#[test]
fn level50_wizard_single_target_specializations_keep_control_and_monster_rules() {
    let mut frost_zone = zone();
    let wizard = session("wizard");
    let mut wizard_join = join("wizard", 101, "Wizard", 330, 270);
    wizard_join.class = MirClass::Wizard;
    wizard_join.level = 50;
    frost_zone.handle(ZoneCommand::Join(wizard_join));
    frost_zone.handle(ZoneCommand::SpawnMonster {
        session_id: wizard.clone(),
        monster: native_monster_spawn(9_100, 334, 270),
        now_ms: 0,
    });
    let frost = frost_zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: wizard.clone(),
        object_id: 9_100,
        spell: Spell::FrostCrunch,
        direction: MirDirection::Right,
        target: Point { x: 334, y: 270 },
        cast: true,
        level: 3,
        damage: 10,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });
    assert!(has_packet(&frost, &wizard, |packet| matches!(
        packet,
        ServerPacket::ObjectPoisoned {
            object_id: 9_100,
            poison: 8
        }
    )));
    assert_eq!(damage_indicator_for(&frost_zone.tick(20), 9_100), Some(10));

    let mut undead_zone = zone();
    let mut wizard_join = join("wizard", 101, "Wizard", 330, 270);
    wizard_join.class = MirClass::Wizard;
    wizard_join.level = 50;
    undead_zone.handle(ZoneCommand::Join(wizard_join));
    let mut undead = native_monster_spawn(9_100, 334, 270);
    undead.name = "BoneFamiliar".to_string();
    undead.max_hp = 100;
    undead.hp = 100;
    undead.level = 10;
    undead_zone.handle(ZoneCommand::SpawnMonster {
        session_id: wizard.clone(),
        monster: undead,
        now_ms: 0,
    });
    undead_zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: wizard.clone(),
        object_id: 9_100,
        spell: Spell::TurnUndead,
        direction: MirDirection::Right,
        target: Point { x: 334, y: 270 },
        cast: true,
        level: 3,
        damage: 10,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });
    let undead_hit = undead_zone.tick(20);
    assert!(has_packet(&undead_hit, &wizard, |packet| matches!(
        packet,
        ServerPacket::ObjectDied { info } if info.object_id == 9_100
    )));

    let mut living_zone = zone();
    let mut wizard_join = join("wizard", 101, "Wizard", 330, 270);
    wizard_join.class = MirClass::Wizard;
    wizard_join.level = 50;
    living_zone.handle(ZoneCommand::Join(wizard_join));
    living_zone.handle(ZoneCommand::SpawnMonster {
        session_id: wizard.clone(),
        monster: native_monster_spawn(9_100, 334, 270),
        now_ms: 0,
    });
    living_zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: wizard.clone(),
        object_id: 9_100,
        spell: Spell::TurnUndead,
        direction: MirDirection::Right,
        target: Point { x: 334, y: 270 },
        cast: true,
        level: 3,
        damage: 10,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });
    assert_eq!(damage_indicator_for(&living_zone.tick(20), 9_100), None);

    let mut disruptor_zone = zone();
    let mut wizard_join = join("wizard", 101, "Wizard", 330, 270);
    wizard_join.class = MirClass::Wizard;
    wizard_join.level = 50;
    disruptor_zone.handle(ZoneCommand::Join(wizard_join));
    let mut living = native_monster_spawn(9_100, 334, 270);
    living.max_hp = 100;
    living.hp = 100;
    disruptor_zone.handle(ZoneCommand::SpawnMonster {
        session_id: wizard.clone(),
        monster: living,
        now_ms: 0,
    });
    disruptor_zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: wizard.clone(),
        object_id: 9_100,
        spell: Spell::FlameDisruptor,
        direction: MirDirection::Right,
        target: Point { x: 334, y: 270 },
        cast: true,
        level: 3,
        damage: 10,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });
    assert_eq!(
        damage_indicator_for(&disruptor_zone.tick(20), 9_100),
        Some(15)
    );
}

#[test]
fn level50_wizard_vampirism_damages_and_heals_through_shared_authority() {
    let mut vampirism_zone = zone();
    let wizard = session("wizard");
    let mut wizard_join = join("wizard", 101, "Wizard", 330, 270);
    wizard_join.class = MirClass::Wizard;
    wizard_join.level = 50;
    wizard_join.hp = 20;
    wizard_join.max_hp = 60;
    vampirism_zone.handle(ZoneCommand::Join(wizard_join));

    let mut target = native_monster_spawn(9_100, 334, 270);
    target.hp = 100;
    target.max_hp = 100;
    vampirism_zone.handle(ZoneCommand::SpawnMonster {
        session_id: wizard.clone(),
        monster: target,
        now_ms: 0,
    });
    vampirism_zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: wizard.clone(),
        object_id: 9_100,
        spell: Spell::Vampirism,
        direction: MirDirection::Right,
        target: Point { x: 334, y: 270 },
        cast: true,
        level: 2,
        damage: 30,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });
    let impact = vampirism_zone.tick(20);
    assert_eq!(damage_indicator_for(&impact, 9_100), Some(30));
    assert_eq!(vampirism_zone.player_vitals(&wizard), Some((50, 60, 100)));
    assert!(impact.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::PlayerHealed { session_id, amount }
            if session_id == &wizard && *amount == 30
    )));
}

#[test]
fn level50_taoist_support_spells_apply_to_shared_friendly_targets() {
    for (spell, buff_type, target, due_ms) in [
        (Spell::Hiding, 2, Point { x: 330, y: 270 }, 520),
        (Spell::MassHiding, 2, Point { x: 332, y: 270 }, 620),
        (Spell::SoulShield, 6, Point { x: 332, y: 270 }, 620),
        (Spell::BlessedArmour, 7, Point { x: 332, y: 270 }, 620),
    ] {
        let mut support_zone = zone();
        let taoist = session("taoist");
        let friend = session("friend");
        let mut taoist_join = join_with_combat_stats(
            "taoist",
            101,
            "Taoist",
            330,
            270,
            ZonePlayerCombatStats {
                min_sc: 20,
                max_sc: 20,
                ..ZonePlayerCombatStats::default()
            },
        );
        taoist_join.class = MirClass::Taoist;
        taoist_join.level = 50;
        support_zone.handle(ZoneCommand::Join(taoist_join));
        let mut friend_join = join("friend", 102, "Friend", 332, 270);
        friend_join.level = 50;
        support_zone.handle(ZoneCommand::Join(friend_join));
        let cast = support_zone.handle(ZoneCommand::PlayerCastMagic {
            session_id: taoist.clone(),
            object_id: 0,
            spell,
            direction: MirDirection::Right,
            target: target.clone(),
            cast: true,
            level: 3,
            damage: 1,
            mp_cost: 0,
            cooldown_ms: 500,
            now_ms: 20,
        });
        assert!(has_packet(&cast, &taoist, |packet| matches!(
            packet,
            ServerPacket::ObjectMagic { spell: packet_spell, .. } if *packet_spell == spell
        )));
        let impact = support_zone.tick(due_ms);
        let expected_object_id = if spell == Spell::Hiding { 101 } else { 102 };
        assert!(has_packet(&impact, &friend, |packet| matches!(
            packet,
            ServerPacket::AddBuff { buff }
                if buff.object_id == expected_object_id && buff.buff_type == buff_type
        )));
        if matches!(spell, Spell::Hiding | Spell::MassHiding) {
            assert!(has_packet(&impact, &friend, |packet| matches!(
                packet,
                ServerPacket::ObjectHidden { object_id, hidden: true }
                    if *object_id == expected_object_id
            )));
        }
    }

    for (spell, buff_type) in [(Spell::UltimateEnhancer, 9), (Spell::EnergyShield, 20)] {
        let mut support_zone = zone();
        let taoist = session("taoist");
        let friend = session("friend");
        let mut taoist_join = join_with_combat_stats(
            "taoist",
            101,
            "Taoist",
            330,
            270,
            ZonePlayerCombatStats {
                min_sc: 20,
                max_sc: 20,
                ..ZonePlayerCombatStats::default()
            },
        );
        taoist_join.class = MirClass::Taoist;
        taoist_join.level = 50;
        support_zone.handle(ZoneCommand::Join(taoist_join));
        let mut friend_join = join("friend", 102, "Friend", 332, 270);
        friend_join.level = 50;
        support_zone.handle(ZoneCommand::Join(friend_join));
        let cast = support_zone.handle(ZoneCommand::PlayerCastMagic {
            session_id: taoist.clone(),
            object_id: 102,
            spell,
            direction: MirDirection::Right,
            target: Point { x: 332, y: 270 },
            cast: true,
            level: 3,
            damage: 10,
            mp_cost: 0,
            cooldown_ms: 500,
            now_ms: 20,
        });
        assert!(has_packet(&cast, &friend, |packet| matches!(
            packet,
            ServerPacket::AddBuff { buff }
                if buff.object_id == 102 && buff.buff_type == buff_type && !buff.stats.is_empty()
        )));
    }

    let mut healing_zone = zone();
    let taoist = session("taoist");
    let friend = session("friend");
    let mut taoist_join = join("taoist", 101, "Taoist", 330, 270);
    taoist_join.class = MirClass::Taoist;
    taoist_join.level = 50;
    healing_zone.handle(ZoneCommand::Join(taoist_join));
    let mut wounded_friend = join("friend", 102, "Friend", 332, 270);
    wounded_friend.hp = 20;
    healing_zone.handle(ZoneCommand::Join(wounded_friend));
    healing_zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: taoist.clone(),
        object_id: 102,
        spell: Spell::Healing,
        direction: MirDirection::Right,
        target: Point { x: 332, y: 270 },
        cast: true,
        level: 3,
        damage: 10,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });
    let healed = healing_zone.tick(520);
    assert_eq!(healing_zone.player_vitals(&friend), Some((60, 60, 100)));
    assert!(healed.iter().any(|outbound| matches!(
        outbound,
        ZoneOutbound::PlayerHealed { session_id, amount }
            if session_id == &friend && *amount == 40
    )));
}

#[test]
fn level50_taoist_control_debuff_and_revelation_paths_are_shared() {
    let taoist = session("taoist");

    let mut repulsion_zone = zone();
    let mut taoist_join = join("taoist", 101, "Taoist", 330, 270);
    taoist_join.class = MirClass::Taoist;
    taoist_join.level = 50;
    repulsion_zone.handle(ZoneCommand::Join(taoist_join));
    repulsion_zone.handle(ZoneCommand::SpawnMonster {
        session_id: taoist.clone(),
        monster: native_monster_spawn(9_100, 331, 270),
        now_ms: 0,
    });
    let repelled = repulsion_zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: taoist.clone(),
        object_id: 0,
        spell: Spell::EnergyRepulsor,
        direction: MirDirection::Right,
        target: Point { x: 330, y: 270 },
        cast: true,
        level: 3,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });
    assert!(
        has_packet(&repelled, &taoist, |packet| matches!(
            packet,
            ServerPacket::ObjectPushed {
                object_id: 9_100,
                ..
            }
        )),
        "expected EnergyRepulsor push packets: {repelled:?}; monsters={:?}",
        repulsion_zone.native_monster_snapshots()
    );
    assert!(
        repulsion_zone
            .native_monster_snapshots()
            .into_iter()
            .find(|monster| monster.object_id == 9_100)
            .expect("repelled monster")
            .position
            .x
            > 331
    );

    let mut reveal_zone = zone();
    let mut taoist_join = join_with_combat_stats(
        "taoist",
        101,
        "Taoist",
        330,
        270,
        ZonePlayerCombatStats {
            min_sc: 20,
            max_sc: 20,
            ..ZonePlayerCombatStats::default()
        },
    );
    taoist_join.class = MirClass::Taoist;
    taoist_join.level = 50;
    reveal_zone.handle(ZoneCommand::Join(taoist_join));
    reveal_zone.handle(ZoneCommand::SpawnMonster {
        session_id: taoist.clone(),
        monster: native_monster_spawn(9_100, 334, 270),
        now_ms: 0,
    });
    let revealed = reveal_zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: taoist.clone(),
        object_id: 9_100,
        spell: Spell::Revelation,
        direction: MirDirection::Right,
        target: Point { x: 334, y: 270 },
        cast: true,
        level: 3,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });
    assert!(has_packet(&revealed, &taoist, |packet| matches!(
        packet,
        ServerPacket::ObjectHealth { info }
            if info.object_id == 9_100 && info.expire > 0
    )));

    let mut hallucination_zone = zone();
    let mut taoist_join = join("taoist", 101, "Taoist", 330, 270);
    taoist_join.class = MirClass::Taoist;
    taoist_join.level = 50;
    hallucination_zone.handle(ZoneCommand::Join(taoist_join));
    hallucination_zone.handle(ZoneCommand::SpawnMonster {
        session_id: taoist.clone(),
        monster: native_monster_spawn(9_100, 331, 270),
        now_ms: 0,
    });
    hallucination_zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: taoist.clone(),
        object_id: 9_100,
        spell: Spell::Hallucination,
        direction: MirDirection::Right,
        target: Point { x: 331, y: 270 },
        cast: true,
        level: 3,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 0,
    });
    assert!(!has_packet(
        &hallucination_zone.tick(0),
        &taoist,
        |packet| matches!(
            packet,
            ServerPacket::ObjectAttack { info } if info.object_id == 9_100
        )
    ));

    let mut curse_zone = zone();
    let mut taoist_join = join("taoist", 101, "Taoist", 330, 270);
    taoist_join.class = MirClass::Taoist;
    taoist_join.level = 50;
    curse_zone.handle(ZoneCommand::Join(taoist_join));
    curse_zone.handle(ZoneCommand::SpawnMonster {
        session_id: taoist.clone(),
        monster: native_monster_spawn(9_102, 334, 270),
        now_ms: 0,
    });
    curse_zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: taoist.clone(),
        object_id: 0,
        spell: Spell::Curse,
        direction: MirDirection::Right,
        target: Point { x: 334, y: 270 },
        cast: true,
        level: 3,
        damage: 10,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 20,
    });
    let cursed = curse_zone.tick(520);
    assert!(has_packet(&cursed, &taoist, |packet| matches!(
        packet,
        ServerPacket::AddBuff { buff } if buff.object_id == 9_102 && buff.buff_type == 12
    )));
    assert!(has_packet(&cursed, &taoist, |packet| matches!(
        packet,
        ServerPacket::ObjectPoisoned { object_id: 9_102, poison } if *poison & 4 != 0
    )));

    let mut plague_zone = zone();
    let mut taoist_join = join_with_combat_stats(
        "taoist",
        101,
        "Taoist",
        330,
        270,
        ZonePlayerCombatStats {
            min_sc: 10,
            max_sc: 10,
            ..ZonePlayerCombatStats::default()
        },
    );
    taoist_join.class = MirClass::Taoist;
    taoist_join.level = 50;
    plague_zone.handle(ZoneCommand::Join(taoist_join));
    let mut plague_target = native_monster_spawn(9_112, 334, 270);
    plague_target.max_hp = 100;
    plague_target.hp = 100;
    plague_zone.handle(ZoneCommand::SpawnMonster {
        session_id: taoist.clone(),
        monster: plague_target,
        now_ms: 0,
    });
    plague_zone.handle(ZoneCommand::PlayerCastMagicWithItem {
        session_id: taoist.clone(),
        object_id: 0,
        spell: Spell::Plague,
        direction: MirDirection::Right,
        target: Point { x: 334, y: 270 },
        cast: true,
        level: 3,
        damage: 10,
        mp_cost: 0,
        cooldown_ms: 500,
        item_param: 2,
        now_ms: 20,
    });
    let plagued = plague_zone.tick(520);
    assert_eq!(damage_indicator_for(&plagued, 9_112), Some(20));
    assert!(has_packet(&plagued, &taoist, |packet| matches!(
        packet,
        ServerPacket::ObjectPoisoned { object_id: 9_112, poison } if *poison & 2 != 0
    )));
}

#[test]
fn level50_taoist_purification_clears_shared_poison_and_curse_state() {
    let mut purification_zone = zone();
    let poisoner = session("poisoner");
    let taoist = session("taoist");
    let friend = session("friend");

    let mut poisoner_join = join_with_profile("poisoner", 101, "Poisoner", 330, 270, |profile| {
        profile.attack_mode = 5;
    });
    poisoner_join.class = MirClass::Taoist;
    poisoner_join.level = 50;
    purification_zone.handle(ZoneCommand::Join(poisoner_join));

    let mut friend_join = join("friend", 102, "Friend", 332, 270);
    friend_join.level = 50;
    purification_zone.handle(ZoneCommand::Join(friend_join));

    let mut taoist_join = join("taoist", 103, "Taoist", 334, 270);
    taoist_join.class = MirClass::Taoist;
    taoist_join.level = 50;
    purification_zone.handle(ZoneCommand::Join(taoist_join));

    let poisoned = purification_zone.handle(ZoneCommand::PlayerCastMagicWithItem {
        session_id: poisoner.clone(),
        object_id: 102,
        spell: Spell::Poisoning,
        direction: MirDirection::Right,
        target: Point { x: 332, y: 270 },
        cast: true,
        level: 3,
        damage: 10,
        mp_cost: 0,
        cooldown_ms: 500,
        item_param: 1,
        now_ms: 20,
    });
    assert!(has_packet(&poisoned, &friend, |packet| matches!(
        packet,
        ServerPacket::ObjectPoisoned { object_id: 102, poison } if *poison != 0
    )));

    purification_zone.handle(ZoneCommand::BroadcastPackets {
        session_id: friend.clone(),
        owner_local_object_id: 1_001,
        packets: vec![ServerPacket::AddBuff {
            buff: ClientBuff {
                buff_type: 12,
                visible: true,
                object_id: 1_001,
                expire_time: 10_000,
                infinite: false,
                paused: false,
                stats: Vec::new(),
                values: Vec::new(),
            },
        }],
        now_ms: 25,
    });

    let purified = purification_zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: taoist.clone(),
        object_id: 102,
        spell: Spell::Purification,
        direction: MirDirection::Left,
        target: Point { x: 332, y: 270 },
        cast: true,
        level: 3,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 500,
        now_ms: 40,
    });
    assert!(has_packet(&purified, &friend, |packet| matches!(
        packet,
        ServerPacket::ObjectPoisoned {
            object_id: 102,
            poison: 0
        }
    )));
    assert!(has_packet(&purified, &friend, |packet| matches!(
        packet,
        ServerPacket::RemoveBuff {
            object_id: 102,
            buff_type: 12
        }
    )));

    let observer = session("observer");
    let joined = purification_zone.handle(ZoneCommand::Join(join(
        "observer", 104, "Observer", 335, 270,
    )));
    assert!(has_packet(&joined, &observer, |packet| matches!(
        packet,
        ServerPacket::ObjectPlayer { info }
            if info.object_id == 102 && info.poison == 0 && !info.buffs.contains(&12)
    )));
}
