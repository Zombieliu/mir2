//! Atomic source checkpoint + bilateral mentorship bank settlement.
use super::shared_relationships::{
    advance, profile, write_mentor, SHARED_MENTOR_DURATION_MS, SHARED_MENTOR_LEVEL_GAP,
};
use crate::config::{CharacterSaveRecord, Stage5FriendIdentity};
use crate::SimulationConfig;

#[derive(Debug, Clone)]
pub struct SharedMentorLiveCheckpoint {
    pub identity: Stage5FriendIdentity,
    pub save: CharacterSaveRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharedMentorBreakReason {
    Manual,
    LoginExpired,
    Graduated,
}

#[derive(Debug, Clone)]
pub struct SharedMentorAccountingReceipt {
    /// Expected and committed full-save revisions, acknowledged under the pair gate.
    pub revisions: Vec<(Stage5FriendIdentity, u64, u64)>,
}

pub(super) fn apply_saved_mentor_credit(
    save: &mut CharacterSaveRecord,
    amount: u64,
    level_up: bool,
) -> Result<(), String> {
    if amount == 0 {
        return Ok(());
    }
    let amount = i64::try_from(amount).map_err(|_| "mentor reward exceeds experience limit")?;
    save.experience = save
        .experience
        .checked_add(amount)
        .ok_or("mentor reward balance exhausted")?;
    if level_up {
        while save.character.level < super::leveling::CRYSTAL_MAX_LEVEL
            && save.max_experience > 0
            && save.experience >= save.max_experience
        {
            save.experience -= save.max_experience;
            save.character.level += 1;
            save.max_experience =
                super::leveling::crystal_max_experience_for_level(save.character.level);
        }
    }
    Ok(())
}

impl SimulationConfig {
    pub fn commit_shared_mentor_accounting(
        &self,
        actor: &Stage5FriendIdentity,
        expected_epoch: u64,
        checkpoints: &[SharedMentorLiveCheckpoint],
        end: Option<SharedMentorBreakReason>,
        now_ms: u64,
        online_teacher_bonus_percent: Option<u16>,
    ) -> Result<SharedMentorAccountingReceipt, String> {
        let peer = self
            .shared_mentor_profile_for(actor)?
            .mentor
            .partner_identity
            .ok_or("server.NoMentorship")?;
        let identities = [actor.clone(), peer.clone()];
        let mut accounts = identities
            .iter()
            .map(|id| id.account_id.clone())
            .collect::<Vec<_>>();
        accounts.sort();
        accounts.dedup();
        self.commit_account_store_transaction(&accounts, |store| {
            let mut revisions = Vec::new();
            for id in &identities {
                let persisted = store
                    .accounts
                    .get(&id.account_id)
                    .and_then(|a| a.saves.get(&id.character_index))
                    .ok_or("mentor source save missing")?
                    .clone();
                let supplied = checkpoints
                    .iter()
                    .filter(|c| c.identity == *id)
                    .collect::<Vec<_>>();
                if supplied.len() > 1 {
                    return Err("duplicate mentor source checkpoint".into());
                }
                let mut next = if let Some(checkpoint) = supplied.first() {
                    if checkpoint.save.revision != persisted.revision
                        || checkpoint.save.character.index != id.character_index
                    {
                        return Err("stale mentor source checkpoint".into());
                    }
                    let mut next = checkpoint.save.clone();
                    super::save::merge_persisted_mail_into_character_save(&mut next, &persisted)?;
                    next
                } else {
                    persisted.clone()
                };
                super::save::validate_character_save_record(&next)?;
                next.revision = persisted
                    .revision
                    .checked_add(1)
                    .ok_or("mentor source revision exhausted")?;
                revisions.push((id.clone(), persisted.revision, next.revision));
                let account = store.accounts.get_mut(&id.account_id).unwrap();
                let character = account
                    .characters
                    .iter_mut()
                    .find(|c| c.index == id.character_index)
                    .ok_or("mentor source character missing")?;
                *character = next.character.clone();
                account.saves.insert(id.character_index, next);
            }
            if checkpoints
                .iter()
                .any(|c| !identities.contains(&c.identity))
            {
                return Err("unrelated mentor source checkpoint".into());
            }
            let mut owner = profile(store, actor)?;
            let mut partner = profile(store, &peer)?;
            if owner.mentor.partner_identity.as_ref() != Some(&peer)
                || partner.mentor.partner_identity.as_ref() != Some(actor)
                || owner.mentor.ledger.relationship_epoch != expected_epoch
                || partner.mentor.ledger.relationship_epoch != expected_epoch
                || owner.mentor.is_mentor == partner.mentor.is_mentor
            {
                return Err("mentor relationship epoch changed".into());
            }
            let (teacher, pupil) = if owner.mentor.is_mentor {
                (&mut owner, &mut partner)
            } else {
                (&mut partner, &mut owner)
            };
            match end {
                Some(SharedMentorBreakReason::LoginExpired)
                    if now_ms
                        <= teacher
                            .mentor
                            .established_at_ms
                            .saturating_add(SHARED_MENTOR_DURATION_MS) =>
                {
                    return Err("mentor relationship is not expired".into())
                }
                Some(SharedMentorBreakReason::Graduated)
                    if pupil.level.saturating_add(SHARED_MENTOR_LEVEL_GAP) <= teacher.level =>
                {
                    return Err("mentee has not graduated".into())
                }
                _ => {}
            }
            let unsettled = pupil
                .mentor
                .ledger
                .bank_earned
                .checked_sub(pupil.mentor.ledger.bank_settled)
                .ok_or("mentor bank is inconsistent")?;
            teacher.mentor.mentee_exp = teacher
                .mentor
                .mentee_exp
                .checked_add(i64::try_from(unsettled).map_err(|_| "mentor bank is too large")?)
                .ok_or("mentor bank exhausted")?;
            if teacher.mentor.mentee_exp < 0 {
                return Err("negative mentor bank".into());
            }
            pupil.mentor.ledger.bank_settled = pupil.mentor.ledger.bank_earned;
            if end.is_some() {
                let bank = teacher.mentor.mentee_exp as u64;
                let online = online_teacher_bonus_percent.is_some();
                if online && !checkpoints.iter().any(|c| c.identity == teacher.identity) {
                    return Err("online teacher checkpoint required".into());
                }
                let bonus = bank
                    .checked_mul(u64::from(online_teacher_bonus_percent.unwrap_or(0)))
                    .ok_or("mentor reward bonus exhausted")?
                    / 100;
                let amount = bank.checked_add(bonus).ok_or("mentor reward exhausted")?;
                let ledger = &mut teacher.mentor.ledger;
                if online {
                    ledger.leveling_credit = ledger
                        .leveling_credit
                        .checked_add(amount)
                        .ok_or("mentor credit exhausted")?;
                    ledger.leveling_applied = ledger.leveling_credit;
                } else {
                    ledger.balance_credit = ledger
                        .balance_credit
                        .checked_add(amount)
                        .ok_or("mentor credit exhausted")?;
                    ledger.balance_applied = ledger.balance_credit;
                }
                let account = store
                    .accounts
                    .get_mut(&teacher.identity.account_id)
                    .unwrap();
                let save = account
                    .saves
                    .get_mut(&teacher.identity.character_index)
                    .unwrap();
                apply_saved_mentor_credit(save, amount, online)?;
                *account
                    .characters
                    .iter_mut()
                    .find(|c| c.index == teacher.identity.character_index)
                    .unwrap() = save.character.clone();
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
                    owner.mentor.cooldown_until_ms =
                        now_ms.saturating_add(SHARED_MENTOR_DURATION_MS);
                }
            }
            advance(&mut owner.mentor)?;
            advance(&mut partner.mentor)?;
            write_mentor(store, actor, owner.mentor)?;
            write_mentor(store, &peer, partner.mentor)?;
            Ok(SharedMentorAccountingReceipt { revisions })
        })
    }
}
