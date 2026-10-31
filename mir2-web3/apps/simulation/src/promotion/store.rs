use std::collections::{BTreeMap, BTreeSet};

use postgres::{GenericClient, Transaction};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::{
    calculate_commission, CommissionInput, CommissionResult, MemberInput, PromotionRules,
    RevenueInput, RevenueRecognition, ALGORITHM_VERSION,
};
use crate::{with_account_store_postgres_client, AccountRecord};

#[derive(Debug, Clone)]
pub struct PromotionStore {
    pub(super) database_url: String,
    pub(super) environment: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PromotionAdminRole {
    Operations,
    Risk,
    Finance,
    PayoutSubmit,
    PayoutReview,
    Auditor,
}
#[derive(Debug, Clone)]
pub struct PromotionAdminActor {
    pub subject: String,
    pub role: PromotionAdminRole,
}

fn default_funding_mode() -> String {
    "budget_capped".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CampaignDraft {
    pub campaign_id: String,
    #[serde(default = "default_funding_mode")]
    pub funding_mode: String,
    pub terms_version: String,
    pub starts_at_ms: u64,
    pub ends_at_ms: u64,
    pub rules: PromotionRules,
    pub budget_micro_usd: u64,
    pub member_limit: u64,
    pub leader_limit: u64,
    #[serde(default)]
    pub per_leader_member_limit: u64,
    pub per_member_revenue_cap_micro_usd: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrustedGameFact {
    pub source: String,
    pub kind: String,
    pub source_event_id: String,
    pub account_id: String,
    pub character_index: i32,
    pub occurred_at_ms: u64,
    pub level: Option<u16>,
    pub evidence_ref: String,
}

/// Only a trusted provider/credit-lot adapter may construct this input. There is
/// deliberately no browser HTTP endpoint accepting amounts or authoritative facts.
/// This release accepts sandbox allocations only; production stays pending until
/// an atomic authoritative paid-credit journal is integrated across every spend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SandboxMonthlyRevenue {
    pub allocation_id: String,
    pub campaign_id: String,
    pub account_id: String,
    pub recharge_order_id: String,
    pub monthly_item_uid: u64,
    pub paid_credits: u32,
    pub eligible_micro_usd: u64,
    pub original_currency: String,
    pub original_amount_minor: u64,
    pub service_start_ms: u64,
    pub service_end_ms: u64,
    pub fx_snapshot_ref: String,
    pub evidence_ref: String,
    pub sandbox: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(
    tag = "action",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum AdminCommand {
    ConfigureDraft {
        draft: CampaignDraft,
        evidence_ref: String,
    },
    ApproveRules {
        evidence_ref: String,
    },
    SetRecruitment {
        enabled: bool,
        evidence_ref: String,
    },
    SetCommission {
        enabled: bool,
        evidence_ref: String,
    },
    SetPayout {
        enabled: bool,
        evidence_ref: String,
    },
    ProposeLaunch {
        manifest: super::PromotionLaunchManifest,
        evidence_ref: String,
    },
    ApproveLaunch {
        manifest_sha256: String,
        evidence_ref: String,
    },
    FreezePayoutIntent {
        intent: super::PromotionPayoutIntent,
        evidence_ref: String,
    },
    ReviewLeader {
        leader_id: String,
        decision: String,
        reason: String,
        evidence_ref: String,
    },
    ExcludeMember {
        account_id: String,
        reason: String,
        evidence_ref: String,
    },
    Reconcile {
        evidence_ref: String,
    },
    SetInputWatermark {
        watermark_ms: u64,
        evidence_ref: String,
    },
    CreateSettlement {
        leader_id: String,
        evidence_ref: String,
    },
    ApproveSettlement {
        settlement_id: String,
        snapshot_sha256: String,
        evidence_ref: String,
    },
    CreatePayoutDraft {
        settlement_id: String,
        network: String,
        coin_type: String,
        evidence_ref: String,
    },
    ApprovePayout {
        payout_id: String,
        #[serde(default)]
        intent_sha256: Option<String>,
        evidence_ref: String,
    },
    SubmitPayout {
        payout_id: String,
    },
}

impl AdminCommand {
    fn role(&self) -> PromotionAdminRole {
        match self {
            Self::ConfigureDraft { .. }
            | Self::SetRecruitment { .. }
            | Self::ProposeLaunch { .. } => PromotionAdminRole::Operations,
            Self::ReviewLeader { .. } | Self::ExcludeMember { .. } => PromotionAdminRole::Risk,
            Self::ApproveRules { .. }
            | Self::SetCommission { .. }
            | Self::SetPayout { .. }
            | Self::ApproveLaunch { .. }
            | Self::Reconcile { .. }
            | Self::SetInputWatermark { .. }
            | Self::CreateSettlement { .. }
            | Self::ApproveSettlement { .. } => PromotionAdminRole::Finance,
            Self::CreatePayoutDraft { .. }
            | Self::SubmitPayout { .. }
            | Self::FreezePayoutIntent { .. } => PromotionAdminRole::PayoutSubmit,
            Self::ApprovePayout { .. } => PromotionAdminRole::PayoutReview,
        }
    }
    fn evidence(&self) -> &str {
        match self {
            Self::ConfigureDraft { evidence_ref, .. }
            | Self::ApproveRules { evidence_ref }
            | Self::SetRecruitment { evidence_ref, .. }
            | Self::SetCommission { evidence_ref, .. }
            | Self::SetPayout { evidence_ref, .. }
            | Self::ProposeLaunch { evidence_ref, .. }
            | Self::ApproveLaunch { evidence_ref, .. }
            | Self::FreezePayoutIntent { evidence_ref, .. }
            | Self::ReviewLeader { evidence_ref, .. }
            | Self::ExcludeMember { evidence_ref, .. }
            | Self::Reconcile { evidence_ref }
            | Self::SetInputWatermark { evidence_ref, .. }
            | Self::CreateSettlement { evidence_ref, .. }
            | Self::ApproveSettlement { evidence_ref, .. }
            | Self::CreatePayoutDraft { evidence_ref, .. }
            | Self::ApprovePayout { evidence_ref, .. } => evidence_ref,
            Self::SubmitPayout { .. } => "signed_payout_worker_required",
        }
    }
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || b"-_.:@".contains(&v))
}
pub(super) fn signed(value: u64) -> Result<i64, String> {
    value
        .try_into()
        .map_err(|_| "promotionAmountOverflow".into())
}
pub(super) fn positive(value: i64) -> u64 {
    value.max(0) as u64
}
pub(super) fn db<T>(result: Result<T, postgres::Error>) -> Result<T, String> {
    result.map_err(|_| "promotionStorageUnavailable".into())
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|v| format!("{v:02x}")).collect()
}
fn canonical_json(value: &Value) -> Result<String, String> {
    Ok(match value {
        Value::Null => "null".into(),
        Value::Bool(v) => v.to_string(),
        Value::Number(v) => {
            if v.is_f64() {
                return Err("promotionFloatingAmountForbidden".into());
            }
            v.to_string()
        }
        Value::String(v) => serde_json::to_string(v).map_err(|_| "promotionInputInvalid")?,
        Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(canonical_json)
                .collect::<Result<Vec<_>, _>>()?
                .join(",")
        ),
        Value::Object(values) => {
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort();
            let entries = keys
                .into_iter()
                .map(|key| {
                    Ok(format!(
                        "{}:{}",
                        serde_json::to_string(key).map_err(|_| "promotionInputInvalid")?,
                        canonical_json(&values[key])?
                    ))
                })
                .collect::<Result<Vec<String>, String>>()?;
            format!("{{{}}}", entries.join(","))
        }
    })
}
pub(super) fn hash(value: &impl Serialize) -> Result<String, String> {
    let value = serde_json::to_value(value).map_err(|_| "promotionInputInvalid")?;
    let data = canonical_json(&value)?;
    Ok(hex(&Sha256::digest(data.as_bytes())))
}
fn token(prefix: &str) -> Result<String, String> {
    let mut bytes = [0u8; 24];
    OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_| "promotionEntropyUnavailable")?;
    Ok(format!(
        "{prefix}{}",
        bytes.iter().map(|v| format!("{v:02x}")).collect::<String>()
    ))
}
fn require_actor(actor: &PromotionAdminActor, role: PromotionAdminRole) -> Result<(), String> {
    if !valid_id(&actor.subject) || actor.role != role {
        return Err("promotionPermissionDenied".into());
    }
    Ok(())
}
pub(super) fn campaign<T: GenericClient>(
    client: &mut T,
    env: &str,
    id: &str,
    lock: bool,
) -> Result<postgres::Row, String> {
    if !valid_id(id) {
        return Err("promotionRequestInvalid".into());
    }
    let sql = if lock {
        "SELECT terms_version,status,starts_at_ms,ends_at_ms,rules_json,rules_approved,recruitment_enabled,commission_enabled,budget_micro_usd,reserved_micro_usd,member_limit,leader_limit,per_member_revenue_cap_micro_usd,input_watermark_ms,configured_by,approved_by,per_leader_member_limit,funding_mode FROM promotion_campaigns WHERE environment=$1 AND campaign_id=$2 FOR UPDATE"
    } else {
        "SELECT terms_version,status,starts_at_ms,ends_at_ms,rules_json,rules_approved,recruitment_enabled,commission_enabled,budget_micro_usd,reserved_micro_usd,member_limit,leader_limit,per_member_revenue_cap_micro_usd,input_watermark_ms,configured_by,approved_by,per_leader_member_limit,funding_mode FROM promotion_campaigns WHERE environment=$1 AND campaign_id=$2"
    };
    db(client.query_opt(sql, &[&env, &id]))?.ok_or_else(|| "promotionCampaignMissing".into())
}
pub(super) fn rules(row: &postgres::Row) -> Result<PromotionRules, String> {
    serde_json::from_value(row.get(4)).map_err(|_| "promotionRulesInvalid".into())
}
fn configured_commission<T: GenericClient>(
    client: &mut T,
    input: &CommissionInput,
) -> Result<CommissionResult, String> {
    let row = campaign(client, &input.environment, &input.campaign_id, false)?;
    let micro =
        super::production_gate::test_micro_enabled(&input.environment, &row.get::<_, Value>(4))?;
    if rules(&row)?.revenue_recognition == RevenueRecognition::MonthlyPurchaseFull {
        super::calculate_full_monthly_commission(input, micro)
    } else if micro {
        super::calculate_test_usdc_micro_commission(input)
    } else {
        calculate_commission(input)
    }
}

fn commission_input_hash(
    input: &CommissionInput,
    result: &CommissionResult,
) -> Result<String, String> {
    if result.algorithm_version == ALGORITHM_VERSION {
        hash(input)
    } else {
        hash(&json!({"input":input,"algorithmVersion":result.algorithm_version}))
    }
}

/// Frozen snapshot decides the amount precision; a live rule cannot reinterpret
/// an existing cent settlement or add a minimum grant to a fractional result.
pub(super) fn settlement_payable_micro_usd<T: GenericClient>(
    client: &mut T,
    env: &str,
    campaign_id: &str,
    snapshot: &Value,
    total: i64,
    reserve: i64,
    cents: i64,
) -> Result<i64, String> {
    let row = campaign(client, env, campaign_id, false)?;
    let micro = super::production_gate::test_micro_enabled(env, &row.get::<_, Value>(4))?;
    let frozen = snapshot
        .get("result")
        .and_then(|v| v.get("payableMicroUsd"));
    if micro {
        let expected = total
            .checked_sub(reserve)
            .filter(|v| *v >= 0)
            .ok_or("promotionSnapshotInvalid")?;
        if frozen.and_then(Value::as_u64) != u64::try_from(expected).ok()
            || snapshot["result"]["commissionMicroUsd"].as_u64() != u64::try_from(total).ok()
            || snapshot["result"]["reserveMicroUsd"].as_u64() != u64::try_from(reserve).ok()
        {
            return Err("promotionSnapshotInvalid".into());
        }
        Ok(expected)
    } else {
        if frozen.is_some() {
            return Err("promotionSnapshotInvalid".into());
        }
        cents
            .checked_mul(10_000)
            .ok_or_else(|| "promotionAmountOverflow".into())
    }
}
pub(super) fn terms(row: &postgres::Row, version: &str) -> Result<(), String> {
    if row.get::<_, String>(0) != version {
        return Err("promotionTermsChanged".into());
    }
    Ok(())
}
pub(super) fn active(row: &postgres::Row, now: u64) -> bool {
    row.get::<_, String>(1) == "active" && now >= positive(row.get(2)) && now < positive(row.get(3))
}
pub(super) fn account<T: GenericClient>(
    client: &mut T,
    id: &str,
    lock: bool,
) -> Result<postgres::Row, String> {
    if !valid_id(id) {
        return Err("promotionAuthenticationRequired".into());
    }
    let sql = if lock {
        "SELECT raw_json,(extract(epoch FROM created_at)*1000)::bigint,is_banned FROM accounts WHERE account_id=$1 FOR UPDATE"
    } else {
        "SELECT raw_json,(extract(epoch FROM created_at)*1000)::bigint,is_banned FROM accounts WHERE account_id=$1"
    };
    let row = db(client.query_opt(sql, &[&id]))?.ok_or("promotionAccountMissing")?;
    if row.get::<_, bool>(2) {
        return Err("promotionAccountBanned".into());
    }
    Ok(row)
}
// Address changes and payment sends share this lock before account/payout rows.
// Taking it after a payout row would invert verify_payout_address's lock order.
pub(super) fn lock_payee<T: GenericClient>(
    tx: &mut T,
    env: &str,
    account: &str,
) -> Result<(), String> {
    let key = format!("promotion-payee:{env}:{account}");
    db(tx.query_one(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
        &[&key],
    ))?;
    Ok(())
}
pub(super) fn audit(
    tx: &mut Transaction<'_>,
    env: &str,
    actor: &str,
    action: &str,
    resource: &str,
    evidence: &str,
    value: Value,
    now: u64,
) -> Result<(), String> {
    db(tx.execute("INSERT INTO promotion_audit_log(environment,actor_id,action,resource_id,evidence_ref,summary_json,occurred_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7)",&[&env,&actor,&action,&resource,&evidence,&value,&signed(now)?]))?;
    // Reading an audited dashboard/export must not recursively create work for
    // the very queue being inspected. Historical read events remain supported.
    if matches!(action, "admin.read" | "settlement.export") {
        return Ok(());
    }
    // This narrowly scoped amendment has already reconciled every leader in
    // the same transaction. The durable audit records completion; it does not
    // create a second projection or an external delivery obligation.
    if action == "rules.full_monthly_adopted" {
        return Ok(());
    }
    db(tx.execute("INSERT INTO promotion_outbox(environment,event_type,resource_id,event_json,created_at_ms) VALUES($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING",&[&env,&action,&resource,&value,&signed(now)?]))?;
    Ok(())
}

