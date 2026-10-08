//! Crystal's transient shared party: exact online identities, no peer session locks.
use super::shared_mentor::{MentorPresence, SocialPresenceEpoch};
use super::*;
use mir2_protocol::{GroupMember, ObjectHealthInfo};

// Globals.MaxGroup and the pinned Setup.ini [Game]/GroupInviteDelay=2000.
const MAX_GROUP: usize = 15;
const GROUP_INVITE_DELAY_MS: u64 = 2_000;

/// These are transient online party notifications. An idle peer must receive
/// the same live FIFO delivery as movement/chat, without entering its private
/// command lane solely to discover an invitation or roster change.
pub(super) fn is_live_shared_group_packet(packet: &ServerPacket) -> bool {
    matches!(
        packet,
        ServerPacket::SwitchGroup { .. }
            | ServerPacket::GroupInvite { .. }
            | ServerPacket::AddMember { .. }
            | ServerPacket::DeleteMember { .. }
            | ServerPacket::DeleteGroup
            | ServerPacket::GroupMemberInfo { .. }
            | ServerPacket::GroupMembersMap { .. }
            | ServerPacket::SendMemberLocation { .. }
    )
}

fn clear_cold_group_notifications(state: &mut SharedInProcessZoneState, key: &ZonePresenceKey) {
    if let Some(packets) = state.pending_zone_packets.get_mut(key) {
        packets.retain(|packet| !is_live_shared_group_packet(packet));
    }
}

#[derive(Debug, Default)]
pub(super) struct SharedGroupCoordinator {
    next_registration: u64,
    players: BTreeMap<ZonePresenceKey, GroupPlayer>,
}

#[derive(Debug)]
struct GroupPlayer {
    session: SessionId,
    registration: u64,
    presence: SocialPresenceEpoch,
    name: String,
    allow_group: bool,
    members: Vec<ZonePresenceKey>,
    invitation: Option<GroupInvitation>,
    next_invite_ms: u64,
}

#[derive(Debug, Clone)]
struct GroupInvitation {
    inviter: ZonePresenceKey,
    inviter_session: SessionId,
    inviter_registration: u64,
    inviter_name: String,
    recipient_registration: u64,
}

struct GroupPresence {
    live: MentorPresence,
    map: ZoneKey,
    position: Point,
    hp: i32,
    max_hp: i32,
    no_group: bool,
    map_title: String,
}

#[derive(Default)]
struct GroupEffects {
    changed: BTreeSet<ZonePresenceKey>,
    packets: BTreeMap<ZonePresenceKey, Vec<ServerPacket>>,
}
impl GroupEffects {
    fn packet(&mut self, key: &ZonePresenceKey, packet: ServerPacket) {
        self.packets.entry(key.clone()).or_default().push(packet);
    }
}

