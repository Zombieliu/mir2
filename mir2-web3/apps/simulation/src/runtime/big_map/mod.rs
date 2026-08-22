use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use mir2_game_data::{
    CrystalMovementTemplate, CrystalNpcInfoTemplate, crystal_npc_info_manifest,
    crystal_respawn_manifest_ref,
};
use mir2_protocol::{
    ClientMapInfo, ClientMovementInfo, ClientNpcInfo, WorldMapIcon, WorldMapSetup,
};

use super::map::runtime_world_map_collision_data;
use super::zone::{ZoneMapMetadata, ZoneNpcTeleportConfig, ZoneNpcTeleportDestination};

pub(super) const MIN_SEARCH_QUERY_CHARS: usize = 3;
pub(super) const MAX_SEARCH_QUERY_CHARS: usize = 64;
const SAFE_DEFAULT_TELEPORT_TO_NPC_COST: i32 = 3_000;
const MAX_TELEPORT_TO_NPC_COST: i128 = 1_000_000;
const MAX_MAP_MOVEMENTS: usize = 128;
const MAX_MAP_NPCS: usize = 128;
const MAX_WORLD_MAP_ICONS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
struct AuthoritativeWorldMapConfig {
    setup: WorldMapSetup,
    teleport_to_npc_cost: i32,
    source_path: Option<PathBuf>,
    setup_source_path: Option<PathBuf>,
    source_diagnostic: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SearchMapHit {
    Map { map_index: i32 },
    Npc { map_index: i32, object_id: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SearchQueryError {
    TooShort,
    TooLong,
}

#[derive(Debug)]
struct BigMapCatalog {
    maps: Vec<BigMapCatalogEntry>,
    npcs: Vec<CrystalNpcInfoTemplate>,
}

#[derive(Debug)]
struct BigMapCatalogEntry {
    map_index: i32,
    map_file_name: String,
    map_title: String,
    big_map: u16,
    mini_map: u16,
    light: u8,
    map_dark_light: u8,
    weather: u16,
    movements: Vec<CrystalMovementTemplate>,
}

fn authoritative_world_map_config() -> &'static AuthoritativeWorldMapConfig {
    static CONFIG: OnceLock<AuthoritativeWorldMapConfig> = OnceLock::new();
    CONFIG.get_or_init(load_authoritative_world_map_config)
}

fn load_authoritative_world_map_config() -> AuthoritativeWorldMapConfig {
    let world_map_path = world_map_ini_candidates()
        .into_iter()
        .find(|candidate| candidate.is_file());
    let setup_path = setup_ini_candidates()
        .into_iter()
        .find(|candidate| candidate.is_file());
    load_authoritative_world_map_config_from_paths(world_map_path.as_deref(), setup_path.as_deref())
}

fn load_authoritative_world_map_config_from_paths(
    world_map_path: Option<&Path>,
    setup_path: Option<&Path>,
) -> AuthoritativeWorldMapConfig {
    let (mut setup, world_map_diagnostic) = match world_map_path {
        Some(path) => match load_world_map_ini(path) {
            Ok(setup) => (setup, format!("world_map_source={}", path.display())),
            Err(error) => (
                WorldMapSetup {
                    enabled: false,
                    icons: Vec::new(),
                },
                format!("world_map_source={} invalid: {error}", path.display()),
            ),
        },
        None => (
            WorldMapSetup {
                enabled: false,
                icons: Vec::new(),
            },
            "world_map_source=missing".to_string(),
        ),
    };

    let (teleport_to_npc_cost, cost_diagnostic) = match setup_path {
        Some(path) => match load_setup_ini(path) {
            Ok(cost) => (
                cost,
                format!(
                    "setup_source={} teleport_to_npc_cost={cost}",
                    path.display()
                ),
            ),
            Err(error) => {
                setup.enabled = false;
                (
                    SAFE_DEFAULT_TELEPORT_TO_NPC_COST,
                    format!(
                        "setup_source={} invalid: {error}; World Map disabled",
                        path.display()
                    ),
                )
            }
        },
        None => {
            setup.enabled = false;
            (
                SAFE_DEFAULT_TELEPORT_TO_NPC_COST,
                "setup_source=missing; World Map disabled".to_string(),
            )
        }
    };

    AuthoritativeWorldMapConfig {
        setup,
        teleport_to_npc_cost,
        source_path: world_map_path.map(Path::to_path_buf),
        setup_source_path: setup_path.map(Path::to_path_buf),
        source_diagnostic: format!("{world_map_diagnostic}; {cost_diagnostic}"),
    }
}

fn world_map_ini_candidates() -> Vec<PathBuf> {
    config_ini_candidates("MIR2_CRYSTAL_WORLD_MAP_INI", "WorldMap.ini")
}

fn setup_ini_candidates() -> Vec<PathBuf> {
    config_ini_candidates("MIR2_CRYSTAL_SETUP_INI", "Setup.ini")
}

fn config_ini_candidates(explicit_variable: &str, file_name: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for variable in ["MIR2_CRYSTAL_SERVER_ROOT", "CRYSTAL_SERVER_ROOT"] {
        if let Ok(root) = env::var(variable) {
            candidates.push(PathBuf::from(root).join("Configs").join(file_name));
        }
    }

    // Runtime discovery must not bake a developer checkout into the binary.
    // Packaged servers can place Configs beside the executable or under their
    // asset bundle; development checkouts are discovered from runtime
    // ancestors on every platform.
    if let Ok(executable) = env::current_exe() {
        if let Some(parent) = executable.parent() {
            candidates.push(parent.join("Configs").join(file_name));
            candidates.push(parent.join("mir2-assets").join("Configs").join(file_name));
        }
    }
    if let Ok(current) = env::current_dir() {
        for ancestor in current.ancestors() {
            candidates.push(
                ancestor
                    .join("Crystal")
                    .join("Build")
                    .join("Server")
                    .join("Debug")
                    .join("Configs")
                    .join(file_name),
            );
            candidates.push(ancestor.join("Configs").join(file_name));
        }
    }
    prioritize_explicit_override(env::var(explicit_variable).ok().as_deref(), candidates)
}

fn prioritize_explicit_override(explicit: Option<&str>, fallback: Vec<PathBuf>) -> Vec<PathBuf> {
    // An explicit override is authoritative. Do not silently fall back to a
    // different checkout when it is missing or malformed.
    explicit
        .map(|path| vec![PathBuf::from(path)])
        .unwrap_or(fallback)
}

fn load_world_map_ini(path: &Path) -> Result<WorldMapSetup, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    parse_world_map_ini(&text)
}

fn parse_world_map_ini(text: &str) -> Result<WorldMapSetup, String> {
    let mut section = String::new();
    let mut values = BTreeMap::<(String, String), String>::new();
    for raw_line in text.lines() {
        let line = raw_line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            section = name.trim().to_ascii_lowercase();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        values.insert(
            (section.clone(), key.trim().to_ascii_lowercase()),
            value.trim().to_string(),
        );
    }

    let enabled = values
        .get(&("setup".to_string(), "enabled".to_string()))
        .map(|value| parse_ini_bool(value))
        .transpose()?
        .unwrap_or(false);
    let mut icons = Vec::new();
    for index in 0..MAX_WORLD_MAP_ICONS {
        let image_key = format!("button{index}imageindex");
        let Some(image_value) = values.get(&("layout".to_string(), image_key)) else {
            break;
        };
        let image_index = image_value
            .parse::<i32>()
            .map_err(|_| format!("invalid Button{index}ImageIndex"))?;
        if image_index == -1 {
            break;
        }
        let title = values
            .get(&("layout".to_string(), format!("button{index}title")))
            .cloned()
            .unwrap_or_default();
        let map_index = values
            .get(&("layout".to_string(), format!("button{index}mapindex")))
            .map(|value| value.parse::<i32>())
            .transpose()
            .map_err(|_| format!("invalid Button{index}MapIndex"))?
            .unwrap_or_default();
        icons.push(WorldMapIcon {
            image_index,
            title,
            map_index,
        });
    }
    Ok(WorldMapSetup { enabled, icons })
}

fn load_setup_ini(path: &Path) -> Result<i32, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    parse_setup_ini(&text)
}

