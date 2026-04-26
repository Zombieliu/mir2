use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

use mir2_game_data::{
    starter_map_collision, starter_scene, DecorObjectTemplate, MapBounds, SceneBootstrap,
    SceneView, StarterMapCollision, TerrainPatchTemplate,
};
use mir2_protocol::{MapInformation, MirClass, MirDirection, MirGender, Point, SelectInfo};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ItemGrade {
    #[default]
    None,
    Common,
    Rare,
    Legendary,
    Mythical,
    Heroic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterRecord {
    pub index: i32,
    pub name: String,
    pub level: u16,
    pub class: MirClass,
    pub gender: MirGender,
}

impl CharacterRecord {
    pub fn to_select_info(&self) -> SelectInfo {
        SelectInfo {
            index: self.index,
            name: self.name.clone(),
            level: self.level,
            class: self.class,
            gender: self.gender,
            last_access_binary_datetime: 638452800000000000,
        }
    }
}

pub type SharedAccountStore = Arc<Mutex<AccountStore>>;
const ACCOUNT_STORE_SCHEMA_VERSION: u16 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountStore {
    #[serde(
        rename = "schemaVersion",
        default = "legacy_account_store_schema_version"
    )]
    pub schema_version: u16,
    pub accounts: BTreeMap<String, AccountRecord>,
}

impl AccountStore {
    pub fn new(default_character: CharacterRecord) -> Self {
        let mut accounts = BTreeMap::new();
        accounts.insert("demo".to_string(), AccountRecord::new(default_character));
        Self {
            schema_version: ACCOUNT_STORE_SCHEMA_VERSION,
            accounts,
        }
    }

    pub fn load_or_new(path: &Path, default_character: CharacterRecord) -> Self {
        let store = fs::read_to_string(path)
            .ok()
            .and_then(|data| serde_json::from_str::<AccountStore>(&data).ok())
            .unwrap_or_else(|| AccountStore::new(default_character.clone()));
        store
            .with_default_account(default_character)
            .migrate_to_current_schema()
    }

    pub fn save_to_path(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create account store directory {}: {error}",
                    parent.display()
                )
            })?;
        }

        let data = serde_json::to_string_pretty(self)
            .map_err(|error| format!("failed to encode account store: {error}"))?;
        write_file_atomically(path, data.as_bytes()).map_err(|error| {
            format!(
                "failed to atomically write account store {}: {error}",
                path.display()
            )
        })
    }

    fn with_default_account(mut self, default_character: CharacterRecord) -> Self {
        self.accounts
            .entry("demo".to_string())
            .or_insert_with(|| AccountRecord::new(default_character));
        self
    }

    fn migrate_to_current_schema(mut self) -> Self {
        if self.schema_version < ACCOUNT_STORE_SCHEMA_VERSION {
            self.schema_version = ACCOUNT_STORE_SCHEMA_VERSION;
        }
        self
    }
}

const fn legacy_account_store_schema_version() -> u16 {
    1
}

fn write_file_atomically(path: &Path, data: &[u8]) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;

    let temp_path = atomic_temp_path(path);
    {
        let mut file = File::create(&temp_path)?;
        file.write_all(data)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
    }

    match replace_file_atomically(&temp_path, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&temp_path);
            Err(error)
        }
    }
}

fn atomic_temp_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("account-store.json");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    path.with_file_name(format!(".{file_name}.tmp-{}-{unique}", std::process::id()))
}

