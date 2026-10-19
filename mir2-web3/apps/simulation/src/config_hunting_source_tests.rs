use super::*;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

const QUEST: i32 = 7;
const HASH: &str = "trusted-quest-template-v1";
const GOLD: u64 = 5000;

fn quest_state(stage: &str) -> String {
    json!({"quest_id":QUEST,"title":"Trusted quest","summary":"","reward_preview":"",
        "required":1,"current":1,"stage":stage,"task_progress":{}})
    .to_string()
}
fn ready_config() -> (SimulationConfig, String, CharacterSaveRecord) {
    let mut config = SimulationConfig::default();
    config.account_store_database_url =
        Some("postgres://fixture@localhost/mir2_hunting_acceptance_fixture".into());
    config.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;
    let config = config.with_hunting_economy_realm("fixture-realm").unwrap();
    let account = "fixture-account".to_string();
    let before = {
        let mut store = config.account_store.lock().unwrap();
        let mut record = store.accounts.remove("demo").unwrap();
        record.saves.get_mut(&0).unwrap().quest_states_json = vec![quest_state("readyToTurnIn")];
        let before = record.saves[&0].clone();
        store.accounts.insert(account.clone(), record);
        before
    };
    (config, account, before)
}
fn stage_done(
    account: String,
    mut after: CharacterSaveRecord,
) -> impl FnOnce(&mut AccountStore) -> Result<(), String> {
    move |store| {
        after.revision += 1;
        after.quest_states_json = vec![quest_state("completed")];
        store
            .accounts
            .get_mut(&account)
            .unwrap()
            .saves
            .insert(after.character.index, after);
        Ok(())
    }
}

#[test]
fn hunting_source_is_explicit_server_opt_in_and_replica_has_no_authority() {
    let plain = SimulationConfig::default();
    assert!(plain.hunting_economy_realm.is_none());
    assert!(plain
        .bind_hunting_character_incarnation(
            "nobody",
            &CharacterSaveRecord::new(plain.default_character.clone())
        )
        .unwrap()
        .is_none());
    assert!(plain.clone().with_hunting_economy_realm("realm").is_err());
    let (config, _, _) = ready_config();
    assert!(config
        .clone()
        .with_hunting_economy_realm(" realm ")
        .is_err());
    assert!(config
        .ensure_hunting_source_replication_supported()
        .is_err());
    assert!(
        config.fork_for_replica_apply().is_err(),
        "enabled source commands must not be replayed with the legacy gold fallback"
    );
    assert!(plain.ensure_hunting_source_replication_supported().is_ok());
    assert!(plain.fork_for_replica_apply().is_ok());
    let mut replica = config.fork_for_hunting_preparation().unwrap();
    assert!(replica.hunting_economy_realm.is_none());
    assert!(replica.account_store_database_url.is_none());
    assert!(replica.account_store_path.is_none());
    replica.freeze_account_store_writes("shadow-only failure".into());
    assert!(config.ensure_account_store_writable().is_ok());
    replica.rebind_account_store_from(&config);
    assert_eq!(replica.hunting_economy_realm, config.hunting_economy_realm);
    assert!(Arc::ptr_eq(
        &replica.account_store_write_state,
        &config.account_store_write_state
    ));
    assert!(
        replica.fork_for_replica_apply().is_err(),
        "rebinding an old shadow must not bypass the replicated-host admission gate"
    );
}

