use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::{AccountRecord, EquipmentSlot, ItemContainer, ItemGrade, SimulationConfig};
use bevy_ecs::prelude::World;
use mir2_game_data::localized_text_or_fallback;
use mir2_protocol::{ChatType, MirGridType, ServerPacket, UserItemStat};

use super::components::current_player_is_dead;
use super::crystal_compat::{
    BASE_STORAGE_SLOTS, CRYSTAL_BIND_DONT_STORE, DOTNET_DATETIME_KIND_LOCAL,
    DOTNET_TICKS_AT_UNIX_EPOCH, EXPANDED_STORAGE_SLOTS,
};
use super::items::{
    crystal_belt_slot_range_for_item_key, crystal_equipment_slot_for_item_key,
    crystal_item_has_bind_flag, crystal_stack_size_for_item_key, default_item_unique_id,
    item_has_rental_bind_flag, item_icon_for_key, item_unique_id, user_item_from_item_state,
    ItemState,
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
            "training-manual",
            "Training Manual",
            2,
            2,
            ItemContainer::Bag1,
            1,
            "Reminds the tester to open inventory, belt, and character panes.",
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
            "belt-lantern-oil",
            "Lantern Oil",
            5,
            5,
            ItemContainer::Belt,
            1,
            "Placeholder consumable for later shortcut actions.",
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
            item.container == ItemContainer::Belt && item_unique_id(item) == unique_id
        }
        MirGridType::QuestInventory => {
            item.container == ItemContainer::Quest && item_unique_id(item) == unique_id
        }
        MirGridType::Storage => {
            item.container == ItemContainer::Storage && item_unique_id(item) == unique_id
        }
        _ => {
            matches!(item.container, ItemContainer::Bag1 | ItemContainer::Bag2)
                && item_unique_id(item) == unique_id
        }
    }
}