enum GroupAction<'a> {
    Switch(bool),
    Add(&'a str),
    Delete(&'a str),
    Reply(bool),
}

impl SharedGroupCoordinator {
    /// The caller holds the social coordinator before entering the Zone and
    /// keeps that guard through chat publication. A private mirror read before
    /// a peer's accepted invitation cannot overwrite the current typed roster.
    pub(super) fn rebase_chat_profile(
        &self,
        key: &ZonePresenceKey,
        chat: &mut mir2_simulation::ZoneChatProfile,
    ) {
        chat.group_members = self.names(key);
    }

    fn register(&mut self, presence: &GroupPresence, allow_group: bool) -> Result<(), String> {
        self.next_registration = self
            .next_registration
            .checked_add(1)
            .ok_or("shared party registration exhausted")?;
        self.players.insert(
            presence.live.key.clone(),
            GroupPlayer {
                session: presence.live.session.clone(),
                registration: self.next_registration,
                presence: SocialPresenceEpoch::new(&presence.live),
                name: presence.live.entity.name.clone(),
                allow_group,
                members: Vec::new(),
                invitation: None,
                next_invite_ms: 0,
            },
        );
        Ok(())
    }

    fn leave(&mut self, key: &ZonePresenceKey, effects: &mut GroupEffects) {
        let Some(player) = self.players.get_mut(key) else {
            return;
        };
        let old = std::mem::take(&mut player.members);
        let name = player.name.clone();
        if old.is_empty() {
            return;
        }
        effects.changed.insert(key.clone());
        effects.packet(key, ServerPacket::DeleteGroup);
        let remaining = old
            .into_iter()
            .filter(|member| member != key)
            .collect::<Vec<_>>();
        let next = if remaining.len() > 1 {
            remaining.clone()
        } else {
            Vec::new()
        };
        for member in remaining {
            if let Some(player) = self.players.get_mut(&member) {
                player.members = next.clone();
                effects.changed.insert(member.clone());
                effects.packet(
                    &member,
                    if next.is_empty() {
                        ServerPacket::DeleteGroup
                    } else {
                        ServerPacket::DeleteMember { name: name.clone() }
                    },
                );
            }
        }
    }

    fn remove(&mut self, key: &ZonePresenceKey, effects: &mut GroupEffects) {
        self.leave(key, effects);
        self.players.remove(key);
        for player in self.players.values_mut() {
            if player
                .invitation
                .as_ref()
                .is_some_and(|invite| &invite.inviter == key)
            {
                player.invitation = None;
            }
        }
    }

    fn reconcile(&mut self, presences: &[GroupPresence], effects: &mut GroupEffects) {
        let absent = self
            .players
            .keys()
            .filter(|key| {
                !presences.iter().any(|presence| {
                    &presence.live.key == *key
                        && presence.live.session == self.players[*key].session
                })
            })
            .cloned()
            .collect::<Vec<_>>();
        for key in absent {
            self.remove(&key, effects);
        }
        for presence in presences {
            if let Some(player) = self.players.get_mut(&presence.live.key) {
                // A successful map handoff keeps the same live character's
                // party. Cold StartGame creates a fresh registration instead.
                if player.session == presence.live.session {
                    player.presence = SocialPresenceEpoch::new(&presence.live);
                    player.name = presence.live.entity.name.clone();
                }
            }
        }
    }

    fn disband(
        &mut self,
        key: &ZonePresenceKey,
        effects: &mut GroupEffects,
        message: &ServerPacket,
    ) {
        let members = self
            .players
            .get(key)
            .map(|player| player.members.clone())
            .unwrap_or_default();
        for member in members {
            if let Some(player) = self.players.get_mut(&member) {
                player.members.clear();
                effects.changed.insert(member.clone());
                effects.packet(&member, ServerPacket::DeleteGroup);
                effects.packet(&member, message.clone());
            }
        }
    }

    fn names(&self, key: &ZonePresenceKey) -> Vec<String> {
        self.players
            .get(key)
            .into_iter()
            .flat_map(|player| &player.members)
            .filter_map(|member| self.players.get(member).map(|player| player.name.clone()))
            .collect()
    }

    fn execute(
        &mut self,
        owner: &GroupPresence,
        action: GroupAction<'_>,
        presences: &[GroupPresence],
        now_ms: u64,
        message: &dyn Fn(&str, &[String]) -> ServerPacket,
        effects: &mut GroupEffects,
    ) {
        let key = &owner.live.key;
        let Some(actor) = self.players.get(key) else {
            return;
        };
        if actor.session != owner.live.session || !actor.presence.matches(&owner.live) {
            return;
        }
        match action {
            GroupAction::Switch(allow) => {
                let changed = actor.allow_group != allow;
                effects.packet(key, ServerPacket::SwitchGroup { allow_group: allow });
                if !changed {
                    return;
                }
                self.players.get_mut(key).unwrap().allow_group = allow;
                effects.changed.insert(key.clone());
                if !allow {
                    self.leave(key, effects);
                }
            }
            GroupAction::Add(name) => {
                if now_ms < actor.next_invite_ms {
                    return;
                }
                let members = actor.members.clone();
                self.players.get_mut(key).unwrap().next_invite_ms =
                    now_ms.saturating_add(GROUP_INVITE_DELAY_MS);
                let reason = if !members.is_empty() && members.first() != Some(key) {
                    Some("server.NotGroupLeader")
                } else if members.len() >= MAX_GROUP {
                    Some("server.YouGroupMaxMembers")
                } else if owner.no_group {
                    Some("server.YouCannotInviteOnSoloMaps")
                } else {
                    None
                };
                if let Some(reason) = reason {
                    effects.packet(key, message(reason, &[]));
                    return;
                }
                let targets = presences
                    .iter()
                    .filter(|presence| presence.live.entity.name.eq_ignore_ascii_case(name.trim()))
                    .collect::<Vec<_>>();
                let Some(target) = (targets.len() == 1).then(|| targets[0]) else {
                    effects.packet(key, message("server.CouldNotBeFound", &[name.into()]));
                    return;
                };
                let Some(peer) = self.players.get(&target.live.key) else {
                    effects.packet(key, message("server.CouldNotBeFound", &[name.into()]));
                    return;
                };
                let reason = if target.live.key == *key {
                    Some("server.CannotGroupSelf")
                } else if !peer.allow_group {
                    Some("server.NotAllowGroup")
                } else if !peer.members.is_empty() {
                    Some("server.AlreadyInAnotherGroup")
                } else if peer.invitation.is_some() {
                    Some("server.AlreadyReceivingInviteFromOtherPlayer")
                } else {
                    None
                };
                if let Some(reason) = reason {
                    effects.packet(key, message(reason, &[target.live.entity.name.clone()]));
                    return;
                }
                if target.no_group {
                    effects.packet(
                        key,
                        message(
                            "server.SoloMapTargetCannotAccept",
                            &[target.live.entity.name.clone()],
                        ),
                    );
                    effects.packet(&target.live.key, message("server.SoloMapCannotAccept", &[]));
                    return;
                }
                let inviter = self.players.get_mut(key).unwrap();
                inviter.allow_group = true;
                let invitation = GroupInvitation {
                    inviter: key.clone(),
                    inviter_session: inviter.session.clone(),
                    inviter_registration: inviter.registration,
                    inviter_name: inviter.name.clone(),
                    recipient_registration: self.players[&target.live.key].registration,
                };
                self.players.get_mut(&target.live.key).unwrap().invitation = Some(invitation);
                effects.changed.insert(key.clone());
                effects.packet(key, ServerPacket::SwitchGroup { allow_group: true });
                effects.packet(
                    &target.live.key,
                    ServerPacket::GroupInvite {
                        name: owner.live.entity.name.clone(),
                    },
                );
            }
            GroupAction::Delete(name) => {
                let members = actor.members.clone();
                if members.is_empty() {
                    effects.packet(key, message("server.NotInGroup", &[]));
                } else if members.first() != Some(key) {
                    effects.packet(key, message("server.NotGroupLeader", &[]));
                } else if let Some(member) = members.iter().find(|member| {
                    self.players
                        .get(*member)
                        .is_some_and(|player| player.name.eq_ignore_ascii_case(name.trim()))
                }) {
                    // DelMember explicitly enqueues this before LeaveGroup's
                    // own DeleteGroup in Crystal, including when kicking self.
                    effects.packet(member, ServerPacket::DeleteGroup);
                    self.leave(member, effects);
                } else {
                    effects.packet(key, message("server.NotInYourGroup", &[name.into()]));
                }
            }
            GroupAction::Reply(accept) => {
                let Some(invite) = self.players.get_mut(key).unwrap().invitation.take() else {
                    effects.packet(key, message("server.NotInvitedToGroup", &[]));
                    return;
                };
                let Some(inviter) = self.players.get(&invite.inviter).filter(|player| {
                    player.session == invite.inviter_session
                        && player.registration == invite.inviter_registration
                        && self.players[key].registration == invite.recipient_registration
                }) else {
                    effects.packet(
                        key,
                        message("server.PlayerNoLongerOnline", &[invite.inviter_name]),
                    );
                    return;
                };
                if !accept {
                    effects.packet(
                        &invite.inviter,
                        message(
                            "server.DeclinedGroupInvite",
                            &[owner.live.entity.name.clone()],
                        ),
                    );
                    return;
                }
                let members = inviter.members.clone();
                let reason = if !self.players[key].members.is_empty() {
                    Some("server.CannotJoinGroup")
                } else if !members.is_empty() && members.first() != Some(&invite.inviter) {
                    Some("server.NoLongerGroupLeader")
                } else if members.len() >= MAX_GROUP {
                    Some("server.GroupMaxMembers")
                } else if !inviter.allow_group {
                    Some("server.NotInAllowGroup")
                } else {
                    None
                };
                if let Some(reason) = reason {
                    effects.packet(key, message(reason, &[invite.inviter_name]));
                    return;
                }
                let Some(leader) = presences
                    .iter()
                    .find(|presence| presence.live.key == invite.inviter)
                else {
                    effects.packet(
                        key,
                        message("server.PlayerNoLongerOnline", &[invite.inviter_name]),
                    );
                    return;
                };
                if leader.no_group {
                    effects.packet(
                        key,
                        message(
                            "server.SoloMapTargetCannotAccept",
                            &[leader.live.entity.name.clone()],
                        ),
                    );
                    effects.packet(&leader.live.key, message("server.SoloMapCannotAccept", &[]));
                    return;
                }
                let mut members = if members.is_empty() {
                    vec![invite.inviter]
                } else {
                    members
                };
                for member in &members {
                    effects.packet(
                        member,
                        ServerPacket::AddMember {
                            name: owner.live.entity.name.clone(),
                        },
                    );
                    effects.packet(
                        key,
                        ServerPacket::AddMember {
                            name: self.players[member].name.clone(),
                        },
                    );
                }
                if members.len() == 1 {
                    let first = &members[0];
                    effects.packets.entry(first.clone()).or_default().insert(
                        0,
                        ServerPacket::AddMember {
                            name: self.players[first].name.clone(),
                        },
                    );
                }
                members.push(key.clone());
                effects.packet(
                    key,
                    ServerPacket::AddMember {
                        name: owner.live.entity.name.clone(),
                    },
                );
                for member in &members {
                    self.players.get_mut(member).unwrap().members = members.clone();
                    effects.changed.insert(member.clone());
                }
                append_current_roster_packets(&members, presences, effects);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_simulation::{SimulationConfig, SimulationSession};

    fn presences(count: usize) -> Vec<GroupPresence> {
        // Prepared server facts for reducer boundary rules. The normal TCP/WS
        // story is verified separately by shared_original_experience_transport.
        let config = SimulationConfig::default();
        let index = config.default_character.index;
        let mut session = SimulationSession::new(config);
        session.handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        });
        session.handle_packet(ClientPacket::StartGame {
            character_index: index,
        });
        let entity = session
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .expect("prepared active character has a self entity");
        let zone = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        (0..count)
            .map(|index| {
                let name = format!("Party{index}");
                let mut entity = entity.clone();
                entity.name = name.clone();
                entity.object_id = 6000 + index as u32;
                GroupPresence {
                    live: MentorPresence {
                        key: ZonePresenceKey {
                            account_id: format!("party-{index}"),
                            character_index: index as i32,
                        },
                        session: SessionId::new(format!("party-session-{index}")),
                        object_id: entity.object_id,
                        entity,
                        map_file_name: "party-test".into(),
                        zone: zone.clone(),
                    },
                    map: ZoneKey::for_map("party-test"),
                    position: Point {
                        x: 10 + index as i32,
                        y: 10,
                    },
                    hp: 10,
                    max_hp: 20,
                    no_group: false,
                    map_title: "Party test".into(),
                }
            })
            .collect()
    }

    fn message(key: &str, _: &[String]) -> ServerPacket {
        ServerPacket::Chat {
            message: key.into(),
            chat_type: mir2_protocol::ChatType::System,
        }
    }
    fn registered(presences: &[GroupPresence]) -> SharedGroupCoordinator {
        let mut groups = SharedGroupCoordinator::default();
        for presence in presences {
            groups.register(presence, true).unwrap();
        }
        groups
    }
    fn execute(
        groups: &mut SharedGroupCoordinator,
        presences: &[GroupPresence],
        index: usize,
        action: GroupAction<'_>,
        now: u64,
    ) -> GroupEffects {
        let mut effects = GroupEffects::default();
        groups.execute(
            &presences[index],
            action,
            presences,
            now,
            &message,
            &mut effects,
        );
        effects
    }
    fn join(groups: &mut SharedGroupCoordinator, presences: &[GroupPresence], index: usize) {
        let name = &presences[index].live.entity.name;
        let sent = execute(
            groups,
            presences,
            0,
            GroupAction::Add(name),
            index as u64 * GROUP_INVITE_DELAY_MS,
        );
        assert!(sent.packets[&presences[index].live.key]
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GroupInvite { .. })));
        execute(
            groups,
            presences,
            index,
            GroupAction::Reply(true),
            index as u64 * GROUP_INVITE_DELAY_MS,
        );
    }

    #[test]
    fn shared_party_join_health_uses_source_float_percentage() {
        let mut online = presences(2);
        online[0].hp = 53;
        online[0].max_hp = 100;
        online[1].hp = 1;
        online[1].max_hp = 1_000;
        let mut groups = registered(&online);
        execute(&mut groups, &online, 0, GroupAction::Add("Party1"), 2_000);
        let accepted = execute(&mut groups, &online, 1, GroupAction::Reply(true), 2_000);
        for (recipient, subject, expected) in [(0, 1, 0), (1, 0, 52)] {
            let info = accepted.packets[&online[recipient].live.key]
                .iter()
                .find_map(|p| match p {
                    ServerPacket::ObjectHealth { info }
                        if info.object_id == online[subject].live.object_id => Some(info),
                    _ => None,
                })
                .expect("nearby ordinary group acceptance exchanges health");
            assert_eq!(info.percent, expected);
            assert_eq!(info.expire, 5);
        }
    }

    #[test]
    fn shared_party_accepts_across_map_without_range_and_auto_enables_leader() {
        let mut online = presences(2);
        online[1].map = ZoneKey::for_map("different-map");
        online[1].position = Point { x: 900, y: 900 };
        let mut groups = registered(&online);
        groups
            .players
            .get_mut(&online[0].live.key)
            .unwrap()
            .allow_group = false;
        join(&mut groups, &online, 1);
        assert!(groups.players[&online[0].live.key].allow_group);
        assert_eq!(groups.names(&online[0].live.key), vec!["Party0", "Party1"]);
        assert_eq!(groups.names(&online[1].live.key), vec!["Party0", "Party1"]);
    }

    #[test]
    fn shared_party_rechecks_full_capacity_at_accept_and_obeys_invite_delay() {
        let online = presences(17);
        let mut groups = registered(&online);
        for index in 1..14 {
            join(&mut groups, &online, index);
        }
        // Two outstanding invitations fit when sent. One final slot remains.
        execute(&mut groups, &online, 0, GroupAction::Add("Party14"), 28_000);
        let throttled = execute(&mut groups, &online, 0, GroupAction::Add("Party15"), 28_001);
        assert!(throttled.packets.is_empty());
        assert!(groups.players[&online[15].live.key].invitation.is_none());
        execute(&mut groups, &online, 0, GroupAction::Add("Party15"), 30_000);
        execute(&mut groups, &online, 14, GroupAction::Reply(true), 30_000);
        let rejected = execute(&mut groups, &online, 15, GroupAction::Reply(true), 30_000);
        assert_eq!(groups.names(&online[0].live.key).len(), MAX_GROUP);
        assert!(groups.names(&online[15].live.key).is_empty());
        assert!(rejected.packets[&online[15].live.key].iter().any(|packet| matches!(packet,ServerPacket::Chat{message,..} if message == "server.GroupMaxMembers")));
    }

    #[test]
    fn shared_party_nonleader_cannot_kick_and_leader_leave_promotes_next_member() {
        let online = presences(3);
        let mut groups = registered(&online);
        join(&mut groups, &online, 1);
        join(&mut groups, &online, 2);
        execute(
            &mut groups,
            &online,
            1,
            GroupAction::Delete("Party2"),
            6_000,
        );
        assert_eq!(groups.names(&online[2].live.key).len(), 3);
        execute(&mut groups, &online, 0, GroupAction::Switch(false), 6_000);
        assert_eq!(groups.names(&online[1].live.key), vec!["Party1", "Party2"]);
        assert!(groups.names(&online[0].live.key).is_empty());
        execute(
            &mut groups,
            &online,
            1,
            GroupAction::Delete("Party2"),
            6_000,
        );
        assert!(groups.names(&online[1].live.key).is_empty());
        assert!(groups.names(&online[2].live.key).is_empty());
    }

    #[test]
    fn shared_party_invitation_cannot_rebind_to_cold_relogin_registration() {
        let online = presences(2);
        let mut groups = registered(&online);
        execute(&mut groups, &online, 0, GroupAction::Add("Party1"), 2_000);
        groups.register(&online[0], true).unwrap();
        execute(&mut groups, &online, 1, GroupAction::Reply(true), 2_000);
        assert!(groups.names(&online[0].live.key).is_empty());
        assert!(groups.names(&online[1].live.key).is_empty());
    }

    #[test]
    fn shared_party_cold_join_discards_old_invite_but_keeps_personal_receipts() {
        let online = presences(1);
        let key = &online[0].live.key;
        let mut state = SharedInProcessZoneState::new();
        state.pending_zone_packets.insert(
            key.clone(),
            vec![
                ServerPacket::GroupInvite {
                    name: "old inviter".into(),
                },
                ServerPacket::AddMember {
                    name: "old member".into(),
                },
                ServerPacket::GainExperience { amount: 17 },
                message("personal settlement", &[]),
            ],
        );
        clear_cold_group_notifications(&mut state, key);
        let retained = &state.pending_zone_packets[key];
        assert_eq!(retained.len(), 2);
        assert!(matches!(
            retained[0],
            ServerPacket::GainExperience { amount: 17 }
        ));
        assert!(
            matches!(&retained[1], ServerPacket::Chat { message, .. } if message == "personal settlement")
        );
    }

    #[test]
    fn shared_party_pending_invite_accept_keeps_changed_recipient_preference() {
        let online = presences(2);
        let mut groups = registered(&online);
        execute(&mut groups, &online, 0, GroupAction::Add("Party1"), 2_000);
        execute(&mut groups, &online, 1, GroupAction::Switch(false), 2_000);
        execute(&mut groups, &online, 1, GroupAction::Reply(true), 2_000);
        assert!(!groups.players[&online[1].live.key].allow_group);
        assert_eq!(groups.names(&online[1].live.key), vec!["Party0", "Party1"]);
        // Crystal acknowledges an unchanged preference before returning; this
        // cannot retroactively revoke an accepted, already pending invitation.
        let repeated = execute(&mut groups, &online, 1, GroupAction::Switch(false), 2_000);
        assert!(repeated.changed.is_empty());
        assert_eq!(repeated.packets[&online[1].live.key].len(), 1);
        assert_eq!(groups.names(&online[1].live.key), vec!["Party0", "Party1"]);
        execute(&mut groups, &online, 1, GroupAction::Switch(true), 2_000);
        execute(&mut groups, &online, 1, GroupAction::Switch(false), 2_000);
        assert!(groups.names(&online[0].live.key).is_empty());
        assert!(groups.names(&online[1].live.key).is_empty());
    }

    #[test]
    fn shared_party_no_group_invites_fail_and_map_entry_disbands_every_member() {
        let mut online = presences(2);
        let mut groups = registered(&online);
        online[1].no_group = true;
        execute(&mut groups, &online, 0, GroupAction::Add("Party1"), 2_000);
        assert!(groups.players[&online[1].live.key].invitation.is_none());
        online[1].no_group = false;
        execute(&mut groups, &online, 0, GroupAction::Add("Party1"), 4_000);
        execute(&mut groups, &online, 1, GroupAction::Reply(true), 4_000);
        let mut effects = GroupEffects::default();
        groups.disband(
            &online[1].live.key,
            &mut effects,
            &message("server.GroupingDisabledOnMap", &[]),
        );
        assert!(groups.names(&online[0].live.key).is_empty());
        assert!(groups.names(&online[1].live.key).is_empty());
        for presence in &online {
            assert!(effects.packets[&presence.live.key]
                .iter()
                .any(|packet| matches!(packet, ServerPacket::DeleteGroup)));
        }
    }
}

