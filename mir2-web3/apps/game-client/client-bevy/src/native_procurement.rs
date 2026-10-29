//! Personal procurement UI. The server owns assets; only explicit confirmation
//! signs a domain-separated, field-checked intent with an in-memory purpose key.
use crate::native_shell::{NativeShellModel, NativeShellScreen};
use bevy::prelude::*;
use ed25519_consensus::{SigningKey, VerificationKey};
use mir2_protocol::procurement::*;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};

pub const NPC_ENTRY: &str = "@procurement:v1:home";
static SEQUENCE: AtomicU64 = AtomicU64::new(1);
const TIMEOUT_MS: u64 = 15_000;
const PAGE_SIZE: usize = 5;
const SHORT_TEST_EXPIRY_MS: u64 = 120_000;
const NORMAL_EXPIRY_OPTIONS: &[u64] = &[3_600_000, 86_400_000, 604_800_000];
const TEST_EXPIRY_OPTIONS: &[u64] = &[SHORT_TEST_EXPIRY_MS, 3_600_000, 86_400_000, 604_800_000];
fn expiry_options(test_mode: bool) -> &'static [u64] {
    if test_mode {
        TEST_EXPIRY_OPTIONS
    } else {
        NORMAL_EXPIRY_OPTIONS
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Public,
    Publish,
    Mine,
    Service,
    Claims,
}
#[derive(Debug, Clone, PartialEq, Eq, Component)]
pub enum Action {
    Page(Page),
    Close,
    Refresh,
    Filter,
    Previous,
    Next,
    Price,
    ClearPrice,
    Duration,
    Conditions,
    ChooseItem,
    PublishQuote,
    DeliverQuote(String, String),
    CancelQuote(String),
    BuyQuota(String),
    Claim(String),
    Reconcile,
    Confirm,
    Back,
    Retry,
}
#[derive(Debug, Clone)]
pub struct PendingRequest {
    pub request: ProcurementRequest,
    pub sent: bool,
    pub started_at_ms: u64,
    pub unknown: bool,
}
#[derive(Resource)]
pub struct ProcurementModel {
    pub open: bool,
    pub page: Page,
    pub status: Option<ProcurementView>,
    pub quote: Option<ProcurementQuote>,
    pub pending: Option<PendingRequest>,
    pub message: String,
    pub price: String,
    pub expiry_duration_ms: u64,
    pub conditions: EquipmentConditions,
    pub filter_target: bool,
    pub list_page: usize,
    pub editing_price: bool,
    pub equipment_index: usize,
    account: String,
    character: i32,
    npc_object_id: u32,
    key: Option<SigningKey>,
    quote_action: Option<ProcurementAction>,
    quote_verified: bool,
    input_ready: bool,
}
impl Default for ProcurementModel {
    fn default() -> Self {
        Self {
            open: false,
            page: Page::Public,
            status: None,
            quote: None,
            pending: None,
            message: String::new(),
            price: "500000".into(),
            expiry_duration_ms: 86_400_000,
            conditions: EquipmentConditions::default(),
            filter_target: true,
            list_page: 0,
            editing_price: false,
            equipment_index: 0,
            account: String::new(),
            character: -1,
            npc_object_id: 0,
            key: None,
            quote_action: None,
            quote_verified: false,
            input_ready: false,
        }
    }
}
impl std::fmt::Debug for ProcurementModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcurementModel")
            .field("open", &self.open)
            .field("page", &self.page)
            .field(
                "pending",
                &self.pending.as_ref().map(|p| &p.request.request_id),
            )
            .field("has_purpose_key", &self.key.is_some())
            .finish()
    }
}
pub fn now_ms() -> u64 {
    crate::native_billing::now_ms()
}
fn hash(bytes: &[u8]) -> String {
    encode_hex(&Sha256::digest(bytes))
}
impl ProcurementModel {
    pub fn open_for(&mut self, account: &str, character: i32, npc_object_id: u32, now: u64) {
        if account.is_empty() || character < 0 || npc_object_id == 0 {
            return;
        }
        if !self.scope_matches(account, Some(character)) || self.key.is_none() {
            *self = Self::default();
            self.account = account.into();
            self.character = character;
            let mut seed = [0u8; 32];
            if rand_core::RngCore::try_fill_bytes(&mut rand_core::OsRng, &mut seed).is_err() {
                self.message = "系統隨機源不可用，無法建立求購用途授權。".into();
                return;
            }
            self.key = Some(SigningKey::from(seed));
            seed.fill(0);
        }
        self.open = true;
        self.input_ready = false;
        self.npc_object_id = npc_object_id;
        if self.pending.is_none() {
            self.begin(ProcurementAction::Status { npc_object_id }, now);
        }
    }
    pub fn scope_matches(&self, account: &str, character: Option<i32>) -> bool {
        self.account == account && Some(self.character) == character && !account.is_empty()
    }
    pub fn public_key(&self) -> Option<String> {
        self.key
            .as_ref()
            .map(|k| encode_hex(&VerificationKey::from(k).to_bytes()))
    }
    fn begin(&mut self, action: ProcurementAction, now: u64) -> bool {
        if self.pending.is_some() || self.account.is_empty() {
            return false;
        }
        if matches!(
            action,
            ProcurementAction::QuotePublish { .. }
                | ProcurementAction::QuoteDeliver { .. }
                | ProcurementAction::QuoteCancel { .. }
                | ProcurementAction::QuoteBuyQuota { .. }
        ) {
            self.quote_action = Some(action.clone());
            self.quote = None;
            self.quote_verified = false;
        }
        let key = self.public_key().unwrap_or_default();
        self.pending = Some(PendingRequest {
            request: ProcurementRequest {
                protocol_version: PROTOCOL_VERSION,
                request_id: format!(
                    "proc-{}-{now}-{}",
                    key.get(2..18).unwrap_or("none"),
                    SEQUENCE.fetch_add(1, Ordering::Relaxed)
                ),
                character_index: self.character,
                action,
            },
            sent: false,
            started_at_ms: now,
            unknown: false,
        });
        self.message = "等待服務端確認；請勿重複建立請求。".into();
        true
    }
    pub fn unsent_request(&self) -> Option<&ProcurementRequest> {
        self.pending
            .as_ref()
            .filter(|p| !p.sent)
            .map(|p| &p.request)
    }
    pub fn mark_sent(&mut self, id: &str) {
        if let Some(p) = self.pending.as_mut().filter(|p| p.request.request_id == id) {
            p.sent = true;
        }
    }
    pub fn expire(&mut self, now: u64) {
        if let Some(p) = self
            .pending
            .as_mut()
            .filter(|p| p.sent && now.saturating_sub(p.started_at_ms) >= TIMEOUT_MS)
        {
            p.unknown = true;
            self.message =
                "結果暫時未知。金幣和裝備仍可能被託管；超時不會退款。可原請求重試或稍後對賬。"
                    .into();
        }
    }
    pub fn accept(
        &mut self,
        reply: ProcurementReply,
        account: &str,
        character: Option<i32>,
        now: u64,
    ) -> bool {
        let Some(pending) = &self.pending else {
            return false;
        };
        if !pending.sent
            || !self.scope_matches(account, character)
            || reply.protocol_version != PROTOCOL_VERSION
            || reply.character_index != self.character
            || reply.request_id != pending.request.request_id
        {
            return false;
        }
        if reply.status.as_ref().is_some_and(|s| !valid_view(s)) {
            self.message = "響應內容無效，未確認新結果。".into();
            return false;
        }
        let submitted = pending.request.action.clone();
        if let Some(error) = reply.error {
            if error.contains("Unknown")
                || error.contains("Unconfirmed")
                || matches!(
                    error.as_str(),
                    "procurementResultUnknown"
                        | "procurementCommitUnknown"
                        | "procurementUnavailable"
                        | "chainResultUnknown"
                )
            {
                if let Some(p) = self.pending.as_mut() {
                    p.unknown = true;
                }
                self.message = format!("結果未確認（{error}）；不釋放資產，保留原請求重試。");
            } else {
                self.pending = None;
                self.message = format!("請求未完成：{error}");
            }
            return true;
        }
        if let Some(status) = reply.status {
            self.status = Some(status);
        }
        self.pending = None;
        if let Some(quote) = reply.quote {
            match self.validate_quote(&quote, now) {
                Ok(()) => {
                    self.quote_verified = true;
                    self.quote = Some(quote);
                    self.message = "請逐項核對報價。只有點選“明確確認”才會簽署本次操作。".into();
                }
                Err(reason) => {
                    self.quote = None;
                    self.quote_verified = false;
                    self.message = format!("報價校驗失敗：{reason}。未簽名、未確認。");
                }
            }
        } else {
            self.quote = None;
            self.quote_verified = false;
            self.message = if reply.replayed {
                "已返回原請求結果；不會重複扣費或發貨。"
            } else {
                "已收到服務端狀態。鏈上成交與遊戲到賬請分別檢視。"
            }
            .into();
            if matches!(
                submitted,
                ProcurementAction::Confirm { .. } | ProcurementAction::Claim { .. }
            ) {
                self.begin(
                    ProcurementAction::Status {
                        npc_object_id: self.npc_object_id,
                    },
                    now,
                );
            }
        }
        true
    }
    fn validate_quote(&self, q: &ProcurementQuote, now: u64) -> Result<(), &'static str> {
        if q.quote_id.is_empty()
            || q.quote_id.len() > 128
            || q.service_fee.production_billing_enabled
        {
            return Err("生產收費未授權");
        }
        if matches!(
            self.quote_action,
            Some(ProcurementAction::QuoteBuyQuota { .. })
        ) {
            if q.intent.is_some() || q.canonical_intent_hex.is_some() || q.service_fee.credit_cost!=0
                || q.service_fee.quota_source!="test" || q.quota_package_id.as_ref().is_none_or(|id|
                    !matches!(&self.quote_action,Some(ProcurementAction::QuoteBuyQuota{package_id}) if package_id==id)) {return Err("測試額度報價不符");}
            return Ok(());
        }
        let view = self.status.as_ref().ok_or("缺少當前身份和網路狀態")?;
        let i = q.intent.as_ref().ok_or("缺少可驗證授權")?;
        let bytes = i.canonical_bcs()?;
        if Some(encode_hex(&bytes)) != q.canonical_intent_hex
            || i.actor_public_key != self.public_key().ok_or("授權金鑰失效")?
            || i.network_hash != view.network_hash
            || i.package_id != view.package_id
            || i.dapp_storage_id != view.dapp_storage_id
            || i.conditions_hash != hash(&q.conditions.canonical_bcs()?)
            || i.rule_version != q.service_fee.rule_version
            || decimal(&i.valid_until_ms)? <= now
            || decimal(&i.valid_until_ms)? > now.saturating_add(900_000)
        {
            return Err("授權位元組、網路、條件或有效期不符");
        }
        let zero = encode_hex(&[0; 32]);
        match self.quote_action.as_ref().ok_or("缺少對應操作")? {
            ProcurementAction::QuotePublish {
                template_id,
                price_gold,
                expires_at_ms,
                conditions,
                ..
            } => {
                if i.operation != 1
                    || i.template_id != *template_id
                    || i.price_gold != *price_gold
                    || i.expires_at_ms != *expires_at_ms
                    || q.conditions != *conditions
                    || i.buyer_ref != view.actor_ref
                    || i.seller_ref != zero
                    || i.fill_id != zero
                    || !view.test_mode
                    || view.production_billing_enabled
                    || view.fee_rule.production_billing_enabled
                    || view.fee_rule.quota_units == 0
                    || view.fee_rule.credit_cost != 0
                    || view.fee_rule.quota_source != "test"
                    || !decimal(&view.fee_rule.rule_version).is_ok_and(|version| version > 0)
                    || q.service_fee != view.fee_rule
                {
                    return Err("釋出金額、買家或費用不符");
                }
            }
            ProcurementAction::QuoteDeliver {
                order_id, item_uid, ..
            } => {
                let order = view
                    .public_orders
                    .iter()
                    .find(|o| &o.order_id == order_id)
                    .ok_or("訂單已離開公開列表")?;
                if i.operation != 2
                    || &i.order_id != order_id
                    || q.item_uid.as_ref() != Some(item_uid)
                    || i.seller_ref != view.actor_ref
                    || i.buyer_ref != order.buyer_ref
                    || i.price_gold != order.price_gold
                    || i.expires_at_ms != order.expires_at_ms
                    || i.rule_version != order.rule_version
                    || q.conditions != order.conditions
                    || i.fill_id == zero
                    || q.service_fee.quota_units != 0
                    || q.service_fee.credit_cost != 0
                {
                    return Err("交貨裝備、雙方或條件不符");
                }
            }
            ProcurementAction::QuoteCancel { order_id, .. } => {
                let order = view
                    .my_orders
                    .iter()
                    .find(|o| &o.order_id == order_id)
                    .ok_or("找不到自己的訂單")?;
                if i.operation != 3
                    || &i.order_id != order_id
                    || i.buyer_ref != view.actor_ref
                    || i.seller_ref != zero
                    || i.fill_id != zero
                    || i.price_gold != order.price_gold
                    || i.expires_at_ms != order.expires_at_ms
                    || q.conditions != order.conditions
                    || i.rule_version != order.rule_version
                    || q.service_fee.quota_units != 0
                    || q.service_fee.credit_cost != 0
                {
                    return Err("取消範圍不符");
                }
            }
            _ => return Err("報價操作不符"),
        }
        Ok(())
    }
    pub fn action(&mut self, a: Action, now: u64) {
        if a == Action::Close {
            self.open = false;
            self.editing_price = false;
            return;
        }
        if a == Action::Retry {
            if let Some(p) = self.pending.as_mut().filter(|p| p.unknown) {
                p.sent = false;
                p.unknown = false;
                p.started_at_ms = now;
            }
            return;
        }
        if self.pending.is_some() {
            return;
        }
        match a {
            Action::Page(page) => {
                self.page = page;
                self.list_page = 0;
                self.editing_price = false;
            }
            Action::Refresh => {
                self.begin(
                    ProcurementAction::Status {
                        npc_object_id: self.npc_object_id,
                    },
                    now,
                );
            }
            Action::Filter => {
                self.filter_target = !self.filter_target;
                self.list_page = 0;
                self.begin(
                    ProcurementAction::List {
                        template_id: self.filter_target.then_some(TARGET_TEMPLATE_ID),
                    },
                    now,
                );
            }
            Action::Previous => self.list_page = self.list_page.saturating_sub(1),
            Action::Next => self.list_page = self.list_page.saturating_add(1).min(100),
            Action::Price => self.editing_price = !self.editing_price,
            Action::ClearPrice => {
                self.price.clear();
                self.editing_price = true;
            }
            Action::Duration => {
                let options = expiry_options(self.status.as_ref().is_some_and(|v| v.test_mode));
                self.expiry_duration_ms = options
                    .iter()
                    .position(|ms| *ms == self.expiry_duration_ms)
                    .map(|index| options[(index + 1) % options.len()])
                    .unwrap_or(options[0]);
            }
            Action::Conditions => {
                self.conditions.min_current_durability =
                    if self.conditions.min_current_durability == 0 {
                        1000
                    } else {
                        0
                    }
            }
            Action::ChooseItem => self.equipment_index = self.equipment_index.saturating_add(1),
            Action::Back => {
                self.quote = None;
                self.quote_verified = false;
            }
            Action::PublishQuote => {
                if !expiry_options(self.status.as_ref().is_some_and(|v| v.test_mode))
                    .contains(&self.expiry_duration_ms)
                {
                    self.expiry_duration_ms = NORMAL_EXPIRY_OPTIONS[0];
                    self.message = "短時到期僅供伺服器測試模式；請重新核對有效期。".into();
                    return;
                }
                if decimal(&self.price).is_ok_and(|p| p > 0 && p <= u32::MAX as u64) {
                    if let Some(key) = self.public_key() {
                        self.begin(
                            ProcurementAction::QuotePublish {
                                template_id: TARGET_TEMPLATE_ID,
                                price_gold: self.price.clone(),
                                expires_at_ms: now
                                    .saturating_add(self.expiry_duration_ms)
                                    .to_string(),
                                conditions: self.conditions.clone(),
                                purpose_public_key: key,
                            },
                            now,
                        );
                    }
                } else {
                    self.message = "請輸入 1–4294967295 的整數金幣價格。".into();
                }
            }
            Action::DeliverQuote(order_id, item_uid) => {
                if let Some(key) = self.public_key() {
                    self.begin(
                        ProcurementAction::QuoteDeliver {
                            order_id,
                            item_uid,
                            purpose_public_key: key,
                        },
                        now,
                    );
                }
            }
            Action::CancelQuote(order_id) => {
                if let Some(key) = self.public_key() {
                    self.begin(
                        ProcurementAction::QuoteCancel {
                            order_id,
                            purpose_public_key: key,
                        },
                        now,
                    );
                }
            }
            Action::BuyQuota(package_id) => {
                self.begin(ProcurementAction::QuoteBuyQuota { package_id }, now);
            }
            Action::Claim(delivery_id) => {
                self.begin(ProcurementAction::Claim { delivery_id }, now);
            }
            Action::Reconcile => {
                self.begin(ProcurementAction::Reconcile { order_id: None }, now);
            }
            Action::Confirm => {
                let Some(q) = self.quote.clone().filter(|_| self.quote_verified) else {
                    return;
                };
                if let Err(reason) = self.validate_quote(&q, now) {
                    self.quote_verified = false;
                    self.message = format!("報價已不可確認：{reason}");
                    return;
                }
                let signature = if let Some(i) = &q.intent {
                    let Ok(bytes) = i.canonical_bcs() else {
                        return;
                    };
                    let Some(key) = &self.key else {
                        return;
                    };
                    encode_hex(&key.sign(&Sha256::digest(bytes)).to_bytes())
                } else {
                    String::new()
                };
                self.begin(
                    ProcurementAction::Confirm {
                        quote_id: q.quote_id,
                        signature,
                    },
                    now,
                );
            }
            Action::Close | Action::Retry => {}
        }
    }
}
fn valid_view(v: &ProcurementView) -> bool {
    v.public_orders.len() <= 200
        && v.my_orders.len() <= 200
        && v.deliveries.len() <= 200
        && v.fee_history.len() <= 200
        && v.quota_packages.len() <= 32
        && decimal(&v.available_gold).is_ok()
        && decimal(&v.escrow_gold).is_ok()
        && [
            &v.actor_ref,
            &v.network_hash,
            &v.package_id,
            &v.dapp_storage_id,
        ]
        .into_iter()
        .all(|s| hex32(s).is_ok())
        && v.public_orders.iter().chain(&v.my_orders).all(|o| {
            hex32(&o.order_id).is_ok()
                && hex32(&o.buyer_ref).is_ok()
                && o.template_id == TARGET_TEMPLATE_ID
                && decimal(&o.price_gold).is_ok()
                && decimal(&o.expires_at_ms).is_ok()
                && decimal(&o.rule_version).is_ok()
                && o.chain_state.len() <= 64
                && o.delivery_state.len() <= 64
                && o.conditions.canonical_bcs().is_ok()
        })
        && v.deliveries.iter().all(|d| {
            d.delivery_id.len() <= 128
                && d.description.len() <= 1024
                && decimal(&d.item_uid).is_ok()
        })
}