#[test]
fn hunting_source_preparation_preserves_legacy_gold_and_requires_full_revision() {
    let (config, account, before) = ready_config();
    let unchanged = to_json(&*config.account_store.lock().unwrap()).unwrap();
    let prepared = config
        .prepare_hunting_quest_source(
            &account,
            "pg-character-v1:fixture",
            QUEST,
            HASH,
            GOLD,
            before.clone(),
            stage_done(account.clone(), before.clone()),
        )
        .unwrap();
    assert_eq!(prepared.checkpoint().revision, before.revision + 1);
    assert_eq!(prepared.checkpoint().gold, before.gold);
    assert_eq!(
        unchanged,
        to_json(&*config.account_store.lock().unwrap()).unwrap()
    );
    let mut minted = before.clone();
    minted.gold += 1;
    assert!(config
        .prepare_hunting_quest_source(
            &account,
            "pg-character-v1:fixture",
            QUEST,
            HASH,
            GOLD,
            before.clone(),
            stage_done(account.clone(), minted)
        )
        .is_err());
    assert!(config
        .prepare_hunting_quest_source(
            &account,
            "pg-character-v1:fixture",
            QUEST,
            HASH,
            GOLD,
            before.clone(),
            |_| Ok(())
        )
        .is_err());
    assert!(config
        .prepare_hunting_quest_source(
            &account,
            "",
            QUEST,
            HASH,
            GOLD,
            before.clone(),
            stage_done(account.clone(), before.clone())
        )
        .is_err());
    assert_eq!(source_key(QUEST), "quest-once-v1:7");
    assert!(config
        .recover_pending_hunting_quest_source(&account, "pg-character-v1:fixture", 0, QUEST)
        .unwrap()
        .is_none());
    assert!(
        config
            .recover_hunting_quest_source(&account, "pg-character-v1:fixture", 0, QUEST, HASH, GOLD)
            .unwrap()
            .is_none(),
        "a normal writable source needs no recovery or database call"
    );
    config.freeze_hunting_quest_publication("unrelated owner projection failure".into());
    assert!(config
        .recover_pending_hunting_quest_source(&account, "pg-character-v1:fixture", 0, QUEST)
        .unwrap()
        .is_none());
    assert!(
        config
            .recover_hunting_quest_source(&account, "pg-character-v1:fixture", 0, QUEST, HASH, GOLD)
            .is_err(),
        "no source fence may release a general publication failure"
    );
}

#[test]
fn hunting_source_cannot_adopt_an_earlier_unrelated_publication_freeze() {
    let (config, account, before) = ready_config();
    let source = config
        .prepare_hunting_quest_source(
            &account,
            "pg-character-v1:fixture",
            QUEST,
            HASH,
            GOLD,
            before.clone(),
            stage_done(account.clone(), before),
        )
        .unwrap();
    let original_reason =
        config.freeze_hunting_quest_publication("earlier projection failure".into());
    let earlier_epoch = config.account_store_freeze_epoch.load(Ordering::Acquire);
    let returned_reason =
        config.fence_hunting_source_operation(&source, "later source outcome unknown".into());
    assert_eq!(returned_reason, original_reason);
    assert!(config.account_store_freeze_epoch.load(Ordering::Acquire) > earlier_epoch);
    assert!(
        config.hunting_source_fence.lock().unwrap().is_none(),
        "a new epoch on an existing freeze cannot grant recovery ownership"
    );
    assert!(config
        .recover_pending_hunting_quest_source(&account, "pg-character-v1:fixture", 0, QUEST)
        .unwrap()
        .is_none());
    assert!(config
        .recover_hunting_quest_source(&account, "pg-character-v1:fixture", 0, QUEST, HASH, GOLD)
        .is_err());
    assert!(matches!(&*config.account_store_write_state.lock().unwrap(),
        AccountStoreWriteState::Frozen { reason } if reason == &original_reason));
    assert!(config.ensure_account_store_writable().is_err());
}

#[test]
fn hunting_source_same_reason_refreeze_invalidates_without_reassigning_its_fence() {
    let (config, account, before) = ready_config();
    let source = config
        .prepare_hunting_quest_source(
            &account,
            "pg-character-v1:fixture",
            QUEST,
            HASH,
            GOLD,
            before.clone(),
            stage_done(account.clone(), before),
        )
        .unwrap();
    let original_reason =
        config.fence_hunting_source_operation(&source, "same publication failure".into());
    let original_epoch = config
        .hunting_source_fence
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .epoch;
    assert_eq!(
        config.freeze_hunting_quest_publication("same publication failure".into()),
        original_reason
    );
    assert!(
        config.account_store_freeze_epoch.load(Ordering::Acquire) > original_epoch,
        "identical reasons must still invalidate the old owner"
    );
    config.fence_hunting_source_operation(&source, "same publication failure".into());
    assert_eq!(
        config
            .hunting_source_fence
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .epoch,
        original_epoch,
        "an already frozen state must not refresh a stale source capability"
    );
    assert!(config
        .recover_pending_hunting_quest_source(&account, "pg-character-v1:fixture", 0, QUEST)
        .unwrap()
        .is_none());
    assert!(config
        .recover_hunting_quest_source(&account, "pg-character-v1:fixture", 0, QUEST, HASH, GOLD)
        .is_err());
    assert!(config.ensure_account_store_writable().is_err());
}

