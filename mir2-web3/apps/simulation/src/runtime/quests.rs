use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::config::{ItemContainer, QuestObjectiveSnapshot, QuestSnapshot, QuestStage};
use bevy_ecs::prelude::World;
use mir2_game_data::{
    crystal_item_by_index, crystal_item_by_name, crystal_npc_info_manifest,
    crystal_quest_packet_manifest, localized_text_or_fallback, starter_server_data,
    CrystalQuestItemTaskTemplate, CrystalQuestPacketTemplate, LanguageCode, QuestStageCopy,
    QuestTemplate,
};
use mir2_protocol::{
    decode_server_packet, encode_frame, ChatType, ClientQuestInfo, ItemInfo, MirClass,
    QuestItemReward, ServerPacket, ServerPacketId,
};

use super::crystal_compat::GUIDE_QUEST_ID;
use super::equipment::{equipment_template_to_state, replace_equipment};
use super::inventory::{
    add_or_increment_item, additional_slots_needed_for_item_quantity, can_gain_item_quantity,
    free_bag_slots,
};
use super::items::{
    crystal_item_key_for_template, item_info_from_crystal_template, normalize_crystal_item_key,
};
use super::npc::{localized_npc_dialog_base_key, npc_script_for_object_id, NpcDialogLinkState};
use super::npc_script::{NpcInteractionContext, NpcQuestDialog};
use super::resources::{
    InventoryResource, PlayerRuntimeResource, QuestResource, RuntimeConfigResource, SessionResource,
};

const CRYSTAL_NORMAL_QUEST_MIN_LEVEL: i32 = 1;
const CRYSTAL_NORMAL_QUEST_MAX_LEVEL: i32 = 45;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct QuestState {
    pub(super) quest_id: i32,
    pub(super) title: String,
    pub(super) summary: String,
    pub(super) reward_preview: String,
    pub(super) required: u32,
    pub(super) current: u32,
    pub(super) stage: QuestStage,
    #[serde(default)]
    pub(super) task_progress: BTreeMap<String, u32>,
}

impl QuestState {
    pub(super) fn guide_training() -> Self {
        let template = guide_quest_template();
        Self {
            quest_id: template.quest_id,
            title: template.title.clone(),
            summary: template.summary.clone(),
            reward_preview: template.reward_preview.clone(),
            required: template.required,
            current: 0,
            stage: QuestStage::Available,
            task_progress: BTreeMap::new(),
        }
    }

    pub(super) fn from_crystal_info(info: &ClientQuestInfo, stage: QuestStage) -> Self {
        let required = crystal_quest_required_count(info);
        Self {
            quest_id: info.index,
            title: info.name.clone(),
            summary: crystal_quest_summary(info),
            reward_preview: crystal_quest_reward_preview(info),
            required,
            current: 0,
            stage,
            task_progress: BTreeMap::new(),
        }
    }

    pub(super) fn objective(&self, language: LanguageCode) -> String {
        stage_copy(self, language).objective
    }

    pub(super) fn progress_label(&self, language: LanguageCode) -> String {
        stage_copy(self, language).progress_label
    }

    pub(super) fn tracker(&self, language: LanguageCode) -> String {
        stage_copy(self, language).tracker
    }

    pub(super) fn snapshot(&self, language: LanguageCode) -> QuestSnapshot {
        QuestSnapshot {
            quest_id: self.quest_id,
            title: localized_quest_title(language, self.quest_id, &self.title),
            summary: localized_quest_summary(language, self.quest_id, &self.summary),
            objective: self.objective(language),
            progress_label: self.progress_label(language),
            tracker: self.tracker(language),
            stage: self.stage,
            current: self.current,
            required: self.required,
            reward_preview: localized_quest_reward_preview(
                language,
                self.quest_id,
                &self.reward_preview,
            ),
            objectives: self.objective_snapshots(),
        }
    }

    fn objective_snapshots(&self) -> Vec<QuestObjectiveSnapshot> {
        let Some(template) = crystal_quest_template_by_id(self.quest_id) else {
            return Vec::new();
        };
        let mut objectives = Vec::with_capacity(
            template.kill_tasks.len() + template.item_tasks.len() + template.flag_tasks.len(),
        );
        for task in &template.kill_tasks {
            let required = task.count.max(1);
            let current =
                crystal_quest_task_progress(self, &crystal_kill_task_key(task.monster_index))
                    .min(required);
            objectives.push(QuestObjectiveSnapshot {
                label: objective_snapshot_label(
                    &task.message,
                    &format!("Kill {}", task.monster_name),
                ),
                current,
                required,
                done: current >= required,
            });
        }
        for task in &template.item_tasks {
            let required = u32::from(task.count.max(1));
            let current =
                crystal_quest_task_progress(self, &crystal_item_task_key(task.item_index))
                    .min(required);
            objectives.push(QuestObjectiveSnapshot {
                label: objective_snapshot_label(
                    &task.message,
                    &format!("Collect {}", task.item_name),
                ),
                current,
                required,
                done: current >= required,
            });
        }
        for task in &template.flag_tasks {
            let required = 1;
            let current = crystal_quest_task_progress(self, &crystal_flag_task_key(task.number))
                .min(required);
            objectives.push(QuestObjectiveSnapshot {
                label: objective_snapshot_label(
                    &task.message,
                    &format!("Activate flag {}", task.number),
                ),
                current,
                required,
                done: current >= required,
            });
        }
        objectives
    }
}

fn objective_snapshot_label(message: &str, fallback: &str) -> String {
    let label = if message.trim().is_empty() {
        fallback
    } else {
        message.trim()
    };
    strip_crystal_inline_colour(label).trim().to_string()
}

pub(super) fn guide_quest_template() -> QuestTemplate {
    quest_template_by_id(GUIDE_QUEST_ID).expect("guide quest template should exist")
}

pub(super) fn quest_template_by_id(quest_id: i32) -> Option<QuestTemplate> {
    starter_server_data()
        .quests
        .into_iter()
        .find(|quest| quest.quest_id == quest_id)
}

pub(super) fn crystal_client_quest_infos() -> Vec<ClientQuestInfo> {
    static QUESTS: OnceLock<Vec<ClientQuestInfo>> = OnceLock::new();
    QUESTS
        .get_or_init(|| {
            crystal_quest_packet_manifest()
                .quests
                .into_iter()
                .filter_map(|quest| decode_crystal_quest_packet_payload(&quest.payload_hex))
                .collect()
        })
        .clone()
}

pub(super) fn crystal_normal_quest_infos_1_to_45() -> Vec<ClientQuestInfo> {
    crystal_client_quest_infos()
        .into_iter()
        .filter(|info| {
            (CRYSTAL_NORMAL_QUEST_MIN_LEVEL..=CRYSTAL_NORMAL_QUEST_MAX_LEVEL)
                .contains(&info.min_level_needed)
        })
        .collect()
}

