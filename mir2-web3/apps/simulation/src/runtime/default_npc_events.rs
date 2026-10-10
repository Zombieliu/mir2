//! Trusted Crystal AutoPlayer / 00Default hooks. This is a bounded source
//! queue, not a scheduler inferred from otherwise uncalled Event filenames.
use bevy_ecs::prelude::{Resource, World};
use mir2_game_data::CrystalNpcScript;
use mir2_protocol::{Point, ServerPacket};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

use super::npc::{dismiss_dialog, set_dialog};
use super::npc_script::{normalize_crystal_npc_label, run_crystal_default_npc_script, NpcInteractionContext};
use super::resources::{is_in_world, MapRuntimeResource, NpcStateResource, PlayerRuntimeResource, SessionResource};
use super::components::current_player_object_id;

pub(super) const MAX_PENDING: usize = 128;
pub(super) const DRAIN_BUDGET: usize = 32;
const SCRIPT_KEY: &str = "00Default";
const SOURCE_EXECUTION_SHA256: &str = "5522ab1f667a363e34965f7552f7b482f93954ad53c2dc776c5eed8a03f9f8be";
const SOURCE_PARSER_CONTRACT_SHA256: &str = "44f92fb3f80c8fb939f442c62781685a45033bd0392d2aeb7cf5526857832e51";
const SOURCE: &str = include_str!("../../../../packages/game-data/data/generated/crystal_default_npc_scripts.json");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum DefaultNpcEvent {
    Login,
    UseItem { shape: u16 },
    Trigger { name: String },
    MapCoord { map_file_name: String, position: Point },
    MapEnter { map_file_name: String },
    Die,
    LevelUp,
    CustomCommand { name: String },
    OnAcceptQuest { quest_id: i32 },
    OnFinishQuest { quest_id: i32 },
    Daily,
    Client,
}

