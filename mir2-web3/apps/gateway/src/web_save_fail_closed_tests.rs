use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::{SimulationConfig, SimulationSession};

use super::{
    apply_teardown_persistence_to_resume, execute_session_action, gateway_unix_ms,
    session_action_error_event, GatewayCapacityState, GatewaySaveQueueConfig,
    NativeResumeConnectionState, ReconnectSessionStore, SessionAction, WebSessionSaveQueue,
    WebTeardownPersistenceOutcome,
};
use crate::resume::{ResumeConnectionNonce, ResumeIssueContext};
use crate::session::save_recovery::{
    journal_checkpoint, provision_account_if_recovery_clear, release_directory_lease_for_tests,
    replay_account,
};
use crate::{GatewaySession, GatewaySessionCacheKey};

static RECOVERY_TEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);
const RECOVERY_TEST_MAC_KEY: [u8; 32] = [
    0x91, 0x82, 0x73, 0x64, 0x55, 0x46, 0x37, 0x28, 0x19, 0x0a, 0xfb, 0xec, 0xdd, 0xce, 0xbf, 0xa0,
    0x92, 0x83, 0x74, 0x65, 0x56, 0x47, 0x38, 0x29, 0x1a, 0x0b, 0xfc, 0xed, 0xde, 0xcf, 0xb1, 0xa2,
];

#[derive(Debug, Clone, Copy)]
enum StandardLoginPath {
    TcpGatewaySession,
    DevelopmentWeb,
    ProductionWeb,
}

struct RecoveryLoginFixture {
    root: PathBuf,
    recovery_root: PathBuf,
    config: SimulationConfig,
}

impl RecoveryLoginFixture {
    fn new(label: &str) -> Self {
        let sequence = RECOVERY_TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "mir2-web-recovery-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        let recovery_root = root.join("recovery-owned");
        let config = SimulationConfig::default()
            .with_account_store_path(root.join("accounts.json"))
            .with_save_recovery_dir(&recovery_root)
            .with_save_recovery_mac_key(RECOVERY_TEST_MAC_KEY)
            .unwrap();
        Self {
            root,
            recovery_root,
            config,
        }
    }

    fn checkpoint(&self) -> mir2_simulation::CharacterSaveRecord {
        let mut source = SimulationSession::new(self.config.clone());
        source.select_account_for_recovery("demo").unwrap();
        let packets = source.handle_packet(ClientPacket::StartGame { character_index: 0 });
        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
        source.active_character_checkpoint().unwrap()
    }

    fn journal_count(&self) -> usize {
        fs::read_dir(&self.recovery_root)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .filter(|entry| {
                        entry
                            .file_name()
                            .to_str()
                            .is_some_and(|name| name.ends_with(".journal.json"))
                    })
                    .count()
            })
            .unwrap_or_default()
    }
}

