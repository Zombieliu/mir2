//! AI108/170 public shared Zone contracts; checkpoints are never modified.
//! The imported MudZombie/BoulderSpirit templates have zero attack stats.
//! Guard supplies fixed DC/MC=255 solely for nonzero mechanism fixtures;
//! the final test explicitly retains the real names' zero-damage behavior.
use mir2_protocol::{
    MirClass, MirDirection, MirGender, ObjectDiedInfo, ObjectRevivedInfo, Point, ServerPacket,
};
use mir2_simulation::{
    GroundDropLootSnapshot, GroundDropSnapshot, SessionId, WorldEntityDisposition, ZoneCommand,
    ZoneJoin, ZoneKey, ZoneMonsterSpawn, ZoneOutbound, ZonePlayerCombatStats, ZoneRuntime,
};
use serde_json::Value;

const MAP: &str = "shared-mud-boulder-fixture";
const ID: u32 = 91_108;
fn point(x: i32, y: i32) -> Point {
    Point { x, y }
}
fn player(name: &str, id: u32, x: i32, y: i32) -> ZoneJoin {
    ZoneJoin {
        session_id: SessionId::new(name),
        account_id: format!("{name}-account"),
        character_index: id as i32,
        object_id: id,
        name: name.into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 60,
        hp: 100_000,
        max_hp: 100_000,
        mp: 100,
        map_file_name: MAP.into(),
        position: point(x, y),
        direction: MirDirection::Up,
        chat_profile: Default::default(),
        combat_stats: ZonePlayerCombatStats {
            min_dc: 100_000,
            max_dc: 100_000,
            accuracy: 255,
            poison_resist: 10,
            ..Default::default()
        },
    }
}
fn spawn(ai: u8) -> ZoneMonsterSpawn {
    ZoneMonsterSpawn {
        object_id: ID,
        name: "Guard".into(),
        name_colour_argb: -1,
        image: 5,
        ai,
        disposition: Some(WorldEntityDisposition::Hostile),
        level: 60,
        hp: 100,
        max_hp: 100,
        experience: 100,
        move_speed_ms: 500,
        attack_speed_ms: 5_000,
        friendly_guild: None,
        position: point(20, 20),
        direction: MirDirection::Down,
        defense: Default::default(),
        respawn: None,
        drops: vec![GroundDropSnapshot {
            object_id: 0,
            name: "Gold".into(),
            name_colour_argb: -1,
            icon: 0,
            x: 0,
            y: 0,
            quantity: 1,
            source_monster: "Guard".into(),
            owner_object_id: None,
            ownership_remaining_ticks: None,
            loot: GroundDropLootSnapshot::Gold { amount: 7 },
        }],
    }
}
fn fixture(ai: u8, y: i32) -> ZoneRuntime {
    let mut zone = ZoneRuntime::new(ZoneKey::for_map(MAP));
    zone.handle(ZoneCommand::Join(player("first", 101, 20, y)));
    zone.handle(ZoneCommand::Join(player("observer", 102, 28, 20)));
    zone.handle(ZoneCommand::sync_player_combat_state(
        SessionId::new("first"),
        MirClass::Warrior,
        false,
        false,
        true,
        false,
        false,
        false,
    ));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("first"),
        monster: spawn(ai),
        now_ms: 0,
    });
    zone
}

