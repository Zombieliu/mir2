use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Database {
    url: String,
    schema: String,
    scoped: String,
}
impl Database {
    fn new() -> Self {
        let url = std::env::var("MIR2_GUILD_TEST_DATABASE_URL")
            .expect("dedicated test PostgreSQL required");
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let schema = format!("prepared_kill_{nonce:x}");
        Client::connect(&url, NoTls)
            .unwrap()
            .batch_execute(&format!("CREATE SCHEMA {schema}"))
            .unwrap();
        let scoped = format!("{url}?options=-c%20search_path%3D{schema}%20-c%20lock_timeout%3D3000%20-c%20statement_timeout%3D10000");
        let db = Self {
            url,
            schema,
            scoped,
        };
        db.store().ensure_migrated().unwrap();
        db.connect().execute("INSERT INTO zone_owner_leases(zone_id,owner_id,fencing_token,expires_at_ms,heartbeat_at_ms) VALUES('test-zone','test-owner',7,floor(extract(epoch FROM clock_timestamp())*1000)::bigint+120000,0)", &[]).unwrap();
        db
    }
    fn store(&self) -> PostgresEconomyStore {
        PostgresEconomyStore::new(&self.scoped)
    }
    fn connect(&self) -> Client {
        Client::connect(&self.scoped, NoTls).unwrap()
    }
    fn scalar(&self, sql: &str) -> i64 {
        self.connect().query_one(sql, &[]).unwrap().get(0)
    }
}
impl Drop for Database {
    fn drop(&mut self) {
        if let Ok(mut client) = Client::connect(&self.url, NoTls) {
            let _ = client.batch_execute(&format!("DROP SCHEMA {} CASCADE", self.schema));
        }
    }
}

