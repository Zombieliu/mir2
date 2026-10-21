//! Private full-instance custody for real player and death ground drops.
//! No economic classification, fresh reward or client-provided state is minted.
use super::super::items::{
    try_user_item_from_item_state, validate_committed_item_state_carrier, ItemState,
};
use crate::config::{GroundDropItemCustody, GroundDropItemPayload, ItemContainer};
use mir2_protocol::UserItem;
use serde_json::Value;

pub(in crate::runtime) const MAX_STATE_BYTES: usize = 256 * 1024;
const VERSION: u8 = 1;
const MAX_JSON_NODES: usize = 32_768;
const MAX_JSON_DEPTH: usize = 64;

fn bounded(value: &Value) -> bool {
    let mut pending = vec![(value, 0)];
    let mut nodes = 0_usize;
    let mut text_bytes = 0_usize;
    while let Some((value, depth)) = pending.pop() {
        nodes = nodes.saturating_add(1);
        if nodes > MAX_JSON_NODES || depth > MAX_JSON_DEPTH {
            return false;
        }
        match value {
            Value::Array(values) => {
                if values
                    .len()
                    .saturating_add(nodes)
                    .saturating_add(pending.len())
                    > MAX_JSON_NODES
                {
                    return false;
                }
                pending.extend(values.iter().map(|value| (value, depth + 1)));
            }
            Value::Object(fields) => {
                if fields
                    .len()
                    .saturating_add(nodes)
                    .saturating_add(pending.len())
                    > MAX_JSON_NODES
                {
                    return false;
                }
                for (key, value) in fields {
                    text_bytes = text_bytes.saturating_add(key.len());
                    pending.push((value, depth + 1));
                }
            }
            Value::String(text) => text_bytes = text_bytes.saturating_add(text.len()),
            _ => {}
        }
        if text_bytes > MAX_STATE_BYTES {
            return false;
        }
    }
    serde_json::to_vec(value).is_ok_and(|bytes| bytes.len() <= MAX_STATE_BYTES)
}

fn has_nested_identity(item: &ItemState) -> bool {
    !item.socketed.is_empty()
        || item.user_item_metadata.as_ref().is_some_and(|metadata| {
            metadata.slots.iter().any(Option::is_some)
                || metadata
                    .captured_socket_positions
                    .as_ref()
                    .is_some_and(|slots| slots.iter().any(Option::is_some))
        })
}

fn clear_wire_ids(item: &mut UserItem) {
    item.unique_id = 0;
    for child in item.slots.iter_mut().flatten() {
        clear_wire_ids(child);
    }
}

fn clear_state_ids(item: &mut ItemState) {
    item.unique_id = 0;
    if item.user_item_metadata.is_none() {
        // Without exact metadata, zero means the location-derived legacy UID.
        // This unassigned private carrier has no live inventory location yet.
        item.container = ItemContainer::Bag1;
        item.slot = 0;
    }
    if let Some(metadata) = item.user_item_metadata.as_mut() {
        for child in metadata.slots.iter_mut().flatten() {
            clear_wire_ids(child);
        }
        if let Some(positions) = metadata.captured_socket_positions.as_mut() {
            for identity in positions.iter_mut().flatten() {
                identity.unique_id = 0;
            }
        }
    }
    for child in &mut item.socketed {
        clear_state_ids(child);
    }
}

fn assigned(item: &UserItem) -> bool {
    item.unique_id != 0 && item.slots.iter().flatten().all(assigned)
}

fn unique_assigned_ids(item: &UserItem) -> bool {
    let mut pending = vec![item];
    let mut ids = std::collections::BTreeSet::new();
    while let Some(item) = pending.pop() {
        if item.unique_id == 0 || !ids.insert(item.unique_id) {
            return false;
        }
        pending.extend(item.slots.iter().flatten());
    }
    true
}

fn unassigned(item: &UserItem) -> bool {
    item.unique_id == 0 && item.slots.iter().flatten().all(unassigned)
}

fn freeze_assigned_state_ids(item: &mut ItemState) -> Option<()> {
    for child in &mut item.socketed {
        freeze_assigned_state_ids(child)?;
    }
    item.unique_id = try_user_item_from_item_state(item).ok()?.unique_id;
    Some(())
}

