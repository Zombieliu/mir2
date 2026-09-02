use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::{
    crystal_bag_slot_capacity, AccountRecord, CharacterRecord, EquipmentSlot,
    GroundDropItemPayload, ItemContainer, ItemGrade, SimulationConfig,
};
use bevy_ecs::prelude::World;
use mir2_game_data::{crystal_item_by_index, crystal_item_manifest, localized_text_or_fallback};
use mir2_protocol::{
    ChatType, MirClass, MirGender, MirGridType, ServerPacket, UserItem, UserItemStat,
};

use super::components::current_player_is_dead;
use super::crystal_compat::{
    BASE_STORAGE_SLOTS, CRYSTAL_BIND_DONT_STORE, CRYSTAL_STAT_MAX_AC, CRYSTAL_STAT_MAX_DC,
    DOTNET_DATETIME_KIND_LOCAL, DOTNET_TICKS_AT_UNIX_EPOCH, EXPANDED_STORAGE_SLOTS,
};
use super::items::{
    crystal_belt_slot_range_for_item_key, crystal_equipment_slot_for_item_key,
    crystal_equipment_slot_for_template, crystal_item_has_bind_flag, crystal_item_key_for_template,
    crystal_item_stat_value, crystal_item_template_for_item_key, crystal_stack_size_for_item_key,
    default_item_unique_id, embedded_item_state_from_template, item_has_rental_bind_flag,
    item_icon_for_key, item_unique_id, try_item_state_from_user_item,
    try_user_item_from_item_state, user_item_from_item_state,
    validate_committed_item_state_carrier, validate_committed_user_item_carrier, ItemState,
    ItemStateUserItemMetadata,
};
use super::npc::active_crystal_storage_service;
use super::resources::{InventoryResource, RuntimeConfigResource, SessionResource};

#[allow(clippy::too_many_arguments)]
fn seed_item(
    key: &str,
    name: &str,
    slot: u8,
    unique_id: u64,
    container: ItemContainer,
    quantity: u32,
    description: &str,
    durability_current: Option<u16>,
    durability_max: Option<u16>,
    weight: u16,
    equip_slot: Option<EquipmentSlot>,
    grade: ItemGrade,
    added_attack: i32,
    added_defence: i32,
    attack: i32,
    defence: i32,
    heal_hp: i32,
    heal_mp: i32,
) -> ItemState {
    ItemState {
        key: key.to_string(),
        name: name.to_string(),
        icon: item_icon_for_key(key),
        slot,
        unique_id,
        container,
        quantity,
        description: description.to_string(),
        durability_current,
        durability_max,
        weight,
        equip_slot,
        grade,
        added_attack,
        added_defence,
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
        attack,
        defence,
        heal_hp,
        heal_mp,
    }
}

/// Reproduce Crystal's `Envir.StartItems` / `HumanObject.NewCharacter` path.
pub(super) fn crystal_start_inventory_items(character: &CharacterRecord) -> Vec<ItemState> {
    let required_class = match character.class {
        MirClass::Warrior => 1,
        MirClass::Wizard => 2,
        MirClass::Taoist => 4,
        MirClass::Assassin => 8,
        MirClass::Archer => 16,
    };
    let required_gender = match character.gender {
        MirGender::Male => 1,
        MirGender::Female => 2,
    };

    crystal_item_manifest()
        .items
        .into_iter()
        .filter(|template| {
            template.start_item
                && template.required_class & required_class != 0
                && template.required_gender & required_gender != 0
        })
        .enumerate()
        .map(|(slot, template)| {
            let key = crystal_item_key_for_template(&template);
            let equip_slot = crystal_equipment_slot_for_template(&template);
            let durability = (template.durability > 0).then_some(template.durability);
            let description = template
                .tooltip
                .unwrap_or_else(|| "Crystal start item.".to_string());
            ItemState {
                key,
                name: template.name,
                icon: template.image,
                slot: u8::try_from(slot).expect("Crystal start item slot should fit in u8"),
                unique_id: 0,
                container: ItemContainer::Bag1,
                quantity: 1,
                description,
                durability_current: durability,
                durability_max: durability,
                weight: u16::from(template.weight),
                equip_slot,
                grade: match template.grade {
                    1 => ItemGrade::Common,
                    2 => ItemGrade::Rare,
                    3 => ItemGrade::Legendary,
                    4 => ItemGrade::Mythical,
                    5 => ItemGrade::Heroic,
                    _ => ItemGrade::None,
                },
                added_attack: 0,
                added_defence: 0,
                added_stats: Vec::new(),
                socketed: Vec::new(),
                user_item_metadata: None,
                cursed: false,
                socket_slots: template.slots,
                gem_count: 0,
                identified: (!template.need_identify).then_some(true),
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
            }
        })
        .collect()
}

pub(super) fn seed_inventory_items() -> Vec<ItemState> {
    vec![
        seed_item(
            "red-potion",
            "Red Potion",
            0,
            0,
            ItemContainer::Bag1,
            5,
            "Basic healing potion for starter field testing.",
            None,
            None,
            1,
            None,
            ItemGrade::None,
            0,
            0,
            0,
            0,
            35,
            0,
        ),
        seed_item(
            "blue-potion",
            "Blue Potion",
            1,
            1,
            ItemContainer::Bag1,
            3,
            "Starter mana potion reserved for belt wiring.",
            None,
            None,
            1,
            None,
            ItemGrade::None,
            0,
            0,
            0,
            0,
            0,
            20,
        ),
        seed_item(
            "crystal-item-990",
            "FireBall",
            2,
            2,
            ItemContainer::Bag1,
            1,
            "Real Crystal FireBall skill book for inspecting the starter inventory.",
            None,
            None,
            1,
            None,
            ItemGrade::None,
            0,
            0,
            0,
            0,
            0,
            0,
        ),
        seed_item(
            "bronze-helmet",
            "Bronze Helmet",
            3,
            3,
            ItemContainer::Bag1,
            1,
            "Starter equippable helmet used to validate the equip pipeline.",
            Some(16),
            Some(16),
            2,
            Some(EquipmentSlot::Helmet),
            ItemGrade::Common,
            0,
            1,
            0,
            2,
            0,
            0,
        ),
        seed_item(
            "dagger",
            "Dagger",
            4,
            4,
            ItemContainer::Bag1,
            1,
            "Standard shape-01 weapon sample for visible equipment rendering.",
            Some(20),
            Some(20),
            1,
            Some(EquipmentSlot::Weapon),
            ItemGrade::Common,
            0,
            0,
            5,
            0,
            0,
            0,
        ),
        seed_item(
            "leather-armour",
            "Leather Armour",
            5,
            5,
            ItemContainer::Bag1,
            1,
            "Standard shape-01 armour sample for visible equipment rendering.",
            Some(18),
            Some(18),
            2,
            Some(EquipmentSlot::Armour),
            ItemGrade::Common,
            0,
            0,
            0,
            4,
            0,
            0,
        ),
        seed_item(
            "town-teleport",
            "Town Teleport",
            0,
            0,
            ItemContainer::Bag2,
            1,
            "Reserved slot for future travel skill and safe-zone routing.",
            None,
            None,
            1,
            None,
            ItemGrade::None,
            0,
            0,
            0,
            0,
            0,
            0,
        ),
    ]
}

pub(super) fn seed_belt_items() -> Vec<ItemState> {
    vec![
        seed_item(
            "belt-red-potion",
            "Red Potion",
            0,
            0,
            ItemContainer::Belt,
            5,
            "Hotkey potion wired into slot 1.",
            None,
            None,
            1,
            None,
            ItemGrade::None,
            0,
            0,
            0,
            0,
            35,
            0,
        ),
        seed_item(
            "belt-blue-potion",
            "Blue Potion",
            1,
            1,
            ItemContainer::Belt,
            3,
            "Hotkey potion wired into slot 2.",
            None,
            None,
            1,
            None,
            ItemGrade::None,
            0,
            0,
            0,
            0,
            0,
            20,
        ),
        seed_item(
            "crystal-item-706",
            "RepairOil",
            2,
            5,
            ItemContainer::Belt,
            1,
            "Real Crystal RepairOil assigned to the starter belt.",
            None,
            None,
            1,
            None,
            ItemGrade::None,
            0,
            0,
            0,
            0,
            0,
            0,
        ),
    ]
}

pub(super) fn seed_storage_items() -> Vec<ItemState> {
    vec![
        seed_item(
            "stored-red-potion",
            "Red Potion",
            0,
            0,
            ItemContainer::Storage,
            10,
            "Warehouse starter stack used to validate storage flows.",
            None,
            None,
            1,
            None,
            ItemGrade::None,
            0,
            0,
            0,
            0,
            35,
            0,
        ),
        seed_item(
            "stored-bronze-helmet",
            "Bronze Helmet",
            1,
            1,
            ItemContainer::Storage,
            1,
            "Warehouse equipment sample for store/take-back validation.",
            Some(16),
            Some(16),
            2,
            Some(EquipmentSlot::Helmet),
            ItemGrade::Common,
            0,
            1,
            0,
            2,
            0,
            0,
        ),
    ]
}

#[derive(Clone, Copy)]
pub(super) struct StorageAccountState {
    has_password: bool,
    storage_password_last_set_binary_datetime: i64,
    storage_size: u16,
    has_expanded_storage: bool,
    expanded_storage_expiry_time_binary_datetime: i64,
    expanded_storage_expiry_notice_pending: bool,
}

pub(super) fn refresh_storage_password_state(world: &mut World) {
    let (config, account_id) = {
        let config = world.resource::<RuntimeConfigResource>().config.clone();
        let session = world.resource::<SessionResource>();
        (
            config,
            session
                .account_id
                .clone()
                .unwrap_or_else(|| "demo".to_string()),
        )
    };
    let state = storage_account_state(&config, &account_id);
    let mut resources = world.resource_mut::<InventoryResource>();
    resources.storage_has_password = state.has_password;
    resources.storage_password_last_set_binary_datetime =
        state.storage_password_last_set_binary_datetime;
    resources.storage_size = state.storage_size;
    resources.has_expanded_storage = state.has_expanded_storage;
    resources.expanded_storage_expiry_time_binary_datetime =
        state.expanded_storage_expiry_time_binary_datetime;
    resources.expanded_storage_expiry_notice_pending = state.expanded_storage_expiry_notice_pending;
    resources.storage_unlocked = !state.has_password || !config.require_storage_password;
    resources.storage_sent = false;
}

pub(super) fn storage_account_state(
    config: &SimulationConfig,
    account_id: &str,
) -> StorageAccountState {
    let store = config
        .account_store
        .lock()
        .expect("account store mutex should not be poisoned");
    store
        .accounts
        .get(account_id)
        .map(|account| {
            let expanded_storage_expiry_time_binary_datetime =
                account.expanded_storage_expiry_time_binary_datetime;
            let has_expanded_storage = expanded_storage_is_active(
                account.has_expanded_storage,
                expanded_storage_expiry_time_binary_datetime,
            );
            StorageAccountState {
                has_password: !account.storage_password.is_empty(),
                storage_password_last_set_binary_datetime: account
                    .storage_password_last_set_binary_datetime,
                storage_size: normalized_storage_size(account.storage_size),
                has_expanded_storage,
                expanded_storage_expiry_time_binary_datetime,
                expanded_storage_expiry_notice_pending: account.has_expanded_storage
                    && !has_expanded_storage,
            }
        })
        .unwrap_or(StorageAccountState {
            has_password: false,
            storage_password_last_set_binary_datetime: 0,
            storage_size: BASE_STORAGE_SLOTS,
            has_expanded_storage: false,
            expanded_storage_expiry_time_binary_datetime: 0,
            expanded_storage_expiry_notice_pending: false,
        })
}

pub(super) fn storage_password_binary_datetime() -> i64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch");
    let utc_ticks = DOTNET_TICKS_AT_UNIX_EPOCH
        + i64::try_from(now.as_secs()).expect("unix seconds should fit in i64") * 10_000_000
        + i64::from(now.subsec_nanos() / 100);
    utc_ticks | DOTNET_DATETIME_KIND_LOCAL
}

pub(super) fn future_binary_datetime(days: u64) -> i64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch");
    let future_seconds = now
        .as_secs()
        .saturating_add(days.saturating_mul(24 * 60 * 60));
    let utc_ticks = DOTNET_TICKS_AT_UNIX_EPOCH
        + i64::try_from(future_seconds).expect("future unix seconds should fit in i64")
            * 10_000_000
        + i64::from(now.subsec_nanos() / 100);
    utc_ticks | DOTNET_DATETIME_KIND_LOCAL
}

pub(super) fn future_binary_datetime_minutes(minutes: u64) -> i64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch");
    let future_seconds = now.as_secs().saturating_add(minutes.saturating_mul(60));
    let utc_ticks = DOTNET_TICKS_AT_UNIX_EPOCH
        + i64::try_from(future_seconds).expect("future unix seconds should fit in i64")
            * 10_000_000
        + i64::from(now.subsec_nanos() / 100);
    utc_ticks | DOTNET_DATETIME_KIND_LOCAL
}

pub(super) fn binary_datetime_ticks(value: i64) -> i64 {
    ((value as u64) & 0x3fff_ffff_ffff_ffff) as i64
}

pub(super) fn add_minutes_to_binary_datetime(base_binary_datetime: i64, minutes: u64) -> i64 {
    const TICKS_PER_MINUTE: i64 = 60 * 10_000_000;

    binary_datetime_ticks(base_binary_datetime).saturating_add(
        i64::try_from(minutes)
            .expect("minutes should fit in i64")
            .saturating_mul(TICKS_PER_MINUTE),
    ) | DOTNET_DATETIME_KIND_LOCAL
}

pub(super) fn crystal_duration_label_from_minutes(minutes: u64) -> String {
    if minutes == 1 {
        "1 minute".to_string()
    } else {
        format!("{minutes} minutes")
    }
}

pub(super) fn crystal_duration_label_from_seconds(seconds: u64) -> String {
    let rounded_minutes = seconds.saturating_add(59) / 60;
    crystal_duration_label_from_minutes(rounded_minutes.max(1))
}

pub(super) fn current_binary_datetime() -> i64 {
    future_binary_datetime(0)
}

pub(super) fn expanded_storage_is_active(
    has_expanded_storage: bool,
    expiry_time_binary_datetime: i64,
) -> bool {
    has_expanded_storage
        && binary_datetime_ticks(expiry_time_binary_datetime)
            > binary_datetime_ticks(current_binary_datetime())
}

pub(super) fn extend_binary_datetime(base_binary_datetime: i64, days: u64) -> i64 {
    const TICKS_PER_DAY: i64 = 24 * 60 * 60 * 10_000_000;

    let current_ticks = binary_datetime_ticks(current_binary_datetime());
    let base_ticks = binary_datetime_ticks(base_binary_datetime);
    let start_ticks = base_ticks.max(current_ticks);
    start_ticks.saturating_add(
        i64::try_from(days)
            .expect("days should fit in i64")
            .saturating_mul(TICKS_PER_DAY),
    ) | DOTNET_DATETIME_KIND_LOCAL
}

pub(super) fn expand_storage_rental_impl(world: &mut World) -> Vec<ServerPacket> {
    let (config, account_id) = {
        let config = world.resource::<RuntimeConfigResource>().config.clone();
        let session = world.resource::<SessionResource>();
        (
            config,
            session
                .account_id
                .clone()
                .unwrap_or_else(|| "demo".to_string()),
        )
    };

    let expiry_time_binary_datetime = {
        let mut store = config
            .account_store
            .lock()
            .expect("account store mutex should not be poisoned");
        let account = store
            .accounts
            .entry(account_id)
            .or_insert_with(|| AccountRecord::new(config.default_character.clone()));
        let expiry =
            extend_binary_datetime(account.expanded_storage_expiry_time_binary_datetime, 30);
        account.storage_size = EXPANDED_STORAGE_SLOTS;
        account.has_expanded_storage = true;
        account.expanded_storage_expiry_time_binary_datetime = expiry;
        expiry
    };

    if let Err(error) = config.save_account_store() {
        eprintln!("failed to persist account store: {error}");
    }

    {
        let mut resources = world.resource_mut::<InventoryResource>();
        resources.storage_size = EXPANDED_STORAGE_SLOTS;
        resources.has_expanded_storage = true;
        resources.expanded_storage_expiry_time_binary_datetime = expiry_time_binary_datetime;
        resources.expanded_storage_expiry_notice_pending = false;
    }

    vec![ServerPacket::ResizeStorage {
        size: i32::from(EXPANDED_STORAGE_SLOTS),
        has_expanded_storage: true,
        expiry_time_binary_datetime,
    }]
}

