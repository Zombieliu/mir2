use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::{CharacterSaveRecord, SimulationConfig, SimulationSession};

use super::{
    account_hash, atomic_write_new, atomic_write_new_with_hook, hex_encode, journal_checkpoint,
    open_exclusive_recovery_lock, read_stable_regular_file_with_hook,
    release_directory_lease_for_tests, replay_account, replay_startup, sha256_hex,
    validate_existing_directory_ancestors, AtomicWriteHookPhase, RecoveryJournal,
    ReplayCleanupAction, ValidationKind, DIRECTORY_DURABILITY_MARKER, DIRECTORY_LOCK_FILE,
    DIRECTORY_OWNER_SENTINEL, JOURNAL_DIRECTORY,
};
#[cfg(windows)]
use super::{stable_file_identity, validate_windows_file_id};

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);
const TEST_RECOVERY_MAC_KEY: [u8; 32] = [
    0x10, 0x21, 0x32, 0x43, 0x54, 0x65, 0x76, 0x87, 0x98, 0xa9, 0xba, 0xcb, 0xdc, 0xed, 0xfe, 0x0f,
    0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xf1, 0x02,
];

struct TestState {
    root: PathBuf,
    config: SimulationConfig,
}

impl TestState {
    fn new(label: &str) -> Self {
        let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "mir2-save-recovery-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("test state directory should be created");
        let config = SimulationConfig::default()
            .with_account_store_path(root.join("accounts.json"))
            .with_save_recovery_mac_key(TEST_RECOVERY_MAC_KEY)
            .expect("test recovery MAC key should be valid");
        Self { root, config }
    }

    fn journal_root(&self) -> PathBuf {
        self.root.join(JOURNAL_DIRECTORY)
    }

    fn active_checkpoint(&self) -> CharacterSaveRecord {
        let mut session = SimulationSession::new(self.config.clone());
        session
            .select_account_for_recovery("demo")
            .expect("demo recovery account should exist");
        let packets = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        assert!(packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
        session
            .active_character_checkpoint()
            .expect("demo checkpoint should exist")
    }

    fn persisted_checkpoint(&self) -> CharacterSaveRecord {
        self.config
            .account_store
            .lock()
            .expect("account store should lock")
            .accounts
            .get("demo")
            .and_then(|account| account.saves.get(&0))
            .cloned()
            .expect("demo persisted checkpoint should exist")
    }
}

impl Drop for TestState {
    fn drop(&mut self) {
        let expected_prefix = format!("mir2-save-recovery-");
        let safe = self
            .root
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(&expected_prefix));
        if safe {
            release_directory_lease_for_tests(&self.journal_root());
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn journal_files(state: &TestState) -> Vec<PathBuf> {
    fs::read_dir(state.journal_root())
        .expect("journal root should exist")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.ends_with(".journal.json") || name.ends_with(".committed.json")
                })
        })
        .collect()
}

#[test]
fn journal_is_secret_free_idempotent_and_replays_before_cleanup() {
    let state = TestState::new("replay");
    let secret = "RECOVERY-TEST-PASSWORD-MUST-NOT-LEAK";
    state
        .config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .password = secret.to_string();

    let mut checkpoint = state.active_checkpoint();
    checkpoint.gold = checkpoint.gold.saturating_add(777);
    let first = journal_checkpoint(&state.config, "demo", 0, &checkpoint)
        .expect("first journal write should be durable");
    assert!(!first.already_durable);
    let second = journal_checkpoint(&state.config, "demo", 0, &checkpoint)
        .expect("identical journal write should be idempotent");
    assert!(second.already_durable);
    assert_eq!(first.key_hash, second.key_hash);
    assert_eq!(first.payload_hash, second.payload_hash);

    let files = journal_files(&state);
    assert_eq!(files.len(), 1);
    let bytes = fs::read(&files[0]).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(!text.contains(secret));
    assert!(!text.contains("password"));
    assert!(!text.contains("token"));
    assert!(!text.contains("sessionSecret"));
    assert!(!text.contains(&hex_encode(&TEST_RECOVERY_MAC_KEY)));

    let summary = replay_startup(&state.config).expect("startup replay should succeed");
    assert_eq!(summary.replayed, 1);
    assert_eq!(summary.quarantined, 0);
    let persisted = state.persisted_checkpoint();
    assert_eq!(persisted.gold, checkpoint.gold);
    assert!(persisted.revision > checkpoint.revision);
    assert!(journal_files(&state).is_empty());
}

