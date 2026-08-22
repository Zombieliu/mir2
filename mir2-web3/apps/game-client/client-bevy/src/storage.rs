//! Authoritative storage / warehouse model.

//! Mirrors `WorldSnapshot.storage*` so the Windows Warehouse panel shows the
//! server's storage items, size and password state.

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::inventory::{InventoryModel, ItemModel};

pub const STORAGE_BASE_SIZE: u16 = 30;
pub const STORAGE_EXPANDED_SIZE: u16 = 42;
pub const STORAGE_EXPAND_COST: u32 = 1_000_000;
pub const BAG_SLOTS: u32 = 46;

#[derive(Debug, Clone, Default, Resource, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageModel {
    pub items: Vec<ItemModel>,
    pub size: u16,
    pub has_password: bool,
    pub unlocked: bool,
    pub has_expanded: bool,
    pub expiry: i64,
    pub selected_bag_slot: Option<u32>,
    pub selected_storage_slot: Option<u32>,
    pub password_draft: String,
    pub new_password_draft: String,
    pub confirm_password_draft: String,
}

impl StorageModel {
    pub fn new() -> Self {
        Self {
            size: STORAGE_BASE_SIZE,
            has_password: false,
            unlocked: true,
            has_expanded: false,
            expiry: 0,
            ..Default::default()
        }
    }

    pub fn storage_occupied(&self) -> usize {
        self.items.iter().filter(|i| i.container == 4).count()
    }

    pub fn free_storage_slots(&self) -> u16 {
        self.size.saturating_sub(self.storage_occupied() as u16)
    }

    pub fn item_in_storage(&self, slot: u32) -> Option<&ItemModel> {
        self.items
            .iter()
            .find(|i| i.container == 4 && i.slot == slot)
    }
}

pub fn storage_deposit_enabled(storage: &StorageModel, inventory: &InventoryModel) -> bool {
    if storage.has_password && !storage.unlocked {
        return false;
    }
    let Some(slot) = storage.selected_bag_slot else {
        return false;
    };
    if !inventory
        .items
        .iter()
        .any(|i| i.container == 0 && i.slot == slot)
    {
        return false;
    }
    if storage.free_storage_slots() == 0 {
        return false;
    }
    true
}

pub fn storage_withdraw_enabled(storage: &StorageModel, inventory: &InventoryModel) -> bool {
    if storage.has_password && !storage.unlocked {
        return false;
    }
    let Some(slot) = storage.selected_storage_slot else {
        return false;
    };
    if storage.item_in_storage(slot).is_none() {
        return false;
    }
    let occupied = inventory.items.iter().filter(|i| i.container == 0).count() as u32;
    if occupied >= BAG_SLOTS {
        return false;
    }
    true
}

pub fn storage_unlock_enabled(storage: &StorageModel) -> bool {
    storage.has_password
        && !storage.unlocked
        && !storage.password_draft.trim().is_empty()
        && storage.password_draft.len() >= 4
}

pub fn storage_set_password_enabled(storage: &StorageModel) -> bool {
    let new = storage.new_password_draft.trim();
    let confirm = storage.confirm_password_draft.trim();
    if new.is_empty() || confirm.is_empty() {
        return false;
    }
    if new != confirm {
        return false;
    }
    if new.len() < 4 || new.len() > 16 {
        return false;
    }
    if storage.has_password && storage.password_draft.trim().is_empty() {
        return false;
    }
    true
}

pub fn storage_remove_password_enabled(storage: &StorageModel) -> bool {
    storage.has_password && !storage.password_draft.trim().is_empty()
}

pub fn storage_expand_enabled(storage: &StorageModel, gold: u32) -> bool {
    if storage.has_expanded {
        return false;
    }
    if gold < STORAGE_EXPAND_COST {
        return false;
    }
    true
}

pub fn storage_password_display(draft: &str) -> String {
    "*".repeat(draft.chars().count())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::ItemModel;

    fn item(slot: u32, container: u8) -> ItemModel {
        ItemModel {
            unique_id: Some(u64::from(slot) + 1),
            key: "k".to_owned(),
            name: "n".to_owned(),
            quantity: 1,
            slot,
            container,
        }
    }

    #[test]
    fn storage_occupied_and_free() {
        let mut m = StorageModel {
            size: 10,
            ..Default::default()
        };
        m.items.push(item(0, 4));
        m.items.push(item(1, 4));
        m.items.push(item(2, 0));
        assert_eq!(m.storage_occupied(), 2);
        assert_eq!(m.free_storage_slots(), 8);
    }

    #[test]
    fn deposit_requires_unlocked_and_free_slot() {
        let storage = StorageModel {
            size: 1,
            has_password: true,
            unlocked: false,
            selected_bag_slot: Some(0),
            ..Default::default()
        };
        let inv = InventoryModel {
            gold: 0,
            items: vec![item(0, 0)],
        };
        assert!(!storage_deposit_enabled(&storage, &inv));
        let mut unlocked = storage;
        unlocked.unlocked = true;
        assert!(storage_deposit_enabled(&unlocked, &inv));
        unlocked.items.push(item(0, 4));
        assert!(!storage_deposit_enabled(&unlocked, &inv)); // no free slot
    }

    #[test]
    fn serde_roundtrip() {
        let m = StorageModel {
            size: 30,
            has_password: true,
            unlocked: true,
            has_expanded: false,
            expiry: 123,
            items: vec![item(0, 4)],
            ..Default::default()
        };
        let json = serde_json::to_string(&m).expect("ser");
        let restored: StorageModel = serde_json::from_str(&json).expect("de");
        assert_eq!(m, restored);
    }
}
