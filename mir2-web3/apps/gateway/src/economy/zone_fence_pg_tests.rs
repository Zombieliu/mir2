//! Real PostgreSQL economy authority checks. Every configured fixture is an
//! explicit disposable loopback database; no default/production fallback.
use super::*;
use mir2_simulation::{AccountStoreDatabaseMode, SimulationConfig, ZoneSaveFence};
use postgres::Config as PostgresConfig;
use serde_json::Value;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

static PG_LOCK: Mutex<()> = Mutex::new(());
fn checked_url(value: &str) -> Result<PostgresConfig, ()> {
    if !value.starts_with("postgres://") && !value.starts_with("postgresql://") {
        return Err(());
    }
    let parsed: PostgresConfig = value.parse().map_err(|_| ())?;
    if parsed.get_hosts().len() != 1
        || parsed.get_ports().len() != 1
        || matches!(parsed.get_ports()[0], 0 | 5432)
        || parsed.get_options().is_some()
        || !parsed
            .get_dbname()
            .and_then(|name| name.strip_prefix("mir2_zone_lease_test_"))
            .is_some_and(|suffix| {
                !suffix.is_empty()
                    && suffix
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'_')
            })
    {
        return Err(());
    }
    match &parsed.get_hosts()[0] {
        postgres::config::Host::Tcp(host)
            if host
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback()) =>
        {
            Ok(parsed)
        }
        _ => Err(()),
    }
}
fn connect(url: &str) -> Client {
    let mut parsed: PostgresConfig = url.parse().unwrap();
    parsed
        .connect_timeout(Duration::from_secs(2))
        .notice_callback(|_| {});
    parsed
        .connect(NoTls)
        .unwrap_or_else(|_| panic!("configured economy Zone fence PostgreSQL fixture unavailable"))
}
struct Database {
    url: String,
    scoped: String,
    schema: String,
    zone: String,
}
impl Database {
    fn from_env() -> Option<Self> {
        let url = match std::env::var("MIR2_TEST_ZONE_SAVE_OPERATION_URL") {
            Ok(url) => url,
            Err(std::env::VarError::NotPresent) => return None,
            Err(_) => panic!("economy Zone fixture URL must be Unicode"),
        };
        checked_url(&url)
            .unwrap_or_else(|_| panic!("unsafe configured economy Zone fence fixture"));
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let schema = format!("economy_zone_{}_{stamp}", std::process::id());
        let separator = if url.contains('?') { '&' } else { '?' };
        let scoped = format!(
            "{url}{separator}options=-c%20search_path%3D{schema}&application_name={schema}"
        );
        let db = Self {
            url,
            scoped,
            schema: schema.clone(),
            zone: format!("{schema}:zone"),
        };
        let mut admin = db.admin();
        admin
            .batch_execute(include_str!(
                "../../../../infra/postgres/migrations/0003_zone_owner_leases.sql"
            ))
            .unwrap();
        admin
            .batch_execute(&format!("CREATE SCHEMA {schema}"))
            .unwrap();
        let mut scoped = db.connect();
        // Provisioning is fixture-only. Fenced economy workers must not DDL.
        for (version, sql) in mir2_simulation::MIGRATIONS {
            if *version != "0003_zone_owner_leases" {
                scoped.batch_execute(sql).unwrap();
            }
        }
        db.lease("owner-a", 7, 120000);
        Some(db)
    }
    fn admin(&self) -> Client {
        connect(&self.url)
    }
    fn connect(&self) -> Client {
        connect(&self.scoped)
    }
    fn store(&self) -> PostgresEconomyStore {
        PostgresEconomyStore::new(&self.scoped)
    }
    fn config(&self) -> SimulationConfig {
        self.config_for(&self.scoped)
    }
    fn config_for(&self, url: &str) -> SimulationConfig {
        let mut config = SimulationConfig::default();
        config.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;
        config.account_store_database_url = Some(url.into());
        config
            .with_zone_save_fence(
                ZoneSaveFence::new(self.zone.clone(), "owner-a".into(), 7).unwrap(),
            )
            .unwrap()
    }
    fn lease(&self, owner: &str, token: i64, ttl: i64) {
        self.admin().execute("INSERT INTO public.zone_owner_leases(zone_id,owner_id,fencing_token,expires_at_ms,heartbeat_at_ms) \
            VALUES($1,$2,$3,floor(extract(epoch FROM clock_timestamp())*1000)::bigint+$4,1) \
            ON CONFLICT(zone_id) DO UPDATE SET owner_id=$2,fencing_token=$3,expires_at_ms=EXCLUDED.expires_at_ms,heartbeat_at_ms=1",
            &[&self.zone,&owner,&token,&ttl]).unwrap();
    }
    fn context(&self) -> SharedAccountInventoryExecutionContext {
        SharedAccountInventoryExecutionContext {
            zone_id: crate::ZoneId::new(&self.zone),
            fencing_generation: 7,
            source_sequence: 11,
            created_at_ms: 1000,
            external_commit_authorized: true,
        }
    }
    fn envelope(&self, name: &str) -> EconomyTransactionEnvelope {
        EconomyTransactionEnvelope {
            idempotency_key: format!("{}:{name}", self.schema),
            transaction_kind: EconomyTransactionKind::Reward,
            zone_id: self.zone.clone(),
            fencing_generation: 7,
            source_sequence: 11,
            created_at_ms: 1000,
            legs: vec![EconomyLeg {
                balance: EconomyBalanceKey::gold("fenced-a", 0),
                delta: 17,
            }],
            metadata: BTreeMap::new(),
        }
    }
    fn counts(&self) -> (i64, i64, i64, i64) {
        let row = self
            .connect()
            .query_one(
                "SELECT \
            (SELECT count(*) FROM game_economy_balances), \
            (SELECT count(*) FROM game_economy_transactions), \
            (SELECT count(*) FROM game_economy_outbox), \
            (SELECT count(*) FROM game_economy_bootstraps)",
                &[],
            )
            .unwrap();
        (row.get(0), row.get(1), row.get(2), row.get(3))
    }
    fn image(&self) -> Value {
        self.connect().query_one("SELECT jsonb_build_object( \
            'balances',(SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY account_id,character_index,asset_kind,asset_key),'[]') FROM game_economy_balances t), \
            'transactions',(SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY idempotency_key),'[]') FROM game_economy_transactions t), \
            'outbox',(SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY event_id),'[]') FROM game_economy_outbox t), \
            'bootstrap',(SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY account_id,character_index),'[]') FROM game_economy_bootstraps t))",&[]).unwrap().get(0)
    }
    fn source_image(&self) -> Value {
        self.connect().query_one("SELECT jsonb_build_object( \
            'accounts',(SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY account_id),'[]') FROM accounts t), \
            'characters',(SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY account_id,character_index),'[]') FROM characters t), \
            'saves',(SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY account_id,character_index),'[]') FROM character_saves t), \
            'guilds',(SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY guild_id),'[]') FROM shared_guilds t))",&[]).unwrap().get(0)
    }
    fn amount(&self, key: &EconomyBalanceKey) -> i64 {
        self.connect().query_opt("SELECT amount FROM game_economy_balances WHERE account_id=$1 AND character_index=$2 AND asset_kind=$3 AND asset_key=$4",
            &[&key.account_id,&key.character_index,&key.asset_kind,&key.asset_key]).unwrap().map_or(0,|row|row.get(0))
    }
    fn reject_commit(&self) {
        self.connect().batch_execute("CREATE FUNCTION economy_reject_commit() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN \
            RAISE EXCEPTION 'owned economy deferred COMMIT rollback' USING ERRCODE='23505'; END; $$; \
            CREATE CONSTRAINT TRIGGER economy_reject_commit AFTER INSERT ON game_economy_transactions \
            DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION economy_reject_commit()").unwrap();
    }
    fn allow_commit(&self) {
        self.connect().batch_execute("DROP TRIGGER economy_reject_commit ON game_economy_transactions; DROP FUNCTION economy_reject_commit()").unwrap();
    }
    fn block_commit(&self, key: i64) {
        self.connect().batch_execute(&format!("CREATE FUNCTION economy_wait_commit() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN \
            PERFORM set_config('lock_timeout','3s',true); PERFORM pg_advisory_xact_lock({key}::bigint); RETURN NEW; END; $$; \
            CREATE CONSTRAINT TRIGGER economy_wait_commit AFTER INSERT ON game_economy_transactions \
            DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION economy_wait_commit()")).unwrap();
    }
    fn gate_key(&self) -> i64 {
        self.admin()
            .query_one(
                "SELECT hashtextextended($1,0)",
                &[&format!("{}:owned-commit-gate", self.schema)],
            )
            .unwrap()
            .get(0)
    }
    fn blocked_commit_pid(&self) -> i32 {
        let deadline = Instant::now() + Duration::from_millis(700);
        let mut observer = self.admin();
        while Instant::now() < deadline {
            if let Some(row) = observer.query_opt("SELECT pid FROM pg_stat_activity WHERE datname=current_database() AND application_name=$1 AND query='COMMIT' AND wait_event_type='Lock'",&[&self.schema]).unwrap() { return row.get(0); }
            thread::sleep(Duration::from_millis(5));
        }
        panic!("real economy worker did not reach owned deferred COMMIT lock");
    }
}
impl Drop for Database {
    fn drop(&mut self) {
        let mut admin = self.admin();
        admin
            .execute(
                "DELETE FROM public.zone_owner_leases WHERE zone_id=$1",
                &[&self.zone],
            )
            .unwrap();
        admin
            .batch_execute(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .unwrap();
    }
}
fn memory_runtime(name: &str) -> InProcessWorldRuntime {
    let character = format!(
        "{}Hero",
        name.chars()
            .filter(char::is_ascii_alphanumeric)
            .collect::<String>()
    );
    super::tests::start_test_runtime(name, &character).unwrap()
}
fn seed_trade_opening_gold(db: &Database, runtime: &mut InProcessWorldRuntime) {
    // Fixture-only authoritative pickup, followed by the ordinary fenced source
    // save. Opening ledger funds must come from a real saved runtime checkpoint.
    let before = runtime.world_snapshot().gold;
    let gained = 10;
    let drop = mir2_simulation::GroundDropSnapshot {
        object_id: 0x7f00_4466,
        name: "10 Gold".into(),
        name_colour_argb: -1,
        icon: 0,
        x: 10,
        y: 20,
        quantity: gained,
        source_monster: "owned trade opening fixture".into(),
        owner_object_id: None,
        ownership_remaining_ticks: None,
        loot: GroundDropLootSnapshot::Gold { amount: gained },
    };
    assert!(
        runtime
            .commit_shared_ground_drop_pickup_transaction(&drop)
            .committed
    );
    runtime.save_active_character().unwrap();
    assert_eq!(runtime.world_snapshot().gold, before + gained);
    let identity = runtime.active_identity().unwrap();
    let saved: Value = db
        .connect()
        .query_one(
            "SELECT snapshot_json FROM character_saves WHERE account_id=$1 AND character_index=$2",
            &[&identity.account_id, &identity.character_index],
        )
        .unwrap()
        .get(0);
    assert_eq!(saved["gold"].as_u64(), Some(u64::from(before + gained)));
}
fn offer(
    identity: &ActiveSessionIdentity,
    partner: &ActiveSessionIdentity,
    nonce: &str,
    gold: u32,
) -> SharedTradeOffer {
    SharedTradeOffer {
        settlement_nonce: nonce.into(),
        account_id: identity.account_id.clone(),
        character_index: identity.character_index,
        character_name: identity.character_name.clone(),
        partner_name: partner.character_name.clone(),
        gold,
        items: Vec::new(),
    }
}

fn prepared_kill_award(
    db: &Database,
    runtime: &InProcessWorldRuntime,
    key: &str,
) -> SharedAccountInventoryCommandEnvelope {
    let identity = runtime.active_identity().unwrap();
    // A source-captured NoGuild selection exercises the production prepared
    // source without inventing Guild membership or a Guild clock engine.
    let selection = mir2_simulation::ZoneExperienceSelection {
        source_zone: db.zone.clone(),
        monster_object_id: 0x7f00_5566,
        killed_at_ms: 1000,
        account_id: identity.account_id.clone(),
        character_index: identity.character_index,
        guild: None,
        final_amount: 5,
        guild_amount: 0,
    };
    SharedAccountInventoryCommandEnvelope {
        identity,
        command: SharedAccountInventoryCommand::MonsterKillAward(
            mir2_simulation::ZoneMonsterKillAward {
                monster_object_id: selection.monster_object_id,
                killed_at_ms: selection.killed_at_ms,
                monster_name: "Scarecrow".into(),
                experience: selection.final_amount,
                drops: vec![],
                boss_audit: None,
                mentor_bank: None,
                experience_selection: Some(selection),
                source_receipt_key: Some(key.into()),
            },
        ),
    }
}

#[test]
fn economy_zone_fence_fixture_and_wrong_database_binding_reject_before_connect() {
    for value in [
        "",
        "postgres://u@127.0.0.1/mir2_zone_lease_test_a",
        "postgres://u@127.0.0.1:5432/mir2_zone_lease_test_a",
        "postgres://u@localhost:55491/mir2_zone_lease_test_a",
        "postgres://u@192.0.2.1:55491/mir2_zone_lease_test_a",
        "postgres://u@127.0.0.1:55491/shared",
    ] {
        assert!(checked_url(value).is_err());
    }
    let source = "postgres://fixture@127.0.0.1:55491/mir2_zone_lease_test_unreachable";
    let mut config = SimulationConfig::default();
    config.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;
    config.account_store_database_url = Some(source.into());
    let config = config
        .with_zone_save_fence(
            ZoneSaveFence::new("unreachable-zone".into(), "owner-a".into(), 7).unwrap(),
        )
        .unwrap();
    let permit = config.zone_economy_write_permit().unwrap();
    let target = "postgres://private:do-not-emit@127.0.0.1:1/different_database";
    let expected = permit.validate_target_database(target).unwrap_err();
    let envelope = EconomyTransactionEnvelope {
        idempotency_key: "wrong-db".into(),
        transaction_kind: EconomyTransactionKind::Reward,
        zone_id: "unreachable-zone".into(),
        fencing_generation: 7,
        source_sequence: 1,
        created_at_ms: 1,
        legs: vec![EconomyLeg {
            balance: EconomyBalanceKey::gold("a", 0),
            delta: 1,
        }],
        metadata: BTreeMap::new(),
    };
    let error = PostgresEconomyStore::new(target)
        .transact_fenced(&permit, &envelope)
        .unwrap_err();
    assert_eq!(error, expected);
    assert!(!error.contains("do-not-emit"));
}

#[test]
fn economy_zone_fence_postgres_bootstrap_gold_item_skill_and_trade_with_current_owner() {
    let _guard = PG_LOCK.lock().unwrap();
    let Some(db) = Database::from_env() else {
        return;
    };
    let config = db.config();
    let permit = config.zone_economy_write_permit().unwrap();
    let context = db.context();
    let store = db.store();
    let first = memory_runtime("fenced-a");
    let second = memory_runtime("fenced-b");
    let first_id = first.active_identity().unwrap();
    let second_id = second.active_identity().unwrap();
    let first_open = store
        .bootstrap_character_fenced(&permit, &context, &first_id, &first.world_snapshot())
        .unwrap();
    store
        .bootstrap_character_fenced(&permit, &context, &second_id, &second.world_snapshot())
        .unwrap();
    assert!(
        store
            .bootstrap_character_fenced(&permit, &context, &first_id, &first.world_snapshot())
            .unwrap()
            .duplicate
    );
    let mut gold = db.envelope("gold");
    gold.legs[0].balance = EconomyBalanceKey::gold(&first_id.account_id, first_id.character_index);
    store.transact_fenced(&permit, &gold).unwrap();
    let item_key = EconomyBalanceKey::item_quantity(
        &first_id.account_id,
        first_id.character_index,
        "fence-rune",
    );
    let mut item = db.envelope("item");
    item.legs = vec![EconomyLeg {
        balance: item_key.clone(),
        delta: 3,
    }];
    store.transact_fenced(&permit, &item).unwrap();
    let mut unique = db.envelope("unique-item");
    unique.legs = vec![EconomyLeg {
        balance: EconomyBalanceKey::item(
            &first_id.account_id,
            first_id.character_index,
            "owned-fence-unique-item",
        ),
        delta: 1,
    }];
    store.transact_fenced(&permit, &unique).unwrap();
    let command = SharedAccountInventoryCommandEnvelope {
        identity: first_id.clone(),
        command: SharedAccountInventoryCommand::SkillItemConsume {
            spell: mir2_protocol::Spell::FireBall,
            request_id: 3,
            components: vec![mir2_simulation::SharedSkillItemConsumptionComponent {
                item_key: "fence-rune".into(),
                quantity: 1,
            }],
        },
    };
    let skill = economy_transaction_for_command(&context, &command, None).unwrap();
    store.transact_fenced(&permit, &skill).unwrap();
    assert_eq!(db.amount(&item_key), 2);
    let trade = economy_transaction_for_trade(
        &context,
        &offer(&first_id, &second_id, &"1".repeat(32), 2),
        &offer(&second_id, &first_id, &"2".repeat(32), 0),
    )
    .unwrap()
    .unwrap();
    store.transact_fenced(&permit, &trade).unwrap();
    assert_eq!(db.amount(&gold.legs[0].balance), first_open.gold + 17 - 2);
    assert_eq!(db.counts().1, 5);
    assert_eq!(db.counts().2, 5);
    assert_eq!(db.counts().3, 2);
    assert_eq!(
        db.connect()
            .query_one("SELECT count(*) FROM game_economy_trade_projections", &[])
            .unwrap()
            .get::<_, i64>(0),
        2
    );
    let committed = db.image();
    db.lease("owner-b", 8, 120000);
    let replacement = db
        .config()
        .with_zone_save_fence(ZoneSaveFence::new(db.zone.clone(), "owner-b".into(), 8).unwrap())
        .unwrap();
    let replacement_permit = replacement.zone_economy_write_permit().unwrap();
    assert!(
        store
            .bootstrap_character_fenced(
                &replacement_permit,
                &context,
                &first_id,
                &first.world_snapshot()
            )
            .unwrap()
            .duplicate
    );
    let mut missing = first_id.clone();
    missing.account_id = "not-bootstrapped".into();
    assert!(store
        .bootstrap_character_fenced(
            &replacement_permit,
            &context,
            &missing,
            &first.world_snapshot()
        )
        .is_err());
    assert_eq!(db.image(), committed);
}

#[test]
fn economy_zone_fence_postgres_stale_owner_token_expiry_missing_lease_and_schema_write_nothing() {
    let _guard = PG_LOCK.lock().unwrap();
    let Some(db) = Database::from_env() else {
        return;
    };
    let config = db.config();
    let permit = config.zone_economy_write_permit().unwrap();
    let runtime = memory_runtime("fenced-a");
    let identity = runtime.active_identity().unwrap();
    let context = db.context();
    let store = db.store();
    for (owner, token, ttl) in [
        ("owner-b", 7, 120000),
        ("owner-a", 8, 120000),
        ("owner-a", 7, -1),
    ] {
        db.lease(owner, token, ttl);
        assert!(store
            .bootstrap_character_fenced(&permit, &context, &identity, &runtime.world_snapshot())
            .is_err());
        assert!(store
            .transact_fenced(&permit, &db.envelope("stale"))
            .is_err());
        assert_eq!(db.counts(), (0, 0, 0, 0));
    }
    db.lease("owner-a", 7, 120000);
    let mut wrong = db.envelope("wrong-generation");
    wrong.fencing_generation = 8;
    assert!(store.transact_fenced(&permit, &wrong).is_err());
    db.admin()
        .execute(
            "DELETE FROM public.zone_owner_leases WHERE zone_id=$1",
            &[&db.zone],
        )
        .unwrap();
    assert!(store
        .bootstrap_character_fenced(&permit, &context, &identity, &runtime.world_snapshot())
        .is_err());
    assert!(store
        .transact_fenced(&permit, &db.envelope("missing-lease"))
        .is_err());
    assert_eq!(db.counts(), (0, 0, 0, 0));
    db.lease("owner-a", 7, 120000);
    let missing = db
        .scoped
        .replace(&db.schema, &format!("{}_absent", db.schema));
    let missing_permit = db.config_for(&missing).zone_economy_write_permit().unwrap();
    assert!(PostgresEconomyStore::new(missing)
        .transact_fenced(&missing_permit, &db.envelope("missing-schema"))
        .is_err());
    assert_eq!(db.counts(), (0, 0, 0, 0));
}

#[test]
fn economy_zone_fence_postgres_commit_unknown_freezes_no_receipt_retry_but_old_receipt_stays_readable(
) {
    let _guard = PG_LOCK.lock().unwrap();
    let Some(db) = Database::from_env() else {
        return;
    };
    let config = db.config();
    let permit = config.zone_economy_write_permit().unwrap();
    let store = db.store();
    let committed = db.envelope("known");
    let receipt = store.transact_fenced(&permit, &committed).unwrap();
    let before = db.image();
    db.reject_commit();
    let uncertain = db.envelope("rollback-unknown");
    let error = store.transact_fenced(&permit, &uncertain).unwrap_err();
    assert!(error.contains("COMMIT_OUTCOME_UNKNOWN"));
    db.allow_commit();
    assert_eq!(db.image(), before);
    assert!(store.lookup_fenced(&permit, &uncertain).unwrap().is_none());
    assert!(store.transact_fenced(&permit, &uncertain).is_err());
    assert!(config
        .save_account_store()
        .unwrap_err()
        .contains("writes are frozen"));
    db.lease("owner-b", 8, 120000);
    let duplicate = store.lookup_fenced(&permit, &committed).unwrap().unwrap();
    assert!(duplicate.duplicate);
    assert_eq!(duplicate.event_id, receipt.event_id);
    assert_eq!(duplicate.balances_after, receipt.balances_after);
    let replacement = db
        .config()
        .with_zone_save_fence(ZoneSaveFence::new(db.zone.clone(), "owner-b".into(), 8).unwrap())
        .unwrap();
    let replacement_permit = replacement.zone_economy_write_permit().unwrap();
    assert!(
        store
            .lookup_fenced(&replacement_permit, &committed)
            .unwrap()
            .unwrap()
            .duplicate
    );
    assert!(
        store
            .transact_fenced(&replacement_permit, &committed)
            .unwrap()
            .duplicate
    );
    assert!(store
        .transact_fenced(&replacement_permit, &db.envelope("old-context-no-proof"))
        .is_err());
    assert_eq!(db.image(), before);
}

#[test]
fn economy_zone_fence_postgres_lease_lock_survives_until_actual_commit() {
    let _guard = PG_LOCK.lock().unwrap();
    let Some(db) = Database::from_env() else {
        return;
    };
    let permit = db.config().zone_economy_write_permit().unwrap();
    let envelope = db.envelope("held-commit");
    let gate = db.gate_key();
    let mut blocker = db.admin();
    blocker
        .query_one("SELECT pg_advisory_lock($1)", &[&gate])
        .unwrap();
    db.block_commit(gate);
    let store = db.store();
    let worker = thread::spawn(move || store.transact_fenced(&permit, &envelope));
    let _pid = db.blocked_commit_pid();
    let mut takeover = db.admin();
    let takeover_pid: i32 = takeover
        .query_one("SELECT pg_backend_pid()", &[])
        .unwrap()
        .get(0);
    let zone = db.zone.clone();
    let (sender, receiver) = mpsc::sync_channel(1);
    let taker = thread::spawn(move || {
        takeover.batch_execute("SET lock_timeout='3s'").unwrap();
        let result=takeover.execute("UPDATE public.zone_owner_leases SET owner_id='owner-b',fencing_token=8 WHERE zone_id=$1",&[&zone]);
        sender.send(result.is_ok()).unwrap();
    });
    let deadline = Instant::now() + Duration::from_millis(700);
    let mut observer = db.admin();
    let mut waiting = false;
    while Instant::now() < deadline {
        waiting=observer.query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid=$1 AND wait_event_type='Lock')",&[&takeover_pid]).unwrap().get(0);
        if waiting {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    assert!(
        waiting,
        "actual concurrent owner UPDATE must wait on transaction lease row"
    );
    assert!(receiver.try_recv().is_err());
    blocker
        .query_one("SELECT pg_advisory_unlock($1)", &[&gate])
        .unwrap();
    assert!(!worker.join().unwrap().unwrap().duplicate);
    assert!(receiver.recv_timeout(Duration::from_secs(3)).unwrap());
    taker.join().unwrap();
    assert_eq!(db.counts(), (1, 1, 1, 0));
}

#[test]
fn economy_zone_fence_postgres_terminated_commit_backend_rolls_back_every_asset_and_receipt() {
    let _guard = PG_LOCK.lock().unwrap();
    let Some(db) = Database::from_env() else {
        return;
    };
    let config = db.config();
    let permit = config.zone_economy_write_permit().unwrap();
    let envelope = db.envelope("terminate-commit");
    let retry = envelope.clone();
    let retry_permit = permit.clone();
    let gate = db.gate_key();
    let mut blocker = db.admin();
    blocker
        .query_one("SELECT pg_advisory_lock($1)", &[&gate])
        .unwrap();
    db.block_commit(gate);
    let store = db.store();
    let worker = thread::spawn(move || store.transact_fenced(&permit, &envelope));
    let pid = db.blocked_commit_pid();
    assert!(db
        .admin()
        .query_one("SELECT pg_terminate_backend($1)", &[&pid])
        .unwrap()
        .get::<_, bool>(0));
    blocker
        .query_one("SELECT pg_advisory_unlock($1)", &[&gate])
        .unwrap();
    assert!(worker
        .join()
        .unwrap()
        .unwrap_err()
        .contains("COMMIT_OUTCOME_UNKNOWN"));
    assert_eq!(db.counts(), (0, 0, 0, 0));
    assert!(db
        .store()
        .lookup_fenced(&retry_permit, &retry)
        .unwrap()
        .is_none());
    assert!(db.store().transact_fenced(&retry_permit, &retry).is_err());
    assert_eq!(db.counts(), (0, 0, 0, 0));
}

#[test]
fn economy_zone_fence_permissionless_service_rejects_forged_context_without_store_connection() {
    let mut runtime = memory_runtime("no-permit-a");
    let second = memory_runtime("no-permit-b");
    let identity = runtime.active_identity().unwrap();
    let partner = second.active_identity().unwrap();
    let before = serde_json::to_value(runtime.active_character_checkpoint().unwrap()).unwrap();
    // Deliberately not even a PostgreSQL URL. Reaching the store would produce
    // a store error/Unknown; the missing runtime permit must return Deferred.
    let service = PostgresEconomyAccountInventoryService::new("must-not-open-a-connection");
    let context = SharedAccountInventoryExecutionContext {
        zone_id: crate::ZoneId::new("forged-zone"),
        fencing_generation: 7,
        source_sequence: 123,
        created_at_ms: 1,
        external_commit_authorized: true,
    };
    let command = SharedAccountInventoryCommandEnvelope {
        identity: identity.clone(),
        command: SharedAccountInventoryCommand::GoldDrop {
            amount: 1,
            request_id: 12,
        },
    };
    assert!(!service.bootstrap_fenced(&runtime, Some(&context)));
    assert!(matches!(
        service.commit_fenced(&mut runtime, Some(&context), command.clone()),
        SharedAccountInventoryCommitOutcome::Deferred { .. }
    ));
    let key = economy_transaction_for_command(&context, &command, None)
        .unwrap()
        .idempotency_key;
    assert!(matches!(
        service.retry_commit_fenced(&mut runtime, Some(&context), &key, command),
        SharedAccountInventoryCommitOutcome::Deferred { .. }
    ));
    let first_offer = offer(&identity, &partner, &"3".repeat(32), 1);
    let second_offer = offer(&partner, &identity, &"4".repeat(32), 0);
    assert_eq!(
        service.settle_trade_with_runtime(&runtime, Some(&context), &first_offer, &second_offer),
        SharedTradeSettlementOutcome::Deferred
    );
    let key = economy_transaction_for_trade(&context, &first_offer, &second_offer)
        .unwrap()
        .unwrap()
        .idempotency_key;
    assert_eq!(
        service.retry_trade_with_runtime(
            &runtime,
            Some(&context),
            &key,
            &first_offer,
            &second_offer
        ),
        SharedTradeSettlementOutcome::Deferred
    );
    assert_eq!(
        serde_json::to_value(runtime.active_character_checkpoint().unwrap()).unwrap(),
        before
    );
}

#[test]
fn economy_zone_fence_postgres_runtime_trade_override_requires_permit_and_has_no_legacy_fallback() {
    let _guard = PG_LOCK.lock().unwrap();
    let Some(db) = Database::from_env() else {
        return;
    };
    let service = PostgresEconomyAccountInventoryService::new(&db.scoped);
    let context = db.context();
    let mut permissionless = memory_runtime("no-permit-pg");
    let forged = SharedAccountInventoryCommandEnvelope {
        identity: permissionless.active_identity().unwrap(),
        command: SharedAccountInventoryCommand::GoldDrop {
            amount: 1,
            request_id: 1,
        },
    };
    assert!(matches!(
        service.commit_fenced(&mut permissionless, Some(&context), forged),
        SharedAccountInventoryCommitOutcome::Deferred { .. }
    ));
    assert_eq!(db.counts(), (0, 0, 0, 0));
    let config = db.config();
    let mut first =
        super::tests::start_test_runtime_with_config(config.clone(), "runtime-trade-a", "TradeA")
            .unwrap();
    let second =
        super::tests::start_test_runtime_with_config(config, "runtime-trade-b", "TradeB").unwrap();
    seed_trade_opening_gold(&db, &mut first);
    let first_id = first.active_identity().unwrap();
    let second_id = second.active_identity().unwrap();
    assert!(service.bootstrap_fenced(&first, Some(&context)));
    assert!(service.bootstrap_fenced(&second, Some(&context)));
    assert!(
        db.amount(&EconomyBalanceKey::gold(
            &first_id.account_id,
            first_id.character_index
        )) >= 2
    );
    let first_offer = offer(&first_id, &second_id, &"5".repeat(32), 2);
    let second_offer = offer(&second_id, &first_id, &"6".repeat(32), 0);
    let before = db.image();
    assert_eq!(
        service.settle_trade_fenced(Some(&context), &first_offer, &second_offer),
        SharedTradeSettlementOutcome::Deferred
    );
    assert_eq!(
        db.image(),
        before,
        "legacy context-only API cannot publish a PostgreSQL trade"
    );
    let outcome =
        service.settle_trade_with_runtime(&first, Some(&context), &first_offer, &second_offer);
    let SharedTradeSettlementOutcome::DurableCommitted { event_id } = outcome else {
        panic!("runtime-bound PostgreSQL override must commit real durable trade: {outcome:?}");
    };
    let committed = db.image();
    assert_eq!(
        service.settle_trade_with_runtime(&first, Some(&context), &first_offer, &second_offer),
        SharedTradeSettlementOutcome::DurableDuplicate { event_id }
    );
    assert_eq!(db.image(), committed);
    assert_eq!(db.counts().1, 1);
    assert_eq!(db.counts().2, 1);
    assert_eq!(
        db.connect()
            .query_one("SELECT count(*) FROM game_economy_trade_projections", &[])
            .unwrap()
            .get::<_, i64>(0),
        2
    );
}

#[test]
fn economy_zone_fence_postgres_runtime_trade_unknown_none_receipt_never_retries_publication() {
    let _guard = PG_LOCK.lock().unwrap();
    let Some(db) = Database::from_env() else {
        return;
    };
    let config = db.config();
    let mut first =
        super::tests::start_test_runtime_with_config(config.clone(), "unknown-trade-a", "UnknownA")
            .unwrap();
    let second =
        super::tests::start_test_runtime_with_config(config.clone(), "unknown-trade-b", "UnknownB")
            .unwrap();
    seed_trade_opening_gold(&db, &mut first);
    let first_id = first.active_identity().unwrap();
    let second_id = second.active_identity().unwrap();
    let context = db.context();
    let service = PostgresEconomyAccountInventoryService::new(&db.scoped);
    assert!(service.bootstrap_fenced(&first, Some(&context)));
    assert!(service.bootstrap_fenced(&second, Some(&context)));
    let first_offer = offer(&first_id, &second_id, &"7".repeat(32), 2);
    let second_offer = offer(&second_id, &first_id, &"8".repeat(32), 0);
    let transaction = economy_transaction_for_trade(&context, &first_offer, &second_offer)
        .unwrap()
        .unwrap();
    let before = db.image();
    db.reject_commit();
    let initial =
        service.settle_trade_with_runtime(&first, Some(&context), &first_offer, &second_offer);
    assert!(
        matches!(initial,SharedTradeSettlementOutcome::OutcomeUnknown { ref idempotency_key,.. } if idempotency_key==&transaction.idempotency_key)
    );
    db.allow_commit();
    assert_eq!(db.image(), before);
    let permit = first.zone_economy_write_permit().unwrap();
    assert!(db
        .store()
        .lookup_fenced(&permit, &transaction)
        .unwrap()
        .is_none());
    let retry = service.retry_trade_with_runtime(
        &first,
        Some(&context),
        &transaction.idempotency_key,
        &first_offer,
        &second_offer,
    );
    assert!(
        matches!(retry,SharedTradeSettlementOutcome::OutcomeUnknown { ref idempotency_key,.. } if idempotency_key==&transaction.idempotency_key)
    );
    let restarted_service = PostgresEconomyAccountInventoryService::new(&db.scoped);
    assert!(matches!(
        restarted_service.retry_trade_with_runtime(
            &first,
            Some(&context),
            &transaction.idempotency_key,
            &first_offer,
            &second_offer
        ),
        SharedTradeSettlementOutcome::OutcomeUnknown { .. }
    ));
    assert!(config
        .save_account_store()
        .unwrap_err()
        .contains("writes are frozen"));
    assert_eq!(
        db.image(),
        before,
        "a visible None after Unknown does not authorize any new trade/assets/outbox writes"
    );
}

#[test]
fn economy_zone_fence_postgres_actual_service_gold_claim_uses_runtime_permit_and_denies_stale_owner(
) {
    let _guard = PG_LOCK.lock().unwrap();
    let Some(db) = Database::from_env() else {
        return;
    };
    let config = db.config();
    let mut runtime =
        super::tests::start_test_runtime_with_config(config, "service-a", "ServiceA").unwrap();
    let identity = runtime.active_identity().unwrap();
    let before = runtime.world_snapshot().gold;
    let context = db.context();
    let service = PostgresEconomyAccountInventoryService::new(&db.scoped);
    let command = SharedAccountInventoryCommandEnvelope {
        identity,
        command: SharedAccountInventoryCommand::GroundDropClaimPickup {
            drop: mir2_simulation::GroundDropSnapshot {
                object_id: 0x7f00_1122,
                name: "7 Gold".into(),
                name_colour_argb: -1,
                icon: 0,
                x: 10,
                y: 20,
                quantity: 7,
                source_monster: "actual PG fence fixture".into(),
                owner_object_id: None,
                ownership_remaining_ticks: None,
                loot: GroundDropLootSnapshot::Gold { amount: 7 },
            },
            claim_idempotency_key: format!("{}:service-claim", db.schema),
        },
    };
    assert!(
        service
            .commit_fenced(&mut runtime, Some(&context), command.clone())
            .committed
    );
    assert_eq!(runtime.world_snapshot().gold, before + 7);
    let committed = db.image();
    db.lease("owner-b", 7, 120000);
    let mut next = command;
    if let SharedAccountInventoryCommand::GroundDropClaimPickup {
        claim_idempotency_key,
        ..
    } = &mut next.command
    {
        claim_idempotency_key.push_str("-next");
    }
    assert!(
        !service
            .commit_fenced(&mut runtime, Some(&context), next)
            .committed
    );
    assert_eq!(runtime.world_snapshot().gold, before + 7);
    assert_eq!(db.image(), committed);
}

#[test]
fn economy_zone_fence_postgres_prepared_kill_source_ledger_replay_relogin_and_stale_owner() {
    use mir2_protocol::ClientPacket;
    use mir2_simulation::WorldCommand;

    let _guard = PG_LOCK.lock().unwrap();
    let Some(db) = Database::from_env() else {
        return;
    };
    let config = db.config();
    let mut runtime = super::tests::start_test_runtime_with_config(
        config.clone(),
        "strict-kill-a",
        "StrictKillA",
    )
    .unwrap();
    let identity = runtime.active_identity().unwrap();
    let context = db.context();
    let key = format!("{}:prepared-kill", db.schema);
    let command = prepared_kill_award(&db, &runtime, &key);
    let before = runtime.active_character_checkpoint().unwrap();
    let service = PostgresEconomyAccountInventoryService::new(&db.scoped);
    let outcome = service.commit_fenced(&mut runtime, Some(&context), command.clone());
    assert!(
        matches!(outcome, SharedAccountInventoryCommitOutcome::Confirmed(ref receipt) if receipt.committed),
        "{outcome:?}"
    );
    let after = runtime.active_character_checkpoint().unwrap();
    assert_eq!(after.experience, before.experience + 5);
    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(
        db.amount(&EconomyBalanceKey::experience(
            &identity.account_id,
            identity.character_index
        )),
        after.experience
    );
    let saved: Value = db
        .connect()
        .query_one(
            "SELECT snapshot_json FROM character_saves WHERE account_id=$1 AND character_index=$2",
            &[&identity.account_id, &identity.character_index],
        )
        .unwrap()
        .get(0);
    let saved_checkpoint: mir2_simulation::CharacterSaveRecord =
        serde_json::from_value(saved.clone()).unwrap();
    assert_eq!(saved_checkpoint.experience, after.experience);
    assert_eq!(saved_checkpoint.revision, after.revision);
    assert_eq!(
        serde_json::to_value(&saved_checkpoint.guild_experience_journal.kill_outcomes[&key])
            .unwrap(),
        serde_json::json!({"outcome": "noGuild"})
    );
    let row = db.connect().query_one(
        "SELECT tx.receipt,outbox.payload FROM game_economy_transactions tx \
         JOIN game_economy_outbox outbox ON outbox.event_id=tx.event_id WHERE tx.idempotency_key=$1",
        &[&key],
    ).unwrap();
    let receipt: EconomyTransactionReceipt = serde_json::from_value(row.get(0)).unwrap();
    let event: EconomyOutboxEvent = serde_json::from_value(row.get(1)).unwrap();
    validate_stored_economy_transaction(&event, &receipt).unwrap();
    let projection = event.source_checkpoint.as_ref().unwrap();
    assert_eq!(
        receipt.source_checkpoint_digest.as_deref(),
        Some(projection.digest().unwrap().as_str())
    );
    assert_eq!(
        serde_json::to_value(projection).unwrap()["checkpoint"],
        saved
    );
    assert_eq!(
        saved_checkpoint
            .guild_experience_journal
            .applied_kill_receipts
            .get(&key),
        event.envelope.metadata.get("awardHash")
    );
    let committed_source = db.source_image();
    let committed_economy = db.image();
    let replay = service.commit_fenced(&mut runtime, Some(&context), command.clone());
    assert!(
        matches!(replay, SharedAccountInventoryCommitOutcome::Confirmed(ref receipt) if receipt.committed && receipt.packets.is_empty()),
        "{replay:?}"
    );
    assert_eq!(db.source_image(), committed_source);
    assert_eq!(db.image(), committed_economy);

    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
        .unwrap();
    drop(runtime);
    let mut relogin = InProcessWorldRuntime::new(config);
    relogin
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: identity.account_id.clone(),
            password: identity.account_id.clone(),
        }))
        .unwrap();
    relogin
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: identity.character_index,
        }))
        .unwrap();
    assert_eq!(relogin.active_identity().as_ref(), Some(&identity));
    assert_eq!(relogin.world_snapshot().player_experience, after.experience);
    let relogin_source = db.source_image();
    let restarted_service = PostgresEconomyAccountInventoryService::new(&db.scoped);
    let replay = restarted_service.retry_commit_fenced(&mut relogin, Some(&context), &key, command);
    assert!(
        matches!(replay, SharedAccountInventoryCommitOutcome::Confirmed(ref receipt) if receipt.committed && receipt.packets.is_empty()),
        "{replay:?}"
    );
    assert_eq!(db.source_image(), relogin_source);
    assert_eq!(db.image(), committed_economy);

    // An identical token belongs to a different owner now. A new delivery must
    // not publish source, balances or a receipt through the stale runtime.
    db.lease("owner-b", 7, 120000);
    let stale = prepared_kill_award(&db, &relogin, &format!("{key}:stale-owner"));
    let outcome = restarted_service.commit_fenced(&mut relogin, Some(&context), stale);
    assert!(
        matches!(outcome, SharedAccountInventoryCommitOutcome::Deferred { ref receipt } if !receipt.committed && receipt.packets.is_empty()),
        "{outcome:?}"
    );
    assert_eq!(relogin.world_snapshot().player_experience, after.experience);
    assert_eq!(db.source_image(), relogin_source);
    assert_eq!(db.image(), committed_economy);
    assert_eq!(db.counts().1, 1);
    assert_eq!(db.counts().2, 1);
}

