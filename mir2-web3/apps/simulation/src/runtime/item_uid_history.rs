//! Typed, read-only traversal of retained item identities. This is not an
//! ownership census: mirrors and historical references may repeat an identity.
//! Strings are decoded only at fields whose declared schema is an asset carrier.
use super::super::{
    equipment::{item_state_from_equipment_state, EquipmentState},
    items::{validate_committed_item_state_carrier, ItemState},
    npc::{NpcBuyBackState, NpcUsedGoodsState},
    resources::ItemRentalRecordState,
};
use crate::{
    config::{
        AccountRecord, AccountStore, CharacterSaveRecord, SharedHeroCustody, Stage5SystemsState,
    },
    runtime::NpcPurchaseProcessingOutcome,
    ItemContainer,
};
use mir2_protocol::UserItem;
use serde::de::DeserializeOwned;
use std::collections::BTreeSet;

/// A closed list of real schema roots. It intentionally accepts neither an
/// arbitrary Serialize value nor JSON/text whose meaning must be guessed.
pub(in crate::runtime) enum UidHistoryRoot<'a> {
    Store(&'a AccountStore),
    Save {
        account_id: &'a str,
        save: &'a CharacterSaveRecord,
    },
    Systems {
        account_id: &'a str,
        character_index: i32,
        systems: &'a Stage5SystemsState,
    },
    Item(&'a ItemState),
    WireItem(&'a UserItem),
}

pub(in crate::runtime) fn historical_uid_floor(root: UidHistoryRoot<'_>) -> Result<u64, String> {
    let mut floor = 0;
    visit_historical_item_uids(root, &mut |uid| floor = floor.max(uid))?;
    Ok(floor)
}

pub(in crate::runtime) fn historical_uid_conflicts(
    root: UidHistoryRoot<'_>,
    ids: &BTreeSet<u64>,
) -> Result<bool, String> {
    let mut conflict = false;
    visit_historical_item_uids(root, &mut |uid| conflict |= uid != 0 && ids.contains(&uid))?;
    Ok(conflict)
}

/// Callbacks are provisional until this returns Ok. Callers must discard the
/// aggregation on error, before asking an allocator to publish a higher floor.
pub(in crate::runtime) fn visit_historical_item_uids(
    root: UidHistoryRoot<'_>,
    visitor: &mut dyn FnMut(u64),
) -> Result<(), String> {
    match root {
        UidHistoryRoot::Store(store) => visit_store(store, visitor),
        UidHistoryRoot::Save { account_id, save } => visit_save(account_id, save, visitor),
        UidHistoryRoot::Systems {
            account_id,
            character_index,
            systems,
        } => visit_systems(account_id, character_index, systems, visitor),
        UidHistoryRoot::Item(item) => visit_item(item, visitor),
        UidHistoryRoot::WireItem(item) => visit_wire(item, visitor),
    }
}

fn decode<T: DeserializeOwned>(encoded: &str, carrier: &str) -> Result<T, String> {
    serde_json::from_str(encoded)
        .map_err(|error| format!("invalid declared UID history {carrier}: {error}"))
}

fn record(uid: u64, visitor: &mut dyn FnMut(u64)) {
    if uid != 0 {
        visitor(uid);
    }
}

fn walk_wire(item: &UserItem, visitor: &mut dyn FnMut(u64)) {
    record(item.unique_id, visitor);
    for child in item.slots.iter().flatten() {
        walk_wire(child, visitor);
    }
}

fn visit_wire(item: &UserItem, visitor: &mut dyn FnMut(u64)) -> Result<(), String> {
    // History is not physical uniqueness validation. Validate structure and
    // committed quantities using the existing private temporary-ID copy, then
    // read only the original real IDs, including legitimate unassigned zeros.
    super::super::item_grants::validate_fresh_user_item_tree(item)?;
    walk_wire(item, visitor);
    Ok(())
}

fn walk_item_descendants(item: &ItemState, visitor: &mut dyn FnMut(u64)) {
    if let Some(metadata) = &item.user_item_metadata {
        for child in metadata.slots.iter().flatten() {
            walk_wire(child, visitor);
        }
        // Reconciliation removes live children but deliberately keeps their
        // captured direct-slot identities. Those typed references are retained
        // history even when no complete child payload remains in this carrier.
        for captured in metadata
            .captured_socket_positions
            .iter()
            .flatten()
            .flatten()
        {
            record(captured.unique_id, visitor);
        }
    }
    for child in &item.socketed {
        walk_item(child, visitor);
    }
}

fn walk_item(item: &ItemState, visitor: &mut dyn FnMut(u64)) {
    record(item.unique_id, visitor);
    walk_item_descendants(item, visitor);
}

fn visit_item(item: &ItemState, visitor: &mut dyn FnMut(u64)) -> Result<(), String> {
    // This validator admits legacy raw UID0. It bounds both retained metadata
    // and live sockets without replacing missing IDs with grid aliases.
    validate_committed_item_state_carrier(item).map_err(|error| error.to_string())?;
    walk_item(item, visitor);
    Ok(())
}

fn visit_encoded_item(encoded: &str, visitor: &mut dyn FnMut(u64)) -> Result<(), String> {
    visit_item(&decode::<ItemState>(encoded, "ItemState")?, visitor)
}

fn visit_equipment(item: &EquipmentState, visitor: &mut dyn FnMut(u64)) -> Result<(), String> {
    let carrier =
        item_state_from_equipment_state(item.clone(), ItemContainer::Bag1, item.slot as u8);
    validate_committed_item_state_carrier(&carrier).map_err(|error| error.to_string())?;
    // The validation carrier may derive an old grid fallback. Only the actual
    // optional equipment root and its real descendants belong in history.
    if let Some(uid) = item.user_item_unique_id {
        record(uid, visitor);
    }
    walk_item_descendants(&carrier, visitor);
    Ok(())
}

fn visit_systems(
    account_id: &str,
    character_index: i32,
    systems: &Stage5SystemsState,
    visitor: &mut dyn FnMut(u64),
) -> Result<(), String> {
    for mail in &systems.mail {
        // Retained payloads count even after terminal status. Body/header text
        // never becomes an asset merely because it resembles JSON.
        for encoded in &mail.item_states_json {
            visit_encoded_item(encoded, visitor)?;
        }
    }
    for encoded in systems.guild.storage_item_states.values() {
        visit_encoded_item(encoded, visitor)?;
    }
    for listing in &systems.auction {
        if let Some(encoded) = &listing.item_state_json {
            visit_encoded_item(encoded, visitor)?;
        }
    }
    for encoded in systems.refine.item_states.values() {
        visit_encoded_item(encoded, visitor)?;
    }
    if let Some(encoded) = &systems.refine.oven_item_state_json {
        visit_encoded_item(encoded, visitor)?;
    }
    record(systems.refine.pending_unique_id, visitor);
    if let Some(trade) = &systems.trade {
        for uid in trade.offered_unique_ids.values() {
            record(*uid, visitor);
        }
    }
    for rental in &systems.item_rental.rented_items {
        record(rental.item_id, visitor);
    }
    // NativeGameShopLedgerV2 has no instance UID: its mail_id/product IDs are
    // different domains. GiftV1 owns a validated, exact monthly-card UID list.
    super::super::game_shop_gift::visit_historical_monthly_card_uids(
        account_id,
        character_index,
        systems,
        visitor,
    )
}

fn visit_save(
    account_id: &str,
    save: &CharacterSaveRecord,
    visitor: &mut dyn FnMut(u64),
) -> Result<(), String> {
    for encoded in save
        .inventory_items_json
        .iter()
        .chain(&save.belt_items_json)
        .chain(&save.storage_items_json)
        .chain(&save.hero_inventory_items_json)
        .chain(&save.hero_equipment_items_json)
    {
        visit_encoded_item(encoded, visitor)?;
    }
    for encoded in &save.equipment_items_json {
        visit_equipment(
            &decode::<EquipmentState>(encoded, "EquipmentState")?,
            visitor,
        )?;
    }
    for encoded in &save.npc_buy_back_items_json {
        for entry in decode::<NpcBuyBackState>(encoded, "NpcBuyBackState")?.items {
            visit_wire(&entry.item, visitor)?;
        }
    }
    for encoded in &save.npc_used_goods_items_json {
        for item in decode::<NpcUsedGoodsState>(encoded, "NpcUsedGoodsState")?.items {
            visit_wire(&item, visitor)?;
        }
    }
    for encoded in &save.item_rental_records_json {
        record(
            decode::<ItemRentalRecordState>(encoded, "ItemRentalRecordState")?.item_id,
            visitor,
        );
    }
    if let Some(encoded) = &save.stage5_systems_json {
        let systems: Stage5SystemsState = decode(encoded, "Stage5SystemsState")?;
        visit_systems(account_id, save.character.index, &systems, visitor)?;
    }
    if let Some(journal) = &save.npc_purchase_journal {
        journal
            .validate_for(
                account_id,
                save.character.index,
                &save.character.name,
                save.revision,
            )
            .map_err(|error| error.to_string())?;
        for entry in &journal.entries {
            // request.item_index is client input, including for Rejected rows.
            // Only the server's committed incoming instance is issued history.
            if let NpcPurchaseProcessingOutcome::Committed {
                incoming_unique_id, ..
            } = &entry.outcome
            {
                record(*incoming_unique_id, visitor);
            }
        }
    }
    super::super::production::visit_historical_item_uids(account_id, save, visitor)
}

fn visit_account(
    account_id: &str,
    account: &AccountRecord,
    visitor: &mut dyn FnMut(u64),
) -> Result<(), String> {
    for save in account.saves.values() {
        visit_save(account_id, save, visitor)?;
    }
    if let Some(ledger) = &account.monthly_card {
        ledger.validate()?;
        for receipt in ledger.item_receipts.values() {
            if receipt.account_id != account_id {
                return Err("monthly-card UID history owner mismatch".into());
            }
            record(receipt.unique_id, visitor);
        }
    }
    if let Some(ledger) = &account.procurement {
        ledger.validate()?;
        for lock in ledger.locks.values() {
            let item: ItemState = decode(&lock.original_item_json, "procurement lock ItemState")?;
            if item.unique_id != lock.item_uid {
                return Err("procurement lock UID history carrier mismatch".into());
            }
            visit_item(&item, visitor)?;
            record(lock.item_uid, visitor);
        }
        for delivery in ledger.deliveries.values() {
            match (&delivery.original_item_json, delivery.item_uid) {
                (Some(encoded), Some(uid)) => {
                    let item: ItemState = decode(encoded, "procurement delivery ItemState")?;
                    if item.unique_id != uid {
                        return Err("procurement delivery UID history carrier mismatch".into());
                    }
                    visit_item(&item, visitor)?;
                    record(uid, visitor);
                }
                (None, None) => {}
                _ => return Err("procurement delivery UID history is incomplete".into()),
            }
        }
    }
    Ok(())
}

fn visit_store(store: &AccountStore, visitor: &mut dyn FnMut(u64)) -> Result<(), String> {
    // Loaded legacy versions may have no newer optional authorities. Do not
    // reinterpret a future schema whose additional UID carriers are unknown.
    if store.schema_version == 0 || store.schema_version > crate::billing::BILLING_SCHEMA_VERSION {
        return Err("unsupported account-store UID history schema".into());
    }
    for (account_id, account) in &store.accounts {
        visit_account(account_id, account, visitor)?;
    }
    for guild in store.shared_guilds.values() {
        for item in guild.storage.values() {
            visit_encoded_item(&item.item_state_json, visitor)?;
        }
    }
    for hero in store.shared_heroes.values() {
        if let SharedHeroCustody::Sealed { carrier_uid } = &hero.custody {
            record(*carrier_uid, visitor);
        }
        // Released records still retain history. Mirrors are harmless repeats.
        for encoded in hero
            .state
            .inventory_items_json
            .iter()
            .chain(&hero.state.equipment_items_json)
        {
            visit_encoded_item(encoded, visitor)?;
        }
    }
    for record_state in store.sealed_stones.records() {
        record(record_state.carrier_uid(), visitor);
    }
    Ok(())
}

#[cfg(test)]
#[path = "item_uid_history_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "item_uid_history_ledger_tests.rs"]
mod ledger_tests;
