//! Mail claims plan the entire batch before contacting any real UID issuer.
//! Temporary Legacy identities exist only in the private capacity plan.
use super::super::item_uid_issuance::UidHistoryRoot;
use super::super::items::{try_item_state_from_user_item, try_user_item_from_item_state};
use super::super::save::{
    active_session_mutating_account_id, encode_state_vec, merge_external_stage5_mail,
};
use super::super::{billing_monthly_card, item_grants, item_uid_issuance};
use super::*;
use crate::{CharacterSaveRecord, Stage5MailAttachmentOrigin, UserItemUidReason};
use mir2_protocol::UserItem;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
pub(in crate::runtime) struct MailClaimPlan {
    pub(in crate::runtime) claim: Stage5MailClaimOutcome,
    pub(in crate::runtime) inventory: InventoryResource,
    pub(in crate::runtime) gold: u32,
    pub(in crate::runtime) credit: u32,
    pub(in crate::runtime) systems: Stage5SystemsState,
    issued: BTreeSet<u64>,
    custody_ids: BTreeSet<u64>,
    custody_deliveries: BTreeSet<String>,
}

fn failure(_: impl std::fmt::Display) -> Stage5MailClaimError {
    Stage5MailClaimError::AuthorityUnavailable
}

fn live_socket_positions<'a>(
    item: &'a ItemState,
    wire: &UserItem,
) -> Result<BTreeMap<usize, &'a ItemState>, Stage5MailClaimError> {
    let mut positions = BTreeMap::new();
    for child in &item.socketed {
        let child_wire = try_user_item_from_item_state(child)
            .map_err(|_| Stage5MailClaimError::InvalidExactItemState)?;
        let mut matches = wire.slots.iter().enumerate().filter_map(|(index, slot)| {
            slot.as_ref()
                .is_some_and(|slot| {
                    slot.unique_id == child_wire.unique_id
                        && slot.item_index == child_wire.item_index
                })
                .then_some(index)
        });
        let resolved = matches.next();
        if matches.next().is_some() {
            return Err(Stage5MailClaimError::InvalidExactItemState);
        }
        // Existing protocol rules, including Mount Bells fixed at slot 1,
        // outrank stale local/captured hints. Only a child omitted by the
        // sidecar-less compatibility converter needs an explicit fallback.
        let position = if let Some(position) = resolved {
            position
        } else if item.user_item_metadata.is_none() {
            child
                .user_item_metadata
                .as_ref()
                .and_then(|metadata| metadata.captured_socket_position)
                .map_or(usize::from(child.slot), usize::from)
        } else {
            return Err(Stage5MailClaimError::InvalidExactItemState);
        };
        if position >= wire.slots.len() || positions.insert(position, child).is_some() {
            return Err(Stage5MailClaimError::InvalidExactItemState);
        }
    }
    Ok(positions)
}

/// Legacy protocol conversion intentionally omits ordinary sidecar-less sockets.
/// Fresh mail custody must account for every live descendant before any issuer
/// is contacted, including empty positions and children below exact parents.
fn complete_item_tree(item: &ItemState) -> Result<UserItem, Stage5MailClaimError> {
    let mut wire = try_user_item_from_item_state(item)
        .map_err(|_| Stage5MailClaimError::InvalidExactItemState)?;
    for (position, child) in live_socket_positions(item, &wire)? {
        wire.slots[position] = Some(complete_item_tree(child)?);
    }
    item_grants::validate_fresh_user_item_tree(&wire)
        .map_err(|_| Stage5MailClaimError::InvalidExactItemState)?;
    Ok(wire)
}

fn preserve_live_fields(
    before: &ItemState,
    hydrated: ItemState,
) -> Result<ItemState, Stage5MailClaimError> {
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
                Some(original) => preserve_live_fields(original, child),
                None => Ok(child),
            }
        })
        .collect::<Result<_, _>>()?;
    if !originals.is_empty() {
        return Err(Stage5MailClaimError::InvalidExactItemState);
    }
    validate_exact_stage5_item_state(&result)
        .map_err(|_| Stage5MailClaimError::InvalidExactItemState)?;
    Ok(result)
}

fn hydrate_complete_item(
    before: &ItemState,
    wire: &UserItem,
) -> Result<ItemState, Stage5MailClaimError> {
    let hydrated = try_item_state_from_user_item(before.clone(), wire)
        .map_err(|_| Stage5MailClaimError::InvalidExactItemState)?;
    preserve_live_fields(before, hydrated)
}

fn collect_custody_ids(
    item: &ItemState,
    ids: &mut BTreeSet<u64>,
) -> Result<(), Stage5MailClaimError> {
    fn nonzero(item: &ItemState) -> bool {
        item.unique_id != 0 && item.socketed.iter().all(nonzero)
    }
    fn collect(item: &UserItem, ids: &mut BTreeSet<u64>) -> Result<(), Stage5MailClaimError> {
        if item.unique_id == 0
            || crate::monthly_card::skip_monthly_card_reserved_unique_id(item.unique_id)
                != item.unique_id
            || !ids.insert(item.unique_id)
        {
            return Err(Stage5MailClaimError::InvalidExactItemState);
        }
        for child in item.slots.iter().flatten() {
            collect(child, ids)?;
        }
        Ok(())
    }
    if !nonzero(item) {
        return Err(Stage5MailClaimError::InvalidExactItemState);
    }
    collect(&complete_item_tree(item)?, ids)
}

