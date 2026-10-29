//! Personal procurement: durable custody, signed intents and recoverable chain jobs.
//! The game server is a trusted asset custodian. Chain observers must verify finality
//! against the configured fullnode, never authorize delivery from an indexer row.
use crate::{
    AccountRecord, AccountStore, AccountStoreDatabaseMode, CharacterSaveRecord, SimulationConfig,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const SCHEMA_VERSION: u16 = crate::sui_usdc_billing::SCHEMA_VERSION;
pub const COMMIT_UNKNOWN: &str = "PROCUREMENT_COMMIT_OUTCOME_UNKNOWN";
pub const DRAGON_SWORD_TEMPLATE: u32 = 245;
// One serial task may make four bounded 20-second fullnode calls. Claiming a
// single task avoids expiring a batch while its predecessors are still running.
const OUTBOX_LEASE_MS: u64 = 120_000;
pub type Bytes32 = [u8; 32];
pub fn digest(bytes: &[u8]) -> Bytes32 {
    Sha256::digest(bytes).into()
}
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub fn actor_ref(account: &str, save: &CharacterSaveRecord) -> Bytes32 {
    digest(
        &serde_json::to_vec(&("MIR2_PROCUREMENT_ACTOR_V1", account, save.character.index))
            .expect("serializable identity"),
    )
}
fn vec_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    let mut n = bytes.len();
    loop {
        let b = (n & 127) as u8;
        n >>= 7;
        out.push(if n == 0 { b } else { b | 128 });
        if n == 0 {
            break;
        }
    }
    out.extend(bytes);
}

