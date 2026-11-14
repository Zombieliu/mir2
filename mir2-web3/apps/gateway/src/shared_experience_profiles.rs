//! Trusted personal rates and durable social authority projected into the Zone.
//! No peer private runtime/FIFO lane is acquired by these updates.
use super::shared_mentor::identity;
use super::*;

#[cfg(test)]
std::thread_local! {
    static OWNER_PROFILE_PUBLICATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(super) fn owner_profile_publications_for_test() -> usize {
    OWNER_PROFILE_PUBLICATIONS.with(std::cell::Cell::get)
}

#[cfg(test)]
#[derive(Clone, Copy)]
pub(super) enum ProjectionTestPhase { Entry, OwnerPublished, ScanStarted, ScanFinished }
#[cfg(test)]
pub(super) type ProjectionTestHook = Arc<dyn Fn(ProjectionTestPhase) + Send + Sync>;
#[cfg(test)]
std::thread_local! {
    static PROJECTION_TEST_HOOK: std::cell::RefCell<Option<ProjectionTestHook>> = const { std::cell::RefCell::new(None) };
}
#[cfg(test)]
pub(super) fn set_projection_test_hook(hook: Option<ProjectionTestHook>) {
    PROJECTION_TEST_HOOK.with(|slot| { *slot.borrow_mut() = hook; });
}
#[cfg(test)]
fn projection_test_phase(phase: ProjectionTestPhase) {
    let hook = PROJECTION_TEST_HOOK.with(|slot| slot.borrow().clone());
    if let Some(hook) = hook { hook(phase); }
}

#[derive(Default)]
pub(super) struct SharedExperienceProjectionFlight {
    sequence: AtomicU64,
    completed: Mutex<Option<ExperienceProjectionCompletion>>,
}
impl fmt::Debug for SharedExperienceProjectionFlight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.debug_struct("SharedExperienceProjectionFlight").finish_non_exhaustive() }
}
struct ExperienceProjectionCompletion {
    sequence: u64,
    signature: (u64, u64),
    authority: GatewayConfig,
    zones: std::sync::Weak<Mutex<BTreeMap<ZoneId, SharedInProcessZoneResources>>>,
    replicas: std::sync::Weak<Mutex<BTreeSet<ZoneId>>>,
}

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
        let (updated, _) = state
            .zone_manager
            .update_experience_and_chat_profile(&session, profile, chat);
        if !updated {
            return Err("admission experience profile lost its registered character".into());
        }
    }
    Ok(())
}

/// Personal GainExp reads only its owner's two explicit social partners. Resolve
/// those identities from this locked authority image, rather than the previous
/// Zone projection. Shared death/cadence admission still rebases every actor.
pub(super) fn capture_locked_personal_experience_context(
    state: &mut SharedInProcessZoneState,
    key: &ZonePresenceKey,
    rebase: &ExperienceAuthorityRebase<'_>,
) -> Result<
    Option<(
        mir2_simulation::ZoneExperienceProfile,
        mir2_simulation::ZoneKey,
    )>,
    String,