#[test]
fn mud_zombie_snake_totem_forces_actual_attack_away_from_retained_player() {
    use mir2_protocol::Spell;
    let mut zone = ZoneRuntime::new(ZoneKey::for_map(MAP));
    let mut owner = player("first", 101, 20, 24);
    owner.class = MirClass::Archer;
    zone.handle(ZoneCommand::Join(owner));
    let mut mud = spawn(108);
    // Keep the real zero-damage template: this verifies its actual attack
    // target and allows both potential victims to survive the full sequence.
    mud.name = "MudZombie".into();
    mud.hp = 100_000;
    mud.max_hp = 100_000;
    mud.attack_speed_ms = 500;
    zone.spawn_world_event_monster(&mud, 0);
    let initial = zone.tick(2001);
    assert!(packets(&initial, "first").iter().any(|p| matches!(
        p, ServerPacket::ObjectRangeAttack { info }
            if info.object_id == ID && info.target_id == 101
    )));
    assert_eq!(state(&zone)["target"][1], 101);

    zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("first"),
        object_id: 0,
        spell: Spell::SummonSnakes,
        direction: MirDirection::Up,
        target: point(24, 20),
        cast: true,
        level: 0,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1000,
        now_ms: 2100,
    });
    let mut totem_id = None;
    let mut attacked_totem = false;
    for now in (2200..=12_000).step_by(100) {
        let out = zone.tick(now);
        for packet in packets(&out, "first") {
            match packet {
                ServerPacket::ObjectMonster { info }
                    if info.name == "SnakeTotem" && info.master_object_id == 101 =>
                {
                    totem_id = Some(info.object_id);
                }
                ServerPacket::ObjectRangeAttack { info }
                    if info.object_id == ID && Some(info.target_id) == totem_id =>
                {
                    attacked_totem = true;
                }
                _ => {}
            }
        }
        if attacked_totem {
            break;
        }
    }
    let totem_id = totem_id.expect("real SummonSnakes must create the forcing totem");
    assert!(
        attacked_totem,
        "the old local player target must not overwrite forced aggro"
    );
    assert_eq!(state(&zone)["monster_target"][0], totem_id);
    assert!(state(&zone)["target"].is_null());
    assert!(monster(&zone)["hp"].as_i64().unwrap() > 0);
}

fn packets<'a>(out: &'a [ZoneOutbound], who: &str) -> Vec<&'a ServerPacket> {
    let id = SessionId::new(who);
    out.iter()
        .flat_map(|o| match o {
            ZoneOutbound::ToMany {
                session_ids,
                packets,
            } if session_ids.contains(&id) => packets.iter(),
            ZoneOutbound::ToSession {
                session_id,
                packets,
            } if session_id == &id => packets.iter(),
            ZoneOutbound::ToAll { packets } => packets.iter(),
            _ => [].iter(),
        })
        .collect()
}
fn damage(out: &[ZoneOutbound], who: &str) -> Vec<i32> {
    out.iter()
        .filter_map(|o| match o {
            ZoneOutbound::PlayerDamaged {
                session_id, damage, ..
            } if session_id.as_str() == who => Some(*damage),
            _ => None,
        })
        .collect()
}
fn monster(zone: &ZoneRuntime) -> Value {
    let json: Value = serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
    json["native_monsters"][ID.to_string()].clone()
}
fn state(zone: &ZoneRuntime) -> Value {
    monster(zone)["special_ai"]["mud_boulder"].clone()
}
fn died(out: &[ZoneOutbound], who: &str) -> usize {
    packets(out, who)
        .iter()
        .filter(|p| matches!(p,ServerPacket::ObjectDied{info} if info.object_id==ID))
        .count()
}
fn attack(zone: &mut ZoneRuntime, now: u64) -> Vec<ZoneOutbound> {
    let mut out = zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("first"),
        object_id: ID,
        direction: MirDirection::Up,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 1,
        now_ms: now,
    });
    out.extend(zone.tick(now));
    out
}
fn teleport(zone: &mut ZoneRuntime, name: &str, x: i32, y: i32) {
    zone.handle(ZoneCommand::SyncPlayerTransform {
        session_id: SessionId::new(name),
        position: point(x, y),
        direction: MirDirection::Up,
    });
}

