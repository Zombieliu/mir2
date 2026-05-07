use bevy_ecs::prelude::World;
use mir2_game_data::{crystal_item_by_name, localized_text_or_fallback};
use mir2_protocol::{ChatType, ServerPacket};

use crate::config::{EquipmentSlot, ItemContainer};

use super::components::current_player_object_id;
use super::crystal_compat::{
    CRYSTAL_FISHING_ROD_SHAPES, CRYSTAL_ITEM_TYPE_BAIT, CRYSTAL_ITEM_TYPE_WEAPON,
};
use super::equipment::{equipment_slot_unique_id, EquipmentState};
use super::inventory::{add_or_increment_item, can_gain_item_quantity};
use super::items::{
    crystal_item_key_for_template, crystal_item_template_for_item_key, user_item_from_item_state,
    ItemState,
};
use super::movement::{current_location, offset_point};
use super::resources::{current_language, is_in_world, FishingResource, InventoryResource};

const CRYSTAL_FISHING_SUCCESS_START_PERCENT: i32 = 10;
const CRYSTAL_FISHING_PROGRESS_PER_TICK: i32 = 25;
const CRYSTAL_FISHING_CHANCE_PER_TICK: i32 = 15;
const CRYSTAL_FISHING_LOOT_ITEM: &str = "Walleye";

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

    if !has_equipped_fishing_rod(world) {
        return Vec::new();
    }
    if !world.resource::<FishingResource>().rod_has_reel {
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
            fishing.fishing = false;
            fishing.found_fish = false;
        }
        begin_fishing_cast(world, packets);
    }
    packets.push(fishing_update_packet(world));
}

fn begin_fishing_cast(world: &mut World, packets: &mut Vec<ServerPacket>) -> bool {
    if !fishing_rod_and_point_are_valid(world) {
        reject_fishing(world);
        return false;
    }
    if !world.resource::<FishingResource>().rod_has_hook {
        reject_fishing(world);
        packets.push(fishing_chat(world, "server.NeedHook", "NeedHook"));
        return false;
    }
    if !consume_one_fishing_bait(world) {
        reject_fishing(world);
        packets.push(fishing_chat(world, "server.YouNeedBait", "YouNeedBait"));
        return false;
    }
    if let Some(packet) = damage_equipped_fishing_rod(world) {
        packets.push(packet);
    }
    {
        let mut fishing = world.resource_mut::<FishingResource>();
        fishing.fishing = true;
        fishing.progress_percent = 0;
        fishing.chance_percent = CRYSTAL_FISHING_SUCCESS_START_PERCENT;
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

fn consume_one_fishing_bait(world: &mut World) -> bool {
    let mut inventory = world.resource_mut::<InventoryResource>();
    consume_one_fishing_bait_from_items(&mut inventory.inventory_items)
        || consume_one_fishing_bait_from_items(&mut inventory.belt_items)
}

fn consume_one_fishing_bait_from_items(items: &mut Vec<ItemState>) -> bool {
    let Some(index) = items.iter().position(item_is_fishing_bait) else {
        return false;
    };
    if items[index].quantity > 1 {
        items[index].quantity -= 1;
    } else {
        items.remove(index);
    }
    true
}

fn item_is_fishing_bait(item: &ItemState) -> bool {
    crystal_item_template_for_item_key(&item.key)
        .map(|template| template.item_type == CRYSTAL_ITEM_TYPE_BAIT)
        .unwrap_or_else(|| item.name.contains("Bait"))
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
