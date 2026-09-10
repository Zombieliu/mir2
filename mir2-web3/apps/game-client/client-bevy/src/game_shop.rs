//! Server-driven Crystal GameShop read model.
//!
//! This is intentionally separate from [`crate::shop::ShopModel`]: NPC shops
//! buy/sell/repair bag items, while GameShop spends account Credit or Gold and
//! delivers purchases through Mail. The client only disables obviously invalid
//! actions; the server remains authoritative for every purchase decision.

use bevy::prelude::Resource;
use mir2_ui_core::game_shop::{next_request_sequence, request_id_for_sequence};
pub use mir2_ui_core::game_shop::{GameShopFailureCode, GameShopReceipt, GameShopRequest};
use serde::{Deserialize, Serialize};

use crate::inventory::CrystalItemTooltipSourceModel;

pub const GAME_SHOP_QUANTITY_MIN: u8 = 1;
pub const GAME_SHOP_QUANTITY_MAX: u8 = 99;
/// Defensive ceiling for server-provided catalog rows. The authoritative
/// Crystal catalog currently contains 105 entries, so this leaves ample room
/// for future additions while preventing an invalid stream from growing the
/// native read model without bound.
pub const MAX_GAME_SHOP_ITEMS: usize = 512;
/// Stock may arrive before its matching catalog row. Keep that race buffer
/// bounded and retain the most recent patches when a malformed stream exceeds
/// the expected catalog size.
pub const MAX_PENDING_STOCK_PATCHES: usize = 512;
/// Maximum number of cash-shop rows rendered in one native page.
///
/// The catalog remains authoritative and untruncated in [`GameShopModel`];
/// this bound only keeps the operable panel finite.
pub const GAME_SHOP_PAGE_SIZE: usize = 24;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GameShopPaymentType {
    Credit,
    #[default]
    Gold,
}

