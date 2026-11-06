//! Death-time Crystal WinExp/GainExp selection from the current shared world.
use super::*;
use crate::runtime::zone::{ZoneExperienceProfile, ZoneExperienceSelection};
use crate::{ExperienceCharacter, ExperiencePresence, Stage5FriendIdentity};

const ORIGINAL_PARTY_EXPERIENCE_RATES: [f32; 11] =
    [1.0, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8, 1.9, 2.0, 2.1, 2.2];

fn transition_safe_experience_clocks(
    profile: &mut ZoneExperienceProfile,
    in_safe_zone: bool,
    now_ms: u64,
) -> Vec<(u8, bool)> {
    let Some(source) = &mut profile.rate_source else {
        return Vec::new();
    };
    let now_ms = now_ms.max(source.captured_at_ms);
    let mut changed = Vec::new();
    for row in &mut source.buffs {
        if !row.pause_in_safe_zone {
            continue;
        }
        let buff_type = match row.source_buff_key.as_deref() {
            Some("exp") => 102,
            Some("drop") => 103,
            _ => continue,
        };
        if in_safe_zone {
            if let Some(expiry) = row.expires_at_ms.take() {
                row.paused_remaining_ms = Some(expiry.saturating_sub(now_ms));
                changed.push((buff_type, true));
            }
        } else if let Some(remaining) = row.paused_remaining_ms.take() {
            // A malformed overflowing clock closes rather than creating an
            // immortal rate; normal source durations fit checked milliseconds.
            row.expires_at_ms = Some(now_ms.checked_add(remaining).unwrap_or(now_ms));
            changed.push((buff_type, false));
        }
    }
    source.captured_at_ms = now_ms;
    let _ = profile.refresh_captured_rate_projection();
    changed
}

fn preserve_accepted_experience_clocks(
    next: &mut ZoneExperienceProfile,
    previous: Option<&ZoneExperienceProfile>,
    in_safe_zone: bool,
) {
    let Some(source) = &mut next.rate_source else {
        return;
    };
    if let Some(previous) = previous.and_then(|profile| profile.rate_source.as_ref()) {
        source.captured_at_ms = source.captured_at_ms.max(previous.captured_at_ms);
        for row in &mut source.buffs {
            if !row.pause_in_safe_zone {
                continue;
            }
            if let Some(accepted) = previous.buffs.iter().find(|accepted| {
                accepted.pause_in_safe_zone
                    && accepted.source_buff_key == row.source_buff_key
                    && accepted.source_buff_instance == row.source_buff_instance
            }) {
                // The current Zone image may have accepted movement after the
                // owner read its clock. Only a fresh stacked instance replaces
                // it; the same potion cannot rewind that accepted transition.
                row.expires_at_ms = accepted.expires_at_ms;
                row.paused_remaining_ms = accepted.paused_remaining_ms;
            }
        }
    }
    let now_ms = source.captured_at_ms;
    transition_safe_experience_clocks(next, in_safe_zone, now_ms);
}

#[derive(Clone)]
struct ExperienceActor {
    session: SessionId,
    identity: Stage5FriendIdentity,
    key: ZoneKey,
    name: String,
    position: Point,
    level: u16,
    dead: bool,
    group_members: Vec<String>,
    profile: Option<ZoneExperienceProfile>,
    mentor_bank: Option<super::super::ZoneMentorBankAttribution>,
}

impl ZoneRuntime {
    /// Trusted current map rule, used by the Gateway's shared party admission.
    pub fn current_map_allows_group(&self) -> bool {
        !self
            .npc_teleport_config
            .map(&self.key.map_file_name)
            .is_some_and(|map| map.no_group)
            && !mir2_game_data::crystal_map_respawns_ref(&self.key.map_file_name)
                .is_some_and(|map| map.no_group)
    }

