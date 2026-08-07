use serde::{Deserialize, Serialize};

use crate::config::{
    EquipmentSlot, ItemContainer, ItemGrade, Stage5HeroMagicState, WorldItemSnapshot,
};
use bevy_ecs::prelude::World;
use mir2_game_data::{
    crystal_item_by_index, crystal_item_by_name, crystal_recipes, localized_text_or_fallback,
    CrystalItemTemplate, LanguageCode,
};
use mir2_protocol::{
    ChatType, ClientMagic, ItemInfo, MirClass, MirGender, MirGridType, ServerPacket, UserItem,
    UserItemRentalInformation, UserItemSealedInfo, UserItemStat,
};

use super::buffs::{
    apply_crystal_template_consumable_buffs, buff_attack_bonus, buff_defence_bonus,
    queue_crystal_normal_potion_restore, queue_crystal_normal_potion_restore_amounts,
    restore_current_player_vitals, BuffState,
};
use super::combat::deterministic_chance_roll;
use super::components::{
    current_player_is_dead, current_player_object_id, player_entity, PlayerVitals,
};
use super::crystal_compat::*;
use super::drops::drop_item_packet;
use super::equipment::{
    equip_item_impl, equipment_slot_index, feed_mount_with_crystal_food,
    repair_equipped_durability, repair_equipped_weapon_with_oil, slugify_name,
    toggle_mount_ride_from_use_item, try_equip_item, try_luck_weapon, CrystalLuckWeaponOutcome,
    EquipmentState,
};
use super::inventory::{
    add_minutes_to_binary_datetime, add_or_increment_item_with_durability_and_stats,
    binary_datetime_ticks, can_gain_item_quantity, consume_item_at_use_location,
    crystal_duration_label_from_minutes, crystal_duration_label_from_seconds,
    current_binary_datetime, find_use_item_location, future_binary_datetime_minutes,
    item_at_use_location, UseItemLocation,
};
use super::map::{
    current_map_disallows_drug, current_map_disallows_escape,
    current_map_disallows_random_teleport, current_map_disallows_reincarnation,
    current_map_disallows_town_teleport,
};
use super::monsters::deterministic_roll;
use super::movement::{crystal_random_same_map_teleport_packets, town_teleport_packets};
use super::npc_script::gain_credit;
use super::packets::{
    object_health_info_for_entity, object_mana_info_for_entity, object_revived_info_for_entity,
    prepend_optional_packet, use_item_ack,
};
use super::resources::{
    BuffResource, HeroInventoryResource, InventoryResource, PlayerPermissionResource,
    PlayerRuntimeResource, RuntimeConfigResource, SessionResource, SkillResource,
    Stage5SystemsResource,
};
use super::session::{
    current_language, hint_chat_key, hint_chat_key_args, is_in_world, runtime_tick,
    system_message_key, SimulationSession,
};
use super::skills::{client_magic_for_skill_state, crystal_book_skill_state, SkillState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct ItemState {
    pub(super) key: String,
    pub(super) name: String,
    pub(super) icon: u16,
    pub(super) slot: u8,
    #[serde(default)]
    pub(super) unique_id: u64,
    pub(super) container: ItemContainer,
    pub(super) quantity: u32,
    pub(super) description: String,
    pub(super) durability_current: Option<u16>,
    pub(super) durability_max: Option<u16>,
    pub(super) weight: u16,
    pub(super) equip_slot: Option<EquipmentSlot>,
    #[serde(default)]
    pub(super) grade: ItemGrade,
    #[serde(default)]
    pub(super) added_attack: i32,
    #[serde(default)]
    pub(super) added_defence: i32,
    #[serde(default)]
    pub(super) added_stats: Vec<UserItemStat>,
    /// Socket items (Crystal `ItemType.Socket`) inserted into this item's
    /// slots; their stats contribute while the item is worn.
    #[serde(default)]
    pub(super) socketed: Vec<ItemState>,
    #[serde(default)]
    pub(super) cursed: bool,
    #[serde(default)]
    pub(super) socket_slots: u8,
    #[serde(default)]
    pub(super) gem_count: u16,
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
    pub(super) heal_hp: i32,
    pub(super) heal_mp: i32,
}

impl ItemState {
    pub(super) fn snapshot(&self, language: LanguageCode) -> WorldItemSnapshot {
        WorldItemSnapshot {
            key: self.key.clone(),
            name: localized_item_name(language, &self.key, &self.name),
            icon: self.icon,
            unique_id: item_unique_id(self),
            slot: self.slot,
            container: self.container,
            quantity: self.quantity,
            description: localized_item_description(language, &self.key, &self.description),
            durability_current: self.durability_current,
            durability_max: self.durability_max,
            grade: self.grade,
            added_attack: self.added_attack,
            added_defence: self.added_defence,
        }
    }

    pub(super) fn total_weight(&self) -> u32 {
        u32::from(self.weight) * self.quantity
    }
}

pub(super) fn default_item_unique_id(container: ItemContainer, slot: u8) -> u64 {
    match container {
        ItemContainer::Bag1 => u64::from(slot),
        ItemContainer::Bag2 => 40 + u64::from(slot),
        _ => u64::from(slot),
    }
}

pub(super) fn item_unique_id(item: &ItemState) -> u64 {
    if item.unique_id == 0 {
        default_item_unique_id(item.container, item.slot)
    } else {
        item.unique_id
    }
}

pub(super) fn item_state_identified(item: &ItemState) -> bool {
    item.identified
        .unwrap_or_else(|| crystal_default_identified_for_item_key(&item.key))
}

pub(super) fn item_state_soul_bound_id(item: &ItemState) -> i32 {
    item.soul_bound_id.unwrap_or(-1)
}

pub(super) fn localized_item_base_key(key: &str) -> Option<&'static str> {
    match key {
        "red-potion" | "belt-red-potion" => Some("content.item.redPotion"),
        "blue-potion" | "belt-blue-potion" => Some("content.item.bluePotion"),
        "training-manual" => Some("content.item.trainingManual"),
        "bronze-helmet" => Some("content.item.bronzeHelmet"),
        "iron-helmet" => Some("content.item.ironHelmet"),
        "town-teleport" => Some("content.item.townTeleport"),
        "belt-lantern-oil" => Some("content.item.lanternOil"),
        "training-splinter" => Some("content.item.trainingSplinter"),
        "quest-wasp-stinger" => Some("content.item.waspStinger"),
        "repair-powder" => Some("content.item.repairPowder"),
        _ => None,
    }
}

pub(super) fn localized_drop_name_key(name: &str) -> Option<&'static str> {
    match name {
        "Wasp Gold" => Some("content.item.waspGold.name"),
        "Training Splinter" => Some("content.item.trainingSplinter.name"),
        _ => None,
    }
}

pub(super) fn localized_item_name(language: LanguageCode, key: &str, fallback: &str) -> String {
    localized_item_base_key(key)
        .map(|base| localized_text_or_fallback(language, &format!("{base}.name"), fallback))
        .unwrap_or_else(|| fallback.to_string())
}

pub(super) fn localized_item_description(
    language: LanguageCode,
    key: &str,
    fallback: &str,
) -> String {
    localized_item_base_key(key)
        .map(|base| localized_text_or_fallback(language, &format!("{base}.description"), fallback))
        .unwrap_or_else(|| fallback.to_string())
}

pub(super) fn normalize_crystal_item_key(name: &str) -> String {
    let normalized = slugify_name(name.trim()).trim_matches('-').to_string();
    match name.trim().to_ascii_lowercase().as_str() {
        "townteleport" => "town-teleport".to_string(),
        "timestonepiece" => "time-stone-piece".to_string(),
        _ => normalized,
    }
}

pub(super) fn crystal_item_display_name(name: &str) -> String {
    match name.trim() {
        "TownTeleport" => "Town Teleport".to_string(),
        "TimeStonePiece" => "Time Stone Piece".to_string(),
        other => other.to_string(),
    }
}

pub(super) fn crystal_item_description(name: &str) -> String {
    match name.trim() {
        "TownTeleport" => "NPC-issued town teleport token.".to_string(),
        "TimeStonePiece" => "Temporal fragment required by the Prajna time stone.".to_string(),
        other => format!("Crystal NPC item reward: {other}."),
    }
}

pub(super) fn crystal_default_identified_for_item_key(key: &str) -> bool {
    crystal_item_template_for_item_key(key)
        .map(|template| !template.need_identify)
        .unwrap_or(false)
}

pub(super) fn crystal_item_needs_identify(key: &str) -> bool {
    crystal_item_template_for_item_key(key)
        .map(|template| template.need_identify)
        .unwrap_or(false)
}

pub(super) fn item_icon_for_key(key: &str) -> u16 {
    match key {
        "red-potion" | "belt-red-potion" => 23,
        "blue-potion" | "belt-blue-potion" => 15,
        "training-manual" => 121,
        "bronze-helmet" => 106,
        "iron-helmet" => 107,
        "dagger" => 37,
        "assassin-dagger" => 38,
        "training-bow" => 39,
        "leather-armour" => 95,
        "town-teleport" => 79,
        "benediction-oil" => 26,
        "repair-oil" => 3368,
        "war-god-oil" => 3367,
        "belt-lantern-oil" => 119,
        "quest-wasp-stinger" => 174,
        "training-splinter" => 6,
        "repair-powder" => 118,
        key if key.starts_with("credit-token-") => 1813,
        _ => crystal_item_template_for_dynamic_key(key)
            .map(|template| template.image)
            .unwrap_or(0),
    }
}

pub(super) fn crystal_item_index_for_item_state(item: &ItemState) -> i32 {
    crystal_item_template_for_item_key(&item.key)
        .map(|template| template.item_index)
        .unwrap_or_else(|| i32::from(item.icon))
}

pub(super) fn item_info_from_crystal_template(template: CrystalItemTemplate) -> ItemInfo {
    ItemInfo {
        index: template.item_index,
        name: template.name,
        item_type: template.item_type,
        grade: template.grade,
        required_type: template.required_type,
        required_class: template.required_class,
        required_gender: template.required_gender,
        item_set: template.item_set,
        shape: template.shape,
        weight: template.weight,
        light: template.light,
        required_amount: template.required_amount,
        image: template.image,
        durability: template.durability,
        stack_size: template.stack_size,
        price: template.price,
        start_item: template.start_item,
        effect: template.effect,
        need_identify: template.need_identify,
        show_group_pickup: template.show_group_pickup,
        class_based: template.class_based,
        level_based: template.level_based,
        can_mine: template.can_mine,
        global_drop_notify: template.global_drop_notify,
        bind: template.bind,
        unique: template.unique,
        random_stats_id: template.random_stats_id,
        can_fast_run: template.can_fast_run,
        can_awakening: template.can_awakening,
        slots: template.slots,
        stats: template
            .stats
            .into_iter()
            .map(|stat| UserItemStat {
                stat: stat.stat,
                value: stat.value,
            })
            .collect(),
        tooltip: template.tooltip,
    }
}

