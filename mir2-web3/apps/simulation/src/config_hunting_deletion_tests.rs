use super::super::*;
use super::*;
use crate::{AccountStoreRepository, SimulationSession};
use mir2_protocol::{ClientPacket, ServerPacket};
use postgres::NoTls;
use serde_json::json;
use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn character(index: i32) -> CharacterRecord {
    CharacterRecord {
        index,
        name: format!("DeleteGuard{index}"),
        level: 1,
        class: MirClass::Warrior,
        gender: MirGender::Male,
    }
}

#[test]
fn hunting_deletion_retention_matches_roster_and_saved_character_union() {
    let mut account = AccountRecord::new(character(0));
    account
        .saves
        .insert(2, CharacterSaveRecord::new(character(2)));
    let mutation = AccountStoreAccountMutation {
        expected_version: None,
        desired_account: Some(account),
        saves: BTreeMap::new(),
    };
    assert_eq!(retained_character_indices(&mutation), vec![0, 2]);
    let removal = AccountStoreAccountMutation {
        expected_version: Some(1),
        desired_account: None,
        saves: BTreeMap::new(),
    };
    assert!(retained_character_indices(&removal).is_empty());
}

fn validate_test_database_url(value: &str) -> Result<(), &'static str> {
    if !value.starts_with("postgres://") && !value.starts_with("postgresql://") {
        return Err("fixture requires a PostgreSQL URL");
    }
    let parsed = value
        .parse::<postgres::Config>()
        .map_err(|_| "invalid isolated test database URL")?;
    if !parsed
        .get_dbname()
        .is_some_and(|name| name.starts_with("mir2_hunting_acceptance_"))
    {
        return Err("test database name must start with mir2_hunting_acceptance_");
    }
    if parsed.get_hosts().is_empty()
        || !parsed.get_hosts().iter().all(|host| match host {
            postgres::config::Host::Tcp(host) => {
                host.eq_ignore_ascii_case("localhost")
                    || host
                        .parse::<std::net::IpAddr>()
                        .is_ok_and(|address| address.is_loopback())
            }
            _ => false,
        })
        || !parsed
            .get_hostaddrs()
            .iter()
            .all(std::net::IpAddr::is_loopback)
    {
        return Err("test database must use only explicit loopback TCP hosts");
    }
    Ok(())
}

#[test]
fn hunting_deletion_database_fixture_rejects_nonlocal_or_nonacceptance_targets() {
    assert!(validate_test_database_url(
        "postgres://u@localhost:55439/mir2_hunting_acceptance_fixture"
    )
    .is_ok());
    assert!(validate_test_database_url(
        "postgres://u@127.0.0.1:55439/mir2_hunting_acceptance_fixture"
    )
    .is_ok());
    assert!(validate_test_database_url("postgres://u@127.0.0.1:55439/production").is_err());
    assert!(validate_test_database_url(
        "postgres://u@database.example/mir2_hunting_acceptance_fixture"
    )
    .is_err());
    assert!(validate_test_database_url(
        "host=localhost hostaddr=192.0.2.1 dbname=mir2_hunting_acceptance_fixture"
    )
    .is_err());
    assert!(validate_test_database_url("dbname=mir2_hunting_acceptance_fixture").is_err());
}