#[test]
fn tamper_with_recomputed_sha_is_rejected_by_mac_without_quarantining_untrusted_bytes() {
    let state = TestState::new("hash-mismatch");
    let mut checkpoint = state.active_checkpoint();
    checkpoint.gold = checkpoint.gold.saturating_add(321);
    journal_checkpoint(&state.config, "demo", 0, &checkpoint).unwrap();
    let journal_path = journal_files(&state).pop().unwrap();
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(&journal_path).unwrap()).unwrap();
    value["payload"]["checkpoint"]["gold"] = serde_json::json!(999_999u32);
    let payload_bytes = serde_json::to_vec(&value["payload"]).unwrap();
    value["payloadSha256"] = serde_json::json!(sha256_hex(&payload_bytes));
    fs::write(&journal_path, serde_json::to_vec(&value).unwrap()).unwrap();

    let error = replay_startup(&state.config).unwrap_err();
    assert!(error.contains("mac-mismatch"));
    assert_eq!(state.persisted_checkpoint().gold, 1_280);
    let quarantine = state.journal_root().join("quarantine");
    assert_eq!(
        fs::read_dir(quarantine)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.ends_with(".q")))
            .count(),
        0
    );
    let error = replay_account(&state.config, "demo").unwrap_err();
    assert!(error.contains("mac-mismatch"));
}

#[test]
fn newer_durable_state_is_quarantined_instead_of_overwritten() {
    let state = TestState::new("conflict");
    let mut journal_checkpoint_value = state.active_checkpoint();
    journal_checkpoint_value.gold = 2_000;
    journal_checkpoint(&state.config, "demo", 0, &journal_checkpoint_value).unwrap();

    let mut newer = SimulationSession::new(state.config.clone());
    newer.select_account_for_recovery("demo").unwrap();
    newer.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let mut newer_checkpoint = newer.active_character_checkpoint().unwrap();
    newer_checkpoint.gold = 3_000;
    newer
        .restore_active_character_checkpoint(&newer_checkpoint)
        .unwrap();
    newer.save_active_character().unwrap();

    let summary = replay_startup(&state.config).expect("conflict should be quarantined");
    assert_eq!(summary.replayed, 0);
    assert_eq!(summary.quarantined, 1);
    assert_eq!(state.persisted_checkpoint().gold, 3_000);
    assert!(replay_account(&state.config, "demo").is_err());
}

#[test]
fn entry_limit_rejects_without_silent_eviction() {
    let state = TestState::new("capacity");
    let journal =
        RecoveryJournal::with_limits(&state.config, 1, 8 * 1024 * 1024, 4 * 1024 * 1024).unwrap();
    journal.ensure_layout().unwrap();
    let first = state.active_checkpoint();
    journal.store("demo", 0, &first).unwrap();
    let mut second = first.clone();
    second.character.index = 1;
    let error = journal.store("demo", 1, &second).unwrap_err();
    assert!(error.contains("capacity reached"));
    assert!(error.contains("no entry was evicted"));
    assert!(journal.journal_path("demo", 0).is_file());
    assert!(!journal.journal_path("demo", 1).exists());
}

#[test]
fn record_size_limit_rejects_before_creating_a_file() {
    let state = TestState::new("record-size");
    let journal = RecoveryJournal::with_limits(&state.config, 8, 1024, 256).unwrap();
    journal.ensure_layout().unwrap();
    let checkpoint = state.active_checkpoint();
    let error = journal.store("demo", 0, &checkpoint).unwrap_err();
    assert!(error.contains("record exceeds"));
    assert!(journal_files(&state).is_empty());
}

#[test]
fn recovery_publication_source_has_no_link_then_unlink_fallback() {
    let source = include_str!("save_recovery.rs");
    assert!(!source.contains("fs::hard_link(from, to)"));
    assert!(!source.contains("fs::remove_file(from)"));
    assert!(source.contains("RENAME_NOREPLACE"));
    assert!(source.contains("renameat2("));
}