pub(super) fn user_item_from_item_state(item: &ItemState) -> UserItem {
    let added_stats = merged_user_item_stats(
        &item.added_stats,
        item.added_defence,
        item.added_attack,
        None,
    );

    UserItem {
        unique_id: item_unique_id(item),
        item_index: crystal_item_index_for_item_state(item),
        current_dura: item.durability_current.unwrap_or(0),
        max_dura: item.durability_max.unwrap_or(0),
        count: item.quantity.min(u32::from(u16::MAX)) as u16,
        soul_bound_id: item_state_soul_bound_id(item),
        identified: item_state_identified(item),
        cursed: item.cursed,
        slots: vec![None; usize::from(item.socket_slots)],
        gem_count: item.gem_count,
        added_stats,
        awake_type: 0,
        awake_values: Vec::new(),
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
    }
}

pub(super) fn upsert_user_item_stat(stats: &mut Vec<UserItemStat>, stat: u8, value: i32) {
    if value == 0 || stats.iter().any(|existing| existing.stat == stat) {
        return;
    }

    stats.push(UserItemStat { stat, value });
}

pub(super) fn increment_user_item_stat(stats: &mut Vec<UserItemStat>, stat: u8, value: i32) {
    if value == 0 {
        return;
    }

    if let Some(existing) = stats.iter_mut().find(|existing| existing.stat == stat) {
        existing.value = existing.value.saturating_add(value);
    } else {
        stats.push(UserItemStat { stat, value });
    }
}

pub(super) fn merged_user_item_stats(
    base: &[UserItemStat],
    added_defence: i32,
    added_attack: i32,
    added_luck: Option<i32>,
) -> Vec<UserItemStat> {
    let mut stats = base.to_vec();
    upsert_user_item_stat(&mut stats, 1, added_defence);
    upsert_user_item_stat(&mut stats, 5, added_attack);
    if let Some(added_luck) = added_luck {
        upsert_user_item_stat(&mut stats, 15, added_luck);
    }
    stats
}

pub(super) fn user_item_stat_total(stats: &[UserItemStat], stat: u8) -> i32 {
    stats
        .iter()
        .filter(|entry| entry.stat == stat)
        .map(|entry| entry.value)
        .sum()
}

pub(super) fn user_item_added_attack_defence(item: &UserItem) -> (i32, i32) {
    let mut added_attack = 0;
    let mut added_defence = 0;
    for stat in &item.added_stats {
        match stat.stat {
            1 => added_defence += stat.value,
            5 => added_attack += stat.value,
            _ => {}
        }
    }
    (added_attack, added_defence)
}

pub(super) fn user_item_rental_information(
    binding_flags: i16,
    owner_name: &str,
    expiry_binary_datetime: i64,
    rental_locked: bool,
) -> Option<UserItemRentalInformation> {
    (binding_flags != 0 || rental_locked || !owner_name.is_empty() || expiry_binary_datetime != 0)
        .then_some(UserItemRentalInformation {
            owner_name: owner_name.to_string(),
            binding_flags,
            expiry_binary_datetime,
            rental_locked,
        })
}

pub(super) fn crystal_socket_slot_limit_for_item_key(key: &str) -> Option<u8> {
    crystal_item_template_for_item_key(key).map(|template| template.slots)
}

pub(super) fn crystal_socket_source_valid_for_item(source: &ItemState, target_key: &str) -> bool {
    if source.key == "stage5-socket-source" {
        return true;
    }

    let Some(source_template) = crystal_item_template_for_item_key(&source.key) else {
        return false;
    };
    if source_template.item_type != CRYSTAL_ITEM_TYPE_GEM
        || source_template.shape != CRYSTAL_GEM_SHAPE_SOCKET
    {
        return false;
    }

    let Some(target_template) = crystal_item_template_for_item_key(target_key) else {
        return false;
    };

    crystal_socket_source_unique_matches_item_type(
        source_template.unique,
        target_template.item_type,
    )
}

pub(super) fn crystal_socket_source_unique_matches_item_type(
    source_unique: i16,
    target_item_type: u8,
) -> bool {
    let required_flag = match target_item_type {
        1 => CRYSTAL_SPECIAL_PARALYZE,
        2 => CRYSTAL_SPECIAL_TELEPORT,
        4 => CRYSTAL_SPECIAL_CLEAR_RING,
        5 => CRYSTAL_SPECIAL_PROTECTION,
        6 => CRYSTAL_SPECIAL_REVIVAL,
        7 => CRYSTAL_SPECIAL_MUSCLE,
        8 => CRYSTAL_SPECIAL_FLAME,
        9 => CRYSTAL_SPECIAL_HEALING,
        10 => CRYSTAL_SPECIAL_PROBE,
        11 => CRYSTAL_SPECIAL_SKILL,
        12 => CRYSTAL_SPECIAL_NO_DURA_LOSS,
        _ => return false,
    };

    source_unique & required_flag != 0
}

pub(super) fn crystal_seal_minutes_for_source_item(
    item: &ItemState,
    fallback_minutes: u64,
) -> Option<u64> {
    if item.key == "stage5-seal-source" {
        return Some(
            item.durability_current
                .filter(|minutes| *minutes > 0)
                .map(u64::from)
                .unwrap_or(fallback_minutes)
                .max(1),
        );
    }

    let template = crystal_item_template_for_item_key(&item.key)?;
    if template.item_type != CRYSTAL_ITEM_TYPE_GEM || template.shape != CRYSTAL_GEM_SHAPE_SEAL {
        return None;
    }

    let minutes = item.durability_current.unwrap_or(template.durability);
    (minutes > 0).then_some(u64::from(minutes))
}

pub(super) fn crystal_item_stat_value(template: &CrystalItemTemplate, stat: u8) -> i32 {
    template
        .stats
        .iter()
        .find(|entry| entry.stat == stat)
        .map(|entry| entry.value)
        .unwrap_or(0)
}

pub(super) fn crystal_item_added_stat_value(item: &ItemState, stat: u8) -> i32 {
    let stats_total: i32 = item
        .added_stats
        .iter()
        .filter(|entry| entry.stat == stat)
        .map(|entry| entry.value)
        .sum();

    match stat {
        CRYSTAL_STAT_MAX_AC => stats_total.saturating_add(item.added_defence),
        CRYSTAL_STAT_MAX_DC => stats_total.saturating_add(item.added_attack),
        _ => stats_total,
    }
}

pub(super) fn crystal_equipment_added_stat_total(resources: &InventoryResource, stat: u8) -> i32 {
    resources
        .equipment_items
        .iter()
        .filter(|item| !item.is_broken())
        .map(|item| {
            item.added_stats
                .iter()
                .filter(|entry| entry.stat == stat)
                .map(|entry| entry.value)
                .sum::<i32>()
                + item.socketed_added_stat(stat)
        })
        .sum()
}

pub(super) fn current_player_gem_rate_bonus(world: &World) -> i32 {
    crystal_equipment_added_stat_total(
        world.resource::<InventoryResource>(),
        CRYSTAL_STAT_GEM_RATE_PERCENT,
    )
}

pub(super) fn crystal_upgrade_target_stat(source_template: &CrystalItemTemplate) -> Option<u8> {
    // Current Crystal gem/orb data uses HPDrainRatePercent as the max-added-stats
    // control field, not as the applied upgrade stat. Durability gems must fall
    // through to the MaxDura path below instead of being treated as stat-48 upgrades.
    [
        CRYSTAL_STAT_MAX_DC,
        CRYSTAL_STAT_MAX_MC,
        CRYSTAL_STAT_MAX_SC,
        CRYSTAL_STAT_MAX_AC,
        CRYSTAL_STAT_MAX_MAC,
        CRYSTAL_STAT_ATTACK_SPEED,
        CRYSTAL_STAT_AGILITY,
        CRYSTAL_STAT_ACCURACY,
        CRYSTAL_STAT_POISON_ATTACK,
        CRYSTAL_STAT_FREEZING,
        CRYSTAL_STAT_MAGIC_RESIST,
        CRYSTAL_STAT_POISON_RESIST,
        CRYSTAL_STAT_LUCK,
        CRYSTAL_STAT_POISON_RECOVERY,
        CRYSTAL_STAT_HP,
        CRYSTAL_STAT_MP,
        CRYSTAL_STAT_HEALTH_RECOVERY,
        CRYSTAL_STAT_SPELL_RECOVERY,
        CRYSTAL_STAT_STRONG,
    ]
    .into_iter()
    .find(|stat| crystal_item_stat_value(source_template, *stat) > 0)
}

pub(super) fn crystal_upgrade_current_stat_count(
    source_item: &ItemState,
    source_template: &CrystalItemTemplate,
    target_item: &ItemState,
    target_template: &CrystalItemTemplate,
) -> i32 {
    if let Some(stat) = crystal_upgrade_target_stat(source_template) {
        return crystal_item_added_stat_value(target_item, stat);
    }

    let source_durability = source_item
        .durability_max
        .unwrap_or(source_template.durability);
    if source_durability == 0 && source_template.durability == 0 {
        return 0;
    }

    let base_max = i32::from(target_template.durability);
    let current_max = i32::from(
        target_item
            .durability_max
            .unwrap_or(target_template.durability),
    );
    if current_max <= base_max {
        0
    } else {
        (current_max - base_max) / 1000
    }
}

#[cfg(test)]
pub(super) fn crystal_upgrade_success_chance(
    source_template: &CrystalItemTemplate,
    target_item: &ItemState,
) -> i32 {
    crystal_upgrade_success_chance_with_player_bonus(source_template, target_item, 0)
}

pub(super) fn crystal_upgrade_success_chance_with_player_bonus(
    source_template: &CrystalItemTemplate,
    target_item: &ItemState,
    player_gem_rate_bonus: i32,
) -> i32 {
    let reflect = crystal_item_stat_value(source_template, CRYSTAL_STAT_REFLECT).max(0);
    let multiplier = crystal_upgrade_target_stat(source_template)
        .map(|stat| crystal_item_added_stat_value(target_item, stat).max(0))
        .unwrap_or(i32::from(target_item.gem_count));
    let adjusted = reflect.saturating_mul(multiplier);
    let critical_rate = crystal_item_stat_value(source_template, CRYSTAL_STAT_CRITICAL_RATE).max(0);

    if adjusted >= critical_rate {
        0
    } else {
        critical_rate
            .saturating_sub(adjusted)
            .saturating_add(player_gem_rate_bonus)
    }
}

pub(super) fn crystal_upgrade_roll_succeeds(
    current_tick: u64,
    player_object_id: u32,
    from_slot: u8,
    to_slot: u8,
    success_chance: i32,
) -> bool {
    if success_chance <= 0 {
        return false;
    }

    deterministic_roll(
        current_tick,
        player_object_id as usize,
        usize::from(from_slot) * 257 + usize::from(to_slot),
        100,
    ) < u64::try_from(success_chance.min(100)).unwrap_or(0)
}

pub(super) fn crystal_upgrade_roll_destroys(
    current_tick: u64,
    player_object_id: u32,
    from_slot: u8,
    to_slot: u8,
) -> bool {
    deterministic_chance_roll(
        current_tick,
        player_object_id,
        u64::from(from_slot) * 521 + u64::from(to_slot) + 3,
        5,
    )
}