fn append_current_roster_packets(
    members: &[ZonePresenceKey],
    presences: &[GroupPresence],
    effects: &mut GroupEffects,
) {
    let current = members
        .iter()
        .filter_map(|key| presences.iter().find(|presence| &presence.live.key == key))
        .collect::<Vec<_>>();
    let enriched = current
        .iter()
        .map(|presence| GroupMember {
            name: presence.live.entity.name.clone(),
            level: presence.live.entity.level,
            class: presence.live.entity.class.map(|class| class as u8),
            hp: Some(presence.hp),
            max_hp: Some(presence.max_hp),
            online: Some(true),
        })
        .collect::<Vec<_>>();
    let leader_name = current
        .first()
        .map(|presence| presence.live.entity.name.clone())
        .unwrap_or_default();
    for recipient in &current {
        for member in &current {
            effects.packet(
                &recipient.live.key,
                ServerPacket::GroupMembersMap {
                    player_name: member.live.entity.name.clone(),
                    player_map: member.map_title.clone(),
                },
            );
            effects.packet(
                &recipient.live.key,
                ServerPacket::SendMemberLocation {
                    member_name: member.live.entity.name.clone(),
                    member_location: member.position.clone(),
                },
            );
            if recipient.live.key != member.live.key
                && recipient.map == member.map
                && recipient.position.x.abs_diff(member.position.x) <= 16
                && recipient.position.y.abs_diff(member.position.y) <= 16
            {
                effects.packet(
                    &recipient.live.key,
                    ServerPacket::ObjectHealth {
                        info: ObjectHealthInfo {
                            object_id: member.live.object_id,
                            percent: mir2_simulation::crystal_health::crystal_health_percent(
                                member.hp,
                                member.max_hp,
                            ),
                            expire: 5,
                        },
                    },
                );
            }
        }
        effects.packet(
            &recipient.live.key,
            ServerPacket::GroupMemberInfo {
                members: enriched.clone(),
                leader_name: leader_name.clone(),
            },
        );
    }
}