impl DefaultNpcEvent {
    pub(super) fn label(&self) -> String {
        match self {
            Self::Login => "@_Login".into(),
            Self::UseItem { shape } => format!("@_UseItem({shape})"),
            Self::Trigger { name } => format!("@_Trigger({name})"),
            Self::MapCoord { map_file_name, position } => format!("@_MapCoord({map_file_name},{},{})", position.x, position.y),
            Self::MapEnter { map_file_name } => format!("@_MapEnter({map_file_name})"),
            Self::Die => "@_Die".into(),
            Self::LevelUp => "@_LevelUp".into(),
            Self::CustomCommand { name } => format!("@_CustomCommand({name})"),
            Self::OnAcceptQuest { quest_id } => format!("@_OnAcceptQuest({quest_id})"),
            Self::OnFinishQuest { quest_id } => format!("@_OnFinishQuest({quest_id})"),
            Self::Daily => "@_Daily".into(),
            Self::Client => "@_Client".into(),
        }
    }
    fn validate(&self) -> Result<(), String> {
        let token = |name: &str| !name.is_empty() && name.len() <= 128
            && !name.chars().any(|c| c.is_control() || matches!(c, '(' | ')' | ',' | '[' | ']'));
        let valid = match self {
            Self::Trigger { name } | Self::CustomCommand { name } => token(name),
            Self::MapEnter { map_file_name } => token(map_file_name),
            Self::MapCoord { map_file_name, position } => token(map_file_name)
                && position.x >= 0 && position.y >= 0,
            Self::OnAcceptQuest { quest_id } | Self::OnFinishQuest { quest_id } => *quest_id >= 0,
            _ => true,
        };
        valid.then_some(()).ok_or_else(|| "invalid trusted default NPC hook parameter".into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueuedDefaultNpcEvent {
    pub sequence: u64,
    pub event: DefaultNpcEvent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enter_map: Option<DefaultNpcEnterMapTicket>,
}

/// Produced only by the actual trusted NeedMove route. A client cannot choose
/// a destination by calling NPC0, and a saved queued hook cannot cross actors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultNpcEnterMapTicket {
    pub account_id: String,
    pub character_index: i32,
    pub object_id: u32,
    pub source_map_file_name: String,
    pub source_position: Point,
    pub destination_map_file_name: String,
    pub destination_position: Point,
    pub event_sequence: u64,
}

/// Kept with the complete source checkpoint. `committed_sequence` is tentative
/// until its enclosing ordinary command's durable checkpoint succeeds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultNpcEventSnapshot {
    #[serde(default)]
    pub next_sequence: u64,
    #[serde(default)]
    pub committed_sequence: u64,
    #[serde(default)]
    pub pending: Vec<QueuedDefaultNpcEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_daily_local_date: Option<u32>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub observed_dead: bool,
}

impl DefaultNpcEventSnapshot {
    pub fn is_empty(&self) -> bool {
        self.next_sequence == 0 && self.committed_sequence == 0 && self.pending.is_empty()
            && self.last_daily_local_date.is_none() && !self.observed_dead
    }
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.pending.len() > MAX_PENDING || self.committed_sequence > self.next_sequence
            || self.next_sequence - self.committed_sequence != self.pending.len() as u64 {
            return Err("default NPC source queue sequence or size is invalid".into());
        }
        if self.last_daily_local_date.is_some_and(|date|
            date / 10_000 == 0 || !(1..=12).contains(&((date / 100) % 100)) || !(1..=31).contains(&(date % 100))) {
            return Err("default NPC daily local date is invalid".into());
        }
        for (index, queued) in self.pending.iter().enumerate() {
            if self.committed_sequence.checked_add(index as u64 + 1) != Some(queued.sequence) {
                return Err("default NPC source queue sequence is missing or duplicated".into());
            }
            queued.event.validate()?;
            if let Some(ticket) = &queued.enter_map {
                let DefaultNpcEvent::MapCoord { map_file_name, position } = &queued.event else {
                    return Err("default NPC entry ticket is not paired with a coordinate event".into());
                };
                if ticket.event_sequence != queued.sequence || ticket.account_id.is_empty()
                    || ticket.character_index < 0 || ticket.object_id == 0
                    || ticket.source_map_file_name != *map_file_name || ticket.source_position != *position
                    || !source_entry_binding(ticket) {
                    return Err("default NPC entry ticket differs from its original movement or event".into());
                }
            }
        }
        Ok(())
    }
}

/// Crystal keeps CanGainExp on the live PlayerObject, outside CharacterInfo.
/// Source command checkpoints clone it for rollback, while a new actor/relog
/// starts with the original true default rather than restoring it from a save.
#[derive(Debug, Clone, PartialEq, Eq, Resource)]
pub(super) struct DefaultNpcTransientState {
    can_gain_experience: bool,
}

impl Default for DefaultNpcTransientState {
    fn default() -> Self { Self { can_gain_experience: true } }
}

pub(super) fn can_gain_experience(world: &World) -> bool {
    world.get_resource::<DefaultNpcTransientState>()
        .map_or(true, |state| state.can_gain_experience)
}

pub(super) fn capture_transient(world: &World) -> DefaultNpcTransientState {
    world.get_resource::<DefaultNpcTransientState>().cloned().unwrap_or_default()
}

pub(super) fn restore_transient(world: &mut World, state: &DefaultNpcTransientState) {
    world.insert_resource(state.clone());
}

pub(super) fn reset_transient(world: &mut World) {
    world.insert_resource(DefaultNpcTransientState::default());
}

pub(super) fn set_can_gain_experience(world: &mut World, value: bool) {
    if world.get_resource::<DefaultNpcTransientState>().is_none() {
        reset_transient(world);
    }
    world.resource_mut::<DefaultNpcTransientState>().can_gain_experience = value;
}

