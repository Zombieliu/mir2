use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::prelude::World;
use mir2_game_data::LanguageCode;
use mir2_protocol::{MirGridType, ServerPacket};

use crate::config::{
    ItemContainer, Stage5MailAttachmentOrigin, Stage5MailMessage, Stage5TradeState,
    WorldItemSnapshot, CRYSTAL_MAX_INVENTORY_CAPACITY,
};

use super::inventory::{
    current_weight, find_empty_inventory_item_slot, inventory_container_and_slot_for_index,
    inventory_index_for_item, inventory_item_matches_index, inventory_unique_id_is_used,
    is_valid_inventory_slot, item_list_unique_id_is_used, item_stack_identity_compatible,
};
use super::item_custody::{encode, item_ids};
use super::item_uid_issuance::ItemUidIssuance;
use super::items::{crystal_stack_size_for_item_key, item_unique_id, ItemState};
use super::resources::{
    InventoryResource, PlayerRuntimeResource, SessionResource, Stage5SystemsResource,
};
use super::social_economy::stage5_trade_item_can_enter;

pub(super) const SLOT_COUNT: usize = 10;

/// Old saves may have a bag-backed quote and no detached carriers. New quotes
/// use the full key set, with origin slots kept only as a return preference.
pub(super) fn held_items(
    trade: &Stage5TradeState,
) -> Result<Option<BTreeMap<u8, ItemState>>, String> {
    let Some(encoded) = &trade.held_items_json else {
        return Ok(None);
    };
    if encoded.len() > SLOT_COUNT
        || encoded.keys().ne(trade.offered_slots.keys())
        || encoded.keys().ne(trade.offered_unique_ids.keys())
    {
        return Err("trade custody slots disagree with the offer".into());
    }
    let mut held = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for (&slot, encoded) in encoded {
        let item: ItemState = serde_json::from_str(encoded).map_err(|error| error.to_string())?;
        let origin = trade.offered_slots[&slot];
        if usize::from(slot) >= SLOT_COUNT
            || inventory_container_and_slot_for_index(origin) != Some((item.container, item.slot))
            || item_unique_id(&item) != trade.offered_unique_ids[&slot]
            || (item.unique_id == 0 && item.user_item_metadata.is_none())
            || !stage5_trade_item_can_enter(&item)
        {
            return Err("trade custody item disagrees with its slot or identity".into());
        }
        if item_ids(&item)?.into_iter().any(|id| !seen.insert(id)) {
            return Err("duplicate trade custody item identity".into());
        }
        held.insert(slot, item);
    }
    Ok(Some(held))
}

pub(super) fn project_owned_items(
    trade: &Stage5TradeState,
    language: LanguageCode,
) -> Option<Vec<Option<WorldItemSnapshot>>> {
    match held_items(trade) {
        Ok(None) => None,
        // A present malformed grid stays present so clients cannot fall back
        // to unrelated legacy inventory references.
        Err(_) => Some(Vec::new()),
        Ok(Some(held)) => {
            let mut slots = vec![None; SLOT_COUNT];
            for (slot, item) in held {
                let mut row = item.snapshot(language);
                row.slot = slot;
                slots[usize::from(slot)] = Some(row);
            }
            Some(slots)
        }
    }
}

pub(super) fn legacy_root_uid(inventory: &InventoryResource, item: &ItemState) -> Option<u64> {
    let id = item_unique_id(item);
    (matches!(inventory.item_uid_issuance, ItemUidIssuance::Legacy)
        && id < u64::from(CRYSTAL_MAX_INVENTORY_CAPACITY))
    .then_some(id)
}

fn conflicts(inventory: &InventoryResource, item: &ItemState, id: u64) -> bool {
    // Old carried items can already share a slot-derived root ID across
    // independent grids. Keep the original bag identity during this hold.
    if legacy_root_uid(inventory, item) == Some(id) {
        inventory.reserved_item_unique_ids.contains(&id)
            || item_list_unique_id_is_used(&inventory.inventory_items, id)
    } else {
        inventory_unique_id_is_used(inventory, id)
    }
}

pub(super) fn release_ids(
    inventory: &mut InventoryResource,
    held: &BTreeMap<u8, ItemState>,
) -> Result<(), String> {
    for item in held.values() {
        for id in item_ids(item)? {
            inventory.reserved_item_unique_ids.remove(&id);
        }
    }
    Ok(())
}

