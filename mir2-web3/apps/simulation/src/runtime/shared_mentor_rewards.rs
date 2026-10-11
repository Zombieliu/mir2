//! Pair accounting queues durable credit; only the recipient changes their XP.
use super::resources::{PlayerRuntimeResource, RuntimeConfigResource, Stage5SystemsResource};
use super::shared_relationships::{
    advance, profile, write_mentor, SHARED_MENTOR_DURATION_MS, SHARED_MENTOR_LEVEL_GAP,
};
use crate::{SimulationConfig, SimulationSession, Stage5FriendIdentity, Stage5MentorState};
use bevy_ecs::prelude::{Resource, World};
use mir2_protocol::ServerPacket;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharedMentorBreakReason { Manual, LoginExpired, Graduated }

#[derive(Debug, Clone)]
pub struct SharedMentorAccountingReceipt {
    pub participants: Vec<Stage5FriendIdentity>,
    pub teacher: Stage5FriendIdentity,
    pub transferred: u64,
    pub ended: bool,
}

impl SimulationConfig {
    /// No peer XP, vitals or full-save revision is changed here.
    pub fn settle_shared_mentor_bank(
        &self, actor: &Stage5FriendIdentity, expected_epoch: u64,
        end: Option<SharedMentorBreakReason>, now_ms: u64, teacher_online: bool,
    ) -> Result<SharedMentorAccountingReceipt, String> {
        self.settle_shared_mentor_bank_with_live_levels(
            actor, expected_epoch, end, now_ms, teacher_online, (None, None),
        )
    }

    /// Server-captured pupil/teacher levels, never client input. Offline levels
    /// are read inside the same durable relationship transaction; online levels
    /// must not be written over a peer's character save or revision.
    pub fn settle_shared_mentor_bank_with_live_levels(
        &self, actor: &Stage5FriendIdentity, expected_epoch: u64,
        end: Option<SharedMentorBreakReason>, now_ms: u64, teacher_online: bool,
        live_levels: (Option<u16>, Option<u16>),
    ) -> Result<SharedMentorAccountingReceipt, String> {
        let peer = self.shared_mentor_profile_for(actor)?.mentor.partner_identity
            .ok_or("server.NoMentorship")?;
        let mut accounts = vec![actor.account_id.clone(), peer.account_id.clone()];
        accounts.sort();
        accounts.dedup();
        self.commit_account_store_transaction(&accounts, |store| {
            let mut owner = profile(store, actor)?;
            let mut partner = profile(store, &peer)?;
            if owner.mentor.partner_identity.as_ref() != Some(&peer)
                || partner.mentor.partner_identity.as_ref() != Some(actor)
                || owner.mentor.ledger.relationship_epoch != expected_epoch
                || partner.mentor.ledger.relationship_epoch != expected_epoch
                || owner.mentor.is_mentor == partner.mentor.is_mentor
            { return Err("mentor relationship epoch changed".into()); }
            let (teacher, pupil) = if owner.mentor.is_mentor {
                (&mut owner, &mut partner)
            } else { (&mut partner, &mut owner) };
            validate_ledger(&teacher.mentor)?;
            validate_ledger(&pupil.mentor)?;
            match end {
                Some(SharedMentorBreakReason::LoginExpired)
                    if now_ms <= teacher.mentor.established_at_ms
                        .checked_add(SHARED_MENTOR_DURATION_MS).ok_or("mentor date exhausted")? =>
                    { return Err("mentor relationship is not expired".into()); }
                Some(SharedMentorBreakReason::Graduated)
                    if live_levels.0.unwrap_or(pupil.level)
                        .saturating_add(SHARED_MENTOR_LEVEL_GAP)
                        <= live_levels.1.unwrap_or(teacher.level) =>
                    { return Err("mentee has not graduated".into()); }
                _ => {}
            }
            let transferred = pupil.mentor.ledger.bank_earned
                .checked_sub(pupil.mentor.ledger.bank_settled).ok_or("mentor bank inconsistent")?;
            teacher.mentor.mentee_exp = teacher.mentor.mentee_exp
                .checked_add(i64::try_from(transferred).map_err(|_| "mentor bank too large")?)
                .ok_or("mentor bank exhausted")?;
            pupil.mentor.ledger.bank_settled = pupil.mentor.ledger.bank_earned;
            let teacher_id = teacher.identity.clone();
            if end.is_some() {
                let amount = u64::try_from(teacher.mentor.mentee_exp).map_err(|_| "negative mentor bank")?;
                let ledger = &mut teacher.mentor.ledger;
                let credit = if teacher_online { &mut ledger.leveling_credit } else { &mut ledger.balance_credit };
                *credit = credit.checked_add(amount).ok_or("mentor reward credit exhausted")?;
                if *credit > i64::MAX as u64 { return Err("mentor credit exceeds experience limit".into()); }
                teacher.mentor.mentee_exp = 0;
                for state in [&mut owner.mentor, &mut partner.mentor] {
                    state.partner_identity = None;
                    state.name.clear();
                    state.level = 0;
                    state.online = false;
                    state.is_mentor = false;
                    state.established_at_ms = 0;
                }
                if end == Some(SharedMentorBreakReason::Manual) {
                    owner.mentor.cooldown_until_ms = now_ms.checked_add(SHARED_MENTOR_DURATION_MS)
                        .ok_or("mentor cooldown exhausted")?;
                }
            }
            if transferred > 0 || end.is_some() {
                advance(&mut owner.mentor)?;
                advance(&mut partner.mentor)?;
                write_mentor(store, actor, owner.mentor)?;
                write_mentor(store, &peer, partner.mentor)?;
            }
            Ok(SharedMentorAccountingReceipt {
                participants: vec![actor.clone(), peer], teacher: teacher_id,
                transferred, ended: end.is_some(),
            })
        })
    }
}