pub(super) fn apply_crystal_item_upgrade(
    target_item: &mut ItemState,
    target_template: &CrystalItemTemplate,
    source_item: &ItemState,
    source_template: &CrystalItemTemplate,
) -> bool {
    if let Some(stat) = crystal_upgrade_target_stat(source_template) {
        let value = crystal_item_stat_value(source_template, stat);
        if value <= 0 {
            return false;
        }

        match stat {
            CRYSTAL_STAT_MAX_DC => {
                target_item.added_attack = target_item.added_attack.saturating_add(value);
            }
            CRYSTAL_STAT_MAX_AC => {
                target_item.added_defence = target_item.added_defence.saturating_add(value);
            }
            _ => increment_user_item_stat(&mut target_item.added_stats, stat, value),
        }
        return true;
    }

    let source_durability = source_item
        .durability_max
        .unwrap_or(source_template.durability);
    if source_durability == 0 && source_template.durability == 0 {
        return false;
    }

    let current_max = target_item
        .durability_max
        .unwrap_or(target_template.durability);
    target_item.durability_max = Some(current_max.saturating_add(source_durability));
    true
}

enum CombineItemOutcome {
    AckOnlyFailure,
    FailureHint {
        key: &'static str,
        args: Vec<String>,
    },
    RepairSuccess {
        unique_id: u64,
        max_dura: u16,
        current_dura: u16,
    },
    SocketSuccess {
        unique_id: u64,
        slot_size: i32,
    },
    SealSuccess {
        unique_id: u64,
        expiry_date_binary_datetime: i64,
        minutes: u64,
    },
    UpgradeResult {
        key: &'static str,
        args: Vec<String>,
        item: Option<UserItem>,
        destroy: bool,
    },
}

pub(super) fn crystal_combine_repair_matches_item_type(
    source_shape: i16,
    target_item_type: u8,
) -> bool {
    match source_shape {
        CRYSTAL_GEM_SHAPE_REPAIR_HAMMER | CRYSTAL_GEM_SHAPE_SPECIAL_REPAIR_HAMMER => matches!(
            target_item_type,
            CRYSTAL_ITEM_TYPE_WEAPON
                | CRYSTAL_ITEM_TYPE_NECKLACE
                | CRYSTAL_ITEM_TYPE_RING
                | CRYSTAL_ITEM_TYPE_BRACELET
        ),
        CRYSTAL_GEM_SHAPE_REPAIR_SEWING | CRYSTAL_GEM_SHAPE_SPECIAL_REPAIR_SEWING => matches!(
            target_item_type,
            CRYSTAL_ITEM_TYPE_ARMOUR
                | CRYSTAL_ITEM_TYPE_HELMET
                | CRYSTAL_ITEM_TYPE_BOOTS
                | CRYSTAL_ITEM_TYPE_BELT
        ),
        _ => false,
    }
}

pub(super) fn crystal_combine_repair_max_dura_loss(
    current_tick: u64,
    player_object_id: u32,
    from_slot: u8,
    to_slot: u8,
) -> u16 {
    u16::try_from(deterministic_roll(
        current_tick,
        usize::try_from(player_object_id).unwrap_or_default(),
        usize::from(from_slot) * 733 + usize::from(to_slot) + 41,
        10,
    ))
    .expect("combine repair roll should fit u16")
    .saturating_mul(100)
}

pub(super) fn combine_item_impl(
    world: &mut World,
    grid: MirGridType,
    id_from: u64,
    id_to: u64,
) -> Vec<ServerPacket> {
    let failed_packet = ServerPacket::CombineItem {
        grid,
        id_from,
        id_to,
        success: false,
        destroy: false,
    };

    if grid != MirGridType::Inventory {
        return vec![failed_packet];
    }
    if current_player_is_dead(world) {
        return vec![failed_packet];
    }

    let now_binary_datetime = current_binary_datetime();
    let now_ticks = binary_datetime_ticks(now_binary_datetime);
    let current_tick = runtime_tick(world);
    let player_object_id = current_player_object_id(world).unwrap_or(0);
    let player_gem_rate_bonus = current_player_gem_rate_bonus(world);
    let outcome = {
        let mut resources = world.resource_mut::<InventoryResource>();
        let Some(from_index) = resources.inventory_items.iter().position(|item| {
            item_unique_id(item) == id_from
                && matches!(item.container, ItemContainer::Bag1 | ItemContainer::Bag2)
        }) else {
            return vec![failed_packet];
        };
        let Some(to_index) = resources.inventory_items.iter().position(|item| {
            item_unique_id(item) == id_to
                && matches!(item.container, ItemContainer::Bag1 | ItemContainer::Bag2)
        }) else {
            return vec![failed_packet];
        };
        if from_index == to_index {
            return vec![failed_packet];
        }

        let source_item = resources.inventory_items[from_index].clone();
        let from_slot = source_item.slot;
        let to_slot = resources.inventory_items[to_index].slot;
        let target_unique_id = item_unique_id(&resources.inventory_items[to_index]);
        let target_key = resources.inventory_items[to_index].key.clone();
        let Some(target_template) = crystal_item_template_for_item_key(&target_key) else {
            return vec![failed_packet];
        };
        if !(1..=11).contains(&target_template.item_type) {
            return vec![failed_packet];
        }

        let source_shape = if source_item.key == "stage5-socket-source" {
            Some(CRYSTAL_GEM_SHAPE_SOCKET)
        } else if source_item.key == "stage5-seal-source" {
            Some(CRYSTAL_GEM_SHAPE_SEAL)
        } else {
            crystal_item_template_for_item_key(&source_item.key).and_then(|template| {
                (template.item_type == CRYSTAL_ITEM_TYPE_GEM
                    && (template.shape == CRYSTAL_GEM_SHAPE_REPAIR_HAMMER
                        || template.shape == CRYSTAL_GEM_SHAPE_REPAIR_SEWING
                        || template.shape == CRYSTAL_GEM_SHAPE_UPGRADE_GEM
                        || template.shape == CRYSTAL_GEM_SHAPE_UPGRADE_ORB
                        || template.shape == CRYSTAL_GEM_SHAPE_SPECIAL_REPAIR_HAMMER
                        || template.shape == CRYSTAL_GEM_SHAPE_SPECIAL_REPAIR_SEWING
                        || template.shape == CRYSTAL_GEM_SHAPE_SOCKET
                        || template.shape == CRYSTAL_GEM_SHAPE_SEAL))
                    .then_some(template.shape)
            })
        };

        match source_shape {
            Some(CRYSTAL_GEM_SHAPE_UPGRADE_GEM) | Some(CRYSTAL_GEM_SHAPE_UPGRADE_ORB) => {
                if !(1..=11).contains(&target_template.item_type) {
                    CombineItemOutcome::AckOnlyFailure
                } else if item_has_crystal_or_rental_bind_flag(
                    &resources.inventory_items[to_index],
                    CRYSTAL_BIND_DONT_UPGRADE,
                ) || target_template.unique != 0
                {
                    CombineItemOutcome::AckOnlyFailure
                } else {
                    let Some(source_template) =
                        crystal_item_template_for_item_key(&source_item.key)
                    else {
                        return vec![failed_packet];
                    };

                    let max_gem_count =
                        crystal_item_stat_value(&source_template, CRYSTAL_STAT_CRITICAL_DAMAGE);
                    let max_stat_count = crystal_item_stat_value(
                        &source_template,
                        CRYSTAL_STAT_HP_DRAIN_RATE_PERCENT,
                    );
                    if i32::from(resources.inventory_items[to_index].gem_count) >= max_gem_count
                        || crystal_upgrade_current_stat_count(
                            &source_item,
                            &source_template,
                            &resources.inventory_items[to_index],
                            &target_template,
                        ) >= max_stat_count
                    {
                        CombineItemOutcome::FailureHint {
                            key: "server.ItemMaxAddedStats",
                            args: Vec::new(),
                        }
                    } else if !crystal_socket_source_unique_matches_item_type(
                        source_template.unique,
                        target_template.item_type,
                    ) {
                        CombineItemOutcome::FailureHint {
                            key: "server.InvalidCombination",
                            args: Vec::new(),
                        }
                    } else {
                        let success_chance = crystal_upgrade_success_chance_with_player_bonus(
                            &source_template,
                            &resources.inventory_items[to_index],
                            player_gem_rate_bonus,
                        );
                        let succeeded = crystal_upgrade_roll_succeeds(
                            current_tick,
                            player_object_id,
                            from_slot,
                            to_slot,
                            success_chance,
                        );
                        let mut destroy = false;
                        let key = if succeeded {
                            if !apply_crystal_item_upgrade(
                                &mut resources.inventory_items[to_index],
                                &target_template,
                                &source_item,
                                &source_template,
                            ) {
                                return vec![
                                    hint_chat_key(world, "server.CannotCombineItems"),
                                    failed_packet,
                                ];
                            }
                            resources.inventory_items[to_index].gem_count = resources
                                .inventory_items[to_index]
                                .gem_count
                                .saturating_add(1);
                            "server.ItemUpgraded"
                        } else if matches!(source_shape, Some(CRYSTAL_GEM_SHAPE_UPGRADE_GEM))
                            && crystal_upgrade_roll_destroys(
                                current_tick,
                                player_object_id,
                                from_slot,
                                to_slot,
                            )
                        {
                            destroy = true;
                            "server.ItemHasBeenDestroyed"
                        } else {
                            "server.UpgradeNoEffect"
                        };

                        let item = if succeeded {
                            Some(user_item_from_item_state(
                                &resources.inventory_items[to_index],
                            ))
                        } else {
                            None
                        };
                        let consume_source_stack =
                            resources.inventory_items[from_index].quantity <= 1;
                        if !consume_source_stack {
                            resources.inventory_items[from_index].quantity -= 1;
                        }

                        let mut removal_indexes = Vec::new();
                        if consume_source_stack {
                            removal_indexes.push(from_index);
                        }
                        if destroy {
                            removal_indexes.push(to_index);
                        }
                        removal_indexes.sort_unstable();
                        removal_indexes.dedup();
                        for index in removal_indexes.into_iter().rev() {
                            resources.inventory_items.remove(index);
                        }

                        CombineItemOutcome::UpgradeResult {
                            key,
                            args: Vec::new(),
                            item,
                            destroy,
                        }
                    }
                }
            }
            Some(CRYSTAL_GEM_SHAPE_REPAIR_HAMMER)
            | Some(CRYSTAL_GEM_SHAPE_REPAIR_SEWING)
            | Some(CRYSTAL_GEM_SHAPE_SPECIAL_REPAIR_HAMMER)
            | Some(CRYSTAL_GEM_SHAPE_SPECIAL_REPAIR_SEWING) => {
                let source_shape = source_shape.expect("repair branch should have source shape");
                if crystal_item_has_bind_flag(&target_key, CRYSTAL_BIND_DONT_REPAIR) {
                    CombineItemOutcome::AckOnlyFailure
                } else if !crystal_combine_repair_matches_item_type(
                    source_shape,
                    target_template.item_type,
                ) {
                    CombineItemOutcome::AckOnlyFailure
                } else {
                    let current_dura = resources.inventory_items[to_index]
                        .durability_current
                        .unwrap_or(0);
                    let max_dura = resources.inventory_items[to_index]
                        .durability_max
                        .unwrap_or(0);
                    if current_dura == max_dura {
                        CombineItemOutcome::FailureHint {
                            key: "server.ItemNoRepairNeeded",
                            args: Vec::new(),
                        }
                    } else {
                        let next_max_dura = if matches!(target_template.shape, 1 | 2) {
                            max_dura.saturating_sub(crystal_combine_repair_max_dura_loss(
                                current_tick,
                                player_object_id,
                                from_slot,
                                to_slot,
                            ))
                        } else {
                            max_dura
                        };
                        resources.inventory_items[to_index].durability_max = Some(next_max_dura);
                        resources.inventory_items[to_index].durability_current =
                            Some(next_max_dura);
                        if resources.inventory_items[from_index].quantity > 1 {
                            resources.inventory_items[from_index].quantity -= 1;
                        } else {
                            resources.inventory_items.remove(from_index);
                        }
                        CombineItemOutcome::RepairSuccess {
                            unique_id: target_unique_id,
                            max_dura: next_max_dura,
                            current_dura: next_max_dura,
                        }
                    }
                }
            }
            Some(CRYSTAL_GEM_SHAPE_SOCKET) => {
                if item_has_crystal_or_rental_bind_flag(
                    &resources.inventory_items[to_index],
                    CRYSTAL_BIND_DONT_UPGRADE,
                ) || target_template.unique != 0
                {
                    CombineItemOutcome::AckOnlyFailure
                } else if !crystal_socket_source_valid_for_item(&source_item, &target_key) {
                    CombineItemOutcome::FailureHint {
                        key: "server.InvalidCombination",
                        args: Vec::new(),
                    }
                } else if target_template.slots == 0
                    || resources.inventory_items[to_index].socket_slots >= target_template.slots
                {
                    CombineItemOutcome::FailureHint {
                        key: "server.ItemMaxSockets",
                        args: Vec::new(),
                    }
                } else {
                    resources.inventory_items[to_index].socket_slots = resources.inventory_items
                        [to_index]
                        .socket_slots
                        .saturating_add(1);
                    let unique_id = target_unique_id;
                    let slot_size = i32::from(resources.inventory_items[to_index].socket_slots);
                    if resources.inventory_items[from_index].quantity > 1 {
                        resources.inventory_items[from_index].quantity -= 1;
                    } else {
                        resources.inventory_items.remove(from_index);
                    }
                    CombineItemOutcome::SocketSuccess {
                        unique_id,
                        slot_size,
                    }
                }
            }
            Some(CRYSTAL_GEM_SHAPE_SEAL) => {
                if crystal_item_has_bind_flag(&target_key, CRYSTAL_BIND_DONT_UPGRADE)
                    || target_template.unique != 0
                {
                    CombineItemOutcome::AckOnlyFailure
                } else if resources.inventory_items[to_index].sealed_expiry_time_binary_datetime
                    != 0
                    && binary_datetime_ticks(
                        resources.inventory_items[to_index].sealed_expiry_time_binary_datetime,
                    ) > now_ticks
                {
                    CombineItemOutcome::FailureHint {
                        key: "server.ItemAlreadySealed",
                        args: Vec::new(),
                    }
                } else if resources.inventory_items[to_index].sealed_next_time_binary_datetime != 0
                    && binary_datetime_ticks(
                        resources.inventory_items[to_index].sealed_next_time_binary_datetime,
                    ) > now_ticks
                {
                    let remaining_ticks = binary_datetime_ticks(
                        resources.inventory_items[to_index].sealed_next_time_binary_datetime,
                    ) - now_ticks;
                    let remaining_seconds =
                        u64::try_from((remaining_ticks + 9_999_999) / 10_000_000).unwrap_or(1);
                    CombineItemOutcome::FailureHint {
                        key: "server.ItemCannotBeResealedFor",
                        args: vec![crystal_duration_label_from_seconds(
                            remaining_seconds.max(1),
                        )],
                    }
                } else {
                    let Some(minutes) = crystal_seal_minutes_for_source_item(&source_item, 1)
                    else {
                        return vec![failed_packet];
                    };
                    let expiry_date_binary_datetime = future_binary_datetime_minutes(minutes);
                    let next_seal_binary_datetime = add_minutes_to_binary_datetime(
                        expiry_date_binary_datetime,
                        CRYSTAL_ITEM_SEAL_DELAY_MINUTES,
                    );
                    resources.inventory_items[to_index].sealed_expiry_time_binary_datetime =
                        expiry_date_binary_datetime;
                    resources.inventory_items[to_index].sealed_next_time_binary_datetime =
                        next_seal_binary_datetime;
                    let unique_id = target_unique_id;
                    if resources.inventory_items[from_index].quantity > 1 {
                        resources.inventory_items[from_index].quantity -= 1;
                    } else {
                        resources.inventory_items.remove(from_index);
                    }
                    CombineItemOutcome::SealSuccess {
                        unique_id,
                        expiry_date_binary_datetime,
                        minutes,
                    }
                }
            }
            _ => CombineItemOutcome::AckOnlyFailure,
        }
    };

    match outcome {
        CombineItemOutcome::AckOnlyFailure => vec![failed_packet],
        CombineItemOutcome::FailureHint { key, args } => {
            let message = if args.is_empty() {
                hint_chat_key(world, key)
            } else {
                hint_chat_key_args(world, key, args)
            };
            vec![message, failed_packet]
        }
        CombineItemOutcome::RepairSuccess {
            unique_id,
            max_dura,
            current_dura,
        } => vec![
            hint_chat_key(world, "server.ItemRepaired"),
            ServerPacket::ItemRepaired {
                unique_id,
                max_dura,
                current_dura,
            },
            ServerPacket::CombineItem {
                grid,
                id_from,
                id_to,
                success: true,
                destroy: false,
            },
        ],
        CombineItemOutcome::SocketSuccess {
            unique_id,
            slot_size,
        } => vec![
            hint_chat_key(world, "server.ItemSocketsIncreased"),
            ServerPacket::ItemSlotSizeChanged {
                unique_id,
                slot_size,
            },
            ServerPacket::CombineItem {
                grid,
                id_from,
                id_to,
                success: true,
                destroy: false,
            },
        ],
        CombineItemOutcome::SealSuccess {
            unique_id,
            expiry_date_binary_datetime,
            minutes,
        } => vec![
            hint_chat_key_args(
                world,
                "server.ItemSealedFor",
                [crystal_duration_label_from_minutes(minutes)],
            ),
            ServerPacket::ItemSealChanged {
                unique_id,
                expiry_date_binary_datetime,
            },
            ServerPacket::CombineItem {
                grid,
                id_from,
                id_to,
                success: true,
                destroy: false,
            },
        ],
        CombineItemOutcome::UpgradeResult {
            key,
            args,
            item,
            destroy,
        } => {
            let message = if args.is_empty() {
                hint_chat_key(world, key)
            } else {
                hint_chat_key_args(world, key, args)
            };
            let mut packets = vec![message];
            if let Some(item) = item {
                packets.push(ServerPacket::ItemUpgraded { item });
            }
            packets.push(ServerPacket::CombineItem {
                grid,
                id_from,
                id_to,
                success: true,
                destroy,
            });
            packets
        }
    }
}

