use bevy_ecs::prelude::World;
use mir2_protocol::ServerPacket;

use super::components::{current_player_object_id, player_entity, CharacterBody, PlayerVitals};
use super::packets::object_health_info_for_entity;
use super::resources::{PlayerRuntimeResource, RuntimeConfigResource, SessionResource};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ExperienceGrant {
    pub(super) awarded: i64,
    pub(super) old_level: u16,
    pub(super) new_level: u16,
    pub(super) experience: i64,
    pub(super) max_experience: i64,
    pub(super) packets: Vec<ServerPacket>,
}

impl ExperienceGrant {
    fn unchanged(level: u16, experience: i64, max_experience: i64) -> Self {
        Self {
            awarded: 0,
            old_level: level,
            new_level: level,
            experience,
            max_experience,
            packets: Vec::new(),
        }
    }
}

/// Apply player experience through one server-authoritative path.
///
/// Experience is stored as progress inside the current level. Every crossed
/// threshold is consumed, overflow is carried, the selected character and ECS
/// body advance together, derived stats are recalculated, and Crystal-shaped
/// progression packets are emitted. No caller is allowed to clamp experience
/// directly to `max_experience`.
pub(super) fn grant_player_experience(world: &mut World, amount: i64) -> ExperienceGrant {
    let (old_level, current_experience, current_max_experience) = {
        let session = world.resource::<SessionResource>();
        let Some(character) = session.selected_character.as_ref() else {
            let runtime = world.resource::<PlayerRuntimeResource>();
            return ExperienceGrant::unchanged(0, runtime.experience, runtime.max_experience);
        };
        let runtime = world.resource::<PlayerRuntimeResource>();
        (
            character.level,
            runtime.experience.max(0),
            runtime.max_experience.max(1),
        )
    };
    if amount <= 0 || old_level == u16::MAX {
        return ExperienceGrant::unchanged(old_level, current_experience, current_max_experience);
    }

    let config = world.resource::<RuntimeConfigResource>().config.clone();
    let mut level = old_level;
    let mut experience = current_experience.saturating_add(amount);
    let mut threshold = config.experience_required_for_level(level);
    let mut level_events = Vec::new();

    while level < u16::MAX && experience >= threshold {
        experience = experience.saturating_sub(threshold);
        level = level.saturating_add(1);
        threshold = config.experience_required_for_level(level);
        level_events.push((level, experience, threshold));
    }

    {
        let mut session = world.resource_mut::<SessionResource>();
        if let Some(character) = session.selected_character.as_mut() {
            character.level = level;
        }
    }
    {
        let mut runtime = world.resource_mut::<PlayerRuntimeResource>();
        runtime.experience = experience;
        runtime.max_experience = threshold;
    }

    if level != old_level {
        if let Some(player) = player_entity(world) {
            if let Ok(mut entity) = world.get_entity_mut(player) {
                if let Some(mut body) = entity.get_mut::<CharacterBody>() {
                    body.level = level;
                }
            }
        }
        super::stats::refresh_player_stats(world);
        restore_player_pools_after_level_up(world);
    }

    let awarded = experience_awarded(old_level, current_experience, level, experience, &config);
    let mut packets = gain_experience_packets(awarded);
    let player_object_id = current_player_object_id(world);
    for (event_level, event_experience, event_max_experience) in level_events {
        packets.push(ServerPacket::LevelChanged {
            level: event_level,
            experience: event_experience,
            max_experience: event_max_experience,
        });
        if let Some(object_id) = player_object_id {
            packets.push(ServerPacket::ObjectLeveled { object_id });
        }
    }
    if level != old_level {
        if let Some(player) = player_entity(world) {
            if let Some(info) = object_health_info_for_entity(world, player, 0) {
                packets.push(ServerPacket::ObjectHealth { info });
            }
        }
    }

    ExperienceGrant {
        awarded,
        old_level,
        new_level: level,
        experience,
        max_experience: threshold,
        packets,
    }
}

fn restore_player_pools_after_level_up(world: &mut World) {
    let Some(player) = player_entity(world) else {
        return;
    };
    let restored = {
        let mut entity = world.entity_mut(player);
        entity.get_mut::<PlayerVitals>().map(|mut vitals| {
            vitals.hp = vitals.max_hp;
            vitals.mp = vitals.max_mp;
            *vitals
        })
    };
    if let Some(restored) = restored {
        world.resource_mut::<PlayerRuntimeResource>().player_vitals = restored;
    }
}

fn experience_awarded(
    old_level: u16,
    old_experience: i64,
    new_level: u16,
    new_experience: i64,
    config: &crate::config::SimulationConfig,
) -> i64 {
    let consumed = (old_level..new_level).fold(0_i64, |total, level| {
        total.saturating_add(config.experience_required_for_level(level))
    });
    consumed
        .saturating_add(new_experience)
        .saturating_sub(old_experience)
        .max(0)
}

fn gain_experience_packets(mut amount: i64) -> Vec<ServerPacket> {
    let mut packets = Vec::new();
    while amount > 0 {
        let chunk = amount.min(i64::from(u32::MAX)) as u32;
        packets.push(ServerPacket::GainExperience { amount: chunk });
        amount -= i64::from(chunk);
    }
    packets
}

#[cfg(test)]
mod tests {
    use super::gain_experience_packets;
    use mir2_protocol::ServerPacket;

    #[test]
    fn gain_packets_preserve_awards_larger_than_the_protocol_field() {
        let packets = gain_experience_packets(i64::from(u32::MAX) + 7);
        assert_eq!(
            packets,
            vec![
                ServerPacket::GainExperience { amount: u32::MAX },
                ServerPacket::GainExperience { amount: 7 },
            ]
        );
    }
}
