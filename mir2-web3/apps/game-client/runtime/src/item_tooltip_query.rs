//! Read-only ABI1 projection of the existing Crystal item-label algorithm.
//!
//! This module owns no world, connection, intent, or Core token. JSON metadata
//! is display input only. Hosts must bind it to their current authoritative
//! item before requesting a document. Missing raw fields never become proof
//! of completeness through the shared models' legacy serde defaults.

use std::{collections::HashSet, fmt};

use mir2_client_bevy::{
    crystal_ui::item_tooltip::{
        crystal_item_tooltip_document, crystal_item_tooltip_document_from_source,
        CrystalItemTooltipColour, CrystalItemTooltipDocument, CrystalItemTooltipSectionKind,
    },
    inventory::{CrystalItemInfoModel, CrystalItemTooltipSourceModel, CrystalUserItemModel, ItemModel},
    read_model::PlayerStats,
};
use mir2_game_data::{crystal_item_manifest_ref, crystal_real_item_for_player, CrystalItemTemplate};
use mir2_protocol::MirClass;
use serde::{de::{self, MapAccess, SeqAccess, Visitor}, Deserialize, Deserializer};
use serde_json::{json, Map, Value};

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

/// Input: `{version:1,item:{uniqueId,itemIndex,name,icon,count,sourceKind?,tooltipSource?},
/// player:{level,...PlayerStats}}`. Nested source fields retain their real
/// snake_case protocol schema; the source envelope is camelCase.
/// sourceKind defaults to instance. catalogInstance accepts only raw userItem;
/// catalogPreview accepts only complete raw info and requires uniqueId zero.
/// Neither catalogue mode grants ownership or mutation authority.
///
/// Output is ABI1 JSON, with either `ok:false,error` or `ok:true,document,
/// viewerComplete,partialReasons`. Documents preserve native nonempty section
/// ordering, line colours, broken state, and item-source completeness.
pub fn query_item_tooltip_json(input: &str) -> String {
    match query(input) {
        Ok(value) => value.to_string(),
        Err(error) => json!({"version":1,"ok":false,"error":error}).to_string(),
    }
}

#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
enum SourceKind {
    #[default]
    Instance,
    CatalogInstance,
    CatalogPreview,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct QueryItem {
    unique_id: u64,
    item_index: i32,
    name: String,
    icon: u16,
    count: u16,
    #[serde(default)]
    source_kind: SourceKind,
    #[serde(default)]
    tooltip_source: Option<Value>,
}

fn query(input: &str) -> Result<Value, &'static str> {
    if input.is_empty() || input.len() > MAX_JSON_BYTES { return Err("invalidJsonSize"); }
    let StrictValue(mut value) = serde_json::from_str(input).map_err(|_| "invalidJson")?;
    bounded_value(&value, 0, &mut 0)?;
    let root = object(&mut value)?;
    exact_keys(root, &["version", "item", "player"])?;
    if root.get("version").and_then(Value::as_u64) != Some(1) { return Err("unsupportedVersion"); }
    let item: QueryItem = serde_json::from_value(root.remove("item").ok_or("missingItem")?)
        .map_err(|_| "invalidItem")?;
    if item.unique_id > MAX_SAFE_JS_INTEGER || item.count == 0
        || item.name.is_empty() || item.name.len() > 512 { return Err("invalidItemIdentity"); }
    match item.source_kind {
        SourceKind::CatalogPreview if item.unique_id != 0 => return Err("invalidPreviewIdentity"),
        _ => {}
    }
    let mut raw_player = root.remove("player").ok_or("missingPlayer")?;
    let (player, viewer_complete) = parse_player(&mut raw_player)?;
    let mut reasons = Vec::new();
    let source = match item.tooltip_source.as_ref() {
        Some(raw) if !raw.is_null() => {
            let mut normalized = match item.source_kind {
                SourceKind::Instance => raw.clone(),
                SourceKind::CatalogInstance => catalog_instance_source(raw, &item, &player, &mut reasons)?,
                SourceKind::CatalogPreview => catalog_preview_source(raw, &item, &player, &mut reasons)?,
            };
            // Generated catalogue metadata obeys the same limits as wire metadata.
            if !matches!(item.source_kind, SourceKind::Instance) {
                bounded_value(&normalized, 0, &mut 0)?;
                if normalized.to_string().len() > MAX_JSON_BYTES { return Err("invalidJsonSize"); }
            }
            validate_source(&mut normalized, &item, &mut reasons)?
        },
        _ => { reason(&mut reasons, "missingSource"); None }
    };
    let mut document = if let Some(source) = source.as_ref() {
        crystal_item_tooltip_document_from_source(&item.name, item.icon,
            u32::from(item.count), Some(source), &player).ok_or("missingDocument")?
    } else {
        // The native legacy path carries only these independently supplied
        // fields; no template/instance/stat fields are filled in for display.
        let user = item.tooltip_source.as_ref().and_then(|source| source.get("userItem"));
        let durability_current = user.and_then(|user| user.get("current_dura"))
            .and_then(Value::as_u64).and_then(|value| u16::try_from(value).ok());
        let durability_max = user.and_then(|user| user.get("max_dura"))
            .and_then(Value::as_u64).and_then(|value| u16::try_from(value).ok());
        crystal_item_tooltip_document(&ItemModel {
            name: item.name, icon: item.icon, quantity: u32::from(item.count),
            durability_current, durability_max,
            ..Default::default()
        }, &player)
    };
    if !document.source_complete && reasons.is_empty() { reason(&mut reasons, "incompleteSource"); }
    if !reasons.is_empty() { document.source_complete = false; }
    Ok(json!({"version":1,"ok":true,"document":document_json(&document),
        "viewerComplete":viewer_complete,"partialReasons":reasons}))
}

