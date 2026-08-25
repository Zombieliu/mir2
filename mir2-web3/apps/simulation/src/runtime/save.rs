use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use sha2::{Digest, Sha256};

use bevy_ecs::prelude::World;
use mir2_game_data::{crystal_item_by_name, format_localized_text, localized_text_or_fallback};
use mir2_protocol::{ChatType, ClientPacket, MirDirection, Point, ServerPacket};
use serde::{Deserialize, Serialize};

use crate::config::{
    apply_crystal_map_metadata, crystal_base_vitals, AccountBanStatus, AccountRecord,
    AccountSourceRefreshOutcome, CharacterRecord, CharacterSaveRecord, ItemContainer,
    SimulationConfig, Stage5MailMessage, Stage5SystemsState,
};

use super::components::{
    entity_facing, entity_player_vitals, entity_position, player_entity, PlayerVitals,
};
use super::crystal_compat::BASE_STORAGE_SLOTS;
use super::equipment::{
    equipment_state_from_item_state, item_state_from_equipment_state,
    refresh_mount_resource_from_equipment, seed_equipment_items_for_character, EquipmentState,
};
use super::inventory::{
    crystal_start_inventory_items, normalize_inventory_known_item_metadata,
    normalize_inventory_unique_ids, refresh_storage_password_state, seed_belt_items,
    seed_inventory_items, seed_storage_items,
};
use super::items::{
    crystal_item_key_for_template, embedded_item_state_from_template,
    validate_committed_item_state_carrier, validate_committed_user_item_carrier, ItemState,
};
use super::map::{
    clear_non_player_world_entities, rebuild_world, refresh_runtime_map_collision,
    runtime_world_map_collision_data, should_use_crystal_current_map_world,
    spawn_config_visible_npcs, spawn_visible_world_for_current_map,
};
use super::npc::{current_crystal_npc_local_time, NpcBuyBackState, NpcUsedGoodsState};
use super::packets::*;
use super::quests::{effective_crystal_quest_info_packets, QuestState};
use super::resources::{
    current_language, runtime_tick, set_runtime_tick, BuffResource, HeroInventoryResource,
    InventoryResource, ItemRentalResource, MapRuntimeResource, NpcStateResource,
    ObjectIdAllocatorResource, PlayerPermissionResource, PlayerRuntimeResource,
    PotionRecoveryResource, QuestResource, RuntimeConfigResource, RuntimeQueueResource,
    SessionResource, SkillResource, Stage5SystemsResource,
};
use super::session::SimulationSession;
use super::skills::seed_skills;
use super::stage5::{
    merge_native_game_shop_ledger_mail, validate_stage5_mail_item_carriers,
    validate_stage5_systems_item_carriers,
};

#[derive(Debug, Clone)]
pub(super) struct ActiveCharacterRuntimeState {
    pub(super) position: Point,
    pub(super) direction: MirDirection,
    pub(super) vitals: PlayerVitals,
}

pub(super) fn default_save_for_character(
    config: &SimulationConfig,
    character: CharacterRecord,
) -> CharacterSaveRecord {
    let starter_equipment = if config.content_profile.is_some() {
        Vec::new()
    } else {
        seed_equipment_items_for_character(character.class, character.gender)
    };
    let mut save = CharacterSaveRecord::new(character);
    let starter_inventory = if config.content_profile.is_some() {
        crystal_start_inventory_items(&save.character)
    } else {
        Vec::new()
    };
    let (max_hp, mp) = crystal_base_vitals(save.character.class, save.character.level);
    save.position = config.spawn.clone();
    save.map_file_name = config.map.file_name.clone();
    save.map_title = config.map.title.clone();
    save.direction = MirDirection::Down;
    save.hp = max_hp;
    save.max_hp = max_hp;
    save.mp = mp;
    save.max_mp = mp;
    save.experience = 0;
    save.max_experience = config.experience_required_for_level(save.character.level);
    save.gold = 0;
    save.credit = 0;
    save.city_currencies.clear();
    save.inventory_items_json = encode_state_vec(&starter_inventory);
    save.belt_items_json = Vec::new();
    save.storage_items_json = Vec::new();
    save.equipment_items_json = encode_state_vec(&starter_equipment);
    save.equipment_items_explicit_empty = config.content_profile.is_some();
    save.quest_states_json = Vec::new();
    save.skill_states_json = Vec::new();
    save.npc_flag_states_json = Vec::new();
    save.npc_saved_values_json = Vec::new();
    save.npc_buy_back_items_json = Vec::new();
    save.npc_used_goods_items_json = Vec::new();
    save.item_rental_records_json = Vec::new();
    save.has_rented_item = false;
    save.stage5_systems_json = Some(
        serde_json::to_string(&Stage5SystemsState::default())
            .expect("stage5 systems state should serialize"),
    );
    save
}

pub(super) fn active_character_runtime_state(world: &World) -> Option<ActiveCharacterRuntimeState> {
    let player = player_entity(world)?;
    Some(ActiveCharacterRuntimeState {
        position: entity_position(world, player)?,
        direction: entity_facing(world, player)?,
        vitals: entity_player_vitals(world, player)?,
    })
}

pub(super) fn encode_state_vec<T>(items: &[T]) -> Vec<String>
where
    T: Serialize,
{
    items
        .iter()
        .map(|item| serde_json::to_string(item).expect("save state should serialize"))
        .collect()
}

pub(super) fn decode_state_vec<T>(items: &[String]) -> Option<Vec<T>>
where
    T: for<'de> Deserialize<'de>,
{
    items
        .iter()
        .map(|item| serde_json::from_str(item).ok())
        .collect()
}

pub(super) fn snapshot_active_character_save(world: &World) -> Option<CharacterSaveRecord> {
    let resources = world.resource::<InventoryResource>();
    let hero_inventory = world.resource::<HeroInventoryResource>();
    let player_runtime = world.resource::<PlayerRuntimeResource>();
    let map = world.resource::<MapRuntimeResource>();
    let quests = world.resource::<QuestResource>();
    let skills = world.resource::<SkillResource>();
    let npc_state = world.resource::<NpcStateResource>();
    let rental = world.resource::<ItemRentalResource>();
    let stage5 = world.resource::<Stage5SystemsResource>();
    let session = world.resource::<SessionResource>();
    let character = session.selected_character.clone()?;
    let revision = session.active_save_revision()?;
    let player = player_entity(world)?;
    let position = entity_position(world, player)?;
    let direction = entity_facing(world, player)?;
    let vitals = entity_player_vitals(world, player)?;

    Some(CharacterSaveRecord {
        revision,
        character,
        map_file_name: map.current_map.file_name.clone(),
        map_title: map.current_map.title.clone(),
        position,
        direction,
        hp: vitals.hp,
        max_hp: vitals.max_hp,
        mp: vitals.mp,
        max_mp: vitals.max_mp,
        experience: player_runtime.experience,
        max_experience: player_runtime.max_experience.max(1),
        gold: player_runtime.gold,
        credit: player_runtime.credit,
        city_currencies: player_runtime.city_currencies.clone(),
        pk_points: player_runtime.pk_points,
        chat_banned: player_runtime.chat_banned,
        chat_ban_until_ms: player_runtime.chat_ban_until_ms,
        inventory_items_json: encode_state_vec(&resources.inventory_items),
        belt_items_json: encode_state_vec(&resources.belt_items),
        hero_inventory_items_json: encode_state_vec(&hero_inventory.items),
        storage_items_json: encode_state_vec(&resources.storage_items),
        equipment_items_json: encode_state_vec(&resources.equipment_items),
        equipment_items_explicit_empty: resources.equipment_items.is_empty(),
        quest_states_json: encode_state_vec(&quests.quests),
        skill_states_json: encode_state_vec(&skills.skills),
        npc_flag_states_json: encode_state_vec(&npc_state.npc_flags),
        npc_saved_values_json: encode_state_vec(&npc_state.npc_saved_values),
        npc_buy_back_items_json: encode_state_vec(&npc_state.npc_buy_back_items),
        npc_used_goods_items_json: encode_state_vec(&npc_state.npc_used_goods_items),
        item_rental_records_json: encode_state_vec(&rental.rented_items),
        has_rented_item: rental.has_rented_item,
        stage5_systems_json: Some(
            serde_json::to_string(&stage5.stage5_systems)
                .expect("stage5 systems state should serialize"),
        ),
    })
}

pub(super) fn active_session_mutating_account_id(session: &SessionResource) -> Option<String> {
    let account_id = session.account_id.as_ref()?;
    if account_id.is_empty() || account_id.as_str() != account_id.trim() {
        return None;
    }
    Some(account_id.clone())
}

