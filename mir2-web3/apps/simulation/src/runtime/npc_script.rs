use bevy_ecs::{
    entity::Entity,
    prelude::{Resource, World},
};
use serde::{Deserialize, Serialize};

use crate::config::{
    CurrencyKind, ItemContainer, QuestStage, Stage5HeroState, WorldEntityDisposition,
};
use mir2_game_data::{
    crystal_map_respawns_by_file_name, crystal_npc_info_manifest, crystal_npc_script_by_key,
    CrystalNpcInfoTemplate, CrystalNpcScript, CrystalNpcSection,
};
use mir2_protocol::{
    ChatType, MapInformation, MirClass, MirDirection, MirGender, Point, ServerPacket,
};

use super::buffs::*;
use super::components::{
    current_player_object_id, entity_by_object_id, entity_facing, entity_name, entity_position,
    player_entity, CharacterBody, DisplayName, Facing, Monster, MonsterAgent, MonsterVitals, Npc,
    NpcAgent, NpcPetState, ObjectId, Position, RemotePlayer, SummonedMonster, WorldObject,
};
use super::crystal_compat::*;
use super::inventory::*;
use super::items::*;
use super::map::*;
use super::monsters::*;
use super::movement::*;
use super::npc::*;
use super::packets::{collect_visible_objects, localized_npc_name_key};
use super::quests::*;
use super::resources::*;
use super::session::SimulationSession;
use super::skills::*;
use super::stage5::*;
use crate::config::{WorldEntityKind, WorldEntitySnapshot};

#[cfg(windows)]
#[repr(C)]
struct WindowsSystemTime {
    year: u16,
    month: u16,
    day_of_week: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
    milliseconds: u16,
}

#[cfg(windows)]
#[link(name = "Kernel32")]
extern "system" {
    fn GetLocalTime(system_time: *mut WindowsSystemTime);
}

#[derive(Resource, Debug, Clone, Copy)]
pub(super) struct CrystalNpcRandomState {
    pub(super) seed: u64,
}

impl CrystalNpcRandomState {
    pub(super) fn new() -> Self {
        Self { seed: 0 }
    }

