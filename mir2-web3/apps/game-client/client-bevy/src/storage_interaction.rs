//! Pure personal-warehouse decisions over the current authoritative read models.
//! Plans do not mutate items or reserve requests; hosts retain their existing
//! queue, pending-operation and receipt ownership.

use crate::inventory::{InventoryModel, ItemModel};
use crate::storage::{StorageItemSelection, StorageModel};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageGrid {
    Inventory,
    Storage,
}

impl StorageGrid {
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Inventory => "inventory",
            Self::Storage => "storage",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageTransferPlan {
    Store {
        unique_id: u64,
        from: u32,
        to: u32,
    },
    TakeBack {
        unique_id: u64,
        from: u32,
        to: u32,
    },
    Move {
        unique_id: u64,
        from: u32,
        to: u32,
    },
    Merge {
        grid_from: StorageGrid,
        grid_to: StorageGrid,
        id_from: u64,
        id_to: u64,
    },
}

/// Occupancy is limited to real bag cells. Stray out-of-capacity rows cannot
/// make a valid empty destination disappear, nor can duplicate rows fill it.
pub fn first_empty_bag_slot(inventory: &InventoryModel) -> Option<u32> {
    (0..u32::from(inventory.bag_slot_capacity())).find(|slot| {
        !inventory
            .items
            .iter()
            .any(|item| item.container == 0 && item.slot == *slot)
    })
}

/// The outer Option rejects ambiguous cells; the inner one represents vacancy.
fn unique_cell(items: &[ItemModel], container: u8, slot: u32) -> Option<Option<&ItemModel>> {
    let mut rows = items
        .iter()
        .filter(|item| item.container == container && item.slot == slot);
    let first = rows.next();
    rows.next().is_none().then_some(first)
}

/// Only the actual grids involved in this gesture count. Other containers in
/// either read model can be mirrors of those items and must not count twice.
fn unique_instance_id(
    unique_id: u64,
    inventory: Option<&InventoryModel>,
    storage: &StorageModel,
) -> bool {
    let bag_count = inventory.map_or(0, |inventory| {
        inventory
            .items
            .iter()
            .filter(|item| item.container == 0 && item.unique_id == Some(unique_id))
            .count()
    });
    let storage_count = storage
        .items
        .iter()
        .filter(|item| item.container == 4 && item.unique_id == Some(unique_id))
        .count();
    bag_count + storage_count == 1
}

fn unique_endpoints(
    source: &ItemModel,
    target: Option<&ItemModel>,
    inventory: Option<&InventoryModel>,
    storage: &StorageModel,
) -> bool {
    source
        .unique_id
        .is_some_and(|id| unique_instance_id(id, inventory, storage))
        && target.is_none_or(|item| {
            item.unique_id
                .is_none_or(|id| unique_instance_id(id, inventory, storage))
        })
}

fn selected_bag_item(
    inventory: &InventoryModel,
    selection: StorageItemSelection,
) -> Option<&ItemModel> {
    if selection.slot >= u32::from(inventory.bag_slot_capacity()) {
        return None;
    }
    unique_cell(&inventory.items, 0, selection.slot)?
        .filter(|item| item.unique_id == Some(selection.unique_id))
}

fn selected_storage_item(
    storage: &StorageModel,
    selection: StorageItemSelection,
) -> Option<&ItemModel> {
    if !storage.is_valid_slot(selection.slot) {
        return None;
    }
    unique_cell(&storage.items, 4, selection.slot)?
        .filter(|item| item.unique_id == Some(selection.unique_id))
}

fn complete_live_template_index(item: &ItemModel) -> Option<i32> {
    let source = item.tooltip_source.as_ref()?;
    let user_item = source.user_item.as_ref()?;
    (item.unique_id == Some(user_item.unique_id)
        && user_item.item_index == source.info.item_index
        && u16::try_from(item.quantity).ok() == Some(user_item.count))
    .then_some(source.info.item_index)
}

/// A displayed name or a stale tooltip is insufficient evidence for merging.
pub fn compatible_storage_stack(source: &ItemModel, target: &ItemModel) -> bool {
    complete_live_template_index(source)
        .zip(complete_live_template_index(target))
        .is_some_and(|(from_index, to_index)| {
            from_index == to_index
                && target.tooltip_source.as_ref().is_some_and(|source| {
                    target.quantity < u32::from(source.info.stack_size)
                })
        })
}

fn merge_plan(
    source: &ItemModel,
    target: &ItemModel,
    grid_from: StorageGrid,
    grid_to: StorageGrid,
) -> Option<StorageTransferPlan> {
    if !compatible_storage_stack(source, target) {
        return None;
    }
    Some(StorageTransferPlan::Merge {
        grid_from,
        grid_to,
        id_from: source.unique_id?,
        id_to: target.unique_id?,
    })
}

pub fn plan_bag_to_storage(
    inventory: &InventoryModel,
    storage: &StorageModel,
    selection: StorageItemSelection,
    target_slot: u32,
) -> Option<StorageTransferPlan> {
    if !storage.transfers_unlocked() || !storage.is_valid_slot(target_slot) {
        return None;
    }
    let source = selected_bag_item(inventory, selection)?;
    let target = unique_cell(&storage.items, 4, target_slot)?;
    if !unique_endpoints(source, target, Some(inventory), storage) {
        return None;
    }
    if let Some(plan) = target.and_then(|target| {
        merge_plan(source, target, StorageGrid::Inventory, StorageGrid::Storage)
    }) {
        return Some(plan);
    }
    let to = if target.is_none() {
        target_slot
    } else {
        storage.first_empty_storage_slot()?
    };
    Some(StorageTransferPlan::Store {
        unique_id: selection.unique_id,
        from: selection.slot,
        to,
    })
}

pub fn plan_storage_to_bag(
    inventory: &InventoryModel,
    storage: &StorageModel,
    selection: StorageItemSelection,
    target_slot: u32,
) -> Option<StorageTransferPlan> {
    if !storage.transfers_unlocked() || target_slot >= u32::from(inventory.bag_slot_capacity()) {
        return None;
    }
    let source = selected_storage_item(storage, selection)?;
    let target = unique_cell(&inventory.items, 0, target_slot)?;
    if !unique_endpoints(source, target, Some(inventory), storage) {
        return None;
    }
    if let Some(plan) = target.and_then(|target| {
        merge_plan(source, target, StorageGrid::Storage, StorageGrid::Inventory)
    }) {
        return Some(plan);
    }
    let to = if target.is_none() {
        target_slot
    } else {
        first_empty_bag_slot(inventory)?
    };
    Some(StorageTransferPlan::TakeBack {
        unique_id: selection.unique_id,
        from: selection.slot,
        to,
    })
}

pub fn plan_storage_to_storage(
    storage: &StorageModel,
    selection: StorageItemSelection,
    target_slot: u32,
) -> Option<StorageTransferPlan> {
    if !storage.transfers_unlocked()
        || !storage.is_valid_slot(target_slot)
        || selection.slot == target_slot
    {
        return None;
    }
    let source = selected_storage_item(storage, selection)?;
    let target = unique_cell(&storage.items, 4, target_slot)?;
    if !unique_endpoints(source, target, None, storage) {
        return None;
    }
    if let Some(plan) = target.and_then(|target| {
        merge_plan(source, target, StorageGrid::Storage, StorageGrid::Storage)
    }) {
        return Some(plan);
    }
    Some(StorageTransferPlan::Move {
        unique_id: selection.unique_id,
        from: selection.slot,
        to: target_slot,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{
        CrystalItemInfoModel, CrystalItemTooltipSourceModel, CrystalUserItemModel,
    };
    use crate::storage::{storage_withdraw_enabled, storage_withdraw_enabled_for_selection};

    fn item(container: u8, slot: u32, unique_id: u64, template: i32, count: u16) -> ItemModel {
        ItemModel {
            container,
            slot,
            unique_id: Some(unique_id),
            quantity: u32::from(count),
            tooltip_source: Some(CrystalItemTooltipSourceModel {
                info: CrystalItemInfoModel {
                    item_index: template,
                    stack_size: 20,
                    ..Default::default()
                },
                user_item: Some(CrystalUserItemModel {
                    unique_id,
                    item_index: template,
                    count,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn bag(items: Vec<ItemModel>) -> InventoryModel {
        InventoryModel {
            items,
            ..Default::default()
        }
    }

    fn warehouse(items: Vec<ItemModel>) -> StorageModel {
        StorageModel {
            items,
            ..StorageModel::new()
        }
    }

    fn selection(slot: u32, unique_id: u64) -> StorageItemSelection {
        StorageItemSelection { slot, unique_id }
    }

    #[test]
    fn transfers_recheck_slot_identity_container_and_current_capacity() {
        let inventory = bag(vec![item(0, 2, 10, 1, 2)]);
        let storage = warehouse(vec![item(4, 3, 20, 1, 2)]);
        assert_eq!(plan_bag_to_storage(&inventory, &storage, selection(2, 99), 4), None);
        assert_eq!(plan_bag_to_storage(&inventory, &storage, selection(3, 10), 4), None);
        assert_eq!(plan_storage_to_bag(&inventory, &storage, selection(3, 99), 4), None);
        assert_eq!(plan_storage_to_storage(&storage, selection(3, 99), 4), None);
        assert_eq!(plan_bag_to_storage(&inventory, &storage, selection(2, 10), 80), None);
        assert_eq!(
            plan_storage_to_bag(
                &inventory,
                &storage,
                selection(3, 20),
                u32::from(inventory.bag_slot_capacity()),
            ),
            None,
        );
        let wrong_container = bag(vec![item(1, 2, 10, 1, 2)]);
        assert_eq!(
            plan_bag_to_storage(&wrong_container, &storage, selection(2, 10), 4),
            None,
        );
        let out_of_capacity = bag(vec![item(0, 100, 10, 1, 2)]);
        assert_eq!(
            plan_bag_to_storage(&out_of_capacity, &storage, selection(100, 10), 4),
            None,
        );
    }

    #[test]
    fn password_and_rental_locks_are_rechecked_for_every_direction() {
        let inventory = bag(vec![item(0, 2, 10, 1, 2)]);
        let mut storage = warehouse(vec![item(4, 3, 20, 1, 2), item(4, 80, 30, 1, 2)]);
        storage.has_password = true;
        storage.unlocked = false;
        storage.size = 160;
        assert_eq!(plan_bag_to_storage(&inventory, &storage, selection(2, 10), 4), None);
        assert_eq!(plan_storage_to_bag(&inventory, &storage, selection(3, 20), 4), None);
        assert_eq!(plan_storage_to_storage(&storage, selection(3, 20), 4), None);
        storage.unlocked = true;
        assert_eq!(plan_storage_to_bag(&inventory, &storage, selection(80, 30), 4), None);
        assert_eq!(plan_storage_to_storage(&storage, selection(3, 20), 80), None);
        storage.has_expanded = true;
        assert_eq!(
            plan_storage_to_bag(&inventory, &storage, selection(80, 30), 4),
            Some(StorageTransferPlan::TakeBack { unique_id: 30, from: 80, to: 4 }),
        );
    }

    #[test]
    fn occupied_targets_merge_only_complete_current_matching_stacks() {
        let inventory = bag(vec![item(0, 2, 10, 1, 2)]);
        let mut storage = warehouse(vec![item(4, 3, 20, 1, 5)]);
        assert_eq!(
            plan_bag_to_storage(&inventory, &storage, selection(2, 10), 3),
            Some(StorageTransferPlan::Merge {
                grid_from: StorageGrid::Inventory,
                grid_to: StorageGrid::Storage,
                id_from: 10,
                id_to: 20,
            }),
        );
        storage.items[0].quantity = 6; // The carrier still says five: reject this merge.
        let fallback = Some(StorageTransferPlan::Store { unique_id: 10, from: 2, to: 0 });
        assert_eq!(plan_bag_to_storage(&inventory, &storage, selection(2, 10), 3), fallback);
        storage.items[0] = item(4, 3, 20, 2, 5);
        assert_eq!(plan_bag_to_storage(&inventory, &storage, selection(2, 10), 3), fallback);
        storage.items[0] = item(4, 3, 20, 1, 20);
        assert_eq!(plan_bag_to_storage(&inventory, &storage, selection(2, 10), 3), fallback);
        storage.items[0] = item(4, 3, 20, 1, 5);
        storage.items[0]
            .tooltip_source.as_mut().unwrap()
            .user_item.as_mut().unwrap()
            .unique_id = 99;
        assert_eq!(plan_bag_to_storage(&inventory, &storage, selection(2, 10), 3), fallback);
    }

    #[test]
    fn full_storage_accepts_merge_but_rejects_incompatible_fallback() {
        let inventory = bag(vec![item(0, 2, 10, 1, 2)]);
        let mut storage = warehouse(vec![item(4, 0, 20, 2, 1), item(4, 1, 30, 1, 5)]);
        storage.size = 2;
        assert_eq!(plan_bag_to_storage(&inventory, &storage, selection(2, 10), 0), None);
        assert!(matches!(
            plan_bag_to_storage(&inventory, &storage, selection(2, 10), 1),
            Some(StorageTransferPlan::Merge { id_to: 30, .. }),
        ));
        storage.items[1].tooltip_source = None;
        assert_eq!(plan_bag_to_storage(&inventory, &storage, selection(2, 10), 1), None);
    }

    #[test]
    fn take_back_uses_clicked_empty_cell_then_first_real_empty_cell() {
        let inventory = bag(vec![item(0, 2, 10, 2, 2)]);
        let storage = warehouse(vec![item(4, 3, 20, 1, 2)]);
        assert_eq!(
            plan_storage_to_bag(&inventory, &storage, selection(3, 20), 4),
            Some(StorageTransferPlan::TakeBack { unique_id: 20, from: 3, to: 4 }),
        );
        assert_eq!(
            plan_storage_to_bag(&inventory, &storage, selection(3, 20), 2),
            Some(StorageTransferPlan::TakeBack { unique_id: 20, from: 3, to: 0 }),
        );
        let same = bag(vec![item(0, 2, 10, 1, 2)]);
        assert_eq!(
            plan_storage_to_bag(&same, &storage, selection(3, 20), 2),
            Some(StorageTransferPlan::Merge {
                grid_from: StorageGrid::Storage,
                grid_to: StorageGrid::Inventory,
                id_from: 20,
                id_to: 10,
            }),
        );
    }

    #[test]
    fn internal_storage_move_preserves_slots_and_same_slot_is_noop() {
        let mut storage = warehouse(vec![item(4, 3, 20, 1, 2), item(4, 4, 30, 2, 2)]);
        assert_eq!(plan_storage_to_storage(&storage, selection(3, 20), 3), None);
        assert_eq!(
            plan_storage_to_storage(&storage, selection(3, 20), 4),
            Some(StorageTransferPlan::Move { unique_id: 20, from: 3, to: 4 }),
        );
        storage.items[1] = item(4, 4, 30, 1, 2);
        assert_eq!(
            plan_storage_to_storage(&storage, selection(3, 20), 4),
            Some(StorageTransferPlan::Merge {
                grid_from: StorageGrid::Storage,
                grid_to: StorageGrid::Storage,
                id_from: 20,
                id_to: 30,
            }),
        );
        assert_eq!(storage.items[0].slot, 3);
        assert_eq!(storage.items[0].quantity, 2);
        assert_eq!(storage.items[1].quantity, 2);
    }

    #[test]
    fn withdraw_buttons_follow_real_capacity_and_ignore_stray_or_duplicate_rows() {
        let mut storage = warehouse(vec![item(4, 3, 20, 1, 2)]);
        storage.selected_storage_slot = Some(3);
        let selected = selection(3, 20);
        let mut inventory = InventoryModel { capacity: 86, ..Default::default() };
        let capacity = u32::from(inventory.bag_slot_capacity());
        assert_eq!(capacity, 80);
        inventory.items = (0..capacity - 1)
            .map(|slot| item(0, slot, u64::from(slot) + 100, 2, 1))
            .collect();
        inventory.items.extend([item(0, 999, 999, 2, 1), item(0, 0, 1000, 2, 1)]);
        assert!(storage_withdraw_enabled(&storage, &inventory));
        assert!(storage_withdraw_enabled_for_selection(&storage, &inventory, selected));
        assert_eq!(first_empty_bag_slot(&inventory), Some(capacity - 1));
        inventory.items.push(item(0, capacity - 1, 2000, 2, 1));
        assert!(!storage_withdraw_enabled(&storage, &inventory));
        assert!(!storage_withdraw_enabled_for_selection(&storage, &inventory, selected));
        inventory.items.retain(|item| item.slot != capacity - 1);
        storage.items[0].unique_id = Some(21);
        assert!(!storage_withdraw_enabled_for_selection(&storage, &inventory, selected));
    }

    #[test]
    fn base_bag_full_rejects_take_back_even_with_fewer_than_46_items() {
        let mut inventory = InventoryModel::default();
        let capacity = u32::from(inventory.bag_slot_capacity());
        inventory.items = (0..capacity)
            .map(|slot| item(0, slot, u64::from(slot) + 100, 2, 1))
            .collect();
        let mut storage = warehouse(vec![item(4, 3, 20, 1, 2)]);
        storage.selected_storage_slot = Some(3);
        assert!(!storage_withdraw_enabled(&storage, &inventory));
        assert!(!storage_withdraw_enabled_for_selection(&storage, &inventory, selection(3, 20)));
        assert_eq!(plan_storage_to_bag(&inventory, &storage, selection(3, 20), 0), None);
    }

    #[test]
    fn duplicate_endpoint_cells_or_uids_reject_all_directions_but_mirrors_do_not() {
        let inventory = bag(vec![item(0, 2, 10, 1, 2), item(0, 4, 40, 1, 2)]);
        let storage = warehouse(vec![item(4, 3, 20, 1, 2), item(4, 5, 50, 1, 2)]);
        let mut damaged_bag = inventory.clone();
        damaged_bag.items.push(item(0, 2, 99, 1, 2)); // Same source cell, another UID.
        assert_eq!(plan_bag_to_storage(&damaged_bag, &storage, selection(2, 10), 5), None);
        assert_eq!(plan_storage_to_bag(&damaged_bag, &storage, selection(3, 20), 2), None);

        let mut damaged_storage = storage.clone();
        damaged_storage.items.push(item(4, 3, 99, 1, 2));
        assert_eq!(plan_bag_to_storage(&inventory, &damaged_storage, selection(2, 10), 3), None);
        assert_eq!(plan_storage_to_bag(&inventory, &damaged_storage, selection(3, 20), 4), None);
        assert_eq!(plan_storage_to_storage(&damaged_storage, selection(3, 20), 5), None);
        assert_eq!(plan_storage_to_storage(&damaged_storage, selection(5, 50), 3), None);

        let mut duplicate_bag_uid = inventory.clone();
        duplicate_bag_uid.items.push(item(0, 8, 10, 1, 2));
        assert_eq!(plan_bag_to_storage(&duplicate_bag_uid, &storage, selection(2, 10), 5), None);
        assert_eq!(plan_storage_to_bag(&duplicate_bag_uid, &storage, selection(3, 20), 2), None);

        let mut duplicate_storage_uid = storage.clone();
        duplicate_storage_uid.items.push(item(4, 8, 20, 1, 2));
        assert_eq!(
            plan_bag_to_storage(&inventory, &duplicate_storage_uid, selection(2, 10), 3),
            None,
        );
        assert_eq!(
            plan_storage_to_bag(&inventory, &duplicate_storage_uid, selection(3, 20), 4),
            None,
        );
        assert_eq!(plan_storage_to_storage(&duplicate_storage_uid, selection(3, 20), 5), None);
        assert_eq!(plan_storage_to_storage(&duplicate_storage_uid, selection(5, 50), 3), None);

        let mut cross_grid_uid = storage.clone();
        cross_grid_uid.items.push(item(4, 8, 10, 1, 2));
        assert_eq!(plan_bag_to_storage(&inventory, &cross_grid_uid, selection(2, 10), 5), None);
        assert_eq!(plan_storage_to_bag(&inventory, &cross_grid_uid, selection(3, 20), 2), None);

        let mut mirrored_bag = inventory.clone();
        mirrored_bag.items.extend(storage.items.clone());
        let mut mirrored_storage = storage.clone();
        mirrored_storage.items.extend(inventory.items.clone());
        assert!(matches!(
            plan_bag_to_storage(&mirrored_bag, &mirrored_storage, selection(2, 10), 3),
            Some(StorageTransferPlan::Merge { id_from: 10, id_to: 20, .. }),
        ));
        assert!(matches!(
            plan_storage_to_bag(&mirrored_bag, &mirrored_storage, selection(3, 20), 2),
            Some(StorageTransferPlan::Merge { id_from: 20, id_to: 10, .. }),
        ));
        assert!(matches!(
            plan_storage_to_storage(&mirrored_storage, selection(3, 20), 5),
            Some(StorageTransferPlan::Merge { id_from: 20, id_to: 50, .. }),
        ));
    }
}
