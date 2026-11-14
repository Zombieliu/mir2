//! Trusted continuation of an admission, separate from player StartGame.
use super::*;
use crate::world_runtime::{
    CharacterHandoffCheckpoint, CharacterHandoffClockWitness, CharacterHandoffCommitment,
};
use crate::ActiveSessionIdentity;

#[derive(bevy_ecs::prelude::Resource)]
pub(super) struct LastRestoreClock(pub super::super::refine_oven::RefineClockSample);

#[derive(bevy_ecs::prelude::Resource)]
struct HandoffInstallWitness {
    checkpoint: serde_json::Value,
    clock: super::super::refine_oven::RefineClockSample,
}

fn same_record(left: &CharacterSaveRecord, right: &CharacterSaveRecord) -> Result<bool, String> {
    Ok(serde_json::to_value(left).map_err(|e| e.to_string())?
        == serde_json::to_value(right).map_err(|e| e.to_string())?)
}

fn authority_account(
    config: &SimulationConfig,
    account_id: &str,
) -> Result<crate::config::AccountRecord, String> {
    config.ensure_account_store_writable()?;
    if config.account_store_database_mode == crate::config::AccountStoreDatabaseMode::SourceOfTruth
    {
        config.refresh_account_store_account(account_id)?;
    } else if let Some(path) = config.account_store_path.as_ref() {
        // A missing/corrupt source is an error, never an opportunity to seed a
        // demo account. Read the actual published File, not a login projection.
        let bytes = std::fs::read(path).map_err(|e| format!("handoff source read failed: {e}"))?;
        let store: crate::config::AccountStore = serde_json::from_slice(&bytes)
            .map_err(|e| format!("handoff source decode failed: {e}"))?;
        return store
            .accounts
            .get(account_id)
            .cloned()
            .ok_or("handoff source account is missing".into());
    }
    config
        .account_store
        .lock()
        .map_err(|_| "handoff account image lock poisoned")?
        .accounts
        .get(account_id)
        .cloned()
        .ok_or("handoff source account is missing".into())
}

fn authenticated_select(world: &World, save: &CharacterSaveRecord) -> Result<String, String> {
    let session = world.resource::<SessionResource>();
    let CrystalCharacterSelectState::Authenticated { account_id } =
        crystal_character_select_state(session)
    else {
        return Err("handoff resume requires an authenticated character-select session".into());
    };
    let member = session.characters.iter().any(|character| {
        character.index == save.character.index
            && character.name == save.character.name
            && character.class == save.character.class
            && character.gender == save.character.gender
    });
    if !member {
        return Err("handoff character is outside its authenticated roster".into());
    }
    let config = &world.resource::<RuntimeConfigResource>().config;
    if config.content_profile.as_ref().is_some_and(|profile| {
        !profile.class_is_allowed(save.character.class)
            || !profile.map_is_allowed(&save.map_file_name)
    }) {
        return Err("handoff checkpoint is outside the configured content profile".into());
    }
    validate_character_save_record(save)?;
    Ok(account_id)
}

fn stage5_value(save: &CharacterSaveRecord) -> Result<serde_json::Value, String> {
    serde_json::from_str(
        save.stage5_systems_json
            .as_deref()
            .ok_or("handoff Stage5 image is missing")?,
    )
    .map_err(|e| e.to_string())
}