/// Finds the first offered inventory slot (not already used) holding an item whose
/// Crystal template index matches `required_index`, mirroring Crystal's
/// `item.Info != ingredient.Info` comparison. Returns the inventory vector index
/// and the matched slot.
fn find_recipe_ingredient_slot(
    resources: &InventoryResource,
    slots: &[i32],
    used_slots: &[i32],
    required_index: i32,
) -> Option<(usize, i32)> {
    for &slot in slots {
        if slot < 0 || used_slots.contains(&slot) {
            continue;
        }
        let Some(inv_index) = resources.inventory_items.iter().position(|item| {
            i32::from(item.slot) == slot
                && matches!(item.container, ItemContainer::Bag1 | ItemContainer::Bag2)
        }) else {
            continue;
        };
        let matches_index =
            crystal_item_template_for_item_key(&resources.inventory_items[inv_index].key)
                .map(|template| template.item_index)
                == Some(required_index);
        if matches_index {
            return Some((inv_index, slot));
        }
    }
    None
}

/// Crystal `NPCScript.Craft`: combine the recipe's ingredients — consuming tool
/// durability and gold — to attempt producing the output item. Ingredient counts,
/// gold cost and success chance come straight from the decoded recipe data, so the
/// transaction stays 1:1 with the original server. Crystal semantics preserved
/// exactly: a *valid* attempt always consumes the ingredients/gold and returns
/// `success: true`; the produced item is granted only when the chance roll passes
/// (a failed roll still consumes everything).
pub(super) fn craft_item_impl(
    world: &mut World,
    unique_id: u64,
    count: u16,
    slots: Vec<i32>,
) -> Vec<ServerPacket> {
    let fail = || vec![ServerPacket::CraftItem { success: false }];
    if !is_in_world(world) || current_player_is_dead(world) {
        return fail();
    }

    // Locate the recipe by the output item's unique id (recipe.Item.UniqueID).
    let Some(recipe) = crystal_recipes()
        .into_iter()
        .find(|recipe| recipe.output_unique_id == unique_id)
    else {
        return fail();
    };
    let Some(goods_template) = crystal_item_by_index(recipe.output.item_index) else {
        return fail();
    };

    let goods_stack = u32::from(goods_template.stack_size.max(1));
    let goods_count = u32::from(recipe.output.count.max(1));
    let craft_count = u32::from(count);

    // goods == null || count == 0 || count > goods.Info.StackSize
    if count == 0 || craft_count > goods_stack {
        return fail();
    }
    // Account.Gold < recipe.Gold * count
    let needed_gold = recipe.gold.saturating_mul(craft_count);
    if world.resource::<PlayerRuntimeResource>().gold < needed_gold {
        return fail();
    }
    // count > goods.Info.StackSize / goods.Count
    if craft_count > goods_stack / goods_count {
        return fail();
    }

    // Resolve every required tool/ingredient against the offered slots and build the
    // consumption plan before mutating anything.
    let mut used_slots: Vec<i32> = Vec::new();
    let mut tool_indexes: Vec<usize> = Vec::new();
    let mut ingredient_plan: Vec<(usize, u32)> = Vec::new();
    {
        let resources = world.resource::<InventoryResource>();

        // Tools: present with floor(CurrentDura / 1000) >= count.
        for tool in &recipe.tools {
            let Some((inv_index, slot)) =
                find_recipe_ingredient_slot(&resources, &slots, &used_slots, tool.item_index)
            else {
                return fail();
            };
            used_slots.push(slot);
            let current_dura = u32::from(
                resources.inventory_items[inv_index]
                    .durability_current
                    .unwrap_or(0),
            );
            if current_dura / 1000 < craft_count {
                return fail();
            }
            tool_indexes.push(inv_index);
        }

        // Ingredients: a single matching stack must supply Count * count.
        for ingredient in &recipe.ingredients {
            let Some(ingredient_template) = crystal_item_by_index(ingredient.item_index) else {
                return fail();
            };
            let ingredient_stack = u32::from(ingredient_template.stack_size.max(1));
            let amount = u32::from(ingredient.count).saturating_mul(craft_count);
            // ingredient.Count * count > ingredient.Info.StackSize
            if amount > ingredient_stack {
                return fail();
            }

            let Some((inv_index, slot)) =
                find_recipe_ingredient_slot(&resources, &slots, &used_slots, ingredient.item_index)
            else {
                return fail();
            };
            used_slots.push(slot);

            let item = &resources.inventory_items[inv_index];
            // Durability requirement: ingredient.CurrentDura < MaxDura && > item.CurrentDura.
            if ingredient.current_dura < ingredient.max_dura
                && u32::from(ingredient.current_dura)
                    > u32::from(item.durability_current.unwrap_or(0))
            {
                return fail();
            }
            if amount > item.quantity {
                return fail();
            }
            ingredient_plan.push((inv_index, amount));
        }

        // usedSlots.Count != Tools.Count + Ingredients.Count
        if used_slots.len() != recipe.tools.len() + recipe.ingredients.len() {
            return fail();
        }

        // CanGainItem(craftedItem)
        let key = crystal_item_key_for_template(&goods_template);
        let produced = goods_count.saturating_mul(craft_count);
        if !can_gain_item_quantity(&resources, ItemContainer::Bag1, &key, produced) {
            return fail();
        }
    }

    // Validation passed — apply consumption (tool durability, ingredients, gold).
    let current_tick = runtime_tick(world);
    {
        let mut resources = world.resource_mut::<InventoryResource>();
        for &inv_index in &tool_indexes {
            let item = &mut resources.inventory_items[inv_index];
            let remaining = u32::from(item.durability_current.unwrap_or(0))
                .saturating_sub(craft_count.saturating_mul(1000));
            item.durability_current = Some(u16::try_from(remaining).unwrap_or(u16::MAX));
        }

        let mut removals: Vec<usize> = Vec::new();
        for &(inv_index, amount) in &ingredient_plan {
            let item = &mut resources.inventory_items[inv_index];
            if item.quantity > amount {
                item.quantity -= amount;
            } else {
                removals.push(inv_index);
            }
        }
        removals.sort_unstable_by(|a, b| b.cmp(a));
        removals.dedup();
        for inv_index in removals {
            resources.inventory_items.remove(inv_index);
        }
    }

    world.resource_mut::<PlayerRuntimeResource>().gold -= needed_gold;
    let mut packets = vec![ServerPacket::LoseGold { gold: needed_gold }];

    // Success roll mirrors `Random.Next(100) >= Chance` (no CraftRatePercent source).
    let roll = deterministic_roll(
        current_tick,
        recipe.output.item_index.max(0) as usize,
        unique_id as usize,
        100,
    );
    if roll < u64::from(recipe.chance) {
        let key = crystal_item_key_for_template(&goods_template);
        let produced = goods_count.saturating_mul(craft_count);
        let durability = (goods_template.durability > 0).then_some(goods_template.durability);
        let gained = add_or_increment_item_with_durability_and_stats(
            world,
            ItemContainer::Bag1,
            &key,
            &goods_template.name,
            goods_template
                .tooltip
                .as_deref()
                .unwrap_or("Crystal crafted item."),
            8,
            produced,
            u16::from(goods_template.weight.max(1)),
            durability,
            durability,
            0,
            0,
        );
        packets.push(ServerPacket::GainedItem {
            item: user_item_from_item_state(&gained),
        });
    }

    packets.push(ServerPacket::CraftItem { success: true });
    packets
}

