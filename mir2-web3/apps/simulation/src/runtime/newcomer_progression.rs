use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use bevy_ecs::prelude::World;
use mir2_game_data::{
    crystal_item_by_name, CrystalItemTemplate, CrystalQuestKillTaskTemplate,
    CrystalQuestPacketTemplate,
};
use mir2_protocol::{ClientQuestInfo, MirClass, MirGender, QuestItemReward};
use serde::Deserialize;

use crate::config::QuestStage;

use super::super::items::item_info_from_crystal_template;
use super::super::resources::{QuestResource, SessionResource};

pub(super) const DAILY_OPTION_IDS: [i32; 3] = [2_100_001, 2_100_002, 2_100_003];
pub(super) const DAILY_BONUS_ID: i32 = 2_100_004;
pub(super) const MILESTONE_IDS: [i32; 6] = [
    2_100_015, 2_100_020, 2_100_025, 2_100_030, 2_100_035, 2_100_040,
];
const REWARDED_MILESTONE_LEVELS: [u16; 3] = [15, 20, 25];
pub(super) const NEWCOMER_BOARD_OBJECT_ID: u32 = 24;

const NEWCOMER_JOURNEY_JSON: &str =
    include_str!("../../../../config/quest-guidance/newcomer-journey-v1.json");

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewcomerJourneyFile {
    schema: u8,
    profile: String,
    max_level: u16,
    #[serde(default)]
    milestone_rewards: Vec<NewcomerMilestoneReward>,
    #[serde(default)]
    quest_overrides: Vec<NewcomerQuestOverride>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewcomerMilestoneReward {
    level: u16,
    class_rewards: NewcomerClassRewards,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct NewcomerClassRewards {
    warrior: Vec<NewcomerRewardItem>,
    wizard: Vec<NewcomerRewardItem>,
    taoist: Vec<NewcomerRewardItem>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewcomerRewardItem {
    #[serde(default)]
    item: Option<String>,
    #[serde(default)]
    male_item: Option<String>,
    #[serde(default)]
    female_item: Option<String>,
    count: u16,
}

impl NewcomerRewardItem {
    fn configured_item_names(&self) -> impl Iterator<Item = &str> {
        self.item
            .iter()
            .chain(self.male_item.iter())
            .chain(self.female_item.iter())
            .map(String::as_str)
    }

    fn item_name_for_gender(&self, gender: MirGender) -> Option<&str> {
        self.item.as_deref().or_else(|| match gender {
            MirGender::Male => self.male_item.as_deref(),
            MirGender::Female => self.female_item.as_deref(),
        })
    }

    fn is_valid(&self) -> bool {
        let plain = self.item.is_some() && self.male_item.is_none() && self.female_item.is_none();
        let gendered =
            self.item.is_none() && self.male_item.is_some() && self.female_item.is_some();
        self.count > 0
            && (plain || gendered)
            && self
                .configured_item_names()
                .all(|name| crystal_item_by_name(name).is_some())
    }
}

impl NewcomerClassRewards {
    fn for_class(&self, class: MirClass) -> Option<&[NewcomerRewardItem]> {
        match class {
            MirClass::Warrior => Some(&self.warrior),
            MirClass::Wizard => Some(&self.wizard),
            MirClass::Taoist => Some(&self.taoist),
            _ => None,
        }
    }

    fn all(&self) -> impl Iterator<Item = &NewcomerRewardItem> {
        self.warrior
            .iter()
            .chain(self.wizard.iter())
            .chain(self.taoist.iter())
    }

    fn is_valid(&self) -> bool {
        !self.warrior.is_empty()
            && !self.wizard.is_empty()
            && !self.taoist.is_empty()
            && self.all().all(NewcomerRewardItem::is_valid)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewcomerQuestOverride {
    quest_id: i32,
    reward_experience: u32,
    #[serde(default)]
    kill_count_cap: Option<u32>,
    #[serde(default)]
    item_count_cap: Option<u16>,
    #[serde(default)]
    required_quest_id: Option<i32>,
    #[serde(default)]
    guaranteed_quest_drops: Vec<String>,
    #[serde(default)]
    task_description: Option<Vec<String>>,
}

#[derive(Debug)]
struct NewcomerJourneyConfig {
    overrides: BTreeMap<i32, NewcomerQuestOverride>,
    milestone_rewards: BTreeMap<u16, NewcomerClassRewards>,
}

fn journey_config() -> Option<&'static NewcomerJourneyConfig> {
    static CONFIG: OnceLock<Option<NewcomerJourneyConfig>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let file = serde_json::from_str::<NewcomerJourneyFile>(NEWCOMER_JOURNEY_JSON).ok()?;
            if file.schema != 1
                || !file.profile.eq_ignore_ascii_case("newcomer-v1")
                || file.max_level != 30
            {
                return None;
            }
            let mut overrides = BTreeMap::new();
            let mut seen_quest_ids = BTreeSet::new();
            for rule in file.quest_overrides {
                if rule.quest_id <= 0
                    || rule.kill_count_cap == Some(0)
                    || rule.item_count_cap == Some(0)
                    || !seen_quest_ids.insert(rule.quest_id)
                {
                    return None;
                }
                overrides.insert(rule.quest_id, rule);
            }
            let mut milestone_rewards = BTreeMap::new();
            for reward in file.milestone_rewards {
                if !REWARDED_MILESTONE_LEVELS.contains(&reward.level)
                    || !reward.class_rewards.is_valid()
                    || milestone_rewards
                        .insert(reward.level, reward.class_rewards)
                        .is_some()
                {
                    return None;
                }
            }
            if !REWARDED_MILESTONE_LEVELS
                .iter()
                .all(|level| milestone_rewards.contains_key(level))
            {
                return None;
            }
            Some(NewcomerJourneyConfig {
                overrides,
                milestone_rewards,
            })
        })
        .as_ref()
}

fn journey_override(world: &World, quest_id: i32) -> Option<&'static NewcomerQuestOverride> {
    enabled(world).then_some(())?;
    journey_config()?.overrides.get(&quest_id)
}

pub(super) fn is_journey_quest(quest_id: i32) -> bool {
    journey_config().is_some_and(|config| config.overrides.contains_key(&quest_id))
}

pub(super) fn apply_quest_info_override(world: &World, info: &mut ClientQuestInfo) {
    let Some(rule) = journey_override(world, info.index) else {
        return;
    };
    info.reward_exp = rule.reward_experience;
    if let Some(required_quest_id) = rule.required_quest_id {
        info.quest_needed = required_quest_id;
    }
    if let Some(task_description) = rule.task_description.as_ref() {
        info.task_description = task_description.clone();
    }
}

pub(super) fn apply_quest_template_override(
    world: &World,
    template: &mut CrystalQuestPacketTemplate,
) {
    let Some(rule) = journey_override(world, template.index) else {
        return;
    };
    if let Some(required_quest_id) = rule.required_quest_id {
        template.required_quest = required_quest_id;
    }
    if let Some(cap) = rule.kill_count_cap {
        for task in &mut template.kill_tasks {
            task.count = task.count.min(cap).max(1);
        }
    }
    if let Some(cap) = rule.item_count_cap {
        for task in &mut template.item_tasks {
            task.count = task.count.min(cap).max(1);
        }
    }
}

pub(super) fn overrides_required_quest(world: &World, quest_id: i32) -> bool {
    journey_override(world, quest_id).is_some_and(|rule| rule.required_quest_id.is_some())
}

pub(in crate::runtime) fn guaranteed_quest_drops(world: &World) -> Vec<(i32, String)> {
    if !enabled(world) {
        return Vec::new();
    }
    journey_config()
        .into_iter()
        .flat_map(|config| config.overrides.values())
        .flat_map(|rule| {
            rule.guaranteed_quest_drops
                .iter()
                .cloned()
                .map(move |item_name| (rule.quest_id, item_name))
        })
        .collect()
}

#[derive(Debug, Clone)]
struct NewcomerQuestDefinition {
    info: ClientQuestInfo,
    template: CrystalQuestPacketTemplate,
}

pub(in crate::runtime) fn enabled(world: &World) -> bool {
    world
        .get_resource::<QuestResource>()
        .is_some_and(|quests| quests.newcomer_v1_cadence)
}

/// Crystal's travelling merchant Damian is visible for only one server-hour,
/// but quest 124 is mandatory in the one-day newcomer route. Keep the original
/// schedule in the Crystal profile and relax only this NPC's time gate while
/// the explicit newcomer profile is active.
pub(in crate::runtime) fn keeps_npc_available_outside_time_window(
    object_id: Option<u32>,
) -> bool {
    object_id == Some(1_358)
}

pub(super) fn is_daily_quest(quest_id: i32) -> bool {
    DAILY_OPTION_IDS.contains(&quest_id) || quest_id == DAILY_BONUS_ID
}

pub(super) fn is_daily_option(quest_id: i32) -> bool {
    DAILY_OPTION_IDS.contains(&quest_id)
}

pub(super) fn is_newcomer_quest(quest_id: i32) -> bool {
    is_daily_quest(quest_id) || MILESTONE_IDS.contains(&quest_id)
}

pub(super) fn quest_info_by_id(world: &World, quest_id: i32) -> Option<ClientQuestInfo> {
    if !enabled(world) {
        return None;
    }
    let mut info = quest_info_by_id_unchecked(quest_id)?;
    apply_milestone_rewards(world, &mut info);
    Some(info)
}

pub(super) fn quest_info_by_id_unchecked(quest_id: i32) -> Option<ClientQuestInfo> {
    definitions()
        .iter()
        .find(|definition| definition.info.index == quest_id)
        .map(|definition| definition.info.clone())
}

pub(super) fn quest_template_by_id(quest_id: i32) -> Option<CrystalQuestPacketTemplate> {
    definitions()
        .iter()
        .find(|definition| definition.info.index == quest_id)
        .map(|definition| definition.template.clone())
}

pub(super) fn quest_infos(world: &World) -> Vec<ClientQuestInfo> {
    if !enabled(world) {
        return Vec::new();
    }
    definitions()
        .iter()
        .map(|definition| {
            let mut info = definition.info.clone();
            apply_milestone_rewards(world, &mut info);
            info
        })
        .collect()
}

fn apply_milestone_rewards(world: &World, info: &mut ClientQuestInfo) {
    let Ok(level) = u16::try_from(info.index - 2_100_000) else {
        return;
    };
    let Some(rewards) = journey_config().and_then(|config| config.milestone_rewards.get(&level))
    else {
        return;
    };
    let Some(character) = world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()
    else {
        return;
    };
    let Some(class_rewards) = rewards.for_class(character.class) else {
        return;
    };
    info.rewards_fixed_item
        .extend(class_rewards.iter().filter_map(|reward| {
            let item_name = reward.item_name_for_gender(character.gender)?;
            let template: CrystalItemTemplate = crystal_item_by_name(item_name)?;
            Some(QuestItemReward {
                item: item_info_from_crystal_template(template),
                count: reward.count,
            })
        }));
}

pub(super) fn quest_ids_for_npc(world: &World, npc_object_id: u32) -> Vec<i32> {
    if enabled(world) && npc_object_id == NEWCOMER_BOARD_OBJECT_ID {
        definitions()
            .iter()
            .map(|definition| definition.info.index)
            .collect()
    } else {
        Vec::new()
    }
}

pub(in crate::runtime) fn configured_quest_ids_for_npc(npc_object_id: u32) -> Vec<i32> {
    if super::quest_recurrence::server_newcomer_v1_enabled()
        && npc_object_id == NEWCOMER_BOARD_OBJECT_ID
    {
        definitions()
            .iter()
            .map(|definition| definition.info.index)
            .collect()
    } else {
        Vec::new()
    }
}

pub(super) fn can_accept(world: &World, quest_id: i32) -> bool {
    if !enabled(world) || !is_newcomer_quest(quest_id) {
        return false;
    }
    if is_daily_option(quest_id) {
        return selected_daily_count(&world.resource::<QuestResource>().quests) < 2;
    }
    true
}

pub(super) fn can_finish(world: &World, quest_id: i32) -> bool {
    if !is_newcomer_quest(quest_id) {
        return true;
    }
    if !enabled(world) {
        return false;
    }
    let quests = &world.resource::<QuestResource>().quests;
    if is_daily_option(quest_id) {
        return completed_daily_option_count(quests) < 2;
    }
    if quest_id == DAILY_BONUS_ID {
        return completed_daily_option_count(quests) >= 2;
    }
    let Some(info) = quest_info_by_id_unchecked(quest_id) else {
        return false;
    };
    let Some(character) = world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()
    else {
        return false;
    };
    let level = i32::from(character.level);
    level >= info.min_level_needed && (info.max_level_needed <= 0 || level <= info.max_level_needed)
}

pub(super) fn begin_stage(world: &World, quest_id: i32) -> Option<(QuestStage, u32)> {
    if quest_id != DAILY_BONUS_ID || !enabled(world) {
        return None;
    }
    let current = completed_daily_option_count(&world.resource::<QuestResource>().quests);
    Some((
        if current >= 2 {
            QuestStage::ReadyToTurnIn
        } else {
            QuestStage::InProgress
        },
        current,
    ))
}

pub(super) fn sync_daily_bonus(world: &mut World) -> bool {
    if !enabled(world) {
        return false;
    }
    sync_daily_bonus_states(&mut world.resource_mut::<QuestResource>().quests, true)
}

pub(super) fn sync_daily_bonus_states(quests: &mut [super::QuestState], newcomer_v1: bool) -> bool {
    if !newcomer_v1 {
        return false;
    }
    let current = completed_daily_option_count(quests);
    let Some(bonus) = quests
        .iter_mut()
        .find(|quest| quest.quest_id == DAILY_BONUS_ID)
    else {
        return false;
    };
    if !matches!(
        bonus.stage,
        QuestStage::InProgress | QuestStage::ReadyToTurnIn
    ) {
        return false;
    }
    let stage = if current >= 2 {
        QuestStage::ReadyToTurnIn
    } else {
        QuestStage::InProgress
    };
    let changed = bonus.current != current || bonus.stage != stage;
    bonus.current = current;
    bonus.stage = stage;
    changed
}

fn selected_daily_count(quests: &[super::QuestState]) -> u32 {
    DAILY_OPTION_IDS
        .iter()
        .filter(|quest_id| {
            quests.iter().any(|quest| {
                quest.quest_id == **quest_id
                    && matches!(
                        quest.stage,
                        QuestStage::InProgress | QuestStage::ReadyToTurnIn | QuestStage::Completed
                    )
            })
        })
        .count()
        .try_into()
        .unwrap_or(u32::MAX)
}

fn completed_daily_option_count(quests: &[super::QuestState]) -> u32 {
    let effective_period = DAILY_OPTION_IDS
        .iter()
        .filter_map(|quest_id| {
            quests
                .iter()
                .find(|quest| quest.quest_id == *quest_id)
                .and_then(|quest| quest.cadence_high_watermark_period)
        })
        .max();
    DAILY_OPTION_IDS
        .iter()
        .filter(|quest_id| {
            quests.iter().any(|quest| {
                quest.quest_id == **quest_id
                    && quest.stage == QuestStage::Completed
                    && quest.cadence_last_claimed_period == effective_period
            })
        })
        .count()
        .try_into()
        .unwrap_or(u32::MAX)
}

fn definitions() -> &'static [NewcomerQuestDefinition] {
    static DEFINITIONS: OnceLock<Vec<NewcomerQuestDefinition>> = OnceLock::new();
    DEFINITIONS.get_or_init(|| {
        let mut definitions = vec![
            daily_definition(
                DAILY_OPTION_IDS[0],
                "Daily: Oma Cave Patrol",
                "Defeat 10 OmaFighters. Choose any two of today's three commissions.",
                50,
                "OmaFighter",
                10,
            ),
            daily_definition(
                DAILY_OPTION_IDS[1],
                "Daily: Skeleton Cull",
                "Defeat 12 Skeletons. Choose any two of today's three commissions.",
                85,
                "Skeleton",
                12,
            ),
            daily_definition(
                DAILY_OPTION_IDS[2],
                "Daily: Forest Yeti Patrol",
                "Defeat 8 ForestYetis. Choose any two of today's three commissions.",
                55,
                "ForestYeti",
                8,
            ),
            immediate_definition(
                DAILY_BONUS_ID,
                "Daily: Two-of-Three Bonus",
                "Complete any two of today's three commissions, then return for the daily bonus.",
                "Daily",
                10,
                40,
                1,
                5_000,
                10_000,
            ),
        ];
        for (level, gold) in [
            (15, 5_000),
            (20, 10_000),
            (25, 15_000),
            (30, 25_000),
            (35, 40_000),
            (40, 60_000),
        ] {
            definitions.push(immediate_definition(
                2_100_000 + level,
                &format!("Level {level} Growth Reward"),
                &format!("A one-time newcomer reward for reaching level {level}."),
                "Milestone",
                level,
                i32::from(u16::MAX),
                0,
                gold,
                0,
            ));
        }
        definitions
    })
}

