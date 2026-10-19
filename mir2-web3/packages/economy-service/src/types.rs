use mir2_economy_policy::{ConversionQuote, PoolState, SettlementPlan};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EconomyError {
    InvalidInput(&'static str),
    InvalidPolicy,
    Disabled,
    RealmMissing,
    PolicyConflict,
    RequestConflict,
    SettlementRequired {
        next_day: i64,
        current_day: i64,
    },
    InsufficientBoundGold,
    InsufficientPermits,
    Unauthenticated,
    AccountUnavailable,
    CharacterNotFound,
    Overflow(&'static str),
    CorruptHistory(&'static str),
    Database {
        sqlstate: Option<String>,
    },
    /// COMMIT may have succeeded. Retry/query the ORIGINAL operation; no refund.
    OutcomeUnknown,
    SourceRejected,
    #[cfg(any(test, feature = "test-support"))]
    InjectedFailure,
}
impl fmt::Display for EconomyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InsufficientPermits => f.write_str("当前全服变现额度不足，请等待。"),
            Self::Database { sqlstate } => write!(
                f,
                "economy database error ({})",
                sqlstate.as_deref().unwrap_or("connection")
            ),
            Self::OutcomeUnknown => {
                f.write_str("economy commit outcome unknown; recover the original request")
            }
            other => write!(f, "{other:?}"),
        }
    }
}
impl std::error::Error for EconomyError {}
pub type EconomyResult<T> = Result<T, EconomyError>;

/// The authenticated game server derives this identity, never a browser body.
/// `incarnation` is stable for one creation, and MUST change after delete/recreate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalletOwner {
    pub account_id: String,
    pub character_index: i32,
    pub incarnation: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BurnLayer {
    Daily,
    LightPvp,
    Siege,
}
impl BurnLayer {
    pub(crate) fn as_db(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::LightPvp => "light_pvp",
            Self::Siege => "siege",
        }
    }
}

/// Trusted server-only source description. No Deserialize implementation: wire
/// parsing must never turn a player's claims into a source authorization.
#[derive(Debug, Clone)]
pub struct TrustedBurn {
    pub source_kind: String,
    pub source_key: String,
    pub layer: BurnLayer,
    pub gold_equivalent: u64,
    pub valuation_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct WalletBalance {
    pub bound_gold: u64,
    pub unbound_gold: u64,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealmState {
    pub policy_version: String,
    pub pool: PoolState,
    pub genesis_day: i64,
    pub settled_through_day: i64,
    pub revision: u64,
}

/// One lock-qualified read. A missing wallet remains absent; a GET never mints
/// assets or provisions a wallet as a side effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedWalletSnapshot {
    pub owner: WalletOwner,
    pub wallet: Option<WalletBalance>,
    pub realm: RealmState,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversionReceipt {
    pub realm_id: String,
    pub operation_key: String,
    pub request_digest: String,
    pub owner: WalletOwner,
    pub request_id: String,
    pub policy_version: String,
    pub business_day: i64,
    pub committed_at_ms: i64,
    pub quote: ConversionQuote,
    pub wallet_after: WalletBalance,
    pub pool_after_atoms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundCreditReceipt {
    pub realm_id: String,
    pub operation_key: String,
    pub request_digest: String,
    pub owner: WalletOwner,
    pub source_key: String,
    pub bound_gold_credit: u64,
    pub business_day: i64,
    pub committed_at_ms: i64,
    pub wallet_after: WalletBalance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BurnReceipt {
    pub realm_id: String,
    pub operation_key: String,
    pub request_digest: String,
    pub event_id: String,
    pub gold_equivalent: u64,
    pub layer: BurnLayer,
    pub business_day: i64,
    pub committed_at_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct BurnTotals {
    pub daily: u64,
    pub light_pvp: u64,
    pub siege: u64,
}
impl BurnTotals {
    pub fn total(self) -> EconomyResult<u64> {
        self.daily
            .checked_add(self.light_pvp)
            .and_then(|n| n.checked_add(self.siege))
            .ok_or(EconomyError::Overflow("burn_total"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettlementReceipt {
    pub realm_id: String,
    /// This is the CLOSED day's key, whose destruction creates new permits.
    pub business_day: i64,
    pub policy_version: String,
    pub committed_at_ms: i64,
    pub burns: BurnTotals,
    pub plan: SettlementPlan,
    pub consumed_atoms: u64,
    pub gross_conversion_gold: u64,
    pub net_unbound_gold_inflow: u64,
    /// Exact decimal totals; the sum of many BIGINT wallets may exceed BIGINT.
    pub bound_gold_stock: String,
    pub unbound_gold_stock: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Idempotent<T> {
    pub receipt: T,
    pub replayed: bool,
}

/// A trusted source callback uses this SAME transaction and database timestamp.
#[derive(Debug, Clone, Copy)]
pub struct SourceContext {
    pub business_day: i64,
    pub database_time_ms: i64,
}

/// Test-only fault injection never appears in production request deserialization.
#[cfg(any(test, feature = "test-support"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestFault {
    BeforeCommit,
    CommitAcknowledgementLost,
}
