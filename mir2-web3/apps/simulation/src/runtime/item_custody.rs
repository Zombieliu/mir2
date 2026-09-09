//! Exact instance custody shared by ordinary market and refine packets.
//! Key mirrors remain readable for old saves; missing carriers never mint replacements.
use super::inventory::{
    find_empty_inventory_item_slot, inventory_container_and_slot_for_index,
    inventory_unique_id_is_used, is_valid_inventory_slot, item_stack_identity_compatible,
};
use super::items::{
    crystal_stack_size_for_item_key, try_item_state_from_user_item, try_user_item_from_item_state,
    validate_committed_item_state_carrier, ItemState,
};
use super::resources::{InventoryResource, Stage5SystemsResource};
use crate::config::{ItemContainer, Stage5SystemsState};
use bevy_ecs::prelude::World;
use mir2_protocol::{ServerPacket, UserItem};
use std::collections::BTreeSet;

fn wire_ids(item: &UserItem, ids: &mut BTreeSet<u64>) -> Result<(), String> {
    if !ids.insert(item.unique_id) {
        return Err("custody contains duplicate item UID".into());
    }
    for child in item.slots.iter().flatten() {
        wire_ids(child, ids)?;
    }
    Ok(())
}
pub(super) fn item_ids(item: &ItemState) -> Result<BTreeSet<u64>, String> {
    validate_committed_item_state_carrier(item).map_err(|e| e.to_string())?;
    let wire = try_user_item_from_item_state(item).map_err(|e| e.to_string())?;
    let mut ids = BTreeSet::new();
    wire_ids(&wire, &mut ids)?;
    Ok(ids)
}
fn freeze_legacy_zero(item: ItemState) -> Result<ItemState, String> {
    if item.unique_id == 0 && item.user_item_metadata.is_none() {
        let wire = try_user_item_from_item_state(&item).map_err(|e| e.to_string())?;
        return try_item_state_from_user_item(item, &wire).map_err(|e| e.to_string());
    }
    Ok(item)
}
pub(super) fn encode(item: &ItemState) -> Option<String> {
    let item = freeze_legacy_zero(item.clone()).ok()?;
    item_ids(&item).ok()?;
    serde_json::to_string(&item).ok()
}
pub(super) fn decode(key: &str, encoded: &str) -> Result<ItemState, String> {
    let item: ItemState = serde_json::from_str(encoded).map_err(|e| e.to_string())?;
    let item = freeze_legacy_zero(item)?;
    if item.key != key {
        return Err("custody item key disagrees with carrier".into());
    }
    item_ids(&item)?;
    Ok(item)
}
pub(super) fn reserved_ids(systems: &Stage5SystemsState) -> Result<BTreeSet<u64>, String> {
    super::refine_oven::validate(&systems.refine)?;
    let mut ids = BTreeSet::new();
    let mut reserve = |item: ItemState| -> Result<(), String> {
        for id in item_ids(&item)? {
            if !ids.insert(id) {
                return Err("duplicate UID across custody records".into());
            }
        }
        Ok(())
    };
    for listing in &systems.auction {
        if let Some(encoded) = &listing.item_state_json {
            let item = decode(&listing.item_key, encoded)?;
            if !listing.sold {
                reserve(item)?;
            }
        }
    }
    if let Some(encoded) = &systems.refine.oven_item_state_json {
        let key = systems
            .refine
            .current_item
            .as_deref()
            .ok_or("oven key missing")?;
        reserve(decode(key, encoded)?)?;
    }
    for (slot, encoded) in &systems.refine.item_states {
        if *slot >= 16 {
            return Err("invalid refine custody slot".into());
        }
        let key = systems
            .refine
            .slots
            .get(slot)
            .ok_or("orphan refine custody carrier")?;
        reserve(decode(key, encoded)?)?;
    }
    Ok(ids)
}
pub(super) fn refresh(world: &mut World) -> Result<(), String> {
    let ids = reserved_ids(&world.resource::<Stage5SystemsResource>().stage5_systems)?;
    world
        .resource_mut::<InventoryResource>()
        .reserved_item_unique_ids = ids;
    Ok(())
}

pub(super) fn can_take(inventory: &InventoryResource, item: &ItemState) -> bool {
    let Ok(ids) = item_ids(item) else {
        return false;
    };
    let mut remaining = inventory.clone();
    let Some(index) = remaining
        .inventory_items
        .iter()
        .position(|candidate| candidate.container == item.container && candidate.slot == item.slot)
    else {
        return false;
    };
    remaining.inventory_items.remove(index);
    // Old saves can contain aliases shared across grids. Never turn such an
    // ambiguous reference into custody while another live item still owns it.
    ids.iter()
        .all(|id| !inventory_unique_id_is_used(&remaining, *id))
}

/// Stage the entire operation, including all merges. A failure leaves both bag
/// and custody unchanged. Explicit refine destinations never merge or swap.
pub(super) fn plan_return(
    inventory: &InventoryResource,
    item: &ItemState,
    destination: Option<u8>,
    merge: bool,
) -> Option<(InventoryResource, Vec<ItemState>)> {
    let ids = item_ids(item).ok()?;
    let mut staged = inventory.clone();
    for id in &ids {
        staged.reserved_item_unique_ids.remove(id);
    }
    if ids
        .iter()
        .any(|id| inventory_unique_id_is_used(&staged, *id))
    {
        return None;
    }
    let mut incoming = item.clone();
    let mut changed = Vec::new();
    if merge {
        let limit = crystal_stack_size_for_item_key(&item.key);
        for existing in staged
            .belt_items
            .iter_mut()
            .chain(staged.inventory_items.iter_mut())
            .filter(|i| {
                matches!(
                    i.container,
                    ItemContainer::Belt | ItemContainer::Bag1 | ItemContainer::Bag2
                )
            })
        {
            if limit <= 1
                || existing.quantity >= limit
                || !item_stack_identity_compatible(existing, &incoming)
            {
                continue;
            }
            let count = incoming.quantity.min(limit - existing.quantity);
            if count == 0 {
                break;
            }
            existing.quantity += count;
            incoming.quantity -= count;
            changed.push(existing.clone());
        }
    }
    if incoming.quantity > 0 {
        let (container, slot) = if let Some(index) = destination {
            if !is_valid_inventory_slot(index, staged.inventory_capacity) {
                return None;
            }
            let pair = inventory_container_and_slot_for_index(index)?;
            if staged
                .inventory_items
                .iter()
                .any(|i| (i.container, i.slot) == pair)
            {
                return None;
            }
            pair
        } else {
            find_empty_inventory_item_slot(
                &staged.inventory_items,
                ItemContainer::Bag1,
                staged.inventory_capacity,
            )?
        };
        incoming.container = container;
        incoming.slot = slot;
        staged.inventory_items.push(incoming.clone());
        changed.push(incoming);
    }
    Some((staged, changed))
}
pub(super) fn returned_packets(
    before: &InventoryResource,
    items: &[ItemState],
) -> Vec<ServerPacket> {
    items
        .iter()
        .map(|item| {
            let wire = try_user_item_from_item_state(item).expect("validated custody result");
            if before
                .belt_items
                .iter()
                .chain(before.inventory_items.iter())
                .any(|old| old.unique_id == item.unique_id)
            {
                ServerPacket::RefreshItem { item: wire }
            } else {
                ServerPacket::GainedItem { item: wire }
            }
        })
        .collect()
}