fn parse_setup_ini(text: &str) -> Result<i32, String> {
    let mut section = String::new();
    let mut teleport_cost = None;
    for raw_line in text.lines() {
        let line = raw_line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            section = name.trim().to_ascii_lowercase();
            continue;
        }
        let Some((key, raw_value)) = line.split_once('=') else {
            continue;
        };
        if section != "game" || !key.trim().eq_ignore_ascii_case("TeleportToNPCCost") {
            continue;
        }
        let value = raw_value.split(';').next().unwrap_or_default().trim();
        let value = value
            .parse::<i128>()
            .map_err(|_| "invalid TeleportToNPCCost".to_string())?;
        if value < 0 {
            return Err("TeleportToNPCCost must not be negative".to_string());
        }
        if value > MAX_TELEPORT_TO_NPC_COST {
            return Err(format!(
                "TeleportToNPCCost exceeds safe maximum {MAX_TELEPORT_TO_NPC_COST}"
            ));
        }
        teleport_cost = Some(value as i32);
    }
    teleport_cost.ok_or_else(|| "TeleportToNPCCost is missing from [Game]".to_string())
}

fn parse_ini_bool(value: &str) -> Result<bool, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err("invalid WorldMap Enabled boolean".to_string()),
    }
}

fn catalog() -> &'static BigMapCatalog {
    static CATALOG: OnceLock<BigMapCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        // Keep only Big Map fields. Cloning the full respawn manifest here
        // would duplicate every monster spawn merely to answer map metadata.
        let mut maps = crystal_respawn_manifest_ref()
            .maps
            .iter()
            .map(|map| BigMapCatalogEntry {
                map_index: map.map_index,
                map_file_name: map.map_file_name.clone(),
                map_title: map.map_title.clone(),
                big_map: map.big_map,
                mini_map: map.mini_map,
                light: map.light,
                map_dark_light: map.map_dark_light,
                weather: map.weather_particles,
                movements: map.movements.clone(),
            })
            .collect::<Vec<_>>();
        maps.sort_by_key(|map| map.map_index);

        let mut npcs = crystal_npc_info_manifest().npcs;
        npcs.sort_by_key(|npc| (npc.map_index, npc.big_map_icon, npc.npc_index));

        BigMapCatalog { maps, npcs }
    })
}