> {
    let Some(session) = state.zone_sessions.get(key).cloned() else {
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
    // Teardown can consume a committed payout. Its owner is rebased as a copy,
    // and neither this owner nor any fenced partner is published below.
    rebase(&identity(key), &mut profile, &mut chat)?;
    let partners = profile
        .iter()
        .flat_map(|profile| [&profile.lover_partner, &profile.mentee_partner])
        .flatten()
        .map(|partner| ZonePresenceKey {
            account_id: partner.identity.account_id.clone(),
            character_index: partner.identity.character_index,
        })
        .filter(|partner| partner != key)
        .collect::<BTreeSet<_>>();
    let mut projected = vec![(key.clone(), session.clone(), profile.clone(), chat)];
    for partner in partners {
        if state.teardown_fences.contains(&partner) {
            continue;
        }
        let Some(peer_session) = state.zone_sessions.get(&partner).cloned() else {
            continue;
        };
        if state
            .zone_manager
            .zone_key_for_session(&peer_session)
            .as_ref()
            != Some(&zone)
        {
            continue;
        }
        let mut peer_chat = state
            .zone_manager
            .zone(&zone)
            .and_then(|world| world.player_chat_profile(&peer_session))
            .ok_or("personal experience partner lost its online projection")?;
        let mut peer_profile = state.zone_manager.player_experience_profile(&peer_session);
        rebase(&identity(&partner), &mut peer_profile, &mut peer_chat)?;
        projected.push((partner, peer_session, peer_profile, peer_chat));
    }
    // A required partner's unavailable/malformed authority must not leave an
    // already published owner or earlier partner behind. All source rebases and
    // recipient checks finish before the first update on this same Zone writer.
    let world = state
        .zone_manager
        .zone(&zone)
        .ok_or("personal experience Zone disappeared")?;
    for (recipient, recipient_session, next, _) in &projected {
        if world.player_identity(recipient_session)
            != Some((
                recipient_session.clone(),
                recipient.account_id.clone(),
                recipient.character_index,
            ))
            || next.as_ref().is_some_and(|next| {
                next.validate_for_online_character(&recipient.account_id, recipient.character_index)
                    .is_err()
            })
        {
            return Err("personal experience projection differs from its online recipient".into());
        }
        world.current_social_experience_profile_with(recipient_session, next.clone())?;
    }
    for (recipient, recipient_session, next, chat) in projected {
        if state.teardown_fences.contains(&recipient) {
            continue;
        }
        if !state
            .zone_manager
            .update_experience_and_chat_profile(&recipient_session, next, chat)
            .0
        {
            return Err("personal experience projection lost its registered recipient".into());
        }
    }
    if !state.teardown_fences.contains(key) {
        profile = state.zone_manager.player_experience_profile(&session);
    }
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
            let _stage = shared_session_substage("shared_session.experience.admission_capture");
            config.with_shared_zone_experience_admission(|rebase| {
                let mut state = self
                    .zone_state
                    .lock()
                    .map_err(|_| "personal experience Zone unavailable")?;
                capture_locked_personal_experience_context(&mut state, &key, rebase)
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
        let previous_level = state
            .zone_manager
            .player_experience_profile(&session)
            .map(|p| p.level);
        let next_level = profile.as_ref().map(|p| p.level);
        if !state
            .zone_manager
            .update_experience_profile(&session, profile)
        {
            return Err("current experience profile does not match the online character".into());
        }
        #[cfg(test)]
        OWNER_PROFILE_PUBLICATIONS.with(|count| count.set(count.get() + 1));
        if previous_level != next_level {
            // Peers must refresh MentorUpdate on their next normal command even
            // when the viewport metadata and daily social timer have not moved.
            self.shared_social_generation.fetch_add(1, Ordering::AcqRel);
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
        let flight = self.experience_projection_flight.clone();
        let entered_sequence = flight.sequence.load(Ordering::Acquire);
        #[cfg(test)]
        projection_test_phase(ProjectionTestPhase::Entry);
        // Personal level/stats/live buff inputs can change without a shared
        // transaction. Always publish the owner's current trusted rates.
        {
            let _stage = shared_session_substage("shared_session.experience.owner_publication");
            self.refresh_current_experience_profile()?;
        }
        #[cfg(test)]
        projection_test_phase(ProjectionTestPhase::OwnerPublished);
        let signature = (
            self.shared_social_generation.load(Ordering::Acquire),
            config.shared_guild_clock_generation(),
        );
        if self.last_experience_projection == Some(signature) {
            return Ok(());
        }
        // A partial/error projection is retried on the next command.
        self.last_experience_projection = None;
        let mut completed = flight.completed.lock().map_err(|_| "experience projection flight poisoned")?;
        let signature = (self.shared_social_generation.load(Ordering::Acquire), config.shared_guild_clock_generation());
        if completed.as_ref().is_some_and(|last| last.sequence > entered_sequence
            && last.signature == signature && config.same_shared_experience_projection_authority(&last.authority)
            && last.zones.ptr_eq(&Arc::downgrade(&self.ranking_zones))
            && last.replicas.ptr_eq(&Arc::downgrade(&self.ranking_replica_zones))) {
            self.last_experience_projection = Some(signature);
            return Ok(());
        }
        // Never leave an older success reusable after this miss fails or changes generation.
        *completed = None;
        let peer_stage = shared_session_substage("shared_session.experience.peer_projection_and_wars");
        #[cfg(test)]
        projection_test_phase(ProjectionTestPhase::ScanStarted);
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
        drop(peer_stage);
        #[cfg(test)]
        projection_test_phase(ProjectionTestPhase::ScanFinished);
        let after = (
            self.shared_social_generation.load(Ordering::Acquire),
            config.shared_guild_clock_generation(),
        );
        if after == signature {
            self.last_experience_projection = Some(signature);
            if let Some(sequence) = flight.sequence.load(Ordering::Relaxed).checked_add(1) {
                *completed = Some(ExperienceProjectionCompletion { sequence, signature, authority: config,
                    zones: Arc::downgrade(&self.ranking_zones), replicas: Arc::downgrade(&self.ranking_replica_zones) });
                flight.sequence.store(sequence, Ordering::Release);
            }
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
