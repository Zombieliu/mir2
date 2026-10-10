//! Ordinary shared SummonSnakes and the immediate Monster Master lifetime.
use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneCommand, ZoneJoin, ZoneKey,
    ZoneMonsterDefense, ZoneMonsterSpawn, ZoneOutbound, ZoneRuntime,
};

const TARGET: u32 = 9100;
const MAP: &str = "0";

fn packets(out: &[ZoneOutbound], owner: &SessionId) -> Vec<ServerPacket> {
    out.iter()
        .flat_map(|out| match out {
            ZoneOutbound::ToSession {
                session_id,
                packets,
            } if session_id == owner => packets.clone(),
            ZoneOutbound::ToMany {
                session_ids,
                packets,
            } if session_ids.contains(owner) => packets.clone(),
            _ => Vec::new(),
        })
        .collect()
}

fn target() -> ZoneMonsterSpawn {
    ZoneMonsterSpawn {
        crystal_drop_seed: None,
        object_id: TARGET,
        name: "Field Wasp".into(),
        name_colour_argb: -1,
        image: 900,
        ai: 0,
        disposition: Some(WorldEntityDisposition::Hostile),
        level: 4,
        max_hp: 5000,
        hp: 5000,
        experience: 0,
        move_speed_ms: 600,
        attack_speed_ms: 1200,
        friendly_guild: None,
        position: Point { x: 328, y: 299 },
        direction: MirDirection::Left,
        defense: Default::default(),
        respawn: None,
        drops: Vec::new(),
    }
}

fn fixture() -> (ZoneRuntime, SessionId, u32, u32, u64) {
    fixture_with_ac(0)
}

fn fixture_with_ac(ac: i32) -> (ZoneRuntime, SessionId, u32, u32, u64) {
    let owner = SessionId::new("snake-owner");
    // Strict cold recovery reloads this actual map's collision. A synthetic
    // unbounded arena has no identical terrain identity to restore from disk.
    // Translate the original geometry by (+222,+199) into walkable terrain;
    // all actor offsets, distances, deadlines and lifecycle assertions remain.
    let mut zone = ZoneRuntime::new(ZoneKey::for_map(MAP));
    assert!(zone.has_available_collision(), "real fixture terrain must load");
    zone.handle(ZoneCommand::Join(ZoneJoin {
        session_id: owner.clone(),
        account_id: "snake-source-account".into(),
        character_index: 1,
        object_id: 101,
        name: "Archer".into(),
        class: MirClass::Archer,
        gender: MirGender::Male,
        level: 50,
        hp: 10000,
        max_hp: 10000,
        mp: 1000,
        map_file_name: MAP.into(),
        position: Point { x: 322, y: 299 },
        direction: MirDirection::Right,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }));
    let mut victim = target();
    victim.defense = ZoneMonsterDefense {
        min_ac: ac,
        max_ac: ac,
        agility: ac,
        ..Default::default()
    };
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: owner.clone(),
        monster: victim,
        now_ms: 0,
    });
    let accepted = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: owner.clone(),
        object_id: 0,
        spell: Spell::SummonSnakes,
        direction: MirDirection::Right,
        target: Point { x: 326, y: 299 },
        cast: true,
        level: 3,
        damage: 0,
        mp_cost: 7,
        cooldown_ms: 1000,
        now_ms: 10,
    });
    assert!(
        packets(&accepted, &owner).iter().any(|packet| matches!(
            packet,
            ServerPacket::Magic {
                spell: Spell::SummonSnakes,
                cast: true,
                ..
            }
        )),
        "real ground-target summon must pass unchanged source admission"
    );
    let spawned = packets(&zone.tick(1310), &owner);
    let parent = spawned
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectMonster { info }
                if info.name == "SnakeTotem" && info.master_object_id == 101 =>
            {
                Some(info.object_id)
            }
            _ => None,
        })
        .expect("accepted ordinary SummonSnakes creates its real Totem");
    for now in (1610..=7310).step_by(300) {
        let spawned = packets(&zone.tick(now), &owner);
        if let Some(child) = spawned.iter().find_map(|packet| match packet {
            ServerPacket::ObjectMonster { info }
                if info.name == "CharmedSnake" && info.master_object_id == parent =>
            {
                Some(info.object_id)
            }
            _ => None,
        }) {
            assert_ne!(child, parent);
            assert_ne!(parent, 101);
            return (zone, owner, parent, child, now);
        }
    }
    panic!("real Totem did not produce its child on the original search clock");
}

fn hp(zone: &ZoneRuntime, object: u32) -> i32 {
    zone.native_monster_snapshots()
        .into_iter()
        .find(|monster| monster.object_id == object)
        .expect("live fixture monster")
        .hp
}

