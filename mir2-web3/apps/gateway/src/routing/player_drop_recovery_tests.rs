//! Pure factory/world fixtures. The .invalid source binding supplies only an
//! in-process test capability; no PostgreSQL connection or commit is claimed.
use super::*;
use mir2_simulation::{
    AccountStoreDatabaseMode, PlayerDropCommitReceipt, PlayerDropEffectState,
    PlayerDropOperationBinding, PlayerDropOutcome, PlayerDropPreparedOperation,
    PlayerDropWorldIntent, SimulationConfig, SimulationSession, ZoneSaveFence,
};

fn factory() -> (SharedInProcessZoneRuntimeFactory, ZoneId) {
    let zone = ZoneId::primary();
    let factory = SharedInProcessZoneRuntimeFactory::without_background_owner_for_test(&zone);
    factory.mark_zone_as_replica(&zone);
    (factory, zone)
}

fn canonical(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(fields) => serde_json::Value::Object(
            fields
                .into_iter()
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .map(|(key, value)| (key, canonical(value)))
                .collect(),
        ),
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(canonical).collect())
        }
        value => value,
    }
}

fn proof(
    zone: &ZoneId,
    map: &str,
    nonce: &str,
    state: PlayerDropEffectState,
) -> PlayerDropCommittedEffect {
    const URL: &str = "postgresql://fixture@player-drop-recovery.invalid:55491/source";
    let mut session = SimulationSession::new(SimulationConfig::default());
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let mut before = session.active_character_checkpoint().unwrap();
    before.map_file_name = map.into();
    let mut after = before.clone();
    after.revision += 1;
    after.gold -= 7;
    let binding = PlayerDropOperationBinding::new(
        nonce.into(),
        "departed-player:0".into(),
        "demo".into(),
        before.character.index,
        zone.as_str().into(),
        "old-owner".into(),
        7,
        1,
    )
    .unwrap();
    let intent = PlayerDropWorldIntent::new(
        binding,
        ZoneKey::for_map(map),
        before.position.clone(),
        100,
        1_800_100,
        PlayerDropOutcome::Gold { amount: 7 },
    )
    .unwrap();
    let source = canonical(serde_json::to_value((&intent, &before, &before, &after)).unwrap());
    let mut hash = Sha256::new();
    hash.update(b"mir2/player-drop/prepared-source/v1\0");
    hash.update(serde_json::to_vec(&source).unwrap());
    let digest: String = hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let operation: PlayerDropPreparedOperation = serde_json::from_value(serde_json::json!({
        "intent": intent, "before": before, "runtime_before": before,
        "checkpoint": after, "operation_digest": digest,
    }))
    .unwrap();
    let receipt = PlayerDropCommitReceipt::new(
        operation.binding().operation_key(),
        operation.intent().world_effect_id().into(),
        operation.operation_digest().into(),
        operation.intent().intent_digest().into(),
        operation.checkpoint().revision,
        state,
    );
    let mut source = SimulationConfig::default();
    source.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;
    source.account_store_database_url = Some(URL.into());
    let source = source
        .with_zone_save_fence(
            ZoneSaveFence::new(zone.as_str().into(), "successor-owner".into(), 8).unwrap(),
        )
        .unwrap();
    PlayerDropCommittedEffect::from_verified_receipt(
        &source.zone_economy_write_permit().unwrap(),
        URL,
        operation,
        receipt,
    )
    .unwrap()
}

fn ordinary_drop(object_id: u32) -> GroundDropSnapshot {
    GroundDropSnapshot {
        object_id,
        name: "unrelated Gold".into(),
        name_colour_argb: -1,
        icon: 0,
        x: 0,
        y: 0,
        quantity: 23,
        source_monster: "unrelated ordinary fixture".into(),
        owner_object_id: None,
        ownership_remaining_ticks: None,
        loot: GroundDropLootSnapshot::Gold { amount: 23 },
    }
}

