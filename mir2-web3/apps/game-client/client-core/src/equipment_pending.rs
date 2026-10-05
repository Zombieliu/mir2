//! Transport reservation and authoritative-placement barrier for equipment.
//! This ledger never changes inventory contents and never unlocks by elapsed time.

use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EquipmentOperationKind {
    Equip,
    Remove,
}

/// The original normalized grid string is retained for exact host routing.
/// ACK matching follows the legacy protocol's ASCII case-insensitive grid.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EquipmentOperation {
    pub kind: EquipmentOperationKind,
    pub grid: String,
    pub unique_id: u64,
    pub to: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EquipmentReserveError {
    InstanceBusy,
    EquipmentSlotBusy,
    AtCapacity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InventoryPlacement {
    pub unique_id: Option<u64>,
    pub container: u8,
    pub slot: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct InventoryLayout<'a> {
    /// Authoritative number of bag cells, excluding Crystal's six belt cells.
    pub bag_capacity: u16,
    pub placements: &'a [InventoryPlacement],
}

#[derive(Debug, Default)]
pub struct EquipmentPendingLedger {
    awaiting_ack: HashSet<EquipmentOperation>,
    observed_before_ack: HashSet<EquipmentOperation>,
    successful_barriers: HashSet<EquipmentOperation>,
}

impl EquipmentPendingLedger {
    /// `max_entries` is the ledger's allowed share of the caller's total
    /// pending capacity, including success barriers. The caller subtracts its
    /// other pending domains before reserving here.
    pub fn reserve(
        &mut self,
        operation: EquipmentOperation,
        max_entries: usize,
    ) -> Result<(), EquipmentReserveError> {
        if self.has_instance(operation.unique_id) {
            return Err(EquipmentReserveError::InstanceBusy);
        }
        if operation.kind == EquipmentOperationKind::Equip && self.has_equipment_slot(operation.to)
        {
            return Err(EquipmentReserveError::EquipmentSlotBusy);
        }
        if self.len() >= max_entries {
            return Err(EquipmentReserveError::AtCapacity);
        }
        self.awaiting_ack.insert(operation);
        Ok(())
    }

    /// ACK/NACK addresses one in-flight tuple. A successful receipt keeps a
    /// supported operation reserved until its authoritative placement is
    /// observed, unless that observation arrived before the ACK.
    pub fn acknowledge(&mut self, operation: &EquipmentOperation, success: bool) -> bool {
        let stored = self
            .awaiting_ack
            .iter()
            .find(|pending| {
                pending.kind == operation.kind
                    && pending.unique_id == operation.unique_id
                    && pending.to == operation.to
                    && pending.grid.eq_ignore_ascii_case(&operation.grid)
            })
            .cloned();
        let Some(stored) = stored else {
            return false;
        };
        self.awaiting_ack.remove(&stored);
        let observed = self.observed_before_ack.remove(&stored);
        if success && transition_supported(&stored) && !observed {
            self.successful_barriers.insert(stored);
        }
        true
    }

    /// Observe a real old→new authoritative inventory transition. This marks
    /// in-flight operations for a later ACK and releases successful barriers.
    /// Ordinary refreshes, ambiguous UID/cell projections, and unsupported
    /// grids never prove placement.
    pub fn reconcile(&mut self, before: InventoryLayout<'_>, after: InventoryLayout<'_>) -> usize {
        self.observed_before_ack.extend(
            self.awaiting_ack
                .iter()
                .filter(|operation| transition_observed(operation, before, after))
                .cloned(),
        );
        let previous = self.successful_barriers.len();
        self.successful_barriers
            .retain(|operation| !transition_observed(operation, before, after));
        previous - self.successful_barriers.len()
    }

    /// Explicit transport rejection or session cleanup; never a timer.
    pub fn release(&mut self, operation: &EquipmentOperation) -> bool {
        self.observed_before_ack.remove(operation);
        self.awaiting_ack.remove(operation) || self.successful_barriers.remove(operation)
    }

    pub fn clear(&mut self) {
        self.awaiting_ack.clear();
        self.observed_before_ack.clear();
        self.successful_barriers.clear();
    }

    pub fn contains(&self, operation: &EquipmentOperation) -> bool {
        self.awaiting_ack.contains(operation) || self.successful_barriers.contains(operation)
    }

    pub fn has_instance(&self, unique_id: u64) -> bool {
        self.iter()
            .any(|operation| operation.unique_id == unique_id)
    }

    pub fn has_equipment_slot(&self, slot: i32) -> bool {
        self.iter().any(|operation| {
            operation.kind == EquipmentOperationKind::Equip && operation.to == slot
        })
    }

    pub fn len(&self) -> usize {
        self.awaiting_ack.len() + self.successful_barriers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.awaiting_ack.is_empty() && self.successful_barriers.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &EquipmentOperation> {
        self.awaiting_ack
            .iter()
            .chain(self.successful_barriers.iter())
    }

    pub fn observed_len(&self) -> usize {
        self.observed_before_ack.len()
    }
    pub fn success_barrier_len(&self) -> usize {
        self.successful_barriers.len()
    }
}

/// Storage is a separate read model. Unknown and storage grids preserve the
/// old ACK-only behavior rather than creating an unprovable permanent lock.
fn transition_supported(operation: &EquipmentOperation) -> bool {
    if operation.to < 0 {
        return false;
    }
    match operation.kind {
        EquipmentOperationKind::Equip => {
            operation.grid.eq_ignore_ascii_case("inventory")
                || operation.grid.eq_ignore_ascii_case("belt")
        }
        EquipmentOperationKind::Remove => operation.grid.eq_ignore_ascii_case("inventory"),
    }
}

fn transition_observed(
    operation: &EquipmentOperation,
    before: InventoryLayout<'_>,
    after: InventoryLayout<'_>,
) -> bool {
    if !transition_supported(operation) {
        return false;
    }
    let Some(target_slot) = u32::try_from(operation.to).ok() else {
        return false;
    };
    let (source, target) = match operation.kind {
        EquipmentOperationKind::Equip => (
            if operation.grid.eq_ignore_ascii_case("belt") {
                1
            } else {
                0
            },
            2,
        ),
        EquipmentOperationKind::Remove => (2, 0),
    };
    let (Some(old), Some(new)) = (
        unique_by_id(before.placements, operation.unique_id),
        unique_by_id(after.placements, operation.unique_id),
    ) else {
        return false;
    };
    let target_valid = match operation.kind {
        EquipmentOperationKind::Equip => target_slot < 14 && new.slot == target_slot,
        EquipmentOperationKind::Remove => {
            target_slot < u32::from(after.bag_capacity) && new.slot < u32::from(after.bag_capacity)
        }
    };
    target_valid
        && old.container == source
        && new.container == target
        && (old.container != new.container || old.slot != new.slot)
        && cell_unique(before.placements, source, old.slot)
        && cell_unique(after.placements, target, new.slot)
}

fn unique_by_id(items: &[InventoryPlacement], unique_id: u64) -> Option<&InventoryPlacement> {
    let mut matches = items
        .iter()
        .filter(|item| item.unique_id == Some(unique_id));
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn cell_unique(items: &[InventoryPlacement], container: u8, slot: u32) -> bool {
    items
        .iter()
        .filter(|item| item.container == container && item.slot == slot)
        .count()
        == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(kind: EquipmentOperationKind, id: u64, grid: &str, to: i32) -> EquipmentOperation {
        EquipmentOperation {
            kind,
            grid: grid.into(),
            unique_id: id,
            to,
        }
    }
    fn layout(capacity: u16, items: &[InventoryPlacement]) -> InventoryLayout<'_> {
        InventoryLayout {
            bag_capacity: capacity,
            placements: items,
        }
    }
    fn place(id: Option<u64>, container: u8, slot: u32) -> InventoryPlacement {
        InventoryPlacement {
            unique_id: id,
            container,
            slot,
        }
    }

    #[test]
    fn zero_identity_same_instance_and_same_equipment_slot_are_reserved() {
        let mut ledger = EquipmentPendingLedger::default();
        assert_eq!(
            ledger.reserve(op(EquipmentOperationKind::Equip, 0, "inventory", 0), 128),
            Ok(())
        );
        assert_eq!(
            ledger.reserve(op(EquipmentOperationKind::Remove, 0, "inventory", 2), 128),
            Err(EquipmentReserveError::InstanceBusy)
        );
        assert_eq!(
            ledger.reserve(op(EquipmentOperationKind::Equip, 1, "storage", 0), 128),
            Err(EquipmentReserveError::EquipmentSlotBusy)
        );
        assert_eq!(ledger.len(), 1);
    }

    #[test]
    fn exact_ack_grid_case_nack_and_unrelated_tuple() {
        let mut ledger = EquipmentPendingLedger::default();
        let key = op(EquipmentOperationKind::Equip, 7, "inventory", 5);
        ledger.reserve(key.clone(), 128).unwrap();
        assert!(!ledger.acknowledge(&op(EquipmentOperationKind::Equip, 7, "inventory", 6), true));
        assert!(!ledger.acknowledge(&op(EquipmentOperationKind::Remove, 7, "inventory", 5), true));
        assert!(ledger.contains(&key));
        assert!(ledger.acknowledge(&op(EquipmentOperationKind::Equip, 7, "Inventory", 5), false));
        assert!(ledger.is_empty());
    }

    #[test]
    fn snapshot_before_ack_and_ack_before_snapshot_both_clear_only_proven_placement() {
        let old = [place(Some(8), 0, 3)];
        let new = [place(Some(8), 2, 0)];
        let key = op(EquipmentOperationKind::Equip, 8, "inventory", 0);
        let mut ledger = EquipmentPendingLedger::default();
        ledger.reserve(key.clone(), 128).unwrap();
        assert_eq!(ledger.reconcile(layout(40, &old), layout(40, &new)), 0);
        assert_eq!(ledger.observed_len(), 1);
        assert!(ledger.acknowledge(&key, true));
        assert!(ledger.is_empty());
        ledger.reserve(key.clone(), 128).unwrap();
        assert!(ledger.acknowledge(&key, true));
        assert!(ledger.contains(&key));
        assert_eq!(ledger.success_barrier_len(), 1);
        assert_eq!(ledger.reconcile(layout(40, &old), layout(40, &old)), 0);
        assert_eq!(ledger.reconcile(layout(40, &old), layout(40, &new)), 1);
        assert!(ledger.is_empty());
    }

    #[test]
    fn duplicate_uid_or_cell_never_proves_equip() {
        let key = op(EquipmentOperationKind::Equip, 9, "inventory", 0);
        let old = [place(Some(9), 0, 3)];
        let duplicate = [place(Some(9), 2, 0), place(Some(9), 0, 3)];
        let crowded = [place(Some(9), 2, 0), place(Some(10), 2, 0)];
        let mut ledger = EquipmentPendingLedger::default();
        ledger.reserve(key.clone(), 128).unwrap();
        ledger.acknowledge(&key, true);
        assert_eq!(
            ledger.reconcile(layout(40, &old), layout(40, &duplicate)),
            0
        );
        assert_eq!(ledger.reconcile(layout(40, &old), layout(40, &crowded)), 0);
        assert!(ledger.contains(&key));
    }

    #[test]
    fn remove_accepts_coalesced_move_to_any_unique_valid_bag_cell() {
        let key = op(EquipmentOperationKind::Remove, 0, "inventory", 0);
        let old = [place(Some(0), 2, 0)];
        let moved = [place(Some(0), 0, 4)];
        let duplicate_cell = [place(Some(0), 0, 4), place(Some(2), 0, 4)];
        let mut ledger = EquipmentPendingLedger::default();
        ledger.reserve(key.clone(), 128).unwrap();
        ledger.acknowledge(&key, true);
        assert_eq!(
            ledger.reconcile(layout(40, &old), layout(40, &duplicate_cell)),
            0
        );
        assert_eq!(ledger.reconcile(layout(4, &old), layout(4, &moved)), 0);
        assert_eq!(ledger.reconcile(layout(40, &old), layout(40, &moved)), 1);
    }

    #[test]
    fn storage_and_unknown_grids_release_on_ack_without_placement_barrier() {
        let mut ledger = EquipmentPendingLedger::default();
        for (id, grid) in [(10, "storage"), (11, "hero")] {
            let key = op(EquipmentOperationKind::Equip, id, grid, id as i32);
            ledger.reserve(key.clone(), 128).unwrap();
            assert!(ledger.acknowledge(&key, true));
            assert!(!ledger.contains(&key));
        }
    }

    #[test]
    fn barriers_count_against_capacity_and_clear_resets_all_states() {
        let mut ledger = EquipmentPendingLedger::default();
        let key = op(EquipmentOperationKind::Equip, 12, "inventory", 0);
        ledger.reserve(key.clone(), 1).unwrap();
        ledger.acknowledge(&key, true);
        assert_eq!(
            ledger.reserve(op(EquipmentOperationKind::Remove, 13, "inventory", 0), 1),
            Err(EquipmentReserveError::AtCapacity)
        );
        ledger.clear();
        assert!(ledger.is_empty());
        assert_eq!(ledger.observed_len(), 0);
        assert!(ledger.reserve(key, 1).is_ok());
    }
}