fn publish_edit(
    world: &mut World,
    mut inventory: InventoryResource,
    mut trade: Stage5TradeState,
    held: BTreeMap<u8, ItemState>,
) -> Option<()> {
    let previous = held_items(&trade).ok()??;
    release_ids(&mut inventory, &previous).ok()?;
    let mut encoded = BTreeMap::new();
    trade.offered_items.clear();
    trade.offered_slots.clear();
    trade.offered_unique_ids.clear();
    for (slot, item) in held {
        let ids = item_ids(&item).ok()?;
        if ids.iter().any(|&id| conflicts(&inventory, &item, id)) {
            return None;
        }
        inventory.reserved_item_unique_ids.extend(ids);
        encoded.insert(slot, encode(&item)?);
        trade
            .offered_slots
            .insert(slot, inventory_index_for_item(&item)?);
        trade.offered_unique_ids.insert(slot, item_unique_id(&item));
        if !trade.offered_items.contains(&item.key) {
            trade.offered_items.push(item.key);
        }
    }
    trade.held_items_json = Some(encoded);
    trade.accepted = false;
    *world.resource_mut::<InventoryResource>() = inventory;
    world
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .trade = Some(trade);
    Some(())
}

pub(super) fn deposit(world: &mut World, trade: Stage5TradeState, from: u8, to: u8) -> Option<()> {
    let mut held = held_items(&trade).ok()??;
    let mut inventory = world.resource::<InventoryResource>().clone();
    if held.contains_key(&to) || !is_valid_inventory_slot(from, inventory.inventory_capacity) {
        return None;
    }
    let index = inventory
        .inventory_items
        .iter()
        .position(|item| inventory_item_matches_index(item, from))?;
    let item = inventory.inventory_items.remove(index);
    if !stage5_trade_item_can_enter(&item) {
        return None;
    }
    held.insert(to, item);
    publish_edit(world, inventory, trade, held)
}

pub(super) fn retrieve(world: &mut World, trade: Stage5TradeState, from: u8, to: u8) -> Option<()> {
    let mut held = held_items(&trade).ok()??;
    let mut inventory = world.resource::<InventoryResource>().clone();
    if !is_valid_inventory_slot(to, inventory.inventory_capacity)
        || inventory
            .inventory_items
            .iter()
            .any(|item| inventory_item_matches_index(item, to))
    {
        return None;
    }
    let mut item = held.remove(&from)?;
    let ids = item_ids(&item).ok()?;
    for id in &ids {
        inventory.reserved_item_unique_ids.remove(id);
    }
    if ids.iter().any(|&id| conflicts(&inventory, &item, id)) {
        return None;
    }
    // Decode returned the exact carrier, including a captured zero ID. The
    // destination changes only its physical location.
    (item.container, item.slot) = inventory_container_and_slot_for_index(to)?;
    inventory.inventory_items.push(item);
    publish_edit(world, inventory, trade, held)
}

pub(super) fn move_slot(
    world: &mut World,
    trade: Stage5TradeState,
    from: u8,
    to: u8,
) -> Option<()> {
    let mut held = held_items(&trade).ok()??;
    let source = held.remove(&from)?;
    if let Some(target) = held.insert(to, source) {
        held.insert(from, target);
    }
    let inventory = world.resource::<InventoryResource>().clone();
    publish_edit(world, inventory, trade, held)
}

#[derive(Clone, Copy)]
enum EditSlot {
    Bag(usize),
    Trade(u8),
}

pub(super) fn merge(
    world: &mut World,
    trade: Stage5TradeState,
    grid_from: MirGridType,
    grid_to: MirGridType,
    id_from: u64,
    id_to: u64,
) -> Option<()> {
    let mut held = held_items(&trade).ok()??;
    let mut inventory = world.resource::<InventoryResource>().clone();
    let resolve = |grid, id| match grid {
        MirGridType::Trade => held
            .iter()
            .find(|(_, item)| item_unique_id(item) == id)
            .map(|(&slot, item)| (EditSlot::Trade(slot), item)),
        MirGridType::Inventory => {
            let mut matches = inventory
                .inventory_items
                .iter()
                .enumerate()
                .filter(|(_, item)| {
                    item_unique_id(item) == id
                        && inventory_index_for_item(item).is_some_and(|slot| {
                            is_valid_inventory_slot(slot, inventory.inventory_capacity)
                        })
                });
            let (index, item) = matches.next()?;
            matches
                .next()
                .is_none()
                .then_some((EditSlot::Bag(index), item))
        }
        _ => None,
    };
    let (from, source) = resolve(grid_from, id_from)?;
    let (to, target) = resolve(grid_to, id_to)?;
    let limit = crystal_stack_size_for_item_key(&target.key);
    if !stage5_trade_item_can_enter(source)
        || !stage5_trade_item_can_enter(target)
        || !item_stack_identity_compatible(source, target)
        || target.quantity >= limit
    {
        return None;
    }
    let count = source.quantity.min(limit - target.quantity);
    let consumed = source.quantity == count;
    match to {
        EditSlot::Bag(index) => inventory.inventory_items[index].quantity += count,
        EditSlot::Trade(slot) => held.get_mut(&slot)?.quantity += count,
    }
    match (from, consumed) {
        (EditSlot::Bag(index), true) => {
            inventory.inventory_items.remove(index);
        }
        (EditSlot::Bag(index), false) => inventory.inventory_items[index].quantity -= count,
        (EditSlot::Trade(slot), true) => {
            held.remove(&slot)?;
        }
        (EditSlot::Trade(slot), false) => held.get_mut(&slot)?.quantity -= count,
    }
    publish_edit(world, inventory, trade, held)
}

