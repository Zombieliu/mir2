use bevy_ecs::prelude::World;
use mir2_game_data::crystal_map_events::{
    crystal_map_event_manifest_ref, CrystalMapCoordinateBinding,
};
use mir2_game_data::{crystal_map_respawns_by_index, crystal_map_respawns_ref, MapBounds};
use mir2_protocol::{ChatType, MirDirection, Point, ServerPacket};

use crate::MapTransferRecord;

use super::map::{crystal_movement_transfer_key, normalize_map_file_name};
use super::npc_script::player_level;
use super::resources::{MapRuntimeResource, PlayerRuntimeResource};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CrystalMapCoordinateDecision {
    EnterMap(MapTransferRecord),
    Denied {
        message: String,
        chat_type: ChatType,
    },
    /// A source binding exists, but its imported script or matching NeedMove
    /// record cannot be executed safely. Unknown data is never admitted.
    FailClosed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScriptPhase {
    None,
    If,
    Act,
    ElseAct,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedMapCoordinateScript {
    conditions_pass: bool,
    enter_map: bool,
    else_message: Option<(String, ChatType)>,
}

pub(super) fn crystal_map_coordinate_decision(
    map_file_name: &str,
    point: &Point,
    level: u16,
    pk_points: i32,
    direction: MirDirection,
) -> Option<CrystalMapCoordinateDecision> {
    let normalized_map = normalize_map_file_name(map_file_name);
    let binding = crystal_map_event_manifest_ref()
        .map_coordinates
        .iter()
        .find(|binding| {
            normalize_map_file_name(&binding.map_id) == normalized_map
                && binding.x == point.x
                && binding.y == point.y
        })?;

    let Some(script) = parse_map_coordinate_script(binding, level, pk_points) else {
        return Some(CrystalMapCoordinateDecision::FailClosed);
    };
    if !script.conditions_pass {
        return Some(match script.else_message {
            Some((message, chat_type)) => {
                CrystalMapCoordinateDecision::Denied { message, chat_type }
            }
            None => CrystalMapCoordinateDecision::FailClosed,
        });
    }
    if !script.enter_map {
        return Some(CrystalMapCoordinateDecision::FailClosed);
    }

    Some(
        need_move_transfer_for_coordinate(map_file_name, point, direction)
            .map(CrystalMapCoordinateDecision::EnterMap)
            .unwrap_or(CrystalMapCoordinateDecision::FailClosed),
    )
}

pub(super) fn crystal_map_coordinate_source_cells(
    map_file_name: &str,
) -> impl Iterator<Item = (i32, i32)> + '_ {
    let normalized_map = normalize_map_file_name(map_file_name);
    crystal_map_event_manifest_ref()
        .map_coordinates
        .iter()
        .filter(move |binding| normalize_map_file_name(&binding.map_id) == normalized_map)
        .map(|binding| (binding.x, binding.y))
}

pub(super) fn authorized_map_coordinate_transfers(world: &World) -> Vec<MapTransferRecord> {
    let map_file_name = world
        .resource::<MapRuntimeResource>()
        .current_map
        .file_name
        .clone();
    let level = u16::try_from(player_level(world)).unwrap_or_default();
    let pk_points = world.resource::<PlayerRuntimeResource>().pk_points;
    let direction = world.resource::<PlayerRuntimeResource>().player_direction;

    crystal_map_coordinate_source_cells(&map_file_name)
        .filter_map(|(x, y)| {
            match crystal_map_coordinate_decision(
                &map_file_name,
                &Point { x, y },
                level,
                pk_points,
                direction,
            ) {
                Some(CrystalMapCoordinateDecision::EnterMap(transfer)) => Some(transfer),
                _ => None,
            }
        })
        .collect()
}

pub(super) fn map_coordinate_hint_packets_for_path(
    world: &World,
    path: &[Point],
) -> Vec<ServerPacket> {
    let map_file_name = world
        .resource::<MapRuntimeResource>()
        .current_map
        .file_name
        .clone();
    let level = u16::try_from(player_level(world)).unwrap_or_default();
    let pk_points = world.resource::<PlayerRuntimeResource>().pk_points;
    let direction = world.resource::<PlayerRuntimeResource>().player_direction;

    map_coordinate_hint_packets(&map_file_name, path, level, pk_points, direction)
}

pub(super) fn map_coordinate_hint_packets(
    map_file_name: &str,
    path: &[Point],
    level: u16,
    pk_points: i32,
    direction: MirDirection,
) -> Vec<ServerPacket> {
    path.iter()
        .filter_map(|point| {
            match crystal_map_coordinate_decision(map_file_name, point, level, pk_points, direction)
            {
                Some(CrystalMapCoordinateDecision::Denied { message, chat_type }) => {
                    Some(ServerPacket::Chat { message, chat_type })
                }
                _ => None,
            }
        })
        .collect()
}

fn need_move_transfer_for_coordinate(
    map_file_name: &str,
    point: &Point,
    direction: MirDirection,
) -> Option<MapTransferRecord> {
    let source_map = crystal_map_respawns_ref(map_file_name)?;
    let movement = source_map
        .movements
        .iter()
        .find(|movement| movement.need_move && movement.source == *point)?;
    if movement.destination.x == 0 && movement.destination.y == 0 {
        return None;
    }
    let target = crystal_map_respawns_by_index(movement.map_index)?;

    Some(MapTransferRecord {
        key: crystal_movement_transfer_key(
            &source_map.map_file_name,
            point.x,
            point.y,
            movement.map_index,
            movement.destination.x,
            movement.destination.y,
        ),
        from_map_file_name: source_map.map_file_name.clone(),
        from_bounds: MapBounds {
            min_x: point.x,
            max_x: point.x,
            min_y: point.y,
            max_y: point.y,
        },
        to_map_file_name: target.map_file_name,
        to_map_title: target.map_title,
        to_position: movement.destination.clone(),
        // Crystal ENTERMAP calls Teleport without replacing the player's
        // facing, unlike the old generic direct-transfer default.
        to_direction: direction,
        conquest_index: movement.conquest_index,
    })
}

fn parse_map_coordinate_script(
    binding: &CrystalMapCoordinateBinding,
    level: u16,
    pk_points: i32,
) -> Option<ParsedMapCoordinateScript> {
    let mut phase = ScriptPhase::None;
    let mut saw_condition = false;
    let mut conditions_pass = true;
    let mut enter_map = false;
    let mut else_message = None;

    for source_line in &binding.resolved_section.lines {
        let line = source_line.text.trim();
        if line.is_empty() || matches!(line, "{" | "}") {
            continue;
        }
        match line.to_ascii_uppercase().as_str() {
            "#IF" => {
                phase = ScriptPhase::If;
                continue;
            }
            "#ACT" => {
                phase = ScriptPhase::Act;
                continue;
            }
            "#ELSEACT" => {
                phase = ScriptPhase::ElseAct;
                continue;
            }
            _ => {}
        }

        match phase {
            ScriptPhase::If => {
                saw_condition = true;
                conditions_pass &= evaluate_condition(line, level, pk_points)?;
            }
            ScriptPhase::Act => {
                if line.eq_ignore_ascii_case("ENTERMAP") {
                    enter_map = true;
                } else {
                    return None;
                }
            }
            ScriptPhase::ElseAct => {
                else_message = Some(parse_local_message(line)?);
            }
            ScriptPhase::None => return None,
        }
    }

    saw_condition.then_some(ParsedMapCoordinateScript {
        conditions_pass,
        enter_map,
        else_message,
    })
}

fn evaluate_condition(line: &str, level: u16, pk_points: i32) -> Option<bool> {
    let parts = line.split_whitespace().collect::<Vec<_>>();
    if parts.len() != 3 {
        return None;
    }
    let left = match parts[0].to_ascii_uppercase().as_str() {
        "LEVEL" => i32::from(level),
        "CHECKPKPOINT" => pk_points,
        _ => return None,
    };
    let right = parts[2].parse::<i32>().ok()?;
    compare_i32(parts[1], left, right)
}

fn compare_i32(operator: &str, left: i32, right: i32) -> Option<bool> {
    match operator {
        "<" => Some(left < right),
        ">" => Some(left > right),
        "<=" => Some(left <= right),
        ">=" => Some(left >= right),
        "==" => Some(left == right),
        "!=" => Some(left != right),
        _ => None,
    }
}

fn parse_local_message(line: &str) -> Option<(String, ChatType)> {
    let command_end = line.find(char::is_whitespace)?;
    if !line[..command_end].eq_ignore_ascii_case("LocalMessage") {
        return None;
    }
    let remainder = line[command_end..].trim();
    let message_start = remainder.find('"')? + 1;
    let message_end = remainder[message_start..].find('"')? + message_start;
    let message = remainder[message_start..message_end].to_string();
    let chat_type = remainder[message_end + 1..].trim();
    if !chat_type.eq_ignore_ascii_case("Hint") {
        return None;
    }
    Some((message, ChatType::Hint))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_six_active_bindings_resolve_fail_closed_conditions_and_need_move_targets() {
        let cases = [
            ("3", Point { x: 861, y: 686 }, 1, 199, false, "D1801"),
            ("3", Point { x: 862, y: 687 }, 1, 200, true, "D1801"),
            (
                "DogYoArena2",
                Point { x: 117, y: 26 },
                49,
                0,
                false,
                "DogYoHyun",
            ),
            (
                "DogYoArena2",
                Point { x: 118, y: 27 },
                50,
                0,
                true,
                "DogYoHyun",
            ),
            (
                "DogYoArena2",
                Point { x: 119, y: 28 },
                49,
                0,
                false,
                "DogYoHyun",
            ),
            (
                "DogYoArena2",
                Point { x: 119, y: 29 },
                50,
                0,
                true,
                "DogYoHyun",
            ),
        ];

        for (map, point, level, pk_points, allowed, target) in cases {
            let decision = crystal_map_coordinate_decision(
                map,
                &point,
                level,
                pk_points,
                MirDirection::UpLeft,
            )
            .expect("active coordinate should resolve");
            match (allowed, decision) {
                (true, CrystalMapCoordinateDecision::EnterMap(transfer)) => {
                    assert_eq!(transfer.to_map_file_name, target);
                    assert_eq!(transfer.to_direction, MirDirection::UpLeft);
                }
                (false, CrystalMapCoordinateDecision::Denied { chat_type, .. }) => {
                    assert_eq!(chat_type, ChatType::Hint);
                }
                (_, other) => panic!("unexpected decision for {map} {point:?}: {other:?}"),
            }
        }
    }
}