fn test_url() -> String {
    let url = std::env::var("MIR2_ECONOMY_TEST_DATABASE_URL")
        .expect("ignored PG source acceptance requires MIR2_ECONOMY_TEST_DATABASE_URL");
    assert!(url.starts_with("postgres://") || url.starts_with("postgresql://"));
    let parsed: postgres::Config = url.parse().expect("isolated PostgreSQL URL");
    assert!(parsed
        .get_dbname()
        .is_some_and(|name| name.starts_with("mir2_hunting_acceptance_")));
    assert!(!parsed.get_hosts().is_empty()&&parsed.get_hosts().iter().all(|host|matches!(host,postgres::config::Host::Tcp(name) if name.eq_ignore_ascii_case("localhost")||name.parse::<std::net::IpAddr>().is_ok_and(|ip|ip.is_loopback()))));
    assert!(parsed
        .get_hostaddrs()
        .iter()
        .all(std::net::IpAddr::is_loopback));
    url
}
struct Fixture {
    client: Client,
    config: SimulationConfig,
    account: String,
    realm: String,
    incarnation: String,
    before: CharacterSaveRecord,
    url: String,
}
impl Fixture {
    fn new() -> Self {
        let base = test_url();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let schema = format!("hunting_quest_{}_{}", std::process::id(), nonce);
        let mut bootstrap = Client::connect(&base, NoTls).expect("isolated fixture database");
        bootstrap
            .batch_execute(&format!("CREATE SCHEMA {schema}"))
            .unwrap();
        let separator = if base.contains('?') { '&' } else { '?' };
        let url = format!("{base}{separator}options=-csearch_path%3D{schema}");
        let mut client = Client::connect(&url, NoTls).unwrap();
        assert_eq!(
            client
                .query_one("SELECT current_schema()", &[])
                .unwrap()
                .get::<_, String>(0),
            schema
        );
        crate::apply_migrations(&mut client).unwrap();
        // Idempotent until the parent registers this migration centrally.
        client
            .batch_execute(include_str!(
                "../../../infra/postgres/migrations/0021_hunting_quest_sources.sql"
            ))
            .unwrap();
        let (_, account, before) = ready_config();
        let account = format!("{account}-{nonce}");
        let mut store = AccountStore::new(before.character.clone());
        let mut record = store.accounts.remove("demo").unwrap();
        record.saves.insert(0, before.clone());
        store.accounts.insert(account.clone(), record);
        PostgresAccountStoreRepository::new(&url, AccountStoreDatabaseMode::SourceOfTruth)
            .save(&store)
            .unwrap();
        let realm = format!("quest-realm-{nonce}");
        let policy=serde_json::from_value(json!({"schema_version":1,"policy_version":"quest-fixture-v1","enabled":true,
            "k":{"numerator":1,"denominator":10},"conversion_fee":{"numerator":2,"denominator":100},
            "daily_decay":{"numerator":1,"denominator":150},"baseline_daily_mint_atoms":72000000,
            "daily_mint_cap_multiplier":{"numerator":3,"denominator":2},"business_timezone":"Asia/Shanghai","settlement_hour":4})).unwrap();
        economy::initialize_realm(&mut client, &realm, &policy).unwrap();
        let config = SimulationConfig::default()
            .with_postgres_account_store(&url)
            .unwrap()
            .with_hunting_economy_realm(&realm)
            .unwrap();
        let incarnation = config
            .bind_hunting_character_incarnation(&account, &before)
            .unwrap()
            .unwrap();
        Self {
            client,
            config,
            account,
            realm,
            incarnation,
            before,
            url,
        }
    }
    fn prepared(&self) -> PreparedHuntingQuestSource {
        self.config
            .prepare_hunting_quest_source(
                &self.account,
                &self.incarnation,
                QUEST,
                HASH,
                GOLD,
                self.before.clone(),
                stage_done(self.account.clone(), self.before.clone()),
            )
            .unwrap()
    }
    fn counts(&mut self) -> (i64, i64, i64) {
        let row=self.client.query_one("SELECT (SELECT count(*) FROM hunting_economy_wallets),(SELECT count(*) FROM hunting_economy_operations),(SELECT count(*) FROM hunting_quest_source_checkpoints)",&[]).unwrap();
        (row.get(0), row.get(1), row.get(2))
    }
    fn bound(&mut self) -> i64 {
        self.client.query_one("SELECT bound_gold FROM hunting_economy_wallets WHERE realm_id=$1 AND account_id=$2 AND character_index=0 AND character_incarnation=$3",&[&self.realm,&self.account,&self.incarnation]).unwrap().get(0)
    }
    fn durable(&mut self) -> CharacterSaveRecord {
        serde_json::from_value(self.client.query_one("SELECT snapshot_json FROM character_saves WHERE account_id=$1 AND character_index=0",&[&self.account]).unwrap().get(0)).unwrap()
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_source_postgres_atomic_checkpoint_wallet_and_immutable_replay() {
    let mut db = Fixture::new();
    let source = db.prepared();
    let memory = to_json(&*db.config.account_store.lock().unwrap()).unwrap();
    assert!(matches!(
        db.config.publish_hunting_quest_source_inner(&source, 1),
        Err(HuntingSourceError::Rejected(_))
    ));
    assert_eq!(db.counts(), (0, 0, 0));
    assert_eq!(db.durable().revision, db.before.revision);
    assert_eq!(
        memory,
        to_json(&*db.config.account_store.lock().unwrap()).unwrap()
    );
    let committed = db.config.publish_hunting_quest_source(&source).unwrap();
    assert!(!committed.credit.replayed);
    assert_eq!(committed.checkpoint.gold, db.before.gold);
    assert_eq!(db.counts(), (1, 1, 1));
    assert_eq!(db.bound(), GOLD as i64);
    assert_eq!(db.durable().revision, db.before.revision + 1);
    // A later ordinary complete save must survive recovery of an old receipt.
    db.config
        .commit_account_store_transaction(&[db.account.clone()], |store| {
            let save = store
                .accounts
                .get_mut(&db.account)
                .unwrap()
                .saves
                .get_mut(&0)
                .unwrap();
            save.revision += 1;
            save.credit += 9;
            Ok(())
        })
        .unwrap();
    let replay = db.config.publish_hunting_quest_source(&source).unwrap();
    assert!(replay.credit.replayed);
    assert_eq!(replay.credit.receipt, committed.credit.receipt);
    assert_eq!(replay.checkpoint.revision, db.before.revision + 2);
    assert_eq!(replay.checkpoint.credit, db.before.credit + 9);
    assert_eq!(db.counts(), (1, 1, 1));
    assert_eq!(db.bound(), GOLD as i64);
    db.client
        .execute(
            "UPDATE accounts SET is_banned=TRUE,ban_until_ms=NULL WHERE account_id=$1",
            &[&db.account],
        )
        .unwrap();
    assert!(
        matches!(
            db.config.publish_hunting_quest_source(&source),
            Err(HuntingSourceError::Rejected(_))
        ),
        "fresh authorization must precede even an existing receipt"
    );
    assert!(db
        .client
        .execute(
            "UPDATE hunting_quest_source_checkpoints SET source_digest='rewrite'",
            &[]
        )
        .is_err());
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_source_postgres_stale_cas_and_retired_incarnation_cannot_credit() {
    let mut db = Fixture::new();
    let stale = db.prepared();
    let separate = SimulationConfig::default()
        .with_postgres_account_store(&db.url)
        .unwrap();
    separate
        .commit_account_store_transaction(&[db.account.clone()], |store| {
            store
                .accounts
                .get_mut(&db.account)
                .unwrap()
                .saves
                .get_mut(&0)
                .unwrap()
                .revision += 1;
            Ok(())
        })
        .unwrap();
    assert!(matches!(
        db.config.publish_hunting_quest_source(&stale),
        Err(HuntingSourceError::Rejected(_))
    ));
    assert_eq!(db.counts(), (0, 0, 0));
    assert!(
        db.config
            .bind_hunting_character_incarnation(&db.account, &db.before)
            .is_err(),
        "StartGame cannot bind an old complete checkpoint to a newer save"
    );
    // Simulate the exact generation boundary; a source may not rebind itself.
    db.client.execute("UPDATE characters SET hunting_incarnation=gen_random_uuid() WHERE account_id=$1 AND character_index=0",&[&db.account]).unwrap();
    assert!(matches!(
        db.config.publish_hunting_quest_source(&stale),
        Err(HuntingSourceError::Rejected(_))
    ));
    assert_eq!(db.counts(), (0, 0, 0));
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_source_postgres_two_independent_instances_credit_one_lifetime_slot() {
    let mut db = Fixture::new();
    let second = SimulationConfig::default()
        .with_postgres_account_store(&db.url)
        .unwrap()
        .with_hunting_economy_realm(&db.realm)
        .unwrap();
    let first_source = db.prepared();
    let second_source = second
        .prepare_hunting_quest_source(
            &db.account,
            &db.incarnation,
            QUEST,
            HASH,
            GOLD,
            db.before.clone(),
            stage_done(db.account.clone(), db.before.clone()),
        )
        .unwrap();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let first_config = db.config.clone();
    let first_barrier = barrier.clone();
    let first = std::thread::spawn(move || {
        first_barrier.wait();
        first_config
            .publish_hunting_quest_source(&first_source)
            .unwrap()
    });
    let second = std::thread::spawn(move || {
        barrier.wait();
        second.publish_hunting_quest_source(&second_source).unwrap()
    });
    let first = first.join().unwrap();
    let second = second.join().unwrap();
    assert_ne!(first.credit.replayed, second.credit.replayed);
    assert_eq!(first.credit.receipt, second.credit.receipt);
    assert_eq!(db.bound(), GOLD as i64);
    assert_eq!(db.counts(), (1, 1, 1));
    assert_eq!(db.durable().revision, db.before.revision + 1);
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_source_postgres_original_unknown_recovery_and_other_fence_isolation() {
    let mut db = Fixture::new();
    let source = db.prepared();
    assert!(matches!(
        db.config.publish_hunting_quest_source_inner(&source, 2),
        Err(HuntingSourceError::OutcomeUnknown(_))
    ));
    assert!(db.config.ensure_account_store_writable().is_err());
    assert_eq!(db.bound(), GOLD as i64);
    assert!(db
        .config
        .recover_pending_hunting_quest_source("another-account", &db.incarnation, 0, QUEST)
        .unwrap()
        .is_none());
    assert!(db
        .config
        .recover_pending_hunting_quest_source(&db.account, "pg-character-v1:retired", 0, QUEST)
        .unwrap()
        .is_none());
    assert!(db
        .config
        .recover_pending_hunting_quest_source(&db.account, &db.incarnation, 0, QUEST + 1)
        .unwrap()
        .is_none());
    let mut rebound = db.config.clone();
    rebound.account_store_database_url =
        Some("postgres://fixture@127.0.0.1:1/mir2_hunting_acceptance_wrong_binding".into());
    assert!(
        rebound
            .recover_pending_hunting_quest_source(&db.account, &db.incarnation, 0, QUEST)
            .unwrap()
            .is_none(),
        "a changed repository binding cannot recover another configuration's source"
    );
    assert!(db
        .config
        .recover_hunting_quest_source(&db.account, &db.incarnation, 0, QUEST, "wrong-digest", GOLD)
        .is_err());
    let recovered = db
        .config
        .recover_pending_hunting_quest_source(&db.account, &db.incarnation, 0, QUEST)
        .unwrap()
        .unwrap();
    assert!(recovered.credit.replayed);
    assert_eq!(recovered.checkpoint.revision, db.before.revision + 1);
    assert!(db.config.ensure_account_store_writable().is_ok());
    assert_eq!(db.counts(), (1, 1, 1));
    let mut other = Fixture::new();
    let source = other.prepared();
    assert!(matches!(
        other.config.publish_hunting_quest_source_inner(&source, 2),
        Err(HuntingSourceError::OutcomeUnknown(_))
    ));
    other
        .config
        .freeze_account_store_writes("unrelated subsystem freeze".into());
    assert!(other
        .config
        .recover_pending_hunting_quest_source(&other.account, &other.incarnation, 0, QUEST)
        .unwrap()
        .is_none());
    assert!(other
        .config
        .recover_hunting_quest_source(&other.account, &other.incarnation, 0, QUEST, HASH, GOLD)
        .is_err());
    assert!(other.config.ensure_account_store_writable().is_err());
    assert_eq!(other.bound(), GOLD as i64);
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_source_postgres_other_freeze_before_unknown_fence_keeps_original_owner() {
    let mut db = Fixture::new();
    let source = db.prepared();
    let before = to_json(&*db.config.account_store.lock().unwrap()).unwrap();
    assert!(matches!(
        db.config.publish_hunting_quest_source_inner(&source, 10),
        Err(HuntingSourceError::OutcomeUnknown(_))
    ));
    assert_eq!(
        db.counts(),
        (1, 1, 1),
        "fault runs only after the real atomic COMMIT"
    );
    assert_eq!(db.bound(), GOLD as i64);
    assert_eq!(db.durable().revision, db.before.revision + 1);
    assert!(
        db.config.hunting_source_fence.lock().unwrap().is_none(),
        "the committed source must not claim the earlier unrelated freeze"
    );
    let reason = match &*db.config.account_store_write_state.lock().unwrap() {
        AccountStoreWriteState::Frozen { reason } => reason.clone(),
        AccountStoreWriteState::Writable => panic!("unrelated freeze was lost"),
    };
    assert!(reason.contains("unrelated failure before source fencing"));
    assert!(db
        .config
        .recover_pending_hunting_quest_source(&db.account, &db.incarnation, 0, QUEST)
        .unwrap()
        .is_none());
    assert!(db
        .config
        .recover_hunting_quest_source(&db.account, &db.incarnation, 0, QUEST, HASH, GOLD)
        .is_err());
    assert!(db.config.ensure_account_store_writable().is_err());
    assert!(
        matches!(&*db.config.account_store_write_state.lock().unwrap(),
        AccountStoreWriteState::Frozen { reason: current } if current == &reason)
    );
    assert_eq!(
        before,
        to_json(&*db.config.account_store.lock().unwrap()).unwrap(),
        "an unrelated freeze cannot be cleared by publishing this source's mirror"
    );
    assert!(db
        .config
        .commit_account_store_transaction(&[db.account.clone()], |_| Ok(()))
        .is_err());
    assert_eq!(db.counts(), (1, 1, 1));
    assert_eq!(db.bound(), GOLD as i64);
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_source_postgres_known_commit_keeps_receipt_and_absent_unknown_mints_nothing() {
    let mut db = Fixture::new();
    let source = db.prepared();
    let error = db
        .config
        .publish_hunting_quest_source_inner(&source, 3)
        .unwrap_err();
    match error {
        HuntingSourceError::PostCommit { commit, .. } => {
            assert!(!commit.credit.replayed);
            assert_eq!(commit.credit.receipt.bound_gold_credit, GOLD);
        }
        other => panic!("wrong failure class: {other:?}"),
    }
    assert_eq!(db.counts(), (1, 1, 1));
    assert_eq!(db.bound(), GOLD as i64);
    assert!(db.config.ensure_account_store_writable().is_err());
    let mut absent = Fixture::new();
    let source = absent.prepared();
    assert!(matches!(
        absent.config.publish_hunting_quest_source_inner(&source, 4),
        Err(HuntingSourceError::OutcomeUnknown(_))
    ));
    assert!(absent
        .config
        .recover_hunting_quest_source(&absent.account, &absent.incarnation, 0, QUEST, HASH, GOLD)
        .unwrap()
        .is_none());
    assert_eq!(absent.counts(), (0, 0, 0));
    assert!(absent.config.ensure_account_store_writable().is_ok());
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_source_postgres_worker_result_loss_fences_old_saves_and_recovers_original() {
    for fault in [5, 9] {
        let mut db = Fixture::new();
        let source = db.prepared();
        let before = to_json(&*db.config.account_store.lock().unwrap()).unwrap();
        assert!(
            matches!(
                db.config
                    .publish_hunting_quest_source_with_test_fault(&source, fault),
                Err(HuntingSourceError::OutcomeUnknown(_))
            ),
            "an outer worker failure is never a definite rejection"
        );
        assert_eq!(
            before,
            to_json(&*db.config.account_store.lock().unwrap()).unwrap(),
            "no undelivered outcome may publish its staged mirror"
        );
        assert!(
            db.config
                .commit_account_store_transaction(&[db.account.clone()], |store| {
                    store
                        .accounts
                        .get_mut(&db.account)
                        .unwrap()
                        .saves
                        .insert(0, db.before.clone());
                    Ok(())
                })
                .is_err(),
            "an old complete save cannot overwrite an uncertain commit"
        );
        let recovered = db
            .config
            .recover_hunting_quest_source(&db.account, &db.incarnation, 0, QUEST, HASH, GOLD)
            .unwrap();
        if fault == 5 {
            let recovered =
                recovered.expect("the worker actually committed before its result was lost");
            assert!(recovered.credit.replayed);
            assert_eq!(recovered.checkpoint.revision, db.before.revision + 1);
            assert_eq!(db.bound(), GOLD as i64);
            assert_eq!(db.counts(), (1, 1, 1));
        } else {
            assert!(recovered.is_none());
            assert_eq!(db.counts(), (0, 0, 0));
        }
        assert!(db.config.ensure_account_store_writable().is_ok());
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_source_postgres_known_receipt_survives_missing_current_projection() {
    let mut fresh = Fixture::new();
    let source = fresh.prepared();
    match fresh
        .config
        .publish_hunting_quest_source_inner(&source, 6)
        .unwrap_err()
    {
        HuntingSourceError::KnownCommitted { credit, .. } => {
            assert!(!credit.replayed);
            assert_eq!(credit.receipt.bound_gold_credit, GOLD);
        }
        other => panic!("known receipt was discarded: {other:?}"),
    }
    let recovered = fresh
        .config
        .recover_hunting_quest_source(&fresh.account, &fresh.incarnation, 0, QUEST, HASH, GOLD)
        .unwrap()
        .unwrap();
    assert_eq!(recovered.checkpoint.revision, fresh.before.revision + 1);
    assert_eq!(fresh.counts(), (1, 1, 1));
    let mut replay = Fixture::new();
    let source = replay.prepared();
    let original = replay.config.publish_hunting_quest_source(&source).unwrap();
    replay
        .config
        .commit_account_store_transaction(&[replay.account.clone()], |store| {
            let save = store
                .accounts
                .get_mut(&replay.account)
                .unwrap()
                .saves
                .get_mut(&0)
                .unwrap();
            save.revision += 1;
            save.credit += 9;
            Ok(())
        })
        .unwrap();
    let latest = to_json(&*replay.config.account_store.lock().unwrap()).unwrap();
    match replay
        .config
        .publish_hunting_quest_source_inner(&source, 7)
        .unwrap_err()
    {
        HuntingSourceError::KnownCommitted { credit, .. } => {
            assert!(credit.replayed);
            assert_eq!(credit.receipt, original.credit.receipt);
        }
        other => panic!("replay receipt was discarded: {other:?}"),
    }
    assert_eq!(
        latest,
        to_json(&*replay.config.account_store.lock().unwrap()).unwrap(),
        "missing current projection must never publish the original old source checkpoint"
    );
    let recovered = replay
        .config
        .recover_hunting_quest_source(&replay.account, &replay.incarnation, 0, QUEST, HASH, GOLD)
        .unwrap()
        .unwrap();
    assert_eq!(recovered.checkpoint.revision, replay.before.revision + 2);
    assert_eq!(recovered.checkpoint.credit, replay.before.credit + 9);
    assert_eq!(replay.counts(), (1, 1, 1));
    assert_eq!(replay.bound(), GOLD as i64);
}