struct Fixture {
    url: String,
    client: Client,
    config: SimulationConfig,
    account: String,
    realm: String,
    incarnation: String,
}
impl Fixture {
    fn new(label: &str) -> Self {
        let base = std::env::var("MIR2_ECONOMY_TEST_DATABASE_URL")
            .expect("ignored acceptance requires an explicitly isolated PostgreSQL URL");
        validate_test_database_url(&base)
            .expect("fixture refuses a nonlocal or nonacceptance database");
        let mut bootstrap = Client::connect(&base, NoTls).expect("isolated acceptance database");
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let schema = format!("hunting_delete_{}_{}", std::process::id(), unique);
        assert!(schema
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'));
        bootstrap
            .batch_execute(&format!("CREATE SCHEMA {schema}"))
            .unwrap();
        // Every repository/pool connection receives the same private schema.
        // The URL/password is never printed. Fixtures remain for later review.
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
        let private_views:i64=client.query_one("SELECT count(*) FROM pg_class object JOIN pg_namespace namespace ON namespace.oid=object.relnamespace WHERE namespace.nspname=$1 AND object.relname IN ('hunting_permit_observability_realms','hunting_permit_daily_observations') AND object.relkind='v'",&[&schema]).unwrap().get(0);
        assert_eq!(
            private_views, 2,
            "observation views must stay inside this private fixture schema"
        );
        let account = format!("hunting-delete-{label}-{unique}");
        let mut store = AccountStore::new(character(0));
        let initial = store.accounts.remove("demo").unwrap();
        store.accounts.insert(account.clone(), initial);
        PostgresAccountStoreRepository::new(&url, AccountStoreDatabaseMode::SourceOfTruth)
            .save(&store)
            .unwrap();
        let config = SimulationConfig::default()
            .with_postgres_account_store(&url)
            .unwrap();
        let incarnation=format!("pg-character-v1:{}",client.query_one("SELECT hunting_incarnation::text FROM characters WHERE account_id=$1 AND character_index=0",&[&account]).unwrap().get::<_,String>(0));
        let realm = format!("delete-guard-{label}");
        client.execute("INSERT INTO hunting_economy_policies(realm_id,policy_version,policy_digest,policy_json,created_at_ms) VALUES($1,'fixture','fixture',$2,1)",&[&realm,&json!({"fixture":true})]).unwrap();
        client.execute("INSERT INTO hunting_economy_pools(realm_id,policy_version,genesis_day,settled_through_day,updated_at_ms) VALUES($1,'fixture',0,-1,1)",&[&realm]).unwrap();
        Self {
            url,
            client,
            config,
            account,
            realm,
            incarnation,
        }
    }
    fn wallet(&mut self, bound: i64, unbound: i64) {
        self.client.execute("INSERT INTO hunting_economy_wallets(realm_id,account_id,character_index,character_incarnation,bound_gold,unbound_gold,updated_at_ms) VALUES($1,$2,0,$3,$4,$5,1) ON CONFLICT(realm_id,account_id,character_index,character_incarnation) DO UPDATE SET bound_gold=EXCLUDED.bound_gold,unbound_gold=EXCLUDED.unbound_gold",
            &[&self.realm,&self.account,&self.incarnation,&bound,&unbound]).unwrap();
    }
    fn pending_audit(&mut self) {
        let payload = json!({"operation_key":"retained-audit","owner":{"account_id":self.account,"character_index":0,"incarnation":self.incarnation}});
        self.client.execute("INSERT INTO hunting_economy_operations(realm_id,operation_key,operation_kind,request_digest,receipt_json,receipt_digest,business_day,committed_at_ms) VALUES($1,'retained-audit','conversion','fixture',$2,'fixture',0,1)",&[&self.realm,&payload]).unwrap();
        self.client.execute("INSERT INTO hunting_economy_outbox(realm_id,event_id,event_kind,payload_json,created_at_ms) VALUES($1,'retained-audit','conversion',$2,1)",&[&self.realm,&payload]).unwrap();
    }
    fn login(&self) -> SimulationSession {
        let mut session = SimulationSession::new(self.config.clone());
        let login = session.handle_packet(ClientPacket::Login {
            account_id: self.account.clone(),
            password: "demo".into(),
        });
        assert!(
            login
                .iter()
                .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })),
            "ordinary authenticated character-select login"
        );
        session
    }
    fn has_character(&mut self) -> bool {
        self.client
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM characters WHERE account_id=$1 AND character_index=0)",
                &[&self.account],
            )
            .unwrap()
            .get(0)
    }
    fn memory(&self) -> Value {
        serde_json::to_value(&*self.config.account_store.lock().unwrap()).unwrap()
    }
    fn balance(&mut self) -> (i64, i64) {
        let row=self.client.query_one("SELECT bound_gold,unbound_gold FROM hunting_economy_wallets WHERE realm_id=$1 AND account_id=$2 AND character_index=0 AND character_incarnation=$3",&[&self.realm,&self.account,&self.incarnation]).unwrap();
        (row.get(0), row.get(1))
    }
}

