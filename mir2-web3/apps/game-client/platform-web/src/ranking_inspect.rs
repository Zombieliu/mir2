//! Optional renderer-free ABI for shared ranking-row inspect admission.
//! Proposals do not own a row, expected name, gesture, transport or clock.

use crate::item_tooltip::StrictJsonValue;
use mir2_client_core::ranking_inspect::{plan_ranking_inspect, RankingInspectFacts};
use serde_json::Value;
use wasm_bindgen::prelude::*;

const MAX_INPUT_BYTES: usize = 4096;
const MAX_SAFE_JS_INTEGER: u64 = 9_007_199_254_740_991;
const REJECTED: &str = "{\"version\":1,\"ok\":false}";
const FIELDS: [&str; 7] = ["version", "opened", "rankingsReady", "pending",
    "playerId", "nowMs", "nextReadyMs"];

#[wasm_bindgen]
pub fn ranking_inspect_abi_version() -> u32 { 1 }

fn field<'a>(value: &'a Value, index: usize) -> Option<&'a Value> {
    match value {
        Value::Object(fields) => fields.get(FIELDS[index]),
        Value::Array(fields) => fields.get(index),
        _ => None,
    }
}

/// ABI1 has exactly seven fields, as an object or a positional array in FIELDS
/// order. Every flag, identity and clock is required; null never supplies a fact.
/// The host retains literal ranking=true/hero=false and owns final dispatch.
#[wasm_bindgen]
pub fn ranking_inspect_admission(input: &str) -> String {
    let resolve = || {
        if input.is_empty() || input.len() > MAX_INPUT_BYTES { return None; }
        let StrictJsonValue(value) = serde_json::from_str::<StrictJsonValue<256>>(input).ok()?;
        match &value {
            Value::Object(fields) if fields.len() == FIELDS.len()
                && fields.keys().all(|key| FIELDS.contains(&key.as_str())) => {},
            Value::Array(fields) if fields.len() == FIELDS.len() => {},
            _ => return None,
        }
        if field(&value, 0)?.as_u64()? != 1 { return None; }
        let player_id = u32::try_from(field(&value, 4)?.as_u64()?).ok()?;
        let now_ms = field(&value, 5)?.as_u64()?;
        let next_ready_ms = field(&value, 6)?.as_u64()?;
        if now_ms > MAX_SAFE_JS_INTEGER || next_ready_ms > MAX_SAFE_JS_INTEGER { return None; }
        let plan = plan_ranking_inspect(&RankingInspectFacts {
            opened: Some(field(&value, 1)?.as_bool()?),
            rankings_ready: Some(field(&value, 2)?.as_bool()?),
            pending: Some(field(&value, 3)?.as_bool()?),
            player_id: Some(i64::from(player_id)), now_ms, next_ready_ms,
        })?;
        if plan.next_ready_ms > MAX_SAFE_JS_INTEGER { return None; }
        Some(format!(r#"{{"version":1,"ok":true,"objectId":{},"nextReadyMs":{}}}"#,
            plan.object_id, plan.next_ready_ms))
    };
    resolve().unwrap_or_else(|| REJECTED.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request() -> Value {
        json!({"version":1,"opened":true,"rankingsReady":true,"pending":false,
            "playerId":0,"nowMs":1001,"nextReadyMs":1000})
    }

    #[test]
    fn ranking_inspect_abi_object_and_seven_field_array_preserve_raw_zero_id_and_clock() {
        assert_eq!(ranking_inspect_abi_version(), 1);
        let expected = r#"{"version":1,"ok":true,"objectId":0,"nextReadyMs":1501}"#;
        let input = request(); let original = input.clone();
        assert_eq!(ranking_inspect_admission(&input.to_string()), expected);
        assert_eq!(ranking_inspect_admission("[1,true,true,false,0,1001,1000]"), expected);
        assert_eq!(input, original);
        let mut maximum = request(); maximum["playerId"] = json!(u32::MAX);
        assert_eq!(ranking_inspect_admission(&maximum.to_string()),
            r#"{"version":1,"ok":true,"objectId":4294967295,"nextReadyMs":1501}"#);
    }

    #[test]
    fn ranking_inspect_abi_requires_known_flags_and_exact_strict_cooldown_boundary() {
        for key in ["opened", "rankingsReady", "pending"] {
            for value in [Value::Null, json!(0), json!("true"), json!([]), json!({})] {
                let mut input = request(); input[key] = value;
                assert_eq!(ranking_inspect_admission(&input.to_string()), REJECTED, "{key}");
            }
            let mut input = request(); input[key] = json!(key == "pending");
            assert_eq!(ranking_inspect_admission(&input.to_string()), REJECTED, "{key}");
        }
        for now in [0, 999, 1000] {
            let mut input = request(); input["nowMs"] = json!(now);
            assert_eq!(ranking_inspect_admission(&input.to_string()), REJECTED);
        }
        assert_eq!(ranking_inspect_admission(&request().to_string()),
            r#"{"version":1,"ok":true,"objectId":0,"nextReadyMs":1501}"#);
    }

    #[test]
    fn ranking_inspect_abi_rejects_missing_extra_duplicate_and_malformed_facts() {
        for key in FIELDS {
            let mut input = request(); input.as_object_mut().unwrap().remove(key);
            assert_eq!(ranking_inspect_admission(&input.to_string()), REJECTED, "missing {key}");
            let raw = request().to_string();
            let original = format!("\"{key}\":{}", request()[key]);
            let duplicate = raw.replacen(&original, &format!("{original},{original}"), 1);
            assert_ne!(duplicate, raw);
            assert_eq!(ranking_inspect_admission(&duplicate), REJECTED, "duplicate {key}");
        }
        let mut extra = request(); extra["hero"] = json!(false);
        assert_eq!(ranking_inspect_admission(&extra.to_string()), REJECTED);
        for input in ["", "null", "{}", "[]", "{", "[1,true,true,false,0,1001]",
            "[1,true,true,false,0,1001,1000,null]", "[1,true,true,false,null,1001,1000]",
            r#"{"version":1,"vers\u0069on":1,"opened":true,"rankingsReady":true,"pending":false,"playerId":0,"nowMs":1001,"nextReadyMs":1000}"#,
            r#"{"version":1,"opened":true,"rankingsReady":true,"pending":false,"playerId":0,"nowMs":1001,"nextReadyMs":1000}{}"#] {
            assert_eq!(ranking_inspect_admission(input), REJECTED, "{input}");
        }
    }

    #[test]
    fn ranking_inspect_abi_integer_types_and_safe_js_output_boundary_are_strict() {
        for key in ["version", "playerId", "nowMs", "nextReadyMs"] {
            for value in [Value::Null, json!(-1), json!(1.0), json!("1"), json!(true), json!([]), json!({})] {
                let mut input = request(); input[key] = value;
                assert_eq!(ranking_inspect_admission(&input.to_string()), REJECTED, "{key}");
            }
        }
        for id in [u64::from(u32::MAX) + 1, MAX_SAFE_JS_INTEGER, u64::MAX] {
            let mut input = request(); input["playerId"] = json!(id);
            assert_eq!(ranking_inspect_admission(&input.to_string()), REJECTED);
        }
        let mut input = request(); input["version"] = json!(2);
        assert_eq!(ranking_inspect_admission(&input.to_string()), REJECTED);
        for key in ["nowMs", "nextReadyMs"] {
            input = request(); input[key] = json!(MAX_SAFE_JS_INTEGER + 1);
            assert_eq!(ranking_inspect_admission(&input.to_string()), REJECTED);
        }
        input = request(); input["nowMs"] = json!(MAX_SAFE_JS_INTEGER - 500);
        input["nextReadyMs"] = json!(MAX_SAFE_JS_INTEGER - 501);
        assert_eq!(ranking_inspect_admission(&input.to_string()),
            r#"{"version":1,"ok":true,"objectId":0,"nextReadyMs":9007199254740991}"#);
        for now in [MAX_SAFE_JS_INTEGER - 499, MAX_SAFE_JS_INTEGER, u64::MAX] {
            input["nowMs"] = json!(now);
            assert_eq!(ranking_inspect_admission(&input.to_string()), REJECTED);
        }
    }

    #[test]
    fn ranking_inspect_abi_bounded_utf8_and_raw_number_forms_do_not_fabricate_admission() {
        let text = request().to_string();
        let boundary = format!("{}{}", text, " ".repeat(MAX_INPUT_BYTES - text.len()));
        assert_eq!(ranking_inspect_admission(&boundary),
            r#"{"version":1,"ok":true,"objectId":0,"nextReadyMs":1501}"#);
        assert_eq!(ranking_inspect_admission(&(boundary + " ")), REJECTED);
        assert_eq!(ranking_inspect_admission(&"水".repeat(1366)), REJECTED);
        for number in ["-0", "0.0", "0e0", "01", "+0", "18446744073709551616"] {
            let input = text.replacen("\"playerId\":0", &format!("\"playerId\":{number}"), 1);
            assert_ne!(input, text);
            assert_eq!(ranking_inspect_admission(&input), REJECTED, "{number}");
        }
    }
}