fn exact_character_identity_matches(
    character: &CharacterRecord,
    expected: &CharacterRecord,
) -> bool {
    character.index == expected.index && character.name == expected.name
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CrystalCharacterSelectState {
    Authenticated { account_id: String },
    Unauthenticated,
    InGame,
}

/// Crystal dispatches character creation, deletion, and StartGame only while
/// the connection is in Select. Preserve that stage check before considering
/// whether an account is bound, then require the exact canonical account key.
pub(super) fn crystal_character_select_state(
    session: &SessionResource,
) -> CrystalCharacterSelectState {
    if session.selected_character.is_some() {
        return CrystalCharacterSelectState::InGame;
    }
    match active_session_mutating_account_id(session) {
        Some(account_id) => CrystalCharacterSelectState::Authenticated { account_id },
        None => CrystalCharacterSelectState::Unauthenticated,
    }
}

pub(super) fn persist_active_character_save(world: &World) -> Result<(), String> {
    let Some(save) = snapshot_active_character_save(world) else {
        return Ok(());
    };
    let session = world.resource::<SessionResource>();
    let Some(account_id) = active_session_mutating_account_id(session) else {
        return Err(
            "refusing to persist active character without an authenticated account identity"
                .to_string(),
        );
    };
    let active_character = session.selected_character.as_ref().ok_or_else(|| {
        "refusing to persist active character without a selected character".to_string()
    })?;
    if !exact_character_identity_matches(&save.character, active_character) {
        return Err("active character full-save snapshot identity mismatch".to_string());
    }
    let expected_revision = save.revision;
    match persist_character_save(world, &account_id, save)? {
        PersistCharacterSaveResult::Full(committed_revision) => {
            if !world
                .resource::<SessionResource>()
                .advance_active_save_revision(expected_revision, committed_revision)
            {
                return Err(
                    "active character revision changed while completing full-save CAS".to_string(),
                );
            }
            Ok(())
        }
        PersistCharacterSaveResult::StaleMailStatusOnly => Err(
            "stale full character save rejected after preserving safe mailbox status deltas"
                .to_string(),
        ),
    }
}

/// Commit a mutation of the active character's durable private state before
/// exposing the corresponding change through the live World mirror.
///
/// Mail-producing economy paths use this helper so the mailbox ID allocation
/// and its currency debit share the account-store transaction.  The active
/// snapshot is merged with mail delivered by other sessions while the store is
/// locked, then persisted exactly once.  Callers must update World resources
/// only after this function returns `Ok`.
pub(super) fn commit_active_character_save_transaction<T, F>(
    world: &World,
    transaction: F,
) -> Result<T, String>
where
    F: FnOnce(&mut CharacterSaveRecord) -> Result<T, String>,
{
    let config = world.resource::<RuntimeConfigResource>().config.clone();
    let session = world.resource::<SessionResource>();
    let account_id = active_session_mutating_account_id(session)
        .ok_or_else(|| "active character transaction requires an account identity".to_string())?;
    let active_character = session
        .selected_character
        .as_ref()
        .ok_or_else(|| "active character transaction requires a selected character".to_string())?
        .clone();
    let active_save = snapshot_active_character_save(world)
        .ok_or_else(|| "active character transaction requires an in-world snapshot".to_string())?;
    let expected_revision = active_save.revision;
    let touched_accounts = vec![account_id.clone()];

    let (result, baseline_revision, committed_revision) =
        config.commit_account_store_transaction(&touched_accounts, move |store| {
            let account = store
                .accounts
                .get(&account_id)
                .ok_or_else(|| "active character account changed before commit".to_string())?;
            let persisted_character = account
                .characters
                .iter()
                .find(|character| character.index == active_character.index)
                .ok_or_else(|| "active character changed before commit".to_string())?;
            if !exact_character_identity_matches(persisted_character, &active_character)
                || !exact_character_identity_matches(&active_save.character, &active_character)
            {
                return Err("active character transaction identity mismatch".to_string());
            }
            let persisted_save = account
                .saves
                .get(&active_character.index)
                .cloned()
                .ok_or_else(|| "active character save changed before commit".to_string())?;
            if !exact_character_identity_matches(&persisted_save.character, &active_character) {
                return Err(
                    "active character durable save identity mismatch before transaction"
                        .to_string(),
                );
            }
            let baseline_revision = persisted_save.revision;

            // A current session may atomically include its own unsaved World state.
            // A stale session must instead mutate the lock-held persisted baseline,
            // never its old full snapshot.
            let mut staged_save = if baseline_revision == expected_revision {
                let mut current = active_save;
                merge_persisted_mail_into_character_save(&mut current, &persisted_save)?;
                current
            } else {
                persisted_save
            };

            let result = transaction(&mut staged_save)?;
            migrate_legacy_candidate_save_record(&mut staged_save)?;
            validate_character_save_record(&staged_save)?;
            let committed_revision = baseline_revision
                .checked_add(1)
                .ok_or_else(|| "active character revision exhausted".to_string())?;
            staged_save.revision = committed_revision;
            store
                .accounts
                .get_mut(&account_id)
                .expect("validated active account should exist")
                .saves
                .insert(active_character.index, staged_save);
            Ok((result, baseline_revision, committed_revision))
        })?;

    if baseline_revision == expected_revision {
        world
            .resource::<SessionResource>()
            .advance_active_save_revision(expected_revision, committed_revision);
    }
    Ok(result)
}

impl SimulationSession {
    /// Capture the exact durable private state for the active character without
    /// mutating the configured account store.
    pub fn active_character_checkpoint(&self) -> Option<CharacterSaveRecord> {
        snapshot_active_character_save(self.app.world())
    }

    /// Restore the active character's durable private state after journal
    /// replay. Shared Zone state is restored separately by the Gateway and
    /// remains authoritative for map entities, position, and vitals.
    pub fn restore_active_character_checkpoint(
        &mut self,
        save: &CharacterSaveRecord,
    ) -> Result<(), String> {
        let identity = self
            .active_identity()
            .ok_or_else(|| "active character checkpoint requires an active identity".to_string())?;
        if identity.character_index != save.character.index
            || identity.character_name != save.character.name
        {
            return Err(format!(
                "active character checkpoint identity mismatch: runtime={}/{}, checkpoint={}/{}",
                identity.character_index,
                identity.character_name,
                save.character.index,
                save.character.name
            ));
        }

        let replay_tick = runtime_tick(self.app.world());
        apply_character_save(self.app.world_mut(), save)?;
        refresh_runtime_map_collision(self.app.world_mut());
        refresh_storage_password_state(self.app.world_mut());
        rebuild_world(self.app.world_mut());
        if should_use_crystal_current_map_world(self.app.world()) {
            clear_non_player_world_entities(self.app.world_mut());
            spawn_visible_world_for_current_map(self.app.world_mut());
            spawn_config_visible_npcs(self.app.world_mut());
        }
        self.visible_objects = collect_visible_objects(self.app.world())
            .keys()
            .copied()
            .collect();
        set_runtime_tick(self.app.world_mut(), replay_tick);
        Ok(())
    }
}

#[cfg(test)]
#[path = "save_fail_closed_tests.rs"]
mod save_fail_closed_tests;

pub(super) fn refresh_active_external_mail(world: &mut World) -> bool {
    let (config, account_id, selected_character) = {
        let config = world.resource::<RuntimeConfigResource>().config.clone();
        let session = world.resource::<SessionResource>();
        let Some(account_id) = active_session_mutating_account_id(session) else {
            return false;
        };
        let Some(character) = session.selected_character.as_ref() else {
            return false;
        };
        (config, account_id, character.clone())
    };

    let external_mail = {
        let Ok(store) = config.account_store.lock() else {
            return false;
        };
        let Some(account) = store.accounts.get(&account_id) else {
            return false;
        };
        if !account
            .characters
            .iter()
            .any(|character| exact_character_identity_matches(character, &selected_character))
        {
            return false;
        }
        let Some(save) = account.saves.get(&selected_character.index) else {
            return false;
        };
        if !exact_character_identity_matches(&save.character, &selected_character) {
            return false;
        }
        match save.stage5_systems_json.as_deref() {
            Some(encoded) => {
                let systems = match serde_json::from_str::<Stage5SystemsState>(encoded) {
                    Ok(systems) => systems,
                    Err(error) => {
                        eprintln!("failed to decode externally delivered stage5 mail: {error}");
                        return false;
                    }
                };
                if let Err(error) = validate_stage5_systems_item_carriers(&systems) {
                    eprintln!("rejected externally delivered stage5 mail: {error}");
                    return false;
                }
                systems.mail
            }
            None => Vec::new(),
        }
    };

    if external_mail.is_empty() {
        return false;
    }

    let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
    match merge_external_stage5_mail(&mut stage5.stage5_systems.mail, external_mail) {
        Ok(changed) => changed,
        Err(error) => {
            eprintln!("failed to refresh externally delivered mail: {error}");
            false
        }
    }
}

pub(super) fn merge_external_stage5_mail(
    local_mail: &mut Vec<Stage5MailMessage>,
    external_mail: Vec<Stage5MailMessage>,
) -> Result<bool, String> {
    validate_stage5_mail_item_carriers(local_mail)?;
    validate_stage5_mail_item_carriers(&external_mail)?;

    let mut staged_mail = local_mail.clone();
    let changed = merge_external_stage5_mail_staged(&mut staged_mail, external_mail)?;
    validate_stage5_mail_item_carriers(&staged_mail)?;
    if changed {
        *local_mail = staged_mail;
    }
    Ok(changed)
}

fn merge_external_stage5_mail_staged(
    local_mail: &mut Vec<Stage5MailMessage>,
    mut external_mail: Vec<Stage5MailMessage>,
) -> Result<bool, String> {
    let mut changed = normalize_stage5_mail_delivery_nonces(local_mail)?;
    normalize_stage5_mail_delivery_nonces(&mut external_mail)?;
    let mut used_ids = local_mail
        .iter()
        .map(|mail| mail.id)
        .chain(external_mail.iter().map(|mail| mail.id))
        .collect::<BTreeSet<_>>();
    for mut external in external_mail {
        // Match the same delivery across the whole active mailbox before
        // looking at IDs. A previous refresh may already have re-keyed this
        // incoming durable entry; matching by immutable content makes repeated
        // refreshes idempotent while preserving the ID visible to the client.
        if let Some(local_index) = local_mail
            .iter()
            .position(|local| stage5_mail_same_delivery(local, &external))
        {
            let local = &mut local_mail[local_index];
            if let Some(ledger_changed) = merge_native_game_shop_ledger_mail(local, &external)? {
                changed |= ledger_changed;
                continue;
            }
            let external_claim_payload_is_authoritative =
                external.claimed && stage5_mail_payload_is_consumed(&external);
            // Mail content is immutable after delivery. The active copy is the
            // command input and must not be silently repaired from storage;
            // otherwise corrupt exact attachment JSON could bypass rejection.
            // The sole payload exception is a durably claimed entry whose
            // consumed payload must prevent a stale session from claiming it.
            let mut merged = local.clone();
            if external_claim_payload_is_authoritative {
                merged.gold = external.gold;
                merged.items = external.items.clone();
                merged.item_states_json = external.item_states_json.clone();
            }
            // Read/claim/delete are monotonic. Lock is explicitly reversible,
            // so the active local value wins when a live snapshot is merged
            // for refresh or save.
            let merged = Stage5MailMessage {
                id: local.id,
                opened: local.opened || external.opened,
                locked: local.locked,
                claimed: local.claimed || external.claimed,
                deleted: local.deleted || external.deleted,
                ..merged
            };
            if local != &merged {
                *local = merged;
                changed = true;
            }
        } else {
            if local_mail.iter().any(|local| local.id == external.id) {
                // Active IDs may already be referenced by in-flight client
                // Read/Claim/Delete packets. Keep the local ID stable and
                // deterministically re-key the not-yet-exposed incoming mail.
                external.id = next_available_mail_id(&used_ids).ok_or_else(|| {
                    "mail ID space exhausted while resolving collision".to_string()
                })?;
            }
            used_ids.insert(external.id);
            local_mail.push(external);
            changed = true;
        }
    }
    Ok(changed)
}

fn stage5_mail_same_delivery(local: &Stage5MailMessage, external: &Stage5MailMessage) -> bool {
    if !local.delivery_nonce.is_empty() || !external.delivery_nonce.is_empty() {
        return !local.delivery_nonce.is_empty() && local.delivery_nonce == external.delivery_nonce;
    }

    // Defensive legacy fallback. Normal merge inputs are upgraded first, but
    // if a caller ever compares two raw legacy entries, use only the stable
    // address/header identity. Payload and status fields change when a parcel
    // is read, locked or claimed and therefore cannot identify a delivery.
    local.id == external.id
        && local.from == external.from
        && local.to == external.to
        && local.subject == external.subject
        && local.body == external.body
}

fn normalize_stage5_mail_delivery_nonces(mail: &mut [Stage5MailMessage]) -> Result<bool, String> {
    let mut changed = false;
    for message in mail {
        if message.delivery_nonce.is_empty() {
            message.delivery_nonce = legacy_stage5_mail_delivery_nonce(message)?;
            changed = true;
        }
    }
    Ok(changed)
}

fn legacy_stage5_mail_delivery_nonce(mail: &Stage5MailMessage) -> Result<String, String> {
    // Legacy rows have no true delivery identity. Use the client-visible ID
    // plus immutable header fields so the identity remains stable after a
    // different session claims the parcel and clears gold/items. Two legacy
    // rows with the same ID and header are intentionally treated as one
    // delivery: where history is ambiguous, preventing a duplicate claim is
    // safer than preserving a possibly duplicated entry.
    let identity = serde_json::to_vec(&(mail.id, &mail.from, &mail.to, &mail.subject, &mail.body))
        .map_err(|error| format!("failed to encode legacy mail identity: {error}"))?;
    let digest = Sha256::digest(identity);
    let mut nonce = String::with_capacity(7 + digest.len() * 2);
    nonce.push_str("legacy-");
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut nonce, "{byte:02x}")
            .map_err(|error| format!("failed to format legacy mail identity: {error}"))?;
    }
    Ok(nonce)
}

fn stage5_mail_payload_is_consumed(mail: &Stage5MailMessage) -> bool {
    mail.gold == 0 && mail.items.is_empty() && mail.item_states_json.is_empty()
}

fn next_available_mail_id(used_ids: &BTreeSet<u32>) -> Option<u32> {
    let after_max = used_ids
        .iter()
        .next_back()
        .copied()
        .unwrap_or(0)
        .checked_add(1);
    if let Some(candidate) = after_max.filter(|candidate| !used_ids.contains(candidate)) {
        return Some(candidate);
    }
    (1..=u32::MAX).find(|candidate| !used_ids.contains(candidate))
}

pub(super) enum PersistCharacterSaveResult {
    Full(u64),
    StaleMailStatusOnly,
}

fn merge_stale_mail_status_into_persisted(
    persisted_save: &mut CharacterSaveRecord,
    stale_save: &CharacterSaveRecord,
) -> Result<bool, String> {
    let Some(stale_state) = stale_save.stage5_systems_json.as_deref() else {
        return Ok(false);
    };
    let mut stale_systems = serde_json::from_str::<Stage5SystemsState>(stale_state)
        .map_err(|error| format!("failed to decode stale mailbox status: {error}"))?;
    validate_stage5_systems_item_carriers(&stale_systems)?;
    let mut persisted_systems = persisted_save
        .stage5_systems_json
        .as_deref()
        .map(serde_json::from_str::<Stage5SystemsState>)
        .transpose()
        .map_err(|error| format!("failed to decode persisted mailbox status: {error}"))?
        .unwrap_or_default();
    validate_stage5_systems_item_carriers(&persisted_systems)?;
    normalize_stage5_mail_delivery_nonces(&mut stale_systems.mail)?;
    let mut changed = normalize_stage5_mail_delivery_nonces(&mut persisted_systems.mail)?;

    for persisted in &mut persisted_systems.mail {
        let Some(stale) = stale_systems
            .mail
            .iter()
            .find(|stale| stage5_mail_same_delivery(stale, persisted))
        else {
            continue;
        };
        let opened = persisted.opened || stale.opened;
        // Locking is a protection boundary, so a stale full-World snapshot
        // may add a lock but must never remove a newer durable lock. A current
        // session can still unlock through the normal revision-matched save.
        let locked = persisted.locked || stale.locked;
        let claimed = persisted.claimed || stale.claimed;
        // Deletion is protected by the effective lock as well. Otherwise an
        // old unlocked session could mark the message deleted after another
        // session locked the durable copy.
        let deleted = persisted.deleted || (!locked && stale.deleted);
        if persisted.opened != opened
            || persisted.locked != locked
            || persisted.claimed != claimed
            || persisted.deleted != deleted
        {
            persisted.opened = opened;
            persisted.locked = locked;
            persisted.claimed = claimed;
            persisted.deleted = deleted;
            changed = true;
        }
    }
    if changed {
        validate_stage5_systems_item_carriers(&persisted_systems)?;
        persisted_save.stage5_systems_json = Some(
            serde_json::to_string(&persisted_systems)
                .map_err(|error| format!("failed to encode persisted mailbox status: {error}"))?,
        );
    }
    Ok(changed)
}

#[cfg(test)]
mod stale_mail_status_merge_tests {
    use super::*;
    use mir2_protocol::{MirClass, MirGender};

    fn save_with_mail(locked: bool, deleted: bool) -> CharacterSaveRecord {
        let mut save = CharacterSaveRecord::new(CharacterRecord {
            index: 0,
            name: "LockOwner".to_string(),
            level: 1,
            class: MirClass::Warrior,
            gender: MirGender::Male,
        });
        let mut systems = Stage5SystemsState::default();
        systems.mail.push(Stage5MailMessage {
            id: 7,
            delivery_nonce: "00112233445566778899aabbccddeeff".to_string(),
            from: "Sender".to_string(),
            to: "LockOwner".to_string(),
            subject: "Protected".to_string(),
            body: "mail".to_string(),
            gold: 0,
            items: Vec::new(),
            item_states_json: Vec::new(),
            opened: false,
            locked,
            claimed: false,
            deleted,
        });
        save.stage5_systems_json = Some(serde_json::to_string(&systems).unwrap());
        save
    }

    #[test]
    fn stale_snapshot_cannot_remove_newer_durable_mail_lock() {
        let mut persisted = save_with_mail(true, false);
        let stale = save_with_mail(false, false);

        assert!(!merge_stale_mail_status_into_persisted(&mut persisted, &stale).unwrap());
        let systems: Stage5SystemsState =
            serde_json::from_str(persisted.stage5_systems_json.as_deref().unwrap()).unwrap();
        assert!(systems.mail[0].locked);
    }

    #[test]
    fn stale_snapshot_can_only_strengthen_mail_lock() {
        let mut persisted = save_with_mail(false, false);
        let stale = save_with_mail(true, false);

        assert!(merge_stale_mail_status_into_persisted(&mut persisted, &stale).unwrap());
        let systems: Stage5SystemsState =
            serde_json::from_str(persisted.stage5_systems_json.as_deref().unwrap()).unwrap();
        assert!(systems.mail[0].locked);
    }

    #[test]
    fn stale_unlocked_snapshot_cannot_delete_newer_durable_locked_mail() {
        let mut persisted = save_with_mail(true, false);
        let stale = save_with_mail(false, true);

        assert!(!merge_stale_mail_status_into_persisted(&mut persisted, &stale).unwrap());
        let systems: Stage5SystemsState =
            serde_json::from_str(persisted.stage5_systems_json.as_deref().unwrap()).unwrap();
        assert!(systems.mail[0].locked);
        assert!(!systems.mail[0].deleted);
    }
}

