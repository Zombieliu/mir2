use serde::{Deserialize, Serialize};

use crate::config::{ItemContainer, QuestSnapshot, QuestStage};
use bevy_ecs::prelude::World;
use mir2_game_data::{
    localized_text_or_fallback, starter_server_data, LanguageCode, QuestStageCopy, QuestTemplate,
};

use super::crystal_compat::GUIDE_QUEST_ID;
use super::equipment::{equipment_template_to_state, replace_equipment};
use super::inventory::{
    add_or_increment_item, additional_slots_needed_for_item_quantity, free_bag_slots,
};
use super::npc::{localized_npc_dialog_base_key, npc_script_for_object_id};
use super::npc_script::{NpcInteractionContext, NpcQuestDialog};
use super::resources::{InventoryResource, PlayerRuntimeResource, QuestResource, SessionResource};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct QuestState {
    pub(super) quest_id: i32,
    pub(super) title: String,
    pub(super) summary: String,
    pub(super) reward_preview: String,
    pub(super) required: u32,
    pub(super) current: u32,
    pub(super) stage: QuestStage,
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
        }
    }
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

pub(super) fn stage_copy(quest: &QuestState, language: LanguageCode) -> QuestStageCopy {
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

    let Some(template) = quest_template_by_id(quest_id) else {
        return QuestStage::Available;
    };
    let quest = QuestState {
        quest_id: template.quest_id,
        title: template.title,
        summary: template.summary,
        reward_preview: template.reward_preview,
        required: template.required,
        current: 0,
        stage: QuestStage::Available,
    };
    world.resource_mut::<QuestResource>().quests.push(quest);
    QuestStage::Available
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

pub(super) fn complete_quest(world: &mut World, quest_id: i32) {
    let Some(quest) = quest_template_by_id(quest_id) else {
        return;
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
            return;
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