// This is a deliberately test-only source participant. It uses the real source
// and Guild tables with their real CAS, but does not stand in for the pending
// production SimulationConfig preparation/adoption adapter.
struct Source {
    identity: ActiveSessionIdentity,
    award_hash: String,
    expected_legs: Vec<EconomyLeg>,
    expected_before: BTreeMap<EconomyBalanceKey, i64>,
    checkpoint: mir2_simulation::CharacterSaveRecord,
    calls: AtomicUsize,
    fail_after_source: bool,
}
impl Source {
    fn fixture(db: &Database) -> Self {
        let runtime =
            crate::economy::tests::start_test_runtime("kill-source", "KillSource").unwrap();
        let identity = runtime.active_identity().unwrap();
        let mut checkpoint = runtime.active_character_checkpoint().unwrap();
        let mut client = db.connect();
        let account = serde_json::json!({"fixture": true});
        client
            .execute(
                "INSERT INTO accounts(account_id,raw_json) VALUES($1,$2)",
                &[&identity.account_id, &account],
            )
            .unwrap();
        let character = serde_json::to_value(&checkpoint.character).unwrap();
        client.execute("INSERT INTO characters(account_id,character_index,character_name,class,gender,level,raw_json) VALUES($1,$2,'KillSource','Warrior','Male',1,$3)", &[&identity.account_id,&identity.character_index,&character]).unwrap();
        client.execute("INSERT INTO character_saves(account_id,character_index,snapshot_json,save_version) VALUES($1,$2,$3,0)", &[&identity.account_id,&identity.character_index,&serde_json::to_value(&checkpoint).unwrap()]).unwrap();
        client.execute("INSERT INTO shared_guilds(guild_id,name_key,raw_json,store_version) VALUES('original-guild','original-guild',$1,1)", &[&serde_json::json!({"experience":0})]).unwrap();
        checkpoint.experience += 123;
        checkpoint.revision += 1;
        let award_hash = "a".repeat(64);
        checkpoint
            .guild_experience_journal
            .applied_kill_receipts
            .insert("prepared-kill-1".into(), award_hash.clone());
        let expected_legs = vec![EconomyLeg {
            balance: EconomyBalanceKey::experience(&identity.account_id, identity.character_index),
            delta: 123,
        }];
        Self {
            identity,
            award_hash,
            expected_before: expected_legs
                .iter()
                .map(|leg| (leg.balance.clone(), 0))
                .collect(),
            expected_legs,
            checkpoint,
            calls: AtomicUsize::new(0),
            fail_after_source: false,
        }
    }
    fn envelope(&self, delta: i64) -> EconomyTransactionEnvelope {
        EconomyTransactionEnvelope {
            idempotency_key: "prepared-kill-1".into(),
            transaction_kind: EconomyTransactionKind::Adjustment,
            zone_id: "test-zone".into(),
            fencing_generation: 7,
            source_sequence: 1,
            created_at_ms: 1,
            legs: if delta == 0 {
                vec![]
            } else {
                vec![EconomyLeg {
                    balance: EconomyBalanceKey::experience(
                        &self.identity.account_id,
                        self.identity.character_index,
                    ),
                    delta,
                }]
            },
            metadata: BTreeMap::from([
                ("operation".into(), "preparedMonsterKillExperience".into()),
                ("awardHash".into(), self.award_hash.clone()),
                ("sourceAccount".into(), self.identity.account_id.clone()),
                (
                    "sourceCharacter".into(),
                    self.identity.character_index.to_string(),
                ),
            ]),
        }
    }
}
impl PreparedKillSource for Source {
    fn identity(&self) -> &ActiveSessionIdentity {
        &self.identity
    }
    fn award_hash(&self) -> &str {
        &self.award_hash
    }
    fn expected_legs(&self) -> &[EconomyLeg] {
        &self.expected_legs
    }
    fn expected_before(&self) -> &BTreeMap<EconomyBalanceKey, i64> {
        &self.expected_before
    }
    fn lock_guilds(&self, tx: &mut Transaction<'_>) -> Result<(), String> {
        tx.query_one(
            "SELECT store_version FROM shared_guilds WHERE guild_id='original-guild' FOR UPDATE",
            &[],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }
    fn publish_source(&self, tx: &mut Transaction<'_>) -> Result<PreparedKillProjection, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let changed = tx
            .execute(
                "UPDATE accounts SET store_version=1 WHERE account_id=$1 AND store_version=0",
                &[&self.identity.account_id],
            )
            .map_err(|e| e.to_string())?;
        if changed != 1 {
            return Err("source CAS rejected".into());
        }
        tx.execute("UPDATE character_saves SET snapshot_json=$3,save_version=1 WHERE account_id=$1 AND character_index=$2", &[&self.identity.account_id,&self.identity.character_index,&serde_json::to_value(&self.checkpoint).unwrap()]).map_err(|e| e.to_string())?;
        tx.execute("UPDATE shared_guilds SET raw_json=jsonb_set(raw_json,'{experience}','7'),store_version=2 WHERE guild_id='original-guild' AND store_version=1", &[]).map_err(|e| e.to_string())?;
        if self.fail_after_source {
            return Err("injected after complete source/Guild write".into());
        }
        PreparedKillProjection::new(
            self.identity.clone(),
            self.award_hash.clone(),
            &self.checkpoint,
        )
    }
}

#[test]
#[ignore = "requires dedicated MIR2_GUILD_TEST_DATABASE_URL"]
fn prepared_kill_postgres_source_guild_ledger_rollback_and_exact_checkpoint_replay() {
    let db = Database::new();
    let mut source = Source::fixture(&db);
    source.fail_after_source = true;
    let envelope = source.envelope(123);
    assert!(db
        .store()
        .transact_prepared_kill(&envelope, &source)
        .unwrap_err()
        .contains("injected"));
    assert_eq!(db.scalar("SELECT store_version FROM accounts"), 0);
    assert_eq!(db.scalar("SELECT store_version FROM shared_guilds"), 1);
    assert_eq!(
        db.scalar("SELECT count(*) FROM game_economy_transactions"),
        0
    );
    source.fail_after_source = false;
    // A later ledger failure must also roll back the already-written checkpoint/Guild.
    assert!(db
        .store()
        .transact_prepared_kill(&source.envelope(99999), &source)
        .unwrap_err()
        .contains("bind"));
    db.connect().execute("INSERT INTO game_economy_balances(account_id,character_index,asset_kind,asset_key,amount) VALUES($1,$2,'experience','experience',$3)", &[&source.identity.account_id, &source.identity.character_index, &i64::MAX]).unwrap();
    assert!(db
        .store()
        .transact_prepared_kill(&envelope, &source)
        .unwrap_err()
        .contains("baseline"));
    db.connect()
        .execute("UPDATE game_economy_balances SET amount=0", &[])
        .unwrap();
    assert_eq!(db.scalar("SELECT save_version FROM character_saves"), 0);
    assert_eq!(db.scalar("SELECT store_version FROM shared_guilds"), 1);
    let (receipt, saved) = db
        .store()
        .transact_prepared_kill(&envelope, &source)
        .unwrap();
    assert!(!receipt.duplicate);
    assert_eq!(db.scalar("SELECT amount FROM game_economy_balances"), 123);
    assert_eq!(
        db.scalar("SELECT (raw_json->>'experience')::bigint FROM shared_guilds"),
        7
    );
    assert_eq!(db.scalar("SELECT save_version FROM character_saves"), 1);
    let calls = source.calls.load(Ordering::SeqCst);
    source.checkpoint.experience += 999; // retry preparation must never replace original roll.
    source.fail_after_source = true;
    db.connect()
        .execute(
            "UPDATE zone_owner_leases SET fencing_token=8,expires_at_ms=0",
            &[],
        )
        .unwrap();
    let (duplicate, replay) = db
        .store()
        .transact_prepared_kill(&envelope, &source)
        .unwrap();
    assert!(duplicate.duplicate);
    assert_eq!(saved, replay);
    assert_eq!(source.calls.load(Ordering::SeqCst), calls);
    let lookup = db.store().lookup_prepared_kill(&envelope).unwrap().unwrap();
    assert!(lookup.0.duplicate);
    assert_eq!(lookup.1, saved);
    assert_eq!(db.scalar("SELECT count(*) FROM game_economy_outbox"), 1);
    source.award_hash = "b".repeat(64);
    assert!(db
        .store()
        .transact_prepared_kill(&source.envelope(123), &source)
        .unwrap_err()
        .contains("conflict"));
}

#[test]
#[ignore = "requires dedicated MIR2_GUILD_TEST_DATABASE_URL"]
fn prepared_kill_postgres_zero_xp_still_commits_source_and_fences_new_writes() {
    let db = Database::new();
    let mut source = Source::fixture(&db);
    source.expected_legs.clear();
    source.expected_before.clear();
    source.checkpoint.experience -= 123;
    let envelope = source.envelope(0);
    assert!(db.store().transact(&envelope).is_err());
    let mut stale = envelope.clone();
    stale.fencing_generation = 6;
    assert!(db
        .store()
        .transact_prepared_kill(&stale, &source)
        .unwrap_err()
        .contains("fenced"));
    db.connect()
        .execute("UPDATE zone_owner_leases SET expires_at_ms=1", &[])
        .unwrap();
    assert!(db
        .store()
        .transact_prepared_kill(&envelope, &source)
        .unwrap_err()
        .contains("expired"));
    assert_eq!(source.calls.load(Ordering::SeqCst), 0);
    db.connect().execute("UPDATE zone_owner_leases SET expires_at_ms=floor(extract(epoch FROM clock_timestamp())*1000)::bigint+120000", &[]).unwrap();
    let (receipt, _) = db
        .store()
        .transact_prepared_kill(&envelope, &source)
        .unwrap();
    assert!(receipt.balances_after.is_empty());
    assert_eq!(db.scalar("SELECT count(*) FROM game_economy_balances"), 0);
    assert_eq!(db.scalar("SELECT save_version FROM character_saves"), 1);
    assert!(
        db.store()
            .transact_prepared_kill(&envelope, &source)
            .unwrap()
            .0
            .duplicate
    );
    let event: EconomyOutboxEvent = serde_json::from_value(
        db.connect()
            .query_one("SELECT payload FROM game_economy_outbox", &[])
            .unwrap()
            .get(0),
    )
    .unwrap();
    db.store().ingest("prepared-test", &event, 1).unwrap();
    let mut stripped = event.clone();
    stripped.source_checkpoint = None;
    assert!(db
        .store()
        .ingest("prepared-stripped", &stripped, 2)
        .is_err());
    assert_eq!(db.scalar("SELECT count(*) FROM game_economy_inbox"), 1);
    // Damaged full source must not be accepted just because its receipt still exists.
    db.connect().execute("UPDATE game_economy_outbox SET payload=jsonb_set(payload,'{sourceCheckpoint,checkpoint,experience}','999999')", &[]).unwrap();
    assert!(db
        .store()
        .transact_prepared_kill(&envelope, &source)
        .unwrap_err()
        .contains("integrity"));
}

#[test]
#[ignore = "requires dedicated MIR2_GUILD_TEST_DATABASE_URL; independent connections"]
fn prepared_kill_postgres_two_connections_only_publish_source_once() {
    let db = Database::new();
    let source = Arc::new(Source::fixture(&db));
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let threads = (0..2)
        .map(|_| {
            let source = source.clone();
            let barrier = barrier.clone();
            let store = db.store();
            std::thread::spawn(move || {
                barrier.wait();
                store.transact_prepared_kill(&source.envelope(123), source.as_ref())
            })
        })
        .collect::<Vec<_>>();
    let results = threads
        .into_iter()
        .map(|t| t.join().unwrap().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|(r, _)| r.duplicate).count(), 1);
    assert_eq!(results[0].1, results[1].1);
    assert_eq!(source.calls.load(Ordering::SeqCst), 1);
    assert_eq!(db.scalar("SELECT amount FROM game_economy_balances"), 123);
}