fn validate_ledger(state: &Stage5MentorState) -> Result<(), String> {
    let ledger = &state.ledger;
    if state.mentee_exp < 0 || ledger.bank_settled > ledger.bank_earned
        || ledger.leveling_applied > ledger.leveling_credit
        || ledger.balance_applied > ledger.balance_credit
        || ledger.bank_earned > i64::MAX as u64
        || ledger.leveling_credit > i64::MAX as u64 || ledger.balance_credit > i64::MAX as u64
    { return Err("invalid mentor ledger".into()); }
    Ok(())
}

pub(super) fn has_novel_bank(local: &Stage5MentorState, durable: &Stage5MentorState) -> bool {
    local.ledger.bank_earned > durable.ledger.bank_earned
        || local.ledger.local_event_sequence > durable.ledger.local_event_sequence
        || !local.ledger.bank_events.is_subset(&durable.ledger.bank_events)
}

/// Authority refresh never acknowledges XP. Keep lifetime bank counters across
/// relationships, so already-committed old receipts remain distinguishable.
pub(super) fn merge_mentor_state(
    local: &Stage5MentorState, durable: &Stage5MentorState, reject_stale_gain: bool,
) -> Result<Stage5MentorState, String> {
    validate_ledger(local)?;
    validate_ledger(durable)?;
    let same_relationship = local.ledger.relationship_epoch == durable.ledger.relationship_epoch
        && local.partner_identity == durable.partner_identity && !local.is_mentor
        && !durable.is_mentor && durable.partner_identity.is_some();
    if has_novel_bank(local, durable) && !same_relationship && reject_stale_gain {
        return Err("mentor source relationship changed".into());
    }
    let mut result = durable.clone();
    if same_relationship {
        result.ledger.bank_earned = result.ledger.bank_earned.max(local.ledger.bank_earned);
        result.ledger.local_event_sequence = result.ledger.local_event_sequence.max(local.ledger.local_event_sequence);
        result.ledger.bank_events.extend(local.ledger.bank_events.iter().cloned());
    }
    if local.ledger.leveling_applied > durable.ledger.leveling_credit
        || local.ledger.balance_applied > durable.ledger.balance_credit
    { return Err("mentor checkpoint cannot mint reward credit".into()); }
    result.ledger.leveling_applied = local.ledger.leveling_applied;
    result.ledger.balance_applied = local.ledger.balance_applied;
    validate_ledger(&result)?;
    Ok(result)
}

#[derive(Resource)]
struct MentorSettlementCredit;

/// Called on the final XP amount, within the source's rollback checkpoint.
pub(super) fn record_gain(world: &mut World, amount: u64) -> Result<(), String> {
    if world.contains_resource::<MentorSettlementCredit>() { return Ok(()); }
    let session = world.resource::<super::resources::SessionResource>();
    let (Some(account_id), Some(character)) = (&session.account_id, &session.selected_character)
        else { return Ok(()); };
    let identity = Stage5FriendIdentity { account_id: account_id.clone(), character_index: character.index };
    let config = world.resource::<RuntimeConfigResource>().config.clone();
    let durable = config.shared_mentor_profile_for(&identity)?.mentor;
    let local = world.resource::<Stage5SystemsResource>().stage5_systems.mentor.clone();
    let mut mentor = merge_mentor_state(&local, &durable, true)?;
    let receipt = if let Some(permit) = world.get_resource::<super::shared_guild_experience::SharedKillExperiencePermit>() {
        let Some(capture) = permit.1.as_ref() else { return Ok(()); };
        if capture.pupil != identity || mentor.partner_identity.as_ref() != Some(&capture.teacher)
            || capture.relationship_epoch != mentor.ledger.relationship_epoch || mentor.is_mentor
        { return Ok(()); }
        Some(permit.0.kill_key.clone())
    } else { None };
    if super::shared_relationships::bank_mentor_gain(&mut mentor, amount, receipt.as_deref())? {
        world.resource_mut::<Stage5SystemsResource>().stage5_systems.mentor = mentor;
    }
    Ok(())
}