impl GameShopPaymentType {
    pub const fn protocol_value(self) -> i32 {
        match self {
            Self::Credit => 0,
            Self::Gold => 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GameShopEntry {
    #[serde(alias = "item_index")]
    pub item_index: i32,
    #[serde(alias = "gIndex", alias = "g_index", alias = "game_shop_index")]
    pub game_shop_index: i32,
    pub item_name: String,
    pub image: u32,
    pub item_type: u8,
    pub gold_price: u32,
    pub credit_price: u32,
    pub count: u16,
    pub class: String,
    pub category: String,
    /// `0` means unlimited in Crystal.
    pub stock: i32,
    pub stock_level: i32,
    pub deal: bool,
    pub top_item: bool,
    pub date_binary_datetime: i64,
    pub can_buy_credit: bool,
    pub can_buy_gold: bool,
    /// Source-faithful temporary `UserItem` created by Crystal while the
    /// pointer is inside the product image (`MirGameShopCell.OnMouseMove`).
    pub tooltip_source: Option<CrystalItemTooltipSourceModel>,
}

impl Default for GameShopEntry {
    fn default() -> Self {
        Self {
            item_index: 0,
            game_shop_index: 0,
            item_name: String::new(),
            image: 0,
            item_type: 0,
            gold_price: 0,
            credit_price: 0,
            count: 1,
            class: "All".to_owned(),
            category: String::new(),
            stock: 0,
            stock_level: 0,
            deal: false,
            top_item: false,
            date_binary_datetime: 0,
            can_buy_credit: false,
            can_buy_gold: false,
            tooltip_source: None,
        }
    }
}

impl GameShopEntry {
    pub fn unit_price(&self, payment: GameShopPaymentType) -> Option<u32> {
        match payment {
            GameShopPaymentType::Credit if self.can_buy_credit && self.credit_price > 0 => {
                Some(self.credit_price)
            }
            GameShopPaymentType::Gold if self.can_buy_gold && self.gold_price > 0 => {
                Some(self.gold_price)
            }
            _ => None,
        }
    }

    pub fn total_price(&self, payment: GameShopPaymentType, quantity: u8) -> Option<u32> {
        if !(GAME_SHOP_QUANTITY_MIN..=GAME_SHOP_QUANTITY_MAX).contains(&quantity) {
            return None;
        }
        self.unit_price(payment)?.checked_mul(u32::from(quantity))
    }

    pub fn stock_available(&self, quantity: u8) -> bool {
        if !(GAME_SHOP_QUANTITY_MIN..=GAME_SHOP_QUANTITY_MAX).contains(&quantity) {
            return false;
        }
        self.stock == 0 || self.stock_level >= i32::from(quantity)
    }

    pub fn stock_label(&self) -> String {
        if self.stock == 0 {
            "∞".to_owned()
        } else if self.stock_level >= 99 {
            "99+".to_owned()
        } else {
            self.stock_level.max(0).to_string()
        }
    }

    pub fn visible_for_class(&self, player_class: &str) -> bool {
        let product = self.class.trim();
        product.is_empty()
            || product.eq_ignore_ascii_case("all")
            || product.eq_ignore_ascii_case("show all")
            || product.eq_ignore_ascii_case(player_class.trim())
    }
}

/// Wire-neutral representation of one packet-first GameShop stock update.
///
/// The gateway may use either Crystal's snake_case field names or the native
/// camelCase envelope. Keeping this tiny patch type separate from
/// [`GameShopEntry`] prevents a stock event from replacing catalog metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameShopStockPatch {
    #[serde(alias = "gIndex", alias = "g_index", alias = "game_shop_index")]
    pub game_shop_index: i32,
    #[serde(alias = "stock_level")]
    pub stock_level: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Resource, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GameShopModel {
    pub items: Vec<GameShopEntry>,
    /// Stock patches can legally race the catalog packet on a native
    /// transport. Retain an unknown patch and apply it when its GameShopInfo
    /// arrives instead of dropping authoritative data.
    #[serde(default)]
    pub pending_stock_patches: Vec<GameShopStockPatch>,
    pub selected_game_shop_index: Option<i32>,
    pub quantity: u8,
    pub payment: GameShopPaymentType,
    #[serde(default = "default_next_request_id")]
    pub next_request_id: u64,
    #[serde(default)]
    pub pending_purchase: Option<GameShopRequest>,
    #[serde(default)]
    pub last_receipt: Option<GameShopReceipt>,
    #[serde(default)]
    pub purchase_unknown: bool,
}

fn default_next_request_id() -> u64 {
    1
}

impl Default for GameShopModel {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            pending_stock_patches: Vec::new(),
            selected_game_shop_index: None,
            quantity: GAME_SHOP_QUANTITY_MIN,
            payment: GameShopPaymentType::Gold,
            next_request_id: 1,
            pending_purchase: None,
            last_receipt: None,
            purchase_unknown: false,
        }
    }
}

impl GameShopModel {
    /// Reserve one native purchase request. The reservation is local only;
    /// the server still validates and commits the transaction.
    pub fn begin_purchase(
        &mut self,
        g_index: i32,
        quantity: u8,
        price_type: i32,
    ) -> Option<GameShopRequest> {
        if self.pending_purchase.is_some() || self.next_request_id == 0 {
            return None;
        }
        let request = GameShopRequest::new(
            request_id_for_sequence(self.next_request_id),
            g_index,
            quantity,
            price_type,
        )?;
        self.next_request_id = next_request_sequence(self.next_request_id);
        self.pending_purchase = Some(request.clone());
        self.purchase_unknown = false;
        Some(request)
    }

    /// Mirror a request reserved by the shared UiState. The request id must
    /// be the next id this model would have generated; this prevents the
    /// native overlay and shared reducer from silently developing different
    /// correlation sequences.
    pub fn reserve_purchase(&mut self, request: GameShopRequest) -> bool {
        if self.pending_purchase.is_some()
            || self.next_request_id == 0
            || request.request_id != request_id_for_sequence(self.next_request_id)
            || !request.is_valid()
        {
            return false;
        }
        self.next_request_id = next_request_sequence(self.next_request_id);
        self.pending_purchase = Some(request);
        self.purchase_unknown = false;
        true
    }

    pub fn cancel_purchase_reservation(&mut self, request_id: &str) -> bool {
        if self
            .pending_purchase
            .as_ref()
            .is_some_and(|request| request.request_id == request_id)
        {
            self.pending_purchase = None;
            return true;
        }
        false
    }

    /// Apply only a valid, exact receipt. Wallet/chat/mail/catalog updates
    /// never call this method and therefore cannot release the purchase.
    pub fn apply_receipt(&mut self, receipt: GameShopReceipt) -> bool {
        if !receipt.is_valid() {
            return false;
        }
        let Some(request) = self.pending_purchase.as_ref() else {
            return false;
        };
        if !receipt.matches_request(request) {
            return false;
        }
        if let Some(stock) = receipt.new_stock_level {
            let _ = self.update_stock(receipt.g_index, stock);
        }
        self.last_receipt = Some(receipt);
        self.pending_purchase = None;
        self.purchase_unknown = false;
        true
    }

