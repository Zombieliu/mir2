use bevy_ecs::prelude::World;
use mir2_game_data::crystal_item_by_name;
use mir2_protocol::ServerPacket;

use crate::config::ItemContainer;

use super::components::current_player_object_id;
use super::inventory::{add_or_increment_item, can_gain_item_quantity};
use super::items::{crystal_item_key_for_template, user_item_from_item_state};
use super::movement::{current_location, offset_point};
use super::resources::{is_in_world, FishingResource, InventoryResource};

const CRYSTAL_FISHING_SUCCESS_START_PERCENT: i32 = 10;
const CRYSTAL_FISHING_PROGRESS_PER_TICK: i32 = 25;
const CRYSTAL_FISHING_CHANCE_PER_TICK: i32 = 15;
const CRYSTAL_FISHING_LOOT_ITEM: &str = "Walleye";

pub(super) fn fishing_cast_impl(world: &mut World, cast_out: bool) -> Vec<ServerPacket> {
    if !is_in_world(world) {
        return Vec::new();
    }

    {
        let mut fishing = world.resource_mut::<FishingResource>();
        if cast_out {
            fishing.fishing = true;
            fishing.progress_percent = 0;
            fishing.chance_percent = CRYSTAL_FISHING_SUCCESS_START_PERCENT;
            fishing.found_fish = false;
        } else if fishing.fishing {
            fishing.fishing = false;
            fishing.progress_percent = 100;
        }
    }

    let mut packets = Vec::new();
    if !cast_out && world.resource::<FishingResource>().found_fish {
        packets.extend(award_fishing_loot(world));
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

    world.resource_mut::<FishingResource>().auto_cast = auto_cast;
    vec![fishing_update_packet(world)]
}

pub(super) fn tick_fishing(world: &mut World, packets: &mut Vec<ServerPacket>) {
    if !is_in_world(world) || !world.resource::<FishingResource>().fishing {
        return;
    }
    let auto_cast_ready = {
        let mut fishing = world.resource_mut::<FishingResource>();
        if fishing.found_fish {
            fishing.auto_cast
        } else {
            fishing.progress_percent =
                (fishing.progress_percent + CRYSTAL_FISHING_PROGRESS_PER_TICK).min(100);
            fishing.chance_percent =
                (fishing.chance_percent + CRYSTAL_FISHING_CHANCE_PER_TICK).min(100);
            if fishing.progress_percent >= 100 {
                fishing.found_fish = true;
            }
            false
        }
    };
    if auto_cast_ready {
        packets.extend(award_fishing_loot(world));
        {
            let mut fishing = world.resource_mut::<FishingResource>();
            fishing.fishing = true;
            fishing.progress_percent = 0;
            fishing.chance_percent = CRYSTAL_FISHING_SUCCESS_START_PERCENT;
            fishing.found_fish = false;
        }
    }
    packets.push(fishing_update_packet(world));
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

fn award_fishing_loot(world: &mut World) -> Vec<ServerPacket> {
    let Some(template) = crystal_item_by_name(CRYSTAL_FISHING_LOOT_ITEM) else {
        return Vec::new();
    };
    {
        let inventory = world.resource::<InventoryResource>();
        let key = crystal_item_key_for_template(&template);
        if !can_gain_item_quantity(&inventory, ItemContainer::Bag1, &key, 1) {
            return Vec::new();
        }
    }
    let key = crystal_item_key_for_template(&template);
    let item = add_or_increment_item(
        world,
        ItemContainer::Bag1,
        &key,
        &template.name,
        template
            .tooltip
            .as_deref()
            .unwrap_or("Freshly caught fish."),
        30,
        1,
        u16::from(template.weight),
    );
    vec![ServerPacket::GainedItem {
        item: user_item_from_item_state(&item),
    }]
}
