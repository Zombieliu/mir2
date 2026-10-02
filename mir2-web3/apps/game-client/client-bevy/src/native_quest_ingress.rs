//! Shared Windows/Android authoritative quest and NPC presentation projection.
//! Pure functions extracted from frozen Windows3d735745f. No quest progress,
//! reward, inventory, script, authentication or save authority.
use crate::inventory::{CrystalItemInfoModel, CrystalItemTooltipSourceModel, CrystalUserItemModel};
use crate::native_inventory_ingress::{
    native_real_tooltip_info as crystal_real_tooltip_info,
    native_tooltip_info as unique_crystal_tooltip_info,
    native_tooltip_viewer as crystal_tooltip_viewer,
};
use crate::native_player_ingress::NativeUiPlayerCursor;
use crate::quest_model::{
    NearbyNpc, NearbyNpcModel, NpcDialogModel, NpcDialogOption, NpcDialogUpdate, Quest,
    QuestDetailText, QuestObjective, QuestReward, QuestStatus, QuestTracker,
};
use serde_json::{json, Value};
use std::collections::HashMap;
const MAX_NEARBY_NPCS: usize = 8;
const MAX_NEARBY_DISTANCE: u32 = 18;

#[derive(Debug, Clone, Default)]
pub struct QuestDefinition {
    pub title: String,
    pub group: Option<String>,
    pub min_level_needed: i32,
    pub detail: QuestDetailText,
    pub accept_npc_index: Option<u32>,
    pub finish_npc_index: Option<u32>,
    pub objectives: Vec<String>,
    pub rewards: Vec<QuestReward>,
    pub description: Option<String>,
}

pub fn parse_quest_definition(payload: &Value) -> QuestDefinition {
    let info = payload.get("info");
    let title = string_at(payload, "name")
        .or_else(|| info.and_then(|value| string_at(value, "name")))
        .unwrap_or_default();
    let group = string_at(payload, "group")
        .or_else(|| info.and_then(|value| string_at(value, "group")))
        .filter(|value| !value.trim().is_empty());
    let min_level_needed = payload
        .get("minLevelNeeded")
        .and_then(value_i32)
        .or_else(|| {
            info.and_then(|value| {
                value
                    .get("min_level_needed")
                    .or_else(|| value.get("minLevelNeeded"))
                    .and_then(value_i32)
            })
        })
        .unwrap_or(0);
    let accept_npc_index = info
        .and_then(|value| value.get("npc_index").or_else(|| value.get("npcIndex")))
        .and_then(value_u32);
    let finish_npc_index = info
        .and_then(|value| {
            value
                .get("finish_npc_index")
                .or_else(|| value.get("finishNpcIndex"))
        })
        .and_then(value_u32)
        .or(accept_npc_index);
    let objectives: Vec<String> = payload
        .get("objectives")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| string_at(item, "text"))
                .map(|text| strip_crystal_markup(&text))
                .filter(|text| !text.trim().is_empty())
                .collect()
        })
        .unwrap_or_default();
    let description_lines = payload
        .get("descriptionLines")
        .and_then(string_array)
        .or_else(|| {
            info.and_then(|value| value.get("description"))
                .and_then(string_array)
        })
        .unwrap_or_default()
        .into_iter()
        .map(|line| strip_crystal_markup(&line))
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    let task_description_lines = if objectives.is_empty() {
        info.and_then(|value| value.get("task_description"))
            .and_then(string_array)
            .unwrap_or_default()
            .into_iter()
            .map(|line| strip_crystal_markup(&line))
            .filter(|line| !line.trim().is_empty())
            .collect()
    } else {
        objectives.clone()
    };
    let return_description_lines = payload
        .get("returnDescriptionLines")
        .and_then(string_array)
        .or_else(|| {
            info.and_then(|value| value.get("return_description"))
                .and_then(string_array)
        })
        .unwrap_or_default()
        .into_iter()
        .map(|line| strip_crystal_markup(&line))
        .filter(|line| !line.trim().is_empty())
        .collect();
    let completion_description_lines = payload
        .get("completionDescriptionLines")
        .and_then(string_array)
        .or_else(|| {
            info.and_then(|value| value.get("completion_description"))
                .and_then(string_array)
        })
        .unwrap_or_default()
        .into_iter()
        .map(|line| strip_crystal_markup(&line))
        .filter(|line| !line.trim().is_empty())
        .collect();
    let time_limit = string_at(payload, "timeLimit").filter(|value| !value.trim().is_empty());
    let description = (!description_lines.is_empty()).then(|| description_lines.join("\n"));
    let detail = QuestDetailText {
        description_lines,
        task_description_lines,
        return_description_lines,
        completion_description_lines,
        time_limit,
    };

    QuestDefinition {
        title: strip_crystal_markup(&title),
        group,
        min_level_needed,
        detail,
        accept_npc_index,
        finish_npc_index,
        objectives,
        rewards: parse_quest_rewards(payload.get("rewards")),
        description,
    }
}