#[test]
fn atomic_publish_never_overwrites_an_existing_final_path() {
    let state = TestState::new("no-overwrite");
    let directory = state.root.join("publish");
    fs::create_dir_all(&directory).unwrap();
    let final_path = directory.join("entry.journal.json");
    let original = b"existing-writer-won";
    fs::write(&final_path, original).unwrap();

    let error = atomic_write_new(&final_path, b"late-writer", &directory).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read(&final_path).unwrap(), original);
}

#[test]
fn atomic_publish_rejects_temp_replacement_and_preserves_both_files() {
    let state = TestState::new("temp-replacement");
    let directory = state.root.join("publish-temp-replacement");
    fs::create_dir_all(&directory).unwrap();
    let final_path = directory.join("entry.journal.json");
    let authentic_backup = directory.join("authentic-written-handle.json");
    let replacement = b"replacement-at-temp-path";
    let mut replaced_temp = None;

    let error = atomic_write_new_with_hook(
        &final_path,
        b"authenticated-original",
        &directory,
        |phase, temp, _final_path| {
            if phase == AtomicWriteHookPhase::TempSynced {
                fs::rename(temp, &authentic_backup)
                    .expect("test must replace the temp path after its handle is synced");
                fs::write(temp, replacement).unwrap();
                replaced_temp = Some(temp.to_path_buf());
            }
        },
    )
    .unwrap_err();

    assert!(error.to_string().contains("temp no longer identifies"));
    assert!(!final_path.exists());
    assert_eq!(
        fs::read(&authentic_backup).unwrap(),
        b"authenticated-original"
    );
    let replaced_temp = replaced_temp.expect("temp replacement hook must run");
    assert_eq!(fs::read(replaced_temp).unwrap(), replacement);
}

#[test]
fn authenticated_final_replacement_is_rejected_before_store_receipt() {
    let state = TestState::new("authenticated-final-replacement");
    let journal = RecoveryJournal::from_config(&state.config)
        .unwrap()
        .unwrap();
    journal.ensure_layout().unwrap();
    let checkpoint = state.active_checkpoint();
    let authentic_backup = state.root.join("authenticated-published-backup.json");
    let mut replaced = false;

    let error = journal
        .store_with_post_publish_hook("demo", 0, &checkpoint, |path| {
            fs::rename(path, &authentic_backup)
                .expect("test must replace the final path after atomic publication");
            fs::copy(&authentic_backup, path)
                .expect("replacement remains authenticated but has a different object identity");
            replaced = true;
        })
        .unwrap_err();

    assert!(replaced);
    assert!(error.contains("no longer identifies the written file"));
    let final_path = journal.journal_path("demo", 0);
    assert!(final_path.is_file(), "replacement evidence must remain");
    assert!(
        authentic_backup.is_file(),
        "written object evidence must remain"
    );
    assert_eq!(
        fs::read(&final_path).unwrap(),
        fs::read(&authentic_backup).unwrap()
    );
    assert!(journal.read_valid_entry(&final_path).is_ok());
}

#[test]
fn directory_durability_marker_and_active_temp_are_ignored_by_journal_scans() {
    let state = TestState::new("durability-artifacts");
    let journal = RecoveryJournal::from_config(&state.config)
        .unwrap()
        .unwrap();
    journal.ensure_layout().unwrap();
    let marker = state.journal_root().join(DIRECTORY_DURABILITY_MARKER);
    let active_temp = state
        .journal_root()
        .join(".mir2-directory-durability.4242.7.tmp");
    fs::write(&marker, b"marker").unwrap();
    fs::write(&active_temp, b"active-marker-publish").unwrap();

    assert_eq!(journal.capacity_usage().unwrap(), (0, 0));
    let summary = replay_startup(&state.config).unwrap();
    assert_eq!(summary, Default::default());
    assert!(marker.exists());
    assert!(active_temp.exists());
}

