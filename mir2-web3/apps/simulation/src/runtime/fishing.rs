use std::hash::BuildHasher;
use std::time::{Duration, Instant};

use bevy_ecs::prelude::World;
use mir2_game_data::{crystal_drop_table_by_key, localized_text_or_fallback, CrystalItemTemplate};
use mir2_protocol::{ChatType, Point, ServerPacket, UserItem};

use crate::config::{EquipmentSlot, ItemContainer, WorldEntityDisposition};
use crate::CRYSTAL_FISHING_STEP;

use super::components::{
    current_player_is_dead, current_player_object_id, entity_position, player_entity,
};
use super::crystal_compat::{
    CRYSTAL_FISHING_ROD_SHAPES, CRYSTAL_ITEM_TYPE_BAIT, CRYSTAL_ITEM_TYPE_WEAPON,
    CRYSTAL_STAT_CRITICAL_RATE, CRYSTAL_STAT_MAX_AC, CRYSTAL_STAT_MAX_MAC, CRYSTAL_STAT_MIN_AC,
};
use super::drops::{can_gain_gold, crystal_attempt_drop_entry, ResolvedDropTemplate};
use super::equipment::{
    equipment_slot_unique_id, equipment_state_from_item_state, item_state_from_equipment_state,
    user_item_from_equipment_state, validated_item_state_user_item, EquipmentState,
};
use super::inventory::can_gain_item_quantity;
use super::item_grants::{FreshItemGrant, publish_world_grants};
use super::items::{
    crystal_item_template_for_item_key, item_unique_id, try_item_state_from_user_item,
    user_item_from_item_state, user_item_stat_total, ItemState,
};
use super::monsters::{
    crystal_dynamic_monster_template, crystal_spawn_candidates_on_map, deterministic_roll,
    spawn_runtime_monster,
};
use super::movement::{current_location, offset_point};
use super::resources::{
    current_language, is_in_world, runtime_tick, FishingResource, InventoryResource,
    MapRuntimeResource, PlayerRuntimeResource, RuntimeConfigResource, SessionResource,
};
use super::stats::player_stats;

const CRYSTAL_FISHING_SUCCESS_START_PERCENT: i32 = 10;
const CRYSTAL_FISHING_BITE_WINDOW: Duration = Duration::from_secs(3);
const CRYSTAL_FISHING_MONSTER_EVENT_SALT: usize = 910_023;
const CRYSTAL_FISHING_MONSTER_EVENT_DENOMINATOR: u64 = 95;
const CRYSTAL_FISHING_MONSTER: &str = "GiantKeratoid";
const CRYSTAL_STAT_FISH_RATE_PERCENT: u8 = 105;
const CRYSTAL_FISHING_SUCCESS_COUNTER_MULTIPLIER: i32 = 10;
const CRYSTAL_ITEM_TYPE_HOOK: u8 = 28;
const CRYSTAL_ITEM_TYPE_FLOAT: u8 = 29;
const CRYSTAL_ITEM_TYPE_FINDER: u8 = 31;
const CRYSTAL_ITEM_TYPE_REEL: u8 = 32;
const FISHING_SLOT_HOOK: usize = 0;
const FISHING_SLOT_FLOAT: usize = 1;
const FISHING_SLOT_BAIT: usize = 2;
const FISHING_SLOT_FINDER: usize = 3;
const FISHING_SLOT_REEL: usize = 4;

#[derive(Clone, Copy, Hash)]
enum FishingRoll {
    FloatBonus,
    FinderBonus,
    Nibble(u32),
    AutoReel(u32),
    ReelBonus,
    ReelSuccess,
}

#[derive(Debug, Clone, Copy, Default)]
struct FishingBaitStats {
    success_bonus: i32,
}

#[derive(Debug, Clone, Default)]
struct FishingBaitUse {
    stats: FishingBaitStats,
    packet: Option<ServerPacket>,
}

#[derive(Debug, Default)]
struct FishingReelOutcome {
    packets: Vec<ServerPacket>,
    cancel_autocast: bool,
}

pub(super) fn fishing_cast_impl(world: &mut World, cast_out: bool) -> Vec<ServerPacket> {
    fishing_cast_at(world, cast_out, Instant::now())
}

fn fishing_cast_at(world: &mut World, cast_out: bool, now: Instant) -> Vec<ServerPacket> {
    if !is_in_world(world) || current_player_is_dead(world) {
        return Vec::new();
    }
    sync_fishing_slots_from_equipment(world);

    if cast_out {
        let mut packets = Vec::new();
        if !world.resource::<FishingResource>().fishing
            && begin_fishing_cast(world, now, &mut packets)
        {
            packets.push(fishing_update_packet(world));
        }
        return packets;
    }

    let mut packets = Vec::new();
    if !fishing_rod_and_point_are_valid(world) {
        interrupt_fishing(world, &mut packets);
        return packets;
    }

    if !world.resource::<FishingResource>().fishing {
        return vec![fishing_update_packet(world)];
    }

    let progress = world.resource::<FishingResource>().progress_percent;
    finish_fishing_cast(world, now, progress, false, &mut packets);
    packets
}

