use bevy_ecs::prelude::World;
use mir2_game_data::crystal_map_events::{
    crystal_map_event_manifest_ref, CrystalMapCoordinateAction, CrystalMapCoordinateComparison,
    CrystalMapCoordinateCondition, CrystalMapCoordinateConditionKind,
    CrystalTypedMapCoordinateBinding,
};
use mir2_game_data::{crystal_map_respawns_by_index, MapBounds};
use mir2_protocol::{ChatType, MirDirection, Point, ServerPacket};

use crate::MapTransferRecord;

use super::map::{crystal_movement_transfer_key, normalize_map_file_name, zone_map_collision_data};
use super::npc_script::player_level;
use super::resources::{MapRuntimeResource, PlayerRuntimeResource};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CrystalMapCoordinateDecision {
    EnterMap(MapTransferRecord),
    Denied {
        message: String,
        chat_type: ChatType,
    },
    /// A source binding exists, but its generated E1 record is invalid. Unknown
    /// data is never admitted to the authoritative movement path.
    FailClosed,
}

pub(super) fn crystal_map_coordinate_decision(
    map_file_name: &str,
    point: &Point,
    level: u16,
    pk_points: i32,
    direction: MirDirection,
) -> Option<CrystalMapCoordinateDecision> {
    let normalized_map = normalize_map_file_name(map_file_name);
    let bindings = crystal_map_event_manifest_ref()
        .typed_map_coordinate_bindings
        .iter()
        .filter(|binding| {
            normalize_map_file_name(&binding.map_id) == normalized_map
                && binding.x == point.x
                && binding.y == point.y
        })
        .collect::<Vec<_>>();
    let binding = match bindings.as_slice() {
        [] => return None,
        [binding] => *binding,
        _ => return Some(CrystalMapCoordinateDecision::FailClosed),
    };

    if !binding
        .conditions
        .iter()
        .all(|condition| condition_matches(condition, level, pk_points))
    {
        return Some(match &binding.on_fail {
            CrystalMapCoordinateAction::LocalMessage {
                message, chat_type, ..
            } if chat_type.eq_ignore_ascii_case("Hint") => CrystalMapCoordinateDecision::Denied {
                message: message.clone(),
                chat_type: ChatType::Hint,
            },
            _ => CrystalMapCoordinateDecision::FailClosed,
        });
    }

    Some(match &binding.on_pass {
        CrystalMapCoordinateAction::EnterMap { .. } => {
            typed_need_move_transfer(binding, point, direction)
                .map(CrystalMapCoordinateDecision::EnterMap)
                .unwrap_or(CrystalMapCoordinateDecision::FailClosed)
        }
        _ => CrystalMapCoordinateDecision::FailClosed,
    })
}

pub(super) fn crystal_map_coordinate_source_cells(
    map_file_name: &str,
) -> impl Iterator<Item = (i32, i32)> + '_ {
    let normalized_map = normalize_map_file_name(map_file_name);
    crystal_map_event_manifest_ref()
        .typed_map_coordinate_bindings
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

fn typed_need_move_transfer(
    binding: &CrystalTypedMapCoordinateBinding,
    point: &Point,
    direction: MirDirection,
) -> Option<MapTransferRecord> {
    let need_move = &binding.need_move;
    if need_move.source != *point
        || normalize_map_file_name(&need_move.source_map_file_name)
            != normalize_map_file_name(&binding.map_id)
        || !need_move_target_is_valid(need_move)
    {
        return None;
    }

    Some(MapTransferRecord {
        key: crystal_movement_transfer_key(
            &need_move.source_map_file_name,
            point.x,
            point.y,
            need_move.target_map_index,
            need_move.destination.x,
            need_move.destination.y,
        ),
        from_map_file_name: need_move.source_map_file_name.clone(),
        from_bounds: MapBounds {
            min_x: point.x,
            max_x: point.x,
            min_y: point.y,
            max_y: point.y,
        },
        to_map_file_name: need_move.target_map_file_name.clone(),
        to_map_title: need_move.target_map_title.clone(),
        to_position: need_move.destination.clone(),
        // Crystal ENTERMAP calls Teleport without replacing the player's
        // facing, unlike the old generic direct-transfer default.
        to_direction: direction,
        conquest_index: need_move.conquest_index,
    })
}

/// Validate a generated `NeedMove` target against the same authoritative map
/// metadata used by the runtime collision layer.  The generator is data-only;
/// an index/file mismatch, missing collision metadata, an out-of-bounds point,
/// or a blocked destination must therefore fail closed instead of becoming an
/// executable transfer.
fn need_move_target_is_valid(
    need_move: &mir2_game_data::crystal_map_events::CrystalNeedMoveBinding,
) -> bool {
    let Some(target) = crystal_map_respawns_by_index(need_move.target_map_index) else {
        return false;
    };
    if normalize_map_file_name(&target.map_file_name)
        != normalize_map_file_name(&need_move.target_map_file_name)
    {
        return false;
    }

    let Some(collision) = zone_map_collision_data(&target.map_file_name) else {
        return false;
    };
    let destination = &need_move.destination;
    destination.x >= collision.bounds.min_x
        && destination.x <= collision.bounds.max_x
        && destination.y >= collision.bounds.min_y
        && destination.y <= collision.bounds.max_y
        && !collision
            .blocked_cells
            .contains(&(destination.x, destination.y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn need_move_rejects_nonzero_out_of_bounds_target() {
        let mut binding = crystal_map_event_manifest_ref()
            .typed_map_coordinate_bindings
            .iter()
            .find(|binding| binding.map_id == "3" && binding.x == 861 && binding.y == 686)
            .expect("Penal Cavern E1 binding")
            .clone();
        binding.need_move.destination = Point { x: -1, y: 1 };

        assert!(
            typed_need_move_transfer(&binding, &Point { x: 861, y: 686 }, MirDirection::Left,)
                .is_none()
        );
    }

    #[test]
    fn need_move_rejects_target_map_index_file_mismatch() {
        let mut binding = crystal_map_event_manifest_ref()
            .typed_map_coordinate_bindings
            .iter()
            .find(|binding| binding.map_id == "3" && binding.x == 861 && binding.y == 686)
            .expect("Penal Cavern E1 binding")
            .clone();
        binding.need_move.target_map_file_name = "not-a-crystal-map".to_string();

        assert!(
            typed_need_move_transfer(&binding, &Point { x: 861, y: 686 }, MirDirection::Left,)
                .is_none()
        );
    }
}

fn condition_matches(
    condition: &CrystalMapCoordinateCondition,
    level: u16,
    pk_points: i32,
) -> bool {
    let left = match condition.kind {
        CrystalMapCoordinateConditionKind::Level => i32::from(level),
        CrystalMapCoordinateConditionKind::PkPoints => pk_points,
    };
    match condition.operator {
        CrystalMapCoordinateComparison::LessThan => left < condition.value,
        CrystalMapCoordinateComparison::GreaterThan => left > condition.value,
        CrystalMapCoordinateComparison::LessThanOrEqual => left <= condition.value,
        CrystalMapCoordinateComparison::GreaterThanOrEqual => left >= condition.value,
        CrystalMapCoordinateComparison::Equal => left == condition.value,
        CrystalMapCoordinateComparison::NotEqual => left != condition.value,
    }
}
