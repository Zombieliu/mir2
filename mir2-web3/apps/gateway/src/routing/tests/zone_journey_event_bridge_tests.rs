use super::*;
use super::super::{ZoneJourneyEventIdentity, ZoneJourneyEventProgress};
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
