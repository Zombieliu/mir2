use bevy_ecs::prelude::{Resource, World};
use mir2_game_data::{DoorMapCellTemplate, LanguageCode, MapBounds};
use mir2_protocol::{IntelligentCreatureRules, MapInformation, MirDirection, Point, Spell};
use serde::{Deserialize, Serialize};

use crate::config::{CharacterRecord, SimulationConfig, Stage5SystemsState};

use super::buffs::BuffState;
use super::combat::PendingCombatAction;
use super::components::PlayerVitals;
use super::equipment::EquipmentState;
use super::items::ItemState;
use super::monsters::PendingMonsterSpawnAction;
use super::npc::{
    ActiveNpcDialogState, ActiveNpcServiceState, NpcBuyBackState, NpcFlagState, NpcUsedGoodsState,
};
use super::npc_script::{CrystalNpcSavedValue, CrystalNpcScriptDiagnostic};
use super::quests::QuestState;
use super::skills::SkillState;

use std::collections::{BTreeMap, BTreeSet};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn current_language(world: &World) -> LanguageCode {
    world.resource::<SessionResource>().language
}

pub(super) fn is_in_world(world: &World) -> bool {
    world
        .resource::<SessionResource>()
        .selected_character
        .is_some()
}

pub(super) fn runtime_tick(world: &World) -> u64 {
    world.resource::<RuntimeClockResource>().tick
}