pub(super) fn fishing_change_autocast_impl(
    world: &mut World,
    auto_cast: bool,
) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }
    sync_fishing_slots_from_equipment(world);

    if !has_equipped_fishing_rod(world) {
        // The clients retire the preference after an authoritative weapon change.
        if !auto_cast {
            world.resource_mut::<FishingResource>().auto_cast = false;
        }
        return Vec::new();
    }
    if !fishing_has_reel(world) {
        world.resource_mut::<FishingResource>().auto_cast = false;
        return Vec::new();
    }

    world.resource_mut::<FishingResource>().auto_cast = auto_cast;
    vec![fishing_update_packet(world)]
}

pub(super) fn tick_fishing(world: &mut World, packets: &mut Vec<ServerPacket>) {
    tick_fishing_at(world, packets, Instant::now());
}

impl crate::SimulationSession {
    pub(crate) fn tick_fishing(&mut self) -> Vec<ServerPacket> {
        let mut packets = Vec::new();
        tick_fishing(self.app.world_mut(), &mut packets);
        packets
    }
}

pub(super) fn tick_fishing_at(world: &mut World, packets: &mut Vec<ServerPacket>, now: Instant) {
    if !is_in_world(world)
        || world
            .resource::<FishingResource>()
            .next_step_at
            .is_none_or(|deadline| now < deadline)
    {
        return;
    }
    sync_fishing_slots_from_equipment(world);
    if current_player_is_dead(world) || !fishing_rod_and_point_are_valid(world) {
        interrupt_fishing(world, packets);
        return;
    }
    let (attempt, had_bite, nibble_chance, auto_reel_chance) = {
        let mut fishing = world.resource_mut::<FishingResource>();
        expire_fishing_bite(&mut fishing, now);
        fishing.next_step_at = Some(now + CRYSTAL_FISHING_STEP);
        fishing.attempts += 1;
        (
            fishing.attempts,
            fishing.bite_expires_at.is_some(),
            fishing.nibble_chance,
            fishing.auto_reel_chance,
        )
    };
    let attempt_limit = world
        .resource::<RuntimeConfigResource>()
        .config
        .crystal_fishing_attempts
        .get();
    let progress = (attempt * 100 / u32::from(attempt_limit)) as i32;
    if had_bite
        && auto_reel_chance > 0
        && fishing_draw(world, FishingRoll::AutoReel(attempt), 0, 100) <= auto_reel_chance
    {
        // UpdateFish reels before publishing this attempt. If that starts the
        // next cast, its final 200 ms assignment supersedes FishingCast's delay.
        if finish_fishing_cast(world, now, progress, true, packets) {
            world.resource_mut::<FishingResource>().next_step_at = Some(now + CRYSTAL_FISHING_STEP);
            packets.push(fishing_update_packet(world));
        }
        return;
    }
    if !had_bite && fishing_draw(world, FishingRoll::Nibble(attempt), 0, 100) <= nibble_chance {
        {
            let mut fishing = world.resource_mut::<FishingResource>();
            fishing.found_fish = true;
            fishing.bite_expires_at = Some(now + CRYSTAL_FISHING_BITE_WINDOW);
        }
        if fishing_slot_model_available(world) {
            damage_fishing_slot_item(world, FISHING_SLOT_FLOAT, packets);
        }
    }
    world.resource_mut::<FishingResource>().progress_percent = progress;
    packets.push(fishing_update_packet(world));
    if progress > 100 {
        finish_fishing_cast(world, now, progress, true, packets);
    }
}

fn expire_fishing_bite(fishing: &mut FishingResource, now: Instant) {
    if fishing.found_fish && fishing.bite_expires_at.is_some_and(|deadline| now > deadline) {
        fishing.found_fish = false;
    }
}

fn finish_fishing_cast(
    world: &mut World,
    now: Instant,
    published_progress: i32,
    allow_recast: bool,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    let outcome = if fishing_has_hook(world) {
        damage_fishing_slot_item(world, FISHING_SLOT_HOOK, packets);
        wear_retry_finder(world, packets);
        let rate = player_stats(world).get(CRYSTAL_STAT_FISH_RATE_PERCENT);
        let found_fish = {
            let mut fishing = world.resource_mut::<FishingResource>();
            expire_fishing_bite(&mut fishing, now);
            fishing.chance_percent = clamp_percent(fishing.chance_percent.saturating_add(rate));
            fishing.found_fish
        };
        if found_fish {
            complete_fishing_reel(world)
        } else {
            count_completed_fishing_attempt(world);
            FishingReelOutcome::default()
        }
    } else {
        FishingReelOutcome {
            packets: vec![fishing_chat(world, "server.NeedHook", "NeedHook")],
            cancel_autocast: true,
        }
    };
    packets.extend(outcome.packets);
    clear_active_fishing(world);
    world.resource_mut::<FishingResource>().progress_percent = published_progress;
    packets.push(fishing_update_packet(world));

    if allow_recast
        && !outcome.cancel_autocast
        && fishing_autocast_enabled(world)
        && fishing_has_hook(world)
        && fishing_bait_available(world)
        && begin_fishing_cast(world, now, packets)
    {
        packets.push(fishing_update_packet(world));
        return true;
    }
    false
}