pub(super) fn persist_character_save(
    world: &World,
    account_id: &str,
    mut save: CharacterSaveRecord,
) -> Result<PersistCharacterSaveResult, String> {
    let config = world.resource::<RuntimeConfigResource>().config.clone();
    let active_character = {
        let session = world.resource::<SessionResource>();
        let active_account_id = active_session_mutating_account_id(session).ok_or_else(|| {
            "full character save requires an authenticated account identity".to_string()
        })?;
        if active_account_id != account_id {
            return Err("full character save account identity mismatch".to_string());
        }
        session
            .selected_character
            .as_ref()
            .ok_or_else(|| "full character save requires a selected character".to_string())?
            .clone()
    };
    if !exact_character_identity_matches(&save.character, &active_character) {
        return Err("full character save snapshot identity mismatch".to_string());
    }
    migrate_legacy_candidate_save_record(&mut save)?;
    validate_character_save_record(&save)?;
    let account_id = account_id.to_string();
    let expected_revision = save.revision;
    let character_index = active_character.index;
    let touched_accounts = vec![account_id.clone()];

    config.commit_account_store_transaction(&touched_accounts, move |store| {
        let account = store
            .accounts
            .get(&account_id)
            .ok_or_else(|| "full character save requires an existing account".to_string())?;
        let persisted_character = account
            .characters
            .iter()
            .find(|character| character.index == character_index)
            .ok_or_else(|| "full character save requires an existing character".to_string())?;
        if !exact_character_identity_matches(persisted_character, &active_character)
            || !exact_character_identity_matches(&save.character, &active_character)
        {
            return Err("full character save identity mismatch".to_string());
        }
        let mut persisted_save = account
            .saves
            .get(&character_index)
            .cloned()
            .ok_or_else(|| "full character save requires an existing durable save".to_string())?;
        if !exact_character_identity_matches(&persisted_save.character, &active_character) {
            return Err("full character durable save identity mismatch".to_string());
        }
        migrate_legacy_candidate_save_record(&mut persisted_save)?;
        if persisted_save.revision != expected_revision {
            let durable_revision = persisted_save.revision;
            let mut durable_save = persisted_save;
            if !merge_stale_mail_status_into_persisted(&mut durable_save, &save)? {
                return Err(format!(
                    "stale full character save rejected: expected revision {expected_revision}, durable revision {durable_revision}"
                ));
            }
            validate_character_save_record(&durable_save)?;
            durable_save.revision = durable_revision
                .checked_add(1)
                .ok_or_else(|| "mail-status revision exhausted".to_string())?;
            store
                .accounts
                .get_mut(&account_id)
                .expect("validated stale-save account should exist")
                .saves
                .insert(character_index, durable_save);
            return Ok(PersistCharacterSaveResult::StaleMailStatusOnly);
        }

        merge_persisted_mail_into_character_save(&mut save, &persisted_save)?;
        validate_character_save_record(&save)?;
        let committed_revision = expected_revision
            .checked_add(1)
            .ok_or_else(|| "full character save revision exhausted".to_string())?;
        save.revision = committed_revision;

        let account = store
            .accounts
            .get_mut(&account_id)
            .expect("validated full-save account should exist");
        if let Some(character) = account
            .characters
            .iter_mut()
            .find(|character| character.index == character_index)
        {
            *character = save.character.clone();
        }
        account.saves.insert(character_index, save);
        Ok(PersistCharacterSaveResult::Full(committed_revision))
    })
}

pub(super) fn merge_persisted_mail_into_character_save(
    save: &mut CharacterSaveRecord,
    persisted_save: &CharacterSaveRecord,
) -> Result<bool, String> {
    let Some(persisted_state) = persisted_save.stage5_systems_json.as_deref() else {
        return Ok(false);
    };
    let persisted_systems = serde_json::from_str::<Stage5SystemsState>(persisted_state)
        .map_err(|error| format!("failed to decode persisted stage5 mail: {error}"))?;
    validate_stage5_systems_item_carriers(&persisted_systems)?;
    if persisted_systems.mail.is_empty() {
        return Ok(false);
    }
    let mut systems = match save.stage5_systems_json.as_deref() {
        Some(state) => serde_json::from_str::<Stage5SystemsState>(state)
            .map_err(|error| format!("failed to decode active stage5 mail: {error}"))?,
        None => Stage5SystemsState::default(),
    };
    validate_stage5_systems_item_carriers(&systems)?;
    if !merge_external_stage5_mail(&mut systems.mail, persisted_systems.mail)? {
        return Ok(false);
    }
    save.stage5_systems_json = Some(
        serde_json::to_string(&systems)
            .map_err(|error| format!("failed to encode merged stage5 mail: {error}"))?,
    );
    Ok(true)
}

pub(super) fn account_characters(
    config: &SimulationConfig,
    account_id: &str,
) -> Vec<CharacterRecord> {
    let store = config
        .account_store
        .lock()
        .expect("account store mutex should not be poisoned");
    store
        .accounts
        .get(account_id)
        .map(|account| account.characters.clone())
        .unwrap_or_default()
}

/// Legacy hashed-password marker. New writes use a standard Argon2id PHC string.
const LEGACY_PASSWORD_HASH_PREFIX: &str = "sha256$";
const ARGON2ID_PASSWORD_HASH_PREFIX: &str = "$argon2id$";
/// Iteration count for the stretched SHA-256 password hash. Not as strong as
/// Argon2id; retained only to verify and transparently migrate existing rows.
const PASSWORD_HASH_ITERATIONS: u32 = 100_000;

/// Account-id namespace reserved for wallet/passkey accounts. Accounts in this
/// namespace authenticate out-of-band through the HMAC-token-verified passkey
/// path (gateway `verify_passkey_gateway_token` -> [`login_passkey_account`]) and
/// must NEVER be reachable through the classic account/password operations
/// ([`login_account`], [`create_account_with_password`],
/// [`change_account_password`]). A Sui address is public on-chain, so letting the
/// password path touch this namespace would let anyone who knows a victim's
/// address take over their wallet account.
const WALLET_ACCOUNT_PREFIX: &str = "sui:";

/// Stored-password sentinel for wallet/passkey accounts. [`account_password_matches`]
/// special-cases it to always return `false`, so even if such an account is ever
/// reached by the classic password path (e.g. a record persisted before this
/// hardening), no candidate — including a literal copy of the sentinel string —
/// can authenticate. Wallet accounts have no password; they authenticate via the
/// passkey token.
pub(super) const LOCKED_ACCOUNT_PASSWORD: &str = "\u{0}wallet-locked";

/// Whether `account_id` belongs to the reserved wallet/passkey namespace and must
/// therefore bypass every classic password operation. Panic-safe and
/// ASCII-case-insensitive on the reserved prefix.
pub(super) fn is_wallet_namespaced_account(account_id: &str) -> bool {
    account_id
        .get(..WALLET_ACCOUNT_PREFIX.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(WALLET_ACCOUNT_PREFIX))
}

fn stretch_password(salt: &[u8], password: &str) -> [u8; 32] {
    let mut digest = [0_u8; 32];
    let mut hasher = Sha256::new();
    hasher.update(salt);
    hasher.update(password.as_bytes());
    digest.copy_from_slice(&hasher.finalize());
    for _ in 1..PASSWORD_HASH_ITERATIONS {
        let mut hasher = Sha256::new();
        hasher.update(salt);
        hasher.update(digest);
        digest.copy_from_slice(&hasher.finalize());
    }
    digest
}

fn hex_decode(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).ok())
        .collect()
}

/// Hash an account password with Argon2id and a CSPRNG-generated salt.
fn hash_account_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| format!("argon2id password hashing failed: {error}"))
}

/// Constant-time byte comparison to avoid leaking match length via timing.
fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0_u8;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PasswordVerification {
    Invalid,
    Current,
    LegacyNeedsMigration,
}

/// Verify the current Argon2id PHC format and the two historical formats.
/// Historical successful logins are immediately rewritten through Argon2id.
fn verify_account_password(stored: &str, candidate: &str) -> PasswordVerification {
    if stored == LOCKED_ACCOUNT_PASSWORD {
        return PasswordVerification::Invalid;
    }
    if stored.starts_with(ARGON2ID_PASSWORD_HASH_PREFIX) {
        let Ok(parsed) = PasswordHash::new(stored) else {
            return PasswordVerification::Invalid;
        };
        return if Argon2::default()
            .verify_password(candidate.as_bytes(), &parsed)
            .is_ok()
        {
            PasswordVerification::Current
        } else {
            PasswordVerification::Invalid
        };
    }
    if let Some(rest) = stored.strip_prefix(LEGACY_PASSWORD_HASH_PREFIX) {
        let mut parts = rest.splitn(2, '$');
        let (Some(salt_hex), Some(hash_hex)) = (parts.next(), parts.next()) else {
            return PasswordVerification::Invalid;
        };
        let (Some(salt), Some(expected)) = (hex_decode(salt_hex), hex_decode(hash_hex)) else {
            return PasswordVerification::Invalid;
        };
        let actual = stretch_password(&salt, candidate);
        if constant_time_eq(&expected, &actual) {
            PasswordVerification::LegacyNeedsMigration
        } else {
            PasswordVerification::Invalid
        }
    } else if constant_time_eq(stored.as_bytes(), candidate.as_bytes()) {
        PasswordVerification::LegacyNeedsMigration
    } else {
        PasswordVerification::Invalid
    }
}

#[cfg(test)]
pub(super) fn account_password_matches(stored: &str, candidate: &str) -> bool {
    verify_account_password(stored, candidate) != PasswordVerification::Invalid
}

fn production_identity_policy_enabled() -> bool {
    if std::env::var("MIR2_IDENTITY_POLICY")
        .is_ok_and(|value| value.trim().eq_ignore_ascii_case("commercial"))
    {
        return true;
    }
    ["MIR2_RUNTIME_ENV", "MIR2_DEPLOYMENT_ENV", "MIR2_ENV"]
        .into_iter()
        .filter_map(|name| std::env::var(name).ok())
        .any(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "production" | "prod" | "staging"
            )
        })
}