pub(super) fn matches_offer(
    trade: &Stage5TradeState,
    offer: &super::session::SharedTradeOffer,
) -> bool {
    let Some(encoded) = &trade.held_items_json else {
        return false;
    };
    let Ok(Some(held)) = held_items(trade) else {
        return false;
    };
    held.len() == offer.items.len()
        && held
            .iter()
            .zip(&offer.items)
            .all(|((&slot, item), offered)| {
                encoded[&slot] == offered.item_state_json
                    && item.key == offered.key
                    && item_unique_id(item) == offered.unique_id
            })
}

/// Restore or mail all unspent offers with the held gold in one owner update.
/// The caller owns the right to cancel this trade, including prepared escrow.
pub(super) fn cancel(
    world: &mut World,
    trade: &Stage5TradeState,
) -> Result<Vec<ServerPacket>, String> {
    let held = held_items(trade)?.ok_or("missing detached trade grid")?;
    let gold = trade
        .validated_held_gold()
        .ok_or("invalid held trade gold")?;
    let final_gold = world
        .resource::<PlayerRuntimeResource>()
        .gold
        .checked_add(gold)
        .ok_or("cannot return held trade gold")?;
    let mut inventory = world.resource::<InventoryResource>().clone();
    let mut systems = world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .clone();
    let recipient = world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()
        .ok_or("trade return requires its original character")?
        .name
        .clone();
    let max_weight =
        u16::try_from(super::stats::player_stats(world).bag_weight().max(0)).unwrap_or(u16::MAX);
    let mut packets = Vec::new();
    let mut mailed = false;
    release_ids(&mut inventory, &held)?;
    for (trade_slot, mut item) in held {
        if item_ids(&item)?
            .into_iter()
            .any(|id| conflicts(&inventory, &item, id))
        {
            return Err("cannot return duplicate held trade item".into());
        }
        let free = find_empty_inventory_item_slot(
            &inventory.inventory_items,
            ItemContainer::Bag1,
            inventory.inventory_capacity,
        );
        let weight = item.total_weight();
        let can_carry =
            u32::from(current_weight(&inventory)).saturating_add(weight) <= u32::from(max_weight);
        if let Some((container, slot)) = free.filter(|_| can_carry) {
            (item.container, item.slot) = (container, slot);
            let to = inventory_index_for_item(&item).ok_or("invalid return bag cell")?;
            inventory.inventory_items.push(item);
            packets.push(ServerPacket::RetrieveTradeItem {
                from: i32::from(trade_slot),
                to: i32::from(to),
                success: true,
            });
        } else {
            let id = systems
                .mail
                .iter()
                .map(|message| message.id)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or("mail identity exhausted")?;
            let encoded = encode(&item).ok_or("cannot mail held trade item")?;
            systems.mail.push(Stage5MailMessage {
                id,
                delivery_nonce: format!("trade-return-{}-{trade_slot}", trade.settlement_nonce),
                from: "Bichon Administrator".into(),
                to: recipient.clone(),
                subject: "Trade item returned".into(),
                body: "Could not return item to bag after trade.".into(),
                gold: 0,
                items: vec![item.key],
                item_states_json: vec![encoded],
                attachment_origin: Stage5MailAttachmentOrigin::ExistingCustody,
                opened: false,
                locked: false,
                claimed: false,
                deleted: false,
            });
            mailed = true;
        }
    }
    systems.trade = None;
    *world.resource_mut::<InventoryResource>() = inventory;
    world.resource_mut::<Stage5SystemsResource>().stage5_systems = systems;
    world.resource_mut::<PlayerRuntimeResource>().gold = final_gold;
    if gold > 0 {
        packets.push(ServerPacket::GainedGold { gold });
    }
    if mailed {
        packets.push(super::packets::stage5_receive_mail_packet(world));
    }
    packets.push(ServerPacket::TradeCancel { unlock: false });
    Ok(packets)
}