impl CharacterHandoffCheckpoint {
    fn validate_source_pair(&self) -> Result<(), String> {
        if self.account_id.is_empty() || self.account_id.chars().any(char::is_control) {
            return Err("handoff source account identity is invalid".into());
        }
        validate_character_save_record(&self.durable)?;
        validate_character_save_record(&self.runtime)?;
        let mut runtime = self.runtime.clone();
        if runtime.skill_states_json.len() != self.durable.skill_states_json.len() {
            return Err("handoff skill count changed".into());
        }
        for (durable, runtime) in self
            .durable
            .skill_states_json
            .iter()
            .zip(&runtime.skill_states_json)
        {
            let mut saved: serde_json::Value =
                serde_json::from_str(durable).map_err(|e| e.to_string())?;
            let live: serde_json::Value =
                serde_json::from_str(runtime).map_err(|e| e.to_string())?;
            let metadata = saved
                .as_object_mut()
                .ok_or("handoff skill is invalid")?
                .remove(SKILL_CLOCK_METADATA_KEY)
                .ok_or("handoff durable skill clock is missing")?;
            let fields = metadata
                .as_object()
                .ok_or("handoff durable skill clock is invalid")?;
            if fields.len() != 3
                || !["schema", "savedAtUnixMs", "remainingMs"]
                    .iter()
                    .all(|key| {
                        fields
                            .get(*key)
                            .and_then(serde_json::Value::as_u64)
                            .is_some()
                    })
            {
                return Err("handoff durable skill clock has unknown or invalid fields".into());
            }
            let clock: DurableSkillClock =
                serde_json::from_value(metadata).map_err(|e| e.to_string())?;
            let skill: SkillState =
                serde_json::from_value(live.clone()).map_err(|e| e.to_string())?;
            let remaining = skill
                .cooldown_ends_at
                .saturating_sub(self.source_tick)
                .min(u64::from(skill.cooldown_ticks))
                .saturating_mul(1_000);
            if clock.schema != DURABLE_SKILL_CLOCK_SCHEMA
                || clock.remaining_ms != remaining
                || (remaining == 0) != (clock.saved_at_unix_ms == 0)
                || saved != live
            {
                return Err(
                    "handoff runtime skill does not match its complete durable clock".into(),
                );
            }
        }
        runtime.skill_states_json = self.durable.skill_states_json.clone();
        // Runtime transport is projected from this very committed row. Refine
        // fields are frozen exactly; only a trusted target restore may age it.
        if !same_record(&runtime, &self.durable)? {
            return Err("handoff runtime changed complete durable contents".into());
        }
        Ok(())
    }

    /// Complete post-install comparison; only the trusted refine clock rebase
    /// and monotonic aging have a different wire representation.
    pub fn validate_commitment(
        &self,
        commitment: &CharacterHandoffCommitment,
    ) -> Result<(), String> {
        self.validate_source_pair()?;
        let projected = &commitment.runtime;
        validate_character_save_record(projected)?;
        if commitment.restored_clock.epoch != commitment.observed_clock.epoch
            || commitment.restored_clock.now_ms > commitment.observed_clock.now_ms
        {
            return Err("handoff target clock witness is not monotonic".into());
        }
        let restored = super::super::refine_oven::RefineClockSample {
            epoch: commitment.restored_clock.epoch.clone(),
            now_ms: commitment.restored_clock.now_ms,
        };
        let observed = super::super::refine_oven::RefineClockSample {
            epoch: commitment.observed_clock.epoch.clone(),
            now_ms: commitment.observed_clock.now_ms,
        };
        let mut systems: Stage5SystemsState = serde_json::from_str(
            self.runtime
                .stage5_systems_json
                .as_deref()
                .ok_or("handoff Stage5 image is missing")?,
        )
        .map_err(|e| e.to_string())?;
        super::super::refine_oven::restore_timer_at(&mut systems.refine, &restored)?;
        super::super::refine_oven::snapshot_timer_at(&mut systems.refine, true, &observed);
        let mut expected = stage5_value(&self.runtime)?;
        let clocks = serde_json::to_value(systems.refine).map_err(|e| e.to_string())?;
        let refine = expected
            .get_mut("refine")
            .and_then(serde_json::Value::as_object_mut)
            .ok_or("handoff raw refine image is missing")?;
        // A typed round trip would silently discard future fields or fill
        // defaults. Only these four clock fields have an allowed conversion.
        for field in ["remainingMs", "clockEpoch", "collectDeadlineMs", "ready"] {
            refine.insert(
                field.into(),
                clocks
                    .get(field)
                    .ok_or("handoff refine clock field is missing")?
                    .clone(),
            );
        }
        let actual = stage5_value(projected)?;
        if actual != expected {
            return Err("handoff target changed complete Stage5 contents".into());
        }
        let mut normalized = projected.clone();
        if normalized.skill_states_json.len() != self.runtime.skill_states_json.len() {
            return Err("handoff target changed skill count".into());
        }
        for (actual, expected) in normalized
            .skill_states_json
            .iter()
            .zip(&self.runtime.skill_states_json)
        {
            let actual: serde_json::Value =
                serde_json::from_str(actual).map_err(|e| e.to_string())?;
            let expected: serde_json::Value =
                serde_json::from_str(expected).map_err(|e| e.to_string())?;
            if actual != expected {
                return Err("handoff target changed complete skill contents".into());
            }
        }
        normalized.skill_states_json = self.runtime.skill_states_json.clone();
        normalized.stage5_systems_json = self.runtime.stage5_systems_json.clone();
        if !same_record(&normalized, &self.runtime)? {
            return Err("handoff target changed complete runtime contents".into());
        }
        Ok(())
    }
}

