//! Opt-in campaign reservation proof in a dedicated, generated PostgreSQL realm.
//! No public database URI, Stripe request, administrative game command or DROP.
use mir2_protocol::{ClientPacket, MirDirection, ServerPacket};
use mir2_simulation::activities::{ActivityPhase, ActivityPolicy, ActivityStatus};
use mir2_simulation::monthly_card::monthly_card_now_ms;
use mir2_simulation::{
    AccountRecord, AccountStoreDatabaseMode, CharacterSaveRecord, InProcessWorldRuntime,
    SimulationConfig, WorldCommand, WorldRuntime,
};
use postgres::{Client, Config, NoTls};
use serde_json::Value;
use std::sync::{Arc, Barrier};
use std::time::{SystemTime, UNIX_EPOCH};

const ACCOUNT: &str = "activities_pg_qa";
const CHARACTER: i32 = 41;
const PASSWORD: &str = "isolated-activities-fixture-password";
const WRITERS: usize = 4;

fn checked<T>(result: Result<T, String>, operation: &str) -> T {
    result.unwrap_or_else(|error| {
        // Domain labels classify a failure without ever printing a connection URI.
        let class = if error.contains("OUTCOME_UNKNOWN") {
            "outcome-unknown"
        } else if error.contains("stale postgres") {
            "stale-source-CAS"
        } else if error.contains("stale full character") {
            "stale-character-revision"
        } else {
            "rejected"
        };
        panic!("isolated activities {operation}: {class}; credentials suppressed")
    })
}

fn open(scoped: &str, policy: &ActivityPolicy) -> SimulationConfig {
    let config = SimulationConfig::default()
        .with_postgres_account_store(scoped.to_owned())
        .unwrap_or_else(|_| panic!("isolated activities load failed; credentials suppressed"))
        .with_activity_policy(policy.clone());
    assert_eq!(
        config.account_store_database_mode,
        AccountStoreDatabaseMode::SourceOfTruth
    );
    assert!(config.account_store_path.is_none());
    config
}

struct Durable {
    account: AccountRecord,
    raw: Value,
    save_raw: Value,
    account_version: i64,
    save_version: i64,
}

fn durable(db: &mut Client) -> Durable {
    let account_row = db
        .query_one(
            "SELECT raw_json,store_version FROM accounts WHERE account_id=$1",
            &[&ACCOUNT],
        )
        .unwrap_or_else(|_| panic!("isolated activities account observer failed"));
    let save_row = db.query_one("SELECT snapshot_json,save_version FROM character_saves WHERE account_id=$1 AND character_index=$2", &[&ACCOUNT, &CHARACTER])
        .unwrap_or_else(|_| panic!("isolated activities save observer failed"));
    let raw: Value = account_row.get(0);
    let save_raw: Value = save_row.get(0);
    let account: AccountRecord = serde_json::from_value(raw.clone())
        .unwrap_or_else(|_| panic!("isolated activities account JSON invalid"));
    let save: CharacterSaveRecord = serde_json::from_value(save_raw.clone())
        .unwrap_or_else(|_| panic!("isolated activities saved JSON invalid"));
    assert_eq!(
        serde_json::to_value(&account.saves[&CHARACTER]).unwrap(),
        serde_json::to_value(&save).unwrap(),
        "account JSON and authoritative character snapshot must agree"
    );
    Durable {
        account,
        raw,
        save_raw,
        account_version: account_row.get(1),
        save_version: save_row.get(1),
    }
}

fn unchanged(before: &Durable, after: &Durable) {
    assert_eq!(before.raw, after.raw);
    assert_eq!(before.save_raw, after.save_raw);
    assert_eq!(before.account_version, after.account_version);
    assert_eq!(before.save_version, after.save_version);
}

fn claimed(status: &ActivityStatus, id: &str) -> bool {
    status
        .rewards
        .iter()
        .any(|reward| reward.id == id && reward.claimed)
}

fn login_start(source: &mut InProcessWorldRuntime) {
    let login = checked(
        source.execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: ACCOUNT.into(),
            password: PASSWORD.into(),
        })),
        "ordinary login",
    );
    assert!(login
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let start = checked(
        source.execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: CHARACTER,
        })),
        "ordinary StartGame",
    );
    assert!(start
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    let identity = source.active_identity().unwrap();
    assert_eq!(
        (identity.account_id.as_str(), identity.character_index),
        (ACCOUNT, CHARACTER)
    );
}

