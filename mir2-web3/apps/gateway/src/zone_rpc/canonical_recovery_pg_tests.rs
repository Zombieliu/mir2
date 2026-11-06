//! Actual PostgreSQL Host/coordinator lifecycle checks using disposable DBs.
//! Commands enter the real in-process Host; these are not TCP fault tests.
use super::*;
use crate::economy::{
    PlayerDropPostgresTestDatabase, PostgresEconomyAccountInventoryService,
    start_test_runtime_with_config,
};
use crate::routing::{SharedAccountInventoryService, ZoneOwnerLeaseAuthority};
use crate::zone_lease::PostgresZoneOwnerLeaseAuthority;
use mir2_simulation::{
    GroundDropLootSnapshot, GroundDropSnapshot, InProcessWorldRuntime, PlayerDropCustodyState,
    PlayerDropOperationBinding, PlayerDropRequest, WorldRuntime, ZoneKey, ZoneManager,
};
use serde_json::{Value, json};

const ACCOUNT: &str = "host-drop-source";
const RPC_SESSION: &str = "host-drop-recovery-rpc";

pub(super) fn seeded_source(db: &PlayerDropPostgresTestDatabase) -> InProcessWorldRuntime {
    let mut runtime =
        start_test_runtime_with_config(db.config("owner-a", 7), ACCOUNT, "HostDropSource").unwrap();
    let seed = GroundDropSnapshot {
        object_id: 0x7f10_7005,
        name: "Gold".into(),
        name_colour_argb: -1,
        icon: 0,
        x: 0,
        y: 0,
        quantity: 100,
        source_monster: "owned Host recovery fixture".into(),
        owner_object_id: None,
        ownership_remaining_ticks: None,
        loot: GroundDropLootSnapshot::Gold { amount: 100 },
    };
    assert!(
        runtime
            .commit_shared_ground_drop_pickup_transaction(&seed)
            .committed
    );
    runtime.save_active_character().unwrap();
    runtime
}

pub(super) fn host(
    db: &PlayerDropPostgresTestDatabase,
    owner: &str,
    epoch: u64,
) -> (ZoneHostServer, Arc<PostgresZoneOwnerLeaseAuthority>) {
    let authority = Arc::new(PostgresZoneOwnerLeaseAuthority::new(
        db.url(),
        owner,
        120_000,
    ));
    let service = Arc::new(PostgresEconomyAccountInventoryService::new(db.url()));
    let factory =
        SharedInProcessZoneRuntimeFactory::with_tick_cadences_and_account_inventory_service(
            Duration::from_secs(3600),
            BTreeMap::new(),
            service,
        )
        .fresh_replica();
    let server = ZoneHostServer::with_identity_and_factory(
        owner,
        8,
        db.config(owner, epoch),
        authority.clone(),
        None,
        ZoneRpcLimits::default(),
        Arc::new(factory),
    );
    server.configure_zone_map_catalog(BTreeMap::new(), BTreeSet::from([db.zone().into()]));
    (server, authority)
}

fn envelope(zone: &ZoneId, request: ZoneRpcRequest) -> ZoneRpcEnvelope {
    ZoneRpcEnvelope {
        protocol_version: ZONE_RPC_PROTOCOL_VERSION,
        session_id: RPC_SESSION.into(),
        zone_id: zone.as_str().into(),
        auth_token: None,
        request,
    }
}

pub(super) fn execute(
    server: &ZoneHostServer,
    lease: &ZoneOwnerLease,
    command: WorldCommand,
) -> Result<ZoneRpcPayload, ZoneRpcFault> {
    server.handle_envelope(envelope(
        lease.zone_id(),
        ZoneRpcRequest::Execute {
            owner_lease: lease.into(),
            mode: WireZoneOwnerCommandMode::ProductionPlayer {
                authenticated: true,
            },
            command: WireWorldCommand::from_world(command).unwrap(),
        },
    ))
}

pub(super) fn snapshot(server: &ZoneHostServer, zone: &ZoneId) -> ZoneBaseSnapshot {
    let ZoneRpcPayload::BaseSnapshot { snapshot } = server.export_base_snapshot(zone).unwrap()
    else {
        panic!("Host base")
    };
    snapshot
}

pub(super) fn source_image(db: &PlayerDropPostgresTestDatabase, index: i32) -> Value {
    let mut client = db.connect();
    let row = client
        .query_one(
            "SELECT a.raw_json,a.store_version,s.snapshot_json,s.save_version FROM accounts a \
         JOIN character_saves s USING(account_id) WHERE a.account_id=$1 AND s.character_index=$2",
            &[&ACCOUNT, &index],
        )
        .unwrap();
    let ledger: Value = client.query_one(
        "SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY event_id),'[]'::jsonb) FROM game_economy_transactions t", &[],
    ).unwrap().get(0);
    let outbox: Value = client.query_one(
        "SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY event_id),'[]'::jsonb) FROM game_economy_outbox t", &[],
    ).unwrap().get(0);
    json!({ "account": row.get::<_,Value>(0), "account_version": row.get::<_,i64>(1),
        "save": row.get::<_,Value>(2), "save_version": row.get::<_,i64>(3),
        "ledger": ledger, "outbox": outbox })
}

