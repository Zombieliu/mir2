//! In-process wire/journal fixtures. These do not exercise PostgreSQL or claim
//! durable source commits; the spy models the trusted runtime result boundary.
use super::*;
use crate::routing::ZoneOwnerLeaseAuthority;
use mir2_simulation::{
    PlayerDropCommitReceipt, PlayerDropEffectState, PlayerDropOperationBinding, PlayerDropOutcome,
    PlayerDropPreparedOperation, PlayerDropRequest, PlayerDropWorldIntent, SimulationConfig,
    SimulationSession, WorldRuntime, ZoneKey,
};

const RPC_SESSION: &str = "outer-gateway-rpc-session";
const SOURCE_SESSION: &str = "demo:0";
const TOKEN: &str = "player-drop-journal-token";
const SOURCE_SEQUENCE: u64 = 1;

fn server() -> (ZoneHostServer, ZoneOwnerLease) {
    let authority = Arc::new(crate::InMemoryZoneOwnerLeaseAuthority::new());
    let lease = authority.owner_lease(&ZoneId::primary());
    let server = ZoneHostServer::with_identity_and_factory(
        "player-drop-journal-host",
        8,
        GatewayConfig::default(),
        authority,
        Some(TOKEN.into()),
        ZoneRpcLimits::default(),
        Arc::new(SharedInProcessZoneRuntimeFactory::with_tick_cadences(
            Duration::from_secs(3600),
            BTreeMap::new(),
        )),
    );
    (server, lease)
}

fn envelope(lease: &ZoneOwnerLease, request: ZoneRpcRequest) -> ZoneRpcEnvelope {
    ZoneRpcEnvelope {
        protocol_version: ZONE_RPC_PROTOCOL_VERSION,
        session_id: RPC_SESSION.into(),
        zone_id: lease.zone_id().as_str().into(),
        auth_token: Some(TOKEN.into()),
        request,
    }
}

fn entry(lease: &ZoneOwnerLease, command: WireWorldCommand) -> WireHostJournalEntry {
    WireHostJournalEntry {
        sequence: 0,
        session_id: RPC_SESSION.into(),
        zone_id: lease.zone_id().as_str().into(),
        owner_lease: lease.into(),
        mode: WireZoneOwnerCommandMode::Direct,
        command: Some(command),
        closed: false,
        zone_tick_ms: None,
    }
}

fn drop_command() -> WireWorldCommand {
    WireWorldCommand::from_world(WorldCommand::ClientPacket(ClientPacket::DropGold {
        amount: 7,
    }))
    .unwrap()
}