fn newer_source_revision_rejects_an_older_owner(
    scoped: &str,
    policy: &ActivityPolicy,
    observer: &mut Client,
) {
    let before = durable(observer);
    let mut older = InProcessWorldRuntime::new(open(scoped, policy));
    login_start(&mut older);
    let older_bootstrap = durable(observer);
    let original = older.active_character_checkpoint().unwrap();
    // Normal StartGame can commit its bootstrap save. Compare the owner to
    // that observed durable revision, never the pre-login snapshot.
    assert_eq!(
        original.revision,
        older_bootstrap.account.saves[&CHARACTER].revision
    );
    assert!(original.revision >= before.account.saves[&CHARACTER].revision);
    assert_eq!(
        older_bootstrap.account.activities,
        before.account.activities
    );
    checked(
        older.execute(WorldCommand::ClientPacket(ClientPacket::Turn {
            direction: MirDirection::Left,
        })),
        "older Source ordinary Turn",
    );
    let older_debit = checked(
        older.execute(WorldCommand::ClientPacket(ClientPacket::DropGold {
            amount: 3,
        })),
        "older Source ordinary gold debit",
    );
    assert!(older_debit
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoseGold { gold: 3 })));
    let old_unsaved = older.active_character_checkpoint().unwrap();
    assert_eq!(old_unsaved.revision, original.revision);
    assert_eq!(old_unsaved.gold, original.gold - 3);
    assert_eq!(old_unsaved.direction, MirDirection::Left);
    unchanged(&older_bootstrap, &durable(observer));

    // This is another ordinary Source with its own cache, not an administrative
    // SQL rewrite. Its full save genuinely advances the private character fence.
    let before_newer_bootstrap = durable(observer);
    let mut newer = InProcessWorldRuntime::new(open(scoped, policy));
    login_start(&mut newer);
    let newer_bootstrap = durable(observer);
    let newer_original = newer.active_character_checkpoint().unwrap();
    assert_eq!(
        newer_original.revision,
        newer_bootstrap.account.saves[&CHARACTER].revision
    );
    assert!(newer_original.revision >= before_newer_bootstrap.account.saves[&CHARACTER].revision);
    assert_eq!(
        newer_bootstrap.account.activities,
        before_newer_bootstrap.account.activities
    );
    checked(
        newer.execute(WorldCommand::ClientPacket(ClientPacket::Turn {
            direction: MirDirection::Down,
        })),
        "newer Source ordinary Turn",
    );
    let newer_debit = checked(
        newer.execute(WorldCommand::ClientPacket(ClientPacket::DropGold {
            amount: 7,
        })),
        "newer Source ordinary gold debit",
    );
    assert!(newer_debit
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoseGold { gold: 7 })));
    checked(
        newer.save_active_character(),
        "newer independent Source save",
    );
    let saved_newer = durable(observer);
    let newer_save = &saved_newer.account.saves[&CHARACTER];
    assert_eq!(newer_save.revision, newer_original.revision + 1);
    assert!(newer_save.revision > old_unsaved.revision);
    assert_eq!(newer_save.direction, MirDirection::Down);
    assert_eq!(newer_save.gold, newer_original.gold - 7);
    assert_eq!(newer_save.credit, newer_original.credit);
    assert_eq!(
        saved_newer.account.activities,
        newer_bootstrap.account.activities
    );
    assert!(saved_newer.account_version > newer_bootstrap.account_version);
    assert!(saved_newer.save_version > newer_bootstrap.save_version);

    // No cache refresh in the test: the real full-save path may refresh account
    // metadata, but must retain this old ECS snapshot's original revision.
    let error = older
        .save_active_character()
        .expect_err("old owner must not overwrite a newer private character revision");
    assert!(
        error.starts_with("stale full character save rejected:"),
        "metadata refresh must preserve the private full-save revision fence"
    );
    unchanged(&saved_newer, &durable(observer));
    let still_old = older.active_character_checkpoint().unwrap();
    assert_eq!(still_old.revision, old_unsaved.revision);
    assert_eq!(still_old.direction, old_unsaved.direction);
    assert_eq!(still_old.gold, old_unsaved.gold);
    assert_eq!(still_old.credit, old_unsaved.credit);
    drop(older);
    assert!(checked(
        newer.execute(WorldCommand::ClientPacket(ClientPacket::LogOut)),
        "newer owner ordinary logout after old-save rejection"
    )
    .iter()
    .any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })));
    drop(newer);

    let restarted = open(scoped, policy);
    let status = checked(
        restarted.activity_status(ACCOUNT, policy, monthly_card_now_ms()),
        "restart after genuine stale character rejection",
    );
    assert!(claimed(&status, "closed-v1-lv40") && claimed(&status, "closed-v1-lv50"));
    let preserved = durable(observer);
    assert_eq!(preserved.account.activities, saved_newer.account.activities);
    let preserved_save = &preserved.account.saves[&CHARACTER];
    assert_eq!(preserved_save.direction, MirDirection::Down);
    assert_eq!(preserved_save.gold, newer_original.gold - 7);
    assert_eq!(preserved_save.credit, newer_original.credit);
    assert!(preserved_save.revision >= newer_save.revision);
}