    /// Internal server projection, never constructed from a gameplay packet.
    /// A character/online registration cannot update another character's rates.
    pub fn update_experience_profile(
        &mut self,
        session: &SessionId,
        mut profile: Option<ZoneExperienceProfile>,
    ) -> bool {
        let Some(player) = self.players.get_mut(session) else {
            return false;
        };
        if profile.as_ref().is_some_and(|profile| {
            profile.account_id != player.account_id
                || profile.character_index != player.character_index
                || profile.account_id.is_empty()
                || profile.monster_multiplier == 0
                || profile.natural_kill_multiplier == 0
                || profile
                    .guild
                    .as_ref()
                    .is_some_and(|guild| guild.guild_id.is_empty())
                || !f32::from_bits(profile.world_experience_rate_bits).is_finite()
                || f32::from_bits(profile.world_experience_rate_bits) < 0.0
                || !f32::from_bits(profile.guild_experience_rate_bits).is_finite()
                || f32::from_bits(profile.guild_experience_rate_bits) < 0.0
                || profile.gain_rates_at(0).is_err()
        }) {
            return false;
        }
        if let Some(next) = &mut profile {
            preserve_accepted_experience_clocks(
                next,
                player.experience_profile.as_ref(),
                player.chat_profile.in_safe_zone,
            );
        }
        if let Some(level) = profile
            .as_ref()
            .map(|profile| profile.level)
            .filter(|level| *level > 0)
        {
            player.level = level;
        }
        player.experience_profile = profile;
        self.refresh_local_online_presence();
        true
    }

    /// Called only after accepted authoritative movement changed the safe-area
    /// profile. This clock transition is immediate, before a peer's private
    /// Tick/transform drain, and never enters that peer's personal runtime.
    pub(super) fn refresh_safe_experience_buff_clocks(
        &mut self,
        session: &SessionId,
        now_ms: u64,
    ) -> Vec<ServerPacket> {
        let Some(player) = self.players.get_mut(session) else {
            return Vec::new();
        };
        let Some(profile) = &mut player.experience_profile else {
            return Vec::new();
        };
        let packets =
            transition_safe_experience_clocks(profile, player.chat_profile.in_safe_zone, now_ms)
                .into_iter()
                .map(|(buff_type, paused)| ServerPacket::PauseBuff {
                    object_id: player.object_id,
                    buff_type,
                    paused,
                })
                .collect();
        self.refresh_local_online_presence();
        packets
    }

    pub fn player_experience_profile(&self, session: &SessionId) -> Option<ZoneExperienceProfile> {
        self.players.get(session)?.experience_profile.clone()
    }

    fn experience_actor(&self, session: &SessionId) -> Option<ExperienceActor> {
        let presence = self.online_presence.get(session)?;
        if let Some(player) = self.players.get(session) {
            if !presence.owner.matches_player(player) {
                return None;
            }
            return Some(ExperienceActor {
                session: session.clone(),
                identity: Stage5FriendIdentity {
                    account_id: player.account_id.clone(),
                    character_index: player.character_index,
                },
                key: self.key.clone(),
                name: player.name.clone(),
                position: player.position.clone(),
                level: player.level,
                dead: player.dead || player.hp <= 0,
                group_members: player.chat_profile.group_members.clone(),
                profile: player.experience_profile.clone(),
                mentor_bank: player.mentor_bank.clone(),
            });
        }
        Some(ExperienceActor {
            session: session.clone(),
            identity: Stage5FriendIdentity {
                account_id: presence.owner.account_id.clone(),
                character_index: presence.owner.character_index,
            },
            key: presence.key.clone(),
            name: presence.name.clone(),
            position: presence.position.clone(),
            level: presence.level,
            dead: presence.dead,
            group_members: presence.group_members.clone(),
            profile: presence.experience_profile.clone(),
            mentor_bank: presence.mentor_bank.clone(),
        })
    }