impl SharedInProcessZoneSessionRuntime {
    fn group_presences(&self) -> Result<Vec<GroupPresence>, String> {
        self.mentor_presences()?
            .into_iter()
            .filter_map(|live| {
                let projection = (|| {
                    let state = live
                        .zone
                        .lock()
                        .map_err(|_| "shared party Zone unavailable")?;
                    if state.zone_sessions.get(&live.key) != Some(&live.session)
                        || state.teardown_fences.contains(&live.key)
                        || state
                            .players
                            .get(&live.key)
                            .is_none_or(|player| player.zone_object_id != live.object_id)
                    {
                        return Ok(None);
                    }
                    let Some(map) = state.zone_manager.zone_key_for_session(&live.session) else {
                        return Ok(None);
                    };
                    let Some(zone) = state.zone_manager.zone(&map) else {
                        return Ok(None);
                    };
                    let no_group = !zone.current_map_allows_group();
                    let map_title = mir2_game_data::crystal_map_respawns_ref(&map.map_file_name)
                        .map(|map| map.map_title.clone())
                        .unwrap_or_else(|| map.map_file_name.clone());
                    let Some((position, _)) = state.zone_manager.player_transform(&live.session)
                    else {
                        return Ok(None);
                    };
                    let Some((hp, max_hp, _)) = state.zone_manager.player_vitals(&live.session)
                    else {
                        return Ok(None);
                    };
                    Ok::<_, String>(Some((map, position, hp, max_hp, no_group, map_title)))
                })();
                match projection {
                    Ok(Some((map, position, hp, max_hp, no_group, map_title))) => {
                        Some(Ok(GroupPresence {
                            live,
                            map,
                            position,
                            hp,
                            max_hp,
                            no_group,
                            map_title,
                        }))
                    }
                    Ok(None) => None,
                    Err(error) => Some(Err(error)),
                }
            })
            .collect()
    }