#[cfg(windows)]
fn replace_file_atomically(from: &Path, to: &Path) -> io::Result<()> {
    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

    #[link(name = "Kernel32")]
    extern "system" {
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    }

    let from_wide = wide_null_terminated(from.as_os_str());
    let to_wide = wide_null_terminated(to.as_os_str());
    let ok = unsafe {
        MoveFileExW(
            from_wide.as_ptr(),
            to_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn wide_null_terminated(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(not(windows))]
fn replace_file_atomically(from: &Path, to: &Path) -> io::Result<()> {
    fs::rename(from, to)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountRecord {
    #[serde(default = "default_account_password")]
    pub password: String,
    #[serde(default = "default_storage_size")]
    pub storage_size: u16,
    #[serde(default)]
    pub has_expanded_storage: bool,
    #[serde(default)]
    pub expanded_storage_expiry_time_binary_datetime: i64,
    #[serde(default)]
    pub storage_password: String,
    #[serde(default)]
    pub storage_password_last_set_binary_datetime: i64,
    pub characters: Vec<CharacterRecord>,
    pub saves: BTreeMap<i32, CharacterSaveRecord>,
}

impl AccountRecord {
    pub fn new(default_character: CharacterRecord) -> Self {
        let mut saves = BTreeMap::new();
        saves.insert(
            default_character.index,
            CharacterSaveRecord::new(default_character.clone()),
        );
        Self {
            password: default_account_password(),
            storage_size: default_storage_size(),
            has_expanded_storage: false,
            expanded_storage_expiry_time_binary_datetime: 0,
            storage_password: String::new(),
            storage_password_last_set_binary_datetime: 0,
            characters: vec![default_character],
            saves,
        }
    }
}

fn default_account_password() -> String {
    "demo".to_string()
}

const fn default_storage_size() -> u16 {
    80
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterSaveRecord {
    pub character: CharacterRecord,
    pub map_file_name: String,
    pub map_title: String,
    pub position: Point,
    pub direction: MirDirection,
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    #[serde(default)]
    pub experience: i64,
    #[serde(default = "default_max_experience")]
    pub max_experience: i64,
    pub gold: u32,
    #[serde(default)]
    pub credit: u32,
    pub inventory_items_json: Vec<String>,
    pub belt_items_json: Vec<String>,
    #[serde(default)]
    pub storage_items_json: Vec<String>,
    pub equipment_items_json: Vec<String>,
    pub quest_states_json: Vec<String>,
    pub skill_states_json: Vec<String>,
    #[serde(default)]
    pub npc_flag_states_json: Vec<String>,
    #[serde(default)]
    pub npc_saved_values_json: Vec<String>,
    #[serde(default)]
    pub npc_buy_back_items_json: Vec<String>,
    #[serde(default)]
    pub npc_used_goods_items_json: Vec<String>,
    #[serde(default)]
    pub stage5_systems_json: Option<String>,
}

impl CharacterSaveRecord {
    pub fn new(character: CharacterRecord) -> Self {
        Self {
            character,
            map_file_name: String::new(),
            map_title: String::new(),
            position: Point { x: 0, y: 0 },
            direction: MirDirection::Down,
            hp: 120,
            max_hp: 120,
            mp: 45,
            experience: 0,
            max_experience: default_max_experience(),
            gold: 1280,
            credit: 0,
            inventory_items_json: Vec::new(),
            belt_items_json: Vec::new(),
            storage_items_json: Vec::new(),
            equipment_items_json: Vec::new(),
            quest_states_json: Vec::new(),
            skill_states_json: Vec::new(),
            npc_flag_states_json: Vec::new(),
            npc_saved_values_json: Vec::new(),
            npc_buy_back_items_json: Vec::new(),
            npc_used_goods_items_json: Vec::new(),
            stage5_systems_json: None,
        }
    }
}

const fn default_max_experience() -> i64 {
    100
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn default_character() -> CharacterRecord {
        CharacterRecord {
            index: 0,
            name: "Tester".to_string(),
            level: 1,
            class: MirClass::Warrior,
            gender: MirGender::Male,
        }
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!(
            "mir2-config-{label}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("temp dir should be created");
        dir
    }

    #[test]
    fn legacy_character_save_without_npc_flag_states_uses_default() {
        let store = serde_json::from_str::<AccountStore>(
            r#"{
                "accounts": {
                    "demo": {
                        "characters": [
                            {
                                "index": 0,
                                "name": "Legacy",
                                "level": 7,
                                "class": "Warrior",
                                "gender": "Male"
                            }
                        ],
                        "saves": {
                            "0": {
                                "character": {
                                    "index": 0,
                                    "name": "Legacy",
                                    "level": 7,
                                    "class": "Warrior",
                                    "gender": "Male"
                                },
                                "map_file_name": "0",
                                "map_title": "Bichon",
                                "position": { "x": 330, "y": 270 },
                                "direction": "Down",
                                "hp": 120,
                                "max_hp": 120,
                                "mp": 45,
                                "gold": 1280,
                                "inventory_items_json": [],
                                "belt_items_json": [],
                                "equipment_items_json": [],
                                "quest_states_json": [],
                                "skill_states_json": []
                            }
                        }
                    }
                }
            }"#,
        )
        .expect("legacy save without npc flags should deserialize");

        let save = store
            .accounts
            .get("demo")
            .and_then(|account| account.saves.get(&0))
            .expect("legacy character save should exist");
        assert!(save.npc_flag_states_json.is_empty());
        assert!(save.npc_saved_values_json.is_empty());
        assert!(save.npc_buy_back_items_json.is_empty());
        assert!(save.npc_used_goods_items_json.is_empty());
        assert_eq!(save.experience, 0);
        assert_eq!(save.max_experience, 100);
        let account = store
            .accounts
            .get("demo")
            .expect("legacy account should exist");
        assert_eq!(account.password, "demo");
        assert_eq!(account.storage_size, 80);
        assert!(!account.has_expanded_storage);
        assert_eq!(account.expanded_storage_expiry_time_binary_datetime, 0);
        assert!(account.storage_password.is_empty());
        assert_eq!(account.storage_password_last_set_binary_datetime, 0);
    }

    #[test]
    fn account_store_new_records_current_schema_version() {
        let store = AccountStore::new(default_character());
        assert_eq!(store.schema_version, ACCOUNT_STORE_SCHEMA_VERSION);
    }

    #[test]
    fn legacy_account_store_without_schema_version_migrates_on_load() {
        let dir = unique_temp_dir("legacy-schema");
        let path = dir.join("accounts.json");
        fs::write(
            &path,
            r#"{
                "accounts": {
                    "demo": {
                        "characters": [
                            {
                                "index": 0,
                                "name": "Legacy",
                                "level": 7,
                                "class": "Warrior",
                                "gender": "Male"
                            }
                        ],
                        "saves": {}
                    }
                }
            }"#,
        )
        .expect("legacy account store fixture should write");

        let store = AccountStore::load_or_new(&path, default_character());

        assert_eq!(store.schema_version, ACCOUNT_STORE_SCHEMA_VERSION);
        assert!(store.accounts.contains_key("demo"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn corrupt_account_store_loads_default_without_deleting_source() {
        let dir = unique_temp_dir("corrupt-load");
        let path = dir.join("accounts.json");
        fs::write(&path, "{not-json").expect("corrupt account store fixture should write");

        let store = AccountStore::load_or_new(&path, default_character());

        assert_eq!(store.schema_version, ACCOUNT_STORE_SCHEMA_VERSION);
        assert_eq!(
            fs::read_to_string(&path).expect("corrupt source should still exist"),
            "{not-json"
        );
        assert!(store.accounts.contains_key("demo"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn account_store_save_replaces_existing_file_and_cleans_temp_file() {
        let dir = unique_temp_dir("atomic-save");
        let path = dir.join("accounts.json");
        fs::write(&path, r#"{"stale":true}"#).expect("stale file should write");

        let store = AccountStore::new(default_character());
        store
            .save_to_path(&path)
            .expect("account store should save atomically");

        let saved = fs::read_to_string(&path).expect("saved account store should exist");
        assert!(saved.contains(r#""schemaVersion": 2"#));
        assert!(saved.contains(r#""accounts""#));

        let temp_files = fs::read_dir(&dir)
            .expect("temp dir should list")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".accounts.json.tmp-")
            })
            .count();
        assert_eq!(temp_files, 0);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn simulation_config_backup_and_restore_roundtrips_account_store() {
        let dir = unique_temp_dir("backup-restore");
        let primary_path = dir.join("accounts.json");
        let backup_path = dir.join("accounts.backup.json");
        let config = SimulationConfig::default().with_account_store_path(primary_path.clone());

        {
            let mut store = config
                .account_store
                .lock()
                .expect("account store mutex should not be poisoned");
            let character = CharacterRecord {
                index: 1,
                name: "BackupBlade".to_string(),
                level: 11,
                class: MirClass::Wizard,
                gender: MirGender::Female,
            };
            store
                .accounts
                .insert("backup".to_string(), AccountRecord::new(character));
        }
        config
            .save_account_store()
            .expect("primary account store should save");
        config
            .backup_account_store(&backup_path)
            .expect("account store backup should save");

        {
            let mut store = config
                .account_store
                .lock()
                .expect("account store mutex should not be poisoned");
            store.accounts.remove("backup");
        }
        config
            .save_account_store()
            .expect("mutated primary account store should save");

        config
            .restore_account_store_from_backup(&backup_path)
            .expect("account store backup should restore");
        let restored_config = SimulationConfig::default().with_account_store_path(primary_path);
        let restored_store = restored_config
            .account_store
            .lock()
            .expect("account store mutex should not be poisoned");
        assert!(restored_store.accounts.contains_key("backup"));
        assert!(restored_store
            .accounts
            .get("backup")
            .expect("restored account")
            .characters
            .iter()
            .any(|character| character.name == "BackupBlade"));

        let _ = fs::remove_dir_all(dir);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisiblePlayerRecord {
    pub object_id: u32,
    pub name: String,
    pub class: MirClass,
    pub gender: MirGender,
    pub level: u16,
    pub armour_shape: Option<u16>,
    pub weapon_shape: Option<u16>,
    pub position: Point,
    pub direction: MirDirection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleMonsterRecord {
    pub object_id: u32,
    pub name: String,
    pub image: u16,
    pub position: Point,
    pub direction: MirDirection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleNpcRecord {
    pub object_id: u32,
    pub name: String,
    pub image: u16,
    pub colour_argb: i32,
    pub position: Point,
    pub direction: MirDirection,
    pub quest_ids: Vec<i32>,
    pub script_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapTransferRecord {
    pub key: String,
    pub from_map_file_name: String,
    pub from_bounds: MapBounds,
    pub to_map_file_name: String,
    pub to_map_title: String,
    pub to_position: Point,
    pub to_direction: MirDirection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeZoneRecord {
    pub map_file_name: String,
    pub bounds: MapBounds,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapDropRuleRecord {
    pub map_file_name: String,
    pub no_town_teleport: bool,
    pub no_escape: bool,
    pub no_random: bool,
    pub no_drug: bool,
    pub no_reincarnation: bool,
    pub no_throw_item: bool,
    pub no_drop_player: bool,
    pub no_drop_monster: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonsterSpawnSource {
    StarterScenario,
    CrystalStarterRegion,
}

#[derive(Debug, Clone)]
pub struct SimulationConfig {
    pub map: MapInformation,
    pub spawn: Point,
    pub scene_view: SceneView,
    pub map_collision: StarterMapCollision,
    pub require_storage_password: bool,
    pub monster_spawn_source: MonsterSpawnSource,
    pub terrain_patches: Vec<TerrainPatchTemplate>,
    pub decor_objects: Vec<DecorObjectTemplate>,
    pub default_character: CharacterRecord,
    pub object_id: u32,
    pub real_id: u32,
    pub visible_players: Vec<VisiblePlayerRecord>,
    pub group_member_object_ids: Vec<u32>,
    pub visible_monsters: Vec<VisibleMonsterRecord>,
    pub visible_npcs: Vec<VisibleNpcRecord>,
    pub conquest_wars: BTreeMap<i32, bool>,
    pub map_transfers: Vec<MapTransferRecord>,
    pub safe_zones: Vec<SafeZoneRecord>,
    pub map_drop_rules: Vec<MapDropRuleRecord>,
    pub account_store: SharedAccountStore,
    pub account_store_path: Option<PathBuf>,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self::from_scene(&starter_scene())
    }
}

impl SimulationConfig {
    pub fn from_scene(scene: &SceneBootstrap) -> Self {
        Self::from_scene_with_collision(scene, starter_map_collision())
    }

    pub fn from_scene_with_collision(
        scene: &SceneBootstrap,
        map_collision: StarterMapCollision,
    ) -> Self {
        let default_character = CharacterRecord {
            index: 0,
            name: scene.default_character.name.clone(),
            level: scene.default_character.level,
            class: scene.default_character.class,
            gender: scene.default_character.gender,
        };

        Self {
            map: scene.map.clone(),
            spawn: scene.spawn.clone(),
            scene_view: scene.scene_view.clone(),
            map_collision,
            require_storage_password: true,
            monster_spawn_source: MonsterSpawnSource::StarterScenario,
            terrain_patches: scene.terrain_patches.clone(),
            decor_objects: scene.decor_objects.clone(),
            default_character: default_character.clone(),
            object_id: scene.object_id,
            real_id: scene.real_id,
            visible_players: scene
                .visible_players
                .iter()
                .map(|player| VisiblePlayerRecord {
                    object_id: player.object_id,
                    name: player.name.clone(),
                    class: player.class,
                    gender: player.gender,
                    level: player.level,
                    armour_shape: None,
                    weapon_shape: None,
                    position: player.position.clone(),
                    direction: player.direction,
                })
                .collect(),
            group_member_object_ids: Vec::new(),
            visible_monsters: scene
                .visible_monsters
                .iter()
                .map(|monster| VisibleMonsterRecord {
                    object_id: monster.object_id,
                    name: monster.name.clone(),
                    image: monster.image,
                    position: monster.position.clone(),
                    direction: monster.direction,
                })
                .collect(),
            visible_npcs: scene
                .visible_npcs
                .iter()
                .map(|npc| VisibleNpcRecord {
                    object_id: npc.object_id,
                    name: npc.name.clone(),
                    image: npc.image,
                    colour_argb: npc.colour_argb,
                    position: npc.position.clone(),
                    direction: npc.direction,
                    quest_ids: npc.quest_ids.clone(),
                    script_key: npc.script_key.clone(),
                })
                .collect(),
            conquest_wars: BTreeMap::new(),
            map_transfers: starter_map_transfers(),
            safe_zones: starter_safe_zones(),
            map_drop_rules: Vec::new(),
            account_store: Arc::new(Mutex::new(AccountStore::new(default_character))),
            account_store_path: None,
        }
    }

    pub fn with_account_store_path(mut self, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        self.account_store = Arc::new(Mutex::new(AccountStore::load_or_new(
            &path,
            self.default_character.clone(),
        )));
        self.account_store_path = Some(path);
        self
    }

    pub fn save_account_store(&self) -> Result<(), String> {
        let Some(path) = self.account_store_path.as_deref() else {
            return Ok(());
        };
        let store = self
            .account_store
            .lock()
            .expect("account store mutex should not be poisoned");
        store.save_to_path(path)
    }

    pub fn backup_account_store(&self, backup_path: impl AsRef<Path>) -> Result<(), String> {
        let store = self
            .account_store
            .lock()
            .expect("account store mutex should not be poisoned");
        store.save_to_path(backup_path.as_ref())
    }

    pub fn restore_account_store_from_backup(
        &self,
        backup_path: impl AsRef<Path>,
    ) -> Result<(), String> {
        let backup_path = backup_path.as_ref();
        let data = fs::read_to_string(backup_path).map_err(|error| {
            format!(
                "failed to read account store backup {}: {error}",
                backup_path.display()
            )
        })?;
        let restored = serde_json::from_str::<AccountStore>(&data)
            .map_err(|error| {
                format!(
                    "failed to decode account store backup {}: {error}",
                    backup_path.display()
                )
            })?
            .with_default_account(self.default_character.clone())
            .migrate_to_current_schema();

        {
            let mut store = self
                .account_store
                .lock()
                .expect("account store mutex should not be poisoned");
            *store = restored;
        }

        self.save_account_store()
    }
}

fn starter_map_transfers() -> Vec<MapTransferRecord> {
    vec![MapTransferRecord {
        key: "starter-east-field-gate".to_string(),
        from_map_file_name: "0".to_string(),
        from_bounds: MapBounds {
            min_x: 339,
            max_x: 341,
            min_y: 268,
            max_y: 271,
        },
        to_map_file_name: "0".to_string(),
        to_map_title: "BichonProvince".to_string(),
        to_position: Point { x: 330, y: 270 },
        to_direction: MirDirection::Down,
    }]
}

fn starter_safe_zones() -> Vec<SafeZoneRecord> {
    vec![SafeZoneRecord {
        map_file_name: "0".to_string(),
        bounds: MapBounds {
            min_x: 324,
            max_x: 332,
            min_y: 268,
            max_y: 273,
        },
    }]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WorldEntityKind {
    SelfPlayer,
    Player,
    Monster,
    Npc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WorldEntityDisposition {
    Friendly,
    Neutral,
    Hostile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemContainer {
    Bag1,
    Bag2,
    Quest,
    Belt,
    Storage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EquipmentSlot {
    Weapon,
    Armour,
    Helmet,
    Mount,
    Necklace,
    Torch,
    BraceletLeft,
    BraceletRight,
    RingLeft,
    RingRight,
    Amulet,
    Boots,
    Belt,
    Stone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QuestStage {
    Available,
    InProgress,
    ReadyToTurnIn,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldEntitySpriteSnapshot {
    pub body_library: String,
    pub hair_library: Option<String>,
    pub weapon_library: Option<String>,
    pub weapon_library_secondary: Option<String>,
    pub frame_base_offset: u16,
    pub weapon_frame_offset: Option<u16>,
    pub alt_body_library: Option<String>,
    pub alt_hair_library: Option<String>,
    pub alt_weapon_library: Option<String>,
    pub alt_weapon_library_secondary: Option<String>,
    pub alt_frame_base_offset: Option<u16>,
    pub alt_weapon_frame_offset: Option<u16>,
    pub frame_count: u8,
    pub direction_stride: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldEntitySnapshot {
    pub object_id: u32,
    pub kind: WorldEntityKind,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub direction: MirDirection,
    pub class: Option<MirClass>,
    pub gender: Option<MirGender>,
    pub level: Option<u16>,
    pub hp: Option<i32>,
    pub max_hp: Option<i32>,
    pub dead: bool,
    pub disposition: WorldEntityDisposition,
    pub sprite: Option<WorldEntitySpriteSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldItemSnapshot {
    pub key: String,
    pub name: String,
    pub icon: u16,
    pub slot: u8,
    pub container: ItemContainer,
    pub quantity: u32,
    pub description: String,
    pub durability_current: Option<u16>,
    pub durability_max: Option<u16>,
    pub grade: ItemGrade,
    pub added_attack: i32,
    pub added_defence: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EquipmentItemSnapshot {
    pub slot: EquipmentSlot,
    pub name: String,
    pub icon: u16,
    pub shape: Option<u16>,
    pub description: String,
    pub durability_current: u16,
    pub durability_max: u16,
    pub grade: ItemGrade,
    pub attack: i32,
    pub defence: i32,
    pub added_attack: i32,
    pub added_defence: i32,
    pub added_luck: i32,
    pub socket_slots: u8,
    pub sealed_expiry_time_binary_datetime: i64,
    pub sealed_next_time_binary_datetime: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroundDropSnapshot {
    pub object_id: u32,
    pub name: String,
    pub name_colour_argb: i32,
    pub x: i32,
    pub y: i32,
    pub quantity: u32,
    pub source_monster: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestSnapshot {
    pub quest_id: i32,
    pub title: String,
    pub summary: String,
    pub objective: String,
    pub progress_label: String,
    pub tracker: String,
    pub stage: QuestStage,
    pub current: u32,
    pub required: u32,
    pub reward_preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NpcDialogSnapshot {
    pub npc_object_id: u32,
    pub npc_name: String,
    pub title: String,
    pub body: Vec<String>,
    pub footer: String,
    pub links: Vec<NpcDialogLinkSnapshot>,
    pub input: Option<NpcDialogInputSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NpcDialogLinkSnapshot {
    pub text: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NpcDialogInputSnapshot {
    pub target: String,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSnapshot {
    pub key: String,
    pub name: String,
    pub description: String,
    pub cooldown_remaining_ticks: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffSnapshot {
    pub key: String,
    pub name: String,
    pub description: String,
    pub remaining_ticks: u32,
    pub attack_bonus: i32,
    pub defence_bonus: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage5SystemsState {
    pub group: Stage5GroupState,
    pub guild: Stage5GuildState,
    pub social: Stage5SocialState,
    pub mail: Vec<Stage5MailMessage>,
    pub trade: Option<Stage5TradeState>,
    pub auction: Vec<Stage5AuctionListing>,
    pub conquest: Stage5ConquestState,
    #[serde(default)]
    pub guild_territory: Stage5GuildTerritoryState,
    pub hero: Option<Stage5HeroState>,
    pub profession: Stage5ProfessionState,
    #[serde(default)]
    pub appearance: Stage5AppearanceState,
    #[serde(default)]
    pub name_lists: Vec<String>,
}

impl Default for Stage5SystemsState {
    fn default() -> Self {
        Self {
            group: Stage5GroupState::default(),
            guild: Stage5GuildState::default(),
            social: Stage5SocialState::default(),
            mail: Vec::new(),
            trade: None,
            auction: Vec::new(),
            conquest: Stage5ConquestState::default(),
            guild_territory: Stage5GuildTerritoryState::default(),
            hero: None,
            profession: Stage5ProfessionState::default(),
            appearance: Stage5AppearanceState::default(),
            name_lists: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage5GroupState {
    pub members: Vec<String>,
    pub loot_mode: String,
}

impl Default for Stage5GroupState {
    fn default() -> Self {
        Self {
            members: Vec::new(),
            loot_mode: "free".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Stage5GuildState {
    pub name: String,
    pub members: Vec<String>,
    pub rank: String,
    pub permissions: Vec<String>,
    pub chat_log: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Stage5SocialState {
    pub friends: Vec<String>,
    pub blocked: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage5MailMessage {
    pub id: u32,
    pub from: String,
    pub to: String,
    pub subject: String,
    pub body: String,
    pub gold: u32,
    #[serde(default)]
    pub items: Vec<String>,
    pub claimed: bool,
    pub deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage5TradeState {
    pub partner: String,
    pub offered_items: Vec<String>,
    pub offered_gold: u32,
    pub accepted: bool,
    pub completed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage5AuctionListing {
    pub id: u32,
    pub seller: String,
    pub item_key: String,
    pub price: u32,
    pub sold: bool,
    pub cancelled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Stage5ConquestState {
    pub castle_owner: String,
    pub active_wars: Vec<String>,
    pub event_log: Vec<String>,
    pub tax_rate_percent: u8,
    pub gold: u32,
    pub guards: Vec<u8>,
    pub walls: Vec<u8>,
    pub gates: Vec<u8>,
    pub open_gates: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage5GuildTerritoryState {
    pub owned: bool,
    pub map_file_name: String,
    pub rental_days_left: u32,
    pub recall_log: Vec<String>,
}

impl Default for Stage5GuildTerritoryState {
    fn default() -> Self {
        Self {
            owned: false,
            map_file_name: "GA0".to_string(),
            rental_days_left: 0,
            recall_log: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Stage5AppearanceState {
    pub hair: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage5HeroState {
    pub name: String,
    pub level: u16,
    pub behaviour: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Stage5ProfessionState {
    pub mining_level: u8,
    pub ore: u32,
    pub crafted_items: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapTransferSnapshot {
    pub key: String,
    pub map_file_name: String,
    pub bounds: MapBounds,
    pub to_map_file_name: String,
    pub to_map_title: String,
    pub to_position: Point,
    pub to_direction: MirDirection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldSnapshot {
    pub tick: u64,
    pub map_title: Option<String>,
    pub map_file_name: Option<String>,
    pub in_safe_zone: bool,
    pub player_object_id: Option<u32>,
    pub player_hp: Option<i32>,
    pub player_max_hp: Option<i32>,
    pub player_mp: Option<i32>,
    pub player_experience: i64,
    pub player_max_experience: i64,
    pub gold: u32,
    pub credit: u32,
    pub current_weight: u16,
    pub max_weight: u16,
    pub free_bag_slots: u16,
    pub max_bag_slots: u16,
    pub storage_size: u16,
    pub has_expanded_storage: bool,
    pub has_storage_password: bool,
    pub require_storage_password: bool,
    pub storage_password_last_set_binary_datetime: i64,
    pub expanded_storage_expiry_time_binary_datetime: i64,
    pub scene_view: Option<SceneView>,
    pub terrain_patches: Vec<TerrainPatchTemplate>,
    pub decor_objects: Vec<DecorObjectTemplate>,
    pub entities: Vec<WorldEntitySnapshot>,
    pub ground_drops: Vec<GroundDropSnapshot>,
    pub belt_items: Vec<WorldItemSnapshot>,
    pub inventory_items: Vec<WorldItemSnapshot>,
    #[serde(default)]
    pub storage_items: Vec<WorldItemSnapshot>,
    pub equipment_items: Vec<EquipmentItemSnapshot>,
    pub quest_log: Vec<QuestSnapshot>,
    pub active_npc_dialog: Option<NpcDialogSnapshot>,
    pub known_skills: Vec<SkillSnapshot>,
    pub active_buffs: Vec<BuffSnapshot>,
    pub stage5_systems: Stage5SystemsState,
    pub map_transfers: Vec<MapTransferSnapshot>,
    pub interaction_hints: Vec<String>,
}