impl SimulationSession {
    /// Run outside a kill permit, after Zone life/vitals sync and before the
    /// next private command/save. Only this recipient's runtime is touched.
    pub fn consume_shared_mentor_credits(&mut self) -> Result<Vec<ServerPacket>, String> {
        let Some(active) = self.active_identity() else { return Ok(Vec::new()); };
        let Some(config) = self.shared_mentor_config() else { return Ok(Vec::new()); };
        if self.app.world().contains_resource::<super::shared_guild_experience::SharedKillExperiencePermit>() {
            return Err("mentor reward cannot run inside a kill source".into());
        }
        let identity = Stage5FriendIdentity { account_id: active.account_id, character_index: active.character_index };
        let durable = config.shared_mentor_profile_for(&identity)?.mentor;
        let local = self.app.world().resource::<Stage5SystemsResource>().stage5_systems.mentor.clone();
        let merged = merge_mentor_state(&local, &durable, true)?;
        let normal = merged.ledger.leveling_credit.checked_sub(merged.ledger.leveling_applied)
            .ok_or("mentor leveling credit inconsistent")?;
        let balance = merged.ledger.balance_credit.checked_sub(merged.ledger.balance_applied)
            .ok_or("mentor balance credit inconsistent")?;
        if normal == 0 && balance == 0 {
            self.app.world_mut().resource_mut::<Stage5SystemsResource>().stage5_systems.mentor = merged;
            return Ok(Vec::new());
        }
        let before = self.begin_guild_experience_command(true)?
            .ok_or("mentor reward checkpoint unavailable")?;
        self.app.world_mut().resource_mut::<Stage5SystemsResource>().stage5_systems.mentor = merged.clone();
        self.app.world_mut().insert_resource(MentorSettlementCredit);
        let result = (|| {
            let world = self.app.world_mut();
            let mut packets = Vec::new();
            if balance > 0 {
                let amount = i64::try_from(balance).map_err(|_| "mentor balance credit too large")?;
                let runtime = &mut world.resource_mut::<PlayerRuntimeResource>();
                runtime.experience = runtime.experience.checked_add(amount).ok_or("mentor balance exhausted")?;
                packets.push(ServerPacket::GainExperience { amount: u32::try_from(balance).unwrap_or(u32::MAX) });
            }
            let mut remaining = normal;
            // A durable bank can exceed one protocol gain. Bound work in each
            // owner turn; unconsumed credit remains durable for the next turn.
            for _ in 0..4 {
                if remaining == 0 { break; }
                // This is an ordinary GainExp source: current social/general
                // rates and NoExperience apply, with no monster multiplier.
                // Bound using every possible positive rate before truncation.
                let stats = super::stats::player_stats(world);
                let denominator = [120, 123, 100].into_iter().fold(1_u128,
                    |product, key| product * (100 + stats.get(key).max(0) as u128));
                let maximum_chunk = ((u128::from(u32::MAX) * 1_000_000 / denominator).max(1)) as u64;
                let chunk = remaining.min(maximum_chunk);
                let boosted = u64::from(super::shared_experience_profile::apply_current_personal_experience_rates(world,
                    u32::try_from(chunk).map_err(|_| "mentor source chunk exhausted")?));
                if boosted > u64::from(u32::MAX)
                    || world.resource::<PlayerRuntimeResource>().experience.checked_add(boosted as i64).is_none()
                {
                    return Err("mentor reward experience exhausted".into());
                }
                packets.extend(super::leveling::apply_experience_gain(world, boosted as i64));
                remaining -= chunk;
            }
            let ledger = &mut world.resource_mut::<Stage5SystemsResource>().stage5_systems.mentor.ledger;
            ledger.leveling_applied = merged.ledger.leveling_applied
                .checked_add(normal - remaining).ok_or("mentor applied credit exhausted")?;
            ledger.balance_applied = merged.ledger.balance_credit;
            Ok(packets)
        })();
        self.app.world_mut().remove_resource::<MentorSettlementCredit>();
        let packets = match result {
            Ok(packets) => packets,
            Err(error) => {
                super::shared_guild_experience::reject_source(self.app.world(), error);
                Vec::new()
            }
        };
        self.finish_guild_experience_command(Some(before), packets)
    }
}
