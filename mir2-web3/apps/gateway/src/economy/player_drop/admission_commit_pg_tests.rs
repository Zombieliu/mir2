//! Real admission COMMIT outcomes in the strictly owned disposable PG fixture.
//! The lost-ACK seam follows a successful COMMIT; the deferred trigger instead
//! rolls that COMMIT back. Both outcomes must revoke the original continuation.
use crate::economy::player_drop::recovery::tests::fixture_heads;
use crate::economy::player_drop::{
    FrozenPlayerDropAdmission, PlayerDropAdmissionPublication, PlayerDropAdmissionStatus,
};
use crate::economy::{
    PlayerDropPostgresTestDatabase as Database, PostgresEconomyAccountInventoryService,
    start_test_runtime_with_config,
};
use mir2_simulation::{
    GroundDropLootSnapshot, GroundDropSnapshot, InProcessWorldRuntime, PlayerDropOperationBinding,
    PlayerDropRequest, PreparedPlayerDropPublicationFailure, SimulationConfig, WorldRuntime,
    ZoneKey,
};
use std::time::{SystemTime, UNIX_EPOCH};

const ACCOUNT: &str = "admission-commit-source";

struct Fixture {
    db: Database,
    config: SimulationConfig,
    runtime: InProcessWorldRuntime,
    frozen: FrozenPlayerDropAdmission,
}
impl Fixture {
    fn new(nonce: &str) -> Self {
        let db = Database::new();
        let config = db.config("owner-a", 7);
        let mut runtime =
            start_test_runtime_with_config(config.clone(), ACCOUNT, "AdmitCommit").unwrap();
        let seed = GroundDropSnapshot {
            object_id: 0x7f10_7024,
            name: "Gold".into(),
            name_colour_argb: -1,
            icon: 0,
            x: 0,
            y: 0,
            quantity: 100,
            source_monster: "owned admission commit fixture".into(),
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
        let checkpoint = runtime.active_character_checkpoint().unwrap();
        let binding = PlayerDropOperationBinding::new(
            nonce.into(),
            "original-admission-commit-session".into(),
            identity.account_id,
            identity.character_index,
            db.zone().into(),
            "owner-a".into(),
            7,
            1,
        )
        .unwrap();
        let created = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let request = PlayerDropRequest::Gold { amount: 25 };
        let frozen = FrozenPlayerDropAdmission::new(
            binding,
            ZoneKey::for_map(&checkpoint.map_file_name),
            checkpoint.position,
            created,
            created + 30 * 60 * 1000,
            request.clone(),
            runtime.preview_shared_player_drop(&request).unwrap(),
        )
        .unwrap();
        Self {
            db,
            config,
            runtime,
            frozen,
        }
    }
    fn canonical_source(&self) -> serde_json::Value {
        self.db.connect().query_one("SELECT jsonb_build_object('account',a.raw_json,'accountVersion',a.store_version,'save',s.snapshot_json,'saveVersion',s.save_version) FROM accounts a JOIN character_saves s USING(account_id) WHERE a.account_id=$1 AND s.character_index=$2", &[&ACCOUNT,&self.frozen.binding().character_index()]).unwrap().get(0)
    }
    fn assert_no_source_or_ledger_effects(&self, original: &serde_json::Value) {
        // A boolean comparison keeps account JSON out of assertion diagnostics.
        assert!(
            self.canonical_source() == *original,
            "canonical source changed"
        );
        let counts: Vec<i64> = self.db.connect().query_one("SELECT ARRAY[(SELECT count(*) FROM game_economy_bootstraps),(SELECT count(*) FROM game_economy_balances),(SELECT count(*) FROM public.zone_player_drop_operations),(SELECT count(*) FROM public.zone_player_drop_item_uid_batches),(SELECT count(*) FROM public.zone_player_drop_item_uid_reservations),(SELECT count(*) FROM game_economy_transactions),(SELECT count(*) FROM game_economy_outbox)]", &[]).unwrap().get(0);
        assert_eq!(counts, vec![0; 7], "admission must precede every effect");
        assert_eq!(
            self.runtime.active_character_checkpoint().unwrap().gold,
            100
        );
    }
    fn assert_original_source_frozen(&self) {
        for permit in [
            self.config.zone_economy_write_permit().unwrap(),
            self.runtime.zone_economy_write_permit().unwrap(),
        ] {
            assert!(
                permit
                    .validate_source_write_state()
                    .unwrap_err()
                    .contains("COMMIT_OUTCOME_UNKNOWN")
            );
            let mut client = self.db.connect();
            let mut tx = client.transaction().unwrap();
            assert!(
                permit
                    .lock_and_validate(&mut tx, self.db.url(), self.db.zone(), 7)
                    .unwrap_err()
                    .contains("COMMIT_OUTCOME_UNKNOWN")
            );
        }
    }
    fn assert_new_service_cannot_resume(&mut self) {
        let restarted = PostgresEconomyAccountInventoryService::new(self.db.url());
        assert!(matches!(
            restarted.publish_player_drop(
                &mut self.runtime,
                self.frozen.binding().clone(),
                self.frozen.zone(),
                self.frozen.position().clone(),
                self.frozen.created_at_unix_ms(),
                self.frozen.expires_at_unix_ms(),
                self.frozen.request().clone(),
            ),
            Err(PreparedPlayerDropPublicationFailure::OutcomeUnknown(_))
        ));
    }
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_postgres_admission_commit_lost_ack_has_no_continuation_and_successor_aborts() {
    let mut fixture = Fixture::new("admission-commit-lost-ack");
    let before = fixture.canonical_source();
    fixture.assert_no_source_or_ledger_effects(&before);
    let store = fixture.db.store();
    let original_permit = fixture.config.zone_economy_write_permit().unwrap();
    let error = store
        .admit_player_drop_losing_acknowledgement(&original_permit, &fixture.frozen)
        .unwrap_err();
    assert!(error.contains("COMMIT_OUTCOME_UNKNOWN"));
    fixture.assert_original_source_frozen();
    let recorded = store
        .lookup_player_drop_admission(fixture.frozen.binding())
        .unwrap()
        .unwrap();
    assert_eq!(recorded.status(), PlayerDropAdmissionStatus::Admitted);
    assert_eq!(recorded.frozen(), &fixture.frozen);
    assert!(recorded.linked_operation_digest().is_none());
    assert!(
        store
            .lookup_player_drop_operation(fixture.frozen.binding())
            .unwrap()
            .is_none()
    );
    // A newly constructed service/config can observe the original row, but a
    // duplicate admission yields data only, never a resumed Fresh capability.
    let independent = fixture.db.config("owner-a", 7);
    let duplicate = fixture
        .db
        .store()
        .admit_player_drop(
            &independent.zone_economy_write_permit().unwrap(),
            &fixture.frozen,
        )
        .unwrap();
    match duplicate {
        PlayerDropAdmissionPublication::Existing(record) => {
            assert_eq!(record.status(), PlayerDropAdmissionStatus::Admitted);
            assert_eq!(record.frozen(), &fixture.frozen);
        }
        PlayerDropAdmissionPublication::Fresh(_) => {
            panic!("lost-ACK lookup must not mint a continuation")
        }
    }
    fixture.assert_new_service_cannot_resume();
    fixture.assert_no_source_or_ledger_effects(&before);

    fixture.db.lease("owner-b", 8);
    let successor = fixture.db.config("owner-b", 8);
    let permit = successor.zone_economy_write_permit().unwrap();
    let aborted = store
        .abort_admitted_player_drop(
            &permit,
            fixture.frozen.binding(),
            "lost-ack-successor-abort",
            "quiesced successor resolves admission without source publication",
        )
        .unwrap();
    assert_eq!(aborted.status(), PlayerDropAdmissionStatus::Aborted);
    assert_eq!(aborted.frozen(), &fixture.frozen);
    let audit = aborted.abort_audit().unwrap();
    assert_eq!(audit.resolution_kind(), "successorAborted");
    assert_eq!(audit.recovery_owner_id(), "owner-b");
    assert_eq!(audit.recovery_epoch(), 8);
    assert!(audit.bootstrap_witness().is_none());
    assert!(audit.uid_reservations().is_none());
    assert_eq!(
        aborted.abort_audit_digest(),
        Some(audit.digest().unwrap().as_str())
    );
    let reloaded = store
        .lookup_player_drop_admission(fixture.frozen.binding())
        .unwrap()
        .unwrap();
    assert_eq!(reloaded.abort_audit_digest(), aborted.abort_audit_digest());
    fixture.assert_original_source_frozen();
    fixture.assert_no_source_or_ledger_effects(&before);
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_postgres_admission_deferred_commit_rollback_freezes_and_certifies_empty_successor() {
    let mut fixture = Fixture::new("admission-commit-deferred-rollback");
    let before = fixture.canonical_source();
    fixture.assert_no_source_or_ledger_effects(&before);
    let store = fixture.db.store();
    fixture.db.connect().batch_execute("CREATE FUNCTION public.reject_owned_admission_commit() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'owned deferred admission rollback' USING ERRCODE='23505'; END; $$; CREATE CONSTRAINT TRIGGER reject_owned_admission_commit AFTER INSERT ON public.zone_player_drop_admissions DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.reject_owned_admission_commit()").unwrap();
    let permit = fixture.config.zone_economy_write_permit().unwrap();
    let error = store
        .admit_player_drop(&permit, &fixture.frozen)
        .unwrap_err();
    assert!(error.contains("COMMIT_OUTCOME_UNKNOWN"));
    fixture.db.connect().batch_execute("DROP TRIGGER reject_owned_admission_commit ON public.zone_player_drop_admissions; DROP FUNCTION public.reject_owned_admission_commit()").unwrap();
    fixture.assert_original_source_frozen();
    assert!(
        store
            .lookup_player_drop_admission(fixture.frozen.binding())
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .enumerate_zone_player_drop_admissions(fixture.db.zone(), None, 256)
            .unwrap()
            .is_empty()
    );
    fixture.assert_new_service_cannot_resume();
    assert!(
        store
            .lookup_player_drop_admission(fixture.frozen.binding())
            .unwrap()
            .is_none()
    );
    fixture.assert_no_source_or_ledger_effects(&before);

    fixture.db.lease("owner-b", 8);
    let successor = fixture.db.config("owner-b", 8);
    let successor_permit = successor.zone_economy_write_permit().unwrap();
    let source = successor.prepare_zone_source_recovery(&[]).unwrap();
    let complete = store
        .read_zone_player_drop_recovery_set(fixture.db.zone(), 256)
        .unwrap();
    assert!(complete.records().is_empty());
    assert!(complete.admissions().is_empty());
    assert!(complete.required_characters().is_empty());
    let heads = fixture_heads(fixture.db.zone(), "owner-b");
    let receipt = store
        .commit_zone_player_drop_recovery(
            &successor_permit,
            &source,
            &complete,
            &heads,
            "admission-rollback-empty-successor",
            42,
            5000,
        )
        .unwrap();
    assert_eq!(receipt.operation_count(), 0);
    assert_eq!(receipt.admission_count(), Some(0));
    assert_eq!(receipt.source_digest(), source.source_digest());
    assert_eq!(
        receipt.admission_set_digest(),
        Some(complete.admission_set_digest())
    );
    assert!(
        store
            .validate_zone_player_drop_recovery_receipt(
                &successor_permit,
                &source,
                &receipt,
                &heads,
            )
            .unwrap()
            > 0
    );
    fixture.assert_original_source_frozen();
    fixture.assert_no_source_or_ledger_effects(&before);
}
