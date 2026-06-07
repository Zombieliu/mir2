use bevy_ecs::{component::Component, entity::Entity, prelude::World, query::With};
use mir2_game_data::{
    crystal_drop_table_for_monster_name, crystal_item_by_name, format_localized_text,
    starter_server_data, CrystalDropEntry, CrystalItemTemplate, CrystalRandomItemStatProfile,
    CrystalRandomStatRoll, DropTemplate, QuestTemplate,
};
use mir2_protocol::{ChatType, MirDirection, ObjectGoldInfo, Point, ServerPacket, UserItemStat};

use crate::config::{GroundDropLootSnapshot, GroundDropSnapshot, ItemContainer, QuestStage};

use super::components::{
    current_player_is_dead, entity_by_object_id, entity_name, entity_object_id, entity_position,
    player_entity, DisplayName, DropExpiry, DropOwnership, GroundDrop, HarvestMonsterState,
    HarvestOwnership, MonsterAgent, MonsterAiState, ObjectId, Position, WorldObject, YimoogiState,
};
use super::crystal_compat::*;
use super::inventory::{
    add_or_increment_item_with_durability, add_or_increment_item_with_random_metadata,
    can_gain_item_quantity, item_matches_inventory_unique_id,
};
use super::items::{
    crystal_item_has_bind_flag, crystal_item_key_for_template, crystal_item_template_for_item_key,
    item_has_crystal_or_rental_bind_flag, localized_drop_name_key, localized_item_name,
    normalize_crystal_item_key, user_item_from_item_state,
};
use super::map::{
    crystal_movement_transfer_records_for_map, current_map_disallows_monster_drop,
    current_map_disallows_throw_item, normalize_map_file_name,
};
use super::monsters::{deterministic_roll, initial_harvest_monster_state};
use super::movement::{has_blocking_entity, is_blocked_tile, offset_point, point_in_bounds};
use super::packets::object_movement;
use super::quests::{
    advance_bespoke_quest_kill, advance_crystal_quest_item_task, advance_crystal_quest_kill,
    crystal_quest_item_task_drop, crystal_quest_update_packet, guide_quest_template,
    quest_template_by_id,
};
use super::resources::{
    GroupResource, InventoryResource, MapRuntimeResource, ObjectIdAllocatorResource,
    PlayerRuntimeResource, QuestResource, RuntimeConfigResource,
};
use super::session::SimulationSession;
use super::session::{
    current_language, is_in_world, localized_monster_name_key, runtime_tick, system_message_key,
    system_message_key_args,
};

#[derive(Component, Clone, Debug, Default)]
pub(super) struct PendingHarvestDrops {
    pub(super) drops: Vec<ResolvedDropTemplate>,
}

pub(super) const CRYSTAL_DROP_OWNER_WINDOW_TICKS: u64 = 60;
pub(super) const CRYSTAL_DROP_EXPIRE_TICKS: u64 = 30 * 60;
pub(super) const CRYSTAL_DROP_RANGE: i32 = 4;
pub(super) const CRYSTAL_PLAYER_DROP_ITEM_RANGE: i32 = 1;
pub(super) const CRYSTAL_PLAYER_DROP_GOLD_RANGE: i32 = 5;
pub(super) const CRYSTAL_DROP_STACK_SIZE: usize = 5;
pub(super) const CRYSTAL_MAX_DROP_GOLD: u32 = 2_000;
pub(super) const CRYSTAL_BAG_WEIGHT_LIMIT: u32 = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HarvestTargetSelection {
    Target(Entity),
    OwnerBlocked,
}

pub(super) fn harvest_target_in_direction(
    world: &World,
    direction: MirDirection,
) -> Option<HarvestTargetSelection> {
    let player = player_entity(world)?;
    let player_object_id = entity_object_id(world, player)?;
    let player_position = entity_position(world, player)?;
    let front = offset_point(&player_position, direction, 1);
    let bounds = world.resource::<MapRuntimeResource>().map_region_bounds;
    let mut owner_blocked = false;

    for distance in 0..=1 {
        for y in (front.y - distance)..=(front.y + distance) {
            for x in harvest_scan_x_values(front.x, front.y, y, distance) {
                let point = Point { x, y };
                if !point_in_bounds(&bounds, &point) || is_blocked_tile(world, &point) {
                    continue;
                }
                for entity in harvestable_carcasses_at(world, &point) {
                    if harvest_ownership_allows(world, entity, player_object_id) {
                        return Some(HarvestTargetSelection::Target(entity));
                    }
                    owner_blocked = true;
                }
            }
        }
    }

    owner_blocked.then_some(HarvestTargetSelection::OwnerBlocked)
}

pub(super) fn harvest_scan_x_values(front_x: i32, front_y: i32, y: i32, distance: i32) -> Vec<i32> {
    if distance == 0 || (y - front_y).abs() == distance {
        return ((front_x - distance)..=(front_x + distance)).collect();
    }

    vec![front_x - distance, front_x + distance]
}

#[allow(deprecated)]
pub(super) fn harvestable_carcasses_at(world: &World, point: &Point) -> Vec<Entity> {
    let mut entities = world
        .iter_entities()
        .filter_map(|entity| {
            let position = entity.get::<Position>()?;
            if position.0 != *point {
                return None;
            }

            let monster = entity.get::<MonsterAgent>()?;
            if !monster.dead {
                return None;
            }

            let state = entity
                .get::<HarvestMonsterState>()
                .copied()
                .or_else(|| initial_harvest_monster_state(monster.ai))?;
            if state.harvested {
                return None;
            }

            Some((entity.get::<ObjectId>()?.0, entity.id()))
        })
        .collect::<Vec<_>>();
    entities.sort_by_key(|(object_id, _)| *object_id);
    entities.into_iter().map(|(_, entity)| entity).collect()
}

pub(super) fn harvest_ownership_allows(
    world: &World,
    entity: Entity,
    player_object_id: u32,
) -> bool {
    let Some(ownership) = world.entity(entity).get::<HarvestOwnership>() else {
        return true;
    };
    if ownership.owner_object_id == player_object_id {
        return true;
    }
    world
        .resource::<GroupResource>()
        .group_member_object_ids
        .contains(&ownership.owner_object_id)
}

