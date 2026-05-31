//! In-game GM (`@`) chat commands.
//!
//! Crystal lets privileged players issue `@`-prefixed chat commands (e.g.
//! `@LEVEL`, `@MOVE`). This module provides the dispatch + a first batch of
//! commands, gated strictly on [`PlayerPermissionResource::is_gm`]. A non-GM
//! typing `@foo` gets the normal chat path (no command leakage, no error that
//! reveals command existence).
//!
//! Effects are applied to the live session world and acknowledged with a system
//! chat line, plus the relevant client refresh packet (UserLocation, LevelChanged,
//! ObjectGold, ObjectHealth) so the client stays in sync without a relog.

use mir2_protocol::{Point, ServerPacket};

use super::components::{current_player_object_id, player_entity, PlayerVitals, Position};
use super::movement::current_location;
use super::resources::{PlayerPermissionResource, PlayerRuntimeResource, SessionResource};
use bevy_ecs::prelude::World;

/// True when `message` is a GM command attempt (`@` prefix, non-empty body).
pub(super) fn is_gm_command(message: &str) -> bool {
    let trimmed = message.trim_start();
    trimmed.starts_with('@') && trimmed.len() > 1
}

fn gm_feedback(world: &World, text: impl Into<String>) -> ServerPacket {
    ServerPacket::ObjectChat {
        object_id: current_player_object_id(world).unwrap_or(0),
        text: text.into(),
        chat_type: mir2_protocol::ChatType::Hint,
    }
}

/// Dispatch a GM `@` command. Returns `None` when the caller is not a GM (the
/// chat handler then falls through to normal chat), or `Some(packets)` with the
/// command's acknowledgement + client-refresh packets.
pub(super) fn dispatch_gm_command(world: &mut World, message: &str) -> Option<Vec<ServerPacket>> {
    if !world.resource::<PlayerPermissionResource>().is_gm() {
        return None;
    }
    let body = message.trim_start().strip_prefix('@')?.trim();
    let mut parts = body.split_whitespace();
    let command = parts.next()?.to_ascii_uppercase();
    let args: Vec<&str> = parts.collect();

    let packets = match command.as_str() {
        "HELP" | "COMMANDS" => vec![gm_feedback(
            world,
            "GM commands: @HELP @WHERE @LEVEL <n> @GOLD <n> @HEAL @MOVE <x> <y>",
        )],
        "WHERE" | "POS" => {
            let location = current_location(world);
            vec![gm_feedback(
                world,
                format!(
                    "map={} pos=({},{}) dir={:?}",
                    world
                        .resource::<super::resources::MapRuntimeResource>()
                        .current_map
                        .file_name,
                    location.position.x,
                    location.position.y,
                    location.direction,
                ),
            )]
        }
        "LEVEL" | "SETLEVEL" => gm_set_level(world, args.first().copied()),
        "GOLD" | "SETGOLD" => gm_set_gold(world, args.first().copied()),
        "HEAL" | "FULLHEAL" => gm_heal(world),
        "MOVE" | "POS2" => gm_move(world, args.first().copied(), args.get(1).copied()),
        other => vec![gm_feedback(world, format!("unknown GM command: @{other}"))],
    };
    Some(packets)
}

fn parse_u16(arg: Option<&str>) -> Option<u16> {
    arg.and_then(|value| value.parse::<u16>().ok())
}

fn gm_set_level(world: &mut World, arg: Option<&str>) -> Vec<ServerPacket> {
    let Some(level) = parse_u16(arg).filter(|level| (1..=255).contains(level)) else {
        return vec![gm_feedback(world, "usage: @LEVEL <1-255>")];
    };
    {
        let mut session = world.resource_mut::<SessionResource>();
        if let Some(character) = session.selected_character.as_mut() {
            character.level = level;
        }
    }
    let (experience, max_experience) = {
        let runtime = world.resource::<PlayerRuntimeResource>();
        (runtime.experience, runtime.max_experience)
    };
    vec![
        ServerPacket::LevelChanged {
            level,
            experience,
            max_experience,
        },
        gm_feedback(world, format!("level set to {level}")),
    ]
}

fn gm_set_gold(world: &mut World, arg: Option<&str>) -> Vec<ServerPacket> {
    let Some(gold) = arg.and_then(|value| value.parse::<u32>().ok()) else {
        return vec![gm_feedback(world, "usage: @GOLD <amount>")];
    };
    world.resource_mut::<PlayerRuntimeResource>().gold = gold;
    // GainedGold is the self-player gold-counter packet (ObjectGold is for world
    // gold piles and is filtered by the visible-object post-processor).
    vec![
        ServerPacket::GainedGold { gold },
        gm_feedback(world, format!("gold set to {gold}")),
    ]
}

fn gm_heal(world: &mut World) -> Vec<ServerPacket> {
    let Some(player) = player_entity(world) else {
        return vec![gm_feedback(world, "no active player")];
    };
    let healed = {
        let mut entity = world.entity_mut(player);
        let Some(mut vitals) = entity.get_mut::<PlayerVitals>() else {
            return vec![gm_feedback(world, "no player vitals")];
        };
        vitals.hp = vitals.max_hp;
        vitals.mp = vitals.max_hp;
        *vitals
    };
    world.resource_mut::<PlayerRuntimeResource>().player_vitals = healed;
    let mut packets = Vec::new();
    if let Some(info) = super::packets::object_health_info_for_entity(world, player, 0) {
        packets.push(ServerPacket::ObjectHealth { info });
    }
    packets.push(gm_feedback(world, "healed to full"));
    packets
}

fn gm_move(world: &mut World, x: Option<&str>, y: Option<&str>) -> Vec<ServerPacket> {
    let (Some(x), Some(y)) = (
        x.and_then(|value| value.parse::<i32>().ok()),
        y.and_then(|value| value.parse::<i32>().ok()),
    ) else {
        return vec![gm_feedback(world, "usage: @MOVE <x> <y>")];
    };
    let destination = Point { x, y };
    if let Some(player) = player_entity(world) {
        world
            .entity_mut(player)
            .insert(Position(destination.clone()));
    }
    world
        .resource_mut::<PlayerRuntimeResource>()
        .player_position = destination.clone();
    vec![
        ServerPacket::UserLocation {
            location: current_location(world),
        },
        gm_feedback(world, format!("moved to ({x},{y})")),
    ]
}
