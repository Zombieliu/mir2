use std::{collections::HashSet, error::Error, fmt, sync::OnceLock};

use serde::{Deserialize, Serialize};

use crate::config::{
    EquipmentSlot, ItemContainer, ItemGrade, Stage5HeroMagicState, WorldItemSnapshot,
};
use bevy_ecs::prelude::World;
use mir2_game_data::{
    crystal_item_by_index, crystal_item_by_name, crystal_item_manifest, crystal_recipes,
    localized_text_or_fallback, CrystalItemTemplate, LanguageCode,
};
use mir2_protocol::{
    ChatType, ClientMagic, ItemInfo, MirClass, MirGender, MirGridType, ServerPacket, UserItem,
    UserItemExpireInfo, UserItemRentalInformation, UserItemSealedInfo, UserItemStat,
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
    repair_equipped_weapon_with_oil, slugify_name, toggle_mount_ride_from_use_item, try_equip_item,
    try_luck_weapon, CrystalLuckWeaponOutcome, EquipmentState,
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
use super::npc::crystal_sell_value_for_item;
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
use super::skills::{
    client_magic_for_skill_state, crystal_book_skill_state, crystal_magic_for_skill_key, SkillState,
};

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
    /// Protocol identity that is not yet modeled as live ItemState fields.
    ///
    /// This is deliberately a serde-default-compatible sidecar. It lets old
    /// saves decode as before while keeping UserItem-only identity through a
    /// save/reload boundary. Live ItemState fields remain authoritative when
    /// the protocol item is rebuilt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) user_item_metadata: Option<ItemStateUserItemMetadata>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ItemStateUserItemMetadata {
    /// Exact protocol index. None identifies a pre-sidecar save and keeps the
    /// legacy template-derived fallback available for that save only.
    #[serde(default)]
    pub(super) item_index: Option<i32>,
    #[serde(default)]
    pub(super) awake_type: u8,
    #[serde(default)]
    pub(super) awake_values: Vec<u8>,
    #[serde(default)]
    pub(super) refined_value: u8,
    #[serde(default)]
    pub(super) refine_added: u8,
    #[serde(default)]
    pub(super) refine_success_chance: i32,
    #[serde(default = "default_wedding_ring")]
    pub(super) wedding_ring: i32,
    #[serde(default)]
    pub(super) expire_info: Option<UserItemExpireInfo>,
    /// Preserve protocol Some(default) separately from the live flattened
    /// rental fields, whose all-default value otherwise looks like None.
    #[serde(default)]
    pub(super) rental_information: Option<UserItemRentalInformation>,
    /// Preserve protocol Some even when both timestamps are zero.
    #[serde(default)]
    pub(super) sealed_info: Option<UserItemSealedInfo>,
    /// Legacy Phase 1 sidecars stored the complete recursive slot tree here.
    /// Newly hydrated carriers use captured_socket_positions plus live socketed
    /// children and leave this empty to avoid two recursive truth sources.
    #[serde(default)]
    pub(super) slots: Vec<Option<UserItem>>,
    #[serde(default)]
    pub(super) is_shop_item: bool,
    #[serde(default)]
    pub(super) gm_made: bool,
    /// Legacy marker retained for serde compatibility.
    #[serde(default)]
    pub(super) live_socketed_at_capture: bool,
    /// True when protocol slots were converted into bounded live ItemState
    /// children and may be reconciled by position/identity.
    #[serde(default)]
    pub(super) socket_layout_hydrated: bool,
    /// Original direct-slot identity map. Empty slots remain explicit.
    #[serde(default)]
    pub(super) captured_socket_positions: Option<Vec<Option<CapturedSocketIdentity>>>,
    /// Original slot occupied by this embedded item inside its parent.
    #[serde(default)]
    pub(super) captured_socket_position: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct CapturedSocketIdentity {
    pub(super) unique_id: u64,
    pub(super) item_index: i32,
}

impl CapturedSocketIdentity {
    fn from_user_item(item: &UserItem) -> Self {
        Self {
            unique_id: item.unique_id,
            item_index: item.item_index,
        }
    }
}

fn default_wedding_ring() -> i32 {
    -1
}

impl ItemStateUserItemMetadata {
    fn from_hydrated_user_item(
        item: &UserItem,
        captured_socket_positions: Vec<Option<CapturedSocketIdentity>>,
        captured_socket_position: Option<u8>,
    ) -> Self {
        Self {
            item_index: Some(item.item_index),
            awake_type: item.awake_type,
            awake_values: item.awake_values.clone(),
            refined_value: item.refined_value,
            refine_added: item.refine_added,
            refine_success_chance: item.refine_success_chance,
            wedding_ring: item.wedding_ring,
            expire_info: item.expire_info.clone(),
            rental_information: item.rental_information.clone(),
            sealed_info: item.sealed_info.clone(),
            slots: Vec::new(),
            is_shop_item: item.is_shop_item,
            gm_made: item.gm_made,
            live_socketed_at_capture: true,
            socket_layout_hydrated: true,
            captured_socket_positions: Some(captured_socket_positions),
            captured_socket_position,
        }
    }
}
/// Operational limits for the recursive Crystal UserItem carrier.
///
/// The wire format encodes all three collection counts as signed i32. These
/// stricter limits keep conversion work and saved sidecars bounded well before
/// that protocol ceiling. They are explicit so callers and tests can reason
/// about the accepted identity envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct UserItemCarrierBudget {
    pub(super) max_depth: usize,
    pub(super) max_total_nodes: usize,
    pub(super) max_slots_per_item: usize,
    pub(super) max_added_stats_per_item: usize,
    pub(super) max_awake_values_per_item: usize,
}

impl Default for UserItemCarrierBudget {
    fn default() -> Self {
        Self {
            max_depth: 8,
            max_total_nodes: 256,
            max_slots_per_item: 64,
            max_added_stats_per_item: 256,
            max_awake_values_per_item: 64,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum UserItemCarrierError {
    DepthExceeded {
        depth: usize,
        max: usize,
    },
    TotalNodesExceeded {
        nodes: usize,
        max: usize,
    },
    SlotsExceeded {
        count: usize,
        max: usize,
    },
    AddedStatsExceeded {
        count: usize,
        max: usize,
    },
    AwakeValuesExceeded {
        count: usize,
        max: usize,
    },
    QuantityExceeded {
        quantity: u32,
        max: u16,
    },
    CommittedQuantityOutOfRange {
        item_index: i32,
        quantity: u32,
        max: u32,
    },
    ProtocolCountExceeded {
        field: &'static str,
        count: usize,
    },
    SocketSlotWidthExceeded {
        count: usize,
    },
    UnknownItemIndex {
        item_index: i32,
    },
    AmbiguousItemIndex {
        item_index: i32,
    },
    UnknownItemKey {
        key: String,
    },
    MissingExactItemIndex {
        key: String,
    },
    ConflictingItemIdentity {
        key: String,
        key_item_index: i32,
        exact_item_index: i32,
    },
    AmbiguousSocketIdentity {
        unique_id: u64,
        item_index: i32,
    },
    SocketMapping {
        reason: &'static str,
    },
}

impl fmt::Display for UserItemCarrierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DepthExceeded { depth, max } => {
                write!(
                    formatter,
                    "UserItem depth {depth} exceeds carrier limit {max}"
                )
            }
            Self::TotalNodesExceeded { nodes, max } => write!(
                formatter,
                "UserItem node count {nodes} exceeds carrier limit {max}"
            ),
            Self::SlotsExceeded { count, max } => write!(
                formatter,
                "UserItem slot count {count} exceeds per-item carrier limit {max}"
            ),
            Self::AddedStatsExceeded { count, max } => write!(
                formatter,
                "UserItem added-stat count {count} exceeds per-item carrier limit {max}"
            ),
            Self::AwakeValuesExceeded { count, max } => write!(
                formatter,
                "UserItem awake-value count {count} exceeds per-item carrier limit {max}"
            ),
            Self::QuantityExceeded { quantity, max } => write!(
                formatter,
                "ItemState quantity {quantity} exceeds UserItem u16 limit {max}"
            ),
            Self::CommittedQuantityOutOfRange {
                item_index,
                quantity,
                max,
            } => write!(
                formatter,
                "committed UserItem index {item_index} quantity {quantity} is outside Crystal stack range 1..={max}"
            ),
            Self::ProtocolCountExceeded { field, count } => write!(
                formatter,
                "UserItem {field} count {count} exceeds the signed i32 protocol count"
            ),
            Self::SocketSlotWidthExceeded { count } => write!(
                formatter,
                "UserItem slot count {count} cannot fit ItemState's u8 socket width"
            ),
            Self::UnknownItemIndex { item_index } => {
                write!(formatter, "UserItem index {item_index} has no Crystal template")
            }
            Self::AmbiguousItemIndex { item_index } => write!(
                formatter,
                "UserItem index {item_index} resolves to multiple Crystal templates"
            ),
            Self::UnknownItemKey { key } => {
                write!(formatter, "ItemState key {key:?} has no Crystal template")
            }
            Self::MissingExactItemIndex { key } => write!(
                formatter,
                "exact ItemState carrier {key:?} is missing metadata.item_index"
            ),
            Self::ConflictingItemIdentity {
                key,
                key_item_index,
                exact_item_index,
            } => write!(
                formatter,
                "ItemState key {key:?} resolves to Crystal index {key_item_index}, but exact metadata carries index {exact_item_index}"
            ),
            Self::AmbiguousSocketIdentity {
                unique_id,
                item_index,
            } => write!(
                formatter,
                "socket identity unique_id={unique_id} item_index={item_index} is ambiguous"
            ),
            Self::SocketMapping { reason } => {
                write!(
                    formatter,
                    "UserItem live socket mapping is invalid: {reason}"
                )
            }
        }
    }
}

