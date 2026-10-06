//! Thin ABI for the shared Bag-to-Belt MoveItem endpoint plan.
//! This adapter owns no gesture, inventory, pending operation or transport.

use mir2_client_core::equipment_pending::InventoryPlacement;
use mir2_client_core::intent::{inventory_bag_capacity, plan_bag_to_belt_move};
use serde::Deserialize;
use serde_json::json;
use wasm_bindgen::prelude::*;

const MAX_INPUT_BYTES: usize = 4096;
const MAX_SAFE_JS_INTEGER: u64 = 9_007_199_254_740_991;

#[wasm_bindgen]
pub fn bag_to_belt_move_abi_version() -> u32 { 1 }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MoveInput {
    version: u32,
    /// Crystal User.Inventory.Length, including the six belt cells.
    inventory_capacity: u16,
    source: SourceInput,
    target_slot: u8,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourceInput {
    container: u8,
    /// Normalized global bag index; Bag2 already begins at index 40.
    slot: u32,
    unique_id: Option<u64>,
}

/// ABI1 input: {version:1,inventoryCapacity,source:{container,slot,uniqueId},targetSlot}.
/// Serde's required typed fields reject duplicates before a value can replace
/// an endpoint. Missing/null UID remains absent and is rejected by the planner.
/// A plan is display/intent metadata; hosts still validate current custody and
/// consume their gesture before reserving and sending the existing MoveItem.
#[wasm_bindgen]
pub fn bag_to_belt_move_plan(input: &str) -> String {
    let resolve = || {
        if input.is_empty() || input.len() > MAX_INPUT_BYTES { return None; }
        let input: MoveInput = serde_json::from_str(input).ok()?;
        if input.version != 1 { return None; }
        inventory_bag_capacity(input.inventory_capacity)?;
        let unique_id = input.source.unique_id?;
        if unique_id > MAX_SAFE_JS_INTEGER { return None; }
        plan_bag_to_belt_move(input.inventory_capacity, InventoryPlacement {
            unique_id: Some(unique_id), container: input.source.container, slot: input.source.slot,
        }, input.target_slot)
    };
    match resolve() {
        Some(plan) => json!({"version":1,"ok":true,"plan":{
            "uniqueId":plan.unique_id,"from":plan.from,"to":plan.to,
        }}).to_string(),
        None => "{\"version\":1,\"ok\":false}".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn request(capacity: u16, slot: u32, unique_id: Option<u64>, target: u8) -> Value {
        json!({"version":1,"inventoryCapacity":capacity,
            "source":{"container":0,"slot":slot,"uniqueId":unique_id},"targetSlot":target})
    }

    fn response(request: &Value) -> Value {
        serde_json::from_str(&bag_to_belt_move_plan(&request.to_string())).unwrap()
    }

    #[test]
    fn abi_raw_endpoints_match_core_goldens_with_zero_and_safe_max_identity() {
        assert_eq!(bag_to_belt_move_abi_version(), 1);
        for (capacity, slot, unique_id, target, from) in [
            (46, 0, 0, 0, 6), (46, 2, 7001, 0, 8), (46, 39, 42, 5, 45),
            (54, 40, 43, 0, 46), (54, 47, 44, 5, 53), (86, 79, MAX_SAFE_JS_INTEGER, 5, 85),
        ] {
            let input = request(capacity, slot, Some(unique_id), target);
            let original = input.clone();
            let result = response(&input);
            assert_eq!(result, json!({"version":1,"ok":true,
                "plan":{"uniqueId":unique_id,"from":from,"to":target}}));
            let core = plan_bag_to_belt_move(capacity,
                InventoryPlacement { unique_id:Some(unique_id), container:0, slot }, target).unwrap();
            assert_eq!(result["plan"]["from"], core.from);
            assert_eq!(result["plan"]["to"], core.to);
            assert_eq!(input, original, "planning cannot rewrite the source");
        }
    }

    #[test]
    fn abi_rejects_unpurchased_bag2_missing_identity_and_arbitrary_endpoints() {
        let rejected = json!({"version":1,"ok":false});
        for (capacity, slot, target) in [
            (46, 40, 0), (47, 40, 0), (54, 48, 0), (86, 80, 0),
            (86, u32::MAX, 0), (46, 0, 6), (46, 0, u8::MAX),
        ] {
            assert_eq!(response(&request(capacity, slot, Some(0), target)), rejected);
        }
        assert_eq!(response(&request(46, 0, None, 0)), rejected);
        let mut absent = request(46, 0, Some(0), 0);
        absent["source"].as_object_mut().unwrap().remove("uniqueId");
        assert_eq!(response(&absent), rejected);
        for container in [1, 2, 3, 4, u8::MAX] {
            let mut input = request(46, 0, Some(0), 0);
            input["source"]["container"] = json!(container);
            assert_eq!(response(&input), rejected);
        }
        assert_eq!(response(&request(46, 0, Some(MAX_SAFE_JS_INTEGER + 1), 0)), rejected);
        assert_eq!(response(&request(46, 0, Some(u64::MAX), 0)), rejected);
    }

    #[test]
    fn abi_requires_canonical_unified_capacity_without_native_legacy_fallback() {
        for capacity in [0, 40, 45, 47, 48, 50, 53, 55, 80, 87, u16::MAX] {
            assert_eq!(response(&request(capacity, 0, Some(0), 0)), json!({"version":1,"ok":false}));
        }
        for capacity in [46, 54, 58, 62, 66, 70, 74, 78, 82, 86] {
            assert_eq!(response(&request(capacity, 0, Some(0), 0))["ok"], true);
        }
    }

    #[test]
    fn abi_strict_typed_fields_reject_duplicates_unknown_fields_and_missing_endpoints() {
        let input = request(46, 2, Some(0), 0);
        let original = input.to_string();
        for (field, replacement) in [
            ("\"version\":1", "\"version\":1,\"version\":1"),
            ("\"inventoryCapacity\":46", "\"inventoryCapacity\":46,\"inventoryCapacity\":86"),
            ("\"targetSlot\":0", "\"targetSlot\":0,\"targetSlot\":5"),
            ("\"container\":0", "\"container\":0,\"container\":1"),
            ("\"slot\":2", "\"slot\":2,\"slot\":40"),
            ("\"uniqueId\":0", "\"uniqueId\":0,\"uniqueId\":1"),
        ] {
            let duplicate = original.replacen(field, replacement, 1);
            assert_ne!(duplicate, original, "duplicate fixture must change the request");
            assert_eq!(bag_to_belt_move_plan(&duplicate), "{\"version\":1,\"ok\":false}");
        }
        for field in ["version", "inventoryCapacity", "source", "targetSlot"] {
            let mut missing = input.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert_eq!(response(&missing)["ok"], false);
        }
        for field in ["container", "slot"] {
            let mut missing = input.clone();
            missing["source"].as_object_mut().unwrap().remove(field);
            assert_eq!(response(&missing)["ok"], false);
        }
        let mut unknown = input.clone(); unknown["extra"] = json!(true);
        assert_eq!(response(&unknown)["ok"], false);
        let mut unknown = input.clone(); unknown["source"]["extra"] = json!(true);
        assert_eq!(response(&unknown)["ok"], false);
        let mut wrong_version = input; wrong_version["version"] = json!(2);
        assert_eq!(response(&wrong_version)["ok"], false);
    }

    #[test]
    fn abi_bounded_json_preserves_numeric_ranges_without_wasm_argument_coercion() {
        for input in ["", "null", "[]", "{}", "{", "true"] {
            assert_eq!(bag_to_belt_move_plan(input), "{\"version\":1,\"ok\":false}");
        }
        let input = request(46, 0, Some(0), 0);
        for (field, value) in [
            ("targetSlot", json!(-1)), ("targetSlot", json!(256)), ("targetSlot", json!(0.5)),
            ("inventoryCapacity", json!(65536)), ("inventoryCapacity", json!("46")),
        ] {
            let mut malformed = input.clone(); malformed[field] = value;
            assert_eq!(response(&malformed)["ok"], false);
        }
        for (field, value) in [
            ("slot", json!(-1)), ("slot", json!(4294967296_u64)), ("slot", json!(0.5)),
            ("container", json!(256)), ("uniqueId", json!(-1)), ("uniqueId", json!("0")),
        ] {
            let mut malformed = input.clone(); malformed["source"][field] = value;
            assert_eq!(response(&malformed)["ok"], false);
        }
        assert_eq!(bag_to_belt_move_plan(&" ".repeat(MAX_INPUT_BYTES + 1)), "{\"version\":1,\"ok\":false}");
        let exact_limit = format!("{}{}", input, " ".repeat(MAX_INPUT_BYTES - input.to_string().len()));
        assert_eq!(exact_limit.len(), MAX_INPUT_BYTES);
        let result: Value = serde_json::from_str(&bag_to_belt_move_plan(&exact_limit)).unwrap();
        assert_eq!(result["ok"], true);
    }
}
