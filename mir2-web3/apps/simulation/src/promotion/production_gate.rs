//! Human acceptance is an explicit, dual-reviewed attestation bound to the
//! exact campaign configuration. Tests never create a production attestation.
use super::store::{audit, db, hash, signed};
use super::PromotionStore;
use crate::with_account_store_postgres_client;
use postgres::GenericClient;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PromotionLaunchManifest {
    pub configuration_sha256: String,
    pub network: String,
    pub chain_identifier: String,
    pub coin_type: String,
    pub coin_decimals: u8,
    pub fx_snapshot_ref: String,
    pub minimum_payout_micro_usd: String,
    pub gas_fee_policy: String,
    pub identity_tax_policy_ref: String,
    pub regions: Vec<String>,
    pub terms_url: String,
    pub mobile_signature_evidence: String,
    pub provider_payment_evidence: String,
    pub onchain_payout_digest: String,
}
fn evidence(v: &str) -> bool {
    !v.trim().is_empty() && v.len() <= 512 && !v.chars().any(char::is_control)
}
fn hex(v: &str) -> bool {
    v.len() == 64
        && v.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn digest(v: &str) -> bool {
    (32..=64).contains(&v.len())
        && v.bytes()
            .all(|b| b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".contains(&b))
}
impl PromotionLaunchManifest {
    fn validate(&self, environment: &str) -> Result<(), String> {
        let network = if environment == "production" {
            "sui:mainnet"
        } else {
            "sui:testnet"
        };
        if !hex(&self.configuration_sha256)
            || self.network != network
            || !digest(&self.chain_identifier)
            || (environment == "production" && !digest(&self.onchain_payout_digest))
            || self.coin_decimals > 18
            || !evidence(&self.coin_type)
            || !self.coin_type.contains("::")
            || !evidence(&self.fx_snapshot_ref)
            || self
                .minimum_payout_micro_usd
                .parse::<u64>()
                .ok()
                .is_none_or(|v| v == 0 || v.to_string() != self.minimum_payout_micro_usd)
            || self.gas_fee_policy != "treasury"
            || !evidence(&self.identity_tax_policy_ref)
            || (environment == "production"
                && (!evidence(&self.mobile_signature_evidence)
                    || !evidence(&self.provider_payment_evidence)))
            || !self.terms_url.starts_with("https://")
            || !evidence(&self.terms_url)
            || self.regions.is_empty()
            || self.regions.len() > 250
            || self
                .regions
                .iter()
                .any(|r| r.len() != 2 || !r.bytes().all(|b| b.is_ascii_uppercase()))
        {
            return Err("promotionLaunchManifestInvalid".into());
        }
        Ok(())
    }
}
fn actor<T: GenericClient>(tx: &mut T, env: &str, id: &str, role: &str) -> Result<(), String> {
    let allowed:bool=db(tx.query_one("SELECT EXISTS(SELECT 1 FROM promotion_operator_roles r JOIN accounts a ON a.account_id=r.account_id WHERE r.environment=$1 AND r.account_id=$2 AND r.role=$3 AND r.revoked_at_ms IS NULL AND NOT a.is_banned)",&[&env,&id,&role]))?.get(0);
    if !allowed {
        return Err("promotionPermissionDenied".into());
    }
    Ok(())
}
pub(super) fn configuration<T: GenericClient>(
    tx: &mut T,
    env: &str,
    campaign: &str,
) -> Result<Value, String> {
    let r=db(tx.query_opt("SELECT terms_version,starts_at_ms,ends_at_ms,rules_json,budget_micro_usd,member_limit,leader_limit,per_leader_member_limit,per_member_revenue_cap_micro_usd,rules_approved,funding_mode FROM promotion_campaigns WHERE environment=$1 AND campaign_id=$2",&[&env,&campaign]))?.ok_or("promotionCampaignMissing")?;
    if !r.get::<_, bool>(9)
        || (r.get::<_, String>(10) == "budget_capped" && (4..=8).any(|i| r.get::<_, i64>(i) <= 0))
    {
        return Err("promotionCampaignApprovalRequired".into());
    }
    Ok(
        json!({"environment":env,"campaignId":campaign,"fundingMode":r.get::<_,String>(10),"termsVersion":r.get::<_,String>(0),"startsAtMs":r.get::<_,i64>(1).to_string(),"endsAtMs":r.get::<_,i64>(2).to_string(),"rules":r.get::<_,Value>(3),"budgetMicroUsd":r.get::<_,i64>(4).to_string(),"memberLimit":r.get::<_,i64>(5).to_string(),"leaderLimit":r.get::<_,i64>(6).to_string(),"perLeaderMemberLimit":r.get::<_,i64>(7).to_string(),"perMemberRevenueCapMicroUsd":r.get::<_,i64>(8).to_string()}),
    )
}
pub(super) fn approved<T: GenericClient>(
    tx: &mut T,
    env: &str,
    campaign: &str,
) -> Result<bool, String> {
    let Some(r)=db(tx.query_opt("SELECT configuration_sha256 FROM promotion_production_approvals WHERE environment=$1 AND campaign_id=$2 AND approved_by IS NOT NULL",&[&env,&campaign]))? else {return Ok(false)};
    Ok(r.get::<_, String>(0) == hash(&configuration(tx, env, campaign)?)?)
}
pub(super) fn gaps<T: GenericClient>(
    tx: &mut T,
    env: &str,
    campaign: &str,
) -> Result<Vec<String>, String> {
    let mut result = Vec::new();
    if !approved(tx, env, campaign)? {
        result.push("production_acceptance_and_launch_approval_required".into());
    }
    let held:bool=db(tx.query_one("SELECT EXISTS(SELECT 1 FROM promotion_revenue_source_holds WHERE environment=$1 AND campaign_id=$2 AND active)",&[&env,&campaign]))?.get(0);
    if held {
        result.push("payment_source_reconciliation_required".into());
    }
    let pending:bool=db(tx.query_one("SELECT EXISTS(SELECT 1 FROM promotion_game_source_events e LEFT JOIN promotion_game_source_deliveries d USING(environment,source_event_id) WHERE e.environment=$1 AND d.source_event_id IS NULL)",&[&env]))?.get(0);
    if pending {
        result.push("committed_game_events_pending".into());
    }
    Ok(result)
}
pub(super) fn public_policy<T: GenericClient>(
    tx: &mut T,
    env: &str,
    campaign: &str,
) -> Result<Value, String> {
    let row=db(tx.query_opt("SELECT manifest_json,approved_by FROM promotion_production_approvals WHERE environment=$1 AND campaign_id=$2",&[&env,&campaign]))?;
    Ok(row.map(|r| {
        let manifest:Value=r.get(0);
        let coin=manifest["coinType"].as_str().unwrap_or("");
        json!({"status":if r.get::<_,Option<String>>(1).is_some(){"approved"}else{"proposed"},"network":manifest["network"],"coinType":coin,"coinDecimals":manifest["coinDecimals"],"symbol":if coin.ends_with("::usdc::USDC"){"USDC"}else if coin.ends_with("::sui::SUI"){"SUI"}else{coin}})
    }).unwrap_or(Value::Null))
}
impl PromotionStore {
    pub fn propose_launch(
        &self,
        campaign: &str,
        account: &str,
        manifest: &PromotionLaunchManifest,
        reference: &str,
        now: u64,
    ) -> Result<Value, String> {
        manifest.validate(&self.environment)?;
        if !evidence(reference) {
            return Err("promotionEvidenceRequired".into());
        }
        with_account_store_postgres_client(&self.database_url, |c| {
            let mut tx = db(c.transaction())?;
            actor(&mut tx, &self.environment, account, "operations")?;
            // Serialize proposals and financial gate changes with the campaign row.
            db(tx.query_one("SELECT campaign_id FROM promotion_campaigns WHERE environment=$1 AND campaign_id=$2 FOR UPDATE",&[&self.environment,&campaign]))?;
            if hash(&configuration(&mut tx, &self.environment, campaign)?)?
                != manifest.configuration_sha256
            {
                return Err("promotionLaunchConfigurationChanged".into());
            }
            let sha = hash(manifest)?;
            if let Some(r)=db(tx.query_opt("SELECT manifest_sha256 FROM promotion_production_approvals WHERE environment=$1 AND campaign_id=$2",&[&self.environment,&campaign]))? {
    if r.get::<_,String>(0)!=sha{return Err("promotionLaunchManifestImmutable".into());}
    return Ok(json!({"campaignId":campaign,"manifestSha256":sha,"status":"proposed"}));
   }
            db(tx.execute("INSERT INTO promotion_production_approvals(environment,campaign_id,configuration_sha256,manifest_sha256,manifest_json,proposed_by,proposed_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7)",&[&self.environment,&campaign,&manifest.configuration_sha256,&sha,&serde_json::to_value(manifest).map_err(|_|"promotionInputInvalid")?,&account,&signed(now)?]))?;
            let result = json!({"campaignId":campaign,"manifestSha256":sha,"status":"proposed"});
            audit(
                &mut tx,
                &self.environment,
                account,
                "admin.command",
                campaign,
                reference,
                result.clone(),
                now,
            )?;
            db(tx.commit())?;
            Ok(result)
        })
    }
    pub fn approve_launch(
        &self,
        campaign: &str,
        account: &str,
        sha: &str,
        reference: &str,
        now: u64,
    ) -> Result<Value, String> {
        if !hex(sha) || !evidence(reference) {
            return Err("promotionEvidenceRequired".into());
        }
        with_account_store_postgres_client(&self.database_url, |c| {
            let mut tx = db(c.transaction())?;
            actor(&mut tx, &self.environment, account, "finance")?;
            db(tx.query_one("SELECT campaign_id FROM promotion_campaigns WHERE environment=$1 AND campaign_id=$2 FOR UPDATE",&[&self.environment,&campaign]))?;
            let r=db(tx.query_opt("SELECT configuration_sha256,manifest_sha256,proposed_by,approved_by FROM promotion_production_approvals WHERE environment=$1 AND campaign_id=$2 FOR UPDATE",&[&self.environment,&campaign]))?.ok_or("promotionLaunchApprovalMissing")?;
            if r.get::<_, String>(2) == account {
                return Err("promotionDistinctReviewerRequired".into());
            }
            if r.get::<_, String>(1) != sha
                || r.get::<_, String>(0)
                    != hash(&configuration(&mut tx, &self.environment, campaign)?)?
            {
                return Err("promotionLaunchConfigurationChanged".into());
            }
            if r.get::<_, Option<String>>(3).is_none() {
                db(tx.execute("UPDATE promotion_production_approvals SET approved_by=$3,approved_at_ms=$4 WHERE environment=$1 AND campaign_id=$2",&[&self.environment,&campaign,&account,&signed(now)?]))?;
            }
            let result = json!({"campaignId":campaign,"manifestSha256":sha,"status":"approved"});
            audit(
                &mut tx,
                &self.environment,
                account,
                "admin.command",
                campaign,
                reference,
                result.clone(),
                now,
            )?;
            db(tx.commit())?;
            Ok(result)
        })
    }
}
