//! Durable payment journal for the separately validated Sui execution worker.
//! No signing keys and no RPC submission exist in this module.
use crate::with_account_store_postgres_client;
use postgres::{GenericClient, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct PromotionPayoutJournal {
    database_url: String,
    environment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PromotionPayoutObjectRef {
    pub object_id: String,
    pub version: String,
    pub digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PromotionPayoutGasValidDuring {
    pub min_epoch: String,
    pub nonce: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PromotionPayoutIntent {
    pub version: String,
    pub environment: String,
    pub campaign_id: String,
    pub payout_id: String,
    pub settlement_id: String,
    pub settlement_sha256: String,
    pub network: String,
    pub chain_identifier: String,
    pub coin_type: String,
    pub coin_decimals: u8,
    pub amount_micro_usd: String,
    pub coin_amount_atomic: String,
    pub quote_numerator: String,
    pub quote_denominator: String,
    pub fx_snapshot_ref: String,
    pub destination: String,
    pub address_version: String,
    pub sender: String,
    pub gas_budget_atomic: String,
    pub gas_price_atomic: String,
    pub gas_source: String,
    pub gas_valid_during: Option<PromotionPayoutGasValidDuring>,
    pub gas_payment: Vec<PromotionPayoutObjectRef>,
    pub payout_coin: Option<PromotionPayoutObjectRef>,
    pub expires_at_epoch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PromotionSignedPayout {
    pub intent_sha256: String,
    pub transaction_bytes: String,
    pub digest: String,
    pub signatures: Vec<String>,
}

fn err<T>(value: Result<T, postgres::Error>) -> Result<T, String> {
    value.map_err(|_| "promotionStorageUnavailable".into())
}
fn int(value: &str) -> Result<u64, String> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|v| v.is_ascii_digit())
    {
        return Err("promotionPayoutIntegerRequired".into());
    }
    value
        .parse::<u64>()
        .ok()
        .filter(|v| *v > 0)
        .ok_or_else(|| "promotionPayoutAmountInvalid".into())
}
fn id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || b"-_.:@".contains(&v))
}
fn address(value: &str) -> bool {
    value.len() == 66
        && value.starts_with("0x")
        && value[2..]
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
}
fn digest(value: &str) -> bool {
    (32..=64).contains(&value.len())
        && value
            .bytes()
            .all(|v| b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".contains(&v))
}
fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
}
fn signed(value: u64) -> Result<i64, String> {
    value
        .try_into()
        .map_err(|_| "promotionAmountOverflow".into())
}
fn canonical(value: &Value) -> Result<String, String> {
    Ok(match value {
        Value::Object(v) => {
            let mut keys: Vec<_> = v.keys().collect();
            keys.sort();
            format!(
                "{{{}}}",
                keys.into_iter()
                    .map(|k| Ok(format!(
                        "{}:{}",
                        serde_json::to_string(k).map_err(|_| "promotionInputInvalid")?,
                        canonical(&v[k])?
                    )))
                    .collect::<Result<Vec<_>, String>>()?
                    .join(",")
            )
        }
        Value::Array(v) => format!(
            "[{}]",
            v.iter()
                .map(canonical)
                .collect::<Result<Vec<_>, _>>()?
                .join(",")
        ),
        _ => serde_json::to_string(value).map_err(|_| "promotionInputInvalid")?,
    })
}
impl PromotionPayoutIntent {
    pub fn sha256(&self) -> Result<String, String> {
        let value = serde_json::to_value(self).map_err(|_| "promotionPayoutIntentInvalid")?;
        Ok(Sha256::digest(canonical(&value)?.as_bytes())
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect())
    }
    pub fn validate(&self, environment: &str, allow_mainnet: bool) -> Result<(), String> {
        if self.version != "numeron-sui-payout-v1"
            || self.environment != environment
            || !["development", "test", "production"].contains(&environment)
        {
            return Err("promotionPayoutVersionInvalid".into());
        }
        if ![
            &self.campaign_id,
            &self.payout_id,
            &self.settlement_id,
            &self.fx_snapshot_ref,
        ]
        .iter()
        .all(|v| id(v))
            || !hash(&self.settlement_sha256)
            || !digest(&self.chain_identifier)
        {
            return Err("promotionPayoutIntentInvalid".into());
        }
        if self.network != "sui:testnet" && self.network != "sui:mainnet" {
            return Err("promotionPayoutNetworkInvalid".into());
        }
        if self.network == "sui:mainnet" && (!allow_mainnet || environment != "production") {
            return Err("promotionPayoutMainnetApprovalRequired".into());
        }
        if environment == "production" && self.network != "sui:mainnet" {
            return Err("promotionPayoutNetworkInvalid".into());
        }
        if !address(&self.destination)
            || !address(&self.sender)
            || self.destination == self.sender
            || self.coin_decimals > 18
            || self.coin_type.is_empty()
            || self.coin_type.len() > 256
        {
            return Err("promotionPayoutAddressInvalid".into());
        }
        let amount = int(&self.amount_micro_usd)?;
        let num = int(&self.quote_numerator)?;
        let den = int(&self.quote_denominator)?;
        let converted = (amount as u128) * (num as u128) / (den as u128);
        if converted != int(&self.coin_amount_atomic)? as u128 {
            return Err("promotionPayoutQuoteMismatch".into());
        }
        int(&self.address_version)?;
        int(&self.gas_budget_atomic)?;
        int(&self.gas_price_atomic)?;
        int(&self.expires_at_epoch)?;
        let sui = "0x0000000000000000000000000000000000000000000000000000000000000002::sui::SUI";
        if self.gas_payment.len() > 8
            || !["coinObjects", "addressBalance"].contains(&self.gas_source.as_str())
            || (self.gas_source == "coinObjects" && self.gas_payment.is_empty())
            || (self.gas_source == "addressBalance"
                && (!self.gas_payment.is_empty()
                    || self.network != "sui:testnet"
                    || self.coin_type == sui))
        {
            return Err("promotionPayoutGasRequired".into());
        }
        if self.gas_source == "addressBalance" {
            let valid = self
                .gas_valid_during
                .as_ref()
                .ok_or("promotionPayoutExpirationInvalid")?;
            let min = if valid.min_epoch == "0" {
                0
            } else {
                int(&valid.min_epoch)?
            };
            let max = int(&self.expires_at_epoch)?;
            if max < min || max - min > 1 {
                return Err("promotionPayoutExpirationInvalid".into());
            }
        } else if self.gas_valid_during.is_some() {
            return Err("promotionPayoutExpirationInvalid".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for object in self.gas_payment.iter().chain(self.payout_coin.iter()) {
            if !address(&object.object_id)
                || !digest(&object.digest)
                || !ids.insert(&object.object_id)
            {
                return Err("promotionPayoutObjectConflict".into());
            }
            int(&object.version)?;
        }
        let sui = "0x0000000000000000000000000000000000000000000000000000000000000002::sui::SUI";
        if (self.coin_type == sui) != self.payout_coin.is_none() {
            return Err("promotionPayoutObjectInvalid".into());
        }
        Ok(())
    }
}
fn actor<T: GenericClient>(tx: &mut T, env: &str, account: &str, role: &str) -> Result<(), String> {
    if !id(account) {
        return Err("promotionPermissionDenied".into());
    }
    let allowed:bool=err(tx.query_one("SELECT EXISTS(SELECT 1 FROM promotion_operator_roles r JOIN accounts a ON a.account_id=r.account_id WHERE r.environment=$1 AND r.account_id=$2 AND r.role=$3 AND r.revoked_at_ms IS NULL AND NOT a.is_banned)",&[&env,&account,&role]))?.get(0);
    if !allowed {
        return Err("promotionPermissionDenied".into());
    }
    Ok(())
}
fn audit(
    tx: &mut Transaction<'_>,
    env: &str,
    account: &str,
    action: &str,
    payout: &str,
    evidence: &str,
    value: &Value,
    now: u64,
) -> Result<(), String> {
    err(tx.execute("INSERT INTO promotion_audit_log(environment,actor_id,action,resource_id,evidence_ref,summary_json,occurred_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7)",&[&env,&account,&action,&payout,&evidence,&value,&signed(now)?]))?;
    Ok(())
}
fn lock_payee_for_payout(tx: &mut Transaction<'_>, env: &str, payout: &str) -> Result<(), String> {
    let account:String=err(tx.query_opt("SELECT l.account_id FROM promotion_payouts p JOIN promotion_settlements s USING(environment,settlement_id) JOIN promotion_leaders l USING(environment,campaign_id,leader_id) WHERE p.environment=$1 AND p.payout_id=$2",&[&env,&payout]))?.ok_or("promotionPayoutMissing")?.get(0);
    super::store::lock_payee(tx, env, &account)
}
fn readiness<T: GenericClient>(
    tx: &mut T,
    env: &str,
    payout: &str,
    intent: Option<&PromotionPayoutIntent>,
    now: u64,
) -> Result<(), String> {
    let row=err(tx.query_opt("SELECT c.payout_enabled,l.status,a.is_banned,EXISTS(SELECT 1 FROM promotion_revenue_source_holds h WHERE h.environment=p.environment AND h.campaign_id=s.campaign_id AND h.active AND (h.account_id=l.account_id OR EXISTS(SELECT 1 FROM promotion_attributions x WHERE x.environment=h.environment AND x.campaign_id=h.campaign_id AND x.account_id=h.account_id AND x.leader_id=l.leader_id))),c.rules_json FROM promotion_payouts p JOIN promotion_settlements s USING(environment,settlement_id) JOIN promotion_campaigns c USING(environment,campaign_id) JOIN promotion_leaders l USING(environment,campaign_id,leader_id) JOIN accounts a ON a.account_id=l.account_id WHERE p.environment=$1 AND p.payout_id=$2",&[&env,&payout]))?.ok_or("promotionPayoutMissing")?;
    if !row.get::<_, bool>(0) {
        return Err("promotionPayoutExecutionDisabled".into());
    }
    {
        let stored;
        let intent = match intent {
            Some(v) => v,
            None => {
                let r=err(tx.query_opt("SELECT intent_json FROM promotion_payout_intents WHERE environment=$1 AND payout_id=$2",&[&env,&payout]))?.ok_or("promotionPayoutIntentMissing")?;
                stored = serde_json::from_value::<PromotionPayoutIntent>(r.get(0))
                    .map_err(|_| "promotionPayoutIntentInvalid")?;
                &stored
            }
        };
        if !super::production_gate::approved(tx, env, &intent.campaign_id)? {
            return Err("promotionProductionApprovalRequired".into());
        }
        let manifest:Value=err(tx.query_opt("SELECT manifest_json FROM promotion_production_approvals WHERE environment=$1 AND campaign_id=$2 AND approved_by IS NOT NULL",&[&env,&intent.campaign_id]))?.ok_or("promotionProductionApprovalRequired")?.get(0);
        let micro =
            super::production_gate::test_micro_manifest(env, &row.get::<_, Value>(4), &manifest)?;
        if micro && (intent.quote_numerator != "1" || intent.quote_denominator != "1") {
            return Err("promotionPayoutQuoteMismatch".into());
        }
        if manifest.get("network").and_then(Value::as_str) != Some(intent.network.as_str())
            || manifest.get("chainIdentifier").and_then(Value::as_str)
                != Some(intent.chain_identifier.as_str())
            || manifest.get("coinType").and_then(Value::as_str) != Some(intent.coin_type.as_str())
            || manifest.get("coinDecimals").and_then(Value::as_u64)
                != Some(u64::from(intent.coin_decimals))
            || manifest.get("fxSnapshotRef").and_then(Value::as_str)
                != Some(intent.fx_snapshot_ref.as_str())
        {
            return Err("promotionPayoutManifestMismatch".into());
        }
        let minimum = manifest
            .get("minimumPayoutMicroUsd")
            .and_then(Value::as_str)
            .ok_or("promotionPayoutManifestMismatch")?;
        if int(&intent.amount_micro_usd)? < int(minimum)? {
            return Err("promotionPayoutBelowMinimum".into());
        }
        if manifest.get("gasFeePolicy").and_then(Value::as_str) != Some("treasury")
            || manifest
                .get("identityTaxPolicyRef")
                .and_then(Value::as_str)
                .is_none_or(|v| v.trim().is_empty())
        {
            return Err("promotionPayoutManifestMismatch".into());
        }
    }
    if row.get::<_, String>(1) != "approved" || row.get::<_, bool>(2) || row.get::<_, bool>(3) {
        return Err("promotionPayoutRiskReviewRequired".into());
    }
    // Serialize account bans with the final gate. Lock in a stable order after
    // the payee lock, matching address verification's payee -> account order.
    let risk_accounts=err(tx.query("SELECT a.account_id,a.is_banned FROM accounts a WHERE a.account_id IN (SELECT l.account_id FROM promotion_payouts p JOIN promotion_settlements s USING(environment,settlement_id) JOIN promotion_leaders l USING(environment,campaign_id,leader_id) WHERE p.environment=$1 AND p.payout_id=$2 UNION SELECT x.account_id FROM promotion_payouts p JOIN promotion_settlements s USING(environment,settlement_id) JOIN promotion_attributions x USING(environment,campaign_id,leader_id) WHERE p.environment=$1 AND p.payout_id=$2 UNION SELECT m->>'accountId' FROM promotion_payouts p JOIN promotion_settlements s USING(environment,settlement_id) CROSS JOIN LATERAL jsonb_array_elements(COALESCE(s.snapshot_json#>'{input,members}','[]'::jsonb)) m WHERE p.environment=$1 AND p.payout_id=$2) ORDER BY a.account_id FOR SHARE OF a",&[&env,&payout]))?;
    if risk_accounts.iter().any(|a| a.get::<_, bool>(1)) {
        return Err("promotionRiskReviewRequired".into());
    }
    let member_banned:bool=err(tx.query_one("SELECT EXISTS(SELECT 1 FROM promotion_payouts p JOIN promotion_settlements s USING(environment,settlement_id) JOIN promotion_attributions x USING(environment,campaign_id,leader_id) JOIN accounts a ON a.account_id=x.account_id WHERE p.environment=$1 AND p.payout_id=$2 AND a.is_banned) OR EXISTS(SELECT 1 FROM promotion_payouts p JOIN promotion_settlements s USING(environment,settlement_id) CROSS JOIN LATERAL jsonb_array_elements(COALESCE(s.snapshot_json#>'{input,members}','[]'::jsonb)) m JOIN accounts a ON a.account_id=m->>'accountId' WHERE p.environment=$1 AND p.payout_id=$2 AND a.is_banned)",&[&env,&payout]))?.get(0);
    if member_banned {
        return Err("promotionRiskReviewRequired".into());
    }
    let credential=err(tx.query_opt("SELECT i.credential_id,pa.version,pa.cooldown_until_ms,s.campaign_id FROM promotion_payouts p JOIN promotion_settlements s USING(environment,settlement_id) JOIN promotion_leaders l USING(environment,campaign_id,leader_id) JOIN promotion_payout_addresses pa ON pa.environment=p.environment AND pa.account_id=l.account_id AND pa.version=p.address_version AND pa.wallet_address=p.destination AND pa.network=p.network JOIN identity_credentials i ON i.credential_id=pa.verified_credential_id AND i.account_id=l.account_id AND i.credential_subject=pa.wallet_address WHERE p.environment=$1 AND p.payout_id=$2 AND i.credential_kind='sui_wallet' AND i.revoked_at_ms IS NULL FOR SHARE OF i",&[&env,&payout]))?.ok_or("promotionWalletProofRequired")?;
    if super::production_gate::test_micro_enabled(env, &row.get::<_, Value>(4))?
        && signed(now)? < credential.get::<_, i64>(2)
        && !super::production_gate::first_address_exempt(
            tx,
            env,
            &credential.get::<_, String>(3),
            credential.get(1),
        )?
    {
        return Err("promotionWalletCooldown".into());
    }
    let basis=err(tx.query_one("SELECT s.campaign_id,s.snapshot_json,s.total_micro_usd,s.reserve_micro_usd,s.payable_cents,p.amount_micro_usd,COALESCE((SELECT -sum(e.amount_micro_usd) FROM promotion_commission_entries e WHERE e.environment=s.environment AND e.settlement_id=s.settlement_id),0)::bigint FROM promotion_payouts p JOIN promotion_settlements s USING(environment,settlement_id) WHERE p.environment=$1 AND p.payout_id=$2",&[&env,&payout]))?;
    let available = super::store::settlement_payable_micro_usd(
        tx,
        env,
        &basis.get::<_, String>(0),
        &basis.get::<_, Value>(1),
        basis.get(2),
        basis.get(3),
        basis.get(4),
    )?
    .checked_sub(basis.get::<_, i64>(6))
    .ok_or("promotionAmountOverflow")?;
    if available <= 0 || available != basis.get::<_, i64>(5) {
        return Err("promotionPayoutBalanceChanged".into());
    }
    Ok(())
}
impl PromotionPayoutJournal {
    pub fn new(database_url: impl Into<String>, environment: &str) -> Result<Self, String> {
        let database_url = database_url.into();
        if database_url.trim().is_empty()
            || !["development", "test", "production"].contains(&environment)
        {
            return Err("promotionConfigurationInvalid".into());
        }
        Ok(Self {
            database_url,
            environment: environment.into(),
        })
    }
    /// Called after draft creation and before a different payout reviewer acts.
    pub fn freeze_intent(
        &self,
        account: &str,
        intent: &PromotionPayoutIntent,
        evidence: &str,
        allow_mainnet: bool,
        now: u64,
    ) -> Result<Value, String> {
        intent.validate(&self.environment, allow_mainnet)?;
        let sha = intent.sha256()?;
        if evidence.trim().is_empty() || evidence.len() > 512 {
            return Err("promotionEvidenceRequired".into());
        }
        with_account_store_postgres_client(&self.database_url, |client| {
            let mut tx = err(client.transaction())?;
            actor(&mut tx, &self.environment, account, "payoutSubmit")?;
            lock_payee_for_payout(&mut tx, &self.environment, &intent.payout_id)?;
            let row=err(tx.query_opt("SELECT p.status,p.network,p.coin_type,p.destination,p.address_version,p.amount_micro_usd,p.created_by,s.snapshot_sha256,s.status,s.campaign_id,p.settlement_id,s.created_by,s.reviewed_by FROM promotion_payouts p JOIN promotion_settlements s USING(environment,settlement_id) WHERE p.environment=$1 AND p.payout_id=$2 FOR UPDATE OF p,s",&[&self.environment,&intent.payout_id]))?.ok_or("promotionPayoutMissing")?;
            if row.get::<_, String>(0) != "draft"
                || row.get::<_, String>(8) != "approved"
                || row.get::<_, String>(6) != account
                || row
                    .get::<_, Option<String>>(12)
                    .is_none_or(|v| v == row.get::<_, String>(11))
            {
                return Err("promotionPayoutStateInvalid".into());
            }
            if row.get::<_, String>(1) != intent.network
                || row.get::<_, String>(2) != intent.coin_type
                || row.get::<_, String>(3) != intent.destination
                || row.get::<_, i64>(4).to_string() != intent.address_version
                || row.get::<_, i64>(5).to_string() != intent.amount_micro_usd
                || row.get::<_, String>(7) != intent.settlement_sha256
                || row.get::<_, String>(9) != intent.campaign_id
                || row.get::<_, String>(10) != intent.settlement_id
            {
                return Err("promotionPayoutIntentChanged".into());
            }
            readiness(
                &mut tx,
                &self.environment,
                &intent.payout_id,
                Some(intent),
                now,
            )?;
            if let Some(existing)=err(tx.query_opt("SELECT intent_sha256 FROM promotion_payout_intents WHERE environment=$1 AND payout_id=$2",&[&self.environment,&intent.payout_id]))? {
                if existing.get::<_,String>(0)!=sha{return Err("promotionPayoutIntentChanged".into());}return Ok(json!({"payoutId":intent.payout_id,"intentSha256":sha,"status":"draft"}));
            }
            err(tx.execute("INSERT INTO promotion_payout_intents(environment,payout_id,intent_sha256,intent_json,settlement_sha256,created_by,created_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7)",&[&self.environment,&intent.payout_id,&sha,&serde_json::to_value(intent).map_err(|_|"promotionInputInvalid")?,&intent.settlement_sha256,&account,&signed(now)?]))?;
            let result = json!({"payoutId":intent.payout_id,"intentSha256":sha,"status":"draft"});
            audit(
                &mut tx,
                &self.environment,
                account,
                "payout.intent_frozen",
                &intent.payout_id,
                evidence,
                &result,
                now,
            )?;
            err(tx.commit())?;
            Ok(result)
        })
    }
    pub fn approve_intent(
        &self,
        account: &str,
        payout: &str,
        sha: &str,
        evidence: &str,
        now: u64,
    ) -> Result<Value, String> {
        if !hash(sha) || evidence.trim().is_empty() || evidence.len() > 512 {
            return Err("promotionEvidenceRequired".into());
        }
        with_account_store_postgres_client(&self.database_url, |client| {
            let mut tx = err(client.transaction())?;
            actor(&mut tx, &self.environment, account, "payoutReview")?;
            lock_payee_for_payout(&mut tx, &self.environment, payout)?;
            let row=err(tx.query_opt("SELECT p.status,p.created_by,i.intent_sha256,i.created_by,i.reviewed_by FROM promotion_payouts p JOIN promotion_payout_intents i USING(environment,payout_id) WHERE p.environment=$1 AND p.payout_id=$2 FOR UPDATE OF p,i",&[&self.environment,&payout]))?.ok_or("promotionPayoutIntentMissing")?;
            if row.get::<_, String>(1) == account || row.get::<_, String>(3) == account {
                return Err("promotionDistinctReviewerRequired".into());
            }
            if row.get::<_, String>(2) != sha {
                return Err("promotionPayoutIntentChanged".into());
            }
            readiness(&mut tx, &self.environment, payout, None, now)?;
            if row.get::<_, Option<String>>(4).as_deref() == Some(account)
                && row.get::<_, String>(0) == "awaiting_approval"
            {
                return Ok(
                    json!({"payoutId":payout,"intentSha256":sha,"status":"awaiting_approval"}),
                );
            }
            if row.get::<_, String>(0) != "draft" || row.get::<_, Option<String>>(4).is_some() {
                return Err("promotionPayoutStateInvalid".into());
            }
            err(tx.execute("UPDATE promotion_payout_intents SET reviewed_by=$3,reviewed_at_ms=$4 WHERE environment=$1 AND payout_id=$2",&[&self.environment,&payout,&account,&signed(now)?]))?;
            err(tx.execute("UPDATE promotion_payouts SET status='awaiting_approval',reviewed_by=$3 WHERE environment=$1 AND payout_id=$2",&[&self.environment,&payout,&account]))?;
            let result = json!({"payoutId":payout,"intentSha256":sha,"status":"awaiting_approval"});
            audit(
                &mut tx,
                &self.environment,
                account,
                "payout.intent_approved",
                payout,
                evidence,
                &result,
                now,
            )?;
            err(tx.commit())?;
            Ok(result)
        })
    }
    /// Execution-worker only: the Node adapter validates signature and exactly
    /// rebuilds frozen bytes before calling this. Never expose directly to clients.
    pub fn reserve_signed_attempt(
        &self,
        account: &str,
        intent: &PromotionPayoutIntent,
        signed_tx: &PromotionSignedPayout,
        allow_mainnet: bool,
        now: u64,
    ) -> Result<Value, String> {
        intent.validate(&self.environment, allow_mainnet)?;
        let sha = intent.sha256()?;
        if sha != signed_tx.intent_sha256
            || !digest(&signed_tx.digest)
            || signed_tx.transaction_bytes.is_empty()
            || signed_tx.transaction_bytes.len() > 65536
            || signed_tx.signatures.len() != 1
            || signed_tx.signatures[0].is_empty()
            || signed_tx.signatures[0].len() > 16384
        {
            return Err("promotionPayoutSignedIntentMismatch".into());
        }
        with_account_store_postgres_client(&self.database_url, |client| {
            let mut tx = err(client.transaction())?;
            actor(&mut tx, &self.environment, account, "payoutSubmit")?;
            lock_payee_for_payout(&mut tx, &self.environment, &intent.payout_id)?;
            // Lock parent first consistently with approval/address/refund paths.
            let p=err(tx.query_opt("SELECT p.status,p.tx_digest,p.created_by,p.reviewed_by,i.intent_sha256,i.reviewed_by,s.snapshot_sha256,s.status,p.amount_micro_usd,s.payable_cents,s.settlement_id,p.destination,p.address_version,l.account_id,s.snapshot_json,s.total_micro_usd,s.reserve_micro_usd FROM promotion_payouts p JOIN promotion_payout_intents i USING(environment,payout_id) JOIN promotion_settlements s USING(environment,settlement_id) JOIN promotion_leaders l USING(environment,campaign_id,leader_id) WHERE p.environment=$1 AND p.payout_id=$2 FOR UPDATE OF p,s,i",&[&self.environment,&intent.payout_id]))?.ok_or("promotionPayoutIntentMissing")?;
            if p.get::<_, String>(4) != sha
                || p.get::<_, Option<String>>(5).is_none()
                || p.get::<_, Option<String>>(3).is_none()
                || p.get::<_, String>(6) != intent.settlement_sha256
                || p.get::<_, String>(7) != "approved"
            {
                return Err("promotionPayoutApprovalRequired".into());
            }
            if let Some(previous)=err(tx.query_opt("SELECT tx_digest,intent_sha256,transaction_bytes_base64,signatures_json,status FROM promotion_payout_attempts WHERE environment=$1 AND payout_id=$2 FOR UPDATE",&[&self.environment,&intent.payout_id]))? {
                if previous.get::<_,String>(0)!=signed_tx.digest||previous.get::<_,String>(1)!=sha||previous.get::<_,String>(2)!=signed_tx.transaction_bytes||previous.get::<_,Value>(3)!=json!(signed_tx.signatures){return Err("promotionPayoutAttemptConflict".into());}
                return Ok(json!({"digest":signed_tx.digest,"intentSha256":sha,"status":previous.get::<_,String>(4),"maySubmit":false}));
            }
            if p.get::<_, String>(0) != "awaiting_approval" {
                return Err("promotionPayoutStateInvalid".into());
            }
            readiness(
                &mut tx,
                &self.environment,
                &intent.payout_id,
                Some(intent),
                now,
            )?;
            let risk_clear:bool=err(tx.query_one("SELECT EXISTS(SELECT 1 FROM promotion_leaders l JOIN accounts a ON a.account_id=l.account_id WHERE l.environment=$1 AND l.campaign_id=$2 AND l.account_id=$3 AND l.status='approved' AND NOT a.is_banned) AND NOT EXISTS(SELECT 1 FROM promotion_pending_reversals WHERE environment=$1 AND status='pending')",&[&self.environment,&intent.campaign_id,&p.get::<_,String>(13)]))?.get(0);
            if !risk_clear {
                return Err("promotionPayoutRiskReviewRequired".into());
            }
            let reversals:i64=err(tx.query_one("SELECT COALESCE(-sum(amount_micro_usd),0)::bigint FROM promotion_commission_entries WHERE environment=$1 AND settlement_id=$2",&[&self.environment,&intent.settlement_id]))?.get(0);
            let available = super::store::settlement_payable_micro_usd(
                &mut tx,
                &self.environment,
                &intent.campaign_id,
                &p.get::<_, Value>(14),
                p.get(15),
                p.get(16),
                p.get(9),
            )?
            .checked_sub(reversals)
            .ok_or("promotionAmountOverflow")?;
            if available != p.get::<_, i64>(8) || available.to_string() != intent.amount_micro_usd {
                return Err("promotionPayoutBalanceChanged".into());
            }
            let current=err(tx.query_one("SELECT version,wallet_address,network FROM promotion_payout_addresses WHERE environment=$1 AND account_id=$2 ORDER BY version DESC LIMIT 1",&[&self.environment,&p.get::<_,String>(13)]))?;
            if current.get::<_, i64>(0).to_string() != intent.address_version
                || current.get::<_, String>(1) != intent.destination
                || current.get::<_, String>(2) != intent.network
            {
                return Err("promotionPayoutAddressChanged".into());
            }
            err(tx.execute("INSERT INTO promotion_payout_attempts(environment,payout_id,intent_sha256,network,tx_digest,transaction_bytes_base64,signatures_json,status,created_by,created_at_ms,updated_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,'reserved',$8,$9,$9)",&[&self.environment,&intent.payout_id,&sha,&intent.network,&signed_tx.digest,&signed_tx.transaction_bytes,&json!(signed_tx.signatures),&account,&signed(now)?]))?;
            err(tx.execute("UPDATE promotion_payouts SET status='unknown',tx_digest=$3 WHERE environment=$1 AND payout_id=$2",&[&self.environment,&intent.payout_id,&signed_tx.digest]))?;
            let result = json!({"digest":signed_tx.digest,"intentSha256":sha,"status":"reserved","maySubmit":true});
            audit(
                &mut tx,
                &self.environment,
                account,
                "payout.signed_reserved",
                &intent.payout_id,
                &signed_tx.digest,
                &result,
                now,
            )?;
            err(tx.commit())?;
            Ok(result)
        })
    }
    pub fn mark_submitted(
        &self,
        account: &str,
        payout: &str,
        digest: &str,
        now: u64,
    ) -> Result<(), String> {
        with_account_store_postgres_client(&self.database_url, |client| {
            let mut tx = err(client.transaction())?;
            actor(&mut tx, &self.environment, account, "payoutSubmit")?;
            lock_payee_for_payout(&mut tx, &self.environment, payout)?;
            // A refund may have completed while the worker was checking gas.
            // Revalidate the frozen payable basis at the last durable send gate.
            let basis=err(tx.query_opt("SELECT p.amount_micro_usd,p.destination,p.address_version,s.payable_cents,i.intent_json,l.account_id,s.snapshot_json,s.total_micro_usd,s.reserve_micro_usd,s.campaign_id FROM promotion_payouts p JOIN promotion_settlements s USING(environment,settlement_id) JOIN promotion_payout_intents i USING(environment,payout_id) JOIN promotion_leaders l USING(environment,campaign_id,leader_id) WHERE p.environment=$1 AND p.payout_id=$2 FOR UPDATE OF p,s,i",&[&self.environment,&payout]))?.ok_or("promotionPayoutIntentMissing")?;
            let intent: Value = basis.get(4);
            let reversals:i64=err(tx.query_one("SELECT COALESCE(-sum(amount_micro_usd),0)::bigint FROM promotion_commission_entries WHERE environment=$1 AND settlement_id=(SELECT settlement_id FROM promotion_payouts WHERE environment=$1 AND payout_id=$2)",&[&self.environment,&payout]))?.get(0);
            let available = super::store::settlement_payable_micro_usd(
                &mut tx,
                &self.environment,
                &basis.get::<_, String>(9),
                &basis.get::<_, Value>(6),
                basis.get(7),
                basis.get(8),
                basis.get(3),
            )?
            .checked_sub(reversals)
            .ok_or("promotionAmountOverflow")?;
            if available <= 0
                || available != basis.get::<_, i64>(0)
                || intent.get("amountMicroUsd").and_then(Value::as_str)
                    != Some(available.to_string().as_str())
            {
                return Err("promotionPayoutBalanceChanged".into());
            }
            let pending:bool=err(tx.query_one("SELECT EXISTS(SELECT 1 FROM promotion_pending_reversals WHERE environment=$1 AND status='pending')",&[&self.environment]))?.get(0);
            if pending {
                return Err("promotionPayoutRiskReviewRequired".into());
            }
            let current=err(tx.query_opt("SELECT version,wallet_address,network FROM promotion_payout_addresses WHERE environment=$1 AND account_id=$2 ORDER BY version DESC LIMIT 1",&[&self.environment,&basis.get::<_,String>(5)]))?.ok_or("promotionWalletProofRequired")?;
            if current.get::<_, i64>(0) != basis.get::<_, i64>(2)
                || current.get::<_, String>(1) != basis.get::<_, String>(1)
                || intent.get("network").and_then(Value::as_str)
                    != Some(current.get::<_, String>(2).as_str())
            {
                return Err("promotionPayoutAddressChanged".into());
            }
            readiness(&mut tx, &self.environment, payout, None, now)?;
            let changed=err(tx.execute("UPDATE promotion_payout_attempts SET status='submitted',submitted_at_ms=$5,updated_at_ms=$5 WHERE environment=$1 AND payout_id=$2 AND tx_digest=$3 AND created_by=$4 AND status='reserved'",&[&self.environment,&payout,&digest,&account,&signed(now)?]))?;
            if changed != 1 {
                return Err("promotionPayoutStateInvalid".into());
            }
            err(tx.execute("UPDATE promotion_payouts SET status='submitted' WHERE environment=$1 AND payout_id=$2 AND tx_digest=$3",&[&self.environment,&payout,&digest]))?;
            err(tx.commit())?;
            Ok(())
        })
    }
    /// Trusted verifier observations only. Unknown cannot overwrite a final state.
    pub fn record_outcome(
        &self,
        account: &str,
        payout: &str,
        outcome: &Value,
        now: u64,
    ) -> Result<Value, String> {
        let digest = outcome
            .get("digest")
            .and_then(Value::as_str)
            .filter(|v| self::digest(v))
            .ok_or("promotionPayoutDigestInvalid")?;
        let status = outcome
            .get("status")
            .and_then(Value::as_str)
            .filter(|v| ["unknown", "confirmed", "failed"].contains(v))
            .ok_or("promotionPayoutStateInvalid")?;
        with_account_store_postgres_client(&self.database_url, |client| {
            let mut tx = err(client.transaction())?;
            actor(&mut tx, &self.environment, account, "payoutSubmit")?;
            // Keep the parent -> attempt lock order used by reserve/submission.
            err(tx.query_opt("SELECT payout_id FROM promotion_payouts WHERE environment=$1 AND payout_id=$2 FOR UPDATE",&[&self.environment,&payout]))?.ok_or("promotionPayoutMissing")?;
            let row=err(tx.query_opt("SELECT a.tx_digest,a.intent_sha256,a.status,i.intent_json FROM promotion_payout_attempts a JOIN promotion_payout_intents i USING(environment,payout_id) WHERE a.environment=$1 AND a.payout_id=$2 FOR UPDATE OF a",&[&self.environment,&payout]))?.ok_or("promotionPayoutAttemptMissing")?;
            if row.get::<_, String>(0) != digest {
                return Err("promotionPayoutAttemptConflict".into());
            }
            if ["confirmed", "failed"].contains(&row.get::<_, String>(2).as_str()) {
                return Ok(json!({"digest":digest,"status":row.get::<_,String>(2)}));
            }
            if status == "confirmed" {
                let i: Value = row.get(3);
                if outcome.get("intentSha256").and_then(Value::as_str)
                    != Some(row.get::<_, String>(1).as_str())
                    || outcome.get("destination") != i.get("destination")
                    || outcome.get("coinType") != i.get("coinType")
                    || outcome.get("coinAmountAtomic") != i.get("coinAmountAtomic")
                    || outcome
                        .get("gasUsedAtomic")
                        .and_then(Value::as_str)
                        .is_none()
                {
                    return Err("promotionPayoutTransferMismatch".into());
                }
            }
            err(tx.execute("INSERT INTO promotion_payout_chain_observations(environment,payout_id,tx_digest,result_json,observed_at_ms) VALUES($1,$2,$3,$4,$5)",&[&self.environment,&payout,&digest,&outcome,&signed(now)?]))?;
            err(tx.execute("UPDATE promotion_payout_attempts SET status=$3,updated_at_ms=$4 WHERE environment=$1 AND payout_id=$2",&[&self.environment,&payout,&status,&signed(now)?]))?;
            err(tx.execute("UPDATE promotion_payouts SET status=$3 WHERE environment=$1 AND payout_id=$2 AND tx_digest=$4",&[&self.environment,&payout,&status,&digest]))?;
            audit(
                &mut tx,
                &self.environment,
                account,
                "payout.chain_observed",
                payout,
                digest,
                outcome,
                now,
            )?;
            err(tx.commit())?;
            Ok(outcome.clone())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn payout_integer_input_never_uses_float_or_exponent() {
        for v in ["1.0", "1e3", "01", "0", "-1", "18446744073709551616"] {
            assert!(int(v).is_err(), "{v}");
        }
        assert_eq!(int("18446744073709551615").unwrap(), u64::MAX);
    }
    #[test]
    fn payout_canonical_hash_has_stable_key_order() {
        assert_eq!(
            canonical(&json!({"z":"9007199254740993","a":{"z":1,"a":2}})).unwrap(),
            "{\"a\":{\"a\":2,\"z\":1},\"z\":\"9007199254740993\"}"
        );
    }
}
