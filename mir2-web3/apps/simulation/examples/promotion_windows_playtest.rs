//! Explicit, local-only test-season setup. No payment signer or game data seeding.
use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use mir2_simulation::promotion::{CampaignDraft, PromotionStore};
use mir2_simulation::{apply_migrations, AccountRecord};
use postgres::{Client, Config, NoTls};
use rand_core::{OsRng, RngCore};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const CAMPAIGN: &str = "numeron-windows-playtest-20261030";
const OPERATIONS: &str = "system_promotion_windows_operations_20261030";
const FINANCE: &str = "system_promotion_windows_finance_20261030";

const GRANT_MARKER: &str = "windows-test-automation-not-human-finance-approval";
fn validate_target(url: &str) -> Result<(), String> {
    let parsed: Config = url
        .parse()
        .map_err(|_| "invalid Windows test database configuration")?;
    let name = parsed.get_dbname().unwrap_or_default();
    let acceptance = name
        .strip_prefix("mir2_promotion_windows_acceptance_")
        .is_some_and(|s| {
            !s.is_empty()
                && s.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        });
    if (name != "mir2_playtest" && !acceptance)
        || parsed.get_hostaddrs().iter().any(|ip| !ip.is_loopback())
        || parsed.get_hosts().is_empty()
        || parsed
            .get_hosts()
            .iter()
            .any(|h| !matches!(h,postgres::config::Host::Tcp(n) if n=="127.0.0.1"||n=="localhost"))
    {
        return Err(
            "only the loopback Windows test database or explicit acceptance clone is accepted"
                .into(),
        );
    }
    Ok(())
}
fn existing_accounts(db: &mut Client) -> Result<(usize, String), String> {
    let rows=db.query("SELECT account_id,raw_json FROM accounts WHERE account_id<>$1 AND account_id<>$2 ORDER BY account_id",&[&OPERATIONS,&FINANCE]).map_err(|_|"cannot fingerprint original accounts")?;
    let records: Vec<(String, Value)> = rows.iter().map(|r| (r.get(0), r.get(1))).collect();
    let hash = Sha256::digest(
        serde_json::to_vec(&records).map_err(|_| "cannot encode original accounts")?,
    )
    .iter()
    .map(|b| format!("{b:02x}"))
    .collect();
    Ok((records.len(), hash))
}
fn activate(url: &str, config: Value, now: u64) -> Result<Value, String> {
    validate_target(url)?;
    let draft: CampaignDraft = serde_json::from_value(config["draft"].clone())
        .map_err(|_| "invalid test campaign draft")?;
    if config["environment"] != "test"
        || draft.campaign_id != CAMPAIGN
        || !draft.rules.test_wallet_enrollment
        || draft.funding_mode != "revenue_share"
        || draft.budget_micro_usd != 0
        || draft.rules.reserve_bps != 0
        || draft.ends_at_ms.checked_sub(draft.starts_at_ms) != Some(7 * 86_400_000)
        || draft.starts_at_ms != 1793219738449
        || draft.ends_at_ms != 1793824538449
        || draft.rules.minimum_level != 1
        || draft.rules.active_utc_days != 1
        || draft.member_limit != 0
        || draft.leader_limit != 0
        || draft.per_leader_member_limit != 0
        || draft.per_member_revenue_cap_micro_usd != 0
        || draft.rules.test_usdc_micro_payouts
        || draft.rules.test_first_payout_address_no_cooldown
        || config["gameClient"] != "windows"
        || config["paymentNetwork"] != "sui:testnet"
        || config["paymentCurrency"] != "USDC"
        || config["commissionTiersBps"] != json!([1000, 2000, 3000])
        || config["intendedSwitches"]
            != json!({"recruitmentEnabled":true,"commissionEnabled":true,"payoutEnabled":false})
    {
        return Err(
            "only the reviewed seven-day, non-paying test configuration is accepted".into(),
        );
    }
    if now < draft.starts_at_ms || now >= draft.ends_at_ms {
        return Err("test season must currently be in its configured date range".into());
    }
    let mut db = Client::connect(url, NoTls).map_err(|_| "local test database unavailable")?;
    apply_migrations(&mut db)?;
    let before = existing_accounts(&mut db)?;
    let before_characters: i64 = db
        .query_one("SELECT count(*) FROM characters", &[])
        .map_err(|_| "cannot count original characters")?
        .get(0);
    let mut created = 0u64;
    let mut tx = db.transaction().map_err(|_| "cannot begin test setup")?;
    for (id, role) in [(OPERATIONS, "operations"), (FINANCE, "finance")] {
        if let Some(row) = tx
            .query_opt("SELECT raw_json FROM accounts WHERE account_id=$1", &[&id])
            .map_err(|_| "cannot inspect system actor")?
        {
            let raw: Value = row.get(0);
            let actor: AccountRecord =
                serde_json::from_value(raw).map_err(|_| "existing system actor is invalid")?;
            let known:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM promotion_operator_roles WHERE environment='test' AND account_id=$1 AND role=$2 AND granted_by=$3)",&[&id,&role,&GRANT_MARKER]).map_err(|_|"cannot inspect system role")?.get(0);
            let credential:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM identity_credentials WHERE account_id=$1) OR EXISTS(SELECT 1 FROM identity_sessions WHERE account_id=$1)",&[&id]).map_err(|_|"cannot inspect system credentials")?.get(0);
            if !known
                || credential
                || !actor.characters.is_empty()
                || !actor.saves.is_empty()
                || !actor.password.starts_with("$argon2id$")
                || argon2::PasswordHash::new(&actor.password).is_err()
            {
                return Err("existing system actor conflicts; no account was overwritten".into());
            }
        }
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
        created+=tx.execute("INSERT INTO accounts(account_id,raw_json) VALUES($1,$2) ON CONFLICT(account_id) DO NOTHING", &[&id, &raw])
            .map_err(|_| "cannot provision local test actor")?;
        tx.execute("INSERT INTO promotion_operator_roles(environment,account_id,role,granted_by,granted_at_ms) VALUES('test',$1,$2,'windows-test-automation-not-human-finance-approval',$3) ON CONFLICT DO NOTHING", &[&id, &role, &(now as i64)])
            .map_err(|_| "cannot grant local test role")?;
    }
    tx.commit()
        .map_err(|_| "cannot persist local test actors")?;
    let store = PromotionStore::new(url.to_owned(), "test")?;
    let current = store.public_campaign(CAMPAIGN)?;
    if current["status"] == "unconfigured" || current["status"] == "draft" {
        let command = json!({"action":"configureDraft", "draft":draft.clone(),
            "evidenceRef":"user-requested-seven-day-test-season-not-production-approval"});
        store.admin(
            CAMPAIGN,
            OPERATIONS,
            command,
            "windows-playtest-configure-20261030",
            now,
        )?;
        store.admin(
            CAMPAIGN,
            FINANCE,
            json!({"action":"approveRules",
            "evidenceRef":"automated-test-only-rules-review-not-human-finance-acceptance"}),
            "windows-playtest-approve-20261030",
            now,
        )?;
    } else if current["rules"]
        != serde_json::to_value(&draft.rules).map_err(|_| "rules encoding failed")?
        || current["startsAtMs"].as_u64() != Some(draft.starts_at_ms)
        || current["endsAtMs"].as_u64() != Some(draft.ends_at_ms)
    {
        return Err("existing test season differs; no active configuration was overwritten".into());
    }
    if current["status"] != "unconfigured" && current["status"] != "draft" {
        let row=db.query_one("SELECT configured_by,approved_by FROM promotion_campaigns WHERE environment='test' AND campaign_id=$1",&[&CAMPAIGN]).map_err(|_|"cannot verify existing season ownership")?;
        if row.get::<_, String>(0) != OPERATIONS
            || row.get::<_, Option<String>>(1).as_deref() != Some(FINANCE)
            || current["payoutEnabled"] != false
            || current["fundingMode"] != "revenue_share"
            || current["termsVersion"] != draft.terms_version
        {
            return Err("existing non-draft season was not created by this test helper".into());
        }
        if current["status"] == "active"
            && (current["recruitmentEnabled"] != true || current["commissionEnabled"] != true)
        {
            return Err("existing active season switches differ; no switch was overwritten".into());
        }
        if current["status"] != "active" && current["status"] != "approved" {
            return Err("existing season is closed or locked; no state was overwritten".into());
        }
    }
    if current["status"] != "active" {
        // Exact idempotency keys replay without changing an existing non-draft
        // campaign. A previously disabled switch is detected below, never re-enabled.
        store.admin(
            CAMPAIGN,
            OPERATIONS,
            json!({"action":"setRecruitment","enabled":true,
        "evidenceRef":"user-authorized-public-test-wallet-enrollment"}),
            "windows-playtest-recruitment-20261030",
            now,
        )?;
        store.admin(
            CAMPAIGN,
            FINANCE,
            json!({"action":"setCommission","enabled":true,
        "evidenceRef":"test-only-actual-source-accounting-no-synthetic-revenue"}),
            "windows-playtest-commission-20261030",
            now,
        )?;
    }
    let result = store.public_campaign(CAMPAIGN)?;
    if result["payoutEnabled"] != false
        || result["recruitmentEnabled"] != true
        || result["commissionEnabled"] != true
    {
        return Err("unexpected test season switch state".into());
    }
    let after = existing_accounts(&mut db)?;
    let after_characters: i64 = db
        .query_one("SELECT count(*) FROM characters", &[])
        .map_err(|_| "cannot count original characters")?
        .get(0);
    if before != after || before_characters != after_characters {
        return Err("original account or character data drifted during setup".into());
    }
    Ok(
        json!({"testSeason":result,"gameClient":"windows","originalAccounts":before.0,
        "originalAccountsSha256":before.1,"originalAccountsPreserved":true,"originalCharacters":before_characters,
        "createdSystemActors":created,"systemActorsTotal":2,"systemActorsCanLogin":false,
        "noGameDataSeeded":true,"noRevenueSeeded":true,"payoutEnabled":false,"humanFinanceApproval":false}),
    )
}
fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 || args[0] != "--activate-test-season" {
        return Err(
            "usage: promotion_windows_playtest --activate-test-season <reviewed-test-config.json>"
                .into(),
        );
    }
    if std::env::var("MIR2_PROMOTION_ENVIRONMENT").as_deref() != Ok("test") {
        return Err("explicit test environment required".into());
    }
    let url = std::env::var("MIR2_ACCOUNT_STORE_DATABASE_URL")
        .map_err(|_| "Windows test database environment required")?;
    let bytes = std::fs::read(&args[1]).map_err(|_| "cannot read test configuration")?;
    if bytes.len() > 65536 {
        return Err("test configuration too large".into());
    }
    let config = serde_json::from_slice(&bytes).map_err(|_| "invalid test configuration JSON")?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "clock unavailable")?
        .as_millis() as u64;
    let parsed: Config = url.parse().map_err(|_| "invalid database configuration")?;
    if parsed.get_options().is_some() {
        return Err("database options are not accepted by this operator CLI".into());
    }
    println!("{}", activate(&url, config, now)?);
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Windows promotion playtest setup failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reviewed() -> Value {
        serde_json::from_str(include_str!(
            "../../../config/promotion-windows-playtest-20261030.test.json"
        ))
        .unwrap()
    }
    #[test]
    fn target_guard_rejects_wrong_database_host_and_acceptance_suffix() {
        for url in ["host=127.0.0.1 hostaddr=203.0.113.1 dbname=mir2_promotion_windows_acceptance_123456789abc",
            "postgresql://localhost/postgres",
            "postgresql://localhost/mir2_promotion_old",
            "postgresql://remote.example/mir2_playtest",
            "postgresql://127.0.0.1/mir2_promotion_windows_acceptance_",
            "postgresql://127.0.0.1/mir2_promotion_windows_acceptance_BAD",
        ] {
            assert!(validate_target(url).is_err());
        }
        for url in [
            "postgresql://127.0.0.1/mir2_playtest",
            "postgresql://localhost/mir2_promotion_windows_acceptance_123abc",
        ] {
            validate_target(url).unwrap();
        }
    }
    #[test]
    fn changed_network_campaign_dates_or_payment_switches_fail_before_connection() {
        let now = 1793219738449 + 1000;
        for field in ["campaign", "network", "date", "payout"] {
            let mut v = reviewed();
            match field {
                "campaign" => v["draft"]["campaignId"] = json!("another-season"),
                "network" => v["paymentNetwork"] = json!("sui:mainnet"),
                "date" => v["draft"]["startsAtMs"] = json!(now),
                _ => v["intendedSwitches"]["payoutEnabled"] = json!(true),
            };
            let e = activate("postgresql://127.0.0.1:1/mir2_playtest", v, now).unwrap_err();
            assert!(e.contains("configuration"));
        }
    }
    #[test]
    fn native_pg_setup_is_idempotent_creates_only_two_locked_system_actors_and_preserves_old_rows()
    {
        if std::env::var("MIR2_RUN_PROMOTION_PG_TESTS").as_deref() != Ok("1") {
            eprintln!("SKIPPED: native PG opt-in required");
            return;
        }
        let url = std::env::var("MIR2_PROMOTION_WINDOWS_TEST_DATABASE_URL")
            .expect("dedicated Windows acceptance DB required");
        validate_target(&url).unwrap();
        let parsed: Config = url.parse().unwrap();
        assert!(parsed
            .get_dbname()
            .unwrap()
            .starts_with("mir2_promotion_windows_acceptance_"));
        assert!(parsed.get_options().is_none());
        let schema = format!(
            "windows_setup_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let mut base = Client::connect(&url, NoTls).unwrap();
        base.batch_execute(&format!("CREATE SCHEMA {schema}"))
            .unwrap();
        let scoped = format!(
            "{url}{}options=-csearch_path%3D{schema}",
            if url.contains('?') { "&" } else { "?" }
        );
        let mut db = Client::connect(&scoped, NoTls).unwrap();
        apply_migrations(&mut db).unwrap();
        let raw = serde_json::to_value(AccountRecord::empty()).unwrap();
        db.execute(
            "INSERT INTO accounts(account_id,raw_json)VALUES('original-fixture',$1)",
            &[&raw],
        )
        .unwrap();
        let now = 1793219738449 + 1000;
        let first = activate(&scoped, reviewed(), now).unwrap();
        assert_eq!(first["createdSystemActors"], 2);
        assert_eq!(first["originalAccounts"], 1);
        assert_eq!(first["humanFinanceApproval"], false);
        let all_before: Vec<(String, Value)> = db
            .query(
                "SELECT account_id,raw_json FROM accounts ORDER BY account_id",
                &[],
            )
            .unwrap()
            .iter()
            .map(|r| (r.get(0), r.get(1)))
            .collect();
        let second = activate(&scoped, reviewed(), now + 1).unwrap();
        assert_eq!(second["createdSystemActors"], 0);
        assert_eq!(
            first["originalAccountsSha256"],
            second["originalAccountsSha256"]
        );
        let all_after: Vec<(String, Value)> = db
            .query(
                "SELECT account_id,raw_json FROM accounts ORDER BY account_id",
                &[],
            )
            .unwrap()
            .iter()
            .map(|r| (r.get(0), r.get(1)))
            .collect();
        assert_eq!(all_before, all_after);
        assert_eq!(
            db.query_one("SELECT count(*) FROM identity_credentials", &[])
                .unwrap()
                .get::<_, i64>(0),
            0
        );
        assert_eq!(
            db.query_one("SELECT count(*) FROM characters", &[])
                .unwrap()
                .get::<_, i64>(0),
            0
        );
        db.execute("UPDATE promotion_campaigns SET recruitment_enabled=FALSE WHERE environment='test' AND campaign_id=$1",&[&CAMPAIGN]).unwrap();
        assert!(activate(&scoped, reviewed(), now + 2)
            .unwrap_err()
            .contains("no switch was overwritten"));
        assert!(!db.query_one("SELECT recruitment_enabled FROM promotion_campaigns WHERE environment='test' AND campaign_id=$1",&[&CAMPAIGN]).unwrap().get::<_,bool>(0));
        drop(db);
        assert!(schema.starts_with("windows_setup_"));
        base.batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
            .unwrap();
    }
}
