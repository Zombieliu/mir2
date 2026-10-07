//! Renderer-free ABI1 item-label projection with bounded, duplicate-rejecting JSON.
//! Root stays {version:1,item,player}; time is a separate canonical i64 decimal.
//! Optional item.legacy: key/description default empty; grade/durability nullable;
//! sellValue/attack/defence/addedAttack/addedDefence/addedLuck/socketSlots default zero.
//! Missing source fields never become complete through DTO defaults.

use std::{collections::HashSet, fmt};
use mir2_client_core::{item_tooltip::{crystal_item_tooltip_document_with_options, CrystalItemTooltipColour, CrystalItemTooltipDocument, CrystalItemTooltipSectionKind, CrystalItemTooltipOptions, CrystalStackSplitHint}, item_tooltip_types::*};
use serde::{de::{self, MapAccess, SeqAccess, Visitor}, Deserialize, Deserializer};
use serde_json::{json, Map, Value};
use wasm_bindgen::prelude::*;

const MAX_JSON_BYTES: usize = 262_144;
const MAX_DEPTH: usize = 16;
const MAX_NODES: usize = 8_192;
const MAX_ARRAY: usize = 256;
const MAX_SOCKETS: usize = 32;
const MAX_ITEM_NODES: usize = 64;
const MAX_STRING_BYTES: usize = 16_384;
const MAX_SAFE_JS_INTEGER: u64 = 9_007_199_254_740_991;
// A global absolute sum bounds every native i32 stat subtotal and addition.
const MAX_STAT_BUDGET: i64 = i32::MAX as i64 / 4;

#[wasm_bindgen]
pub fn item_tooltip_abi_version() -> u32 { 1 }

/// Display metadata only. Catalogue modes remain on the existing heavy renderer.
#[wasm_bindgen]
pub fn item_tooltip_document(input: &str, now_dotnet_ticks: &str) -> String {
    let query = || {
        let now = now_dotnet_ticks.parse::<i64>().map_err(|_| "invalidClock")?;
        if now.to_string() != now_dotnet_ticks { return Err("invalidClock"); }
        query(input, now)
    };
    match query() {
        Ok(value) => value.to_string(),
        Err(error) => json!({"version":1,"ok":false,"error":error}).to_string(),
    }
}

#[cfg_attr(test, derive(Debug, Deserialize, PartialEq))]
#[cfg_attr(test, serde(rename_all="camelCase", deny_unknown_fields))]
struct QueryItem {
    unique_id: u64, item_index: i32, name: String, icon: u16, count: u16,
    #[cfg_attr(test, serde(default))]
    source_kind: Option<String>,
    #[cfg_attr(test, serde(default))]
    tooltip_source: Option<Value>,
    #[cfg_attr(test, serde(default))]
    legacy: Option<Value>,
}

// Avoid serde's owned Value deserializer in the production ABI. The old typed
// decoder remains a test oracle for both object and positional-array inputs.
fn take_query_item_field(raw: &mut Value, key: &str, index: usize) -> Option<Value> {
    match raw {
        Value::Object(fields) => fields.remove(key),
        Value::Array(fields) => fields.get_mut(index).map(std::mem::take),
        _ => None,
    }
}

fn parse_query_item(mut raw: Value) -> Result<QueryItem, &'static str> {
    match &raw {
        Value::Object(fields) => exact_keys(fields, &["uniqueId", "itemIndex", "name", "icon", "count", "sourceKind", "tooltipSource", "legacy"])
            .map_err(|_| "invalidItem")?,
        Value::Array(fields) if (5..=8).contains(&fields.len()) => {},
        _ => return Err("invalidItem"),
    }
    let unique_id = take_query_item_field(&mut raw, "uniqueId", 0)
        .and_then(|value| value.as_u64()).ok_or("invalidItem")?;
    let item_index = take_query_item_field(&mut raw, "itemIndex", 1)
        .and_then(|value| value.as_i64()).and_then(|value| i32::try_from(value).ok()).ok_or("invalidItem")?;
    let name = match take_query_item_field(&mut raw, "name", 2) {
        Some(Value::String(value)) => value,
        _ => return Err("invalidItem"),
    };
    let icon = take_query_item_field(&mut raw, "icon", 3)
        .and_then(|value| value.as_u64()).and_then(|value| u16::try_from(value).ok()).ok_or("invalidItem")?;
    let count = take_query_item_field(&mut raw, "count", 4)
        .and_then(|value| value.as_u64()).and_then(|value| u16::try_from(value).ok()).ok_or("invalidItem")?;
    let source_kind = match take_query_item_field(&mut raw, "sourceKind", 5) {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) => Some(value),
        _ => return Err("invalidItem"),
    };
    let tooltip_source = take_query_item_field(&mut raw, "tooltipSource", 6).filter(|value| !value.is_null());
    let legacy = take_query_item_field(&mut raw, "legacy", 7).filter(|value| !value.is_null());
    Ok(QueryItem { unique_id, item_index, name, icon, count, source_kind, tooltip_source, legacy })
}

fn query(input: &str, now: i64) -> Result<Value, &'static str> {
    if input.is_empty() || input.len() > MAX_JSON_BYTES { return Err("invalidJsonSize"); }
    let StrictValue(mut value) = serde_json::from_str(input).map_err(|_| "invalidJson")?;
    bounded_value(&value, 0, &mut 0)?;
    let root = object(&mut value)?;
    exact_keys(root, &["version", "item", "player"])?;
    if root.get("version").and_then(Value::as_u64) != Some(1) { return Err("unsupportedVersion"); }
    let raw_item = root.remove("item").ok_or("missingItem")?;
    if raw_item.as_object().and_then(|item| item.get("sourceKind")).is_some_and(Value::is_null) { return Err("invalidSourceKind"); }
    let item = parse_query_item(raw_item)?;
    if item.unique_id > MAX_SAFE_JS_INTEGER || item.count == 0 || item.name.is_empty() || item.name.len() > 512 { return Err("invalidItemIdentity"); }
    if item.source_kind.as_deref().is_some_and(|kind| kind != "instance") { return Err("unsupportedSourceKind"); }
    let mut raw_player = root.remove("player").ok_or("missingPlayer")?;
    let mut stat_budget = 0;
    let (player, viewer_complete) = parse_player(&mut raw_player, &mut stat_budget)?;
    let mut display = parse_legacy(item.legacy.as_ref(), &mut stat_budget)?;
    display.unique_id = Some(item.unique_id);
    display.name = item.name.clone();
    display.icon = item.icon;
    display.quantity = u32::from(item.count);
    let mut reasons = Vec::new();
    display.tooltip_source = match item.tooltip_source.as_ref() {
        Some(raw) => validate_source(&mut raw.clone(), &item, &mut reasons, &mut stat_budget)?,
        None => { reason(&mut reasons, "missingSource"); None }
    };
    // This DOM host exposes stack splitting through its More actions menu.
    let mut document = crystal_item_tooltip_document_with_options(&display, &player, now,
        CrystalItemTooltipOptions { hide_added_stats: false, stack_split_hint: CrystalStackSplitHint::MoreActions });
    if !document.source_complete && reasons.is_empty() { reason(&mut reasons, "incompleteSource"); }
    if !reasons.is_empty() { document.source_complete = false; }
    Ok(json!({"version":1,"ok":true,"document":document_json(&document),"viewerComplete":viewer_complete,"partialReasons":reasons}))
}

