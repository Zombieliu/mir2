//! Pure routing guards around retained publication evidence. The complete
//! carrier below is a serialized fixture, not evidence of a PostgreSQL commit.
use super::*;
use mir2_simulation::{
    PlayerDropCommitReceipt, PlayerDropEffectState, PlayerDropJournalResult,
    PlayerDropOperationBinding, PlayerDropOutcome, PlayerDropPreparedOperation, PlayerDropRequest,
    PlayerDropWorldIntent, PreparedPlayerDropPublicationFailure, ZoneSaveFence,
};

fn started_runtime() -> SharedInProcessZoneSessionRuntime {
    let state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = super::tests::shared_session_runtime(state);
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
    runtime
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

fn frozen_result(runtime: &SharedInProcessZoneSessionRuntime) -> PlayerDropJournalResult {
    let before = runtime.inner.active_character_checkpoint().unwrap();
    let identity = runtime.inner.active_identity().unwrap();
    let session = runtime.current_zone_session_id().unwrap();
    let mut after = before.clone();
    after.revision += 1;
    after.gold -= 7;
    let binding = PlayerDropOperationBinding::new(
        "guard-fixture-original-operation".into(),
        session.as_str().into(),
        identity.account_id,
        identity.character_index,
        runtime.inventory_zone_id.as_str().into(),
        "guard-fixture-owner".into(),
        1,
        1,
    )
    .unwrap();
    let intent = PlayerDropWorldIntent::new(
        binding,
        ZoneKey::for_map(&before.map_file_name),
        before.position.clone(),
        100,
        1_800_100,
        PlayerDropOutcome::Gold { amount: 7 },
    )
    .unwrap();
    let mut hash = Sha256::new();
    hash.update(b"mir2/player-drop/prepared-source/v1\0");
    hash.update(
        serde_json::to_vec(&canonical(
            serde_json::to_value((&intent, &before, &before, &after)).unwrap(),
        ))
        .unwrap(),
    );
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
        PlayerDropEffectState::Live,
    );
    let reservation = serde_json::from_value(serde_json::json!({
        "zone": operation.intent().zone(), "world_effect_id": operation.intent().world_effect_id(),
        "session_id": session, "player_object_id": runtime.inner.world_snapshot().player_object_id.unwrap(),
        "origin": operation.runtime_before().position, "position": operation.intent().position(),
        "kind": "Gold", "reserved_at_unix_ms": 100,
    })).unwrap();
    let result = PlayerDropJournalResult {
        operation,
        receipt,
        request: PlayerDropRequest::Gold { amount: 7 },
        reservation: Some(reservation),
    };
    result.validate_integrity().unwrap();
    result
}

fn assert_guard(error: String) {
    assert!(
        error.contains("ZONE_ECONOMY_COMMIT_OUTCOME_UNKNOWN"),
        "{error}"
    );
}

#[test]
fn player_drop_journal_peeks_retain_evidence_until_exact_acknowledgement() {
    let mut runtime = started_runtime();
    let result = frozen_result(&runtime);
    let expected = serde_json::to_value(&result).unwrap();
    let key = result.operation.binding().operation_key();
    let digest = result.operation.intent().intent_digest().to_owned();
    runtime.last_player_drop_journal_result = Some(result);
    for _ in 0..3 {
        let observed = runtime.take_player_drop_journal_result().unwrap();
        assert_eq!(serde_json::to_value(observed).unwrap(), expected);
    }
    assert_guard(runtime.execute(WorldCommand::Tick).unwrap_err());
    assert!(runtime
        .acknowledge_player_drop_journal_result("wrong-operation", &digest)
        .is_err());
    assert_eq!(
        serde_json::to_value(runtime.take_player_drop_journal_result().unwrap()).unwrap(),
        expected
    );
    assert!(runtime
        .acknowledge_player_drop_journal_result(&key, "wrong-intent")
        .is_err());
    assert_eq!(
        serde_json::to_value(runtime.take_player_drop_journal_result().unwrap()).unwrap(),
        expected
    );
    assert_guard(runtime.execute(WorldCommand::Tick).unwrap_err());
    runtime
        .acknowledge_player_drop_journal_result(&key, &digest)
        .unwrap();
    assert!(runtime.take_player_drop_journal_result().is_none());
    runtime
        .execute(WorldCommand::Tick)
        .expect("exact publication ACK permits the next command");
}