/// Derive the fishing attribute from the cell three tiles ahead of the player
/// (Crystal `FishingCast` reads `Cells[PointMove(loc, dir, 3)].FishingAttribute`).
///
/// Only applied when the current map actually declares fishing cells; maps
/// without parsed fishing data (e.g. the synthetic starter field) keep the
/// permissive default attribute so they remain fishable.
fn sync_fishing_attribute_from_map(world: &mut World) {
    if world
        .resource::<MapRuntimeResource>()
        .fishing_cells
        .is_empty()
    {
        return;
    }
    let location = current_location(world);
    let point = offset_point(&location.position, location.direction, 3);
    let attribute = world
        .resource::<MapRuntimeResource>()
        .fishing_cells
        .get(&(point.x, point.y))
        .copied()
        .unwrap_or(-1);
    world.resource_mut::<FishingResource>().fishing_attribute = attribute;
}

fn begin_fishing_cast(world: &mut World, now: Instant, packets: &mut Vec<ServerPacket>) -> bool {
    sync_fishing_attribute_from_map(world);
    if !fishing_rod_and_point_are_valid(world) {
        clear_active_fishing(world);
        return false;
    }
    let rod_success_bonus = equipped_fishing_rod_stat(world, CRYSTAL_STAT_MAX_AC);
    if let Some(packet) = damage_equipped_fishing_rod(world) {
        packets.push(packet);
    }
    if !fishing_has_hook(world) {
        clear_active_fishing(world);
        packets.push(fishing_chat(world, "server.NeedHook", "NeedHook"));
        return false;
    }
    damage_fishing_slot_item(world, FISHING_SLOT_HOOK, packets);
    world.resource_mut::<FishingResource>().rolls = Default::default();
    let retry_bonus = fishing_retry_success_bonus(world);
    wear_retry_finder(world, packets);
    let Some(bait_stats) = consume_one_fishing_bait(world) else {
        clear_active_fishing(world);
        packets.push(fishing_chat(world, "server.YouNeedBait", "YouNeedBait"));
        return false;
    };
    let chance_percent = fishing_cast_success_percent(
        world,
        rod_success_bonus,
        bait_stats.stats.success_bonus,
        retry_bonus,
    );
    let nibble_min = fishing_slot_template_stat(world, FISHING_SLOT_FLOAT, CRYSTAL_STAT_MIN_AC);
    let nibble_max = fishing_slot_template_stat(world, FISHING_SLOT_FLOAT, CRYSTAL_STAT_MAX_AC);
    let nibble_chance = clamp_percent(
        5 + fishing_byte_range(world, FishingRoll::FloatBonus, nibble_min, nibble_max),
    );
    let auto_reel_chance = clamp_percent(fishing_slot_template_stat(
        world,
        FISHING_SLOT_REEL,
        CRYSTAL_STAT_MAX_MAC,
    ));
    let cast_delay = world
        .resource::<RuntimeConfigResource>()
        .config
        .crystal_fishing_cast_delay();
    if let Some(packet) = bait_stats.packet {
        packets.push(packet);
    }
    {
        let mut fishing = world.resource_mut::<FishingResource>();
        fishing.fishing = true;
        fishing.progress_percent = 0;
        fishing.chance_percent = chance_percent;
        fishing.found_fish = false;
        fishing.bite_expires_at = None;
        fishing.next_step_at = Some(now + cast_delay);
        fishing.attempts = 0;
        fishing.nibble_chance = nibble_chance;
        fishing.auto_reel_chance = auto_reel_chance;
    }
    true
}

fn wear_retry_finder(world: &mut World, packets: &mut Vec<ServerPacket>) {
    if world.resource::<FishingResource>().chance_counter > 0 {
        damage_fishing_slot_item(world, FISHING_SLOT_FINDER, packets);
    }
}

fn fishing_rod_and_point_are_valid(world: &World) -> bool {
    has_equipped_fishing_rod(world) && world.resource::<FishingResource>().fishing_attribute >= 0
}

fn clear_active_fishing(world: &mut World) {
    let mut fishing = world.resource_mut::<FishingResource>();
    fishing.fishing = false;
    fishing.found_fish = false;
    fishing.bite_expires_at = None;
    fishing.next_step_at = None;
}

pub(super) fn interrupt_fishing(world: &mut World, packets: &mut Vec<ServerPacket>) {
    if world.resource::<FishingResource>().fishing {
        clear_active_fishing(world);
        packets.push(fishing_update_packet(world));
    }
}

pub(super) fn reset_fishing_owner(world: &mut World) {
    world.insert_resource(FishingResource::new());
}

