use super::*;
use mir2_simulation::{validate_shared_mentor_request, SharedMentorMutation, Stage5FriendIdentity};

#[derive(Debug, Default)]
pub(super) struct SharedMentorCoordinator {
    pub(super) generation: Arc<AtomicU64>,
    invitations: BTreeMap<ZonePresenceKey, MentorInvitation>,
    pub(super) guild_invitations: BTreeMap<ZonePresenceKey, super::shared_guilds::GuildInvitation>,
    pub(super) guild_permissions: BTreeSet<ZonePresenceKey>,
    pub(super) marriages: BTreeMap<ZonePresenceKey, super::shared_marriage::MarriageInvitation>,
    pub(super) divorces: BTreeMap<ZonePresenceKey, super::shared_marriage::MarriageInvitation>,
}

#[derive(Debug, Clone)]
struct MentorInvitation {
    student: ZonePresenceKey,
    student_session: SessionId,
    teacher_session: SessionId,
    student_epoch: SocialPresenceEpoch,
    teacher_epoch: SocialPresenceEpoch,
}

pub(super) struct MentorPresence {
    pub(super) key: ZonePresenceKey,
    pub(super) session: SessionId,
    pub(super) entity: WorldEntitySnapshot,
    pub(super) object_id: u32,
    pub(super) map_file_name: String,
    pub(super) zone: Arc<Mutex<SharedInProcessZoneState>>,
}

#[derive(Debug, Clone)]
pub(super) struct SocialPresenceEpoch {
    zone: std::sync::Weak<Mutex<SharedInProcessZoneState>>,
    object_id: u32,
}
impl SocialPresenceEpoch {
    pub(super) fn new(presence: &MentorPresence) -> Self {
        Self {
            zone: Arc::downgrade(&presence.zone),
            object_id: presence.object_id,
        }
    }
    pub(super) fn matches(&self, presence: &MentorPresence) -> bool {
        self.object_id == presence.object_id && self.zone.ptr_eq(&Arc::downgrade(&presence.zone))
    }
}

pub(super) fn identity(key: &ZonePresenceKey) -> Stage5FriendIdentity {
    Stage5FriendIdentity {
        account_id: key.account_id.clone(),
        character_index: key.character_index,
    }
}

impl SharedMentorCoordinator {
    pub(super) fn changed(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
    }
    pub(super) fn forget(&mut self, key: &ZonePresenceKey) {
        self.marriages.retain(|recipient, _invite| recipient != key);
        self.divorces.retain(|recipient, _invite| recipient != key);
        self.invitations.retain(|teacher, _invite| teacher != key);
    }
}

impl SharedInProcessZoneSessionRuntime {
    pub(super) fn refresh_shared_social_buff_packets(&mut self) -> Vec<ServerPacket> {
        let Ok(presences) = self.mentor_presences() else { return Vec::new(); };
        let online = presences.into_iter().map(|presence| (presence.key.account_id, presence.key.character_index)).collect();
        // A status refresh failure must not discard an already committed command
        // response. No new buff is created from unavailable relationship data.
        self.inner.refresh_shared_social_buffs(&online).unwrap_or_default()
    }
    pub(super) fn refresh_shared_mentor_packets(&mut self, force: bool) -> Option<ServerPacket> {
        let online = self
            .mentor_presences()
            .ok()?
            .into_iter()
            .map(|p| ((p.key.account_id, p.key.character_index), p.entity.level))
            .collect();
        self.inner.refresh_shared_mentor_with_levels(&online, force)
    }

    pub(super) fn mentor_presences(&self) -> Result<Vec<MentorPresence>, String> {
        let zones: Vec<_> = {
            let replicas = self
                .ranking_replica_zones
                .lock()
                .map_err(|_| "social replica registry unavailable")?;
            self.ranking_zones
                .lock()
                .map_err(|_| "social Zone registry unavailable")?
                .iter()
                .filter(|(id, _)| !replicas.contains(*id))
                .map(|(_, r)| r.zone_state.clone())
                .collect()
        };
        let mut result = Vec::new();
        for zone in zones {
            let state = zone.lock().map_err(|_| "social presence unavailable")?;
            for (key, player) in &state.players {
                if state.teardown_fences.contains(key) {
                    continue;
                }
                if let Some(session) = state.zone_sessions.get(key) {
                    result.push(MentorPresence {
                        key: key.clone(),
                        session: session.clone(),
                        entity: player.entity.clone(),
                        object_id: player.zone_object_id,
                        map_file_name: player.map_file_name.clone(),
                        zone: zone.clone(),
                    });
                }
            }
        }
        Ok(result)
    }

