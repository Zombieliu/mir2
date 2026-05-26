use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::config::{
    CharacterRecord, ItemContainer, NpcDialogInputSnapshot, NpcDialogLinkSnapshot,
    NpcDialogSnapshot, QuestStage,
};
use bevy_ecs::prelude::World;
use mir2_game_data::{
    crystal_item_by_index, crystal_item_by_name, crystal_npc_info_by_script_key,
    crystal_npc_info_manifest, crystal_npc_script_by_key, crystal_quest_packet_manifest,
    localized_text_or_fallback, starter_server_data, CrystalItemTemplate, CrystalNpcInfoTemplate,
    CrystalNpcScript, LanguageCode, NpcScriptTemplate,
};
use mir2_protocol::{MirClass, ServerPacket, UserItem, UserItemStat};

use super::components::{
    current_player_is_dead, entity_by_object_id, entity_position, player_entity, Npc,
};
use super::crystal_compat::{
    CRYSTAL_BIND_DONT_SELL, CRYSTAL_DATA_RANGE, CRYSTAL_GOODS_BUY_BACK_MAX_STORED,
    CRYSTAL_GOODS_BUY_BACK_TIME_MINUTES, CRYSTAL_GOODS_HIDE_ADDED_STATS,
    CRYSTAL_GOODS_MAX_STORED_PER_ITEM, CRYSTAL_PANEL_BUY, CRYSTAL_PANEL_BUY_SUB,
    CRYSTAL_PANEL_CRAFT, GUIDE_NPC_ID,
};
use super::equipment::crystal_item_current_price;
use super::inventory::{
    add_or_increment_item_with_durability_and_stats, binary_datetime_ticks, can_gain_item_quantity,
    current_binary_datetime, future_binary_datetime_minutes, item_matches_inventory_unique_id,
};
use super::items::{
    crystal_item_key_for_template, crystal_item_template_for_item_key, merged_user_item_stats,
    user_item_added_attack_defence, user_item_from_item_state, ItemState,
};
use super::movement::tile_distance;
use super::npc_script::{crystal_npc_label_base, crystal_npc_labels_match, crystal_npc_section};
use super::resources::{
    InventoryResource, NpcStateResource, PlayerRuntimeResource, SessionResource,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct NpcFlagState {
    pub(super) index: u32,
    pub(super) value: bool,
}

#[derive(Debug, Clone)]
pub(super) struct ActiveNpcDialogState {
    pub(super) npc_object_id: u32,
    pub(super) npc_name: String,
    pub(super) npc_name_key: Option<String>,
    pub(super) stage: Option<QuestStage>,
    pub(super) current: u32,
    pub(super) required: u32,
    pub(super) title: String,
    pub(super) body: Vec<String>,
    pub(super) footer: String,
    pub(super) links: Vec<NpcDialogLinkState>,
    pub(super) input: Option<NpcDialogInputState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct NpcDialogLinkState {
    pub(super) text: String,
    pub(super) target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct NpcDialogInputState {
    pub(super) target: String,
    pub(super) prompt: String,
}

#[derive(Debug, Clone)]
pub(super) struct ActiveNpcServiceState {
    pub(super) script_key: String,
    pub(super) label_key: String,
    pub(super) npc_object_id: u32,
}

pub(super) fn dismiss_dialog(world: &mut World) {
    world.resource_mut::<NpcStateResource>().active_npc_dialog = None;
}

pub(super) fn set_dialog(world: &mut World, dialog: ActiveNpcDialogState) {
    let mut resources = world.resource_mut::<NpcStateResource>();
    resources.active_npc_service = None;
    resources.active_npc_dialog = Some(dialog);
}

pub(super) fn crystal_npc_input_prompt(dialog: &ActiveNpcDialogState, target: &str) -> String {
    dialog
        .links
        .iter()
        .find(|link| crystal_npc_labels_match(&link.target, target))
        .map(|link| link.text.clone())
        .unwrap_or_else(|| "Enter value".to_string())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct NpcBuyBackState {
    pub(super) script_key: String,
    pub(super) player_name: String,
    pub(super) items: Vec<NpcBuyBackItemState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct NpcBuyBackItemState {
    pub(super) item: UserItem,
    pub(super) expires_at_binary_datetime: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct NpcUsedGoodsState {
    pub(super) script_key: String,
    pub(super) items: Vec<UserItem>,
}

impl ActiveNpcDialogState {
    pub(super) fn snapshot_name(&self, language: LanguageCode) -> String {
        match self.npc_name_key.as_deref() {
            Some(key) => localized_text_or_fallback(language, key, &self.npc_name),
            None => self.npc_name.clone(),
        }
    }

    pub(super) fn snapshot_links(&self) -> Vec<NpcDialogLinkSnapshot> {
        self.links
            .iter()
            .map(|link| NpcDialogLinkSnapshot {
                text: link.text.clone(),
                target: link.target.clone(),
            })
            .collect()
    }

    pub(super) fn snapshot(&self, language: LanguageCode) -> NpcDialogSnapshot {
        if let Some(stage) = self.stage {
            if let Some((title, body, footer, _)) = super::quests::npc_stage_dialog_for_object(
                language,
                self.npc_object_id,
                stage,
                self.current,
                self.required,
            ) {
                return NpcDialogSnapshot {
                    npc_object_id: self.npc_object_id,
                    npc_name: self.snapshot_name(language),
                    title,
                    body,
                    footer,
                    links: self.snapshot_links(),
                    input: self.snapshot_input(),
                };
            }
        }

        NpcDialogSnapshot {
            npc_object_id: self.npc_object_id,
            npc_name: self.snapshot_name(language),
            title: self.title.clone(),
            body: self.body.clone(),
            footer: self.footer.clone(),
            links: self.snapshot_links(),
            input: self.snapshot_input(),
        }
    }

    pub(super) fn snapshot_input(&self) -> Option<NpcDialogInputSnapshot> {
        self.input.as_ref().map(|input| NpcDialogInputSnapshot {
            target: input.target.clone(),
            prompt: input.prompt.clone(),
        })
    }
}

pub(super) fn npc_script_for_object_id(npc_object_id: u32) -> Option<NpcScriptTemplate> {
    starter_server_data()
        .npc_scripts
        .into_iter()
        .find(|script| script.npc_object_id == npc_object_id)
}

pub(super) fn localized_npc_dialog_base_key(npc_object_id: u32) -> String {
    match npc_object_id {
        GUIDE_NPC_ID => "content.npcDialog.villageGuide".to_string(),
        _ => format!("content.npcDialog.{npc_object_id}"),
    }
}

pub(super) fn crystal_quest_ids_by_npc() -> BTreeMap<u32, BTreeSet<i32>> {
    let npc_loaded_ids = crystal_npc_info_manifest()
        .npcs
        .into_iter()
        .filter_map(|npc| Some((u32::try_from(npc.npc_index).ok()?, npc.loaded_object_id?)))
        .collect::<BTreeMap<_, _>>();
    crystal_quest_packet_manifest().quests.into_iter().fold(
        BTreeMap::<u32, BTreeSet<i32>>::new(),
        |mut by_npc, quest| {
            by_npc
                .entry(quest.npc_index)
                .or_default()
                .insert(quest.index);
            if let Some(loaded_object_id) = npc_loaded_ids.get(&quest.npc_index) {
                by_npc
                    .entry(*loaded_object_id)
                    .or_default()
                    .insert(quest.index);
            }
            by_npc
                .entry(quest.finish_npc_index)
                .or_default()
                .insert(quest.index);
            if let Some(loaded_object_id) = npc_loaded_ids.get(&quest.finish_npc_index) {
                by_npc
                    .entry(*loaded_object_id)
                    .or_default()
                    .insert(quest.index);
            }
            by_npc
        },
    )
}

pub(super) fn crystal_npc_visible_to_character(
    npc: &CrystalNpcInfoTemplate,
    character: &CharacterRecord,
) -> bool {
    if npc.flag_needed != 0 {
        return false;
    }
    if npc.min_level != 0 && i32::from(character.level) < i32::from(npc.min_level) {
        return false;
    }
    if npc.max_level != 0 && i32::from(character.level) > i32::from(npc.max_level) {
        return false;
    }
    if !npc.class_required.is_empty()
        && !npc
            .class_required
            .eq_ignore_ascii_case(crystal_class_name(character.class))
    {
        return false;
    }
    if npc.time_visible {
        return false;
    }
    true
}

pub(super) fn crystal_class_name(class: MirClass) -> &'static str {
    match class {
        MirClass::Warrior => "Warrior",
        MirClass::Wizard => "Wizard",
        MirClass::Taoist => "Taoist",
        MirClass::Assassin => "Assassin",
        MirClass::Archer => "Archer",
    }
}

pub(super) fn crystal_npc_service_object_in_range(world: &World, npc_object_id: u32) -> bool {
    let Some(npc_entity) = entity_by_object_id(world, npc_object_id) else {
        return false;
    };
    if !world.entity(npc_entity).contains::<Npc>() {
        return false;
    }

    let Some(player) = player_entity(world) else {
        return false;
    };
    let Some(player_position) = entity_position(world, player) else {
        return false;
    };
    let Some(npc_position) = entity_position(world, npc_entity) else {
        return false;
    };

    tile_distance(&player_position, &npc_position) <= CRYSTAL_DATA_RANGE
}

pub(super) fn current_crystal_npc_service_in_range(world: &World) -> Option<ActiveNpcServiceState> {
    let service = {
        world
            .resource::<NpcStateResource>()
            .active_npc_service
            .clone()
    }?;
    crystal_npc_service_object_in_range(world, service.npc_object_id).then_some(service)
}

pub(super) fn active_crystal_storage_service(world: &World) -> bool {
    current_crystal_npc_service_in_range(world)
        .is_some_and(|service| service.label_key == "STORAGE")
}

pub(super) fn push_crystal_npc_buy_back_item(
    resources: &mut NpcStateResource,
    script_key: &str,
    player_name: &str,
    item: &ItemState,
) {
    let buy_back_item = NpcBuyBackItemState {
        item: user_item_from_item_state(item),
        expires_at_binary_datetime: future_binary_datetime_minutes(
            CRYSTAL_GOODS_BUY_BACK_TIME_MINUTES,
        ),
    };
    let entry_index = resources
        .npc_buy_back_items
        .iter()
        .position(|entry| {
            entry.script_key.eq_ignore_ascii_case(script_key) && entry.player_name == player_name
        })
        .unwrap_or_else(|| {
            resources.npc_buy_back_items.push(NpcBuyBackState {
                script_key: script_key.to_string(),
                player_name: player_name.to_string(),
                items: Vec::new(),
            });
            resources.npc_buy_back_items.len() - 1
        });
    let entry = &mut resources.npc_buy_back_items[entry_index];
    if CRYSTAL_GOODS_BUY_BACK_MAX_STORED > 0
        && entry.items.len() >= CRYSTAL_GOODS_BUY_BACK_MAX_STORED
    {
        entry.items.remove(0);
    }
    entry.items.push(buy_back_item);
}

pub(super) fn push_crystal_npc_used_good_item(
    resources: &mut NpcStateResource,
    script_key: &str,
    item: UserItem,
) {
    let entry_index = resources
        .npc_used_goods_items
        .iter()
        .position(|entry| entry.script_key.eq_ignore_ascii_case(script_key))
        .unwrap_or_else(|| {
            resources.npc_used_goods_items.push(NpcUsedGoodsState {
                script_key: script_key.to_string(),
                items: Vec::new(),
            });
            resources.npc_used_goods_items.len() - 1
        });
    let entry = &mut resources.npc_used_goods_items[entry_index];
    let multi_count = entry
        .items
        .iter()
        .filter(|existing| existing.item_index == item.item_index)
        .count();
    if multi_count >= CRYSTAL_GOODS_MAX_STORED_PER_ITEM {
        if let Some(index) = entry
            .items
            .iter()
            .position(|existing| existing.added_stats.is_empty())
        {
            entry.items.remove(index);
        } else if !entry.items.is_empty() {
            entry.items.remove(0);
        }
    }
    entry.items.push(item);
}

pub(super) fn crystal_npc_buy_back_items_for_script(
    world: &World,
    script_key: &str,
) -> Vec<UserItem> {
    let player_name = current_player_name_for_market(world);
    world
        .resource::<NpcStateResource>()
        .npc_buy_back_items
        .iter()
        .find(|entry| {
            entry.script_key.eq_ignore_ascii_case(script_key) && entry.player_name == player_name
        })
        .map(|entry| entry.items.iter().map(|item| item.item.clone()).collect())
        .unwrap_or_default()
}

pub(super) fn crystal_npc_used_goods_for_script(world: &World, script_key: &str) -> Vec<UserItem> {
    world
        .resource::<NpcStateResource>()
        .npc_used_goods_items
        .iter()
        .find(|entry| entry.script_key.eq_ignore_ascii_case(script_key))
        .map(|entry| entry.items.clone())
        .unwrap_or_default()
}

pub(super) fn current_player_name_for_market(world: &World) -> String {
    world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()
        .map(|character| character.name.clone())
        .unwrap_or_default()
}

pub(super) fn process_crystal_npc_goods_expiry(world: &mut World) {
    let now = binary_datetime_ticks(current_binary_datetime());
    let expired = {
        let mut resources = world.resource_mut::<NpcStateResource>();
        let mut expired = Vec::new();
        for entry in &mut resources.npc_buy_back_items {
            let mut retained = Vec::new();
            for item in entry.items.drain(..) {
                if binary_datetime_ticks(item.expires_at_binary_datetime) <= now {
                    expired.push((entry.script_key.clone(), item.item));
                } else {
                    retained.push(item);
                }
            }
            entry.items = retained;
        }
        resources
            .npc_buy_back_items
            .retain(|entry| !entry.items.is_empty());
        expired
    };

    if expired.is_empty() {
        return;
    }

    let mut resources = world.resource_mut::<NpcStateResource>();
    for (script_key, item) in expired {
        if crystal_npc_used_goods_accepts_item(&script_key, item.item_index) {
            push_crystal_npc_used_good_item(&mut resources, &script_key, item);
        }
    }
}

pub(super) fn crystal_npc_used_goods_accepts_item(script_key: &str, item_index: i32) -> bool {
    let Some(script) = crystal_npc_script_by_key(script_key) else {
        return true;
    };
    let types = crystal_npc_script_used_item_types(&script);
    if types.is_empty() {
        return true;
    }
    crystal_item_by_index(item_index)
        .map(|template| types.contains(&template.item_type))
        .unwrap_or(false)
}

pub(super) fn crystal_npc_script_used_item_types(script: &CrystalNpcScript) -> Vec<u8> {
    let Some(section) = crystal_npc_section(script, "UsedTypes") else {
        return Vec::new();
    };

    section
        .lines
        .iter()
        .filter_map(|line| line.trim().parse::<u8>().ok())
        .collect()
}

pub(super) fn crystal_npc_script_item_types(script: &CrystalNpcScript) -> Vec<u8> {
    let Some(section) = crystal_npc_section(script, "Types") else {
        return Vec::new();
    };

    section
        .lines
        .iter()
        .filter_map(|line| line.trim().parse::<u8>().ok())
        .collect()
}

pub(super) fn crystal_npc_goods_packet_for_script(
    script: Option<&CrystalNpcScript>,
    panel_type: u8,
    hide_added_stats: bool,
) -> ServerPacket {
    let list = script
        .map(crystal_npc_trade_goods_for_script)
        .unwrap_or_default();
    crystal_npc_goods_packet(
        list,
        crystal_npc_price_rate_for_script(script),
        panel_type,
        hide_added_stats,
    )
}

pub(super) fn crystal_npc_goods_packet_for_script_with_extra(
    script: Option<&CrystalNpcScript>,
    extra_items: &[UserItem],
    panel_type: u8,
    hide_added_stats: bool,
) -> ServerPacket {
    let mut list = script
        .map(crystal_npc_trade_goods_for_script)
        .unwrap_or_default();
    list.extend(extra_items.iter().cloned());
    crystal_npc_goods_packet(
        list,
        crystal_npc_price_rate_for_script(script),
        panel_type,
        hide_added_stats,
    )
}

pub(super) fn crystal_npc_trade_goods_for_script(script: &CrystalNpcScript) -> Vec<UserItem> {
    crystal_npc_goods_for_script_section(script, "Trade")
}

pub(super) fn crystal_npc_recipe_goods_for_script(script: &CrystalNpcScript) -> Vec<UserItem> {
    crystal_npc_goods_for_script_section(script, "Recipe")
}

pub(super) fn crystal_npc_goods_for_script_section(
    script: &CrystalNpcScript,
    label: &str,
) -> Vec<UserItem> {
    let Some(section) = crystal_npc_section(script, label) else {
        return Vec::new();
    };

    section
        .lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| crystal_npc_trade_good_from_line(line, index))
        .collect()
}

pub(super) fn crystal_npc_trade_good_from_line(line: &str, line_index: usize) -> Option<UserItem> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('[') {
        return None;
    }

    let (item_name, count) = match trimmed.rsplit_once(' ') {
        Some((name, count)) => match count.trim().parse::<u16>() {
            Ok(parsed) => (name.trim(), parsed.max(1)),
            Err(_) => (trimmed, 1),
        },
        None => (trimmed, 1),
    };
    let template = crystal_item_by_name(item_name)?;
    let item_index = template.item_index.max(0) as u64;

    Some(UserItem {
        unique_id: (item_index << 16) | u64::try_from(line_index).ok()?,
        item_index: template.item_index,
        current_dura: template.durability,
        max_dura: template.durability,
        count,
        soul_bound_id: -1,
        identified: !template.need_identify,
        cursed: false,
        slots: Vec::new(),
        gem_count: 0,
        added_stats: template
            .stats
            .into_iter()
            .map(|stat| UserItemStat {
                stat: stat.stat,
                value: stat.value,
            })
            .collect(),
        awake_type: 0,
        awake_values: Vec::new(),
        refined_value: 0,
        refine_added: 0,
        refine_success_chance: 0,
        wedding_ring: -1,
        expire_info: None,
        rental_information: None,
        is_shop_item: true,
        sealed_info: None,
        gm_made: false,
    })
}

pub(super) fn crystal_npc_goods_packet(
    list: Vec<UserItem>,
    rate: f32,
    panel_type: u8,
    hide_added_stats: bool,
) -> ServerPacket {
    ServerPacket::NPCGoods {
        list,
        rate,
        panel_type,
        hide_added_stats,
    }
}

pub(super) fn crystal_npc_price_rate_for_script(script: Option<&CrystalNpcScript>) -> f32 {
    script
        .and_then(|script| crystal_npc_info_by_script_key(&script.script_key))
        .map(|npc| npc.price_rate)
        .unwrap_or(1.0)
}

pub(super) fn crystal_npc_service_label_key(label: &str) -> String {
    crystal_npc_label_base(label)
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim_start_matches('@')
        .trim()
        .to_ascii_uppercase()
}

#[cfg(test)]
pub(super) fn crystal_npc_service_packets_for_label(
    script: Option<&CrystalNpcScript>,
    label: &str,
) -> Option<Vec<ServerPacket>> {
    crystal_npc_service_packets_for_label_with_markets(script, label, &[], &[])
}

pub(super) fn crystal_npc_service_packets_for_label_with_markets(
    script: Option<&CrystalNpcScript>,
    label: &str,
    buy_back_items: &[UserItem],
    used_goods_items: &[UserItem],
) -> Option<Vec<ServerPacket>> {
    let label_key = crystal_npc_service_label_key(label);
    match label_key.as_str() {
        "BUY" => Some(vec![crystal_npc_goods_packet_for_script_with_extra(
            script,
            used_goods_items,
            CRYSTAL_PANEL_BUY,
            CRYSTAL_GOODS_HIDE_ADDED_STATS,
        )]),
        "BUYNEW" => Some(vec![crystal_npc_goods_packet_for_script(
            script,
            CRYSTAL_PANEL_BUY,
            CRYSTAL_GOODS_HIDE_ADDED_STATS,
        )]),
        "BUYBACK" => Some(vec![crystal_npc_goods_packet(
            buy_back_items.to_vec(),
            crystal_npc_price_rate_for_script(script),
            CRYSTAL_PANEL_BUY,
            false,
        )]),
        "BUYUSED" => Some(vec![crystal_npc_goods_packet(
            used_goods_items.to_vec(),
            crystal_npc_price_rate_for_script(script),
            CRYSTAL_PANEL_BUY_SUB,
            CRYSTAL_GOODS_HIDE_ADDED_STATS,
        )]),
        "BUYSELL" => Some(vec![
            crystal_npc_goods_packet_for_script_with_extra(
                script,
                used_goods_items,
                CRYSTAL_PANEL_BUY,
                CRYSTAL_GOODS_HIDE_ADDED_STATS,
            ),
            ServerPacket::NPCSell,
        ]),
        "BUYSELLNEW" => Some(vec![
            crystal_npc_goods_packet_for_script(
                script,
                CRYSTAL_PANEL_BUY,
                CRYSTAL_GOODS_HIDE_ADDED_STATS,
            ),
            ServerPacket::NPCSell,
        ]),
        "SELL" => Some(vec![ServerPacket::NPCSell]),
        "REPAIR" => Some(vec![ServerPacket::NPCRepair {
            rate: crystal_npc_price_rate_for_script(script),
        }]),
        "SREPAIR" => Some(vec![ServerPacket::NPCSRepair {
            rate: crystal_npc_price_rate_for_script(script),
        }]),
        "CRAFT" => Some(vec![crystal_npc_goods_packet(
            script
                .map(crystal_npc_recipe_goods_for_script)
                .unwrap_or_default(),
            crystal_npc_price_rate_for_script(script),
            CRYSTAL_PANEL_CRAFT,
            false,
        )]),
        "REFINE" => Some(vec![ServerPacket::NPCRefine {
            rate: 1.0,
            refining: false,
        }]),
        "REFINECHECK" => Some(vec![ServerPacket::NPCCheckRefine]),
        "REPLACEWEDDINGRING" => Some(vec![ServerPacket::NPCReplaceWedRing { rate: 1.0 }]),
        "STORAGE" => Some(vec![ServerPacket::NPCStorage]),
        _ => None,
    }
}

pub(super) fn record_crystal_npc_service_context(
    world: &mut World,
    npc_object_id: u32,
    script: &CrystalNpcScript,
    label: &str,
    packets: &[ServerPacket],
) {
    if !packets.iter().any(|packet| {
        matches!(
            packet,
            ServerPacket::NPCSell
                | ServerPacket::NPCGoods { .. }
                | ServerPacket::NPCRepair { .. }
                | ServerPacket::NPCSRepair { .. }
                | ServerPacket::NPCStorage
        )
    }) {
        return;
    }

    let label_key = crystal_npc_service_label_key(label);
    let mut resources = world.resource_mut::<NpcStateResource>();
    resources.active_npc_service = Some(ActiveNpcServiceState {
        script_key: script.script_key.clone(),
        label_key: label_key.clone(),
        npc_object_id,
    });
    drop(resources);
    if label_key == "STORAGE" {
        world.resource_mut::<InventoryResource>().storage_unlocked = false;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CrystalNpcPurchaseSource {
    Trade,
    BuyBack,
    Used,
}

pub(super) struct CrystalNpcPurchaseItem {
    item: UserItem,
    source: CrystalNpcPurchaseSource,
}

pub(super) fn active_crystal_buy_service(service: &ActiveNpcServiceState) -> bool {
    matches!(
        service.label_key.as_str(),
        "BUY" | "BUYSELL" | "BUYBACK" | "BUYUSED" | "PEARLBUY" | "BUYNEW" | "BUYSELLNEW"
    )
}

pub(super) fn buy_item_impl(
    world: &mut World,
    item_index: u64,
    count: u16,
    panel_type: u8,
) -> Vec<ServerPacket> {
    if count == 0 || panel_type != CRYSTAL_PANEL_BUY {
        return Vec::new();
    }
    if current_player_is_dead(world) {
        return Vec::new();
    }

    let Some(service) =
        current_crystal_npc_service_in_range(world).filter(active_crystal_buy_service)
    else {
        return Vec::new();
    };

    process_crystal_npc_goods_expiry(world);
    let rate = crystal_npc_info_by_script_key(&service.script_key)
        .map(|npc| npc.price_rate)
        .unwrap_or(1.0);
    let Some(purchase_item) = crystal_npc_service_item_for_purchase(world, &service, item_index)
    else {
        return Vec::new();
    };
    let source_item = purchase_item.item.clone();
    let Some(template) = crystal_item_by_index(source_item.item_index) else {
        return Vec::new();
    };

    let requested_count = u32::from(count);
    let is_resale = matches!(
        purchase_item.source,
        CrystalNpcPurchaseSource::BuyBack | CrystalNpcPurchaseSource::Used
    );
    let buy_count = if is_resale {
        requested_count.min(u32::from(source_item.count.max(1)))
    } else {
        requested_count
    };
    if buy_count == 0 || buy_count > u32::from(template.stack_size.max(1)) {
        return Vec::new();
    }

    let key = crystal_item_key_for_template(&template);
    let cost = crystal_npc_purchase_cost(&template, buy_count, rate);
    let player_name = world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()
        .map(|character| character.name.clone())
        .unwrap_or_default();
    {
        let resources = world.resource::<InventoryResource>();
        if world.resource::<PlayerRuntimeResource>().gold < cost {
            return Vec::new();
        }
        if !can_gain_item_quantity(&resources, ItemContainer::Bag1, &key, buy_count) {
            return Vec::new();
        }
    }

    {
        world.resource_mut::<PlayerRuntimeResource>().gold -= cost;
    }
    match purchase_item.source {
        CrystalNpcPurchaseSource::BuyBack => {
            remove_crystal_npc_buy_back_item(
                &mut world.resource_mut::<NpcStateResource>(),
                &service.script_key,
                &player_name,
                item_index,
            );
        }
        CrystalNpcPurchaseSource::Used => {
            remove_crystal_npc_used_good_item(
                &mut world.resource_mut::<NpcStateResource>(),
                &service.script_key,
                item_index,
            );
        }
        CrystalNpcPurchaseSource::Trade => {}
    }

    let (added_attack, added_defence) = user_item_added_attack_defence(&source_item);
    let gained = add_or_increment_item_with_durability_and_stats(
        world,
        ItemContainer::Bag1,
        &key,
        &template.name,
        template
            .tooltip
            .as_deref()
            .unwrap_or("Crystal NPC shop item."),
        8,
        buy_count,
        u16::from(template.weight.max(1)),
        Some(source_item.current_dura),
        Some(source_item.max_dura),
        added_attack,
        added_defence,
    );

    let mut packets = vec![
        ServerPacket::LoseGold { gold: cost },
        ServerPacket::GainedItem {
            item: user_item_from_item_state(&gained),
        },
    ];
    match purchase_item.source {
        CrystalNpcPurchaseSource::BuyBack => {
            packets.push(crystal_npc_goods_packet(
                crystal_npc_buy_back_items_for_script(world, &service.script_key),
                rate,
                CRYSTAL_PANEL_BUY,
                false,
            ));
        }
        CrystalNpcPurchaseSource::Used => {
            let script = crystal_npc_script_by_key(&service.script_key);
            let used_goods = crystal_npc_used_goods_for_script(world, &service.script_key);
            let panel_type = if service.label_key == "BUYUSED" {
                CRYSTAL_PANEL_BUY_SUB
            } else {
                CRYSTAL_PANEL_BUY
            };
            packets.push(crystal_npc_goods_packet_for_script_with_extra(
                script.as_ref(),
                &used_goods,
                panel_type,
                CRYSTAL_GOODS_HIDE_ADDED_STATS,
            ));
        }
        CrystalNpcPurchaseSource::Trade => {}
    }
    packets
}

pub(super) fn crystal_npc_service_item_for_purchase(
    world: &World,
    service: &ActiveNpcServiceState,
    item_index: u64,
) -> Option<CrystalNpcPurchaseItem> {
    if service.label_key == "BUYBACK" {
        return crystal_npc_buy_back_items_for_script(world, &service.script_key)
            .into_iter()
            .find(|item| item.unique_id == item_index)
            .map(|item| CrystalNpcPurchaseItem {
                item,
                source: CrystalNpcPurchaseSource::BuyBack,
            });
    }

    if service.label_key != "BUYUSED" {
        if let Some(item) = crystal_npc_script_by_key(&service.script_key)
            .map(|script| crystal_npc_trade_goods_for_script(&script))
            .unwrap_or_default()
            .into_iter()
            .find(|item| item.unique_id == item_index)
        {
            return Some(CrystalNpcPurchaseItem {
                item,
                source: CrystalNpcPurchaseSource::Trade,
            });
        }
    }

    crystal_npc_used_goods_for_script(world, &service.script_key)
        .into_iter()
        .find(|item| item.unique_id == item_index)
        .map(|item| CrystalNpcPurchaseItem {
            item,
            source: CrystalNpcPurchaseSource::Used,
        })
}

pub(super) fn crystal_npc_purchase_cost(
    template: &CrystalItemTemplate,
    count: u32,
    rate: f32,
) -> u32 {
    let base_cost = template.price.saturating_mul(count);
    ((base_cost as f32) * rate).floor() as u32
}

pub(super) fn remove_crystal_npc_buy_back_item(
    resources: &mut NpcStateResource,
    script_key: &str,
    player_name: &str,
    unique_id: u64,
) {
    if let Some(entry) = resources.npc_buy_back_items.iter_mut().find(|entry| {
        entry.script_key.eq_ignore_ascii_case(script_key) && entry.player_name == player_name
    }) {
        if let Some(index) = entry
            .items
            .iter()
            .position(|item| item.item.unique_id == unique_id)
        {
            entry.items.remove(index);
        }
    }
}

pub(super) fn remove_crystal_npc_used_good_item(
    resources: &mut NpcStateResource,
    script_key: &str,
    unique_id: u64,
) {
    if let Some(entry) = resources
        .npc_used_goods_items
        .iter_mut()
        .find(|entry| entry.script_key.eq_ignore_ascii_case(script_key))
    {
        if let Some(index) = entry
            .items
            .iter()
            .position(|item| item.unique_id == unique_id)
        {
            entry.items.remove(index);
        }
    }
}

pub(super) fn active_crystal_sell_service(service: &ActiveNpcServiceState) -> bool {
    matches!(service.label_key.as_str(), "SELL" | "BUYSELL")
}

pub(super) fn sell_item_impl(world: &mut World, unique_id: u64, count: u16) -> Vec<ServerPacket> {
    let failed_packet = ServerPacket::SellItem {
        unique_id,
        count,
        success: false,
    };

    if current_player_is_dead(world) || count == 0 {
        return vec![failed_packet];
    }

    let Some(service) =
        current_crystal_npc_service_in_range(world).filter(active_crystal_sell_service)
    else {
        return vec![failed_packet];
    };
    let player_name = world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()
        .map(|character| character.name.clone())
        .unwrap_or_default();

    let current_gold = world.resource::<PlayerRuntimeResource>().gold;
    let sold = {
        let mut resources = world.resource_mut::<InventoryResource>();
        let Some(index) = resources
            .inventory_items
            .iter()
            .position(|item| item_matches_inventory_unique_id(item, unique_id))
        else {
            return vec![failed_packet];
        };
        let requested = u32::from(count);
        let item_count = resources.inventory_items[index].quantity;
        if requested > item_count {
            return vec![failed_packet];
        }

        let mut item = resources.inventory_items[index].clone();
        item.quantity = requested;
        let template = crystal_item_template_for_item_key(&item.key);

        if template
            .as_ref()
            .is_some_and(|template| template.bind & CRYSTAL_BIND_DONT_SELL != 0)
        {
            return vec![failed_packet];
        }

        if let Some(script) = crystal_npc_script_by_key(&service.script_key) {
            let sell_types = crystal_npc_script_item_types(&script);
            if !sell_types.is_empty()
                && !template
                    .as_ref()
                    .is_some_and(|template| sell_types.contains(&template.item_type))
            {
                return vec![
                    super::session::system_message_key(world, "server.CannotSellItemHere"),
                    failed_packet,
                ];
            }
        }

        let value = crystal_sell_value_for_item(&item);
        let partial_crystal_stack = template
            .as_ref()
            .is_some_and(|template| template.stack_size > 1)
            && requested != item_count;
        if partial_crystal_stack
            && u64::from(current_gold).saturating_add(u64::from(value)) > u64::from(u32::MAX)
        {
            return vec![failed_packet];
        }

        if requested == item_count {
            resources.inventory_items.remove(index);
        } else {
            resources.inventory_items[index].quantity -= requested;
        }

        (value, value != 0, item)
    };
    let gained_gold = {
        let mut player_runtime = world.resource_mut::<PlayerRuntimeResource>();
        let gained_gold = sold.0.min(u32::MAX.saturating_sub(player_runtime.gold));
        player_runtime.gold = player_runtime.gold.saturating_add(gained_gold);
        gained_gold
    };
    push_crystal_npc_buy_back_item(
        &mut world.resource_mut::<NpcStateResource>(),
        &service.script_key,
        &player_name,
        &sold.2,
    );

    let mut packets = vec![ServerPacket::SellItem {
        unique_id,
        count,
        success: true,
    }];
    if sold.1 {
        packets.push(ServerPacket::GainedGold { gold: gained_gold });
    }
    packets
}

pub(super) fn crystal_sell_value_for_item(item: &ItemState) -> u32 {
    crystal_item_template_for_item_key(&item.key)
        .map(|template| {
            let added_stat_count = merged_user_item_stats(
                &item.added_stats,
                item.added_defence,
                item.added_attack,
                None,
            )
            .len();
            crystal_item_current_price(item, &template, added_stat_count) / 2
        })
        .unwrap_or_else(|| u32::from(item.weight.max(1)) * item.quantity.max(1))
}
