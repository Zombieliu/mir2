//! Ordinary Bag material conversion controls. This module emits typed local
//! intents only; prices, owner custody, the three-lane send arbiter and Applied
//! completion remain owned by the physical host and authoritative server.
use super::*;
use crate::hunting_material::{HuntingMaterialActorScope, HuntingMaterialPhase, HuntingMaterialUi};
use std::collections::BTreeMap;

const MAX_DISPLAY_ACTORS: usize = 32;

#[derive(Debug, Clone, PartialEq)]
struct RetainedActorUi {
    guard: HuntingMaterialUi,
    selection: Option<crate::bag_ui::BagSelection>,
    quantity: u32,
    quote_source: Option<ItemModel>,
    open: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct MaterialServiceUi {
    pub(super) guard: HuntingMaterialUi,
    selection: Option<crate::bag_ui::BagSelection>,
    quantity: u32,
    quote_source: Option<ItemModel>,
    open: bool,
    actor_scope: Option<HuntingMaterialActorScope>,
    retained: BTreeMap<HuntingMaterialActorScope, RetainedActorUi>,
}
impl Default for MaterialServiceUi {
    fn default() -> Self {
        Self {
            guard: Default::default(),
            selection: None,
            quantity: 1,
            quote_source: None,
            open: false,
            actor_scope: None,
            retained: BTreeMap::new(),
        }
    }
}

impl MaterialServiceUi {
    pub(super) fn withdraw_display(&mut self) {
        self.observe(&HuntingMaterialModel::default());
    }

    fn take_active(&mut self) -> RetainedActorUi {
        RetainedActorUi {
            guard: std::mem::take(&mut self.guard),
            selection: self.selection.take(),
            quantity: std::mem::replace(&mut self.quantity, 1),
            quote_source: self.quote_source.take(),
            open: std::mem::replace(&mut self.open, false),
        }
    }

    fn restore(&mut self, actor: RetainedActorUi) {
        self.guard = actor.guard;
        self.selection = actor.selection;
        self.quantity = actor.quantity;
        self.quote_source = actor.quote_source;
        self.open = actor.open;
    }

    fn observe(&mut self, model: &HuntingMaterialModel) {
        let scope = model
            .actor_scope
            .as_ref()
            .filter(|scope| scope.is_valid())
            .cloned();
        if scope != self.actor_scope {
            if let Some(previous) = self.actor_scope.take() {
                let saved = self.take_active();
                self.retained.insert(previous, saved);
            } else {
                self.take_active();
            }
            let Some(scope) = scope else {
                return;
            };
            if let Some(saved) = self.retained.remove(&scope) {
                self.restore(saved);
            } else if self.retained.len() >= MAX_DISPLAY_ACTORS {
                // No eviction can silently discard an original pending result.
                // Known scopes remain restorable even after capacity is reached.
                return;
            }
            self.actor_scope = Some(scope);
        }
        if self.actor_scope.is_none() {
            return;
        }
        if self.guard.pending() && self.guard.original_quote_id() != model.quote_id.as_deref() {
            // A generic Ready/Unknown or unrelated quote cannot erase the
            // retained original. Refresh only display availability for the
            // verified current scope; a real original Query decides its result.
            let mut retained = self.guard.model().clone();
            retained.actor_scope = self.actor_scope.clone();
            retained.phase = HuntingMaterialPhase::Pending;
            retained.protocol_v1 = model.protocol_v1;
            retained.reload_required = model.reload_required;
            retained.notice = Some("originalQueryRequired".into());
            self.guard.observe(&retained);
        } else {
            self.guard.observe(model);
        }
    }
}

pub(super) fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub(super) fn sync_model(
    model: Option<Res<HuntingMaterialModel>>,
    mut state: ResMut<NativePlayerUiState>,
) {
    // A real equal-valued host delivery still acknowledges a UI intent. An
    // unchanged resource on another frame must never fabricate that delivery.
    let Some(model) = model.filter(|model| model.is_changed()) else {
        return;
    };
    state.hunting_material_ui.observe(&model);
}

fn selected<'a>(
    state: &NativePlayerUiState,
    inventory: &'a InventoryModel,
) -> Option<&'a ItemModel> {
    let item = inspected_inventory_item(state, inventory)?;
    let uid = item.unique_id.filter(|uid| *uid > 0)?;
    (item.quantity > 0
        && inventory
            .items
            .iter()
            .filter(|item| item.unique_id == Some(uid))
            .count()
            == 1)
        .then_some(item)
}