#[test]
#[ignore = "requires dedicated MIR2_GUILD_TEST_DATABASE_URL"]
fn prepared_kill_postgres_commit_error_is_never_a_definite_retry_receipt() {
    let db = Database::new();
    let source = Source::fixture(&db);
    db.connect().batch_execute("CREATE FUNCTION fail_kill_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected commit boundary failure'; END $$; CREATE CONSTRAINT TRIGGER fail_kill_commit AFTER INSERT ON game_economy_transactions DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_kill_commit()").unwrap();
    let error = db
        .store()
        .transact_prepared_kill(&source.envelope(123), &source)
        .unwrap_err();
    assert!(
        error.starts_with("PREPARED_KILL_COMMIT_OUTCOME_UNKNOWN:"),
        "{error}"
    );
    assert_eq!(db.scalar("SELECT store_version FROM accounts"), 0);
    assert_eq!(db.scalar("SELECT store_version FROM shared_guilds"), 1);
    assert_eq!(
        db.scalar("SELECT count(*) FROM game_economy_transactions"),
        0
    );
    // The future production adapter must keep its write-ahead freeze on this
    // outcome. This isolated layer deliberately does not guess or auto-retry.
}

fn runtime_context() -> SharedAccountInventoryExecutionContext {
    SharedAccountInventoryExecutionContext {
        zone_id: crate::ZoneId::new("test-zone"),
        fencing_generation: 7,
        source_sequence: 1,
        created_at_ms: 1,
        external_commit_authorized: true,
    }
}
fn runtime_award(runtime: &InProcessWorldRuntime) -> SharedAccountInventoryCommandEnvelope {
    SharedAccountInventoryCommandEnvelope {
        identity: runtime.active_identity().unwrap(),
        command: SharedAccountInventoryCommand::MonsterKillAward(
            mir2_simulation::ZoneMonsterKillAward {
                monster_object_id: 912300,
                killed_at_ms: 1,
                monster_name: "Scarecrow".into(),
                experience: 5,
                drops: vec![],
                boss_audit: None,
                experience_selection: None,
                source_receipt_key: None,
            },
        ),
    }
}

