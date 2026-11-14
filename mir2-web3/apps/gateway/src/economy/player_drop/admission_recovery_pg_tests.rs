//! Real disposable PostgreSQL recovery checks; no transport-fault claims.
use crate::economy::player_drop::recovery::tests::fixture_heads;
use crate::economy::player_drop::{
    FrozenPlayerDropAdmission, PlayerDropAdmissionPublication, PlayerDropAdmissionStatus,
};
use crate::economy::{start_test_runtime_with_config, PlayerDropPostgresTestDatabase as Database};
use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::{
    GroundDropLootSnapshot, GroundDropSnapshot, InProcessWorldRuntime, PlayerDropOperationBinding,
    PlayerDropRequest, PreparedPlayerDropPublicationFailure, WorldCommand, WorldRuntime, ZoneKey,
};
use std::time::{SystemTime, UNIX_EPOCH};

const ACCOUNT: &str = "admit-recovery";

fn runtime(db: &Database) -> InProcessWorldRuntime {
    let mut runtime =
        start_test_runtime_with_config(db.config("owner-a", 7), ACCOUNT, "AdmitRecovery").unwrap();
    let seed = GroundDropSnapshot {
        object_id: 0x7f10_7023,
        name: "Gold".into(),
        name_colour_argb: -1,
        icon: 0,
        x: 0,
        y: 0,
        quantity: 100,
        source_monster: "owned admission recovery fixture".into(),
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

fn frozen(
    db: &Database,
    runtime: &InProcessWorldRuntime,
    sequence: u64,
) -> FrozenPlayerDropAdmission {
    let identity = runtime.active_identity().unwrap();
    let checkpoint = runtime.active_character_checkpoint().unwrap();
    let binding = PlayerDropOperationBinding::new(
        format!("admission-recovery-{sequence}"),
        "authoritative-source-session".into(),
        identity.account_id,
        identity.character_index,
        db.zone().into(),
        "owner-a".into(),
        7,
        sequence,
    )
    .unwrap();
    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let request = PlayerDropRequest::Gold { amount: 25 };
    FrozenPlayerDropAdmission::new(
        binding,
        ZoneKey::for_map(&checkpoint.map_file_name),
        checkpoint.position,
        created,
        created + 30 * 60 * 1000,
        request.clone(),
        runtime.preview_shared_player_drop(&request).unwrap(),
    )
    .unwrap()
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_postgres_admission_recovery_reads_every_page_and_blocks_unresolved_admissions() {
    let db = Database::new();
    let runtime = runtime(&db);
    let store = db.store();
    let permit = runtime.zone_economy_write_permit().unwrap();
    for sequence in 1..=257 {
        assert!(matches!(
            store
                .admit_player_drop(&permit, &frozen(&db, &runtime, sequence))
                .unwrap(),
            PlayerDropAdmissionPublication::Fresh(_)
        ));
    }
    let set = store
        .read_zone_player_drop_recovery_set(db.zone(), 512)
        .unwrap();
    assert_eq!(set.admission_count(), 257);
    assert!(set.records().is_empty());
    assert!(set
        .admissions()
        .iter()
        .all(|record| record.status() == PlayerDropAdmissionStatus::Admitted));
    let keys = set
        .admissions()
        .iter()
        .map(|record| record.frozen().binding().operation_key())
        .collect::<Vec<_>>();
    assert!(keys.windows(2).all(|keys| keys[0] < keys[1]));
    assert!(store
        .read_zone_player_drop_recovery_set(db.zone(), 256)
        .unwrap_err()
        .contains("bound"));
    db.lease("owner-b", 8);
    let config = db.config("owner-b", 8);
    let permit = config.zone_economy_write_permit().unwrap();
    let source = config
        .prepare_zone_source_recovery(&[ACCOUNT.into()])
        .unwrap();
    let error = store
        .commit_zone_player_drop_recovery(
            &permit,
            &source,
            &set,
            &fixture_heads(db.zone(), "owner-b"),
            "all-admission-pages",
            42,
            5000,
        )
        .unwrap_err();
    assert!(error.contains("unresolved Admitted"), "{error}");
    assert_eq!(
        source
            .checkpoint(ACCOUNT, runtime.active_identity().unwrap().character_index)
            .unwrap()
            .gold,
        runtime.active_character_checkpoint().unwrap().gold
    );
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_postgres_admission_recovery_rejects_changed_sets_and_only_retires_audited_terminal_actors(
) {
    let db = Database::new();
    let runtime = runtime(&db);
    let identity = runtime.active_identity().unwrap();
    let store = db.store();
    let old = store
        .read_zone_player_drop_recovery_set(db.zone(), 256)
        .unwrap();
    let frozen = frozen(&db, &runtime, 1);
    assert!(matches!(
        store
            .admit_player_drop(&runtime.zone_economy_write_permit().unwrap(), &frozen)
            .unwrap(),
        PlayerDropAdmissionPublication::Fresh(_)
    ));
    db.lease("owner-b", 8);
    let config = db.config("owner-b", 8);
    let permit = config.zone_economy_write_permit().unwrap();
    let source = config
        .prepare_zone_source_recovery(&[ACCOUNT.into()])
        .unwrap();
    let heads = fixture_heads(db.zone(), "owner-b");
    assert!(store
        .commit_zone_player_drop_recovery(
            &permit,
            &source,
            &old,
            &heads,
            "changed-admission-set",
            42,
            5000
        )
        .unwrap_err()
        .contains("changed after enumeration"));
    let unresolved = store
        .read_zone_player_drop_recovery_set(db.zone(), 256)
        .unwrap();
    assert_ne!(
        old.operation_set_digest(),
        unresolved.operation_set_digest()
    );
    assert_ne!(
        old.admission_set_digest(),
        unresolved.admission_set_digest()
    );

    // Delete through the real canonical account path. An absent live admission
    // actor is still unresolved; roster absence cannot discard that action.
    let mut account = InProcessWorldRuntime::new(config.clone());
    account
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: ACCOUNT.into(),
            password: ACCOUNT.into(),
        }))
        .unwrap();
    assert!(account
        .execute(WorldCommand::ClientPacket(ClientPacket::DeleteCharacter {
            character_index: identity.character_index,
        }))
        .unwrap()
        .contains(&ServerPacket::DeleteCharacterSuccess {
            character_index: identity.character_index
        }));
    let retired = config
        .prepare_zone_source_recovery(&[ACCOUNT.into()])
        .unwrap();
    assert!(retired
        .is_retired_character(ACCOUNT, identity.character_index)
        .unwrap());
    assert!(store
        .commit_zone_player_drop_recovery(
            &permit,
            &retired,
            &unresolved,
            &heads,
            "unresolved-retired-admission",
            42,
            5000
        )
        .unwrap_err()
        .contains("unresolved admission actor"));
    let aborted = store
        .abort_admitted_player_drop(
            &permit,
            frozen.binding(),
            "successor-admission-abort",
            "quiesced successor; no source operation published",
        )
        .unwrap();
    assert_eq!(aborted.status(), PlayerDropAdmissionStatus::Aborted);
    assert_eq!(
        aborted.abort_audit().unwrap().resolution_kind(),
        "successorAborted"
    );
    let resolved = store
        .read_zone_player_drop_recovery_set(db.zone(), 256)
        .unwrap();
    let receipt = store
        .commit_zone_player_drop_recovery(
            &permit,
            &retired,
            &resolved,
            &heads,
            "resolved-retired-admission",
            43,
            5000,
        )
        .unwrap();
    assert_eq!(receipt.admission_count(), Some(1));
    assert_eq!(
        receipt.admission_set_digest(),
        Some(resolved.admission_set_digest())
    );
    store
        .validate_zone_player_drop_recovery_receipt(&permit, &retired, &receipt, &heads)
        .unwrap();
    assert_eq!(
        db.connect()
            .query_one(
                "SELECT count(*) FROM character_saves WHERE account_id=$1",
                &[&ACCOUNT]
            )
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    assert!(
        resolved.records().is_empty(),
        "resolution never manufactures a Prepared operation"
    );
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_postgres_admission_recovery_link_requires_its_exact_prepared_digest() {
    let db = Database::new();
    let mut runtime = runtime(&db);
    let store = db.store();
    let frozen = frozen(&db, &runtime, 1);
    let permit = runtime.zone_economy_write_permit().unwrap();
    let PlayerDropAdmissionPublication::Fresh(capability) =
        store.admit_player_drop(&permit, &frozen).unwrap()
    else {
        panic!("fresh admission");
    };
    let stopped = runtime.commit_shared_player_drop_via_postgres::<(), _>(
        frozen.binding().clone(),
        frozen.zone(),
        frozen.position().clone(),
        frozen.created_at_unix_ms(),
        frozen.expires_at_unix_ms(),
        frozen.request().clone(),
        |source| {
            store
                .prepare_admitted_player_drop_operation(&permit, &capability, source)
                .map_err(PreparedPlayerDropPublicationFailure::Rejected)?;
            Err(PreparedPlayerDropPublicationFailure::OutcomeUnknown(
                "fixture stops after durable Prepared".into(),
            ))
        },
    );
    assert!(stopped.is_err());
    let set = store
        .read_zone_player_drop_recovery_set(db.zone(), 256)
        .unwrap();
    assert_eq!(set.records().len(), 1);
    assert_eq!(set.admission_count(), 1);
    assert_eq!(
        set.admissions()[0].operation_digest(),
        Some(set.records()[0].operation().operation_digest())
    );
    // The real FK/immutability guards also prevent a missing Linked operation.
    assert!(db.connect().execute("DELETE FROM public.zone_player_drop_operations WHERE source_namespace=current_schema() AND operation_key=$1",
        &[&frozen.binding().operation_key()]).is_err());

    // Deliberate corruption of this owned disposable fixture bypasses only its
    // user trigger, so the read path must still reject a different valid digest.
    let mut client = db.connect();
    let mut tx = client.transaction().unwrap();
    tx.batch_execute("ALTER TABLE public.zone_player_drop_admissions DISABLE TRIGGER USER")
        .unwrap();
    tx.execute("UPDATE public.zone_player_drop_admissions SET linked_operation_digest=$2 WHERE source_namespace=current_schema() AND operation_key=$1",
        &[&frozen.binding().operation_key(), &"0".repeat(64)]).unwrap();
    tx.batch_execute("ALTER TABLE public.zone_player_drop_admissions ENABLE TRIGGER USER")
        .unwrap();
    tx.commit().unwrap();
    assert!(store
        .read_zone_player_drop_recovery_set(db.zone(), 256)
        .unwrap_err()
        .contains("digest conflict"));
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_postgres_recovery_rejects_unattributed_legacy_uid_batches_without_reissuing_them() {
    let db = Database::new();
    let runtime = runtime(&db);
    let store = db.store();
    store
        .initialize_player_drop_item_uid_authority(
            1_000_000_000,
            &"a".repeat(64),
            "owned-recovery-fixture-census",
        )
        .unwrap();
    let frozen = frozen(&db, &runtime, 1);
    let ids = store
        .reserve_player_drop_item_uids(
            &runtime.zone_economy_write_permit().unwrap(),
            frozen.binding(),
            1,
        )
        .unwrap();
    db.lease("owner-b", 8);
    let config = db.config("owner-b", 8);
    let permit = config.zone_economy_write_permit().unwrap();
    let source = config
        .prepare_zone_source_recovery(&[ACCOUNT.into()])
        .unwrap();
    let set = store
        .read_zone_player_drop_recovery_set(db.zone(), 256)
        .unwrap();
    assert!(set.records().is_empty() && set.admissions().is_empty());
    let error = store
        .commit_zone_player_drop_recovery(
            &permit,
            &source,
            &set,
            &fixture_heads(db.zone(), "owner-b"),
            "legacy-orphan-uid",
            42,
            5000,
        )
        .unwrap_err();
    assert!(
        error.contains("unattributed player drop UID batch"),
        "{error}"
    );
    assert_eq!(
        store
            .lookup_player_drop_item_uids(frozen.binding(), 1)
            .unwrap(),
        Some(ids)
    );
    assert!(store
        .lookup_player_drop_admission(frozen.binding())
        .unwrap()
        .is_none());
}