pub(super) fn world_map_setup() -> WorldMapSetup {
    authoritative_world_map_config().setup.clone()
}

pub(super) fn teleport_to_npc_cost() -> i32 {
    authoritative_world_map_config().teleport_to_npc_cost
}

#[cfg(test)]
fn authoritative_source_diagnostic() -> String {
    authoritative_world_map_config().source_diagnostic.clone()
}

pub(super) fn authoritative_zone_npc_teleport_config() -> ZoneNpcTeleportConfig {
    static CONFIG: OnceLock<ZoneNpcTeleportConfig> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let config = authoritative_world_map_config();
            let setup = &config.setup;
            let catalog = catalog();
            let maps = catalog
                .maps
                .iter()
                .map(|map| {
                    let metadata = ZoneMapMetadata {
                        map_index: map.map_index,
                        file_name: map.map_file_name.clone(),
                        title: map.map_title.clone(),
                        mini_map: map.mini_map,
                        big_map: map.big_map,
                        lights: map.light,
                        map_dark_light: map.map_dark_light,
                        music: 0,
                        weather: map.weather,
                    };
                    (map.map_file_name.to_ascii_lowercase(), metadata)
                })
                .collect();
            let destinations = catalog
                .npcs
                .iter()
                .filter(|npc| npc.can_teleport_to)
                .filter_map(|npc| {
                    let map_file_name = npc.map_file_name.clone().or_else(|| {
                        catalog
                            .maps
                            .iter()
                            .find(|map| map.map_index == npc.map_index)
                            .map(|map| map.map_file_name.clone())
                    })?;
                    let object_id = npc
                        .loaded_object_id
                        .unwrap_or_else(|| u32::try_from(npc.npc_index.max(0)).unwrap_or_default());
                    (object_id != 0).then_some(ZoneNpcTeleportDestination {
                        map_file_name,
                        object_id,
                    })
                })
                .collect();
            ZoneNpcTeleportConfig {
                enabled: setup.enabled,
                cost: config.teleport_to_npc_cost as u32,
                maps,
                destinations,
            }
        })
        .clone()
}

pub(super) fn normalize_search_query(text: &str) -> Result<String, SearchQueryError> {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let char_count = normalized.chars().count();
    if char_count < MIN_SEARCH_QUERY_CHARS {
        return Err(SearchQueryError::TooShort);
    }
    if char_count > MAX_SEARCH_QUERY_CHARS {
        return Err(SearchQueryError::TooLong);
    }
    Ok(normalized.to_lowercase())
}

