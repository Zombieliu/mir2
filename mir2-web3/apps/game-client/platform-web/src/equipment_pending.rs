//! Small JSON/wasm-bindgen host for the shared equipment pending ledger.
//! Inventory content and transport ownership remain with the browser host.

use std::collections::HashSet;

use mir2_client_core::equipment_pending::{
    EquipmentOperation, EquipmentOperationKind, EquipmentPendingLedger, EquipmentReserveError,
    InventoryLayout, InventoryPlacement,
};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use wasm_bindgen::prelude::*;

const MAX_SAFE_JS_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_OPERATION_JSON: usize = 512;
const MAX_SNAPSHOT_JSON: usize = 65_536;
const MAX_PLACEMENTS: usize = 512;
const MAX_PENDING: usize = 128;

/// Additive capability: Quest's existing `client_core_abi_version() == 1`
/// remains valid for an older, Quest-only package.
#[wasm_bindgen]
pub fn equipment_pending_abi_version() -> u32 {
    1
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum OperationKindInput {
    Equip,
    Remove,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OperationInput {
    kind: OperationKindInput,
    grid: String,
    unique_id: u64,
    to: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AckInput {
    operation: OperationInput,
    success: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UniqueInput {
    unique_id: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SnapshotInput {
    capacity: u16,
    #[serde(default)]
    storage_capacity: Option<u16>,
    placements: Vec<PlacementInput>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlacementInput {
    #[serde(default)]
    unique_id: Option<u64>,
    container: u8,
    slot: u32,
}

struct OwnedLayout {
    bag_capacity: u16,
    placements: Vec<InventoryPlacement>,
}

impl OwnedLayout {
    fn view(&self) -> InventoryLayout<'_> {
        InventoryLayout {
            bag_capacity: self.bag_capacity,
            placements: &self.placements,
        }
    }
}

fn parse<T: DeserializeOwned>(json: &str, max_bytes: usize) -> Result<T, &'static str> {
    if json.len() > max_bytes {
        return Err("invalidInput");
    }
    serde_json::from_str(json).map_err(|_| "invalidInput")
}

fn safe_id(unique_id: u64) -> Result<u64, &'static str> {
    (unique_id <= MAX_SAFE_JS_INTEGER)
        .then_some(unique_id)
        .ok_or("invalidInput")
}

fn canonical_capacity(capacity: u16) -> Option<u16> {
    (capacity == 46 || ((54..=86).contains(&capacity) && (capacity - 54) % 4 == 0))
        .then_some(capacity - 6)
}

fn validate_snapshot(input: SnapshotInput) -> Result<OwnedLayout, &'static str> {
    let bag_capacity = canonical_capacity(input.capacity).ok_or("invalidCapacity")?;
    if input.placements.len() > MAX_PLACEMENTS {
        return Err("invalidPlacement");
    }
    let storage_capacity = match input.storage_capacity {
        Some(1..=256) => input.storage_capacity,
        None => None,
        _ => return Err("invalidCapacity"),
    };
    let mut cells = HashSet::new();
    let mut identities = HashSet::new();
    let mut placements = Vec::with_capacity(input.placements.len());
    for item in input.placements {
        let limit = match item.container {
            0 => u32::from(bag_capacity),
            1 => 6,
            2 => 14,
            3 => 40,
            4 => u32::from(storage_capacity.ok_or("invalidPlacement")?),
            _ => return Err("invalidPlacement"),
        };
        if item.slot >= limit || !cells.insert((item.container, item.slot)) {
            return Err("invalidPlacement");
        }
        let unique_id = item.unique_id.map(safe_id).transpose()?;
        if unique_id.is_some_and(|id| !identities.insert(id)) {
            return Err("invalidPlacement");
        }
        placements.push(InventoryPlacement {
            unique_id,
            container: item.container,
            slot: item.slot,
        });
    }
    Ok(OwnedLayout {
        bag_capacity,
        placements,
    })
}

fn operation(
    input: OperationInput,
    bag_capacity: u16,
    require_current_destination: bool,
) -> Result<EquipmentOperation, &'static str> {
    let unique_id = safe_id(input.unique_id)?;
    if input.grid.len() > 24 || !input.grid.is_ascii() || input.grid.trim() != input.grid {
        return Err("invalidOperation");
    }
    let kind = match input.kind {
        OperationKindInput::Equip => EquipmentOperationKind::Equip,
        OperationKindInput::Remove => EquipmentOperationKind::Remove,
    };
    let grid = if input.grid.eq_ignore_ascii_case("inventory") {
        "inventory"
    } else if input.grid.eq_ignore_ascii_case("belt") {
        "belt"
    } else if input.grid.eq_ignore_ascii_case("storage") {
        "storage"
    } else {
        return Err("invalidOperation");
    };
    let valid = match kind {
        EquipmentOperationKind::Equip => (0..14).contains(&input.to),
        EquipmentOperationKind::Remove => {
            // ACKs may follow a legal capacity resize. The original destination
            // was checked at reservation time; only new reservations use the
            // current bag limit.
            let limit = if require_current_destination {
                i32::from(bag_capacity)
            } else {
                80
            };
            grid == "inventory" && (0..limit).contains(&input.to)
        }
    };
    if !valid {
        return Err("invalidOperation");
    }
    Ok(EquipmentOperation {
        kind,
        grid: grid.to_owned(),
        unique_id,
        to: input.to,
    })
}

fn source_placement_valid(op: &EquipmentOperation, layout: &OwnedLayout) -> bool {
    let source_container = match op.kind {
        EquipmentOperationKind::Remove => 2,
        EquipmentOperationKind::Equip => match op.grid.as_str() {
            "inventory" => 0,
            "belt" => 1,
            "storage" => 4,
            _ => return false,
        },
    };
    let mut matches = layout
        .placements
        .iter()
        .filter(|item| item.unique_id == Some(op.unique_id));
    let Some(source) = matches.next() else {
        return false;
    };
    if matches.next().is_some() || source.container != source_container {
        return false;
    }
    if op.kind == EquipmentOperationKind::Remove {
        let target = u32::try_from(op.to).expect("validated nonnegative target");
        if layout
            .placements
            .iter()
            .any(|item| item.container == 0 && item.slot == target)
        {
            return false;
        }
    }
    true
}

fn response(value: Value) -> String {
    serde_json::to_string(&value).expect("bounded JSON result serializes")
}

#[wasm_bindgen]
pub struct EquipmentPendingBridge {
    ledger: EquipmentPendingLedger,
    layout: Option<OwnedLayout>,
}

#[wasm_bindgen]
impl EquipmentPendingBridge {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            ledger: EquipmentPendingLedger::default(),
            layout: None,
        }
    }

    /// A snapshot is fully parsed before any ledger or layout mutation.
    /// The first valid snapshot is a baseline; later ones reconcile transitions.
    pub fn replace_snapshot(&mut self, input_json: &str) -> String {
        let parsed =
            parse::<SnapshotInput>(input_json, MAX_SNAPSHOT_JSON).and_then(validate_snapshot);
        let next = match parsed {
            Ok(next) => next,
            Err(error) => {
                return response(json!({"ok":false,"error":error,"pending":self.ledger.len()}));
            }
        };
        let released = self
            .layout
            .as_ref()
            .map_or(0, |old| self.ledger.reconcile(old.view(), next.view()));
        self.layout = Some(next);
        response(json!({"ok":true,"released":released,"pending":self.ledger.len()}))
    }

    pub fn reserve(&mut self, input_json: &str) -> String {
        let Some(layout) = self.layout.as_ref() else {
            return response(json!({"ok":false,"error":"notReady","pending":self.ledger.len()}));
        };
        let op = parse::<OperationInput>(input_json, MAX_OPERATION_JSON)
            .and_then(|input| operation(input, layout.bag_capacity, true));
        let op = match op {
            Ok(op) => op,
            Err(error) => {
                return response(json!({"ok":false,"error":error,"pending":self.ledger.len()}));
            }
        };
        if !source_placement_valid(&op, layout) {
            return response(
                json!({"ok":false,"error":"placementMismatch","pending":self.ledger.len()}),
            );
        }
        match self.ledger.reserve(op, MAX_PENDING) {
            Ok(()) => response(json!({"ok":true,"pending":self.ledger.len()})),
            Err(error) => {
                let reason = match error {
                    EquipmentReserveError::InstanceBusy => "instanceBusy",
                    EquipmentReserveError::EquipmentSlotBusy => "equipmentSlotBusy",
                    EquipmentReserveError::AtCapacity => "atCapacity",
                };
                response(json!({"ok":false,"error":reason,"pending":self.ledger.len()}))
            }
        }
    }

    pub fn acknowledge(&mut self, input_json: &str) -> String {
        let Some(layout) = self.layout.as_ref() else {
            return response(json!({"ok":false,"error":"notReady","pending":self.ledger.len()}));
        };
        let parsed = parse::<AckInput>(input_json, MAX_OPERATION_JSON).and_then(|ack| {
            operation(ack.operation, layout.bag_capacity, false).map(|op| (op, ack.success))
        });
        let (op, success) = match parsed {
            Ok(value) => value,
            Err(error) => {
                return response(json!({"ok":false,"error":error,"pending":self.ledger.len()}));
            }
        };
        let matched = self.ledger.acknowledge(&op, success);
        response(json!({"ok":true,"matched":matched,"pending":self.ledger.len()}))
    }

    /// Called only when the host can prove that no bytes were sent.
    pub fn release_unsent(&mut self, input_json: &str) -> String {
        let Some(layout) = self.layout.as_ref() else {
            return response(json!({"ok":false,"error":"notReady","pending":self.ledger.len()}));
        };
        let op = parse::<OperationInput>(input_json, MAX_OPERATION_JSON)
            .and_then(|input| operation(input, layout.bag_capacity, false));
        let op = match op {
            Ok(op) => op,
            Err(error) => {
                return response(json!({"ok":false,"error":error,"pending":self.ledger.len()}));
            }
        };
        let released = self.ledger.release(&op);
        response(json!({"ok":true,"released":released,"pending":self.ledger.len()}))
    }

    pub fn contains(&self, input_json: &str) -> String {
        let Some(layout) = self.layout.as_ref() else {
            return response(json!({"ok":false,"error":"notReady","pending":self.ledger.len()}));
        };
        let op = parse::<OperationInput>(input_json, MAX_OPERATION_JSON)
            .and_then(|input| operation(input, layout.bag_capacity, false));
        match op {
            Ok(op) => response(
                json!({"ok":true,"reserved":self.ledger.contains(&op),"pending":self.ledger.len()}),
            ),
            Err(error) => response(json!({"ok":false,"error":error,"pending":self.ledger.len()})),
        }
    }

    pub fn has_instance(&self, input_json: &str) -> String {
        let input = parse::<UniqueInput>(input_json, MAX_OPERATION_JSON)
            .and_then(|input| safe_id(input.unique_id));
        match input {
            Ok(unique_id) => response(
                json!({"ok":true,"reserved":self.ledger.has_instance(unique_id),"pending":self.ledger.len()}),
            ),
            Err(error) => response(json!({"ok":false,"error":error,"pending":self.ledger.len()})),
        }
    }

    pub fn status(&self) -> String {
        response(json!({
            "ok":true,
            "ready":self.layout.is_some(),
            "pending":self.ledger.len(),
            "barriers":self.ledger.success_barrier_len(),
        }))
    }
}