impl PromotionStore {
    pub fn new(database_url: impl Into<String>, environment: &str) -> Result<Self, String> {
        let database_url = database_url.into();
        if database_url.trim().is_empty()
            || !matches!(environment, "development" | "test" | "production")
        {
            return Err("promotionConfigurationInvalid".into());
        }
        Ok(Self {
            database_url,
            environment: environment.into(),
        })
    }
    pub fn environment(&self) -> &str {
        &self.environment
    }
    fn data_mode(&self) -> &'static str {
        if self.environment == "production" {
            "real"
        } else {
            "sandbox"
        }
    }

    pub(super) fn mutation(
        &self,
        account: &str,
        key: &str,
        operation: &str,
        request: Value,
        now: u64,
        action: impl FnOnce(&mut Transaction<'_>) -> Result<Value, String>,
    ) -> Result<Value, String> {
        if !valid_id(account) || !valid_id(key) || now == 0 {
            return Err("promotionRequestInvalid".into());
        }
        let request_hash = hash(&request)?;
        with_account_store_postgres_client(&self.database_url, |client| {
            let mut tx = db(client.transaction())?;
            // Serialize a principal's idempotency keys before checking/replaying.
            let lock = format!("promotion:{}:{account}:{key}", self.environment);
            db(tx.query_one(
                "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
                &[&lock],
            ))?;
            if let Some(row)=db(tx.query_opt("SELECT operation,request_sha256,response_json FROM promotion_idempotency WHERE environment=$1 AND account_id=$2 AND idempotency_key=$3",&[&self.environment,&account,&key]))? {
                if row.get::<_,String>(0)!=operation||row.get::<_,String>(1)!=request_hash { return Err("promotionIdempotencyConflict".into()); }
                return Ok(row.get(2));
            }
            let value = action(&mut tx)?;
            db(tx.execute("INSERT INTO promotion_idempotency(environment,account_id,idempotency_key,operation,request_sha256,response_json,created_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7)",&[&self.environment,&account,&key,&operation,&request_hash,&value,&signed(now)?]))?;
            db(tx.commit())?;
            Ok(value)
        })
    }

    pub fn public_campaign(&self, campaign_id: &str) -> Result<Value, String> {
        with_account_store_postgres_client(&self.database_url, |client| {
            let row = match campaign(client, &self.environment, campaign_id, false) {
                Ok(v) => v,
                Err(e) if e == "promotionCampaignMissing" => {
                    return Ok(
                        json!({"campaignId":campaign_id,"environment":self.environment,"status":"unconfigured","termsStatus":"draft","recruitmentEnabled":false,"commissionEnabled":false,"payoutEnabled":false,"dataMode":self.data_mode(),"sourceGaps":super::production_gate::gaps(client,&self.environment,campaign_id)?}),
                    )
                }
                Err(e) => return Err(e),
            };
            let approved: bool = row.get(5);
            Ok(
                json!({"campaignId":campaign_id,"environment":self.environment,"status":row.get::<_,String>(1),"termsVersion":row.get::<_,String>(0),"fundingMode":row.get::<_,String>(17),"termsStatus":if approved {"approved"} else {"draft"},"startsAtMs":positive(row.get(2)),"endsAtMs":positive(row.get(3)),"recruitmentEnabled":row.get::<_,bool>(6),"commissionEnabled":row.get::<_,bool>(7),"payoutEnabled":db(client.query_one("SELECT payout_enabled FROM promotion_campaigns WHERE environment=$1 AND campaign_id=$2",&[&self.environment,&campaign_id]))?.get::<_,bool>(0),"payoutPolicy":super::production_gate::public_policy(client,&self.environment,campaign_id)?,"rules":if approved {row.get::<_,Value>(4)} else {Value::Null},"dataMode":self.data_mode(),"sourceGaps":super::production_gate::gaps(client,&self.environment,campaign_id)?,"disclosure":"Direct referrals only. Unpaid registration and activity do not create redeemable rewards."}),
            )
        })
    }

    pub fn invite_preview(&self, campaign_id: &str, code: &str) -> Result<Value, String> {
        self.invite_preview_at(
            campaign_id,
            code,
            crate::monthly_card::monthly_card_now_ms(),
        )
    }

    /// The clock is supplied by the server, never a browser request field.
    pub fn invite_preview_at(
        &self,
        campaign_id: &str,
        code: &str,
        now_ms: u64,
    ) -> Result<Value, String> {
        if !valid_id(code) {
            return Err("promotionInviteMissing".into());
        }
        with_account_store_postgres_client(&self.database_url, |client| {
            let row = campaign(client, &self.environment, campaign_id, false)?;
            let leader=db(client.query_opt("SELECT display_name,status,leader_id FROM promotion_leaders WHERE environment=$1 AND campaign_id=$2 AND invite_code=$3",&[&self.environment,&campaign_id,&code]))?.ok_or("promotionInviteMissing")?;
            if leader.get::<_, String>(1) != "approved" {
                return Err("promotionInviteUnavailable".into());
            }
            let capacity = admission_capacity(
                client,
                &self.environment,
                campaign_id,
                &leader.get::<_, String>(2),
                &row,
                now_ms,
            )?;
            Ok(
                json!({"campaignId":campaign_id,"leaderDisplayName":leader.get::<_,String>(0),"termsVersion":row.get::<_,String>(0),"termsStatus":if row.get::<_,bool>(5){"approved"}else{"draft"},"recruitmentEnabled":row.get::<_,bool>(6)&&active(&row,now_ms),"rewardedPlacesAvailable":capacity.available,"capacityReason":capacity.reason,"unrewardedPlayAvailable":true,"asOfMs":now_ms,"capacityIsReservation":false,"reservationOccursOnConfirmation":true,"dataMode":self.data_mode()}),
            )
        })
    }

    pub fn apply(
        &self,
        campaign_id: &str,
        account_id: &str,
        character_index: i32,
        terms_version: &str,
        idempotency: &str,
        now_ms: u64,
    ) -> Result<Value, String> {
        self.mutation(account_id,idempotency,"leader.apply",json!([campaign_id,character_index,terms_version]),now_ms,|tx| {
            let row=campaign(tx,&self.environment,campaign_id,true)?; terms(&row,terms_version)?;
            let rule=rules(&row)?;
            if rule.test_wallet_enrollment&&self.environment!="test" {return Err("promotionEnvironmentMismatch".into());}
            let _=account(tx,account_id,false)?;
            if character_index<0 {
                if character_index != -1||self.environment!="test"||!rule.test_wallet_enrollment {return Err("promotionTestWalletEnrollmentUnavailable".into());}
                if !row.get::<_,bool>(5)||!row.get::<_,bool>(6)||!active(&row,now_ms) {return Err("promotionRecruitmentClosed".into());}
                let credential=db(tx.query_opt("SELECT credential_id,credential_subject FROM identity_credentials WHERE account_id=$1 AND credential_kind='sui_wallet' AND revoked_at_ms IS NULL ORDER BY created_at_ms,credential_id LIMIT 1 FOR SHARE",&[&account_id]))?.ok_or("promotionWalletProofRequired")?;
                let address:String=credential.get(1);
                if address.len()!=66||!address.starts_with("0x")||!address[2..].bytes().all(|b|b.is_ascii_hexdigit()) {return Err("promotionWalletProofRequired".into());}
                if let Some(existing)=db(tx.query_opt("SELECT leader_id,status FROM promotion_leaders WHERE environment=$1 AND campaign_id=$2 AND account_id=$3",&[&self.environment,&campaign_id,&account_id]))? {
                    return Ok(json!({"leaderId":existing.get::<_,String>(0),"status":existing.get::<_,String>(1),"replayed":true}));
                }
                let count:i64=db(tx.query_one("SELECT count(*) FROM promotion_leaders WHERE environment=$1 AND campaign_id=$2 AND status='approved'",&[&self.environment,&campaign_id]))?.get(0);
                if row.get::<_,i64>(11)>0&&count>=row.get::<_,i64>(11) {return Err("promotionLeaderLimit".into());}
                let id=token("ldr_")?;let code=token("inv_")?;
                let name=format!("Wallet {}…{}",&address[..8],&address[62..]);
                db(tx.execute("INSERT INTO promotion_leaders(environment,campaign_id,account_id,leader_id,invite_code,display_character_index,display_name,status,accepted_terms_version,applied_at_ms) VALUES($1,$2,$3,$4,$5,-1,$6,'approved',$7,$8)",&[&self.environment,&campaign_id,&account_id,&id,&code,&name,&terms_version,&signed(now_ms)?]))?;
                audit(tx,&self.environment,account_id,"leader.test_wallet_enrolled",&id,&credential.get::<_,String>(0),json!({"campaignId":campaign_id,"termsVersion":terms_version,"characterIndex":-1,"testOnly":true,"enrollment":"test_wallet_enrollment","gameQualificationGranted":false,"paymentEvidenceCreated":false}),now_ms)?;
                return Ok(json!({"leaderId":id,"status":"approved","termsStatus":"approved","recruitmentEnabled":true,"testWalletEnrollment":true}));
            }
            let owned=db(tx.query_opt("SELECT character_name,level FROM characters WHERE account_id=$1 AND character_index=$2",&[&account_id,&character_index]))?.ok_or("promotionCharacterMissing")?;
            if owned.get::<_,i32>(1)<i32::from(rule.minimum_level) { return Err("promotionLevelRequired".into()); }
            if let Some(existing)=db(tx.query_opt("SELECT leader_id,status FROM promotion_leaders WHERE environment=$1 AND campaign_id=$2 AND account_id=$3",&[&self.environment,&campaign_id,&account_id]))? {
                return Ok(json!({"leaderId":existing.get::<_,String>(0),"status":existing.get::<_,String>(1),"replayed":true}));
            }
            let id=token("ldr_")?; let code=token("inv_")?; let name:String=owned.get(0);
            db(tx.execute("INSERT INTO promotion_leaders(environment,campaign_id,account_id,leader_id,invite_code,display_character_index,display_name,accepted_terms_version,applied_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)",&[&self.environment,&campaign_id,&account_id,&id,&code,&character_index,&name,&terms_version,&signed(now_ms)?]))?;
            audit(tx,&self.environment,account_id,"leader.applied",&id,"accepted_terms",json!({"campaignId":campaign_id,"termsVersion":terms_version,"characterIndex":character_index}),now_ms)?;
            Ok(json!({"leaderId":id,"status":"applied","termsStatus":if row.get::<_,bool>(5){"approved"}else{"draft"},"recruitmentEnabled":false}))
        })
    }

    pub fn record_invitation_click(
        &self,
        campaign_id: &str,
        code: &str,
        now_ms: u64,
    ) -> Result<Value, String> {
        if !valid_id(code) || now_ms == 0 {
            return Err("promotionRequestInvalid".into());
        }
        with_account_store_postgres_client(&self.database_url, |client| {
            let mut tx = db(client.transaction())?;
            let row = campaign(&mut tx, &self.environment, campaign_id, false)?;
            let leader=db(tx.query_opt("SELECT leader_id,display_name,status FROM promotion_leaders WHERE environment=$1 AND campaign_id=$2 AND invite_code=$3",&[&self.environment,&campaign_id,&code]))?.ok_or("promotionInviteMissing")?;
            if leader.get::<_, String>(2) != "approved" {
                return Err("promotionInviteUnavailable".into());
            }
            let id = token("click_")?;
            let expires = now_ms
                .checked_add(rules(&row)?.candidate_ttl_ms)
                .ok_or("promotionRequestInvalid")?;
            db(tx.execute("INSERT INTO promotion_invitation_clicks(environment,campaign_id,click_id,leader_id,clicked_at_ms,expires_at_ms) VALUES($1,$2,$3,$4,$5,$6)",&[&self.environment,&campaign_id,&id,&leader.get::<_,String>(0),&signed(now_ms)?,&signed(expires)?]))?;
            db(tx.commit())?;
            Ok(
                json!({"clickId":id,"expiresAtMs":expires,"leaderDisplayName":leader.get::<_,String>(1),"recruitmentEnabled":row.get::<_,bool>(6),"unrewardedPlayAvailable":true}),
            )
        })
    }

    pub fn candidate_from_click(
        &self,
        campaign_id: &str,
        account_id: &str,
        click_id: &str,
        idempotency: &str,
        now_ms: u64,
    ) -> Result<Value, String> {
        self.mutation(account_id,idempotency,"attribution.candidateFromClick",json!([campaign_id,click_id]),now_ms,|tx| {
            let row=campaign(tx,&self.environment,campaign_id,false)?;let rule=rules(&row)?;let acc=account(tx,account_id,false)?;
            let click=db(tx.query_opt("SELECT leader_id,clicked_at_ms,expires_at_ms,claimed_by_account_id FROM promotion_invitation_clicks WHERE environment=$1 AND campaign_id=$2 AND click_id=$3 FOR UPDATE",&[&self.environment,&campaign_id,&click_id]))?.ok_or("promotionClickMissing")?;
            let clicked=positive(click.get(1));let expires=positive(click.get(2));let registered=positive(acc.get(1));
            if now_ms>=expires||registered<clicked||registered>=expires||now_ms>registered.saturating_add(rule.confirmation_window_ms){return Err("promotionCandidateExpired".into());}
            if click.get::<_,Option<String>>(3).is_some_and(|v|v!=account_id){return Err("promotionCandidateOwnershipMismatch".into());}
            let leader_id:String=click.get(0);
            let leader=db(tx.query_opt("SELECT display_name,status,account_id FROM promotion_leaders WHERE environment=$1 AND campaign_id=$2 AND leader_id=$3",&[&self.environment,&campaign_id,&leader_id]))?.ok_or("promotionInviteMissing")?;
            if leader.get::<_,String>(1)!="approved"{return Err("promotionInviteUnavailable".into());}
            if leader.get::<_,String>(2)==account_id{return Err("promotionSelfReferral".into());}
            let candidate=token("cand_")?;
            db(tx.execute("INSERT INTO promotion_candidates(environment,campaign_id,candidate_id,account_id,leader_id,first_clicked_at_ms,expires_at_ms,source) VALUES($1,$2,$3,$4,$5,$6,$7,'invite_link')",&[&self.environment,&campaign_id,&candidate,&account_id,&leader_id,&signed(clicked)?,&signed(expires)?]))?;
            db(tx.execute("UPDATE promotion_invitation_clicks SET claimed_by_account_id=$3 WHERE environment=$1 AND click_id=$2",&[&self.environment,&click_id,&account_id]))?;
            audit(tx,&self.environment,account_id,"attribution.candidate",&candidate,click_id,json!({"campaignId":campaign_id,"leaderId":leader_id,"firstClickedAtMs":clicked}),now_ms)?;
            Ok(json!({"candidateId":candidate,"leaderDisplayName":leader.get::<_,String>(0),"firstClickedAtMs":clicked,"expiresAtMs":expires,"recruitmentEnabled":row.get::<_,bool>(6),"unrewardedPlayAvailable":true}))
        })
    }

    pub fn create_candidate(
        &self,
        campaign_id: &str,
        code: &str,
        account_id: &str,
        source: &str,
        idempotency: &str,
        now_ms: u64,
    ) -> Result<Value, String> {
        if source == "invite_link" {
            return Err("promotionClickRequired".into());
        }
        if source != "manual_code" {
            return Err("promotionRequestInvalid".into());
        }
        self.mutation(account_id,idempotency,"attribution.candidate",json!([campaign_id,code,source]),now_ms,|tx| {
            let row=campaign(tx,&self.environment,campaign_id,false)?; let _=account(tx,account_id,false)?;
            let leader=db(tx.query_opt("SELECT leader_id,account_id,status,display_name FROM promotion_leaders WHERE environment=$1 AND campaign_id=$2 AND invite_code=$3",&[&self.environment,&campaign_id,&code]))?.ok_or("promotionInviteMissing")?;
            if leader.get::<_,String>(1)==account_id { return Err("promotionSelfReferral".into()); }
            if leader.get::<_,String>(2)!="approved" { return Err("promotionInviteUnavailable".into()); }
            let id=token("cand_")?; let expires=now_ms.checked_add(rules(&row)?.candidate_ttl_ms).ok_or("promotionRequestInvalid")?;
            let leader_id:String=leader.get(0);
            db(tx.execute("INSERT INTO promotion_candidates(environment,campaign_id,candidate_id,account_id,leader_id,first_clicked_at_ms,expires_at_ms,source) VALUES($1,$2,$3,$4,$5,$6,$7,$8)",&[&self.environment,&campaign_id,&id,&account_id,&leader_id,&signed(now_ms)?,&signed(expires)?,&source]))?;
            audit(tx,&self.environment,account_id,"attribution.candidate",&id,"authenticated_candidate",json!({"campaignId":campaign_id,"leaderId":leader_id}),now_ms)?;
            Ok(json!({"candidateId":id,"leaderDisplayName":leader.get::<_,String>(3),"expiresAtMs":expires,"recruitmentEnabled":row.get::<_,bool>(6),"unrewardedPlayAvailable":true}))
        })
    }

    pub fn confirm(
        &self,
        campaign_id: &str,
        account_id: &str,
        candidate_id: &str,
        terms_version: &str,
        idempotency: &str,
        now_ms: u64,
    ) -> Result<Value, String> {
        self.mutation(account_id,idempotency,"attribution.confirm",json!([campaign_id,candidate_id,terms_version]),now_ms,|tx| {
            if let Some(result)=super::attribution_migration::try_confirm(tx,&self.environment,campaign_id,account_id,candidate_id,terms_version,now_ms)? {
                return Ok(result);
            }
            // Campaign row serializes every reservation, even across leaders.
            let row=campaign(tx,&self.environment,campaign_id,true)?; terms(&row,terms_version)?;
            if !active(&row,now_ms)||!row.get::<_,bool>(5)||!row.get::<_,bool>(6) { return Err("promotionRecruitmentClosed".into()); }
            let acc=account(tx,account_id,true)?; let registered=positive(acc.get(1)); let rules=rules(&row)?;
            if registered<positive(row.get(2))||registered>=positive(row.get(3)) { return Err("promotionExistingAccountExcluded".into()); }
            if now_ms<registered||now_ms>registered.saturating_add(rules.confirmation_window_ms) { return Err("promotionConfirmationExpired".into()); }
            let saved:AccountRecord=serde_json::from_value(acc.get(0)).map_err(|_|"promotionAccountUnavailable")?;
            if saved.billing.as_ref().is_some_and(|l|l.orders.values().any(|o|o.paid_at_ms.is_some())||l.sui_usdc_orders.values().any(|o|o.has_payment_receipt())) { return Err("promotionAlreadyPaid".into()); }
            let cand=db(tx.query_opt("SELECT leader_id,first_clicked_at_ms,expires_at_ms,source FROM promotion_candidates WHERE environment=$1 AND campaign_id=$2 AND candidate_id=$3 AND account_id=$4",&[&self.environment,&campaign_id,&candidate_id,&account_id]))?.ok_or("promotionCandidateMissing")?;
            if now_ms>=positive(cand.get(2)) { return Err("promotionCandidateExpired".into()); }
            let clicked=positive(cand.get(1));
            // Authenticated cross-device/manual candidates can be created just
            // after registration. Never infer an earlier unauthenticated click.
            if clicked>registered.saturating_add(rules.confirmation_window_ms)||registered>clicked.saturating_add(rules.candidate_ttl_ms) { return Err("promotionCandidateExpired".into()); }
            let leader:String=cand.get(0);
            let owner=db(tx.query_opt("SELECT account_id,status FROM promotion_leaders WHERE environment=$1 AND campaign_id=$2 AND leader_id=$3",&[&self.environment,&campaign_id,&leader]))?.ok_or("promotionInviteMissing")?;
            if owner.get::<_,String>(0)==account_id { return Err("promotionSelfReferral".into()); }
            if owner.get::<_,String>(1)!="approved" { return Err("promotionInviteUnavailable".into()); }
            if let Some(existing)=db(tx.query_opt("SELECT leader_id FROM promotion_attributions WHERE environment=$1 AND campaign_id=$2 AND account_id=$3",&[&self.environment,&campaign_id,&account_id]))? {
                if existing.get::<_,String>(0)==leader { return Ok(json!({"status":"bound","replayed":true,"campaignId":campaign_id})); }
                return Err("promotionAttributionFrozen".into());
            }
            let capacity=admission_capacity(tx,&self.environment,campaign_id,&leader,&row,now_ms)?;
            if !capacity.available{return Err("promotionBudgetFull".into());}
            let reserve=capacity.reserve_micro_usd;let total=capacity.total_reserved_micro_usd;
            let source:String=cand.get(3);
            db(tx.execute("INSERT INTO promotion_attributions(environment,campaign_id,account_id,leader_id,candidate_id,first_clicked_at_ms,registered_at_ms,confirmed_at_ms,terms_version,source,reserved_micro_usd) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",&[&self.environment,&campaign_id,&account_id,&leader,&candidate_id,&signed(clicked)?,&signed(registered)?,&signed(now_ms)?,&terms_version,&source,&signed(reserve)?]))?;
            db(tx.execute("UPDATE promotion_campaigns SET reserved_micro_usd=$3 WHERE environment=$1 AND campaign_id=$2",&[&self.environment,&campaign_id,&signed(total)?]))?;
            refresh_account_qualifications(tx,&self.environment,account_id,now_ms)?;
            audit(tx,&self.environment,account_id,"attribution.confirmed",&format!("{campaign_id}:{account_id}"),candidate_id,json!({"leaderId":leader,"reservedMicroUsd":reserve.to_string(),"termsVersion":terms_version}),now_ms)?;
            Ok(json!({"campaignId":campaign_id,"status":"bound","reasonCode":"requirements_pending","reservedMicroUsd":reserve.to_string()}))
        })
    }

    pub fn me(&self, campaign_id: &str, account_id: &str, now_ms: u64) -> Result<Value, String> {
        with_account_store_postgres_client(&self.database_url, |client| {
            let acc = account(client, account_id, false)?;
            let characters=db(client.query("SELECT character_index,character_name,level,class,gender FROM characters WHERE account_id=$1 ORDER BY character_index",&[&account_id]))?.iter().map(|c| json!({"index":c.get::<_,i32>(0),"name":c.get::<_,String>(1),"level":c.get::<_,i32>(2),"class":c.get::<_,String>(3),"gender":c.get::<_,String>(4)})).collect::<Vec<_>>();
            let operators=db(client.query("SELECT role FROM promotion_operator_roles WHERE environment=$1 AND account_id=$2 AND revoked_at_ms IS NULL ORDER BY role",&[&self.environment,&account_id]))?.iter().map(|r|r.get::<_,String>(0)).collect::<Vec<_>>();
            let leader=db(client.query_opt("SELECT leader_id,status,display_name,display_character_index,invite_code FROM promotion_leaders WHERE environment=$1 AND campaign_id=$2 AND account_id=$3",&[&self.environment,&campaign_id,&account_id]))?;
            let leader_json=leader.as_ref().map(|r|json!({"id":r.get::<_,String>(0),"status":r.get::<_,String>(1),"displayName":r.get::<_,String>(2),"characterIndex":r.get::<_,i32>(3),"inviteCode":if r.get::<_,String>(1)=="approved"{Some(r.get::<_,String>(4))}else{None}}));
            let mut attribution=db(client.query_opt("SELECT leader_id,confirmed_at_ms,status,reason_code,qualified_at_ms FROM promotion_attributions WHERE environment=$1 AND campaign_id=$2 AND account_id=$3",&[&self.environment,&campaign_id,&account_id]))?.map(|r|json!({"leaderId":r.get::<_,String>(0),"confirmedAtMs":positive(r.get(1)),"status":r.get::<_,String>(2),"reasonCode":r.get::<_,String>(3),"qualifiedAtMs":r.get::<_,Option<i64>>(4).map(positive)}));
            let (migration_candidate, migration) = super::attribution_migration::account_view(
                client,
                &self.environment,
                campaign_id,
                account_id,
                now_ms,
            )?;
            if let (Some(attribution), Some(migration)) = (attribution.as_mut(), migration) {
                attribution["migration"] = migration;
            }
            let max_level = characters
                .iter()
                .filter_map(|c| c["level"].as_u64())
                .max()
                .unwrap_or(0);
            let active_days:i64=db(client.query_one("SELECT count(DISTINCT utc_day) FROM promotion_game_facts WHERE environment=$1 AND account_id=$2 AND source IN ('combat_commit','quest_commit')",&[&self.environment,&account_id]))?.get(0);
            let current = campaign(client, &self.environment, campaign_id, false).ok();
            let minimum = current
                .as_ref()
                .map(rules)
                .transpose()?
                .map(|r| r.minimum_level)
                .unwrap_or(30);
            let dashboard = if let Some(ref leader) = leader {
                dashboard(
                    client,
                    &self.environment,
                    campaign_id,
                    &leader.get::<_, String>(0),
                    now_ms,
                )?
            } else {
                json!({"clicks":0,"registrations":0,"confirmed":0,"qualified":0,"tiers":[],"eligibleRevenueMicroUsd":"0","estimatedCommissionMicroUsd":"0","commissionEnabled":false,"status":"not_a_leader"})
            };
            let payout = payout_settings(client, &self.environment, account_id)?;
            let settlements = list_settlements(client, &self.environment, campaign_id, account_id)?;
            let appeals=db(client.query("SELECT appeal_id,category,status,created_at_ms FROM promotion_appeals WHERE environment=$1 AND campaign_id=$2 AND account_id=$3 ORDER BY created_at_ms DESC LIMIT 25",&[&self.environment,&campaign_id,&account_id]))?.iter().map(|r|json!({"id":r.get::<_,String>(0),"category":r.get::<_,String>(1),"status":r.get::<_,String>(2),"createdAtMs":positive(r.get(3))})).collect::<Vec<_>>();
            Ok(
                json!({"environment":self.environment,"campaignId":campaign_id,"account":{"id":account_id,"registeredAtMs":positive(acc.get(1)),"banned":false},"characters":characters,"leader":leader_json,"dashboard":dashboard,"attribution":attribution,"migrationCandidate":migration_candidate,"qualification":{"levelReached":max_level>=u64::from(minimum),"activeUtcDays":active_days,"status":attribution.as_ref().and_then(|a|a["status"].as_str()).unwrap_or("not_bound"),"qualifiedAtMs":attribution.as_ref().and_then(|a|a.get("qualifiedAtMs"))},"sourceGaps":super::production_gate::gaps(client,&self.environment,campaign_id)?,"settlements":settlements,"payoutSettings":payout,"appeals":appeals,"permissions":{"isLeader":leader.is_some(),"isAdmin":!operators.is_empty(),"roles":operators},"dataMode":self.data_mode(),"payoutExecutionEnabled":campaign_payout_enabled(client,&self.environment,campaign_id)?}),
            )
        })
    }

    pub fn members(
        &self,
        campaign_id: &str,
        account_id: &str,
        offset: u32,
        limit: u32,
        now_ms: u64,
    ) -> Result<Value, String> {
        if limit == 0 || limit > 100 || offset > 100_000 {
            return Err("promotionPaginationInvalid".into());
        }
        with_account_store_postgres_client(&self.database_url, |client| {
            let _ = account(client, account_id, false)?;
            let leader=db(client.query_opt("SELECT leader_id FROM promotion_leaders WHERE environment=$1 AND campaign_id=$2 AND account_id=$3",&[&self.environment,&campaign_id,&account_id]))?.ok_or("promotionLeaderMissing")?.get::<_,String>(0);
            let result = calculation(client, &self.environment, campaign_id, &leader)?;
            let rows=db(client.query("SELECT account_id,status,reason_code,confirmed_at_ms,qualified_at_ms FROM promotion_attributions WHERE environment=$1 AND campaign_id=$2 AND leader_id=$3 ORDER BY confirmed_at_ms,account_id LIMIT $4 OFFSET $5",&[&self.environment,&campaign_id,&leader,&i64::from(limit),&i64::from(offset)]))?;
            let items=rows.iter().map(|r| {
                let member:String=r.get(0); let masked=masked_member(&self.environment,campaign_id,&leader,&member);
                let estimate=result.members.iter().find(|m|m.account_id==member);
                json!({"memberRef":masked,"status":r.get::<_,String>(1),"reasonCode":r.get::<_,String>(2),"confirmedAtMs":positive(r.get(3)),"qualifiedAtMs":r.get::<_,Option<i64>>(4).map(positive),"rank":estimate.map(|m|m.rank),"rateBps":estimate.map(|m|m.rate_bps),"estimatedCommissionMicroUsd":estimate.map(|m|m.commission_micro_usd).unwrap_or(0).to_string()})
            }).collect::<Vec<_>>();
            Ok(
                json!({"items":items,"offset":offset,"limit":limit,"nextOffset":if rows.len()==limit as usize{Some(offset+limit)}else{None},"asOfMs":now_ms,"dataMode":self.data_mode()}),
            )
        })
    }

    pub fn settlements(&self, campaign_id: &str, account_id: &str) -> Result<Value, String> {
        with_account_store_postgres_client(&self.database_url, |client| {
            let _ = account(client, account_id, false)?;
            Ok(
                json!({"items":list_settlements(client,&self.environment,campaign_id,account_id)?,"payoutExecutionEnabled":campaign_payout_enabled(client,&self.environment,campaign_id)?}),
            )
        })
    }
    pub fn payout_settings(&self, account_id: &str) -> Result<Value, String> {
        with_account_store_postgres_client(&self.database_url, |client| {
            let _ = account(client, account_id, false)?;
            payout_settings(client, &self.environment, account_id)
        })
    }
    pub fn verify_payout_address(
        &self,
        account_id: &str,
        credential_id: &str,
        network: &str,
        idempotency: &str,
        now_ms: u64,
    ) -> Result<Value, String> {
        if !matches!(network, "sui:testnet" | "sui:mainnet") {
            return Err("promotionNetworkInvalid".into());
        }
        if (self.environment == "production") != (network == "sui:mainnet") {
            return Err("promotionEnvironmentMismatch".into());
        }
        self.mutation(account_id,idempotency,"payout.address.verify",json!([credential_id,network]),now_ms,|tx| {
            lock_payee(tx,&self.environment,account_id)?;
            let _=account(tx,account_id,true)?;
            let credential=db(tx.query_opt("SELECT credential_subject,credential_kind FROM identity_credentials WHERE account_id=$1 AND credential_id=$2 AND revoked_at_ms IS NULL FOR SHARE",&[&account_id,&credential_id]))?.ok_or("promotionWalletProofRequired")?;
            if !matches!(credential.get::<_,String>(1).as_str(),"sui_wallet"|"sui_passkey") { return Err("promotionWalletProofRequired".into()); }
            let address:String=credential.get(0);
            if address.len()!=66||!address.starts_with("0x")||!address[2..].bytes().all(|b|b.is_ascii_hexdigit()) { return Err("promotionWalletProofRequired".into()); }
            let latest=db(tx.query_opt("SELECT version,network,wallet_address,cooldown_until_ms,verified_credential_id FROM promotion_payout_addresses WHERE environment=$1 AND account_id=$2 ORDER BY version DESC LIMIT 1",&[&self.environment,&account_id]))?;
            if let Some(ref last)=latest { if last.get::<_,String>(1)==network&&last.get::<_,String>(2)==address&&last.get::<_,String>(4)==credential_id { return Ok(json!({"status":"verified","network":network,"address":address,"version":last.get::<_,i64>(0),"cooldownUntilMs":positive(last.get(3)),"payoutEnabled":false,"replayed":true})); } }
            let pending:bool=db(tx.query_one("SELECT EXISTS(SELECT 1 FROM promotion_payouts p JOIN promotion_settlements s USING(environment,settlement_id) JOIN promotion_leaders l USING(environment,campaign_id,leader_id) WHERE p.environment=$1 AND l.account_id=$2 AND p.status IN ('submitted','unknown'))",&[&self.environment,&account_id]))?.get(0);
            if pending { return Err("promotionPayoutUnknown".into()); }
            let version=latest.map(|r|r.get::<_,i64>(0)+1).unwrap_or(1); let cooldown=now_ms.checked_add(72*3_600_000).ok_or("promotionRequestInvalid")?;
            db(tx.execute("INSERT INTO promotion_payout_addresses(environment,account_id,version,network,wallet_address,verified_credential_id,verified_at_ms,cooldown_until_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8)",&[&self.environment,&account_id,&version,&network,&address,&credential_id,&signed(now_ms)?,&signed(cooldown)?]))?;
            db(tx.execute("UPDATE promotion_payouts p SET status='cancelled' FROM promotion_settlements s,promotion_leaders l WHERE p.environment=$1 AND p.environment=s.environment AND p.settlement_id=s.settlement_id AND s.environment=l.environment AND s.campaign_id=l.campaign_id AND s.leader_id=l.leader_id AND l.account_id=$2 AND p.status IN ('draft','awaiting_approval')",&[&self.environment,&account_id]))?;
            audit(tx,&self.environment,account_id,"payout.address.verified",&format!("{account_id}:{version}"),credential_id,json!({"network":network,"version":version,"cooldownUntilMs":cooldown}),now_ms)?;
            Ok(json!({"status":"verified","network":network,"address":address,"version":version,"cooldownUntilMs":cooldown,"payoutEnabled":false}))
        })
    }
    pub fn appeal(
        &self,
        campaign_id: &str,
        account_id: &str,
        category: &str,
        message: &str,
        idempotency: &str,
        now_ms: u64,
    ) -> Result<Value, String> {
        if !matches!(
            category,
            "attribution" | "qualification" | "revenue" | "settlement" | "wallet" | "other"
        ) || message.trim().len() < 5
            || message.len() > 4000
            || message
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            return Err("promotionAppealInvalid".into());
        }
        self.mutation(account_id,idempotency,"appeal.create",json!([campaign_id,category,message]),now_ms,|tx| {
            let _=campaign(tx,&self.environment,campaign_id,false)?;let _=account(tx,account_id,false)?;
            let count:i64=db(tx.query_one("SELECT count(*) FROM promotion_appeals WHERE environment=$1 AND campaign_id=$2 AND account_id=$3 AND status IN ('open','review')",&[&self.environment,&campaign_id,&account_id]))?.get(0);
            if count>=5 { return Err("promotionAppealLimit".into()); }
            let id=token("appeal_")?;
            db(tx.execute("INSERT INTO promotion_appeals(environment,appeal_id,campaign_id,account_id,category,message,created_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7)",&[&self.environment,&id,&campaign_id,&account_id,&category,&message,&signed(now_ms)?]))?;
            audit(tx,&self.environment,account_id,"appeal.created",&id,"user_submission",json!({"campaignId":campaign_id,"category":category}),now_ms)?;
            Ok(json!({"appealId":id,"status":"open"}))
        })
    }

    /// Call only after a durable authoritative combat/quest/save commit. A fact
    /// event ID must identify that commit, never a browser heartbeat or timer.
    pub fn record_game_fact(&self, fact: &TrustedGameFact, now_ms: u64) -> Result<Value, String> {
        self.record_game_fact_inner(fact, now_ms, false)
    }

    /// Source ownership is proved by the immutable row written in the same
    /// authoritative save transaction. Deleting a character later cannot erase it.
    pub fn record_committed_game_fact(
        &self,
        fact: &TrustedGameFact,
        now_ms: u64,
    ) -> Result<Value, String> {
        self.record_game_fact_inner(fact, now_ms, true)
    }

    fn record_game_fact_inner(
        &self,
        fact: &TrustedGameFact,
        now_ms: u64,
        committed: bool,
    ) -> Result<Value, String> {
        if !valid_id(&fact.source_event_id)
            || fact.evidence_ref.trim().is_empty()
            || fact.evidence_ref.len() > 512
            || fact.occurred_at_ms == 0
            || fact.occurred_at_ms > now_ms
            || !matches!(
                fact.source.as_str(),
                "level_commit" | "combat_commit" | "quest_commit"
            )
            || (fact.source == "level_commit") != fact.level.is_some()
        {
            return Err("promotionSourceEventInvalid".into());
        }
        if !matches!(
            (fact.source.as_str(), fact.kind.as_str()),
            ("level_commit", "levelCommitted")
                | ("combat_commit", "combatKill")
                | ("quest_commit", "questCompleted")
        ) {
            return Err("promotionSourceEventInvalid".into());
        }
        let payload_hash = hash(fact)?;
        with_account_store_postgres_client(&self.database_url, |client| {
            let mut tx = db(client.transaction())?;
            let event_lock = format!(
                "promotion.fact:{}:{}:{}",
                self.environment, fact.source, fact.source_event_id
            );
            db(tx.query_one(
                "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
                &[&event_lock],
            ))?;
            for attr in db(tx.query("SELECT campaign_id FROM promotion_attributions WHERE environment=$1 AND account_id=$2 ORDER BY campaign_id",&[&self.environment,&fact.account_id]))? {
                let _=campaign(&mut tx,&self.environment,&attr.get::<_,String>(0),true)?;
            }
            if let Some(row)=db(tx.query_opt("SELECT payload_hash FROM promotion_game_facts WHERE environment=$1 AND source=$2 AND source_event_id=$3",&[&self.environment,&fact.source,&fact.source_event_id]))? {
                if row.get::<_,String>(0)!=payload_hash { return Err("promotionSourceEventConflict".into()); }
                return Ok(json!({"replayed":true}));
            }
            if committed {
                let source = db(tx.query_opt("SELECT fact_json FROM promotion_game_source_events WHERE environment=$1 AND source_event_id=$2 AND source=$3 AND account_id=$4 AND character_index=$5 AND committed_at_ms<=$6 AND source_save_version>0", &[&self.environment,&fact.source_event_id,&fact.source,&fact.account_id,&fact.character_index,&signed(now_ms)?]))?
                    .ok_or("promotionCommittedSourceRequired")?;
                if source.get::<_, Value>(0)
                    != serde_json::to_value(fact).map_err(|_| "promotionSourceEventInvalid")?
                {
                    return Err("promotionSourceEventConflict".into());
                }
            } else {
                let _ = account(&mut tx, &fact.account_id, false)?;
                let char_row = db(tx.query_opt(
                    "SELECT level FROM characters WHERE account_id=$1 AND character_index=$2",
                    &[&fact.account_id, &fact.character_index],
                ))?
                .ok_or("promotionCharacterMissing")?;
                if fact
                    .level
                    .is_some_and(|v| v == 0 || i32::from(v) > char_row.get::<_, i32>(0))
                {
                    return Err("promotionSourceEventInvalid".into());
                }
            }
            db(tx.execute("INSERT INTO promotion_game_facts(environment,source,source_event_id,account_id,character_index,source_occurred_at_ms,utc_day,level,evidence_ref,payload_hash,kind) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",&[&self.environment,&fact.source,&fact.source_event_id,&fact.account_id,&fact.character_index,&signed(fact.occurred_at_ms)?,&signed(fact.occurred_at_ms/86_400_000)?,&fact.level.map(i32::from),&fact.evidence_ref,&payload_hash,&fact.kind]))?;
            audit(
                &mut tx,
                &self.environment,
                "game_authority",
                "source.game_fact",
                &format!("{}:{}", fact.source, fact.source_event_id),
                &fact.evidence_ref,
                json!({"accountId":fact.account_id,"occurredAtMs":fact.occurred_at_ms}),
                now_ms,
            )?;
            refresh_account_qualifications(&mut tx, &self.environment, &fact.account_id, now_ms)?;
            db(tx.commit())?;
            Ok(json!({"accepted":true,"replayed":false}))
        })
    }

    pub fn ingest_sandbox_monthly_revenue(
        &self,
        event: &SandboxMonthlyRevenue,
        now_ms: u64,
    ) -> Result<Value, String> {
        if self.environment == "production" || !event.sandbox {
            return Err("promotionFundingSourceUnavailable".into());
        }
        if !valid_id(&event.allocation_id)
            || event.evidence_ref.trim().is_empty()
            || event.fx_snapshot_ref.trim().is_empty()
            || event.original_currency != "usd"
            || event.paid_credits == 0
            || event.service_end_ms <= event.service_start_ms
            || event.service_end_ms - event.service_start_ms > 31 * 86_400_000
        {
            return Err("promotionRevenueSourceInvalid".into());
        }
        let payload_hash = hash(event)?;
        let result = with_account_store_postgres_client(&self.database_url, |client| {
            let mut tx = db(client.transaction())?;
            let _ = campaign(&mut tx, &self.environment, &event.campaign_id, true)?;
            let acc = account(&mut tx, &event.account_id, true)?;
            if let Some(row)=db(tx.query_opt("SELECT source_payload_hash FROM promotion_revenue_allocations WHERE environment=$1 AND allocation_id=$2",&[&self.environment,&event.allocation_id]))? {
                if row.get::<_,String>(0)!=payload_hash { return Err("promotionSourceEventConflict".into()); } return Ok(json!({"replayed":true}));
            }
            let saved: AccountRecord =
                serde_json::from_value(acc.get(0)).map_err(|_| "promotionAccountUnavailable")?;
            let order = saved
                .billing
                .as_ref()
                .and_then(|v| v.orders.get(&event.recharge_order_id))
                .ok_or("promotionPaymentEvidenceMissing")?;
            let Some(paid_at) = order.paid_at_ms else {
                return Err("promotionPaymentEvidenceMissing".into());
            };
            if order.livemode
                || order.offer.credits == 0
                || order.state != crate::billing::RechargeOrderState::Applied
                || order.applied.as_ref().is_none_or(|r| {
                    r.credits != order.offer.credits
                        || r.at_ms < paid_at
                        || r.character_index != order.character_index
                })
                || !order.review_events.is_empty()
                || order.payment_intent_id.is_none()
                || order.offer.currency != event.original_currency
                || order.offer.amount_minor != event.original_amount_minor
                || paid_at > now_ms
            {
                return Err("promotionPaymentEvidenceMissing".into());
            }
            let receipt = saved
                .monthly_card
                .as_ref()
                .and_then(|v| {
                    v.item_receipts
                        .values()
                        .find(|r| r.unique_id == event.monthly_item_uid)
                })
                .ok_or("promotionSubscriptionEvidenceMissing")?;
            let entitlement_at = receipt
                .redeemed_at_ms
                .ok_or("promotionSubscriptionEvidenceMissing")?;
            let service_end = receipt
                .credited_until_ms
                .ok_or("promotionSubscriptionEvidenceMissing")?;
            let service_start = service_end
                .checked_sub(crate::monthly_card::MONTHLY_CARD_DURATION_MS)
                .ok_or("promotionSubscriptionEvidenceMissing")?;
            if receipt.account_id != event.account_id
                || entitlement_at > now_ms
                || event.service_start_ms != service_start
                || event.service_end_ms != service_end
                || service_start < entitlement_at
                || paid_at > receipt.issued_at_ms
                || receipt.credit_price < event.paid_credits
            {
                return Err("promotionSubscriptionEvidenceMissing".into());
            }
            let (used_credits, used_amount): (i64, i64) = {
                let row=db(tx.query_one("SELECT COALESCE(sum(paid_credits),0)::bigint,COALESCE(sum(net_micro_usd),0)::bigint FROM promotion_credit_lot_allocations WHERE environment=$1 AND account_id=$2 AND recharge_order_id=$3",&[&self.environment,&event.account_id,&event.recharge_order_id]))?;
                (row.get(0), row.get(1))
            };
            let new_credits = positive(used_credits)
                .checked_add(u64::from(event.paid_credits))
                .ok_or("promotionAmountOverflow")?;
            let max_net = u128::from(order.offer.amount_minor) * 10_000 * u128::from(new_credits)
                / u128::from(order.offer.credits);
            if new_credits > u64::from(order.offer.credits)
                || u128::from(positive(used_amount)) + u128::from(event.eligible_micro_usd)
                    > max_net
            {
                return Err("promotionFundingAllocationExceeded".into());
            }
            let subscription_id = format!("monthly-item:{}", event.monthly_item_uid);
            db(tx.execute("INSERT INTO promotion_revenue_allocations(environment,allocation_id,campaign_id,account_id,source_order_id,subscription_id,paid_at_ms,service_start_ms,service_end_ms,eligible_micro_usd,original_currency,original_amount_minor,fx_snapshot_ref,source_evidence_ref,source_payload_hash,funding_status,reason_code,is_card_payment,entitlement_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,'verified_paid','sandbox_verified',TRUE,$16)",&[&self.environment,&event.allocation_id,&event.campaign_id,&event.account_id,&event.recharge_order_id,&subscription_id,&signed(paid_at)?,&signed(event.service_start_ms)?,&signed(event.service_end_ms)?,&signed(event.eligible_micro_usd)?,&event.original_currency,&signed(event.original_amount_minor)?,&event.fx_snapshot_ref,&event.evidence_ref,&payload_hash,&signed(entitlement_at)?]))?;
            db(tx.execute("INSERT INTO promotion_credit_lot_allocations(environment,allocation_id,account_id,recharge_order_id,paid_credits,net_micro_usd,source_evidence_ref) VALUES($1,$2,$3,$4,$5,$6,$7)",&[&self.environment,&event.allocation_id,&event.account_id,&event.recharge_order_id,&i64::from(event.paid_credits),&signed(event.eligible_micro_usd)?,&event.evidence_ref]))?;
            audit(
                &mut tx,
                &self.environment,
                "sandbox_payment_adapter",
                "source.revenue",
                &event.allocation_id,
                &event.evidence_ref,
                json!({"campaignId":event.campaign_id,"sandbox":true,"eligibleMicroUsd":event.eligible_micro_usd.to_string()}),
                now_ms,
            )?;
            refresh_account_qualifications(&mut tx, &self.environment, &event.account_id, now_ms)?;
            db(tx.commit())?;
            Ok(json!({"accepted":true,"sandbox":true,"replayed":false}))
        })?;
        let recovery = self.retry_pending_reversals(100, now_ms)?;
        Ok(
            json!({"accepted":true,"sandbox":true,"replayed":result["replayed"],"reversalRecovery":recovery}),
        )
    }

    /// Source adapters verify provider signatures and order reconciliation before
    /// calling this method. Reversals are cumulative across immutable events.
    pub fn ingest_reversal(
        &self,
        allocation_id: &str,
        source_event_id: &str,
        reversal_micro_usd: u64,
        evidence_ref: &str,
        occurred_at_ms: u64,
        now_ms: u64,
    ) -> Result<Value, String> {
        if !valid_id(source_event_id)
            || reversal_micro_usd == 0
            || evidence_ref.trim().is_empty()
            || occurred_at_ms > now_ms
        {
            return Err("promotionReversalInvalid".into());
        }
        with_account_store_postgres_client(&self.database_url, |client| {
            let mut tx = db(client.transaction())?;
            let event_lock = format!("promotion.reversal:{}:{source_event_id}", self.environment);
            db(tx.query_one(
                "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
                &[&event_lock],
            ))?;
            if let Some(pending)=db(tx.query_opt("SELECT allocation_id,reversal_micro_usd,evidence_ref,occurred_at_ms FROM promotion_pending_reversals WHERE environment=$1 AND source_event_id=$2",&[&self.environment,&source_event_id]))? {
                if pending.get::<_,String>(0)!=allocation_id||positive(pending.get(1))!=reversal_micro_usd||pending.get::<_,String>(2)!=evidence_ref||positive(pending.get(3))!=occurred_at_ms {return Err("promotionSourceEventConflict".into());}
            }
            let parent=db(tx.query_opt("SELECT campaign_id FROM promotion_revenue_allocations WHERE environment=$1 AND allocation_id=$2",&[&self.environment,&allocation_id]))?;
            let Some(parent) = parent else {
                db(tx.execute("INSERT INTO promotion_pending_reversals(environment,source_event_id,allocation_id,reversal_micro_usd,occurred_at_ms,evidence_ref) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING",&[&self.environment,&source_event_id,&allocation_id,&signed(reversal_micro_usd)?,&signed(occurred_at_ms)?,&evidence_ref]))?;
                audit(
                    &mut tx,
                    &self.environment,
                    "payment_adapter",
                    "source.reversal.pending",
                    source_event_id,
                    evidence_ref,
                    json!({"allocationId":allocation_id,"reasonCode":"source_allocation_pending"}),
                    now_ms,
                )?;
                db(tx.commit())?;
                return Ok(
                    json!({"accepted":true,"status":"pending_source","reasonCode":"source_allocation_pending"}),
                );
            };
            let _ = campaign(
                &mut tx,
                &self.environment,
                &parent.get::<_, String>(0),
                true,
            )?;
            let original=db(tx.query_opt("SELECT campaign_id,account_id,eligible_micro_usd FROM promotion_revenue_allocations WHERE environment=$1 AND allocation_id=$2 FOR UPDATE",&[&self.environment,&allocation_id]))?.ok_or("promotionAllocationMissing")?;
            let campaign_id: String = original.get(0);
            let account_id: String = original.get(1);
            if let Some(existing)=db(tx.query_opt("SELECT allocation_id,reversal_micro_usd,evidence_ref,occurred_at_ms FROM promotion_revenue_adjustments WHERE environment=$1 AND source_event_id=$2",&[&self.environment,&source_event_id]))? {
                if existing.get::<_,String>(0)!=allocation_id||positive(existing.get(1))!=reversal_micro_usd||existing.get::<_,String>(2)!=evidence_ref||positive(existing.get(3))!=occurred_at_ms {return Err("promotionSourceEventConflict".into());} return Ok(json!({"replayed":true}));
            }
            let prior:i64=db(tx.query_one("SELECT COALESCE(sum(reversal_micro_usd),0)::bigint FROM promotion_revenue_adjustments WHERE environment=$1 AND allocation_id=$2",&[&self.environment,&allocation_id]))?.get(0);
            if positive(prior)
                .checked_add(reversal_micro_usd)
                .is_none_or(|v| v > positive(original.get(2)))
            {
                return Err("promotionReversalExceeded".into());
            }
            let adjustment = token("rev_")?;
            db(tx.execute("INSERT INTO promotion_revenue_adjustments(environment,adjustment_id,allocation_id,source_event_id,reversal_micro_usd,occurred_at_ms,evidence_ref) VALUES($1,$2,$3,$4,$5,$6,$7)",&[&self.environment,&adjustment,&allocation_id,&source_event_id,&signed(reversal_micro_usd)?,&signed(occurred_at_ms)?,&evidence_ref]))?;
            db(tx.execute("UPDATE promotion_pending_reversals SET status='applied' WHERE environment=$1 AND source_event_id=$2",&[&self.environment,&source_event_id]))?;
            let settlement=db(tx.query_opt("SELECT s.settlement_id,s.snapshot_json,l.leader_id,s.snapshot_sha256 FROM promotion_settlements s JOIN promotion_leaders l USING(environment,campaign_id,leader_id) JOIN promotion_attributions a USING(environment,campaign_id,leader_id) WHERE s.environment=$1 AND s.campaign_id=$2 AND a.account_id=$3",&[&self.environment,&campaign_id,&account_id]))?;
            if let Some(snapshot) = settlement {
                let snapshot_json: Value = snapshot.get(1);
                if hash(&snapshot_json)? != snapshot.get::<_, String>(3) {
                    return Err("promotionSnapshotInvalid".into());
                }
                let input: CommissionInput = serde_json::from_value(snapshot_json["input"].clone())
                    .map_err(|_| "promotionSnapshotInvalid")?;
                let result: CommissionResult =
                    serde_json::from_value(snapshot_json["result"].clone())
                        .map_err(|_| "promotionSnapshotInvalid")?;
                if let Some(member) = result.members.iter().find(|m| m.account_id == account_id) {
                    let frozen = hydrate_frozen_basis(&input, member)?;
                    let ids = frozen
                        .allocation_basis
                        .iter()
                        .map(|b| b.allocation_id.clone())
                        .collect::<Vec<_>>();
                    let reversal_rows=db(tx.query("SELECT r.allocation_id,r.eligible_micro_usd,COALESCE(sum(a.reversal_micro_usd),0)::bigint FROM promotion_revenue_allocations r LEFT JOIN promotion_revenue_adjustments a USING(environment,allocation_id) WHERE r.environment=$1 AND r.campaign_id=$2 AND r.account_id=$3 AND r.allocation_id=ANY($4) GROUP BY r.allocation_id,r.eligible_micro_usd",&[&self.environment,&campaign_id,&account_id,&ids]))?;
                    if reversal_rows.len() != ids.len() {
                        return Err("promotionSnapshotInvalid".into());
                    }
                    let mut cumulative = BTreeMap::new();
                    for row in reversal_rows {
                        let id: String = row.get(0);
                        let basis = frozen
                            .allocation_basis
                            .iter()
                            .find(|b| b.allocation_id == id)
                            .ok_or("promotionSnapshotInvalid")?;
                        if basis.original_amount_micro_usd != positive(row.get(1)) {
                            return Err("promotionSnapshotInvalid".into());
                        }
                        cumulative.insert(id, positive(row.get(2)));
                    }
                    let prev:i64=db(tx.query_one("SELECT COALESCE(-sum(amount_micro_usd),0)::bigint FROM promotion_commission_entries WHERE environment=$1 AND settlement_id=$2 AND account_id=$3",&[&self.environment,&snapshot.get::<_,String>(0),&account_id]))?.get(0);
                    let delta = super::frozen_member_reversal_micro_usd(
                        &frozen,
                        &cumulative,
                        positive(prev),
                    )?;
                    if delta > 0 {
                        db(tx.execute("INSERT INTO promotion_commission_entries(environment,entry_id,campaign_id,leader_id,account_id,kind,amount_micro_usd,source_adjustment_id,settlement_id,occurred_at_ms) VALUES($1,$2,$3,$4,$5,'locked_reversal',$6,$7,$8,$9)",&[&self.environment,&token("entry_")?,&campaign_id,&snapshot.get::<_,String>(2),&account_id,&(-signed(delta)?),&adjustment,&snapshot.get::<_,String>(0),&signed(occurred_at_ms)?]))?;
                    }
                }
            } else {
                refresh_account_qualifications(&mut tx, &self.environment, &account_id, now_ms)?;
            }
            audit(
                &mut tx,
                &self.environment,
                "payment_adapter",
                "source.reversal",
                &adjustment,
                evidence_ref,
                json!({"allocationId":allocation_id,"amountMicroUsd":reversal_micro_usd.to_string()}),
                now_ms,
            )?;
            db(tx.commit())?;
            Ok(json!({"accepted":true,"replayed":false}))
        })
    }

    /// Recover refunds that arrived before a verified allocation. A worker can
    /// call this after import or restart; retries retain source-event uniqueness.
    pub fn retry_pending_reversals(&self, limit: u32, now_ms: u64) -> Result<Value, String> {
        if limit == 0 || limit > 100 {
            return Err("promotionPaginationInvalid".into());
        }
        let events = with_account_store_postgres_client(&self.database_url, |client| {
            db(client.query("SELECT p.allocation_id,p.source_event_id,p.reversal_micro_usd,p.evidence_ref,p.occurred_at_ms FROM promotion_pending_reversals p JOIN promotion_revenue_allocations r USING(environment,allocation_id) WHERE p.environment=$1 AND p.status='pending' ORDER BY p.occurred_at_ms,p.source_event_id LIMIT $2",&[&self.environment,&i64::from(limit)]))
        })?;
        let mut completed = 0;
        let mut failed = Vec::new();
        for row in events {
            let source: String = row.get(1);
            match self.ingest_reversal(
                &row.get::<_, String>(0),
                &source,
                positive(row.get(2)),
                &row.get::<_, String>(3),
                positive(row.get(4)),
                now_ms,
            ) {
                Ok(_) => completed += 1,
                Err(error) => failed.push(json!({"sourceEventId":source,"error":error})),
            }
        }
        Ok(json!({"completed":completed,"failures":failed}))
    }

    /// Process only internal promotion projections. Claim, projection writes and
    /// completion share one transaction; a process crash releases row locks and
    /// rolls the entire claim back. No payment/notification/alert is dispatched.
    pub fn process_pending_work(&self, limit: u32, now_ms: u64) -> Result<Value, String> {
        if limit == 0 || limit > 100 || now_ms == 0 {
            return Err("promotionWorkLimitInvalid".into());
        }
        with_account_store_postgres_client(&self.database_url, |client| {
            let mut tx = db(client.transaction())?;
            let rows=db(tx.query("SELECT outbox_id,event_type,resource_id,event_json,attempts FROM promotion_outbox WHERE environment=$1 AND status IN ('pending','failed') AND created_at_ms <= $2 AND (next_attempt_at_ms IS NULL OR next_attempt_at_ms <= $2) ORDER BY created_at_ms,outbox_id LIMIT $3 FOR UPDATE SKIP LOCKED",&[&self.environment,&signed(now_ms)?,&i64::from(limit)]))?;
            let mut claims = Vec::new();
            let mut groups: BTreeMap<String, BTreeSet<i64>> = BTreeMap::new();
            let mut errors: BTreeMap<i64, String> = BTreeMap::new();
            let mut event_campaigns: BTreeMap<i64, Vec<String>> = BTreeMap::new();
            for row in rows {
                let claim = OutboxClaim {
                    id: row.get(0),
                    event_type: row.get(1),
                    resource_id: row.get(2),
                    value: row.get(3),
                    attempts: row.get::<_, i32>(4).saturating_add(1),
                };
                db(tx.execute("UPDATE promotion_outbox SET status='processing',attempts=$3,last_attempt_at_ms=$4 WHERE environment=$1 AND outbox_id=$2",&[&self.environment,&claim.id,&claim.attempts,&signed(now_ms)?]))?;
                match savepoint(&mut tx, |inner| {
                    work_campaigns(inner, &self.environment, &claim)
                })? {
                    Ok(campaigns) => {
                        for campaign in &campaigns {
                            groups.entry(campaign.clone()).or_default().insert(claim.id);
                        }
                        event_campaigns.insert(claim.id, campaigns);
                    }
                    Err(error) => {
                        errors.insert(claim.id, error);
                    }
                }
                claims.push(claim);
            }
            let mut projections = BTreeMap::new();
            let mut inserted_runs = 0u64;
            for (campaign_id, events) in groups {
                match savepoint(&mut tx, |inner| {
                    reconcile_projection(inner, &self.environment, &campaign_id, now_ms)
                })? {
                    Ok(summary) => {
                        inserted_runs += summary["insertedRuns"].as_u64().unwrap_or(0);
                        projections.insert(campaign_id, summary);
                    }
                    Err(error) => {
                        for id in events {
                            errors.entry(id).or_insert_with(|| error.clone());
                        }
                    }
                }
            }
            let mut completed = 0u64;
            let mut failed = Vec::new();
            for claim in &claims {
                let campaigns = event_campaigns.get(&claim.id).cloned().unwrap_or_default();
                if let Some(error) = errors.get(&claim.id) {
                    let shift = (claim.attempts.saturating_sub(1) as u32).min(8);
                    let next = now_ms
                        .checked_add(1000u64 << shift)
                        .ok_or("promotionAmountOverflow")?;
                    let result = json!({"scope":"internal_promotion_projection","internalProjectionCompleted":false,"externalDispatchCompleted":false,"campaigns":campaigns,"error":error});
                    db(tx.execute("UPDATE promotion_outbox SET status='failed',last_error_code=$3,next_attempt_at_ms=$4,completed_at_ms=NULL,result_json=$5 WHERE environment=$1 AND outbox_id=$2",&[&self.environment,&claim.id,&error,&signed(next)?,&result]))?;
                    failed.push(json!({"outboxId":claim.id,"error":error,"nextAttemptAtMs":next}));
                } else {
                    let result = json!({"scope":"internal_promotion_projection","internalProjectionCompleted":true,"externalDispatchCompleted":false,"campaigns":campaigns});
                    db(tx.execute("UPDATE promotion_outbox SET status='completed',last_error_code=NULL,next_attempt_at_ms=NULL,completed_at_ms=$3,result_json=$4 WHERE environment=$1 AND outbox_id=$2",&[&self.environment,&claim.id,&signed(now_ms)?,&result]))?;
                    completed += 1;
                }
            }
            let health = queue_health(&mut tx, &self.environment, now_ms)?;
            let result = json!({"claimed":claims.len(),"completed":completed,"failed":failed.len(),"failures":failed,"insertedRuns":inserted_runs,"campaigns":projections,"queueHealth":health,"scope":"internal_promotion_projection","externalDispatchImplemented":false,"environment":self.environment,"dataMode":self.data_mode()});
            if !claims.is_empty() {
                // Audit the worker without enqueueing another worker event.
                db(tx.execute("INSERT INTO promotion_audit_log(environment,actor_id,action,resource_id,evidence_ref,summary_json,occurred_at_ms) VALUES($1,'promotion_worker','worker.internal_projection',$2,'transactional_outbox',$3,$4)",&[&self.environment,&format!("batch:{}:{now_ms}",claims[0].id),&result,&signed(now_ms)?]))?;
            }
            db(tx.commit())?;
            Ok(result)
        })
    }

    /// The gateway supplies only a verified identity's account. Roles are read
    /// from trusted PG grants; no request header or browser-selected role counts.
    pub fn admin(
        &self,
        campaign_id: &str,
        actor_account: &str,
        command: Value,
        idempotency: &str,
        now_ms: u64,
    ) -> Result<Value, String> {
        let parsed: AdminCommand =
            serde_json::from_value(command.clone()).map_err(|_| "promotionAdminCommandInvalid")?;
        if parsed.evidence().trim().is_empty() || parsed.evidence().len() > 512 {
            return Err("promotionEvidenceRequired".into());
        }
        // The journal owns its transaction and lock order; do not call it
        // from inside mutation's transaction (which could lock the same row).
        match &parsed {
            AdminCommand::ProposeLaunch {
                manifest,
                evidence_ref,
            } => {
                return self.propose_launch(
                    campaign_id,
                    actor_account,
                    manifest,
                    evidence_ref,
                    now_ms,
                )
            }
            AdminCommand::ApproveLaunch {
                manifest_sha256,
                evidence_ref,
            } => {
                return self.approve_launch(
                    campaign_id,
                    actor_account,
                    manifest_sha256,
                    evidence_ref,
                    now_ms,
                )
            }
            AdminCommand::FreezePayoutIntent {
                intent,
                evidence_ref,
            } => {
                if intent.campaign_id != campaign_id {
                    return Err("promotionPayoutIntentChanged".into());
                }
                return super::PromotionPayoutJournal::new(
                    self.database_url.clone(),
                    &self.environment,
                )?
                .freeze_intent(
                    actor_account,
                    intent,
                    evidence_ref,
                    std::env::var("MIR2_PROMOTION_ALLOW_MAINNET_PAYOUT").as_deref() == Ok("true"),
                    now_ms,
                );
            }
            AdminCommand::ApprovePayout {
                payout_id,
                intent_sha256,
                evidence_ref,
            } => {
                let sha = intent_sha256
                    .as_deref()
                    .ok_or("promotionPayoutIntentRequired")?;
                with_account_store_postgres_client(&self.database_url, |c| {
                    let exists:bool=db(c.query_one("SELECT EXISTS(SELECT 1 FROM promotion_payouts p JOIN promotion_settlements s USING(environment,settlement_id) WHERE p.environment=$1 AND p.payout_id=$2 AND s.campaign_id=$3)",&[&self.environment,&payout_id,&campaign_id]))?.get(0);
                    if !exists {
                        return Err("promotionPayoutMissing".into());
                    }
                    Ok(())
                })?;
                return super::PromotionPayoutJournal::new(
                    self.database_url.clone(),
                    &self.environment,
                )?
                .approve_intent(
                    actor_account,
                    payout_id,
                    sha,
                    evidence_ref,
                    now_ms,
                );
            }
            _ => {}
        }
        self.mutation(actor_account,idempotency,"admin.command",json!([campaign_id,command]),now_ms,|tx| {
            let _=account(tx,actor_account,false)?;
            let role=serde_json::to_value(parsed.role()).map_err(|_|"promotionPermissionDenied")?.as_str().ok_or("promotionPermissionDenied")?.to_owned();
            let allowed:bool=db(tx.query_one("SELECT EXISTS(SELECT 1 FROM promotion_operator_roles WHERE environment=$1 AND account_id=$2 AND role=$3 AND revoked_at_ms IS NULL)",&[&self.environment,&actor_account,&role]))?.get(0);
            if !allowed {return Err("promotionPermissionDenied".into());}
            let actor=PromotionAdminActor{subject:actor_account.into(),role:parsed.role()}; require_actor(&actor,parsed.role())?;
            let evidence=parsed.evidence().to_string();
            let mut result=match parsed {
                AdminCommand::ConfigureDraft{draft,..}=>{
                    draft.rules.validate_environment(&self.environment)?;
                    if draft.rules.test_wallet_enrollment&&self.environment!="test" {return Err("promotionEnvironmentMismatch".into());}
                    if !["budget_capped","revenue_share"].contains(&draft.funding_mode.as_str()) || (draft.funding_mode=="revenue_share"&&draft.budget_micro_usd!=0) {return Err("promotionFundingModeInvalid".into());}
                    if draft.campaign_id!=campaign_id||!valid_id(campaign_id)||!valid_id(&draft.terms_version)||draft.starts_at_ms==0||draft.ends_at_ms<=draft.starts_at_ms {return Err("promotionCampaignInvalid".into());}
                    let serialized=serde_json::to_value(&draft.rules).map_err(|_|"promotionRulesInvalid")?;
                    if let Ok(existing)=campaign(tx,&self.environment,campaign_id,true) {if existing.get::<_,String>(1)!="draft" {return Err("promotionCampaignImmutable".into());}}
                    db(tx.execute("INSERT INTO promotion_campaigns(environment,campaign_id,terms_version,starts_at_ms,ends_at_ms,rules_json,budget_micro_usd,member_limit,leader_limit,per_member_revenue_cap_micro_usd,created_at_ms,configured_by,per_leader_member_limit,funding_mode) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14) ON CONFLICT(environment,campaign_id) DO UPDATE SET terms_version=EXCLUDED.terms_version,starts_at_ms=EXCLUDED.starts_at_ms,ends_at_ms=EXCLUDED.ends_at_ms,rules_json=EXCLUDED.rules_json,budget_micro_usd=EXCLUDED.budget_micro_usd,member_limit=EXCLUDED.member_limit,leader_limit=EXCLUDED.leader_limit,per_member_revenue_cap_micro_usd=EXCLUDED.per_member_revenue_cap_micro_usd,per_leader_member_limit=EXCLUDED.per_leader_member_limit,configured_by=EXCLUDED.configured_by,funding_mode=EXCLUDED.funding_mode WHERE promotion_campaigns.status='draft'",&[&self.environment,&campaign_id,&draft.terms_version,&signed(draft.starts_at_ms)?,&signed(draft.ends_at_ms)?,&serialized,&signed(draft.budget_micro_usd)?,&signed(draft.member_limit)?,&signed(draft.leader_limit)?,&signed(draft.per_member_revenue_cap_micro_usd)?,&signed(now_ms)?,&actor_account,&signed(draft.per_leader_member_limit)?,&draft.funding_mode]))?;
                    json!({"status":"draft","rulesApproved":false,"recruitmentEnabled":false,"commissionEnabled":false,"payoutEnabled":campaign_payout_enabled(tx,&self.environment,campaign_id)?})
                }
                AdminCommand::ApproveRules{..}=>{
                    let row=campaign(tx,&self.environment,campaign_id,true)?;
                    if row.get::<_,String>(1)!="draft"||row.get::<_,String>(14)==actor_account {return Err("promotionDistinctReviewerRequired".into());}
                    db(tx.execute("UPDATE promotion_campaigns SET status='approved',rules_approved=TRUE,approved_by=$3 WHERE environment=$1 AND campaign_id=$2",&[&self.environment,&campaign_id,&actor_account]))?;
                    json!({"status":"approved","recruitmentEnabled":false,"commissionEnabled":false,"payoutEnabled":campaign_payout_enabled(tx,&self.environment,campaign_id)?})
                }
                AdminCommand::SetRecruitment{enabled,..}=>{
                    let row=campaign(tx,&self.environment,campaign_id,true)?;
                    if enabled&&((self.environment=="production"&&!super::production_gate::approved(tx,&self.environment,campaign_id)?)||!row.get::<_,bool>(5)||(row.get::<_,String>(17)=="budget_capped"&&(row.get::<_,i64>(8)<=0||row.get::<_,i64>(10)<=0||row.get::<_,i64>(11)<=0||row.get::<_,i64>(12)<=0||row.get::<_,i64>(16)<=0))||now_ms<positive(row.get(2))||now_ms>=positive(row.get(3))||matches!(row.get::<_,String>(1).as_str(),"closed"|"locked")) {return Err("promotionRecruitmentApprovalRequired".into());}
                    db(tx.execute("UPDATE promotion_campaigns SET recruitment_enabled=$3,status=CASE WHEN $3 THEN 'active' ELSE status END WHERE environment=$1 AND campaign_id=$2",&[&self.environment,&campaign_id,&enabled]))?;
                    json!({"recruitmentEnabled":enabled})
                }
                AdminCommand::SetCommission{enabled,..}=>{
                    let row=campaign(tx,&self.environment,campaign_id,true)?;
                    if enabled&&((self.environment=="production"&&!super::production_gate::approved(tx,&self.environment,campaign_id)?)||!row.get::<_,bool>(5)) {return Err("promotionCommissionApprovalRequired".into());}
                    db(tx.execute("UPDATE promotion_campaigns SET commission_enabled=$3 WHERE environment=$1 AND campaign_id=$2",&[&self.environment,&campaign_id,&enabled]))?;
                    json!({"commissionEnabled":enabled})
                }
                AdminCommand::SetPayout{enabled,..}=>{
                    let _=campaign(tx,&self.environment,campaign_id,true)?;
                    if enabled&&!super::production_gate::approved(tx,&self.environment,campaign_id)? {return Err("promotionProductionApprovalRequired".into());}
                    db(tx.execute("UPDATE promotion_campaigns SET payout_enabled=$3 WHERE environment=$1 AND campaign_id=$2",&[&self.environment,&campaign_id,&enabled]))?;
                    json!({"payoutEnabled":enabled})
                }
                AdminCommand::ProposeLaunch{..}|AdminCommand::ApproveLaunch{..}|AdminCommand::FreezePayoutIntent{..}|AdminCommand::ApprovePayout{..}=>return Err("promotionAdminCommandInvalid".into()),
                AdminCommand::ReviewLeader{leader_id,decision,reason,..}=>{
                    if !matches!(decision.as_str(),"approved"|"restricted"|"suspended"|"closed")||reason.trim().is_empty()||reason.len()>1000 {return Err("promotionReviewInvalid".into());}
                    let row=campaign(tx,&self.environment,campaign_id,true)?;
                    let target=db(tx.query_opt("SELECT status,account_id,display_character_index,accepted_terms_version FROM promotion_leaders WHERE environment=$1 AND campaign_id=$2 AND leader_id=$3 FOR UPDATE",&[&self.environment,&campaign_id,&leader_id]))?.ok_or("promotionLeaderMissing")?;
                    if decision=="approved" {
                        if target.get::<_,String>(3)!=row.get::<_,String>(0){return Err("promotionTermsChanged".into());}
                        let count:i64=db(tx.query_one("SELECT count(*) FROM promotion_leaders WHERE environment=$1 AND campaign_id=$2 AND status='approved'",&[&self.environment,&campaign_id]))?.get(0);
                        if target.get::<_,String>(0)!="approved"&&row.get::<_,i64>(11)>0&&count>=row.get::<_,i64>(11) {return Err("promotionLeaderLimit".into());}
                        let _=account(tx,&target.get::<_,String>(1),false)?;
                    }
                    let changed=db(tx.execute("UPDATE promotion_leaders SET status=$4 WHERE environment=$1 AND campaign_id=$2 AND leader_id=$3",&[&self.environment,&campaign_id,&leader_id,&decision]))?;
                    json!({"leaderId":leader_id,"status":decision,"reason":reason,"changed":changed})
                }
                AdminCommand::ExcludeMember{account_id,reason,..}=>{
                    if reason.trim().is_empty()||reason.len()>1000 {return Err("promotionReviewInvalid".into());}
                    let row=campaign(tx,&self.environment,campaign_id,true)?;
                    let locked:bool=db(tx.query_one("SELECT EXISTS(SELECT 1 FROM promotion_settlements s JOIN promotion_attributions a USING(environment,campaign_id,leader_id) WHERE s.environment=$1 AND s.campaign_id=$2 AND a.account_id=$3)",&[&self.environment,&campaign_id,&account_id]))?.get(0);
                    if locked {return Err("promotionSeasonLocked".into());}
                    let attr=db(tx.query_opt("SELECT reserved_micro_usd,status FROM promotion_attributions WHERE environment=$1 AND campaign_id=$2 AND account_id=$3 FOR UPDATE",&[&self.environment,&campaign_id,&account_id]))?.ok_or("promotionAttributionMissing")?;
                    if attr.get::<_,String>(1)!="excluded" {
                        let reserved=row.get::<_,i64>(9).checked_sub(attr.get::<_,i64>(0)).ok_or("promotionBudgetInvalid")?;
                        db(tx.execute("UPDATE promotion_campaigns SET reserved_micro_usd=$3 WHERE environment=$1 AND campaign_id=$2",&[&self.environment,&campaign_id,&reserved]))?;
                        db(tx.execute("UPDATE promotion_attributions SET status='excluded',reason_code='review_excluded',reserved_micro_usd=0 WHERE environment=$1 AND campaign_id=$2 AND account_id=$3",&[&self.environment,&campaign_id,&account_id]))?;
                    }
                    json!({"status":"excluded","reason":reason})
                }
                AdminCommand::Reconcile{..}=>{
                    let _=campaign(tx,&self.environment,campaign_id,true)?;
                    let members=db(tx.query("SELECT account_id FROM promotion_attributions WHERE environment=$1 AND campaign_id=$2",&[&self.environment,&campaign_id]))?;
                    for member in &members {refresh_account_qualifications(tx,&self.environment,&member.get::<_,String>(0),now_ms)?;}
                    let leaders=db(tx.query("SELECT leader_id FROM promotion_leaders WHERE environment=$1 AND campaign_id=$2",&[&self.environment,&campaign_id]))?;
                    for leader in &leaders {
                        let id:String=leader.get(0); let input=build_input(tx,&self.environment,campaign_id,&id)?; let result=configured_commission(tx,&input)?; let input_hash=commission_input_hash(&input,&result)?;
                        db(tx.execute("INSERT INTO promotion_commission_runs(environment,campaign_id,leader_id,input_sha256,input_json,result_json,created_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT DO NOTHING",&[&self.environment,&campaign_id,&id,&input_hash,&serde_json::to_value(&input).map_err(|_|"promotionSnapshotInvalid")?,&serde_json::to_value(&result).map_err(|_|"promotionSnapshotInvalid")?,&signed(now_ms)?]))?;
                    }
                    json!({"status":"reconciled","memberCount":members.len(),"leaderCount":leaders.len(),"sourceGaps":super::production_gate::gaps(tx,&self.environment,campaign_id)?})
                }
                AdminCommand::SetInputWatermark{watermark_ms,..}=>{
                    let row=campaign(tx,&self.environment,campaign_id,true)?;
                    if watermark_ms>now_ms||watermark_ms<positive(row.get(2))||row.get::<_,Option<i64>>(13).is_some_and(|v|positive(v)>watermark_ms) {return Err("promotionWatermarkInvalid".into());}
                    db(tx.execute("UPDATE promotion_campaigns SET input_watermark_ms=$3 WHERE environment=$1 AND campaign_id=$2",&[&self.environment,&campaign_id,&signed(watermark_ms)?]))?;
                    json!({"inputWatermarkMs":watermark_ms,"evidenceRef":evidence})
                }
                AdminCommand::CreateSettlement{leader_id,..}=>{
                    let row=campaign(tx,&self.environment,campaign_id,true)?;
                    if !row.get::<_,bool>(7)||now_ms<positive(row.get(3))||row.get::<_,Option<i64>>(13).is_none_or(|w|w<row.get::<_,i64>(3)) {return Err("promotionReconciliationIncomplete".into());}
                    let pending:bool=db(tx.query_one("SELECT EXISTS(SELECT 1 FROM promotion_pending_reversals WHERE environment=$1 AND status='pending')",&[&self.environment]))?.get(0);
                    if pending{return Err("promotionReconciliationIncomplete".into());}
                    ensure_no_source_holds(tx,&self.environment,campaign_id,&leader_id)?;
                    let input=build_input(tx,&self.environment,campaign_id,&leader_id)?; let result=configured_commission(tx,&input)?;
                    let snapshot=json!({"input":input,"result":result,"inputWatermarkMs":row.get::<_,Option<i64>>(13),"evidenceRef":evidence});
                    let snapshot_hash=hash(&snapshot)?; let id=token("settle_")?;
                    db(tx.execute("INSERT INTO promotion_settlements(environment,campaign_id,settlement_id,leader_id,algorithm_version,snapshot_json,snapshot_sha256,total_micro_usd,payable_cents,reserve_micro_usd,created_by,created_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",&[&self.environment,&campaign_id,&id,&leader_id,&result.algorithm_version,&snapshot,&snapshot_hash,&signed(result.commission_micro_usd)?,&signed(result.payable_cents)?,&signed(result.reserve_micro_usd)?,&actor_account,&signed(now_ms)?]))?;
                    db(tx.execute("UPDATE promotion_campaigns SET recruitment_enabled=FALSE,status='locked' WHERE environment=$1 AND campaign_id=$2",&[&self.environment,&campaign_id]))?;
                    json!({"settlementId":id,"status":"review","snapshotSha256":snapshot_hash,"totalMicroUsd":result.commission_micro_usd.to_string(),"payableCents":result.payable_cents,"payableMicroUsd":result.payable_micro_usd.unwrap_or(result.payable_cents*10_000).to_string(),"payoutPrecision":if result.payable_micro_usd.is_some(){6}else{2},"payoutEnabled":campaign_payout_enabled(tx,&self.environment,campaign_id)?})
                }
                AdminCommand::ApproveSettlement{settlement_id,snapshot_sha256,..}=>{
                    let row=db(tx.query_opt("SELECT created_by,status,snapshot_sha256,leader_id FROM promotion_settlements WHERE environment=$1 AND campaign_id=$2 AND settlement_id=$3 FOR UPDATE",&[&self.environment,&campaign_id,&settlement_id]))?.ok_or("promotionSettlementMissing")?;
                    ensure_no_source_holds(tx,&self.environment,campaign_id,&row.get::<_,String>(3))?;
                    if row.get::<_,String>(0)==actor_account {return Err("promotionDistinctReviewerRequired".into());}
                    if row.get::<_,String>(2)!=snapshot_sha256 {return Err("promotionSnapshotChanged".into());}
                    if row.get::<_,String>(1)!="review" {return Err("promotionSettlementStateInvalid".into());}
                    db(tx.execute("UPDATE promotion_settlements SET status='approved',reviewed_by=$3,reviewed_at_ms=$4 WHERE environment=$1 AND settlement_id=$2",&[&self.environment,&settlement_id,&actor_account,&signed(now_ms)?]))?;
                    json!({"settlementId":settlement_id,"status":"approved","payoutEnabled":campaign_payout_enabled(tx,&self.environment,campaign_id)?})
                }
                AdminCommand::CreatePayoutDraft{settlement_id,network,coin_type,..}=>{
                    if (self.environment=="production"&&(!super::production_gate::approved(tx,&self.environment,campaign_id)?||network!="sui:mainnet"))||(self.environment!="production"&&network!="sui:testnet")||coin_type.trim().is_empty()||coin_type.len()>256 {return Err("promotionPayoutApprovalRequired".into());}
                    let payee:String=db(tx.query_opt("SELECT l.account_id FROM promotion_settlements s JOIN promotion_leaders l USING(environment,campaign_id,leader_id) WHERE s.environment=$1 AND s.campaign_id=$2 AND s.settlement_id=$3",&[&self.environment,&campaign_id,&settlement_id]))?.ok_or("promotionSettlementMissing")?.get(0);
                    lock_payee(tx,&self.environment,&payee)?;
                    let row=db(tx.query_opt("SELECT s.status,s.payable_cents,l.account_id,s.snapshot_json,s.total_micro_usd,s.reserve_micro_usd FROM promotion_settlements s JOIN promotion_leaders l USING(environment,campaign_id,leader_id) WHERE s.environment=$1 AND s.campaign_id=$2 AND s.settlement_id=$3 FOR UPDATE OF s",&[&self.environment,&campaign_id,&settlement_id]))?.ok_or("promotionSettlementMissing")?;
                    if row.get::<_,String>(0)!="approved" {return Err("promotionSettlementStateInvalid".into());}
                    let base=settlement_payable_micro_usd(tx,&self.environment,campaign_id,&row.get::<_,Value>(3),row.get(4),row.get(5),row.get(1))?;
                    let campaign_row=campaign(tx,&self.environment,campaign_id,false)?;
                    if super::production_gate::test_micro_enabled(&self.environment,&campaign_row.get::<_,Value>(4))? {
                        if network!="sui:testnet"||coin_type!=super::TESTNET_USDC_COIN_TYPE||!super::production_gate::approved(tx,&self.environment,campaign_id)? {return Err("promotionPayoutManifestMismatch".into());}
                        let manifest:Value=db(tx.query_one("SELECT manifest_json FROM promotion_production_approvals WHERE environment=$1 AND campaign_id=$2 AND approved_by IS NOT NULL",&[&self.environment,&campaign_id]))?.get(0);
                        super::production_gate::test_micro_manifest(&self.environment,&campaign_row.get::<_,Value>(4),&manifest)?;
                    }
                    let address=db(tx.query_opt("SELECT version,network,wallet_address,cooldown_until_ms,verified_credential_id FROM promotion_payout_addresses WHERE environment=$1 AND account_id=$2 ORDER BY version DESC LIMIT 1",&[&self.environment,&row.get::<_,String>(2)]))?.ok_or("promotionWalletProofRequired")?;
                    if address.get::<_,String>(1)!=network||(now_ms<positive(address.get(3))&&!super::production_gate::first_address_exempt(tx,&self.environment,campaign_id,address.get(0))?) {return Err("promotionWalletCooldown".into());}
                    let reversals:i64=db(tx.query_one("SELECT COALESCE(-sum(amount_micro_usd),0)::bigint FROM promotion_commission_entries WHERE environment=$1 AND settlement_id=$2",&[&self.environment,&settlement_id]))?.get(0);
                    let amount=base.checked_sub(reversals).ok_or("promotionAmountOverflow")?;
                    if amount<=0 {return Err("promotionNoPayableBalance".into());}
                    let id=token("payout_")?;
                    db(tx.execute("INSERT INTO promotion_payouts(environment,payout_id,settlement_id,network,coin_type,destination,address_version,amount_micro_usd,created_by,created_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",&[&self.environment,&id,&settlement_id,&network,&coin_type,&address.get::<_,String>(2),&address.get::<_,i64>(0),&amount,&actor_account,&signed(now_ms)?]))?;
                    json!({"payoutId":id,"status":"draft","amountMicroUsd":amount.to_string(),"payoutEnabled":campaign_payout_enabled(tx,&self.environment,campaign_id)?,"executionAvailable":false})
                }
                AdminCommand::SubmitPayout{payout_id}=>{let _=payout_id;return Err("promotionPayoutSignedEnvelopeRequired".into());}
            };
            result.as_object_mut().ok_or("promotionAdminCommandInvalid")?.insert("campaignId".into(),json!(campaign_id));
            audit(tx,&self.environment,actor_account,"admin.command",&format!("{campaign_id}:{idempotency}"),&evidence,result.clone(),now_ms)?;
            Ok(result)
        })
    }

    pub fn admin_view(
        &self,
        campaign_id: &str,
        actor_account: &str,
        now_ms: u64,
    ) -> Result<Value, String> {
        with_account_store_postgres_client(&self.database_url, |client| {
            let mut tx = db(client.transaction())?;
            let _ = account(&mut tx, actor_account, false)?;
            let roles=db(tx.query("SELECT role FROM promotion_operator_roles WHERE environment=$1 AND account_id=$2 AND revoked_at_ms IS NULL",&[&self.environment,&actor_account]))?.iter().map(|r|r.get::<_,String>(0)).collect::<Vec<_>>();
            if roles.is_empty() {
                return Err("promotionPermissionDenied".into());
            }
            let config=campaign(&mut tx,&self.environment,campaign_id,false).ok().map(|r|json!({"status":r.get::<_,String>(1),"termsVersion":r.get::<_,String>(0),"rulesApproved":r.get::<_,bool>(5),"recruitmentEnabled":r.get::<_,bool>(6),"commissionEnabled":r.get::<_,bool>(7),"fundingMode":r.get::<_,String>(17),"budgetMicroUsd":r.get::<_,i64>(8).to_string(),"reservedMicroUsd":r.get::<_,i64>(9).to_string(),"memberLimit":r.get::<_,i64>(10),"leaderLimit":r.get::<_,i64>(11),"inputWatermarkMs":r.get::<_,Option<i64>>(13)}));
            let leaders=db(tx.query("SELECT leader_id,display_name,status,applied_at_ms FROM promotion_leaders WHERE environment=$1 AND campaign_id=$2 ORDER BY applied_at_ms DESC LIMIT 100",&[&self.environment,&campaign_id]))?.iter().map(|r|json!({"leaderId":r.get::<_,String>(0),"displayName":r.get::<_,String>(1),"status":r.get::<_,String>(2),"appliedAtMs":positive(r.get(3))})).collect::<Vec<_>>();
            let settlements=db(tx.query("SELECT settlement_id,leader_id,status,snapshot_sha256,total_micro_usd,created_by FROM promotion_settlements WHERE environment=$1 AND campaign_id=$2 ORDER BY created_at_ms DESC LIMIT 100",&[&self.environment,&campaign_id]))?.iter().map(|r|json!({"settlementId":r.get::<_,String>(0),"leaderId":r.get::<_,String>(1),"status":r.get::<_,String>(2),"snapshotSha256":r.get::<_,String>(3),"totalMicroUsd":r.get::<_,i64>(4).to_string(),"createdBy":r.get::<_,String>(5)})).collect::<Vec<_>>();
            let health = queue_health(&mut tx, &self.environment, now_ms)?;
            audit(
                &mut tx,
                &self.environment,
                actor_account,
                "admin.read",
                &format!("{campaign_id}:{now_ms}"),
                "authorized_dashboard",
                json!({"campaignId":campaign_id,"roles":roles}),
                now_ms,
            )?;
            let source_gaps =
                super::production_gate::gaps(&mut tx, &self.environment, campaign_id)?;
            let launch_configuration =
                super::production_gate::configuration(&mut tx, &self.environment, campaign_id).ok();
            let launch_configuration_sha256 =
                launch_configuration.as_ref().map(hash).transpose()?;
            let launch_approval=db(tx.query_opt("SELECT manifest_sha256,manifest_json,proposed_by,approved_by FROM promotion_production_approvals WHERE environment=$1 AND campaign_id=$2",&[&self.environment,&campaign_id]))?.map(|r|json!({"manifestSha256":r.get::<_,String>(0),"manifest":r.get::<_,Value>(1),"proposedBy":r.get::<_,String>(2),"approvedBy":r.get::<_,Option<String>>(3)}));
            let payout_enabled=db(tx.query_opt("SELECT payout_enabled FROM promotion_campaigns WHERE environment=$1 AND campaign_id=$2",&[&self.environment,&campaign_id]))?.is_some_and(|r|r.get::<_,bool>(0));
            db(tx.commit())?;
            Ok(
                json!({"campaign":config,"roles":roles,"leaders":leaders,"settlements":settlements,"queueHealth":health,"payoutEnabled":payout_enabled,"sourceGaps":source_gaps,"launchConfiguration":launch_configuration,"launchConfigurationSha256":launch_configuration_sha256,"launchApproval":launch_approval,"environment":self.environment,"dataMode":self.data_mode()}),
            )
        })
    }

    /// Finance/audit-only reproducible input export. Player member pages never
    /// receive this account-level source data. Every export is audited.
    pub fn settlement_export(
        &self,
        campaign_id: &str,
        actor_account: &str,
        settlement_id: &str,
        now_ms: u64,
    ) -> Result<Value, String> {
        with_account_store_postgres_client(&self.database_url, |client| {
            let mut tx = db(client.transaction())?;
            let _ = account(&mut tx, actor_account, false)?;
            let allowed:bool=db(tx.query_one("SELECT EXISTS(SELECT 1 FROM promotion_operator_roles WHERE environment=$1 AND account_id=$2 AND role IN ('finance','auditor') AND revoked_at_ms IS NULL)",&[&self.environment,&actor_account]))?.get(0);
            if !allowed {
                return Err("promotionPermissionDenied".into());
            }
            let row=db(tx.query_opt("SELECT snapshot_json,snapshot_sha256,status,algorithm_version FROM promotion_settlements WHERE environment=$1 AND campaign_id=$2 AND settlement_id=$3",&[&self.environment,&campaign_id,&settlement_id]))?.ok_or("promotionSettlementMissing")?;
            let adjustments=db(tx.query("SELECT entry_id,amount_micro_usd,source_adjustment_id,occurred_at_ms FROM promotion_commission_entries WHERE environment=$1 AND settlement_id=$2 ORDER BY occurred_at_ms,entry_id",&[&self.environment,&settlement_id]))?.iter().map(|r|json!({"entryId":r.get::<_,String>(0),"amountMicroUsd":r.get::<_,i64>(1).to_string(),"sourceAdjustmentId":r.get::<_,String>(2),"occurredAtMs":positive(r.get(3))})).collect::<Vec<_>>();
            audit(
                &mut tx,
                &self.environment,
                actor_account,
                "settlement.export",
                &format!("{settlement_id}:{now_ms}"),
                "authorized_finance_export",
                json!({"settlementId":settlement_id,"snapshotSha256":row.get::<_,String>(1)}),
                now_ms,
            )?;
            let canonical = canonical_json(&row.get::<_, Value>(0))?;
            if hex(&Sha256::digest(canonical.as_bytes())) != row.get::<_, String>(1) {
                return Err("promotionSnapshotInvalid".into());
            }
            db(tx.commit())?;
            Ok(
                json!({"environment":self.environment,"campaignId":campaign_id,"settlementId":settlement_id,"snapshotCanonicalJson":canonical,"snapshotSha256":row.get::<_,String>(1),"status":row.get::<_,String>(2),"algorithmVersion":row.get::<_,String>(3),"adjustments":adjustments}),
            )
        })
    }
}

