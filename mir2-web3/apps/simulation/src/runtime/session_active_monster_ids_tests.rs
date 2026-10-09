use super::*;
use crate::{InProcessWorldRuntime, WorldCommand, WorldEntityDisposition, WorldRuntime};
use bevy_ecs::entity::Entity;
use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender};
use std::time::Instant;

fn legacy_active_monster_ids(session: &SimulationSession) -> Vec<u32> {
    session
        .current_map_shared_entity_snapshots()
        .into_iter()
        .filter(|entity| {
            entity.kind == WorldEntityKind::Monster
                && !entity.dead
                && !entity.hp.is_some_and(|hp| hp <= 0)
        })
        .map(|entity| entity.object_id)
        .collect()
}

fn assert_equivalent(session: &SimulationSession) -> Vec<u32> {
    let legacy = legacy_active_monster_ids(session);
    let ids = session.current_map_active_monster_ids();
    assert_eq!(ids, legacy);
    ids
}

fn login_demo(session: &mut SimulationSession) {
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(session.current_map_file_name().is_some());
}

fn fixture_monster(world: &mut World, object_id: u32, hp: Option<i32>, dead: bool) -> Entity {
    let position = Point { x: -400, y: -500 };
    // No Monster marker is required by the old projection. A position outside
    // the scene/map bounds is also retained when its scene_view argument is None.
    let mut entity = world.spawn((
        ObjectId(object_id),
        DisplayName::literal("Skeleton"),
        Position(position.clone()),
        Facing(MirDirection::Left),
        MonsterAgent {
            image: 3,
            dead,
            patrol_origin: position,
            ai: 0,
            disposition: WorldEntityDisposition::Hostile,
            hostile_to_player: true,
            tracking_player: false,
            view_range: 10,
            can_wander: false,
            move_interval_ticks: 1,
            attack_interval_ticks: 1,
            next_move_tick: 0,
            next_attack_tick: 0,
            route: Vec::new(),
            route_index: 0,
            route_waiting: false,
            next_route_tick: 0,
        },
    ));
    if let Some(hp) = hp {
        entity.insert(MonsterVitals { hp, max_hp: 20 });
    }
    entity.id()
}

#[test]
fn active_monster_ids_match_real_map_lifecycle_and_do_not_mutate_snapshots() {
    let config = SimulationConfig::default()
        .with_crystal_world_runtime()
        .with_platinum_176_profile();
    let mut session = SimulationSession::new(config);
    assert_equivalent(&session);
    login_demo(&mut session);
    assert_eq!(session.current_map_file_name().as_deref(), Some("0"));
    assert!(!assert_equivalent(&session).is_empty());
    let before = session.world_snapshot();
    assert_equivalent(&session);
    assert_eq!(session.world_snapshot(), before);

    for (key, map) in [("crystal:D401:24:181", "D401"), ("crystal:0:290:618", "0")] {
        // Trusted in-process transfer fixture, never a normal-client command.
        let packets = session.transfer_map(key);
        assert!(packets.iter().any(|packet| matches!(
            packet, ServerPacket::MapInformation { info } if info.file_name == map
        )));
        assert_eq!(session.current_map_file_name().as_deref(), Some(map));
        assert!(!assert_equivalent(&session).is_empty());
    }
    session.handle_packet(ClientPacket::LogOut);
    assert_eq!(session.current_map_file_name(), None);
    assert_equivalent(&session);
}