    pub fn mark_purchase_unknown(&mut self) {
        if self.pending_purchase.take().is_some() {
            self.purchase_unknown = true;
        }
    }

    pub fn normalize(&mut self) {
        self.quantity = self
            .quantity
            .clamp(GAME_SHOP_QUANTITY_MIN, GAME_SHOP_QUANTITY_MAX);
        if self
            .selected_game_shop_index
            .is_some_and(|index| !self.items.iter().any(|item| item.game_shop_index == index))
        {
            self.selected_game_shop_index = None;
        }
        self.items.sort_by(|left, right| {
            left.game_shop_index.cmp(&right.game_shop_index).then(
                left.item_name
                    .to_ascii_lowercase()
                    .cmp(&right.item_name.to_ascii_lowercase()),
            )
        });
    }

    pub fn selected(&self) -> Option<&GameShopEntry> {
        let index = self.selected_game_shop_index?;
        self.items.iter().find(|item| item.game_shop_index == index)
    }

    pub fn upsert(&mut self, entry: GameShopEntry) {
        let pending_stock = self
            .pending_stock_patches
            .iter()
            .position(|patch| patch.game_shop_index == entry.game_shop_index)
            .map(|index| self.pending_stock_patches.remove(index));
        let mut entry = entry;
        if let Some(patch) = pending_stock {
            entry.stock_level = patch.stock_level.max(0);
        }
        if let Some(existing) = self
            .items
            .iter_mut()
            .find(|item| item.game_shop_index == entry.game_shop_index)
        {
            *existing = entry;
        } else if self.items.len() < MAX_GAME_SHOP_ITEMS {
            self.items.push(entry);
        }
        self.normalize();
    }

    pub fn update_stock(&mut self, game_shop_index: i32, stock_level: i32) -> bool {
        let Some(entry) = self
            .items
            .iter_mut()
            .find(|item| item.game_shop_index == game_shop_index)
        else {
            return false;
        };
        entry.stock_level = stock_level.max(0);
        true
    }

    /// Apply a server stock patch without changing the authoritative product
    /// metadata or the current selection. Unknown products are rejected so a
    /// malformed packet cannot manufacture a purchasable entry.
    pub fn apply_stock_patch(&mut self, game_shop_index: i32, stock_level: i32) -> bool {
        if self.update_stock(game_shop_index, stock_level) {
            return true;
        }
        if let Some(existing) = self
            .pending_stock_patches
            .iter_mut()
            .find(|patch| patch.game_shop_index == game_shop_index)
        {
            existing.stock_level = stock_level;
        } else {
            if self.pending_stock_patches.len() >= MAX_PENDING_STOCK_PATCHES {
                self.pending_stock_patches.remove(0);
            }
            self.pending_stock_patches.push(GameShopStockPatch {
                game_shop_index,
                stock_level,
            });
        }
        false
    }

    pub fn apply_stock_patch_value(&mut self, patch: GameShopStockPatch) -> bool {
        self.apply_stock_patch(patch.game_shop_index, patch.stock_level)
    }

    pub fn set_quantity(&mut self, quantity: u8) {
        self.quantity = quantity.clamp(GAME_SHOP_QUANTITY_MIN, GAME_SHOP_QUANTITY_MAX);
    }

    pub fn quantity_inc(&mut self) {
        self.set_quantity(self.quantity.saturating_add(1));
    }

    pub fn quantity_dec(&mut self) {
        self.set_quantity(self.quantity.saturating_sub(1));
    }

    pub fn buy_enabled(&self, gold: u32, credit: u32, player_class: &str) -> bool {
        self.buy_disabled_reason(gold, credit, player_class)
            .is_none()
    }

    pub fn buy_disabled_reason(
        &self,
        gold: u32,
        credit: u32,
        player_class: &str,
    ) -> Option<&'static str> {
        if self.pending_purchase.is_some() {
            return Some("purchase pending");
        }
        let Some(entry) = self.selected() else {
            return Some("select a product");
        };
        if !entry.visible_for_class(player_class) {
            return Some("class restricted");
        }
        if !entry.stock_available(self.quantity) {
            return Some("out of stock");
        }
        let Some(price) = entry.total_price(self.payment, self.quantity) else {
            return Some("payment disabled");
        };
        let balance = match self.payment {
            GameShopPaymentType::Credit => credit,
            GameShopPaymentType::Gold => gold,
        };
        (balance < price).then_some("insufficient balance")
    }

