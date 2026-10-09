//! Shared local ordinary NPC shop controls. Pricing and purchase transport remain
//! in the existing planner and Core attempt slot respectively.
use bevy::prelude::{Component, Resource};
use crate::inventory::InventoryModel;
use crate::npc_gold_buy_attempt::{npc_gold_buy_model_authority, NpcGoldBuyFeedback};
use crate::npc_shop_buy::{plan_npc_gold_buy, NpcGoldBuyCommand, NpcGoldBuyPlan};
use crate::shop::ShopModel;

pub const NPC_SHOP_VISIBLE_ROWS: usize = 8;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NpcShopUiContext {
    pub source_revision: Option<u64>,
    pub open: bool,
    pub input_enabled: bool,
    pub show_buy: bool,
}

/// A local presentation fence only. Hosts also attach/check the actual Gate
/// instance; neither this stamp nor its counter is purchase authority.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct NpcShopUiStamp {
    pub source_revision: u64,
    pub presentation_revision: u64,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcShopUiAction {
    Select(u64), QuantityInc, QuantityDec, PageUp, PageDown,
    Buy, Close, HandoffSell,
}

/// Hosts must withdraw before mutating local controls, not after receiving an
/// effect: a Bound attempt may be waiting on another transport thread.
pub fn action_retires_preentry(action: NpcShopUiAction) -> bool {
    !matches!(action, NpcShopUiAction::Buy)
}

