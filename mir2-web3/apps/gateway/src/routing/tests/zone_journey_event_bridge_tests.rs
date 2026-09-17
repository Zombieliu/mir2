use super::super::{ZoneJourneyEventIdentity, ZoneJourneyEventProgress};
use super::*;
use mir2_simulation::{
    ZoneJourneyEventKind, ZoneJourneyEventReceipt, ZoneJourneyPhysicalTechnique,
};

fn fixture(
    name: &str,
) -> (
    Arc<Mutex<SharedInProcessZoneState>>,
    SharedInProcessZoneSessionRuntime,
) {
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(shared.clone());
    start_new_runtime_with_class(&mut runtime, name, name, MirClass::Warrior);
    (shared, runtime)
}

fn receipt(
    runtime: &SharedInProcessZoneSessionRuntime,
    event_sequence: u64,
) -> ZoneJourneyEventReceipt {
    let identity = runtime.inner.active_identity().unwrap();
    let key = ZonePresenceKey::from_identity(&identity);
    let state = runtime.zone_state.lock().unwrap();
    let session_id = state.zone_sessions[&key].clone();
    ZoneJourneyEventReceipt {
        kind: ZoneJourneyEventKind::PhysicalDamage {
            technique: ZoneJourneyPhysicalTechnique::Normal,
        },
        session_id: session_id.clone(),
        account_id: identity.account_id,
        character_index: identity.character_index,
        object_id: state.players[&key].zone_object_id,
        life_generation: state
            .zone_manager
            .player_life_generation(&session_id)
            .unwrap(),
        zone_key: ZoneKey::for_map(&state.players[&key].map_file_name),
        source_action_at_ms: 100,
        committed_at_ms: 101,
        event_sequence,
        source_object_id: state.players[&key].zone_object_id,
        target_object_id: Some(9101),
        target_location: Some(Point { x: 12, y: 10 }),
        effect_object_id: None,
        damage: Some(7),
        material_consumed: false,
    }
}

fn queue(runtime: &SharedInProcessZoneSessionRuntime, receipts: Vec<ZoneJourneyEventReceipt>) {
    runtime.zone_state.lock().unwrap().dispatch_zone_outbounds(
        receipts
            .into_iter()
            .map(|receipt| ZoneOutbound::JourneyEvent { receipt })
            .collect(),
        None,
    );
}

fn progress_for(receipt: &ZoneJourneyEventReceipt) -> ZoneJourneyEventProgress {
    ZoneJourneyEventProgress {
        object_id: receipt.object_id,
        life_generation: receipt.life_generation,
        committed: BTreeSet::from([ZoneJourneyEventIdentity::from(receipt)]),
    }
}

fn taoist_healing_training_fixture(
    name: &str,
) -> (
    Arc<Mutex<SharedInProcessZoneState>>,
    SharedInProcessZoneSessionRuntime,
) {
    // Both the personal session and the Zone cache the server cadence when
    // constructed. Restore a durable in-progress N4 record only after those
    // authorities have been created with the actual V2 cadence.
    assert_eq!(
        std::env::var("MIR2_QUEST_CADENCE").as_deref(),
        Ok("newcomer-v2"),
        "run this opt-in integration test in an isolated V2 process"
    );
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(shared.clone());
    start_new_runtime_with_class(&mut runtime, name, name, MirClass::Taoist);

    let mut save = runtime
        .inner
        .active_character_checkpoint()
        .expect("Taoist fixture should have an active durable character");
    save.character.level = 7;
    // Healing's real starter-book state is deliberately the `minor-heal`
    // alias, rather than a test-only `healing` spelling.
    save.skill_states_json = vec![serde_json::json!({
        "key": "minor-heal",
        "name": "Minor Heal",
        "description": "Restores a small amount of HP.",
        "level": 0,
        "experience": 0,
        "cooldown_ticks": 6,
        "cooldown_ends_at": 0,
    })
    .to_string()];
    save.quest_states_json = vec![serde_json::json!({
        "quest_id": 2_110_004,
        "title": "Your first class skill",
        "summary": "",
        "reward_preview": "",
        "required": 3,
        "current": 0,
        "stage": "inProgress",
        "task_progress": { "v2:accepted_at:1": 1 },
        "cadence_last_claimed_period": null,
        "cadence_high_watermark_period": null,
    })
    .to_string()];
    runtime
        .inner
        .restore_active_character_checkpoint(&save)
        .expect("restored N4 fixture should be valid");
    runtime.sync_zone_snapshot();
    (shared, runtime)
}