#[test]
fn active_monster_ids_preserve_component_marker_hp_order_and_duplicate_semantics() {
    const FIRST_ID: u32 = 9_000_000;
    let mut session = SimulationSession::new(SimulationConfig::default());
    login_demo(&mut session);
    let mut expected = Vec::new();
    let mut ordinal = 0u32;
    for markers in 0..16 {
        for missing_component in 0..6 {
            for hp in [None, Some(-1), Some(0), Some(1)] {
                for dead in [false, true] {
                    ordinal += 1;
                    // Reverse input IDs require the getter to reproduce sorting.
                    let id = FIRST_ID + 1_000 - ordinal;
                    let entity_id = fixture_monster(session.app.world_mut(), id, hp, dead);
                    let mut entity = session.app.world_mut().entity_mut(entity_id);
                    if markers & 1 != 0 {
                        entity.insert(SelfPlayer);
                    }
                    if markers & 2 != 0 {
                        entity.insert(Hero {
                            owner_name: "Fixture".into(),
                            next_attack_tick: 0,
                            next_move_tick: 0,
                        });
                    }
                    if markers & 4 != 0 {
                        entity.insert(RemotePlayer);
                    }
                    if markers & 8 != 0 {
                        entity.insert(Npc);
                    }
                    match missing_component {
                        1 => {
                            entity.remove::<ObjectId>();
                        }
                        2 => {
                            entity.remove::<DisplayName>();
                        }
                        3 => {
                            entity.remove::<Position>();
                        }
                        4 => {
                            entity.remove::<Facing>();
                        }
                        5 => {
                            entity.remove::<MonsterAgent>();
                        }
                        _ => {}
                    }
                    if missing_component == 0
                        && markers & 7 == 0
                        && !dead
                        && !hp.is_some_and(|hp| hp <= 0)
                    {
                        expected.push(id);
                    }
                }
            }
        }
    }
    let duplicate_id = FIRST_ID + 1_001;
    fixture_monster(session.app.world_mut(), duplicate_id, Some(1), false);
    fixture_monster(session.app.world_mut(), duplicate_id, None, false);
    expected.extend([duplicate_id, duplicate_id]);
    expected.sort();
    let actual = assert_equivalent(&session)
        .into_iter()
        .filter(|id| *id >= FIRST_ID)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    assert_eq!(actual.iter().filter(|id| **id == duplicate_id).count(), 2);
    assert_eq!(ordinal, 768, "all finite combinations must be exercised");
}

#[test]
fn active_monster_ids_track_death_hp_absence_and_revival_without_caching() {
    let mut session = SimulationSession::new(SimulationConfig::default());
    login_demo(&mut session);
    let id = 9_100_001;
    let entity_id = fixture_monster(session.app.world_mut(), id, Some(1), false);
    assert!(assert_equivalent(&session).contains(&id));
    session
        .app
        .world_mut()
        .entity_mut(entity_id)
        .get_mut::<MonsterVitals>()
        .unwrap()
        .hp = 0;
    assert!(!assert_equivalent(&session).contains(&id));
    session
        .app
        .world_mut()
        .entity_mut(entity_id)
        .get_mut::<MonsterVitals>()
        .unwrap()
        .hp = 1;
    session
        .app
        .world_mut()
        .entity_mut(entity_id)
        .get_mut::<MonsterAgent>()
        .unwrap()
        .dead = true;
    assert!(!assert_equivalent(&session).contains(&id));
    session
        .app
        .world_mut()
        .entity_mut(entity_id)
        .get_mut::<MonsterAgent>()
        .unwrap()
        .dead = false;
    assert!(assert_equivalent(&session).contains(&id));
    session
        .app
        .world_mut()
        .entity_mut(entity_id)
        .remove::<MonsterVitals>();
    assert!(assert_equivalent(&session).contains(&id));
    assert!(
        session.zone_monster_spawn_snapshot(id).is_none(),
        "ID discovery must not absorb the later spawn snapshot's stricter vitals rule"
    );
}

#[test]
fn active_monster_ids_preserve_real_npc_visibility_for_mixed_markers() {
    let mut session = SimulationSession::new(SimulationConfig::default());
    login_demo(&mut session);
    let npc = mir2_game_data::crystal_npc_info_manifest()
        .npcs
        .into_iter()
        .find(|npc| {
            npc.loaded_object_id.is_some()
                && npc.flag_needed == 0
                && npc.min_level == 0
                && npc.max_level == 0
                && npc.class_required.is_empty()
                && npc.day_of_week.is_empty()
                && !npc.time_visible
        })
        .expect("real manifest contains an unrestricted NPC");
    let id = npc.loaded_object_id.unwrap();
    let entity_id = fixture_monster(session.app.world_mut(), id, Some(1), false);
    session.app.world_mut().entity_mut(entity_id).insert(Npc);
    assert!(assert_equivalent(&session).contains(&id));
    let character = session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .selected_character
        .take();
    assert!(!assert_equivalent(&session).contains(&id));
    // With the marker removed the same object ID is a monster: NPC visibility
    // must not be applied merely because its ID also occurs in the NPC catalogue.
    session
        .app
        .world_mut()
        .entity_mut(entity_id)
        .remove::<Npc>();
    assert!(assert_equivalent(&session).contains(&id));
    session.app.world_mut().entity_mut(entity_id).insert(Npc);
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .selected_character = character;
    assert!(assert_equivalent(&session).contains(&id));
}

