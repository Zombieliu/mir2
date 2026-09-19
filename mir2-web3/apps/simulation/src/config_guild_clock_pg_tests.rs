use super::*;
use std::sync::Barrier;

#[path="config_guild_xp_pg_tests.rs"]
mod experience_compensation_tests;

#[path="config_guild_clock_late_pg_tests.rs"]
mod late_owner_tests;

struct Database {
    url: String,
    schema: String,
}
impl Database {
    fn new() -> Self {
        let url = std::env::var("MIR2_GUILD_TEST_DATABASE_URL")
            .expect("dedicated guild test database required");
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let schema = format!("guild_clock_{nonce:x}");
        let db = Self { url, schema };
        let mut client = Client::connect(&db.url, NoTls).unwrap();
        client
            .batch_execute(&format!("CREATE SCHEMA {}", db.schema))
            .unwrap();
        let mut client = db.connect();
        crate::db_projection::apply_migrations(&mut client).unwrap();
        // CHECK must reject NULL-owner/nonzero-expiry, not pass SQL UNKNOWN.
        assert!(client.execute("UPDATE shared_guild_clock SET owner_token=NULL,lease_expires_ms=1 WHERE singleton=TRUE",&[]).is_err());
        db
    }
    fn connect(&self) -> Client {
        let mut client = Client::connect(&self.url, NoTls).unwrap();
        client
            .batch_execute(&format!(
                "SET search_path TO {}; SET lock_timeout='3s'; SET statement_timeout='10s'",
                self.schema
            ))
            .unwrap();
        client
    }
}
impl Drop for Database {
    fn drop(&mut self) {
        if let Ok(mut client) = Client::connect(&self.url, NoTls) {
            let _ = client.batch_execute(&format!("DROP SCHEMA {} CASCADE", self.schema));
        }
    }
}
const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const TTL: u64 = 120000;
fn acquire(client: &mut Client, owner: &str) -> Result<(GuildClockRecord, i64), String> {
    let mut transaction = client.transaction().map_err(|e| e.to_string())?;
    let clock = LockedPostgresGuildClock::lock(&mut transaction)?;
    let result = clock.acquire(&mut transaction, owner, TTL)?;
    transaction.commit().map_err(|e| e.to_string())?;
    Ok(result)
}
#[test]
#[ignore = "requires dedicated MIR2_GUILD_TEST_DATABASE_URL; independent PostgreSQL connections"]
fn guild_clock_postgres_concurrent_owners_and_stale_generation_are_fenced() {
    let db = Database::new();
    let clients = [db.connect(), db.connect()];
    let barrier = Arc::new(Barrier::new(2));
    let handles = clients
        .into_iter()
        .zip([A, B])
        .map(|(mut client, owner)| {
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                acquire(&mut client, owner)
            })
        })
        .collect::<Vec<_>>();
    let outcomes = handles
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        outcomes.iter().filter(|result| result.is_ok()).count(),
        1,
        "{outcomes:?}"
    );
    assert!(outcomes
        .iter()
        .find_map(|result| result.as_ref().err())
        .unwrap()
        .contains("busy"));
    let (before, _) = outcomes
        .iter()
        .find_map(|result| result.as_ref().ok())
        .unwrap();
    let old_lease = before.lease().unwrap();
    let mut client = db.connect();
    // A deterministic repository fixture moves only expiry into the past;
    // all transition decisions still read clock_timestamp() while row-locked.
    client.execute("UPDATE shared_guild_clock SET minute_anchor_ms=1,lease_expires_ms=2 WHERE singleton=TRUE",&[]).unwrap();
    let owner = if old_lease.owner_token == A { B } else { A };
    let (after, _) = acquire(&mut client, owner).unwrap();
    assert!(after.generation > before.generation);
    assert!(after.minute_anchor_ms > before.minute_anchor_ms.saturating_sub(1000));
    let mut transaction = client.transaction().unwrap();
    let clock = LockedPostgresGuildClock::lock(&mut transaction).unwrap();
    assert!(clock
        .advance(&mut transaction, &old_lease, TTL)
        .unwrap_err()
        .to_string()
        .contains("lost"));
    let (advanced, _) = clock
        .advance(&mut transaction, &after.lease().unwrap(), TTL)
        .unwrap();
    assert_eq!(
        advanced.admitted_minutes, 0,
        "takeover must not charge offline duration"
    );
    transaction.commit().unwrap();
}
#[test]
#[ignore = "requires dedicated MIR2_GUILD_TEST_DATABASE_URL; transactional rollback and receipt CAS"]
fn guild_clock_postgres_failed_write_preserves_anchor_and_compensation_cannot_clobber_owner() {
    let db = Database::new();
    let mut client = db.connect();
    let (initial, _) = acquire(&mut client, A).unwrap();
    client.execute("UPDATE shared_guild_clock SET minute_anchor_ms=floor(extract(epoch FROM clock_timestamp())*1000)::bigint-60001 WHERE singleton=TRUE",&[]).unwrap();
    let (before, version) = {
        let mut transaction = client.transaction().unwrap();
        let clock = LockedPostgresGuildClock::lock(&mut transaction).unwrap();
        (clock.record, clock.version)
    };
    {
        let mut transaction = client.transaction().unwrap();
        let clock = LockedPostgresGuildClock::lock(&mut transaction).unwrap();
        let (advance, _) = clock
            .advance(&mut transaction, &initial.lease().unwrap(), TTL)
            .unwrap();
        assert_eq!(advance.admitted_minutes, 1);
        // A database write error aborts the enclosing clock+guild transaction.
        assert!(transaction
            .execute(
                "UPDATE shared_guild_clock SET store_version=0 WHERE singleton=TRUE",
                &[]
            )
            .is_err());
        assert!(transaction.commit().is_ok()); // PostgreSQL COMMIT of an aborted txn rolls back.
    }
    let failed_receipt = {
        let mut transaction = client.transaction().unwrap();
        let clock = LockedPostgresGuildClock::lock(&mut transaction).unwrap();
        assert_eq!(clock.record, before);
        assert_eq!(clock.version, version);
        let (_, receipt) = clock
            .advance(&mut transaction, &initial.lease().unwrap(), TTL)
            .unwrap();
        transaction.commit().unwrap();
        receipt
    };
    let compensated = {
        let mut transaction = client.transaction().unwrap();
        let clock = LockedPostgresGuildClock::lock(&mut transaction).unwrap();
        let (record, _) = clock
            .compensate(&mut transaction, failed_receipt, &before)
            .unwrap();
        assert_eq!(record.minute_anchor_ms, before.minute_anchor_ms);
        assert!(record.generation > before.generation);
        assert!(record.owner_token.is_none());
        transaction.commit().unwrap();
        record
    };
    let (new_owner, new_version) = acquire(&mut client, B).unwrap();
    assert!(new_owner.generation > compensated.generation);
    let mut transaction = client.transaction().unwrap();
    let clock = LockedPostgresGuildClock::lock(&mut transaction).unwrap();
    assert!(clock
        .compensate(&mut transaction, failed_receipt, &before)
        .unwrap_err()
        .to_string()
        .contains("stale"));
    assert_eq!(clock.record, new_owner);
    assert_eq!(clock.version, new_version);
    let (restored, _) = clock
        .invalidate_for_restore(&mut transaction, &before)
        .unwrap();
    assert!(restored.generation > new_owner.generation);
    assert!(restored.owner_token.is_none());
    assert_eq!(restored.minute_anchor_ms, before.minute_anchor_ms);
    transaction.commit().unwrap();
}

