use std::any::Any;
use std::cell::Cell;

use mir2_protocol::{ClientPacket, MirDirection, Point, ServerPacket};

use super::commit_active_character_save_transaction;
use crate::config::zone_save_fence::{ZoneSaveFence, REJECTED};
use crate::config::{AccountStoreDatabaseMode, CharacterSaveRecord};
use crate::runtime::resources::RuntimeConfigResource;
use crate::runtime::session::{ActiveSessionIdentity, SimulationSession};
use crate::world_runtime::{InProcessWorldRuntime, WorldCommand, WorldRuntime};
use crate::{SimulationConfig, WorldSnapshot};

fn fence(owner: &str, token: u64) -> ZoneSaveFence {
    ZoneSaveFence::new("map:zone-save-fence-test".into(), owner.into(), token).unwrap()
}

// Binding only validates configuration; none of these tests connect to a DB.
fn postgres_source_config() -> SimulationConfig {
    let mut config = SimulationConfig::default();
    config.account_store_database_url =
        Some("postgresql://zone-save-fence-test.invalid/test".into());
    config.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;
    config
}

fn active_session() -> (SimulationSession, SimulationConfig) {
    let config = SimulationConfig::default();
    let mut session = SimulationSession::new(config.clone());
    session
        .try_handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        })
        .unwrap();
    let packets = session
        .try_handle_packet(ClientPacket::StartGame { character_index: 0 })
        .unwrap();
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    (session, config)
}

fn make_postgres_source(session: &mut SimulationSession) {
    let mut resource = session
        .app
        .world_mut()
        .resource_mut::<RuntimeConfigResource>();
    resource.config.account_store_database_url =
        Some("postgresql://zone-save-fence-test.invalid/test".into());
    resource.config.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;
}

#[test]
fn world_runtime_zone_save_fence_rejects_memory_file_and_mirror_configs() {
    let mut memory = InProcessWorldRuntime::new(SimulationConfig::default());
    assert!(memory.supports_zone_save_fence());
    assert_eq!(
        memory.bind_zone_save_fence(fence("owner-a", 1)),
        Err(REJECTED.into())
    );
    assert_eq!(memory.bound_zone_save_fence(), None);

    let mut mirror_config = postgres_source_config();
    mirror_config.account_store_database_mode = AccountStoreDatabaseMode::Mirror;
    let mut mirror = InProcessWorldRuntime::new(mirror_config);
    assert_eq!(
        mirror.bind_zone_save_fence(fence("owner-a", 1)),
        Err(REJECTED.into())
    );
    assert_eq!(mirror.bound_zone_save_fence(), None);

    let mut file_config = postgres_source_config();
    file_config.account_store_path = Some(std::path::PathBuf::from("unused-zone-fence-test.json"));
    let mut file = InProcessWorldRuntime::new(file_config);
    assert_eq!(
        file.bind_zone_save_fence(fence("owner-a", 1)),
        Err(REJECTED.into())
    );
    assert_eq!(file.bound_zone_save_fence(), None);
}

#[test]
fn world_runtime_zone_save_fence_is_bound_to_each_runtime_config() {
    let source = postgres_source_config();
    let mut first = InProcessWorldRuntime::new(source.clone());
    let mut second = InProcessWorldRuntime::new(source.clone());
    let first_fence = fence("owner-a", 17);
    let second_fence = fence("owner-b", 18);
    first.bind_zone_save_fence(first_fence.clone()).unwrap();
    second.bind_zone_save_fence(second_fence.clone()).unwrap();
    assert_eq!(first.bound_zone_save_fence(), Some(first_fence));
    assert_eq!(second.bound_zone_save_fence(), Some(second_fence.clone()));
    let replacement = fence("owner-a", 19);
    first.bind_zone_save_fence(replacement.clone()).unwrap();
    assert_eq!(first.bound_zone_save_fence(), Some(replacement));
    assert_eq!(second.bound_zone_save_fence(), Some(second_fence));
    assert_eq!(source.zone_save_fence(), None);
}

#[test]
fn zone_save_fence_checkpoint_restore_cannot_replace_or_remove_trusted_binding() {
    let (mut session, _) = active_session();
    let checkpoint = session.active_character_checkpoint().unwrap();
    make_postgres_source(&mut session);
    let original = fence("owner-a", 17);
    session.bind_zone_save_fence(original.clone()).unwrap();

    // Even a decoded checkpoint with extra forged authority data carries only
    // CharacterSaveRecord fields into the restore path.
    let mut encoded = serde_json::to_value(&checkpoint).unwrap();
    encoded["zoneSaveFence"] = serde_json::json!({
        "zoneId": "foreign-zone", "ownerId": "foreign-owner", "fencingToken": 99
    });
    encoded["zone_save_fence"] = encoded["zoneSaveFence"].clone();
    let forged: CharacterSaveRecord = serde_json::from_value(encoded).unwrap();
    session
        .restore_active_character_checkpoint(&forged)
        .unwrap();
    assert_eq!(session.bound_zone_save_fence(), Some(original.clone()));
    let plain = serde_json::to_value(session.active_character_checkpoint().unwrap()).unwrap();
    assert!(plain.get("zoneSaveFence").is_none());
    assert!(plain.get("zone_save_fence").is_none());

    let mut wrong_identity = checkpoint;
    wrong_identity.character.index += 1;
    assert!(session
        .restore_active_character_checkpoint(&wrong_identity)
        .is_err());
    assert_eq!(session.bound_zone_save_fence(), Some(original.clone()));

    let foreign_source = postgres_source_config()
        .with_zone_save_fence(fence("foreign-owner", 99))
        .unwrap();
    session.rebind_account_store(&foreign_source);
    assert_eq!(session.bound_zone_save_fence(), Some(original));
}

