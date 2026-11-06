use super::*;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use mir2_protocol::ClientPacket;
use mir2_simulation::{SimulationSession, WorldCommandExecution};

use crate::routing::{
    persist_zone_teardown_checkpoint, zone_teardown_is_fenced, InMemoryZoneOwnerLeaseAuthority,
    PreparedZoneTeardown, SharedZoneOwnerLeaseAuthority, ZoneId, ZoneOwnerCommandClient,
};

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);
const TEST_MAC_KEY: [u8; 32] = [
    0x10, 0x21, 0x32, 0x43, 0x54, 0x65, 0x76, 0x87, 0x98, 0xa9, 0xba, 0xcb, 0xdc, 0xed, 0xfe, 0x0f,
    0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xf1, 0x02,
];

struct TeardownFixture {
    root: PathBuf,
    recovery_root: PathBuf,
    config: GatewayConfig,
}

impl TeardownFixture {
    fn new(label: &str) -> Self {
        let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "mir2-abnormal-teardown-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create teardown test root");
        let recovery_root = root.join("recovery-owned");
        let config = GatewayConfig::default()
            .with_account_store_path(root.join("accounts.json"))
            .with_save_recovery_dir(&recovery_root)
            .with_save_recovery_mac_key(TEST_MAC_KEY)
            .expect("valid recovery MAC key");
        Self {
            root,
            recovery_root,
            config,
        }
    }

    fn block_journal_after_login(&self) {
        save_recovery::release_directory_lease_for_tests(&self.recovery_root);
        if self.recovery_root.is_dir() {
            fs::remove_dir_all(&self.recovery_root).expect("remove recovery directory");
        }
        fs::write(&self.recovery_root, b"not-a-directory").expect("create blocked journal path");
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

impl Drop for TeardownFixture {
    fn drop(&mut self) {
        save_recovery::release_directory_lease_for_tests(&self.recovery_root);
        let safe = self
            .root
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("mir2-abnormal-teardown-"));
        if safe {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

#[derive(Debug)]
struct InjectedPersistFailureClient {
    inner: InProcessZoneOwnerCommandClient,
    remaining_failures: AtomicUsize,
    seen_checkpoints: Mutex<Vec<serde_json::Value>>,
}

impl InjectedPersistFailureClient {
    fn new(failures: usize) -> Self {
        Self {
            inner: InProcessZoneOwnerCommandClient::new(),
            remaining_failures: AtomicUsize::new(failures),
            seen_checkpoints: Mutex::new(Vec::new()),
        }
    }

    fn seen_checkpoints(&self) -> Vec<serde_json::Value> {
        self.seen_checkpoints
            .lock()
            .expect("seen checkpoints")
            .clone()
    }
}

impl ZoneOwnerCommandClient for InjectedPersistFailureClient {
    fn execute(
        &self,
        runtime: &mut ZoneRuntimeHandle,
        request: ZoneOwnerCommandRequest,
    ) -> Result<WorldCommandExecution, String> {
        self.inner.execute(runtime, request)
    }

    fn persist_teardown_checkpoint(
        &self,
        runtime: &mut ZoneRuntimeHandle,
        prepared: &PreparedZoneTeardown,
    ) -> Result<(), String> {
        self.seen_checkpoints
            .lock()
            .map_err(|_| "seen checkpoint mutex poisoned".to_string())?
            .push(serde_json::to_value(prepared.checkpoint()).map_err(|error| error.to_string())?);
        let fail = self
            .remaining_failures
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                remaining.checked_sub(1)
            })
            .is_ok();
        if fail {
            return Err("injected DB checkpoint failure".to_string());
        }
        persist_zone_teardown_checkpoint(runtime, prepared)
    }
}

#[derive(Debug, Clone, Copy)]
enum PreparedIdentityFault {
    SwitchedAccount,
    MismatchedCharacter,
}

#[derive(Debug)]
struct AdversarialPreparedOwner {
    inner: InProcessZoneOwnerCommandClient,
    fault: PreparedIdentityFault,
    active_identity_reads: AtomicUsize,
    persist_calls: AtomicUsize,
    release_calls: AtomicUsize,
}

impl AdversarialPreparedOwner {
    fn new(fault: PreparedIdentityFault) -> Self {
        Self {
            inner: InProcessZoneOwnerCommandClient::new(),
            fault,
            active_identity_reads: AtomicUsize::new(0),
            persist_calls: AtomicUsize::new(0),
            release_calls: AtomicUsize::new(0),
        }
    }
}

impl ZoneOwnerCommandClient for AdversarialPreparedOwner {
    fn execute(
        &self,
        runtime: &mut ZoneRuntimeHandle,
        request: ZoneOwnerCommandRequest,
    ) -> Result<WorldCommandExecution, String> {
        self.inner.execute(runtime, request)
    }

    fn active_identity(
        &self,
        runtime: &ZoneRuntimeHandle,
    ) -> Result<Option<ActiveSessionIdentity>, String> {
        self.active_identity_reads.fetch_add(1, Ordering::SeqCst);
        Ok(runtime.active_identity())
    }

    fn prepare_teardown_checkpoint(
        &self,
        runtime: &mut ZoneRuntimeHandle,
        owner_lease: &ZoneOwnerLease,
    ) -> Result<Option<PreparedZoneTeardown>, String> {
        let mut prepared = self
            .inner
            .prepare_teardown_checkpoint(runtime, owner_lease)?;
        if let Some(prepared) = prepared.as_mut() {
            match self.fault {
                PreparedIdentityFault::SwitchedAccount => {
                    prepared.identity.account_id = "other-account".to_string();
                }
                PreparedIdentityFault::MismatchedCharacter => {
                    prepared.checkpoint.character.name.push_str("-mismatch");
                }
            }
        }
        Ok(prepared)
    }

    fn persist_teardown_checkpoint(
        &self,
        runtime: &mut ZoneRuntimeHandle,
        prepared: &PreparedZoneTeardown,
    ) -> Result<(), String> {
        self.persist_calls.fetch_add(1, Ordering::SeqCst);
        self.inner.persist_teardown_checkpoint(runtime, prepared)
    }

    fn release_teardown_fence(&self, runtime: &mut ZoneRuntimeHandle) -> Result<(), String> {
        self.release_calls.fetch_add(1, Ordering::SeqCst);
        self.inner.release_teardown_fence(runtime)
    }
}

#[derive(Debug)]
struct LeaseRacingPreparedOwner {
    inner: InProcessZoneOwnerCommandClient,
    authority: Arc<InMemoryZoneOwnerLeaseAuthority>,
    zone_id: ZoneId,
    persist_calls: AtomicUsize,
    release_calls: AtomicUsize,
}

impl LeaseRacingPreparedOwner {
    fn new(authority: Arc<InMemoryZoneOwnerLeaseAuthority>, zone_id: ZoneId) -> Self {
        Self {
            inner: InProcessZoneOwnerCommandClient::with_owner_lease_authority(
                authority.clone() as SharedZoneOwnerLeaseAuthority
            ),
            authority,
            zone_id,
            persist_calls: AtomicUsize::new(0),
            release_calls: AtomicUsize::new(0),
        }
    }
}

impl ZoneOwnerCommandClient for LeaseRacingPreparedOwner {
    fn execute(
        &self,
        runtime: &mut ZoneRuntimeHandle,
        request: ZoneOwnerCommandRequest,
    ) -> Result<WorldCommandExecution, String> {
        self.inner.execute(runtime, request)
    }

    fn prepare_teardown_checkpoint(
        &self,
        runtime: &mut ZoneRuntimeHandle,
        owner_lease: &ZoneOwnerLease,
    ) -> Result<Option<PreparedZoneTeardown>, String> {
        let prepared = self
            .inner
            .prepare_teardown_checkpoint(runtime, owner_lease)?;
        self.authority
            .handoff_zone_owner(&self.zone_id, "promoted-owner");
        Ok(prepared)
    }

    fn persist_teardown_checkpoint(
        &self,
        runtime: &mut ZoneRuntimeHandle,
        prepared: &PreparedZoneTeardown,
    ) -> Result<(), String> {
        self.persist_calls.fetch_add(1, Ordering::SeqCst);
        self.inner.persist_teardown_checkpoint(runtime, prepared)
    }

    fn release_teardown_fence(&self, runtime: &mut ZoneRuntimeHandle) -> Result<(), String> {
        self.release_calls.fetch_add(1, Ordering::SeqCst);
        self.inner.release_teardown_fence(runtime)
    }
}

fn started_session(config: &GatewayConfig) -> GatewaySession {
    let registry = ZoneRegistry::in_process();
    let mut session = GatewaySession::new_with_zone_registry(config.clone(), &registry);
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".to_string(),
        password: "demo".to_string(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(session.active_identity().is_some());
    session
}

#[test]
fn mismatched_or_switched_prepared_identity_is_rejected_without_persist_journal_or_cleanup() {
    for (label, fault, error_fragment) in [
        (
            "switched-account",
            PreparedIdentityFault::SwitchedAccount,
            "session-bound identity",
        ),
        (
            "mismatched-character",
            PreparedIdentityFault::MismatchedCharacter,
            "character name",
        ),
    ] {
        let fixture = TeardownFixture::new(label);
        let mut session = started_session(&fixture.config);
        let client = Arc::new(AdversarialPreparedOwner::new(fault));
        session.zone_owner_command_client = client.clone() as SharedZoneOwnerCommandClient;

        let outcome = session.try_persist_teardown_once();
        let GatewayTeardownPersistenceOutcome::Retry {
            prepare_error: Some(error),
            save_error: None,
            journal_error: None,
        } = outcome
        else {
            panic!("invalid prepared identity must fail during preparation: {outcome:?}");
        };
        assert!(error.contains(error_fragment), "{error}");
        assert_eq!(client.active_identity_reads.load(Ordering::SeqCst), 0);
        assert_eq!(client.persist_calls.load(Ordering::SeqCst), 0);
        assert_eq!(client.release_calls.load(Ordering::SeqCst), 0);
        assert_eq!(fixture.journal_count(), 0);
        assert!(zone_teardown_is_fenced(&session.runtime));
        assert!(session.prepared_teardown_checkpoint().is_none());
    }
}

#[test]
fn owner_lease_handoff_racing_prepare_is_rejected_before_cache_or_persistence() {
    let fixture = TeardownFixture::new("lease-race");
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let shared_authority = authority.clone() as SharedZoneOwnerLeaseAuthority;
    let registry = ZoneRegistry::in_process_with_owner_lease_authority(shared_authority);
    let mut session = GatewaySession::new_with_zone_registry(fixture.config.clone(), &registry);
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".to_string(),
        password: "demo".to_string(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let client = Arc::new(LeaseRacingPreparedOwner::new(authority, ZoneId::primary()));
    session.zone_owner_command_client = client.clone() as SharedZoneOwnerCommandClient;

    let outcome = session.try_persist_teardown_once();
    let GatewayTeardownPersistenceOutcome::Retry {
        prepare_error: Some(error),
        save_error: None,
        journal_error: None,
    } = outcome
    else {
        panic!("lease handoff during prepare must fail closed: {outcome:?}");
    };
    assert!(error.contains("stale zone owner lease"), "{error}");
    assert_eq!(client.persist_calls.load(Ordering::SeqCst), 0);
    assert_eq!(client.release_calls.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.journal_count(), 0);
    assert!(zone_teardown_is_fenced(&session.runtime));
    assert!(session.prepared_teardown_checkpoint().is_none());
}

#[test]
fn db_failure_journals_the_exact_frozen_checkpoint_and_revokes_resume_eligibility() {
    let fixture = TeardownFixture::new("journal");
    let mut session = started_session(&fixture.config);
    let client = Arc::new(InjectedPersistFailureClient::new(1));
    session.zone_owner_command_client = client.clone() as SharedZoneOwnerCommandClient;

    let outcome = session.try_persist_teardown_once();
    assert!(matches!(
        outcome,
        GatewayTeardownPersistenceOutcome::Journaled { .. }
    ));
    assert!(zone_teardown_is_fenced(&session.runtime));
    let prepared = serde_json::to_value(
        session
            .prepared_teardown_checkpoint()
            .expect("immutable checkpoint remains cached"),
    )
    .expect("serialize prepared checkpoint");
    assert_eq!(client.seen_checkpoints(), vec![prepared.clone()]);
    assert_eq!(fixture.journal_count(), 1);

    let replay = save_recovery::replay_account(&fixture.config, "demo")
        .expect("real recovery journal should replay");
    assert_eq!(replay.replayed, 1);
    let mut recovered = SimulationSession::new(fixture.config.clone());
    recovered.handle_packet(ClientPacket::Login {
        account_id: "demo".to_string(),
        password: "demo".to_string(),
    });
    recovered.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let mut recovered = serde_json::to_value(
        recovered
            .active_character_checkpoint()
            .expect("replayed checkpoint"),
    )
    .expect("serialize replayed checkpoint");
    let prepared_revision = prepared["revision"].as_u64().expect("prepared revision");
    let recovered_revision = recovered["revision"].as_u64().expect("recovered revision");
    assert_eq!(recovered_revision, prepared_revision + 1);
    recovered["revision"] = prepared["revision"].clone();
    assert_eq!(recovered, prepared);
}

#[test]
fn db_and_journal_failure_retain_frozen_authority_and_retry_same_checkpoint() {
    let fixture = TeardownFixture::new("retry");
    let mut session = started_session(&fixture.config);
    let client = Arc::new(InjectedPersistFailureClient::new(1));
    session.zone_owner_command_client = client.clone() as SharedZoneOwnerCommandClient;
    fixture.block_journal_after_login();

    let first = session.try_persist_teardown_once();
    assert!(matches!(
        first,
        GatewayTeardownPersistenceOutcome::Retry {
            prepare_error: None,
            save_error: Some(_),
            journal_error: Some(_),
        }
    ));
    assert!(session.active_identity().is_some());
    assert!(zone_teardown_is_fenced(&session.runtime));
    let frozen = serde_json::to_value(
        session
            .prepared_teardown_checkpoint()
            .expect("checkpoint retained after dual failure"),
    )
    .expect("serialize frozen checkpoint");

    assert_eq!(
        session.try_persist_teardown_once(),
        GatewayTeardownPersistenceOutcome::Saved
    );
    assert_eq!(client.seen_checkpoints(), vec![frozen.clone(), frozen]);
    assert!(zone_teardown_is_fenced(&session.runtime));
    session
        .release_teardown_for_resume()
        .expect("Saved checkpoint may thaw for resume");
    assert!(!zone_teardown_is_fenced(&session.runtime));
}
