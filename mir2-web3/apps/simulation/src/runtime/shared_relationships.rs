//! Durable bilateral social relationships. Transport owns invitation lifetimes.
use std::collections::{BTreeMap, BTreeSet};

use mir2_protocol::{MirClass, ServerPacket};

use crate::config::{AccountStore, Stage5FriendIdentity, Stage5MentorState, Stage5SystemsState};
use crate::{SimulationConfig, SimulationSession};

use super::resources::{is_in_world, RuntimeConfigResource, Stage5SystemsResource};

pub const SHARED_MENTOR_LEVEL_GAP: u16 = 10;
pub const SHARED_MENTOR_DURATION_MS: u64 = 7 * 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedMentorProfile {
    pub identity: Stage5FriendIdentity,
    pub name: String,
    pub class: MirClass,
    pub level: u16,
    pub mentor: Stage5MentorState,
}

#[derive(Debug, Clone)]
pub enum SharedMentorMutation {
    TogglePermission,
    Accept { student: Stage5FriendIdentity },
    Cancel,
}

pub(super) fn profile(
    store: &AccountStore,
    identity: &Stage5FriendIdentity,
) -> Result<SharedMentorProfile, String> {
    let account = store
        .accounts
        .get(&identity.account_id)
        .ok_or("social account no longer exists")?;
    let character = account
        .characters
        .iter()
        .find(|c| c.index == identity.character_index)
        .ok_or("social character no longer exists")?;
    let save = account
        .saves
        .get(&identity.character_index)
        .ok_or("social character save missing")?;
    let systems: Stage5SystemsState = match save.stage5_systems_json.as_deref() {
        Some(value) => {
            serde_json::from_str(value).map_err(|e| format!("invalid social state: {e}"))?
        }
        None => Stage5SystemsState::default(),
    };
    Ok(SharedMentorProfile {
        identity: identity.clone(),
        name: character.name.clone(),
        class: character.class,
        level: character.level,
        mentor: systems.mentor,
    })
}

pub(super) fn write_mentor(
    store: &mut AccountStore,
    identity: &Stage5FriendIdentity,
    mentor: Stage5MentorState,
) -> Result<(), String> {
    let save = store
        .accounts
        .get_mut(&identity.account_id)
        .and_then(|a| a.saves.get_mut(&identity.character_index))
        .ok_or("social character save missing")?;
    let mut systems: Stage5SystemsState = match save.stage5_systems_json.as_deref() {
        Some(value) => {
            serde_json::from_str(value).map_err(|e| format!("invalid social state: {e}"))?
        }
        None => Stage5SystemsState::default(),
    };
    systems.mentor = mentor;
    save.stage5_systems_json = Some(serde_json::to_string(&systems).map_err(|e| e.to_string())?);
    Ok(())
}

pub(super) fn advance(mentor: &mut Stage5MentorState) -> Result<(), String> {
    mentor.authority_revision = mentor
        .authority_revision
        .checked_add(1)
        .ok_or("mentor revision exhausted")?;
    mentor.pending_request_from = None;
    mentor.pending_request_level = 0;
    Ok(())
}

pub fn validate_shared_mentor_request(
    student: &SharedMentorProfile,
    teacher: &SharedMentorProfile,
    now_ms: u64,
) -> Result<(), String> {
    if student.identity == teacher.identity {
        return Err("server.YouCantMentorYourself".into());
    }
    if student.mentor.partner_identity.is_some() {
        return Err("server.YouAlreadyHaveMentor".into());
    }
    if teacher.mentor.partner_identity.is_some() {
        return Err("server.PlayerIsAlreadyMentor".into());
    }
    if student.mentor.cooldown_until_ms > now_ms {
        return Err("server.YouCantStartNewMentorship".into());
    }
    if teacher.mentor.cooldown_until_ms > now_ms {
        return Err("server.PlayerCantStartMentorshipYet".into());
    }
    if !teacher.mentor.allow_mentor {
        return Err("server.PlayerNotAllowMentorRequests".into());
    }
    if student.class != teacher.class {
        return Err("server.MentoredBySameClass".into());
    }
    if student.level.saturating_add(SHARED_MENTOR_LEVEL_GAP) > teacher.level {
        return Err("server.YouCanBeMentoredByHigherLevel".into());
    }
    Ok(())
}