fn remove_character(config: &SimulationConfig, account: &str) -> Result<(), String> {
    config.commit_account_store_transaction(&[account.to_owned()], |store| {
        let record = store.accounts.get_mut(account).unwrap();
        record.characters.retain(|character| character.index != 0);
        record.saves.remove(&0);
        record.character_last_access_binary_datetimes.remove(&0);
        Ok(())
    })
}

#[test]
#[ignore = "requires explicitly isolated PostgreSQL; run with --ignored"]
fn hunting_deletion_postgres_nonzero_wallet_rejects_normal_packet_and_account_tombstone() {
    let mut f = Fixture::new("nonzero");
    let mut session = f.login();
    f.wallet(1, 0);
    let before = f.memory();
    let result = session.handle_packet(ClientPacket::DeleteCharacter { character_index: 0 });
    assert_eq!(result, vec![ServerPacket::DeleteCharacter { result: 1 }]);
    assert_eq!(
        f.memory(),
        before,
        "rejected PG commit cannot publish staged in-memory deletion"
    );
    assert!(f.has_character());
    assert_eq!(f.balance(), (1, 0));
    f.wallet(0, 1);
    let result = session.handle_packet(ClientPacket::DeleteCharacter { character_index: 0 });
    assert_eq!(result, vec![ServerPacket::DeleteCharacter { result: 1 }]);
    assert!(f.has_character());
    assert_eq!(f.balance(), (0, 1));
    let result = f
        .config
        .commit_account_store_transaction(&[f.account.clone()], |store| {
            store.accounts.remove(&f.account);
            Ok(())
        });
    assert!(result.unwrap_err().contains(NONZERO_WALLET));
    assert!(f.has_character());
    assert_eq!(f.balance(), (0, 1));
    assert_eq!(f.memory(), before);
}

#[test]
#[ignore = "requires explicitly isolated PostgreSQL; run with --ignored"]
fn hunting_deletion_postgres_zero_wallet_allows_pending_audit_without_erasing_history() {
    let mut f = Fixture::new("zero");
    let mut session = f.login();
    f.wallet(0, 0);
    f.pending_audit();
    let result = session.handle_packet(ClientPacket::DeleteCharacter { character_index: 0 });
    assert_eq!(
        result,
        vec![ServerPacket::DeleteCharacterSuccess { character_index: 0 }]
    );
    assert!(!f.has_character());
    assert_eq!(f.balance(), (0, 0));
    let pending:i64=f.client.query_one("SELECT count(*) FROM hunting_economy_outbox WHERE realm_id=$1 AND delivered_at_ms IS NULL",&[&f.realm]).unwrap().get(0);
    assert_eq!(
        pending, 1,
        "committed audit work is not an unfinished asset operation"
    );
    let history: i64 = f
        .client
        .query_one(
            "SELECT count(*) FROM hunting_economy_operations WHERE realm_id=$1",
            &[&f.realm],
        )
        .unwrap()
        .get(0);
    assert_eq!(
        history, 1,
        "ordinary deletion never removes immutable audit history"
    );
    f.client.execute("INSERT INTO characters(account_id,character_index,character_name,class,gender,level,raw_json) VALUES($1,0,'Recreated','Warrior','Male',1,$2)",&[&f.account,&json!({})]).unwrap();
    let recreated:String=f.client.query_one("SELECT hunting_incarnation::text FROM characters WHERE account_id=$1 AND character_index=0",&[&f.account]).unwrap().get(0);
    assert_ne!(format!("pg-character-v1:{recreated}"), f.incarnation);
}