fn seed_journal(server: &ZoneHostServer, lease: &ZoneOwnerLease) {
    server
        .append_journal(entry(lease, WireWorldCommand::Tick))
        .unwrap();
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

fn result(lease: &ZoneOwnerLease) -> PlayerDropJournalResult {
    let mut session = SimulationSession::new(SimulationConfig::default());
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let before = session.active_character_checkpoint().unwrap();
    let mut after = before.clone();
    after.revision += 1;
    after.gold -= 7;
    let binding = PlayerDropOperationBinding::new(
        "original-player-drop-operation".into(),
        SOURCE_SESSION.into(),
        "demo".into(),
        before.character.index,
        lease.zone_id().as_str().into(),
        lease.owner_id().into(),
        lease.fencing_token(),
        SOURCE_SEQUENCE,
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
    // Production construction is crate-private. Build this test-only complete
    // carrier using the public serialization contract and canonical digest.
    let source = canonical(serde_json::to_value((&intent, &before, &before, &after)).unwrap());
    let mut hash = Sha256::new();
    hash.update(b"mir2/player-drop/prepared-source/v1\0");
    hash.update(serde_json::to_vec(&source).unwrap());
    let operation: PlayerDropPreparedOperation = serde_json::from_value(serde_json::json!({
        "intent": intent,
        "before": before,
        "runtime_before": before,
        "checkpoint": after,
        "operation_digest": hex_lower_bytes(&hash.finalize()),
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
        "zone": operation.intent().zone(),
        "world_effect_id": operation.intent().world_effect_id(),
        "session_id": SOURCE_SESSION,
        "player_object_id": 1,
        "origin": operation.runtime_before().position,
        "position": operation.intent().position(),
        "kind": "Gold",
        "reserved_at_unix_ms": 100,
    }))
    .unwrap();
    let result = PlayerDropJournalResult {
        operation,
        receipt,
        request: PlayerDropRequest::Gold { amount: 7 },
        reservation: Some(reservation),
    };
    result.validate_integrity().unwrap();
    result
}

#[derive(Default)]
struct Calls {
    execute: AtomicUsize,
    apply: AtomicUsize,
    acknowledge: AtomicUsize,
    snapshot: AtomicUsize,
    reject_acknowledgement: AtomicBool,
}

struct JournalSpy {
    calls: Arc<Calls>,
    pending: Option<PlayerDropJournalResult>,
    executed_pending: bool,
    exhaust_on_execute: Option<Arc<Mutex<ZoneHostJournal>>>,
}

impl WorldRuntime for JournalSpy {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_connect(&self) -> Vec<ServerPacket> {
        Vec::new()
    }

    fn execute(&mut self, _command: WorldCommand) -> Result<Vec<ServerPacket>, String> {
        if self.executed_pending && self.pending.is_some() {
            return Err("PLAYER_DROP_COMMIT_OUTCOME_UNKNOWN: frozen publication is pending".into());
        }
        self.calls.execute.fetch_add(1, Ordering::Relaxed);
        self.executed_pending = self.pending.is_some();
        if let Some(journal) = &self.exhaust_on_execute {
            journal
                .lock()
                .unwrap()
                .replication
                .cursors
                .get_mut(ZoneId::primary().as_str())
                .unwrap()
                .next_sequence = u64::MAX;
        }
        Ok(Vec::new())
    }

    fn world_snapshot(&self) -> WorldSnapshot {
        self.calls.snapshot.fetch_add(1, Ordering::Relaxed);
        SimulationSession::new(SimulationConfig::default()).world_snapshot()
    }

    fn active_identity(&self) -> Option<ActiveSessionIdentity> {
        None
    }

    fn save_active_character(&mut self) -> Result<(), String> {
        panic!("frozen player-drop journal replay must not save a character")
    }

    fn refresh_active_external_mail(&mut self) -> bool {
        // This isolated journal spy has no active account or external mailbox.
        false
    }

    fn take_player_drop_journal_result(&mut self) -> Option<PlayerDropJournalResult> {
        self.pending.clone()
    }

    fn acknowledge_player_drop_journal_result(
        &mut self,
        operation_key: &str,
        intent_digest: &str,
    ) -> Result<(), String> {
        if self.calls.reject_acknowledgement.load(Ordering::Acquire) {
            return Err("fixture acknowledgement failed".into());
        }
        let pending = self
            .pending
            .as_ref()
            .ok_or("no pending player-drop result")?;
        if pending.receipt.operation_key() != operation_key
            || pending.receipt.intent_digest() != intent_digest
        {
            return Err("player-drop journal acknowledgement binding mismatch".into());
        }
        self.pending = None;
        self.executed_pending = false;
        self.calls.acknowledge.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn apply_player_drop_journal_result(
        &mut self,
        result: &PlayerDropJournalResult,
    ) -> Result<Vec<ServerPacket>, String> {
        result.validate_integrity()?;
        self.calls.apply.fetch_add(1, Ordering::Relaxed);
        Ok(Vec::new())
    }
}

fn install_spy(
    server: &ZoneHostServer,
    lease: &ZoneOwnerLease,
    pending: Option<PlayerDropJournalResult>,
    exhaust_on_execute: bool,
) -> Arc<Calls> {
    let calls = Arc::new(Calls::default());
    let hosted = Arc::new(HostedZoneOwnerCommandClient::new(Box::new(JournalSpy {
        calls: Arc::clone(&calls),
        pending,
        executed_pending: false,
        exhaust_on_execute: exhaust_on_execute.then(|| Arc::clone(&server.journal)),
    })));
    server.sessions.lock().unwrap().insert(
        (RPC_SESSION.into(), lease.zone_id().as_str().into()),
        Arc::new(ZoneHostSession::new(hosted, 8)),
    );
    calls
}

fn hosted(server: &ZoneHostServer, lease: &ZoneOwnerLease) -> Arc<HostedZoneOwnerCommandClient> {
    Arc::clone(
        &server
            .sessions
            .lock()
            .unwrap()
            .get(&(RPC_SESSION.into(), lease.zone_id().as_str().into()))
            .unwrap()
            .hosted,
    )
}

#[test]
fn frozen_result_peek_retains_guard_until_exact_publication_acknowledgement() {
    let (_, lease) = server();
    let result = result(&lease);
    let operation_key = result.receipt.operation_key().to_string();
    let intent_digest = result.receipt.intent_digest().to_string();
    let calls = Arc::new(Calls::default());
    let mut runtime = JournalSpy {
        calls: Arc::clone(&calls),
        pending: Some(result),
        executed_pending: true,
        exhaust_on_execute: None,
    };
    assert!(runtime.take_player_drop_journal_result().is_some());
    assert!(runtime.take_player_drop_journal_result().is_some());
    assert!(runtime
        .acknowledge_player_drop_journal_result("different-operation", &intent_digest)
        .is_err());
    assert!(runtime
        .acknowledge_player_drop_journal_result(&operation_key, "different-intent")
        .is_err());
    assert!(runtime.execute(WorldCommand::Tick).is_err());
    runtime
        .acknowledge_player_drop_journal_result(&operation_key, &intent_digest)
        .unwrap();
    assert!(runtime.take_player_drop_journal_result().is_none());
    runtime.execute(WorldCommand::Tick).unwrap();
    assert_eq!(calls.acknowledge.load(Ordering::Acquire), 1);
    assert_eq!(calls.execute.load(Ordering::Acquire), 1);
}

#[test]
fn frozen_player_drop_is_rejected_as_client_execute_before_runtime_or_journal() {
    let (server, lease) = server();
    let calls = install_spy(&server, &lease, None, false);
    let command = WireWorldCommand::PlayerDropJournalResult {
        result: Box::new(result(&lease)),
    };
    let encoded = serde_json::to_vec(&command).unwrap();
    let decoded: WireWorldCommand = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(decoded.into_world().unwrap_err().code, "checkpoint_command");
    let request = ZoneRpcRequest::Execute {
        owner_lease: (&lease).into(),
        mode: WireZoneOwnerCommandMode::Direct,
        command,
    };
    assert_eq!(
        server
            .handle_envelope(envelope(&lease, request))
            .unwrap_err()
            .code,
        "checkpoint_command"
    );
    assert_eq!(calls.execute.load(Ordering::Acquire), 0);
    assert_eq!(calls.apply.load(Ordering::Acquire), 0);
    assert!(server.journal.lock().unwrap().entries.is_empty());
}

#[test]
fn frozen_player_drop_binds_zone_lease_and_zone_sequence_not_outer_rpc_session() {
    let (_, lease) = server();
    let result = result(&lease);
    let original = entry(&lease, drop_command());
    assert_ne!(
        original.session_id,
        result.operation.binding().source_rpc_session()
    );
    original
        .validate_player_drop_result(&result, SOURCE_SEQUENCE)
        .unwrap();
    for kind in ["zone", "lease-zone", "owner", "epoch", "closed", "tick"] {
        let mut changed = original.clone();
        match kind {
            "zone" => changed.zone_id = "other-zone".into(),
            "lease-zone" => changed.owner_lease.zone_id = "other-zone".into(),
            "owner" => changed.owner_lease.owner_id = "other-owner".into(),
            "epoch" => changed.owner_lease.fencing_token += 1,
            "closed" => changed.closed = true,
            "tick" => changed.zone_tick_ms = Some(100),
            _ => unreachable!(),
        }
        assert_eq!(
            changed
                .validate_player_drop_result(&result, SOURCE_SEQUENCE)
                .unwrap_err()
                .code,
            "player_drop_journal_binding",
            "{kind}"
        );
    }
    assert!(original
        .validate_player_drop_result(&result, SOURCE_SEQUENCE + 1)
        .is_err());
    let mut changed = serde_json::to_value(&result).unwrap();
    changed["reservation"]["position"]["x"] = serde_json::json!(999);
    let changed: PlayerDropJournalResult = serde_json::from_value(changed).unwrap();
    assert!(original
        .validate_player_drop_result(&changed, SOURCE_SEQUENCE)
        .is_err());
    let mut changed = serde_json::to_value(&result).unwrap();
    changed["request"]["amount"] = serde_json::json!(8);
    let changed: PlayerDropJournalResult = serde_json::from_value(changed).unwrap();
    assert!(original
        .validate_player_drop_result(&changed, SOURCE_SEQUENCE)
        .is_err());
    let mut missing_request = serde_json::to_value(&result).unwrap();
    missing_request.as_object_mut().unwrap().remove("request");
    assert!(serde_json::from_value::<PlayerDropJournalResult>(missing_request).is_err());
}

#[test]
fn successful_host_drop_replaces_procedure_with_exact_frozen_result() {
    let (server, lease) = server();
    seed_journal(&server, &lease);
    let result = result(&lease);
    let expected = serde_json::to_value(&result).unwrap();
    let calls = install_spy(&server, &lease, Some(result), false);
    let request = ZoneRpcRequest::Execute {
        owner_lease: (&lease).into(),
        mode: WireZoneOwnerCommandMode::Direct,
        command: drop_command(),
    };
    assert!(matches!(
        server.handle_envelope(envelope(&lease, request)).unwrap(),
        ZoneRpcPayload::Execution { .. }
    ));
    let journal = server.journal.lock().unwrap();
    let frozen = journal.entries.last().unwrap();
    assert_eq!(calls.execute.load(Ordering::Acquire), 1);
    assert_eq!(calls.apply.load(Ordering::Acquire), 0);
    assert_eq!(frozen.session_id, RPC_SESSION);
    assert_eq!(frozen.sequence, SOURCE_SEQUENCE);
    let encoded = serde_json::to_vec(frozen).unwrap();
    let decoded: WireHostJournalEntry = serde_json::from_slice(&encoded).unwrap();
    let Some(WireWorldCommand::PlayerDropJournalResult { result }) = decoded.command else {
        panic!("journal must contain the frozen result")
    };
    assert_eq!(serde_json::to_value(result).unwrap(), expected);
    assert!(zone_replication_build_id().starts_with("mir2-zone/v5-durable-drop-admission/"));
    drop(journal);
    assert_eq!(calls.acknowledge.load(Ordering::Acquire), 1);
    assert!(hosted(&server, &lease)
        .take_player_drop_journal_result()
        .unwrap()
        .is_none());
    let next = ZoneRpcRequest::Execute {
        owner_lease: (&lease).into(),
        mode: WireZoneOwnerCommandMode::Direct,
        command: WireWorldCommand::Tick,
    };
    assert!(matches!(
        server.handle_envelope(envelope(&lease, next)).unwrap(),
        ZoneRpcPayload::Execution { .. }
    ));
    assert_eq!(calls.execute.load(Ordering::Acquire), 2);
}

#[test]
fn authenticated_digest_verified_batch_applies_frozen_result_without_execute() {
    let (source, lease) = server();
    seed_journal(&source, &lease);
    source
        .append_execution_journal(
            entry(&lease, drop_command()),
            SOURCE_SEQUENCE,
            Some(result(&lease)),
        )
        .unwrap();
    let batch = source
        .journal
        .lock()
        .unwrap()
        .export_batch(
            lease.zone_id().as_str(),
            SOURCE_SEQUENCE,
            8,
            DEFAULT_ZONE_RPC_MAX_FRAME_BYTES,
        )
        .unwrap();
    batch.verify().unwrap();
    let (replica, replica_lease) = server();
    seed_journal(&replica, &replica_lease);
    let calls = install_spy(&replica, &replica_lease, None, false);
    let mut unauthenticated = envelope(
        &lease,
        ZoneRpcRequest::ApplyMutationBatch {
            batch: batch.clone(),
        },
    );
    unauthenticated.auth_token = None;
    assert_eq!(
        replica.handle_envelope(unauthenticated).unwrap_err().code,
        "unauthorized"
    );
    let mut altered = batch.clone();
    let mut payload: serde_json::Value =
        serde_json::from_slice(&altered.entries[0].payload).unwrap();
    payload["ownerLease"]["ownerId"] = serde_json::json!("forged-owner");
    altered.entries[0].payload = serde_json::to_vec(&payload).unwrap();
    assert_eq!(
        replica
            .handle_envelope(envelope(
                &lease,
                ZoneRpcRequest::ApplyMutationBatch { batch: altered }
            ))
            .unwrap_err()
            .code,
        "mutation_batch_invalid"
    );
    assert_eq!(calls.apply.load(Ordering::Acquire), 0);
    assert!(matches!(
        replica
            .handle_envelope(envelope(
                &lease,
                ZoneRpcRequest::ApplyMutationBatch { batch }
            ))
            .unwrap(),
        ZoneRpcPayload::Unit
    ));
    assert_eq!(calls.apply.load(Ordering::Acquire), 1);
    assert_eq!(calls.execute.load(Ordering::Acquire), 0);
    assert_eq!(replica.runtime_execute_calls.load(Ordering::Acquire), 0);
}

#[test]
fn frozen_checkpoint_replay_dispatch_preserves_per_zone_binding() {
    let (_, lease) = server();
    let result = result(&lease);
    let frozen = entry(
        &lease,
        WireWorldCommand::PlayerDropJournalResult {
            result: Box::new(result),
        },
    );
    let (replica, _) = server();
    let calls = install_spy(&replica, &lease, None, false);
    let session = replica
        .sessions
        .lock()
        .unwrap()
        .get(&(RPC_SESSION.into(), lease.zone_id().as_str().into()))
        .unwrap()
        .clone();
    frozen
        .replay_session_mutation(&session, SOURCE_SEQUENCE, false)
        .unwrap();
    assert_eq!(calls.apply.load(Ordering::Acquire), 1);
    assert_eq!(calls.execute.load(Ordering::Acquire), 0);
    assert!(frozen
        .replay_session_mutation(&session, SOURCE_SEQUENCE + 1, false)
        .is_err());
    assert_eq!(calls.apply.load(Ordering::Acquire), 1);

    let other = ZoneOwnerLease::new(ZoneId::new("unrelated-zone"), "other-owner", 1);
    let mut journal = ZoneHostJournal::default();
    for mut value in [
        entry(&other, WireWorldCommand::Tick),
        entry(&lease, WireWorldCommand::Tick),
        frozen,
    ] {
        value.sequence = journal.entries.len() as u64;
        journal
            .replication
            .append(journal.entries.len(), &value)
            .unwrap();
        journal.entries.push(value);
    }
    assert_eq!(journal.entries.last().unwrap().sequence, 2);
    assert_eq!(
        ZoneReplicationCatalog::from_entries(&journal.entries)
            .unwrap()
            .head(lease.zone_id().as_str())
            .next_sequence,
        2
    );
}

#[test]
fn post_commit_host_append_failure_is_unknown_and_freezes_zone() {
    let (server, lease) = server();
    seed_journal(&server, &lease);
    let calls = install_spy(&server, &lease, Some(result(&lease)), true);
    let request = ZoneRpcRequest::Execute {
        owner_lease: (&lease).into(),
        mode: WireZoneOwnerCommandMode::Direct,
        command: drop_command(),
    };
    let error = server
        .handle_envelope(envelope(&lease, request))
        .unwrap_err();
    assert_eq!(error.code, "commit_outcome_unknown");
    assert!(error.message.contains("PLAYER_DROP_COMMIT_OUTCOME_UNKNOWN"));
    assert_eq!(calls.execute.load(Ordering::Acquire), 1);
    assert_eq!(calls.acknowledge.load(Ordering::Acquire), 0);
    assert_eq!(server.journal.lock().unwrap().entries.len(), 1);
    let hosted = hosted(&server, &lease);
    assert!(hosted.take_player_drop_journal_result().unwrap().is_some());
    let direct_error = hosted
        .execute_request(ZoneOwnerCommandRequest::direct(
            lease.clone(),
            WorldCommand::Tick,
        ))
        .unwrap_err();
    assert!(direct_error.contains("COMMIT_OUTCOME_UNKNOWN"));
    assert!(server
        .unknown_commit_frozen_zones
        .lock()
        .unwrap()
        .contains(lease.zone_id().as_str()));
    assert_eq!(
        server
            .require_known_zone_commit(lease.zone_id().as_str())
            .unwrap_err()
            .code,
        "commit_outcome_unknown"
    );
}

#[test]
fn post_commit_registration_refresh_failure_is_unknown_and_freezes_zone() {
    let (server, lease) = server();
    seed_journal(&server, &lease);
    let calls = install_spy(&server, &lease, Some(result(&lease)), false);
    let session = server
        .sessions
        .lock()
        .unwrap()
        .get(&(RPC_SESSION.into(), lease.zone_id().as_str().into()))
        .unwrap()
        .clone();
    let poison = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _registration = session.registration_refresh_gate.lock().unwrap();
        panic!("fixture registration refresh failure after runtime execution");
    }));
    assert!(poison.is_err());
    let request = ZoneRpcRequest::Execute {
        owner_lease: (&lease).into(),
        mode: WireZoneOwnerCommandMode::Direct,
        command: drop_command(),
    };
    let error = server
        .handle_envelope(envelope(&lease, request))
        .unwrap_err();
    assert_eq!(calls.execute.load(Ordering::Acquire), 1);
    assert_eq!(calls.acknowledge.load(Ordering::Acquire), 0);
    assert!(hosted(&server, &lease)
        .take_player_drop_journal_result()
        .unwrap()
        .is_some());
    assert_eq!(error.code, "commit_outcome_unknown");
    assert!(error.message.contains("PLAYER_DROP_COMMIT_OUTCOME_UNKNOWN"));
    assert!(error
        .message
        .contains("registration refresh mutex poisoned"));
    assert_eq!(server.journal.lock().unwrap().entries.len(), 1);
    assert_eq!(
        server
            .require_known_zone_commit(lease.zone_id().as_str())
            .unwrap_err()
            .code,
        "commit_outcome_unknown"
    );
}