fn parse_legacy(raw: Option<&Value>, budget: &mut i64) -> Result<ItemTooltipItem, &'static str> {
    let mut item = ItemTooltipItem::default();
    let Some(raw) = raw else { return Ok(item); };
    let raw = raw.as_object().ok_or("invalidLegacy")?;
    exact_keys(raw, &["key", "grade", "description", "sellValue", "durabilityCurrent", "durabilityMax", "attack", "defence", "addedAttack", "addedDefence", "addedLuck", "socketSlots"])?;
    if let Some(value) = raw.get("key") { bounded_string(value,512)?; item.key = value.as_str().unwrap().to_owned(); }
    if let Some(value) = raw.get("grade").filter(|v| !v.is_null()) { bounded_string(value,512)?; item.grade = Some(value.as_str().unwrap().to_owned()); }
    if let Some(value) = raw.get("description") { bounded_string(value,8192)?; item.description = value.as_str().unwrap().to_owned(); }
    if let Some(value) = raw.get("sellValue") { item.sell_value = u32::try_from(unsigned(value,u32::MAX as u64)?).map_err(|_| "invalidLegacy")?; }
    for (key,destination) in [("durabilityCurrent", &mut item.durability_current), ("durabilityMax", &mut item.durability_max)] {
        if let Some(value) = raw.get(key).filter(|v| !v.is_null()) { *destination=Some(u16::try_from(unsigned(value,u16::MAX as u64)?).map_err(|_| "invalidLegacy")?); }
    }
    for (key,destination) in [("attack", &mut item.attack), ("defence", &mut item.defence), ("addedAttack", &mut item.added_attack), ("addedDefence", &mut item.added_defence), ("addedLuck", &mut item.added_luck)] {
        if let Some(value) = raw.get(key) {
            let exact=signed(value,i32::MIN as i64,i32::MAX as i64)?;
            *budget += exact.abs();
            if *budget > MAX_STAT_BUDGET { return Err("statBudgetExceeded"); }
            *destination=i32::try_from(exact).map_err(|_| "invalidLegacy")?;
        }
    }
    if let Some(value) = raw.get("socketSlots") { item.socket_slots=u8::try_from(unsigned(value,255)?).map_err(|_| "invalidLegacy")?; }
    Ok(item)
}

fn parse_player(raw: &mut Value, stat_budget: &mut i64) -> Result<(ItemTooltipPlayer, bool), &'static str> {
    let player = object(raw)?;
    let level = player.get("level").ok_or("missingPlayerLevel")?;
    unsigned(level, u32::MAX as u64)?;
    if let Some(stats) = player.get("crystalStats").filter(|v| !v.is_null()) {
        stats_array(stats, stat_budget)?;
        let mut ids = HashSet::new();
        for stat in stats.as_array().ok_or("invalidPlayerStats")? {
            if !ids.insert(stat.get("stat").and_then(Value::as_u64).ok_or("invalidPlayerStat")?) { return Err("duplicatePlayerStat"); }
        }
    }
    if let Some(weights) = player.get("weights").filter(|v| !v.is_null()) {
        let weights = weights.as_object().ok_or("invalidPlayerWeights")?;
        exact_keys(weights, &["bag", "wear", "hand"])?;
        for key in ["bag", "wear", "hand"] {
            unsigned(weights.get(key).ok_or("incompletePlayerWeights")?, u32::MAX as u64)?;
        }
    }
    for key in ["className", "gender"] {
        if let Some(value) = player.get(key).filter(|v| !v.is_null()) {
            let name = value.as_str().ok_or("invalidPlayerField")?;
            let valid = match key {
                "className" => matches!(name, "Warrior" | "Wizard" | "Taoist" | "Assassin" | "Archer"
                    | "warrior" | "wizard" | "taoist" | "assassin" | "archer"),
                _ => matches!(name, "Male" | "Female" | "male" | "female"),
            };
            if !valid { return Err("invalidPlayerField"); }
        }
    }
    if player.get("experience").is_some_and(|v| v.as_i64().is_none_or(|v| v.unsigned_abs() > MAX_SAFE_JS_INTEGER))
        || player.get("maxExperience").is_some_and(|v| v.as_i64().is_none_or(|v| v.unsigned_abs() > MAX_SAFE_JS_INTEGER)) {
        return Err("unsafePlayerInteger");
    }
    // A flag alone cannot invent the missing server weight block.
    if player.get("weights").is_none_or(Value::is_null) {
        player.insert("currentWeightKnown".into(), Value::Bool(false));
    } else {
        player.entry("currentWeightKnown").or_insert(Value::Bool(false));
    }
    let known = ["crystalStats", "weights", "className", "gender"].into_iter()
        .all(|key| player.get(key).is_some_and(|v| !v.is_null()));
    validate_player_fields(player)?;
    let parsed = ItemTooltipPlayer {
        level: u32::try_from(player["level"].as_u64().ok_or("missingPlayerLevel")?).map_err(|_| "invalidPlayer")?,
        class_name: player.get("className").and_then(Value::as_str).map(str::to_owned),
        crystal_stats: player.get("crystalStats").filter(|v| !v.is_null()).map(parse_stats),
    };
    Ok((parsed, known))
}

fn validate_player_fields(player: &Map<String,Value>) -> Result<(), &'static str> {
    // Preserve the old typed PlayerStats field validation without linking Bevy.
    for key in ["hp", "maxHp", "mp", "maxMp"] { if let Some(v)=player.get(key) { signed(v,i32::MIN as i64,i32::MAX as i64)?; } }
    for (key,max) in [("gold",u32::MAX as u64),("credit",u32::MAX as u64),("currentWeight",65535),("maxWeight",65535),("hair",255),("wingEffect",255)] {
        if let Some(v)=player.get(key).filter(|v| !v.is_null() || !matches!(key,"hair"|"wingEffect")) { unsigned(v,max)?; }
    }
    for key in ["currentWeightKnown","inSafeZone"] { if let Some(v)=player.get(key) { boolean(v)?; } }
    for key in ["name","guildName","guildRankName","mapName"] { if let Some(v)=player.get(key).filter(|v| !v.is_null()) { bounded_string(v,MAX_STRING_BYTES)?; } }
    Ok(())
}

fn validate_source(raw: &mut Value, item: &QueryItem, reasons: &mut Vec<&'static str>, stat_budget: &mut i64)
    -> Result<Option<CrystalItemTooltipSourceModel>, &'static str>
{
    let source = object(raw)?;
    exact_keys(source, &["info", "realInfo", "userItem", "socketInfos", "realSocketInfos"])?;
    let mut state = SourceValidation { stat_budget: *stat_budget, ..Default::default() };
    let info_complete = if let Some(info) = source.get("info") {
        let complete = validate_info(info, &mut state.stat_budget)?;
        matching_index(info, item.item_index)?;
        complete
    } else { false };
    if !info_complete { reason(reasons, "incompleteInfo"); }
    let user_complete = if let Some(user) = source.get_mut("userItem").filter(|v| !v.is_null()) {
        let complete = validate_user(user, &mut state, 0)?;
        let user = user.as_object().ok_or("invalidUserItem")?;
        matching_unsigned(user, "unique_id", item.unique_id)?;
        matching_unsigned(user, "count", u64::from(item.count))?;
        matching_index(&Value::Object(user.clone()), item.item_index)?;
        complete
    } else { reason(reasons, "missingUserItem"); false };
    if !user_complete { reason(reasons, "incompleteUserItem"); }
    let mut raw_complete = info_complete && user_complete;
    if let Some(info) = source.get("realInfo").filter(|v| !v.is_null()) {
        if !validate_info(info, &mut state.stat_budget)? {
            raw_complete = false; reason(reasons, "incompleteRealInfo");
        }
    }
    for key in ["socketInfos", "realSocketInfos"] {
        if let Some(infos) = source.get(key) {
            let infos = infos.as_array().ok_or("invalidSocketInfos")?;
            if infos.len() > MAX_SOCKETS { return Err("tooManySockets"); }
            for info in infos.iter().filter(|info| !info.is_null()) {
                if !validate_info(info, &mut state.stat_budget)? {
                    raw_complete = false; reason(reasons, "incompleteSocketInfo");
                }
            }
        }
    }
    // Bind each original socket template to its actual physical socket index;
    // resolved class/level variants may legitimately use another template id.
    if let Some(slots) = source.get("userItem").and_then(|v| v.get("slots")).and_then(Value::as_array) {
        for key in ["socketInfos", "realSocketInfos"] {
            if source.get(key).and_then(Value::as_array).is_some_and(|infos| infos.len() > slots.len()) {
                return Err("socketIdentityMismatch");
            }
        }
        for (index, socket) in slots.iter().enumerate() {
            let base = source.get("socketInfos").and_then(Value::as_array).and_then(|infos| infos.get(index));
            let real = source.get("realSocketInfos").and_then(Value::as_array).and_then(|infos| infos.get(index));
            if socket.is_null() {
                if base.is_some_and(|v| !v.is_null()) || real.is_some_and(|v| !v.is_null()) {
                    return Err("socketIdentityMismatch");
                }
                continue;
            }
            if let Some(base) = base.filter(|v| !v.is_null()) {
                if let Some(index) = socket.get("item_index").and_then(Value::as_i64) {
                    matching_index(base, i32::try_from(index).map_err(|_| "invalidItemIndex")?)?;
                }
                if dynamic_info(base) && real.is_none_or(Value::is_null) {
                    reason(reasons, "missingRealSocketInfo");
                }
            } else { reason(reasons, "missingSocketInfo"); }
        }
    }
    if source.get("info").is_some_and(dynamic_info)
        && source.get("realInfo").is_none_or(Value::is_null) {
        reason(reasons, "missingRealInfo");
    }
    *stat_budget = state.stat_budget;
    if !raw_complete { return Ok(None); }
    Ok(Some(parse_crystal_item_tooltip_source_model(&Value::Object(source.clone()))))
}

#[derive(Default)]
struct SourceValidation { ids: HashSet<u64>, item_nodes: usize, stat_budget: i64 }

fn validate_info(info: &Value, budget: &mut i64) -> Result<bool, &'static str> {
    let info = info.as_object().ok_or("invalidInfo")?;
    let mut complete = true;
    for (key, max) in [
        ("item_type", 255), ("grade", 255), ("required_type", 255), ("required_class", 255),
        ("required_gender", 255), ("item_set", 255), ("weight", 255), ("light", 255),
        ("required_amount", 255), ("image", 65_535), ("durability", 65_535),
        ("stack_size", 65_535), ("price", u32::MAX as u64), ("effect", 255),
        ("random_stats_id", 255), ("slots", 255),
    ] {
        if let Some(v) = info.get(key) { unsigned(v, max)?; } else { complete = false; }
    }
    for (key, min, max) in [("item_index", i32::MIN as i64, i32::MAX as i64),
        ("shape", i16::MIN as i64, i16::MAX as i64), ("bind", i16::MIN as i64, i16::MAX as i64),
        ("unique", i16::MIN as i64, i16::MAX as i64)] {
        if let Some(v) = info.get(key) { signed(v, min, max)?; } else { complete = false; }
    }
    for key in ["start_item", "need_identify", "show_group_pickup", "class_based", "level_based",
        "can_mine", "global_drop_notify", "can_fast_run", "can_awakening"] {
        if let Some(v) = info.get(key) { boolean(v)?; } else { complete = false; }
    }
    if let Some(name) = info.get("name") { bounded_string(name, 512)?; } else { complete = false; }
    if let Some(tooltip) = info.get("tooltip") {
        if !tooltip.is_null() { bounded_string(tooltip, 8_192)?; }
    } else { complete = false; }
    if let Some(stats) = info.get("stats") { stats_array(stats, budget)?; } else { complete = false; }
    Ok(complete)
}