    fn deliver_shared_group_effects(
        &mut self,
        groups: &SharedGroupCoordinator,
        presences: &[GroupPresence],
        mut effects: GroupEffects,
    ) -> Result<Vec<ServerPacket>, String> {
        let own = self.current_presence_key();
        // A Zone owns all its maps. Publish every affected actor in that Zone
        // under one lock, so same-worker death observes one roster generation.
        let mut zones: Vec<Arc<Mutex<SharedInProcessZoneState>>> = Vec::new();
        for presence in presences {
            if (effects.changed.contains(&presence.live.key)
                || effects.packets.contains_key(&presence.live.key))
                && !zones
                    .iter()
                    .any(|zone| Arc::ptr_eq(zone, &presence.live.zone))
            {
                zones.push(presence.live.zone.clone());
            }
        }
        for zone in zones {
            let mut state = zone
                .lock()
                .map_err(|_| "shared party projection Zone unavailable")?;
            for presence in presences
                .iter()
                .filter(|presence| Arc::ptr_eq(&zone, &presence.live.zone))
            {
                let key = &presence.live.key;
                if state.teardown_fences.contains(key)
                    || state.zone_sessions.get(key) != Some(&presence.live.session)
                    || state
                        .players
                        .get(key)
                        .is_none_or(|player| player.zone_object_id != presence.live.object_id)
                {
                    continue;
                }
                if effects.changed.contains(key) {
                    if let Some(mut chat) = state
                        .zone_manager
                        .zone(&presence.map)
                        .and_then(|zone| zone.player_chat_profile(&presence.live.session))
                    {
                        chat.group_members = groups.names(key);
                        state.zone_manager.handle(ZoneCommand::UpdateChatProfile {
                            session_id: presence.live.session.clone(),
                            profile: chat,
                        });
                    }
                }
                if own.as_ref() != Some(key) {
                    if let Some(packets) = effects.packets.remove(key) {
                        state.queue_zone_packets(key.clone(), packets);
                    }
                }
            }
        }
        if let Some(own) = own {
            if let Some(player) = groups.players.get(&own) {
                self.inner
                    .apply_shared_group_projection(player.allow_group, &groups.names(&own))?;
            }
            Ok(effects.packets.remove(&own).unwrap_or_default())
        } else {
            Ok(Vec::new())
        }
    }