#[test]
#[ignore = "requires explicitly isolated PostgreSQL; run with --ignored"]
fn hunting_deletion_postgres_waits_for_account_qualified_wallet_publication() {
    let mut f = Fixture::new("publication-race");
    f.wallet(0, 0);
    let mut source = Client::connect(&f.url, NoTls).unwrap();
    let mut tx = source.transaction().unwrap();
    // This is the actual HTTP authorization's account SHARE lock, held until its
    // wallet transaction commits; use SQL directly to avoid a crate dependency.
    tx.query_one(
        "SELECT account_id FROM accounts WHERE account_id=$1 FOR SHARE",
        &[&f.account],
    )
    .unwrap();
    let holder: i32 = tx.query_one("SELECT pg_backend_pid()", &[]).unwrap().get(0);
    tx.execute("UPDATE hunting_economy_wallets SET unbound_gold=7 WHERE realm_id=$1 AND account_id=$2 AND character_index=0 AND character_incarnation=$3",&[&f.realm,&f.account,&f.incarnation]).unwrap();
    let config = f.config.clone();
    let account = f.account.clone();
    let (started_tx, started_rx) = mpsc::channel();
    let deletion = thread::spawn(move || {
        started_tx.send(()).unwrap();
        remove_character(&config, &account)
    });
    started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut blocked = false;
    while Instant::now() < deadline {
        blocked=f.client.query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1::int=ANY(pg_blocking_pids(pid)))",&[&holder]).unwrap().get(0);
        if blocked {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    tx.commit().unwrap();
    let result = deletion.join().unwrap();
    assert!(
        blocked,
        "delete must wait for the account-qualified wallet transaction"
    );
    assert!(result.unwrap_err().contains(NONZERO_WALLET));
    assert!(f.has_character());
    assert_eq!(f.balance(), (0, 7));
    assert!(f.config.account_store.lock().unwrap().accounts[&f.account]
        .characters
        .iter()
        .any(|character| character.index == 0));
}

#[test]
#[ignore = "requires explicitly isolated PostgreSQL; run with --ignored"]
fn hunting_deletion_postgres_committed_delete_rejects_late_authorized_publication() {
    let mut f = Fixture::new("deletion-race");
    f.wallet(0, 0);
    let mut source = Client::connect(&f.url, NoTls).unwrap();
    let mut tx = source.transaction().unwrap();
    tx.query_one(
        "SELECT account_id FROM accounts WHERE account_id=$1 FOR UPDATE",
        &[&f.account],
    )
    .unwrap();
    let desired = AccountStoreAccountMutation {
        expected_version: None,
        desired_account: Some(AccountRecord::empty()),
        saves: BTreeMap::new(),
    };
    guard_character_tombstones(&mut tx, &f.account, &desired).unwrap();
    tx.execute(
        "DELETE FROM characters WHERE account_id=$1 AND character_index=0",
        &[&f.account],
    )
    .unwrap();
    let holder: i32 = tx.query_one("SELECT pg_backend_pid()", &[]).unwrap().get(0);
    let url = f.url.clone();
    let account = f.account.clone();
    let (pid_tx, pid_rx) = mpsc::channel();
    let publisher = thread::spawn(move || {
        let mut client = Client::connect(&url, NoTls).unwrap();
        let mut publication = client.transaction().unwrap();
        let pid: i32 = publication
            .query_one("SELECT pg_backend_pid()", &[])
            .unwrap()
            .get(0);
        pid_tx.send(pid).unwrap();
        publication
            .query_one(
                "SELECT account_id FROM accounts WHERE account_id=$1 FOR SHARE",
                &[&account],
            )
            .unwrap();
        let owner=publication.query_opt("SELECT hunting_incarnation::text FROM characters WHERE account_id=$1 AND character_index=0 FOR SHARE",&[&account]).unwrap();
        // The real Gateway authorize_owner likewise rejects before convert_locked.
        assert!(owner.is_none());
        publication.rollback().unwrap();
    });
    let publisher_pid = pid_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut blocked = false;
    while Instant::now() < deadline {
        blocked = f
            .client
            .query_one(
                "SELECT $1::int=ANY(pg_blocking_pids($2::int))",
                &[&holder, &publisher_pid],
            )
            .unwrap()
            .get(0);
        if blocked {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    tx.commit().unwrap();
    publisher.join().unwrap();
    assert!(
        blocked,
        "late publication must wait until deletion is authoritative"
    );
    assert!(!f.has_character());
    assert_eq!(f.balance(), (0, 0));
}
