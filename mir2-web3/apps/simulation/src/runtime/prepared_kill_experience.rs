//! Execute private kill effects behind a caller-owned PostgreSQL transaction.
use super::*;
use crate::{PreparedKillAccountSource, PreparedKillPublication, PreparedKillPublicationFailure};

impl SimulationSession {
    pub fn reconcile_shared_monster_kill_receipt(
        &mut self,
        key: &str,
        hash: &str,
    ) -> Result<(), PreparedKillPublicationFailure> {
        let identity = self
            .active_identity()
            .ok_or_else(|| "kill recovery requires active identity".to_string())?;
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        config.ensure_account_store_writable()?;
        if snapshot(self.app.world())
            .applied_kill_receipts
            .get(key)
            .is_some_and(|existing| existing == hash)
        {
            return Ok(());
        }
        config.refresh_account_store_account(&identity.account_id)?;
        let save = config
            .account_store
            .lock()
            .map_err(|_| "kill recovery account image poisoned".to_string())?
            .accounts
            .get(&identity.account_id)
            .and_then(|a| a.saves.get(&identity.character_index))
            .cloned()
            .ok_or_else(|| "kill recovery checkpoint missing".to_string())?;
        if save
            .guild_experience_journal
            .applied_kill_receipts
            .get(key)
            .map(String::as_str)
            != Some(hash)
        {
            return Err(PreparedKillPublicationFailure::OutcomeUnknown(
                config.freeze_prepared_kill_source(
                    "kill ledger committed but canonical source requires reconciliation".into(),
                ),
            ));
        }
        self.restore_active_character_checkpoint(&save)
            .map_err(|error| {
                PreparedKillPublicationFailure::OutcomeUnknown(
                    config.freeze_prepared_kill_source(error),
                )
            })
    }

