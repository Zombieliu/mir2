use bevy_ecs::prelude::World;
use mir2_game_data::{crystal_drop_table_by_key, localized_text_or_fallback, CrystalItemTemplate};
use mir2_protocol::{ChatType, Point, ServerPacket};

use crate::config::{EquipmentSlot, ItemContainer, WorldEntityDisposition};

use super::components::{current_player_object_id, entity_position, player_entity};
use super::crystal_compat::{
    CRYSTAL_FISHING_ROD_SHAPES, CRYSTAL_ITEM_TYPE_BAIT, CRYSTAL_ITEM_TYPE_WEAPON,
    CRYSTAL_STAT_CRITICAL_RATE, CRYSTAL_STAT_MAX_AC, CRYSTAL_STAT_MAX_DC, CRYSTAL_STAT_MIN_AC,
};
use super::drops::{can_gain_gold, crystal_attempt_drop_entry, ResolvedDropTemplate};
use super::equipment::{equipment_slot_unique_id, EquipmentState};
use super::inventory::{add_or_increment_item_with_random_metadata, can_gain_item_quantity};
use super::items::{
    crystal_item_template_for_item_key, item_unique_id, user_item_from_item_state,
    user_item_stat_total, ItemState,
};
use super::monsters::{
    crystal_dynamic_monster_template, crystal_spawn_candidates_on_map, deterministic_roll,
    spawn_runtime_monster,
};
use super::movement::{current_location, offset_point};
use super::resources::{
    current_language, is_in_world, runtime_tick, FishingResource, InventoryResource,
    MapRuntimeResource, PlayerRuntimeResource, RuntimeConfigResource,
};

const CRYSTAL_FISHING_SUCCESS_START_PERCENT: i32 = 10;
const CRYSTAL_FISHING_PROGRESS_PER_TICK: i32 = 25;
const CRYSTAL_FISHING_REEL_BONUS_MIN: i32 = 10;
const CRYSTAL_FISHING_REEL_BONUS_RANGE: u64 = 14;
const CRYSTAL_FISHING_REEL_BONUS_SALT: usize = 910_000;
const CRYSTAL_FISHING_REEL_SUCCESS_SALT: usize = 910_010;
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
    if !is_in_world(world) {
        return Vec::new();
    }

    if cast_out {
        let mut packets = Vec::new();
        if begin_fishing_cast(world, &mut packets) {
            packets.push(fishing_update_packet(world));
        }
        return packets;
    }

    if !fishing_rod_and_point_are_valid(world) {
        reject_fishing(world);
        return Vec::new();
    }

    if !world.resource::<FishingResource>().fishing {
        return vec![fishing_update_packet(world)];
    }

    let mut packets = Vec::new();
    {
        let mut fishing = world.resource_mut::<FishingResource>();
        fishing.fishing = false;
        fishing.progress_percent = 100;
    }
    if world.resource::<FishingResource>().found_fish {
        packets.extend(complete_fishing_reel(world).packets);
        world.resource_mut::<FishingResource>().found_fish = false;
    }
    packets.push(fishing_update_packet(world));
    packets
}