impl Error for UserItemCarrierError {}

fn ambiguous_crystal_item_indexes() -> &'static HashSet<i32> {
    static AMBIGUOUS: OnceLock<HashSet<i32>> = OnceLock::new();
    AMBIGUOUS.get_or_init(|| {
        let mut seen = HashSet::new();
        let mut ambiguous = HashSet::new();
        for template in crystal_item_manifest().items {
            if !seen.insert(template.item_index) {
                ambiguous.insert(template.item_index);
            }
        }
        ambiguous
    })
}

fn unique_crystal_item_by_index(
    item_index: i32,
) -> Result<CrystalItemTemplate, UserItemCarrierError> {
    let template = crystal_item_by_index(item_index)
        .ok_or(UserItemCarrierError::UnknownItemIndex { item_index })?;
    if ambiguous_crystal_item_indexes().contains(&item_index) {
        return Err(UserItemCarrierError::AmbiguousItemIndex { item_index });
    }
    Ok(template)
}

fn crystal_template_for_item_state_key(
    item: &ItemState,
) -> Result<CrystalItemTemplate, UserItemCarrierError> {
    let template = crystal_item_template_for_item_key(&item.key).ok_or_else(|| {
        UserItemCarrierError::UnknownItemKey {
            key: item.key.clone(),
        }
    })?;
    unique_crystal_item_by_index(template.item_index)
}

fn exact_item_index_for_item_state(item: &ItemState) -> Result<i32, UserItemCarrierError> {
    let key_template = crystal_template_for_item_state_key(item)?;
    let Some(metadata) = item.user_item_metadata.as_ref() else {
        return Ok(key_template.item_index);
    };
    let exact_item_index =
        metadata
            .item_index
            .ok_or_else(|| UserItemCarrierError::MissingExactItemIndex {
                key: item.key.clone(),
            })?;
    let exact_template = unique_crystal_item_by_index(exact_item_index)?;
    if exact_template.item_index != key_template.item_index {
        return Err(UserItemCarrierError::ConflictingItemIdentity {
            key: item.key.clone(),
            key_item_index: key_template.item_index,
            exact_item_index,
        });
    }
    Ok(exact_item_index)
}

fn validate_item_state_key_for_exact_index(
    item: &ItemState,
    exact_item_index: i32,
) -> Result<(), UserItemCarrierError> {
    let key_template = crystal_template_for_item_state_key(item)?;
    let exact_template = unique_crystal_item_by_index(exact_item_index)?;
    if exact_template.item_index != key_template.item_index {
        return Err(UserItemCarrierError::ConflictingItemIdentity {
            key: item.key.clone(),
            key_item_index: key_template.item_index,
            exact_item_index,
        });
    }
    Ok(())
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
            sell_value: {
                let mut unit = self.clone();
                unit.quantity = 1;
                crystal_sell_value_for_item(&unit)
            },
            equip_slot: self
                .equip_slot
                .or_else(|| crystal_equipment_slot_for_item_key(&self.key)),
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
    if item.user_item_metadata.is_some() {
        item.unique_id
    } else if item.unique_id == 0 {
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
        "bronze-helmet" => Some("content.item.bronzeHelmet"),
        "iron-helmet" => Some("content.item.ironHelmet"),
        "town-teleport" => Some("content.item.townTeleport"),
        _ => None,
    }
}

