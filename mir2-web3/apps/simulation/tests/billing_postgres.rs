//! Opt-in billing JSON/CAS proof against an explicitly isolated PostgreSQL DB.
//! No production URI, shared schema, Stripe call, or uncertain-commit retry.
use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::billing::{RechargeOffer, RechargeOrderState, VerifiedRechargePayment};
use mir2_simulation::monthly_card::{
    monthly_card_now_ms, MonthlyCardPolicy, MONTHLY_CARD_DURATION_MS,
};
use mir2_simulation::{
    AccountRecord, AccountStoreDatabaseMode, CharacterSaveRecord, InProcessWorldRuntime,
    SimulationConfig, WorldCommand, WorldRuntime,
};
use postgres::{Client, Config, NoTls};
use serde_json::Value;
use std::sync::{Arc, Barrier};
use std::time::{SystemTime, UNIX_EPOCH};

const ACCOUNT: &str = "billing_pg_qa";
const CHARACTER: i32 = 37;
const PASSWORD: &str = "isolated-billing-fixture-password";
const KEY: &str = "isolated-billing-postgres-qa-key-never-deploy";
const WRITERS: usize = 4;

fn open(scoped: &str) -> SimulationConfig {
    let config = SimulationConfig::default()
        .with_postgres_account_store(scoped.to_owned())
        .unwrap_or_else(|_| panic!("isolated billing load failed; credentials suppressed"))
        .with_billing_monthly_card_credit_price(Some(10))
        .unwrap()
        .with_monthly_card_policy(MonthlyCardPolicy::new(true, Some(KEY)).unwrap());
    assert_eq!(
        config.account_store_database_mode,
        AccountStoreDatabaseMode::SourceOfTruth
    );
    assert!(config.account_store_path.is_none());
    config
}

fn offer(credits: u32) -> RechargeOffer {
    RechargeOffer {
        id: format!("pg-credits-{credits}"),
        label: "Isolated Credits".into(),
        currency: "usd".into(),
        amount_minor: 500,
        credits,
    }
}

fn payment(session: &str, intent: &str) -> VerifiedRechargePayment {
    VerifiedRechargePayment {
        session_id: session.into(),
        payment_intent_id: intent.into(),
        amount_minor: 500,
        currency: "usd".into(),
        livemode: false,
    }
}

fn checked<T>(result: Result<T, String>, phase: &str) -> T {
    result.unwrap_or_else(|_| panic!("isolated billing {phase} failed; credentials suppressed"))
}

