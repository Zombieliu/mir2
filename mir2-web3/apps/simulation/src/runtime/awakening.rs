//! Crystal item awakening — the NPC awakening flow (`PlayerObject.Awakening` /
//! `AwakeningNeedMaterials` / `DowngradeAwakening` / `AwakeningLockedItem`,
//! `PlayerObject.cs:8806`). Awakening rolls an extra stat onto an inventory
//! item, consuming awakening orbs + gold; a failed roll destroys the item.
//!
//! Config is `Configs/AwakeningSystem.ini` (every material base = 1, rate = 1).
//! The roll/check/value model is shared with the `@AWAKENING` GM command via
//! `super::gm_commands`.

use bevy_ecs::prelude::World;
use mir2_game_data::crystal_item_manifest;
use mir2_protocol::{AwakeningMaterial, ServerPacket};

use super::gm_commands::{
    awake_check, awake_key_seed, awake_make_value, AWAKE_MAX_LEVEL, AWAKE_SUCCESS_RATE,
    BIND_DONT_UPGRADE,
};
use super::items::{
    crystal_item_template_for_item_key, item_info_from_crystal_template, user_item_from_item_state,
};
use super::monsters::deterministic_roll;
use super::resources::{InventoryResource, PlayerRuntimeResource};
use super::session::runtime_tick;

const CRYSTAL_ITEM_TYPE_AWAKENING: u8 = 35;
/// `Awake` orb whose `Shape` is `type - 1` is the type-specific material; the
/// shared second material is the `Shape == 100` orb (Crystal
/// `HasAwakeningNeedMaterials`).
const AWAKE_COMMON_ORB_SHAPE: i16 = 100;

/// Crystal `UserItem.AwakeningPrice()` = `1500 * (1 + level*2) * grade`.
fn awakening_price(level: usize, grade: u8) -> u32 {
    1500u32
        .saturating_mul(1 + (level as u32) * 2)
        .saturating_mul(u32::from(grade.max(1)))
}

/// Crystal `UserItem.DowngradePrice()` = `3000 * (1 + (level+1)*2) * grade`.
fn downgrade_price(level: usize, grade: u8) -> u32 {
    3000u32
        .saturating_mul(1 + ((level as u32) + 1) * 2)
        .saturating_mul(u32::from(grade.max(1)))
}

/// `AwakeningSystem.ini` keeps every material base = 1 and rate = 1, so each of
/// the two materials needs `1 + level` copies (Crystal `HasAwakeningNeedMaterials`).
fn material_count(level: usize) -> u32 {
    1 + level as u32
}

struct AwakeItem {
    index: usize,
    key: String,
    grade: u8,
    current_type: u8,
    level: usize,
}

fn find_awake_item(world: &World, unique_id: u64) -> Option<AwakeItem> {
    let inventory = world.resource::<InventoryResource>();
    inventory
        .inventory_items
        .iter()
        .enumerate()
        .find(|(_, item)| item.unique_id == unique_id)
        .and_then(|(index, item)| {
            let template = crystal_item_template_for_item_key(&item.key)?;
            Some(AwakeItem {
                index,
                key: item.key.clone(),
                grade: template.grade.max(1),
                current_type: item.awake_type,
                level: item.awake_values.len(),
            })
        })
}

/// How many awakening orbs of (`grade`, `shape`) the bag currently holds.
fn count_orbs(world: &World, grade: u8, shape: i16) -> u32 {
    world
        .resource::<InventoryResource>()
        .inventory_items
        .iter()
        .filter_map(|item| {
            let template = crystal_item_template_for_item_key(&item.key)?;
            (template.item_type == CRYSTAL_ITEM_TYPE_AWAKENING
                && template.grade == grade
                && template.shape == shape)
                .then_some(item.quantity)
        })
        .sum()
}

/// Removes `need` orbs of (`grade`, `shape`), emitting `DeleteItem` for each
/// reduced/emptied stack. Caller has already checked `count_orbs >= need`.
fn consume_orbs(world: &mut World, grade: u8, shape: i16, mut need: u32) -> Vec<ServerPacket> {
    let mut packets = Vec::new();
    let mut emptied = Vec::new();
    {
        let mut inventory = world.resource_mut::<InventoryResource>();
        for (idx, item) in inventory.inventory_items.iter_mut().enumerate() {
            if need == 0 {
                break;
            }
            let Some(template) = crystal_item_template_for_item_key(&item.key) else {
                continue;
            };
            if template.item_type != CRYSTAL_ITEM_TYPE_AWAKENING
                || template.grade != grade
                || template.shape != shape
            {
                continue;
            }
            let take = need.min(item.quantity);
            item.quantity -= take;
            need -= take;
            packets.push(ServerPacket::DeleteItem {
                unique_id: item.unique_id,
                count: u16::try_from(take).unwrap_or(u16::MAX),
            });
            if item.quantity == 0 {
                emptied.push(idx);
            }
        }
        for idx in emptied.into_iter().rev() {
            inventory.inventory_items.remove(idx);
        }
    }
    packets
}