#[test]
fn zone_save_fence_rejected_rebind_retains_previous_authority() {
    let mut session = SimulationSession::new(postgres_source_config());
    let original = fence("owner-a", 17);
    session.bind_zone_save_fence(original.clone()).unwrap();
    session
        .app
        .world_mut()
        .resource_mut::<RuntimeConfigResource>()
        .config
        .account_store_database_mode = AccountStoreDatabaseMode::Mirror;
    assert_eq!(
        session.bind_zone_save_fence(fence("owner-b", 18)),
        Err(REJECTED.into())
    );
    assert_eq!(session.bound_zone_save_fence(), Some(original));
}

#[test]
fn zone_save_fence_full_logout_frozen_and_prepared_saves_use_bound_runtime_config() {
    let (mut session, original_config) = active_session();
    make_postgres_source(&mut session);
    let original = fence("owner-a", 17);
    session.bind_zone_save_fence(original.clone()).unwrap();
    // Rebinding the public backend fields cannot silently downgrade the fence.
    // This deliberately invalid config fails before any network or file IO.
    session
        .app
        .world_mut()
        .resource_mut::<RuntimeConfigResource>()
        .config
        .account_store_database_mode = AccountStoreDatabaseMode::Mirror;
    let final_position = Point { x: 330, y: 266 };
    session.force_authoritative_player_transform(final_position.clone(), MirDirection::Down);
    let identity = session.active_identity();
    let checkpoint = session.active_character_teardown_checkpoint().unwrap();
    let store_before =
        serde_json::to_value(&*original_config.account_store.lock().unwrap()).unwrap();

    assert_eq!(session.save_active_character(), Err(REJECTED.into()));
    assert_eq!(
        session.save_active_character_for_logout(),
        Err(REJECTED.into())
    );
    assert_eq!(
        session.try_handle_packet(ClientPacket::LogOut),
        Err(REJECTED.into())
    );
    assert_eq!(
        session.try_handle_packet(ClientPacket::Disconnect),
        Err(REJECTED.into())
    );
    assert_eq!(
        session.save_frozen_character_checkpoint_for_logout(&checkpoint),
        Err(REJECTED.into())
    );
    assert_eq!(session.prepare_character_handoff().unwrap_err(), REJECTED);
    let callback_ran = Cell::new(false);
    assert_eq!(
        commit_active_character_save_transaction(session.app.world(), |_| {
            callback_ran.set(true);
            Ok(())
        }),
        Err(REJECTED.into())
    );
    assert!(!callback_ran.get());
    assert_eq!(session.bound_zone_save_fence(), Some(original));
    assert_eq!(session.active_identity(), identity);
    let retained = session.active_character_checkpoint().unwrap();
    assert_eq!(retained.revision, checkpoint.revision);
    assert_eq!(retained.position, final_position);
    assert_eq!(retained.direction, MirDirection::Down);
    assert_eq!(
        serde_json::to_value(&*original_config.account_store.lock().unwrap()).unwrap(),
        store_before
    );
}

#[test]
fn zone_save_fence_replica_shadow_cannot_inherit_source_authority() {
    let source = postgres_source_config()
        .with_zone_save_fence(fence("owner-a", 17))
        .unwrap();
    let shadow = source.fork_for_replica_apply().unwrap();
    let mut runtime = InProcessWorldRuntime::new(shadow);
    assert_eq!(runtime.bound_zone_save_fence(), None);
    assert_eq!(
        runtime.bind_zone_save_fence(fence("owner-a", 17)),
        Err(REJECTED.into())
    );
    assert_eq!(runtime.bound_zone_save_fence(), None);
}

struct LegacyRuntime(SimulationSession);

impl WorldRuntime for LegacyRuntime {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn on_connect(&self) -> Vec<ServerPacket> {
        Vec::new()
    }
    fn execute(&mut self, _: WorldCommand) -> Result<Vec<ServerPacket>, String> {
        Ok(Vec::new())
    }
    fn world_snapshot(&self) -> WorldSnapshot {
        self.0.world_snapshot()
    }
    fn active_identity(&self) -> Option<ActiveSessionIdentity> {
        self.0.active_identity()
    }
    fn save_active_character(&mut self) -> Result<(), String> {
        self.0.save_active_character()
    }
    fn refresh_active_external_mail(&mut self) -> bool {
        false
    }
}

#[test]
fn zone_save_fence_legacy_runtime_defaults_remain_unsupported() {
    let mut runtime = LegacyRuntime(SimulationSession::new(SimulationConfig::default()));
    assert!(!runtime.supports_zone_save_fence());
    assert_eq!(runtime.bound_zone_save_fence(), None);
    assert_eq!(
        runtime.bind_zone_save_fence(fence("owner-a", 17)),
        Err("world runtime does not support zone save fencing".into())
    );
}
