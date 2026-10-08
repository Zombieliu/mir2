//! Trusted personal rates and durable social authority projected into the Zone.
//! No peer private runtime/FIFO lane is acquired by these updates.
use super::shared_mentor::identity;
use super::*;

pub(super) type ExperienceAuthorityRebase<'a> = dyn Fn(
        &mir2_simulation::Stage5FriendIdentity,
        &mut Option<mir2_simulation::ZoneExperienceProfile>,
        &mut mir2_simulation::ZoneChatProfile,
    ) -> Result<(), String>
    + 'a;

/// Called only from the authority admission callback, after taking the Zone
/// mutex. The supplied rebase reads the already-locked AccountStore; it never
/// enters another character's private command lane or reacquires the store.
pub(super) fn rebase_locked_zone_experience_authority(
    state: &mut SharedInProcessZoneState,
    rebase: &ExperienceAuthorityRebase<'_>,
) -> Result<(), String> {
    let recipients = state
        .zone_sessions
        .iter()
        .filter(|(key, _)| !state.teardown_fences.contains(*key))
        .map(|(key, session)| (identity(key), session.clone()))
        .collect::<Vec<_>>();
    let mut projected = Vec::new();
    for (actor, session) in recipients {
        let Some(key) = state.zone_manager.zone_key_for_session(&session) else {
            continue;
        };
        let Some(mut chat) = state
            .zone_manager
            .zone(&key)
            .and_then(|zone| zone.player_chat_profile(&session))
        else {
            continue;
        };
        let mut profile = state.zone_manager.player_experience_profile(&session);
        rebase(&actor, &mut profile, &mut chat)?;
        projected.push((session, profile, chat));
    }
    // Validate every source before changing any Zone projection.
    for (session, profile, chat) in projected {
        if !state
            .zone_manager
            .update_experience_profile(&session, profile)
        {
            return Err("admission experience profile lost its registered character".into());
        }
        state.zone_manager.handle(ZoneCommand::UpdateChatProfile {
            session_id: session,
            profile: chat,
        });
    }
    Ok(())
}

impl SharedInProcessZoneSessionRuntime {
    /// Capture current shared peers before a private NPC/quest/periodic GainExp
    /// source. Personal rates/buff presence are rechecked at application.
    pub(super) fn refresh_personal_experience_context(&mut self) -> Result<(), String> {
        // An unavailable refresh must not leave the previous command's social
        // admission attached to a later personal GainExp source.
        self.inner
            .set_shared_personal_experience_context(None, None)?;
        self.refresh_shared_experience_profiles()?;
        let captured = if let (Some(key), Some(config)) = (
            self.current_presence_key(),
            self.inner.shared_mentor_config(),
        ) {
            config.with_shared_zone_experience_admission(|rebase| {
                let mut state = self
                    .zone_state
                    .lock()
                    .map_err(|_| "personal experience Zone unavailable")?;
                rebase_locked_zone_experience_authority(&mut state, rebase)?;
                let Some(session) = state.zone_sessions.get(&key).cloned() else {
                    return Ok(None);
                };
                let Some(zone) = state.zone_manager.zone_key_for_session(&session) else {
                    return Ok(None);
                };
                let Some(mut chat) = state
                    .zone_manager
                    .zone(&zone)
                    .and_then(|world| world.player_chat_profile(&session))
                else {
                    return Ok(None);
                };
                let mut profile = state.zone_manager.player_experience_profile(&session);
                // Teardown may consume an already committed payout while its
                // private lane is owned. Rebase this copy, not the fenced actor.
                rebase(&identity(&key), &mut profile, &mut chat)?;
                let Some(mut profile) = state
                    .zone_manager
                    .zone(&zone)
                    .ok_or("personal experience Zone disappeared")?
                    .current_social_experience_profile_with(&session, profile)?
                else {
                    return Ok(None);
                };
                let peer_is_leaving = |partner: &mir2_simulation::ZoneExperiencePartner| {
                    state.teardown_fences.iter().any(|fenced| {
                        fenced.account_id == partner.identity.account_id
                            && fenced.character_index == partner.identity.character_index
                    })
                };
                if profile.lover_partner.as_ref().is_some_and(peer_is_leaving) {
                    profile.lover_eligible = false;
                }
                if profile.mentee_partner.as_ref().is_some_and(peer_is_leaving) {
                    profile.mentee_eligible = false;
                }
                Ok(Some((profile, zone)))
            })?
        } else {
            None
        };
        let (profile, zone) = match captured {
            Some((profile, zone)) => (Some(profile), Some(zone)),
            None => (None, None),
        };
        self.inner
            .set_shared_personal_experience_context(profile, zone)
    }

    /// Run after the owner's accepted equipment/buff/level changes and before
    /// another normal kill can capture this character's reward inputs.
    pub(super) fn refresh_current_experience_profile(&mut self) -> Result<(), String> {
        let Some(key) = self.current_presence_key() else {
            return Ok(());
        };
        let accepted = {
            let state = self
                .zone_state
                .lock()
                .map_err(|_| "experience Zone clock unavailable")?;
            // A fenced owner may consume a committed payout during serialized
            // teardown. Read its accepted clock here; the publication below
            // still excludes fenced actors and no peer private lane is entered.
            state
                .zone_sessions
                .get(&key)
                .and_then(|session| state.zone_manager.player_experience_profile(session))
        };
        // Release the Zone before entering the owner's private world. A peer
        // never holds this lane, and publish merges any transition accepted
        // after this read by the exact source duration instance.
        self.inner
            .sync_shared_experience_buff_clocks(accepted.as_ref(), Self::zone_now_ms())?;
        let profile = self.inner.active_zone_experience_profile()?;
        let mut state = self
            .zone_state
            .lock()
            .map_err(|_| "experience Zone unavailable")?;
        if state.teardown_fences.contains(&key) {
            return Ok(());
        }
        let Some(session) = state.zone_sessions.get(&key).cloned() else {
            return Ok(());
        };
        if !state
            .zone_manager
            .update_experience_profile(&session, profile)
        {
            return Err("current experience profile does not match the online character".into());
        }
        Ok(())
    }

