//! Explicit isolated PostgreSQL receipt/CAS proof. No production URL fallback.
use mir2_simulation::monthly_card::{
    monthly_card_now_ms, MonthlyCardPolicy, MONTHLY_CARD_DURATION_MS,
};
use mir2_simulation::{AccountRecord, SimulationConfig};
use postgres::{Client, Config, NoTls};
use std::sync::{Arc, Barrier};
const KEY: &str = "isolated-monthly-postgres-qa-key-never-deploy";

#[test]
#[ignore = "requires MIR2_MONTHLY_CARD_TEST_ISOLATED=1 and a dedicated mir2_monthly_qa_ PostgreSQL database"]
fn monthly_card_postgres_independent_writers_and_restart_are_once() {
    assert_eq!(
        std::env::var("MIR2_MONTHLY_CARD_TEST_ISOLATED")
            .ok()
            .as_deref(),
        Some("1")
    );
    let url = std::env::var("MIR2_MONTHLY_CARD_TEST_DATABASE_URL")
        .expect("explicit private monthly QA URI required");
    let parsed: Config = url
        .parse()
        .unwrap_or_else(|_| panic!("invalid monthly QA URI; credentials suppressed"));
    let name = parsed.get_dbname().unwrap_or_default();
    assert!(
        name.starts_with("mir2_monthly_qa_")
            && name.len() >= 28
            && name.len() <= 63
            && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    );
    assert!(
        parsed.get_options().is_none(),
        "QA base URI cannot override schema options"
    );
    let schema = format!(
        "monthly_qa_{}_{}",
        std::process::id(),
        monthly_card_now_ms()
    );
    let mut db = Client::connect(&url, NoTls)
        .unwrap_or_else(|_| panic!("QA connection failed; credentials suppressed"));
    assert_eq!(
        db.query_one("SELECT current_database()", &[])
            .unwrap()
            .get::<_, String>(0),
        name
    );
    db.batch_execute(&format!("CREATE SCHEMA {schema}"))
        .expect("create generated absent QA schema");
    let scoped = format!("{url}{}options=-csearch_path%3D{schema}%20-clock_timeout%3D5000%20-cstatement_timeout%3D15000",
        if url.contains('?') { "&" } else { "?" });
    let open = || {
        SimulationConfig::default()
            .with_postgres_account_store(scoped.clone())
            .unwrap_or_else(|_| {
                panic!("isolated account store load failed; credentials suppressed")
            })
            .with_monthly_card_policy(MonthlyCardPolicy::new(true, Some(KEY)).unwrap())
    };
    let config = open();
    let character = config.default_character.clone();
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .insert("monthly_pg_qa".into(), AccountRecord::new(character));
    config
        .save_account_store_account("monthly_pg_qa")
        .unwrap_or_else(|_| panic!("owned account seed failed"));
    let now = monthly_card_now_ms();
    let issued = config
        .issue_monthly_card("monthly_pg_qa", "pg-issuance-20261006-01", now)
        .unwrap();
    let barrier = Arc::new(Barrier::new(4));
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let writer = open();
            let barrier = Arc::clone(&barrier);
            let code = issued.code.clone();
            std::thread::spawn(move || {
                barrier.wait();
                for _ in 0..8 {
                    match writer.redeem_monthly_card("monthly_pg_qa", &code, now) {
                        Ok(receipt) => return receipt,
                        Err(error) if error.contains("stale postgres account-store write") => {
                            std::thread::yield_now()
                        }
                        Err(_) => panic!("isolated redemption failed outside expected CAS race"),
                    }
                }
                panic!("isolated CAS retry bound exceeded");
            })
        })
        .collect();
    let receipts: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(receipts.iter().filter(|r| !r.replayed).count(), 1);
    let reloaded = open();
    assert_eq!(
        reloaded
            .monthly_card_status("monthly_pg_qa", now)
            .unwrap()
            .expires_at_ms,
        Some(now + MONTHLY_CARD_DURATION_MS)
    );
    assert!(
        reloaded
            .redeem_monthly_card("monthly_pg_qa", &issued.code, now + 1)
            .unwrap()
            .replayed
    );
    let renewal = reloaded
        .issue_monthly_card("monthly_pg_qa", "pg-issuance-20261006-02", now + 2)
        .unwrap();
    let renewed = reloaded
        .redeem_monthly_card("monthly_pg_qa", &renewal.code, now + 2)
        .unwrap();
    assert_eq!(
        renewed.status.expires_at_ms,
        Some(now + 2 * MONTHLY_CARD_DURATION_MS)
    );
    let mut observer =
        Client::connect(&scoped, NoTls).unwrap_or_else(|_| panic!("isolated observer failed"));
    let row = observer
        .query_one(
            "SELECT raw_json,store_version FROM accounts WHERE account_id=$1",
            &[&"monthly_pg_qa"],
        )
        .unwrap();
    let value: serde_json::Value = row.get(0);
    let account: AccountRecord = serde_json::from_value(value.clone())
        .unwrap_or_else(|_| panic!("owned account JSON invalid"));
    assert_eq!(
        account
            .monthly_card
            .unwrap()
            .codes
            .values()
            .filter(|row| row.redeemed_at_ms.is_some())
            .count(),
        2
    );
    assert!(!value.to_string().contains(&issued.code) && !value.to_string().contains(KEY));
    assert!(row.get::<_, i64>(1) >= 5);
    // The CI service is ephemeral. Keep the generated schema as failure evidence
    // until the isolated database itself is disposed by the runner.
}