#[test]
fn recovery_requires_replica_and_disabled_ticks() {
    let (factory, zone) = factory();
    factory.replica_zone_ids.lock().unwrap().remove(&zone);
    assert!(
        factory
            .reconcile_player_drop_recovery(&zone, &[], 200)
            .is_err()
    );
    factory.mark_zone_as_replica(&zone);
    let resources = factory.zones.lock().unwrap().get(&zone).unwrap().clone();
    resources
        .autonomous_ticks_enabled
        .store(true, Ordering::Release);
    assert!(
        factory
            .reconcile_player_drop_recovery(&zone, &[], 200)
            .is_err()
    );
    resources
        .autonomous_ticks_enabled
        .store(false, Ordering::Release);
    factory
        .reconcile_player_drop_recovery(&zone, &[], 200)
        .unwrap();
    assert!(factory.is_zone_replica(&zone));
    assert!(!factory.autonomous_ticks_enabled(&zone));
}

#[test]
fn recovery_creates_inactive_resources_without_original_session() {
    let zone = ZoneId::primary();
    let factory = SharedInProcessZoneRuntimeFactory::new().fresh_replica();
    factory.mark_zone_as_replica(&zone);
    assert_eq!(factory.active_zone_count(), 0);
    let value = proof(&zone, "cold-map", "cold-drop", PlayerDropEffectState::Live);
    factory
        .reconcile_player_drop_recovery(&zone, &[value.clone()], 200)
        .unwrap();
    assert_eq!(factory.active_zone_count(), 1);
    assert!(!factory.autonomous_ticks_enabled(&zone));
    let resources = factory.zones.lock().unwrap().get(&zone).unwrap().clone();
    let state = resources.zone_state.lock().unwrap();
    assert!(state.zone_sessions.is_empty());
    assert!(matches!(
        state.zone_manager.player_drop_effect_state_for_key(
            value.intent().zone(),
            value.intent().world_effect_id()
        ),
        Some(PlayerDropCustodyState::Live { .. })
    ));
}

#[test]
fn empty_recovery_creates_a_promotable_inactive_owner_resource() {
    let zone = ZoneId::primary();
    let factory = SharedInProcessZoneRuntimeFactory::new().fresh_replica();
    factory.mark_zone_as_replica(&zone);
    assert_eq!(factory.active_zone_count(), 0);
    factory
        .reconcile_player_drop_recovery(&zone, &[], 200)
        .unwrap();
    assert_eq!(factory.active_zone_count(), 1);
    assert!(!factory.autonomous_ticks_enabled(&zone));
    factory.promote_zone_from_replica(&zone).unwrap();
    assert!(factory.autonomous_ticks_enabled(&zone));
}

#[test]
fn recovery_installs_all_logical_maps_without_original_session_and_prunes_empty_sets() {
    let (factory, zone) = factory();
    let a = proof(&zone, "recovery-a", "drop-a", PlayerDropEffectState::Live);
    let b = proof(&zone, "recovery-b", "drop-b", PlayerDropEffectState::Live);
    factory
        .reconcile_player_drop_recovery(&zone, &[a.clone(), b.clone()], 200)
        .unwrap();
    let resources = factory.zones.lock().unwrap().get(&zone).unwrap().clone();
    {
        let mut state = resources.zone_state.lock().unwrap();
        assert!(state.players.is_empty());
        assert!(state.zone_sessions.is_empty());
        for value in [&a, &b] {
            let key = value.intent().zone();
            let PlayerDropCustodyState::Live { object_id, .. } = state
                .zone_manager
                .player_drop_effect_state_for_key(key, value.intent().world_effect_id())
                .unwrap()
            else {
                panic!("live recovered custody")
            };
            assert_eq!(
                state.maps[&key.map_file_name].ground_drops[&object_id].quantity,
                7
            );
            assert_eq!(
                state.maps[&key.map_file_name].drop_expires_at_ms[&object_id],
                1_800_100
            );
        }
        state
            .maps
            .get_mut("recovery-b")
            .unwrap()
            .ground_drops
            .insert(880_001, ordinary_drop(880_001));
    }
    let consumed = proof(
        &zone,
        "recovery-a",
        "drop-a",
        PlayerDropEffectState::Consumed,
    );
    factory
        .reconcile_player_drop_recovery(&zone, &[consumed], 300)
        .unwrap();
    let state = resources.zone_state.lock().unwrap();
    assert_eq!(
        state
            .zone_manager
            .player_drop_effect_state_for_key(a.intent().zone(), a.intent().world_effect_id()),
        Some(PlayerDropCustodyState::Consumed)
    );
    assert!(
        state
            .zone_manager
            .player_drop_effect_state_for_key(b.intent().zone(), b.intent().world_effect_id())
            .is_none()
    );
    assert!(state.maps["recovery-a"].ground_drops.is_empty());
    assert_eq!(state.maps["recovery-b"].ground_drops.len(), 1);
    assert_eq!(
        state.maps["recovery-b"].ground_drops[&880_001],
        ordinary_drop(880_001)
    );
}