/// Read-only ABI1 static catalogue lookup. Only a unique exact item index is
/// returned; null means absent, ambiguous, or unrepresentable metadata.
/// This result is a display template, never an inventory instance or authority.
pub fn query_item_catalog_json(input: &str) -> String {
    match catalog_query(input) {
        Ok(value) => value.to_string(),
        Err(error) => json!({"version":1,"ok":false,"error":error}).to_string(),
    }
}

/// Display metadata only: never a character, operation or custody authority.
/// Required current HP/MP and the stat block cannot be supplied by serde defaults.
pub fn query_character_stats_json(input: &str) -> String {
    let query = || -> Result<Value, &'static str> {
        if input.is_empty() || input.len() > MAX_JSON_BYTES { return Err("invalidJsonSize"); }
        let StrictValue(mut value) = serde_json::from_str(input).map_err(|_| "invalidJson")?;
        bounded_value(&value, 0, &mut 0)?;
        let root = object(&mut value)?;
        exact_keys(root, &["version", "player", "statePage"])?;
        if root.get("version").and_then(Value::as_u64) != Some(1) { return Err("unsupportedVersion"); }
        let state_page = root.get("statePage").and_then(Value::as_bool).ok_or("invalidPage")?;
        let raw = root.remove("player").ok_or("missingPlayer")?;
        let player = raw.as_object().ok_or("invalidPlayer")?;
        for key in ["hp", "mp"] {
            signed(player.get(key).ok_or("incompletePlayer")?, i32::MIN as i64, i32::MAX as i64)?;
        }
        unsigned(player.get("level").ok_or("missingPlayerLevel")?, u32::MAX as u64)?;
        let stats = player.get("crystalStats").and_then(Value::as_array).ok_or("missingPlayerStats")?;
        if stats.len() > 256 { return Err("tooManyPlayerStats"); }
        let mut stat_ids = HashSet::new();
        for stat in stats {
            let stat = stat.as_object().ok_or("invalidPlayerStat")?;
            exact_keys(stat, &["stat", "value"])?;
            let id = unsigned(stat.get("stat").ok_or("invalidPlayerStat")?, u8::MAX as u64)?;
            if !stat_ids.insert(id) { return Err("duplicatePlayerStat"); }
            signed(stat.get("value").ok_or("invalidPlayerStat")?, i32::MIN as i64, i32::MAX as i64)?;
        }
        if let Some(weights) = player.get("weights").filter(|v| !v.is_null()) {
            let weights = weights.as_object().ok_or("invalidPlayerWeights")?;
            exact_keys(weights, &["bag", "wear", "hand"])?;
            for key in ["bag", "wear", "hand"] {
                unsigned(weights.get(key).ok_or("incompletePlayerWeights")?, u32::MAX as u64)?;
            }
        }
        if state_page && ["experience", "maxExperience"].into_iter().any(|key| !player.contains_key(key)) {
            return Err("missingPlayerExperience");
        }
        for key in ["experience", "maxExperience"] {
            if let Some(value) = player.get(key) {
                if value.as_i64().is_none_or(|value| value.unsigned_abs() > MAX_SAFE_JS_INTEGER) {
                    return Err("unsafePlayerInteger");
                }
            }
        }
        // Rows only format i32 values: the tooltip subtotal budget is inapplicable.
        let player: PlayerStats = serde_json::from_value(raw).map_err(|_| "invalidPlayer")?;
        let rows = mir2_client_bevy::crystal_ui::character_stats::authoritative_lines(&player, state_page)
            .into_iter().map(|(text, top)| json!({"text":text,"top":top})).collect::<Vec<_>>();
        Ok(json!({"version":1,"ok":true,"statePage":state_page,"rows":rows}))
    };
    match query() {
        Ok(value) => value.to_string(),
        Err(error) => json!({"version":1,"ok":false,"error":error}).to_string(),
    }
}

fn catalog_query(input: &str) -> Result<Value, &'static str> {
    if input.is_empty() || input.len() > MAX_JSON_BYTES { return Err("invalidJsonSize"); }
    let StrictValue(mut value) = serde_json::from_str(input).map_err(|_| "invalidJson")?;
    bounded_value(&value, 0, &mut 0)?;
    let root = object(&mut value)?;
    exact_keys(root, &["version", "itemIndex"])?;
    if root.get("version").and_then(Value::as_u64) != Some(1) { return Err("unsupportedVersion"); }
    let index = signed(root.get("itemIndex").ok_or("missingItemIndex")?,
        i32::MIN as i64, i32::MAX as i64)? as i32;
    let info = unique_catalog_info(index);
    let item_info = serde_json::to_value(info).map_err(|_| "invalidCatalogInfo")?;
    bounded_value(&item_info, 0, &mut 0)?;
    if !item_info.is_null() {
        if !validate_info(&item_info, &mut 0)? { return Err("incompleteCatalogInfo"); }
        if item_info.to_string().len() > MAX_JSON_BYTES { return Err("invalidJsonSize"); }
    }
    Ok(json!({"version":1,"ok":true,"itemInfo":item_info}))
}

fn unique_catalog_template_in(items: &[CrystalItemTemplate], index: i32)
    -> Option<&CrystalItemTemplate>
{
    let mut matches = items.iter().filter(|item| item.item_index == index);
    let item = matches.next()?;
    if matches.next().is_some() { return None; }
    Some(item)
}

fn unique_catalog_template(index: i32) -> Option<&'static CrystalItemTemplate> {
    unique_catalog_template_in(&crystal_item_manifest_ref().items, index)
}

fn template_info(template: &CrystalItemTemplate) -> Option<CrystalItemInfoModel> {
    serde_json::from_value(serde_json::to_value(template).ok()?).ok()
}

fn unique_catalog_info(index: i32) -> Option<CrystalItemInfoModel> {
    template_info(unique_catalog_template(index)?)
}

