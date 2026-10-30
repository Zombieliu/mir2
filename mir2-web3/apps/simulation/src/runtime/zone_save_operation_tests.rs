//! Pure staging/configuration tests. These never claim a PostgreSQL commit.
use std::any::Any;

use super::{same_zone_save_operation, stage_zone_save_operation};
use crate::config::zone_save_fence::ZoneSaveFence;
use crate::config::zone_save_operation::{ZoneSaveOperation, ZoneSaveOperationBinding, REJECTED};
use crate::config::{
    AccountStoreDatabaseMode, CharacterSaveRecord, Stage5MailMessage, Stage5SystemsState,
};
use crate::runtime::resources::RuntimeConfigResource;
use crate::runtime::session::{ActiveSessionIdentity, SimulationSession};
use crate::world_runtime::{WorldCommand, WorldRuntime};
use crate::{AccountStore, SimulationConfig, WorldSnapshot};
use mir2_protocol::{ClientPacket, MirDirection, Point, ServerPacket};

fn binding(owner: &str, operation: &str) -> ZoneSaveOperationBinding {
    ZoneSaveOperationBinding::new(
        operation.into(),
        "rpc-zone-save-runtime-test".into(),
        ZoneSaveFence::new("map:zone-save-runtime-test".into(), owner.into(), 17).unwrap(),
    )
    .unwrap()
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

fn bind_without_database_io(session: &mut SimulationSession, binding: &ZoneSaveOperationBinding) {
    let mut resource = session
        .app
        .world_mut()
        .resource_mut::<RuntimeConfigResource>();
    resource.config.account_store_database_url =
        Some("postgresql://zone-save-operation-test.invalid/test".into());
    resource.config.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;
    drop(resource);
    session
        .bind_zone_save_fence(binding.fence().clone())
        .unwrap();
}

fn cloned_store(config: &SimulationConfig) -> AccountStore {
    config.account_store.lock().unwrap().clone()
}

fn store_image(config: &SimulationConfig) -> serde_json::Value {
    serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap()
}

fn stage(
    session: &SimulationSession,
    config: &SimulationConfig,
    binding: ZoneSaveOperationBinding,
    frozen: &CharacterSaveRecord,
) -> ZoneSaveOperation {
    stage_zone_save_operation(
        session.app.world(),
        binding,
        frozen,
        cloned_store(config),
        Some(123456),
    )
    .unwrap()
}

#[test]
fn zone_save_operation_stage_preserves_frozen_bytes_and_never_publishes_live_cas() {
    let (mut session, config) = active_session();
    let binding = binding("owner-a", "operation-stage");
    bind_without_database_io(&mut session, &binding);
    session.force_authoritative_player_transform(Point { x: 330, y: 266 }, MirDirection::Down);
    let frozen = session.active_character_teardown_checkpoint().unwrap();
    let frozen_image = serde_json::to_value(&frozen).unwrap();
    let before = store_image(&config);
    let operation = stage(&session, &config, binding.clone(), &frozen);
    assert_eq!(operation.binding(), &binding);
    assert_eq!(operation.account_id(), "demo");
    assert_eq!(operation.last_access_binary_datetime(), Some(123456));
    assert_eq!(
        serde_json::to_value(operation.frozen_checkpoint()).unwrap(),
        frozen_image
    );
    assert_eq!(
        operation.expected_saved_checkpoint().revision,
        frozen.revision + 1
    );
    assert_eq!(
        operation.expected_saved_checkpoint().position,
        frozen.position
    );
    assert_eq!(
        operation.expected_saved_checkpoint().skill_states_json,
        frozen.skill_states_json
    );
    assert_eq!(store_image(&config), before);
    assert_eq!(
        session
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .zone_save_operation(),
        None
    );
}

#[test]
fn zone_save_operation_stage_captures_durable_mail_and_protected_journals_in_exact_expected_image()
{
    let (session, config) = active_session();
    let frozen = session.active_character_teardown_checkpoint().unwrap();
    let journal = crate::npc_purchase_journal::NpcPurchaseJournal::new(
        "demo",
        frozen.character.index,
        &frozen.character.name,
        [7; 32],
    )
    .unwrap();
    let ledger = mir2_production::ProductionLedger::default()
        .checkpoint_json()
        .unwrap();
    let production = serde_json::from_value(serde_json::json!({
        "owner": { "account_id": "demo", "character_id": "07".repeat(32) },
        "ledger_json": ledger,
    }))
    .unwrap();
    let mail = Stage5MailMessage {
        id: 7001,
        delivery_nonce: "00112233445566778899aabbccddeeff".into(),
        from: "Procurement".into(),
        to: frozen.character.name.clone(),
        subject: "Delivered externally".into(),
        body: "retained".into(),
        gold: 50,
        items: Vec::new(),
        item_states_json: Vec::new(),
        opened: false,
        locked: false,
        claimed: false,
        deleted: false,
    };
    {
        let mut store = config.account_store.lock().unwrap();
        let durable = store
            .accounts
            .get_mut("demo")
            .unwrap()
            .saves
            .get_mut(&0)
            .unwrap();
        let mut systems: Stage5SystemsState =
            serde_json::from_str(durable.stage5_systems_json.as_deref().unwrap()).unwrap();
        systems.mail.push(mail.clone());
        durable.stage5_systems_json = Some(serde_json::to_string(&systems).unwrap());
        durable.npc_purchase_journal = Some(journal.clone());
        durable.personal_production = Some(production);
    }
    let before = store_image(&config);
    let operation = stage(
        &session,
        &config,
        binding("owner-a", "operation-merge"),
        &frozen,
    );
    let expected = operation.expected_saved_checkpoint();
    let systems: Stage5SystemsState =
        serde_json::from_str(expected.stage5_systems_json.as_deref().unwrap()).unwrap();
    assert!(systems
        .mail
        .iter()
        .any(|entry| entry.id == mail.id && entry.gold == mail.gold));
    assert_eq!(expected.npc_purchase_journal, Some(journal));
    assert!(expected.personal_production.is_some());
    assert!(operation.frozen_checkpoint().npc_purchase_journal.is_none());
    assert!(operation.frozen_checkpoint().personal_production.is_none());
    assert_eq!(store_image(&config), before);
}

#[test]
fn zone_save_operation_stage_rejects_revision_or_character_change_without_mutation() {
    let (session, config) = active_session();
    let frozen = session.active_character_teardown_checkpoint().unwrap();
    let before = store_image(&config);
    for change in [0, 1] {
        let mut invalid = frozen.clone();
        if change == 0 {
            invalid.revision += 1;
        } else {
            invalid.character.name.push('x');
        }
        assert!(stage_zone_save_operation(
            session.app.world(),
            binding("owner-a", "operation-invalid"),
            &invalid,
            cloned_store(&config),
            Some(123456)
        )
        .is_err());
    }
    let mut stale_store = cloned_store(&config);
    stale_store
        .accounts
        .get_mut("demo")
        .unwrap()
        .saves
        .get_mut(&0)
        .unwrap()
        .revision += 1;
    assert!(stage_zone_save_operation(
        session.app.world(),
        binding("owner-a", "operation-stale"),
        &frozen,
        stale_store,
        Some(123456)
    )
    .is_err());
    assert_eq!(store_image(&config), before);
}

#[test]
fn zone_save_operation_prepare_rejects_legacy_skill_clock_before_database_io() {
    let (mut session, config) = active_session();
    let binding = binding("owner-a", "operation-legacy-clock");
    let rollback = session.active_character_checkpoint().unwrap();
    assert!(!rollback.skill_states_json.is_empty());
    bind_without_database_io(&mut session, &binding);
    let before = store_image(&config);
    assert_eq!(
        session
            .prepare_zone_save_operation(binding, &rollback)
            .unwrap_err(),
        REJECTED
    );
    assert_eq!(store_image(&config), before);
    assert!(session
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .zone_save_operation()
        .is_none());
}

#[test]
fn zone_save_operation_runtime_requires_its_existing_exact_fence_for_all_seams() {
    let (mut session, config) = active_session();
    let frozen = session.active_character_teardown_checkpoint().unwrap();
    let foreign = binding("foreign-owner", "operation-foreign");
    let operation = stage(&session, &config, foreign.clone(), &frozen);
    let before = store_image(&config);
    assert_eq!(
        session
            .prepare_zone_save_operation(foreign.clone(), &frozen)
            .unwrap_err(),
        REJECTED
    );
    assert_eq!(
        session.persist_zone_save_operation(&operation).unwrap_err(),
        REJECTED
    );
    assert_eq!(
        session.query_zone_save_operation(&foreign).unwrap_err(),
        REJECTED
    );
    assert_eq!(
        session.release_zone_save_operation(&foreign).unwrap_err(),
        REJECTED
    );
    let original = binding("owner-a", "operation-bound");
    bind_without_database_io(&mut session, &original);
    assert_eq!(
        session
            .prepare_zone_save_operation(foreign.clone(), &frozen)
            .unwrap_err(),
        REJECTED
    );
    assert_eq!(
        session.persist_zone_save_operation(&operation).unwrap_err(),
        REJECTED
    );
    assert_eq!(
        session.query_zone_save_operation(&foreign).unwrap_err(),
        REJECTED
    );
    assert_eq!(
        session.release_zone_save_operation(&foreign).unwrap_err(),
        REJECTED
    );
    assert_eq!(
        session.bound_zone_save_fence(),
        Some(original.fence().clone())
    );
    assert_eq!(store_image(&config), before);
}

#[test]
fn zone_save_operation_payload_comparison_includes_fixed_date_and_complete_expected_image() {
    let (session, config) = active_session();
    let frozen = session.active_character_teardown_checkpoint().unwrap();
    let operation = stage(
        &session,
        &config,
        binding("owner-a", "operation-equality"),
        &frozen,
    );
    assert!(same_zone_save_operation(&operation, &operation.clone()).unwrap());
    let changed_date = ZoneSaveOperation::new(
        operation.binding().clone(),
        "demo".into(),
        frozen.clone(),
        operation.expected_saved_checkpoint().clone(),
        Some(123457),
    )
    .unwrap();
    assert!(!same_zone_save_operation(&operation, &changed_date).unwrap());
    let mut expected = operation.expected_saved_checkpoint().clone();
    expected.gold += 1;
    let changed_image = ZoneSaveOperation::new(
        operation.binding().clone(),
        "demo".into(),
        frozen,
        expected,
        Some(123456),
    )
    .unwrap();
    assert!(!same_zone_save_operation(&operation, &changed_image).unwrap());
}

#[test]
fn zone_save_operation_release_refuses_shared_unknown_freeze_before_database_io() {
    let (mut session, config) = active_session();
    let binding = binding("owner-a", "operation-frozen-release");
    bind_without_database_io(&mut session, &binding);
    let before_store = store_image(&config);
    let before_checkpoint =
        serde_json::to_value(session.active_character_checkpoint().unwrap()).unwrap();
    let before_identity = session.active_identity();
    config.freeze_zone_save_operation_writes_for_test();

    // This source URL is intentionally unreachable. A frozen release must stop
    // locally, even when a previous ledger transition could already be Released.
    let error = session.release_zone_save_operation(&binding).unwrap_err();
    assert!(error.contains("writes are frozen"));
    assert!(config
        .ensure_account_store_writable()
        .unwrap_err()
        .contains("writes are frozen"));
    assert_eq!(
        session.bound_zone_save_fence(),
        Some(binding.fence().clone())
    );
    assert_eq!(session.active_identity(), before_identity);
    assert_eq!(
        serde_json::to_value(session.active_character_checkpoint().unwrap()).unwrap(),
        before_checkpoint
    );
    assert_eq!(store_image(&config), before_store);
    assert!(session
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .zone_save_operation()
        .is_none());
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
fn zone_save_operation_legacy_runtime_fails_closed_for_every_lifecycle_operation() {
    let (session, config) = active_session();
    let frozen = session.active_character_teardown_checkpoint().unwrap();
    let binding = binding("owner-a", "operation-unsupported");
    let operation = stage(&session, &config, binding.clone(), &frozen);
    let mut runtime = LegacyRuntime(session);
    let unsupported = "world runtime does not support durable zone save operations";
    assert_eq!(
        runtime
            .prepare_zone_save_operation(binding.clone(), &frozen)
            .unwrap_err(),
        unsupported
    );
    assert_eq!(
        runtime.persist_zone_save_operation(&operation).unwrap_err(),
        unsupported
    );
    assert_eq!(
        runtime.query_zone_save_operation(&binding).unwrap_err(),
        unsupported
    );
    assert_eq!(
        runtime.release_zone_save_operation(&binding).unwrap_err(),
        unsupported
    );
}