pub(super) fn crystal_quest_info_by_id(quest_id: i32) -> Option<ClientQuestInfo> {
    crystal_client_quest_infos()
        .into_iter()
        .find(|info| info.index == quest_id)
}

fn effective_crystal_quest_info_by_id(world: &World, quest_id: i32) -> Option<ClientQuestInfo> {
    let mut info = crystal_quest_info_by_id(quest_id)?;
    apply_content_profile_quest_reward_overrides(world, &mut info);
    Some(info)
}

fn apply_content_profile_quest_reward_overrides(world: &World, info: &mut ClientQuestInfo) {
    let Some(profile) = world
        .resource::<RuntimeConfigResource>()
        .config
        .content_profile
        .as_ref()
    else {
        return;
    };
    for rule in profile
        .profile
        .quest_reward_overrides
        .iter()
        .filter(|rule| rule.quest_id == info.index)
    {
        if info
            .rewards_fixed_item
            .iter()
            .any(|reward| reward.item.name.eq_ignore_ascii_case(&rule.item))
        {
            continue;
        }
        let Some(template) = crystal_item_by_name(&rule.item) else {
            continue;
        };
        info.rewards_fixed_item.push(QuestItemReward {
            item: item_info_from_crystal_template(template),
            count: rule.count,
        });
    }
}

pub(super) fn effective_crystal_quest_info_packets(world: &World) -> Vec<ServerPacket> {
    crystal_client_quest_infos()
        .into_iter()
        .map(|mut info| {
            apply_content_profile_quest_reward_overrides(world, &mut info);
            ServerPacket::NewQuestInfo { info }
        })
        .collect()
}

pub(super) fn crystal_quest_template_by_id(quest_id: i32) -> Option<CrystalQuestPacketTemplate> {
    crystal_quest_packet_manifest()
        .quests
        .into_iter()
        .find(|quest| quest.index == quest_id)
}

pub(super) fn quest_definition_exists(quest_id: i32) -> bool {
    quest_template_by_id(quest_id).is_some() || crystal_quest_info_by_id(quest_id).is_some()
}

fn decode_crystal_quest_packet_payload(payload_hex: &str) -> Option<ClientQuestInfo> {
    let payload = decode_hex(payload_hex)?;
    let frame = encode_frame(ServerPacketId::NewQuestInfo as i16, &payload).ok()?;
    match decode_server_packet(&frame).ok()? {
        ServerPacket::NewQuestInfo { info } => Some(info),
        _ => None,
    }
}

fn decode_hex(input: &str) -> Option<Vec<u8>> {
    if input.len() % 2 != 0 {
        return None;
    }
    (0..input.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&input[index..index + 2], 16).ok())
        .collect()
}

fn crystal_quest_summary(info: &ClientQuestInfo) -> String {
    first_non_empty_line(&info.description)
        .or_else(|| first_non_empty_line(&info.task_description))
        .unwrap_or_else(|| info.name.clone())
}

fn first_non_empty_line(lines: &[String]) -> Option<String> {
    lines
        .iter()
        .map(|line| strip_crystal_inline_colour(line).trim().to_string())
        .find(|line| !line.is_empty())
}

fn strip_crystal_inline_colour(line: &str) -> String {
    let mut output = String::with_capacity(line.len());
    let mut skip = false;
    for ch in line.chars() {
        match ch {
            '{' => skip = true,
            '}' => skip = false,
            _ if !skip => output.push(ch),
            _ => {}
        }
    }
    output
}

fn crystal_quest_reward_preview(info: &ClientQuestInfo) -> String {
    let mut rewards = Vec::new();
    if info.reward_exp > 0 {
        rewards.push(format!("EXP {}", info.reward_exp));
    }
    if info.reward_gold > 0 {
        rewards.push(format!("Gold {}", info.reward_gold));
    }
    if info.reward_credit > 0 {
        rewards.push(format!("Credit {}", info.reward_credit));
    }
    for reward in &info.rewards_fixed_item {
        rewards.push(format!("{} x{}", reward.item.name, reward.count.max(1)));
    }
    if !info.rewards_select_item.is_empty() {
        let names = info
            .rewards_select_item
            .iter()
            .map(|reward| reward.item.name.as_str())
            .collect::<Vec<_>>()
            .join(" / ");
        rewards.push(format!("Choose: {names}"));
    }
    if rewards.is_empty() {
        "Original Crystal quest reward.".to_string()
    } else {
        rewards.join(", ")
    }
}

fn crystal_quest_required_count(info: &ClientQuestInfo) -> u32 {
    let Some(template) = crystal_quest_template_by_id(info.index) else {
        return 1;
    };
    let kill_total = template
        .kill_tasks
        .iter()
        .map(|task| task.count.max(1))
        .sum::<u32>();
    let item_total = template
        .item_tasks
        .iter()
        .map(|task| u32::from(task.count.max(1)))
        .sum::<u32>();
    let flag_total = u32::try_from(template.flag_tasks.len()).unwrap_or(0);
    let total = kill_total
        .saturating_add(item_total)
        .saturating_add(flag_total);
    total.max(1)
}

pub(super) fn stage_copy(quest: &QuestState, language: LanguageCode) -> QuestStageCopy {
    if let Some(info) = crystal_quest_info_by_id(quest.quest_id) {
        return crystal_stage_copy(quest, &info);
    }
    let fallback_template =
        quest_template_by_id(quest.quest_id).unwrap_or_else(guide_quest_template);
    let copy =
        localized_quest_stage_copy(language, quest.quest_id, quest.stage, &fallback_template);
    QuestStageCopy {
        objective: render_template(copy.objective, quest.current, quest.required),
        progress_label: render_template(copy.progress_label, quest.current, quest.required),
        tracker: render_template(copy.tracker, quest.current, quest.required),
    }
}

fn crystal_stage_copy(quest: &QuestState, info: &ClientQuestInfo) -> QuestStageCopy {
    let objective = match quest.stage {
        QuestStage::Available => join_quest_lines(&info.description, &quest.summary),
        QuestStage::InProgress => join_quest_lines(&info.task_description, &quest.summary),
        QuestStage::ReadyToTurnIn => join_quest_lines(
            &info.return_description,
            "Return to the quest NPC and hand in the task.",
        ),
        QuestStage::Completed => join_quest_lines(&info.completion_description, "Quest completed."),
    };
    let progress_label = match quest.stage {
        QuestStage::Available => level_and_class_requirement_label(info),
        QuestStage::InProgress => {
            format!(
                "Progress {}/{}",
                quest.current.min(quest.required),
                quest.required
            )
        }
        QuestStage::ReadyToTurnIn => "Ready to turn in.".to_string(),
        QuestStage::Completed => "Completed.".to_string(),
    };
    let tracker = match quest.stage {
        QuestStage::Available => format!("{} is available.", info.name),
        QuestStage::InProgress => {
            first_non_empty_line(&info.task_description).unwrap_or_else(|| info.name.clone())
        }
        QuestStage::ReadyToTurnIn => first_non_empty_line(&info.return_description)
            .unwrap_or_else(|| format!("Return to {}.", crystal_quest_finish_npc_label(info))),
        QuestStage::Completed => format!("{} completed.", info.name),
    };
    QuestStageCopy {
        objective,
        progress_label,
        tracker,
    }
}

