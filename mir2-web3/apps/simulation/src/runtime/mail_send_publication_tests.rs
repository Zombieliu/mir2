use super::*;
use crate::config::{AccountStore, AccountStoreTransactionFault};

struct Fixture {
    config: SimulationConfig,
    session: SimulationSession,
    path: std::path::PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

impl Fixture {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "mir2-mail-send-publication-{label}-{}-{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let config = SimulationConfig::default().with_account_store_path(path.clone());
        let mut session = SimulationSession::new(config.clone());
        assert!(session
            .handle_packet(ClientPacket::Login {
                account_id: "demo".into(),
                password: "demo".into(),
            })
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
        assert!(session
            .handle_packet(ClientPacket::StartGame { character_index: 0 })
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
        session
            .app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .gold = 10_000;
        session.save_active_character().unwrap();
        assert!(path.is_file());
        assert!(config.ensure_account_store_writable().is_ok());
        assert_eq!(
            config.account_store.lock().unwrap().accounts["demo"].saves[&0].gold,
            10_000
        );
        Self {
            config,
            session,
            path,
        }
    }

    fn send(&mut self) -> Vec<ServerPacket> {
        self.session.handle_packet(ClientPacket::SendMail {
            name: "Scout".into(),
            message: "publication result".into(),
            gold: 25,
            items_idx: [0; 5],
            stamped: false,
        })
    }

    fn file_store(&self) -> AccountStore {
        serde_json::from_slice(&std::fs::read(&self.path).unwrap()).unwrap()
    }

    fn cached_store(&self) -> serde_json::Value {
        serde_json::to_value(&*self.config.account_store.lock().unwrap()).unwrap()
    }

    fn commit_without_live_publication(&self) -> (u64, u32, Stage5MailCommit) {
        let world = self.session.app.world();
        let sender = stage5_mail_sender(world).unwrap();
        let target = stage5_mail_target_for_name(&self.config, "Scout").unwrap();
        let before = snapshot_active_character_save(world).unwrap();
        let plan = stage5_mail_parcel_plan(
            &world.resource::<InventoryResource>().inventory_items,
            25,
            &[0; 5],
            false,
        )
        .unwrap();
        let total = 25u32.checked_add(plan.cost).unwrap();
        let committed = stage5_commit_mail_transaction(
            &self.config,
            &sender,
            &target,
            Stage5MailMessage {
                id: 0,
                delivery_nonce: new_stage5_mail_delivery_nonce(),
                from: sender.character_name.clone(),
                to: target.character_name.clone(),
                subject: String::new(),
                body: "publication result".into(),
                gold: 25,
                items: Vec::new(),
                item_states_json: Vec::new(),
                attachment_origin: crate::config::Stage5MailAttachmentOrigin::LegacyUnknown,
                opened: false,
                locked: false,
                claimed: false,
                deleted: false,
            },
            total,
            &[],
            &[],
            &[0; 5],
            false,
            None,
        )
        .unwrap();
        (before.revision, total, committed)
    }
}

#[test]
fn mail_send_known_file_rejection_reports_failure_and_can_retry_once() {
    let mut fixture = Fixture::new("known-rejection");
    let before_world = fixture.session.world_snapshot();
    let before_cache = fixture.cached_store();
    let before_file = std::fs::read(&fixture.path).unwrap();
    fixture
        .config
        .inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforeFileRename);

    assert_eq!(fixture.send(), vec![ServerPacket::MailSent { result: -1 }]);
    assert!(fixture.config.ensure_account_store_writable().is_ok());
    assert_eq!(fixture.session.world_snapshot(), before_world);
    assert_eq!(fixture.cached_store(), before_cache);
    assert_eq!(std::fs::read(&fixture.path).unwrap(), before_file);