pub(super) fn world(server: &ZoneHostServer, zone: &ZoneId) -> ZoneManager {
    let factory = server.runtime_factory.lock().unwrap().clone();
    let bytes = factory.zone_checkpoint_bytes(zone).unwrap();
    let checkpoint: Value = serde_json::from_slice(&bytes).unwrap();
    let encoded = checkpoint["zones"][zone.as_str()]["zoneManagerBytes"]
        .as_str()
        .unwrap();
    ZoneManager::restore_checkpoint(&BASE64_STANDARD.decode(encoded).unwrap()).unwrap()
}

fn assert_one_live_effect(server: &ZoneHostServer, zone: &ZoneId, effect: &str, amount: u32) {
    let manager = world(server, zone);
    let mut live = Vec::new();
    for key in manager.zone_keys() {
        for (id, custody) in manager.player_drop_custody_states_for_key(&key) {
            if let PlayerDropCustodyState::Live { object_id, .. } = custody {
                live.push((
                    id,
                    manager
                        .ground_drop_snapshot_for_key(&key, object_id)
                        .unwrap(),
                ));
            }
        }
    }
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].0, effect);
    assert_eq!(live[0].1.loot, GroundDropLootSnapshot::Gold { amount });
    assert_eq!(live[0].1.quantity, amount);
}

pub(super) fn promoted(
    server: &ZoneHostServer,
    authority: &PostgresZoneOwnerLeaseAuthority,
    zone: &ZoneId,
) -> ZoneOwnerLease {
    let lease = authority.try_owner_lease(zone).unwrap();
    let head = server
        .journal
        .lock()
        .unwrap()
        .replication
        .head(zone.as_str());
    let readiness = server
        .assess_promotion_readiness(zone, &head, unix_now_ms(), 5_000)
        .unwrap();
    assert!(readiness.ready, "{:?}", readiness.reason);
    server
        .promote_replica(zone, readiness.readiness_id.as_deref().unwrap(), &lease)
        .unwrap();
    lease
}