impl Default for EquipmentPendingBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(json: String) -> Value {
        serde_json::from_str(&json).unwrap()
    }

    fn snapshot(placements: &str) -> String {
        format!(r#"{{"capacity":46,"placements":{placements}}}"#)
    }

    #[test]
    fn ack_then_snapshot_keeps_barrier_and_rejects_duplicate_target() {
        let mut bridge = EquipmentPendingBridge::new();
        assert_eq!(
            value(bridge.replace_snapshot(&snapshot(
                r#"[{"uniqueId":0,"container":0,"slot":4},{"uniqueId":4,"container":0,"slot":5}]"#
            )))["ok"],
            true
        );
        let op = r#"{"kind":"equip","grid":"inventory","uniqueId":0,"to":0}"#;
        assert_eq!(value(bridge.reserve(op))["ok"], true);
        let competing = r#"{"kind":"equip","grid":"inventory","uniqueId":4,"to":0}"#;
        assert_eq!(
            value(bridge.reserve(competing))["error"],
            "equipmentSlotBusy"
        );
        assert_eq!(value(bridge.acknowledge(r#"{"operation":{"kind":"equip","grid":"inventory","uniqueId":0,"to":0},"success":true}"#))["pending"], 1);
        assert_eq!(value(bridge.status())["barriers"], 1);
        assert_eq!(
            value(bridge.replace_snapshot(&snapshot(
                r#"[{"uniqueId":0,"container":2,"slot":0},{"uniqueId":4,"container":0,"slot":5}]"#
            )))["pending"],
            0
        );
    }

    #[test]
    fn snapshot_then_ack_requires_exact_tuple() {
        let mut bridge = EquipmentPendingBridge::new();
        bridge.replace_snapshot(&snapshot(r#"[{"uniqueId":4,"container":0,"slot":4}]"#));
        bridge.reserve(r#"{"kind":"equip","grid":"inventory","uniqueId":4,"to":0}"#);
        assert_eq!(
            value(bridge.replace_snapshot(&snapshot(r#"[{"uniqueId":4,"container":2,"slot":0}]"#)))
                ["pending"],
            1
        );
        assert_eq!(value(bridge.acknowledge(r#"{"operation":{"kind":"equip","grid":"inventory","uniqueId":4,"to":1},"success":true}"#))["matched"], false);
        assert_eq!(value(bridge.acknowledge(r#"{"operation":{"kind":"equip","grid":"Inventory","uniqueId":4,"to":0},"success":true}"#))["pending"], 0);
    }

    #[test]
    fn invalid_snapshot_never_releases_existing_lock_or_loses_zero_identity() {
        let mut bridge = EquipmentPendingBridge::new();
        bridge.replace_snapshot(&snapshot(r#"[{"uniqueId":0,"container":2,"slot":0}]"#));
        assert_eq!(
            value(bridge.reserve(r#"{"kind":"remove","grid":"inventory","uniqueId":0,"to":4}"#))
                ["ok"],
            true
        );
        assert_eq!(
            value(bridge.replace_snapshot(&snapshot(
                r#"[{"uniqueId":0,"container":0,"slot":4},{"uniqueId":0,"container":2,"slot":0}]"#
            )))["ok"],
            false
        );
        assert_eq!(value(bridge.status())["pending"], 1);
        for invalid in [
            r#"{"kind":"remove","grid":"inventory","uniqueId":9007199254740992,"to":4}"#,
            r#"{"kind":"remove","grid":"inventory","uniqueId":-1,"to":4}"#,
            r#"{"kind":"remove","grid":"inventory","uniqueId":0.5,"to":4}"#,
        ] {
            assert_eq!(value(bridge.reserve(invalid))["ok"], false);
            assert_eq!(value(bridge.status())["pending"], 1);
        }
    }

    #[test]
    fn storage_requires_current_source_and_releases_on_ack() {
        let mut bridge = EquipmentPendingBridge::new();
        let stored = r#"{"capacity":46,"storageCapacity":64,"placements":[{"uniqueId":5,"container":4,"slot":3}]}"#;
        assert_eq!(value(bridge.replace_snapshot(stored))["ok"], true);
        assert_eq!(
            value(bridge.reserve(r#"{"kind":"equip","grid":"inventory","uniqueId":5,"to":1}"#))
                ["error"],
            "placementMismatch"
        );
        let operation = r#"{"kind":"equip","grid":"storage","uniqueId":5,"to":1}"#;
        assert_eq!(value(bridge.reserve(operation))["ok"], true);
        assert_eq!(value(bridge.acknowledge(r#"{"operation":{"kind":"equip","grid":"storage","uniqueId":5,"to":1},"success":true}"#))["pending"], 0);
    }

    #[test]
    fn resize_does_not_make_original_remove_ack_unmatchable() {
        let mut bridge = EquipmentPendingBridge::new();
        bridge.replace_snapshot(
            r#"{"capacity":54,"placements":[{"uniqueId":0,"container":2,"slot":0}]}"#,
        );
        let operation = r#"{"kind":"remove","grid":"inventory","uniqueId":0,"to":47}"#;
        assert_eq!(value(bridge.reserve(operation))["ok"], true);
        assert_eq!(
            value(bridge.replace_snapshot(
                r#"{"capacity":46,"placements":[{"uniqueId":0,"container":2,"slot":0}]}"#
            ))["ok"],
            true
        );
        assert_eq!(value(bridge.acknowledge(r#"{"operation":{"kind":"remove","grid":"inventory","uniqueId":0,"to":47},"success":false}"#))["matched"], true);
    }
}