#[test]
#[ignore = "requires dedicated MIR2_GUILD_TEST_DATABASE_URL; actual guild buffs and transaction fault"]
fn guild_clock_postgres_guild_expiry_and_receipt_compensation_are_atomic() {
    use super::transactions::{commit_postgres_tick, compensate_postgres_tick};
    let db = Database::new();
    let mut client = db.connect();
    let config = SimulationConfig::default();
    let before = config.account_store.lock().unwrap().clone();
    let mut seeded = before.clone();
    let id = "123456789abcdef0123456789abcdef0".to_string();
    let character = &before.accounts["demo"].characters[0];
    let buff = mir2_game_data::crystal_guild_buff_definitions()
        .iter()
        .find(|definition| definition.time_limit > 0)
        .unwrap();
    seeded.shared_guilds.insert(
        id.clone(),
        SharedGuildRecord {
            id: id.clone(),
            name: "ClockQA".into(),
            revision: 1,
            level: 0,
            experience: 0,
            spare_points: 0,
            gold: 0,
            ranks: vec![SharedGuildRank {
                index: 0,
                name: "Leader".into(),
                options: 255,
            }],
            members: vec![SharedGuildMember { membership_epoch: 0,
                identity: Stage5FriendIdentity {
                    account_id: "demo".into(),
                    character_index: character.index,
                },
                name: character.name.clone(),
                rank_index: 0,
            }],
            notice: vec![],
            storage: BTreeMap::new(),
            buffs: BTreeMap::from([(
                buff.id,
                SharedGuildBuff {
                    id: buff.id,
                    active: true,
                    remaining_minutes: 0,
                },
            )]),
            last_buff_tick_ms: 0,
            experience_receipts: BTreeSet::new(), experience_receipt_payloads: Default::default(),
        },
    );
    let plan = build_account_store_mutation_plan(
        &before,
        &seeded,
        AccountStoreMutationScope::AccountsWithGuilds {
            account_ids: &["demo".into()],
            guild_ids: std::slice::from_ref(&id),
        },
        false,
    );
    write_account_store_mutation_plan_to_postgres(
        &mut client,
        &plan,
        AccountStoreDatabaseMode::SourceOfTruth,
    )
    .unwrap();
    let acquired = commit_postgres_tick(&mut client, A, TTL).unwrap().unwrap();
    assert_eq!(acquired.admitted_minutes, 0);
    client.execute("UPDATE shared_guild_clock SET minute_anchor_ms=floor(extract(epoch FROM clock_timestamp())*1000)::bigint-60001 WHERE singleton=TRUE",&[]).unwrap();
    let baseline = {
        let mut transaction = client.transaction().unwrap();
        let clock = LockedPostgresGuildClock::lock(&mut transaction).unwrap();
        (clock.record, clock.version)
    };
    client.batch_execute("CREATE FUNCTION reject_guild_tick() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected guild row write failure'; END $$; CREATE TRIGGER reject_guild_tick BEFORE UPDATE ON shared_guilds FOR EACH ROW EXECUTE FUNCTION reject_guild_tick()").unwrap();
    assert!(commit_postgres_tick(&mut client, A, TTL)
        .unwrap_err()
        .to_string()
        .contains("guild write"));
    {
        let mut transaction = client.transaction().unwrap();
        let clock = LockedPostgresGuildClock::lock(&mut transaction).unwrap();
        assert_eq!((clock.record, clock.version), baseline);
    }
    client
        .batch_execute(
            "DROP TRIGGER reject_guild_tick ON shared_guilds; DROP FUNCTION reject_guild_tick()",
        )
        .unwrap();
    let receipt = commit_postgres_tick(&mut client, A, TTL).unwrap().unwrap();
    assert_eq!(receipt.admitted_minutes, 1);
    assert_eq!(receipt.after_guilds.len(), 1);
    let guild = &receipt.after_guilds[&id];
    assert!(!guild.buffs[&buff.id].active);
    assert_eq!(guild.buffs[&buff.id].remaining_minutes, -1);
    let restored = compensate_postgres_tick(&mut client, &receipt).unwrap();
    assert_eq!(restored.after_guilds[&id], receipt.before_guilds[&id]);
    assert!(restored.after.owner_token.is_none());
    assert!(restored.after.generation > receipt.after.generation);
    let takeover = commit_postgres_tick(&mut client, B, TTL).unwrap().unwrap();
    assert_eq!(takeover.admitted_minutes, 0);
    assert!(compensate_postgres_tick(&mut client, &receipt)
        .unwrap_err()
        .to_string()
        .contains("stale"));
    // A new successful guild mutation after a minute receipt must also block
    // compensation; the failed compensation must not invalidate its clock.
    client.execute("UPDATE shared_guild_clock SET minute_anchor_ms=floor(extract(epoch FROM clock_timestamp())*1000)::bigint-60001 WHERE singleton=TRUE",&[]).unwrap();
    let next = commit_postgres_tick(&mut client, B, TTL).unwrap().unwrap();
    client.execute("UPDATE shared_guilds SET store_version=store_version+1,raw_json=jsonb_set(raw_json,'{gold}','1'::jsonb) WHERE guild_id=$1",&[&id]).unwrap();
    assert!(compensate_postgres_tick(&mut client, &next)
        .unwrap_err()
        .to_string()
        .contains("stale"));
    {
        let mut transaction = client.transaction().unwrap();
        let clock = LockedPostgresGuildClock::lock(&mut transaction).unwrap();
        assert_eq!(clock.record, next.after);
        assert_eq!(clock.version, next.clock_version);
    }
    let row = client
        .query_one(
            "SELECT raw_json FROM shared_guilds WHERE guild_id=$1",
            &[&id],
        )
        .unwrap();
    let final_guild: SharedGuildRecord = serde_json::from_value(row.get(0)).unwrap();
    assert_eq!(final_guild.gold, 1);
    assert!(!final_guild.buffs[&buff.id].active);
}

