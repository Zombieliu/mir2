//! Thin ABI for the shared Bag-to-Belt MoveItem endpoint plan.
//! This adapter owns no gesture, inventory, pending operation or transport.

use mir2_client_core::equipment_pending::InventoryPlacement;
use mir2_client_core::intent::{inventory_bag_capacity, plan_bag_to_belt_move};
#[cfg(test)]
use serde::Deserialize;
use serde_json::{json, Value};
use crate::item_tooltip::StrictJsonValue;
use wasm_bindgen::prelude::*;

const MAX_INPUT_BYTES: usize = 4096;
const MAX_SAFE_JS_INTEGER: u64 = 9_007_199_254_740_991;

#[wasm_bindgen]
pub fn bag_to_belt_move_abi_version() -> u32 { 1 }

#[cfg_attr(test, derive(Debug, Deserialize, PartialEq))]
#[cfg_attr(test, serde(rename_all = "camelCase", deny_unknown_fields))]
struct MoveInput {
    version: u32,
    /// Crystal User.Inventory.Length, including the six belt cells.
    inventory_capacity: u16,
    source: SourceInput,
    target_slot: u8,
}

#[cfg_attr(test, derive(Debug, Deserialize, PartialEq))]
#[cfg_attr(test, serde(rename_all = "camelCase", deny_unknown_fields))]
struct SourceInput {
    container: u8,
    /// Normalized global bag index; Bag2 already begins at index 40.
    slot: u32,
    unique_id: Option<u64>,
}

// Preserve serde's positional structs as well as its strict object fields.
fn input_shape(value: &Value, allowed: &[&str], min: usize, max: usize) -> Option<()> {
    match value {
        Value::Object(fields) if fields.keys().all(|key| allowed.contains(&key.as_str())) => Some(()),
        Value::Array(fields) if (min..=max).contains(&fields.len()) => Some(()),
        _ => None,
    }
}

fn input_field<'a>(value: &'a Value, key: &str, index: usize) -> Option<&'a Value> {
    match value {
        Value::Object(fields) => fields.get(key),
        Value::Array(fields) => fields.get(index),
        _ => None,
    }
}

fn parse_move_input(value: &Value) -> Option<MoveInput> {
    input_shape(value, &["version", "inventoryCapacity", "source", "targetSlot"], 4, 4)?;
    let version = u32::try_from(input_field(value, "version", 0)?.as_u64()?).ok()?;
    let inventory_capacity = u16::try_from(input_field(value, "inventoryCapacity", 1)?.as_u64()?).ok()?;
    let source = input_field(value, "source", 2)?;
    input_shape(source, &["container", "slot", "uniqueId"], 3, 3)?;
    let container = u8::try_from(input_field(source, "container", 0)?.as_u64()?).ok()?;
    let slot = u32::try_from(input_field(source, "slot", 1)?.as_u64()?).ok()?;
    let unique_id = match input_field(source, "uniqueId", 2) {
        None | Some(Value::Null) => None,
        Some(value) => Some(value.as_u64()?),
    };
    let target_slot = u8::try_from(input_field(value, "targetSlot", 3)?.as_u64()?).ok()?;
    Some(MoveInput { version, inventory_capacity, source: SourceInput { container, slot, unique_id }, target_slot })
}