#[test]
fn postgres_style_config_uses_explicit_recovery_directory() {
    let state = TestState::new("postgres-explicit");
    let checkpoint = state.active_checkpoint();
    let explicit = state.root.join("postgres-recovery");
    let mut config = SimulationConfig::default()
        .with_save_recovery_dir(&explicit)
        .with_save_recovery_mac_key(TEST_RECOVERY_MAC_KEY)
        .unwrap();
    config.account_store_path = None;
    config.account_store_database_url = Some("postgres://not-opened.invalid/mir2".to_string());

    // This test only validates journal placement; it does not imply that missing
    // authoritative account-store writes are safe to ignore.
    journal_checkpoint(&config, "demo", 0, &checkpoint).unwrap();
    assert!(explicit.is_dir());
    assert!(fs::read_dir(&explicit)
        .unwrap()
        .filter_map(Result::ok)
        .any(|entry| entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.ends_with(".journal.json"))));
    release_directory_lease_for_tests(&explicit);
}

#[test]
fn postgres_style_startup_without_recovery_directory_is_rejected() {
    let mut config = SimulationConfig::default();
    config.account_store_path = None;
    config.account_store_database_url = Some("postgres://not-opened.invalid/mir2".to_string());

    let error = replay_startup(&config).unwrap_err();
    assert!(error.contains("MIR2_SAVE_RECOVERY_DIR is required"));
}

#[test]
fn recovery_root_below_a_symlink_or_reparse_ancestor_is_rejected() {
    let state = TestState::new("root-ancestor-link");
    let real_root = state.root.join("real-recovery-root");
    let linked_root = state.root.join("linked-recovery-root");
    fs::create_dir_all(&real_root).unwrap();
    create_directory_symlink(&real_root, &linked_root)
        .expect("test platform must support a directory symlink/reparse-point negative case");
    let configured_root = linked_root.join("nested");

    let error = validate_existing_directory_ancestors(&configured_root).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);

    let mut config = SimulationConfig::default()
        .with_save_recovery_dir(&configured_root)
        .with_save_recovery_mac_key(TEST_RECOVERY_MAC_KEY)
        .unwrap();
    config.account_store_path = None;
    config.account_store_database_url = Some("postgres://not-opened.invalid/mir2".to_string());
    let startup_error = replay_startup(&config).unwrap_err();
    assert!(startup_error.contains("symbolic-link") || startup_error.contains("reparse-point"));
    assert!(!real_root.join("nested").exists());
}

#[test]
fn symlink_or_reparse_journal_entry_is_rejected_without_following() {
    let state = TestState::new("symlink");
    let checkpoint = state.active_checkpoint();
    journal_checkpoint(&state.config, "demo", 0, &checkpoint).unwrap();
    let path = journal_files(&state).pop().unwrap();
    let target = state.root.join("journal-target.json");
    fs::rename(&path, &target).unwrap();
    create_file_symlink(&target, &path).expect("test platform should create a file symlink");
    let journal = RecoveryJournal::from_config(&state.config)
        .unwrap()
        .unwrap();

    let error = journal.read_valid_entry(&path).unwrap_err();
    assert!(matches!(
        error.kind,
        ValidationKind::Corrupt | ValidationKind::IdentityMismatch
    ));
}

#[test]
fn path_swap_during_open_is_rejected_by_stable_identity_check() {
    let state = TestState::new("swap");
    let path = state.root.join("stable-entry.json");
    let replacement = state.root.join("stable-entry-replacement.json");
    let backup = state.root.join("stable-entry-backup.json");
    fs::write(&path, b"original-stable-bytes").unwrap();
    fs::write(&replacement, b"replacement-new-bytes").unwrap();

    let error = read_stable_regular_file_with_hook(&path, 1024, || {
        fs::rename(&path, &backup).expect("open handle should permit deterministic swap test");
        fs::rename(&replacement, &path).unwrap();
    })
    .unwrap_err();
    assert_eq!(error.kind, ValidationKind::IdentityMismatch);
}

#[cfg(windows)]
#[test]
fn windows_full_file_id_detects_path_replacement_while_original_handle_is_held() {
    let zero_error = validate_windows_file_id([0u8; 16]).unwrap_err();
    assert_eq!(zero_error.kind(), std::io::ErrorKind::PermissionDenied);

    let state = TestState::new("windows-file-id-replacement");
    let path = state.root.join("identity-target.json");
    let backup = state.root.join("identity-original.json");
    fs::write(&path, b"same-sized-original").unwrap();
    let held = fs::File::open(&path).unwrap();
    let original_identity = stable_file_identity(&held).unwrap();

    fs::rename(&path, &backup).unwrap();
    fs::write(&path, b"same-sized-replaced").unwrap();
    let replacement = fs::File::open(&path).unwrap();
    let replacement_identity = stable_file_identity(&replacement).unwrap();

    assert!(original_identity.same_object(stable_file_identity(&held).unwrap()));
    assert!(!original_identity.same_object(replacement_identity));
}