    pub fn commit_shared_monster_kill_via_postgres<T, F>(
        &mut self,
        key: &str,
        award: &ZoneMonsterKillAward,
        publish: F,
    ) -> Result<(T, Vec<ServerPacket>), PreparedKillPublicationFailure>
    where
        F: FnOnce(
            &PreparedKillAccountSource,
        ) -> Result<PreparedKillPublication<T>, PreparedKillPublicationFailure>,
    {
        let identity = self
            .active_identity()
            .ok_or_else(|| "prepared kill requires active identity".to_string())?;
        if award.source_receipt_key.as_deref() != Some(key) {
            return Err("prepared kill requires its immutable delivery key"
                .to_string()
                .into());
        }
        if award
            .experience_selection
            .as_ref()
            .is_some_and(|selection| {
                selection.monster_object_id != award.monster_object_id
                    || selection.killed_at_ms != award.killed_at_ms
            })
        {
            return Err("prepared kill source selection mismatch".to_string().into());
        }
        let hash: String = Sha256::digest(serde_json::to_vec(award).map_err(|e| e.to_string())?)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let permit = GuildExperienceCommitPermit::from_verified_shared_kill(
            Stage5FriendIdentity {
                account_id: identity.account_id.clone(),
                character_index: identity.character_index,
            },
            key.into(),
            hash.clone(),
            award.experience_selection.clone(),
        )?;
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        config.ensure_account_store_writable()?;
        if self
            .app
            .world()
            .contains_resource::<SharedKillExperiencePermit>()
        {
            return Err("prepared kill already active".to_string().into());
        }
        let before = self
            .begin_guild_experience_command(true)?
            .ok_or_else(|| "prepared kill checkpoint unavailable".to_string())?;
        self.app
            .world_mut()
            .insert_resource(SharedKillExperiencePermit(permit.clone()));
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
                .expect("guild source poisoned")
                .applied_kill_receipts
                .insert(key.into(), hash);
        }
        let source_error = self
            .app
            .world()
            .resource::<GuildExperienceResource>()
            .2
            .lock()
            .expect("guild source error poisoned")
            .take();
        let preparation = if !receipt.committed {
            Err("prepared kill gameplay rejected".into())
        } else if let Some(error) = source_error {
            Err(error)
        } else if let Some(save) = self.active_character_checkpoint() {
            config.prepare_kill_account_source(&permit, before.save.clone(), |store| {
                crate::runtime::save::stage_prepared_character_save(self.app.world(), store, save)
            })
        } else {
            Err("prepared kill full checkpoint unavailable".into())
        };
        self.app
            .world()
            .resource::<GuildExperienceResource>()
            .1
            .store(false, Ordering::Release);
        self.app
            .world_mut()
            .remove_resource::<SharedKillExperiencePermit>();
        let source = match preparation {
            Ok(source) => source,
            Err(error) => {
                self.restore_prepared_kill_transient(&before, &before.save)?;
                return Err(error.into());
            }
        };
        // Keep original monotonic anchors when applying the merged, revisioned
        // full checkpoint. Rebuilding from serialized duration would extend buffs.
        let after = GuildExperienceCheckpoint {
            save: self
                .active_character_checkpoint()
                .ok_or_else(|| "prepared after checkpoint missing".to_string())?,
            visible: self.visible_objects.clone(),
            dirty_economy: self.dirty_economy_projection_event_ids.clone(),
            buffs: self.app.world().resource::<BuffResource>().clone(),
            npc: self.app.world().resource::<NpcStateResource>().clone(),
            queue: self.app.world().resource::<RuntimeQueueResource>().clone(),
            tick: runtime_tick(self.app.world()),
            hero: crate::runtime::hero_ai::hero_transient::capture(self.app.world()),
        };
        match config.publish_prepared_kill_source(&source, publish) {
            Ok(PreparedKillPublication::Written(value)) => {
                self.restore_prepared_kill_transient(&after, source.checkpoint())
                    .map_err(|error| {
                        PreparedKillPublicationFailure::OutcomeUnknown(
                            config.freeze_prepared_kill_source(error),
                        )
                    })?;
                Ok((value, self.finalize_packets(receipt.packets)))
            }
            Ok(PreparedKillPublication::AlreadyCommitted(value)) => {
                self.restore_prepared_kill_transient(&before, &before.save)?;
                config.refresh_account_store_account(&identity.account_id)?;
                let saved = config
                    .account_store
                    .lock()
                    .map_err(|_| "source replay image poisoned".to_string())?
                    .accounts
                    .get(&identity.account_id)
                    .and_then(|a| a.saves.get(&identity.character_index))
                    .cloned()
                    .ok_or_else(|| "source replay checkpoint missing".to_string())?;
                if saved
                    .guild_experience_journal
                    .applied_kill_receipts
                    .get(key)
                    != Some(&permit.kill_payload_hash)
                {
                    return Err(PreparedKillPublicationFailure::OutcomeUnknown(
                        config.freeze_prepared_kill_source(
                            "committed kill source requires canonical replay reconciliation".into(),
                        ),
                    ));
                }
                self.restore_prepared_kill_transient(&before, &saved)?;
                Ok((value, vec![]))
            }
            Err(PreparedKillPublicationFailure::Rejected(error)) => {
                if config.ensure_account_store_writable().is_err() {
                    return Err(PreparedKillPublicationFailure::OutcomeUnknown(error));
                }
                self.restore_prepared_kill_transient(&before, &before.save)?;
                Err(PreparedKillPublicationFailure::Rejected(error))
            }
            Err(error @ PreparedKillPublicationFailure::OutcomeUnknown(_)) => Err(error),
        }
    }

    fn restore_prepared_kill_transient(
        &mut self,
        snapshot: &GuildExperienceCheckpoint,
        save: &crate::CharacterSaveRecord,
    ) -> Result<(), String> {
        self.restore_active_character_checkpoint(save)?;
        *self.app.world_mut().resource_mut::<BuffResource>() = snapshot.buffs.clone();
        *self.app.world_mut().resource_mut::<NpcStateResource>() = snapshot.npc.clone();
        *self.app.world_mut().resource_mut::<RuntimeQueueResource>() = snapshot.queue.clone();
        set_runtime_tick(self.app.world_mut(), snapshot.tick);
        crate::runtime::hero_ai::hero_transient::restore(self.app.world_mut(), &snapshot.hero)?;
        self.visible_objects = snapshot.visible.clone();
        self.dirty_economy_projection_event_ids = snapshot.dirty_economy.clone();
        Ok(())
    }
}
