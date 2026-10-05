//! Instance-safe inventory selection and equipment decisions shared by hosts.
//! Inventory contents and operation results remain server authoritative.

use crate::inventory::{InventoryModel, ItemModel, CRYSTAL_FIRST_BAG_PAGE_SLOTS};
use crate::pending_operations::{PendingOperationKey, PendingOperations};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BagPage {
    First,
    Second,
}

impl BagPage {
    pub fn slots(self, inventory: &InventoryModel) -> std::ops::Range<u32> {
        match self {
            Self::First => 0..u32::from(CRYSTAL_FIRST_BAG_PAGE_SLOTS),
            Self::Second if inventory.second_bag_unlocked() => {
                u32::from(CRYSTAL_FIRST_BAG_PAGE_SLOTS)..u32::from(inventory.bag_slot_capacity())
            }
            Self::Second => 0..0,
        }
    }
}

/// An instance, not a template key. `None` means identity is unavailable;
/// zero is a valid server item id (including the N1 WoodenSword reward).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BagSelection {
    pub container: u8,
    pub slot: u32,
    pub unique_id: u64,
}

impl BagSelection {
    pub fn from_item(item: &ItemModel) -> Option<Self> {
        Some(Self {
            container: item.container,
            slot: item.slot,
            unique_id: item.unique_id?,
        })
    }
}

pub fn selection_at_slot(
    inventory: &InventoryModel,
    page: BagPage,
    slot: u32,
) -> Option<BagSelection> {
    page.slots(inventory).contains(&slot).then_some(())?;
    unique_at_location(inventory, 0, slot).and_then(BagSelection::from_item)
}

pub fn resolve_exact<'a>(
    inventory: &'a InventoryModel,
    selection: BagSelection,
) -> Option<&'a ItemModel> {
    if selection.container == 0 && selection.slot >= u32::from(inventory.bag_slot_capacity()) {
        return None;
    }
    unique_at_location(inventory, selection.container, selection.slot)
        .filter(|item| item.unique_id == Some(selection.unique_id))
}