pub(super) fn localized_drop_name_key(name: &str) -> Option<&'static str> {
    match name {
        "Wasp Gold" => Some("content.item.waspGold.name"),
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

fn check_protocol_count(field: &'static str, count: usize) -> Result<(), UserItemCarrierError> {
    if count > i32::MAX as usize {
        Err(UserItemCarrierError::ProtocolCountExceeded { field, count })
    } else {
        Ok(())
    }
}

fn check_user_item_collections(
    slots: usize,
    added_stats: usize,
    awake_values: usize,
    budget: UserItemCarrierBudget,
) -> Result<(), UserItemCarrierError> {
    check_protocol_count("slots", slots)?;
    check_protocol_count("added_stats", added_stats)?;
    check_protocol_count("awake_values", awake_values)?;
    if slots > budget.max_slots_per_item {
        return Err(UserItemCarrierError::SlotsExceeded {
            count: slots,
            max: budget.max_slots_per_item,
        });
    }
    if added_stats > budget.max_added_stats_per_item {
        return Err(UserItemCarrierError::AddedStatsExceeded {
            count: added_stats,
            max: budget.max_added_stats_per_item,
        });
    }
    if awake_values > budget.max_awake_values_per_item {
        return Err(UserItemCarrierError::AwakeValuesExceeded {
            count: awake_values,
            max: budget.max_awake_values_per_item,
        });
    }
    Ok(())
}

fn check_item_state_quantity(quantity: u32) -> Result<u16, UserItemCarrierError> {
    // Crystal's binary UserItem carrier can represent count zero while an item
    // is being assembled. Mutation boundaries decide whether that transient
    // state may be committed; the carrier only rejects protocol overflow.
    u16::try_from(quantity).map_err(|_| UserItemCarrierError::QuantityExceeded {
        quantity,
        max: u16::MAX,
    })
}

fn enter_user_item_node(
    depth: usize,
    nodes: &mut usize,
    budget: UserItemCarrierBudget,
) -> Result<(), UserItemCarrierError> {
    if depth > budget.max_depth {
        return Err(UserItemCarrierError::DepthExceeded {
            depth,
            max: budget.max_depth,
        });
    }
    *nodes = nodes.saturating_add(1);
    if *nodes > budget.max_total_nodes {
        return Err(UserItemCarrierError::TotalNodesExceeded {
            nodes: *nodes,
            max: budget.max_total_nodes,
        });
    }
    Ok(())
}

fn validate_user_item_tree_with_budget(
    item: &UserItem,
    budget: UserItemCarrierBudget,
    depth: usize,
    nodes: &mut usize,
) -> Result<(), UserItemCarrierError> {
    enter_user_item_node(depth, nodes, budget)?;
    unique_crystal_item_by_index(item.item_index)?;
    check_user_item_collections(
        item.slots.len(),
        item.added_stats.len(),
        item.awake_values.len(),
        budget,
    )?;
    ensure_unambiguous_socket_identities(
        &item
            .slots
            .iter()
            .map(|slot| slot.as_ref().map(CapturedSocketIdentity::from_user_item))
            .collect::<Vec<_>>(),
    )?;
    for embedded in item.slots.iter().flatten() {
        validate_user_item_tree_with_budget(embedded, budget, depth + 1, nodes)?;
    }
    Ok(())
}

/// Complete fail-closed validation for an incoming or persisted protocol
/// UserItem tree. Save/mail/Stage5 boundaries should call this before storing
/// a UserItem that may later reach an infallible internal packet builder.
pub(super) fn validate_user_item_carrier_with_budget(
    item: &UserItem,
    budget: UserItemCarrierBudget,
) -> Result<(), UserItemCarrierError> {
    let mut nodes = 0;
    validate_user_item_tree_with_budget(item, budget, 0, &mut nodes)
}

pub(super) fn validate_user_item_carrier(item: &UserItem) -> Result<(), UserItemCarrierError> {
    validate_user_item_carrier_with_budget(item, UserItemCarrierBudget::default())
}

fn validate_committed_item_quantity(
    item_index: i32,
    quantity: u32,
) -> Result<(), UserItemCarrierError> {
    let template = unique_crystal_item_by_index(item_index)?;
    let max = u32::from(template.stack_size.max(1));
    if quantity == 0 || quantity > max {
        return Err(UserItemCarrierError::CommittedQuantityOutOfRange {
            item_index,
            quantity,
            max,
        });
    }
    Ok(())
}

fn validate_committed_user_item_tree(item: &UserItem) -> Result<(), UserItemCarrierError> {
    validate_committed_item_quantity(item.item_index, u32::from(item.count))?;
    for embedded in item.slots.iter().flatten() {
        validate_committed_user_item_tree(embedded)?;
    }
    Ok(())
}

/// Validation for a UserItem that is about to become durable or enter a live
/// item container. The generic wire carrier intentionally permits count zero;
/// committed roots and every recursive socket child must be real stacks.
pub(super) fn validate_committed_user_item_carrier(
    item: &UserItem,
) -> Result<(), UserItemCarrierError> {
    validate_user_item_carrier(item)?;
    validate_committed_user_item_tree(item)
}

fn ensure_unambiguous_socket_identities(
    positions: &[Option<CapturedSocketIdentity>],
) -> Result<(), UserItemCarrierError> {
    for (index, identity) in positions.iter().enumerate() {
        let Some(identity) = identity else {
            continue;
        };
        unique_crystal_item_by_index(identity.item_index)?;
        if positions[..index]
            .iter()
            .flatten()
            .any(|existing| existing == identity)
        {
            return Err(UserItemCarrierError::AmbiguousSocketIdentity {
                unique_id: identity.unique_id,
                item_index: identity.item_index,
            });
        }
    }
    Ok(())
}

fn captured_positions_from_user_item(
    item: &UserItem,
) -> Result<Vec<Option<CapturedSocketIdentity>>, UserItemCarrierError> {
    let positions = item
        .slots
        .iter()
        .map(|slot| slot.as_ref().map(CapturedSocketIdentity::from_user_item))
        .collect::<Vec<_>>();
    ensure_unambiguous_socket_identities(&positions)?;
    Ok(positions)
}

fn item_grade_from_crystal_carrier(grade: u8) -> ItemGrade {
    match grade {
        1 => ItemGrade::Common,
        2 => ItemGrade::Rare,
        3 => ItemGrade::Legendary,
        4 => ItemGrade::Mythical,
        5 => ItemGrade::Heroic,
        _ => ItemGrade::None,
    }
}

pub(super) fn embedded_item_state_from_template(
    template: &CrystalItemTemplate,
    container: ItemContainer,
    slot: u8,
) -> ItemState {
    ItemState {
        key: crystal_item_key_for_template(template),
        name: template.name.clone(),
        icon: template.image,
        slot,
        unique_id: 0,
        container,
        quantity: 1,
        description: template.tooltip.clone().unwrap_or_default(),
        durability_current: (template.durability != 0).then_some(template.durability),
        durability_max: (template.durability != 0).then_some(template.durability),
        weight: u16::from(template.weight),
        equip_slot: crystal_equipment_slot_for_template(template),
        grade: item_grade_from_crystal_carrier(template.grade),
        added_attack: 0,
        added_defence: 0,
        added_stats: Vec::new(),
        socketed: Vec::new(),
        user_item_metadata: None,
        cursed: false,
        socket_slots: template.slots,
        gem_count: 0,
        identified: Some(!template.need_identify),
        soul_bound_id: None,
        sealed_expiry_time_binary_datetime: 0,
        sealed_next_time_binary_datetime: 0,
        rental_binding_flags: 0,
        rental_owner_name: String::new(),
        rental_expiry_binary_datetime: 0,
        rental_locked: false,
        attack: crystal_item_stat_value(template, CRYSTAL_STAT_MAX_DC),
        defence: crystal_item_stat_value(template, CRYSTAL_STAT_MAX_AC),
        heal_hp: crystal_item_stat_value(template, CRYSTAL_STAT_HP),
        heal_mp: crystal_item_stat_value(template, CRYSTAL_STAT_MP),
    }
}

fn hydrate_user_item_into_state(
    mut state: ItemState,
    item: &UserItem,
    budget: UserItemCarrierBudget,
    depth: usize,
    nodes: &mut usize,
    captured_socket_position: Option<u8>,
) -> Result<ItemState, UserItemCarrierError> {
    validate_item_state_key_for_exact_index(&state, item.item_index)?;
    enter_user_item_node(depth, nodes, budget)?;
    check_user_item_collections(
        item.slots.len(),
        item.added_stats.len(),
        item.awake_values.len(),
        budget,
    )?;
    let socket_count = item.slots.len();
    let socket_slots =
        u8::try_from(socket_count).map_err(|_| UserItemCarrierError::SocketSlotWidthExceeded {
            count: socket_count,
        })?;
    let captured_socket_positions = captured_positions_from_user_item(item)?;
    let is_mount = crystal_item_template_for_item_key(&state.key)
        .is_some_and(|template| template.item_type == CRYSTAL_ITEM_TYPE_MOUNT);

    let mut hydrated = Vec::with_capacity(item.slots.iter().flatten().count());
    for (slot_index, embedded) in item.slots.iter().enumerate() {
        let Some(embedded) = embedded else {
            continue;
        };
        let template = unique_crystal_item_by_index(embedded.item_index)?;
        let is_bells = template.item_type == CRYSTAL_ITEM_TYPE_BELLS;
        if is_mount && is_bells && slot_index != 1 {
            return Err(UserItemCarrierError::SocketMapping {
                reason: "captured mount Bells must occupy protocol slot 1",
            });
        }
        if is_mount && slot_index == 1 && !is_bells {
            return Err(UserItemCarrierError::SocketMapping {
                reason: "captured mount protocol slot 1 is reserved for Bells",
            });
        }
        let slot = u8::try_from(slot_index).map_err(|_| {
            UserItemCarrierError::SocketSlotWidthExceeded {
                count: socket_count,
            }
        })?;
        let child = embedded_item_state_from_template(&template, state.container, slot);
        hydrated.push(hydrate_user_item_into_state(
            child,
            embedded,
            budget,
            depth + 1,
            nodes,
            Some(slot),
        )?);
    }

    let (added_attack, added_defence) = user_item_added_attack_defence(item);
    state.unique_id = item.unique_id;
    state.quantity = u32::from(item.count);
    state.durability_current =
        (item.current_dura != 0 || item.max_dura != 0).then_some(item.current_dura);
    state.durability_max = (item.max_dura != 0).then_some(item.max_dura);
    state.soul_bound_id = (item.soul_bound_id != -1).then_some(item.soul_bound_id);
    state.identified = Some(item.identified);
    state.cursed = item.cursed;
    state.gem_count = item.gem_count;
    state.added_attack = added_attack;
    state.added_defence = added_defence;
    state.added_stats = item.added_stats.clone();
    state.socket_slots = socket_slots;
    state.socketed = hydrated;
    state.rental_binding_flags = item
        .rental_information
        .as_ref()
        .map_or(0, |rental| rental.binding_flags);
    state.rental_owner_name = item
        .rental_information
        .as_ref()
        .map_or_else(String::new, |rental| rental.owner_name.clone());
    state.rental_expiry_binary_datetime = item
        .rental_information
        .as_ref()
        .map_or(0, |rental| rental.expiry_binary_datetime);
    state.rental_locked = item
        .rental_information
        .as_ref()
        .is_some_and(|rental| rental.rental_locked);
    state.sealed_expiry_time_binary_datetime = item
        .sealed_info
        .as_ref()
        .map_or(0, |sealed| sealed.expiry_binary_datetime);
    state.sealed_next_time_binary_datetime = item
        .sealed_info
        .as_ref()
        .map_or(0, |sealed| sealed.next_seal_binary_datetime);
    state.user_item_metadata = Some(ItemStateUserItemMetadata::from_hydrated_user_item(
        item,
        captured_socket_positions,
        captured_socket_position,
    ));
    Ok(state)
}

/// Fallible, bounded protocol-to-save conversion. Occupied protocol slots are
/// recursively hydrated into live ItemState children before the carrier is
/// returned.
pub(super) fn try_item_state_from_user_item(
    state: ItemState,
    item: &UserItem,
) -> Result<ItemState, UserItemCarrierError> {
    try_item_state_from_user_item_with_budget(state, item, UserItemCarrierBudget::default())
}

fn try_item_state_from_user_item_with_budget(
    state: ItemState,
    item: &UserItem,
    budget: UserItemCarrierBudget,
) -> Result<ItemState, UserItemCarrierError> {
    validate_user_item_carrier_with_budget(item, budget)?;
    let mut nodes = 0;
    let state = hydrate_user_item_into_state(state, item, budget, 0, &mut nodes, None)?;
    validate_item_state_carrier_storage_with_budget(&state, budget)?;
    Ok(state)
}

fn captured_rental_matches_live(captured: &UserItemRentalInformation, item: &ItemState) -> bool {
    captured.binding_flags == item.rental_binding_flags
        && captured.owner_name == item.rental_owner_name
        && captured.expiry_binary_datetime == item.rental_expiry_binary_datetime
        && captured.rental_locked == item.rental_locked
}

fn captured_sealed_matches_live(captured: &UserItemSealedInfo, item: &ItemState) -> bool {
    captured.expiry_binary_datetime == item.sealed_expiry_time_binary_datetime
        && captured.next_seal_binary_datetime == item.sealed_next_time_binary_datetime
}

fn rental_information_from_item_state(
    item: &ItemState,
    metadata: Option<&ItemStateUserItemMetadata>,
) -> Option<UserItemRentalInformation> {
    let live = user_item_rental_information(
        item.rental_binding_flags,
        &item.rental_owner_name,
        item.rental_expiry_binary_datetime,
        item.rental_locked,
    );
    metadata
        .and_then(|metadata| metadata.rental_information.as_ref())
        .filter(|captured| captured_rental_matches_live(captured, item))
        .cloned()
        .or(live)
}

fn sealed_information_from_item_state(
    item: &ItemState,
    metadata: Option<&ItemStateUserItemMetadata>,
) -> Option<UserItemSealedInfo> {
    let live = (item.sealed_expiry_time_binary_datetime != 0
        || item.sealed_next_time_binary_datetime != 0)
        .then_some(UserItemSealedInfo {
            expiry_binary_datetime: item.sealed_expiry_time_binary_datetime,
            next_seal_binary_datetime: item.sealed_next_time_binary_datetime,
        });
    metadata
        .and_then(|metadata| metadata.sealed_info.as_ref())
        .filter(|captured| captured_sealed_matches_live(captured, item))
        .cloned()
        .or(live)
}

fn clone_embedded_user_item_with_budget(
    item: &UserItem,
    slot: usize,
    budget: UserItemCarrierBudget,
    depth: usize,
    nodes: &mut usize,
) -> Result<UserItem, UserItemCarrierError> {
    let _ = slot;
    unique_crystal_item_by_index(item.item_index)?;
    enter_user_item_node(depth, nodes, budget)?;
    check_user_item_collections(
        item.slots.len(),
        item.added_stats.len(),
        item.awake_values.len(),
        budget,
    )?;

    let mut slots = Vec::with_capacity(item.slots.len());
    for (child_slot, embedded) in item.slots.iter().enumerate() {
        slots.push(
            embedded
                .as_ref()
                .map(|embedded| {
                    clone_embedded_user_item_with_budget(
                        embedded,
                        child_slot,
                        budget,
                        depth + 1,
                        nodes,
                    )
                })
                .transpose()?,
        );
    }

    Ok(UserItem {
        unique_id: item.unique_id,
        item_index: item.item_index,
        current_dura: item.current_dura,
        max_dura: item.max_dura,
        count: item.count,
        soul_bound_id: item.soul_bound_id,
        identified: item.identified,
        cursed: item.cursed,
        slots,
        gem_count: item.gem_count,
        added_stats: item.added_stats.clone(),
        awake_type: item.awake_type,
        awake_values: item.awake_values.clone(),
        refined_value: item.refined_value,
        refine_added: item.refine_added,
        refine_success_chance: item.refine_success_chance,
        wedding_ring: item.wedding_ring,
        expire_info: item.expire_info.clone(),
        rental_information: item.rental_information.clone(),
        is_shop_item: item.is_shop_item,
        sealed_info: item.sealed_info.clone(),
        gm_made: item.gm_made,
    })
}

fn clone_legacy_captured_slots_with_budget(
    captured: &[Option<UserItem>],
    socket_slots: usize,
    budget: UserItemCarrierBudget,
    depth: usize,
    nodes: &mut usize,
) -> Result<Vec<Option<UserItem>>, UserItemCarrierError> {
    check_user_item_collections(socket_slots, 0, 0, budget)?;
    check_protocol_count("captured_slots", captured.len())?;
    if captured.len() > socket_slots {
        return Err(UserItemCarrierError::SocketMapping {
            reason: "live socket slot width would truncate captured protocol slots",
        });
    }
    let mut slots = Vec::with_capacity(socket_slots);
    for (slot_index, slot) in captured.iter().enumerate() {
        slots.push(
            slot.as_ref()
                .map(|embedded| {
                    clone_embedded_user_item_with_budget(
                        embedded,
                        slot_index,
                        budget,
                        depth + 1,
                        nodes,
                    )
                })
                .transpose()?,
        );
    }
    slots.resize(socket_slots, None);
    Ok(slots)
}

fn metadata_captured_positions(
    metadata: Option<&ItemStateUserItemMetadata>,
    socket_slots: usize,
) -> Result<Option<Vec<Option<CapturedSocketIdentity>>>, UserItemCarrierError> {
    let Some(metadata) = metadata else {
        return Ok(None);
    };
    let positions = if let Some(positions) = &metadata.captured_socket_positions {
        if positions.len() != socket_slots {
            return Err(UserItemCarrierError::SocketMapping {
                reason: "captured socket position map width differs from live socket width",
            });
        }
        positions.clone()
    } else if !metadata.slots.is_empty() {
        if metadata.slots.len() > socket_slots {
            return Err(UserItemCarrierError::SocketMapping {
                reason: "legacy captured slots exceed live socket width",
            });
        }
        let mut positions = metadata
            .slots
            .iter()
            .map(|slot| slot.as_ref().map(CapturedSocketIdentity::from_user_item))
            .collect::<Vec<_>>();
        positions.resize(socket_slots, None);
        positions
    } else {
        return Ok(None);
    };
    ensure_unambiguous_socket_identities(&positions)?;
    Ok(Some(positions))
}

struct LiveSocketCandidate {
    is_bells: bool,
    identity: CapturedSocketIdentity,
    captured_position: Option<usize>,
    item: UserItem,
}

fn legacy_item_state_protocol_slots_with_budget(
    item: &ItemState,
    budget: UserItemCarrierBudget,
    depth: usize,
    nodes: &mut usize,
) -> Result<Vec<Option<UserItem>>, UserItemCarrierError> {
    let socket_slots = usize::from(item.socket_slots);
    check_user_item_collections(socket_slots, 0, 0, budget)?;
    let mut slots = vec![None; socket_slots];

    // Sidecar-less ItemState is the legacy internal carrier. Preserve its
    // historical wire behavior: only Mount Bells occupied a protocol slot;
    // arbitrary internal descendants were not emitted as UserItem sockets.
    if socket_slots > 1
        && crystal_item_template_for_item_key(&item.key)
            .is_some_and(|template| template.item_type == CRYSTAL_ITEM_TYPE_MOUNT)
    {
        for embedded in &item.socketed {
            if crystal_item_template_for_item_key(&embedded.key)
                .is_some_and(|template| template.item_type == CRYSTAL_ITEM_TYPE_BELLS)
            {
                slots[1] = Some(try_user_item_from_item_state_inner(
                    embedded,
                    budget,
                    depth + 1,
                    nodes,
                )?);
            }
        }
    }
    Ok(slots)
}

fn reconcile_live_socket_slots_with_budget(
    item: &ItemState,
    budget: UserItemCarrierBudget,
    depth: usize,
    nodes: &mut usize,
) -> Result<Vec<Option<UserItem>>, UserItemCarrierError> {
    let socket_slots = usize::from(item.socket_slots);
    check_user_item_collections(socket_slots, 0, 0, budget)?;
    if item.socketed.len() > socket_slots {
        return Err(UserItemCarrierError::SocketMapping {
            reason: "more live socket items than declared socket slots",
        });
    }

    let is_mount = crystal_item_template_for_item_key(&item.key)
        .is_some_and(|template| template.item_type == CRYSTAL_ITEM_TYPE_MOUNT);
    let captured = metadata_captured_positions(item.user_item_metadata.as_ref(), socket_slots)?
        .unwrap_or_else(|| vec![None; socket_slots]);
    let mut candidates = Vec::with_capacity(item.socketed.len());
    for embedded in &item.socketed {
        let protocol = try_user_item_from_item_state_inner(embedded, budget, depth + 1, nodes)?;
        let template = unique_crystal_item_by_index(protocol.item_index)?;
        candidates.push(LiveSocketCandidate {
            is_bells: template.item_type == CRYSTAL_ITEM_TYPE_BELLS,
            identity: CapturedSocketIdentity::from_user_item(&protocol),
            captured_position: embedded
                .user_item_metadata
                .as_ref()
                .and_then(|metadata| metadata.captured_socket_position)
                .map(usize::from),
            item: protocol,
        });
    }

    let mut slots = vec![None; socket_slots];
    let mut unresolved = Vec::new();
    for candidate in candidates {
        let target = if is_mount && candidate.is_bells {
            Some(1)
        } else if let Some(position) = candidate.captured_position {
            Some(position)
        } else {
            let matches = captured
                .iter()
                .enumerate()
                .filter_map(|(position, identity)| {
                    (*identity == Some(candidate.identity)).then_some(position)
                })
                .collect::<Vec<_>>();
            if matches.len() > 1 {
                return Err(UserItemCarrierError::AmbiguousSocketIdentity {
                    unique_id: candidate.identity.unique_id,
                    item_index: candidate.identity.item_index,
                });
            }
            matches.first().copied()
        };

        let Some(target) = target else {
            unresolved.push(candidate);
            continue;
        };
        if target >= socket_slots {
            return Err(UserItemCarrierError::SocketMapping {
                reason: "captured socket position is outside live socket width",
            });
        }
        if is_mount && !candidate.is_bells && target == 1 {
            return Err(UserItemCarrierError::SocketMapping {
                reason: "mount protocol slot 1 is reserved for Bells",
            });
        }
        if slots[target].is_some() {
            return Err(UserItemCarrierError::SocketMapping {
                reason: "multiple live socket items resolve to the same protocol slot",
            });
        }
        slots[target] = Some(candidate.item);
    }

    let mut reserved = captured.iter().map(Option::is_some).collect::<Vec<_>>();
    if is_mount && socket_slots > 1 {
        reserved[1] = true;
    }
    for candidate in unresolved {
        if is_mount && candidate.is_bells {
            return Err(UserItemCarrierError::SocketMapping {
                reason: "mount Bells could not resolve protocol slot 1",
            });
        }
        let target = slots
            .iter()
            .enumerate()
            .find_map(|(position, slot)| {
                (!reserved[position] && slot.is_none()).then_some(position)
            })
            .or_else(|| {
                slots.iter().enumerate().find_map(|(position, slot)| {
                    (!(is_mount && position == 1) && slot.is_none()).then_some(position)
                })
            })
            .ok_or(UserItemCarrierError::SocketMapping {
                reason: "no protocol slot remains for a live socket item",
            })?;
        slots[target] = Some(candidate.item);
        reserved[target] = true;
    }
    Ok(slots)
}

fn try_user_item_from_item_state_inner(
    item: &ItemState,
    budget: UserItemCarrierBudget,
    depth: usize,
    nodes: &mut usize,
) -> Result<UserItem, UserItemCarrierError> {
    enter_user_item_node(depth, nodes, budget)?;
    let item_index = exact_item_index_for_item_state(item)?;
    let count = check_item_state_quantity(item.quantity)?;
    let metadata = item.user_item_metadata.as_ref();
    let awake_values_len = metadata.map_or(0, |metadata| metadata.awake_values.len());
    check_user_item_collections(
        usize::from(item.socket_slots),
        item.added_stats.len(),
        awake_values_len,
        budget,
    )?;

    let live_socketed_is_authoritative = !item.socketed.is_empty()
        || metadata.is_some_and(|metadata| {
            metadata.live_socketed_at_capture || metadata.socket_layout_hydrated
        });
    let slots = if metadata.is_none() {
        legacy_item_state_protocol_slots_with_budget(item, budget, depth, nodes)?
    } else if live_socketed_is_authoritative {
        reconcile_live_socket_slots_with_budget(item, budget, depth, nodes)?
    } else {
        clone_legacy_captured_slots_with_budget(
            metadata.map_or(&[], |metadata| metadata.slots.as_slice()),
            usize::from(item.socket_slots),
            budget,
            depth,
            nodes,
        )?
    };

    let added_stats = merged_user_item_stats(
        &item.added_stats,
        item.added_defence,
        item.added_attack,
        None,
    );
    check_user_item_collections(slots.len(), added_stats.len(), awake_values_len, budget)?;

    Ok(UserItem {
        unique_id: if metadata.is_some() {
            item.unique_id
        } else {
            item_unique_id(item)
        },
        item_index,
        current_dura: item.durability_current.unwrap_or(0),
        max_dura: item.durability_max.unwrap_or(0),
        count,
        soul_bound_id: item_state_soul_bound_id(item),
        identified: item_state_identified(item),
        cursed: item.cursed,
        slots,
        gem_count: item.gem_count,
        added_stats,
        awake_type: metadata.map_or(0, |metadata| metadata.awake_type),
        awake_values: metadata.map_or_else(Vec::new, |metadata| metadata.awake_values.clone()),
        refined_value: metadata.map_or(0, |metadata| metadata.refined_value),
        refine_added: metadata.map_or(0, |metadata| metadata.refine_added),
        refine_success_chance: metadata.map_or(0, |metadata| metadata.refine_success_chance),
        wedding_ring: metadata.map_or(-1, |metadata| metadata.wedding_ring),
        expire_info: metadata.and_then(|metadata| metadata.expire_info.clone()),
        rental_information: rental_information_from_item_state(item, metadata),
        is_shop_item: metadata.is_some_and(|metadata| metadata.is_shop_item),
        sealed_info: sealed_information_from_item_state(item, metadata),
        gm_made: metadata.is_some_and(|metadata| metadata.gm_made),
    })
}

fn validate_legacy_user_item_tree(
    item: &UserItem,
    slot: usize,
    budget: UserItemCarrierBudget,
    depth: usize,
    nodes: &mut usize,
) -> Result<(), UserItemCarrierError> {
    let _ = slot;
    validate_user_item_tree_with_budget(item, budget, depth, nodes)
}

fn validate_item_state_carrier_storage_inner(
    item: &ItemState,
    budget: UserItemCarrierBudget,
    depth: usize,
    nodes: &mut usize,
) -> Result<(), UserItemCarrierError> {
    enter_user_item_node(depth, nodes, budget)?;
    exact_item_index_for_item_state(item)?;
    check_item_state_quantity(item.quantity)?;
    let metadata = item.user_item_metadata.as_ref();
    check_user_item_collections(
        usize::from(item.socket_slots).max(item.socketed.len()),
        item.added_stats.len(),
        metadata.map_or(0, |metadata| metadata.awake_values.len()),
        budget,
    )?;
    if metadata.is_some() && item.socketed.len() > usize::from(item.socket_slots) {
        return Err(UserItemCarrierError::SocketMapping {
            reason: "more live socket items than declared socket slots",
        });
    }
    if let Some(metadata) = metadata {
        if let Some(positions) = &metadata.captured_socket_positions {
            if positions.len() != usize::from(item.socket_slots) {
                return Err(UserItemCarrierError::SocketMapping {
                    reason: "captured socket position map width differs from live socket width",
                });
            }
            ensure_unambiguous_socket_identities(positions)?;
        }
        check_user_item_collections(metadata.slots.len(), 0, metadata.awake_values.len(), budget)?;
        for (slot, embedded) in metadata.slots.iter().enumerate() {
            if let Some(embedded) = embedded {
                validate_legacy_user_item_tree(embedded, slot, budget, depth + 1, nodes)?;
            }
        }
    }
    for embedded in &item.socketed {
        validate_item_state_carrier_storage_inner(embedded, budget, depth + 1, nodes)?;
    }
    Ok(())
}

fn validate_item_state_carrier_storage_with_budget(
    item: &ItemState,
    budget: UserItemCarrierBudget,
) -> Result<(), UserItemCarrierError> {
    let mut nodes = 0;
    validate_item_state_carrier_storage_inner(item, budget, 0, &mut nodes)
}

/// Reusable loading-boundary validation for a complete ItemState carrier.
/// Both persisted sidecar data and hydrated live socket children are bounded.
pub(super) fn validate_item_state_carrier_with_budget(
    item: &ItemState,
    budget: UserItemCarrierBudget,
) -> Result<(), UserItemCarrierError> {
    validate_item_state_carrier_storage_with_budget(item, budget)?;
    let mut output_nodes = 0;
    try_user_item_from_item_state_inner(item, budget, 0, &mut output_nodes)?;
    Ok(())
}

pub(super) fn validate_item_state_carrier(item: &ItemState) -> Result<(), UserItemCarrierError> {
    validate_item_state_carrier_with_budget(item, UserItemCarrierBudget::default())
}

fn validate_committed_item_state_tree(item: &ItemState) -> Result<(), UserItemCarrierError> {
    validate_committed_item_quantity(exact_item_index_for_item_state(item)?, item.quantity)?;
    if let Some(metadata) = &item.user_item_metadata {
        for embedded in metadata.slots.iter().flatten() {
            validate_committed_user_item_tree(embedded)?;
        }
    }
    for embedded in &item.socketed {
        validate_committed_item_state_tree(embedded)?;
    }
    Ok(())
}

/// ItemState counterpart of `validate_committed_user_item_carrier`. Generic
/// carrier validation runs first; the committed check then walks both legacy
/// sidecar slots and live ItemState socket children without relying on lossy
/// compatibility conversion.
pub(super) fn validate_committed_item_state_carrier(
    item: &ItemState,
) -> Result<(), UserItemCarrierError> {
    validate_item_state_carrier(item)?;
    validate_committed_item_state_tree(item)
}

fn try_user_item_from_item_state_with_budget(
    item: &ItemState,
    budget: UserItemCarrierBudget,
) -> Result<UserItem, UserItemCarrierError> {
    validate_item_state_carrier_storage_with_budget(item, budget)?;
    let mut nodes = 0;
    try_user_item_from_item_state_inner(item, budget, 0, &mut nodes)
}

/// Fallible, bounded save-carrier-to-protocol conversion.
pub(super) fn try_user_item_from_item_state(
    item: &ItemState,
) -> Result<UserItem, UserItemCarrierError> {
    try_user_item_from_item_state_with_budget(item, UserItemCarrierBudget::default())
}

/// Compatibility entry for already-validated internal runtime state only.
/// Save/mail/incoming data must use `validate_item_state_carrier` and callers
/// that can propagate errors must use `try_user_item_from_item_state` instead.
/// Keeping the panic here makes an internal invariant violation loud without
/// turning malformed external data into a process-level failure.
#[track_caller]
pub(super) fn user_item_from_item_state(item: &ItemState) -> UserItem {
    try_user_item_from_item_state(item)
        .unwrap_or_else(|error| panic!("internal ItemState carrier rejected: {error}"))
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
        let content_spell = crystal_magic_for_skill_key(&skill.key)
            .map(|magic| magic.spell)
            .unwrap_or_else(|| skill.key.clone());
        if !world
            .resource::<RuntimeConfigResource>()
            .config
            .skill_is_allowed(&content_spell, character.class, character.level)
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

#[cfg(test)]
mod item_identity_roundtrip_tests {
    use super::*;

    fn crystal_template(name: &str) -> CrystalItemTemplate {
        crystal_item_by_name(name).unwrap_or_else(|| panic!("Crystal template {name} must exist"))
    }

    fn minimal_user_item_for_template(unique_id: u64, template_name: &str) -> UserItem {
        UserItem {
            unique_id,
            item_index: crystal_template(template_name).item_index,
            current_dura: 0,
            max_dura: 0,
            count: 1,
            soul_bound_id: -1,
            identified: true,
            cursed: false,
            slots: Vec::new(),
            gem_count: 0,
            added_stats: Vec::new(),
            awake_type: 0,
            awake_values: Vec::new(),
            refined_value: 0,
            refine_added: 0,
            refine_success_chance: 0,
            wedding_ring: -1,
            expire_info: None,
            rental_information: None,
            is_shop_item: false,
            sealed_info: None,
            gm_made: false,
        }
    }

    fn minimal_user_item(unique_id: u64) -> UserItem {
        minimal_user_item_for_template(unique_id, "BronzeHelmet")
    }

    fn known_embedded_user_item(unique_id: u64, template_name: &str) -> UserItem {
        minimal_user_item_for_template(unique_id, template_name)
    }

    fn complex_user_item() -> UserItem {
        let mut nested = known_embedded_user_item(9002, "BronzeBell");
        nested.count = 2;
        nested.awake_type = 8;
        nested.awake_values = vec![4, 9];

        let mut recursive = known_embedded_user_item(9003, "DemonicBells");
        recursive.current_dura = 11;
        recursive.max_dura = 22;
        recursive.slots = vec![Some(nested.clone())];
        recursive.added_stats = vec![UserItemStat { stat: 12, value: 5 }];

        let mut root = minimal_user_item(9001);
        root.current_dura = 321;
        root.max_dura = 654;
        root.count = 7;
        root.soul_bound_id = 77;
        root.identified = false;
        root.cursed = true;
        root.slots = vec![None, Some(nested), Some(recursive)];
        root.gem_count = 13;
        root.added_stats = vec![
            UserItemStat { stat: 1, value: 70 },
            UserItemStat { stat: 5, value: 50 },
            UserItemStat { stat: 17, value: 3 },
        ];
        root.awake_type = 6;
        root.awake_values = vec![2, 7, 8];
        root.refined_value = 4;
        root.refine_added = 3;
        root.refine_success_chance = 61;
        root.wedding_ring = 1234;
        root.expire_info = Some(UserItemExpireInfo {
            expiry_binary_datetime: 987654321,
        });
        root.rental_information = Some(UserItemRentalInformation {
            owner_name: "renter".to_string(),
            binding_flags: 12,
            expiry_binary_datetime: 123456789,
            rental_locked: true,
        });
        root.is_shop_item = true;
        root.sealed_info = Some(UserItemSealedInfo {
            expiry_binary_datetime: 111,
            next_seal_binary_datetime: 222,
        });
        root.gm_made = true;
        root
    }

    fn base_item_state_for_index(item_index: i32) -> ItemState {
        let template = unique_crystal_item_by_index(item_index).expect("known Crystal index");
        let mut state = embedded_item_state_from_template(&template, ItemContainer::Bag1, 3);
        state.unique_id = 1;
        state.quantity = 1;
        state.socket_slots = 0;
        state.socketed.clear();
        state.user_item_metadata = None;
        state
    }

    fn base_item_state() -> ItemState {
        base_item_state_for_index(crystal_template("BronzeHelmet").item_index)
    }

    fn state_for_user_item(item: &UserItem) -> ItemState {
        base_item_state_for_index(item.item_index)
    }

    fn budget_with(
        max_depth: usize,
        max_total_nodes: usize,
        max_slots_per_item: usize,
    ) -> UserItemCarrierBudget {
        UserItemCarrierBudget {
            max_depth,
            max_total_nodes,
            max_slots_per_item,
            max_added_stats_per_item: 4,
            max_awake_values_per_item: 4,
        }
    }

    #[test]
    fn complex_user_item_survives_item_state_save_json_reload() {
        let expected = complex_user_item();
        validate_user_item_carrier(&expected).expect("incoming protocol carrier should validate");
        let state = try_item_state_from_user_item(state_for_user_item(&expected), &expected)
            .expect("protocol identity should hydrate");
        let save_json = serde_json::to_string(&state).expect("ItemState save JSON should encode");
        let reloaded: ItemState =
            serde_json::from_str(&save_json).expect("ItemState save JSON should reload");

        validate_item_state_carrier(&reloaded).expect("saved carrier should validate");
        assert_eq!(user_item_from_item_state(&reloaded), expected);
    }

    #[test]
    fn sidecar_less_legal_crystal_item_state_remains_compatible() {
        let state = base_item_state();
        let mut old_json = serde_json::to_value(&state).expect("old ItemState JSON should encode");
        old_json
            .as_object_mut()
            .expect("ItemState should encode as an object")
            .remove("user_item_metadata");
        let reloaded: ItemState =
            serde_json::from_value(old_json).expect("old ItemState JSON should load");

        assert_eq!(reloaded.user_item_metadata, None);
        validate_item_state_carrier(&reloaded).expect("known sidecar-less Crystal state is legal");
        let item =
            try_user_item_from_item_state(&reloaded).expect("legacy output should be fallible");
        assert_eq!(item.item_index, crystal_template("BronzeHelmet").item_index);
        assert_eq!(item.awake_type, 0);
        assert!(item.awake_values.is_empty());
        assert_eq!(item.wedding_ring, -1);
        assert!(item.slots.is_empty());
    }

    #[test]
    fn unknown_sidecar_less_key_fails_closed_without_icon_fallback() {
        let mut state = base_item_state();
        state.key = "not-a-crystal-item".to_string();
        state.icon = 0;

        assert!(matches!(
            validate_item_state_carrier(&state),
            Err(UserItemCarrierError::UnknownItemKey { key }) if key == "not-a-crystal-item"
        ));
        assert!(try_user_item_from_item_state(&state).is_err());
    }

    #[test]
    fn exact_carrier_requires_known_root_item_index() {
        let valid = minimal_user_item(9_130);
        let mut state = try_item_state_from_user_item(state_for_user_item(&valid), &valid)
            .expect("valid exact carrier");
        state
            .user_item_metadata
            .as_mut()
            .expect("exact metadata")
            .item_index = Some(i32::MAX);

        assert!(matches!(
            validate_item_state_carrier(&state),
            Err(UserItemCarrierError::UnknownItemIndex {
                item_index: i32::MAX
            })
        ));

        let mut incoming = valid;
        incoming.item_index = i32::MAX;
        assert!(matches!(
            validate_user_item_carrier(&incoming),
            Err(UserItemCarrierError::UnknownItemIndex {
                item_index: i32::MAX
            })
        ));
        assert!(try_item_state_from_user_item(base_item_state(), &incoming).is_err());
    }

    #[test]
    fn exact_carrier_missing_index_or_conflicting_key_fails_closed() {
        let valid = minimal_user_item(9_131);
        let mut missing = try_item_state_from_user_item(state_for_user_item(&valid), &valid)
            .expect("valid exact carrier");
        missing
            .user_item_metadata
            .as_mut()
            .expect("exact metadata")
            .item_index = None;
        assert!(matches!(
            validate_item_state_carrier(&missing),
            Err(UserItemCarrierError::MissingExactItemIndex { .. })
        ));

        let mut conflicting = try_item_state_from_user_item(state_for_user_item(&valid), &valid)
            .expect("valid exact carrier");
        conflicting
            .user_item_metadata
            .as_mut()
            .expect("exact metadata")
            .item_index = Some(crystal_template("BronzeBell").item_index);
        assert!(matches!(
            try_user_item_from_item_state(&conflicting),
            Err(UserItemCarrierError::ConflictingItemIdentity { .. })
        ));
    }

    #[test]
    fn zero_current_durability_remains_a_live_broken_item() {
        let mut expected = minimal_user_item(9004);
        expected.current_dura = 0;
        expected.max_dura = 500;

        let state = try_item_state_from_user_item(state_for_user_item(&expected), &expected)
            .expect("protocol identity should hydrate");

        assert_eq!(state.durability_current, Some(0));
        assert_eq!(state.durability_max, Some(500));
        assert_eq!(user_item_from_item_state(&state), expected);
    }

    #[test]
    fn current_item_state_fields_override_stale_sidecar_values() {
        let expected = complex_user_item();
        let mut state = try_item_state_from_user_item(state_for_user_item(&expected), &expected)
            .expect("protocol identity should hydrate");
        state.unique_id = 9010;
        state.quantity = 99;
        state.durability_current = Some(111);
        state.durability_max = Some(222);
        state.soul_bound_id = None;
        state.identified = Some(true);
        state.cursed = false;
        state.gem_count = 2;
        state.added_attack = 0;
        state.added_defence = 0;
        state.added_stats = vec![UserItemStat { stat: 5, value: 14 }];
        state.rental_binding_flags = 0;
        state.rental_owner_name.clear();
        state.rental_expiry_binary_datetime = 0;
        state.rental_locked = false;
        state.sealed_expiry_time_binary_datetime = 0;
        state.sealed_next_time_binary_datetime = 0;

        let actual = try_user_item_from_item_state(&state).expect("valid exact carrier");
        assert_eq!(actual.unique_id, 9010);
        assert_eq!(actual.count, 99);
        assert_eq!(actual.current_dura, 111);
        assert_eq!(actual.max_dura, 222);
        assert_eq!(actual.soul_bound_id, -1);
        assert!(actual.identified);
        assert!(!actual.cursed);
        assert_eq!(actual.gem_count, 2);
        assert_eq!(
            actual.added_stats,
            vec![UserItemStat { stat: 5, value: 14 }]
        );
        assert_eq!(actual.rental_information, None);
        assert_eq!(actual.sealed_info, None);
        assert_eq!(actual.awake_type, expected.awake_type);
        assert_eq!(actual.refined_value, expected.refined_value);
        assert_eq!(actual.wedding_ring, expected.wedding_ring);
        assert_eq!(actual.expire_info, expected.expire_info);
        assert!(actual.is_shop_item);
        assert!(actual.gm_made);
    }

    #[test]
    fn recursive_protocol_slots_are_preserved_when_unmodified() {
        let expected = complex_user_item();
        let state = try_item_state_from_user_item(state_for_user_item(&expected), &expected)
            .expect("protocol identity should hydrate");
        let reloaded: ItemState =
            serde_json::from_str(&serde_json::to_string(&state).expect("ItemState should encode"))
                .expect("ItemState should reload");
        let actual = try_user_item_from_item_state(&reloaded).expect("valid recursive carrier");

        assert_eq!(actual.slots, expected.slots);
        assert_eq!(
            actual.slots[1].as_ref().and_then(|item| item.slots.first()),
            None
        );
        assert_eq!(
            actual.slots[2]
                .as_ref()
                .and_then(|item| item.slots.first())
                .and_then(Option::as_ref)
                .map(|item| item.unique_id),
            Some(9002)
        );
    }

    #[test]
    fn carrier_budget_rejects_excessive_depth_total_nodes_and_slots() {
        let leaf = known_embedded_user_item(9_101, "BronzeBell");
        let mut child = known_embedded_user_item(9_102, "DemonicBells");
        child.slots = vec![Some(leaf.clone())];
        let mut root = minimal_user_item(9_103);
        root.slots = vec![Some(child)];

        let depth_budget = budget_with(1, 8, 4);
        let depth_error = validate_user_item_carrier_with_budget(&root, depth_budget)
            .expect_err("input depth two must exceed a depth-one budget");
        assert!(matches!(
            depth_error,
            UserItemCarrierError::DepthExceeded { depth: 2, max: 1 }
        ));

        let mut wide = minimal_user_item(9_104);
        wide.slots = vec![
            Some(leaf),
            Some(known_embedded_user_item(9_107, "DemonicBells")),
        ];
        let node_error = validate_user_item_carrier_with_budget(&wide, budget_with(2, 2, 4))
            .expect_err("root plus two children must exceed a two-node budget");
        assert!(matches!(
            node_error,
            UserItemCarrierError::TotalNodesExceeded { nodes: 3, max: 2 }
        ));

        let slot_error = validate_user_item_carrier_with_budget(&wide, budget_with(2, 8, 1))
            .expect_err("two slots must exceed a one-slot budget");
        assert!(matches!(
            slot_error,
            UserItemCarrierError::SlotsExceeded { count: 2, max: 1 }
        ));
    }

    #[test]
    fn carrier_budget_rejects_excessive_added_stats_and_awake_values() {
        let mut stats = minimal_user_item(9_105);
        stats.added_stats = vec![
            UserItemStat { stat: 1, value: 1 },
            UserItemStat { stat: 2, value: 2 },
        ];
        let mut budget = budget_with(1, 4, 2);
        budget.max_added_stats_per_item = 1;
        assert!(matches!(
            validate_user_item_carrier_with_budget(&stats, budget),
            Err(UserItemCarrierError::AddedStatsExceeded { count: 2, max: 1 })
        ));

        let mut awake = minimal_user_item(9_106);
        awake.awake_values = vec![1, 2];
        budget.max_added_stats_per_item = 4;
        budget.max_awake_values_per_item = 1;
        assert!(matches!(
            validate_user_item_carrier_with_budget(&awake, budget),
            Err(UserItemCarrierError::AwakeValuesExceeded { count: 2, max: 1 })
        ));
    }

    #[test]
    fn captured_socket_positions_and_exact_indices_survive_live_reconciliation() {
        let socket_a = known_embedded_user_item(9_110, "BronzeBell");
        let mut host = minimal_user_item(9_109);
        host.slots = vec![Some(socket_a), None, None];
        let mut state = try_item_state_from_user_item(state_for_user_item(&host), &host)
            .expect("captured socket A should hydrate");
        assert_eq!(
            state.socketed[0]
                .user_item_metadata
                .as_ref()
                .and_then(|metadata| metadata.captured_socket_position),
            Some(0)
        );

        let socket_b = known_embedded_user_item(9_111, "DemonicBells");
        let socket_b_state =
            try_item_state_from_user_item(state_for_user_item(&socket_b), &socket_b)
                .expect("new socket B should hydrate");
        state.socketed.push(socket_b_state);

        let inserted = try_user_item_from_item_state(&state).expect("A plus B should reconcile");
        assert_eq!(
            inserted.slots[0].as_ref().map(|item| item.unique_id),
            Some(9_110)
        );
        assert_eq!(
            inserted.slots[1].as_ref().map(|item| item.unique_id),
            Some(9_111)
        );
        assert_eq!(inserted.slots[2], None);

        state.socketed.retain(|item| item.unique_id != 9_110);
        let removed = try_user_item_from_item_state(&state).expect("removing A should reconcile");
        assert_eq!(removed.slots[0], None);
        assert_eq!(
            removed.slots[1].as_ref().map(|item| item.unique_id),
            Some(9_111)
        );
        assert_eq!(removed.slots[2], None);
        validate_item_state_carrier(&state).expect("reconciled carrier should validate");
    }

    #[test]
    fn unknown_captured_socket_index_fails_closed() {
        let socket = known_embedded_user_item(9_112, "BronzeBell");
        let mut host = minimal_user_item(9_113);
        host.slots = vec![Some(socket)];
        let mut state = try_item_state_from_user_item(state_for_user_item(&host), &host)
            .expect("captured socket should hydrate");
        state
            .user_item_metadata
            .as_mut()
            .and_then(|metadata| metadata.captured_socket_positions.as_mut())
            .and_then(|positions| positions[0].as_mut())
            .expect("captured identity")
            .item_index = i32::MAX;

        assert!(matches!(
            validate_item_state_carrier(&state),
            Err(UserItemCarrierError::UnknownItemIndex {
                item_index: i32::MAX
            })
        ));
    }

    #[test]
    fn hydrated_mount_bells_clear_protocol_slot_one_when_removed() {
        let mount_template = crystal_template("RedTiger");
        let bells = known_embedded_user_item(9_121, "BronzeBell");

        let mut host = minimal_user_item_for_template(9_120, "RedTiger");
        host.slots = vec![None, Some(bells)];
        let mount_state = base_item_state_for_index(mount_template.item_index);
        let mut mount_state = try_item_state_from_user_item(mount_state, &host)
            .expect("captured mount Bells should hydrate");

        let inserted = try_user_item_from_item_state(&mount_state).expect("Bells should reconcile");
        assert_eq!(inserted.slots[0], None);
        assert_eq!(
            inserted.slots[1].as_ref().map(|item| item.unique_id),
            Some(9_121)
        );

        mount_state.socketed.clear();
        let removed =
            try_user_item_from_item_state(&mount_state).expect("removed Bells should clear");
        assert_eq!(removed.slots, vec![None, None]);
    }

    #[test]
    fn zero_quantity_is_representable_but_overflow_is_rejected() {
        let mut state = base_item_state();
        state.quantity = 0;

        let transient = try_user_item_from_item_state(&state)
            .expect("Crystal carrier should preserve a transient zero count");
        assert_eq!(transient.count, 0);
        validate_item_state_carrier(&state)
            .expect("carrier validation should not impose a commit-boundary invariant");

        state.quantity = u32::from(u16::MAX) + 1;

        assert!(matches!(
            try_user_item_from_item_state(&state),
            Err(UserItemCarrierError::QuantityExceeded { quantity, max: u16::MAX })
                if quantity == u32::from(u16::MAX) + 1
        ));
        assert!(matches!(
            validate_item_state_carrier(&state),
            Err(UserItemCarrierError::QuantityExceeded { .. })
        ));
    }

    #[test]
    fn committed_validator_rejects_zero_root_child_and_real_stack_overflow() {
        let mut zero_root = minimal_user_item(9_150);
        zero_root.count = 0;
        validate_user_item_carrier(&zero_root)
            .expect("generic wire carrier must continue accepting transient count zero");
        assert!(matches!(
            validate_committed_user_item_carrier(&zero_root),
            Err(UserItemCarrierError::CommittedQuantityOutOfRange {
                item_index,
                quantity: 0,
                ..
            }) if item_index == zero_root.item_index
        ));

        let mut zero_child = known_embedded_user_item(9_152, "BronzeBell");
        zero_child.count = 0;
        let mut root_with_zero_child = minimal_user_item_for_template(9_151, "RedTiger");
        root_with_zero_child.slots = vec![None, Some(zero_child)];
        validate_user_item_carrier(&root_with_zero_child)
            .expect("generic wire carrier must accept a transient zero-count child");
        assert!(matches!(
            validate_committed_user_item_carrier(&root_with_zero_child),
            Err(UserItemCarrierError::CommittedQuantityOutOfRange { quantity: 0, .. })
        ));

        let state = try_item_state_from_user_item(
            state_for_user_item(&root_with_zero_child),
            &root_with_zero_child,
        )
        .expect("generic ItemState hydration must preserve the transient zero child");
        validate_item_state_carrier(&state)
            .expect("generic ItemState carrier must continue accepting quantity zero");
        assert!(matches!(
            validate_committed_item_state_carrier(&state),
            Err(UserItemCarrierError::CommittedQuantityOutOfRange { quantity: 0, .. })
        ));

        let mut overstack = minimal_user_item(9_153);
        let max = crystal_template("BronzeHelmet").stack_size.max(1);
        overstack.count = max.saturating_add(1);
        validate_user_item_carrier(&overstack)
            .expect("generic wire carrier only validates representability");
        assert!(matches!(
            validate_committed_user_item_carrier(&overstack),
            Err(UserItemCarrierError::CommittedQuantityOutOfRange {
                quantity,
                max: committed_max,
                ..
            }) if quantity == u32::from(max) + 1 && committed_max == u32::from(max)
        ));
    }

    #[test]
    fn unknown_and_ambiguous_embedded_identity_fail_closed() {
        let mut unknown = minimal_user_item(9_130);
        unknown.item_index = i32::MAX;
        let mut unknown_host = minimal_user_item(9_129);
        unknown_host.slots = vec![Some(unknown)];
        assert!(matches!(
            validate_user_item_carrier(&unknown_host),
            Err(UserItemCarrierError::UnknownItemIndex {
                item_index: i32::MAX
            })
        ));

        let duplicate = known_embedded_user_item(9_131, "BronzeBell");
        let mut ambiguous_host = minimal_user_item(9_132);
        ambiguous_host.slots = vec![Some(duplicate.clone()), Some(duplicate)];
        assert!(matches!(
            try_item_state_from_user_item(state_for_user_item(&ambiguous_host), &ambiguous_host),
            Err(UserItemCarrierError::AmbiguousSocketIdentity {
                unique_id: 9_131,
                ..
            })
        ));
    }

    #[test]
    fn exact_item_index_and_default_optional_presence_survive_roundtrip() {
        let mut expected = minimal_user_item(9_140);
        expected.rental_information = Some(UserItemRentalInformation {
            owner_name: String::new(),
            binding_flags: 0,
            expiry_binary_datetime: 0,
            rental_locked: false,
        });
        expected.sealed_info = Some(UserItemSealedInfo {
            expiry_binary_datetime: 0,
            next_seal_binary_datetime: 0,
        });

        let state = try_item_state_from_user_item(state_for_user_item(&expected), &expected)
            .expect("protocol identity should hydrate");
        let reloaded: ItemState =
            serde_json::from_str(&serde_json::to_string(&state).expect("state should encode"))
                .expect("state should reload");
        let actual = try_user_item_from_item_state(&reloaded).expect("presence roundtrip is valid");

        assert_eq!(actual.item_index, expected.item_index);
        assert_eq!(actual.rental_information, expected.rental_information);
        assert_eq!(actual.sealed_info, expected.sealed_info);
    }

    #[test]
    fn sidecar_backed_zero_unique_id_is_preserved_without_slot_derivation() {
        let expected = minimal_user_item(0);
        let state = try_item_state_from_user_item(state_for_user_item(&expected), &expected)
            .expect("protocol identity should hydrate");

        assert_eq!(state.unique_id, 0);
        assert_eq!(
            try_user_item_from_item_state(&state)
                .expect("zero identity is preserved by the carrier")
                .unique_id,
            0
        );
    }

    #[test]
    fn signed_i32_protocol_collection_ceiling_is_explicit_for_all_counts() {
        let too_many = i32::MAX as usize + 1;
        for field in ["slots", "added_stats", "awake_values"] {
            assert!(matches!(
                check_protocol_count(field, too_many),
                Err(UserItemCarrierError::ProtocolCountExceeded {
                    field: rejected,
                    count
                }) if rejected == field && count == too_many
            ));
        }
    }

    #[test]
    fn save_carrier_to_protocol_conversion_honours_the_same_recursive_budget() {
        let expected = complex_user_item();
        let state = try_item_state_from_user_item(state_for_user_item(&expected), &expected)
            .expect("complex identity should hydrate");
        let error = try_user_item_from_item_state_with_budget(&state, budget_with(1, 8, 4))
            .expect_err("nested saved identity must obey output depth budget");
        assert!(matches!(
            error,
            UserItemCarrierError::DepthExceeded { depth: 2, max: 1 }
        ));
    }
}