fn quantity(state: &NativePlayerUiState, item: &ItemModel) -> u32 {
    if state.hunting_material_ui.selection == crate::bag_ui::BagSelection::from_item(item) {
        state
            .hunting_material_ui
            .quantity
            .clamp(1, item.quantity.max(1))
    } else {
        1
    }
}

fn other_lane_busy(state: &NativePlayerUiState, queue: &NativePlayerUiIntentQueue) -> bool {
    state.hunting_repair_ui.pending() || queue.npc_gold_buy_feedback().pending
}

fn other_lane_display_busy(state: &NativePlayerUiState) -> bool {
    state.hunting_repair_ui.pending()
        || state
            .npc_gold_buy_gate
            .as_ref()
            .is_some_and(|gate| gate.feedback().pending)
}

pub(super) fn adjust_quantity(
    state: &mut NativePlayerUiState,
    inventory: &InventoryModel,
    action: OverlayButton,
    mail_locked: bool,
) -> bool {
    if mail_locked
        || state.hunting_material_ui.guard.pending()
        || state.hunting_material_ui.guard.input().is_some()
    {
        return false;
    }
    let Some(item) = selected(state, inventory) else {
        return false;
    };
    let count = item.quantity;
    let selected = crate::bag_ui::BagSelection::from_item(item);
    let current = quantity(state, item);
    let next = match action {
        OverlayButton::HuntingMaterialOne => 1,
        OverlayButton::HuntingMaterialQuantityDec => current.saturating_sub(1).max(1),
        OverlayButton::HuntingMaterialQuantityInc => current.saturating_add(1).min(count),
        OverlayButton::HuntingMaterialAll => count,
        _ => return false,
    };
    state.hunting_material_ui.selection = selected;
    state.hunting_material_ui.quantity = next;
    true
}

pub(super) fn request_quote(
    state: &mut NativePlayerUiState,
    inventory: &InventoryModel,
    queue: &mut NativePlayerUiIntentQueue,
    mail_locked: bool,
) -> bool {
    if mail_locked || other_lane_busy(state, queue) {
        return false;
    }
    let Some(item) = selected(state, inventory).cloned() else {
        return false;
    };
    let uid = item.unique_id.expect("selected positive material UID");
    let quantity = quantity(state, &item);
    let mut next = state.hunting_material_ui.guard.clone();
    if !next.begin_quote(uid, u64::from(quantity))
        || !queue.push_transient_unique(NativePlayerUiIntent::HuntingMaterialQuote {
            unique_id: uid,
            quantity: u64::from(quantity),
        })
    {
        return false;
    }
    state.hunting_material_ui.guard = next;
    state.hunting_material_ui.selection = crate::bag_ui::BagSelection::from_item(&item);
    state.hunting_material_ui.quantity = quantity;
    state.hunting_material_ui.quote_source = Some(item);
    state.hunting_material_ui.open = true;
    true
}

fn current_confirm_source<'a>(
    state: &NativePlayerUiState,
    inventory: &'a InventoryModel,
) -> Option<&'a ItemModel> {
    let item = selected(state, inventory)?;
    let service = &state.hunting_material_ui;
    let model = service.guard.model();
    (service.open
        && service.quote_source.as_ref() == Some(item)
        && model.input_count == Some(u64::from(item.quantity))
        && model.quantity == Some(u64::from(quantity(state, item))))
    .then_some(item)
}

pub(super) fn confirm(
    state: &mut NativePlayerUiState,
    inventory: &InventoryModel,
    queue: &mut NativePlayerUiIntentQueue,
    mail_locked: bool,
    now: i64,
) -> bool {
    if mail_locked || other_lane_busy(state, queue) {
        return false;
    }
    let Some(item) = current_confirm_source(state, inventory) else {
        return false;
    };
    let uid = item.unique_id.expect("selected positive material UID");
    let quantity = u64::from(quantity(state, item));
    let mut next = state.hunting_material_ui.guard.clone();
    if !next.begin_confirm(uid, quantity, now) {
        return false;
    }
    let Some(quote_id) = next.original_quote_id().map(str::to_owned) else {
        return false;
    };
    if !queue.push_transient_unique(NativePlayerUiIntent::HuntingMaterialConfirm { quote_id }) {
        return false;
    }
    state.hunting_material_ui.guard = next;
    true
}

