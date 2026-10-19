//! Real PostgreSQL carrier acceptance. Explicit opt-in only; never starts a
//! database, external server, Docker or virtualization. All state is appended
//! in this test's unique schema in a loopback acceptance database.
use super::*;
use crate::routing::{
    SharedInProcessZoneRuntimeFactory, SharedZoneRuntimeFactory, ZoneId, ZoneRegistry,
    ZoneRuntimeFactory,
};
use mir2_client_wire::{hunting_repair as wire, U64};
use mir2_economy_service as economy;
use mir2_protocol::{ClientPacket, MirDirection, Point};
use mir2_simulation::{
    AccountRecord, AccountStore, AccountStoreDatabaseMode, AccountStoreRepository,
    CharacterSaveRecord, EquipmentSlot, InProcessWorldRuntime, PostgresAccountStoreRepository,
    SimulationConfig, SimulationSession, VisibleNpcRecord,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    net::IpAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const FUNDING: u64 = 1_000_000;
// This is the real legacy equipment-slot protocol UID, not a fake nonce.
const WEAPON_UID: u64 = 0;
const NPC_ID: u32 = 4990;
// Test-only key for a separate, unique synthetic recovery directory. Never
// loaded from or shared with deployment configuration or real player state.
const RECOVERY_TEST_KEY: [u8; 32] = [
    0x10, 0x21, 0x32, 0x43, 0x54, 0x65, 0x76, 0x87, 0x98, 0xa9, 0xba, 0xcb, 0xdc, 0xed, 0xfe, 0x0f,
    0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xf1, 0x02,
];

/// The real standalone runtime, with the same explicit shared-guild authority
/// initialization used by simulation's real repair PG fixture. This factory
/// never handles commands or supplies quotes, repair plans or mock receipts.
struct RepairInProcessFactory;
impl ZoneRuntimeFactory for RepairInProcessFactory {
    fn supports_local_hunting_source_authority(&self) -> bool {
        true
    }
    fn create_runtime(
        &self,
        config: SimulationConfig,
        _zone: &ZoneId,
    ) -> mir2_simulation::ZoneRuntimeHandle {
        let mut runtime = InProcessWorldRuntime::new(config);
        runtime.enable_shared_guild_authority();
        Box::new(runtime)
    }
}

fn isolated_url() -> String {
    let url = std::env::var("MIR2_ECONOMY_TEST_DATABASE_URL")
        .expect("ignored carrier acceptance requires explicit isolated PostgreSQL URL");
    assert!(
        url.starts_with("postgres://") || url.starts_with("postgresql://"),
        "isolated PostgreSQL URL required"
    );
    let parsed: postgres::Config = url
        .parse()
        .unwrap_or_else(|_| panic!("invalid isolated PostgreSQL configuration"));
    assert!(
        parsed
            .get_dbname()
            .is_some_and(|name| name.starts_with("mir2_hunting_acceptance_")),
        "acceptance database prefix required"
    );
    assert!(!parsed.get_hosts().is_empty() && parsed.get_hosts().iter().all(|host|
        matches!(host,postgres::config::Host::Tcp(name) if name.eq_ignore_ascii_case("localhost")
            || name.parse::<IpAddr>().is_ok_and(|ip|ip.is_loopback()))), "only loopback PostgreSQL hosts permitted");
    assert!(
        parsed.get_hostaddrs().iter().all(IpAddr::is_loopback),
        "only loopback PostgreSQL hostaddrs permitted"
    );
    url
}

fn repair_config() -> SimulationConfig {
    let mut config = SimulationConfig::default();
    config.visible_npcs.push(VisibleNpcRecord {
        object_id: NPC_ID,
        name: "Synthetic carrier repair NPC".into(),
        image: 5,
        colour_argb: -1,
        position: Point { x: 331, y: 271 },
        direction: MirDirection::Left,
        quest_ids: Vec::new(),
        script_key: Some("BichonProvince/NaturalCave/WickedTrader".into()),
    });
    config
}

fn seeded_checkpoint(free: bool) -> CharacterSaveRecord {
    let mut session = SimulationSession::new(repair_config());
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into()
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    assert!(session
        .handle_packet(ClientPacket::StartGame { character_index: 0 })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    let mut save = session
        .active_character_checkpoint()
        .expect("real seed StartGame checkpoint");
    // Seed an actual catalogue WoodenSword using EquipmentState's stored
    // representation. No runtime/private-resource mutation or mock repair API.
    let weapon = json!({"key":"crystal-item-221","slot":"weapon","quantity":1,
        "name":"WoodenSword","icon":0,"shape":null,"description":"synthetic carrier target",
        "durability_current":if free {1000}else{100},"durability_max":1000,
        "attack":1,"defence":0,"added_attack":0,"added_defence":0,"added_luck":0,
        "user_item_unique_id":null});
    save.equipment_items_json
        .retain(|item| serde_json::from_str::<Value>(item).unwrap()["slot"] != "weapon");
    save.equipment_items_json.push(weapon.to_string());
    save.gold = 1234;
    save
}

struct Fixture {
    client: postgres::Client,
    session: GatewaySession,
    account: String,
    realm: String,
    url: String,
    config: SimulationConfig,
    registry: ZoneRegistry,
    recovery_dir: std::path::PathBuf,
}
impl Fixture {
    fn new(shared: bool, free: bool) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nonce = format!(
            "{}_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let base = isolated_url();
        let schema = format!("repair_carrier_{nonce}");
        let mut bootstrap = postgres::Client::connect(&base, postgres::NoTls)
            .unwrap_or_else(|_| panic!("isolated PostgreSQL connection failed"));
        bootstrap
            .batch_execute(&format!("CREATE SCHEMA {schema}"))
            .unwrap();
        let separator = if base.contains('?') { '&' } else { '?' };
        let url = format!("{base}{separator}options=-csearch_path%3D{schema}");
        let mut client = postgres::Client::connect(&url, postgres::NoTls)
            .unwrap_or_else(|_| panic!("isolated schema connection failed"));
        assert_eq!(
            client
                .query_one("SELECT current_schema()", &[])
                .unwrap()
                .get::<_, String>(0),
            schema
        );
        mir2_simulation::apply_migrations(&mut client).unwrap();
        let save = seeded_checkpoint(free);
        let account = format!("carrier-account-{nonce}");
        let mut record = AccountRecord::new(save.character.clone());
        record.saves.insert(0, save.clone());
        let mut store = AccountStore::new(save.character.clone());
        store.accounts.clear();
        store.accounts.insert(account.clone(), record);
        PostgresAccountStoreRepository::new(&url, AccountStoreDatabaseMode::SourceOfTruth)
            .save(&store)
            .unwrap();
        let realm = format!("carrier-realm-{nonce}");
        let policy = serde_json::from_value(json!({"schema_version":1,"policy_version":"synthetic-carrier-v1","enabled":true,
            "k":{"numerator":1,"denominator":10},"conversion_fee":{"numerator":2,"denominator":100},
            "daily_decay":{"numerator":1,"denominator":150},"baseline_daily_mint_atoms":720000,
            "daily_mint_cap_multiplier":{"numerator":3,"denominator":2},"business_timezone":"Asia/Shanghai","settlement_hour":4})).unwrap();
        economy::initialize_realm(&mut client, &realm, &policy).unwrap();
        let recovery_dir =
            std::env::temp_dir().join(format!("mir2-repair-carrier-recovery-{nonce}"));
        let config = repair_config()
            .with_postgres_account_store(&url)
            .unwrap()
            .with_hunting_economy_realm(&realm)
            .unwrap()
            .with_save_recovery_dir(&recovery_dir)
            .with_save_recovery_mac_key(RECOVERY_TEST_KEY)
            .unwrap();
        let factory = if shared {
            // The real SharedInProcessZoneSessionRuntime factory, with a long
            // deterministic cadence so unrelated autonomous ticks cannot race
            // the acceptance turn. No custom/mock WorldRuntime is installed.
            Arc::new(SharedInProcessZoneRuntimeFactory::with_tick_cadences(
                Duration::from_secs(3600),
                BTreeMap::new(),
            )) as SharedZoneRuntimeFactory
        } else {
            Arc::new(RepairInProcessFactory) as SharedZoneRuntimeFactory
        };
        // Both paths use a real routing context. A manually attached runtime
        // cannot perform Gateway's required standard-login recovery preflight.
        let registry = ZoneRegistry::new(ZoneId::primary(), factory);
        let mut session = GatewaySession::new_with_zone_registry(config.clone(), &registry);
        let logged_in = session
            .try_handle_packet(ClientPacket::Login {
                account_id: account.clone(),
                password: "demo".into(),
            })
            .unwrap_or_else(|error| {
                panic!(
                    "real Gateway Login failed: {}",
                    error
                        .replace(&url, "[redacted isolated URL]")
                        .replace(&base, "[redacted isolated URL]")
                )
            });
        assert!(
            logged_in
                .iter()
                .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })),
            "real Gateway Login packets: {logged_in:?}"
        );
        let entered = session
            .try_handle_packet(ClientPacket::StartGame { character_index: 0 })
            .unwrap_or_else(|error| {
                panic!(
                    "real Gateway StartGame failed: {}",
                    error
                        .replace(&url, "[redacted isolated URL]")
                        .replace(&base, "[redacted isolated URL]")
                )
            });
        assert!(
            entered
                .iter()
                .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })),
            "{entered:?}"
        );
        assert_eq!(session.active_identity().unwrap().account_id, account);
        let incarnation:String=client.query_one("SELECT hunting_incarnation::text FROM characters WHERE account_id=$1 AND character_index=0",&[&account]).unwrap().get(0);
        if !free {
            let owner = economy::WalletOwner {
                account_id: account.clone(),
                character_index: 0,
                incarnation: format!("pg-character-v1:{incarnation}"),
            };
            economy::credit_bound_authorized_with_source(
                &mut client,
                &realm,
                "synthetic-carrier-funding",
                "synthetic-carrier-funding-v1",
                FUNDING,
                |_, _| Ok(owner),
                |_, _| Ok(()),
            )
            .unwrap();
        }
        session.interact(NPC_ID);
        session.select_npc_dialog_target("@Repair");
        Self {
            client,
            session,
            account,
            realm,
            url,
            config,
            registry,
            recovery_dir,
        }
    }
    fn durable(&mut self) -> CharacterSaveRecord {
        serde_json::from_value(self.client.query_one("SELECT snapshot_json FROM character_saves WHERE account_id=$1 AND character_index=0",&[&self.account]).unwrap().get(0)).unwrap()
    }
    fn balance(&mut self) -> Option<i64> {
        self.client.query_opt("SELECT bound_gold FROM hunting_economy_wallets WHERE realm_id=$1 AND account_id=$2",&[&self.realm,&self.account]).unwrap().map(|row|row.get(0))
    }
    fn count(&mut self, table: &str) -> i64 {
        assert!([
            "hunting_repair_quotes",
            "hunting_repair_results",
            "hunting_economy_burn_events"
        ]
        .contains(&table));
        self.client
            .query_one(
                &format!("SELECT count(*) FROM {table} WHERE realm_id=$1"),
                &[&self.realm],
            )
            .unwrap()
            .get(0)
    }
    fn dispatch(
        &mut self,
        account: Option<&str>,
        request_id: u64,
        action: wire::Action,
    ) -> BrowserHuntingRepairDispatch {
        // Same strict byte parser used by the actual WebSocket reader; every
        // external u64 is decimal text, including legacy equipment slot UID 0.
        let outgoing = wire::ClientRequest::new(U64::new(request_id), action).unwrap();
        let encoded = wire::encode_client_request(&outgoing).unwrap();
        let request = wire::parse_client_request(&encoded).unwrap();
        let dispatch =
            execute_browser_hunting_repair(&mut self.session, account, true, &request).unwrap();
        let response =
            wire::parse_server_frame(&wire::encode_server_frame(&dispatch.frame).unwrap()).unwrap();
        response.validate_for_request(&request).unwrap();
        dispatch
    }
    fn quote(&mut self, request_id: u64) -> wire::Quote {
        let account = self.account.clone();
        match self
            .dispatch(
                Some(&account),
                request_id,
                wire::Action::Quote {
                    unique_id: U64::new(WEAPON_UID),
                    special: false,
                },
            )
            .frame
            .reply
        {
            wire::Reply::Quoted { quote } => quote,
            reply => panic!("real NPC carrier quote failed: {reply:?}"),
        }
    }

    fn assert_no_recovery_checkpoint(&self) {
        let files: Vec<_> = std::fs::read_dir(&self.recovery_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            !files
                .iter()
                .any(|name| name.contains(".journal.json") || name.contains(".committed.json")),
            "old World checkpoint must never enter recovery journal"
        );
        for entry in std::fs::read_dir(self.recovery_dir.join("quarantine")).unwrap() {
            let entry = entry.unwrap();
            // Native Windows sync_directory publishes this control marker in
            // every durable directory, including an empty quarantine. It is
            // not a recovery payload. Permit only its exact name/type/bytes;
            // every .q, unexpected file, directory or symlink still fails.
            assert_eq!(entry.file_name().as_os_str(),std::ffi::OsStr::new(".mir2-directory-durability"),
                "repair recovery must not produce a stale-checkpoint quarantine or untrusted content");
            let metadata = std::fs::symlink_metadata(entry.path()).unwrap();
            assert!(
                metadata.is_file() && !metadata.file_type().is_symlink(),
                "quarantine control marker must be a plain file"
            );
            assert_eq!(
                std::fs::read(entry.path()).unwrap().as_slice(),
                b"mir2-directory-durability-v1\n",
                "quarantine control marker must not contain a recovery payload"
            );
        }
    }

    fn confirm_unknown(&mut self) -> wire::Quote {
        let quote = self.quote(80);
        self.session.set_hunting_repair_test_fault(2).unwrap();
        let account = self.account.clone();
        let pending = self.dispatch(
            Some(&account),
            81,
            wire::Action::Confirm {
                quote_id: quote.quote_id.clone(),
            },
        );
        assert!(matches!(pending.frame.reply, wire::Reply::Pending { .. }));
        assert!(!pending.close_session);
        let live = self.session.world_snapshot();
        let weapon = live
            .equipment_items
            .iter()
            .find(|item| item.slot == EquipmentSlot::Weapon)
            .unwrap();
        assert_eq!(
            weapon.durability_current, 100,
            "unknown keeps original World until Query decides projection"
        );
        assert_eq!(
            self.count("hunting_repair_results"),
            1,
            "fault models a real committed source, not a fake receipt"
        );
        assert_eq!(self.count("hunting_economy_burn_events"), 1);
        quote
    }

    fn restart_with_real_login(&mut self) {
        let replacement =
            GatewaySession::new_with_zone_registry(self.config.clone(), &self.registry);
        let old = std::mem::replace(&mut self.session, replacement);
        drop(old);
        let login = self
            .session
            .try_handle_packet(ClientPacket::Login {
                account_id: self.account.clone(),
                password: "demo".into(),
            })
            .unwrap_or_else(|error| {
                panic!(
                    "real post-teardown Login failed: {}",
                    error.replace(&self.url, "[redacted isolated URL]")
                )
            });
        assert!(login
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
        let entered = self
            .session
            .try_handle_packet(ClientPacket::StartGame { character_index: 0 })
            .unwrap_or_else(|error| {
                panic!(
                    "real post-teardown StartGame failed: {}",
                    error.replace(&self.url, "[redacted isolated URL]")
                )
            });
        assert!(entered
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
        assert_eq!(
            self.session.active_identity().unwrap().account_id,
            self.account
        );
    }

    fn set_moderation(&mut self, banned: bool) {
        // A separate trusted moderation writer changes this synthetic account
        // while the original repair's local source mirror is still frozen.
        let mut account: AccountRecord = serde_json::from_value(
            self.client
                .query_one(
                    "SELECT raw_json FROM accounts WHERE account_id=$1",
                    &[&self.account],
                )
                .unwrap()
                .get(0),
        )
        .unwrap();
        account.is_banned = banned;
        account.ban_until_ms = None;
        account.ban_reason = if banned {
            "synthetic moderation after unknown repair".into()
        } else {
            String::new()
        };
        self.client.execute("UPDATE accounts SET is_banned=$2,ban_until_ms=NULL,raw_json=$3,store_version=store_version+1 WHERE account_id=$1",
            &[&self.account,&banned,&serde_json::to_value(account).unwrap()]).unwrap();
    }
}

fn charged_roundtrip(shared: bool) {
    let mut db = Fixture::new(shared, false);
    let before = db.durable();
    assert_eq!(db.balance(), Some(FUNDING as i64));
    let wrong = db.dispatch(
        Some("different-authenticated-account"),
        1,
        wire::Action::Quote {
            unique_id: U64::new(0),
            special: false,
        },
    );
    assert!(matches!(
        wrong.frame.reply,
        wire::Reply::Rejected {
            code: wire::Rejection::Unauthenticated,
            ..
        }
    ));
    assert_eq!(db.count("hunting_repair_quotes"), 0);
    assert_eq!(db.count("hunting_repair_results"), 0);
    assert_eq!(db.count("hunting_economy_burn_events"), 0);
    let quote = db.quote(2);
    assert_eq!(quote.unique_id.get(), 0);
    assert!(quote.cost_bound_gold.get() > 0);
    assert_eq!(db.balance(), Some(FUNDING as i64));
    assert_eq!(db.count("hunting_repair_quotes"), 1);
    let account = db.account.clone();
    let refused = db.dispatch(
        Some("different-authenticated-account"),
        3,
        wire::Action::Confirm {
            quote_id: quote.quote_id.clone(),
        },
    );
    assert!(matches!(
        refused.frame.reply,
        wire::Reply::Rejected {
            code: wire::Rejection::Unauthenticated,
            ..
        }
    ));
    assert_eq!(db.count("hunting_repair_results"), 0);
    assert_eq!(db.balance(), Some(FUNDING as i64));
    let confirmed = db.dispatch(
        Some(&account),
        4,
        wire::Action::Confirm {
            quote_id: quote.quote_id.clone(),
        },
    );
    let receipt = match &confirmed.frame.reply {
        wire::Reply::Committed {
            receipt,
            replayed: false,
            reload_required: false,
        } => receipt.clone(),
        reply => panic!("real carrier confirmation failed: {reply:?}"),
    };
    assert!(!confirmed.close_session);
    assert!(confirmed
        .packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::ItemRepaired { .. })));
    assert!(!confirmed
        .packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoseGold { .. })));
    let repaired = db.durable();
    assert_eq!(repaired.revision, before.revision + 1);
    assert_eq!(repaired.gold, before.gold);
    let weapon: Value = repaired
        .equipment_items_json
        .iter()
        .map(|item| serde_json::from_str::<Value>(item).unwrap())
        .find(|item| item["slot"] == "weapon")
        .unwrap();
    assert_eq!(weapon["durability_current"], json!(970));
    assert_eq!(weapon["durability_max"], json!(970));
    let live = db.session.world_snapshot();
    let live_weapon = live
        .equipment_items
        .iter()
        .find(|item| item.slot == EquipmentSlot::Weapon)
        .unwrap();
    assert_eq!(
        (live_weapon.durability_current, live_weapon.durability_max),
        (970, 970)
    );
    assert_eq!(live.gold, before.gold);
    for (id, action) in [
        (
            5,
            wire::Action::Query {
                quote_id: quote.quote_id.clone(),
            },
        ),
        (
            6,
            wire::Action::Confirm {
                quote_id: quote.quote_id.clone(),
            },
        ),
    ] {
        let replay = db.dispatch(Some(&account), id, action);
        assert!(
            matches!(&replay.frame.reply,wire::Reply::Committed {receipt:actual,replayed:true,reload_required:false} if actual==&receipt)
        );
        assert!(replay.packets.is_empty());
        assert!(!replay.close_session);
    }
    assert_eq!(
        db.balance(),
        Some((FUNDING - quote.cost_bound_gold.get()) as i64)
    );
    assert_eq!(db.count("hunting_repair_results"), 1);
    assert_eq!(db.count("hunting_economy_burn_events"), 1);
    assert_eq!(db.durable().revision, repaired.revision);
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_carrier_postgres_inprocess_quote_confirm_query_once() {
    charged_roundtrip(false);
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_carrier_postgres_real_shared_quote_confirm_query_once() {
    charged_roundtrip(true);
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_carrier_postgres_free_source_without_wallet_or_burn() {
    let mut db = Fixture::new(false, true);
    let before = db.durable();
    let quote = db.quote(1);
    assert_eq!(quote.cost_bound_gold.get(), 0);
    assert_eq!(db.balance(), None);
    let account = db.account.clone();
    let confirmed = db.dispatch(
        Some(&account),
        2,
        wire::Action::Confirm {
            quote_id: quote.quote_id.clone(),
        },
    );
    let receipt = match confirmed.frame.reply {
        wire::Reply::Committed {
            receipt,
            replayed: false,
            reload_required: false,
        } => receipt,
        reply => panic!("real free carrier confirmation failed: {reply:?}"),
    };
    assert!(
        receipt.free && !receipt.operation_key.is_empty() && !receipt.request_digest.is_empty()
    );
    let query = db.dispatch(
        Some(&account),
        3,
        wire::Action::Query {
            quote_id: quote.quote_id,
        },
    );
    assert!(
        matches!(query.frame.reply,wire::Reply::Committed {receipt:actual,replayed:true,reload_required:false} if actual==receipt)
    );
    assert!(query.packets.is_empty());
    assert_eq!(db.balance(), None);
    assert_eq!(db.count("hunting_economy_burn_events"), 0);
    assert_eq!(db.count("hunting_repair_results"), 1);
    let after = db.durable();
    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(after.gold, before.gold);
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_carrier_postgres_shared_unknown_teardown_retries_before_capture_then_real_login()
{
    use crate::session::GatewayTeardownPersistenceOutcome;
    let mut db = Fixture::new(true, false);
    let quote = db.confirm_unknown();
    let committed = db.durable();
    let mut blocker = postgres::Client::connect(&db.url, postgres::NoTls)
        .unwrap_or_else(|_| panic!("isolated blocker connection failed"));
    let mut held = blocker.transaction().unwrap();
    economy::lock_realm_and_read_database_clock(&mut held, &db.realm).unwrap();
    let unresolved = db.session.try_persist_teardown_once();
    assert!(
        matches!(
            unresolved,
            GatewayTeardownPersistenceOutcome::Retry {
                prepare_error: Some(_),
                save_error: None,
                journal_error: None
            }
        ),
        "{unresolved:?}"
    );
    assert!(
        db.session.prepared_teardown_checkpoint().is_none(),
        "an unresolved source must not cache old revision"
    );
    db.assert_no_recovery_checkpoint();
    held.rollback().unwrap();
    let recovered = db.session.try_persist_teardown_once();
    assert!(
        matches!(recovered, GatewayTeardownPersistenceOutcome::Saved),
        "{recovered:?}"
    );
    let captured = db.session.prepared_teardown_checkpoint().unwrap();
    assert_eq!(captured.revision, committed.revision);
    assert_eq!(
        captured.equipment_items_json,
        committed.equipment_items_json
    );
    assert_eq!(captured.gold, committed.gold);
    db.assert_no_recovery_checkpoint();
    assert_eq!(
        db.balance(),
        Some((FUNDING - quote.cost_bound_gold.get()) as i64)
    );
    assert_eq!(db.count("hunting_economy_burn_events"), 1);
    db.restart_with_real_login();
    let live = db.session.world_snapshot();
    assert_eq!(
        live.equipment_items
            .iter()
            .find(|item| item.slot == EquipmentSlot::Weapon)
            .unwrap()
            .durability_current,
        970
    );
    let account = db.account.clone();
    let queried = db.dispatch(
        Some(&account),
        82,
        wire::Action::Query {
            quote_id: quote.quote_id,
        },
    );
    assert!(matches!(
        queried.frame.reply,
        wire::Reply::Committed {
            replayed: true,
            reload_required: false,
            ..
        }
    ));
    assert!(queried.packets.is_empty());
    db.assert_no_recovery_checkpoint();
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_repair_carrier_postgres_shared_unknown_retirement_clears_binding_without_journal() {
    use crate::session::GatewayTeardownPersistenceOutcome;
    let mut db = Fixture::new(true, false);
    let quote = db.confirm_unknown();
    let committed = db.durable();
    db.set_moderation(true);
    for _ in 0..2 {
        let outcome = db.session.try_persist_teardown_once();
        assert!(
            matches!(outcome, GatewayTeardownPersistenceOutcome::Saved),
            "{outcome:?}"
        );
        assert!(db.session.active_identity().is_none());
        assert!(db.session.prepared_teardown_checkpoint().is_none());
    }
    assert_eq!(
        db.durable().revision,
        committed.revision,
        "retired World cannot save over real repaired checkpoint"
    );
    assert_eq!(
        db.durable().equipment_items_json,
        committed.equipment_items_json
    );
    assert_eq!(db.count("hunting_economy_burn_events"), 1);
    assert_eq!(
        db.balance(),
        Some((FUNDING - quote.cost_bound_gold.get()) as i64)
    );
    db.assert_no_recovery_checkpoint();
    db.set_moderation(false);
    db.restart_with_real_login();
    let live = db.session.world_snapshot();
    assert_eq!(
        live.equipment_items
            .iter()
            .find(|item| item.slot == EquipmentSlot::Weapon)
            .unwrap()
            .durability_current,
        970
    );
    db.assert_no_recovery_checkpoint();
}