    pub(super) fn execute_shared_group_packet(
        &mut self,
        command: &WorldCommand,
    ) -> Option<Result<Vec<ServerPacket>, String>> {
        let action = match command {
            WorldCommand::ClientPacket(ClientPacket::SwitchGroup { allow_group }) => {
                GroupAction::Switch(*allow_group)
            }
            WorldCommand::ClientPacket(ClientPacket::AddMember { name }) => GroupAction::Add(name),
            WorldCommand::ClientPacket(ClientPacket::DelMember { name }) => {
                GroupAction::Delete(name)
            }
            WorldCommand::ClientPacket(ClientPacket::GroupInvite { accept_invite }) => {
                GroupAction::Reply(*accept_invite)
            }
            _ => return None,
        };
        Some((|| {
            let Some(key) = self.current_presence_key() else {
                return Ok(Vec::new());
            };
            let coordinator = self.shared_mentors.clone();
            let mut coordinator = coordinator
                .lock()
                .map_err(|_| "shared party coordinator unavailable")?;
            let presences = self.group_presences()?;
            let mut effects = GroupEffects::default();
            coordinator.groups.reconcile(&presences, &mut effects);
            let Some(owner) = presences.iter().find(|presence| presence.live.key == key) else {
                return Ok(Vec::new());
            };
            if !coordinator.groups.players.contains_key(&key) {
                coordinator
                    .groups
                    .register(owner, self.inner.shared_group_permission().unwrap_or(true))?;
            }
            coordinator.groups.execute(
                owner,
                action,
                &presences,
                Self::zone_now_ms(),
                &|key, args| self.inner.shared_social_message(key, args),
                &mut effects,
            );
            if !effects.changed.is_empty() {
                coordinator.changed();
            }
            self.deliver_shared_group_effects(&coordinator.groups, &presences, effects)
        })())
    }