#[test]
fn recovery_enabled_without_dedicated_mac_key_is_rejected_before_directory_adoption() {
    let state = TestState::new("missing-mac-key");
    let root = state.root.join("unsigned-recovery");
    let config = SimulationConfig::default().with_save_recovery_dir(&root);

    let error = replay_startup(&config).unwrap_err();

    assert!(error.contains("no dedicated 32-byte MAC key"));
    assert!(!root.join(DIRECTORY_OWNER_SENTINEL).exists());
}

#[test]
fn unowned_nonempty_directory_fails_closed_without_moving_unrelated_content() {
    let state = TestState::new("unowned-shared-dir");
    let root = state.root.join("shared-data");
    fs::create_dir_all(&root).unwrap();
    let unrelated = root.join("family-photo.txt");
    fs::write(&unrelated, b"must remain exactly here").unwrap();
    let config = SimulationConfig::default()
        .with_save_recovery_dir(&root)
        .with_save_recovery_mac_key(TEST_RECOVERY_MAC_KEY)
        .unwrap();

    let error = replay_startup(&config).unwrap_err();

    assert!(error.contains("refusing adoption"));
    assert_eq!(fs::read(&unrelated).unwrap(), b"must remain exactly here");
    assert!(!root.join("quarantine").exists());
}

#[test]
fn recovery_directory_cannot_overlap_account_store_parent() {
    let state = TestState::new("overlap");
    let config = state
        .config
        .clone()
        .with_save_recovery_dir(state.root.clone());

    let error = replay_startup(&config).unwrap_err();

    assert!(error.contains("must be dedicated"));
    assert!(!state.root.join(DIRECTORY_OWNER_SENTINEL).exists());
}

#[test]
fn authenticated_filename_identity_mismatch_blocks_payload_and_claimed_accounts() {
    let state = TestState::new("dual-attribution");
    let checkpoint = state.active_checkpoint();
    journal_checkpoint(&state.config, "demo", 0, &checkpoint).unwrap();
    let original = journal_files(&state).pop().unwrap();
    let original_name = original.file_name().unwrap().to_str().unwrap();
    let claimed_account = "claimed-other-account";
    let claimed_name = format!("{}{}", account_hash(claimed_account), &original_name[64..]);
    let claimed_path = state.journal_root().join(claimed_name);
    fs::rename(&original, &claimed_path).unwrap();

    let summary = replay_startup(&state.config).unwrap();

    assert_eq!(summary.quarantined, 1);
    assert!(replay_account(&state.config, "demo").is_err());
    assert!(replay_account(&state.config, claimed_account).is_err());
}

#[test]
fn publication_temp_only_is_promoted_idempotently() {
    let state = TestState::new("temp-only");
    let journal = RecoveryJournal::from_config(&state.config)
        .unwrap()
        .unwrap();
    journal.ensure_layout().unwrap();
    journal
        .store("demo", 0, &state.active_checkpoint())
        .unwrap();
    let final_path = journal.journal_path("demo", 0);
    let temp_path = publication_temp_path(&final_path, 501, 1);
    fs::rename(&final_path, &temp_path).unwrap();

    journal.reconcile_publication_temps().unwrap();

    assert!(final_path.is_file());
    assert!(!temp_path.exists());
    journal.reconcile_publication_temps().unwrap();
}