fn first_child_attack(zone: &mut ZoneRuntime, owner: &SessionId, child: u32, birth: u64) -> u64 {
    for now in (birth + 2001..=birth + 9001).step_by(300) {
        let out = packets(&zone.tick(now), owner);
        if out.iter().any(|packet| {
            matches!(packet,
                ServerPacket::ObjectAttack { info } if info.object_id == child
            )
        }) {
            return now;
        }
    }
    panic!("source-born live child must launch before this lifecycle test");
}

#[test]
fn ordinary_snake_master_birth_delayed_mac_hit_and_paralysis() {
    let (mut zone, owner, parent, child, birth) = fixture_with_ac(1000);
    for now in [birth + 1999, birth + 2000] {
        let out = packets(&zone.tick(now), &owner);
        assert!(
            !out.iter().any(|packet| matches!(packet,
                ServerPacket::ObjectAttack { info } if info.object_id == child
            )),
            "child cannot attack through Spawned ActionTime: {now}"
        );
        assert!(
            !out.iter().any(|packet| matches!(packet,
                ServerPacket::ObjectWalk { movement } if movement.object_id == child
            )),
            "child cannot move through Spawned ActionTime: {now}"
        );
    }
    let mut launch = None;
    for now in (birth + 2001..=birth + 9001).step_by(300) {
        let out = packets(&zone.tick(now), &owner);
        if out.iter().any(|packet| {
            matches!(packet,
                ServerPacket::ObjectAttack { info } if info.object_id == child
            )
        }) {
            launch = Some(now);
            break;
        }
    }
    let launch =
        launch.expect("typed Totem child attacks the wild that targets its immediate Master");
    assert!(launch > birth + 2000);
    assert!(hp(&zone, parent) > 0);
    let before = hp(&zone, TARGET);
    let early = packets(&zone.tick(launch + 299), &owner);
    assert_eq!(hp(&zone, TARGET), before, "original 300ms delayed damage");
    assert!(!early.iter().any(|packet| matches!(packet,
        ServerPacket::ObjectStruck { info } if info.object_id == TARGET && info.attacker_id == child
    )));
    let impact = packets(&zone.tick(launch + 300), &owner);
    assert!(impact.iter().any(|packet| matches!(packet,
        ServerPacket::ObjectStruck { info } if info.object_id == TARGET && info.attacker_id == child
    )), "real child-attacker delayed impact");
    assert!(hp(&zone, TARGET) < before);
    let after = hp(&zone, TARGET);
    zone.tick(launch + 300);
    assert_eq!(hp(&zone, TARGET), after, "a due hit is consumed once");
    let mut poisoned = impact.iter().any(|packet| matches!(packet,
        ServerPacket::ObjectPoisoned { object_id, poison } if *object_id == TARGET && poison & 32 != 0
    ));
    for now in (launch + 600..=birth + 15000).step_by(300) {
        let out = packets(&zone.tick(now), &owner);
        poisoned |= out.iter().any(|packet| matches!(packet,
            ServerPacket::ObjectPoisoned { object_id, poison } if *object_id == TARGET && poison & 32 != 0
        ));
        if poisoned {
            break;
        }
    }
    assert!(
        poisoned,
        "actual positive child hit reaches the original paralysis producer"
    );
}

#[test]
fn ordinary_snake_pending_hit_is_revoked_on_logout_and_cold_restore() {
    for cold in [false, true] {
        let (mut zone, owner, _, child, birth) = fixture();
        let launch = first_child_attack(&mut zone, &owner, child, birth);
        let before = hp(&zone, TARGET);
        if cold {
            zone = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
        } else {
            zone.handle(ZoneCommand::Leave {
                session_id: owner.clone(),
            });
        }
        let impact = packets(&zone.tick(launch + 300), &owner);
        assert_eq!(hp(&zone, TARGET), before, "cold={cold}");
        assert!(!impact.iter().any(|packet| matches!(packet,
            ServerPacket::ObjectStruck { info } if info.object_id == TARGET && info.attacker_id == child
        )), "serialized/local owner ids never regrant a live chain, cold={cold}");
    }
}

#[test]
fn ordinary_snake_pending_hit_rejects_real_victim_id_reuse() {
    let (mut zone, owner, _, child, birth) = fixture();
    let launch = first_child_attack(&mut zone, &owner, child, birth);
    zone.despawn_world_event_monster(TARGET, launch + 100);
    let spawned = zone.handle(ZoneCommand::SpawnMonster {
        session_id: owner.clone(),
        monster: target(),
        now_ms: launch + 100,
    });
    assert!(
        packets(&spawned, &owner)
            .iter()
            .any(|packet| matches!(packet,
                ServerPacket::ObjectMonster { info } if info.object_id == TARGET
            )),
        "new incarnation really enters the same public shared map"
    );
    let impact = packets(&zone.tick(launch + 300), &owner);
    assert_eq!(hp(&zone, TARGET), 5000);
    assert!(!impact.iter().any(|packet| matches!(packet,
        ServerPacket::ObjectStruck { info } if info.object_id == TARGET && info.attacker_id == child
    )), "old pending hit cannot attach to the reused victim id");
}