fn validate_user(user: &mut Value, state: &mut SourceValidation, depth: usize) -> Result<bool, &'static str> {
    state.item_nodes += 1;
    if depth > 4 || state.item_nodes > MAX_ITEM_NODES { return Err("tooManyItemNodes"); }
    let user = object(user)?;
    let mut complete = true;
    if let Some(id) = user.get("unique_id") {
        let id = unsigned(id, MAX_SAFE_JS_INTEGER)?;
        if !state.ids.insert(id) { return Err("duplicateItemIdentity"); }
    } else { complete = false; }
    for (key, max) in [("current_dura", 65_535), ("max_dura", 65_535), ("count", 65_535),
        ("gem_count", 65_535), ("awake_type", 255), ("refined_value", 255), ("refine_added", 255)] {
        if let Some(v) = user.get(key) {
            let value = unsigned(v, max)?;
            if key == "count" && value == 0 { return Err("invalidItemCount"); }
        } else { complete = false; }
    }
    for key in ["item_index", "soul_bound_id", "refine_success_chance", "wedding_ring"] {
        if let Some(v) = user.get(key) { signed(v, i32::MIN as i64, i32::MAX as i64)?; }
        else { complete = false; }
    }
    for key in ["identified", "cursed", "is_shop_item", "gm_made"] {
        if let Some(v) = user.get(key) { boolean(v)?; } else { complete = false; }
    }
    if let Some(stats) = user.get("added_stats") { stats_array(stats, &mut state.stat_budget)?; }
    else { complete = false; }
    if let Some(awake) = user.get("awake_values") {
        let awake = awake.as_array().ok_or("invalidAwakening")?;
        if awake.len() > MAX_SOCKETS { return Err("tooManyAwakeningValues"); }
        for v in awake { unsigned(v, 255)?; }
    } else { complete = false; }
    if let Some(slots) = user.get_mut("slots") {
        let slots = slots.as_array_mut().ok_or("invalidSockets")?;
        if slots.len() > MAX_SOCKETS { return Err("tooManySockets"); }
        for socket in slots.iter_mut().filter(|v| !v.is_null()) {
            complete &= validate_user(socket, state, depth + 1)?;
        }
    } else { complete = false; }
    for key in ["expire_info", "rental_information", "sealed_info"] {
        if let Some(v) = user.get_mut(key) {
            if !v.is_null() { complete &= validate_dates(v, key)?; }
        } else { complete = false; }
    }
    Ok(complete)
}

fn validate_dates(raw: &mut Value, kind: &str) -> Result<bool, &'static str> {
    let fields: &[&str] = match kind {
        "expire_info" => &["expiry_binary_datetime"],
        "sealed_info" => &["expiry_binary_datetime", "next_seal_binary_datetime"],
        _ => &["owner_name", "binding_flags", "expiry_binary_datetime", "rental_locked"],
    };
    let values = object(raw)?;
    exact_keys(values, fields)?;
    let complete = fields.iter().all(|key| values.contains_key(*key));
    for key in ["expiry_binary_datetime", "next_seal_binary_datetime"] {
        if let Some(value) = values.get_mut(key) {
            let exact = if let Some(decimal) = value.as_str() {
                let exact = decimal.parse::<i64>().map_err(|_| "invalidBinaryDateTime")?;
                if exact.to_string() != decimal { return Err("invalidBinaryDateTime"); }
                exact
            } else {
                let exact = value.as_i64().ok_or("invalidBinaryDateTime")?;
                if exact.unsigned_abs() > MAX_SAFE_JS_INTEGER { return Err("unsafeBinaryDateTime"); }
                exact
            };
            // Rust-to-Rust conversion is exact, including signed .NET kind bits.
            // Rental/Sealed's current model takes i64; no JS Number is involved.
            *value = Value::Number(exact.into());
        }
    }
    if let Some(v) = values.get("owner_name") { bounded_string(v, 512)?; }
    if let Some(v) = values.get("binding_flags") { signed(v, i16::MIN as i64, i16::MAX as i64)?; }
    if let Some(v) = values.get("rental_locked") { boolean(v)?; }
    Ok(complete)
}

fn stats_array(stats: &Value, budget: &mut i64) -> Result<(), &'static str> {
    let stats = stats.as_array().ok_or("invalidStats")?;
    if stats.len() > MAX_ARRAY { return Err("tooManyStats"); }
    for entry in stats {
        let entry = entry.as_object().ok_or("invalidStat")?;
        exact_keys(entry, &["stat", "value"])?;
        unsigned(entry.get("stat").ok_or("incompleteStat")?, 255)?;
        let value = signed(entry.get("value").ok_or("incompleteStat")?, i32::MIN as i64, i32::MAX as i64)?;
        *budget += value.abs();
        if *budget > MAX_STAT_BUDGET { return Err("statBudgetExceeded"); }
    }
    Ok(())
}

fn parse_stats(value: &Value) -> Vec<CrystalItemStatModel> {
    value.as_array().map(|stats| stats.iter().map(parse_crystal_item_stat_model).collect()).unwrap_or_default()
}