pub(super) fn crystal_quest_task_list(world: &World, quest_id: i32) -> Option<Vec<String>> {
    let info = crystal_quest_info_by_id(quest_id)?;
    let quest = world
        .resource::<QuestResource>()
        .quests
        .iter()
        .find(|quest| quest.quest_id == quest_id)?;
    let Some(template) = crystal_quest_template_by_id(quest_id) else {
        return Some(info.task_description.clone());
    };
    let mut tasks = Vec::new();
    for task in &template.kill_tasks {
        push_task_line(
            &mut tasks,
            &task.message,
            &format!(
                "Kill {} {}/{}",
                task.monster_name,
                crystal_quest_task_progress(quest, &crystal_kill_task_key(task.monster_index)),
                task.count.max(1)
            ),
        );
    }
    for task in &template.item_tasks {
        push_task_line(
            &mut tasks,
            &task.message,
            &format!(
                "Collect {} {}/{}",
                task.item_name,
                crystal_quest_task_progress(quest, &crystal_item_task_key(task.item_index)),
                task.count.max(1)
            ),
        );
    }
    for task in &template.flag_tasks {
        push_task_line(
            &mut tasks,
            &task.message,
            &format!(
                "Activate flag {} {}",
                task.number,
                if crystal_quest_task_progress(quest, &crystal_flag_task_key(task.number)) > 0 {
                    "Complete"
                } else {
                    ""
                }
            ),
        );
    }
    if tasks.is_empty() {
        tasks.extend(info.task_description.clone());
    }
    if quest.stage == QuestStage::ReadyToTurnIn && !template.goto_message.is_empty() {
        tasks.push(template.goto_message.clone());
    }
    Some(tasks)
}

fn push_task_line(tasks: &mut Vec<String>, message: &str, fallback: &str) {
    let text = if message.trim().is_empty() {
        fallback
    } else {
        message.trim()
    };
    if !text.is_empty() && !tasks.iter().any(|task| task == text) {
        tasks.push(text.to_string());
    }
}

fn crystal_quest_task_progress(quest: &QuestState, key: &str) -> u32 {
    quest.task_progress.get(key).copied().unwrap_or(0)
}

fn crystal_kill_task_key(monster_index: i32) -> String {
    format!("kill:{monster_index}")
}

fn crystal_item_task_key(item_index: i32) -> String {
    format!("item:{item_index}")
}

fn crystal_flag_task_key(number: i32) -> String {
    format!("flag:{number}")
}

fn recompute_crystal_quest_current(quest: &mut QuestState, template: &CrystalQuestPacketTemplate) {
    let mut total = 0_u32;
    for task in &template.kill_tasks {
        total = total.saturating_add(
            crystal_quest_task_progress(quest, &crystal_kill_task_key(task.monster_index))
                .min(task.count.max(1)),
        );
    }
    for task in &template.item_tasks {
        total = total.saturating_add(
            crystal_quest_task_progress(quest, &crystal_item_task_key(task.item_index))
                .min(u32::from(task.count.max(1))),
        );
    }
    for task in &template.flag_tasks {
        total = total.saturating_add(
            crystal_quest_task_progress(quest, &crystal_flag_task_key(task.number)).min(1),
        );
    }
    quest.current = total.min(quest.required);
    if quest.current >= quest.required {
        quest.stage = QuestStage::ReadyToTurnIn;
    }
}

fn increment_crystal_quest_task(
    quest: &mut QuestState,
    task_key: String,
    amount: u32,
    task_required: u32,
) -> bool {
    let current = quest.task_progress.get(&task_key).copied().unwrap_or(0);
    if current >= task_required {
        return false;
    }
    quest
        .task_progress
        .insert(task_key, current.saturating_add(amount).min(task_required));
    true
}

fn join_quest_lines(lines: &[String], fallback: &str) -> String {
    let joined = lines
        .iter()
        .map(|line| strip_crystal_inline_colour(line))
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if joined.is_empty() {
        fallback.to_string()
    } else {
        joined
    }
}

fn level_and_class_requirement_label(info: &ClientQuestInfo) -> String {
    let mut parts = Vec::new();
    if info.min_level_needed > 0 {
        parts.push(format!("Level {}+", info.min_level_needed));
    }
    if info.max_level_needed > 0 && info.max_level_needed < i32::from(u16::MAX) {
        parts.push(format!("up to {}", info.max_level_needed));
    }
    if info.class_needed != 0 {
        parts.push("class restricted".to_string());
    }
    if parts.is_empty() {
        "Available.".to_string()
    } else {
        parts.join(", ")
    }
}

pub(super) fn render_template(template: String, current: u32, required: u32) -> String {
    template
        .replace("{current}", &current.to_string())
        .replace("{required}", &required.to_string())
        .replace("{0}", &current.to_string())
        .replace("{1}", &required.to_string())
}

pub(super) fn localized_quest_base_key(quest_id: i32) -> Option<&'static str> {
    match quest_id {
        GUIDE_QUEST_ID => Some("content.quest.fieldWaspTrial"),
        _ => None,
    }
}

pub(super) fn quest_stage_key(stage: QuestStage) -> &'static str {
    match stage {
        QuestStage::Available => "available",
        QuestStage::InProgress => "inProgress",
        QuestStage::ReadyToTurnIn => "readyToTurnIn",
        QuestStage::Completed => "completed",
    }
}

pub(super) fn localized_quest_title(
    language: LanguageCode,
    quest_id: i32,
    fallback: &str,
) -> String {
    localized_quest_base_key(quest_id)
        .map(|base| localized_text_or_fallback(language, &format!("{base}.title"), fallback))
        .unwrap_or_else(|| fallback.to_string())
}

pub(super) fn localized_quest_summary(
    language: LanguageCode,
    quest_id: i32,
    fallback: &str,
) -> String {
    localized_quest_base_key(quest_id)
        .map(|base| localized_text_or_fallback(language, &format!("{base}.summary"), fallback))
        .unwrap_or_else(|| fallback.to_string())
}

pub(super) fn localized_quest_reward_preview(
    language: LanguageCode,
    quest_id: i32,
    fallback: &str,
) -> String {
    localized_quest_base_key(quest_id)
        .map(|base| {
            localized_text_or_fallback(language, &format!("{base}.rewardPreview"), fallback)
        })
        .unwrap_or_else(|| fallback.to_string())
}