struct OutboxClaim {
    id: i64,
    event_type: String,
    resource_id: String,
    value: Value,
    attempts: i32,
}

fn hydrate_frozen_basis(
    input: &CommissionInput,
    member: &super::MemberCommission,
) -> Result<super::MemberCommission, String> {
    let mut frozen = member.clone();
    if !frozen.allocation_basis.is_empty() {
        return Ok(frozen);
    }
    // Legacy draft snapshots can be reconstructed only from their own frozen
    // inputs. Current income must never enter this compatibility path.
    let mut single = input.clone();
    single.members.retain(|m| m.account_id == member.account_id);
    for original in &mut single.members {
        original.established_qualification_at_ms = None;
        for source in &mut original.revenue {
            if source.entitlement_at_ms == 0 {
                source.entitlement_at_ms = member.qualified_at_ms;
            }
        }
    }
    let result = calculate_commission(&single)?;
    let original = result
        .members
        .iter()
        .find(|m| m.account_id == member.account_id)
        .ok_or("promotionSnapshotInvalid")?;
    if original.eligible_revenue_micro_usd != member.eligible_revenue_micro_usd {
        return Err("promotionSnapshotInvalid".into());
    }
    frozen.allocation_basis = original.allocation_basis.clone();
    Ok(frozen)
}

