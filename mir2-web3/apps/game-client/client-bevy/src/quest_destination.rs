//! Display-only destinations for authoritative quest objectives.
use crate::quest_model::{Quest, QuestStatus, QuestTracker};

pub const BICHON_SAFE_X: i32 = 328;
pub const BICHON_SAFE_Y: i32 = 264;
pub const BICHON_SAFE_RADIUS: i32 = 10;

/// Display-only authored semantics, shared by destination and monster hints.
/// None preserves the legacy packet-label matcher for non-V2 quests.
fn authored_config() -> &'static serde_json::Value {
    static CONFIG: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();
    CONFIG.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../config/quest-guidance/newcomer-journey-v2.json"
        ))
        .expect("bundled V2 guidance")
    })
}

pub(crate) fn authored_quest_definition(quest_index: i32) -> Option<&'static serde_json::Value> {
    let config = authored_config();
    config["quests"]
        .as_array()?
        .iter()
        .find(|quest| quest["id"].as_i64() == Some(i64::from(quest_index)))
}

/// V2 uses loaded NPC object IDs, including its separate growth claims.
pub(crate) fn uses_loaded_npc_ids(quest_index: i32) -> bool {
    if mir2_game_data::periodic_quests::is_periodic(quest_index) {
        return true;
    }
    ["quests", "growthRewards"].iter().any(|key| {
        authored_config()[key].as_array().is_some_and(|quests| {
            quests
                .iter()
                .any(|quest| quest["id"].as_i64() == Some(i64::from(quest_index)))
        })
    })
}

/// Imported V2 task-map declarations resolved to Crystal map identities.
/// Empty means the task has no authored cross-map destination.
pub fn authored_target_map_indices(quest_index: i32) -> Vec<i32> {
    if let Some(quest) = mir2_game_data::periodic_quests::quest(quest_index) {
        return periodic_map_indices(quest.kills.iter().flat_map(|kill| kill.maps.iter()));
    }
    let Some(files) =
        authored_quest_definition(quest_index).and_then(|quest| quest["maps"].as_array())
    else {
        return Vec::new();
    };
    mir2_game_data::crystal_respawn_manifest_ref()
        .maps
        .iter()
        .filter(|map| {
            files
                .iter()
                .any(|file| file.as_str() == Some(map.map_file_name.as_str()))
        })
        .map(|map| map.map_index)
        .collect()
}

fn periodic_map_indices<'a>(files: impl Iterator<Item = &'a String>) -> Vec<i32> {
    let files = files
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    mir2_game_data::crystal_respawn_manifest_ref()
        .maps
        .iter()
        .filter(|map| files.contains(map.map_file_name.as_str()))
        .map(|map| map.map_index)
        .collect()
}

/// Navigate only to maps that can advance an authoritative unfinished kill.
/// Completed objectives must not keep a weekly task pointing at the old area.
pub fn active_target_map_indices(quest: &Quest) -> Vec<i32> {
    if let Some(definition) = mir2_game_data::periodic_quests::quest(quest.quest_index) {
        if quest.status != QuestStatus::InProgress {
            return Vec::new();
        }
        return periodic_map_indices(
            definition
                .kills
                .iter()
                .enumerate()
                .filter(|(index, _)| {
                    quest
                        .objectives
                        .get(*index)
                        .is_some_and(|objective| objective.target > 0 && !objective.is_complete())
                })
                .flat_map(|(_, kill)| kill.maps.iter()),
        );
    }
    authored_target_map_indices(quest.quest_index)
}

pub fn bichon_safe_arrival_pending(tracker: &QuestTracker) -> bool {
    tracker.active_quests.iter().any(|quest| {
        quest.quest_index == 2_110_005
            && quest.status == QuestStatus::InProgress
            && quest
                .objectives
                .iter()
                .any(|objective| objective.target > 0 && objective.current < objective.target)
    })
}

#[cfg(feature = "native-ui")]
pub fn is_bichon_map(map_index: i32) -> bool {
    mir2_game_data::crystal_respawn_manifest_ref()
        .maps
        .iter()
        .any(|map| map.map_index == map_index && map.map_file_name == "0")
}

#[cfg(all(test, feature = "native-ui"))]
mod tests {
    use super::*;

    #[test]
    fn bichon_destination_matches_imported_safe_zone() {
        let map = mir2_game_data::crystal_respawn_manifest_ref()
            .maps
            .iter()
            .find(|map| map.map_file_name == "0")
            .unwrap();
        assert!(is_bichon_map(map.map_index));
        assert_eq!(map.map_index, 1);
        assert!(!is_bichon_map(0));
        assert!(map.safe_zones.iter().any(|zone| {
            zone.location.x == BICHON_SAFE_X
                && zone.location.y == BICHON_SAFE_Y
                && i32::from(zone.size) == BICHON_SAFE_RADIUS
        }));
    }

    #[test]
    fn v2_oma_task_resolves_to_the_imported_oma_map_identity() {
        let oma = mir2_game_data::crystal_respawn_manifest_ref()
            .maps
            .iter()
            .find(|map| map.map_file_name == "D001")
            .unwrap();
        assert_eq!(authored_target_map_indices(2_110_010), vec![oma.map_index]);
        assert!(authored_target_map_indices(-1).is_empty());
    }
}