#[test]
#[ignore = "requires dedicated MIR2_GUILD_TEST_DATABASE_URL; full File+PostgreSQL restore compensation"]
fn guild_clock_postgres_full_restore_file_failure_persists_fenced_compensation() {
    let db = Database::new();
    let mut client = db.connect();
    let url = format!(
        "{}{}options=-csearch_path%3D{}",
        db.url,
        if db.url.contains('?') { "&" } else { "?" },
        db.schema
    );
    let directory = std::env::temp_dir().join(format!(
        "mir2-clock-full-restore-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path = directory.join("accounts.json");
    let backup_path = directory.join("backup.json");
    let config = SimulationConfig::default()
        .with_account_store_path(&path)
        .with_account_store_database_url(&url);
    let mut backup = config.account_store.lock().unwrap().clone();
    let guild = super::schema_tests::fixture_guild(&backup);
    let id = guild.id.clone();
    backup.shared_guilds.insert(id.clone(), guild.clone());
    backup.guild_clock = Some(GuildClockRecord::default().acquire(A, 1000, TTL).unwrap());
    backup.save_to_path(&backup_path).unwrap();
    config
        .restore_account_store_from_backup(&backup_path)
        .unwrap();
    let original_clock = config
        .account_store
        .lock()
        .unwrap()
        .guild_clock
        .clone()
        .unwrap();
    assert!(original_clock.owner_token.is_none());
    backup.shared_guilds.get_mut(&id).unwrap().gold = 999;
    backup.guild_clock = Some(GuildClockRecord::default().acquire(B, 2000, TTL).unwrap());
    backup.save_to_path(&backup_path).unwrap();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforeFileRename);
    assert!(config
        .restore_account_store_from_backup(&backup_path)
        .is_err());
    config.ensure_account_store_writable().unwrap();
    let file = FileAccountStoreRepository::new(&path)
        .load(config.default_character.clone())
        .unwrap();
    let clock = file.guild_clock.clone().unwrap();
    assert!(clock.owner_token.is_none());
    assert!(clock.generation > original_clock.generation);
    assert_eq!(clock.minute_anchor_ms, original_clock.minute_anchor_ms);
    assert_eq!(file.shared_guilds[&id], guild);
    let durable = load_postgres(&mut client).unwrap();
    assert_eq!(durable.record, clock);
    assert_eq!(
        config.account_store.lock().unwrap().guild_clock,
        Some(clock.clone())
    );
    let row = client
        .query_one(
            "SELECT raw_json FROM shared_guilds WHERE guild_id=$1",
            &[&id],
        )
        .unwrap();
    let persisted: SharedGuildRecord = serde_json::from_value(row.get(0)).unwrap();
    assert_eq!(persisted, guild);
    let source = SimulationConfig::default()
        .with_postgres_account_store(url)
        .unwrap();
    assert_eq!(
        source.account_store.lock().unwrap().guild_clock,
        Some(clock)
    );
    drop(source);
    drop(config);
    fs::remove_dir_all(directory).unwrap();
}
#[test]
#[ignore = "requires dedicated MIR2_GUILD_TEST_DATABASE_URL; real Config source clock pump"]
fn guild_clock_postgres_config_pump_advances_without_sessions_and_updates_authority() {
    let db=Database::new();
    let mut client=db.connect();
    let url=format!("{}{}options=-csearch_path%3D{}",db.url,if db.url.contains('?'){"&"}else{"?"},db.schema);
    let directory=std::env::temp_dir().join(format!("mir2-clock-config-pg-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&directory).unwrap();
    let backup_path=directory.join("backup.json");
    let config=SimulationConfig::default().with_postgres_account_store(url).unwrap();
    let mut backup=AccountStore::new(config.default_character.clone());
    let mut guild=super::schema_tests::fixture_guild(&backup);
    let buff=mir2_game_data::crystal_guild_buff_definitions().iter().find(|buff|buff.time_limit>0).unwrap();
    guild.buffs.insert(buff.id,SharedGuildBuff{id:buff.id,active:true,remaining_minutes:0});
    let id=guild.id.clone();
    backup.shared_guilds.insert(id.clone(),guild);
    backup.save_to_path(&backup_path).unwrap();
    config.restore_account_store_from_backup(&backup_path).unwrap();
    assert!(config.tick_shared_guild_clock().unwrap().is_empty());
    // Advance only the test database clock anchor; no session is ever created.
    client.execute("UPDATE shared_guild_clock SET minute_anchor_ms=minute_anchor_ms-60001 WHERE singleton",&[]).unwrap();
    assert_eq!(config.tick_shared_guild_clock().unwrap(),vec![id.clone()]);
    assert_eq!(config.shared_guild_clock_generation(),1);
    assert!(config.tick_shared_guild_clock().unwrap().is_empty());
    assert_eq!(config.shared_guild_clock_generation(),1);
    let store=config.account_store.lock().unwrap();
    assert!(!store.shared_guilds[&id].buffs[&buff.id].active);
    let saved:SharedGuildRecord=serde_json::from_value(client.query_one("SELECT raw_json FROM shared_guilds WHERE guild_id=$1",&[&id]).unwrap().get(0)).unwrap();
    assert_eq!(store.shared_guilds[&id],saved);
    drop(store);drop(config);fs::remove_dir_all(directory).unwrap();
}
#[test]
#[ignore = "requires dedicated MIR2_GUILD_TEST_DATABASE_URL; live mirror pump compensation"]
fn guild_clock_postgres_mirror_pump_file_failure_reconciles_both_domains() {
    let db=Database::new();let mut client=db.connect();
    let url=format!("{}{}options=-csearch_path%3D{}",db.url,if db.url.contains('?'){"&"}else{"?"},db.schema);
    let directory=std::env::temp_dir().join(format!("mir2-clock-mirror-pump-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
    let path=directory.join("accounts.json");let backup_path=directory.join("backup.json");
    let config=SimulationConfig::default().with_account_store_path(&path).with_account_store_database_url(&url);
    let mut backup=config.account_store.lock().unwrap().clone();
    let mut guild=super::schema_tests::fixture_guild(&backup);
    let buff=mir2_game_data::crystal_guild_buff_definitions().iter().find(|buff|buff.time_limit>0).unwrap();
    guild.buffs.insert(buff.id,SharedGuildBuff{id:buff.id,active:true,remaining_minutes:0});
    let id=guild.id.clone();backup.shared_guilds.insert(id.clone(),guild);
    backup.save_to_path(&backup_path).unwrap();config.restore_account_store_from_backup(&backup_path).unwrap();
    config.tick_shared_guild_clock().unwrap();
    let old_generation=load_postgres(&mut client).unwrap().record.generation;
    // A controlled test clock shift updates both durable mirrors consistently.
    client.execute("UPDATE shared_guild_clock SET minute_anchor_ms=minute_anchor_ms-60001 WHERE singleton",&[]).unwrap();
    {let mut store=config.account_store.lock().unwrap();store.guild_clock=Some(load_postgres(&mut client).unwrap().record);FileAccountStoreRepository::new(&path).save(&store).unwrap();}
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforeFileRename);
    assert!(config.tick_shared_guild_clock().is_err());
    config.ensure_account_store_writable().unwrap();
    let file=FileAccountStoreRepository::new(&path).load(config.default_character.clone()).unwrap();
    let clock=load_postgres(&mut client).unwrap();
    assert_eq!(file.guild_clock,Some(clock.record.clone()));
    assert!(clock.record.generation>old_generation);assert!(clock.record.owner_token.is_none());
    assert!(file.shared_guilds[&id].buffs[&buff.id].active);assert_eq!(file.shared_guilds[&id].buffs[&buff.id].remaining_minutes,0);
    assert_eq!(config.shared_guild_clock_generation(),0);
    assert!(config.tick_shared_guild_clock().unwrap().is_empty());
    // An already divergent mirror must fail before advancing the PG anchor.
    {let mut store=config.account_store.lock().unwrap();store.shared_guilds.get_mut(&id).unwrap().gold+=1;FileAccountStoreRepository::new(&path).save(&store).unwrap();}
    let before=load_postgres(&mut client).unwrap();
    assert!(config.tick_shared_guild_clock().unwrap_err().contains("authority mismatch"));
    let after=load_postgres(&mut client).unwrap();assert_eq!(before.record,after.record);assert_eq!(before.version,after.version);
    drop(config);fs::remove_dir_all(directory).unwrap();
}
