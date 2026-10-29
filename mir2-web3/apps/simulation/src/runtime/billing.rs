//! Apply externally confirmed recharge receipts on the serial private owner.
use crate::billing::{
    apply_pending_to_save, has_pending, validate_complete_store, RechargeCreditApplication,
};
use mir2_protocol::ServerPacket;

use super::resources::{PlayerRuntimeResource, RuntimeConfigResource, SessionResource};
use super::save::{
    active_session_mutating_account_id, merge_persisted_mail_into_character_save,
    snapshot_active_character_save, validate_character_save_record,
};
use super::session::SimulationSession;

impl SimulationSession {
    /// Ordinary Source Tick calls this with current wall-clock milliseconds.
    /// Disabled accounts with cached absent/empty ledgers need no repository
    /// I/O. Configure billing_monthly_card_credit_price consistently on Source
    /// and Gateway so enabled accounts also discover their first external order.
    /// Explicit refresh and purchase preflight remain immediate.
    pub fn poll_pending_recharge_credits(
        &mut self,
        now_ms: u64,
    ) -> Result<Vec<ServerPacket>, String> {
        let Some(identity) = self.active_identity().filter(|identity| {
            !identity.account_id.is_empty()
                && identity.account_id.as_str() == identity.account_id.trim()
                && super::resources::is_in_world(self.app.world())
        }) else {
            self.billing_last_recharge_poll = None;
            return Ok(Vec::new());
        };
        let config = &self.app.world().resource::<RuntimeConfigResource>().config;
        let has_ledger = config
            .account_store
            .lock()
            .map_err(|_| "billingStoreUnavailable")?
            .accounts
            .get(&identity.account_id)
            .ok_or("billingAccountMissing")?
            .billing
            .as_ref()
            .is_some_and(|ledger| !ledger.orders.is_empty());
        if !has_ledger && config.billing_monthly_card_credit_price.is_none() {
            self.billing_last_recharge_poll = None;
            return Ok(Vec::new());
        }
        if self
            .billing_last_recharge_poll
            .as_ref()
            .is_some_and(|(previous, at)| {
                previous == &identity && now_ms >= *at && now_ms - *at < 5_000
            })
        {
            return Ok(Vec::new());
        }
        // Throttle failed repository polls too. A clock rollback or a different
        // authenticated character starts a new interval instead of inheriting it.
        self.billing_last_recharge_poll = Some((identity, now_ms));
        self.consume_pending_recharge_credits()
    }

    /// Trusted Source commands may obtain the account established by successful
    /// Login/Passkey authentication, including before character StartGame.
    /// No client-provided account ID or development fallback is accepted here.
    pub fn billing_authenticated_config(
        &self,
    ) -> Result<(crate::SimulationConfig, String), String> {
        let session = self.app.world().resource::<SessionResource>();
        let account_id =
            active_session_mutating_account_id(session).ok_or("billingAuthenticationRequired")?;
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        config.refresh_account_store_account(&account_id)?;
        {
            let store = config
                .account_store
                .lock()
                .map_err(|_| "billingStoreUnavailable")?;
            validate_complete_store(&store)?;
            let account = store
                .accounts
                .get(&account_id)
                .ok_or("billingAccountMissing")?;
            if account
                .active_ban(crate::monthly_card::monthly_card_now_ms())
                .is_some()
            {
                return Err("billingAccountBanned".into());
            }
            if let Some(selected) = &session.selected_character {
                if !account.characters.iter().any(|character| {
                    character.index == selected.index
                        && character.name == selected.name
                        && character.class == selected.class
                        && character.gender == selected.gender
                }) {
                    return Err("billingCharacterIdentityMismatch".into());
                }
            }
        }
        Ok((config, account_id))
    }

    /// Call outside any Zone writer, before normal Tick or purchase preflight.
    /// Payment confirmation alone never advances this owner's save revision.
    pub fn consume_pending_recharge_credits(&mut self) -> Result<Vec<ServerPacket>, String> {
        let Some(identity) = self.active_identity() else {
            return Ok(Vec::new());
        };
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        config.refresh_account_store_account(&identity.account_id)?;
        {
            let store = config
                .account_store
                .lock()
                .map_err(|_| "billingStoreUnavailable")?;
            validate_complete_store(&store)?;
            let account = store
                .accounts
                .get(&identity.account_id)
                .ok_or("billingAccountMissing")?;
            if !has_pending(account, identity.character_index) {
                return Ok(Vec::new());
            }
        }
        let mut checkpoint =
            snapshot_active_character_save(self.app.world()).ok_or("billingOwnerNotInWorld")?;
        let expected_revision = checkpoint.revision;
        validate_character_save_record(&checkpoint)?;
        let now_ms = crate::monthly_card::monthly_card_now_ms();
        let result =
            config.commit_account_store_transaction(&[identity.account_id.clone()], |store| {
                let account = store
                    .accounts
                    .get_mut(&identity.account_id)
                    .ok_or("billingAccountMissing")?;
                let character = account
                    .characters
                    .iter()
                    .find(|c| c.index == identity.character_index)
                    .ok_or("billingCharacterMissing")?;
                let durable = account
                    .saves
                    .get(&identity.character_index)
                    .ok_or("billingCharacterSaveMissing")?;
                let matches = |c: &crate::CharacterRecord| {
                    c.index == character.index
                        && c.name == character.name
                        && c.class == character.class
                        && c.gender == character.gender
                };
                if character.name != identity.character_name
                    || !matches(&durable.character)
                    || !matches(&checkpoint.character)
                {
                    return Err("billingCharacterIdentityMismatch".into());
                }
                // A review may have removed all pending orders after the initial
                // read. Do not publish the checkpoint without a revision advance.
                if !has_pending(account, identity.character_index) {
                    return Ok(RechargeCreditApplication {
                        credit: durable.credit,
                        granted: 0,
                        revision: durable.revision,
                        order_ids: Vec::new(),
                    });
                }
                // Rebase only a current checkpoint. A newer inventory/XP/mail claim
                // cannot be replaced with this owner's older complete snapshot.
                if durable.revision != expected_revision {
                    return Err("billingOwnerCheckpointStale".into());
                }
                merge_persisted_mail_into_character_save(&mut checkpoint, durable)?;
                let result = apply_pending_to_save(account, &mut checkpoint, now_ms)?;
                if result.granted != 0 {
                    validate_character_save_record(&checkpoint)?;
                    account.saves.insert(identity.character_index, checkpoint);
                }
                Ok(result)
            })?;
        if result.granted == 0 {
            return Ok(Vec::new());
        }
        if !self
            .app
            .world()
            .resource::<SessionResource>()
            .advance_active_save_revision(expected_revision, result.revision)
        {
            // The durable receipts already say Applied. Do not describe this as
            // a rollback or grant again; the gateway must close/reconcile owner.
            return Err("billingOwnerPublicationOutcomeUnknown".into());
        }
        self.app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .credit = result.credit;
        Ok(vec![ServerPacket::GainedCredit {
            credit: result.granted,
        }])
    }

    pub fn refresh_recharge_credit_preflight(&mut self) -> Result<Vec<ServerPacket>, String> {
        self.consume_pending_recharge_credits()
    }
}