fn safe_arrival_fixture(name: &str) -> SharedInProcessZoneSessionRuntime {
    assert_eq!(
        std::env::var("MIR2_QUEST_CADENCE").as_deref(),
        Ok("newcomer-v2")
    );
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(shared);
    runtime.inner = InProcessWorldRuntime::new(
        GatewayConfig::default()
            .with_crystal_world_runtime()
            .with_platinum_176_profile(),
    );
    start_new_runtime_with_class(&mut runtime, name, name, MirClass::Warrior);
    let mut save = runtime.inner.active_character_checkpoint().unwrap();
    save.character.level = 8;
    save.position = Point { x: 324, y: 275 };
    save.map_file_name = "0".into();
    save.quest_states_json = vec![serde_json::json!({
        "quest_id": 2_110_005,
        "title": "A safe return",
        "summary": "",
        "reward_preview": "",
        "required": 1,
        "current": 0,
        "stage": "inProgress",
        "task_progress": { "v2:accepted_at:1": 1 },
        "cadence_last_claimed_period": null,
        "cadence_high_watermark_period": null,
    })
    .to_string()];
    runtime
        .inner
        .restore_active_character_checkpoint(&save)
        .unwrap();
    runtime.force_next_zone_transform_sync = true;
    runtime.sync_zone_snapshot();
    assert_eq!(
        runtime.inner.local_player_position(),
        Some(Point { x: 324, y: 275 })
    );
    assert_eq!(safe_arrival_progress(&runtime), 0);
    runtime
}

fn safe_arrival_progress(runtime: &SharedInProcessZoneSessionRuntime) -> u64 {
    runtime
        .inner
        .active_character_checkpoint()
        .unwrap()
        .quest_states_json
        .iter()
        .map(|json| serde_json::from_str::<serde_json::Value>(json).unwrap())
        .find(|quest| quest["quest_id"] == 2_110_005)
        .unwrap()["task_progress"]["flag:2210051"]
        .as_u64()
        .unwrap_or(0)
}

#[test]
#[ignore = "requires isolated MIR2_QUEST_CADENCE=newcomer-v2 process"]
fn public_v2_safe_arrival_direct_shared_walk_refreshes_owner_quest() {
    let mut owner = safe_arrival_fixture("ArrivalDirect");
    let packets = owner
        .execute(WorldCommand::ClientPacket(ClientPacket::Walk {
            direction: MirDirection::Up,
        }))
        .unwrap();
    assert_eq!(
        owner.inner.local_player_position(),
        Some(Point { x: 324, y: 274 }),
        "{packets:?}"
    );
    assert_eq!(safe_arrival_progress(&owner), 1);
    assert!(
        packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ChangeQuest {
                quest_id: 2_110_005,
                ..
            }
        )),
        "{packets:?}"
    );
}

#[test]
#[ignore = "requires isolated MIR2_QUEST_CADENCE=newcomer-v2 process"]
fn public_v2_safe_arrival_pending_shared_walk_refreshes_owner_quest() {
    let mut owner = safe_arrival_fixture("ArrivalPending");
    let execution = owner
        .movement_ingress
        .try_execute(ClientPacket::Walk {
            direction: MirDirection::Up,
        })
        .unwrap()
        .expect("low-latency shared movement should execute");
    assert_eq!(
        safe_arrival_progress(&owner),
        0,
        "private quest must await authoritative delivery"
    );
    let packets = owner.execute(WorldCommand::Tick).unwrap();
    assert_eq!(
        owner.inner.local_player_position(),
        Some(Point { x: 324, y: 274 }),
        "{execution:?} {packets:?}"
    );
    assert_eq!(safe_arrival_progress(&owner), 1);
    assert!(
        packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ChangeQuest {
                quest_id: 2_110_005,
                ..
            }
        )),
        "{packets:?}"
    );
}

#[test]
#[ignore = "requires isolated MIR2_QUEST_CADENCE=newcomer-v2 process"]
fn public_v2_safe_arrival_pending_walk_survives_autosave_before_tick() {
    let mut owner = safe_arrival_fixture("ArrivalAutosave");
    owner
        .movement_ingress
        .try_execute(ClientPacket::Walk {
            direction: MirDirection::Up,
        })
        .unwrap()
        .expect("low-latency shared movement should execute");
    owner.save_active_character().unwrap();
    let packets = owner.execute(WorldCommand::Tick).unwrap();
    assert_eq!(
        owner.inner.local_player_position(),
        Some(Point { x: 324, y: 274 })
    );
    assert_eq!(
        safe_arrival_progress(&owner),
        1,
        "autosave must not consume the arrival receipt"
    );
    assert!(
        packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ChangeQuest {
                quest_id: 2_110_005,
                ..
            }
        )),
        "{packets:?}"
    );
}

