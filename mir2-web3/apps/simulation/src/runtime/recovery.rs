//! Trusted journal selection is a read, not a new player login.
use bevy_ecs::prelude::{Resource, World};

use super::resources::{RuntimeConfigResource, SessionResource};
use super::save::{frozen_recovery_checkpoints_match, validate_character_save_record};
use super::SimulationSession;
use crate::config::{AccountSourceRefreshOutcome, CharacterRecord, CharacterSaveRecord};

#[derive(Resource)]
pub(super) struct TrustedRecoveryAccount(pub(super) String);

#[derive(Resource)]
struct TrustedRecoveryCharacter {
    account_id: String,
    character: CharacterRecord,
    revision: u64,
}

pub(super) fn clear_binding(world: &mut World) {
    world.remove_resource::<TrustedRecoveryAccount>();
    world.remove_resource::<TrustedRecoveryCharacter>();
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryCharacterLoadError {
    /// Authority/eligibility can become available again. Retain the journal.
    Deferred(String),
    /// The trusted character identity or durable state cannot be reconciled.
    Conflict(String),
}

impl SimulationSession {
    /// Exact frozen comparison, with only the known legacy skill-clock wrapper
    /// normalized when the authenticated journal predates captured clocks.
    pub fn recovery_checkpoints_equivalent(
        saved: &CharacterSaveRecord,
        journal: &CharacterSaveRecord,
    ) -> Result<bool, String> {
        frozen_recovery_checkpoints_match(saved, journal)
    }
    /// Fresh authority lookup for the server's authenticated journal path.
    /// Unavailable authority is retryable, not an identity conflict.
    pub fn bind_account_for_recovery(
        &mut self,
        account_id: &str,
    ) -> Result<(), RecoveryCharacterLoadError> {
        use RecoveryCharacterLoadError::{Conflict, Deferred};
        clear_binding(self.app.world_mut());
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        if config
            .refresh_account_store_account(account_id)
            .map_err(Deferred)?
            == AccountSourceRefreshOutcome::Missing
        {
            return Err(Conflict("recovery account no longer exists".into()));
        }
        // In-memory authority has no external refresh outcome. Distinguish a
        // genuinely missing identity from an unavailable/poisoned store here.
        if !config
            .account_store
            .lock()
            .map_err(|_| Deferred("recovery account source is unavailable".into()))?
            .accounts
            .contains_key(account_id)
        {
            return Err(Conflict("recovery account no longer exists".into()));
        }
        self.select_account_for_recovery(account_id)
            .map_err(Deferred)
    }

    /// Only the server's validated journal path may call this after
    /// `select_account_for_recovery`. No client packet, command or RPC exposes
    /// it. A one-use account binding prevents ordinary Login from selecting it.
    /// Read the journal's frozen representation without migrations,
    /// new-life recovery, DefaultNPC hooks, offline credits or any DB write.
    pub fn select_character_for_recovery(
        &mut self,
        character_index: i32,
    ) -> Result<CharacterSaveRecord, RecoveryCharacterLoadError> {
        use RecoveryCharacterLoadError::{Conflict, Deferred};
        let account_id = self
            .app
            .world_mut()
            .remove_resource::<TrustedRecoveryAccount>()
            .ok_or_else(|| Conflict("trusted recovery selection is not bound".into()))?
            .0;
        let session = self.app.world().resource::<SessionResource>();
        if session.account_id.as_deref() != Some(account_id.as_str())
            || session.selected_character.is_some()
        {
            return Err(Conflict(
                "trusted recovery selection identity changed".into(),
            ));
        }
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        if config
            .refresh_account_store_account(&account_id)
            .map_err(Deferred)?
            == AccountSourceRefreshOutcome::Missing
        {
            return Err(Conflict("recovery account no longer exists".into()));
        }
        // This lookup may itself refresh authority. Clone the final roster
        // and save together only after all source refreshes have completed.
        if config.monthly_card_policy.required
            && !config
                .refresh_monthly_card_status(
                    &account_id,
                    crate::monthly_card::monthly_card_now_ms(),
                )
                .map_err(Deferred)?
                .can_enter_game
        {
            return Err(Deferred(
                "recovery account access is currently denied".into(),
            ));
        }
        let (characters, save) = {
            let store = config
                .account_store
                .lock()
                .map_err(|_| Deferred("recovery account source is unavailable".into()))?;
            let account = store
                .accounts
                .get(&account_id)
                .ok_or_else(|| Conflict("recovery account no longer exists".into()))?;
            if account.active_ban(super::save::unix_now_ms()).is_some() {
                return Err(Deferred(
                    "recovery account access is currently denied".into(),
                ));
            }
            let character = account
                .characters
                .iter()
                .find(|character| character.index == character_index)
                .ok_or_else(|| Conflict("recovery character no longer exists".into()))?;
            let save = account
                .saves
                .get(&character_index)
                .ok_or_else(|| Conflict("recovery character has no durable save".into()))?;
            if save.character.index != character.index
                || save.character.name != character.name
                || save.character.class != character.class
                || save.character.gender != character.gender
            {
                return Err(Conflict("recovery character/save identity mismatch".into()));
            }
            validate_character_save_record(save).map_err(Conflict)?;
            (account.characters.clone(), save.clone())
        };
        {
            let mut session = self.app.world_mut().resource_mut::<SessionResource>();
            session.characters = characters;
            session.selected_character = Some(save.character.clone());
            session.bind_active_save_revision(save.revision);
        }
        let checkpoint = save.clone();
        self.app
            .world_mut()
            .insert_resource(TrustedRecoveryCharacter {
                account_id,
                character: save.character,
                revision: save.revision,
            });
        Ok(checkpoint)
    }

    /// One-use server capability. No packet or WorldCommand/RPC exposes this.
    /// Restoring prepares authority resources; the save itself uses the frozen
    /// journal record through the ordinary identity/CAS/source transaction.
    pub fn persist_character_for_recovery(
        &mut self,
        checkpoint: &CharacterSaveRecord,
    ) -> Result<CharacterSaveRecord, String> {
        let bound = self
            .app
            .world_mut()
            .remove_resource::<TrustedRecoveryCharacter>()
            .ok_or("trusted recovery character is not bound")?;
        let session = self.app.world().resource::<SessionResource>();
        let same_character = |character: &CharacterRecord| {
            character.index == bound.character.index
                && character.name == bound.character.name
                && character.class == bound.character.class
                && character.gender == bound.character.gender
        };
        if session.account_id.as_deref() != Some(bound.account_id.as_str())
            || !session
                .selected_character
                .as_ref()
                .is_some_and(same_character)
            || !same_character(&checkpoint.character)
            || checkpoint.revision != bound.revision
            || session.active_save_revision() != Some(bound.revision)
        {
            return Err("trusted recovery checkpoint identity/revision mismatch".into());
        }
        validate_character_save_record(checkpoint)?;
        self.save_frozen_character_checkpoint_for_logout(checkpoint)?;
        let config = &self.app.world().resource::<RuntimeConfigResource>().config;
        let stored = config
            .account_store
            .lock()
            .map_err(|_| "recovery source unavailable after commit")?
            .accounts
            .get(&bound.account_id)
            .and_then(|account| account.saves.get(&bound.character.index))
            .cloned()
            .ok_or("recovery character disappeared after commit")?;
        Ok(stored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SimulationConfig;
    use mir2_protocol::{ClientPacket, ServerPacket};

    fn login(config: &SimulationConfig) -> SimulationSession {
        let mut session = SimulationSession::new(config.clone());
        assert!(session
            .handle_packet(ClientPacket::Login {
                account_id: "demo".into(),
                password: "demo".into(),
            })
            .iter()
            .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
        session
    }

    #[test]
    fn recovery_selection_requires_server_only_account_binding() {
        let config = SimulationConfig::default();
        let mut session = login(&config);
        assert!(matches!(
            session.select_character_for_recovery(0),
            Err(RecoveryCharacterLoadError::Conflict(_))
        ));
        assert!(session.active_identity().is_none());
    }

    #[test]
    fn ordinary_login_and_passkey_revoke_recovery_capabilities() {
        let config = SimulationConfig::default();
        let mut session = SimulationSession::new(config.clone());
        session.bind_account_for_recovery("demo").unwrap();
        session.handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        });
        assert!(session.select_character_for_recovery(0).is_err());
        session.bind_account_for_recovery("demo").unwrap();
        session.passkey_login("demo");
        assert!(session.select_character_for_recovery(0).is_err());
    }

    #[test]
    fn recovery_authority_failure_is_deferred_but_missing_account_is_conflict() {
        let config = SimulationConfig::default();
        let mut session = SimulationSession::new(config.clone());
        assert!(matches!(
            session.bind_account_for_recovery("missing-account"),
            Err(RecoveryCharacterLoadError::Conflict(_))
        ));
        let store = config.account_store.clone();
        assert!(std::thread::spawn(move || {
            let _guard = store.lock().unwrap();
            panic!("isolated poisoned authority fixture");
        })
        .join()
        .is_err());
        assert!(matches!(
            session.bind_account_for_recovery("demo"),
            Err(RecoveryCharacterLoadError::Deferred(_))
        ));
    }

    #[test]
    fn recovery_persist_requires_selected_identity_revision_and_is_one_use() {
        let config = SimulationConfig::default();
        let mut ordinary = login(&config);
        ordinary.handle_packet(ClientPacket::StartGame { character_index: 0 });
        let mut recovery = SimulationSession::new(config.clone());
        recovery.bind_account_for_recovery("demo").unwrap();
        let checkpoint = recovery.select_character_for_recovery(0).unwrap();
        let before =
            serde_json::to_vec(&config.account_store.lock().unwrap().accounts["demo"].saves[&0])
                .unwrap();
        let mut wrong = checkpoint.clone();
        wrong.character.class = mir2_protocol::MirClass::Taoist;
        assert!(recovery.persist_character_for_recovery(&wrong).is_err());
        assert!(recovery
            .persist_character_for_recovery(&checkpoint)
            .is_err());
        assert_eq!(
            serde_json::to_vec(&config.account_store.lock().unwrap().accounts["demo"].saves[&0])
                .unwrap(),
            before
        );
        recovery.bind_account_for_recovery("demo").unwrap();
        let checkpoint = recovery.select_character_for_recovery(0).unwrap();
        let committed = recovery
            .persist_character_for_recovery(&checkpoint)
            .unwrap();
        assert_eq!(committed.revision, checkpoint.revision + 1);
        assert!(recovery
            .persist_character_for_recovery(&checkpoint)
            .is_err());
    }

    #[test]
    fn frozen_teardown_cooldown_survives_cross_session_replay_without_resurrecting_expiry() {
        use crate::runtime::resources::{set_runtime_tick, SkillResource};
        for (deadline, expected_ms) in [(99, 0), (101, 1_000)] {
            let config = SimulationConfig::default();
            let mut original = login(&config);
            original.handle_packet(ClientPacket::StartGame { character_index: 0 });
            let skill = serde_json::from_value(serde_json::json!({
                "key":"FireBall", "name":"FireBall", "description":"Cross-session clock fixture",
                "level":2, "experience":321, "hotkey":4, "cooldown_ticks":2,
                "delay_ms":1800, "cooldown_ends_at":deadline, "cast_time_ms":99_000,
            }))
            .unwrap();
            original
                .app
                .world_mut()
                .resource_mut::<SkillResource>()
                .skills = vec![skill];
            set_runtime_tick(original.app.world_mut(), 100);
            let frozen = original.active_character_teardown_checkpoint().unwrap();
            let captured: serde_json::Value =
                serde_json::from_str(&frozen.skill_states_json[0]).unwrap();
            assert_eq!(
                captured["_runtimeCooldownClock"]["remainingMs"],
                expected_ms
            );
            let mut recovery = SimulationSession::new(config.clone());
            recovery.bind_account_for_recovery("demo").unwrap();
            recovery.select_character_for_recovery(0).unwrap();
            let committed = recovery.persist_character_for_recovery(&frozen).unwrap();
            assert_eq!(committed.skill_states_json, frozen.skill_states_json);
            assert!(
                SimulationSession::recovery_checkpoints_equivalent(&committed, &frozen).unwrap()
            );
            let mut cold = login(&config);
            cold.handle_packet(ClientPacket::StartGame { character_index: 0 });
            let restored = &cold.app.world().resource::<SkillResource>().skills[0];
            assert!(restored.cooldown_ends_at <= expected_ms / 1_000);
            assert_eq!(restored.experience, 321);
            assert_eq!(restored.hotkey, 4);
            let mut changed = committed.clone();
            let mut wrong_clock = captured.clone();
            wrong_clock["_runtimeCooldownClock"]["remainingMs"] =
                serde_json::json!(expected_ms + 1_000);
            changed.skill_states_json[0] = wrong_clock.to_string();
            assert!(
                !SimulationSession::recovery_checkpoints_equivalent(&changed, &frozen).unwrap()
            );
        }
    }

    #[test]
    fn recovery_selection_rejects_changed_account_and_consumes_binding() {
        let config = SimulationConfig::default();
        let mut session = SimulationSession::new(config);
        session.select_account_for_recovery("demo").unwrap();
        session
            .app
            .world_mut()
            .resource_mut::<SessionResource>()
            .account_id = Some("other".into());
        assert!(matches!(
            session.select_character_for_recovery(0),
            Err(RecoveryCharacterLoadError::Conflict(_))
        ));
        assert!(session
            .app
            .world()
            .get_resource::<TrustedRecoveryAccount>()
            .is_none());
        assert!(session.active_identity().is_none());
    }

    #[test]
    fn recovery_selection_does_not_write_login_events_or_revive_dead_character() {
        let config = SimulationConfig::default();
        let mut original = login(&config);
        original.handle_packet(ClientPacket::StartGame { character_index: 0 });
        let mut checkpoint = original.active_character_checkpoint().unwrap();
        checkpoint.hp = 0;
        original
            .restore_active_character_checkpoint(&checkpoint)
            .unwrap();
        original.save_active_character().unwrap();
        let before = config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
        let before_bytes = serde_json::to_vec(&before).unwrap();
        let mut recovery = SimulationSession::new(config.clone());
        recovery.select_account_for_recovery("demo").unwrap();
        let selected = recovery.select_character_for_recovery(0).unwrap();
        assert_eq!(selected.hp, 0);
        assert_eq!(selected.map_file_name, before.map_file_name);
        assert_eq!(selected.position, before.position);
        assert_eq!(selected.revision, before.revision);
        assert_eq!(selected.default_npc_events, before.default_npc_events);
        assert_eq!(
            serde_json::to_vec(&config.account_store.lock().unwrap().accounts["demo"].saves[&0])
                .unwrap(),
            before_bytes
        );
    }
}
