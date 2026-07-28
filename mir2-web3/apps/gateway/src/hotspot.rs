use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::ZoneId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotMapPolicy {
    pub target_players_per_line: usize,
    pub maximum_players_per_line: usize,
    pub maximum_lines: u16,
    pub scale_in_grace_ms: u64,
}

impl HotMapPolicy {
    pub fn validate(&self) -> Result<(), String> {
        if self.target_players_per_line == 0 {
            return Err("target players per line must be positive".to_string());
        }
        if self.maximum_players_per_line < self.target_players_per_line {
            return Err(
                "maximum players per line must be at least the target players per line".to_string(),
            );
        }
        if self.maximum_lines == 0 {
            return Err("maximum lines must be positive".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotMapPlacementRequest {
    pub session_key: String,
    pub map_file_name: String,
    pub affinity_key: Option<String>,
    pub explicit_line: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotMapPlacement {
    pub session_key: String,
    pub map_file_name: String,
    pub line_id: u16,
    pub zone_id: ZoneId,
    pub players_on_line: usize,
    pub line_count: usize,
    pub overflowed_target: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HotMapLineSnapshot {
    pub map_file_name: String,
    pub total_players: usize,
    pub line_players: BTreeMap<u16, usize>,
    pub affinity_count: usize,
}

#[derive(Debug, Default)]
struct HotMapState {
    assignments: BTreeMap<(String, String), u16>,
    affinities: BTreeMap<(String, String), u16>,
    session_affinities: BTreeMap<(String, String), String>,
    lines: BTreeMap<String, BTreeMap<u16, BTreeSet<String>>>,
    empty_since_ms: BTreeMap<(String, u16), u64>,
}

#[derive(Debug)]
pub struct HotMapLineScheduler {
    policies: BTreeMap<String, HotMapPolicy>,
    state: Mutex<HotMapState>,
}

impl HotMapLineScheduler {
    pub fn new(policies: BTreeMap<String, HotMapPolicy>) -> Result<Self, String> {
        for (map, policy) in &policies {
            if map.trim().is_empty() {
                return Err("hot-map policy map name must not be empty".to_string());
            }
            policy
                .validate()
                .map_err(|error| format!("invalid hot-map policy for {map}: {error}"))?;
        }
        Ok(Self {
            policies,
            state: Mutex::new(HotMapState::default()),
        })
    }

    pub fn place(&self, request: HotMapPlacementRequest) -> Result<HotMapPlacement, String> {
        validate_request(&request)?;
        let policy = self.policies.get(&request.map_file_name).ok_or_else(|| {
            format!(
                "map {} is not configured as a hot map",
                request.map_file_name
            )
        })?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| "hot-map scheduler mutex poisoned".to_string())?;
        let assignment_key = (request.map_file_name.clone(), request.session_key.clone());
        if let Some(line_id) = state.assignments.get(&assignment_key).copied() {
            return Ok(placement(&state, &request, line_id, policy));
        }

        let affinity_line = request.affinity_key.as_ref().and_then(|affinity| {
            state
                .affinities
                .get(&(request.map_file_name.clone(), affinity.clone()))
                .copied()
        });
        let line_id = match (request.explicit_line, affinity_line) {
            (Some(explicit), Some(affinity)) if explicit != affinity => {
                return Err(format!(
                    "explicit line {explicit} conflicts with affinity line {affinity}"
                ));
            }
            (Some(explicit), _) => {
                if explicit == 0 || explicit > policy.maximum_lines {
                    return Err(format!(
                        "explicit line {explicit} is outside 1..={}",
                        policy.maximum_lines
                    ));
                }
                explicit
            }
            (None, Some(affinity)) => affinity,
            (None, None) => choose_line(&state, &request.map_file_name, policy),
        };

        let players_on_line = state
            .lines
            .get(&request.map_file_name)
            .and_then(|lines| lines.get(&line_id))
            .map(BTreeSet::len)
            .unwrap_or_default();
        if players_on_line >= policy.maximum_players_per_line {
            return Err(format!(
                "line {line_id} reached its hard capacity {}",
                policy.maximum_players_per_line
            ));
        }
        state
            .lines
            .entry(request.map_file_name.clone())
            .or_default()
            .entry(line_id)
            .or_default()
            .insert(request.session_key.clone());
        state.assignments.insert(assignment_key.clone(), line_id);
        state
            .empty_since_ms
            .remove(&(request.map_file_name.clone(), line_id));
        if let Some(affinity) = request.affinity_key.as_ref() {
            state
                .affinities
                .insert((request.map_file_name.clone(), affinity.clone()), line_id);
            state
                .session_affinities
                .insert(assignment_key, affinity.clone());
        }
        Ok(placement(&state, &request, line_id, policy))
    }

    pub fn release(
        &self,
        map_file_name: &str,
        session_key: &str,
        now_ms: u64,
    ) -> Result<bool, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "hot-map scheduler mutex poisoned".to_string())?;
        let assignment_key = (map_file_name.to_string(), session_key.to_string());
        let Some(line_id) = state.assignments.remove(&assignment_key) else {
            return Ok(false);
        };
        if let Some(affinity) = state.session_affinities.remove(&assignment_key) {
            let affinity_still_active =
                state
                    .session_affinities
                    .iter()
                    .any(|((assigned_map, _), assigned_affinity)| {
                        assigned_map == map_file_name && assigned_affinity == &affinity
                    });
            if !affinity_still_active {
                state
                    .affinities
                    .remove(&(map_file_name.to_string(), affinity));
            }
        }
        let became_empty = state
            .lines
            .get_mut(map_file_name)
            .and_then(|lines| lines.get_mut(&line_id))
            .is_some_and(|sessions| {
                sessions.remove(session_key);
                sessions.is_empty()
            });
        if became_empty && line_id > 1 {
            state
                .empty_since_ms
                .insert((map_file_name.to_string(), line_id), now_ms);
        }
        Ok(true)
    }

    /// Remove only empty lines after a grace period. Live sessions are never
    /// migrated implicitly, preserving party/guild-war visibility.
    pub fn reconcile(&self, now_ms: u64) -> Result<usize, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "hot-map scheduler mutex poisoned".to_string())?;
        let removable = state
            .empty_since_ms
            .iter()
            .filter_map(|((map, line), empty_since_ms)| {
                let policy = self.policies.get(map)?;
                let empty = state
                    .lines
                    .get(map)
                    .and_then(|lines| lines.get(line))
                    .is_some_and(BTreeSet::is_empty);
                (empty && now_ms.saturating_sub(*empty_since_ms) >= policy.scale_in_grace_ms)
                    .then_some((map.clone(), *line))
            })
            .collect::<Vec<_>>();
        for (map, line) in &removable {
            if let Some(lines) = state.lines.get_mut(map) {
                lines.remove(line);
            }
            state.empty_since_ms.remove(&(map.clone(), *line));
        }
        let active_lines = state
            .assignments
            .iter()
            .map(|((map, _), line)| (map.clone(), *line))
            .collect::<BTreeSet<_>>();
        state
            .affinities
            .retain(|(map, _), line| active_lines.contains(&(map.clone(), *line)));
        Ok(removable.len())
    }

    pub fn snapshot(&self, map_file_name: &str) -> Result<HotMapLineSnapshot, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "hot-map scheduler mutex poisoned".to_string())?;
        let line_players = state
            .lines
            .get(map_file_name)
            .map(|lines| {
                lines
                    .iter()
                    .map(|(line, sessions)| (*line, sessions.len()))
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default();
        Ok(HotMapLineSnapshot {
            map_file_name: map_file_name.to_string(),
            total_players: line_players.values().sum(),
            line_players,
            affinity_count: state
                .affinities
                .keys()
                .filter(|(map, _)| map == map_file_name)
                .count(),
        })
    }
}

fn choose_line(state: &HotMapState, map: &str, policy: &HotMapPolicy) -> u16 {
    let lines = state.lines.get(map);
    let existing_count = lines.map(BTreeMap::len).unwrap_or_default();
    if existing_count == 0 {
        return 1;
    }
    if let Some((line, _)) = lines.and_then(|lines| {
        lines
            .iter()
            .filter(|(_, sessions)| sessions.len() < policy.target_players_per_line)
            .min_by_key(|(line, sessions)| (Reverse(sessions.len()), **line))
    }) {
        return *line;
    }
    if existing_count < usize::from(policy.maximum_lines) {
        return (1..=policy.maximum_lines)
            .find(|line| lines.is_none_or(|lines| !lines.contains_key(line)))
            .unwrap_or(policy.maximum_lines);
    }
    lines
        .and_then(|lines| {
            lines
                .iter()
                .min_by_key(|(line, sessions)| (sessions.len(), **line))
                .map(|(line, _)| *line)
        })
        .unwrap_or(1)
}

fn placement(
    state: &HotMapState,
    request: &HotMapPlacementRequest,
    line_id: u16,
    policy: &HotMapPolicy,
) -> HotMapPlacement {
    let lines = state.lines.get(&request.map_file_name);
    let players_on_line = lines
        .and_then(|lines| lines.get(&line_id))
        .map(BTreeSet::len)
        .unwrap_or_default();
    HotMapPlacement {
        session_key: request.session_key.clone(),
        map_file_name: request.map_file_name.clone(),
        line_id,
        zone_id: ZoneId::new(format!("map:{}:line:{line_id}", request.map_file_name)),
        players_on_line,
        line_count: lines.map(BTreeMap::len).unwrap_or_default(),
        overflowed_target: players_on_line > policy.target_players_per_line,
    }
}

fn validate_request(request: &HotMapPlacementRequest) -> Result<(), String> {
    if request.session_key.trim().is_empty() {
        return Err("hot-map session key must not be empty".to_string());
    }
    if request.map_file_name.trim().is_empty() {
        return Err("hot-map file name must not be empty".to_string());
    }
    if request
        .affinity_key
        .as_ref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err("hot-map affinity key must not be empty".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scheduler() -> HotMapLineScheduler {
        HotMapLineScheduler::new(BTreeMap::from([(
            "0".to_string(),
            HotMapPolicy {
                target_players_per_line: 50,
                maximum_players_per_line: 64,
                maximum_lines: 8,
                scale_in_grace_ms: 1_000,
            },
        )]))
        .unwrap()
    }

    fn request(index: usize) -> HotMapPlacementRequest {
        HotMapPlacementRequest {
            session_key: format!("player-{index}"),
            map_file_name: "0".to_string(),
            affinity_key: None,
            explicit_line: None,
        }
    }

    #[test]
    fn three_hundred_players_create_six_bounded_lines() {
        let scheduler = scheduler();
        for index in 0..300 {
            scheduler.place(request(index)).unwrap();
        }
        let snapshot = scheduler.snapshot("0").unwrap();
        assert_eq!(snapshot.total_players, 300);
        assert_eq!(snapshot.line_players.len(), 6);
        assert!(snapshot.line_players.values().all(|players| *players == 50));
    }

    #[test]
    fn transient_empty_lines_do_not_fragment_final_hotspot_placement() {
        let scheduler = scheduler();
        for index in 0..400 {
            scheduler.place(request(index)).unwrap();
        }
        for index in 0..400 {
            scheduler
                .release("0", &format!("player-{index}"), 1_000)
                .unwrap();
        }
        for index in 0..300 {
            scheduler.place(request(index)).unwrap();
        }

        let snapshot = scheduler.snapshot("0").unwrap();
        let active_lines = snapshot
            .line_players
            .values()
            .filter(|players| **players > 0)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(snapshot.total_players, 300);
        assert_eq!(active_lines.len(), 6);
        assert!(active_lines.iter().all(|players| *players == 50));
        assert_eq!(snapshot.line_players.get(&7), Some(&0));
        assert_eq!(snapshot.line_players.get(&8), Some(&0));
    }

    #[test]
    fn affinity_and_explicit_line_are_stable() {
        let scheduler = scheduler();
        let first = scheduler
            .place(HotMapPlacementRequest {
                affinity_key: Some("party-7".to_string()),
                explicit_line: Some(4),
                ..request(1)
            })
            .unwrap();
        let second = scheduler
            .place(HotMapPlacementRequest {
                affinity_key: Some("party-7".to_string()),
                ..request(2)
            })
            .unwrap();
        assert_eq!(first.line_id, 4);
        assert_eq!(second.line_id, 4);
        assert_eq!(scheduler.place(request(1)).unwrap().line_id, 4);
    }

    #[test]
    fn affinity_pin_is_removed_after_its_last_session_leaves() {
        let scheduler = scheduler();
        scheduler
            .place(HotMapPlacementRequest {
                affinity_key: Some("party-7".to_string()),
                explicit_line: Some(4),
                ..request(1)
            })
            .unwrap();
        scheduler.release("0", "player-1", 1_000).unwrap();
        let rejoined = scheduler
            .place(HotMapPlacementRequest {
                affinity_key: Some("party-7".to_string()),
                explicit_line: Some(3),
                ..request(2)
            })
            .unwrap();
        assert_eq!(rejoined.line_id, 3);
    }

    #[test]
    fn only_empty_lines_scale_in_after_grace() {
        let scheduler = scheduler();
        let placed = scheduler
            .place(HotMapPlacementRequest {
                explicit_line: Some(3),
                ..request(1)
            })
            .unwrap();
        assert_eq!(placed.line_id, 3);
        assert!(scheduler.release("0", "player-1", 5_000).unwrap());
        assert_eq!(scheduler.reconcile(5_999).unwrap(), 0);
        assert_eq!(scheduler.reconcile(6_000).unwrap(), 1);
        assert!(!scheduler
            .snapshot("0")
            .unwrap()
            .line_players
            .contains_key(&3));
    }

    #[test]
    fn hard_capacity_rejects_overflow_instead_of_overloading_a_line() {
        let scheduler = HotMapLineScheduler::new(BTreeMap::from([(
            "0".to_string(),
            HotMapPolicy {
                target_players_per_line: 1,
                maximum_players_per_line: 1,
                maximum_lines: 2,
                scale_in_grace_ms: 1_000,
            },
        )]))
        .unwrap();
        scheduler.place(request(1)).unwrap();
        scheduler.place(request(2)).unwrap();
        let error = scheduler.place(request(3)).unwrap_err();
        assert!(error.contains("hard capacity"));
        assert_eq!(scheduler.snapshot("0").unwrap().total_players, 2);
    }
}