#[test]
#[ignore = "requires isolated MIR2_QUEST_CADENCE=newcomer-v2 process"]
fn public_v2_safe_arrival_turn_and_occupancy_rejection_do_not_grant_arrival() {
    let mut owner = safe_arrival_fixture("ArrivalRejected");
    owner
        .execute(WorldCommand::ClientPacket(ClientPacket::Turn {
            direction: MirDirection::Up,
        }))
        .unwrap();
    assert_eq!(safe_arrival_progress(&owner), 0);
    let mut observer = shared_session_runtime(owner.zone_state.clone());
    observer.inner = InProcessWorldRuntime::new(
        GatewayConfig::default()
            .with_crystal_world_runtime()
            .with_platinum_176_profile(),
    );
    start_new_runtime_with_class(
        &mut observer,
        "ArrivalBlocker",
        "ArrivalBlocker",
        MirClass::Warrior,
    );
    let mut save = observer.inner.active_character_checkpoint().unwrap();
    save.position = Point { x: 324, y: 274 };
    observer
        .inner
        .restore_active_character_checkpoint(&save)
        .unwrap();
    observer.force_next_zone_transform_sync = true;
    observer.sync_zone_snapshot();
    let packets = owner
        .execute(WorldCommand::ClientPacket(ClientPacket::Walk {
            direction: MirDirection::Up,
        }))
        .unwrap();
    assert_eq!(
        owner.inner.local_player_position(),
        Some(Point { x: 324, y: 275 }),
        "{packets:?}"
    );
    assert_eq!(safe_arrival_progress(&owner), 0);
    assert!(
        !packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ChangeQuest {
                quest_id: 2_110_005,
                ..
            }
        )),
        "rejected movement must not grant arrival: {packets:?}"
    );
}

#[test]
fn zone_journey_event_is_owner_bound_deduplicated_and_checkpointed() {
    let (_, owner) = fixture("JourneyOwner");
    let event = receipt(&owner, 11);
    queue(&owner, vec![event.clone(), event.clone()]);
    let key = owner.current_presence_key().unwrap();
    {
        let state = owner.zone_state.lock().unwrap();
        assert_eq!(state.pending_zone_journey_events[&key].len(), 1);
    }
    {
        let mut state = owner.zone_state.lock().unwrap();
        let checkpoint = serde_json::to_vec(&state.checkpoint().unwrap()).unwrap();
        *state = SharedInProcessZoneState::restore(serde_json::from_slice(&checkpoint).unwrap())
            .unwrap();
    }
    queue(&owner, vec![event]);
    assert_eq!(
        owner.zone_state.lock().unwrap().pending_zone_journey_events[&key].len(),
        1
    );
}

#[test]
fn zone_journey_event_rejects_wrong_owner_stale_life_and_post_fence_receipts() {
    let (_, runtime) = fixture("JourneyFence");
    let valid = receipt(&runtime, 21);
    let key = runtime.current_presence_key().unwrap();
    let mut invalid = Vec::new();
    for case in 0..7 {
        let mut event = valid.clone();
        match case {
            0 => event.account_id.push_str("-wrong"),
            1 => event.character_index += 1,
            2 => event.object_id += 1,
            3 => event.life_generation += 1,
            4 => event.zone_key = ZoneKey::for_map("3"),
            5 => event.event_sequence = 0,
            _ => event.source_object_id = 0,
        }
        invalid.push(event);
    }
    queue(&runtime, invalid);
    assert!(runtime
        .zone_state
        .lock()
        .unwrap()
        .pending_zone_journey_events
        .get(&key)
        .is_none());
    queue(&runtime, vec![valid.clone()]);
    {
        let mut state = runtime.zone_state.lock().unwrap();
        state.begin_teardown_fence(&key).unwrap();
    }
    let mut late = valid;
    late.event_sequence += 1;
    queue(&runtime, vec![late]);
    let state = runtime.zone_state.lock().unwrap();
    assert_eq!(state.pending_zone_journey_events[&key].len(), 1);
    assert_eq!(
        ZoneJourneyEventIdentity::from(&state.pending_zone_journey_events[&key][0]),
        ZoneJourneyEventIdentity {
            source_action_at_ms: 100,
            event_sequence: 21,
        }
    );
}

