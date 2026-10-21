//! Real PG source/claim atomicity. Clock boundary fixtures can modify only the
//! private, non-Serde capability; production callers cannot choose a period.
use super::*;
use postgres::{Client, NoTls};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

const QUEST: i32 = 2_100_001;
const GOLD: u64 = 3_000;
const DIGEST: &str = "trusted-daily-definition-v1";

fn state(id: i32, stage: &str, claimed: Option<i64>, high: Option<i64>) -> String {
    json!({"quest_id":id,"title":"Daily commission","summary":"","reward_preview":"",
        "required":10,"current":10,"stage":stage,"task_progress":{},
        "cadence_last_claimed_period":claimed,"cadence_high_watermark_period":high})
    .to_string()
}

fn completed(mut before: CharacterSaveRecord, quest: i32, period: i64) -> CharacterSaveRecord {
    before.revision += 1;
    for row in &mut before.quest_states_json {
        let mut value: Value = serde_json::from_str(row).unwrap();
        if value["quest_id"].as_i64() == Some(i64::from(quest)) {
            value["stage"] = json!("completed");
            value["cadence_last_claimed_period"] = json!(period);
            value["cadence_high_watermark_period"] = json!(period);
            *row = value.to_string();
        }
    }
    before
}

#[test]
fn hunting_daily_source_clock_keys_and_legacy_group_guards() {
    let midnight_utc = 20_000 * DAY_MS - UTC8_MS;
    assert_eq!(period_at(midnight_utc - 1).unwrap(), 19_999);
    assert_eq!(period_at(midnight_utc).unwrap(), 20_000);
    assert_eq!(source_key(7), "quest-once-v1:7");
    assert_eq!(slot_source_key(7, QuestSourceSlot::Once), "quest-once-v1:7");
    assert_eq!(
        slot_source_key(QUEST, QuestSourceSlot::Daily { period: 20_000 }),
        "quest-daily-v1:2100001:20000"
    );
    assert_ne!(
        slot_source_key(QUEST, QuestSourceSlot::Daily { period: 20_000 }),
        slot_source_key(QUEST, QuestSourceSlot::Daily { period: 20_001 })
    );
    let mut before = CharacterSaveRecord::new(SimulationConfig::default().default_character);
    before.quest_states_json = vec![state(QUEST, "readyToTurnIn", Some(19_999), Some(20_000))];
    assert!(validate_claimable(&before, QUEST, 20_000).is_ok());
    before
        .quest_states_json
        .push(state(2_100_002, "completed", None, Some(20_000)));
    assert!(validate_claimable(&before, QUEST, 20_000).is_err());
    before.quest_states_json[1] = state(2_100_002, "available", None, Some(20_001));
    assert!(validate_claimable(&before, QUEST, 20_000).is_err());
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
        let base = std::env::var("MIR2_ECONOMY_TEST_DATABASE_URL")
            .expect("ignored daily acceptance requires MIR2_ECONOMY_TEST_DATABASE_URL");
        assert!(base.starts_with("postgres://") || base.starts_with("postgresql://"));
        let parsed: postgres::Config = base
            .parse()
            .unwrap_or_else(|_| panic!("invalid isolated database"));
        assert!(parsed
            .get_dbname()
            .is_some_and(|name| name.starts_with("mir2_hunting_acceptance_")));
        assert!(!parsed.get_hosts().is_empty() && parsed.get_hosts().iter().all(|host|
            matches!(host,postgres::config::Host::Tcp(name) if name.eq_ignore_ascii_case("localhost") || name.parse::<std::net::IpAddr>().is_ok_and(|ip|ip.is_loopback()))));
        assert!(parsed
            .get_hostaddrs()
            .iter()
            .all(std::net::IpAddr::is_loopback));
        let nonce = format!(
            "{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let schema = format!("hunting_daily_{nonce}");
        let mut bootstrap = Client::connect(&base, NoTls)
            .unwrap_or_else(|_| panic!("isolated database unavailable"));
        bootstrap
            .batch_execute(&format!("CREATE SCHEMA {schema}"))
            .unwrap();
        let separator = if base.contains('?') { '&' } else { '?' };
        let url = format!("{base}{separator}options=-csearch_path%3D{schema}");
        let mut client =
            Client::connect(&url, NoTls).unwrap_or_else(|_| panic!("isolated schema unavailable"));
        assert_eq!(
            client
                .query_one("SELECT current_schema()", &[])
                .unwrap()
                .get::<_, String>(0),
            schema
        );
        crate::apply_migrations(&mut client).unwrap();
        // Re-running the new SQL is safe and keeps both immutable constraints.
        client
            .batch_execute(include_str!(
                "../../../infra/postgres/migrations/0025_hunting_daily_quest_sources.sql"
            ))
            .unwrap();
        let account = "dailyfixture".to_owned();
        let mut store = AccountStore::new(SimulationConfig::default().default_character);
        let mut record = store.accounts.remove("demo").unwrap();
        let before = record.saves.get_mut(&0).unwrap();
        before.revision = 5;
        before.gold = 123;
        before.credit = 17;
        before.quest_states_json = vec![state(QUEST, "readyToTurnIn", None, None)];
        let before = before.clone();
        store.accounts.insert(account.clone(), record);
        PostgresAccountStoreRepository::new(&url, AccountStoreDatabaseMode::SourceOfTruth)
            .save(&store)
            .unwrap();
        let realm = format!("daily-{nonce}");
        let policy = serde_json::from_value(Self::policy_json("daily-v1", true)).unwrap();
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
    fn policy_json(version: &str, enabled: bool) -> Value {
        json!({"schema_version":1,"policy_version":version,"enabled":enabled,
            "k":{"numerator":1,"denominator":10},"conversion_fee":{"numerator":1,"denominator":50},
            "daily_decay":{"numerator":1,"denominator":150},"baseline_daily_mint_atoms":720000,
            "daily_mint_cap_multiplier":{"numerator":3,"denominator":2},"business_timezone":"Asia/Shanghai","settlement_hour":4})
    }
    fn cold(&self) -> SimulationConfig {
        SimulationConfig::default()
            .with_postgres_account_store(&self.url)
            .unwrap()
            .with_hunting_economy_realm(&self.realm)
            .unwrap()
    }
    fn cap(&self) -> HuntingDailyQuestPeriod {
        self.config
            .read_hunting_daily_quest_period(&self.account, &self.incarnation, QUEST, &self.before)
            .unwrap()
    }
    fn prepare(
        &self,
        config: &SimulationConfig,
        cap: &HuntingDailyQuestPeriod,
        before: CharacterSaveRecord,
        digest: &str,
        amount: u64,
    ) -> PreparedHuntingQuestSource {
        let account = self.account.clone();
        let after = completed(before.clone(), cap.quest_id, cap.period());
        config
            .prepare_hunting_daily_quest_source(cap, digest, amount, before, move |store| {
                store
                    .accounts
                    .get_mut(&account)
                    .unwrap()
                    .saves
                    .insert(0, after);
                Ok(())
            })
            .unwrap()
    }
    fn counts(&mut self) -> (i64, i64, i64, i64) {
        let row = self.client.query_one("SELECT
            (SELECT count(*) FROM hunting_economy_wallets WHERE realm_id=$1),
            (SELECT count(*) FROM hunting_economy_operations WHERE realm_id=$1 AND operation_kind='bound_credit'),
            (SELECT count(*) FROM hunting_daily_quest_source_checkpoints WHERE realm_id=$1),
            (SELECT count(*) FROM hunting_economy_outbox WHERE realm_id=$1 AND event_kind='bound_credit')",&[&self.realm]).unwrap();
        (row.get(0), row.get(1), row.get(2), row.get(3))
    }
    fn durable(&mut self) -> CharacterSaveRecord {
        serde_json::from_value(self.client.query_one("SELECT snapshot_json FROM character_saves WHERE account_id=$1 AND character_index=0",&[&self.account]).unwrap().get(0)).unwrap()
    }
    fn save_before(&mut self, before: CharacterSaveRecord) {
        let account = self.account.clone();
        self.config
            .commit_account_store_transaction(&[account.clone()], |store| {
                store
                    .accounts
                    .get_mut(&account)
                    .unwrap()
                    .saves
                    .insert(0, before.clone());
                Ok(())
            })
            .unwrap();
        self.before = before;
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_daily_source_postgres_atomic_rollback_receipt_and_immutable_claim_slot() {
    let mut db = Fixture::new();
    let cap = db.cap();
    let source = db.prepare(&db.config, &cap, db.before.clone(), DIGEST, GOLD);
    let mut changed = db.prepare(
        &db.config,
        &cap,
        db.before.clone(),
        "changed-reward-v2",
        GOLD + 1,
    );
    assert!(matches!(
        db.config.publish_hunting_quest_source_inner(&source, 1),
        Err(HuntingSourceError::Rejected(_))
    ));
    assert_eq!(db.counts(), (0, 0, 0, 0));
    assert_eq!(db.durable().revision, db.before.revision);
    let original = db.config.publish_hunting_quest_source(&source).unwrap();
    assert!(!original.credit.replayed);
    assert_eq!(
        original.credit.receipt.source_key,
        slot_source_key(
            QUEST,
            QuestSourceSlot::Daily {
                period: cap.period()
            }
        )
    );
    assert_eq!(db.counts(), (1, 1, 1, 1));
    assert_eq!(db.durable().gold, db.before.gold);
    let replay = db.config.publish_hunting_quest_source(&source).unwrap();
    assert!(replay.credit.replayed);
    assert_eq!(replay.credit.receipt, original.credit.receipt);
    assert!(matches!(
        db.config.publish_hunting_quest_source(&changed),
        Err(HuntingSourceError::Rejected(_))
    ));
    changed.source_digest = DIGEST.into();
    assert!(matches!(
        db.config.publish_hunting_quest_source(&changed),
        Err(HuntingSourceError::Rejected(_))
    ));
    assert_eq!(db.counts(), (1, 1, 1, 1));
    assert!(db
        .client
        .execute(
            "UPDATE hunting_daily_quest_source_checkpoints SET source_digest='rewrite'",
            &[]
        )
        .is_err());
    assert!(db
        .client
        .execute("DELETE FROM hunting_daily_quest_source_checkpoints", &[])
        .is_err());
    let row = db.client.query_one("SELECT claim_period,cadence,source_digest,bound_gold FROM hunting_daily_quest_source_checkpoints",&[]).unwrap();
    assert_eq!(row.get::<_, i64>("claim_period"), cap.period());
    assert_eq!(row.get::<_, String>("cadence"), "daily-utc8-v1");
    assert_eq!(row.get::<_, String>("source_digest"), DIGEST);
    assert_eq!(row.get::<_, i64>("bound_gold"), GOLD as i64);
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_daily_source_postgres_two_independent_sessions_share_one_daily_slot() {
    let mut db = Fixture::new();
    let second = db.cold();
    let first_cap = db.cap();
    let second_cap = second
        .read_hunting_daily_quest_period(&db.account, &db.incarnation, QUEST, &db.before)
        .unwrap();
    let first_source = db.prepare(&db.config, &first_cap, db.before.clone(), DIGEST, GOLD);
    let second_source = db.prepare(&second, &second_cap, db.before.clone(), DIGEST, GOLD);
    let first_config = db.config.clone();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let other_barrier = barrier.clone();
    let first = std::thread::spawn(move || {
        other_barrier.wait();
        first_config
            .publish_hunting_quest_source(&first_source)
            .unwrap()
    });
    let other = std::thread::spawn(move || {
        barrier.wait();
        second.publish_hunting_quest_source(&second_source).unwrap()
    });
    let first = first.join().unwrap();
    let second = other.join().unwrap();
    assert_ne!(first.credit.replayed, second.credit.replayed);
    assert_eq!(first.credit.receipt, second.credit.receipt);
    assert_eq!(db.counts(), (1, 1, 1, 1));
    assert_eq!(db.durable().revision, db.before.revision + 1);
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_daily_source_postgres_stale_period_returns_typed_replan_without_writes() {
    let mut db = Fixture::new();
    let mut stale = db.cap();
    // Simulate a capability issued before civil midnight using test-private
    // state. The actual S validator still queries the real locked DB clock.
    stale.period -= 1;
    let source = db.prepare(&db.config, &stale, db.before.clone(), DIGEST, GOLD);
    assert!(matches!(
        db.config.publish_hunting_quest_source(&source),
        Err(HuntingSourceError::PeriodChanged)
    ));
    assert_eq!(db.counts(), (0, 0, 0, 0));
    assert!(db.config.ensure_account_store_writable().is_ok());
    let fresh = db.cap();
    let source = db.prepare(&db.config, &fresh, db.before.clone(), DIGEST, GOLD);
    assert!(matches!(
        db.config.publish_hunting_quest_source_inner(&source, 11),
        Err(HuntingSourceError::PeriodChanged)
    ));
    assert_eq!(db.counts(), (0, 0, 0, 0));
    db.config.publish_hunting_quest_source(&source).unwrap();
    assert_eq!(db.counts(), (1, 1, 1, 1));
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_daily_source_postgres_canonical_group_legacy_future_and_claimed_guards() {
    for variant in [
        "legacy-target",
        "legacy-peer",
        "future-target",
        "future-peer",
        "claimed-target",
        "two-options",
    ] {
        let mut db = Fixture::new();
        let period = db.cap().period();
        let mut canonical = db.before.clone();
        canonical.revision += 1;
        canonical.quest_states_json = match variant {
            "legacy-target" => vec![state(QUEST, "completed", None, None)],
            "legacy-peer" => vec![
                state(QUEST, "readyToTurnIn", None, None),
                state(2_100_002, "completed", None, None),
            ],
            "future-target" => vec![state(QUEST, "readyToTurnIn", None, Some(period + 1))],
            "future-peer" => vec![
                state(QUEST, "readyToTurnIn", None, None),
                state(2_100_004, "available", None, Some(period + 1)),
            ],
            "claimed-target" => vec![state(QUEST, "completed", Some(period), Some(period))],
            _ => vec![
                state(QUEST, "readyToTurnIn", None, None),
                state(2_100_002, "completed", Some(period), Some(period)),
                state(2_100_003, "completed", Some(period), Some(period)),
            ],
        };
        db.save_before(canonical);
        let mut live = db.before.clone();
        live.quest_states_json = vec![state(QUEST, "readyToTurnIn", None, None)];
        assert!(
            db.config
                .read_hunting_daily_quest_period(&db.account, &db.incarnation, QUEST, &live)
                .is_err(),
            "canonical {variant} cannot be erased by a live OS refresh"
        );
        assert_eq!(db.counts(), (0, 0, 0, 0));
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_daily_source_postgres_prior_period_and_bonus_use_canonical_current_group() {
    let mut db = Fixture::new();
    let period = db.cap().period();
    let mut canonical = db.before.clone();
    canonical.revision += 1;
    canonical.quest_states_json = vec![state(
        QUEST,
        "completed",
        Some(period - 1),
        Some(period - 1),
    )];
    db.save_before(canonical);
    let mut live = db.before.clone();
    live.quest_states_json = vec![state(
        QUEST,
        "readyToTurnIn",
        Some(period - 1),
        Some(period),
    )];
    let cap = db
        .config
        .read_hunting_daily_quest_period(&db.account, &db.incarnation, QUEST, &live)
        .unwrap();
    let source = db.prepare(&db.config, &cap, live, DIGEST, GOLD);
    db.config.publish_hunting_quest_source(&source).unwrap();
    assert_eq!(db.counts(), (1, 1, 1, 1));
    let mut bonus = db.durable();
    bonus.revision += 1;
    bonus
        .quest_states_json
        .push(state(2_100_004, "readyToTurnIn", None, Some(period)));
    db.save_before(bonus);
    assert!(db
        .config
        .read_hunting_daily_quest_period(&db.account, &db.incarnation, 2_100_004, &db.before)
        .is_err());
    let mut second_done = db.before.clone();
    second_done.revision += 1;
    second_done
        .quest_states_json
        .push(state(2_100_002, "completed", Some(period), Some(period)));
    db.save_before(second_done);
    let cap = db
        .config
        .read_hunting_daily_quest_period(&db.account, &db.incarnation, 2_100_004, &db.before)
        .unwrap();
    let source = db.prepare(&db.config, &cap, db.before.clone(), "daily-bonus-v1", 5_000);
    db.config.publish_hunting_quest_source(&source).unwrap();
    assert_eq!(db.counts(), (1, 2, 2, 2));
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_daily_source_postgres_capability_cas_generation_and_epoch_are_bound() {
    let mut db = Fixture::new();
    let cap = db.cap();
    let other = db.cold();
    assert!(other
        .prepare_hunting_daily_quest_source(&cap, DIGEST, GOLD, db.before.clone(), |_| Ok(()))
        .is_err());
    let mut wrong_before = db.before.clone();
    wrong_before.credit += 1;
    assert!(db
        .config
        .prepare_hunting_daily_quest_source(&cap, DIGEST, GOLD, wrong_before, |_| Ok(()))
        .is_err());
    let source = db.prepare(&db.config, &cap, db.before.clone(), DIGEST, GOLD);
    other
        .commit_account_store_transaction(&[db.account.clone()], |store| {
            let save = store
                .accounts
                .get_mut(&db.account)
                .unwrap()
                .saves
                .get_mut(&0)
                .unwrap();
            save.revision += 1;
            save.credit += 7;
            Ok(())
        })
        .unwrap();
    assert!(matches!(
        db.config.publish_hunting_quest_source(&source),
        Err(HuntingSourceError::Rejected(_))
    ));
    assert_eq!(db.counts(), (0, 0, 0, 0));
    db.client.execute("UPDATE characters SET hunting_incarnation=gen_random_uuid() WHERE account_id=$1 AND character_index=0",&[&db.account]).unwrap();
    assert!(matches!(
        db.config.publish_hunting_quest_source(&source),
        Err(HuntingSourceError::Rejected(_))
    ));
    assert_eq!(db.counts(), (0, 0, 0, 0));
    db.config
        .freeze_account_store_writes("foreign epoch".into());
    assert!(db
        .config
        .prepare_hunting_daily_quest_source(&cap, DIGEST, GOLD, db.before.clone(), |_| Ok(()))
        .is_err());
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_daily_source_postgres_original_slot_recovers_disabled_policy_and_lost_session() {
    for fault in [2, 4] {
        let mut db = Fixture::new();
        let cap = db.cap();
        let source = db.prepare(&db.config, &cap, db.before.clone(), DIGEST, GOLD);
        assert!(matches!(
            db.config.publish_hunting_quest_source_inner(&source, fault),
            Err(HuntingSourceError::OutcomeUnknown(_))
        ));
        let fence = db
            .config
            .hunting_source_fence
            .lock()
            .unwrap()
            .clone()
            .unwrap();
        assert_eq!(
            fence.slot,
            QuestSourceSlot::Daily {
                period: cap.period()
            }
        );
        let policy = serde_json::from_value(Fixture::policy_json("daily-disabled", false)).unwrap();
        economy::register_policy(&mut db.client, &db.realm, &policy).unwrap();
        economy::activate_policy(&mut db.client, &db.realm, &policy.policy_version, false).unwrap();
        let new_config = db.config.clone();
        assert!(crate::SimulationSession::standard_login_recovery_preflight(
            &new_config,
            &db.account,
            &default_account_password()
        )
        .unwrap());
        assert!(new_config.ensure_account_store_writable().is_ok());
        assert!(new_config.hunting_source_fence.lock().unwrap().is_none());
        let n = i64::from(fault == 2);
        assert_eq!(db.counts(), (n, n, n, n));
        assert_eq!(db.durable().gold, db.before.gold);
        if fault == 2 {
            let replay = new_config.publish_hunting_quest_source(&source).unwrap();
            assert!(replay.credit.replayed);
            assert_eq!(
                replay.credit.receipt.source_key,
                slot_source_key(
                    QUEST,
                    QuestSourceSlot::Daily {
                        period: cap.period()
                    }
                )
            );
        }
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_daily_source_postgres_unknown_cannot_release_foreign_freeze() {
    let mut db = Fixture::new();
    let cap = db.cap();
    let source = db.prepare(&db.config, &cap, db.before.clone(), DIGEST, GOLD);
    assert!(matches!(
        db.config.publish_hunting_quest_source_inner(&source, 2),
        Err(HuntingSourceError::OutcomeUnknown(_))
    ));
    db.config
        .freeze_account_store_writes("foreign source projection".into());
    let epoch = db.config.account_store_freeze_epoch.load(Ordering::Acquire);
    assert!(db
        .config
        .refresh_account_store_account(&db.account)
        .is_err());
    assert!(db.config.ensure_account_store_writable().is_err());
    assert_eq!(
        db.config.account_store_freeze_epoch.load(Ordering::Acquire),
        epoch
    );
    assert_eq!(db.counts(), (1, 1, 1, 1));
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_daily_source_postgres_original_period_recovery_ignores_later_cadence() {
    let mut db = Fixture::new();
    let cap = db.cap();
    let source = db.prepare(&db.config, &cap, db.before.clone(), DIGEST, GOLD);
    assert!(matches!(
        db.config.publish_hunting_quest_source_inner(&source, 2),
        Err(HuntingSourceError::OutcomeUnknown(_))
    ));
    // A later canonical checkpoint contains a newer cadence watermark. This
    // fixture does not pretend to advance the actual PostgreSQL wall clock:
    // it proves historical recovery cannot derive its slot from current state.
    db.cold()
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
            save.quest_states_json = vec![state(
                QUEST,
                "completed",
                Some(cap.period() + 1),
                Some(cap.period() + 1),
            )];
            Ok(())
        })
        .unwrap();
    let current = db.durable();
    let recovered = db
        .config
        .recover_hunting_quest_source(&db.account, &db.incarnation, 0, QUEST, DIGEST, GOLD)
        .unwrap()
        .unwrap();
    assert!(recovered.credit.replayed);
    assert_eq!(
        recovered.credit.receipt.source_key,
        slot_source_key(
            QUEST,
            QuestSourceSlot::Daily {
                period: cap.period()
            }
        )
    );
    assert_eq!(recovered.checkpoint.revision, current.revision);
    assert_eq!(recovered.checkpoint.credit, current.credit);
    assert_eq!(
        recovered.checkpoint.quest_states_json,
        current.quest_states_json
    );
    assert!(db.config.ensure_account_store_writable().is_ok());
    assert!(
        db.config
            .read_hunting_daily_quest_period(&db.account, &db.incarnation, QUEST, &current,)
            .is_err(),
        "a historical receipt grants no future fresh-claim permission"
    );
    assert_eq!(db.counts(), (1, 1, 1, 1));
    // History has no FK/cascade from the deletable account or character.
    db.client
        .execute("DELETE FROM accounts WHERE account_id=$1", &[&db.account])
        .unwrap();
    assert_eq!(db.counts(), (1, 1, 1, 1));
}
