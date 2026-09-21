//! Authoritative storage / warehouse model.

//! Mirrors `WorldSnapshot.storage*` so the Windows Warehouse panel shows the
//! server's storage items, size and password state.

use bevy::prelude::Resource;
use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::inventory::{InventoryModel, ItemModel};

/// Crystal allocates one 10×16 storage grid. Its two tabs expose the first
/// and second 10×8 halves, so the renderer-neutral model uses two equivalent
/// 80-slot pages and retains the real wire slot (0..160).
pub const STORAGE_GRID_COLUMNS: u32 = 10;
pub const STORAGE_GRID_ROWS: u32 = 16;
pub const STORAGE_PAGE_ROWS: u32 = STORAGE_GRID_ROWS / 2;
pub const STORAGE_PAGE_SIZE: u32 = STORAGE_GRID_COLUMNS * STORAGE_PAGE_ROWS;
pub const STORAGE_VIEW_PAGE_COUNT: usize = 2;
pub const STORAGE_BASE_SIZE: u16 = 80;
pub const STORAGE_EXPANDED_SIZE: u16 = 160;
pub const STORAGE_EXPAND_COST: u32 = 1_000_000;
pub const BAG_SLOTS: u32 = 46;

fn deserialize_bounded_items<'de, D>(deserializer: D) -> Result<Vec<ItemModel>, D::Error>
where
    D: de::Deserializer<'de>,
{
    struct StorageItemsVisitor;

    impl<'de> Visitor<'de> for StorageItemsVisitor {
        type Value = Vec<ItemModel>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a bounded storage item sequence")
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut items =
                Vec::with_capacity(usize::try_from(STORAGE_EXPANDED_SIZE).unwrap_or_default());
            while let Some(item) = sequence.next_element::<ItemModel>()? {
                if items.len() < usize::from(STORAGE_EXPANDED_SIZE) {
                    items.push(item);
                }
            }
            Ok(items)
        }
    }

    deserializer.deserialize_seq(StorageItemsVisitor)
}

#[derive(Debug, Clone, Resource, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageModel {
    #[serde(default, deserialize_with = "deserialize_bounded_items")]
    pub items: Vec<ItemModel>,
    #[serde(alias = "storageSize")]
    pub size: u16,
    #[serde(alias = "hasStoragePassword")]
    pub has_password: bool,
    #[serde(alias = "storageUnlocked")]
    pub unlocked: bool,
    #[serde(alias = "hasExpandedStorage")]
    pub has_expanded: bool,
    #[serde(alias = "expiryTimeBinaryDatetime")]
    pub expiry: i64,
    pub selected_bag_slot: Option<u32>,
    pub selected_storage_slot: Option<u32>,
    pub password_draft: String,
    pub new_password_draft: String,
    pub confirm_password_draft: String,
}

impl Default for StorageModel {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            size: STORAGE_BASE_SIZE,
            has_password: false,
            unlocked: false,
            has_expanded: false,
            expiry: 0,
            selected_bag_slot: None,
            selected_storage_slot: None,
            password_draft: String::new(),
            new_password_draft: String::new(),
            confirm_password_draft: String::new(),
        }
    }
}

impl StorageModel {
    pub fn new() -> Self {
        Self {
            size: STORAGE_BASE_SIZE,
            unlocked: true,
            ..Default::default()
        }
    }

    pub fn storage_occupied(&self) -> usize {
        self.items
            .iter()
            .filter(|i| i.container == 4 && self.is_valid_slot(i.slot))
            .count()
    }

    pub fn free_storage_slots(&self) -> u16 {
        self.effective_size()
            .saturating_sub(self.storage_occupied() as u16)
    }

    /// Crystal StorageDialog permits transfers without a password and after a
    /// successful password unlock. The `unlocked` bit alone is not a lock when
    /// the account does not require storage protection.
    pub fn transfers_unlocked(&self) -> bool {
        !self.has_password || self.unlocked
    }

    pub fn first_empty_storage_slot(&self) -> Option<u32> {
        (0..u32::from(self.effective_size())).find(|slot| self.item_in_storage(*slot).is_none())
    }

    pub fn item_in_storage(&self, slot: u32) -> Option<&ItemModel> {
        self.items
            .iter()
            .find(|i| i.container == 4 && i.slot == slot && self.is_valid_slot(i.slot))
    }

