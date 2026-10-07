//! Optional renderer-free ABI for shared blocked-walk fishing decisions.
//! Returned Cast clock metadata is a proposal, not custody or send permission.
//! No persistent clock, gesture, transport or game state is created here.

use mir2_client_core::fishing_click::{
    decide_fishing_click, fishing_walk_candidates, fishing_water_target,
    FishingClickDecision, FishingClickFacts, FishingWaterCell,
};
#[cfg(test)]
use serde::Deserialize;
use serde_json::{json, Value};
use wasm_bindgen::prelude::*;

const MAX_INPUT_BYTES: usize = 4096;
const MAX_SAFE_JS_INTEGER: u64 = 9_007_199_254_740_991;
const REJECTED: &str = "{\"version\":1,\"ok\":false}";

#[wasm_bindgen]
pub fn fishing_click_abi_version() -> u32 { 1 }

#[derive(Clone, Copy)]
#[cfg_attr(test, derive(Debug, PartialEq, Deserialize))]
#[cfg_attr(test, serde(deny_unknown_fields))]
struct CellInput {
    x: i32,
    y: i32,
}
impl CellInput {
    fn pair(self) -> (i32, i32) { (self.x, self.y) }
}

#[cfg_attr(test, derive(Debug, PartialEq, Deserialize))]
#[cfg_attr(test, serde(deny_unknown_fields))]
struct TargetsInput {
    version: u32,
    origin: CellInput,
    direction: u8,
}

#[cfg_attr(test, derive(Debug, PartialEq, Deserialize))]
#[cfg_attr(test, serde(deny_unknown_fields))]
struct WaterInput {
    cell: CellInput,
    light: u8,
}

#[cfg_attr(test, derive(Debug, PartialEq, Deserialize))]
#[cfg_attr(test, serde(rename_all = "camelCase", deny_unknown_fields))]
struct DecisionInput {
    version: u32,
    origin: CellInput,
    direction: u8,
    requested_walk: bool,
    auto_route: bool,
    walk_blocked: [Option<bool>; 3],
    rod_present: Option<bool>,
    water: Option<WaterInput>,
    facing_matches: Option<bool>,
    standing: Option<bool>,
    fishing: Option<bool>,
    transform_type: Option<i16>,
    now_ms: u64,
    last_cast_ms: u64,
}

// Reuse the existing bounded, duplicate-rejecting parser so ABI DTO derives are
// needed only by the test oracle, not by the presentation WASM artifact.
fn strict_input(input: &str) -> Option<Value> {
    if input.is_empty() || input.len() > MAX_INPUT_BYTES { return None; }
    serde_json::from_str::<crate::item_tooltip::StrictJsonValue<256>>(input).ok().map(|value| value.0)
}
// Serde-derived structs also accepted exact-length positional arrays. Preserve
// that ABI input form alongside exact object keys, including nested carriers.
fn input_fields<'a>(value: &'a Value, allowed: &[&str]) -> Option<&'a Value> {
    match value {
        Value::Object(fields) if fields.keys().all(|key| allowed.contains(&key.as_str())) => {},
        Value::Array(fields) if fields.len() == allowed.len() => {},
        _ => return None,
    }
    Some(value)
}
fn field<'a>(value: &'a Value, key: &str, index: usize) -> Option<&'a Value> {
    match value {
        Value::Object(fields) => fields.get(key),
        Value::Array(fields) => fields.get(index),
        _ => None,
    }
}
fn cell_input(value: &Value) -> Option<CellInput> {
    let fields = input_fields(value, &["x", "y"])?;
    Some(CellInput {
        x: i32::try_from(field(fields, "x", 0)?.as_i64()?).ok()?,
        y: i32::try_from(field(fields, "y", 1)?.as_i64()?).ok()?,
    })
}
fn optional_bool(value: Option<&Value>) -> Option<Option<bool>> {
    match value {
        None | Some(Value::Null) => Some(None),
        Some(value) => Some(Some(value.as_bool()?)),
    }
}
fn water_input(value: &Value) -> Option<WaterInput> {
    let fields = input_fields(value, &["cell", "light"])?;
    Some(WaterInput {
        cell: cell_input(field(fields, "cell", 0)?)?,
        light: u8::try_from(field(fields, "light", 1)?.as_u64()?).ok()?,
    })
}
fn targets_input(input: &str) -> Option<TargetsInput> {
    let input = strict_input(input)?;
    let fields = input_fields(&input, &["version", "origin", "direction"])?;
    Some(TargetsInput {
        version: u32::try_from(field(fields, "version", 0)?.as_u64()?).ok()?,
        origin: cell_input(field(fields, "origin", 1)?)?,
        direction: u8::try_from(field(fields, "direction", 2)?.as_u64()?).ok()?,
    })
}
fn decision_input(input: &str) -> Option<DecisionInput> {
    let input = strict_input(input)?;
    let fields = input_fields(&input, &["version", "origin", "direction", "requestedWalk",
        "autoRoute", "walkBlocked", "rodPresent", "water", "facingMatches", "standing",
        "fishing", "transformType", "nowMs", "lastCastMs"])?;
    let blocked = field(fields, "walkBlocked", 5)?.as_array()?;
    if blocked.len() != 3 { return None; }
    Some(DecisionInput {
        version: u32::try_from(field(fields, "version", 0)?.as_u64()?).ok()?,
        origin: cell_input(field(fields, "origin", 1)?)?,
        direction: u8::try_from(field(fields, "direction", 2)?.as_u64()?).ok()?,
        requested_walk: field(fields, "requestedWalk", 3)?.as_bool()?,
        auto_route: field(fields, "autoRoute", 4)?.as_bool()?,
        walk_blocked: [optional_bool(Some(&blocked[0]))?, optional_bool(Some(&blocked[1]))?,
            optional_bool(Some(&blocked[2]))?],
        rod_present: optional_bool(field(fields, "rodPresent", 6))?,
        water: match field(fields, "water", 7) {
            None | Some(Value::Null) => None,
            Some(value) => Some(water_input(value)?),
        },
        facing_matches: optional_bool(field(fields, "facingMatches", 8))?,
        standing: optional_bool(field(fields, "standing", 9))?,
        fishing: optional_bool(field(fields, "fishing", 10))?,
        transform_type: match field(fields, "transformType", 11) {
            None | Some(Value::Null) => None,
            Some(value) => Some(i16::try_from(value.as_i64()?).ok()?),
        },
        now_ms: field(fields, "nowMs", 12)?.as_u64()?,
        last_cast_ms: field(fields, "lastCastMs", 13)?.as_u64()?,
    })
}