pub(super) fn resolve_npc_quest_dialog(
    world: &mut World,
    context: &NpcInteractionContext,
) -> Option<NpcQuestDialog> {
    let quest_id = context
        .quest_ids
        .iter()
        .copied()
        .find(|quest_id| quest_template_by_id(*quest_id).is_some())?;
    let stage_before_action = ensure_runtime_quest(world, quest_id);
    let language = world.resource::<SessionResource>().language;
    let (current, required) = quest_progress(world, quest_id).unwrap_or((0, 1));

    let (dialog_current, dialog_required) = match stage_before_action {
        QuestStage::Available => (0, required),
        QuestStage::ReadyToTurnIn => (required, required),
        QuestStage::Completed => (current.max(required), required),
        QuestStage::InProgress => (current, required),
    };

    let (title, body, footer, object_chat) = npc_stage_dialog_for_object(
        language,
        context.object_id,
        stage_before_action,
        dialog_current,
        dialog_required,
    )?;

    match stage_before_action {
        QuestStage::Available => set_quest_stage(world, quest_id, QuestStage::InProgress),
        QuestStage::ReadyToTurnIn => complete_quest(world, quest_id),
        QuestStage::InProgress | QuestStage::Completed => {}
    }

    Some(NpcQuestDialog {
        stage_before_action,
        current: dialog_current,
        required: dialog_required,
        title,
        body,
        footer,
        object_chat,
    })
}

pub(super) fn ensure_runtime_quest(world: &mut World, quest_id: i32) -> QuestStage {
    if let Some(stage) = world
        .resource::<QuestResource>()
        .quests
        .iter()
        .find(|quest| quest.quest_id == quest_id)
        .map(|quest| quest.stage)
    {
        return stage;
    }

    let quest = if let Some(template) = quest_template_by_id(quest_id) {
        QuestState {
            quest_id: template.quest_id,
            title: template.title,
            summary: template.summary,
            reward_preview: template.reward_preview,
            required: template.required,
            current: 0,
            stage: QuestStage::Available,
            task_progress: BTreeMap::new(),
        }
    } else if let Some(info) = effective_crystal_quest_info_by_id(world, quest_id) {
        QuestState::from_crystal_info(&info, QuestStage::Available)
    } else {
        return QuestStage::Available;
    };
    world.resource_mut::<QuestResource>().quests.push(quest);
    QuestStage::Available
}

pub(super) fn quest_stage(world: &World, quest_id: i32) -> Option<QuestStage> {
    world
        .resource::<QuestResource>()
        .quests
        .iter()
        .find(|quest| quest.quest_id == quest_id)
        .map(|quest| quest.stage)
}

pub(super) fn quest_progress(world: &World, quest_id: i32) -> Option<(u32, u32)> {
    world
        .resource::<QuestResource>()
        .quests
        .iter()
        .find(|quest| quest.quest_id == quest_id)
        .map(|quest| (quest.current, quest.required))
}

pub(super) fn set_quest_stage(world: &mut World, quest_id: i32, stage: QuestStage) {
    if let Some(quest) = world
        .resource_mut::<QuestResource>()
        .quests
        .iter_mut()
        .find(|quest| quest.quest_id == quest_id)
    {
        quest.stage = stage;
    }
}

pub(super) fn begin_quest(world: &mut World, quest_id: i32) -> QuestStage {
    let stage = ensure_runtime_quest(world, quest_id);
    if stage != QuestStage::Available {
        return stage;
    }
    if let Some(template) = quest_template_by_id(quest_id) {
        set_quest_stage(world, template.quest_id, QuestStage::InProgress);
        return QuestStage::InProgress;
    }
    let Some(info) = effective_crystal_quest_info_by_id(world, quest_id) else {
        return QuestStage::Available;
    };
    let Some(template) = crystal_quest_template_by_id(quest_id) else {
        set_quest_stage(world, quest_id, QuestStage::InProgress);
        return QuestStage::InProgress;
    };

    grant_crystal_carry_items(world, &template);
    let next_stage = if crystal_quest_has_progress_tasks(&template) {
        QuestStage::InProgress
    } else {
        QuestStage::ReadyToTurnIn
    };
    if let Some(quest) = world
        .resource_mut::<QuestResource>()
        .quests
        .iter_mut()
        .find(|quest| quest.quest_id == info.index)
    {
        quest.stage = next_stage;
        if next_stage == QuestStage::ReadyToTurnIn {
            quest.current = quest.required;
        }
    }
    next_stage
}

fn crystal_quest_has_progress_tasks(template: &CrystalQuestPacketTemplate) -> bool {
    !template.kill_tasks.is_empty()
        || !template.item_tasks.is_empty()
        || !template.flag_tasks.is_empty()
}

fn grant_crystal_carry_items(world: &mut World, template: &CrystalQuestPacketTemplate) {
    for task in &template.carry_items {
        grant_crystal_quest_task_item(world, task, u32::from(task.count.max(1)));
    }
}

pub(super) fn complete_quest(world: &mut World, quest_id: i32) {
    let _ = complete_quest_with_selection(world, quest_id, None);
}

pub(super) fn complete_quest_with_selection(
    world: &mut World,
    quest_id: i32,
    selected_item_index: Option<i32>,
) -> bool {
    if let Some(info) = effective_crystal_quest_info_by_id(world, quest_id) {
        return complete_crystal_quest(world, &info, selected_item_index);
    }
    let Some(quest) = quest_template_by_id(quest_id) else {
        return false;
    };
    {
        let resources = world.resource::<InventoryResource>();
        let needed_slots = quest
            .completion_rewards
            .items
            .iter()
            .map(|item| {
                additional_slots_needed_for_item_quantity(
                    resources,
                    ItemContainer::Bag1,
                    &item.key,
                    item.quantity,
                )
            })
            .sum::<u32>();
        if u32::from(free_bag_slots(resources)) < needed_slots {
            return false;
        }
    }
    {
        let mut resources = world.resource_mut::<InventoryResource>();
        resources
            .inventory_items
            .retain(|item| item.key != quest.quest_item.key);
    }
    world.resource_mut::<PlayerRuntimeResource>().gold += quest.completion_rewards.gold;
    if let Some(quest) = world
        .resource_mut::<QuestResource>()
        .quests
        .iter_mut()
        .find(|quest| quest.quest_id == quest_id)
    {
        quest.stage = QuestStage::Completed;
        quest.current = quest.required;
    }

    for item in quest.completion_rewards.items {
        add_or_increment_item(
            world,
            ItemContainer::Bag1,
            &item.key,
            &item.name,
            &item.description,
            item.preferred_slot,
            item.quantity,
            item.weight,
        );
    }
    for equipment in quest.completion_rewards.equipment {
        replace_equipment(world, equipment_template_to_state(&equipment));
    }
    true
}