fn commercial_account_id_is_valid(account_id: &str) -> bool {
    let length = account_id.len();
    (3..=32).contains(&length)
        && account_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn commercial_password_is_valid(account_id: &str, password: &str) -> bool {
    let length = password.chars().count();
    (10..=128).contains(&length)
        && !password.chars().any(char::is_control)
        && !password.eq_ignore_ascii_case(account_id)
        && !matches!(
            password.to_ascii_lowercase().as_str(),
            "1234567890" | "password123" | "qwerty12345" | "mir2password"
        )
}

pub(super) fn create_account_with_password(
    config: &SimulationConfig,
    account_id: &str,
    password: &str,
) -> u8 {
    if is_wallet_namespaced_account(account_id) {
        return 0;
    }
    if production_identity_policy_enabled() {
        if !commercial_account_id_is_valid(account_id) {
            return 1;
        }
        if !commercial_password_is_valid(account_id, password) {
            return 2;
        }
    }
    let Ok(password_hash) = hash_account_password(password) else {
        return 0;
    };
    let account_id = account_id.to_string();
    let touched_accounts = vec![account_id.clone()];
    match config.commit_account_store_transaction(&touched_accounts, move |store| {
        if store.accounts.contains_key(&account_id) {
            return Err("account already exists".to_string());
        }
        let mut account = AccountRecord::empty();
        account.password = password_hash;
        store.accounts.insert(account_id, account);
        Ok(())
    }) {
        Ok(()) => 8,
        Err(error) if error == "account already exists" => 7,
        Err(_) => 0,
    }
}

pub(super) enum AccountLoginResult {
    Success(Vec<CharacterRecord>),
    Banned(AccountBanStatus),
    InvalidCredentials,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RecoveryLoginPreflight {
    Eligible,
    Missing,
    Banned(AccountBanStatus),
    Rejected,
}

enum RecoveryCredential<'a> {
    Standard(&'a str),
    Passkey,
}

fn recovery_login_preflight_after_source_refresh(
    config: &SimulationConfig,
    account_id: &str,
    credential: RecoveryCredential<'_>,
    source_refresh: Result<AccountSourceRefreshOutcome, String>,
) -> Result<RecoveryLoginPreflight, String> {
    let source_refresh = source_refresh?;
    if source_refresh == AccountSourceRefreshOutcome::Missing {
        return Ok(RecoveryLoginPreflight::Missing);
    }
    let store = config
        .account_store
        .lock()
        .map_err(|_| "account store mutex poisoned during login preflight".to_string())?;
    let Some(account) = store.accounts.get(account_id) else {
        return Ok(match credential {
            RecoveryCredential::Passkey => RecoveryLoginPreflight::Missing,
            RecoveryCredential::Standard(_) => RecoveryLoginPreflight::Rejected,
        });
    };
    if let Some(ban) = account.active_ban(unix_now_ms()) {
        return Ok(RecoveryLoginPreflight::Banned(ban));
    }
    match credential {
        RecoveryCredential::Standard(password) => {
            if verify_account_password(&account.password, password) == PasswordVerification::Invalid
            {
                Ok(RecoveryLoginPreflight::Rejected)
            } else {
                Ok(RecoveryLoginPreflight::Eligible)
            }
        }
        RecoveryCredential::Passkey => Ok(RecoveryLoginPreflight::Eligible),
    }
}

pub(super) fn standard_login_recovery_preflight(
    config: &SimulationConfig,
    account_id: &str,
    password: &str,
) -> Result<RecoveryLoginPreflight, String> {
    if is_wallet_namespaced_account(account_id)
        || (production_identity_policy_enabled() && !commercial_account_id_is_valid(account_id))
    {
        return Ok(RecoveryLoginPreflight::Rejected);
    }
    recovery_login_preflight_after_source_refresh(
        config,
        account_id,
        RecoveryCredential::Standard(password),
        config.refresh_account_store_account(account_id),
    )
}

pub(super) fn passkey_login_recovery_preflight(
    config: &SimulationConfig,
    account_id: &str,
) -> Result<RecoveryLoginPreflight, String> {
    recovery_login_preflight_after_source_refresh(
        config,
        account_id,
        RecoveryCredential::Passkey,
        config.refresh_account_store_account(account_id),
    )
}

pub(super) fn login_account(
    config: &SimulationConfig,
    account_id: &str,
    password: &str,
) -> AccountLoginResult {
    match standard_login_recovery_preflight(config, account_id, password) {
        Ok(RecoveryLoginPreflight::Eligible) => {}
        Ok(RecoveryLoginPreflight::Missing) => {
            return AccountLoginResult::InvalidCredentials;
        }
        Ok(RecoveryLoginPreflight::Banned(ban)) => return AccountLoginResult::Banned(ban),
        Ok(RecoveryLoginPreflight::Rejected) => {
            return AccountLoginResult::InvalidCredentials;
        }
        Err(_error) => {
            eprintln!("authoritative account source unavailable during standard login");
            return AccountLoginResult::InvalidCredentials;
        }
    }
    let verification = {
        let store = match config.account_store.lock() {
            Ok(store) => store,
            Err(_) => return AccountLoginResult::InvalidCredentials,
        };
        let Some(account) = store.accounts.get(account_id) else {
            return AccountLoginResult::InvalidCredentials;
        };
        if let Some(ban) = account.active_ban(unix_now_ms()) {
            return AccountLoginResult::Banned(ban);
        }
        match verify_account_password(&account.password, password) {
            PasswordVerification::Invalid => return AccountLoginResult::InvalidCredentials,
            PasswordVerification::Current => {
                return AccountLoginResult::Success(account.characters.clone());
            }
            PasswordVerification::LegacyNeedsMigration => {
                PasswordVerification::LegacyNeedsMigration
            }
        }
    };
    debug_assert_eq!(verification, PasswordVerification::LegacyNeedsMigration);
    let Ok(password_hash) = hash_account_password(password) else {
        return AccountLoginResult::InvalidCredentials;
    };
    let account_id = account_id.to_string();
    let password = password.to_string();
    let touched_accounts = vec![account_id.clone()];
    let migration = config.commit_account_store_transaction(&touched_accounts, move |store| {
        let account = store
            .accounts
            .get_mut(&account_id)
            .ok_or_else(|| "account disappeared during password migration".to_string())?;
        if let Some(ban) = account.active_ban(unix_now_ms()) {
            return Ok(Err(ban));
        }
        match verify_account_password(&account.password, &password) {
            PasswordVerification::Invalid => {
                Err("credentials changed during password migration".to_string())
            }
            PasswordVerification::Current => Ok(Ok(account.characters.clone())),
            PasswordVerification::LegacyNeedsMigration => {
                account.password = password_hash;
                Ok(Ok(account.characters.clone()))
            }
        }
    });
    match migration {
        Ok(Ok(characters)) => AccountLoginResult::Success(characters),
        Ok(Err(ban)) => AccountLoginResult::Banned(ban),
        Err(_) => AccountLoginResult::InvalidCredentials,
    }
}

pub(super) fn login_passkey_account(
    config: &SimulationConfig,
    account_id: &str,
) -> AccountLoginResult {
    match passkey_login_recovery_preflight(config, account_id) {
        Ok(RecoveryLoginPreflight::Eligible) => {}
        Ok(RecoveryLoginPreflight::Missing) | Ok(RecoveryLoginPreflight::Rejected) => {
            return AccountLoginResult::InvalidCredentials;
        }
        Ok(RecoveryLoginPreflight::Banned(ban)) => return AccountLoginResult::Banned(ban),
        Err(_) => {
            eprintln!("authoritative account source unavailable during passkey login");
            return AccountLoginResult::InvalidCredentials;
        }
    }

    {
        let store = match config.account_store.lock() {
            Ok(store) => store,
            Err(_) => return AccountLoginResult::InvalidCredentials,
        };
        let Some(account) = store.accounts.get(account_id) else {
            return AccountLoginResult::InvalidCredentials;
        };
        if account.password == LOCKED_ACCOUNT_PASSWORD {
            return AccountLoginResult::Success(account.characters.clone());
        }
    }

    let account_id_owned = account_id.to_string();
    let transaction_accounts = vec![account_id_owned.clone()];
    let mutation = config.commit_account_store_transaction(&transaction_accounts, |store| {
        let account = store
            .accounts
            .get_mut(&account_id_owned)
            .ok_or_else(|| "passkey account disappeared before durable healing".to_string())?;
        if let Some(ban) = account.active_ban(unix_now_ms()) {
            return Ok(Err(ban));
        }
        account.password = LOCKED_ACCOUNT_PASSWORD.to_string();
        Ok(Ok(account.characters.clone()))
    });
    match mutation {
        Ok(Ok(characters)) => AccountLoginResult::Success(characters),
        Ok(Err(ban)) => AccountLoginResult::Banned(ban),
        Err(_) => {
            eprintln!("durable passkey account healing failed");
            AccountLoginResult::InvalidCredentials
        }
    }
}

pub(super) fn provision_passkey_account(
    config: &SimulationConfig,
    account_id: &str,
) -> Result<(), String> {
    match passkey_login_recovery_preflight(config, account_id)? {
        RecoveryLoginPreflight::Missing => {}
        RecoveryLoginPreflight::Eligible => {
            return Err("passkey account already exists".to_string());
        }
        RecoveryLoginPreflight::Banned(_) | RecoveryLoginPreflight::Rejected => {
            return Err("passkey account is not eligible for provisioning".to_string());
        }
    }

    let account_id = account_id.to_string();
    let touched_accounts = vec![account_id.clone()];
    config.commit_account_store_transaction(&touched_accounts, move |store| {
        if store.accounts.contains_key(&account_id) {
            return Err("passkey account appeared during provisioning".to_string());
        }
        let mut account = AccountRecord::empty();
        account.password = LOCKED_ACCOUNT_PASSWORD.to_string();
        store.accounts.insert(account_id, account);
        Ok(())
    })
}

pub(super) fn active_account_ban(
    config: &SimulationConfig,
    account_id: &str,
) -> Option<AccountBanStatus> {
    let store = config
        .account_store
        .lock()
        .expect("account store mutex should not be poisoned");
    store
        .accounts
        .get(account_id)
        .and_then(|account| account.active_ban(unix_now_ms()))
}

pub(super) fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

pub(super) fn change_account_password(
    config: SimulationConfig,
    account_id: &str,
    current_password: &str,
    new_password: &str,
) -> u8 {
    // Wallet/passkey accounts have no classic password to change; refuse the
    // operation rather than letting a legacy default password be mutated. (5 =
    // current password rejected, per the Crystal ChangePassword result codes.)
    if is_wallet_namespaced_account(account_id) {
        return 5;
    }
    if account_id.trim().is_empty() {
        return 1;
    }
    if current_password.trim().is_empty() {
        return 2;
    }
    if new_password.trim().is_empty() {
        return 3;
    }
    if production_identity_policy_enabled()
        && (!commercial_account_id_is_valid(account_id)
            || !commercial_password_is_valid(account_id, new_password))
    {
        return 3;
    }

    let Ok(password_hash) = hash_account_password(new_password) else {
        return 0;
    };
    let account_id = account_id.to_string();
    let current_password = current_password.to_string();
    let touched_accounts = vec![account_id.clone()];
    match config.commit_account_store_transaction(&touched_accounts, move |store| {
        let account = store
            .accounts
            .get_mut(&account_id)
            .ok_or_else(|| "account was not found".to_string())?;
        if verify_account_password(&account.password, &current_password)
            == PasswordVerification::Invalid
        {
            return Err("current password was rejected".to_string());
        }
        account.password = password_hash;
        Ok(())
    }) {
        Ok(()) => 6,
        Err(error) if error == "account was not found" => 4,
        Err(error) if error == "current password was rejected" => 5,
        Err(_) => 0,
    }
}

/// Recovery-only password reset used by the authenticated Gateway identity
/// service after it atomically consumes a one-time recovery code.  This path
/// always applies the commercial password policy, regardless of the local
/// fixture compatibility mode used by legacy protocol tests.
pub fn reset_account_password_after_recovery(
    config: &SimulationConfig,
    account_id: &str,
    new_password: &str,
) -> Result<(), String> {
    validate_commercial_identity_credentials(account_id, new_password)?;
    config.refresh_account_store_account(account_id)?;
    let password_hash = hash_account_password(new_password)?;
    let account_id = account_id.to_string();
    let touched_accounts = vec![account_id.clone()];
    config.commit_account_store_transaction(&touched_accounts, move |store| {
        let account = store
            .accounts
            .get_mut(&account_id)
            .ok_or_else(|| "account was not found".to_string())?;
        account.password = password_hash;
        Ok(())
    })
}

pub fn validate_commercial_identity_credentials(
    account_id: &str,
    password: &str,
) -> Result<(), String> {
    if commercial_account_id_is_valid(account_id)
        && commercial_password_is_valid(account_id, password)
    {
        Ok(())
    } else {
        Err("account id or password does not meet commercial policy".to_string())
    }
}

pub(super) fn add_character_to_account(
    config: &SimulationConfig,
    account_id: &str,
    mut character: CharacterRecord,
) -> Result<CharacterRecord, String> {
    let account_id = account_id.to_string();
    let touched_accounts = vec![account_id.clone()];
    config.commit_account_store_transaction(&touched_accounts, move |store| {
        character.index = store.allocate_character_index();
        let save = crystal_new_character_save(config, character.clone());
        validate_character_save_record(&save)?;
        let account = store
            .accounts
            .get_mut(&account_id)
            .ok_or_else(|| "character creation requires an existing account".to_string())?;
        account.saves.insert(character.index, save);
        account.characters.push(character.clone());
        Ok(character)
    })
}

pub(super) fn delete_character_from_account(
    config: &SimulationConfig,
    account_id: &str,
    character_index: i32,
) -> Result<String, String> {
    let account_id = account_id.to_string();
    let touched_accounts = vec![account_id.clone()];
    config.commit_account_store_transaction(&touched_accounts, move |store| {
        let account = store
            .accounts
            .get_mut(&account_id)
            .ok_or_else(|| "account was not found".to_string())?;
        let existing = account
            .characters
            .iter()
            .find(|character| character.index == character_index)
            .cloned()
            .ok_or_else(|| "Character not found.".to_string())?;
        account
            .characters
            .retain(|character| character.index != character_index);
        account.saves.remove(&character_index);
        Ok(existing.name)
    })
}

pub(super) fn character_save_for_start(
    config: &SimulationConfig,
    account_id: &str,
    character_index: i32,
) -> Result<Option<CharacterSaveRecord>, String> {
    if config.refresh_account_store_account(account_id)? == AccountSourceRefreshOutcome::Missing {
        return Ok(None);
    }
    let mut save = {
        let store = config
            .account_store
            .lock()
            .map_err(|_| "account store lock poisoned".to_string())?;
        let Some(account) = store.accounts.get(account_id) else {
            return Ok(None);
        };
        let Some(character) = account
            .characters
            .iter()
            .find(|character| character.index == character_index)
        else {
            return Ok(None);
        };
        let Some(save) = account.saves.get(&character_index) else {
            return Ok(None);
        };
        if save.character.index != character.index || save.character.name != character.name {
            return Ok(None);
        }
        save.clone()
    };
    let mut changed = false;
    changed |= normalize_legacy_default_vitals(&mut save);
    changed |= normalize_legacy_synthetic_starter_equipment(&mut save);
    changed |= normalize_legacy_default_account_demo_seed_state(&mut save);
    changed |= normalize_legacy_crystal_new_character_seed_state(&mut save);
    changed |= normalize_legacy_crystal_level_one_empty_equipment(&mut save);
    changed |= migrate_legacy_candidate_save_record(&mut save)?;
    if !changed {
        return Ok(Some(save));
    }

    let account_id = account_id.to_string();
    let touched_accounts = vec![account_id.clone()];
    config
        .commit_account_store_transaction(&touched_accounts, move |store| {
            let account = store
                .accounts
                .get_mut(&account_id)
                .ok_or_else(|| "character start account disappeared".to_string())?;
            let character_name = account
                .characters
                .iter()
                .find(|character| character.index == character_index)
                .map(|character| character.name.clone())
                .ok_or_else(|| "character start character disappeared".to_string())?;
            let persisted_save = account
                .saves
                .get_mut(&character_index)
                .ok_or_else(|| "character start save disappeared".to_string())?;
            if persisted_save.character.index != character_index
                || persisted_save.character.name != character_name
            {
                return Err("character start save identity changed".to_string());
            }
            normalize_legacy_default_vitals(persisted_save);
            normalize_legacy_synthetic_starter_equipment(persisted_save);
            normalize_legacy_default_account_demo_seed_state(persisted_save);
            normalize_legacy_crystal_new_character_seed_state(persisted_save);
            normalize_legacy_crystal_level_one_empty_equipment(persisted_save);
            migrate_legacy_candidate_save_record(persisted_save)?;
            validate_character_save_record(persisted_save)?;
            Ok(persisted_save.clone())
        })
        .map(Some)
}

pub(super) fn crystal_new_character_save(
    config: &SimulationConfig,
    character: CharacterRecord,
) -> CharacterSaveRecord {
    let starter_equipment = if config.content_profile.is_some() {
        Vec::new()
    } else {
        seed_equipment_items_for_character(character.class, character.gender)
    };
    let mut save = CharacterSaveRecord::new(character);
    save.max_experience = config.experience_required_for_level(save.character.level);
    save.gold = 0;
    save.inventory_items_json = if config.content_profile.is_some() {
        encode_state_vec(&crystal_start_inventory_items(&save.character))
    } else {
        Vec::new()
    };
    save.belt_items_json = Vec::new();
    save.storage_items_json = Vec::new();
    save.equipment_items_json = encode_state_vec(&starter_equipment);
    save.equipment_items_explicit_empty = config.content_profile.is_some();
    save.quest_states_json = Vec::new();
    save.skill_states_json = Vec::new();
    save.item_rental_records_json = Vec::new();
    save.has_rented_item = false;
    save.stage5_systems_json = Some(
        serde_json::to_string(&Stage5SystemsState::default())
            .expect("stage5 systems state should serialize"),
    );
    save
}

pub(super) fn normalize_legacy_default_vitals(save: &mut CharacterSaveRecord) -> bool {
    if save.hp != 120 || save.max_hp != 120 || save.mp != 45 {
        return false;
    }

    let (max_hp, mp) = crystal_base_vitals(save.character.class, save.character.level);
    save.hp = max_hp;
    save.max_hp = max_hp;
    save.mp = mp;
    save.max_mp = mp;
    true
}

pub(super) fn normalize_legacy_default_account_demo_seed_state(
    save: &mut CharacterSaveRecord,
) -> bool {
    if save.character.index != 0 || save.character.level != 7 {
        return false;
    }

    let mut changed = false;
    if save.gold == 0 {
        save.gold = 1280;
        changed = true;
    }
    if save.inventory_items_json.is_empty() {
        save.inventory_items_json = encode_state_vec(&seed_inventory_items());
        changed = true;
    }
    if save.belt_items_json.is_empty() {
        save.belt_items_json = encode_state_vec(&seed_belt_items());
        changed = true;
    }
    if save.storage_items_json.is_empty() {
        save.storage_items_json = encode_state_vec(&seed_storage_items());
        changed = true;
    }
    if save.equipment_items_json.is_empty() && !save.equipment_items_explicit_empty {
        save.equipment_items_json = encode_state_vec(&seed_equipment_items_for_character(
            save.character.class,
            save.character.gender,
        ));
        changed = true;
    }
    if save.quest_states_json.is_empty() {
        save.quest_states_json = encode_state_vec(&vec![QuestState::guide_training()]);
        changed = true;
    }
    if save.skill_states_json.is_empty() {
        save.skill_states_json = encode_state_vec(&seed_skills());
        changed = true;
    }
    changed
}

pub(super) fn normalize_legacy_crystal_new_character_seed_state(
    save: &mut CharacterSaveRecord,
) -> bool {
    let starter_equipment =
        seed_equipment_items_for_character(save.character.class, save.character.gender);
    if save.character.level != 1
        || save.gold != 1280
        || save.credit != 0
        || save.city_currencies.values().any(|&amount| amount != 0)
    {
        return false;
    }
    if !encoded_items_match_seed(&save.inventory_items_json, seed_inventory_items)
        || !encoded_items_match_seed(&save.belt_items_json, seed_belt_items)
        || !encoded_items_match_seed(&save.storage_items_json, seed_storage_items)
        || save.equipment_items_json != encode_state_vec(&starter_equipment)
        || !encoded_items_match_seed(&save.quest_states_json, || {
            vec![QuestState::guide_training()]
        })
        || !encoded_items_match_seed(&save.skill_states_json, seed_skills)
    {
        return false;
    }

    save.gold = 0;
    save.inventory_items_json = Vec::new();
    save.belt_items_json = Vec::new();
    save.storage_items_json = Vec::new();
    save.equipment_items_json = encode_state_vec(&starter_equipment);
    save.equipment_items_explicit_empty = false;
    save.quest_states_json = Vec::new();
    save.skill_states_json = Vec::new();
    true
}

pub(super) fn normalize_legacy_crystal_level_one_empty_equipment(
    save: &mut CharacterSaveRecord,
) -> bool {
    if save.character.level != 1
        || save.gold != 0
        || !save.equipment_items_explicit_empty
        || !save.equipment_items_json.is_empty()
        || !save.inventory_items_json.is_empty()
        || !save.belt_items_json.is_empty()
        || !save.storage_items_json.is_empty()
        || !save.skill_states_json.is_empty()
    {
        return false;
    }

    save.equipment_items_json = encode_state_vec(&seed_equipment_items_for_character(
        save.character.class,
        save.character.gender,
    ));
    save.equipment_items_explicit_empty = false;
    true
}

pub(super) fn normalize_legacy_synthetic_starter_equipment(save: &mut CharacterSaveRecord) -> bool {
    const LEGACY_INVALID_KEYS: [&str; 5] = [
        "cloth-armour",
        "copper-necklace",
        "wood-bracelet-left",
        "straw-sandals",
        "rope-belt",
    ];
    const LEGACY_STARTER_KEYS: [&str; 6] = [
        "wooden-sword",
        "cloth-armour",
        "copper-necklace",
        "wood-bracelet-left",
        "straw-sandals",
        "rope-belt",
    ];

    let Some(mut equipment) = decode_state_vec::<EquipmentState>(&save.equipment_items_json) else {
        return false;
    };
    if !equipment
        .iter()
        .any(|item| LEGACY_INVALID_KEYS.contains(&item.key.as_str()))
    {
        return false;
    }

    equipment.retain(|item| !LEGACY_STARTER_KEYS.contains(&item.key.as_str()));
    for starter in seed_equipment_items_for_character(save.character.class, save.character.gender) {
        if equipment.iter().all(|item| item.slot != starter.slot) {
            equipment.push(starter);
        }
    }
    equipment.sort_by_key(|item| item.slot as u8);
    save.equipment_items_json = encode_state_vec(&equipment);
    save.equipment_items_explicit_empty = equipment.is_empty();
    true
}

pub(super) fn encoded_items_match_seed<T, F>(encoded: &[String], seed: F) -> bool
where
    T: Serialize + for<'de> Deserialize<'de>,
    F: FnOnce() -> Vec<T>,
{
    encoded == encode_state_vec(&seed())
}

fn legacy_candidate_item_target(key: &str) -> Option<(&'static str, i32)> {
    match key {
        "training-manual" => Some(("FireBall", 990)),
        "belt-lantern-oil" => Some(("RepairOil", 706)),
        "repair-powder" => Some(("RareCopperOre", 1135)),
        "training-splinter" => Some(("Timber", 1112)),
        "quest-wasp-stinger" => Some(("SkyStingerEgg", 1112)),
        "guide-ring-right" => Some(("CopperRing", 404)),
        _ => None,
    }
}

fn canonicalize_legacy_candidate_key(key: &mut String) -> Result<bool, String> {
    let Some((target_name, _)) = legacy_candidate_item_target(key) else {
        return Ok(false);
    };
    let template = crystal_item_by_name(target_name)
        .ok_or_else(|| format!("missing Crystal migration target {target_name}"))?;
    *key = crystal_item_key_for_template(&template);
    Ok(true)
}

fn migrate_legacy_candidate_item_state(item: &mut ItemState) -> Result<bool, String> {
    let mut changed = false;
    for child in &mut item.socketed {
        changed |= migrate_legacy_candidate_item_state(child)?;
    }

    let Some((target_name, expected_legacy_index)) = legacy_candidate_item_target(&item.key) else {
        return Ok(changed);
    };
    let legacy_key = item.key.clone();
    if !item.socketed.is_empty()
        || item.user_item_metadata.as_ref().is_some_and(|metadata| {
            metadata.slots.iter().any(Option::is_some)
                || metadata
                    .captured_socket_positions
                    .as_ref()
                    .is_some_and(|positions| positions.iter().any(Option::is_some))
        })
    {
        return Err(format!(
            "legacy Candidate item {legacy_key} cannot migrate with embedded items"
        ));
    }

    let template = crystal_item_by_name(target_name)
        .ok_or_else(|| format!("missing Crystal migration target {target_name}"))?;
    if let Some(index) = item
        .user_item_metadata
        .as_ref()
        .and_then(|metadata| metadata.item_index)
    {
        if index != expected_legacy_index && index != template.item_index {
            return Err(format!(
                "legacy Candidate item {legacy_key} metadata index {index} conflicts with expected {expected_legacy_index} or target {}",
                template.item_index
            ));
        }
    }

    let mut canonical = embedded_item_state_from_template(&template, item.container, item.slot);
    canonical.unique_id = item.unique_id;
    canonical.quantity = item.quantity;
    canonical.added_attack = item.added_attack;
    canonical.added_defence = item.added_defence;
    canonical.added_stats = std::mem::take(&mut item.added_stats);
    canonical.cursed = item.cursed;
    canonical.soul_bound_id = item.soul_bound_id;
    canonical.sealed_expiry_time_binary_datetime = item.sealed_expiry_time_binary_datetime;
    canonical.sealed_next_time_binary_datetime = item.sealed_next_time_binary_datetime;
    canonical.rental_binding_flags = item.rental_binding_flags;
    canonical.rental_owner_name = std::mem::take(&mut item.rental_owner_name);
    canonical.rental_expiry_binary_datetime = item.rental_expiry_binary_datetime;
    canonical.rental_locked = item.rental_locked;
    if let Some(mut metadata) = item.user_item_metadata.take() {
        metadata.item_index = Some(template.item_index);
        metadata.slots.clear();
        metadata.live_socketed_at_capture = true;
        metadata.socket_layout_hydrated = true;
        metadata.captured_socket_positions = Some(vec![None; usize::from(canonical.socket_slots)]);
        canonical.user_item_metadata = Some(metadata);
    }

    *item = canonical;
    Ok(true)
}
fn migrate_legacy_candidate_equipment_state(item: &mut EquipmentState) -> Result<bool, String> {
    let slot = item.slot;
    let mut carrier =
        item_state_from_equipment_state(item.clone(), ItemContainer::Bag1, slot as u8);
    if !migrate_legacy_candidate_item_state(&mut carrier)? {
        return Ok(false);
    }
    *item = equipment_state_from_item_state(&carrier, slot);
    Ok(true)
}

fn migrate_legacy_candidate_stage5_systems(systems: &mut Stage5SystemsState) -> Result<(), String> {
    for (mail_index, mail) in systems.mail.iter_mut().enumerate() {
        for key in &mut mail.items {
            canonicalize_legacy_candidate_key(key)?;
        }
        for (attachment_index, encoded) in mail.item_states_json.iter_mut().enumerate() {
            let mut item = serde_json::from_str::<ItemState>(encoded).map_err(|error| {
                format!(
                    "failed to decode legacy migration mail {mail_index} attachment {attachment_index}: {error}"
                )
            })?;
            if migrate_legacy_candidate_item_state(&mut item)? {
                *encoded = serde_json::to_string(&item).map_err(|error| {
                    format!(
                        "failed to encode migrated mail {mail_index} attachment {attachment_index}: {error}"
                    )
                })?;
            }
        }
    }

    let guild_slots = systems
        .guild
        .storage_item_states
        .keys()
        .copied()
        .collect::<Vec<_>>();
    for slot in guild_slots {
        let encoded = systems
            .guild
            .storage_item_states
            .get_mut(&slot)
            .expect("collected guild storage slot must exist");
        let mut item = serde_json::from_str::<ItemState>(encoded).map_err(|error| {
            format!("failed to decode legacy migration guild slot {slot}: {error}")
        })?;
        if migrate_legacy_candidate_item_state(&mut item)? {
            *encoded = serde_json::to_string(&item)
                .map_err(|error| format!("failed to encode migrated guild slot {slot}: {error}"))?;
            systems.guild.storage_items.insert(slot, item.key.clone());
        }
    }
    if let Some(trade) = systems.trade.as_mut() {
        for key in &mut trade.offered_items {
            canonicalize_legacy_candidate_key(key)?;
        }
    }
    for listing in &mut systems.auction {
        canonicalize_legacy_candidate_key(&mut listing.item_key)?;
    }
    for key in systems.refine.slots.values_mut() {
        canonicalize_legacy_candidate_key(key)?;
    }
    if let Some(key) = systems.refine.current_item.as_mut() {
        canonicalize_legacy_candidate_key(key)?;
    }
    Ok(())
}
fn migrate_legacy_candidate_save_record(save: &mut CharacterSaveRecord) -> Result<bool, String> {
    fn migrate_item_vec(label: &str, encoded: &mut [String]) -> Result<bool, String> {
        let mut changed = false;
        for (index, value) in encoded.iter_mut().enumerate() {
            let mut item = serde_json::from_str::<ItemState>(value).map_err(|error| {
                format!("failed to decode {label} item at index {index}: {error}")
            })?;
            if migrate_legacy_candidate_item_state(&mut item)? {
                *value = serde_json::to_string(&item).map_err(|error| {
                    format!("failed to encode migrated {label} item at index {index}: {error}")
                })?;
                changed = true;
            }
        }
        Ok(changed)
    }

    fn migrate_equipment_vec(label: &str, encoded: &mut [String]) -> Result<bool, String> {
        let mut changed = false;
        for (index, value) in encoded.iter_mut().enumerate() {
            let mut item = serde_json::from_str::<EquipmentState>(value).map_err(|error| {
                format!("failed to decode {label} equipment item at index {index}: {error}")
            })?;
            if migrate_legacy_candidate_equipment_state(&mut item)? {
                *value = serde_json::to_string(&item).map_err(|error| {
                    format!(
                        "failed to encode migrated {label} equipment item at index {index}: {error}"
                    )
                })?;
                changed = true;
            }
        }
        Ok(changed)
    }

    let mut changed = false;
    changed |= migrate_item_vec("inventory", &mut save.inventory_items_json)?;
    changed |= migrate_item_vec("belt", &mut save.belt_items_json)?;
    changed |= migrate_item_vec("storage", &mut save.storage_items_json)?;
    changed |= migrate_item_vec("hero inventory", &mut save.hero_inventory_items_json)?;
    changed |= migrate_equipment_vec("equipment", &mut save.equipment_items_json)?;

    if let Some(encoded) = save.stage5_systems_json.as_mut() {
        let mut systems = serde_json::from_str::<Stage5SystemsState>(encoded)
            .map_err(|error| format!("failed to decode stage5 systems state: {error}"))?;
        let before = serde_json::to_string(&systems)
            .map_err(|error| format!("failed to encode stage5 migration baseline: {error}"))?;
        migrate_legacy_candidate_stage5_systems(&mut systems)?;
        let migrated = serde_json::to_string(&systems)
            .map_err(|error| format!("failed to encode migrated stage5 systems: {error}"))?;
        if migrated != before {
            *encoded = migrated;
            changed = true;
        }
    }

    Ok(changed)
}
fn decode_saved_item_states(label: &str, encoded: &[String]) -> Result<Vec<ItemState>, String> {
    encoded
        .iter()
        .enumerate()
        .map(|(index, item)| {
            serde_json::from_str(item)
                .map_err(|error| format!("failed to decode {label} item at index {index}: {error}"))
        })
        .collect()
}

fn decode_saved_equipment_states(
    label: &str,
    encoded: &[String],
) -> Result<Vec<EquipmentState>, String> {
    encoded
        .iter()
        .enumerate()
        .map(|(index, item)| {
            serde_json::from_str(item).map_err(|error| {
                format!("failed to decode {label} equipment item at index {index}: {error}")
            })
        })
        .collect()
}

fn validate_saved_item_state(label: &str, index: usize, item: &ItemState) -> Result<(), String> {
    validate_committed_item_state_carrier(item)
        .map_err(|error| format!("invalid {label} item at index {index}: {error}"))
}

fn validate_saved_item_states(label: &str, items: &[ItemState]) -> Result<(), String> {
    for (index, item) in items.iter().enumerate() {
        validate_saved_item_state(label, index, item)?;
    }
    Ok(())
}

fn validate_saved_equipment_states(
    label: &str,
    equipment: &[EquipmentState],
) -> Result<(), String> {
    let mut exact_root_unique_ids = BTreeSet::new();
    for (index, item) in equipment.iter().enumerate() {
        if let Some(unique_id) = item.user_item_unique_id {
            if !exact_root_unique_ids.insert(unique_id) {
                return Err(format!(
                    "invalid {label} item at index {index}: duplicate exact root unique id {unique_id}"
                ));
            }
        }
        // Bag1 is only a validation carrier here. Exact equipment UIDs override
        // this fallback; legacy None carriers derive the same Bag1/slot UID as
        // the historical equipment_slot_unique_id path.
        let item_state =
            item_state_from_equipment_state(item.clone(), ItemContainer::Bag1, item.slot as u8);
        validate_saved_item_state(label, index, &item_state)?;
    }
    Ok(())
}

fn decode_and_validate_character_items(
    save: &CharacterSaveRecord,
) -> Result<
    (
        Vec<ItemState>,
        Vec<ItemState>,
        Vec<ItemState>,
        Vec<EquipmentState>,
        Vec<ItemState>,
    ),
    String,
> {
    let mut inventory_items = decode_saved_item_states("inventory", &save.inventory_items_json)?;
    let mut belt_items = decode_saved_item_states("belt", &save.belt_items_json)?;
    let mut storage_items = decode_saved_item_states("storage", &save.storage_items_json)?;
    let mut hero_inventory_items =
        decode_saved_item_states("hero inventory", &save.hero_inventory_items_json)?;
    let mut equipment_items =
        if save.equipment_items_json.is_empty() && !save.equipment_items_explicit_empty {
            seed_equipment_items_for_character(save.character.class, save.character.gender)
        } else {
            decode_saved_equipment_states("equipment", &save.equipment_items_json)?
        };

    for item in inventory_items
        .iter_mut()
        .chain(belt_items.iter_mut())
        .chain(storage_items.iter_mut())
        .chain(hero_inventory_items.iter_mut())
    {
        migrate_legacy_candidate_item_state(item)?;
    }
    for item in &mut equipment_items {
        migrate_legacy_candidate_equipment_state(item)?;
    }

    validate_saved_item_states("inventory", &inventory_items)?;
    validate_saved_item_states("belt", &belt_items)?;
    validate_saved_item_states("storage", &storage_items)?;
    validate_saved_item_states("hero inventory", &hero_inventory_items)?;
    validate_saved_equipment_states("equipment", &equipment_items)?;

    Ok((
        inventory_items,
        belt_items,
        storage_items,
        equipment_items,
        hero_inventory_items,
    ))
}

fn decode_and_validate_stage5_systems(
    save: &CharacterSaveRecord,
) -> Result<Stage5SystemsState, String> {
    let mut systems = match save.stage5_systems_json.as_deref() {
        Some(encoded) => serde_json::from_str::<Stage5SystemsState>(encoded)
            .map_err(|error| format!("failed to decode stage5 systems state: {error}"))?,
        None => Stage5SystemsState::default(),
    };
    migrate_legacy_candidate_stage5_systems(&mut systems)?;
    validate_stage5_systems_item_carriers(&systems)?;
    normalize_stage5_mail_delivery_nonces(&mut systems.mail)?;
    Ok(systems)
}

fn decode_and_validate_npc_buy_back_items(
    encoded: &[String],
) -> Result<Vec<NpcBuyBackState>, String> {
    let mut states = Vec::with_capacity(encoded.len());
    for (state_index, value) in encoded.iter().enumerate() {
        let state = serde_json::from_str::<NpcBuyBackState>(value).map_err(|error| {
            format!("failed to decode npc buy-back state at index {state_index}: {error}")
        })?;
        for (item_index, entry) in state.items.iter().enumerate() {
            validate_committed_user_item_carrier(&entry.item).map_err(|error| {
                format!(
                    "invalid npc buy-back state at index {state_index} item {item_index}: {error}"
                )
            })?;
        }
        states.push(state);
    }
    Ok(states)
}

fn decode_and_validate_npc_used_goods_items(
    encoded: &[String],
) -> Result<Vec<NpcUsedGoodsState>, String> {
    let mut states = Vec::with_capacity(encoded.len());
    for (state_index, value) in encoded.iter().enumerate() {
        let state = serde_json::from_str::<NpcUsedGoodsState>(value).map_err(|error| {
            format!("failed to decode npc used-goods state at index {state_index}: {error}")
        })?;
        for (item_index, item) in state.items.iter().enumerate() {
            validate_committed_user_item_carrier(item).map_err(|error| {
                format!(
                    "invalid npc used-goods state at index {state_index} item {item_index}: {error}"
                )
            })?;
        }
        states.push(state);
    }
    Ok(states)
}

struct DecodedCharacterSavePreflight {
    inventory_items: Vec<ItemState>,
    belt_items: Vec<ItemState>,
    storage_items: Vec<ItemState>,
    equipment_items: Vec<EquipmentState>,
    hero_inventory_items: Vec<ItemState>,
    stage5_systems: Stage5SystemsState,
    npc_buy_back_items: Vec<NpcBuyBackState>,
    npc_used_goods_items: Vec<NpcUsedGoodsState>,
}

fn decode_and_validate_character_save(
    save: &CharacterSaveRecord,
) -> Result<DecodedCharacterSavePreflight, String> {
    let (inventory_items, belt_items, storage_items, equipment_items, hero_inventory_items) =
        decode_and_validate_character_items(save)?;
    let stage5_systems = decode_and_validate_stage5_systems(save)?;
    let npc_buy_back_items = decode_and_validate_npc_buy_back_items(&save.npc_buy_back_items_json)?;
    let npc_used_goods_items =
        decode_and_validate_npc_used_goods_items(&save.npc_used_goods_items_json)?;
    Ok(DecodedCharacterSavePreflight {
        inventory_items,
        belt_items,
        storage_items,
        equipment_items,
        hero_inventory_items,
        stage5_systems,
        npc_buy_back_items,
        npc_used_goods_items,
    })
}

pub(super) fn validate_character_save_record(save: &CharacterSaveRecord) -> Result<(), String> {
    decode_and_validate_character_save(save)?;
    Ok(())
}

pub(super) fn apply_character_save(
    world: &mut World,
    save: &CharacterSaveRecord,
) -> Result<(), String> {
    let DecodedCharacterSavePreflight {
        inventory_items,
        belt_items,
        storage_items,
        equipment_items,
        hero_inventory_items,
        stage5_systems,
        npc_buy_back_items,
        npc_used_goods_items,
    } = decode_and_validate_character_save(save)?;
    {
        let mut session = world.resource_mut::<SessionResource>();
        session.selected_character = Some(save.character.clone());
        session.bind_active_save_revision(save.revision);
    }
    world
        .resource_mut::<PlayerPermissionResource>()
        .unlock_curse = false;
    world
        .resource_mut::<PlayerPermissionResource>()
        .free_map_shout = false;
    world
        .resource_mut::<PlayerPermissionResource>()
        .free_server_shout = false;
    {
        // Source GM rank from the authoritative account record (0 for normal
        // players). Gates the in-game `@` command dispatcher. `MIR2_GM_ACCOUNTS`
        // can additionally grant GM for the session without mutating the record.
        let gm_level = {
            let config = world.resource::<RuntimeConfigResource>().config.clone();
            let account_id =
                active_session_mutating_account_id(world.resource::<SessionResource>());
            let stored = account_id
                .as_ref()
                .and_then(|account_id| {
                    config
                        .account_store
                        .lock()
                        .ok()
                        .and_then(|store| store.accounts.get(account_id).map(|a| a.gm_level))
                })
                .unwrap_or(0);
            let env_gm = account_id
                .as_deref()
                .map(crate::config::account_is_env_gm)
                .unwrap_or(false);
            stored.max(if env_gm { 1 } else { 0 })
        };
        world.resource_mut::<PlayerPermissionResource>().gm_level = gm_level;
    }
    {
        let mut recovery = world.resource_mut::<PotionRecoveryResource>();
        recovery.pending_pot_health_amount = 0;
        recovery.pending_pot_mana_amount = 0;
        recovery.hero_pending_pot_health_amount = 0;
        recovery.hero_pending_pot_mana_amount = 0;
    }
    {
        let mut npc_state = world.resource_mut::<NpcStateResource>();
        npc_state.npc_variables = Vec::new();
        npc_state.active_npc_dialog = None;
        npc_state.active_npc_service = None;
    }
    world
        .resource_mut::<RuntimeQueueResource>()
        .pending_combat_actions = Vec::new();
    world
        .resource_mut::<RuntimeQueueResource>()
        .pending_monster_spawns = Vec::new();
    world
        .resource_mut::<RuntimeQueueResource>()
        .pending_ground_spell_actions = Vec::new();
    let config = world.resource::<RuntimeConfigResource>().config.clone();
    {
        let mut map = world.resource_mut::<MapRuntimeResource>();
        map.current_map = config.map.clone();
        if !save.map_file_name.is_empty() {
            map.current_map.file_name = save.map_file_name.clone();
        }
        if !save.map_title.is_empty() {
            map.current_map.title = save.map_title.clone();
        }
        if config.monster_spawn_source.uses_crystal_current_map() {
            apply_crystal_map_metadata(&mut map.current_map);
        }
    }
    {
        let mut player_runtime = world.resource_mut::<PlayerRuntimeResource>();
        player_runtime.player_position = if save.position == (Point { x: 0, y: 0 }) {
            config.spawn.clone()
        } else {
            save.position.clone()
        };
        player_runtime.player_direction = save.direction;
        let restored_max_mp = if save.max_mp > 0 {
            save.max_mp
        } else {
            crate::config::crystal_base_vitals(save.character.class, save.character.level).1
        };
        player_runtime.player_vitals = PlayerVitals {
            hp: save.hp.clamp(0, save.max_hp.max(1)),
            max_hp: save.max_hp.max(1),
            mp: save.mp.max(0),
            max_mp: restored_max_mp.max(save.mp.max(0)),
        };
        player_runtime.experience = save.experience.max(0);
        player_runtime.max_experience = if config.content_profile.is_some() {
            config.experience_required_for_level(save.character.level)
        } else {
            save.max_experience.max(1)
        };
        player_runtime.gold = save.gold;
        player_runtime.credit = save.credit;
        player_runtime.city_currencies = save.city_currencies.clone();
        player_runtime.pk_points = save.pk_points;
        player_runtime.chat_banned = save.chat_banned;
        player_runtime.chat_ban_until_ms = save.chat_ban_until_ms;
        player_runtime.chat_next_allowed_at_ms = 0;
        player_runtime.chat_spam_tick = 0;
    }
    let mut resources = world.resource_mut::<InventoryResource>();
    resources.inventory_items = inventory_items;
    resources.belt_items = belt_items;
    resources.storage_items = storage_items;
    resources.equipment_items = equipment_items;
    normalize_inventory_known_item_metadata(&mut resources);
    normalize_inventory_unique_ids(&mut resources);
    drop(resources);
    refresh_mount_resource_from_equipment(world);
    world.resource_mut::<HeroInventoryResource>().items = hero_inventory_items;
    world.resource_mut::<Stage5SystemsResource>().stage5_systems = stage5_systems;
    {
        let mut npc_state = world.resource_mut::<NpcStateResource>();
        npc_state.npc_flags = if save.npc_flag_states_json.is_empty() {
            Vec::new()
        } else {
            decode_state_vec(&save.npc_flag_states_json).unwrap_or_default()
        };
        npc_state.npc_saved_values = if save.npc_saved_values_json.is_empty() {
            Vec::new()
        } else {
            decode_state_vec(&save.npc_saved_values_json).unwrap_or_default()
        };
        npc_state.npc_buy_back_items = npc_buy_back_items;
        npc_state.npc_used_goods_items = npc_used_goods_items;
        npc_state.npc_variables = Vec::new();
        npc_state.active_npc_dialog = None;
        npc_state.active_npc_service = None;
    }
    {
        let mut queue = world.resource_mut::<RuntimeQueueResource>();
        queue.pending_combat_actions = Vec::new();
        queue.pending_monster_spawns = Vec::new();
        queue.pending_ground_spell_actions = Vec::new();
        queue.pending_movement_command = None;
    }
    world.resource_mut::<QuestResource>().quests =
        decode_state_vec(&save.quest_states_json).unwrap_or_default();
    world.resource_mut::<SkillResource>().skills =
        decode_state_vec(&save.skill_states_json).unwrap_or_default();
    world.resource_mut::<BuffResource>().buffs = Vec::new();
    {
        let mut rental = world.resource_mut::<ItemRentalResource>();
        rental.rented_items = decode_state_vec(&save.item_rental_records_json).unwrap_or_default();
        rental.has_rented_item = save.has_rented_item;
        rental.active = None;
    }
    super::session::set_runtime_tick(world, 0);
    world.resource_mut::<ObjectIdAllocatorResource>().reset();
    Ok(())
}

#[cfg(test)]
mod character_save_item_validation_tests {
    use super::*;
    use crate::config::{CurrencyKind, Stage5AuctionListing, Stage5TradeState};
    use mir2_protocol::{MirClass, MirGender};
    use std::collections::BTreeMap;

    fn legacy_save_with_inventory(config: &SimulationConfig) -> CharacterSaveRecord {
        let character = CharacterRecord {
            index: 0,
            name: "LegacyCarrier".to_string(),
            level: 7,
            class: MirClass::Warrior,
            gender: MirGender::Male,
        };
        let mut save = default_save_for_character(config, character);
        if save.inventory_items_json.is_empty() {
            save.inventory_items_json =
                encode_state_vec(&crystal_start_inventory_items(&save.character));
        }
        save
    }

    fn atomic_fixture(
        label: &str,
    ) -> (
        SimulationConfig,
        SimulationSession,
        std::path::PathBuf,
        std::path::PathBuf,
    ) {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("mir2-save-{label}-{}-{unique}", std::process::id()));
        fs::create_dir_all(&root).expect("atomic save fixture directory should be created");
        let path = root.join("accounts.json");
        let config = SimulationConfig::default().with_account_store_path(&path);
        config
            .save_account_store()
            .expect("atomic save fixture should have a durable baseline");
        let session = SimulationSession::new(config.clone());
        (config, session, root, path)
    }

    fn session_save_identity(
        session: &SimulationSession,
    ) -> (Option<CharacterRecord>, Option<u64>) {
        let session_resource = session.app.world().resource::<SessionResource>();
        (
            session_resource.selected_character.clone(),
            session_resource.active_save_revision(),
        )
    }

    fn assert_apply_rejected_without_world_session_or_disk_changes(
        label: &str,
        save: &CharacterSaveRecord,
        expected_error: &str,
    ) {
        use std::fs;

        let (_config, mut session, root, path) = atomic_fixture(label);
        let world_before = session.world_snapshot();
        let session_before = session_save_identity(&session);
        let disk_before = fs::read(&path).expect("durable baseline should be readable");

        let error = match apply_character_save(session.app.world_mut(), save) {
            Err(error) => error,
            Ok(()) => panic!("{label}: expected invalid save to be rejected"),
        };
        assert!(
            error.contains(expected_error),
            "expected {expected_error:?} in {error:?}"
        );
        assert_eq!(session.world_snapshot(), world_before);
        assert_eq!(session_save_identity(&session), session_before);
        assert_eq!(
            fs::read(&path).expect("durable baseline should remain readable"),
            disk_before
        );

        drop(session);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn malformed_npc_buy_back_json_rejects_before_world_session_or_disk_changes() {
        let config = SimulationConfig::default();
        let mut save = legacy_save_with_inventory(&config);
        save.npc_buy_back_items_json = vec!["{not valid npc buy-back JSON".to_string()];

        assert_apply_rejected_without_world_session_or_disk_changes(
            "npc-buy-back-json",
            &save,
            "failed to decode npc buy-back state at index 0",
        );
    }

    #[test]
    fn overbudget_npc_used_goods_item_rejects_before_world_session_or_disk_changes() {
        let config = SimulationConfig::default();
        let mut save = legacy_save_with_inventory(&config);
        let inventory: Vec<ItemState> = decode_state_vec(&save.inventory_items_json)
            .expect("legacy fixture inventory should decode");
        let mut nested = super::super::items::try_user_item_from_item_state(
            inventory
                .first()
                .expect("legacy fixture should contain a real Crystal item"),
        )
        .expect("real Crystal fixture should convert to UserItem");
        for _ in 0..=8 {
            let mut parent = nested.clone();
            parent.slots = vec![Some(nested)];
            nested = parent;
        }
        save.npc_used_goods_items_json = encode_state_vec(&[NpcUsedGoodsState {
            script_key: "npc-used-goods-overbudget".to_string(),
            items: vec![nested],
        }]);

        assert_apply_rejected_without_world_session_or_disk_changes(
            "npc-used-goods-overbudget",
            &save,
            "invalid npc used-goods state at index 0 item 0",
        );
    }

    #[test]
    fn duplicate_exact_equipment_root_uid_rejects_start_game_atomically() {
        use std::fs;

        let (config, mut session, root, path) = atomic_fixture("duplicate-equipment-root-uid");
        let mut save = legacy_save_with_inventory(&config);
        let mut equipment =
            seed_equipment_items_for_character(save.character.class, save.character.gender);
        assert!(
            equipment.len() >= 2,
            "fixture requires two real equipment items"
        );
        for equipped in &mut equipment {
            let slot = equipped.slot;
            let carrier =
                item_state_from_equipment_state(equipped.clone(), ItemContainer::Bag1, slot as u8);
            let mut wire = super::super::items::try_user_item_from_item_state(&carrier)
                .expect("real equipment carrier should convert to UserItem");
            wire.unique_id = 0;
            let exact = super::super::items::try_item_state_from_user_item(carrier, &wire)
                .expect("exact zero UID carrier should hydrate");
            *equipped = super::super::equipment::equipment_state_from_item_state(&exact, slot);
            assert_eq!(equipped.user_item_unique_id, Some(0));
        }
        save.equipment_items_json = encode_state_vec(&equipment);
        save.equipment_items_explicit_empty = true;

        let validation_error = validate_character_save_record(&save).unwrap_err();
        assert!(validation_error.contains("duplicate exact root unique id 0"));

        {
            let mut store = config.account_store.lock().unwrap();
            store
                .accounts
                .get_mut("demo")
                .expect("demo account should exist")
                .saves
                .insert(0, save);
        }
        assert!(session
            .handle_packet(ClientPacket::Login {
                account_id: "demo".to_string(),
                password: "demo".to_string(),
            })
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
        let disk_before = fs::read(&path).expect("post-login durable baseline should be readable");
        let world_before = session.world_snapshot();
        let session_before = session_save_identity(&session);

        assert_eq!(
            session.handle_packet(ClientPacket::StartGame { character_index: 0 }),
            vec![ServerPacket::StartGame {
                result: 2,
                resolution: 0,
            }]
        );
        assert_eq!(session.world_snapshot(), world_before);
        assert_eq!(session_save_identity(&session), session_before);
        assert_eq!(
            fs::read(&path).expect("durable baseline should remain readable"),
            disk_before
        );

        drop(session);
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn committed_zero_quantity_persisted_roots_and_children_reject_atomically() {
        let config = SimulationConfig::default();

        let mut root_save = legacy_save_with_inventory(&config);
        let mut root_inventory: Vec<ItemState> =
            decode_state_vec(&root_save.inventory_items_json).expect("root fixture inventory");
        root_inventory[0].quantity = 0;
        root_save.inventory_items_json = encode_state_vec(&root_inventory);
        assert_apply_rejected_without_world_session_or_disk_changes(
            "zero-inventory-root",
            &root_save,
            "quantity 0",
        );

        let mut child_save = legacy_save_with_inventory(&config);
        let mut child_inventory: Vec<ItemState> =
            decode_state_vec(&child_save.inventory_items_json).expect("child fixture inventory");
        let mut child = child_inventory[0].clone();
        child.unique_id = 90_001;
        child.slot = 0;
        child.quantity = 0;
        child.socket_slots = 0;
        child.socketed.clear();
        child.user_item_metadata = None;
        child_inventory[0].socket_slots = 1;
        child_inventory[0].socketed = vec![child];
        child_inventory[0].user_item_metadata = None;
        child_save.inventory_items_json = encode_state_vec(&child_inventory);
        assert_apply_rejected_without_world_session_or_disk_changes(
            "zero-inventory-child",
            &child_save,
            "quantity 0",
        );

        let mut equipment_save = legacy_save_with_inventory(&config);
        let mut equipment = seed_equipment_items_for_character(
            equipment_save.character.class,
            equipment_save.character.gender,
        );
        let mut equipment_child = child_inventory[0].clone();
        equipment_child.unique_id = 90_002;
        equipment_child.quantity = 0;
        equipment_child.socket_slots = 0;
        equipment_child.socketed.clear();
        equipment_child.user_item_metadata = None;
        equipment[0].socket_slots = 1;
        equipment[0].socketed = vec![equipment_child];
        equipment_save.equipment_items_json = encode_state_vec(&equipment);
        equipment_save.equipment_items_explicit_empty = true;
        assert_apply_rejected_without_world_session_or_disk_changes(
            "zero-equipment-child",
            &equipment_save,
            "quantity 0",
        );

        let mut commercial_item = super::super::items::try_user_item_from_item_state(
            root_inventory
                .first()
                .expect("real Crystal commercial fixture"),
        )
        .expect("commercial fixture should convert through the generic carrier");
        commercial_item.count = 0;

        let mut buy_back_save = legacy_save_with_inventory(&config);
        buy_back_save.npc_buy_back_items_json = encode_state_vec(&[NpcBuyBackState {
            script_key: "committed-zero-buyback".to_string(),
            player_name: "LegacyCarrier".to_string(),
            items: vec![super::super::npc::NpcBuyBackItemState {
                item: commercial_item.clone(),
                expires_at_binary_datetime: 0,
            }],
        }]);
        assert_apply_rejected_without_world_session_or_disk_changes(
            "zero-buyback-root",
            &buy_back_save,
            "quantity 0",
        );

        let mut used_goods_save = legacy_save_with_inventory(&config);
        used_goods_save.npc_used_goods_items_json = encode_state_vec(&[NpcUsedGoodsState {
            script_key: "committed-zero-used-goods".to_string(),
            items: vec![commercial_item],
        }]);
        assert_apply_rejected_without_world_session_or_disk_changes(
            "zero-used-goods-root",
            &used_goods_save,
            "quantity 0",
        );
    }

    #[test]
    fn zero_quantity_persisted_inventory_rejects_start_game_atomically() {
        let config = SimulationConfig::default();
        let mut save = legacy_save_with_inventory(&config);
        let mut inventory: Vec<ItemState> =
            decode_state_vec(&save.inventory_items_json).expect("persisted inventory fixture");
        inventory[0].quantity = 0;
        save.inventory_items_json = encode_state_vec(&inventory);
        {
            let mut store = config.account_store.lock().unwrap();
            store
                .accounts
                .get_mut("demo")
                .expect("demo account should exist")
                .saves
                .insert(0, save);
        }

        let mut session = SimulationSession::new(config);
        assert!(session
            .handle_packet(ClientPacket::Login {
                account_id: "demo".to_string(),
                password: "demo".to_string(),
            })
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
        let world_before = session.world_snapshot();
        let session_before = session_save_identity(&session);

        assert_eq!(
            session.handle_packet(ClientPacket::StartGame { character_index: 0 }),
            vec![ServerPacket::StartGame {
                result: 2,
                resolution: 0,
            }]
        );
        assert_eq!(session.world_snapshot(), world_before);
        assert_eq!(session_save_identity(&session), session_before);
    }

    #[test]
    fn zero_quantity_live_inventory_rejects_active_save_without_mutation() {
        use std::fs;

        let (config, mut session, root, path) = atomic_fixture("zero-active-save");
        assert!(session
            .handle_packet(ClientPacket::Login {
                account_id: "demo".to_string(),
                password: "demo".to_string(),
            })
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
        assert!(session
            .handle_packet(ClientPacket::StartGame { character_index: 0 })
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
        config
            .save_account_store()
            .expect("active-save fixture should have a durable baseline");
        session
            .app
            .world_mut()
            .resource_mut::<InventoryResource>()
            .inventory_items[0]
            .quantity = 0;
        let world_before = session.world_snapshot();
        let session_before = session_save_identity(&session);
        let disk_before = fs::read(&path).expect("durable baseline should be readable");

        let error = session.save_active_character().unwrap_err();
        assert!(error.contains("quantity 0"), "unexpected error: {error}");
        assert_eq!(session.world_snapshot(), world_before);
        assert_eq!(session_save_identity(&session), session_before);
        assert_eq!(fs::read(&path).unwrap(), disk_before);

        drop(session);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn malformed_item_carrier_rejects_before_mutating_the_world() {
        let config = SimulationConfig::default();
        let mut session = SimulationSession::new(config.clone());
        let mut save = legacy_save_with_inventory(&config);
        save.inventory_items_json = vec!["{not valid JSON".to_string()];

        let before_world = session.world_snapshot();
        let before_session = {
            let session_resource = session.app.world().resource::<SessionResource>();
            (
                session_resource.selected_character.clone(),
                session_resource.active_save_revision(),
            )
        };

        let error = apply_character_save(session.app.world_mut(), &save).unwrap_err();
        assert!(error.contains("failed to decode inventory item at index 0"));
        assert_eq!(session.world_snapshot(), before_world);
        let session_resource = session.app.world().resource::<SessionResource>();
        assert_eq!(
            (
                session_resource.selected_character.clone(),
                session_resource.active_save_revision(),
            ),
            before_session
        );
    }

    #[test]
    fn malformed_stage5_json_rejects_before_mutating_world_or_session() {
        let config = SimulationConfig::default();
        let mut session = SimulationSession::new(config.clone());
        let mut save = legacy_save_with_inventory(&config);
        save.stage5_systems_json = Some("{not valid stage5 JSON".to_string());

        let before_world = session.world_snapshot();
        let before_session = {
            let session_resource = session.app.world().resource::<SessionResource>();
            (
                session_resource.selected_character.clone(),
                session_resource.active_save_revision(),
            )
        };

        let error = apply_character_save(session.app.world_mut(), &save).unwrap_err();
        assert!(error.contains("failed to decode stage5 systems state"));
        assert_eq!(session.world_snapshot(), before_world);
        let session_resource = session.app.world().resource::<SessionResource>();
        assert_eq!(
            (
                session_resource.selected_character.clone(),
                session_resource.active_save_revision(),
            ),
            before_session
        );
    }

    #[test]
    fn oversized_item_carrier_start_game_returns_result_two_without_partial_state() {
        let config = SimulationConfig::default();
        let mut save = legacy_save_with_inventory(&config);
        let mut inventory: Vec<ItemState> = decode_state_vec(&save.inventory_items_json)
            .expect("legacy fixture inventory should decode");
        inventory[0].quantity = u32::from(u16::MAX) + 1;
        save.inventory_items_json = encode_state_vec(&inventory);
        {
            let mut store = config.account_store.lock().unwrap();
            store
                .accounts
                .get_mut("demo")
                .expect("demo account should exist")
                .saves
                .insert(0, save);
        }

        let mut session = SimulationSession::new(config);
        assert!(session
            .handle_packet(ClientPacket::Login {
                account_id: "demo".to_string(),
                password: "demo".to_string(),
            })
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
        let before_world = session.world_snapshot();

        let packets = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        assert_eq!(
            packets,
            vec![ServerPacket::StartGame {
                result: 2,
                resolution: 0,
            }]
        );
        assert_eq!(session.world_snapshot(), before_world);
        assert!(session
            .app
            .world()
            .resource::<SessionResource>()
            .selected_character
            .is_none());
    }

    #[test]
    fn overbudget_mail_attachment_is_rejected_before_disk_or_world_changes() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "mir2-save-stage5-carrier-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("accounts.json");
        let config = SimulationConfig::default().with_account_store_path(&path);
        let mut session = SimulationSession::new(config.clone());

        assert!(session
            .handle_packet(ClientPacket::Login {
                account_id: "demo".to_string(),
                password: "demo".to_string(),
            })
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
        let mut active_save = default_save_for_character(&config, config.default_character.clone());
        if active_save.inventory_items_json.is_empty() {
            active_save.inventory_items_json =
                encode_state_vec(&crystal_start_inventory_items(&active_save.character));
        }
        apply_character_save(session.app.world_mut(), &active_save)
            .expect("persistence preflight fixture should enter a valid active state");
        rebuild_world(session.app.world_mut());
        config
            .save_account_store()
            .expect("persistence preflight fixture should have a durable baseline");

        let disk_before = fs::read(&path).expect("durable account store should exist");
        let mut attachment = session
            .app
            .world()
            .resource::<InventoryResource>()
            .inventory_items
            .first()
            .cloned()
            .expect("demo inventory should contain a valid carrier");
        for _ in 0..=9 {
            let mut parent = attachment.clone();
            parent.socket_slots = 1;
            parent.gem_count = 1;
            parent.socketed = vec![attachment];
            parent.user_item_metadata = None;
            attachment = parent;
        }
        session
            .app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .mail
            .push(Stage5MailMessage {
                id: 91,
                delivery_nonce: "save-preflight-overbudget".to_string(),
                from: "System".to_string(),
                to: "Demo".to_string(),
                subject: "Over budget".to_string(),
                body: "Must never persist.".to_string(),
                gold: 0,
                items: Vec::new(),
                item_states_json: vec![serde_json::to_string(&attachment).unwrap()],
                opened: false,
                locked: false,
                claimed: false,
                deleted: false,
            });

        let world_before = session.world_snapshot();
        let active_before =
            serde_json::to_value(snapshot_active_character_save(session.app.world()).unwrap())
                .unwrap();
        let error = session.save_active_character().unwrap_err();
        assert!(error.contains("stage5 mail") && error.contains("attachment 0"));
        assert_eq!(fs::read(&path).unwrap(), disk_before);
        assert_eq!(session.world_snapshot(), world_before);
        assert_eq!(
            serde_json::to_value(snapshot_active_character_save(session.app.world()).unwrap())
                .unwrap(),
            active_before
        );

        drop(session);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn legal_legacy_item_save_without_metadata_carrier_still_applies() {
        let config = SimulationConfig::default();
        let mut session = SimulationSession::new(config.clone());
        let mut save = legacy_save_with_inventory(&config);
        let mut inventory: Vec<ItemState> = decode_state_vec(&save.inventory_items_json)
            .expect("legacy fixture inventory should decode");
        inventory[0].user_item_metadata = None;
        save.inventory_items_json = encode_state_vec(&inventory);

        apply_character_save(session.app.world_mut(), &save)
            .expect("legacy item save without a metadata carrier should remain compatible");
        let resources = session.app.world().resource::<InventoryResource>();
        assert_eq!(resources.inventory_items[0].key, inventory[0].key);
        assert!(resources.inventory_items[0].user_item_metadata.is_none());
    }

    fn legacy_alias_state(key: &str, unique_id: u64, quantity: u32) -> ItemState {
        let historical_name = match key {
            "training-manual" => "FireBall",
            "belt-lantern-oil" => "RepairOil",
            "repair-powder" => "RareCopperOre",
            "training-splinter" | "quest-wasp-stinger" => "GingerTea",
            "guide-ring-right" => "CopperRing",
            _ => panic!("unexpected legacy Candidate key {key}"),
        };
        let template =
            crystal_item_by_name(historical_name).expect("real historical alias template");
        let mut state = embedded_item_state_from_template(&template, ItemContainer::Bag1, 9);
        state.unique_id = unique_id;
        state.quantity = quantity;
        let wire = super::super::items::try_user_item_from_item_state(&state)
            .expect("historical alias fixture should encode");
        let mut state = super::super::items::try_item_state_from_user_item(state, &wire)
            .expect("historical alias fixture should hydrate exact metadata");
        state.key = key.to_string();
        state.name = format!("Legacy {key}");
        state
    }

    #[test]
    fn six_legacy_candidate_keys_migrate_once_to_exact_crystal_templates() {
        let cases = [
            ("training-manual", "FireBall", 990, 1),
            ("belt-lantern-oil", "RepairOil", 706, 1),
            ("repair-powder", "RareCopperOre", 1135, 2),
            ("training-splinter", "Timber", 865, 1),
            ("quest-wasp-stinger", "SkyStingerEgg", 876, 1),
            ("guide-ring-right", "CopperRing", 404, 1),
        ];

        for (offset, (legacy_key, target_name, target_index, quantity)) in
            cases.into_iter().enumerate()
        {
            let unique_id = 71_000 + offset as u64;
            let mut state = legacy_alias_state(legacy_key, unique_id, quantity);
            assert!(migrate_legacy_candidate_item_state(&mut state).unwrap());
            let target = crystal_item_by_name(target_name).expect("migration target must exist");
            assert_eq!(target.item_index, target_index);
            assert_eq!(state.key, format!("crystal-item-{target_index}"));
            assert_eq!(state.name, target.name);
            assert_eq!(state.icon, target.image);
            assert_eq!(state.weight, u16::from(target.weight));
            assert_eq!(state.unique_id, unique_id);
            assert_eq!(state.quantity, quantity);
            assert_eq!(
                state
                    .user_item_metadata
                    .as_ref()
                    .and_then(|metadata| metadata.item_index),
                Some(target_index)
            );
            validate_committed_item_state_carrier(&state)
                .expect("migrated item must be an exact committed Crystal carrier");
            assert!(!migrate_legacy_candidate_item_state(&mut state).unwrap());
        }
    }

    #[test]
    fn legacy_candidate_key_with_conflicting_exact_index_is_rejected() {
        let mut state = legacy_alias_state("training-splinter", 72_000, 1);
        state
            .user_item_metadata
            .as_mut()
            .expect("exact fixture metadata")
            .item_index = Some(404);

        let before = serde_json::to_value(&state).unwrap();
        let error = migrate_legacy_candidate_item_state(&mut state).unwrap_err();
        assert!(error.contains("metadata index 404 conflicts"), "{error}");
        assert_eq!(serde_json::to_value(&state).unwrap(), before);
    }

    #[test]
    fn legacy_candidate_keys_migrate_across_stage5_item_carriers() {
        let mut systems = Stage5SystemsState::default();
        let mail_item = legacy_alias_state("training-manual", 73_001, 1);
        systems.mail.push(Stage5MailMessage {
            id: 1,
            delivery_nonce: "legacy-item-migration".to_string(),
            from: "System".to_string(),
            to: "LegacyCarrier".to_string(),
            subject: "Legacy".to_string(),
            body: "Legacy attachment".to_string(),
            gold: 0,
            items: vec!["training-manual".to_string()],
            item_states_json: vec![serde_json::to_string(&mail_item).unwrap()],
            opened: false,
            locked: false,
            claimed: false,
            deleted: false,
        });

        let guild_item = legacy_alias_state("quest-wasp-stinger", 73_002, 1);
        systems
            .guild
            .storage_items
            .insert(3, "quest-wasp-stinger".to_string());
        systems
            .guild
            .storage_item_states
            .insert(3, serde_json::to_string(&guild_item).unwrap());
        systems.guild.storage_item_users.insert(3, 77);
        systems.trade = Some(Stage5TradeState {
            partner: "Partner".to_string(),
            offered_items: vec!["belt-lantern-oil".to_string()],
            offered_slots: BTreeMap::new(),
            offered_unique_ids: BTreeMap::new(),
            offered_gold: 0,
            offered_currency: CurrencyKind::Gold,
            accepted: false,
            locked: false,
            completed: false,
        });
        systems.auction.push(Stage5AuctionListing {
            id: 1,
            seller: "LegacyCarrier".to_string(),
            item_key: "guide-ring-right".to_string(),
            price: 1,
            currency: CurrencyKind::Gold,
            sold: false,
            cancelled: false,
            expired: false,
        });
        systems
            .refine
            .slots
            .insert(0, "training-splinter".to_string());
        systems.refine.current_item = Some("repair-powder".to_string());

        migrate_legacy_candidate_stage5_systems(&mut systems).unwrap();

        assert_eq!(systems.mail[0].items, ["crystal-item-990"]);
        let migrated_mail: ItemState =
            serde_json::from_str(&systems.mail[0].item_states_json[0]).unwrap();
        assert_eq!(migrated_mail.key, "crystal-item-990");
        assert_eq!(
            systems.guild.storage_items.get(&3).map(String::as_str),
            Some("crystal-item-876")
        );
        let migrated_guild: ItemState = serde_json::from_str(
            systems
                .guild
                .storage_item_states
                .get(&3)
                .expect("guild state should remain paired"),
        )
        .unwrap();
        assert_eq!(migrated_guild.key, "crystal-item-876");
        assert_eq!(
            systems.trade.as_ref().unwrap().offered_items,
            ["crystal-item-706"]
        );
        assert_eq!(systems.auction[0].item_key, "crystal-item-404");
        assert_eq!(
            systems.refine.slots.get(&0).map(String::as_str),
            Some("crystal-item-865")
        );
        assert_eq!(
            systems.refine.current_item.as_deref(),
            Some("crystal-item-1135")
        );
        validate_stage5_systems_item_carriers(&systems)
            .expect("migrated Stage5 carriers must pass committed validation");
    }

    #[test]
    fn start_game_legacy_item_migration_is_persisted_and_idempotent() {
        let (config, session, root, _path) = atomic_fixture("legacy-item-writeback");
        drop(session);
        let mut save = {
            let store = config.account_store.lock().unwrap();
            store.accounts["demo"].saves[&0].clone()
        };
        let mut inventory: Vec<ItemState> =
            decode_state_vec(&save.inventory_items_json).expect("demo inventory should decode");
        if inventory.is_empty() {
            inventory = crystal_start_inventory_items(&save.character);
        }
        inventory[0] = legacy_alias_state("training-splinter", 74_001, 1);
        save.inventory_items_json = encode_state_vec(&inventory);
        {
            let mut store = config.account_store.lock().unwrap();
            store
                .accounts
                .get_mut("demo")
                .unwrap()
                .saves
                .insert(0, save);
        }
        config.save_account_store().unwrap();

        let loaded = character_save_for_start(&config, "demo", 0)
            .unwrap()
            .expect("legacy save should migrate at StartGame load");
        let loaded_inventory: Vec<ItemState> =
            decode_state_vec(&loaded.inventory_items_json).unwrap();
        assert_eq!(loaded_inventory[0].key, "crystal-item-865");

        let first_persisted = {
            let store = config.account_store.lock().unwrap();
            store.accounts["demo"].saves[&0]
                .inventory_items_json
                .clone()
        };
        let persisted_inventory: Vec<ItemState> = decode_state_vec(&first_persisted).unwrap();
        assert_eq!(persisted_inventory[0].key, "crystal-item-865");

        let second = character_save_for_start(&config, "demo", 0)
            .unwrap()
            .expect("canonical save should load again");
        assert_eq!(second.inventory_items_json, first_persisted);
        let second_persisted = {
            let store = config.account_store.lock().unwrap();
            store.accounts["demo"].saves[&0]
                .inventory_items_json
                .clone()
        };
        assert_eq!(second_persisted, first_persisted);

        let _ = std::fs::remove_dir_all(root);
    }
}

/// A durable transform must belong to its durable map. Older builds could save
/// a town-revive position while retaining the field map name, which leaves the
/// player outside that map's collision bounds on the next StartGame. Validate
/// against the authoritative full-map collision rather than the active starter
/// window, so ordinary Bichon field positions are not mistaken for corruption.
fn recover_out_of_bounds_loaded_transform(world: &mut World) -> bool {
    let position = world
        .resource::<PlayerRuntimeResource>()
        .player_position
        .clone();
    let current_map_file_name = world
        .resource::<MapRuntimeResource>()
        .current_map
        .file_name
        .clone();
    let Some(collision) = runtime_world_map_collision_data(&current_map_file_name) else {
        return false;
    };
    let bounds = collision.collision.region_bounds;
    if super::movement::point_in_bounds(&bounds, &position) {
        return false;
    }

    let config = world.resource::<RuntimeConfigResource>().config.clone();
    world.resource_mut::<MapRuntimeResource>().current_map = config.map;
    {
        let mut runtime = world.resource_mut::<PlayerRuntimeResource>();
        runtime.player_position = config.spawn;
        runtime.player_direction = MirDirection::Down;
    }
    true
}

impl SimulationSession {
    pub fn delete_character(&mut self, character_index: i32) -> Vec<ServerPacket> {
        self.handle_packet(ClientPacket::DeleteCharacter { character_index })
    }
    pub(super) fn delete_character_impl(&mut self, character_index: i32) -> Vec<ServerPacket> {
        let account_id =
            match crystal_character_select_state(self.app.world().resource::<SessionResource>()) {
                CrystalCharacterSelectState::Authenticated { account_id } => account_id,
                CrystalCharacterSelectState::Unauthenticated
                | CrystalCharacterSelectState::InGame => return Vec::new(),
            };
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();

        match delete_character_from_account(&config, &account_id, character_index) {
            Ok(deleted_name) => {
                let mut session = self.app.world_mut().resource_mut::<SessionResource>();
                session.characters = account_characters(&config, &account_id);
                if session
                    .selected_character
                    .as_ref()
                    .is_some_and(|character| character.index == character_index)
                {
                    session.selected_character = None;
                    session.clear_active_save_revision();
                    drop(session);
                    self.app
                        .world_mut()
                        .resource_mut::<PlayerPermissionResource>()
                        .unlock_curse = false;
                    self.app
                        .world_mut()
                        .resource_mut::<NpcStateResource>()
                        .active_npc_dialog = None;
                    let mut inventory = self.app.world_mut().resource_mut::<InventoryResource>();
                    inventory.storage_unlocked =
                        !inventory.storage_has_password || !config.require_storage_password;
                }

                let _ = deleted_name;
                vec![ServerPacket::DeleteCharacterSuccess { character_index }]
            }
            Err(_error) => vec![ServerPacket::DeleteCharacter { result: 1 }],
        }
    }
}

impl SimulationSession {
    pub(super) fn start_game(&mut self, character_index: i32) -> Vec<ServerPacket> {
        let account_id =
            match crystal_character_select_state(self.app.world().resource::<SessionResource>()) {
                CrystalCharacterSelectState::Authenticated { account_id } => account_id,
                CrystalCharacterSelectState::Unauthenticated => {
                    return vec![ServerPacket::StartGame {
                        result: 1,
                        resolution: 0,
                    }];
                }
                CrystalCharacterSelectState::InGame => return Vec::new(),
            };
        let save = {
            let config = self
                .app
                .world()
                .resource::<RuntimeConfigResource>()
                .config
                .clone();
            if let Some(ban) = active_account_ban(&config, &account_id) {
                return vec![ServerPacket::StartGameBanned {
                    reason: ban.reason,
                    expiry_binary_datetime: ban.ban_until_ms.unwrap_or_default() as i64,
                }];
            }
            character_save_for_start(&config, &account_id, character_index)
        };

        let Ok(Some(save)) = save else {
            return vec![ServerPacket::StartGame {
                result: 2,
                resolution: 0,
            }];
        };
        let character = save.character.clone();

        if apply_character_save(self.app.world_mut(), &save).is_err() {
            return vec![ServerPacket::StartGame {
                result: 2,
                resolution: 0,
            }];
        }
        refresh_runtime_map_collision(self.app.world_mut());
        if recover_out_of_bounds_loaded_transform(self.app.world_mut()) {
            refresh_runtime_map_collision(self.app.world_mut());
        }
        refresh_storage_password_state(self.app.world_mut());
        rebuild_world(self.app.world_mut());
        if should_use_crystal_current_map_world(self.app.world()) {
            clear_non_player_world_entities(self.app.world_mut());
            spawn_visible_world_for_current_map(self.app.world_mut());
            spawn_config_visible_npcs(self.app.world_mut());
        }

        let visible_objects = collect_visible_objects(self.app.world());
        self.visible_objects = visible_objects.keys().copied().collect();

        let resources = self.app.world().resource::<InventoryResource>();
        let player_runtime = self.app.world().resource::<PlayerRuntimeResource>();
        let map = self.app.world().resource::<MapRuntimeResource>();
        let config = &self.app.world().resource::<RuntimeConfigResource>().config;
        let mut sent_item_info_indices = BTreeSet::new();
        let mut packets = vec![
            ServerPacket::StartGame {
                result: 4,
                resolution: 1920,
            },
            ServerPacket::Chat {
                message: format_localized_text(
                    current_language(self.app.world()),
                    "server.Welcome",
                    [localized_text_or_fallback(
                        current_language(self.app.world()),
                        "server.GameName",
                        "Legend of Mir 2",
                    )],
                ),
                chat_type: ChatType::Hint,
            },
        ];
        packets.extend(start_game_item_info_packets(
            resources,
            &mut sent_item_info_indices,
        ));
        packets.extend([
            ServerPacket::MapInformation {
                info: {
                    let mut info = map.current_map.clone();
                    info.title =
                        localized_map_title(current_language(self.app.world()), &info.title);
                    info
                },
            },
            ServerPacket::UserInformation {
                info: build_user_information(
                    config,
                    &character,
                    &entity_position(
                        self.app.world(),
                        player_entity(self.app.world()).expect("player"),
                    )
                    .expect("player position"),
                    entity_facing(
                        self.app.world(),
                        player_entity(self.app.world()).expect("player"),
                    )
                    .expect("player facing"),
                    entity_player_vitals(
                        self.app.world(),
                        player_entity(self.app.world()).expect("player"),
                    )
                    .expect("player vitals"),
                    player_runtime.experience,
                    player_runtime.max_experience,
                    player_runtime.gold,
                    player_runtime.credit,
                    resources.storage_size,
                    resources.has_expanded_storage,
                    resources.storage_has_password,
                    config.require_storage_password,
                    resources.storage_password_last_set_binary_datetime,
                    resources.expanded_storage_expiry_time_binary_datetime,
                    self.app
                        .world()
                        .resource::<Stage5SystemsResource>()
                        .stage5_systems
                        .appearance
                        .hair,
                    &resources.inventory_items,
                    &resources.equipment_items,
                    self.app
                        .world()
                        .resource::<Stage5SystemsResource>()
                        .stage5_systems
                        .hero
                        .as_ref(),
                ),
            },
        ]);
        packets.extend(effective_crystal_quest_info_packets(self.app.world()));
        packets.extend(start_game_recipe_info_packets(&mut sent_item_info_indices));
        packets.extend(start_game_account_social_and_shop_packets());
        packets.extend(start_game_base_stats_packet(character.class));
        let npc_flags = self
            .app
            .world()
            .resource::<NpcStateResource>()
            .npc_flags
            .clone();
        packets.extend(start_game_static_visible_object_packets(
            &map.current_map.file_name,
            &player_runtime.player_position,
            &character,
            &config,
            &npc_flags,
            current_crystal_npc_local_time(),
        ));
        if resources.storage_size != BASE_STORAGE_SLOTS
            || resources.has_expanded_storage
            || resources.expanded_storage_expiry_time_binary_datetime != 0
        {
            packets.push(ServerPacket::ResizeStorage {
                size: i32::from(resources.storage_size),
                has_expanded_storage: resources.has_expanded_storage,
                expiry_time_binary_datetime: resources.expanded_storage_expiry_time_binary_datetime,
            });
        }
        for bundle in visible_objects.into_values() {
            packets.push(bundle.spawn_packet);
            if let Some(health_packet) = bundle.health_packet {
                packets.push(health_packet);
            }
        }
        packets.extend(start_game_post_visible_crystal_bootstrap_packets());
        // Render mineable veins immediately on entry, not just after the first swing.
        packets.extend(super::mining::mine_node_state_packets(self.app.world()));
        // On-chain veins render on entry too, from the last chain-reported stones (M4).
        packets.extend(super::onchain::onchain_mine_node_state_packets(
            self.app.world(),
        ));
        packets
    }
}