/// Whole-catalog handoff only. A positive ordinary marker with missing raw
/// metadata stays here and is refused by the strict planner, never by legacy.
pub fn ordinary_gold_surface(shop: &ShopModel, showing_buy: bool) -> bool {
    showing_buy && shop.allows_buy() && !shop.goods.is_empty()
        && shop.goods.iter().all(|good| good.uses_gold_buy_plan()
            && !good.use_pearls && good.stock < 0 && good.panel_type == 0)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NpcShopUiEffect {
    pub preentry_withdraw: bool,
    pub changed: bool,
    pub close: bool,
    pub handoff_sell: bool,
    pub buy: Option<NpcGoldBuyCommand>,
}

#[derive(Debug, Clone)]
pub struct NpcShopUiView {
    pub stamp: Option<NpcShopUiStamp>,
    pub selected_id: Option<u64>,
    pub quantity: u16,
    pub start_index: usize,
    pub gold: u32,
    pub plan: NpcGoldBuyPlan,
    pub can_buy: bool,
    pub can_select: bool,
    pub can_inc: bool,
    pub can_dec: bool,
    pub can_page_up: bool,
    pub can_page_down: bool,
    pub show_sell: bool,
    pub feedback: NpcGoldBuyFeedback,
}

#[derive(Debug, Clone, Resource, PartialEq, Eq)]
pub struct NpcShopUiState {
    pub selected_id: Option<u64>,
    pub quantity: u16,
    pub start_index: usize,
    context: NpcShopUiContext,
    ordinary: bool,
    model_key: Option<String>,
    presentation_revision: u64,
    exhausted: bool,
}
impl Default for NpcShopUiState {
    fn default() -> Self {
        Self { selected_id: None, quantity: 1, start_index: 0,
            context: NpcShopUiContext::default(), ordinary: false, model_key: None,
            presentation_revision: 1, exhausted: false }
    }
}
impl NpcShopUiState {
    /// Also called by the host before applying a layout change. Exhaustion is
    /// terminal; remounting a control tree must not reuse an old stamp.
    pub fn invalidate_presentation(&mut self) -> bool {
        let Some(next) = self.presentation_revision.checked_add(1) else {
            self.exhausted = true; return false;
        };
        self.presentation_revision = next; true
    }
    pub fn stamp(&self) -> Option<NpcShopUiStamp> {
        if self.exhausted || self.model_key.is_none() || !self.ordinary || !self.context.open || !self.context.show_buy { return None; }
        Some(NpcShopUiStamp { source_revision: self.context.source_revision.filter(|r| *r != 0)?,
            presentation_revision: self.presentation_revision })
    }
    pub fn reconcile(&mut self, context: NpcShopUiContext, shop: &ShopModel,
        inventory: &InventoryModel) -> NpcShopUiEffect {
        let ordinary = ordinary_gold_surface(shop, context.show_buy);
        // Reuse the Gate's complete model projection only as a UI stale fence.
        // This key contains no owner and cannot reserve or authorize a purchase.
        let model_key = npc_gold_buy_model_authority(shop, inventory);
        let source_changed = self.context.source_revision != context.source_revision;
        let model_changed = self.model_key != model_key;
        let mut changed = source_changed || model_changed || self.context != context || self.ordinary != ordinary;
        if source_changed || model_changed {
            self.selected_id = None; self.quantity = 1; self.start_index = 0;
        } else {
            let start = self.start_index.min(shop.goods.len().saturating_sub(NPC_SHOP_VISIBLE_ROWS));
            changed |= start != self.start_index; self.start_index = start;
        }
        // Visibility/input/layout affect presentation and pre-entry availability,
        // never the Core's authority or an Entered/Unknown barrier.
        self.context = context; self.ordinary = ordinary; self.model_key = model_key;
        if changed { self.invalidate_presentation(); }
        NpcShopUiEffect { preentry_withdraw: changed, changed, ..Default::default() }
    }
    pub fn view(&self, context: NpcShopUiContext, shop: &ShopModel,
        inventory: &InventoryModel, feedback: NpcGoldBuyFeedback) -> NpcShopUiView {
        let mut selected_shop = shop.clone(); selected_shop.selected_id = self.selected_id;
        let plan = plan_npc_gold_buy(&selected_shop, inventory, self.quantity);
        let current_key = npc_gold_buy_model_authority(shop, inventory);
        let stamp = (context == self.context && current_key.is_some() && current_key == self.model_key
            && ordinary_gold_surface(shop, context.show_buy))
            .then(|| self.stamp()).flatten();
        let interactive = stamp.is_some() && context.input_enabled;
        NpcShopUiView { stamp, selected_id: self.selected_id, quantity: self.quantity,
            start_index: self.start_index, gold: inventory.gold, can_buy: interactive && plan.can_buy && feedback.can_reserve,
            can_select: interactive, can_inc: interactive && self.selected_id.is_some() && self.quantity < plan.max_quantity,
            can_dec: interactive && self.selected_id.is_some() && self.quantity > 1,
            can_page_up: interactive && self.start_index > 0,
            can_page_down: interactive && self.start_index.saturating_add(NPC_SHOP_VISIBLE_ROWS) < shop.goods.len(),
            show_sell: shop.allows_sell(), plan, feedback }
    }
    /// The host has already refreshed context, compared the node stamp/Gate,
    /// and withdrawn pre-entry work for action_retires_preentry actions.
    pub fn apply(&mut self, action: NpcShopUiAction, context: NpcShopUiContext,
        shop: &ShopModel, inventory: &InventoryModel, feedback: NpcGoldBuyFeedback) -> NpcShopUiEffect {
        let view = self.view(context, shop, inventory, feedback);
        if !view.can_select { return NpcShopUiEffect::default(); }
        let before = (self.selected_id, self.quantity, self.start_index);
        let mut effect = NpcShopUiEffect { preentry_withdraw: action_retires_preentry(action), ..Default::default() };
        match action {
            NpcShopUiAction::Select(id) => {
                if !shop.goods.iter().any(|good| good.unique_id == id) { return NpcShopUiEffect::default(); }
                self.selected_id = Some(id); self.quantity = 1;
            }
            NpcShopUiAction::QuantityInc if view.can_inc => self.quantity += 1,
            NpcShopUiAction::QuantityDec if view.can_dec => self.quantity -= 1,
            NpcShopUiAction::PageUp if view.can_page_up => {
                self.start_index -= 1; self.selected_id = None; self.quantity = 1;
            }
            NpcShopUiAction::PageDown if view.can_page_down => {
                self.start_index += 1; self.selected_id = None; self.quantity = 1;
            }
            NpcShopUiAction::Buy if view.can_buy => effect.buy = view.plan.command,
            NpcShopUiAction::Close => {
                self.selected_id = None; self.quantity = 1; self.start_index = 0;
                self.context.open = false; effect.close = true;
            }
            NpcShopUiAction::HandoffSell if view.show_sell => {
                self.selected_id = None; self.quantity = 1; self.start_index = 0;
                self.context.show_buy = false; effect.handoff_sell = true;
            }
            _ => return NpcShopUiEffect::default(),
        }
        effect.changed = before != (self.selected_id, self.quantity, self.start_index) || effect.close || effect.handoff_sell;
        if effect.changed { self.invalidate_presentation(); }
        effect
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{CrystalItemInfoModel, CrystalItemTooltipSourceModel, CrystalUserItemModel};
    use crate::npc_gold_buy_attempt::NpcGoldBuyAttemptPhase;
    use crate::npc_shop_buy::NpcGoldBuyBlockReason;
    use crate::shop::{NpcShopServiceMode, ShopGood};
    fn models(count: usize) -> (ShopModel, InventoryModel) {
        let goods = (0..count).map(|id| ShopGood {
            unique_id: id as u64, price: 1, purchase_rate: Some(1.5),
            requires_gold_buy_plan: true, stock: -1, count: 1,
            tooltip_source: Some(CrystalItemTooltipSourceModel {
                info: CrystalItemInfoModel { item_index: 658, price: 1, stack_size: 2, item_type: 13, ..Default::default() },
                user_item: Some(CrystalUserItemModel { unique_id: id as u64, item_index: 658,
                    count: 1, is_shop_item: true, ..Default::default() }), ..Default::default()
            }), ..Default::default()
        }).collect();
        (ShopModel { goods, service_mode: NpcShopServiceMode::Buy, ..Default::default() },
            InventoryModel { gold: 100, ..Default::default() })
    }
    fn context() -> NpcShopUiContext {
        NpcShopUiContext { source_revision: Some(1), open: true, input_enabled: true, show_buy: true }
    }
    fn ready() -> NpcGoldBuyFeedback { NpcGoldBuyFeedback { can_reserve: true, ..Default::default() } }
    fn select(state: &mut NpcShopUiState, shop: &ShopModel, inventory: &InventoryModel) {
        assert!(state.apply(NpcShopUiAction::Select(0), context(), shop, inventory, ready()).preentry_withdraw);
    }
    #[test]
    fn whole_catalog_handoff_checks_off_page_rows_and_preserves_marked_blocking() {
        let (shop, inventory) = models(9);
        assert!(ordinary_gold_surface(&shop, true));
        assert!(!ordinary_gold_surface(&shop, false));
        assert!(!ordinary_gold_surface(&ShopModel::default(), true));
        for kind in 0..4 {
            let mut legacy = shop.clone();
            match kind {
                0 => legacy.goods[8].use_pearls = true,
                1 => legacy.goods[8].stock = 0,
                2 => legacy.goods[8].panel_type = 1,
                _ => { legacy.goods[8].requires_gold_buy_plan = false;
                    legacy.goods[8].purchase_rate = None; legacy.goods[8].tooltip_source = None; }
            }
            assert!(!ordinary_gold_surface(&legacy, true));
        }
        let mut malformed = shop; malformed.goods[0].tooltip_source = None;
        malformed.goods[0].purchase_rate = None;
        let mut state = NpcShopUiState::default(); state.reconcile(context(), &malformed, &inventory);
        select(&mut state, &malformed, &inventory);
        let view = state.view(context(), &malformed, &inventory, ready());
        assert!(view.stamp.is_some()); assert!(!view.can_buy);
        assert_eq!(view.plan.block_reason, Some(NpcGoldBuyBlockReason::InvalidSource));
    }
    #[test]
    fn controls_consume_raw_planner_quote_and_do_not_reclamp_seeded_quantity() {
        let (shop, inventory) = models(1); let mut state = NpcShopUiState::default();
        state.reconcile(context(), &shop, &inventory); select(&mut state, &shop, &inventory);
        let stamp = state.stamp();
        let inc = state.apply(NpcShopUiAction::QuantityInc, context(), &shop, &inventory, ready());
        assert!(inc.changed && inc.preentry_withdraw); assert_ne!(stamp, state.stamp());
        let view = state.view(context(), &shop, &inventory, ready());
        assert_eq!((view.quantity, view.plan.max_quantity, view.plan.total_gold), (2, 2, Some(3)));
        assert_eq!(view.gold, inventory.gold); assert!(!view.can_inc);
        let buy = state.apply(NpcShopUiAction::Buy, context(), &shop, &inventory, ready());
        assert!(!buy.preentry_withdraw); assert_eq!(buy.buy, view.plan.command);
        assert_eq!(buy.buy.unwrap().item_index, 0);
        state.quantity = 3; state.invalidate_presentation();
        let invalid = state.view(context(), &shop, &inventory, ready());
        assert_eq!(invalid.quantity, 3); assert!(!invalid.can_buy);
        assert_eq!(invalid.plan.block_reason, Some(NpcGoldBuyBlockReason::InvalidQuantity));
    }
    #[test]
    fn view_is_read_only_and_model_or_source_replacement_retires_local_state() {
        let (shop, mut inventory) = models(9); let mut state = NpcShopUiState::default();
        state.reconcile(context(), &shop, &inventory); select(&mut state, &shop, &inventory);
        state.quantity = 2; state.start_index = 1; state.invalidate_presentation();
        let before = state.clone(); let old_stamp = state.stamp(); inventory.gold -= 1;
        assert!(state.view(context(), &shop, &inventory, ready()).stamp.is_none());
        assert_eq!(state, before);
        let change = state.reconcile(context(), &shop, &inventory);
        assert!(change.changed && change.preentry_withdraw);
        assert_eq!((state.selected_id, state.quantity, state.start_index), (None, 1, 0));
        assert_ne!(old_stamp, state.stamp());
        select(&mut state, &shop, &inventory);
        let mut new_context = context(); new_context.source_revision = Some(2);
        assert!(state.reconcile(new_context, &shop, &inventory).preentry_withdraw);
        assert!(state.selected_id.is_none());
        let newer = state.stamp(); state.reconcile(context(), &shop, &inventory);
        assert_ne!(state.stamp(), old_stamp); assert_ne!(state.stamp(), newer);
    }
    #[test]
    fn input_and_visibility_reconcile_do_not_become_purchase_authority_or_reset_locals() {
        let (mut shop, inventory) = models(1); let mut state = NpcShopUiState::default();
        state.reconcile(context(), &shop, &inventory); select(&mut state, &shop, &inventory);
        state.quantity = 2; state.invalidate_presentation();
        // Host compatibility mirroring of selection is excluded from the key.
        shop.selected_id = Some(0);
        assert!(!state.reconcile(context(), &shop, &inventory).changed);
        let mut blocked = context(); blocked.input_enabled = false;
        assert!(state.reconcile(blocked, &shop, &inventory).preentry_withdraw);
        let view = state.view(blocked, &shop, &inventory, ready());
        assert!(view.stamp.is_some()); assert!(!view.can_select && !view.can_buy);
        assert_eq!((state.selected_id, state.quantity), (Some(0), 2));
        let mut closed = blocked; closed.open = false;
        state.reconcile(closed, &shop, &inventory); assert!(state.stamp().is_none());
        state.reconcile(context(), &shop, &inventory);
        assert_eq!((state.selected_id, state.quantity), (Some(0), 2));
    }
    #[test]
    fn single_row_paging_close_and_sell_handoff_emit_preentry_withdraw() {
        let (mut shop, inventory) = models(10); shop.supports_sell = true;
        let mut state = NpcShopUiState::default(); state.reconcile(context(), &shop, &inventory);
        select(&mut state, &shop, &inventory);
        let page = state.apply(NpcShopUiAction::PageDown, context(), &shop, &inventory, ready());
        assert!(page.preentry_withdraw && page.changed);
        assert_eq!((state.start_index, state.selected_id, state.quantity), (1, None, 1));
        state.apply(NpcShopUiAction::PageUp, context(), &shop, &inventory, ready());
        assert_eq!(state.start_index, 0);
        let handoff = state.apply(NpcShopUiAction::HandoffSell, context(), &shop, &inventory, ready());
        assert!(handoff.handoff_sell && handoff.preentry_withdraw); assert!(state.stamp().is_none());
        state.reconcile(context(), &shop, &inventory);
        let close = state.apply(NpcShopUiAction::Close, context(), &shop, &inventory, ready());
        assert!(close.close && close.preentry_withdraw); assert!(state.stamp().is_none());
    }
    #[test]
    fn attempt_feedback_only_disables_buy_and_cannot_release_entry_barriers() {
        let (shop, inventory) = models(1); let mut state = NpcShopUiState::default();
        state.reconcile(context(), &shop, &inventory); select(&mut state, &shop, &inventory);
        for phase in [NpcGoldBuyAttemptPhase::Queued, NpcGoldBuyAttemptPhase::Bound,
            NpcGoldBuyAttemptPhase::Entered, NpcGoldBuyAttemptPhase::Flushed, NpcGoldBuyAttemptPhase::Unknown] {
            let feedback = NpcGoldBuyFeedback { phase: Some(phase), pending: true, can_reserve: false, ..Default::default() };
            let view = state.view(context(), &shop, &inventory, feedback);
            assert!(view.plan.can_buy && view.can_select); assert!(!view.can_buy);
            assert_eq!(view.feedback, feedback);
            assert!(state.apply(NpcShopUiAction::Buy, context(), &shop, &inventory, feedback).buy.is_none());
            assert!(state.apply(NpcShopUiAction::Select(0), context(), &shop, &inventory, feedback).preentry_withdraw);
        }
        assert!(state.view(context(), &shop, &inventory, ready()).can_buy);
    }
    #[test]
    fn checked_presentation_exhaustion_and_zero_source_never_reuse_a_stamp() {
        let (shop, inventory) = models(1); let mut state = NpcShopUiState::default();
        state.reconcile(context(), &shop, &inventory); state.presentation_revision = u64::MAX;
        assert!(!state.invalidate_presentation()); assert!(state.stamp().is_none());
        state.reconcile(context(), &shop, &inventory); assert!(state.stamp().is_none());
        let mut zero = context(); zero.source_revision = Some(0);
        let mut fresh = NpcShopUiState::default(); fresh.reconcile(zero, &shop, &inventory);
        assert!(fresh.view(zero, &shop, &inventory, ready()).stamp.is_none());
        assert!(!action_retires_preentry(NpcShopUiAction::Buy));
        assert!(action_retires_preentry(NpcShopUiAction::QuantityInc));
        assert!(action_retires_preentry(NpcShopUiAction::QuantityDec));
    }
}