    pub fn clear_session(&mut self) {
        let had_pending_purchase = self.pending_purchase.is_some();
        self.items.clear();
        self.pending_stock_patches.clear();
        self.selected_game_shop_index = None;
        self.quantity = GAME_SHOP_QUANTITY_MIN;
        self.payment = GameShopPaymentType::Gold;
        self.next_request_id = 1;
        self.pending_purchase = None;
        self.last_receipt = None;
        self.purchase_unknown = had_pending_purchase;
    }

    /// Clear account/session catalog state while retaining only the exact
    /// request needed to consume a receipt already accepted by the transport.
    pub fn clear_session_preserving_exact_receipt(&mut self, receipt: &GameShopReceipt) -> bool {
        if !receipt.is_valid() {
            return false;
        }
        let Some(request) = GameShopRequest::new(
            receipt.request_id.clone(),
            receipt.g_index,
            receipt.quantity,
            receipt.price_type,
        ) else {
            return false;
        };
        self.items.clear();
        self.pending_stock_patches.clear();
        self.selected_game_shop_index = None;
        self.quantity = GAME_SHOP_QUANTITY_MIN;
        self.payment = GameShopPaymentType::Gold;
        self.next_request_id = 1;
        self.pending_purchase = Some(request);
        self.last_receipt = None;
        self.purchase_unknown = false;
        true
    }
}

/// Return the number of bounded pages needed for a catalog.
pub fn game_shop_page_count(item_count: usize) -> usize {
    item_count
        .saturating_add(GAME_SHOP_PAGE_SIZE.saturating_sub(1))
        .checked_div(GAME_SHOP_PAGE_SIZE)
        .unwrap_or(0)
        .max(1)
}

/// Find the page containing an authoritative GameShop index.
pub fn game_shop_page_for_index(model: &GameShopModel, game_shop_index: i32) -> Option<usize> {
    model
        .items
        .iter()
        .position(|entry| entry.game_shop_index == game_shop_index)
        .map(|position| position / GAME_SHOP_PAGE_SIZE)
}