pub(super) fn fishing_change_autocast_impl(
    world: &mut World,
    auto_cast: bool,
) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }

    if !has_equipped_fishing_rod(world) {
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
    if !is_in_world(world) || !world.resource::<FishingResource>().fishing {
        return;
    }
    let mut found_this_tick = false;
    let auto_cast_ready = {
        let mut fishing = world.resource_mut::<FishingResource>();
        if fishing.found_fish {
            fishing.auto_cast
        } else {
            fishing.progress_percent =
                (fishing.progress_percent + CRYSTAL_FISHING_PROGRESS_PER_TICK).min(100);
            if fishing.progress_percent >= 100 {
                fishing.found_fish = true;
                found_this_tick = true;
            }
            false
        }
    };
    if found_this_tick && fishing_slot_model_available(world) {
        damage_fishing_slot_item(world, FISHING_SLOT_FLOAT, packets);
    }
    if auto_cast_ready {
        let outcome = complete_fishing_reel(world);
        let cancel_autocast = outcome.cancel_autocast;
        packets.extend(outcome.packets);
        {
            let mut fishing = world.resource_mut::<FishingResource>();
            fishing.fishing = false;
            fishing.found_fish = false;
        }
        if !cancel_autocast {
            begin_fishing_cast(world, packets);
        }
    }
    packets.push(fishing_update_packet(world));
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

fn begin_fishing_cast(world: &mut World, packets: &mut Vec<ServerPacket>) -> bool {
    sync_fishing_attribute_from_map(world);
    if !fishing_rod_and_point_are_valid(world) {
        reject_fishing(world);
        return false;
    }
    if !fishing_has_hook(world) {
        reject_fishing(world);
        packets.push(fishing_chat(world, "server.NeedHook", "NeedHook"));
        return false;
    }
    if fishing_slot_model_available(world) {
        damage_fishing_slot_item(world, FISHING_SLOT_HOOK, packets);
    }
    let Some(bait_stats) = consume_one_fishing_bait(world) else {
        reject_fishing(world);
        packets.push(fishing_chat(world, "server.YouNeedBait", "YouNeedBait"));
        return false;
    };
    let chance_percent = fishing_cast_success_percent(world, bait_stats.stats.success_bonus);
    if let Some(packet) = bait_stats.packet {
        packets.push(packet);
    }
    if world.resource::<FishingResource>().chance_counter > 0 && fishing_slot_model_available(world)
    {
        damage_fishing_slot_item(world, FISHING_SLOT_FINDER, packets);
    }
    if let Some(packet) = damage_equipped_fishing_rod(world) {
        packets.push(packet);
    }
    {
        let mut fishing = world.resource_mut::<FishingResource>();
        fishing.fishing = true;
        fishing.progress_percent = 0;
        fishing.chance_percent = chance_percent;
        fishing.found_fish = false;
    }
    true
}

fn fishing_rod_and_point_are_valid(world: &World) -> bool {
    has_equipped_fishing_rod(world) && world.resource::<FishingResource>().fishing_attribute >= 0
}

fn reject_fishing(world: &mut World) {
    let mut fishing = world.resource_mut::<FishingResource>();
    fishing.fishing = false;
    fishing.found_fish = false;
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
    world
        .resource::<FishingResource>()
        .slot_items
        .iter()
        .any(Option::is_some)
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

fn consume_one_fishing_slot_bait(world: &mut World) -> Option<FishingBaitUse> {
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
    let mut fishing = world.resource_mut::<FishingResource>();
    let Some(slot) = fishing.slot_items.get_mut(slot_index) else {
        return false;
    };
    let Some(item) = slot.as_mut() else {
        return false;
    };
    if !item_is_fishing_item_type(item, expected_type) {
        return false;
    }

    let unique_id = item_unique_id(item);
    let Some(current_dura) = item.durability_current else {
        return true;
    };
    if current_dura == 0 {
        *slot = None;
        packets.push(ServerPacket::DeleteItem {
            unique_id,
            count: 1,
        });
        return false;
    }

    let next_dura = current_dura.saturating_sub(1);
    item.durability_current = Some(next_dura);
    packets.push(ServerPacket::DuraChanged {
        unique_id,
        current_dura: next_dura,
    });
    next_dura > 0
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
    let unique_id = equipment_slot_unique_id(EquipmentSlot::Weapon).unwrap_or_default();
    let mut inventory = world.resource_mut::<InventoryResource>();
    let rod = inventory
        .equipment_items
        .iter_mut()
        .find(|item| equipment_is_fishing_rod(item))?;
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

fn complete_fishing_reel(world: &mut World) -> FishingReelOutcome {
    if world.resource::<FishingResource>().progress_percent > 99 {
        let mut fishing = world.resource_mut::<FishingResource>();
        fishing.chance_counter = fishing.chance_counter.saturating_add(1);
    }
    if !fishing_reel_succeeds(world) {
        return FishingReelOutcome {
            packets: vec![fishing_chat(world, "server.FishGotAway", "FishGotAway")],
            cancel_autocast: false,
        };
    }

    world.resource_mut::<FishingResource>().chance_counter = 0;
    let mut outcome = award_fishing_loot(world);
    maybe_spawn_fishing_monster_event(world);
    if fishing_has_reel(world) {
        if fishing_slot_model_available(world) {
            let reel_has_durability =
                damage_fishing_slot_item(world, FISHING_SLOT_REEL, &mut outcome.packets);
            if !reel_has_durability {
                outcome.cancel_autocast = true;
            }
        } else {
            if let Some(packet) = damage_equipped_fishing_rod(world) {
                outcome.packets.push(packet);
            }
            if !has_equipped_fishing_rod(world) {
                outcome.cancel_autocast = true;
            }
        }
        if !fishing_has_reel(world) {
            outcome.cancel_autocast = true;
        }
    }
    outcome
}

fn fishing_reel_succeeds(world: &World) -> bool {
    let tick = runtime_tick(world);
    let object_id = current_player_object_id(world).unwrap_or_default();
    let fishing = world.resource::<FishingResource>();
    let random_bonus = CRYSTAL_FISHING_REEL_BONUS_MIN
        + i32::try_from(deterministic_roll(
            tick,
            usize::try_from(object_id).unwrap_or_default(),
            CRYSTAL_FISHING_REEL_BONUS_SALT,
            CRYSTAL_FISHING_REEL_BONUS_RANGE,
        ))
        .expect("fishing reel bonus should fit i32");
    let flexibility_bonus = if fishing.progress_percent > 50 {
        (equipped_fishing_rod_stat(world, CRYSTAL_STAT_CRITICAL_RATE)
            + fishing_slot_stat(world, FISHING_SLOT_HOOK, CRYSTAL_STAT_CRITICAL_RATE))
            / 2
    } else {
        0
    };
    let chance = clamp_percent(fishing.chance_percent + random_bonus + flexibility_bonus);
    i32::try_from(deterministic_roll(
        tick,
        usize::try_from(object_id).unwrap_or_default(),
        CRYSTAL_FISHING_REEL_SUCCESS_SALT,
        100,
    ))
    .expect("fishing reel roll should fit i32")
        <= chance
}

fn award_fishing_loot(world: &mut World) -> FishingReelOutcome {
    let Some(drop) = resolved_fishing_drop(world) else {
        return FishingReelOutcome {
            packets: vec![fishing_chat(world, "server.FishGotAway", "FishGotAway")],
            cancel_autocast: false,
        };
    };

    match drop {
        ResolvedDropTemplate::Gold { amount, .. } => {
            if !can_gain_gold(world.resource::<PlayerRuntimeResource>(), amount) {
                return FishingReelOutcome {
                    packets: vec![fishing_chat(world, "server.NoBagSpace", "NoBagSpace")],
                    cancel_autocast: true,
                };
            }
            world.resource_mut::<PlayerRuntimeResource>().gold += amount;
            FishingReelOutcome {
                packets: vec![ServerPacket::GainedGold { gold: amount }],
                cancel_autocast: false,
            }
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
                    return FishingReelOutcome {
                        packets: vec![fishing_chat(world, "server.NoBagSpace", "NoBagSpace")],
                        cancel_autocast: true,
                    };
                }
            }
            let item = add_or_increment_item_with_random_metadata(
                world,
                ItemContainer::Bag1,
                &key,
                &name,
                &description,
                30,
                quantity,
                weight,
                durability_current,
                durability_max,
                added_attack,
                added_defence,
                added_stats,
                cursed,
                socket_slots,
            );
            FishingReelOutcome {
                packets: vec![ServerPacket::GainedItem {
                    item: user_item_from_item_state(&item),
                }],
                cancel_autocast: false,
            }
        }
    }
}

fn resolved_fishing_drop(world: &World) -> Option<ResolvedDropTemplate> {
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

fn fishing_cast_success_percent(world: &World, bait_success_bonus: i32) -> i32 {
    clamp_percent(
        CRYSTAL_FISHING_SUCCESS_START_PERCENT
            + equipped_fishing_rod_stat(world, CRYSTAL_STAT_MAX_AC)
            + bait_success_bonus
            + fishing_slot_stat(world, FISHING_SLOT_REEL, CRYSTAL_STAT_MAX_AC)
            + fishing_retry_success_bonus(world)
            + current_fish_rate_percent(world),
    )
}

fn fishing_retry_success_bonus(world: &World) -> i32 {
    let chance_counter = world.resource::<FishingResource>().chance_counter;
    if chance_counter <= 0 {
        return 0;
    }
    let finder_min = fishing_slot_stat(world, FISHING_SLOT_FINDER, CRYSTAL_STAT_MIN_AC);
    let finder_max = fishing_slot_stat(world, FISHING_SLOT_FINDER, CRYSTAL_STAT_MAX_AC);
    chance_counter.saturating_mul(CRYSTAL_FISHING_SUCCESS_COUNTER_MULTIPLIER)
        + finder_min.max(finder_max)
}

fn equipped_fishing_rod_stat(world: &World, stat: u8) -> i32 {
    world
        .resource::<InventoryResource>()
        .equipment_items
        .iter()
        .find(|equipment| equipment_is_fishing_rod(equipment))
        .map(|equipment| equipment_state_stat(equipment, stat))
        .unwrap_or_default()
}

fn current_fish_rate_percent(world: &World) -> i32 {
    world
        .resource::<InventoryResource>()
        .equipment_items
        .iter()
        .map(|equipment| equipment_state_stat(equipment, CRYSTAL_STAT_FISH_RATE_PERCENT))
        .sum()
}

fn fishing_slot_stat(world: &World, slot: usize, stat: u8) -> i32 {
    let Some(expected_type) = fishing_slot_expected_item_type(slot) else {
        return 0;
    };
    fishing_slot_item_of_type(world, slot, expected_type)
        .map(|item| item_state_stat(item, stat))
        .unwrap_or_default()
}

fn equipment_state_stat(equipment: &EquipmentState, stat: u8) -> i32 {
    let template_stats = crystal_item_template_for_item_key(&equipment.key)
        .map(|template| crystal_template_stat(&template, stat))
        .unwrap_or_default();
    let added = user_item_stat_total(&equipment.added_stats, stat);
    let legacy_added = match stat {
        CRYSTAL_STAT_MAX_AC => equipment.added_defence,
        _ => 0,
    };
    template_stats + added + legacy_added
}

fn item_state_stat(item: &ItemState, stat: u8) -> i32 {
    let template_stats = crystal_item_template_for_item_key(&item.key)
        .map(|template| crystal_template_stat(&template, stat))
        .unwrap_or_default();
    let added = user_item_stat_total(&item.added_stats, stat);
    let legacy_added = match stat {
        CRYSTAL_STAT_MAX_AC => item.added_defence,
        CRYSTAL_STAT_MAX_DC => item.added_attack,
        _ => 0,
    };
    template_stats + added + legacy_added
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