fn recovery_receipt(
    db: &PlayerDropPostgresTestDatabase,
    epoch: i64,
) -> crate::economy::player_drop::PlayerDropRecoveryReceipt {
    let id: String = db
        .connect()
        .query_one(
            "SELECT recovery_id FROM public.zone_player_drop_recovery_receipts \
         WHERE source_namespace=current_schema() AND zone_id=$1 AND owner_epoch=$2 \
         ORDER BY observed_at_ms DESC,recovery_id LIMIT 1",
            &[&db.zone(), &epoch],
        )
        .unwrap()
        .get(0);
    db.store()
        .lookup_zone_player_drop_recovery_receipt(db.zone(), &id)
        .unwrap()
        .unwrap()
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn canonical_recovery_host_post_commit_gap_recovers_source_world_and_reset_without_second_debit() {
    let db = PlayerDropPostgresTestDatabase::new();
    let seed = seeded_source(&db);
    let identity = seed.active_identity().unwrap();
    let zone = ZoneId::new(db.zone());
    let (source, source_authority) = host(&db, "owner-a", 7);
    let source_lease = source_authority.try_owner_lease(&zone).unwrap();
    execute(
        &source,
        &source_lease,
        WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: ACCOUNT.into(),
            password: ACCOUNT.into(),
        }),
    )
    .unwrap();
    execute(
        &source,
        &source_lease,
        WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: identity.character_index,
        }),
    )
    .unwrap();
    let old_base = snapshot(&source, &zone);
    let pre_head = source
        .journal
        .lock()
        .unwrap()
        .replication
        .head(zone.as_str());
    let (target, target_authority) = host(&db, "owner-b", 8);
    target
        .install_base_snapshot(zone.as_str(), &old_base)
        .unwrap();
    let before = source_image(&db, identity.character_index);
    let before_gold = before["save"]["gold"].as_u64().unwrap();
    source
        .test_player_drop_before_journal_failure
        .store(true, Ordering::Release);
    let failure = execute(
        &source,
        &source_lease,
        WorldCommand::ClientPacket(ClientPacket::DropGold { amount: 25 }),
    )
    .unwrap_err();
    assert_eq!(failure.code, "commit_outcome_unknown");
    assert!(replication_heads_match(
        &source
            .journal
            .lock()
            .unwrap()
            .replication
            .head(zone.as_str()),
        &pre_head
    ));
    let committed = source_image(&db, identity.character_index);
    assert_eq!(committed["save"]["gold"].as_u64(), Some(before_gold - 25));
    let records = db
        .store()
        .enumerate_zone_player_drop_operations(db.zone(), None, 256)
        .unwrap();
    assert_eq!(records.len(), 1);
    let effect = records[0]
        .operation()
        .intent()
        .world_effect_id()
        .to_string();
    db.lease("owner-b", 8);
    let successor = promoted(&target, target_authority.as_ref(), &zone);
    assert_eq!(
        source_image(&db, identity.character_index),
        committed,
        "canonical recovery must not replay debit, save, ledger or outbox publication"
    );
    assert_one_live_effect(&target, &zone, &effect, 25);
    let receipt = recovery_receipt(&db, 8);
    let recovered = receipt.host_heads().unwrap().base_snapshot().unwrap();
    let head = target
        .journal
        .lock()
        .unwrap()
        .replication
        .head(zone.as_str());
    assert_eq!(
        head.base_snapshot_id.as_deref(),
        Some(recovered.snapshot_id.as_str())
    );
    assert_eq!(head.next_sequence, pre_head.next_sequence + 1);
    assert!(matches!(
        &target
            .journal
            .lock()
            .unwrap()
            .entries
            .last()
            .unwrap()
            .command,
        Some(WireWorldCommand::CanonicalZoneRecovery { .. })
    ));
    assert!(execute(&source, &source_lease, WorldCommand::Tick).is_err());
    assert!(
        source_authority
            .validate_owner_lease(&source_lease)
            .is_err()
    );
    execute(&target, &successor, WorldCommand::Tick).unwrap();
    let session = target
        .sessions
        .lock()
        .unwrap()
        .get(&(RPC_SESSION.into(), db.zone().into()))
        .unwrap()
        .clone();
    assert_eq!(
        session
            .hosted
            .active_character_checkpoint()
            .unwrap()
            .unwrap()
            .gold,
        (before_gold - 25) as u32
    );
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn canonical_recovery_cold_host_without_original_session_reuses_durable_base_and_effect_identity() {
    let db = PlayerDropPostgresTestDatabase::new();
    let mut runtime = seeded_source(&db);
    let identity = runtime.active_identity().unwrap();
    let before = runtime.active_character_checkpoint().unwrap();
    let zone = ZoneId::new(db.zone());
    let binding = PlayerDropOperationBinding::new(
        "cold-host-source-drop".into(),
        "departed-source-session".into(),
        ACCOUNT.into(),
        identity.character_index,
        db.zone().into(),
        "owner-a".into(),
        7,
        41,
    )
    .unwrap();
    let now = unix_now_ms();
    let service = PostgresEconomyAccountInventoryService::new(db.url());
    let committed = service
        .publish_durable_player_drop(
            &mut runtime,
            binding,
            &ZoneKey::for_map(&before.map_file_name),
            before.position.clone(),
            now,
            now + 30 * 60 * 1000,
            PlayerDropRequest::Gold { amount: 25 },
        )
        .unwrap();
    let effect = committed.operation.intent().world_effect_id().to_string();
    let durable = source_image(&db, identity.character_index);
    assert_eq!(
        durable["save"]["gold"].as_u64(),
        Some(u64::from(before.gold - 25))
    );
    let empty = ZoneBaseSnapshot::new(
        db.zone().into(),
        zone_replication_build_id(),
        0,
        "0".repeat(64),
        SharedInProcessZoneRuntimeFactory::new()
            .checkpoint_bytes()
            .unwrap(),
        Vec::new(),
    )
    .unwrap();
    let (target, authority) = host(&db, "owner-b", 8);
    target.install_base_snapshot(db.zone(), &empty).unwrap();
    assert_eq!(target.session_count(), 0);
    db.lease("owner-b", 8);
    promoted(&target, authority.as_ref(), &zone);
    assert_eq!(target.session_count(), 0);
    assert_one_live_effect(&target, &zone, &effect, 25);
    assert_eq!(source_image(&db, identity.character_index), durable);
    let receipt = recovery_receipt(&db, 8);
    let recovered = receipt.host_heads().unwrap().base_snapshot().unwrap();
    assert_eq!(recovered.session_count, 0);
    let reset = target
        .journal
        .lock()
        .unwrap()
        .export_batch(
            db.zone(),
            recovered.base_sequence,
            8,
            DEFAULT_ZONE_RPC_MAX_FRAME_BYTES,
        )
        .unwrap();
    let previous_head = target.journal.lock().unwrap().replication.head(db.zone());
    let (cold, cold_authority) = host(&db, "owner-c", 9);
    cold.install_base_snapshot(db.zone(), &recovered).unwrap();
    cold.handle_envelope(envelope(
        &zone,
        ZoneRpcRequest::ApplyMutationBatch { batch: reset },
    ))
    .unwrap();
    assert!(replication_heads_match(
        &cold.journal.lock().unwrap().replication.head(db.zone()),
        &previous_head
    ));
    db.lease("owner-c", 9);
    promoted(&cold, cold_authority.as_ref(), &zone);
    assert_eq!(cold.session_count(), 0);
    assert_one_live_effect(&cold, &zone, &effect, 25);
    assert_eq!(source_image(&db, identity.character_index), durable);
    assert_eq!(
        db.store()
            .enumerate_zone_player_drop_operations(db.zone(), None, 256)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(recovery_receipt(&db, 9).owner_epoch(), 9);
}