impl SimulationSession {
    /// The caller is the server's lease-checked handoff seam, not a credential
    /// login. Selecting a proven source admission never heals a password,
    /// grants a recovery-journal capability, emits Login, or publishes a save.
    fn select_continuation_account(
        &mut self,
        account_id: &str,
        account: &crate::config::AccountRecord,
    ) -> Result<(), String> {
        let session = self.app.world().resource::<SessionResource>();
        if session.selected_character.is_some()
            || session
                .account_id
                .as_deref()
                .is_some_and(|current| current != account_id)
        {
            return Err("continuation requires an inactive target for the same account".into());
        }
        if account.active_ban(unix_now_ms()).is_some() {
            return Err("continuation account is banned".into());
        }
        crate::runtime::recovery::clear_binding(self.app.world_mut());
        let mut session = self.app.world_mut().resource_mut::<SessionResource>();
        session.account_id = Some(account_id.into());
        session.characters = account.characters.clone();
        session.selected_character = None;
        session.clear_active_save_revision();
        Ok(())
    }

    pub(crate) fn resume_authenticated_character_handoff(
        &mut self,
        checkpoint: &CharacterHandoffCheckpoint,
    ) -> Result<CharacterHandoffCommitment, String> {
        checkpoint.validate_source_pair()?;
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        if config.has_isolated_replica_bootstrap_authority() {
            return Err("live handoff cannot use replica authority".into());
        }
        let account = authority_account(&config, &checkpoint.account_id)?;
        if !account
            .characters
            .iter()
            .any(|character| character == &checkpoint.durable.character)
            || !account
                .saves
                .get(&checkpoint.durable.character.index)
                .map(|save| same_record(save, &checkpoint.durable))
                .transpose()?
                .unwrap_or(false)
        {
            return Err("handoff requires its complete saved authority baseline".into());
        }
        self.select_continuation_account(&checkpoint.account_id, &account)?;
        let previous_tick = runtime_tick(self.app.world());
        let result = self.resume_character_handoff(checkpoint);
        if result.is_err() {
            self.abandon_failed_handoff(config, previous_tick);
        }
        result
    }

    pub(crate) fn apply_authenticated_character_handoff_replica(
        &mut self,
        checkpoint: &CharacterHandoffCheckpoint,
    ) -> Result<(), String> {
        checkpoint.validate_source_pair()?;
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        if !config.has_isolated_replica_bootstrap_authority() {
            return Err("handoff replay requires its bound isolated authority".into());
        }
        let account = authority_account(&config, &checkpoint.account_id)?;
        self.select_continuation_account(&checkpoint.account_id, &account)?;
        let previous_tick = runtime_tick(self.app.world());
        let result = self.apply_character_handoff_replica(checkpoint);
        if result.is_err() {
            self.abandon_failed_handoff(config, previous_tick);
        }
        result
    }