#[derive(Debug, Clone, Default, Resource)]
pub(super) struct DefaultNpcEventQueue {
    pub(super) snapshot: DefaultNpcEventSnapshot,
    draining: bool,
    active_sequence: Option<u64>,
}

#[derive(Deserialize, Serialize)]
struct SourceFile {
    relative_path: String,
    sha256: String,
    raw_text: String,
}
#[derive(Deserialize, Serialize)]
struct IgnoredLine {
    section_label: String,
    line_number: usize,
    line: String,
    reason: String,
}
#[derive(Deserialize, Serialize)]
struct NameListSource {
    relative_path: String,
    exists: bool,
}
#[derive(Deserialize, Serialize)]
struct ParserContract {
    parser_source: SourceFile,
    ignored_lines: Vec<IgnoredLine>,
    name_lists: Vec<NameListSource>,
}
#[derive(Deserialize)]
struct SourceBundle {
    schema_version: u32,
    root_sha256: String,
    execution_sha256: String,
    parser_contract_sha256: String,
    parser_contract: ParserContract,
    sources: Vec<SourceFile>,
    script: CrystalNpcScript,
}
fn digest(bytes: impl AsRef<[u8]>) -> String {
    Sha256::digest(bytes.as_ref()).iter().map(|v| format!("{v:02x}")).collect()
}

pub(super) fn expanded_script() -> Result<&'static CrystalNpcScript, String> {
    static SCRIPT: OnceLock<Result<CrystalNpcScript, String>> = OnceLock::new();
    SCRIPT.get_or_init(|| {
        let mut source: SourceBundle = serde_json::from_str(SOURCE).map_err(|e| e.to_string())?;
        if source.schema_version != 1 || source.script.script_key != SCRIPT_KEY
            || source.script.relative_path != "00Default.txt" || source.sources.is_empty()
            || source.sources.len() > MAX_PENDING || source.execution_sha256 != SOURCE_EXECUTION_SHA256
            || digest(serde_json::to_vec(&source.script).map_err(|e| e.to_string())?) != SOURCE_EXECUTION_SHA256 {
            return Err("default NPC expanded original source proof is invalid".into());
        }
        let root = &source.sources[0];
        if root.relative_path != "NPCs/00Default.txt" || root.sha256 != source.root_sha256
            || root.raw_text != source.script.raw_text {
            return Err("default NPC root source proof is invalid".into());
        }
        for file in &source.sources {
            if file.relative_path != "NPCs/00Default.txt"
                && (!file.relative_path.starts_with("SystemScripts/00Default/")
                    || file.relative_path.split('/').any(|p| p == "..")) {
                return Err("default NPC source escaped its original script family".into());
            }
            if digest(file.raw_text.as_bytes()) != file.sha256 {
                return Err("default NPC source text hash differs".into());
            }
        }
        let contract = &source.parser_contract;
        if source.parser_contract_sha256 != SOURCE_PARSER_CONTRACT_SHA256
            || digest(serde_json::to_vec(contract).map_err(|e| e.to_string())?) != SOURCE_PARSER_CONTRACT_SHA256
            || contract.parser_source.relative_path != "Server/MirObjects/NPC/NPCSegment.cs"
            || digest(contract.parser_source.raw_text.as_bytes()) != contract.parser_source.sha256
            || contract.ignored_lines.len() > MAX_PENDING {
            return Err("default NPC original parser proof is invalid".into());
        }
        // Preserve line offsets and the sealed raw script while reproducing the
        // original parser's omitted instructions. In particular an absent
        // CHECKNAMELIST is omitted, not converted into a failed membership test.
        for ignored in &contract.ignored_lines {
            if !matches!(ignored.reason.as_str(), "unknown-original-opcode" | "missing-original-name-list"
                | "missing-original-checkhum-arguments") {
                return Err("default NPC source has an unverified omission reason".into());
            }
            let section = source.script.sections.iter_mut().find(|section| {
                section.label == ignored.section_label && ignored.line_number > section.line_number
                    && ignored.line_number <= section.line_number + section.lines.len()
            }).ok_or("default NPC omitted source line has no matching section")?;
            let line = &mut section.lines[ignored.line_number - section.line_number - 1];
            if *line != ignored.line {
                return Err("default NPC omitted source line differs".into());
            }
            line.clear();
        }
        Ok(source.script)
    }).as_ref().map_err(Clone::clone)
}