/// Exact BCS field order shared with the Dubhe system (addresses are fixed 32 bytes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentV1 {
    pub network_hash: Bytes32,
    pub package_id: Bytes32,
    pub dapp_storage_id: Bytes32,
    pub operation: u8,
    pub order_id: Bytes32,
    pub fill_id: Bytes32,
    pub buyer_ref: Bytes32,
    pub seller_ref: Bytes32,
    pub template_id: u32,
    pub conditions_hash: Bytes32,
    pub quantity: u64,
    pub price_gold: u64,
    pub expires_at_ms: u64,
    pub rule_version: u64,
    pub nonce: Bytes32,
    pub valid_until_ms: u64,
    pub actor_public_key: Bytes32,
}
impl IntentV1 {
    pub fn to_wire(&self) -> mir2_protocol::procurement::IntentV1 {
        use mir2_protocol::procurement::{encode_hex as h, INTENT_DOMAIN};
        mir2_protocol::procurement::IntentV1 {
            domain: INTENT_DOMAIN.into(),
            network_hash: h(&self.network_hash),
            package_id: h(&self.package_id),
            dapp_storage_id: h(&self.dapp_storage_id),
            operation: self.operation,
            order_id: h(&self.order_id),
            fill_id: h(&self.fill_id),
            buyer_ref: h(&self.buyer_ref),
            seller_ref: h(&self.seller_ref),
            template_id: self.template_id,
            conditions_hash: h(&self.conditions_hash),
            quantity: self.quantity.to_string(),
            price_gold: self.price_gold.to_string(),
            expires_at_ms: self.expires_at_ms.to_string(),
            rule_version: self.rule_version.to_string(),
            nonce: h(&self.nonce),
            valid_until_ms: self.valid_until_ms.to_string(),
            actor_public_key: h(&self.actor_public_key),
        }
    }
    pub fn bcs_bytes(&self) -> Vec<u8> {
        self.to_wire()
            .canonical_bcs()
            .expect("validated normalized intent")
    }
    pub fn hash(&self) -> Bytes32 {
        digest(&self.bcs_bytes())
    }
}
impl TryFrom<&mir2_protocol::procurement::IntentV1> for IntentV1 {
    type Error = String;
    fn try_from(i: &mir2_protocol::procurement::IntentV1) -> Result<Self, String> {
        use mir2_protocol::procurement::{decimal as d, hex32 as h};
        i.canonical_bcs().map_err(str::to_string)?;
        Ok(Self {
            network_hash: h(&i.network_hash).map_err(str::to_string)?,
            package_id: h(&i.package_id).map_err(str::to_string)?,
            dapp_storage_id: h(&i.dapp_storage_id).map_err(str::to_string)?,
            operation: i.operation,
            order_id: h(&i.order_id).map_err(str::to_string)?,
            fill_id: h(&i.fill_id).map_err(str::to_string)?,
            buyer_ref: h(&i.buyer_ref).map_err(str::to_string)?,
            seller_ref: h(&i.seller_ref).map_err(str::to_string)?,
            template_id: i.template_id,
            conditions_hash: h(&i.conditions_hash).map_err(str::to_string)?,
            quantity: d(&i.quantity).map_err(str::to_string)?,
            price_gold: d(&i.price_gold).map_err(str::to_string)?,
            expires_at_ms: d(&i.expires_at_ms).map_err(str::to_string)?,
            rule_version: d(&i.rule_version).map_err(str::to_string)?,
            nonce: h(&i.nonce).map_err(str::to_string)?,
            valid_until_ms: d(&i.valid_until_ms).map_err(str::to_string)?,
            actor_public_key: h(&i.actor_public_key).map_err(str::to_string)?,
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedIntent {
    pub intent: IntentV1,
    pub signature: Vec<u8>,
}
impl SignedIntent {
    pub fn verify(&self) -> Result<(), String> {
        let bytes = self
            .intent
            .to_wire()
            .canonical_bcs()
            .map_err(str::to_string)?;
        let key = ed25519_consensus::VerificationKey::try_from(self.intent.actor_public_key)
            .map_err(|_| "procurementInvalidKey")?;
        let sig = ed25519_consensus::Signature::try_from(self.signature.as_slice())
            .map_err(|_| "procurementInvalidSignature")?;
        key.verify(&sig, &digest(&bytes))
            .map_err(|_| "procurementInvalidSignature")?;
        let mut identity = [0u8; 64];
        identity[0] = 1;
        let probe = ed25519_consensus::Signature::try_from(identity.as_slice())
            .map_err(|_| "procurementInvalidSignature")?;
        if key
            .verify(&probe, b"MIR2_REJECT_MESSAGE_INDEPENDENT_KEYS")
            .is_ok()
        {
            return Err("procurementWeakActorKey".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleProofV1 {
    pub intent_hash: Bytes32,
    pub asset_lock_id: Bytes32,
    pub asset_instance_hash: Bytes32,
    pub oracle_key_id: u64,
    pub valid_until_ms: u64,
    pub fence: u64,
}
impl OracleProofV1 {
    pub fn bcs_bytes(&self) -> Vec<u8> {
        let mut b = Vec::new();
        vec_bytes(&mut b, b"MIR2_PROCUREMENT_ORACLE_V1");
        for x in [
            &self.intent_hash,
            &self.asset_lock_id,
            &self.asset_instance_hash,
        ] {
            vec_bytes(&mut b, x)
        }
        for x in [self.oracle_key_id, self.valid_until_ms, self.fence] {
            b.extend(x.to_le_bytes())
        }
        b
    }
    pub fn hash(&self) -> Bytes32 {
        digest(&self.bcs_bytes())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Domain {
    pub network_hash: Bytes32,
    pub package_id: Bytes32,
    pub dapp_storage_id: Bytes32,
    pub oracle_key_id: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub version: u64,
    pub template_id: u32,
    pub fee_units: u32,
    pub cancel_fee_refundable: bool,
    pub expiry_fee_refundable: bool,
    pub maximum_lifetime_ms: u64,
    pub maximum_authorization_ms: u64,
}
impl Rule {
    pub fn validate(&self) -> Result<(), String> {
        if self.version == 0
            || self.template_id != 245
            || self.fee_units == 0
            || self.fee_units > 1000
            || self.maximum_lifetime_ms < 1000
            || self.maximum_lifetime_ms > 30 * 86400000
            || self.maximum_authorization_ms < 1000
            || self.maximum_authorization_ms > 900000
            || self.maximum_authorization_ms > self.maximum_lifetime_ms
        {
            return Err("procurementRuleInvalid".into());
        }
        Ok(())
    }
}
impl Default for Rule {
    fn default() -> Self {
        Self {
            version: 1,
            template_id: 245,
            fee_units: 1,
            cancel_fee_refundable: false,
            expiry_fee_refundable: false,
            maximum_lifetime_ms: 7 * 86400000,
            maximum_authorization_ms: 300000,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcurementConfig {
    pub enabled: bool,
    pub allow_test_credits: bool,
    pub production_charging_enabled: bool,
    pub domain: Domain,
    pub rule: Rule,
}
impl Default for ProcurementConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            allow_test_credits: false,
            production_charging_enabled: false,
            domain: Domain {
                network_hash: [0; 32],
                package_id: [0; 32],
                dapp_storage_id: [0; 32],
                oracle_key_id: 0,
            },
            rule: Rule::default(),
        }
    }
}
impl ProcurementConfig {
    pub fn from_env() -> Result<Self, String> {
        let runtime_value = |key| match std::env::var(key) {
            Ok(value) => Ok(Some(value)),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(std::env::VarError::NotUnicode(_)) => Err(()),
        };
        Self::from_values_with_runtime(
            |key| std::env::var(key).ok(),
            ["MIR2_RUNTIME_ENV", "MIR2_DEPLOYMENT_ENV", "MIR2_ENV"].map(runtime_value),
            runtime_value("MIR2_PLAYTEST_TESTNET"),
        )
    }
    #[cfg(test)]
    fn from_values(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let markers =
            ["MIR2_RUNTIME_ENV", "MIR2_DEPLOYMENT_ENV", "MIR2_ENV"].map(|key| Ok(lookup(key)));
        let playtest_testnet = Ok(lookup("MIR2_PLAYTEST_TESTNET"));
        Self::from_values_with_runtime(lookup, markers, playtest_testnet)
    }
    fn from_values_with_runtime(
        lookup: impl Fn(&str) -> Option<String>,
        markers: impl IntoIterator<Item = Result<Option<String>, ()>>,
        playtest_testnet: Result<Option<String>, ()>,
    ) -> Result<Self, String> {
        let playtest_testnet = procurement_playtest_testnet_flag(playtest_testnet)?;
        let mut c = Self::default();
        c.enabled = match lookup("MIR2_PROCUREMENT_ENABLED").as_deref() {
            None | Some("0") | Some("false") | Some("") => false,
            Some("1") | Some("true") => true,
            _ => return Err("procurementEnableFlagInvalid".into()),
        };
        let rule_json = lookup("MIR2_PROCUREMENT_RULE_JSON");
        if let Some(json) = &rule_json {
            if json.len() > 4096 {
                return Err("procurementRuleJsonInvalid".into());
            }
            c.rule = serde_json::from_str(json).map_err(|_| "procurementRuleJsonInvalid")?;
        }
        if let Some(version) = lookup("MIR2_PROCUREMENT_RULE_VERSION") {
            let version = version
                .parse()
                .map_err(|_| "procurementRuleVersionInvalid")?;
            if rule_json.is_some() && version != c.rule.version {
                return Err("procurementRuleVersionMismatch".into());
            }
            c.rule.version = version;
        }
        c.rule.validate()?;
        let domain_keys = [
            "MIR2_PROCUREMENT_NETWORK_HASH",
            "MIR2_PROCUREMENT_PACKAGE_ID",
            "MIR2_PROCUREMENT_DAPP_STORAGE_ID",
            "MIR2_PROCUREMENT_ORACLE_KEY_ID",
        ];
        if !c.enabled && !domain_keys.iter().any(|key| lookup(key).is_some()) {
            return Ok(c);
        }
        use mir2_protocol::procurement::hex32;
        let env = |k: &str| lookup(k).ok_or_else(|| format!("procurementConfigMissing:{k}"));
        c.domain = Domain {
            network_hash: hex32(&env("MIR2_PROCUREMENT_NETWORK_HASH")?).map_err(str::to_string)?,
            package_id: hex32(&env("MIR2_PROCUREMENT_PACKAGE_ID")?).map_err(str::to_string)?,
            dapp_storage_id: hex32(&env("MIR2_PROCUREMENT_DAPP_STORAGE_ID")?)
                .map_err(str::to_string)?,
            oracle_key_id: env("MIR2_PROCUREMENT_ORACLE_KEY_ID")?
                .parse()
                .map_err(|_| "procurementOracleKeyInvalid")?,
        };
        if c.domain.network_hash == [0; 32]
            || c.domain.package_id == [0; 32]
            || c.domain.dapp_storage_id == [0; 32]
            || c.domain.oracle_key_id == 0
        {
            return Err("procurementDomainInvalid".into());
        }
        c.allow_test_credits = lookup("MIR2_PROCUREMENT_TEST_CREDITS").as_deref() == Some("1");
        if c.allow_test_credits || playtest_testnet {
            validate_procurement_testnet_runtime(markers, playtest_testnet)?;
        }
        // Public playtest activation cannot reuse a localnet or arbitrary
        // network domain. The pinned Node driver also verifies the real chain.
        if playtest_testnet && c.domain.network_hash != digest(b"4c78adac") {
            return Err("procurementPlaytestTestnetRequired".into());
        }
        Ok(c)
    }
}
fn procurement_playtest_testnet_flag(value: Result<Option<String>, ()>) -> Result<bool, String> {
    match value
        .map_err(|_| "procurementPlaytestFlagInvalid")?
        .as_deref()
    {
        None | Some("0" | "false") => Ok(false),
        Some("1" | "true") => Ok(true),
        _ => Err("procurementPlaytestFlagInvalid".into()),
    }
}
fn validate_procurement_testnet_runtime(
    markers: impl IntoIterator<Item = Result<Option<String>, ()>>,
    playtest_testnet: bool,
) -> Result<(), String> {
    for marker in markers {
        let value = marker.map_err(|_| "procurementTestCreditsForbiddenInProduction")?;
        if value.is_some_and(|value| match value.trim().to_ascii_lowercase().as_str() {
            "production" | "prod" => true,
            "staging" | "stage" | "preprod" | "preproduction" => !playtest_testnet,
            _ => false,
        }) {
            return Err("procurementTestCreditsForbiddenInProduction".into());
        }
    }
    Ok(())
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreditSource {
    Test,
    Paid,
    Gift,
    Unknown,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditBatch {
    pub id: String,
    pub source: CreditSource,
    pub granted: u32,
    pub reserved: u32,
    pub consumed: u32,
    pub expires_at_ms: u64,
    pub payment_order_id: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActorBinding {
    pub character_index: i32,
    pub character_name: String,
    pub public_key: Bytes32,
    pub valid_until_ms: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    Publishing,
    PublishFailed,
    Open,
    Filled,
    Cancelled,
    Expired,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub buyer_account: String,
    pub buyer_character: i32,
    pub buyer_name: String,
    pub publish: SignedIntent,
    pub rule: Rule,
    pub status: OrderStatus,
    pub escrow_gold: u32,
    pub fee_batch: String,
    pub fee_consumed: bool,
    #[serde(default)]
    pub fee_refunded: bool,
    pub fence: u64,
    pub object_id: Option<String>,
    pub offers: BTreeMap<String, String>,
    pub winning_fill: Option<String>,
    pub final_receipt: Option<String>,
    #[serde(default)]
    pub last_chain_transaction: Option<String>,
    pub conditions: mir2_protocol::procurement::EquipmentConditions,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquipmentLock {
    pub id: String,
    pub order_id: String,
    pub buyer_account: String,
    pub seller_character: i32,
    pub seller_name: String,
    pub signed: SignedIntent,
    pub original_item_json: String,
    pub item_uid: u64,
    pub instance_hash: Bytes32,
    pub fence: u64,
    pub released: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobKind {
    Publish,
    Fill,
    Cancel,
    Expire,
    Reconcile,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    Pending,
    Submitted,
    Unknown,
    Confirmed,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboxTask {
    pub id: String,
    pub order_id: String,
    pub kind: JobKind,
    pub signed: Option<SignedIntent>,
    pub proof: Option<OracleProofV1>,
    pub status: JobStatus,
    pub attempts: u32,
    pub transaction_digests: Vec<String>,
    pub object_id: Option<String>,
    pub lease_owner: Option<String>,
    pub lease_until_ms: u64,
    #[serde(default)]
    pub signed_transactions: BTreeMap<String, PreparedTransaction>,
    /// Only finalized, failed effects permit a step to be signed again. An
    /// RPC timeout or missing result must never enter this audit trail.
    #[serde(default)]
    pub failed_transactions: BTreeMap<String, u64>,
    #[serde(default)]
    pub archived_transactions: BTreeMap<String, PreparedTransaction>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedTransaction {
    pub step: String,
    pub transaction_digest: String,
    pub signed_transaction_bytes: Vec<u8>,
    pub oracle_signature: Vec<u8>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Delivery {
    pub id: String,
    pub order_id: String,
    pub character_index: i32,
    pub character_name: String,
    pub gold: u32,
    pub original_item_json: Option<String>,
    pub item_uid: Option<u64>,
    pub claimed: bool,
    pub claim_revision: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainReceipt {
    pub network_hash: Bytes32,
    pub package_id: Bytes32,
    pub dapp_storage_id: Bytes32,
    pub order_id: Bytes32,
    pub object_id: String,
    pub status: u8,
    pub fill_id: Bytes32,
    pub buyer_ref: Bytes32,
    pub seller_ref: Bytes32,
    pub template_id: u32,
    pub conditions_hash: Bytes32,
    pub price_gold: u64,
    pub rule_version: u64,
    pub asset_instance_hash: Bytes32,
    pub fence: u64,
    pub checkpoint: u64,
    pub digest: String,
    pub event_seq: u64,
    pub chain_timestamp_ms: u64,
}
/// Trusted coherent registry absence: the worker samples finalized Clock first,
/// then two unchanged registry reads and verifies registry previous-TX effects.
/// It is not an order object, cancellation event, or a local RPC timeout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainAbsenceReceipt {
    pub network_hash: Bytes32,
    pub package_id: Bytes32,
    pub dapp_storage_id: Bytes32,
    pub order_id: Bytes32,
    pub clock_checkpoint: u64,
    pub clock_digest: String,
    pub chain_timestamp_ms: u64,
    pub registry_version: String,
    pub registry_previous_transaction: String,
    pub registry_absence: bool,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcurementLedger {
    #[serde(default)]
    pub absence_inbox: BTreeMap<String, ChainAbsenceReceipt>,
    pub bindings: BTreeMap<String, ActorBinding>,
    pub credits: BTreeMap<String, CreditBatch>,
    pub orders: BTreeMap<String, Order>,
    pub locks: BTreeMap<String, EquipmentLock>,
    pub outbox: BTreeMap<String, OutboxTask>,
    pub deliveries: BTreeMap<String, Delivery>,
    pub chain_inbox: BTreeMap<String, ChainReceipt>,
    pub cursors: BTreeMap<String, String>,
    pub requests: BTreeMap<String, String>,
    #[serde(default)]
    pub quotes: BTreeMap<String, StoredQuote>,
    #[serde(default)]
    pub quota_quotes: BTreeMap<String, StoredQuotaQuote>,
    #[serde(default)]
    pub quota_purchases: BTreeMap<String, QuotaPurchaseReceipt>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredQuotaQuote {
    pub public: mir2_protocol::procurement::ProcurementQuote,
    pub character_index: i32,
    pub character_name: String,
    pub request_id: String,
    pub package_id: String,
    pub rule: Rule,
    pub units: u32,
    pub created_at_ms: u64,
    pub valid_until_ms: u64,
    pub quota_expires_at_ms: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuotaPurchaseReceipt {
    pub quote_id: String,
    pub request_id: String,
    pub package_id: String,
    pub rule: Rule,
    pub character_index: i32,
    pub credit_batch_id: String,
    pub source: CreditSource,
    pub units: u32,
    pub granted_at_ms: u64,
    pub expires_at_ms: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredQuote {
    pub public: mir2_protocol::procurement::ProcurementQuote,
    pub intent: IntentV1,
    pub buyer_account: String,
    pub item_uid: Option<u64>,
    pub character_index: i32,
    pub character_name: String,
    pub request_id: String,
    #[serde(default)]
    pub rule: Option<Rule>,
}
#[derive(Debug, Clone)]
pub struct ProcurementExecution {
    pub packets: Vec<mir2_protocol::ServerPacket>,
    pub reply: mir2_protocol::procurement::ProcurementReply,
}
impl ProcurementLedger {
    pub fn validate(&self) -> Result<(), String> {
        if [
            self.orders.len(),
            self.locks.len(),
            self.outbox.len(),
            self.deliveries.len(),
            self.chain_inbox.len(),
            self.requests.len(),
            self.quota_quotes.len(),
            self.quota_purchases.len(),
        ]
        .into_iter()
        .any(|n| n > 8192)
        {
            return Err("procurementLedgerFull".into());
        }
        for (id, b) in &self.credits {
            if id != &b.id
                || b.reserved
                    .checked_add(b.consumed)
                    .is_none_or(|n| n > b.granted)
                || (b.source == CreditSource::Paid && b.payment_order_id.is_none())
            {
                return Err("procurementCreditLedgerInvalid".into());
            }
        }
        for (id, o) in &self.orders {
            o.rule.validate()?;
            o.publish.verify()?;
            if digest(&o.conditions.canonical_bcs().map_err(str::to_string)?)
                != o.publish.intent.conditions_hash
            {
                return Err("procurementStoredConditionsMismatch".into());
            }
            if id != &o.id
                || id != &hex(&o.publish.intent.order_id)
                || o.publish.intent.quantity != 1
                || o.publish.intent.template_id != 245
                || o.publish.intent.price_gold > u32::MAX as u64
                || o.rule.template_id != 245
                || o.rule.version != o.publish.intent.rule_version
                || o.escrow_gold
                    != if matches!(o.status, OrderStatus::Publishing | OrderStatus::Open) {
                        o.publish.intent.price_gold as u32
                    } else {
                        0
                    }
                || o.fee_consumed
                    != !matches!(
                        o.status,
                        OrderStatus::Publishing | OrderStatus::PublishFailed
                    )
            {
                return Err("procurementOrderLedgerInvalid".into());
            }
        }
        for (id, l) in &self.locks {
            l.signed.verify()?;
            if id != &l.id
                || digest(l.original_item_json.as_bytes()) != l.instance_hash
                || l.id != hex(&l.signed.intent.fill_id)
            {
                return Err("procurementEquipmentLockInvalid".into());
            }
        }
        for (id, d) in &self.deliveries {
            if id != &d.id
                || d.claimed != d.claim_revision.is_some()
                || d.original_item_json.is_some() != d.item_uid.is_some()
            {
                return Err("procurementDeliveryInvalid".into());
            }
        }
        for (id, q) in &self.quota_quotes {
            q.rule.validate()?;
            if id != &q.public.quote_id
                || q.public.operation != "buyQuota"
                || q.public.intent.is_some()
                || q.public.canonical_intent_hex.is_some()
                || q.public.service_fee.quota_source != "test"
                || q.public.service_fee.credit_cost != 0
                || q.public.service_fee.production_billing_enabled
                || q.public.quota_package_id.as_deref() != Some(q.package_id.as_str())
                || q.package_id != test_quota_package_id(&q.rule)
                || q.units != test_quota_package_units(&q.rule)?
                || q.public.service_fee.quota_units != q.units
                || q.public.service_fee.rule_version != q.rule.version.to_string()
                || q.valid_until_ms <= q.created_at_ms
                || q.quota_expires_at_ms <= q.created_at_ms
            {
                return Err("procurementQuotaQuoteInvalid".into());
            }
        }
        for (id, receipt) in &self.quota_purchases {
            receipt.rule.validate()?;
            let q = self
                .quota_quotes
                .get(&receipt.quote_id)
                .ok_or("procurementQuotaQuoteMissing")?;
            let b = self
                .credits
                .get(&receipt.credit_batch_id)
                .ok_or("procurementQuotaBatchMissing")?;
            if id != &receipt.package_id
                || receipt.package_id != q.package_id
                || receipt.rule != q.rule
                || receipt.character_index != q.character_index
                || receipt.source != CreditSource::Test
                || receipt.units != q.units
                || receipt.request_id != q.request_id
                || receipt.expires_at_ms != q.quota_expires_at_ms
                || receipt.granted_at_ms < q.created_at_ms
                || receipt.granted_at_ms >= q.valid_until_ms
                || b.source != CreditSource::Test
                || b.granted != receipt.units
                || b.expires_at_ms != receipt.expires_at_ms
                || b.payment_order_id.is_some()
                || self.requests.get(&receipt.request_id) != Some(&receipt.quote_id)
            {
                return Err("procurementQuotaReceiptInvalid".into());
            }
        }
        Ok(())
    }
}
pub(crate) fn validate_store(store: &AccountStore) -> Result<(), String> {
    let mut ids = BTreeSet::new();
    let mut orders = BTreeSet::new();
    for a in store.accounts.values() {
        if let Some(l) = &a.procurement {
            for o in l.orders.values() {
                if !orders.insert(o.id.clone()) {
                    return Err("procurementDuplicateOrderId".into());
                }
            }
            for json in l
                .locks
                .values()
                .filter(|x| !x.released)
                .map(|x| &x.original_item_json)
                .chain(
                    l.deliveries
                        .values()
                        .filter(|x| !x.claimed)
                        .filter_map(|x| x.original_item_json.as_ref()),
                )
            {
                for id in crate::runtime::procurement::custody_ids(json)? {
                    if !ids.insert(id) {
                        return Err("procurementDuplicateCustodyUid".into());
                    }
                }
            }
        }
    }
    if !ids.is_empty() {
        for a in store.accounts.values() {
            for save in a.saves.values() {
                for json in save
                    .inventory_items_json
                    .iter()
                    .chain(save.belt_items_json.iter())
                    .chain(save.storage_items_json.iter())
                {
                    for id in crate::runtime::procurement::custody_ids(json)? {
                        if ids.contains(&id) {
                            return Err("procurementCustodyStillInInventory".into());
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
pub(crate) fn validate_account(account: &AccountRecord) -> Result<(), String> {
    if let Some(l) = &account.procurement {
        if account.billing.is_none() {
            return Err("procurementDowngradeFenceMissing".into());
        }
        l.validate()?
    }
    Ok(())
}
pub(crate) fn ensure_deletable(account: &AccountRecord, character: i32) -> Result<(), String> {
    if let Some(l) = &account.procurement {
        if account.billing.is_none() {
            return Err("procurementDowngradeFenceMissing".into());
        }
        if l.orders.values().any(|o| {
            o.buyer_character == character
                && matches!(o.status, OrderStatus::Publishing | OrderStatus::Open)
        }) || l
            .locks
            .values()
            .any(|x| x.seller_character == character && !x.released)
            || l.deliveries
                .values()
                .any(|d| d.character_index == character && !d.claimed)
        {
            return Err("procurementCharacterHasCustody".into());
        }
    }
    Ok(())
}
pub(crate) fn require_source(config: &SimulationConfig) -> Result<(), String> {
    if (config.account_store_path.is_some() && config.account_store_database_url.is_some())
        || (config.account_store_path.is_none()
            && !(config.account_store_database_url.is_some()
                && config.account_store_database_mode == AccountStoreDatabaseMode::SourceOfTruth))
    {
        return Err("procurementSingleDurableSourceRequired".into());
    }
    config.ensure_account_store_writable()
}
fn ledger(account: &mut AccountRecord) -> &mut ProcurementLedger {
    account
        .billing
        .get_or_insert_with(crate::billing::BillingLedger::default);
    account
        .procurement
        .get_or_insert_with(ProcurementLedger::default)
}
pub(crate) fn test_quota_package_id(rule: &Rule) -> String {
    format!("test-free-10-v{}", rule.version)
}
pub(crate) fn test_quota_package_units(rule: &Rule) -> Result<u32, String> {
    rule.validate()?;
    rule.fee_units
        .checked_mul(10)
        .ok_or_else(|| "procurementCreditOverflow".into())
}
pub(crate) fn test_quota_sale_enabled(config: &ProcurementConfig) -> bool {
    config.enabled && config.allow_test_credits && !config.production_charging_enabled
}
pub(crate) fn check_rule_version(l: &ProcurementLedger, rule: &Rule) -> Result<(), String> {
    rule.validate()?;
    if l.orders
        .values()
        .map(|o| &o.rule)
        .chain(l.quota_quotes.values().map(|q| &q.rule))
        .chain(l.quotes.values().filter_map(|q| q.rule.as_ref()))
        .any(|old| old.version == rule.version && old != rule)
    {
        return Err("procurementRuleVersionReused".into());
    }
    Ok(())
}
pub(crate) fn quote_test_quota(
    store: &mut AccountStore,
    config: &ProcurementConfig,
    account: &str,
    save: &CharacterSaveRecord,
    request: &str,
    package: &str,
    now: u64,
) -> Result<mir2_protocol::procurement::ProcurementQuote, String> {
    if !test_quota_sale_enabled(config) || package != test_quota_package_id(&config.rule) {
        return Err("procurementProductionBillingDisabled".into());
    }
    let l = ledger(
        store
            .accounts
            .get_mut(account)
            .ok_or("procurementAccountMissing")?,
    );
    check_rule_version(l, &config.rule)?;
    if let Some(old) = l.quota_quotes.values().find(|q| q.request_id == request) {
        return if old.package_id == package
            && old.character_index == save.character.index
            && old.character_name == save.character.name
            && old.rule == config.rule
        {
            Ok(old.public.clone())
        } else {
            Err("procurementIdempotencyConflict".into())
        };
    }
    if l.requests.contains_key(request) {
        return Err("procurementIdempotencyConflict".into());
    }
    if l.quota_quotes.len() >= 128 {
        return Err("procurementQuoteLimit".into());
    }
    let units = test_quota_package_units(&config.rule)?;
    let valid_until_ms = now
        .checked_add(config.rule.maximum_authorization_ms)
        .ok_or("procurementClockOverflow")?;
    let expires = now
        .checked_add(config.rule.maximum_lifetime_ms)
        .ok_or("procurementClockOverflow")?;
    let id = format!(
        "quota-{}",
        hex(&digest(
            &serde_json::to_vec(&(
                "MIR2_TEST_QUOTA_QUOTE_V1",
                account,
                save.character.index,
                request,
                package,
                &config.rule
            ))
            .map_err(|e| e.to_string())?
        ))
    );
    let public = mir2_protocol::procurement::ProcurementQuote {
        quote_id: id.clone(),
        operation: "buyQuota".into(),
        intent: None,
        canonical_intent_hex: None,
        conditions: Default::default(),
        item_uid: None,
        item_description: Some(
            "Free Test quota; once per account/rule version; no cash revenue".into(),
        ),
        service_fee: mir2_protocol::procurement::ServiceFeeQuote {
            rule_version: config.rule.version.to_string(),
            quota_units: units,
            quota_source: "test".into(),
            credit_cost: 0,
            refund_on_cancel: config.rule.cancel_fee_refundable,
            refund_on_expiry: config.rule.expiry_fee_refundable,
            quota_valid_until_ms: Some(expires.to_string()),
            production_billing_enabled: false,
        },
        quota_package_id: Some(package.into()),
    };
    l.quota_quotes.insert(
        id,
        StoredQuotaQuote {
            public: public.clone(),
            character_index: save.character.index,
            character_name: save.character.name.clone(),
            request_id: request.into(),
            package_id: package.into(),
            rule: config.rule.clone(),
            units,
            created_at_ms: now,
            valid_until_ms,
            quota_expires_at_ms: expires,
        },
    );
    Ok(public)
}
pub(crate) fn confirm_test_quota(
    store: &mut AccountStore,
    config: &ProcurementConfig,
    account: &str,
    save: &CharacterSaveRecord,
    quote: &str,
    now: u64,
) -> Result<bool, String> {
    let l = ledger(
        store
            .accounts
            .get_mut(account)
            .ok_or("procurementAccountMissing")?,
    );
    let q = l
        .quota_quotes
        .get(quote)
        .cloned()
        .ok_or("procurementQuotaQuoteMissing")?;
    if q.character_index != save.character.index || q.character_name != save.character.name {
        return Err("procurementQuoteOwnerMismatch".into());
    }
    // The package receipt is the idempotency authority, including retries after
    // the quote expires or after operators close new publication/test grants.
    if l.quota_purchases.contains_key(&q.package_id) {
        return Ok(true);
    }
    if !test_quota_sale_enabled(config) {
        return Err("procurementProductionBillingDisabled".into());
    }
    if now >= q.valid_until_ms || now >= q.quota_expires_at_ms || now < q.created_at_ms {
        return Err("procurementQuotaQuoteExpired".into());
    }
    if let Some(previous) = l.requests.get(&q.request_id) {
        return Err(if previous == quote {
            "procurementQuotaReceiptMissing"
        } else {
            "procurementIdempotencyConflict"
        }
        .into());
    }
    let batch_id = format!("test-package:{}", q.package_id);
    if l.credits.contains_key(&batch_id) {
        return Err("procurementQuotaBatchConflict".into());
    }
    l.credits.insert(
        batch_id.clone(),
        CreditBatch {
            id: batch_id.clone(),
            source: CreditSource::Test,
            granted: q.units,
            reserved: 0,
            consumed: 0,
            expires_at_ms: q.quota_expires_at_ms,
            payment_order_id: None,
        },
    );
    l.requests.insert(q.request_id.clone(), quote.into());
    l.quota_purchases.insert(
        q.package_id.clone(),
        QuotaPurchaseReceipt {
            quote_id: quote.into(),
            request_id: q.request_id,
            package_id: q.package_id,
            rule: q.rule,
            character_index: q.character_index,
            credit_batch_id: batch_id,
            source: CreditSource::Test,
            units: q.units,
            granted_at_ms: now,
            expires_at_ms: q.quota_expires_at_ms,
        },
    );
    Ok(false)
}
pub(crate) fn verify_intent(
    config: &ProcurementConfig,
    account: &str,
    save: &CharacterSaveRecord,
    signed: &SignedIntent,
    operation: u8,
    now: u64,
) -> Result<(), String> {
    let i = &signed.intent;
    config.rule.validate()?;
    signed.verify()?;
    if (operation == 1 && !config.enabled)
        || config.domain.network_hash == [0; 32]
        || config.domain.package_id == [0; 32]
        || i.network_hash != config.domain.network_hash
        || i.package_id != config.domain.package_id
        || i.dapp_storage_id != config.domain.dapp_storage_id
        || i.operation != operation
        || i.order_id == [0; 32]
        || i.nonce == [0; 32]
        || i.quantity != 1
        || i.template_id != 245
        || i.price_gold == 0
        || i.price_gold > u32::MAX as u64
        || i.rule_version != config.rule.version
        || now >= i.valid_until_ms
        || i.valid_until_ms > i.expires_at_ms
        || i.valid_until_ms.saturating_sub(now) > config.rule.maximum_authorization_ms
        || i.expires_at_ms <= now
        || i.expires_at_ms.saturating_sub(now) > config.rule.maximum_lifetime_ms
        || (operation != 2 && (i.fill_id != [0; 32] || i.seller_ref != [0; 32]))
        || (operation == 2 && i.fill_id == [0; 32])
    {
        return Err("procurementIntentInvalid".into());
    }
    let actor = if operation == 2 {
        i.seller_ref
    } else {
        i.buyer_ref
    };
    if actor != actor_ref(account, save) {
        return Err("procurementActorMismatch".into());
    }
    Ok(())
}
pub(crate) fn verify_binding(
    account: &AccountRecord,
    save: &CharacterSaveRecord,
    i: &IntentV1,
    now: u64,
) -> Result<(), String> {
    let l = account
        .procurement
        .as_ref()
        .ok_or("procurementKeyNotBound")?;
    let b = l
        .bindings
        .get(&hex(&i.actor_public_key))
        .ok_or("procurementKeyNotBound")?;
    if b.character_index != save.character.index
        || b.character_name != save.character.name
        || b.valid_until_ms < i.valid_until_ms
        || now >= b.valid_until_ms
    {
        return Err("procurementKeyExpired".into());
    }
    Ok(())
}
fn job(
    id: String,
    order: String,
    kind: JobKind,
    signed: Option<SignedIntent>,
    proof: Option<OracleProofV1>,
) -> OutboxTask {
    OutboxTask {
        id,
        order_id: order,
        kind,
        signed,
        proof,
        status: JobStatus::Pending,
        attempts: 0,
        transaction_digests: vec![],
        object_id: None,
        lease_owner: None,
        lease_until_ms: 0,
        signed_transactions: BTreeMap::new(),
        failed_transactions: BTreeMap::new(),
        archived_transactions: BTreeMap::new(),
    }
}
fn proof(
    config: &ProcurementConfig,
    s: &SignedIntent,
    instance: Bytes32,
    fence: u64,
) -> OracleProofV1 {
    OracleProofV1 {
        intent_hash: s.intent.hash(),
        asset_lock_id: digest(&[b"MIR2_ASSET_LOCK_V1".as_slice(), &s.intent.hash()].concat()),
        asset_instance_hash: instance,
        oracle_key_id: config.domain.oracle_key_id,
        valid_until_ms: s.intent.valid_until_ms,
        fence,
    }
}
pub(crate) fn publish(
    store: &mut AccountStore,
    config: &ProcurementConfig,
    account_id: &str,
    save: &mut CharacterSaveRecord,
    request: &str,
    signed: &SignedIntent,
    conditions: mir2_protocol::procurement::EquipmentConditions,
    now: u64,
) -> Result<bool, String> {
    if digest(&conditions.canonical_bcs().map_err(str::to_string)?) != signed.intent.conditions_hash
    {
        return Err("procurementConditionsMismatch".into());
    }
    verify_intent(config, account_id, save, signed, 1, now)?;
    let account = store
        .accounts
        .get_mut(account_id)
        .ok_or("procurementAccountMissing")?;
    verify_binding(account, save, &signed.intent, now)?;
    let id = hex(&signed.intent.order_id);
    let request_hash = hex(&signed.intent.hash());
    let l = ledger(account);
    check_rule_version(l, &config.rule)?;
    if let Some(old) = l.requests.get(request) {
        if old != &request_hash {
            return Err("procurementIdempotencyConflict".into());
        }
        return Ok(true);
    }
    if request.is_empty() || request.len() > 128 || l.orders.contains_key(&id) {
        return Err("procurementOrderExists".into());
    }
    let batch = l
        .credits
        .values_mut()
        .find(|b| {
            now < b.expires_at_ms
                && (b.source == CreditSource::Paid
                    || (b.source == CreditSource::Test && config.allow_test_credits))
                && b.granted
                    .saturating_sub(b.reserved)
                    .saturating_sub(b.consumed)
                    >= config.rule.fee_units
        })
        .ok_or("procurementServiceCreditsInsufficient")?;
    let price = u32::try_from(signed.intent.price_gold).map_err(|_| "procurementGoldOverflow")?;
    save.gold = save
        .gold
        .checked_sub(price)
        .ok_or("procurementGoldInsufficient")?;
    batch.reserved = batch
        .reserved
        .checked_add(config.rule.fee_units)
        .ok_or("procurementCreditOverflow")?;
    let fee_batch = batch.id.clone();
    l.orders.insert(
        id.clone(),
        Order {
            id: id.clone(),
            buyer_account: account_id.into(),
            buyer_character: save.character.index,
            buyer_name: save.character.name.clone(),
            publish: signed.clone(),
            rule: config.rule.clone(),
            status: OrderStatus::Publishing,
            escrow_gold: price,
            fee_batch,
            fee_consumed: false,
            fee_refunded: false,
            fence: 0,
            object_id: None,
            offers: BTreeMap::new(),
            winning_fill: None,
            final_receipt: None,
            last_chain_transaction: None,
            conditions,
        },
    );
    l.outbox.insert(
        format!("publish:{id}"),
        job(
            format!("publish:{id}"),
            id,
            JobKind::Publish,
            Some(signed.clone()),
            Some(proof(config, signed, [0; 32], 0)),
        ),
    );
    l.requests.insert(request.into(), request_hash);
    Ok(false)
}
pub(crate) fn offer(
    store: &mut AccountStore,
    config: &ProcurementConfig,
    seller: &str,
    buyer: &str,
    save: &CharacterSaveRecord,
    request: &str,
    signed: &SignedIntent,
    item_json: String,
    uid: u64,
    now: u64,
) -> Result<bool, String> {
    signed.verify()?;
    if seller == buyer {
        return Err("procurementSelfTrade".into());
    }
    let seller_record = store
        .accounts
        .get(seller)
        .ok_or("procurementAccountMissing")?;
    verify_binding(seller_record, save, &signed.intent, now)?;
    let hash = hex(&signed.intent.hash());
    if let Some(h) = seller_record
        .procurement
        .as_ref()
        .and_then(|l| l.requests.get(request))
    {
        return if h == &hash {
            Ok(true)
        } else {
            Err("procurementIdempotencyConflict".into())
        };
    }
    let order_id = hex(&signed.intent.order_id);
    let fill_id = hex(&signed.intent.fill_id);
    let order = store
        .accounts
        .get(buyer)
        .and_then(|a| a.procurement.as_ref())
        .and_then(|l| l.orders.get(&order_id))
        .ok_or("procurementOrderMissing")?;
    let i = &signed.intent;
    let p = &order.publish.intent;
    let mut frozen_config = config.clone();
    frozen_config.rule = order.rule.clone();
    frozen_config.domain.network_hash = p.network_hash;
    frozen_config.domain.package_id = p.package_id;
    frozen_config.domain.dapp_storage_id = p.dapp_storage_id;
    verify_intent(&frozen_config, seller, save, signed, 2, now)?;
    if order.status != OrderStatus::Open
        || now >= p.expires_at_ms
        || i.buyer_ref != p.buyer_ref
        || i.template_id != p.template_id
        || i.conditions_hash != p.conditions_hash
        || i.price_gold != p.price_gold
        || i.expires_at_ms != p.expires_at_ms
        || i.rule_version != p.rule_version
        || order.offers.contains_key(&fill_id)
    {
        return Err("procurementOrderUnavailable".into());
    }
    let fence = order.fence;
    let instance_hash = digest(item_json.as_bytes());
    let l = ledger(store.accounts.get_mut(seller).unwrap());
    if request.is_empty() || request.len() > 128 || l.locks.contains_key(&fill_id) {
        return Err("procurementFillExists".into());
    }
    l.locks.insert(
        fill_id.clone(),
        EquipmentLock {
            id: fill_id.clone(),
            order_id: order_id.clone(),
            buyer_account: buyer.into(),
            seller_character: save.character.index,
            seller_name: save.character.name.clone(),
            signed: signed.clone(),
            original_item_json: item_json,
            item_uid: uid,
            instance_hash,
            fence,
            released: false,
        },
    );
    l.outbox.insert(
        format!("fill:{fill_id}"),
        job(
            format!("fill:{fill_id}"),
            order_id.clone(),
            JobKind::Fill,
            Some(signed.clone()),
            Some(proof(config, signed, instance_hash, fence)),
        ),
    );
    l.requests.insert(request.into(), hash);
    ledger(store.accounts.get_mut(buyer).unwrap())
        .orders
        .get_mut(&order_id)
        .unwrap()
        .offers
        .insert(fill_id, seller.into());
    Ok(false)
}
pub(crate) fn cancel(
    store: &mut AccountStore,
    config: &ProcurementConfig,
    account: &str,
    save: &CharacterSaveRecord,
    request: &str,
    signed: &SignedIntent,
    now: u64,
) -> Result<bool, String> {
    signed.verify()?;
    let a = store
        .accounts
        .get_mut(account)
        .ok_or("procurementAccountMissing")?;
    verify_binding(a, save, &signed.intent, now)?;
    let l = ledger(a);
    let h = hex(&signed.intent.hash());
    if let Some(old) = l.requests.get(request) {
        return if old == &h {
            Ok(true)
        } else {
            Err("procurementIdempotencyConflict".into())
        };
    }
    let id = hex(&signed.intent.order_id);
    let o = l.orders.get(&id).ok_or("procurementOrderMissing")?;
    let mut frozen_config = config.clone();
    frozen_config.rule = o.rule.clone();
    frozen_config.domain.network_hash = o.publish.intent.network_hash;
    frozen_config.domain.package_id = o.publish.intent.package_id;
    frozen_config.domain.dapp_storage_id = o.publish.intent.dapp_storage_id;
    verify_intent(&frozen_config, account, save, signed, 3, now)?;
    let mut expected = o.publish.intent.clone();
    expected.operation = 3;
    expected.nonce = signed.intent.nonce;
    expected.valid_until_ms = signed.intent.valid_until_ms;
    expected.actor_public_key = signed.intent.actor_public_key;
    if o.status != OrderStatus::Open || expected != signed.intent {
        return Err("procurementOrderUnavailable".into());
    }
    if l.outbox
        .values()
        .any(|j| j.order_id == id && j.kind == JobKind::Cancel && j.status != JobStatus::Confirmed)
    {
        return Err("procurementCancellationAlreadyPending".into());
    }
    let mut cancellation_proof = proof(config, signed, [0; 32], o.fence);
    // Cancellation authorizes the existing gold escrow, rather than creating
    // a new lock from the cancellation's distinct operation/nonce/key.
    cancellation_proof.asset_lock_id = proof(config, &o.publish, [0; 32], 0).asset_lock_id;
    l.outbox.insert(
        format!("cancel:{id}:{h}"),
        job(
            format!("cancel:{id}:{h}"),
            id,
            JobKind::Cancel,
            Some(signed.clone()),
            Some(cancellation_proof),
        ),
    );
    l.requests.insert(request.into(), h);
    Ok(false)
}

impl SimulationConfig {
    pub fn procurement_advance_cursor(
        &self,
        account: &str,
        source: &str,
        cursor: &str,
    ) -> Result<(), String> {
        require_source(self)?;
        if source.is_empty() || source.len() > 256 || cursor.len() > 4096 {
            return Err("procurementCursorInvalid".into());
        }
        self.refresh_account_store_account(account)?;
        self.commit_account_store_transaction(&[account.into()], |s| {
            ledger(
                s.accounts
                    .get_mut(account)
                    .ok_or("procurementAccountMissing")?,
            )
            .cursors
            .insert(source.into(), cursor.into());
            Ok(())
        })
    }
    pub fn procurement_apply_chain_absence(
        &self,
        _config: &ProcurementConfig,
        buyer: &str,
        r: &ChainAbsenceReceipt,
    ) -> Result<(), String> {
        require_source(self)?;
        self.refresh_account_store_account(buyer)?;
        self.commit_account_store_transaction(&[buyer.into()], |s| {
            let id = hex(&r.order_id);
            let event = format!("absence:{id}:{}:{}", r.clock_digest, r.clock_checkpoint);
            let l = ledger(
                s.accounts
                    .get_mut(buyer)
                    .ok_or("procurementAccountMissing")?,
            );
            if let Some(old) = l.absence_inbox.get(&event) {
                return if old == r {
                    Ok(())
                } else {
                    Err("procurementAbsenceReplayConflict".into())
                };
            }
            let o = l
                .orders
                .get(&id)
                .cloned()
                .ok_or("procurementOrderMissing")?;
            let i = &o.publish.intent;
            if r.network_hash != i.network_hash
                || r.package_id != i.package_id
                || r.dapp_storage_id != i.dapp_storage_id
                || !r.registry_absence
                || r.clock_checkpoint == 0
                || r.clock_digest.is_empty()
                || r.registry_version.is_empty()
                || r.registry_previous_transaction.is_empty()
                || o.object_id.is_some()
            {
                return Err("procurementAbsenceEvidenceInvalid".into());
            }
            if o.status == OrderStatus::PublishFailed {
                return Ok(());
            }
            if o.status != OrderStatus::Publishing || o.fee_consumed {
                return Err("procurementAbsenceOrderStateInvalid".into());
            }
            let deadline = l
                .outbox
                .values()
                .filter(|j| j.order_id == id && j.kind == JobKind::Publish)
                .filter_map(|j| j.proof.as_ref().map(|p| p.valid_until_ms))
                .chain(std::iter::once(i.valid_until_ms))
                .max()
                .ok_or("procurementPublishDeadlineMissing")?;
            if r.chain_timestamp_ms <= deadline {
                return Err("procurementAbsenceProofStillLive".into());
            }
            let batch = l
                .credits
                .get_mut(&o.fee_batch)
                .ok_or("procurementFeeReservationMissing")?;
            batch.reserved = batch
                .reserved
                .checked_sub(o.rule.fee_units)
                .ok_or("procurementFeeReservationMissing")?;
            let order = l.orders.get_mut(&id).unwrap();
            order.status = OrderStatus::PublishFailed;
            order.escrow_gold = 0;
            order.final_receipt = Some(event.clone());
            order.last_chain_transaction = Some(r.clock_digest.clone());
            delivery(
                l,
                format!("refund:{id}"),
                &id,
                o.buyer_character,
                &o.buyer_name,
                o.escrow_gold,
                None,
            )?;
            for j in l.outbox.values_mut().filter(|j| j.order_id == id) {
                j.status = JobStatus::Confirmed;
            }
            l.absence_inbox.insert(event, r.clone());
            Ok(())
        })
    }
    pub fn procurement_release_watch(
        &self,
        account: &str,
        job_id: &str,
        worker: &str,
        now: u64,
        delay_ms: u64,
    ) -> Result<(), String> {
        require_source(self)?;
        self.refresh_account_store_account(account)?;
        self.commit_account_store_transaction(&[account.into()], |s| {
            let j = ledger(
                s.accounts
                    .get_mut(account)
                    .ok_or("procurementAccountMissing")?,
            )
            .outbox
            .get_mut(job_id)
            .ok_or("procurementJobMissing")?;
            if j.status == JobStatus::Confirmed {
                return Ok(());
            }
            if j.lease_owner.as_deref() != Some(worker) || j.lease_until_ms <= now {
                return Err("procurementLeaseLost".into());
            }
            j.lease_owner = None;
            j.lease_until_ms = now
                .checked_add(delay_ms.min(30000))
                .ok_or("procurementClockOverflow")?;
            Ok(())
        })
    }
    /// Read source metadata rather than a gateway's partial account cache.
    pub fn procurement_find_order(&self, id: &str) -> Result<(String, Order), String> {
        let id = id.strip_prefix("0x").unwrap_or(id);
        if id.len() != 64
            || !id
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err("procurementOrderIdInvalid".into());
        }
        if self.account_store_path.is_none() {
            if let Some(url) = self.account_store_database_url.as_deref() {
                let account=crate::with_account_store_postgres_client(url,|client|client.query_opt("SELECT account_id FROM accounts WHERE COALESCE(raw_json->'procurement'->'orders','{}'::jsonb) ? $1 LIMIT 1",&[&id]).map(|r|r.map(|r|r.get::<_,String>(0))).map_err(|e|e.to_string()))?.ok_or("procurementOrderMissing")?;
                self.refresh_account_store_account(&account)?;
                let order = self
                    .procurement_view(&account)?
                    .orders
                    .get(id)
                    .cloned()
                    .ok_or("procurementOrderMissing")?;
                return Ok((account, order));
            }
        }
        let s = self
            .account_store
            .lock()
            .map_err(|_| "procurementStoreUnavailable")?;
        s.accounts
            .iter()
            .find_map(|(account, a)| {
                a.procurement
                    .as_ref()
                    .and_then(|l| l.orders.get(id))
                    .map(|o| (account.clone(), o.clone()))
            })
            .ok_or("procurementOrderMissing".into())
    }
    pub fn procurement_public_orders(&self, now: u64) -> Result<Vec<Order>, String> {
        if self.account_store_path.is_none() {
            if let Some(url) = self.account_store_database_url.as_deref() {
                let now = i64::try_from(now).map_err(|_| "procurementClockOverflow")?;
                return crate::with_account_store_postgres_client(url, |client| {
                    let rows=client.query("SELECT entry.value FROM accounts a CROSS JOIN LATERAL jsonb_each(COALESCE(a.raw_json->'procurement'->'orders','{}'::jsonb)) entry WHERE entry.value->>'status'='Open' AND (entry.value->'publish'->'intent'->>'expires_at_ms')::numeric > $1::bigint ORDER BY entry.key LIMIT 256",&[&now]).map_err(|e|e.to_string())?;
                    rows.into_iter()
                        .map(|r| {
                            serde_json::from_value::<Order>(r.get(0)).map_err(|e| e.to_string())
                        })
                        .collect()
                });
            }
        }
        let s = self
            .account_store
            .lock()
            .map_err(|_| "procurementStoreUnavailable")?;
        Ok(s.accounts
            .values()
            .filter_map(|a| a.procurement.as_ref())
            .flat_map(|l| l.orders.values())
            .filter(|o| o.status == OrderStatus::Open && o.publish.intent.expires_at_ms > now)
            .take(256)
            .cloned()
            .collect())
    }
    pub fn procurement_prepare_transaction(
        &self,
        account: &str,
        job_id: &str,
        worker: &str,
        now: u64,
        prepared: PreparedTransaction,
    ) -> Result<(), String> {
        require_source(self)?;
        if prepared.transaction_digest.is_empty()
            || prepared.signed_transaction_bytes.is_empty()
            || prepared.signed_transaction_bytes.len() > 262144
            || !matches!(
                prepared.step.as_str(),
                "create" | "activate" | "fill" | "cancel" | "expire" | "revoke"
            )
        {
            return Err("procurementTransactionInvalid".into());
        }
        self.refresh_account_store_account(account)?;
        self.commit_account_store_transaction(&[account.into()], |s| {
            let j = ledger(
                s.accounts
                    .get_mut(account)
                    .ok_or("procurementAccountMissing")?,
            )
            .outbox
            .get_mut(job_id)
            .ok_or("procurementJobMissing")?;
            if j.status == JobStatus::Confirmed {
                return Err("procurementJobAlreadyFinal".into());
            }
            let allowed = match j.kind {
                JobKind::Publish => matches!(prepared.step.as_str(), "create" | "activate"),
                JobKind::Fill => prepared.step == "fill",
                JobKind::Cancel => prepared.step == "cancel",
                JobKind::Expire => prepared.step == "expire",
                JobKind::Reconcile => matches!(prepared.step.as_str(), "expire" | "revoke"),
            };
            if !allowed || (j.proof.is_some() && prepared.oracle_signature.len() != 64) {
                return Err("procurementTransactionOperationMismatch".into());
            }
            if j.lease_owner.as_deref() != Some(worker) || j.lease_until_ms <= now {
                return Err("procurementLeaseLost".into());
            }
            if let Some(old) = j.signed_transactions.get(&prepared.step) {
                if old != &prepared {
                    if !j.failed_transactions.contains_key(&old.transaction_digest)
                        || old.transaction_digest == prepared.transaction_digest
                        || j.failed_transactions
                            .contains_key(&prepared.transaction_digest)
                    {
                        return Err("procurementSignedTransactionConflict".into());
                    }
                    j.archived_transactions
                        .insert(old.transaction_digest.clone(), old.clone());
                    j.signed_transactions
                        .insert(prepared.step.clone(), prepared);
                }
            } else {
                j.signed_transactions
                    .insert(prepared.step.clone(), prepared);
            }
            Ok(())
        })
    }
    /// Trusted chain-observer entry point. Only successful finality retrieval
    /// with effects status `failure` may call this method; no local exception,
    /// transaction-not-found response or timeout establishes a failed attempt.
    pub fn procurement_record_finalized_failure(
        &self,
        account: &str,
        job_id: &str,
        worker: &str,
        now: u64,
        digest: &str,
        checkpoint: u64,
    ) -> Result<(), String> {
        require_source(self)?;
        if digest.is_empty() || checkpoint == 0 {
            return Err("procurementFailedTransactionFinalityInvalid".into());
        }
        self.refresh_account_store_account(account)?;
        self.commit_account_store_transaction(&[account.into()], |s| {
            let j = ledger(
                s.accounts
                    .get_mut(account)
                    .ok_or("procurementAccountMissing")?,
            )
            .outbox
            .get_mut(job_id)
            .ok_or("procurementJobMissing")?;
            if !j
                .signed_transactions
                .values()
                .chain(j.archived_transactions.values())
                .any(|p| p.transaction_digest == digest)
            {
                return Err("procurementFailedTransactionUnprepared".into());
            }
            if let Some(old) = j.failed_transactions.get(digest) {
                return if *old == checkpoint {
                    Ok(())
                } else {
                    Err("procurementFailedTransactionReplayConflict".into())
                };
            }
            if j.lease_owner.as_deref() != Some(worker) || j.lease_until_ms <= now {
                return Err("procurementLeaseLost".into());
            }
            j.failed_transactions.insert(digest.into(), checkpoint);
            if !j.transaction_digests.iter().any(|d| d == digest) {
                j.transaction_digests.push(digest.into());
            }
            // A failed transaction decides nothing about the order. All gold,
            // equipment and service reservations remain in their prior state.
            if j.status != JobStatus::Confirmed {
                j.status = JobStatus::Pending;
            }
            j.lease_owner = None;
            j.lease_until_ms = 0;
            Ok(())
        })
    }
    pub fn procurement_accounts_with_work(&self) -> Result<Vec<String>, String> {
        if self.account_store_path.is_none() {
            if let Some(url) = self.account_store_database_url.as_deref() {
                return crate::with_account_store_postgres_client(url, |client| {
                    client.query("SELECT account_id FROM accounts a WHERE EXISTS (SELECT 1 FROM jsonb_each(COALESCE(a.raw_json->'procurement'->'outbox','{}'::jsonb)) entry WHERE entry.value->>'status' <> 'Confirmed') ORDER BY account_id LIMIT 10000",&[]).map(|rows|rows.into_iter().map(|r|r.get(0)).collect()).map_err(|e|e.to_string())
                });
            }
        }
        let s = self
            .account_store
            .lock()
            .map_err(|_| "procurementStoreUnavailable")?;
        Ok(s.accounts
            .iter()
            .filter(|(_, a)| {
                a.procurement
                    .as_ref()
                    .is_some_and(|l| l.outbox.values().any(|j| j.status != JobStatus::Confirmed))
            })
            .map(|(id, _)| id.clone())
            .collect())
    }
    pub fn procurement_apply_chain_receipt_with_cursor(
        &self,
        c: &ProcurementConfig,
        buyer: &str,
        r: &ChainReceipt,
        source: &str,
        cursor: &str,
    ) -> Result<(), String> {
        require_source(self)?;
        if source.is_empty() || source.len() > 256 || cursor.len() > 4096 {
            return Err("procurementCursorInvalid".into());
        }
        self.refresh_account_store_account(buyer)?;
        let id = hex(&r.order_id);
        let o = self
            .procurement_view(buyer)?
            .orders
            .get(&id)
            .cloned()
            .ok_or("procurementOrderMissing")?;
        let mut accounts = vec![buyer.into()];
        for a in o.offers.values().collect::<BTreeSet<_>>() {
            self.refresh_account_store_account(a)?;
            accounts.push(a.clone());
        }
        self.commit_account_store_transaction(&accounts, |s| {
            apply_receipt(s, c, buyer, r)?;
            ledger(s.accounts.get_mut(buyer).unwrap())
                .cursors
                .insert(source.into(), cursor.into());
            Ok(())
        })
    }
    /// Only test quotas are issuable here. Paid purchases require a separately
    /// approved provenance-aware recharge integration; production stays closed.
    pub fn procurement_grant_test_credits(
        &self,
        config: &ProcurementConfig,
        account: &str,
        id: &str,
        units: u32,
        expires: u64,
    ) -> Result<(), String> {
        require_source(self)?;
        if !config.allow_test_credits
            || config.production_charging_enabled
            || units == 0
            || id.is_empty()
        {
            return Err("procurementTestCreditsDisabled".into());
        }
        self.commit_account_store_transaction(&[account.into()], |s| {
            let l = ledger(
                s.accounts
                    .get_mut(account)
                    .ok_or("procurementAccountMissing")?,
            );
            let b = CreditBatch {
                id: id.into(),
                source: CreditSource::Test,
                granted: units,
                reserved: 0,
                consumed: 0,
                expires_at_ms: expires,
                payment_order_id: None,
            };
            if let Some(old) = l.credits.get(id) {
                if old != &b {
                    return Err("procurementCreditIdempotencyConflict".into());
                }
                return Ok(());
            }
            l.credits.insert(id.into(), b);
            Ok(())
        })
    }
    pub fn procurement_view(&self, account: &str) -> Result<ProcurementLedger, String> {
        self.refresh_account_store_account(account)?;
        let s = self
            .account_store
            .lock()
            .map_err(|_| "procurementStoreUnavailable")?;
        Ok(s.accounts
            .get(account)
            .ok_or("procurementAccountMissing")?
            .procurement
            .clone()
            .unwrap_or_default())
    }
    /// Backend worker obtains leased durable work. Retries preserve original
    /// signatures/proof expiry and transaction digests; timeout never unlocks.
    pub fn procurement_claim_outbox(
        &self,
        account: &str,
        worker: &str,
        now: u64,
    ) -> Result<Vec<OutboxTask>, String> {
        require_source(self)?;
        if worker.is_empty() {
            return Err("procurementWorkerInvalid".into());
        }
        self.refresh_account_store_account(account)?;
        self.commit_account_store_transaction(&[account.into()], |s| {
            let l = ledger(
                s.accounts
                    .get_mut(account)
                    .ok_or("procurementAccountMissing")?,
            );
            let mut out = vec![];
            for j in l
                .outbox
                .values_mut()
                .filter(|j| j.status != JobStatus::Confirmed && j.lease_until_ms <= now)
                .take(1)
            {
                j.lease_owner = Some(worker.into());
                j.lease_until_ms = now
                    .checked_add(OUTBOX_LEASE_MS)
                    .ok_or("procurementClockOverflow")?;
                j.attempts = j
                    .attempts
                    .checked_add(1)
                    .ok_or("procurementAttemptOverflow")?;
                out.push(j.clone())
            }
            Ok(out)
        })
    }
    pub fn procurement_record_submission(
        &self,
        account: &str,
        job_id: &str,
        worker: &str,
        now: u64,
        tx_digest: Option<String>,
        object_id: Option<String>,
    ) -> Result<(), String> {
        require_source(self)?;
        self.refresh_account_store_account(account)?;
        self.commit_account_store_transaction(&[account.into()], |s| {
            let j = ledger(
                s.accounts
                    .get_mut(account)
                    .ok_or("procurementAccountMissing")?,
            )
            .outbox
            .get_mut(job_id)
            .ok_or("procurementJobMissing")?;
            if j.status == JobStatus::Confirmed {
                return Ok(());
            }
            if j.lease_owner.as_deref() != Some(worker) || j.lease_until_ms <= now {
                return Err("procurementLeaseLost".into());
            }
            if let Some(d) = tx_digest {
                if !j.transaction_digests.contains(&d) {
                    j.transaction_digests.push(d)
                }
                j.status = JobStatus::Submitted
            } else {
                j.status = JobStatus::Unknown
            }
            if object_id.is_some() {
                j.object_id = object_id
            }
            j.lease_until_ms = 0;
            j.lease_owner = None;
            Ok(())
        })
    }
    /// This is a trusted finality seam, never a normal player command. The
    /// caller must authenticate a fullnode receipt (effects + object + checkpoint).
    pub fn procurement_apply_chain_receipt(
        &self,
        config: &ProcurementConfig,
        buyer: &str,
        r: &ChainReceipt,
    ) -> Result<(), String> {
        require_source(self)?;
        self.refresh_account_store_account(buyer)?;
        let id = hex(&r.order_id);
        let o = self
            .procurement_view(buyer)?
            .orders
            .get(&id)
            .cloned()
            .ok_or("procurementOrderMissing")?;
        let sellers = o.offers.values().cloned().collect::<BTreeSet<_>>();
        for a in &sellers {
            self.refresh_account_store_account(a)?;
        }
        let mut accounts = vec![buyer.into()];
        accounts.extend(sellers);
        self.commit_account_store_transaction(&accounts, |s| apply_receipt(s, config, buyer, r))
    }
}
fn delivery(
    l: &mut ProcurementLedger,
    id: String,
    order: &str,
    char_index: i32,
    name: &str,
    gold: u32,
    item: Option<(String, u64)>,
) -> Result<(), String> {
    let d = Delivery {
        id: id.clone(),
        order_id: order.into(),
        character_index: char_index,
        character_name: name.into(),
        gold,
        original_item_json: item.as_ref().map(|x| x.0.clone()),
        item_uid: item.map(|x| x.1),
        claimed: false,
        claim_revision: None,
    };
    if let Some(old) = l.deliveries.get(&id) {
        if old != &d {
            return Err("procurementDeliveryConflict".into());
        }
    } else {
        l.deliveries.insert(id, d);
    }
    Ok(())
}
fn apply_receipt(
    s: &mut AccountStore,
    c: &ProcurementConfig,
    buyer: &str,
    r: &ChainReceipt,
) -> Result<(), String> {
    let id = hex(&r.order_id);
    let event = format!("{}:{}:{}:{}", id, r.digest, r.event_seq, r.checkpoint);
    let l = s
        .accounts
        .get(buyer)
        .and_then(|a| a.procurement.as_ref())
        .ok_or("procurementOrderMissing")?;
    if let Some(old) = l.chain_inbox.get(&event) {
        return if old == r {
            Ok(())
        } else {
            Err("procurementChainReplayConflict".into())
        };
    }
    let o = l
        .orders
        .get(&id)
        .cloned()
        .ok_or("procurementOrderMissing")?;
    // Uninitialized order observations never consume quota or become public.
    // Only a finalized chain Clock beyond the authorization deadline proves
    // the signed activation cannot arrive later and permits compensation.
    if r.status == 0 {
        if r.network_hash != o.publish.intent.network_hash
            || r.package_id != o.publish.intent.package_id
            || r.dapp_storage_id != o.publish.intent.dapp_storage_id
            || r.checkpoint == 0
            || r.digest.is_empty()
            || r.object_id.is_empty()
            || o.status != OrderStatus::Publishing
        {
            return Err("procurementUnpublishedObservationInvalid".into());
        }
        let l = ledger(s.accounts.get_mut(buyer).unwrap());
        l.orders.get_mut(&id).unwrap().object_id = Some(r.object_id.clone());
        l.orders.get_mut(&id).unwrap().last_chain_transaction = Some(r.digest.clone());
        if let Some(j) = l.outbox.get_mut(&format!("publish:{id}")) {
            j.object_id = Some(r.object_id.clone());
        }
        if r.chain_timestamp_ms > o.publish.intent.valid_until_ms {
            let b = l
                .credits
                .get_mut(&o.fee_batch)
                .ok_or("procurementFeeReservationMissing")?;
            b.reserved = b
                .reserved
                .checked_sub(o.rule.fee_units)
                .ok_or("procurementFeeReservationMissing")?;
            let order = l.orders.get_mut(&id).unwrap();
            order.status = OrderStatus::PublishFailed;
            order.escrow_gold = 0;
            order.final_receipt = Some(event.clone());
            delivery(
                l,
                format!("refund:{id}"),
                &id,
                o.buyer_character,
                &o.buyer_name,
                o.escrow_gold,
                None,
            )?;
            if let Some(j) = l.outbox.get_mut(&format!("publish:{id}")) {
                j.status = JobStatus::Confirmed
            }
        }
        l.chain_inbox.insert(event, r.clone());
        return Ok(());
    }
    let p = &o.publish.intent;
    if r.network_hash != p.network_hash
        || r.package_id != p.package_id
        || r.dapp_storage_id != p.dapp_storage_id
        || r.buyer_ref != p.buyer_ref
        || r.template_id != p.template_id
        || r.conditions_hash != p.conditions_hash
        || r.price_gold != p.price_gold
        || r.rule_version != p.rule_version
        || r.fence < o.fence
        || r.checkpoint == 0
        || r.digest.is_empty()
        || r.object_id.is_empty()
        || !matches!(r.status, 1..=4)
    {
        return Err("procurementChainReceiptInvalid".into());
    }
    let same_terminal = matches!(
        (o.status, r.status),
        (OrderStatus::Filled, 2) | (OrderStatus::Cancelled, 3) | (OrderStatus::Expired, 4)
    ) && o.last_chain_transaction.as_deref() == Some(r.digest.as_str())
        && (r.status != 2 || o.winning_fill.as_deref() == Some(hex(&r.fill_id).as_str()));
    if same_terminal {
        ledger(s.accounts.get_mut(buyer).unwrap())
            .chain_inbox
            .insert(event, r.clone());
        return Ok(());
    }
    if !matches!(o.status, OrderStatus::Publishing | OrderStatus::Open) {
        return Err("procurementFinalStateConflict".into());
    }
    if r.status == 1 && o.status == OrderStatus::Open {
        for (fill, seller) in &o.offers {
            let sl = ledger(
                s.accounts
                    .get_mut(seller)
                    .ok_or("procurementSellerMissing")?,
            );
            let lock = sl
                .locks
                .get(fill)
                .cloned()
                .ok_or("procurementLockMissing")?;
            if !lock.released
                && (r.fence > lock.fence
                    || r.chain_timestamp_ms > lock.signed.intent.valid_until_ms)
            {
                delivery(
                    sl,
                    format!("return:{fill}"),
                    &id,
                    lock.seller_character,
                    &lock.seller_name,
                    0,
                    Some((lock.original_item_json, lock.item_uid)),
                )?;
                sl.locks.get_mut(fill).unwrap().released = true;
                if let Some(j) = sl.outbox.get_mut(&format!("fill:{fill}")) {
                    j.status = JobStatus::Confirmed
                }
            }
        }
        let l = ledger(s.accounts.get_mut(buyer).unwrap());
        // Only the same coherent Open observation which invalidates the old
        // proof can retire a cancellation attempt. Keep its signature and all
        // transaction attempts so a fresh explicit confirmation is auditable.
        for j in l
            .outbox
            .values_mut()
            .filter(|j| j.order_id == id && j.kind == JobKind::Cancel)
        {
            if j.status != JobStatus::Confirmed
                && j.proof
                    .as_ref()
                    .is_some_and(|p| r.chain_timestamp_ms > p.valid_until_ms || r.fence > p.fence)
            {
                j.status = JobStatus::Confirmed;
            }
        }
        l.orders.get_mut(&id).unwrap().fence = r.fence;
        l.chain_inbox.insert(event, r.clone());
        return Ok(());
    }
    if r.status == 4 && r.chain_timestamp_ms < p.expires_at_ms {
        return Err("procurementPrematureExpiry".into());
    }
    let mut winner = None;
    if r.status == 2 {
        let fill = hex(&r.fill_id);
        let seller = o.offers.get(&fill).ok_or("procurementUnknownFill")?;
        let lock = s
            .accounts
            .get(seller)
            .and_then(|a| a.procurement.as_ref())
            .and_then(|l| l.locks.get(&fill))
            .cloned()
            .ok_or("procurementLockMissing")?;
        if lock.released
            || lock.instance_hash != r.asset_instance_hash
            || lock.signed.intent.seller_ref != r.seller_ref
            || lock.fence != r.fence
        {
            return Err("procurementWinningLockMismatch".into());
        }
        winner = Some((fill, seller.clone(), lock));
    }
    // Active or final publication proves the one reserved service unit was used.
    let l = ledger(s.accounts.get_mut(buyer).unwrap());
    if !o.fee_consumed {
        let b = l
            .credits
            .get_mut(&o.fee_batch)
            .ok_or("procurementFeeReservationMissing")?;
        b.reserved = b
            .reserved
            .checked_sub(o.rule.fee_units)
            .ok_or("procurementFeeReservationMissing")?;
        b.consumed = b
            .consumed
            .checked_add(o.rule.fee_units)
            .ok_or("procurementCreditOverflow")?;
    }
    let order = l.orders.get_mut(&id).unwrap();
    order.fee_consumed = true;
    order.object_id = Some(r.object_id.clone());
    order.last_chain_transaction = Some(r.digest.clone());
    order.fence = r.fence;
    order.status = match r.status {
        1 => OrderStatus::Open,
        2 => OrderStatus::Filled,
        3 => OrderStatus::Cancelled,
        _ => OrderStatus::Expired,
    };
    if r.status != 1 {
        order.escrow_gold = 0;
        order.final_receipt = Some(event.clone());
        order.winning_fill = winner.as_ref().map(|x| x.0.clone());
    }
    if let Some(j) = l.outbox.get_mut(&format!("publish:{id}")) {
        j.status = JobStatus::Confirmed;
        j.object_id = Some(r.object_id.clone())
    }
    if r.status == 1 {
        l.outbox.entry(format!("watch:{id}")).or_insert_with(|| {
            job(
                format!("watch:{id}"),
                id.clone(),
                JobKind::Reconcile,
                None,
                None,
            )
        });
    }
    if matches!(r.status, 3 | 4) {
        if (r.status == 3 && o.rule.cancel_fee_refundable)
            || (r.status == 4 && o.rule.expiry_fee_refundable)
        {
            let batch = l
                .credits
                .get_mut(&o.fee_batch)
                .ok_or("procurementFeeReservationMissing")?;
            batch.consumed = batch
                .consumed
                .checked_sub(o.rule.fee_units)
                .ok_or("procurementFeeRefundInvalid")?;
            l.orders.get_mut(&id).unwrap().fee_refunded = true;
        }
        delivery(
            l,
            format!("refund:{id}"),
            &id,
            o.buyer_character,
            &o.buyer_name,
            o.escrow_gold,
            None,
        )?;
    }
    if let Some((fill, seller, lock)) = &winner {
        delivery(
            l,
            format!("item:{id}"),
            &id,
            o.buyer_character,
            &o.buyer_name,
            0,
            Some((lock.original_item_json.clone(), lock.item_uid)),
        )?;
        let sl = ledger(s.accounts.get_mut(seller).unwrap());
        delivery(
            sl,
            format!("proceeds:{fill}"),
            &id,
            lock.seller_character,
            &lock.seller_name,
            o.escrow_gold,
            None,
        )?;
    }
    if r.status != 1 {
        for (fill, seller) in &o.offers {
            let sl = ledger(
                s.accounts
                    .get_mut(seller)
                    .ok_or("procurementSellerMissing")?,
            );
            let lock = sl
                .locks
                .get(fill)
                .cloned()
                .ok_or("procurementLockMissing")?;
            if !lock.released && winner.as_ref().is_none_or(|x| &x.0 != fill) {
                delivery(
                    sl,
                    format!("return:{fill}"),
                    &id,
                    lock.seller_character,
                    &lock.seller_name,
                    0,
                    Some((lock.original_item_json.clone(), lock.item_uid)),
                )?;
            }
            sl.locks.get_mut(fill).unwrap().released = true;
            if let Some(j) = sl.outbox.get_mut(&format!("fill:{fill}")) {
                j.status = JobStatus::Confirmed
            }
        }
        let l = ledger(s.accounts.get_mut(buyer).unwrap());
        if let Some(j) = l.outbox.get_mut(&format!("cancel:{id}")) {
            j.status = JobStatus::Confirmed
        }
        for j in l.outbox.values_mut().filter(|j| j.order_id == id) {
            j.status = JobStatus::Confirmed;
        }
    }
    ledger(s.accounts.get_mut(buyer).unwrap())
        .chain_inbox
        .insert(event, r.clone());
    Ok(())
}

#[cfg(test)]
mod tests;