// Matches platform-windows gateway's actual tooltip viewer and realInfo
// resolution. Dynamic wire metadata must equal the unique catalogue origin.
fn catalog_viewer(player: &PlayerStats) -> Option<(u16, MirClass)> {
    let level = u16::try_from(player.level).ok()?;
    let class = match player.class_name.as_deref()?.trim().to_ascii_lowercase().as_str() {
        "warrior" => MirClass::Warrior,
        "wizard" => MirClass::Wizard,
        "taoist" => MirClass::Taoist,
        "assassin" => MirClass::Assassin,
        "archer" => MirClass::Archer,
        _ => return None,
    };
    Some((level, class))
}

fn catalog_real_info(info: &CrystalItemInfoModel, viewer: Option<(u16, MirClass)>)
    -> Option<CrystalItemInfoModel>
{
    let (level, class) = viewer?;
    if !info.class_based && !info.level_based { return Some(info.clone()); }
    let origin = unique_catalog_template(info.item_index)?;
    if template_info(origin)? != *info { return None; }
    template_info(&crystal_real_item_for_player(origin, level, class))
}

fn catalog_instance_source(raw: &Value, item: &QueryItem, player: &PlayerStats,
    reasons: &mut Vec<&'static str>) -> Result<Value, &'static str>
{
    let envelope = raw.as_object().ok_or("invalidSource")?;
    exact_keys(envelope, &["userItem"])?;
    let user = envelope.get("userItem").ok_or("missingUserItem")?;
    if user.get("slots").and_then(Value::as_array).is_some_and(|slots| slots.len() > MAX_SOCKETS) {
        return Err("tooManySockets");
    }
    let mut source = Map::new();
    // Keep raw presence and exact binary date strings until validate_source.
    // Serializing a serde-defaulted UserItem here would invent completeness.
    source.insert("userItem".into(), user.clone());
    let viewer = catalog_viewer(player);
    if let Some(info) = unique_catalog_info(item.item_index) {
        source.insert("realInfo".into(), json!(catalog_real_info(&info, viewer)));
        source.insert("info".into(), json!(info));
    } else { reason(reasons, "missingOrAmbiguousCatalogInfo"); }
    let socket_infos = user.get("slots").and_then(Value::as_array).map(|slots| {
        slots.iter().map(|socket| socket.get("item_index").and_then(Value::as_i64)
            .and_then(|index| i32::try_from(index).ok()).and_then(unique_catalog_info))
            .collect::<Vec<_>>()
    }).unwrap_or_default();
    let real_socket_infos = if viewer.is_some() {
        socket_infos.iter().map(|socket| socket.as_ref()
            .and_then(|info| catalog_real_info(info, viewer))).collect::<Vec<_>>()
    } else { Vec::new() };
    source.insert("socketInfos".into(), json!(socket_infos));
    source.insert("realSocketInfos".into(), json!(real_socket_infos));
    Ok(Value::Object(source))
}


/// Crystal new UserItem(info) display previews. UID zero carries no custody.
/// This uses only an unambiguous packaged catalogue entry, never owner defaults.
pub(crate) fn hero_auto_pot_catalog_view(hero: &mir2_client_bevy::hero_model::HeroModel)
    -> mir2_client_bevy::inventory::InventoryModel
{
    let mut model = mir2_client_bevy::inventory::InventoryModel::default();
    let Some(packet) = hero.info.as_ref() else { return model; };
    if !packet.auto_pot { return model; }
    let viewer = (packet.level, packet.class);
    for (slot,index) in [(0,packet.hp_item_index),(1,packet.mp_item_index)] {
        let Some(info) = unique_catalog_info(index) else { continue; };
        if usize::from(info.slots) > MAX_SOCKETS { continue; }
        let user = CrystalUserItemModel {
            unique_id:0,item_index:info.item_index,current_dura:info.durability,
            max_dura:info.durability,count:1,identified:false,
            slots:vec![None;usize::from(info.slots)],..Default::default()
        };
        let source = CrystalItemTooltipSourceModel {
            real_info:catalog_real_info(&info,Some(viewer)),
            info:info.clone(),user_item:Some(user),..Default::default()
        };
        model.items.push(ItemModel {
            key:format!("hero-auto-pot-preview:{slot}:{index}"),unique_id:Some(0),
            name:info.name.clone(),icon:info.image,quantity:1,slot,container:0,
            durability_current:Some(info.durability),durability_max:Some(info.durability),
            tooltip_source:Some(source),..Default::default()
        });
    }
    model
}

fn catalog_preview_source(raw: &Value, item: &QueryItem, player: &PlayerStats,
    reasons: &mut Vec<&'static str>) -> Result<Value, &'static str>
{
    let envelope = raw.as_object().ok_or("invalidSource")?;
    exact_keys(envelope, &["info"])?;
    let raw_info = envelope.get("info").ok_or("missingInfo")?;
    matching_index(raw_info, item.item_index)?;
    // A partial ItemInfo uses the existing legacy document; never complete
    // it via serde defaults or manufacture a preview from missing fields.
    if !validate_info(raw_info, &mut 0)? {
        reason(reasons, "incompletePreviewInfo");
        return Ok(json!({"info":raw_info}));
    }
    let info: CrystalItemInfoModel = serde_json::from_value(raw_info.clone())
        .map_err(|_| "invalidInfo")?;
    if usize::from(info.slots) > MAX_SOCKETS { return Err("tooManySockets"); }
    // Exact Native new UserItem(info) catalogue preview, not a held stack.
    let user = CrystalUserItemModel {
        item_index: info.item_index, current_dura: info.durability,
        max_dura: info.durability, count: item.count, identified: false,
        slots: vec![None; usize::from(info.slots)], ..Default::default()
    };
    Ok(json!({"realInfo":catalog_real_info(&info, catalog_viewer(player)),
        "info":info,"userItem":user,"socketInfos":[],"realSocketInfos":[]}))
}

fn parse_player(raw: &mut Value) -> Result<(PlayerStats, bool), &'static str> {
    let player = object(raw)?;
    let level = player.get("level").ok_or("missingPlayerLevel")?;
    unsigned(level, u32::MAX as u64)?;
    let mut stat_budget = 0;
    if let Some(stats) = player.get("crystalStats").filter(|v| !v.is_null()) {
        stats_array(stats, &mut stat_budget)?;
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
    let parsed = serde_json::from_value(raw.clone()).map_err(|_| "invalidPlayer")?;
    Ok((parsed, known))
}

fn validate_source(raw: &mut Value, item: &QueryItem, reasons: &mut Vec<&'static str>)
    -> Result<Option<CrystalItemTooltipSourceModel>, &'static str>
{
    let source = object(raw)?;
    exact_keys(source, &["info", "realInfo", "userItem", "socketInfos", "realSocketInfos"])?;
    let mut state = SourceValidation::default();
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
    if !raw_complete { return Ok(None); }
    serde_json::from_value(Value::Object(source.clone())).map(Some).map_err(|_| "invalidSource")
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
struct StrictJsonValue<const ARRAY_LIMIT: usize>(Value);
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

/// Shared duplicate-rejecting data parser. Existing tooltip callers retain the
/// original 256-element limits; the learned-skill document admits at most 512.
pub(crate) fn strict_json_value<const ARRAY_LIMIT: usize>(input: &str) -> Result<Value, &'static str> {
    if input.is_empty() || input.len() > MAX_JSON_BYTES { return Err("invalidJsonSize"); }
    let StrictJsonValue(value) = serde_json::from_str::<StrictJsonValue<ARRAY_LIMIT>>(input)
        .map_err(|_| "invalidJson")?;
    bounded_value_with_array_limit::<ARRAY_LIMIT>(&value, 0, &mut 0)?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_client_bevy::inventory::{CrystalItemInfoModel, CrystalItemStatModel, CrystalUserItemModel};

    fn fixture() -> Value {
        let info = CrystalItemInfoModel { item_index: 23, name: "Sword".into(), durability: 10_000,
            item_type: 1, weight: 5, required_amount: 10,
            stats: vec![CrystalItemStatModel { stat: 5, value: 5 }], ..Default::default() };
        let user = CrystalUserItemModel { unique_id: 42, item_index: 23, count: 1,
            current_dura: 8_000, max_dura: 10_000,
            added_stats: vec![CrystalItemStatModel { stat: 5, value: 3 }], ..Default::default() };
        json!({"version":1,"item":{"uniqueId":42,"itemIndex":23,"name":"Sword","icon":0,"count":1,
            "tooltipSource":{"info":info,"userItem":user,"realInfo":null,"socketInfos":[],"realSocketInfos":[]}},
            "player":{"level":5,"className":"Warrior","gender":"Male","crystalStats":[],
                "weights":{"bag":0,"wear":0,"hand":0}}})
    }

    fn response(request: &Value) -> Value {
        serde_json::from_str(&query_item_tooltip_json(&request.to_string())).unwrap()
    }

    fn stats_request(state_page: bool) -> Value {
        json!({"version":1,"statePage":state_page,"player":{
            "level":17,"hp":23,"mp":11,"experience":125,"maxExperience":1000,
            "crystalStats":[{"stat":12,"value":30},{"stat":13,"value":20},
                {"stat":0,"value":2},{"stat":1,"value":7},{"stat":16,"value":80},
                {"stat":18,"value":50},{"stat":17,"value":25}],
            "weights":{"bag":4294967295_u32,"wear":31,"hand":9}}})
    }

    #[test]
    fn item_tooltip_query_character_stats_uses_exact_native_status_and_state_rows() {
        for state_page in [false, true] {
            let request = stats_request(state_page);
            let player: PlayerStats = serde_json::from_value(request["player"].clone()).unwrap();
            let expected = mir2_client_bevy::crystal_ui::character_stats::authoritative_lines(&player, state_page);
            let result: Value = serde_json::from_str(&query_character_stats_json(&request.to_string())).unwrap();
            assert_eq!(result["ok"], true);
            let rows = result["rows"].as_array().unwrap();
            assert_eq!(rows.len(), expected.len());
            for (row, (text, top)) in rows.iter().zip(expected) {
                assert_eq!(row["text"], text);
                assert_eq!(row["top"].as_f64(), Some(f64::from(top)));
            }
            if state_page {
                assert_eq!(rows[0]["text"], "12.5%");
                assert_eq!(rows[1]["text"], "4294967295/80");
            } else {
                assert_eq!(rows[0]["text"], "23/30");
                assert_eq!(rows[2]["text"], "2-7");
            }
        }
    }

    #[test]
    fn item_tooltip_query_character_stats_keeps_missing_weights_unknown() {
        let mut request = stats_request(true);
        request["player"].as_object_mut().unwrap().remove("weights");
        let result: Value = serde_json::from_str(&query_character_stats_json(&request.to_string())).unwrap();
        assert_eq!(result["ok"], true);
        assert!(result["rows"].as_array().unwrap()[1..4].iter().all(|row| row["text"] == ""));
    }

    #[test]
    fn item_tooltip_query_character_stats_rejects_missing_authority_fields() {
        for missing in ["hp", "mp", "level", "crystalStats", "experience", "maxExperience"] {
            let mut request = stats_request(true);
            request["player"].as_object_mut().unwrap().remove(missing);
            let result: Value = serde_json::from_str(&query_character_stats_json(&request.to_string())).unwrap();
            assert_eq!(result["ok"], false, "{missing}");
            assert!(result["rows"].is_null());
        }
    }

    #[test]
    fn item_tooltip_query_character_stats_rejects_unsafe_and_duplicate_metadata() {
        let mut request = stats_request(true);
        request["player"]["experience"] = json!(9_007_199_254_740_992_i64);
        let unsafe_result: Value = serde_json::from_str(&query_character_stats_json(&request.to_string())).unwrap();
        assert_eq!(unsafe_result["ok"], false);
        let mut request = stats_request(false);
        request["player"]["crystalStats"] = json!([{"stat":12,"value":i32::MAX}]);
        let full_range: Value = serde_json::from_str(&query_character_stats_json(&request.to_string())).unwrap();
        assert_eq!(full_range["rows"][0]["text"], "23/2147483647");
        request["player"]["crystalStats"] = json!([{"stat":12,"value":30},{"stat":12,"value":31}]);
        let duplicate_stat: Value = serde_json::from_str(&query_character_stats_json(&request.to_string())).unwrap();
        assert_eq!(duplicate_stat["error"], "duplicatePlayerStat");
        for input in [r#"{"version":1,"version":1,"statePage":false,"player":{}}"#,
            r#"{"version":2,"statePage":false,"player":{}}"#,
            r#"{"version":1,"statePage":"state","player":{}}"#] {
            let result: Value = serde_json::from_str(&query_character_stats_json(input)).unwrap();
            assert_eq!(result["ok"], false);
        }
    }

    #[test]
    fn item_tooltip_query_matches_native_document_and_requirement_colours() {
        let request = fixture();
        let source: CrystalItemTooltipSourceModel = serde_json::from_value(request["item"]["tooltipSource"].clone()).unwrap();
        let player: PlayerStats = serde_json::from_value(request["player"].clone()).unwrap();
        let native = crystal_item_tooltip_document_from_source("Sword", 0, 1, Some(&source), &player).unwrap();
        let result = response(&request);
        assert_eq!(result["ok"], true);
        assert_eq!(result["document"], document_json(&native));
        assert_eq!(result["document"]["sourceComplete"], true);
        assert_eq!(result["viewerComplete"], true);
        assert!(result["document"]["sections"].as_array().unwrap().iter().any(|s|
            s["kind"] == "need" && s["lines"].as_array().unwrap().iter().any(|l| l["colour"] == "red")));
    }

    #[test]
    fn item_tooltip_query_missing_raw_fields_never_uses_serde_defaults_as_complete() {
        let mut request = fixture();
        request["item"]["tooltipSource"]["userItem"].as_object_mut().unwrap().remove("identified");
        let result = response(&request);
        assert_eq!(result["ok"], true);
        assert_eq!(result["document"]["sourceComplete"], false);
        assert_eq!(result["document"]["sections"].as_array().unwrap().len(), 1);
        assert!(result["partialReasons"].as_array().unwrap().contains(&json!("incompleteUserItem")));
        request["item"]["tooltipSource"]["userItem"]["current_dura"] = json!(0);
        assert_eq!(response(&request)["document"]["broken"], true);
        request["player"].as_object_mut().unwrap().remove("level");
        assert_eq!(response(&request)["error"], "missingPlayerLevel");
    }

    #[test]
    fn item_tooltip_query_missing_dynamic_and_viewer_source_stays_partial() {
        let mut request = fixture();
        request["item"]["tooltipSource"]["info"]["class_based"] = json!(true);
        request["player"].as_object_mut().unwrap().remove("crystalStats");
        request["player"].as_object_mut().unwrap().remove("weights");
        request["player"].as_object_mut().unwrap().remove("className");
        request["player"].as_object_mut().unwrap().remove("gender");
        let result = response(&request);
        assert_eq!(result["ok"], true);
        assert_eq!(result["document"]["sourceComplete"], false);
        assert_eq!(result["viewerComplete"], false);
        assert_eq!(result["partialReasons"], json!(["missingRealInfo"]));
    }

    #[test]
    fn item_tooltip_query_preserves_all_native_sections_and_physical_socket_holes() {
        let mut request = fixture();
        let socket_info = CrystalItemInfoModel { item_index: 24, name: "Ruby".into(),
            class_based: true, stats: vec![CrystalItemStatModel { stat: 5, value: 2 }], ..Default::default() };
        let real_socket_info = CrystalItemInfoModel { item_index: 25, name: "Ruby[Warrior]".into(),
            stats: vec![CrystalItemStatModel { stat: 5, value: 4 }], ..Default::default() };
        let socket = CrystalUserItemModel { unique_id: 99, item_index: 24, count: 1,
            current_dura: 1_000, max_dura: 1_000, ..Default::default() };
        let source = &mut request["item"]["tooltipSource"];
        source["userItem"]["slots"] = json!([null, socket]);
        source["socketInfos"] = json!([null, socket_info]);
        source["realSocketInfos"] = json!([null, real_socket_info]);
        source["userItem"]["awake_values"] = json!([1, 2]);
        source["userItem"]["awake_type"] = json!(1);
        source["userItem"]["gm_made"] = json!(true);
        source["info"]["bind"] = json!(16);
        source["info"]["stack_size"] = json!(10);
        source["info"]["tooltip"] = json!("Actual story");
        source["info"]["stats"] = json!([{"stat":5,"value":5},{"stat":1,"value":3},{"stat":16,"value":2}]);
        let native_source: CrystalItemTooltipSourceModel = serde_json::from_value(source.clone()).unwrap();
        let player: PlayerStats = serde_json::from_value(request["player"].clone()).unwrap();
        let native = crystal_item_tooltip_document_from_source("Sword", 0, 1, Some(&native_source), &player).unwrap();
        let result = response(&request);
        assert_eq!(result["document"], document_json(&native));
        assert_eq!(result["document"]["sourceComplete"], true);
        assert_eq!(result["document"]["sections"].as_array().unwrap().iter().map(|s| s["kind"].clone()).collect::<Vec<_>>(),
            vec![json!("name"),json!("attack"),json!("defence"),json!("weight"),json!("awake"),json!("socket"),
                json!("need"),json!("bind"),json!("overlap"),json!("story"),json!("gmMade")]);
        request["item"]["tooltipSource"]["realSocketInfos"][1] = Value::Null;
        let partial = response(&request);
        assert_eq!(partial["document"]["sourceComplete"], false);
        assert_eq!(partial["partialReasons"], json!(["missingRealSocketInfo"]));
        request["item"]["tooltipSource"]["socketInfos"][1]["item_index"] = json!(26);
        assert_eq!(response(&request)["error"], "itemIdentityMismatch");
    }

    #[test]
    fn item_tooltip_query_rejects_unsafe_or_mismatching_instance_identity() {
        let mut request = fixture();
        request["item"]["tooltipSource"]["userItem"]["unique_id"] = json!(MAX_SAFE_JS_INTEGER + 1);
        assert_eq!(response(&request)["ok"], false);
        request["item"]["tooltipSource"]["userItem"]["unique_id"] = json!(43);
        assert_eq!(response(&request)["error"], "itemIdentityMismatch");
        request = fixture();
        request["item"]["tooltipSource"]["userItem"]["count"] = json!(2);
        assert_eq!(response(&request)["error"], "itemIdentityMismatch");
    }

    #[test]
    fn item_tooltip_query_keeps_binary_expiry_exact_without_js_number_conversion() {
        let mut request = fixture();
        let binary = 3_155_378_975_999_999_999_i64 | (1_i64 << 62);
        request["item"]["tooltipSource"]["userItem"]["expire_info"] = json!({"expiry_binary_datetime":binary.to_string()});
        request["item"]["tooltipSource"]["userItem"]["rental_information"] = json!({"owner_name":"Owner",
            "binding_flags":0,"expiry_binary_datetime":binary.to_string(),"rental_locked":false});
        let result = response(&request);
        assert_eq!(result["ok"], true);
        assert_eq!(result["document"]["sourceComplete"], true);
        assert!(result["document"]["sections"].as_array().unwrap().iter().any(|s| s["kind"] == "bind"
            && s["lines"].as_array().unwrap().iter().any(|l| l["text"].as_str().unwrap().starts_with("Expires in "))));
        request["item"]["tooltipSource"]["userItem"]["expire_info"]["expiry_binary_datetime"] = json!(binary);
        assert_eq!(response(&request)["error"], "unsafeBinaryDateTime");
    }

    #[test]
    fn item_tooltip_query_broken_and_unidentified_items_follow_native_hidden_stats() {
        let mut request = fixture();
        request["item"]["tooltipSource"]["info"]["need_identify"] = json!(true);
        request["item"]["tooltipSource"]["userItem"]["identified"] = json!(false);
        request["item"]["tooltipSource"]["userItem"]["current_dura"] = json!(0);
        let result = response(&request);
        let source: CrystalItemTooltipSourceModel = serde_json::from_value(request["item"]["tooltipSource"].clone()).unwrap();
        let player: PlayerStats = serde_json::from_value(request["player"].clone()).unwrap();
        let native = crystal_item_tooltip_document_from_source("Sword", 0, 1, Some(&source), &player).unwrap();
        assert_eq!(result["document"], document_json(&native));
        assert_eq!(result["document"]["broken"], true);
        assert!(!result["document"]["sections"].to_string().contains("(+3)"));
    }

    #[test]
    fn item_tooltip_query_rejects_duplicate_fields_and_bounded_input_overflows() {
        assert_eq!(serde_json::from_str::<Value>(&query_item_tooltip_json("{\"version\":1,\"version\":1}")).unwrap()["ok"], false);
        let mut request = fixture();
        request["item"]["name"] = json!("x".repeat(513));
        assert_eq!(response(&request)["ok"], false);
        request = fixture();
        request["item"]["tooltipSource"]["info"]["stats"] = json!([{"stat":5,"value":i32::MAX}]);
        assert_eq!(response(&request)["error"], "statBudgetExceeded");
    }
    fn catalog_request(index: i32, preview: bool) -> Value {
        let template = unique_catalog_template(index).expect("checked-in unique catalogue fixture");
        let info: CrystalItemInfoModel = serde_json::from_value(serde_json::to_value(template).unwrap()).unwrap();
        let user = CrystalUserItemModel { unique_id: 42, item_index: index, count: 3,
            current_dura: info.durability, max_dura: info.durability, ..Default::default() };
        json!({"version":1,"item":{"uniqueId":if preview {0} else {42},"itemIndex":index,
            "name":info.name,"icon":info.image,"count":3,
            "sourceKind":if preview {"catalogPreview"} else {"catalogInstance"},
            "tooltipSource":if preview {json!({"info":info})} else {json!({"userItem":user})}},
            "player":{"level":40,"className":"Warrior","gender":"Male","crystalStats":[],
                "weights":{"bag":0,"wear":0,"hand":0}}})
    }

    #[test]
    fn catalog_instance_matches_native_dynamic_sockets_dates_and_actual_zero_uid() {
        use mir2_client_bevy::inventory::{CrystalUserItemExpireModel, CrystalUserItemRentalModel};
        let origin = unique_catalog_template(1).unwrap();
        let socket_origin = unique_catalog_template(375).unwrap();
        let info: CrystalItemInfoModel = serde_json::from_value(serde_json::to_value(origin).unwrap()).unwrap();
        let socket_info: CrystalItemInfoModel = serde_json::from_value(serde_json::to_value(socket_origin).unwrap()).unwrap();
        let real_info = serde_json::from_value(serde_json::to_value(
            crystal_real_item_for_player(origin, 40, MirClass::Warrior)).unwrap()).unwrap();
        let real_socket_info = serde_json::from_value(serde_json::to_value(
            crystal_real_item_for_player(socket_origin, 40, MirClass::Warrior)).unwrap()).unwrap();
        let binary = 3_155_378_975_999_999_999_i64 | (1_i64 << 62);
        let socket = CrystalUserItemModel { unique_id: 99, item_index: 375, count: 1,
            current_dura: socket_info.durability, max_dura: socket_info.durability, ..Default::default() };
        let user = CrystalUserItemModel { unique_id: 0, item_index: 1, count: 3,
            current_dura: 0, max_dura: info.durability, slots: vec![None, Some(socket)],
            expire_info: Some(CrystalUserItemExpireModel { expiry_binary_datetime: binary }),
            rental_information: Some(CrystalUserItemRentalModel { owner_name: "Owner".into(),
                binding_flags: 0, expiry_binary_datetime: binary, rental_locked: false }), ..Default::default() };
        let mut request = catalog_request(1, false);
        request["item"]["uniqueId"] = json!(0);
        request["item"]["tooltipSource"]["userItem"] = json!(user);
        request["item"]["tooltipSource"]["userItem"]["rental_information"]["expiry_binary_datetime"] = json!(binary.to_string());
        let native_source = CrystalItemTooltipSourceModel { info, real_info: Some(real_info),
            user_item: Some(user), socket_infos: vec![None, Some(socket_info)],
            real_socket_infos: vec![None, Some(real_socket_info)] };
        let player: PlayerStats = serde_json::from_value(request["player"].clone()).unwrap();
        let native = crystal_item_tooltip_document_from_source(request["item"]["name"].as_str().unwrap(),
            request["item"]["icon"].as_u64().unwrap() as u16, 3, Some(&native_source), &player).unwrap();
        let result = response(&request);
        assert_eq!(result["ok"], true);
        assert_eq!(result["document"], document_json(&native));
        assert_eq!(result["document"]["sourceComplete"], true);
        assert_eq!(result["document"]["broken"], true);
        assert_eq!(result["partialReasons"], json!([]));
        let parsed: QueryItem = serde_json::from_value(request["item"].clone()).unwrap();
        let normalized = catalog_instance_source(&request["item"]["tooltipSource"], &parsed, &player, &mut Vec::new()).unwrap();
        assert_eq!(normalized["socketInfos"][0], Value::Null);
        assert_eq!(normalized["socketInfos"][1]["item_index"], 375);
        assert_eq!(normalized["userItem"]["rental_information"]["expiry_binary_datetime"], binary.to_string());
        request["item"]["uniqueId"] = json!(1);
        assert_eq!(response(&request)["error"], "itemIdentityMismatch");
    }

    #[test]
    fn catalog_preview_matches_native_constructor_and_hidden_stats() {
        let mut request = catalog_request(375, true);
        let info: CrystalItemInfoModel = serde_json::from_value(request["item"]["tooltipSource"]["info"].clone()).unwrap();
        let origin = unique_catalog_template(375).unwrap();
        let real_info = serde_json::from_value(serde_json::to_value(
            crystal_real_item_for_player(origin, 40, MirClass::Warrior)).unwrap()).unwrap();
        let user = CrystalUserItemModel { item_index: info.item_index,
            current_dura: info.durability, max_dura: info.durability, count: 3, identified: false,
            slots: vec![None; usize::from(info.slots)], ..Default::default() };
        let native_source = CrystalItemTooltipSourceModel { info, real_info: Some(real_info),
            user_item: Some(user), socket_infos: Vec::new(), real_socket_infos: Vec::new() };
        let player: PlayerStats = serde_json::from_value(request["player"].clone()).unwrap();
        let native = crystal_item_tooltip_document_from_source(request["item"]["name"].as_str().unwrap(),
            request["item"]["icon"].as_u64().unwrap() as u16, 3, Some(&native_source), &player).unwrap();
        let result = response(&request);
        assert_eq!(result["document"], document_json(&native));
        assert_eq!(result["document"]["sourceComplete"], true);
        let parsed: QueryItem = serde_json::from_value(request["item"].clone()).unwrap();
        let normalized = catalog_preview_source(&request["item"]["tooltipSource"], &parsed, &player, &mut Vec::new()).unwrap();
        assert_eq!(normalized["userItem"]["unique_id"], 0);
        assert_eq!(normalized["userItem"]["identified"], false);
        assert_eq!(normalized["userItem"]["added_stats"], json!([]));
        assert_eq!(normalized["userItem"]["max_dura"], request["item"]["tooltipSource"]["info"]["durability"]);
        assert_eq!(normalized["userItem"]["count"], 3);
        // A complete non-dynamic ItemInfo need not originate in the manifest;
        // Native still applies its real unidentified-item hidden-stat logic.
        let mut hidden_info = fixture()["item"]["tooltipSource"]["info"].clone();
        hidden_info["need_identify"] = json!(true);
        hidden_info["slots"] = json!(2);
        request["item"]["itemIndex"] = json!(23);
        request["item"]["name"] = json!("Sword");
        request["item"]["icon"] = json!(0);
        request["item"]["tooltipSource"]["info"] = hidden_info.clone();
        let hidden_info: CrystalItemInfoModel = serde_json::from_value(hidden_info).unwrap();
        let hidden_user = CrystalUserItemModel { item_index: 23, current_dura: hidden_info.durability,
            max_dura: hidden_info.durability, count: 3, identified: false,
            slots: vec![None, None], ..Default::default() };
        let hidden_source = CrystalItemTooltipSourceModel { real_info: Some(hidden_info.clone()),
            info: hidden_info, user_item: Some(hidden_user), socket_infos: Vec::new(), real_socket_infos: Vec::new() };
        let native = crystal_item_tooltip_document_from_source("Sword", 0, 3, Some(&hidden_source), &player).unwrap();
        assert_eq!(response(&request)["document"], document_json(&native));
        assert!(native.plain_text().contains("DC + 0~5"));
        assert!(!native.plain_text().contains("(+"));
    }

    #[test]
    fn catalogue_sources_missing_raw_or_dynamic_authority_stay_partial() {
        let mut request = catalog_request(1, false);
        request["item"]["tooltipSource"]["userItem"].as_object_mut().unwrap().remove("identified");
        let result = response(&request);
        assert_eq!(result["ok"], true);
        assert_eq!(result["document"]["sourceComplete"], false);
        assert!(result["partialReasons"].as_array().unwrap().contains(&json!("incompleteUserItem")));
        request = catalog_request(1, false);
        request["item"]["itemIndex"] = json!(i32::MIN);
        request["item"]["tooltipSource"]["userItem"]["item_index"] = json!(i32::MIN);
        let result = response(&request);
        assert_eq!(result["ok"], true);
        assert_eq!(result["document"]["sourceComplete"], false);
        assert!(result["partialReasons"].as_array().unwrap().contains(&json!("missingOrAmbiguousCatalogInfo")));
        request = catalog_request(1, true);
        request["item"]["tooltipSource"]["info"].as_object_mut().unwrap().remove("durability");
        assert_eq!(response(&request)["document"]["sourceComplete"], false);
        assert!(response(&request)["partialReasons"].as_array().unwrap().contains(&json!("incompletePreviewInfo")));
        request = catalog_request(1, true);
        request["item"]["tooltipSource"]["info"]["stats"][0]["value"] = json!(123);
        let result = response(&request);
        assert_eq!(result["document"]["sourceComplete"], false);
        assert!(result["partialReasons"].as_array().unwrap().contains(&json!("missingRealInfo")));
        request = catalog_request(1, true);
        request["player"].as_object_mut().unwrap().remove("className");
        assert_eq!(response(&request)["viewerComplete"], false);
        assert_eq!(response(&request)["document"]["sourceComplete"], false);
        request["player"].as_object_mut().unwrap().remove("level");
        assert_eq!(response(&request)["error"], "missingPlayerLevel");
    }

    #[test]
    fn catalogue_modes_reject_cross_kind_metadata_and_unsafe_raw_values() {
        let unchanged = response(&fixture());
        let mut explicit_instance = fixture();
        explicit_instance["item"]["sourceKind"] = json!("instance");
        assert_eq!(response(&explicit_instance), unchanged);
        explicit_instance["item"]["sourceKind"] = json!("preview");
        assert_eq!(response(&explicit_instance)["error"], "invalidItem");
        let mut request = catalog_request(1, true);
        request["item"]["uniqueId"] = json!(42);
        assert_eq!(response(&request)["error"], "invalidPreviewIdentity");
        request = catalog_request(1, true);
        request["item"]["tooltipSource"]["userItem"] = fixture()["item"]["tooltipSource"]["userItem"].clone();
        assert_eq!(response(&request)["error"], "unexpectedField");
        request = catalog_request(1, true);
        request["item"]["tooltipSource"]["info"]["item_index"] = json!(2);
        assert_eq!(response(&request)["error"], "itemIdentityMismatch");
        request = catalog_request(1, true);
        request["item"]["tooltipSource"]["info"]["stats"] = json!([{"stat":5,"value":i32::MAX}]);
        assert_eq!(response(&request)["error"], "statBudgetExceeded");
        request = catalog_request(1, true);
        request["item"]["tooltipSource"]["info"]["slots"] = json!(33);
        assert_eq!(response(&request)["error"], "tooManySockets");
        request = catalog_request(1, false);
        request["item"]["tooltipSource"]["info"] = catalog_request(1, true)["item"]["tooltipSource"]["info"].clone();
        assert_eq!(response(&request)["error"], "unexpectedField");
        request = catalog_request(1, false);
        request["item"]["tooltipSource"]["userItem"]["count"] = json!(2);
        assert_eq!(response(&request)["error"], "itemIdentityMismatch");
        request["item"]["tooltipSource"]["userItem"]["count"] = json!(3);
        request["item"]["tooltipSource"]["userItem"]["unique_id"] = json!(MAX_SAFE_JS_INTEGER + 1);
        assert_eq!(response(&request)["ok"], false);
        request = catalog_request(1, false);
        request["item"]["tooltipSource"]["userItem"]["expire_info"] = json!({"expiry_binary_datetime":i64::MAX});
        assert_eq!(response(&request)["error"], "unsafeBinaryDateTime");
    }

    #[test]
    fn item_catalog_query_returns_only_the_unique_exact_native_template() {
        let origin = unique_catalog_template(1).unwrap();
        let expected: CrystalItemInfoModel = serde_json::from_value(serde_json::to_value(origin).unwrap()).unwrap();
        let result: Value = serde_json::from_str(&query_item_catalog_json(r#"{"version":1,"itemIndex":1}"#)).unwrap();
        assert_eq!(result, json!({"version":1,"ok":true,"itemInfo":expected}));
        let missing: Value = serde_json::from_str(&query_item_catalog_json(
            &json!({"version":1,"itemIndex":i32::MIN}).to_string())).unwrap();
        assert_eq!(missing, json!({"version":1,"ok":true,"itemInfo":null}));
        let duplicate = vec![origin.clone(), origin.clone()];
        assert!(unique_catalog_template_in(&duplicate, 1).is_none());
        assert!(unique_catalog_template_in(&duplicate, i32::MIN).is_none());
        assert_eq!(unique_catalog_template_in(&duplicate[..1], 1), Some(origin));
    }

    #[test]
    fn item_catalog_query_rejects_extra_duplicate_missing_or_non_i32_identity() {
        for invalid in [r#"{"version":1,"itemIndex":1,"userItem":{}}"#,
            r#"{"version":1,"itemIndex":1,"itemIndex":2}"#,
            r#"{"version":1}"#, r#"{"version":2,"itemIndex":1}"#,
            r#"{"version":1,"itemIndex":"1"}"#, r#"{"version":1,"itemIndex":1.5}"#,
            r#"{"version":1,"itemIndex":2147483648}"#, r#"{"version":1,"itemIndex":-2147483649}"#] {
            let result: Value = serde_json::from_str(&query_item_catalog_json(invalid)).unwrap();
            assert_eq!(result["ok"], false, "{invalid}");
        }
        let oversized = " ".repeat(MAX_JSON_BYTES + 1);
        let result: Value = serde_json::from_str(&query_item_catalog_json(&oversized)).unwrap();
        assert_eq!(result["error"], "invalidJsonSize");
    }

}