#[test]
fn mud_line_hits_two_tiles_at_550_and_600ms_and_is_shared_once() {
    let mut z = fixture(108, 21);
    z.handle(ZoneCommand::Join(player("second", 103, 20, 22)));
    z.handle(ZoneCommand::Join(player("off-axis", 104, 21, 21)));
    assert!(!z
        .tick(2_000)
        .iter()
        .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
    let launch = z.tick(2_001);
    for who in ["first", "observer", "second"] {
        assert_eq!(
            packets(&launch, who)
                .iter()
                .filter(|p| matches!(p,ServerPacket::ObjectAttack{info} if info.object_id==ID))
                .count(),
            1
        );
    }
    assert_eq!(state(&z)["hits"][0]["hit"]["ready_at_ms"], 2_551);
    assert_eq!(state(&z)["hits"][1]["hit"]["ready_at_ms"], 2_601);
    assert!(damage(&launch, "first").is_empty());
    assert!(damage(&z.tick(2_550), "first").is_empty());
    let first = z.tick(2_551);
    assert_eq!(damage(&first, "first"), vec![255]);
    assert!(damage(&first, "second").is_empty());
    let second = z.tick(2_601);
    assert_eq!(damage(&second, "second"), vec![255]);
    assert!(damage(&second, "off-axis").is_empty());
    assert!(damage(&z.tick(2_601), "first").is_empty());
    assert!(state(&z)["hits"].as_array().unwrap().is_empty());
}

#[test]
fn mud_ranged_uses_mac_delay_extra_cooldown_and_keeps_chasing_during_cooldown() {
    let mut z = fixture(108, 24);
    z.handle(ZoneCommand::UpdatePlayerCombatStats {
        session_id: SessionId::new("first"),
        stats: ZonePlayerCombatStats {
            min_ac: 100_000,
            max_ac: 100_000,
            ..Default::default()
        },
    });
    let launch = z.tick(2_001);
    for who in ["first", "observer"] {
        assert_eq!(packets(&launch,who).iter().filter(|p|matches!(p,ServerPacket::ObjectRangeAttack{info} if info.object_id==ID && info.target_id==101 && info.attack_type==0 && info.spell==0)).count(),1);
    }
    assert_eq!(monster(&z)["next_attack_ready_at_ms"], 7_501);
    assert_eq!(state(&z)["hits"][0]["hit"]["ready_at_ms"], 2_501);
    let chasing = z.tick(2_302);
    assert_eq!(z.native_monster_snapshots()[0].position, point(20, 21));
    assert!(packets(&chasing, "observer")
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectWalk{movement} if movement.object_id==ID)));
    teleport(&mut z, "first", 25, 24); // A launched target hit is not an old tile hit.
    assert!(damage(&z.tick(2_500), "first").is_empty());
    assert_eq!(damage(&z.tick(2_501), "first"), vec![255]);
    assert_eq!(monster(&z)["next_attack_ready_at_ms"], 7_501);
}

#[test]
fn mud_mac_can_absorb_the_ranged_hit_and_absorbed_hits_do_not_apply_green() {
    let mut z = fixture(108, 24);
    z.handle(ZoneCommand::UpdatePlayerCombatStats {
        session_id: SessionId::new("first"),
        stats: ZonePlayerCombatStats {
            min_mac: 100_000,
            max_mac: 100_000,
            ..Default::default()
        },
    });
    z.tick(2_001);
    let out = z.tick(2_501);
    assert!(damage(&out, "first").is_empty());
    assert!(!packets(&out, "first")
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectPoisoned{object_id:101,poison} if poison & 1 !=0)));
}

#[test]
fn mud_checkpoint_and_dead_attacker_keep_launched_actions_without_new_incarnation_overwrite() {
    let mut z = fixture(108, 21);
    z.tick(2_001);
    let death = attack(&mut z, 2_100);
    assert_eq!(died(&death, "first"), 1);
    assert_eq!(monster(&z)["dead"], true);
    let before = state(&z);
    z.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("observer"),
        monster: spawn(108),
        now_ms: 2_101,
    });
    assert_eq!(state(&z), before);
    let bytes = z.checkpoint_bytes().unwrap();
    let mut restored = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
    assert_eq!(restored.checkpoint_bytes().unwrap(), bytes);
    let original = z.tick(2_551);
    assert_eq!(damage(&original, "first"), vec![255]);
    assert_eq!(restored.tick(2_551), original);
    assert_eq!(
        restored.canonical_state_root().unwrap(),
        z.canonical_state_root().unwrap()
    );
    assert!(damage(&restored.tick(2_551), "first").is_empty());
}