    pub(crate) fn restore_replica_session_checkpoint(
        &mut self,
        identity: &ActiveSessionIdentity,
        checkpoint: &CharacterSaveRecord,
    ) -> Result<(), String> {
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        if !config.has_isolated_replica_bootstrap_authority() {
            return Err("session restore requires its bound isolated replica authority".into());
        }
        if identity.character_index != checkpoint.character.index
            || identity.character_name != checkpoint.character.name
        {
            return Err("replica session checkpoint identity mismatch".into());
        }
        let account = authority_account(&config, &identity.account_id)?;
        self.select_continuation_account(&identity.account_id, &account)?;
        let previous_tick = runtime_tick(self.app.world());
        // Existing restore enforces the whole frozen baseline, roster, profile,
        // and private capability before installing any personal runtime state.
        let result = self.restore_active_character_checkpoint(checkpoint);
        if result.is_err() {
            self.abandon_failed_handoff(config, previous_tick);
        }
        result
    }

    pub fn prepare_character_handoff(&mut self) -> Result<CharacterHandoffCheckpoint, String> {
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        if config.has_isolated_replica_bootstrap_authority() {
            return Err("replica cannot prepare a source handoff".into());
        }
        let identity = self
            .active_identity()
            .ok_or("handoff preparation requires an active identity")?;
        let world = self.app.world();
        let source_tick = runtime_tick(world);
        let save = snapshot_active_character_save(world)
            .ok_or("handoff runtime checkpoint is unavailable")?;
        let expected_revision = save.revision;
        let account_id = identity.account_id;
        let committed_revision =
            persist_character_save_inner_with(world, &account_id, save, None, |_, result| {
                match result {
                    PersistCharacterSaveResult::Full(revision) => Ok(revision),
                    PersistCharacterSaveResult::StaleMailStatusOnly => {
                        Err("handoff preparation requires a complete source CAS".into())
                    }
                }
            })?;
        if !world
            .resource::<SessionResource>()
            .advance_active_save_revision(expected_revision, committed_revision)
        {
            return Err("handoff source revision changed during publication".into());
        }
        // Settlement/post-actions run after the transaction callback. Read the
        // acknowledged authority row only after that whole publication succeeds.
        let account = authority_account(&config, &account_id)?;
        let durable = account
            .saves
            .get(&identity.character_index)
            .cloned()
            .ok_or("handoff committed checkpoint is unavailable")?;
        if durable.revision != committed_revision
            || durable.character.index != identity.character_index
            || durable.character.name != identity.character_name
            || !account
                .characters
                .iter()
                .any(|character| character == &durable.character)
        {
            return Err("handoff authority advanced after the source publication".into());
        }
        let mut runtime = durable.clone();
        runtime.skill_states_json = durable
            .skill_states_json
            .iter()
            .map(|encoded| {
                let mut skill: serde_json::Value =
                    serde_json::from_str(encoded).map_err(|e| e.to_string())?;
                skill
                    .as_object_mut()
                    .ok_or("handoff skill is invalid")?
                    .remove(SKILL_CLOCK_METADATA_KEY)
                    .ok_or("handoff durable skill clock is missing")?;
                serde_json::to_string(&skill).map_err(|e| e.to_string())
            })
            .collect::<Result<Vec<_>, String>>()?;
        let checkpoint = CharacterHandoffCheckpoint {
            account_id,
            durable,
            runtime,
            source_tick,
        };
        checkpoint.validate_source_pair()?;
        Ok(checkpoint)
    }

