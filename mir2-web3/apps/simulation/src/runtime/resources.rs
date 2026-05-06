use bevy_ecs::prelude::{Resource, World};
use mir2_game_data::{LanguageCode, MapBounds};
use mir2_protocol::{MapInformation, MirDirection, Point};
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

pub(super) fn set_runtime_tick(world: &mut World, tick: u64) {
    world.resource_mut::<RuntimeClockResource>().tick = tick;
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
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub(super) struct MapRuntimeResource {
    pub(super) current_map: MapInformation,
    pub(super) map_region_bounds: MapBounds,
    pub(super) blocked_cells: BTreeSet<(i32, i32)>,
    pub(super) closed_door_cells: BTreeSet<(i32, i32)>,
    pub(super) conquest_wars: BTreeMap<i32, bool>,
}

impl MapRuntimeResource {
    pub(super) fn new(
        config: &SimulationConfig,
        map_region_bounds: MapBounds,
        blocked_cells: BTreeSet<(i32, i32)>,
        closed_door_cells: BTreeSet<(i32, i32)>,
    ) -> Self {
        Self {
            current_map: config.map.clone(),
            map_region_bounds,
            blocked_cells,
            closed_door_cells,
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
}

impl FishingResource {
    pub(super) fn new() -> Self {
        Self {
            fishing: false,
            auto_cast: false,
            progress_percent: 0,
            chance_percent: 0,
            found_fish: false,
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
}

impl SkillResource {
    pub(super) fn new() -> Self {
        Self { skills: Vec::new() }
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

#[derive(Resource, Debug, Clone)]
pub(super) struct MountResource {
    pub(super) mount_type: i16,
    pub(super) riding_mount: bool,
}

impl MountResource {
    pub(super) fn new() -> Self {
        Self {
            mount_type: -1,
            riding_mount: false,
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
}

impl RuntimeQueueResource {
    pub(super) fn new() -> Self {
        Self {
            pending_combat_actions: Vec::new(),
            pending_monster_spawns: Vec::new(),
        }
    }
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
}

impl PotionRecoveryResource {
    pub(super) fn new() -> Self {
        Self {
            pending_pot_health_amount: 0,
            pending_pot_mana_amount: 0,
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
