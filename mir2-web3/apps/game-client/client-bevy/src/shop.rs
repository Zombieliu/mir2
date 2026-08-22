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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShopGood {
    pub unique_id: u64,
    pub name: String,
    pub price: u32,
    pub count: u16,
    pub stock: i32,
    pub panel_type: u8,
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

#[derive(Debug, Clone, Default, Resource, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShopModel {
    pub goods: Vec<ShopGood>,
    pub selected_id: Option<u64>,
    /// NPC sell selection is independent from Warehouse deposit selection.
    pub selected_bag_slot_for_sell: Option<u32>,
    /// NPC repair selection is independent from sell and Warehouse state.
    pub selected_bag_slot_for_repair: Option<u32>,
}

impl ShopModel {
    pub fn selected(&self) -> Option<&ShopGood> {
        self.selected_id
            .and_then(|id| self.goods.iter().find(|g| g.unique_id == id))
    }

    pub fn find_mut(&mut self, id: u64) -> Option<&mut ShopGood> {
        self.goods.iter_mut().find(|g| g.unique_id == id)
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
        };
        let json = serde_json::to_string(&model).expect("ser");
        let restored: ShopModel = serde_json::from_str(&json).expect("de");
        assert_eq!(model, restored);
    }
}