fn savepoint<T>(
    tx: &mut Transaction<'_>,
    action: impl FnOnce(&mut Transaction<'_>) -> Result<T, String>,
) -> Result<Result<T, String>, String> {
    let mut nested = db(tx.transaction())?;
    match action(&mut nested) {
        Ok(value) => {
            db(nested.commit())?;
            Ok(Ok(value))
        }
        Err(error) => {
            db(nested.rollback())?;
            Ok(Err(error))
        }
    }
}

fn work_campaigns(
    tx: &mut Transaction<'_>,
    env: &str,
    event: &OutboxClaim,
) -> Result<Vec<String>, String> {
    let rows=match event.event_type.as_str() {
        "source.game_fact"=>{
            let fact=db(tx.query_opt("SELECT account_id FROM promotion_game_facts WHERE environment=$1 AND source||':'||source_event_id=$2",&[&env,&event.resource_id]))?.ok_or("promotionWorkSourceMissing")?;
            db(tx.query("SELECT campaign_id FROM promotion_attributions WHERE environment=$1 AND account_id=$2 ORDER BY campaign_id",&[&env,&fact.get::<_,String>(0)]))?
        },
        "source.revenue"=>db(tx.query("SELECT campaign_id FROM promotion_revenue_allocations WHERE environment=$1 AND allocation_id=$2",&[&env,&event.resource_id]))?,
        "source.reversal"=>db(tx.query("SELECT r.campaign_id FROM promotion_revenue_adjustments a JOIN promotion_revenue_allocations r USING(environment,allocation_id) WHERE a.environment=$1 AND a.adjustment_id=$2",&[&env,&event.resource_id]))?,
        "source.reversal.pending"=>{
            let source=db(tx.query_opt("SELECT status FROM promotion_pending_reversals WHERE environment=$1 AND source_event_id=$2",&[&env,&event.resource_id]))?.ok_or("promotionWorkSourceMissing")?;
            if source.get::<_,String>(0)!="applied" {return Err("promotionSourceReversalPending".into());}
            db(tx.query("SELECT r.campaign_id FROM promotion_revenue_adjustments a JOIN promotion_revenue_allocations r USING(environment,allocation_id) WHERE a.environment=$1 AND a.source_event_id=$2",&[&env,&event.resource_id]))?
        },
        "leader.applied"|"leader.test_wallet_enrolled"=>db(tx.query("SELECT campaign_id FROM promotion_leaders WHERE environment=$1 AND leader_id=$2",&[&env,&event.resource_id]))?,
        "attribution.candidate"=>db(tx.query("SELECT campaign_id FROM promotion_candidates WHERE environment=$1 AND candidate_id=$2",&[&env,&event.resource_id]))?,
        "appeal.created"=>db(tx.query("SELECT campaign_id FROM promotion_appeals WHERE environment=$1 AND appeal_id=$2",&[&env,&event.resource_id]))?,
        "settlement.export"=>{
            let settlement=event.value["settlementId"].as_str().ok_or("promotionWorkSourceMissing")?;
            db(tx.query("SELECT campaign_id FROM promotion_settlements WHERE environment=$1 AND settlement_id=$2",&[&env,&settlement]))?
        },
        "attribution.confirmed"|"qualification.updated"|"admin.command"|"admin.read"|"source.revenue_observation"|"migration.prepared"|"migration.confirmed"=>{
            if let Some(campaign)=event.value["campaignId"].as_str() {
                db(tx.query("SELECT campaign_id FROM promotion_campaigns WHERE environment=$1 AND campaign_id=$2",&[&env,&campaign]))?
            } else if event.event_type=="attribution.confirmed" {
                db(tx.query("SELECT campaign_id FROM promotion_attributions WHERE environment=$1 AND campaign_id||':'||account_id=$2",&[&env,&event.resource_id]))?
            } else if event.event_type=="qualification.updated" {
                let resource=event.resource_id.rsplit_once(':').map(|(left,_)|left).ok_or("promotionWorkSourceMissing")?;
                db(tx.query("SELECT campaign_id FROM promotion_attributions WHERE environment=$1 AND campaign_id||':'||account_id=$2",&[&env,&resource]))?
            } else {
                db(tx.query("SELECT campaign_id FROM promotion_campaigns WHERE environment=$1 AND starts_with($2,campaign_id||':')",&[&env,&event.resource_id]))?
            }
        },
        // An address-change notification needs a separately authorized delivery
        // adapter. Never label that obligation, payment sync, or an alert sent.
        _=>return Err("promotionWorkExternalAdapterRequired".into()),
    };
    let unique = rows
        .iter()
        .map(|row| row.get::<_, String>(0))
        .collect::<BTreeSet<_>>();
    if unique.is_empty()
        && event.event_type != "source.game_fact"
        && event.event_type != "admin.read"
    {
        return Err("promotionWorkSourceMissing".into());
    }
    if unique.len() > 1 && event.event_type != "source.game_fact" {
        return Err("promotionWorkCampaignAmbiguous".into());
    }
    Ok(unique.into_iter().collect())
}

pub(super) struct AdmissionCapacity {
    pub(super) available: bool,
    pub(super) reason: &'static str,
    pub(super) reserve_micro_usd: u64,
    pub(super) total_reserved_micro_usd: u64,
}
fn worst_case_reserve(cap: u64) -> Result<u64, String> {
    cap.checked_mul(3000)
        .ok_or("promotionAmountOverflow")?
        .div_ceil(100_000_000)
        .checked_mul(10_000)
        .ok_or("promotionAmountOverflow".into())
}
pub(super) fn admission_capacity<T: GenericClient>(
    client: &mut T,
    env: &str,
    campaign_id: &str,
    leader: &str,
    row: &postgres::Row,
    now_ms: u64,
) -> Result<AdmissionCapacity, String> {
    let revenue_share = row.get::<_, String>(17) == "revenue_share";
    let reserve = if revenue_share {
        0
    } else {
        worst_case_reserve(positive(row.get(12)))?
    };
    let total = positive(row.get::<_, i64>(9))
        .checked_add(reserve)
        .ok_or("promotionAmountOverflow")?;
    let count: i64 = db(client.query_one(
        "SELECT count(*) FROM promotion_attributions WHERE environment=$1 AND campaign_id=$2",
        &[&env, &campaign_id],
    ))?
    .get(0);
    let leader_count:i64=db(client.query_one("SELECT count(*) FROM promotion_attributions WHERE environment=$1 AND campaign_id=$2 AND leader_id=$3",&[&env,&campaign_id,&leader]))?.get(0);
    let reason = if now_ms >= positive(row.get(3)) {
        "campaign_ended"
    } else if now_ms < positive(row.get(2)) {
        "campaign_not_started"
    } else if !active(row, now_ms) || !row.get::<_, bool>(5) || !row.get::<_, bool>(6) {
        "recruitment_closed"
    } else if !revenue_share && (reserve == 0 || total > positive(row.get(8))) {
        "budget_full"
    } else if row.get::<_, i64>(10) > 0 && count >= row.get::<_, i64>(10) {
        "member_limit"
    } else if row.get::<_, i64>(16) > 0 && leader_count >= row.get::<_, i64>(16) {
        "leader_member_limit"
    } else {
        "available"
    };
    Ok(AdmissionCapacity {
        available: reason == "available",
        reason,
        reserve_micro_usd: reserve,
        total_reserved_micro_usd: total,
    })
}

fn verify_budget(
    tx: &mut Transaction<'_>,
    env: &str,
    campaign_id: &str,
    row: &postgres::Row,
) -> Result<(u64, u64), String> {
    let total=db(tx.query_one("SELECT COALESCE(sum(reserved_micro_usd),0)::bigint,count(*) FROM promotion_attributions WHERE environment=$1 AND campaign_id=$2",&[&env,&campaign_id]))?;
    let reserved = positive(total.get(0));
    let members = positive(total.get(1));
    if reserved != positive(row.get(9)) || reserved > positive(row.get(8)) {
        return Err("promotionBudgetReservationMismatch".into());
    }
    if row.get::<_, i64>(10) > 0 && members > positive(row.get(10)) {
        return Err("promotionMemberLimitExceeded".into());
    }
    let expected = if row.get::<_, String>(17) == "revenue_share" {
        0
    } else {
        worst_case_reserve(positive(row.get(12)))?
    };
    let invalid:bool=db(tx.query_one("SELECT EXISTS(SELECT 1 FROM promotion_attributions WHERE environment=$1 AND campaign_id=$2 AND ((status='excluded' AND reserved_micro_usd<>0) OR (status<>'excluded' AND reserved_micro_usd<>$3)))",&[&env,&campaign_id,&signed(expected)?]))?.get(0);
    if invalid {
        return Err("promotionBudgetReservationMismatch".into());
    }
    let over_leader:bool=db(tx.query_one("SELECT EXISTS(SELECT 1 FROM promotion_attributions WHERE environment=$1 AND campaign_id=$2 GROUP BY leader_id HAVING count(*)>$3)",&[&env,&campaign_id,&row.get::<_,i64>(16)]))?.get(0);
    if over_leader && row.get::<_, i64>(16) > 0 {
        return Err("promotionLeaderMemberLimitExceeded".into());
    }
    Ok((reserved, members))
}

fn reconcile_projection(
    tx: &mut Transaction<'_>,
    env: &str,
    campaign_id: &str,
    now_ms: u64,
) -> Result<Value, String> {
    let row = campaign(tx, env, campaign_id, true)?;
    let (reserved, members) = verify_budget(tx, env, campaign_id, &row)?;
    let leaders=db(tx.query("SELECT leader_id FROM promotion_leaders WHERE environment=$1 AND campaign_id=$2 ORDER BY leader_id",&[&env,&campaign_id]))?;
    let mut inserted = 0u64;
    for leader in &leaders {
        let id: String = leader.get(0);
        let locked=db(tx.query_opt("SELECT snapshot_json FROM promotion_settlements WHERE environment=$1 AND campaign_id=$2 AND leader_id=$3",&[&env,&campaign_id,&id]))?;
        let (input, result) = if let Some(locked) = locked {
            let snapshot: Value = locked.get(0);
            (
                serde_json::from_value::<CommissionInput>(snapshot["input"].clone())
                    .map_err(|_| "promotionSnapshotInvalid")?,
                serde_json::from_value::<CommissionResult>(snapshot["result"].clone())
                    .map_err(|_| "promotionSnapshotInvalid")?,
            )
        } else {
            let mut input = build_input(tx, env, campaign_id, &id)?;
            let result = configured_commission(tx, &input)?;
            for member in &mut input.members {
                if member.excluded {
                    continue;
                }
                let held:bool=db(tx.query_one("SELECT EXISTS(SELECT 1 FROM promotion_revenue_source_holds WHERE environment=$1 AND campaign_id=$2 AND account_id=$3 AND active)",&[&env,&campaign_id,&member.account_id]))?.get(0);
                if held {
                    continue;
                }
                let qualified = result
                    .members
                    .iter()
                    .find(|v| v.account_id == member.account_id);
                let status = if qualified.is_some() {
                    "qualified"
                } else {
                    "bound"
                };
                let reason = if qualified.is_some() {
                    "requirements_complete"
                } else {
                    "authoritative_sources_pending"
                };
                let at = qualified.map(|m| signed(m.qualified_at_ms)).transpose()?;
                member.established_qualification_at_ms = qualified.map(|v| v.qualified_at_ms);
                let changed=db(tx.execute("UPDATE promotion_attributions SET status=$4,reason_code=$5,qualified_at_ms=$6,qualification_algorithm_version=$7 WHERE environment=$1 AND campaign_id=$2 AND account_id=$3 AND status NOT IN ('review','excluded') AND (status<>$4 OR reason_code<>$5 OR qualified_at_ms IS DISTINCT FROM $6 OR qualification_algorithm_version IS DISTINCT FROM $7)",&[&env,&campaign_id,&member.account_id,&status,&reason,&at,&result.algorithm_version]))?;
                if changed > 0 {
                    audit(
                        tx,
                        env,
                        "promotion_worker",
                        "qualification.updated",
                        &format!("{campaign_id}:{}:{now_ms}", member.account_id),
                        "source_facts",
                        json!({"campaignId":campaign_id,"status":status,"reasonCode":reason,"qualifiedAtMs":at}),
                        now_ms,
                    )?;
                }
            }
            let result = configured_commission(tx, &input)?;
            (input, result)
        };
        let input_hash = commission_input_hash(&input, &result)?;
        inserted+=db(tx.execute("INSERT INTO promotion_commission_runs(environment,campaign_id,leader_id,input_sha256,input_json,result_json,created_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT DO NOTHING",&[&env,&campaign_id,&id,&input_hash,&serde_json::to_value(&input).map_err(|_|"promotionSnapshotInvalid")?,&serde_json::to_value(&result).map_err(|_|"promotionSnapshotInvalid")?,&signed(now_ms)?]))?;
    }
    verify_budget(tx, env, campaign_id, &row)?;
    Ok(
        json!({"memberCount":members,"leaderCount":leaders.len(),"insertedRuns":inserted,"budgetVerified":true,"reservedMicroUsd":reserved.to_string(),"commissionEnabled":row.get::<_,bool>(7),"payoutExecutionEnabled":campaign_payout_enabled(tx,env,campaign_id)?}),
    )
}

fn queue_health<T: GenericClient>(client: &mut T, env: &str, now_ms: u64) -> Result<Value, String> {
    let row=db(client.query_one("SELECT count(*) FILTER(WHERE status='pending'),count(*) FILTER(WHERE status='failed'),count(*) FILTER(WHERE status='processing'),count(*) FILTER(WHERE status='completed'),min(created_at_ms) FILTER(WHERE status='pending'),min(created_at_ms) FILTER(WHERE status='failed'),count(*) FILTER(WHERE status IN ('pending','failed') AND created_at_ms<=$2 AND (next_attempt_at_ms IS NULL OR next_attempt_at_ms<=$2)),max(last_attempt_at_ms),count(*) FILTER(WHERE last_error_code='promotionWorkExternalAdapterRequired') FROM promotion_outbox WHERE environment=$1",&[&env,&signed(now_ms)?]))?;
    let payout_enabled: bool = db(client.query_one(
        "SELECT EXISTS(SELECT 1 FROM promotion_campaigns WHERE environment=$1 AND payout_enabled)",
        &[&env],
    ))?
    .get(0);
    let oldest = row.get::<_, Option<i64>>(4).map(positive);
    Ok(
        json!({"scope":"environment","pending":row.get::<_,i64>(0),"failed":row.get::<_,i64>(1),"processing":row.get::<_,i64>(2),"completed":row.get::<_,i64>(3),"oldestPendingCreatedAtMs":oldest,"oldestPendingAgeMs":oldest.map(|v|now_ms.saturating_sub(v)),"oldestFailedCreatedAtMs":row.get::<_,Option<i64>>(5).map(positive),"due":row.get::<_,i64>(6),"lastAttemptAtMs":row.get::<_,Option<i64>>(7).map(positive),"externalAdapterBlocked":row.get::<_,i64>(8),"internalProjectionWorkerImplemented":true,"externalDispatchImplemented":false,"payoutExecutionEnabled":payout_enabled,"asOfMs":now_ms}),
    )
}

fn masked_member(env: &str, campaign: &str, leader: &str, account: &str) -> String {
    let digest = Sha256::digest(
        format!("numeron.member.v1\0{env}\0{campaign}\0{leader}\0{account}").as_bytes(),
    );
    format!("member-{}", &hex(&digest)[..16])
}

fn campaign_payout_enabled<T: GenericClient>(
    client: &mut T,
    env: &str,
    campaign: &str,
) -> Result<bool, String> {
    Ok(db(client.query_opt(
        "SELECT payout_enabled FROM promotion_campaigns WHERE environment=$1 AND campaign_id=$2",
        &[&env, &campaign],
    ))?
    .is_some_and(|r| r.get::<_, bool>(0)))
}

fn account_payout_enabled<T: GenericClient>(
    client: &mut T,
    env: &str,
    account: &str,
) -> Result<bool, String> {
    Ok(db(client.query_one("SELECT EXISTS(SELECT 1 FROM promotion_campaigns c JOIN promotion_leaders l USING(environment,campaign_id) WHERE c.environment=$1 AND l.account_id=$2 AND l.status='approved' AND c.payout_enabled)",&[&env,&account]))?.get(0))
}

fn payout_settings<T: GenericClient>(
    client: &mut T,
    env: &str,
    account: &str,
) -> Result<Value, String> {
    let enabled = account_payout_enabled(client, env, account)?;
    let row=db(client.query_opt("SELECT version,network,wallet_address,verified_at_ms,cooldown_until_ms FROM promotion_payout_addresses WHERE environment=$1 AND account_id=$2 ORDER BY version DESC LIMIT 1",&[&env,&account]))?;
    Ok(if let Some(row) = row {
        json!({"status":"verified","version":row.get::<_,i64>(0),"network":row.get::<_,String>(1),"address":row.get::<_,String>(2),"verifiedAtMs":positive(row.get(3)),"cooldownUntilMs":positive(row.get(4)),"payoutEnabled":enabled,"reasonCode":if enabled {"reviewed_signed_payout_required"}else{"campaign_payout_disabled"}})
    } else {
        json!({"status":"unverified","payoutEnabled":false,"reasonCode":"wallet_proof_required"})
    })
}

fn list_settlements<T: GenericClient>(
    client: &mut T,
    env: &str,
    campaign: &str,
    account: &str,
) -> Result<Vec<Value>, String> {
    let enabled = campaign_payout_enabled(client, env, campaign)?;
    let rows=db(client.query("SELECT s.settlement_id,s.status,s.snapshot_sha256,s.total_micro_usd,s.payable_cents,s.reserve_micro_usd,s.created_at_ms,p.payout_id,p.status,COALESCE((SELECT -sum(e.amount_micro_usd) FROM promotion_commission_entries e WHERE e.environment=s.environment AND e.settlement_id=s.settlement_id),0)::bigint,s.snapshot_json,p.amount_micro_usd,p.tx_digest,(SELECT o.result_json->>'checkpoint' FROM promotion_payout_chain_observations o WHERE o.environment=p.environment AND o.payout_id=p.payout_id AND o.result_json->>'status'='confirmed' ORDER BY o.observed_at_ms DESC LIMIT 1) FROM promotion_settlements s JOIN promotion_leaders l USING(environment,campaign_id,leader_id) LEFT JOIN promotion_payouts p USING(environment,settlement_id) WHERE s.environment=$1 AND s.campaign_id=$2 AND l.account_id=$3 ORDER BY s.created_at_ms DESC LIMIT 100",&[&env,&campaign,&account]))?;
    rows.iter().map(|r| {
        let snapshot:Value=r.get(10);
        let gross=settlement_payable_micro_usd(client,env,campaign,&snapshot,r.get(3),r.get(5),r.get(4))?;
        let payable=gross.checked_sub(r.get::<_,i64>(9)).ok_or("promotionAmountOverflow")?.max(0);
        let amount=r.get::<_,Option<i64>>(11);
        let status=r.get::<_,Option<String>>(8);
        Ok(json!({"id":r.get::<_,String>(0),"status":r.get::<_,String>(1),"snapshotSha256":r.get::<_,String>(2),"totalMicroUsd":r.get::<_,i64>(3).to_string(),"payableCents":r.get::<_,i64>(4),"payableMicroUsd":payable.to_string(),"grossPayableMicroUsd":gross.to_string(),"payoutPrecision":if snapshot["result"].get("payableMicroUsd").is_some(){6}else{2},"reserveMicroUsd":r.get::<_,i64>(5).to_string(),"createdAtMs":positive(r.get(6)),"payoutId":r.get::<_,Option<String>>(7),"payoutStatus":status,"payoutAmountMicroUsd":amount.map(|v|v.to_string()),"paidMicroUsd":if status.as_deref()==Some("confirmed"){amount.unwrap_or(0).to_string()}else{"0".into()},"txDigest":r.get::<_,Option<String>>(12),"checkpoint":r.get::<_,Option<String>>(13),"reversalMicroUsd":r.get::<_,i64>(9).to_string(),"payoutEnabled":enabled}))
    }).collect()
}

fn qualification_times<T: GenericClient>(
    client: &mut T,
    env: &str,
    account: &str,
    start: u64,
    end: u64,
    rules: &PromotionRules,
) -> Result<(Option<u64>, Option<u64>), String> {
    let level=db(client.query_one("SELECT min(source_occurred_at_ms) FROM promotion_game_facts WHERE environment=$1 AND account_id=$2 AND source='level_commit' AND level >= $3 AND source_occurred_at_ms >= $4 AND source_occurred_at_ms < $5",&[&env,&account,&i32::from(rules.minimum_level),&signed(start)?,&signed(end)?]))?.get::<_,Option<i64>>(0).map(positive);
    let play=db(client.query_opt("SELECT min(source_occurred_at_ms) AS first_event FROM promotion_game_facts WHERE environment=$1 AND account_id=$2 AND source IN ('combat_commit','quest_commit') AND source_occurred_at_ms >= $3 AND source_occurred_at_ms < $4 GROUP BY utc_day ORDER BY first_event,utc_day OFFSET $5 LIMIT 1",&[&env,&account,&signed(start)?,&signed(end)?,&i64::from(rules.active_utc_days-1)]))?.map(|r|positive(r.get(0)));
    Ok((level, play))
}

fn ensure_no_source_holds<T: GenericClient>(
    client: &mut T,
    env: &str,
    campaign_id: &str,
    leader: &str,
) -> Result<(), String> {
    let held:bool=db(client.query_one("SELECT EXISTS(SELECT 1 FROM promotion_revenue_source_holds h JOIN promotion_attributions a USING(environment,campaign_id,account_id) WHERE h.environment=$1 AND h.campaign_id=$2 AND a.leader_id=$3 AND h.active)",&[&env,&campaign_id,&leader]))?.get(0);
    if held {
        return Err("promotionRevenueReconciliationRequired".into());
    }
    Ok(())
}

fn build_input<T: GenericClient>(
    client: &mut T,
    env: &str,
    campaign_id: &str,
    leader: &str,
) -> Result<CommissionInput, String> {
    let row = campaign(client, env, campaign_id, false)?;
    let rules = rules(&row)?;
    let start = positive(row.get(2));
    let end = positive(row.get(3));
    let mut members = Vec::new();
    for attr in db(client.query("SELECT a.account_id,a.status,a.qualified_at_ms,a.qualification_algorithm_version,owner.is_banned FROM promotion_attributions a JOIN accounts owner ON owner.account_id=a.account_id WHERE a.environment=$1 AND a.campaign_id=$2 AND a.leader_id=$3 ORDER BY a.account_id",&[&env,&campaign_id,&leader]))? {
        let account:String=attr.get(0);let (level,play)=qualification_times(client,env,&account,start,end,&rules)?;
        let revenue=db(client.query("SELECT r.allocation_id,r.paid_at_ms,r.service_start_ms,r.service_end_ms,r.eligible_micro_usd,COALESCE((SELECT sum(a.reversal_micro_usd) FROM promotion_revenue_adjustments a WHERE a.environment=r.environment AND a.allocation_id=r.allocation_id),0)::bigint,r.is_card_payment,r.entitlement_at_ms FROM promotion_revenue_allocations r WHERE r.environment=$1 AND r.campaign_id=$2 AND r.account_id=$3 AND r.funding_status='verified_paid' AND NOT EXISTS(SELECT 1 FROM promotion_revenue_source_holds h WHERE h.environment=r.environment AND h.campaign_id=r.campaign_id AND h.account_id=r.account_id AND h.subscription_id=r.subscription_id AND h.active) ORDER BY r.service_start_ms,r.allocation_id",&[&env,&campaign_id,&account]))?.iter().map(|r|RevenueInput{allocation_id:r.get(0),paid_at_ms:positive(r.get(1)),service_start_ms:positive(r.get(2)),service_end_ms:positive(r.get(3)),amount_micro_usd:positive(r.get(4)),reversed_micro_usd:positive(r.get(5)),is_card_payment:r.get(6),entitlement_at_ms:positive(r.get(7))}).collect();
        let prior_version=attr.get::<_,Option<String>>(3); let expected_version=rules.revenue_recognition.algorithm_version(); let established=if prior_version.as_deref()==Some(expected_version) || (rules.revenue_recognition==RevenueRecognition::MonthlyPurchaseFull && prior_version.as_deref()==Some(ALGORITHM_VERSION)){attr.get::<_,Option<i64>>(2).map(positive)}else{None};
        members.push(MemberInput{account_id:account,level_completed_at_ms:level,play_completed_at_ms:play,established_qualification_at_ms:established,excluded:attr.get::<_,bool>(4)||matches!(attr.get::<_,String>(1).as_str(),"excluded"|"review"),revenue});
    }
    Ok(CommissionInput {
        environment: env.into(),
        campaign_id: campaign_id.into(),
        terms_version: row.get(0),
        starts_at_ms: start,
        ends_at_ms: end,
        maximum_coverage_ms: rules.maximum_coverage_ms,
        per_member_revenue_cap_micro_usd: if row.get::<_, String>(17) == "revenue_share"
            && row.get::<_, i64>(12) == 0
        {
            u64::MAX
        } else {
            positive(row.get(12)).max(1)
        },
        reserve_bps: rules.reserve_bps,
        members,
    })
}

fn calculation<T: GenericClient>(
    client: &mut T,
    env: &str,
    campaign_id: &str,
    leader: &str,
) -> Result<CommissionResult, String> {
    if let Some(row)=db(client.query_opt("SELECT snapshot_json FROM promotion_settlements WHERE environment=$1 AND campaign_id=$2 AND leader_id=$3",&[&env,&campaign_id,&leader]))? {
        let snapshot:Value=row.get(0);return serde_json::from_value(snapshot["result"].clone()).map_err(|_|"promotionSnapshotInvalid".into());
    }
    let input = build_input(client, env, campaign_id, leader)?;
    configured_commission(client, &input)
}

fn dashboard<T: GenericClient>(
    client: &mut T,
    env: &str,
    campaign_id: &str,
    leader: &str,
    now: u64,
) -> Result<Value, String> {
    let row = campaign(client, env, campaign_id, false)?;
    let clicks:i64=db(client.query_one("SELECT (SELECT count(*) FROM promotion_invitation_clicks WHERE environment=$1 AND campaign_id=$2 AND leader_id=$3)+(SELECT count(*) FROM promotion_candidates WHERE environment=$1 AND campaign_id=$2 AND leader_id=$3 AND source='manual_code')",&[&env,&campaign_id,&leader]))?.get(0);
    let registrations:i64=db(client.query_one("SELECT count(DISTINCT c.account_id) FROM promotion_candidates c JOIN accounts a USING(account_id) WHERE c.environment=$1 AND c.campaign_id=$2 AND c.leader_id=$3 AND (extract(epoch FROM a.created_at)*1000)::bigint >= $4 AND (extract(epoch FROM a.created_at)*1000)::bigint < $5",&[&env,&campaign_id,&leader,&row.get::<_,i64>(2),&row.get::<_,i64>(3)]))?.get(0);
    let confirmed:i64=db(client.query_one("SELECT count(*) FROM promotion_attributions WHERE environment=$1 AND campaign_id=$2 AND leader_id=$3",&[&env,&campaign_id,&leader]))?.get(0);
    let estimate = calculation(client, env, campaign_id, leader)?;
    let tiers=[1000u16,2000,3000].iter().map(|bps|json!({"rateBps":bps,"count":estimate.members.iter().filter(|m|m.rate_bps==*bps).count()})).collect::<Vec<_>>();
    let reversals:i64=db(client.query_one("SELECT COALESCE(-sum(amount_micro_usd),0)::bigint FROM promotion_commission_entries WHERE environment=$1 AND campaign_id=$2 AND leader_id=$3",&[&env,&campaign_id,&leader]))?.get(0);
    Ok(
        json!({"clicks":clicks,"clickMeasurement":"anonymous_clicks_and_manual_candidates","registrations":registrations,"confirmed":confirmed,"qualified":estimate.members.len(),"tiers":tiers,"eligibleRevenueMicroUsd":estimate.eligible_revenue_micro_usd.to_string(),"estimatedCommissionMicroUsd":if row.get::<_,bool>(7){estimate.commission_micro_usd.saturating_sub(positive(reversals))}else{0}.to_string(),"reserveMicroUsd":estimate.reserve_micro_usd.to_string(),"reversalMicroUsd":reversals.to_string(),"commissionEnabled":row.get::<_,bool>(7),"status":if row.get::<_,String>(1)=="locked"{"locked"}else if row.get::<_,bool>(7){"estimated"}else{"commission_disabled"},"asOfMs":now,"algorithmVersion":estimate.algorithm_version}),
    )
}

pub(super) fn refresh_account_qualifications(
    tx: &mut Transaction<'_>,
    env: &str,
    account_id: &str,
    now: u64,
) -> Result<(), String> {
    let attrs=db(tx.query("SELECT a.campaign_id,a.leader_id,a.status FROM promotion_attributions a WHERE a.environment=$1 AND a.account_id=$2 AND a.status NOT IN ('review','excluded') AND NOT EXISTS(SELECT 1 FROM promotion_settlements s WHERE s.environment=a.environment AND s.campaign_id=a.campaign_id AND s.leader_id=a.leader_id)",&[&env,&account_id]))?;
    for attr in attrs {
        let campaign_id: String = attr.get(0);
        let leader: String = attr.get(1);
        // A temporary provider dispute hides income but must not erase the
        // already established qualification timestamp and reorder the cohort.
        let held:bool=db(tx.query_one("SELECT EXISTS(SELECT 1 FROM promotion_revenue_source_holds WHERE environment=$1 AND campaign_id=$2 AND account_id=$3 AND active)",&[&env,&campaign_id,&account_id]))?.get(0);
        if held {
            continue;
        }
        let input = build_input(tx, env, &campaign_id, &leader)?;
        let result = configured_commission(tx, &input)?;
        let qualified = result.members.iter().find(|m| m.account_id == account_id);
        let status = if qualified.is_some() {
            "qualified"
        } else {
            "bound"
        };
        let reason = if qualified.is_some() {
            "requirements_complete"
        } else {
            "authoritative_sources_pending"
        };
        let at = qualified.map(|m| signed(m.qualified_at_ms)).transpose()?;
        let changed=db(tx.execute("UPDATE promotion_attributions SET status=$4,reason_code=$5,qualified_at_ms=$6,qualification_algorithm_version=$7 WHERE environment=$1 AND campaign_id=$2 AND account_id=$3 AND (status<>$4 OR reason_code<>$5 OR qualified_at_ms IS DISTINCT FROM $6 OR qualification_algorithm_version IS DISTINCT FROM $7)",&[&env,&campaign_id,&account_id,&status,&reason,&at,&result.algorithm_version]))?;
        if changed > 0 {
            audit(
                tx,
                env,
                "promotion_worker",
                "qualification.updated",
                &format!("{campaign_id}:{account_id}:{now}"),
                "source_facts",
                json!({"campaignId":campaign_id,"status":status,"reasonCode":reason,"qualifiedAtMs":at}),
                now,
            )?;
        }
    }
    Ok(())
}

fn full_monthly_policy_guard<T: GenericClient>(
    client: &mut T,
    env: &str,
    campaign_id: &str,
    now: u64,
    lock: bool,
) -> Result<postgres::Row, String> {
    if env != "test" || campaign_id != super::TARGET_CAMPAIGN_ID {
        return Err("promotionFullMonthlyPolicyScopeInvalid".into());
    }
    let row = campaign(client, env, campaign_id, lock)?;
    if !row.get::<_, bool>(5)
        || !matches!(row.get::<_, String>(1).as_str(), "approved" | "active")
        || now < positive(row.get(2))
        || now >= positive(row.get(3))
        || campaign_payout_enabled(client, env, campaign_id)?
    {
        return Err("promotionFullMonthlyPolicyStateInvalid".into());
    }
    let locked: bool = db(client.query_one(
        "SELECT EXISTS(SELECT 1 FROM promotion_settlements WHERE environment=$1 AND campaign_id=$2)",
        &[&env,&campaign_id],
    ))?.get(0);
    if locked {
        return Err("promotionSeasonLocked".into());
    }
    rules(&row)?.validate_environment(env)?;
    Ok(row)
}

fn full_monthly_policy_preview<T: GenericClient>(
    client: &mut T,
    env: &str,
    campaign_id: &str,
    row: &postgres::Row,
) -> Result<Value, String> {
    let policy = rules(row)?;
    let micro = super::production_gate::test_micro_enabled(env, &row.get::<_, Value>(4))?;
    let mut leaders = Vec::new();
    for leader in db(client.query("SELECT leader_id FROM promotion_leaders WHERE environment=$1 AND campaign_id=$2 ORDER BY leader_id", &[&env,&campaign_id]))? {
        let id: String = leader.get(0);
        ensure_no_source_holds(client,env,campaign_id,&id)?;
        let input = build_input(client,env,campaign_id,&id)?;
        let before = configured_commission(client,&input)?;
        let after = super::calculate_full_monthly_commission(&input,micro)?;
        leaders.push(json!({"leaderId":id,"before":before,"after":after}));
    }
    Ok(
        json!({"campaignId":campaign_id,"currentRecognition":policy.revenue_recognition,
        "rulesSha256":hash(&row.get::<_,Value>(4))?,"leaders":leaders,"payoutEnabled":false}),
    )
}

impl PromotionStore {
    /// Narrow test-only preview. It never creates a settlement, entitlement or payment.
    pub fn preview_full_monthly_policy(
        &self,
        campaign_id: &str,
        now_ms: u64,
    ) -> Result<Value, String> {
        with_account_store_postgres_client(&self.database_url, |client| {
            let mut tx = db(client.build_transaction().read_only(true).start())?;
            let row =
                full_monthly_policy_guard(&mut tx, &self.environment, campaign_id, now_ms, false)?;
            let result =
                full_monthly_policy_preview(&mut tx, &self.environment, campaign_id, &row)?;
            db(tx.commit())?;
            Ok(result)
        })
    }

    /// Owner-authorized favorable amendment for this single unlocked Testnet season.
    /// Existing role identities are checked, not created or presented as human signatures.
    pub fn adopt_full_monthly_policy(
        &self,
        campaign_id: &str,
        operations_actor: &str,
        finance_actor: &str,
        expected_rules_sha256: &str,
        evidence_ref: &str,
        now_ms: u64,
    ) -> Result<Value, String> {
        if evidence_ref.trim().is_empty()
            || evidence_ref.len() > 512
            || expected_rules_sha256.len() != 64
            || !expected_rules_sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("promotionFullMonthlyPolicyEvidenceInvalid".into());
        }
        if operations_actor == finance_actor {
            return Err("promotionDistinctReviewerRequired".into());
        }
        with_account_store_postgres_client(&self.database_url, |client| {
            let mut tx = db(client.transaction())?;
            let row =
                full_monthly_policy_guard(&mut tx, &self.environment, campaign_id, now_ms, true)?;
            for (actor, role) in [(operations_actor, "operations"), (finance_actor, "finance")] {
                let _ = account(&mut tx, actor, false)?;
                let allowed: bool=db(tx.query_one("SELECT EXISTS(SELECT 1 FROM promotion_operator_roles WHERE environment=$1 AND account_id=$2 AND role=$3 AND revoked_at_ms IS NULL)", &[&self.environment,&actor,&role]))?.get(0);
                if !allowed {
                    return Err("promotionPermissionDenied".into());
                }
            }
            let policy_id = hash(&json!([
                "full-monthly-policy-v1",
                &self.environment,
                campaign_id,
                operations_actor,
                finance_actor,
                expected_rules_sha256,
                evidence_ref
            ]))?;
            if let Some(prior)=db(tx.query_opt("SELECT summary_json FROM promotion_audit_log WHERE environment=$1 AND action='rules.full_monthly_adopted' AND resource_id=$2", &[&self.environment,&policy_id]))? {
                if rules(&row)?.revenue_recognition!=RevenueRecognition::MonthlyPurchaseFull { return Err("promotionFullMonthlyPolicyChanged".into()); }
                let mut result:Value=prior.get::<_,Value>(0)["result"].clone();result["status"]=json!("already_applied");
                db(tx.commit())?;return Ok(result);
            }
            let old_rules: Value = row.get(4);
            if hash(&old_rules)? != expected_rules_sha256 {
                return Err("promotionFullMonthlyPolicyChanged".into());
            }
            if rules(&row)?.revenue_recognition != RevenueRecognition::ServicePeriodProrated {
                return Err("promotionFullMonthlyPolicyChanged".into());
            }
            let mut result =
                full_monthly_policy_preview(&mut tx, &self.environment, campaign_id, &row)?;
            let mut new_rules = old_rules.clone();
            new_rules["revenueRecognition"] = json!("monthly_purchase_full");
            serde_json::from_value::<PromotionRules>(new_rules.clone())
                .map_err(|_| "promotionRulesInvalid")?
                .validate_environment(&self.environment)?;
            db(tx.execute("UPDATE promotion_campaigns SET rules_json=$3 WHERE environment=$1 AND campaign_id=$2", &[&self.environment,&campaign_id,&new_rules]))?;
            let projection = reconcile_projection(&mut tx, &self.environment, campaign_id, now_ms)?;
            result["status"] = json!("applied");
            result["policyId"] = json!(policy_id);
            result["newRulesSha256"] = json!(hash(&new_rules)?);
            result["newRecognition"] = json!("monthly_purchase_full");
            result["projection"] = projection;
            audit(
                &mut tx,
                &self.environment,
                operations_actor,
                "rules.full_monthly_adopted",
                &policy_id,
                evidence_ref,
                json!({"campaignId":campaign_id,"operationsActor":operations_actor,"financeActor":finance_actor,
                    "authorization":"owner_requested_test_policy_amendment","beforeRules":old_rules,"afterRules":new_rules,"result":result}),
                now_ms,
            )?;
            db(tx.commit())?;
            Ok(result)
        })
    }
}

#[cfg(test)]
mod canonical_tests {
    use super::*;
    #[test]
    fn canonical_snapshot_sorts_keys_and_keeps_wide_integer_bytes() {
        let value: Value = serde_json::from_str(
            r#"{"z":18446744073709551615,"a":{"y":9007199254740993,"b":true}}"#,
        )
        .unwrap();
        let canonical = canonical_json(&value).unwrap();
        assert_eq!(
            canonical,
            r#"{"a":{"b":true,"y":9007199254740993},"z":18446744073709551615}"#
        );
        let roundtrip: Value = serde_json::from_str(&canonical).unwrap();
        assert_eq!(hash(&value).unwrap(), hash(&roundtrip).unwrap());
        assert!(canonical_json(&json!({"money":1.5})).is_err());
    }
}
