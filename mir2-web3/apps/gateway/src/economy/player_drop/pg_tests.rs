//! Explicit disposable-database tests. These are never run against the configured
//! game's database, and require the same strict loopback fixture as fence tests.
use super::*;
use mir2_simulation::{
    AccountStoreDatabaseMode, PlayerDropRequest, PreparedPlayerDropPublication,
    PreparedPlayerDropPublicationFailure, SimulationConfig, ZoneSaveFence,
};
#[path = "recovery_pg_tests.rs"]
mod recovery_pg_tests;
#[path = "admission_service_pg_tests.rs"]
mod admission_service_pg_tests;
#[path = "admission_recovery_pg_tests.rs"]
mod admission_recovery_pg_tests;
#[path = "admission_commit_pg_tests.rs"]
mod admission_commit_pg_tests;

pub(crate) struct Database {
    control_url: String,
    url: String,
    name: String,
    zone: String,
}
impl Database {
    pub(crate) fn new() -> Self {
        let control_url = std::env::var("MIR2_TEST_ZONE_SAVE_OPERATION_URL")
            .expect("dedicated Zone PostgreSQL fixture is required");
        let parsed: postgres::Config = control_url.parse().expect("valid fixture URL");
        assert!(control_url.starts_with("postgres://") || control_url.starts_with("postgresql://"));
        assert_eq!(parsed.get_hosts().len(), 1);
        assert_eq!(parsed.get_ports().len(), 1);
        assert!(!matches!(parsed.get_ports()[0], 0 | 5432));
        assert!(parsed.get_options().is_none());
        assert!(
            matches!(&parsed.get_hosts()[0],postgres::config::Host::Tcp(host) if host.parse::<std::net::IpAddr>().is_ok_and(|ip|ip.is_loopback()))
        );
        assert!(parsed
            .get_dbname()
            .unwrap_or("")
            .strip_prefix("mir2_zone_lease_test_")
            .is_some_and(
                |s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            ));
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let name = format!("mir2_zone_lease_test_drop_{}_{stamp}", std::process::id());
        assert!(name.len() < 63);
        Client::connect(&control_url, NoTls)
            .expect("fixture connection")
            .batch_execute(&format!("CREATE DATABASE {name}"))
            .expect("dedicated fixture may create disposable database");
        let (server, tail) = control_url.rsplit_once('/').unwrap();
        let query = tail
            .split_once('?')
            .map_or(String::new(), |(_, query)| format!("?{query}"));
        let url = format!("{server}/{name}{query}");
        let db = Self {
            control_url,
            url,
            name,
            zone: "durable-drop-zone".into(),
        };
        db.store().ensure_migrated().unwrap();
        db.lease("owner-a", 7);
        db
    }
    pub(crate) fn connect(&self) -> Client {
        Client::connect(&self.url, NoTls).expect("disposable fixture connection")
    }
    pub(crate) fn store(&self) -> PostgresEconomyStore {
        PostgresEconomyStore::new(&self.url)
    }
    pub(crate) fn config(&self, owner: &str, epoch: u64) -> SimulationConfig {
        let mut config = SimulationConfig::default();
        config.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;
        config.account_store_database_url = Some(self.url.clone());
        config
            .with_zone_save_fence(
                ZoneSaveFence::new(self.zone.clone(), owner.into(), epoch).unwrap(),
            )
            .unwrap()
    }
    pub(crate) fn lease(&self, owner: &str, epoch: i64) {
        self.connect().execute("INSERT INTO public.zone_owner_leases(zone_id,owner_id,fencing_token,expires_at_ms,heartbeat_at_ms) VALUES($1,$2,$3,floor(extract(epoch FROM clock_timestamp())*1000)::bigint+120000,1) ON CONFLICT(zone_id) DO UPDATE SET owner_id=$2,fencing_token=$3,expires_at_ms=EXCLUDED.expires_at_ms,heartbeat_at_ms=1",&[&self.zone,&owner,&epoch]).unwrap();
    }
    fn binding(&self, key: &str) -> PlayerDropOperationBinding {
        PlayerDropOperationBinding::new(
            key.into(),
            "original-rpc".into(),
            "drop-source".into(),
            0,
            self.zone.clone(),
            "owner-a".into(),
            7,
            1,
        )
        .unwrap()
    }
    pub(crate) fn url(&self) -> &str {
        &self.url
    }
    pub(crate) fn zone(&self) -> &str {
        &self.zone
    }
}
impl Drop for Database {
    fn drop(&mut self) {
        let target: postgres::Config = self
            .url
            .parse()
            .unwrap_or_else(|_| panic!("owned drop fixture target is invalid"));
        let control: postgres::Config = self
            .control_url
            .parse()
            .unwrap_or_else(|_| panic!("owned drop fixture control is invalid"));
        assert!(
            self.name.starts_with("mir2_zone_lease_test_drop_")
                && self
                    .name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        );
        assert!(
            target.get_dbname() == Some(self.name.as_str())
                && target.get_dbname() != control.get_dbname(),
            "cleanup target must be this fixture's self-created database"
        );
        assert!(
            target.get_hosts().len() == 1
                && control.get_hosts().len() == 1
                && target.get_ports().len() == 1
                && target.get_ports() == control.get_ports()
                && !matches!(target.get_ports()[0], 0 | 5432)
                && target.get_options().is_none()
                && control.get_options().is_none(),
            "cleanup requires the exact private fixture cluster"
        );
        assert!(
            matches!((&target.get_hosts()[0],&control.get_hosts()[0]),(postgres::config::Host::Tcp(target_host),postgres::config::Host::Tcp(control_host))
            if target_host==control_host && target_host.parse::<std::net::IpAddr>().is_ok_and(|ip|ip.is_loopback())),
            "cleanup requires the exact loopback fixture host"
        );
        let mut admin = Client::connect(&self.control_url, NoTls).unwrap_or_else(|_| {
            panic!("owned drop fixture cleanup control connection unavailable")
        });
        let actual_control: String = admin
            .query_one("SELECT current_database()", &[])
            .unwrap_or_else(|_| panic!("owned drop fixture cleanup control identity unavailable"))
            .get(0);
        assert!(
            control.get_dbname() == Some(actual_control.as_str()) && actual_control != self.name,
            "cleanup cannot target its control database"
        );
        // The simulation's process-wide account pool retains idle Clients after
        // runtime drop. FORCE disconnects only this exact self-created database.
        admin
            .batch_execute(&format!("DROP DATABASE {} WITH (FORCE)", self.name))
            .unwrap_or_else(|_| panic!("owned disposable drop fixture database cleanup failed"));
    }
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_postgres_complete_source_ledger_ack_restart_and_terminal_pickup() {
    let db = Database::new();
    let config = db.config("owner-a", 7);
    let store = db.store();
    let mut runtime = crate::economy::tests::start_test_runtime_with_config(
        config.clone(),
        "drop-source",
        "DropSource",
    )
    .unwrap();
    let seed = mir2_simulation::GroundDropSnapshot {
        object_id: 0x7f007001,
        name: "Gold".into(),
        name_colour_argb: -1,
        icon: 0,
        x: 0,
        y: 0,
        quantity: 100,
        source_monster: "owned fixture".into(),
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
    let permit = runtime.zone_economy_write_permit().unwrap();
    let mut context = super::tests::context();
    context.zone_id = crate::ZoneId::new(&db.zone);
    context.created_at_ms = now_ms();
    store
        .bootstrap_character_fenced(&permit, &context, &identity, &runtime.world_snapshot())
        .unwrap();
    let binding = PlayerDropOperationBinding::new(
        "ordinary-gold".into(),
        "source-rpc".into(),
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
        instance_id: "".into(),
    };
    let created = now_ms();
    let (receipt, _) = runtime
        .commit_shared_player_drop_via_postgres(
            binding.clone(),
            &zone,
            before.position.clone(),
            created,
            created + 30 * 60 * 1000,
            PlayerDropRequest::Gold { amount: 25 },
            |source| {
                store
                    .prepare_player_drop_operation(&permit, source)
                    .map_err(PreparedPlayerDropPublicationFailure::Rejected)?;
                let prepared = store
                    .lookup_player_drop_operation(&binding)
                    .unwrap()
                    .unwrap();
                assert_eq!(prepared.status(), PlayerDropOperationStatus::Prepared);
                assert!(prepared.committed_effect(&permit, &db.url).is_err());
                let receipt = store
                    .commit_player_drop_operation(&permit, source)
                    .map_err(|error| {
                        if error.contains("COMMIT_OUTCOME_UNKNOWN") {
                            PreparedPlayerDropPublicationFailure::OutcomeUnknown(error)
                        } else {
                            PreparedPlayerDropPublicationFailure::Rejected(error)
                        }
                    })?;
                Ok(PreparedPlayerDropPublication::Written(receipt))
            },
        )
        .unwrap();
    let after = runtime.active_character_checkpoint().unwrap();
    assert_eq!(after.gold, before.gold - 25);
    assert!(receipt.source_checkpoint_digest.is_some());
    let record = store
        .lookup_player_drop_operation(&binding)
        .unwrap()
        .unwrap();
    assert_eq!(record.status(), PlayerDropOperationStatus::Committed);
    store
        .acknowledge_player_drop_effect(&permit, &record, 1)
        .unwrap();
    assert_eq!(
        PostgresEconomyStore::new(&db.url)
            .enumerate_zone_player_drop_operations(&db.zone, None, 1)
            .unwrap()
            .len(),
        1,
        "ACKed live effects remain enumerable on restart"
    );
    let mut pickup = super::tests::pickup(record.operation(), "recipient-a", 700);
    let SharedAccountInventoryCommand::PlayerDropClaimPickup { drop, .. } = &mut pickup.command
    else {
        unreachable!()
    };
    drop.x = before.position.x;
    drop.y = before.position.y;
    context.source_sequence += 1;
    let first = economy_transaction_for_command(&context, &pickup, None).unwrap();
    let credited = store.transact_fenced(&permit, &first).unwrap();
    let consumed = store
        .lookup_player_drop_operation(&binding)
        .unwrap()
        .unwrap();
    assert_eq!(consumed.status(), PlayerDropOperationStatus::Consumed);
    assert_eq!(
        consumed.terminal_event_id(),
        Some(credited.event_id.as_str())
    );
    let mut next = super::tests::pickup(record.operation(), "recipient-b", 999);
    let SharedAccountInventoryCommand::PlayerDropClaimPickup { drop, .. } = &mut next.command
    else {
        unreachable!()
    };
    drop.x = before.position.x;
    drop.y = before.position.y;
    let next = economy_transaction_for_command(&context, &next, None).unwrap();
    let duplicate = store.transact_fenced(&permit, &next).unwrap();
    assert!(duplicate.duplicate && duplicate.settled_elsewhere);
    assert_eq!(
        store
            .balance(&EconomyBalanceKey::gold("recipient-b", 1))
            .unwrap(),
        0
    );
    assert_eq!(
        store
            .pending_ground_drop_projections(&pickup.identity)
            .unwrap()
            .len(),
        1,
        "terminal recipient intent survives delivery crash"
    );
    db.lease("owner-b", 8);
    let successor = db.config("owner-b", 8).zone_economy_write_permit().unwrap();
    assert!(store
        .lookup_player_drop_operation(&binding)
        .unwrap()
        .unwrap()
        .committed_effect(&successor, &db.url)
        .is_ok());
    assert!(store
        .acknowledge_player_drop_effect(&permit, &consumed, 2)
        .is_err());
    store
        .acknowledge_player_drop_effect(&successor, &consumed, 2)
        .unwrap();
    assert_eq!(
        store
            .enumerate_zone_player_drop_operations(&db.zone, None, 256)
            .unwrap()[0]
            .status(),
        PlayerDropOperationStatus::Consumed
    );
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_postgres_uid_requires_census_idempotent_full_u64_fenced_and_exhaustion() {
    let db = Database::new();
    let config = db.config("owner-a", 7);
    let permit = config.zone_economy_write_permit().unwrap();
    let store = db.store();
    let binding = db.binding("uids");
    assert!(store
        .reserve_player_drop_item_uids(&permit, &binding, 2)
        .is_err());
    store
        .initialize_player_drop_item_uid_authority(
            u64::MAX - 2,
            &"a".repeat(64),
            "owned-fixture-complete-census",
        )
        .unwrap();
    let ids = store
        .reserve_player_drop_item_uids(&permit, &binding, 2)
        .unwrap();
    assert_eq!(ids, vec![u64::MAX - 1, u64::MAX]);
    assert_eq!(
        store
            .reserve_player_drop_item_uids(&permit, &binding, 2)
            .unwrap(),
        ids
    );
    assert_eq!(
        store.lookup_player_drop_item_uids(&binding, 2).unwrap(),
        Some(ids)
    );
    assert!(store
        .reserve_player_drop_item_uids(&permit, &binding, 1)
        .is_err());
    assert!(store
        .reserve_player_drop_item_uids(&permit, &db.binding("exhausted"), 1)
        .is_err());
    assert!(store
        .initialize_player_drop_item_uid_authority(0, &"b".repeat(64), "forbidden-lower-census")
        .is_err());
    db.lease("owner-b", 8);
    assert!(store
        .reserve_player_drop_item_uids(&permit, &db.binding("stale"), 1)
        .is_err());
    assert_eq!(
        store
            .lookup_player_drop_item_uids(&binding, 2)
            .unwrap()
            .unwrap(),
        vec![u64::MAX - 1, u64::MAX]
    );
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_postgres_namespace_cannot_adopt_prepared_operation_or_uid_authority() {
    let db = Database::new();
    let scoped = |schema: &str| {
        format!(
            "{}{}options=-c%20search_path%3D{schema}",
            db.url,
            if db.url.contains('?') { '&' } else { '?' }
        )
    };
    db.connect()
        .batch_execute("CREATE SCHEMA drop_source_a; CREATE SCHEMA drop_source_b")
        .unwrap();
    let url_a = scoped("drop_source_a");
    let url_b = scoped("drop_source_b");
    let a = PostgresEconomyStore::new(&url_a);
    let b = PostgresEconomyStore::new(&url_b);
    a.ensure_migrated().unwrap();
    b.ensure_migrated().unwrap();
    let operation = super::tests::fixture();
    let binding = operation.binding();
    Client::connect(&url_a,NoTls).unwrap().execute("INSERT INTO public.zone_player_drop_operations(operation_key,world_effect_id,zone_id,owner_id,owner_epoch,operation_digest,intent_digest,operation,expires_at_ms,status,prepared_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,'Prepared',1)",
        &[&binding.operation_key(),&operation.intent().world_effect_id(),&binding.zone_id(),&binding.owner_id(),&(binding.owner_epoch() as i64),&operation.operation_digest(),&operation.intent().intent_digest(),&serde_json::to_value(&operation).unwrap(),&(operation.intent().expires_at_unix_ms() as i64)]).unwrap();
    assert_eq!(
        a.lookup_player_drop_operation(binding)
            .unwrap()
            .unwrap()
            .account_namespace(),
        "drop_source_a"
    );
    assert!(b.lookup_player_drop_operation(binding).unwrap().is_none());
    assert_eq!(
        a.enumerate_zone_player_drop_operations(binding.zone_id(), None, 256)
            .unwrap()
            .len(),
        1
    );
    assert!(b
        .enumerate_zone_player_drop_operations(binding.zone_id(), None, 256)
        .unwrap()
        .is_empty());
    a.initialize_player_drop_item_uid_authority(100, &"a".repeat(64), "complete-census-source-a")
        .unwrap();
    let mut config = db.config("owner-a", 7);
    config.account_store_database_url = Some(url_b);
    let permit_b = config.zone_economy_write_permit().unwrap();
    assert!(b
        .reserve_player_drop_item_uids(&permit_b, &db.binding("namespace-b-uids"), 1)
        .unwrap_err()
        .contains("complete-history census"));
    assert!(b
        .lookup_player_drop_item_uids(&db.binding("namespace-b-uids"), 1)
        .unwrap()
        .is_none());
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_postgres_unknown_commit_keeps_prepared_and_freezes_absent_receipt_retry() {
    let db = Database::new();
    let config = db.config("owner-a", 7);
    let store = db.store();
    let mut runtime = crate::economy::tests::start_test_runtime_with_config(
        config.clone(),
        "drop-source",
        "DropSource",
    )
    .unwrap();
    let seed = mir2_simulation::GroundDropSnapshot {
        object_id: 0x7f007002,
        name: "Gold".into(),
        name_colour_argb: -1,
        icon: 0,
        x: 0,
        y: 0,
        quantity: 100,
        source_monster: "owned fixture".into(),
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
    let permit = runtime.zone_economy_write_permit().unwrap();
    let mut context = super::tests::context();
    context.zone_id = crate::ZoneId::new(&db.zone);
    context.created_at_ms = now_ms();
    store
        .bootstrap_character_fenced(&permit, &context, &identity, &runtime.world_snapshot())
        .unwrap();
    let binding = PlayerDropOperationBinding::new(
        "unknown-gold".into(),
        "source-rpc".into(),
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
        instance_id: "".into(),
    };
    let created = now_ms();
    db.connect().batch_execute("CREATE FUNCTION reject_drop_commit() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'owned deferred rollback' USING ERRCODE='23505'; END; $$; CREATE CONSTRAINT TRIGGER reject_drop_commit AFTER INSERT ON game_economy_transactions DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_drop_commit()").unwrap();
    let result = runtime.commit_shared_player_drop_via_postgres(
        binding.clone(),
        &zone,
        before.position.clone(),
        created,
        created + 30 * 60 * 1000,
        PlayerDropRequest::Gold { amount: 25 },
        |source| {
            store
                .prepare_player_drop_operation(&permit, source)
                .map_err(PreparedPlayerDropPublicationFailure::Rejected)?;
            store
                .commit_player_drop_operation(&permit, source)
                .map(PreparedPlayerDropPublication::Written)
                .map_err(|error| {
                    assert!(error.contains("COMMIT_OUTCOME_UNKNOWN"));
                    PreparedPlayerDropPublicationFailure::OutcomeUnknown(error)
                })
        },
    );
    assert!(matches!(
        result,
        Err(PreparedPlayerDropPublicationFailure::OutcomeUnknown(_))
    ));
    let prepared = store
        .lookup_player_drop_operation(&binding)
        .unwrap()
        .unwrap();
    assert_eq!(prepared.status(), PlayerDropOperationStatus::Prepared);
    assert!(prepared.receipt().is_none());
    assert_eq!(
        store
            .balance(&EconomyBalanceKey::gold(
                &identity.account_id,
                identity.character_index
            ))
            .unwrap(),
        i64::from(before.gold)
    );
    let saved: serde_json::Value = db
        .connect()
        .query_one(
            "SELECT snapshot_json FROM character_saves WHERE account_id=$1 AND character_index=$2",
            &[&identity.account_id, &identity.character_index],
        )
        .unwrap()
        .get(0);
    assert_eq!(saved["gold"].as_u64(), Some(u64::from(before.gold)));
    db.connect().batch_execute("DROP TRIGGER reject_drop_commit ON game_economy_transactions; DROP FUNCTION reject_drop_commit()").unwrap();
    let service = PostgresEconomyAccountInventoryService::new(&db.url);
    assert!(matches!(
        service.publish_durable_player_drop(
            &mut runtime,
            binding.clone(),
            &zone,
            before.position.clone(),
            created,
            created + 30 * 60 * 1000,
            PlayerDropRequest::Gold { amount: 25 }
        ),
        Err(PreparedPlayerDropPublicationFailure::OutcomeUnknown(_))
    ));
    assert_eq!(
        db.connect()
            .query_one("SELECT count(*) FROM game_economy_transactions", &[])
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    let mut tx = db.connect();
    let mut tx = tx.transaction().unwrap();
    assert!(permit
        .lock_and_validate(&mut tx, &db.url, &db.zone, 7)
        .is_err());
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_postgres_successor_abort_retires_old_prepared_source_without_assets_or_uid_reuse() {
    let db = Database::new();
    let config = db.config("owner-a", 7);
    let store = db.store();
    let mut runtime = crate::economy::tests::start_test_runtime_with_config(
        config.clone(),
        "drop-source",
        "DropSource",
    )
    .unwrap();
    let seed = mir2_simulation::GroundDropSnapshot {
        object_id: 0x7f007003,
        name: "Gold".into(),
        name_colour_argb: -1,
        icon: 0,
        x: 0,
        y: 0,
        quantity: 100,
        source_monster: "owned fixture".into(),
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
    let original = runtime.zone_economy_write_permit().unwrap();
    let mut context = super::tests::context();
    context.zone_id = crate::ZoneId::new(&db.zone);
    context.created_at_ms = now_ms();
    store
        .bootstrap_character_fenced(&original, &context, &identity, &runtime.world_snapshot())
        .unwrap();
    let binding = PlayerDropOperationBinding::new(
        "abandoned-gold".into(),
        "source-rpc".into(),
        identity.account_id.clone(),
        identity.character_index,
        db.zone.clone(),
        "owner-a".into(),
        7,
        41,
    )
    .unwrap();
    store
        .initialize_player_drop_item_uid_authority(
            1000,
            &"a".repeat(64),
            "complete-owned-fixture-census",
        )
        .unwrap();
    let reserved = store
        .reserve_player_drop_item_uids(&original, &binding, 1)
        .unwrap();
    assert_eq!(reserved, vec![1001]);
    let source_image = || {
        db.connect().query_one("SELECT jsonb_build_object('accounts',(SELECT jsonb_agg(to_jsonb(a) ORDER BY account_id) FROM accounts a),'saves',(SELECT jsonb_agg(to_jsonb(s) ORDER BY account_id,character_index) FROM character_saves s),'balances',(SELECT jsonb_agg(to_jsonb(b) ORDER BY account_id,character_index,asset_kind,asset_key) FROM game_economy_balances b))",&[]).unwrap().get::<_,serde_json::Value>(0)
    };
    let before_image = source_image();
    let zone = mir2_simulation::ZoneKey {
        shard_id: "test".into(),
        map_file_name: before.map_file_name.clone(),
        channel_id: 1,
        instance_id: "".into(),
    };
    let created = now_ms();
    let result: Result<((), Vec<ServerPacket>), PreparedPlayerDropPublicationFailure> = runtime
        .commit_shared_player_drop_via_postgres(
            binding.clone(),
            &zone,
            before.position.clone(),
            created,
            created + 30 * 60 * 1000,
            PlayerDropRequest::Gold { amount: 25 },
            |source| {
                store
                    .prepare_player_drop_operation(&original, source)
                    .unwrap();
                assert!(store
                    .abort_prepared_player_drop_operation(
                        &original,
                        &binding,
                        "recovery-1",
                        "old writer fenced before publication"
                    )
                    .is_err());
                let prepared = store
                    .lookup_player_drop_operation(&binding)
                    .unwrap()
                    .unwrap();
                assert!(prepared.committed_effect(&original, &db.url).is_err());
                db.lease("owner-b", 8);
                let successor = db.config("owner-b", 8).zone_economy_write_permit().unwrap();
                let aborted = store
                    .abort_prepared_player_drop_operation(
                        &successor,
                        &binding,
                        "recovery-1",
                        "old writer fenced before publication",
                    )
                    .unwrap();
                assert_eq!(aborted.status(), PlayerDropOperationStatus::Aborted);
                assert!(aborted.receipt().is_none());
                assert!(aborted.commit_receipt().is_err());
                assert!(aborted.committed_effect(&successor, &db.url).is_err());
                assert_eq!(
                    aborted.abort_audit().unwrap().canonical_source_relation(),
                    "exactBefore"
                );
                assert_eq!(
                    source_image(),
                    before_image,
                    "aborting changes neither complete source nor balances"
                );
                assert!(store
                    .commit_player_drop_operation(&original, source)
                    .is_err());
                assert!(store
                    .commit_player_drop_operation(&successor, source)
                    .is_err());
                let participant = PlayerDropLedgerSource::new(source).unwrap();
                let mut client = db.connect();
                let mut tx = client.transaction().unwrap();
                successor
                    .lock_and_validate(&mut tx, &db.url, &db.zone, 8)
                    .unwrap();
                assert!(participant.lock_prepared_operation(&mut tx).is_err());
                drop(tx);
                drop(client);
                let duplicate = store
                    .abort_prepared_player_drop_operation(
                        &successor,
                        &binding,
                        "recovery-1",
                        "old writer fenced before publication",
                    )
                    .unwrap();
                assert_eq!(duplicate.abort_audit(), aborted.abort_audit());
            assert!(db.connect().execute("UPDATE public.zone_player_drop_abort_audit SET recovery_id='tampered' WHERE operation_key=$1",&[&binding.operation_key()]).is_err());
            assert!(db.connect().execute("UPDATE public.zone_player_drop_operations SET status='Prepared',aborted_at_ms=NULL,abort_audit=NULL,abort_digest=NULL WHERE operation_key=$1",&[&binding.operation_key()]).is_err());
                assert!(store
                    .abort_prepared_player_drop_operation(
                        &successor,
                        &binding,
                        "changed-recovery",
                        "old writer fenced before publication"
                    )
                    .is_err());
                assert!(store
                    .abort_prepared_player_drop_operation(
                        &successor,
                        &binding,
                        "recovery-1",
                        "changed reason"
                    )
                    .is_err());
                let changed = PlayerDropOperationBinding::new(
                    "abandoned-gold".into(),
                    "other-rpc".into(),
                    identity.account_id.clone(),
                    identity.character_index,
                    db.zone.clone(),
                    "owner-a".into(),
                    7,
                    41,
                )
                .unwrap();
                assert!(store
                    .abort_prepared_player_drop_operation(
                        &successor,
                        &changed,
                        "recovery-1",
                        "old writer fenced before publication"
                    )
                    .is_err());
                assert_eq!(
                    db.connect()
                        .query_one(
                            "SELECT count(*) FROM public.zone_player_drop_abort_audit",
                            &[]
                        )
                        .unwrap()
                        .get::<_, i64>(0),
                    1
                );
                assert_eq!(
                    store.lookup_player_drop_item_uids(&binding, 1).unwrap(),
                    Some(reserved.clone())
                );
                let next = PlayerDropOperationBinding::new(
                    "next-operation".into(),
                    "next-rpc".into(),
                    identity.account_id.clone(),
                    identity.character_index,
                    db.zone.clone(),
                    "owner-b".into(),
                    8,
                    42,
                )
                .unwrap();
                assert_eq!(
                    store
                        .reserve_player_drop_item_uids(&successor, &next, 1)
                        .unwrap(),
                    vec![1002]
                );
                assert_eq!(
                    store
                        .enumerate_zone_player_drop_operations(&db.zone, None, 256)
                        .unwrap()[0]
                        .status(),
                    PlayerDropOperationStatus::Aborted
                );
                Err(PreparedPlayerDropPublicationFailure::Rejected(
                    "owned fixture retires its unpublished old operation".into(),
                ))
            },
        );
    assert!(matches!(
        result,
        Err(PreparedPlayerDropPublicationFailure::Rejected(_))
    ));
    assert_eq!(runtime.world_snapshot().gold, before.gold);
    assert_eq!(source_image(), before_image);
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
fn player_drop_postgres_abort_unknown_freezes_successor_and_only_queries_original_operation() {
    let db = Database::new();
    let config = db.config("owner-a", 7);
    let store = db.store();
    let mut runtime = crate::economy::tests::start_test_runtime_with_config(
        config.clone(),
        "drop-source",
        "DropSource",
    )
    .unwrap();
    let seed = mir2_simulation::GroundDropSnapshot {
        object_id: 0x7f007004,
        name: "Gold".into(),
        name_colour_argb: -1,
        icon: 0,
        x: 0,
        y: 0,
        quantity: 100,
        source_monster: "owned fixture".into(),
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
    let original = runtime.zone_economy_write_permit().unwrap();
    let mut context = super::tests::context();
    context.zone_id = crate::ZoneId::new(&db.zone);
    context.created_at_ms = now_ms();
    store
        .bootstrap_character_fenced(&original, &context, &identity, &runtime.world_snapshot())
        .unwrap();
    let binding = PlayerDropOperationBinding::new(
        "abort-unknown".into(),
        "source-rpc".into(),
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
        instance_id: "".into(),
    };
    let created = now_ms();
    let result:Result<((),Vec<ServerPacket>),PreparedPlayerDropPublicationFailure>=runtime.commit_shared_player_drop_via_postgres(binding.clone(),&zone,before.position.clone(),created,created+30*60*1000,PlayerDropRequest::Gold{amount:25},|source|{
        store.prepare_player_drop_operation(&original,source).unwrap();db.lease("owner-b",8);
        let recovery_config=db.config("owner-b",8);let successor=recovery_config.zone_economy_write_permit().unwrap();
        db.connect().batch_execute("CREATE FUNCTION reject_abort_commit() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'owned deferred abort rollback' USING ERRCODE='23505'; END; $$; CREATE CONSTRAINT TRIGGER reject_abort_commit AFTER INSERT ON public.zone_player_drop_abort_audit DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_abort_commit()").unwrap();
        assert!(store.abort_prepared_player_drop_operation(&successor,&binding,"recovery-unknown","old writer fenced before publication").unwrap_err().contains("COMMIT_OUTCOME_UNKNOWN"));
        db.connect().batch_execute("DROP TRIGGER reject_abort_commit ON public.zone_player_drop_abort_audit; DROP FUNCTION reject_abort_commit()").unwrap();
        let record=store.lookup_player_drop_operation(&binding).unwrap().unwrap();assert_eq!(record.status(),PlayerDropOperationStatus::Prepared);assert!(record.abort_audit().is_none());assert!(record.committed_effect(&successor,&db.url).is_err());
        assert!(store.abort_prepared_player_drop_operation(&successor,&binding,"recovery-unknown","old writer fenced before publication").is_err(),"removing the fault cannot authorize a replay after Unknown");
        let cloned=recovery_config.clone().zone_economy_write_permit().unwrap();
        let mut client=db.connect();let mut tx=client.transaction().unwrap();assert!(cloned.lock_and_validate(&mut tx,&db.url,&db.zone,8).is_err());drop(tx);drop(client);
        assert_eq!(db.connect().query_one("SELECT count(*) FROM public.zone_player_drop_abort_audit",&[]).unwrap().get::<_,i64>(0),0);
        assert_eq!(store.balance(&EconomyBalanceKey::gold(&identity.account_id,identity.character_index)).unwrap(),i64::from(before.gold));
        assert!(store.commit_player_drop_operation(&original,source).is_err());
        Err(PreparedPlayerDropPublicationFailure::Rejected("owned fixture leaves original operation query-only".into()))
    });
    assert!(matches!(
        result,
        Err(PreparedPlayerDropPublicationFailure::Rejected(_))
    ));
    assert_eq!(runtime.world_snapshot().gold, before.gold);
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_postgres_service_gold_exact_retry_and_successor_terminal_preserve_newer_canonical() {
    let db = Database::new();
    let config = db.config("owner-a", 7);
    let store = db.store();
    let service = PostgresEconomyAccountInventoryService::new(&db.url);
    assert!(service.supports_durable_player_drops());
    let mut runtime =
        crate::economy::tests::start_test_runtime_with_config(config, "drop-source", "DropSource")
            .unwrap();
    let seed = mir2_simulation::GroundDropSnapshot {
        object_id: 0x7f007005,
        name: "Gold".into(),
        name_colour_argb: -1,
        icon: 0,
        x: 0,
        y: 0,
        quantity: 100,
        source_monster: "owned service fixture".into(),
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
        "service-gold".into(),
        "source-rpc".into(),
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
        instance_id: "".into(),
    };
    let created = now_ms();
    let expires = created + 30 * 60 * 1000;
    let committed = service
        .publish_durable_player_drop(
            &mut runtime,
            binding.clone(),
            &zone,
            before.position.clone(),
            created,
            expires,
            PlayerDropRequest::Gold { amount: 25 },
        )
        .unwrap();
    assert_eq!(committed.operation.binding(), &binding);
    assert_eq!(committed.receipt.state(), PlayerDropEffectState::Live);
    assert_eq!(committed.effect.intent(), committed.operation.intent());
    assert_eq!(
        committed
            .packets
            .iter()
            .filter(|packet| matches!(packet, ServerPacket::LoseGold { gold: 25 }))
            .count(),
        1
    );
    let after = runtime.active_character_checkpoint().unwrap();
    assert_eq!(after.gold, before.gold - 25);
    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(
        serde_json::to_value(&after).unwrap(),
        serde_json::to_value(committed.operation.checkpoint()).unwrap()
    );
    assert_eq!(
        store
            .balance(&EconomyBalanceKey::gold(
                &identity.account_id,
                identity.character_index
            ))
            .unwrap(),
        i64::from(after.gold)
    );
    let duplicate = service
        .publish_durable_player_drop(
            &mut runtime,
            binding.clone(),
            &zone,
            before.position.clone(),
            created,
            expires,
            PlayerDropRequest::Gold { amount: 25 },
        )
        .unwrap();
    assert_eq!(
        duplicate.operation.operation_digest(),
        committed.operation.operation_digest()
    );
    assert_eq!(duplicate.receipt, committed.receipt);
    assert!(duplicate.packets.is_empty());
    assert_eq!(
        serde_json::to_value(runtime.active_character_checkpoint().unwrap()).unwrap(),
        serde_json::to_value(&after).unwrap()
    );
    assert_eq!(
        db.connect()
            .query_one("SELECT count(*) FROM game_economy_transactions", &[])
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    // Later legitimate source progress must survive an exact old receipt. The
    // successor consumes custody without requiring the original player online.
    runtime.save_active_character().unwrap();
    let later = runtime.active_character_checkpoint().unwrap();
    assert!(later.revision > after.revision);
    let canonical_before: serde_json::Value = db
        .connect()
        .query_one(
            "SELECT raw_json FROM accounts WHERE account_id=$1",
            &[&identity.account_id],
        )
        .unwrap()
        .get(0);
    db.lease("owner-b", 8);
    let successor = db.config("owner-b", 8).zone_economy_write_permit().unwrap();
    let record = PostgresEconomyStore::new(&db.url)
        .lookup_player_drop_operation(&binding)
        .unwrap()
        .unwrap();
    assert_eq!(
        record
            .committed_effect(&successor, &db.url)
            .unwrap()
            .state(),
        PlayerDropEffectState::Live
    );
    let mut pickup = super::tests::pickup(record.operation(), "recipient-a", 800);
    let SharedAccountInventoryCommand::PlayerDropClaimPickup { drop, .. } = &mut pickup.command
    else {
        unreachable!()
    };
    drop.x = before.position.x;
    drop.y = before.position.y;
    let context = SharedAccountInventoryExecutionContext {
        zone_id: crate::ZoneId::new(&db.zone),
        fencing_generation: 8,
        source_sequence: 42,
        created_at_ms: now_ms(),
        external_commit_authorized: true,
    };
    let envelope = economy_transaction_for_command(&context, &pickup, None).unwrap();
    store.transact_fenced(&successor, &envelope).unwrap();
    let terminal = store
        .lookup_player_drop_operation(&binding)
        .unwrap()
        .unwrap();
    assert_eq!(terminal.status(), PlayerDropOperationStatus::Consumed);
    assert_eq!(
        terminal
            .committed_effect(&successor, &db.url)
            .unwrap()
            .state(),
        PlayerDropEffectState::Consumed
    );
    // Even this stale original permit may only recover an exact old outcome.
    let recovered = PostgresEconomyAccountInventoryService::new(&db.url)
        .publish_durable_player_drop(
            &mut runtime,
            binding.clone(),
            &zone,
            before.position.clone(),
            created,
            expires,
            PlayerDropRequest::Gold { amount: 25 },
        )
        .unwrap();
    assert_eq!(recovered.receipt.state(), PlayerDropEffectState::Consumed);
    assert!(recovered.packets.is_empty());
    assert_eq!(
        serde_json::to_value(runtime.active_character_checkpoint().unwrap()).unwrap(),
        serde_json::to_value(&later).unwrap()
    );
    let canonical_after: serde_json::Value = db
        .connect()
        .query_one(
            "SELECT raw_json FROM accounts WHERE account_id=$1",
            &[&identity.account_id],
        )
        .unwrap()
        .get(0);
    assert_eq!(canonical_after, canonical_before);
    assert_eq!(
        store
            .balance(&EconomyBalanceKey::gold(
                &identity.account_id,
                identity.character_index
            ))
            .unwrap(),
        i64::from(later.gold)
    );
    assert_eq!(
        store
            .balance(&EconomyBalanceKey::gold("recipient-a", 1))
            .unwrap(),
        25
    );
    assert_eq!(
        db.connect()
            .query_one("SELECT count(*) FROM game_economy_transactions", &[])
            .unwrap()
            .get::<_, i64>(0),
        2
    );
}

#[test]
#[ignore = "requires dedicated disposable loopback PostgreSQL and CREATE DATABASE authority"]
fn player_drop_postgres_service_split_preserves_exact_nested_attributes_and_reserves_uids_once() {
    use mir2_protocol::{ClientPacket, MirGridType, UserItem, UserItemStat};
    use mir2_simulation::{ItemContainer, WorldCommand};
    let db = Database::new();
    let config = db.config("owner-a", 7);
    let store = db.store();
    let service = PostgresEconomyAccountInventoryService::new(&db.url);
    let mut runtime =
        crate::economy::tests::start_test_runtime_with_config(config, "drop-source", "DropSource")
            .unwrap();
    // This is the canonical red-potion template used by the simulation's
    // partial_player_drop_roundtrip fixture, rather than an arbitrary first
    // stackable catalogue entry (which may have a different inventory route).
    let template = mir2_game_data::crystal_item_by_name("(HP)DrugSmall")
        .expect("Red Potion canonical template");
    assert!(template.stack_size >= 5 && template.bind & 0x0082 == 0);
    let child_template =
        mir2_game_data::crystal_item_by_name("BronzeBell").expect("ordinary child fixture");
    let mut raw = UserItem {
        unique_id: 765_001,
        item_index: template.item_index,
        current_dura: 123,
        max_dura: 456,
        count: 5,
        soul_bound_id: 0,
        identified: false,
        cursed: true,
        slots: Vec::new(),
        gem_count: 3,
        added_stats: vec![UserItemStat { stat: 17, value: 6 }],
        awake_type: 2,
        awake_values: vec![4, 5],
        refined_value: 6,
        refine_added: 7,
        refine_success_chance: 88,
        wedding_ring: 23,
        expire_info: None,
        rental_information: None,
        is_shop_item: true,
        sealed_info: None,
        gm_made: true,
    };
    let mut child = raw.clone();
    child.unique_id = 765_002;
    child.item_index = child_template.item_index;
    child.count = 1;
    child.slots = Vec::new();
    raw.slots = vec![None, Some(child)];
    let seeded = mir2_simulation::GroundDropSnapshot {
        object_id: 0x7f007006,
        name: template.name.clone(),
        name_colour_argb: -1,
        icon: template.image,
        x: 0,
        y: 0,
        quantity: 5,
        source_monster: "owned exact service fixture".into(),
        owner_object_id: None,
        ownership_remaining_ticks: None,
        loot: GroundDropLootSnapshot::InventoryItem {
            key: format!("crystal-item-{}", template.item_index),
            name: template.name.clone(),
            description: "owned complete split carrier".into(),
            weight: u16::from(template.weight),
            durability_current: Some(raw.current_dura),
            durability_max: Some(raw.max_dura),
            added_attack: 0,
            added_defence: 0,
            added_stats: raw.added_stats.clone(),
            cursed: raw.cursed,
            socket_slots: 2,
            show_group_pickup: template.show_group_pickup,
            exact_item: Some(mir2_simulation::GroundDropItemPayload {
                item: raw.clone(),
                uid_assigned: true,
            }),
        },
    };
    let pickup = runtime.commit_shared_ground_drop_pickup_transaction(&seeded);
    assert!(pickup.committed);
    let gained = pickup
        .packets
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::GainedItem { item } => Some(item),
            _ => None,
        })
        .expect("the exact seeded stack is acquired");
    assert_eq!(gained, &raw);
    let picked_up = runtime.world_snapshot();
    let seeded_items = picked_up
        .inventory_items
        .iter()
        .chain(&picked_up.belt_items)
        .filter(|item| item.unique_id == raw.unique_id)
        .collect::<Vec<_>>();
    assert_eq!(seeded_items.len(), 1, "seeded UID has one exact carrier");
    assert_eq!(seeded_items[0].quantity, u32::from(raw.count));
    assert_eq!(
        seeded_items[0]
            .tooltip_source
            .as_ref()
            .and_then(|source| source.user_item.as_ref()),
        Some(&raw)
    );
    if let Some(belt_item) = picked_up
        .belt_items
        .iter()
        .find(|item| item.unique_id == raw.unique_id)
    {
        // Crystal MoveItem's Belt grid addresses its unified inventory array:
        // the first six cells are belt cells, followed by ordinary bag cells.
        let empty_bag_slot = (0..picked_up.inventory_capacity.saturating_sub(6).min(40))
            .find(|slot| {
                !picked_up.inventory_items.iter().any(|item| {
                    item.container == ItemContainer::Bag1 && u16::from(item.slot) == *slot
                })
            })
            .expect("an empty ordinary bag cell for the exact source stack");
        let from = i32::from(belt_item.slot);
        let to = i32::from(empty_bag_slot) + 6;
        let moved = runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::MoveItem {
                grid: MirGridType::Belt,
                from,
                to,
            }))
            .unwrap();
        assert!(moved.contains(&ServerPacket::MoveItem {
            grid: MirGridType::Belt,
            from,
            to,
            success: true,
        }));
    }
    let ready = runtime.world_snapshot();
    assert!(!ready
        .belt_items
        .iter()
        .any(|item| item.unique_id == raw.unique_id));
    let source = ready
        .inventory_items
        .iter()
        .find(|item| item.unique_id == raw.unique_id)
        .expect("exact seeded source UID is in the ordinary bag before preparation");
    assert_eq!(source.quantity, u32::from(raw.count));
    assert_eq!(
        source
            .tooltip_source
            .as_ref()
            .and_then(|source| source.user_item.as_ref()),
        Some(&raw)
    );
    runtime.save_active_character().unwrap();
    let identity = runtime.active_identity().unwrap();
    let before = runtime.active_character_checkpoint().unwrap();
    let request = PlayerDropRequest::Item {
        unique_id: raw.unique_id,
        count: 2,
        hero_inventory: false,
    };
    let preview = runtime.preview_shared_player_drop(&request).unwrap();
    let PlayerDropOutcome::Item {
        loot:
            GroundDropLootSnapshot::InventoryItem {
                exact_item: Some(mut expected),
                ..
            },
        ..
    } = preview
    else {
        panic!("complete split item preview")
    };
    assert!(!expected.uid_assigned);
    assert_eq!(expected.item.unique_id, 0);
    assert_eq!(expected.item.slots[1].as_ref().unwrap().unique_id, 0);
    // This database is created empty by the fixture; the only UID history is
    // these canonical saves. No Zone/world item or prior drop receipt exists.
    let census:serde_json::Value=db.connect().query_one("SELECT jsonb_build_object('accounts',(SELECT jsonb_agg(raw_json ORDER BY account_id) FROM accounts),'saves',(SELECT jsonb_agg(snapshot_json ORDER BY account_id,character_index) FROM character_saves),'operations',(SELECT coalesce(jsonb_agg(operation ORDER BY operation_key),'[]'::jsonb) FROM public.zone_player_drop_operations),'reservations',(SELECT coalesce(jsonb_agg(item_uid::text ORDER BY item_uid),'[]'::jsonb) FROM public.zone_player_drop_item_uid_reservations))",&[]).unwrap().get(0);
    fn highest_uid(value: &serde_json::Value) -> u64 {
        match value {
            serde_json::Value::Object(fields) => fields
                .iter()
                .map(|(key, value)| {
                    let direct = if matches!(
                        key.as_str(),
                        "unique_id"
                            | "uniqueId"
                            | "user_item_unique_id"
                            | "userItemUniqueId"
                            | "item_uid"
                            | "itemUid"
                    ) {
                        value
                            .as_u64()
                            .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
                            .unwrap_or(0)
                    } else {
                        0
                    };
                    direct.max(highest_uid(value))
                })
                .max()
                .unwrap_or(0),
            serde_json::Value::Array(values) => values.iter().map(highest_uid).max().unwrap_or(0),
            serde_json::Value::String(value) => serde_json::from_str::<serde_json::Value>(value)
                .ok()
                .filter(|value| value.is_object() || value.is_array())
                .as_ref()
                .map(highest_uid)
                .unwrap_or(0),
            _ => 0,
        }
    }
    let floor = highest_uid(&census);
    assert!(floor >= 765_002);
    let census_digest = hex_lower(&Sha256::digest(serde_json::to_vec(&census).unwrap()));
    store
        .initialize_player_drop_item_uid_authority(
            floor,
            &census_digest,
            "owned complete isolated-source census",
        )
        .unwrap();
    let root_uid = mir2_simulation::player_drop::player_drop_next_ordinary_item_uid(floor).unwrap();
    let child_uid =
        mir2_simulation::player_drop::player_drop_next_ordinary_item_uid(root_uid).unwrap();
    expected.item.unique_id = root_uid;
    expected.item.slots[1].as_mut().unwrap().unique_id = child_uid;
    expected.uid_assigned = true;
    let binding = PlayerDropOperationBinding::new(
        "service-split".into(),
        "source-rpc".into(),
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
        instance_id: "".into(),
    };
    let created = now_ms();
    let expires = created + 30 * 60 * 1000;
    let committed = service
        .publish_durable_player_drop(
            &mut runtime,
            binding.clone(),
            &zone,
            before.position.clone(),
            created,
            expires,
            request.clone(),
        )
        .unwrap();
    let PlayerDropOutcome::Item {
        quantity: 2,
        loot:
            GroundDropLootSnapshot::InventoryItem {
                exact_item: Some(actual),
                ..
            },
    } = committed.operation.intent().outcome()
    else {
        panic!("frozen assigned split item")
    };
    assert_eq!(actual, &expected);
    assert_eq!(actual.item.added_stats, raw.added_stats);
    assert_eq!(actual.item.awake_values, raw.awake_values);
    assert_eq!(actual.item.current_dura, raw.current_dura);
    let after = runtime.active_character_checkpoint().unwrap();
    let original = after
        .inventory_items_json
        .iter()
        .map(|encoded| serde_json::from_str::<serde_json::Value>(encoded).unwrap())
        .find(|item| item["unique_id"].as_u64() == Some(raw.unique_id))
        .expect("retained original source UID");
    assert_eq!(original["quantity"].as_u64(), Some(3));
    assert_eq!(
        store.lookup_player_drop_item_uids(&binding, 2).unwrap(),
        Some(vec![root_uid, child_uid])
    );
    let duplicate = service
        .publish_durable_player_drop(
            &mut runtime,
            binding.clone(),
            &zone,
            before.position.clone(),
            created,
            expires,
            request,
        )
        .unwrap();
    assert_eq!(
        duplicate.operation.operation_digest(),
        committed.operation.operation_digest()
    );
    assert!(duplicate.packets.is_empty());
    assert_eq!(
        serde_json::to_value(runtime.active_character_checkpoint().unwrap()).unwrap(),
        serde_json::to_value(&after).unwrap()
    );
    assert_eq!(
        db.connect()
            .query_one("SELECT count(*) FROM game_economy_transactions", &[])
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    assert_eq!(
        db.connect()
            .query_one(
                "SELECT count(*) FROM public.zone_player_drop_item_uid_reservations",
                &[]
            )
            .unwrap()
            .get::<_, i64>(0),
        2
    );
    assert_eq!(
        db.connect()
            .query_one(
                "SELECT issued_through::text FROM public.zone_player_drop_item_uid_authority",
                &[]
            )
            .unwrap()
            .get::<_, String>(0),
        child_uid.to_string()
    );
}