/// ABI1 input: {version:1,inventoryCapacity,source:{container,slot,uniqueId},targetSlot}.
/// The shared strict JSON visitor rejects duplicates before a value can replace
/// an endpoint. Missing/null UID remains absent and is rejected by the planner.
/// A plan is display/intent metadata; hosts still validate current custody and
/// consume their gesture before reserving and sending the existing MoveItem.
#[wasm_bindgen]
pub fn bag_to_belt_move_plan(input: &str) -> String {
    let resolve = || {
        if input.is_empty() || input.len() > MAX_INPUT_BYTES { return None; }
        let StrictJsonValue(value) = serde_json::from_str::<StrictJsonValue<256>>(input).ok()?;
        let input = parse_move_input(&value)?;
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
    #[test]
    fn shared_value_decoder_matches_typed_facts_for_objects_arrays_and_numeric_options() {
        let base = request(54, 40, Some(0), 5);
        let mut cases = vec![base.clone(), json!([1,54,[0,40,0],5]),
            json!([1,54,{"container":0,"slot":40,"uniqueId":0},5])];
        for source in [json!([0,40]),json!([0,40,null]),json!([0,40,0]),
            json!({"container":0,"slot":40}),json!({"container":0,"slot":40,"uniqueId":null})] {
            let mut input = base.clone(); input["source"] = source.clone(); cases.push(input);
            cases.push(json!([1,54,source,5]));
        }
        for (field, value) in [("version",json!(u32::MAX)),("version",json!(4294967296_u64)),
            ("version",json!(1.0)),("inventoryCapacity",json!(u16::MAX)),
            ("inventoryCapacity",json!(65536)),("inventoryCapacity",json!("54")),
            ("targetSlot",json!(u8::MAX)),("targetSlot",json!(256)),("targetSlot",json!(-1)),
            ("targetSlot",json!(5.0)),("source",Value::Null),("unknown",json!(true))] {
            let mut input = base.clone(); input[field] = value; cases.push(input);
        }
        for field in ["version","inventoryCapacity","source","targetSlot"] {
            let mut input = base.clone(); input.as_object_mut().unwrap().remove(field); cases.push(input);
            let mut input = base.clone(); input[field] = Value::Null; cases.push(input);
        }
        for (field, value) in [("container",json!(u8::MAX)),("container",json!(256)),
            ("container",json!(0.0)),("slot",json!(u32::MAX)),("slot",json!(4294967296_u64)),
            ("slot",json!(-1)),("slot",json!(40.0)),("uniqueId",json!(u64::MAX)),
            ("uniqueId",json!(MAX_SAFE_JS_INTEGER)),("uniqueId",json!(MAX_SAFE_JS_INTEGER+1)),
            ("uniqueId",json!(-1)),("uniqueId",json!(0.0)),("uniqueId",json!("0")),
            ("unknown",Value::Null)] {
            let mut input = base.clone(); input["source"][field] = value; cases.push(input);
        }
        for source in [json!([]),json!([0]),json!([0,40,0,null]),json!([null,40,0]),
            json!([0,null,0]),json!([0,40,true]),json!([0.0,40,0]),json!([0,40.0,0])] {
            cases.push(json!([1,54,source,5]));
        }
        cases.extend([json!([]),json!([1,54,[0,40,0]]),json!([1,54,[0,40,0],5,null]),Value::Null]);
        for value in cases {
            let old = serde_json::from_value::<MoveInput>(value.clone());
            let new = parse_move_input(&value);
            match (old, new) {
                (Ok(old), Some(new)) => assert_eq!(old, new, "input: {value}"),
                (Err(_), None) => {},
                (old, new) => panic!("decoder disagreement for {value}: old={old:?}, new={new:?}"),
            }
        }
    }

    #[test]
    fn shared_strict_codec_matches_typed_full_output_for_wire_aliases_duplicates_and_byte_bounds() {
        let typed_output = |input: &str| -> String {
            if input.is_empty() || input.len() > MAX_INPUT_BYTES { return "{\"version\":1,\"ok\":false}".to_owned(); }
            let Ok(old) = serde_json::from_str::<MoveInput>(input) else {
                return "{\"version\":1,\"ok\":false}".to_owned();
            };
            // Feed every old typed fact to the unchanged real Core/output path.
            let canonical = json!({"version":old.version,"inventoryCapacity":old.inventory_capacity,
                "source":{"container":old.source.container,"slot":old.source.slot,"uniqueId":old.source.unique_id},
                "targetSlot":old.target_slot});
            bag_to_belt_move_plan(&canonical.to_string())
        };
        let base = request(54,40,Some(0),5).to_string();
        let mut inputs = vec![base.clone(),
            r#"[1,54,[0,40,0],5]"#.to_owned(),
            r#"[1,54,{"container":0,"slot":40,"uniqueId":0},5]"#.to_owned(),
            r#"{"version":1,"inventoryCapacity":54,"source":[0,40,0],"targetSlot":5}"#.to_owned(),
            r#"[1,54,[0,40],5]"#.to_owned(), r#"[1,54,[0,40,null],5]"#.to_owned(),
            r#"[1,54,[0,40,0,null],5]"#.to_owned(), r#"[1,54,[0,40,0],5,null]"#.to_owned(),
            r#"[1,54,[0,40.0,0],5]"#.to_owned(), r#"[1,54,[0,40,-0],5]"#.to_owned(),
            r#"[1,54,[0,40,9007199254740991],5]"#.to_owned(),
            r#"[1,54,[0,40,9007199254740992],5]"#.to_owned(),
            r#"[1,54,[0,40,18446744073709551615],5]"#.to_owned(),
            r#"[1,54,[0,40,18446744073709551616],5]"#.to_owned(),
            r#"[1,54,[0,4294967296,0],5]"#.to_owned(),
            r#"[1,65536,[0,40,0],5]"#.to_owned(), r#"[1,54,[256,40,0],5]"#.to_owned(),
            r#"[1,54,[0,40,"0"],5]"#.to_owned(), r#"[1,54,[0,40,0],256]"#.to_owned(),
        ];
        for (field, replacement) in [
            ("\"version\":1","\"\\u0076ersion\":1"),
            ("\"version\":1","\"version\":1,\"\\u0076ersion\":1"),
            ("\"uniqueId\":0","\"uniqueId\":null,\"uniqueId\":0"),
            ("\"uniqueId\":0","\"uniqueId\":0,\"\\u0075niqueId\":1"),
            ("\"slot\":40","\"slot\":40,\"unknown\":false"),
            ("\"version\":1","\"version\":1,\"unknown\":true"),
            ("\"version\":1","\"version\":1.0"),
        ] {
            let input = base.replacen(field,replacement,1);
            assert_ne!(input,base,"wire fixture must change"); inputs.push(input);
        }
        let exact_limit = format!("{}{}",base," ".repeat(MAX_INPUT_BYTES-base.len()));
        assert_eq!(exact_limit.len(),MAX_INPUT_BYTES);
        inputs.push(exact_limit.clone()); inputs.push(format!("{exact_limit} "));
        for input in inputs { assert_eq!(bag_to_belt_move_plan(&input),typed_output(&input),"wire input: {input}"); }
        assert_eq!(bag_to_belt_move_plan(r#"[1,54,[0,40,0],5]"#),
            json!({"version":1,"ok":true,"plan":{"uniqueId":0,"from":46,"to":5}}).to_string());
        assert_eq!(serde_json::from_str::<Value>(&bag_to_belt_move_plan(&exact_limit)).unwrap()["ok"],true);
        assert_eq!(bag_to_belt_move_plan(&format!("{exact_limit} ")),"{\"version\":1,\"ok\":false}");
    }

}