pub(super) fn revive_current_player_from_resurrection_scroll(
    world: &mut World,
) -> Vec<ServerPacket> {
    let Some(player) = player_entity(world) else {
        return Vec::new();
    };

    let revived_vitals = {
        let mut entry = world.entity_mut(player);
        let mut vitals = entry.get_mut::<PlayerVitals>().expect("player vitals");
        vitals.hp = vitals.max_hp.max(1);
        vitals.mp = vitals.max_mp;
        *vitals
    };

    world.resource_mut::<PlayerRuntimeResource>().player_vitals = revived_vitals;

    let mut packets = Vec::new();
    if let Some(info) = object_revived_info_for_entity(world, player, true) {
        packets.push(ServerPacket::ObjectRevived { info });
    }
    if let Some(info) = object_health_info_for_entity(world, player, 0) {
        packets.push(ServerPacket::ObjectHealth { info });
    }
    packets
}

pub(super) fn use_dynamic_crystal_template_item(
    world: &mut World,
    template: &CrystalItemTemplate,
    location: UseItemLocation,
    packet_ack: Option<(u64, MirGridType)>,
) -> Option<Vec<ServerPacket>> {
    let mut packets = Vec::new();

    match (template.item_type, template.shape) {
        (CRYSTAL_ITEM_TYPE_POTION, CRYSTAL_POTION_SHAPE_SUN_POTION) => {
            restore_current_player_vitals(
                world,
                crystal_item_stat_value(template, CRYSTAL_STAT_HP),
                crystal_item_stat_value(template, CRYSTAL_STAT_MP),
            );
            if let Some(player) = player_entity(world) {
                if let Some(info) = object_health_info_for_entity(world, player, 0) {
                    packets.push(ServerPacket::ObjectHealth { info });
                }
                if crystal_item_stat_value(template, CRYSTAL_STAT_MP) > 0 {
                    if let Some(info) = object_mana_info_for_entity(world, player) {
                        packets.push(ServerPacket::ObjectMana { info });
                    }
                }
            }
            consume_item_at_use_location(world, location);
            Some(prepend_optional_packet(
                use_item_ack(packet_ack, true),
                packets,
            ))
        }
        (CRYSTAL_ITEM_TYPE_POTION, CRYSTAL_POTION_SHAPE_BUFF)
        | (CRYSTAL_ITEM_TYPE_POTION, CRYSTAL_POTION_SHAPE_EXP)
        | (CRYSTAL_ITEM_TYPE_POTION, CRYSTAL_POTION_SHAPE_DROP) => {
            packets.extend(apply_crystal_template_consumable_buffs(world, template));
            if packets.is_empty() {
                return Some(prepend_optional_packet(
                    use_item_ack(packet_ack, false),
                    packets,
                ));
            }
            consume_item_at_use_location(world, location);
            Some(prepend_optional_packet(
                use_item_ack(packet_ack, true),
                packets,
            ))
        }
        (CRYSTAL_ITEM_TYPE_FOOD, _) => {
            let Some(food_item) = ({
                let resources = world.resource::<InventoryResource>();
                item_at_use_location(resources, location)
            }) else {
                return Some(prepend_optional_packet(
                    use_item_ack(packet_ack, false),
                    packets,
                ));
            };
            let Some(repair_packet) = feed_mount_with_crystal_food(world, template, &food_item)
            else {
                return Some(prepend_optional_packet(
                    use_item_ack(packet_ack, false),
                    packets,
                ));
            };
            consume_item_at_use_location(world, location);
            packets.push(hint_chat_key(world, "server.MountFed"));
            packets.push(repair_packet);
            Some(prepend_optional_packet(
                use_item_ack(packet_ack, true),
                packets,
            ))
        }
        (CRYSTAL_ITEM_TYPE_SCROLL, CRYSTAL_SCROLL_SHAPE_TOWN_TELEPORT) => {
            if current_map_disallows_town_teleport(world) {
                packets.push(system_message_key(world, "server.NoTownTeleport"));
                return Some(prepend_optional_packet(
                    use_item_ack(packet_ack, false),
                    packets,
                ));
            }
            consume_item_at_use_location(world, location);
            packets.extend(town_teleport_packets(world));
            Some(prepend_optional_packet(
                use_item_ack(packet_ack, true),
                packets,
            ))
        }
        (CRYSTAL_ITEM_TYPE_SCROLL, CRYSTAL_SCROLL_SHAPE_DUNGEON_ESCAPE)
            if !template.name.ends_with("WarGodOil") =>
        {
            if current_map_disallows_escape(world) {
                packets.push(system_message_key(world, "server.CanNotDungeon"));
                return Some(prepend_optional_packet(
                    use_item_ack(packet_ack, false),
                    packets,
                ));
            }
            let Some(teleport_packets) = crystal_random_same_map_teleport_packets(world, 20) else {
                return Some(prepend_optional_packet(
                    use_item_ack(packet_ack, false),
                    packets,
                ));
            };
            consume_item_at_use_location(world, location);
            Some(prepend_optional_packet(
                use_item_ack(packet_ack, true),
                teleport_packets,
            ))
        }
        (CRYSTAL_ITEM_TYPE_SCROLL, CRYSTAL_SCROLL_SHAPE_RANDOM_TELEPORT) => {
            if current_map_disallows_random_teleport(world) {
                packets.push(system_message_key(world, "server.CanNotRandom"));
                return Some(prepend_optional_packet(
                    use_item_ack(packet_ack, false),
                    packets,
                ));
            }
            let Some(teleport_packets) = crystal_random_same_map_teleport_packets(world, 200)
            else {
                return Some(prepend_optional_packet(
                    use_item_ack(packet_ack, false),
                    packets,
                ));
            };
            consume_item_at_use_location(world, location);
            Some(prepend_optional_packet(
                use_item_ack(packet_ack, true),
                teleport_packets,
            ))
        }
        (CRYSTAL_ITEM_TYPE_SCROLL, shape)
            if shape == CRYSTAL_SCROLL_SHAPE_GT_INVITE
                || shape == CRYSTAL_SCROLL_SHAPE_GT_TELEPORT =>
        {
            consume_item_at_use_location(world, location);
            Some(prepend_optional_packet(
                use_item_ack(packet_ack, true),
                packets,
            ))
        }
        (CRYSTAL_ITEM_TYPE_SCROLL, CRYSTAL_SCROLL_SHAPE_BENEDICTION_OIL) => {
            let Some(outcome) = try_luck_weapon(world) else {
                return Some(prepend_optional_packet(
                    use_item_ack(packet_ack, false),
                    packets,
                ));
            };

            consume_item_at_use_location(world, location);
            match outcome {
                CrystalLuckWeaponOutcome::Changed {
                    refresh_packet,
                    message_key,
                    chat_type,
                } => {
                    packets.push(refresh_packet);
                    packets.push(ServerPacket::Chat {
                        message: localized_text_or_fallback(
                            current_language(world),
                            message_key,
                            message_key,
                        ),
                        chat_type,
                    });
                }
                CrystalLuckWeaponOutcome::NoEffect { message_key } => {
                    packets.push(ServerPacket::Chat {
                        message: localized_text_or_fallback(
                            current_language(world),
                            message_key,
                            message_key,
                        ),
                        chat_type: ChatType::Hint,
                    });
                }
            }
            Some(prepend_optional_packet(
                use_item_ack(packet_ack, true),
                packets,
            ))
        }
        (CRYSTAL_ITEM_TYPE_SCROLL, shape)
            if shape == CRYSTAL_SCROLL_SHAPE_REPAIR_OIL
                || shape == CRYSTAL_SCROLL_SHAPE_WAR_GOD_OIL
                || template.name.ends_with("WarGodOil") =>
        {
            let full_repair =
                shape == CRYSTAL_SCROLL_SHAPE_WAR_GOD_OIL || template.name.ends_with("WarGodOil");
            let Some(repair_packet) = repair_equipped_weapon_with_oil(world, full_repair) else {
                return Some(prepend_optional_packet(
                    use_item_ack(packet_ack, false),
                    packets,
                ));
            };

            consume_item_at_use_location(world, location);
            packets.push(repair_packet);
            packets.push(hint_chat_key(
                world,
                if full_repair {
                    "server.WeaponCompletelyRepaired"
                } else {
                    "server.WeaponPartiallyRepaired"
                },
            ));
            Some(prepend_optional_packet(
                use_item_ack(packet_ack, true),
                packets,
            ))
        }
        (CRYSTAL_ITEM_TYPE_SCROLL, CRYSTAL_SCROLL_SHAPE_MAP_SHOUT) => {
            world
                .resource_mut::<PlayerPermissionResource>()
                .free_map_shout = true;
            consume_item_at_use_location(world, location);
            packets.push(hint_chat_key(world, "server.FreeMapShout"));
            Some(prepend_optional_packet(
                use_item_ack(packet_ack, true),
                packets,
            ))
        }
        (CRYSTAL_ITEM_TYPE_SCROLL, CRYSTAL_SCROLL_SHAPE_SERVER_SHOUT) => {
            world
                .resource_mut::<PlayerPermissionResource>()
                .free_server_shout = true;
            consume_item_at_use_location(world, location);
            packets.push(hint_chat_key(world, "server.FreeServerShout"));
            Some(prepend_optional_packet(
                use_item_ack(packet_ack, true),
                packets,
            ))
        }
        _ => None,
    }
}

