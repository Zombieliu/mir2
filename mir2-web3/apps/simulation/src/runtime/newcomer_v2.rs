use std::collections::BTreeSet;
use std::sync::OnceLock;

use bevy_ecs::prelude::World;
use mir2_game_data::{
    crystal_item_by_name, CrystalItemTemplate, CrystalQuestFlagTaskTemplate,
    CrystalQuestKillTaskTemplate, CrystalQuestPacketTemplate,
};
use mir2_protocol::{ClientQuestInfo, MirClass, MirGender, QuestItemReward};
use serde::Deserialize;

use super::super::items::item_info_from_crystal_template;
use super::super::resources::{QuestResource, SessionResource};

const CONFIG_JSON: &str =
    include_str!("../../../../config/quest-guidance/newcomer-journey-v2.json");

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::runtime) struct V2FlagObjective {
    pub(in crate::runtime) number: i32,
    pub(in crate::runtime) message: String,
    pub(in crate::runtime) conditions: Vec<String>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    schema: u8,
    profile: String,
    max_level: i32,
    quests: Vec<QuestConfig>,
    growth_rewards: Vec<GrowthConfig>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuestConfig {
    id: i32,
    order: i32,
    title: String,
    min_level: i32,
    required_quest_id: i32,
    #[serde(default)]
    after_quest_ids: Vec<i32>,
    start_npc_id: u32,
    finish_npc_id: u32,
    #[serde(default)]
    maps: Vec<String>,
    experience: u32,
    gold: u32,
    #[serde(default)]
    kills: Vec<KillConfig>,
    #[serde(default)]
    flags: Vec<FlagConfig>,
    rewards: RewardSet,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GrowthConfig {
    id: i32,
    level: i32,
    after_quest_id: i32,
    finish_npc_id: u32,
    gold: u32,
    experience: u32,
    rewards: RewardSet,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KillConfig {
    monster: String,
    monster_index: i32,
    count: u32,
    message: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FlagConfig {
    number: i32,
    message: String,
    kind: FlagKind,
    #[serde(default)]
    conditions: Vec<String>,
    #[serde(default)]
    requirements: Option<ClassRequirements>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum FlagKind {
    ServerEvent,
    ClassPractice,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClassRequirements {
    warrior: Vec<String>,
    wizard: Vec<String>,
    taoist: Vec<String>,
}

impl ClassRequirements {
    fn for_class(&self, class: MirClass) -> &[String] {
        match class {
            MirClass::Warrior => &self.warrior,
            MirClass::Wizard => &self.wizard,
            MirClass::Taoist => &self.taoist,
            MirClass::Assassin | MirClass::Archer => &[],
        }
    }

    fn valid(&self) -> bool {
        !self.warrior.is_empty() && !self.wizard.is_empty() && !self.taoist.is_empty()
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RewardSet {
    #[serde(default)]
    common: Vec<RewardItem>,
    #[serde(default)]
    warrior: Vec<RewardItem>,
    #[serde(default)]
    wizard: Vec<RewardItem>,
    #[serde(default)]
    taoist: Vec<RewardItem>,
}

impl RewardSet {
    fn class_items(&self, class: MirClass) -> &[RewardItem] {
        match class {
            MirClass::Warrior => &self.warrior,
            MirClass::Wizard => &self.wizard,
            MirClass::Taoist => &self.taoist,
            MirClass::Assassin | MirClass::Archer => &[],
        }
    }

    fn all(&self) -> impl Iterator<Item = &RewardItem> {
        self.common
            .iter()
            .chain(self.warrior.iter())
            .chain(self.wizard.iter())
            .chain(self.taoist.iter())
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RewardItem {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    male_item: Option<String>,
    #[serde(default)]
    female_item: Option<String>,
    count: u16,
}

impl RewardItem {
    fn name_for(&self, gender: MirGender) -> Option<&str> {
        self.name.as_deref().or_else(|| match gender {
            MirGender::Male => self.male_item.as_deref(),
            MirGender::Female => self.female_item.as_deref(),
        })
    }

    fn valid(&self) -> bool {
        let plain = self.name.is_some() && self.male_item.is_none() && self.female_item.is_none();
        let gendered =
            self.name.is_none() && self.male_item.is_some() && self.female_item.is_some();
        self.count > 0
            && (plain || gendered)
            && [
                self.name.as_deref(),
                self.male_item.as_deref(),
                self.female_item.as_deref(),
            ]
            .into_iter()
            .flatten()
            .all(|name| crystal_item_by_name(name).is_some())
    }
}

struct Definition {
    info: ClientQuestInfo,
    template: CrystalQuestPacketTemplate,
    dependencies: Vec<i32>,
    flags: Vec<FlagConfig>,
    rewards: RewardSet,
    maps: Vec<String>,
}

pub(in crate::runtime) fn enabled(world: &World) -> bool {
    world
        .get_resource::<QuestResource>()
        .is_some_and(|quests| quests.newcomer_v2_cadence && !has_v1_claim_or_profile(quests))
}

pub(in crate::runtime) fn is_v2_quest(quest_id: i32) -> bool {
    definition(quest_id).is_some()
}

pub(in crate::runtime) fn has_v2_progress(world: &World) -> bool {
    world.get_resource::<QuestResource>().is_some_and(|quests| {
        quests.quests.iter().any(|quest| {
            quest.stage != crate::config::QuestStage::Available && is_v2_quest(quest.quest_id)
        })
    })
}

pub(in crate::runtime) fn quest_info_by_id(
    world: &World,
    quest_id: i32,
) -> Option<ClientQuestInfo> {
    if !enabled(world) {
        return None;
    }
    let definition = definition(quest_id)?;
    let mut info = definition.info.clone();
    apply_rewards(world, &definition.rewards, &mut info);
    Some(info)
}

/// Class and gender rewards are intentionally added only by quest_info_by_id.
pub(in crate::runtime) fn quest_info_by_id_unchecked(quest_id: i32) -> Option<ClientQuestInfo> {
    definition(quest_id).map(|definition| definition.info.clone())
}

pub(in crate::runtime) fn quest_template_by_id(
    world: &World,
    quest_id: i32,
) -> Option<CrystalQuestPacketTemplate> {
    enabled(world)
        .then(|| definition(quest_id).map(|definition| definition.template.clone()))
        .flatten()
}

pub(in crate::runtime) fn quest_infos(world: &World) -> Vec<ClientQuestInfo> {
    if !enabled(world) {
        return Vec::new();
    }
    definitions()
        .iter()
        .map(|definition| {
            let mut info = definition.info.clone();
            apply_rewards(world, &definition.rewards, &mut info);
            info
        })
        .collect()
}

pub(in crate::runtime) fn quest_ids_for_npc(world: &World, npc_id: u32) -> Vec<i32> {
    enabled(world)
        .then(|| ids_for_npc(npc_id))
        .unwrap_or_default()
}

pub(in crate::runtime) fn configured_quest_ids_for_npc(npc_id: u32) -> Vec<i32> {
    if super::quest_recurrence::server_newcomer_v2_enabled() {
        ids_for_npc(npc_id)
    } else {
        Vec::new()
    }
}

pub(in crate::runtime) fn can_accept(world: &World, quest_id: i32) -> bool {
    if !enabled(world) {
        return false;
    }
    let Some(definition) = definition(quest_id) else {
        return false;
    };
    let Some(character) = selected_character(world) else {
        return false;
    };
    if !matches!(
        character.class,
        MirClass::Warrior | MirClass::Wizard | MirClass::Taoist
    ) {
        return false;
    }
    let level = i32::from(character.level);
    level >= definition.info.min_level_needed
        && (definition.info.max_level_needed <= 0 || level <= definition.info.max_level_needed)
        && definition
            .dependencies
            .iter()
            .all(|id| completed(world, *id))
}

pub(in crate::runtime) fn can_finish(world: &World, quest_id: i32) -> bool {
    if !is_v2_quest(quest_id) {
        return true;
    }
    can_accept(world, quest_id)
}

pub(in crate::runtime) fn flag_objectives(world: &World, quest_id: i32) -> Vec<V2FlagObjective> {
    if !enabled(world) {
        return Vec::new();
    }
    let Some(character) = selected_character(world) else {
        return Vec::new();
    };
    definition(quest_id)
        .into_iter()
        .flat_map(|definition| definition.flags.iter())
        .filter_map(|flag| {
            let conditions = match flag.kind {
                FlagKind::ServerEvent => flag.conditions.clone(),
                FlagKind::ClassPractice => flag
                    .requirements
                    .as_ref()
                    .map(|requirements| requirements.for_class(character.class).to_vec())
                    .unwrap_or_default(),
            };
            (!conditions.is_empty()).then(|| V2FlagObjective {
                number: flag.number,
                message: flag.message.clone(),
                conditions,
            })
        })
        .collect()
}

/// V2 bridge events must pass this exact, configured map check before they can
/// change a task. Missing map metadata deliberately fails closed.
pub(in crate::runtime) fn objective_map_matches(
    world: &World,
    quest_id: i32,
    map_file_name: &str,
) -> bool {
    enabled(world)
        && definition(quest_id).is_some_and(|definition| {
            definition
                .maps
                .iter()
                .any(|map| map.eq_ignore_ascii_case(map_file_name.trim()))
        })
}

fn selected_character(world: &World) -> Option<&crate::config::CharacterRecord> {
    world
        .get_resource::<SessionResource>()
        .and_then(|session| session.selected_character.as_ref())
}

fn completed(world: &World, quest_id: i32) -> bool {
    world
        .resource::<QuestResource>()
        .quests
        .iter()
        .any(|quest| {
            quest.quest_id == quest_id && quest.stage == crate::config::QuestStage::Completed
        })
}

fn has_v1_claim_or_profile(quests: &QuestResource) -> bool {
    quests.newcomer_v1_cadence
        || quests.quests.iter().any(|quest| {
            quest.stage != crate::config::QuestStage::Available
                && (super::newcomer_progression::is_newcomer_quest(quest.quest_id)
                    || super::newcomer_progression::is_journey_quest(quest.quest_id)
                    || quest.quest_id == super::super::crystal_compat::GUIDE_QUEST_ID)
        })
}

fn ids_for_npc(npc_id: u32) -> Vec<i32> {
    definitions()
        .iter()
        .filter(|definition| {
            definition.info.npc_index == npc_id || definition.info.finish_npc_index == npc_id
        })
        .map(|definition| definition.info.index)
        .collect()
}

fn definition(quest_id: i32) -> Option<&'static Definition> {
    definitions()
        .iter()
        .find(|definition| definition.info.index == quest_id)
}

fn definitions() -> &'static [Definition] {
    static DEFINITIONS: OnceLock<Vec<Definition>> = OnceLock::new();
    DEFINITIONS.get_or_init(build_definitions)
}

fn build_definitions() -> Vec<Definition> {
    let Ok(config) = serde_json::from_str::<Config>(CONFIG_JSON) else {
        return Vec::new();
    };
    if config.schema != 1
        || !config.profile.eq_ignore_ascii_case("newcomer-v2")
        || config.max_level < 0
        || config.quests.len() != 22
        || config.growth_rewards.len() != 4
    {
        return Vec::new();
    }
    let mut ids = BTreeSet::new();
    let mut orders = BTreeSet::new();
    let mut definitions = Vec::with_capacity(26);
    let max_level = config.max_level;
    for quest in config.quests {
        if !ids.insert(quest.id)
            || !orders.insert(quest.order)
            || !(1..=22).contains(&quest.order)
            || !valid_endpoint(quest.start_npc_id)
            || !valid_endpoint(quest.finish_npc_id)
            || !valid_quest(&quest)
        {
            return Vec::new();
        }
        definitions.push(main_definition(max_level, quest));
    }
    if !(1..=22).all(|order| orders.contains(&order)) {
        return Vec::new();
    }
    for growth in config.growth_rewards {
        if !ids.insert(growth.id)
            || ![15, 20, 25, 30].contains(&growth.level)
            || growth.after_quest_id <= 0
            || !valid_endpoint(growth.finish_npc_id)
            || !growth.rewards.all().all(RewardItem::valid)
        {
            return Vec::new();
        }
        definitions.push(growth_definition(max_level, growth));
    }
    definitions.sort_by_key(|definition| definition.info.index);
    definitions
}

fn valid_endpoint(id: u32) -> bool {
    matches!(id, 0 | 3 | 24)
}

fn valid_quest(quest: &QuestConfig) -> bool {
    quest.id > 0
        && (1..=30).contains(&quest.min_level)
        && quest.maps.iter().all(|map| !map.trim().is_empty())
        && quest.kills.iter().all(|kill| {
            kill.monster_index > 0
                && kill.count > 0
                && !kill.monster.trim().is_empty()
                && !kill.message.trim().is_empty()
        })
        && quest.flags.iter().all(valid_flag)
        && quest.rewards.all().all(RewardItem::valid)
}

fn valid_flag(flag: &FlagConfig) -> bool {
    flag.number >= 2_210_000
        && !flag.message.trim().is_empty()
        && match flag.kind {
            FlagKind::ServerEvent => {
                !flag.conditions.is_empty()
                    && flag
                        .conditions
                        .iter()
                        .all(|condition| !condition.trim().is_empty())
            }
            FlagKind::ClassPractice => flag
                .requirements
                .as_ref()
                .is_some_and(ClassRequirements::valid),
        }
}

fn main_definition(max_level: i32, quest: QuestConfig) -> Definition {
    let mut dependencies = quest.after_quest_ids.clone();
    if quest.required_quest_id > 0 {
        dependencies.push(quest.required_quest_id);
    }
    dependencies.retain(|id| *id > 0);
    dependencies.sort_unstable();
    dependencies.dedup();
    let tasks = task_lines(&quest);
    let group = format!("Newcomer V2 Node {}", quest.order);
    let info = ClientQuestInfo {
        index: quest.id,
        npc_index: quest.start_npc_id,
        name: quest.title.clone(),
        group: group.clone(),
        description: vec![format!("Newcomer V2 main node {}.", quest.order)],
        task_description: tasks,
        return_description: vec![return_line(quest.finish_npc_id)],
        completion_description: vec!["Main-node reward claimed.".to_string()],
        min_level_needed: quest.min_level,
        max_level_needed: max_level,
        quest_needed: quest.required_quest_id,
        class_needed: 0,
        quest_type: 0,
        time_limit_in_seconds: 0,
        reward_gold: quest.gold,
        reward_exp: quest.experience,
        reward_credit: 0,
        rewards_fixed_item: Vec::new(),
        rewards_select_item: Vec::new(),
        finish_npc_index: quest.finish_npc_id,
    };
    let template = CrystalQuestPacketTemplate {
        index: quest.id,
        name: quest.title,
        group,
        file_name: format!("NewcomerV2/Node{}", quest.order),
        required_min_level: quest.min_level,
        required_max_level: max_level,
        required_quest: quest.required_quest_id,
        required_class: 0,
        quest_type: 0,
        goto_message: return_line(quest.finish_npc_id),
        kill_message: String::new(),
        item_message: String::new(),
        flag_message: String::new(),
        time_limit_in_seconds: 0,
        npc_index: quest.start_npc_id,
        finish_npc_index: quest.finish_npc_id,
        carry_items: Vec::new(),
        kill_tasks: quest
            .kills
            .iter()
            .map(|kill| CrystalQuestKillTaskTemplate {
                monster_index: kill.monster_index,
                monster_name: kill.monster.clone(),
                count: kill.count,
                message: kill.message.clone(),
            })
            .collect(),
        item_tasks: Vec::new(),
        flag_tasks: quest
            .flags
            .iter()
            .map(|flag| CrystalQuestFlagTaskTemplate {
                number: flag.number,
                message: flag.message.clone(),
            })
            .collect(),
        payload_len: 0,
        payload_hex: String::new(),
    };
    Definition {
        info,
        template,
        dependencies,
        flags: quest.flags,
        rewards: quest.rewards,
        maps: quest.maps,
    }
}

fn growth_definition(max_level: i32, growth: GrowthConfig) -> Definition {
    let title = format!("Level {} Growth Reward", growth.level);
    let group = "Newcomer V2 Growth".to_string();
    let info = ClientQuestInfo {
        index: growth.id,
        npc_index: growth.finish_npc_id,
        name: title.clone(),
        group: group.clone(),
        description: vec![format!(
            "Claim after node {} at level {}.",
            growth.after_quest_id - 2_110_000,
            growth.level
        )],
        task_description: vec!["No automatic completion condition.".to_string()],
        return_description: vec![return_line(growth.finish_npc_id)],
        completion_description: vec!["Growth reward claimed.".to_string()],
        min_level_needed: growth.level,
        max_level_needed: max_level,
        quest_needed: growth.after_quest_id,
        class_needed: 0,
        quest_type: 0,
        time_limit_in_seconds: 0,
        reward_gold: growth.gold,
        reward_exp: growth.experience,
        reward_credit: 0,
        rewards_fixed_item: Vec::new(),
        rewards_select_item: Vec::new(),
        finish_npc_index: growth.finish_npc_id,
    };
    let template = CrystalQuestPacketTemplate {
        index: growth.id,
        name: title,
        group,
        file_name: format!("NewcomerV2/Growth{}", growth.level),
        required_min_level: growth.level,
        required_max_level: max_level,
        required_quest: growth.after_quest_id,
        required_class: 0,
        quest_type: 0,
        goto_message: return_line(growth.finish_npc_id),
        kill_message: String::new(),
        item_message: String::new(),
        flag_message: String::new(),
        time_limit_in_seconds: 0,
        npc_index: growth.finish_npc_id,
        finish_npc_index: growth.finish_npc_id,
        carry_items: Vec::new(),
        kill_tasks: Vec::new(),
        item_tasks: Vec::new(),
        flag_tasks: Vec::new(),
        payload_len: 0,
        payload_hex: String::new(),
    };
    Definition {
        info,
        template,
        dependencies: vec![growth.after_quest_id],
        flags: Vec::new(),
        rewards: growth.rewards,
        maps: Vec::new(),
    }
}

fn task_lines(quest: &QuestConfig) -> Vec<String> {
    let mut lines = quest
        .kills
        .iter()
        .map(|kill| kill.message.clone())
        .collect::<Vec<_>>();
    lines.extend(quest.flags.iter().map(|flag| flag.message.clone()));
    if lines.is_empty() {
        lines.push("Return when the server-confirmed requirement is complete.".to_string());
    }
    lines
}

fn return_line(npc_id: u32) -> String {
    match npc_id {
        0 => "Return to the Quest Diary.".to_string(),
        3 => "Return to Assistant Jane.".to_string(),
        24 => "Return to the Bichon Wall Board.".to_string(),
        _ => "Return to the quest endpoint.".to_string(),
    }
}

fn apply_rewards(world: &World, rewards: &RewardSet, info: &mut ClientQuestInfo) {
    let Some(character) = selected_character(world) else {
        return;
    };
    info.rewards_fixed_item.extend(
        rewards
            .common
            .iter()
            .chain(rewards.class_items(character.class))
            .filter_map(|reward| reward_packet(reward, character.gender)),
    );
}

fn reward_packet(reward: &RewardItem, gender: MirGender) -> Option<QuestItemReward> {
    let template: CrystalItemTemplate = crystal_item_by_name(reward.name_for(gender)?)?;
    Some(QuestItemReward {
        item: item_info_from_crystal_template(template),
        count: reward.count,
    })
}

#[cfg(test)]
#[path = "newcomer_v2_tests.rs"]
mod newcomer_v2_tests;
