//! Normal GuildWarReturn transport. Durability precedes every notification.
use super::super::shared_mentor::{MentorPresence, SharedMentorCoordinator};
use super::*;
use mir2_simulation::SimulationConfig;

pub(super) fn execute(
    packet: &ClientPacket,
    config: &SimulationConfig,
    key: &ZonePresenceKey,
    presences: &[MentorPresence],
    coordinator: &mut SharedMentorCoordinator,
) -> Option<Vec<ServerPacket>> {
    let ClientPacket::GuildWarReturn { name } = packet else {
        return None;
    };
    Some(match config.commit_shared_guild_war(&identity(key), name) {
        Err(reason) => super::management::error(&reason),
        Ok((own, enemy)) => {
            coordinator.changed();
            let actor_name = own
                .member(&identity(key))
                .expect("committed declaring member")
                .name
                .clone();
            let bank = ServerPacket::GuildStorageGoldChange {
                change_type: 2,
                amount: mir2_game_data::crystal_guild_settings().war_cost,
                name: actor_name,
            };
            super::management::queue_members(presences, &own, key, std::slice::from_ref(&bank));
            let enemy_packets = vec![ServerPacket::Chat {
                message: super::management::text("server.HasStartedWar", [&own.name]),
                chat_type: mir2_protocol::ChatType::System,
            }];
            super::management::queue_members(presences, &enemy, key, &enemy_packets);
            vec![
                ServerPacket::Chat {
                    message: super::management::text("server.YouStartedWarWith", [&enemy.name]),
                    chat_type: mir2_protocol::ChatType::System,
                },
                bank,
            ]
        }
    })
}