pub(super) fn use_item(
    world: &mut World,
    key: &str,
    packet_ack: Option<(u64, MirGridType)>,
) -> Vec<ServerPacket> {
    if let Some(packets) = toggle_mount_ride_from_use_item(world, packet_ack) {
        return packets;
    }

    let location = {
        let resources = world.resource::<InventoryResource>();
        find_use_item_location(resources, key, packet_ack)
    };

    let Some(location) = location else {
        return prepend_optional_packet(use_item_ack(packet_ack, false), Vec::new());
    };

    let Some(item) = item_at_use_location(world.resource::<InventoryResource>(), location) else {
        return prepend_optional_packet(use_item_ack(packet_ack, false), Vec::new());
    };
    let mut packets = Vec::new();
    let item_template = crystal_item_template_for_item_key(&item.key);
    let dynamic_item_template = crystal_item_template_for_dynamic_key(&item.key);
    let is_resurrection_scroll = item_template.as_ref().is_some_and(|template| {
        template.item_type == CRYSTAL_ITEM_TYPE_SCROLL
            && template.shape == CRYSTAL_SCROLL_SHAPE_RESURRECTION
    });
    let is_mystery_water = item_template.as_ref().is_some_and(|template| {
        template.item_type == CRYSTAL_ITEM_TYPE_POTION
            && template.shape == CRYSTAL_POTION_SHAPE_MYSTERY_WATER
    });

    if let Some(template) = item_template.as_ref() {
        match crystal_use_item_eligibility(world, template) {
            CrystalUseItemEligibility::Allowed => {}
            CrystalUseItemEligibility::Rejected(packet) => {
                return prepend_optional_packet(
                    use_item_ack(packet_ack, false),
                    packet.into_iter().collect(),
                );
            }
        }
    }

    if current_player_is_dead(world) && !is_resurrection_scroll {
        return prepend_optional_packet(use_item_ack(packet_ack, false), packets);
    }

    if is_resurrection_scroll {
        if !current_player_is_dead(world) {
            packets.push(hint_chat_key(world, "server.CannotResurrection"));
            return prepend_optional_packet(use_item_ack(packet_ack, false), packets);
        }
        if current_map_disallows_reincarnation(world) {
            packets.push(system_message_key(world, "server.CannotUseOnMap"));
            return prepend_optional_packet(use_item_ack(packet_ack, false), packets);
        }

        consume_item_at_use_location(world, location);
        packets.extend(revive_current_player_from_resurrection_scroll(world));
        return prepend_optional_packet(use_item_ack(packet_ack, true), packets);
    }

    if is_mystery_water {
        if world.resource::<PlayerPermissionResource>().unlock_curse {
            packets.push(hint_chat_key(world, "server.CanAlreadyUnequipCursedItem"));
            return prepend_optional_packet(use_item_ack(packet_ack, false), packets);
        }

        world
            .resource_mut::<PlayerPermissionResource>()
            .unlock_curse = true;
        consume_item_at_use_location(world, location);
        packets.push(hint_chat_key(world, "server.CanNowUnequipCursedItem"));
        return prepend_optional_packet(use_item_ack(packet_ack, true), packets);
    }

    if let Some(template) = dynamic_item_template.as_ref() {
        if template.item_type == CRYSTAL_ITEM_TYPE_POTION
            && template.shape == CRYSTAL_POTION_SHAPE_NORMAL
        {
            if !queue_crystal_normal_potion_restore(world, template) {
                return prepend_optional_packet(use_item_ack(packet_ack, false), packets);
            }
            consume_item_at_use_location(world, location);
            return prepend_optional_packet(use_item_ack(packet_ack, true), packets);
        }
        if let Some(result) =
            use_dynamic_crystal_template_item(world, template, location, packet_ack)
        {
            return result;
        }
    }

    if let Some(template) = item_template.as_ref() {
        if template.item_type == CRYSTAL_ITEM_TYPE_BOOK {
            let Some(skill) = crystal_learn_book_skill(world, template) else {
                return prepend_optional_packet(use_item_ack(packet_ack, false), packets);
            };
            if let Some(magic) = client_magic_for_skill_state(&skill, runtime_tick(world)) {
                packets.push(ServerPacket::NewMagic { magic, hero: false });
            }
            consume_item_at_use_location(world, location);
            return prepend_optional_packet(use_item_ack(packet_ack, true), packets);
        }
    }

    let equip_slot = item.equip_slot.or_else(|| {
        item_template
            .as_ref()
            .and_then(crystal_equipment_slot_for_template)
    });
    if let Some(slot) = equip_slot {
        let UseItemLocation::Inventory(_) = location else {
            return prepend_optional_packet(use_item_ack(packet_ack, false), packets);
        };
        let to = match equipment_slot_index(slot).and_then(|index| i32::try_from(index).ok()) {
            Some(index) => index,
            None => return prepend_optional_packet(use_item_ack(packet_ack, false), packets),
        };
        let Some(result) = try_equip_item(world, MirGridType::Inventory, item_unique_id(&item), to)
        else {
            return prepend_optional_packet(use_item_ack(packet_ack, false), packets);
        };
        packets.extend(result.refresh_packets);
        return prepend_optional_packet(use_item_ack(packet_ack, true), packets);
    }

    if item.key == "town-teleport" {
        if current_map_disallows_town_teleport(world) {
            packets.push(system_message_key(world, "server.NoTownTeleport"));
            return prepend_optional_packet(use_item_ack(packet_ack, false), packets);
        }
        consume_item_at_use_location(world, location);
        packets.extend(town_teleport_packets(world));
        return prepend_optional_packet(use_item_ack(packet_ack, true), packets);
    }

    if item.key == "repair-powder" {
        let repair_packets = repair_equipped_durability(world);
        let repaired_count = repair_packets.len();
        if repaired_count == 0 {
            return prepend_optional_packet(use_item_ack(packet_ack, false), packets);
        }

        consume_item_at_use_location(world, location);
        packets.extend(repair_packets);
        return prepend_optional_packet(use_item_ack(packet_ack, true), packets);
    }

    if item.key == "benediction-oil" {
        let Some(outcome) = try_luck_weapon(world) else {
            return prepend_optional_packet(use_item_ack(packet_ack, false), packets);
        };

        consume_item_at_use_location(world, location);
        match outcome {
            CrystalLuckWeaponOutcome::Changed {
                refresh_packet,
                message_key,
                chat_type,
            } => {
                packets.push(refresh_packet);
                packets.push(ServerPacket::Chat {
                    message: localized_text_or_fallback(
                        current_language(world),
                        message_key,
                        message_key,
                    ),
                    chat_type,
                });
            }
            CrystalLuckWeaponOutcome::NoEffect { message_key } => {
                packets.push(ServerPacket::Chat {
                    message: localized_text_or_fallback(
                        current_language(world),
                        message_key,
                        message_key,
                    ),
                    chat_type: ChatType::Hint,
                });
            }
        }
        return prepend_optional_packet(use_item_ack(packet_ack, true), packets);
    }

    if item.key == "repair-oil" || item.key == "war-god-oil" {
        let full_repair = item.key == "war-god-oil";
        let Some(repair_packet) = repair_equipped_weapon_with_oil(world, full_repair) else {
            return prepend_optional_packet(use_item_ack(packet_ack, false), packets);
        };

        consume_item_at_use_location(world, location);
        packets.push(repair_packet);
        packets.push(hint_chat_key(
            world,
            if full_repair {
                "server.WeaponCompletelyRepaired"
            } else {
                "server.WeaponPartiallyRepaired"
            },
        ));
        return prepend_optional_packet(use_item_ack(packet_ack, true), packets);
    }

    if let Some(credit) = crystal_credit_value_for_item(&item) {
        consume_item_at_use_location(world, location);
        if let Some(packet) = gain_credit(world, credit) {
            packets.push(packet);
        }
        packets.push(hint_chat_key_args(
            world,
            "server.CreditsAddedToAccount",
            [credit.to_string()],
        ));
        return prepend_optional_packet(use_item_ack(packet_ack, true), packets);
    }

    if item.heal_hp > 0 || item.heal_mp > 0 {
        if current_map_disallows_drug(world) {
            packets.push(system_message_key(world, "server.YouCannotUsePotionsHere"));
            return prepend_optional_packet(use_item_ack(packet_ack, false), packets);
        }
        queue_crystal_normal_potion_restore_amounts(
            world,
            item.heal_hp.max(0),
            item.heal_mp.max(0),
        );
        consume_item_at_use_location(world, location);
        return prepend_optional_packet(use_item_ack(packet_ack, true), packets);
    }

    prepend_optional_packet(use_item_ack(packet_ack, false), packets)
}

pub(super) enum CrystalUseItemEligibility {
    Allowed,
    Rejected(Option<ServerPacket>),
}

pub(super) fn crystal_required_class_flag(class: MirClass) -> u8 {
    match class {
        MirClass::Warrior => CRYSTAL_REQUIRED_CLASS_WARRIOR,
        MirClass::Wizard => CRYSTAL_REQUIRED_CLASS_WIZARD,
        MirClass::Taoist => CRYSTAL_REQUIRED_CLASS_TAOIST,
        MirClass::Assassin => CRYSTAL_REQUIRED_CLASS_ASSASSIN,
        MirClass::Archer => CRYSTAL_REQUIRED_CLASS_ARCHER,
    }
}