pub(super) fn item_matches_inventory_unique_id(item: &ItemState, unique_id: u64) -> bool {
    matches!(item.container, ItemContainer::Bag1 | ItemContainer::Bag2)
        && item_unique_id(item) == unique_id
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

fn inventory_unique_id_is_used(resources: &InventoryResource, unique_id: u64) -> bool {
    resources
        .belt_items
        .iter()
        .chain(resources.inventory_items.iter())
        .chain(resources.storage_items.iter())
        .any(|item| item_unique_id(item) == unique_id)
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
    let preferred = default_item_unique_id(container, slot);
    if !inventory_unique_id_is_used(resources, preferred) {
        return preferred;
    }

    let max_existing = resources
        .belt_items
        .iter()
        .chain(resources.inventory_items.iter())
        .chain(resources.storage_items.iter())
        .map(item_unique_id)
        .max()
        .unwrap_or(0);
    next_available_unique_id(resources, max_existing.saturating_add(1))
}

fn normalize_item_list_unique_ids(
    items: &mut [ItemState],
    seen: &mut BTreeSet<u64>,
    next_unique_id: &mut u64,
) {
    for item in items {
        let current_unique_id = item_unique_id(item);
        if seen.insert(current_unique_id) {
            item.unique_id = current_unique_id;
            continue;
        }

        let preferred = default_item_unique_id(item.container, item.slot);
        let normalized_unique_id = if !seen.contains(&preferred) {
            preferred
        } else {
            while seen.contains(&*next_unique_id) {
                *next_unique_id = next_unique_id.saturating_add(1);
            }
            let allocated = *next_unique_id;
            *next_unique_id = next_unique_id.saturating_add(1);
            allocated
        };
        item.unique_id = normalized_unique_id;
        seen.insert(normalized_unique_id);
    }
}

pub(super) fn normalize_inventory_unique_ids(resources: &mut InventoryResource) {
    let mut seen = BTreeSet::new();
    let mut next_unique_id = resources
        .belt_items
        .iter()
        .map(item_unique_id)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    normalize_item_list_unique_ids(&mut resources.belt_items, &mut seen, &mut next_unique_id);

    let mut seen = BTreeSet::new();
    let mut next_unique_id = resources
        .inventory_items
        .iter()
        .map(item_unique_id)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    normalize_item_list_unique_ids(
        &mut resources.inventory_items,
        &mut seen,
        &mut next_unique_id,
    );

    let mut seen = BTreeSet::new();
    let mut next_unique_id = resources
        .storage_items
        .iter()
        .map(item_unique_id)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    normalize_item_list_unique_ids(&mut resources.storage_items, &mut seen, &mut next_unique_id);
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
    80u16.saturating_sub(used)
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
) -> Vec<(ItemContainer, u8)> {
    match container {
        ItemContainer::Bag1 | ItemContainer::Bag2 => [ItemContainer::Bag1, ItemContainer::Bag2]
            .into_iter()
            .flat_map(|container| {
                (0..40).filter_map(move |slot| {
                    let slot = u8::try_from(slot).expect("bag slot count should fit in u8");
                    let occupied = items
                        .iter()
                        .any(|item| item.container == container && item.slot == slot);
                    (!occupied).then_some((container, slot))
                })
            })
            .collect(),
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
) -> Option<(ItemContainer, u8)> {
    empty_slots_for_inventory_container(items, container)
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
            None => match find_empty_inventory_item_slot(&resources.inventory_items, container) {
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
            grade: ItemGrade::None,
            added_attack,
            added_defence,
            added_stats: added_stats.clone(),
            socketed: Vec::new(),
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
            attack: 0,
            defence: 0,
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
            ));
        }
        ItemContainer::Belt => {
            slots.extend(empty_slots_for_inventory_container(
                &resources.belt_items,
                container,
            ));
        }
        other => {
            slots.extend(empty_slots_for_inventory_container(
                &resources.inventory_items,
                other,
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

pub(super) fn is_valid_inventory_slot(slot: u8) -> bool {
    inventory_container_and_slot_for_index(slot).is_some()
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
        MirGridType::Inventory => inventory_container_and_slot_for_index(slot),
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
        if !is_valid_inventory_slot(from_slot) || !is_valid_storage_slot(&resources, to_slot) {
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
        if inventory_unique_id_is_used(&resources, item_unique_id(&item)) {
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
        if !is_valid_storage_slot(&resources, from_slot) || !is_valid_inventory_slot(to_slot) {
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
        if inventory_unique_id_is_used(&resources, item_unique_id(&item)) {
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

    if matches!(grid, MirGridType::Inventory)
        && (!is_valid_inventory_slot(from_slot) || !is_valid_inventory_slot(to_slot))
    {
        return vec![failed_packet];
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
    if items[from_index].key != items[to_index].key {
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
    if from_items[from_index].key != to_items[to_index].key {
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
            let storage_slot_limit = accessible_storage_size(&resources);
            let Some(next_slot) =
                find_empty_storage_slot(&resources.storage_items, storage_slot_limit)
            else {
                return vec![failed_packet];
            };

            resources.storage_items[index].quantity -= u32::from(count);
            let mut split = resources.storage_items[index].clone();
            split.slot = next_slot;
            split.unique_id = default_item_unique_id(split.container, next_slot);
            split.quantity = u32::from(count);
            let split_packet_item = user_item_from_item_state(&split);
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
            let source_container = resources.inventory_items[index].container;
            let source_key = resources.inventory_items[index].key.clone();
            let Some((next_container, next_slot)) =
                crystal_empty_add_item_slots(&resources, source_container, &source_key)
                    .into_iter()
                    .next()
            else {
                return vec![failed_packet];
            };

            resources.inventory_items[index].quantity -= u32::from(count);
            let mut split = resources.inventory_items[index].clone();
            split.container = next_container;
            split.slot = next_slot;
            split.unique_id = allocate_item_unique_id(&resources, split.container, next_slot);
            split.quantity = u32::from(count);
            let split_packet_item = user_item_from_item_state(&split);
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