#[test]
fn publication_final_and_same_hard_link_temp_fails_closed_and_retains_evidence() {
    let state = TestState::new("temp-same");
    let journal = RecoveryJournal::from_config(&state.config)
        .unwrap()
        .unwrap();
    journal.ensure_layout().unwrap();
    journal
        .store("demo", 0, &state.active_checkpoint())
        .unwrap();
    let final_path = journal.journal_path("demo", 0);
    let original = fs::read(&final_path).unwrap();
    let temp_path = publication_temp_path(&final_path, 502, 2);
    fs::hard_link(&final_path, &temp_path).unwrap();

    let error = journal.reconcile_publication_temps().unwrap_err();

    assert!(error.contains("automatic path cleanup is disabled"));
    assert_eq!(fs::read(&final_path).unwrap(), original);
    assert_eq!(fs::read(&temp_path).unwrap(), original);
}

#[test]
fn publication_final_and_different_temp_fails_closed_without_cleanup() {
    let state = TestState::new("temp-different");
    let journal = RecoveryJournal::from_config(&state.config)
        .unwrap()
        .unwrap();
    journal.ensure_layout().unwrap();
    journal
        .store("demo", 0, &state.active_checkpoint())
        .unwrap();
    let final_path = journal.journal_path("demo", 0);
    let temp_path = publication_temp_path(&final_path, 503, 3);
    fs::copy(&final_path, &temp_path).unwrap();

    let error = journal.reconcile_publication_temps().unwrap_err();

    assert!(error.contains("different temp"));
    assert!(final_path.exists());
    assert!(temp_path.exists());
}

#[cfg(unix)]
#[test]
fn unix_root_lock_survives_lock_path_replacement_across_processes() {
    if let Some(root) = std::env::var_os("MIR2_TEST_RECOVERY_CHILD_ROOT") {
        let mut config = SimulationConfig::default()
            .with_save_recovery_dir(PathBuf::from(root))
            .with_save_recovery_mac_key(TEST_RECOVERY_MAC_KEY)
            .unwrap();
        config.account_store_path = None;
        config.account_store_database_url = Some("postgres://not-opened.invalid/mir2".to_string());
        let error = RecoveryJournal::from_config(&config).unwrap_err();
        assert!(
            error.contains("already owned by another manager"),
            "child must be rejected by the held root-directory lock: {error}"
        );
        return;
    }

    let state = TestState::new("unix-root-cross-process-lock");
    let journal = RecoveryJournal::from_config(&state.config)
        .unwrap()
        .unwrap();
    journal.ensure_layout().unwrap();
    let lock_path = state.journal_root().join(DIRECTORY_LOCK_FILE);

    fs::remove_file(&lock_path).expect("Unix permits unlinking the held advisory-lock path");
    fs::write(&lock_path, b"replacement-lock-object").unwrap();
    let trust_error = journal.verify_directory_trust().unwrap_err();
    assert!(trust_error.contains("no longer identifies the held lock"));

    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("unix_root_lock_survives_lock_path_replacement_across_processes")
        .arg("--nocapture")
        .env("MIR2_TEST_RECOVERY_CHILD_ROOT", state.journal_root())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("child test process should launch");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if child.try_wait().expect("query child test status").is_some() {
            break;
        }
        if std::time::Instant::now() >= deadline {
            child.kill().expect("kill timed-out child lock probe");
            let _ = child.wait();
            panic!("child lock probe exceeded 10-second timeout");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let output = child
        .wait_with_output()
        .expect("collect completed child lock probe output");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "child lock probe failed: stdout={stdout} stderr={stderr}"
    );
    assert!(
        stdout.contains("1 passed"),
        "child process must execute the lock probe: {stdout}"
    );
}

#[test]
fn second_independent_manager_cannot_lock_same_recovery_directory() {
    let state = TestState::new("exclusive-lock");
    let journal = RecoveryJournal::from_config(&state.config)
        .unwrap()
        .unwrap();
    journal.ensure_layout().unwrap();

    let second = open_exclusive_recovery_lock(&state.journal_root().join(DIRECTORY_LOCK_FILE));

    assert!(second.is_err());
}

#[test]
fn recovery_root_replacement_is_blocked_or_detected_by_stable_identity() {
    let state = TestState::new("root-replacement");
    let journal = RecoveryJournal::from_config(&state.config)
        .unwrap()
        .unwrap();
    journal.ensure_layout().unwrap();
    let original_root = state.journal_root();
    let displaced_root = state.root.join("displaced-recovery-root");

    match fs::rename(&original_root, &displaced_root) {
        Err(_) => assert!(journal.verify_directory_trust().is_ok()),
        Ok(()) => {
            fs::create_dir(&original_root).unwrap();
            fs::copy(
                displaced_root.join(DIRECTORY_OWNER_SENTINEL),
                original_root.join(DIRECTORY_OWNER_SENTINEL),
            )
            .unwrap();
            let error = journal.verify_directory_trust().unwrap_err();
            assert!(error.contains("identity changed") || error.contains("lock file"));
        }
    }
}

