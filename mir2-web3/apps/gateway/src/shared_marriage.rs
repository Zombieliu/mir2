use super::shared_mentor::{identity, MentorPresence, SocialPresenceEpoch};
use super::*;
use mir2_simulation::{
    validate_shared_marriage_request, SharedMarriageMutation, Stage5FriendIdentity,
};

#[derive(Debug, Clone)]
pub(super) struct MarriageInvitation {
    pub(super) proposer: ZonePresenceKey,
    proposer_session: SessionId,
    recipient_session: SessionId,
    proposer_epoch: SocialPresenceEpoch,
    recipient_epoch: SocialPresenceEpoch,
}
fn facing_pair(a: &MentorPresence, b: &MentorPresence) -> bool {
    if !Arc::ptr_eq(&a.zone, &b.zone)
        || a.map_file_name != b.map_file_name
        || a.key == b.key
        || a.entity.dead
        || b.entity.dead
        || a.entity.hp == Some(0)
        || b.entity.hp == Some(0)
    {
        return false;
    }
    let here = Point {
        x: a.entity.x,
        y: a.entity.y,
    };
    let there = Point {
        x: b.entity.x,
        y: b.entity.y,
    };
    point_in_direction(&here, a.entity.direction) == there
        && point_in_direction(&there, b.entity.direction) == here
}

impl SharedInProcessZoneSessionRuntime {
    pub(super) fn next_marriage_day_refresh_ms(&self) -> u64 {
        let Some(active) = self.inner.active_identity() else {
            return u64::MAX;
        };
        let Some(config) = self.inner.shared_mentor_config() else {
            return u64::MAX;
        };
        let id = Stage5FriendIdentity {
            account_id: active.account_id.clone(),
            character_index: active.character_index,
        };
        let Ok(profile) = config.shared_marriage_profile_for(&id) else {
            return u64::MAX;
        };
        let relation = profile.relationship;
        if relation.partner_identity.is_none() {
            return u64::MAX;
        }
        let day = 86_400_000;
        let elapsed_days = Self::zone_now_ms().saturating_sub(relation.married_at_ms) / day;
        relation
            .married_at_ms
            .saturating_add(elapsed_days.saturating_add(1).saturating_mul(day))
    }

    pub(super) fn refresh_shared_marriage_packets(&mut self, force: bool) -> Vec<ServerPacket> {
        let Ok(presences) = self.mentor_presences() else {
            return Vec::new();
        };
        let online = presences
            .into_iter()
            .map(|p| {
                let title = mir2_game_data::crystal_map_respawns_ref(&p.map_file_name)
                    .map(|map| map.map_title.clone())
                    .unwrap_or(p.map_file_name);
                ((p.key.account_id, p.key.character_index), title)
            })
            .collect();
        self.inner
            .refresh_shared_marriage(&online, Self::zone_now_ms(), force)
    }

