//! Trusted starter/start migration planning. UID reservations happen before the
//! save transaction; a failed source CAS retires the IDs rather than reminting.
use super::super::item_uid_issuance::{historical_uid_floor, ItemUidIssuance};
use super::super::items::{try_item_state_from_user_item, try_user_item_from_item_state};
use super::*;
use crate::{UserItemUidAllocator, UserItemUidReason};
use std::collections::BTreeMap;

#[derive(Default)]
struct FreshRoots {
    inventory: BTreeSet<usize>,
    belt: BTreeSet<usize>,
    storage: BTreeSet<usize>,
    equipment: BTreeSet<usize>,
}

impl FreshRoots {
    fn all(save: &CharacterSaveRecord) -> Self {
        Self {
            inventory: (0..save.inventory_items_json.len()).collect(),
            belt: (0..save.belt_items_json.len()).collect(),
            storage: (0..save.storage_items_json.len()).collect(),
            equipment: (0..save.equipment_items_json.len()).collect(),
        }
    }
}

fn durable_allocator(config: &SimulationConfig) -> Result<Option<UserItemUidAllocator>, String> {
    match &config.item_uid_issuance {
        ItemUidIssuance::Legacy => Ok(None),
        ItemUidIssuance::Durable(_) => {
            config.ensure_account_store_writable()?;
            config.server_item_uid_allocator().map(Some)
        }
        ItemUidIssuance::Unavailable | ItemUidIssuance::Fenced => {
            Err("starter item mint is unavailable in shadow/fenced configuration".into())
        }
    }
}

fn live_socket_positions<'a>(
    item: &'a ItemState,
    wire: &mir2_protocol::UserItem,
) -> Result<BTreeMap<usize, &'a ItemState>, String> {
    let mut positions = BTreeMap::new();
    for child in &item.socketed {
        let captured = child
            .user_item_metadata
            .as_ref()
            .and_then(|metadata| metadata.captured_socket_position)
            .map(usize::from);
        let child_wire = try_user_item_from_item_state(child).map_err(|error| error.to_string())?;
        let mut matches = wire
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
        let resolved = matches.next();
        if matches.next().is_some() {
            return Err("starter live socket has ambiguous exact parent positions".into());
        }
        // Keep positions already resolved by the original converter, including
        // Mount Bells at protocol slot 1 despite a legacy local slot/hint 0.
        // Only a Legacy parent can intentionally omit an ordinary live child.
        let position = match resolved {
            Some(position) => position,
            None if item.user_item_metadata.is_none() => {
                captured.unwrap_or(usize::from(child.slot))
            }
            None => return Err("starter live socket is absent from its exact parent".into()),
        };
        if position >= wire.slots.len() || positions.insert(position, child).is_some() {
            return Err("starter live sockets have invalid or duplicate positions".into());
        }
    }
    Ok(positions)
}

fn preserve_live_fields(before: &ItemState, hydrated: ItemState) -> Result<ItemState, String> {
    let wire = complete_item_tree(before)?;
    let mut originals = live_socket_positions(before, &wire)?;
    let mut result = before.clone();
    result.unique_id = hydrated.unique_id;
    result.user_item_metadata = hydrated.user_item_metadata;
    result.socket_slots = hydrated.socket_slots;
    result.socketed = hydrated
        .socketed
        .into_iter()
        .map(|child| {
            let position = child
                .user_item_metadata
                .as_ref()
                .and_then(|metadata| metadata.captured_socket_position)
                .unwrap_or(child.slot) as usize;
            match originals.remove(&position) {
                Some(old) => preserve_live_fields(old, child),
                None => Ok(child),
            }
        })
        .collect::<Result<_, _>>()?;
    if !originals.is_empty() {
        return Err("starter hydration omitted a live socket descendant".into());
    }
    Ok(result)
}

fn complete_item_tree(item: &ItemState) -> Result<mir2_protocol::UserItem, String> {
    let mut wire = try_user_item_from_item_state(item).map_err(|error| error.to_string())?;
    // The Legacy wire path intentionally omits ordinary sidecar-less sockets.
    // A fresh durable tree must include them before reserving identities; it
    // must never discard those live children while hydrating exact metadata.
    // Exact parents may contain a live legacy child whose own wire conversion
    // omits grandchildren. Recurse through every resolved live position,
    // regardless of whether this particular parent has exact metadata.
    for (position, child) in live_socket_positions(item, &wire)? {
        wire.slots[position] = Some(complete_item_tree(child)?);
    }
    super::super::item_grants::validate_fresh_user_item_tree(&wire)?;
    Ok(wire)
}