pub(super) fn query(
    state: &mut NativePlayerUiState,
    queue: &mut NativePlayerUiIntentQueue,
) -> bool {
    let mut next = state.hunting_material_ui.guard.clone();
    if !next.begin_query() {
        return false;
    }
    let Some(quote_id) = next.original_quote_id().map(str::to_owned) else {
        return false;
    };
    if !queue.push_transient_unique(NativePlayerUiIntent::HuntingMaterialQuery { quote_id }) {
        return false;
    }
    state.hunting_material_ui.guard = next;
    true
}

pub(super) fn close(state: &mut NativePlayerUiState, queue: &mut NativePlayerUiIntentQueue) {
    let service = &mut state.hunting_material_ui;
    if service.guard.pending()
        || service.guard.model().quote_id.is_some()
        || service.guard.input().is_some()
    {
        let _ = queue.push_transient_unique(NativePlayerUiIntent::HuntingMaterialClose);
        service.guard.close();
    }
    service.open = false;
}

fn can_confirm(state: &NativePlayerUiState, inventory: &InventoryModel, now: i64) -> bool {
    current_confirm_source(state, inventory).is_some_and(|item| {
        state.hunting_material_ui.guard.can_confirm(
            item.unique_id.unwrap_or(0),
            u64::from(quantity(state, item)),
            now,
        )
    })
}

pub(super) fn render_inspect(
    parent: &mut ChildSpawnerCommands,
    state: &NativePlayerUiState,
    inventory: &InventoryModel,
    mail_locked: bool,
) {
    body(parent, "Bound material conversion");
    let item = selected(state, inventory);
    let service = &state.hunting_material_ui;
    let mutable = !mail_locked && !service.guard.pending() && service.guard.input().is_none();
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(2.0),
            ..default()
        })
        .with_children(|row| {
            overlay_button(
                row,
                "1",
                OverlayButton::HuntingMaterialOne,
                item.is_some() && mutable,
            );
            overlay_button(
                row,
                "-",
                OverlayButton::HuntingMaterialQuantityDec,
                item.is_some_and(|item| quantity(state, item) > 1) && mutable,
            );
            body(
                row,
                &item
                    .map(|item| format!("{}/{}", quantity(state, item), item.quantity))
                    .unwrap_or_else(|| "-".into()),
            );
            overlay_button(
                row,
                "+",
                OverlayButton::HuntingMaterialQuantityInc,
                item.is_some_and(|item| quantity(state, item) < item.quantity) && mutable,
            );
            overlay_button(
                row,
                "All",
                OverlayButton::HuntingMaterialAll,
                item.is_some() && mutable,
            );
        });
    let available = !mail_locked && !other_lane_display_busy(state);
    overlay_button(
        parent,
        "Request material quote",
        OverlayButton::HuntingMaterialQuote,
        available
            && item.is_some_and(|item| {
                service.guard.can_quote(
                    item.unique_id.unwrap_or(0),
                    u64::from(quantity(state, item)),
                )
            }),
    );
    if service.guard.model().phase == HuntingMaterialPhase::Quoted {
        overlay_button(
            parent,
            "Confirm material conversion",
            OverlayButton::HuntingMaterialConfirm,
            available && can_confirm(state, inventory, now_ms()),
        );
    }
    if !service.guard.model().protocol_v1 {
        body(parent, "Material conversion unavailable.");
    }
}

#[derive(Component)]
pub(super) struct MaterialRecoveryRoot;

pub(super) fn spawn_recovery_root(root: &mut ChildSpawnerCommands) {
    root.spawn((
        MaterialRecoveryRoot,
        GlobalZIndex(OVERLAY_NPC_DIALOG_Z),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(650.0),
            top: Val::Px(80.0),
            width: Val::Px(300.0),
            display: Display::None,
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(8.0)),
            row_gap: Val::Px(3.0),
            ..default()
        },
        BackgroundColor(PANEL_BG),
    ));
}