pub(super) fn capture(world: &World) -> DefaultNpcEventSnapshot {
    world.get_resource::<DefaultNpcEventQueue>().map(|queue| queue.snapshot.clone()).unwrap_or_default()
}
pub(super) fn restore(world: &mut World, snapshot: &DefaultNpcEventSnapshot) -> Result<(), String> {
    snapshot.validate()?;
    world.insert_resource(DefaultNpcEventQueue { snapshot: snapshot.clone(), draining:false, active_sequence:None });
    Ok(())
}
pub(super) fn has_pending(world: &World) -> bool {
    world.get_resource::<DefaultNpcEventQueue>().is_some_and(|queue| !queue.snapshot.pending.is_empty())
}
fn local_date() -> Result<u32, String> {
    let time = super::npc_script::crystal_local_time_snapshot().ok_or("default NPC server local clock unavailable")?;
    Ok(u32::from(time.year)*10_000 + u32::from(time.month)*100 + u32::from(time.day))
}
pub(super) fn daily_due(world: &World) -> bool {
    if !is_in_world(world) { return false; }
    local_date().is_ok_and(|today| capture(world).last_daily_local_date.is_some_and(|previous| previous != today))
}
pub(super) fn enqueue_daily_if_due(world: &mut World) -> Result<bool, String> {
    if !is_in_world(world) { return Ok(false); }
    let today = local_date().map_err(|error| reject(world,error))?;
    world.init_resource::<DefaultNpcEventQueue>();
    let previous = world.resource::<DefaultNpcEventQueue>().snapshot.last_daily_local_date;
    if previous == Some(today) { return Ok(false); }
    // New characters/migrated snapshots begin with NewDay=false. Once a
    // character has a recorded server-local date, the next date change mirrors
    // Envir.ProcessNewDay, including offline characters on their next login.
    if previous.is_some() { enqueue(world,DefaultNpcEvent::Daily)?; }
    world.resource_mut::<DefaultNpcEventQueue>().snapshot.last_daily_local_date = Some(today);
    Ok(previous.is_some())
}

pub(super) fn dispatch_source_packets(world: &mut World, packets: &mut Vec<ServerPacket>) {
    if !is_in_world(world) { return; }
    let _ = observe_life(world);
    let _ = enqueue_daily_if_due(world);
    match dispatch(world) {
        Ok(events) => packets.extend(events),
        Err(error) => { reject(world,error); }
    }
}
pub(super) fn observe_life(world: &mut World) -> Result<(), String> {
    if !is_in_world(world) { return Ok(()); }
    let dead = super::components::current_player_is_dead(world);
    world.init_resource::<DefaultNpcEventQueue>();
    let previous = world.resource::<DefaultNpcEventQueue>().snapshot.observed_dead;
    if dead && !previous { enqueue(world,DefaultNpcEvent::Die)?; }
    world.resource_mut::<DefaultNpcEventQueue>().snapshot.observed_dead = dead;
    Ok(())
}
fn reject(world: &World, error: String) -> String {
    super::shared_guild_experience::reject_source(world, error.clone());
    error
}

