//! Explicit, local-only test-season setup. No payment signer or game data seeding.
use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use mir2_simulation::promotion::{CampaignDraft, PromotionStore};
use mir2_simulation::{apply_migrations, AccountRecord};
use postgres::{Client, Config, NoTls};
use rand_core::{OsRng, RngCore};
use serde_json::{json, Value};

const CAMPAIGN: &str = "numeron-playtest-20261029";
const OPERATIONS: &str = "system_promotion_playtest_operations_20261029";
const FINANCE: &str = "system_promotion_playtest_finance_20261029";

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 || args[0] != "--activate-test-season" {
        return Err("usage: promotion_playtest --activate-test-season <test-config.json>".into());
    }
    let url = std::env::var("MIR2_PROMOTION_TEST_DATABASE_URL")
        .map_err(|_| "local test database environment required")?;
    let parsed: Config = url
        .parse()
        .map_err(|_| "invalid test database configuration")?;
    if !parsed
        .get_dbname()
        .unwrap_or_default()
        .contains("promotion")
        || parsed.get_hosts().is_empty()
        || parsed.get_hosts().iter().any(|h| {
            !matches!(h,
            postgres::config::Host::Tcp(n) if n == "127.0.0.1" || n == "localhost")
        })
    {
        return Err("only a loopback promotion test database is accepted".into());
    }
    let config: Value = serde_json::from_slice(
        &std::fs::read(&args[1]).map_err(|_| "cannot read test configuration")?,
    )
    .map_err(|_| "invalid test configuration JSON")?;
    let draft: CampaignDraft = serde_json::from_value(config["draft"].clone())
        .map_err(|_| "invalid test campaign draft")?;
    if config["environment"] != "test"
        || draft.campaign_id != CAMPAIGN
        || !draft.rules.test_wallet_enrollment
        || draft.funding_mode != "revenue_share"
        || draft.budget_micro_usd != 0
        || draft.rules.reserve_bps != 0
        || draft.ends_at_ms.checked_sub(draft.starts_at_ms) != Some(7 * 86_400_000)
        || config["intendedSwitches"]["payoutEnabled"] != false
    {
        return Err(
            "only the reviewed seven-day, non-paying test configuration is accepted".into(),
        );
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "clock unavailable")?
        .as_millis() as u64;
    if now < draft.starts_at_ms || now >= draft.ends_at_ms {
        return Err("test season must currently be in its configured date range".into());
    }
    let mut db = Client::connect(&url, NoTls).map_err(|_| "local test database unavailable")?;
    apply_migrations(&mut db)?;
    let mut tx = db.transaction().map_err(|_| "cannot begin test setup")?;
    for (id, role) in [(OPERATIONS, "operations"), (FINANCE, "finance")] {
        let mut random = [0u8; 32];
        OsRng
            .try_fill_bytes(&mut random)
            .map_err(|_| "entropy unavailable")?;
        let mut record = AccountRecord::empty();
        record.password = Argon2::default()
            .hash_password(&random, &SaltString::generate(&mut OsRng))
            .map_err(|_| "cannot lock system account password")?
            .to_string();
        let raw = serde_json::to_value(record).map_err(|_| "system account encoding failed")?;
        tx.execute("INSERT INTO accounts(account_id,raw_json) VALUES($1,$2) ON CONFLICT(account_id) DO NOTHING", &[&id, &raw])
            .map_err(|_| "cannot provision local test actor")?;
        tx.execute("INSERT INTO promotion_operator_roles(environment,account_id,role,granted_by,granted_at_ms) VALUES('test',$1,$2,'user-requested-isolated-playtest-not-human-finance-approval',$3) ON CONFLICT DO NOTHING", &[&id, &role, &(now as i64)])
            .map_err(|_| "cannot grant local test role")?;
    }
    tx.commit()
        .map_err(|_| "cannot persist local test actors")?;
    let store = PromotionStore::new(url, "test")?;
    let current = store.public_campaign(CAMPAIGN)?;
    if current["status"] == "unconfigured" || current["status"] == "draft" {
        let command = json!({"action":"configureDraft", "draft":draft,
            "evidenceRef":"user-requested-seven-day-test-season-not-production-approval"});
        store.admin(
            CAMPAIGN,
            OPERATIONS,
            command,
            "playtest-configure-20261029",
            now,
        )?;
        store.admin(
            CAMPAIGN,
            FINANCE,
            json!({"action":"approveRules",
            "evidenceRef":"automated-test-only-rules-review-not-human-finance-acceptance"}),
            "playtest-approve-20261029",
            now,
        )?;
    } else if current["rules"]
        != serde_json::to_value(&draft.rules).map_err(|_| "rules encoding failed")?
        || current["startsAtMs"].as_u64() != Some(draft.starts_at_ms)
        || current["endsAtMs"].as_u64() != Some(draft.ends_at_ms)
    {
        return Err("existing test season differs; no active configuration was overwritten".into());
    }
    store.admin(
        CAMPAIGN,
        OPERATIONS,
        json!({"action":"setRecruitment","enabled":true,
        "evidenceRef":"user-authorized-public-test-wallet-enrollment"}),
        "playtest-recruitment-20261029",
        now,
    )?;
    store.admin(
        CAMPAIGN,
        FINANCE,
        json!({"action":"setCommission","enabled":true,
        "evidenceRef":"test-only-actual-source-accounting-no-synthetic-revenue"}),
        "playtest-commission-20261029",
        now,
    )?;
    let result = store.public_campaign(CAMPAIGN)?;
    if result["payoutEnabled"] != false || result["recruitmentEnabled"] != true {
        return Err("unexpected test season switch state".into());
    }
    println!(
        "{}",
        json!({"testSeason":result,"noGameDataSeeded":true,
        "noRevenueSeeded":true,"paymentEnabled":false,"humanFinanceApproval":false})
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("promotion playtest setup failed: {error}");
        std::process::exit(1);
    }
}