pub(super) fn storage_password_required(
    config: &SimulationConfig,
    has_password: bool,
    unlocked: bool,
) -> bool {
    config.require_storage_password && has_password && !unlocked
}

pub(super) fn storage_locked(world: &World) -> bool {
    let resources = world.resource::<InventoryResource>();
    let config = &world.resource::<RuntimeConfigResource>().config;
    storage_password_required(
        config,
        resources.storage_has_password,
        resources.storage_unlocked,
    )
}

pub(super) fn crystal_password_is_valid(password: &str) -> bool {
    let length = password.len();
    (5..=15).contains(&length) && password.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

pub(super) fn normalized_storage_size(size: u16) -> u16 {
    if size > BASE_STORAGE_SLOTS {
        EXPANDED_STORAGE_SLOTS
    } else {
        BASE_STORAGE_SLOTS
    }
}

pub(super) fn storage_backing_size(resources: &InventoryResource) -> u16 {
    normalized_storage_size(resources.storage_size)
}

pub(super) fn sync_expired_expanded_storage(world: &mut World, packets: &mut Vec<ServerPacket>) {
    let (config, account_id, language, storage_size, expiry_time_binary_datetime, notice_pending) = {
        let resources = world.resource::<InventoryResource>();
        let session = world.resource::<SessionResource>();
        (
            world.resource::<RuntimeConfigResource>().config.clone(),
            session
                .account_id
                .clone()
                .unwrap_or_else(|| "demo".to_string()),
            session.language,
            resources.storage_size,
            resources.expanded_storage_expiry_time_binary_datetime,
            resources.expanded_storage_expiry_notice_pending,
        )
    };
    if !notice_pending {
        return;
    }

    {
        let mut resources = world.resource_mut::<InventoryResource>();
        resources.has_expanded_storage = false;
        resources.expanded_storage_expiry_notice_pending = false;
    }

    let changed = {
        let mut store = config
            .account_store
            .lock()
            .expect("account store mutex should not be poisoned");
        let Some(account) = store.accounts.get_mut(&account_id) else {
            return;
        };
        if !account.has_expanded_storage {
            false
        } else {
            account.has_expanded_storage = false;
            true
        }
    };

    if changed {
        if let Err(error) = config.save_account_store() {
            eprintln!("failed to persist account store: {error}");
        }
    }

    packets.push(ServerPacket::Chat {
        message: localized_text_or_fallback(
            language,
            "server.ExpandedStorageExpired",
            "server.ExpandedStorageExpired",
        ),
        chat_type: ChatType::System,
    });
    packets.push(ServerPacket::ResizeStorage {
        size: i32::from(normalized_storage_size(storage_size)),
        has_expanded_storage: false,
        expiry_time_binary_datetime,
    });
}

pub(super) fn current_player_storage_packet(world: &World) -> ServerPacket {
    let resources = world.resource::<InventoryResource>();
    let mut storage = vec![None; usize::from(storage_backing_size(&resources))];

    for item in &resources.storage_items {
        let slot = usize::from(item.slot);
        if let Some(storage_slot) = storage.get_mut(slot) {
            *storage_slot = Some(user_item_from_item_state(item));
        }
    }

    ServerPacket::UserStorage {
        storage: Some(storage),
    }
}

pub(super) fn crystal_send_storage_packet(world: &mut World) -> Option<ServerPacket> {
    let (password_required, storage_sent) = {
        let resources = world.resource::<InventoryResource>();
        let config = &world.resource::<RuntimeConfigResource>().config;
        (
            storage_password_required(
                config,
                resources.storage_has_password,
                resources.storage_unlocked,
            ),
            resources.storage_sent,
        )
    };

    if password_required {
        world.resource_mut::<InventoryResource>().storage_sent = false;
        return None;
    }

    if storage_sent {
        return None;
    }

    world.resource_mut::<InventoryResource>().storage_sent = true;
    Some(current_player_storage_packet(world))
}

pub(super) fn crystal_npc_storage_open_packets(world: &mut World) -> Vec<ServerPacket> {
    let mut packets = Vec::new();
    if let Some(packet) = crystal_send_storage_packet(world) {
        packets.push(packet);
    }
    packets.push(ServerPacket::NPCStorage);
    packets
}

pub(super) fn accessible_storage_size(resources: &InventoryResource) -> u16 {
    if resources.has_expanded_storage {
        storage_backing_size(resources)
    } else {
        BASE_STORAGE_SLOTS
    }
}

pub(super) fn is_valid_storage_slot(resources: &InventoryResource, slot: u8) -> bool {
    u16::from(slot) < accessible_storage_size(resources)
}

pub(super) fn unlock_storage_impl(world: &mut World, password: &str) -> Vec<ServerPacket> {
    let (config, account_id) = {
        let config = world.resource::<RuntimeConfigResource>().config.clone();
        let session = world.resource::<SessionResource>();
        (
            config,
            session
                .account_id
                .clone()
                .unwrap_or_else(|| "demo".to_string()),
        )
    };
    let account_state = storage_account_state(&config, &account_id);
    if !active_crystal_storage_service(world) {
        let mut resources = world.resource_mut::<InventoryResource>();
        resources.storage_has_password = account_state.has_password;
        resources.storage_password_last_set_binary_datetime =
            account_state.storage_password_last_set_binary_datetime;
        return vec![ServerPacket::StorageUnlockResult {
            result: 3,
            has_password: account_state.has_password,
        }];
    }

    let (result, has_password, last_set_binary_datetime) = {
        let mut store = config
            .account_store
            .lock()
            .expect("account store mutex should not be poisoned");
        let account = store
            .accounts
            .entry(account_id)
            .or_insert_with(|| AccountRecord::new(config.default_character.clone()));
        if account.storage_password.is_empty() {
            (4, false, account.storage_password_last_set_binary_datetime)
        } else if !crystal_password_is_valid(password) {
            (1, true, account.storage_password_last_set_binary_datetime)
        } else if account.storage_password != password {
            (2, true, account.storage_password_last_set_binary_datetime)
        } else {
            (0, true, account.storage_password_last_set_binary_datetime)
        }
    };

    let require_storage_password = world
        .resource::<RuntimeConfigResource>()
        .config
        .require_storage_password;
    {
        let mut resources = world.resource_mut::<InventoryResource>();
        resources.storage_has_password = has_password;
        resources.storage_password_last_set_binary_datetime = last_set_binary_datetime;
        resources.storage_unlocked = result == 0 || !has_password || !require_storage_password;
    }

    let mut packets = vec![ServerPacket::StorageUnlockResult {
        result,
        has_password,
    }];
    if result == 0 {
        if let Some(packet) = crystal_send_storage_packet(world) {
            packets.push(packet);
        }
    }
    packets
}

pub(super) fn set_storage_password_impl(
    world: &mut World,
    current_password: &str,
    new_password: &str,
) -> Vec<ServerPacket> {
    let (config, account_id) = {
        let config = world.resource::<RuntimeConfigResource>().config.clone();
        let session = world.resource::<SessionResource>();
        (
            config,
            session
                .account_id
                .clone()
                .unwrap_or_else(|| "demo".to_string()),
        )
    };
    let account_state = storage_account_state(&config, &account_id);
    if !active_crystal_storage_service(world) {
        let mut resources = world.resource_mut::<InventoryResource>();
        resources.storage_has_password = account_state.has_password;
        resources.storage_password_last_set_binary_datetime =
            account_state.storage_password_last_set_binary_datetime;
        return vec![ServerPacket::StoragePasswordResult {
            result: 0,
            removing: false,
            has_password: account_state.has_password,
            last_set_binary_datetime: account_state.storage_password_last_set_binary_datetime,
        }];
    }

    let (result, has_password, last_set_binary_datetime) = {
        let mut store = config
            .account_store
            .lock()
            .expect("account store mutex should not be poisoned");
        let account = store
            .accounts
            .entry(account_id)
            .or_insert_with(|| AccountRecord::new(config.default_character.clone()));
        if !crystal_password_is_valid(new_password) {
            (
                3,
                !account.storage_password.is_empty(),
                account.storage_password_last_set_binary_datetime,
            )
        } else if account.storage_password.is_empty() {
            let last_set = storage_password_binary_datetime();
            account.storage_password = new_password.to_string();
            account.storage_password_last_set_binary_datetime = last_set;
            (4, true, last_set)
        } else if !crystal_password_is_valid(current_password) {
            (1, true, account.storage_password_last_set_binary_datetime)
        } else if account.storage_password != current_password {
            (2, true, account.storage_password_last_set_binary_datetime)
        } else {
            let last_set = storage_password_binary_datetime();
            account.storage_password = new_password.to_string();
            account.storage_password_last_set_binary_datetime = last_set;
            (4, true, last_set)
        }
    };

    if result == 4 {
        if let Err(error) = config.save_account_store() {
            eprintln!("failed to persist account store: {error}");
        }
    }
    {
        let mut resources = world.resource_mut::<InventoryResource>();
        resources.storage_has_password = has_password;
        resources.storage_password_last_set_binary_datetime = last_set_binary_datetime;
        resources.storage_unlocked =
            result == 4 || !has_password || !config.require_storage_password;
    }

    vec![ServerPacket::StoragePasswordResult {
        result,
        removing: false,
        has_password,
        last_set_binary_datetime,
    }]
}

pub(super) fn remove_storage_password_impl(
    world: &mut World,
    current_password: &str,
) -> Vec<ServerPacket> {
    let (config, account_id) = {
        let config = world.resource::<RuntimeConfigResource>().config.clone();
        let session = world.resource::<SessionResource>();
        (
            config,
            session
                .account_id
                .clone()
                .unwrap_or_else(|| "demo".to_string()),
        )
    };
    let account_state = storage_account_state(&config, &account_id);
    if !active_crystal_storage_service(world) {
        let mut resources = world.resource_mut::<InventoryResource>();
        resources.storage_has_password = account_state.has_password;
        resources.storage_password_last_set_binary_datetime =
            account_state.storage_password_last_set_binary_datetime;
        return vec![ServerPacket::StoragePasswordResult {
            result: 0,
            removing: true,
            has_password: account_state.has_password,
            last_set_binary_datetime: account_state.storage_password_last_set_binary_datetime,
        }];
    }

    let (result, has_password, last_set_binary_datetime) = {
        let mut store = config
            .account_store
            .lock()
            .expect("account store mutex should not be poisoned");
        let account = store
            .accounts
            .entry(account_id)
            .or_insert_with(|| AccountRecord::new(config.default_character.clone()));
        if account.storage_password.is_empty() {
            (5, false, 0)
        } else if !crystal_password_is_valid(current_password) {
            (1, true, account.storage_password_last_set_binary_datetime)
        } else if account.storage_password != current_password {
            (2, true, account.storage_password_last_set_binary_datetime)
        } else {
            account.storage_password.clear();
            account.storage_password_last_set_binary_datetime = 0;
            (4, false, 0)
        }
    };

    if result == 4 {
        if let Err(error) = config.save_account_store() {
            eprintln!("failed to persist account store: {error}");
        }
    }
    {
        let mut resources = world.resource_mut::<InventoryResource>();
        resources.storage_has_password = has_password;
        resources.storage_password_last_set_binary_datetime = last_set_binary_datetime;
        resources.storage_unlocked = !has_password || !config.require_storage_password;
    }

    vec![ServerPacket::StoragePasswordResult {
        result,
        removing: true,
        has_password,
        last_set_binary_datetime,
    }]
}

pub(super) fn item_matches_client_reference(
    item: &ItemState,
    grid: MirGridType,
    unique_id: u64,
) -> bool {
    match grid {
        MirGridType::Belt => {
            item.container == ItemContainer::Belt && inventory_item_unique_id(item) == unique_id
        }
        MirGridType::QuestInventory => {
            item.container == ItemContainer::Quest && inventory_item_unique_id(item) == unique_id
        }
        MirGridType::Storage => {
            item.container == ItemContainer::Storage && inventory_item_unique_id(item) == unique_id
        }
        _ => {
            matches!(item.container, ItemContainer::Bag1 | ItemContainer::Bag2)
                && inventory_item_unique_id(item) == unique_id
        }
    }
}

pub(super) fn item_matches_inventory_unique_id(item: &ItemState, unique_id: u64) -> bool {
    matches!(item.container, ItemContainer::Bag1 | ItemContainer::Bag2)
        && inventory_item_unique_id(item) == unique_id
}

/// Protocol sidecars make `ItemState::unique_id` authoritative, including
/// exact zero. Sidecar-less legacy records retain the historical slot-derived
/// fallback so old saves continue to normalize deterministically.
fn inventory_item_unique_id(item: &ItemState) -> u64 {
    item_unique_id(item)
}

pub(super) fn item_index_for_client_reference(
    items: &[ItemState],
    grid: MirGridType,
    unique_id: u64,
) -> Option<usize> {
    items
        .iter()
        .position(|item| item_matches_client_reference(item, grid, unique_id))
}

fn user_item_tree_unique_id_is_used(item: &UserItem, unique_id: u64) -> bool {
    item.unique_id == unique_id
        || item
            .slots
            .iter()
            .flatten()
            .any(|embedded| user_item_tree_unique_id_is_used(embedded, unique_id))
}

fn metadata_unique_id_is_used(
    metadata: Option<&ItemStateUserItemMetadata>,
    unique_id: u64,
) -> bool {
    metadata.is_some_and(|metadata| {
        metadata
            .slots
            .iter()
            .flatten()
            .any(|embedded| user_item_tree_unique_id_is_used(embedded, unique_id))
    })
}

pub(super) fn item_tree_unique_id_is_used(item: &ItemState, unique_id: u64) -> bool {
    inventory_item_unique_id(item) == unique_id
        || metadata_unique_id_is_used(item.user_item_metadata.as_ref(), unique_id)
        || item
            .socketed
            .iter()
            .any(|embedded| item_tree_unique_id_is_used(embedded, unique_id))
}

pub(super) fn item_list_unique_id_is_used(items: &[ItemState], unique_id: u64) -> bool {
    items
        .iter()
        .any(|item| item_tree_unique_id_is_used(item, unique_id))
}

fn user_item_tree_max_unique_id(item: &UserItem) -> u64 {
    item.slots
        .iter()
        .flatten()
        .map(user_item_tree_max_unique_id)
        .fold(item.unique_id, u64::max)
}

fn metadata_max_unique_id(metadata: Option<&ItemStateUserItemMetadata>) -> u64 {
    metadata
        .into_iter()
        .flat_map(|metadata| metadata.slots.iter().flatten())
        .map(user_item_tree_max_unique_id)
        .max()
        .unwrap_or(0)
}

fn collect_user_item_tree_unique_ids(item: &UserItem, seen: &mut BTreeSet<u64>) {
    seen.insert(item.unique_id);
    for embedded in item.slots.iter().flatten() {
        collect_user_item_tree_unique_ids(embedded, seen);
    }
}

fn collect_metadata_unique_ids(
    metadata: Option<&ItemStateUserItemMetadata>,
    seen: &mut BTreeSet<u64>,
) {
    if let Some(metadata) = metadata {
        for embedded in metadata.slots.iter().flatten() {
            collect_user_item_tree_unique_ids(embedded, seen);
        }
    }
}

fn normalize_user_item_tree_unique_ids(
    item: &mut UserItem,
    seen: &mut BTreeSet<u64>,
    next_unique_id: &mut u64,
    preserve_exact_zero: bool,
) {
    let current_unique_id = item.unique_id;
    if preserve_exact_zero && current_unique_id == 0 && seen.insert(0) {
        // A protocol UserItem always carries an exact identity. Preserve only
        // the first exact zero; every later zero is a collision.
    } else if current_unique_id == 0 || !seen.insert(current_unique_id) {
        while *next_unique_id == 0 || seen.contains(&*next_unique_id) {
            *next_unique_id = next_unique_id.saturating_add(1);
        }
        item.unique_id = *next_unique_id;
        seen.insert(item.unique_id);
        *next_unique_id = next_unique_id.saturating_add(1);
    }
    for embedded in item.slots.iter_mut().flatten() {
        normalize_user_item_tree_unique_ids(embedded, seen, next_unique_id, preserve_exact_zero);
    }
}

fn clear_user_item_tree_unique_ids(item: &mut UserItem) {
    item.unique_id = 0;
    for embedded in item.slots.iter_mut().flatten() {
        clear_user_item_tree_unique_ids(embedded);
    }
}

/// The root protocol UID of an equipped item is distinct from the historical
/// equipment-slot packet UID. New saves retain the exact former inventory UID;
/// old saves intentionally fall back to the latter. In particular, `Some(0)`
/// is an exact captured marker and must not be replaced by the slot fallback.
fn equipment_root_unique_id(equipment: &super::equipment::EquipmentState) -> u64 {
    equipment
        .user_item_unique_id
        .unwrap_or_else(|| super::equipment::equipment_slot_unique_id(equipment.slot).unwrap_or(0))
}

fn equipment_tree_unique_id_is_used(
    equipment: &super::equipment::EquipmentState,
    unique_id: u64,
) -> bool {
    equipment_root_unique_id(equipment) == unique_id
        || metadata_unique_id_is_used(equipment.user_item_metadata.as_ref(), unique_id)
        || item_list_unique_id_is_used(&equipment.socketed, unique_id)
}

pub(super) fn inventory_unique_id_is_used(resources: &InventoryResource, unique_id: u64) -> bool {
    item_list_unique_id_is_used(&resources.belt_items, unique_id)
        || item_list_unique_id_is_used(&resources.inventory_items, unique_id)
        || item_list_unique_id_is_used(&resources.storage_items, unique_id)
        || resources
            .equipment_items
            .iter()
            .any(|equipment| equipment_tree_unique_id_is_used(equipment, unique_id))
}

fn item_tree_max_unique_id(item: &ItemState) -> u64 {
    item.socketed
        .iter()
        .map(item_tree_max_unique_id)
        .chain(std::iter::once(metadata_max_unique_id(
            item.user_item_metadata.as_ref(),
        )))
        .fold(inventory_item_unique_id(item), u64::max)
}

fn equipment_tree_max_unique_id(equipment: &super::equipment::EquipmentState) -> u64 {
    equipment
        .socketed
        .iter()
        .map(item_tree_max_unique_id)
        .chain(std::iter::once(metadata_max_unique_id(
            equipment.user_item_metadata.as_ref(),
        )))
        .fold(equipment_root_unique_id(equipment), u64::max)
}

fn collect_item_tree_unique_ids(items: &[ItemState], seen: &mut BTreeSet<u64>) {
    for item in items {
        seen.insert(inventory_item_unique_id(item));
        collect_metadata_unique_ids(item.user_item_metadata.as_ref(), seen);
        collect_item_tree_unique_ids(&item.socketed, seen);
    }
}

fn collect_inventory_unique_ids(resources: &InventoryResource, seen: &mut BTreeSet<u64>) {
    collect_item_tree_unique_ids(&resources.belt_items, seen);
    collect_item_tree_unique_ids(&resources.inventory_items, seen);
    collect_item_tree_unique_ids(&resources.storage_items, seen);
    for equipment in &resources.equipment_items {
        seen.insert(equipment_root_unique_id(equipment));
        collect_metadata_unique_ids(equipment.user_item_metadata.as_ref(), seen);
        collect_item_tree_unique_ids(&equipment.socketed, seen);
    }
}

fn inventory_max_unique_id(resources: &InventoryResource) -> u64 {
    resources
        .belt_items
        .iter()
        .chain(resources.inventory_items.iter())
        .chain(resources.storage_items.iter())
        .map(item_tree_max_unique_id)
        .chain(
            resources
                .equipment_items
                .iter()
                .map(equipment_tree_max_unique_id),
        )
        .max()
        .unwrap_or(0)
}

fn next_available_unique_id(resources: &InventoryResource, minimum: u64) -> u64 {
    let mut unique_id = minimum.max(1);
    while inventory_unique_id_is_used(resources, unique_id) {
        unique_id = unique_id.saturating_add(1);
    }
    unique_id
}

pub(super) fn allocate_item_unique_id(
    resources: &InventoryResource,
    container: ItemContainer,
    slot: u8,
) -> u64 {
    allocate_item_unique_id_avoiding(resources, container, slot, &[])
}

pub(super) fn allocate_item_unique_id_avoiding(
    resources: &InventoryResource,
    container: ItemContainer,
    slot: u8,
    reserved_items: &[ItemState],
) -> u64 {
    let preferred = default_item_unique_id(container, slot);
    if !inventory_unique_id_is_used(resources, preferred)
        && !item_list_unique_id_is_used(reserved_items, preferred)
    {
        return preferred;
    }

    let max_existing = reserved_items
        .iter()
        .map(item_tree_max_unique_id)
        .fold(inventory_max_unique_id(resources), u64::max);
    let mut unique_id = next_available_unique_id(resources, max_existing.saturating_add(1));
    while item_list_unique_id_is_used(reserved_items, unique_id) {
        unique_id = next_available_unique_id(resources, unique_id.saturating_add(1));
    }
    unique_id
}

/// Normalize an externally returned item tree before inserting it into the
/// active inventory. Existing global IDs and `reserved_items` are immutable;
/// the incoming parent and every socket are visited parent-first DFS. Valid,
/// non-conflicting IDs are preserved, while zeroes, global conflicts and
/// internal duplicates are reassigned deterministically above the complete
/// recursive high-water mark.
pub(super) fn normalize_incoming_item_tree_unique_ids(
    resources: &InventoryResource,
    item: &mut ItemState,
    reserved_items: &[ItemState],
) {
    normalize_incoming_item_tree_unique_ids_impl(resources, item, reserved_items, true);
}

fn normalize_incoming_item_tree_unique_ids_impl(
    resources: &InventoryResource,
    item: &mut ItemState,
    reserved_items: &[ItemState],
    preserve_exact_zero: bool,
) {
    let mut seen = BTreeSet::new();
    collect_inventory_unique_ids(resources, &mut seen);
    collect_item_tree_unique_ids(reserved_items, &mut seen);

    let incoming_max = item_tree_max_unique_id(item);
    let reserved_max = reserved_items
        .iter()
        .map(item_tree_max_unique_id)
        .max()
        .unwrap_or(0);
    let mut next_unique_id = inventory_max_unique_id(resources)
        .max(reserved_max)
        .max(incoming_max)
        .saturating_add(1)
        .max(1);

    fn normalize_tree(
        item: &mut ItemState,
        seen: &mut BTreeSet<u64>,
        next_unique_id: &mut u64,
        preserve_exact_zero: bool,
    ) {
        let current_unique_id = item.unique_id;
        let exact_zero =
            preserve_exact_zero && current_unique_id == 0 && item.user_item_metadata.is_some();
        if exact_zero && seen.insert(0) {
            // Preserve the first exact zero. A second zero is an impossible
            // protocol identity collision and is deterministically repaired.
        } else if current_unique_id == 0 || !seen.insert(current_unique_id) {
            while *next_unique_id == 0 || seen.contains(&*next_unique_id) {
                *next_unique_id = next_unique_id.saturating_add(1);
            }
            item.unique_id = *next_unique_id;
            seen.insert(item.unique_id);
            *next_unique_id = next_unique_id.saturating_add(1);
        }
        if let Some(metadata) = item.user_item_metadata.as_mut() {
            for embedded in metadata.slots.iter_mut().flatten() {
                normalize_user_item_tree_unique_ids(
                    embedded,
                    seen,
                    next_unique_id,
                    preserve_exact_zero,
                );
            }
        }
        for socketed in &mut item.socketed {
            normalize_tree(socketed, seen, next_unique_id, preserve_exact_zero);
        }
    }

    normalize_tree(item, &mut seen, &mut next_unique_id, preserve_exact_zero);
}

/// Assign new server identities to an entire delivered item tree. Exact mail
/// and GameShop attachments are grants, not returning inventory objects, so
/// sender-provided parent and embedded IDs must never survive collection.
pub(super) fn normalize_fresh_item_tree_unique_ids(
    resources: &InventoryResource,
    item: &mut ItemState,
    reserved_items: &[ItemState],
) {
    fn clear_tree(item: &mut ItemState) {
        item.unique_id = 0;
        if let Some(metadata) = item.user_item_metadata.as_mut() {
            for embedded in metadata.slots.iter_mut().flatten() {
                clear_user_item_tree_unique_ids(embedded);
            }
        }
        for socketed in &mut item.socketed {
            clear_tree(socketed);
        }
    }

    clear_tree(item);
    normalize_incoming_item_tree_unique_ids_impl(resources, item, reserved_items, false);
}

fn normalize_item_list_unique_ids(
    items: &mut [ItemState],
    seen: &mut BTreeSet<u64>,
    exact_equipment_root_unique_ids: &BTreeSet<u64>,
    next_unique_id: &mut u64,
) {
    // Exact metadata-backed identities participate in the shared global set.
    // Sidecar-less records remain grid-scoped legacy aliases, but still use a
    // local set so duplicate references inside one packet grid are repaired.
    // Exact equipped roots keep precedence over both forms for save compatibility.
    let mut local_seen = BTreeSet::new();
    for item in items {
        let exact_identity = item.user_item_metadata.is_some();
        let current_unique_id = inventory_item_unique_id(item);
        let collides_locally = local_seen.contains(&current_unique_id);
        let collides_globally = if exact_identity {
            seen.contains(&current_unique_id)
        } else {
            exact_equipment_root_unique_ids.contains(&current_unique_id)
        };
        if !collides_locally && !collides_globally {
            item.unique_id = current_unique_id;
            local_seen.insert(current_unique_id);
            if exact_identity {
                seen.insert(current_unique_id);
            }
            continue;
        }

        let preferred = default_item_unique_id(item.container, item.slot);
        let preferred_available = !local_seen.contains(&preferred)
            && if exact_identity {
                !seen.contains(&preferred)
            } else {
                !exact_equipment_root_unique_ids.contains(&preferred)
            };
        let normalized_unique_id = if preferred_available {
            preferred
        } else {
            while local_seen.contains(&*next_unique_id)
                || if exact_identity {
                    seen.contains(&*next_unique_id)
                } else {
                    exact_equipment_root_unique_ids.contains(&*next_unique_id)
                }
            {
                *next_unique_id = next_unique_id.saturating_add(1);
            }
            let allocated = *next_unique_id;
            *next_unique_id = next_unique_id.saturating_add(1);
            allocated
        };
        item.unique_id = normalized_unique_id;
        local_seen.insert(normalized_unique_id);
        if exact_identity {
            seen.insert(normalized_unique_id);
        }
    }
}

fn normalize_item_state_children_unique_ids(
    item: &mut ItemState,
    seen: &mut BTreeSet<u64>,
    next_unique_id: &mut u64,
) {
    if let Some(metadata) = item.user_item_metadata.as_mut() {
        for embedded in metadata.slots.iter_mut().flatten() {
            normalize_user_item_tree_unique_ids(embedded, seen, next_unique_id, true);
        }
    }
    normalize_embedded_item_unique_ids(&mut item.socketed, seen, next_unique_id);
}

fn normalize_embedded_item_unique_ids(
    items: &mut [ItemState],
    seen: &mut BTreeSet<u64>,
    next_unique_id: &mut u64,
) {
    for item in items {
        let current_unique_id = item.unique_id;
        if current_unique_id == 0 && item.user_item_metadata.is_some() && seen.insert(0) {
            // Preserve the first exact zero; repair subsequent collisions.
        } else if current_unique_id == 0 || !seen.insert(current_unique_id) {
            while *next_unique_id == 0 || seen.contains(&*next_unique_id) {
                *next_unique_id = next_unique_id.saturating_add(1);
            }
            item.unique_id = *next_unique_id;
            seen.insert(item.unique_id);
            *next_unique_id = next_unique_id.saturating_add(1);
        }
        normalize_item_state_children_unique_ids(item, seen, next_unique_id);
    }
}

pub(super) fn normalize_inventory_unique_ids(resources: &mut InventoryResource) {
    // Exact equipped roots retain their existing precedence. Every remaining root is
    // then normalized through one global belt -> inventory -> storage identity
    // set and allocator, so only the first occurrence survives a collision.
    let exact_equipment_root_unique_ids = resources
        .equipment_items
        .iter()
        .filter_map(|equipment| equipment.user_item_unique_id)
        .collect::<BTreeSet<_>>();
    let next_after_all_existing = inventory_max_unique_id(resources).saturating_add(1).max(1);

    let mut seen = exact_equipment_root_unique_ids.clone();
    let mut next_unique_id = next_after_all_existing;
    normalize_item_list_unique_ids(
        &mut resources.belt_items,
        &mut seen,
        &exact_equipment_root_unique_ids,
        &mut next_unique_id,
    );
    normalize_item_list_unique_ids(
        &mut resources.inventory_items,
        &mut seen,
        &exact_equipment_root_unique_ids,
        &mut next_unique_id,
    );
    normalize_item_list_unique_ids(
        &mut resources.storage_items,
        &mut seen,
        &exact_equipment_root_unique_ids,
        &mut next_unique_id,
    );

    // Embedded items (legacy metadata-only slots, ordinary sockets, and
    // MountSlot.Bells) continue from the same global identity state after every
    // equipped and top-level root.
    for item in &mut resources.belt_items {
        normalize_item_state_children_unique_ids(item, &mut seen, &mut next_unique_id);
    }
    for item in &mut resources.inventory_items {
        normalize_item_state_children_unique_ids(item, &mut seen, &mut next_unique_id);
    }
    for item in &mut resources.storage_items {
        normalize_item_state_children_unique_ids(item, &mut seen, &mut next_unique_id);
    }
    for equipment in &mut resources.equipment_items {
        if let Some(metadata) = equipment.user_item_metadata.as_mut() {
            for embedded in metadata.slots.iter_mut().flatten() {
                normalize_user_item_tree_unique_ids(embedded, &mut seen, &mut next_unique_id, true);
            }
        }
        normalize_embedded_item_unique_ids(&mut equipment.socketed, &mut seen, &mut next_unique_id);
    }
}

pub(super) fn item_heal_values_for_key(key: &str) -> (i32, i32) {
    match key {
        "red-potion" | "belt-red-potion" | "stored-red-potion" => (35, 0),
        "blue-potion" | "belt-blue-potion" => (0, 20),
        _ => (0, 0),
    }
}

fn normalize_item_list_known_metadata(items: &mut [ItemState]) {
    for item in items {
        let (heal_hp, heal_mp) = item_heal_values_for_key(&item.key);
        if heal_hp > 0 || heal_mp > 0 {
            item.heal_hp = heal_hp;
            item.heal_mp = heal_mp;
        }
    }
}

pub(super) fn normalize_inventory_known_item_metadata(resources: &mut InventoryResource) {
    normalize_item_list_known_metadata(&mut resources.belt_items);
    normalize_item_list_known_metadata(&mut resources.inventory_items);
    normalize_item_list_known_metadata(&mut resources.storage_items);
}

pub(super) fn item_key_for_client_reference(
    world: &World,
    unique_id: u64,
    grid: MirGridType,
) -> Option<String> {
    let resources = world.resource::<InventoryResource>();

    match grid {
        MirGridType::Belt => {
            item_index_for_client_reference(&resources.belt_items, grid, unique_id)
                .and_then(|index| resources.belt_items.get(index))
                .map(|item| item.key.clone())
        }
        MirGridType::QuestInventory => {
            item_index_for_client_reference(&resources.inventory_items, grid, unique_id)
                .and_then(|index| resources.inventory_items.get(index))
                .map(|item| item.key.clone())
        }
        _ => item_index_for_client_reference(&resources.inventory_items, grid, unique_id)
            .and_then(|index| resources.inventory_items.get(index))
            .map(|item| item.key.clone()),
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum UseItemLocation {
    Inventory(usize),
    Belt(usize),
}

pub(super) fn find_use_item_location(
    resources: &InventoryResource,
    key: &str,
    packet_ack: Option<(u64, MirGridType)>,
) -> Option<UseItemLocation> {
    match packet_ack {
        Some((unique_id, MirGridType::Belt)) => {
            item_index_for_client_reference(&resources.belt_items, MirGridType::Belt, unique_id)
                .map(UseItemLocation::Belt)
        }
        Some((unique_id, grid)) => {
            item_index_for_client_reference(&resources.inventory_items, grid, unique_id)
                .map(UseItemLocation::Inventory)
        }
        None => resources
            .inventory_items
            .iter()
            .position(|item| item.key == key)
            .map(UseItemLocation::Inventory),
    }
}

pub(super) fn item_at_use_location(
    resources: &InventoryResource,
    location: UseItemLocation,
) -> Option<ItemState> {
    match location {
        UseItemLocation::Inventory(index) => resources.inventory_items.get(index),
        UseItemLocation::Belt(index) => resources.belt_items.get(index),
    }
    .cloned()
}

pub(super) fn consume_item_at_use_location(world: &mut World, location: UseItemLocation) {
    let mut resources = world.resource_mut::<InventoryResource>();
    let items = match location {
        UseItemLocation::Inventory(_) => &mut resources.inventory_items,
        UseItemLocation::Belt(_) => &mut resources.belt_items,
    };
    let index = match location {
        UseItemLocation::Inventory(index) | UseItemLocation::Belt(index) => index,
    };
    if let Some(item) = items.get_mut(index) {
        if item.quantity > 1 {
            item.quantity -= 1;
        } else {
            items.remove(index);
        }
    }
}

pub(super) fn current_weight(resources: &InventoryResource) -> u16 {
    let total = resources
        .inventory_items
        .iter()
        .chain(resources.belt_items.iter())
        .map(ItemState::total_weight)
        .sum::<u32>();
    total.min(u32::from(u16::MAX)) as u16
}

pub(super) fn free_bag_slots(resources: &InventoryResource) -> u16 {
    let used = resources
        .inventory_items
        .iter()
        .filter(|item| matches!(item.container, ItemContainer::Bag1 | ItemContainer::Bag2))
        .count() as u16;
    crystal_bag_slot_capacity(resources.inventory_capacity).saturating_sub(used)
}

pub(super) fn item_containers_stack_together(
    existing: ItemContainer,
    target: ItemContainer,
) -> bool {
    match (existing, target) {
        (ItemContainer::Bag1, ItemContainer::Bag2) | (ItemContainer::Bag2, ItemContainer::Bag1) => {
            true
        }
        _ => existing == target,
    }
}

pub(super) fn empty_slots_for_inventory_container(
    items: &[ItemState],
    container: ItemContainer,
    inventory_capacity: u16,
) -> Vec<(ItemContainer, u8)> {
    match container {
        ItemContainer::Bag1 | ItemContainer::Bag2 => {
            (0..crystal_bag_slot_capacity(inventory_capacity))
                .filter_map(|logical_slot| {
                    let logical_slot = u8::try_from(logical_slot)
                        .expect("Crystal bag slot count should fit in u8");
                    let (container, slot) = inventory_container_and_slot_for_index(logical_slot)?;
                    let occupied = items
                        .iter()
                        .any(|item| item.container == container && item.slot == slot);
                    (!occupied).then_some((container, slot))
                })
                .collect()
        }
        other => {
            let max_slots = match other {
                ItemContainer::Quest => 40,
                ItemContainer::Storage => BASE_STORAGE_SLOTS,
                ItemContainer::Belt => 6,
                ItemContainer::Bag1 | ItemContainer::Bag2 => unreachable!(),
            };
            (0..max_slots)
                .filter_map(|slot| {
                    let slot = u8::try_from(slot).expect("slot count should fit in u8");
                    let occupied = items
                        .iter()
                        .any(|item| item.container == other && item.slot == slot);
                    (!occupied).then_some((other, slot))
                })
                .collect()
        }
    }
}

pub(super) fn find_empty_inventory_item_slot(
    items: &[ItemState],
    container: ItemContainer,
    inventory_capacity: u16,
) -> Option<(ItemContainer, u8)> {
    empty_slots_for_inventory_container(items, container, inventory_capacity)
        .into_iter()
        .next()
}

pub(super) fn additional_slots_needed_for_item_quantity(
    resources: &InventoryResource,
    container: ItemContainer,
    key: &str,
    quantity: u32,
) -> u32 {
    let max_stack = crystal_stack_size_for_item_key(key);
    let mut remaining = quantity.max(1);
    if max_stack > 1 {
        if matches!(container, ItemContainer::Bag1 | ItemContainer::Bag2) {
            for item in resources
                .belt_items
                .iter()
                .filter(|item| item.key == key && item.quantity < max_stack)
            {
                let available = max_stack.saturating_sub(item.quantity);
                remaining = remaining.saturating_sub(available);
                if remaining == 0 {
                    return 0;
                }
            }
        }
        for item in resources.inventory_items.iter().filter(|item| {
            item.key == key && item_containers_stack_together(item.container, container)
        }) {
            let available = max_stack.saturating_sub(item.quantity);
            remaining = remaining.saturating_sub(available);
            if remaining == 0 {
                return 0;
            }
        }
    }

    remaining.div_ceil(max_stack)
}

pub(super) fn can_gain_item_quantity(
    resources: &InventoryResource,
    container: ItemContainer,
    key: &str,
    quantity: u32,
) -> bool {
    let needed_slots =
        additional_slots_needed_for_item_quantity(resources, container, key, quantity);
    let free_slots = crystal_empty_add_item_slots(resources, container, key).len();
    needed_slots <= u32::try_from(free_slots).unwrap_or(u32::MAX)
}

fn ground_drop_user_item_uids_are_assigned(item: &UserItem) -> bool {
    item.unique_id != 0
        && item
            .slots
            .iter()
            .flatten()
            .all(ground_drop_user_item_uids_are_assigned)
}

#[allow(clippy::too_many_arguments)]
fn plan_exact_ground_drop_item(
    resources: &InventoryResource,
    container: ItemContainer,
    key: &str,
    name: &str,
    description: &str,
    preferred_slot: u8,
    expected_quantity: u32,
    payload: &GroundDropItemPayload,
) -> Option<(InventoryResource, Vec<ItemState>)> {
    validate_committed_user_item_carrier(&payload.item).ok()?;
    if u32::from(payload.item.count) != expected_quantity || expected_quantity == 0 {
        return None;
    }
    if payload.uid_assigned && !ground_drop_user_item_uids_are_assigned(&payload.item) {
        return None;
    }

    let direct_template = crystal_item_template_for_item_key(key);
    let template = direct_template
        .clone()
        .or_else(|| crystal_item_by_index(payload.item.item_index))?;
    if payload.item.item_index != template.item_index {
        return None;
    }
    let canonical_key = direct_template
        .map(|_| key.to_string())
        .unwrap_or_else(|| crystal_item_key_for_template(&template));

    let mut base = embedded_item_state_from_template(&template, container, preferred_slot);
    base.key = canonical_key.clone();
    base.name = name.to_string();
    base.description = description.to_string();
    let mut canonical = try_item_state_from_user_item(base, &payload.item).ok()?;
    canonical.container = container;
    canonical.slot = preferred_slot;

    let max_stack = u32::from(template.stack_size.max(1));
    if payload.uid_assigned && expected_quantity > max_stack {
        return None;
    }

    let mut staged = resources.clone();
    if payload.uid_assigned {
        if max_stack > 1 {
            let mut merge_capacity = 0_u32;
            if matches!(container, ItemContainer::Bag1 | ItemContainer::Bag2) {
                merge_capacity = merge_capacity.saturating_add(
                    staged
                        .belt_items
                        .iter()
                        .filter(|item| {
                            item.key == canonical_key
                                && item.quantity < max_stack
                                && item_stack_identity_compatible(item, &canonical)
                        })
                        .map(|item| max_stack.saturating_sub(item.quantity))
                        .sum::<u32>(),
                );
            }
            merge_capacity = merge_capacity.saturating_add(
                staged
                    .inventory_items
                    .iter()
                    .filter(|item| {
                        item.key == canonical_key
                            && item_containers_stack_together(item.container, container)
                            && item.quantity < max_stack
                            && item_stack_identity_compatible(item, &canonical)
                    })
                    .map(|item| max_stack.saturating_sub(item.quantity))
                    .sum::<u32>(),
            );

            // An assigned source UID is retired only when the entire source
            // stack is absorbed. This prevents a partial merge from either
            // duplicating the source UID or silently changing exact identity.
            if merge_capacity >= expected_quantity {
                let mut remaining = expected_quantity;
                let mut changed = Vec::new();
                if matches!(container, ItemContainer::Bag1 | ItemContainer::Bag2) {
                    for existing in staged.belt_items.iter_mut().filter(|item| {
                        item.key == canonical_key
                            && item.quantity < max_stack
                            && item_stack_identity_compatible(item, &canonical)
                    }) {
                        let added = remaining.min(max_stack - existing.quantity);
                        if added == 0 {
                            continue;
                        }
                        existing.quantity += added;
                        remaining -= added;
                        validate_committed_item_state_carrier(existing).ok()?;
                        changed.push(existing.clone());
                        if remaining == 0 {
                            return Some((staged, changed));
                        }
                    }
                }
                for existing in staged.inventory_items.iter_mut().filter(|item| {
                    item.key == canonical_key
                        && item_containers_stack_together(item.container, container)
                        && item.quantity < max_stack
                        && item_stack_identity_compatible(item, &canonical)
                }) {
                    let added = remaining.min(max_stack - existing.quantity);
                    if added == 0 {
                        continue;
                    }
                    existing.quantity += added;
                    remaining -= added;
                    validate_committed_item_state_carrier(existing).ok()?;
                    changed.push(existing.clone());
                    if remaining == 0 {
                        return Some((staged, changed));
                    }
                }
                return None;
            }
        }

        let exact_before = try_user_item_from_item_state(&canonical).ok()?;
        normalize_incoming_item_tree_unique_ids(resources, &mut canonical, &[]);
        if try_user_item_from_item_state(&canonical).ok()? != exact_before {
            return None;
        }
        let (item_container, slot) =
            crystal_empty_add_item_slots(&staged, container, &canonical_key)
                .into_iter()
                .next()
                .or_else(|| {
                    find_empty_inventory_item_slot(
                        &staged.inventory_items,
                        container,
                        staged.inventory_capacity,
                    )
                    .or(Some((container, preferred_slot)))
                    .filter(|(candidate_container, candidate_slot)| {
                        !collection_slot_occupied(&staged, *candidate_container, *candidate_slot)
                    })
                })?;
        canonical.container = item_container;
        canonical.slot = slot;
        validate_committed_item_state_carrier(&canonical).ok()?;
        if item_container == ItemContainer::Belt {
            staged.belt_items.push(canonical.clone());
        } else {
            staged.inventory_items.push(canonical.clone());
        }
        return Some((staged, vec![canonical]));
    }

    let mut remaining = expected_quantity;
    let mut changed = Vec::new();
    if max_stack > 1 {
        if matches!(container, ItemContainer::Bag1 | ItemContainer::Bag2) {
            for existing in staged.belt_items.iter_mut().filter(|item| {
                item.key == canonical_key
                    && item.quantity < max_stack
                    && item_stack_identity_compatible(item, &canonical)
            }) {
                let added = remaining.min(max_stack - existing.quantity);
                if added == 0 {
                    continue;
                }
                existing.quantity += added;
                remaining -= added;
                validate_committed_item_state_carrier(existing).ok()?;
                changed.push(existing.clone());
                if remaining == 0 {
                    return Some((staged, changed));
                }
            }
        }
        for existing in staged.inventory_items.iter_mut().filter(|item| {
            item.key == canonical_key
                && item_containers_stack_together(item.container, container)
                && item.quantity < max_stack
                && item_stack_identity_compatible(item, &canonical)
        }) {
            let added = remaining.min(max_stack - existing.quantity);
            if added == 0 {
                continue;
            }
            existing.quantity += added;
            remaining -= added;
            validate_committed_item_state_carrier(existing).ok()?;
            changed.push(existing.clone());
            if remaining == 0 {
                return Some((staged, changed));
            }
        }
    }

    while remaining > 0 {
        let (item_container, slot) =
            crystal_empty_add_item_slots(&staged, container, &canonical_key)
                .into_iter()
                .next()
                .or_else(|| {
                    find_empty_inventory_item_slot(
                        &staged.inventory_items,
                        container,
                        staged.inventory_capacity,
                    )
                    .or(Some((container, preferred_slot)))
                    .filter(|(candidate_container, candidate_slot)| {
                        !collection_slot_occupied(&staged, *candidate_container, *candidate_slot)
                    })
                })?;
        let mut item = canonical.clone();
        item.container = item_container;
        item.slot = slot;
        item.quantity = remaining.min(max_stack);
        normalize_fresh_item_tree_unique_ids(&staged, &mut item, &[]);
        validate_committed_item_state_carrier(&item).ok()?;
        if item_container == ItemContainer::Belt {
            staged.belt_items.push(item.clone());
        } else {
            staged.inventory_items.push(item.clone());
        }
        remaining -= item.quantity;
        changed.push(item);
    }

    Some((staged, changed))
}
#[allow(clippy::too_many_arguments)]
pub(super) fn can_gain_exact_ground_drop_item(
    world: &World,
    container: ItemContainer,
    key: &str,
    name: &str,
    description: &str,
    preferred_slot: u8,
    expected_quantity: u32,
    payload: &GroundDropItemPayload,
) -> bool {
    plan_exact_ground_drop_item(
        world.resource::<InventoryResource>(),
        container,
        key,
        name,
        description,
        preferred_slot,
        expected_quantity,
        payload,
    )
    .is_some()
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn add_exact_ground_drop_item(
    world: &mut World,
    container: ItemContainer,
    key: &str,
    name: &str,
    description: &str,
    preferred_slot: u8,
    expected_quantity: u32,
    payload: &GroundDropItemPayload,
) -> Option<ItemState> {
    add_exact_ground_drop_items(
        world,
        container,
        key,
        name,
        description,
        preferred_slot,
        expected_quantity,
        payload,
    )?
    .into_iter()
    .last()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn add_exact_ground_drop_items(
    world: &mut World,
    container: ItemContainer,
    key: &str,
    name: &str,
    description: &str,
    preferred_slot: u8,
    expected_quantity: u32,
    payload: &GroundDropItemPayload,
) -> Option<Vec<ItemState>> {
    let (staged, changed_items) = {
        let resources = world.resource::<InventoryResource>();
        plan_exact_ground_drop_item(
            resources,
            container,
            key,
            name,
            description,
            preferred_slot,
            expected_quantity,
            payload,
        )?
    };
    *world.resource_mut::<InventoryResource>() = staged;
    Some(changed_items)
}
pub(super) fn add_or_increment_item(
    world: &mut World,
    container: ItemContainer,
    key: &str,
    name: &str,
    description: &str,
    preferred_slot: u8,
    quantity: u32,
    weight: u16,
) -> ItemState {
    add_or_increment_item_with_durability(
        world,
        container,
        key,
        name,
        description,
        preferred_slot,
        quantity,
        weight,
        None,
        None,
    )
}

pub(super) fn add_or_increment_item_with_durability(
    world: &mut World,
    container: ItemContainer,
    key: &str,
    name: &str,
    description: &str,
    preferred_slot: u8,
    quantity: u32,
    weight: u16,
    durability_current: Option<u16>,
    durability_max: Option<u16>,
) -> ItemState {
    add_or_increment_item_with_durability_and_stats(
        world,
        container,
        key,
        name,
        description,
        preferred_slot,
        quantity,
        weight,
        durability_current,
        durability_max,
        0,
        0,
    )
}

pub(super) fn add_or_increment_item_with_durability_and_stats(
    world: &mut World,
    container: ItemContainer,
    key: &str,
    name: &str,
    description: &str,
    preferred_slot: u8,
    quantity: u32,
    weight: u16,
    durability_current: Option<u16>,
    durability_max: Option<u16>,
    added_attack: i32,
    added_defence: i32,
) -> ItemState {
    add_or_increment_item_with_random_metadata(
        world,
        container,
        key,
        name,
        description,
        preferred_slot,
        quantity,
        weight,
        durability_current,
        durability_max,
        added_attack,
        added_defence,
        Vec::new(),
        false,
        0,
    )
}

pub(super) fn add_or_increment_item_with_random_metadata(
    world: &mut World,
    container: ItemContainer,
    key: &str,
    name: &str,
    description: &str,
    preferred_slot: u8,
    quantity: u32,
    weight: u16,
    durability_current: Option<u16>,
    durability_max: Option<u16>,
    added_attack: i32,
    added_defence: i32,
    added_stats: Vec<UserItemStat>,
    cursed: bool,
    socket_slots: u8,
) -> ItemState {
    let crystal_template = crystal_item_template_for_item_key(key);
    let template_durability = crystal_template
        .as_ref()
        .map(|template| template.durability)
        .filter(|durability| *durability > 0);
    let durability_current = durability_current.or(template_durability);
    let durability_max = durability_max.or(template_durability);
    let grade = crystal_template
        .as_ref()
        .map(|template| match template.grade {
            1 => ItemGrade::Common,
            2 => ItemGrade::Rare,
            3 => ItemGrade::Legendary,
            _ => ItemGrade::None,
        })
        .unwrap_or(ItemGrade::None);
    let attack = crystal_template
        .as_ref()
        .map(|template| crystal_item_stat_value(template, CRYSTAL_STAT_MAX_DC))
        .unwrap_or_default();
    let defence = crystal_template
        .as_ref()
        .map(|template| crystal_item_stat_value(template, CRYSTAL_STAT_MAX_AC))
        .unwrap_or_default();
    let socket_slots = crystal_template
        .as_ref()
        .map(|template| socket_slots.max(template.slots))
        .unwrap_or(socket_slots);
    let max_stack = crystal_stack_size_for_item_key(key);
    let mut remaining = quantity.max(1);
    let mut resources = world.resource_mut::<InventoryResource>();
    let mut last_changed = None;
    let has_per_item_metadata = durability_current.is_some()
        || durability_max.is_some()
        || added_attack != 0
        || added_defence != 0
        || !added_stats.is_empty()
        || cursed
        || socket_slots != 0;
    if max_stack > 1 && !has_per_item_metadata {
        if matches!(container, ItemContainer::Bag1 | ItemContainer::Bag2) {
            for existing in resources
                .belt_items
                .iter_mut()
                .filter(|item| item.key == key && item.quantity < max_stack)
            {
                let added = remaining.min(max_stack - existing.quantity);
                if added == 0 {
                    continue;
                }
                existing.quantity += added;
                remaining -= added;
                last_changed = Some(existing.clone());
                if remaining == 0 {
                    return last_changed.expect("changed stack should exist");
                }
            }
        }
        for existing in resources.inventory_items.iter_mut().filter(|item| {
            item.key == key
                && item_containers_stack_together(item.container, container)
                && item.quantity < max_stack
        }) {
            let added = remaining.min(max_stack - existing.quantity);
            if added == 0 {
                continue;
            }
            existing.quantity += added;
            remaining -= added;
            last_changed = Some(existing.clone());
            if remaining == 0 {
                return last_changed.expect("changed stack should exist");
            }
        }
    }

    while remaining > 0 || last_changed.is_none() {
        let stack_quantity = remaining.min(max_stack);
        let (item_container, slot) = match crystal_empty_add_item_slots(&resources, container, key)
            .into_iter()
            .next()
        {
            Some(slot) => slot,
            None if last_changed.is_some() => break,
            None => match find_empty_inventory_item_slot(
                &resources.inventory_items,
                container,
                resources.inventory_capacity,
            ) {
                Some(slot) => slot,
                None if last_changed.is_some() => break,
                None => (container, preferred_slot),
            },
        };
        let item = ItemState {
            key: key.to_string(),
            name: name.to_string(),
            icon: item_icon_for_key(key),
            slot,
            unique_id: allocate_item_unique_id(&resources, item_container, slot),
            container: item_container,
            quantity: stack_quantity,
            description: description.to_string(),
            durability_current,
            durability_max,
            weight,
            equip_slot: crystal_equipment_slot_for_item_key(key),
            grade,
            added_attack,
            added_defence,
            added_stats: added_stats.clone(),
            socketed: Vec::new(),
            user_item_metadata: None,
            cursed,
            socket_slots,
            gem_count: 0,
            identified: None,
            soul_bound_id: None,
            sealed_expiry_time_binary_datetime: 0,
            sealed_next_time_binary_datetime: 0,
            rental_binding_flags: 0,
            rental_owner_name: String::new(),
            rental_expiry_binary_datetime: 0,
            rental_locked: false,
            attack,
            defence,
            heal_hp: 0,
            heal_mp: 0,
        };
        if item_container == ItemContainer::Belt {
            resources.belt_items.push(item.clone());
        } else {
            resources.inventory_items.push(item.clone());
        }
        last_changed = Some(item);
        if remaining <= stack_quantity {
            break;
        }
        remaining -= stack_quantity;
    }

    last_changed.expect("added item should exist")
}

pub(super) fn crystal_empty_add_item_slots(
    resources: &InventoryResource,
    container: ItemContainer,
    key: &str,
) -> Vec<(ItemContainer, u8)> {
    let mut slots = Vec::new();
    match container {
        ItemContainer::Bag1 | ItemContainer::Bag2 => {
            if let Some((start, end)) = crystal_belt_slot_range_for_item_key(key) {
                slots.extend((start..end).filter_map(|slot| {
                    let occupied = resources
                        .belt_items
                        .iter()
                        .any(|item| item.container == ItemContainer::Belt && item.slot == slot);
                    (!occupied).then_some((ItemContainer::Belt, slot))
                }));
            }
            slots.extend(empty_slots_for_inventory_container(
                &resources.inventory_items,
                container,
                resources.inventory_capacity,
            ));
        }
        ItemContainer::Belt => {
            slots.extend(empty_slots_for_inventory_container(
                &resources.belt_items,
                container,
                resources.inventory_capacity,
            ));
        }
        other => {
            slots.extend(empty_slots_for_inventory_container(
                &resources.inventory_items,
                other,
                resources.inventory_capacity,
            ));
        }
    }
    slots
}

pub(super) fn find_empty_storage_slot(items: &[ItemState], max_slots: u16) -> Option<u8> {
    (0..max_slots)
        .map(|slot| u8::try_from(slot).expect("storage slot count should fit in u8"))
        .find(|slot| !items.iter().any(|item| item.slot == *slot))
}

pub(super) fn inventory_container_and_slot_for_index(index: u8) -> Option<(ItemContainer, u8)> {
    match index {
        0..=39 => Some((ItemContainer::Bag1, index)),
        40..=79 => Some((ItemContainer::Bag2, index - 40)),
        _ => None,
    }
}

pub(super) fn inventory_index_for_item(item: &ItemState) -> Option<u8> {
    match item.container {
        ItemContainer::Bag1 => Some(item.slot),
        ItemContainer::Bag2 => Some(40u8.saturating_add(item.slot)),
        _ => None,
    }
}

pub(super) fn inventory_item_matches_index(item: &ItemState, index: u8) -> bool {
    inventory_index_for_item(item).is_some_and(|item_index| item_index == index)
}

pub(super) fn is_valid_inventory_slot(slot: u8, inventory_capacity: u16) -> bool {
    u16::from(slot) < crystal_bag_slot_capacity(inventory_capacity)
        && inventory_container_and_slot_for_index(slot).is_some()
}

pub(super) fn move_item_slot_matches_grid(item: &ItemState, grid: MirGridType, slot: u8) -> bool {
    if item.slot != slot && !matches!(grid, MirGridType::Inventory) {
        return false;
    }

    match grid {
        MirGridType::Inventory => inventory_item_matches_index(item, slot),
        MirGridType::Storage => item.container == ItemContainer::Storage,
        _ => false,
    }
}

pub(super) fn storage_slot_within_limit(slot: u8, storage_slot_limit: u16) -> bool {
    u16::from(slot) < storage_slot_limit
}

pub(super) fn crystal_item_can_merge_between_inventory_and_belt(key: &str) -> bool {
    crystal_belt_slot_range_for_item_key(key).is_some()
}

pub(super) fn belt_merge_item_pair_supported(
    from_items: &[ItemState],
    grid_from: MirGridType,
    id_from: u64,
    to_items: &[ItemState],
    grid_to: MirGridType,
    id_to: u64,
) -> bool {
    let Some(from_index) = item_index_for_client_reference(from_items, grid_from, id_from) else {
        return false;
    };
    let Some(to_index) = item_index_for_client_reference(to_items, grid_to, id_to) else {
        return false;
    };

    from_items[from_index].key == to_items[to_index].key
        && crystal_item_can_merge_between_inventory_and_belt(&from_items[from_index].key)
}

pub(super) fn collection_slot_occupied(
    resources: &InventoryResource,
    container: ItemContainer,
    slot: u8,
) -> bool {
    match container {
        ItemContainer::Bag1 | ItemContainer::Bag2 => resources
            .inventory_items
            .iter()
            .any(|item| item.container == container && item.slot == slot),
        ItemContainer::Storage => resources.storage_items.iter().any(|item| item.slot == slot),
        _ => false,
    }
}

pub(super) fn equipment_index_for_client_reference(
    resources: &InventoryResource,
    unique_id: u64,
) -> Option<usize> {
    resources
        .equipment_items
        .iter()
        .position(|item| super::equipment::equipment_slot_unique_id(item.slot) == Some(unique_id))
}

pub(super) fn remove_item_destination(
    resources: &InventoryResource,
    grid: MirGridType,
    to: i32,
) -> Option<(ItemContainer, u8)> {
    let slot = u8::try_from(to).ok()?;
    match grid {
        MirGridType::Inventory => is_valid_inventory_slot(slot, resources.inventory_capacity)
            .then(|| inventory_container_and_slot_for_index(slot))
            .flatten(),
        MirGridType::Storage => {
            is_valid_storage_slot(resources, slot).then_some((ItemContainer::Storage, slot))
        }
        _ => None,
    }
}

pub(super) fn store_item_impl(world: &mut World, from: i32, to: i32) -> Vec<ServerPacket> {
    let failed_packet = ServerPacket::StoreItem {
        from,
        to,
        success: false,
    };

    if !active_crystal_storage_service(world) {
        return vec![failed_packet];
    }

    if storage_locked(world) {
        return vec![failed_packet];
    }
    let Some(from_slot) = u8::try_from(from).ok() else {
        return vec![failed_packet];
    };
    let Some(to_slot) = u8::try_from(to).ok() else {
        return vec![failed_packet];
    };

    {
        let mut resources = world.resource_mut::<InventoryResource>();
        if !is_valid_inventory_slot(from_slot, resources.inventory_capacity)
            || !is_valid_storage_slot(&resources, to_slot)
        {
            return vec![failed_packet];
        }
        let Some(index) = resources
            .inventory_items
            .iter()
            .position(|item| inventory_item_matches_index(item, from_slot))
        else {
            return vec![failed_packet];
        };

        if crystal_item_has_bind_flag(
            &resources.inventory_items[index].key,
            CRYSTAL_BIND_DONT_STORE,
        ) || item_has_rental_bind_flag(
            &resources.inventory_items[index],
            CRYSTAL_BIND_DONT_STORE,
        ) {
            return vec![failed_packet];
        }

        if resources
            .storage_items
            .iter()
            .any(|item| item.slot == to_slot)
        {
            return vec![failed_packet];
        }

        let mut item = resources.inventory_items.remove(index);
        item.slot = to_slot;
        item.container = ItemContainer::Storage;
        if inventory_unique_id_is_used(&resources, inventory_item_unique_id(&item)) {
            item.unique_id = allocate_item_unique_id(&resources, item.container, item.slot);
        }
        resources.storage_items.push(item);
    }

    vec![ServerPacket::StoreItem {
        from,
        to,
        success: true,
    }]
}

pub(super) fn take_back_item_impl(world: &mut World, from: i32, to: i32) -> Vec<ServerPacket> {
    let failed_packet = ServerPacket::TakeBackItem {
        from,
        to,
        success: false,
    };

    if !active_crystal_storage_service(world) {
        return vec![failed_packet];
    }

    if storage_locked(world) {
        return vec![failed_packet];
    }
    let Some(from_slot) = u8::try_from(from).ok() else {
        return vec![failed_packet];
    };
    let Some(to_slot) = u8::try_from(to).ok() else {
        return vec![failed_packet];
    };

    {
        let mut resources = world.resource_mut::<InventoryResource>();
        if !is_valid_storage_slot(&resources, from_slot)
            || !is_valid_inventory_slot(to_slot, resources.inventory_capacity)
        {
            return vec![failed_packet];
        }
        let Some((to_container, to_inventory_slot)) =
            inventory_container_and_slot_for_index(to_slot)
        else {
            return vec![failed_packet];
        };
        let Some(index) = resources
            .storage_items
            .iter()
            .position(|item| item.slot == from_slot)
        else {
            return vec![failed_packet];
        };

        if resources
            .inventory_items
            .iter()
            .any(|item| inventory_item_matches_index(item, to_slot))
        {
            return vec![failed_packet];
        }

        let mut item = resources.storage_items.remove(index);
        item.slot = to_inventory_slot;
        item.container = to_container;
        if inventory_unique_id_is_used(&resources, inventory_item_unique_id(&item)) {
            item.unique_id = allocate_item_unique_id(&resources, item.container, item.slot);
        }
        resources.inventory_items.push(item);
    }

    vec![ServerPacket::TakeBackItem {
        from,
        to,
        success: true,
    }]
}

pub(super) fn move_item_impl(
    world: &mut World,
    grid: MirGridType,
    from: i32,
    to: i32,
) -> Vec<ServerPacket> {
    let failed_packet = ServerPacket::MoveItem {
        grid,
        from,
        to,
        success: false,
    };
    if !matches!(
        grid,
        MirGridType::Inventory
            | MirGridType::Storage
            | MirGridType::Trade
            | MirGridType::Refine
            | MirGridType::HeroInventory
    ) {
        return vec![failed_packet];
    }
    if matches!(grid, MirGridType::Storage) && !active_crystal_storage_service(world) {
        return vec![failed_packet];
    }
    if matches!(grid, MirGridType::Storage) && storage_locked(world) {
        return vec![failed_packet];
    }
    let Some(to_slot) = u8::try_from(to).ok() else {
        return vec![failed_packet];
    };
    let Some(from_slot) = u8::try_from(from).ok() else {
        return vec![failed_packet];
    };

    if matches!(grid, MirGridType::Inventory) {
        let resources = world.resource::<InventoryResource>();
        if !is_valid_inventory_slot(from_slot, resources.inventory_capacity)
            || !is_valid_inventory_slot(to_slot, resources.inventory_capacity)
        {
            return vec![failed_packet];
        }
    }

    if matches!(grid, MirGridType::Storage) {
        let resources = world.resource::<InventoryResource>();
        if !is_valid_storage_slot(&resources, from_slot)
            || !is_valid_storage_slot(&resources, to_slot)
        {
            return vec![failed_packet];
        }
    }

    let mut resources = world.resource_mut::<InventoryResource>();
    let items = match grid {
        MirGridType::Inventory => &mut resources.inventory_items,
        MirGridType::Storage => &mut resources.storage_items,
        _ => return vec![failed_packet],
    };

    let Some(index) = items
        .iter()
        .position(|item| move_item_slot_matches_grid(item, grid, from_slot))
    else {
        return match grid {
            MirGridType::Inventory | MirGridType::Storage => vec![
                super::session::system_message_key(world, "server.ItemMoveErrorReport"),
                failed_packet,
            ],
            _ => vec![failed_packet],
        };
    };

    match grid {
        MirGridType::Inventory => {
            let Some((from_container, from_inventory_slot)) =
                inventory_container_and_slot_for_index(from_slot)
            else {
                return vec![failed_packet];
            };
            let Some((to_container, to_inventory_slot)) =
                inventory_container_and_slot_for_index(to_slot)
            else {
                return vec![failed_packet];
            };

            if let Some(other_index) = items
                .iter()
                .position(|item| inventory_item_matches_index(item, to_slot))
            {
                items[other_index].slot = from_inventory_slot;
                items[other_index].container = from_container;
            }
            items[index].slot = to_inventory_slot;
            items[index].container = to_container;
        }
        MirGridType::Storage => {
            if let Some(other_index) = items.iter().position(|item| item.slot == to_slot) {
                items[other_index].slot = from_slot;
            }
            items[index].slot = to_slot;
        }
        _ => return vec![failed_packet],
    }

    vec![ServerPacket::MoveItem {
        grid,
        from,
        to,
        success: true,
    }]
}

pub(super) fn merge_item_impl(
    world: &mut World,
    grid_from: MirGridType,
    grid_to: MirGridType,
    id_from: u64,
    id_to: u64,
) -> Vec<ServerPacket> {
    let failed_packet = ServerPacket::MergeItem {
        grid_from,
        grid_to,
        id_from,
        id_to,
        success: false,
    };
    if matches!(
        grid_from,
        MirGridType::HeroInventory
            | MirGridType::HeroEquipment
            | MirGridType::Equipment
            | MirGridType::Fishing
            | MirGridType::QuestInventory
            | MirGridType::Trade
            | MirGridType::Refine
    ) || matches!(
        grid_to,
        MirGridType::HeroInventory
            | MirGridType::HeroEquipment
            | MirGridType::Equipment
            | MirGridType::Fishing
            | MirGridType::QuestInventory
            | MirGridType::Trade
            | MirGridType::Refine
    ) {
        return vec![failed_packet];
    }
    if (matches!(grid_from, MirGridType::Storage) || matches!(grid_to, MirGridType::Storage))
        && !active_crystal_storage_service(world)
    {
        return vec![failed_packet];
    }
    if matches!(grid_from, MirGridType::Storage) || matches!(grid_to, MirGridType::Storage) {
        if storage_locked(world) {
            return vec![failed_packet];
        }
    }

    let mut resources = world.resource_mut::<InventoryResource>();
    let storage_slot_limit = accessible_storage_size(&resources);
    let success = match (grid_from, grid_to) {
        (MirGridType::Belt, MirGridType::Belt) => {
            merge_item_within_collection(&mut resources.belt_items, grid_from, id_from, id_to, None)
        }
        (MirGridType::Inventory, MirGridType::Inventory)
        | (MirGridType::QuestInventory, MirGridType::QuestInventory) => {
            merge_item_within_collection(
                &mut resources.inventory_items,
                grid_from,
                id_from,
                id_to,
                None,
            )
        }
        (MirGridType::Storage, MirGridType::Storage) => merge_item_within_collection(
            &mut resources.storage_items,
            grid_from,
            id_from,
            id_to,
            Some(storage_slot_limit),
        ),
        (MirGridType::Inventory, MirGridType::Storage) => {
            let InventoryResource {
                inventory_items,
                storage_items,
                ..
            } = &mut *resources;
            merge_item_across_collections(
                inventory_items,
                grid_from,
                id_from,
                storage_items,
                grid_to,
                id_to,
                Some(storage_slot_limit),
            )
        }
        (MirGridType::Storage, MirGridType::Inventory) => {
            let InventoryResource {
                inventory_items,
                storage_items,
                ..
            } = &mut *resources;
            merge_item_across_collections(
                storage_items,
                grid_from,
                id_from,
                inventory_items,
                grid_to,
                id_to,
                Some(storage_slot_limit),
            )
        }
        (MirGridType::Inventory, MirGridType::Belt) => {
            if !belt_merge_item_pair_supported(
                &resources.inventory_items,
                grid_from,
                id_from,
                &resources.belt_items,
                grid_to,
                id_to,
            ) {
                false
            } else {
                let InventoryResource {
                    inventory_items,
                    belt_items,
                    ..
                } = &mut *resources;
                merge_item_across_collections(
                    inventory_items,
                    grid_from,
                    id_from,
                    belt_items,
                    grid_to,
                    id_to,
                    None,
                )
            }
        }
        (MirGridType::Belt, MirGridType::Inventory) => {
            if !belt_merge_item_pair_supported(
                &resources.belt_items,
                grid_from,
                id_from,
                &resources.inventory_items,
                grid_to,
                id_to,
            ) {
                false
            } else {
                let InventoryResource {
                    inventory_items,
                    belt_items,
                    ..
                } = &mut *resources;
                merge_item_across_collections(
                    belt_items,
                    grid_from,
                    id_from,
                    inventory_items,
                    grid_to,
                    id_to,
                    None,
                )
            }
        }
        _ if grid_from != grid_to => return vec![failed_packet],
        _ => return vec![failed_packet],
    };
    if !success {
        return vec![failed_packet];
    }

    vec![ServerPacket::MergeItem {
        grid_from,
        grid_to,
        id_from,
        id_to,
        success: true,
    }]
}

fn item_stack_identity_compatible(left: &ItemState, right: &ItemState) -> bool {
    let (Ok(mut left_protocol), Ok(mut right_protocol)) = (
        try_user_item_from_item_state(left),
        try_user_item_from_item_state(right),
    ) else {
        return false;
    };

    // Only the root objects are the two stacks being merged. Normalize their
    // location-scoped identity and count while retaining every nested UID and
    // count inside UserItem::slots for the complete equality comparison.
    left_protocol.unique_id = 0;
    right_protocol.unique_id = 0;
    left_protocol.count = 0;
    right_protocol.count = 0;

    left_protocol == right_protocol && item_state_functional_identity_compatible(left, right)
}

fn item_state_socket_authority_is_empty(metadata: &ItemStateUserItemMetadata) -> bool {
    metadata.slots.is_empty()
        && metadata
            .captured_socket_positions
            .as_ref()
            .is_none_or(Vec::is_empty)
        && metadata.captured_socket_position.is_none()
}

fn item_state_socket_authority_compatible(
    left: Option<&ItemStateUserItemMetadata>,
    right: Option<&ItemStateUserItemMetadata>,
) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            left.live_socketed_at_capture == right.live_socketed_at_capture
                && left.socket_layout_hydrated == right.socket_layout_hydrated
                && left.captured_socket_positions == right.captured_socket_positions
                && left.captured_socket_position == right.captured_socket_position
        }
        (None, Some(metadata)) | (Some(metadata), None) => {
            item_state_socket_authority_is_empty(metadata)
        }
    }
}

/// Compare ItemState-only functional identity after both trees have passed the
/// bounded protocol conversion. Iteration avoids a second unbudgeted recursive
/// traversal; try_user_item_from_item_state has already bounded both trees.
fn item_state_functional_identity_compatible(left: &ItemState, right: &ItemState) -> bool {
    let mut pending = vec![(left, right)];
    while let Some((left, right)) = pending.pop() {
        let left_metadata = left.user_item_metadata.as_ref();
        let right_metadata = right.user_item_metadata.as_ref();
        if left.key != right.key
            || left.name != right.name
            || left.icon != right.icon
            || left.description != right.description
            || left.weight != right.weight
            || left.equip_slot != right.equip_slot
            || left.grade != right.grade
            || left.attack != right.attack
            || left.defence != right.defence
            || left.heal_hp != right.heal_hp
            || left.heal_mp != right.heal_mp
            || !item_state_socket_authority_compatible(left_metadata, right_metadata)
            || left.socketed.len() != right.socketed.len()
        {
            return false;
        }
        pending.extend(left.socketed.iter().zip(&right.socketed));
    }
    true
}

/// Splitting clones the complete ItemState carrier. Until nested identities
/// have an allocator-aware clone path, reject any embedded item instead of
/// duplicating its protocol UID into two stacks.
fn item_has_nested_identity_for_split(item: &ItemState) -> bool {
    !item.socketed.is_empty()
        || item.user_item_metadata.as_ref().is_some_and(|metadata| {
            metadata.slots.iter().any(Option::is_some)
                || metadata
                    .captured_socket_positions
                    .as_ref()
                    .is_some_and(|positions| positions.iter().any(Option::is_some))
        })
}

pub(super) fn merge_item_within_collection(
    items: &mut Vec<ItemState>,
    grid: MirGridType,
    id_from: u64,
    id_to: u64,
    storage_slot_limit: Option<u16>,
) -> bool {
    let Some(from_index) = item_index_for_client_reference(items, grid, id_from) else {
        return false;
    };
    let Some(to_index) = item_index_for_client_reference(items, grid, id_to) else {
        return false;
    };
    if from_index == to_index {
        return false;
    }
    if let Some(limit) = storage_slot_limit {
        if !storage_slot_within_limit(items[from_index].slot, limit)
            || !storage_slot_within_limit(items[to_index].slot, limit)
        {
            return false;
        }
    }
    if !item_stack_identity_compatible(&items[from_index], &items[to_index]) {
        return false;
    }

    let max_stack = crystal_stack_size_for_item_key(&items[to_index].key);
    if max_stack <= 1 || items[to_index].quantity >= max_stack {
        return false;
    }

    let quantity = items[from_index].quantity;
    let available_space = max_stack - items[to_index].quantity;
    if quantity <= available_space {
        items[to_index].quantity += quantity;
        items.remove(from_index);
    } else {
        items[from_index].quantity -= available_space;
        items[to_index].quantity = max_stack;
    }

    true
}

pub(super) fn merge_item_across_collections(
    from_items: &mut Vec<ItemState>,
    grid_from: MirGridType,
    id_from: u64,
    to_items: &mut Vec<ItemState>,
    grid_to: MirGridType,
    id_to: u64,
    storage_slot_limit: Option<u16>,
) -> bool {
    let Some(from_index) = item_index_for_client_reference(from_items, grid_from, id_from) else {
        return false;
    };
    let Some(to_index) = item_index_for_client_reference(to_items, grid_to, id_to) else {
        return false;
    };
    if let Some(limit) = storage_slot_limit {
        if matches!(grid_from, MirGridType::Storage)
            && !storage_slot_within_limit(from_items[from_index].slot, limit)
        {
            return false;
        }
        if matches!(grid_to, MirGridType::Storage)
            && !storage_slot_within_limit(to_items[to_index].slot, limit)
        {
            return false;
        }
    }
    if !item_stack_identity_compatible(&from_items[from_index], &to_items[to_index]) {
        return false;
    }

    let max_stack = crystal_stack_size_for_item_key(&to_items[to_index].key);
    if max_stack <= 1 || to_items[to_index].quantity >= max_stack {
        return false;
    }

    let quantity = from_items[from_index].quantity;
    let available_space = max_stack - to_items[to_index].quantity;
    if quantity <= available_space {
        to_items[to_index].quantity += quantity;
        from_items.remove(from_index);
    } else {
        from_items[from_index].quantity -= available_space;
        to_items[to_index].quantity = max_stack;
    }

    true
}

pub(super) fn split_item_impl(
    world: &mut World,
    grid: MirGridType,
    unique_id: u64,
    count: u16,
) -> Vec<ServerPacket> {
    let failed_packet = ServerPacket::SplitItem1 {
        grid,
        unique_id,
        count,
        success: false,
    };

    if !matches!(grid, MirGridType::Inventory | MirGridType::Storage) {
        return vec![failed_packet];
    }

    if matches!(grid, MirGridType::Storage) && !active_crystal_storage_service(world) {
        return vec![failed_packet];
    }

    if matches!(grid, MirGridType::Storage) && storage_locked(world) {
        return vec![failed_packet];
    }
    if count == 0 {
        return vec![failed_packet];
    }

    let mut resources = world.resource_mut::<InventoryResource>();
    let split_packet_item = match grid {
        MirGridType::Storage => {
            let Some(index) = resources
                .storage_items
                .iter()
                .position(|item| item_matches_client_reference(item, grid, unique_id))
            else {
                return vec![failed_packet];
            };
            if resources.storage_items[index].quantity <= u32::from(count) {
                return vec![failed_packet];
            }
            if item_has_nested_identity_for_split(&resources.storage_items[index]) {
                return vec![failed_packet];
            }
            let storage_slot_limit = accessible_storage_size(&resources);
            let Some(next_slot) =
                find_empty_storage_slot(&resources.storage_items, storage_slot_limit)
            else {
                return vec![failed_packet];
            };

            let mut split = resources.storage_items[index].clone();
            split.slot = next_slot;
            // Crystal's storage item identity is scoped by the Storage grid;
            // the empty slot is therefore the canonical ID even when another
            // grid uses the same numeric value.
            split.unique_id = default_item_unique_id(split.container, next_slot);
            split.quantity = u32::from(count);
            let Ok(split_packet_item) = try_user_item_from_item_state(&split) else {
                return vec![failed_packet];
            };

            resources.storage_items[index].quantity -= u32::from(count);
            resources.storage_items.push(split);
            split_packet_item
        }
        MirGridType::Inventory => {
            let Some(index) = resources
                .inventory_items
                .iter()
                .position(|item| item_matches_client_reference(item, grid, unique_id))
            else {
                return vec![failed_packet];
            };
            if resources.inventory_items[index].quantity <= u32::from(count) {
                return vec![failed_packet];
            }
            if item_has_nested_identity_for_split(&resources.inventory_items[index]) {
                return vec![failed_packet];
            }
            let source_container = resources.inventory_items[index].container;
            let source_key = resources.inventory_items[index].key.clone();
            let Some((next_container, next_slot)) =
                crystal_empty_add_item_slots(&resources, source_container, &source_key)
                    .into_iter()
                    .next()
            else {
                return vec![failed_packet];
            };

            let mut split = resources.inventory_items[index].clone();
            split.container = next_container;
            split.slot = next_slot;
            split.unique_id = allocate_item_unique_id(&resources, split.container, next_slot);
            split.quantity = u32::from(count);
            let Ok(split_packet_item) = try_user_item_from_item_state(&split) else {
                return vec![failed_packet];
            };

            resources.inventory_items[index].quantity -= u32::from(count);
            match split.container {
                ItemContainer::Belt => resources.belt_items.push(split),
                _ => resources.inventory_items.push(split),
            }
            split_packet_item
        }
        _ => unreachable!("unsupported SplitItem grids return early"),
    };

    vec![
        ServerPacket::SplitItem1 {
            grid,
            unique_id,
            count,
            success: true,
        },
        ServerPacket::SplitItem {
            item: Some(split_packet_item),
            grid,
        },
    ]
}

pub(super) fn delete_item_impl(
    world: &mut World,
    unique_id: u64,
    count: u16,
    _hero_inventory: bool,
) -> Vec<ServerPacket> {
    if current_player_is_dead(world) {
        return vec![ServerPacket::DeleteItem { unique_id, count }];
    }

    {
        let mut resources = world.resource_mut::<InventoryResource>();
        if let Some(index) = resources
            .inventory_items
            .iter()
            .position(|item| item_matches_inventory_unique_id(item, unique_id))
        {
            let item_count = resources.inventory_items[index].quantity;
            let requested = u32::from(count);
            let delete_count = if requested == 0 || requested > item_count {
                item_count
            } else {
                requested
            };

            if delete_count >= item_count {
                resources.inventory_items.remove(index);
            } else {
                resources.inventory_items[index].quantity -= delete_count;
            }
        }
    }

    vec![ServerPacket::DeleteItem { unique_id, count }]
}

#[cfg(test)]
mod stack_identity_tests {
    use super::*;
    use mir2_protocol::{Point, UserItemRentalInformation, UserItemSealedInfo};

    use super::super::components::{Npc, ObjectId, Position, SelfPlayer};
    use super::super::equipment::EquipmentState;
    use super::super::npc::ActiveNpcServiceState;
    use super::super::resources::NpcStateResource;

    fn identity_stack(
        unique_id: u64,
        slot: u8,
        container: ItemContainer,
        quantity: u32,
    ) -> ItemState {
        crystal_fixture_item("red-potion", unique_id, slot, container, quantity)
    }

    fn identity_socket(
        unique_id: u64,
        slot: u8,
        container: ItemContainer,
        quantity: u32,
    ) -> ItemState {
        // BronzeBell is a real Crystal ItemType.Bells carrier. It keeps nested
        // identity comparisons on a catalog-valid socket tree.
        crystal_fixture_item("crystal-item-778", unique_id, slot, container, quantity)
    }

    fn crystal_fixture_item(
        key: &str,
        unique_id: u64,
        slot: u8,
        container: ItemContainer,
        quantity: u32,
    ) -> ItemState {
        let template = crystal_item_template_for_item_key(key)
            .unwrap_or_else(|| panic!("Crystal fixture {key} must exist"));
        seed_item(
            key,
            &template.name,
            slot,
            unique_id,
            container,
            quantity,
            template.tooltip.as_deref().unwrap_or_default(),
            None,
            None,
            u16::from(template.weight),
            crystal_equipment_slot_for_template(&template),
            ItemGrade::None,
            0,
            0,
            0,
            0,
            0,
            0,
        )
    }

    fn metadata(awake_type: u8) -> ItemStateUserItemMetadata {
        ItemStateUserItemMetadata {
            item_index: Some(
                crystal_item_template_for_item_key("red-potion")
                    .expect("red-potion Crystal fixture must exist")
                    .item_index,
            ),
            awake_type,
            awake_values: vec![awake_type],
            refined_value: 1,
            refine_added: 2,
            refine_success_chance: 3,
            wedding_ring: -1,
            expire_info: None,
            rental_information: None,
            sealed_info: None,
            slots: Vec::new(),
            is_shop_item: false,
            gm_made: false,
            live_socketed_at_capture: false,
            socket_layout_hydrated: false,
            captured_socket_positions: None,
            captured_socket_position: None,
        }
    }

    fn live_socket_metadata() -> ItemStateUserItemMetadata {
        let mut metadata = metadata(1);
        metadata.slots = vec![None];
        metadata.live_socketed_at_capture = true;
        metadata
    }

    fn equipped_item_with_root_uid(
        slot: EquipmentSlot,
        user_item_unique_id: Option<u64>,
    ) -> EquipmentState {
        EquipmentState {
            key: "uid-reservation-equipment".to_string(),
            slot,
            quantity: 1,
            name: "UID Reservation Equipment".to_string(),
            icon: 0,
            shape: None,
            description: String::new(),
            durability_current: 1,
            durability_max: 1,
            grade: ItemGrade::None,
            added_attack: 0,
            added_defence: 0,
            added_luck: 0,
            added_stats: Vec::new(),
            socketed: Vec::new(),
            cursed: false,
            socket_slots: 0,
            gem_count: 0,
            awake_type: 0,
            awake_values: Vec::new(),
            user_item_metadata: None,
            user_item_unique_id,
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
        }
    }

    fn assert_within_merge_rejected_without_mutation(from: ItemState, to: ItemState) {
        let mut items = vec![from, to];
        let before = items.clone();

        assert!(!merge_item_within_collection(
            &mut items,
            MirGridType::Inventory,
            before[0].unique_id,
            before[1].unique_id,
            None,
        ));
        assert_eq!(format!("{items:?}"), format!("{before:?}"));
    }

    fn storage_world_with_item(item: ItemState) -> World {
        const STORAGE_NPC_OBJECT_ID: u32 = 4_294_960_001;

        let mut world = World::new();
        let mut inventory = InventoryResource::new(BASE_STORAGE_SLOTS);
        inventory.storage_items.push(item);
        world.insert_resource(inventory);
        world.insert_resource(RuntimeConfigResource::new(&SimulationConfig::default()));
        world.insert_resource(NpcStateResource::new());
        world.spawn((SelfPlayer, Position(Point { x: 10, y: 10 })));
        world.spawn((
            Npc,
            ObjectId(STORAGE_NPC_OBJECT_ID),
            Position(Point { x: 10, y: 10 }),
        ));
        world.resource_mut::<NpcStateResource>().active_npc_service = Some(ActiveNpcServiceState {
            script_key: "identity-test-storage".to_string(),
            label_key: "STORAGE".to_string(),
            npc_object_id: STORAGE_NPC_OBJECT_ID,
        });
        world
    }

    fn add_over_budget_stats(item: &mut ItemState) {
        item.added_stats = (0..257)
            .map(|index| UserItemStat {
                stat: (index % 255) as u8,
                value: index + 1,
            })
            .collect();
    }
    #[test]
    fn equipped_root_uids_are_reserved_for_allocation_and_incoming_rekeys() {
        let mut resources = InventoryResource::new(BASE_STORAGE_SLOTS);
        resources
            .equipment_items
            .push(equipped_item_with_root_uid(EquipmentSlot::Weapon, Some(44)));
        resources
            .equipment_items
            .push(equipped_item_with_root_uid(EquipmentSlot::Armour, None));
        resources
            .equipment_items
            .push(equipped_item_with_root_uid(EquipmentSlot::Helmet, Some(0)));
        resources.equipment_items[0]
            .socketed
            .push(identity_stack(45, 0, ItemContainer::Bag1, 1));

        // Explicit roots, legacy slot fallback, exact zero, and nested sockets
        // all reserve their identity before any fresh item is allocated.
        for unique_id in [0, 1, 44, 45] {
            assert!(inventory_unique_id_is_used(&resources, unique_id));
        }
        let allocated = allocate_item_unique_id(&resources, ItemContainer::Bag1, 44);
        assert_ne!(allocated, 44);
        assert_ne!(allocated, 45);
        assert!(allocated > 45);

        let mut incoming = identity_stack(44, 7, ItemContainer::Bag1, 1);
        normalize_incoming_item_tree_unique_ids(&resources, &mut incoming, &[]);
        assert_ne!(incoming.unique_id, 44);
        assert_ne!(incoming.unique_id, 45);
        assert_ne!(incoming.unique_id, 0);

        // Load-time normalization reserves the worn root as well, without
        // rewriting the equipped item's captured UID or its nested socket.
        resources
            .inventory_items
            .push(identity_stack(44, 8, ItemContainer::Bag1, 1));
        normalize_inventory_unique_ids(&mut resources);
        assert_ne!(resources.inventory_items[0].unique_id, 44);
        assert_eq!(resources.equipment_items[0].user_item_unique_id, Some(44));
        assert_eq!(resources.equipment_items[0].socketed[0].unique_id, 45);
    }

    #[test]
    fn seeded_legacy_packet_items_remain_addressable_after_global_normalization() {
        let mut resources = InventoryResource::new(BASE_STORAGE_SLOTS);
        resources.belt_items = seed_belt_items();
        resources.inventory_items = seed_inventory_items();
        resources.storage_items = seed_storage_items();
        resources.equipment_items = super::super::equipment::seed_equipment_items();
        assert!(resources
            .belt_items
            .iter()
            .chain(resources.inventory_items.iter())
            .chain(resources.storage_items.iter())
            .all(|item| item.user_item_metadata.is_none()));

        let expected_belt = resources
            .belt_items
            .iter()
            .map(item_unique_id)
            .collect::<Vec<_>>();
        let expected_inventory = resources
            .inventory_items
            .iter()
            .map(item_unique_id)
            .collect::<Vec<_>>();
        let expected_storage = resources
            .storage_items
            .iter()
            .map(item_unique_id)
            .collect::<Vec<_>>();

        normalize_inventory_unique_ids(&mut resources);

        assert_eq!(
            resources
                .belt_items
                .iter()
                .map(inventory_item_unique_id)
                .collect::<Vec<_>>(),
            expected_belt
        );
        assert_eq!(
            resources
                .inventory_items
                .iter()
                .map(inventory_item_unique_id)
                .collect::<Vec<_>>(),
            expected_inventory
        );
        assert_eq!(
            resources
                .storage_items
                .iter()
                .map(inventory_item_unique_id)
                .collect::<Vec<_>>(),
            expected_storage
        );

        for (index, unique_id) in expected_belt.iter().copied().enumerate() {
            assert_eq!(
                item_index_for_client_reference(
                    &resources.belt_items,
                    MirGridType::Belt,
                    unique_id,
                ),
                Some(index)
            );
        }
        for (index, unique_id) in expected_inventory.iter().copied().enumerate() {
            assert_eq!(
                item_index_for_client_reference(
                    &resources.inventory_items,
                    MirGridType::Inventory,
                    unique_id,
                ),
                Some(index)
            );
            assert!(item_matches_inventory_unique_id(
                &resources.inventory_items[index],
                unique_id
            ));
        }
        for (index, unique_id) in expected_storage.iter().copied().enumerate() {
            assert_eq!(
                item_index_for_client_reference(
                    &resources.storage_items,
                    MirGridType::Storage,
                    unique_id,
                ),
                Some(index)
            );
        }

        assert!(matches!(
            find_use_item_location(&resources, "red-potion", Some((0, MirGridType::Inventory)),),
            Some(UseItemLocation::Inventory(0))
        ));
        assert!(matches!(
            find_use_item_location(&resources, "belt-red-potion", Some((0, MirGridType::Belt)),),
            Some(UseItemLocation::Belt(0))
        ));
        let equip_index = resources
            .inventory_items
            .iter()
            .position(|item| item.equip_slot == Some(EquipmentSlot::Helmet))
            .expect("seeded helmet remains available to EquipItem");
        let equip_unique_id = expected_inventory[equip_index];
        assert_eq!(
            item_index_for_client_reference(
                &resources.inventory_items,
                MirGridType::Inventory,
                equip_unique_id,
            ),
            Some(equip_index)
        );
    }

    #[test]
    fn normalization_rekeys_cross_container_zero_and_nonzero_collisions_globally() {
        let mut belt_zero = identity_stack(0, 0, ItemContainer::Belt, 1);
        belt_zero.user_item_metadata = Some(metadata(1));
        let mut belt_nonzero = identity_stack(700, 1, ItemContainer::Belt, 1);
        belt_nonzero.user_item_metadata = Some(metadata(1));
        let mut inventory_zero = identity_stack(0, 8, ItemContainer::Bag1, 1);
        inventory_zero.user_item_metadata = Some(metadata(1));
        let mut inventory_nonzero = identity_stack(700, 9, ItemContainer::Bag1, 1);
        inventory_nonzero.user_item_metadata = Some(metadata(1));
        let mut storage_zero = identity_stack(0, 2, ItemContainer::Storage, 1);
        storage_zero.user_item_metadata = Some(metadata(1));
        let mut storage_nonzero = identity_stack(700, 3, ItemContainer::Storage, 1);
        storage_nonzero.user_item_metadata = Some(metadata(1));

        let mut resources = InventoryResource::new(BASE_STORAGE_SLOTS);
        resources.belt_items = vec![belt_zero, belt_nonzero];
        resources.inventory_items = vec![inventory_zero, inventory_nonzero];
        resources.storage_items = vec![storage_zero, storage_nonzero];

        normalize_inventory_unique_ids(&mut resources);

        let root_ids = resources
            .belt_items
            .iter()
            .chain(resources.inventory_items.iter())
            .chain(resources.storage_items.iter())
            .map(inventory_item_unique_id)
            .collect::<Vec<_>>();
        assert_eq!(root_ids, vec![0, 700, 8, 9, 2, 3]);
        assert_eq!(
            root_ids
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            root_ids.len(),
            "all top-level container identities must be globally unique"
        );

        assert_eq!(
            item_index_for_client_reference(&resources.belt_items, MirGridType::Belt, 0),
            Some(0)
        );
        assert_eq!(
            item_index_for_client_reference(&resources.inventory_items, MirGridType::Inventory, 8,),
            Some(0)
        );
        assert_eq!(
            item_index_for_client_reference(&resources.storage_items, MirGridType::Storage, 2),
            Some(0)
        );
        assert_eq!(
            item_index_for_client_reference(&resources.inventory_items, MirGridType::Inventory, 0,),
            None,
            "the later inventory zero must no longer alias the belt zero"
        );
        assert_eq!(
            item_index_for_client_reference(&resources.storage_items, MirGridType::Storage, 700,),
            None,
            "the later storage nonzero collision must be rekeyed"
        );

        normalize_inventory_unique_ids(&mut resources);
        let reloaded_ids = resources
            .belt_items
            .iter()
            .chain(resources.inventory_items.iter())
            .chain(resources.storage_items.iter())
            .map(inventory_item_unique_id)
            .collect::<Vec<_>>();
        assert_eq!(reloaded_ids, root_ids, "normalization must be stable");
    }

    #[test]
    fn normalization_preserves_metadata_exact_zero_and_rekeys_legacy_zero() {
        let mut exact_zero = identity_stack(0, 8, ItemContainer::Bag1, 1);
        exact_zero.user_item_metadata = Some(metadata(1));
        let legacy_zero = identity_stack(0, 9, ItemContainer::Bag1, 1);
        let mut second_exact_zero = identity_stack(0, 10, ItemContainer::Bag1, 1);
        second_exact_zero.user_item_metadata = Some(metadata(2));
        let mut resources = InventoryResource::new(BASE_STORAGE_SLOTS);
        resources.inventory_items = vec![exact_zero, legacy_zero, second_exact_zero];

        normalize_inventory_unique_ids(&mut resources);

        let exact_zero = resources
            .inventory_items
            .iter()
            .find(|item| item.slot == 8)
            .expect("exact-zero fixture remains in its slot");
        assert_eq!(exact_zero.unique_id, 0);
        assert_eq!(item_unique_id(exact_zero), 0);
        assert!(item_matches_inventory_unique_id(exact_zero, 0));
        assert!(!item_matches_inventory_unique_id(exact_zero, 8));
        let second_exact_zero = resources
            .inventory_items
            .iter()
            .find(|item| item.slot == 10)
            .expect("second exact-zero fixture remains in its slot");
        assert_ne!(
            second_exact_zero.unique_id, 0,
            "a duplicate exact zero must be deterministically repaired"
        );
        let legacy_zero = resources
            .inventory_items
            .iter()
            .find(|item| item.slot == 9)
            .expect("legacy-zero fixture remains in its slot");
        assert_eq!(legacy_zero.unique_id, 9);
        assert!(item_matches_inventory_unique_id(legacy_zero, 9));
        assert!(!item_matches_inventory_unique_id(legacy_zero, 0));

        let encoded = serde_json::to_string(&resources.inventory_items)
            .expect("normalized inventory should encode");
        resources.inventory_items =
            serde_json::from_str(&encoded).expect("normalized inventory should reload");
        normalize_inventory_unique_ids(&mut resources);
        assert_eq!(
            resources
                .inventory_items
                .iter()
                .find(|item| item.slot == 8)
                .expect("exact-zero fixture reloads")
                .unique_id,
            0
        );

        let mut incoming_exact = identity_stack(0, 10, ItemContainer::Bag1, 1);
        incoming_exact.user_item_metadata = Some(metadata(1));
        let empty = InventoryResource::new(BASE_STORAGE_SLOTS);
        normalize_incoming_item_tree_unique_ids(&empty, &mut incoming_exact, &[]);
        assert_eq!(incoming_exact.unique_id, 0);

        let mut occupied = InventoryResource::new(BASE_STORAGE_SLOTS);
        occupied.inventory_items.push(incoming_exact.clone());
        let mut colliding_exact = identity_stack(0, 11, ItemContainer::Bag1, 1);
        colliding_exact.user_item_metadata = Some(metadata(3));
        normalize_incoming_item_tree_unique_ids(&occupied, &mut colliding_exact, &[]);
        assert_ne!(
            colliding_exact.unique_id, 0,
            "an incoming second exact zero must be rekeyed instead of remaining ambiguous"
        );

        normalize_fresh_item_tree_unique_ids(&empty, &mut incoming_exact, &[]);
        assert_ne!(incoming_exact.unique_id, 0);
    }

    #[test]
    fn metadata_difference_rejects_within_merge_without_mutation() {
        let mut items = vec![
            {
                let mut item = identity_stack(101, 1, ItemContainer::Bag1, 3);
                item.user_item_metadata = Some(metadata(1));
                item
            },
            {
                let mut item = identity_stack(102, 2, ItemContainer::Bag1, 5);
                item.user_item_metadata = Some(metadata(2));
                item
            },
        ];
        let before = items.clone();

        assert!(!merge_item_within_collection(
            &mut items,
            MirGridType::Inventory,
            101,
            102,
            None,
        ));
        assert_eq!(format!("{items:?}"), format!("{before:?}"));
    }

    #[test]
    fn fully_compatible_within_merge_succeeds() {
        let mut items = vec![
            identity_stack(201, 1, ItemContainer::Bag1, 3),
            identity_stack(202, 2, ItemContainer::Bag1, 5),
        ];

        assert!(merge_item_within_collection(
            &mut items,
            MirGridType::Inventory,
            201,
            202,
            None,
        ));
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].unique_id, 202);
        assert_eq!(items[0].quantity, 8);
    }

    #[test]
    fn nested_item_uid_or_count_difference_rejects_merge() {
        let mut from = identity_stack(211, 1, ItemContainer::Bag1, 3);
        from.socket_slots = 1;
        from.user_item_metadata = Some(live_socket_metadata());
        from.socketed
            .push(identity_socket(9_001, 0, ItemContainer::Bag1, 1));
        let mut to = identity_stack(212, 2, ItemContainer::Bag1, 5);
        to.socket_slots = 1;
        to.user_item_metadata = Some(live_socket_metadata());
        to.socketed
            .push(identity_socket(9_002, 0, ItemContainer::Bag1, 1));
        assert_within_merge_rejected_without_mutation(from, to);

        let mut from = identity_stack(213, 3, ItemContainer::Bag1, 3);
        from.socket_slots = 1;
        from.user_item_metadata = Some(live_socket_metadata());
        from.socketed
            .push(identity_socket(9_003, 0, ItemContainer::Bag1, 1));
        let mut to = identity_stack(214, 4, ItemContainer::Bag1, 5);
        to.socket_slots = 1;
        to.user_item_metadata = Some(live_socket_metadata());
        to.socketed
            .push(identity_socket(9_003, 0, ItemContainer::Bag1, 2));
        assert_within_merge_rejected_without_mutation(from, to);

        let nested = user_item_from_item_state(&identity_socket(9_004, 0, ItemContainer::Bag1, 1));
        let mut from = identity_stack(215, 5, ItemContainer::Bag1, 3);
        from.socket_slots = 1;
        let mut from_metadata = metadata(1);
        from_metadata.slots = vec![Some(nested.clone())];
        from.user_item_metadata = Some(from_metadata);
        let mut to = identity_stack(216, 6, ItemContainer::Bag1, 5);
        to.socket_slots = 1;
        let mut to_metadata = metadata(1);
        let mut different_nested_count = nested;
        different_nested_count.count = 2;
        to_metadata.slots = vec![Some(different_nested_count)];
        to.user_item_metadata = Some(to_metadata);
        assert_within_merge_rejected_without_mutation(from, to);
    }

    #[test]
    fn exact_item_index_difference_rejects_merge() {
        let mut from = identity_stack(221, 1, ItemContainer::Bag1, 3);
        let mut from_metadata = metadata(1);
        from_metadata.item_index = Some(-77);
        from.user_item_metadata = Some(from_metadata);

        let mut to = identity_stack(222, 2, ItemContainer::Bag1, 5);
        let mut to_metadata = metadata(1);
        to_metadata.item_index = Some(-78);
        to.user_item_metadata = Some(to_metadata);

        assert_within_merge_rejected_without_mutation(from, to);
    }

    #[test]
    fn default_optional_rental_or_sealed_presence_rejects_merge() {
        let mut from = identity_stack(223, 3, ItemContainer::Bag1, 3);
        let mut from_metadata = metadata(1);
        from_metadata.rental_information = Some(UserItemRentalInformation {
            owner_name: String::new(),
            binding_flags: 0,
            expiry_binary_datetime: 0,
            rental_locked: false,
        });
        from.user_item_metadata = Some(from_metadata);
        let mut to = identity_stack(224, 4, ItemContainer::Bag1, 5);
        to.user_item_metadata = Some(metadata(1));
        assert_within_merge_rejected_without_mutation(from, to);

        let mut from = identity_stack(225, 5, ItemContainer::Bag1, 3);
        let mut from_metadata = metadata(1);
        from_metadata.sealed_info = Some(UserItemSealedInfo {
            expiry_binary_datetime: 0,
            next_seal_binary_datetime: 0,
        });
        from.user_item_metadata = Some(from_metadata);
        let mut to = identity_stack(226, 6, ItemContainer::Bag1, 5);
        to.user_item_metadata = Some(metadata(1));
        assert_within_merge_rejected_without_mutation(from, to);
    }

    #[test]
    fn live_socket_authority_difference_rejects_merge() {
        let mut from = identity_stack(227, 7, ItemContainer::Bag1, 3);
        from.socket_slots = 1;
        let mut from_metadata = live_socket_metadata();
        from_metadata.live_socketed_at_capture = false;
        from.user_item_metadata = Some(from_metadata);
        from.socketed
            .push(identity_socket(9_005, 0, ItemContainer::Bag1, 1));

        let mut to = identity_stack(228, 8, ItemContainer::Bag1, 5);
        to.socket_slots = 1;
        let to_metadata = live_socket_metadata();
        to.user_item_metadata = Some(to_metadata);
        to.socketed
            .push(identity_socket(9_005, 0, ItemContainer::Bag1, 1));

        let mut from_protocol =
            try_user_item_from_item_state(&from).expect("from carrier should serialize");
        let mut to_protocol =
            try_user_item_from_item_state(&to).expect("to carrier should serialize");
        from_protocol.unique_id = 0;
        to_protocol.unique_id = 0;
        from_protocol.count = 0;
        to_protocol.count = 0;
        assert_eq!(
            from_protocol, to_protocol,
            "the authority marker itself is intentionally outside UserItem"
        );
        assert_within_merge_rejected_without_mutation(from, to);
    }
    #[test]
    fn metadata_difference_rejects_across_merge_without_mutation() {
        let mut from_items = vec![{
            let mut item = identity_stack(301, 1, ItemContainer::Bag1, 3);
            item.user_item_metadata = Some(metadata(1));
            item
        }];
        let mut to_items = vec![{
            let mut item = identity_stack(302, 2, ItemContainer::Storage, 5);
            item.user_item_metadata = Some(metadata(2));
            item
        }];
        let before_from = from_items.clone();
        let before_to = to_items.clone();

        assert!(!merge_item_across_collections(
            &mut from_items,
            MirGridType::Inventory,
            301,
            &mut to_items,
            MirGridType::Storage,
            302,
            None,
        ));
        assert_eq!(format!("{from_items:?}"), format!("{before_from:?}"));
        assert_eq!(format!("{to_items:?}"), format!("{before_to:?}"));
    }

    #[test]
    fn fully_compatible_across_merge_succeeds() {
        let mut from_items = vec![identity_stack(401, 1, ItemContainer::Bag1, 3)];
        let mut to_items = vec![identity_stack(402, 2, ItemContainer::Storage, 5)];

        assert!(merge_item_across_collections(
            &mut from_items,
            MirGridType::Inventory,
            401,
            &mut to_items,
            MirGridType::Storage,
            402,
            None,
        ));
        assert!(from_items.is_empty());
        assert_eq!(to_items.len(), 1);
        assert_eq!(to_items[0].unique_id, 402);
        assert_eq!(to_items[0].quantity, 8);
    }

    #[test]
    fn split_rejects_nested_socket_identity_without_mutation() {
        let mut source = identity_stack(501, 1, ItemContainer::Bag1, 6);
        source.socket_slots = 1;
        source
            .socketed
            .push(identity_stack(9001, 0, ItemContainer::Bag1, 1));

        let mut world = World::new();
        let mut resources = InventoryResource::new(BASE_STORAGE_SLOTS);
        resources.inventory_items.push(source);
        world.insert_resource(resources);

        let packets = split_item_impl(&mut world, MirGridType::Inventory, 501, 2);
        assert!(matches!(
            packets.as_slice(),
            [ServerPacket::SplitItem1 { success: false, .. }]
        ));

        let resources = world.resource::<InventoryResource>();
        assert_eq!(resources.inventory_items.len(), 1);
        assert_eq!(resources.inventory_items[0].quantity, 6);
        assert_eq!(resources.inventory_items[0].socketed.len(), 1);
        assert_eq!(resources.inventory_items[0].socketed[0].unique_id, 9001);
    }
    #[test]
    fn storage_split_rejects_nested_socket_identity_without_mutation() {
        let mut source = identity_stack(502, 1, ItemContainer::Storage, 6);
        source.socket_slots = 1;
        source
            .socketed
            .push(identity_stack(9_101, 0, ItemContainer::Bag1, 1));
        let mut world = storage_world_with_item(source);

        let packets = split_item_impl(&mut world, MirGridType::Storage, 502, 2);
        assert!(matches!(
            packets.as_slice(),
            [ServerPacket::SplitItem1 { success: false, .. }]
        ));

        let resources = world.resource::<InventoryResource>();
        assert_eq!(resources.storage_items.len(), 1);
        assert_eq!(resources.storage_items[0].quantity, 6);
        assert_eq!(resources.storage_items[0].socketed.len(), 1);
        assert_eq!(resources.storage_items[0].socketed[0].unique_id, 9_101);
    }

    #[test]
    fn split_conversion_error_preserves_inventory_and_storage_state() {
        let mut inventory_source = identity_stack(503, 1, ItemContainer::Bag1, 6);
        add_over_budget_stats(&mut inventory_source);
        let mut world = World::new();
        let mut inventory = InventoryResource::new(BASE_STORAGE_SLOTS);
        inventory.inventory_items.push(inventory_source);
        world.insert_resource(inventory);
        let before = format!(
            "{:?}",
            world.resource::<InventoryResource>().inventory_items
        );

        let packets = split_item_impl(&mut world, MirGridType::Inventory, 503, 2);
        assert!(matches!(
            packets.as_slice(),
            [ServerPacket::SplitItem1 { success: false, .. }]
        ));
        assert_eq!(
            format!(
                "{:?}",
                world.resource::<InventoryResource>().inventory_items
            ),
            before
        );

        let mut storage_source = identity_stack(504, 1, ItemContainer::Storage, 6);
        add_over_budget_stats(&mut storage_source);
        let mut world = storage_world_with_item(storage_source);
        let before = format!("{:?}", world.resource::<InventoryResource>().storage_items);

        let packets = split_item_impl(&mut world, MirGridType::Storage, 504, 2);
        assert!(matches!(
            packets.as_slice(),
            [ServerPacket::SplitItem1 { success: false, .. }]
        ));
        assert_eq!(
            format!("{:?}", world.resource::<InventoryResource>().storage_items),
            before
        );
    }
    #[test]
    fn metadata_only_child_uids_participate_in_usage_high_water_and_normalization() {
        let mut parent = identity_stack(100, 1, ItemContainer::Bag1, 1);
        parent.socket_slots = 1;
        let child = user_item_from_item_state(&identity_socket(9_000, 0, ItemContainer::Bag1, 1));
        let mut parent_metadata = metadata(1);
        parent_metadata.slots = vec![Some(child)];
        parent.user_item_metadata = Some(parent_metadata);

        let mut resources = InventoryResource::new(BASE_STORAGE_SLOTS);
        resources.inventory_items.push(parent);
        assert!(inventory_unique_id_is_used(&resources, 9_000));
        assert!(allocate_item_unique_id(&resources, ItemContainer::Bag1, 100) > 9_000);

        let mut incoming = identity_stack(101, 2, ItemContainer::Bag1, 1);
        incoming.socket_slots = 1;
        let mut incoming_metadata = metadata(2);
        incoming_metadata.slots = vec![Some(user_item_from_item_state(&identity_socket(
            9_000,
            0,
            ItemContainer::Bag1,
            1,
        )))];
        incoming.user_item_metadata = Some(incoming_metadata);
        normalize_incoming_item_tree_unique_ids(&resources, &mut incoming, &[]);
        let incoming_child_uid = incoming
            .user_item_metadata
            .as_ref()
            .and_then(|metadata| metadata.slots[0].as_ref())
            .expect("metadata-only child remains present")
            .unique_id;
        assert_ne!(incoming_child_uid, 9_000);
        assert!(incoming_child_uid > 9_000);

        resources.inventory_items.push(incoming);
        normalize_inventory_unique_ids(&mut resources);
        let mut all_ids = BTreeSet::new();
        collect_inventory_unique_ids(&resources, &mut all_ids);
        assert_eq!(
            all_ids.len(),
            4,
            "two roots and two metadata-only children must remain globally unique"
        );
    }

    #[test]
    fn fresh_and_equipment_metadata_only_children_cannot_reuse_existing_uids() {
        let mut equipment = equipped_item_with_root_uid(EquipmentSlot::Weapon, Some(44));
        equipment.socket_slots = 1;
        let mut equipment_metadata = metadata(1);
        equipment_metadata.slots = vec![Some(user_item_from_item_state(&identity_socket(
            46,
            0,
            ItemContainer::Bag1,
            1,
        )))];
        equipment.user_item_metadata = Some(equipment_metadata);

        let mut resources = InventoryResource::new(BASE_STORAGE_SLOTS);
        resources.equipment_items.push(equipment);
        assert!(inventory_unique_id_is_used(&resources, 46));

        let mut fresh = identity_stack(46, 3, ItemContainer::Bag1, 1);
        fresh.socket_slots = 1;
        let mut fresh_metadata = metadata(2);
        fresh_metadata.slots = vec![Some(user_item_from_item_state(&identity_socket(
            46,
            0,
            ItemContainer::Bag1,
            1,
        )))];
        fresh.user_item_metadata = Some(fresh_metadata);
        normalize_fresh_item_tree_unique_ids(&resources, &mut fresh, &[]);

        let fresh_child_uid = fresh
            .user_item_metadata
            .as_ref()
            .and_then(|metadata| metadata.slots[0].as_ref())
            .expect("fresh metadata-only child remains present")
            .unique_id;
        assert_ne!(fresh.unique_id, 0);
        assert_ne!(fresh.unique_id, 44);
        assert_ne!(fresh.unique_id, 46);
        assert_ne!(fresh_child_uid, 0);
        assert_ne!(fresh_child_uid, 44);
        assert_ne!(fresh_child_uid, 46);
        assert_ne!(fresh_child_uid, fresh.unique_id);
    }
}