impl SimulationConfig {
    pub fn shared_mentor_profile_for(
        &self,
        identity: &Stage5FriendIdentity,
    ) -> Result<SharedMentorProfile, String> {
        profile(
            &*self
                .account_store
                .lock()
                .map_err(|_| "social account store unavailable")?,
            identity,
        )
    }

    pub fn commit_shared_mentor_mutation(
        &self,
        actor: &Stage5FriendIdentity,
        mutation: SharedMentorMutation,
        now_ms: u64,
    ) -> Result<Vec<Stage5FriendIdentity>, String> {
        self.commit_shared_mentor_mutation_with_live_levels(actor, mutation, now_ms, None)
    }

    pub fn commit_shared_mentor_mutation_with_live_levels(
        &self,
        actor: &Stage5FriendIdentity,
        mutation: SharedMentorMutation,
        now_ms: u64,
        live_levels: Option<(u16, u16)>,
    ) -> Result<Vec<Stage5FriendIdentity>, String> {
        // Resolve cancellation's peer before entering the transaction, then
        // compare again inside; a concurrent relationship change cannot widen scope.
        let peer = match &mutation {
            SharedMentorMutation::Accept { student } => Some(student.clone()),
            SharedMentorMutation::Cancel => {
                self.shared_mentor_profile_for(actor)?
                    .mentor
                    .partner_identity
            }
            SharedMentorMutation::TogglePermission => None,
        };
        let mut accounts = vec![actor.account_id.clone()];
        if let Some(peer) = &peer {
            accounts.push(peer.account_id.clone());
        }
        accounts.sort();
        accounts.dedup();
        self.commit_account_store_transaction(&accounts, |store| {
            let mut owner = profile(store, actor)?;
            match mutation {
                SharedMentorMutation::TogglePermission => {
                    owner.mentor.allow_mentor = !owner.mentor.allow_mentor;
                    advance(&mut owner.mentor)?;
                    write_mentor(store, actor, owner.mentor)?;
                    Ok(vec![actor.clone()])
                }
                SharedMentorMutation::Accept { student } => {
                    let mut pupil = profile(store, &student)?;
                    if let Some((student_level, teacher_level)) = live_levels {
                        pupil.level = student_level;
                        owner.level = teacher_level;
                    }
                    validate_shared_mentor_request(&pupil, &owner, now_ms)?;
                    let epoch = owner
                        .mentor
                        .authority_revision
                        .max(pupil.mentor.authority_revision)
                        .checked_add(1)
                        .ok_or("mentor relationship epoch exhausted")?;
                    owner.mentor.ledger.relationship_epoch = epoch;
                    pupil.mentor.ledger.relationship_epoch = epoch;
                    owner.mentor.partner_identity = Some(student.clone());
                    owner.mentor.name = pupil.name.clone();
                    owner.mentor.level = pupil.level;
                    owner.mentor.is_mentor = true;
                    owner.mentor.established_at_ms = now_ms;
                    pupil.mentor.partner_identity = Some(actor.clone());
                    pupil.mentor.name = owner.name.clone();
                    pupil.mentor.level = owner.level;
                    pupil.mentor.is_mentor = false;
                    pupil.mentor.established_at_ms = now_ms;
                    advance(&mut owner.mentor)?;
                    advance(&mut pupil.mentor)?;
                    write_mentor(store, actor, owner.mentor)?;
                    write_mentor(store, &student, pupil.mentor)?;
                    Ok(vec![actor.clone(), student])
                }
                SharedMentorMutation::Cancel => {
                    let peer = peer.as_ref().ok_or("server.NoMentorship")?;
                    if owner.mentor.partner_identity.as_ref() != Some(peer) {
                        return Err("mentor relationship changed".into());
                    }
                    let mut partner = profile(store, peer)?;
                    if partner.mentor.partner_identity.as_ref() != Some(actor) {
                        return Err("mentor relationship is not reciprocal".into());
                    }
                    // Retain earned experience until the shared reward ledger
                    // can settle it; cancellation must never destroy it.
                    for state in [&mut owner.mentor, &mut partner.mentor] {
                        state.partner_identity = None;
                        state.name.clear();
                        state.level = 0;
                        state.online = false;
                        state.is_mentor = false;
                        state.established_at_ms = 0;
                        advance(state)?;
                    }
                    owner.mentor.cooldown_until_ms =
                        now_ms.saturating_add(SHARED_MENTOR_DURATION_MS);
                    write_mentor(store, actor, owner.mentor)?;
                    write_mentor(store, peer, partner.mentor)?;
                    Ok(vec![actor.clone(), peer.clone()])
                }
            }
        })
    }
}