fn fishing_chat(world: &World, key: &str, fallback: &str) -> ServerPacket {
    ServerPacket::Chat {
        message: localized_text_or_fallback(current_language(world), key, fallback),
        chat_type: ChatType::System,
    }
}

fn has_equipped_fishing_rod(world: &World) -> bool {
    world
        .resource::<InventoryResource>()
        .equipment_items
        .iter()
        .any(equipment_is_fishing_rod)
}

fn equipment_is_fishing_rod(equipment: &EquipmentState) -> bool {
    if equipment.slot != EquipmentSlot::Weapon || equipment.durability_current == 0 {
        return false;
    }
    let template_match = crystal_item_template_for_item_key(&equipment.key)
        .map(|template| {
            template.item_type == CRYSTAL_ITEM_TYPE_WEAPON
                && CRYSTAL_FISHING_ROD_SHAPES.contains(&template.shape)
        })
        .unwrap_or(false);
    let shape_match = equipment
        .shape
        .and_then(|shape| i16::try_from(shape).ok())
        .is_some_and(|shape| CRYSTAL_FISHING_ROD_SHAPES.contains(&shape));
    let name_match = equipment.name.contains("FishingRod") || equipment.name.contains("SkyRod");
    template_match || shape_match || name_match
}

fn fishing_slot_model_available(world: &World) -> bool {
    if has_crystal_fishing_slot_carrier(world) {
        return true;
    }
    world
        .resource::<FishingResource>()
        .slot_items
        .iter()
        .any(Option::is_some)
}

fn has_crystal_fishing_slot_carrier(world: &World) -> bool {
    world
        .resource::<InventoryResource>()
        .equipment_items
        .iter()
        .any(|item| {
            item.slot == EquipmentSlot::Weapon
                && crystal_item_template_for_item_key(&item.key).is_some_and(|template| {
                    template.item_type == CRYSTAL_ITEM_TYPE_WEAPON
                        && CRYSTAL_FISHING_ROD_SHAPES.contains(&template.shape)
                })
        })
}

fn equipped_fishing_slot_carrier(world: &World) -> Option<(usize, ItemState, UserItem)> {
    let resources = world.resource::<InventoryResource>();
    let (index, rod) = resources
        .equipment_items
        .iter()
        .enumerate()
        .find(|(_, item)| item.slot == EquipmentSlot::Weapon)?;
    let template = crystal_item_template_for_item_key(&rod.key)?;
    if template.item_type != CRYSTAL_ITEM_TYPE_WEAPON
        || !CRYSTAL_FISHING_ROD_SHAPES.contains(&template.shape)
    {
        return None;
    }
    let user_item = user_item_from_equipment_state(rod)?;
    if user_item.slots.len() != 5 {
        return None;
    }
    let carrier = item_state_from_equipment_state(rod.clone(), ItemContainer::Bag1, 0);
    Some((index, carrier, user_item))
}

fn sync_fishing_slots_from_equipment(world: &mut World) {
    let Some((_, carrier, user_item)) = equipped_fishing_slot_carrier(world) else {
        if has_crystal_fishing_slot_carrier(world) {
            world.resource_mut::<FishingResource>().slot_items = vec![None; 5];
        }
        return;
    };
    let Some(hydrated) = try_item_state_from_user_item(carrier, &user_item).ok() else {
        world.resource_mut::<FishingResource>().slot_items = vec![None; 5];
        return;
    };
    let slots = user_item
        .slots
        .iter()
        .enumerate()
        .map(|(slot, item)| {
            let item = item.as_ref()?;
            let mut child = hydrated
                .socketed
                .iter()
                .find(|child| child.unique_id == item.unique_id)?
                .clone();
            child.slot = slot as u8;
            Some(child)
        })
        .collect();
    world.resource_mut::<FishingResource>().slot_items = slots;
}

/// FishingResource is a transient view; the equipped rod owns every child.
/// Also used by fixtures migrating old sidecar-only fishing state.
pub(super) fn persist_fishing_slots_to_equipment(world: &mut World) -> bool {
    let Some((index, carrier, mut user_item)) = equipped_fishing_slot_carrier(world) else {
        return !has_crystal_fishing_slot_carrier(world);
    };
    let fishing = world.resource::<FishingResource>();
    if fishing.slot_items.len() != 5 {
        return false;
    }
    let Some(slots) = fishing
        .slot_items
        .iter()
        .map(|item| match item {
            Some(item) => validated_item_state_user_item(item).map(Some),
            None => Some(None),
        })
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    user_item.slots = slots;
    let Ok(next) = try_item_state_from_user_item(carrier, &user_item) else {
        return false;
    };
    let next = equipment_state_from_item_state(&next, EquipmentSlot::Weapon);
    if user_item_from_equipment_state(&next).is_none() {
        return false;
    }
    world.resource_mut::<InventoryResource>().equipment_items[index] = next;
    true
}