fn unique_at_location(inventory: &InventoryModel, container: u8, slot: u32) -> Option<&ItemModel> {
    let mut matches = inventory
        .items
        .iter()
        .filter(|item| item.container == container && item.slot == slot);
    let item = matches.next()?;
    matches.next().is_none().then_some(item)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BagEquipmentKind {
    Equip,
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BagEquipmentPlan {
    pub kind: BagEquipmentKind,
    pub unique_id: u64,
    pub grid: &'static str,
    pub to: i32,
}

impl BagEquipmentPlan {
    pub fn pending_key(&self) -> PendingOperationKey {
        match self.kind {
            BagEquipmentKind::Equip => PendingOperationKey::Equip {
                grid: self.grid.to_owned(),
                unique_id: self.unique_id,
                to: self.to,
            },
            BagEquipmentKind::Remove => PendingOperationKey::Remove {
                grid: self.grid.to_owned(),
                unique_id: self.unique_id,
                to: self.to,
            },
        }
    }
}

pub fn plan_equipment(
    inventory: &InventoryModel,
    selection: BagSelection,
    pending: &PendingOperations,
) -> Option<BagEquipmentPlan> {
    let item = resolve_exact(inventory, selection)?;
    if pending.has_pending_equipment_instance(selection.unique_id) {
        return None;
    }
    if item.container == 2 {
        // The gateway RemoveItem contract addresses a normalized bag cell.
        let to = (0..inventory.bag_slot_capacity()).find(|slot| {
            !inventory
                .items
                .iter()
                .any(|candidate| candidate.container == 0 && candidate.slot == u32::from(*slot))
        })?;
        return Some(BagEquipmentPlan {
            kind: BagEquipmentKind::Remove,
            unique_id: selection.unique_id,
            grid: "inventory",
            to: i32::from(to),
        });
    }
    let grid = match item.container {
        0 => "inventory",
        1 => "belt",
        4 => "storage",
        _ => return None,
    };
    Some(BagEquipmentPlan {
        kind: BagEquipmentKind::Equip,
        unique_id: selection.unique_id,
        grid,
        to: equip_destination_for_item(item),
    })
}

pub fn equip_destination_for_item(item: &ItemModel) -> i32 {
    item.equip_slot
        .as_deref()
        .and_then(|slot| match slot.to_ascii_lowercase().as_str() {
            "weapon" => Some(0),
            "armour" | "armor" => Some(1),
            "helmet" => Some(2),
            "torch" => Some(3),
            "necklace" => Some(4),
            "braceletleft" => Some(5),
            "braceletright" => Some(6),
            "ringleft" => Some(7),
            "ringright" => Some(8),
            "amulet" => Some(9),
            "belt" => Some(10),
            "boots" => Some(11),
            "stone" => Some(12),
            "mount" => Some(13),
            _ => None,
        })
        .unwrap_or_else(|| equip_destination_for_name(&item.name))
}

pub fn equip_destination_for_name(name: &str) -> i32 {
    let lowered = name.to_ascii_lowercase();
    if lowered.contains("pendant") || lowered.contains("necklace") {
        4
    } else if lowered.contains("ring") {
        7
    } else if lowered.contains("bracelet") {
        5
    } else if lowered.contains("helmet") {
        2
    } else if lowered.contains("boot") {
        11
    } else if lowered.contains("belt") {
        10
    } else if lowered.contains("armour") || lowered.contains("armor") {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pending_operations::{
        apply_inventory_operation_ack, reconcile_inventory_refresh, InventoryOperationAck,
        InventoryOperationFeedback,
    };

    fn item(id: Option<u64>, slot: u32, container: u8) -> ItemModel {
        ItemModel {
            unique_id: id,
            key: "same-template".into(),
            name: "Same Ring".into(),
            slot,
            container,
            quantity: 1,
            equip_slot: Some("RingLeft".into()),
            ..Default::default()
        }
    }

    #[test]
    fn bag_pages_obey_authoritative_capacity_and_only_missing_identity_fails_closed() {
        let mut inventory = InventoryModel {
            items: vec![
                item(Some(7), 39, 0),
                item(Some(8), 40, 0),
                item(None, 0, 0),
                item(Some(0), 1, 0),
            ],
            ..Default::default()
        };
        assert_eq!(BagPage::First.slots(&inventory), 0..40);
        assert_eq!(BagPage::Second.slots(&inventory), 0..0);
        assert!(selection_at_slot(&inventory, BagPage::First, 39).is_some());
        assert!(selection_at_slot(&inventory, BagPage::Second, 40).is_none());
        assert!(selection_at_slot(&inventory, BagPage::First, 0).is_none());
        assert_eq!(
            selection_at_slot(&inventory, BagPage::First, 1)
                .unwrap()
                .unique_id,
            0
        );
        inventory.capacity = 54;
        assert_eq!(BagPage::Second.slots(&inventory), 40..48);
        assert_eq!(
            selection_at_slot(&inventory, BagPage::Second, 40)
                .unwrap()
                .unique_id,
            8
        );
        inventory.capacity = 55;
        assert_eq!(BagPage::Second.slots(&inventory), 0..0);
    }

    #[test]
    fn zero_id_item_can_be_selected_equipped_and_reserved() {
        let inventory = InventoryModel {
            items: vec![item(Some(0), 1, 0)],
            ..Default::default()
        };
        let selected = selection_at_slot(&inventory, BagPage::First, 1).unwrap();
        assert_eq!(
            resolve_exact(&inventory, selected).unwrap().unique_id,
            Some(0)
        );
        let mut pending = PendingOperations::default();
        let plan = plan_equipment(&inventory, selected, &pending).unwrap();
        assert_eq!(plan.unique_id, 0);
        assert!(pending.try_begin(plan.pending_key()));
        assert!(plan_equipment(&inventory, selected, &pending).is_none());
        assert!(pending.has_pending_equipment_instance(0));
        let mut feedback = InventoryOperationFeedback::default();
        assert_eq!(
            apply_inventory_operation_ack(
                &mut pending,
                &mut feedback,
                InventoryOperationAck::Equip {
                    grid: "inventory".into(),
                    unique_id: 0,
                    to: plan.to,
                    success: true
                }
            ),
            1
        );
        assert!(pending.has_pending_equipment_instance(0));
        let worn = InventoryModel {
            items: vec![item(Some(0), 7, 2)],
            ..Default::default()
        };
        assert_eq!(reconcile_inventory_refresh(&mut pending, &inventory, &worn), 1);
        assert!(!pending.has_pending_equipment_instance(0));
        let remove = plan_equipment(
            &worn,
            BagSelection::from_item(&worn.items[0]).unwrap(),
            &pending,
        )
        .unwrap();
        assert_eq!(
            (remove.kind, remove.unique_id, remove.to),
            (BagEquipmentKind::Remove, 0, 0)
        );
    }

    #[test]
    fn same_template_same_slot_replacement_cannot_resolve_or_equip() {
        let original = InventoryModel {
            items: vec![item(Some(7), 4, 0)],
            ..Default::default()
        };
        let selected = selection_at_slot(&original, BagPage::First, 4).unwrap();
        let replacement = InventoryModel {
            items: vec![item(Some(8), 4, 0)],
            ..Default::default()
        };
        assert!(resolve_exact(&replacement, selected).is_none());
        assert!(plan_equipment(&replacement, selected, &PendingOperations::default()).is_none());
    }

    #[test]
    fn duplicated_location_fails_closed_even_if_one_identity_matches() {
        let selected = BagSelection {
            container: 0,
            slot: 4,
            unique_id: 7,
        };
        let inventory = InventoryModel {
            items: vec![item(Some(7), 4, 0), item(Some(8), 4, 0)],
            ..Default::default()
        };
        assert!(selection_at_slot(&inventory, BagPage::First, 4).is_none());
        assert!(resolve_exact(&inventory, selected).is_none());
        assert!(plan_equipment(&inventory, selected, &PendingOperations::default()).is_none());
        let worn = InventoryModel {
            items: vec![item(Some(7), 0, 2), item(Some(7), 0, 2)],
            ..Default::default()
        };
        assert!(resolve_exact(
            &worn,
            BagSelection {
                container: 2,
                slot: 0,
                unique_id: 7
            }
        )
        .is_none());
    }

    #[test]
    fn equipment_plan_preserves_slot_mapping_and_authoritative_bag_destination() {
        let bag = InventoryModel {
            items: vec![item(Some(7), 40, 0)],
            capacity: 54,
            ..Default::default()
        };
        let selected = selection_at_slot(&bag, BagPage::Second, 40).unwrap();
        let plan = plan_equipment(&bag, selected, &PendingOperations::default()).unwrap();
        assert_eq!(
            (plan.kind, plan.grid, plan.to),
            (BagEquipmentKind::Equip, "inventory", 7)
        );
        let worn = InventoryModel {
            items: vec![item(Some(7), 7, 2), item(Some(8), 0, 0)],
            ..Default::default()
        };
        assert!(
            resolve_exact(&worn, selected).is_none(),
            "authoritative relocation invalidates the old bag selection"
        );
        let plan = plan_equipment(
            &worn,
            BagSelection::from_item(&worn.items[0]).unwrap(),
            &PendingOperations::default(),
        )
        .unwrap();
        assert_eq!(
            (plan.kind, plan.grid, plan.to),
            (BagEquipmentKind::Remove, "inventory", 1)
        );
        let returned = InventoryModel {
            items: vec![item(Some(7), 1, 0)],
            ..Default::default()
        };
        assert!(
            resolve_exact(&returned, BagSelection::from_item(&worn.items[0]).unwrap()).is_none(),
            "authoritative return invalidates the old equipment selection"
        );
    }

    #[test]
    fn pending_equipment_instance_blocks_replan_until_exact_ack_or_nack() {
        let inventory = InventoryModel {
            items: vec![item(Some(7), 4, 0)],
            ..Default::default()
        };
        let selected = selection_at_slot(&inventory, BagPage::First, 4).unwrap();
        let mut pending = PendingOperations::default();
        let plan = plan_equipment(&inventory, selected, &pending).unwrap();
        assert!(pending.try_begin(plan.pending_key()));
        assert!(plan_equipment(&inventory, selected, &pending).is_none());
        assert!(!pending.try_begin(PendingOperationKey::Equip {
            grid: "inventory".into(),
            unique_id: 7,
            to: 8
        }));
        pending.release(&plan.pending_key());
        assert!(plan_equipment(&inventory, selected, &pending).is_some());
    }
}