fn assign_item(allocator: &UserItemUidAllocator, item: &mut ItemState) -> Result<(), String> {
    let mut wire = complete_item_tree(item)?;
    super::super::item_grants::assign_fresh_user_item_tree_uids(
        allocator,
        &mut wire,
        UserItemUidReason::CharacterStartItem,
    )?;
    let hydrated =
        try_item_state_from_user_item(item.clone(), &wire).map_err(|error| error.to_string())?;
    *item = preserve_live_fields(item, hydrated)?;
    validate_committed_item_state_carrier(item).map_err(|error| error.to_string())
}

fn entirely_unassigned_legacy_tree(item: &ItemState) -> bool {
    item.unique_id == 0
        && item.user_item_metadata.is_none()
        && item.socketed.iter().all(entirely_unassigned_legacy_tree)
}

/// Numbers in this set only exclude collisions for a private validation copy.
/// They do not establish a census or advance the actual durable authority.
fn private_id_exclusions(
    value: &serde_json::Value,
    depth: usize,
    ids: &mut BTreeSet<u64>,
) -> Result<(), String> {
    if depth > 64 {
        return Err("starter validation history depth exceeded".into());
    }
    match value {
        serde_json::Value::Number(number) => {
            if let Some(id) = number.as_u64() {
                ids.insert(id);
            }
        }
        serde_json::Value::String(text) => {
            if let Ok(id) = text.parse::<u64>() {
                ids.insert(id);
            }
            if text.trim_start().starts_with('{') || text.trim_start().starts_with('[') {
                if let Ok(encoded) = serde_json::from_str::<serde_json::Value>(text) {
                    private_id_exclusions(&encoded, depth + 1, ids)?;
                }
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                private_id_exclusions(value, depth + 1, ids)?;
            }
        }
        serde_json::Value::Object(values) => {
            for value in values.values() {
                private_id_exclusions(value, depth + 1, ids)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn private_tree_ids(
    wire: &mut mir2_protocol::UserItem,
    excluded: &mut BTreeSet<u64>,
    next: &mut u64,
) -> Result<(), String> {
    while *next == 0 || excluded.contains(next) {
        *next = next
            .checked_add(1)
            .ok_or("private starter validation identity exhausted")?;
    }
    wire.unique_id = *next;
    excluded.insert(*next);
    *next = next
        .checked_add(1)
        .ok_or("private starter validation identity exhausted")?;
    for child in wire.slots.iter_mut().flatten() {
        private_tree_ids(child, excluded, next)?;
    }
    Ok(())
}

fn validate_fresh_item_in_private_copy(
    item: &mut ItemState,
    excluded: &mut BTreeSet<u64>,
    next: &mut u64,
) -> Result<(), String> {
    let mut wire = complete_item_tree(item)?;
    super::super::item_grants::validate_fresh_user_item_tree(&wire)?;
    private_tree_ids(&mut wire, excluded, next)?;
    let hydrated = try_item_state_from_user_item(item.clone(), &wire).map_err(|e| e.to_string())?;
    *item = preserve_live_fields(item, hydrated)?;
    validate_committed_item_state_carrier(item).map_err(|e| e.to_string())
}

fn preflight_roots(
    save: &CharacterSaveRecord,
    mut fresh: FreshRoots,
    legacy_start_migration: bool,
) -> Result<FreshRoots, String> {
    let mut validation = save.clone();
    let mut excluded = BTreeSet::new();
    private_id_exclusions(
        &serde_json::to_value(save).map_err(|e| e.to_string())?,
        0,
        &mut excluded,
    )?;
    let mut next = 1;
    for (label, encoded, fresh_indices) in [
        (
            "inventory",
            &mut validation.inventory_items_json,
            &mut fresh.inventory,
        ),
        ("belt", &mut validation.belt_items_json, &mut fresh.belt),
        (
            "storage",
            &mut validation.storage_items_json,
            &mut fresh.storage,
        ),
    ] {
        let mut items = decode_saved_item_states(label, encoded)?;
        for (index, item) in items.iter_mut().enumerate() {
            if legacy_start_migration && item.unique_id == 0 && item.user_item_metadata.is_none() {
                if !entirely_unassigned_legacy_tree(item) {
                    return Err("legacy zero root contains an already assigned/exact child; explicit reconciliation required".into());
                }
                fresh_indices.insert(index);
            }
            if fresh_indices.contains(&index) {
                validate_fresh_item_in_private_copy(item, &mut excluded, &mut next)?;
            }
        }
        *encoded = encode_state_vec(&items);
    }
    let mut equipment =
        decode_saved_equipment_states("equipment", &validation.equipment_items_json)?;
    for (index, item) in equipment.iter_mut().enumerate() {
        if legacy_start_migration && item.user_item_unique_id.is_none() {
            if item.user_item_metadata.is_some()
                || !item.socketed.iter().all(entirely_unassigned_legacy_tree)
            {
                return Err("legacy equipment contains exact/assigned child identities; explicit reconciliation required".into());
            }
            fresh.equipment.insert(index);
        }
        if fresh.equipment.contains(&index) {
            let mut carrier =
                item_state_from_equipment_state(item.clone(), ItemContainer::Bag1, item.slot as u8);
            validate_fresh_item_in_private_copy(&mut carrier, &mut excluded, &mut next)?;
            item.user_item_unique_id = Some(carrier.unique_id);
            item.user_item_metadata = carrier.user_item_metadata;
            item.socket_slots = carrier.socket_slots;
            item.socketed = carrier.socketed;
        }
    }
    validation.equipment_items_json = encode_state_vec(&equipment);
    // Validates every retained identity and cross-container/Hero/held collision
    // only after all fresh structures have passed, without touching a store.
    validate_save_identities(&validation)?;
    Ok(fresh)
}

fn assign_roots(
    config: &SimulationConfig,
    save: &mut CharacterSaveRecord,
    fresh: FreshRoots,
    legacy_start_migration: bool,
) -> Result<(), String> {
    let Some(allocator) = durable_allocator(config)? else {
        return Ok(());
    };
    let fresh = preflight_roots(save, fresh, legacy_start_migration)?;
    let floor = {
        let store = config
            .account_store
            .lock()
            .map_err(|_| "account store lock poisoned")?;
        historical_uid_floor(
            &serde_json::to_value(&*store).map_err(|e| e.to_string())?,
            0,
        )?
    }
    .max(historical_uid_floor(
        &serde_json::to_value(&*save).map_err(|e| e.to_string())?,
        0,
    )?);
    allocator
        .ensure_issued_through_at_least(floor)
        .map_err(|error| super::super::item_uid_issuance::source_item_uid_error(config, error))?;
    for (label, encoded, fresh_indices) in [
        (
            "inventory",
            &mut save.inventory_items_json,
            &fresh.inventory,
        ),
        ("belt", &mut save.belt_items_json, &fresh.belt),
        ("storage", &mut save.storage_items_json, &fresh.storage),
    ] {
        let mut items = decode_saved_item_states(label, encoded)?;
        for (index, item) in items.iter_mut().enumerate() {
            if fresh_indices.contains(&index) {
                assign_item(&allocator, item).map_err(|error| {
                    super::super::item_uid_issuance::handle_item_uid_failure(config, error)
                })?;
            }
        }
        *encoded = encode_state_vec(&items);
    }
    let mut equipment = decode_saved_equipment_states("equipment", &save.equipment_items_json)?;
    for (index, item) in equipment.iter_mut().enumerate() {
        if fresh.equipment.contains(&index) {
            let mut carrier =
                item_state_from_equipment_state(item.clone(), ItemContainer::Bag1, item.slot as u8);
            assign_item(&allocator, &mut carrier).map_err(|error| {
                super::super::item_uid_issuance::handle_item_uid_failure(config, error)
            })?;
            // Preserve slot, shape, live stats and every functional field.
            item.user_item_unique_id = Some(carrier.unique_id);
            item.user_item_metadata = carrier.user_item_metadata;
            item.socket_slots = carrier.socket_slots;
            item.socketed = carrier.socketed;
        }
    }
    save.equipment_items_json = encode_state_vec(&equipment);
    validate_save_identities(save)
}

pub(super) fn prepare_new_character(
    config: &SimulationConfig,
    save: &mut CharacterSaveRecord,
) -> Result<(), String> {
    let fresh = FreshRoots::all(save);
    assign_roots(config, save, fresh, false)
}

pub(super) fn prepare_character_start(
    config: &SimulationConfig,
    save: &mut CharacterSaveRecord,
) -> Result<bool, String> {
    let original = save.clone();
    let mut fresh = FreshRoots::default();
    normalize_legacy_default_vitals(save);
    let before_equipment = save.equipment_items_json.clone();
    if normalize_legacy_synthetic_starter_equipment(save) {
        // Synthetic legacy keys are removed before canonical starters are
        // added. Preserve every retained exact item rather than reminting it.
        for (index, encoded) in save.equipment_items_json.iter().enumerate() {
            if !before_equipment.contains(encoded) {
                fresh.equipment.insert(index);
            }
        }
    }
    let before_empty = (
        save.inventory_items_json.is_empty(),
        save.belt_items_json.is_empty(),
        save.storage_items_json.is_empty(),
        save.equipment_items_json.is_empty(),
    );
    normalize_legacy_default_account_demo_seed_state(save);
    if before_empty.0 {
        fresh.inventory.extend(0..save.inventory_items_json.len());
    }
    if before_empty.1 {
        fresh.belt.extend(0..save.belt_items_json.len());
    }
    if before_empty.2 {
        fresh.storage.extend(0..save.storage_items_json.len());
    }
    if before_empty.3 {
        fresh.equipment.extend(0..save.equipment_items_json.len());
    }
    if normalize_legacy_crystal_new_character_seed_state(save) {
        fresh.inventory.clear();
        fresh.belt.clear();
        fresh.storage.clear();
        fresh.equipment = (0..save.equipment_items_json.len()).collect();
    }
    if normalize_legacy_crystal_level_one_empty_equipment(save) {
        fresh.equipment = (0..save.equipment_items_json.len()).collect();
    }
    migrate_legacy_candidate_save_record(save)?;
    match &config.item_uid_issuance {
        ItemUidIssuance::Legacy => {}
        ItemUidIssuance::Durable(_) => assign_roots(config, save, fresh, true)?,
        ItemUidIssuance::Unavailable | ItemUidIssuance::Fenced => {
            if !same_character_save(&original, save)? {
                return Err(
                    "shadow/fenced character start cannot generate legacy starter state".into(),
                );
            }
            validate_save_identities(save)?;
        }
    }
    validate_character_save_record(save)?;
    Ok(!same_character_save(&original, save)?)
}

fn require_ids(item: &mir2_protocol::UserItem, ids: &mut BTreeSet<u64>) -> Result<(), String> {
    if item.unique_id == 0 || !ids.insert(item.unique_id) {
        return Err("saved owned inventory contains zero or duplicate item UID; controlled migration required".into());
    }
    for child in item.slots.iter().flatten() {
        require_ids(child, ids)?;
    }
    Ok(())
}

fn require_raw_item_ids(item: &ItemState) -> Result<(), String> {
    if item.unique_id == 0 {
        return Err("saved inventory root/child lacks a durable item UID".into());
    }
    for child in &item.socketed {
        require_raw_item_ids(child)?;
    }
    Ok(())
}

pub(super) fn validate_inventory_identities(inventory: &InventoryResource) -> Result<(), String> {
    let mut ids = inventory.reserved_item_unique_ids.clone();
    for item in inventory
        .inventory_items
        .iter()
        .chain(&inventory.belt_items)
        .chain(&inventory.storage_items)
    {
        require_raw_item_ids(item)?;
        require_ids(&complete_item_tree(item)?, &mut ids)?;
    }
    for item in &inventory.equipment_items {
        if item.user_item_unique_id.is_none_or(|id| id == 0) {
            return Err("saved equipment lacks an exact durable item UID".into());
        }
        let carrier =
            item_state_from_equipment_state(item.clone(), ItemContainer::Bag1, item.slot as u8);
        require_raw_item_ids(&carrier)?;
        require_ids(&complete_item_tree(&carrier)?, &mut ids)?;
    }
    Ok(())
}

fn validate_save_identities(save: &CharacterSaveRecord) -> Result<(), String> {
    let decoded = decode_and_validate_character_save(save)?;
    let mut inventory = InventoryResource::new(BASE_STORAGE_SLOTS);
    inventory.inventory_items = decoded.inventory_items;
    inventory.belt_items = decoded.belt_items;
    inventory.storage_items = decoded.storage_items;
    inventory.equipment_items = decoded.equipment_items;
    inventory.reserved_item_unique_ids =
        super::super::item_custody::reserved_ids(&decoded.stage5_systems)?;
    for id in super::super::hero_inventory::validate_hero_custody(&decoded.hero_inventory)? {
        if !inventory.reserved_item_unique_ids.insert(id) {
            return Err("Hero UID collides with held custody".into());
        }
    }
    validate_inventory_identities(&inventory)
}

#[cfg(test)]
#[path = "starter_uid_planning_tests.rs"]
mod tests;