    pub fn resume_character_handoff(
        &mut self,
        checkpoint: &CharacterHandoffCheckpoint,
    ) -> Result<CharacterHandoffCommitment, String> {
        checkpoint.validate_source_pair()?;
        let account_id = authenticated_select(self.app.world(), &checkpoint.runtime)?;
        if account_id != checkpoint.account_id {
            return Err("handoff account differs from its authenticated source admission".into());
        }
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        if config.has_isolated_replica_bootstrap_authority() {
            return Err("live handoff cannot use replica authority".into());
        }
        let account = authority_account(&config, &account_id)?;
        if account.active_ban(unix_now_ms()).is_some() {
            return Err("handoff source account is banned".into());
        }
        if !account
            .characters
            .iter()
            .any(|character| character == &checkpoint.durable.character)
            || !account
                .saves
                .get(&checkpoint.durable.character.index)
                .map(|save| same_record(save, &checkpoint.durable))
                .transpose()?
                .unwrap_or(false)
        {
            return Err("handoff requires its complete saved authority baseline".into());
        }
        let previous_tick = runtime_tick(self.app.world());
        let result = (|| {
            self.install_handoff_runtime(checkpoint)?;
            let confirmed = authority_account(&config, &account_id)?;
            if confirmed.active_ban(unix_now_ms()).is_some()
                || !confirmed
                    .characters
                    .iter()
                    .any(|character| character == &checkpoint.durable.character)
                || !confirmed
                    .saves
                    .get(&checkpoint.durable.character.index)
                    .map(|save| same_record(save, &checkpoint.durable))
                    .transpose()?
                    .unwrap_or(false)
            {
                return Err("handoff source advanced during target installation".into());
            }
            self.character_handoff_commitment(checkpoint)
        })();
        if result.is_err() {
            self.abandon_failed_handoff(config, previous_tick);
        }
        result
    }

    fn abandon_failed_handoff(&mut self, config: SimulationConfig, previous_tick: u64) {
        // A failed prepared target has no admission to retain. Recreate an
        // unlogged personal runtime without Login/Logout/save side effects;
        // even a caller retaining this instance must authenticate again.
        *self = SimulationSession::new(config);
        set_runtime_tick(self.app.world_mut(), previous_tick);
    }

    fn install_handoff_runtime(
        &mut self,
        checkpoint: &CharacterHandoffCheckpoint,
    ) -> Result<(), String> {
        let previous_tick = runtime_tick(self.app.world());
        set_runtime_tick(self.app.world_mut(), checkpoint.source_tick);
        if let Err(error) = self.install_active_character_checkpoint(&checkpoint.runtime) {
            set_runtime_tick(self.app.world_mut(), previous_tick);
            return Err(error);
        }
        let clock = self.app.world().resource::<LastRestoreClock>().0.clone();
        self.app.world_mut().insert_resource(HandoffInstallWitness {
            checkpoint: serde_json::to_value(checkpoint).map_err(|e| e.to_string())?,
            clock,
        });
        let mut session = self.app.world_mut().resource_mut::<SessionResource>();
        if let Some(character) = session
            .characters
            .iter_mut()
            .find(|character| character.index == checkpoint.runtime.character.index)
        {
            *character = checkpoint.runtime.character.clone();
        }
        Ok(())
    }

    pub fn character_handoff_commitment(
        &self,
        checkpoint: &CharacterHandoffCheckpoint,
    ) -> Result<CharacterHandoffCommitment, String> {
        let world = self.app.world();
        let witness = world
            .get_resource::<HandoffInstallWitness>()
            .ok_or("handoff install witness is unavailable")?;
        if witness.checkpoint != serde_json::to_value(checkpoint).map_err(|e| e.to_string())?
            || runtime_tick(world) != checkpoint.source_tick
        {
            return Err(
                "handoff install witness belongs to another checkpoint or clock domain".into(),
            );
        }
        let observed = super::super::refine_oven::capture_clock();
        let mut runtime = snapshot_active_character_save_at(world, &observed)
            .ok_or("handoff target readback is unavailable")?;
        runtime.skill_states_json = encode_state_vec(&world.resource::<SkillResource>().skills);
        let commitment = CharacterHandoffCommitment {
            runtime,
            restored_clock: CharacterHandoffClockWitness {
                epoch: witness.clock.epoch.clone(),
                now_ms: witness.clock.now_ms,
            },
            observed_clock: CharacterHandoffClockWitness {
                epoch: observed.epoch,
                now_ms: observed.now_ms,
            },
        };
        checkpoint.validate_commitment(&commitment)?;
        Ok(commitment)
    }

