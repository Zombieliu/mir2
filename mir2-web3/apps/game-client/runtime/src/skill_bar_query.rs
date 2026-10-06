//! Read-only Native skill metadata for the browser's two skill bars.
//! Rows are presentation only. The host must retain the original raw owner,
//! source and slot lease, and obtain a fresh CombatProof for every cast.
use std::collections::HashSet;
use mir2_client_bevy::{skill_model::MAX_LEARNED_SKILLS, skill_page_state::normalize_raw_skills};
use serde_json::{json, Map, Value};
use crate::item_tooltip_query::strict_json_value;

const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const RAW_FIELDS: &[&str] = &["id", "key", "name", "description", "spell", "level", "experience",
    "hotkey", "icon", "mpCost", "delayMs", "castTimeMs", "cooldownRemainingTicks", "castSequence",
    "castKind", "offensive", "canUse", "need1", "need2", "need3"];

pub fn query_skill_bar_json(input: &str) -> String {
    match query(input) {
        Ok(rows) => json!({"version":1,"ok":true,"rows":rows}).to_string(),
        Err(error) => json!({"version":1,"ok":false,"error":error}).to_string(),
    }
}
fn query(input: &str) -> Result<Vec<Value>, &'static str> {
    let value = strict_json_value::<MAX_LEARNED_SKILLS>(input)?;
    let root = value.as_object().ok_or("invalidRoot")?;
    if root.len() != 2 || root.keys().any(|key| !["version", "learned"].contains(&key.as_str())) {
        return Err("unexpectedField");
    }
    if root.get("version").and_then(Value::as_u64) != Some(1) { return Err("unsupportedVersion"); }
    let learned = root.get("learned").and_then(Value::as_array).ok_or("invalidLearned")?;
    if learned.len() > MAX_LEARNED_SKILLS { return Err("tooManyLearned"); }
    let mut keys = HashSet::new();
    let mut hotkeys = HashSet::new();
    let mut ids = HashSet::new();
    for (index, value) in learned.iter().enumerate() {
        let raw = value.as_object().ok_or("invalidLearnedRow")?;
        if raw.keys().any(|key| !RAW_FIELDS.contains(&key.as_str())) { return Err("unexpectedLearnedField"); }
        let key = string(raw.get("key").ok_or("missingLearnedKey")?, 128, false)?;
        string(raw.get("name").ok_or("missingLearnedName")?, 256, false)?;
        if !keys.insert(key) { return Err("duplicateLearnedKey"); }
        // Internal normalization uses an absent id's raw index. This check only
        // prevents metadata aliasing; neither id nor index leaves the document.
        let id = match raw.get("id") { Some(value) => unsigned(value, u32::MAX as u64)?, None => index as u64 };
        if !ids.insert(id) { return Err("duplicateLearnedIdentity"); }
        if let Some(value) = raw.get("hotkey").filter(|v| !v.is_null()) {
            let hotkey = unsigned(value, u8::MAX as u64)?;
            if (1..=16).contains(&hotkey) && !hotkeys.insert(hotkey) { return Err("duplicateLearnedHotkey"); }
        }
        for (field, max, nullable) in [("level", 255, false), ("icon", 255, true),
            ("experience", 65535, true), ("need1", 65535, true), ("need2", 65535, true),
            ("need3", 65535, true), ("mpCost", u32::MAX as u64, true),
            ("delayMs", u32::MAX as u64, true), ("cooldownRemainingTicks", u32::MAX as u64, false),
            ("castSequence", MAX_SAFE_INTEGER, false)] {
            if let Some(value) = raw.get(field) {
                if !(nullable && value.is_null()) { unsigned(value, max)?; }
            }
        }
        if let Some(value) = raw.get("castTimeMs").filter(|v| !v.is_null()) {
            if value.as_i64().is_none_or(|v| v.unsigned_abs() > MAX_SAFE_INTEGER) { return Err("unsafeCastTime"); }
        }
        if let Some(value) = raw.get("spell").filter(|v| !v.is_null()) { string(value, 128, false)?; }
        if let Some(value) = raw.get("description") { string(value, 16_384, true)?; }
        if let Some(value) = raw.get("castKind").filter(|v| !v.is_null()) {
            let kind = string(value, 16, false)?;
            if !["passive", "toggle", "self", "target", "ground", "direction"].contains(&kind) { return Err("invalidCastKind"); }
        }
        for field in ["offensive", "canUse"] {
            if raw.get(field).filter(|v| !v.is_null()).is_some_and(|v| !v.is_boolean()) { return Err("invalidLearnedBoolean"); }
        }
    }
    // Never normalize a skipped, compacted or truncated raw list as complete.
    let model = normalize_raw_skills(learned).map_err(|_| "invalidLearnedMetadata")?;
    if model.skills.len() != learned.len() || model.bindings.len() != learned.len() { return Err("incompleteLearned"); }
    let mut rows = Vec::with_capacity(learned.len());
    for ((raw, skill), binding) in learned.iter().zip(&model.skills).zip(&model.bindings) {
        let key = raw["key"].as_str().ok_or("missingLearnedKey")?;
        if skill.key.as_deref() != Some(key) || binding.skill_id != skill.id { return Err("mismatchedLearnedSource"); }
        let mut output = Map::new();
        output.insert("key".into(), json!(key));
        output.insert("name".into(), json!(skill.name));
        if let Some(value) = binding.hotkey { output.insert("hotkey".into(), json!(value)); }
        if let Some(value) = binding.icon { output.insert("icon".into(), json!(value)); }
        if let Some(value) = binding.mp_cost { output.insert("mpCost".into(), json!(value)); }
        if let Some(value) = binding.delay_ms { output.insert("delayMs".into(), json!(value)); }
        if raw.get("cooldownRemainingTicks").is_some() {
            output.insert("cooldownRemainingTicks".into(), json!(binding.cooldown_remaining_ticks));
        }
        rows.push(Value::Object(output));
    }
    Ok(rows)
}
fn unsigned(value: &Value, max: u64) -> Result<u64, &'static str> {
    value.as_u64().filter(|v| *v <= max).ok_or("invalidLearnedInteger")
}
fn string(value: &Value, max: usize, empty: bool) -> Result<&str, &'static str> {
    value.as_str().filter(|s| s.len() <= max && (empty || !s.is_empty()) && !s.contains('\0'))
        .ok_or("invalidLearnedString")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn learned() -> Value {
        json!({"key":"fire-ball","name":"Fire Ball","description":"Actual source", "spell":"FireBall",
            "level":1,"experience":0,"hotkey":9,"mpCost":7,"delayMs":2200,
            "castTimeMs":-100,"cooldownRemainingTicks":3,"castKind":"target","offensive":true})
    }
    fn response(rows: Vec<Value>) -> Value {
        serde_json::from_str(&query_skill_bar_json(&json!({"version":1,"learned":rows}).to_string())).unwrap()
    }
    #[test]
    fn skill_bar_query_uses_native_normalization_catalog_and_true_raw_order() {
        let first = learned();
        let second = json!({"key":"unknown-key","name":"Uncatalogued","hotkey":0,"spell":"UncataloguedSpell"});
        let raw = vec![first.clone(), second];
        let normalized = normalize_raw_skills(&raw).unwrap();
        let result = response(raw); assert_eq!(result["ok"], true);
        assert_eq!(result["rows"][0]["key"], "fire-ball");
        assert_eq!(result["rows"][1]["key"], "unknown-key");
        let template = mir2_game_data::crystal_magic_by_spell("FireBall").unwrap();
        assert_eq!(result["rows"][0]["icon"], template.icon);
        assert_eq!(result["rows"][0]["icon"], normalized.bindings[0].icon.unwrap());
        assert_eq!(result["rows"][0]["mpCost"], 7);
        assert_eq!(result["rows"][0]["delayMs"], 2200);
        assert_eq!(result["rows"][0]["cooldownRemainingTicks"], 3);
        assert_eq!(result["rows"][0]["hotkey"], 9);
        assert!(result["rows"][0].get("id").is_none());
        assert!(result["rows"][0].get("rawIndex").is_none());
        assert!(result["rows"][1].get("icon").is_none());
        assert!(result["rows"][1].get("mpCost").is_none());
        assert!(result["rows"][1].get("delayMs").is_none());
        assert!(result["rows"][1].get("cooldownRemainingTicks").is_none());
        for hotkey in [0, 17, 255] {
            let mut valid = learned(); valid["hotkey"] = json!(hotkey);
            let result = response(vec![valid]); assert_eq!(result["ok"], true);
            assert_eq!(result["rows"][0]["hotkey"], hotkey);
        }
        let non_castable = vec![json!({"key":"a","name":"A","hotkey":17}), json!({"key":"b","name":"B","hotkey":17})];
        assert_eq!(response(non_castable)["ok"], true);
    }
    #[test]
    fn skill_bar_query_preserves_explicit_unknown_and_supplied_metadata_without_catalog_overwrite() {
        let mut raw = learned(); raw["icon"] = Value::Null; raw["mpCost"] = Value::Null; raw["delayMs"] = Value::Null;
        let result = response(vec![raw]); assert_eq!(result["ok"], true);
        for field in ["icon", "mpCost", "delayMs"] { assert!(result["rows"][0].get(field).is_none(), "{field}"); }
        let mut raw = learned(); raw["icon"] = json!(0); raw["mpCost"] = json!(0); raw["delayMs"] = json!(0);
        let result = response(vec![raw]);
        for field in ["icon", "mpCost", "delayMs"] { assert_eq!(result["rows"][0][field], 0); }
    }
    #[test]
    fn skill_bar_query_never_skips_or_truncates_invalid_learned_rows() {
        for bad in [Value::Null, json!(7), json!("row"), json!({"name":"No actual key"}), json!({"key":"No name"})] {
            let result = response(vec![learned(), bad]); assert_eq!(result["ok"], false); assert!(result.get("rows").is_none());
        }
        let rows: Vec<Value> = (0..512).map(|i| json!({"key":format!("key-{i}"),"name":"Learned","hotkey":0})).collect();
        let result = response(rows.clone()); assert_eq!(result["ok"], true); assert_eq!(result["rows"].as_array().unwrap().len(), 512);
        let mut too_many = rows; too_many.push(json!({"key":"last","name":"Extra"}));
        assert_eq!(response(too_many)["ok"], false);
        assert_eq!(response(vec![])["rows"], json!([]));
    }
    #[test]
    fn skill_bar_query_rejects_duplicate_keys_hotkeys_and_internal_metadata_aliasing() {
        let raw = learned(); assert_eq!(response(vec![raw.clone(),raw.clone()])["error"], "duplicateLearnedKey");
        let mut other = raw.clone(); other["key"] = json!("other");
        assert_eq!(response(vec![raw.clone(),other.clone()])["error"], "duplicateLearnedHotkey");
        other["hotkey"] = json!(0); let mut first = raw.clone(); first["id"] = json!(1);
        assert_eq!(response(vec![first,other])["error"], "duplicateLearnedIdentity");
    }
    #[test]
    fn skill_bar_query_rejects_malformed_optional_fields_unsafe_units_and_unexpected_data() {
        for (field, value) in [("hotkey",json!(256)),("hotkey",json!(-1)),("icon",json!(256)),
            ("mpCost",json!(4_294_967_296_u64)),("delayMs",json!(-1)),("delayMs",json!(1.5)),
            ("castTimeMs",json!(9_007_199_254_740_992_i64)),("castSequence",json!(9_007_199_254_740_992_u64)),
            ("cooldownRemainingTicks",Value::Null),("offensive",json!(1)),("canUse",json!("true")),
            ("level",json!(256)),("description",json!(["text"])),("madeUpIcon",json!(1)),
            ("key",json!("")),("name",json!(""))] {
            let mut raw = learned(); raw[field]=value;
            let result=response(vec![raw]); assert_eq!(result["ok"],false,"{field}"); assert!(result.get("rows").is_none());
        }
    }
    #[test]
    fn skill_bar_query_strict_json_bounds_do_not_relax_original_tooltip_arrays() {
        for raw in [r#"{"version":1,"version":1,"learned":[]}"#,
            r#"{"version":1,"learned":[{"key":"a","key":"b","name":"Name"}]}"#,
            r#"{"version":1,"learned":[],"authority":true}"#,
            r#"{"version":2,"learned":[]}"#, r#"{"version":1,"learned":{}}"#] {
            let result:Value=serde_json::from_str(&query_skill_bar_json(raw)).unwrap(); assert_eq!(result["ok"],false);
        }
        let array=json!(vec![0;257]).to_string();
        assert!(strict_json_value::<256>(&array).is_err());
        assert!(strict_json_value::<512>(&array).is_ok());
        assert!(strict_json_value::<512>(&"x".repeat(262_145)).is_err());
    }
}