/// Return one bounded, sorted catalog page. An empty model yields an empty
/// slice; callers can still render the single empty page from
/// [`game_shop_page_count`].
pub fn game_shop_page_entries(model: &GameShopModel, page: usize) -> &[GameShopEntry] {
    let page = page.min(game_shop_page_count(model.items.len()).saturating_sub(1));
    let start = page.saturating_mul(GAME_SHOP_PAGE_SIZE);
    let end = (start + GAME_SHOP_PAGE_SIZE).min(model.items.len());
    &model.items[start.min(end)..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(index: i32) -> GameShopEntry {
        GameShopEntry {
            item_index: 1288,
            game_shop_index: index,
            item_name: format!("Item {index}"),
            gold_price: 100,
            credit_price: 10,
            count: 5,
            can_buy_gold: true,
            can_buy_credit: true,
            ..Default::default()
        }
    }

    #[test]
    fn upsert_is_keyed_by_authoritative_game_shop_index() {
        let mut model = GameShopModel::default();
        model.upsert(entry(31));
        let mut changed = entry(31);
        changed.gold_price = 250;
        model.upsert(changed);
        assert_eq!(model.items.len(), 1);
        assert_eq!(model.items[0].gold_price, 250);
    }

    #[test]
    fn purchase_guard_checks_payment_stock_class_balance_and_overflow() {
        let mut model = GameShopModel::default();
        model.upsert(entry(31));
        model.selected_game_shop_index = Some(31);
        model.set_quantity(2);
        assert!(model.buy_enabled(200, 0, "Warrior"));
        assert!(!model.buy_enabled(199, 0, "Warrior"));

        model.payment = GameShopPaymentType::Credit;
        assert!(model.buy_enabled(0, 20, "Warrior"));
        assert!(!model.buy_enabled(0, 19, "Warrior"));

        model.items[0].class = "Wizard".to_owned();
        assert!(!model.buy_enabled(0, 20, "Warrior"));
        assert!(model.buy_enabled(0, 20, "Wizard"));

        model.items[0].stock = 10;
        model.items[0].stock_level = 1;
        assert!(!model.buy_enabled(0, 20, "Wizard"));

        model.items[0].credit_price = u32::MAX;
        model.items[0].stock = 0;
        assert!(!model.buy_enabled(0, u32::MAX, "Wizard"));
    }

    #[test]
    fn quantity_and_stock_follow_crystal_bounds() {
        let mut model = GameShopModel::default();
        model.set_quantity(0);
        assert_eq!(model.quantity, 1);
        model.set_quantity(255);
        assert_eq!(model.quantity, 99);
        model.quantity_inc();
        assert_eq!(model.quantity, 99);

        let mut unlimited = entry(1);
        assert!(unlimited.stock_available(99));
        assert_eq!(unlimited.stock_label(), "∞");
        unlimited.stock = 100;
        unlimited.stock_level = 120;
        assert_eq!(unlimited.stock_label(), "99+");
        assert!(unlimited.stock_available(99));
        unlimited.stock_level = 3;
        assert!(!unlimited.stock_available(4));
    }

    #[test]
    fn session_clear_removes_server_catalog_and_transient_selection() {
        let mut model = GameShopModel::default();
        model.upsert(entry(31));
        model.selected_game_shop_index = Some(31);
        model.quantity = 8;
        model.payment = GameShopPaymentType::Credit;
        model.clear_session();
        assert!(model.items.is_empty());
        assert_eq!(model.selected_game_shop_index, None);
        assert_eq!(model.quantity, 1);
        assert_eq!(model.payment, GameShopPaymentType::Gold);
    }

    #[test]
    fn one_hundred_five_products_accumulate_in_gindex_order_and_duplicate_upsert() {
        let mut model = GameShopModel::default();
        assert!(!model.apply_stock_patch(104, 7));
        assert_eq!(model.pending_stock_patches.len(), 1);
        for index in (0..105).rev() {
            model.upsert(entry(index));
        }
        assert_eq!(model.items.len(), 105);
        assert_eq!(
            model.items.first().map(|item| item.game_shop_index),
            Some(0)
        );
        assert_eq!(
            model.items.last().map(|item| item.game_shop_index),
            Some(104)
        );
        let mut replacement = entry(42);
        replacement.item_name = "Authoritative replacement".to_owned();
        replacement.stock = 4;
        replacement.stock_level = 2;
        model.upsert(replacement);
        assert_eq!(model.items.len(), 105);
        assert_eq!(model.items[42].item_name, "Authoritative replacement");
        assert_eq!(model.items[104].stock_level, 7);
        assert!(model.apply_stock_patch(42, 1));
        assert_eq!(model.items[42].stock_level, 1);
        assert!(!model.apply_stock_patch(999, 1));
    }

    #[test]
    fn disabled_reason_distinguishes_currency_stock_and_selection() {
        let mut model = GameShopModel::default();
        assert_eq!(
            model.buy_disabled_reason(0, 0, "Warrior"),
            Some("select a product")
        );
        model.upsert(entry(1));
        model.selected_game_shop_index = Some(1);
        model.payment = GameShopPaymentType::Credit;
        model.set_quantity(2);
        assert_eq!(
            model.buy_disabled_reason(0, 19, "Warrior"),
            Some("insufficient balance")
        );
        model.items[0].stock = 1;
        model.items[0].stock_level = 1;
        assert_eq!(
            model.buy_disabled_reason(0, 20, "Warrior"),
            Some("out of stock")
        );
        model.items[0].stock = 0;
        model.items[0].can_buy_credit = false;
        assert_eq!(
            model.buy_disabled_reason(0, 20, "Warrior"),
            Some("payment disabled")
        );
    }

    #[test]
    fn bounded_pages_keep_products_after_the_first_24_selectable() {
        let mut model = GameShopModel::default();
        for index in 0..105 {
            model.upsert(entry(index));
        }

        assert_eq!(game_shop_page_count(model.items.len()), 5);
        let page_for_25 = game_shop_page_for_index(&model, 25).expect("product 25 page");
        assert_eq!(page_for_25, 1);
        let second_page = game_shop_page_entries(&model, page_for_25);
        assert_eq!(second_page.len(), GAME_SHOP_PAGE_SIZE);
        assert!(second_page.iter().any(|entry| entry.game_shop_index == 25));

        let last_page =
            game_shop_page_entries(&model, game_shop_page_for_index(&model, 104).unwrap());
        assert_eq!(last_page.len(), 9);
        assert!(last_page.iter().any(|entry| entry.game_shop_index == 104));
    }

    #[test]
    fn malformed_catalog_and_early_stock_streams_remain_bounded() {
        let mut model = GameShopModel::default();
        for index in 0..(MAX_GAME_SHOP_ITEMS as i32 + 50) {
            model.upsert(entry(index));
        }
        assert_eq!(model.items.len(), MAX_GAME_SHOP_ITEMS);

        for index in 10_000..(10_000 + MAX_PENDING_STOCK_PATCHES as i32 + 50) {
            assert!(!model.apply_stock_patch(index, index));
        }
        assert_eq!(model.pending_stock_patches.len(), MAX_PENDING_STOCK_PATCHES);
        assert_eq!(
            model
                .pending_stock_patches
                .first()
                .map(|patch| patch.game_shop_index),
            Some(10_050)
        );
    }

    #[test]
    fn receipt_exact_match_releases_pending_and_late_receipt_does_not() {
        let mut model = GameShopModel::default();
        let request = model.begin_purchase(31, 2, 1).unwrap();
        let wrong = GameShopReceipt {
            protocol: "nativeGameShopReceiptV1".into(),
            request_id: "gs-other".into(),
            success: true,
            g_index: 31,
            quantity: 2,
            price_type: 1,
            new_stock_level: None,
            mail_id: None,
            code: None,
        };
        assert!(!model.apply_receipt(wrong));
        let receipt = GameShopReceipt {
            protocol: "nativeGameShopReceiptV1".into(),
            request_id: request.request_id,
            success: true,
            g_index: 31,
            quantity: 2,
            price_type: 1,
            new_stock_level: Some(3),
            mail_id: Some(1842),
            code: None,
        };
        assert!(model.apply_receipt(receipt.clone()));
        assert!(!model.apply_receipt(receipt));
        assert_eq!(model.last_receipt.and_then(|r| r.mail_id), Some(1842));
    }

    #[test]
    fn terminal_session_reset_marks_lost_purchase_unknown_without_replay() {
        let mut model = GameShopModel::default();
        assert!(model.begin_purchase(31, 1, 1).is_some());
        model.clear_session();
        assert!(model.pending_purchase.is_none());
        assert!(model.purchase_unknown);
    }

    #[test]
    fn request_sequence_exhaustion_fails_closed_instead_of_repeating() {
        let mut model = GameShopModel::default();
        model.next_request_id = u64::MAX;
        let request = model
            .begin_purchase(31, 1, 1)
            .expect("last id is usable once");
        assert_eq!(request.request_id, "gs-18446744073709551615");
        assert_eq!(model.next_request_id, 0);
        let receipt = GameShopReceipt {
            protocol: "nativeGameShopReceiptV1".into(),
            request_id: request.request_id,
            success: true,
            g_index: 31,
            quantity: 1,
            price_type: 1,
            new_stock_level: None,
            mail_id: Some(1842),
            code: None,
        };
        assert!(model.apply_receipt(receipt));
        assert!(model.begin_purchase(31, 1, 1).is_none());
    }

    #[test]
    fn shared_ui_and_model_mirror_the_same_request_exactly() {
        let mut ui = mir2_ui_core::state::UiState::default();
        let request = ui.begin_game_shop_purchase(31, 2, 1).unwrap();
        let mut model = GameShopModel::default();
        assert!(model.reserve_purchase(request.clone()));
        assert_eq!(model.pending_purchase, Some(request.clone()));
        assert_eq!(ui.game_shop_pending, Some(request.clone()));

        let wrong = GameShopReceipt {
            protocol: "nativeGameShopReceiptV1".into(),
            request_id: request.request_id.clone(),
            success: true,
            g_index: 31,
            quantity: 1,
            price_type: 1,
            new_stock_level: None,
            mail_id: None,
            code: None,
        };
        assert!(!model.apply_receipt(wrong.clone()));
        assert!(!ui.apply_game_shop_receipt(wrong));
        assert!(model.pending_purchase.is_some());
        assert!(ui.game_shop_pending.is_some());

        let receipt = GameShopReceipt {
            protocol: "nativeGameShopReceiptV1".into(),
            request_id: request.request_id,
            success: true,
            g_index: 31,
            quantity: 2,
            price_type: 1,
            new_stock_level: Some(4),
            mail_id: Some(7),
            code: None,
        };
        assert!(model.apply_receipt(receipt.clone()));
        assert!(ui.apply_game_shop_receipt(receipt));
        assert!(model.pending_purchase.is_none());
        assert!(ui.game_shop_pending.is_none());
    }
}