    /// Only a digest/sequence-verified internal journal calls this distinct seam.
    /// The bound isolated image may advance; a live source can never use it.
    pub fn apply_character_handoff_replica(
        &mut self,
        checkpoint: &CharacterHandoffCheckpoint,
    ) -> Result<(), String> {
        checkpoint.validate_source_pair()?;
        let account_id = authenticated_select(self.app.world(), &checkpoint.runtime)?;
        if account_id != checkpoint.account_id {
            return Err("handoff account differs from its authenticated source admission".into());
        }
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        if !config.has_isolated_replica_bootstrap_authority() {
            return Err("handoff replay requires its bound isolated authority".into());
        }
        let observed = {
            let store = config
                .account_store
                .lock()
                .map_err(|_| "handoff replica account image lock poisoned")?;
            let account = store
                .accounts
                .get(&account_id)
                .ok_or("handoff replica account is missing")?;
            let previous = account.saves.get(&checkpoint.durable.character.index);
            if let Some(previous) = previous {
                if previous.revision > checkpoint.durable.revision
                    || (previous.revision == checkpoint.durable.revision
                        && !same_record(previous, &checkpoint.durable)?)
                {
                    return Err(
                        "handoff replay would roll back or rewrite a complete checkpoint".into(),
                    );
                }
                if let Some(journal) = &previous.npc_purchase_journal {
                    if !checkpoint
                        .durable
                        .npc_purchase_journal
                        .as_ref()
                        .is_some_and(|next| {
                            next.actor == journal.actor
                                && next.entries.starts_with(&journal.entries)
                        })
                    {
                        return Err(
                            "handoff replay would replace an NPC actor or terminal history".into(),
                        );
                    }
                }
                if previous.default_npc_events.committed_sequence
                    > checkpoint.durable.default_npc_events.committed_sequence
                    || previous.default_npc_events.next_sequence
                        > checkpoint.durable.default_npc_events.next_sequence
                    || previous.guild_experience_journal.next_sequence
                        > checkpoint.durable.guild_experience_journal.next_sequence
                    || !previous.guild_experience_journal.event_payloads.iter().all(
                        |(key, value)| {
                            checkpoint
                                .durable
                                .guild_experience_journal
                                .event_payloads
                                .get(key)
                                == Some(value)
                        },
                    )
                    || !previous
                        .guild_experience_journal
                        .applied_kill_receipts
                        .iter()
                        .all(|(key, value)| {
                            checkpoint
                                .durable
                                .guild_experience_journal
                                .applied_kill_receipts
                                .get(key)
                                == Some(value)
                        })
                    || !previous.guild_experience_journal.kill_outcomes.iter().all(
                        |(key, value)| {
                            checkpoint
                                .durable
                                .guild_experience_journal
                                .kill_outcomes
                                .get(key)
                                == Some(value)
                        },
                    )
                    || !previous.player_kill_receipts.iter().all(|(key, value)| {
                        checkpoint.durable.player_kill_receipts.get(key) == Some(value)
                    })
                {
                    return Err("handoff replay would erase committed source history".into());
                }
            }
            serde_json::to_value(account).map_err(|e| e.to_string())?
        };
        let previous_tick = runtime_tick(self.app.world());
        let result = (|| {
            self.install_handoff_runtime(checkpoint)?;
            // Verified journal transport still cannot authorize a lossy local
            // representation. Prove the full installed image before advancing
            // the isolated account baseline.
            self.character_handoff_commitment(checkpoint)?;
            let mut store = config
                .account_store
                .lock()
                .map_err(|_| "handoff replica account image lock poisoned")?;
            let account = store
                .accounts
                .get_mut(&account_id)
                .ok_or("handoff replica account retired")?;
            if serde_json::to_value(&*account).map_err(|e| e.to_string())? != observed {
                return Err("handoff replica account advanced during live installation".into());
            }
            let character = account
                .characters
                .iter_mut()
                .find(|character| {
                    character.index == checkpoint.durable.character.index
                        && character.name == checkpoint.durable.character.name
                })
                .ok_or("handoff replica character retired")?;
            *character = checkpoint.durable.character.clone();
            account.saves.insert(
                checkpoint.durable.character.index,
                checkpoint.durable.clone(),
            );
            Ok(())
        })();
        if result.is_err() {
            self.abandon_failed_handoff(config, previous_tick);
        }
        result
    }
}

#[cfg(test)]
#[path = "character_handoff_tests.rs"]
mod tests;