fn fishing_has_hook(world: &World) -> bool {
    if fishing_slot_model_available(world) {
        fishing_slot_item_of_type(world, FISHING_SLOT_HOOK, CRYSTAL_ITEM_TYPE_HOOK).is_some()
    } else {
        world.resource::<FishingResource>().rod_has_hook
    }
}

fn fishing_has_reel(world: &World) -> bool {
    if fishing_slot_model_available(world) {
        fishing_slot_item_of_type(world, FISHING_SLOT_REEL, CRYSTAL_ITEM_TYPE_REEL).is_some()
    } else {
        world.resource::<FishingResource>().rod_has_reel
    }
}

fn fishing_autocast_enabled(world: &World) -> bool {
    // Successful wear controls that cast's cancellation. An already-zero reel
    // is still mounted and can retry a later miss until a successful wear removes it.
    world.resource::<FishingResource>().auto_cast && fishing_has_reel(world)
}

fn fishing_slot_item_of_type(world: &World, slot: usize, item_type: u8) -> Option<&ItemState> {
    world
        .resource::<FishingResource>()
        .slot_items
        .get(slot)
        .and_then(Option::as_ref)
        .filter(|item| item.quantity > 0 && item_is_fishing_item_type(item, item_type))
}

fn item_is_fishing_item_type(item: &ItemState, item_type: u8) -> bool {
    crystal_item_template_for_item_key(&item.key)
        .map(|template| template.item_type == item_type)
        .unwrap_or_else(|| match item_type {
            CRYSTAL_ITEM_TYPE_HOOK => item.name.contains("Hook"),
            CRYSTAL_ITEM_TYPE_FLOAT => item.name.contains("Float"),
            CRYSTAL_ITEM_TYPE_BAIT => item.name.contains("Bait"),
            CRYSTAL_ITEM_TYPE_FINDER => {
                item.name.contains("Finder") || item.name.contains("Detector")
            }
            CRYSTAL_ITEM_TYPE_REEL => item.name.contains("Reel"),
            _ => false,
        })
}

fn consume_one_fishing_bait(world: &mut World) -> Option<FishingBaitUse> {
    if fishing_slot_model_available(world) {
        return consume_one_fishing_slot_bait(world);
    }

    let mut inventory = world.resource_mut::<InventoryResource>();
    consume_one_fishing_bait_from_items(&mut inventory.inventory_items)
        .or_else(|| consume_one_fishing_bait_from_items(&mut inventory.belt_items))
}

fn fishing_bait_available(world: &World) -> bool {
    if fishing_slot_model_available(world) {
        return fishing_slot_item_of_type(world, FISHING_SLOT_BAIT, CRYSTAL_ITEM_TYPE_BAIT).is_some();
    }
    let inventory = world.resource::<InventoryResource>();
    inventory
        .inventory_items
        .iter()
        .chain(&inventory.belt_items)
        .any(|item| item.quantity > 0 && item_is_fishing_bait(item))
}

fn consume_one_fishing_slot_bait(world: &mut World) -> Option<FishingBaitUse> {
    let previous = world.resource::<FishingResource>().slot_items.clone();
    let mut fishing = world.resource_mut::<FishingResource>();
    let slot = fishing.slot_items.get_mut(FISHING_SLOT_BAIT)?;
    let item = slot.as_mut()?;
    if item.quantity == 0 || !item_is_fishing_bait(item) {
        return None;
    }
    let stats = fishing_bait_stats(item);
    let unique_id = item_unique_id(item);
    if item.quantity > 1 {
        item.quantity -= 1;
    } else {
        *slot = None;
    }
    drop(fishing);
    if !persist_fishing_slots_to_equipment(world) {
        world.resource_mut::<FishingResource>().slot_items = previous;
        return None;
    }
    Some(FishingBaitUse {
        stats,
        packet: Some(ServerPacket::DeleteItem {
            unique_id,
            count: 1,
        }),
    })
}

fn consume_one_fishing_bait_from_items(items: &mut Vec<ItemState>) -> Option<FishingBaitUse> {
    let Some(index) = items.iter().position(item_is_fishing_bait) else {
        return None;
    };
    let stats = fishing_bait_stats(&items[index]);
    if items[index].quantity > 1 {
        items[index].quantity -= 1;
    } else {
        items.remove(index);
    }
    Some(FishingBaitUse {
        stats,
        packet: None,
    })
}

fn item_is_fishing_bait(item: &ItemState) -> bool {
    crystal_item_template_for_item_key(&item.key)
        .map(|template| template.item_type == CRYSTAL_ITEM_TYPE_BAIT)
        .unwrap_or_else(|| item.name.contains("Bait"))
}

fn fishing_bait_stats(item: &ItemState) -> FishingBaitStats {
    crystal_item_template_for_item_key(&item.key)
        .map(|template| FishingBaitStats {
            success_bonus: crystal_template_stat(&template, CRYSTAL_STAT_MAX_AC),
        })
        .unwrap_or_default()
}