#[derive(Component, Clone)]
pub(super) struct DropPayload {
    pub(super) quantity: u32,
    pub(super) source_monster: String,
    pub(super) source_monster_key: Option<String>,
    pub(super) loot: DropLoot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SpawnedDrop {
    pub(super) object_id: u32,
    pub(super) position: Point,
}

#[derive(Debug, Clone)]
pub(super) enum DropLoot {
    Gold(u32),
    InventoryItem {
        key: String,
        name: String,
        description: String,
        weight: u16,
        durability_current: Option<u16>,
        durability_max: Option<u16>,
        added_attack: i32,
        added_defence: i32,
        added_stats: Vec<UserItemStat>,
        cursed: bool,
        socket_slots: u8,
        show_group_pickup: bool,
    },
}

#[derive(Debug, Clone)]
pub(super) enum ResolvedDropTemplate {
    Gold {
        name: String,
        amount: u32,
        quantity: u32,
    },
    Item {
        key: String,
        name: String,
        description: String,
        weight: u16,
        quantity: u32,
        durability_current: Option<u16>,
        durability_max: Option<u16>,
        added_attack: i32,
        added_defence: i32,
        added_stats: Vec<UserItemStat>,
        cursed: bool,
        socket_slots: u8,
        item_type: u8,
        show_group_pickup: bool,
        quest_required: bool,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct CrystalRandomDropStats {
    pub(super) durability_bonus: u16,
    pub(super) added_attack: i32,
    pub(super) added_defence: i32,
    pub(super) added_stats: Vec<UserItemStat>,
    pub(super) cursed: bool,
    pub(super) socket_slots: u8,
}

pub(super) fn resolved_monster_drop_templates(
    world: &World,
    monster_object_id: u32,
    monster_name: &str,
) -> Vec<ResolvedDropTemplate> {
    let current_tick = runtime_tick(world);
    resolved_monster_drop_templates_at_tick(monster_object_id, monster_name, current_tick)
}

pub(super) fn resolved_monster_drop_templates_at_tick(
    monster_object_id: u32,
    monster_name: &str,
    current_tick: u64,
) -> Vec<ResolvedDropTemplate> {
    let crystal_drops =
        crystal_monster_drop_templates(monster_name, monster_object_id, current_tick);
    if !crystal_drops.is_empty() {
        return crystal_drops;
    }

    starter_server_data()
        .monster_drops
        .into_iter()
        .find(|table| table.monster_object_id == monster_object_id)
        .map(|table| {
            table
                .drops
                .into_iter()
                .map(resolved_drop_template_from_starter)
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn resolved_drop_template_from_starter(drop: DropTemplate) -> ResolvedDropTemplate {
    match drop {
        DropTemplate::Gold {
            name,
            amount,
            quantity,
        } => ResolvedDropTemplate::Gold {
            name,
            amount,
            quantity,
        },
        DropTemplate::Item {
            key,
            name,
            description,
            weight,
            quantity,
        } => ResolvedDropTemplate::Item {
            key,
            name,
            description,
            weight,
            quantity,
            durability_current: None,
            durability_max: None,
            added_attack: 0,
            added_defence: 0,
            added_stats: Vec::new(),
            cursed: false,
            socket_slots: 0,
            item_type: 0,
            show_group_pickup: false,
            quest_required: false,
        },
    }
}

pub(super) fn crystal_monster_drop_templates(
    monster_name: &str,
    monster_object_id: u32,
    current_tick: u64,
) -> Vec<ResolvedDropTemplate> {
    let Some(drop_table) = crystal_drop_table_for_monster_name(monster_name) else {
        return Vec::new();
    };

    let mut drops = Vec::new();
    let mut entry_salt = 0_usize;
    for entry in drop_table
        .sections
        .into_iter()
        .flat_map(|section| section.entries)
    {
        if let Some(mut resolved) =
            crystal_attempt_drop_entry(&entry, current_tick, monster_object_id, entry_salt)
        {
            drops.append(&mut resolved);
        }
        entry_salt = entry_salt.saturating_add(1);
    }
    drops
}

pub(super) fn crystal_group_child_drop_salt(parent_salt: usize, child_index: usize) -> usize {
    1_000_000_usize
        .saturating_add(parent_salt.saturating_mul(10_000))
        .saturating_add(child_index)
}

pub(super) fn crystal_attempt_drop_entry(
    entry: &CrystalDropEntry,
    current_tick: u64,
    monster_object_id: u32,
    entry_salt: usize,
) -> Option<Vec<ResolvedDropTemplate>> {
    if !crystal_drop_entry_should_drop(entry, current_tick, monster_object_id, entry_salt) {
        return None;
    }

    if let Some(group) = &entry.group {
        return Some(crystal_resolve_group_drop(
            group,
            current_tick,
            monster_object_id,
            entry_salt,
        ));
    }

    resolved_drop_template_from_crystal_entry(entry, current_tick, monster_object_id, entry_salt)
        .map(|drop| vec![drop])
}

pub(super) fn crystal_resolve_group_drop(
    group: &mir2_game_data::CrystalDropGroup,
    current_tick: u64,
    monster_object_id: u32,
    entry_salt: usize,
) -> Vec<ResolvedDropTemplate> {
    let mut gold_drops = Vec::new();
    let mut item_drops = Vec::new();

    for (child_index, child) in group.entries.iter().enumerate() {
        let child_salt = crystal_group_child_drop_salt(entry_salt, child_index);
        let Some(resolved) =
            crystal_attempt_drop_entry(child, current_tick, monster_object_id, child_salt)
        else {
            continue;
        };

        for drop in resolved {
            match drop {
                ResolvedDropTemplate::Gold { .. } => gold_drops.push(drop),
                ResolvedDropTemplate::Item { .. } => item_drops.push(drop),
            }
        }

        if group.first {
            break;
        }
    }

    if group.random && !item_drops.is_empty() {
        let selected_index = usize::try_from(deterministic_roll(
            current_tick,
            usize::try_from(monster_object_id).expect("monster object id should fit usize"),
            entry_salt + 700_000,
            u64::try_from(item_drops.len()).expect("item drop count should fit u64"),
        ))
        .expect("random group item index should fit usize");
        gold_drops.push(item_drops.remove(selected_index));
    } else {
        gold_drops.extend(item_drops);
    }

    gold_drops
}

pub(super) fn crystal_drop_entry_should_drop(
    entry: &CrystalDropEntry,
    current_tick: u64,
    monster_object_id: u32,
    entry_index: usize,
) -> bool {
    match (entry.chance_numerator, entry.chance_denominator) {
        (Some(numerator), Some(denominator)) => deterministic_drop_ratio_roll(
            current_tick,
            monster_object_id,
            u64::try_from(entry_index).expect("entry index should fit u64"),
            numerator,
            denominator,
        ),
        _ => entry.chance_raw == "0",
    }
}

pub(super) fn deterministic_drop_ratio_roll(
    current_tick: u64,
    object_id: u32,
    salt: u64,
    numerator: u32,
    denominator: u32,
) -> bool {
    if numerator == 0 {
        return false;
    }
    if denominator <= numerator {
        return true;
    }

    deterministic_roll(
        current_tick,
        usize::try_from(object_id).expect("object id should fit usize"),
        usize::try_from(salt).expect("drop salt should fit usize"),
        u64::from(denominator),
    ) < u64::from(numerator)
}

pub(super) fn crystal_drop_gold_amount(
    base_amount: u32,
    current_tick: u64,
    monster_object_id: u32,
    entry_index: usize,
) -> u32 {
    let lower = base_amount / 2;
    let upper = base_amount.saturating_add(base_amount / 2);
    if upper <= lower {
        return lower;
    }

    lower
        + u32::try_from(deterministic_roll(
            current_tick,
            usize::try_from(monster_object_id).expect("monster object id should fit usize"),
            entry_index + 100_000,
            u64::from(upper - lower),
        ))
        .expect("gold amount roll should fit u32")
}

pub(super) fn crystal_ground_gold_chunks(gold: u32) -> Vec<u32> {
    let count = if gold / CRYSTAL_MAX_DROP_GOLD == 0 {
        1
    } else {
        gold / CRYSTAL_MAX_DROP_GOLD + 1
    };

    (0..count)
        .map(|index| {
            if index != count - 1 {
                CRYSTAL_MAX_DROP_GOLD
            } else {
                gold % CRYSTAL_MAX_DROP_GOLD
            }
        })
        .collect()
}

pub(super) fn crystal_drop_item_current_durability(
    max_durability: u16,
    current_tick: u64,
    monster_object_id: u32,
    entry_index: usize,
) -> u16 {
    if max_durability == 0 {
        return 0;
    }

    let roll = deterministic_roll(
        current_tick,
        usize::try_from(monster_object_id).expect("monster object id should fit usize"),
        entry_index + 300_000,
        u64::from(max_durability),
    );
    let current = u32::try_from(roll)
        .expect("item durability roll should fit u32")
        .saturating_add(1000);
    current.min(u32::from(max_durability)) as u16
}

pub(super) fn crystal_random_item_stat_profile(
    random_stats_id: u8,
) -> Option<CrystalRandomItemStatProfile> {
    if random_stats_id == 0 {
        return None;
    }

    mir2_game_data::crystal_random_item_stat_profile(random_stats_id)
}

pub(super) fn crystal_randomom_range(
    count: u8,
    rate: u8,
    current_tick: u64,
    monster_object_id: u32,
    entry_index: usize,
    salt: usize,
) -> u16 {
    if count == 0 || rate == 0 {
        return 0;
    }

    (0..count)
        .filter(|index| {
            deterministic_roll(
                current_tick,
                usize::try_from(monster_object_id).expect("monster object id should fit usize"),
                entry_index + salt + usize::from(*index),
                u64::from(rate),
            ) == 0
        })
        .count()
        .try_into()
        .expect("random stat roll count should fit u16")
}

pub(super) fn crystal_roll_random_stat_value(
    roll: CrystalRandomStatRoll,
    current_tick: u64,
    monster_object_id: u32,
    entry_index: usize,
    chance_salt: usize,
    value_salt: usize,
    add_one: bool,
) -> u16 {
    if roll.chance == 0
        || deterministic_roll(
            current_tick,
            usize::try_from(monster_object_id).expect("monster object id should fit usize"),
            entry_index + chance_salt,
            u64::from(roll.chance),
        ) != 0
    {
        return 0;
    }

    let count = if add_one {
        roll.max_stat.saturating_sub(1)
    } else {
        roll.max_stat
    };
    let value = crystal_randomom_range(
        count,
        roll.stat_chance,
        current_tick,
        monster_object_id,
        entry_index,
        value_salt,
    );
    if add_one {
        value.saturating_add(1)
    } else {
        value
    }
}

pub(super) fn crystal_roll_added_stat(
    stats: &mut Vec<UserItemStat>,
    stat: u8,
    roll: CrystalRandomStatRoll,
    current_tick: u64,
    monster_object_id: u32,
    entry_index: usize,
    chance_salt: usize,
    value_salt: usize,
) {
    let value = crystal_roll_random_stat_value(
        roll,
        current_tick,
        monster_object_id,
        entry_index,
        chance_salt,
        value_salt,
        true,
    );
    if value != 0 {
        stats.push(UserItemStat {
            stat,
            value: i32::from(value),
        });
    }
}

pub(super) fn crystal_random_drop_stats(
    template: &CrystalItemTemplate,
    current_tick: u64,
    monster_object_id: u32,
    entry_index: usize,
) -> CrystalRandomDropStats {
    let Some(profile) = crystal_random_item_stat_profile(template.random_stats_id) else {
        return CrystalRandomDropStats::default();
    };

    let durability_steps = crystal_roll_random_stat_value(
        profile.max_dura,
        current_tick,
        monster_object_id,
        entry_index,
        400_000,
        410_000,
        false,
    );
    let mut added_stats = Vec::new();
    crystal_roll_added_stat(
        &mut added_stats,
        CRYSTAL_STAT_MAX_AC,
        profile.max_ac,
        current_tick,
        monster_object_id,
        entry_index,
        420_000,
        430_000,
    );
    crystal_roll_added_stat(
        &mut added_stats,
        CRYSTAL_STAT_MAX_MAC,
        profile.max_mac,
        current_tick,
        monster_object_id,
        entry_index,
        432_000,
        434_000,
    );
    crystal_roll_added_stat(
        &mut added_stats,
        CRYSTAL_STAT_MAX_DC,
        profile.max_dc,
        current_tick,
        monster_object_id,
        entry_index,
        440_000,
        450_000,
    );
    crystal_roll_added_stat(
        &mut added_stats,
        CRYSTAL_STAT_MAX_MC,
        profile.max_mc,
        current_tick,
        monster_object_id,
        entry_index,
        452_000,
        454_000,
    );
    crystal_roll_added_stat(
        &mut added_stats,
        CRYSTAL_STAT_MAX_SC,
        profile.max_sc,
        current_tick,
        monster_object_id,
        entry_index,
        456_000,
        458_000,
    );
    crystal_roll_added_stat(
        &mut added_stats,
        CRYSTAL_STAT_ACCURACY,
        profile.accuracy,
        current_tick,
        monster_object_id,
        entry_index,
        460_000,
        462_000,
    );
    crystal_roll_added_stat(
        &mut added_stats,
        CRYSTAL_STAT_AGILITY,
        profile.agility,
        current_tick,
        monster_object_id,
        entry_index,
        464_000,
        466_000,
    );
    crystal_roll_added_stat(
        &mut added_stats,
        CRYSTAL_STAT_HP,
        profile.hp,
        current_tick,
        monster_object_id,
        entry_index,
        468_000,
        470_000,
    );
    crystal_roll_added_stat(
        &mut added_stats,
        CRYSTAL_STAT_MP,
        profile.mp,
        current_tick,
        monster_object_id,
        entry_index,
        472_000,
        474_000,
    );
    crystal_roll_added_stat(
        &mut added_stats,
        CRYSTAL_STAT_STRONG,
        profile.strong,
        current_tick,
        monster_object_id,
        entry_index,
        476_000,
        478_000,
    );
    crystal_roll_added_stat(
        &mut added_stats,
        CRYSTAL_STAT_MAGIC_RESIST,
        profile.magic_resist,
        current_tick,
        monster_object_id,
        entry_index,
        480_000,
        482_000,
    );
    crystal_roll_added_stat(
        &mut added_stats,
        CRYSTAL_STAT_POISON_RESIST,
        profile.poison_resist,
        current_tick,
        monster_object_id,
        entry_index,
        484_000,
        486_000,
    );
    crystal_roll_added_stat(
        &mut added_stats,
        CRYSTAL_STAT_HEALTH_RECOVERY,
        profile.hp_recovery,
        current_tick,
        monster_object_id,
        entry_index,
        488_000,
        490_000,
    );
    crystal_roll_added_stat(
        &mut added_stats,
        CRYSTAL_STAT_SPELL_RECOVERY,
        profile.mp_recovery,
        current_tick,
        monster_object_id,
        entry_index,
        492_000,
        494_000,
    );
    crystal_roll_added_stat(
        &mut added_stats,
        CRYSTAL_STAT_POISON_RECOVERY,
        profile.poison_recovery,
        current_tick,
        monster_object_id,
        entry_index,
        496_000,
        498_000,
    );
    crystal_roll_added_stat(
        &mut added_stats,
        CRYSTAL_STAT_CRITICAL_RATE,
        profile.critical_rate,
        current_tick,
        monster_object_id,
        entry_index,
        500_000,
        502_000,
    );
    crystal_roll_added_stat(
        &mut added_stats,
        CRYSTAL_STAT_CRITICAL_DAMAGE,
        profile.critical_damage,
        current_tick,
        monster_object_id,
        entry_index,
        504_000,
        506_000,
    );
    crystal_roll_added_stat(
        &mut added_stats,
        CRYSTAL_STAT_FREEZING,
        profile.freezing,
        current_tick,
        monster_object_id,
        entry_index,
        508_000,
        510_000,
    );
    crystal_roll_added_stat(
        &mut added_stats,
        CRYSTAL_STAT_POISON_ATTACK,
        profile.poison_attack,
        current_tick,
        monster_object_id,
        entry_index,
        512_000,
        514_000,
    );
    crystal_roll_added_stat(
        &mut added_stats,
        CRYSTAL_STAT_ATTACK_SPEED,
        profile.attack_speed,
        current_tick,
        monster_object_id,
        entry_index,
        516_000,
        518_000,
    );
    crystal_roll_added_stat(
        &mut added_stats,
        CRYSTAL_STAT_LUCK,
        profile.luck,
        current_tick,
        monster_object_id,
        entry_index,
        520_000,
        522_000,
    );
    let cursed = profile.curse_chance > 0
        && deterministic_roll(
            current_tick,
            usize::try_from(monster_object_id).expect("monster object id should fit usize"),
            entry_index + 524_000,
            100,
        ) <= u64::from(profile.curse_chance);
    let socket_slots = crystal_roll_random_stat_value(
        profile.slot,
        current_tick,
        monster_object_id,
        entry_index,
        526_000,
        528_000,
        true,
    )
    .max(u16::from(template.slots));

    let added_defence = added_stats
        .iter()
        .find(|stat| stat.stat == CRYSTAL_STAT_MAX_AC)
        .map(|stat| stat.value)
        .unwrap_or_default();
    let added_attack = added_stats
        .iter()
        .find(|stat| stat.stat == CRYSTAL_STAT_MAX_DC)
        .map(|stat| stat.value)
        .unwrap_or_default();

    CrystalRandomDropStats {
        durability_bonus: durability_steps.saturating_mul(1000),
        added_attack,
        added_defence,
        added_stats,
        cursed,
        socket_slots: u8::try_from(socket_slots).expect("socket slot count should fit in u8"),
    }
}

pub(super) fn resolved_drop_template_from_crystal_entry(
    entry: &CrystalDropEntry,
    current_tick: u64,
    monster_object_id: u32,
    entry_index: usize,
) -> Option<ResolvedDropTemplate> {
    let item_name = entry.item_name.trim();
    let quantity = entry.amount.unwrap_or(1).max(1);
    if item_name.eq_ignore_ascii_case("Gold") {
        return Some(ResolvedDropTemplate::Gold {
            name: "Gold".to_string(),
            amount: crystal_drop_gold_amount(
                quantity,
                current_tick,
                monster_object_id,
                entry_index,
            ),
            quantity: 1,
        });
    }

    let template = crystal_item_by_name(item_name)?;
    let random_stats =
        crystal_random_drop_stats(&template, current_tick, monster_object_id, entry_index);
    let durability_max = template
        .durability
        .saturating_add(random_stats.durability_bonus);
    let current_durability = if durability_max > 0 {
        Some(
            crystal_drop_item_current_durability(
                template.durability,
                current_tick,
                monster_object_id,
                entry_index,
            )
            .saturating_add(random_stats.durability_bonus),
        )
    } else {
        None
    };
    Some(ResolvedDropTemplate::Item {
        key: crystal_item_key_for_template(&template),
        name: template.name,
        description: template
            .tooltip
            .unwrap_or_else(|| "Crystal drop item.".to_string()),
        weight: u16::from(template.weight.max(1)),
        quantity,
        durability_current: current_durability,
        durability_max: (durability_max > 0).then_some(durability_max),
        added_attack: random_stats.added_attack,
        added_defence: random_stats.added_defence,
        added_stats: random_stats.added_stats,
        cursed: random_stats.cursed,
        socket_slots: random_stats.socket_slots,
        item_type: template.item_type,
        show_group_pickup: template.show_group_pickup,
        quest_required: entry
            .modifiers
            .iter()
            .any(|modifier| modifier.eq_ignore_ascii_case("Q")),
    })
}

pub(super) fn quest_template_matches_drop_item(
    quest: &QuestTemplate,
    key: &str,
    name: &str,
) -> bool {
    let item = &quest.quest_item;
    item.key.eq_ignore_ascii_case(key)
        || item.name.eq_ignore_ascii_case(name.trim())
        || normalize_crystal_item_key(&item.name).eq_ignore_ascii_case(key)
}

pub(super) fn try_gain_crystal_quest_drop(
    world: &mut World,
    key: &str,
    name: &str,
    quantity: u32,
    durability_current: Option<u16>,
    durability_max: Option<u16>,
    packets: &mut Vec<ServerPacket>,
) -> bool {
    let quantity = quantity.max(1);
    let mut matched_but_full = false;
    let quest_match = {
        let resources = world.resource::<InventoryResource>();
        world
            .resource::<QuestResource>()
            .quests
            .iter()
            .find_map(|state| {
                if state.stage != QuestStage::InProgress || state.current >= state.required {
                    return None;
                }
                let template = quest_template_by_id(state.quest_id)?;
                if !quest_template_matches_drop_item(&template, key, name) {
                    return None;
                }
                if !can_gain_item_quantity(
                    resources,
                    ItemContainer::Quest,
                    &template.quest_item.key,
                    quantity,
                ) {
                    matched_but_full = true;
                    return None;
                }
                Some(template)
            })
    };

    let Some(quest_template) = quest_match else {
        if matched_but_full {
            packets.push(system_message_key(
                world,
                "server.CannotCarryMoreQuestItems",
            ));
            return false;
        }
        if let Some(task_drop) = crystal_quest_item_task_drop(world, key, name, quantity) {
            let gained_item = add_or_increment_item_with_durability(
                world,
                ItemContainer::Quest,
                &task_drop.item_key,
                &task_drop.item_name,
                &task_drop.description,
                0,
                quantity,
                task_drop.weight,
                durability_current,
                durability_max,
            );
            packets.push(ServerPacket::GainedItem {
                item: user_item_from_item_state(&gained_item),
            });
            packets.push(system_message_key_args(
                world,
                "server.YouFound",
                [localized_item_name(
                    current_language(world),
                    &task_drop.item_key,
                    &task_drop.item_name,
                )],
            ));
            if advance_crystal_quest_item_task(
                world,
                task_drop.quest_id,
                &task_drop.task_key,
                quantity.min(task_drop.required).max(1),
            ) {
                if let Some(packet) = crystal_quest_update_packet(world, task_drop.quest_id) {
                    packets.push(packet);
                }
            }
            return true;
        }
        return false;
    };

    let gained_item = add_or_increment_item_with_durability(
        world,
        ItemContainer::Quest,
        &quest_template.quest_item.key,
        &quest_template.quest_item.name,
        &quest_template.quest_item.description,
        quest_template.quest_item.preferred_slot,
        quantity,
        quest_template.quest_item.weight,
        durability_current,
        durability_max,
    );
    packets.push(ServerPacket::GainedItem {
        item: user_item_from_item_state(&gained_item),
    });
    packets.push(system_message_key_args(
        world,
        "server.YouFound",
        [localized_item_name(
            current_language(world),
            &quest_template.quest_item.key,
            &quest_template.quest_item.name,
        )],
    ));

    {
        let mut quests = world.resource_mut::<QuestResource>();
        if let Some(quest) = quests
            .quests
            .iter_mut()
            .find(|quest| quest.quest_id == quest_template.quest_id)
        {
            quest.current = quest.current.saturating_add(quantity).min(quest.required);
            if quest.current >= quest.required {
                quest.stage = QuestStage::ReadyToTurnIn;
            }
        }
    }

    true
}

pub(super) fn spawn_configured_monster_drops(
    world: &mut World,
    monster_object_id: u32,
    position: &Point,
    monster_name: &str,
    packets: &mut Vec<ServerPacket>,
) {
    if current_map_disallows_monster_drop(world) {
        return;
    }

    let drops = resolved_monster_drop_templates(world, monster_object_id, monster_name);
    if drops.is_empty() {
        return;
    }

    let ownership = current_player_drop_ownership(world);
    for drop in drops {
        match drop {
            ResolvedDropTemplate::Gold {
                name,
                amount,
                quantity,
            } => {
                for gold_chunk in crystal_ground_gold_chunks(amount) {
                    let _ = drop_ground_drop_with_ownership(
                        world,
                        position.clone(),
                        CRYSTAL_DROP_RANGE,
                        monster_name,
                        localized_monster_name_key(monster_object_id).map(str::to_string),
                        DropLoot::Gold(gold_chunk),
                        quantity,
                        name.clone(),
                        ownership,
                    );
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
                item_type: _,
                show_group_pickup,
                quest_required,
            } => {
                if quest_required {
                    let _ = try_gain_crystal_quest_drop(
                        world,
                        &key,
                        &name,
                        quantity,
                        durability_current,
                        durability_max,
                        packets,
                    );
                    continue;
                }
                if drop_ground_drop_with_ownership(
                    world,
                    position.clone(),
                    CRYSTAL_DROP_RANGE,
                    monster_name,
                    localized_monster_name_key(monster_object_id).map(str::to_string),
                    DropLoot::InventoryItem {
                        key: key.clone(),
                        name: name.clone(),
                        description: description.clone(),
                        weight,
                        durability_current,
                        durability_max,
                        added_attack,
                        added_defence,
                        added_stats,
                        cursed,
                        socket_slots,
                        show_group_pickup,
                    },
                    quantity,
                    name.clone(),
                    ownership,
                )
                .is_none()
                {
                    return;
                }
            }
        }
    }
}

pub(super) fn zone_ground_drop_snapshots_for_monster(
    world: &World,
    monster_object_id: u32,
    monster_name: &str,
) -> Vec<GroundDropSnapshot> {
    if current_map_disallows_monster_drop(world) {
        return Vec::new();
    }

    resolved_monster_drop_templates(world, monster_object_id, monster_name)
        .into_iter()
        .filter_map(|drop| zone_ground_drop_snapshot_from_template(monster_name, drop))
        .collect()
}

pub(super) fn zone_ground_drop_snapshots_for_monster_at_tick(
    monster_object_id: u32,
    monster_name: &str,
    current_tick: u64,
) -> Vec<GroundDropSnapshot> {
    resolved_monster_drop_templates_at_tick(monster_object_id, monster_name, current_tick)
        .into_iter()
        .filter_map(|drop| zone_ground_drop_snapshot_from_template(monster_name, drop))
        .collect()
}

fn zone_ground_drop_snapshot_from_template(
    monster_name: &str,
    drop: ResolvedDropTemplate,
) -> Option<GroundDropSnapshot> {
    match drop {
        ResolvedDropTemplate::Gold {
            name,
            amount,
            quantity,
        } => Some(GroundDropSnapshot {
            object_id: 0,
            name,
            name_colour_argb: -1,
            x: 0,
            y: 0,
            quantity,
            source_monster: monster_name.to_string(),
            owner_object_id: None,
            ownership_remaining_ticks: Some(CRYSTAL_DROP_OWNER_WINDOW_TICKS),
            loot: GroundDropLootSnapshot::Gold { amount },
        }),
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
            show_group_pickup,
            quest_required,
            ..
        } => {
            if quest_required {
                return None;
            }
            let colour_probe = DropLoot::InventoryItem {
                key: key.clone(),
                name: name.clone(),
                description: description.clone(),
                weight,
                durability_current,
                durability_max,
                added_attack,
                added_defence,
                added_stats: added_stats.clone(),
                cursed,
                socket_slots,
                show_group_pickup,
            };
            Some(GroundDropSnapshot {
                object_id: 0,
                name: name.clone(),
                name_colour_argb: crystal_drop_name_colour_argb(&colour_probe),
                x: 0,
                y: 0,
                quantity,
                source_monster: monster_name.to_string(),
                owner_object_id: None,
                ownership_remaining_ticks: Some(CRYSTAL_DROP_OWNER_WINDOW_TICKS),
                loot: GroundDropLootSnapshot::InventoryItem {
                    key,
                    name,
                    description,
                    weight,
                    durability_current,
                    durability_max,
                    added_attack,
                    added_defence,
                    added_stats,
                    cursed,
                    socket_slots,
                    show_group_pickup,
                },
            })
        }
    }
}

#[derive(Debug, Clone)]
pub(super) enum HarvestDropTransfer {
    NoDrops,
    Blocked { packets: Vec<ServerPacket> },
    Transferred { packets: Vec<ServerPacket> },
}

pub(super) fn harvest_monster_entity(world: &mut World, entity: Entity) -> Vec<ServerPacket> {
    let Some(mut state) = ({
        let entry = world.entity(entity);
        let ai = entry
            .get::<MonsterAgent>()
            .map(|agent| agent.ai)
            .unwrap_or(0);
        entry
            .get::<HarvestMonsterState>()
            .copied()
            .or_else(|| initial_harvest_monster_state(ai))
    }) else {
        return Vec::new();
    };

    if state.harvested {
        return Vec::new();
    }

    let object_id = entity_object_id(world, entity).expect("harvest monster object id");
    let monster_name = entity_name(world, entity).unwrap_or_default();
    let mut packets = Vec::new();

    if state.remaining_skin_count == 0 {
        let transfer = if state.drops_ready {
            transfer_harvest_drops_to_player(world, entity, object_id)
        } else {
            HarvestDropTransfer::NoDrops
        };
        match transfer {
            HarvestDropTransfer::Blocked {
                packets: gained_packets,
            } => {
                packets.extend(gained_packets);
                packets.push(system_message_key(world, "server.YouCannotCarryAnymore"));
                world.entity_mut(entity).insert(state);
            }
            HarvestDropTransfer::NoDrops => {
                packets.push(system_message_key(world, "server.NothingWasFound"));
                mark_harvest_monster_harvested(world, entity, &mut state, &mut packets);
            }
            HarvestDropTransfer::Transferred {
                packets: gained_packets,
            } => {
                packets.extend(gained_packets);
                mark_harvest_monster_harvested(world, entity, &mut state, &mut packets);
            }
        }
        return packets;
    }

    state.remaining_skin_count = state.remaining_skin_count.saturating_sub(1);
    if state.remaining_skin_count > 0 {
        world.entity_mut(entity).insert(state);
        return packets;
    }

    let pending_drops = prepare_harvest_drops(world, object_id, &monster_name);
    if !pending_drops.is_empty() {
        state.drops_ready = true;
        world.entity_mut(entity).insert(state);
        world.entity_mut(entity).insert(PendingHarvestDrops {
            drops: pending_drops,
        });
    } else {
        packets.push(system_message_key(world, "server.NothingWasFound"));
        mark_harvest_monster_harvested(world, entity, &mut state, &mut packets);
    }

    packets
}

pub(super) fn mark_harvest_monster_harvested(
    world: &mut World,
    entity: Entity,
    state: &mut HarvestMonsterState,
    packets: &mut Vec<ServerPacket>,
) {
    state.harvested = true;
    state.drops_ready = false;
    world.entity_mut(entity).insert(*state);
    world.entity_mut(entity).remove::<PendingHarvestDrops>();

    if let Some(movement) = object_movement(world, entity) {
        packets.push(ServerPacket::ObjectHarvested { movement });
    }
}

pub(super) fn monster_uses_harvest_drops(world: &World, entity: Entity) -> bool {
    let entry = world.entity(entity);
    entry.get::<HarvestMonsterState>().is_some()
        || entry
            .get::<MonsterAgent>()
            .and_then(|agent| initial_harvest_monster_state(agent.ai))
            .is_some()
}

pub(super) fn prepare_harvest_drops(
    world: &World,
    monster_object_id: u32,
    monster_name: &str,
) -> Vec<ResolvedDropTemplate> {
    if current_map_disallows_monster_drop(world) {
        return Vec::new();
    }

    resolved_monster_drop_templates(world, monster_object_id, monster_name)
        .into_iter()
        .filter(|drop| match drop {
            ResolvedDropTemplate::Gold { .. } => false,
            ResolvedDropTemplate::Item {
                key,
                name,
                quantity,
                quest_required,
                ..
            } => {
                !*quest_required
                    || can_accept_crystal_quest_drop(world, key, name, (*quantity).max(1))
            }
        })
        .collect()
}

pub(super) fn can_accept_crystal_quest_drop(
    world: &World,
    key: &str,
    name: &str,
    quantity: u32,
) -> bool {
    let quantity = quantity.max(1);
    let starter_accepts = {
        let resources = world.resource::<InventoryResource>();
        world
            .resource::<QuestResource>()
            .quests
            .iter()
            .any(|state| {
                if state.stage != QuestStage::InProgress || state.current >= state.required {
                    return false;
                }
                let Some(template) = quest_template_by_id(state.quest_id) else {
                    return false;
                };
                quest_template_matches_drop_item(&template, key, name)
                    && can_gain_item_quantity(
                        resources,
                        ItemContainer::Quest,
                        &template.quest_item.key,
                        quantity,
                    )
            })
    };
    starter_accepts || crystal_quest_item_task_drop(world, key, name, quantity).is_some()
}

pub(super) fn transfer_harvest_drops_to_player(
    world: &mut World,
    entity: Entity,
    monster_object_id: u32,
) -> HarvestDropTransfer {
    let Some(pending) = world.entity(entity).get::<PendingHarvestDrops>() else {
        return HarvestDropTransfer::NoDrops;
    };
    if pending.drops.is_empty() {
        return HarvestDropTransfer::NoDrops;
    }

    let pending_drops = pending.drops.clone();
    let mut packets = Vec::new();
    let mut remaining = Vec::new();
    let harvest_quality = crystal_harvest_quality(world, monster_object_id);

    for drop in pending_drops.into_iter().rev() {
        let ResolvedDropTemplate::Item {
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
            item_type,
            show_group_pickup,
            quest_required,
        } = drop
        else {
            continue;
        };

        if quest_required && can_accept_crystal_quest_drop(world, &key, &name, quantity) {
            if try_gain_crystal_quest_drop(
                world,
                &key,
                &name,
                quantity,
                durability_current,
                durability_max,
                &mut packets,
            ) {
                continue;
            }
        }

        if can_gain_item_quantity(
            world.resource::<InventoryResource>(),
            ItemContainer::Bag1,
            &key,
            quantity,
        ) {
            let gained_item = add_or_increment_item_with_random_metadata(
                world,
                ItemContainer::Bag1,
                &key,
                &name,
                &description,
                8,
                quantity,
                weight,
                durability_current.map(|current| {
                    if item_type == CRYSTAL_ITEM_TYPE_MEAT {
                        current.saturating_add(harvest_quality)
                    } else {
                        current
                    }
                }),
                durability_max,
                added_attack,
                added_defence,
                added_stats,
                cursed,
                socket_slots,
            );
            packets.push(ServerPacket::GainedItem {
                item: user_item_from_item_state(&gained_item),
            });
        } else {
            remaining.push(ResolvedDropTemplate::Item {
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
                item_type,
                show_group_pickup,
                quest_required,
            });
        }
    }

    remaining.reverse();
    if remaining.is_empty() {
        world.entity_mut(entity).remove::<PendingHarvestDrops>();
        if packets.is_empty() {
            HarvestDropTransfer::NoDrops
        } else {
            HarvestDropTransfer::Transferred { packets }
        }
    } else {
        world
            .entity_mut(entity)
            .insert(PendingHarvestDrops { drops: remaining });
        HarvestDropTransfer::Blocked { packets }
    }
}

pub(super) fn crystal_harvest_quality(world: &World, monster_object_id: u32) -> u16 {
    let Some(entity) = entity_by_object_id(world, monster_object_id) else {
        return 0;
    };
    let entry = world.entity(entity);
    let Some(agent) = entry.get::<MonsterAgent>() else {
        return 0;
    };
    if agent.ai != 2 {
        return 0;
    }

    let runaway = entry
        .get::<MonsterAiState>()
        .map(|state| state.mode)
        .unwrap_or(false);
    let (roll_modulo, multiplier) = if runaway {
        (8_u64, 2000_u16)
    } else {
        (4_u64, 1000_u16)
    };
    let roll = deterministic_roll(
        0,
        usize::try_from(monster_object_id).expect("monster object id should fit usize"),
        200_000,
        roll_modulo,
    );
    u16::try_from(roll).expect("harvest quality roll should fit u16") * multiplier
}

pub(super) fn yimoogi_has_living_sister(world: &World, entity: Entity) -> bool {
    let Some(state) = world.entity(entity).get::<YimoogiState>() else {
        return false;
    };
    let Some(sister_object_id) = state.sister_object_id else {
        return false;
    };
    let Some(sister) = entity_by_object_id(world, sister_object_id) else {
        return false;
    };
    world
        .entity(sister)
        .get::<MonsterAgent>()
        .map(|agent| !agent.dead && agent.ai == 36)
        .unwrap_or(false)
}

pub(super) fn handle_monster_defeat(
    world: &mut World,
    monster_object_id: u32,
    monster_name: &str,
    packets: &mut Vec<ServerPacket>,
) {
    let Some(entity) = entity_by_object_id(world, monster_object_id) else {
        return;
    };
    let harvests_drops = monster_uses_harvest_drops(world, entity);
    let suppress_drops = yimoogi_has_living_sister(world, entity);
    let map_disallows_monster_drop = current_map_disallows_monster_drop(world);
    let position = entity_position(world, entity).expect("monster position");

    if harvests_drops {
        if let Some(player) = player_entity(world) {
            if let Some(owner_object_id) = entity_object_id(world, player) {
                world
                    .entity_mut(entity)
                    .insert(HarvestOwnership { owner_object_id });
            }
        }
    }

    if monster_object_id == FIELD_WASP_ID {
        let quest = guide_quest_template();
        if !map_disallows_monster_drop {
            let _ = try_gain_crystal_quest_drop(
                world,
                &quest.quest_item.key,
                &quest.quest_item.name,
                quest.quest_item.quantity,
                None,
                None,
                packets,
            );
        }

        if !harvests_drops && !suppress_drops {
            spawn_configured_monster_drops(
                world,
                monster_object_id,
                &position,
                monster_name,
                packets,
            );
        }
    } else if !harvests_drops && !suppress_drops {
        spawn_configured_monster_drops(world, monster_object_id, &position, monster_name, packets);
    }
}

pub(super) fn crystal_item_grade_for_key(key: &str) -> u8 {
    crystal_item_template_for_item_key(key)
        .map(|template| template.grade)
        .unwrap_or(0)
}

pub(super) fn crystal_item_name_colour_argb_for_drop(
    key: &str,
    added_attack: i32,
    added_defence: i32,
    added_stats: &[UserItemStat],
    socket_slots: u8,
) -> i32 {
    crystal_item_name_colour_argb(
        crystal_item_grade_for_key(key),
        drop_has_added_stats(added_attack, added_defence, added_stats, socket_slots),
    )
}

pub(super) fn crystal_drop_name_colour_argb(loot: &DropLoot) -> i32 {
    match loot {
        DropLoot::Gold(_) => -1,
        DropLoot::InventoryItem {
            key,
            added_attack,
            added_defence,
            added_stats,
            socket_slots,
            ..
        } => crystal_item_name_colour_argb_for_drop(
            key,
            *added_attack,
            *added_defence,
            added_stats,
            *socket_slots,
        ),
    }
}

pub(super) fn drop_has_added_stats(
    added_attack: i32,
    added_defence: i32,
    added_stats: &[UserItemStat],
    socket_slots: u8,
) -> bool {
    added_attack != 0 || added_defence != 0 || !added_stats.is_empty() || socket_slots != 0
}

pub(super) fn crystal_item_name_colour_argb(grade: u8, is_added: bool) -> i32 {
    if is_added {
        return 0xFF00_FFFFu32 as i32;
    }

    match grade {
        2 => 0xFF00_BFFFu32 as i32,
        3 => 0xFFFF_8C00u32 as i32,
        4 => 0xFFDD_A0DDu32 as i32,
        5 => 0xFFFF_0000u32 as i32,
        _ => -1,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DropPickupCandidateStatus {
    Pickable,
    OwnerBlocked,
    GoldCapBlocked,
    NoFreeSlot,
}

#[allow(deprecated)]
pub(super) fn current_cell_ground_drop_candidates(world: &World) -> Vec<(u32, Entity)> {
    let Some(player) = player_entity(world) else {
        return Vec::new();
    };
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };

    let mut candidates = world
        .iter_entities()
        .filter_map(|entity| {
            if entity.get::<GroundDrop>().is_none() {
                return None;
            }
            let object_id = entity.get::<ObjectId>()?.0;
            let position = entity.get::<Position>()?.0.clone();
            (position == player_position).then_some((object_id, entity.id()))
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(object_id, _)| *object_id);
    candidates
}

pub(super) fn drop_pickup_candidate_status(
    world: &World,
    drop_entity: Entity,
    picker_object_id: u32,
) -> DropPickupCandidateStatus {
    if !drop_ownership_allows_pickup(world, drop_entity, picker_object_id) {
        return DropPickupCandidateStatus::OwnerBlocked;
    }

    let resources = world.resource::<InventoryResource>();
    let payload = world
        .entity(drop_entity)
        .get::<DropPayload>()
        .expect("drop payload");
    match &payload.loot {
        DropLoot::Gold(amount) => {
            if can_gain_gold(world.resource::<PlayerRuntimeResource>(), *amount) {
                DropPickupCandidateStatus::Pickable
            } else {
                DropPickupCandidateStatus::GoldCapBlocked
            }
        }
        DropLoot::InventoryItem { key, .. } => {
            if !can_gain_item_quantity(resources, ItemContainer::Bag1, key, payload.quantity) {
                DropPickupCandidateStatus::NoFreeSlot
            } else {
                DropPickupCandidateStatus::Pickable
            }
        }
    }
}

pub(super) fn current_player_drop_ownership(world: &World) -> Option<DropOwnership> {
    let player = player_entity(world)?;
    let owner_object_id = entity_object_id(world, player)?;
    let current_tick = runtime_tick(world);
    Some(DropOwnership {
        owner_object_id,
        expires_at_tick: current_tick + CRYSTAL_DROP_OWNER_WINDOW_TICKS,
    })
}

pub(super) fn drop_ownership_allows_pickup(
    world: &World,
    drop_entity: Entity,
    picker_object_id: u32,
) -> bool {
    let Some(ownership) = world.entity(drop_entity).get::<DropOwnership>() else {
        return true;
    };
    let current_tick = runtime_tick(world);
    if current_tick > ownership.expires_at_tick {
        return true;
    }
    if ownership.owner_object_id == picker_object_id {
        return true;
    }
    world
        .resource::<GroupResource>()
        .group_member_object_ids
        .contains(&ownership.owner_object_id)
}

pub(super) fn can_gain_gold(player_runtime: &PlayerRuntimeResource, gold: u32) -> bool {
    u64::from(player_runtime.gold).saturating_add(u64::from(gold)) <= u64::from(u32::MAX)
}

#[cfg(test)]
pub(super) fn spawn_ground_drop(
    world: &mut World,
    position: Point,
    source_monster: &str,
    source_monster_key: Option<String>,
    loot: DropLoot,
    quantity: u32,
    display_name: String,
) -> SpawnedDrop {
    spawn_ground_drop_with_ownership(
        world,
        position,
        source_monster,
        source_monster_key,
        loot,
        quantity,
        display_name,
        None,
    )
}

pub(super) fn spawn_ground_drop_with_ownership(
    world: &mut World,
    position: Point,
    source_monster: &str,
    source_monster_key: Option<String>,
    loot: DropLoot,
    quantity: u32,
    display_name: String,
    ownership: Option<DropOwnership>,
) -> SpawnedDrop {
    let object_id = world
        .resource_mut::<ObjectIdAllocatorResource>()
        .next_drop_id();
    let expires_at_tick = runtime_tick(world).saturating_add(CRYSTAL_DROP_EXPIRE_TICKS);

    let mut entity = world.spawn((
        WorldObject,
        GroundDrop,
        DropExpiry { expires_at_tick },
        ObjectId(object_id),
        localized_drop_name_key(&display_name)
            .map(|key| DisplayName::localized(key, display_name.clone()))
            .unwrap_or_else(|| DisplayName::literal(display_name)),
        Position(position.clone()),
        DropPayload {
            quantity,
            source_monster: source_monster.to_string(),
            source_monster_key,
            loot,
        },
    ));
    if let Some(ownership) = ownership {
        entity.insert(ownership);
    }
    SpawnedDrop {
        object_id,
        position,
    }
}

pub(super) fn drop_ground_drop(
    world: &mut World,
    origin: Point,
    distance: i32,
    source_monster: &str,
    source_monster_key: Option<String>,
    loot: DropLoot,
    quantity: u32,
    display_name: String,
) -> Option<SpawnedDrop> {
    drop_ground_drop_with_ownership(
        world,
        origin,
        distance,
        source_monster,
        source_monster_key,
        loot,
        quantity,
        display_name,
        None,
    )
}

pub(super) fn drop_ground_drop_with_ownership(
    world: &mut World,
    origin: Point,
    distance: i32,
    source_monster: &str,
    source_monster_key: Option<String>,
    loot: DropLoot,
    quantity: u32,
    display_name: String,
    ownership: Option<DropOwnership>,
) -> Option<SpawnedDrop> {
    let position = crystal_ground_drop_position(world, &origin, distance)?;
    Some(spawn_ground_drop_with_ownership(
        world,
        position,
        source_monster,
        source_monster_key,
        loot,
        quantity,
        display_name,
        ownership,
    ))
}

#[allow(deprecated)]
pub(super) fn crystal_ground_drop_position(
    world: &World,
    origin: &Point,
    distance: i32,
) -> Option<Point> {
    let distance = distance.max(0);
    let mut best: Option<(Point, usize)> = None;

    for d in 0..=distance {
        for y in (origin.y - d)..=(origin.y + d) {
            let mut x = origin.x - d;
            let step = if (y - origin.y).abs() == d {
                1
            } else {
                (d * 2).max(1)
            };
            while x <= origin.x + d {
                let point = Point { x, y };
                x += step;

                if !crystal_ground_drop_point_is_valid(world, &point) {
                    continue;
                }

                let count = ground_drop_count_at(world, &point);
                if count >= CRYSTAL_DROP_STACK_SIZE {
                    continue;
                }
                if count == 0 {
                    return Some(point);
                }
                if best
                    .as_ref()
                    .map(|(_, best_count)| count < *best_count)
                    .unwrap_or(true)
                {
                    best = Some((point, count));
                }
            }
        }
    }

    best.map(|(point, _)| point)
}

pub(super) fn crystal_ground_drop_point_is_valid(world: &World, point: &Point) -> bool {
    !is_blocked_tile(world, point)
        && !has_blocking_entity(world, point, None)
        && !is_current_map_transfer_source(world, point)
}

#[allow(deprecated)]
pub(super) fn ground_drop_count_at(world: &World, point: &Point) -> usize {
    world
        .iter_entities()
        .filter(|entity| {
            entity.get::<GroundDrop>().is_some()
                && entity
                    .get::<Position>()
                    .map(|position| &position.0 == point)
                    .unwrap_or(false)
        })
        .count()
}

pub(super) fn is_current_map_transfer_source(world: &World, point: &Point) -> bool {
    let map = world.resource::<MapRuntimeResource>();
    let current_map = normalize_map_file_name(&map.current_map.file_name);
    let config = &world.resource::<RuntimeConfigResource>().config;
    config
        .map_transfers
        .iter()
        .filter(|transfer| normalize_map_file_name(&transfer.from_map_file_name) == current_map)
        .any(|transfer| point_in_bounds(&transfer.from_bounds, point))
        || crystal_movement_transfer_records_for_map(&map.current_map.file_name)
            .iter()
            .any(|transfer| {
                normalize_map_file_name(&transfer.from_map_file_name) == current_map
                    && point_in_bounds(&transfer.from_bounds, point)
            })
}

pub(super) fn pick_up_current_cell_ground_drop(world: &mut World) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }

    let Some(player) = player_entity(world) else {
        return Vec::new();
    };
    let picker_object_id = entity_object_id(world, player).expect("player object id");
    let candidates = current_cell_ground_drop_candidates(world);
    let mut owner_blocked = false;
    let mut fallback_status = None;

    for (object_id, drop_entity) in candidates {
        match drop_pickup_candidate_status(world, drop_entity, picker_object_id) {
            DropPickupCandidateStatus::Pickable => return pick_up_ground_drop(world, object_id),
            DropPickupCandidateStatus::OwnerBlocked => owner_blocked = true,
            status => {
                fallback_status.get_or_insert(status);
            }
        }
    }

    if owner_blocked {
        return vec![system_message_key(world, "server.CannotPickupNotOwner")];
    }

    match fallback_status {
        Some(DropPickupCandidateStatus::NoFreeSlot | DropPickupCandidateStatus::GoldCapBlocked) => {
            Vec::new()
        }
        Some(DropPickupCandidateStatus::Pickable | DropPickupCandidateStatus::OwnerBlocked)
        | None => Vec::new(),
    }
}

pub(super) fn pick_up_ground_drop(world: &mut World, object_id: u32) -> Vec<ServerPacket> {
    let Some(drop_entity) = entity_by_object_id(world, object_id) else {
        return Vec::new();
    };

    if world.entity(drop_entity).get::<GroundDrop>().is_none() {
        return Vec::new();
    }

    let player = player_entity(world).expect("player should exist");
    let player_object_id = entity_object_id(world, player).expect("player object id");
    let player_position = entity_position(world, player).expect("player position");
    let drop_position = entity_position(world, drop_entity).expect("drop position");
    if player_position != drop_position {
        return Vec::new();
    }
    if !drop_ownership_allows_pickup(world, drop_entity, player_object_id) {
        return vec![system_message_key(world, "server.CannotPickupNotOwner")];
    }

    let (display_name, payload) = {
        let entity = world.entity(drop_entity);
        (
            entity
                .get::<DisplayName>()
                .expect("drop name")
                .resolve(current_language(world)),
            entity.get::<DropPayload>().expect("drop payload").clone(),
        )
    };
    let player_name = world
        .entity(player)
        .get::<DisplayName>()
        .map(|name| name.resolve(current_language(world)))
        .unwrap_or_else(|| "Player".to_string());

    match payload.loot {
        DropLoot::Gold(amount) => {
            if !can_gain_gold(world.resource::<PlayerRuntimeResource>(), amount) {
                return Vec::new();
            }
            world.resource_mut::<PlayerRuntimeResource>().gold += amount;
            let _ = world.despawn(drop_entity);
            vec![ServerPacket::GainedGold { gold: amount }]
        }
        DropLoot::InventoryItem {
            key,
            name,
            description,
            weight,
            durability_current,
            durability_max,
            added_attack,
            added_defence,
            added_stats,
            cursed,
            socket_slots,
            show_group_pickup,
        } => {
            {
                let resources = world.resource::<InventoryResource>();
                if !can_gain_item_quantity(&resources, ItemContainer::Bag1, &key, payload.quantity)
                {
                    return vec![system_message_key(world, "server.YouCannotCarryAnymore")];
                }
            }
            let gained_item = add_or_increment_item_with_random_metadata(
                world,
                ItemContainer::Bag1,
                &key,
                &name,
                &description,
                8,
                payload.quantity,
                weight,
                durability_current,
                durability_max,
                added_attack,
                added_defence,
                added_stats,
                cursed,
                socket_slots,
            );
            let _ = world.despawn(drop_entity);
            let mut packets = vec![ServerPacket::GainedItem {
                item: user_item_from_item_state(&gained_item),
            }];
            if show_group_pickup
                && !world
                    .resource::<GroupResource>()
                    .group_member_object_ids
                    .is_empty()
            {
                let message = format_localized_text(
                    current_language(world),
                    "server.FriendlyPickedUpItem",
                    [player_name.as_str(), display_name.as_str()],
                );
                packets.push(ServerPacket::Chat {
                    message,
                    chat_type: ChatType::System,
                });
            }
            packets
        }
    }
}

pub(super) fn drop_gold_impl(world: &mut World, amount: u32) -> Vec<ServerPacket> {
    let Some(player) = player_entity(world) else {
        return Vec::new();
    };
    let Some(player_position) = entity_position(world, player) else {
        return Vec::new();
    };

    if world.resource::<PlayerRuntimeResource>().gold < amount {
        return Vec::new();
    }

    let Some(spawned) = drop_ground_drop(
        world,
        player_position.clone(),
        CRYSTAL_PLAYER_DROP_GOLD_RANGE,
        "",
        None,
        DropLoot::Gold(amount),
        amount,
        format!("{amount} Gold"),
    ) else {
        return Vec::new();
    };

    world.resource_mut::<PlayerRuntimeResource>().gold -= amount;

    vec![
        ServerPacket::ObjectGold {
            info: ObjectGoldInfo {
                object_id: spawned.object_id,
                gold: amount,
                location: spawned.position,
            },
        },
        ServerPacket::LoseGold { gold: amount },
    ]
}

pub(super) fn drop_item_packet(
    world: &mut World,
    unique_id: u64,
    count: u16,
    hero_inventory: bool,
) -> Vec<ServerPacket> {
    let failed_packet = ServerPacket::DropItem {
        unique_id,
        count,
        hero_inventory,
        success: false,
    };

    if current_player_is_dead(world) {
        return vec![failed_packet];
    }

    if current_map_disallows_throw_item(world) {
        return vec![
            system_message_key(world, "server.CanNotDrop"),
            failed_packet,
        ];
    }

    if hero_inventory {
        return vec![failed_packet];
    }

    let item_index = {
        let resources = world.resource::<InventoryResource>();
        resources
            .inventory_items
            .iter()
            .position(|item| item_matches_inventory_unique_id(item, unique_id))
    };

    let Some(index) = item_index else {
        return vec![failed_packet];
    };

    let item = world.resource::<InventoryResource>().inventory_items[index].clone();
    let requested = u32::from(count);
    if requested == 0 || requested > item.quantity {
        return vec![failed_packet];
    }

    if item_has_crystal_or_rental_bind_flag(&item, CRYSTAL_BIND_DONT_DROP) {
        return vec![failed_packet];
    }

    let destroy_on_drop = crystal_item_has_bind_flag(&item.key, CRYSTAL_BIND_DESTROY_ON_DROP);
    let Some(player) = player_entity(world) else {
        return vec![failed_packet];
    };
    let Some(player_position) = entity_position(world, player) else {
        return vec![failed_packet];
    };

    if !destroy_on_drop {
        if drop_ground_drop(
            world,
            player_position,
            CRYSTAL_PLAYER_DROP_ITEM_RANGE,
            "",
            None,
            DropLoot::InventoryItem {
                key: item.key.clone(),
                name: item.name.clone(),
                description: item.description.clone(),
                weight: item.weight,
                durability_current: item.durability_current,
                durability_max: item.durability_max,
                added_attack: item.added_attack,
                added_defence: item.added_defence,
                added_stats: item.added_stats.clone(),
                cursed: item.cursed,
                socket_slots: item.socket_slots,
                show_group_pickup: crystal_item_template_for_item_key(&item.key)
                    .map(|template| template.show_group_pickup)
                    .unwrap_or(false),
            },
            requested,
            item.name.clone(),
        )
        .is_none()
        {
            return vec![failed_packet];
        }
    }

    {
        let mut resources = world.resource_mut::<InventoryResource>();
        if requested == item.quantity {
            resources.inventory_items.remove(index);
        } else {
            resources.inventory_items[index].quantity -= requested;
        }
    }

    vec![ServerPacket::DropItem {
        unique_id,
        count,
        hero_inventory,
        success: true,
    }]
}

pub(super) fn tick_ground_drop_expiry(world: &mut World, tick: u64) {
    let expired_entities: Vec<Entity> = world
        .query_filtered::<(Entity, &DropExpiry), With<GroundDrop>>()
        .iter(world)
        .filter_map(|(entity, expiry)| (tick > expiry.expires_at_tick).then_some(entity))
        .collect();

    for entity in expired_entities {
        let _ = world.despawn(entity);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharedAccountInventoryTransactionKind {
    GroundDropPickup,
    MonsterKillAward,
    SkillItemConsumption,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SharedAccountInventoryTransactionReceipt {
    pub kind: SharedAccountInventoryTransactionKind,
    pub committed: bool,
    pub packets: Vec<ServerPacket>,
}

impl SharedAccountInventoryTransactionReceipt {
    fn ground_drop_pickup(committed: bool, packets: Vec<ServerPacket>) -> Self {
        Self {
            kind: SharedAccountInventoryTransactionKind::GroundDropPickup,
            committed,
            packets,
        }
    }

    fn monster_kill_award(committed: bool, packets: Vec<ServerPacket>) -> Self {
        Self {
            kind: SharedAccountInventoryTransactionKind::MonsterKillAward,
            committed,
            packets,
        }
    }

    pub(crate) fn skill_item_consumption(committed: bool, packets: Vec<ServerPacket>) -> Self {
        Self {
            kind: SharedAccountInventoryTransactionKind::SkillItemConsumption,
            committed,
            packets,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SharedGroundDropPickupCommit {
    pub committed: bool,
    pub packets: Vec<ServerPacket>,
}

impl SimulationSession {
    pub fn pick_up(&mut self, object_id: u32) -> Vec<ServerPacket> {
        let packets = self.pick_up_impl(object_id);
        self.finalize_packets(packets)
    }

    pub fn apply_shared_ground_drop_pickup(
        &mut self,
        drop: &GroundDropSnapshot,
    ) -> Vec<ServerPacket> {
        self.apply_shared_ground_drop_pickup_commit(drop).packets
    }

    pub fn apply_shared_ground_drop_pickup_commit(
        &mut self,
        drop: &GroundDropSnapshot,
    ) -> SharedGroundDropPickupCommit {
        let receipt = self.commit_shared_ground_drop_pickup_transaction(drop);
        SharedGroundDropPickupCommit {
            committed: receipt.committed,
            packets: receipt.packets,
        }
    }

    pub fn apply_shared_monster_kill_award(
        &mut self,
        monster_object_id: u32,
        monster_name: &str,
        experience: u32,
    ) -> Vec<ServerPacket> {
        self.commit_shared_monster_kill_award_transaction(
            monster_object_id,
            monster_name,
            experience,
        )
        .packets
    }

    pub fn commit_shared_ground_drop_pickup_transaction(
        &mut self,
        drop: &GroundDropSnapshot,
    ) -> SharedAccountInventoryTransactionReceipt {
        let receipt = self.commit_shared_ground_drop_pickup_transaction_impl(drop);
        SharedAccountInventoryTransactionReceipt {
            kind: receipt.kind,
            committed: receipt.committed,
            packets: self.finalize_packets(receipt.packets),
        }
    }

    pub fn commit_shared_monster_kill_award_transaction(
        &mut self,
        monster_object_id: u32,
        monster_name: &str,
        experience: u32,
    ) -> SharedAccountInventoryTransactionReceipt {
        let receipt = self.commit_shared_monster_kill_award_transaction_impl(
            monster_object_id,
            monster_name,
            experience,
        );
        SharedAccountInventoryTransactionReceipt {
            kind: receipt.kind,
            committed: receipt.committed,
            packets: self.finalize_packets(receipt.packets),
        }
    }

    fn commit_shared_monster_kill_award_transaction_impl(
        &mut self,
        monster_object_id: u32,
        monster_name: &str,
        experience: u32,
    ) -> SharedAccountInventoryTransactionReceipt {
        let world = self.app.world_mut();
        if !is_in_world(world) {
            return SharedAccountInventoryTransactionReceipt::monster_kill_award(false, Vec::new());
        }

        let mut packets = advance_crystal_quest_kill(world, monster_name);
        packets.extend(advance_bespoke_quest_kill(world, monster_name));
        if monster_object_id == FIELD_WASP_ID && !current_map_disallows_monster_drop(world) {
            let quest = guide_quest_template();
            let _ = try_gain_crystal_quest_drop(
                world,
                &quest.quest_item.key,
                &quest.quest_item.name,
                quest.quest_item.quantity,
                None,
                None,
                &mut packets,
            );
        }
        if experience > 0 {
            // Marriage / mentorship / guild membership grant a Crystal-style
            // experience-rate bonus (no-op for an unattached player).
            let experience = super::stats::crystal_apply_social_exp_rate(world, experience);
            // GainExp: accumulate, resolve any level-ups, and emit
            // GainExperience/LevelChanged (Crystal PlayerObject.GainExp).
            packets.extend(super::leveling::apply_player_experience_gain(
                world,
                i64::from(experience),
            ));
        }
        SharedAccountInventoryTransactionReceipt::monster_kill_award(true, packets)
    }

    pub(super) fn pick_up_impl(&mut self, object_id: u32) -> Vec<ServerPacket> {
        if !is_in_world(self.app.world()) {
            return Vec::new();
        }

        pick_up_ground_drop(self.app.world_mut(), object_id)
    }

    fn commit_shared_ground_drop_pickup_transaction_impl(
        &mut self,
        drop: &GroundDropSnapshot,
    ) -> SharedAccountInventoryTransactionReceipt {
        let world = self.app.world_mut();
        if !is_in_world(world) {
            return SharedAccountInventoryTransactionReceipt::ground_drop_pickup(false, Vec::new());
        }

        match &drop.loot {
            GroundDropLootSnapshot::Gold { amount } => {
                if !can_gain_gold(world.resource::<PlayerRuntimeResource>(), *amount) {
                    return SharedAccountInventoryTransactionReceipt::ground_drop_pickup(
                        false,
                        Vec::new(),
                    );
                }
                world.resource_mut::<PlayerRuntimeResource>().gold += *amount;
                SharedAccountInventoryTransactionReceipt::ground_drop_pickup(
                    true,
                    vec![ServerPacket::GainedGold { gold: *amount }],
                )
            }
            GroundDropLootSnapshot::InventoryItem {
                key,
                name,
                description,
                weight,
                durability_current,
                durability_max,
                added_attack,
                added_defence,
                added_stats,
                cursed,
                socket_slots,
                show_group_pickup,
            } => {
                {
                    let resources = world.resource::<InventoryResource>();
                    if !can_gain_item_quantity(resources, ItemContainer::Bag1, key, drop.quantity) {
                        return SharedAccountInventoryTransactionReceipt::ground_drop_pickup(
                            false,
                            vec![system_message_key(world, "server.YouCannotCarryAnymore")],
                        );
                    }
                }

                let player = player_entity(world).expect("player should exist");
                let player_name = world
                    .entity(player)
                    .get::<DisplayName>()
                    .map(|name| name.resolve(current_language(world)))
                    .unwrap_or_else(|| "Player".to_string());
                let gained_item = add_or_increment_item_with_random_metadata(
                    world,
                    ItemContainer::Bag1,
                    key,
                    name,
                    description,
                    8,
                    drop.quantity,
                    *weight,
                    *durability_current,
                    *durability_max,
                    *added_attack,
                    *added_defence,
                    added_stats.clone(),
                    *cursed,
                    *socket_slots,
                );
                let mut packets = vec![ServerPacket::GainedItem {
                    item: user_item_from_item_state(&gained_item),
                }];
                if *show_group_pickup
                    && !world
                        .resource::<GroupResource>()
                        .group_member_object_ids
                        .is_empty()
                {
                    let message = format_localized_text(
                        current_language(world),
                        "server.FriendlyPickedUpItem",
                        [player_name.as_str(), drop.name.as_str()],
                    );
                    packets.push(ServerPacket::Chat {
                        message,
                        chat_type: ChatType::System,
                    });
                }
                SharedAccountInventoryTransactionReceipt::ground_drop_pickup(true, packets)
            }
        }
    }
}