pub(super) fn crystal_required_gender_flag(gender: MirGender) -> u8 {
    match gender {
        MirGender::Male => CRYSTAL_REQUIRED_GENDER_MALE,
        MirGender::Female => CRYSTAL_REQUIRED_GENDER_FEMALE,
    }
}

pub(super) fn current_equipment_required_stat(item: &EquipmentState, stat: u8) -> i32 {
    if item.is_broken() {
        return 0;
    }

    match stat {
        CRYSTAL_STAT_MAX_AC => item.total_defence(),
        CRYSTAL_STAT_MAX_DC => item.total_attack(),
        _ => user_item_stat_total(&item.added_stats, stat),
    }
}

pub(super) fn current_buff_required_stat(buff: &BuffState, stat: u8) -> i32 {
    match stat {
        CRYSTAL_STAT_MAX_AC => buff_defence_bonus(buff),
        CRYSTAL_STAT_MAX_DC => buff_attack_bonus(buff),
        _ => user_item_stat_total(&buff.stats, stat),
    }
}

pub(super) fn current_player_required_stat_total(
    resources: &InventoryResource,
    buffs: &BuffResource,
    stat: u8,
) -> i32 {
    resources
        .equipment_items
        .iter()
        .map(|item| current_equipment_required_stat(item, stat))
        .sum::<i32>()
        + buffs
            .buffs
            .iter()
            .map(|buff| current_buff_required_stat(buff, stat))
            .sum::<i32>()
}

pub(super) fn crystal_item_requirement_rejection_key(
    world: &World,
    resources: &InventoryResource,
    template: &CrystalItemTemplate,
) -> Option<&'static str> {
    let character = world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()?;

    if template.required_gender & crystal_required_gender_flag(character.gender) == 0 {
        return Some(match character.gender {
            MirGender::Male => "server.NotFemale",
            MirGender::Female => "server.NotMale",
        });
    }

    if template.required_class & crystal_required_class_flag(character.class) == 0 {
        return Some(match character.class {
            MirClass::Warrior => "server.WarriorsCannotUseItem",
            MirClass::Wizard => "server.WizardsCannotUseItem",
            MirClass::Taoist => "server.TaoistsCannotUseItem",
            MirClass::Assassin => "server.AssassinsCannotUseItem",
            MirClass::Archer => "server.ArchersCannotUseItem",
        });
    }

    let required_amount = i32::from(template.required_amount);
    let buffs = world.resource::<BuffResource>();
    match template.required_type {
        CRYSTAL_REQUIRED_TYPE_LEVEL if character.level < u16::from(template.required_amount) => {
            Some("server.LowLevel")
        }
        CRYSTAL_REQUIRED_TYPE_MAX_AC
            if current_player_required_stat_total(resources, buffs, CRYSTAL_STAT_MAX_AC)
                < required_amount =>
        {
            Some("server.YouNotEnoughAC")
        }
        CRYSTAL_REQUIRED_TYPE_MAX_MAC
            if current_player_required_stat_total(resources, buffs, CRYSTAL_STAT_MAX_MAC)
                < required_amount =>
        {
            Some("server.YouNotEnoughMAC")
        }
        CRYSTAL_REQUIRED_TYPE_MAX_DC
            if current_player_required_stat_total(resources, buffs, CRYSTAL_STAT_MAX_DC)
                < required_amount =>
        {
            Some("server.LowDC")
        }
        CRYSTAL_REQUIRED_TYPE_MAX_MC
            if current_player_required_stat_total(resources, buffs, CRYSTAL_STAT_MAX_MC)
                < required_amount =>
        {
            Some("server.LowMC")
        }
        CRYSTAL_REQUIRED_TYPE_MAX_SC
            if current_player_required_stat_total(resources, buffs, CRYSTAL_STAT_MAX_SC)
                < required_amount =>
        {
            Some("server.LowSC")
        }
        CRYSTAL_REQUIRED_TYPE_MAX_LEVEL
            if character.level > u16::from(template.required_amount) =>
        {
            Some("server.YouExceededMaxLevel")
        }
        CRYSTAL_REQUIRED_TYPE_MIN_AC
            if current_player_required_stat_total(resources, buffs, CRYSTAL_STAT_MIN_AC)
                < required_amount =>
        {
            Some("server.YouNoBaseAC")
        }
        CRYSTAL_REQUIRED_TYPE_MIN_MAC
            if current_player_required_stat_total(resources, buffs, CRYSTAL_STAT_MIN_MAC)
                < required_amount =>
        {
            Some("server.YouNoBaseMAC")
        }
        CRYSTAL_REQUIRED_TYPE_MIN_DC
            if current_player_required_stat_total(resources, buffs, CRYSTAL_STAT_MIN_DC)
                < required_amount =>
        {
            Some("server.YouNoBaseDC")
        }
        CRYSTAL_REQUIRED_TYPE_MIN_MC
            if current_player_required_stat_total(resources, buffs, CRYSTAL_STAT_MIN_MC)
                < required_amount =>
        {
            Some("server.YouNoBaseMC")
        }
        CRYSTAL_REQUIRED_TYPE_MIN_SC
            if current_player_required_stat_total(resources, buffs, CRYSTAL_STAT_MIN_SC)
                < required_amount =>
        {
            Some("server.YouNoBaseSC")
        }
        _ => None,
    }
}

pub(super) fn crystal_skill_is_known(skills: &SkillResource, skill_key: &str) -> bool {
    skills
        .skills
        .iter()
        .any(|known| known.key.eq_ignore_ascii_case(skill_key))
}

pub(super) fn crystal_use_item_eligibility(
    world: &World,
    template: &CrystalItemTemplate,
) -> CrystalUseItemEligibility {
    let resources = world.resource::<InventoryResource>();
    let skills = world.resource::<SkillResource>();
    if let Some(key) = crystal_item_requirement_rejection_key(world, resources, template) {
        return CrystalUseItemEligibility::Rejected(Some(super::session::system_message_key(
            world, key,
        )));
    }

    if template.item_type == CRYSTAL_ITEM_TYPE_BOOK {
        let Some(skill) = crystal_book_skill_state(template) else {
            return CrystalUseItemEligibility::Rejected(None);
        };
        let Some(character) = world
            .resource::<SessionResource>()
            .selected_character
            .as_ref()
        else {
            return CrystalUseItemEligibility::Rejected(None);
        };
        if !world
            .resource::<RuntimeConfigResource>()
            .config
            .skill_is_allowed(&skill.key, character.class, character.level)
        {
            return CrystalUseItemEligibility::Rejected(Some(super::session::system_message(
                "This skill is unavailable in the active content profile.",
            )));
        }
    }

    if template.item_type == CRYSTAL_ITEM_TYPE_BOOK
        && crystal_book_skill_state(template)
            .as_ref()
            .is_some_and(|skill| crystal_skill_is_known(skills, &skill.key))
    {
        return CrystalUseItemEligibility::Rejected(None);
    }

    if template.item_type == CRYSTAL_ITEM_TYPE_POTION && current_map_disallows_drug(world) {
        return CrystalUseItemEligibility::Rejected(Some(super::session::system_message_key(
            world,
            "server.YouCannotUsePotionsHere",
        )));
    }

    CrystalUseItemEligibility::Allowed
}

pub(super) fn crystal_learn_book_skill(
    world: &mut World,
    template: &CrystalItemTemplate,
) -> Option<SkillState> {
    let Some(skill) = crystal_book_skill_state(template) else {
        return None;
    };

    let mut skills = world.resource_mut::<SkillResource>();
    if crystal_skill_is_known(&skills, &skill.key) {
        return None;
    }
    skills.skills.push(skill.clone());
    Some(skill)
}

fn hero_inventory_item_is_broken(item: &ItemState) -> bool {
    item.durability_max.unwrap_or_default() > 0 && item.durability_current.unwrap_or_default() == 0
}

fn hero_inventory_requirement_stat(item: &ItemState, stat: u8) -> i32 {
    if item
        .equip_slot
        .or_else(|| crystal_equipment_slot_for_item_key(&item.key))
        .is_none()
        || hero_inventory_item_is_broken(item)
    {
        return 0;
    }

    let modeled_base = match stat {
        CRYSTAL_STAT_MAX_AC => item.defence,
        CRYSTAL_STAT_MAX_DC => item.attack,
        _ => 0,
    };
    let template_base = crystal_item_template_for_item_key(&item.key)
        .map(|template| crystal_item_stat_value(&template, stat))
        .unwrap_or_default();
    let base = if modeled_base != 0 {
        modeled_base
    } else {
        template_base
    };
    base.saturating_add(crystal_item_added_stat_value(item, stat))
}

fn current_hero_required_stat_total(world: &World, stat: u8) -> i32 {
    world
        .resource::<HeroInventoryResource>()
        .items
        .iter()
        .map(|item| hero_inventory_requirement_stat(item, stat))
        .sum()
}

fn crystal_hero_item_requirement_rejected(
    world: &World,
    hero_level: u16,
    hero_class: MirClass,
    hero_gender: MirGender,
    template: &CrystalItemTemplate,
) -> bool {
    if template.required_gender & crystal_required_gender_flag(hero_gender) == 0 {
        return true;
    }
    if template.required_class & crystal_required_class_flag(hero_class) == 0 {
        return true;
    }

    let required_amount = i32::from(template.required_amount);
    match template.required_type {
        CRYSTAL_REQUIRED_TYPE_LEVEL => hero_level < u16::from(template.required_amount),
        CRYSTAL_REQUIRED_TYPE_MAX_AC => {
            current_hero_required_stat_total(world, CRYSTAL_STAT_MAX_AC) < required_amount
        }
        CRYSTAL_REQUIRED_TYPE_MAX_MAC => {
            current_hero_required_stat_total(world, CRYSTAL_STAT_MAX_MAC) < required_amount
        }
        CRYSTAL_REQUIRED_TYPE_MAX_DC => {
            current_hero_required_stat_total(world, CRYSTAL_STAT_MAX_DC) < required_amount
        }
        CRYSTAL_REQUIRED_TYPE_MAX_MC => {
            current_hero_required_stat_total(world, CRYSTAL_STAT_MAX_MC) < required_amount
        }
        CRYSTAL_REQUIRED_TYPE_MAX_SC => {
            current_hero_required_stat_total(world, CRYSTAL_STAT_MAX_SC) < required_amount
        }
        CRYSTAL_REQUIRED_TYPE_MAX_LEVEL => hero_level > u16::from(template.required_amount),
        CRYSTAL_REQUIRED_TYPE_MIN_AC => {
            current_hero_required_stat_total(world, CRYSTAL_STAT_MIN_AC) < required_amount
        }
        CRYSTAL_REQUIRED_TYPE_MIN_MAC => {
            current_hero_required_stat_total(world, CRYSTAL_STAT_MIN_MAC) < required_amount
        }
        CRYSTAL_REQUIRED_TYPE_MIN_DC => {
            current_hero_required_stat_total(world, CRYSTAL_STAT_MIN_DC) < required_amount
        }
        CRYSTAL_REQUIRED_TYPE_MIN_MC => {
            current_hero_required_stat_total(world, CRYSTAL_STAT_MIN_MC) < required_amount
        }
        CRYSTAL_REQUIRED_TYPE_MIN_SC => {
            current_hero_required_stat_total(world, CRYSTAL_STAT_MIN_SC) < required_amount
        }
        _ => false,
    }
}

