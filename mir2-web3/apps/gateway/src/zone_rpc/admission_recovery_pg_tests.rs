//! Actual PG + real in-process Host gap injection; no physical process kill.
use super::canonical_recovery_pg_tests::{
    execute, host, promoted, seeded_source, snapshot, source_image, world,
};
use super::*;
use crate::economy::player_drop::{PlayerDropAdmissionStatus, PlayerDropOperationStatus};
use crate::economy::{PlayerDropPostgresTestDatabase, PostgresEconomyAccountInventoryService};
use crate::routing::ZoneOwnerLeaseAuthority;
use crate::zone_lease::PostgresZoneOwnerLeaseAuthority;
use mir2_simulation::WorldRuntime;

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_admission_host_pg_recovers_prepared_and_preparedless_gaps_without_asset_replay() {
    for phase in [1, 2, 4] {
        let db = PlayerDropPostgresTestDatabase::new();
        let seed = seeded_source(&db);
        let identity = seed.active_identity().unwrap();
        let zone = ZoneId::new(db.zone());
        let authority = Arc::new(PostgresZoneOwnerLeaseAuthority::new(
            db.url(),
            "owner-a",
            120_000,
        ));
        let service = Arc::new(PostgresEconomyAccountInventoryService::new(db.url()));
        let factory =
            SharedInProcessZoneRuntimeFactory::with_tick_cadences_and_account_inventory_service(
                Duration::from_secs(3600),
                BTreeMap::new(),
                service.clone(),
            )
            .fresh_replica();
        let source = ZoneHostServer::with_identity_and_factory(
            "owner-a",
            8,
            db.config("owner-a", 7),
            authority.clone(),
            None,
            ZoneRpcLimits::default(),
            Arc::new(factory),
        );
        source.configure_zone_map_catalog(BTreeMap::new(), BTreeSet::from([db.zone().into()]));
        let source_lease = authority.try_owner_lease(&zone).unwrap();
        execute(
            &source,
            &source_lease,
            WorldCommand::ClientPacket(ClientPacket::Login {
                account_id: identity.account_id.clone(),
                password: identity.account_id.clone(),
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
        let before = source_image(&db, identity.character_index);
        let (target, target_authority) = host(&db, "owner-b", 8);
        target.install_base_snapshot(db.zone(), &old_base).unwrap();
        service.fail_next_player_drop_admission_phase_for_test(phase);
        let fault = execute(
            &source,
            &source_lease,
            WorldCommand::ClientPacket(ClientPacket::DropGold { amount: 25 }),
        )
        .unwrap_err();
        assert_eq!(fault.code, "commit_outcome_unknown", "phase {phase}");
        assert_eq!(source_image(&db, identity.character_index), before);
        let admissions = db
            .store()
            .enumerate_zone_player_drop_admissions(db.zone(), None, 256)
            .unwrap();
        assert_eq!(admissions.len(), 1);
        let binding = admissions[0].frozen().binding().clone();
        assert_eq!(
            admissions[0].status(),
            if phase == 4 {
                PlayerDropAdmissionStatus::Linked
            } else {
                PlayerDropAdmissionStatus::Admitted
            }
        );
        let operation = db.store().lookup_player_drop_operation(&binding).unwrap();
        assert_eq!(
            operation.as_ref().map(|op| op.status()),
            (phase == 4).then_some(PlayerDropOperationStatus::Prepared)
        );
        db.lease("owner-b", 8);
        let successor = promoted(&target, target_authority.as_ref(), &zone);
        assert_eq!(
            source_image(&db, identity.character_index),
            before,
            "no source debit, refund or ledger/outbox replay during phase {phase} recovery"
        );
        let resolved = db
            .store()
            .lookup_player_drop_admission(&binding)
            .unwrap()
            .unwrap();
        if phase == 4 {
            assert_eq!(resolved.status(), PlayerDropAdmissionStatus::Linked);
            assert_eq!(
                db.store()
                    .lookup_player_drop_operation(&binding)
                    .unwrap()
                    .unwrap()
                    .status(),
                PlayerDropOperationStatus::Aborted
            );
        } else {
            assert_eq!(resolved.status(), PlayerDropAdmissionStatus::Aborted);
            assert!(resolved.abort_audit_digest().is_some());
            assert!(
                db.store()
                    .lookup_player_drop_operation(&binding)
                    .unwrap()
                    .is_none()
            );
        }
        let recovered = world(&target, &zone);
        for key in recovered.zone_keys() {
            assert!(
                recovered
                    .player_drop_custody_states_for_key(&key)
                    .is_empty(),
                "uncommitted request must not create a world drop"
            );
        }
        assert_eq!(
            target
                .journal
                .lock()
                .unwrap()
                .replication
                .head(db.zone())
                .next_sequence,
            old_base.base_sequence + 1
        );
        assert!(authority.validate_owner_lease(&source_lease).is_err());
        execute(&target, &successor, WorldCommand::Tick).unwrap();
    }
}
