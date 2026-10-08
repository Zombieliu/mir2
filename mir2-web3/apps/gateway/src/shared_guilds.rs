use super::shared_mentor::{identity, SocialPresenceEpoch};
use super::*;
#[derive(Debug, Clone)]
pub(super) struct GuildInvitation {
    guild_id: String,
    recruiter: ZonePresenceKey,
    recruiter_session: SessionId,
    target_session: SessionId,
    recruiter_epoch: SocialPresenceEpoch,
    target_epoch: SocialPresenceEpoch,
}
pub(super) fn is_guild_command(command: &WorldCommand) -> bool {
    match command {
        WorldCommand::ClientPacket(ClientPacket::Chat { message, .. }) => {
            message.trim().eq_ignore_ascii_case("@ALLOWGUILD")
                || message.trim().eq_ignore_ascii_case("@LEAVEGUILD")
        }
        WorldCommand::ClientPacket(
            ClientPacket::GuildNameReturn { .. }
            | ClientPacket::GuildInvite { .. }
            | ClientPacket::EditGuildMember { .. }
            | ClientPacket::EditGuildNotice { .. }
            | ClientPacket::RequestGuildInfo { .. }
            | ClientPacket::GuildStorageGoldChange { .. }
            | ClientPacket::GuildStorageItemChange { .. }
            | ClientPacket::GuildBuffUpdate { .. }
            | ClientPacket::GuildWarReturn { .. }
            | ClientPacket::GuildTerritoryPage { .. }
            | ClientPacket::PurchaseGuildTerritory { .. },
        ) => true,
        _ => false,
    }
}
impl SharedInProcessZoneSessionRuntime {
    pub(super) fn refresh_shared_guild_packets(&mut self) -> Vec<ServerPacket> {
        let Ok(presences) = self.mentor_presences() else {
            return Vec::new();
        };
        let online = presences
            .into_iter()
            .map(|p| (p.key.account_id, p.key.character_index))
            .collect();
        self.inner.shared_guild_packets(&online)
    }
    pub(super) fn execute_shared_guild(&mut self, command: &WorldCommand) -> Vec<ServerPacket> {
        let WorldCommand::ClientPacket(packet) = command else {
            return Vec::new();
        };
        let Some(key) = self.current_presence_key() else {
            return Vec::new();
        };
        let Some(config) = self.inner.shared_mentor_config() else {
            return Vec::new();
        };
        let error = management::error;
        if !matches!(packet, ClientPacket::GuildNameReturn { .. })
            && config.refresh_shared_guild_authority().is_err()
        {
            return error("Guild authority could not be refreshed.");
        }
        let coordinator = self.shared_mentors.clone();
        let Ok(mut coordinator) = coordinator.lock() else {
            return error("Guild service unavailable.");
        };
        let Ok(presences) = self.mentor_presences() else {
            return error("Guild presence unavailable.");
        };
        let Some(owner) = presences.iter().find(|presence| presence.key == key) else {
            return Vec::new();
        };
        coordinator.guild_invitations.retain(|target, invite| {
            presences.iter().any(|p| {
                &p.key == target
                    && p.session == invite.target_session
                    && invite.target_epoch.matches(p)
            })
        });
        if let Some(packets) = management::execute(self, packet, &config, &key, &presences, &mut coordinator) {
            return packets;
        }
        if let Some(packets) = wars::execute(packet, &config, &key, &presences, &mut coordinator) {
            return packets;
        }
        match packet {
            ClientPacket::Chat { .. } => {
                let enabled = if coordinator.guild_permissions.remove(&key) {
                    false
                } else {
                    coordinator.guild_permissions.insert(key);
                    true
                };
                error(if enabled {
                    "Guild invitations enabled."
                } else {
                    "Guild invitations disabled."
                })
            }
            ClientPacket::GuildNameReturn { name } => {
                match self.inner.submit_shared_guild_name(name) {
                    Ok(packets)=>{coordinator.changed();packets},
                    Err(_)=>error("Guild creation failed. Check the name, level and required materials, then ask the NPC again."),
                }
            }
            ClientPacket::EditGuildMember {
                change_type: 0,
                name,
                ..
            } => {
                let Ok(Some(guild)) = config.shared_guild_for_identity(&identity(&key)) else {
                    return error("You are not in a guild.");
                };
                let Some(member) = guild.member(&identity(&key)) else {
                    return Vec::new();
                };
                if !guild
                    .ranks
                    .iter()
                    .any(|rank| rank.index == member.rank_index && rank.options & 2 != 0)
                {
                    return error("Your rank cannot recruit members.");
                }
                if !guild.active_wars.is_empty() {
                    return error("server.CannotRecruitDuringWar");
                }
                let targets = presences
                    .iter()
                    .filter(|p| p.entity.name.eq_ignore_ascii_case(name))
                    .collect::<Vec<_>>();
                if targets.len() != 1 {
                    return error("That character is not available.");
                }
                let target = targets[0];
                if target.key == key
                    || !coordinator.guild_permissions.contains(&target.key)
                    || coordinator.guild_invitations.contains_key(&target.key)
                {
                    return error("That character is not accepting guild invitations.");
                }
                if !matches!(
                    config.shared_guild_for_identity(&identity(&target.key)),
                    Ok(None)
                ) {
                    return error("That character already belongs to a guild.");
                }
                coordinator.guild_invitations.insert(
                    target.key.clone(),
                    GuildInvitation {
                        guild_id: guild.id,
                        recruiter: key.clone(),
                        recruiter_session: owner.session.clone(),
                        target_session: target.session.clone(),
                        recruiter_epoch: SocialPresenceEpoch::new(owner),
                        target_epoch: SocialPresenceEpoch::new(target),
                    },
                );
                target
                    .zone
                    .lock()
                    .expect("validated guild zone")
                    .queue_zone_packets(
                        target.key.clone(),
                        vec![ServerPacket::GuildInvite { name: guild.name }],
                    );
                Vec::new()
            }
            ClientPacket::GuildInvite { accept_invite } => {
                let Some(invite) = coordinator.guild_invitations.get(&key).cloned() else {
                    return Vec::new();
                };
                if !accept_invite {
                    coordinator.guild_invitations.remove(&key);
                    return Vec::new();
                }
                let Some(recruiter) = presences.iter().find(|p| {
                    p.key == invite.recruiter
                        && p.session == invite.recruiter_session
                        && invite.recruiter_epoch.matches(p)
                }) else {
                    coordinator.guild_invitations.remove(&key);
                    return error("That guild invitation has expired.");
                };
                match config.commit_shared_guild_join(
                    &identity(&recruiter.key),
                    &identity(&key),
                    &invite.guild_id,
                ) {
                    Ok(_) => {
                        coordinator.guild_invitations.remove(&key);
                        coordinator.guild_permissions.remove(&key);
                        coordinator.changed();
                        drop(coordinator);
                        self.refresh_shared_guild_packets()
                    }
                    Err(reason) => {
                        if reason != "server.GuildFull" {
                            coordinator.guild_invitations.remove(&key);
                        }
                        error(if reason == "server.GuildFull" {
                            "The guild is full."
                        } else if reason == "server.CannotRecruitDuringWar" {
                            "server.CannotRecruitDuringWar"
                        } else {
                            "The guild invitation could not be saved."
                        })
                    }
                }
            }
            ClientPacket::GuildStorageGoldChange {
                change_type,
                amount,
            } => match self.inner.change_shared_guild_gold(*change_type, *amount) {
                Ok(packets) => {
                    coordinator.changed();
                    if let Ok(Some(guild)) = config.shared_guild_for_identity(&identity(&key)) {
                        let broadcasts: Vec<_> = packets
                            .iter()
                            .filter(|packet| {
                                matches!(packet, ServerPacket::GuildStorageGoldChange { .. })
                            })
                            .cloned()
                            .collect();
                        for presence in &presences {
                            if presence.key != key
                                && guild.member(&identity(&presence.key)).is_some()
                            {
                                presence
                                    .zone
                                    .lock()
                                    .expect("validated guild zone")
                                    .queue_zone_packets(presence.key.clone(), broadcasts.clone());
                            }
                        }
                    }
                    packets
                }
                Err(_) => error("Guild gold transfer failed. Check safe zone, balance and rank."),
            },
            ClientPacket::GuildStorageItemChange {
                change_type,
                from,
                to,
            } => {
                match self
                    .inner
                    .change_shared_guild_item(*change_type, *from, *to)
                {
                    Ok(packets) => {
                        if *change_type != 3 {
                            coordinator.changed();
                            if let Ok(Some(guild)) =
                                config.shared_guild_for_identity(&identity(&key))
                            {
                                for presence in &presences {
                                    if presence.key != key
                                        && guild.member(&identity(&presence.key)).is_some()
                                    {
                                        presence
                                            .zone
                                            .lock()
                                            .expect("validated guild zone")
                                            .queue_zone_packets(
                                                presence.key.clone(),
                                                packets.clone(),
                                            );
                                    }
                                }
                            }
                        }
                        packets
                    }
                    Err(_) => vec![ServerPacket::GuildStorageItemChange {
                        change_type: 3u8.saturating_add(*change_type),
                        from: *from,
                        to: *to,
                        user: 0,
                        item: None,
                    }],
                }
            }
            ClientPacket::GuildBuffUpdate { action, id } => {
                match self.inner.change_shared_guild_buff(*action, *id) {
                    Ok(packets) => {
                        if *action != 0 {
                            coordinator.changed();
                            if let Ok(Some(guild)) =
                                config.shared_guild_for_identity(&identity(&key))
                            {
                                for presence in &presences {
                                    if presence.key != key
                                        && guild.member(&identity(&presence.key)).is_some()
                                    {
                                        presence
                                            .zone
                                            .lock()
                                            .expect("validated guild zone")
                                            .queue_zone_packets(
                                                presence.key.clone(),
                                                packets.clone(),
                                            );
                                    }
                                }
                            }
                        }
                        packets
                    }
                    Err(_) => {
                        error("Guild buff operation failed. Check level, points, gold and rank.")
                    }
                }
            }
            ClientPacket::EditGuildMember {
                change_type: 3,
                rank_index,
                rank_name,
                ..
            } => {
                match config.commit_shared_guild_rank_name(&identity(&key), *rank_index, rank_name) {
                    Ok(guild) => {
                        let actor = guild
                            .member(&identity(&key))
                            .expect("committed rename retains actor");
                        let rank = guild
                            .ranks
                            .iter()
                            .find(|rank| rank.index == *rank_index)
                            .expect("committed rename retains target rank");
                        let members = {
                            let store = config.account_store.lock().expect("committed guild store");
                            guild
                                .members
                                .iter()
                                .filter(|member| member.rank_index == *rank_index)
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
                                    online: presences.iter().any(|presence| {
                                        identity(&presence.key) == member.identity
                                    }),
                                })
                                .collect()
                        };
                        let packets = vec![ServerPacket::GuildMemberChange {
                            name: actor.name.clone(),
                            rank_index: 0,
                            status: 7,
                            ranks: vec![mir2_protocol::GuildRank {
                                name: rank.name.clone(),
                                options: rank.options,
                                index: i32::from(rank.index),
                                members,
                            }],
                        }];
                        coordinator.changed();
                        for presence in &presences {
                            if presence.key != key
                                && guild.member(&identity(&presence.key)).is_some()
                            {
                                presence
                                    .zone
                                    .lock()
                                    .expect("validated guild zone")
                                    .queue_zone_packets(presence.key.clone(), packets.clone());
                            }
                        }
                        packets
                    }
                    Err(reason) => error(&reason),
                }
            }
            ClientPacket::EditGuildNotice { notice } => {
                match config.commit_shared_guild_notice(&identity(&key), notice) {
                    Ok(guild) => {
                        // Preserve Crystal's terminal invalidation, then project the committed
                        // content for native clients that do not automatically request page 0.
                        let packets = vec![
                            ServerPacket::GuildNoticeChange {
                                update: -1,
                                notice: Vec::new(),
                            },
                            ServerPacket::GuildNoticeChange {
                                update: 0,
                                notice: guild.notice.clone(),
                            },
                        ];
                        coordinator.changed();
                        for presence in &presences {
                            if presence.key != key
                                && guild.member(&identity(&presence.key)).is_some()
                            {
                                presence
                                    .zone
                                    .lock()
                                    .expect("validated guild zone")
                                    .queue_zone_packets(presence.key.clone(), packets.clone());
                            }
                        }
                        packets
                    }
                    Err(reason) => error(&reason),
                }
            }
            ClientPacket::RequestGuildInfo { info_type } => {
                let mut packets = self.inner.shared_guild_packets(
                    &presences
                        .iter()
                        .map(|p| (p.key.account_id.clone(), p.key.character_index))
                        .collect(),
                );
                if *info_type == 0 {
                    if let Ok(Some(guild)) = config.shared_guild_for_identity(&identity(&key)) {
                        packets.push(ServerPacket::GuildNoticeChange {
                            update: guild.notice.len() as i32,
                            notice: guild.notice,
                        });
                    }
                }
                packets
            }
            // These paths are implemented by subsequent shared transactions. Never
            // fall back to the legacy personal guild bank or echo a false success.
            _ => error("This guild operation is not available yet."),
        }
    }
}

#[path = "shared_guild_management.rs"]
mod management;
#[path = "shared_guild_wars.rs"]
mod wars;