impl Drop for RecoveryLoginFixture {
    fn drop(&mut self) {
        release_directory_lease_for_tests(&self.recovery_root);
        let safe = self
            .root
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("mir2-web-recovery-"));
        if safe {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

#[test]
fn forced_save_failure_marks_clean_queue_dirty_and_retry_clears_it() {
    let started = Instant::now();
    let mut queue = WebSessionSaveQueue::new(GatewaySaveQueueConfig::new(
        Duration::from_secs(60),
        Duration::from_secs(60),
        8,
    ));

    let error = queue
        .force_save_now(started, || Err("injected save failure".to_string()))
        .expect_err("forced save failure must reach the caller");

    assert_eq!(error, "injected save failure");
    assert!(queue.has_pending_save());
    assert_eq!(queue.queued_requests, 1);

    queue
        .force_save_now(started + Duration::from_millis(1), || Ok(()))
        .expect("retry should clear the pending save");
    assert!(!queue.has_pending_save());
    assert_eq!(queue.queued_requests, 0);
}

#[test]
fn debounced_save_failure_keeps_dirty_until_successful_retry() {
    let started = Instant::now();
    let mut queue = WebSessionSaveQueue::new(GatewaySaveQueueConfig::new(
        Duration::ZERO,
        Duration::ZERO,
        1,
    ));

    queue
        .request_save(started, || Err("injected queue failure".to_string()))
        .expect_err("queue flush failure must reach the caller");
    assert!(queue.has_pending_save());

    queue
        .checkpoint(started + Duration::from_millis(1), || Ok(()))
        .expect("checkpoint retry should commit the dirty state");
    assert!(!queue.has_pending_save());
}

#[test]
fn wrong_password_never_replays_recovery_on_tcp_production_web_or_dev_web() {
    for path in [
        StandardLoginPath::TcpGatewaySession,
        StandardLoginPath::DevelopmentWeb,
        StandardLoginPath::ProductionWeb,
    ] {
        let fixture = RecoveryLoginFixture::new(&format!("wrong-password-{path:?}"));
        let mut checkpoint = fixture.checkpoint();
        checkpoint.character.name = format!("MustNotReplay{path:?}");
        journal_checkpoint(&fixture.config, "demo", 0, &checkpoint).unwrap();
        let mut session = GatewaySession::new(fixture.config.clone());

        let responses = execute_standard_login(path, &mut session, "definitely-wrong").unwrap();

        assert!(responses
            .iter()
            .all(|packet| !matches!(packet, ServerPacket::LoginSuccess { .. })));
        assert_eq!(
            fixture.journal_count(),
            1,
            "wrong password must not acquire the global recovery lock for replay on {path:?}"
        );
        let durable_name = fixture
            .config
            .account_store
            .lock()
            .unwrap()
            .accounts
            .get("demo")
            .unwrap()
            .characters[0]
            .name
            .clone();
        assert_ne!(durable_name, checkpoint.character.name);
    }
}

#[test]
fn banned_standard_login_never_replays_recovery_on_all_three_paths() {
    for path in [
        StandardLoginPath::TcpGatewaySession,
        StandardLoginPath::DevelopmentWeb,
        StandardLoginPath::ProductionWeb,
    ] {
        let fixture = RecoveryLoginFixture::new(&format!("banned-standard-{path:?}"));
        let checkpoint = fixture.checkpoint();
        journal_checkpoint(&fixture.config, "demo", 0, &checkpoint).unwrap();
        fixture
            .config
            .account_store
            .lock()
            .unwrap()
            .accounts
            .get_mut("demo")
            .unwrap()
            .is_banned = true;
        let mut session = GatewaySession::new(fixture.config.clone());

        let responses = execute_standard_login(path, &mut session, "demo").unwrap();

        assert!(responses
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginBanned { .. })));
        assert_eq!(fixture.journal_count(), 1);
    }
}

#[test]
fn source_rejected_or_unavailable_preflight_never_cleans_or_applies_journal() {
    let fixture = RecoveryLoginFixture::new("source-preflight-no-replay");
    let mut checkpoint = fixture.checkpoint();
    checkpoint.character.name = "MustRemainPendingAfterSourceFailure".to_string();
    journal_checkpoint(&fixture.config, "demo", 0, &checkpoint).unwrap();
    let original_name = fixture
        .config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get("demo")
        .unwrap()
        .characters[0]
        .name
        .clone();
    let mut session = GatewaySession::new(fixture.config.clone());

    session
        .replay_account_recovery_after_preflight("demo", Ok(false))
        .unwrap();
    assert!(session
        .replay_account_recovery_after_preflight(
            "demo",
            Err("injected authoritative source unavailable".to_string()),
        )
        .is_err());

    assert_eq!(fixture.journal_count(), 1);
    assert_eq!(
        fixture
            .config
            .account_store
            .lock()
            .unwrap()
            .accounts
            .get("demo")
            .unwrap()
            .characters[0]
            .name,
        original_name
    );
}

