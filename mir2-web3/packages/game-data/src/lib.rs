use mir2_protocol::{MapInformation, MirClass, MirDirection, MirGender, Point};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneView {
    pub center: Point,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapBounds {
    pub min_x: i32,
    pub max_x: i32,
    pub min_y: i32,
    pub max_y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerrainKind {
    Grass,
    Dirt,
    Road,
    Water,
    Stone,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerrainPatchTemplate {
    pub x: i32,
    pub y: i32,
    pub width: u16,
    pub height: u16,
    pub kind: TerrainKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecorKind {
    Lantern,
    Banner,
    Tree,
    Rock,
    Campfire,
    Stump,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecorObjectTemplate {
    pub id: String,
    pub kind: DecorKind,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterTemplate {
    pub name: String,
    pub level: u16,
    pub class: MirClass,
    pub gender: MirGender,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisiblePlayerTemplate {
    pub object_id: u32,
    pub name: String,
    pub class: MirClass,
    pub gender: MirGender,
    pub level: u16,
    pub position: Point,
    pub direction: MirDirection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisibleMonsterTemplate {
    pub object_id: u32,
    pub name: String,
    pub image: u16,
    pub position: Point,
    pub direction: MirDirection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisibleNpcTemplate {
    pub object_id: u32,
    pub name: String,
    pub image: u16,
    pub colour_argb: i32,
    pub position: Point,
    pub direction: MirDirection,
    pub quest_ids: Vec<i32>,
    #[serde(default)]
    pub script_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneBootstrap {
    pub map: MapInformation,
    pub spawn: Point,
    pub scene_view: SceneView,
    pub terrain_patches: Vec<TerrainPatchTemplate>,
    pub decor_objects: Vec<DecorObjectTemplate>,
    pub default_character: CharacterTemplate,
    pub object_id: u32,
    pub real_id: u32,
    pub visible_players: Vec<VisiblePlayerTemplate>,
    pub visible_monsters: Vec<VisibleMonsterTemplate>,
    pub visible_npcs: Vec<VisibleNpcTemplate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapCellAttribute {
    LowWall,
    HighWall,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockedMapCellTemplate {
    pub x: i32,
    pub y: i32,
    pub attribute: MapCellAttribute,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoorMapCellTemplate {
    pub x: i32,
    pub y: i32,
    pub index: u8,
    pub closed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StarterMapCollision {
    pub map_file_name: String,
    pub map_width: u16,
    pub map_height: u16,
    pub region_bounds: MapBounds,
    pub play_bounds: MapBounds,
    pub blocked_cells: Vec<BlockedMapCellTemplate>,
    pub doors: Vec<DoorMapCellTemplate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StarterServerData {
    pub monster_spawns: Vec<MonsterSpawnTemplate>,
    pub skills: Vec<SkillTemplate>,
    pub monster_drops: Vec<MonsterDropTable>,
    pub quests: Vec<QuestTemplate>,
    pub npc_scripts: Vec<NpcScriptTemplate>,
    pub buffs: Vec<BuffTemplate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonsterSpawnTemplate {
    pub object_id: u32,
    pub key: String,
    pub name: String,
    pub image: u16,
    pub position: Point,
    pub direction: MirDirection,
    pub count: u16,
    pub spread: u16,
    pub respawn_delay_ticks: u64,
    pub random_delay_ticks: u64,
    pub can_wander: bool,
    pub max_hp: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillTemplate {
    pub key: String,
    pub crystal_spell: Option<String>,
    pub name: String,
    pub description: String,
    pub cooldown_ticks: u32,
    pub mana_cost: i32,
    pub effect: SkillEffectTemplate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SkillEffectTemplate {
    Heal {
        hp: i32,
    },
    Buff {
        buff_key: String,
        buff_name: String,
        buff_description: String,
        duration_ticks: u64,
        attack_bonus: i32,
        defence_bonus: i32,
    },
    Summon {
        spell: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonsterDropTable {
    pub monster_object_id: u32,
    pub drops: Vec<DropTemplate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DropTemplate {
    Gold {
        name: String,
        amount: u32,
        quantity: u32,
    },
    Item {
        key: String,
        name: String,
        description: String,
        weight: u16,
        quantity: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestTemplate {
    pub quest_id: i32,
    pub title: String,
    pub summary: String,
    pub reward_preview: String,
    pub required: u32,
    pub stages: QuestStageTextTemplate,
    pub quest_item: ItemTemplate,
    pub completion_rewards: QuestRewardTemplate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestStageTextTemplate {
    pub available: QuestStageCopy,
    pub in_progress: QuestStageCopy,
    pub ready_to_turn_in: QuestStageCopy,
    pub completed: QuestStageCopy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestStageCopy {
    pub objective: String,
    pub progress_label: String,
    pub tracker: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestRewardTemplate {
    pub gold: u32,
    pub items: Vec<ItemTemplate>,
    pub equipment: Vec<EquipmentTemplate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NpcScriptTemplate {
    pub npc_object_id: u32,
    pub stages: Vec<NpcStageTemplate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NpcStageTemplate {
    pub quest_stage: String,
    pub title: String,
    pub body: Vec<String>,
    pub footer: String,
    pub object_chat: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuffTemplate {
    pub key: String,
    pub crystal_buff_type: Option<String>,
    pub name: String,
    pub description: String,
    pub visible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemTemplate {
    pub key: String,
    pub name: String,
    pub description: String,
    pub weight: u16,
    pub quantity: u32,
    pub preferred_slot: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquipmentTemplate {
    pub slot: String,
    pub name: String,
    pub shape: Option<u16>,
    pub description: String,
    pub durability_current: u16,
    pub durability_max: u16,
    pub attack: i32,
    pub defence: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalItemManifest {
    pub generated_at: String,
    pub source_file: String,
    pub crystal_db_version: i32,
    pub crystal_db_custom_version: i32,
    pub total_items: usize,
    pub items: Vec<CrystalItemTemplate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalItemTemplate {
    pub item_index: i32,
    pub name: String,
    pub item_type: u8,
    pub grade: u8,
    pub required_type: u8,
    pub required_class: u8,
    pub required_gender: u8,
    pub item_set: u8,
    pub shape: i16,
    pub weight: u8,
    pub light: u8,
    pub required_amount: u8,
    pub image: u16,
    pub durability: u16,
    pub stack_size: u16,
    pub price: u32,
    pub start_item: bool,
    pub effect: u8,
    pub need_identify: bool,
    pub show_group_pickup: bool,
    pub class_based: bool,
    pub level_based: bool,
    pub can_mine: bool,
    pub global_drop_notify: bool,
    pub bind: i16,
    pub unique: i16,
    pub random_stats_id: u8,
    pub can_fast_run: bool,
    pub can_awakening: bool,
    pub slots: u8,
    pub stats: Vec<CrystalItemStat>,
    pub tooltip: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalItemStat {
    pub stat: u8,
    pub value: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalRandomItemStatsManifest {
    pub generated_at: String,
    pub source_file: String,
    pub total_profiles: usize,
    pub profiles: Vec<CrystalRandomItemStatProfile>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalRandomStatRoll {
    pub chance: u8,
    pub stat_chance: u8,
    pub max_stat: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalRandomItemStatProfile {
    pub id: u8,
    pub max_dura: CrystalRandomStatRoll,
    pub max_ac: CrystalRandomStatRoll,
    pub max_mac: CrystalRandomStatRoll,
    pub max_dc: CrystalRandomStatRoll,
    pub max_mc: CrystalRandomStatRoll,
    pub max_sc: CrystalRandomStatRoll,
    pub accuracy: CrystalRandomStatRoll,
    pub agility: CrystalRandomStatRoll,
    pub hp: CrystalRandomStatRoll,
    pub mp: CrystalRandomStatRoll,
    pub strong: CrystalRandomStatRoll,
    pub magic_resist: CrystalRandomStatRoll,
    pub poison_resist: CrystalRandomStatRoll,
    pub hp_recovery: CrystalRandomStatRoll,
    pub mp_recovery: CrystalRandomStatRoll,
    pub poison_recovery: CrystalRandomStatRoll,
    pub critical_rate: CrystalRandomStatRoll,
    pub critical_damage: CrystalRandomStatRoll,
    pub freezing: CrystalRandomStatRoll,
    pub poison_attack: CrystalRandomStatRoll,
    pub attack_speed: CrystalRandomStatRoll,
    pub luck: CrystalRandomStatRoll,
    pub curse_chance: u8,
    pub slot: CrystalRandomStatRoll,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalMagicManifest {
    pub generated_at: String,
    pub source_file: String,
    pub total_magics: usize,
    pub magics: Vec<CrystalMagicTemplate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrystalMagicTemplate {
    pub name: String,
    pub spell: String,
    pub base_cost: u8,
    pub level_cost: u8,
    pub icon: u8,
    pub level1: u8,
    pub level2: u8,
    pub level3: u8,
    pub need1: u16,
    pub need2: u16,
    pub need3: u16,
    pub delay_base: u32,
    pub delay_reduction: u32,
    pub power_base: u16,
    pub power_bonus: u16,
    pub mpower_base: u16,
    pub mpower_bonus: u16,
    pub multiplier_base: f32,
    pub multiplier_bonus: f32,
    pub range: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalBuffManifest {
    pub generated_at: String,
    pub source_file: String,
    pub total_buffs: usize,
    pub buffs: Vec<CrystalBuffTemplate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrystalBuffTemplate {
    pub buff_type: String,
    pub stack_type: String,
    pub properties: Vec<String>,
    pub visible: Option<bool>,
    pub visible_expression: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalDropManifest {
    pub generated_at: String,
    pub source_dir: String,
    pub total_tables: usize,
    pub total_entries: usize,
    pub tables: Vec<CrystalDropTable>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalDropTable {
    pub table_key: String,
    pub relative_path: String,
    pub total_entries: usize,
    pub sections: Vec<CrystalDropSection>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalDropSection {
    pub name: String,
    pub entries: Vec<CrystalDropEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalDropEntry {
    pub raw_line: String,
    pub chance_raw: String,
    pub chance_numerator: Option<u32>,
    pub chance_denominator: Option<u32>,
    pub item_name: String,
    pub amount: Option<u32>,
    pub modifiers: Vec<String>,
    pub group: Option<CrystalDropGroup>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalDropGroup {
    pub random: bool,
    pub first: bool,
    #[serde(default)]
    pub entries: Vec<CrystalDropEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalNpcManifest {
    pub generated_at: String,
    pub source_dir: String,
    pub total_scripts: usize,
    pub total_labels: usize,
    pub total_inserts: usize,
    pub scripts: Vec<CrystalNpcScript>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalNpcInfoManifest {
    pub generated_at: String,
    pub source_file: String,
    pub crystal_db_version: i32,
    pub crystal_db_custom_version: i32,
    pub total_npcs: usize,
    pub npcs: Vec<CrystalNpcInfoTemplate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalNpcInfoTemplate {
    pub npc_index: i32,
    pub map_index: i32,
    pub map_file_name: Option<String>,
    pub file_name: String,
    pub script_key: String,
    pub name: String,
    pub location: Point,
    pub image: u16,
    pub rate: u16,
    pub price_rate: f32,
    pub collect_quest_indexes: Vec<i32>,
    pub finish_quest_indexes: Vec<i32>,
    pub time_visible: bool,
    pub hour_start: u8,
    pub minute_start: u8,
    pub hour_end: u8,
    pub minute_end: u8,
    pub min_level: i16,
    pub max_level: i16,
    pub day_of_week: String,
    pub class_required: String,
    pub conquest: i32,
    pub flag_needed: i32,
    pub show_on_big_map: bool,
    pub big_map_icon: i32,
    pub can_teleport_to: bool,
    pub conquest_visible: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalNpcScript {
    pub script_key: String,
    pub relative_path: String,
    pub raw_text: String,
    pub lines: Vec<String>,
    pub line_count: usize,
    pub non_empty_line_count: usize,
    pub label_count: usize,
    pub insert_count: usize,
    pub command_directives: Vec<String>,
    pub labels: Vec<CrystalNpcLabel>,
    pub sections: Vec<CrystalNpcSection>,
    pub inserts: Vec<CrystalNpcInsert>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalNpcLabel {
    pub label: String,
    pub line_number: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalNpcSection {
    pub label: String,
    pub line_number: usize,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalNpcInsert {
    pub line_number: usize,
    pub target_path: String,
    pub target_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalNpcCommandSummary {
    pub generated_at: String,
    pub source_file: String,
    pub total_scripts: usize,
    pub total_commands: usize,
    pub implemented_commands: usize,
    pub unimplemented_commands: usize,
    pub implemented_occurrences: usize,
    pub unimplemented_occurrences: usize,
    pub commands: Vec<CrystalNpcCommandSummaryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrystalNpcCommandSummaryEntry {
    pub kind: String,
    pub command: String,
    pub count: usize,
    pub script_count: usize,
    pub runtime_status: String,
    pub examples: Vec<CrystalNpcCommandExample>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrystalNpcCommandExample {
    pub script_key: String,
    pub line_number: usize,
    pub line: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalMonsterManifest {
    pub generated_at: String,
    pub source_file: String,
    pub crystal_db_version: i32,
    pub crystal_db_custom_version: i32,
    pub total_monsters: usize,
    pub monsters: Vec<CrystalMonsterTemplate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalMonsterTemplate {
    pub monster_index: i32,
    pub name: String,
    pub image: u16,
    pub ai: u8,
    pub effect: u8,
    pub level: u16,
    pub view_range: u8,
    pub cool_eye: u8,
    pub hp: i32,
    pub min_ac: i32,
    pub max_ac: i32,
    pub min_mac: i32,
    pub max_mac: i32,
    pub min_dc: i32,
    pub max_dc: i32,
    pub min_mc: i32,
    pub max_mc: i32,
    pub min_sc: i32,
    pub max_sc: i32,
    pub light: u8,
    pub attack_speed: u16,
    pub move_speed: u16,
    pub experience: u32,
    pub can_push: bool,
    pub can_tame: bool,
    pub auto_rev: bool,
    pub undead: bool,
    pub drop_path: Option<String>,
    pub can_recall: bool,
    pub is_boss: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalMonsterAiSummary {
    pub generated_at: String,
    pub source_files: BTreeMap<String, String>,
    pub total_monsters: usize,
    pub total_ai_families: usize,
    pub spawned_ai_families: usize,
    pub implemented_runtime_families: usize,
    pub generic_runtime_families: usize,
    pub data_only_families: usize,
    pub unknown_source_ai_values: Vec<u8>,
    #[serde(default)]
    pub remaining_runtime_priorities: Vec<CrystalMonsterAiPrioritySummary>,
    pub families: Vec<CrystalMonsterAiFamilySummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalMonsterAiPrioritySummary {
    pub rank: usize,
    pub ai: u8,
    pub crystal_class: String,
    pub runtime_status: String,
    pub priority_score: usize,
    pub respawn_entity_count: usize,
    pub respawn_rule_count: usize,
    pub map_count: usize,
    pub monster_count: usize,
    #[serde(default)]
    pub boss_monster_count: usize,
    #[serde(default)]
    pub example_monsters: Vec<String>,
    #[serde(default)]
    pub example_maps: Vec<String>,
    #[serde(default)]
    pub priority_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalMonsterAiFamilySummary {
    pub ai: u8,
    pub crystal_class: String,
    #[serde(default)]
    pub crystal_notes: Vec<String>,
    pub crystal_todo: bool,
    pub runtime_status: String,
    #[serde(default)]
    pub runtime_notes: Vec<String>,
    pub monster_count: usize,
    pub respawn_rule_count: usize,
    pub respawn_entity_count: usize,
    pub map_count: usize,
    #[serde(default)]
    pub boss_monster_count: usize,
    #[serde(default)]
    pub example_monsters: Vec<String>,
    #[serde(default)]
    pub example_maps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalRespawnManifest {
    pub generated_at: String,
    pub source_file: String,
    pub source_routes_dir: String,
    pub crystal_db_version: i32,
    pub crystal_db_custom_version: i32,
    pub total_maps: usize,
    pub total_respawns: usize,
    pub maps: Vec<CrystalRespawnMap>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalRespawnMap {
    pub map_index: i32,
    pub map_file_name: String,
    pub map_title: String,
    #[serde(default)]
    pub mini_map: u16,
    #[serde(default)]
    pub big_map: u16,
    #[serde(default)]
    pub light: u8,
    #[serde(default)]
    pub no_throw_item: bool,
    #[serde(default)]
    pub no_drop_player: bool,
    #[serde(default)]
    pub no_drop_monster: bool,
    #[serde(default)]
    pub safe_zones: Vec<CrystalSafeZoneTemplate>,
    #[serde(default)]
    pub movement_count: usize,
    #[serde(default)]
    pub movements: Vec<CrystalMovementTemplate>,
    pub respawn_count: usize,
    pub respawns: Vec<CrystalRespawnTemplate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalSafeZoneTemplate {
    pub location: Point,
    pub size: u16,
    pub start_point: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalMovementTemplate {
    pub map_index: i32,
    pub source: Point,
    pub destination: Point,
    pub need_hole: bool,
    pub need_move: bool,
    pub conquest_index: i32,
    pub show_on_big_map: bool,
    pub icon: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalRespawnTemplate {
    pub monster_index: i32,
    pub location: Point,
    pub count: u16,
    pub spread: u16,
    pub delay_minutes: u16,
    pub direction: MirDirection,
    pub route_path: Option<String>,
    pub random_delay_minutes: u16,
    pub respawn_index: i32,
    pub save_respawn_time: bool,
    pub respawn_ticks: u16,
    pub monster_name: String,
    pub monster_image: u16,
    pub monster_ai: u8,
    pub monster_view_range: u8,
    pub monster_hp: i32,
    pub monster_attack_speed: u16,
    pub monster_move_speed: u16,
    pub monster_can_push: bool,
    pub monster_can_tame: bool,
    pub monster_auto_rev: bool,
    pub monster_undead: bool,
    pub route: Vec<CrystalRoutePoint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalRoutePoint {
    pub x: i32,
    pub y: i32,
    pub delay: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LanguageCode {
    English,
    ChineseSimplified,
    Spanish,
}

impl LanguageCode {
    pub fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::ChineseSimplified => "zh-CN",
            Self::Spanish => "es",
        }
    }

    pub fn locale(self) -> &'static str {
        match self {
            Self::English => "en-US",
            Self::ChineseSimplified => "zh-CN",
            Self::Spanish => "es-ES",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "en" | "en-us" | "english" => Some(Self::English),
            "zh" | "zh-cn" | "zh-hans" | "chinese" | "chinese-simplified" => {
                Some(Self::ChineseSimplified)
            }
            "es" | "es-es" | "spanish" => Some(Self::Spanish),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizationBundle {
    pub default_language: String,
    pub generated_at: String,
    pub sources: BTreeMap<String, String>,
    pub languages: BTreeMap<String, LocalizationLanguage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizationLanguage {
    pub native_name: String,
    pub locale: String,
    pub texts: BTreeMap<String, String>,
}

pub fn localization_bundle() -> &'static LocalizationBundle {
    static LOCALIZATION_BUNDLE: OnceLock<LocalizationBundle> = OnceLock::new();
    LOCALIZATION_BUNDLE.get_or_init(|| {
        serde_json::from_str(include_str!("../data/generated/localization_bundle.json"))
            .expect("localization bundle json should be valid")
    })
}

pub fn localized_text(language: LanguageCode, key: &str) -> Option<String> {
    let bundle = localization_bundle();
    bundle
        .languages
        .get(language.code())
        .and_then(|entry| entry.texts.get(key))
        .cloned()
        .or_else(|| {
            bundle
                .languages
                .get(bundle.default_language.as_str())
                .and_then(|entry| entry.texts.get(key))
                .cloned()
        })
}

pub fn localized_text_or_key(language: LanguageCode, key: &str) -> String {
    localized_text(language, key).unwrap_or_else(|| key.to_string())
}

pub fn localized_text_or_fallback(language: LanguageCode, key: &str, fallback: &str) -> String {
    localized_text(language, key).unwrap_or_else(|| fallback.to_string())
}

pub fn format_localized_text<I, S>(language: LanguageCode, key: &str, args: I) -> String
where
    I: IntoIterator<Item = S>,
    S: ToString,
{
    format_localized_text_or_fallback(language, key, key, args)
}

pub fn format_localized_text_or_fallback<I, S>(
    language: LanguageCode,
    key: &str,
    fallback: &str,
    args: I,
) -> String
where
    I: IntoIterator<Item = S>,
    S: ToString,
{
    let template = localized_text_or_fallback(language, key, fallback);
    let values = args
        .into_iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    apply_localized_args(&template, &values)
}

fn apply_localized_args(template: &str, args: &[String]) -> String {
    let mut rendered = template.to_string();
    for (index, value) in args.iter().enumerate() {
        rendered = rendered.replace(&format!("{{{index}}}"), value);
        let format_prefix = format!("{{{index}:");
        while let Some(start) = rendered.find(&format_prefix) {
            let Some(end_offset) = rendered[start..].find('}') else {
                break;
            };
            let end = start + end_offset + 1;
            rendered.replace_range(start..end, value);
        }
    }
    rendered
}

pub fn starter_scene() -> SceneBootstrap {
    static STARTER_SCENE: OnceLock<SceneBootstrap> = OnceLock::new();
    STARTER_SCENE
        .get_or_init(|| {
            serde_json::from_str(include_str!("../data/starter_scene.json"))
                .expect("starter scene json should be valid")
        })
        .clone()
}

pub fn starter_map_collision() -> StarterMapCollision {
    static STARTER_MAP_COLLISION: OnceLock<StarterMapCollision> = OnceLock::new();
    STARTER_MAP_COLLISION
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../data/generated/crystal_starter_map_collision.json"
            ))
            .expect("starter map collision json should be valid")
        })
        .clone()
}

pub fn starter_server_data() -> StarterServerData {
    static STARTER_SERVER_DATA: OnceLock<StarterServerData> = OnceLock::new();
    STARTER_SERVER_DATA
        .get_or_init(|| {
            serde_json::from_str(include_str!("../data/starter_server_data.json"))
                .expect("starter server data json should be valid")
        })
        .clone()
}

pub fn crystal_item_manifest() -> CrystalItemManifest {
    static CRYSTAL_ITEM_MANIFEST: OnceLock<CrystalItemManifest> = OnceLock::new();
    CRYSTAL_ITEM_MANIFEST
        .get_or_init(|| {
            serde_json::from_str(include_str!("../data/generated/crystal_item_manifest.json"))
                .expect("crystal item manifest json should be valid")
        })
        .clone()
}

pub fn crystal_random_item_stats_manifest() -> CrystalRandomItemStatsManifest {
    static CRYSTAL_RANDOM_ITEM_STATS_MANIFEST: OnceLock<CrystalRandomItemStatsManifest> =
        OnceLock::new();
    CRYSTAL_RANDOM_ITEM_STATS_MANIFEST
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../data/generated/crystal_random_item_stats_manifest.json"
            ))
            .expect("crystal random item stats manifest json should be valid")
        })
        .clone()
}

pub fn crystal_magic_manifest() -> CrystalMagicManifest {
    static CRYSTAL_MAGIC_MANIFEST: OnceLock<CrystalMagicManifest> = OnceLock::new();
    CRYSTAL_MAGIC_MANIFEST
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../data/generated/crystal_magic_manifest.json"
            ))
            .expect("crystal magic manifest json should be valid")
        })
        .clone()
}

pub fn crystal_buff_manifest() -> CrystalBuffManifest {
    static CRYSTAL_BUFF_MANIFEST: OnceLock<CrystalBuffManifest> = OnceLock::new();
    CRYSTAL_BUFF_MANIFEST
        .get_or_init(|| {
            serde_json::from_str(include_str!("../data/generated/crystal_buff_manifest.json"))
                .expect("crystal buff manifest json should be valid")
        })
        .clone()
}

pub fn crystal_drop_manifest() -> CrystalDropManifest {
    static CRYSTAL_DROP_MANIFEST: OnceLock<CrystalDropManifest> = OnceLock::new();
    CRYSTAL_DROP_MANIFEST
        .get_or_init(|| {
            serde_json::from_str(include_str!("../data/generated/crystal_drop_manifest.json"))
                .expect("crystal drop manifest json should be valid")
        })
        .clone()
}

pub fn crystal_npc_manifest() -> CrystalNpcManifest {
    static CRYSTAL_NPC_MANIFEST: OnceLock<CrystalNpcManifest> = OnceLock::new();
    CRYSTAL_NPC_MANIFEST
        .get_or_init(|| {
            serde_json::from_str(include_str!("../data/generated/crystal_npc_manifest.json"))
                .expect("crystal npc manifest json should be valid")
        })
        .clone()
}

pub fn crystal_npc_info_manifest() -> CrystalNpcInfoManifest {
    static CRYSTAL_NPC_INFO_MANIFEST: OnceLock<CrystalNpcInfoManifest> = OnceLock::new();
    CRYSTAL_NPC_INFO_MANIFEST
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../data/generated/crystal_npc_info_manifest.json"
            ))
            .expect("crystal npc info manifest json should be valid")
        })
        .clone()
}

pub fn crystal_npc_command_summary() -> CrystalNpcCommandSummary {
    static CRYSTAL_NPC_COMMAND_SUMMARY: OnceLock<CrystalNpcCommandSummary> = OnceLock::new();
    CRYSTAL_NPC_COMMAND_SUMMARY
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../data/generated/crystal_npc_command_summary.json"
            ))
            .expect("crystal npc command summary json should be valid")
        })
        .clone()
}

pub fn crystal_monster_manifest() -> CrystalMonsterManifest {
    static CRYSTAL_MONSTER_MANIFEST: OnceLock<CrystalMonsterManifest> = OnceLock::new();
    CRYSTAL_MONSTER_MANIFEST
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../data/generated/crystal_monster_manifest.json"
            ))
            .expect("crystal monster manifest json should be valid")
        })
        .clone()
}

pub fn crystal_monster_ai_summary() -> CrystalMonsterAiSummary {
    static CRYSTAL_MONSTER_AI_SUMMARY: OnceLock<CrystalMonsterAiSummary> = OnceLock::new();
    CRYSTAL_MONSTER_AI_SUMMARY
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../data/generated/crystal_monster_ai_summary.json"
            ))
            .expect("crystal monster ai summary json should be valid")
        })
        .clone()
}

pub fn crystal_respawn_manifest() -> CrystalRespawnManifest {
    static CRYSTAL_RESPAWN_MANIFEST: OnceLock<CrystalRespawnManifest> = OnceLock::new();
    CRYSTAL_RESPAWN_MANIFEST
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../data/generated/crystal_respawn_manifest.json"
            ))
            .expect("crystal respawn manifest json should be valid")
        })
        .clone()
}

pub fn crystal_magic_by_spell(spell: &str) -> Option<CrystalMagicTemplate> {
    crystal_magic_manifest()
        .magics
        .into_iter()
        .find(|magic| magic.spell == spell)
}

pub fn crystal_buff_by_type(buff_type: &str) -> Option<CrystalBuffTemplate> {
    crystal_buff_manifest()
        .buffs
        .into_iter()
        .find(|buff| buff.buff_type == buff_type)
}

pub fn crystal_drop_table_by_key(table_key: &str) -> Option<CrystalDropTable> {
    crystal_drop_manifest()
        .tables
        .into_iter()
        .find(|table| table.table_key == table_key)
}

pub fn crystal_drop_table_for_monster_name(monster_name: &str) -> Option<CrystalDropTable> {
    let monster = crystal_monster_by_name(monster_name)?;
    let drop_path = monster.drop_path?;
    let table_key = drop_path.replace('\\', "/");
    crystal_drop_table_by_key(&table_key)
}

pub fn crystal_item_by_name(name: &str) -> Option<CrystalItemTemplate> {
    crystal_item_manifest()
        .items
        .into_iter()
        .find(|item| item.name.eq_ignore_ascii_case(name))
}

pub fn crystal_item_by_index(item_index: i32) -> Option<CrystalItemTemplate> {
    crystal_item_manifest()
        .items
        .into_iter()
        .find(|item| item.item_index == item_index)
}

pub fn crystal_random_item_stat_profile(id: u8) -> Option<CrystalRandomItemStatProfile> {
    crystal_random_item_stats_manifest()
        .profiles
        .into_iter()
        .find(|profile| profile.id == id)
}

pub fn crystal_npc_script_by_key(script_key: &str) -> Option<CrystalNpcScript> {
    crystal_npc_manifest()
        .scripts
        .into_iter()
        .find(|script| script.script_key == script_key)
}

pub fn crystal_npc_info_by_script_key(script_key: &str) -> Option<CrystalNpcInfoTemplate> {
    crystal_npc_info_manifest()
        .npcs
        .into_iter()
        .find(|npc| npc.script_key.eq_ignore_ascii_case(script_key))
}

pub fn crystal_monster_by_name(name: &str) -> Option<CrystalMonsterTemplate> {
    crystal_monster_manifest()
        .monsters
        .into_iter()
        .find(|monster| monster.name.eq_ignore_ascii_case(name))
}

pub fn crystal_monster_by_index(monster_index: i32) -> Option<CrystalMonsterTemplate> {
    crystal_monster_manifest()
        .monsters
        .into_iter()
        .find(|monster| monster.monster_index == monster_index)
}

pub fn crystal_map_respawns_by_file_name(file_name: &str) -> Option<CrystalRespawnMap> {
    let normalized = normalize_crystal_map_file_name(file_name);
    crystal_respawn_manifest()
        .maps
        .into_iter()
        .find(|map| normalize_crystal_map_file_name(&map.map_file_name) == normalized)
}

pub fn crystal_map_respawns_by_index(map_index: i32) -> Option<CrystalRespawnMap> {
    crystal_respawn_manifest()
        .maps
        .into_iter()
        .find(|map| map.map_index == map_index)
}

pub fn crystal_starter_region_respawns() -> Vec<CrystalRespawnTemplate> {
    let collision = starter_map_collision();
    let map_file_name = normalize_crystal_map_file_name(&collision.map_file_name);

    crystal_map_respawns_by_file_name(&map_file_name)
        .map(|map| {
            map.respawns
                .into_iter()
                .filter(|respawn| respawn_overlaps_bounds(respawn, collision.region_bounds))
                .collect()
        })
        .unwrap_or_default()
}

fn normalize_crystal_map_file_name(file_name: &str) -> String {
    file_name
        .trim()
        .trim_end_matches(".map")
        .trim_end_matches(".MAP")
        .to_ascii_lowercase()
}

fn respawn_overlaps_bounds(respawn: &CrystalRespawnTemplate, bounds: MapBounds) -> bool {
    let spread = i32::from(respawn.spread);
    respawn.location.x + spread >= bounds.min_x
        && respawn.location.x - spread <= bounds.max_x
        && respawn.location.y + spread >= bounds.min_y
        && respawn.location.y - spread <= bounds.max_y
}

#[cfg(test)]
mod tests {
    use super::{
        crystal_buff_by_type, crystal_buff_manifest, crystal_drop_manifest,
        crystal_drop_table_by_key, crystal_drop_table_for_monster_name, crystal_item_by_index,
        crystal_item_by_name, crystal_item_manifest, crystal_magic_by_spell,
        crystal_magic_manifest, crystal_map_respawns_by_file_name, crystal_map_respawns_by_index,
        crystal_monster_ai_summary, crystal_monster_by_index, crystal_monster_by_name,
        crystal_monster_manifest, crystal_npc_command_summary, crystal_npc_info_by_script_key,
        crystal_npc_info_manifest, crystal_npc_manifest, crystal_npc_script_by_key,
        crystal_random_item_stat_profile, crystal_random_item_stats_manifest,
        crystal_respawn_manifest, crystal_starter_region_respawns, format_localized_text,
        localization_bundle, localized_text, starter_map_collision, starter_scene,
        starter_server_data, DropTemplate, LanguageCode, MapCellAttribute, SkillEffectTemplate,
    };
    use mir2_protocol::Point;

    #[test]
    fn starter_scene_loads() {
        let scene = starter_scene();

        assert_eq!(scene.default_character.name, "Scout");
        assert_eq!(scene.scene_view.width, 24);
        assert_eq!(scene.terrain_patches.len(), 6);
        assert_eq!(scene.decor_objects.len(), 8);
        assert_eq!(scene.visible_players.len(), 1);
        assert_eq!(scene.visible_monsters.len(), 2);
        assert_eq!(scene.visible_npcs.len(), 1);
    }

    #[test]
    fn starter_server_data_loads() {
        let data = starter_server_data();

        assert_eq!(data.monster_spawns.len(), 2);
        assert_eq!(data.skills.len(), 7);
        assert_eq!(data.monster_drops.len(), 2);
        assert_eq!(data.quests.len(), 1);
        assert_eq!(data.npc_scripts.len(), 1);
        assert_eq!(data.buffs.len(), 1);
        assert_eq!(data.monster_spawns[0].count, 1);
        assert_eq!(data.skills[0].mana_cost, 6);
        assert!(matches!(
            data.skills[1].effect,
            SkillEffectTemplate::Buff {
                attack_bonus: 4,
                defence_bonus: 1,
                ..
            }
        ));
        assert!(matches!(
            data.skills[2].effect,
            SkillEffectTemplate::Summon { ref spell } if spell == "SummonShinsu"
        ));
        assert!(matches!(
            data.monster_drops[0].drops[0],
            DropTemplate::Gold { amount: 45, .. }
        ));
        assert_eq!(data.quests[0].completion_rewards.gold, 300);
        assert_eq!(data.npc_scripts[0].npc_object_id, 4001);
        assert_eq!(data.buffs[0].key, "battle-focus");
        assert_eq!(data.skills[0].crystal_spell.as_deref(), Some("Healing"));
        assert_eq!(data.buffs[0].crystal_buff_type.as_deref(), Some("Fury"));
    }

    #[test]
    fn starter_map_collision_loads() {
        let collision = starter_map_collision();

        assert_eq!(collision.map_file_name, "0.map");
        assert_eq!(collision.map_width, 700);
        assert_eq!(collision.map_height, 700);
        assert_eq!(collision.region_bounds.min_x, 302);
        assert_eq!(collision.region_bounds.max_y, 313);
        assert_eq!(collision.blocked_cells.len(), 744);
        assert_eq!(collision.doors.len(), 7);
        assert!(collision.blocked_cells.iter().any(|cell| {
            cell.x == 327 && cell.y == 265 && cell.attribute == MapCellAttribute::HighWall
        }));
    }

    #[test]
    fn localization_bundle_loads() {
        let bundle = localization_bundle();

        assert_eq!(bundle.default_language, "en");
        assert!(bundle.languages.contains_key("zh-CN"));
        assert!(bundle.languages.contains_key("es"));
    }

    #[test]
    fn localization_lookup_uses_selected_language() {
        let value = localized_text(LanguageCode::ChineseSimplified, "client.GameName")
            .expect("client.GameName should exist");

        assert!(value.contains("传奇"));
    }

    #[test]
    fn localization_formatter_replaces_placeholders() {
        let value = format_localized_text(
            LanguageCode::English,
            "sim.youHitTargetForDamage",
            ["Field Wasp", "18"],
        );

        assert_eq!(value, "You hit Field Wasp for 18.");
    }

    #[test]
    fn crystal_magic_manifest_loads() {
        let manifest = crystal_magic_manifest();

        assert!(manifest.total_magics > 50);
        assert!(manifest
            .magics
            .iter()
            .any(|magic| magic.spell == "FireBall"));
        assert!(manifest
            .magics
            .iter()
            .any(|magic| magic.spell == "Thrusting" && magic.multiplier_base > 0.0));
    }

    #[test]
    fn crystal_buff_manifest_loads() {
        let manifest = crystal_buff_manifest();

        assert!(manifest.total_buffs > 40);
        assert!(manifest
            .buffs
            .iter()
            .any(|buff| buff.buff_type == "Curse"
                && buff.properties.iter().any(|prop| prop == "Debuff")));
        assert!(manifest
            .buffs
            .iter()
            .any(|buff| buff.buff_type == "SwiftFeet" && buff.visible == Some(true)));
    }

    #[test]
    fn crystal_drop_manifest_loads() {
        let manifest = crystal_drop_manifest();

        assert!(manifest.total_tables > 1000);
        assert!(manifest.total_entries > 70_000);

        let fishing = manifest
            .tables
            .iter()
            .find(|table| table.table_key == "00Fishing")
            .expect("00Fishing table should exist");
        assert!(fishing
            .sections
            .iter()
            .any(|section| section.name == "Fish Market"
                && section.entries.iter().any(|entry| {
                    entry.item_name == "Trout"
                        && entry.chance_numerator == Some(1)
                        && entry.chance_denominator == Some(500)
                })));

        let bone_fighter = manifest
            .tables
            .iter()
            .find(|table| table.table_key == "BichonProvince/OmaCave/BoneFighter")
            .expect("nested bone fighter table should exist");
        assert!(bone_fighter.sections.iter().any(|section| {
            section.entries.iter().any(|entry| {
                entry.item_name == "Gold"
                    && entry.amount == Some(200)
                    && entry.chance_numerator == Some(1)
                    && entry.chance_denominator == Some(10)
            })
        }));
    }

    #[test]
    fn crystal_drop_lookup_preserves_special_entry_shapes() {
        let dice = crystal_drop_table_by_key("00DiceItem").expect("00DiceItem should exist");
        assert!(dice.sections.iter().any(|section| {
            section.entries.iter().any(|entry| {
                entry.chance_raw == "0"
                    && entry.item_name == "Gold"
                    && entry.amount == Some(1000)
                    && entry.modifiers.is_empty()
            })
        }));

        let mir_king = crystal_drop_table_by_key("MirKing").expect("MirKing should exist");
        assert!(mir_king.sections.iter().any(|section| {
            section.entries.iter().any(|entry| {
                entry.item_name == "Gold"
                    && entry.amount == Some(15_000)
                    && entry.modifiers.iter().any(|modifier| modifier == "LV1")
            })
        }));

        let hi_great_ghoul =
            crystal_drop_table_by_key("HiGreatGhoul").expect("HiGreatGhoul should exist");
        assert!(hi_great_ghoul.sections.iter().any(|section| {
            section.entries.iter().any(|entry| {
                entry.raw_line == "1/1RedDagger Q"
                    && entry.item_name == "RedDagger"
                    && entry.modifiers.iter().any(|modifier| modifier == "Q")
            })
        }));
    }

    #[test]
    fn crystal_drop_entry_deserializes_group_shape() {
        let entry: super::CrystalDropEntry = serde_json::from_value(serde_json::json!({
            "raw_line": "0 GROUP*",
            "chance_raw": "0",
            "chance_numerator": null,
            "chance_denominator": null,
            "item_name": "GROUP",
            "amount": null,
            "modifiers": [],
            "group": {
                "random": true,
                "first": false,
                "entries": [
                    {
                        "raw_line": "0 Gold 1000",
                        "chance_raw": "0",
                        "chance_numerator": null,
                        "chance_denominator": null,
                        "item_name": "Gold",
                        "amount": 1000,
                        "modifiers": []
                    }
                ]
            }
        }))
        .expect("group-shaped drop entry should deserialize");

        let group = entry.group.expect("group metadata");
        assert!(group.random);
        assert!(!group.first);
        assert_eq!(group.entries.len(), 1);
        assert_eq!(group.entries[0].item_name, "Gold");
    }

    #[test]
    fn crystal_monster_drop_path_resolves_imported_drop_table() {
        let hen = crystal_drop_table_for_monster_name("Hen").expect("Hen drops should resolve");
        assert_eq!(hen.table_key, "Provinces/Hen");
        assert!(hen.sections.iter().any(|section| {
            section.entries.iter().any(|entry| {
                entry.item_name == "Chicken"
                    && entry.chance_numerator == Some(1)
                    && entry.chance_denominator == Some(1)
            })
        }));

        let deer = crystal_drop_table_for_monster_name("Deer").expect("Deer drops should resolve");
        assert_eq!(deer.table_key, "Provinces/Deer");
        assert!(deer.sections.iter().any(|section| {
            section.entries.iter().any(|entry| {
                entry.item_name == "Venison"
                    && entry.chance_numerator == Some(1)
                    && entry.chance_denominator == Some(1)
            })
        }));
    }

    #[test]
    fn crystal_item_manifest_loads() {
        let manifest = crystal_item_manifest();

        assert!(manifest.total_items > 1_000);

        let bronze_helmet =
            crystal_item_by_name("BronzeHelmet").expect("BronzeHelmet should exist");
        assert_eq!(bronze_helmet.item_index, 595);
        assert_eq!(bronze_helmet.item_type, 4);
        assert_eq!(bronze_helmet.image, 100);
        assert!(bronze_helmet
            .stats
            .iter()
            .any(|stat| stat.stat == 1 && stat.value == 1));

        let hp_drug = crystal_item_by_index(658).expect("(HP)DrugSmall should exist");
        assert_eq!(hp_drug.name, "(HP)DrugSmall");
        assert_eq!(hp_drug.stack_size, 20);
        assert!(hp_drug
            .stats
            .iter()
            .any(|stat| stat.stat == 12 && stat.value == 30));
    }

    #[test]
    fn crystal_random_item_stats_manifest_loads() {
        let manifest = crystal_random_item_stats_manifest();

        assert_eq!(manifest.total_profiles, 11);

        let weapon = crystal_random_item_stat_profile(1).expect("profile 1 should exist");
        assert_eq!(weapon.max_dura.chance, 2);
        assert_eq!(weapon.max_dc.stat_chance, 15);
        assert_eq!(weapon.max_mc.max_stat, 13);
        assert_eq!(weapon.accuracy.max_stat, 2);
        assert_eq!(weapon.attack_speed.chance, 130);

        let cursed = crystal_random_item_stat_profile(8).expect("profile 8 should exist");
        assert_eq!(cursed.curse_chance, 5);
        assert_eq!(cursed.max_ac.chance, 3);
        assert!(crystal_random_item_stat_profile(11).is_none());
    }

    #[test]
    fn crystal_npc_manifest_loads() {
        let manifest = crystal_npc_manifest();

        assert!(manifest.total_scripts > 300);
        assert!(manifest.total_labels > 1_000);
        assert!(manifest.total_inserts >= 10);

        let default_script =
            crystal_npc_script_by_key("00Default").expect("00Default script should exist");
        assert_eq!(default_script.insert_count, 10);
        assert!(default_script
            .raw_text
            .contains("SystemScripts\\00Default\\Login.txt"));
        assert_eq!(default_script.lines.len(), default_script.line_count);
        assert!(default_script.inserts.iter().any(|insert| {
            insert.target_path == "SystemScripts/00Default/Login.txt"
                && insert.target_label.as_deref() == Some("@Main")
        }));

        let test_script = crystal_npc_script_by_key("Test").expect("Test script should exist");
        assert!(test_script
            .command_directives
            .iter()
            .any(|directive| directive == "#SAY"));
        assert!(test_script
            .command_directives
            .iter()
            .any(|directive| directive == "#ACT"));
        assert!(test_script
            .labels
            .iter()
            .any(|label| label.label == "@Warrior"));
        assert!(test_script
            .labels
            .iter()
            .any(|label| label.label == "@Level"));
        assert!(test_script.lines.iter().any(|line| line.trim() == "#SAY"));
        assert!(test_script.sections.iter().any(|section| {
            section.label.eq_ignore_ascii_case("@main")
                && section.lines.iter().any(|line| line.trim() == "#SAY")
        }));

        let administrator = crystal_npc_script_by_key("BichonProvince/BichonWall/Administrator")
            .expect("Administrator script should exist");
        assert!(administrator
            .labels
            .iter()
            .any(|label| label.label == "@requestcastlewar"));
    }

    #[test]
    fn crystal_npc_info_manifest_loads() {
        let manifest = crystal_npc_info_manifest();

        assert!(manifest.total_npcs > 300);

        let wicked_trader =
            crystal_npc_info_by_script_key("BichonProvince/NaturalCave/WickedTrader")
                .expect("natural cave wicked trader NPC info should exist");
        assert_eq!(wicked_trader.map_file_name.as_deref(), Some("DM001"));
        assert_eq!(wicked_trader.rate, 200);
        assert_eq!(wicked_trader.price_rate, 2.0);
        assert_eq!(wicked_trader.location, Point { x: 4, y: 6 });
    }

    #[test]
    fn crystal_npc_command_summary_classifies_runtime_coverage() {
        let summary = crystal_npc_command_summary();

        assert!(summary.total_scripts > 300);
        assert!(summary.total_commands >= 80);
        assert!(summary.implemented_commands >= 45);
        assert!(summary.implemented_occurrences > summary.unimplemented_occurrences);
        assert!(summary.commands.iter().any(|entry| {
            entry.kind == "action"
                && entry.command == "GOTO"
                && entry.runtime_status == "implemented"
                && entry.count > 1_000
        }));
        assert!(summary.commands.iter().any(|entry| {
            entry.kind == "condition"
                && entry.command == "LEVEL"
                && entry.runtime_status == "implemented"
        }));
        assert!(summary.commands.iter().any(|entry| {
            entry.command == "CONQUESTGUARD" && entry.runtime_status == "implemented"
        }));
    }

    #[test]
    fn crystal_monster_manifest_loads() {
        let manifest = crystal_monster_manifest();

        assert!(manifest.total_monsters > 200);

        let bug_bat = crystal_monster_by_name("BugBat").expect("BugBat should exist");
        assert_eq!(bug_bat.image, 42);
        assert!(bug_bat.attack_speed > 0);
        assert!(bug_bat.move_speed > 0);
        assert!(bug_bat.max_dc >= bug_bat.min_dc);
        assert_eq!(
            crystal_monster_by_index(bug_bat.monster_index)
                .expect("BugBat should resolve by index")
                .name,
            "BugBat"
        );

        let bomb_spider = crystal_monster_by_name("BombSpider").expect("BombSpider should exist");
        assert!(bomb_spider.max_dc > 0);
        assert!(bomb_spider.max_sc >= bomb_spider.min_sc);
    }

    #[test]
    fn crystal_monster_ai_summary_classifies_manifest_families() {
        let manifest = crystal_monster_manifest();
        let summary = crystal_monster_ai_summary();

        assert_eq!(summary.total_monsters, manifest.total_monsters);
        assert!(summary.total_ai_families > 100);
        assert!(summary.spawned_ai_families > 80);
        assert!(summary.implemented_runtime_families >= 95);
        assert!(summary.unknown_source_ai_values.is_empty());
        if !summary.remaining_runtime_priorities.is_empty() {
            assert!(summary
                .remaining_runtime_priorities
                .windows(2)
                .all(|entries| {
                    let left = &entries[0];
                    let right = &entries[1];
                    left.priority_score > right.priority_score
                        || (left.priority_score == right.priority_score
                            && left.respawn_entity_count >= right.respawn_entity_count)
                }));
            assert!(summary
                .remaining_runtime_priorities
                .iter()
                .all(|entry| entry.rank > 0
                    && entry.respawn_entity_count > 0
                    && matches!(
                        entry.runtime_status.as_str(),
                        "generic_baseline" | "wildlife_partial"
                    )
                    && !entry.priority_reasons.is_empty()));
        }

        let by_ai: std::collections::BTreeMap<u8, _> = summary
            .families
            .iter()
            .map(|family| (family.ai, family))
            .collect();
        assert!(manifest
            .monsters
            .iter()
            .all(|monster| by_ai.contains_key(&monster.ai)));

        let default_monster = by_ai.get(&0).expect("AI 0 family should be classified");
        assert_eq!(default_monster.crystal_class, "MonsterObject");
        assert_eq!(default_monster.runtime_status, "implemented_special");
        assert!(default_monster.respawn_entity_count > 40_000);

        let spider = by_ai.get(&4).expect("AI 4 family should be classified");
        assert_eq!(spider.crystal_class, "SpittingSpider");
        assert_eq!(spider.runtime_status, "implemented_special");
        assert!(spider.respawn_entity_count > 1_000);

        let dig_out_zombie = by_ai.get(&24).expect("AI 24 family should be classified");
        assert_eq!(dig_out_zombie.crystal_class, "DigOutZombie");
        assert_eq!(dig_out_zombie.runtime_status, "implemented_special");

        let reviving_zombie = by_ai.get(&25).expect("AI 25 family should be classified");
        assert_eq!(reviving_zombie.crystal_class, "RevivingZombie");
        assert_eq!(reviving_zombie.runtime_status, "implemented_special");

        let shaman = by_ai.get(&26).expect("AI 26 family should be classified");
        assert_eq!(shaman.crystal_class, "ShamanZombie");
        assert_eq!(shaman.runtime_status, "implemented_special");

        let guard = by_ai.get(&6).expect("AI 6 family should be classified");
        assert_eq!(guard.crystal_class, "Guard");
        assert_eq!(guard.runtime_status, "neutral_guard_baseline");

        let black_foxman = by_ai.get(&44).expect("AI 44 family should be classified");
        assert_eq!(black_foxman.crystal_class, "BlackFoxman");
        assert_eq!(black_foxman.runtime_status, "implemented_special");

        let yin_devil_node = by_ai.get(&42).expect("AI 42 family should be classified");
        assert_eq!(yin_devil_node.crystal_class, "YinDevilNode");
        assert_eq!(yin_devil_node.runtime_status, "implemented_special");

        let red_foxman = by_ai.get(&45).expect("AI 45 family should be classified");
        assert_eq!(red_foxman.crystal_class, "RedFoxman");
        assert_eq!(red_foxman.runtime_status, "implemented_special");

        let white_foxman = by_ai.get(&46).expect("AI 46 family should be classified");
        assert_eq!(white_foxman.crystal_class, "WhiteFoxman");
        assert_eq!(white_foxman.runtime_status, "implemented_special");

        let black_hammer_cat = by_ai.get(&116).expect("AI 116 family should be classified");
        assert_eq!(black_hammer_cat.crystal_class, "BlackHammerCat");
        assert_eq!(black_hammer_cat.runtime_status, "implemented_special");

        let stray_cat = by_ai.get(&117).expect("AI 117 family should be classified");
        assert_eq!(stray_cat.crystal_class, "StrayCat");
        assert_eq!(stray_cat.runtime_status, "implemented_special");

        let cat_shaman = by_ai.get(&118).expect("AI 118 family should be classified");
        assert_eq!(cat_shaman.crystal_class, "CatShaman");
        assert_eq!(cat_shaman.runtime_status, "implemented_special");

        let water_dragon = by_ai.get(&181).expect("AI 181 family should be classified");
        assert_eq!(water_dragon.crystal_class, "WaterDragon");
        assert_eq!(water_dragon.runtime_status, "implemented_special");

        let black_tortoise = by_ai.get(&182).expect("AI 182 family should be classified");
        assert_eq!(black_tortoise.crystal_class, "BlackTortoise");
        assert_eq!(black_tortoise.runtime_status, "implemented_special");

        let hell_knight = by_ai.get(&97).expect("AI 97 family should be classified");
        assert_eq!(hell_knight.crystal_class, "HellKnight");
        assert_eq!(hell_knight.runtime_status, "implemented_special");
        assert!(hell_knight.respawn_entity_count > 1_000);

        let hell_lord = by_ai.get(&98).expect("AI 98 family should be classified");
        assert_eq!(hell_lord.crystal_class, "HellLord");
        assert_eq!(hell_lord.runtime_status, "implemented_special");

        let hell_bomb = by_ai.get(&99).expect("AI 99 family should be classified");
        assert_eq!(hell_bomb.crystal_class, "HellBomb");
        assert_eq!(hell_bomb.runtime_status, "implemented_special");

        let stone_trap = by_ai.get(&255).expect("AI 255 family should be classified");
        assert_eq!(stone_trap.crystal_class, "StoneTrap");
        assert_eq!(stone_trap.runtime_status, "implemented_special");
    }

    #[test]
    fn crystal_respawn_manifest_loads() {
        let manifest = crystal_respawn_manifest();

        assert!(manifest.total_maps > 400);
        assert!(manifest.total_respawns > 6_000);
        assert_eq!(manifest.total_maps, manifest.maps.len());
        assert!(manifest.maps.iter().any(|map| map.respawn_count == 0));
        assert!(manifest.maps.iter().all(|map| {
            map.respawn_count == map.respawns.len() && map.movement_count == map.movements.len()
        }));
        let map_indices: std::collections::BTreeSet<i32> =
            manifest.maps.iter().map(|map| map.map_index).collect();
        let missing_movement_targets: Vec<_> = manifest
            .maps
            .iter()
            .flat_map(|map| {
                let map_indices = &map_indices;
                map.movements.iter().filter_map(move |movement| {
                    (!map_indices.contains(&movement.map_index)).then_some((
                        map.map_file_name.as_str(),
                        movement.map_index,
                        movement.source.clone(),
                        movement.destination.clone(),
                    ))
                })
            })
            .collect();
        assert_eq!(
            missing_movement_targets,
            vec![
                ("4", 388, Point { x: 70, y: 191 }, Point { x: 77, y: 74 }),
                ("4", 388, Point { x: 71, y: 190 }, Point { x: 77, y: 74 }),
            ]
        );

        let bichon = crystal_map_respawns_by_file_name("0.map").expect("Bichon map should exist");
        assert_eq!(bichon.map_title, "BichonProvince");
        assert_eq!(bichon.mini_map, 101);
        assert_eq!(bichon.big_map, 101);
        assert!(bichon.safe_zones.iter().any(|safe_zone| {
            safe_zone.location == (Point { x: 288, y: 616 })
                && safe_zone.size == 10
                && safe_zone.start_point
        }));
        assert!(bichon.movements.iter().any(|movement| {
            movement.map_index == 2
                && movement.source == (Point { x: 347, y: 188 })
                && movement.destination == (Point { x: 13, y: 40 })
                && !movement.need_hole
                && !movement.need_move
        }));
        assert!(bichon
            .respawns
            .iter()
            .any(|respawn| respawn.monster_name == "Hen"));
        assert!(bichon
            .respawns
            .iter()
            .any(|respawn| respawn.monster_name == "Royal_Guard"));
        assert!(bichon.respawns.iter().any(|respawn| {
            respawn.monster_name == "Royal_Guard"
                && respawn.monster_ai == 6
                && respawn.monster_view_range > 0
                && respawn.monster_move_speed >= 400
        }));
        assert_eq!(
            crystal_map_respawns_by_index(bichon.map_index)
                .expect("Bichon should resolve by index")
                .map_file_name,
            "0"
        );

        let penal_cavern =
            crystal_map_respawns_by_file_name("D1801").expect("Penal Cavern should exist");
        assert_eq!(penal_cavern.map_title, "PenalCavern");
        assert_eq!(penal_cavern.mini_map, 0);
        assert_eq!(penal_cavern.big_map, 0);
        assert_eq!(penal_cavern.light, 4);
        assert!(!penal_cavern.movements.is_empty());
    }

    #[test]
    fn crystal_starter_region_respawns_include_expected_monsters() {
        let respawns = crystal_starter_region_respawns();

        assert!(respawns.len() >= 13);
        assert!(respawns.iter().any(|respawn| {
            respawn.monster_name == "Hen" && respawn.spread > 0 && respawn.count >= 5
        }));
        assert!(respawns
            .iter()
            .any(|respawn| respawn.monster_name == "Royal_Guard"));
        assert!(respawns
            .iter()
            .any(|respawn| respawn.monster_name == "Royal_Archer"));
        assert!(respawns.iter().any(|respawn| {
            respawn.monster_name == "Royal_Archer"
                && respawn.monster_ai == 57
                && respawn.monster_attack_speed >= 400
        }));
    }

    #[test]
    fn starter_runtime_data_maps_into_crystal_manifests() {
        let data = starter_server_data();

        assert!(data.skills.iter().all(|skill| {
            skill
                .crystal_spell
                .as_deref()
                .and_then(crystal_magic_by_spell)
                .is_some()
        }));
        assert!(data.buffs.iter().all(|buff| {
            buff.crystal_buff_type
                .as_deref()
                .and_then(crystal_buff_by_type)
                .is_some()
        }));
    }
}
