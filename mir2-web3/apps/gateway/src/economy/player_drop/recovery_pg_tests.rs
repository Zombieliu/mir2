//! Uses only this module's strictly owned disposable database helper.
use super::super::recovery::tests::fixture_heads;
use super::*;
use mir2_simulation::{PlayerDropWorldIntent, WorldRuntime};

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_postgres_recovery_retains_exact_base_without_original_session_and_rejects_stale_source_or_head()
 {
    let db = Database::new();
    let store = db.store();
    let mut runtime = crate::economy::tests::start_test_runtime_with_config(
        db.config("owner-a", 7),
        "drop-source",
        "DropSource",
    )
    .unwrap();
    let seed = mir2_simulation::GroundDropSnapshot {
        object_id: 0x7f007021,
        name: "Gold".into(),
        name_colour_argb: -1,
        icon: 0,
        x: 0,
        y: 0,
        quantity: 100,
        source_monster: "owned recovery fixture".into(),
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
    let identity = runtime.active_identity().unwrap();
    let before = runtime.active_character_checkpoint().unwrap();
    let binding = PlayerDropOperationBinding::new(
        "recovery-live".into(),
        "old-source-rpc".into(),
        identity.account_id.clone(),
        identity.character_index,
        db.zone.clone(),
        "owner-a".into(),
        7,
        41,
    )
    .unwrap();
    let zone = mir2_simulation::ZoneKey {
        shard_id: "test".into(),
        map_file_name: before.map_file_name.clone(),
        channel_id: 1,
        instance_id: String::new(),
    };
    let created = now_ms();
    PostgresEconomyAccountInventoryService::new(&db.url)
        .publish_durable_player_drop(
            &mut runtime,
            binding.clone(),
            &zone,
            before.position.clone(),
            created,
            created + 30 * 60 * 1000,
            PlayerDropRequest::Gold { amount: 25 },
        )
        .unwrap();
    let old = runtime.zone_economy_write_permit().unwrap();
    let live = store
        .lookup_player_drop_operation(&binding)
        .unwrap()
        .unwrap();
    store
        .acknowledge_player_drop_effect(&old, &live, 41)
        .unwrap();
    drop(runtime); // Neither recovery nor its proof needs the source Session.
    db.lease("owner-b", 8);
    let config = db.config("owner-b", 8);
    let permit = config.zone_economy_write_permit().unwrap();
    let source = config
        .prepare_zone_source_recovery(&[identity.account_id.clone()])
        .unwrap();
    let expected = store
        .read_zone_player_drop_recovery_set(&db.zone, 512)
        .unwrap();
    assert_eq!(
        expected.records().len(),
        1,
        "ACKed live custody is included"
    );
    let heads = fixture_heads(&db.zone, "owner-b");
    assert!(
        store
            .commit_zone_player_drop_recovery(
                &permit,
                &source,
                &expected,
                &heads,
                "recovery-live",
                42,
                5000
            )
            .unwrap_err()
            .contains("observation")
    );
    store
        .acknowledge_player_drop_effect(&permit, &live, 42)
        .unwrap();
    let receipt = store
        .commit_zone_player_drop_recovery(
            &permit,
            &source,
            &expected,
            &heads,
            "recovery-live",
            42,
            5000,
        )
        .unwrap();
    let cold = PostgresEconomyStore::new(&db.url)
        .lookup_zone_player_drop_recovery_receipt(&db.zone, "recovery-live")
        .unwrap()
        .unwrap();
    assert_eq!(cold.receipt_digest(), receipt.receipt_digest());
    assert_eq!(cold.base_snapshot_bytes(), heads.base_snapshot_bytes());
    assert_eq!(
        cold.host_heads().unwrap().base_snapshot_id(),
        heads.base_snapshot_id()
    );
    store
        .validate_zone_player_drop_recovery_receipt(&permit, &source, &cold, &heads)
        .unwrap();
    let duplicate = store
        .commit_zone_player_drop_recovery(
            &permit,
            &source,
            &expected,
            &heads,
            "recovery-live",
            42,
            5000,
        )
        .unwrap();
    assert_eq!(duplicate.receipt_digest(), receipt.receipt_digest());
    assert_eq!(
        store
            .balance(&EconomyBalanceKey::gold(
                &identity.account_id,
                identity.character_index
            ))
            .unwrap(),
        i64::from(before.gold - 25)
    );
    assert!(db.connect().execute("UPDATE public.zone_player_drop_recovery_receipts SET base_snapshot_bytes='tamper'::bytea WHERE recovery_id=$1",&[&"recovery-live"]).is_err());
    let mut changed_head = heads.pre_recovery_head().clone();
    changed_head.oldest_available_sequence += 1;
    let changed = PlayerDropRecoveryHostHeads::new(
        "owner-b".into(),
        changed_head,
        heads.base_snapshot().unwrap(),
    )
    .unwrap();
    assert!(
        store
            .validate_zone_player_drop_recovery_receipt(&permit, &source, &cold, &changed)
            .is_err()
    );
    db.connect()
        .execute(
            "UPDATE accounts SET store_version=store_version+1 WHERE account_id=$1",
            &[&identity.account_id],
        )
        .unwrap();
    assert!(
        store
            .validate_zone_player_drop_recovery_receipt(&permit, &source, &cold, &heads)
            .is_err(),
        "unchanged JSON with a newer CAS version invalidates the proof"
    );
    let newer = config
        .prepare_zone_source_recovery(&[identity.account_id])
        .unwrap();
    assert_ne!(newer.source_digest(), cold.source_digest());
    assert!(
        store
            .validate_zone_player_drop_recovery_receipt(&permit, &newer, &cold, &heads)
            .is_err()
    );
    db.lease("owner-c", 9);
    assert!(
        store
            .validate_zone_player_drop_recovery_receipt(&permit, &source, &cold, &heads)
            .is_err()
    );
    assert!(
        store
            .lookup_zone_player_drop_recovery_receipt(&db.zone, "recovery-live")
            .unwrap()
            .is_some(),
        "historical proof stays readable after fencing"
    );
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_postgres_recovery_commit_unknown_keeps_durable_base_but_freezes_all_new_writes() {
    let db = Database::new();
    let store = db.store();
    let config = db.config("owner-a", 7);
    let permit = config.zone_economy_write_permit().unwrap();
    let source = config.prepare_zone_source_recovery(&[]).unwrap();
    let expected = store
        .read_zone_player_drop_recovery_set(&db.zone, 256)
        .unwrap();
    let heads = fixture_heads(&db.zone, "owner-a");
    let error = store
        .commit_recovery_losing_acknowledgement(
            &permit,
            &source,
            &expected,
            &heads,
            "recovery-unknown",
            42,
            5000,
        )
        .unwrap_err();
    assert!(error.contains("COMMIT_OUTCOME_UNKNOWN"));
    let record = PostgresEconomyStore::new(&db.url)
        .lookup_zone_player_drop_recovery_receipt(&db.zone, "recovery-unknown")
        .unwrap()
        .unwrap();
    assert_eq!(record.base_snapshot_bytes(), heads.base_snapshot_bytes());
    assert_eq!(record.operation_count(), 0);
    assert!(
        store
            .commit_zone_player_drop_recovery(
                &permit,
                &source,
                &expected,
                &heads,
                "recovery-unknown",
                42,
                5000
            )
            .is_err()
    );
    assert!(
        store
            .validate_zone_player_drop_recovery_receipt(&permit, &source, &record, &heads)
            .is_err(),
        "an exact historical receipt cannot clear the source freeze"
    );
    let cloned = config.clone().zone_economy_write_permit().unwrap();
    let mut client = db.connect();
    let mut tx = client.transaction().unwrap();
    assert!(
        cloned
            .lock_and_validate(&mut tx, &db.url, &db.zone, 7)
            .is_err()
    );
    drop(tx);
    assert_eq!(
        db.connect()
            .query_one(
                "SELECT count(*) FROM public.zone_player_drop_recovery_receipts",
                &[]
            )
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    assert_eq!(
        db.connect()
            .query_one("SELECT count(*) FROM game_economy_transactions", &[])
            .unwrap()
            .get::<_, i64>(0),
        0
    );
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_postgres_recovery_certifies_successor_abort_and_database_expiry_without_asset_mutation()
 {
    let db = Database::new();
    let store = db.store();
    let mut runtime = crate::economy::tests::start_test_runtime_with_config(
        db.config("owner-a", 7),
        "drop-source",
        "DropSource",
    )
    .unwrap();
    let seed = mir2_simulation::GroundDropSnapshot {
        object_id: 0x7f007022,
        name: "Gold".into(),
        name_colour_argb: -1,
        icon: 0,
        x: 0,
        y: 0,
        quantity: 100,
        source_monster: "owned recovery fixture".into(),
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
    let identity = runtime.active_identity().unwrap();
    let before = runtime.active_character_checkpoint().unwrap();
    let binding = |nonce: &str, sequence| {
        PlayerDropOperationBinding::new(
            nonce.into(),
            "old-source-rpc".into(),
            identity.account_id.clone(),
            identity.character_index,
            db.zone.clone(),
            "owner-a".into(),
            7,
            sequence,
        )
        .unwrap()
    };
    let zone = mir2_simulation::ZoneKey {
        shard_id: "test".into(),
        map_file_name: before.map_file_name.clone(),
        channel_id: 1,
        instance_id: String::new(),
    };
    // Preserve Crystal's exact 30-minute lifetime while seeding a historical
    // effect that is already due according to the real database clock.
    let created = now_ms() - 31 * 60 * 1000;
    let expired_binding = binding("recovery-due", 41);
    PostgresEconomyAccountInventoryService::new(&db.url)
        .publish_durable_player_drop(
            &mut runtime,
            expired_binding.clone(),
            &zone,
            before.position.clone(),
            created,
            created + 30 * 60 * 1000,
            PlayerDropRequest::Gold { amount: 25 },
        )
        .unwrap();
    let prepared_binding = binding("recovery-prepared", 42);
    let original = runtime.zone_economy_write_permit().unwrap();
    let prepared_created = now_ms();
    let prepared: Result<((), Vec<ServerPacket>), PreparedPlayerDropPublicationFailure> = runtime
        .commit_shared_player_drop_via_postgres(
            prepared_binding.clone(),
            &zone,
            before.position.clone(),
            prepared_created,
            prepared_created + 30 * 60 * 1000,
            PlayerDropRequest::Gold { amount: 10 },
            |source| {
                store
                    .prepare_player_drop_operation(&original, source)
                    .unwrap();
                Err(PreparedPlayerDropPublicationFailure::Rejected(
                    "owned fixture stops before source debit".into(),
                ))
            },
        );
    assert!(matches!(
        prepared,
        Err(PreparedPlayerDropPublicationFailure::Rejected(_))
    ));
    let canonical: serde_json::Value = db
        .connect()
        .query_one(
            "SELECT raw_json FROM accounts WHERE account_id=$1",
            &[&identity.account_id],
        )
        .unwrap()
        .get(0);
    drop(runtime);
    db.lease("owner-b", 8);
    let config = db.config("owner-b", 8);
    let permit = config.zone_economy_write_permit().unwrap();
    store
        .abort_prepared_player_drop_operation(
            &permit,
            &prepared_binding,
            "recovery-retire",
            "fenced original writer before publication",
        )
        .unwrap();
    let source = config
        .prepare_zone_source_recovery(&[identity.account_id.clone()])
        .unwrap();
    let heads = fixture_heads(&db.zone, "owner-b");
    let due = store
        .read_zone_player_drop_recovery_set(&db.zone, 256)
        .unwrap();
    assert!(
        store
            .commit_zone_player_drop_recovery(
                &permit,
                &source,
                &due,
                &heads,
                "recovery-retire",
                43,
                5000
            )
            .unwrap_err()
            .contains("database expiry")
    );
    let record = store
        .lookup_player_drop_operation(&expired_binding)
        .unwrap()
        .unwrap();
    store.expire_player_drop_effect(&permit, &record).unwrap();
    let expired = store
        .lookup_player_drop_operation(&expired_binding)
        .unwrap()
        .unwrap();
    assert_eq!(expired.status(), PlayerDropOperationStatus::Expired);
    store
        .acknowledge_player_drop_effect(&permit, &expired, 43)
        .unwrap();
    let final_set = store
        .read_zone_player_drop_recovery_set(&db.zone, 256)
        .unwrap();
    let receipt = store
        .commit_zone_player_drop_recovery(
            &permit,
            &source,
            &final_set,
            &heads,
            "recovery-retire",
            43,
            5000,
        )
        .unwrap();
    assert_eq!(receipt.operation_count(), 2);
    store
        .validate_zone_player_drop_recovery_receipt(&permit, &source, &receipt, &heads)
        .unwrap();
    assert_eq!(
        store
            .balance(&EconomyBalanceKey::gold(
                &identity.account_id,
                identity.character_index
            ))
            .unwrap(),
        i64::from(before.gold - 25)
    );
    assert_eq!(
        db.connect()
            .query_one(
                "SELECT raw_json FROM accounts WHERE account_id=$1",
                &[&identity.account_id]
            )
            .unwrap()
            .get::<_, serde_json::Value>(0),
        canonical
    );
    assert_eq!(
        db.connect()
            .query_one("SELECT count(*) FROM game_economy_transactions", &[])
            .unwrap()
            .get::<_, i64>(0),
        1
    );
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_postgres_recovery_reads_all_pages_and_never_certifies_truncated_or_changed_prepared_set()
 {
    let db = Database::new();
    let store = db.store();
    let config = db.config("owner-a", 7);
    let permit = config.zone_economy_write_permit().unwrap();
    let source = config.prepare_zone_source_recovery(&[]).unwrap();
    let empty = store
        .read_zone_player_drop_recovery_set(&db.zone, 512)
        .unwrap();
    let template = super::super::tests::fixture();
    let mut client = db.connect();
    let mut tx = client.transaction().unwrap();
    for ordinal in 0..257 {
        let binding = PlayerDropOperationBinding::new(
            format!("pagination-{ordinal}"),
            "old-source-rpc".into(),
            template.binding().account_id().into(),
            template.binding().character_index(),
            db.zone.clone(),
            "owner-a".into(),
            7,
            ordinal + 1,
        )
        .unwrap();
        let intent = PlayerDropWorldIntent::new(
            binding,
            template.intent().zone().clone(),
            template.intent().position().clone(),
            template.intent().created_at_unix_ms(),
            template.intent().expires_at_unix_ms(),
            template.intent().outcome().clone(),
        )
        .unwrap();
        let mut value = serde_json::to_value(&template).unwrap();
        value["intent"] = serde_json::to_value(&intent).unwrap();
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
        let bytes = serde_json::to_vec(&canonical(
            serde_json::to_value((
                &intent,
                template.before(),
                template.runtime_before(),
                template.checkpoint(),
            ))
            .unwrap(),
        ))
        .unwrap();
        let mut hash = Sha256::new();
        hash.update(b"mir2/player-drop/prepared-source/v1\0");
        hash.update(bytes);
        value["operation_digest"] = hex_lower(&hash.finalize()).into();
        let operation: PlayerDropPreparedOperation = serde_json::from_value(value.clone()).unwrap();
        operation.validate_integrity().unwrap();
        tx.execute("INSERT INTO public.zone_player_drop_operations(operation_key,world_effect_id,zone_id,owner_id,owner_epoch,operation_digest,intent_digest,operation,expires_at_ms,status,prepared_at_ms) VALUES($1,$2,$3,'owner-a',7,$4,$5,$6,$7,'Prepared',1)",
            &[&operation.binding().operation_key(),&intent.world_effect_id(),&db.zone,&operation.operation_digest(),&intent.intent_digest(),&value,&(intent.expires_at_unix_ms() as i64)]).unwrap();
    }
    tx.commit().unwrap();
    assert_eq!(
        store
            .read_zone_player_drop_recovery_set(&db.zone, 512)
            .unwrap()
            .records()
            .len(),
        257
    );
    assert!(
        store
            .read_zone_player_drop_recovery_set(&db.zone, 256)
            .unwrap_err()
            .contains("total bound")
    );
    assert!(
        store
            .commit_zone_player_drop_recovery(
                &permit,
                &source,
                &empty,
                &fixture_heads(&db.zone, "owner-a"),
                "changed-set",
                42,
                5000
            )
            .unwrap_err()
            .contains("custody changed")
    );
    assert!(
        store
            .lookup_zone_player_drop_recovery_receipt(&db.zone, "changed-set")
            .unwrap()
            .is_none()
    );
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_postgres_recovery_accepts_deleted_terminal_actors_only_with_completed_delivery_and_roster_absence()
 {
    use mir2_protocol::ClientPacket;
    use mir2_simulation::WorldCommand;
    let db = Database::new();
    let config = db.config("owner-a", 7);
    let store = db.store();
    let service = PostgresEconomyAccountInventoryService::new(&db.url);
    let mut runtime = crate::economy::tests::start_test_runtime_with_config(
        config.clone(),
        "drop-source",
        "DropSource",
    )
    .unwrap();
    let mut recipient = crate::economy::tests::start_test_runtime_with_config(
        config,
        "drop-recipient",
        "DropRecipient",
    )
    .unwrap();
    let seed = mir2_simulation::GroundDropSnapshot {
        object_id: 0x7f007023,
        name: "Gold".into(),
        name_colour_argb: -1,
        icon: 0,
        x: 0,
        y: 0,
        quantity: 100,
        source_monster: "owned retirement fixture".into(),
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
    let identity = runtime.active_identity().unwrap();
    let recipient_identity = recipient.active_identity().unwrap();
    let before = runtime.active_character_checkpoint().unwrap();
    let binding = PlayerDropOperationBinding::new(
        "retired-actors".into(),
        "old-source-rpc".into(),
        identity.account_id.clone(),
        identity.character_index,
        db.zone.clone(),
        "owner-a".into(),
        7,
        41,
    )
    .unwrap();
    let zone = mir2_simulation::ZoneKey {
        shard_id: "test".into(),
        map_file_name: before.map_file_name.clone(),
        channel_id: 1,
        instance_id: String::new(),
    };
    let created = now_ms();
    service
        .publish_durable_player_drop(
            &mut runtime,
            binding.clone(),
            &zone,
            before.position.clone(),
            created,
            created + 30 * 60 * 1000,
            PlayerDropRequest::Gold { amount: 25 },
        )
        .unwrap();
    let original = runtime.zone_economy_write_permit().unwrap();
    let context = SharedAccountInventoryExecutionContext {
        zone_id: crate::ZoneId::new(&db.zone),
        fencing_generation: 7,
        source_sequence: 42,
        created_at_ms: now_ms(),
        external_commit_authorized: true,
    };
    store
        .bootstrap_character_fenced(
            &original,
            &context,
            &recipient_identity,
            &recipient.world_snapshot(),
        )
        .unwrap();
    let live = store
        .lookup_player_drop_operation(&binding)
        .unwrap()
        .unwrap();
    let mut pickup =
        super::super::tests::pickup(live.operation(), &recipient_identity.account_id, 701);
    pickup.identity = recipient_identity.clone();
    let SharedAccountInventoryCommand::PlayerDropClaimPickup {
        drop: pickup_drop, ..
    } = &mut pickup.command
    else {
        unreachable!()
    };
    pickup_drop.x = before.position.x;
    pickup_drop.y = before.position.y;
    let envelope = economy_transaction_for_command(&context, &pickup, None).unwrap();
    let credit = store.transact_fenced(&original, &envelope).unwrap();
    service.reconcile_ground_drop_projections_fenced(&mut recipient, Some(&context));
    assert!(
        store
            .pending_ground_drop_projections(&recipient_identity)
            .unwrap()
            .is_empty()
    );
    let delivery=db.connect().query_one("SELECT status,projected_at_ms FROM game_economy_ground_drop_projections WHERE event_id=$1",&[&credit.event_id]).unwrap();
    assert_eq!(delivery.get::<_, String>(0), "projected");
    assert!(
        delivery
            .get::<_, Option<i64>>(1)
            .is_some_and(|time| time > 0)
    );
    for (actor, index) in [
        (&mut runtime, identity.character_index),
        (&mut recipient, recipient_identity.character_index),
    ] {
        actor
            .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
            .unwrap();
        assert!(
            actor
                .execute(WorldCommand::ClientPacket(ClientPacket::DeleteCharacter {
                    character_index: index
                }))
                .unwrap()
                .contains(&ServerPacket::DeleteCharacterSuccess {
                    character_index: index
                })
        );
    }
    drop(runtime);
    drop(recipient);
    db.lease("owner-b", 8);
    let config = db.config("owner-b", 8);
    let permit = config.zone_economy_write_permit().unwrap();
    let accounts = vec![
        identity.account_id.clone(),
        recipient_identity.account_id.clone(),
    ];
    let source = config.prepare_zone_source_recovery(&accounts).unwrap();
    assert!(
        source
            .is_retired_character(&identity.account_id, identity.character_index)
            .unwrap()
    );
    assert!(
        source
            .is_retired_character(
                &recipient_identity.account_id,
                recipient_identity.character_index
            )
            .unwrap()
    );
    assert!(source.character_keys().is_empty());
    let terminal = store
        .lookup_player_drop_operation(&binding)
        .unwrap()
        .unwrap();
    assert_eq!(terminal.status(), PlayerDropOperationStatus::Consumed);
    store
        .acknowledge_player_drop_effect(&permit, &terminal, 43)
        .unwrap();
    let expected = store
        .read_zone_player_drop_recovery_set(&db.zone, 256)
        .unwrap();
    let heads = fixture_heads(&db.zone, "owner-b");
    let canonical: Vec<serde_json::Value> = db
        .connect()
        .query(
            "SELECT raw_json FROM accounts WHERE account_id=ANY($1) ORDER BY account_id",
            &[&accounts],
        )
        .unwrap()
        .into_iter()
        .map(|row| row.get(0))
        .collect();
    let receipt = store
        .commit_zone_player_drop_recovery(
            &permit,
            &source,
            &expected,
            &heads,
            "retired-actors",
            43,
            5000,
        )
        .unwrap();
    store
        .validate_zone_player_drop_recovery_receipt(&permit, &source, &receipt, &heads)
        .unwrap();
    assert_eq!(
        db.connect()
            .query(
                "SELECT raw_json FROM accounts WHERE account_id=ANY($1) ORDER BY account_id",
                &[&accounts]
            )
            .unwrap()
            .into_iter()
            .map(|row| row.get::<_, serde_json::Value>(0))
            .collect::<Vec<_>>(),
        canonical
    );
    assert_eq!(
        db.connect()
            .query_one(
                "SELECT count(*) FROM character_saves WHERE account_id=ANY($1)",
                &[&accounts]
            )
            .unwrap()
            .get::<_, i64>(0),
        0,
        "recovery never recreates deleted actors"
    );
    // A pending or missing projection row cannot be hidden by roster absence.
    db.connect().execute("UPDATE game_economy_ground_drop_projections SET status='pending',projected_at_ms=NULL WHERE event_id=$1",&[&credit.event_id]).unwrap();
    assert!(
        store
            .validate_zone_player_drop_recovery_receipt(&permit, &source, &receipt, &heads)
            .unwrap_err()
            .contains("delivery")
    );
    db.connect()
        .execute(
            "DELETE FROM game_economy_ground_drop_projections WHERE event_id=$1",
            &[&credit.event_id],
        )
        .unwrap();
    assert!(
        store
            .validate_zone_player_drop_recovery_receipt(&permit, &source, &receipt, &heads)
            .unwrap_err()
            .contains("delivery proof missing")
    );
    // Corrupt fixture evidence: the roster still names the actor, but both save
    // representations are absent. This is never an authorized retirement.
    let mut bad: serde_json::Value = db
        .connect()
        .query_one(
            "SELECT raw_json FROM accounts WHERE account_id=$1",
            &[&identity.account_id],
        )
        .unwrap()
        .get(0);
    bad["characters"] = serde_json::to_value(vec![before.character]).unwrap();
    db.connect()
        .execute(
            "UPDATE accounts SET raw_json=$2,store_version=store_version+1 WHERE account_id=$1",
            &[&identity.account_id, &bad],
        )
        .unwrap();
    if let Ok(inconsistent) = config.prepare_zone_source_recovery(&accounts) {
        assert!(
            inconsistent
                .is_retired_character(&identity.account_id, identity.character_index)
                .is_err()
        );
        assert!(
            store
                .commit_zone_player_drop_recovery(
                    &permit,
                    &inconsistent,
                    &expected,
                    &heads,
                    "unexplained-absence",
                    43,
                    5000
                )
                .is_err()
        );
    }
    assert!(
        store
            .lookup_zone_player_drop_recovery_receipt(&db.zone, "unexplained-absence")
            .unwrap()
            .is_none()
    );
}