#[test]
fn authenticated_login_replays_once_and_returns_fresh_roster_on_all_three_paths() {
    for path in [
        StandardLoginPath::TcpGatewaySession,
        StandardLoginPath::DevelopmentWeb,
        StandardLoginPath::ProductionWeb,
    ] {
        let fixture = RecoveryLoginFixture::new(&format!("fresh-roster-{path:?}"));
        let mut checkpoint = fixture.checkpoint();
        checkpoint.character.level = checkpoint.character.level.saturating_add(7);
        let level_defaults =
            mir2_simulation::CharacterSaveRecord::new(checkpoint.character.clone());
        checkpoint.max_hp = level_defaults.max_hp;
        checkpoint.max_mp = level_defaults.max_mp;
        checkpoint.max_experience = fixture
            .config
            .experience_required_for_level(checkpoint.character.level);
        journal_checkpoint(&fixture.config, "demo", 0, &checkpoint).unwrap();
        let mut session = GatewaySession::new(fixture.config.clone());

        let responses = execute_standard_login(path, &mut session, "demo").unwrap();

        let rosters = responses
            .iter()
            .filter_map(|packet| match packet {
                ServerPacket::LoginSuccess { characters } => Some(characters),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            rosters.len(),
            1,
            "LoginSuccess must be emitted once on {path:?}"
        );
        assert!(rosters[0]
            .iter()
            .any(|character| character.level == checkpoint.character.level));
        assert_eq!(fixture.journal_count(), 0);
    }
}

#[test]
fn first_time_trusted_passkey_provisions_only_after_recovery_clear_check() {
    let fixture = RecoveryLoginFixture::new("first-passkey-provision");
    let account_id = "wallet:first-time-recovery-clear";
    let mut session = GatewaySession::new(fixture.config.clone());

    let responses = session.try_passkey_login(account_id).unwrap();

    assert!(responses
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let store = fixture.config.account_store.lock().unwrap();
    let account = store.accounts.get(account_id).unwrap();
    assert_ne!(account.password, "demo");
    assert_eq!(fixture.journal_count(), 0);
}

#[test]
fn deleted_passkey_account_with_pending_journal_is_not_recreated() {
    let fixture = RecoveryLoginFixture::new("deleted-passkey-journal");
    let checkpoint = fixture.checkpoint();
    journal_checkpoint(&fixture.config, "demo", 0, &checkpoint).unwrap();
    fixture
        .config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .remove("demo");
    let provision_called = std::cell::Cell::new(false);

    let error = provision_account_if_recovery_clear(&fixture.config, "demo", || {
        provision_called.set(true);
        Ok(())
    })
    .unwrap_err();

    assert!(error.contains("pending recovery state"));
    assert!(!provision_called.get());
    assert_eq!(fixture.journal_count(), 1);
    assert!(!fixture
        .config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .contains_key("demo"));
}

#[test]
fn banned_passkey_account_never_replays_its_pending_journal() {
    let fixture = RecoveryLoginFixture::new("banned-passkey-no-replay");
    let mut checkpoint = fixture.checkpoint();
    checkpoint.character.name = "MustRemainJournaledWhileBanned".to_string();
    journal_checkpoint(&fixture.config, "demo", 0, &checkpoint).unwrap();
    fixture
        .config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .is_banned = true;
    let mut session = GatewaySession::new(fixture.config.clone());

    let responses = session.try_passkey_login("demo").unwrap();

    assert!(responses
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginBanned { .. })));
    assert_eq!(fixture.journal_count(), 1);
}

#[test]
fn unverified_web_passkey_action_cannot_provision_an_account() {
    for production in [false, true] {
        let fixture = RecoveryLoginFixture::new(if production {
            "raw-passkey-production"
        } else {
            "raw-passkey-development"
        });
        let account_id = "wallet:raw-client-must-not-provision";
        let mut session = GatewaySession::new(fixture.config.clone());

        let error = execute_session_action(
            &mut session,
            SessionAction::PasskeyLogin {
                account_id: account_id.to_string(),
                proof_account_id: account_id.to_string(),
                token: "not-a-valid-passkey-token".to_string(),
            },
            false,
            production,
        )
        .unwrap_err();

        assert!(!error.is_empty());
        assert!(!fixture
            .config
            .account_store
            .lock()
            .unwrap()
            .accounts
            .contains_key(account_id));
    }
}

#[test]
fn journaled_teardown_revokes_resume_then_replay_fences_stale_writer() {
    let fixture = RecoveryLoginFixture::new("journal-terminal-takeover");
    let cached_session = started_gateway_session(&fixture.config);
    let mut stale_writer = started_gateway_session(&fixture.config);
    let active_identity = cached_session.active_identity().unwrap();
    let gateway_session_id = cached_session.session_id().to_string();
    let identity_session_id = "journal-terminal-identity";
    let nonce = ResumeConnectionNonce::generate();
    let now_ms = gateway_unix_ms();
    let reconnect_store = ReconnectSessionStore::default();
    let issued = reconnect_store
        .issue_resume_credential(
            None,
            ResumeIssueContext {
                account_id: &active_identity.account_id,
                character_index: active_identity.character_index,
                gateway_session_id: &gateway_session_id,
                identity_session_id,
                identity_expires_at_ms: now_ms.saturating_add(60_000),
                source_connection_nonce: &nonce,
            },
            now_ms,
            1,
            || true,
        )
        .unwrap();
    let binding = issued.binding.clone();
    let capacity = Arc::new(GatewayCapacityState::with_limits(None, Some(1), Some(1)));
    let active_permit = capacity.try_acquire_active_session().unwrap();
    let reconnect_permit = capacity.try_acquire_reconnect_lease().unwrap();
    reconnect_store.store(
        GatewaySessionCacheKey {
            account_id: active_identity.account_id.clone(),
            character_index: active_identity.character_index,
        },
        cached_session,
        Some(active_permit),
        reconnect_permit,
        Some(binding.family_id.clone()),
        Duration::from_secs(60),
    );
    let mut native_resume = NativeResumeConnectionState::new();
    native_resume.opted_in = true;
    native_resume.family_id = Some(binding.family_id.clone());
    let mut checkpoint = fixture.checkpoint();
    checkpoint.gold = checkpoint.gold.saturating_add(4_321);
    journal_checkpoint(&fixture.config, "demo", 0, &checkpoint).unwrap();

    let retain = apply_teardown_persistence_to_resume(
        WebTeardownPersistenceOutcome::Journaled,
        &mut native_resume,
        &reconnect_store,
        Some("demo"),
    );

    assert!(!retain);
    assert!(!native_resume.resume_allowed);
    assert_eq!(reconnect_store.len(), 0);
    assert!(reconnect_store
        .take_by_credential(&issued.credential, &binding, now_ms)
        .is_none());

    let replay = replay_account(&fixture.config, "demo").unwrap();
    assert_eq!(replay.replayed, 1);
    assert_eq!(fixture.journal_count(), 0);
    let stale_error = stale_writer.save_active_character().unwrap_err();
    assert!(stale_error.contains("stale"));
    let durable = fixture
        .config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get("demo")
        .unwrap()
        .saves
        .get(&0)
        .unwrap()
        .clone();
    assert_eq!(durable.gold, checkpoint.gold);
}

