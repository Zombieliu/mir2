//! Pure, narrow physical attachment manifest for canonical SendMail receipts.
//! This file is loaded inside runtime so ItemState remains private to runtime.
//! Original typed JSON preserves retained metadata; only the current complete
//! physical tree contributes UIDs. This is not a whole-store ownership census.
use super::items::{
    try_user_item_from_item_state, validate_committed_item_state_carrier,
    validate_committed_user_item_carrier, ItemState,
};
use crate::ItemContainer;
use mir2_protocol::UserItem;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AttachmentManifest {
    pub(crate) original_items: Value,
    pub(crate) live_uids: BTreeSet<u64>,
    pub(crate) root_uids: Vec<u64>,
}

fn reject_unissued_live_nodes(item: &ItemState) -> Result<(), String> {
    if item.unique_id == 0 || crate::monthly_card::monthly_card_item_uid_is_reserved(item.unique_id)
    {
        return Err("mail transfer requires a nonzero ordinary original item UID".into());
    }
    if super::billing_monthly_card::is_card(item) {
        return Err("mail transfer ordinary manifest excludes monthly-card items".into());
    }
    for child in &item.socketed {
        reject_unissued_live_nodes(child)?;
    }
    Ok(())
}

// Deliberately mirrors mail_uid_planning::live_socket_positions/complete_item_tree.
// The claim helper is private and has claim-specific errors. Keeping this pure
// receipt extractor separate avoids granting config a gameplay/issuance API.
// Protocol-resolved positions (notably Mount Bells at slot 1) win over old hints;
// only sidecar-less omitted live children use their original explicit position.
fn complete_item_tree(item: &ItemState) -> Result<UserItem, String> {
    let mut wire = try_user_item_from_item_state(item).map_err(|error| error.to_string())?;
    if wire.unique_id != item.unique_id {
        return Err("mail transfer projection changed the original root UID".into());
    }
    let mut positions = BTreeMap::new();
    for child in &item.socketed {
        let child_wire = try_user_item_from_item_state(child).map_err(|error| error.to_string())?;
        let mut matching = wire
            .slots
            .iter()
            .enumerate()
            .filter_map(|(position, slot)| {
                slot.as_ref()
                    .is_some_and(|slot| {
                        slot.unique_id == child_wire.unique_id
                            && slot.item_index == child_wire.item_index
                    })
                    .then_some(position)
            });
        let resolved = matching.next();
        if matching.next().is_some() {
            return Err("mail transfer has ambiguous live socket position".into());
        }
        let position = if let Some(position) = resolved {
            position
        } else if item.user_item_metadata.is_none() {
            child
                .user_item_metadata
                .as_ref()
                .and_then(|metadata| metadata.captured_socket_position)
                .map_or(usize::from(child.slot), usize::from)
        } else {
            return Err("mail transfer live socket is absent from its exact parent".into());
        };
        if position >= wire.slots.len() || positions.insert(position, child).is_some() {
            return Err("mail transfer has invalid or duplicate live socket position".into());
        }
    }
    for (position, child) in positions {
        wire.slots[position] = Some(complete_item_tree(child)?);
    }
    validate_committed_user_item_carrier(&wire).map_err(|error| error.to_string())?;
    Ok(wire)
}

fn collect_live_uids(item: &UserItem, ids: &mut BTreeSet<u64>) -> Result<(), String> {
    if item.unique_id == 0
        || crate::monthly_card::monthly_card_item_uid_is_reserved(item.unique_id)
        || item.item_index == mir2_game_data::BILLING_MONTHLY_CARD_ITEM_INDEX
        || !ids.insert(item.unique_id)
    {
        return Err("mail transfer has zero, monthly-card, or duplicate live UID".into());
    }
    for child in item.slots.iter().flatten() {
        collect_live_uids(child, ids)?;
    }
    Ok(())
}

/// Canonicalize at most the five original bag attachment roots supported by
/// SendMail, preserving order and every modeled live/retained field. Never
/// normalize identities or decode descriptive strings as nested JSON.
pub(crate) fn attachment_manifest(encoded: &[String]) -> Result<AttachmentManifest, String> {
    if encoded.len() > 5 {
        return Err("mail transfer exceeds five attachment roots".into());
    }
    let mut originals = Vec::with_capacity(encoded.len());
    let mut root_uids = Vec::with_capacity(encoded.len());
    let mut live_uids = BTreeSet::new();
    for encoded_item in encoded {
        let item: ItemState = serde_json::from_str(encoded_item)
            .map_err(|error| format!("invalid mail transfer ItemState: {error}"))?;
        // Bound and validate all raw metadata/live descendants before recursive
        // inspection, including retained mappings that are not physical nodes.
        validate_committed_item_state_carrier(&item).map_err(|error| error.to_string())?;
        if !matches!(item.container, ItemContainer::Bag1 | ItemContainer::Bag2) {
            return Err("mail transfer attachment root must be an original bag item".into());
        }
        reject_unissued_live_nodes(&item)?;
        collect_live_uids(&complete_item_tree(&item)?, &mut live_uids)?;
        root_uids.push(item.unique_id);
        originals.push(item);
    }
    let original_items = serde_json::to_value(originals).map_err(|error| error.to_string())?;
    Ok(AttachmentManifest {
        original_items,
        live_uids,
        root_uids,
    })
}

#[cfg(test)]
#[path = "mail_transfer_items_tests.rs"]
mod tests;