fn damage_fishing_slot_item(
    world: &mut World,
    slot_index: usize,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    let Some(expected_type) = fishing_slot_expected_item_type(slot_index) else {
        return false;
    };
    let Some(item) = fishing_slot_item_of_type(world, slot_index, expected_type) else {
        return false;
    };
    let unique_id = item_unique_id(item);
    let Some(current_dura) = item.durability_current else {
        return true;
    };
    // Crystal removes an already broken attachment on its next wear event.
    // The transition from one to zero is an ordinary durability receipt.
    let (replacement, receipt, usable) = if current_dura == 0 {
        (
            None,
            ServerPacket::DeleteItem { unique_id, count: 1 },
            false,
        )
    } else {
        let next_dura = current_dura - 1;
        let mut item = item.clone();
        item.durability_current = Some(next_dura);
        (
            Some(item),
            ServerPacket::DuraChanged {
                unique_id,
                current_dura: next_dura,
            },
            next_dura > 0,
        )
    };
    let previous = std::mem::replace(
        &mut world.resource_mut::<FishingResource>().slot_items[slot_index],
        replacement,
    );
    if !persist_fishing_slots_to_equipment(world) {
        world.resource_mut::<FishingResource>().slot_items[slot_index] = previous;
        return false;
    }
    packets.push(receipt);
    usable
}

fn fishing_slot_expected_item_type(slot_index: usize) -> Option<u8> {
    match slot_index {
        FISHING_SLOT_HOOK => Some(CRYSTAL_ITEM_TYPE_HOOK),
        FISHING_SLOT_FLOAT => Some(CRYSTAL_ITEM_TYPE_FLOAT),
        FISHING_SLOT_BAIT => Some(CRYSTAL_ITEM_TYPE_BAIT),
        FISHING_SLOT_FINDER => Some(CRYSTAL_ITEM_TYPE_FINDER),
        FISHING_SLOT_REEL => Some(CRYSTAL_ITEM_TYPE_REEL),
        _ => None,
    }
}

fn damage_equipped_fishing_rod(world: &mut World) -> Option<ServerPacket> {
    let mut inventory = world.resource_mut::<InventoryResource>();
    let rod = inventory
        .equipment_items
        .iter_mut()
        .find(|item| equipment_is_fishing_rod(item))?;
    let unique_id = user_item_from_equipment_state(rod)
        .map(|item| item.unique_id)
        .unwrap_or_else(|| equipment_slot_unique_id(EquipmentSlot::Weapon).unwrap_or_default());
    rod.durability_current = rod.durability_current.saturating_sub(1);
    Some(ServerPacket::DuraChanged {
        unique_id,
        current_dura: rod.durability_current,
    })
}

fn fishing_update_packet(world: &World) -> ServerPacket {
    let object_id = current_player_object_id(world).unwrap_or_default();
    let location = current_location(world);
    let fishing = world.resource::<FishingResource>();

    ServerPacket::FishingUpdate {
        object_id,
        fishing: fishing.fishing,
        progress_percent: fishing.progress_percent,
        chance_percent: fishing.chance_percent,
        fishing_point: offset_point(&location.position, location.direction, 3),
        found_fish: fishing.found_fish,
    }
}

fn count_completed_fishing_attempt(world: &mut World) {
    let mut fishing = world.resource_mut::<FishingResource>();
    if fishing.progress_percent > 99 {
        fishing.chance_counter = fishing.chance_counter.saturating_add(1);
    }
}

fn complete_fishing_reel(world: &mut World) -> FishingReelOutcome {
    count_completed_fishing_attempt(world);
    if !fishing_reel_succeeds(world) {
        return FishingReelOutcome {
            packets: vec![fishing_chat(world, "server.FishGotAway", "FishGotAway")],
            cancel_autocast: false,
        };
    }

    let mut outcome = match award_fishing_loot(world) {
        Ok(outcome) => outcome,
        Err(_) => return FishingReelOutcome {
            packets: vec![fishing_chat(world, "server.ItemDeliveryUnavailable", "Item delivery is temporarily unavailable. Please try again.")],
            cancel_autocast: true,
        },
    };
    world.resource_mut::<FishingResource>().chance_counter = 0;
    maybe_spawn_fishing_monster_event(world);
    if fishing_slot_model_available(world)
        && !damage_fishing_slot_item(world, FISHING_SLOT_REEL, &mut outcome.packets)
    {
        outcome.cancel_autocast = true;
    }
    outcome
}

pub(super) fn fishing_reel_succeeds(world: &World) -> bool {
    let fishing = world.resource::<FishingResource>();
    let random_bonus = fishing_draw(world, FishingRoll::ReelBonus, 10, 24);
    let flexibility_bonus = if fishing.progress_percent > 50 {
        let rod = equipped_fishing_rod_stat(world, CRYSTAL_STAT_CRITICAL_RATE).clamp(0, 255);
        let hook = fishing_slot_template_stat(world, FISHING_SLOT_HOOK, CRYSTAL_STAT_CRITICAL_RATE);
        let added_hook = fishing_slot_item_of_type(world, FISHING_SLOT_HOOK, CRYSTAL_ITEM_TYPE_HOOK)
            .map_or(0, |item| user_item_stat_total(&item.added_stats, CRYSTAL_STAT_CRITICAL_RATE));
        rod.saturating_add(hook).saturating_add(added_hook).clamp(0, 255) / 2
    } else {
        0
    };
    let chance = clamp_percent(fishing.chance_percent + random_bonus + flexibility_bonus);
    fishing_draw(world, FishingRoll::ReelSuccess, 0, 100) <= chance
}