    /// Resolve an accepted server roster to exact current online identities.
    /// Offline references are absent, but duplicate/ambiguous live names reject
    /// allocation instead of assigning somebody else's share.
    fn experience_roster(&self, owner: &ExperienceActor) -> Result<Vec<ExperienceActor>, String> {
        let actors: Vec<_> = self
            .online_presence
            .keys()
            .filter_map(|sid| self.experience_actor(sid))
            .collect();
        let mut members = BTreeMap::new();
        for name in &owner.group_members {
            let matches: Vec<_> = actors
                .iter()
                .filter(|peer| peer.name.eq_ignore_ascii_case(name))
                .collect();
            if matches.len() > 1 {
                return Err("ambiguous current experience party member".into());
            }
            if let Some(member) = matches.first() {
                if members
                    .insert(member.session.clone(), (*member).clone())
                    .is_some()
                {
                    return Err("duplicate current experience party member".into());
                }
            }
        }
        if !members.contains_key(&owner.session) {
            return Err("experience party does not contain its owner".into());
        }
        Ok(members.into_values().collect())
    }

    fn experience_group_key(&self, actor: &ExperienceActor) -> Option<String> {
        if actor.group_members.is_empty() {
            return None;
        }
        let roster = self.experience_roster(actor).ok()?;
        let mut identities: Vec<_> = roster
            .into_iter()
            .map(|peer| (peer.identity.account_id, peer.identity.character_index))
            .collect();
        identities.sort();
        serde_json::to_string(&identities).ok()
    }

    fn experience_peer_eligible(
        &self,
        owner: &ExperienceActor,
        partner: Option<&super::super::ZoneExperiencePartner>,
        has_buff: bool,
        require_group: bool,
    ) -> bool {
        let Some(partner) = partner else {
            return false;
        };
        let peers: Vec<_> = self
            .online_presence
            .keys()
            .filter_map(|sid| self.experience_actor(sid))
            .filter(|peer| peer.identity == partner.identity)
            .collect();
        if peers.len() != 1 {
            return false;
        }
        let peer = &peers[0];
        if require_group {
            if owner.mentor_bank.as_ref().is_none_or(|bank| {
                bank.pupil != owner.identity
                    || bank.teacher != partner.identity
                    || bank.relationship_epoch != partner.relationship_epoch
            }) {
                return false;
            }
        } else if peer
            .profile
            .as_ref()
            .and_then(|profile| profile.lover_partner.as_ref())
            .is_none_or(|mate| mate.identity != owner.identity)
        {
            return false;
        }
        let owner_group = self.experience_group_key(owner);
        let peer_group = self.experience_group_key(peer);
        crate::crystal_social_experience_peer_eligible(
            ExperiencePresence {
                character: ExperienceCharacter {
                    account_id: &owner.identity.account_id,
                    character_index: owner.identity.character_index,
                },
                zone: &owner.key,
                position: [owner.position.x, owner.position.y],
                active: true,
                dead: owner.dead,
                group_id: owner_group.as_deref(),
            },
            Some(ExperienceCharacter {
                account_id: &partner.identity.account_id,
                character_index: partner.identity.character_index,
            }),
            Some(ExperiencePresence {
                character: ExperienceCharacter {
                    account_id: &peer.identity.account_id,
                    character_index: peer.identity.character_index,
                },
                zone: &peer.key,
                position: [peer.position.x, peer.position.y],
                active: true,
                dead: peer.dead,
                group_id: peer_group.as_deref(),
            }),
            has_buff,
            require_group,
        )
    }

    /// Unknown old profiles keep their absent selection; a delayed recipient
    /// must never infer a Guild from a later login. New profiles freeze all
    /// GainExp rates now, using the recipient's current map even when the kill
    /// happened in a map the retained EXPOwner has since left.
    pub(super) fn capture_monster_experience(
        &self,
        session: &SessionId,
        award: &ZoneMonsterKillAward,
    ) -> Result<Option<ZoneExperienceSelection>, String> {
        let Some(profile) = self.current_social_experience_profile(session)? else {
            return Ok(None);
        };
        let zone = serde_json::to_string(&self.key).map_err(|error| error.to_string())?;
        profile
            .select_after_win_experience(
                zone,
                award.monster_object_id,
                award.killed_at_ms,
                award.experience,
            )
            .map(Some)
    }

