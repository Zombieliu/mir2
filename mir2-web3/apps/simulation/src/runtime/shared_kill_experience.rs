//! Trusted shared-envelope delivery. The receipt and complete private reward
//! checkpoint are committed together; client packets never call this entry.
use super::*;
use crate::runtime::zone::ZoneMonsterKillAward;
use crate::runtime::SharedAccountInventoryTransactionReceipt;
use sha2::{Digest, Sha256};
#[path = "prepared_kill_experience.rs"]
mod prepared;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SharedMonsterKillCommitFailure {
    Deferred(String),
    OutcomeUnknown(String),
}

impl SimulationSession {
    pub fn commit_shared_monster_kill_award_with_receipt(
        &mut self,
        key: &str,
        award: &ZoneMonsterKillAward,
    ) -> SharedAccountInventoryTransactionReceipt {
        self.try_commit_shared_monster_kill_award_with_receipt(key, award)
            .map(|(receipt, _)| receipt)
            .unwrap_or_else(|_| {
                SharedAccountInventoryTransactionReceipt::monster_kill_award(false, vec![])
            })
    }
    pub fn try_commit_shared_monster_kill_award_with_receipt(
        &mut self,
        key: &str,
        award: &ZoneMonsterKillAward,
    ) -> Result<(SharedAccountInventoryTransactionReceipt, bool), SharedMonsterKillCommitFailure>
    {
        let rejected = || {
            Ok((
                SharedAccountInventoryTransactionReceipt::monster_kill_award(false, vec![]),
                false,
            ))
        };
        if award
            .source_receipt_key
            .as_deref()
            .is_some_and(|source_key| source_key != key)
        {
            return rejected();
        }
        let Some(identity) = self.active_identity() else {
            return rejected();
        };
        let hash: String = match serde_json::to_vec(award) {
            Ok(bytes) => Sha256::digest(bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
            Err(_) => return rejected(),
        };
        let permit = match GuildExperienceCommitPermit::from_verified_shared_kill(
            Stage5FriendIdentity {
                account_id: identity.account_id.clone(),
                character_index: identity.character_index,
            },
            key.into(),
            hash.clone(),
            award.experience_selection.clone(),
        ) {
            Ok(permit) => permit,
            Err(_) => return rejected(),
        };
        if award
            .experience_selection
            .as_ref()
            .is_some_and(|selection| {
                selection.monster_object_id != award.monster_object_id
                    || selection.killed_at_ms != award.killed_at_ms
            })
        {
            return rejected();
        }
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        config
            .ensure_account_store_writable()
            .map_err(SharedMonsterKillCommitFailure::Deferred)?;
        let durable_receipt = match config.account_store.lock() {
            Ok(store) => store
                .accounts
                .get(&identity.account_id)
                .and_then(|account| account.saves.get(&identity.character_index))
                .map(|save| {
                    save.guild_experience_journal
                        .applied_kill_receipts
                        .get(key)
                        .cloned()
                }),
            Err(_) => return rejected(),
        };
        match durable_receipt {
            None => return rejected(),
            Some(Some(previous)) => {
                return Ok((
                    SharedAccountInventoryTransactionReceipt::monster_kill_award(
                        previous == hash,
                        vec![],
                    ),
                    previous == hash,
                ))
            }
            Some(None) => {}
        }
        if self
            .app
            .world()
            .contains_resource::<SharedKillExperiencePermit>()
        {
            return Err(SharedMonsterKillCommitFailure::Deferred(
                "shared kill source already executing".into(),
            ));
        }
        let before = match self.begin_guild_experience_command(true) {
            Ok(Some(before)) => Some(before),
            Ok(None) => return rejected(),
            Err(error) => return Err(SharedMonsterKillCommitFailure::Deferred(error)),
        };
        self.app
            .world_mut()
            .insert_resource(SharedKillExperiencePermit(permit));
        let receipt = self.commit_shared_monster_kill_award_transaction_impl(
            award.monster_object_id,
            &award.monster_name,
            award.experience,
            award
                .experience_selection
                .as_ref()
                .map(|selection| selection.final_amount),
        );
        if receipt.committed {
            self.app
                .world()
                .resource::<GuildExperienceResource>()
                .0
                .lock()
                .expect("guild XP journal poisoned")
                .applied_kill_receipts
                .insert(key.into(), hash);
        } else {
            *self
                .app
                .world()
                .resource::<GuildExperienceResource>()
                .2
                .lock()
                .expect("guild XP error poisoned") =
                Some("shared kill gameplay was rejected".into());
        }
        let result = self.finish_guild_experience_command(before, receipt.packets);
        self.app
            .world_mut()
            .remove_resource::<SharedKillExperiencePermit>();
        match result {
            Ok(packets) => Ok((
                SharedAccountInventoryTransactionReceipt::monster_kill_award(
                    true,
                    self.finalize_packets(packets),
                ),
                false,
            )),
            Err(error) if config.ensure_account_store_writable().is_err() => {
                Err(SharedMonsterKillCommitFailure::OutcomeUnknown(error))
            }
            Err(error) => Err(SharedMonsterKillCommitFailure::Deferred(error)),
        }
    }
}