fn parse_crystal_item_info_model(value: &Value) -> CrystalItemInfoModel {
    CrystalItemInfoModel {
        item_index: i32::try_from(value.get("item_index").and_then(Value::as_i64).unwrap_or(0)).unwrap_or_default(),
        name: value.get("name").and_then(Value::as_str).unwrap_or_default().to_owned(),
        item_type: u8::try_from(value.get("item_type").and_then(Value::as_u64).unwrap_or(0)).unwrap_or_default(),
        grade: u8::try_from(value.get("grade").and_then(Value::as_u64).unwrap_or(0)).unwrap_or_default(),
        required_type: u8::try_from(value.get("required_type").and_then(Value::as_u64).unwrap_or(0)).unwrap_or_default(),
        required_class: u8::try_from(value.get("required_class").and_then(Value::as_u64).unwrap_or(31)).unwrap_or_default(),
        required_gender: u8::try_from(value.get("required_gender").and_then(Value::as_u64).unwrap_or(3)).unwrap_or_default(),
        item_set: u8::try_from(value.get("item_set").and_then(Value::as_u64).unwrap_or(0)).unwrap_or_default(),
        shape: i16::try_from(value.get("shape").and_then(Value::as_i64).unwrap_or(0)).unwrap_or_default(),
        weight: u8::try_from(value.get("weight").and_then(Value::as_u64).unwrap_or(0)).unwrap_or_default(),
        light: u8::try_from(value.get("light").and_then(Value::as_u64).unwrap_or(0)).unwrap_or_default(),
        required_amount: u8::try_from(value.get("required_amount").and_then(Value::as_u64).unwrap_or(0)).unwrap_or_default(),
        image: u16::try_from(value.get("image").and_then(Value::as_u64).unwrap_or(0)).unwrap_or_default(),
        durability: u16::try_from(value.get("durability").and_then(Value::as_u64).unwrap_or(0)).unwrap_or_default(),
        stack_size: u16::try_from(value.get("stack_size").and_then(Value::as_u64).unwrap_or(1)).unwrap_or_default(),
        price: u32::try_from(value.get("price").and_then(Value::as_u64).unwrap_or(0)).unwrap_or_default(),
        start_item: value.get("start_item").and_then(Value::as_bool).unwrap_or_default(),
        effect: u8::try_from(value.get("effect").and_then(Value::as_u64).unwrap_or(0)).unwrap_or_default(),
        need_identify: value.get("need_identify").and_then(Value::as_bool).unwrap_or_default(),
        show_group_pickup: value.get("show_group_pickup").and_then(Value::as_bool).unwrap_or_default(),
        class_based: value.get("class_based").and_then(Value::as_bool).unwrap_or_default(),
        level_based: value.get("level_based").and_then(Value::as_bool).unwrap_or_default(),
        can_mine: value.get("can_mine").and_then(Value::as_bool).unwrap_or_default(),
        global_drop_notify: value.get("global_drop_notify").and_then(Value::as_bool).unwrap_or_default(),
        bind: i16::try_from(value.get("bind").and_then(Value::as_i64).unwrap_or(0)).unwrap_or_default(),
        unique: i16::try_from(value.get("unique").and_then(Value::as_i64).unwrap_or(0)).unwrap_or_default(),
        random_stats_id: u8::try_from(value.get("random_stats_id").and_then(Value::as_u64).unwrap_or(0)).unwrap_or_default(),
        can_fast_run: value.get("can_fast_run").and_then(Value::as_bool).unwrap_or_default(),
        can_awakening: value.get("can_awakening").and_then(Value::as_bool).unwrap_or_default(),
        slots: u8::try_from(value.get("slots").and_then(Value::as_u64).unwrap_or(0)).unwrap_or_default(),
        stats: value.get("stats").map(parse_stats).unwrap_or_default(),
        tooltip: value.get("tooltip").and_then(Value::as_str).map(str::to_owned),
    }
}

fn parse_crystal_item_stat_model(value: &Value) -> CrystalItemStatModel {
    CrystalItemStatModel {
        stat: u8::try_from(value.get("stat").and_then(Value::as_u64).unwrap_or(0)).unwrap_or_default(),
        value: i32::try_from(value.get("value").and_then(Value::as_i64).unwrap_or(0)).unwrap_or_default(),
    }
}

fn parse_crystal_user_item_model(value: &Value) -> CrystalUserItemModel {
    CrystalUserItemModel {
        unique_id: u64::try_from(value.get("unique_id").and_then(Value::as_u64).unwrap_or(0)).unwrap_or_default(),
        item_index: i32::try_from(value.get("item_index").and_then(Value::as_i64).unwrap_or(0)).unwrap_or_default(),
        current_dura: u16::try_from(value.get("current_dura").and_then(Value::as_u64).unwrap_or(0)).unwrap_or_default(),
        max_dura: u16::try_from(value.get("max_dura").and_then(Value::as_u64).unwrap_or(0)).unwrap_or_default(),
        count: u16::try_from(value.get("count").and_then(Value::as_u64).unwrap_or(0)).unwrap_or_default(),
        soul_bound_id: i32::try_from(value.get("soul_bound_id").and_then(Value::as_i64).unwrap_or(-1)).unwrap_or_default(),
        identified: value.get("identified").and_then(Value::as_bool).unwrap_or(true),
        cursed: value.get("cursed").and_then(Value::as_bool).unwrap_or_default(),
        slots: value.get("slots").and_then(Value::as_array).map(|v| v.iter().map(|socket| (!socket.is_null()).then(|| parse_crystal_user_item_model(socket))).collect()).unwrap_or_default(),
        gem_count: u16::try_from(value.get("gem_count").and_then(Value::as_u64).unwrap_or(0)).unwrap_or_default(),
        added_stats: value.get("added_stats").map(parse_stats).unwrap_or_default(),
        awake_type: u8::try_from(value.get("awake_type").and_then(Value::as_u64).unwrap_or(0)).unwrap_or_default(),
        awake_values: value.get("awake_values").and_then(Value::as_array).map(|v| v.iter().filter_map(|n| n.as_u64().and_then(|n| u8::try_from(n).ok())).collect()).unwrap_or_default(),
        refined_value: u8::try_from(value.get("refined_value").and_then(Value::as_u64).unwrap_or(0)).unwrap_or_default(),
        refine_added: u8::try_from(value.get("refine_added").and_then(Value::as_u64).unwrap_or(0)).unwrap_or_default(),
        refine_success_chance: i32::try_from(value.get("refine_success_chance").and_then(Value::as_i64).unwrap_or(0)).unwrap_or_default(),
        wedding_ring: i32::try_from(value.get("wedding_ring").and_then(Value::as_i64).unwrap_or(-1)).unwrap_or_default(),
        expire_info: value.get("expire_info").filter(|v| !v.is_null()).map(parse_crystal_user_item_expire_model),
        rental_information: value.get("rental_information").filter(|v| !v.is_null()).map(parse_crystal_user_item_rental_model),
        is_shop_item: value.get("is_shop_item").and_then(Value::as_bool).unwrap_or_default(),
        sealed_info: value.get("sealed_info").filter(|v| !v.is_null()).map(parse_crystal_user_item_sealed_model),
        gm_made: value.get("gm_made").and_then(Value::as_bool).unwrap_or_default(),
    }
}

fn parse_crystal_user_item_expire_model(value: &Value) -> CrystalUserItemExpireModel {
    CrystalUserItemExpireModel {
        expiry_binary_datetime: i64::try_from(value.get("expiry_binary_datetime").and_then(Value::as_i64).unwrap_or(0)).unwrap_or_default(),
    }
}

fn parse_crystal_user_item_rental_model(value: &Value) -> CrystalUserItemRentalModel {
    CrystalUserItemRentalModel {
        owner_name: value.get("owner_name").and_then(Value::as_str).unwrap_or_default().to_owned(),
        binding_flags: i16::try_from(value.get("binding_flags").and_then(Value::as_i64).unwrap_or(0)).unwrap_or_default(),
        expiry_binary_datetime: i64::try_from(value.get("expiry_binary_datetime").and_then(Value::as_i64).unwrap_or(0)).unwrap_or_default(),
        rental_locked: value.get("rental_locked").and_then(Value::as_bool).unwrap_or_default(),
    }
}

fn parse_crystal_user_item_sealed_model(value: &Value) -> CrystalUserItemSealedModel {
    CrystalUserItemSealedModel {
        expiry_binary_datetime: i64::try_from(value.get("expiry_binary_datetime").and_then(Value::as_i64).unwrap_or(0)).unwrap_or_default(),
        next_seal_binary_datetime: i64::try_from(value.get("next_seal_binary_datetime").and_then(Value::as_i64).unwrap_or(0)).unwrap_or_default(),
    }
}

fn parse_crystal_item_tooltip_source_model(value: &Value) -> CrystalItemTooltipSourceModel {
    CrystalItemTooltipSourceModel {
        info: parse_crystal_item_info_model(&value["info"]),
        real_info: value.get("realInfo").filter(|v| !v.is_null()).map(parse_crystal_item_info_model),
        user_item: value.get("userItem").filter(|v| !v.is_null()).map(parse_crystal_user_item_model),
        socket_infos: value.get("socketInfos").and_then(Value::as_array).map(|v| v.iter().map(|info| (!info.is_null()).then(|| parse_crystal_item_info_model(info))).collect()).unwrap_or_default(),
        real_socket_infos: value.get("realSocketInfos").and_then(Value::as_array).map(|v| v.iter().map(|info| (!info.is_null()).then(|| parse_crystal_item_info_model(info))).collect()).unwrap_or_default(),
    }
}