#[test]
fn post_commit_acknowledgement_failure_is_unknown_and_retains_runtime_guard() {
    let (server, lease) = server();
    seed_journal(&server, &lease);
    let calls = install_spy(&server, &lease, Some(result(&lease)), false);
    calls.reject_acknowledgement.store(true, Ordering::Release);
    let request = ZoneRpcRequest::Execute {
        owner_lease: (&lease).into(),
        mode: WireZoneOwnerCommandMode::Direct,
        command: drop_command(),
    };
    let error = server
        .handle_envelope(envelope(&lease, request))
        .unwrap_err();
    assert_eq!(error.code, "commit_outcome_unknown");
    assert!(error.message.contains("journal acknowledgement failed"));
    assert_eq!(server.journal.lock().unwrap().entries.len(), 2);
    assert_eq!(calls.acknowledge.load(Ordering::Acquire), 0);
    assert!(hosted(&server, &lease)
        .take_player_drop_journal_result()
        .unwrap()
        .is_some());
    assert!(server
        .require_known_zone_commit(lease.zone_id().as_str())
        .is_err());
}

#[test]
fn unknown_zone_freeze_blocks_shared_tick_authorizer_before_world_mutation() {
    let (server, lease) = server();
    let factory = server.runtime_factory.lock().unwrap().clone();
    server.classify_zone_runtime_error(
        lease.zone_id().as_str(),
        "PLAYER_DROP_COMMIT_OUTCOME_UNKNOWN: missing frozen result".into(),
    );
    let error = factory
        .broadcast_world_event_message(lease.zone_id(), "0", "frozen fixture event", 100)
        .unwrap_err();
    assert!(error.contains("ZONE_COMMIT_OUTCOME_UNKNOWN"));
    assert!(server.journal.lock().unwrap().entries.is_empty());
    factory
        .broadcast_world_event_message(
            &ZoneId::new("healthy-zone"),
            "0",
            "healthy fixture event",
            100,
        )
        .unwrap();
    assert_eq!(server.journal.lock().unwrap().entries.len(), 1);
}