pub(super) fn enqueue(world: &mut World, event: DefaultNpcEvent) -> Result<(), String> {
    event.validate().map_err(|e| reject(world,e))?;
    if !is_in_world(world) || world.resource::<SessionResource>().account_id.is_none() {
        return Err(reject(world, "default NPC hook requires an authenticated active character".into()));
    }
    world.init_resource::<DefaultNpcEventQueue>();
    let state = &world.resource::<DefaultNpcEventQueue>().snapshot;
    if state.pending.len() >= MAX_PENDING {
        return Err(reject(world, "default NPC source queue overflow".into()));
    }
    let sequence = state.next_sequence.checked_add(1)
        .ok_or_else(|| reject(world, "default NPC source queue sequence exhausted".into()))?;
    let mut queue = world.resource_mut::<DefaultNpcEventQueue>();
    queue.snapshot.next_sequence = sequence;
    queue.snapshot.pending.push(QueuedDefaultNpcEvent { sequence,event, enter_map:None });
    Ok(())
}

fn source_entry_binding(ticket: &DefaultNpcEnterMapTicket) -> bool {
    let Some(source) = mir2_game_data::crystal_map_respawns_ref(&ticket.source_map_file_name) else { return false; };
    source.movements.iter().any(|movement| movement.need_move && !movement.need_hole
        && movement.source == ticket.source_position && movement.destination == ticket.destination_position
        && mir2_game_data::crystal_map_respawns_by_index(movement.map_index)
            .is_some_and(|map| map.map_file_name.eq_ignore_ascii_case(&ticket.destination_map_file_name)))
}

pub(super) fn enqueue_map_coord_with_destination(
    world: &mut World, source_map_file_name: &str, source_position: Point,
    destination_map_file_name: &str, destination_position: Point,
) -> Result<(), String> {
    let session = world.resource::<SessionResource>();
    let ticket = DefaultNpcEnterMapTicket {
        account_id:session.account_id.clone().ok_or("default NPC entry account is missing")?,
        character_index:session.selected_character.as_ref().ok_or("default NPC entry character is missing")?.index,
        object_id:current_player_object_id(world).ok_or("default NPC entry object is missing")?,
        source_map_file_name:source_map_file_name.into(), source_position:source_position.clone(),
        destination_map_file_name:destination_map_file_name.into(), destination_position,
        event_sequence:0,
    };
    if !source_entry_binding(&ticket) {
        return Err(reject(world,"default NPC entry destination is not an original NeedMove binding".into()));
    }
    enqueue(world,DefaultNpcEvent::MapCoord { map_file_name:source_map_file_name.into(),position:source_position })?;
    let mut queue = world.resource_mut::<DefaultNpcEventQueue>();
    let last = queue.snapshot.pending.last_mut().expect("just enqueued coordinate event");
    last.enter_map = Some(DefaultNpcEnterMapTicket { event_sequence:last.sequence, ..ticket });
    Ok(())
}

/// EnterMap only consumes the destination attached to the hook currently being
/// drained. It cannot reuse a previous hook, a menu, or an obsolete position.
pub(super) fn consume_enter_map(world: &mut World) -> Result<Vec<ServerPacket>, String> {
    let Some(queue) = world.get_resource::<DefaultNpcEventQueue>() else { return Ok(vec![]); };
    let Some(sequence) = queue.active_sequence else { return Ok(vec![]); };
    let Some(ticket) = queue.snapshot.pending.first().filter(|event| event.sequence == sequence)
        .and_then(|event| event.enter_map.clone()) else { return Ok(vec![]); };
    let session = world.resource::<SessionResource>();
    if session.account_id.as_deref() != Some(ticket.account_id.as_str())
        || session.selected_character.as_ref().map(|c| c.index) != Some(ticket.character_index)
        || current_player_object_id(world) != Some(ticket.object_id) || !source_entry_binding(&ticket) {
        return Err(reject(world,"default NPC entry ticket actor or original source changed".into()));
    }
    world.resource_mut::<DefaultNpcEventQueue>().snapshot.pending[0].enter_map = None;
    if !world.resource::<MapRuntimeResource>().current_map.file_name.eq_ignore_ascii_case(&ticket.source_map_file_name)
        || super::components::player_entity(world).and_then(|player| super::components::entity_position(world,player))
            .unwrap_or_else(|| world.resource::<PlayerRuntimeResource>().player_position.clone()) != ticket.source_position {
        return Ok(vec![]);
    }
    let Some(source) = mir2_game_data::crystal_map_respawns_ref(&ticket.source_map_file_name) else { return Ok(vec![]); };
    let movement = source.movements.iter().find(|movement| movement.need_move && !movement.need_hole
        && movement.source == ticket.source_position && movement.destination == ticket.destination_position)
        .ok_or("default NPC original entry movement disappeared")?;
    if !super::map::conquest_movement_allowed(world,movement.conquest_index) { return Ok(vec![]); }
    let map = super::npc_script::crystal_npc_move_map_information(world,&ticket.destination_map_file_name);
    let direction = world.resource::<PlayerRuntimeResource>().player_direction;
    Ok(super::map::relocate_player_to_map(world,map,ticket.destination_position,direction,None))
}