fn document_json(document: &CrystalItemTooltipDocument) -> Value {
    json!({"sections":document.sections.iter().map(|section| json!({
        "kind":section_kind(section.kind),
        "lines":section.lines.iter().map(|line| json!({"text":line.text,
            "colour":colour_name(line.colour)})).collect::<Vec<_>>()
    })).collect::<Vec<_>>(),"broken":document.broken,"sourceComplete":document.source_complete})
}

fn section_kind(kind: CrystalItemTooltipSectionKind) -> &'static str {
    match kind {
        CrystalItemTooltipSectionKind::Name => "name", CrystalItemTooltipSectionKind::Attack => "attack",
        CrystalItemTooltipSectionKind::Defence => "defence", CrystalItemTooltipSectionKind::Weight => "weight",
        CrystalItemTooltipSectionKind::Awake => "awake", CrystalItemTooltipSectionKind::Socket => "socket",
        CrystalItemTooltipSectionKind::Need => "need", CrystalItemTooltipSectionKind::Bind => "bind",
        CrystalItemTooltipSectionKind::Overlap => "overlap", CrystalItemTooltipSectionKind::Story => "story",
        CrystalItemTooltipSectionKind::GmMade => "gmMade",
    }
}

fn colour_name(colour: CrystalItemTooltipColour) -> &'static str {
    match colour {
        CrystalItemTooltipColour::White => "white", CrystalItemTooltipColour::Yellow => "yellow",
        CrystalItemTooltipColour::DeepSkyBlue => "deepSkyBlue", CrystalItemTooltipColour::DarkOrange => "darkOrange",
        CrystalItemTooltipColour::Plum => "plum", CrystalItemTooltipColour::Red => "red",
        CrystalItemTooltipColour::Cyan => "cyan", CrystalItemTooltipColour::DarkKhaki => "darkKhaki",
        CrystalItemTooltipColour::Khaki => "khaki", CrystalItemTooltipColour::Orchid => "orchid",
    }
}

fn object(value: &mut Value) -> Result<&mut Map<String, Value>, &'static str> {
    value.as_object_mut().ok_or("expectedObject")
}
fn exact_keys(value: &Map<String, Value>, keys: &[&str]) -> Result<(), &'static str> {
    if value.keys().any(|key| !keys.contains(&key.as_str())) { return Err("unexpectedField"); }
    Ok(())
}
fn unsigned(value: &Value, max: u64) -> Result<u64, &'static str> {
    value.as_u64().filter(|v| *v <= max).ok_or("invalidUnsignedInteger")
}
fn signed(value: &Value, min: i64, max: i64) -> Result<i64, &'static str> {
    value.as_i64().filter(|v| (min..=max).contains(v)).ok_or("invalidSignedInteger")
}
fn boolean(value: &Value) -> Result<(), &'static str> {
    value.as_bool().map(|_| ()).ok_or("invalidBoolean")
}
fn bounded_string(value: &Value, max: usize) -> Result<(), &'static str> {
    value.as_str().filter(|v| v.len() <= max).map(|_| ()).ok_or("invalidString")
}
fn matching_index(value: &Value, expected: i32) -> Result<(), &'static str> {
    if value.get("item_index").is_some_and(|v| v.as_i64() != Some(i64::from(expected))) {
        return Err("itemIdentityMismatch");
    }
    Ok(())
}
fn matching_unsigned(value: &Map<String, Value>, key: &str, expected: u64) -> Result<(), &'static str> {
    if value.get(key).is_some_and(|v| v.as_u64() != Some(expected)) { return Err("itemIdentityMismatch"); }
    Ok(())
}
fn dynamic_info(info: &Value) -> bool {
    ["class_based", "level_based"].into_iter().any(|key| info.get(key).and_then(Value::as_bool) == Some(true))
}
fn reason(reasons: &mut Vec<&'static str>, value: &'static str) {
    if !reasons.contains(&value) { reasons.push(value); }
}
fn bounded_value(value: &Value, depth: usize, nodes: &mut usize) -> Result<(), &'static str> {
    bounded_value_with_array_limit::<MAX_ARRAY>(value, depth, nodes)
}
fn bounded_value_with_array_limit<const ARRAY_LIMIT: usize>(value: &Value, depth: usize, nodes: &mut usize) -> Result<(), &'static str> {
    *nodes += 1;
    if depth > MAX_DEPTH || *nodes > MAX_NODES.saturating_mul((ARRAY_LIMIT / MAX_ARRAY).max(1)) { return Err("jsonLimitExceeded"); }
    match value {
        Value::String(v) if v.len() > MAX_STRING_BYTES => Err("stringLimitExceeded"),
        Value::Array(values) => {
            if values.len() > ARRAY_LIMIT { return Err("arrayLimitExceeded"); }
            for value in values { bounded_value_with_array_limit::<ARRAY_LIMIT>(value, depth + 1, nodes)?; }
            Ok(())
        }
        Value::Object(values) => {
            if values.len() > 64 || values.keys().any(|v| v.len() > 128) { return Err("objectLimitExceeded"); }
            for value in values.values() { bounded_value_with_array_limit::<ARRAY_LIMIT>(value, depth + 1, nodes)?; }
            Ok(())
        }
        _ => Ok(()),
    }
}