/// Each contender owns its own cache, mutexes, source versions and connection.
/// Only a known pre-commit CAS rejection is retried; unknown outcomes fail.
fn race<T, F>(scoped: &str, phase: &'static str, operation: F) -> Vec<T>
where
    T: Send + 'static,
    F: Fn(&SimulationConfig) -> Result<T, String> + Send + Sync + 'static,
{
    let barrier = Arc::new(Barrier::new(WRITERS));
    let operation = Arc::new(operation);
    // Open every cache before spawning barrier participants, so a connection
    // failure cannot strand a partial group at the barrier.
    let writers: Vec<_> = (0..WRITERS).map(|_| open(scoped)).collect();
    let threads: Vec<_> = writers
        .into_iter()
        .map(|writer| {
            let barrier = Arc::clone(&barrier);
            let operation = Arc::clone(&operation);
            std::thread::spawn(move || {
                barrier.wait();
                for _ in 0..16 {
                    match operation(&writer) {
                        Ok(receipt) => return receipt,
                        Err(error)
                            if !error.contains("OUTCOME_UNKNOWN")
                                && !error.contains("frozen")
                                && (error.contains("stale postgres account-store write")
                                    || error.contains("stale postgres character-save write")) =>
                        {
                            std::thread::yield_now();
                        }
                        Err(_) => panic!(
                            "isolated billing {phase} failed outside a known CAS race; credentials suppressed"
                        ),
                    }
                }
                panic!("isolated billing {phase} CAS retry bound exceeded");
            })
        })
        .collect();
    // Join every writer before surfacing a failure; no test writer is detached.
    let joined: Vec<_> = threads.into_iter().map(|thread| thread.join()).collect();
    joined
        .into_iter()
        .map(|result| result.unwrap_or_else(|_| panic!("isolated billing {phase} writer failed")))
        .collect()
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
        .unwrap_or_else(|_| panic!("isolated billing account observer failed"));
    let save_row = db
        .query_one(
            "SELECT snapshot_json,save_version FROM character_saves \
             WHERE account_id=$1 AND character_index=$2",
            &[&ACCOUNT, &CHARACTER],
        )
        .unwrap_or_else(|_| panic!("isolated billing save observer failed"));
    let raw: Value = account_row.get(0);
    let save_raw: Value = save_row.get(0);
    let account: AccountRecord = serde_json::from_value(raw.clone())
        .unwrap_or_else(|_| panic!("isolated billing account JSON invalid"));
    let snapshot: CharacterSaveRecord = serde_json::from_value(save_raw.clone())
        .unwrap_or_else(|_| panic!("isolated billing character JSON invalid"));
    assert!(
        serde_json::to_value(&account.saves[&CHARACTER]).unwrap()
            == serde_json::to_value(&snapshot).unwrap(),
        "authoritative account JSON and character snapshot disagree"
    );
    Durable {
        account,
        raw,
        save_raw,
        account_version: account_row.get(1),
        save_version: save_row.get(1),
    }
}

fn unchanged(before: &Durable, after: &Durable, phase: &str) {
    unchanged_business(before, after, phase);
    assert_eq!(before.account_version, after.account_version, "{phase}");
    assert_eq!(before.save_version, after.save_version, "{phase}");
}

fn unchanged_business(before: &Durable, after: &Durable, phase: &str) {
    assert!(before.raw == after.raw, "{phase} changed account JSON");
    assert!(
        before.save_raw == after.save_raw,
        "{phase} changed save JSON"
    );
}

#[test]
#[ignore = "requires MIR2_BILLING_TEST_ISOLATED=1 and a dedicated mir2_billing_qa_ PostgreSQL database"]
fn billing_postgres_independent_writers_restart_and_stale_owner_preserve_cash() {
    assert_eq!(
        std::env::var("MIR2_BILLING_TEST_ISOLATED").ok().as_deref(),
        Some("1"),
        "explicit isolated billing opt-in required"
    );
    let url = std::env::var("MIR2_BILLING_TEST_DATABASE_URL")
        .expect("explicit private billing QA URI required");
    assert!(
        url.starts_with("postgres://") || url.starts_with("postgresql://"),
        "isolated billing QA requires a PostgreSQL URI"
    );
    let parsed: Config = url
        .parse()
        .unwrap_or_else(|_| panic!("invalid billing QA URI; credentials suppressed"));
    let name = parsed.get_dbname().unwrap_or_default();
    assert!(
        name.starts_with("mir2_billing_qa_")
            && (28..=63).contains(&name.len())
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'),
        "billing QA database must have a dedicated generated name"
    );
    assert!(
        parsed.get_options().is_none(),
        "QA base URI cannot override schema options"
    );
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let schema = format!("billing_qa_{}_{stamp}", std::process::id());
    assert!(schema.len() <= 63);
    let mut base = Client::connect(&url, NoTls)
        .unwrap_or_else(|_| panic!("isolated billing connection failed; credentials suppressed"));
    assert_eq!(
        base.query_one("SELECT current_database()", &[])
            .unwrap_or_else(|_| panic!("isolated billing database identity query failed"))
            .get::<_, String>(0),
        name
    );
    base.batch_execute(&format!("CREATE SCHEMA {schema}"))
        .unwrap_or_else(|_| panic!("isolated billing generated schema creation failed"));
    let scoped = format!(
        "{url}{}options=-csearch_path%3D{schema}%20-clock_timeout%3D5000%20-cstatement_timeout%3D15000",
        if url.contains('?') { "&" } else { "?" }
    );
    let mut observer = Client::connect(&scoped, NoTls)
        .unwrap_or_else(|_| panic!("isolated billing observer connection failed"));
    assert_eq!(
        observer
            .query_one("SELECT current_schema()", &[])
            .unwrap_or_else(|_| panic!("isolated billing schema identity query failed"))
            .get::<_, String>(0),
        schema
    );

    let seed = open(&scoped);
    let mut character = seed.default_character.clone();
    character.index = CHARACTER;
    character.name = "BillingPg".into();
    let mut account = AccountRecord::new(character);
    account.password = PASSWORD.into();
    account.saves.get_mut(&CHARACTER).unwrap().gold = 321;
    {
        let mut store = seed.account_store.lock().unwrap();
        store.next_character_index = CHARACTER + 1;
        store.accounts.insert(ACCOUNT.into(), account);
    }
    checked(seed.save_account_store_account(ACCOUNT), "owned seed");
    let stale_writer = open(&scoped);
    let now = monthly_card_now_ms();

    let prepared = race(&scoped, "prepare", move |writer| {
        writer.prepare_recharge_order(
            ACCOUNT,
            CHARACTER,
            "pg-recharge-once",
            &offer(100),
            now,
            false,
        )
    });
    let order = prepared[0].clone();
    assert!(prepared.iter().all(|receipt| receipt == &order));
    assert_eq!(order.state, RechargeOrderState::Prepared);
    assert_eq!(order.created_at_ms, now);
    assert_eq!(
        checked(
            open(&scoped).recharge_orders(ACCOUNT, CHARACTER),
            "prepared reload"
        )
        .len(),
        1
    );
    let before_stale = durable(&mut observer);
    // A deterministic obsolete source-version write complements the live
    // concurrent retries. It must not erase the newly persisted billing ledger.
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
        .credit = 999;
    let error = stale_writer
        .save_account_store_account(ACCOUNT)
        .unwrap_err();
    assert!(error.contains("stale postgres account-store write"));
    unchanged(&before_stale, &durable(&mut observer), "stale source CAS");

    let order_id = order.id.clone();
    let bound = race(&scoped, "checkout bind", move |writer| {
        writer.bind_recharge_checkout(ACCOUNT, &order_id, "cs_billing_pg_once", now + 1)
    });
    assert!(bound.iter().all(|receipt| receipt == &bound[0]));
    assert_eq!(bound[0].state, RechargeOrderState::CheckoutBound);
    let order_id = order.id.clone();
    let confirmed = race(&scoped, "duplicate webhook", move |writer| {
        writer.confirm_recharge_payment(
            ACCOUNT,
            &order_id,
            &payment("cs_billing_pg_once", "pi_billing_pg_once"),
            now + 2,
        )
    });
    assert!(confirmed.iter().all(|receipt| receipt == &confirmed[0]));
    assert_eq!(confirmed[0].state, RechargeOrderState::PaidPending);
    assert_eq!(
        confirmed[0].checkout_session_id.as_deref(),
        Some("cs_billing_pg_once")
    );
    assert_eq!(
        confirmed[0].payment_intent_id.as_deref(),
        Some("pi_billing_pg_once")
    );
    assert_eq!(confirmed[0].paid_at_ms, Some(now + 2));
    let binding_writer = open(&scoped);
    assert_eq!(
        checked(
            binding_writer.recharge_credit_status(ACCOUNT, CHARACTER),
            "pending status"
        ),
        (0, 100)
    );
    let before_invalid = durable(&mut observer);
    for field in 0..5 {
        let mut invalid = payment("cs_billing_pg_once", "pi_billing_pg_once");
        match field {
            0 => invalid.session_id = "cs_billing_pg_rebound".into(),
            1 => invalid.payment_intent_id = "pi_billing_pg_rebound".into(),
            2 => invalid.amount_minor += 1,
            3 => invalid.currency = "eur".into(),
            _ => invalid.livemode = true,
        }
        assert_eq!(
            binding_writer
                .confirm_recharge_payment(ACCOUNT, &order.id, &invalid, now + 3)
                .unwrap_err(),
            "billingPaymentBindingMismatch"
        );
    }
    assert_eq!(
        binding_writer
            .bind_recharge_checkout(ACCOUNT, &order.id, "cs_billing_pg_rebound", now + 3)
            .unwrap_err(),
        "billingCheckoutBindingMismatch"
    );
    unchanged(
        &before_invalid,
        &durable(&mut observer),
        "immutable provider tuple rejection",
    );

    let applied = race(&scoped, "inactive grant", move |writer| {
        writer.consume_inactive_pending_recharge_credits(ACCOUNT, CHARACTER, now + 3)
    });
    assert_eq!(
        applied
            .iter()
            .filter(|receipt| receipt.granted != 0)
            .count(),
        1
    );
    assert_eq!(
        applied.iter().map(|receipt| receipt.granted).sum::<u32>(),
        100
    );
    assert!(applied
        .iter()
        .all(|receipt| receipt.credit == 100 && receipt.revision == 1));
    assert_eq!(
        applied
            .iter()
            .flat_map(|receipt| &receipt.order_ids)
            .collect::<Vec<_>>(),
        vec![&order.id]
    );
    let purchased = race(&scoped, "monthly purchase", move |writer| {
        writer.buy_monthly_card_for_character(ACCOUNT, CHARACTER, "pg-card-once", now + 4)
    });
    assert_eq!(
        purchased.iter().filter(|receipt| !receipt.replayed).count(),
        1
    );
    let uid = purchased[0].unique_id;
    assert!(purchased.iter().all(|receipt| receipt.unique_id == uid));
    let purchased_store = open(&scoped);
    assert_eq!(
        checked(
            purchased_store.recharge_credit_status(ACCOUNT, CHARACTER),
            "purchase credit"
        ),
        (90, 0)
    );
    let cards = checked(
        purchased_store.list_billing_monthly_cards(ACCOUNT, CHARACTER),
        "one owned unit",
    );
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].unique_id, uid);
    let activated = race(&scoped, "monthly activation", move |writer| {
        writer.activate_monthly_card_for_character(ACCOUNT, CHARACTER, uid, now + 5)
    });
    let expiry = now + 5 + MONTHLY_CARD_DURATION_MS;
    assert_eq!(
        activated.iter().filter(|receipt| !receipt.replayed).count(),
        1
    );
    assert!(activated.iter().all(|receipt| receipt.unique_id == uid
        && receipt.status.expires_at_ms == Some(expiry)
        && receipt.status.can_enter_game));

    drop(seed);
    drop(stale_writer);
    drop(binding_writer);
    drop(purchased_store);
    let restarted = open(&scoped);
    assert_eq!(restarted.account_store.lock().unwrap().schema_version, 8);
    assert_eq!(
        checked(
            restarted.recharge_credit_status(ACCOUNT, CHARACTER),
            "restart credit"
        ),
        (90, 0)
    );
    assert!(checked(
        restarted.list_billing_monthly_cards(ACCOUNT, CHARACTER),
        "restart owned units"
    )
    .is_empty());
    let stable = durable(&mut observer);
    let persisted_order = &stable.account.billing.as_ref().unwrap().orders[&order.id];
    assert_eq!(persisted_order.state, RechargeOrderState::Applied);
    let receipt = persisted_order.applied.as_ref().unwrap();
    assert_eq!(
        (
            receipt.credits,
            receipt.credit_before,
            receipt.credit_after,
            receipt.revision
        ),
        (100, 0, 100, 1)
    );
    let monthly = stable.account.monthly_card.as_ref().unwrap();
    assert_eq!(monthly.expires_at_ms, expiry);
    assert_eq!(monthly.item_receipts.len(), 1);
    let unit = monthly.item_receipts.values().next().unwrap();
    assert_eq!(unit.unique_id, uid);
    assert_eq!(unit.account_id, ACCOUNT);
    assert_eq!(unit.character_index, CHARACTER);
    assert_eq!(unit.credit_price, 10);
    assert_eq!(unit.purchase_quantity, 1);
    assert_eq!(unit.redeemed_at_ms, Some(now + 5));
    assert_eq!(unit.credited_until_ms, Some(expiry));
    assert_eq!(stable.account.saves[&CHARACTER].revision, 3);
    assert!(stable.account_version >= 7 && stable.save_version >= 4);
    assert!(!stable.raw.to_string().contains(KEY));
    assert_eq!(
        checked(
            restarted.confirm_recharge_payment(
                ACCOUNT,
                &order.id,
                &payment("cs_billing_pg_once", "pi_billing_pg_once"),
                now + 6
            ),
            "restart webhook"
        )
        .applied,
        persisted_order.applied
    );
    assert_eq!(
        checked(
            restarted.consume_inactive_pending_recharge_credits(ACCOUNT, CHARACTER, now + 6),
            "restart grant replay"
        )
        .granted,
        0
    );
    assert!(
        checked(
            restarted.buy_monthly_card_for_character(ACCOUNT, CHARACTER, "pg-card-once", now + 6),
            "restart purchase replay"
        )
        .replayed
    );
    assert!(
        checked(
            restarted.activate_monthly_card_for_character(ACCOUNT, CHARACTER, uid, now + 6),
            "restart activation replay"
        )
        .replayed
    );
    // The repository may advance internal CAS versions for a replayed account
    // write. The business JSON and character revision must remain unchanged.
    unchanged_business(&stable, &durable(&mut observer), "restart receipt replays");
    drop(restarted);

    // Real Source login/StartGame and ordinary Tick observe the independent
    // payment writer. The grant commits the current unsaved owner checkpoint.
    let mut source = InProcessWorldRuntime::new(open(&scoped));
    let login = checked(
        source.execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: ACCOUNT.into(),
            password: PASSWORD.into(),
        })),
        "ordinary Source login",
    );
    assert!(login
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let start = checked(
        source.execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: CHARACTER,
        })),
        "ordinary Source StartGame",
    );
    assert!(start
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    assert_eq!(source.active_identity().unwrap().account_id, ACCOUNT);
    let stale_checkpoint = source.active_character_checkpoint().unwrap();
    assert_eq!((stale_checkpoint.credit, stale_checkpoint.gold), (90, 321));
    let mut unsaved = stale_checkpoint.clone();
    unsaved.gold = 444;
    checked(
        source.restore_active_character_checkpoint(&unsaved),
        "current owner checkpoint",
    );
    let external = open(&scoped);
    let paid_at = monthly_card_now_ms();
    let online_order = checked(
        external.prepare_recharge_order(
            ACCOUNT,
            CHARACTER,
            "pg-online-recharge",
            &offer(25),
            paid_at,
            false,
        ),
        "external online prepare",
    );
    checked(
        external.confirm_recharge_payment(
            ACCOUNT,
            &online_order.id,
            &payment("cs_billing_pg_online", "pi_billing_pg_online"),
            paid_at,
        ),
        "external online payment",
    );
    let tick = checked(
        source.execute(WorldCommand::Tick),
        "ordinary Source Tick grant",
    );
    assert_eq!(
        tick.iter()
            .filter_map(|packet| match packet {
                ServerPacket::GainedCredit { credit } => Some(*credit),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![25]
    );
    let current = source.active_character_checkpoint().unwrap();
    assert_eq!((current.credit, current.gold), (115, 444));
    assert_eq!(current.revision, stale_checkpoint.revision + 1);
    let paid = checked(
        external.confirm_recharge_payment(
            ACCOUNT,
            &online_order.id,
            &payment("cs_billing_pg_online", "pi_billing_pg_online"),
            paid_at + 1,
        ),
        "online webhook replay",
    );
    assert_eq!(paid.state, RechargeOrderState::Applied);
    let refresh = checked(
        source.execute(WorldCommand::BillingRefresh {
            character_index: CHARACTER,
        }),
        "immediate Source refresh replay",
    );
    assert!(!refresh
        .iter()
        .any(|packet| matches!(packet, ServerPacket::GainedCredit { .. })));
    let committed = durable(&mut observer);
    assert_eq!(
        (
            committed.account.saves[&CHARACTER].credit,
            committed.account.saves[&CHARACTER].gold
        ),
        (115, 444)
    );
    assert_eq!(committed.account.billing.as_ref().unwrap().orders.len(), 2);
    assert_eq!(paid.applied.as_ref().unwrap().credit_before, 90);
    assert_eq!(paid.applied.as_ref().unwrap().credit_after, 115);
    assert_eq!(paid.applied.as_ref().unwrap().revision, current.revision);
    checked(
        source.restore_active_character_checkpoint(&stale_checkpoint),
        "obsolete owner fixture",
    );
    let error = source.save_active_character().unwrap_err();
    assert!(error.contains("stale full character save rejected"));
    unchanged(
        &committed,
        &durable(&mut observer),
        "obsolete owner full save",
    );
    let final_restart = open(&scoped);
    assert_eq!(
        checked(
            final_restart.recharge_credit_status(ACCOUNT, CHARACTER),
            "final restart credit"
        ),
        (115, 0)
    );
    assert_eq!(
        checked(
            final_restart.recharge_orders(ACCOUNT, CHARACTER),
            "final restart receipts"
        )
        .iter()
        .filter(|row| row.state == RechargeOrderState::Applied)
        .count(),
        2
    );
    assert_eq!(
        checked(
            final_restart.monthly_card_status(ACCOUNT, monthly_card_now_ms()),
            "final restart access"
        )
        .expires_at_ms,
        Some(expiry)
    );
    // Keep the generated schema as failure evidence. Only the runner disposes
    // the explicitly isolated database; this test performs no DROP or cleanup.
}