#[test]
fn active_monster_ids_in_process_forwarding_matches_old_projection() {
    let mut runtime = InProcessWorldRuntime::new(SimulationConfig::default());
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        }))
        .unwrap();
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: 0,
        }))
        .unwrap();
    let legacy = runtime
        .current_map_shared_entity_snapshots()
        .into_iter()
        .filter(|entity| {
            entity.kind == WorldEntityKind::Monster
                && !entity.dead
                && !entity.hp.is_some_and(|hp| hp <= 0)
        })
        .map(|entity| entity.object_id)
        .collect::<Vec<_>>();
    assert!(!legacy.is_empty());
    assert_eq!(runtime.current_map_active_monster_ids(), legacy);
}

fn ordinary_benchmark_session(config: SimulationConfig, index: usize) -> SimulationSession {
    let mut session = SimulationSession::new(config);
    let account_id = format!("monid-bench-{index:02}");
    let password = "BenchPass123!";
    assert!(session
        .handle_packet(ClientPacket::NewAccount {
            account_id: account_id.clone(),
            password: password.into(),
            birth_date_binary: 0,
            user_name: String::new(),
            secret_question: String::new(),
            secret_answer: String::new(),
            email_address: String::new(),
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::NewAccount { result: 8 })));
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id,
            password: password.into(),
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let class = [MirClass::Warrior, MirClass::Wizard, MirClass::Taoist][index % 3];
    let character_index = session
        .handle_packet(ClientPacket::NewCharacter {
            name: format!("MonIdB{index:02}"),
            class,
            gender: MirGender::Male,
        })
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
            _ => None,
        })
        .expect("ordinary character creation");
    session.handle_packet(ClientPacket::StartGame { character_index });
    assert_eq!(session.current_map_file_name().as_deref(), Some("0"));
    session
}

#[test]
#[ignore = "explicit release microbenchmark; run with MIR2_QUEST_CADENCE=newcomer-v2"]
fn active_monster_ids_fifteen_crystal_sessions_release_benchmark() {
    assert!(!cfg!(debug_assertions), "run with --release");
    assert_eq!(
        std::env::var("MIR2_QUEST_CADENCE").as_deref(),
        Ok("newcomer-v2")
    );
    const SESSIONS: usize = 15;
    const ROUNDS: usize = 25;
    const TRIALS: usize = 5;
    let config = SimulationConfig::default()
        .with_crystal_world_runtime()
        .with_platinum_176_profile();
    let sessions = (0..SESSIONS)
        .map(|index| ordinary_benchmark_session(config.clone(), index))
        .collect::<Vec<_>>();
    let mut old_shared_records = 0;
    let mut active_ids = 0;
    for session in &sessions {
        let ids = assert_equivalent(session);
        assert!(!ids.is_empty());
        active_ids += ids.len();
        old_shared_records += session.current_map_shared_entity_snapshots().len();
    }
    let time_reads = |legacy: bool| {
        let started = Instant::now();
        let mut checksum = 0usize;
        for _ in 0..ROUNDS {
            for session in &sessions {
                let session = std::hint::black_box(session);
                let ids = if legacy {
                    legacy_active_monster_ids(session)
                } else {
                    session.current_map_active_monster_ids()
                };
                checksum += std::hint::black_box(ids).len();
            }
        }
        (started.elapsed(), checksum)
    };
    let mut old_times = Vec::new();
    let mut new_times = Vec::new();
    for trial in 0..TRIALS {
        let (old, new) = if trial % 2 == 0 {
            (time_reads(true), time_reads(false))
        } else {
            let new = time_reads(false);
            (time_reads(true), new)
        };
        assert_eq!(old.1, active_ids * ROUNDS);
        assert_eq!(new.1, old.1);
        old_times.push(old.0);
        new_times.push(new.0);
        eprintln!(
            "active-monster-ids trial={} sessions={} reads={} old_us={} new_us={}",
            trial + 1,
            SESSIONS,
            SESSIONS * ROUNDS,
            old.0.as_micros(),
            new.0.as_micros()
        );
    }
    old_times.sort();
    new_times.sort();
    eprintln!("active-monster-ids median reads={} old_us={} new_us={} old_shared_entity_projections={} new_entity_projections=0 active_ids={}; counts are logical records, not heap bytes or server capacity",
        SESSIONS * ROUNDS, old_times[TRIALS / 2].as_micros(), new_times[TRIALS / 2].as_micros(),
        old_shared_records * ROUNDS, active_ids * ROUNDS);
    // No timing threshold: these fifteen local personal sessions use real
    // CrystalWorld/platinum176 data and ordinary authentication/StartGame, but
    // this is only a projection microbenchmark, without sockets or a shared Zone.
}