/// Capture only an existing trusted server instance, before it leaves custody.
/// Splitting a tree would duplicate embedded identities, so only plain stacks
/// may use the unassigned partial-drop path.
pub(in crate::runtime) fn capture(
    original: &ItemState,
    quantity: u32,
    preserve_uid: bool,
) -> Option<GroundDropItemPayload> {
    if quantity == 0
        || quantity > original.quantity
        || (quantity < original.quantity && has_nested_identity(original))
    {
        return None;
    }
    let mut state = original.clone();
    state.quantity = quantity;
    validate_committed_item_state_carrier(&state).ok()?;
    let mut item = try_user_item_from_item_state(&state).ok()?;
    let uid_assigned = preserve_uid && quantity == original.quantity && assigned(&item);
    if uid_assigned {
        if !unique_assigned_ids(&item) {
            return None;
        }
        // Freeze any legacy location-derived UID before the ground object
        // moves to a different grid. No other server field is rebuilt.
        freeze_assigned_state_ids(&mut state)?;
    } else {
        clear_state_ids(&mut state);
        clear_wire_ids(&mut item);
    }
    validate_committed_item_state_carrier(&state).ok()?;
    if try_user_item_from_item_state(&state).ok()? != item {
        return None;
    }
    let value = serde_json::to_value(&state).ok()?;
    if !bounded(&value) {
        return None;
    }
    Some(GroundDropItemPayload {
        item,
        uid_assigned,
        server_state: Some(GroundDropItemCustody {
            version: VERSION,
            state: value,
        }),
    })
}

/// The caller distinguishes absent legacy custody from present invalid custody.
/// Unknown/defaulted-away fields are rejected by exact round trip, without
/// changing the legacy ItemState serde behavior throughout the game.
pub(in crate::runtime) fn decode(
    payload: &GroundDropItemPayload,
    key: &str,
    quantity: u32,
) -> Option<ItemState> {
    let custody = payload.server_state.as_ref()?;
    if custody.version != VERSION
        || !bounded(&custody.state)
        || (payload.uid_assigned && !unique_assigned_ids(&payload.item))
        || (!payload.uid_assigned && !unassigned(&payload.item))
    {
        return None;
    }
    let state: ItemState = serde_json::from_value(custody.state.clone()).ok()?;
    if serde_json::to_value(&state).ok()? != custody.state
        || state.key != key
        || state.quantity != quantity
        || quantity == 0
    {
        return None;
    }
    validate_committed_item_state_carrier(&state).ok()?;
    if try_user_item_from_item_state(&state).ok()? != payload.item {
        return None;
    }
    Some(state)
}

/// Ground pickup must not discard a server-only field through an otherwise
/// equal binary UserItem merge. Only root location, count and UID may differ.
pub(in crate::runtime) fn stack_identity_matches(left: &ItemState, right: &ItemState) -> bool {
    fn identity(item: &ItemState) -> Option<Value> {
        let mut value = serde_json::to_value(item).ok()?;
        let fields = value.as_object_mut()?;
        for key in ["unique_id", "quantity", "container", "slot"] {
            fields.remove(key);
        }
        Some(value)
    }
    identity(left)
        .zip(identity(right))
        .is_some_and(|(left, right)| left == right)
}

/// Fresh allocation changes live child UIDs as well as root UIDs. Reconcile
/// the private position evidence with those same children, never rehydrate a
/// tree from UserItem and lose its server-only functional fields.
pub(in crate::runtime) fn refresh_reallocated_socket_ids(item: &mut ItemState) -> Option<()> {
    for child in &mut item.socketed {
        refresh_reallocated_socket_ids(child)?;
    }
    let children = item
        .socketed
        .iter()
        .map(|child| {
            let position = child
                .user_item_metadata
                .as_ref()?
                .captured_socket_position?;
            Some((
                usize::from(position),
                try_user_item_from_item_state(child).ok()?,
            ))
        })
        .collect::<Option<Vec<_>>>()?;
    if let Some(positions) = item
        .user_item_metadata
        .as_mut()
        .filter(|metadata| metadata.socket_layout_hydrated)
        .and_then(|metadata| metadata.captured_socket_positions.as_mut())
    {
        for (position, child) in children {
            let identity = positions.get_mut(position)?.as_mut()?;
            if identity.item_index != child.item_index {
                return None;
            }
            identity.unique_id = child.unique_id;
        }
    }
    validate_committed_item_state_carrier(item).ok()?;
    Some(())
}

#[cfg(test)]
#[path = "ground_drop_custody_tests.rs"]
mod tests;
