//! Read-only projection of Candidate's own trade offer. Crystal's
//! TradeGold/TradeItem packets describe the guest; they never acknowledge ours.
//! No wallet, inventory, reservation, settlement or wire contract is changed.

use std::collections::{BTreeMap, BTreeSet};

use crate::inventory::{CrystalItemTooltipSourceModel, InventoryModel};
use crate::social::{SocialAuthoritativeEvent, SocialModel, TradeItemModel, MAX_TRADE_ITEMS};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OwnOffer {
    settlement_nonce: String,
    partner: String,
    offered_slots: BTreeMap<u8, u8>,
    offered_unique_ids: BTreeMap<u8, u64>,
    offered_gold: u32,
    offered_currency: String,
    locked: bool,
    completed: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DetachedOfferItem {
    slot: u8,
    unique_id: u64,
    container: String,
    key: String,
    name: String,
    #[serde(rename = "icon")]
    _icon: u16,
    quantity: u32,
    tooltip_source: Option<CrystalItemTooltipSourceModel>,
}

pub fn observe_own_offer(
    snapshot: &Value,
    inventory_json: &Value,
    social: &mut SocialModel,
) -> bool {
    // An offer also exists before TradeAccept. A snapshot must not implicitly
    // accept an invitation, resurrect a completed exchange or open its windows.
    if social.trade.state != "open" {
        return false;
    }
    let Some(value) = snapshot.get("stage5Systems").and_then(|s| s.get("trade")) else {
        return false;
    };
    let Ok(offer) = serde_json::from_value::<OwnOffer>(value.clone()) else {
        return false;
    };
    let mut used_unique_ids = BTreeSet::new();
    if Some(offer.partner.as_str()) != social.trade.partner.as_deref()
        || offer.settlement_nonce.is_empty()
        || social
            .trade
            .my_offer_nonce
            .as_ref()
            .is_some_and(|nonce| nonce != &offer.settlement_nonce)
        || offer.completed
        || offer.offered_currency != "gold"
        || offer.offered_slots.len() > MAX_TRADE_ITEMS
        || offer
            .offered_slots
            .keys()
            .ne(offer.offered_unique_ids.keys())
        || offer
            .offered_slots
            .iter()
            .any(|(&slot, &origin)| usize::from(slot) >= MAX_TRADE_ITEMS || origin >= 80)
        || offer
            .offered_unique_ids
            .values()
            .any(|&id| !used_unique_ids.insert(id))
    {
        return false;
    }
    let Ok(inventory) = serde_json::from_value::<InventoryModel>(inventory_json.clone()) else {
        return false;
    };
    if !offer.offered_slots.is_empty()
        && snapshot
            .get("inventoryItems")
            .and_then(Value::as_array)
            .is_none()
    {
        return false;
    }
    let slots = match value.get("ownItems") {
        Some(items) => detached_offer_items(items, &offer, &inventory),
        None => legacy_offer_items(&offer, &inventory),
    };
    let Some(slots) = slots else {
        return false;
    };
    if social.trade.my_offer_nonce.as_deref() == Some(offer.settlement_nonce.as_str())
        && social.trade.my_gold == offer.offered_gold
        && social.trade.my_items == slots
        && social.trade.my_confirmed == offer.locked
    {
        return false;
    }
    social.trade.my_offer_nonce = Some(offer.settlement_nonce);
    social.trade.my_gold = offer.offered_gold;
    social.trade.my_items = slots;
    social.trade.my_confirmed = offer.locked;
    social.trade.event_revision = social.trade.event_revision.wrapping_add(1);
    social.last_event = Some(SocialAuthoritativeEvent {
        packet: "NativeOwnTradeSnapshot".into(),
        success: None,
        subject: Some(offer.partner),
        from: None,
        to: None,
        change_type: None,
        rank_index: None,
        amount: Some(offer.offered_gold),
    });
    true
}

fn detached_offer_items(
    items: &Value,
    offer: &OwnOffer,
    inventory: &InventoryModel,
) -> Option<Vec<Option<TradeItemModel>>> {
    let items = items.as_array()?;
    if items.len() != MAX_TRADE_ITEMS {
        return None;
    }
    items
        .iter()
        .enumerate()
        .map(|(slot, value)| {
            let Some(&origin) = offer.offered_slots.get(&(slot as u8)) else {
                return value.is_null().then_some(None);
            };
            let unique_id = offer.offered_unique_ids[&(slot as u8)];
            let origin_bag = if origin < 40 { "bag1" } else { "bag2" };
            let item: DetachedOfferItem = serde_json::from_value(value.clone()).ok()?;
            // An origin is historical in a detached offer. Its bag cell can
            // already hold another item, but the offered ID cannot be in both.
            if usize::from(item.slot) != slot
                || item.unique_id != unique_id
                || item.container != origin_bag
                || item.key.is_empty()
                || item.name.is_empty()
                || inventory
                    .items
                    .iter()
                    .any(|carried| carried.container == 0 && carried.unique_id == Some(unique_id))
                || !has_exact_detached_tooltip(value, &item)
            {
                return None;
            }
            trade_item(
                unique_id,
                &item.name,
                item.quantity,
                item.tooltip_source.as_ref(),
            )
            .map(Some)
        })
        .collect()
}

fn has_exact_detached_tooltip(value: &Value, item: &DetachedOfferItem) -> bool {
    let Some(source) = item.tooltip_source.as_ref() else {
        return true;
    };
    // The shared tooltip types have defaults for older partial projections.
    // An exact detached quote cannot obtain its identity from those defaults.
    let Some(raw) = value.get("tooltipSource") else {
        return false;
    };
    let info_index = raw
        .get("info")
        .and_then(|info| info.get("item_index"))
        .and_then(Value::as_i64);
    let Some(raw_user) = raw.get("userItem") else {
        return false;
    };
    source.info.item_index >= 0
        && info_index == Some(i64::from(source.info.item_index))
        && raw_user.get("unique_id").and_then(Value::as_u64) == Some(item.unique_id)
        && raw_user.get("item_index").and_then(Value::as_i64) == info_index
        && raw_user.get("count").and_then(Value::as_u64) == Some(u64::from(item.quantity))
}

fn legacy_offer_items(
    offer: &OwnOffer,
    inventory: &InventoryModel,
) -> Option<Vec<Option<TradeItemModel>>> {
    let mut slots = vec![None; MAX_TRADE_ITEMS];
    let mut used_bag_slots = BTreeSet::new();
    for (&trade_slot, &bag_slot) in &offer.offered_slots {
        let unique_id = offer.offered_unique_ids[&trade_slot];
        let bag_slot = u32::from(bag_slot);
        if !used_bag_slots.insert(bag_slot) {
            return None;
        }
        // Crystal encodes slot presence separately, so Some(0) is a valid ID.
        let mut candidates = inventory.items.iter().filter(|item| {
            item.container == 0 && (item.slot == bag_slot || item.unique_id == Some(unique_id))
        });
        let item = candidates.next()?;
        if candidates.next().is_some() || item.slot != bag_slot || item.unique_id != Some(unique_id)
        {
            return None;
        }
        slots[usize::from(trade_slot)] = Some(trade_item(
            unique_id,
            &item.name,
            item.quantity,
            item.tooltip_source.as_ref(),
        )?);
    }
    Some(slots)
}

fn trade_item(
    unique_id: u64,
    name: &str,
    quantity: u32,
    tooltip_source: Option<&CrystalItemTooltipSourceModel>,
) -> Option<TradeItemModel> {
    let count = u16::try_from(quantity).ok()?;
    if count == 0
        || tooltip_source.is_some_and(|source| {
            source.user_item.as_ref().is_some_and(|user| {
                user.unique_id != unique_id || user.item_index != source.info.item_index
            })
        })
    {
        return None;
    }
    Some(TradeItemModel {
        unique_id: Some(unique_id),
        item_index: tooltip_source.map(|source| source.info.item_index),
        name: (!name.is_empty()).then(|| name.to_owned()),
        count,
        tooltip_source: tooltip_source.cloned(),
    })
}

#[cfg(test)]
#[path = "social_trade_projection_tests.rs"]
mod tests;
