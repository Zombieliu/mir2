use crate::config::{ItemContainer, Stage5SocialState};

use super::crystal_compat::CRYSTAL_BIND_DONT_TRADE;
use super::inventory::empty_slots_for_inventory_container;
use super::items::{item_has_crystal_or_rental_bind_flag, ItemState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Stage5SocialAddResult {
    Added,
    EmptyTarget,
    SelfTarget,
    AlreadyAdded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MarketSettlement {
    pub gross: u32,
    pub earnings: u32,
    pub commission: u32,
}

pub(super) fn social_blocks_outgoing_mail(social: &Stage5SocialState, recipient: &str) -> bool {
    let recipient = recipient.trim();
    !recipient.is_empty()
        && social
            .blocked
            .iter()
            .any(|blocked| blocked.eq_ignore_ascii_case(recipient))
}

pub(super) fn stage5_social_add_friend_entry(
    social: &mut Stage5SocialState,
    player_name: &str,
    target_name: &str,
    blocked: bool,
) -> Stage5SocialAddResult {
    let target_name = target_name.trim();
    if target_name.is_empty() {
        return Stage5SocialAddResult::EmptyTarget;
    }
    if target_name.eq_ignore_ascii_case(player_name.trim()) {
        return Stage5SocialAddResult::SelfTarget;
    }
    if social
        .friends
        .iter()
        .chain(social.blocked.iter())
        .any(|name| name.eq_ignore_ascii_case(target_name))
    {
        return Stage5SocialAddResult::AlreadyAdded;
    }

    if blocked {
        social.blocked.push(target_name.to_string());
    } else {
        social.friends.push(target_name.to_string());
    }
    Stage5SocialAddResult::Added
}

pub(super) fn stage5_market_sale_price(list_price: u32, bid_price: u32) -> Option<u32> {
    if bid_price == 0 {
        Some(list_price)
    } else if bid_price >= list_price {
        Some(bid_price)
    } else {
        None
    }
}

pub(super) fn stage5_market_settlement(gross: u32) -> MarketSettlement {
    let commission = gross.div_ceil(20);
    MarketSettlement {
        gross,
        earnings: gross.saturating_sub(commission),
        commission,
    }
}

pub(super) fn stage5_trade_item_can_enter(item: &ItemState) -> bool {
    !item_has_crystal_or_rental_bind_flag(item, CRYSTAL_BIND_DONT_TRADE)
        && item
            .soul_bound_id
            .is_none_or(|soul_bound_id| soul_bound_id < 0)
        && item.rental_binding_flags == 0
        && item.rental_owner_name.trim().is_empty()
        && item.rental_expiry_binary_datetime == 0
        && !item.rental_locked
}

pub(super) fn stage5_mail_exact_item_slots(
    inventory_items: &[ItemState],
    item_states: &[ItemState],
) -> Option<Vec<(ItemContainer, u8)>> {
    let mut reserved_items = inventory_items.to_vec();
    let mut slots = Vec::with_capacity(item_states.len());
    for item in item_states {
        let (container, slot) =
            empty_slots_for_inventory_container(&reserved_items, ItemContainer::Bag1)
                .into_iter()
                .next()?;
        let mut reserved = item.clone();
        reserved.container = container;
        reserved.slot = slot;
        reserved_items.push(reserved);
        slots.push((container, slot));
    }
    Some(slots)
}