fn status(guard: &HuntingMaterialUi) -> &'static str {
    let model = guard.model();
    if model.reload_required {
        return "Reconnect to reload character.";
    }
    if guard.pending() {
        return "Pending: query the original conversion.";
    }
    if model.phase == HuntingMaterialPhase::Committed {
        return "Material conversion completed.";
    }
    if matches!(
        guard.input(),
        Some(crate::hunting_material::HuntingMaterialInput::Quote { .. })
    ) || model.phase == HuntingMaterialPhase::Quoting
    {
        return "Requesting server quote...";
    }
    match model.notice.as_deref() {
        Some("unconfigured") => "This material has no configured conversion value.",
        Some("insufficientUnboundGold") => "Not enough unbound gold for the fee.",
        Some("insufficientPermits") => {
            "Current server conversion quota is insufficient. Please wait."
        }
        Some("expired") => "Quote expired. Request a fresh quote.",
        Some("itemChanged") => "Item changed. Request a fresh quote.",
        Some("unsupported") => "This item cannot be converted.",
        Some("notSubmitted" | "definitelyUnsent") => "Not submitted. Request a fresh quote.",
        Some(_) => "Material conversion unavailable. Try later.",
        None if model.phase == HuntingMaterialPhase::Quoted => {
            "Review the server quote before confirming."
        }
        None => "Request a server quote from a Bag item.",
    }
}

pub(super) fn render_recovery(
    mut commands: Commands,
    state: Res<NativePlayerUiState>,
    inventory: Res<InventoryModel>,
    shell: Option<Res<NativeShellModel>>,
    parcel: Option<Res<mail_parcel::MailParcelUi>>,
    mut panels: Query<(Entity, &mut Node), With<MaterialRecoveryRoot>>,
) {
    let service = &state.hunting_material_ui;
    let visible = shell.is_some_and(|shell| shell.screen == NativeShellScreen::InGame)
        && (service.open || service.guard.recovery_visible());
    fill_panel(&mut commands, &mut panels, visible, |parent| {
        title(parent, "Material conversion");
        let model = service.guard.model();
        if let Some(qty) = model.quantity {
            body(parent, &format!("Quantity: {qty}"));
        }
        if let Some(unit) = model.unit_gold {
            body(parent, &format!("Unit value: {unit} gold equivalent"));
        }
        if let Some(total) = model.gold_equivalent {
            body(parent, &format!("Total value: {total} gold equivalent"));
        }
        if let Some(fee) = model.fee_unbound_gold {
            body(parent, &format!("Fee: {fee} existing unbound gold"));
        }
        if let Some(atoms) = model.license_atoms_debit {
            body(
                parent,
                &format!(
                    "Hunting permits: {}.{:06}",
                    atoms / 1_000_000,
                    atoms % 1_000_000
                ),
            );
        }
        if let Some(version) = model.valuation_version.as_ref() {
            body(parent, &format!("Valuation: {version}"));
        }
        if model.phase == HuntingMaterialPhase::Quoted {
            if let Some(expires) = model.expires_at_ms {
                let remaining = expires.saturating_sub(now_ms()).max(0);
                body(
                    parent,
                    &format!("Quote remaining: {}s", remaining.saturating_add(999) / 1000),
                );
            }
        }
        body(parent, status(&service.guard));
        let locked = parcel
            .as_deref()
            .is_some_and(|parcel| inspected_bag_item_is_mail_locked(&state, &inventory, parcel));
        if model.phase == HuntingMaterialPhase::Quoted {
            overlay_button(
                parent,
                "Confirm material conversion",
                OverlayButton::HuntingMaterialConfirm,
                !locked
                    && !other_lane_display_busy(&state)
                    && can_confirm(&state, &inventory, now_ms()),
            );
        }
        if service.guard.pending() {
            overlay_button(
                parent,
                "Query original conversion",
                OverlayButton::HuntingMaterialQuery,
                service.guard.can_query(),
            );
        }
        overlay_button(parent, "Close", OverlayButton::HuntingMaterialClose, true);
    });
}

#[cfg(test)]
#[path = "hunting_material_service_tests.rs"]
mod tests;