#[test]
fn economy_zone_fence_postgres_prepared_kill_unknown_freezes_source_and_absent_receipt_retry() {
    let _guard = PG_LOCK.lock().unwrap();
    let Some(db) = Database::from_env() else {
        return;
    };
    let config = db.config();
    let mut runtime = super::tests::start_test_runtime_with_config(
        config.clone(),
        "unknown-kill-a",
        "UnknownKillA",
    )
    .unwrap();
    let context = db.context();
    let key = format!("{}:prepared-kill-unknown", db.schema);
    let command = prepared_kill_award(&db, &runtime, &key);
    let service = PostgresEconomyAccountInventoryService::new(&db.scoped);
    assert!(service.bootstrap_fenced(&runtime, Some(&context)));
    let source_before = db.source_image();
    let economy_before = db.image();
    db.reject_commit();
    let outcome = service.commit_fenced(&mut runtime, Some(&context), command.clone());
    assert!(
        matches!(outcome, SharedAccountInventoryCommitOutcome::OutcomeUnknown { ref idempotency_key, ref receipt, .. } if idempotency_key == &key && !receipt.committed && receipt.packets.is_empty()),
        "{outcome:?}"
    );
    db.allow_commit();
    assert_eq!(db.source_image(), source_before);
    assert_eq!(db.image(), economy_before);
    assert!(!db
        .connect()
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM game_economy_transactions WHERE idempotency_key=$1)",
            &[&key],
        )
        .unwrap()
        .get::<_, bool>(0));
    assert!(config
        .save_account_store()
        .unwrap_err()
        .contains("writes are frozen"));
    assert!(runtime.save_active_character().is_err());

    let retry = service.retry_commit_fenced(&mut runtime, Some(&context), &key, command.clone());
    assert!(
        matches!(retry, SharedAccountInventoryCommitOutcome::OutcomeUnknown { ref idempotency_key, ref receipt, .. } if idempotency_key == &key && !receipt.committed && receipt.packets.is_empty()),
        "{retry:?}"
    );
    let restarted_service = PostgresEconomyAccountInventoryService::new(&db.scoped);
    let retry = restarted_service.retry_commit_fenced(&mut runtime, Some(&context), &key, command);
    assert!(
        matches!(retry, SharedAccountInventoryCommitOutcome::OutcomeUnknown { ref idempotency_key, ref receipt, .. } if idempotency_key == &key && !receipt.committed && receipt.packets.is_empty()),
        "{retry:?}"
    );
    assert_eq!(db.source_image(), source_before);
    assert_eq!(db.image(), economy_before);
    assert_eq!(db.counts().1, 0);
    assert_eq!(db.counts().2, 0);
}
