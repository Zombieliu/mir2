//! Bounded Alice transactions. Physical jewellery reclassifies one real ore UID;
//! fashion is an independent wardrobe entry, never equipment or a stat source.
use super::components::current_player_is_dead;
use super::items::{
    crystal_item_bind_for_item_key, embedded_item_state_from_template,
    item_info_from_crystal_template, try_item_state_from_user_item, try_user_item_from_item_state,
    validate_committed_item_state_carrier, ItemState,
};
use super::npc_purchase_transaction::PreparedCheckpointMerge;
use super::resources::{
    is_in_world, InventoryResource, NpcStateResource, PlayerRuntimeResource, RuntimeConfigResource,
    SessionResource, Stage5SystemsResource,
};
use super::save::{
    active_session_mutating_account_id, encode_state_vec, merge_persisted_mail_into_character_save,
    snapshot_active_character_save, validate_character_save_record,
};
use super::session::SimulationSession;
use crate::{AccountStoreDatabaseMode, CharacterSaveRecord, ItemContainer, Stage5SystemsState};
use bevy_ecs::prelude::{Resource, World};
use mir2_game_data::{atelier as catalog, crystal_item_by_index};
use mir2_protocol::ServerPacket;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Resource)]
struct Enabled;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Error {
    Unavailable,
    InvalidRequest,
    Stale,
    Materials,
    Gold,
    AlreadyOwned,
    UnsupportedClass,
    NoChange,
    AuthorityRejected,
    OutcomeUnknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) enum Action {
    Craft { recipe: String },
    Wear { fashion: String },
    Clear,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Command {
    pub request_id: String,
    pub expected_revision: u64,
    pub action: Action,
}

#[derive(Debug)]
pub(super) struct Execution {
    pub packets: Vec<ServerPacket>,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Material {
    uid: u64,
    count: u32,
    template: i32,
    carrier: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Canonical {
    catalog_version: u16,
    account: String,
    character_index: i32,
    character_name: String,
    actor: String,
    expected_revision: u64,
    action: Action,
    gold: u32,
    materials: Vec<Material>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
enum Outcome {
    Jewellery { uid: u64, template: i32 },
    Unlocked { fashion: String },
    Selected { fashion: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Receipt {
    schema: u16,
    canonical: Canonical,
    committed_revision: u64,
    outcome: Outcome,
}

fn request_id(value: &Canonical) -> Result<String, String> {
    let bytes = serde_json::to_vec(&("mir2-alice-atelier-v1", value)).map_err(|e| e.to_string())?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

fn systems(save: &CharacterSaveRecord) -> Result<Stage5SystemsState, String> {
    save.stage5_systems_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map(|s| s.unwrap_or_default())
        .map_err(|e| e.to_string())
}

fn source(world: &World) -> Result<crate::SimulationConfig, Error> {
    let config = world.resource::<RuntimeConfigResource>().config.clone();
    let file = config.account_store_path.is_some() && config.account_store_database_url.is_none();
    let pg = config.account_store_path.is_none()
        && config.account_store_database_url.is_some()
        && config.account_store_database_mode == AccountStoreDatabaseMode::SourceOfTruth;
    if !file && !pg {
        return Err(Error::Unavailable);
    }
    config
        .ensure_account_store_writable()
        .map_err(|_| Error::AuthorityRejected)?;
    Ok(config)
}

pub(super) fn enabled(world: &World) -> bool {
    (world.contains_resource::<Enabled>()
        || std::env::var("MIR2_PLAYER_ATELIER").is_ok_and(|v| v == "1"))
        && source(world).is_ok()
}

pub(super) fn allowed(world: &World) -> bool {
    if !enabled(world) || !is_in_world(world) || current_player_is_dead(world) {
        return false;
    }
    let Some(dialog) = world
        .resource::<NpcStateResource>()
        .active_npc_dialog
        .as_ref()
    else {
        return false;
    };
    super::atelier_npc::approved_npc(world, dialog.npc_object_id)
        && super::npc::crystal_npc_object_in_data_range(world, dialog.npc_object_id)
}

fn identity(save: &CharacterSaveRecord, account: &str) -> Result<String, String> {
    let journal = save
        .npc_purchase_journal
        .as_ref()
        .ok_or("atelier requires durable actor")?;
    journal
        .validate_for(
            account,
            save.character.index,
            &save.character.name,
            save.revision,
        )
        .map_err(|e| e.to_string())?;
    Ok(journal.actor.iter().map(|b| format!("{b:02x}")).collect())
}

fn active(world: &World) -> Result<(String, CharacterSaveRecord), Error> {
    if !allowed(world) {
        return Err(Error::Unavailable);
    }
    let session = world.resource::<SessionResource>();
    if session.ranking_inspect_generation().is_none() {
        return Err(Error::Unavailable);
    }
    let account = active_session_mutating_account_id(session).ok_or(Error::Unavailable)?;
    let save = snapshot_active_character_save(world).ok_or(Error::Unavailable)?;
    validate_character_save_record(&save).map_err(|_| Error::AuthorityRejected)?;
    identity(&save, &account).map_err(|_| Error::AuthorityRejected)?;
    Ok((account, save))
}

fn plain(item: &ItemState, character: i32) -> bool {
    let Ok(wire) = try_user_item_from_item_state(item) else {
        return false;
    };
    matches!(item.container, ItemContainer::Bag1 | ItemContainer::Bag2)
        && item.unique_id != 0
        && item.quantity > 0
        && item.equip_slot.is_none()
        && item.stone_serial.is_none()
        && item.socketed.is_empty()
        && wire.slots.is_empty()
        && wire.added_stats.is_empty()
        && wire.awake_values.is_empty()
        && wire.awake_type == 0
        && wire.refined_value == 0
        && wire.refine_added == 0
        && wire.refine_success_chance == 0
        && wire.gem_count == 0
        && wire.wedding_ring == -1
        && !wire.cursed
        && wire.rental_information.is_none()
        && wire.expire_info.is_none()
        && wire.sealed_info.is_none()
        && item.rental_binding_flags == 0
        && !item.rental_locked
        && item.rental_owner_name.is_empty()
        && item.rental_expiry_binary_datetime == 0
        && item.sealed_expiry_time_binary_datetime == 0
        && item.sealed_next_time_binary_datetime == 0
        && (wire.soul_bound_id == -1 || wire.soul_bound_id == character)
        && validate_committed_item_state_carrier(item).is_ok()
}

fn canonical(
    world: &World,
    account: &str,
    save: &CharacterSaveRecord,
    action: Action,
) -> Result<Canonical, Error> {
    let mut value = Canonical {
        catalog_version: catalog::CATALOG_VERSION,
        account: account.into(),
        character_index: save.character.index,
        character_name: save.character.name.clone(),
        actor: identity(save, account).map_err(|_| Error::AuthorityRejected)?,
        expected_revision: save.revision,
        action,
        gold: 0,
        materials: Vec::new(),
    };
    let state = &world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .appearance;
    match &value.action {
        Action::Craft { recipe } => {
            let recipe = catalog::recipe(recipe).ok_or(Error::InvalidRequest)?;
            if !recipe.supports_class(save.character.class) {
                return Err(Error::UnsupportedClass);
            }
            if let catalog::Product::Fashion(id) = recipe.product {
                if state.owned_fashions.contains(id) {
                    return Err(Error::AlreadyOwned);
                }
            }
            let config = &world.resource::<RuntimeConfigResource>().config;
            if let catalog::Product::Jewellery(index) = recipe.product {
                let template = crystal_item_by_index(index).ok_or(Error::Unavailable)?;
                if !config.item_is_allowed(&template.name) {
                    return Err(Error::Unavailable);
                }
            }
            if save.gold < recipe.gold {
                return Err(Error::Gold);
            }
            value.gold = recipe.gold;
            let inventory = world.resource::<InventoryResource>();
            let mut used = BTreeSet::new();
            for input in recipe.ingredients {
                let template =
                    crystal_item_by_index(input.template_id).ok_or(Error::Unavailable)?;
                if !config.item_is_allowed(&template.name) {
                    return Err(Error::Unavailable);
                }
                let mut remaining = input.count;
                let mut candidates: Vec<_> = inventory
                    .inventory_items
                    .iter()
                    .filter(|item| {
                        !inventory.reserved_item_unique_ids.contains(&item.unique_id)
                            && plain(item, save.character.index)
                            && try_user_item_from_item_state(item)
                                .is_ok_and(|w| w.item_index == input.template_id)
                    })
                    .collect();
                candidates.sort_by_key(|item| item.unique_id);
                for item in candidates {
                    if remaining == 0 {
                        break;
                    }
                    if !used.insert(item.unique_id) {
                        continue;
                    }
                    let count = remaining.min(item.quantity);
                    value.materials.push(Material {
                        uid: item.unique_id,
                        count,
                        template: input.template_id,
                        carrier: serde_json::to_string(item).map_err(|_| Error::InvalidRequest)?,
                    });
                    remaining -= count;
                }
                if remaining != 0 {
                    return Err(Error::Materials);
                }
            }
        }
        Action::Wear { fashion } => {
            let style = catalog::fashion(fashion).ok_or(Error::InvalidRequest)?;
            if !state.owned_fashions.contains(fashion) {
                return Err(Error::InvalidRequest);
            }
            if !style.supports_class(save.character.class) {
                return Err(Error::UnsupportedClass);
            }
            if state.selected_fashion.as_ref() == Some(fashion) {
                return Err(Error::NoChange);
            }
        }
        Action::Clear => {
            if state.selected_fashion.is_none() {
                return Err(Error::NoChange);
            }
        }
    }
    Ok(value)
}

fn validate_canonical(value: &Canonical) -> Result<(), String> {
    if value.catalog_version != catalog::CATALOG_VERSION
        || value.account.is_empty()
        || value.character_name.is_empty()
        || value.character_index < 0
        || value.expected_revision == 0
        || value.actor.len() != 64
        || !value
            .actor
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("invalid atelier operation identity".into());
    }
    match &value.action {
        Action::Craft { recipe } => {
            let recipe = catalog::recipe(recipe).ok_or("unknown atelier recipe")?;
            if value.gold != recipe.gold || value.materials.len() > 8 {
                return Err("invalid atelier fee/materials".into());
            }
            let mut seen = BTreeSet::new();
            for material in &value.materials {
                let item: ItemState =
                    serde_json::from_str(&material.carrier).map_err(|e| e.to_string())?;
                if !plain(&item, value.character_index)
                    || item.unique_id != material.uid
                    || material.count == 0
                    || material.count > item.quantity
                    || !seen.insert(material.uid)
                    || !try_user_item_from_item_state(&item)
                        .is_ok_and(|w| w.item_index == material.template)
                    || !recipe
                        .ingredients
                        .iter()
                        .any(|input| input.template_id == material.template)
                {
                    return Err("invalid atelier material custody".into());
                }
            }
            for input in recipe.ingredients {
                let total = value
                    .materials
                    .iter()
                    .filter(|m| m.template == input.template_id)
                    .try_fold(0u32, |sum, m| {
                        sum.checked_add(m.count).ok_or("atelier quantity overflow")
                    })?;
                if total != input.count {
                    return Err("atelier material count differs".into());
                }
            }
        }
        Action::Wear { fashion } => {
            if catalog::fashion(fashion).is_none() || value.gold != 0 || !value.materials.is_empty()
            {
                return Err("invalid atelier wear operation".into());
            }
        }
        Action::Clear => {
            if value.gold != 0 || !value.materials.is_empty() {
                return Err("invalid atelier clear operation".into());
            }
        }
    }
    Ok(())
}

/// Validate persisted private receipts before accepting any checkpoint. Public
/// snapshots separately omit this ledger; old evicted operations remain CAS fenced.
pub(super) fn validate_receipts(save: &CharacterSaveRecord) -> Result<(), String> {
    let state = systems(save)?;
    super::fashion::validate_appearance(&state.appearance, save.character.class)?;
    if state.appearance.atelier_receipts.len() > catalog::RECEIPT_LIMIT {
        return Err("atelier receipt limit exceeded".into());
    }
    let mut revisions = BTreeSet::new();
    for (key, encoded) in &state.appearance.atelier_receipts {
        if encoded.len() > 32768 {
            return Err("atelier receipt exceeds bound".into());
        }
        let receipt: Receipt = serde_json::from_str(encoded).map_err(|e| e.to_string())?;
        let value = &receipt.canonical;
        validate_canonical(value)?;
        if receipt.schema != 1
            || request_id(value)? != *key
            || value.character_index != save.character.index
            || value.character_name != save.character.name
            || identity(save, &value.account)? != value.actor
            || value.expected_revision.checked_add(1) != Some(receipt.committed_revision)
            || receipt.committed_revision > save.revision
            || !revisions.insert(receipt.committed_revision)
        {
            return Err("atelier receipt identity/revision differs".into());
        }
        let valid = match (&value.action, &receipt.outcome) {
            (Action::Craft { recipe }, Outcome::Jewellery { uid, template }) => {
                catalog::recipe(recipe)
                    .is_some_and(|r| r.product == catalog::Product::Jewellery(*template))
                    && value.materials.first().is_some_and(|m| m.uid == *uid)
            }
            (Action::Craft { recipe }, Outcome::Unlocked { fashion }) => {
                catalog::recipe(recipe).is_some_and(
                    |r| matches!(r.product, catalog::Product::Fashion(id) if id == fashion.as_str()),
                ) && state.appearance.owned_fashions.contains(fashion)
            }
            (Action::Wear { fashion }, Outcome::Selected { fashion: selected }) => {
                selected.as_ref() == Some(fashion)
                    && state.appearance.owned_fashions.contains(fashion)
            }
            (Action::Clear, Outcome::Selected { fashion: None }) => true,
            _ => false,
        };
        if !valid {
            return Err("atelier receipt outcome differs".into());
        }
    }
    Ok(())
}

fn reclassify(input: &ItemState, template_id: i32) -> Result<ItemState, Error> {
    let original = try_user_item_from_item_state(input).map_err(|_| Error::InvalidRequest)?;
    let template = crystal_item_by_index(template_id).ok_or(Error::Unavailable)?;
    if input.quantity != 1
        || template.stack_size != 1
        || template.slots != 0
        || template.bind != crystal_item_bind_for_item_key(&input.key)
    {
        return Err(Error::InvalidRequest);
    }
    let base = embedded_item_state_from_template(&template, input.container, input.slot);
    let mut wire = try_user_item_from_item_state(&base).map_err(|_| Error::InvalidRequest)?;
    wire.unique_id = original.unique_id;
    wire.soul_bound_id = original.soul_bound_id;
    wire.is_shop_item = original.is_shop_item;
    wire.gm_made = original.gm_made;
    let result = try_item_state_from_user_item(base, &wire).map_err(|_| Error::InvalidRequest)?;
    validate_committed_item_state_carrier(&result).map_err(|_| Error::InvalidRequest)?;
    Ok(result)
}

impl SimulationSession {
    fn prepare_atelier(&mut self) -> Result<(String, CharacterSaveRecord), Error> {
        if !allowed(self.app.world()) {
            return Err(Error::Unavailable);
        }
        if self
            .app
            .world()
            .resource::<SessionResource>()
            .npc_purchase_journal
            .is_none()
        {
            self.begin_npc_purchase_producer()
                .map_err(|_| Error::AuthorityRejected)?;
        }
        active(self.app.world())
    }

    pub(super) fn quote_atelier(&mut self, action: Action) -> Result<Command, Error> {
        let (account, save) = self.prepare_atelier()?;
        let value = canonical(self.app.world(), &account, &save, action.clone())?;
        validate_canonical(&value).map_err(|_| Error::InvalidRequest)?;
        Ok(Command {
            request_id: request_id(&value).map_err(|_| Error::InvalidRequest)?,
            expected_revision: save.revision,
            action,
        })
    }

    pub(super) fn execute_atelier(&mut self, command: Command) -> Result<Execution, Error> {
        let (account, active) = self.prepare_atelier()?;
        let config = source(self.app.world())?;
        config
            .refresh_account_store_account(&account)
            .map_err(|_| Error::AuthorityRejected)?;
        let durable = {
            let store = config
                .account_store
                .lock()
                .map_err(|_| Error::AuthorityRejected)?;
            store
                .accounts
                .get(&account)
                .and_then(|a| a.saves.get(&active.character.index))
                .cloned()
                .ok_or(Error::Unavailable)?
        };
        validate_character_save_record(&durable).map_err(|_| Error::AuthorityRejected)?;
        if identity(&durable, &account).ok() != identity(&active, &account).ok() {
            return Err(Error::Stale);
        }
        if let Some(encoded) = systems(&durable)
            .map_err(|_| Error::AuthorityRejected)?
            .appearance
            .atelier_receipts
            .get(&command.request_id)
        {
            let receipt: Receipt =
                serde_json::from_str(encoded).map_err(|_| Error::AuthorityRejected)?;
            if receipt.canonical.action != command.action
                || receipt.canonical.expected_revision != command.expected_revision
            {
                return Err(Error::InvalidRequest);
            }
            return Ok(Execution {
                packets: Vec::new(),
                replayed: true,
            });
        }
        if command.expected_revision != active.revision || durable.revision != active.revision {
            return Err(Error::Stale);
        }
        let value = canonical(self.app.world(), &account, &active, command.action.clone())?;
        validate_canonical(&value).map_err(|_| Error::InvalidRequest)?;
        if request_id(&value).map_err(|_| Error::InvalidRequest)? != command.request_id {
            return Err(Error::InvalidRequest);
        }
        let mut inventory = self.app.world().resource::<InventoryResource>().clone();
        let mut state = systems(&active).map_err(|_| Error::AuthorityRejected)?;
        let gold = active.gold.checked_sub(value.gold).ok_or(Error::Gold)?;
        let mut packets = Vec::new();
        let outcome = match &value.action {
            Action::Craft { recipe } => {
                let recipe = catalog::recipe(recipe).ok_or(Error::InvalidRequest)?;
                let mut replacement = None;
                if let catalog::Product::Jewellery(template) = recipe.product {
                    let first = value.materials.first().ok_or(Error::Materials)?;
                    let item = inventory
                        .inventory_items
                        .iter()
                        .find(|i| i.unique_id == first.uid)
                        .ok_or(Error::Materials)?;
                    replacement = Some(reclassify(item, template)?);
                }
                for material in &value.materials {
                    let index = inventory
                        .inventory_items
                        .iter()
                        .position(|i| i.unique_id == material.uid)
                        .ok_or(Error::Materials)?;
                    let row = &mut inventory.inventory_items[index];
                    if serde_json::to_string(row).map_err(|_| Error::InvalidRequest)?
                        != material.carrier
                    {
                        return Err(Error::Stale);
                    }
                    row.quantity = row
                        .quantity
                        .checked_sub(material.count)
                        .ok_or(Error::Materials)?;
                    if row.quantity == 0 {
                        inventory.inventory_items.remove(index);
                    }
                    packets.push(ServerPacket::DeleteItem {
                        unique_id: material.uid,
                        count: u16::try_from(material.count).map_err(|_| Error::InvalidRequest)?,
                    });
                }
                match recipe.product {
                    catalog::Product::Jewellery(template) => {
                        let item = replacement.ok_or(Error::InvalidRequest)?;
                        let wire = try_user_item_from_item_state(&item)
                            .map_err(|_| Error::InvalidRequest)?;
                        let outcome = Outcome::Jewellery {
                            uid: item.unique_id,
                            template,
                        };
                        packets.insert(
                            0,
                            ServerPacket::NewItemInfo {
                                info: item_info_from_crystal_template(
                                    crystal_item_by_index(template).ok_or(Error::Unavailable)?,
                                ),
                            },
                        );
                        packets.push(ServerPacket::GainedItem { item: wire });
                        inventory.inventory_items.push(item);
                        // In-place replacement needs no new cell, but the whole
                        // postimage must still pass legal capacity/weight/custody.
                        let weights = super::player_weights::compute(
                            &inventory,
                            active.character.level,
                            active.character.class,
                            false,
                        )
                        .ok_or(Error::InvalidRequest)?;
                        if u64::from(weights.bag)
                            > super::stats::player_stats(self.app.world())
                                .bag_weight()
                                .max(0) as u64
                        {
                            return Err(Error::Materials);
                        }
                        outcome
                    }
                    catalog::Product::Fashion(id) => {
                        if !state.appearance.owned_fashions.insert(id.into()) {
                            return Err(Error::AlreadyOwned);
                        }
                        Outcome::Unlocked { fashion: id.into() }
                    }
                }
            }
            Action::Wear { fashion } => {
                state.appearance.selected_fashion = Some(fashion.clone());
                Outcome::Selected {
                    fashion: Some(fashion.clone()),
                }
            }
            Action::Clear => {
                state.appearance.selected_fashion = None;
                Outcome::Selected { fashion: None }
            }
        };
        if value.gold != 0 {
            packets.push(ServerPacket::LoseGold { gold: value.gold });
        }
        let revision = active
            .revision
            .checked_add(1)
            .ok_or(Error::AuthorityRejected)?;
        let receipt = Receipt {
            schema: 1,
            canonical: value.clone(),
            committed_revision: revision,
            outcome,
        };
        state.appearance.atelier_receipts.insert(
            command.request_id.clone(),
            serde_json::to_string(&receipt).map_err(|_| Error::InvalidRequest)?,
        );
        while state.appearance.atelier_receipts.len() > catalog::RECEIPT_LIMIT {
            let oldest = state
                .appearance
                .atelier_receipts
                .iter()
                .map(|(key, json)| {
                    serde_json::from_str::<Receipt>(json)
                        .map(|r| (r.committed_revision, key.clone()))
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| Error::AuthorityRejected)?
                .into_iter()
                .min()
                .ok_or(Error::AuthorityRejected)?
                .1;
            state.appearance.atelier_receipts.remove(&oldest);
        }
        let mut save = active.clone();
        save.gold = gold;
        save.inventory_items_json = encode_state_vec(&inventory.inventory_items);
        save.stage5_systems_json =
            Some(serde_json::to_string(&state).map_err(|_| Error::InvalidRequest)?);
        save.revision = revision;
        validate_character_save_record(&save).map_err(|_| Error::InvalidRequest)?;
        let commit = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            super::shared_guild_experience::commit_source(
                self.app.world(),
                &config,
                &[account.clone()],
                |store| {
                    let record = store
                        .accounts
                        .get(&account)
                        .ok_or("atelier account disappeared")?;
                    if !record.characters.iter().any(|c| {
                        c.index == active.character.index && c.name == active.character.name
                    }) {
                        return Err("atelier roster changed".into());
                    }
                    let durable = record
                        .saves
                        .get(&active.character.index)
                        .ok_or("atelier save disappeared")?;
                    if durable.revision != active.revision
                        || identity(durable, &account)? != value.actor
                    {
                        return Err("atelier checkpoint advanced".into());
                    }
                    for material in &value.materials {
                        let expected: serde_json::Value =
                            serde_json::from_str(&material.carrier).map_err(|e| e.to_string())?;
                        let matches = durable
                            .inventory_items_json
                            .iter()
                            .filter_map(|text| serde_json::from_str::<serde_json::Value>(text).ok())
                            .filter(|item| {
                                item.get("unique_id").and_then(|v| v.as_u64()) == Some(material.uid)
                            })
                            .collect::<Vec<_>>();
                        if matches.len() != 1 || matches[0] != expected {
                            return Err("atelier durable material custody differs".into());
                        }
                    }
                    merge_persisted_mail_into_character_save(&mut save, durable)?;
                    validate_character_save_record(&save)?;
                    let publication = PreparedCheckpointMerge::new(self.app.world(), &save)?;
                    store
                        .accounts
                        .get_mut(&account)
                        .ok_or("atelier account disappeared")?
                        .saves
                        .insert(save.character.index, save.clone());
                    Ok(publication)
                },
            )
        }));
        let publication = match commit {
            Ok(Ok(publication)) => publication,
            Ok(Err(_)) if config.ensure_account_store_writable().is_ok() => {
                return Err(Error::AuthorityRejected)
            }
            _ => return Err(Error::OutcomeUnknown),
        };
        let committed = systems(&save)
            .map_err(|_| Error::OutcomeUnknown)?
            .appearance;
        *self.app.world_mut().resource_mut::<InventoryResource>() = inventory;
        self.app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .gold = gold;
        publication.apply(self.app.world_mut());
        self.app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .appearance = committed;
        if !self
            .app
            .world_mut()
            .resource_mut::<SessionResource>()
            .advance_active_save_revision(active.revision, revision)
        {
            return Err(Error::OutcomeUnknown);
        }
        if matches!(command.action, Action::Wear { .. } | Action::Clear) {
            if let Some(appearance) = super::packets::self_zone_player_appearance(self.app.world())
            {
                if let Some(packet) =
                    super::packets::self_player_update_packet(self.app.world(), appearance.light)
                {
                    packets.push(packet);
                }
            }
        }
        Ok(Execution {
            packets,
            replayed: false,
        })
    }
}

#[cfg(test)]
#[path = "atelier_tests.rs"]
mod tests;
