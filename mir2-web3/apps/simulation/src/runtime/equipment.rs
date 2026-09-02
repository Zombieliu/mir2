use serde::{Deserialize, Serialize};

use crate::config::{
    EquipmentItemSnapshot, EquipmentSlot, ItemContainer, ItemGrade, WorldItemTooltipSource,
};
use bevy_ecs::prelude::World;
use mir2_game_data::{
    crystal_npc_info_by_script_key, crystal_npc_script_by_key, localized_text_or_fallback,
    CrystalItemTemplate, EquipmentTemplate, LanguageCode,
};
use mir2_protocol::{
    ChatType, MirClass, MirGender, MirGridType, ServerPacket, UserItem, UserItemStat,
};

use super::buffs::{buff_attack_bonus, buff_defence_bonus};
use super::combat::deterministic_chance_roll;
use super::components::{
    current_character_index, current_player_is_dead, current_player_object_id,
};
use super::crystal_compat::{
    CRYSTAL_BIND_DONT_REPAIR, CRYSTAL_BIND_DONT_STORE, CRYSTAL_BIND_NO_SREPAIR,
    CRYSTAL_BIND_ON_EQUIP, CRYSTAL_ITEM_TYPE_BELLS, CRYSTAL_ITEM_TYPE_MOUNT, CRYSTAL_STAT_MAX_AC,
    CRYSTAL_STAT_MAX_DC,
};
use super::inventory::{
    collection_slot_occupied, item_matches_inventory_unique_id, remove_item_destination,
    storage_locked,
};
use super::items::{
    crystal_default_identified_for_item_key, crystal_item_has_bind_flag,
    crystal_item_needs_identify, crystal_item_requirement_rejection_key, crystal_item_stat_value,
    crystal_item_template_for_dynamic_key, crystal_item_template_for_item_key,
    default_item_unique_id, equipment_has_crystal_or_rental_bind_flag,
    item_has_crystal_or_rental_bind_flag, item_is_socket_type, item_state_can_equip_to_slot,
    item_state_identified, item_state_soul_bound_id, merged_user_item_stats,
    try_item_state_from_user_item, try_user_item_from_item_state, upsert_user_item_stat,
    user_item_from_item_state, ItemState, ItemStateUserItemMetadata,
};
use super::map::{current_map_disallows_mount, current_map_requires_bridle};
use super::monsters::deterministic_roll;
use super::npc::{
    active_crystal_storage_service, crystal_npc_script_item_types,
    current_crystal_npc_service_in_range, ActiveNpcServiceState,
};
use super::resources::{
    BuffResource, InventoryResource, MountResource, PlayerPermissionResource, PlayerRuntimeResource,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct EquipmentState {
    pub(super) key: String,
    pub(super) slot: EquipmentSlot,
    #[serde(default = "default_equipment_quantity")]
    pub(super) quantity: u32,
    pub(super) name: String,
    pub(super) icon: u16,
    pub(super) shape: Option<u16>,
    pub(super) description: String,
    pub(super) durability_current: u16,
    pub(super) durability_max: u16,
    #[serde(default)]
    pub(super) grade: ItemGrade,
    #[serde(default)]
    pub(super) added_attack: i32,
    #[serde(default)]
    pub(super) added_defence: i32,
    #[serde(default)]
    pub(super) added_luck: i32,
    #[serde(default)]
    pub(super) added_stats: Vec<UserItemStat>,
    /// Socket items (Crystal `ItemType.Socket`) inserted into this worn item's
    /// slots; their stats contribute to the wearer (RefreshSocketStats).
    #[serde(default)]
    pub(super) socketed: Vec<ItemState>,
    #[serde(default)]
    pub(super) cursed: bool,
    #[serde(default)]
    pub(super) socket_slots: u8,
    #[serde(default)]
    pub(super) gem_count: u16,
    /// Crystal `UserItem.Awake`: the awakening line on this worn item. `awake_type`
    /// is the [`AwakeType`] byte (0 = none, 1 = DC, …), `awake_values` holds one
    /// byte per awakened level (length = awake level, sum = awake value).
    #[serde(default)]
    pub(super) awake_type: u8,
    #[serde(default)]
    pub(super) awake_values: Vec<u8>,
    /// UserItem-only identity data retained while the item is worn.
    #[serde(default)]
    pub(super) user_item_metadata: Option<ItemStateUserItemMetadata>,
    /// Exact inventory/root protocol UID captured before this item was worn.
    /// `None` is a legacy equipment save; `Some(0)` is exact zero.
    #[serde(default)]
    pub(super) user_item_unique_id: Option<u64>,
    #[serde(default)]
    pub(super) identified: Option<bool>,
    #[serde(default)]
    pub(super) soul_bound_id: Option<i32>,
    #[serde(default)]
    pub(super) sealed_expiry_time_binary_datetime: i64,
    #[serde(default)]
    pub(super) sealed_next_time_binary_datetime: i64,
    #[serde(default)]
    pub(super) rental_binding_flags: i16,
    #[serde(default)]
    pub(super) rental_owner_name: String,
    #[serde(default)]
    pub(super) rental_expiry_binary_datetime: i64,
    #[serde(default)]
    pub(super) rental_locked: bool,
    pub(super) attack: i32,
    pub(super) defence: i32,
}

fn default_equipment_quantity() -> u32 {
    1
}

impl EquipmentState {
    pub(super) fn snapshot(&self, language: LanguageCode) -> EquipmentItemSnapshot {
        let user_item = user_item_from_equipment_state(self);
        let tooltip_info = user_item
            .as_ref()
            .and_then(|item| mir2_game_data::crystal_item_by_index(item.item_index))
            .or_else(|| crystal_item_template_for_item_key(&self.key));
        let tooltip_source = tooltip_info.map(|info| {
            let socket_infos = user_item
                .as_ref()
                .map(|item| {
                    item.slots
                        .iter()
                        .map(|slot| {
                            slot.as_ref().and_then(|socket| {
                                mir2_game_data::crystal_item_by_index(socket.item_index)
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            WorldItemTooltipSource {
                info,
                real_info: None,
                user_item,
                socket_infos,
                real_socket_infos: Vec::new(),
            }
        });
        EquipmentItemSnapshot {
            slot: self.slot,
            key: self.key.clone(),
            unique_id: self.user_item_unique_id,
            quantity: self.quantity,
            name: localized_equipment_name(language, &self.key, &self.name),
            icon: self.icon,
            state_image: crystal_item_template_for_item_key(&self.key)
                .map(|template| template.image)
                .unwrap_or_default(),
            shape: self.shape,
            description: localized_equipment_description(language, &self.key, &self.description),
            durability_current: self.durability_current,
            durability_max: self.durability_max,
            grade: self.grade,
            attack: self.total_attack(),
            defence: self.total_defence(),
            added_attack: if self.is_broken() {
                0
            } else {
                self.added_attack
            },
            added_defence: if self.is_broken() {
                0
            } else {
                self.added_defence
            },
            added_luck: if self.is_broken() { 0 } else { self.added_luck },
            socket_slots: self.socket_slots,
            sealed_expiry_time_binary_datetime: self.sealed_expiry_time_binary_datetime,
            sealed_next_time_binary_datetime: self.sealed_next_time_binary_datetime,
            tooltip_source,
        }
    }

    pub(super) fn is_broken(&self) -> bool {
        self.durability_max > 0 && self.durability_current == 0
    }

    pub(super) fn total_attack(&self) -> i32 {
        if self.is_broken() {
            0
        } else {
            let base = if self.attack != 0 {
                self.attack
            } else {
                crystal_item_template_for_item_key(&self.key)
                    .as_ref()
                    .map(|template| crystal_item_stat_value(template, CRYSTAL_STAT_MAX_DC))
                    .unwrap_or_default()
            };
            let added = self
                .added_stats
                .iter()
                .filter(|entry| entry.stat == CRYSTAL_STAT_MAX_DC)
                .map(|entry| entry.value)
                .sum::<i32>();
            base + if added != 0 { added } else { self.added_attack } + self.socketed_attack()
        }
    }

    pub(super) fn total_defence(&self) -> i32 {
        if self.is_broken() {
            0
        } else {
            let base = if self.defence != 0 {
                self.defence
            } else {
                crystal_item_template_for_item_key(&self.key)
                    .as_ref()
                    .map(|template| crystal_item_stat_value(template, CRYSTAL_STAT_MAX_AC))
                    .unwrap_or_default()
            };
            let added = self
                .added_stats
                .iter()
                .filter(|entry| entry.stat == CRYSTAL_STAT_MAX_AC)
                .map(|entry| entry.value)
                .sum::<i32>();
            base + if added != 0 {
                added
            } else {
                self.added_defence
            } + self.socketed_defence()
        }
    }

    /// DC contributed by socketed gems (Crystal RefreshSocketStats folds the
    /// socketed items' stats into the wearer's totals).
    fn socketed_attack(&self) -> i32 {
        self.socketed
            .iter()
            .map(|gem| {
                let base = if gem.attack != 0 {
                    gem.attack
                } else {
                    crystal_item_template_for_item_key(&gem.key)
                        .as_ref()
                        .map(|template| crystal_item_stat_value(template, CRYSTAL_STAT_MAX_DC))
                        .unwrap_or_default()
                };
                let added = gem
                    .added_stats
                    .iter()
                    .filter(|entry| entry.stat == CRYSTAL_STAT_MAX_DC)
                    .map(|entry| entry.value)
                    .sum::<i32>();
                base + if added != 0 { added } else { gem.added_attack }
            })
            .sum()
    }

    /// AC contributed by socketed gems.
    fn socketed_defence(&self) -> i32 {
        self.socketed
            .iter()
            .map(|gem| {
                let base = if gem.defence != 0 {
                    gem.defence
                } else {
                    crystal_item_template_for_item_key(&gem.key)
                        .as_ref()
                        .map(|template| crystal_item_stat_value(template, CRYSTAL_STAT_MAX_AC))
                        .unwrap_or_default()
                };
                let added = gem
                    .added_stats
                    .iter()
                    .filter(|entry| entry.stat == CRYSTAL_STAT_MAX_AC)
                    .map(|entry| entry.value)
                    .sum::<i32>();
                base + if added != 0 { added } else { gem.added_defence }
            })
            .sum()
    }

    /// `stat` contributed by socketed gems' added stats (accuracy/agility/etc.).
    pub(super) fn socketed_added_stat(&self, stat: u8) -> i32 {
        self.socketed
            .iter()
            .flat_map(|gem| gem.added_stats.iter())
            .filter(|entry| entry.stat == stat)
            .map(|entry| entry.value)
            .sum()
    }

    pub(super) fn socketed_count(&self) -> usize {
        self.socketed.len()
    }
}

pub(super) fn equipment_state_identified(item: &EquipmentState) -> bool {
    item.identified
        .unwrap_or_else(|| crystal_default_identified_for_item_key(&item.key))
}

pub(super) fn equipment_state_soul_bound_id(item: &EquipmentState) -> i32 {
    item.soul_bound_id.unwrap_or(-1)
}

pub(super) fn localized_equipment_base_key(key: &str) -> Option<&'static str> {
    match key {
        "wooden-sword" => Some("content.equipment.woodenSword"),
        "cloth-armour" => Some("content.equipment.clothArmour"),
        "copper-necklace" => Some("content.equipment.copperNecklace"),
        "wood-bracelet-left" | "wood-bracelet-right" => Some("content.equipment.woodBracelet"),
        "straw-sandals" => Some("content.equipment.strawSandals"),
        "rope-belt" => Some("content.equipment.ropeBelt"),
        "bronze-helmet-equipment" => Some("content.item.bronzeHelmet"),
        "iron-helmet-equipment" => Some("content.item.ironHelmet"),
        _ => None,
    }
}

pub(super) fn equipment_state_key(slot: EquipmentSlot, name: &str) -> String {
    match (slot, name) {
        (EquipmentSlot::Weapon, "Wooden Sword") => "wooden-sword".to_string(),
        (EquipmentSlot::Armour, "Cloth Armour") => "cloth-armour".to_string(),
        (EquipmentSlot::Helmet, "Bronze Helmet") => "bronze-helmet-equipment".to_string(),
        (EquipmentSlot::Helmet, "Iron Helmet") => "iron-helmet-equipment".to_string(),
        (EquipmentSlot::Necklace, "Copper Necklace") => "copper-necklace".to_string(),
        (EquipmentSlot::BraceletLeft, "Wood Bracelet") => "wood-bracelet-left".to_string(),
        (EquipmentSlot::BraceletRight, "Wood Bracelet") => "wood-bracelet-right".to_string(),
        (EquipmentSlot::RingRight, "CopperRing") => "crystal-item-404".to_string(),
        (EquipmentSlot::Boots, "Straw Sandals") => "straw-sandals".to_string(),
        (EquipmentSlot::Belt, "Rope Belt") => "rope-belt".to_string(),
        _ => format!("{}-{}", equipment_slot_key(slot), slugify_name(name)),
    }
}

pub(super) fn localized_equipment_name(
    language: LanguageCode,
    key: &str,
    fallback: &str,
) -> String {
    localized_equipment_base_key(key)
        .map(|base| localized_text_or_fallback(language, &format!("{base}.name"), fallback))
        .unwrap_or_else(|| fallback.to_string())
}

pub(super) fn localized_equipment_description(
    language: LanguageCode,
    key: &str,
    fallback: &str,
) -> String {
    localized_equipment_base_key(key)
        .map(|base| localized_text_or_fallback(language, &format!("{base}.description"), fallback))
        .unwrap_or_else(|| fallback.to_string())
}

pub(super) fn equipment_slot_from_template(slot: &str) -> EquipmentSlot {
    match slot {
        "weapon" => EquipmentSlot::Weapon,
        "armour" => EquipmentSlot::Armour,
        "helmet" => EquipmentSlot::Helmet,
        "mount" => EquipmentSlot::Mount,
        "necklace" => EquipmentSlot::Necklace,
        "torch" => EquipmentSlot::Torch,
        "bracelet_left" => EquipmentSlot::BraceletLeft,
        "bracelet_right" => EquipmentSlot::BraceletRight,
        "ring_left" => EquipmentSlot::RingLeft,
        "ring_right" => EquipmentSlot::RingRight,
        "amulet" => EquipmentSlot::Amulet,
        "boots" => EquipmentSlot::Boots,
        "belt" => EquipmentSlot::Belt,
        "stone" => EquipmentSlot::Stone,
        other => panic!("unknown equipment slot template: {other}"),
    }
}

pub(super) fn equipment_slot_key(slot: EquipmentSlot) -> &'static str {
    match slot {
        EquipmentSlot::Weapon => "weapon",
        EquipmentSlot::Armour => "armour",
        EquipmentSlot::Helmet => "helmet",
        EquipmentSlot::Mount => "mount",
        EquipmentSlot::Necklace => "necklace",
        EquipmentSlot::Torch => "torch",
        EquipmentSlot::BraceletLeft => "bracelet-left",
        EquipmentSlot::BraceletRight => "bracelet-right",
        EquipmentSlot::RingLeft => "ring-left",
        EquipmentSlot::RingRight => "ring-right",
        EquipmentSlot::Amulet => "amulet",
        EquipmentSlot::Boots => "boots",
        EquipmentSlot::Belt => "belt",
        EquipmentSlot::Stone => "stone",
    }
}

pub(super) fn equipment_slot_from_index(index: i32) -> Option<EquipmentSlot> {
    match index {
        0 => Some(EquipmentSlot::Weapon),
        1 => Some(EquipmentSlot::Armour),
        2 => Some(EquipmentSlot::Helmet),
        3 => Some(EquipmentSlot::Torch),
        4 => Some(EquipmentSlot::Necklace),
        5 => Some(EquipmentSlot::BraceletLeft),
        6 => Some(EquipmentSlot::BraceletRight),
        7 => Some(EquipmentSlot::RingLeft),
        8 => Some(EquipmentSlot::RingRight),
        9 => Some(EquipmentSlot::Amulet),
        10 => Some(EquipmentSlot::Belt),
        11 => Some(EquipmentSlot::Boots),
        12 => Some(EquipmentSlot::Stone),
        13 => Some(EquipmentSlot::Mount),
        _ => None,
    }
}

pub(super) fn equipment_slot_from_stage5_arg(value: &str) -> Option<EquipmentSlot> {
    if let Ok(index) = value.parse::<i32>() {
        return equipment_slot_from_index(index);
    }

    match value.trim().replace('_', "-").to_ascii_lowercase().as_str() {
        "weapon" => Some(EquipmentSlot::Weapon),
        "armour" | "armor" => Some(EquipmentSlot::Armour),
        "helmet" => Some(EquipmentSlot::Helmet),
        "torch" => Some(EquipmentSlot::Torch),
        "necklace" => Some(EquipmentSlot::Necklace),
        "bracelet-left" => Some(EquipmentSlot::BraceletLeft),
        "bracelet-right" => Some(EquipmentSlot::BraceletRight),
        "ring-left" => Some(EquipmentSlot::RingLeft),
        "ring-right" => Some(EquipmentSlot::RingRight),
        "amulet" => Some(EquipmentSlot::Amulet),
        "belt" => Some(EquipmentSlot::Belt),
        "boots" => Some(EquipmentSlot::Boots),
        "stone" => Some(EquipmentSlot::Stone),
        "mount" => Some(EquipmentSlot::Mount),
        _ => None,
    }
}

pub(super) fn equipment_slot_index(slot: EquipmentSlot) -> Option<usize> {
    match slot {
        EquipmentSlot::Weapon => Some(0),
        EquipmentSlot::Armour => Some(1),
        EquipmentSlot::Helmet => Some(2),
        EquipmentSlot::Torch => Some(3),
        EquipmentSlot::Necklace => Some(4),
        EquipmentSlot::BraceletLeft => Some(5),
        EquipmentSlot::BraceletRight => Some(6),
        EquipmentSlot::RingLeft => Some(7),
        EquipmentSlot::RingRight => Some(8),
        EquipmentSlot::Amulet => Some(9),
        EquipmentSlot::Belt => Some(10),
        EquipmentSlot::Boots => Some(11),
        EquipmentSlot::Stone => Some(12),
        EquipmentSlot::Mount => Some(13),
    }
}

pub(super) fn equipment_slot_unique_id(slot: EquipmentSlot) -> Option<u64> {
    equipment_slot_index(slot).and_then(|index| u64::try_from(index).ok())
}

fn validated_item_state_user_item(item: &ItemState) -> Option<UserItem> {
    let user_item = try_user_item_from_item_state(item).ok()?;
    mir2_game_data::crystal_item_by_index(user_item.item_index)?;
    Some(user_item)
}

fn item_state_matches_protocol_reference(
    item: &ItemState,
    grid: MirGridType,
    unique_id: u64,
) -> bool {
    let container_matches = match grid {
        MirGridType::Inventory => {
            matches!(item.container, ItemContainer::Bag1 | ItemContainer::Bag2)
        }
        MirGridType::Storage => item.container == ItemContainer::Storage,
        _ => false,
    };
    container_matches
        && validated_item_state_user_item(item)
            .is_some_and(|user_item| user_item.unique_id == unique_id)
}

fn unique_item_index_for_protocol_reference(
    items: &[ItemState],
    grid: MirGridType,
    unique_id: u64,
) -> Option<usize> {
    let mut matches = items.iter().enumerate().filter_map(|(index, item)| {
        item_state_matches_protocol_reference(item, grid, unique_id).then_some(index)
    });
    let index = matches.next()?;
    matches.next().is_none().then_some(index)
}

fn equipment_matches_protocol_reference(item: &EquipmentState, unique_id: u64) -> bool {
    user_item_from_equipment_state(item).is_some_and(|user_item| user_item.unique_id == unique_id)
}

fn unique_equipment_index_for_protocol_reference(
    resources: &InventoryResource,
    unique_id: u64,
) -> Option<usize> {
    let mut matches = resources
        .equipment_items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            equipment_matches_protocol_reference(item, unique_id).then_some(index)
        });
    let index = matches.next()?;
    matches.next().is_none().then_some(index)
}

pub(super) fn slugify_name(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

pub(super) fn equipment_template_to_state(template: &EquipmentTemplate) -> EquipmentState {
    let slot = equipment_slot_from_template(&template.slot);
    EquipmentState {
        key: equipment_state_key(slot, &template.name),
        slot,
        quantity: 1,
        name: template.name.clone(),
        icon: equipment_icon_for_slot_and_name(slot, &template.name),
        shape: template
            .shape
            .or_else(|| equipment_shape_for_slot_and_name(slot, &template.name)),
        description: template.description.clone(),
        durability_current: template.durability_current,
        durability_max: template.durability_max,
        grade: ItemGrade::Common,
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
        user_item_unique_id: None,
        identified: None,
        soul_bound_id: None,
        sealed_expiry_time_binary_datetime: 0,
        sealed_next_time_binary_datetime: 0,
        rental_binding_flags: 0,
        rental_owner_name: String::new(),
        rental_expiry_binary_datetime: 0,
        rental_locked: false,
        attack: template.attack,
        defence: template.defence,
    }
}

pub(super) fn equipment_icon_for_slot_and_name(slot: EquipmentSlot, name: &str) -> u16 {
    match (slot, name) {
        (EquipmentSlot::Weapon, "Wooden Sword") => 36,
        (EquipmentSlot::Weapon, "Dagger") => 37,
        (EquipmentSlot::Weapon, "Assassin Dagger") => 38,
        (EquipmentSlot::Weapon, "Training Bow") => 39,
        (EquipmentSlot::Armour, "Cloth Armour") => 94,
        (EquipmentSlot::Armour, "Leather Armour") => 95,
        (EquipmentSlot::Helmet, "Bronze Helmet") => 106,
        (EquipmentSlot::Helmet, "Iron Helmet") => 107,
        (EquipmentSlot::Necklace, "Copper Necklace") => 205,
        (EquipmentSlot::BraceletLeft | EquipmentSlot::BraceletRight, "Wood Bracelet") => 204,
        (EquipmentSlot::RingLeft | EquipmentSlot::RingRight, "CopperRing") => 145,
        (EquipmentSlot::Boots, "Straw Sandals") => 219,
        (EquipmentSlot::Belt, "Rope Belt") => 180,
        _ => match slot {
            EquipmentSlot::Weapon => 36,
            EquipmentSlot::Armour => 94,
            EquipmentSlot::Helmet => 106,
            EquipmentSlot::Mount => 208,
            EquipmentSlot::Necklace => 205,
            EquipmentSlot::Torch => 119,
            EquipmentSlot::BraceletLeft | EquipmentSlot::BraceletRight => 204,
            EquipmentSlot::RingLeft | EquipmentSlot::RingRight => 149,
            EquipmentSlot::Amulet => 201,
            EquipmentSlot::Boots => 219,
            EquipmentSlot::Belt => 180,
            EquipmentSlot::Stone => 239,
        },
    }
}

pub(super) fn equipment_shape_for_slot_and_name(slot: EquipmentSlot, name: &str) -> Option<u16> {
    match (slot, name) {
        (EquipmentSlot::Weapon, "Wooden Sword") => Some(0),
        (EquipmentSlot::Weapon, "Dagger") => Some(1),
        (EquipmentSlot::Weapon, "Assassin Dagger") => Some(100),
        (EquipmentSlot::Weapon, "Training Bow") => Some(200),
        (EquipmentSlot::Armour, "Cloth Armour") => Some(0),
        (EquipmentSlot::Armour, "Leather Armour") => Some(1),
        _ => None,
    }
}

pub(super) fn equipment_shape(
    items: Option<&[EquipmentState]>,
    slot: EquipmentSlot,
) -> Option<u16> {
    items.and_then(|items| {
        items
            .iter()
            .find(|item| item.slot == slot)
            .and_then(|item| item.shape)
    })
}

// Retained as test-introspection helpers (combat mitigation now rolls armour
// from the stat block rather than summing these flat totals).
#[allow(dead_code)]
pub(super) fn total_attack_bonus(resources: &InventoryResource, buffs: &BuffResource) -> i32 {
    resources
        .equipment_items
        .iter()
        .filter(|item| !item.is_broken())
        .map(EquipmentState::total_attack)
        .sum::<i32>()
        + buffs.buffs.iter().map(buff_attack_bonus).sum::<i32>()
}

#[allow(dead_code)]
pub(super) fn total_defence_bonus(resources: &InventoryResource, buffs: &BuffResource) -> i32 {
    resources
        .equipment_items
        .iter()
        .filter(|item| !item.is_broken())
        .map(EquipmentState::total_defence)
        .sum::<i32>()
        + buffs.buffs.iter().map(buff_defence_bonus).sum::<i32>()
}

fn item_grade_from_crystal(grade: u8) -> ItemGrade {
    match grade {
        1 => ItemGrade::Common,
        2 => ItemGrade::Rare,
        3 => ItemGrade::Legendary,
        _ => ItemGrade::None,
    }
}

fn crystal_start_equipment(item_index: i32, slot: EquipmentSlot) -> EquipmentState {
    let key = format!("crystal-item-{item_index}");
    let template = crystal_item_template_for_item_key(&key)
        .unwrap_or_else(|| panic!("Crystal start item {item_index} must exist in the manifest"));
    let attack = crystal_item_stat_value(&template, CRYSTAL_STAT_MAX_DC);
    let defence = crystal_item_stat_value(&template, CRYSTAL_STAT_MAX_AC);

    EquipmentState {
        key,
        slot,
        quantity: 1,
        name: template.name.clone(),
        icon: template.image,
        shape: u16::try_from(template.shape).ok(),
        description: format!("Crystal start item: {}.", template.name),
        durability_current: template.durability,
        durability_max: template.durability,
        grade: item_grade_from_crystal(template.grade),
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
        user_item_unique_id: None,
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
    }
}

pub(super) fn seed_equipment_items_for_character(
    class: MirClass,
    gender: MirGender,
) -> Vec<EquipmentState> {
    let weapon_index = match class {
        MirClass::Assassin => 281, // HoaSword
        MirClass::Archer => 298,   // WoodenBow
        _ => 221,                  // WoodenSword
    };
    let armour_index = match gender {
        MirGender::Female => 318, // BaseDress(F)
        _ => 317,                 // BaseDress(M)
    };

    vec![
        crystal_start_equipment(weapon_index, EquipmentSlot::Weapon),
        crystal_start_equipment(armour_index, EquipmentSlot::Armour),
    ]
}

pub(super) fn seed_equipment_items() -> Vec<EquipmentState> {
    seed_equipment_items_for_character(MirClass::Warrior, MirGender::Male)
}

pub(super) fn user_item_from_equipment_state(item: &EquipmentState) -> Option<UserItem> {
    if let Some(item_index) = item
        .user_item_metadata
        .as_ref()
        .and_then(|metadata| metadata.item_index)
    {
        mir2_game_data::crystal_item_by_index(item_index)?;
    } else {
        crystal_item_template_for_item_key(&item.key)?;
    }
    let slot = u8::try_from(equipment_slot_index(item.slot)?).ok()?;
    // Bag1 is only a temporary carrier discriminator: exact UID is restored
    // below, while the legacy fallback is the same zero-based slot index.
    let carrier = item_state_from_equipment_state(item.clone(), ItemContainer::Bag1, slot);
    try_user_item_from_item_state(&carrier).ok()
}

pub(super) fn replace_equipment(world: &mut World, next: EquipmentState) {
    {
        let mut resources = world.resource_mut::<InventoryResource>();
        if let Some(existing) = resources
            .equipment_items
            .iter_mut()
            .find(|item| item.slot == next.slot)
        {
            *existing = next;
        } else {
            resources.equipment_items.push(next);
        }
    }
    refresh_mount_resource_from_equipment(world);
    super::stats::refresh_player_stats(world);
}

/// Rebuild Crystal `MountInfo.CanAttack` input from the authoritative equipped
/// mount and its embedded `MountSlot.Bells` item. The client never supplies this
/// predicate directly.
pub(super) fn refresh_mount_resource_from_equipment(world: &mut World) {
    let (mount_type, has_bells) = world
        .resource::<InventoryResource>()
        .equipment_items
        .iter()
        .find(|item| item.slot == EquipmentSlot::Mount)
        .map(|mount| {
            let mount_type = mount
                .shape
                .and_then(|shape| i16::try_from(shape).ok())
                .unwrap_or_else(|| i16::try_from(mount.icon).unwrap_or(0));
            let has_bells = mount.socketed.iter().any(|item| {
                crystal_item_template_for_item_key(&item.key)
                    .is_some_and(|template| template.item_type == CRYSTAL_ITEM_TYPE_BELLS)
            });
            (mount_type, has_bells)
        })
        .unwrap_or((-1, false));
    let mut mount = world.resource_mut::<MountResource>();
    mount.mount_type = mount_type;
    mount.has_bells = has_bells;
    if mount_type < 0 {
        mount.riding_mount = false;
    }
}

pub(super) fn item_state_from_equipment_state(
    equipment: EquipmentState,
    container: ItemContainer,
    slot: u8,
) -> ItemState {
    let user_item_unique_id = equipment.user_item_unique_id;
    let mut user_item_metadata = equipment.user_item_metadata;
    if let Some(metadata) = user_item_metadata.as_mut() {
        // Awake is live while an item is worn, so it wins over an older
        // protocol sidecar on unequip.
        metadata.awake_type = equipment.awake_type;
        metadata.awake_values = equipment.awake_values.clone();
    } else if user_item_unique_id.is_some()
        || equipment.awake_type != 0
        || !equipment.awake_values.is_empty()
    {
        user_item_metadata = Some(ItemStateUserItemMetadata {
            item_index: None,
            awake_type: equipment.awake_type,
            awake_values: equipment.awake_values.clone(),
            refined_value: 0,
            refine_added: 0,
            refine_success_chance: 0,
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
        });
    }
    let mut added_stats = equipment.added_stats;
    upsert_user_item_stat(&mut added_stats, 15, equipment.added_luck);

    ItemState {
        key: equipment.key,
        name: equipment.name,
        icon: equipment.icon,
        slot,
        unique_id: user_item_unique_id.unwrap_or_else(|| default_item_unique_id(container, slot)),
        container,
        quantity: equipment.quantity,
        description: equipment.description,
        durability_current: Some(equipment.durability_current),
        durability_max: Some(equipment.durability_max),
        weight: 1,
        equip_slot: Some(equipment.slot),
        grade: equipment.grade,
        added_attack: equipment.added_attack,
        added_defence: equipment.added_defence,
        added_stats,
        socketed: equipment.socketed,
        user_item_metadata,
        cursed: equipment.cursed,
        socket_slots: equipment.socket_slots,
        gem_count: equipment.gem_count,
        identified: equipment.identified,
        soul_bound_id: equipment.soul_bound_id,
        sealed_expiry_time_binary_datetime: equipment.sealed_expiry_time_binary_datetime,
        sealed_next_time_binary_datetime: equipment.sealed_next_time_binary_datetime,
        rental_binding_flags: equipment.rental_binding_flags,
        rental_owner_name: equipment.rental_owner_name,
        rental_expiry_binary_datetime: equipment.rental_expiry_binary_datetime,
        rental_locked: equipment.rental_locked,
        attack: equipment.attack,
        defence: equipment.defence,
        heal_hp: 0,
        heal_mp: 0,
    }
}

pub(super) fn equipment_state_from_item_state(
    item: &ItemState,
    slot: EquipmentSlot,
) -> EquipmentState {
    let durability_current = item.durability_current.unwrap_or(10);
    let durability_max = item.durability_max.unwrap_or(durability_current);
    let validated_user_item = validated_item_state_user_item(item);
    let user_item_unique_id = validated_user_item.as_ref().map(|item| item.unique_id);
    // Once a legacy sidecar-less bag item enters the exact equipment path, hydrate
    // its Crystal item index into the retained carrier. Otherwise the equipment
    // root UID would be exact while its metadata remained legacy/ambiguous, and
    // the next strict save preflight would correctly reject it.
    let user_item_metadata = item.user_item_metadata.clone().or_else(|| {
        validated_user_item.as_ref().and_then(|user_item| {
            try_item_state_from_user_item(item.clone(), user_item)
                .ok()
                .and_then(|state| state.user_item_metadata)
        })
    });
    EquipmentState {
        key: item.key.clone(),
        slot,
        quantity: item.quantity,
        name: item.name.clone(),
        icon: item.icon,
        // Crystal `Looks_Armour`/`Looks_Weapon` come from the worn item's
        // `ItemInfo.Shape` (HumanObject.RefreshStats / SetLooks). Read the
        // authoritative template shape first so equipping any real item changes
        // the rendered body/weapon (`CArmour/{shape}` / `CWeapon/{shape}` in
        // `entity_sprite_snapshot`); fall back to the legacy name lookup only for
        // items whose key does not resolve to a Crystal template.
        shape: crystal_item_template_for_item_key(&item.key)
            .map(|template| u16::try_from(template.shape).unwrap_or(0))
            .or_else(|| equipment_shape_for_slot_and_name(slot, &item.name)),
        description: item.description.clone(),
        durability_current,
        durability_max,
        grade: item.grade,
        added_attack: item.added_attack,
        added_defence: item.added_defence,
        added_luck: 0,
        added_stats: item.added_stats.clone(),
        socketed: item.socketed.clone(),
        cursed: item.cursed,
        socket_slots: item.socket_slots,
        gem_count: item.gem_count,
        awake_type: user_item_metadata
            .as_ref()
            .map_or(0, |metadata| metadata.awake_type),
        awake_values: user_item_metadata
            .as_ref()
            .map_or_else(Vec::new, |metadata| metadata.awake_values.clone()),
        user_item_metadata,
        user_item_unique_id,
        identified: item.identified,
        soul_bound_id: item.soul_bound_id,
        sealed_expiry_time_binary_datetime: item.sealed_expiry_time_binary_datetime,
        sealed_next_time_binary_datetime: item.sealed_next_time_binary_datetime,
        rental_binding_flags: item.rental_binding_flags,
        rental_owner_name: item.rental_owner_name.clone(),
        rental_expiry_binary_datetime: item.rental_expiry_binary_datetime,
        rental_locked: item.rental_locked,
        attack: item.attack,
        defence: item.defence,
    }
}

pub(super) fn equipment_uses_durability(item: &EquipmentState) -> bool {
    item.durability_max > 0 && item.slot != EquipmentSlot::Amulet
}

pub(super) fn equipment_can_lose_durability(item: &EquipmentState) -> bool {
    equipment_uses_durability(item) && item.durability_current > 0
}

pub(super) fn crystal_weapon_durability_loss(current_tick: u64) -> u16 {
    u16::try_from(deterministic_roll(current_tick, 0, 0, 4) + 1)
        .expect("weapon durability loss should fit u16")
}

pub(super) fn crystal_worn_durability_loss(_current_tick: u64) -> u16 {
    1
}

pub(super) fn damage_weapon_durability(
    world: &mut World,
    current_tick: u64,
) -> Option<ServerPacket> {
    let amount = crystal_weapon_durability_loss(current_tick);
    let mut resources = world.resource_mut::<InventoryResource>();
    let item = resources
        .equipment_items
        .iter_mut()
        .find(|item| item.slot == EquipmentSlot::Weapon)?;

    if !damage_equipment_item(item, amount) {
        return None;
    }

    Some(ServerPacket::DuraChanged {
        unique_id: equipment_slot_unique_id(item.slot)?,
        current_dura: item.durability_current,
    })
}

pub(super) fn damage_worn_durability(world: &mut World, current_tick: u64) -> Vec<ServerPacket> {
    let amount = crystal_worn_durability_loss(current_tick);
    let mut packets = Vec::new();
    let mut resources = world.resource_mut::<InventoryResource>();

    for item in resources
        .equipment_items
        .iter_mut()
        .filter(|item| item.slot != EquipmentSlot::Weapon)
    {
        if damage_equipment_item(item, amount) {
            if let Some(unique_id) = equipment_slot_unique_id(item.slot) {
                packets.push(ServerPacket::DuraChanged {
                    unique_id,
                    current_dura: item.durability_current,
                });
            }
        }
    }

    packets
}

pub(super) fn damage_equipment_item(item: &mut EquipmentState, amount: u16) -> bool {
    if !equipment_can_lose_durability(item) {
        return false;
    }

    item.durability_current = item.durability_current.saturating_sub(amount);
    true
}

pub(super) fn repair_equipped_weapon_with_oil(
    world: &mut World,
    full_repair: bool,
) -> Option<ServerPacket> {
    let mut resources = world.resource_mut::<InventoryResource>();
    let weapon = resources
        .equipment_items
        .iter_mut()
        .find(|item| item.slot == EquipmentSlot::Weapon)?;
    if !equipment_uses_durability(weapon) || weapon.durability_current >= weapon.durability_max {
        return None;
    }
    if equipment_has_crystal_or_rental_bind_flag(weapon, CRYSTAL_BIND_DONT_REPAIR)
        || (full_repair
            && equipment_has_crystal_or_rental_bind_flag(weapon, CRYSTAL_BIND_NO_SREPAIR))
    {
        return None;
    }

    if full_repair {
        weapon.durability_current = weapon.durability_max;
    } else {
        weapon.durability_current = weapon
            .durability_current
            .saturating_add(5)
            .min(weapon.durability_max);
    }

    Some(ServerPacket::ItemRepaired {
        unique_id: equipment_slot_unique_id(weapon.slot)?,
        max_dura: weapon.durability_max,
        current_dura: weapon.durability_current,
    })
}

pub(super) enum CrystalLuckWeaponOutcome {
    Changed {
        refresh_packet: ServerPacket,
        message_key: &'static str,
        chat_type: ChatType,
    },
    NoEffect {
        message_key: &'static str,
    },
}

pub(super) fn try_luck_weapon(world: &mut World) -> Option<CrystalLuckWeaponOutcome> {
    let player_object_id = current_player_object_id(world).unwrap_or(0);
    let current_tick = super::session::runtime_tick(world);
    let mut resources = world.resource_mut::<InventoryResource>();
    let weapon = resources
        .equipment_items
        .iter_mut()
        .find(|item| item.slot == EquipmentSlot::Weapon)?;
    if weapon.added_luck >= 7 {
        return None;
    }

    if weapon.added_luck > -10 && deterministic_chance_roll(current_tick, player_object_id, 777, 20)
    {
        weapon.added_luck -= 1;
        return Some(CrystalLuckWeaponOutcome::Changed {
            refresh_packet: ServerPacket::RefreshItem {
                item: user_item_from_equipment_state(weapon)?,
            },
            message_key: "server.WeaponCurse",
            chat_type: ChatType::System,
        });
    }

    if weapon.added_luck <= 0
        || deterministic_chance_roll(
            current_tick,
            player_object_id,
            778,
            10 * u64::try_from(weapon.added_luck).ok()?,
        )
    {
        weapon.added_luck += 1;
        return Some(CrystalLuckWeaponOutcome::Changed {
            refresh_packet: ServerPacket::RefreshItem {
                item: user_item_from_equipment_state(weapon)?,
            },
            message_key: "server.WeaponLuck",
            chat_type: ChatType::Hint,
        });
    }

    Some(CrystalLuckWeaponOutcome::NoEffect {
        message_key: "server.WeaponNoEffect",
    })
}

pub(super) fn feed_mount_with_crystal_food(
    world: &mut World,
    template: &CrystalItemTemplate,
    item: &ItemState,
) -> Option<ServerPacket> {
    let mut resources = world.resource_mut::<InventoryResource>();
    let mount = resources
        .equipment_items
        .iter_mut()
        .find(|equipment| equipment.slot == EquipmentSlot::Mount)?;
    if mount.durability_current == mount.durability_max {
        return None;
    }

    if template.shape == 0 {
        let loss = 1000_u16.min(
            mount
                .durability_max
                .saturating_sub(mount.durability_current / 30),
        );
        mount.durability_max = mount.durability_max.saturating_sub(loss);
    }
    let feed_amount = item
        .durability_current
        .unwrap_or(template.durability)
        .max(1);
    mount.durability_current = mount
        .durability_current
        .saturating_add(feed_amount)
        .min(mount.durability_max);

    Some(ServerPacket::ItemRepaired {
        unique_id: equipment_slot_unique_id(EquipmentSlot::Mount).unwrap_or(13),
        max_dura: mount.durability_max,
        current_dura: mount.durability_current,
    })
}

pub(super) fn toggle_mount_ride_from_use_item(
    world: &mut World,
    packet_ack: Option<(u64, MirGridType)>,
) -> Option<Vec<ServerPacket>> {
    let Some((unique_id, MirGridType::Equipment)) = packet_ack else {
        return None;
    };
    if Some(unique_id) != equipment_slot_unique_id(EquipmentSlot::Mount) {
        return None;
    }
    let Some(mount) = world
        .resource::<InventoryResource>()
        .equipment_items
        .iter()
        .find(|equipment| equipment.slot == EquipmentSlot::Mount)
        .cloned()
    else {
        return Some(vec![ServerPacket::UseItem {
            unique_id,
            success: false,
            grid: MirGridType::Equipment,
        }]);
    };
    let mount_type = mount
        .shape
        .and_then(|shape| i16::try_from(shape).ok())
        .unwrap_or_else(|| i16::try_from(mount.icon).unwrap_or(0));
    let map_disallows_mount = current_map_disallows_mount(world);
    let map_requires_bridle = current_map_requires_bridle(world);
    let (riding_mount, mount_type) = {
        let mut mount_resource = world.resource_mut::<MountResource>();
        mount_resource.mount_type = mount_type;
        let wants_ride = !mount_resource.riding_mount;
        let can_ride = !wants_ride
            || (mount_resource.has_saddle
                && !map_disallows_mount
                && (!map_requires_bridle || mount_resource.has_reins));
        mount_resource.riding_mount = wants_ride && can_ride;
        (mount_resource.riding_mount, mount_resource.mount_type)
    };
    Some(vec![
        ServerPacket::UseItem {
            unique_id,
            success: true,
            grid: MirGridType::Equipment,
        },
        ServerPacket::MountUpdate {
            object_id: current_player_object_id(world).unwrap_or_default(),
            mount_type,
            riding_mount,
        },
    ])
}

/// Crystal `@RIDE` (`ToggleRide`): mount or dismount the equipped mount. A no-op
/// when no mount is equipped (Crystal's `ToggleRide` early-returns), and it honours
/// the same saddle / map-disallows / bridle gates as the use-item ride toggle.
pub(super) fn gm_toggle_ride(world: &mut World) -> Vec<ServerPacket> {
    let Some(mount) = world
        .resource::<InventoryResource>()
        .equipment_items
        .iter()
        .find(|equipment| equipment.slot == EquipmentSlot::Mount)
        .cloned()
    else {
        return Vec::new();
    };
    let mount_type = mount
        .shape
        .and_then(|shape| i16::try_from(shape).ok())
        .unwrap_or_else(|| i16::try_from(mount.icon).unwrap_or(0));
    let map_disallows_mount = current_map_disallows_mount(world);
    let map_requires_bridle = current_map_requires_bridle(world);
    let (riding_mount, mount_type) = {
        let mut mount_resource = world.resource_mut::<MountResource>();
        mount_resource.mount_type = mount_type;
        let wants_ride = !mount_resource.riding_mount;
        let can_ride = !wants_ride
            || (mount_resource.has_saddle
                && !map_disallows_mount
                && (!map_requires_bridle || mount_resource.has_reins));
        mount_resource.riding_mount = wants_ride && can_ride;
        (mount_resource.riding_mount, mount_resource.mount_type)
    };
    vec![ServerPacket::MountUpdate {
        object_id: current_player_object_id(world).unwrap_or_default(),
        mount_type,
        riding_mount,
    }]
}

pub(super) struct EquipItemMutationResult {
    pub(super) refresh_packets: Vec<ServerPacket>,
}

pub(super) fn try_equip_item(
    world: &mut World,
    grid: MirGridType,
    unique_id: u64,
    to: i32,
) -> Option<EquipItemMutationResult> {
    if !matches!(grid, MirGridType::Inventory | MirGridType::Storage) {
        return None;
    }
    if matches!(grid, MirGridType::Storage) && !active_crystal_storage_service(world) {
        return None;
    }
    if matches!(grid, MirGridType::Storage) && storage_locked(world) {
        return None;
    }

    let target_slot = equipment_slot_from_index(to)?;
    let player_character_index = current_character_index(world).unwrap_or(-1);

    let (
        source_index,
        source_slot,
        source_container,
        source_item,
        replaced_cursed,
        refresh_packets,
    ) = {
        let resources = world.resource::<InventoryResource>();
        let source_items = match grid {
            MirGridType::Inventory => &resources.inventory_items,
            MirGridType::Storage => &resources.storage_items,
            _ => unreachable!("unsupported grids return early"),
        };
        let index = unique_item_index_for_protocol_reference(source_items, grid, unique_id)?;
        let mut item = source_items[index].clone();
        validated_item_state_user_item(&item)?;
        if !item_state_can_equip_to_slot(&item, target_slot) {
            return None;
        }
        if crystal_item_template_for_dynamic_key(&item.key)
            .as_ref()
            .is_some_and(|template| {
                crystal_item_requirement_rejection_key(world, resources, template).is_some()
            })
        {
            return None;
        }
        let soul_bound_id = item_state_soul_bound_id(&item);
        if soul_bound_id != -1 && soul_bound_id != player_character_index {
            return None;
        }

        let replaced = resources
            .equipment_items
            .iter()
            .find(|item| item.slot == target_slot);
        if replaced.is_some_and(|item| user_item_from_equipment_state(item).is_none()) {
            return None;
        }
        if replaced.is_some_and(|item| {
            item.cursed && !world.resource::<PlayerPermissionResource>().unlock_curse
        }) {
            return None;
        }
        if matches!(grid, MirGridType::Storage)
            && replaced.is_some_and(|item| {
                equipment_has_crystal_or_rental_bind_flag(item, CRYSTAL_BIND_DONT_STORE)
            })
        {
            return None;
        }

        let mut refresh_packets = Vec::new();
        if crystal_item_needs_identify(&item.key) && !item_state_identified(&item) {
            item.identified = Some(true);
            refresh_packets.push(ServerPacket::RefreshItem {
                item: user_item_from_item_state(&item),
            });
        }
        if crystal_item_has_bind_flag(&item.key, CRYSTAL_BIND_ON_EQUIP)
            && item_state_soul_bound_id(&item) == -1
            && player_character_index >= 0
        {
            item.soul_bound_id = Some(player_character_index);
            refresh_packets.push(ServerPacket::RefreshItem {
                item: user_item_from_item_state(&item),
            });
        }

        (
            index,
            item.slot,
            item.container,
            item,
            replaced.is_some_and(|item| item.cursed),
            refresh_packets,
        )
    };

    {
        let mut resources = world.resource_mut::<InventoryResource>();
        let next_equipment = equipment_state_from_item_state(&source_item, target_slot);
        match grid {
            MirGridType::Inventory => {
                resources.inventory_items.remove(source_index);
            }
            MirGridType::Storage => {
                if !super::inventory::is_valid_storage_slot(&resources, source_slot) {
                    return None;
                }
                resources.storage_items.remove(source_index);
            }
            _ => unreachable!("unsupported grids return early"),
        }

        let replaced = resources
            .equipment_items
            .iter()
            .position(|item| item.slot == target_slot)
            .map(|index| resources.equipment_items.remove(index));
        resources.equipment_items.push(next_equipment);

        if let Some(existing) = replaced {
            let returned = item_state_from_equipment_state(existing, source_container, source_slot);
            match source_container {
                ItemContainer::Bag1 | ItemContainer::Bag2 => {
                    resources.inventory_items.push(returned)
                }
                ItemContainer::Storage => resources.storage_items.push(returned),
                _ => {}
            }
        }
    }
    if replaced_cursed {
        world
            .resource_mut::<PlayerPermissionResource>()
            .unlock_curse = false;
    }
    refresh_mount_resource_from_equipment(world);

    Some(EquipItemMutationResult { refresh_packets })
}

pub(super) fn equip_item_impl(
    world: &mut World,
    grid: MirGridType,
    unique_id: u64,
    to: i32,
) -> Vec<ServerPacket> {
    let failed_packet = ServerPacket::EquipItem {
        grid,
        unique_id,
        to,
        success: false,
    };
    if matches!(grid, MirGridType::HeroInventory) {
        return vec![failed_packet];
    }
    let Some(result) = try_equip_item(world, grid, unique_id, to) else {
        return vec![failed_packet];
    };

    let mut packets = result.refresh_packets;
    packets.push(ServerPacket::EquipItem {
        grid,
        unique_id,
        to,
        success: true,
    });
    refresh_mount_resource_from_equipment(world);
    super::stats::refresh_player_stats(world);
    packets
}

pub(super) fn remove_equipped_item_impl(
    world: &mut World,
    grid: MirGridType,
    unique_id: u64,
    to: i32,
) -> Vec<ServerPacket> {
    let failed_packet = ServerPacket::RemoveItem {
        grid,
        unique_id,
        to,
        success: false,
    };
    if matches!(grid, MirGridType::HeroInventory) {
        return vec![failed_packet];
    }
    if !matches!(grid, MirGridType::Inventory | MirGridType::Storage) {
        return vec![failed_packet];
    }
    if matches!(grid, MirGridType::Storage) && !active_crystal_storage_service(world) {
        return vec![failed_packet];
    }
    if matches!(grid, MirGridType::Storage) && storage_locked(world) {
        return vec![failed_packet];
    }

    let (destination_container, destination_slot) = {
        let resources = world.resource::<InventoryResource>();
        let Some(destination) = remove_item_destination(&resources, grid, to) else {
            return vec![failed_packet];
        };
        if collection_slot_occupied(&resources, destination.0, destination.1) {
            return vec![failed_packet];
        }
        let Some(index) = unique_equipment_index_for_protocol_reference(&resources, unique_id)
        else {
            return vec![failed_packet];
        };
        let equipment = &resources.equipment_items[index];
        let unequipped =
            item_state_from_equipment_state(equipment.clone(), destination.0, destination.1);
        if validated_item_state_user_item(&unequipped).is_none() {
            return vec![failed_packet];
        }
        if !crystal_item_removal_allowed(
            world,
            equipment.cursed,
            grid,
            equipment_has_crystal_or_rental_bind_flag(equipment, CRYSTAL_BIND_DONT_STORE),
        ) {
            return vec![failed_packet];
        }
        destination
    };

    {
        let mut resources = world.resource_mut::<InventoryResource>();
        let Some(index) = unique_equipment_index_for_protocol_reference(&resources, unique_id)
        else {
            return vec![failed_packet];
        };
        let equipment = resources.equipment_items.remove(index);
        let removed_cursed = equipment.cursed;
        let item =
            item_state_from_equipment_state(equipment, destination_container, destination_slot);
        match destination_container {
            ItemContainer::Bag1 | ItemContainer::Bag2 => resources.inventory_items.push(item),
            ItemContainer::Storage => resources.storage_items.push(item),
            _ => {}
        }
        drop(resources);
        consume_curse_unlock_after_success(world, removed_cursed);
    }

    refresh_mount_resource_from_equipment(world);
    super::stats::refresh_player_stats(world);
    vec![ServerPacket::RemoveItem {
        grid,
        unique_id,
        to,
        success: true,
    }]
}

fn crystal_item_removal_allowed(
    world: &World,
    cursed: bool,
    destination_grid: MirGridType,
    dont_store: bool,
) -> bool {
    (!cursed || world.resource::<PlayerPermissionResource>().unlock_curse)
        && !(destination_grid == MirGridType::Storage && dont_store)
}

fn consume_curse_unlock_after_success(world: &mut World, removed_cursed: bool) {
    if removed_cursed {
        world
            .resource_mut::<PlayerPermissionResource>()
            .unlock_curse = false;
    }
}

fn item_state_with_socket_inserted(
    host: &ItemState,
    socket_item: &ItemState,
    to: i32,
) -> Option<ItemState> {
    let target = usize::try_from(to).ok()?;
    let mut host_user_item = validated_item_state_user_item(host)?;
    if target >= host_user_item.slots.len() || host_user_item.slots[target].is_some() {
        return None;
    }
    host_user_item.slots[target] = Some(validated_item_state_user_item(socket_item)?);
    let next = try_item_state_from_user_item(host.clone(), &host_user_item).ok()?;
    validated_item_state_user_item(&next)?;
    Some(next)
}

fn equipment_state_with_socket_inserted(
    host: &EquipmentState,
    socket_item: &ItemState,
    to: i32,
) -> Option<EquipmentState> {
    let slot = host.slot;
    let carrier_slot = u8::try_from(equipment_slot_index(slot)?).ok()?;
    let carrier = item_state_from_equipment_state(host.clone(), ItemContainer::Bag1, carrier_slot);
    let next = item_state_with_socket_inserted(&carrier, socket_item, to)?;
    let next = equipment_state_from_item_state(&next, slot);
    user_item_from_equipment_state(&next)?;
    Some(next)
}

fn item_state_with_socket_removed(
    host: &ItemState,
    socket_unique_id: u64,
    destination_container: ItemContainer,
    destination_slot: u8,
) -> Option<(ItemState, ItemState)> {
    let mut host_user_item = validated_item_state_user_item(host)?;
    let mut matching_slots = host_user_item
        .slots
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            item.as_ref()
                .is_some_and(|item| item.unique_id == socket_unique_id)
                .then_some(index)
        });
    let socket_position = matching_slots.next()?;
    if matching_slots.next().is_some() {
        return None;
    }
    let removed_user_item = host_user_item.slots[socket_position].take()?;

    // Hydrate legacy sidecar-only saves before selecting the live child. This
    // keeps old legal saves removable without accepting malformed carriers.
    let hydrated_host =
        try_item_state_from_user_item(host.clone(), &validated_item_state_user_item(host)?).ok()?;
    let mut matching_children = hydrated_host.socketed.iter().filter(|item| {
        validated_item_state_user_item(item).is_some_and(|candidate| {
            candidate.unique_id == removed_user_item.unique_id
                && candidate.item_index == removed_user_item.item_index
        })
    });
    let mut removed = matching_children.next()?.clone();
    if matching_children.next().is_some() {
        return None;
    }
    removed.container = destination_container;
    removed.slot = destination_slot;
    let removed = try_item_state_from_user_item(removed, &removed_user_item).ok()?;
    validated_item_state_user_item(&removed)?;

    let next_host = try_item_state_from_user_item(hydrated_host, &host_user_item).ok()?;
    validated_item_state_user_item(&next_host)?;
    Some((next_host, removed))
}

fn equipment_state_with_socket_removed(
    host: &EquipmentState,
    socket_unique_id: u64,
    destination_container: ItemContainer,
    destination_slot: u8,
) -> Option<(EquipmentState, ItemState)> {
    let slot = host.slot;
    let carrier_slot = u8::try_from(equipment_slot_index(slot)?).ok()?;
    let carrier = item_state_from_equipment_state(host.clone(), ItemContainer::Bag1, carrier_slot);
    let (next, removed) = item_state_with_socket_removed(
        &carrier,
        socket_unique_id,
        destination_container,
        destination_slot,
    )?;
    let next = equipment_state_from_item_state(&next, slot);
    user_item_from_equipment_state(&next)?;
    Some((next, removed))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SocketHostReference {
    Inventory(usize),
    Equipment(usize),
}

fn unique_socket_host_reference(
    resources: &InventoryResource,
    unique_id: u64,
    inventory_only: bool,
) -> Option<SocketHostReference> {
    let inventory = unique_item_index_for_protocol_reference(
        &resources.inventory_items,
        MirGridType::Inventory,
        unique_id,
    )
    .map(SocketHostReference::Inventory);
    let equipment = (!inventory_only)
        .then(|| unique_equipment_index_for_protocol_reference(resources, unique_id))
        .flatten()
        .map(SocketHostReference::Equipment);
    match (inventory, equipment) {
        (Some(reference), None) | (None, Some(reference)) => Some(reference),
        _ => None,
    }
}

enum SocketHostReplacement {
    Inventory {
        unique_id: u64,
        item: ItemState,
    },
    Equipment {
        slot: EquipmentSlot,
        item: EquipmentState,
    },
}

fn apply_socket_host_replacement(
    resources: &mut InventoryResource,
    replacement: SocketHostReplacement,
) -> bool {
    match replacement {
        SocketHostReplacement::Inventory { unique_id, item } => {
            let Some(index) = unique_item_index_for_protocol_reference(
                &resources.inventory_items,
                MirGridType::Inventory,
                unique_id,
            ) else {
                return false;
            };
            resources.inventory_items[index] = item;
            true
        }
        SocketHostReplacement::Equipment { slot, item } => {
            let Some(index) = resources
                .equipment_items
                .iter()
                .position(|candidate| candidate.slot == slot)
            else {
                return false;
            };
            resources.equipment_items[index] = item;
            true
        }
    }
}

fn equip_mount_slot_item_impl(
    world: &mut World,
    grid: MirGridType,
    unique_id: u64,
    to: i32,
    to_unique_id: u64,
) -> bool {
    if !matches!(grid, MirGridType::Inventory | MirGridType::Storage)
        || to != 1
        || (grid == MirGridType::Storage
            && (!active_crystal_storage_service(world) || storage_locked(world)))
    {
        return false;
    }

    let Some((source_index, next_mount)) = (|| -> Option<(usize, EquipmentState)> {
        let resources = world.resource::<InventoryResource>();
        let source_items = match grid {
            MirGridType::Inventory => &resources.inventory_items,
            MirGridType::Storage => &resources.storage_items,
            _ => unreachable!("mount accessory source was validated"),
        };
        let source_index = unique_item_index_for_protocol_reference(source_items, grid, unique_id)?;
        let source = &source_items[source_index];
        let source_template = crystal_item_template_for_item_key(&source.key)?;
        if source_template.item_type != CRYSTAL_ITEM_TYPE_BELLS
            || crystal_item_requirement_rejection_key(world, resources, &source_template).is_some()
        {
            return None;
        }
        let soul_bound_id = item_state_soul_bound_id(source);
        if soul_bound_id != -1 && soul_bound_id != current_character_index(world).unwrap_or(-1) {
            return None;
        }
        let mount = resources
            .equipment_items
            .iter()
            .find(|item| item.slot == EquipmentSlot::Mount)?;
        let mount_user_item = user_item_from_equipment_state(mount)?;
        let legacy_mount_reference = equipment_slot_unique_id(EquipmentSlot::Mount);
        if mount_user_item.unique_id != to_unique_id && legacy_mount_reference != Some(to_unique_id)
        {
            return None;
        }
        if !crystal_item_template_for_item_key(&mount.key)
            .is_some_and(|template| template.item_type == CRYSTAL_ITEM_TYPE_MOUNT)
        {
            return None;
        }
        let next_mount = equipment_state_with_socket_inserted(mount, source, to)?;
        Some((source_index, next_mount))
    })() else {
        return false;
    };

    let mut next_resources = world.resource::<InventoryResource>().clone();
    let source_items = match grid {
        MirGridType::Inventory => &mut next_resources.inventory_items,
        MirGridType::Storage => &mut next_resources.storage_items,
        _ => unreachable!("mount accessory source was validated"),
    };
    if source_index >= source_items.len()
        || !item_state_matches_protocol_reference(&source_items[source_index], grid, unique_id)
    {
        return false;
    }
    source_items.remove(source_index);
    let Some(mount_index) = next_resources
        .equipment_items
        .iter()
        .position(|item| item.slot == EquipmentSlot::Mount)
    else {
        return false;
    };
    next_resources.equipment_items[mount_index] = next_mount;
    *world.resource_mut::<InventoryResource>() = next_resources;
    refresh_mount_resource_from_equipment(world);
    super::stats::refresh_player_stats(world);
    true
}

/// Insert a Crystal socket item into the exact protocol slot requested by the
/// client. `Socket` is the canonical Crystal target grid; `Inventory` remains
/// accepted for the earlier native-client envelope and only targets bag hosts.
pub(super) fn equip_slot_item_impl(
    world: &mut World,
    grid: MirGridType,
    unique_id: u64,
    to: i32,
    grid_to: MirGridType,
    to_unique_id: u64,
) -> Vec<ServerPacket> {
    let failed = ServerPacket::EquipSlotItem {
        grid,
        unique_id,
        to,
        grid_to,
        success: false,
    };
    if !super::resources::is_in_world(world) {
        return vec![failed];
    }
    if grid_to == MirGridType::Mount {
        if !equip_mount_slot_item_impl(world, grid, unique_id, to, to_unique_id) {
            return vec![failed];
        }
        return vec![ServerPacket::EquipSlotItem {
            grid,
            unique_id,
            to,
            grid_to,
            success: true,
        }];
    }
    if !matches!(grid, MirGridType::Inventory | MirGridType::Storage)
        || !matches!(grid_to, MirGridType::Socket | MirGridType::Inventory)
        || (grid == MirGridType::Storage
            && (!active_crystal_storage_service(world) || storage_locked(world)))
    {
        return vec![failed];
    }

    let inventory_only_host = grid_to == MirGridType::Inventory;
    let (source_index, host_replacement) = {
        let resources = world.resource::<InventoryResource>();
        let source_items = match grid {
            MirGridType::Inventory => &resources.inventory_items,
            MirGridType::Storage => &resources.storage_items,
            _ => unreachable!("socket source was validated"),
        };
        let Some(source_index) =
            unique_item_index_for_protocol_reference(source_items, grid, unique_id)
        else {
            return vec![failed];
        };
        let socket_item = &source_items[source_index];
        if !item_is_socket_type(socket_item) {
            return vec![failed];
        }
        let Some(host_reference) =
            unique_socket_host_reference(resources, to_unique_id, inventory_only_host)
        else {
            return vec![failed];
        };
        if matches!(host_reference, SocketHostReference::Inventory(index) if grid == MirGridType::Inventory && index == source_index)
        {
            return vec![failed];
        }
        let replacement = match host_reference {
            SocketHostReference::Inventory(index) => {
                let Some(next) = item_state_with_socket_inserted(
                    &resources.inventory_items[index],
                    socket_item,
                    to,
                ) else {
                    return vec![failed];
                };
                SocketHostReplacement::Inventory {
                    unique_id: to_unique_id,
                    item: next,
                }
            }
            SocketHostReference::Equipment(index) => {
                let host = &resources.equipment_items[index];
                let Some(next) = equipment_state_with_socket_inserted(host, socket_item, to) else {
                    return vec![failed];
                };
                SocketHostReplacement::Equipment {
                    slot: host.slot,
                    item: next,
                }
            }
        };
        (source_index, replacement)
    };

    let mut next_resources = world.resource::<InventoryResource>().clone();
    let source_items = match grid {
        MirGridType::Inventory => &mut next_resources.inventory_items,
        MirGridType::Storage => &mut next_resources.storage_items,
        _ => unreachable!("socket source was validated"),
    };
    if source_index >= source_items.len()
        || !item_state_matches_protocol_reference(&source_items[source_index], grid, unique_id)
    {
        return vec![failed];
    }
    source_items.remove(source_index);
    if !apply_socket_host_replacement(&mut next_resources, host_replacement) {
        return vec![failed];
    }
    *world.resource_mut::<InventoryResource>() = next_resources;
    super::stats::refresh_player_stats(world);
    vec![ServerPacket::EquipSlotItem {
        grid,
        unique_id,
        to,
        grid_to,
        success: true,
    }]
}

pub(super) fn remove_equipped_slot_item_impl(
    world: &mut World,
    grid: MirGridType,
    grid_to: MirGridType,
    unique_id: u64,
    to: i32,
    from_unique_id: u64,
) -> Vec<ServerPacket> {
    let failed_packet = ServerPacket::RemoveSlotItem {
        grid,
        grid_to,
        unique_id,
        to,
        success: false,
    };
    if !super::resources::is_in_world(world)
        || matches!(
            grid,
            MirGridType::HeroEquipment | MirGridType::HeroInventory
        )
        || matches!(
            grid_to,
            MirGridType::HeroEquipment | MirGridType::HeroInventory
        )
        || !matches!(grid_to, MirGridType::Inventory | MirGridType::Storage)
        || (grid_to == MirGridType::Storage
            && (!active_crystal_storage_service(world) || storage_locked(world)))
    {
        return vec![failed_packet];
    }

    let (destination_container, host_replacement, removed, removed_cursed) = {
        let resources = world.resource::<InventoryResource>();
        let Some(destination) = remove_item_destination(&resources, grid_to, to) else {
            return vec![failed_packet];
        };
        if collection_slot_occupied(&resources, destination.0, destination.1) {
            return vec![failed_packet];
        }

        let host_reference = match grid {
            MirGridType::Mount => {
                let Some(index) = resources
                    .equipment_items
                    .iter()
                    .position(|item| item.slot == EquipmentSlot::Mount)
                else {
                    return vec![failed_packet];
                };
                let host = &resources.equipment_items[index];
                let Some(host_user_item) = user_item_from_equipment_state(host) else {
                    return vec![failed_packet];
                };
                if host_user_item.unique_id != from_unique_id
                    && equipment_slot_unique_id(EquipmentSlot::Mount) != Some(from_unique_id)
                {
                    return vec![failed_packet];
                }
                SocketHostReference::Equipment(index)
            }
            MirGridType::Socket => {
                let Some(reference) =
                    unique_socket_host_reference(resources, from_unique_id, false)
                else {
                    return vec![failed_packet];
                };
                reference
            }
            // Fishing embedded slots remain a separate follow-on.
            _ => return vec![failed_packet],
        };

        let (replacement, removed) = match host_reference {
            SocketHostReference::Inventory(index) => {
                let Some((next, removed)) = item_state_with_socket_removed(
                    &resources.inventory_items[index],
                    unique_id,
                    destination.0,
                    destination.1,
                ) else {
                    return vec![failed_packet];
                };
                (
                    SocketHostReplacement::Inventory {
                        unique_id: from_unique_id,
                        item: next,
                    },
                    removed,
                )
            }
            SocketHostReference::Equipment(index) => {
                let host = &resources.equipment_items[index];
                let Some((next, removed)) = equipment_state_with_socket_removed(
                    host,
                    unique_id,
                    destination.0,
                    destination.1,
                ) else {
                    return vec![failed_packet];
                };
                (
                    SocketHostReplacement::Equipment {
                        slot: host.slot,
                        item: next,
                    },
                    removed,
                )
            }
        };
        let Some(removed_user_item) = validated_item_state_user_item(&removed) else {
            return vec![failed_packet];
        };
        if removed_user_item.wedding_ring != -1
            || !crystal_item_removal_allowed(
                world,
                removed.cursed,
                grid_to,
                item_has_crystal_or_rental_bind_flag(&removed, CRYSTAL_BIND_DONT_STORE),
            )
        {
            return vec![failed_packet];
        }
        let removed_cursed = removed.cursed;
        (destination.0, replacement, removed, removed_cursed)
    };

    let mut next_resources = world.resource::<InventoryResource>().clone();
    if !apply_socket_host_replacement(&mut next_resources, host_replacement) {
        return vec![failed_packet];
    }
    match destination_container {
        ItemContainer::Bag1 | ItemContainer::Bag2 => next_resources.inventory_items.push(removed),
        ItemContainer::Storage => next_resources.storage_items.push(removed),
        _ => return vec![failed_packet],
    }
    *world.resource_mut::<InventoryResource>() = next_resources;
    consume_curse_unlock_after_success(world, removed_cursed);
    refresh_mount_resource_from_equipment(world);
    super::stats::refresh_player_stats(world);
    vec![ServerPacket::RemoveSlotItem {
        grid,
        grid_to,
        unique_id,
        to,
        success: true,
    }]
}

pub(super) fn active_crystal_repair_service(
    service: &ActiveNpcServiceState,
    special: bool,
) -> bool {
    matches!(
        (special, service.label_key.as_str()),
        (false, "REPAIR") | (true, "SREPAIR")
    )
}

pub(super) fn repair_item_impl(
    world: &mut World,
    unique_id: u64,
    special: bool,
) -> Vec<ServerPacket> {
    if current_player_is_dead(world) {
        return Vec::new();
    }

    let Some(service) = current_crystal_npc_service_in_range(world)
        .filter(|service| active_crystal_repair_service(service, special))
    else {
        return Vec::new();
    };

    let Some(script) = crystal_npc_script_by_key(&service.script_key) else {
        return Vec::new();
    };

    let repair_types = crystal_npc_script_item_types(&script);
    let rate = crystal_npc_info_by_script_key(&service.script_key)
        .map(|npc| npc.price_rate)
        .unwrap_or(1.0);

    let equipment_index = {
        let resources = world.resource::<InventoryResource>();
        resources
            .equipment_items
            .iter()
            .position(|item| equipment_slot_unique_id(item.slot) == Some(unique_id))
    };
    if let Some(equipment_index) = equipment_index {
        let equipment = {
            let resources = world.resource::<InventoryResource>();
            resources.equipment_items[equipment_index].clone()
        };
        let item = item_state_from_equipment_state(
            equipment.clone(),
            ItemContainer::Bag1,
            u8::try_from(unique_id).unwrap_or_default(),
        );
        let Some(template) = crystal_item_template_for_item_key(&item.key) else {
            return Vec::new();
        };

        if equipment_has_crystal_or_rental_bind_flag(&equipment, CRYSTAL_BIND_DONT_REPAIR)
            || (special
                && equipment_has_crystal_or_rental_bind_flag(&equipment, CRYSTAL_BIND_NO_SREPAIR))
        {
            return vec![super::session::system_message_key(
                world,
                "server.CannotRepairItem",
            )];
        }

        if !repair_types.is_empty() && !repair_types.contains(&template.item_type) {
            return vec![super::session::system_message_key(
                world,
                "server.CannotRepairItemHere",
            )];
        }

        let cost = crystal_npc_repair_cost(&item, &template, rate, special);
        {
            if world.resource::<PlayerRuntimeResource>().gold < cost {
                return Vec::new();
            }
        }

        let (max_dura, current_dura) = {
            world.resource_mut::<PlayerRuntimeResource>().gold -= cost;
            let mut resources = world.resource_mut::<InventoryResource>();
            let item = &mut resources.equipment_items[equipment_index];
            let current = item.durability_current;
            let mut max = item.durability_max;
            if !special {
                let loss = max.saturating_sub(current) / 30;
                max = max.saturating_sub(loss);
                item.durability_max = max;
            }
            item.durability_current = max;
            (max, max)
        };

        return vec![
            ServerPacket::LoseGold { gold: cost },
            ServerPacket::ItemRepaired {
                unique_id,
                max_dura,
                current_dura,
            },
        ];
    }

    let item_index = {
        let resources = world.resource::<InventoryResource>();
        resources
            .inventory_items
            .iter()
            .position(|item| item_matches_inventory_unique_id(item, unique_id))
    };
    let Some(item_index) = item_index else {
        return Vec::new();
    };

    let item = {
        let resources = world.resource::<InventoryResource>();
        resources.inventory_items[item_index].clone()
    };
    let Some(template) = crystal_item_template_for_item_key(&item.key) else {
        return Vec::new();
    };

    if crystal_item_has_bind_flag(&item.key, CRYSTAL_BIND_DONT_REPAIR)
        || (special && crystal_item_has_bind_flag(&item.key, CRYSTAL_BIND_NO_SREPAIR))
    {
        return vec![super::session::system_message_key(
            world,
            "server.CannotRepairItem",
        )];
    }

    if !repair_types.is_empty() && !repair_types.contains(&template.item_type) {
        return vec![super::session::system_message_key(
            world,
            "server.CannotRepairItemHere",
        )];
    }

    let cost = crystal_npc_repair_cost(&item, &template, rate, special);
    {
        if world.resource::<PlayerRuntimeResource>().gold < cost {
            return Vec::new();
        }
    }

    let (max_dura, current_dura) = {
        world.resource_mut::<PlayerRuntimeResource>().gold -= cost;
        let mut resources = world.resource_mut::<InventoryResource>();
        let item = &mut resources.inventory_items[item_index];
        let current = item.durability_current.unwrap_or(0);
        let mut max = item.durability_max.unwrap_or(0);
        if !special {
            let loss = max.saturating_sub(current) / 30;
            max = max.saturating_sub(loss);
            item.durability_max = Some(max);
        }
        item.durability_current = Some(max);
        (max, max)
    };

    vec![
        ServerPacket::LoseGold { gold: cost },
        ServerPacket::ItemRepaired {
            unique_id,
            max_dura,
            current_dura,
        },
    ]
}

pub(super) fn crystal_npc_repair_cost(
    item: &ItemState,
    template: &CrystalItemTemplate,
    rate: f32,
    special: bool,
) -> u32 {
    let repair_price = crystal_item_repair_price(item, template);
    let multiplier = if special { 3.0 } else { 1.0 };
    ((repair_price as f32) * multiplier * rate).floor() as u32
}

pub(super) fn crystal_item_repair_price(item: &ItemState, template: &CrystalItemTemplate) -> u32 {
    if template.durability == 0 {
        return 0;
    }

    let added_stat_count = merged_user_item_stats(
        &item.added_stats,
        item.added_defence,
        item.added_attack,
        None,
    )
    .len();
    let count = item.quantity.max(1);
    let price_when_full = crystal_item_full_durability_price(item, template, added_stat_count);
    let current_price = crystal_item_current_price(item, template, added_stat_count);
    price_when_full
        .saturating_mul(count)
        .saturating_sub(current_price)
}

pub(super) fn crystal_item_full_durability_price(
    item: &ItemState,
    template: &CrystalItemTemplate,
    added_stat_count: usize,
) -> u32 {
    let max_dura = item.durability_max.unwrap_or(0);
    let base = (f32::from(max_dura)
        * ((template.price as f32 / 2.0) / f32::from(template.durability))
        + (template.price as f32 / 2.0))
        .floor();
    crystal_apply_added_stat_price_factor(base, added_stat_count)
}

pub(super) fn crystal_item_current_price(
    item: &ItemState,
    template: &CrystalItemTemplate,
    added_stat_count: usize,
) -> u32 {
    let mut price = template.price as f32;
    if template.durability > 0 {
        let max_dura = item.durability_max.unwrap_or(0);
        let current_dura = item.durability_current.unwrap_or(0);
        let per_dura = (template.price as f32 / 2.0) / f32::from(template.durability);
        let max_value = (f32::from(max_dura) * per_dura).trunc();
        let durability_ratio = if max_dura > 0 {
            f32::from(current_dura) / f32::from(max_dura)
        } else {
            0.0
        };
        price = (max_value / 2.0
            + ((max_value / 2.0) * durability_ratio)
            + (template.price as f32 / 2.0))
            .floor();
    }
    crystal_apply_added_stat_price_factor(price, added_stat_count)
        .saturating_mul(item.quantity.max(1))
}

#[cfg(test)]
mod native_start_equipment_tests {
    use super::*;
    use crate::config::SimulationConfig;
    use crate::runtime::inventory::{item_index_for_client_reference, move_item_impl};
    use crate::runtime::resources::SessionResource;
    use crate::runtime::session::SimulationSession;

    fn identity_metadata(item_index: i32) -> ItemStateUserItemMetadata {
        ItemStateUserItemMetadata {
            item_index: Some(item_index),
            awake_type: 2,
            awake_values: vec![4, 5],
            refined_value: 6,
            refine_added: 7,
            refine_success_chance: 88,
            wedding_ring: 23,
            expire_info: Some(mir2_protocol::UserItemExpireInfo {
                expiry_binary_datetime: 123_456,
            }),
            rental_information: None,
            sealed_info: None,
            slots: Vec::new(),
            is_shop_item: true,
            gm_made: true,
            live_socketed_at_capture: false,
            socket_layout_hydrated: false,
            captured_socket_positions: None,
            captured_socket_position: None,
        }
    }

    fn inventory_item_with_identity(unique_id: u64, item_index: i32) -> ItemState {
        let mut item = item_state_from_equipment_state(
            seed_equipment_items()[0].clone(),
            ItemContainer::Bag1,
            7,
        );
        item.unique_id = unique_id;
        item.user_item_metadata = Some(identity_metadata(item_index));
        item
    }

    #[test]
    fn every_character_start_equipment_item_has_native_item_info() {
        let classes = [
            MirClass::Warrior,
            MirClass::Wizard,
            MirClass::Taoist,
            MirClass::Assassin,
            MirClass::Archer,
        ];
        let genders = [MirGender::Male, MirGender::Female];

        for class in classes {
            for gender in genders {
                let equipment = seed_equipment_items_for_character(class, gender);
                assert_eq!(equipment.len(), 2);
                for item in &equipment {
                    let template = crystal_item_template_for_item_key(&item.key)
                        .expect("starter equipment must resolve to Crystal ItemInfo");
                    let user_item = user_item_from_equipment_state(item)
                        .expect("starter equipment must serialize for Crystal");
                    assert_eq!(user_item.item_index, template.item_index);
                    assert_eq!(
                        item.snapshot(LanguageCode::English).state_image,
                        template.image,
                        "CharacterDialog must receive Crystal ItemInfo.Image"
                    );
                    let snapshot = item.snapshot(LanguageCode::English);
                    let tooltip = snapshot
                        .tooltip_source
                        .expect("starter equipment must expose Crystal tooltip source");
                    assert_eq!(tooltip.info.item_index, template.item_index);
                    assert_eq!(tooltip.user_item, Some(user_item));
                }
            }
        }
    }

    #[test]
    fn unresolved_equipment_never_emits_a_dangling_native_user_item() {
        let mut unresolved_key = seed_equipment_items()[0].clone();
        unresolved_key.key = "web-only-equipment-without-crystal-item-info".to_string();
        assert!(user_item_from_equipment_state(&unresolved_key).is_none());

        let mut unresolved_index = seed_equipment_items()[0].clone();
        unresolved_index.user_item_metadata = Some(identity_metadata(i32::MIN));
        assert!(user_item_from_equipment_state(&unresolved_index).is_none());
    }

    #[test]
    fn exact_and_zero_uid_survive_equip_serde_reload_unequip() {
        for exact_uid in [9_001, 0] {
            let inventory_item = inventory_item_with_identity(exact_uid, 221);
            let equipped = equipment_state_from_item_state(&inventory_item, EquipmentSlot::Weapon);
            assert_eq!(equipped.user_item_unique_id, Some(exact_uid));

            let reloaded: EquipmentState = serde_json::from_str(
                &serde_json::to_string(&equipped).expect("equipment state should encode"),
            )
            .expect("equipment state should reload");
            let unequipped =
                item_state_from_equipment_state(reloaded.clone(), ItemContainer::Bag1, 8);

            assert_eq!(reloaded.user_item_unique_id, Some(exact_uid));
            assert_eq!(
                reloaded.snapshot(LanguageCode::English).unique_id,
                Some(exact_uid),
                "the world snapshot must retain the worn Crystal instance UID"
            );
            assert_eq!(
                user_item_from_equipment_state(&reloaded)
                    .expect("equipped exact UID carrier should serialize")
                    .unique_id,
                exact_uid
            );
            assert_eq!(unequipped.unique_id, exact_uid);
            assert_eq!(
                try_user_item_from_item_state(&unequipped)
                    .expect("exact UID carrier should serialize")
                    .unique_id,
                exact_uid
            );
        }

        let exact = equipment_state_from_item_state(
            &inventory_item_with_identity(0, 221),
            EquipmentSlot::Weapon,
        );
        let mut old_json = serde_json::to_value(exact).expect("equipment state should encode");
        old_json
            .as_object_mut()
            .expect("equipment state JSON should be an object")
            .remove("user_item_unique_id");
        let legacy: EquipmentState =
            serde_json::from_value(old_json).expect("legacy equipment save should reload");
        assert_eq!(legacy.user_item_unique_id, None);
        assert_eq!(
            item_state_from_equipment_state(legacy, ItemContainer::Bag1, 8).unique_id,
            8
        );
    }

    #[test]
    fn shared_serializer_preserves_exact_fields_and_rejects_large_quantity() {
        let item_index = crystal_item_template_for_item_key(&seed_equipment_items()[0].key)
            .expect("starter projection template")
            .item_index;
        let mut inventory_item = inventory_item_with_identity(9_130, item_index);
        let default_rental = mir2_protocol::UserItemRentalInformation {
            owner_name: String::new(),
            binding_flags: 0,
            expiry_binary_datetime: 0,
            rental_locked: false,
        };
        let default_sealed = mir2_protocol::UserItemSealedInfo {
            expiry_binary_datetime: 0,
            next_seal_binary_datetime: 0,
        };
        let metadata = inventory_item
            .user_item_metadata
            .as_mut()
            .expect("identity fixture has metadata");
        metadata.rental_information = Some(default_rental.clone());
        metadata.sealed_info = Some(default_sealed.clone());

        let equipped = equipment_state_from_item_state(&inventory_item, EquipmentSlot::Weapon);
        let mut equipped: EquipmentState = serde_json::from_str(
            &serde_json::to_string(&equipped).expect("equipment state should encode"),
        )
        .expect("equipment state should reload");
        let wire = user_item_from_equipment_state(&equipped).unwrap_or_else(|| {
            let carrier = item_state_from_equipment_state(equipped.clone(), ItemContainer::Bag1, 0);
            panic!(
                "valid exact equipment carrier should serialize: {:?}",
                try_user_item_from_item_state(&carrier)
            );
        });
        assert_eq!(wire.item_index, item_index);
        assert_eq!(wire.rental_information, Some(default_rental));
        assert_eq!(wire.sealed_info, Some(default_sealed));

        equipped.quantity = u32::from(u16::MAX) + 1;
        assert!(user_item_from_equipment_state(&equipped).is_none());
    }

    #[test]
    fn unequip_live_awake_overrides_stale_sidecar_awake() {
        let inventory_item = inventory_item_with_identity(9_140, 221);
        let mut equipped = equipment_state_from_item_state(&inventory_item, EquipmentSlot::Weapon);
        assert_eq!(equipped.awake_type, 2);
        assert_eq!(equipped.awake_values, vec![4, 5]);

        equipped.awake_type = 9;
        equipped.awake_values = vec![3, 3, 3];
        let unequipped = item_state_from_equipment_state(equipped, ItemContainer::Bag1, 8);
        let metadata = unequipped
            .user_item_metadata
            .as_ref()
            .expect("unequipped item retains metadata carrier");
        assert_eq!(metadata.awake_type, 9);
        assert_eq!(metadata.awake_values, vec![3, 3, 3]);
        let wire = try_user_item_from_item_state(&unequipped)
            .expect("live Awake carrier should serialize");
        assert_eq!(wire.awake_type, 9);
        assert_eq!(wire.awake_values, vec![3, 3, 3]);
    }

    #[test]
    fn ordinary_socket_and_mount_bells_removal_do_not_revive_captured_slots() {
        let stale_socket = try_user_item_from_item_state(&inventory_item_with_identity(9_151, 221))
            .expect("socket fixture should serialize");
        let mut ordinary = inventory_item_with_identity(9_150, 221);
        ordinary.socket_slots = 2;
        ordinary.socketed = vec![inventory_item_with_identity(9_151, 221)];
        let ordinary_metadata = ordinary
            .user_item_metadata
            .as_mut()
            .expect("ordinary host has metadata");
        ordinary_metadata.slots = vec![Some(stale_socket), None];
        ordinary_metadata.live_socketed_at_capture = true;
        let mut ordinary_equipment =
            equipment_state_from_item_state(&ordinary, EquipmentSlot::Weapon);
        assert_eq!(
            user_item_from_equipment_state(&ordinary_equipment)
                .expect("ordinary socket carrier should serialize")
                .slots[0]
                .as_ref()
                .map(|item| item.unique_id),
            Some(9_151)
        );
        ordinary_equipment.socketed.clear();
        assert_eq!(
            user_item_from_equipment_state(&ordinary_equipment)
                .expect("ordinary socket removal should serialize")
                .slots,
            vec![None, None]
        );

        let mount_template =
            mir2_game_data::crystal_item_by_name("RedTiger").expect("RedTiger template");
        let bells_template =
            mir2_game_data::crystal_item_by_name("BronzeBell").expect("BronzeBell template");
        let mut bells = inventory_item_with_identity(9_161, bells_template.item_index);
        bells.key = format!("crystal-item-{}", bells_template.item_index);
        bells.icon = bells_template.image;
        let stale_bells = try_user_item_from_item_state(&bells)
            .expect("Bells fixture should serialize through shared carrier");

        let mut mount = inventory_item_with_identity(9_160, mount_template.item_index);
        mount.key = format!("crystal-item-{}", mount_template.item_index);
        mount.icon = mount_template.image;
        mount.socket_slots = 2;
        mount.socketed = vec![bells];
        let mount_metadata = mount
            .user_item_metadata
            .as_mut()
            .expect("mount host has metadata");
        mount_metadata.slots = vec![None, Some(stale_bells)];
        mount_metadata.live_socketed_at_capture = true;
        let mut mount_equipment = equipment_state_from_item_state(&mount, EquipmentSlot::Mount);
        assert_eq!(
            user_item_from_equipment_state(&mount_equipment)
                .expect("mount Bells carrier should serialize")
                .slots[1]
                .as_ref()
                .map(|item| item.unique_id),
            Some(9_161)
        );
        mount_equipment.socketed.clear();
        assert_eq!(
            user_item_from_equipment_state(&mount_equipment)
                .expect("mount Bells removal should serialize")
                .slots,
            vec![None, None]
        );
    }
    fn mutation_session() -> SimulationSession {
        let config = SimulationConfig::default();
        let selected_character = config.default_character.clone();
        let mut session = SimulationSession::new(config);
        session
            .app
            .world_mut()
            .resource_mut::<SessionResource>()
            .selected_character = Some(selected_character);
        let mut resources = session.app.world_mut().resource_mut::<InventoryResource>();
        resources.inventory_items.clear();
        resources.storage_items.clear();
        resources.equipment_items.clear();
        drop(resources);
        session
    }

    #[test]
    fn actual_equip_reload_remove_preserves_exact_root_uid_including_zero() {
        for exact_uid in [9_201, 0] {
            let mut session = mutation_session();
            session
                .app
                .world_mut()
                .resource_mut::<InventoryResource>()
                .inventory_items
                .push(inventory_item_with_identity(exact_uid, 221));

            let weapon_slot = i32::try_from(
                equipment_slot_index(EquipmentSlot::Weapon).expect("weapon slot index"),
            )
            .expect("weapon slot fits i32");
            assert!(try_equip_item(
                session.app.world_mut(),
                MirGridType::Inventory,
                exact_uid,
                weapon_slot,
            )
            .is_some());
            {
                let resources = session.app.world().resource::<InventoryResource>();
                assert_eq!(resources.equipment_items.len(), 1);
                assert_eq!(
                    user_item_from_equipment_state(&resources.equipment_items[0])
                        .expect("equipped item remains a valid carrier")
                        .unique_id,
                    exact_uid
                );
            }

            let encoded = {
                let resources = session.app.world().resource::<InventoryResource>();
                serde_json::to_string(&resources.equipment_items)
                    .expect("equipment save should encode")
            };
            session
                .app
                .world_mut()
                .resource_mut::<InventoryResource>()
                .equipment_items =
                serde_json::from_str(&encoded).expect("equipment save should reload");

            let packets = remove_equipped_item_impl(
                session.app.world_mut(),
                MirGridType::Inventory,
                exact_uid,
                8,
            );
            assert!(packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::RemoveItem {
                    unique_id,
                    success: true,
                    ..
                } if *unique_id == exact_uid
            )));
            {
                let resources = session.app.world().resource::<InventoryResource>();
                assert!(resources.equipment_items.is_empty());
                let returned = resources
                    .inventory_items
                    .iter()
                    .find(|item| {
                        validated_item_state_user_item(item)
                            .is_some_and(|item| item.unique_id == exact_uid)
                    })
                    .expect("unequipped item returns with its exact root UID");
                assert_eq!(returned.slot, 8);
                assert_eq!(
                    validated_item_state_user_item(returned)
                        .expect("returned item remains valid")
                        .unique_id,
                    exact_uid
                );
            }

            if exact_uid == 0 {
                let packets = move_item_impl(session.app.world_mut(), MirGridType::Inventory, 8, 9);
                assert!(packets.iter().any(|packet| matches!(
                    packet,
                    ServerPacket::MoveItem {
                        grid: MirGridType::Inventory,
                        from: 8,
                        to: 9,
                        success: true,
                    }
                )));

                let resources = session.app.world().resource::<InventoryResource>();
                let moved_index = item_index_for_client_reference(
                    &resources.inventory_items,
                    MirGridType::Inventory,
                    0,
                )
                .expect("moved exact-zero item remains addressable by protocol UID 0");
                let moved = &resources.inventory_items[moved_index];
                assert_eq!(moved.slot, 9);
                assert_eq!(moved.unique_id, 0);
                assert!(item_matches_inventory_unique_id(moved, 0));
                assert!(!item_matches_inventory_unique_id(moved, 9));
            }
        }
    }

    #[test]
    fn actual_equip_swap_returns_replaced_item_without_reallocating_uid() {
        let mut session = mutation_session();
        let incoming = inventory_item_with_identity(9_211, 221);
        let worn = equipment_state_from_item_state(
            &inventory_item_with_identity(9_210, 221),
            EquipmentSlot::Weapon,
        );
        {
            let mut resources = session.app.world_mut().resource_mut::<InventoryResource>();
            resources.inventory_items.push(incoming);
            resources.equipment_items.push(worn);
        }

        assert!(try_equip_item(
            session.app.world_mut(),
            MirGridType::Inventory,
            9_211,
            i32::try_from(equipment_slot_index(EquipmentSlot::Weapon).unwrap()).unwrap(),
        )
        .is_some());
        let resources = session.app.world().resource::<InventoryResource>();
        assert_eq!(
            user_item_from_equipment_state(&resources.equipment_items[0])
                .expect("incoming item is equipped")
                .unique_id,
            9_211
        );
        let returned = resources
            .inventory_items
            .iter()
            .find(|item| {
                validated_item_state_user_item(item).is_some_and(|item| item.unique_id == 9_210)
            })
            .expect("replaced item returns to the source slot");
        assert_eq!(returned.slot, 7);
    }

    #[test]
    fn socket_mutation_uses_exact_target_and_restores_destination_metadata() {
        let mut host = inventory_item_with_identity(9_220, 221);
        host.socket_slots = 2;
        let bells_template =
            mir2_game_data::crystal_item_by_name("BronzeBell").expect("real Bells template");
        let mut socket_item = inventory_item_with_identity(0, bells_template.item_index);
        socket_item.key = format!("crystal-item-{}", bells_template.item_index);
        socket_item.name = bells_template.name.clone();
        socket_item.icon = bells_template.image;

        assert!(item_state_with_socket_inserted(&host, &socket_item, -1).is_none());
        assert!(item_state_with_socket_inserted(&host, &socket_item, 2).is_none());
        let inserted = item_state_with_socket_inserted(&host, &socket_item, 1)
            .expect("exact free socket accepts the item");
        let inserted_wire = validated_item_state_user_item(&inserted)
            .expect("inserted host remains a valid carrier");
        assert!(inserted_wire.slots[0].is_none());
        assert_eq!(
            inserted_wire.slots[1].as_ref().map(|item| item.unique_id),
            Some(0)
        );
        let host_metadata = inserted
            .user_item_metadata
            .as_ref()
            .expect("host layout is hydrated");
        assert!(host_metadata.socket_layout_hydrated);
        assert_eq!(
            host_metadata
                .captured_socket_positions
                .as_ref()
                .expect("captured positions exist")
                .len(),
            2
        );
        assert_eq!(
            inserted.socketed[0]
                .user_item_metadata
                .as_ref()
                .and_then(|metadata| metadata.captured_socket_position),
            Some(1)
        );

        let mut second = inventory_item_with_identity(9_221, bells_template.item_index);
        second.key = format!("crystal-item-{}", bells_template.item_index);
        assert!(item_state_with_socket_inserted(&inserted, &second, 1).is_none());

        let (cleared, returned) =
            item_state_with_socket_removed(&inserted, 0, ItemContainer::Bag2, 9)
                .expect("exact socket item can be removed");
        assert_eq!(
            validated_item_state_user_item(&cleared)
                .expect("cleared host remains valid")
                .slots,
            vec![None, None]
        );
        assert_eq!(returned.container, ItemContainer::Bag2);
        assert_eq!(returned.slot, 9);
        assert_eq!(
            returned
                .user_item_metadata
                .as_ref()
                .and_then(|metadata| metadata.captured_socket_position),
            None
        );
        assert_eq!(
            validated_item_state_user_item(&returned)
                .expect("returned socket item remains valid")
                .unique_id,
            0
        );
    }

    #[test]
    fn actual_equip_rejects_invalid_root_carrier_without_mutation() {
        for invalid_item_index in [i32::MIN, 658] {
            let mut session = mutation_session();
            let invalid = inventory_item_with_identity(9_230, invalid_item_index);
            session
                .app
                .world_mut()
                .resource_mut::<InventoryResource>()
                .inventory_items
                .push(invalid.clone());

            assert!(try_equip_item(
                session.app.world_mut(),
                MirGridType::Inventory,
                9_230,
                i32::try_from(equipment_slot_index(EquipmentSlot::Weapon).unwrap()).unwrap(),
            )
            .is_none());
            let resources = session.app.world().resource::<InventoryResource>();
            assert!(resources.equipment_items.is_empty());
            assert_eq!(resources.inventory_items.len(), 1);
            assert_eq!(resources.inventory_items[0].unique_id, invalid.unique_id);
            assert_eq!(
                resources.inventory_items[0]
                    .user_item_metadata
                    .as_ref()
                    .and_then(|metadata| metadata.item_index),
                Some(invalid_item_index)
            );
        }
    }
}

pub(super) fn crystal_apply_added_stat_price_factor(price: f32, added_stat_count: usize) -> u32 {
    (price * (1.0 + (added_stat_count as f32 * 0.1))).trunc() as u32
}