    pub fn effective_size(&self) -> u16 {
        // `ResizeStorage` carries the concrete storage count. Keep a smaller
        // authoritative nonzero count, but never expose the second Crystal page
        // after `has_expanded` is revoked while a stale 160-slot count remains.
        let declared_size = if self.size == 0 {
            STORAGE_BASE_SIZE
        } else {
            self.size.min(STORAGE_EXPANDED_SIZE)
        };
        if self.has_expanded {
            declared_size
        } else {
            declared_size.min(STORAGE_BASE_SIZE)
        }
    }

    pub fn is_valid_slot(&self, slot: u32) -> bool {
        slot < u32::from(self.effective_size())
    }

    /// Clears the model-owned storage selection when refreshed authority no
    /// longer exposes that exact storage slot (for example, expansion revocation).
    pub fn clear_invalid_storage_selection(&mut self) {
        if self
            .selected_storage_slot
            .is_some_and(|slot| self.item_in_storage(slot).is_none())
        {
            self.selected_storage_slot = None;
        }
    }

    pub fn page_count(&self) -> usize {
        STORAGE_VIEW_PAGE_COUNT
    }

    pub fn clamp_page(&self, page: usize) -> usize {
        page.min(self.page_count().saturating_sub(1))
    }

    pub fn page(&self, page: usize) -> StoragePage<'_> {
        let page = self.clamp_page(page);
        let start = u32::try_from(page).unwrap_or_default() * STORAGE_PAGE_SIZE;
        let slots = (0..STORAGE_PAGE_SIZE)
            .map(|offset| {
                let slot = start + offset;
                let item = self.item_in_storage(slot);
                StorageSlot {
                    slot,
                    unique_id: item.and_then(|value| value.unique_id),
                    item,
                    locked: !self.is_valid_slot(slot) || (page == 1 && !self.has_expanded),
                }
            })
            .collect();
        StoragePage {
            page,
            page_count: self.page_count(),
            // Password protection and the un-rented second page are separate
            // source states. RefreshStorage2 remains reachable while its
            // rental page is locked, and shows its own cover instead of a
            // disabled tab.
            locked: !self.transfers_unlocked(),
            rental_locked: page == 1 && !self.has_expanded,
            expanded: self.has_expanded && self.effective_size() > STORAGE_BASE_SIZE,
            expiry: self.expiry,
            slots,
        }
    }

    pub fn clamp_after_refresh(&mut self, cursor: &mut StoragePageCursor) {
        cursor.page = self.clamp_page(cursor.page);
        self.clear_invalid_storage_selection();
    }

    pub fn selection_for_slot(&self, slot: u32) -> Option<StorageItemSelection> {
        let item = self.item_in_storage(slot)?;
        Some(StorageItemSelection {
            slot,
            unique_id: item.unique_id?,
        })
    }

    pub fn item_for_selection(&self, selection: StorageItemSelection) -> Option<&ItemModel> {
        self.item_in_storage(selection.slot)
            .filter(|item| item.unique_id == Some(selection.unique_id))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct StoragePageCursor {
    pub page: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageItemSelection {
    pub slot: u32,
    pub unique_id: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StorageSlot<'a> {
    pub slot: u32,
    pub unique_id: Option<u64>,
    pub item: Option<&'a ItemModel>,
    pub locked: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoragePage<'a> {
    pub page: usize,
    pub page_count: usize,
    /// Password protection state. It does not disable the second tab.
    pub locked: bool,
    /// Crystal RefreshStorage2 covers an un-rented second page with
    /// Prguse[2443] and hides every storage cell.
    pub rental_locked: bool,
    pub expanded: bool,
    pub expiry: i64,
    pub slots: Vec<StorageSlot<'a>>,
}

pub fn storage_deposit_enabled(storage: &StorageModel, inventory: &InventoryModel) -> bool {
    if !storage.transfers_unlocked() {
        return false;
    }
    let Some(slot) = storage.selected_bag_slot else {
        return false;
    };
    if !inventory
        .items
        .iter()
        .any(|i| i.container == 0 && i.slot == slot && i.unique_id.is_some())
    {
        return false;
    }
    if storage.free_storage_slots() == 0 {
        return false;
    }
    true
}

pub fn inventory_selection_for_slot(
    inventory: &InventoryModel,
    slot: u32,
) -> Option<StorageItemSelection> {
    let item = inventory
        .items
        .iter()
        .find(|item| item.container == 0 && item.slot == slot)?;
    Some(StorageItemSelection {
        slot,
        unique_id: item.unique_id?,
    })
}

pub fn storage_deposit_enabled_for_selection(
    storage: &StorageModel,
    inventory: &InventoryModel,
    selection: StorageItemSelection,
) -> bool {
    if !storage.transfers_unlocked() {
        return false;
    }
    if inventory_selection_for_slot(inventory, selection.slot) != Some(selection) {
        return false;
    }
    storage.free_storage_slots() > 0
}

pub fn storage_withdraw_enabled(storage: &StorageModel, inventory: &InventoryModel) -> bool {
    if !storage.transfers_unlocked() {
        return false;
    }
    let Some(slot) = storage.selected_storage_slot else {
        return false;
    };
    let Some(item) = storage.item_in_storage(slot) else {
        return false;
    };
    if item.unique_id.is_none() {
        return false;
    }
    let occupied = inventory.items.iter().filter(|i| i.container == 0).count() as u32;
    if occupied >= BAG_SLOTS {
        return false;
    }
    true
}

pub fn storage_withdraw_enabled_for_selection(
    storage: &StorageModel,
    inventory: &InventoryModel,
    selection: StorageItemSelection,
) -> bool {
    if !storage.transfers_unlocked() {
        return false;
    }
    if storage.item_for_selection(selection).is_none() {
        return false;
    }
    let occupied = inventory.items.iter().filter(|i| i.container == 0).count() as u32;
    occupied < BAG_SLOTS
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

/// Crystal permits the same Rent control to extend an active storage rental.
/// The confirmation differs by state, but the wallet threshold is identical.
pub fn storage_expand_enabled(_storage: &StorageModel, gold: u32) -> bool {
    gold >= STORAGE_EXPAND_COST
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
            ..ItemModel::default()
        }
    }

    #[test]
    fn storage_occupied_and_free() {
        let mut m = StorageModel::new();
        m.items.push(item(0, 4));
        m.items.push(item(1, 4));
        m.items.push(item(2, 0));
        assert_eq!(m.storage_occupied(), 2);
        assert_eq!(m.free_storage_slots(), 78);
    }

    #[test]
    fn deposit_requires_unlocked_and_free_slot() {
        let storage = StorageModel {
            has_password: true,
            unlocked: false,
            selected_bag_slot: Some(0),
            ..Default::default()
        };
        let inv = InventoryModel {
            gold: 0,
            items: vec![item(0, 0)],
            ..Default::default()
        };
        assert!(!storage_deposit_enabled(&storage, &inv));
        let mut unlocked = storage;
        unlocked.unlocked = true;
        assert!(storage_deposit_enabled(&unlocked, &inv));
        unlocked.items.extend(
            (0..STORAGE_BASE_SIZE).map(|slot| item(u32::from(slot), 4)),
        );
        assert!(!storage_deposit_enabled(&unlocked, &inv)); // no free slot
    }

    #[test]
    fn serde_roundtrip() {
        let m = StorageModel {
            size: STORAGE_BASE_SIZE,
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

    #[test]
    fn crystal_storage_is_two_real_ten_by_eight_pages() {
        let mut model = StorageModel::new();
        model.items.push(item(0, 4));
        assert_eq!(STORAGE_GRID_COLUMNS * STORAGE_GRID_ROWS, 160);
        assert_eq!(model.effective_size(), STORAGE_BASE_SIZE);
        assert_eq!(model.page(0).slots.len(), 80);
        assert_eq!(model.page(1).page, 1);
        assert_eq!(model.page_count(), STORAGE_VIEW_PAGE_COUNT);

        let locked_page = model.page(1);
        assert!(!locked_page.locked, "the second tab remains clickable");
        assert!(locked_page.rental_locked, "unrented page is covered");
        assert!(
            locked_page.slots[0].locked,
            "unrented page slots remain unavailable"
        );

        model.has_expanded = true;
        model.size = STORAGE_EXPANDED_SIZE;
        let page = model.page(1);
        assert_eq!(page.page, 1);
        assert_eq!(page.page_count, STORAGE_VIEW_PAGE_COUNT);
        assert_eq!(page.slots[0].slot, 80);
        assert!(!page.slots[0].locked);
        assert_eq!(page.slots.len(), 80);
        assert_eq!(page.expiry, 0);

        let mut cursor = StoragePageCursor { page: 99 };
        model.clamp_after_refresh(&mut cursor);
        assert_eq!(cursor.page, 1);
    }

    #[test]
    fn storage_capacity_uses_crystal_80_and_160_slot_boundaries() {
        let mut model = StorageModel {
            size: STORAGE_EXPANDED_SIZE,
            ..StorageModel::new()
        };
        assert!(model.is_valid_slot(79));
        assert!(!model.is_valid_slot(80));
        assert!(model.page(1).rental_locked);
        assert!(model.page(1).slots[0].locked);

        model.has_expanded = true;
        assert!(model.is_valid_slot(80));
        assert!(model.is_valid_slot(159));
        assert!(!model.is_valid_slot(160));
        assert!(!model.page(1).rental_locked);
        assert!(!model.page(1).slots[0].locked);
        assert!(!model.page(1).slots[79].locked);
    }

    #[test]
    fn storage_keeps_smaller_authoritative_capacity_and_locks_second_page() {
        let model = StorageModel {
            size: 79,
            has_expanded: true,
            ..StorageModel::new()
        };
        assert_eq!(model.effective_size(), 79);
        assert!(model.is_valid_slot(78));
        assert!(!model.is_valid_slot(79));
        assert!(!model.page(1).locked);
        assert!(!model.page(1).rental_locked, "expanded authority has no rental cover");
    }

    #[test]
    fn storage_expansion_revocation_invalidates_selected_second_page_slot() {
        let mut model = StorageModel {
            size: STORAGE_EXPANDED_SIZE,
            has_expanded: true,
            selected_storage_slot: Some(80),
            ..StorageModel::new()
        };
        model.items.push(item(80, 4));
        assert!(model.selection_for_slot(80).is_some());

        model.has_expanded = false;
        let mut cursor = StoragePageCursor { page: 1 };
        model.clamp_after_refresh(&mut cursor);
        assert_eq!(model.selected_storage_slot, None);
        assert!(model.selection_for_slot(80).is_none());
        assert!(model.page(1).rental_locked);
    }

    #[test]
    fn storage_selection_requires_exact_slot_and_unique_id() {
        let mut model = StorageModel::new();
        model.items.push(item(3, 4));
        let selection = model.selection_for_slot(3).expect("selection");
        assert_eq!(selection.unique_id, 4);
        assert!(model.item_for_selection(selection).is_some());
        assert!(model
            .item_for_selection(StorageItemSelection {
                slot: 4,
                unique_id: 4,
            })
            .is_none());
        assert!(model
            .item_for_selection(StorageItemSelection {
                slot: 3,
                unique_id: 999,
            })
            .is_none());

        let inventory = InventoryModel {
            items: vec![item(3, 0)],
            ..Default::default()
        };
        let bag_selection = inventory_selection_for_slot(&inventory, 3).expect("bag selection");
        assert!(storage_deposit_enabled_for_selection(
            &model,
            &inventory,
            bag_selection
        ));
        assert!(!storage_deposit_enabled_for_selection(
            &model,
            &inventory,
            StorageItemSelection {
                unique_id: 999,
                ..bag_selection
            }
        ));

        model.items[0].unique_id = None;
        assert!(model.selection_for_slot(3).is_none());
    }

    #[test]
    fn storage_serde_defaults_and_bounds_legacy_payloads() {
        let payload = serde_json::json!({
            "storageSize": 160,
            "hasStoragePassword": true,
            "storageUnlocked": false,
            "hasExpandedStorage": true,
            "expiryTimeBinaryDatetime": 123,
            "items": (0..(usize::from(STORAGE_EXPANDED_SIZE) + 9)).map(|slot| serde_json::json!({
                "uniqueId": slot + 1,
                "key": "item",
                "name": "Item",
                "quantity": 1,
                "slot": slot,
                "container": 4
            })).collect::<Vec<_>>()
        });
        let model: StorageModel = serde_json::from_value(payload).expect("legacy storage");
        assert_eq!(model.size, 160);
        assert_eq!(model.effective_size(), STORAGE_EXPANDED_SIZE);
        assert_eq!(model.items.len(), usize::from(STORAGE_EXPANDED_SIZE));
        assert!(model.has_expanded);
        assert!(!model.unlocked);
        assert_eq!(model.expiry, 123);
        assert_eq!(model.page_count(), 2);

        let defaults: StorageModel = serde_json::from_str("{}").expect("old empty model");
        assert_eq!(defaults.size, STORAGE_BASE_SIZE);
        assert_eq!(defaults.effective_size(), STORAGE_BASE_SIZE);
    }
}