    pub(super) fn execute_shared_mentor(&mut self, packet: &ClientPacket) -> Vec<ServerPacket> {
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
        let now = Self::zone_now_ms();
        let owner_id = identity(&key);
        let msg = |key: &str, args: &[String]| self.inner.shared_social_message(key, args);
        let error = |reason: String| {
            if reason.starts_with("server.") {
                msg(&reason, &["10".into()])
            } else {
                ServerPacket::Chat {
                    message: "The relationship could not be saved. Please try again.".into(),
                    chat_type: mir2_protocol::ChatType::System,
                }
            }
        };
        // Tickets refer to exact live registrations, never just a character name.
        coordinator.invitations.retain(|teacher, invite| {
            presences.iter().any(|p| {
                &p.key == teacher
                    && p.session == invite.teacher_session
                    && invite.teacher_epoch.matches(p)
            })
        });
        match packet {
            ClientPacket::AddMentor { name } => {
                let Some(teacher) = presences
                    .iter()
                    .find(|p| p.entity.name.eq_ignore_ascii_case(name.trim()))
                else {
                    return vec![msg("server.CannotFindPlayerByName", &[name.clone()])];
                };
                let Ok(mut pupil) = config.shared_mentor_profile_for(&owner_id) else {
                    return Vec::new();
                };
                let Ok(mut mentor) = config.shared_mentor_profile_for(&identity(&teacher.key))
                else {
                    return Vec::new();
                };
                pupil.level = owner.entity.level.unwrap_or(pupil.level);
                mentor.level = teacher.entity.level.unwrap_or(mentor.level);
                if let Err(reason) = validate_shared_mentor_request(&pupil, &mentor, now) {
                    return vec![error(reason)];
                }
                if coordinator.invitations.contains_key(&teacher.key) {
                    return Vec::new();
                }
                coordinator.invitations.insert(
                    teacher.key.clone(),
                    MentorInvitation {
                        student: key.clone(),
                        student_session: owner.session.clone(),
                        teacher_session: teacher.session.clone(),
                        student_epoch: SocialPresenceEpoch::new(owner),
                        teacher_epoch: SocialPresenceEpoch::new(teacher),
                    },
                );
                teacher
                    .zone
                    .lock()
                    .expect("validated social Zone")
                    .queue_zone_packets(
                        teacher.key.clone(),
                        vec![ServerPacket::MentorRequest {
                            name: pupil.name,
                            level: pupil.level,
                        }],
                    );
                vec![msg("server.RequestSent", &[])]
            }
            ClientPacket::MentorReply { accept_invite } => {
                let Some(invite) = coordinator.invitations.get(&key).cloned() else {
                    return Vec::new();
                };
                let Some(student) = presences.iter().find(|p| {
                    p.key == invite.student
                        && p.session == invite.student_session
                        && invite.student_epoch.matches(p)
                }) else {
                    coordinator.invitations.remove(&key);
                    return Vec::new();
                };
                if !accept_invite {
                    coordinator.invitations.remove(&key);
                    student
                        .zone
                        .lock()
                        .expect("validated social Zone")
                        .queue_zone_packets(
                            student.key.clone(),
                            vec![msg(
                                "server.PlayerRefusedMentor",
                                &[owner.entity.name.clone()],
                            )],
                        );
                    return Vec::new();
                }
                let Ok(mut pupil) = config.shared_mentor_profile_for(&identity(&student.key))
                else {
                    return Vec::new();
                };
                let Ok(mut mentor) = config.shared_mentor_profile_for(&owner_id) else {
                    return Vec::new();
                };
                pupil.level = student.entity.level.unwrap_or(pupil.level);
                mentor.level = owner.entity.level.unwrap_or(mentor.level);
                if let Err(reason) = validate_shared_mentor_request(&pupil, &mentor, now) {
                    coordinator.invitations.remove(&key);
                    return vec![error(reason)];
                }
                let result = config.commit_shared_mentor_mutation_with_live_levels(
                    &owner_id,
                    SharedMentorMutation::Accept {
                        student: identity(&student.key),
                    },
                    now,
                    Some((pupil.level, mentor.level)),
                );
                match result {
                    Ok(_) => {
                        coordinator.changed();
                        coordinator.forget(&key);
                        coordinator.forget(&student.key);
                        student
                            .zone
                            .lock()
                            .expect("validated social Zone")
                            .queue_zone_packets(
                                student.key.clone(),
                                vec![msg("server.YouAreMentoredBy", &[owner.entity.name.clone()])],
                            );
                        vec![msg("server.YouAreMentorOf", &[student.entity.name.clone()])]
                    }
                    Err(reason) => vec![error(reason)],
                }
            }
            ClientPacket::AllowMentor => match config.commit_shared_mentor_mutation(
                &owner_id,
                SharedMentorMutation::TogglePermission,
                now,
            ) {
                Ok(_) => {
                    coordinator.changed();
                    let allow = config
                        .shared_mentor_profile_for(&owner_id)
                        .is_ok_and(|p| p.mentor.allow_mentor);
                    vec![msg(
                        if allow {
                            "server.AllowingMentorRequests"
                        } else {
                            "server.BlockingMentorRequests"
                        },
                        &[],
                    )]
                }
                Err(reason) => vec![error(reason)],
            },
            ClientPacket::CancelMentor => match config.commit_shared_mentor_mutation(
                &owner_id,
                SharedMentorMutation::Cancel,
                now,
            ) {
                Ok(_) => {
                    coordinator.changed();
                    coordinator.forget(&key);
                    vec![msg("server.YouHaveMentorshipCooldown", &["7".into()])]
                }
                Err(reason) => vec![error(reason)],
            },
            _ => Vec::new(),
        }
    }
}