pub struct NativeProcurementPlugin;
impl Plugin for NativeProcurementPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ProcurementModel>()
            .add_systems(
                Update,
                process_input
                    .after(crate::pending_operations::PendingLifecycleSet::UiReset)
                    .before(crate::crystal_ui::overlays::NativePlayerUiSet::Mutate),
            )
            .add_systems(Update, render.after(process_input));
    }
}
#[derive(Component)]
struct ProcurementRoot;
#[derive(Component)]
struct ProcurementButton(Action, bool);
fn process_input(
    mut model: ResMut<ProcurementModel>,
    shell: Option<Res<NativeShellModel>>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    buttons: Query<(&Interaction, &ProcurementButton), Changed<Interaction>>,
    mut ui: Option<ResMut<crate::crystal_ui::overlays::NativePlayerUiState>>,
    mut keyboard: MessageReader<bevy::input::keyboard::KeyboardInput>,
) {
    let was_open = model.open;
    if let Some(shell) = shell.as_deref() {
        if !model.account.is_empty()
            && (shell.screen != NativeShellScreen::InGame
                || !model.scope_matches(
                    shell.last_account.as_deref().unwrap_or_default(),
                    shell.billing_character_index(),
                ))
        {
            *model = Default::default();
        }
    }
    if model.pending.as_ref().is_some_and(|p| {
        p.sent && !p.unknown && now_ms().saturating_sub(p.started_at_ms) >= TIMEOUT_MS
    }) {
        model.expire(now_ms());
    }
    let ready = model.input_ready;
    if model.input_ready != model.open {
        model.input_ready = model.open;
    }
    if model.open && ready {
        if keys.just_pressed(KeyCode::Escape) {
            model.action(Action::Close, now_ms());
        }
        if model.editing_price && model.pending.is_none() {
            for event in keyboard.read() {
                if event.state == bevy::input::ButtonState::Pressed {
                    match &event.logical_key {
                        bevy::input::keyboard::Key::Character(text) => {
                            for ch in text.chars().filter(|c| c.is_ascii_digit()) {
                                if model.price.len() < 10 {
                                    if model.price == "0" {
                                        model.price.clear();
                                    }
                                    model.price.push(ch);
                                }
                            }
                        }
                        bevy::input::keyboard::Key::Backspace => {
                            model.price.pop();
                        }
                        _ => {}
                    }
                }
            }
        } else {
            keyboard.clear();
        }
        if mouse
            .as_deref()
            .is_some_and(|mouse| mouse.just_pressed(MouseButton::Left))
        {
            for (interaction, button) in &buttons {
                if *interaction == Interaction::Pressed && button.1 && model.open {
                    model.action(button.0.clone(), now_ms());
                    break;
                }
            }
        }
    } else {
        keyboard.clear();
    }
    if let Some(ui) = ui.as_deref_mut() {
        ui.procurement_open = model.open;
        ui.procurement_input_consumed = was_open && !model.open;
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
    p: &mut ChildSpawnerCommands,
    value: impl Into<String>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    size: f32,
) {
    let mut n = rect(x, y, w, h);
    n.overflow = Overflow::clip();
    p.spawn((
        n,
        Text::new(value.into()),
        crate::crystal_ui::typography::crystal_text_font(size),
        TextColor(Color::srgb(0.94, 0.89, 0.75)),
    ));
}
fn button(
    p: &mut ChildSpawnerCommands,
    a: Action,
    label: &str,
    x: f32,
    y: f32,
    w: f32,
    enabled: bool,
) {
    let mut n = rect(x, y, w, 27.);
    n.border = UiRect::all(Val::Px(1.));
    p.spawn((
        n,
        Button,
        bevy::ui::FocusPolicy::Block,
        ProcurementButton(a, enabled),
        BackgroundColor(if enabled {
            Color::srgb(0.20, 0.15, 0.08)
        } else {
            Color::srgb(0.08, 0.08, 0.08)
        }),
        BorderColor::all(Color::srgb(0.55, 0.42, 0.21)),
    ))
    .with_children(|b| text(b, label, 6., 4., w - 12., 22., 12.));
}
fn short(value: &str) -> String {
    if value.chars().count() > 18 {
        format!(
            "{}…{}",
            value.chars().take(10).collect::<String>(),
            value
                .chars()
                .rev()
                .take(6)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>()
        )
    } else {
        value.into()
    }
}
fn conditions_label(c: &EquipmentConditions) -> String {
    format!(
        "當前耐久≥{}；最大耐久≥{}；附加屬性 {:?}",
        c.min_current_durability, c.min_max_durability, c.min_added_stats
    )
}
fn chain_label(state: &str) -> &str {
    match state {
        "open" => "公開中",
        "filled" => "已成交",
        "cancelled" | "canceled" => "已取消",
        "expired" => "已到期",
        "creating" | "created" | "activating" | "pendingPublish" => "釋出待確認",
        "unknown" | "resultUnknown" => "結果未知",
        other => other,
    }
}
fn delivery_label(state: &str) -> &str {
    match state {
        "delivered" => "已到帳",
        "pendingClaim" => "待領取",
        "escrowed" => "金幣託管中",
        "refunded" => "已退款",
        "pending" | "delivering" => "交付處理中",
        "unknown" | "resultUnknown" => "結果未知",
        "none" => "尚未交付",
        other => other,
    }
}
fn claim_button_label(delivery: &ProcurementDelivery) -> &'static str {
    if delivery.template_id == 0 && decimal(&delivery.item_uid) == Ok(0) {
        "領取金幣"
    } else {
        "領取原始裝備"
    }
}
fn delivery_candidate(item: &crate::inventory::ItemModel) -> bool {
    item.container == 0
        && item.quantity == 1
        && item
            .tooltip_source
            .as_ref()
            .and_then(|s| s.user_item.as_ref())
            .is_some_and(|u| {
                u.item_index == TARGET_TEMPLATE_ID as i32
                    && u.count == 1
                    && u.soul_bound_id == -1
                    && u.unique_id > 0
                    && item.unique_id == Some(u.unique_id)
            })
}
fn render(
    mut commands: Commands,
    model: Res<ProcurementModel>,
    inventory: Option<Res<crate::inventory::InventoryModel>>,
    roots: Query<Entity, With<ProcurementRoot>>,
) {
    if !model.is_changed() && !inventory.as_ref().is_some_and(|i| i.is_changed()) {
        return;
    }
    for e in &roots {
        commands.entity(e).despawn();
    }
    if !model.open {
        return;
    }
    commands.spawn((ProcurementRoot,rect(0.,0.,1024.,768.),GlobalZIndex(6200),BackgroundColor(Color::srgba(0.,0.,0.,0.78)),bevy::ui::FocusPolicy::Block))
        .with_children(|layer|{let mut n=rect(80.,35.,864.,700.);n.border=UiRect::all(Val::Px(2.));
            layer.spawn((n,BackgroundColor(Color::srgb(0.055,0.04,0.02)),BorderColor::all(Color::srgb(0.65,0.49,0.2))))
            .with_children(|p|{
                text(p,"個人智慧求購 · Dubhe / Sui",18.,12.,790.,28.,22.);
                text(p,"金幣與原始裝備由遊戲伺服器可信託管、核驗和交付；鏈上不能獨立核驗背包或強迫離線伺服器交貨。",18.,47.,825.,34.,12.);
                text(p,"獨立於基礎月卡；無需另買 SUI。生產收費預設關閉，測試額度不代表現金收入。",18.,80.,825.,28.,12.);
                for (idx,(page,label)) in [(Page::Public,"公開求購"),(Page::Publish,"釋出求購"),(Page::Mine,"我的訂單"),(Page::Service,"服務與明細"),(Page::Claims,"待領取區")].into_iter().enumerate(){button(p,Action::Page(page),label,18.+idx as f32*140.,115.,132.,model.pending.is_none());}
                button(p,Action::Close,"關閉",750.,115.,95.,true);
                if let Some(q)=&model.quote {render_quote(p,&model,q);}else{render_page(p,&model,inventory.as_deref());}
                text(p,&model.message,18.,592.,825.,53.,12.);
                button(p,Action::Refresh,"重新整理狀態",18.,655.,120.,model.pending.is_none());
                button(p,Action::Reconcile,"主動對賬",150.,655.,120.,model.pending.is_none());
                button(p,Action::Retry,"原請求重試",282.,655.,140.,model.pending.as_ref().is_some_and(|p|p.unknown));
                text(p,"關閉視窗不會撤銷訂單或解鎖資產。結果未知時不會因超時退款。",438.,657.,400.,35.,11.);
            });});
}
fn render_quote(p: &mut ChildSpawnerCommands, m: &ProcurementModel, q: &ProcurementQuote) {
    text(
        p,
        format!("二次確認：{} / 報價 {}", q.operation, short(&q.quote_id)),
        18.,
        155.,
        825.,
        25.,
        16.,
    );
    let mut rows = Vec::new();
    if let Some(i) = &q.intent {
        rows.extend([
            format!(
                "目標：井中月 / DragonSword · 模板 {} · 數量 {} · 金幣 {}",
                i.template_id, i.quantity, i.price_gold
            ),
            conditions_label(&q.conditions),
            format!("訂單 {}", i.order_id),
            format!("成交 {}", i.fill_id),
            format!("買家 {}", i.buyer_ref),
            format!("賣家 {}", i.seller_ref),
            format!("網路 {}", i.network_hash),
            format!("合約 {}", i.package_id),
            format!("Dapp {}", i.dapp_storage_id),
            format!(
                "訂單到期 {} · 授權到期 {} · 規則 {}",
                i.expires_at_ms, i.valid_until_ms, i.rule_version
            ),
        ]);
    }
    if let Some(uid) = &q.item_uid {
        rows.push(format!(
            "交付原始裝備 UID {uid}：{}",
            q.item_description
                .as_deref()
                .unwrap_or("以服務端核驗例項為準")
        ));
    }
    if q.operation == "publish"
        && m.expiry_duration_ms == SHORT_TEST_EXPIRY_MS
        && m.status.as_ref().is_some_and(|v| v.test_mode)
    {
        rows.push("短時測試訂單：120 秒後到期；僅供測試，不是正式有效期選項。".into());
    }
    rows.push(format!(
        "服務：{} 次，來源 {}，點券 {}，規則 {}",
        q.service_fee.quota_units,
        q.service_fee.quota_source,
        q.service_fee.credit_cost,
        q.service_fee.rule_version
    ));
    rows.push(format!(
        "主動取消退釋出費：{}；到期未成交退釋出費：{}；額度到期：{}",
        q.service_fee.refund_on_cancel,
        q.service_fee.refund_on_expiry,
        q.service_fee
            .quota_valid_until_ms
            .as_deref()
            .unwrap_or("按當前規則")
    ));
    rows.push(
        "釋出費覆蓋後續成交、取消、到期與技術重試；賣家交貨不另收費；本版無金幣成交稅。".into(),
    );
    for (idx, row) in rows.iter().enumerate() {
        text(p, row, 18., 190. + idx as f32 * 22., 825., 22., 11.);
    }
    text(p,"本視窗使用記憶體短期求購用途金鑰。明確確認後只簽署以上操作；登入或繫結錢包不等於任意交易授權。",18.,507.,825.,40.,12.);
    button(
        p,
        Action::Confirm,
        "明確確認",
        638.,
        550.,
        190.,
        m.quote_verified && m.pending.is_none(),
    );
    button(
        p,
        Action::Back,
        "返回，不簽名",
        18.,
        550.,
        170.,
        m.pending.is_none(),
    );
}
fn render_page(
    p: &mut ChildSpawnerCommands,
    m: &ProcurementModel,
    inventory: Option<&crate::inventory::InventoryModel>,
) {
    let ready = m.pending.is_none();
    let Some(v) = &m.status else {
        text(
            p,
            "先通過當前 NPC 獲取身份、託管、服務規則與訂單狀態。",
            18.,
            160.,
            825.,
            60.,
            14.,
        );
        return;
    };
    text(
        p,
        format!(
            "可用金幣 {} / 託管金幣 {} · 測試額度 {} / 付費額度 {} / 預留 {}",
            v.available_gold, v.escrow_gold, v.test_quota, v.paid_quota, v.reserved_quota
        ),
        18.,
        155.,
        825.,
        30.,
        13.,
    );
    match m.page {
        Page::Publish => {
            text(
                p,
                "目標：井中月 / DragonSword（真實模板 245）· 單件裝備、單次完整成交",
                18.,
                200.,
                825.,
                30.,
                15.,
            );
            text(
                p,
                format!(
                    "求購金幣：{}{}",
                    m.price,
                    if m.editing_price {
                        " | 鍵入數字或退格修改"
                    } else {
                        ""
                    }
                ),
                18.,
                245.,
                610.,
                30.,
                16.,
            );
            button(p, Action::Price, "修改金額", 655., 242., 150., ready);
            button(p, Action::ClearPrice, "清空", 655., 276., 150., ready);
            text(
                p,
                if m.expiry_duration_ms == SHORT_TEST_EXPIRY_MS {
                    "有效期：120 秒（僅供測試）".into()
                } else {
                    format!("有效期：{} 小時", m.expiry_duration_ms / 3_600_000)
                },
                18.,
                294.,
                600.,
                30.,
                14.,
            );
            button(p, Action::Duration, "切換有效期", 655., 310., 150., ready);
            text(
                p,
                conditions_label(&m.conditions),
                18.,
                341.,
                620.,
                60.,
                13.,
            );
            button(p, Action::Conditions, "耐久條件", 655., 350., 150., ready);
            text(p,format!("服務規則 {}：成功公開後消費 {} 次釋出額度，失敗不消費。當前使用明確標記的測試額度。",v.fee_rule.rule_version,v.fee_rule.quota_units),18.,420.,825.,50.,13.);
            text(
                p,
                "下一步僅請求報價；需核對金幣託管、到期、服務費和授權內容後再次明確確認。",
                18.,
                480.,
                825.,
                40.,
                13.,
            );
            button(
                p,
                Action::PublishQuote,
                "獲取釋出報價",
                18.,
                544.,
                200.,
                ready && v.enabled && !v.production_billing_enabled,
            );
        }
        Page::Public => {
            button(
                p,
                Action::Filter,
                if m.filter_target {
                    "篩選：模板245"
                } else {
                    "篩選：全部"
                },
                18.,
                194.,
                180.,
                ready,
            );
            let orders: Vec<_> = v
                .public_orders
                .iter()
                .filter(|o| !m.filter_target || o.template_id == TARGET_TEMPLATE_ID)
                .collect();
            let candidates: Vec<_> = inventory
                .map(|v| {
                    v.items
                        .iter()
                        .filter(|item| delivery_candidate(item))
                        .collect()
                })
                .unwrap_or_default();
            let candidates: Vec<&crate::inventory::ItemModel> = candidates;
            let selected_item = candidates.get(m.equipment_index % candidates.len().max(1));
            button(
                p,
                Action::ChooseItem,
                &format!(
                    "切換裝備 UID {}",
                    selected_item
                        .and_then(|i| i.unique_id)
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "無".into())
                ),
                214.,
                194.,
                250.,
                ready && !candidates.is_empty(),
            );
            for (idx, o) in orders
                .iter()
                .skip(m.list_page * PAGE_SIZE)
                .take(PAGE_SIZE)
                .enumerate()
            {
                let y = 233. + idx as f32 * 65.;
                text(
                    p,
                    format!(
                        "{} · 井中月245 · {}金幣 · 到期 {}",
                        short(&o.order_id),
                        o.price_gold,
                        o.expires_at_ms
                    ),
                    18.,
                    y,
                    610.,
                    24.,
                    13.,
                );
                text(
                    p,
                    format!(
                        "鏈上 {} / 遊戲 {} · {}",
                        chain_label(&o.chain_state),
                        delivery_label(&o.delivery_state),
                        conditions_label(&o.conditions)
                    ),
                    18.,
                    y + 24.,
                    610.,
                    32.,
                    11.,
                );
                if let Some(item) = selected_item {
                    let uid = item.unique_id.unwrap_or(0).to_string();
                    button(
                        p,
                        Action::DeliverQuote(o.order_id.clone(), uid.clone()),
                        &format!("交貨 UID {uid}"),
                        638.,
                        y,
                        205.,
                        ready
                            && o.buyer_ref != v.actor_ref
                            && o.chain_state == "open"
                            && uid != "0",
                    );
                } else {
                    text(p, "背包無可選單件未繫結裝備", 638., y, 205., 40., 12.);
                }
            }
            text(
                p,
                "交貨報價將展示原始 UID、強化、耐久和附加屬性；以服務端所有權與鎖定核驗為準。",
                18.,
                565.,
                825.,
                25.,
                11.,
            );
            button(p, Action::Previous, "上一頁", 478., 194., 100., ready);
            button(p, Action::Next, "下一頁", 590., 194., 100., ready);
        }
        Page::Mine => {
            for (idx, o) in v
                .my_orders
                .iter()
                .skip(m.list_page * PAGE_SIZE)
                .take(PAGE_SIZE)
                .enumerate()
            {
                let y = 205. + idx as f32 * 68.;
                text(
                    p,
                    format!(
                        "{} · {}金幣 · 鏈上 {}",
                        short(&o.order_id),
                        o.price_gold,
                        chain_label(&o.chain_state)
                    ),
                    18.,
                    y,
                    610.,
                    25.,
                    13.,
                );
                text(
                    p,
                    format!(
                        "遊戲到賬：{} · 成交 {}",
                        delivery_label(&o.delivery_state),
                        o.fill_id
                            .as_deref()
                            .map(short)
                            .unwrap_or_else(|| "無".into())
                    ),
                    18.,
                    y + 26.,
                    610.,
                    25.,
                    12.,
                );
                button(
                    p,
                    Action::CancelQuote(o.order_id.clone()),
                    "申請取消報價",
                    652.,
                    y,
                    185.,
                    ready && o.chain_state == "open",
                );
            }
            button(p, Action::Previous, "上一頁", 478., 550., 100., ready);
            button(p, Action::Next, "下一頁", 590., 550., 100., ready);
        }
        Page::Service => {
            text(p,"基礎月卡不含無限求購。釋出服務按次；點券是服務收費憑證，官方獨立 Gas 預算承擔鏈上成本。",18.,200.,825.,42.,13.);
            text(
                p,
                format!(
                    "測試模式 {} / 生產收費 {} · 規則 {} · 取消退款 {} / 到期退款 {}",
                    v.test_mode,
                    v.production_billing_enabled,
                    v.fee_rule.rule_version,
                    v.fee_rule.refund_on_cancel,
                    v.fee_rule.refund_on_expiry
                ),
                18.,
                245.,
                825.,
                35.,
                13.,
            );
            for (idx, pack) in v.quota_packages.iter().take(2).enumerate() {
                let y = 290. + idx as f32 * 38.;
                text(
                    p,
                    format!(
                        "{}：{} 次 / {} 點券 / 來源 {}",
                        pack.label, pack.units, pack.credit_cost, pack.source
                    ),
                    18.,
                    y,
                    610.,
                    30.,
                    13.,
                );
                button(
                    p,
                    Action::BuyQuota(pack.id.clone()),
                    "獲取套餐確認",
                    655.,
                    y,
                    170.,
                    ready
                        && pack.enabled
                        && pack.source == "test"
                        && pack.credit_cost == 0
                        && !v.production_billing_enabled,
                );
            }
            text(p,"以下為持久化收費明細（測試/充值/贈送來源分開）；參考美元價格尚未批准，不承諾盈利。",18.,380.,825.,32.,12.);
            for (idx, entry) in v.fee_history.iter().rev().take(5).enumerate() {
                text(
                    p,
                    format!(
                        "{} · {} / {}次 / {}點券 / {} / 規則{}",
                        short(&entry.order_id),
                        entry.source,
                        entry.units,
                        entry.credit_cost,
                        entry.state,
                        entry.rule_version
                    ),
                    18.,
                    420. + idx as f32 * 28.,
                    825.,
                    27.,
                    12.,
                );
            }
        }
        Page::Claims => {
            text(
                p,
                "背包滿或離線時保留原始例項。領取只轉移同一 UID，不按模板重新生成裝備。",
                18.,
                200.,
                825.,
                45.,
                13.,
            );
            for (idx, d) in v
                .deliveries
                .iter()
                .skip(m.list_page * PAGE_SIZE)
                .take(PAGE_SIZE)
                .enumerate()
            {
                let y = 256. + idx as f32 * 57.;
                text(
                    p,
                    format!(
                        "UID {} · 模板 {} · {} · {}",
                        d.item_uid,
                        d.template_id,
                        d.description,
                        delivery_label(&d.state)
                    ),
                    18.,
                    y,
                    615.,
                    43.,
                    12.,
                );
                button(
                    p,
                    Action::Claim(d.delivery_id.clone()),
                    claim_button_label(d),
                    655.,
                    y,
                    170.,
                    ready && d.state == "pendingClaim",
                );
            }
            button(p, Action::Previous, "上一頁", 478., 550., 100., ready);
            button(p, Action::Next, "下一頁", 590., 550., 100., ready);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "explicit offline Windows GPU evidence of the real procurement renderer; no socket or chain transaction"]
    fn procurement_windows_gpu_five_pages_and_explicit_confirmations() {
        use crate::native_shell_ui::i18n_visual_tests::{
            capture_i18n, i18n_offscreen_app_configured, i18n_step, i18n_text_layouts,
            warm_i18n_images,
        };
        use std::{fs, path::PathBuf};
        let assets = PathBuf::from(
            std::env::var_os("MIR2_I18N_VISUAL_ASSET_ROOT").expect("real native assets"),
        )
        .canonicalize()
        .unwrap();
        let output = PathBuf::from(
            std::env::var_os("MIR2_PROCUREMENT_VISUAL_OUTPUT").expect("fresh evidence directory"),
        );
        assert!(output.is_absolute() && !output.starts_with(&assets));
        fs::create_dir_all(&output).unwrap();
        let report = output.join("procurement-native-gpu.json");
        fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&report)
            .unwrap();
        fs::write(&report, br#"{"passed":false,"complete":false,"evidenceKind":"offline-native-production-renderer"}"#).unwrap();
        struct RestoreLocale(crate::native_i18n::Locale);
        impl Drop for RestoreLocale {
            fn drop(&mut self) {
                crate::native_i18n::activate(self.0);
            }
        }
        let _restore = RestoreLocale(crate::native_i18n::locale());
        crate::native_i18n::activate(crate::native_i18n::Locale::TraditionalChinese);
        let short_expiry_only =
            std::env::var_os("MIR2_PROCUREMENT_VISUAL_SHORT_EXPIRY_ONLY").is_some();
        let (mut model, mut quote, _) = publish_quote_fixture();
        let now = now_ms();
        let duration = if short_expiry_only {
            SHORT_TEST_EXPIRY_MS
        } else {
            86_400_000
        };
        model.expiry_duration_ms = duration;
        let intent = quote.intent.as_mut().unwrap();
        intent.valid_until_ms = now.saturating_add(duration.min(600_000)).to_string();
        intent.expires_at_ms = now.saturating_add(duration).to_string();
        quote.canonical_intent_hex = Some(encode_hex(&intent.canonical_bcs().unwrap()));
        if let Some(ProcurementAction::QuotePublish { expires_at_ms, .. }) = &mut model.quote_action
        {
            *expires_at_ms = intent.expires_at_ms.clone();
        }
        let order = ProcurementOrder {
            order_id: intent.order_id.clone(),
            buyer_ref: intent.buyer_ref.clone(),
            seller_ref: None,
            template_id: 245,
            price_gold: "500000".into(),
            expires_at_ms: intent.expires_at_ms.clone(),
            rule_version: "1".into(),
            conditions: EquipmentConditions::default(),
            chain_state: "open".into(),
            delivery_state: "escrowed".into(),
            fill_id: None,
            chain_transaction: None,
        };
        let mut public_order = order.clone();
        public_order.buyer_ref = encode_hex(&[3; 32]);
        model.status.as_mut().unwrap().enabled = true;
        model.status.as_mut().unwrap().test_mode = true;
        model.status.as_mut().unwrap().available_gold = "1000000".into();
        model.status.as_mut().unwrap().escrow_gold = "500000".into();
        model.status.as_mut().unwrap().test_quota = 10;
        model.status.as_mut().unwrap().fee_rule = quote.service_fee.clone();
        model.status.as_mut().unwrap().public_orders = vec![public_order];
        model.status.as_mut().unwrap().my_orders = vec![order];
        model.status.as_mut().unwrap().quota_packages = vec![QuotaPackage {
            id: "test-10".into(),
            label: "測試釋出額度包".into(),
            units: 10,
            credit_cost: 0,
            source: "test".into(),
            enabled: true,
        }];
        model.status.as_mut().unwrap().fee_history = vec![ServiceFeeEntry {
            order_id: intent.order_id.clone(),
            rule_version: "1".into(),
            source: "test".into(),
            units: 1,
            state: "consumed".into(),
            credit_cost: 0,
        }];
        model.status.as_mut().unwrap().deliveries = vec![ProcurementDelivery {
            delivery_id: "delivery-fixture".into(),
            order_id: intent.order_id.clone(),
            item_uid: "777".into(),
            template_id: 245,
            description: "井中月 強化+3 耐久28000/34000 附加攻擊+2".into(),
            state: "pendingClaim".into(),
        }];
        model.message = "離線原生渲染測試資料；未連線伺服器、未產生鏈上交易。".into();
        let mut inventory = crate::inventory::InventoryModel::default();
        inventory.items.push(crate::inventory::ItemModel {
            unique_id: Some(777),
            quantity: 1,
            name: "井中月".into(),
            container: 0,
            tooltip_source: Some(crate::inventory::CrystalItemTooltipSourceModel {
                user_item: Some(crate::inventory::CrystalUserItemModel {
                    unique_id: 777,
                    item_index: 245,
                    count: 1,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        });
        let (mut app, target, _) = i18n_offscreen_app_configured(&assets, false, |app| {
            app.insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                last_account: Some("alice".into()),
                active_character: Some(crate::native_shell::CharacterSummary::new(
                    0,
                    "ProcurementFixture",
                    22,
                    "Warrior",
                    "Male",
                )),
                ..Default::default()
            })
            .insert_resource(model)
            .insert_resource(inventory)
            .init_resource::<crate::crystal_ui::overlays::NativePlayerUiState>()
            .add_plugins(NativeProcurementPlugin);
        });
        let mut screenshots = Vec::new();
        let mut layout_checks = Vec::new();
        for (page, label) in [
            (Page::Public, "public"),
            (Page::Publish, "publish"),
            (Page::Mine, "mine"),
            (Page::Service, "service"),
            (Page::Claims, "claims"),
        ]
        .into_iter()
        .filter(|(page, _)| !short_expiry_only || *page == Page::Publish)
        {
            app.world_mut().resource_mut::<ProcurementModel>().page = page;
            warm_i18n_images(&mut app);
            let path = output.join(format!("procurement-{label}-1024x768.png"));
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
                "{label} glyph/layout failures: {failures:?}"
            );
            if page == Page::Public {
                assert!(
                    layouts
                        .iter()
                        .any(|row| row["text"].as_str().is_some_and(|t| t.contains("UID 777"))),
                    "real UID must be selectable"
                );
            }
            if short_expiry_only {
                assert!(layouts.iter().any(|row| row["text"]
                    .as_str()
                    .is_some_and(|t| t.contains("120 秒（僅供測試）"))));
            }
            layout_checks.push(serde_json::json!({"page":label,"layouts":layouts}));
            assert!(
                app.world()
                    .resource::<crate::crystal_ui::overlays::NativePlayerUiState>()
                    .procurement_open
            );
            screenshots.push(path.file_name().unwrap().to_string_lossy().to_string());
        }
        {
            let mut m = app.world_mut().resource_mut::<ProcurementModel>();
            assert!(m.validate_quote(&quote, now_ms()).is_ok());
            m.quote = Some(quote);
            m.quote_verified = true;
        }
        for _ in 0..5 {
            i18n_step(&mut app);
        }
        let path = output.join("procurement-publish-confirm-1024x768.png");
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
            "publish confirmation glyph/layout failures: {failures:?}"
        );
        layout_checks.push(serde_json::json!({"page":"publish-confirm","layouts":layouts}));
        screenshots.push(path.file_name().unwrap().to_string_lossy().to_string());
        let labels: Vec<String> = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|t| t.0.clone())
            .collect();
        assert!(labels.iter().any(|t| t == "明確確認"));
        assert!(labels.iter().any(|t| t.contains("500000")));
        if short_expiry_only {
            assert!(labels
                .iter()
                .any(|t| t.contains("短時測試訂單：120 秒後到期")));
        } else {
            {
                let mut m = app.world_mut().resource_mut::<ProcurementModel>();
                let order = m.status.as_ref().unwrap().public_orders[0].clone();
                let actor = m.status.as_ref().unwrap().actor_ref.clone();
                let mut delivery = m.quote.clone().unwrap();
                delivery.quote_id = "delivery-quote-fixture".into();
                delivery.operation = "deliver".into();
                delivery.item_uid = Some("777".into());
                delivery.item_description =
                    Some("強化+3 耐久28000/34000 附加攻擊+2（原始例項）".into());
                delivery.service_fee.quota_units = 0;
                delivery.service_fee.quota_source = "none".into();
                let intent = delivery.intent.as_mut().unwrap();
                intent.operation = 2;
                intent.buyer_ref = order.buyer_ref;
                intent.seller_ref = actor;
                intent.fill_id = encode_hex(&[8; 32]);
                delivery.canonical_intent_hex = Some(encode_hex(&intent.canonical_bcs().unwrap()));
                m.quote_action = Some(ProcurementAction::QuoteDeliver {
                    order_id: order.order_id,
                    item_uid: "777".into(),
                    purpose_public_key: m.public_key().unwrap(),
                });
                assert!(m.validate_quote(&delivery, now_ms()).is_ok());
                m.quote = Some(delivery);
                m.quote_verified = true;
            }
            for _ in 0..5 {
                i18n_step(&mut app);
            }
            let path = output.join("procurement-delivery-confirm-1024x768.png");
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
                "delivery confirmation glyph/layout failures: {failures:?}"
            );
            layout_checks.push(serde_json::json!({"page":"delivery-confirm","layouts":layouts}));
            screenshots.push(path.file_name().unwrap().to_string_lossy().to_string());
        }
        assert!(
            app.world().resource::<ProcurementModel>().pending.is_none(),
            "rendering never signs or dispatches"
        );
        fs::write(&report,serde_json::to_vec_pretty(&serde_json::json!({"passed":true,"complete":true,
            "evidenceKind":"offline-native-production-renderer","width":1024,"height":768,"screenshots":screenshots,
            "usesProductionRenderer":true,"serverConnected":false,"testnetTransactions":0,"humanAccepted":false,
            "scenario":if short_expiry_only {"short-expiry-publish-and-confirmation"}else{"five-pages-and-confirmations"},
            "modelAndSignatureTestsAreSeparate":true,"layoutChecks":layout_checks,
            "missingGlyphs":0,"overflowingTextNodes":0,"outsideViewportNodes":0})).unwrap()).unwrap();
    }
    #[test]
    fn procurement_delivery_candidates_use_original_unbound_sentinel_and_same_actual_uid() {
        let mut item = crate::inventory::ItemModel {
            unique_id: Some(77),
            quantity: 1,
            container: 0,
            tooltip_source: Some(crate::inventory::CrystalItemTooltipSourceModel {
                user_item: Some(crate::inventory::CrystalUserItemModel {
                    unique_id: 77,
                    item_index: 245,
                    count: 1,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(delivery_candidate(&item));
        item.tooltip_source
            .as_mut()
            .unwrap()
            .user_item
            .as_mut()
            .unwrap()
            .soul_bound_id = 0;
        assert!(
            !delivery_candidate(&item),
            "character zero is bound, not unbound"
        );
        item.tooltip_source
            .as_mut()
            .unwrap()
            .user_item
            .as_mut()
            .unwrap()
            .soul_bound_id = -1;
        item.unique_id = Some(78);
        assert!(!delivery_candidate(&item));
        item.unique_id = Some(77);
        item.container = 2;
        assert!(!delivery_candidate(&item));
        item.container = 0;
        item.quantity = 2;
        assert!(!delivery_candidate(&item));
    }
    fn fixed_fixture() -> serde_json::Value {
        serde_json::from_str(include_str!(
            "../../../../onchain/procurement/fixtures/intent-v1.json"
        ))
        .unwrap()
    }
    #[test]
    fn procurement_cross_language_bcs_hash_and_actor_signature_match_public_fixture() {
        let fixture = fixed_fixture();
        let intent: IntentV1 = serde_json::from_value(fixture["intent"].clone()).unwrap();
        let bytes = intent.canonical_bcs().unwrap();
        assert_eq!(
            encode_hex(&bytes),
            fixture["canonical_intent_hex"].as_str().unwrap()
        );
        assert_eq!(hash(&bytes), fixture["intent_sha256"].as_str().unwrap());
        // Public, deterministic test seed only; never used by production UI.
        let key = SigningKey::from([1u8; 32]);
        assert_eq!(
            encode_hex(&VerificationKey::from(&key).to_bytes()),
            intent.actor_public_key
        );
        assert_eq!(
            encode_hex(&key.sign(&Sha256::digest(bytes)).to_bytes()),
            fixture["actor_signature"].as_str().unwrap()
        );
    }
    fn publish_quote_fixture() -> (ProcurementModel, ProcurementQuote, u64) {
        let fixture = fixed_fixture();
        let mut intent: IntentV1 = serde_json::from_value(fixture["intent"].clone()).unwrap();
        let conditions = EquipmentConditions::default();
        intent.conditions_hash = hash(&conditions.canonical_bcs().unwrap());
        let mut model = ProcurementModel::default();
        model.open_for("alice", 0, 12, 1000);
        model.key = Some(SigningKey::from([1u8; 32]));
        model.pending = None;
        model.status = Some(ProcurementView {
            actor_ref: intent.buyer_ref.clone(),
            network_hash: intent.network_hash.clone(),
            package_id: intent.package_id.clone(),
            dapp_storage_id: intent.dapp_storage_id.clone(),
            ..Default::default()
        });
        model.quote_action = Some(ProcurementAction::QuotePublish {
            template_id: TARGET_TEMPLATE_ID,
            price_gold: intent.price_gold.clone(),
            expires_at_ms: intent.expires_at_ms.clone(),
            conditions: conditions.clone(),
            purpose_public_key: intent.actor_public_key.clone(),
        });
        let quote = ProcurementQuote {
            quote_id: "quote-fixture".into(),
            operation: "publish".into(),
            canonical_intent_hex: Some(encode_hex(&intent.canonical_bcs().unwrap())),
            intent: Some(intent),
            conditions,
            item_uid: None,
            item_description: None,
            quota_package_id: None,
            service_fee: ServiceFeeQuote {
                rule_version: "1".into(),
                quota_units: 1,
                quota_source: "test".into(),
                ..Default::default()
            },
        };
        model.status.as_mut().unwrap().test_mode = true;
        model.status.as_mut().unwrap().fee_rule = quote.service_fee.clone();
        (model, quote, 1_999_999_980_000)
    }
    #[test]
    fn procurement_publication_accepts_the_previously_displayed_non_one_fee_rule() {
        let (mut model, mut quote, now) = publish_quote_fixture();
        quote.service_fee.rule_version = "7".into();
        quote.service_fee.quota_units = 3;
        quote.service_fee.refund_on_cancel = true;
        quote.service_fee.quota_valid_until_ms = Some((now + 86_400_000).to_string());
        quote.intent.as_mut().unwrap().rule_version = "7".into();
        quote.canonical_intent_hex = Some(encode_hex(
            &quote.intent.as_ref().unwrap().canonical_bcs().unwrap(),
        ));
        model.status.as_mut().unwrap().fee_rule = quote.service_fee.clone();
        assert!(model.validate_quote(&quote, now).is_ok());
        model.quote = Some(quote);
        model.quote_verified = true;
        model.action(Action::Confirm, now);
        assert!(
            matches!(model.unsent_request().map(|r| &r.action), Some(ProcurementAction::Confirm { signature, .. }) if decode_hex(signature).unwrap().len() == 64)
        );
    }
    #[test]
    fn procurement_publication_rejects_changed_fee_fields_and_unknown_sources() {
        let (mut model, quote, now) = publish_quote_fixture();
        for change in 0..8 {
            let mut changed = quote.clone();
            match change {
                0 => changed.service_fee.quota_units += 1,
                1 => changed.service_fee.rule_version = "2".into(),
                2 => changed.service_fee.refund_on_cancel = true,
                3 => changed.service_fee.refund_on_expiry = true,
                4 => changed.service_fee.quota_valid_until_ms = Some((now + 1000).to_string()),
                5 => {
                    changed.service_fee.quota_source =
                        "test (explicit) / paid provenance required".into()
                }
                6 => changed.service_fee.credit_cost = 1,
                _ => changed.service_fee.production_billing_enabled = true,
            }
            model.quote = Some(changed.clone());
            model.quote_verified = true;
            assert!(
                model.validate_quote(&changed, now).is_err(),
                "fee change {change}"
            );
            model.action(Action::Confirm, now);
            assert!(
                model.unsent_request().is_none(),
                "must not sign fee change {change}"
            );
        }
        let mut unknown = quote.clone();
        unknown.service_fee.quota_source = "gift".into();
        model.status.as_mut().unwrap().fee_rule = unknown.service_fee.clone();
        assert!(
            model.validate_quote(&unknown, now).is_err(),
            "matching an unknown source in both payloads must still fail closed"
        );
        model.status.as_mut().unwrap().fee_rule = quote.service_fee.clone();
        model.status.as_mut().unwrap().test_mode = false;
        assert!(
            model.validate_quote(&quote, now).is_err(),
            "test fees require server test mode"
        );
    }
    #[test]
    fn procurement_short_expiry_is_only_offered_and_requested_in_test_mode() {
        let (mut model, _, now) = publish_quote_fixture();
        model.status.as_mut().unwrap().test_mode = false;
        for _ in 0..6 {
            model.action(Action::Duration, now);
            assert!(NORMAL_EXPIRY_OPTIONS.contains(&model.expiry_duration_ms));
        }
        model.status.as_mut().unwrap().test_mode = true;
        model.expiry_duration_ms = 86_400_000;
        model.action(Action::Duration, now);
        model.action(Action::Duration, now);
        assert_eq!(model.expiry_duration_ms, SHORT_TEST_EXPIRY_MS);
        model.action(Action::PublishQuote, now);
        assert!(matches!(&model.unsent_request().unwrap().action,
            ProcurementAction::QuotePublish { expires_at_ms, .. }
            if expires_at_ms == &(now + SHORT_TEST_EXPIRY_MS).to_string()));
        model.pending = None;
        model.status.as_mut().unwrap().test_mode = false;
        model.action(Action::PublishQuote, now);
        assert!(
            model.unsent_request().is_none(),
            "test mode cannot survive a server configuration change"
        );
        assert_eq!(model.expiry_duration_ms, NORMAL_EXPIRY_OPTIONS[0]);
    }
    #[test]
    fn procurement_short_expiry_confirmation_still_binds_canonical_deadline() {
        let (mut model, mut quote, now) = publish_quote_fixture();
        model.status.as_mut().unwrap().test_mode = true;
        model.expiry_duration_ms = SHORT_TEST_EXPIRY_MS;
        model.action(Action::PublishQuote, now);
        let expires = (now + SHORT_TEST_EXPIRY_MS).to_string();
        let intent = quote.intent.as_mut().unwrap();
        intent.expires_at_ms = expires.clone();
        intent.valid_until_ms = (now + 60_000).to_string();
        quote.canonical_intent_hex = Some(encode_hex(&intent.canonical_bcs().unwrap()));
        model.pending = None;
        assert!(model.validate_quote(&quote, now).is_ok());
        let mut altered = quote.clone();
        altered.intent.as_mut().unwrap().expires_at_ms =
            (now + SHORT_TEST_EXPIRY_MS + 1).to_string();
        altered.canonical_intent_hex = Some(encode_hex(
            &altered.intent.as_ref().unwrap().canonical_bcs().unwrap(),
        ));
        assert!(model.validate_quote(&altered, now).is_err());
        model.quote = Some(quote.clone());
        model.quote_verified = true;
        model.action(Action::Confirm, now);
        let ProcurementAction::Confirm { signature, .. } = &model.unsent_request().unwrap().action
        else {
            panic!("explicit signed confirmation");
        };
        let signature =
            ed25519_consensus::Signature::try_from(decode_hex(signature).unwrap().as_slice())
                .unwrap();
        let key = VerificationKey::try_from(
            hex32(&quote.intent.as_ref().unwrap().actor_public_key).unwrap(),
        )
        .unwrap();
        key.verify(
            &signature,
            &Sha256::digest(quote.intent.as_ref().unwrap().canonical_bcs().unwrap()),
        )
        .unwrap();
        assert_eq!(quote.intent.as_ref().unwrap().expires_at_ms, expires);
    }
    #[test]
    fn procurement_claim_label_distinguishes_gold_from_original_equipment() {
        let mut delivery = ProcurementDelivery {
            delivery_id: "proceeds:fill".into(),
            order_id: encode_hex(&[1; 32]),
            item_uid: "0".into(),
            template_id: 0,
            description: "500000 gold".into(),
            state: "pendingClaim".into(),
        };
        assert_eq!(claim_button_label(&delivery), "領取金幣");
        delivery.item_uid = "1028024501".into();
        delivery.template_id = 245;
        assert_eq!(claim_button_label(&delivery), "領取原始裝備");
        delivery.item_uid = "0".into();
        assert_eq!(
            claim_button_label(&delivery),
            "領取原始裝備",
            "a missing equipment UID must not be presented as a gold payout"
        );
    }
    #[test]
    fn procurement_never_blind_signs_changed_price_network_conditions_key_or_bytes() {
        let (mut model, quote, now) = publish_quote_fixture();
        assert!(model.validate_quote(&quote, now).is_ok());
        for change in 0..8 {
            let mut q = quote.clone();
            match change {
                0 => q.intent.as_mut().unwrap().price_gold = "500001".into(),
                1 => q.intent.as_mut().unwrap().network_hash = encode_hex(&[9; 32]),
                2 => q.conditions.min_current_durability = 1,
                3 => q.intent.as_mut().unwrap().actor_public_key = encode_hex(&[9; 32]),
                4 => q.canonical_intent_hex.as_mut().unwrap().push_str("00"),
                5 => q.intent.as_mut().unwrap().valid_until_ms = now.to_string(),
                6 => q.service_fee.credit_cost = 1,
                _ => q.intent.as_mut().unwrap().fill_id = encode_hex(&[9; 32]),
            }
            assert!(model.validate_quote(&q, now).is_err(), "change {change}");
            model.quote = Some(q);
            model.quote_verified = true;
            model.action(Action::Confirm, now);
            assert!(
                model.pending.is_none(),
                "changed quote must not create signed request"
            );
        }
        model.quote = Some(quote);
        model.quote_verified = true;
        assert!(model.unsent_request().is_none());
        model.action(Action::Confirm, now);
        let request = model.unsent_request().unwrap().clone();
        let ProcurementAction::Confirm {
            quote_id,
            signature,
        } = &request.action
        else {
            panic!("explicit confirmation required");
        };
        assert_eq!(quote_id, "quote-fixture");
        assert_eq!(decode_hex(signature).unwrap().len(), 64);
        model.action(Action::Confirm, now + 1);
        assert_eq!(
            model.unsent_request(),
            Some(&request),
            "double confirm cannot create another operation"
        );
    }
    #[test]
    fn procurement_timeout_and_retry_preserve_exact_request_and_never_confirm() {
        let mut m = ProcurementModel::default();
        m.open_for("alice", 0, 12, 1000);
        let req = m.unsent_request().unwrap().clone();
        m.mark_sent(&req.request_id);
        m.expire(16000);
        assert!(m.pending.as_ref().unwrap().unknown);
        m.action(Action::Confirm, 16001);
        assert_eq!(m.pending.as_ref().unwrap().request, req);
        m.action(Action::Retry, 17000);
        assert_eq!(m.unsent_request(), Some(&req));
        m.action(Action::Close, 17001);
        assert!(!m.open);
        assert!(m.pending.is_some());
    }
    #[test]
    fn procurement_replies_require_current_account_character_and_request() {
        let mut m = ProcurementModel::default();
        m.open_for("alice", 0, 12, 1000);
        let id = m.unsent_request().unwrap().request_id.clone();
        m.mark_sent(&id);
        let mut r = ProcurementReply {
            protocol_version: 1,
            request_id: id,
            character_index: 0,
            status: None,
            quote: None,
            error: None,
            replayed: false,
        };
        assert!(!m.accept(r.clone(), "bob", Some(0), 1001));
        assert!(!m.accept(r.clone(), "alice", Some(1), 1001));
        r.request_id.push('x');
        assert!(!m.accept(r, "alice", Some(0), 1001));
        assert!(m.pending.is_some());
    }
    #[test]
    fn procurement_gateway_unknown_error_keeps_original_request_retryable() {
        let mut m = ProcurementModel::default();
        m.open_for("alice", 0, 12, 1000);
        let request = m.unsent_request().unwrap().clone();
        m.mark_sent(&request.request_id);
        assert!(m.accept(
            ProcurementReply {
                protocol_version: 1,
                request_id: request.request_id.clone(),
                character_index: 0,
                status: None,
                quote: None,
                error: Some("procurementResultUnconfirmed".into()),
                replayed: false
            },
            "alice",
            Some(0),
            1001
        ));
        assert!(m.pending.as_ref().unwrap().unknown);
        m.action(Action::Retry, 1002);
        assert_eq!(m.unsent_request(), Some(&request));
    }
    #[test]
    fn procurement_unsigned_test_package_cannot_enable_cash_charge() {
        let mut m = ProcurementModel::default();
        m.open_for("alice", 0, 12, 1000);
        m.pending = None;
        m.quote_action = Some(ProcurementAction::QuoteBuyQuota {
            package_id: "test-10".into(),
        });
        let mut q = ProcurementQuote {
            quote_id: "q1".into(),
            operation: "buyQuota".into(),
            intent: None,
            canonical_intent_hex: None,
            conditions: Default::default(),
            item_uid: None,
            item_description: None,
            service_fee: ServiceFeeQuote {
                quota_source: "test".into(),
                ..Default::default()
            },
            quota_package_id: Some("test-10".into()),
        };
        assert!(m.validate_quote(&q, 1000).is_ok());
        q.service_fee.credit_cost = 1;
        assert!(m.validate_quote(&q, 1000).is_err());
        q.service_fee.credit_cost = 0;
        q.service_fee.production_billing_enabled = true;
        assert!(m.validate_quote(&q, 1000).is_err());
    }
}
