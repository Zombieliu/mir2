//! Authoritative shop model shared by every host.

//! Mirrors the NPC goods (`S.NPCGoods`) offered by the server so the Windows
//! Shop panel shows server-priced goods with correct Buy/Sell disabled states.

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::inventory::InventoryModel;

pub const SHOP_QUANTITY_MIN: u16 = 1;
pub const SHOP_QUANTITY_MAX: u16 = 99;
pub const SHOP_QUANTITY_STEP: u16 = 1;
pub const BAG_SLOTS: u32 = 46;

/// Authoritative Crystal NPC service currently offered by the NPC dialog.
/// Opening one service must not implicitly authorize any of the others.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NpcShopServiceMode {
    #[default]
    Closed,
    Buy,
    Sell,
    Repair,
    SpecialRepair,
}

/// Packet-first service transition delivered independently from `NPCGoods`.
/// `repair_rate` is present only for the two Crystal repair services.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NpcShopServiceSignal {
    pub mode: NpcShopServiceMode,
    pub repair_rate: Option<f32>,
}

impl NpcShopServiceSignal {
    pub fn is_valid(self) -> bool {
        match self.mode {
            NpcShopServiceMode::Closed | NpcShopServiceMode::Buy | NpcShopServiceMode::Sell => {
                self.repair_rate.is_none()
            }
            NpcShopServiceMode::Repair | NpcShopServiceMode::SpecialRepair => self
                .repair_rate
                .is_some_and(|rate| rate.is_finite() && rate >= 0.0),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShopGood {
    pub unique_id: u64,
    pub name: String,
    pub price: u32,
    pub count: u16,
    pub stock: i32,
    pub panel_type: u8,
    /// Uses the same Crystal Items atlas exported for carried items.
    pub icon: u16,
    pub description: String,
}

impl ShopGood {
    pub fn stock_label(&self) -> String {
        if self.stock < 0 {
            "∞".to_owned()
        } else {
            self.stock.to_string()
        }
    }
}

#[derive(Debug, Clone, Default, Resource, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShopModel {
    pub goods: Vec<ShopGood>,
    pub selected_id: Option<u64>,
    /// NPC sell selection is independent from Warehouse deposit selection.
    pub selected_bag_slot_for_sell: Option<u32>,
    /// NPC repair selection is independent from sell and Warehouse state.
    pub selected_bag_slot_for_repair: Option<u32>,
    /// Server-selected service surface. This is reset at every session/data
    /// boundary and changed only by NPCGoods/NPCSell/NPCRepair/NPCSRepair.
    pub service_mode: NpcShopServiceMode,
    /// Authoritative capabilities of the current NPC service session. Crystal
    /// can send NPCGoods followed by NPCSell for one BUYSELL NPC; keeping the
    /// capabilities separate from `service_mode` prevents the latter packet
    /// from erasing the former. These fields are additive so older JSON that
    /// only contains `serviceMode` remains valid through `allows_*` fallbacks.
    pub supports_buy: bool,
    pub supports_sell: bool,
    /// Crystal repair multiplier supplied by NPCRepair/NPCSRepair.
    pub repair_rate: Option<f32>,
}

impl ShopModel {
    pub fn selected(&self) -> Option<&ShopGood> {
        self.selected_id
            .and_then(|id| self.goods.iter().find(|g| g.unique_id == id))
    }

    pub fn find_mut(&mut self, id: u64) -> Option<&mut ShopGood> {
        self.goods.iter_mut().find(|g| g.unique_id == id)
    }

    pub fn apply_service_signal(&mut self, signal: NpcShopServiceSignal) -> bool {
        if !signal.is_valid() {
            return false;
        }
        match signal.mode {
            NpcShopServiceMode::Closed => {
                self.supports_buy = false;
                self.supports_sell = false;
            }
            NpcShopServiceMode::Buy => {
                self.supports_buy = true;
                self.supports_sell = false;
            }
            NpcShopServiceMode::Sell => {
                // NPCGoods -> NPCSell is the two-packet representation of a
                // single BUYSELL service session. A standalone NPCSell still
                // remains sell-only because a fresh/default model has no buy
                // capability to preserve.
                self.supports_sell = true;
                self.supports_buy =
                    self.service_mode == NpcShopServiceMode::Buy || self.supports_buy;
            }
            NpcShopServiceMode::Repair | NpcShopServiceMode::SpecialRepair => {
                // Repair services are deliberately fail-closed: neither
                // trade capability may leak from an earlier shop session.
                self.supports_buy = false;
                self.supports_sell = false;
            }
        }
        self.service_mode = signal.mode;
        self.repair_rate = signal.repair_rate;
        self.selected_id = None;
        self.selected_bag_slot_for_sell = None;
        self.selected_bag_slot_for_repair = None;
        true
    }

    pub fn allows_buy(&self) -> bool {
        self.supports_buy || self.service_mode == NpcShopServiceMode::Buy
    }

    pub fn allows_sell(&self) -> bool {
        self.supports_sell || self.service_mode == NpcShopServiceMode::Sell
    }

    pub fn allows_repair(&self) -> bool {
        self.service_mode == NpcShopServiceMode::Repair
    }

    pub fn allows_special_repair(&self) -> bool {
        self.service_mode == NpcShopServiceMode::SpecialRepair
    }
}

pub fn shop_quantity_clamped(q: u16) -> u16 {
    q.clamp(SHOP_QUANTITY_MIN, SHOP_QUANTITY_MAX)
}

pub fn shop_quantity_inc(q: u16) -> u16 {
    shop_quantity_clamped(q.saturating_add(SHOP_QUANTITY_STEP))
}

pub fn shop_quantity_dec(q: u16) -> u16 {
    shop_quantity_clamped(q.saturating_sub(SHOP_QUANTITY_STEP))
}

pub fn shop_buy_enabled(shop: &ShopModel, inventory: &InventoryModel, quantity: u16) -> bool {
    let Some(good) = shop.selected() else {
        return false;
    };
    let qty = shop_quantity_clamped(quantity) as u32;
    if qty == 0 {
        return false;
    }
    if good.stock >= 0 && (good.stock as u32) < qty {
        return false;
    }
    let total_price = good.price.saturating_mul(qty);
    if inventory.gold < total_price {
        return false;
    }
    let occupied = inventory.items.iter().filter(|i| i.container == 0).count() as u32;
    if occupied >= BAG_SLOTS {
        return false;
    }
    true
}

pub fn shop_sell_enabled(inventory: &InventoryModel, slot: Option<u32>) -> bool {
    let Some(slot) = slot else { return false };
    inventory
        .items
        .iter()
        .any(|i| i.container == 0 && i.slot == slot)
}

pub fn shop_repair_enabled(inventory: &InventoryModel, slot: Option<u32>) -> bool {
    let Some(slot) = slot else { return false };
    inventory
        .items
        .iter()
        .any(|i| (i.container == 0 || i.container == 2) && i.slot == slot)
}

pub fn shop_repair_all_enabled(inventory: &InventoryModel) -> bool {
    !inventory.items.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{InventoryModel, ItemModel};

    fn good(id: u64, price: u32, stock: i32) -> ShopGood {
        ShopGood {
            unique_id: id,
            name: format!("Item{id}"),
            price,
            count: 1,
            stock,
            panel_type: 0,
            ..Default::default()
        }
    }

    fn item(slot: u32) -> ItemModel {
        ItemModel {
            unique_id: Some(u64::from(slot) + 1),
            key: "k".to_owned(),
            name: "n".to_owned(),
            quantity: 1,
            slot,
            container: 0,
            ..ItemModel::default()
        }
    }

    #[test]
    fn quantity_clamping() {
        assert_eq!(shop_quantity_clamped(0), 1);
        assert_eq!(shop_quantity_clamped(200), 99);
        assert_eq!(shop_quantity_inc(99), 99);
        assert_eq!(shop_quantity_dec(1), 1);
    }

    #[test]
    fn buy_disabled_when_not_enough_gold_or_stock() {
        let mut shop = ShopModel::default();
        shop.goods.push(good(1, 100, 1));
        shop.selected_id = Some(1);
        let mut inv = InventoryModel {
            gold: 50,
            items: vec![],
        };
        assert!(!shop_buy_enabled(&shop, &inv, 1));
        inv.gold = 500;
        assert!(shop_buy_enabled(&shop, &inv, 1));
        assert!(!shop_buy_enabled(&shop, &inv, 2)); // not enough stock
        shop.goods[0].stock = -1;
        assert!(shop_buy_enabled(&shop, &inv, 2));
    }

    #[test]
    fn sell_requires_bag_item() {
        let inv = InventoryModel {
            gold: 0,
            items: vec![item(3)],
        };
        assert!(shop_sell_enabled(&inv, Some(3)));
        assert!(!shop_sell_enabled(&inv, Some(4)));
        assert!(!shop_sell_enabled(&inv, None));
    }

    #[test]
    fn serde_roundtrip() {
        let model = ShopModel {
            goods: vec![good(1, 10, -1)],
            selected_id: Some(1),
            selected_bag_slot_for_sell: Some(2),
            selected_bag_slot_for_repair: Some(3),
            service_mode: NpcShopServiceMode::Buy,
            supports_buy: true,
            supports_sell: false,
            repair_rate: None,
        };
        let json = serde_json::to_string(&model).expect("ser");
        let restored: ShopModel = serde_json::from_str(&json).expect("de");
        assert_eq!(model, restored);
    }

    #[test]
    fn legacy_service_mode_json_keeps_default_capability_fallbacks() {
        let mut legacy = serde_json::to_value(ShopModel {
            service_mode: NpcShopServiceMode::Buy,
            ..Default::default()
        })
        .expect("legacy source model serializes");
        let object = legacy.as_object_mut().expect("shop model object");
        object.remove("supportsBuy");
        object.remove("supportsSell");

        let restored: ShopModel = serde_json::from_value(legacy).expect("legacy shop model");
        assert!(!restored.supports_buy);
        assert!(!restored.supports_sell);
        assert!(restored.allows_buy());
        assert!(!restored.allows_sell());
    }

    #[test]
    fn service_modes_are_authoritative_and_buy_sell_can_be_combined() {
        let mut model = ShopModel {
            selected_id: Some(7),
            selected_bag_slot_for_sell: Some(2),
            selected_bag_slot_for_repair: Some(3),
            ..Default::default()
        };
        assert!(model.apply_service_signal(NpcShopServiceSignal {
            mode: NpcShopServiceMode::Repair,
            repair_rate: Some(1.5),
        }));
        assert!(model.allows_repair());
        assert!(!model.allows_buy());
        assert!(!model.allows_sell());
        assert!(!model.allows_special_repair());
        assert_eq!(model.repair_rate, Some(1.5));
        assert_eq!(model.selected_id, None);
        assert_eq!(model.selected_bag_slot_for_sell, None);
        assert_eq!(model.selected_bag_slot_for_repair, None);

        // A standalone sell packet is sell-only.
        assert!(model.apply_service_signal(NpcShopServiceSignal {
            mode: NpcShopServiceMode::Sell,
            repair_rate: None,
        }));
        assert!(!model.allows_buy());
        assert!(model.allows_sell());

        // The normal BUYSELL sequence retains both capabilities.
        assert!(model.apply_service_signal(NpcShopServiceSignal {
            mode: NpcShopServiceMode::Buy,
            repair_rate: None,
        }));
        assert!(model.allows_buy());
        assert!(!model.allows_sell());
        assert!(model.apply_service_signal(NpcShopServiceSignal {
            mode: NpcShopServiceMode::Sell,
            repair_rate: None,
        }));
        assert!(model.allows_buy());
        assert!(model.allows_sell());

        // A repair transition closes both trade capabilities, even after the
        // combined session, and keeps the two repair modes exact.
        assert!(model.apply_service_signal(NpcShopServiceSignal {
            mode: NpcShopServiceMode::SpecialRepair,
            repair_rate: Some(2.0),
        }));
        assert!(!model.allows_buy());
        assert!(!model.allows_sell());
        assert!(model.allows_special_repair());
    }

    #[test]
    fn malformed_repair_signal_fails_closed() {
        let mut model = ShopModel::default();
        assert!(!model.apply_service_signal(NpcShopServiceSignal {
            mode: NpcShopServiceMode::SpecialRepair,
            repair_rate: None,
        }));
        assert_eq!(model.service_mode, NpcShopServiceMode::Closed);
    }
}