fn complete_crystal_quest(
    world: &mut World,
    info: &ClientQuestInfo,
    selected_item_index: Option<i32>,
) -> bool {
    let fixed_rewards = info.rewards_fixed_item.clone();
    let selected_reward = selected_crystal_quest_reward(info, selected_item_index);
    if !info.rewards_select_item.is_empty() && selected_reward.is_none() {
        return false;
    }
    let needed_slots = crystal_quest_reward_slots_needed(
        world,
        fixed_rewards.iter().chain(selected_reward.iter()),
    );
    if u32::from(free_bag_slots(world.resource::<InventoryResource>())) < needed_slots {
        return false;
    }

    {
        let mut player = world.resource_mut::<PlayerRuntimeResource>();
        player.gold = player.gold.saturating_add(info.reward_gold);
        player.credit = player.credit.saturating_add(info.reward_credit);
    }
    // Crystal `GainExp`: quest experience rolls into levels via the shared curve;
    // the new level/exp/HP reach the client through the post-completion snapshot.
    let _ = super::leveling::apply_experience_gain(world, i64::from(info.reward_exp));

    for reward in fixed_rewards.iter().chain(selected_reward.iter()) {
        grant_crystal_quest_reward_item(world, reward);
    }
    if let Some(template) = crystal_quest_template_by_id(info.index) {
        take_crystal_quest_task_items(world, &template);
    }

    if let Some(quest) = world
        .resource_mut::<QuestResource>()
        .quests
        .iter_mut()
        .find(|quest| quest.quest_id == info.index)
    {
        quest.stage = QuestStage::Completed;
        quest.current = quest.required;
    }
    true
}

fn selected_crystal_quest_reward(
    info: &ClientQuestInfo,
    selected_item_index: Option<i32>,
) -> Option<QuestItemReward> {
    if info.rewards_select_item.is_empty() {
        return None;
    }
    let selected = selected_item_index?;
    usize::try_from(selected)
        .ok()
        .and_then(|index| info.rewards_select_item.get(index).cloned())
        .or_else(|| {
            info.rewards_select_item
                .iter()
                .find(|reward| reward.item.index == selected)
                .cloned()
        })
}

pub(super) fn crystal_quest_reward_selection_missing(
    quest_id: i32,
    selected_item_index: Option<i32>,
) -> bool {
    crystal_quest_info_by_id(quest_id).is_some_and(|info| {
        !info.rewards_select_item.is_empty()
            && selected_crystal_quest_reward(&info, selected_item_index).is_none()
    })
}

fn crystal_quest_reward_slots_needed<'a>(
    world: &World,
    rewards: impl Iterator<Item = &'a QuestItemReward>,
) -> u32 {
    let resources = world.resource::<InventoryResource>();
    rewards
        .map(|reward| {
            let key = crystal_quest_reward_item_key(&reward.item);
            additional_slots_needed_for_item_quantity(
                resources,
                ItemContainer::Bag1,
                &key,
                u32::from(reward.count.max(1)),
            )
        })
        .sum()
}

fn grant_crystal_quest_reward_item(world: &mut World, reward: &QuestItemReward) {
    let quantity = u32::from(reward.count.max(1));
    let key = crystal_quest_reward_item_key(&reward.item);
    add_or_increment_item(
        world,
        ItemContainer::Bag1,
        &key,
        &reward.item.name,
        reward
            .item
            .tooltip
            .as_deref()
            .unwrap_or("Original Crystal quest reward."),
        0,
        quantity,
        u16::from(reward.item.weight),
    );
}

fn crystal_quest_reward_item_key(item: &ItemInfo) -> String {
    crystal_item_by_index(item.index)
        .map(|template| crystal_item_key_for_template(&template))
        .unwrap_or_else(|| {
            if item.index > 0 {
                format!("crystal-item-{}", item.index)
            } else {
                normalize_crystal_item_key(&item.name)
            }
        })
}

fn crystal_quest_task_item_key(task: &CrystalQuestItemTaskTemplate) -> String {
    crystal_item_by_index(task.item_index)
        .map(|template| crystal_item_key_for_template(&template))
        .unwrap_or_else(|| normalize_crystal_item_key(&task.item_name))
}

fn grant_crystal_quest_task_item(
    world: &mut World,
    task: &CrystalQuestItemTaskTemplate,
    quantity: u32,
) {
    let quantity = quantity.max(1);
    let key = crystal_quest_task_item_key(task);
    let (name, description, weight) = crystal_item_by_index(task.item_index)
        .map(|template| {
            (
                template.name,
                template
                    .tooltip
                    .unwrap_or_else(|| "Original Crystal quest item.".to_string()),
                u16::from(template.weight.max(1)),
            )
        })
        .unwrap_or_else(|| {
            (
                task.item_name.clone(),
                "Original Crystal quest item.".to_string(),
                1,
            )
        });
    add_or_increment_item(
        world,
        ItemContainer::Quest,
        &key,
        &name,
        &description,
        0,
        quantity,
        weight,
    );
}

fn take_crystal_quest_task_items(world: &mut World, template: &CrystalQuestPacketTemplate) {
    let keys = template
        .carry_items
        .iter()
        .chain(template.item_tasks.iter())
        .map(crystal_quest_task_item_key)
        .collect::<BTreeSet<_>>();
    if keys.is_empty() {
        return;
    }
    world
        .resource_mut::<InventoryResource>()
        .inventory_items
        .retain(|item| item.container != ItemContainer::Quest || !keys.contains(&item.key));
}

pub(super) fn can_accept_quest(world: &World, quest_id: i32) -> bool {
    if quest_template_by_id(quest_id).is_some() {
        return !matches!(
            quest_stage(world, quest_id),
            Some(QuestStage::InProgress | QuestStage::ReadyToTurnIn | QuestStage::Completed)
        );
    }
    crystal_quest_info_by_id(quest_id)
        .as_ref()
        .is_some_and(|info| can_accept_crystal_quest(world, info))
}

pub(super) fn can_accept_crystal_quest(world: &World, info: &ClientQuestInfo) -> bool {
    if matches!(
        quest_stage(world, info.index),
        Some(QuestStage::InProgress | QuestStage::ReadyToTurnIn | QuestStage::Completed)
    ) {
        return false;
    }
    let Some(character) = world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()
    else {
        return false;
    };
    let level = i32::from(character.level);
    if level < info.min_level_needed {
        return false;
    }
    if info.max_level_needed > 0 && info.max_level_needed < i32::from(u16::MAX) {
        if level > info.max_level_needed {
            return false;
        }
    }
    if !crystal_quest_class_matches(character.class, info.class_needed) {
        return false;
    }
    crystal_required_quest_chain_completed(
        world,
        effective_crystal_required_quest_id(world, info.index, info.quest_needed),
    )
}

fn effective_crystal_required_quest_id(
    world: &World,
    quest_id: i32,
    imported_required_quest_id: i32,
) -> i32 {
    world
        .resource::<RuntimeConfigResource>()
        .config
        .content_profile
        .as_ref()
        .and_then(|profile| {
            profile
                .profile
                .quest_prerequisite_overrides
                .iter()
                .find(|rule| rule.quest_id == quest_id)
        })
        .map(|rule| rule.required_quest_id)
        .unwrap_or(imported_required_quest_id)
}