// Duplicate fields are rejected before they can overwrite an identity or a
// raw-source presence check. serde_json's normal recursion limit also applies.
struct StrictValue(Value);
impl<'de> Deserialize<'de> for StrictValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        StrictJsonValue::<MAX_ARRAY>::deserialize(deserializer).map(|value| StrictValue(value.0))
    }
}
pub(crate) struct StrictJsonValue<const ARRAY_LIMIT: usize>(pub(crate) Value);
impl<'de, const ARRAY_LIMIT: usize> Deserialize<'de> for StrictJsonValue<ARRAY_LIMIT> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct StrictVisitor<const ARRAY_LIMIT: usize>;
        impl<'de, const ARRAY_LIMIT: usize> Visitor<'de> for StrictVisitor<ARRAY_LIMIT> {
            type Value = StrictJsonValue<ARRAY_LIMIT>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result { f.write_str("bounded JSON without duplicate fields") }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> { Ok(StrictJsonValue(v.into())) }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> { Ok(StrictJsonValue(v.into())) }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> { Ok(StrictJsonValue(v.into())) }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
                serde_json::Number::from_f64(v).map(|v| StrictJsonValue(Value::Number(v))).ok_or_else(|| E::custom("nonfinite JSON"))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> { self.visit_string(v.to_owned()) }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
                if v.len() > MAX_STRING_BYTES { return Err(E::custom("JSON string too large")); }
                Ok(StrictJsonValue(Value::String(v)))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> { Ok(StrictJsonValue(Value::Null)) }
            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> { self.visit_unit() }
            fn visit_seq<A: SeqAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(StrictJsonValue(value)) = access.next_element::<StrictJsonValue<ARRAY_LIMIT>>()? {
                    if values.len() >= ARRAY_LIMIT { return Err(de::Error::custom("JSON array too large")); }
                    values.push(value);
                }
                Ok(StrictJsonValue(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
                let mut values = Map::new();
                while let Some(key) = access.next_key::<String>()? {
                    if values.len() >= 64 || key.len() > 128 || values.contains_key(&key) {
                        return Err(de::Error::custom("duplicate or oversized JSON field"));
                    }
                    let StrictJsonValue(value) = access.next_value::<StrictJsonValue<ARRAY_LIMIT>>()?;
                    values.insert(key, value);
                }
                Ok(StrictJsonValue(Value::Object(values)))
            }
        }
        deserializer.deserialize_any(StrictVisitor::<ARRAY_LIMIT>)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    const NOW: &str = "621355968000000000";

    fn info(index: i32, name: &str) -> Value {
        json!({"item_index":index,"name":name,"item_type":1,"grade":1,"required_type":0,"required_class":31,"required_gender":3,"item_set":0,"shape":0,"weight":4,"light":0,"required_amount":5,"image":0,"durability":4000,"stack_size":1,"price":999999,"start_item":false,"effect":0,"need_identify":false,"show_group_pickup":false,"class_based":false,"level_based":false,"can_mine":false,"global_drop_notify":false,"bind":0,"unique":0,"random_stats_id":0,"can_fast_run":false,"can_awakening":false,"slots":0,"stats":[{"stat":4,"value":2},{"stat":5,"value":4}],"tooltip":null})
    }
    fn user(id: u64, index: i32) -> Value {
        json!({"unique_id":id,"item_index":index,"current_dura":3000,"max_dura":4000,"count":1,"soul_bound_id":-1,"identified":true,"cursed":false,"slots":[],"gem_count":0,"added_stats":[{"stat":5,"value":1}],"awake_type":0,"awake_values":[],"refined_value":0,"refine_added":0,"refine_success_chance":0,"wedding_ring":-1,"expire_info":null,"rental_information":null,"is_shop_item":false,"sealed_info":null,"gm_made":false})
    }
    fn fixture() -> Value {
        json!({"version":1,"item":{"uniqueId":0,"itemIndex":-23,"name":"Wooden Sword","icon":0,"count":1,"legacy":{"sellValue":25},"tooltipSource":{"info":info(-23,"WoodenSword"),"userItem":user(0,-23),"realInfo":null,"socketInfos":[],"realSocketInfos":[]}},"player":{"level":3,"className":"Warrior","gender":"Male","crystalStats":[],"weights":{"bag":0,"wear":0,"hand":0}}})
    }
    fn response(request: &Value) -> Value {
        serde_json::from_str(&item_tooltip_document(&request.to_string(),NOW)).unwrap()
    }
    fn text(response: &Value) -> String {
        response["document"]["sections"].as_array().unwrap().iter().flat_map(|s| s["lines"].as_array().unwrap()).map(|l| l["text"].as_str().unwrap()).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn abi_instance_matches_native_golden_with_zero_uid_negative_index_and_real_sell_value() {
        assert_eq!(item_tooltip_abi_version(),1);
        let result=response(&fixture());
        assert_eq!(result["ok"],true);
        assert_eq!(result["viewerComplete"],true);
        assert_eq!(result["document"]["sourceComplete"],true);
        assert_eq!(result["partialReasons"],json!([]));
        assert_eq!(text(&result),"木剑\nCommon\nWeapon\nW: 4  Durability: 3/4\nDC + 2~5 (+1)\nRequired Level : 5\nSelling Price : 25 Gold");
        assert_eq!(result["document"]["sections"][2]["kind"],"need");
        assert_eq!(result["document"]["sections"][2]["lines"][0]["colour"],"red");
        assert!(!text(&result).contains("999,999"));
    }

    #[test]
    fn legacy_fields_are_independent_and_incomplete_source_never_invents_template_stats() {
        let mut request=fixture();
        request["item"]["legacy"]=json!({"key":"GreenPoison","grade":"rare","description":"Original story\nsecond line","sellValue":25,"durabilityCurrent":0,"durabilityMax":5000,"attack":7,"defence":8,"addedAttack":2,"addedDefence":3,"addedLuck":-1,"socketSlots":2});
        request["item"]["tooltipSource"]["info"].as_object_mut().unwrap().remove("required_class");
        let result=response(&request);
        assert_eq!(result["ok"],true);
        assert_eq!(result["document"]["sourceComplete"],false);
        assert_eq!(result["document"]["broken"],true);
        assert!(result["partialReasons"].as_array().unwrap().contains(&json!("incompleteInfo")));
        let label=text(&result);
        assert!(label.contains("Attack: 7 (+2)"));
        assert!(label.contains("Defence: 8 (+3)"));
        assert!(label.contains("Luck: -1"));
        assert!(label.contains("Sockets: 2"));
        assert!(label.contains("Original story\nsecond line"));
        assert!(!label.contains("DC +"));
        assert!(!label.contains("Selling Price"));
        let parsed=parse_legacy(request["item"].get("legacy"),&mut 0).unwrap();
        assert_eq!(parsed.key,"GreenPoison");
        assert_eq!(parsed.sell_value,25);
        request["item"].as_object_mut().unwrap().remove("tooltipSource");
        let absent=response(&request);
        assert_eq!(absent["partialReasons"],json!(["missingSource"]));
        request["item"].as_object_mut().unwrap().remove("legacy");
        let minimal=response(&request);
        assert_eq!(text(&minimal),"木剑");
        assert_eq!(minimal["document"]["broken"],false);
    }

    #[test]
    fn all_full_info_and_user_presence_fields_are_checked_before_dto_defaults() {
        let original=fixture();
        for target in ["info","userItem"] {
            let keys=original["item"]["tooltipSource"][target].as_object().unwrap().keys().cloned().collect::<Vec<_>>();
            for key in keys {
                let mut request=original.clone();
                request["item"]["tooltipSource"][target].as_object_mut().unwrap().remove(&key);
                let result=response(&request);
                assert_eq!(result["ok"],true,"missing {target}.{key} should be explicit partial");
                assert_eq!(result["document"]["sourceComplete"],false,"missing {target}.{key} cannot become complete through defaults");
                assert_eq!(text(&result),"木剑");
            }
        }
    }

    #[test]
    fn viewer_missing_class_stats_gender_or_weights_remains_incomplete() {
        let mut request=fixture();
        request["item"]["tooltipSource"]["info"]["required_type"]=json!(3);
        request["item"]["tooltipSource"]["info"]["required_class"]=json!(1);
        for key in ["className","crystalStats","gender","weights"] {
            let mut partial=request.clone();
            partial["player"].as_object_mut().unwrap().remove(key);
            let result=response(&partial);
            assert_eq!(result["ok"],true);
            assert_eq!(result["viewerComplete"],false);
            if key=="crystalStats" { assert_eq!(result["document"]["sections"][2]["lines"][0]["colour"],"white"); }
            if key=="className" { assert_eq!(result["document"]["sections"][2]["lines"][1]["colour"],"white"); }
        }
        request["player"]["crystalStats"]=json!([{"stat":5,"value":2},{"stat":5,"value":3}]);
        assert_eq!(response(&request)["error"],"duplicatePlayerStat");
    }

    #[test]
    fn exact_clock_and_binary_dates_preserve_subsecond_expiry_and_kind_bits() {
        let mut request=fixture();
        for clock in ["", "+1", "01", "-0", " 1", "1.0", "9223372036854775808"] {
            let result:Value=serde_json::from_str(&item_tooltip_document(&request.to_string(),clock)).unwrap();
            assert_eq!(result["error"],"invalidClock");
        }
        for clock in ["0", "-1", "9223372036854775807", "-9223372036854775808"] {
            let result:Value=serde_json::from_str(&item_tooltip_document(&request.to_string(),clock)).unwrap();
            assert_eq!(result["ok"],true);
        }
        request["item"]["tooltipSource"]["userItem"]["expire_info"]=json!({"expiry_binary_datetime":"621355968000000001"});
        assert!(text(&response(&request)).contains("Expires in 0s"));
        request["item"]["tooltipSource"]["userItem"]["expire_info"]["expiry_binary_datetime"]=json!("621355968000000000");
        assert!(text(&response(&request)).contains("Expired"));
        for date in ["3155378975999999999","7767064994427387903"] {
            request["item"]["tooltipSource"]["userItem"]["expire_info"]["expiry_binary_datetime"]=json!(date);
            let label=text(&response(&request));
            assert!(label.contains("Expires in")); assert!(!label.contains("Expired"));
        }
        request["item"]["tooltipSource"]["userItem"]["expire_info"]["expiry_binary_datetime"]=json!(621355968000000001_i64);
        assert_eq!(response(&request)["error"],"unsafeBinaryDateTime");
        for date in ["01", "+1", "-0", "9223372036854775808"] {
            request["item"]["tooltipSource"]["userItem"]["expire_info"]["expiry_binary_datetime"]=json!(date);
            assert_eq!(response(&request)["error"],"invalidBinaryDateTime");
        }
        request["item"]["tooltipSource"]["userItem"]["expire_info"]["expiry_binary_datetime"]=json!(0);
        assert!(text(&response(&request)).contains("Expired"));
    }

    #[test]
    fn duplicate_and_unsafe_identity_or_live_source_mismatch_are_rejected() {
        let original=fixture();
        let mut request=original.clone();
        request["item"]["uniqueId"]=json!(9007199254740992_u64);
        assert_eq!(response(&request)["error"],"invalidItemIdentity");
        for (key,value) in [("unique_id",json!(2)),("count",json!(2)),("item_index",json!(23))] {
            request=original.clone(); request["item"]["tooltipSource"]["userItem"][key]=value;
            assert_eq!(response(&request)["error"],"itemIdentityMismatch");
        }
        request=original.clone(); request["item"]["count"]=json!(0);
        assert_eq!(response(&request)["error"],"invalidItemIdentity");
        request=original.clone(); request["item"]["tooltipSource"]["userItem"]["slots"]=json!([user(0,900)]);
        assert_eq!(response(&request)["error"],"duplicateItemIdentity");
        let input=original.to_string().replacen("\"uniqueId\":0","\"uniqueId\":0,\"uniqueId\":1",1);
        let result:Value=serde_json::from_str(&item_tooltip_document(&input,NOW)).unwrap();
        assert_eq!(result["error"],"invalidJson");
    }

    #[test]
    fn physical_socket_holes_original_names_and_resolved_stats_remain_distinct() {
        let mut request=fixture();
        request["item"]["tooltipSource"]["userItem"]["slots"]=json!([null,user(2,900)]);
        let mut base=info(900,"Ruby[Warrior]1"); base["class_based"]=json!(true); base["stats"]=json!([{"stat":5,"value":2}]);
        let mut real=info(901,"ChangedName"); real["stats"]=json!([{"stat":5,"value":7}]);
        request["item"]["tooltipSource"]["socketInfos"]=json!([null,base]);
        request["item"]["tooltipSource"]["realSocketInfos"]=json!([null,real]);
        let result=response(&request);
        assert_eq!(result["document"]["sourceComplete"],true);
        assert!(text(&result).contains("DC + 2~13 (+9)"));
        assert!(text(&result).contains("Socket : Ruby"));
        assert!(!text(&result).contains("ChangedName"));
        request["item"]["tooltipSource"]["realSocketInfos"]=json!([null,null]);
        let result=response(&request);
        assert_eq!(result["document"]["sourceComplete"],false);
        assert!(result["partialReasons"].as_array().unwrap().contains(&json!("missingRealSocketInfo")));
        assert!(text(&result).contains("DC + 2~8 (+4)"));
        request["item"]["tooltipSource"]["socketInfos"][0]=info(3,"Fake");
        assert_eq!(response(&request)["error"],"socketIdentityMismatch");
    }

    #[test]
    fn missing_dynamic_real_info_uses_original_info_and_catalogue_modes_stay_heavy() {
        let mut request=fixture(); request["item"]["tooltipSource"]["info"]["class_based"]=json!(true);
        let result=response(&request);
        assert_eq!(result["document"]["sourceComplete"],false);
        assert_eq!(result["partialReasons"],json!(["missingRealInfo"]));
        assert!(text(&result).contains("DC + 2~5 (+1)"));
        for kind in ["catalogInstance","catalogPreview","unknown"] {
            request["item"]["sourceKind"]=json!(kind);
            assert_eq!(response(&request)["error"],"unsupportedSourceKind");
        }
        request["item"]["sourceKind"]=json!("instance"); assert_eq!(response(&request)["ok"],true);
    }

    #[test]
    fn aggregate_stat_budget_and_existing_json_limits_are_enforced() {
        let mut request=fixture();
        request["player"]["crystalStats"]=json!([{"stat":5,"value":MAX_STAT_BUDGET}]);
        assert_eq!(response(&request)["error"],"statBudgetExceeded");
        request=fixture(); request["item"]["tooltipSource"]["userItem"]["slots"]=json!(vec![Value::Null;33]);
        assert_eq!(response(&request)["error"],"tooManySockets");
        request=fixture(); request["player"]["crystalStats"]=json!(vec![json!({"stat":1,"value":0});257]);
        assert_eq!(response(&request)["error"],"invalidJson");
        request=fixture(); request["item"]["legacy"]=json!({"description":"x".repeat(8193)});
        assert_eq!(response(&request)["error"],"invalidString");
        let oversized=" ".repeat(MAX_JSON_BYTES+1);
        let result:Value=serde_json::from_str(&item_tooltip_document(&oversized,NOW)).unwrap(); assert_eq!(result["error"],"invalidJsonSize");
        request=fixture(); let mut nested=json!(0); for _ in 0..17 { nested=json!({"nested":nested}); } request["player"]["extra"]=nested;
        assert_eq!(response(&request)["error"],"jsonLimitExceeded");
    }
    #[test]
    fn rental_and_sealed_dates_use_exact_strings_and_required_presence() {
        let mut request=fixture();
        request["item"]["tooltipSource"]["userItem"]["sealed_info"]=json!({"expiry_binary_datetime":"621355968000000001","next_seal_binary_datetime":"-9223372036854775808"});
        request["item"]["tooltipSource"]["userItem"]["rental_information"]=json!({"owner_name":"Owner","binding_flags":-32768,"expiry_binary_datetime":"621355968000000001","rental_locked":false});
        let result=response(&request);
        assert_eq!(result["document"]["sourceComplete"],true);
        assert!(text(&result).contains("Sealed for 0s"));
        assert!(text(&result).contains("Item rented from: Owner"));
        assert!(text(&result).contains("Rental expires in: 0s"));
        request["item"]["tooltipSource"]["userItem"]["sealed_info"]["expiry_binary_datetime"]=json!("621355968000000000");
        request["item"]["tooltipSource"]["userItem"]["rental_information"]["expiry_binary_datetime"]=json!("621355968000000000");
        let result=response(&request);
        assert!(!text(&result).contains("Sealed for"));
        assert!(text(&result).contains("Rental expired"));
        request["item"]["tooltipSource"]["userItem"]["rental_information"]["rental_locked"]=json!(true);
        let result=response(&request);
        assert!(!text(&result).contains("Item rented from"));
        assert!(!text(&result).contains("Rental lock expires"));
        request["item"]["tooltipSource"]["userItem"]["sealed_info"].as_object_mut().unwrap().remove("next_seal_binary_datetime");
        assert_eq!(response(&request)["document"]["sourceComplete"],false);
        request=fixture();
        request["item"]["tooltipSource"]["userItem"]["rental_information"]=json!({"owner_name":"Owner","binding_flags":0,"expiry_binary_datetime":621355968000000001_i64,"rental_locked":false});
        assert_eq!(response(&request)["error"],"unsafeBinaryDateTime");
    }

    #[test]
    fn abi_stacked_instance_describes_more_actions_and_preserves_count_unit_price_and_added_stats() {
        let mut request=fixture();
        request["item"]["count"]=json!(5);
        request["item"]["tooltipSource"]["userItem"]["count"]=json!(5);
        request["item"]["tooltipSource"]["info"]["stack_size"]=json!(20);
        let before=request.clone();
        let result=response(&request);
        assert_eq!(result["ok"],true);
        assert_eq!(result["document"]["sourceComplete"],true);
        assert_eq!(result["partialReasons"],json!([]));
        assert_eq!(text(&result),"木剑 (5)\nCommon\nWeapon\nW: 20  Durability: 3/4\nDC + 2~5 (+1)\nRequired Level : 5\nSelling Price : 125 Gold\nMax Combine Count : 20\nUse More actions to split the stack");
        assert!(!text(&result).contains("Shift + Left click"));
        assert_eq!(result["document"]["sections"][3]["kind"],"overlap");
        assert_eq!(request,before,"presentation cannot rewrite quantity or independent unit price");
        assert_eq!(request["item"]["count"],5);
        assert_eq!(request["item"]["legacy"]["sellValue"],25);
    }

    #[test]
    fn manual_item_decoder_matches_typed_oracle_for_fields_nulls_and_numeric_boundaries() {
        let base = json!({"uniqueId":0,"itemIndex":-23,"name":"木剑🗡","icon":0,"count":1});
        let mut cases = vec![base.clone(), fixture()["item"].clone()];
        for (field, value) in [
            ("uniqueId",json!(u64::MAX)), ("uniqueId",json!(MAX_SAFE_JS_INTEGER)),
            ("itemIndex",json!(i32::MIN)), ("itemIndex",json!(i32::MAX)),
            ("icon",json!(u16::MAX)), ("count",json!(u16::MAX)),
            ("count",json!(0)), ("name",json!("")),
            ("sourceKind",Value::Null), ("sourceKind",json!("instance")),
            ("sourceKind",json!("catalogPreview")),
            ("tooltipSource",Value::Null), ("tooltipSource",json!([null,{"x":1}])),
            ("legacy",Value::Null), ("legacy",json!(false)),
        ] {
            let mut item = base.clone(); item[field] = value; cases.push(item);
        }
        for field in ["uniqueId","itemIndex","name","icon","count"] {
            let mut value = base.clone(); value.as_object_mut().unwrap().remove(field); cases.push(value);
            let mut value = base.clone(); value[field] = Value::Null; cases.push(value);
        }
        for (field, invalid) in [
            ("uniqueId",json!(-1)), ("uniqueId",json!(1.0)), ("uniqueId",json!("0")),
            ("itemIndex",json!(i64::from(i32::MIN)-1)), ("itemIndex",json!(i64::from(i32::MAX)+1)),
            ("itemIndex",json!(0.0)), ("itemIndex",Value::Null),
            ("icon",json!(65536)), ("icon",json!(-1)), ("icon",json!(0.0)),
            ("count",json!(65536)), ("count",json!(false)), ("count",Value::Null),
            ("name",Value::Null), ("name",json!(1)), ("sourceKind",json!(false)),
            ("extra",json!(0)),
        ] {
            let mut value = base.clone(); value[field] = invalid; cases.push(value);
        }
        let positional = json!([0,-23,"木剑🗡",0,1,null,null,null]);
        for length in 5..=8 {
            let mut value = positional.clone(); value.as_array_mut().unwrap().truncate(length); cases.push(value);
        }
        cases.push(json!([0,-23,"木剑🗡",0,1,"instance",{"raw":true},{"sellValue":25}]));
        for length in 0..5 {
            let mut value = positional.clone(); value.as_array_mut().unwrap().truncate(length); cases.push(value);
        }
        let mut extra = positional.clone(); extra.as_array_mut().unwrap().push(Value::Null); cases.push(extra);
        for (index, invalid) in [(0,json!(-1)),(0,json!(1.0)),(1,json!(2147483648_i64)),
            (1,json!(-2147483649_i64)),(1,json!(0.0)),(2,json!(true)),
            (3,json!(65536)),(3,json!(0.0)),(4,json!(65536)),(5,json!(true))] {
            let mut value = positional.clone(); value[index] = invalid; cases.push(value);
        }
        for index in 0..5 {
            let mut value = positional.clone(); value[index] = Value::Null; cases.push(value);
        }
        cases.extend([Value::Null, json!([]), json!(false)]);
        for value in cases {
            let old = serde_json::from_value::<QueryItem>(value.clone());
            let new = parse_query_item(value.clone());
            match (old, new) {
                (Ok(old), Ok(new)) => assert_eq!(new, old, "item: {value}"),
                (Err(_), Err(error)) => assert_eq!(error, "invalidItem", "item: {value}"),
                (old, new) => panic!("decoder disagreement for {value}: old={old:?}, new={new:?}"),
            }
        }
    }

    #[test]
    fn manual_item_decoder_keeps_strict_json_and_abi_error_precedence_against_typed_oracle() {
        let raw = r#"{"uniqueId":0,"itemIndex":-23,"name":"木剑🗡","icon":0,"count":1}"#;
        let strict_decode = |input: &str| -> Result<QueryItem, &'static str> {
            let StrictValue(value) = serde_json::from_str(input).map_err(|_| "invalidJson")?;
            parse_query_item(value)
        };
        for input in [
            r#"[0,-23,"木剑🗡",0,1]"#.to_owned(),
            r#"[0,-23,"木剑🗡",0,1,null]"#.to_owned(),
            r#"[0,-23,"木剑🗡",0,1,null,null]"#.to_owned(),
            r#"[0,-23,"木剑🗡",0,1,null,null,null]"#.to_owned(),
            r#"[0,-23,"木剑🗡",0]"#.to_owned(),
            r#"[0,-23,"木剑🗡",0,1,null,null,null,null]"#.to_owned(),
            r#"[0,-0,"木剑🗡",0,1]"#.to_owned(),
            r#"[0,-23,"木剑🗡",0,1.0]"#.to_owned(),
            r#"[0,-23,"木剑🗡",0,1,true]"#.to_owned(),
            raw.to_owned(),
            raw.replace("\"uniqueId\":0", "\"uniqueId\":9007199254740991"),
            raw.replace("\"name\":\"木剑🗡\"", "\"name\":\"\\u6728\\u5251\""),
            raw.replace("\"uniqueId\":0", "\"uniqueId\":0,\"uniqueId\":1"),
            raw.replace("\"count\":1", "\"count\":1.0"),
            raw.replace("\"itemIndex\":-23", "\"itemIndex\":-0"),
            raw.replace("\"count\":1", "\"count\":1,\"sourceKind\":null,\"sourceKind\":\"instance\""),
            raw.replace("\"count\":1", "\"count\":1,\"unknown\":false"),
        ] {
            let old = serde_json::from_str::<QueryItem>(&input);
            let new = strict_decode(&input);
            assert_eq!(old.is_ok(), new.is_ok(), "raw item: {input}");
            if let (Ok(old), Ok(new)) = (old, new) { assert_eq!(old, new); }
        }
        // Reconstruct the old typed item's object form only inside this oracle.
        // Omitting None keeps the object's special sourceKind-null rule separate
        // from the old array Option semantics, then compare the complete ABI JSON.
        let typed_item_abi = |item: Value| -> Value {
            let Ok(old) = serde_json::from_value::<QueryItem>(item) else {
                return json!({"version":1,"ok":false,"error":"invalidItem"});
            };
            let mut canonical = json!({"uniqueId":old.unique_id,"itemIndex":old.item_index,
                "name":old.name,"icon":old.icon,"count":old.count});
            if let Some(value) = old.source_kind { canonical["sourceKind"] = json!(value); }
            if let Some(value) = old.tooltip_source { canonical["tooltipSource"] = value; }
            if let Some(value) = old.legacy { canonical["legacy"] = value; }
            let mut request = fixture(); request["item"] = canonical; response(&request)
        };
        let original = fixture();
        let full = json!([0,-23,"Wooden Sword",0,1,null,
            original["item"]["tooltipSource"].clone(),original["item"]["legacy"].clone()]);
        let mut arrays = Vec::new();
        for length in 0..=8 {
            let mut item = full.clone(); item.as_array_mut().unwrap().truncate(length); arrays.push(item);
        }
        let mut extra = full.clone(); extra.as_array_mut().unwrap().push(Value::Null); arrays.push(extra);
        for (index, value) in [(0,json!(MAX_SAFE_JS_INTEGER+1)),(0,json!(1.0)),
            (1,json!(i64::from(i32::MIN)-1)),(2,Value::Null),(3,json!(65536)),
            (4,json!(0)),(4,Value::Null),(5,json!("instance")),
            (5,json!("catalogPreview")),(5,json!(true)),(6,Value::Null),(7,json!(false))] {
            let mut item = full.clone(); item[index] = value; arrays.push(item);
        }
        for item in arrays {
            let expected = typed_item_abi(item.clone());
            let mut request = fixture(); request["item"] = item.clone();
            assert_eq!(response(&request), expected, "positional item ABI: {item}");
        }
        let mut request = fixture(); request["item"] = full;
        assert_eq!(response(&request)["ok"], true, "positional null sourceKind is None");
        assert_eq!(response(&request)["document"]["sourceComplete"], true);
        let mut request = fixture();
        request["item"]["sourceKind"] = Value::Null;
        request["item"]["unknown"] = json!(0);
        assert_eq!(response(&request)["error"], "invalidSourceKind");
        request["version"] = json!(2);
        assert_eq!(response(&request)["error"], "unsupportedVersion");
        request = fixture(); request["item"]["unknown"] = json!(0);
        assert_eq!(response(&request)["error"], "invalidItem");
        request = fixture(); request["item"]["uniqueId"] = json!(MAX_SAFE_JS_INTEGER+1);
        assert_eq!(response(&request)["error"], "invalidItemIdentity");
        request = fixture(); request["item"]["name"] = json!("木".repeat(171));
        assert_eq!(response(&request)["error"], "invalidItemIdentity");
        request = fixture(); request["item"]["name"] = json!("木".repeat(170));
        assert_eq!(response(&request)["ok"], true);
        let duplicate = request.to_string().replacen("\"uniqueId\":0", "\"uniqueId\":0,\"uniqueId\":1", 1);
        let result: Value = serde_json::from_str(&item_tooltip_document(&duplicate, NOW)).unwrap();
        assert_eq!(result["error"], "invalidJson");
    }

}