#[test]
fn player_drop_pending_journal_blocks_all_source_save_callbacks_and_handoff() {
    let mut runtime = started_runtime();
    let handoff = runtime.prepare_character_handoff().unwrap();
    let result = frozen_result(&runtime);
    let checkpoint = runtime.inner.active_character_checkpoint().unwrap();
    let before_gold = checkpoint.gold;
    let before_revision = checkpoint.revision;
    let before_inventory = checkpoint.inventory_items_json.clone();
    let lease = ZoneOwnerLease::new(runtime.inventory_zone_id.clone(), "guard-fixture-owner", 1);
    let binding = ZoneSaveOperationBinding::new(
        "guard-save-operation".into(),
        "guard-rpc".into(),
        ZoneSaveFence::new(
            lease.zone_id().as_str().into(),
            lease.owner_id().into(),
            lease.fencing_token(),
        )
        .unwrap(),
    )
    .unwrap();
    let mut after = checkpoint.clone();
    after.revision += 1;
    let operation = ZoneSaveOperation::new(
        binding.clone(),
        "demo".into(),
        checkpoint.clone(),
        after,
        None,
    )
    .unwrap();
    runtime.last_player_drop_journal_result = Some(result);
    let errors = [
        runtime
            .prepare_zone_save_operation(binding.clone(), &checkpoint)
            .unwrap_err(),
        runtime.persist_zone_save_operation(&operation).unwrap_err(),
        runtime.release_zone_save_operation(&binding).unwrap_err(),
        runtime.save_active_character().unwrap_err(),
        runtime.save_active_character_for_logout().unwrap_err(),
        runtime
            .save_frozen_character_checkpoint_for_logout(&checkpoint)
            .unwrap_err(),
        runtime.prepare_teardown_checkpoint(&lease).unwrap_err(),
        runtime.release_teardown_fence().unwrap_err(),
        runtime.prepare_character_handoff().unwrap_err(),
        runtime.resume_character_handoff(&handoff).unwrap_err(),
    ];
    for error in errors {
        assert_guard(error);
    }
    let after = runtime.inner.active_character_checkpoint().unwrap();
    assert_eq!(after.gold, before_gold);
    assert_eq!(after.revision, before_revision);
    assert_eq!(after.inventory_items_json, before_inventory);
    assert!(runtime.take_player_drop_journal_result().is_some());
}

#[derive(Debug, Default)]
struct StandaloneDurableServiceProbe {
    publications: AtomicU64,
    legacy_commits: AtomicU64,
}
impl SharedAccountInventoryService for StandaloneDurableServiceProbe {
    fn supports_durable_player_drops(&self) -> bool {
        true
    }
    fn publish_durable_player_drop(
        &self,
        _runtime: &mut InProcessWorldRuntime,
        _binding: PlayerDropOperationBinding,
        _zone: &ZoneKey,
        _position: Point,
        _created_at_unix_ms: u64,
        _expires_at_unix_ms: u64,
        _request: PlayerDropRequest,
    ) -> Result<SharedPlayerDropCommit, PreparedPlayerDropPublicationFailure> {
        self.publications.fetch_add(1, Ordering::SeqCst);
        panic!("standalone runtime must reject before the durable publication service");
    }
    fn commit(
        &self,
        _runtime: &mut InProcessWorldRuntime,
        _envelope: SharedAccountInventoryCommandEnvelope,
    ) -> SharedAccountInventoryTransactionReceipt {
        self.legacy_commits.fetch_add(1, Ordering::SeqCst);
        panic!("standalone durable drop must not fall back to a legacy debit");
    }
}

#[test]
fn player_drop_standalone_durable_runtime_without_journal_rejects_before_debit_or_service() {
    let mut runtime = started_runtime();
    let probe = Arc::new(StandaloneDurableServiceProbe::default());
    runtime.account_inventory_service = probe.clone();
    runtime.economy_execution_context = Some(SharedAccountInventoryExecutionContext {
        zone_id: runtime.inventory_zone_id.clone(),
        fencing_generation: 1,
        source_sequence: 1,
        created_at_ms: 100,
        external_commit_authorized: true,
    });
    assert!(!runtime.player_drop_journal_capture_configured);
    let before = runtime.inner.active_character_checkpoint().unwrap();
    let error = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::DropGold {
            amount: 7,
        }))
        .unwrap_err();
    assert!(
        error.contains("ordered Zone Host journal publisher"),
        "{error}"
    );
    assert_eq!(probe.publications.load(Ordering::SeqCst), 0);
    assert_eq!(probe.legacy_commits.load(Ordering::SeqCst), 0);
    let after = runtime.inner.active_character_checkpoint().unwrap();
    assert_eq!(after.gold, before.gold);
    assert_eq!(after.revision, before.revision);
    assert_eq!(after.inventory_items_json, before.inventory_items_json);
    assert!(runtime.pending_player_drop_binding.is_none());
    assert!(runtime.take_player_drop_journal_result().is_none());
}
