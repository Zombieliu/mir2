//! Pure ordinary NPC gold-buy quote and command planner.
//!
//! This is a local admission plan, not a purchase acknowledgement. Catalog
//! count is the advertised carrier count, not a finite-stock claim. The host
//! still owns service/session identity, final socket entry and result recovery.

use std::collections::HashSet;
use std::fmt;

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Number, Value};

use crate::inventory::{CrystalItemInfoModel, CrystalUserItemModel, InventoryModel, ItemModel};
use crate::shop::{ShopModel, SHOP_QUANTITY_MAX};

const MAX_INPUT_BYTES: usize = 1024 * 1024;
const MAX_GOODS: usize = 1024;
const MAX_INVENTORY_ROWS: usize = 256;
const MAX_JSON_DEPTH: usize = 32;
const MAX_JSON_SEQUENCE: usize = 2048;
const MAX_JSON_MEMBERS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NpcGoldBuyBlockReason {
    InvalidInput,
    ServiceUnavailable,
    NoSelection,
    InvalidCatalog,
    UnsupportedGoods,
    InvalidSource,
    InvalidRate,
    InvalidPrice,
    InvalidQuantity,
    InvalidInventory,
    InsufficientGold,
    InventoryFull,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NpcGoldBuyCommandType {
    BuyItem,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NpcGoldBuyCommand {
    #[serde(rename = "type")]
    pub command_type: NpcGoldBuyCommandType,
    /// NPCGoods UserItem.unique_id, not the ItemInfo template index.
    pub item_index: u64,
    pub count: u16,
    pub panel_type: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NpcGoldBuyPlan {
    pub max_quantity: u16,
    pub total_gold: Option<u32>,
    pub can_buy: bool,
    pub block_reason: Option<NpcGoldBuyBlockReason>,
    pub command: Option<NpcGoldBuyCommand>,
}

impl NpcGoldBuyPlan {
    fn blocked(reason: NpcGoldBuyBlockReason, max_quantity: u16, total_gold: Option<u32>) -> Self {
        Self { max_quantity, total_gold, can_buy: false, block_reason: Some(reason), command: None }
    }
}

/// Match the actual server order, including u32 saturation and f32 rounding.
fn quote_gold(base_price: u32, quantity: u16, rate: f32) -> u32 {
    ((base_price.saturating_mul(u32::from(quantity)) as f32) * rate).floor() as u32
}

fn legal_belt(info: &CrystalItemInfoModel, slot: u32) -> bool {
    match info.item_type { 13 | 17 => slot < 4, 21 if info.effect == 1 => slot < 4, 8 => (4..6).contains(&slot), _ => false }
}
fn fresh_carrier(item: &ItemModel) -> bool {
    let Some(source) = item.tooltip_source.as_ref() else { return false; };
    let Some(user) = source.user_item.as_ref() else { return false; };
    let info = &source.info;
    if info.item_index < 0 || user.unique_id != item.unique_id.unwrap_or(u64::MAX)
        || u32::from(user.count) != item.quantity || user.count == 0
        || user.count > info.stack_size.max(1) || user.item_index != info.item_index { return false; }
    let slots = match info.item_type {
        19 if info.shape < 7 => 4, 19 if info.shape < 12 => 5,
        1 if matches!(info.shape, 49 | 50) => 5, _ => usize::from(info.slots),
    };
    let canonical = CrystalUserItemModel {
        unique_id: user.unique_id, item_index: info.item_index, count: user.count,
        current_dura: info.durability, max_dura: info.durability,
        identified: false, slots: vec![None; slots], ..Default::default()
    };
    *user == canonical && (item.container == 0 || item.container == 1 && legal_belt(info, item.slot))
}

/// Shared NPC-only structural admission. Other inventory surfaces retain their own rules.
/// Unlisted rows are only occupied cells; the evidence never classifies them as legacy.
pub fn valid_npc_gold_buy_inventory(inventory: &InventoryModel) -> bool {
    if InventoryModel::canonical_capacity(inventory.capacity) != inventory.capacity
        || inventory.items.len() > MAX_INVENTORY_ROWS { return false; }
    let evidence = inventory.npc_gold_trade_capacity.as_ref();
    if evidence.is_some_and(|e| !e.valid() || !e.roster_valid) { return false; }
    let mut ids = HashSet::new();
    let mut grid_ids = HashSet::new();
    let mut cells = HashSet::new();
    if !inventory.items.iter().all(|item| {
        let Some(id) = item.unique_id else { return false; };
        let valid_cell = match item.container {
            0 => item.slot < u32::from(inventory.bag_slot_capacity()),
            1 => item.slot < 6, 2 => item.slot < 14, 3 => item.slot < 40, _ => false,
        };
        valid_cell && (1..=u32::from(u16::MAX)).contains(&item.quantity)
            && cells.insert((item.container, item.slot)) && grid_ids.insert((item.container, id))
            && (evidence.is_some() || ids.insert(id))
    }) { return false; }
    evidence.is_none_or(|e| e.fresh_compatible_unique_ids.iter().all(|id| {
        let mut rows = inventory.items.iter().filter(|item| item.unique_id == Some(*id));
        let Some(row) = rows.next() else { return false; };
        rows.next().is_none() && fresh_carrier(row)
    }))
}

fn has_empty_purchase_cell(inventory: &InventoryModel, info: &CrystalItemInfoModel) -> bool {
    let empty = |container: u8, slot: u32| {
        !inventory.items.iter().any(|item| item.container == container && item.slot == slot)
    };
    // Missing evidence grants no stack slack: only a real admissible empty cell.
    if (0..u32::from(inventory.bag_slot_capacity())).any(|slot| empty(0, slot)) {
        return true;
    }
    let range = match info.item_type {
        13 | 17 => 0..4,
        21 if info.effect == 1 => 0..4,
        8 => 4..6,
        _ => 0..0,
    };
    range.into_iter().any(|slot| empty(1, slot))
}

pub fn plan_npc_gold_buy(shop: &ShopModel, inventory: &InventoryModel, quantity: u16) -> NpcGoldBuyPlan {
    use NpcGoldBuyBlockReason as Reason;
    let blocked = |reason| NpcGoldBuyPlan::blocked(reason, 0, None);
    if !shop.allows_buy() { return blocked(Reason::ServiceUnavailable); }
    let Some(selected_id) = shop.selected_id else { return blocked(Reason::NoSelection); };
    let mut ids = HashSet::new();
    if shop.goods.len() > MAX_GOODS || shop.goods.iter().any(|good| !ids.insert(good.unique_id)) {
        return blocked(Reason::InvalidCatalog);
    }
    let Some(good) = shop.goods.iter().find(|good| good.unique_id == selected_id) else {
        return blocked(Reason::InvalidCatalog);
    };
    if good.use_pearls || good.panel_type != 0 || good.stock >= 0 {
        return blocked(Reason::UnsupportedGoods);
    }
    let Some(source) = good.tooltip_source.as_ref() else { return blocked(Reason::InvalidSource); };
    let Some(user) = source.user_item.as_ref() else { return blocked(Reason::InvalidSource); };
    if !user.is_shop_item { return blocked(Reason::UnsupportedGoods); }
    if user.unique_id != good.unique_id || user.item_index != source.info.item_index
        || source.info.item_index < 0 || user.count == 0 || user.count != good.count
        || user.current_dura > user.max_dura {
        return blocked(Reason::InvalidSource);
    }
    let Some(rate) = good.purchase_rate.filter(|rate| rate.is_finite() && *rate >= 0.0) else {
        return blocked(Reason::InvalidRate);
    };
    let max_quantity = source.info.stack_size.max(1).min(SHOP_QUANTITY_MAX);
    if good.price != quote_gold(source.info.price, 1, rate) {
        return NpcGoldBuyPlan::blocked(Reason::InvalidPrice, max_quantity, None);
    }
    if !(1..=max_quantity).contains(&quantity) {
        return NpcGoldBuyPlan::blocked(Reason::InvalidQuantity, max_quantity, None);
    }
    let total = quote_gold(source.info.price, quantity, rate);
    if !valid_npc_gold_buy_inventory(inventory) {
        return NpcGoldBuyPlan::blocked(Reason::InvalidInventory, max_quantity, Some(total));
    }
    if inventory.gold < total {
        return NpcGoldBuyPlan::blocked(Reason::InsufficientGold, max_quantity, Some(total));
    }
    let slack: u32 = inventory.npc_gold_trade_capacity.as_ref().map_or(0, |e| {
        inventory.items.iter().filter(|item| item.unique_id.is_some_and(|id| e.fresh_compatible_unique_ids.contains(&id)))
            .filter_map(|item| item.tooltip_source.as_ref().filter(|owned| owned.info == source.info)
                .map(|owned| u32::from(owned.info.stack_size.max(1)) - item.quantity)).sum()
    });
    if slack < u32::from(quantity) && !has_empty_purchase_cell(inventory, &source.info) {
        return NpcGoldBuyPlan::blocked(Reason::InventoryFull, max_quantity, Some(total));
    }
    NpcGoldBuyPlan {
        max_quantity, total_gold: Some(total), can_buy: true, block_reason: None,
        command: Some(NpcGoldBuyCommand {
            command_type: NpcGoldBuyCommandType::BuyItem,
            item_index: user.unique_id, count: quantity, panel_type: 0,
        }),
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NpcGoldBuyRequest {
    shop: ShopModel,
    inventory: InventoryModel,
    quantity: u16,
}

// The read models retain their existing wire field names. Presence checks
// precede their legacy serde defaults, which must not manufacture authority.
fn required(value: &Value, keys: &[&str]) -> bool {
    value.as_object().is_some_and(|object| keys.iter().all(|key| object.contains_key(*key)))
}

fn full_info(value: &Value) -> bool {
    required(value, &[
        "item_index", "name", "item_type", "grade", "required_type", "required_class",
        "required_gender", "item_set", "shape", "weight", "light", "required_amount", "image",
        "durability", "stack_size", "price", "start_item", "effect", "need_identify",
        "show_group_pickup", "class_based", "level_based", "can_mine", "global_drop_notify",
        "bind", "unique", "random_stats_id", "can_fast_run", "can_awakening", "slots", "stats", "tooltip",
    ]) && value["stats"].as_array().is_some_and(|stats| stats.iter().all(|stat| required(stat, &["stat", "value"])))
}

fn optional_info(value: &Value) -> bool { value.is_null() || full_info(value) }

fn full_user(value: &Value) -> bool {
    required(value, &[
        "unique_id", "item_index", "current_dura", "max_dura", "count", "soul_bound_id",
        "identified", "cursed", "slots", "gem_count", "added_stats", "awake_type", "awake_values",
        "refined_value", "refine_added", "refine_success_chance", "wedding_ring", "expire_info",
        "rental_information", "is_shop_item", "sealed_info", "gm_made",
    ])
        && value["slots"].as_array().is_some_and(|slots| slots.iter().all(|slot| slot.is_null() || full_user(slot)))
        && value["added_stats"].as_array().is_some_and(|stats| stats.iter().all(|stat| required(stat, &["stat", "value"])))
        && (value["expire_info"].is_null() || (required(&value["expire_info"], &["expiry_binary_datetime"])
            && serde_json::from_value::<mir2_protocol::UserItemExpireInfo>(value["expire_info"].clone()).is_ok()))
        && (value["rental_information"].is_null() || required(&value["rental_information"], &[
            "owner_name", "binding_flags", "expiry_binary_datetime", "rental_locked",
        ]))
        && (value["sealed_info"].is_null() || required(&value["sealed_info"], &["expiry_binary_datetime", "next_seal_binary_datetime"]))
}

fn full_source(value: &Value) -> bool {
    required(value, &["info", "realInfo", "userItem", "socketInfos", "realSocketInfos"])
        && full_info(&value["info"]) && optional_info(&value["realInfo"])
        && (value["userItem"].is_null() || full_user(&value["userItem"]))
        && ["socketInfos", "realSocketInfos"].iter().all(|key| {
            value[*key].as_array().is_some_and(|infos| infos.iter().all(optional_info))
        })
}

/// Check every raw carrier member before a tolerant legacy model is decoded.
/// Identity and ordinary-buy admission are separately checked by the planner.
pub fn full_npc_gold_user_item(value: &Value) -> bool { full_user(value) }

/// An ordinary-buy source must carry complete Info and a complete UserItem.
/// This presence check never fills legacy defaults or supplies market identity.
pub fn full_npc_gold_tooltip_source(value: &Value) -> bool {
    full_source(value) && full_npc_gold_user_item(&value["userItem"])
}

/// Raw presence gate shared by the strict planner and Native snapshot projection.
pub fn full_npc_gold_buy_inventory(value: &Value) -> bool {
    if !required(value, &["capacity", "gold", "items"]) { return false; }
    let Some(items) = value["items"].as_array() else { return false; };
    let evidence = match value.get("npcGoldTradeCapacity") {
        None | Some(Value::Null) => None,
        Some(raw) => match serde_json::from_value::<crate::inventory::NpcGoldTradeCapacity>(raw.clone()) {
            Ok(evidence) => Some(evidence), Err(_) => return false,
        },
    };
    items.len() <= MAX_INVENTORY_ROWS && items.iter().all(|item| {
        required(item, &["uniqueId", "key", "name", "quantity", "slot", "container"])
            && item.get("tooltipSource").is_none_or(|source| source.is_null() || full_source(source))
            && !evidence.as_ref().is_some_and(|e| item["uniqueId"].as_u64()
                .is_some_and(|id| e.fresh_compatible_unique_ids.contains(&id))
                && !item.get("tooltipSource").is_some_and(full_npc_gold_tooltip_source))
    })
}

fn full_input(value: &Value) -> bool {
    let Some(root) = value.as_object() else { return false; };
    if root.len() != 3 || !required(value, &["shop", "inventory", "quantity"])
        || !required(&value["shop"], &[
            "goods", "selected_id", "hide_added_stats", "selected_bag_slot_for_sell",
            "selected_bag_slot_for_repair", "service_mode", "supports_buy", "supports_sell", "repair_rate",
        ]) || !full_npc_gold_buy_inventory(&value["inventory"]) {
        return false;
    }
    let Some(goods) = value["shop"]["goods"].as_array() else { return false; };
    let Some(items) = value["inventory"]["items"].as_array() else { return false; };
    goods.len() <= MAX_GOODS && items.len() <= MAX_INVENTORY_ROWS
        && goods.iter().all(|good| {
            required(good, &[
                "unique_id", "name", "price", "purchase_rate", "requires_gold_buy_plan", "use_pearls", "count", "stock",
                "panel_type", "icon", "icon_width", "icon_height", "description", "tooltip_source",
            ]) && (good["tooltip_source"].is_null() || full_source(&good["tooltip_source"]))
        })
        && items.iter().all(|item| {
            required(item, &["uniqueId", "key", "name", "quantity", "slot", "container"])
                && item.get("tooltipSource").is_none_or(|source| source.is_null() || full_source(source))
        })
}

struct BoundedJson(usize);

impl<'de> DeserializeSeed<'de> for BoundedJson {
    type Value = Value;
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        if self.0 > MAX_JSON_DEPTH { return Err(de::Error::custom("nested input")); }
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for BoundedJson {
    type Value = Value;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result { formatter.write_str("bounded JSON") }
    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> { Ok(Value::Bool(value)) }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> { Ok(Value::Number(value.into())) }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> { Ok(Value::Number(value.into())) }
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        Number::from_f64(value).map(Value::Number).ok_or_else(|| de::Error::custom("invalid number"))
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> { Ok(Value::String(value.to_owned())) }
    fn visit_string<E: de::Error>(self, value: String) -> Result<Value, E> { Ok(Value::String(value)) }
    fn visit_unit<E: de::Error>(self) -> Result<Value, E> { Ok(Value::Null) }
    fn visit_none<E: de::Error>(self) -> Result<Value, E> { Ok(Value::Null) }
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(BoundedJson(self.0 + 1))? {
            if values.len() >= MAX_JSON_SEQUENCE { return Err(de::Error::custom("long array")); }
            values.push(value);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut object: A) -> Result<Value, A::Error> {
        let mut values = Map::new();
        while let Some(key) = object.next_key::<String>()? {
            if values.len() >= MAX_JSON_MEMBERS || values.contains_key(&key) {
                return Err(de::Error::custom("invalid members"));
            }
            values.insert(key, object.next_value_seed(BoundedJson(self.0 + 1))?);
        }
        Ok(Value::Object(values))
    }
}

/// Strict, bounded data-only entry point. Errors never echo raw caller data.
pub fn plan_npc_gold_buy_json(json: &str) -> String {
    let request = (|| {
        if json.len() > MAX_INPUT_BYTES { return None; }
        let mut deserializer = serde_json::Deserializer::from_str(json);
        let value = BoundedJson(0).deserialize(&mut deserializer).ok()?;
        deserializer.end().ok()?;
        if !full_input(&value) { return None; }
        serde_json::from_value::<NpcGoldBuyRequest>(value).ok()
    })();
    let plan = match request {
        Some(request) => plan_npc_gold_buy(&request.shop, &request.inventory, request.quantity),
        None => NpcGoldBuyPlan::blocked(NpcGoldBuyBlockReason::InvalidInput, 0, None),
    };
    serde_json::to_string(&plan).expect("bounded NPC gold plan")
}

#[cfg(test)]
#[path = "npc_shop_buy_tests.rs"]
mod tests;
