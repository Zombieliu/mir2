//! Read-only route planning over imported Crystal map-transition data.
//!
//! A route only describes the next ordinary entrance. It never changes the
//! current map or treats reaching an entrance as an acknowledged map change.

use std::collections::{BTreeMap, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestRouteStep {
    pub current_map_index: i32,
    pub current_map_title: String,
    pub entrance_x: i32,
    pub entrance_y: i32,
    pub next_map_index: i32,
    pub next_map_title: String,
    pub remaining_hops: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestRoute {
    AtDestination { map_title: String },
    NextStep(QuestRouteStep),
    Unavailable,
}

/// Resolve one ordinary, player-walkable next entrance using the imported
/// Crystal map graph. The graph is directional: no reverse exit is inferred.
pub fn resolve(current_map_index: Option<i32>, target_map_indices: &[i32]) -> QuestRoute {
    let Some(current_map_index) = current_map_index else {
        return QuestRoute::Unavailable;
    };
    let manifest = mir2_game_data::crystal_respawn_manifest_ref();
    let maps = manifest
        .maps
        .iter()
        .map(|map| (map.map_index, map))
        .collect::<BTreeMap<_, _>>();
    let Some(current) = maps.get(&current_map_index) else {
        return QuestRoute::Unavailable;
    };
    if target_map_indices.contains(&current_map_index) {
        return QuestRoute::AtDestination {
            map_title: current.map_title.clone(),
        };
    }

    #[derive(Clone)]
    struct Visit {
        map_index: i32,
        first_step: QuestRouteStep,
        hops: usize,
    }

    let mut visited = BTreeMap::new();
    let mut queue = VecDeque::new();
    visited.insert(current_map_index, 0usize);
    for movement in ordinary_entrances(current) {
        if !maps.contains_key(&movement.map_index) {
            continue;
        }
        let next = maps[&movement.map_index];
        let step = QuestRouteStep {
            current_map_index,
            current_map_title: current.map_title.clone(),
            entrance_x: movement.source.x,
            entrance_y: movement.source.y,
            next_map_index: next.map_index,
            next_map_title: next.map_title.clone(),
            remaining_hops: 1,
        };
        if target_map_indices.contains(&movement.map_index) {
            return QuestRoute::NextStep(step);
        }
        if visited.insert(movement.map_index, 1).is_none() {
            queue.push_back(Visit {
                map_index: movement.map_index,
                first_step: step,
                hops: 1,
            });
        }
    }

    while let Some(visit) = queue.pop_front() {
        let Some(map) = maps.get(&visit.map_index) else {
            continue;
        };
        for movement in ordinary_entrances(map) {
            if !maps.contains_key(&movement.map_index) {
                continue;
            }
            let hops = visit.hops.saturating_add(1);
            if visited.insert(movement.map_index, hops).is_some() {
                continue;
            }
            let mut first_step = visit.first_step.clone();
            first_step.remaining_hops = hops;
            if target_map_indices.contains(&movement.map_index) {
                return QuestRoute::NextStep(first_step);
            }
            queue.push_back(Visit {
                map_index: movement.map_index,
                first_step,
                hops,
            });
        }
    }

    QuestRoute::Unavailable
}

fn ordinary_entrances(
    map: &mir2_game_data::CrystalRespawnMap,
) -> Vec<&mir2_game_data::CrystalMovementTemplate> {
    let mut entrances = map.movements.iter().filter(|movement| {
        !movement.need_hole && !movement.need_move && movement.conquest_index == 0
    }).collect::<Vec<_>>();
    // Big-map-visible edges are preferable for player guidance, but a normal
    // source edge remains valid when Crystal does not draw its map icon.
    entrances.sort_by_key(|movement| !movement.show_on_big_map);
    entrances
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map_index(file_name: &str) -> i32 {
        mir2_game_data::crystal_respawn_manifest_ref()
            .maps
            .iter()
            .find(|map| map.map_file_name == file_name)
            .map(|map| map.map_index)
            .expect("imported Crystal map")
    }

    #[test]
    fn bichon_to_oma_uses_the_imported_public_entrance_not_the_cave_landing() {
        let bichon = map_index("0");
        let oma = map_index("D001");
        assert_eq!(
            resolve(Some(bichon), &[oma]),
            QuestRoute::NextStep(QuestRouteStep {
                current_map_index: bichon,
                current_map_title: "BichonProvince".into(),
                entrance_x: 147,
                entrance_y: 33,
                next_map_index: oma,
                next_map_title: "OmaCave_1F".into(),
                remaining_hops: 1,
            })
        );
    }

    #[test]
    fn target_switch_and_authoritative_map_change_recompute_the_next_step() {
        let bichon = map_index("0");
        let oma = map_index("D001");
        assert!(matches!(resolve(Some(bichon), &[oma]), QuestRoute::NextStep(_)));
        assert_eq!(
            resolve(Some(oma), &[oma]),
            QuestRoute::AtDestination {
                map_title: "OmaCave_1F".into()
            }
        );
        assert_eq!(resolve(Some(bichon), &[bichon]), QuestRoute::AtDestination {
            map_title: "BichonProvince".into()
        });
    }

    #[test]
    fn hidden_normal_exit_remains_legal_but_visible_exit_is_preferred() {
        let oma = mir2_game_data::crystal_respawn_manifest_ref().maps.iter()
            .find(|map| map.map_file_name == "D001").unwrap();
        let entrances = ordinary_entrances(oma);
        assert!(entrances.iter().any(|movement| !movement.show_on_big_map));
        assert!(entrances.iter().any(|movement| movement.show_on_big_map));
        assert!(entrances.first().is_some_and(|movement| movement.show_on_big_map));
    }

    #[test]
    fn unknown_or_unroutable_targets_never_fabricate_an_entrance() {
        assert_eq!(resolve(None, &[39]), QuestRoute::Unavailable);
        assert_eq!(resolve(Some(map_index("0")), &[i32::MAX]), QuestRoute::Unavailable);
    }
}
