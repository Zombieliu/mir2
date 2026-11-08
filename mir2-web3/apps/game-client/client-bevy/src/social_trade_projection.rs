//! Read-only projection of Candidate's own reserved trade offer. Crystal's
//! TradeGold/TradeItem packets describe the guest; they never acknowledge ours.
//! No wallet, inventory, reservation, settlement or wire contract is changed.

use std::collections::{BTreeMap, BTreeSet};

use crate::inventory::InventoryModel;
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
    let mut slots = vec![None; MAX_TRADE_ITEMS];
    let mut used_bag_slots = BTreeSet::new();
    let mut used_unique_ids = BTreeSet::new();
    for (&trade_slot, &bag_slot) in &offer.offered_slots {
        let unique_id = offer.offered_unique_ids[&trade_slot];
        if usize::from(trade_slot) >= MAX_TRADE_ITEMS
            || bag_slot >= 80
            || unique_id == 0
            || !used_bag_slots.insert(bag_slot)
            || !used_unique_ids.insert(unique_id)
        {
            return false;
        }
        let mut candidates = inventory
            .items
            .iter()
            .filter(|item| item.container == 0 && u32::from(item.slot) == u32::from(bag_slot));
        let Some(item) = candidates.next() else {
            return false;
        };
        if candidates.next().is_some() || item.unique_id != Some(unique_id) {
            return false;
        }
        let Ok(count) = u16::try_from(item.quantity) else {
            return false;
        };
        if item.tooltip_source.as_ref().is_some_and(|source| {
            source.user_item.as_ref().is_some_and(|user| {
                user.unique_id != unique_id || user.item_index != source.info.item_index
            })
        }) {
            return false;
        }
        slots[usize::from(trade_slot)] = Some(TradeItemModel {
            unique_id: Some(unique_id),
            item_index: item
                .tooltip_source
                .as_ref()
                .map(|source| source.info.item_index),
            name: (!item.name.is_empty()).then(|| item.name.clone()),
            count,
            tooltip_source: item.tooltip_source.clone(),
        });
    }
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
