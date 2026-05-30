//! Mining nodes — Crystal `Map.CreateMine`, the `HumanObject` mining label and
//! `GetMinePayout`.
//!
//! Mining is triggered by a normal melee swing (no spell) into a blocked cell
//! that holds a mine spot, while wielding a `CanMine` weapon (a pickaxe). Each
//! swing depletes one stone from the spot; a successful hit may yield ore and
//! damages the pickaxe. Depleted spots regenerate their stone count on a timer.
//!
//! Mine-set rates mirror Crystal's `MineSet` defaults (HitRate 25, DropRate 10,
//! MaxStones 80, SpotRegenRate 5 min). Zone placement comes from
//! `SimulationConfig.mine_zones` because Crystal stores it in the Map DB rather
//! than the `.map` file.

use std::collections::BTreeMap;

use bevy_ecs::prelude::{Resource, World};
use mir2_game_data::crystal_item_by_name;
use mir2_protocol::{MirDirection, Point, ServerPacket};

use super::components::{current_player_object_id, entity_position, player_entity};
use super::crystal_compat::CRYSTAL_ITEM_TYPE_ORE;
use super::equipment::equipment_slot_unique_id;
use super::inventory::{add_or_increment_item_with_random_metadata, can_gain_item_quantity};
use super::items::{crystal_item_key_for_template, crystal_item_template_for_item_key};
use super::monsters::deterministic_roll;
use super::movement::offset_point;
use super::resources::{runtime_tick, InventoryResource, MapRuntimeResource, RuntimeConfigResource};
use crate::config::{EquipmentSlot, ItemContainer};

/// `SpellEffect.Mine` from Crystal's shared enums — the swing/dust visual.
const SPELL_EFFECT_MINE: u8 = 12;

#[derive(Debug, Clone)]
pub(super) struct MineDrop {
    pub item_name: &'static str,
    pub min_slot: u16,
    pub max_slot: u16,
    pub min_dura: u16,
    pub max_dura: u16,
    pub bonus_chance: u8,
    pub max_bonus_dura: u16,
}

#[derive(Debug, Clone)]
pub(super) struct MineSet {
    pub spot_regen_rate_minutes: u8,
    pub max_stones: u8,
    pub hit_rate: u8,
    pub drop_rate: u8,
    pub total_slots: u16,
    pub drops: Vec<MineDrop>,
}

#[derive(Debug, Clone)]
pub(super) struct MineSpot {
    pub mine_set_index: usize,
    pub stones_left: u8,
    pub last_regen_tick: u64,
}

#[derive(Resource, Debug, Clone, Default)]
pub(super) struct MiningResource {
    pub mine_sets: Vec<MineSet>,
    pub spots: BTreeMap<(i32, i32), MineSpot>,
}

impl MiningResource {
    pub(super) fn with_builtin_sets() -> Self {
        Self {
            mine_sets: builtin_mine_sets(),
            spots: BTreeMap::new(),
        }
    }
}

fn mine_drop(item_name: &'static str, min_slot: u16, max_slot: u16) -> MineDrop {
    MineDrop {
        item_name,
        min_slot,
        max_slot,
        min_dura: 3,
        max_dura: 16,
        bonus_chance: 20,
        max_bonus_dura: 10,
    }
}

/// The two built-in mine sets from Crystal's `MineSet(byte mineType)`.
fn builtin_mine_sets() -> Vec<MineSet> {
    let common = |total_slots: u16, drops: Vec<MineDrop>| MineSet {
        spot_regen_rate_minutes: 5,
        max_stones: 80,
        hit_rate: 25,
        drop_rate: 10,
        total_slots,
        drops,
    };
    vec![
        // Mine set 1 (config `mine_set = 1`).
        common(
            120,
            vec![
                mine_drop("GoldOre", 1, 2),
                mine_drop("SilverOre", 3, 20),
                mine_drop("CopperOre", 21, 45),
                mine_drop("BlackIronOre", 46, 56),
            ],
        ),
        // Mine set 2 (config `mine_set = 2`).
        common(
            100,
            vec![
                mine_drop("PlatinumOre", 1, 2),
                mine_drop("RubyOre", 3, 20),
                mine_drop("NephriteOre", 21, 45),
                mine_drop("AmethystOre", 46, 56),
            ],
        ),
    ]
}

