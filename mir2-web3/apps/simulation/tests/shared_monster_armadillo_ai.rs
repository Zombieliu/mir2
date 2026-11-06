//! Public shared-player Armadillo and Elder branch contracts.
//! Reference: Crystal Armadillo.cs and ArmadilloElder.cs.
//! These tests inspect genuine checkpoint output; they never rewrite a root.

use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneCommand, ZoneJoin, ZoneKey, ZoneMonsterSpawn,
    ZoneOutbound, ZonePlayerCombatStats, ZoneRuntime,
};
use serde_json::Value;

const MONSTER_ID: u32 = 9_111;
const SOURCE_MAP: &str = "shared-monster-visibility-ai-fixture";

fn point(x: i32, y: i32) -> Point {
    Point { x, y }
}

fn player(session: &str, object_id: u32, position: Point) -> ZoneJoin {
    ZoneJoin {
        session_id: SessionId::new(session),
        account_id: format!("{session}-account"),
        character_index: object_id as i32,
        object_id,
        name: session.to_string(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 40,
        hp: 100_000,
        max_hp: 100_000,
        mp: 100,
        map_file_name: SOURCE_MAP.to_string(),
        position,
        direction: MirDirection::Down,
        chat_profile: Default::default(),
        combat_stats: ZonePlayerCombatStats {
            min_dc: 10,
            max_dc: 10,
            accuracy: 100,
            min_ac: 100_000,
            max_ac: 100_000,
            ..Default::default()
        },
    }
}

fn monster(ai: u8, position: Point) -> ZoneMonsterSpawn {
    ZoneMonsterSpawn {
        crystal_drop_seed: None,
        object_id: MONSTER_ID,
        name: if ai == 11 { "WoomaTaurus" } else { "Scarecrow" }.to_string(),
        name_colour_argb: -1,
        image: if ai == 11 { 11 } else { 5 },
        ai,
        disposition: Some(WorldEntityDisposition::Hostile),
        level: 30,
        max_hp: 70,
        hp: 70,
        experience: 0,
        move_speed_ms: 3_000,
        attack_speed_ms: 5_000,
        friendly_guild: None,
        position,
        direction: MirDirection::Down,
        defense: Default::default(),
        respawn: None,
        drops: Vec::new(),
    }
}

fn packets_for<'a>(outbounds: &'a [ZoneOutbound], recipient: &str) -> Vec<&'a ServerPacket> {
    let recipient = SessionId::new(recipient);
    let mut packets = Vec::new();
    for outbound in outbounds {
        match outbound {
            ZoneOutbound::ToSession {
                session_id,
                packets: batch,
            } if session_id == &recipient => {
                packets.extend(batch);
            }
            ZoneOutbound::ToMany {
                session_ids,
                packets: batch,
            } if session_ids.contains(&recipient) => {
                packets.extend(batch);
            }
            ZoneOutbound::ToAll { packets: batch } => packets.extend(batch),
            _ => {}
        }
    }
    packets
}

fn checkpoint_monster(zone: &ZoneRuntime) -> Value {
    let bytes = zone.checkpoint_bytes().expect("real Zone checkpoint");
    let checkpoint: Value = serde_json::from_slice(&bytes).expect("checkpoint JSON");
    checkpoint["native_monsters"][MONSTER_ID.to_string()].clone()
}

fn attack_fixture(ai: u8, now: u64) -> (ZoneRuntime, Vec<ZoneOutbound>) {
    attack_fixture_with_blocked_push(ai, now, false)
}

fn attack_fixture_with_blocked_push(
    ai: u8,
    now: u64,
    blocked: bool,
) -> (ZoneRuntime, Vec<ZoneOutbound>) {
    let mut zone = ZoneRuntime::new(ZoneKey::for_map(SOURCE_MAP));
    zone.handle(ZoneCommand::Join(player("target", 101, point(20, 21))));
    zone.handle(ZoneCommand::Join(player(
        "old-edge",
        102,
        point(20, 21 - mir2_simulation::CRYSTAL_OBJECT_DATA_RANGE),
    )));
    zone.handle(ZoneCommand::Join(player(
        "new-edge",
        103,
        point(20, 23 + mir2_simulation::CRYSTAL_OBJECT_DATA_RANGE),
    )));
    if blocked {
        zone.handle(ZoneCommand::Join(player(
            "push-blocker",
            104,
            point(20, 22),
        )));
    }
    let mut m = monster(ai, point(20, 20));
    m.name = if ai == 124 {
        "Armadillo"
    } else {
        "ArmadilloElder"
    }
    .to_string();
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("target"),
        monster: m,
        now_ms: 0,
    });
    zone.tick(1); // Reveal; preserve the original 2000ms action lock.
    let attack = zone.tick(now);
    (zone, attack)
}