pub(super) fn intelligent_creature_default_rules(pet_type: u8) -> IntelligentCreatureRules {
    let mut rules = IntelligentCreatureRules {
        minimal_fullness: 1000,
        mouse_pickup_enabled: false,
        mouse_pickup_range: 0,
        auto_pickup_enabled: false,
        auto_pickup_range: 0,
        semi_auto_pickup_enabled: false,
        semi_auto_pickup_range: 0,
        can_produce_blackstone: false,
    };

    match pet_type {
        // BabyPig
        0 => {
            rules.semi_auto_pickup_enabled = true;
            rules.semi_auto_pickup_range = 3;
            rules.minimal_fullness = 4000;
        }
        // Chick
        1 => {
            rules.mouse_pickup_enabled = true;
            rules.mouse_pickup_range = 11;
            rules.auto_pickup_enabled = true;
            rules.auto_pickup_range = 7;
            rules.semi_auto_pickup_enabled = true;
            rules.semi_auto_pickup_range = 7;
            rules.can_produce_blackstone = true;
        }
        // Kitten
        2 => {
            rules.semi_auto_pickup_enabled = true;
            rules.semi_auto_pickup_range = 3;
            rules.minimal_fullness = 6000;
        }
        // BabySkeleton
        3 => {
            rules.mouse_pickup_enabled = true;
            rules.mouse_pickup_range = 11;
            rules.auto_pickup_enabled = true;
            rules.auto_pickup_range = 7;
            rules.semi_auto_pickup_enabled = true;
            rules.semi_auto_pickup_range = 7;
            rules.can_produce_blackstone = true;
        }
        // Baekdon
        4 => {
            rules.mouse_pickup_enabled = true;
            rules.mouse_pickup_range = 11;
            rules.auto_pickup_enabled = true;
            rules.auto_pickup_range = 7;
            rules.semi_auto_pickup_enabled = true;
            rules.semi_auto_pickup_range = 7;
            rules.can_produce_blackstone = true;
        }
        // Wimaen
        5 => {
            rules.mouse_pickup_enabled = true;
            rules.mouse_pickup_range = 7;
            rules.auto_pickup_enabled = true;
            rules.auto_pickup_range = 5;
            rules.semi_auto_pickup_enabled = true;
            rules.semi_auto_pickup_range = 5;
            rules.minimal_fullness = 5000;
        }
        // BlackKitten
        6 => {
            rules.mouse_pickup_enabled = true;
            rules.mouse_pickup_range = 7;
            rules.auto_pickup_enabled = true;
            rules.auto_pickup_range = 5;
            rules.semi_auto_pickup_enabled = true;
            rules.semi_auto_pickup_range = 5;
            rules.minimal_fullness = 5000;
        }
        // BabyDragon
        7 => {
            rules.mouse_pickup_enabled = true;
            rules.mouse_pickup_range = 7;
            rules.auto_pickup_enabled = true;
            rules.auto_pickup_range = 5;
            rules.semi_auto_pickup_enabled = true;
            rules.semi_auto_pickup_range = 5;
            rules.minimal_fullness = 7000;
        }
        // OlympicFlame
        8 => {
            rules.mouse_pickup_enabled = true;
            rules.mouse_pickup_range = 11;
            rules.auto_pickup_enabled = true;
            rules.auto_pickup_range = 11;
            rules.semi_auto_pickup_enabled = true;
            rules.semi_auto_pickup_range = 11;
            rules.can_produce_blackstone = true;
        }
        // BabySnowMan
        9 => {
            rules.mouse_pickup_enabled = true;
            rules.mouse_pickup_range = 11;
            rules.auto_pickup_enabled = true;
            rules.auto_pickup_range = 11;
            rules.semi_auto_pickup_enabled = true;
            rules.semi_auto_pickup_range = 11;
            rules.can_produce_blackstone = true;
        }
        // Frog
        10 => {
            rules.mouse_pickup_enabled = true;
            rules.mouse_pickup_range = 11;
            rules.auto_pickup_enabled = true;
            rules.auto_pickup_range = 11;
            rules.semi_auto_pickup_enabled = true;
            rules.semi_auto_pickup_range = 11;
            rules.can_produce_blackstone = true;
        }
        // BabyMonkey
        11 => {
            rules.mouse_pickup_enabled = true;
            rules.mouse_pickup_range = 11;
            rules.auto_pickup_enabled = true;
            rules.auto_pickup_range = 11;
            rules.semi_auto_pickup_enabled = true;
            rules.semi_auto_pickup_range = 11;
            rules.can_produce_blackstone = true;
        }
        // AngryBird
        12 => {
            rules.mouse_pickup_enabled = true;
            rules.mouse_pickup_range = 11;
            rules.auto_pickup_enabled = true;
            rules.auto_pickup_range = 11;
            rules.semi_auto_pickup_enabled = true;
            rules.semi_auto_pickup_range = 11;
            rules.can_produce_blackstone = true;
        }
        // Foxey
        13 => {
            rules.mouse_pickup_enabled = true;
            rules.mouse_pickup_range = 11;
            rules.auto_pickup_enabled = true;
            rules.auto_pickup_range = 11;
            rules.semi_auto_pickup_enabled = true;
            rules.semi_auto_pickup_range = 11;
            rules.can_produce_blackstone = true;
        }
        // MedicalRat
        14 => {
            rules.mouse_pickup_enabled = true;
            rules.mouse_pickup_range = 11;
            rules.auto_pickup_enabled = true;
            rules.auto_pickup_range = 11;
            rules.semi_auto_pickup_enabled = true;
            rules.semi_auto_pickup_range = 11;
            rules.can_produce_blackstone = true;
        }
        _ => {}
    }

    rules
}

pub(super) fn set_runtime_tick(world: &mut World, tick: u64) {
    world.resource_mut::<RuntimeClockResource>().tick = tick;
    if let Some(mut timing) = world.get_resource_mut::<PlayerActionTimingResource>() {
        timing.reset();
    }
    if let Some(mut movement) = world.get_resource_mut::<PlayerMovementTimingResource>() {
        movement.reset();
    }
}

pub(super) fn advance_runtime_tick(world: &mut World) -> u64 {
    let mut clock = world.resource_mut::<RuntimeClockResource>();
    clock.tick += 1;
    clock.tick
}

#[derive(Resource, Debug, Clone)]
pub(super) struct RuntimeConfigResource {
    pub(super) config: SimulationConfig,
}