#[test]
#[ignore = "requires dedicated MIR2_GUILD_TEST_DATABASE_URL"]
fn prepared_kill_postgres_actual_runtime_source_and_ledger_retry_relogin() {
    actual_runtime_source(false);
}

#[test]
#[ignore = "requires dedicated MIR2_GUILD_TEST_DATABASE_URL"]
fn prepared_kill_postgres_source_of_truth_runtime_and_ledger_retry_relogin() {
    actual_runtime_source(true);
}

fn actual_runtime_source(source_of_truth: bool) {
    let db = Database::new();
    let mut config = crate::GatewayConfig::default();
    if source_of_truth {
        config.account_store_database_mode =
            mir2_simulation::AccountStoreDatabaseMode::SourceOfTruth;
    }
    let config = config.with_account_store_database_url(&db.scoped);
    let mut runtime = crate::economy::tests::start_test_runtime_with_config(
        config,
        "actual-source",
        "ActualSource",
    )
    .unwrap();
    let envelope = runtime_award(&runtime);
    let context = runtime_context();
    let before = runtime.world_snapshot().player_experience;
    let service = PostgresEconomyAccountInventoryService::new(&db.scoped);
    db.connect().batch_execute("CREATE FUNCTION reject_actual_source() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'source failure'; END $$; CREATE TRIGGER reject_actual_source BEFORE UPDATE ON accounts FOR EACH ROW EXECUTE FUNCTION reject_actual_source()").unwrap();
    assert!(
        !service
            .commit_fenced(&mut runtime, Some(&context), envelope.clone())
            .committed
    );
    assert_eq!(runtime.world_snapshot().player_experience, before);
    assert_eq!(
        db.scalar("SELECT count(*) FROM game_economy_transactions"),
        0
    );
    db.connect()
        .batch_execute("DROP TRIGGER reject_actual_source ON accounts")
        .unwrap();
    let first = service.commit_fenced(&mut runtime, Some(&context), envelope.clone());
    assert!(first.committed, "{first:?}");
    let after = runtime.world_snapshot().player_experience;
    assert!(after > before);
    let key: String = db
        .connect()
        .query_one("SELECT idempotency_key FROM game_economy_transactions", &[])
        .unwrap()
        .get(0);
    let identity = runtime.active_identity().unwrap();
    let saved:mir2_simulation::CharacterSaveRecord=serde_json::from_value(db.connect().query_one("SELECT snapshot_json FROM character_saves WHERE account_id=$1 AND character_index=$2",&[&identity.account_id,&identity.character_index]).unwrap().get(0)).unwrap();
    assert_eq!(saved.experience, after);
    assert!(saved
        .guild_experience_journal
        .applied_kill_receipts
        .contains_key(&key));
    assert_eq!(
        db.scalar("SELECT amount FROM game_economy_balances WHERE asset_kind='experience'"),
        after
    );
    runtime
        .execute(mir2_simulation::WorldCommand::ClientPacket(
            mir2_protocol::ClientPacket::LogOut,
        ))
        .unwrap();
    runtime
        .execute(mir2_simulation::WorldCommand::ClientPacket(
            mir2_protocol::ClientPacket::StartGame {
                character_index: identity.character_index,
            },
        ))
        .unwrap();
    assert!(
        service
            .commit_fenced(&mut runtime, Some(&context), envelope.clone())
            .committed
    );
    assert_eq!(runtime.world_snapshot().player_experience, after);
    assert_eq!(
        db.scalar("SELECT count(*) FROM game_economy_transactions"),
        1
    );
    let mut changed = envelope;
    if let SharedAccountInventoryCommand::MonsterKillAward(award) = &mut changed.command {
        award.experience += 1;
        award.source_receipt_key = Some(key);
    }
    assert!(
        !service
            .commit_fenced(&mut runtime, Some(&context), changed)
            .committed
    );
    assert_eq!(runtime.world_snapshot().player_experience, after);
}