#[test]
fn armadillo_retreat_does_not_hit_rejoined_or_revived_target_life() {
    for life in ["unchanged", "rejoined", "revived"] {
        let (mut zone, due) = (3000..3096)
            .find_map(|now| {
                let (zone, _) = attack_fixture(124, now);
                checkpoint_monster(&zone)["special_ai"]["armadillo"]["retreat"]["due_ms"]
                    .as_u64()
                    .map(|due| (zone, due))
            })
            .expect("retreat branch");
        if life == "rejoined" {
            zone.handle(ZoneCommand::Leave {
                session_id: SessionId::new("target"),
            });
            zone.handle(ZoneCommand::Join(player("target", 101, point(20, 20))));
        } else {
            if life == "revived" {
                zone.handle(ZoneCommand::SyncPlayerVitals {
                    session_id: SessionId::new("target"),
                    hp: 0,
                    max_hp: 100000,
                    mp: 100,
                });
                zone.handle(ZoneCommand::SyncPlayerVitals {
                    session_id: SessionId::new("target"),
                    hp: 100000,
                    max_hp: 100000,
                    mp: 100,
                });
            }
            zone.handle(ZoneCommand::SyncPlayerTransform {
                session_id: SessionId::new("target"),
                position: point(20, 20),
                direction: MirDirection::Down,
            });
        }
        zone.handle(ZoneCommand::UpdatePlayerCombatStats {
            session_id: SessionId::new("target"),
            stats: ZonePlayerCombatStats::default(),
        });
        if life != "unchanged" {
            assert!(
                checkpoint_monster(&zone)["special_ai"]["armadillo"]["retreat"].is_null(),
                "retire pending life before restoring checkpoint"
            );
        }
        let mut restored =
            ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
        let out = restored.tick(due);
        let damaged=out.iter().any(|o|matches!(o,ZoneOutbound::PlayerDamaged { session_id, damage, .. } if session_id.as_str()=="target"&&*damage>0));
        assert_eq!(
            damaged,
            life == "unchanged",
            "ordinary victim is hittable; replacement life is not"
        );
        if life != "unchanged" {
            assert_eq!(
                restored.player_vitals(&SessionId::new("target")).unwrap().0,
                100000
            );
        }
    }
}