#[test]
fn recovery_rejects_wrong_owner_group_and_duplicate_effects_without_publishing() {
    let (factory, zone) = factory();
    let resources = factory.zones.lock().unwrap().get(&zone).unwrap().clone();
    let before = resources
        .zone_state
        .lock()
        .unwrap()
        .zone_manager
        .checkpoint_bytes()
        .unwrap();
    let other = proof(
        &ZoneId::new("other-group"),
        "recovery-a",
        "wrong-group",
        PlayerDropEffectState::Live,
    );
    assert!(
        factory
            .reconcile_player_drop_recovery(&zone, &[other], 200)
            .is_err()
    );
    let value = proof(
        &zone,
        "recovery-a",
        "duplicate",
        PlayerDropEffectState::Live,
    );
    assert!(
        factory
            .reconcile_player_drop_recovery(&zone, &[value.clone(), value], 200)
            .is_err()
    );
    assert_eq!(
        resources
            .zone_state
            .lock()
            .unwrap()
            .zone_manager
            .checkpoint_bytes()
            .unwrap(),
        before
    );
}

#[test]
fn late_projection_conflict_leaves_world_and_unrelated_drop_unchanged() {
    let (factory, zone) = factory();
    let value = proof(&zone, "recovery-a", "conflict", PlayerDropEffectState::Live);
    let resources = factory.zones.lock().unwrap().get(&zone).unwrap().clone();
    let before = resources
        .zone_state
        .lock()
        .unwrap()
        .zone_manager
        .checkpoint_bytes()
        .unwrap();
    let mut staged = ZoneManager::restore_checkpoint(&before).unwrap();
    staged
        .reconcile_committed_player_drops_for_key(value.intent().zone(), &[value.clone()], 200)
        .unwrap();
    let PlayerDropCustodyState::Live { object_id, .. } = staged
        .player_drop_effect_state_for_key(value.intent().zone(), value.intent().world_effect_id())
        .unwrap()
    else {
        panic!("live staged custody")
    };
    resources
        .zone_state
        .lock()
        .unwrap()
        .maps
        .entry("recovery-a".into())
        .or_default()
        .ground_drops
        .insert(object_id, ordinary_drop(object_id));
    assert!(
        factory
            .reconcile_player_drop_recovery(&zone, &[value], 200)
            .is_err()
    );
    let state = resources.zone_state.lock().unwrap();
    assert_eq!(state.zone_manager.checkpoint_bytes().unwrap(), before);
    assert_eq!(
        state.maps["recovery-a"].ground_drops[&object_id],
        ordinary_drop(object_id)
    );
}

#[test]
fn host_clock_cannot_silently_expire_a_canonical_live_receipt() {
    let (factory, zone) = factory();
    let value = proof(
        &zone,
        "recovery-a",
        "clock-skew",
        PlayerDropEffectState::Live,
    );
    let resources = factory.zones.lock().unwrap().get(&zone).unwrap().clone();
    let before = resources
        .zone_state
        .lock()
        .unwrap()
        .zone_manager
        .checkpoint_bytes()
        .unwrap();
    assert!(
        factory
            .reconcile_player_drop_recovery(&zone, &[value], 1_800_101)
            .unwrap_err()
            .contains("disagrees with its canonical receipt")
    );
    assert_eq!(
        resources
            .zone_state
            .lock()
            .unwrap()
            .zone_manager
            .checkpoint_bytes()
            .unwrap(),
        before
    );
}

#[test]
fn expired_recovery_deadline_never_enables_replica_ticks() {
    let (factory, zone) = factory();
    let expired = std::time::Instant::now()
        .checked_sub(Duration::from_secs(1))
        .unwrap();
    assert!(
        factory
            .promote_zone_from_replica_before(&zone, Some(expired))
            .unwrap_err()
            .contains("expired")
    );
    assert!(factory.is_zone_replica(&zone));
    assert!(!factory.autonomous_ticks_enabled(&zone));
}
