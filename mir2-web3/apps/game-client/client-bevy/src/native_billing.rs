//! Account billing presentation. Only correlated server replies settle requests.
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
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
}

impl BillingStatus {
    pub fn is_valid(&self) -> bool {
        self.offers.len() <= 64
            && self.owned_monthly_cards.len() <= 256
            && self.orders.len() <= 256
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
    account: String,
    character_index: Option<i32>,
    last_purchase: Option<BillingPurchase>,
    retry_required: bool,
    checkout_to_open: Option<String>,
    input_ready: bool,
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
                || reply.operation != BillingOperation::Checkout
                || reply.error.is_some()
        }) {
            return false;
        }
        self.pending = None;
        let has_status = reply.status.is_some();
        if let Some(status) = reply.status {
            self.offer_page = self
                .offer_page
                .min(status.offers.len().saturating_sub(1) / PAGE_SIZE);
            self.card_page = self
                .card_page
                .min(status.owned_monthly_cards.len().saturating_sub(1) / PAGE_SIZE);
            self.status = Some(status);
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
            BillingOperation::Checkout => {
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
        if self.retry_required && self.last_purchase.is_some() {
            actions.push(BillingAction::Retry);
        }
        if let Some(status) = &self.status {
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
    let was_open = shell.billing.open;
    let input_ready = shell.billing.input_ready;
    shell.billing.input_ready = shell.billing.open;
    // The Enter that opens this panel belongs to the monthly-card button.
    if shell.billing.open && input_ready {
        let actions = shell.billing.actions();
        if keys.just_pressed(KeyCode::Escape) {
            shell.billing.action(BillingAction::Close, now_ms());
        } else if keys.just_pressed(KeyCode::Tab) && !actions.is_empty() {
            let backwards = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
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
                    text(p, tr("billing.title"), 20.0, 14.0, 700.0, 30.0, 23.0);
                    text(p, tr("billing.instructions"), 20.0, 48.0, 700.0, 42.0, 13.0);
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
                            format!("{}{}", tr(access), expiry),
                            20.0,
                            132.0,
                            700.0,
                            50.0,
                            13.0,
                        );
                        if !s.enabled {
                            text(p, tr("billing.unavailable"), 20.0, 190.0, 700.0, 42.0, 15.0);
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
                                        tr("billing.points").replace("{0}", &o.credits.to_string())
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
                                    tr("billing.monthlyPrice").replace("{0}", &price.to_string()),
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