fn award_fishing_loot(world: &mut World) -> Result<FishingReelOutcome, String> {
    let Some(drop) = resolved_fishing_drop(world) else {
        return Ok(FishingReelOutcome {
            packets: vec![fishing_chat(world, "server.FishGotAway", "FishGotAway")],
            cancel_autocast: false,
        });
    };

    match drop {
        ResolvedDropTemplate::Gold { amount, .. } => {
            if !can_gain_gold(world.resource::<PlayerRuntimeResource>(), amount) {
                return Ok(FishingReelOutcome {
                    packets: vec![fishing_chat(world, "server.NoBagSpace", "NoBagSpace")],
                    cancel_autocast: true,
                });
            }
            world.resource_mut::<PlayerRuntimeResource>().gold += amount;
            Ok(FishingReelOutcome {
                packets: vec![ServerPacket::GainedGold { gold: amount }],
                cancel_autocast: false,
            })
        }
        ResolvedDropTemplate::Item {
            key,
            name,
            description,
            weight,
            quantity,
            durability_current,
            durability_max,
            added_attack,
            added_defence,
            added_stats,
            cursed,
            socket_slots,
            ..
        } => {
            {
                let inventory = world.resource::<InventoryResource>();
                if !can_gain_item_quantity(&inventory, ItemContainer::Bag1, &key, quantity) {
                    return Ok(FishingReelOutcome {
                        packets: vec![fishing_chat(world, "server.NoBagSpace", "NoBagSpace")],
                        cancel_autocast: true,
                    });
                }
            }
            let mut grant = FreshItemGrant::plain(
                ItemContainer::Bag1,
                &key,
                &name,
                &description,
                30,
                quantity,
                weight,
                crate::UserItemUidReason::Fishing,
            )?;
            grant.prototype.durability_current = durability_current.or(grant.prototype.durability_current);
            grant.prototype.durability_max = durability_max.or(grant.prototype.durability_max);
            grant.prototype.added_attack = added_attack;
            grant.prototype.added_defence = added_defence;
            grant.prototype.added_stats = added_stats;
            grant.prototype.cursed = cursed;
            grant.prototype.socket_slots = grant.prototype.socket_slots.max(socket_slots);
            let changed = publish_world_grants(world, &[grant])?;
            Ok(FishingReelOutcome {
                packets: changed.iter().map(|item| ServerPacket::GainedItem {
                    item: user_item_from_item_state(item),
                }).collect(),
                cancel_autocast: false,
            })
        }
    }
}

#[cfg(test)]
#[path = "fishing_uid_tests.rs"]
mod uid_tests;

#[cfg(test)]
#[path = "fishing_lifecycle_tests.rs"]
mod lifecycle_tests;

pub(super) fn resolved_fishing_drop(world: &World) -> Option<ResolvedDropTemplate> {
    let fishing_attribute = world.resource::<FishingResource>().fishing_attribute;
    if fishing_attribute < 0 {
        return None;
    }
    let table_key = format!("{:02}Fishing", fishing_attribute);
    let table = crystal_drop_table_by_key(&table_key)?;
    let tick = runtime_tick(world);
    let object_id = current_player_object_id(world).unwrap_or_default();
    let mut selected = None;
    let mut entry_salt = 0_usize;

    for entry in table
        .sections
        .into_iter()
        .flat_map(|section| section.entries)
    {
        if let Some(resolved) = crystal_attempt_drop_entry(&entry, tick, object_id, entry_salt) {
            for drop in resolved {
                selected = Some(drop);
            }
        }
        entry_salt = entry_salt.saturating_add(1);
    }

    selected
}

fn maybe_spawn_fishing_monster_event(world: &mut World) {
    let tick = runtime_tick(world);
    let object_id = current_player_object_id(world).unwrap_or_default();
    if deterministic_roll(
        tick,
        usize::try_from(object_id).unwrap_or_default(),
        CRYSTAL_FISHING_MONSTER_EVENT_SALT,
        CRYSTAL_FISHING_MONSTER_EVENT_DENOMINATOR,
    ) != 0
    {
        return;
    }
    let Some(template) = crystal_dynamic_monster_template(CRYSTAL_FISHING_MONSTER) else {
        return;
    };
    let Some(player) = player_entity(world) else {
        return;
    };
    let Some(origin) = entity_position(world, player) else {
        return;
    };
    let location = current_location(world);
    let back = offset_point(&location.position, location.direction, -1);
    let (config, map_file_name) = {
        let map = world.resource::<MapRuntimeResource>();
        (
            world.resource::<RuntimeConfigResource>().config.clone(),
            map.current_map.file_name.clone(),
        )
    };
    let mut spawn_points = vec![back.clone()];
    for point in crystal_spawn_candidates_on_map(&config, &map_file_name, &origin, 8) {
        if point != origin && !spawn_points_contains(&spawn_points, &point) {
            spawn_points.push(point);
        }
    }
    spawn_points.sort_by_key(|point| {
        let dx = (point.x - back.x).abs();
        let dy = (point.y - back.y).abs();
        (dx.max(dy), dx + dy, point.y, point.x)
    });

    for position in spawn_points {
        if spawn_runtime_monster(
            world,
            &template,
            position,
            location.direction,
            Some(player),
            None,
            Some(true),
            Some(WorldEntityDisposition::Hostile),
            0,
        )
        .is_some()
        {
            return;
        }
    }
}