pub(super) fn search(text: &str) -> Result<Option<SearchMapHit>, SearchQueryError> {
    let query = normalize_search_query(text)?;
    let catalog = catalog();

    // Crystal searches maps before NPCs (`PlayerObject.SearchMap`) and only
    // considers maps that have a BigMap image. Sorting the imported catalog by
    // map index makes the otherwise insertion-order-dependent first match
    // deterministic.
    if let Some(map) = catalog
        .maps
        .iter()
        .find(|map| map.big_map > 0 && normalize_catalog_text(&map.map_title).starts_with(&query))
    {
        return Ok(Some(SearchMapHit::Map {
            map_index: map.map_index,
        }));
    }

    // Crystal compares against NPCInfo.GameName, i.e. the suffix after the
    // final underscore, and only includes ShowOnBigMap NPCs.
    if let Some(npc) = catalog.npcs.iter().find(|npc| {
        npc.show_on_big_map && normalize_catalog_text(npc_game_name(&npc.name)).starts_with(&query)
    }) {
        return Ok(Some(SearchMapHit::Npc {
            map_index: npc.map_index,
            object_id: npc
                .loaded_object_id
                .unwrap_or_else(|| u32::try_from(npc.npc_index.max(0)).unwrap_or(0)),
        }));
    }

    Ok(None)
}

pub(super) fn client_map_info(map_index: i32) -> Option<ClientMapInfo> {
    let catalog = catalog();
    let map = catalog.maps.iter().find(|map| map.map_index == map_index)?;
    let collision = runtime_world_map_collision_data(&map.map_file_name)?;

    let movements = map
        .movements
        .iter()
        .filter(|movement| movement.show_on_big_map)
        .filter_map(|movement| {
            let destination = catalog
                .maps
                .iter()
                .find(|candidate| candidate.map_index == movement.map_index)?;
            Some(ClientMovementInfo {
                destination: movement.map_index,
                title: destination.map_title.clone(),
                location: movement.source.clone(),
                icon: movement.icon,
            })
        })
        .take(MAX_MAP_MOVEMENTS)
        .collect();

    let npcs = catalog
        .npcs
        .iter()
        .filter(|npc| npc.map_index == map_index && npc.show_on_big_map)
        .take(MAX_MAP_NPCS)
        .map(|npc| ClientNpcInfo {
            index: npc.npc_index,
            file_name: npc.file_name.clone(),
            name: npc.name.clone(),
            map_index: npc.map_index,
            location: npc.location.clone(),
            image: npc.image,
            rate: npc.rate,
            show_on_big_map: npc.show_on_big_map,
            big_map_icon: npc.big_map_icon,
            object_id: npc
                .loaded_object_id
                .unwrap_or_else(|| u32::try_from(npc.npc_index.max(0)).unwrap_or(0)),
            icon: npc.big_map_icon,
            can_teleport_to: npc.can_teleport_to,
        })
        .collect();

    Some(ClientMapInfo {
        title: map.map_title.clone(),
        width: i32::from(collision.collision.map_width),
        height: i32::from(collision.collision.map_height),
        big_map: i32::from(map.big_map),
        movements,
        npcs,
    })
}