    /// Call after accepted StartGame/Zone join. Map handoff preserves membership;
    /// cold login removes saved legacy roster names and issues a new live epoch.
    pub(super) fn sync_shared_group_presence(
        &mut self,
        cold_start: bool,
    ) -> Result<Vec<ServerPacket>, String> {
        let Some(key) = self.current_presence_key() else {
            return Ok(Vec::new());
        };
        let coordinator = self.shared_mentors.clone();
        let mut coordinator = coordinator
            .lock()
            .map_err(|_| "shared party coordinator unavailable")?;
        let presences = self.group_presences()?;
        let mut effects = GroupEffects::default();
        coordinator.groups.reconcile(&presences, &mut effects);
        let Some(owner) = presences.iter().find(|presence| presence.live.key == key) else {
            return Ok(Vec::new());
        };
        if cold_start || !coordinator.groups.players.contains_key(&key) {
            if cold_start {
                let mut state = owner
                    .live
                    .zone
                    .lock()
                    .map_err(|_| "cold party projection Zone unavailable")?;
                // Same-session map handoff keeps its live invitations. A cold
                // registration must not show a previous session's pending UI;
                // already enqueued old epochs are fenced by the live sender.
                clear_cold_group_notifications(&mut state, &key);
            }
            coordinator.groups.remove(&key, &mut effects);
            coordinator
                .groups
                .register(owner, self.inner.shared_group_permission().unwrap_or(true))?;
            effects.changed.insert(key.clone());
            effects.packet(&key, ServerPacket::DeleteGroup);
            effects.packet(
                &key,
                ServerPacket::SwitchGroup {
                    allow_group: coordinator.groups.players[&key].allow_group,
                },
            );
        }
        if owner.no_group {
            let message = self
                .inner
                .shared_social_message("server.GroupingDisabledOnMap", &[]);
            coordinator.groups.disband(&key, &mut effects, &message);
        }
        // A new Zone may have used a private snapshot taken before a concurrent
        // invitation was accepted. Repair this owner even when the typed party
        // itself has not changed; ordinary refreshes do not bump generation.
        if !effects.changed.is_empty() {
            coordinator.changed();
        }
        effects.changed.insert(key.clone());
        self.deliver_shared_group_effects(&coordinator.groups, &presences, effects)
    }

