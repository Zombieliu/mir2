//! Source packet projections for committed ordinary Guild management transactions.
use super::super::shared_mentor::{MentorPresence, SharedMentorCoordinator};
use super::*;
use mir2_simulation::{SharedGuildRecord, SimulationConfig, Stage5FriendIdentity};

pub(super) fn error(reason: &str) -> Vec<ServerPacket> {
    let (key, args) = reason
        .split_once(':')
        .map_or((reason, Vec::new()), |(key, value)| {
            (key, vec![value.to_string()])
        });
    let message = if key.starts_with("server.") {
        text(key, args)
    } else {
        reason.into()
    };
    vec![ServerPacket::Chat {
        message,
        chat_type: mir2_protocol::ChatType::System,
    }]
}

pub(super) fn text<I, S>(key: &str, args: I) -> String
where
    I: IntoIterator<Item = S>,
    S: ToString,
{
    // Crystal's ServerTextMap is server-global. The current exported/default
    // server language is English, independently of a client's input fields.
    mir2_game_data::format_localized_text(mir2_game_data::LanguageCode::English, key, args)
}

pub(super) fn queue_members(
    presences: &[MentorPresence],
    guild: &SharedGuildRecord,
    owner: &ZonePresenceKey,
    packets: &[ServerPacket],
) {
    for presence in presences {
        if &presence.key != owner && guild.member(&identity(&presence.key)).is_some() {
            presence
                .zone
                .lock()
                .expect("validated Guild presence")
                .queue_zone_packets(presence.key.clone(), packets.to_vec());
        }
    }
}

fn rank_projection(
    config: &SimulationConfig,
    guild: &SharedGuildRecord,
    rank_index: u8,
    presences: &[MentorPresence],
) -> mir2_protocol::GuildRank {
    let rank = guild
        .ranks
        .iter()
        .find(|rank| rank.index == rank_index)
        .expect("committed Guild rank");
    let store = config.account_store.lock().expect("committed Guild store");
    mir2_protocol::GuildRank {
        name: rank.name.clone(),
        options: rank.options,
        index: i32::from(rank.index),
        members: guild
            .members
            .iter()
            .filter(|member| member.rank_index == rank_index)
            .map(|member| mir2_protocol::GuildMember {
                name: member.name.clone(),
                id: member.identity.character_index,
                last_login_binary_datetime: store
                    .accounts
                    .get(&member.identity.account_id)
                    .and_then(|account| {
                        account
                            .character_last_access_binary_datetimes
                            .get(&member.identity.character_index)
                    })
                    .copied()
                    .unwrap_or(0),
                has_voted: false,
                online: presences
                    .iter()
                    .any(|presence| identity(&presence.key) == member.identity),
            })
            .collect(),
    }
}

fn empty_status() -> ServerPacket {
    ServerPacket::GuildStatus {
        guild_name: String::new(),
        guild_rank_name: String::new(),
        level: 0,
        experience: 0,
        max_experience: 0,
        gold: 0,
        spare_points: 0,
        member_count: 0,
        max_members: 0,
        voting: false,
        item_count: 0,
        buff_count: 0,
        my_options: 0,
        my_rank_id: -1,
    }
}

fn removal_packets(
    target: &Stage5FriendIdentity,
    actor: &Stage5FriendIdentity,
) -> Vec<ServerPacket> {
    vec![
        ServerPacket::Chat {
            message: text(
                if target == actor {
                    "server.YouHaveLeftGuild"
                } else {
                    "server.YouRemovedFromGuild"
                },
                std::iter::empty::<String>(),
            ),
            chat_type: mir2_protocol::ChatType::Guild,
        },
        empty_status(),
    ]
}

