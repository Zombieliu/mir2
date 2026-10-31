//! Sui-native USDC recharge authority. Only a trusted fullnode verifier may
//! confirm a receipt; client-submitted transaction digests are discovery hints.
//! This first release is testnet-only and never treats test USDC as cash.
use crate::billing::{BillingLedger, RechargeAppliedReceipt, RechargeOffer};
use crate::{AccountRecord, AccountStore, SimulationConfig};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const SCHEMA_VERSION: u16 = 14;
pub const TESTNET_CHAIN_IDENTIFIER: &str = "4c78adac";
pub const TESTNET_GENESIS_DIGEST: &str = "69WiPg3DAQiwdxfncX6wYQ2siKwAe6L9BZthQea3JNMD";
pub const USDC_DECIMALS: u8 = 6;
pub const JOB_LEASE_MS: u64 = 120_000;
const MAX_ORDERS: usize = 4_096;
// Updated only from Circle's official native USDC deployment registry.
pub const TESTNET_USDC_COIN_TYPE: &str =
    "0xa1ec7fc00a6f40db9693ad1415d0c193ad3906494428cf252621037bd7117e29::usdc::USDC";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UsdcDomain {
    pub chain_identifier: String,
    pub package_id: String,
    pub config_id: String,
    pub merchant: String,
    pub coin_type: String,
    pub rule_version: u64,
}
impl UsdcDomain {
    pub fn validate(&self) -> Result<(), String> {
        if self.chain_identifier != TESTNET_CHAIN_IDENTIFIER
            || self.coin_type != TESTNET_USDC_COIN_TYPE
            || !address(&self.package_id)
            || !address(&self.config_id)
            || !address(&self.merchant)
            || self.rule_version == 0
        {
            return Err("usdcDomainInvalid".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UsdcOrderState {
    Prepared,
    PayerBound,
    InvoicePrepared,
    InvoiceBound,
    PaidPending,
    Applied,
    Review,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsdcPaymentMode {
    #[default]
    #[serde(rename = "invoice")]
    Invoice,
    #[serde(rename = "direct_transfer")]
    DirectTransfer,
}
impl UsdcPaymentMode {
    pub fn is_invoice(&self) -> bool {
        *self == Self::Invoice
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UsdcPreparedTransaction {
    pub bytes_base64: String,
    pub signature_base64: String,
    pub digest: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UsdcInvoiceBinding {
    pub invoice_id: String,
    pub create_digest: String,
    pub checkpoint: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UsdcJobLease {
    pub worker: String,
    pub until_ms: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerifiedUsdcReceipt {
    pub chain_identifier: String,
    pub package_id: String,
    pub config_id: String,
    pub invoice_id: String,
    pub order_id: String,
    pub account_ref: String,
    pub payer: String,
    pub merchant: String,
    pub coin_type: String,
    pub amount_micro: u64,
    pub frozen_hash: String,
    pub digest: String,
    pub event_seq: u64,
    pub checkpoint: u64,
    pub confirmed_at_ms: u64,
}
/// Finalized native coin transfer facts. These are deliberately not a Move
/// Invoice receipt: there is no invoice object or fabricated event sequence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerifiedNativeUsdcTransferReceipt {
    pub chain_identifier: String,
    pub genesis_digest: String,
    pub order_id: String,
    pub account_ref: String,
    pub payer: String,
    pub merchant: String,
    pub coin_type: String,
    pub amount_micro: u64,
    pub frozen_hash: String,
    pub digest: String,
    pub checkpoint: u64,
    pub confirmed_at_ms: u64,
    pub observed_at_ms: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UsdcRechargeOrder {
    pub id: String,
    pub request_id: String,
    pub character_index: i32,
    pub offer: RechargeOffer,
    pub domain: UsdcDomain,
    pub credit_origin: crate::procurement_fees::CreditOrigin,
    pub account_ref: String,
    pub amount_micro: u64,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
    pub capability_hash: String,
    pub capability_expires_at_ms: u64,
    pub payer: Option<String>,
    #[serde(default, skip_serializing_if = "UsdcPaymentMode::is_invoice")]
    pub payment_mode: UsdcPaymentMode,
    pub state: UsdcOrderState,
    pub prepared_tx: Option<UsdcPreparedTransaction>,
    pub invoice: Option<UsdcInvoiceBinding>,
    pub payment_hint: Option<String>,
    pub receipt: Option<VerifiedUsdcReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transfer_receipt: Option<VerifiedNativeUsdcTransferReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direct_scan_cursor: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub direct_scan_complete: bool,
    pub applied: Option<RechargeAppliedReceipt>,
    pub job_lease: Option<UsdcJobLease>,
    pub review_reason: Option<String>,
}

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn is_false(value: &bool) -> bool {
    !value
}
fn hex32(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn address(s: &str) -> bool {
    s.strip_prefix("0x")
        .is_some_and(|v| hex32(v) && v.bytes().any(|b| b != b'0'))
}
fn key(s: &str, max: usize) -> bool {
    !s.is_empty()
        && s.len() <= max
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
}
fn digest(s: &str) -> bool {
    (32..=64).contains(&s.len())
        && s.bytes()
            .all(|b| b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".contains(&b))
}
fn valid_base64(s: &str, max: usize) -> bool {
    !s.is_empty()
        && s.len() <= max
        && s.len() % 4 == 0
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b))
}
fn valid_scan_cursor(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 4096
        && serde_json::from_str::<serde_json::Value>(value).is_ok_and(|v| v.is_object())
}
pub fn account_ref(account_id: &str, character_index: i32) -> String {
    format!(
        "0x{}",
        hash(
            &serde_json::to_vec(&("MIR2_SUI_USDC_ACCOUNT_V1", account_id, character_index))
                .expect("serializable account reference")
        )
    )
}
pub fn order_ref(order_id: &str) -> String {
    format!("0x{}", hash(order_id.as_bytes()))
}
pub fn frozen_hash(order: &UsdcRechargeOrder) -> Result<String, String> {
    order.frozen_hash()
}
impl UsdcRechargeOrder {
    /// SHA-256 of this exact JSON tuple, independent of transport field order.
    /// The invoice and final payment event carry this immutable 32-byte value.
    pub fn frozen_hash(&self) -> Result<String, String> {
        let payer = self.payer.as_ref().ok_or("usdcPayerMissing")?;
        Ok(format!(
            "0x{}",
            hash(
                &serde_json::to_vec(&(
                    if self.payment_mode.is_invoice() {
                        "MIR2_SUI_USDC_RECHARGE_V1"
                    } else {
                        "MIR2_SUI_USDC_NATIVE_TRANSFER_V1"
                    },
                    &self.id,
                    &self.account_ref,
                    self.character_index,
                    &self.offer,
                    &self.domain,
                    self.amount_micro,
                    self.created_at_ms,
                    self.expires_at_ms,
                    payer,
                ))
                .map_err(|_| "usdcEncodingFailed")?
            )
        ))
    }
    pub fn has_payment_receipt(&self) -> bool {
        self.receipt.is_some() || self.transfer_receipt.is_some()
    }
    pub fn payment_confirmed_at_ms(&self) -> Option<u64> {
        self.receipt
            .as_ref()
            .map(|r| r.confirmed_at_ms)
            .or_else(|| self.transfer_receipt.as_ref().map(|r| r.confirmed_at_ms))
    }
    pub fn payment_digest(&self) -> Option<&str> {
        self.receipt
            .as_ref()
            .map(|r| r.digest.as_str())
            .or_else(|| self.transfer_receipt.as_ref().map(|r| r.digest.as_str()))
    }
    pub fn validate(&self) -> Result<(), String> {
        self.offer.validate()?;
        self.domain.validate()?;
        if !self.id.strip_prefix("usdc_").is_some_and(hex32)
            || !key(&self.request_id, 128)
            || self.character_index < 0
            || self.offer.currency != "usd"
            || self.credit_origin != crate::procurement_fees::CreditOrigin::Test
            || self.offer.amount_minor.checked_mul(10_000) != Some(self.amount_micro)
            || !self.account_ref.strip_prefix("0x").is_some_and(hex32)
            || !hex32(&self.capability_hash)
            || self.created_at_ms == 0
            || self.expires_at_ms <= self.created_at_ms
            || self.capability_expires_at_ms <= self.created_at_ms
            || self
                .capability_expires_at_ms
                .saturating_sub(self.created_at_ms)
                > 86_400_000
            || self
                .payer
                .as_deref()
                .is_some_and(|p| !address(p) || p == self.domain.merchant)
            || self.payment_hint.as_deref().is_some_and(|d| !digest(d))
            || self
                .job_lease
                .as_ref()
                .is_some_and(|l| !key(&l.worker, 128) || l.until_ms == 0)
        {
            return Err("usdcOrderInvalid".into());
        }
        if (self.payment_mode.is_invoice()
            && (self.transfer_receipt.is_some()
                || self.direct_scan_cursor.is_some()
                || self.direct_scan_complete))
            || (!self.payment_mode.is_invoice()
                && (self.prepared_tx.is_some() || self.invoice.is_some() || self.receipt.is_some()))
            || self
                .direct_scan_cursor
                .as_deref()
                .is_some_and(|v| !valid_scan_cursor(v))
            || (self.direct_scan_complete && self.direct_scan_cursor.is_none())
        {
            return Err("usdcPaymentModeMismatch".into());
        }
        if let Some(tx) = &self.prepared_tx {
            if !valid_base64(&tx.bytes_base64, 256 * 1024)
                || !valid_base64(&tx.signature_base64, 1024)
                || !digest(&tx.digest)
            {
                return Err("usdcPreparedTransactionInvalid".into());
            }
        }
        if let Some(i) = &self.invoice {
            if !address(&i.invoice_id)
                || i.checkpoint == 0
                || self
                    .prepared_tx
                    .as_ref()
                    .is_none_or(|t| t.digest != i.create_digest)
            {
                return Err("usdcInvoiceBindingInvalid".into());
            }
        }
        if let Some(r) = &self.receipt {
            self.validate_receipt(r)?;
        }
        if let Some(r) = &self.transfer_receipt {
            self.validate_transfer_receipt(r)?;
        }
        if let Some(a) = &self.applied {
            if a.character_index != self.character_index
                || a.credits != self.offer.credits
                || a.credit_before.checked_add(a.credits) != Some(a.credit_after)
                || a.revision == 0
                || self.payment_confirmed_at_ms().is_none_or(|at| a.at_ms < at)
            {
                return Err("usdcApplicationReceiptInvalid".into());
            }
        }
        let tuple = (
            self.payer.is_some(),
            self.prepared_tx.is_some(),
            self.invoice.is_some(),
            self.has_payment_receipt(),
            self.applied.is_some(),
        );
        let expected = if self.payment_mode.is_invoice() {
            match self.state {
                UsdcOrderState::Prepared => (false, false, false, false, false),
                UsdcOrderState::PayerBound => (true, false, false, false, false),
                UsdcOrderState::InvoicePrepared => (true, true, false, false, false),
                UsdcOrderState::InvoiceBound => (true, true, true, false, false),
                UsdcOrderState::PaidPending => (true, true, true, true, false),
                UsdcOrderState::Applied => (true, true, true, true, true),
                UsdcOrderState::Review => tuple,
            }
        } else {
            match self.state {
                UsdcOrderState::PayerBound => (true, false, false, false, false),
                UsdcOrderState::PaidPending => (true, false, false, true, false),
                UsdcOrderState::Applied => (true, false, false, true, true),
                UsdcOrderState::Review => tuple,
                _ => return Err("usdcOrderStateInvalid".into()),
            }
        };
        if tuple != expected
            || (self.state == UsdcOrderState::Review) != self.review_reason.is_some()
            || self.review_reason.as_ref().is_some_and(|r| !key(r, 128))
            || (self.state == UsdcOrderState::Review
                && (self.payer.is_none() || self.has_payment_receipt() || self.applied.is_some()))
        {
            return Err("usdcOrderStateInvalid".into());
        }
        Ok(())
    }
    fn validate_receipt(&self, r: &VerifiedUsdcReceipt) -> Result<(), String> {
        if !self.payment_mode.is_invoice() {
            return Err("usdcPaymentModeMismatch".into());
        }
        let i = self.invoice.as_ref().ok_or("usdcInvoiceMissing")?;
        if r.chain_identifier != self.domain.chain_identifier
            || r.package_id != self.domain.package_id
            || r.config_id != self.domain.config_id
            || r.invoice_id != i.invoice_id
            || r.order_id != self.id
            || r.account_ref != self.account_ref
            || Some(r.payer.as_str()) != self.payer.as_deref()
            || r.merchant != self.domain.merchant
            || r.coin_type != self.domain.coin_type
            || r.amount_micro != self.amount_micro
            || r.frozen_hash != self.frozen_hash()?
            || !digest(&r.digest)
            || r.checkpoint < i.checkpoint
            || r.checkpoint == 0
            || r.confirmed_at_ms < self.created_at_ms
            || r.confirmed_at_ms > self.expires_at_ms
        {
            return Err("usdcPaymentBindingMismatch".into());
        }
        Ok(())
    }
    fn validate_transfer_receipt(
        &self,
        r: &VerifiedNativeUsdcTransferReceipt,
    ) -> Result<(), String> {
        if self.payment_mode != UsdcPaymentMode::DirectTransfer
            || self.receipt.is_some()
            || r.chain_identifier != self.domain.chain_identifier
            || r.genesis_digest != TESTNET_GENESIS_DIGEST
            || r.order_id != self.id
            || r.account_ref != self.account_ref
            || Some(r.payer.as_str()) != self.payer.as_deref()
            || r.merchant != self.domain.merchant
            || r.coin_type != self.domain.coin_type
            || r.amount_micro != self.amount_micro
            || r.frozen_hash != self.frozen_hash()?
            || !digest(&r.digest)
            || r.checkpoint == 0
            || r.confirmed_at_ms < self.created_at_ms
            || r.confirmed_at_ms >= self.expires_at_ms
            || r.observed_at_ms < r.confirmed_at_ms
        {
            return Err("usdcNativeTransferBindingMismatch".into());
        }
        Ok(())
    }
}

pub(crate) fn validate_ledger(ledger: &BillingLedger) -> Result<(), String> {
    if ledger
        .sui_usdc_orders
        .len()
        .checked_add(ledger.orders.len())
        .is_none_or(|n| n > MAX_ORDERS)
    {
        return Err("billingLedgerFull".into());
    }
    let mut requests = BTreeSet::new();
    for (id, order) in &ledger.sui_usdc_orders {
        order.validate()?;
        if id != &order.id || !requests.insert(&order.request_id) {
            return Err("usdcLedgerInvalid".into());
        }
    }
    Ok(())
}
pub(crate) fn validate_store(store: &AccountStore) -> Result<(), String> {
    let mut ids = BTreeSet::new();
    let mut invoices = BTreeSet::new();
    let mut receipts = BTreeSet::new();
    let mut active_creations = BTreeSet::new();
    let mut payment_windows: Vec<&UsdcRechargeOrder> = Vec::new();
    for (account_id, account) in &store.accounts {
        for order in account
            .billing
            .iter()
            .flat_map(|l| l.sui_usdc_orders.values())
        {
            if store.schema_version != SCHEMA_VERSION
                || order.account_ref != account_ref(account_id, order.character_index)
                || !ids.insert(&order.id)
            {
                return Err("usdcStoreInvalid".into());
            }
            if let Some(i) = &order.invoice {
                if !invoices.insert((&order.domain.chain_identifier, &i.invoice_id)) {
                    return Err("usdcInvoiceAlreadyUsed".into());
                }
            }
            if let Some(r) = &order.receipt {
                if !receipts.insert((&r.chain_identifier, &r.digest)) {
                    return Err("usdcReceiptAlreadyUsed".into());
                }
            }
            if let Some(r) = &order.transfer_receipt {
                if !receipts.insert((&r.chain_identifier, &r.digest)) {
                    return Err("usdcReceiptAlreadyUsed".into());
                }
            }
            if order.payer.is_some() && !order.has_payment_receipt() {
                if payment_windows
                    .iter()
                    .any(|other| overlapping_payment_windows(other, order))
                {
                    return Err("usdcNativeTransferOrderAmbiguous".into());
                }
                payment_windows.push(order);
            }
            if creation_in_flight(order)
                && !active_creations
                    .insert((&order.domain.chain_identifier, &order.domain.config_id))
            {
                return Err("usdcCreationBusy".into());
            }
        }
    }
    Ok(())
}
/// A sponsor's mutable SUI gas object cannot fund two concurrent invoice
/// creations. This fence has no TTL: only confirmed creation/failure releases
/// it, regardless of a worker lease or browser capability expiring.
pub fn creation_in_flight(order: &UsdcRechargeOrder) -> bool {
    order.payment_mode.is_invoice()
        && order.prepared_tx.is_some()
        && order.invoice.is_none()
        && !(order.state == UsdcOrderState::Review
            && order.review_reason.as_deref() == Some("createFailed"))
}
pub(crate) fn reserves_capacity(order: &UsdcRechargeOrder) -> bool {
    order.applied.is_none()
}
pub(crate) fn reserves_capacity_at(order: &UsdcRechargeOrder, now_ms: u64) -> bool {
    reserves_capacity(order)
        && !(now_ms >= order.expires_at_ms
            && order.state == UsdcOrderState::Prepared
            && order.payer.is_none()
            && order.prepared_tx.is_none()
            && order.invoice.is_none()
            && order.payment_hint.is_none()
            && !order.has_payment_receipt())
}
fn overlapping_payment_windows(a: &UsdcRechargeOrder, b: &UsdcRechargeOrder) -> bool {
    (a.payment_mode == UsdcPaymentMode::DirectTransfer
        || b.payment_mode == UsdcPaymentMode::DirectTransfer)
        && a.domain.chain_identifier == b.domain.chain_identifier
        && a.payer == b.payer
        && a.domain.merchant == b.domain.merchant
        && a.domain.coin_type == b.domain.coin_type
        && a.amount_micro == b.amount_micro
        && a.created_at_ms < b.expires_at_ms
        && b.created_at_ms < a.expires_at_ms
}
pub(crate) fn ensure_deletable(account: &AccountRecord, character: i32) -> Result<(), String> {
    if account
        .billing
        .iter()
        .flat_map(|l| l.sui_usdc_orders.values())
        .any(|o| {
            o.character_index == character
                && (reserves_capacity(o)
                    || account.saves.get(&character).is_some_and(|s| s.credit != 0))
        })
    {
        return Err("billingCharacterHasUnsettledOrders".into());
    }
    Ok(())
}

fn owned_save(
    account: &AccountRecord,
    character: i32,
) -> Result<&crate::CharacterSaveRecord, String> {
    let c = account
        .characters
        .iter()
        .find(|c| c.index == character)
        .ok_or("billingCharacterMissing")?;
    let save = account
        .saves
        .get(&character)
        .ok_or("billingCharacterSaveMissing")?;
    if save.character.index != c.index
        || save.character.name != c.name
        || save.character.class != c.class
        || save.character.gender != c.gender
    {
        return Err("billingCharacterIdentityMismatch".into());
    }
    Ok(save)
}
fn find_mut<'a>(
    store: &'a mut AccountStore,
    account: &str,
    order: &str,
) -> Result<&'a mut UsdcRechargeOrder, String> {
    store
        .accounts
        .get_mut(account)
        .and_then(|a| a.billing.as_mut())
        .and_then(|b| b.sui_usdc_orders.get_mut(order))
        .ok_or_else(|| "usdcOrderMissing".into())
}
fn capability(order: &UsdcRechargeOrder, supplied: &str, now: u64) -> Result<(), String> {
    if !hex32(supplied)
        || supplied != order.capability_hash
        || now >= order.capability_expires_at_ms
        || now < order.created_at_ms
    {
        return Err("usdcCapabilityInvalid".into());
    }
    Ok(())
}
fn lease(order: &UsdcRechargeOrder, worker: &str, now: u64) -> Result<(), String> {
    if order
        .job_lease
        .as_ref()
        .is_none_or(|l| l.worker != worker || now >= l.until_ms)
    {
        return Err("usdcJobLeaseLost".into());
    }
    Ok(())
}
impl SimulationConfig {
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_usdc_recharge_order(
        &self,
        account_id: &str,
        character_index: i32,
        request_id: &str,
        offer: &RechargeOffer,
        domain: &UsdcDomain,
        capability_hash: &str,
        capability_expires_at_ms: u64,
        expires_at_ms: u64,
        now_ms: u64,
    ) -> Result<UsdcRechargeOrder, String> {
        self.prepare_usdc_order_with_mode(
            account_id,
            character_index,
            request_id,
            offer,
            domain,
            capability_hash,
            capability_expires_at_ms,
            expires_at_ms,
            now_ms,
            UsdcPaymentMode::Invoice,
            None,
        )
    }
    /// The gateway must additionally validate its current session. This second
    /// check binds the exact credential to this account at the durable boundary.
    fn require_native_usdc_payer(
        &self,
        account: &str,
        credential_id: &str,
        payer: &str,
    ) -> Result<(), String> {
        if !address(payer) || !key(credential_id, 128) {
            return Err("usdcVerifiedWalletRequired".into());
        }
        let url = self
            .account_store_database_url
            .clone()
            .filter(|_| {
                self.account_store_database_mode == crate::AccountStoreDatabaseMode::SourceOfTruth
            })
            .ok_or("usdcVerifiedWalletRequired")?;
        let (account, credential_id, payer) = (
            account.to_owned(),
            credential_id.to_owned(),
            payer.to_owned(),
        );
        std::thread::spawn(move || {
            let mut db = postgres::Client::connect(&url, postgres::NoTls).map_err(|_| "usdcWalletAuthorityUnavailable".to_owned())?;
            let active = db.query_opt("SELECT 1 FROM identity_credentials WHERE account_id=$1 AND credential_id=$2 AND credential_kind='sui_wallet' AND credential_subject=$3 AND revoked_at_ms IS NULL", &[&account,&credential_id,&payer])
                .map_err(|_| "usdcWalletAuthorityUnavailable".to_owned())?.is_some();
            if active { Ok(()) } else { Err("usdcVerifiedWalletRequired".to_owned()) }
        }).join().map_err(|_| "usdcWalletAuthorityUnavailable".to_owned())?
    }
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_direct_usdc_recharge_order(
        &self,
        account_id: &str,
        character_index: i32,
        request_id: &str,
        offer: &RechargeOffer,
        domain: &UsdcDomain,
        credential_id: &str,
        payer: &str,
        capability_hash: &str,
        capability_expires_at_ms: u64,
        expires_at_ms: u64,
        now_ms: u64,
    ) -> Result<UsdcRechargeOrder, String> {
        self.require_native_usdc_payer(account_id, credential_id, payer)?;
        self.prepare_usdc_order_with_mode(
            account_id,
            character_index,
            request_id,
            offer,
            domain,
            capability_hash,
            capability_expires_at_ms,
            expires_at_ms,
            now_ms,
            UsdcPaymentMode::DirectTransfer,
            Some(payer),
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn prepare_usdc_order_with_mode(
        &self,
        account_id: &str,
        character_index: i32,
        request_id: &str,
        offer: &RechargeOffer,
        domain: &UsdcDomain,
        capability_hash: &str,
        capability_expires_at_ms: u64,
        expires_at_ms: u64,
        now_ms: u64,
        payment_mode: UsdcPaymentMode,
        payer: Option<&str>,
    ) -> Result<UsdcRechargeOrder, String> {
        offer.validate()?;
        domain.validate()?;
        if offer.currency != "usd"
            || !key(request_id, 128)
            || !hex32(capability_hash)
            || now_ms == 0
            || expires_at_ms <= now_ms
            || expires_at_ms - now_ms > 86_400_000
            || capability_expires_at_ms <= now_ms
            || capability_expires_at_ms - now_ms > 86_400_000
        {
            return Err("usdcOrderInvalid".into());
        }
        let amount_micro = offer
            .amount_minor
            .checked_mul(10_000)
            .ok_or("usdcAmountOverflow")?;
        self.refresh_account_store_account(account_id)?;
        self.commit_account_store_transaction(&[account_id.to_owned()], |store| {
            let account = store
                .accounts
                .get_mut(account_id)
                .ok_or("billingAccountMissing")?;
            if account.active_ban(now_ms).is_some() {
                return Err("billingAccountBanned".into());
            }
            let credit = owned_save(account, character_index)?.credit;
            let ledger = account.billing.get_or_insert_with(BillingLedger::default);
            if let Some(existing) = ledger
                .sui_usdc_orders
                .values()
                .find(|o| o.request_id == request_id)
            {
                // A transport retry uses the original deadlines, never extends
                // authorization or silently changes a previously quoted price.
                if existing.character_index != character_index
                    || &existing.offer != offer
                    || &existing.domain != domain
                    || existing.capability_hash != capability_hash
                    || existing.payment_mode != payment_mode
                    || (!payment_mode.is_invoice() && existing.payer.as_deref() != payer)
                {
                    return Err("billingRequestIdReused".into());
                }
                return Ok(existing.clone());
            }
            if ledger.orders.len() + ledger.sui_usdc_orders.len() >= MAX_ORDERS {
                return Err("billingLedgerFull".into());
            }
            let reserved = ledger
                .orders
                .values()
                .filter(|o| {
                    o.character_index == character_index
                        && crate::billing::reserves_credit_capacity(o)
                })
                .map(|o| o.offer.credits)
                .chain(
                    ledger
                        .sui_usdc_orders
                        .values()
                        .filter(|o| {
                            o.character_index == character_index && reserves_capacity_at(o, now_ms)
                        })
                        .map(|o| o.offer.credits),
                )
                .try_fold(credit, |n, c| {
                    n.checked_add(c).ok_or("billingCreditCapacityExceeded")
                })?;
            reserved
                .checked_add(offer.credits)
                .ok_or("billingCreditCapacityExceeded")?;
            let mut bytes = [0u8; 32];
            OsRng
                .try_fill_bytes(&mut bytes)
                .map_err(|_| "billingEntropyUnavailable")?;
            let id = format!(
                "usdc_{}",
                bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
            );
            let order = UsdcRechargeOrder {
                id: id.clone(),
                request_id: request_id.into(),
                character_index,
                offer: offer.clone(),
                domain: domain.clone(),
                credit_origin: crate::procurement_fees::CreditOrigin::Test,
                account_ref: account_ref(account_id, character_index),
                amount_micro,
                created_at_ms: now_ms,
                expires_at_ms,
                capability_hash: capability_hash.into(),
                capability_expires_at_ms,
                payer: payer.map(str::to_owned),
                payment_mode,
                state: if payment_mode.is_invoice() {
                    UsdcOrderState::Prepared
                } else {
                    UsdcOrderState::PayerBound
                },
                prepared_tx: None,
                invoice: None,
                payment_hint: None,
                receipt: None,
                transfer_receipt: None,
                direct_scan_cursor: None,
                direct_scan_complete: false,
                applied: None,
                job_lease: None,
                review_reason: None,
            };
            order.validate()?;
            ledger.sui_usdc_orders.insert(id, order.clone());
            Ok(order)
        })
    }
    pub fn bind_usdc_recharge_payer(
        &self,
        account: &str,
        order: &str,
        payer: &str,
        capability_hash: &str,
        now: u64,
    ) -> Result<UsdcRechargeOrder, String> {
        if !address(payer) {
            return Err("usdcPayerInvalid".into());
        }
        self.refresh_account_store_account(account)?;
        self.commit_account_store_transaction(&[account.to_owned()], |store| {
            let o = find_mut(store, account, order)?;
            capability(o, capability_hash, now)?;
            if payer == o.domain.merchant {
                return Err("usdcPayerInvalid".into());
            }
            if let Some(bound) = &o.payer {
                if bound != payer {
                    return Err("usdcPayerBindingMismatch".into());
                }
                return Ok(o.clone());
            }
            if now >= o.expires_at_ms {
                return Err("usdcOrderExpired".into());
            }
            o.payer = Some(payer.into());
            o.state = UsdcOrderState::PayerBound;
            Ok(o.clone())
        })
    }
    /// Explicit operator/user-owned recovery of an untouched legacy Prepared
    /// order. The actual payment must still fall within its original window.
    pub fn recover_prepared_usdc_native_transfer(
        &self,
        account: &str,
        order: &str,
        credential_id: &str,
        payer: &str,
        tx_digest: &str,
        now: u64,
    ) -> Result<UsdcRechargeOrder, String> {
        self.require_native_usdc_payer(account, credential_id, payer)?;
        if !digest(tx_digest) || now == 0 {
            return Err("usdcDigestInvalid".into());
        }
        self.recover_prepared_native_transfer(account, order, payer, tx_digest)
    }
    fn recover_prepared_native_transfer(
        &self,
        account: &str,
        order: &str,
        payer: &str,
        tx_digest: &str,
    ) -> Result<UsdcRechargeOrder, String> {
        self.refresh_account_store_account(account)?;
        self.commit_account_store_transaction(&[account.to_owned()], |store| {
            let o = find_mut(store, account, order)?;
            if o.payment_mode == UsdcPaymentMode::DirectTransfer {
                if o.payer.as_deref() == Some(payer)
                    && (o.payment_hint.as_deref() == Some(tx_digest)
                        || o.payment_digest() == Some(tx_digest))
                {
                    return Ok(o.clone());
                }
                return Err("usdcNativeTransferRecoveryImmutable".into());
            }
            if o.state != UsdcOrderState::Prepared
                || o.payer.is_some()
                || o.prepared_tx.is_some()
                || o.invoice.is_some()
                || o.payment_hint.is_some()
                || o.has_payment_receipt()
                || o.applied.is_some()
                || o.job_lease.is_some()
                || o.review_reason.is_some()
                || !address(payer)
                || payer == o.domain.merchant
            {
                return Err("usdcNativeTransferRecoveryRejected".into());
            }
            o.payment_mode = UsdcPaymentMode::DirectTransfer;
            o.payer = Some(payer.into());
            o.payment_hint = Some(tx_digest.into());
            o.state = UsdcOrderState::PayerBound;
            Ok(o.clone())
        })
    }
    pub fn claim_usdc_recharge_job(
        &self,
        account: &str,
        order: &str,
        worker: &str,
        now: u64,
    ) -> Result<Option<UsdcRechargeOrder>, String> {
        if !key(worker, 128) {
            return Err("usdcWorkerInvalid".into());
        }
        self.refresh_account_store_account(account)?;
        self.commit_account_store_transaction(&[account.to_owned()], |store| {
            let o = find_mut(store, account, order)?;
            if o.payer.is_none()
                || o.has_payment_receipt()
                || o.state == UsdcOrderState::Review
                || o.job_lease.as_ref().is_some_and(|l| now < l.until_ms)
                || (o.payment_mode.is_invoice()
                    && now >= o.expires_at_ms
                    && o.prepared_tx.is_none())
                || (!o.payment_mode.is_invoice()
                    && o.direct_scan_complete
                    && o.payment_hint.is_none())
            {
                return Ok(None);
            }
            let until_ms = now.checked_add(JOB_LEASE_MS).ok_or("usdcClockInvalid")?;
            o.job_lease = Some(UsdcJobLease {
                worker: worker.into(),
                until_ms,
            });
            Ok(Some(o.clone()))
        })
    }
    pub fn release_usdc_recharge_job(
        &self,
        account: &str,
        order: &str,
        worker: &str,
        _now: u64,
    ) -> Result<(), String> {
        self.refresh_account_store_account(account)?;
        self.commit_account_store_transaction(&[account.to_owned()], |store| {
            let o = find_mut(store, account, order)?;
            if o.job_lease.as_ref().is_some_and(|l| l.worker == worker) {
                o.job_lease = None;
            }
            Ok(())
        })
    }
    pub fn record_usdc_prepared_transaction(
        &self,
        account: &str,
        order: &str,
        worker: &str,
        tx: &UsdcPreparedTransaction,
        now: u64,
    ) -> Result<UsdcRechargeOrder, String> {
        self.refresh_account_store_account(account)?;
        // Reconcile cached foreign creation owners before the in-process
        // uniqueness check. Another host may have confirmed their invoice;
        // refreshing just these accounts cannot overwrite an active ECS owner.
        // The PostgreSQL trigger remains the final fence for unseen races.
        let stale_owners = {
            let store = self
                .account_store
                .lock()
                .map_err(|_| "billingStoreUnavailable")?;
            let domain = &store
                .accounts
                .get(account)
                .and_then(|a| a.billing.as_ref())
                .and_then(|l| l.sui_usdc_orders.get(order))
                .ok_or("usdcOrderMissing")?
                .domain;
            store
                .accounts
                .iter()
                .filter(|(id, a)| {
                    id.as_str() != account
                        && a.billing
                            .iter()
                            .flat_map(|l| l.sui_usdc_orders.values())
                            .any(|o| {
                                creation_in_flight(o)
                                    && o.domain.chain_identifier == domain.chain_identifier
                                    && o.domain.config_id == domain.config_id
                            })
                })
                .map(|(id, _)| id.clone())
                .collect::<Vec<_>>()
        };
        for owner in stale_owners {
            self.refresh_account_store_account(&owner)?;
        }
        self.commit_account_store_transaction(&[account.to_owned()], |store| {
            let target = store
                .accounts
                .get(account)
                .and_then(|a| a.billing.as_ref())
                .and_then(|l| l.sui_usdc_orders.get(order))
                .ok_or("usdcOrderMissing")?;
            if store.accounts.iter().any(|(id, a)| {
                a.billing
                    .iter()
                    .flat_map(|l| l.sui_usdc_orders.values())
                    .any(|o| {
                        (id != account || o.id != order)
                            && creation_in_flight(o)
                            && o.domain.chain_identifier == target.domain.chain_identifier
                            && o.domain.config_id == target.domain.config_id
                    })
            }) {
                return Err("usdcCreationBusy".into());
            }
            let o = find_mut(store, account, order)?;
            lease(o, worker, now)?;
            if !o.payment_mode.is_invoice() {
                return Err("usdcPaymentModeMismatch".into());
            }
            if let Some(existing) = &o.prepared_tx {
                if existing != tx {
                    return Err("usdcPreparedTransactionImmutable".into());
                }
                return Ok(o.clone());
            }
            if o.state != UsdcOrderState::PayerBound || now >= o.expires_at_ms {
                return Err("usdcOrderExpired".into());
            }
            o.prepared_tx = Some(tx.clone());
            o.state = UsdcOrderState::InvoicePrepared;
            o.validate()?;
            Ok(o.clone())
        })
    }
    pub fn bind_usdc_recharge_invoice(
        &self,
        account: &str,
        order: &str,
        worker: &str,
        invoice: &UsdcInvoiceBinding,
        now: u64,
    ) -> Result<UsdcRechargeOrder, String> {
        self.refresh_account_store_account(account)?;
        self.commit_account_store_transaction(&[account.to_owned()], |store| {
            let o = find_mut(store, account, order)?;
            lease(o, worker, now)?;
            if !o.payment_mode.is_invoice() {
                return Err("usdcPaymentModeMismatch".into());
            }
            if let Some(existing) = &o.invoice {
                if existing != invoice {
                    return Err("usdcInvoiceBindingMismatch".into());
                }
                return Ok(o.clone());
            }
            if o.state != UsdcOrderState::InvoicePrepared {
                return Err("usdcInvoiceNotPrepared".into());
            }
            o.invoice = Some(invoice.clone());
            o.state = UsdcOrderState::InvoiceBound;
            o.validate()?;
            Ok(o.clone())
        })
    }
    pub fn submit_usdc_payment_hint(
        &self,
        account: &str,
        order: &str,
        capability_hash: &str,
        tx_digest: &str,
        now: u64,
    ) -> Result<UsdcRechargeOrder, String> {
        if !digest(tx_digest) {
            return Err("usdcDigestInvalid".into());
        }
        self.refresh_account_store_account(account)?;
        self.commit_account_store_transaction(&[account.to_owned()], |store| {
            let o = find_mut(store, account, order)?;
            capability(o, capability_hash, now)?;
            if o.payment_mode.is_invoice() && o.invoice.is_none() {
                return Err("usdcInvoiceMissing".into());
            }
            if o.payer.is_none() {
                return Err("usdcPayerMissing".into());
            }
            if !o.has_payment_receipt() {
                o.payment_hint = Some(tx_digest.into());
            }
            Ok(o.clone())
        })
    }
    /// Trusted verifier only. Both the permanent receipt and PaidPending state
    /// commit together; ordinary Source refresh later applies wallet credit.
    pub fn confirm_usdc_recharge_payment(
        &self,
        account: &str,
        order: &str,
        receipt: &VerifiedUsdcReceipt,
        now: u64,
    ) -> Result<UsdcRechargeOrder, String> {
        self.refresh_account_store_account(account)?;
        self.commit_account_store_transaction(&[account.to_owned()], |store| {
            let o = find_mut(store, account, order)?;
            o.validate_receipt(receipt)?;
            if now < receipt.confirmed_at_ms {
                return Err("usdcClockBeforePayment".into());
            }
            if let Some(existing) = &o.receipt {
                if existing != receipt {
                    return Err("usdcReceiptImmutable".into());
                }
                return Ok(o.clone());
            }
            o.receipt = Some(receipt.clone());
            o.state = UsdcOrderState::PaidPending;
            o.job_lease = None;
            o.review_reason = None;
            Ok(o.clone())
        })
    }
    /// Trusted verifier only: the gateway has independently checked the real
    /// transaction and its final checkpoint, not just a caller-supplied digest.
    pub fn confirm_usdc_native_transfer_payment(
        &self,
        account: &str,
        order: &str,
        receipt: &VerifiedNativeUsdcTransferReceipt,
        now: u64,
    ) -> Result<UsdcRechargeOrder, String> {
        self.refresh_account_store_account(account)?;
        self.commit_account_store_transaction(&[account.to_owned()], |store| {
            let o = find_mut(store, account, order)?;
            o.validate_transfer_receipt(receipt)?;
            if now < receipt.observed_at_ms {
                return Err("usdcClockBeforePayment".into());
            }
            if let Some(existing) = &o.transfer_receipt {
                // A later observation may replay after an unknown commit. Keep
                // the first observation timestamp while comparing all facts.
                let mut comparable = receipt.clone();
                comparable.observed_at_ms = existing.observed_at_ms;
                if existing != &comparable {
                    return Err("usdcReceiptImmutable".into());
                }
                return Ok(o.clone());
            }
            o.transfer_receipt = Some(receipt.clone());
            o.state = UsdcOrderState::PaidPending;
            o.job_lease = None;
            o.review_reason = None;
            Ok(o.clone())
        })
    }
    pub fn save_usdc_native_transfer_scan(
        &self,
        account: &str,
        order: &str,
        worker: &str,
        cursor: &str,
        window_complete: bool,
        now: u64,
    ) -> Result<UsdcRechargeOrder, String> {
        if !valid_scan_cursor(cursor) {
            return Err("usdcNativeTransferCursorInvalid".into());
        }
        self.refresh_account_store_account(account)?;
        self.commit_account_store_transaction(&[account.to_owned()], |store| {
            let o = find_mut(store, account, order)?;
            lease(o, worker, now)?;
            if o.payment_mode != UsdcPaymentMode::DirectTransfer
                || o.has_payment_receipt()
                || (o.direct_scan_complete && !window_complete)
                || (window_complete && now < o.expires_at_ms)
            {
                return Err("usdcNativeTransferScanRejected".into());
            }
            o.direct_scan_cursor = Some(cursor.into());
            o.direct_scan_complete = window_complete;
            Ok(o.clone())
        })
    }
    pub fn usdc_recharge_order(
        &self,
        account: &str,
        order: &str,
    ) -> Result<UsdcRechargeOrder, String> {
        self.refresh_account_store_account(account)?;
        let store = self
            .account_store
            .lock()
            .map_err(|_| "billingStoreUnavailable")?;
        crate::billing::validate_complete_store(&store)?;
        store
            .accounts
            .get(account)
            .and_then(|a| a.billing.as_ref())
            .and_then(|l| l.sui_usdc_orders.get(order))
            .cloned()
            .ok_or_else(|| "usdcOrderMissing".into())
    }
    pub fn find_usdc_recharge_order(
        &self,
        order: &str,
    ) -> Result<Option<(String, UsdcRechargeOrder)>, String> {
        let store = self.ranking_inspect_authority_snapshot()?;
        crate::billing::validate_complete_store(&store)?;
        Ok(store.accounts.iter().find_map(|(id, a)| {
            a.billing
                .as_ref()
                .and_then(|l| l.sui_usdc_orders.get(order))
                .map(|o| (id.clone(), o.clone()))
        }))
    }
    pub fn usdc_recharge_orders(
        &self,
        account: &str,
        character: i32,
    ) -> Result<Vec<UsdcRechargeOrder>, String> {
        self.refresh_account_store_account(account)?;
        let store = self
            .account_store
            .lock()
            .map_err(|_| "billingStoreUnavailable")?;
        crate::billing::validate_complete_store(&store)?;
        let a = store.accounts.get(account).ok_or("billingAccountMissing")?;
        owned_save(a, character)?;
        Ok(a.billing
            .iter()
            .flat_map(|l| l.sui_usdc_orders.values())
            .filter(|o| o.character_index == character)
            .cloned()
            .collect())
    }
    pub fn usdc_recharge_reconcile_candidates(
        &self,
        limit: usize,
    ) -> Result<Vec<(String, UsdcRechargeOrder)>, String> {
        self.usdc_recharge_reconcile_candidates_after(limit, None)
    }
    /// Fresh global authority lookup, independent of pagination and leases.
    /// Recovery of an already signed creation takes priority over constructing
    /// another transaction which would contend for the same sponsor gas object.
    pub fn usdc_recharge_active_creation(
        &self,
        domain: &UsdcDomain,
    ) -> Result<Option<(String, UsdcRechargeOrder)>, String> {
        domain.validate()?;
        let store = self.ranking_inspect_authority_snapshot()?;
        crate::billing::validate_complete_store(&store)?;
        Ok(store.accounts.iter().find_map(|(id, account)| {
            account
                .billing
                .iter()
                .flat_map(|ledger| ledger.sui_usdc_orders.values())
                .find(|order| {
                    creation_in_flight(order)
                        && order.domain.chain_identifier == domain.chain_identifier
                        && order.domain.config_id == domain.config_id
                })
                .map(|order| (id.clone(), order.clone()))
        }))
    }
    /// Seek on the complete authoritative snapshot before bounding the page.
    /// An unpaid first page must not hide later accounts or orders forever.
    /// Workers retain the last (account, order) tuple and wrap after an empty
    /// page; the lease still decides which worker may act on each candidate.
    pub fn usdc_recharge_reconcile_candidates_after(
        &self,
        limit: usize,
        after: Option<(&str, &str)>,
    ) -> Result<Vec<(String, UsdcRechargeOrder)>, String> {
        let store = self.ranking_inspect_authority_snapshot()?;
        crate::billing::validate_complete_store(&store)?;
        let now_ms = crate::monthly_card::monthly_card_now_ms();
        Ok(store
            .accounts
            .iter()
            .flat_map(|(id, a)| {
                a.billing
                    .iter()
                    .flat_map(|l| l.sui_usdc_orders.values())
                    .filter(move |o| {
                        after.is_none_or(|cursor| (id.as_str(), o.id.as_str()) > cursor)
                            && o.payer.is_some()
                            && !o.has_payment_receipt()
                            && o.state != UsdcOrderState::Applied
                            && o.state != UsdcOrderState::Review
                            // An invoice may have been created or paid before
                            // its deadline even when the reply arrives late.
                            // Only orders which never issued creation bytes
                            // can safely stop chain discovery at expiry.
                            && (if o.payment_mode.is_invoice() { o.prepared_tx.is_some() || now_ms < o.expires_at_ms }
                                else { !o.direct_scan_complete || o.payment_hint.is_some() })
                    })
                    .map(move |o| (id.clone(), o.clone()))
            })
            .take(limit.min(MAX_ORDERS))
            .collect())
    }
    /// Only finalized failed creation or a positively observed invoice mismatch
    /// may enter review. An RPC timeout is never a reason to call this method.
    pub fn mark_usdc_recharge_review(
        &self,
        account: &str,
        order: &str,
        worker: &str,
        reason: &str,
        now: u64,
    ) -> Result<UsdcRechargeOrder, String> {
        if !matches!(reason, "createFailed" | "invoiceMismatch") {
            return Err("usdcReviewReasonInvalid".into());
        }
        self.refresh_account_store_account(account)?;
        self.commit_account_store_transaction(&[account.to_owned()], |store| {
            let o = find_mut(store, account, order)?;
            lease(o, worker, now)?;
            if o.has_payment_receipt() || o.payer.is_none() || !o.payment_mode.is_invoice() {
                return Err("usdcOrderTerminal".into());
            }
            o.review_reason = Some(reason.into());
            o.state = UsdcOrderState::Review;
            o.job_lease = None;
            Ok(o.clone())
        })
    }
}

#[cfg(test)]
#[path = "sui_usdc_billing_tests.rs"]
mod tests;