#[test]
fn zone_journey_event_presence_cleanup_discards_pending_and_replay_fence() {
    let (_, runtime) = fixture("JourneyCleanup");
    let event = receipt(&runtime, 31);
    let key = runtime.current_presence_key().unwrap();
    queue(&runtime, vec![event.clone()]);
    let mut state = runtime.zone_state.lock().unwrap();
    state
        .journey_event_progress
        .insert(key.clone(), progress_for(&event));
    assert!(state.journey_event_progress.contains_key(&key));
    state.forget_zone_session(&key);
    assert!(!state.pending_zone_journey_events.contains_key(&key));
    assert!(!state.journey_event_progress.contains_key(&key));
}

#[test]
fn zone_journey_event_not_needed_discards_pending_and_replay_fence() {
    let (_, mut runtime) = fixture("JourneyNotNeeded");
    let event = receipt(&runtime, 41);
    let key = runtime.current_presence_key().unwrap();
    queue(&runtime, vec![event.clone()]);
    {
        let mut state = runtime.zone_state.lock().unwrap();
        state
            .journey_event_progress
            .insert(key.clone(), progress_for(&event));
    }

    // A default V1 session has no accepted in-progress V2 training flag.
    assert!(!runtime.inner.needs_zone_journey_evidence());
    assert!(runtime.apply_pending_zone_journey_events(false).is_empty());
    let state = runtime.zone_state.lock().unwrap();
    assert!(!state.pending_zone_journey_events.contains_key(&key));
    assert!(!state.journey_event_progress.contains_key(&key));
}

#[test]
#[ignore = "requires isolated MIR2_QUEST_CADENCE=newcomer-v2 process"]
fn public_taoist_healing_commits_n4_flag_through_the_zone_journey_bridge() {
    let (shared, mut owner) = taoist_healing_training_fixture("JourneyHealingOwner");
    assert!(
        owner.inner.needs_zone_journey_evidence(),
        "restored N4 class-practice flag should request trusted Zone evidence"
    );
    let mut observer = shared_session_runtime(shared);
    start_new_runtime_with_class(
        &mut observer,
        "JourneyHealingObserver",
        "JourneyHealingObserver",
        MirClass::Warrior,
    );

    let owner_entity = owner
        .world_snapshot()
        .entities
        .iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .expect("owner should be visible in its public snapshot")
        .clone();
    let monster = owner
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.kind == WorldEntityKind::Monster && !entity.dead)
        .expect("fixture should expose a normal monster target");
    let rejected = owner
        .execute(WorldCommand::ClientPacket(ClientPacket::Magic {
            object_id: owner_entity.object_id,
            spell: Spell::Healing,
            direction: MirDirection::Right,
            target_id: monster.object_id,
            location: Point {
                x: monster.x,
                y: monster.y,
            },
            spell_target_lock: true,
        }))
        .unwrap();
    assert!(!rejected.iter().any(|packet| matches!(packet,
        ServerPacket::Magic { spell: Spell::Healing, cast: true, .. })),
        "Healing must reject a monster through the Zone, not fall back to the personal caster: {rejected:?}");
    let packets = owner
        .execute(WorldCommand::ClientPacket(ClientPacket::Magic {
            object_id: owner_entity.object_id,
            spell: Spell::Healing,
            direction: MirDirection::Right,
            target_id: owner_entity.object_id,
            location: Point {
                x: owner_entity.x,
                y: owner_entity.y,
            },
            spell_target_lock: true,
        }))
        .expect("ordinary self Healing should execute through the shared Zone");

    assert!(
        packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::Magic {
                spell: Spell::Healing,
                cast: true,
                target_id,
                ..
            } if *target_id == owner_entity.object_id
        )),
        "{packets:?}"
    );
    assert!(
        packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ChangeQuest {
                quest_id: 2_110_004,
                task_list,
                ..
            } if task_list.iter().any(|task| task == "Complete your class practice 1/1")
        )),
        "the trusted Zone receipt must project the N4 flag to its owner: {packets:?}"
    );

    let save = owner
        .inner
        .active_character_checkpoint()
        .expect("owner quest progress should checkpoint after the bridge commit");
    let n4 = save
        .quest_states_json
        .iter()
        .map(|json| serde_json::from_str::<serde_json::Value>(json).unwrap())
        .find(|quest| quest["quest_id"] == 2_110_004)
        .expect("N4 should remain in the owner checkpoint");
    assert_eq!(n4["task_progress"]["flag:2210041"], 1);
    assert_eq!(n4["current"], 1, "the two Oma kills remain independent");
    assert_eq!(n4["required"], 3);

    let observer_packets = observer
        .execute(WorldCommand::Tick)
        .expect("observer tick should drain only its public Zone packets");
    assert!(
        !observer_packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ChangeQuest {
                quest_id: 2_110_004,
                ..
            }
        )),
        "N4 quest projection must remain owner-only: {observer_packets:?}"
    );
}
