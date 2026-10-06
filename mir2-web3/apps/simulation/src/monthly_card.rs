//! Account-wide, wall-clock monthly access. Codes are issued by a trusted operator;
//! clients can redeem an issued code, never choose an account, duration or expiry.
use crate::config::SimulationConfig;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

pub const MONTHLY_CARD_DURATION_MS: u64 = 30 * 24 * 60 * 60 * 1_000;
const MAX_CODES_PER_ACCOUNT: usize = 4096;

#[derive(Clone, Default)]
pub struct MonthlyCardPolicy {
    pub required: bool,
    code_validity_ms: Option<u64>,
    issuance_key: Option<Arc<Vec<u8>>>,
}
impl std::fmt::Debug for MonthlyCardPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MonthlyCardPolicy")
            .field("required", &self.required)
            .field("issuance_key_configured", &self.issuance_key.is_some())
            .finish()
    }
}
impl MonthlyCardPolicy {
    pub fn new(required: bool, issuance_key: Option<&str>) -> Result<Self, String> {
        if issuance_key.is_some_and(|key| key.len() < 32) {
            return Err("monthlyCardKeyTooShort".into());
        }
        if required && issuance_key.is_none() {
            return Err("monthlyCardKeyRequired".into());
        }
        Ok(Self {
            required,
            code_validity_ms: None,
            issuance_key: issuance_key.map(|key| Arc::new(key.as_bytes().to_vec())),
        })
    }
    pub fn with_code_validity_days(mut self, days: Option<u16>) -> Result<Self, String> {
        if days.is_some_and(|days| days == 0 || days > 3650) {
            return Err("monthlyCardCodeValidityInvalid".into());
        }
        self.code_validity_ms = days.map(|days| u64::from(days) * 86_400_000);
        Ok(self)
    }
    pub fn from_env() -> Result<Self, String> {
        let required = match std::env::var("MIR2_MONTHLY_CARD_REQUIRED") {
            Err(std::env::VarError::NotPresent) => false,
            Ok(value) if matches!(value.as_str(), "0" | "false") => false,
            Ok(value) if matches!(value.as_str(), "1" | "true") => true,
            _ => return Err("invalid MIR2_MONTHLY_CARD_REQUIRED; expected 0 or 1".into()),
        };
        let key = match std::env::var("MIR2_MONTHLY_CARD_ISSUANCE_KEY") {
            Ok(value) => Some(value),
            Err(std::env::VarError::NotPresent) => None,
            _ => return Err("monthlyCardKeyInvalid".into()),
        };
        let validity = match std::env::var("MIR2_MONTHLY_CARD_CODE_VALIDITY_DAYS") {
            Err(std::env::VarError::NotPresent) => None,
            Ok(value) => Some(
                value
                    .parse::<u16>()
                    .map_err(|_| "monthlyCardCodeValidityInvalid")?,
            ),
            _ => return Err("monthlyCardCodeValidityInvalid".into()),
        };
        Self::new(required, key.as_deref())?.with_code_validity_days(validity)
    }
    fn code(&self, account_id: &str, request_id: &str) -> Result<String, String> {
        let key = self
            .issuance_key
            .as_ref()
            .ok_or("monthlyCardIssuanceUnavailable")?;
        let mut mac =
            <Hmac<Sha256> as KeyInit>::new_from_slice(key).map_err(|_| "monthlyCardKeyInvalid")?;
        mac.update(b"mir2-monthly-card-v1\0");
        mac.update(account_id.as_bytes());
        mac.update(b"\0");
        mac.update(request_id.as_bytes());
        Ok(format!(
            "MC1-{}",
            URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
        ))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MonthlyCardLedger {
    pub expires_at_ms: u64,
    pub codes: BTreeMap<String, MonthlyCardCodeRecord>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MonthlyCardCodeRecord {
    pub request_id: String,
    pub issued_at_ms: u64,
    pub redeem_before_ms: Option<u64>,
    pub redeemed_at_ms: Option<u64>,
    pub credited_until_ms: Option<u64>,
}
impl MonthlyCardLedger {
    pub fn validate(&self) -> Result<(), String> {
        if self.codes.len() > MAX_CODES_PER_ACCOUNT {
            return Err("monthlyCardLedgerFull".into());
        }
        let mut request_ids = std::collections::BTreeSet::new();
        let mut last_expiry = 0;
        for (hash, code) in &self.codes {
            if hash.len() != 64
                || !hash.bytes().all(|b| b.is_ascii_hexdigit())
                || !valid_request_id(&code.request_id)
                || !request_ids.insert(&code.request_id)
                || code
                    .redeem_before_ms
                    .is_some_and(|before| before <= code.issued_at_ms)
            {
                return Err("monthlyCardLedgerInvalid".into());
            }
            match (code.redeemed_at_ms, code.credited_until_ms) {
                (None, None) => {}
                (Some(at), Some(until))
                    if at >= code.issued_at_ms
                        && code.redeem_before_ms.is_none_or(|before| at < before)
                        && until
                            >= at
                                .checked_add(MONTHLY_CARD_DURATION_MS)
                                .ok_or("monthlyCardTimeOverflow")? =>
                {
                    last_expiry = last_expiry.max(until);
                }
                _ => return Err("monthlyCardLedgerInvalid".into()),
            }
        }
        if self.expires_at_ms != last_expiry {
            return Err("monthlyCardLedgerInvalid".into());
        }
        Ok(())
    }
}
fn valid_request_id(id: &str) -> bool {
    (12..=96).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
}
fn code_hash(code: &str) -> Result<String, String> {
    let code = code.trim();
    let body = code.strip_prefix("MC1-").ok_or("monthlyCardCodeInvalid")?;
    if body.len() != 43 {
        return Err("monthlyCardCodeInvalid".into());
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(body)
        .map_err(|_| "monthlyCardCodeInvalid")?;
    if bytes.len() != 32 || URL_SAFE_NO_PAD.encode(&bytes) != body {
        return Err("monthlyCardCodeInvalid".into());
    }
    Ok(Sha256::digest(code.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonthlyCardStatus {
    pub required: bool,
    pub active: bool,
    pub expires_at_ms: Option<u64>,
    pub server_now_ms: u64,
    pub remaining_ms: u64,
    pub can_enter_game: bool,
}
impl MonthlyCardStatus {
    fn new(required: bool, expires: u64, now: u64) -> Self {
        let active = expires > now;
        Self {
            required,
            active,
            expires_at_ms: (expires != 0).then_some(expires),
            server_now_ms: now,
            remaining_ms: expires.saturating_sub(now),
            can_enter_game: !required || active,
        }
    }
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonthlyCardIssued {
    pub code: String,
    pub duration_days: u16,
    pub redeem_before_ms: Option<u64>,
    pub replayed: bool,
}
impl std::fmt::Debug for MonthlyCardIssued {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MonthlyCardIssued")
            .field("code", &"[REDACTED]")
            .field("replayed", &self.replayed)
            .finish()
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonthlyCardRedeemed {
    pub replayed: bool,
    pub status: MonthlyCardStatus,
}

pub fn monthly_card_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis().min(u128::from(u64::MAX)) as u64)
}
impl SimulationConfig {
    pub fn with_monthly_card_policy(mut self, policy: MonthlyCardPolicy) -> Self {
        self.monthly_card_policy = policy;
        self
    }
    pub fn monthly_card_status(
        &self,
        account_id: &str,
        now_ms: u64,
    ) -> Result<MonthlyCardStatus, String> {
        let store = self
            .account_store
            .lock()
            .map_err(|_| "monthlyCardStoreUnavailable")?;
        let account = store
            .accounts
            .get(account_id)
            .ok_or("monthlyCardAccountMissing")?;
        let expires = match &account.monthly_card {
            Some(card) => {
                card.validate()?;
                card.expires_at_ms
            }
            None => 0,
        };
        Ok(MonthlyCardStatus::new(
            self.monthly_card_policy.required,
            expires,
            now_ms,
        ))
    }
    /// Loaded records and every mutation are validated by the store. Hot movement
    /// and tick admission only need the deadline, not a scan of all old receipts.
    pub fn monthly_card_can_enter_cached(
        &self,
        account_id: &str,
        now_ms: u64,
    ) -> Result<bool, String> {
        if !self.monthly_card_policy.required {
            return Ok(true);
        }
        let store = self
            .account_store
            .lock()
            .map_err(|_| "monthlyCardStoreUnavailable")?;
        let account = store
            .accounts
            .get(account_id)
            .ok_or("monthlyCardAccountMissing")?;
        Ok(account
            .monthly_card
            .as_ref()
            .is_some_and(|card| card.expires_at_ms > now_ms))
    }
    pub fn refresh_monthly_card_status(
        &self,
        account_id: &str,
        now_ms: u64,
    ) -> Result<MonthlyCardStatus, String> {
        self.refresh_account_store_account(account_id)?;
        self.monthly_card_status(account_id, now_ms)
    }
    /// Trusted administration only. A durable caller request ID makes response-loss
    /// retries return the same code. Plain codes and issuance keys are never stored.
    pub fn issue_monthly_card(
        &self,
        account_id: &str,
        request_id: &str,
        now_ms: u64,
    ) -> Result<MonthlyCardIssued, String> {
        if !valid_request_id(request_id) {
            return Err("monthlyCardRequestIdInvalid".into());
        }
        let code = self.monthly_card_policy.code(account_id, request_id)?;
        let hash = code_hash(&code)?;
        self.refresh_account_store_account(account_id)?;
        self.commit_account_store_transaction(&[account_id.to_owned()], |store| {
            let account = store
                .accounts
                .get_mut(account_id)
                .ok_or("monthlyCardAccountMissing")?;
            if account.active_ban(now_ms).is_some() {
                return Err("monthlyCardAccountBanned".into());
            }
            let card = account
                .monthly_card
                .get_or_insert_with(MonthlyCardLedger::default);
            card.validate()?;
            if let Some((old_hash, existing)) = card
                .codes
                .iter()
                .find(|(_, row)| row.request_id == request_id)
            {
                if old_hash != &hash {
                    return Err("monthlyCardIssuanceKeyChanged".into());
                }
                return Ok(MonthlyCardIssued {
                    code,
                    duration_days: 30,
                    redeem_before_ms: existing.redeem_before_ms,
                    replayed: true,
                });
            }
            if card.codes.len() >= MAX_CODES_PER_ACCOUNT {
                return Err("monthlyCardLedgerFull".into());
            }
            let redeem_before_ms = self
                .monthly_card_policy
                .code_validity_ms
                .map(|ttl| now_ms.checked_add(ttl).ok_or("monthlyCardTimeOverflow"))
                .transpose()?;
            card.codes.insert(
                hash,
                MonthlyCardCodeRecord {
                    request_id: request_id.to_owned(),
                    issued_at_ms: now_ms,
                    redeem_before_ms,
                    redeemed_at_ms: None,
                    credited_until_ms: None,
                },
            );
            Ok(MonthlyCardIssued {
                code,
                duration_days: 30,
                redeem_before_ms,
                replayed: false,
            })
        })
    }
    /// `account_id` MUST come from verified login state, not the redeem request.
    pub fn redeem_monthly_card(
        &self,
        account_id: &str,
        code: &str,
        now_ms: u64,
    ) -> Result<MonthlyCardRedeemed, String> {
        let hash = code_hash(code)?;
        self.refresh_account_store_account(account_id)?;
        self.commit_account_store_transaction(&[account_id.to_owned()], |store| {
            let account = store
                .accounts
                .get_mut(account_id)
                .ok_or("monthlyCardAccountMissing")?;
            if account.active_ban(now_ms).is_some() {
                return Err("monthlyCardAccountBanned".into());
            }
            let card = account
                .monthly_card
                .as_mut()
                .ok_or("monthlyCardCodeInvalid")?;
            card.validate()?;
            let row = card.codes.get_mut(&hash).ok_or("monthlyCardCodeInvalid")?;
            if row.redeemed_at_ms.is_some() {
                return Ok(MonthlyCardRedeemed {
                    replayed: true,
                    status: MonthlyCardStatus::new(
                        self.monthly_card_policy.required,
                        card.expires_at_ms,
                        now_ms,
                    ),
                });
            }
            if now_ms < row.issued_at_ms
                || row.redeem_before_ms.is_some_and(|before| now_ms >= before)
            {
                return Err("monthlyCardCodeExpired".into());
            }
            let expires = card
                .expires_at_ms
                .max(now_ms)
                .checked_add(MONTHLY_CARD_DURATION_MS)
                .ok_or("monthlyCardTimeOverflow")?;
            row.redeemed_at_ms = Some(now_ms);
            row.credited_until_ms = Some(expires);
            card.expires_at_ms = expires;
            card.validate()?;
            Ok(MonthlyCardRedeemed {
                replayed: false,
                status: MonthlyCardStatus::new(self.monthly_card_policy.required, expires, now_ms),
            })
        })
    }
}