#[test]
fn replacement_after_authenticated_read_blocks_commit_cleanup() {
    let state = TestState::new("commit-path-replacement");
    let journal = RecoveryJournal::from_config(&state.config)
        .unwrap()
        .unwrap();
    journal.ensure_layout().unwrap();
    let mut checkpoint = state.active_checkpoint();
    checkpoint.gold = checkpoint.gold.saturating_add(321);
    journal.store("demo", 0, &checkpoint).unwrap();
    let journal_path = journal.journal_path("demo", 0);
    let authenticated_backup = state.root.join("authenticated-commit-backup.json");
    let mut replaced = false;

    let error = journal
        .replay_matching_with_hook(&state.config, None, false, |path, _envelope, action| {
            if action == ReplayCleanupAction::Commit && !replaced {
                fs::rename(path, &authenticated_backup).unwrap();
                fs::copy(&authenticated_backup, path).unwrap();
                replaced = true;
            }
        })
        .unwrap_err();

    assert!(replaced);
    assert!(error.contains("changed before commit") || error.contains("authenticated file"));
    assert!(journal_path.is_file(), "replacement must not be removed");
    assert!(
        authenticated_backup.is_file(),
        "authenticated entry must remain available"
    );
    assert!(!journal.committed_path("demo", 0).exists());
}

#[test]
fn replacement_after_authenticated_read_blocks_quarantine_move() {
    let state = TestState::new("quarantine-path-replacement");
    let journal = RecoveryJournal::from_config(&state.config)
        .unwrap()
        .unwrap();
    journal.ensure_layout().unwrap();
    let mut checkpoint = state.active_checkpoint();
    checkpoint.gold = 2_000;
    journal.store("demo", 0, &checkpoint).unwrap();
    let journal_path = journal.journal_path("demo", 0);

    let mut newer = SimulationSession::new(state.config.clone());
    newer.select_account_for_recovery("demo").unwrap();
    newer.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let mut newer_checkpoint = newer.active_character_checkpoint().unwrap();
    newer_checkpoint.gold = 3_000;
    newer
        .restore_active_character_checkpoint(&newer_checkpoint)
        .unwrap();
    newer.save_active_character().unwrap();

    let authenticated_backup = state.root.join("authenticated-quarantine-backup.json");
    let mut replaced = false;
    let error = journal
        .replay_matching_with_hook(&state.config, None, false, |path, _envelope, action| {
            if action == ReplayCleanupAction::Quarantine && !replaced {
                fs::rename(path, &authenticated_backup).unwrap();
                fs::copy(&authenticated_backup, path).unwrap();
                replaced = true;
            }
        })
        .unwrap_err();

    assert!(replaced);
    assert!(error.contains("changed before quarantine") || error.contains("authenticated file"));
    assert!(journal_path.is_file(), "replacement must not be moved");
    assert!(
        authenticated_backup.is_file(),
        "authenticated entry must remain available"
    );
    assert_eq!(state.persisted_checkpoint().gold, 3_000);
    let quarantine_count = fs::read_dir(&journal.quarantine)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "q")
        })
        .count();
    assert_eq!(quarantine_count, 0);
}

fn publication_temp_path(final_path: &std::path::Path, pid: u32, sequence: u64) -> PathBuf {
    let name = final_path.file_name().unwrap().to_str().unwrap();
    final_path.with_file_name(format!("{name}.{pid}.{sequence}.tmp"))
}

#[cfg(windows)]
fn create_file_symlink(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[cfg(windows)]
fn create_directory_symlink(
    target: &std::path::Path,
    link: &std::path::Path,
) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}

#[cfg(unix)]
fn create_file_symlink(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(unix)]
fn create_directory_symlink(
    target: &std::path::Path,
    link: &std::path::Path,
) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}