#[test]
#[ignore = "requires dedicated MIR2_GUILD_TEST_DATABASE_URL"]
fn prepared_kill_postgres_mirror_file_failure_keeps_committed_ledger_and_durable_fence() {
    let db = Database::new();
    let dir = std::env::temp_dir().join(format!("mir2-prepared-mirror-{}", db.schema));
    let path = dir.join("accounts.json");
    let config = crate::GatewayConfig::default()
        .with_account_store_path(&path)
        .with_account_store_database_url(&db.scoped);
    let mut runtime = crate::economy::tests::start_test_runtime_with_config(
        config.clone(),
        "mirror-source",
        "MirrorSource",
    )
    .unwrap();
    let envelope = runtime_award(&runtime);
    let before = std::fs::read(&path).unwrap();
    config.inject_account_store_transaction_fault(
        mir2_simulation::AccountStoreTransactionFault::BeforeFileRename,
    );
    let service = PostgresEconomyAccountInventoryService::new(&db.scoped);
    let outcome = service.commit_fenced(&mut runtime, Some(&runtime_context()), envelope);
    assert!(
        matches!(
            outcome,
            SharedAccountInventoryCommitOutcome::OutcomeUnknown { .. }
        ),
        "{outcome:?}"
    );
    assert_eq!(
        db.scalar("SELECT count(*) FROM game_economy_transactions"),
        1
    );
    assert!(config.save_account_store().is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    drop(runtime);
    drop(config);
    let reopened = crate::GatewayConfig::default().with_account_store_path(&path);
    assert!(reopened.save_account_store().is_err());
    drop(reopened);
    std::fs::remove_dir_all(dir).unwrap();
}