fn encoded_item_conflicts(encoded: &str, ids: &BTreeSet<u64>) -> Result<bool, String> {
    let item: ItemState = serde_json::from_str(encoded).map_err(|e| e.to_string())?;
    validate_exact_stage5_item_state(&item)?;
    fn intersects(item: &UserItem, ids: &BTreeSet<u64>) -> bool {
        ids.contains(&item.unique_id) || item.slots.iter().flatten().any(|i| intersects(i, ids))
    }
    let raw = complete_item_tree(&item).map_err(|_| "invalid custody ownership tree")?;
    Ok(intersects(&raw, ids))
}

/// Check physical owned carriers, never immutable issuance/trade receipts.
fn save_custody_conflicts(
    save: &CharacterSaveRecord,
    excluded_deliveries: &BTreeSet<String>,
    ids: &BTreeSet<u64>,
) -> Result<bool, String> {
    validate_character_save_record(save)?;
    for encoded in save
        .inventory_items_json
        .iter()
        .chain(&save.belt_items_json)
        .chain(&save.storage_items_json)
        .chain(&save.hero_inventory_items_json)
        .chain(&save.hero_equipment_items_json)
    {
        if encoded_item_conflicts(encoded, ids)? {
            return Ok(true);
        }
    }
    for encoded in &save.equipment_items_json {
        let item: super::super::equipment::EquipmentState =
            serde_json::from_str(encoded).map_err(|e| e.to_string())?;
        if ids
            .iter()
            .any(|id| super::super::inventory::equipment_tree_unique_id_is_used(&item, *id))
        {
            return Ok(true);
        }
    }
    let state = systems(save)?;
    for mail in &state.mail {
        if !mail.claimed && !mail.deleted && !excluded_deliveries.contains(&mail.delivery_nonce) {
            for encoded in &mail.item_states_json {
                if encoded_item_conflicts(encoded, ids)? {
                    return Ok(true);
                }
            }
        }
    }
    for encoded in state
        .guild
        .storage_item_states
        .values()
        .chain(
            state
                .auction
                .iter()
                .filter(|listing| !listing.sold)
                .filter_map(|listing| listing.item_state_json.as_ref()),
        )
        .chain(state.refine.item_states.values())
        .chain(state.refine.oven_item_state_json.iter())
    {
        if encoded_item_conflicts(encoded, ids)? {
            return Ok(true);
        }
    }
    if state.trade.as_ref().is_some_and(|trade| {
        !trade.completed && trade.offered_unique_ids.values().any(|id| ids.contains(id))
    }) || state
        .item_rental
        .rented_items
        .iter()
        .any(|item| ids.contains(&item.item_id))
    {
        return Ok(true);
    }
    for encoded in &save.item_rental_records_json {
        let item: super::super::resources::ItemRentalRecordState =
            serde_json::from_str(encoded).map_err(|e| e.to_string())?;
        if ids.contains(&item.item_id) {
            return Ok(true);
        }
    }
    fn wire_conflicts(item: &UserItem, ids: &BTreeSet<u64>) -> bool {
        ids.contains(&item.unique_id) || item.slots.iter().flatten().any(|i| wire_conflicts(i, ids))
    }
    for encoded in &save.npc_buy_back_items_json {
        let state: super::super::npc::NpcBuyBackState =
            serde_json::from_str(encoded).map_err(|e| e.to_string())?;
        if state
            .items
            .iter()
            .any(|entry| wire_conflicts(&entry.item, ids))
        {
            return Ok(true);
        }
    }
    for encoded in &save.npc_used_goods_items_json {
        let state: super::super::npc::NpcUsedGoodsState =
            serde_json::from_str(encoded).map_err(|e| e.to_string())?;
        if state.items.iter().any(|item| wire_conflicts(item, ids)) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn store_custody_conflicts(
    store: &crate::AccountStore,
    source_account: &str,
    source_character: i32,
    excluded_deliveries: &BTreeSet<String>,
    ids: &BTreeSet<u64>,
) -> Result<bool, String> {
    let empty = BTreeSet::new();
    for (account_id, account) in &store.accounts {
        for (index, save) in &account.saves {
            let excluded = if account_id == source_account && *index == source_character {
                excluded_deliveries
            } else {
                &empty
            };
            if save_custody_conflicts(save, excluded, ids)? {
                return Ok(true);
            }
        }
        if let Some(procurement) = &account.procurement {
            for encoded in procurement
                .locks
                .values()
                .filter(|lock| !lock.released)
                .map(|lock| &lock.original_item_json)
                .chain(
                    procurement
                        .deliveries
                        .values()
                        .filter(|delivery| !delivery.claimed)
                        .filter_map(|delivery| delivery.original_item_json.as_ref()),
                )
            {
                if encoded_item_conflicts(encoded, ids)? {
                    return Ok(true);
                }
            }
        }
    }
    for guild in store.shared_guilds.values() {
        for item in guild.storage.values() {
            if encoded_item_conflicts(&item.item_state_json, ids)? {
                return Ok(true);
            }
        }
    }
    for hero in store.shared_heroes.values() {
        if matches!(hero.custody, crate::config::SharedHeroCustody::Released) {
            continue;
        }
        for encoded in hero
            .state
            .inventory_items_json
            .iter()
            .chain(&hero.state.equipment_items_json)
        {
            if encoded_item_conflicts(encoded, ids)? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Preserve the original key-only attachment's stacking policy in the private
/// capacity view. Real identities are assigned only after every parcel passes.
fn plan_keyed_attachment(
    inventory: &mut InventoryResource,
    key: &str,
    durable: bool,
    planning_floor: u64,
) -> Result<ItemState, Stage5MailClaimError> {
    let grant = item_grants::FreshItemGrant::plain(
        ItemContainer::Bag1,
        key,
        &stage5_item_name(key),
        "Stage 5 mail attachment.",
        20,
        1,
        1,
        UserItemUidReason::SystemMailGrant,
    );
    let mut item = match grant {
        Ok(grant) => grant.prototype,
        Err(_) if !durable && crystal_item_template_for_item_key(key).is_none() => ItemState {
            stone_serial: None,
            key: key.into(),
            name: stage5_item_name(key),
            icon: super::super::items::item_icon_for_key(key),
            slot: 20,
            unique_id: 0,
            container: ItemContainer::Bag1,
            quantity: 1,
            description: "Stage 5 mail attachment.".into(),
            durability_current: None,
            durability_max: None,
            weight: 1,
            equip_slot: None,
            grade: crate::config::ItemGrade::None,
            added_attack: 0,
            added_defence: 0,
            added_stats: Vec::new(),
            socketed: Vec::new(),
            user_item_metadata: None,
            cursed: false,
            socket_slots: 0,
            gem_count: 0,
            identified: None,
            soul_bound_id: None,
            sealed_expiry_time_binary_datetime: 0,
            sealed_next_time_binary_datetime: 0,
            rental_binding_flags: 0,
            rental_owner_name: String::new(),
            rental_expiry_binary_datetime: 0,
            rental_locked: false,
            attack: 0,
            defence: 0,
            heal_hp: 0,
            heal_mp: 0,
        },
        Err(_) => return Err(Stage5MailClaimError::InvalidExactItemState),
    };
    let max_stack = super::super::items::crystal_stack_size_for_item_key(key);
    let metadata = item.durability_current.is_some()
        || item.durability_max.is_some()
        || item.added_attack != 0
        || item.added_defence != 0
        || !item.added_stats.is_empty()
        || item.cursed
        || item.socket_slots != 0;
    if max_stack > 1 && !metadata {
        if let Some(existing) = inventory.belt_items.iter_mut().find(|existing| {
            existing.quantity < max_stack
                && super::super::inventory::item_stack_identity_compatible(existing, &item)
        }) {
            existing.quantity += 1;
            return Ok(existing.clone());
        }
        if let Some(existing) = inventory.inventory_items.iter_mut().find(|existing| {
            existing.quantity < max_stack
                && super::super::inventory::item_stack_identity_compatible(existing, &item)
                && super::super::inventory::item_containers_stack_together(
                    existing.container,
                    ItemContainer::Bag1,
                )
        }) {
            existing.quantity += 1;
            return Ok(existing.clone());
        }
    }
    let (container, slot) =
        super::super::inventory::crystal_empty_add_item_slots(inventory, ItemContainer::Bag1, key)
            .into_iter()
            .next()
            .ok_or(Stage5MailClaimError::Capacity)?;
    item.container = container;
    item.slot = slot;
    item.quantity = 1;
    // This function is used only with the explicitly private Legacy capacity
    // clone. The resulting identity is replaced before any durable publication.
    item.unique_id = crate::player_drop::player_drop_next_ordinary_item_uid(
        planning_floor.max(super::super::inventory::inventory_max_unique_id(inventory)),
    )
    .map_err(failure)?;
    if container == ItemContainer::Belt {
        inventory.belt_items.push(item.clone())
    } else {
        inventory.inventory_items.push(item.clone())
    }
    Ok(item)
}

fn temporary_fresh_tree(
    inventory: &InventoryResource,
    item: ItemState,
    planning_floor: u64,
) -> Result<ItemState, Stage5MailClaimError> {
    let mut raw = complete_item_tree(&item)?;
    // The complete validated carrier is still the original tree here. No
    // private planning identity may enter this historical read or real issuer.
    let incoming =
        item_uid_issuance::historical_uid_floor(UidHistoryRoot::WireItem(&raw)).map_err(failure)?;
    let mut previous = planning_floor
        .max(incoming)
        .max(super::super::inventory::inventory_max_unique_id(inventory));
    fn assign(item: &mut UserItem, previous: &mut u64) -> Result<(), Stage5MailClaimError> {
        *previous =
            crate::player_drop::player_drop_next_ordinary_item_uid(*previous).map_err(failure)?;
        item.unique_id = *previous;
        for child in item.slots.iter_mut().flatten() {
            assign(child, previous)?;
        }
        Ok(())
    }
    assign(&mut raw, &mut previous)?;
    hydrate_complete_item(&item, &raw)
}

fn plan_one(
    world: &mut World,
    mail_id: u32,
    durable: bool,
    planning_floor: u64,
    preserved_roots: &mut BTreeSet<u64>,
    custody_ids: &mut BTreeSet<u64>,
    custody_deliveries: &mut BTreeSet<String>,
) -> Result<Stage5MailClaimOutcome, Stage5MailClaimError> {
    let (index, mail) = world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .mail
        .iter()
        .enumerate()
        .find(|(_, mail)| mail.id == mail_id && !mail.deleted)
        .map(|(i, m)| (i, m.clone()))
        .ok_or(Stage5MailClaimError::NotFound)?;
    if mail.claimed {
        return Err(Stage5MailClaimError::AlreadyClaimed);
    }
    let exact = mail
        .item_states_json
        .iter()
        .map(|encoded| {
            serde_json::from_str::<ItemState>(encoded)
                .map_err(|_| Stage5MailClaimError::InvalidExactItemState)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if exact
        .iter()
        .any(|item| validate_exact_stage5_item_state(item).is_err())
    {
        return Err(Stage5MailClaimError::InvalidExactItemState);
    }
    billing_monthly_card::validate_claim_items(world, &exact)
        .map_err(|_| Stage5MailClaimError::InvalidExactItemState)?;
    let ordinary = exact
        .iter()
        .any(|item| !billing_monthly_card::is_card(item))
        || (mail.item_states_json.is_empty() && !mail.items.is_empty());
    if durable && ordinary {
        match mail.attachment_origin {
            Stage5MailAttachmentOrigin::LegacyUnknown => {
                return Err(Stage5MailClaimError::InvalidExactItemState)
            }
            Stage5MailAttachmentOrigin::ExistingCustody => {
                if mail.item_states_json.is_empty() {
                    return Err(Stage5MailClaimError::InvalidExactItemState);
                }
                for item in exact
                    .iter()
                    .filter(|item| !billing_monthly_card::is_card(item))
                {
                    collect_custody_ids(item, custody_ids)?;
                    preserved_roots.insert(item.unique_id);
                }
                custody_deliveries.insert(mail.delivery_nonce.clone());
            }
            Stage5MailAttachmentOrigin::FreshGrant => {}
        }
    }
    let slots = stage5_mail_exact_item_slots(
        &world.resource::<InventoryResource>().inventory_items,
        &exact,
        world.resource::<InventoryResource>().inventory_capacity,
    )
    .ok_or(Stage5MailClaimError::Capacity)?;
    let gold = world
        .resource::<PlayerRuntimeResource>()
        .gold
        .checked_add(mail.gold)
        .ok_or(Stage5MailClaimError::BalanceOverflow)?;
    let mut gained = Vec::new();
    for (mut item, (container, slot)) in exact.into_iter().zip(slots) {
        item.container = container;
        item.slot = slot;
        if !billing_monthly_card::is_card(&item) {
            item = if durable
                && mail.attachment_origin == Stage5MailAttachmentOrigin::ExistingCustody
            {
                hydrate_complete_item(&item, &complete_item_tree(&item)?)?
            } else {
                temporary_fresh_tree(world.resource::<InventoryResource>(), item, planning_floor)?
            };
        }
        world
            .resource_mut::<InventoryResource>()
            .inventory_items
            .push(item.clone());
        gained.push(item);
    }
    if mail.item_states_json.is_empty() {
        for key in &mail.items {
            if billing_monthly_card::is_card_key(key) {
                return Err(Stage5MailClaimError::InvalidExactItemState);
            }
            gained.push(plan_keyed_attachment(
                &mut world.resource_mut::<InventoryResource>(),
                key,
                durable,
                planning_floor,
            )?);
        }
    }
    world.resource_mut::<PlayerRuntimeResource>().gold = gold;
    let mut systems = world.resource_mut::<Stage5SystemsResource>();
    let consumed = &mut systems.stage5_systems.mail[index];
    consumed.claimed = true;
    consumed.gold = 0;
    consumed.items.clear();
    consumed.item_states_json.clear();
    Ok(Stage5MailClaimOutcome {
        gold: mail.gold,
        items: gained,
    })
}

fn record_mapping(before: &UserItem, after: &UserItem, ids: &mut BTreeMap<u64, u64>) {
    ids.insert(before.unique_id, after.unique_id);
    for (before, after) in before
        .slots
        .iter()
        .flatten()
        .zip(after.slots.iter().flatten())
    {
        record_mapping(before, after, ids);
    }
}
fn remap_tree(item: &mut UserItem, ids: &BTreeMap<u64, u64>) {
    if let Some(uid) = ids.get(&item.unique_id) {
        item.unique_id = *uid;
    }
    for child in item.slots.iter_mut().flatten() {
        remap_tree(child, ids);
    }
}

pub(in crate::runtime) fn prepare_claims(
    world: &World,
    mail_ids: &[u32],
) -> Result<MailClaimPlan, Stage5MailClaimError> {
    prepare_claims_with_authority(world, world, mail_ids)
}
fn prepare_claims_with_authority(
    world: &World,
    authority: &World,
    mail_ids: &[u32],
) -> Result<MailClaimPlan, Stage5MailClaimError> {
    if mail_ids.is_empty()
        || mail_ids.len() > 100
        || mail_ids.iter().collect::<BTreeSet<_>>().len() != mail_ids.len()
    {
        return Err(Stage5MailClaimError::NotFound);
    }
    let original = world.resource::<InventoryResource>().clone();
    let durable = !matches!(
        original.item_uid_issuance,
        item_uid_issuance::ItemUidIssuance::Legacy
    ) || authority
        .get_resource::<RuntimeConfigResource>()
        .is_some_and(|runtime| {
            !matches!(
                runtime.config.item_uid_issuance,
                item_uid_issuance::ItemUidIssuance::Legacy
            )
        });
    let source = if durable {
        let config = &authority
            .get_resource::<RuntimeConfigResource>()
            .ok_or(Stage5MailClaimError::AuthorityUnavailable)?
            .config;
        config.ensure_account_store_writable().map_err(failure)?;
        let item_uid_issuance::ItemUidIssuance::Durable(allocator) = &original.item_uid_issuance
        else {
            return Err(Stage5MailClaimError::AuthorityUnavailable);
        };
        config
            .item_uid_issuance
            .require_same(allocator)
            .map_err(failure)?;
        let session = authority
            .get_resource::<SessionResource>()
            .ok_or(Stage5MailClaimError::AuthorityUnavailable)?;
        let account = active_session_mutating_account_id(session)
            .ok_or(Stage5MailClaimError::AuthorityUnavailable)?;
        let index = session
            .selected_character
            .as_ref()
            .ok_or(Stage5MailClaimError::AuthorityUnavailable)?
            .index;
        let store = config.account_store.lock().map_err(failure)?;
        let canonical = store
            .accounts
            .get(&account)
            .and_then(|record| record.saves.get(&index))
            .ok_or(Stage5MailClaimError::AuthorityUnavailable)?;
        let canonical_mail =
            normalized_mail(&systems(canonical).map_err(failure)?.mail).map_err(failure)?;
        for id in mail_ids {
            let visible = world
                .resource::<Stage5SystemsResource>()
                .stage5_systems
                .mail
                .iter()
                .find(|mail| mail.id == *id)
                .ok_or(Stage5MailClaimError::NotFound)?;
            let stored = canonical_mail
                .iter()
                .find(|mail| mail.delivery_nonce == visible.delivery_nonce)
                .ok_or(Stage5MailClaimError::InvalidExactItemState)?;
            let mut payload = visible.clone();
            payload.id = stored.id;
            payload.opened = stored.opened;
            payload.locked = stored.locked;
            if &payload != stored {
                return Err(Stage5MailClaimError::InvalidExactItemState);
            }
        }
        Some((config, account, index))
    } else {
        None
    };
    let initial: BTreeSet<_> = original
        .inventory_items
        .iter()
        .chain(&original.belt_items)
        .map(|item| item.unique_id)
        .collect();
    let mut inventory = original.clone();
    inventory.item_uid_issuance = item_uid_issuance::ItemUidIssuance::Legacy;
    // Keep private planning identities above every incoming attachment, so a
    // later existing-custody delivery cannot alias an earlier temporary tree.
    let session = authority
        .get_resource::<SessionResource>()
        .ok_or(Stage5MailClaimError::AuthorityUnavailable)?;
    let account_id = active_session_mutating_account_id(session)
        .ok_or(Stage5MailClaimError::AuthorityUnavailable)?;
    let character_index = session
        .selected_character
        .as_ref()
        .ok_or(Stage5MailClaimError::AuthorityUnavailable)?
        .index;
    let planning_floor = item_uid_issuance::historical_uid_floor(UidHistoryRoot::Systems {
        account_id: &account_id,
        character_index,
        systems: &world.resource::<Stage5SystemsResource>().stage5_systems,
    })
    .map_err(failure)?;
    let mut staged = World::new();
    staged.insert_resource(inventory);
    staged.insert_resource(world.resource::<PlayerRuntimeResource>().clone());
    staged.insert_resource(world.resource::<Stage5SystemsResource>().clone());
    // Capture account-backed card authorization before any account transaction.
    if let Ok(units) = billing_monthly_card::trusted_claim_units(world) {
        staged.insert_resource(units);
    }
    let mut claim = Stage5MailClaimOutcome {
        gold: 0,
        items: Vec::new(),
    };
    let mut preserved_roots = BTreeSet::new();
    let mut custody_ids = BTreeSet::new();
    let mut custody_deliveries = BTreeSet::new();
    for id in mail_ids {
        let result = plan_one(
            &mut staged,
            *id,
            durable,
            planning_floor,
            &mut preserved_roots,
            &mut custody_ids,
            &mut custody_deliveries,
        )?;
        claim.gold = claim
            .gold
            .checked_add(result.gold)
            .ok_or(Stage5MailClaimError::BalanceOverflow)?;
        claim.items.extend(result.items);
    }
    let mut inventory = staged.resource::<InventoryResource>().clone();
    inventory.item_uid_issuance = original.item_uid_issuance.clone();
    inventory.reserved_item_unique_ids = original.reserved_item_unique_ids.clone();
    if !custody_ids.is_empty() {
        let (config, account, index) = source
            .as_ref()
            .ok_or(Stage5MailClaimError::AuthorityUnavailable)?;
        // Account-only PG commits cannot lock every external physical owner.
        // Until a complete ownership transaction is supplied, this origin is
        // supported only by the single File-source account-store transaction.
        if config.account_store_database_url.is_some() {
            return Err(Stage5MailClaimError::AuthorityUnavailable);
        }
        if custody_ids.iter().any(|id| {
            super::super::inventory::inventory_unique_id_is_used(
                authority.resource::<InventoryResource>(),
                *id,
            )
        }) {
            return Err(Stage5MailClaimError::InvalidExactItemState);
        }
        let active = snapshot_active_character_save(authority)
            .ok_or(Stage5MailClaimError::AuthorityUnavailable)?;
        if save_custody_conflicts(&active, &custody_deliveries, &custody_ids).map_err(failure)? {
            return Err(Stage5MailClaimError::InvalidExactItemState);
        }
        let store = config.account_store.lock().map_err(failure)?;
        if store_custody_conflicts(&store, account, *index, &custody_deliveries, &custody_ids)
            .map_err(failure)?
        {
            return Err(Stage5MailClaimError::InvalidExactItemState);
        }
    }
    let mut mapping = BTreeMap::new();
    let fresh = inventory
        .inventory_items
        .iter()
        .chain(&inventory.belt_items)
        .any(|item| {
            !initial.contains(&item.unique_id)
                && !preserved_roots.contains(&item.unique_id)
                && !billing_monthly_card::is_card(item)
        });
    if fresh {
        match &original.item_uid_issuance {
            item_uid_issuance::ItemUidIssuance::Unavailable
            | item_uid_issuance::ItemUidIssuance::Fenced => {
                return Err(Stage5MailClaimError::AuthorityUnavailable);
            }
            item_uid_issuance::ItemUidIssuance::Legacy => {
                if authority
                    .get_resource::<RuntimeConfigResource>()
                    .is_some_and(|resource| {
                        !matches!(
                            resource.config.item_uid_issuance,
                            item_uid_issuance::ItemUidIssuance::Legacy
                        )
                    })
                {
                    return Err(Stage5MailClaimError::AuthorityUnavailable);
                }
            }
            item_uid_issuance::ItemUidIssuance::Durable(_) => {}
        }
    }
    if fresh
        && !matches!(
            original.item_uid_issuance,
            item_uid_issuance::ItemUidIssuance::Legacy
        )
    {
        let config = &authority
            .get_resource::<RuntimeConfigResource>()
            .ok_or(Stage5MailClaimError::AuthorityUnavailable)?
            .config;
        item_uid_issuance::refresh_world_history_floor(authority)
            .map_err(|error| failure(item_uid_issuance::handle_item_uid_failure(config, error)))?;
        let allocator = config.server_item_uid_allocator().map_err(failure)?;
        for item in inventory
            .inventory_items
            .iter_mut()
            .chain(&mut inventory.belt_items)
        {
            if initial.contains(&item.unique_id)
                || preserved_roots.contains(&item.unique_id)
                || billing_monthly_card::is_card(item)
            {
                continue;
            }
            let before = complete_item_tree(item)?;
            let mut raw = before.clone();
            fn clear(item: &mut UserItem) {
                item.unique_id = 0;
                for child in item.slots.iter_mut().flatten() {
                    clear(child);
                }
            }
            clear(&mut raw);
            item_grants::assign_fresh_user_item_tree_uids(
                &allocator,
                &mut raw,
                UserItemUidReason::SystemMailGrant,
            )
            .map_err(|error| failure(item_uid_issuance::handle_item_uid_failure(config, error)))?;
            record_mapping(&before, &raw, &mut mapping);
            *item = hydrate_complete_item(item, &raw)?;
        }
        for item in &mut claim.items {
            // Ledger-authorized cards keep their deterministic plain carrier.
            // Hydrating them would add metadata that card validation rejects.
            if billing_monthly_card::is_card(item) {
                continue;
            }
            let mut raw = complete_item_tree(item)?;
            remap_tree(&mut raw, &mapping);
            *item = hydrate_complete_item(item, &raw)?;
        }
    }
    Ok(MailClaimPlan {
        claim,
        inventory,
        gold: staged.resource::<PlayerRuntimeResource>().gold,
        credit: staged.resource::<PlayerRuntimeResource>().credit,
        systems: staged
            .resource::<Stage5SystemsResource>()
            .stage5_systems
            .clone(),
        issued: mapping.values().copied().collect(),
        custody_ids,
        custody_deliveries,
    })
}

pub(in crate::runtime) fn publish_claim(world: &mut World, plan: &MailClaimPlan) {
    *world.resource_mut::<InventoryResource>() = plan.inventory.clone();
    let mut player = world.resource_mut::<PlayerRuntimeResource>();
    player.gold = plan.gold;
    player.credit = plan.credit;
    drop(player);
    world.resource_mut::<Stage5SystemsResource>().stage5_systems = plan.systems.clone();
}

fn systems(save: &CharacterSaveRecord) -> Result<Stage5SystemsState, String> {
    save.stage5_systems_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|_| "mail claim mailbox is invalid".to_owned())
        .map(Option::unwrap_or_default)
}
fn normalized_mail(mail: &[Stage5MailMessage]) -> Result<Vec<Stage5MailMessage>, String> {
    let mut normalized = Vec::new();
    merge_external_stage5_mail(&mut normalized, mail.to_vec())?;
    Ok(normalized)
}
fn without_mail(save: &CharacterSaveRecord) -> Result<serde_json::Value, String> {
    let mut save = save.clone();
    save.revision = 0;
    let mut state = systems(&save)?;
    state.mail.clear();
    save.stage5_systems_json = Some(serde_json::to_string(&state).map_err(|e| e.to_string())?);
    serde_json::to_value(save).map_err(|e| e.to_string())
}

fn inventory_from_canonical_source(
    world: &World,
    save: &CharacterSaveRecord,
) -> Result<InventoryResource, String> {
    validate_character_save_record(save)?;
    fn decode<T: serde::de::DeserializeOwned>(encoded: &[String]) -> Result<Vec<T>, String> {
        encoded
            .iter()
            .map(|value| serde_json::from_str(value).map_err(|error| error.to_string()))
            .collect()
    }
    let mut inventory = world.resource::<InventoryResource>().clone();
    inventory.inventory_capacity = save.inventory_capacity;
    inventory.inventory_items = decode(&save.inventory_items_json)?;
    inventory.belt_items = decode(&save.belt_items_json)?;
    inventory.storage_items = decode(&save.storage_items_json)?;
    inventory.equipment_items = decode(&save.equipment_items_json)?;
    inventory.reserved_item_unique_ids = super::super::item_custody::reserved_ids(&systems(save)?)?;
    let hero = super::super::hero_inventory::restored_hero_inventory(
        save,
        decode(&save.hero_inventory_items_json)?,
        decode(&save.hero_equipment_items_json)?,
    )?;
    inventory
        .reserved_item_unique_ids
        .extend(super::super::hero_inventory::validate_hero_custody(&hero)?);
    inventory
        .reserved_item_unique_ids
        .extend(super::super::procurement::reserved_ids(world)?);
    Ok(inventory)
}

/// Return a committed batch; no packet or live asset is published on failure.
/// A later mailbox-only update may join the plan without changing its assets.
pub(in crate::runtime) fn commit_claims(
    world: &mut World,
    mail_ids: &[u32],
) -> Result<MailClaimPlan, String> {
    let config = world.resource::<RuntimeConfigResource>().config.clone();
    config.ensure_account_store_writable()?;
    let account = active_session_mutating_account_id(world.resource::<SessionResource>())
        .ok_or("mail claim requires an account")?;
    config.refresh_account_store_account(&account)?;
    let active =
        snapshot_active_character_save(world).ok_or("mail claim requires an active character")?;
    let (canonical, card_ledger) = {
        let store = config
            .account_store
            .lock()
            .map_err(|_| "mail source lock poisoned")?;
        let record = store
            .accounts
            .get(&account)
            .ok_or("mail source account missing")?;
        (
            record
                .saves
                .get(&active.character.index)
                .cloned()
                .ok_or("mail source character missing")?,
            record.monthly_card.clone(),
        )
    };
    if canonical.character.index != active.character.index
        || canonical.character.name != active.character.name
        || canonical.character.class != active.character.class
        || canonical.character.gender != active.character.gender
    {
        return Err("mail claim character identity changed".into());
    }
    if active.revision > canonical.revision {
        return Err("mail claim source revision regressed".into());
    }
    let source_is_current = active.revision == canonical.revision;
    // A stale Session may address the same delivery, but must plan from the
    // latest complete source rather than revive its removed personal assets.
    let mut baseline = if source_is_current {
        active.clone()
    } else {
        canonical.clone()
    };
    merge_persisted_mail_into_character_save(&mut baseline, &canonical)?;
    let mut baseline_systems = systems(&baseline)?;
    let mut visible = world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .mail
        .clone();
    merge_external_stage5_mail(&mut visible, std::mem::take(&mut baseline_systems.mail))?;
    baseline_systems.mail = visible;
    let canonical_mail = normalized_mail(&systems(&canonical)?.mail)?;
    let touched: Vec<_> = mail_ids
        .iter()
        .map(|id| {
            baseline_systems
                .mail
                .iter()
                .find(|mail| mail.id == *id)
                .cloned()
                .ok_or("mail plan input missing")
        })
        .collect::<Result<_, _>>()?;
    let original_touched: Vec<_> = touched
        .iter()
        .map(|mail| {
            canonical_mail
                .iter()
                .find(|entry| entry.delivery_nonce == mail.delivery_nonce)
                .cloned()
        })
        .collect();
    if config.account_store_database_url.is_some()
        && config.account_store_database_mode == crate::AccountStoreDatabaseMode::SourceOfTruth
        && original_touched.iter().any(Option::is_none)
    {
        return Err("mail claim requires the canonical source delivery".into());
    }
    let durable = !matches!(
        config.item_uid_issuance,
        item_uid_issuance::ItemUidIssuance::Legacy
    ) || !matches!(
        world.resource::<InventoryResource>().item_uid_issuance,
        item_uid_issuance::ItemUidIssuance::Legacy
    );
    for (visible, stored) in touched.iter().zip(&original_touched).filter(|_| durable) {
        if let Some(stored) = stored {
            let mut payload = visible.clone();
            payload.id = stored.id;
            payload.opened = stored.opened;
            payload.locked = stored.locked;
            payload.claimed = stored.claimed;
            payload.deleted = stored.deleted;
            if &payload != stored {
                return Err("mail claim payload differs from canonical delivery".into());
            }
        }
    }
    let mut staged = World::new();
    staged.insert_resource(if source_is_current {
        world.resource::<InventoryResource>().clone()
    } else {
        inventory_from_canonical_source(world, &canonical)?
    });
    let mut staged_player = world.resource::<PlayerRuntimeResource>().clone();
    staged_player.gold = baseline.gold;
    staged_player.credit = baseline.credit;
    staged.insert_resource(staged_player);
    staged.insert_resource(Stage5SystemsResource {
        stage5_systems: baseline_systems.clone(),
    });
    staged.insert_resource(billing_monthly_card::trusted_claim_units(world)?);
    let mut plan = prepare_claims_with_authority(&staged, world, mail_ids).map_err(|error| {
        config
            .ensure_account_store_writable()
            .err()
            .unwrap_or_else(|| format!("mail claim rejected: {error:?}"))
    })?;
    let canonical_without_mail = without_mail(&canonical)?;
    let has_cards = plan.claim.items.iter().any(billing_monthly_card::is_card);
    let (committed_revision, committed_systems) =
        config.commit_account_store_transaction(&[account.clone()], |store| {
            if !plan.custody_ids.is_empty()
                && store_custody_conflicts(
                    store,
                    &account,
                    active.character.index,
                    &plan.custody_deliveries,
                    &plan.custody_ids,
                )?
            {
                return Err(
                    "mail claim existing custody conflicts with a current physical owner".into(),
                );
            }
            if item_uid_issuance::historical_uid_conflicts(
                UidHistoryRoot::Store(store),
                &plan.issued,
            )? {
                return Err("mail claim reserved UID conflicts with newer source history".into());
            }
            let record = store
                .accounts
                .get(&account)
                .ok_or("mail source account disappeared")?;
            if has_cards && record.monthly_card != card_ledger {
                return Err("mail card authorization changed after planning".into());
            }
            let current = record
                .saves
                .get(&active.character.index)
                .cloned()
                .ok_or("mail source disappeared")?;
            if current.revision < canonical.revision
                || without_mail(&current)? != canonical_without_mail
            {
                return Err("mail claim source changed after reservation".into());
            }
            let current_mail = normalized_mail(&systems(&current)?.mail)?;
            for (mail, expected) in touched.iter().zip(&original_touched) {
                if current_mail
                    .iter()
                    .find(|entry| entry.delivery_nonce == mail.delivery_nonce)
                    != expected.as_ref()
                {
                    return Err("mail claim delivery changed after reservation".into());
                }
            }
            let mut merged = baseline_systems.mail.clone();
            merge_external_stage5_mail(&mut merged, current_mail)?;
            for mail in &touched {
                let current = merged
                    .iter_mut()
                    .find(|entry| entry.delivery_nonce == mail.delivery_nonce)
                    .ok_or("mail claim delivery disappeared")?;
                if current != mail {
                    return Err("mail claim touched mailbox state changed".into());
                }
                current.claimed = true;
                current.gold = 0;
                current.items.clear();
                current.item_states_json.clear();
            }
            let mut save = baseline.clone();
            save.gold = plan.gold;
            save.credit = plan.credit;
            save.inventory_items_json = encode_state_vec(&plan.inventory.inventory_items);
            save.belt_items_json = encode_state_vec(&plan.inventory.belt_items);
            let mut state = systems(&save)?;
            state.mail = merged;
            save.stage5_systems_json =
                Some(serde_json::to_string(&state).map_err(|e| e.to_string())?);
            save.revision = current
                .revision
                .checked_add(1)
                .ok_or("mail claim revision overflow")?;
            validate_character_save_record(&save)?;
            let revision = save.revision;
            store
                .accounts
                .get_mut(&account)
                .ok_or("mail account disappeared")?
                .saves
                .insert(active.character.index, save);
            Ok((revision, state))
        })?;
    plan.systems = committed_systems;
    let session = world.resource::<SessionResource>();
    let revision_published = if source_is_current {
        session.advance_active_save_revision(active.revision, committed_revision)
    } else {
        // Other private business state is still stale. Keeping its revision
        // prevents a later ordinary full save from overwriting canonical state.
        session.active_save_revision() == Some(active.revision)
    };
    if !revision_published {
        return Err(config.freeze_committed_source_publication());
    }
    publish_claim(world, &plan);
    Ok(plan)
}

#[cfg(test)]
#[path = "mail_uid_planning_tests.rs"]
mod tests;