#[test]
fn mud_pending_hit_cannot_cross_leave_rejoin_vital_revive_or_packet_revive() {
    for mode in 0..3 {
        let mut z = fixture(108, 21);
        z.tick(2_001);
        let first = SessionId::new("first");
        match mode {
            0 => {
                z.handle(ZoneCommand::Leave {
                    session_id: first.clone(),
                });
                z.handle(ZoneCommand::Join(player("first", 101, 20, 21)));
            }
            1 => {
                for hp in [0, 100_000] {
                    z.handle(ZoneCommand::SyncPlayerVitals {
                        session_id: first.clone(),
                        hp,
                        max_hp: 100_000,
                        mp: 100,
                    });
                }
            }
            _ => {
                z.handle(ZoneCommand::BroadcastPackets {
                    session_id: first.clone(),
                    owner_local_object_id: 101,
                    packets: vec![
                        ServerPacket::ObjectDied {
                            info: ObjectDiedInfo {
                                object_id: 101,
                                location: point(20, 21),
                                direction: MirDirection::Up,
                                kind: 0,
                            },
                        },
                        ServerPacket::ObjectRevived {
                            info: ObjectRevivedInfo {
                                object_id: 101,
                                effect: true,
                            },
                        },
                    ],
                    now_ms: 2_100,
                });
            }
        }
        assert!(
            state(&z)["hits"].as_array().unwrap().is_empty(),
            "mode {mode} must retire old life actions"
        );
        let mut z = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
        assert!(damage(&z.tick(2_551), "first").is_empty(), "mode {mode}");
    }
}

#[test]
fn boulder_dies_immediately_then_explodes_300ms_later_once_for_all_observers() {
    let mut z = fixture(170, 21);
    teleport(&mut z, "observer", 21, 20);
    let death = z.tick(1);
    assert_eq!(died(&death, "first"), 1);
    assert_eq!(died(&death, "observer"), 1);
    assert_eq!(monster(&z)["dead"], true);
    assert_eq!(state(&z)["explosion_at_ms"], 301);
    assert!(damage(&death, "first").is_empty());
    assert!(!death
        .iter()
        .any(|o| matches!(o, ZoneOutbound::MonsterKillAward { .. })));
    assert!(damage(&z.tick(300), "first").is_empty());
    let explosion = z.tick(301);
    assert_eq!(damage(&explosion, "first"), vec![255]);
    assert_eq!(damage(&explosion, "observer"), vec![255]);
    assert_eq!(died(&explosion, "first"), 0);
    assert_eq!(state(&z)["exploded"], true);
    assert!(!explosion
        .iter()
        .any(|o| matches!(o, ZoneOutbound::MonsterKillAward { .. })));
    assert!(damage(&z.tick(301), "first").is_empty());
    assert!(state(&z)["explosion_at_ms"].is_null());
}

#[test]
fn boulder_explosion_uses_impact_positions_and_ignores_hiding_after_activation() {
    let mut z = fixture(170, 21);
    z.tick(1);
    teleport(&mut z, "first", 20, 28); // Out of ViewRange=7 at impact.
    teleport(&mut z, "observer", 20, 22);
    z.handle(ZoneCommand::BroadcastPackets {
        session_id: SessionId::new("observer"),
        owner_local_object_id: 102,
        packets: vec![ServerPacket::ObjectHidden {
            object_id: 102,
            hidden: true,
        }],
        now_ms: 2,
    });
    let explosion = z.tick(301);
    assert!(damage(&explosion, "first").is_empty());
    assert_eq!(damage(&explosion, "observer"), vec![255]);
}