fn crystal_required_quest_chain_completed(world: &World, required_quest_id: i32) -> bool {
    let mut current = required_quest_id;
    let mut seen = BTreeSet::new();
    while current > 0 {
        if !seen.insert(current) {
            return false;
        }
        if !world
            .resource::<QuestResource>()
            .quests
            .iter()
            .any(|quest| quest.quest_id == current && quest.stage == QuestStage::Completed)
        {
            return false;
        }
        current = crystal_quest_info_by_id(current)
            .map(|info| effective_crystal_required_quest_id(world, info.index, info.quest_needed))
            .unwrap_or(0);
    }
    true
}

pub(super) fn advance_crystal_quest_kill(
    world: &mut World,
    monster_name: &str,
) -> Vec<ServerPacket> {
    let mut changed = Vec::new();
    {
        let mut quests = world.resource_mut::<QuestResource>();
        for quest in quests
            .quests
            .iter_mut()
            .filter(|quest| quest.stage == QuestStage::InProgress)
        {
            let Some(template) = crystal_quest_template_by_id(quest.quest_id) else {
                continue;
            };
            for task in &template.kill_tasks {
                if !monster_name.starts_with(&task.monster_name) {
                    continue;
                }
                if increment_crystal_quest_task(
                    quest,
                    crystal_kill_task_key(task.monster_index),
                    1,
                    task.count.max(1),
                ) {
                    recompute_crystal_quest_current(quest, &template);
                    changed.push((quest.quest_id, task.monster_name.clone()));
                }
                break;
            }
        }
    }

    let mut packets = Vec::new();
    for (quest_id, killed_name) in changed {
        packets.push(ServerPacket::SendOutputMessage {
            message: format!("You killed {}.", killed_name),
            output_type: 1,
        });
        if let Some(packet) = crystal_quest_update_packet(world, quest_id) {
            packets.push(packet);
        }
    }
    packets
}

pub(super) fn advance_crystal_quest_flag(world: &mut World, flag_number: i32) -> Vec<ServerPacket> {
    let mut changed = Vec::new();
    {
        let mut quests = world.resource_mut::<QuestResource>();
        for quest in quests
            .quests
            .iter_mut()
            .filter(|quest| quest.stage == QuestStage::InProgress)
        {
            let Some(template) = crystal_quest_template_by_id(quest.quest_id) else {
                continue;
            };
            for task in &template.flag_tasks {
                if task.number != flag_number {
                    continue;
                }
                if increment_crystal_quest_task(quest, crystal_flag_task_key(task.number), 1, 1) {
                    recompute_crystal_quest_current(quest, &template);
                    changed.push(quest.quest_id);
                }
                break;
            }
        }
    }
    changed
        .into_iter()
        .filter_map(|quest_id| crystal_quest_update_packet(world, quest_id))
        .collect()
}

pub(super) fn crystal_quest_update_packet(world: &World, quest_id: i32) -> Option<ServerPacket> {
    let stage = quest_stage(world, quest_id)?;
    Some(ServerPacket::ChangeQuest {
        quest_id,
        task_list: crystal_quest_task_list(world, quest_id)?,
        taken: matches!(stage, QuestStage::InProgress | QuestStage::ReadyToTurnIn),
        completed: matches!(stage, QuestStage::ReadyToTurnIn | QuestStage::Completed),
        new: false,
        quest_state: 1,
        track_quest: false,
    })
}

#[derive(Debug, Clone)]
pub(super) struct CrystalQuestItemTaskDrop {
    pub(super) quest_id: i32,
    pub(super) task_key: String,
    pub(super) item_key: String,
    pub(super) item_name: String,
    pub(super) description: String,
    pub(super) weight: u16,
    pub(super) required: u32,
}

pub(super) fn crystal_quest_item_task_drop(
    world: &World,
    key: &str,
    name: &str,
    quantity: u32,
) -> Option<CrystalQuestItemTaskDrop> {
    let resources = world.resource::<InventoryResource>();
    world
        .resource::<QuestResource>()
        .quests
        .iter()
        .filter(|quest| quest.stage == QuestStage::InProgress)
        .find_map(|quest| {
            let template = crystal_quest_template_by_id(quest.quest_id)?;
            for task in &template.item_tasks {
                if !crystal_quest_task_item_matches_drop(task, key, name) {
                    continue;
                }
                let task_key = crystal_item_task_key(task.item_index);
                let required = u32::from(task.count.max(1));
                if crystal_quest_task_progress(quest, &task_key) >= required {
                    continue;
                }
                let item_key = crystal_quest_task_item_key(task);
                if !can_gain_item_quantity(resources, ItemContainer::Quest, &item_key, quantity) {
                    continue;
                }
                let (item_name, description, weight) = crystal_item_by_index(task.item_index)
                    .map(|template| {
                        (
                            template.name,
                            template
                                .tooltip
                                .unwrap_or_else(|| "Original Crystal quest item.".to_string()),
                            u16::from(template.weight.max(1)),
                        )
                    })
                    .unwrap_or_else(|| {
                        (
                            task.item_name.clone(),
                            "Original Crystal quest item.".to_string(),
                            1,
                        )
                    });
                return Some(CrystalQuestItemTaskDrop {
                    quest_id: quest.quest_id,
                    task_key,
                    item_key,
                    item_name,
                    description,
                    weight,
                    required,
                });
            }
            None
        })
}

pub(super) fn advance_crystal_quest_item_task(
    world: &mut World,
    quest_id: i32,
    task_key: &str,
    quantity: u32,
) -> bool {
    let Some(template) = crystal_quest_template_by_id(quest_id) else {
        return false;
    };
    let mut quests = world.resource_mut::<QuestResource>();
    let Some(quest) = quests
        .quests
        .iter_mut()
        .find(|quest| quest.quest_id == quest_id && quest.stage == QuestStage::InProgress)
    else {
        return false;
    };
    let Some(task) = template
        .item_tasks
        .iter()
        .find(|task| crystal_item_task_key(task.item_index) == task_key)
    else {
        return false;
    };
    let changed = increment_crystal_quest_task(
        quest,
        task_key.to_string(),
        quantity.max(1),
        u32::from(task.count.max(1)),
    );
    if changed {
        recompute_crystal_quest_current(quest, &template);
    }
    changed
}

fn crystal_quest_task_item_matches_drop(
    task: &CrystalQuestItemTaskTemplate,
    key: &str,
    name: &str,
) -> bool {
    let task_key = crystal_quest_task_item_key(task);
    task_key.eq_ignore_ascii_case(key)
        || task.item_name.eq_ignore_ascii_case(name.trim())
        || normalize_crystal_item_key(&task.item_name).eq_ignore_ascii_case(key)
}

fn crystal_quest_class_matches(class: MirClass, class_needed: u8) -> bool {
    class_needed == 0 || (class_needed & crystal_quest_class_mask(class)) != 0
}