pub(super) fn is_active_map_coord(map_file_name: &str, position: &Point) -> bool {
    let label = DefaultNpcEvent::MapCoord { map_file_name:map_file_name.into(), position:position.clone() }.label();
    expanded_script().is_ok_and(|script| script.sections.iter().any(|section| section.label.eq_ignore_ascii_case(&label)))
}
/// Only accepted server movement produces this event. A run observes each
/// crossed cell, while an entry ticket is bound to the actual entrance cell.
pub(super) fn observe_movement(world: &mut World, previous: &Point, current: &Point) {
    if previous == current || !is_in_world(world)
        || (current.x - previous.x).abs().max((current.y - previous.y).abs()) > 2 { return; }
    let map_file_name = world.resource::<MapRuntimeResource>().current_map.file_name.clone();
    let mut point = previous.clone();
    for _ in 0..2 {
        if point == *current { break; }
        point.x += (current.x - point.x).signum(); point.y += (current.y - point.y).signum();
        if is_active_map_coord(&map_file_name,&point) {
            let _ = enqueue_coordinate_source(world,&map_file_name,point.clone());
        }
    }
}
pub(super) fn enqueue_coordinate_source(world: &mut World, map_file_name: &str, point: Point) -> Result<(),String> {
    let movement = mir2_game_data::crystal_map_respawns_ref(map_file_name)
        .and_then(|map| map.movements.iter().find(|movement| movement.need_move && !movement.need_hole && movement.source == point)).cloned();
    if let Some(movement) = movement {
        if !super::map::conquest_movement_allowed(world,movement.conquest_index) { return Ok(()); }
        let destination = mir2_game_data::crystal_map_respawns_by_index(movement.map_index).ok_or("default NPC entry map missing")?;
        enqueue_map_coord_with_destination(world,map_file_name,point,&destination.map_file_name,movement.destination)
    } else { enqueue(world,DefaultNpcEvent::MapCoord { map_file_name:map_file_name.into(),position:point }) }
}
pub(super) fn has_coordinate_ticket(world:&World,map_file_name:&str,point:&Point)->bool {
    capture(world).pending.iter().any(|event| matches!(&event.event,DefaultNpcEvent::MapCoord { map_file_name:map,position } if map.eq_ignore_ascii_case(map_file_name) && position == point))
}
pub(super) fn is_custom_command(name: &str) -> bool {
    let label = DefaultNpcEvent::CustomCommand { name:name.into() }.label();
    expanded_script().is_ok_and(|script| script.sections.iter().any(|section| section.label.eq_ignore_ascii_case(&label)))
}