/// Crystal `PlayerObject.Awakening` — upgrade an inventory item's awakening.
pub(super) fn handle_awakening(
    world: &mut World,
    unique_id: u64,
    awake_type: u8,
) -> Vec<ServerPacket> {
    if awake_type == 0 {
        return Vec::new();
    }
    let Some(item) = find_awake_item(world, unique_id) else {
        return Vec::new();
    };
    let Some(template) = crystal_item_template_for_item_key(&item.key) else {
        return Vec::new();
    };

    // -1: bound against upgrades / not an awakenable item / incompatible type.
    if template.bind & BIND_DONT_UPGRADE != 0 || !template.can_awakening {
        return vec![ServerPacket::Awakening {
            result: -1,
            remove_id: -1,
        }];
    }
    // -2: already at max awakening level.
    if item.level >= AWAKE_MAX_LEVEL {
        return vec![ServerPacket::Awakening {
            result: -2,
            remove_id: -1,
        }];
    }
    if !awake_check(&template, item.current_type, item.level, awake_type) {
        return vec![ServerPacket::Awakening {
            result: -1,
            remove_id: -1,
        }];
    }
    // -3: not enough gold for the awakening fee.
    let price = awakening_price(item.level, item.grade);
    if world.resource::<PlayerRuntimeResource>().gold < price {
        return vec![ServerPacket::Awakening {
            result: -3,
            remove_id: -1,
        }];
    }
    // Insufficient materials -> Crystal silently no-ops (no response packet).
    let need = material_count(item.level);
    let type_shape = i16::from(awake_type) - 1;
    if count_orbs(world, item.grade, type_shape) < need
        || count_orbs(world, item.grade, AWAKE_COMMON_ORB_SHAPE) < need
    {
        return Vec::new();
    }

    // Consume the fee and both materials.
    let mut packets = Vec::new();
    {
        let mut runtime = world.resource_mut::<PlayerRuntimeResource>();
        runtime.gold = runtime.gold.saturating_sub(price);
    }
    packets.push(ServerPacket::LoseGold { gold: price });
    packets.extend(consume_orbs(world, item.grade, type_shape, need));
    packets.extend(consume_orbs(
        world,
        item.grade,
        AWAKE_COMMON_ORB_SHAPE,
        need,
    ));

    // Consuming orbs may have shifted indices; re-find the target item.
    let Some(index) = world
        .resource::<InventoryResource>()
        .inventory_items
        .iter()
        .position(|item| item.unique_id == unique_id)
    else {
        return packets;
    };

    let tick = runtime_tick(world);
    let seed = awake_key_seed(&item.key);
    let success = deterministic_roll(tick, seed, item.level, 100) < AWAKE_SUCCESS_RATE;
    if success {
        let value = awake_make_value(tick, &template, seed.wrapping_add(item.level));
        let refreshed = {
            let mut inventory = world.resource_mut::<InventoryResource>();
            let state = &mut inventory.inventory_items[index];
            state.awake_type = awake_type;
            state.awake_values.push(value);
            user_item_from_item_state(state)
        };
        packets.push(ServerPacket::RefreshItem { item: refreshed });
        packets.push(ServerPacket::Awakening {
            result: 1,
            remove_id: -1,
        });
    } else {
        // Failure destroys the item (Crystal `Info.Inventory[i] = null`).
        let count = world
            .resource_mut::<InventoryResource>()
            .inventory_items
            .remove(index)
            .quantity;
        packets.push(ServerPacket::DeleteItem {
            unique_id,
            count: u16::try_from(count).unwrap_or(1),
        });
        packets.push(ServerPacket::Awakening {
            result: 0,
            remove_id: unique_id as i64,
        });
    }
    packets
}

/// Crystal `PlayerObject.AwakeningNeedMaterials` — report the two orb stacks
/// (and counts) required to awaken the item with `awake_type`.
pub(super) fn handle_awakening_need_materials(
    world: &World,
    unique_id: u64,
    awake_type: u8,
) -> Vec<ServerPacket> {
    let materials = (awake_type != 0)
        .then(|| find_awake_item(world, unique_id))
        .flatten()
        .map(|item| {
            let need = material_count(item.level);
            let type_shape = i16::from(awake_type) - 1;
            vec![
                orb_material(item.grade, type_shape, need),
                orb_material(item.grade, AWAKE_COMMON_ORB_SHAPE, need),
            ]
        });
    vec![ServerPacket::AwakeningNeedMaterials { materials }]
}

fn orb_material(grade: u8, shape: i16, count: u32) -> Option<AwakeningMaterial> {
    let template = crystal_item_manifest().items.into_iter().find(|template| {
        template.item_type == CRYSTAL_ITEM_TYPE_AWAKENING
            && template.grade == grade
            && template.shape == shape
    })?;
    Some(AwakeningMaterial {
        item: item_info_from_crystal_template(template),
        count: u8::try_from(count).unwrap_or(u8::MAX),
    })
}

/// Crystal `AwakeningLockedItem` — toggle the lock that keeps a chosen awakening
/// stat from re-rolling. The web tracks the flag client-side; echo it back.
pub(super) fn handle_awakening_locked_item(unique_id: u64, locked: bool) -> Vec<ServerPacket> {
    vec![ServerPacket::AwakeningLockedItem { unique_id, locked }]
}

/// Crystal `PlayerObject.DowngradeAwakening` — drop one awakening level for the
/// downgrade fee.
pub(super) fn handle_downgrade_awakening(world: &mut World, unique_id: u64) -> Vec<ServerPacket> {
    let Some(item) = find_awake_item(world, unique_id) else {
        return Vec::new();
    };
    if item.level == 0 {
        return Vec::new();
    }
    let price = downgrade_price(item.level, item.grade);
    if world.resource::<PlayerRuntimeResource>().gold < price {
        return Vec::new();
    }
    let mut packets = vec![ServerPacket::LoseGold { gold: price }];
    {
        let mut runtime = world.resource_mut::<PlayerRuntimeResource>();
        runtime.gold = runtime.gold.saturating_sub(price);
    }
    let refreshed = {
        let mut inventory = world.resource_mut::<InventoryResource>();
        let state = &mut inventory.inventory_items[item.index];
        state.awake_values.pop();
        if state.awake_values.is_empty() {
            state.awake_type = 0;
        }
        user_item_from_item_state(state)
    };
    packets.push(ServerPacket::RefreshItem { item: refreshed });
    packets
}