#[test]
fn armadillo_all_six_roll_outcomes_preserve_original_delays_and_elder_push_only() {
    for ai in [124, 125] {
        let mut seen = [false; 3];
        for offset in 0..96 {
            let now = 3_000 + offset;
            let (zone, out) = attack_fixture(ai, now);
            let packets = packets_for(&out, "target");
            let backstep = packets.iter().any(|p| matches!(p, ServerPacket::ObjectBackStep { movement, distance: 2 } if movement.object_id == MONSTER_ID));
            let typed = packets.iter().find_map(|p| match p {
                ServerPacket::ObjectAttack { info } if info.object_id == MONSTER_ID => {
                    Some(info.attack_type)
                }
                _ => None,
            });
            let data: Value = serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
            let hits = data["native_monsters"][MONSTER_ID.to_string()]["special_ai"]["armadillo"]
                ["entity_hits"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            if backstep {
                seen[0] = true;
                assert!(hits.is_empty());
                assert_eq!(zone.native_monster_snapshots()[0].position, point(20, 18));
                let state = checkpoint_monster(&zone)["special_ai"]["armadillo"].clone();
                assert_eq!(state["fleeing"], ai == 125);
                if ai == 124 {
                    assert_eq!(state["retreat"]["due_ms"], now + 900);
                } else {
                    assert!(state["retreat"].is_null());
                }
            } else if typed == Some(1) {
                seen[1] = true;
                if ai == 124 {
                    let times: Vec<_> =
                        hits.iter().map(|h| h["due_ms"].as_u64().unwrap()).collect();
                    assert_eq!(times, vec![now + 400, now + 600, now + 800]);
                } else {
                    assert!(hits.is_empty(), "Elder type1 must only push, not damage");
                    assert!(packets.iter().any(|p| matches!(p, ServerPacket::Pushed { location, .. } if *location == point(20, 23))));
                }
            } else if typed == Some(0) {
                seen[2] = true;
                assert_eq!(hits.len(), 1);
                assert_eq!(hits[0]["due_ms"], now + 400);
            }
            let bytes = zone.checkpoint_bytes().unwrap();
            let restored = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
            assert_eq!(
                restored.canonical_state_root().unwrap(),
                zone.canonical_state_root().unwrap()
            );
            if seen.iter().all(|v| *v) {
                break;
            }
        }
        assert_eq!(seen, [true, true, true], "all branches exercised for {ai}");
    }
}

#[test]
fn elder_push_owner_and_aoi_edges_receive_correct_surfaces() {
    for offset in 0..96 {
        let (zone, out) = attack_fixture(125, 3_000 + offset);
        let owner = packets_for(&out, "target");
        if !owner
            .iter()
            .any(|p| matches!(p, ServerPacket::Pushed { .. }))
        {
            continue;
        }
        let pushed: Vec<_> = owner
            .iter()
            .filter_map(|p| match p {
                ServerPacket::Pushed { location, .. } => Some(location.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(pushed, vec![point(20, 22), point(20, 23)]);
        let old = packets_for(&out, "old-edge");
        assert!(old
            .iter()
            .any(|p| matches!(p, ServerPacket::ObjectRemove { object_id: 101 })));
        assert!(!old
            .iter()
            .any(|p| matches!(p, ServerPacket::ObjectPushed { object_id: 101, .. })));
        let new = packets_for(&out, "new-edge");
        assert!(new.iter().any(|p| matches!(p, ServerPacket::ObjectPlayer { info } if info.object_id == 101 && info.location == point(20, 23))));
        assert!(!new
            .iter()
            .any(|p| matches!(p, ServerPacket::ObjectPushed { object_id: 101, .. })));
        let data: Value = serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
        assert_eq!(data["players"]["target"]["run_step_until_ms"], 0);
        assert_eq!(
            data["players"]["target"]["movement_actions"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        return;
    }
    panic!("push branch not exercised");
}

fn elder_push_fixture(blocked: bool) -> (ZoneRuntime, u64, Vec<ZoneOutbound>) {
    for offset in 0..96 {
        let now = 3_000 + offset;
        let (zone, out) = attack_fixture_with_blocked_push(125, now, blocked);
        if packets_for(&out, "target").iter().any(|packet| {
            matches!(packet, ServerPacket::ObjectAttack { info }
                if info.object_id == MONSTER_ID && info.attack_type == 1)
        }) {
            return (zone, now, out);
        }
    }
    panic!("Elder push branch not exercised");
}

fn admit_target_and_spawn_attack_dummy(zone: &mut ZoneRuntime, position: Point, now_ms: u64) {
    zone.handle(ZoneCommand::sync_player_combat_state(
        SessionId::new("target"),
        MirClass::Warrior,
        true,
        false,
        false,
        false,
        false,
        false,
    ));
    let mut dummy = monster(0, position);
    dummy.object_id = MONSTER_ID + 1;
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("target"),
        monster: dummy,
        now_ms,
    });
}

fn attack_dummy(zone: &mut ZoneRuntime, now_ms: u64) -> Vec<ZoneOutbound> {
    zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("target"),
        object_id: MONSTER_ID + 1,
        direction: MirDirection::Right,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 10,
        now_ms,
    })
}

fn player_attacked(out: &[ZoneOutbound]) -> bool {
    packets_for(out, "target").iter().any(
        |packet| matches!(packet, ServerPacket::ObjectAttack { info } if info.object_id == 101),
    )
}

#[test]
fn elder_push_blocks_movement_and_attack_for_500ms_across_checkpoint() {
    let (mut zone, now, _) = elder_push_fixture(false);
    let target = SessionId::new("target");
    assert_eq!(zone.player_position(&target), Some(point(20, 23)));
    admit_target_and_spawn_attack_dummy(&mut zone, point(21, 23), now);
    let bytes = zone.checkpoint_bytes().unwrap();
    let mut zone = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
    assert_eq!(zone.checkpoint_bytes().unwrap(), bytes);

    for (seq, time) in [(1, now), (2, now + 499)] {
        assert!(!player_attacked(&attack_dummy(&mut zone, time)));
        // A rejected attack must not reset the movement deadline and let the
        // same player bypass the push lock by immediately sending Walk.
        zone.handle(ZoneCommand::Walk {
            session_id: target.clone(),
            direction: MirDirection::Down,
            seq,
            now_ms: time,
        });
        zone.handle(ZoneCommand::TickPlayerMovement {
            session_id: target.clone(),
            now_ms: time,
        });
        assert_eq!(zone.player_position(&target), Some(point(20, 23)));
    }

    let movement = zone.handle(ZoneCommand::TickPlayerMovement {
        session_id: target.clone(),
        now_ms: now + 500,
    });
    assert_eq!(zone.player_position(&target), Some(point(20, 24)));
    assert!(packets_for(&movement, "target")
        .iter()
        .any(|packet| matches!(packet, ServerPacket::UserLocation { .. })));
    // The dummy remains diagonally adjacent after the first restored walk.
    assert!(player_attacked(&attack_dummy(&mut zone, now + 500)));
}

#[test]
fn occupied_first_push_tile_still_applies_the_500ms_action_lock() {
    let (mut zone, now, push) = elder_push_fixture(true);
    let target = SessionId::new("target");
    assert_eq!(zone.player_position(&target), Some(point(20, 21)));
    assert!(!packets_for(&push, "target")
        .iter()
        .any(|packet| matches!(packet, ServerPacket::Pushed { .. })));
    admit_target_and_spawn_attack_dummy(&mut zone, point(21, 21), now);
    assert!(!player_attacked(&attack_dummy(&mut zone, now + 499)));

    let bytes = zone.checkpoint_bytes().unwrap();
    let mut moving = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
    moving.handle(ZoneCommand::Walk {
        session_id: target.clone(),
        direction: MirDirection::Left,
        seq: 1,
        now_ms: now + 499,
    });
    moving.handle(ZoneCommand::TickPlayerMovement {
        session_id: target.clone(),
        now_ms: now + 499,
    });
    assert_eq!(moving.player_position(&target), Some(point(20, 21)));
    moving.handle(ZoneCommand::TickPlayerMovement {
        session_id: target.clone(),
        now_ms: now + 500,
    });
    assert_eq!(moving.player_position(&target), Some(point(19, 21)));
    assert!(player_attacked(&attack_dummy(&mut zone, now + 500)));
}

#[test]
fn armadillo_pet_only_captured_hits_apply_to_real_pet_after_checkpoint() {
    let mut exercised = false;
    for offset in 0..32 {
        let mut zone = ZoneRuntime::new(ZoneKey::for_map(SOURCE_MAP));
        let mut owner = player("owner", 101, point(20, 22));
        owner.class = MirClass::Taoist;
        owner.chat_profile.in_safe_zone = true;
        zone.handle(ZoneCommand::Join(owner));
        zone.handle(ZoneCommand::PlayerCastMagic {
            session_id: SessionId::new("owner"),
            object_id: 0,
            spell: Spell::SummonSkeleton,
            direction: MirDirection::Up,
            target: point(20, 21),
            cast: true,
            level: 2,
            damage: 0,
            mp_cost: 0,
            cooldown_ms: 1000,
            now_ms: 10,
        });
        zone.tick(510);
        let pet = zone
            .native_monster_snapshots()
            .into_iter()
            .find(|m| m.name == "BoneFamiliar")
            .unwrap()
            .object_id;
        let mut source = monster(124, point(20, 20));
        source.name = "Armadillo".into();
        source.max_hp = 100000;
        source.hp = 100000;
        zone.spawn_world_event_monster(&source, 511);
        // DigOutZombie action gate is part of the public lifecycle.
        zone.tick(512);
        let now = 3000 + offset;
        let out = zone.tick(now);
        if !packets_for(&out, "owner").iter().any(
            |p| matches!(p, ServerPacket::ObjectAttack { info } if info.object_id == MONSTER_ID),
        ) {
            continue;
        }
        let before = zone
            .native_monster_snapshots()
            .into_iter()
            .find(|m| m.object_id == pet)
            .unwrap()
            .hp;
        let mut restored =
            ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
        let mut hit = false;
        for time in [now + 399, now + 400, now + 600, now + 800] {
            let actual = zone.tick(time);
            assert_eq!(actual, restored.tick(time));
            if time == now + 399 {
                assert_eq!(
                    zone.native_monster_snapshots()
                        .into_iter()
                        .find(|m| m.object_id == pet)
                        .unwrap()
                        .hp,
                    before
                );
            }
            hit |= packets_for(&actual, "owner").iter().any(|p| matches!(p, ServerPacket::ObjectStruck { info } if info.object_id == pet && info.attacker_id == MONSTER_ID));
        }
        assert!(hit);
        assert_eq!(
            zone.player_vitals(&SessionId::new("owner")).unwrap().0,
            100000
        );
        exercised = true;
        break;
    }
    assert!(exercised, "real pet must wake and trigger Armadillo attack");
}