pub(super) fn execute(
    runtime: &mut SharedInProcessZoneSessionRuntime,
    packet: &ClientPacket,
    config: &SimulationConfig,
    key: &ZonePresenceKey,
    presences: &[MentorPresence],
    coordinator: &mut SharedMentorCoordinator,
) -> Option<Vec<ServerPacket>> {
    let self_leave = matches!(packet, ClientPacket::Chat { message, .. }
        if message.trim().eq_ignore_ascii_case("@LEAVEGUILD"));
    let actor = identity(key);
    let online = presences
        .iter()
        .map(|presence| {
            (
                presence.key.account_id.clone(),
                presence.key.character_index,
            )
        })
        .collect();
    match packet {
        ClientPacket::EditGuildMember {
            change_type: 1,
            name,
            ..
        }
        | ClientPacket::Chat { message: name, .. }
            if self_leave
                || matches!(packet, ClientPacket::EditGuildMember { change_type: 1, .. }) =>
        {
            let own_name = if self_leave {
                match config.shared_guild_for_identity(&actor) {
                    Ok(Some(guild)) => guild
                        .member(&actor)
                        .map(|member| member.name.clone())
                        .unwrap_or_default(),
                    Ok(None) => return Some(Vec::new()),
                    Err(reason) => return Some(error(&reason)),
                }
            } else {
                name.clone()
            };
            Some(
                match config.commit_shared_guild_remove_member(&actor, &own_name, self_leave) {
                    Err(reason) => error(&reason),
                    Ok((guild, target, target_name)) => {
                        coordinator.changed();
                        coordinator
                            .guild_invitations
                            .retain(|recipient, invitation| {
                                identity(recipient) != target
                                    && identity(&invitation.recruiter) != target
                            });
                        let event = ServerPacket::GuildMemberChange {
                            name: target_name,
                            rank_index: 0,
                            status: if target == actor { 4 } else { 3 },
                            ranks: Vec::new(),
                        };
                        if let Some(guild) = &guild {
                            queue_members(presences, guild, key, std::slice::from_ref(&event));
                        }
                        let mut packets = if target == actor {
                            removal_packets(&target, &actor)
                        } else {
                            if let Some(presence) = presences
                                .iter()
                                .find(|presence| identity(&presence.key) == target)
                            {
                                presence
                                    .zone
                                    .lock()
                                    .expect("validated former Guild member")
                                    .queue_zone_packets(
                                        presence.key.clone(),
                                        removal_packets(&target, &actor),
                                    );
                            }
                            vec![event]
                        };
                        if target == actor {
                            packets.extend(runtime.inner.shared_guild_packets(&online));
                            if guild.is_none() {
                                packets.extend(error("server.YouHaveDisbandedGuild"));
                            }
                        }
                        packets
                    }
                },
            )
        }
        ClientPacket::EditGuildMember {
            change_type: 2,
            name,
            rank_index,
            ..
        } => Some(
            match config.commit_shared_guild_member_rank(&actor, name, *rank_index, &online) {
                Err(reason) => error(&reason),
                Ok((guild, target)) => {
                    let actor_name = guild
                        .member(&actor)
                        .expect("committed Guild actor")
                        .name
                        .clone();
                    let target_name = guild
                        .member(&target)
                        .expect("committed Guild target")
                        .name
                        .clone();
                    let target_packet = ServerPacket::GuildMemberChange {
                        name: actor_name,
                        rank_index: 0,
                        status: 8,
                        ranks: vec![rank_projection(config, &guild, *rank_index, presences)],
                    };
                    let member_packet = ServerPacket::GuildMemberChange {
                        name: target_name,
                        rank_index: *rank_index,
                        status: 5,
                        ranks: Vec::new(),
                    };
                    coordinator.changed();
                    for presence in presences {
                        if &presence.key != key && guild.member(&identity(&presence.key)).is_some()
                        {
                            let packet = if identity(&presence.key) == target {
                                &target_packet
                            } else {
                                &member_packet
                            };
                            presence
                                .zone
                                .lock()
                                .expect("validated Guild member")
                                .queue_zone_packets(presence.key.clone(), vec![packet.clone()]);
                        }
                    }
                    if target == actor {
                        vec![target_packet]
                    } else {
                        vec![member_packet]
                    }
                }
            },
        ),
        ClientPacket::EditGuildMember { change_type: 4, .. } => {
            Some(match config.commit_shared_guild_new_rank(&actor) {
                Err(reason) => error(&reason),
                Ok(guild) => {
                    let new_index = if guild.ranks.len() > 2 {
                        guild.ranks.len() - 2
                    } else {
                        1
                    } as u8;
                    let packets = vec![ServerPacket::GuildMemberChange {
                        name: guild
                            .member(&actor)
                            .expect("committed Guild actor")
                            .name
                            .clone(),
                        rank_index: 0,
                        status: 6,
                        ranks: vec![rank_projection(config, &guild, new_index, presences)],
                    }];
                    coordinator.changed();
                    queue_members(presences, &guild, key, &packets);
                    packets
                }
            })
        }
        ClientPacket::EditGuildMember {
            change_type: 5,
            name,
            rank_index,
            rank_name,
        } => {
            let Ok(option) = rank_name.trim().parse::<i32>() else {
                return Some(error("Invalid Guild rank option."));
            };
            Some(
                match config.commit_shared_guild_rank_options(&actor, *rank_index, option, name) {
                    Err(reason) => error(&reason),
                    Ok(guild) => {
                        let packets = vec![ServerPacket::GuildMemberChange {
                            name: guild
                                .member(&actor)
                                .expect("committed Guild actor")
                                .name
                                .clone(),
                            rank_index: 0,
                            status: 7,
                            ranks: vec![rank_projection(config, &guild, *rank_index, presences)],
                        }];
                        coordinator.changed();
                        queue_members(presences, &guild, key, &packets);
                        packets
                    }
                },
            )
        }
        _ => None,
    }
}
