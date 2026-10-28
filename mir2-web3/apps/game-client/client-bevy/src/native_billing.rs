//! Account billing presentation. Only correlated server replies settle requests.
use std::sync::atomic::{AtomicU64, Ordering};
use web_time::{SystemTime, UNIX_EPOCH};

use bevy::prelude::*;
use serde::Deserialize;

use crate::native_monthly_card::MonthlyCardStatus;
use crate::native_shell::{NativeShellModel, NativeShellScreen};

static REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);
const PAGE_SIZE: usize = 3;
const REQUEST_TIMEOUT_MS: u64 = 15_000;

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BillingOperation {
    Status,
    Checkout,
    UsdcCheckout,
    BuyMonthlyCard,
    ActivateMonthlyCard,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BillingOffer {
    pub id: String,
    pub label: String,
    pub currency: String,
    pub amount_minor: u64,
    pub amount_label: String,
    pub credits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OwnedMonthlyCard {
    pub unique_id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BillingOrder {
    pub id: String,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SuiUsdcOffer {
    pub id: String,
    pub label: String,
    pub amount_micros: String,
    pub credits: u32,
    pub network: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SuiUsdcOrderState {
    Prepared,
    InvoicePending,
    AwaitingPayment,
    PaymentSubmitted,
    PaidPending,
    Applied,
    Expired,
    Review,
}

impl SuiUsdcOrderState {
    fn label(self) -> &'static str {
        match self {
            Self::Prepared | Self::InvoicePending => "billing.usdcPreparing",
            Self::AwaitingPayment => "billing.usdcAwaiting",
            Self::PaymentSubmitted => "billing.usdcSubmitted",
            Self::PaidPending => "billing.usdcPaidPending",
            Self::Applied => "billing.usdcApplied",
            Self::Expired => "billing.usdcExpired",
            Self::Review => "billing.usdcReview",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SuiUsdcOrder {
    pub id: String,
    pub order_request_id: String,
    pub offer_id: String,
    pub credits: u32,
    pub amount_micros: String,
    pub network: String,
    pub state: SuiUsdcOrderState,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
    pub transaction_digest: Option<String>,
}

fn valid_billing_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
}

fn valid_micros(value: &str) -> bool {
    value
        .parse::<u64>()
        .is_ok_and(|n| n > 0 && n.to_string() == value)
}

pub fn usdc_amount_label(value: &str) -> String {
    let Ok(micros) = value.parse::<u64>() else {
        return "? USDC".into();
    };
    let fraction = format!("{:06}", micros % 1_000_000);
    let fraction = fraction.trim_end_matches('0');
    if fraction.is_empty() {
        format!("{} USDC", micros / 1_000_000)
    } else {
        format!("{}.{} USDC", micros / 1_000_000, fraction)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BillingStatus {
    pub enabled: bool,
    pub checkout_enabled: bool,
    pub credit: u32,
    pub pending_credit: u32,
    pub offers: Vec<BillingOffer>,
    pub monthly_card_credit_price: Option<u32>,
    pub owned_monthly_cards: Vec<OwnedMonthlyCard>,
    pub monthly_card: MonthlyCardStatus,
    pub orders: Vec<BillingOrder>,
    #[serde(default)]
    pub sui_usdc_enabled: bool,
    #[serde(default)]
    pub sui_usdc_checkout_origin: Option<String>,
    #[serde(default)]
    pub sui_usdc_offers: Vec<SuiUsdcOffer>,
    #[serde(default)]
    pub sui_usdc_orders: Vec<SuiUsdcOrder>,
}

impl BillingStatus {
    pub fn is_valid(&self) -> bool {
        self.offers.len() <= 64
            && self.owned_monthly_cards.len() <= 256
            && self.orders.len() <= 256
            && self.sui_usdc_offers.len() <= 64
            && self.sui_usdc_orders.len() <= 256
            && self
                .sui_usdc_checkout_origin
                .as_ref()
                .is_none_or(|o| o.len() <= 2048 && o.is_ascii())
            && self.sui_usdc_offers.iter().all(|o| {
                valid_billing_id(&o.id)
                    && o.label.len() <= 256
                    && o.credits > 0
                    && valid_micros(&o.amount_micros)
                    && o.network == "testnet"
            })
            && self.sui_usdc_orders.iter().all(|o| {
                valid_billing_id(&o.id)
                    && valid_billing_id(&o.order_request_id)
                    && valid_billing_id(&o.offer_id)
                    && o.credits > 0
                    && valid_micros(&o.amount_micros)
                    && o.network == "testnet"
                    && o.created_at_ms < o.expires_at_ms
                    && o.transaction_digest.as_ref().is_none_or(|d| {
                        d.len() >= 32
                            && d.len() <= 64
                            && d.bytes().all(|b| b.is_ascii_alphanumeric())
                    })
            })
            && self.offers.iter().all(|o| {
                !o.id.is_empty()
                    && o.id.len() <= 96
                    && o.id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
                    && o.label.len() <= 256
                    && o.amount_label.len() <= 128
                    && o.currency.len() <= 16
            })
            && self
                .owned_monthly_cards
                .iter()
                .all(|c| canonical_item_id(&c.unique_id) && c.label.len() <= 256)
            && self
                .orders
                .iter()
                .all(|o| o.id.len() <= 128 && o.state.len() <= 64)
    }
}

pub fn canonical_item_id(value: &str) -> bool {
    value
        .parse::<u64>()
        .is_ok_and(|id| id > 0 && id.to_string() == value)
}

#[derive(Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BillingReply {
    pub operation: BillingOperation,
    pub request_id: u64,
    pub character_index: i32,
    pub status: Option<BillingStatus>,
    pub checkout_url: Option<String>,
    pub error: Option<String>,
    pub replayed: Option<bool>,
}

impl std::fmt::Debug for BillingReply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BillingReply")
            .field("operation", &self.operation)
            .field("request_id", &self.request_id)
            .field("character_index", &self.character_index)
            .field(
                "checkout_url",
                &self.checkout_url.as_ref().map(|_| "[REDACTED]"),
            )
            .field("has_error", &self.error.is_some())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BillingPurchase {
    Checkout {
        offer_id: String,
        order_request_id: String,
    },
    UsdcCheckout {
        offer_id: String,
        order_request_id: String,
    },
    BuyMonthlyCard {
        order_request_id: String,
    },
    ActivateMonthlyCard {
        item_unique_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BillingRequest {
    pub operation: BillingOperation,
    pub request_id: u64,
    pub character_index: i32,
    pub purchase: Option<BillingPurchase>,
    account: String,
    started_at_ms: u64,
    sent: bool,
}

impl BillingRequest {
    pub fn belongs_to_account(&self, account: &str) -> bool {
        self.account == account
    }
    pub fn expired_at(&self, now: u64) -> bool {
        now.saturating_sub(self.started_at_ms) >= REQUEST_TIMEOUT_MS
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Component)]
pub enum BillingAction {
    Refresh,
    Checkout(String),
    UsdcCheckout(String),
    UsdcReopen(String),
    UsdcPage,
    UsdcOffersPage(bool),
    UsdcOrdersPage(bool),
    BuyMonthlyCard,
    Activate(String),
    Retry,
    Close,
    OffersPage(bool),
    CardsPage(bool),
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct BillingPanel {
    pub open: bool,
    pub status: Option<BillingStatus>,
    pub pending: Option<BillingRequest>,
    pub message: Option<&'static str>,
    pub focus: usize,
    pub offer_page: usize,
    pub card_page: usize,
    pub usdc_page: bool,
    pub usdc_offer_page: usize,
    pub usdc_order_page: usize,
    account: String,
    character_index: Option<i32>,
    last_purchase: Option<BillingPurchase>,
    retry_required: bool,
    checkout_to_open: Option<String>,
    input_ready: bool,
    trusted_sui_checkout_origin: Option<String>,
    last_status_at_ms: u64,
}

impl std::fmt::Debug for BillingPanel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BillingPanel")
            .field("open", &self.open)
            .field("character_index", &self.character_index)
            .field("pending", &self.pending)
            .field("message", &self.message)
            .finish()
    }
}

impl BillingPanel {
    /// Derived by the host from its configured Gateway endpoint, never from a reply.
    pub fn set_sui_checkout_origin(&mut self, origin: Option<String>) {
        self.trusted_sui_checkout_origin = origin;
    }

    pub fn refresh_pending_usdc(&mut self, now: u64) {
        if self.open
            && self.usdc_page
            && self.pending.is_none()
            && now.saturating_sub(self.last_status_at_ms) >= 5_000
            && self.status.as_ref().is_some_and(|s| {
                s.sui_usdc_orders.iter().any(|o| {
                    !matches!(
                        o.state,
                        SuiUsdcOrderState::Applied
                            | SuiUsdcOrderState::Expired
                            | SuiUsdcOrderState::Review
                    )
                })
            })
        {
            self.begin(None, now);
        }
    }

    fn usdc_available(&self) -> bool {
        self.status
            .as_ref()
            .is_some_and(|s| s.sui_usdc_enabled && self.usdc_origin_matches())
    }

    fn usdc_origin_matches(&self) -> bool {
        self.trusted_sui_checkout_origin.is_some()
            && self
                .status
                .as_ref()
                .is_some_and(|s| s.sui_usdc_checkout_origin == self.trusted_sui_checkout_origin)
    }

    fn can_reopen_usdc(&self) -> bool {
        self.pending.is_none() && !self.retry_required && self.usdc_origin_matches()
    }
    pub fn open_for(&mut self, account: &str, character: i32, now: u64) {
        if self.account != account || self.character_index != Some(character) {
            *self = Self::default();
            self.account = account.into();
            self.character_index = Some(character);
        }
        if !self.open {
            self.input_ready = false;
        }
        self.open = true;
        if self.pending.is_none() {
            self.begin(None, now);
        }
    }

    fn begin(&mut self, purchase: Option<BillingPurchase>, now: u64) -> bool {
        if self.pending.is_some() || !self.open {
            return false;
        }
        let Some(character_index) = self.character_index else {
            return false;
        };
        let operation = match &purchase {
            None => BillingOperation::Status,
            Some(BillingPurchase::Checkout { .. }) => BillingOperation::Checkout,
            Some(BillingPurchase::UsdcCheckout { .. }) => BillingOperation::UsdcCheckout,
            Some(BillingPurchase::BuyMonthlyCard { .. }) => BillingOperation::BuyMonthlyCard,
            Some(BillingPurchase::ActivateMonthlyCard { .. }) => {
                BillingOperation::ActivateMonthlyCard
            }
        };
        if purchase.is_some() {
            self.last_purchase = purchase.clone();
        }
        self.pending = Some(BillingRequest {
            operation,
            request_id: REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed),
            character_index,
            purchase,
            account: self.account.clone(),
            started_at_ms: now,
            sent: false,
        });
        true
    }

    pub fn unsent_request(&self) -> Option<&BillingRequest> {
        self.pending.as_ref().filter(|r| !r.sent)
    }
    pub fn scope_matches(&self, account: &str, character: Option<i32>) -> bool {
        self.account == account && self.character_index == character
    }
    pub fn mark_sent(&mut self, id: u64) {
        if let Some(r) = self.pending.as_mut().filter(|r| r.request_id == id) {
            r.sent = true;
        }
    }
    pub fn take_checkout_url(&mut self) -> Option<String> {
        self.checkout_to_open.take()
    }
    pub fn browser_failed(&mut self) {
        self.message = Some("billing.browserFailed");
        self.retry_required = true;
    }

    pub fn expire_request(&mut self, now: u64) {
        if self
            .pending
            .as_ref()
            .is_some_and(|r| now.saturating_sub(r.started_at_ms) >= REQUEST_TIMEOUT_MS)
        {
            self.retry_required |= self.pending.as_ref().is_some_and(|r| r.purchase.is_some());
            self.pending = None;
            self.message = Some("billing.unconfirmed");
        }
    }

    pub fn accept(
        &mut self,
        reply: BillingReply,
        account: &str,
        character: Option<i32>,
        now: u64,
    ) -> bool {
        let Some(pending) = &self.pending else {
            return false;
        };
        if !pending.sent
            || pending.account != account
            || self.account != account
            || character != Some(pending.character_index)
            || reply.character_index != pending.character_index
            || reply.request_id != pending.request_id
            || reply.operation != pending.operation
            || reply.status.as_ref().is_some_and(|s| !s.is_valid())
        {
            return false;
        }
        // URLs are allowed only on a successful, current Checkout result.
        if reply.checkout_url.as_ref().is_some_and(|url| {
            url.len() > 4096
                || !matches!(
                    reply.operation,
                    BillingOperation::Checkout | BillingOperation::UsdcCheckout
                )
                || reply.error.is_some()
        }) {
            return false;
        }
        self.pending = None;
        let has_status = reply.status.is_some();
        if let Some(status) = reply.status {
            self.last_status_at_ms = now;
            self.offer_page = self
                .offer_page
                .min(status.offers.len().saturating_sub(1) / PAGE_SIZE);
            self.card_page = self
                .card_page
                .min(status.owned_monthly_cards.len().saturating_sub(1) / PAGE_SIZE);
            self.status = Some(status);
            if let Some(status) = &self.status {
                self.usdc_offer_page = self
                    .usdc_offer_page
                    .min(status.sui_usdc_offers.len().saturating_sub(1) / PAGE_SIZE);
                self.usdc_order_page = self
                    .usdc_order_page
                    .min(status.sui_usdc_orders.len().saturating_sub(1) / PAGE_SIZE);
            }
        }
        if reply.error.is_some() {
            let unconfirmed = matches!(
                reply.error.as_deref(),
                Some(
                    "billingProviderResultUnconfirmed"
                        | "billingResultUnconfirmed"
                        | "billingCommitOutcomeUnknown"
                        | "billingServiceUnavailable"
                )
            );
            if reply.operation == BillingOperation::Status {
                if !has_status {
                    self.status = None;
                }
            } else if unconfirmed {
                self.retry_required = true;
            } else {
                // A correlated rejection finishes this intent. A later purchase
                // is deliberate; timeout retries keep the original intent.
                self.last_purchase = None;
                self.retry_required = false;
            }
            self.message = Some(if unconfirmed {
                "billing.unconfirmed"
            } else {
                match reply.error.as_deref() {
                    Some("billingInsufficientCredit" | "billingInsufficientCredits") => {
                        "billing.insufficientCredits"
                    }
                    Some("billingInventoryFull") => "billing.inventoryFull",
                    Some("billingUnavailable") => "billing.unavailable",
                    Some("billingMonthlyCardDisabled") => "billing.monthlyDisabled",
                    _ => "billing.failed",
                }
            });
            return true;
        }
        match reply.operation {
            BillingOperation::Checkout | BillingOperation::UsdcCheckout => {
                if let Some(url) = reply.checkout_url {
                    self.checkout_to_open = Some(url);
                    self.retry_required = false;
                    self.message = Some("billing.waiting");
                } else {
                    self.retry_required = true;
                    self.message = Some("billing.unconfirmed");
                }
            }
            BillingOperation::BuyMonthlyCard | BillingOperation::ActivateMonthlyCard => {
                self.last_purchase = None;
                self.retry_required = false;
                self.message = Some("billing.done");
                // A precise operation result is followed by an authoritative refresh.
                self.begin(None, now);
            }
            BillingOperation::Status => {
                if !has_status {
                    self.status = None;
                }
                self.message = Some(if self.retry_required || !has_status {
                    "billing.unconfirmed"
                } else {
                    "billing.updated"
                });
            }
        }
        true
    }

    pub fn action(&mut self, action: BillingAction, now: u64) {
        match action {
            BillingAction::Close => {
                self.retry_required |= self.pending.as_ref().is_some_and(|r| r.purchase.is_some());
                self.pending = None;
                self.checkout_to_open = None;
                self.open = false;
            }
            BillingAction::Refresh => {
                self.begin(None, now);
            }
            BillingAction::UsdcPage if self.pending.is_none() => {
                self.usdc_page = !self.usdc_page;
                self.focus = 0;
            }
            BillingAction::UsdcOffersPage(next) => {
                self.usdc_offer_page = page(
                    self.usdc_offer_page,
                    next,
                    self.status.as_ref().map_or(0, |s| s.sui_usdc_offers.len()),
                );
                self.focus = 0;
            }
            BillingAction::UsdcOrdersPage(next) => {
                self.usdc_order_page = page(
                    self.usdc_order_page,
                    next,
                    self.status.as_ref().map_or(0, |s| s.sui_usdc_orders.len()),
                );
                self.focus = 0;
            }
            BillingAction::Retry if self.retry_required => {
                self.begin(self.last_purchase.clone(), now);
            }
            BillingAction::OffersPage(next) => {
                self.offer_page = page(
                    self.offer_page,
                    next,
                    self.status.as_ref().map_or(0, |s| s.offers.len()),
                );
                self.focus = 0;
            }
            BillingAction::CardsPage(next) => {
                self.card_page = page(
                    self.card_page,
                    next,
                    self.status
                        .as_ref()
                        .map_or(0, |s| s.owned_monthly_cards.len()),
                );
                self.focus = 0;
            }
            BillingAction::Checkout(id)
                if self.can_purchase()
                    && self.status.as_ref().is_some_and(|s| {
                        s.checkout_enabled && s.offers.iter().any(|o| o.id == id)
                    }) =>
            {
                self.begin(
                    Some(BillingPurchase::Checkout {
                        offer_id: id,
                        order_request_id: new_order_id(now),
                    }),
                    now,
                );
            }
            BillingAction::UsdcCheckout(id)
                if self.can_purchase()
                    && self.usdc_available()
                    && self
                        .status
                        .as_ref()
                        .is_some_and(|s| s.sui_usdc_offers.iter().any(|o| o.id == id)) =>
            {
                self.begin(
                    Some(BillingPurchase::UsdcCheckout {
                        offer_id: id,
                        order_request_id: new_order_id(now),
                    }),
                    now,
                );
            }
            BillingAction::UsdcReopen(id) if self.can_reopen_usdc() => {
                if let Some(order) = self.status.as_ref().and_then(|s| {
                    s.sui_usdc_orders.iter().find(|o| {
                        o.id == id
                            && !matches!(
                                o.state,
                                SuiUsdcOrderState::Applied
                                    | SuiUsdcOrderState::Expired
                                    | SuiUsdcOrderState::Review
                            )
                    })
                }) {
                    self.begin(
                        Some(BillingPurchase::UsdcCheckout {
                            offer_id: order.offer_id.clone(),
                            order_request_id: order.order_request_id.clone(),
                        }),
                        now,
                    );
                }
            }
            BillingAction::BuyMonthlyCard
                if self.can_purchase()
                    && self.status.as_ref().is_some_and(|s| {
                        s.monthly_card_credit_price.is_some_and(|p| s.credit >= p)
                    }) =>
            {
                self.begin(
                    Some(BillingPurchase::BuyMonthlyCard {
                        order_request_id: new_order_id(now),
                    }),
                    now,
                );
            }
            BillingAction::Activate(id)
                if self.can_purchase()
                    && self.status.as_ref().is_some_and(|s| {
                        s.owned_monthly_cards.iter().any(|c| c.unique_id == id)
                    }) =>
            {
                self.begin(
                    Some(BillingPurchase::ActivateMonthlyCard { item_unique_id: id }),
                    now,
                );
            }
            _ => {}
        }
    }

    fn can_purchase(&self) -> bool {
        self.pending.is_none()
            && !self.retry_required
            && self.status.as_ref().is_some_and(|s| s.enabled)
    }
    fn actions(&self) -> Vec<BillingAction> {
        let mut actions = vec![BillingAction::Refresh, BillingAction::Close];
        actions.push(BillingAction::UsdcPage);
        if self.retry_required && self.last_purchase.is_some() {
            actions.push(BillingAction::Retry);
        }
        if let Some(status) = &self.status {
            if self.usdc_page {
                if self.can_purchase() && self.usdc_available() {
                    actions.extend(
                        status
                            .sui_usdc_offers
                            .iter()
                            .skip(self.usdc_offer_page * PAGE_SIZE)
                            .take(PAGE_SIZE)
                            .map(|o| BillingAction::UsdcCheckout(o.id.clone())),
                    );
                }
                if self.can_reopen_usdc() {
                    actions.extend(
                        status
                            .sui_usdc_orders
                            .iter()
                            .skip(self.usdc_order_page * PAGE_SIZE)
                            .take(PAGE_SIZE)
                            .filter(|o| {
                                !matches!(
                                    o.state,
                                    SuiUsdcOrderState::Applied
                                        | SuiUsdcOrderState::Expired
                                        | SuiUsdcOrderState::Review
                                )
                            })
                            .map(|o| BillingAction::UsdcReopen(o.id.clone())),
                    );
                }
                if status.sui_usdc_offers.len() > PAGE_SIZE {
                    actions.extend([
                        BillingAction::UsdcOffersPage(false),
                        BillingAction::UsdcOffersPage(true),
                    ]);
                }
                if status.sui_usdc_orders.len() > PAGE_SIZE {
                    actions.extend([
                        BillingAction::UsdcOrdersPage(false),
                        BillingAction::UsdcOrdersPage(true),
                    ]);
                }
                if self.pending.is_some() {
                    actions.retain(|a| matches!(a, BillingAction::Close));
                }
                return actions;
            }
            if self.can_purchase() {
                if status.checkout_enabled {
                    actions.extend(
                        status
                            .offers
                            .iter()
                            .skip(self.offer_page * PAGE_SIZE)
                            .take(PAGE_SIZE)
                            .map(|o| BillingAction::Checkout(o.id.clone())),
                    );
                }
                if status
                    .monthly_card_credit_price
                    .is_some_and(|p| status.credit >= p)
                {
                    actions.push(BillingAction::BuyMonthlyCard);
                }
                actions.extend(
                    status
                        .owned_monthly_cards
                        .iter()
                        .skip(self.card_page * PAGE_SIZE)
                        .take(PAGE_SIZE)
                        .map(|c| BillingAction::Activate(c.unique_id.clone())),
                );
            }
            if status.offers.len() > PAGE_SIZE {
                actions.extend([
                    BillingAction::OffersPage(false),
                    BillingAction::OffersPage(true),
                ]);
            }
            if status.owned_monthly_cards.len() > PAGE_SIZE {
                actions.extend([
                    BillingAction::CardsPage(false),
                    BillingAction::CardsPage(true),
                ]);
            }
        }
        if self.pending.is_some() {
            actions.retain(|a| matches!(a, BillingAction::Close));
        }
        actions
    }
}

fn page(current: usize, next: bool, count: usize) -> usize {
    let last = count.saturating_sub(1) / PAGE_SIZE;
    if next {
        current.saturating_add(1).min(last)
    } else {
        current.saturating_sub(1)
    }
}
fn new_order_id(now: u64) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!(
        "native-{now}-{nanos}-{}",
        REQUEST_SEQUENCE.load(Ordering::Relaxed)
    )
}

#[cfg(feature = "native-ui")]
mod ui {
    use super::*;
    use bevy::ui::FocusPolicy;
    #[derive(Component)]
    pub struct BillingButton {
        action: BillingAction,
        enabled: bool,
    }

    pub fn process_input(
        mut shell: ResMut<NativeShellModel>,
        keys: Res<ButtonInput<KeyCode>>,
        buttons: Query<(&Interaction, &BillingButton), Changed<Interaction>>,
        mut ui: Option<ResMut<crate::crystal_ui::overlays::NativePlayerUiState>>,
    ) {
        if shell.billing.open
            && !shell.billing.scope_matches(
                shell.last_account.as_deref().unwrap_or_default(),
                shell.billing_character_index(),
            )
        {
            shell.billing = Default::default();
        }
        shell.billing.expire_request(now_ms());
        shell.billing.refresh_pending_usdc(now_ms());
        let was_open = shell.billing.open;
        let input_ready = shell.billing.input_ready;
        shell.billing.input_ready = shell.billing.open;
        // The Enter that opens this panel belongs to the monthly-card button.
        if shell.billing.open && input_ready {
            let actions = shell.billing.actions();
            if keys.just_pressed(KeyCode::Escape) {
                shell.billing.action(BillingAction::Close, now_ms());
            } else if keys.just_pressed(KeyCode::Tab) && !actions.is_empty() {
                let backwards =
                    keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
                shell.billing.focus = if backwards {
                    (shell.billing.focus + actions.len() - 1) % actions.len()
                } else {
                    (shell.billing.focus + 1) % actions.len()
                };
            } else if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
                if let Some(action) = actions.get(shell.billing.focus % actions.len().max(1)) {
                    shell.billing.action(action.clone(), now_ms());
                }
            }
            for (interaction, button) in &buttons {
                if *interaction == Interaction::Pressed && button.enabled && shell.billing.open {
                    shell.billing.action(button.action.clone(), now_ms());
                    break;
                }
            }
        }
        if !matches!(
            shell.screen,
            NativeShellScreen::CharacterSelect | NativeShellScreen::InGame
        ) {
            shell.billing = Default::default();
        }
        if let Some(ui) = ui.as_deref_mut() {
            ui.billing_open = shell.billing.open;
            ui.billing_input_consumed = was_open && !shell.billing.open;
        }
    }

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Node {
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(x),
            top: Val::Px(y),
            width: Val::Px(w),
            height: Val::Px(h),
            ..default()
        }
    }
    fn text(
        parent: &mut ChildSpawnerCommands,
        value: String,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        size: f32,
    ) {
        let mut bounds = rect(x, y, w, h);
        bounds.overflow = Overflow::clip();
        parent.spawn((
            bounds,
            Text::new(value),
            crate::crystal_ui::typography::crystal_text_font(size),
            TextColor(Color::srgb(0.94, 0.89, 0.75)),
        ));
    }
    fn button(
        parent: &mut ChildSpawnerCommands,
        panel: &BillingPanel,
        action: BillingAction,
        label: &str,
        x: f32,
        y: f32,
        w: f32,
    ) {
        let actions = panel.actions();
        let enabled = actions.contains(&action);
        let focused = actions.get(panel.focus % actions.len().max(1)) == Some(&action);
        let mut bounds = rect(x, y, w, 30.0);
        bounds.border = UiRect::all(Val::Px(1.0));
        parent
            .spawn((
                bounds,
                Button,
                FocusPolicy::Block,
                BillingButton { action, enabled },
                BackgroundColor(if enabled {
                    Color::srgb(0.19, 0.15, 0.09)
                } else {
                    Color::srgb(0.10, 0.10, 0.10)
                }),
                BorderColor::all(if focused {
                    Color::srgb(1.0, 0.82, 0.25)
                } else {
                    Color::srgb(0.45, 0.36, 0.20)
                }),
            ))
            .with_children(|b| {
                text(
                    b,
                    crate::native_i18n::tr(label),
                    6.0,
                    3.0,
                    w - 12.0,
                    26.0,
                    12.0,
                )
            });
    }

    pub fn render(parent: &mut ChildSpawnerCommands, panel: &BillingPanel) {
        use crate::native_i18n::tr;
        if !panel.open {
            return;
        }
        parent
            .spawn((
                rect(0.0, 0.0, 1024.0, 768.0),
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.80)),
                FocusPolicy::Block,
                GlobalZIndex(6000),
            ))
            .with_children(|layer| {
                layer
                    .spawn((
                        rect(142.0, 45.0, 740.0, 682.0),
                        BackgroundColor(Color::srgb(0.06, 0.045, 0.03)),
                        BorderColor::all(Color::srgb(0.65, 0.49, 0.20)),
                    ))
                    .with_children(|p| {
                        text(p, tr("billing.title"), 20.0, 14.0, 470.0, 30.0, 23.0);
                        button(
                            p,
                            panel,
                            BillingAction::UsdcPage,
                            if panel.usdc_page {
                                "billing.usdcBack"
                            } else {
                                "billing.usdcTab"
                            },
                            500.0,
                            14.0,
                            210.0,
                        );
                        text(
                            p,
                            tr(if panel.usdc_page {
                                "billing.usdcInstructions"
                            } else {
                                "billing.instructions"
                            }),
                            20.0,
                            48.0,
                            700.0,
                            42.0,
                            13.0,
                        );
                        if let Some(s) = &panel.status {
                            text(
                                p,
                                tr("billing.balance")
                                    .replace("{0}", &s.credit.to_string())
                                    .replace("{1}", &s.pending_credit.to_string()),
                                20.0,
                                94.0,
                                700.0,
                                30.0,
                                17.0,
                            );
                            let access = if !s.monthly_card.required {
                                "billing.optional"
                            } else if s.monthly_card.active {
                                "billing.active"
                            } else {
                                "billing.expired"
                            };
                            let expiry = s
                                .monthly_card
                                .expires_at_ms
                                .and_then(|ms| i64::try_from(ms).ok())
                                .and_then(chrono::DateTime::<chrono::Utc>::from_timestamp_millis)
                                .map(|d| {
                                    format!(
                                        "\n{} {} UTC",
                                        tr("billing.expires"),
                                        d.format("%Y-%m-%d %H:%M")
                                    )
                                })
                                .unwrap_or_default();
                            text(
                                p,
                                if panel.usdc_page {
                                    tr("billing.usdcGas")
                                } else {
                                    format!("{}{}", tr(access), expiry)
                                },
                                20.0,
                                132.0,
                                700.0,
                                50.0,
                                13.0,
                            );
                            if !s.enabled && !panel.usdc_page {
                                text(p, tr("billing.unavailable"), 20.0, 190.0, 700.0, 42.0, 15.0);
                            } else if panel.usdc_page {
                                text(
                                    p,
                                    tr(if panel.usdc_available() {
                                        "billing.usdcOffers"
                                    } else {
                                        "billing.usdcUnavailable"
                                    }),
                                    20.0,
                                    190.0,
                                    500.0,
                                    35.0,
                                    14.0,
                                );
                                if s.sui_usdc_offers.len() > PAGE_SIZE {
                                    button(
                                        p,
                                        panel,
                                        BillingAction::UsdcOffersPage(false),
                                        "billing.previous",
                                        524.0,
                                        185.0,
                                        88.0,
                                    );
                                    button(
                                        p,
                                        panel,
                                        BillingAction::UsdcOffersPage(true),
                                        "billing.next",
                                        622.0,
                                        185.0,
                                        88.0,
                                    );
                                }
                                for (i, offer) in s
                                    .sui_usdc_offers
                                    .iter()
                                    .skip(panel.usdc_offer_page * PAGE_SIZE)
                                    .take(PAGE_SIZE)
                                    .enumerate()
                                {
                                    let y = 225.0 + i as f32 * 48.0;
                                    text(
                                        p,
                                        format!(
                                            "{} · {} · {}",
                                            offer.label,
                                            usdc_amount_label(&offer.amount_micros),
                                            tr("billing.points")
                                                .replace("{0}", &offer.credits.to_string())
                                        ),
                                        20.0,
                                        y,
                                        510.0,
                                        42.0,
                                        13.0,
                                    );
                                    button(
                                        p,
                                        panel,
                                        BillingAction::UsdcCheckout(offer.id.clone()),
                                        "billing.usdcPay",
                                        550.0,
                                        y + 4.0,
                                        160.0,
                                    );
                                }
                                text(p, tr("billing.usdcOrders"), 20.0, 392.0, 500.0, 30.0, 14.0);
                                if s.sui_usdc_orders.len() > PAGE_SIZE {
                                    button(
                                        p,
                                        panel,
                                        BillingAction::UsdcOrdersPage(false),
                                        "billing.previous",
                                        524.0,
                                        387.0,
                                        88.0,
                                    );
                                    button(
                                        p,
                                        panel,
                                        BillingAction::UsdcOrdersPage(true),
                                        "billing.next",
                                        622.0,
                                        387.0,
                                        88.0,
                                    );
                                }
                                if s.sui_usdc_orders.is_empty() {
                                    text(
                                        p,
                                        tr("billing.usdcNoOrders"),
                                        20.0,
                                        430.0,
                                        700.0,
                                        32.0,
                                        13.0,
                                    );
                                }
                                for (i, order) in s
                                    .sui_usdc_orders
                                    .iter()
                                    .skip(panel.usdc_order_page * PAGE_SIZE)
                                    .take(PAGE_SIZE)
                                    .enumerate()
                                {
                                    let short_id: String = order.id.chars().take(18).collect();
                                    text(
                                        p,
                                        format!(
                                            "{} · {} · {}\n{}",
                                            short_id,
                                            usdc_amount_label(&order.amount_micros),
                                            tr("billing.points")
                                                .replace("{0}", &order.credits.to_string()),
                                            tr(order.state.label())
                                        ),
                                        20.0,
                                        430.0 + i as f32 * 46.0,
                                        510.0,
                                        44.0,
                                        13.0,
                                    );
                                    if !matches!(
                                        order.state,
                                        SuiUsdcOrderState::Applied
                                            | SuiUsdcOrderState::Expired
                                            | SuiUsdcOrderState::Review
                                    ) {
                                        button(
                                            p,
                                            panel,
                                            BillingAction::UsdcReopen(order.id.clone()),
                                            "billing.usdcReopen",
                                            550.0,
                                            434.0 + i as f32 * 46.0,
                                            160.0,
                                        );
                                    }
                                }
                            } else {
                                text(
                                    p,
                                    tr(if s.checkout_enabled {
                                        "billing.offers"
                                    } else {
                                        "billing.checkoutUnavailable"
                                    }),
                                    20.0,
                                    190.0,
                                    500.0,
                                    35.0,
                                    14.0,
                                );
                                if s.offers.len() > PAGE_SIZE {
                                    button(
                                        p,
                                        panel,
                                        BillingAction::OffersPage(false),
                                        "billing.previous",
                                        524.0,
                                        185.0,
                                        88.0,
                                    );
                                    button(
                                        p,
                                        panel,
                                        BillingAction::OffersPage(true),
                                        "billing.next",
                                        622.0,
                                        185.0,
                                        88.0,
                                    );
                                }
                                for (i, o) in s
                                    .offers
                                    .iter()
                                    .skip(panel.offer_page * PAGE_SIZE)
                                    .take(PAGE_SIZE)
                                    .enumerate()
                                {
                                    let y = 225.0 + i as f32 * 48.0;
                                    text(
                                        p,
                                        format!(
                                            "{} · {} · {}",
                                            o.label,
                                            o.amount_label,
                                            tr("billing.points")
                                                .replace("{0}", &o.credits.to_string())
                                        ),
                                        20.0,
                                        y,
                                        510.0,
                                        42.0,
                                        13.0,
                                    );
                                    button(
                                        p,
                                        panel,
                                        BillingAction::Checkout(o.id.clone()),
                                        "billing.recharge",
                                        550.0,
                                        y + 4.0,
                                        160.0,
                                    );
                                }
                                if let Some(price) = s.monthly_card_credit_price {
                                    text(
                                        p,
                                        tr("billing.monthlyPrice")
                                            .replace("{0}", &price.to_string()),
                                        20.0,
                                        377.0,
                                        510.0,
                                        44.0,
                                        13.0,
                                    );
                                    button(
                                        p,
                                        panel,
                                        BillingAction::BuyMonthlyCard,
                                        "billing.buyMonthly",
                                        550.0,
                                        378.0,
                                        160.0,
                                    );
                                }
                                text(p, tr("billing.owned"), 20.0, 427.0, 500.0, 28.0, 14.0);
                                if s.owned_monthly_cards.len() > PAGE_SIZE {
                                    button(
                                        p,
                                        panel,
                                        BillingAction::CardsPage(false),
                                        "billing.previous",
                                        524.0,
                                        422.0,
                                        88.0,
                                    );
                                    button(
                                        p,
                                        panel,
                                        BillingAction::CardsPage(true),
                                        "billing.next",
                                        622.0,
                                        422.0,
                                        88.0,
                                    );
                                }
                                if s.owned_monthly_cards.is_empty() {
                                    text(p, tr("billing.noCards"), 20.0, 459.0, 510.0, 30.0, 13.0);
                                }
                                for (i, card) in s
                                    .owned_monthly_cards
                                    .iter()
                                    .skip(panel.card_page * PAGE_SIZE)
                                    .take(PAGE_SIZE)
                                    .enumerate()
                                {
                                    let y = 459.0 + i as f32 * 37.0;
                                    text(
                                        p,
                                        format!(
                                            "{} #{}",
                                            if card.label.is_empty() {
                                                tr("billing.card")
                                            } else {
                                                card.label.clone()
                                            },
                                            card.unique_id
                                        ),
                                        20.0,
                                        y,
                                        510.0,
                                        32.0,
                                        13.0,
                                    );
                                    button(
                                        p,
                                        panel,
                                        BillingAction::Activate(card.unique_id.clone()),
                                        "billing.activate",
                                        550.0,
                                        y,
                                        160.0,
                                    );
                                }
                            }
                        } else {
                            text(
                                p,
                                tr(if panel.pending.is_some() {
                                    "billing.checking"
                                } else {
                                    "billing.unavailable"
                                }),
                                20.0,
                                100.0,
                                700.0,
                                44.0,
                                15.0,
                            );
                        }
                        if let Some(message) = panel.message {
                            text(p, tr(message), 20.0, 575.0, 700.0, 48.0, 13.0);
                        }
                        button(
                            p,
                            panel,
                            BillingAction::Refresh,
                            "billing.refresh",
                            20.0,
                            635.0,
                            220.0,
                        );
                        if panel.retry_required && panel.last_purchase.is_some() {
                            button(
                                p,
                                panel,
                                BillingAction::Retry,
                                "billing.retry",
                                260.0,
                                635.0,
                                220.0,
                            );
                        }
                        button(
                            p,
                            panel,
                            BillingAction::Close,
                            "billing.close",
                            500.0,
                            635.0,
                            210.0,
                        );
                    });
            });
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        fn status(enabled: bool) -> BillingStatus {
            BillingStatus {
                enabled,
                checkout_enabled: true,
                credit: 900,
                pending_credit: 0,
                offers: vec![BillingOffer {
                    id: "small".into(),
                    label: "Small".into(),
                    currency: "usd".into(),
                    amount_minor: 100,
                    amount_label: "$1.00".into(),
                    credits: 100,
                }],
                monthly_card_credit_price: Some(900),
                owned_monthly_cards: vec![],
                monthly_card: MonthlyCardStatus {
                    required: true,
                    active: false,
                    expires_at_ms: None,
                    server_now_ms: 1,
                    remaining_ms: 0,
                    can_enter_game: false,
                },
                orders: vec![],
                sui_usdc_enabled: false,
                sui_usdc_checkout_origin: None,
                sui_usdc_offers: vec![],
                sui_usdc_orders: vec![],
            }
        }
        fn reply(request: &BillingRequest) -> BillingReply {
            BillingReply {
                operation: request.operation,
                request_id: request.request_id,
                character_index: request.character_index,
                status: None,
                checkout_url: None,
                error: None,
                replayed: None,
            }
        }
        fn usdc_status() -> BillingStatus {
            let mut s = status(true);
            s.sui_usdc_enabled = true;
            s.sui_usdc_checkout_origin = Some("http://127.0.0.1:17830".into());
            s.sui_usdc_offers = vec![SuiUsdcOffer {
                id: "usdc-small".into(),
                label: "Test package".into(),
                amount_micros: "1000000".into(),
                credits: 100,
                network: "testnet".into(),
            }];
            s
        }

        #[test]
        fn usdc_wire_requires_canonical_amounts_testnet_and_known_states() {
            for state in [
                "prepared",
                "invoicePending",
                "awaitingPayment",
                "paymentSubmitted",
                "paidPending",
                "applied",
                "expired",
                "review",
            ] {
                let payload = serde_json::json!({"id":"usdc_order", "orderRequestId":"native-original-usdc", "offerId":"usdc-small", "credits":100,
                    "amountMicros":"1000000", "network":"testnet", "state":state, "createdAtMs":1,
                    "expiresAtMs":100, "transactionDigest":null});
                let order: SuiUsdcOrder = serde_json::from_value(payload.clone()).unwrap();
                let mut s = usdc_status();
                s.sui_usdc_orders = vec![order];
                assert!(s.is_valid());
                let mut unknown = payload.clone();
                unknown["state"] = serde_json::json!("success");
                assert!(serde_json::from_value::<SuiUsdcOrder>(unknown).is_err());
                let mut extra = payload;
                extra["payerSecret"] = serde_json::json!("forbidden");
                assert!(serde_json::from_value::<SuiUsdcOrder>(extra).is_err());
            }
            for amount in ["0", "01", "1.0", "-1", "18446744073709551616"] {
                let mut s = usdc_status();
                s.sui_usdc_offers[0].amount_micros = amount.into();
                assert!(!s.is_valid());
            }
            let mut s = usdc_status();
            s.sui_usdc_offers[0].network = "mainnet".into();
            assert!(!s.is_valid());
            assert_eq!(usdc_amount_label("1"), "0.000001 USDC");
            assert_eq!(usdc_amount_label("1000000"), "1 USDC");
            assert_eq!(usdc_amount_label("1234500"), "1.2345 USDC");
            assert_eq!(
                usdc_amount_label("18446744073709551615"),
                "18446744073709.551615 USDC"
            );
        }

        #[test]
        fn usdc_status_is_optional_for_existing_stripe_servers() {
            let status: BillingStatus = serde_json::from_value(serde_json::json!({
                "enabled":true,"checkoutEnabled":true,"credit":0,"pendingCredit":0,"offers":[],
                "monthlyCardCreditPrice":null,"ownedMonthlyCards":[],"orders":[],
                "monthlyCard":{"required":false,"active":false,"expiresAtMs":null,"serverNowMs":1,"remainingMs":0,"canEnterGame":true}
            })).unwrap();
            assert!(status.is_valid());
            assert!(!status.sui_usdc_enabled);
            assert!(status.sui_usdc_offers.is_empty());
        }

        #[test]
        fn usdc_checkout_requires_local_origin_and_retries_the_same_order() {
            let mut panel = BillingPanel::default();
            panel.open_for("alice", 7, 1);
            panel.pending = None;
            panel.status = Some(usdc_status());
            panel.usdc_page = true;
            panel.action(BillingAction::UsdcCheckout("usdc-small".into()), 2);
            assert!(panel.pending.is_none(), "reply origin is not an allowlist");
            panel.set_sui_checkout_origin(Some("http://127.0.0.1:17830".into()));
            panel.action(BillingAction::UsdcCheckout("usdc-small".into()), 3);
            let first = panel.pending.clone().unwrap();
            assert_eq!(first.operation, BillingOperation::UsdcCheckout);
            panel.mark_sent(first.request_id);
            panel.expire_request(15_003);
            panel.action(BillingAction::Retry, 15_004);
            let retry = panel.pending.clone().unwrap();
            assert_eq!(first.purchase, retry.purchase);
            assert_ne!(first.request_id, retry.request_id);
            panel.mark_sent(retry.request_id);
            let mut result = reply(&retry);
            result.checkout_url = Some(format!(
                "http://127.0.0.1:17830/v1/billing/sui-usdc/pay#usdc_{}.{}",
                "a".repeat(64),
                "A".repeat(43)
            ));
            assert!(!panel.accept(result.clone(), "bob", Some(7), 15_005));
            assert!(!panel.accept(reply(&first), "alice", Some(7), 15_005));
            assert!(panel.accept(result, "alice", Some(7), 15_005));
            assert!(panel.take_checkout_url().is_some());
            assert!(panel.take_checkout_url().is_none());
            assert_eq!(
                panel.status.unwrap().credit,
                900,
                "opening wallet never grants credits"
            );
        }

        #[test]
        fn usdc_refresh_only_reads_pending_order_status() {
            let mut panel = BillingPanel::default();
            panel.open_for("alice", 7, 1);
            panel.pending = None;
            panel.status = Some(usdc_status());
            panel.usdc_page = true;
            panel.status.as_mut().unwrap().sui_usdc_orders = vec![SuiUsdcOrder {
                id: "usdc_order".into(),
                order_request_id: "native-original-usdc".into(),
                offer_id: "usdc-small".into(),
                credits: 100,
                amount_micros: "1000000".into(),
                network: "testnet".into(),
                state: SuiUsdcOrderState::PaidPending,
                created_at_ms: 1,
                expires_at_ms: 10000,
                transaction_digest: None,
            }];
            panel.refresh_pending_usdc(5000);
            assert_eq!(
                panel.pending.as_ref().unwrap().operation,
                BillingOperation::Status
            );
            assert!(panel.pending.as_ref().unwrap().purchase.is_none());
            panel.pending = None;
            panel.status.as_mut().unwrap().sui_usdc_orders[0].state = SuiUsdcOrderState::Applied;
            panel.refresh_pending_usdc(10000);
            assert!(panel.pending.is_none());
            assert_eq!(panel.status.as_ref().unwrap().credit, 900);
        }

        #[test]
        fn usdc_relogin_reopens_frozen_order_when_new_sales_are_disabled() {
            let mut panel = BillingPanel::default();
            panel.open_for("alice", 7, 1);
            panel.pending = None;
            panel.set_sui_checkout_origin(Some("http://127.0.0.1:17830".into()));
            let mut s = usdc_status();
            s.enabled = false;
            s.sui_usdc_enabled = false;
            s.sui_usdc_offers.clear();
            s.sui_usdc_orders = vec![SuiUsdcOrder {
                id: "usdc_order".into(),
                order_request_id: "native-original-usdc".into(),
                offer_id: "removed-offer".into(),
                credits: 100,
                amount_micros: "1000000".into(),
                network: "testnet".into(),
                state: SuiUsdcOrderState::AwaitingPayment,
                created_at_ms: 1,
                expires_at_ms: 10000,
                transaction_digest: None,
            }];
            panel.status = Some(s);
            panel.usdc_page = true;
            panel.action(BillingAction::UsdcCheckout("removed-offer".into()), 2);
            assert!(panel.pending.is_none());
            panel.action(BillingAction::UsdcReopen("usdc_order".into()), 3);
            assert_eq!(
                panel.pending.as_ref().unwrap().purchase,
                Some(BillingPurchase::UsdcCheckout {
                    offer_id: "removed-offer".into(),
                    order_request_id: "native-original-usdc".into(),
                })
            );
            panel.pending = None;
            for state in [
                SuiUsdcOrderState::Applied,
                SuiUsdcOrderState::Expired,
                SuiUsdcOrderState::Review,
            ] {
                panel.status.as_mut().unwrap().sui_usdc_orders[0].state = state;
                panel.action(BillingAction::UsdcReopen("usdc_order".into()), 4);
                assert!(panel.pending.is_none());
            }
            panel.status.as_mut().unwrap().sui_usdc_orders[0].state =
                SuiUsdcOrderState::AwaitingPayment;
            panel.set_sui_checkout_origin(Some("https://different.invalid".into()));
            panel.action(BillingAction::UsdcReopen("usdc_order".into()), 5);
            assert!(panel.pending.is_none());
        }

        #[test]
        #[ignore = "explicit offline Windows GPU evidence; no server, wallet signing or chain payment"]
        fn usdc_windows_gpu_channels_and_payment_states() {
            use crate::native_i18n::{self, Locale};
            use crate::native_shell_ui::i18n_visual_tests::{
                capture_i18n, i18n_offscreen_app_configured, i18n_text_layouts, warm_i18n_images,
            };
            use std::{fs, path::PathBuf};
            let assets = PathBuf::from(
                std::env::var_os("MIR2_I18N_VISUAL_ASSET_ROOT").expect("real native assets"),
            )
            .canonicalize()
            .unwrap();
            let output = PathBuf::from(
                std::env::var_os("MIR2_USDC_VISUAL_OUTPUT").expect("fresh evidence directory"),
            );
            assert!(output.is_absolute() && !output.starts_with(&assets));
            fs::create_dir_all(&output).unwrap();
            let report = output.join("usdc-native-gpu.json");
            fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&report)
                .unwrap();
            fs::write(
                &report,
                br#"{"passed":false,"evidenceKind":"offline-native-production-renderer"}"#,
            )
            .unwrap();
            struct RestoreLocale(Locale);
            impl Drop for RestoreLocale {
                fn drop(&mut self) {
                    native_i18n::activate(self.0);
                }
            }
            let _restore = RestoreLocale(native_i18n::locale());
            let mut panel = BillingPanel::default();
            panel.open_for("alice", 7, now_ms());
            panel.pending = None;
            panel.set_sui_checkout_origin(Some("http://127.0.0.1:17830".into()));
            let mut s = usdc_status();
            for (i, state) in [
                SuiUsdcOrderState::Prepared,
                SuiUsdcOrderState::InvoicePending,
                SuiUsdcOrderState::AwaitingPayment,
                SuiUsdcOrderState::PaymentSubmitted,
                SuiUsdcOrderState::PaidPending,
                SuiUsdcOrderState::Applied,
                SuiUsdcOrderState::Expired,
                SuiUsdcOrderState::Review,
            ]
            .into_iter()
            .enumerate()
            {
                s.sui_usdc_orders.push(SuiUsdcOrder {
                    id: format!("usdc_{}", format!("{i:x}").repeat(64)),
                    order_request_id: format!("native-original-usdc-{i}"),
                    offer_id: "usdc-small".into(),
                    credits: 100,
                    amount_micros: "1000000".into(),
                    network: "testnet".into(),
                    state,
                    created_at_ms: 1,
                    expires_at_ms: 10000,
                    transaction_digest: None,
                });
            }
            panel.status = Some(s);
            panel.last_status_at_ms = now_ms();
            let (mut app, target, _) = i18n_offscreen_app_configured(&assets, true, |app| {
                app.insert_resource(NativeShellModel {
                    screen: NativeShellScreen::CharacterSelect,
                    last_account: Some("alice".into()),
                    selected_character_index: Some(7),
                    characters: vec![crate::native_shell::CharacterSummary::new(
                        7,
                        "UsdcFixture",
                        22,
                        "Warrior",
                        "Male",
                    )],
                    billing: panel,
                    ..Default::default()
                });
            });
            let mut screenshots = Vec::new();
            let mut checks = Vec::new();
            for locale in [Locale::English, Locale::TraditionalChinese] {
                native_i18n::activate(locale);
                for (label, usdc, page) in [
                    ("stripe", false, 0),
                    ("usdc-preparing", true, 0),
                    ("usdc-confirmation", true, 1),
                    ("usdc-expired-review", true, 2),
                    ("usdc-sales-disabled", true, 0),
                ] {
                    let mut model = app.world_mut().resource_mut::<NativeShellModel>();
                    model.billing.usdc_page = usdc;
                    model.billing.usdc_order_page = page;
                    let status = model.billing.status.as_mut().unwrap();
                    status.enabled = label != "usdc-sales-disabled";
                    status.sui_usdc_enabled = label != "usdc-sales-disabled";
                    status.sui_usdc_offers = if label == "usdc-sales-disabled" {
                        vec![]
                    } else {
                        usdc_status().sui_usdc_offers
                    };
                    model.billing.pending = None;
                    model.billing.last_status_at_ms = now_ms();
                    drop(model);
                    warm_i18n_images(&mut app);
                    let path = output.join(format!("{}-{label}-1024x768.png", locale.code()));
                    capture_i18n(&mut app, &target, &path);
                    let layouts = i18n_text_layouts(&mut app);
                    let failures: Vec<_> = layouts
                        .iter()
                        .filter(|row| {
                            row["missingGlyphs"] != 0
                                || row["layoutExceedsNode"] == true
                                || row["nodeOutsideViewport"] == true
                        })
                        .collect();
                    assert!(
                        failures.is_empty(),
                        "{locale:?}/{label} layout failures: {failures:?}"
                    );
                    assert!(layouts
                        .iter()
                        .any(|row| row["text"].as_str().is_some_and(|t| t.contains("USDC"))));
                    screenshots.push(path.file_name().unwrap().to_string_lossy().to_string());
                    checks.push(serde_json::json!({"locale":locale.code(), "page":label, "layouts":layouts}));
                }
            }
            fs::write(&report, serde_json::to_vec_pretty(&serde_json::json!({"passed":true,
                "evidenceKind":"offline-native-production-renderer", "width":1024,"height":768,
                "screenshots":screenshots,"checks":checks,"missingGlyphs":0,"overflowingTextNodes":0,"outsideViewportNodes":0})).unwrap()).unwrap();
        }
        #[test]
        fn checkout_is_bound_to_current_account_character_and_request() {
            let mut p = BillingPanel::default();
            p.open_for("alice", 7, 10);
            p.pending = None;
            p.begin(
                Some(BillingPurchase::Checkout {
                    offer_id: "small".into(),
                    order_request_id: "native-stable-order".into(),
                }),
                10,
            );
            let r = p.pending.clone().unwrap();
            p.mark_sent(r.request_id);
            let mut result = reply(&r);
            result.checkout_url = Some("https://checkout.stripe.com/c/pay/test".into());
            assert!(!p.accept(result.clone(), "bob", Some(7), 20));
            assert!(!p.accept(result.clone(), "alice", Some(8), 20));
            result.request_id += 1;
            assert!(!p.accept(result.clone(), "alice", Some(7), 20));
            result.request_id = r.request_id;
            assert!(p.accept(result, "alice", Some(7), 20));
            assert!(p.take_checkout_url().is_some());
            assert!(p.take_checkout_url().is_none());
        }
        #[test]
        fn timeout_retry_preserves_order_and_ignores_late_original_url() {
            let mut p = BillingPanel::default();
            p.open_for("alice", 7, 1);
            p.pending = None;
            let purchase = BillingPurchase::Checkout {
                offer_id: "small".into(),
                order_request_id: "native-stable-order".into(),
            };
            p.begin(Some(purchase.clone()), 10);
            let old = p.pending.clone().unwrap();
            p.mark_sent(old.request_id);
            p.expire_request(15_010);
            p.action(BillingAction::Retry, 15_011);
            let retry = p.pending.clone().unwrap();
            assert_ne!(old.request_id, retry.request_id);
            assert_eq!(retry.purchase, Some(purchase));
            assert!(!p.accept(reply(&old), "alice", Some(7), 15_012));
            assert!(p.take_checkout_url().is_none());
        }
        #[test]
        fn unique_ids_remain_canonical_strings() {
            assert!(canonical_item_id("18446744073709551615"));
            for id in ["01", "0", "-1", "+1", "1.0", "18446744073709551616"] {
                assert!(!canonical_item_id(id));
            }
        }
        #[test]
        fn purchase_reply_refreshes_status_and_disabled_status_cannot_purchase() {
            let mut p = BillingPanel::default();
            p.open_for("alice", 7, 10);
            let request = p.pending.clone().unwrap();
            p.mark_sent(request.request_id);
            let mut result = reply(&request);
            result.status = Some(status(false));
            assert!(p.accept(result, "alice", Some(7), 11));
            p.action(BillingAction::Checkout("small".into()), 12);
            p.action(BillingAction::BuyMonthlyCard, 12);
            assert!(p.pending.is_none());
            p.status = Some(status(true));
            p.action(BillingAction::BuyMonthlyCard, 13);
            let request = p.pending.clone().unwrap();
            p.mark_sent(request.request_id);
            assert_eq!(request.operation, BillingOperation::BuyMonthlyCard);
            if let Some(BillingPurchase::BuyMonthlyCard { order_request_id }) = &request.purchase {
                assert!((12..=96).contains(&order_request_id.len()));
                assert!(order_request_id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b)));
            } else {
                panic!("missing stable order");
            }
            assert!(p.accept(reply(&request), "alice", Some(7), 14));
            assert_eq!(
                p.unsent_request().unwrap().operation,
                BillingOperation::Status
            );
            assert_eq!(p.unsent_request().unwrap().character_index, 7);
        }
        #[test]
        fn refresh_never_settles_an_unconfirmed_purchase() {
            let mut p = BillingPanel::default();
            p.open_for("alice", 7, 1);
            p.pending = None;
            let purchase = BillingPurchase::BuyMonthlyCard {
                order_request_id: "native-stable-order".into(),
            };
            p.begin(Some(purchase.clone()), 10);
            p.expire_request(15_010);
            p.action(BillingAction::Refresh, 15_011);
            let request = p.pending.clone().unwrap();
            p.mark_sent(request.request_id);
            let mut result = reply(&request);
            result.status = Some(status(true));
            assert!(p.accept(result, "alice", Some(7), 15_012));
            assert!(p.retry_required);
            assert_eq!(p.last_purchase, Some(purchase.clone()));
            p.action(BillingAction::BuyMonthlyCard, 15_013);
            assert!(p.pending.is_none());
            p.action(BillingAction::Refresh, 15_014);
            let request = p.pending.clone().unwrap();
            p.mark_sent(request.request_id);
            let mut result = reply(&request);
            result.error = Some("unavailable".into());
            assert!(p.accept(result, "alice", Some(7), 15_015));
            p.action(BillingAction::Retry, 15_016);
            assert_eq!(p.pending.unwrap().purchase, Some(purchase));
        }
        #[test]
        fn rejected_purchase_allows_an_intentional_new_offer() {
            let mut p = BillingPanel::default();
            p.open_for("alice", 7, 1);
            p.pending = None;
            p.status = Some(status(true));
            p.action(BillingAction::BuyMonthlyCard, 2);
            let request = p.pending.clone().unwrap();
            p.mark_sent(request.request_id);
            let mut result = reply(&request);
            result.error = Some("insufficient credits".into());
            assert!(p.accept(result, "alice", Some(7), 3));
            p.action(BillingAction::Checkout("small".into()), 4);
            assert_eq!(p.pending.unwrap().operation, BillingOperation::Checkout);
        }
        #[test]
        fn definitive_purchase_errors_are_specific_and_require_current_correlation() {
            for (error, message) in [
                ("billingInsufficientCredit", "billing.insufficientCredits"),
                ("billingInsufficientCredits", "billing.insufficientCredits"),
                ("billingInventoryFull", "billing.inventoryFull"),
                ("billingUnavailable", "billing.unavailable"),
                ("billingMonthlyCardDisabled", "billing.monthlyDisabled"),
            ] {
                let mut p = BillingPanel::default();
                p.open_for("alice", 7, 1);
                let refresh = p.pending.clone().unwrap();
                p.mark_sent(refresh.request_id);
                let mut refreshed = reply(&refresh);
                refreshed.status = Some(status(true));
                assert!(p.accept(refreshed, "alice", Some(7), 2));
                p.action(BillingAction::BuyMonthlyCard, 3);
                let request = p.pending.clone().unwrap();
                p.mark_sent(request.request_id);
                let mut result = reply(&request);
                result.error = Some(error.into());
                let before = p.clone();
                assert!(!p.accept(result.clone(), "bob", Some(7), 4));
                assert_eq!(p, before);
                assert!(!p.accept(result.clone(), "alice", Some(8), 4));
                assert_eq!(p, before);
                result.request_id += 1;
                assert!(!p.accept(result.clone(), "alice", Some(7), 4));
                assert_eq!(p, before);
                result.request_id = request.request_id;
                assert!(p.accept(result, "alice", Some(7), 4));
                assert_eq!(p.message, Some(message));
                assert!(p.pending.is_none());
                assert!(p.last_purchase.is_none());
                assert!(!p.retry_required);
                assert!(p.take_checkout_url().is_none());
                p.action(BillingAction::Retry, 5);
                assert!(p.pending.is_none());
                p.action(BillingAction::BuyMonthlyCard, 6);
                let next = p.pending.unwrap();
                assert_ne!(next.request_id, request.request_id);
                assert_ne!(next.purchase, request.purchase);
            }
        }
        #[test]
        fn panel_opening_enter_is_consumed_and_close_fences_world_input() {
            let mut shell = NativeShellModel::default();
            shell.screen = NativeShellScreen::InGame;
            shell.last_account = Some("alice".into());
            shell.active_character = Some(crate::native_shell::CharacterSummary::new(
                7, "Hero", 1, "Warrior", "Male",
            ));
            shell.billing.open_for("alice", 7, now_ms());
            let mut keys = ButtonInput::<KeyCode>::default();
            keys.press(KeyCode::Enter);
            let mut app = App::new();
            app.insert_resource(shell)
                .insert_resource(keys)
                .init_resource::<crate::crystal_ui::overlays::NativePlayerUiState>()
                .add_systems(Update, process_input);
            app.update();
            assert!(app.world().resource::<NativeShellModel>().billing.open);
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .clear();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::Escape);
            app.update();
            let ui = app
                .world()
                .resource::<crate::crystal_ui::overlays::NativePlayerUiState>();
            assert!(!app.world().resource::<NativeShellModel>().billing.open);
            assert!(ui.billing_input_consumed);
            assert!(ui.blocks_gameplay_keys_except_hero());
        }
        #[test]
        fn uncertain_server_commit_retries_the_same_order() {
            for error in [
                "billingProviderResultUnconfirmed",
                "billingResultUnconfirmed",
                "billingCommitOutcomeUnknown",
                "billingServiceUnavailable",
            ] {
                let mut p = BillingPanel::default();
                p.open_for("alice", 7, 1);
                p.pending = None;
                let purchase = BillingPurchase::BuyMonthlyCard {
                    order_request_id: "native-stable-order".into(),
                };
                p.begin(Some(purchase.clone()), 2);
                let request = p.pending.clone().unwrap();
                p.mark_sent(request.request_id);
                let mut result = reply(&request);
                result.error = Some(error.into());
                assert!(p.accept(result, "alice", Some(7), 3));
                assert_eq!(p.message, Some("billing.unconfirmed"));
                p.action(BillingAction::Retry, 4);
                assert_eq!(p.pending.unwrap().purchase, Some(purchase));
            }
        }
    }
}
#[cfg(feature = "native-ui")]
pub use ui::*;