/// Rebuild the mineable-cell map for the current map from `config.mine_zones`
/// (Crystal `Map.CreateMine`). Called whenever the active map changes.
pub(super) fn rebuild_mine_spots(world: &mut World) {
    let current_map = super::map::normalize_map_file_name(
        &world.resource::<MapRuntimeResource>().current_map.file_name,
    );
    let zones: Vec<(usize, i32, i32, u16)> = world
        .resource::<RuntimeConfigResource>()
        .config
        .mine_zones
        .iter()
        .filter(|zone| super::map::normalize_map_file_name(&zone.map_file_name) == current_map)
        .filter_map(|zone| {
            let set = zone.mine_set.checked_sub(1)? as usize;
            Some((set, zone.x, zone.y, zone.size))
        })
        .collect();

    let mut spots = BTreeMap::new();
    let set_count = world.resource::<MiningResource>().mine_sets.len();
    for (set_index, x0, y0, size) in zones {
        if set_index >= set_count {
            continue;
        }
        let size = i32::from(size);
        // Crystal iterates [loc - size, loc + size).
        for x in (x0 - size)..(x0 + size) {
            for y in (y0 - size)..(y0 + size) {
                spots.entry((x, y)).or_insert(MineSpot {
                    mine_set_index: set_index,
                    stones_left: 0,
                    last_regen_tick: 0,
                });
            }
        }
    }

    world.resource_mut::<MiningResource>().spots = spots;
}

struct EquippedPickaxe {
    unique_id: u64,
}

fn equipped_pickaxe(world: &World) -> Option<EquippedPickaxe> {
    let weapon = world
        .resource::<InventoryResource>()
        .equipment_items
        .iter()
        .find(|item| item.slot == EquipmentSlot::Weapon)?
        .clone();
    let can_mine = crystal_item_template_for_item_key(&weapon.key)
        .map(|template| template.can_mine)
        .unwrap_or(false);
    if !can_mine || weapon.durability_current == 0 {
        return None;
    }
    Some(EquippedPickaxe {
        unique_id: equipment_slot_unique_id(EquipmentSlot::Weapon).unwrap_or_default(),
    })
}

fn roll(tick: u64, object_id: u32, target: &Point, salt: usize, modulo: u64) -> u64 {
    let cell_salt = (target.x as usize)
        .wrapping_mul(73_856_093)
        .wrapping_add((target.y as usize).wrapping_mul(19_349_663))
        .wrapping_add(salt);
    deterministic_roll(tick, object_id as usize, cell_salt, modulo)
}

/// Attempt to mine the cell in front of the player.
///
/// Returns `None` when no mining occurs (no mine spot ahead, or no usable
/// pickaxe), so the caller can fall back to its normal "no target" handling.
/// Returns `Some(packets)` when a mining swing resolves.
pub(super) fn try_mine(world: &mut World, direction: MirDirection) -> Option<Vec<ServerPacket>> {
    let player = player_entity(world)?;
    let player_position = entity_position(world, player)?;
    let target = offset_point(&player_position, direction, 1);

    // Must be a mineable cell and we must wield a working pickaxe.
    if !world
        .resource::<MiningResource>()
        .spots
        .contains_key(&(target.x, target.y))
    {
        return None;
    }
    let pickaxe = equipped_pickaxe(world)?;

    let tick = runtime_tick(world);
    let object_id = current_player_object_id(world).unwrap_or_default();

    let mut packets = vec![ServerPacket::MapEffect {
        location: target.clone(),
        effect: SPELL_EFFECT_MINE,
        value: direction as u8,
    }];

    let (set_index, stones_left, last_regen_tick) = {
        let spot = world
            .resource::<MiningResource>()
            .spots
            .get(&(target.x, target.y))
            .expect("mine spot checked above");
        (spot.mine_set_index, spot.stones_left, spot.last_regen_tick)
    };
    let set = world.resource::<MiningResource>().mine_sets[set_index].clone();

    if stones_left > 0 {
        if let Some(spot) = world
            .resource_mut::<MiningResource>()
            .spots
            .get_mut(&(target.x, target.y))
        {
            spot.stones_left = spot.stones_left.saturating_sub(1);
        }

        let hit = roll(tick, object_id, &target, 0x10, 100) < u64::from(set.hit_rate);
        if hit {
            if roll(tick, object_id, &target, 0x20, 100) < u64::from(set.drop_rate) {
                if let Some(item_packet) = give_mine_payout(world, &set, tick, object_id, &target) {
                    packets.push(item_packet);
                }
            }
            // Damage the pickaxe by 5 + Random(15).
            let damage = 5 + roll(tick, object_id, &target, 0x30, 15) as u16;
            if let Some(packet) = damage_pickaxe(world, &pickaxe, damage) {
                packets.push(packet);
            }
        }
    } else if tick >= last_regen_tick {
        let regen_ticks = u64::from(set.spot_regen_rate_minutes) * 60;
        let new_stones = roll(tick, object_id, &target, 0x40, u64::from(set.max_stones).max(1)) as u8;
        if let Some(spot) = world
            .resource_mut::<MiningResource>()
            .spots
            .get_mut(&(target.x, target.y))
        {
            spot.last_regen_tick = tick.saturating_add(regen_ticks);
            spot.stones_left = new_stones;
        }
    }

    Some(packets)
}