#[test]
fn saved_teardown_preserves_resume_policy_but_journaled_never_retains() {
    let store = ReconnectSessionStore::default();
    let mut saved = NativeResumeConnectionState::new();
    saved.opted_in = true;
    assert!(apply_teardown_persistence_to_resume(
        WebTeardownPersistenceOutcome::Saved,
        &mut saved,
        &store,
        Some("demo"),
    ));
    let mut journaled = NativeResumeConnectionState::new();
    journaled.opted_in = true;
    assert!(!apply_teardown_persistence_to_resume(
        WebTeardownPersistenceOutcome::Journaled,
        &mut journaled,
        &store,
        Some("demo"),
    ));
}

#[test]
fn client_visible_login_and_save_errors_are_fixed_and_secret_free() {
    let sensitive = [
        r"C:\private\recovery\demo.journal.json",
        "demo-account-id",
        "postgres://operator:secret@db.internal/mir2",
        "stale CAS revision 41",
    ];
    for event in [
        session_action_error_event(true, false),
        session_action_error_event(false, true),
    ] {
        let encoded = event.to_string();
        for fragment in sensitive {
            assert!(!encoded.contains(fragment));
        }
    }
    assert_eq!(
        session_action_error_event(true, false)["code"],
        "accountStateUnavailable"
    );
    assert_eq!(
        session_action_error_event(false, true)["code"],
        "saveUnavailable"
    );
}

#[test]
fn persistence_admission_capacity_is_finite_and_permits_are_released() {
    let admission = Arc::new(tokio::sync::Semaphore::new(1));
    let first = Arc::clone(&admission).try_acquire_owned().unwrap();
    assert!(Arc::clone(&admission).try_acquire_owned().is_err());
    drop(first);
    assert!(Arc::clone(&admission).try_acquire_owned().is_ok());
}

#[test]
fn dev_and_production_web_leave_paths_propagate_stale_save_and_keep_identity() {
    for production in [false, true] {
        for packet in [ClientPacket::LogOut, ClientPacket::Disconnect] {
            let config = SimulationConfig::default();
            let mut current = started_gateway_session(&config);
            let mut stale = started_gateway_session(&config);
            current.save_active_character().unwrap();

            let error =
                execute_session_action(&mut stale, SessionAction::Packet(packet), true, production)
                    .unwrap_err();

            assert!(error.contains("stale"));
            assert!(stale.active_identity().is_some());
            assert!(stale.zone_movement_ingress().is_some());
        }
    }
}

fn execute_standard_login(
    path: StandardLoginPath,
    session: &mut GatewaySession,
    password: &str,
) -> Result<Vec<ServerPacket>, String> {
    let packet = ClientPacket::Login {
        account_id: "demo".to_string(),
        password: password.to_string(),
    };
    match path {
        StandardLoginPath::TcpGatewaySession => session.try_handle_packet(packet),
        StandardLoginPath::DevelopmentWeb => {
            execute_session_action(session, SessionAction::Packet(packet), false, false)
        }
        StandardLoginPath::ProductionWeb => {
            execute_session_action(session, SessionAction::Packet(packet), false, true)
        }
    }
}

fn started_gateway_session(config: &SimulationConfig) -> GatewaySession {
    let mut session = GatewaySession::new(config.clone());
    let login = session
        .try_handle_packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        })
        .unwrap();
    assert!(login
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let start = session
        .try_handle_packet(ClientPacket::StartGame { character_index: 0 })
        .unwrap();
    assert!(start
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    session
}