pub fn transform_quest_tracker(
    payload: &Value,
    definitions: &HashMap<i32, QuestDefinition>,
) -> QuestTracker {
    let entities = payload
        .get("entities")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let active_quests = payload
        .get("questLog")
        .and_then(Value::as_array)
        .map(|quests| {
            quests
                .iter()
                .filter_map(|quest| {
                    let quest_index = quest.get("questId").and_then(value_i32)?;
                    let definition = definitions.get(&quest_index).cloned().unwrap_or_default();
                    let status = quest_status(quest.get("stage").and_then(Value::as_str));
                    let npc_index = if status == QuestStatus::ReadyToTurnIn {
                        definition.finish_npc_index
                    } else {
                        definition.accept_npc_index
                    };
                    let npc_name = npc_index.and_then(|npc_index| {
                        entities
                            .iter()
                            .find(|entity| {
                                entity.get("objectId").and_then(value_u32) == Some(npc_index)
                            })
                            .and_then(|entity| string_at(entity, "name"))
                            .map(|name| display_npc_name(&name))
                    });
                    let objectives = transform_quest_objectives(quest, quest_index, &definition);
                    let rewards = if definition.rewards.is_empty() {
                        string_at(quest, "rewardPreview")
                            .filter(|label| !label.trim().is_empty())
                            .map(|label| vec![QuestReward::Unknown { label }])
                            .unwrap_or_default()
                    } else {
                        definition.rewards.clone()
                    };
                    let title = string_at(quest, "title")
                        .filter(|title| !title.trim().is_empty())
                        .unwrap_or_else(|| {
                            if definition.title.is_empty() {
                                format!("Quest {quest_index}")
                            } else {
                                definition.title.clone()
                            }
                        });
                    let unknown_text = definition.description.clone().or_else(|| {
                        string_at(quest, "summary").filter(|text| !text.trim().is_empty())
                    });

                    Some(Quest {
                        quest_index,
                        accept_npc_index: definition.accept_npc_index,
                        finish_npc_index: definition.finish_npc_index,
                        title: strip_crystal_markup(&title),
                        npc_name,
                        group: definition.group.clone(),
                        min_level_needed: definition.min_level_needed,
                        detail: definition.detail.clone(),
                        status,
                        objectives,
                        rewards,
                        unknown_text,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    QuestTracker { active_quests }
}

pub fn transform_quest_objectives(
    quest: &Value,
    quest_index: i32,
    definition: &QuestDefinition,
) -> Vec<QuestObjective> {
    let current_total = quest.get("current").and_then(value_u32).unwrap_or(0);
    let required_total = quest.get("required").and_then(value_u32).unwrap_or(0);
    let from_snapshot = quest
        .get("objectives")
        .and_then(Value::as_array)
        .map(|objectives| {
            objectives
                .iter()
                .enumerate()
                .map(|(index, objective)| QuestObjective {
                    objective_id: format!("{quest_index}:{index}"),
                    text: string_at(objective, "label")
                        .map(|text| strip_crystal_markup(&text))
                        .or_else(|| definition.objectives.get(index).cloned())
                        .unwrap_or_else(|| format!("Objective {}", index + 1)),
                    current: objective
                        .get("current")
                        .and_then(value_u32)
                        .unwrap_or(current_total),
                    target: objective
                        .get("required")
                        .and_then(value_u32)
                        .unwrap_or(required_total),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !from_snapshot.is_empty() {
        return from_snapshot;
    }

    let text = string_at(quest, "objective")
        .map(|text| strip_crystal_markup(&text))
        .or_else(|| definition.objectives.first().cloned())
        .unwrap_or_default();
    if text.trim().is_empty() && required_total == 0 {
        Vec::new()
    } else {
        vec![QuestObjective {
            objective_id: format!("{quest_index}:0"),
            text,
            current: current_total,
            target: required_total,
        }]
    }
}

pub fn parse_quest_rewards(value: Option<&Value>) -> Vec<QuestReward> {
    let Some(rewards) = value.and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut parsed = Vec::new();
    if let Some(amount) = rewards
        .get("gold")
        .and_then(value_u32)
        .filter(|amount| *amount > 0)
    {
        parsed.push(QuestReward::Gold { amount });
    }
    if let Some(amount) = rewards
        .get("experience")
        .and_then(value_u32)
        .filter(|amount| *amount > 0)
    {
        parsed.push(QuestReward::Experience { amount });
    }
    if let Some(amount) = rewards
        .get("credit")
        .and_then(value_u32)
        .filter(|amount| *amount > 0)
    {
        parsed.push(QuestReward::Unknown {
            label: format!("{amount} Credit"),
        });
    }
    for field in ["items", "selectItems"] {
        if let Some(items) = rewards.get(field).and_then(Value::as_array) {
            for item in items {
                let item_id = item
                    .get("itemIndex")
                    .and_then(value_i32)
                    .map(|value| value.to_string())
                    .unwrap_or_default();
                let name = string_at(item, "name").unwrap_or_else(|| "Item".to_owned());
                // Zero is meaningful in Crystal quest data: it marks a listed
                // fixed item that is not actually granted. Preserve it so the
                // newcomer presentation can filter the row instead of
                // fabricating an x1 reward.
                let quantity = item.get("count").and_then(value_u32).unwrap_or(1);
                parsed.push(QuestReward::Item {
                    item_id,
                    name: strip_crystal_markup(&name),
                    quantity,
                    icon: item.get("icon").and_then(value_u32),
                    selection_index: (field == "selectItems")
                        .then(|| item.get("selectionIndex").and_then(value_i32).unwrap_or(0)),
                    tooltip_source: item
                        .get("tooltipSource")
                        .and_then(|source| serde_json::from_value(source.clone()).ok()),
                });
            }
        }
    }
    parsed
}

pub fn transform_npc_dialog(payload: &Value) -> NpcDialogModel {
    let Some(dialog) = payload
        .get("activeNpcDialog")
        .filter(|value| !value.is_null())
    else {
        return NpcDialogModel::default();
    };
    let Some(npc_object_id) = dialog.get("npcObjectId").and_then(value_u32) else {
        return NpcDialogModel::default();
    };
    let mut lines = Vec::new();
    if let Some(title) = string_at(dialog, "title").filter(|title| !title.trim().is_empty()) {
        lines.push(strip_crystal_markup(&title));
    }
    if let Some(body) = dialog.get("body").and_then(string_array) {
        lines.extend(body.into_iter().map(|line| strip_crystal_markup(&line)));
    }
    if let Some(footer) = string_at(dialog, "footer").filter(|footer| !footer.trim().is_empty()) {
        lines.push(strip_crystal_markup(&footer));
    }
    let options = dialog
        .get("links")
        .and_then(Value::as_array)
        .map(|links| {
            links
                .iter()
                .filter_map(|link| {
                    let target = string_at(link, "target")?;
                    Some(NpcDialogOption {
                        option_id: target,
                        label: string_at(link, "text")
                            .map(|text| strip_crystal_markup(&text))
                            .unwrap_or_else(|| "Continue".to_owned()),
                        enabled: true,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let mut model = NpcDialogModel::default();
    model.apply(NpcDialogUpdate {
        npc_object_id,
        npc_name: string_at(dialog, "npcName").map(|name| display_npc_name(&name)),
        lines,
        options,
        open: true,
        replace: true,
    });
    model
}

pub fn transform_nearby_npcs(payload: &Value, player_x: i32, player_y: i32) -> NearbyNpcModel {
    let mut npcs = payload
        .get("entities")
        .and_then(Value::as_array)
        .map(|entities| {
            entities
                .iter()
                .filter(|entity| entity.get("kind").and_then(Value::as_str) == Some("npc"))
                .filter_map(|entity| {
                    let object_id = entity.get("objectId").and_then(value_u32)?;
                    let x = entity.get("x").and_then(value_i32)?;
                    let y = entity.get("y").and_then(value_i32)?;
                    let distance = tile_distance(player_x, player_y, x, y);
                    (distance <= MAX_NEARBY_DISTANCE).then(|| NearbyNpc {
                        object_id,
                        name: string_at(entity, "name")
                            .map(|name| display_npc_name(&name))
                            .unwrap_or_else(|| format!("NPC {object_id}")),
                        x,
                        y,
                        quest_indexes: entity
                            .get("questIds")
                            .and_then(Value::as_array)
                            .map(|ids| ids.iter().filter_map(value_i32).collect())
                            .unwrap_or_default(),
                        distance,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    npcs.sort_by_key(|npc| (npc.distance, npc.object_id));
    npcs.truncate(MAX_NEARBY_NPCS);
    NearbyNpcModel { npcs }
}

pub fn quest_status(stage: Option<&str>) -> QuestStatus {
    match stage.unwrap_or_default().to_ascii_lowercase().as_str() {
        "available" | "notstarted" | "not_started" => QuestStatus::NotStarted,
        "inprogress" | "in_progress" | "active" => QuestStatus::InProgress,
        "readytoturnin" | "ready_to_turn_in" => QuestStatus::ReadyToTurnIn,
        "completed" => QuestStatus::Completed,
        "failed" => QuestStatus::Failed,
        "aborted" => QuestStatus::Aborted,
        other => QuestStatus::Unknown(other.to_owned()),
    }
}

pub fn completed_quest_ids(value: &Value) -> Option<Vec<i32>> {
    let values = value.as_array()?;
    Some(values.iter().filter_map(value_i32).collect())
}

pub fn completed_quest_ids_from_snapshot(payload: &Value) -> Option<Vec<i32>> {
    if let Some(ids) = payload
        .get("completedQuests")
        .or_else(|| payload.get("completed_quests"))
        .or_else(|| payload.get("completedQuestIds"))
        .and_then(completed_quest_ids)
    {
        return Some(ids);
    }
    let quests = payload.get("questLog")?.as_array()?;
    Some(
        quests
            .iter()
            .filter(|quest| {
                quest_status(quest.get("stage").and_then(Value::as_str)) == QuestStatus::Completed
            })
            .filter_map(|quest| quest.get("questId").and_then(value_i32))
            .collect(),
    )
}

pub fn crystal_wire_item_info(value: &Value) -> Option<CrystalItemInfoModel> {
    let mut object = value.as_object()?.clone();
    if !object.contains_key("item_index") {
        object.insert(
            "item_index".to_owned(),
            object.get("index").cloned().unwrap_or(Value::Null),
        );
    }
    serde_json::from_value(Value::Object(object)).ok()
}

/// Mirrors `new UserItem(info)` at the two Crystal catalogue-only surfaces.
/// GameShop supplies a count; QuestCell leaves the constructor's zero count
/// alone and paints the reward quantity as a separate cell label.
pub fn crystal_tooltip_source_for_preview(
    info: CrystalItemInfoModel,
    count: u16,
    cursor: &NativeUiPlayerCursor,
) -> CrystalItemTooltipSourceModel {
    let user_item = CrystalUserItemModel {
        item_index: info.item_index,
        current_dura: info.durability,
        max_dura: info.durability,
        count,
        identified: false,
        slots: vec![None; usize::from(info.slots)],
        ..Default::default()
    };
    let viewer = crystal_tooltip_viewer(cursor);
    CrystalItemTooltipSourceModel {
        real_info: crystal_real_tooltip_info(&info, viewer),
        info,
        user_item: Some(user_item),
        socket_infos: Vec::new(),
        real_socket_infos: Vec::new(),
    }
}

// Gateway and gameplay adapters had different value_i32 signatures. Preserve
// the Gateway function body and its Option<&Value> helper in this namespace.
mod reward_projection {
    use super::*;
    fn value_i32(value: Option<&Value>) -> Option<i32> {
        value.and_then(|value| {
            value
                .as_i64()
                .and_then(|number| i32::try_from(number).ok())
                .or_else(|| value.as_str()?.parse::<i32>().ok())
        })
    }
    pub fn add_quest_reward_tooltip_sources(payload: &mut Value, cursor: &NativeUiPlayerCursor) {
        let Some(payload_object) = payload.as_object_mut() else {
            return;
        };
        let raw_info = payload_object.get("info").cloned();
        let Some(rewards) = payload_object
            .get_mut("rewards")
            .and_then(Value::as_object_mut)
        else {
            return;
        };
        for (rendered_key, raw_key) in [
            ("items", "rewards_fixed_item"),
            ("selectItems", "rewards_select_item"),
        ] {
            let raw_items = raw_info
                .as_ref()
                .and_then(|info| info.get(raw_key))
                .and_then(Value::as_array);
            let Some(rendered_items) = rewards.get_mut(rendered_key).and_then(Value::as_array_mut)
            else {
                continue;
            };
            for (index, rendered) in rendered_items.iter_mut().enumerate() {
                let info = raw_items
                    .and_then(|items| items.get(index))
                    .and_then(|reward| reward.get("item"))
                    .and_then(crystal_wire_item_info)
                    .or_else(|| {
                        value_i32(rendered.get("itemIndex")).and_then(unique_crystal_tooltip_info)
                    });
                let (Some(info), Some(object)) = (info, rendered.as_object_mut()) else {
                    continue;
                };
                object.insert(
                    "tooltipSource".to_owned(),
                    json!(crystal_tooltip_source_for_preview(info, 0, cursor)),
                );
            }
        }
    }
}
pub use reward_projection::add_quest_reward_tooltip_sources;

fn value_u32(value: &Value) -> Option<u32> {
    value
        .as_u64()
        .and_then(|number| u32::try_from(number).ok())
        .or_else(|| value.as_str()?.parse::<u32>().ok())
}

fn value_i32(value: &Value) -> Option<i32> {
    value
        .as_i64()
        .and_then(|number| i32::try_from(number).ok())
        .or_else(|| value.as_str()?.parse::<i32>().ok())
}

fn string_at(value: &Value, field: &str) -> Option<String> {
    value.get(field).and_then(Value::as_str).map(str::to_owned)
}

fn string_array(value: &Value) -> Option<Vec<String>> {
    Some(
        value
            .as_array()?
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
    )
}

fn tile_distance(ax: i32, ay: i32, bx: i32, by: i32) -> u32 {
    ax.abs_diff(bx).max(ay.abs_diff(by))
}

fn display_npc_name(name: &str) -> String {
    name.replace('_', " - ")
}

fn strip_crystal_markup(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(character) = chars.next() {
        if character != '{' {
            output.push(character);
            continue;
        }
        let mut token = String::new();
        let mut closed = false;
        for next in chars.by_ref() {
            if next == '}' {
                closed = true;
                break;
            }
            token.push(next);
        }
        if closed {
            output.push_str(token.split('/').next().unwrap_or_default());
        } else {
            output.push('{');
            output.push_str(&token);
        }
    }
    output
}