#[test]
fn boulder_pending_checkpoint_cannot_be_cancelled_by_personal_refresh_or_repeated_ticks() {
    let mut z = fixture(170, 21);
    z.tick(1);
    let before = state(&z);
    z.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("observer"),
        monster: spawn(170),
        now_ms: 2,
    });
    assert_eq!(state(&z), before);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    for now in [300, 301, 301, 302] {
        assert_eq!(z.tick(now), restored.tick(now));
        assert_eq!(
            z.canonical_state_root().unwrap(),
            restored.canonical_state_root().unwrap()
        );
    }
    let mut again = ZoneRuntime::restore_checkpoint(&restored.checkpoint_bytes().unwrap()).unwrap();
    assert!(damage(&again.tick(303), "first").is_empty());
}

#[test]
fn boulder_direct_kill_awards_once_per_incarnation_and_allocates_fresh_drop_ids() {
    let mut z = fixture(170, 21);
    let first = attack(&mut z, 1);
    let awarded = |out: &[ZoneOutbound]| {
        out.iter()
            .filter_map(|o| match o {
                ZoneOutbound::MonsterKillAward { award, .. } => {
                    Some(award.drops.iter().map(|d| d.object_id).collect::<Vec<_>>())
                }
                _ => None,
            })
            .flatten()
            .collect::<Vec<_>>()
    };
    let first_ids = awarded(&first);
    assert_eq!(first_ids.len(), 1);
    assert!(first_ids[0] > 0);
    assert_eq!(died(&first, "observer"), 1);
    assert_eq!(state(&z)["explosion_at_ms"], 301);
    let impact = z.tick(301);
    assert_eq!(damage(&impact, "first"), vec![255]);
    assert!(awarded(&impact).is_empty());
    z.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("first"),
        monster: spawn(170),
        now_ms: 1_000,
    });
    let second = attack(&mut z, 1_001);
    let second_ids = awarded(&second);
    assert_eq!(second_ids.len(), 1);
    assert_ne!(first_ids, second_ids);
    assert_eq!(state(&z)["explosion_at_ms"], 1_301);
    assert_eq!(damage(&z.tick(1_301), "first"), vec![255]);
}

#[test]
fn boulder_without_visible_targets_stands_and_real_zero_stats_are_not_fabricated() {
    let mut z = fixture(170, 28);
    for now in [1, 2_001, 20_001] {
        let out = z.tick(now);
        assert_eq!(z.native_monster_snapshots()[0].position, point(20, 20));
        assert_eq!(monster(&z)["dead"], false);
        assert!(!packets(&out, "observer")
            .iter()
            .any(|p| matches!(p,ServerPacket::ObjectWalk{movement} if movement.object_id==ID)));
    }
    for (ai, name) in [(108, "MudZombie"), (170, "BoulderSpirit")] {
        let mut z = ZoneRuntime::new(ZoneKey::for_map(MAP));
        z.handle(ZoneCommand::Join(player("first", 101, 20, 21)));
        let mut m = spawn(ai);
        m.name = name.into();
        z.handle(ZoneCommand::SpawnMonster {
            session_id: SessionId::new("first"),
            monster: m,
            now_ms: 0,
        });
        z.tick(1);
        z.tick(2_001);
        assert!(damage(&z.tick(2_601), "first").is_empty());
        assert!(state(&z)["hits"].as_array().unwrap().is_empty());
    }
}