fn crystal_quest_class_mask(class: MirClass) -> u8 {
    match class {
        MirClass::Warrior => 0x01,
        MirClass::Wizard => 0x02,
        MirClass::Taoist => 0x04,
        MirClass::Assassin => 0x08,
        MirClass::Archer => 0x10,
    }
}

pub(super) fn quest_log_snapshots(world: &World, language: LanguageCode) -> Vec<QuestSnapshot> {
    let quests = world.resource::<QuestResource>();
    let mut seen = BTreeSet::<i32>::new();
    let mut snapshots = Vec::new();
    for quest in &quests.quests {
        seen.insert(quest.quest_id);
        snapshots.push(quest.snapshot(language));
    }
    for mut info in crystal_normal_quest_infos_1_to_45() {
        apply_content_profile_quest_reward_overrides(world, &mut info);
        if seen.contains(&info.index) || !can_accept_crystal_quest(world, &info) {
            continue;
        }
        snapshots
            .push(QuestState::from_crystal_info(&info, QuestStage::Available).snapshot(language));
    }
    snapshots
}

pub(super) fn quest_dialog_links_for_npc(
    world: &World,
    npc_object_id: u32,
    quest_ids: &[i32],
) -> Vec<NpcDialogLinkState> {
    let mut links = Vec::new();
    for quest_id in quest_ids {
        let Some(info) = effective_crystal_quest_info_by_id(world, *quest_id) else {
            continue;
        };
        let stage = quest_stage(world, info.index).unwrap_or(QuestStage::Available);
        match stage {
            QuestStage::Available => {
                if crystal_quest_start_npc_matches(&info, npc_object_id)
                    && can_accept_crystal_quest(world, &info)
                {
                    links.push(NpcDialogLinkState {
                        text: format!("Accept {}", info.name),
                        target: format!("@quest:accept:{}", info.index),
                    });
                }
            }
            QuestStage::InProgress => {
                if crystal_quest_finish_npc_matches(&info, npc_object_id)
                    || crystal_quest_start_npc_matches(&info, npc_object_id)
                {
                    links.push(NpcDialogLinkState {
                        text: info.name.clone(),
                        target: format!("@quest:show:{}", info.index),
                    });
                }
            }
            QuestStage::ReadyToTurnIn => {
                if crystal_quest_finish_npc_matches(&info, npc_object_id) {
                    links.push(NpcDialogLinkState {
                        text: format!("Complete {}", info.name),
                        target: format!("@quest:finish:{}", info.index),
                    });
                } else if crystal_quest_start_npc_matches(&info, npc_object_id) {
                    links.push(NpcDialogLinkState {
                        text: info.name.clone(),
                        target: format!("@quest:show:{}", info.index),
                    });
                }
            }
            QuestStage::Completed => {}
        }
    }
    links.sort_by(|left, right| left.text.cmp(&right.text));
    links.dedup_by(|left, right| left.target == right.target);
    links
}