fn daily_definition(
    index: i32,
    name: &str,
    description: &str,
    monster_index: i32,
    monster_name: &str,
    count: u32,
) -> NewcomerQuestDefinition {
    let info = quest_info(index, name, description, "Daily", 10, 40, 1, 3_000, 5_000);
    let task = CrystalQuestKillTaskTemplate {
        monster_index,
        monster_name: monster_name.to_string(),
        count,
        message: format!("Defeat {monster_name}"),
    };
    NewcomerQuestDefinition {
        template: quest_template(&info, vec![task]),
        info,
    }
}

fn immediate_definition(
    index: i32,
    name: &str,
    description: &str,
    group: &str,
    min_level: i32,
    max_level: i32,
    quest_type: u8,
    reward_gold: u32,
    reward_exp: u32,
) -> NewcomerQuestDefinition {
    let info = quest_info(
        index,
        name,
        description,
        group,
        min_level,
        max_level,
        quest_type,
        reward_gold,
        reward_exp,
    );
    NewcomerQuestDefinition {
        template: quest_template(&info, Vec::new()),
        info,
    }
}

fn quest_info(
    index: i32,
    name: &str,
    description: &str,
    group: &str,
    min_level: i32,
    max_level: i32,
    quest_type: u8,
    reward_gold: u32,
    reward_exp: u32,
) -> ClientQuestInfo {
    ClientQuestInfo {
        index,
        npc_index: NEWCOMER_BOARD_OBJECT_ID,
        name: name.to_string(),
        group: group.to_string(),
        description: vec![description.to_string()],
        task_description: vec![description.to_string()],
        return_description: vec!["Return to the Bichon Wall Board.".to_string()],
        completion_description: vec!["Commission complete.".to_string()],
        min_level_needed: min_level,
        max_level_needed: max_level,
        quest_needed: 0,
        class_needed: 0,
        quest_type,
        time_limit_in_seconds: 0,
        reward_gold,
        reward_exp,
        reward_credit: 0,
        rewards_fixed_item: Vec::new(),
        rewards_select_item: Vec::new(),
        finish_npc_index: NEWCOMER_BOARD_OBJECT_ID,
    }
}

fn quest_template(
    info: &ClientQuestInfo,
    kill_tasks: Vec<CrystalQuestKillTaskTemplate>,
) -> CrystalQuestPacketTemplate {
    CrystalQuestPacketTemplate {
        index: info.index,
        name: info.name.clone(),
        group: info.group.clone(),
        file_name: "NewcomerV1/BichonWall/Board".to_string(),
        required_min_level: info.min_level_needed,
        required_max_level: info.max_level_needed,
        required_quest: 0,
        required_class: 0,
        quest_type: info.quest_type,
        goto_message: "Return to the Bichon Wall Board.".to_string(),
        kill_message: String::new(),
        item_message: String::new(),
        flag_message: String::new(),
        time_limit_in_seconds: 0,
        npc_index: NEWCOMER_BOARD_OBJECT_ID,
        finish_npc_index: NEWCOMER_BOARD_OBJECT_ID,
        carry_items: Vec::new(),
        kill_tasks,
        item_tasks: Vec::new(),
        flag_tasks: Vec::new(),
        payload_len: 0,
        payload_hex: String::new(),
    }
}