impl RuntimeConfigResource {
    pub(super) fn new(config: &SimulationConfig) -> Self {
        Self {
            config: config.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PlayerActionKind {
    Move,
    Attack,
    Spell,
}

#[derive(Resource, Debug, Clone)]
pub(super) struct PlayerActionTimingResource {
    next_move_tick: u64,
    next_attack_tick: u64,
    next_spell_tick: u64,
    next_move_at_ms: u64,
    next_attack_at_ms: u64,
    next_spell_at_ms: u64,
}

impl PlayerActionTimingResource {
    pub(super) fn new() -> Self {
        Self {
            next_move_tick: 0,
            next_attack_tick: 0,
            next_spell_tick: 0,
            next_move_at_ms: 0,
            next_attack_at_ms: 0,
            next_spell_at_ms: 0,
        }
    }

    fn reset(&mut self) {
        self.next_move_tick = 0;
        self.next_attack_tick = 0;
        self.next_spell_tick = 0;
        self.next_move_at_ms = 0;
        self.next_attack_at_ms = 0;
        self.next_spell_at_ms = 0;
    }

    fn next_tick_for(&self, kind: PlayerActionKind) -> u64 {
        match kind {
            PlayerActionKind::Move => self.next_move_tick,
            PlayerActionKind::Attack => self.next_attack_tick,
            PlayerActionKind::Spell => self.next_spell_tick,
        }
    }

    fn set_next_tick_for(&mut self, kind: PlayerActionKind, tick: u64) {
        match kind {
            PlayerActionKind::Move => self.next_move_tick = tick,
            PlayerActionKind::Attack => self.next_attack_tick = tick,
            PlayerActionKind::Spell => self.next_spell_tick = tick,
        }
    }

    fn next_at_ms_for(&self, kind: PlayerActionKind) -> u64 {
        match kind {
            PlayerActionKind::Move => self.next_move_at_ms,
            PlayerActionKind::Attack => self.next_attack_at_ms,
            PlayerActionKind::Spell => self.next_spell_at_ms,
        }
    }

    fn set_next_at_ms_for(&mut self, kind: PlayerActionKind, at_ms: u64) {
        match kind {
            PlayerActionKind::Move => self.next_move_at_ms = at_ms,
            PlayerActionKind::Attack => self.next_attack_at_ms = at_ms,
            PlayerActionKind::Spell => self.next_spell_at_ms = at_ms,
        }
    }
}

const CRYSTAL_PACKET_ACTION_TICK_MS: u64 = 300;

fn current_wall_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

pub(super) fn crystal_packet_action_ready(world: &World, kind: PlayerActionKind) -> bool {
    let current_tick = runtime_tick(world);
    let timing = world.resource::<PlayerActionTimingResource>();
    current_tick >= timing.next_tick_for(kind)
        || current_wall_time_ms() >= timing.next_at_ms_for(kind)
}

pub(super) fn mark_crystal_packet_action(
    world: &mut World,
    kind: PlayerActionKind,
    delay_ticks: u64,
) {
    let next_tick = runtime_tick(world).saturating_add(delay_ticks.max(1));
    let next_at_ms =
        current_wall_time_ms().saturating_add(delay_ticks.max(1) * CRYSTAL_PACKET_ACTION_TICK_MS);
    world
        .resource_mut::<PlayerActionTimingResource>()
        .set_next_tick_for(kind, next_tick);
    world
        .resource_mut::<PlayerActionTimingResource>()
        .set_next_at_ms_for(kind, next_at_ms);
}

pub(super) fn crystal_packet_move_delay_ticks(_running: bool) -> u64 {
    2
}

pub(super) fn crystal_packet_attack_delay_ticks() -> u64 {
    2
}

pub(super) fn crystal_packet_spell_delay_ticks() -> u64 {
    1
}

const CRYSTAL_PLAYER_CELL_TIME_MS: u64 = 500;
const CRYSTAL_STEP_COUNTER_RESET_GRACE_MS: u64 = 700;

#[derive(Resource, Debug, Clone)]
pub(super) struct PlayerMovementTimingResource {
    crystal_step_counter: u8,
    crystal_step_counter_until_ms: u64,
}

impl PlayerMovementTimingResource {
    pub(super) fn new() -> Self {
        Self {
            crystal_step_counter: 0,
            crystal_step_counter_until_ms: 0,
        }
    }

    fn reset(&mut self) {
        self.crystal_step_counter = 0;
        self.crystal_step_counter_until_ms = 0;
    }

    fn refresh(&mut self, now_ms: u64) {
        if self.crystal_step_counter_until_ms < now_ms {
            self.reset();
        }
    }
}

pub(super) fn reset_crystal_player_movement_timing(world: &mut World) {
    if let Some(mut timing) = world.get_resource_mut::<PlayerMovementTimingResource>() {
        timing.reset();
    }
}

pub(super) fn crystal_player_can_run(world: &mut World) -> bool {
    let now_ms = current_wall_time_ms();
    let mut timing = world.resource_mut::<PlayerMovementTimingResource>();
    timing.refresh(now_ms);
    timing.crystal_step_counter > 0
}

pub(super) fn mark_crystal_player_move(world: &mut World, running: bool) {
    let now_ms = current_wall_time_ms();
    let mut timing = world.resource_mut::<PlayerMovementTimingResource>();
    timing.refresh(now_ms);
    if !running {
        timing.crystal_step_counter = timing.crystal_step_counter.saturating_add(1).max(1);
    }
    if timing.crystal_step_counter > 0 {
        timing.crystal_step_counter_until_ms = now_ms
            .saturating_add(CRYSTAL_PLAYER_CELL_TIME_MS)
            .saturating_add(CRYSTAL_STEP_COUNTER_RESET_GRACE_MS);
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PendingMovementCommand {
    pub(super) direction: MirDirection,
    pub(super) running: bool,
}

pub(super) fn queue_crystal_movement_retry(
    world: &mut World,
    direction: MirDirection,
    running: bool,
) {
    world
        .resource_mut::<RuntimeQueueResource>()
        .pending_movement_command = Some(PendingMovementCommand { direction, running });
}

pub(super) fn take_crystal_movement_retry_if_ready(
    world: &mut World,
) -> Option<PendingMovementCommand> {
    if !crystal_packet_action_ready(world, PlayerActionKind::Move) {
        return None;
    }

    world
        .resource_mut::<RuntimeQueueResource>()
        .pending_movement_command
        .take()
}

#[derive(Resource, Debug, Clone)]
pub(super) struct SessionResource {
    pub(super) language: LanguageCode,
    pub(super) version_verified: bool,
    pub(super) account_id: Option<String>,
    pub(super) characters: Vec<CharacterRecord>,
    pub(super) selected_character: Option<CharacterRecord>,
}

impl SessionResource {
    pub(super) fn new(config: &SimulationConfig) -> Self {
        Self {
            language: LanguageCode::English,
            version_verified: false,
            account_id: None,
            characters: vec![config.default_character.clone()],
            selected_character: None,
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub(super) struct PlayerRuntimeResource {
    pub(super) player_position: Point,
    pub(super) player_direction: MirDirection,
    pub(super) player_vitals: PlayerVitals,
    pub(super) experience: i64,
    pub(super) max_experience: i64,
    pub(super) gold: u32,
    pub(super) credit: u32,
    pub(super) pk_points: i32,
    pub(super) chat_banned: bool,
    pub(super) chat_ban_until_ms: Option<u64>,
    pub(super) chat_next_allowed_at_ms: u64,
    pub(super) chat_spam_tick: u8,
}

impl PlayerRuntimeResource {
    pub(super) fn new(config: &SimulationConfig) -> Self {
        let (default_max_hp, default_mp) = crate::config::crystal_base_vitals(
            config.default_character.class,
            config.default_character.level,
        );
        Self {
            player_position: config.spawn.clone(),
            player_direction: MirDirection::Down,
            player_vitals: PlayerVitals {
                hp: default_max_hp,
                max_hp: default_max_hp,
                mp: default_mp,
            },
            experience: 0,
            max_experience: 100,
            gold: 0,
            credit: 0,
            pk_points: 0,
            chat_banned: false,
            chat_ban_until_ms: None,
            chat_next_allowed_at_ms: 0,
            chat_spam_tick: 0,
        }
    }
}

/// Runtime state of a single logical door (all map cells that share one door
/// index, deduplicated exactly like Crystal's `Map.AddDoor`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DoorRuntime {
    pub(crate) index: u8,
    pub(crate) cells: Vec<(i32, i32)>,
    /// `None` means closed. `Some(tick)` means the door is open and should
    /// auto-close once the runtime clock reaches this tick — Crystal closes
    /// doors 5000 ms after they were opened (`Map.Process`).
    pub(crate) close_at_tick: Option<u64>,
}

/// All doors on the current map, mirroring Crystal's `Map.Doors` list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DoorRegistry {
    pub(crate) doors: Vec<DoorRuntime>,
}

impl DoorRegistry {
    /// Build the registry from parsed door cells, grouping cells by door index
    /// (`index & 0x7F`) so opening one index opens every cell of that door —
    /// matching Crystal's `AddDoor` dedup.
    pub(crate) fn from_templates(templates: &[DoorMapCellTemplate]) -> Self {
        let mut doors: Vec<DoorRuntime> = Vec::new();
        for template in templates {
            let index = template.index & 0x7F;
            if let Some(existing) = doors.iter_mut().find(|door| door.index == index) {
                if !existing.cells.contains(&(template.x, template.y)) {
                    existing.cells.push((template.x, template.y));
                }
            } else {
                doors.push(DoorRuntime {
                    index,
                    cells: vec![(template.x, template.y)],
                    close_at_tick: None,
                });
            }
        }
        Self { doors }
    }

    pub(crate) fn find_mut(&mut self, index: u8) -> Option<&mut DoorRuntime> {
        let index = index & 0x7F;
        self.doors.iter_mut().find(|door| door.index == index)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.doors.is_empty()
    }
}

#[derive(Resource, Debug, Clone)]
pub(super) struct MapRuntimeResource {
    pub(super) current_map: MapInformation,
    pub(super) map_region_bounds: MapBounds,
    pub(super) blocked_cells: BTreeSet<(i32, i32)>,
    pub(super) closed_door_cells: BTreeSet<(i32, i32)>,
    pub(super) doors: DoorRegistry,
    pub(super) conquest_wars: BTreeMap<i32, bool>,
}

impl MapRuntimeResource {
    pub(super) fn new(
        config: &SimulationConfig,
        map_region_bounds: MapBounds,
        blocked_cells: BTreeSet<(i32, i32)>,
        closed_door_cells: BTreeSet<(i32, i32)>,
        doors: DoorRegistry,
    ) -> Self {
        Self {
            current_map: config.map.clone(),
            map_region_bounds,
            blocked_cells,
            closed_door_cells,
            doors,
            conquest_wars: config.conquest_wars.clone(),
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub(super) struct InventoryResource {
    pub(super) inventory_items: Vec<ItemState>,
    pub(super) belt_items: Vec<ItemState>,
    pub(super) storage_items: Vec<ItemState>,
    pub(super) equipment_items: Vec<EquipmentState>,
    pub(super) storage_size: u16,
    pub(super) has_expanded_storage: bool,
    pub(super) expanded_storage_expiry_time_binary_datetime: i64,
    pub(super) expanded_storage_expiry_notice_pending: bool,
    pub(super) storage_unlocked: bool,
    pub(super) storage_sent: bool,
    pub(super) storage_has_password: bool,
    pub(super) storage_password_last_set_binary_datetime: i64,
}

impl InventoryResource {
    pub(super) fn new(base_storage_slots: u16) -> Self {
        Self {
            inventory_items: Vec::new(),
            belt_items: Vec::new(),
            storage_items: Vec::new(),
            equipment_items: Vec::new(),
            storage_size: base_storage_slots,
            has_expanded_storage: false,
            expanded_storage_expiry_time_binary_datetime: 0,
            expanded_storage_expiry_notice_pending: false,
            storage_unlocked: true,
            storage_sent: false,
            storage_has_password: false,
            storage_password_last_set_binary_datetime: 0,
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub(super) struct HeroInventoryResource {
    pub(super) items: Vec<ItemState>,
}

impl HeroInventoryResource {
    pub(super) fn new() -> Self {
        Self { items: Vec::new() }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct ItemRentalRecordState {
    pub(super) item_id: u64,
    pub(super) item_name: String,
    pub(super) renting_player_name: String,
    pub(super) item_return_date_binary_datetime: i64,
}

#[derive(Debug, Clone)]
pub(super) struct ActiveItemRentalState {
    pub(super) partner_name: String,
    pub(super) fee: u32,
    pub(super) days: u32,
    pub(super) deposited_item: Option<ItemState>,
    pub(super) deposited_from: Option<i32>,
    pub(super) gold_locked: bool,
    pub(super) item_locked: bool,
}

#[derive(Resource, Debug, Clone)]
pub(super) struct ItemRentalResource {
    pub(super) rented_items: Vec<ItemRentalRecordState>,
    pub(super) has_rented_item: bool,
    pub(super) active: Option<ActiveItemRentalState>,
    pub(super) default_partner_name: String,
}

impl ItemRentalResource {
    pub(super) fn new() -> Self {
        Self {
            rented_items: Vec::new(),
            has_rented_item: false,
            active: None,
            default_partner_name: "Crystal Partner".to_string(),
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub(super) struct FishingResource {
    pub(super) fishing: bool,
    pub(super) auto_cast: bool,
    pub(super) progress_percent: i32,
    pub(super) chance_percent: i32,
    pub(super) found_fish: bool,
    pub(super) fishing_attribute: i8,
    pub(super) rod_has_hook: bool,
    pub(super) rod_has_reel: bool,
    pub(super) chance_counter: i32,
    pub(super) slot_items: Vec<Option<ItemState>>,
}

impl FishingResource {
    pub(super) fn new() -> Self {
        Self {
            fishing: false,
            auto_cast: false,
            progress_percent: 0,
            chance_percent: 0,
            found_fish: false,
            fishing_attribute: 0,
            rod_has_hook: true,
            rod_has_reel: true,
            chance_counter: 0,
            slot_items: vec![None; 5],
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub(super) struct QuestResource {
    pub(super) quests: Vec<QuestState>,
}

impl QuestResource {
    pub(super) fn new() -> Self {
        Self { quests: Vec::new() }
    }
}

#[derive(Resource, Debug, Clone)]
pub(super) struct SkillResource {
    pub(super) skills: Vec<SkillState>,
    pub(super) spell_toggles: Vec<(Spell, bool)>,
    pub(super) mental_state: u8,
    pub(super) slaying_armed: bool,
    pub(super) fatal_sword_armed: bool,
    pub(super) mp_eater_armed: bool,
    pub(super) mp_eater_count: u16,
    pub(super) hemorrhage_armed: bool,
    pub(super) hemorrhage_attack_count: u16,
}

impl SkillResource {
    pub(super) fn new() -> Self {
        Self {
            skills: Vec::new(),
            spell_toggles: Vec::new(),
            mental_state: 0,
            slaying_armed: false,
            fatal_sword_armed: false,
            mp_eater_armed: false,
            mp_eater_count: 0,
            hemorrhage_armed: false,
            hemorrhage_attack_count: 0,
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub(super) struct BuffResource {
    pub(super) buffs: Vec<BuffState>,
}

impl BuffResource {
    pub(super) fn new() -> Self {
        Self { buffs: Vec::new() }
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub(super) struct ElementalResource {
    pub(super) has_elemental: bool,
    pub(super) elements_level: u32,
}

impl ElementalResource {
    pub(super) fn new() -> Self {
        Self {
            has_elemental: false,
            elements_level: 0,
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub(super) struct MountResource {
    pub(super) mount_type: i16,
    pub(super) riding_mount: bool,
    pub(super) has_saddle: bool,
    pub(super) has_reins: bool,
}

impl MountResource {
    pub(super) fn new() -> Self {
        Self {
            mount_type: -1,
            riding_mount: false,
            has_saddle: true,
            has_reins: false,
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub(super) struct NpcStateResource {
    pub(super) npc_flags: Vec<NpcFlagState>,
    pub(super) npc_variables: Vec<(String, String)>,
    pub(super) npc_saved_values: Vec<CrystalNpcSavedValue>,
    pub(super) npc_script_diagnostics: Vec<CrystalNpcScriptDiagnostic>,
    pub(super) npc_buy_back_items: Vec<NpcBuyBackState>,
    pub(super) npc_used_goods_items: Vec<NpcUsedGoodsState>,
    pub(super) active_npc_dialog: Option<ActiveNpcDialogState>,
    pub(super) active_npc_service: Option<ActiveNpcServiceState>,
}

impl NpcStateResource {
    pub(super) fn new() -> Self {
        Self {
            npc_flags: Vec::new(),
            npc_variables: Vec::new(),
            npc_saved_values: Vec::new(),
            npc_script_diagnostics: Vec::new(),
            npc_buy_back_items: Vec::new(),
            npc_used_goods_items: Vec::new(),
            active_npc_dialog: None,
            active_npc_service: None,
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub(super) struct RuntimeQueueResource {
    pub(super) pending_combat_actions: Vec<PendingCombatAction>,
    pub(super) pending_monster_spawns: Vec<PendingMonsterSpawnAction>,
    pub(super) pending_ground_spell_actions: Vec<PendingGroundSpellAction>,
    pub(super) pending_movement_command: Option<PendingMovementCommand>,
}

impl RuntimeQueueResource {
    pub(super) fn new() -> Self {
        Self {
            pending_combat_actions: Vec::new(),
            pending_monster_spawns: Vec::new(),
            pending_ground_spell_actions: Vec::new(),
            pending_movement_command: None,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct PendingGroundSpellAction {
    pub(super) spell: Spell,
    pub(super) caster_object_id: u32,
    pub(super) locations: Vec<Point>,
    pub(super) damage: i32,
    pub(super) next_tick: u64,
    pub(super) expires_at_tick: u64,
    pub(super) tick_interval: u64,
}

#[derive(Resource, Debug, Clone)]
pub(super) struct Stage5SystemsResource {
    pub(super) stage5_systems: Stage5SystemsState,
}

impl Stage5SystemsResource {
    pub(super) fn new() -> Self {
        Self {
            stage5_systems: Stage5SystemsState::default(),
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub(super) struct GroupResource {
    pub(super) group_member_object_ids: Vec<u32>,
}

impl GroupResource {
    pub(super) fn new(config: &SimulationConfig) -> Self {
        Self {
            group_member_object_ids: config.group_member_object_ids.clone(),
        }
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub(super) struct PlayerPermissionResource {
    pub(super) unlock_curse: bool,
    pub(super) free_map_shout: bool,
    pub(super) free_server_shout: bool,
}

impl PlayerPermissionResource {
    pub(super) fn new() -> Self {
        Self {
            unlock_curse: false,
            free_map_shout: false,
            free_server_shout: false,
        }
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub(super) struct PotionRecoveryResource {
    pub(super) pending_pot_health_amount: i32,
    pub(super) pending_pot_mana_amount: i32,
    pub(super) hero_pending_pot_health_amount: i32,
    pub(super) hero_pending_pot_mana_amount: i32,
}

impl PotionRecoveryResource {
    pub(super) fn new() -> Self {
        Self {
            pending_pot_health_amount: 0,
            pending_pot_mana_amount: 0,
            hero_pending_pot_health_amount: 0,
            hero_pending_pot_mana_amount: 0,
        }
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub(super) struct RuntimeClockResource {
    pub(super) tick: u64,
}

impl RuntimeClockResource {
    pub(super) fn new() -> Self {
        Self { tick: 0 }
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub(super) struct ObjectIdAllocatorResource {
    pub(super) next_drop_object_id: u32,
    pub(super) next_runtime_monster_object_id: u32,
}

impl ObjectIdAllocatorResource {
    pub(super) fn new() -> Self {
        Self {
            next_drop_object_id: 5000,
            next_runtime_monster_object_id: 80_000,
        }
    }

    pub(super) fn reset(&mut self) {
        self.next_drop_object_id = 5000;
        self.next_runtime_monster_object_id = 80_000;
    }

    pub(super) fn next_drop_id(&mut self) -> u32 {
        let id = self.next_drop_object_id;
        self.next_drop_object_id += 1;
        id
    }

    pub(super) fn next_runtime_monster_id(&mut self) -> u32 {
        let id = self.next_runtime_monster_object_id;
        self.next_runtime_monster_object_id += 1;
        id
    }
}