    pub(super) fn next_index(&mut self, upper_bound: u32) -> Option<u32> {
        if upper_bound == 0 {
            return None;
        }

        self.seed = self
            .seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        Some(((self.seed >> 32) as u32) % upper_bound)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalNpcSavedValue {
    pub file_name: String,
    pub group: String,
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CrystalNpcScriptDiagnostic {
    pub(super) script_key: String,
    pub(super) label: String,
    pub(super) line_number: usize,
    pub(super) command: String,
    pub(super) message: String,
}

#[derive(Debug, Clone)]
pub(super) struct NpcInteractionContext {
    pub(super) object_id: u32,
    pub(super) name: String,
    pub(super) name_key: Option<String>,
    pub(super) position: Point,
    pub(super) quest_ids: Vec<i32>,
    pub(super) script_key: Option<String>,
    pub(super) args: Vec<String>,
    pub(super) input: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct CrystalNpcExecutionState {
    pub(super) map_file_name: Option<String>,
    pub(super) map_instance: i32,
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) label_args: Vec<String>,
    pub(super) input: Option<String>,
    pub(super) script_key: Option<String>,
    pub(super) section_label: Option<String>,
    pub(super) line_number: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CrystalNpcContextState {
    pub(super) npc_position: Point,
}

#[derive(Debug, Clone)]
pub(super) struct NpcQuestDialog {
    pub(super) stage_before_action: QuestStage,
    pub(super) current: u32,
    pub(super) required: u32,
    pub(super) title: String,
    pub(super) body: Vec<String>,
    pub(super) footer: String,
    pub(super) object_chat: String,
}

pub(super) fn crystal_npc_section<'a>(
    script: &'a CrystalNpcScript,
    label: &str,
) -> Option<&'a CrystalNpcSection> {
    script
        .sections
        .iter()
        .find(|section| crystal_npc_labels_match(&section.label, label))
}

pub(super) fn crystal_npc_labels_match(section_label: &str, requested_label: &str) -> bool {
    if section_label.eq_ignore_ascii_case(requested_label) {
        return true;
    }

    let section_base = crystal_npc_label_base(section_label);
    let requested_base = crystal_npc_label_base(requested_label);
    !section_base.is_empty() && section_base.eq_ignore_ascii_case(&requested_base)
}

pub(super) fn crystal_npc_label_base(label: &str) -> String {
    label
        .trim()
        .split_once('(')
        .map(|(head, _)| head.trim().to_string())
        .unwrap_or_else(|| label.trim().to_string())
}

pub(super) fn parse_crystal_goto_target(line: &str) -> Option<String> {
    let mut parts = line.split_whitespace();
    let command = parts.next()?;
    if !command.eq_ignore_ascii_case("GOTO") {
        return None;
    }
    parts
        .next()
        .map(|target| parse_crystal_label_target(target).0)
}

pub(super) fn parse_crystal_npc_links(line: &str) -> Vec<NpcDialogLinkState> {
    let mut links = Vec::new();
    let mut remainder = line;

    while let Some(start) = remainder.find('<') {
        let after_start = &remainder[start + 1..];
        let Some(end) = after_start.find('>') else {
            break;
        };
        let inside = &after_start[..end];
        if let Some((text, target)) = inside.rsplit_once('/') {
            links.push(NpcDialogLinkState {
                text: text.trim().to_string(),
                target: normalize_crystal_npc_label(target.trim()),
            });
        }
        remainder = &after_start[end + 1..];
    }

    links
}

pub(super) fn strip_crystal_npc_links(line: &str) -> String {
    let mut output = String::new();
    let mut remainder = line;

    while let Some(start) = remainder.find('<') {
        output.push_str(&remainder[..start]);
        let after_start = &remainder[start + 1..];
        let Some(end) = after_start.find('>') else {
            output.push_str(&remainder[start..]);
            return output;
        };
        remainder = &after_start[end + 1..];
    }

    output.push_str(remainder);
    output
}

pub(super) fn normalize_crystal_npc_label(label: &str) -> String {
    let trimmed = label.trim();
    if trimmed.starts_with('@') {
        trimmed.to_string()
    } else {
        format!("@{trimmed}")
    }
}

pub(super) fn parse_crystal_label_target(target: &str) -> (String, Vec<String>) {
    let trimmed = target.trim();
    let Some(open_paren) = trimmed.find('(') else {
        return (normalize_crystal_npc_label(trimmed), Vec::new());
    };
    let Some(close_paren) = trimmed.rfind(')') else {
        return (normalize_crystal_npc_label(trimmed), Vec::new());
    };
    if close_paren <= open_paren {
        return (normalize_crystal_npc_label(trimmed), Vec::new());
    }

    let label = normalize_crystal_npc_label(&trimmed[..open_paren]);
    let args = trimmed[open_paren + 1..close_paren]
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect();
    (label, args)
}

pub(super) fn replace_ignore_ascii_case(input: &str, needle: &str, replacement: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;
    while cursor < input.len() {
        if input[cursor..]
            .get(..needle.len())
            .is_some_and(|value| value.eq_ignore_ascii_case(needle))
        {
            output.push_str(replacement);
            cursor += needle.len();
        } else {
            let ch = input[cursor..].chars().next().expect("cursor is in bounds");
            output.push(ch);
            cursor += ch.len_utf8();
        }
    }
    output
}

pub(super) fn parse_crystal_npc_function_token(token: &str) -> Option<(String, Vec<i32>)> {
    let open = token.find('(')?;
    let close = token.rfind(')')?;
    if close <= open {
        return None;
    }
    let name = token[..open].trim().to_ascii_uppercase();
    let args = token[open + 1..close]
        .split(',')
        .filter_map(|value| value.trim().parse::<i32>().ok())
        .collect();
    Some((name, args))
}

pub(super) fn crystal_npc_quest_stage_value(stage: QuestStage) -> i32 {
    match stage {
        QuestStage::Available => 0,
        QuestStage::InProgress => 1,
        QuestStage::ReadyToTurnIn => 2,
        QuestStage::Completed => 3,
    }
}

pub(super) fn parse_crystal_flag_index(token: &str) -> Option<u32> {
    token
        .trim()
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .and_then(|value| value.parse::<u32>().ok())
}

pub(super) fn parse_crystal_mir_class(value: &str) -> Option<MirClass> {
    match value.trim().to_ascii_lowercase().as_str() {
        "warrior" => Some(MirClass::Warrior),
        "wizard" => Some(MirClass::Wizard),
        "taoist" => Some(MirClass::Taoist),
        "assassin" => Some(MirClass::Assassin),
        "archer" => Some(MirClass::Archer),
        _ => None,
    }
}

pub(super) fn parse_crystal_mir_gender(value: &str) -> Option<MirGender> {
    match value.trim().to_ascii_lowercase().as_str() {
        "male" | "man" => Some(MirGender::Male),
        "female" | "woman" => Some(MirGender::Female),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CrystalNpcLoadValueAction {
    pub(super) variable: String,
    pub(super) file_name: String,
    pub(super) group: String,
    pub(super) key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CrystalNpcSaveValueAction {
    pub(super) file_name: String,
    pub(super) group: String,
    pub(super) key: String,
    pub(super) value: String,
}

pub(super) fn parse_crystal_npc_load_value(line: &str) -> Option<CrystalNpcLoadValueAction> {
    let quotes = collect_quoted_segments(line);
    let parts: Vec<&str> = line.split_whitespace().collect();
    let [command, variable, ..] = parts.as_slice() else {
        return None;
    };
    if !command.eq_ignore_ascii_case("LOADVALUE") {
        return None;
    }
    let file_name = quotes.first()?.trim().to_string();
    let group = parts.get(parts.len().checked_sub(2)?).copied()?.to_string();
    let key = parts.last()?.to_string();

    Some(CrystalNpcLoadValueAction {
        variable: (*variable).to_string(),
        file_name,
        group,
        key,
    })
}

pub(super) fn parse_crystal_npc_save_value(line: &str) -> Option<CrystalNpcSaveValueAction> {
    let quotes = collect_quoted_segments(line);
    let parts: Vec<&str> = line.split_whitespace().collect();
    let [command, ..] = parts.as_slice() else {
        return None;
    };
    if !command.eq_ignore_ascii_case("SAVEVALUE") {
        return None;
    }

    let file_name = quotes.first()?.trim().to_string();
    let value = quotes
        .get(1)
        .map(|value| value.trim().to_string())
        .or_else(|| parts.last().map(|value| (*value).to_string()))?;
    let parts_without_value = if quotes.len() > 1 {
        line.replace(&format!("\"{}\"", quotes[1]), "")
    } else {
        line.to_string()
    };
    let trimmed_parts: Vec<&str> = parts_without_value.split_whitespace().collect();
    let group = trimmed_parts
        .get(trimmed_parts.len().checked_sub(2)?)
        .copied()?
        .to_string();
    let key = trimmed_parts.last()?.to_string();

    Some(CrystalNpcSaveValueAction {
        file_name,
        group,
        key,
        value,
    })
}

pub(super) fn collect_quoted_segments(line: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut remainder = line;

    while let Some(start) = remainder.find('"') {
        let after_start = &remainder[start + 1..];
        let Some(end) = after_start.find('"') else {
            break;
        };
        segments.push(after_start[..end].to_string());
        remainder = &after_start[end + 1..];
    }

    segments
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CrystalLocalTimeSnapshot {
    pub(super) day_of_week: &'static str,
    pub(super) hour: u16,
    pub(super) minute: u16,
}

#[cfg(windows)]
pub(super) fn crystal_local_time_snapshot() -> Option<CrystalLocalTimeSnapshot> {
    let mut system_time = WindowsSystemTime {
        year: 0,
        month: 0,
        day_of_week: 0,
        day: 0,
        hour: 0,
        minute: 0,
        second: 0,
        milliseconds: 0,
    };
    unsafe {
        GetLocalTime(&mut system_time);
    }

    let day_of_week = match system_time.day_of_week {
        0 => "SUNDAY",
        1 => "MONDAY",
        2 => "TUESDAY",
        3 => "WEDNESDAY",
        4 => "THURSDAY",
        5 => "FRIDAY",
        6 => "SATURDAY",
        _ => return None,
    };

    Some(CrystalLocalTimeSnapshot {
        day_of_week,
        hour: system_time.hour,
        minute: system_time.minute,
    })
}

#[cfg(unix)]
pub(super) fn crystal_local_time_snapshot() -> Option<CrystalLocalTimeSnapshot> {
    let mut now: libc::time_t = 0;
    unsafe {
        libc::time(&mut now);
    }

    let mut local_time = std::mem::MaybeUninit::<libc::tm>::uninit();
    let local_time = unsafe {
        let result = libc::localtime_r(&now, local_time.as_mut_ptr());
        if result.is_null() {
            return None;
        }
        local_time.assume_init()
    };

    let day_of_week = match local_time.tm_wday {
        0 => "SUNDAY",
        1 => "MONDAY",
        2 => "TUESDAY",
        3 => "WEDNESDAY",
        4 => "THURSDAY",
        5 => "FRIDAY",
        6 => "SATURDAY",
        _ => return None,
    };

    Some(CrystalLocalTimeSnapshot {
        day_of_week,
        hour: u16::try_from(local_time.tm_hour).ok()?,
        minute: u16::try_from(local_time.tm_min).ok()?,
    })
}

#[cfg(not(any(windows, unix)))]
pub(super) fn crystal_local_time_snapshot() -> Option<CrystalLocalTimeSnapshot> {
    None
}

pub(super) fn handle_npc_interaction(
    world: &mut World,
    context: NpcInteractionContext,
    mut packets: Vec<ServerPacket>,
) -> Vec<ServerPacket> {
    if let Some(script_key) = context.script_key.as_deref() {
        if let Some(result) = run_crystal_npc_script(world, &context, script_key, "@Main") {
            packets.extend(result.packets);
            if let Some(mut dialog) = result.dialog {
                append_crystal_quest_links(world, &context, &mut dialog);
                let footer = dialog.footer.clone();
                set_dialog(world, dialog);
                packets.push(ServerPacket::ObjectChat {
                    object_id: context.object_id,
                    text: footer,
                    chat_type: ChatType::Hint,
                });
            } else if let Some(dialog) = crystal_quest_link_dialog(world, &context) {
                let footer = dialog.footer.clone();
                set_dialog(world, dialog);
                packets.push(ServerPacket::ObjectChat {
                    object_id: context.object_id,
                    text: footer,
                    chat_type: ChatType::Hint,
                });
            } else {
                dismiss_dialog(world);
            }
            return packets;
        }
    }

    if let Some(dialog) = crystal_quest_link_dialog(world, &context) {
        let footer = dialog.footer.clone();
        set_dialog(world, dialog);
        packets.push(ServerPacket::ObjectChat {
            object_id: context.object_id,
            text: footer,
            chat_type: ChatType::Hint,
        });
        return packets;
    }

    if npc_script_for_object_id(context.object_id).is_none() {
        return packets;
    }

    let Some(dialog) = resolve_npc_quest_dialog(world, &context) else {
        return packets;
    };

    set_dialog(
        world,
        ActiveNpcDialogState {
            npc_object_id: context.object_id,
            npc_name: context.name,
            npc_name_key: context.name_key,
            stage: Some(dialog.stage_before_action),
            current: dialog.current,
            required: dialog.required,
            title: dialog.title,
            body: dialog.body,
            footer: dialog.footer,
            links: Vec::new(),
            input: None,
        },
    );

    packets.push(ServerPacket::ObjectChat {
        object_id: context.object_id,
        text: dialog.object_chat,
        chat_type: ChatType::Hint,
    });
    packets
}

fn append_crystal_quest_links(
    world: &World,
    context: &NpcInteractionContext,
    dialog: &mut ActiveNpcDialogState,
) {
    let mut quest_links = quest_dialog_links_for_npc(world, context.object_id, &context.quest_ids);
    if quest_links.is_empty() {
        return;
    }
    dialog.links.append(&mut quest_links);
    dialog
        .links
        .sort_by(|left, right| left.text.cmp(&right.text));
    dialog
        .links
        .dedup_by(|left, right| left.target == right.target);
    if dialog.footer.is_empty() || dialog.footer == context.name {
        dialog.footer = dialog
            .links
            .first()
            .map(|link| link.text.clone())
            .unwrap_or_else(|| context.name.clone());
    }
}

fn crystal_quest_link_dialog(
    world: &World,
    context: &NpcInteractionContext,
) -> Option<ActiveNpcDialogState> {
    let mut links = quest_dialog_links_for_npc(world, context.object_id, &context.quest_ids);
    if links.is_empty() {
        return None;
    }
    links.push(NpcDialogLinkState {
        text: "Exit".to_string(),
        target: "@Exit".to_string(),
    });
    let footer = links
        .first()
        .map(|link| link.text.clone())
        .unwrap_or_else(|| context.name.clone());
    Some(ActiveNpcDialogState {
        npc_object_id: context.object_id,
        npc_name: context.name.clone(),
        npc_name_key: context.name_key.clone(),
        stage: None,
        current: 0,
        required: 1,
        title: context.name.clone(),
        body: vec!["How can I help you?".to_string()],
        footer,
        links,
        input: None,
    })
}

fn crystal_npc_info_for_loaded_object_id(object_id: u32) -> Option<CrystalNpcInfoTemplate> {
    crystal_npc_info_manifest()
        .npcs
        .into_iter()
        .find(|npc| npc.loaded_object_id == Some(object_id))
}

#[derive(Debug, Clone, Default)]
pub(super) struct CrystalNpcRunResult {
    pub(super) dialog: Option<ActiveNpcDialogState>,
    pub(super) packets: Vec<ServerPacket>,
}

pub(super) fn run_crystal_npc_script(
    world: &mut World,
    context: &NpcInteractionContext,
    script_key: &str,
    start_label: &str,
) -> Option<CrystalNpcRunResult> {
    let script = crystal_npc_script_by_key(script_key)?;
    run_crystal_npc_script_impl(world, context, &script, start_label)
}

pub(super) fn run_crystal_npc_script_impl(
    world: &mut World,
    context: &NpcInteractionContext,
    script: &CrystalNpcScript,
    start_label: &str,
) -> Option<CrystalNpcRunResult> {
    let mut label = start_label.to_string();
    let mut packets = Vec::new();
    let context_state = CrystalNpcContextState {
        npc_position: context.position.clone(),
    };
    let mut execution_state = CrystalNpcExecutionState {
        script_key: Some(script.script_key.clone()),
        label_args: context.args.clone(),
        input: context.input.clone(),
        ..CrystalNpcExecutionState::default()
    };

    for _ in 0..CRYSTAL_NPC_MAX_SECTION_HOPS {
        let section = crystal_npc_section(&script, &label)?;
        execution_state.section_label = Some(section.label.clone());
        process_crystal_npc_goods_expiry(world);
        let buy_back_items = crystal_npc_buy_back_items_for_script(world, &script.script_key);
        let used_goods_items = crystal_npc_used_goods_for_script(world, &script.script_key);
        if let Some(service_packets) = crystal_npc_service_packets_for_label_with_markets(
            Some(script),
            &section.label,
            &buy_back_items,
            &used_goods_items,
        ) {
            let label_key = crystal_npc_service_label_key(&section.label);
            record_crystal_npc_service_context(
                world,
                context.object_id,
                script,
                &section.label,
                &service_packets,
            );
            if label_key == "STORAGE" {
                packets.extend(crystal_npc_storage_open_packets(world));
            } else {
                packets.extend(service_packets);
            }
            return Some(CrystalNpcRunResult {
                dialog: None,
                packets,
            });
        }
        let outcome =
            execute_crystal_npc_section(world, section, &context_state, &mut execution_state);
        packets.extend(outcome.packets);

        if let Some(next_label) = outcome.goto_label {
            let (next_normalized_label, next_label_args) = parse_crystal_label_target(&next_label);
            label = next_normalized_label;
            execution_state.label_args = next_label_args;
            continue;
        }

        if outcome.close_dialog {
            return Some(CrystalNpcRunResult {
                dialog: None,
                packets,
            });
        }

        if outcome.body.is_empty() && outcome.links.is_empty() {
            if packets.is_empty() {
                return None;
            }
            return Some(CrystalNpcRunResult {
                dialog: None,
                packets,
            });
        }

        return Some(CrystalNpcRunResult {
            dialog: Some(ActiveNpcDialogState {
                npc_object_id: context.object_id,
                npc_name: context.name.clone(),
                npc_name_key: context.name_key.clone(),
                stage: None,
                current: 0,
                required: 0,
                title: context.name.clone(),
                body: outcome.body,
                footer: outcome
                    .links
                    .first()
                    .map(|link| link.text.clone())
                    .unwrap_or_else(|| context.name.clone()),
                links: outcome.links,
                input: None,
            }),
            packets,
        });
    }

    execution_state.section_label = Some(label);
    execution_state.line_number = 0;
    record_crystal_npc_script_diagnostic(
        world,
        &execution_state,
        "GOTO",
        format!("Crystal NPC script exceeded section hop limit of {CRYSTAL_NPC_MAX_SECTION_HOPS}"),
    );
    Some(CrystalNpcRunResult {
        dialog: None,
        packets,
    })
}

#[derive(Debug, Clone, Default)]
pub(super) struct CrystalNpcSectionOutcome {
    body: Vec<String>,
    links: Vec<NpcDialogLinkState>,
    packets: Vec<ServerPacket>,
    goto_label: Option<String>,
    close_dialog: bool,
}

pub(super) fn execute_crystal_npc_section(
    world: &mut World,
    section: &CrystalNpcSection,
    context_state: &CrystalNpcContextState,
    execution_state: &mut CrystalNpcExecutionState,
) -> CrystalNpcSectionOutcome {
    let mut outcome = CrystalNpcSectionOutcome::default();
    let mut mode = CrystalNpcParseMode::None;
    let mut if_conditions = Vec::new();
    let mut if_result = None;

    for (line_index, raw_line) in section.lines.iter().enumerate() {
        execution_state.line_number = section.line_number + line_index;
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            break;
        }

        if line.starts_with('#') {
            mode = match line.to_ascii_uppercase().as_str() {
                "#SAY" => CrystalNpcParseMode::Say,
                "#ACT" => CrystalNpcParseMode::Act,
                "#ELSESAY" => CrystalNpcParseMode::ElseSay,
                "#ELSEACT" => CrystalNpcParseMode::ElseAct,
                "#IF" => CrystalNpcParseMode::If,
                _ => CrystalNpcParseMode::None,
            };
            if mode == CrystalNpcParseMode::If {
                if_conditions.clear();
                if_result = None;
            } else if matches!(
                mode,
                CrystalNpcParseMode::Say
                    | CrystalNpcParseMode::Act
                    | CrystalNpcParseMode::ElseSay
                    | CrystalNpcParseMode::ElseAct
            ) && if_result.is_none()
                && !if_conditions.is_empty()
            {
                if_result = Some(evaluate_crystal_npc_conditions(
                    world,
                    &if_conditions,
                    context_state,
                ));
            }
            continue;
        }

        if matches!(mode, CrystalNpcParseMode::If) {
            let resolved = resolve_crystal_npc_runtime_tokens(world, line, execution_state);
            record_unknown_crystal_npc_condition_command(world, &resolved, execution_state);
            if_conditions.push(resolved);
            continue;
        }

        let should_execute = match mode {
            CrystalNpcParseMode::None => true,
            CrystalNpcParseMode::If => false,
            CrystalNpcParseMode::Say | CrystalNpcParseMode::Act => if_result.unwrap_or(true),
            CrystalNpcParseMode::ElseSay | CrystalNpcParseMode::ElseAct => {
                if_result.is_some_and(|result| !result)
            }
        };

        if !should_execute {
            continue;
        }

        if matches!(
            mode,
            CrystalNpcParseMode::Act | CrystalNpcParseMode::ElseAct
        ) {
            let control = execute_crystal_npc_action_line(
                world,
                &resolve_crystal_npc_runtime_tokens(world, line, execution_state),
                &mut outcome.packets,
                execution_state,
            );
            match control {
                CrystalNpcActionControl::Continue => {}
                CrystalNpcActionControl::Goto(label) => {
                    outcome.goto_label = Some(label);
                    break;
                }
                CrystalNpcActionControl::Break => break,
                CrystalNpcActionControl::Close => {
                    outcome.close_dialog = true;
                    break;
                }
            }
            continue;
        }

        if matches!(
            mode,
            CrystalNpcParseMode::Say | CrystalNpcParseMode::ElseSay
        ) || (!line.starts_with('#') && matches!(mode, CrystalNpcParseMode::None))
        {
            let resolved_line = resolve_crystal_npc_runtime_tokens(world, line, execution_state);
            outcome
                .links
                .extend(parse_crystal_npc_links(&resolved_line));
            let text = strip_crystal_npc_links(&resolved_line);
            if !text.trim().is_empty() {
                outcome.body.push(text.trim().to_string());
            }
        }
    }

    outcome
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CrystalNpcParseMode {
    None,
    If,
    Say,
    Act,
    ElseSay,
    ElseAct,
}

pub(super) fn resolve_crystal_npc_runtime_tokens(
    world: &World,
    line: &str,
    execution_state: &CrystalNpcExecutionState,
) -> String {
    let mut resolved = line.to_string();

    for (index, arg) in execution_state.label_args.iter().enumerate() {
        resolved = resolved.replace(&format!("%ARG({index})"), arg);
    }
    if let Some(input) = execution_state.input.as_deref() {
        resolved = replace_ignore_ascii_case(&resolved, "%INPUTSTR", input);
    }

    let resolved = replace_crystal_npc_output_tokens(world, &resolved);
    let resolved = replace_crystal_npc_named_tokens(world, &resolved);
    replace_crystal_npc_variable_tokens(world, &resolved)
}

pub(super) fn replace_crystal_npc_variable_tokens(world: &World, input: &str) -> String {
    let bytes = input.as_bytes();
    let mut index = 0;
    let mut output = String::with_capacity(input.len());

    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && bytes[index + 1].is_ascii_alphabetic()
            && bytes[index + 2].is_ascii_digit()
        {
            let key = &input[index + 1..index + 3];
            if let Some(value) = crystal_npc_variable_value(world, key) {
                output.push_str(&value);
                index += 3;
                continue;
            }
        }

        output.push(bytes[index] as char);
        index += 1;
    }

    output
}

pub(super) fn replace_crystal_npc_output_tokens(world: &World, input: &str) -> String {
    let bytes = input.as_bytes();
    let mut index = 0;
    let mut output = String::with_capacity(input.len());

    while index < bytes.len() {
        if index + 9 < bytes.len()
            && input[index..]
                .get(..9)
                .is_some_and(|head| head.eq_ignore_ascii_case("<$OUTPUT("))
        {
            let remainder = &input[index + 9..];
            if let Some(close_index) = remainder.find(")>") {
                let key = remainder[..close_index].trim();
                if let Some(value) = crystal_npc_variable_value(world, key) {
                    output.push_str(&value);
                    index += 9 + close_index + 2;
                    continue;
                }
            }
        }

        output.push(bytes[index] as char);
        index += 1;
    }

    output
}

pub(super) fn replace_crystal_npc_named_tokens(world: &World, input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut remainder = input;

    while let Some(start) = remainder.find("<$") {
        output.push_str(&remainder[..start]);
        let after_start = &remainder[start + 2..];
        let Some(end) = after_start.find('>') else {
            output.push_str(&remainder[start..]);
            return output;
        };
        let token = &after_start[..end];
        if let Some(value) = crystal_npc_named_token_value(world, token) {
            output.push_str(&value);
        } else {
            output.push_str("<$");
            output.push_str(token);
            output.push('>');
        }
        remainder = &after_start[end + 1..];
    }

    output.push_str(remainder);
    output
}

pub(super) fn crystal_npc_named_token_value(world: &World, token: &str) -> Option<String> {
    let trimmed = token.trim();
    let upper = trimmed.to_ascii_uppercase();
    match upper.as_str() {
        "USERNAME" => return Some(stage5_player_name(world)),
        "DATE" => return Some(now_unix_ms().to_string()),
        "GUILDGTRENTALDAYSLEFT" => {
            return Some(
                world
                    .resource::<Stage5SystemsResource>()
                    .stage5_systems
                    .guild_territory
                    .rental_days_left
                    .to_string(),
            )
        }
        "GUILDEXTENDFEE" => return Some("1000000".to_string()),
        _ => {}
    }

    let (name, args) = parse_crystal_npc_function_token(trimmed)?;
    match name.as_str() {
        "CONQUESTOWNER" => Some(crystal_npc_conquest_owner_name(world)),
        "CONQUESTRATE" => Some(
            world
                .resource::<Stage5SystemsResource>()
                .stage5_systems
                .conquest
                .tax_rate_percent
                .to_string(),
        ),
        "CONQUESTGOLD" => Some(
            world
                .resource::<Stage5SystemsResource>()
                .stage5_systems
                .conquest
                .gold
                .to_string(),
        ),
        "CONQUESTSCHEDULE" => Some("Not scheduled".to_string()),
        "CONQUESTGUARD" => Some(crystal_npc_conquest_asset_label(
            world,
            ConquestAssetKind::Guard,
            args.get(1).copied().unwrap_or(1),
        )),
        "CONQUESTWALL" => Some(crystal_npc_conquest_asset_label(
            world,
            ConquestAssetKind::Wall,
            args.get(1).copied().unwrap_or(1),
        )),
        "CONQUESTGATE" => Some(crystal_npc_conquest_asset_label(
            world,
            ConquestAssetKind::Gate,
            args.get(1).copied().unwrap_or(1),
        )),
        _ => None,
    }
}

pub(super) fn crystal_npc_variable_value(world: &World, key: &str) -> Option<String> {
    let normalized_key = key.trim().trim_start_matches('%');
    if !is_crystal_npc_variable_key(normalized_key) {
        return None;
    }

    world
        .resource::<NpcStateResource>()
        .npc_variables
        .iter()
        .find(|(existing_key, _)| existing_key.eq_ignore_ascii_case(normalized_key))
        .map(|(_, value)| value.clone())
}

pub(super) fn set_crystal_npc_variable_value(world: &mut World, key: &str, value: String) {
    if !is_crystal_npc_variable_key(key) {
        return;
    }

    let mut resources = world.resource_mut::<NpcStateResource>();
    if let Some((_, existing)) = resources
        .npc_variables
        .iter_mut()
        .find(|(existing_key, _)| existing_key.eq_ignore_ascii_case(key))
    {
        *existing = value;
        return;
    }

    resources.npc_variables.push((key.to_string(), value));
}

pub(super) fn evaluate_crystal_npc_conditions(
    world: &mut World,
    conditions: &[String],
    context_state: &CrystalNpcContextState,
) -> bool {
    !conditions.is_empty()
        && conditions
            .iter()
            .all(|condition| evaluate_crystal_npc_condition(world, condition, context_state))
}

pub(super) fn evaluate_crystal_npc_condition(
    world: &mut World,
    condition: &str,
    context_state: &CrystalNpcContextState,
) -> bool {
    let parts: Vec<&str> = condition.split_whitespace().collect();
    let Some(command) = parts.first().map(|value| value.to_ascii_uppercase()) else {
        return false;
    };

    match command.as_str() {
        "ISADMIN" => crystal_npc_is_admin(world),
        "DAYOFWEEK" => crystal_npc_check_day_of_week(&parts[1..]),
        "HOUR" => crystal_npc_check_hour(&parts[1..]),
        "MIN" => crystal_npc_check_minute(&parts[1..]),
        "LEVEL" => crystal_npc_compare_numeric(world, player_level(world), &parts[1..]),
        "CHECKGOLD" => crystal_npc_compare_numeric(
            world,
            i32::try_from(world.resource::<PlayerRuntimeResource>().gold).unwrap_or(i32::MAX),
            &parts[1..],
        ),
        "CHECKPKPOINT" => crystal_npc_compare_numeric(world, 0, &parts[1..]),
        "CHECKQUEST" => crystal_npc_check_quest(world, &parts[1..]),
        "CHECKITEM" => crystal_npc_check_item(world, &parts[1..]),
        "CHECK" => crystal_npc_check_flag(world, &parts[1..]),
        "CHECKCALC" => crystal_npc_check_calc(world, &parts[1..]),
        "CHECKCLASS" => crystal_npc_check_class(world, &parts[1..]),
        "CHECKGENDER" | "GENDER" => crystal_npc_check_gender(world, &parts[1..]),
        "PETCOUNT" => crystal_npc_check_pet_count(world, &parts[1..]),
        "PETLEVEL" => crystal_npc_check_pet_level(world, &parts[1..]),
        "CHECKPET" => crystal_npc_check_pet(world, &parts[1..]),
        "RANDOM" => crystal_npc_check_random(world, &parts[1..]),
        "CHECKMON" => crystal_npc_check_mon(world, &parts[1..]),
        "CHECKEXACTMON" => crystal_npc_check_exact_mon(world, &parts[1..]),
        "CHECKMAP" => crystal_npc_check_map(world, &parts[1..]),
        "CHECKMAPLIGHT" => crystal_npc_check_map_light(&parts[1..]),
        "CHECKRANGE" => crystal_npc_check_range(world, &parts[1..]),
        "CHECKHUM" => crystal_npc_check_hum(world, &parts[1..]),
        "GROUPLEADER" => crystal_npc_group_leader(world),
        "GROUPCOUNT" => crystal_npc_group_count(world, &parts[1..]),
        "GROUPCHECKNEARBY" => crystal_npc_group_check_nearby(world, context_state),
        "CHECKCONQUEST" => crystal_npc_check_conquest(world, &parts[1..]),
        "CONQUESTOWNER" => crystal_npc_condition_conquest_owner(world, &parts[1..]),
        "CHECKPERMISSION" => crystal_npc_check_permission(world, &parts[1..]),
        "AFFORDGUARD" => crystal_npc_afford_conquest_asset(world, ConquestAssetKind::Guard),
        "AFFORDWALL" => crystal_npc_afford_conquest_asset(world, ConquestAssetKind::Wall),
        "AFFORDGATE" => crystal_npc_afford_conquest_asset(world, ConquestAssetKind::Gate),
        "HASGT" => crystal_npc_has_gt(world),
        "INGUILD" => crystal_npc_in_guild(world, &parts[1..]),
        "CHECKBUFF" => crystal_npc_check_buff(world, &parts[1..]),
        "HASBAGSPACE" => crystal_npc_has_bag_space(world, &parts[1..]),
        _ => false,
    }
}

pub(super) fn record_unknown_crystal_npc_condition_command(
    world: &mut World,
    condition: &str,
    execution_state: &CrystalNpcExecutionState,
) {
    let Some(command) = condition
        .split_whitespace()
        .next()
        .map(|value| value.to_ascii_uppercase())
    else {
        return;
    };
    if is_known_crystal_npc_condition_command(&command) {
        return;
    }

    record_crystal_npc_script_diagnostic(
        world,
        execution_state,
        command,
        "Unknown Crystal NPC condition command",
    );
}

pub(super) fn is_known_crystal_npc_condition_command(command: &str) -> bool {
    matches!(
        command,
        "ISADMIN"
            | "DAYOFWEEK"
            | "HOUR"
            | "MIN"
            | "LEVEL"
            | "CHECKGOLD"
            | "CHECKPKPOINT"
            | "CHECKQUEST"
            | "CHECKITEM"
            | "CHECK"
            | "CHECKCALC"
            | "CHECKCLASS"
            | "CHECKGENDER"
            | "GENDER"
            | "PETCOUNT"
            | "PETLEVEL"
            | "CHECKPET"
            | "RANDOM"
            | "CHECKMON"
            | "CHECKEXACTMON"
            | "CHECKMAP"
            | "CHECKMAPLIGHT"
            | "CHECKRANGE"
            | "CHECKHUM"
            | "GROUPLEADER"
            | "GROUPCOUNT"
            | "GROUPCHECKNEARBY"
            | "CHECKCONQUEST"
            | "CONQUESTOWNER"
            | "CHECKPERMISSION"
            | "AFFORDGUARD"
            | "AFFORDWALL"
            | "AFFORDGATE"
            | "HASGT"
            | "INGUILD"
            | "CHECKBUFF"
            | "HASBAGSPACE"
    )
}

pub(super) fn record_crystal_npc_script_diagnostic(
    world: &mut World,
    execution_state: &CrystalNpcExecutionState,
    command: impl Into<String>,
    message: impl Into<String>,
) {
    let diagnostic = CrystalNpcScriptDiagnostic {
        script_key: execution_state
            .script_key
            .clone()
            .unwrap_or_else(|| "<inline>".to_string()),
        label: execution_state
            .section_label
            .clone()
            .unwrap_or_else(|| "<inline>".to_string()),
        line_number: execution_state.line_number,
        command: command.into(),
        message: message.into(),
    };
    world
        .resource_mut::<NpcStateResource>()
        .npc_script_diagnostics
        .push(diagnostic);
}

pub(super) fn crystal_npc_compare_numeric(_world: &World, left: i32, parts: &[&str]) -> bool {
    let [operator, right] = parts else {
        return false;
    };
    let Ok(right) = right.parse::<i32>() else {
        return false;
    };

    match *operator {
        ">" => left > right,
        "<" => left < right,
        ">=" => left >= right,
        "<=" => left <= right,
        "=" | "==" => left == right,
        "<>" | "!=" => left != right,
        _ => false,
    }
}

pub(super) fn crystal_npc_check_day_of_week(parts: &[&str]) -> bool {
    let [expected_day] = parts else {
        return false;
    };
    crystal_local_time_snapshot().is_some_and(|snapshot| {
        snapshot
            .day_of_week
            .eq_ignore_ascii_case(expected_day.trim())
    })
}

pub(super) fn crystal_npc_check_hour(parts: &[&str]) -> bool {
    let [expected_hour] = parts else {
        return false;
    };
    let Ok(expected_hour) = expected_hour.parse::<u16>() else {
        return false;
    };
    crystal_local_time_snapshot().is_some_and(|snapshot| snapshot.hour == expected_hour)
}

pub(super) fn crystal_npc_check_minute(parts: &[&str]) -> bool {
    let [expected_minute] = parts else {
        return false;
    };
    let Ok(expected_minute) = expected_minute.parse::<u16>() else {
        return false;
    };
    crystal_local_time_snapshot().is_some_and(|snapshot| snapshot.minute == expected_minute)
}

pub(super) fn crystal_npc_is_admin(_world: &World) -> bool {
    false
}

pub(super) fn crystal_npc_check_map_light(parts: &[&str]) -> bool {
    let [expected_light] = parts else {
        return false;
    };
    expected_light.trim().eq_ignore_ascii_case("LIGHT")
}

pub(super) fn crystal_npc_has_bag_space(world: &World, parts: &[&str]) -> bool {
    let [operator, required_slots] = parts else {
        return false;
    };
    let Ok(required_slots) = required_slots.parse::<i32>() else {
        return false;
    };

    let free_slots = crystal_npc_free_bag_slots(world);
    match *operator {
        ">" => free_slots > required_slots,
        "<" => free_slots < required_slots,
        ">=" => free_slots >= required_slots,
        "<=" => free_slots <= required_slots,
        "=" | "==" => free_slots == required_slots,
        "<>" | "!=" => free_slots != required_slots,
        _ => false,
    }
}

pub(super) fn crystal_npc_free_bag_slots(world: &World) -> i32 {
    let occupied_slots = world
        .resource::<InventoryResource>()
        .inventory_items
        .iter()
        .filter(|item| matches!(item.container, ItemContainer::Bag1 | ItemContainer::Bag2))
        .count();
    (80_i32 - i32::try_from(occupied_slots).unwrap_or(80)).max(0)
}

pub(super) fn crystal_npc_group_leader(_world: &World) -> bool {
    true
}

pub(super) fn crystal_npc_group_count(world: &World, parts: &[&str]) -> bool {
    crystal_npc_compare_numeric(world, crystal_npc_group_member_count(world), parts)
}

pub(super) fn crystal_npc_group_member_count(world: &World) -> i32 {
    i32::try_from(crystal_npc_group_member_entities(world).len()).unwrap_or(i32::MAX)
}

pub(super) fn crystal_npc_group_check_nearby(
    world: &World,
    context_state: &CrystalNpcContextState,
) -> bool {
    crystal_npc_group_member_entities(world)
        .into_iter()
        .all(|entity| {
            entity_position(world, entity)
                .is_some_and(|position| tile_distance(&position, &context_state.npc_position) <= 9)
        })
}

#[allow(deprecated)]
pub(super) fn crystal_npc_group_member_entities(world: &World) -> Vec<Entity> {
    let mut members = Vec::new();

    if let Some(player) = player_entity(world) {
        members.push(player);
    }

    for object_id in &world.resource::<GroupResource>().group_member_object_ids {
        if let Some(entity) = entity_by_object_id(world, *object_id) {
            if !members.contains(&entity) {
                members.push(entity);
            }
        }
    }

    members
}

pub(super) fn crystal_npc_check_conquest(world: &World, parts: &[&str]) -> bool {
    let [conquest_id] = parts else {
        return false;
    };
    let Ok(conquest_id) = conquest_id.parse::<i32>() else {
        return false;
    };
    world
        .resource::<MapRuntimeResource>()
        .conquest_wars
        .get(&conquest_id)
        .copied()
        .is_some_and(|war_is_on| !war_is_on)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConquestAssetKind {
    Guard,
    Wall,
    Gate,
}

pub(super) fn crystal_npc_condition_conquest_owner(world: &World, _parts: &[&str]) -> bool {
    let stage5 = world.resource::<Stage5SystemsResource>();
    let owner = stage5.stage5_systems.conquest.castle_owner.trim();
    !owner.is_empty()
        && !stage5.stage5_systems.guild.name.trim().is_empty()
        && owner.eq_ignore_ascii_case(&stage5.stage5_systems.guild.name)
}

pub(super) fn crystal_npc_conquest_owner_name(world: &World) -> String {
    let owner = world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .conquest
        .castle_owner
        .trim()
        .to_string();
    if owner.is_empty() {
        "No Owner".to_string()
    } else {
        owner
    }
}

pub(super) fn crystal_npc_check_permission(world: &World, _parts: &[&str]) -> bool {
    let guild = &world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .guild;
    guild.rank.eq_ignore_ascii_case("Guild Chief")
        || guild
            .permissions
            .iter()
            .any(|permission| permission.eq_ignore_ascii_case("conquest"))
}

pub(super) fn crystal_npc_afford_conquest_asset(world: &World, kind: ConquestAssetKind) -> bool {
    let player_runtime = world.resource::<PlayerRuntimeResource>();
    let stage5 = world.resource::<Stage5SystemsResource>();
    let available = player_runtime
        .gold
        .saturating_add(stage5.stage5_systems.conquest.gold);
    available >= crystal_npc_conquest_asset_cost(kind)
}

pub(super) fn crystal_npc_has_gt(world: &World) -> bool {
    world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .guild_territory
        .owned
}

pub(super) fn crystal_npc_in_guild(world: &World, parts: &[&str]) -> bool {
    let guild = &world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .guild;
    if guild.name.is_empty() {
        return false;
    }
    match parts {
        [] => true,
        [name] => guild.name.eq_ignore_ascii_case(name),
        _ => false,
    }
}

pub(super) fn crystal_npc_check_buff(world: &World, parts: &[&str]) -> bool {
    let [buff_key] = parts else {
        return false;
    };
    let key = normalize_stage5_key(buff_key);
    world
        .resource::<BuffResource>()
        .buffs
        .iter()
        .any(|buff| buff.key.eq_ignore_ascii_case(&key))
}

pub(super) fn crystal_npc_conquest_asset_cost(kind: ConquestAssetKind) -> u32 {
    match kind {
        ConquestAssetKind::Guard => 50_000,
        ConquestAssetKind::Wall => 100_000,
        ConquestAssetKind::Gate => 150_000,
    }
}

pub(super) fn crystal_npc_conquest_asset_label(
    world: &World,
    kind: ConquestAssetKind,
    asset_id: i32,
) -> String {
    let Ok(asset_id) = u8::try_from(asset_id.max(0)) else {
        return "Unknown".to_string();
    };
    let conquest = &world
        .resource::<Stage5SystemsResource>()
        .stage5_systems
        .conquest;
    match kind {
        ConquestAssetKind::Guard => {
            if conquest.guards.contains(&asset_id) {
                "Alive".to_string()
            } else {
                "Dead".to_string()
            }
        }
        ConquestAssetKind::Wall => {
            if conquest.walls.contains(&asset_id) {
                "Repaired".to_string()
            } else {
                "Damaged".to_string()
            }
        }
        ConquestAssetKind::Gate => {
            if conquest.open_gates.contains(&asset_id) {
                "Open".to_string()
            } else if conquest.gates.contains(&asset_id) {
                "Closed".to_string()
            } else {
                "Damaged".to_string()
            }
        }
    }
}

pub(super) fn crystal_npc_check_quest(world: &World, parts: &[&str]) -> bool {
    let [quest_id, expected_state] = parts else {
        return false;
    };
    let Ok(quest_id) = quest_id.parse::<i32>() else {
        return false;
    };
    let expected_state = expected_state.trim().to_ascii_uppercase();
    let quest = world
        .resource::<QuestResource>()
        .quests
        .iter()
        .find(|quest| quest.quest_id == quest_id);

    match expected_state.as_str() {
        "ACTIVE" => quest.is_some_and(|quest| quest.stage != QuestStage::Completed),
        "COMPLETE" => quest.is_some_and(|quest| quest.stage == QuestStage::Completed),
        _ => expected_state
            .parse::<i32>()
            .ok()
            .and_then(|value| {
                quest.map(|quest| crystal_npc_quest_stage_value(quest.stage) == value)
            })
            .unwrap_or(false),
    }
}

pub(super) fn crystal_npc_check_item(world: &World, parts: &[&str]) -> bool {
    let (item_name, quantity) = match parts {
        [item_name] => (*item_name, 1),
        [item_name, quantity] => {
            let Ok(quantity) = quantity.parse::<u32>() else {
                return false;
            };
            (*item_name, quantity)
        }
        _ => return false,
    };
    let item_key = normalize_crystal_item_key(item_name);

    world
        .resource::<InventoryResource>()
        .inventory_items
        .iter()
        .filter(|item| item.key.eq_ignore_ascii_case(&item_key))
        .map(|item| item.quantity)
        .sum::<u32>()
        >= quantity
}

pub(super) fn crystal_npc_check_flag(world: &World, parts: &[&str]) -> bool {
    let [flag_token, expected_value] = parts else {
        return false;
    };
    let Some(flag_index) = parse_crystal_flag_index(flag_token) else {
        return false;
    };
    let expected = match *expected_value {
        "0" => false,
        "1" => true,
        _ => return false,
    };
    crystal_npc_flag_value(world, flag_index) == expected
}

pub(super) fn crystal_npc_check_calc(world: &World, parts: &[&str]) -> bool {
    let [left, operator, right] = parts else {
        return false;
    };
    let left = crystal_npc_condition_operand_value(world, left);
    let right = crystal_npc_condition_operand_value(world, right);

    if let (Ok(left), Ok(right)) = (left.parse::<i32>(), right.parse::<i32>()) {
        return match *operator {
            ">" => left > right,
            "<" => left < right,
            ">=" => left >= right,
            "<=" => left <= right,
            "=" | "==" => left == right,
            "<>" | "!=" => left != right,
            _ => false,
        };
    }

    match *operator {
        "=" | "==" => left.eq_ignore_ascii_case(&right),
        "<>" | "!=" => !left.eq_ignore_ascii_case(&right),
        _ => false,
    }
}

pub(super) fn crystal_npc_condition_operand_value(world: &World, operand: &str) -> String {
    let with_output = replace_crystal_npc_output_tokens(world, operand);
    replace_crystal_npc_variable_tokens(world, &with_output)
}

pub(super) fn crystal_npc_check_class(world: &World, parts: &[&str]) -> bool {
    let [class_name] = parts else {
        return false;
    };
    let Some(expected) = parse_crystal_mir_class(class_name) else {
        return false;
    };
    world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()
        .is_some_and(|character| character.class == expected)
}

pub(super) fn crystal_npc_check_gender(world: &World, parts: &[&str]) -> bool {
    let [gender_name] = parts else {
        return false;
    };
    let Some(expected) = parse_crystal_mir_gender(gender_name) else {
        return false;
    };
    world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()
        .is_some_and(|character| character.gender == expected)
}

pub(super) fn crystal_npc_check_pet_count(world: &World, parts: &[&str]) -> bool {
    let count = crystal_npc_pet_entities(world).len();
    crystal_npc_compare_numeric(world, i32::try_from(count).unwrap_or(i32::MAX), parts)
}

pub(super) fn crystal_npc_check_pet_level(world: &World, parts: &[&str]) -> bool {
    let [operator, level] = parts else {
        return false;
    };
    let Ok(level) = level.parse::<i32>() else {
        return false;
    };

    let pets = crystal_npc_pet_entities(world);
    !pets.is_empty()
        && pets.into_iter().all(|entity| {
            let pet_level = crystal_npc_pet_level_value(world, entity);
            match *operator {
                ">" => pet_level > level,
                "<" => pet_level < level,
                ">=" => pet_level >= level,
                "<=" => pet_level <= level,
                "=" | "==" => pet_level == level,
                "<>" | "!=" => pet_level != level,
                _ => false,
            }
        })
}

pub(super) fn crystal_npc_check_pet(world: &World, parts: &[&str]) -> bool {
    let [pet_name] = parts else {
        return false;
    };
    crystal_npc_pet_entities(world).into_iter().any(|entity| {
        entity_name(world, entity).is_some_and(|name| name.eq_ignore_ascii_case(pet_name))
    })
}

pub(super) fn crystal_npc_check_random(world: &mut World, parts: &[&str]) -> bool {
    let [upper_bound] = parts else {
        return false;
    };
    let Ok(upper_bound) = upper_bound.parse::<u32>() else {
        return false;
    };

    world
        .resource_mut::<CrystalNpcRandomState>()
        .next_index(upper_bound)
        .is_some_and(|value| value == 0)
}

pub(super) fn crystal_npc_check_mon(world: &World, parts: &[&str]) -> bool {
    let [operator, count, map_file_name, instance] = parts else {
        return false;
    };
    let Ok(count) = count.parse::<i32>() else {
        return false;
    };
    let Ok(instance) = instance.parse::<i32>() else {
        return false;
    };

    let current_map = world
        .resource::<MapRuntimeResource>()
        .current_map
        .file_name
        .clone();
    if instance != 1
        || normalize_map_file_name(&current_map) != normalize_map_file_name(map_file_name)
    {
        return false;
    }

    let current_count = crystal_npc_current_map_monster_count(world);
    match *operator {
        ">" => current_count > count,
        "<" => current_count < count,
        ">=" => current_count >= count,
        "<=" => current_count <= count,
        "=" | "==" => current_count == count,
        "<>" | "!=" => current_count != count,
        _ => false,
    }
}

pub(super) fn crystal_npc_check_exact_mon(world: &World, parts: &[&str]) -> bool {
    let [monster_name, operator, count, map_file_name, instance] = parts else {
        return false;
    };
    let Ok(count) = count.parse::<i32>() else {
        return false;
    };
    let Ok(instance) = instance.parse::<i32>() else {
        return false;
    };
    if crystal_dynamic_monster_template(monster_name).is_none() {
        return false;
    }

    let current_map = world
        .resource::<MapRuntimeResource>()
        .current_map
        .file_name
        .clone();
    if instance != 1
        || normalize_map_file_name(&current_map) != normalize_map_file_name(map_file_name)
    {
        return false;
    }

    let current_count = crystal_npc_current_map_monster_count_by_name(world, monster_name);
    match *operator {
        ">" => current_count > count,
        "<" => current_count < count,
        ">=" => current_count >= count,
        "<=" => current_count <= count,
        "=" | "==" => current_count == count,
        "<>" | "!=" => current_count != count,
        _ => false,
    }
}

pub(super) fn crystal_npc_check_map(world: &World, parts: &[&str]) -> bool {
    let [map_file_name] = parts else {
        return false;
    };
    let current_map = world
        .resource::<MapRuntimeResource>()
        .current_map
        .file_name
        .clone();
    normalize_map_file_name(&current_map) == normalize_map_file_name(map_file_name)
}

pub(super) fn crystal_npc_check_range(world: &World, parts: &[&str]) -> bool {
    let [x, y, radius] = parts else {
        return false;
    };
    let (Ok(x), Ok(y), Ok(radius)) = (x.parse::<i32>(), y.parse::<i32>(), radius.parse::<i32>())
    else {
        return false;
    };
    let Some(player) = player_entity(world) else {
        return false;
    };
    let Some(position) = entity_position(world, player) else {
        return false;
    };
    (position.x - x).abs() <= radius && (position.y - y).abs() <= radius
}

pub(super) fn crystal_npc_check_hum(world: &World, parts: &[&str]) -> bool {
    match parts {
        [operator, count, map_file_name] => {
            if !crystal_npc_check_map(world, &[*map_file_name]) {
                return false;
            }
            crystal_npc_compare_numeric(world, 1, &[*operator, *count])
        }
        [count, map_file_name] => {
            if !crystal_npc_check_map(world, &[*map_file_name]) {
                return false;
            }
            count.parse::<i32>().is_ok_and(|expected| expected <= 1)
        }
        _ => false,
    }
}

#[allow(deprecated)]
pub(super) fn crystal_npc_current_map_monster_count(world: &World) -> i32 {
    i32::try_from(
        world
            .iter_entities()
            .filter(|entity| {
                entity.contains::<Monster>()
                    && entity
                        .get::<MonsterAgent>()
                        .is_some_and(|agent| !agent.dead)
            })
            .count(),
    )
    .unwrap_or(i32::MAX)
}

#[allow(deprecated)]
pub(super) fn crystal_npc_current_map_monster_count_by_name(
    world: &World,
    monster_name: &str,
) -> i32 {
    i32::try_from(
        world
            .iter_entities()
            .filter(|entity| {
                entity.contains::<Monster>()
                    && entity
                        .get::<MonsterAgent>()
                        .is_some_and(|agent| !agent.dead)
                    && entity_name(world, entity.id())
                        .is_some_and(|name| name.eq_ignore_ascii_case(monster_name))
            })
            .count(),
    )
    .unwrap_or(i32::MAX)
}

pub(super) fn crystal_npc_flag_value(world: &World, flag_index: u32) -> bool {
    world
        .resource::<NpcStateResource>()
        .npc_flags
        .iter()
        .find(|flag| flag.index == flag_index)
        .map(|flag| flag.value)
        .unwrap_or(false)
}

pub(super) fn set_crystal_npc_flag(world: &mut World, flag_index: u32, value: bool) {
    let mut resources = world.resource_mut::<NpcStateResource>();
    if let Some(flag) = resources
        .npc_flags
        .iter_mut()
        .find(|flag| flag.index == flag_index)
    {
        flag.value = value;
    } else {
        resources.npc_flags.push(NpcFlagState {
            index: flag_index,
            value,
        });
    }
}

pub(super) fn player_level(world: &World) -> i32 {
    world
        .resource::<SessionResource>()
        .selected_character
        .as_ref()
        .map(|character| i32::from(character.level))
        .unwrap_or(1)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CrystalNpcActionControl {
    Continue,
    Goto(String),
    Break,
    Close,
}

pub(super) fn execute_crystal_npc_action_line(
    world: &mut World,
    line: &str,
    packets: &mut Vec<ServerPacket>,
    execution_state: &mut CrystalNpcExecutionState,
) -> CrystalNpcActionControl {
    if let Some(label) = parse_crystal_goto_target(line) {
        return CrystalNpcActionControl::Goto(label);
    }

    let parts: Vec<&str> = line.split_whitespace().collect();
    let Some(command) = parts.first().map(|value| value.to_ascii_uppercase()) else {
        return CrystalNpcActionControl::Continue;
    };

    match command.as_str() {
        "MOVE" => {
            if let Some(move_packets) = crystal_npc_move_packets(world, &parts[1..]) {
                packets.extend(move_packets);
            }
            CrystalNpcActionControl::Continue
        }
        "TAKEGOLD" => {
            if let Some(amount) = parts.get(1).and_then(|value| value.parse::<u32>().ok()) {
                let mut player_runtime = world.resource_mut::<PlayerRuntimeResource>();
                player_runtime.gold = player_runtime.gold.saturating_sub(amount);
            }
            CrystalNpcActionControl::Continue
        }
        "TAKEITEM" => {
            let _ = crystal_npc_take_item(world, &parts[1..]);
            CrystalNpcActionControl::Continue
        }
        "GIVEITEM" => {
            crystal_npc_give_item(world, &parts[1..]);
            CrystalNpcActionControl::Continue
        }
        "GIVEGOLD" => {
            crystal_npc_give_gold(world, &parts[1..]);
            CrystalNpcActionControl::Continue
        }
        "GIVECITYCURRENCY" => {
            crystal_npc_give_city_currency(world, &parts[1..]);
            CrystalNpcActionControl::Continue
        }
        "TAKECITYCURRENCY" => {
            crystal_npc_take_city_currency(world, &parts[1..]);
            CrystalNpcActionControl::Continue
        }
        "GIVESKILL" => {
            crystal_npc_give_skill(world, &parts[1..]);
            CrystalNpcActionControl::Continue
        }
        "GIVEPET" => {
            crystal_npc_give_pet(world, &parts[1..]);
            CrystalNpcActionControl::Continue
        }
        "REMOVEPET" => {
            crystal_npc_remove_pet(world, &parts[1..]);
            CrystalNpcActionControl::Continue
        }
        "CLEARPETS" => {
            crystal_npc_clear_pets(world);
            CrystalNpcActionControl::Continue
        }
        "PARAM1" => {
            crystal_npc_param1(parts.get(1..).unwrap_or_default(), execution_state);
            CrystalNpcActionControl::Continue
        }
        "PARAM2" => {
            crystal_npc_param2(parts.get(1..).unwrap_or_default(), execution_state);
            CrystalNpcActionControl::Continue
        }
        "PARAM3" => {
            crystal_npc_param3(parts.get(1..).unwrap_or_default(), execution_state);
            CrystalNpcActionControl::Continue
        }
        "MONGEN" => {
            crystal_npc_mongen(world, parts.get(1..).unwrap_or_default(), execution_state);
            CrystalNpcActionControl::Continue
        }
        "MONCLEAR" => {
            crystal_npc_monclear(world, parts.get(1..).unwrap_or_default());
            CrystalNpcActionControl::Continue
        }
        "GROUPTELEPORT" => {
            if let Some(move_packets) = crystal_npc_group_teleport_packets(world, &parts[1..]) {
                packets.extend(move_packets);
            }
            CrystalNpcActionControl::Continue
        }
        "SET" => {
            if let Some((flag_index, true)) = crystal_npc_set_flag(world, &parts[1..]) {
                packets.extend(advance_crystal_quest_flag(
                    world,
                    i32::try_from(flag_index).unwrap_or(i32::MAX),
                ));
            }
            CrystalNpcActionControl::Continue
        }
        "MOV" => {
            crystal_npc_set_variable(world, &parts[1..]);
            CrystalNpcActionControl::Continue
        }
        "GROUPGOTO" => {
            if let Some(target) = parts.get(1) {
                return CrystalNpcActionControl::Goto(normalize_crystal_npc_label(target));
            }
            CrystalNpcActionControl::Continue
        }
        "CALC" => {
            crystal_npc_calc(world, &parts[1..]);
            CrystalNpcActionControl::Continue
        }
        "GIVEEXP" => {
            crystal_npc_give_exp(world, &parts[1..]);
            CrystalNpcActionControl::Continue
        }
        "LOADVALUE" => {
            crystal_npc_load_value(world, line);
            CrystalNpcActionControl::Continue
        }
        "SAVEVALUE" => {
            crystal_npc_save_value(world, line);
            CrystalNpcActionControl::Continue
        }
        "LOCALMESSAGE" | "LINEMESSAGE" => {
            if let Some(packet) = crystal_npc_message_packet(line, ChatType::System) {
                packets.push(packet);
            }
            CrystalNpcActionControl::Continue
        }
        "GLOBALMESSAGE" => {
            if let Some(packet) = crystal_npc_message_packet(line, ChatType::Announcement) {
                packets.push(packet);
            }
            CrystalNpcActionControl::Continue
        }
        "SETCONQUESTRATE" => {
            crystal_npc_set_conquest_rate(world, &parts[1..]);
            CrystalNpcActionControl::Continue
        }
        "CONQUESTGUARD" => {
            crystal_npc_repair_conquest_asset(world, ConquestAssetKind::Guard, &parts[1..]);
            CrystalNpcActionControl::Continue
        }
        "CONQUESTWALL" => {
            crystal_npc_repair_conquest_asset(world, ConquestAssetKind::Wall, &parts[1..]);
            CrystalNpcActionControl::Continue
        }
        "CONQUESTGATE" => {
            crystal_npc_repair_conquest_asset(world, ConquestAssetKind::Gate, &parts[1..]);
            CrystalNpcActionControl::Continue
        }
        "CONQUESTREPAIRALL" => {
            crystal_npc_repair_all_conquest_assets(world);
            CrystalNpcActionControl::Continue
        }
        "OPENGATE" => {
            crystal_npc_set_conquest_gate_open(world, &parts[1..], true);
            CrystalNpcActionControl::Continue
        }
        "CLOSEGATE" => {
            crystal_npc_set_conquest_gate_open(world, &parts[1..], false);
            CrystalNpcActionControl::Continue
        }
        "STARTCONQUEST" => {
            crystal_npc_start_conquest(world, &parts[1..]);
            CrystalNpcActionControl::Continue
        }
        "TAKECONQUESTGOLD" => {
            crystal_npc_take_conquest_gold(world, &parts[1..]);
            CrystalNpcActionControl::Continue
        }
        "BUYGT" => {
            crystal_npc_buy_gt(world);
            CrystalNpcActionControl::Continue
        }
        "EXTENDGT" => {
            crystal_npc_extend_gt(world);
            CrystalNpcActionControl::Continue
        }
        "GTRECALL" => {
            crystal_npc_gt_recall(world, parts.get(1).copied());
            CrystalNpcActionControl::Continue
        }
        "GTALLRECALL" => {
            crystal_npc_gt_recall(world, None);
            CrystalNpcActionControl::Continue
        }
        "TELEPORTGT" => {
            if let Some(move_packets) = crystal_npc_teleport_gt_packets(world) {
                packets.extend(move_packets);
            }
            CrystalNpcActionControl::Continue
        }
        "ADDTOGUILD" => {
            crystal_npc_add_to_guild(world, &parts[1..]);
            CrystalNpcActionControl::Continue
        }
        "ADDNAMELIST" => {
            crystal_npc_name_list(world, &parts[1..], true);
            CrystalNpcActionControl::Continue
        }
        "DELNAMELIST" => {
            crystal_npc_name_list(world, &parts[1..], false);
            CrystalNpcActionControl::Continue
        }
        "CHANGELEVEL" => {
            crystal_npc_change_level(world, &parts[1..]);
            CrystalNpcActionControl::Continue
        }
        "CHANGEHAIR" => {
            crystal_npc_change_hair(world, &parts[1..]);
            CrystalNpcActionControl::Continue
        }
        "GIVEBUFF" => {
            crystal_npc_give_buff(world, &parts[1..]);
            CrystalNpcActionControl::Continue
        }
        "REVIVEHERO" => {
            crystal_npc_revive_hero(world);
            CrystalNpcActionControl::Continue
        }
        "SEALHERO" => {
            crystal_npc_seal_hero(world);
            CrystalNpcActionControl::Continue
        }
        "TIMERECALL" => {
            crystal_npc_record_timed_recall(world, parts.get(1).copied(), true);
            CrystalNpcActionControl::Continue
        }
        "BREAKTIMERECALL" => {
            crystal_npc_record_timed_recall(world, None, false);
            CrystalNpcActionControl::Continue
        }
        "CHECKITEM" | "ELSESAY" => CrystalNpcActionControl::Continue,
        "BREAK" => CrystalNpcActionControl::Break,
        "CLOSE" => CrystalNpcActionControl::Close,
        _ => {
            record_crystal_npc_script_diagnostic(
                world,
                execution_state,
                command,
                "Unknown Crystal NPC action command",
            );
            CrystalNpcActionControl::Continue
        }
    }
}

pub(super) fn crystal_npc_move_packets(
    world: &mut World,
    parts: &[&str],
) -> Option<Vec<ServerPacket>> {
    let [map_file_name, x, y] = parts else {
        return None;
    };
    let Ok(x) = x.parse::<i32>() else {
        return None;
    };
    let Ok(y) = y.parse::<i32>() else {
        return None;
    };

    let current_direction = world.resource::<PlayerRuntimeResource>().player_direction;
    let map_info = crystal_npc_move_map_information(world, map_file_name);
    Some(relocate_player_to_map(
        world,
        map_info,
        Point { x, y },
        current_direction,
        None,
    ))
}

pub(super) fn crystal_npc_move_map_information(
    world: &World,
    map_file_name: &str,
) -> MapInformation {
    let mut info = world.resource::<MapRuntimeResource>().current_map.clone();

    if let Some(map) = crystal_map_respawns_by_file_name(map_file_name) {
        info.map_index = map.map_index;
        info.file_name = map.map_file_name;
        info.title = mir2_game_data::brand::map_title(&map.map_title);
        info.mini_map = map.mini_map;
        info.big_map = map.big_map;
        info.lights = map.light;
        return info;
    }

    info.file_name = map_file_name.to_string();
    info.title = map_file_name.to_string();
    if let Ok(index) = map_file_name.parse::<i32>() {
        info.map_index = index;
    }
    info
}

pub(super) fn crystal_npc_group_teleport_packets(
    world: &mut World,
    parts: &[&str],
) -> Option<Vec<ServerPacket>> {
    let [map_file_name, x, y] = parts else {
        return None;
    };
    let Ok(x) = x.parse::<i32>() else {
        return None;
    };
    let Ok(y) = y.parse::<i32>() else {
        return None;
    };

    let current_direction = world.resource::<PlayerRuntimeResource>().player_direction;
    let map_info = crystal_npc_move_map_information(world, map_file_name);
    let group_members = crystal_npc_remote_group_member_snapshots(world);
    let packets = relocate_player_to_map(
        world,
        map_info.clone(),
        Point { x, y },
        current_direction,
        None,
    );
    crystal_npc_restore_remote_group_members(
        world,
        group_members,
        Point { x, y },
        current_direction,
    );
    Some(packets)
}

#[derive(Clone)]
pub(super) struct CrystalNpcRemoteGroupMemberSnapshot {
    object_id: u32,
    name: DisplayName,
    body: CharacterBody,
}

pub(super) fn crystal_npc_remote_group_member_snapshots(
    world: &World,
) -> Vec<CrystalNpcRemoteGroupMemberSnapshot> {
    let self_player = player_entity(world);
    crystal_npc_group_member_entities(world)
        .into_iter()
        .filter(|entity| Some(*entity) != self_player)
        .filter_map(|entity| {
            let entry = world.entity(entity);
            Some(CrystalNpcRemoteGroupMemberSnapshot {
                object_id: entry.get::<ObjectId>()?.0,
                name: entry.get::<DisplayName>()?.clone(),
                body: *entry.get::<CharacterBody>()?,
            })
        })
        .collect()
}

pub(super) fn crystal_npc_restore_remote_group_members(
    world: &mut World,
    members: Vec<CrystalNpcRemoteGroupMemberSnapshot>,
    destination: Point,
    direction: MirDirection,
) {
    for member in members {
        if let Some(entity) = entity_by_object_id(world, member.object_id) {
            if let Ok(mut entry) = world.get_entity_mut(entity) {
                entry.insert((Position(destination.clone()), Facing(direction)));
            }
            continue;
        }

        world.spawn((
            WorldObject,
            RemotePlayer,
            ObjectId(member.object_id),
            member.name,
            Position(destination.clone()),
            Facing(direction),
            member.body,
        ));
    }
}

pub(super) fn crystal_npc_take_item(world: &mut World, parts: &[&str]) -> bool {
    let (item_name, mut quantity) = match parts {
        [item_name] => (*item_name, 1),
        [item_name, quantity] => {
            let Ok(quantity) = quantity.parse::<u32>() else {
                return false;
            };
            (*item_name, quantity)
        }
        _ => return false,
    };
    let item_key = normalize_crystal_item_key(item_name);
    let mut resources = world.resource_mut::<InventoryResource>();

    for index in (0..resources.inventory_items.len()).rev() {
        if !resources.inventory_items[index]
            .key
            .eq_ignore_ascii_case(&item_key)
        {
            continue;
        }

        let available = resources.inventory_items[index].quantity;
        let consumed = available.min(quantity);
        if consumed == available {
            resources.inventory_items.remove(index);
        } else {
            resources.inventory_items[index].quantity -= consumed;
        }
        quantity -= consumed;
        if quantity == 0 {
            return true;
        }
    }

    false
}

pub(super) fn crystal_npc_give_item(world: &mut World, parts: &[&str]) {
    let (item_name, quantity) = match parts {
        [item_name] => (*item_name, 1),
        [item_name, quantity] => {
            let Ok(quantity) = quantity.parse::<u32>() else {
                return;
            };
            (*item_name, quantity)
        }
        _ => return,
    };
    let item_key = normalize_crystal_item_key(item_name);
    {
        let resources = world.resource::<InventoryResource>();
        if !can_gain_item_quantity(&resources, ItemContainer::Bag1, &item_key, quantity) {
            return;
        }
    }
    add_or_increment_item(
        world,
        ItemContainer::Bag1,
        &item_key,
        &crystal_item_display_name(item_name),
        &crystal_item_description(item_name),
        8,
        quantity,
        1,
    );
}

pub(super) fn crystal_npc_give_gold(world: &mut World, parts: &[&str]) {
    let Some(amount) = parts.first().and_then(|value| value.parse::<u32>().ok()) else {
        return;
    };
    let mut player_runtime = world.resource_mut::<PlayerRuntimeResource>();
    player_runtime.gold = player_runtime.gold.saturating_add(amount);
}

pub(super) fn gain_credit(world: &mut World, amount: u32) -> Option<ServerPacket> {
    if amount == 0 {
        return None;
    }

    let mut player_runtime = world.resource_mut::<PlayerRuntimeResource>();
    let gained = amount.min(u32::MAX.saturating_sub(player_runtime.credit));
    if gained == 0 {
        return None;
    }

    player_runtime.credit += gained;
    Some(ServerPacket::GainedCredit { credit: gained })
}

/// NPC script `GIVECITYCURRENCY <city> <amount>` — credit a per-city reputation
/// currency wallet (net-new, beyond Crystal). `<city>` accepts the wallet key
/// (`feitian`/`bichon`) or the Chinese city name; an unrecognised city is
/// ignored. The new balance reaches the browser via the next world snapshot.
pub(super) fn crystal_npc_give_city_currency(world: &mut World, parts: &[&str]) {
    let [city_token, amount_token, ..] = parts else {
        return;
    };
    let Some(key) = CurrencyKind::from_arg(city_token).city_key() else {
        return;
    };
    let Ok(amount) = amount_token.parse::<u32>() else {
        return;
    };
    world
        .resource_mut::<PlayerRuntimeResource>()
        .gain_city_currency(key, amount);
}

/// NPC script `TAKECITYCURRENCY <city> <amount>` — deduct a per-city currency
/// wallet, saturating at zero (mirrors `TAKEGOLD`).
pub(super) fn crystal_npc_take_city_currency(world: &mut World, parts: &[&str]) {
    let [city_token, amount_token, ..] = parts else {
        return;
    };
    let Some(key) = CurrencyKind::from_arg(city_token).city_key() else {
        return;
    };
    let Ok(amount) = amount_token.parse::<u32>() else {
        return;
    };
    let mut player_runtime = world.resource_mut::<PlayerRuntimeResource>();
    let current = player_runtime.city_currency_balance(key);
    player_runtime.spend_city_currency(key, amount.min(current));
}

pub(super) fn crystal_npc_set_flag(world: &mut World, parts: &[&str]) -> Option<(u32, bool)> {
    let [flag_token, value_token] = parts else {
        return None;
    };
    let Some(flag_index) = parse_crystal_flag_index(flag_token) else {
        return None;
    };
    let value = match *value_token {
        "0" => false,
        "1" => true,
        _ => return None,
    };
    set_crystal_npc_flag(world, flag_index, value);
    Some((flag_index, value))
}

pub(super) fn crystal_npc_set_variable(world: &mut World, parts: &[&str]) {
    let [key, value @ ..] = parts else {
        return;
    };
    if !is_crystal_npc_variable_key(key) {
        return;
    }

    set_crystal_npc_variable_value(world, key, crystal_npc_join_value_parts(value));
}

pub(super) fn crystal_npc_calc(world: &mut World, parts: &[&str]) {
    let [key, operator, right @ ..] = parts else {
        return;
    };
    if !is_crystal_npc_variable_key(key) || right.is_empty() {
        return;
    }

    let left_value = crystal_npc_variable_value(world, key).unwrap_or_else(|| format!("%{key}"));
    let right_value = crystal_npc_join_value_parts(right);
    let result = match (left_value.parse::<i32>(), right_value.parse::<i32>()) {
        (Ok(left), Ok(right)) => match *operator {
            "+" => Some((left + right).to_string()),
            "-" => Some((left - right).to_string()),
            "*" => Some((left * right).to_string()),
            "/" if right != 0 => Some((left / right).to_string()),
            "/" => None,
            _ => None,
        },
        _ => Some(format!("{left_value}{right_value}")),
    };

    if let Some(result) = result {
        set_crystal_npc_variable_value(world, key, result);
    }
}

pub(super) fn crystal_npc_join_value_parts(parts: &[&str]) -> String {
    parts.join(" ").trim_matches('"').to_string()
}

pub(super) fn is_crystal_npc_variable_key(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(
        (chars.next(), chars.next(), chars.next()),
        (Some(first), Some(second), None)
            if first.is_ascii_alphabetic() && second.is_ascii_digit()
    )
}

pub(super) fn crystal_npc_give_skill(world: &mut World, parts: &[&str]) {
    let [spell_name, level @ ..] = parts else {
        return;
    };
    let level = level
        .first()
        .and_then(|value| value.parse::<u8>().ok())
        .unwrap_or(0)
        .min(3);
    let Some(skill) = crystal_skill_state(spell_name, level) else {
        return;
    };

    let mut skills = world.resource_mut::<SkillResource>();
    if let Some(existing) = skills
        .skills
        .iter_mut()
        .find(|known| known.key.eq_ignore_ascii_case(&skill.key))
    {
        existing.level = existing.level.max(skill.level);
        if existing.name.is_empty() {
            existing.name = skill.name;
        }
        if existing.description.is_empty() {
            existing.description = skill.description;
        }
        if existing.cooldown_ticks == 0 {
            existing.cooldown_ticks = skill.cooldown_ticks;
        }
        if existing.delay_ms == 0 {
            existing.delay_ms = skill.delay_ms;
        }
        return;
    }
    skills.skills.push(skill);
}

pub(super) fn crystal_npc_give_pet(world: &mut World, parts: &[&str]) {
    let [monster_name, pet_count @ ..] = parts else {
        return;
    };
    let count = pet_count
        .first()
        .and_then(|value| value.parse::<u8>().ok())
        .unwrap_or(1)
        .clamp(1, 5);
    let pet_level = pet_count
        .get(1)
        .and_then(|value| value.parse::<u8>().ok())
        .unwrap_or(0)
        .min(7);

    let Some(player) = player_entity(world) else {
        return;
    };
    let Some(player_object_id) = current_player_object_id(world) else {
        return;
    };
    let Some(player_position) = entity_position(world, player) else {
        return;
    };
    let Some(player_direction) = entity_facing(world, player) else {
        return;
    };
    let Some(template) = crystal_dynamic_monster_template(monster_name) else {
        return;
    };

    for offset in 0..count {
        let spawn_position = Point {
            x: player_position.x + i32::from(offset % 2),
            y: player_position.y + i32::from(offset / 2),
        };
        let _ = spawn_runtime_monster(
            world,
            &template,
            spawn_position,
            player_direction,
            None,
            Some(SummonedMonster {
                summoner_object_id: player_object_id,
                visible_extra: false,
                expire_tick: None,
                require_summoner_within: None,
                despawn_tick_after_death: None,
                totem_master_object_id: None,
                max_minions: Some(5),
            }),
            Some(false),
            Some(WorldEntityDisposition::Friendly),
            0,
        );
    }

    for entity in crystal_npc_pet_entities(world) {
        let is_new_pet =
            entity_name(world, entity).is_some_and(|name| name.eq_ignore_ascii_case(monster_name));
        if !is_new_pet {
            continue;
        }
        if let Ok(mut entry) = world.get_entity_mut(entity) {
            entry.insert(NpcPetState { pet_level });
        }
    }
}

pub(super) fn crystal_npc_clear_pets(world: &mut World) {
    let pets = crystal_npc_pet_entities(world);
    for entity in pets {
        let _ = world.despawn(entity);
    }
}

pub(super) fn crystal_npc_remove_pet(world: &mut World, parts: &[&str]) {
    let [pet_name] = parts else {
        return;
    };

    let pets_to_remove: Vec<Entity> = crystal_npc_pet_entities(world)
        .into_iter()
        .filter(|entity| {
            entity_name(world, *entity).is_some_and(|name| name.eq_ignore_ascii_case(pet_name))
        })
        .collect();

    for entity in pets_to_remove {
        let _ = world.despawn(entity);
    }
}

pub(super) fn crystal_npc_param1(parts: &[&str], execution_state: &mut CrystalNpcExecutionState) {
    let [map_file_name, instance @ ..] = parts else {
        return;
    };
    execution_state.map_file_name = Some((*map_file_name).to_string());
    execution_state.map_instance = instance
        .first()
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(1);
}

pub(super) fn crystal_npc_param2(parts: &[&str], execution_state: &mut CrystalNpcExecutionState) {
    let Some(x) = parts.first().and_then(|value| value.parse::<i32>().ok()) else {
        return;
    };
    execution_state.x = x;
}

pub(super) fn crystal_npc_param3(parts: &[&str], execution_state: &mut CrystalNpcExecutionState) {
    let Some(y) = parts.first().and_then(|value| value.parse::<i32>().ok()) else {
        return;
    };
    execution_state.y = y;
}

pub(super) fn crystal_npc_mongen(
    world: &mut World,
    parts: &[&str],
    execution_state: &CrystalNpcExecutionState,
) {
    let [monster_name, count @ ..] = parts else {
        return;
    };
    let Some(map_file_name) = execution_state.map_file_name.as_deref() else {
        return;
    };
    if execution_state.x == 0 || execution_state.y == 0 {
        return;
    }

    let count = count
        .first()
        .and_then(|value| value.parse::<u8>().ok())
        .unwrap_or(1);
    let Some(template) = crystal_dynamic_monster_template(monster_name) else {
        return;
    };
    let current_map_file_name = world
        .resource::<MapRuntimeResource>()
        .current_map
        .file_name
        .clone();
    if normalize_map_file_name(&current_map_file_name) != normalize_map_file_name(map_file_name) {
        return;
    }

    for index in 0..count {
        let offset = i32::from(index);
        let spawn_position = Point {
            x: execution_state.x + (offset % 2),
            y: execution_state.y + (offset / 2),
        };
        let _ = spawn_runtime_monster(
            world,
            &template,
            spawn_position,
            MirDirection::Up,
            None,
            None,
            None,
            None,
            0,
        );
    }
}

#[allow(deprecated)]
pub(super) fn crystal_npc_monclear(world: &mut World, parts: &[&str]) {
    let Some(map_file_name) = parts.first() else {
        return;
    };
    let current_map_file_name = world
        .resource::<MapRuntimeResource>()
        .current_map
        .file_name
        .clone();
    if normalize_map_file_name(&current_map_file_name) != normalize_map_file_name(map_file_name) {
        return;
    }

    let monsters: Vec<Entity> = world
        .iter_entities()
        .filter_map(|entity| {
            if entity.contains::<Monster>()
                && entity
                    .get::<MonsterAgent>()
                    .is_some_and(|agent| !agent.dead)
            {
                Some(entity.id())
            } else {
                None
            }
        })
        .collect();

    for entity in monsters {
        if let Ok(mut entry) = world.get_entity_mut(entity) {
            if let Some(mut agent) = entry.get_mut::<MonsterAgent>() {
                agent.dead = true;
            }
            if let Some(mut vitals) = entry.get_mut::<MonsterVitals>() {
                vitals.hp = 0;
            }
        }
    }
}

#[allow(deprecated)]
pub(super) fn crystal_npc_pet_entities(world: &World) -> Vec<Entity> {
    let Some(player_object_id) = current_player_object_id(world) else {
        return Vec::new();
    };

    world
        .iter_entities()
        .filter_map(|entity| {
            let summoned = entity.get::<SummonedMonster>()?;
            let agent = entity.get::<MonsterAgent>()?;
            if summoned.summoner_object_id == player_object_id && !agent.dead {
                Some(entity.id())
            } else {
                None
            }
        })
        .collect()
}

pub(super) fn crystal_npc_pet_level_value(world: &World, entity: Entity) -> i32 {
    world
        .entity(entity)
        .get::<NpcPetState>()
        .map(|pet| i32::from(pet.pet_level))
        .unwrap_or_default()
}

pub(super) fn crystal_npc_message_packet(
    line: &str,
    default_chat_type: ChatType,
) -> Option<ServerPacket> {
    let start = line.find('"')?;
    let end_offset = line[start + 1..].find('"')?;
    let message = line[start + 1..start + 1 + end_offset].to_string();
    let remainder = line[start + 1 + end_offset + 1..].trim();
    let chat_type = match remainder.to_ascii_uppercase().as_str() {
        "HINT" => ChatType::Hint,
        "ANNOUNCEMENT" => ChatType::Announcement,
        "GUILD" => ChatType::Guild,
        "GROUP" => ChatType::Group,
        "SHOUT" => ChatType::Shout,
        "SYSTEM" => ChatType::System,
        _ => default_chat_type,
    };
    Some(ServerPacket::Chat { message, chat_type })
}

pub(super) fn crystal_npc_set_conquest_rate(world: &mut World, parts: &[&str]) {
    let Some(rate) = parts.last().and_then(|value| value.parse::<u8>().ok()) else {
        return;
    };
    let mut resources = world.resource_mut::<Stage5SystemsResource>();
    resources.stage5_systems.conquest.tax_rate_percent = rate;
    resources
        .stage5_systems
        .conquest
        .event_log
        .push(format!("Conquest tax rate set to {rate}"));
}

pub(super) fn crystal_npc_repair_conquest_asset(
    world: &mut World,
    kind: ConquestAssetKind,
    parts: &[&str],
) {
    let Some(asset_id) = crystal_npc_asset_id(parts) else {
        return;
    };
    let mut resources = world.resource_mut::<Stage5SystemsResource>();
    match kind {
        ConquestAssetKind::Guard => {
            push_unique_u8(&mut resources.stage5_systems.conquest.guards, asset_id)
        }
        ConquestAssetKind::Wall => {
            push_unique_u8(&mut resources.stage5_systems.conquest.walls, asset_id)
        }
        ConquestAssetKind::Gate => {
            push_unique_u8(&mut resources.stage5_systems.conquest.gates, asset_id)
        }
    }
    resources
        .stage5_systems
        .conquest
        .event_log
        .push(format!("{kind:?} {asset_id} repaired"));
}

pub(super) fn crystal_npc_repair_all_conquest_assets(world: &mut World) {
    let mut resources = world.resource_mut::<Stage5SystemsResource>();
    resources.stage5_systems.conquest.guards = (1..=12).collect();
    resources.stage5_systems.conquest.walls = (1..=3).collect();
    resources.stage5_systems.conquest.gates = vec![1];
    resources
        .stage5_systems
        .conquest
        .event_log
        .push("All conquest assets repaired".to_string());
}

pub(super) fn crystal_npc_set_conquest_gate_open(world: &mut World, parts: &[&str], open: bool) {
    let Some(asset_id) = crystal_npc_asset_id(parts) else {
        return;
    };
    let mut resources = world.resource_mut::<Stage5SystemsResource>();
    push_unique_u8(&mut resources.stage5_systems.conquest.gates, asset_id);
    if open {
        push_unique_u8(&mut resources.stage5_systems.conquest.open_gates, asset_id);
    } else {
        resources
            .stage5_systems
            .conquest
            .open_gates
            .retain(|id| *id != asset_id);
    }
    resources.stage5_systems.conquest.event_log.push(format!(
        "Gate {asset_id} {}",
        if open { "opened" } else { "closed" }
    ));
}

pub(super) fn crystal_npc_start_conquest(world: &mut World, parts: &[&str]) {
    let conquest_id = parts.first().copied().unwrap_or("1");
    let mut resources = world.resource_mut::<Stage5SystemsResource>();
    push_unique(
        &mut resources.stage5_systems.conquest.active_wars,
        format!("Conquest {conquest_id}"),
    );
    resources
        .stage5_systems
        .conquest
        .event_log
        .push(format!("Conquest {conquest_id} started"));
}

pub(super) fn crystal_npc_take_conquest_gold(world: &mut World, parts: &[&str]) {
    let amount = parts
        .get(1)
        .or_else(|| parts.first())
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0);
    let mut stage5 = world.resource_mut::<Stage5SystemsResource>();
    if stage5.stage5_systems.conquest.gold >= amount {
        stage5.stage5_systems.conquest.gold -= amount;
    } else {
        drop(stage5);
        let mut player_runtime = world.resource_mut::<PlayerRuntimeResource>();
        player_runtime.gold = player_runtime.gold.saturating_sub(amount);
    }
}

pub(super) fn crystal_npc_buy_gt(world: &mut World) {
    let mut resources = world.resource_mut::<Stage5SystemsResource>();
    resources.stage5_systems.guild_territory.owned = true;
    resources.stage5_systems.guild_territory.rental_days_left = resources
        .stage5_systems
        .guild_territory
        .rental_days_left
        .max(7);
    if resources
        .stage5_systems
        .guild_territory
        .map_file_name
        .is_empty()
    {
        resources.stage5_systems.guild_territory.map_file_name = "GA0".to_string();
    }
    resources
        .stage5_systems
        .guild_territory
        .recall_log
        .push("Guild territory rented".to_string());
}

pub(super) fn crystal_npc_extend_gt(world: &mut World) {
    let mut resources = world.resource_mut::<Stage5SystemsResource>();
    if !resources.stage5_systems.guild_territory.owned {
        return;
    }
    resources.stage5_systems.guild_territory.rental_days_left = resources
        .stage5_systems
        .guild_territory
        .rental_days_left
        .saturating_add(7);
}

pub(super) fn crystal_npc_gt_recall(world: &mut World, target: Option<&str>) {
    let mut resources = world.resource_mut::<Stage5SystemsResource>();
    if !resources.stage5_systems.guild_territory.owned {
        return;
    }
    let target = target
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("ALL");
    resources
        .stage5_systems
        .guild_territory
        .recall_log
        .push(format!("GT recall: {target}"));
}

pub(super) fn crystal_npc_teleport_gt_packets(world: &mut World) -> Option<Vec<ServerPacket>> {
    let map_file_name = {
        let resources = world.resource::<Stage5SystemsResource>();
        if !resources.stage5_systems.guild_territory.owned {
            return None;
        }
        resources
            .stage5_systems
            .guild_territory
            .map_file_name
            .clone()
    };
    let current_direction = world.resource::<PlayerRuntimeResource>().player_direction;
    let map_info = crystal_npc_move_map_information(world, &map_file_name);
    Some(relocate_player_to_map(
        world,
        map_info,
        Point { x: 54, y: 55 },
        current_direction,
        None,
    ))
}

pub(super) fn crystal_npc_add_to_guild(world: &mut World, parts: &[&str]) {
    let guild_name = parts.first().copied().unwrap_or("NewbieGuild").to_string();
    let player_name = stage5_player_name(world);
    let mut resources = world.resource_mut::<Stage5SystemsResource>();
    if resources.stage5_systems.guild.name.is_empty() {
        resources.stage5_systems.guild.name = guild_name;
    }
    push_unique(&mut resources.stage5_systems.guild.members, player_name);
    if resources.stage5_systems.guild.rank.is_empty() {
        resources.stage5_systems.guild.rank = "Member".to_string();
    }
}

pub(super) fn crystal_npc_name_list(world: &mut World, parts: &[&str], add: bool) {
    let Some(name_list) = parts
        .first()
        .map(|value| value.trim_matches('"').to_string())
    else {
        return;
    };
    let mut resources = world.resource_mut::<Stage5SystemsResource>();
    if add {
        push_unique(&mut resources.stage5_systems.name_lists, name_list);
    } else {
        resources
            .stage5_systems
            .name_lists
            .retain(|entry| !entry.eq_ignore_ascii_case(&name_list));
    }
}

pub(super) fn crystal_npc_change_level(world: &mut World, parts: &[&str]) {
    let Some(level) = parts
        .first()
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|level| *level > 0)
    else {
        return;
    };
    {
        let mut session = world.resource_mut::<SessionResource>();
        if let Some(character) = session.selected_character.as_mut() {
            character.level = level;
        }
    }
    if let Some(player) = player_entity(world) {
        if let Ok(mut entry) = world.get_entity_mut(player) {
            if let Some(mut body) = entry.get_mut::<CharacterBody>() {
                body.level = level;
            }
        }
    }
}

pub(super) fn crystal_npc_change_hair(world: &mut World, parts: &[&str]) {
    let Some(hair) = parts.first().and_then(|value| value.parse::<u8>().ok()) else {
        return;
    };
    world
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .appearance
        .hair = hair;
}

pub(super) fn crystal_npc_give_buff(world: &mut World, parts: &[&str]) {
    let [key, duration @ ..] = parts else {
        return;
    };
    let key = normalize_stage5_key(key);
    let duration_ticks = duration
        .first()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(60)
        .max(1);
    let tick = runtime_tick(world);
    apply_or_refresh_buff(
        world,
        BuffState {
            key: key.clone(),
            name: stage5_item_name(&key),
            description: "Crystal NPC buff.".to_string(),
            expires_at_tick: tick.saturating_add(duration_ticks),
            attack_bonus: 0,
            defence_bonus: 0,
            stats: Vec::new(),
        },
    );
}

pub(super) fn crystal_npc_revive_hero(world: &mut World) {
    let mut resources = world.resource_mut::<Stage5SystemsResource>();
    let hero_name = resources
        .stage5_systems
        .hero
        .as_ref()
        .map(|hero| hero.name.clone())
        .unwrap_or_else(|| "Hero".to_string());
    resources.stage5_systems.hero = Some(Stage5HeroState {
        name: hero_name,
        level: 1,
        class: mir2_protocol::MirClass::Warrior,
        gender: mir2_protocol::MirGender::Male,
        behaviour: 0,
        experience: 0,
        spawned: true,
        auto_pot: true,
        auto_hp_percent: 0,
        auto_mp_percent: 0,
        hp_item_index: 0,
        mp_item_index: 0,
    });
}

pub(super) fn crystal_npc_seal_hero(world: &mut World) {
    let had_hero = world
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .hero
        .take()
        .is_some();
    if had_hero {
        add_or_increment_item(
            world,
            ItemContainer::Bag1,
            "sealed-hero",
            "SealedHero",
            "Crystal sealed hero.",
            23,
            1,
            1,
        );
    }
}

pub(super) fn crystal_npc_record_timed_recall(
    world: &mut World,
    seconds: Option<&str>,
    enabled: bool,
) {
    let mut resources = world.resource_mut::<Stage5SystemsResource>();
    let message = if enabled {
        format!("Timed recall set: {}", seconds.unwrap_or("0"))
    } else {
        "Timed recall cancelled".to_string()
    };
    resources.stage5_systems.conquest.event_log.push(message);
}

pub(super) fn crystal_npc_give_exp(world: &mut World, parts: &[&str]) {
    let Some(amount) = parts.first().and_then(|value| value.parse::<i64>().ok()) else {
        return;
    };

    let mut player_runtime = world.resource_mut::<PlayerRuntimeResource>();
    player_runtime.experience = player_runtime.experience.saturating_add(amount.max(0));
    if player_runtime.experience > player_runtime.max_experience {
        player_runtime.experience = player_runtime.max_experience;
    }
}

pub(super) fn crystal_npc_load_value(world: &mut World, line: &str) {
    let Some(parsed) = parse_crystal_npc_load_value(line) else {
        return;
    };

    let saved = world
        .resource::<NpcStateResource>()
        .npc_saved_values
        .iter()
        .find(|entry| {
            entry.file_name.eq_ignore_ascii_case(&parsed.file_name)
                && entry.group.eq_ignore_ascii_case(&parsed.group)
                && entry.key.eq_ignore_ascii_case(&parsed.key)
        })
        .map(|entry| entry.value.clone());

    if let Some(value) = saved {
        set_crystal_npc_variable_value(world, &parsed.variable, value);
    }
}

pub(super) fn crystal_npc_save_value(world: &mut World, line: &str) {
    let Some(parsed) = parse_crystal_npc_save_value(line) else {
        return;
    };

    let mut resources = world.resource_mut::<NpcStateResource>();
    if let Some(existing) = resources.npc_saved_values.iter_mut().find(|entry| {
        entry.file_name.eq_ignore_ascii_case(&parsed.file_name)
            && entry.group.eq_ignore_ascii_case(&parsed.group)
            && entry.key.eq_ignore_ascii_case(&parsed.key)
    }) {
        existing.value = parsed.value;
        return;
    }

    resources.npc_saved_values.push(CrystalNpcSavedValue {
        file_name: parsed.file_name,
        group: parsed.group,
        key: parsed.key,
        value: parsed.value,
    });
}

impl SimulationSession {
    pub fn shared_npc_random_seed(&self) -> u64 {
        self.app.world().resource::<CrystalNpcRandomState>().seed
    }

    pub fn apply_shared_npc_random_seed(&mut self, seed: u64) {
        self.app
            .world_mut()
            .resource_mut::<CrystalNpcRandomState>()
            .seed = seed;
    }

    pub fn shared_npc_saved_values(&self) -> Vec<CrystalNpcSavedValue> {
        self.app
            .world()
            .resource::<NpcStateResource>()
            .npc_saved_values
            .clone()
    }

    pub fn apply_shared_npc_saved_values(&mut self, values: &[CrystalNpcSavedValue]) {
        let resources = &mut self
            .app
            .world_mut()
            .resource_mut::<NpcStateResource>()
            .npc_saved_values;
        for value in values {
            if let Some(existing) = resources.iter_mut().find(|entry| {
                entry.file_name.eq_ignore_ascii_case(&value.file_name)
                    && entry.group.eq_ignore_ascii_case(&value.group)
                    && entry.key.eq_ignore_ascii_case(&value.key)
            }) {
                existing.value = value.value.clone();
            } else {
                resources.push(value.clone());
            }
        }
    }

    pub fn interact(&mut self, object_id: u32) -> Vec<ServerPacket> {
        let packets = self.interact_impl(object_id);
        self.finalize_packets(packets)
    }

    pub fn interact_shared_npc_snapshot(&mut self, npc: &WorldEntitySnapshot) -> Vec<ServerPacket> {
        let packets = self.interact_shared_npc_snapshot_impl(npc);
        self.finalize_packets(packets)
    }

    pub fn call_shared_npc_snapshot(
        &mut self,
        npc: &WorldEntitySnapshot,
        key: &str,
    ) -> Vec<ServerPacket> {
        let packets = self.call_shared_npc_snapshot_impl(npc, key);
        self.finalize_packets(packets)
    }

    pub fn call_npc(&mut self, object_id: u32, key: &str) -> Vec<ServerPacket> {
        let packets = self.call_npc_impl(object_id, key);
        self.finalize_packets(packets)
    }

    pub fn select_npc_dialog_target(&mut self, target: &str) -> Vec<ServerPacket> {
        let packets = self.select_npc_dialog_target_impl(target);
        self.finalize_packets(packets)
    }

    pub fn submit_npc_input(&mut self, value: &str) -> Vec<ServerPacket> {
        let packets = self.submit_npc_input_impl(value);
        self.finalize_packets(packets)
    }

    pub fn confirm_npc_input(
        &mut self,
        npc_id: u32,
        page_name: &str,
        value: &str,
    ) -> Vec<ServerPacket> {
        let packets = self.confirm_npc_input_impl(npc_id, page_name, value);
        self.finalize_packets(packets)
    }

    pub(super) fn interact_impl(&mut self, object_id: u32) -> Vec<ServerPacket> {
        if !is_in_world(self.app.world()) {
            return Vec::new();
        }

        let Some(npc_entity) = entity_by_object_id(self.app.world(), object_id) else {
            return Vec::new();
        };

        let player_entity = player_entity(self.app.world()).expect("player should exist");
        let player_position =
            entity_position(self.app.world(), player_entity).expect("player position");
        let npc_position = entity_position(self.app.world(), npc_entity).expect("npc position");
        let npc_name =
            entity_name(self.app.world(), npc_entity).unwrap_or_else(|| "NPC".to_string());
        let npc_quest_ids = self
            .app
            .world()
            .entity(npc_entity)
            .get::<NpcAgent>()
            .map(|agent| agent.quest_ids.clone())
            .unwrap_or_default();
        let npc_script_key = self
            .app
            .world()
            .entity(npc_entity)
            .get::<NpcAgent>()
            .and_then(|agent| agent.script_key.clone());

        let Some(direction) = direction_toward(&player_position, &npc_position) else {
            return Vec::new();
        };

        let mut packets = Vec::new();
        {
            let mut player = self.app.world_mut().entity_mut(player_entity);
            let mut facing = player.get_mut::<Facing>().expect("player facing");
            if facing.0 != direction {
                facing.0 = direction;
                packets.push(ServerPacket::ObjectTurn {
                    movement: current_movement(self.app.world()),
                });
            }
        }

        if tile_distance(&player_position, &npc_position) > 1 {
            return packets;
        }

        let context = NpcInteractionContext {
            object_id,
            name: npc_name,
            name_key: localized_npc_name_key(object_id).map(str::to_string),
            position: npc_position,
            quest_ids: npc_quest_ids,
            script_key: npc_script_key,
            args: Vec::new(),
            input: None,
        };
        handle_npc_interaction(self.app.world_mut(), context, packets)
    }

    pub(super) fn interact_shared_npc_snapshot_impl(
        &mut self,
        npc: &WorldEntitySnapshot,
    ) -> Vec<ServerPacket> {
        if !self.ensure_shared_npc_snapshot_entity(npc) {
            return Vec::new();
        }

        self.interact_impl(npc.object_id)
    }

    pub(super) fn call_shared_npc_snapshot_impl(
        &mut self,
        npc: &WorldEntitySnapshot,
        key: &str,
    ) -> Vec<ServerPacket> {
        if !self.ensure_shared_npc_snapshot_entity(npc) {
            return Vec::new();
        }

        self.call_npc_impl(npc.object_id, key)
    }

    fn ensure_shared_npc_snapshot_entity(&mut self, npc: &WorldEntitySnapshot) -> bool {
        if npc.kind != WorldEntityKind::Npc || !is_in_world(self.app.world()) {
            return false;
        }

        if let Some(entity) = entity_by_object_id(self.app.world(), npc.object_id) {
            return self.app.world().entity(entity).contains::<Npc>();
        }

        let npc_info = crystal_npc_info_for_loaded_object_id(npc.object_id);
        let quest_ids = if npc.quest_ids.is_empty() {
            crystal_quest_ids_by_npc()
                .get(&npc.object_id)
                .map(|ids| ids.iter().copied().collect())
                .unwrap_or_default()
        } else {
            npc.quest_ids.clone()
        };
        let script_key = npc_info.as_ref().map(|info| info.script_key.clone());
        let image = npc_info.as_ref().map(|info| info.image).unwrap_or_default();

        self.app.world_mut().spawn((
            WorldObject,
            Npc,
            ObjectId(npc.object_id),
            localized_npc_name_key(npc.object_id)
                .map(|key| DisplayName::localized(key, npc.name.clone()))
                .unwrap_or_else(|| DisplayName::literal(npc.name.clone())),
            Position(Point { x: npc.x, y: npc.y }),
            Facing(npc.direction),
            NpcAgent {
                image,
                colour_argb: npc.name_colour_argb,
                quest_ids,
                script_key,
            },
        ));

        true
    }

    pub(super) fn call_npc_impl(&mut self, object_id: u32, key: &str) -> Vec<ServerPacket> {
        let normalized_key = key.trim();
        let main_call = normalized_key.is_empty() || normalized_key.eq_ignore_ascii_case("@Main");
        if main_call {
            return self.interact_impl(object_id);
        }

        let active_npc_object_id = self
            .app
            .world()
            .resource::<NpcStateResource>()
            .active_npc_dialog
            .as_ref()
            .map(|dialog| dialog.npc_object_id);
        if active_npc_object_id == Some(object_id) {
            return self.select_npc_dialog_target_impl(normalized_key);
        }

        let mut packets = self.interact_impl(object_id);
        let active_npc_object_id = self
            .app
            .world()
            .resource::<NpcStateResource>()
            .active_npc_dialog
            .as_ref()
            .map(|dialog| dialog.npc_object_id);
        if active_npc_object_id == Some(object_id) {
            packets.extend(self.select_npc_dialog_target_impl(normalized_key));
        }
        packets
    }

    pub(super) fn select_npc_dialog_target_impl(&mut self, target: &str) -> Vec<ServerPacket> {
        self.select_npc_dialog_target_with_input_impl(target, None)
    }

    pub(super) fn select_npc_dialog_target_with_input_impl(
        &mut self,
        target: &str,
        input_value: Option<String>,
    ) -> Vec<ServerPacket> {
        if !is_in_world(self.app.world()) {
            return Vec::new();
        }

        let Some(active_dialog) = self
            .app
            .world()
            .resource::<NpcStateResource>()
            .active_npc_dialog
            .clone()
        else {
            return Vec::new();
        };

        let (normalized_target, label_args) = parse_crystal_label_target(target);
        if normalized_target.eq_ignore_ascii_case("@Exit") {
            dismiss_dialog(self.app.world_mut());
            return Vec::new();
        }

        let input_link = target.trim_start().starts_with("@@");
        if input_link && input_value.is_none() {
            let prompt = crystal_npc_input_prompt(&active_dialog, target);
            set_dialog(
                self.app.world_mut(),
                ActiveNpcDialogState {
                    npc_object_id: active_dialog.npc_object_id,
                    npc_name: active_dialog.npc_name.clone(),
                    npc_name_key: active_dialog.npc_name_key.clone(),
                    stage: None,
                    current: 0,
                    required: 0,
                    title: active_dialog.npc_name.clone(),
                    body: vec![prompt.clone()],
                    footer: active_dialog.npc_name.clone(),
                    links: Vec::new(),
                    input: Some(NpcDialogInputState {
                        target: normalized_target,
                        prompt,
                    }),
                },
            );
            return Vec::new();
        }

        if !active_dialog
            .links
            .iter()
            .any(|link| crystal_npc_labels_match(&link.target, &normalized_target))
            && active_dialog
                .input
                .as_ref()
                .is_none_or(|input| !crystal_npc_labels_match(&input.target, &normalized_target))
        {
            return Vec::new();
        }

        if normalized_target
            .trim_start_matches('@')
            .starts_with("quest:")
        {
            let Some((quest_dialog, mut packets)) = crystal_quest_dialog_for_target(
                self.app.world_mut(),
                active_dialog.npc_object_id,
                &normalized_target,
            ) else {
                return Vec::new();
            };
            set_dialog(
                self.app.world_mut(),
                ActiveNpcDialogState {
                    npc_object_id: active_dialog.npc_object_id,
                    npc_name: active_dialog.npc_name.clone(),
                    npc_name_key: active_dialog.npc_name_key.clone(),
                    stage: Some(quest_dialog.stage),
                    current: 0,
                    required: 1,
                    title: quest_dialog.title,
                    body: quest_dialog.body,
                    footer: quest_dialog.footer.clone(),
                    links: quest_dialog.links,
                    input: None,
                },
            );
            packets.push(ServerPacket::ObjectChat {
                object_id: active_dialog.npc_object_id,
                text: quest_dialog.footer,
                chat_type: ChatType::Hint,
            });
            return packets;
        }

        let Some(npc_entity) = entity_by_object_id(self.app.world(), active_dialog.npc_object_id)
        else {
            dismiss_dialog(self.app.world_mut());
            return Vec::new();
        };

        let npc_entry = self.app.world().entity(npc_entity);
        let npc_agent = npc_entry.get::<NpcAgent>().cloned();
        let context = NpcInteractionContext {
            object_id: active_dialog.npc_object_id,
            name: active_dialog.npc_name.clone(),
            name_key: active_dialog.npc_name_key.clone(),
            position: entity_position(self.app.world(), npc_entity).unwrap_or(Point { x: 0, y: 0 }),
            quest_ids: npc_agent
                .as_ref()
                .map(|agent| agent.quest_ids.clone())
                .unwrap_or_default(),
            script_key: npc_agent.and_then(|agent| agent.script_key),
            args: label_args,
            input: input_value,
        };

        let Some(script_key) = context.script_key.clone() else {
            dismiss_dialog(self.app.world_mut());
            return Vec::new();
        };

        let Some(result) = run_crystal_npc_script(
            self.app.world_mut(),
            &context,
            &script_key,
            &normalized_target,
        ) else {
            dismiss_dialog(self.app.world_mut());
            return Vec::new();
        };

        if let Some(dialog) = result.dialog {
            let footer = dialog.footer.clone();
            set_dialog(self.app.world_mut(), dialog);
            let mut packets = result.packets;
            self.visible_objects = collect_visible_objects(self.app.world())
                .keys()
                .copied()
                .collect();
            packets.push(ServerPacket::ObjectChat {
                object_id: context.object_id,
                text: footer,
                chat_type: ChatType::Hint,
            });
            packets
        } else {
            dismiss_dialog(self.app.world_mut());
            result.packets
        }
    }

    pub(super) fn submit_npc_input_impl(&mut self, value: &str) -> Vec<ServerPacket> {
        let Some(active_dialog) = self
            .app
            .world()
            .resource::<NpcStateResource>()
            .active_npc_dialog
            .clone()
        else {
            return Vec::new();
        };
        let Some(input) = active_dialog.input.clone() else {
            return Vec::new();
        };

        self.select_npc_dialog_target_with_input_impl(&input.target, Some(value.to_string()))
    }

    pub(super) fn confirm_npc_input_impl(
        &mut self,
        npc_id: u32,
        page_name: &str,
        value: &str,
    ) -> Vec<ServerPacket> {
        let Some(active_dialog) = self
            .app
            .world()
            .resource::<NpcStateResource>()
            .active_npc_dialog
            .clone()
        else {
            return Vec::new();
        };
        if active_dialog.npc_object_id != npc_id {
            return Vec::new();
        }

        let target = if page_name.trim().is_empty() {
            let Some(input) = active_dialog.input.as_ref() else {
                return Vec::new();
            };
            input.target.clone()
        } else {
            let trimmed = page_name.trim();
            if trimmed.starts_with('@') {
                trimmed.to_string()
            } else {
                format!("@{trimmed}")
            }
        };

        self.select_npc_dialog_target_with_input_impl(&target, Some(value.to_string()))
    }
}
