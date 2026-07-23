use serde::{Deserialize, Serialize};

use crate::config::{EquipmentItemSnapshot, EquipmentSlot, ItemContainer, ItemGrade};
use bevy_ecs::prelude::World;
use mir2_game_data::{
    crystal_npc_info_by_script_key, crystal_npc_script_by_key, localized_text_or_fallback,
    CrystalItemTemplate, EquipmentTemplate, LanguageCode,
};
use mir2_protocol::{
    ChatType, MirClass, MirGender, MirGridType, ServerPacket, UserItem, UserItemSealedInfo,
    UserItemStat,
};

use super::buffs::{buff_attack_bonus, buff_defence_bonus};
use super::combat::deterministic_chance_roll;
use super::components::{
    current_character_index, current_player_is_dead, current_player_object_id,
};
use super::crystal_compat::{
    CRYSTAL_BIND_DONT_REPAIR, CRYSTAL_BIND_DONT_STORE, CRYSTAL_BIND_NO_SREPAIR,
    CRYSTAL_BIND_ON_EQUIP, CRYSTAL_STAT_MAX_AC, CRYSTAL_STAT_MAX_DC,
};
use super::inventory::{
    collection_slot_occupied, equipment_index_for_client_reference,
    item_index_for_client_reference, item_matches_inventory_unique_id, remove_item_destination,
    storage_locked,
};
use super::items::{
    crystal_default_identified_for_item_key, crystal_item_has_bind_flag,
    crystal_item_needs_identify, crystal_item_requirement_rejection_key, crystal_item_stat_value,
    crystal_item_template_for_dynamic_key, crystal_item_template_for_item_key,
    default_item_unique_id, equipment_has_crystal_or_rental_bind_flag, item_is_socket_type,
    item_state_can_equip_to_slot, item_state_identified, item_state_soul_bound_id,
    merged_user_item_stats, upsert_user_item_stat, user_item_from_item_state,
    user_item_rental_information, ItemState,
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

impl EquipmentState {
    pub(super) fn snapshot(&self, language: LanguageCode) -> EquipmentItemSnapshot {
        EquipmentItemSnapshot {
            slot: self.slot,
            name: localized_equipment_name(language, &self.key, &self.name),
            icon: self.icon,
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
        "guide-ring-right" => Some("content.equipment.guideRing"),
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
        (EquipmentSlot::RingRight, "Guide Ring") => "guide-ring-right".to_string(),
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
        (EquipmentSlot::RingLeft | EquipmentSlot::RingRight, "Guide Ring") => 149,
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
    let template = crystal_item_template_for_item_key(&item.key)?;
    let added_stats = merged_user_item_stats(
        &item.added_stats,
        item.added_defence,
        item.added_attack,
        Some(item.added_luck),
    );

    Some(UserItem {
        unique_id: equipment_slot_unique_id(item.slot)?,
        item_index: template.item_index,
        current_dura: item.durability_current,
        max_dura: item.durability_max,
        count: 1,
        soul_bound_id: equipment_state_soul_bound_id(item),
        identified: equipment_state_identified(item),
        cursed: item.cursed,
        slots: vec![None; usize::from(item.socket_slots)],
        gem_count: item.gem_count,
        added_stats,
        awake_type: item.awake_type,
        awake_values: item.awake_values.clone(),
        refined_value: 0,
        refine_added: 0,
        refine_success_chance: 0,
        wedding_ring: -1,
        expire_info: None,
        rental_information: user_item_rental_information(
            item.rental_binding_flags,
            &item.rental_owner_name,
            item.rental_expiry_binary_datetime,
            item.rental_locked,
        ),
        is_shop_item: false,
        sealed_info: (item.sealed_expiry_time_binary_datetime != 0).then_some(UserItemSealedInfo {
            expiry_binary_datetime: item.sealed_expiry_time_binary_datetime,
            next_seal_binary_datetime: item.sealed_next_time_binary_datetime,
        }),
        gm_made: false,
    })
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
    super::stats::refresh_player_stats(world);
}

pub(super) fn item_state_from_equipment_state(
    equipment: EquipmentState,
    container: ItemContainer,
    slot: u8,
) -> ItemState {
    let mut added_stats = equipment.added_stats;
    upsert_user_item_stat(&mut added_stats, 15, equipment.added_luck);

    ItemState {
        key: equipment.key,
        name: equipment.name,
        icon: equipment.icon,
        slot,
        unique_id: default_item_unique_id(container, slot),
        container,
        quantity: 1,
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
    EquipmentState {
        key: item.key.clone(),
        slot,
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
        awake_type: 0,
        awake_values: Vec::new(),
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

pub(super) fn repair_equipped_durability(world: &mut World) -> Vec<ServerPacket> {
    let mut packets = Vec::new();
    let mut resources = world.resource_mut::<InventoryResource>();

    for item in resources.equipment_items.iter_mut() {
        if !equipment_uses_durability(item) || item.durability_current >= item.durability_max {
            continue;
        }

        item.durability_current = item.durability_max;
        if let Some(unique_id) = equipment_slot_unique_id(item.slot) {
            packets.push(ServerPacket::ItemRepaired {
                unique_id,
                max_dura: item.durability_max,
                current_dura: item.durability_current,
            });
        }
    }

    packets
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
        let index = item_index_for_client_reference(source_items, grid, unique_id)?;
        let mut item = source_items[index].clone();
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
        let Some(index) = equipment_index_for_client_reference(&resources, unique_id) else {
            return vec![failed_packet];
        };
        let equipment = &resources.equipment_items[index];
        if equipment.cursed && !world.resource::<PlayerPermissionResource>().unlock_curse {
            return vec![failed_packet];
        }
        if matches!(grid, MirGridType::Storage)
            && equipment_has_crystal_or_rental_bind_flag(equipment, CRYSTAL_BIND_DONT_STORE)
        {
            return vec![failed_packet];
        }
        destination
    };

    {
        let mut resources = world.resource_mut::<InventoryResource>();
        let Some(index) = equipment_index_for_client_reference(&resources, unique_id) else {
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
        if removed_cursed {
            world
                .resource_mut::<PlayerPermissionResource>()
                .unlock_curse = false;
        }
    }

    super::stats::refresh_player_stats(world);
    vec![ServerPacket::RemoveItem {
        grid,
        unique_id,
        to,
        success: true,
    }]
}

/// Insert a socket gem (Crystal `ItemType.Socket`) from the inventory into a
/// free socket of an inventory host item, mirroring `PlayerObject.EquipSlotItem`.
/// v1 targets inventory hosts; the socketed gems then travel onto the equipped
/// `EquipmentState` via the equip carry-through and contribute to the wearer's
/// stats (socketing an already-equipped item is the documented follow-on).
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
    if grid != MirGridType::Inventory || grid_to != MirGridType::Inventory {
        return vec![failed];
    }
    let mut inventory = world.resource_mut::<InventoryResource>();
    let Some(gem_index) = inventory
        .inventory_items
        .iter()
        .position(|item| item_matches_inventory_unique_id(item, unique_id))
    else {
        return vec![failed];
    };
    if !item_is_socket_type(&inventory.inventory_items[gem_index]) {
        return vec![failed];
    }
    let Some(host_index) = inventory
        .inventory_items
        .iter()
        .position(|item| item_matches_inventory_unique_id(item, to_unique_id))
    else {
        return vec![failed];
    };
    if host_index == gem_index {
        return vec![failed];
    }
    {
        let host = &inventory.inventory_items[host_index];
        // Host must still have a free socket (Crystal checks `item.Slots[to] == null`).
        if host.socketed.len() >= usize::from(host.socket_slots) {
            return vec![failed];
        }
    }
    let gem = inventory.inventory_items.remove(gem_index);
    // The host index may shift if the gem preceded it in the bag.
    let host_index = inventory
        .inventory_items
        .iter()
        .position(|item| item_matches_inventory_unique_id(item, to_unique_id))
        .expect("host item is still present after removing the gem");
    inventory.inventory_items[host_index].socketed.push(gem);
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
    if matches!(
        grid,
        MirGridType::HeroEquipment | MirGridType::HeroInventory
    ) || matches!(
        grid_to,
        MirGridType::HeroEquipment | MirGridType::HeroInventory
    ) {
        return vec![failed_packet];
    }
    // Extract a socketed gem from an inventory host back into the inventory
    // (Crystal `PlayerObject.RemoveSlotItem`); cursed gems cannot be removed.
    if super::resources::is_in_world(world)
        && matches!(grid, MirGridType::Socket)
        && matches!(grid_to, MirGridType::Inventory)
    {
        let mut inventory = world.resource_mut::<InventoryResource>();
        if let Some(host_index) = inventory
            .inventory_items
            .iter()
            .position(|item| item_matches_inventory_unique_id(item, from_unique_id))
        {
            if let Some(socket_pos) = inventory.inventory_items[host_index]
                .socketed
                .iter()
                .position(|gem| gem.unique_id == unique_id)
            {
                if inventory.inventory_items[host_index].socketed[socket_pos].cursed {
                    return vec![failed_packet];
                }
                let gem = inventory.inventory_items[host_index]
                    .socketed
                    .remove(socket_pos);
                inventory.inventory_items.push(gem);
                return vec![ServerPacket::RemoveSlotItem {
                    grid,
                    grid_to,
                    unique_id,
                    to,
                    success: true,
                }];
            }
        }
        return vec![failed_packet];
    }
    if !matches!(
        grid,
        MirGridType::Mount | MirGridType::Fishing | MirGridType::Socket
    ) {
        return vec![failed_packet];
    }
    if !matches!(grid_to, MirGridType::Inventory | MirGridType::Storage) {
        return vec![failed_packet];
    }

    // Mount/fishing embedded slot items are not modeled yet; keep Crystal's
    // RemoveSlotItem envelope without falling through to whole-item removal.
    vec![failed_packet]
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
                }
            }
        }
    }

    #[test]
    fn unresolved_equipment_never_emits_a_dangling_native_user_item() {
        let mut item = seed_equipment_items()[0].clone();
        item.key = "web-only-equipment-without-crystal-item-info".to_string();

        assert!(user_item_from_equipment_state(&item).is_none());
    }
}

pub(super) fn crystal_apply_added_stat_price_factor(price: f32, added_stat_count: usize) -> u32 {
    (price * (1.0 + (added_stat_count as f32 * 0.1))).trunc() as u32
}