fn spawn_points_contains(points: &[Point], point: &Point) -> bool {
    points.iter().any(|candidate| candidate == point)
}

fn fishing_cast_success_percent(
    world: &World,
    rod_success_bonus: i32,
    bait_success_bonus: i32,
    retry_bonus: i32,
) -> i32 {
    let mut success_stat = clamp_signed_byte(rod_success_bonus);
    success_stat = clamp_signed_byte(success_stat.saturating_add(bait_success_bonus));
    let reel_success = fishing_slot_template_stat(world, FISHING_SLOT_REEL, CRYSTAL_STAT_MAX_AC);
    success_stat = clamp_signed_byte(success_stat.saturating_add(reel_success));
    let rate = player_stats(world).get(CRYSTAL_STAT_FISH_RATE_PERCENT);
    clamp_percent(
        (CRYSTAL_FISHING_SUCCESS_START_PERCENT + success_stat)
            .saturating_add(retry_bonus)
            .saturating_add(rate),
    )
}

fn clamp_signed_byte(value: i32) -> i32 {
    value.clamp(i32::from(i8::MIN), i32::from(i8::MAX))
}

fn fishing_byte_range(world: &World, roll: FishingRoll, min: i32, max: i32) -> i32 {
    fishing_draw(world, roll, min.clamp(0, 255), max.clamp(0, 255))
}

fn fishing_draw(world: &World, roll: FishingRoll, min: i32, max: i32) -> i32 {
    if min >= max {
        return min;
    }
    let upper = (max - min) as u64;
    let threshold = upper.wrapping_neg() % upper;
    let rolls = &world.resource::<FishingResource>().rolls;
    let tick = runtime_tick(world);
    for retry in 0_u64.. {
        let draw = rolls.hash_one((roll, tick, retry));
        if draw >= threshold {
            return min + (draw % upper) as i32;
        }
    }
    unreachable!("fishing random range exhausted")
}

fn fishing_retry_success_bonus(world: &World) -> i32 {
    let chance_counter = world.resource::<FishingResource>().chance_counter;
    if chance_counter <= 0 {
        return 0;
    }
    let min = fishing_slot_template_stat(world, FISHING_SLOT_FINDER, CRYSTAL_STAT_MIN_AC);
    let max = fishing_slot_template_stat(world, FISHING_SLOT_FINDER, CRYSTAL_STAT_MAX_AC);
    chance_counter
        .saturating_mul(CRYSTAL_FISHING_SUCCESS_COUNTER_MULTIPLIER)
        .saturating_add(fishing_byte_range(world, FishingRoll::FinderBonus, min, max))
}

fn equipped_fishing_rod_stat(world: &World, stat: u8) -> i32 {
    world
        .resource::<InventoryResource>()
        .equipment_items
        .iter()
        .find(|equipment| equipment_is_fishing_rod(equipment))
        .and_then(|equipment| crystal_item_template_for_item_key(&equipment.key))
        .map_or(0, |template| crystal_template_stat(&template, stat))
}

fn fishing_slot_template_stat(world: &World, slot: usize, stat: u8) -> i32 {
    let Some(expected_type) = fishing_slot_expected_item_type(slot) else {
        return 0;
    };
    fishing_slot_item_of_type(world, slot, expected_type)
        .map_or(0, |item| fishing_item_template_stat(world, item, stat))
}

fn fishing_item_template_stat(world: &World, item: &ItemState, stat: u8) -> i32 {
    let Some(template) = crystal_item_template_for_item_key(&item.key) else {
        return 0;
    };
    let Some(character) = world.resource::<SessionResource>().selected_character.as_ref() else {
        return crystal_template_stat(&template, stat);
    };
    let real =
        mir2_game_data::crystal_real_item_for_player(&template, character.level, character.class);
    crystal_template_stat(&real, stat)
}

fn crystal_template_stat(template: &CrystalItemTemplate, stat: u8) -> i32 {
    template
        .stats
        .iter()
        .filter(|entry| entry.stat == stat)
        .map(|entry| entry.value)
        .sum()
}

fn clamp_percent(value: i32) -> i32 {
    value.clamp(0, 100)
}