/// Strict ABI1 targets; all coordinate and direction planning delegates to Core.
/// Required fields reject duplicates, unknown keys and unrepresentable
/// integers before any geometry can be returned.
#[wasm_bindgen]
pub fn fishing_click_targets(input: &str) -> String {
    let resolve = || {
        let input = targets_input(input)?;
        if input.version != 1 { return None; }
        let origin = input.origin.pair();
        let candidates = fishing_walk_candidates(origin, input.direction)?;
        let water = fishing_water_target(origin, input.direction)?;
        Some(json!({"version":1,"ok":true,
            "walkCandidates":candidates.map(|candidate| json!({"direction":candidate.direction,
                "cell":{"x":candidate.cell.0,"y":candidate.cell.1}})),
            "waterTarget":{"x":water.0,"y":water.1}}).to_string())
    };
    resolve().unwrap_or_else(|| REJECTED.to_owned())
}

/// ABI1 explicit facts. Missing/null Option fields stay unknown. Both clocks are
/// safe JavaScript integers; malformed geometry is rejected rather than emitted
/// as a valid no-action decision. A valid unknown fact produces type none.
/// Cast returns lastCastMs=nowMs but does not persist or claim that clock. The
/// host must still recheck current custody before dispatch and then store it.
#[wasm_bindgen]
pub fn fishing_click_decision(input: &str) -> String {
    let resolve = || {
        let input = decision_input(input)?;
        if input.version != 1 || input.now_ms > MAX_SAFE_JS_INTEGER
            || input.last_cast_ms > MAX_SAFE_JS_INTEGER { return None; }
        let origin = input.origin.pair();
        fishing_walk_candidates(origin, input.direction)?;
        fishing_water_target(origin, input.direction)?;
        let facts = FishingClickFacts {
            origin, direction: input.direction, requested_walk: input.requested_walk,
            auto_route: input.auto_route, walk_blocked: input.walk_blocked,
            rod_present: input.rod_present,
            water: input.water.map(|water| FishingWaterCell { cell: water.cell.pair(), light: water.light }),
            facing_matches: input.facing_matches, standing: input.standing,
            fishing: input.fishing, transform_type: input.transform_type,
        };
        let decision = match decide_fishing_click(&facts, input.now_ms, input.last_cast_ms) {
            FishingClickDecision::None => json!({"type":"none"}),
            FishingClickDecision::Turn { direction, delay_ms } => json!({"type":"turn",
                "direction":direction,"delayMs":delay_ms}),
            FishingClickDecision::Cast => json!({"type":"cast","lastCastMs":input.now_ms}),
        };
        Some(json!({"version":1,"ok":true,"decision":decision}).to_string())
    };
    resolve().unwrap_or_else(|| REJECTED.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn request() -> Value {
        json!({"version":1,"origin":{"x":10,"y":10},"direction":2,
            "requestedWalk":true,"autoRoute":false,"walkBlocked":[true,true,true],
            "rodPresent":true,"water":{"cell":{"x":13,"y":10},"light":100},
            "facingMatches":true,"standing":true,"fishing":false,"transformType":0,
            "nowMs":1000,"lastCastMs":0})
    }
    fn response(input: &Value) -> Value {
        serde_json::from_str(&fishing_click_decision(&input.to_string())).unwrap()
    }
    fn rejected() -> Value { json!({"version":1,"ok":false}) }
    fn none() -> Value { json!({"version":1,"ok":true,"decision":{"type":"none"}}) }
    fn cast(now: u64) -> Value { json!({"version":1,"ok":true,"decision":{"type":"cast","lastCastMs":now}}) }

    #[test]
    fn targets_abi_matches_real_core_and_all_eight_three_tile_goldens() {
        assert_eq!(fishing_click_abi_version(), 1);
        let expected_water = [(10, 7), (13, 7), (13, 10), (13, 13), (10, 13), (7, 13), (7, 10), (7, 7)];
        for (direction, water) in expected_water.into_iter().enumerate() {
            let direction = direction as u8;
            let text = json!({"version":1,"origin":{"x":10,"y":10},"direction":direction}).to_string();
            let result: Value = serde_json::from_str(&fishing_click_targets(&text)).unwrap();
            assert_eq!(result["version"], 1); assert_eq!(result["ok"], true);
            assert_eq!(result["waterTarget"], json!({"x":water.0,"y":water.1}));
            let candidates = fishing_walk_candidates((10, 10), direction).unwrap();
            assert_eq!(result["walkCandidates"].as_array().unwrap().len(), 3);
            for (index, candidate) in candidates.into_iter().enumerate() {
                assert_eq!(result["walkCandidates"][index], json!({"direction":candidate.direction,
                    "cell":{"x":candidate.cell.0,"y":candidate.cell.1}}));
            }
        }
        let result: Value = serde_json::from_str(&fishing_click_targets(
            r#"{"version":1,"origin":{"x":10,"y":10},"direction":2}"#)).unwrap();
        assert_eq!(result["walkCandidates"], json!([
            {"direction":2,"cell":{"x":11,"y":10}},
            {"direction":3,"cell":{"x":11,"y":11}},
            {"direction":1,"cell":{"x":11,"y":9}}]));
    }

    #[test]
    fn targets_reject_missing_duplicate_unknown_and_invalid_geometry() {
        for text in ["", "null", "[]", "{}",
            r#"{"version":2,"origin":{"x":10,"y":10},"direction":2}"#,
            r#"{"version":1,"origin":{"x":10},"direction":2}"#,
            r#"{"version":1,"origin":{"x":10,"y":10},"direction":8}"#,
            r#"{"version":1,"origin":{"x":10,"y":10},"direction":256}"#,
            r#"{"version":1,"origin":{"x":10,"y":10},"direction":-1}"#,
            r#"{"version":1,"origin":{"x":10,"y":10},"direction":2.0}"#,
            r#"{"version":1,"version":1,"origin":{"x":10,"y":10},"direction":2}"#,
            r#"{"version":1,"origin":{"x":10,"x":10,"y":10},"direction":2}"#,
            r#"{"version":1,"origin":{"x":10,"y":10,"extra":0},"direction":2}"#,
            r#"{"version":1,"origin":{"x":10,"y":10},"direction":2,"extra":0}"#,
            r#"{"version":1,"origin":{"x":2147483648,"y":10},"direction":2}"#,
            r#"{"version":1,"origin":{"x":2147483647,"y":10},"direction":2}"#,
            r#"{"version":1,"origin":{"x":2147483647,"y":3},"direction":0}"#,
            r#"{"version":1,"origin":{"x":0,"y":-2147483648},"direction":0}"#] {
            assert_eq!(fishing_click_targets(text), REJECTED, "{}", text);
        }
    }

    #[test]
    fn decision_outputs_real_core_cast_turn_and_blocked_walk_none() {
        let mut input = request(); let original = input.clone();
        assert_eq!(response(&input), cast(1000)); assert_eq!(input, original);
        let core = FishingClickFacts { origin:(10,10),direction:2,requested_walk:true,auto_route:false,
            walk_blocked:[Some(true);3],rod_present:Some(true),water:Some(FishingWaterCell {cell:(13,10),light:100}),
            facing_matches:Some(true),standing:Some(true),fishing:Some(false),transform_type:Some(0) };
        assert_eq!(decide_fishing_click(&core, 1000, 0), FishingClickDecision::Cast);
        input["facingMatches"] = json!(false);
        assert_eq!(response(&input), json!({"version":1,"ok":true,"decision":{"type":"turn","direction":2,"delayMs":200}}));
        input["walkBlocked"][1] = json!(false); assert_eq!(response(&input), none());
        assert_eq!(response(&original), cast(1000), "ABI stores no clock or permanent transport barrier");
    }

    #[test]
    fn missing_and_null_optional_facts_stay_unknown_instead_of_fabricated_pose() {
        for key in ["rodPresent", "water", "facingMatches", "standing", "fishing", "transformType"] {
            let mut missing = request(); missing.as_object_mut().unwrap().remove(key);
            assert_eq!(response(&missing), none(), "missing {}", key);
            let mut null = request(); null[key] = Value::Null;
            assert_eq!(response(&null), none(), "null {}", key);
        }
        for index in 0..3 {
            let mut input = request(); input["walkBlocked"][index] = Value::Null;
            assert_eq!(response(&input), none());
        }
    }

    #[test]
    fn required_envelope_and_motion_fields_are_never_defaulted() {
        for key in ["version", "origin", "direction", "requestedWalk", "autoRoute", "walkBlocked", "nowMs", "lastCastMs"] {
            let mut input = request(); input.as_object_mut().unwrap().remove(key);
            assert_eq!(response(&input), rejected(), "missing {}", key);
        }
        let mut input = request(); input["version"] = json!(2); assert_eq!(response(&input), rejected());
        for blocked in [json!([]), json!([true,true]), json!([true,true,true,true]), Value::Null] {
            input = request(); input["walkBlocked"] = blocked; assert_eq!(response(&input), rejected());
        }
    }

    #[test]
    fn unknown_keys_and_incomplete_nested_carriers_are_rejected() {
        let mut input = request(); input["extra"] = json!(0); assert_eq!(response(&input), rejected());
        input = request(); input["origin"]["extra"] = json!(0); assert_eq!(response(&input), rejected());
        input = request(); input["water"]["extra"] = json!(0); assert_eq!(response(&input), rejected());
        input = request(); input["water"]["cell"]["extra"] = json!(0); assert_eq!(response(&input), rejected());
        for key in ["cell", "light"] {
            input = request(); input["water"].as_object_mut().unwrap().remove(key); assert_eq!(response(&input), rejected());
        }
        for key in ["x", "y"] {
            input = request(); input["water"]["cell"].as_object_mut().unwrap().remove(key); assert_eq!(response(&input), rejected());
        }
    }

    #[test]
    fn duplicate_known_root_optional_water_and_cell_fields_cannot_replace_evidence() {
        let text = request().to_string();
        for duplicate in [text.replacen(r#""version":1"#, r#""version":1,"version":1"#, 1),
            text.replacen(r#""standing":true"#, r#""standing":null,"standing":true"#, 1),
            text.replacen(r#""rodPresent":true"#, r#""rodPresent":false,"rodPresent":true"#, 1),
            text.replacen(r#""light":100"#, r#""light":99,"light":100"#, 1),
            text.replacen(r#""x":13"#, r#""x":12,"x":13"#, 1),
            text.replacen(r#""x":10"#, r#""x":10,"x":10"#, 1)] {
            assert_ne!(duplicate, text); assert_eq!(fishing_click_decision(&duplicate), REJECTED);
        }
    }

    #[test]
    fn integer_widths_safe_clock_limits_and_geometry_overflow_are_strict() {
        for (key, value) in [("direction", json!(8)), ("direction", json!(256)), ("direction", json!(-1)),
            ("direction", json!(2.0)), ("transformType", json!(32768)), ("transformType", json!(-32769)),
            ("nowMs", json!(MAX_SAFE_JS_INTEGER + 1)), ("lastCastMs", json!(MAX_SAFE_JS_INTEGER + 1)),
            ("nowMs", json!(-1)), ("nowMs", json!(1000.0)), ("nowMs", json!("1000")), ("nowMs", Value::Null)] {
            let mut input = request(); input[key] = value; assert_eq!(response(&input), rejected(), "{}", key);
        }
        let mut input = request(); input["water"]["light"] = json!(256); assert_eq!(response(&input), rejected());
        input = request(); input["origin"]["x"] = json!(2147483648i64); assert_eq!(response(&input), rejected());
        input = request(); input["origin"]["x"] = json!(i32::MAX); assert_eq!(response(&input), rejected());
        input = request(); input["nowMs"] = json!(MAX_SAFE_JS_INTEGER); input["lastCastMs"] = json!(MAX_SAFE_JS_INTEGER - 1000);
        assert_eq!(response(&input), cast(MAX_SAFE_JS_INTEGER));
    }

    #[test]
    fn one_second_clock_boundaries_remain_exact_and_have_no_persistent_side_effect() {
        for (now, last, expected) in [(999,0,none()),(1000,0,cast(1000)),(1999,1000,none()),(2000,1000,cast(2000)),(500,1000,none())] {
            let mut input = request(); input["nowMs"] = json!(now); input["lastCastMs"] = json!(last);
            assert_eq!(response(&input), expected); assert_eq!(response(&input), expected);
            assert_eq!(input["lastCastMs"], last);
        }
    }

    #[test]
    fn turn_precedes_unknown_pose_active_fishing_transform_and_cooldown_checks() {
        let mut input = request(); input["facingMatches"] = json!(false);
        input.as_object_mut().unwrap().remove("standing"); input.as_object_mut().unwrap().remove("transformType");
        input["fishing"] = json!(true); input["nowMs"] = json!(0); input["lastCastMs"] = json!(MAX_SAFE_JS_INTEGER);
        assert_eq!(response(&input), json!({"version":1,"ok":true,"decision":{"type":"turn","direction":2,"delayMs":200}}));
        input["facingMatches"] = Value::Null; assert_eq!(response(&input), none());
    }

    #[test]
    fn only_known_rod_exact_raw_water_and_original_cast_pose_can_succeed() {
        for (key, value) in [("requestedWalk", json!(false)), ("autoRoute", json!(true)), ("rodPresent", json!(false)),
            ("standing", json!(false)), ("fishing", json!(true)), ("transformType", json!(6)), ("transformType", json!(9))] {
            let mut input = request(); input[key] = value; assert_eq!(response(&input), none());
        }
        for light in [99,120] { let mut input=request(); input["water"]["light"]=json!(light); assert_eq!(response(&input),none()); }
        let mut input = request(); input["water"]["cell"]["x"] = json!(12); assert_eq!(response(&input), none());
        for transform in [i16::MIN,-1,5,10,i16::MAX] { input=request(); input["transformType"]=json!(transform); assert_eq!(response(&input),cast(1000)); }
        input=request(); input["water"]["light"]=json!(119); assert_eq!(response(&input),cast(1000));
    }

    #[test]
    fn bounded_utf8_inputs_reject_oversize_and_malformed_without_partial_success() {
        let target = r#"{"version":1,"origin":{"x":10,"y":10},"direction":2}"#;
        let boundary_target = format!("{}{}", target, " ".repeat(MAX_INPUT_BYTES - target.len()));
        let result: Value = serde_json::from_str(&fishing_click_targets(&boundary_target)).unwrap(); assert_eq!(result["ok"], true);
        assert_eq!(fishing_click_targets(&(boundary_target + " ")), REJECTED);
        let text = request().to_string(); let boundary = format!("{}{}", text, " ".repeat(MAX_INPUT_BYTES - text.len()));
        assert_eq!(serde_json::from_str::<Value>(&fishing_click_decision(&boundary)).unwrap(), cast(1000));
        assert_eq!(fishing_click_decision(&(boundary + " ")), REJECTED);
        for malformed in ["{", "null", "[]", "{}", "", "{\"version\":1,\"水\":\"" ] {
            assert_eq!(fishing_click_decision(malformed), REJECTED);
        }
        let multibyte = "水".repeat(1366); assert!(multibyte.len() > MAX_INPUT_BYTES);
        assert_eq!(fishing_click_targets(&multibyte), REJECTED); assert_eq!(fishing_click_decision(&multibyte), REJECTED);
    }

    /// Strict ABI1 targets; all coordinate and direction planning delegates to Core.
    /// Typed required fields reject duplicates, unknown keys and unrepresentable
    /// integers before any geometry can be returned.
    fn typed_targets_response(input: &str) -> String {
        let resolve = || {
            if input.is_empty() || input.len() > MAX_INPUT_BYTES { return None; }
            let input: TargetsInput = serde_json::from_str(input).ok()?;
            if input.version != 1 { return None; }
            let origin = input.origin.pair();
            let candidates = fishing_walk_candidates(origin, input.direction)?;
            let water = fishing_water_target(origin, input.direction)?;
            Some(json!({"version":1,"ok":true,
                "walkCandidates":candidates.map(|candidate| json!({"direction":candidate.direction,
                    "cell":{"x":candidate.cell.0,"y":candidate.cell.1}})),
                "waterTarget":{"x":water.0,"y":water.1}}).to_string())
        };
        resolve().unwrap_or_else(|| REJECTED.to_owned())
    }

    /// ABI1 explicit facts. Missing/null Option fields stay unknown. Both clocks are
    /// safe JavaScript integers; malformed geometry is rejected rather than emitted
    /// as a valid no-action decision. A valid unknown fact produces type none.
    /// Cast returns lastCastMs=nowMs but does not persist or claim that clock. The
    /// host must still recheck current custody before dispatch and then store it.
    fn typed_decision_response(input: &str) -> String {
        let resolve = || {
            if input.is_empty() || input.len() > MAX_INPUT_BYTES { return None; }
            let input: DecisionInput = serde_json::from_str(input).ok()?;
            if input.version != 1 || input.now_ms > MAX_SAFE_JS_INTEGER
                || input.last_cast_ms > MAX_SAFE_JS_INTEGER { return None; }
            let origin = input.origin.pair();
            fishing_walk_candidates(origin, input.direction)?;
            fishing_water_target(origin, input.direction)?;
            let facts = FishingClickFacts {
                origin, direction: input.direction, requested_walk: input.requested_walk,
                auto_route: input.auto_route, walk_blocked: input.walk_blocked,
                rod_present: input.rod_present,
                water: input.water.map(|water| FishingWaterCell { cell: water.cell.pair(), light: water.light }),
                facing_matches: input.facing_matches, standing: input.standing,
                fishing: input.fishing, transform_type: input.transform_type,
            };
            let decision = match decide_fishing_click(&facts, input.now_ms, input.last_cast_ms) {
                FishingClickDecision::None => json!({"type":"none"}),
                FishingClickDecision::Turn { direction, delay_ms } => json!({"type":"turn",
                    "direction":direction,"delayMs":delay_ms}),
                FishingClickDecision::Cast => json!({"type":"cast","lastCastMs":input.now_ms}),
            };
            Some(json!({"version":1,"ok":true,"decision":decision}).to_string())
        };
        resolve().unwrap_or_else(|| REJECTED.to_owned())
    }

    // The old derives remain test-only and independently decode the raw text.
    fn typed_input<T: for<'de> Deserialize<'de>>(input: &str) -> Option<T> {
        if input.is_empty() || input.len() > MAX_INPUT_BYTES { return None; }
        serde_json::from_str(input).ok()
    }
    fn assert_targets_oracle(input: &str) {
        assert_eq!(targets_input(input), typed_input::<TargetsInput>(input), "target facts: {}", input);
        assert_eq!(fishing_click_targets(input), typed_targets_response(input), "target output: {}", input);
    }
    fn assert_decision_oracle(input: &str) {
        assert_eq!(decision_input(input), typed_input::<DecisionInput>(input), "decision facts: {}", input);
        assert_eq!(fishing_click_decision(input), typed_decision_response(input), "decision output: {}", input);
    }

    #[test]
    fn manual_targets_decoder_matches_old_typed_oracle_for_objects_arrays_and_strict_numbers() {
        let base = json!({"version":1,"origin":{"x":10,"y":10},"direction":2});
        for origin in [json!({"x":10,"y":10}), json!([10,10])] {
            let object = json!({"version":1,"origin":origin,"direction":2});
            let sequence = json!([1,origin,2]);
            for value in [object, sequence] {
                let text = value.to_string();
                assert!(typed_input::<TargetsInput>(&text).is_some());
                assert_ne!(typed_targets_response(&text), REJECTED);
                assert_targets_oracle(&text);
            }
        }
        for direction in 0..8 {
            for coordinate in [i32::MIN, i32::MIN + 3, -1, 0, i32::MAX - 3, i32::MAX] {
                let value = json!({"version":1,"origin":{"x":coordinate,"y":coordinate},"direction":direction});
                assert_targets_oracle(&value.to_string());
            }
        }
        for key in ["version", "origin", "direction"] {
            let mut value = base.clone(); value.as_object_mut().unwrap().remove(key);
            assert_targets_oracle(&value.to_string());
        }
        for (key, values) in [
            ("version", vec![json!(0),json!(u32::MAX),json!(u64::from(u32::MAX)+1),json!(-1),json!(1.0),json!("1"),json!(true),Value::Null]),
            ("direction", vec![json!(0),json!(7),json!(8),json!(u8::MAX),json!(256),json!(-1),json!(2.0),json!("2"),json!(true),Value::Null]),
            ("origin", vec![json!([]),json!([10]),json!([10,10,10]),json!({"x":10}),json!({"x":10,"y":10,"extra":0}),json!("10,10"),Value::Null]),
        ] {
            for input in values { let mut value=base.clone(); value[key]=input; assert_targets_oracle(&value.to_string()); }
        }
        for key in ["x", "y"] {
            for input in [json!(i32::MIN),json!(i32::MAX),json!(i64::from(i32::MIN)-1),json!(i64::from(i32::MAX)+1),json!(10.0),json!("10"),json!(true),Value::Null] {
                let mut value=base.clone(); value["origin"][key]=input; assert_targets_oracle(&value.to_string());
            }
        }
        for text in ["", "null", "{}", "[]", "[1]", "[1,[10,10]]", "[1,[10,10],2,0]", "{",
            r#"{"version":1,"origin":{"x":10,"y":10},"direction":2}{}"#,
            r#"{"vers\u0069on":1,"origin":{"x":10,"y":10},"direction":2}"#,
            r#"{"version":1,"vers\u0069on":1,"origin":{"x":10,"y":10},"direction":2}"#,
            r#"{"version":1,"origin":{"x":10,"\u0078":10,"y":10},"direction":2}"#,
            r#"{"version":1,"origin":{"x":10,"y":10},"direction":2,"extra":null}"#,
            r#"{"version":1.0,"origin":{"x":10,"y":10},"direction":2}"#,
            r#"{"version":1,"origin":{"x":-0,"y":10},"direction":2}"#,
            r#"{"version":1,"origin":{"x":10,"y":10},"direction":2e0}"#] {
            assert_targets_oracle(text);
        }
        let text = base.to_string();
        let boundary = format!("{}{}", text, " ".repeat(MAX_INPUT_BYTES - text.len()));
        assert_targets_oracle(&boundary); assert_targets_oracle(&(boundary + " "));
    }

    #[test]
    fn manual_decision_decoder_matches_old_typed_oracle_for_nullable_facts_and_clock_boundaries() {
        let keys = ["version","origin","direction","requestedWalk","autoRoute","walkBlocked",
            "rodPresent","water","facingMatches","standing","fishing","transformType","nowMs","lastCastMs"];
        let base=request(); let text=base.to_string();
        assert_decision_oracle(&text);
        for origin in [json!({"x":10,"y":10}),json!([10,10])] {
            for water in [json!({"cell":{"x":13,"y":10},"light":100}),json!({"cell":[13,10],"light":100}),
                json!([{"x":13,"y":10},100]),json!([[13,10],100])] {
                let mut object=base.clone(); object["origin"]=origin.clone(); object["water"]=water;
                let sequence=Value::Array(keys.iter().map(|key| object[*key].clone()).collect());
                for value in [object,sequence] {
                    let input=value.to_string(); assert!(typed_input::<DecisionInput>(&input).is_some());
                    assert_eq!(typed_decision_response(&input), cast(1000).to_string());
                    assert_decision_oracle(&input);
                }
            }
        }
        let mut sequence=keys.iter().map(|key| base[*key].clone()).collect::<Vec<_>>();
        for length in 0..14 { assert_decision_oracle(&Value::Array(sequence[..length].to_vec()).to_string()); }
        sequence.push(Value::Null); assert_decision_oracle(&Value::Array(sequence).to_string());
        for key in keys {
            let mut value=base.clone(); value.as_object_mut().unwrap().remove(key); assert_decision_oracle(&value.to_string());
            value=base.clone(); value[key]=Value::Null; assert_decision_oracle(&value.to_string());
        }
        let mut nullable=base.clone();
        for key in ["rodPresent","water","facingMatches","standing","fishing","transformType"] { nullable[key]=Value::Null; }
        nullable["walkBlocked"]=json!([null,null,null]);
        assert_decision_oracle(&nullable.to_string());
        assert_decision_oracle(&Value::Array(keys.iter().map(|key| nullable[*key].clone()).collect()).to_string());
        for key in ["requestedWalk","autoRoute","rodPresent","facingMatches","standing","fishing"] {
            for input in [json!(true),json!(false),Value::Null,json!(0),json!(1),json!("true"),json!([]),json!({})] {
                let mut value=base.clone(); value[key]=input; assert_decision_oracle(&value.to_string());
            }
        }
        for input in [json!([]),json!([true,true]),json!([true,true,true,true]),json!([null,false,true]),json!([true,0,true]),json!([true,"true",true]),json!({}),Value::Null] {
            let mut value=base.clone(); value["walkBlocked"]=input; assert_decision_oracle(&value.to_string());
        }
        for key in ["version","direction","transformType","nowMs","lastCastMs"] {
            for input in [json!(0),json!(1),json!(i16::MIN),json!(i16::MAX),json!(-32769),json!(32768),json!(255),json!(256),json!(u32::MAX),
                json!(u64::from(u32::MAX)+1),json!(MAX_SAFE_JS_INTEGER-1),json!(MAX_SAFE_JS_INTEGER),json!(MAX_SAFE_JS_INTEGER+1),json!(u64::MAX),
                json!(-1),json!(1.0),json!("1"),json!(true),json!([]),Value::Null] {
                let mut value=base.clone(); value[key]=input; assert_decision_oracle(&value.to_string());
            }
        }
        for key in ["nowMs","lastCastMs"] {
            let mut value=base.clone(); value["nowMs"]=json!(MAX_SAFE_JS_INTEGER); value["lastCastMs"]=json!(MAX_SAFE_JS_INTEGER-1000);
            assert_decision_oracle(&value.to_string());
            let needle=if key=="nowMs" { r#""nowMs":1000"# } else { r#""lastCastMs":0"# };
            for number in ["-0","0.0","1e3","01","+1000","18446744073709551616"] {
                let replacement=format!("\"{}\":{}",key,number); let input=text.replacen(needle,&replacement,1);
                assert_ne!(input,text); assert_decision_oracle(&input);
            }
        }
        for input in [json!([]),json!([{"x":13,"y":10}]),json!([{"x":13,"y":10},100,0]),json!({"cell":{"x":13,"y":10}}),
            json!({"light":100}),json!({"cell":{"x":13},"light":100}),json!({"cell":{"x":13,"y":10,"extra":0},"light":100}),
            json!({"cell":{"x":13,"y":10},"light":100,"extra":0}),json!(true),json!("water")] {
            let mut value=base.clone(); value["water"]=input; assert_decision_oracle(&value.to_string());
        }
        for input in [json!(0),json!(99),json!(100),json!(119),json!(120),json!(255),json!(256),json!(-1),json!(100.0),json!("100"),json!(true),Value::Null] {
            let mut value=base.clone(); value["water"]["light"]=input; assert_decision_oracle(&value.to_string());
        }
        for carrier in ["origin","water"] {
            for key in ["x","y"] {
                for input in [json!(i32::MIN),json!(i32::MAX),json!(i64::from(i32::MIN)-1),json!(i64::from(i32::MAX)+1),json!(10.0),json!("10"),json!(true),Value::Null] {
                    let mut value=base.clone();
                    if carrier=="origin" { value[carrier][key]=input; } else { value[carrier]["cell"][key]=input; }
                    assert_decision_oracle(&value.to_string());
                }
            }
        }
        for input in [text.replacen(r#""version":1"#,r#""version":1,"vers\u0069on":1"#,1),
            text.replacen(r#""standing":true"#,r#""standing":null,"stand\u0069ng":true"#,1),
            text.replacen(r#""standing":true"#,r#""stand\u0069ng":true"#,1),
            text.replacen(r#""light":100"#,r#""light":100,"l\u0069ght":100"#,1),
            text.replacen(r#""x":13"#,r#""x":13,"\u0078":13"#,1),
            text.replacen(r#""x":10"#,r#""x":10,"\u0078":10"#,1),
            text.replacen(r#""version":1"#,r#""version":1,"extra":null"#,1),
            text.replacen(r#""cell":{"x":13,"y":10}"#,r#""cell":{"x":13,"y":10},"cell":null"#,1),
            text.replacen(r#""water":"#,r#""water":null,"water":"#,1)] {
            assert_ne!(input,text); assert_decision_oracle(&input);
        }
        for input in ["", "null", "{}", "[]", "{", "[", "true"] { assert_decision_oracle(input); }
        let boundary=format!("{}{}",text," ".repeat(MAX_INPUT_BYTES-text.len()));
        assert_decision_oracle(&boundary); assert_decision_oracle(&(boundary+" "));
    }
}