#[test]
fn ordinary_snake_parent_projection_cannot_forge_native_death_and_real_removal_revokes() {
    for removed in [false, true] {
        let (mut zone, owner, parent, child, birth) = fixture();
        let launch = first_child_attack(&mut zone, &owner, child, birth);
        let before = hp(&zone, TARGET);
        if removed {
            zone.despawn_world_event_monster(parent, launch + 100);
        } else {
            let parent_position = zone
                .native_monster_snapshots()
                .into_iter()
                .find(|m| m.object_id == parent)
                .unwrap()
                .position;
            let projected = zone.handle(ZoneCommand::BroadcastSharedObjectPackets {
                session_id: owner.clone(),
                local_self_object_id: None,
                packets: vec![ServerPacket::ObjectDied {
                    info: mir2_protocol::ObjectDiedInfo {
                        object_id: parent,
                        location: parent_position,
                        direction: MirDirection::Up,
                        kind: 0,
                    },
                }],
                now_ms: launch + 100,
            });
            assert!(
                projected.is_empty(),
                "personal projection cannot replace shared native life"
            );
            assert!(hp(&zone, parent) > 0, "the actual parent remains alive");
        }
        let impact = packets(&zone.tick(launch + 300), &owner);
        assert_eq!(
            hp(&zone, TARGET) < before,
            !removed,
            "actual parent lifecycle removed={removed}"
        );
        assert_eq!(impact.iter().any(|packet| matches!(packet,
            ServerPacket::ObjectStruck { info } if info.object_id == TARGET && info.attacker_id == child
        )), !removed, "only genuine removal retires the shared life, removed={removed}");
    }
}

#[test]
fn ordinary_snake_pending_hit_rejects_actual_parent_hp_zero() {
    let (mut zone, owner, parent, child, birth) = fixture();
    let launch = first_child_attack(&mut zone, &owner, child, birth);
    let before = hp(&zone, TARGET);
    let caster = SessionId::new("parent-killer");
    zone.handle(ZoneCommand::Join(ZoneJoin {
        session_id: caster.clone(),
        account_id: "snake-killer-account".into(),
        character_index: 1,
        object_id: 90001,
        name: "Wizard".into(),
        class: MirClass::Wizard,
        gender: MirGender::Male,
        level: 50,
        hp: 10000,
        max_hp: 10000,
        mp: 1000,
        map_file_name: MAP.into(),
        position: Point { x: 322, y: 300 },
        direction: MirDirection::Right,
        chat_profile: mir2_simulation::ZoneChatProfile {
            attack_mode: 5,
            ..Default::default()
        },
        combat_stats: Default::default(),
    }));
    zone.handle(ZoneCommand::sync_player_combat_state(
        caster.clone(),
        MirClass::Wizard,
        true,
        false,
        false,
        false,
        false,
        false,
    ));
    let parent_position = zone
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.object_id == parent)
        .unwrap()
        .position;
    assert!(
        zone.player_can_attack_owned_pet(&caster, parent, launch + 100),
        "actual All-mode attacker and parent must have current lives: parent={:?}, profile={:?}",
        zone.native_monster_snapshots()
            .into_iter()
            .find(|m| m.object_id == parent),
        zone.player_chat_profile(&caster)
    );
    let accepted = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: caster.clone(),
        object_id: parent,
        spell: Spell::ThunderBolt,
        direction: MirDirection::Right,
        target: parent_position,
        cast: true,
        level: 2,
        damage: 5000,
        mp_cost: 7,
        cooldown_ms: 1000,
        now_ms: launch + 100,
    });
    assert!(
        packets(&accepted, &caster).iter().any(|packet| matches!(
            packet,
            ServerPacket::Magic {
                spell: Spell::ThunderBolt,
                cast: true,
                ..
            }
        )),
        "normal typed All-mode Wizard cast must really be accepted: {accepted:?}"
    );
    let died = packets(&zone.tick(launch + 100), &owner);
    let parent_state = zone
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.object_id == parent)
        .unwrap();
    assert!(
        parent_state.dead && parent_state.hp == 0,
        "actual native HP0 death precedes pending child impact"
    );
    assert!(
        died.iter().any(|packet| matches!(packet,
            ServerPacket::ObjectDied { info } if info.object_id == parent
        )),
        "real native death also reaches its observers"
    );
    assert_eq!(
        hp(&zone, TARGET),
        before,
        "Master-null child death cannot grant an ordinary wild target"
    );
    let impact = packets(&zone.tick(launch + 300), &owner);
    assert_eq!(hp(&zone, TARGET), before);
    assert!(!impact.iter().any(|packet| matches!(packet,
        ServerPacket::ObjectStruck { info } if info.object_id == TARGET && info.attacker_id == child
    )));
}