impl SimulationSession {
    pub fn shared_social_message(&self, key: &str, args: &[String]) -> ServerPacket {
        super::packets::system_message_key_args(self.app.world(), key, args.iter().cloned())
    }

    pub fn shared_mentor_config(&self) -> Option<SimulationConfig> {
        (is_in_world(self.app.world()) && self.active_identity().is_some()).then(|| {
            self.app
                .world()
                .resource::<RuntimeConfigResource>()
                .config
                .clone()
        })
    }

    pub fn refresh_shared_mentor(
        &mut self,
        online: &BTreeSet<(String, i32)>,
        force: bool,
    ) -> Option<ServerPacket> {
        let online = online.iter().cloned().map(|id| (id, None)).collect();
        self.refresh_shared_mentor_with_levels(&online, force)
    }
    pub fn refresh_shared_mentor_with_levels(
        &mut self,
        online: &BTreeMap<(String, i32), Option<u16>>,
        force: bool,
    ) -> Option<ServerPacket> {
        let active = self.active_identity()?;
        let config = self.shared_mentor_config()?;
        let identity = Stage5FriendIdentity {
            account_id: active.account_id,
            character_index: active.character_index,
        };
        let durable = config.shared_mentor_profile_for(&identity).ok()?;
        let mut mentor = durable.mentor;
        if let Some(peer) = mentor
            .partner_identity
            .as_ref()
            .and_then(|id| config.shared_mentor_profile_for(id).ok())
        {
            mentor.name = peer.name;
            mentor.level = online
                .get(&(
                    peer.identity.account_id.clone(),
                    peer.identity.character_index,
                ))
                .copied()
                .flatten()
                .unwrap_or(peer.level);
            mentor.online =
                online.contains_key(&(peer.identity.account_id, peer.identity.character_index));
        } else {
            mentor.name.clear();
            mentor.level = 0;
            mentor.online = false;
        }
        mentor.pending_request_from = None;
        mentor.pending_request_level = 0;
        let state = &mut self
            .app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .mentor;
        let changed = *state != mentor;
        *state = mentor.clone();
        (changed || force).then_some(ServerPacket::MentorUpdate {
            name: mentor.name,
            level: mentor.level,
            online: mentor.online,
            mentee_exp: mentor.mentee_exp,
        })
    }
}

/// Called only after the final experience amount has been determined. The
/// caller's character checkpoint owns rollback of this local mutation.
pub(super) fn bank_mentor_gain(
    mentor: &mut Stage5MentorState,
    amount: u64,
    stable_receipt: Option<&str>,
) -> Result<bool, String> {
    if amount == 0 || mentor.partner_identity.is_none() || mentor.is_mentor {
        return Ok(false);
    }
    let mut ledger = mentor.ledger.clone();
    let key = match stable_receipt {
        Some(receipt) if !receipt.is_empty() => format!("receipt:{receipt}"),
        Some(_) => return Err("mentor experience receipt is empty".into()),
        None => {
            ledger.local_event_sequence = ledger
                .local_event_sequence
                .checked_add(1)
                .ok_or("mentor experience sequence exhausted")?;
            format!(
                "local:{}:{}",
                ledger.relationship_epoch, ledger.local_event_sequence
            )
        }
    };
    if ledger.bank_events.contains(&key) {
        return Ok(false);
    }
    ledger.bank_earned = ledger
        .bank_earned
        .checked_add(amount / 100)
        .ok_or("mentor experience bank exhausted")?;
    if ledger.bank_earned > i64::MAX as u64 {
        return Err("mentor experience bank exceeds protocol limit".into());
    }
    ledger.bank_events.insert(key);
    mentor.ledger = ledger;
    Ok(true)
}
