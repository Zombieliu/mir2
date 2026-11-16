//! Explicit Testnet-only administration for a new short acceptance season.
//! Uses real clocks and fixed locked test actors. Never seeds players, facts or money.
use mir2_simulation::promotion::{CampaignDraft, PromotionStore};
use postgres::{Client, Config, NoTls};
use serde_json::{json, Value};

const OPERATIONS: &str = "system_promotion_playtest_operations_20261029";
const FINANCE: &str = "system_promotion_playtest_finance_20261029";

fn run() -> Result<Value, String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 3 || !matches!(args[0].as_str(), "view" | "command") {
        return Err("usage: promotion_chain_acceptance <view|command> <campaign-id> <operations|finance> [command.json]".into());
    }
    let campaign = &args[1];
    if !campaign.starts_with("numeron-chain-acceptance-")
        || campaign.len() > 100
        || !campaign
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err("only a separate chain acceptance season is allowed".into());
    }
    let actor = match args[2].as_str() {
        "operations" => OPERATIONS,
        "finance" => FINANCE,
        _ => return Err("only existing automated Testnet operator roles are allowed".into()),
    };
    if std::env::var("MIR2_PROMOTION_ENVIRONMENT").as_deref() != Ok("test") {
        return Err("explicit test environment required".into());
    }
    let database =
        std::env::var("MIR2_ACCOUNT_STORE_DATABASE_URL").map_err(|_| "test database required")?;
    let parsed: Config = database
        .parse()
        .map_err(|_| "invalid database configuration")?;
    if !parsed.get_dbname().is_some_and(|name| name.contains("promotion")) || parsed.get_hosts().is_empty() ||
        parsed.get_hosts().iter().any(|h| !matches!(h, postgres::config::Host::Tcp(host) if host == "127.0.0.1" || host == "localhost")) {
        return Err("only a loopback promotion database is allowed".into());
    }
    let store = PromotionStore::new(database.clone(), "test")?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "clock unavailable")?
        .as_millis() as u64;
    if args[0] == "view" {
        if args.len() != 3 {
            return Err("view takes exactly three arguments".into());
        }
        return store.admin_view(campaign, actor, now);
    }
    if args.len() != 4 {
        return Err("command requires one JSON file".into());
    }
    let bytes = std::fs::read(&args[3]).map_err(|_| "cannot read command")?;
    if bytes.len() > 65536 {
        return Err("command too large".into());
    }
    let mut envelope: Value = serde_json::from_slice(&bytes).map_err(|_| "invalid command JSON")?;
    let key = envelope["idempotencyKey"]
        .as_str()
        .ok_or("idempotency key required")?
        .to_owned();
    if key.len() < 16
        || key.len() > 128
        || !key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
    {
        return Err("invalid idempotency key".into());
    }
    let command = envelope
        .get_mut("command")
        .and_then(Value::as_object_mut)
        .ok_or("command object required")?;
    let action = command
        .get("action")
        .and_then(Value::as_str)
        .ok_or("action required")?
        .to_owned();
    if !matches!(
        action.as_str(),
        "configureDraft"
            | "approveRules"
            | "setRecruitment"
            | "setCommission"
            | "proposeLaunch"
            | "approveLaunch"
            | "setPayout"
            | "reconcile"
            | "setInputWatermark"
            | "createSettlement"
            | "approveSettlement"
            | "createPayoutDraft"
            | "freezePayoutIntent"
            | "approvePayout"
    ) {
        return Err("action is outside short Testnet acceptance scope".into());
    }
    if action == "configureDraft" {
        let draft: CampaignDraft =
            serde_json::from_value(command.get("draft").cloned().ok_or("draft required")?)
                .map_err(|_| "invalid campaign draft")?;
        let duration = draft
            .ends_at_ms
            .checked_sub(draft.starts_at_ms)
            .ok_or("invalid season range")?;
        if draft.campaign_id != *campaign
            || !(300_000..=1_800_000).contains(&duration)
            || draft.starts_at_ms.abs_diff(now) > 120_000
            || draft.ends_at_ms <= now
            || draft.funding_mode != "revenue_share"
            || draft.budget_micro_usd != 0
            || !draft.rules.test_wallet_enrollment
            || !draft.rules.test_usdc_micro_payouts
            || !draft.rules.test_first_payout_address_no_cooldown
            || draft.rules.minimum_level != 1
            || draft.rules.active_utc_days != 1
            || draft.rules.reserve_bps != 0
        {
            return Err(
                "only a new real-time, zero-retention, micro-USDC test season is allowed".into(),
            );
        }
    }
    let evidence = command
        .get("evidenceRef")
        .and_then(Value::as_str)
        .ok_or("explicit test evidence reference required")?;
    if !evidence.starts_with("testnet-chain-acceptance:")
        || evidence.contains("human-finance-approved")
    {
        return Err("evidence must identify automated Testnet acceptance".into());
    }
    if action == "configureDraft" {
        let mut db =
            Client::connect(&database, NoTls).map_err(|_| "test role storage unavailable")?;
        let mut tx = db
            .transaction()
            .map_err(|_| "test role transaction unavailable")?;
        for (id, roles) in [
            (OPERATIONS, &["operations", "finance", "payoutSubmit"][..]),
            (FINANCE, &["finance", "payoutReview"][..]),
        ] {
            let exists: bool = tx
                .query_one(
                    "SELECT EXISTS(SELECT 1 FROM accounts WHERE account_id=$1 AND NOT is_banned)",
                    &[&id],
                )
                .map_err(|_| "cannot verify fixed test operator")?
                .get(0);
            if !exists {
                return Err("existing locked test operator is required".into());
            }
            for role in roles {
                let inserted = tx.execute("INSERT INTO promotion_operator_roles(environment,account_id,role,granted_by,granted_at_ms) VALUES('test',$1,$2,'user-authorized-testnet-chain-acceptance-not-human-finance',$3) ON CONFLICT DO NOTHING", &[&id, role, &(now as i64)])
                    .map_err(|_| "cannot grant fixed test role")?;
                if inserted > 0 {
                    tx.execute("INSERT INTO promotion_audit_log(environment,actor_id,action,resource_id,evidence_ref,summary_json,occurred_at_ms) VALUES('test','testnet-chain-acceptance-helper','operator.test_scope_grant',$1,$2,$3,$4)",
                        &[&format!("{id}:{role}"), &evidence, &json!({"campaignId":campaign,"humanFinanceApproval":false}), &(now as i64)])
                        .map_err(|_| "cannot audit fixed test role")?;
                }
            }
        }
        tx.commit().map_err(|_| "cannot persist fixed test roles")?;
    }
    store.admin(campaign, actor, Value::Object(command.clone()), &key, now)
}

fn main() {
    match run() {
        Ok(result) => println!(
            "{}",
            json!({"environment":"test","humanFinanceApproval":false,"businessFactsSeeded":false,"result":result})
        ),
        Err(error) => {
            eprintln!("short Testnet acceptance rejected: {error}");
            std::process::exit(1);
        }
    }
}
