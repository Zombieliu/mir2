//! Display-only destinations for authoritative quest objectives.
use crate::quest_model::{QuestStatus, QuestTracker};

pub const BICHON_SAFE_X: i32 = 328;
pub const BICHON_SAFE_Y: i32 = 264;
pub const BICHON_SAFE_RADIUS: i32 = 10;

pub fn bichon_safe_arrival_pending(tracker: &QuestTracker) -> bool {
    tracker.active_quests.iter().any(|quest| {
        quest.quest_index == 2_110_005
            && quest.status == QuestStatus::InProgress
            && quest.objectives.iter().any(|objective| {
                objective.target > 0 && objective.current < objective.target
            })
    })
}

#[cfg(feature = "native-ui")]
pub fn is_bichon_map(map_index: i32) -> bool {
    mir2_game_data::crystal_respawn_manifest_ref().maps.iter().any(|map| {
        map.map_index == map_index && map.map_file_name == "0"
    })
}

#[cfg(all(test, feature = "native-ui"))]
mod tests {
    use super::*;

    #[test]
    fn bichon_destination_matches_imported_safe_zone() {
        let map = mir2_game_data::crystal_respawn_manifest_ref().maps.iter()
            .find(|map| map.map_file_name == "0").unwrap();
        assert!(is_bichon_map(map.map_index));
        assert_eq!(map.map_index, 1);
        assert!(!is_bichon_map(0));
        assert!(map.safe_zones.iter().any(|zone| {
            zone.location.x == BICHON_SAFE_X && zone.location.y == BICHON_SAFE_Y
                && i32::from(zone.size) == BICHON_SAFE_RADIUS
        }));
    }
}