    /// Only the current owner's private session is changed. Peer packets/Zone
    /// rosters have already been published by the coordinator transaction.
    pub(super) fn consume_shared_group_projection(&mut self) -> Result<Vec<ServerPacket>, String> {
        let Some(key) = self.current_presence_key() else {
            return Ok(Vec::new());
        };
        if self.inner.shared_group_permission().is_none() {
            return Ok(Vec::new());
        }
        let coordinator = self.shared_mentors.clone();
        let coordinator = coordinator
            .lock()
            .map_err(|_| "shared party coordinator unavailable")?;
        let allow = coordinator
            .groups
            .players
            .get(&key)
            .map(|player| player.allow_group)
            .unwrap_or_else(|| self.inner.shared_group_permission().unwrap_or(true));
        self.inner
            .apply_shared_group_projection(allow, &coordinator.groups.names(&key))?;
        Ok(Vec::new())
    }

    /// Call before private logout clears identity and before remove_presence
    /// acquires the social coordinator. Fenced owners may leave; peers retain
    /// only current typed online memberships.
    pub(super) fn leave_shared_group_on_teardown(&mut self) -> Result<Vec<ServerPacket>, String> {
        let Some(key) = self.current_presence_key() else {
            return Ok(Vec::new());
        };
        let coordinator = self.shared_mentors.clone();
        let mut coordinator = coordinator
            .lock()
            .map_err(|_| "shared party coordinator unavailable")?;
        let presences = self.group_presences()?;
        let mut effects = GroupEffects::default();
        coordinator.groups.remove(&key, &mut effects);
        coordinator.changed();
        if let Some(allow) = self.inner.shared_group_permission() {
            self.inner.apply_shared_group_projection(allow, &[])?;
        }
        self.deliver_shared_group_effects(&coordinator.groups, &presences, effects)
    }
}