// Public spell production supplies a real owner/master; no checkpoint mutation.
fn owned_toad_fixture(ai: u8) -> (ZoneRuntime, u32, Point) {
    let mut z = ZoneRuntime::new(ZoneKey::for_map(MAP));
    let mut owner = player("first", 101, 20, 24);
    owner.class = MirClass::Archer;
    z.handle(ZoneCommand::Join(owner));
    z.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("first"),
        object_id: 0,
        spell: mir2_protocol::Spell::SummonToad,
        direction: MirDirection::Up,
        target: point(20, 20),
        cast: true,
        level: 3,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1000,
        now_ms: 10,
    });
    let out = z.tick(1500);
    let (pet, pos) = packets(&out, "first")
        .into_iter()
        .find_map(|p| match p {
            ServerPacket::ObjectMonster { info } if info.name == "SpittingToad" => {
                Some((info.object_id, info.location.clone()))
            }
            _ => None,
        })
        .expect("actual owned toad spell spawn");
    teleport(&mut z, "first", pos.x + 10, pos.y);
    let mut source = spawn(ai);
    source.hp = 100000;
    source.max_hp = 100000;
    source.position = point(pos.x, pos.y - 1);
    assert!(z.spawn_world_event_monster(&source, 1500).0);
    (z, pet, pos)
}
fn native_hp(z: &ZoneRuntime, id: u32) -> i64 {
    let value: Value = serde_json::from_slice(&z.checkpoint_bytes().unwrap()).unwrap();
    value["native_monsters"][id.to_string()]["hp"]
        .as_i64()
        .unwrap()
}
#[test]
fn mud_line_reaches_real_owned_monster_at_550ms_and_restores_pending() {
    let (mut z, pet, _) = owned_toad_fixture(108);
    let before = native_hp(&z, pet);
    let attack = z.tick(3501);
    assert!(packets(&attack, "first")
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectAttack{info} if info.object_id==ID)));
    assert_eq!(native_hp(&z, pet), before);
    assert_eq!(state(&z)["monster_hits"][0]["target"], pet);
    assert_eq!(state(&z)["monster_hits"][0]["due"], 4051);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(4050), restored.tick(4050));
    assert_eq!(native_hp(&z, pet), before);
    let out = z.tick(4051);
    assert_eq!(out, restored.tick(4051));
    assert!(native_hp(&z, pet) < before);
    assert!(damage(&out, "first").is_empty());
}
#[test]
fn mud_pending_native_target_is_invalidated_by_real_retirement() {
    let (mut z, pet, _) = owned_toad_fixture(108);
    z.tick(3501);
    z.despawn_world_event_monster(pet, 3600);
    let out = z.tick(4051);
    assert!(!packets(&out, "first")
        .iter()
        .any(|p| matches!(p,ServerPacket::DamageIndicator{object_id,..} if *object_id==pet)));
}
#[test]
fn boulder_owned_monster_triggers_delayed_blast_without_visible_player() {
    let (mut z, pet, _) = owned_toad_fixture(170);
    let before = native_hp(&z, pet);
    let trigger = z.tick(1501);
    assert_eq!(died(&trigger, "first"), 1);
    assert_eq!(native_hp(&z, pet), before);
    assert_eq!(state(&z)["explosion_at_ms"], 1801);
    z.tick(1800);
    assert_eq!(native_hp(&z, pet), before);
    let out = z.tick(1801);
    assert!(native_hp(&z, pet) < before);
    assert!(damage(&out, "first").is_empty());
}
#[test]
fn boulder_does_not_treat_unowned_wild_monster_as_an_opponent() {
    let mut z = ZoneRuntime::new(ZoneKey::for_map(MAP));
    let mut boulder = spawn(170);
    boulder.drops.clear();
    assert!(z.spawn_world_event_monster(&boulder, 0).0);
    let mut wild = spawn(108);
    wild.object_id = 9900;
    wild.position = point(21, 20);
    wild.drops.clear();
    assert!(z.spawn_world_event_monster(&wild, 0).0);
    z.tick(1);
    assert_eq!(monster(&z)["dead"], false);
    assert!(state(&z)["explosion_at_ms"].is_null());
}

#[test]
fn mud_line_combines_first_native_tile_and_second_player_tile() {
    let (mut z, pet, pos) = owned_toad_fixture(108);
    teleport(&mut z, "first", pos.x, pos.y + 1);
    z.tick(3501);
    assert_eq!(state(&z)["monster_hits"][0]["target"], pet);
    assert_eq!(state(&z)["hits"][0]["hit"]["target_object_id"], 101);
    let first = z.tick(4051);
    assert!(damage(&first, "first").is_empty());
    let second = z.tick(4101);
    assert_eq!(damage(&second, "first"), vec![255]);
}