fn normalize_catalog_text(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn npc_game_name(name: &str) -> &str {
    name.rsplit('_').next().unwrap_or(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_normalization_is_unicode_aware_and_bounded() {
        assert_eq!(normalize_search_query("  ÄBC  ").unwrap(), "äbc");
        assert_eq!(normalize_search_query("a  b  c").unwrap(), "a b c");
        assert_eq!(
            normalize_search_query("  "),
            Err(SearchQueryError::TooShort)
        );
        assert_eq!(
            normalize_search_query("ab"),
            Err(SearchQueryError::TooShort)
        );
        assert_eq!(
            normalize_search_query(&"x".repeat(MAX_SEARCH_QUERY_CHARS + 1)),
            Err(SearchQueryError::TooLong)
        );
    }

    #[test]
    fn world_map_ini_parser_preserves_disabled_empty_authoritative_shape() {
        let setup = parse_world_map_ini("[Setup]\nEnabled=False\n\n[Layout]\n").unwrap();
        assert!(!setup.enabled);
        assert!(setup.icons.is_empty());
    }

    #[test]
    fn world_map_ini_parser_loads_enabled_icons_in_crystal_sequence() {
        let setup = parse_world_map_ini(
            "[Setup]\nEnabled=True\n[Layout]\nButton0ImageIndex=17\nButton0Title=Bichon\nButton0MapIndex=1\nButton1ImageIndex=23\nButton1Title=Natural Cave\nButton1MapIndex=34\n",
        )
        .unwrap();
        assert!(setup.enabled);
        assert_eq!(setup.icons.len(), 2);
        assert_eq!(setup.icons[0].map_index, 1);
        assert_eq!(setup.icons[1].title, "Natural Cave");
    }

    #[test]
    fn setup_ini_parser_reads_game_teleport_cost_and_inline_comments() {
        assert_eq!(
            parse_setup_ini("[Game]\nTeleportToNPCCost=777 ; authoritative override\n").unwrap(),
            777
        );
    }

    #[test]
    fn setup_ini_parser_rejects_missing_negative_and_oversized_costs() {
        for text in [
            "[Game]\nOtherCost=777\n",
            "[Game]\nTeleportToNPCCost=-1\n",
            "[Game]\nTeleportToNPCCost=1000001\n",
            "[Game]\nTeleportToNPCCost=not-a-number\n",
        ] {
            assert!(
                parse_setup_ini(text).is_err(),
                "input should be rejected: {text}"
            );
        }
    }

    #[test]
    fn explicit_setup_override_is_authoritative_over_runtime_candidates() {
        let fallback = vec![PathBuf::from("runtime/Configs/Setup.ini")];
        let candidates = prioritize_explicit_override(Some("override/Setup.ini"), fallback);
        assert_eq!(candidates, vec![PathBuf::from("override/Setup.ini")]);
    }

    #[test]
    fn setup_override_cost_is_used_and_source_is_recorded() {
        let root = std::env::temp_dir().join(format!(
            "mir2-big-map-setup-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let world_map = root.join("WorldMap.ini");
        let setup = root.join("Setup.ini");
        fs::write(
            &world_map,
            "[Setup]\nEnabled=True\n[Layout]\nButton0ImageIndex=1\nButton0Title=Bichon\nButton0MapIndex=1\n",
        )
        .unwrap();
        fs::write(&setup, "[Game]\nTeleportToNPCCost=777\n").unwrap();

        let loaded = load_authoritative_world_map_config_from_paths(Some(&world_map), Some(&setup));
        assert!(loaded.setup.enabled);
        assert_eq!(loaded.teleport_to_npc_cost, 777);
        assert_eq!(loaded.source_path.as_deref(), Some(world_map.as_path()));
        assert_eq!(loaded.setup_source_path.as_deref(), Some(setup.as_path()));
        assert!(
            loaded
                .source_diagnostic
                .contains("teleport_to_npc_cost=777")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_setup_disables_world_map_and_uses_safe_default() {
        let root = std::env::temp_dir().join(format!(
            "mir2-big-map-invalid-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let world_map = root.join("WorldMap.ini");
        let setup = root.join("Setup.ini");
        fs::write(&world_map, "[Setup]\nEnabled=True\n[Layout]\n").unwrap();
        fs::write(&setup, "[Game]\nTeleportToNPCCost=-5\n").unwrap();

        let loaded = load_authoritative_world_map_config_from_paths(Some(&world_map), Some(&setup));
        assert!(!loaded.setup.enabled);
        assert_eq!(
            loaded.teleport_to_npc_cost,
            SAFE_DEFAULT_TELEPORT_TO_NPC_COST
        );
        assert!(loaded.source_diagnostic.contains("World Map disabled"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn authoritative_fixture_remains_disabled_with_zero_eligible_destinations() {
        let loaded = authoritative_world_map_config();
        assert!(!loaded.setup.enabled);
        assert!(loaded.setup.icons.is_empty());
        assert!(
            loaded
                .source_path
                .as_ref()
                .map(|path| path.is_file())
                .unwrap_or(true)
        );
        assert_eq!(loaded.teleport_to_npc_cost, 3_000);
        assert!(authoritative_source_diagnostic().contains("world_map_source="));
        if let Some(setup_path) = loaded.setup_source_path.as_ref() {
            assert_eq!(
                setup_path.file_name().and_then(|name| name.to_str()),
                Some("Setup.ini")
            );
        }
        let teleport = authoritative_zone_npc_teleport_config();
        assert!(!teleport.enabled);
        assert!(teleport.destinations.is_empty());
    }

    #[test]
    fn catalog_search_uses_stable_map_then_npc_order() {
        assert_eq!(
            search("nAt").unwrap(),
            Some(SearchMapHit::Map { map_index: 34 })
        );
        assert_eq!(
            search("gil").unwrap(),
            Some(SearchMapHit::Npc {
                map_index: 1,
                object_id: 1,
            })
        );
        assert_eq!(search("不存在").unwrap(), None);
    }

    #[test]
    fn bichon_client_map_info_uses_authoritative_catalog() {
        let info = client_map_info(1).expect("Bichon map info should be available");
        assert_eq!(info.title, "BichonProvince");
        assert_eq!((info.width, info.height, info.big_map), (700, 700, 101));
        assert_eq!(info.movements.len(), 6);
        assert_eq!(info.npcs.len(), 40);
        assert!(info.npcs.iter().all(|npc| npc.show_on_big_map));
        assert!(info.npcs.iter().all(|npc| !npc.can_teleport_to));
    }
}
