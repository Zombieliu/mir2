//! Source `MailComposeParcelDialog` state and renderer.
//!
//! The mail transport remains authoritative: this resource owns only the
//! source frame, an uncorrelated postage quote state machine, and the local
//! identities currently placed in the five visible parcel cells.
use super::*;
use std::collections::BTreeSet;

pub(super) const MAIL_PARCEL_SIZE: Vec2 = Vec2::new(236.0, 384.0);
pub(super) const MAIL_PARCEL_DEFAULT_POSITION: Vec2 = Vec2::new(326.0, 0.0);
pub(super) const MAIL_PARCEL_BODY_RECT: CrystalRect = CrystalRect::new(15.0, 98.0, 202.0, 165.0);
pub(super) const MAIL_PARCEL_SLOT_ORIGIN: Vec2 = Vec2::new(27.0, 311.0);
pub(super) const MAIL_PARCEL_SLOT_SIZE: Vec2 = Vec2::new(35.0, 31.0);
const MAIL_PARCEL_SLOT_STEP: f32 = 36.0;
const COST_TIMEOUT_MS: u64 = 5_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Fingerprint {
    pub(super) gold: u32,
    pub(super) attachment_unique_ids: Vec<u64>,
    /// MailCost is computed from the server's current item value, not solely
    /// from a UID. Keep the client flight tied to every live/source field
    /// that influences Crystal's current-price calculation, so a durability,
    /// stack, price, or added-stat update cannot authorize an old quote.
    attachment_pricing: Vec<AttachmentPricing>,
    pub(super) stamped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AttachmentPricing {
    unique_id: u64,
    template_price: u32,
    template_durability: u16,
    quantity: u32,
    durability_current: Option<u16>,
    durability_max: Option<u16>,
    added_stat_weight: u32,
}

impl Fingerprint {
    fn from_draft(
        draft: &mir2_ui_core::state::MailComposeDraft,
        inventory: &InventoryModel,
        stamped: bool,
    ) -> Option<Self> {
        let attachment_unique_ids = valid_mail_attachment_ids(inventory, &draft.attachment_unique_ids)?;
        let attachment_pricing = attachment_unique_ids
            .iter()
            .map(|unique_id| attachment_pricing(inventory, *unique_id))
            .collect::<Option<Vec<_>>>()?;
        Some(Self { gold: draft.gold, attachment_unique_ids, attachment_pricing, stamped })
    }
}

/// Captures the exact current-price inputs visible in the authoritative
/// inventory snapshot. `MailCost` must never be reused after a packet changes
/// any of them. Missing concrete tooltip metadata is deliberately unavailable
/// rather than represented by display fields or guessed item prices.
fn attachment_pricing(inventory: &InventoryModel, unique_id: u64) -> Option<AttachmentPricing> {
    let item = inventory.items.iter().find(|item| {
        item.container == 0
            && item.slot < u32::from(inventory.bag_slot_capacity())
            && item.unique_id == Some(unique_id)
    })?;
    let source = item.tooltip_source.as_ref()?;
    let user = source.user_item.as_ref()?;
    if user.unique_id != unique_id || user.item_index != source.info.item_index || item.quantity == 0 {
        return None;
    }
    if source.info.durability > 0 {
        let current = item.durability_current?;
        let maximum = item.durability_max?;
        if current > maximum {
            return None;
        }
    }
    let added_stat_weight = user.added_stats.iter().try_fold(0_u32, |total, stat| {
        total.checked_add(stat.value.checked_abs()? as u32)
    })?;
    Some(AttachmentPricing {
        unique_id,
        template_price: source.info.price,
        template_durability: source.info.durability,
        quantity: item.quantity,
        durability_current: item.durability_current,
        durability_max: item.durability_max,
        added_stat_weight,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingCost {
    fingerprint: Fingerprint,
    requested_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Quote {
    fingerprint: Fingerprint,
    cost: u32,
}

#[derive(Debug, Clone)]
pub(super) struct MailParcelWindow {
    pub position: Vec2,
    drag: Option<(String, Vec2)>,
}

impl Default for MailParcelWindow {
    fn default() -> Self {
        Self { position: MAIL_PARCEL_DEFAULT_POSITION, drag: None }
    }
}

impl MailParcelWindow {
    pub(super) fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    fn clear_drag(&mut self) {
        self.drag = None;
    }
}

/// The source page has five cells but leaves only the first exposed until a
/// real stamp is selected. `locked_ids` records server echoes; `selected_ids`
/// are stored on the authoritative compose draft and are always rechecked
/// before a request is sent.
#[derive(Debug, Resource)]
pub(super) struct MailParcelUi {
    pub window: MailParcelWindow,
    stamped: bool,
    quote: Option<Quote>,
    pending_cost: Option<PendingCost>,
    desired: Option<Fingerprint>,
    quote_error: Option<String>,
    locked_ids: BTreeSet<u64>,
    selected_ids: BTreeSet<u64>,
    reset_revision: Option<u64>,
}

impl Default for MailParcelUi {
    fn default() -> Self {
        Self {
            window: default(),
            stamped: false,
            quote: None,
            pending_cost: None,
            desired: None,
            quote_error: None,
            locked_ids: BTreeSet::new(),
            selected_ids: BTreeSet::new(),
            reset_revision: None,
        }
    }
}

impl MailParcelUi {
    pub(super) fn stamped(&self) -> bool {
        self.stamped
    }

    pub(super) fn postage(&self) -> Option<u32> {
        self.quote.as_ref().map(|quote| quote.cost).filter(|cost| *cost != u32::MAX)
    }

    pub(super) fn quote_error(&self) -> Option<&str> {
        self.quote_error.as_deref()
    }

    pub(super) fn stamp_available(&self, inventory: &InventoryModel) -> bool {
        inventory.items.iter().any(is_live_stamp)
    }

    pub(super) fn toggle_stamp(&mut self, inventory: &InventoryModel) -> bool {
        if self.stamped {
            self.stamped = false;
            self.invalidate_quote();
            return true;
        }
        if self.stamp_available(inventory) {
            self.stamped = true;
            self.invalidate_quote();
            return true;
        }
        false
    }

    pub(super) fn apply_lock_receipt(&mut self, unique_id: u64, locked: bool) {
        if locked {
            self.locked_ids.insert(unique_id);
        } else {
            self.locked_ids.remove(&unique_id);
        }
    }

    pub(super) fn selected_ids(&self) -> impl Iterator<Item = u64> + '_ {
        self.selected_ids.iter().copied()
    }

    pub(super) fn attach(
        &mut self,
        draft: &mut mir2_ui_core::state::MailComposeDraft,
        inventory: &InventoryModel,
        unique_id: u64,
    ) -> bool {
        if !mail_attachment_is_current(inventory, unique_id)
            || draft.attachment_unique_ids.contains(&unique_id)
            || draft.attachment_unique_ids.len() >= self.slot_limit()
        {
            return false;
        }
        draft.attachment_unique_ids.push(unique_id);
        self.selected_ids.insert(unique_id);
        self.invalidate_quote();
        true
    }

    pub(super) fn detach_at(
        &mut self,
        draft: &mut mir2_ui_core::state::MailComposeDraft,
        slot: usize,
    ) -> Option<u64> {
        let unique_id = *draft.attachment_unique_ids.get(slot)?;
        draft.attachment_unique_ids.remove(slot);
        self.selected_ids.remove(&unique_id);
        self.locked_ids.remove(&unique_id);
        self.invalidate_quote();
        Some(unique_id)
    }

    pub(super) fn slot_limit(&self) -> usize {
        if self.stamped { MAX_MAIL_ATTACHMENTS } else { 1 }
    }

    /// Revalidate the source item IDs before each quote. Snapshot changes may
    /// remove an attachment or the real stamp after the local selection was
    /// made; those hidden identities must never remain in a later request.
    /// Returns `(unlock, lock)` identities for the caller to send.
    pub(super) fn reconcile_draft(
        &mut self,
        draft: &mut mir2_ui_core::state::MailComposeDraft,
        inventory: &InventoryModel,
    ) -> (Vec<u64>, Vec<u64>) {
        let mut unlock = Vec::new();
        let mut changed = false;
        if self.stamped && !self.stamp_available(inventory) {
            self.stamped = false;
            changed = true;
        }
        let mut seen = BTreeSet::new();
        draft.attachment_unique_ids.retain(|unique_id| {
            let current = seen.insert(*unique_id) && mail_attachment_is_current(inventory, *unique_id);
            if !current {
                changed = true;
                if self.selected_ids.remove(unique_id) {
                    self.locked_ids.remove(unique_id);
                    unlock.push(*unique_id);
                }
            }
            current
        });
        while draft.attachment_unique_ids.len() > self.slot_limit() {
            let unique_id = draft.attachment_unique_ids.pop().expect("bounded by length");
            changed = true;
            if self.selected_ids.remove(&unique_id) {
                self.locked_ids.remove(&unique_id);
                unlock.push(unique_id);
            }
        }
        let mut lock = Vec::new();
        for unique_id in &draft.attachment_unique_ids {
            if self.selected_ids.insert(*unique_id) {
                lock.push(*unique_id);
            }
        }
        if changed {
            self.invalidate_quote();
        }
        (unlock, lock)
    }

    pub(super) fn quote_is_current(
        &self,
        draft: &mir2_ui_core::state::MailComposeDraft,
        inventory: &InventoryModel,
    ) -> bool {
        let Some(current) = Fingerprint::from_draft(draft, inventory, self.stamped) else {
            return false;
        };
        self.quote.as_ref().is_some_and(|quote| quote.fingerprint == current && quote.cost != u32::MAX)
            && self.pending_cost.is_none()
    }

    pub(super) fn begin_quote(
        &mut self,
        draft: &mir2_ui_core::state::MailComposeDraft,
        inventory: &InventoryModel,
        now_ms: u64,
    ) -> Option<Fingerprint> {
        if self.stamped && !self.stamp_available(inventory) {
            self.stamped = false;
            self.invalidate_quote();
        }
        let desired = Fingerprint::from_draft(draft, inventory, self.stamped)?;
        if self.desired.as_ref() != Some(&desired) {
            self.desired = Some(desired.clone());
            self.quote = None;
            // A prior timeout remains meaningful while its uncorrelated
            // request still owns the only response slot.
            if self.pending_cost.is_none() {
                self.quote_error = None;
            }
        }
        if self.pending_cost.is_some() || self.quote.as_ref().is_some_and(|quote| quote.fingerprint == desired) {
            return None;
        }
        self.pending_cost = Some(PendingCost { fingerprint: desired.clone(), requested_at_ms: now_ms });
        Some(desired)
    }

    pub(super) fn tick_quote_timeout(&mut self, now_ms: u64) {
        let Some(pending) = self.pending_cost.as_ref() else {
            return;
        };
        if now_ms.saturating_sub(pending.requested_at_ms) < COST_TIMEOUT_MS {
            return;
        }
        // `MailCost` has no request ID. Do not issue a replacement quote:
        // a delayed reply could otherwise be bound to that newer request.
        // The exact old request remains reserved until its reply arrives or
        // the session boundary discards the native event stream.
        self.quote_error = Some("Postage quote unavailable".to_owned());
    }

    /// `MailCost` carries no fingerprint. A changed draft must never accept an
    /// earlier response: release the completed flight and let the latest
    /// desired fingerprint issue exactly one subsequent request.
    pub(super) fn apply_cost(
        &mut self,
        cost: u32,
        draft: Option<&mir2_ui_core::state::MailComposeDraft>,
        inventory: &InventoryModel,
    ) {
        let Some(pending) = self.pending_cost.take() else {
            return;
        };
        // `MailCost` does not carry a request identifier. Compare the pending
        // request directly with the live compose data, rather than the
        // previous frame's cached `desired` fingerprint: an edit and a reply
        // can be ingested in the same update.
        let current = draft.and_then(|draft| Fingerprint::from_draft(draft, inventory, self.stamped));
        if current.as_ref() == Some(&pending.fingerprint) {
            self.quote = Some(Quote { fingerprint: pending.fingerprint, cost });
            self.quote_error = (cost == u32::MAX).then(|| "Postage quote unavailable".to_owned());
        }
    }

    pub(super) fn invalidate_quote(&mut self) {
        self.quote = None;
        self.desired = None;
        self.quote_error = None;
        // Keep the in-flight fingerprint. A subsequent response is ignored
        // when it no longer matches `desired`, then the newest draft gets a
        // single new request.
    }

    pub(super) fn release_all(&mut self) -> Vec<u64> {
        let ids = self.selected_ids.iter().copied().collect();
        self.selected_ids.clear();
        self.locked_ids.clear();
        self.stamped = false;
        self.quote = None;
        // A Cost reply is uncorrelated. Preserve its exact in-flight
        // fingerprint across a local close so reopening cannot bind that
        // delayed reply to a replacement request.
        self.desired = None;
        self.quote_error = None;
        ids
    }

    pub(super) fn session_reset(&mut self, revision: u64) -> Option<Vec<u64>> {
        if self.reset_revision.is_some_and(|previous| previous != revision) {
            self.window = default();
            self.reset_revision = Some(revision);
            let released = self.release_all();
            // The runtime clears the packet inbox at this boundary, so a
            // pre-reset `MailCost` can no longer arrive.
            self.pending_cost = None;
            return Some(released);
        }
        self.reset_revision = Some(revision);
        None
    }
}

pub(super) fn is_live_stamp(item: &ItemModel) -> bool {
    item.container == 0
        && item.unique_id.is_some_and(|id| id != 0)
        && item.tooltip_source.as_ref().is_some_and(|source| {
            source.info.item_index == 838
                && source.info.item_type == 0
                && source.info.shape == 1
                && source.user_item.as_ref().is_some_and(|user| user.unique_id != 0 && user.count > 0)
        })
}

pub(super) fn slot_at_cursor(ui: &MailParcelUi, cursor: Vec2) -> Option<usize> {
    let local = cursor - ui.window.position - MAIL_PARCEL_SLOT_ORIGIN;
    if local.y < 0.0 || local.y >= MAIL_PARCEL_SLOT_SIZE.y {
        return None;
    }
    let index = (local.x / MAIL_PARCEL_SLOT_STEP).floor() as i32;
    (0..MAX_MAIL_ATTACHMENTS as i32)
        .contains(&index)
        .then_some(index as usize)
        .filter(|index| local.x - *index as f32 * MAIL_PARCEL_SLOT_STEP < MAIL_PARCEL_SLOT_SIZE.x)
        .filter(|index| *index < ui.slot_limit())
}

pub(super) fn drag_surface(ui: &MailParcelUi, point: Vec2) -> bool {
    let local = point - ui.window.position;
    let contains = |rect: CrystalRect| rect.contains(local.x, local.y);
    contains(CrystalRect::new(0.0, 0.0, MAIL_PARCEL_SIZE.x, MAIL_PARCEL_SIZE.y))
        && !contains(CrystalRect::new(209.0, 3.0, 24.0, 21.0))
        && !contains(MAIL_PARCEL_BODY_RECT)
        && !contains(CrystalRect::new(73.0, 56.0, 20.0, 20.0))
        && !contains(CrystalRect::new(63.0, 269.0, 143.0, 36.0))
        && slot_at_cursor(ui, point).is_none()
        && !contains(CrystalRect::new(30.0, 350.0, 76.0, 25.0))
        && !contains(CrystalRect::new(135.0, 350.0, 68.0, 25.0))
}

pub(super) fn process_drag(
    shell: Res<NativeShellModel>,
    mut state: ResMut<NativePlayerUiState>,
    compose: Res<MailComposeUi>,
    mut parcel: ResMut<MailParcelUi>,
    pending: Res<PendingOperations>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    windows: Query<(Entity, &Window), With<PrimaryWindow>>,
    mut moves: MessageReader<CursorMoved>,
) {
    if shell.screen != NativeShellScreen::InGame
        || !state.mail_open()
        || state.core.mail_compose.is_none()
        || compose.kind != MailComposeKind::Parcel
        || pending.has_pending_mail_send()
        || state.amount_modal_open()
        || state.mail_feedback_prompt.is_some()
        || state.mail_feedback_input_consumed
        || state.mail_recipient_prompt_active
        || state.mail_recipient_input_consumed
        || mail_compose_drag::covered(&state)
    {
        parcel.window.clear_drag();
        moves.clear();
        return;
    }
    let (Some(mouse), Ok((entity, window))) = (mouse, windows.single()) else {
        parcel.window.clear_drag();
        moves.clear();
        return;
    };
    if !window.focused {
        parcel.window.clear_drag();
        return;
    }
    let path: Vec<_> = moves
        .read()
        .filter(|event| event.window == entity)
        .map(|event| cursor_logical(window, event.position))
        .collect();
    let Some(cursor) = path.last().copied().or_else(|| help_cursor_logical(window)) else {
        parcel.window.clear_drag();
        return;
    };
    let recipient = state
        .core
        .mail_compose
        .as_ref()
        .map(|draft| draft.recipient.clone())
        .unwrap_or_default();
    if mouse.just_pressed(MouseButton::Left) && !state.menu_pointer_consumed {
        parcel.window.clear_drag();
        let start = path.first().copied().unwrap_or(cursor);
        if drag_surface(&parcel, start) {
            parcel.window.drag = Some((recipient.clone(), start - parcel.window.position));
        }
    }
    if let Some((owner, offset)) = parcel.window.drag.clone() {
        if owner != recipient {
            parcel.window.clear_drag();
        } else if mouse.pressed(MouseButton::Left) || mouse.just_released(MouseButton::Left) {
            parcel.window.position = (cursor - offset).clamp(Vec2::ZERO, Vec2::new(788.0, 384.0));
            state.menu_pointer_consumed = true;
        }
    }
    if mouse.just_released(MouseButton::Left) || !mouse.pressed(MouseButton::Left) {
        parcel.window.clear_drag();
    }
}

pub(super) fn render(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    draft: &mir2_ui_core::state::MailComposeDraft,
    inventory: &InventoryModel,
    parcel: &MailParcelUi,
    editor: Option<&mail_editor::MailLetterEditor>,
) {
    let quote_ready = parcel.quote_is_current(draft, inventory);
    if let Some(asset_server) = asset_server {
        spawn_overlay_frame(parent, asset_server, "original-ui/Title/674.png", MAIL_PARCEL_SIZE.x, MAIL_PARCEL_SIZE.y);
        spawn_overlay_crystal_button(parent, asset_server, "Prguse2", 360, 361, 362,
            CrystalRect::new(209.0, 3.0, 24.0, 21.0), OverlayButton::CancelMailCompose);
        spawn_overlay_crystal_button_enabled(parent, asset_server, "Prguse2", 203, 204, 205,
            CrystalRect::new(73.0, 56.0, 20.0, 20.0), OverlayButton::MailParcelStamp,
            parcel.stamp_available(inventory));
        spawn_overlay_crystal_button_enabled(parent, asset_server, "Title", 607, 608, 609,
            CrystalRect::new(30.0, 350.0, 76.0, 25.0), OverlayButton::SubmitMail,
            quote_ready && !draft.recipient.trim().is_empty() && !draft.message.trim().is_empty());
        spawn_overlay_crystal_button(parent, asset_server, "Title", 193, 194, 195,
            CrystalRect::new(135.0, 350.0, 68.0, 25.0), OverlayButton::CancelMailCompose);
    }
    overlay_text_at(parent, &draft.recipient, CrystalRect::new(70.0, 35.0, 150.0, 15.0),
        crate::crystal_ui::typography::CRYSTAL_DEFAULT_FONT_SIZE_PX, TEXT);
    if let Some(editor) = editor {
        editor.render_at(parent, MAIL_PARCEL_BODY_RECT);
    } else {
        overlay_text_at(parent, &draft.message, MAIL_PARCEL_BODY_RECT,
            crate::crystal_ui::typography::CRYSTAL_DEFAULT_FONT_SIZE_PX, TEXT);
    }
    overlay_text_at(parent, &format!("Postage: {}", parcel.postage().map_or("…".to_owned(), |cost| cost.to_string())),
        CrystalRect::new(63.0, 269.0, 143.0, 15.0), 9.0, TEXT);
    overlay_absolute_button(
        parent,
        "",
        CrystalRect::new(63.0, 290.0, 143.0, 15.0),
        OverlayButton::MailGoldFocus,
        true,
    );
    overlay_text_at(parent, &format!("Gold: {}", draft.gold), CrystalRect::new(63.0, 290.0, 143.0, 15.0), 9.0, TEXT);
    if let Some(error) = parcel.quote_error() {
        overlay_text_at(parent, error, CrystalRect::new(15.0, 78.0, 202.0, 15.0), 8.0, Color::srgb(0.95, 0.34, 0.28));
    }
    if !parcel.stamped() {
        if let Some(asset_server) = asset_server {
            spawn_static_overlay_sprite(parent, asset_server, "original-ui/Title/676.png".to_owned(),
                CrystalRect::new(63.0, 310.0, 144.0, 33.0));
        }
    }
    for slot in 0..parcel.slot_limit() {
        let left = MAIL_PARCEL_SLOT_ORIGIN.x + slot as f32 * MAIL_PARCEL_SLOT_STEP;
        let rect = CrystalRect::new(left, MAIL_PARCEL_SLOT_ORIGIN.y, MAIL_PARCEL_SLOT_SIZE.x, MAIL_PARCEL_SLOT_SIZE.y);
        overlay_absolute_button(parent, "", rect, OverlayButton::MailParcelSlot(slot as u8), true);
        let Some(unique_id) = draft.attachment_unique_ids.get(slot).copied() else { continue; };
        let Some(item) = inventory.items.iter().find(|item| item.unique_id == Some(unique_id) && item.container == 0) else { continue; };
        if let (Some(asset_server), Some(image)) = (asset_server, concrete_item_image_index(item.icon, item.quantity, item.tooltip_source.as_ref())) {
            parent.spawn((Node { position_type: PositionType::Absolute, left: Val::Px(left), top: Val::Px(MAIL_PARCEL_SLOT_ORIGIN.y), width: Val::Px(35.0), height: Val::Px(31.0), ..default() }, FocusPolicy::Pass))
                .with_children(|cell| spawn_original_item_image(cell, asset_server, image, 35, 31));
        }
        let count = item.crystal_stack_label();
        if !count.is_empty() {
            overlay_text_at(parent, &count, CrystalRect::new(left + 18.0, MAIL_PARCEL_SLOT_ORIGIN.y + 17.0, 16.0, 12.0), 9.0, Color::srgb(1.0, 1.0, 0.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_postage_is_unavailable_and_does_not_retry_every_frame() {
        let mut ui = MailParcelUi::default();
        let inventory = InventoryModel::default();
        let mut draft = mir2_ui_core::state::MailComposeDraft::default();
        draft.gold = 1_000;
        assert!(ui.begin_quote(&draft, &inventory, 1).is_some());
        ui.apply_cost(u32::MAX, Some(&draft), &inventory);
        assert!(!ui.quote_is_current(&draft, &inventory));
        assert_eq!(ui.postage(), None);
        assert_eq!(ui.quote_error(), Some("Postage quote unavailable"));
        assert!(ui.begin_quote(&draft, &inventory, 2).is_none());
        draft.gold = 2_000;
        assert!(ui.begin_quote(&draft, &inventory, 3).is_some());
    }

    fn item(id: u64, slot: u32) -> ItemModel {
        ItemModel {
            unique_id: Some(id),
            slot,
            container: 0,
            quantity: 1,
            tooltip_source: Some(crate::inventory::CrystalItemTooltipSourceModel {
                info: crate::inventory::CrystalItemInfoModel {
                    item_index: 9,
                    price: 100,
                    stack_size: 1,
                    ..default()
                },
                user_item: Some(crate::inventory::CrystalUserItemModel {
                    unique_id: id,
                    item_index: 9,
                    count: 1,
                    ..default()
                }),
                ..default()
            }),
            ..default()
        }
    }

    #[test]
    fn slots_follow_source_cover_and_use_live_identities() {
        let mut ui = MailParcelUi::default();
        let inventory = InventoryModel { items: vec![item(9, 0)], ..default() };
        let mut draft = mir2_ui_core::state::MailComposeDraft::default();
        assert_eq!(slot_at_cursor(&ui, MAIL_PARCEL_DEFAULT_POSITION + MAIL_PARCEL_SLOT_ORIGIN + Vec2::new(2.0, 2.0)), Some(0));
        assert!(slot_at_cursor(&ui, MAIL_PARCEL_DEFAULT_POSITION + MAIL_PARCEL_SLOT_ORIGIN + Vec2::new(38.0, 2.0)).is_none());
        assert!(ui.attach(&mut draft, &inventory, 9));
        assert!(!ui.attach(&mut draft, &inventory, 10), "stale IDs cannot occupy source cells");
        assert_eq!(ui.detach_at(&mut draft, 0), Some(9));
    }

    #[test]
    fn refresh_drops_replaced_attachment_and_releases_only_its_exact_identity() {
        let mut ui = MailParcelUi::default();
        let initial = InventoryModel { items: vec![item(9, 0)], ..default() };
        let mut draft = mir2_ui_core::state::MailComposeDraft::default();
        assert!(ui.attach(&mut draft, &initial, 9));

        let (unlock, lock) = ui.reconcile_draft(&mut draft, &InventoryModel::default());
        assert_eq!(unlock, vec![9]);
        assert!(lock.is_empty());
        assert!(draft.attachment_unique_ids.is_empty());
    }

    #[test]
    fn timed_out_cost_reserves_its_fingerprint_until_the_delayed_reply_arrives() {
        let mut ui = MailParcelUi::default();
        let inventory = InventoryModel { items: vec![item(9, 0)], ..default() };
        let mut draft = mir2_ui_core::state::MailComposeDraft {
            gold: 5,
            attachment_unique_ids: vec![9],
            ..default()
        };
        assert!(ui.begin_quote(&draft, &inventory, 1).is_some());
        ui.tick_quote_timeout(COST_TIMEOUT_MS + 2);
        assert_eq!(ui.quote_error(), Some("Postage quote unavailable"));
        assert!(ui.pending_cost.is_some(), "the uncorrelated request stays reserved");

        draft.gold = 7;
        assert!(ui.begin_quote(&draft, &inventory, COST_TIMEOUT_MS + 3).is_none());
        ui.apply_cost(12, Some(&draft), &inventory);
        assert!(ui.pending_cost.is_none(), "the delayed old reply is consumed and rejected");
        assert!(ui.begin_quote(&draft, &inventory, COST_TIMEOUT_MS + 4).is_some());
    }

    #[test]
    fn local_close_keeps_an_unanswered_cost_request_reserved() {
        let mut ui = MailParcelUi::default();
        let inventory = InventoryModel { items: vec![item(9, 0)], ..default() };
        let draft = mir2_ui_core::state::MailComposeDraft {
            gold: 5,
            attachment_unique_ids: vec![9],
            ..default()
        };
        assert!(ui.begin_quote(&draft, &inventory, 1).is_some());
        ui.release_all();
        assert!(ui.pending_cost.is_some());
        assert!(ui.begin_quote(&draft, &inventory, 2).is_none());
    }

    #[test]
    fn cost_response_is_ignored_when_the_draft_changed_in_flight() {
        let mut ui = MailParcelUi::default();
        let inventory = InventoryModel { items: vec![item(9, 0)], ..default() };
        let mut draft = mir2_ui_core::state::MailComposeDraft { gold: 5, attachment_unique_ids: vec![9], ..default() };
        assert!(ui.begin_quote(&draft, &inventory, 1).is_some());
        draft.gold = 7;
        // The cached desired fingerprint still describes the previous quote.
        // Only the live draft comparison may decide whether this reply applies.
        ui.apply_cost(100, Some(&draft), &inventory);
        assert!(ui.postage().is_none());
        assert!(ui.begin_quote(&draft, &inventory, 3).is_some());
    }
}