    assert!(fixture
        .send()
        .contains(&ServerPacket::MailSent { result: 1 }));
    let durable = fixture.file_store();
    let mail: Stage5SystemsState = serde_json::from_str(
        durable.accounts["demo"].saves[&0]
            .stage5_systems_json
            .as_deref()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(mail.mail.len(), 1);
    assert_eq!(mail.mail[0].gold, 25);
}

#[test]
fn mail_send_unknown_file_commit_freezes_clones_without_false_failure_or_retry() {
    let mut fixture = Fixture::new("unknown");
    let clone = fixture.config.clone();
    let before_world = fixture.session.world_snapshot();
    let before_cache = fixture.cached_store();
    let before_revision = fixture
        .session
        .app
        .world()
        .resource::<SessionResource>()
        .active_save_revision()
        .unwrap();
    fixture.config.inject_account_store_transaction_fault(
        AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync,
    );

    assert!(fixture.send().is_empty());
    assert!(clone.ensure_account_store_writable().is_err());
    assert_eq!(fixture.session.world_snapshot(), before_world);
    assert_eq!(fixture.cached_store(), before_cache);
    let durable = fixture.file_store();
    let save = &durable.accounts["demo"].saves[&0];
    assert_eq!(save.revision, before_revision + 1);
    assert_eq!(save.gold, 9_975);
    let mail: Stage5SystemsState =
        serde_json::from_str(save.stage5_systems_json.as_deref().unwrap()).unwrap();
    assert_eq!(mail.mail.len(), 1);
    assert_eq!(mail.mail[0].gold, 25);
    let after_file = std::fs::read(&fixture.path).unwrap();

    assert!(fixture.send().is_empty());
    assert_eq!(std::fs::read(&fixture.path).unwrap(), after_file);
    assert_eq!(fixture.cached_store(), before_cache);
    assert_eq!(fixture.session.world_snapshot(), before_world);
}

#[test]
fn mail_send_confirmed_source_failed_revision_publication_freezes_without_success() {
    let mut fixture = Fixture::new("publication");
    let clone = fixture.config.clone();
    let before_world = fixture.session.world_snapshot();
    let (revision, total, committed) = fixture.commit_without_live_publication();
    assert_eq!(committed.sender_baseline_revision, revision);
    assert_eq!(committed.sender_committed_revision, revision + 1);
    // Model a competing revision publisher after the real File commit and
    // before this same production publication routine performs its CAS.
    fixture
        .session
        .app
        .world()
        .resource::<SessionResource>()
        .bind_active_save_revision(revision + 10);
    let packets = stage5_publish_mail_commit(
        fixture.session.app.world_mut(),
        &fixture.config,
        revision,
        committed,
        total,
    );

    assert!(packets.is_empty());
    assert!(clone
        .ensure_account_store_writable()
        .unwrap_err()
        .contains("SOURCE_COMMITTED_LIVE_PUBLICATION_FAILED"));
    assert_eq!(fixture.session.world_snapshot(), before_world);
    let durable = fixture.file_store();
    let save = &durable.accounts["demo"].saves[&0];
    assert_eq!(save.gold, 10_000 - total);
    assert_eq!(save.revision, revision + 1);
    let after_file = std::fs::read(&fixture.path).unwrap();
    assert!(fixture.send().is_empty());
    assert_eq!(std::fs::read(&fixture.path).unwrap(), after_file);
}

#[test]
fn mail_send_stale_session_refreshes_affected_assets_without_advancing_old_revision() {
    let mut fixture = Fixture::new("stale");
    let revision = fixture
        .session
        .app
        .world()
        .resource::<SessionResource>()
        .active_save_revision()
        .unwrap();
    fixture
        .config
        .commit_account_store_transaction(&["demo".into()], |store| {
            let save = store
                .accounts
                .get_mut("demo")
                .unwrap()
                .saves
                .get_mut(&0)
                .unwrap();
            save.gold = 9_000;
            save.revision += 1;
            Ok(())
        })
        .unwrap();

    assert!(fixture
        .send()
        .contains(&ServerPacket::MailSent { result: 1 }));
    assert_eq!(fixture.session.world_snapshot().gold, 8_975);
    assert_eq!(
        fixture
            .session
            .app
            .world()
            .resource::<SessionResource>()
            .active_save_revision(),
        Some(revision)
    );
    let before_save = std::fs::read(&fixture.path).unwrap();
    let source = fixture.file_store();
    assert_eq!(source.accounts["demo"].saves[&0].gold, 8_975);
    assert_eq!(source.accounts["demo"].saves[&0].revision, revision + 2);
    assert!(fixture.session.save_active_character().is_err());
    let after_save = fixture.file_store();
    assert_eq!(after_save.accounts["demo"].saves[&0].gold, 8_975);
    assert_eq!(after_save.accounts["demo"].saves[&0].revision, revision + 2);
    assert_eq!(std::fs::read(&fixture.path).unwrap(), before_save);
}
