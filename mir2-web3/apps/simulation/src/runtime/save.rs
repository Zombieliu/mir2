use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

use bevy_ecs::prelude::World;
use mir2_game_data::{
    crystal_quest_packet_payloads, format_localized_text, localized_text_or_fallback,
};
use mir2_protocol::{ChatType, ClientPacket, MirDirection, Point, ServerPacket};
use serde::{Deserialize, Serialize};

use crate::config::{
    crystal_base_vitals, AccountBanStatus, AccountRecord, CharacterRecord, CharacterSaveRecord,
    SimulationConfig, Stage5MailMessage, Stage5SystemsState,
};

use super::components::{
    entity_facing, entity_player_vitals, entity_position, player_entity, PlayerVitals,
};
use super::crystal_compat::BASE_STORAGE_SLOTS;
use super::equipment::seed_equipment_items;
use super::inventory::{
    refresh_storage_password_state, seed_belt_items, seed_inventory_items, seed_storage_items,
};
use super::map::{
    clear_non_player_world_entities, rebuild_world, refresh_runtime_map_collision,
    should_use_crystal_current_map_world, spawn_visible_world_for_current_map,
};
use super::packets::*;
use super::quests::QuestState;
use super::resources::{
    current_language, BuffResource, InventoryResource, ItemRentalResource, MapRuntimeResource,
    NpcStateResource, ObjectIdAllocatorResource, PlayerPermissionResource, PlayerRuntimeResource,
    PotionRecoveryResource, QuestResource, RuntimeConfigResource, RuntimeQueueResource,
    SessionResource, SkillResource, Stage5SystemsResource,
};
use super::session::SimulationSession;
use super::skills::seed_skills;

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
    let mut save = CharacterSaveRecord::new(character);
    let (max_hp, mp) = crystal_base_vitals(save.character.class, save.character.level);
    save.position = config.spawn.clone();
    save.map_file_name = config.map.file_name.clone();
    save.map_title = config.map.title.clone();
    save.direction = MirDirection::Down;
    save.hp = max_hp;
    save.max_hp = max_hp;
    save.mp = mp;
    save.experience = 0;
    save.max_experience = 100;
    save.gold = 0;
    save.credit = 0;
    save.inventory_items_json = Vec::new();
    save.belt_items_json = Vec::new();
    save.storage_items_json = Vec::new();
    save.equipment_items_json = Vec::new();
    save.equipment_items_explicit_empty = true;
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
    let player_runtime = world.resource::<PlayerRuntimeResource>();
    let map = world.resource::<MapRuntimeResource>();
    let quests = world.resource::<QuestResource>();
    let skills = world.resource::<SkillResource>();
    let npc_state = world.resource::<NpcStateResource>();
    let rental = world.resource::<ItemRentalResource>();
    let stage5 = world.resource::<Stage5SystemsResource>();
    let character = world
        .resource::<SessionResource>()
        .selected_character
        .clone()?;
    let player = player_entity(world)?;
    let position = entity_position(world, player)?;
    let direction = entity_facing(world, player)?;
    let vitals = entity_player_vitals(world, player)?;

    Some(CharacterSaveRecord {
        character,
        map_file_name: map.current_map.file_name.clone(),
        map_title: map.current_map.title.clone(),
        position,
        direction,
        hp: vitals.hp,
        max_hp: vitals.max_hp,
        mp: vitals.mp,
        experience: player_runtime.experience,
        max_experience: player_runtime.max_experience.max(1),
        gold: player_runtime.gold,
        credit: player_runtime.credit,
        pk_points: player_runtime.pk_points,
        chat_banned: player_runtime.chat_banned,
        chat_ban_until_ms: player_runtime.chat_ban_until_ms,
        inventory_items_json: encode_state_vec(&resources.inventory_items),
        belt_items_json: encode_state_vec(&resources.belt_items),
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

pub(super) fn persist_active_character_save(world: &World) {
    let Some(save) = snapshot_active_character_save(world) else {
        return;
    };
    let account_id = world
        .resource::<SessionResource>()
        .account_id
        .clone()
        .unwrap_or_else(|| "demo".to_string());
    persist_character_save(world, &account_id, save);
}

pub(super) fn refresh_active_external_mail(world: &mut World) -> bool {
    let (config, account_id, character_index) = {
        let config = world.resource::<RuntimeConfigResource>().config.clone();
        let session = world.resource::<SessionResource>();
        let Some(account_id) = session.account_id.clone() else {
            return false;
        };
        let Some(character) = session.selected_character.as_ref() else {
            return false;
        };
        (config, account_id, character.index)
    };

    let external_mail = {
        let Ok(store) = config.account_store.lock() else {
            return false;
        };
        let Some(save) = store
            .accounts
            .get(&account_id)
            .and_then(|account| account.saves.get(&character_index))
        else {
            return false;
        };
        save.stage5_systems_json
            .as_deref()
            .and_then(|state| serde_json::from_str::<Stage5SystemsState>(state).ok())
            .map(|systems| systems.mail)
            .unwrap_or_default()
    };

    if external_mail.is_empty() {
        return false;
    }

    let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
    merge_external_stage5_mail(&mut stage5.stage5_systems.mail, external_mail)
}

pub(super) fn merge_external_stage5_mail(
    local_mail: &mut Vec<Stage5MailMessage>,
    external_mail: Vec<Stage5MailMessage>,
) -> bool {
    let mut changed = false;
    for external in external_mail {
        if let Some(local) = local_mail.iter_mut().find(|mail| mail.id == external.id) {
            let merged = Stage5MailMessage {
                claimed: local.claimed || external.claimed,
                deleted: local.deleted || external.deleted,
                ..external
            };
            if local != &merged {
                *local = merged;
                changed = true;
            }
        } else {
            local_mail.push(external);
            changed = true;
        }
    }
    changed
}

pub(super) fn persist_character_save(world: &World, account_id: &str, save: CharacterSaveRecord) {
    let config = world.resource::<RuntimeConfigResource>().config.clone();
    let mut store = config
        .account_store
        .lock()
        .expect("account store mutex should not be poisoned");
    let account = store
        .accounts
        .entry(account_id.to_string())
        .or_insert_with(|| AccountRecord::new(config.default_character.clone()));

    if let Some(character) = account
        .characters
        .iter_mut()
        .find(|character| character.index == save.character.index)
    {
        *character = save.character.clone();
    } else {
        account.characters.push(save.character.clone());
        account.characters.sort_by_key(|character| character.index);
    }

    account.saves.insert(save.character.index, save);
    drop(store);
    if let Err(error) = config.save_account_store() {
        eprintln!("failed to persist account store: {error}");
    }
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
        .unwrap_or_else(|| vec![config.default_character.clone()])
}

pub(super) fn create_account_with_password(
    config: &SimulationConfig,
    account_id: &str,
    password: &str,
) -> u8 {
    let mut store = config
        .account_store
        .lock()
        .expect("account store mutex should not be poisoned");
    if store.accounts.contains_key(account_id) {
        return 7;
    }
    let mut account = AccountRecord::empty();
    account.password = password.to_string();
    store.accounts.insert(account_id.to_string(), account);
    drop(store);
    if let Err(error) = config.save_account_store() {
        eprintln!("failed to persist account store: {error}");
    }
    8
}

pub(super) enum AccountLoginResult {
    Success(Vec<CharacterRecord>),
    Banned(AccountBanStatus),
    InvalidCredentials,
}

pub(super) fn login_account(
    config: &SimulationConfig,
    account_id: &str,
    password: &str,
) -> AccountLoginResult {
    let mut store = config
        .account_store
        .lock()
        .expect("account store mutex should not be poisoned");
    let account = store
        .accounts
        .entry(account_id.to_string())
        .or_insert_with(|| AccountRecord::new(config.default_character.clone()));
    let now_ms = unix_now_ms();
    if let Some(ban) = account.active_ban(now_ms) {
        return AccountLoginResult::Banned(ban);
    }
    if account.password == password {
        AccountLoginResult::Success(account.characters.clone())
    } else {
        AccountLoginResult::InvalidCredentials
    }
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
    if account_id.trim().is_empty() {
        return 1;
    }
    if current_password.trim().is_empty() {
        return 2;
    }
    if new_password.trim().is_empty() {
        return 3;
    }

    let mut store = config
        .account_store
        .lock()
        .expect("account store mutex should not be poisoned");
    let Some(account) = store.accounts.get_mut(account_id) else {
        return 4;
    };
    if account.password != current_password {
        return 5;
    }

    account.password = new_password.to_string();
    drop(store);
    if let Err(error) = config.save_account_store() {
        eprintln!("failed to persist account store: {error}");
    }
    6
}

pub(super) fn add_character_to_account(
    config: &SimulationConfig,
    account_id: &str,
    mut character: CharacterRecord,
) -> CharacterRecord {
    let mut store = config
        .account_store
        .lock()
        .expect("account store mutex should not be poisoned");
    character.index = store.allocate_character_index();
    let account = store
        .accounts
        .entry(account_id.to_string())
        .or_insert_with(|| AccountRecord::new(config.default_character.clone()));
    account.saves.insert(
        character.index,
        crystal_new_character_save(character.clone()),
    );
    account.characters.push(character.clone());
    drop(store);
    if let Err(error) = config.save_account_store() {
        eprintln!("failed to persist account store: {error}");
    }
    character
}

pub(super) fn delete_character_from_account(
    config: &SimulationConfig,
    account_id: &str,
    character_index: i32,
) -> Result<String, String> {
    let mut store = config
        .account_store
        .lock()
        .expect("account store mutex should not be poisoned");
    let account = store
        .accounts
        .entry(account_id.to_string())
        .or_insert_with(|| AccountRecord::new(config.default_character.clone()));

    let Some(existing) = account
        .characters
        .iter()
        .find(|character| character.index == character_index)
        .cloned()
    else {
        return Err("Character not found.".to_string());
    };

    account
        .characters
        .retain(|character| character.index != character_index);
    account.saves.remove(&character_index);

    drop(store);
    if let Err(error) = config.save_account_store() {
        eprintln!("failed to persist account store: {error}");
    }
    Ok(existing.name)
}

pub(super) fn character_save_for_start(
    config: &SimulationConfig,
    account_id: &str,
    character_index: i32,
) -> Option<CharacterSaveRecord> {
    let mut store = config
        .account_store
        .lock()
        .expect("account store mutex should not be poisoned");
    let account = store
        .accounts
        .entry(account_id.to_string())
        .or_insert_with(|| AccountRecord::new(config.default_character.clone()));
    let character = account
        .characters
        .iter()
        .find(|character| character.index == character_index)
        .cloned()?;
    let save = account
        .saves
        .entry(character_index)
        .or_insert_with(|| default_save_for_character(config, character.clone()));
    let mut changed = false;
    changed |= normalize_legacy_default_vitals(save);
    changed |= normalize_legacy_default_account_demo_seed_state(save);
    changed |= normalize_legacy_crystal_new_character_seed_state(save);
    let save = save.clone();
    drop(store);
    if changed {
        if let Err(error) = config.save_account_store() {
            eprintln!("failed to persist normalized character save: {error}");
        }
    }
    Some(save)
}

pub(super) fn crystal_new_character_save(character: CharacterRecord) -> CharacterSaveRecord {
    let mut save = CharacterSaveRecord::new(character);
    save.gold = 0;
    save.inventory_items_json = Vec::new();
    save.belt_items_json = Vec::new();
    save.storage_items_json = Vec::new();
    save.equipment_items_json = Vec::new();
    save.equipment_items_explicit_empty = true;
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
        save.equipment_items_json = encode_state_vec(&seed_equipment_items());
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
    if save.character.level != 1 || save.gold != 1280 || save.credit != 0 {
        return false;
    }
    if !encoded_items_match_seed(&save.inventory_items_json, seed_inventory_items)
        || !encoded_items_match_seed(&save.belt_items_json, seed_belt_items)
        || !encoded_items_match_seed(&save.storage_items_json, seed_storage_items)
        || !encoded_items_match_seed(&save.equipment_items_json, seed_equipment_items)
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
    save.equipment_items_json = Vec::new();
    save.equipment_items_explicit_empty = true;
    save.quest_states_json = Vec::new();
    save.skill_states_json = Vec::new();
    true
}

pub(super) fn encoded_items_match_seed<T, F>(encoded: &[String], seed: F) -> bool
where
    T: Serialize + for<'de> Deserialize<'de>,
    F: FnOnce() -> Vec<T>,
{
    encoded == encode_state_vec(&seed())
}

pub(super) fn apply_character_save(world: &mut World, save: &CharacterSaveRecord) {
    world.resource_mut::<SessionResource>().selected_character = Some(save.character.clone());
    world
        .resource_mut::<PlayerPermissionResource>()
        .unlock_curse = false;
    world
        .resource_mut::<PlayerPermissionResource>()
        .free_map_shout = false;
    world
        .resource_mut::<PlayerPermissionResource>()
        .free_server_shout = false;
    world
        .resource_mut::<PotionRecoveryResource>()
        .pending_pot_health_amount = 0;
    world
        .resource_mut::<PotionRecoveryResource>()
        .pending_pot_mana_amount = 0;
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
    }
    {
        let mut player_runtime = world.resource_mut::<PlayerRuntimeResource>();
        player_runtime.player_position = if save.position == (Point { x: 0, y: 0 }) {
            config.spawn.clone()
        } else {
            save.position.clone()
        };
        player_runtime.player_direction = save.direction;
        player_runtime.player_vitals = PlayerVitals {
            hp: save.hp.max(1),
            max_hp: save.max_hp.max(1),
            mp: save.mp.max(0),
        };
        player_runtime.experience = save.experience.max(0);
        player_runtime.max_experience = save.max_experience.max(1);
        player_runtime.gold = save.gold;
        player_runtime.credit = save.credit;
        player_runtime.pk_points = save.pk_points;
        player_runtime.chat_banned = save.chat_banned;
        player_runtime.chat_ban_until_ms = save.chat_ban_until_ms;
    }
    let mut resources = world.resource_mut::<InventoryResource>();
    resources.inventory_items = decode_state_vec(&save.inventory_items_json).unwrap_or_default();
    resources.belt_items = decode_state_vec(&save.belt_items_json).unwrap_or_default();
    resources.storage_items = decode_state_vec(&save.storage_items_json).unwrap_or_default();
    resources.equipment_items =
        if save.equipment_items_json.is_empty() && !save.equipment_items_explicit_empty {
            seed_equipment_items()
        } else {
            decode_state_vec(&save.equipment_items_json).unwrap_or_default()
        };
    drop(resources);
    world.resource_mut::<Stage5SystemsResource>().stage5_systems = save
        .stage5_systems_json
        .as_deref()
        .and_then(|state| serde_json::from_str::<Stage5SystemsState>(state).ok())
        .unwrap_or_default();
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
        npc_state.npc_buy_back_items = if save.npc_buy_back_items_json.is_empty() {
            Vec::new()
        } else {
            decode_state_vec(&save.npc_buy_back_items_json).unwrap_or_default()
        };
        npc_state.npc_used_goods_items = if save.npc_used_goods_items_json.is_empty() {
            Vec::new()
        } else {
            decode_state_vec(&save.npc_used_goods_items_json).unwrap_or_default()
        };
        npc_state.npc_variables = Vec::new();
        npc_state.active_npc_dialog = None;
        npc_state.active_npc_service = None;
    }
    {
        let mut queue = world.resource_mut::<RuntimeQueueResource>();
        queue.pending_combat_actions = Vec::new();
        queue.pending_monster_spawns = Vec::new();
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
}

impl SimulationSession {
    pub fn delete_character(&mut self, character_index: i32) -> Vec<ServerPacket> {
        self.handle_packet(ClientPacket::DeleteCharacter { character_index })
    }
    pub(super) fn delete_character_impl(&mut self, character_index: i32) -> Vec<ServerPacket> {
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        let account_id = self
            .app
            .world()
            .resource::<SessionResource>()
            .account_id
            .clone()
            .unwrap_or_else(|| "demo".to_string());

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
        persist_active_character_save(self.app.world());
        let save = {
            let config = self
                .app
                .world()
                .resource::<RuntimeConfigResource>()
                .config
                .clone();
            let account_id = self
                .app
                .world()
                .resource::<SessionResource>()
                .account_id
                .clone()
                .unwrap_or_else(|| "demo".to_string());
            if let Some(ban) = active_account_ban(&config, &account_id) {
                return vec![ServerPacket::StartGameBanned {
                    reason: ban.reason,
                    expiry_binary_datetime: ban.ban_until_ms.unwrap_or_default() as i64,
                }];
            }
            character_save_for_start(&config, &account_id, character_index)
        };

        let Some(save) = save else {
            return vec![ServerPacket::StartGame {
                result: 2,
                resolution: 0,
            }];
        };
        let character = save.character.clone();

        {
            apply_character_save(self.app.world_mut(), &save);
        }
        refresh_runtime_map_collision(self.app.world_mut());
        refresh_storage_password_state(self.app.world_mut());
        rebuild_world(self.app.world_mut());
        if should_use_crystal_current_map_world(self.app.world()) {
            clear_non_player_world_entities(self.app.world_mut());
            spawn_visible_world_for_current_map(self.app.world_mut());
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
        packets.extend(
            crystal_quest_packet_payloads()
                .into_iter()
                .map(|payload| ServerPacket::NewQuestInfo { payload }),
        );
        packets.extend(start_game_recipe_info_packets(&mut sent_item_info_indices));
        packets.extend(start_game_account_social_and_shop_packets());
        packets.extend(start_game_base_stats_packet(character.class));
        packets.extend(start_game_static_visible_object_packets(
            &map.current_map.file_name,
            &player_runtime.player_position,
            &character,
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
        packets
    }
}