pub(super) fn crystal_quest_dialog_for_target(
    world: &mut World,
    npc_object_id: u32,
    target: &str,
) -> Option<(ActiveCrystalQuestDialog, Vec<ServerPacket>)> {
    let mut parts = target.trim().trim_start_matches('@').split(':');
    if !parts.next()?.eq_ignore_ascii_case("quest") {
        return None;
    }
    let action = parts.next()?;
    let quest_id = parts.next()?.parse::<i32>().ok()?;
    let selected_item_index = parts.next().and_then(|value| value.parse::<i32>().ok());
    let info = effective_crystal_quest_info_by_id(world, quest_id)?;
    match action.to_ascii_lowercase().as_str() {
        "accept" => Some(accept_crystal_quest_from_npc(world, npc_object_id, &info)),
        "finish" => Some(finish_crystal_quest_from_npc(
            world,
            npc_object_id,
            &info,
            selected_item_index,
        )),
        "show" => Some(show_crystal_quest_dialog(world, npc_object_id, &info)),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub(super) struct ActiveCrystalQuestDialog {
    pub(super) stage: QuestStage,
    pub(super) title: String,
    pub(super) body: Vec<String>,
    pub(super) footer: String,
    pub(super) links: Vec<NpcDialogLinkState>,
}

fn accept_crystal_quest_from_npc(
    world: &mut World,
    npc_object_id: u32,
    info: &ClientQuestInfo,
) -> (ActiveCrystalQuestDialog, Vec<ServerPacket>) {
    let mut packets = Vec::new();
    if !crystal_quest_start_npc_matches(info, npc_object_id)
        || !can_accept_crystal_quest(world, info)
    {
        packets.push(ServerPacket::Chat {
            message: "Could not accept quest.".to_string(),
            chat_type: ChatType::System,
        });
        return (
            crystal_quest_dialog_copy(info, QuestStage::Available, false),
            packets,
        );
    }
    let stage = begin_quest(world, info.index);
    packets.push(ServerPacket::ChangeQuest {
        quest_id: info.index,
        task_list: crystal_quest_task_list(world, info.index)
            .unwrap_or_else(|| info.task_description.clone()),
        taken: true,
        completed: stage == QuestStage::ReadyToTurnIn,
        new: true,
        quest_state: 0,
        track_quest: true,
    });
    (crystal_quest_dialog_copy(info, stage, false), packets)
}

fn finish_crystal_quest_from_npc(
    world: &mut World,
    npc_object_id: u32,
    info: &ClientQuestInfo,
    selected_item_index: Option<i32>,
) -> (ActiveCrystalQuestDialog, Vec<ServerPacket>) {
    let mut packets = Vec::new();
    if !crystal_quest_finish_npc_matches(info, npc_object_id) {
        return (
            crystal_quest_dialog_copy(info, QuestStage::InProgress, false),
            packets,
        );
    }
    ensure_runtime_quest(world, info.index);
    let stage = quest_stage(world, info.index).unwrap_or(QuestStage::Available);
    if stage == QuestStage::InProgress {
        packets.push(ServerPacket::Chat {
            message: "Quest task is not complete yet.".to_string(),
            chat_type: ChatType::System,
        });
    } else if stage == QuestStage::ReadyToTurnIn {
        if crystal_quest_reward_selection_missing(info.index, selected_item_index) {
            return (crystal_quest_reward_selection_dialog(info, stage), packets);
        }
        if complete_quest_with_selection(world, info.index, selected_item_index) {
            packets.push(ServerPacket::ChangeQuest {
                quest_id: info.index,
                task_list: Vec::new(),
                taken: false,
                completed: true,
                new: false,
                quest_state: 2,
                track_quest: false,
            });
            packets.push(ServerPacket::CompleteQuest {
                completed_quests: completed_quest_ids(world),
            });
        } else {
            packets.push(ServerPacket::Chat {
                message: "Cannot hand in quest; your bag is full.".to_string(),
                chat_type: ChatType::System,
            });
        }
    }
    (
        crystal_quest_dialog_copy(info, quest_stage(world, info.index).unwrap_or(stage), false),
        packets,
    )
}

fn crystal_quest_reward_selection_dialog(
    info: &ClientQuestInfo,
    stage: QuestStage,
) -> ActiveCrystalQuestDialog {
    let mut dialog = crystal_quest_dialog_copy(info, stage, false);
    dialog.links = info
        .rewards_select_item
        .iter()
        .enumerate()
        .map(|(index, reward)| {
            let count = if reward.count > 1 {
                format!(" x{}", reward.count)
            } else {
                String::new()
            };
            NpcDialogLinkState {
                text: format!("Choose {}{}", reward.item.name, count),
                target: format!("@quest:finish:{}:{index}", info.index),
            }
        })
        .collect();
    dialog.links.push(NpcDialogLinkState {
        text: "Exit".to_string(),
        target: "@Exit".to_string(),
    });
    dialog
}

fn show_crystal_quest_dialog(
    world: &mut World,
    npc_object_id: u32,
    info: &ClientQuestInfo,
) -> (ActiveCrystalQuestDialog, Vec<ServerPacket>) {
    let stage = quest_stage(world, info.index).unwrap_or(QuestStage::Available);
    let has_finish_link =
        stage == QuestStage::ReadyToTurnIn && crystal_quest_finish_npc_matches(info, npc_object_id);
    (
        crystal_quest_dialog_copy(info, stage, has_finish_link),
        Vec::new(),
    )
}

fn crystal_quest_dialog_copy(
    info: &ClientQuestInfo,
    stage: QuestStage,
    include_finish_link: bool,
) -> ActiveCrystalQuestDialog {
    let body = match stage {
        QuestStage::Available => info.description.clone(),
        QuestStage::InProgress => info.task_description.clone(),
        QuestStage::ReadyToTurnIn => info.return_description.clone(),
        QuestStage::Completed => info.completion_description.clone(),
    };
    let body = if body.is_empty() {
        vec![crystal_quest_summary(info)]
    } else {
        body
    };
    let mut links = Vec::new();
    if include_finish_link {
        links.push(NpcDialogLinkState {
            text: format!("Complete {}", info.name),
            target: format!("@quest:finish:{}", info.index),
        });
    }
    links.push(NpcDialogLinkState {
        text: "Exit".to_string(),
        target: "@Exit".to_string(),
    });
    ActiveCrystalQuestDialog {
        stage,
        title: info.name.clone(),
        body,
        footer: crystal_quest_reward_preview(info),
        links,
    }
}

pub(super) fn crystal_quest_start_npc_matches(info: &ClientQuestInfo, npc_object_id: u32) -> bool {
    crystal_quest_npc_index_matches(info.npc_index, npc_object_id)
}

pub(super) fn crystal_quest_finish_npc_matches(info: &ClientQuestInfo, npc_object_id: u32) -> bool {
    if info.finish_npc_index == 0 {
        return crystal_quest_start_npc_matches(info, npc_object_id);
    }
    crystal_quest_npc_index_matches(info.finish_npc_index, npc_object_id)
}

fn crystal_quest_npc_index_matches(npc_index: u32, npc_object_id: u32) -> bool {
    npc_index == npc_object_id
        || crystal_npc_info_manifest().npcs.into_iter().any(|npc| {
            npc.npc_index == i32::try_from(npc_index).unwrap_or(i32::MAX)
                && npc.loaded_object_id == Some(npc_object_id)
        })
}

fn crystal_quest_finish_npc_label(info: &ClientQuestInfo) -> String {
    let index = if info.finish_npc_index == 0 {
        info.npc_index
    } else {
        info.finish_npc_index
    };
    crystal_npc_info_manifest()
        .npcs
        .into_iter()
        .find(|npc| {
            npc.npc_index == i32::try_from(index).unwrap_or(i32::MAX)
                || npc.loaded_object_id == Some(index)
        })
        .map(|npc| npc.name)
        .unwrap_or_else(|| format!("NPC {index}"))
}

pub(super) fn completed_quest_ids(world: &World) -> Vec<i32> {
    world
        .resource::<QuestResource>()
        .quests
        .iter()
        .filter(|quest| quest.stage == QuestStage::Completed)
        .map(|quest| quest.quest_id)
        .collect()
}

pub(super) fn npc_stage_dialog_for_object(
    language: LanguageCode,
    npc_object_id: u32,
    stage: QuestStage,
    current: u32,
    required: u32,
) -> Option<(String, Vec<String>, String, String)> {
    let template_stage_key = match stage {
        QuestStage::Available => "available",
        QuestStage::InProgress => "in_progress",
        QuestStage::ReadyToTurnIn => "ready_to_turn_in",
        QuestStage::Completed => "completed",
    };
    let stage_template = npc_script_for_object_id(npc_object_id)?
        .stages
        .into_iter()
        .find(|entry| entry.quest_stage == template_stage_key)?;
    let base = localized_npc_dialog_base_key(npc_object_id);
    let localized_stage_key = quest_stage_key(stage);
    Some((
        localized_text_or_fallback(
            language,
            &format!("{base}.{localized_stage_key}.title"),
            &stage_template.title,
        ),
        stage_template
            .body
            .into_iter()
            .enumerate()
            .map(|(index, line)| {
                render_template(
                    localized_text_or_fallback(
                        language,
                        &format!("{base}.{localized_stage_key}.body.{index}"),
                        &line,
                    ),
                    current,
                    required,
                )
            })
            .collect(),
        render_template(
            localized_text_or_fallback(
                language,
                &format!("{base}.{localized_stage_key}.footer"),
                &stage_template.footer,
            ),
            current,
            required,
        ),
        render_template(
            localized_text_or_fallback(
                language,
                &format!("{base}.{localized_stage_key}.objectChat"),
                &stage_template.object_chat,
            ),
            current,
            required,
        ),
    ))
}

pub(super) fn localized_quest_stage_copy(
    language: LanguageCode,
    quest_id: i32,
    stage: QuestStage,
    fallback_template: &QuestTemplate,
) -> QuestStageCopy {
    let fallback = match stage {
        QuestStage::Available => &fallback_template.stages.available,
        QuestStage::InProgress => &fallback_template.stages.in_progress,
        QuestStage::ReadyToTurnIn => &fallback_template.stages.ready_to_turn_in,
        QuestStage::Completed => &fallback_template.stages.completed,
    };
    let Some(base) = localized_quest_base_key(quest_id) else {
        return fallback.clone();
    };
    let stage_key = quest_stage_key(stage);
    QuestStageCopy {
        objective: localized_text_or_fallback(
            language,
            &format!("{base}.stage.{stage_key}.objective"),
            &fallback.objective,
        ),
        progress_label: localized_text_or_fallback(
            language,
            &format!("{base}.stage.{stage_key}.progressLabel"),
            &fallback.progress_label,
        ),
        tracker: localized_text_or_fallback(
            language,
            &format!("{base}.stage.{stage_key}.tracker"),
            &fallback.tracker,
        ),
    }
}