#[test]
fn source_of_truth_promotion_requires_canonical_recovery_before_snapshot_or_seed() {
    let (server, lease) = server();
    let calls = install_spy(&server, &lease, None, false);
    server.config.lock().unwrap().account_store_database_mode =
        mir2_simulation::AccountStoreDatabaseMode::SourceOfTruth;
    let error = server
        .activate_replica_session_persistence(lease.zone_id())
        .unwrap_err();
    assert_eq!(error.code, "canonical_recovery_required");
    assert_eq!(calls.snapshot.load(Ordering::Acquire), 0);
    assert_eq!(calls.execute.load(Ordering::Acquire), 0);
    assert_eq!(calls.acknowledge.load(Ordering::Acquire), 0);
    assert!(server.journal.lock().unwrap().entries.is_empty());
}

#[test]
fn ordinary_journal_entries_keep_legacy_canonical_bytes() {
    let (server, lease) = server();
    let ordinary = entry(&lease, WireWorldCommand::Tick);
    let expected = serde_json::to_vec(&ordinary).unwrap();
    server.append_execution_journal(ordinary, 0, None).unwrap();
    let journal = server.journal.lock().unwrap();
    assert_eq!(serde_json::to_vec(&journal.entries[0]).unwrap(), expected);
    assert!(!String::from_utf8(expected).unwrap().contains("playerDrop"));
}