fn damage_pickaxe(world: &mut World, pickaxe: &EquippedPickaxe, damage: u16) -> Option<ServerPacket> {
    let mut inventory = world.resource_mut::<InventoryResource>();
    let weapon = inventory
        .equipment_items
        .iter_mut()
        .find(|item| item.slot == EquipmentSlot::Weapon)?;
    weapon.durability_current = weapon.durability_current.saturating_sub(damage);
    Some(ServerPacket::DuraChanged {
        unique_id: pickaxe.unique_id,
        current_dura: weapon.durability_current,
    })
}

/// Roll an ore payout from the mine set (`GetMinePayout`), gaining the item if
/// there is bag space. Returns the `GainedItem` packet on success.
fn give_mine_payout(
    world: &mut World,
    set: &MineSet,
    tick: u64,
    object_id: u32,
    target: &Point,
) -> Option<ServerPacket> {
    let slot = roll(tick, object_id, target, 0x50, u64::from(set.total_slots).max(1)) as u16;
    for (index, drop) in set.drops.iter().enumerate() {
        if slot < drop.min_slot || slot > drop.max_slot {
            continue;
        }
        // Crystal skips drops whose item is not present in the item table.
        let template = crystal_item_by_name(drop.item_name)?;
        if template.item_type != CRYSTAL_ITEM_TYPE_ORE {
            // Not an ore in this item table — still respect Crystal's payout.
        }

        let span = u64::from(drop.max_dura.saturating_sub(drop.min_dura)).max(1);
        let base_units =
            u64::from(drop.min_dura) + roll(tick, object_id, target, 0x60 + index, span);
        let mut dura = base_units.saturating_mul(1_000).min(u64::from(u16::MAX)) as u16;
        if drop.bonus_chance > 0
            && roll(tick, object_id, target, 0x70 + index, 100) <= u64::from(drop.bonus_chance)
        {
            let bonus = roll(tick, object_id, target, 0x80 + index, u64::from(drop.max_bonus_dura).max(1))
                .saturating_mul(1_000);
            dura = (u64::from(dura) + bonus).min(u64::from(u16::MAX)) as u16;
        }

        let key = crystal_item_key_for_template(&template);
        {
            let inventory = world.resource::<InventoryResource>();
            if !can_gain_item_quantity(inventory, ItemContainer::Bag1, &key, 1) {
                return None;
            }
        }
        let description = template
            .tooltip
            .clone()
            .unwrap_or_else(|| format!("{} mined from the rock face.", template.name));
        let item = add_or_increment_item_with_random_metadata(
            world,
            ItemContainer::Bag1,
            &key,
            &template.name,
            &description,
            30,
            1,
            u16::from(template.weight.max(1)),
            Some(dura),
            Some(dura.max(1)),
            0,
            0,
            Vec::new(),
            false,
            0,
        );
        return Some(ServerPacket::GainedItem {
            item: super::items::user_item_from_item_state(&item),
        });
    }
    None
}