    pub(super) fn execute_shared_marriage(&mut self, packet: &ClientPacket) -> Vec<ServerPacket> {
        let Some(key) = self.current_presence_key() else {
            return Vec::new();
        };
        let Some(config) = self.inner.shared_mentor_config() else {
            return Vec::new();
        };
        let coordinator = self.shared_mentors.clone();
        let Ok(mut coordinator) = coordinator.lock() else {
            return Vec::new();
        };
        let Ok(presences) = self.mentor_presences() else {
            return Vec::new();
        };
        let Some(owner) = presences.iter().find(|p| p.key == key) else {
            return Vec::new();
        };
        let id = identity(&key);
        let now = Self::zone_now_ms();
        let msg = |key: &str, args: &[String]| self.inner.shared_social_message(key, args);
        let error = |reason: String| {
            if reason.starts_with("server.") {
                msg(&reason, &["7".into()])
            } else {
                ServerPacket::Chat {
                    message: "The relationship could not be saved. Please try again.".into(),
                    chat_type: mir2_protocol::ChatType::System,
                }
            }
        };
        let retain = |invitations: &mut BTreeMap<ZonePresenceKey, MarriageInvitation>| {
            invitations.retain(|recipient, invite| {
                presences.iter().any(|p| {
                    &p.key == recipient
                        && p.session == invite.recipient_session
                        && invite.recipient_epoch.matches(p)
                })
            });
        };
        retain(&mut coordinator.marriages);
        retain(&mut coordinator.divorces);
        match packet {
            ClientPacket::ChangeMarriage => match config.commit_shared_marriage_mutation(
                &id,
                SharedMarriageMutation::TogglePermission,
                now,
            ) {
                Ok(()) => {
                    coordinator.changed();
                    let Ok(profile) = config.shared_marriage_profile_for(&id) else {
                        return Vec::new();
                    };
                    let state = profile.relationship;
                    let key = if state.partner_identity.is_some() {
                        if state.allow_lover_recall {
                            "server.YouAllowRecallFromLover"
                        } else {
                            "server.YouBlockRecallFromLover"
                        }
                    } else if state.allow_marriage {
                        "server.YouAllowMarriageRequests"
                    } else {
                        "server.YouBlockMarriageRequests"
                    };
                    vec![msg(key, &[])]
                }
                Err(reason) => vec![error(reason)],
            },
            ClientPacket::MarriageRequest | ClientPacket::DivorceRequest => {
                let divorce = matches!(packet, ClientPacket::DivorceRequest);
                let Some(peer) = presences.iter().find(|p| facing_pair(owner, p)) else {
                    return vec![msg(
                        if divorce {
                            "server.FaceLoverToDivorce"
                        } else {
                            "server.FacePlayerForMarriageRequest"
                        },
                        &[],
                    )];
                };
                let Ok(mut a) = config.shared_marriage_profile_for(&id) else {
                    return Vec::new();
                };
                let Ok(mut b) = config.shared_marriage_profile_for(&identity(&peer.key)) else {
                    return Vec::new();
                };
                a.level = owner.entity.level.unwrap_or(a.level);
                b.level = peer.entity.level.unwrap_or(b.level);
                if divorce {
                    if a.relationship.partner_identity.as_ref() != Some(&b.identity)
                        || b.relationship.partner_identity.as_ref() != Some(&id)
                    {
                        return vec![msg("server.YouNotMarriedTo", &[peer.entity.name.clone()])];
                    }
                } else if let Err(reason) = validate_shared_marriage_request(&a, &b, now) {
                    return vec![error(reason)];
                }
                let invitations = if divorce {
                    &mut coordinator.divorces
                } else {
                    &mut coordinator.marriages
                };
                if invitations.contains_key(&peer.key) {
                    return Vec::new();
                }
                invitations.insert(
                    peer.key.clone(),
                    MarriageInvitation {
                        proposer: key.clone(),
                        proposer_session: owner.session.clone(),
                        recipient_session: peer.session.clone(),
                        proposer_epoch: SocialPresenceEpoch::new(owner),
                        recipient_epoch: SocialPresenceEpoch::new(peer),
                    },
                );
                peer.zone
                    .lock()
                    .expect("validated marriage Zone")
                    .queue_zone_packets(
                        peer.key.clone(),
                        vec![if divorce {
                            ServerPacket::DivorceRequest {
                                name: owner.entity.name.clone(),
                            }
                        } else {
                            ServerPacket::MarriageRequest {
                                name: owner.entity.name.clone(),
                            }
                        }],
                    );
                Vec::new()
            }
            ClientPacket::MarriageReply { accept_invite }
            | ClientPacket::DivorceReply { accept_invite } => {
                let divorce = matches!(packet, ClientPacket::DivorceReply { .. });
                let invitations = if divorce {
                    &mut coordinator.divorces
                } else {
                    &mut coordinator.marriages
                };
                let Some(invite) = invitations.get(&key).cloned() else {
                    return Vec::new();
                };
                let Some(peer) = presences.iter().find(|p| {
                    p.key == invite.proposer
                        && p.session == invite.proposer_session
                        && invite.proposer_epoch.matches(p)
                }) else {
                    invitations.remove(&key);
                    return Vec::new();
                };
                if !accept_invite {
                    invitations.remove(&key);
                    peer.zone
                        .lock()
                        .expect("validated marriage Zone")
                        .queue_zone_packets(
                            peer.key.clone(),
                            vec![msg(
                                if divorce {
                                    "server.HasRefusedDivorceYou"
                                } else {
                                    "server.HasRefusedToMarryYou"
                                },
                                &[owner.entity.name.clone()],
                            )],
                        );
                    return Vec::new();
                }
                if !facing_pair(owner, peer) {
                    invitations.remove(&key);
                    return vec![msg(
                        if divorce {
                            "server.FaceLoverToDivorce"
                        } else {
                            "server.NeedFaceEachOtherForMarriage"
                        },
                        &[],
                    )];
                }
                let Ok(mut proposer) = config.shared_marriage_profile_for(&identity(&peer.key))
                else {
                    return Vec::new();
                };
                let Ok(mut recipient) = config.shared_marriage_profile_for(&id) else {
                    return Vec::new();
                };
                proposer.level = peer.entity.level.unwrap_or(proposer.level);
                recipient.level = owner.entity.level.unwrap_or(recipient.level);
                if !divorce {
                    if let Err(reason) =
                        validate_shared_marriage_request(&proposer, &recipient, now)
                    {
                        invitations.remove(&key);
                        return vec![error(reason)];
                    }
                }
                let mutation = if divorce {
                    SharedMarriageMutation::Divorce {
                        partner: identity(&peer.key),
                    }
                } else {
                    SharedMarriageMutation::Accept {
                        partner: identity(&peer.key),
                    }
                };
                match config.commit_shared_marriage_mutation_with_live_levels(
                    &id,
                    mutation,
                    now,
                    Some((proposer.level, recipient.level)),
                ) {
                    Ok(()) => {
                        invitations.remove(&key);
                        coordinator.changed();
                        peer.zone
                            .lock()
                            .expect("validated marriage Zone")
                            .queue_zone_packets(
                                peer.key.clone(),
                                vec![msg(
                                    if divorce {
                                        "server.YouAreDivorced"
                                    } else {
                                        "server.CongratulationsMarriedTo"
                                    },
                                    &[owner.entity.name.clone()],
                                )],
                            );
                        vec![msg(
                            if divorce {
                                "server.YouAreDivorced"
                            } else {
                                "server.CongratulationsMarriedTo"
                            },
                            &[peer.entity.name.clone()],
                        )]
                    }
                    Err(reason) => vec![error(reason)],
                }
            }
            _ => Vec::new(),
        }
    }
}