#[test]
#[ignore = "requires MIR2_ACTIVITY_TEST_ISOLATED=1 and dedicated mir2_activities_qa_ PostgreSQL database"]
fn activities_postgres_four_writers_restart_and_online_source_preserve_reservations() {
    assert_eq!(
        std::env::var("MIR2_ACTIVITY_TEST_ISOLATED").ok().as_deref(),
        Some("1"),
        "explicit isolated activities opt-in required"
    );
    let url = std::env::var("MIR2_ACTIVITY_TEST_DATABASE_URL")
        .expect("explicit private activities QA URI required");
    assert!(
        url.starts_with("postgres://") || url.starts_with("postgresql://"),
        "activities QA requires a PostgreSQL URI"
    );
    let parsed: Config = url
        .parse()
        .unwrap_or_else(|_| panic!("invalid activities QA URI; credentials suppressed"));
    let name = parsed.get_dbname().unwrap_or_default();
    assert!(
        name.starts_with("mir2_activities_qa_")
            && (28..=63).contains(&name.len())
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'),
        "activities QA database must have a dedicated generated name"
    );
    assert!(
        parsed.get_options().is_none(),
        "QA base URI cannot override schema options"
    );
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let schema = format!("activities_qa_{}_{stamp}", std::process::id());
    assert!(
        schema.len() <= 63
            && schema
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    );
    let mut base = Client::connect(&url, NoTls).unwrap_or_else(|_| {
        panic!("isolated activities connection failed; credentials suppressed")
    });
    assert_eq!(
        base.query_one("SELECT current_database()", &[])
            .unwrap_or_else(|_| panic!("isolated activities DB identity query failed"))
            .get::<_, String>(0),
        name
    );
    base.batch_execute(&format!("CREATE SCHEMA {schema}"))
        .unwrap_or_else(|_| panic!("isolated activities generated schema creation failed"));
    let scoped = format!("{url}{}options=-csearch_path%3D{schema}%20-clock_timeout%3D5000%20-cstatement_timeout%3D15000",
        if url.contains('?') { "&" } else { "?" });
    let mut observer = Client::connect(&scoped, NoTls)
        .unwrap_or_else(|_| panic!("isolated activities observer connection failed"));
    assert_eq!(
        observer
            .query_one("SELECT current_schema()", &[])
            .unwrap_or_else(|_| panic!("isolated activities schema identity query failed"))
            .get::<_, String>(0),
        schema
    );

    let now = monthly_card_now_ms();
    let policy = ActivityPolicy {
        phase: ActivityPhase::Closed,
        phase_start_ms: now - 10_000,
        phase_end_ms: None,
    };
    let seed = open(&scoped, &policy);
    let mut character = seed.default_character.clone();
    character.index = CHARACTER;
    character.name = "ActivitiesPg".into();
    character.level = 50;
    let mut account = AccountRecord::new(character);
    account.password = PASSWORD.into();
    account.saves.get_mut(&CHARACTER).unwrap().gold = 321;
    account.saves.get_mut(&CHARACTER).unwrap().credit = 87;
    {
        let mut store = seed.account_store.lock().unwrap();
        store.next_character_index = CHARACTER + 1;
        store.accounts.insert(ACCOUNT.into(), account);
    }
    checked(
        seed.save_account_store_account(ACCOUNT),
        "owned synthetic seed",
    );
    let stale_writer = open(&scoped, &policy);
    let before = durable(&mut observer);

    // Four truly independent caches/source versions, not SimulationConfig clones.
    let writers = (0..WRITERS)
        .map(|_| open(&scoped, &policy))
        .collect::<Vec<_>>();
    for left in 0..WRITERS {
        for right in (left + 1)..WRITERS {
            assert!(!Arc::ptr_eq(
                &writers[left].account_store,
                &writers[right].account_store
            ));
            assert_eq!(
                serde_json::to_value(&*writers[left].account_store.lock().unwrap()).unwrap(),
                serde_json::to_value(&*writers[right].account_store.lock().unwrap()).unwrap()
            );
        }
    }
    let barrier = Arc::new(Barrier::new(WRITERS));
    let handles = writers
        .into_iter()
        .map(|writer| {
            let barrier = barrier.clone();
            let policy = policy.clone();
            std::thread::spawn(move || {
                barrier.wait();
                // Only an explicitly rejected CAS may retry. A lost COMMIT never does.
                for _ in 0..16 {
                    match writer.activity_claim(
                        ACCOUNT,
                        CHARACTER,
                        "closed-v1-lv40",
                        &policy,
                        monthly_card_now_ms(),
                    ) {
                        Ok(status) => return status,
                        Err(error)
                            if !error.contains("OUTCOME_UNKNOWN")
                                && !error.contains("frozen")
                                && (error.starts_with("stale postgres account-store write")
                                    || error
                                        .starts_with("stale postgres character-save write")) =>
                        {
                            std::thread::yield_now()
                        }
                        Err(_) => panic!(
                            "isolated activities concurrent claim failed; credentials suppressed"
                        ),
                    }
                }
                panic!("isolated activities known CAS retry bound exceeded")
            })
        })
        .collect::<Vec<_>>();
    let joined = handles
        .into_iter()
        .map(|thread| thread.join())
        .collect::<Vec<_>>();
    let statuses = joined
        .into_iter()
        .map(|result| result.unwrap_or_else(|_| panic!("isolated activities writer failed")))
        .collect::<Vec<_>>();
    assert!(statuses
        .iter()
        .all(|status| claimed(status, "closed-v1-lv40")));
    let after = durable(&mut observer);
    let ledger = after.account.activities.as_ref().unwrap();
    assert_eq!(ledger.claims.len(), 1);
    let claim = &ledger.claims["closed-v1-lv40"];
    assert!(statuses.iter().all(|status| status
        .rewards
        .iter()
        .find(|reward| reward.id == "closed-v1-lv40")
        .unwrap()
        .claimed_at_ms
        == Some(claim.claimed_at_ms)));
    assert_eq!(
        before.save_raw, after.save_raw,
        "reservation must not alter a private save"
    );
    assert_eq!(
        after.account.saves[&CHARACTER].revision,
        before.account.saves[&CHARACTER].revision
    );
    assert_eq!(
        (
            after.account.saves[&CHARACTER].gold,
            after.account.saves[&CHARACTER].credit
        ),
        (321, 87)
    );
    assert!(after.account.monthly_card.is_none());
    assert!(after.account_version > before.account_version);

    // The obsolete metadata/source-version cache must never erase this claim.
    stale_writer
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut(ACCOUNT)
        .unwrap()
        .saves
        .get_mut(&CHARACTER)
        .unwrap()
        .gold = 999;
    assert!(stale_writer
        .save_account_store_account(ACCOUNT)
        .unwrap_err()
        .contains("stale postgres account-store write"));
    unchanged(&after, &durable(&mut observer));
    let restarted = open(&scoped, &policy);
    assert_eq!(
        restarted.account_store.lock().unwrap().schema_version,
        mir2_simulation::billing::BILLING_SCHEMA_VERSION,
    );
    let status = checked(
        restarted.activity_status(ACCOUNT, &policy, monthly_card_now_ms()),
        "restart status",
    );
    assert!(claimed(&status, "closed-v1-lv40"));
    checked(
        restarted.activity_claim(
            ACCOUNT,
            CHARACTER,
            "closed-v1-lv40",
            &policy,
            monthly_card_now_ms(),
        ),
        "restart claim replay",
    );
    assert_eq!(durable(&mut observer).save_raw, after.save_raw);

    // A live Source loaded before this independent website claim has stale
    // repository versions. Ordinary gameplay/save/logout must preserve both
    // the new account ledger and the current unsaved owner transform.
    let mut source = InProcessWorldRuntime::new(open(&scoped, &policy));
    login_start(&mut source);
    let source_before = source.active_character_checkpoint().unwrap();
    let saved_before_claim = durable(&mut observer);
    let website = open(&scoped, &policy);
    let claimed_online = checked(
        website.activity_claim(
            ACCOUNT,
            CHARACTER,
            "closed-v1-lv50",
            &policy,
            monthly_card_now_ms(),
        ),
        "website claim while Source active",
    );
    assert!(claimed(&claimed_online, "closed-v1-lv50"));
    let after_online_claim = durable(&mut observer);
    assert_eq!(after_online_claim.save_raw, saved_before_claim.save_raw);
    assert_eq!(
        source.active_character_checkpoint().unwrap().revision,
        source_before.revision
    );
    let direction = if source_before.direction == MirDirection::Up {
        MirDirection::Right
    } else {
        MirDirection::Up
    };
    let turned = checked(
        source.execute(WorldCommand::ClientPacket(ClientPacket::Turn { direction })),
        "ordinary Turn after website claim",
    );
    assert!(turned.iter().any(|packet| matches!(packet, ServerPacket::UserLocation { location } if location.direction == direction)));
    let unsaved = source.active_character_checkpoint().unwrap();
    assert_eq!(unsaved.direction, direction);
    assert_eq!(
        (unsaved.gold, unsaved.credit),
        (source_before.gold, source_before.credit)
    );
    checked(
        source.save_active_character(),
        "ordinary Source save after external metadata claim",
    );
    let saved = durable(&mut observer);
    assert_eq!(saved.account.activities.as_ref().unwrap().claims.len(), 2);
    assert_eq!(saved.account.saves[&CHARACTER].direction, direction);
    assert_eq!(
        (
            saved.account.saves[&CHARACTER].gold,
            saved.account.saves[&CHARACTER].credit
        ),
        (321, 87)
    );
    assert_eq!(
        saved.account.saves[&CHARACTER].character.level,
        source_before.character.level
    );
    assert_eq!(
        saved.account.saves[&CHARACTER].experience,
        source_before.experience
    );
    assert_eq!(
        saved.account.saves[&CHARACTER].revision,
        source_before.revision + 1
    );
    let logout = checked(
        source.execute(WorldCommand::ClientPacket(ClientPacket::LogOut)),
        "ordinary LogOut after claim/save",
    );
    assert!(logout
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })));
    assert!(source.active_identity().is_none());
    drop(source);
    let final_store = open(&scoped, &policy);
    let final_status = checked(
        final_store.activity_status(ACCOUNT, &policy, monthly_card_now_ms()),
        "final restarted status",
    );
    assert!(claimed(&final_status, "closed-v1-lv40") && claimed(&final_status, "closed-v1-lv50"));
    let final_saved = durable(&mut observer);
    assert_eq!(
        (
            final_saved.account.saves[&CHARACTER].gold,
            final_saved.account.saves[&CHARACTER].credit
        ),
        (321, 87)
    );
    assert_eq!(final_saved.account.saves[&CHARACTER].direction, direction);
    assert_eq!(
        final_saved
            .account
            .activities
            .as_ref()
            .unwrap()
            .claims
            .len(),
        2
    );
    assert!(final_saved.account.monthly_card.is_none());
    let mut relogin = InProcessWorldRuntime::new(open(&scoped, &policy));
    login_start(&mut relogin);
    let checkpoint = relogin.active_character_checkpoint().unwrap();
    assert_eq!(
        (checkpoint.gold, checkpoint.credit, checkpoint.direction),
        (321, 87, direction)
    );
    assert!(checked(
        relogin.execute(WorldCommand::ClientPacket(ClientPacket::LogOut)),
        "final normal logout"
    )
    .iter()
    .any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })));
    drop(relogin);
    newer_source_revision_rejects_an_older_owner(&scoped, &policy, &mut observer);
    // Generated schema is retained as evidence; only the isolated runner may
    // dispose its explicitly dedicated QA database after collecting results.
}