    /// Refresh relationship/membership fields on each character's existing
    /// trusted profile, keeping that character's personal rate and buff inputs.
    /// The compare/recheck prevents a concurrent owner refresh from being
    /// overwritten with the stale stats read before an authority lookup.
    pub(super) fn refresh_shared_experience_profiles(&mut self) -> Result<(), String> {
        let Some(config) = self.inner.shared_mentor_config() else {
            self.last_experience_projection = None;
            return Ok(());
        };
        // Personal level/stats/live buff inputs can change without a shared
        // transaction. Always publish the owner's current trusted rates.
        self.refresh_current_experience_profile()?;
        let signature = (
            self.shared_social_generation.load(Ordering::Acquire),
            config.shared_guild_clock_generation(),
        );
        if self.last_experience_projection == Some(signature) {
            return Ok(());
        }
        // A partial/error projection is retried on the next command.
        self.last_experience_projection = None;
        for presence in self.mentor_presences()? {
            let previous = {
                let state = presence
                    .zone
                    .lock()
                    .map_err(|_| "experience presence unavailable")?;
                if state.teardown_fences.contains(&presence.key)
                    || state.zone_sessions.get(&presence.key) != Some(&presence.session)
                    || state
                        .players
                        .get(&presence.key)
                        .is_none_or(|player| player.zone_object_id != presence.object_id)
                {
                    continue;
                }
                state
                    .zone_manager
                    .player_experience_profile(&presence.session)
            };
            let Some(mut next) = previous.clone() else {
                // A character must obtain its rates from its own live runtime.
                continue;
            };
            config.refresh_zone_experience_profile_authority(&mut next)?;
            let mut state = presence
                .zone
                .lock()
                .map_err(|_| "experience presence unavailable")?;
            if state.teardown_fences.contains(&presence.key)
                || state.zone_sessions.get(&presence.key) != Some(&presence.session)
                || state
                    .players
                    .get(&presence.key)
                    .is_none_or(|player| player.zone_object_id != presence.object_id)
                || state
                    .zone_manager
                    .player_experience_profile(&presence.session)
                    != previous
            {
                continue;
            }
            if !state
                .zone_manager
                .update_experience_profile(&presence.session, Some(next))
            {
                return Err("shared experience projection lost its online recipient".into());
            }
        }
        self.project_all_shared_guild_wars()?;
        let after = (
            self.shared_social_generation.load(Ordering::Acquire),
            config.shared_guild_clock_generation(),
        );
        if after == signature {
            self.last_experience_projection = Some(signature);
        }
        Ok(())
    }

    /// Membership and every ordinary war peer are read together. Update the
    /// current Zone chat/PK profile rather than a peer's personal save, so a war
    /// starts/ends for already-online guild members without another login.
    pub(super) fn project_all_shared_guild_wars(&self) -> Result<(), String> {
        let Some(config) = self.inner.shared_mentor_config() else {
            return Ok(());
        };
        let presences = self.mentor_presences()?;
        let actors = presences
            .iter()
            .map(|presence| identity(&presence.key))
            .collect::<Vec<_>>();
        let projections = presences
            .iter()
            .zip(config.shared_zone_guild_projections_for(&actors)?)
            .map(|(presence, projection)| (presence.key.clone(), projection))
            .collect::<BTreeMap<_, _>>();
        for presence in presences {
            let Some((name, wars, membership)) = projections.get(&presence.key) else {
                continue;
            };
            let mut state = presence
                .zone
                .lock()
                .map_err(|_| "guild war presence unavailable")?;
            if state.teardown_fences.contains(&presence.key)
                || state.zone_sessions.get(&presence.key) != Some(&presence.session)
                || state
                    .players
                    .get(&presence.key)
                    .is_none_or(|player| player.zone_object_id != presence.object_id)
            {
                continue;
            }
            if let Some(mut profile) = state
                .zone_manager
                .player_experience_profile(&presence.session)
            {
                if profile.guild != *membership {
                    profile.guild = membership.clone();
                    if !state
                        .zone_manager
                        .update_experience_profile(&presence.session, Some(profile))
                    {
                        return Err("guild membership projection lost its online recipient".into());
                    }
                }
            }
            let Some(key) = state.zone_manager.zone_key_for_session(&presence.session) else {
                continue;
            };
            let Some(mut chat) = state
                .zone_manager
                .zone(&key)
                .and_then(|zone| zone.player_chat_profile(&presence.session))
            else {
                continue;
            };
            if chat.guild_name == *name && chat.active_guild_wars == *wars {
                continue;
            }
            chat.guild_name = name.clone();
            chat.active_guild_wars = wars.clone();
            state.zone_manager.handle(ZoneCommand::UpdateChatProfile {
                session_id: presence.session.clone(),
                profile: chat,
            });
        }
        Ok(())
    }
}