fn execute(world: &mut World, script: &CrystalNpcScript, label: &str) -> Result<Vec<ServerPacket>, String> {
    if !script.sections.iter().any(|section| section.label.eq_ignore_ascii_case(label)) {
        // Source hooks with no matching page are legitimate no-ops. In
        // particular, PARAM1 placeholders are never invented quest IDs.
        return Ok(vec![ServerPacket::NPCUpdate { npc_id:0 }]);
    }
    let context = NpcInteractionContext {
        object_id:0, name:"DefaultNPC".into(), name_key:None,
        position:world.resource::<PlayerRuntimeResource>().player_position.clone(),
        quest_ids:vec![], script_key:Some(SCRIPT_KEY.into()), args:vec![], input:None,
    };
    let diagnostics = world.resource::<NpcStateResource>().npc_script_diagnostics.len();
    let (result, page) = run_crystal_default_npc_script(world,&context,script,label)
        .ok_or_else(|| "default NPC original page did not execute".to_string())?;
    if world.resource::<NpcStateResource>().npc_script_diagnostics.len() != diagnostics {
        return Err("default NPC original page contains unsupported or invalid execution".into());
    }
    let mut packets = vec![ServerPacket::NPCUpdate { npc_id:0 }];
    packets.extend(result.packets);
    if let Some(dialog) = result.dialog {
        set_dialog(world,dialog);
        packets.push(ServerPacket::NPCResponse { page });
    } else {
        world.resource_mut::<NpcStateResource>().active_npc_service = None;
        dismiss_dialog(world);
        packets.push(ServerPacket::NPCResponse { page:vec![] });
    }
    Ok(packets)
}

pub(super) fn dispatch(world: &mut World) -> Result<Vec<ServerPacket>, String> {
    if !has_pending(world) { return Ok(vec![]); }
    if world.resource::<DefaultNpcEventQueue>().draining { return Ok(vec![]); }
    if !is_in_world(world) || world.resource::<SessionResource>().account_id.is_none() {
        return Err(reject(world, "default NPC queued hook has no authenticated active character".into()));
    }
    let script = expanded_script().map_err(|e| reject(world,e))?;
    world.resource_mut::<DefaultNpcEventQueue>().draining = true;
    let mut packets = Vec::new();
    let result = (|| {
        for _ in 0..DRAIN_BUDGET {
            let queued = world.resource::<DefaultNpcEventQueue>().snapshot.pending.first().cloned();
            let Some(queued) = queued else { break; };
            world.resource_mut::<DefaultNpcEventQueue>().active_sequence = Some(queued.sequence);
            packets.extend(execute(world,script,&queued.event.label())?);
            let mut queue = world.resource_mut::<DefaultNpcEventQueue>();
            if queue.snapshot.pending.first().map(|e| e.sequence) != Some(queued.sequence) {
                return Err("default NPC queue changed the event currently being executed".into());
            }
            queue.snapshot.pending.remove(0);
            queue.snapshot.committed_sequence = queued.sequence;
        }
        Ok(packets)
    })();
    world.resource_mut::<DefaultNpcEventQueue>().draining = false;
    world.resource_mut::<DefaultNpcEventQueue>().active_sequence = None;
    result.map_err(|e| reject(world,e))
}

/// A normal client may follow only links from its currently visible default
/// NPC page. It cannot turn a raw key into a trusted Login/UseItem/etc event.
pub(super) fn follow_up(world: &mut World, key: &str) -> Result<Option<Vec<ServerPacket>>, String> {
    let key = normalize_crystal_npc_label(key);
    let permitted = world.resource::<NpcStateResource>().active_npc_dialog.as_ref()
        .filter(|dialog| dialog.npc_object_id == 0)
        .is_some_and(|dialog| dialog.links.iter().any(|link| link.target.eq_ignore_ascii_case(&key)));
    if !permitted || !is_in_world(world) || world.resource::<SessionResource>().account_id.is_none() {
        return Ok(None);
    }
    if key.eq_ignore_ascii_case("@Exit") {
        dismiss_dialog(world);
        return Ok(Some(vec![ServerPacket::NPCResponse { page:vec![] }]));
    }
    let script = expanded_script().map_err(|e| reject(world,e))?;
    execute(world,script,&key).map(Some).map_err(|e| reject(world,e))
}
