//! A reviewed, test-only invitation carry-over. No account or money is imported.
//! Source truth is read directly from a repeatable-read, read-only connection;
//! neither the browser nor a manifest supplies authoritative relationship data.
use std::time::Duration;

use postgres::{Client, GenericClient, IsolationLevel, NoTls, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::store::{
    account, active, admission_capacity, audit, campaign, db, hash, positive,
    refresh_account_qualifications, rules, signed, terms,
};
use super::PromotionStore;
use crate::{
    billing::BillingLedger, procurement_fees::CreditOrigin, with_account_store_postgres_client,
};

pub const SOURCE_CAMPAIGN_ID: &str = "numeron-playtest-20261029";
pub const TARGET_CAMPAIGN_ID: &str = "numeron-windows-playtest-20261030";
const POLICY: &str = "numeron-legacy-windows-invitation-test-v1";
const TTL_MS: u64 = 30 * 60 * 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionMigrationScope {
    pub member_wallet: String,
    pub leader_wallet: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PaymentPin {
    order_id: String,
    payment_identity_sha256: String,
    paid_at_ms: u64,
}
struct Payment {
    pin: PaymentPin,
    kind: &'static str,
    identity: String,
}
struct Owner {
    id: String,
    credential_sha256: String,
    registered_at_ms: u64,
    raw: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Evidence {
    policy: String,
    source_database_sha256: String,
    target_database_sha256: String,
    source_relation_key: String,
    source_account_sha256: String,
    source_leader_account_sha256: String,
    source_member_credential_sha256: String,
    source_leader_credential_sha256: String,
    source_confirmed_at_ms: u64,
    source_terms_version: String,
    source_attribution_sha256: String,
    source_candidate_sha256: String,
    source_audit_sha256: String,
    source_leader_sha256: String,
    source_account_raw_sha256: String,
    source_campaign_sha256: String,
    member_wallet: String,
    leader_wallet: String,
    target_account_sha256: String,
    target_leader_account_sha256: String,
    target_member_credential_sha256: String,
    target_leader_credential_sha256: String,
    target_account_raw_sha256: String,
    target_leader_raw_sha256: String,
    target_terms_version: String,
    target_campaign_sha256: String,
    target_leader_id: String,
    target_leader_display_name: String,
    eligibility_anchor_ms: u64,
    payments: Vec<PaymentPin>,
}
struct Capture {
    evidence: Evidence,
    member: Owner,
    leader: Owner,
}

fn invalid() -> String {
    "promotionMigrationProofInvalid".into()
}
fn sha_text(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect()
}
fn canonical_wallet(value: &str) -> Result<String, String> {
    let value = value.to_ascii_lowercase();
    if value.len() != 66
        || !value.starts_with("0x")
        || !value[2..].bytes().all(|b| b.is_ascii_hexdigit())
        || !value[2..].bytes().any(|b| b != b'0')
    {
        return Err("promotionMigrationIdentityConflict".into());
    }
    Ok(value)
}
fn policy(env: &str, scope: &PromotionMigrationScope) -> Result<(String, String), String> {
    if env != "test" {
        return Err("promotionMigrationPolicyDenied".into());
    }
    let member = canonical_wallet(&scope.member_wallet)?;
    let leader = canonical_wallet(&scope.leader_wallet)?;
    if member == leader {
        return Err("promotionSelfReferral".into());
    }
    Ok((member, leader))
}
fn source_connection(url: &str) -> Result<Client, String> {
    let mut config: postgres::Config = url.parse().map_err(|_| invalid())?;
    config.connect_timeout(Duration::from_secs(3));
    config
        .connect(NoTls)
        .map_err(|_| "promotionStorageUnavailable".into())
}
fn database_fingerprint<T: GenericClient>(client: &mut T) -> Result<String, String> {
    let row = db(client.query_one("SELECT current_database(),current_schema(),COALESCE(inet_server_addr()::text,'local'),COALESCE(inet_server_port(),0)", &[]))?;
    hash(&json!([
        row.get::<_, String>(0),
        row.get::<_, String>(1),
        row.get::<_, String>(2),
        row.get::<_, i32>(3)
    ]))
}
fn namespace<T: GenericClient>(client: &mut T) -> Result<(String, String), String> {
    let row = db(client.query_one("SELECT current_database(),current_schema()", &[]))?;
    Ok((row.get(0), row.get(1)))
}
fn owner<T: GenericClient>(client: &mut T, wallet: &str, lock: bool) -> Result<Owner, String> {
    let sql = if lock {
        "SELECT credential_id,account_id,created_at_ms FROM identity_credentials WHERE credential_kind='sui_wallet' AND lower(credential_subject)=$1 AND revoked_at_ms IS NULL FOR SHARE"
    } else {
        "SELECT credential_id,account_id,created_at_ms FROM identity_credentials WHERE credential_kind='sui_wallet' AND lower(credential_subject)=$1 AND revoked_at_ms IS NULL"
    };
    let rows = db(client.query(sql, &[&wallet]))?;
    if rows.len() != 1 {
        return Err("promotionMigrationIdentityConflict".into());
    }
    let row = &rows[0];
    let id: String = row.get(1);
    let acc = account(client, &id, lock)?;
    Ok(Owner {
        credential_sha256: hash(&json!([
            row.get::<_, String>(0),
            &id,
            wallet,
            row.get::<_, i64>(2)
        ]))?,
        id,
        raw: acc.get(0),
        registered_at_ms: positive(acc.get(1)),
    })
}
fn payments(raw: &Value) -> Result<Vec<Payment>, String> {
    let Some(value) = raw.get("billing").filter(|v| !v.is_null()) else {
        return Ok(vec![]);
    };
    let ledger: BillingLedger = serde_json::from_value(value.clone()).map_err(|_| invalid())?;
    ledger
        .validate()
        .map_err(|_| "promotionMigrationPaymentUnverified".to_owned())?;
    let mut result = Vec::new();
    for order in ledger.orders.values() {
        if let Some(at) = order.paid_at_ms {
            if order.livemode {
                return Err("promotionMigrationPolicyDenied".into());
            }
            let id = order.payment_intent_id.clone().ok_or_else(invalid)?;
            result.push(Payment {
                pin: PaymentPin {
                    order_id: order.id.clone(),
                    paid_at_ms: at,
                    payment_identity_sha256: hash(&json!(["stripe", false, &id]))?,
                },
                kind: "stripe",
                identity: id,
            });
        }
    }
    for order in ledger
        .sui_usdc_orders
        .values()
        .filter(|o| o.has_payment_receipt())
    {
        if order.credit_origin != CreditOrigin::Test {
            return Err("promotionMigrationPolicyDenied".into());
        }
        let digest = order.payment_digest().ok_or_else(invalid)?.to_owned();
        let at = order.payment_confirmed_at_ms().ok_or_else(invalid)?;
        result.push(Payment {
            pin: PaymentPin {
                order_id: order.id.clone(),
                paid_at_ms: at,
                payment_identity_sha256: hash(&json!([
                    "sui",
                    &order.domain.chain_identifier,
                    &digest
                ]))?,
            },
            kind: "sui",
            identity: digest,
        });
    }
    result.sort_by(|a, b| a.pin.order_id.cmp(&b.pin.order_id));
    if result.len() > 64 || result.iter().any(|p| p.pin.paid_at_ms == 0) {
        return Err("promotionMigrationPaymentUnverified".into());
    }
    Ok(result)
}
fn source_payment_exists<T: GenericClient>(
    source: &mut T,
    payment: &Payment,
) -> Result<bool, String> {
    // Check canonical provider identities across the source realm, not just
    // realm-local monthly UID numbers. No source rows are copied or changed.
    let sql = if payment.kind == "stripe" {
        "SELECT EXISTS(SELECT 1 FROM accounts a CROSS JOIN LATERAL jsonb_each(COALESCE(a.raw_json#>'{billing,orders}','{}'::jsonb)) o WHERE o.value->>'paymentIntentId'=$1)"
    } else {
        "SELECT EXISTS(SELECT 1 FROM accounts a CROSS JOIN LATERAL jsonb_each(COALESCE(a.raw_json#>'{billing,suiUsdcOrders}','{}'::jsonb)) o WHERE o.value#>>'{receipt,digest}'=$1 OR o.value#>>'{transferReceipt,digest}'=$1)"
    };
    Ok(db(source.query_one(sql, &[&payment.identity]))?.get(0))
}
fn campaign_basis(row: &postgres::Row) -> Value {
    json!({"terms":row.get::<_,String>(0),"status":row.get::<_,String>(1),
        "start":row.get::<_,i64>(2),"end":row.get::<_,i64>(3),"rules":row.get::<_,Value>(4),
        "approved":row.get::<_,bool>(5),"recruitment":row.get::<_,bool>(6)})
}
fn capture(
    source: &mut Transaction<'_>,
    target: &mut Transaction<'_>,
    env: &str,
    scope: &PromotionMigrationScope,
    now: u64,
    lock: bool,
) -> Result<Capture, String> {
    let (member_wallet, leader_wallet) = policy(env, scope)?;
    let source_db = database_fingerprint(source)?;
    let target_db = database_fingerprint(target)?;
    if source_db == target_db || namespace(source)? == namespace(target)? {
        return Err("promotionMigrationPolicyDenied".into());
    }
    let sc = campaign(source, env, SOURCE_CAMPAIGN_ID, false)?;
    let tc = campaign(target, env, TARGET_CAMPAIGN_ID, lock)?;
    if !active(&sc, now)
        || !active(&tc, now)
        || !sc.get::<_, bool>(5)
        || !tc.get::<_, bool>(5)
        || !tc.get::<_, bool>(6)
        || sc.get::<_, i64>(2) != tc.get::<_, i64>(2)
        || sc.get::<_, i64>(3) != tc.get::<_, i64>(3)
    {
        return Err("promotionMigrationPolicyDenied".into());
    }
    let sm = owner(source, &member_wallet, false)?;
    let sl = owner(source, &leader_wallet, false)?;
    let member = owner(target, &member_wallet, lock)?;
    let leader = owner(target, &leader_wallet, lock)?;
    if member.id == leader.id || sm.id == sl.id {
        return Err("promotionSelfReferral".into());
    }
    let sr = rules(&sc)?;
    let tr = rules(&tc)?;
    if member.registered_at_ms < positive(tc.get(2))
        || member.registered_at_ms >= positive(tc.get(3))
    {
        return Err("promotionExistingAccountExcluded".into());
    }
    if now < member.registered_at_ms
        || now
            > member
                .registered_at_ms
                .saturating_add(tr.confirmation_window_ms)
    {
        return Err("promotionConfirmationExpired".into());
    }
    let attr = db(source.query_opt("SELECT to_jsonb(a),leader_id,candidate_id,confirmed_at_ms,terms_version,status FROM promotion_attributions a WHERE environment=$1 AND campaign_id=$2 AND account_id=$3",&[&env,&SOURCE_CAMPAIGN_ID,&sm.id]))?.ok_or_else(invalid)?;
    let source_confirmed = positive(attr.get(3));
    if !matches!(attr.get::<_, String>(5).as_str(), "bound" | "qualified")
        || attr.get::<_, String>(4) != sc.get::<_, String>(0)
        || source_confirmed > now
        || source_confirmed < sm.registered_at_ms
        || source_confirmed
            > sm.registered_at_ms
                .saturating_add(sr.confirmation_window_ms)
        || sm.registered_at_ms < positive(sc.get(2))
        || sm.registered_at_ms >= positive(sc.get(3))
    {
        return Err("promotionMigrationSourceChanged".into());
    }
    let source_leader_id: String = attr.get(1);
    let source_candidate_id: String = attr.get(2);
    let original_leader = db(source.query_opt("SELECT to_jsonb(l),status,accepted_terms_version,applied_at_ms FROM promotion_leaders l WHERE environment=$1 AND campaign_id=$2 AND leader_id=$3 AND account_id=$4",&[&env,&SOURCE_CAMPAIGN_ID,&source_leader_id,&sl.id]))?.ok_or_else(invalid)?;
    if original_leader.get::<_, String>(1) != "approved"
        || original_leader.get::<_, String>(2) != sc.get::<_, String>(0)
        || positive(original_leader.get(3)) > source_confirmed
    {
        return Err("promotionMigrationSourceChanged".into());
    }
    let candidate = db(source.query_opt("SELECT to_jsonb(c),source,first_clicked_at_ms,expires_at_ms FROM promotion_candidates c WHERE environment=$1 AND campaign_id=$2 AND candidate_id=$3 AND account_id=$4 AND leader_id=$5",&[&env,&SOURCE_CAMPAIGN_ID,&source_candidate_id,&sm.id,&source_leader_id]))?.ok_or_else(invalid)?;
    let attribution_json: Value = attr.get(0);
    let clicked = positive(candidate.get(2));
    if !matches!(
        candidate.get::<_, String>(1).as_str(),
        "invite_link" | "manual_code"
    ) || clicked > source_confirmed
        || positive(candidate.get(3)) <= source_confirmed
        || attribution_json["source"].as_str() != Some(candidate.get::<_, String>(1).as_str())
        || attribution_json["first_clicked_at_ms"].as_u64() != Some(clicked)
        || attribution_json["registered_at_ms"].as_u64() != Some(sm.registered_at_ms)
        || clicked
            > sm.registered_at_ms
                .saturating_add(sr.confirmation_window_ms)
        || sm.registered_at_ms > clicked.saturating_add(sr.candidate_ttl_ms)
    {
        return Err(invalid());
    }
    let audits = db(source.query("SELECT to_jsonb(a) FROM promotion_audit_log a WHERE environment=$1 AND actor_id=$2 AND action='attribution.confirmed' AND resource_id=$3 AND evidence_ref=$4 AND occurred_at_ms=$5 ORDER BY audit_id",&[&env,&sm.id,&format!("{SOURCE_CAMPAIGN_ID}:{}",sm.id),&source_candidate_id,&signed(source_confirmed)?]))?;
    if audits.len() != 1 {
        return Err(invalid());
    }
    let audit_json: Value = audits[0].get(0);
    if audit_json["summary_json"]["leaderId"].as_str() != Some(source_leader_id.as_str())
        || audit_json["summary_json"]["termsVersion"].as_str()
            != Some(sc.get::<_, String>(0).as_str())
    {
        return Err(invalid());
    }
    if payments(&sm.raw)?
        .iter()
        .any(|p| p.pin.paid_at_ms < source_confirmed || p.pin.paid_at_ms > now)
    {
        return Err("promotionMigrationPaymentPrecedesSource".into());
    }
    let old_allocations: i64 = db(source.query_one("SELECT count(*) FROM promotion_revenue_allocations WHERE environment=$1 AND campaign_id=$2 AND account_id=$3",&[&env,&SOURCE_CAMPAIGN_ID,&sm.id]))?.get(0);
    if old_allocations != 0 {
        return Err("promotionMigrationConflict".into());
    }
    let target_leader = db(target.query_opt("SELECT leader_id,display_name,status,accepted_terms_version FROM promotion_leaders WHERE environment=$1 AND campaign_id=$2 AND account_id=$3",&[&env,&TARGET_CAMPAIGN_ID,&leader.id]))?.ok_or("promotionMigrationLeaderConsentRequired")?;
    if target_leader.get::<_, String>(2) != "approved"
        || target_leader.get::<_, String>(3) != tc.get::<_, String>(0)
    {
        return Err("promotionMigrationLeaderConsentRequired".into());
    }
    if db(target.query_one("SELECT EXISTS(SELECT 1 FROM promotion_attributions WHERE environment=$1 AND campaign_id=$2 AND account_id=$3)",&[&env,&TARGET_CAMPAIGN_ID,&member.id]))?.get::<_,bool>(0) {
        return Err("promotionMigrationConflict".into());
    }
    let anchor = source_confirmed
        .max(member.registered_at_ms)
        .max(leader.registered_at_ms)
        .max(positive(tc.get(2)));
    let observed_payments = payments(&member.raw)?;
    for payment in &observed_payments {
        if payment.pin.paid_at_ms < anchor || payment.pin.paid_at_ms > now {
            return Err("promotionMigrationPaymentPrecedesSource".into());
        }
        if source_payment_exists(source, payment)? {
            return Err("promotionMigrationConflict".into());
        }
    }
    let evidence = Evidence {
        policy: POLICY.into(),
        source_relation_key: hash(&json!([
            POLICY,
            &source_db,
            SOURCE_CAMPAIGN_ID,
            sha_text(&sm.id),
            sha_text(&sl.id)
        ]))?,
        source_database_sha256: source_db,
        target_database_sha256: target_db,
        source_account_sha256: sha_text(&sm.id),
        source_leader_account_sha256: sha_text(&sl.id),
        source_member_credential_sha256: sm.credential_sha256,
        source_leader_credential_sha256: sl.credential_sha256,
        source_confirmed_at_ms: source_confirmed,
        source_terms_version: sc.get(0),
        source_attribution_sha256: hash(&attr.get::<_, Value>(0))?,
        source_candidate_sha256: hash(&candidate.get::<_, Value>(0))?,
        source_audit_sha256: hash(&audits[0].get::<_, Value>(0))?,
        source_leader_sha256: hash(&original_leader.get::<_, Value>(0))?,
        source_account_raw_sha256: hash(&sm.raw)?,
        source_campaign_sha256: hash(&campaign_basis(&sc))?,
        member_wallet,
        leader_wallet,
        target_account_sha256: sha_text(&member.id),
        target_leader_account_sha256: sha_text(&leader.id),
        target_member_credential_sha256: member.credential_sha256.clone(),
        target_leader_credential_sha256: leader.credential_sha256.clone(),
        target_account_raw_sha256: hash(&member.raw)?,
        target_leader_raw_sha256: hash(&leader.raw)?,
        target_terms_version: tc.get(0),
        target_campaign_sha256: hash(&campaign_basis(&tc))?,
        target_leader_id: target_leader.get(0),
        target_leader_display_name: target_leader.get(1),
        eligibility_anchor_ms: anchor,
        payments: observed_payments.into_iter().map(|p| p.pin).collect(),
    };
    Ok(Capture {
        evidence,
        member,
        leader,
    })
}

fn verified_evidence(row: &Value) -> Result<Evidence, String> {
    let e: Evidence =
        serde_json::from_value(row["source_evidence_json"].clone()).map_err(|_| invalid())?;
    let digest = hash(&e)?;
    let valid = e.policy == POLICY
        && e.source_database_sha256 != e.target_database_sha256
        && row["environment"] == "test"
        && row["source_campaign_id"] == SOURCE_CAMPAIGN_ID
        && row["target_campaign_id"] == TARGET_CAMPAIGN_ID
        && row["source_evidence_sha256"] == digest
        && row["snapshot_sha256"] == digest
        && row["source_relation_key"] == e.source_relation_key
        && row["source_database_sha256"] == e.source_database_sha256
        && row["source_account_sha256"] == e.source_account_sha256
        && row["source_leader_account_sha256"] == e.source_leader_account_sha256
        && row["source_confirmed_at_ms"].as_u64() == Some(e.source_confirmed_at_ms)
        && row["source_terms_version"] == e.source_terms_version
        && row["target_terms_version"] == e.target_terms_version
        && row["eligibility_anchor_ms"].as_u64() == Some(e.eligibility_anchor_ms)
        && row["leader_id"] == e.target_leader_id
        && row["account_id"].as_str().map(sha_text).as_deref()
            == Some(e.target_account_sha256.as_str())
        && row["leader_account_id"].as_str().map(sha_text).as_deref()
            == Some(e.target_leader_account_sha256.as_str())
        && e.source_confirmed_at_ms > 0
        && e.eligibility_anchor_ms >= e.source_confirmed_at_ms
        && row["prepared_at_ms"]
            .as_u64()
            .is_some_and(|at| at >= e.eligibility_anchor_ms)
        && row["expires_at_ms"].as_u64() > row["prepared_at_ms"].as_u64();
    if !valid {
        return Err(invalid());
    }
    Ok(e)
}

/// Called only inside the normal authenticated/idempotent confirmation
/// transaction. An ordinary invitation returns None and keeps its old guards.
pub(super) fn try_confirm(
    tx: &mut Transaction<'_>,
    env: &str,
    campaign_id: &str,
    account_id: &str,
    candidate_id: &str,
    terms_version: &str,
    now: u64,
) -> Result<Option<Value>, String> {
    let kind=db(tx.query_opt("SELECT source FROM promotion_candidates WHERE environment=$1 AND campaign_id=$2 AND account_id=$3 AND candidate_id=$4",&[&env,&campaign_id,&account_id,&candidate_id]))?;
    if kind.as_ref().map(|r| r.get::<_, String>(0)).as_deref() != Some("migration_import") {
        return Ok(None);
    }
    if env != "test" || campaign_id != TARGET_CAMPAIGN_ID {
        return Err("promotionMigrationPolicyDenied".into());
    }
    let camp = campaign(tx, env, campaign_id, true)?;
    terms(&camp, terms_version)?;
    if !active(&camp, now) || !camp.get::<_, bool>(5) || !camp.get::<_, bool>(6) {
        return Err("promotionRecruitmentClosed".into());
    }
    let stored:Value=db(tx.query_opt("SELECT to_jsonb(e) FROM promotion_attribution_migration_evidence e WHERE environment=$1 AND target_campaign_id=$2 AND account_id=$3 AND candidate_id=$4",&[&env,&campaign_id,&account_id,&candidate_id]))?.ok_or_else(invalid)?.get(0);
    let e = verified_evidence(&stored)?;
    let migration_id = stored["migration_id"].as_str().ok_or_else(invalid)?;
    let prepared = stored["prepared_at_ms"].as_u64().ok_or_else(invalid)?;
    let expires = stored["expires_at_ms"].as_u64().ok_or_else(invalid)?;
    if now < prepared || now >= expires {
        return Err("promotionMigrationExpired".into());
    }
    if terms_version != e.target_terms_version {
        return Err("promotionMigrationTermsRequired".into());
    }
    if database_fingerprint(tx)? != e.target_database_sha256 {
        return Err(invalid());
    }
    let member = owner(tx, &e.member_wallet, true)?;
    let leader = owner(tx, &e.leader_wallet, true)?;
    if member.id != account_id
        || member.credential_sha256 != e.target_member_credential_sha256
        || leader.credential_sha256 != e.target_leader_credential_sha256
        || sha_text(&leader.id) != e.target_leader_account_sha256
    {
        return Err("promotionMigrationIdentityConflict".into());
    }
    let r = rules(&camp)?;
    if member.registered_at_ms < positive(camp.get(2))
        || member.registered_at_ms >= positive(camp.get(3))
    {
        return Err("promotionExistingAccountExcluded".into());
    }
    if now < member.registered_at_ms
        || now
            > member
                .registered_at_ms
                .saturating_add(r.confirmation_window_ms)
    {
        return Err("promotionConfirmationExpired".into());
    }
    let approved:bool=db(tx.query_one("SELECT EXISTS(SELECT 1 FROM promotion_leaders WHERE environment=$1 AND campaign_id=$2 AND account_id=$3 AND leader_id=$4 AND status='approved' AND accepted_terms_version=$5)",&[&env,&campaign_id,&leader.id,&e.target_leader_id,&terms_version]))?.get(0);
    if !approved {
        return Err("promotionMigrationLeaderConsentRequired".into());
    }
    let current: Vec<_> = payments(&member.raw)?.into_iter().map(|p| p.pin).collect();
    if current != e.payments
        || current
            .iter()
            .any(|p| p.paid_at_ms < e.eligibility_anchor_ms || p.paid_at_ms > now)
    {
        return Err("promotionMigrationPaymentUnverified".into());
    }
    let conflict:bool=db(tx.query_one("SELECT EXISTS(SELECT 1 FROM promotion_attributions WHERE environment=$1 AND campaign_id=$2 AND account_id=$3) OR EXISTS(SELECT 1 FROM promotion_attribution_migration_confirmations WHERE environment=$1 AND source_relation_key=$4)",&[&env,&campaign_id,&account_id,&e.source_relation_key]))?.get(0);
    if conflict {
        return Err("promotionMigrationConflict".into());
    }
    let capacity = admission_capacity(tx, env, campaign_id, &e.target_leader_id, &camp, now)?;
    if !capacity.available {
        return Err("promotionBudgetFull".into());
    }
    // prepared is the real creation of this explicit migration candidate,
    // never a fabricated click in the target activity. Original clicks are
    // retained exclusively in the immutable source evidence hash.
    db(tx.execute("INSERT INTO promotion_attributions(environment,campaign_id,account_id,leader_id,candidate_id,first_clicked_at_ms,registered_at_ms,confirmed_at_ms,terms_version,source,reserved_micro_usd) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,'migration_import',$10)",&[&env,&campaign_id,&account_id,&e.target_leader_id,&candidate_id,&signed(prepared)?,&signed(member.registered_at_ms)?,&signed(now)?,&terms_version,&signed(capacity.reserve_micro_usd)?]))?;
    db(tx.execute("INSERT INTO promotion_attribution_migration_confirmations(environment,migration_id,source_relation_key,target_campaign_id,account_id,candidate_id,accepted_terms_version,confirmed_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8)",&[&env,&migration_id,&e.source_relation_key,&campaign_id,&account_id,&candidate_id,&terms_version,&signed(now)?]))?;
    for p in &e.payments {
        db(tx.execute("INSERT INTO promotion_attribution_migration_payment_claims(environment,migration_id,account_id,order_id,payment_identity_sha256,paid_at_ms) VALUES($1,$2,$3,$4,$5,$6)",&[&env,&migration_id,&account_id,&p.order_id,&p.payment_identity_sha256,&signed(p.paid_at_ms)?]))?;
    }
    db(tx.execute("UPDATE promotion_campaigns SET reserved_micro_usd=$3 WHERE environment=$1 AND campaign_id=$2",&[&env,&campaign_id,&signed(capacity.total_reserved_micro_usd)?]))?;
    refresh_account_qualifications(tx, env, account_id, now)?;
    audit(
        tx,
        env,
        account_id,
        "migration.confirmed",
        migration_id,
        "member_current_terms_confirmation",
        json!({"campaignId":campaign_id,"candidateId":candidate_id,"termsVersion":terms_version,
            "confirmedAtMs":now,"sourceConfirmedAtMs":e.source_confirmed_at_ms,
            "eligibilityAnchorMs":e.eligibility_anchor_ms,"snapshotSha256":stored["snapshot_sha256"]}),
        now,
    )?;
    Ok(Some(
        json!({"campaignId":campaign_id,"status":"bound","migrationId":migration_id,
        "confirmedAtMs":now,"sourceConfirmedAtMs":e.source_confirmed_at_ms,
        "reservedMicroUsd":capacity.reserve_micro_usd.to_string()}),
    ))
}

/// The exception covers only exact pre-confirmation provider receipts which
/// were independently checked absent in the source realm during preparation.
/// New/unlisted historical receipts retain the ordinary early-payment hold.
pub(super) fn permits_preconfirmation_payment<T: GenericClient>(
    client: &mut T,
    env: &str,
    campaign_id: &str,
    account_id: &str,
    order_id: &str,
    paid_at: u64,
    confirmed_at: u64,
) -> Result<bool, String> {
    if env != "test" || campaign_id != TARGET_CAMPAIGN_ID {
        return Ok(false);
    }
    let row=db(client.query_opt("SELECT to_jsonb(e),p.payment_identity_sha256,p.paid_at_ms,c.confirmed_at_ms,a.confirmed_at_ms,a.source,a.terms_version FROM promotion_attribution_migration_payment_claims p JOIN promotion_attribution_migration_evidence e USING(environment,migration_id) JOIN promotion_attribution_migration_confirmations c USING(environment,migration_id) JOIN promotion_attributions a ON a.environment=e.environment AND a.campaign_id=e.target_campaign_id AND a.account_id=e.account_id AND a.candidate_id=e.candidate_id WHERE p.environment=$1 AND e.target_campaign_id=$2 AND p.account_id=$3 AND p.order_id=$4",&[&env,&campaign_id,&account_id,&order_id]))?;
    let Some(row) = row else {
        return Ok(false);
    };
    let Ok(e) = verified_evidence(&row.get::<_, Value>(0)) else {
        return Ok(false);
    };
    let matches = row.get::<_, String>(5) == "migration_import"
        && row.get::<_, String>(6) == e.target_terms_version
        && positive(row.get(3)) == confirmed_at
        && positive(row.get(4)) == confirmed_at
        && positive(row.get(2)) == paid_at
        && paid_at >= e.eligibility_anchor_ms
        && e.payments.iter().any(|p| {
            p.order_id == order_id
                && p.paid_at_ms == paid_at
                && p.payment_identity_sha256 == row.get::<_, String>(1)
        });
    Ok(matches)
}

fn view(stored: &Value, e: &Evidence, confirmed: Option<u64>) -> Result<Value, String> {
    let mut v = json!({"migrationId":stored["migration_id"],"candidateId":stored["candidate_id"],
        "leaderDisplayName":e.target_leader_display_name,"sourceCampaignId":SOURCE_CAMPAIGN_ID,
        "sourceConfirmedAtMs":e.source_confirmed_at_ms,"eligibilityAnchorMs":e.eligibility_anchor_ms,
        "preparedAtMs":stored["prepared_at_ms"],"expiresAtMs":stored["expires_at_ms"],
        "targetTermsVersion":e.target_terms_version,"snapshotSha256":stored["snapshot_sha256"]});
    if let Some(at) = confirmed {
        v["confirmedAtMs"] = json!(at);
    }
    Ok(v)
}

pub(super) fn account_view<T: GenericClient>(
    client: &mut T,
    env: &str,
    campaign_id: &str,
    account_id: &str,
    now: u64,
) -> Result<(Option<Value>, Option<Value>), String> {
    if env != "test" || campaign_id != TARGET_CAMPAIGN_ID {
        return Ok((None, None));
    }
    if let Some(row)=db(client.query_opt("SELECT to_jsonb(e),c.confirmed_at_ms FROM promotion_attribution_migration_evidence e JOIN promotion_attribution_migration_confirmations c USING(environment,migration_id) JOIN promotion_attributions a ON a.environment=e.environment AND a.campaign_id=e.target_campaign_id AND a.account_id=e.account_id AND a.candidate_id=e.candidate_id WHERE e.environment=$1 AND e.target_campaign_id=$2 AND e.account_id=$3 AND a.source='migration_import' AND c.confirmed_at_ms=a.confirmed_at_ms",&[&env,&campaign_id,&account_id]))? {
        let stored:Value=row.get(0);let e=verified_evidence(&stored)?;
        return Ok((None,Some(view(&stored,&e,Some(positive(row.get(1))))?)));
    }
    let pending=db(client.query_opt("SELECT to_jsonb(e) FROM promotion_attribution_migration_evidence e WHERE environment=$1 AND target_campaign_id=$2 AND account_id=$3 AND prepared_at_ms<=$4 AND expires_at_ms>$4 AND NOT EXISTS(SELECT 1 FROM promotion_attributions a WHERE a.environment=e.environment AND a.campaign_id=e.target_campaign_id AND a.account_id=e.account_id) ORDER BY prepared_at_ms DESC,migration_id LIMIT 1",&[&env,&campaign_id,&account_id,&signed(now)?]))?;
    if let Some(row) = pending {
        let stored: Value = row.get(0);
        let e = verified_evidence(&stored)?;
        Ok((Some(view(&stored, &e, None)?), None))
    } else {
        Ok((None, None))
    }
}

fn preview(e: &Evidence) -> Result<Value, String> {
    Ok(
        json!({"policy":POLICY,"status":"ready_for_member_confirmation_preparation",
        "snapshotSha256":hash(e)?,"sourceCampaignId":SOURCE_CAMPAIGN_ID,"targetCampaignId":TARGET_CAMPAIGN_ID,
        "sourceConfirmedAtMs":e.source_confirmed_at_ms,"eligibilityAnchorMs":e.eligibility_anchor_ms,
        "targetMemberAccountSha256":e.target_account_sha256,"targetLeaderAccountSha256":e.target_leader_account_sha256,
        "leaderDisplayName":e.target_leader_display_name,"targetTermsVersion":e.target_terms_version,
        "verifiedPaymentCount":e.payments.len(),"requiresMemberConfirmation":true,
        "movesAccountsOrMoney":false}),
    )
}

impl PromotionStore {
    pub fn preview_legacy_invitation_migration(
        &self,
        source_database_url: &str,
        scope: &PromotionMigrationScope,
        now_ms: u64,
    ) -> Result<Value, String> {
        policy(&self.environment, scope)?;
        let mut source = source_connection(source_database_url)?;
        let mut st = db(source
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start())?;
        with_account_store_postgres_client(&self.database_url, |client| {
            let mut tt = db(client
                .build_transaction()
                .isolation_level(IsolationLevel::RepeatableRead)
                .read_only(true)
                .start())?;
            preview(&capture(&mut st, &mut tt, &self.environment, scope, now_ms, false)?.evidence)
        })
    }

    pub fn stage_legacy_invitation_migration(
        &self,
        source_database_url: &str,
        scope: &PromotionMigrationScope,
        expected_snapshot_sha256: &str,
        operator: &str,
        reviewer: &str,
        evidence_ref: &str,
        idempotency: &str,
        now_ms: u64,
    ) -> Result<Value, String> {
        policy(&self.environment, scope)?;
        if operator == reviewer
            || evidence_ref.trim().is_empty()
            || evidence_ref.len() > 512
            || expected_snapshot_sha256.len() != 64
        {
            return Err("promotionMigrationProofInvalid".into());
        }
        self.mutation(operator,idempotency,"migration.prepare",
            json!([scope,expected_snapshot_sha256,reviewer,evidence_ref]),now_ms,|tx| {
            for (actor,role) in [(operator,"operations"),(reviewer,"finance")] {
                let allowed:bool=db(tx.query_one("SELECT EXISTS(SELECT 1 FROM promotion_operator_roles r JOIN accounts a ON a.account_id=r.account_id WHERE r.environment=$1 AND r.account_id=$2 AND r.role=$3 AND r.revoked_at_ms IS NULL AND NOT a.is_banned)",&[&self.environment,&actor,&role]))?.get(0);
                if !allowed { return Err("promotionPermissionDenied".into()); }
            }
            let mut source=source_connection(source_database_url)?;
            let mut st=db(source.build_transaction().isolation_level(IsolationLevel::RepeatableRead).read_only(true).start())?;
            let c=capture(&mut st,tx,&self.environment,scope,now_ms,true)?;
            let e=&c.evidence; let digest=hash(e)?;
            if digest!=expected_snapshot_sha256 { return Err("promotionMigrationSourceChanged".into()); }
            let migration_id=format!("migration_{}",hash(&json!([POLICY,operator,idempotency,&digest]))?);
            let candidate_id=format!("migration_candidate_{}",hash(&migration_id)?);
            let expires=now_ms.checked_add(TTL_MS).ok_or_else(invalid)?;
            db(tx.execute("INSERT INTO promotion_candidates(environment,campaign_id,candidate_id,account_id,leader_id,first_clicked_at_ms,expires_at_ms,source) VALUES($1,$2,$3,$4,$5,$6,$7,'migration_import')",&[&self.environment,&TARGET_CAMPAIGN_ID,&candidate_id,&c.member.id,&e.target_leader_id,&signed(now_ms)?,&signed(expires)?]))?;
            let payload=serde_json::to_value(e).map_err(|_|invalid())?;
            db(tx.execute("INSERT INTO promotion_attribution_migration_evidence(environment,migration_id,source_relation_key,source_database_sha256,source_campaign_id,source_account_sha256,source_leader_account_sha256,source_confirmed_at_ms,source_terms_version,source_evidence_json,source_evidence_sha256,target_campaign_id,account_id,leader_account_id,leader_id,candidate_id,target_terms_version,eligibility_anchor_ms,prepared_at_ms,expires_at_ms,snapshot_sha256,prepared_by,reviewed_by,evidence_ref) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24)",&[&self.environment,&migration_id,&e.source_relation_key,&e.source_database_sha256,&SOURCE_CAMPAIGN_ID,&e.source_account_sha256,&e.source_leader_account_sha256,&signed(e.source_confirmed_at_ms)?,&e.source_terms_version,&payload,&digest,&TARGET_CAMPAIGN_ID,&c.member.id,&c.leader.id,&e.target_leader_id,&candidate_id,&e.target_terms_version,&signed(e.eligibility_anchor_ms)?,&signed(now_ms)?,&signed(expires)?,&digest,&operator,&reviewer,&evidence_ref]))?;
            audit(tx,&self.environment,operator,"migration.prepared",&migration_id,evidence_ref,
                json!({"campaignId":TARGET_CAMPAIGN_ID,"snapshotSha256":digest,"reviewedBy":reviewer,"requiresMemberConfirmation":true}),now_ms)?;
            let mut result=preview(e)?;
            result["status"]=json!("pending_member_confirmation");result["migrationId"]=json!(migration_id);
            result["candidateId"]=json!(candidate_id);result["preparedAtMs"]=json!(now_ms);result["expiresAtMs"]=json!(expires);
            Ok(result)
        })
    }
}