    /// Trusted eligibility for a personal GainExp command admitted against
    /// this current Zone view. The personal session still reads its live stats
    /// and actual buffs when it applies the amount.
    pub fn current_social_experience_profile(
        &self,
        session: &SessionId,
    ) -> Result<Option<ZoneExperienceProfile>, String> {
        let profile = self
            .experience_actor(session)
            .ok_or("experience recipient has no current online authority")?
            .profile;
        self.current_social_experience_profile_with(session, profile)
    }

    /// Evaluate a rebased server copy without changing a fenced owner's Zone
    /// state. The Gateway supplies it only while holding source authority and
    /// the Zone read together; identity and every current peer gate still apply.
    pub fn current_social_experience_profile_with(
        &self,
        session: &SessionId,
        profile: Option<ZoneExperienceProfile>,
    ) -> Result<Option<ZoneExperienceProfile>, String> {
        let Some(actor) = self.experience_actor(session) else {
            return Err("experience recipient has no current online authority".into());
        };
        let Some(mut profile) = profile else {
            return Ok(None);
        };
        if profile.account_id != actor.identity.account_id
            || profile.character_index != actor.identity.character_index
        {
            return Err("experience recipient profile identity changed".into());
        }
        profile.lover_eligible = self.experience_peer_eligible(
            &actor,
            profile.lover_partner.as_ref(),
            profile.lover_eligible,
            false,
        );
        profile.mentee_eligible = self.experience_peer_eligible(
            &actor,
            profile.mentee_partner.as_ref(),
            profile.mentee_eligible,
            true,
        );
        Ok(Some(profile))
    }

    pub(super) fn original_party_monster_kill_awards(
        &mut self,
        owner_session: &SessionId,
        mut award: ZoneMonsterKillAward,
    ) -> Vec<ZoneOutbound> {
        let Some(owner) = self.experience_actor(owner_session) else {
            return Vec::new();
        };
        if let Some(profile) = &owner.profile {
            if profile.account_id != owner.identity.account_id
                || profile.character_index != owner.identity.character_index
            {
                return Vec::new();
            }
            let Some(monster_level) = self
                .native_monsters
                .get(&award.monster_object_id)
                .map(|monster| monster.level)
            else {
                return Vec::new();
            };
            let Ok(experience) =
                profile.win_experience(award.experience, owner.level, monster_level)
            else {
                return Vec::new();
            };
            award.experience = experience;
        }
        // Crystal allows a spawned dead solo EXPOwner to GainExp. The grouped
        // branch excludes dead recipients, but still counts their levels.
        if owner.group_members.is_empty() {
            return self
                .monster_award_envelope(owner_session.clone(), award)
                .into_iter()
                .collect();
        }
        let Ok(roster) = self.experience_roster(&owner) else {
            return Vec::new();
        };
        let nearby: Vec<_> = roster
            .into_iter()
            .filter(|member| {
                member.position.x.abs_diff(owner.position.x) <= 16
                    && member.position.y.abs_diff(owner.position.y) <= 16
            })
            .collect();
        let sum_level: u32 = nearby.iter().map(|member| u32::from(member.level)).sum();
        if nearby.is_empty() || sum_level == 0 || nearby.iter().any(|member| member.level == 0) {
            return Vec::new();
        }
        let rate = ORIGINAL_PARTY_EXPERIENCE_RATES
            [nearby.len().min(ORIGINAL_PARTY_EXPERIENCE_RATES.len()) - 1];
        nearby
            .into_iter()
            .filter(|member| member.key == owner.key && !member.dead)
            .filter_map(|member| {
                let mut recipient = award.clone();
                // Preserve C# single precision evaluation and truncate each
                // recipient independently. There is no conserved remainder.
                recipient.experience = (award.experience as f32 * rate * f32::from(member.level)
                    / sum_level as f32) as u32;
                if member.session != *owner_session {
                    recipient.drops.clear();
                    recipient.boss_audit = None;
                }
                self.monster_award_envelope(member.session, recipient)
            })
            .collect()
    }
}