pub(super) fn crystal_learn_hero_book_magic(
    world: &mut World,
    template: &CrystalItemTemplate,
) -> Option<ClientMagic> {
    if template.item_type != CRYSTAL_ITEM_TYPE_BOOK {
        return None;
    }
    let hero = world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .hero
        .clone()?;
    if crystal_hero_item_requirement_rejected(world, hero.level, hero.class, hero.gender, template)
    {
        return None;
    }
    let skill = crystal_book_skill_state(template)?;
    let magic = client_magic_for_skill_state(&skill, runtime_tick(world))?;
    let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
    let learned_magics = &mut stage5.stage5_systems.hero_learned_magics;
    if learned_magics
        .iter()
        .any(|learned| learned.spell == magic.spell)
    {
        return None;
    }
    learned_magics.push(Stage5HeroMagicState {
        spell: magic.spell,
        level: skill.level,
        key: 0,
        experience: skill.experience,
    });
    Some(magic)
}

pub(super) fn crystal_item_template_for_item_key(key: &str) -> Option<CrystalItemTemplate> {
    if let Some(template) = crystal_item_template_for_dynamic_key(key) {
        return Some(template);
    }
    crystal_item_name_for_item_key(key).and_then(crystal_item_by_name)
}

/// Whether an item is a Crystal `ItemType.Socket` insert (the gem that goes into
/// an item's socket via `EquipSlotItem`).
pub(super) fn item_is_socket_type(item: &ItemState) -> bool {
    crystal_item_template_for_item_key(&item.key)
        .map(|template| template.item_type == CRYSTAL_ITEM_TYPE_SOCKET)
        .unwrap_or(false)
}

pub(super) fn crystal_item_template_for_dynamic_key(key: &str) -> Option<CrystalItemTemplate> {
    key.strip_prefix("crystal-item-")
        .and_then(|index| index.parse::<i32>().ok())
        .and_then(crystal_item_by_index)
}

pub(super) fn crystal_item_key_for_template(template: &CrystalItemTemplate) -> String {
    format!("crystal-item-{}", template.item_index)
}

pub(super) fn crystal_equipment_slot_for_template(
    template: &CrystalItemTemplate,
) -> Option<EquipmentSlot> {
    match template.item_type {
        CRYSTAL_ITEM_TYPE_WEAPON => Some(EquipmentSlot::Weapon),
        CRYSTAL_ITEM_TYPE_ARMOUR => Some(EquipmentSlot::Armour),
        CRYSTAL_ITEM_TYPE_HELMET => Some(EquipmentSlot::Helmet),
        CRYSTAL_ITEM_TYPE_NECKLACE => Some(EquipmentSlot::Necklace),
        CRYSTAL_ITEM_TYPE_BRACELET => Some(EquipmentSlot::BraceletLeft),
        CRYSTAL_ITEM_TYPE_RING => Some(EquipmentSlot::RingLeft),
        CRYSTAL_ITEM_TYPE_AMULET => Some(EquipmentSlot::Amulet),
        CRYSTAL_ITEM_TYPE_BELT => Some(EquipmentSlot::Belt),
        CRYSTAL_ITEM_TYPE_BOOTS => Some(EquipmentSlot::Boots),
        CRYSTAL_ITEM_TYPE_STONE => Some(EquipmentSlot::Stone),
        CRYSTAL_ITEM_TYPE_TORCH => Some(EquipmentSlot::Torch),
        CRYSTAL_ITEM_TYPE_MOUNT => Some(EquipmentSlot::Mount),
        _ => None,
    }
}

pub(super) fn crystal_template_can_equip_to_slot(
    template: &CrystalItemTemplate,
    target_slot: EquipmentSlot,
) -> bool {
    match target_slot {
        EquipmentSlot::Weapon => template.item_type == CRYSTAL_ITEM_TYPE_WEAPON,
        EquipmentSlot::Armour => template.item_type == CRYSTAL_ITEM_TYPE_ARMOUR,
        EquipmentSlot::Helmet => template.item_type == CRYSTAL_ITEM_TYPE_HELMET,
        EquipmentSlot::Torch => template.item_type == CRYSTAL_ITEM_TYPE_TORCH,
        EquipmentSlot::Necklace => template.item_type == CRYSTAL_ITEM_TYPE_NECKLACE,
        EquipmentSlot::BraceletLeft => template.item_type == CRYSTAL_ITEM_TYPE_BRACELET,
        EquipmentSlot::BraceletRight => {
            template.item_type == CRYSTAL_ITEM_TYPE_BRACELET
                || template.item_type == CRYSTAL_ITEM_TYPE_AMULET
        }
        EquipmentSlot::RingLeft | EquipmentSlot::RingRight => {
            template.item_type == CRYSTAL_ITEM_TYPE_RING
        }
        EquipmentSlot::Amulet => template.item_type == CRYSTAL_ITEM_TYPE_AMULET,
        EquipmentSlot::Belt => template.item_type == CRYSTAL_ITEM_TYPE_BELT,
        EquipmentSlot::Boots => template.item_type == CRYSTAL_ITEM_TYPE_BOOTS,
        EquipmentSlot::Stone => template.item_type == CRYSTAL_ITEM_TYPE_STONE,
        EquipmentSlot::Mount => template.item_type == CRYSTAL_ITEM_TYPE_MOUNT,
    }
}

pub(super) fn item_state_can_equip_to_slot(item: &ItemState, target_slot: EquipmentSlot) -> bool {
    if let Some(template) = crystal_item_template_for_item_key(&item.key) {
        return crystal_template_can_equip_to_slot(&template, target_slot);
    }

    item.equip_slot.is_some_and(|slot| slot == target_slot)
}

pub(super) fn crystal_equipment_slot_for_item_key(key: &str) -> Option<EquipmentSlot> {
    crystal_item_template_for_item_key(key)
        .and_then(|template| crystal_equipment_slot_for_template(&template))
}

pub(super) fn crystal_stack_size_for_item_key(key: &str) -> u32 {
    crystal_item_template_for_item_key(key)
        .map(|template| u32::from(template.stack_size.max(1)))
        .unwrap_or(u32::from(u16::MAX))
}

pub(super) fn crystal_belt_slot_range_for_item_key(key: &str) -> Option<(u8, u8)> {
    let template = crystal_item_template_for_item_key(key)?;
    match template.item_type {
        13 | 17 => Some((0, 4)),
        21 if template.effect == 1 => Some((0, 4)),
        8 => Some((4, 6)),
        _ => None,
    }
}

pub(super) fn crystal_item_bind_for_item_key(key: &str) -> i16 {
    crystal_item_template_for_item_key(key)
        .map(|template| template.bind)
        .unwrap_or(0)
}

pub(super) fn crystal_item_has_bind_flag(key: &str, flag: i16) -> bool {
    crystal_item_bind_for_item_key(key) & flag != 0
}

pub(super) fn item_has_rental_bind_flag(item: &ItemState, flag: i16) -> bool {
    item.rental_binding_flags & flag != 0
}

pub(super) fn item_has_crystal_or_rental_bind_flag(item: &ItemState, flag: i16) -> bool {
    crystal_item_has_bind_flag(&item.key, flag) || item_has_rental_bind_flag(item, flag)
}

pub(super) fn equipment_has_rental_bind_flag(item: &EquipmentState, flag: i16) -> bool {
    item.rental_binding_flags & flag != 0
}

pub(super) fn equipment_has_crystal_or_rental_bind_flag(item: &EquipmentState, flag: i16) -> bool {
    crystal_item_has_bind_flag(&item.key, flag) || equipment_has_rental_bind_flag(item, flag)
}

pub(super) fn crystal_credit_value_for_item(item: &ItemState) -> Option<u32> {
    let template = crystal_item_template_for_item_key(&item.key)?;
    (template.item_type == 17 && template.name.starts_with("CreditToken") && template.price > 0)
        .then_some(template.price)
}

pub(super) fn crystal_item_name_for_item_key(key: &str) -> Option<&'static str> {
    match key {
        "red-potion" | "belt-red-potion" | "stored-red-potion" => Some("(HP)DrugSmall"),
        "blue-potion" | "belt-blue-potion" => Some("(MP)DrugSmall"),
        "bronze-helmet" | "stored-bronze-helmet" | "bronze-helmet-equipment" => {
            Some("BronzeHelmet")
        }
        "wooden-sword" => Some("WoodenSword"),
        "dagger" => Some("Dagger"),
        "leather-armour" => Some("LightLeatherArmour(M)"),
        "town-teleport" => Some("TownTeleport"),
        "benediction-oil" => Some("BenedictionOil"),
        "repair-oil" => Some("RepairOil"),
        "war-god-oil" => Some("WarGodOil"),
        "credit-token-1" => Some("CreditToken1"),
        "credit-token-2" => Some("CreditToken2"),
        "credit-token-3" => Some("CreditToken3"),
        "credit-token-4" => Some("CreditToken4"),
        "credit-token-5" => Some("CreditToken5"),
        "credit-token-6" => Some("CreditToken6"),
        "credit-token-7" => Some("CreditToken7"),
        "credit-token-8" => Some("CreditToken8"),
        _ => None,
    }
}

impl SimulationSession {
    pub fn use_item(&mut self, key: &str) -> Vec<ServerPacket> {
        let packets = self.use_item_impl(key);
        self.finalize_packets(packets)
    }

    pub fn drop_item(&mut self, key: &str) -> Vec<ServerPacket> {
        let packets = self.drop_item_impl(key);
        self.finalize_packets(packets)
    }
    pub(super) fn use_item_impl(&mut self, key: &str) -> Vec<ServerPacket> {
        if !is_in_world(self.app.world()) {
            return Vec::new();
        }

        use_item(self.app.world_mut(), key, None)
    }

    pub(super) fn drop_item_impl(&mut self, key: &str) -> Vec<ServerPacket> {
        if !is_in_world(self.app.world()) {
            return Vec::new();
        }

        let item_reference = {
            let resources = self.app.world().resource::<InventoryResource>();
            resources
                .inventory_items
                .iter()
                .find(|item| {
                    item.key == key
                        && matches!(item.container, ItemContainer::Bag1 | ItemContainer::Bag2)
                })
                .map(|item| {
                    (
                        item_unique_id(item),
                        u16::try_from(item.quantity).unwrap_or(u16::MAX),
                    )
                })
        };

        match item_reference {
            Some((unique_id, count)) => {
                drop_item_packet(self.app.world_mut(), unique_id, count, false)
            }
            None => Vec::new(),
        }
    }

    pub(super) fn equip_item_packet_impl(
        &mut self,
        grid: MirGridType,
        unique_id: u64,
        to: i32,
    ) -> Vec<ServerPacket> {
        equip_item_impl(self.app.world_mut(), grid, unique_id, to)
    }
}
