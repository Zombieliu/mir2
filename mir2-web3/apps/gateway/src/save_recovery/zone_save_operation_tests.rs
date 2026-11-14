//! Pure receipt reconciliation and local journal tests; no PostgreSQL success is
//! simulated or claimed. Remote journals must never use legacy source replay.
use super::*;
use mir2_simulation::{ZoneSaveFence, ZoneSaveOperation};

const TEST_MAC_KEY: [u8; 32] = [
    0x10, 0x21, 0x32, 0x43, 0x54, 0x65, 0x76, 0x87, 0x98, 0xa9, 0xba, 0xcb, 0xdc, 0xed, 0xfe, 0x0f,
    0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xf1, 0x02,
];

fn binding(operation: &str) -> ZoneSaveOperationBinding {
    ZoneSaveOperationBinding::new(
        operation.into(),
        "rpc-remote-recovery".into(),
        ZoneSaveFence::new("remote-recovery-zone".into(), "owner-a".into(), 17).unwrap(),
    )
    .unwrap()
}

fn checkpoint(config: &SimulationConfig) -> CharacterSaveRecord {
    config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone()
}

fn remote_payload(
    checkpoint: &CharacterSaveRecord,
    binding: &ZoneSaveOperationBinding,
) -> RecoveryPayload {
    let mut payload = RecoveryPayload::new("demo", 0, checkpoint).unwrap();
    payload.remote_zone_save_operation =
        Some(RemoteRecoveryOperation::from_binding(binding).unwrap());
    payload
}

// Checked wire data lets pure reconciliation exercise each receipt state. Only
// a repository query in production can establish that a receipt is durable.
fn receipt(
    binding: &ZoneSaveOperationBinding,
    frozen: &CharacterSaveRecord,
    status: &str,
) -> ZoneSaveOperationReceipt {
    let mut expected = frozen.clone();
    expected.revision += 1;
    let operation = ZoneSaveOperation::new(
        binding.clone(),
        "demo".into(),
        frozen.clone(),
        expected.clone(),
        Some(42),
    )
    .unwrap();
    let frozen_json = serde_json::to_value(frozen).unwrap();
    let expected_json = serde_json::to_value(&expected).unwrap();
    let committed = status != "Prepared";
    ZoneSaveOperationReceipt::from_json_checked(serde_json::json!({
        "operation": {
            "binding": binding.to_json().unwrap(),
            "accountId": "demo", "frozenCheckpoint": frozen_json,
            "expectedSavedCheckpoint": expected_json,
            "checkpointDigest": operation.checkpoint_digest(),
            "expectedDigest": sha256_hex(&serde_json::to_vec(&serde_json::to_value(&expected).unwrap()).unwrap()),
            "lastAccessBinaryDatetime": 42,
        },
        "status": status,
        "savedCheckpoint": if committed { Some(serde_json::to_value(&expected).unwrap()) } else { None },
        "accountStoreVersion": if committed { Some(2) } else { None },
        "characterSaveVersion": if committed { Some(3) } else { None },
    })).unwrap()
}

fn envelope(payload: RecoveryPayload) -> RecoveryEnvelope {
    let bytes = canonical_payload_bytes(&payload).unwrap();
    RecoveryEnvelope {
        journal_schema: JOURNAL_SCHEMA.into(),
        journal_version: JOURNAL_VERSION,
        hash_algorithm: HASH_ALGORITHM.into(),
        payload_sha256: sha256_hex(&bytes),
        mac_algorithm: MAC_ALGORITHM.into(),
        payload_hmac_sha256: hmac_sha256_hex(&TEST_MAC_KEY, b"mir2-recovery-payload-v1\0", &bytes)
            .unwrap(),
        payload,
    }
}

#[test]
fn remote_recovery_legacy_v1_payload_retains_exact_canonical_hash_and_mac_bytes() {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct LegacyPayload<'a> {
        checkpoint_schema_version: u16,
        account_id: &'a str,
        character_index: i32,
        authoritative_transform: RecoveryTransform,
        checkpoint: &'a CharacterSaveRecord,
    }
    let frozen = checkpoint(&SimulationConfig::default());
    let legacy = LegacyPayload {
        checkpoint_schema_version: CHECKPOINT_SCHEMA_VERSION,
        account_id: "demo",
        character_index: 0,
        authoritative_transform: RecoveryTransform {
            map_file_name: frozen.map_file_name.clone(),
            position: frozen.position.clone(),
            direction: frozen.direction,
        },
        checkpoint: &frozen,
    };
    let old_bytes = serde_json::to_vec(&legacy).unwrap();
    let payload: RecoveryPayload = serde_json::from_slice(&old_bytes).unwrap();
    assert!(payload.remote_zone_save_operation.is_none());
    let new_bytes = canonical_payload_bytes(&payload).unwrap();
    assert_eq!(new_bytes, old_bytes);
    assert_eq!(sha256_hex(&new_bytes), sha256_hex(&old_bytes));
    assert_eq!(
        hmac_sha256_hex(&TEST_MAC_KEY, b"mir2-recovery-payload-v1\0", &new_bytes).unwrap(),
        hmac_sha256_hex(&TEST_MAC_KEY, b"mir2-recovery-payload-v1\0", &old_bytes).unwrap()
    );
}

#[test]
fn remote_recovery_binding_extension_checks_version_shape_and_capability_tuple() {
    let remote = RemoteRecoveryOperation::from_binding(&binding("binding-checks")).unwrap();
    assert_eq!(remote.checked_binding().unwrap(), binding("binding-checks"));
    for alteration in 0..5 {
        let mut invalid = remote.clone();
        match alteration {
            0 => invalid.version += 1,
            1 => invalid.binding["operationId"] = serde_json::json!(""),
            2 => invalid.binding["rpcSessionId"] = serde_json::json!("rpc\n"),
            3 => invalid.binding["fencingToken"] = serde_json::json!(0),
            _ => invalid.binding["unexpectedAuthority"] = serde_json::json!(true),
        }
        assert!(invalid.checked_binding().is_err());
    }
}

#[test]
fn remote_recovery_pending_absent_or_unavailable_receipt_is_deferred_without_source_work() {
    let frozen = checkpoint(&SimulationConfig::default());
    let binding = binding("pending");
    let payload = remote_payload(&frozen, &binding);
    for result in [
        Ok(Some(receipt(&binding, &frozen, "Prepared"))),
        Ok(None),
        Err("store unavailable".into()),
    ] {
        let error = replay_remote_checkpoint_with_query(&payload, &binding, |received| {
            assert_eq!(received, &binding);
            result
        })
        .unwrap_err();
        assert!(!is_replay_conflict(&error));
        assert!(error.contains("journal retained"));
        assert!(error.contains("login blocked"));
    }
}

#[test]
fn remote_recovery_terminal_receipt_requires_exact_binding_identity_and_frozen_image() {
    let frozen = checkpoint(&SimulationConfig::default());
    let binding = binding("terminal");
    for status in ["Committed", "Released"] {
        let payload = remote_payload(&frozen, &binding);
        assert_eq!(
            replay_remote_checkpoint_with_query(&payload, &binding, |_| Ok(Some(receipt(
                &binding, &frozen, status
            ))))
            .unwrap(),
            ReplayDecision::AlreadyCommitted
        );
        for alteration in 0..4 {
            let mut altered = payload.clone();
            match alteration {
                0 => altered.account_id.push('x'),
                1 => altered.character_index += 1,
                2 => altered.checkpoint.gold += 1,
                _ => altered.checkpoint.revision += 1,
            }
            assert!(is_replay_conflict(
                &replay_remote_checkpoint_with_query(&altered, &binding, |_| Ok(Some(receipt(
                    &binding, &frozen, status
                ))))
                .unwrap_err()
            ));
        }
        let foreign = ZoneSaveOperationBinding::new(
            "foreign-op".into(),
            binding.rpc_session_id().into(),
            binding.fence().clone(),
        )
        .unwrap();
        assert!(is_replay_conflict(
            &replay_remote_checkpoint_with_query(&payload, &binding, |_| Ok(Some(receipt(
                &foreign, &frozen, status
            ))))
            .unwrap_err()
        ));
    }
}

struct JournalFixture {
    root: PathBuf,
    config: SimulationConfig,
}
impl JournalFixture {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "mir2-remote-recovery-{label}-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let config = SimulationConfig::default()
            .with_account_store_path(root.join("accounts.json"))
            .with_save_recovery_mac_key(TEST_MAC_KEY)
            .unwrap();
        config.save_account_store().unwrap();
        Self { root, config }
    }
    fn journal(&self) -> RecoveryJournal {
        RecoveryJournal::from_config(&self.config).unwrap().unwrap()
    }
}
impl Drop for JournalFixture {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir();
        if self.root.parent() == Some(temp_root.as_path())
            && self
                .root
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("mir2-remote-recovery-"))
        {
            release_directory_lease_for_tests(&self.root.join(JOURNAL_DIRECTORY));
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

#[test]
fn remote_recovery_journal_never_falls_back_to_legacy_file_save_and_blocks_login() {
    let fixture = JournalFixture::new("no-legacy");
    let mut frozen = checkpoint(&fixture.config);
    frozen.gold += 700;
    let binding = binding("no-legacy");
    let store_before =
        serde_json::to_value(&*fixture.config.account_store.lock().unwrap()).unwrap();
    let file_before = fs::read(fixture.root.join("accounts.json")).unwrap();
    let first = journal_remote_checkpoint(&fixture.config, "demo", 0, &frozen, &binding).unwrap();
    assert!(
        journal_remote_checkpoint(&fixture.config, "demo", 0, &frozen, &binding)
            .unwrap()
            .already_durable
    );
    let path = fixture.journal().journal_path("demo", 0);
    let journal_before = fs::read(&path).unwrap();
    assert!(!first.already_durable);
    let error = replay_account(&fixture.config, "demo").unwrap_err();
    assert!(error.contains("journal retained"));
    assert!(error.contains("login blocked"));
    assert_eq!(fs::read(&path).unwrap(), journal_before);
    assert_eq!(
        fs::read(fixture.root.join("accounts.json")).unwrap(),
        file_before
    );
    assert_eq!(
        serde_json::to_value(&*fixture.config.account_store.lock().unwrap()).unwrap(),
        store_before
    );
    assert!(journal_checkpoint(&fixture.config, "demo", 0, &frozen)
        .unwrap_err()
        .contains("key conflict"));
}

#[test]
fn remote_recovery_binding_tamper_remains_mac_rejected_even_with_recomputed_hash() {
    let fixture = JournalFixture::new("mac-binding");
    let frozen = checkpoint(&fixture.config);
    journal_remote_checkpoint(&fixture.config, "demo", 0, &frozen, &binding("mac-binding"))
        .unwrap();
    let journal = fixture.journal();
    let path = journal.journal_path("demo", 0);
    let mut entry: RecoveryEnvelope = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    entry
        .payload
        .remote_zone_save_operation
        .as_mut()
        .unwrap()
        .binding["ownerId"] = serde_json::json!("owner-b");
    entry.payload_sha256 = sha256_hex(&canonical_payload_bytes(&entry.payload).unwrap());
    fs::write(&path, serde_json::to_vec(&entry).unwrap()).unwrap();
    assert_eq!(
        journal.read_authenticated_entry(&path).unwrap_err().kind,
        ValidationKind::MacMismatch
    );
}

#[test]
fn remote_recovery_envelope_dispatches_remote_before_any_legacy_session_restore() {
    let config = SimulationConfig::default();
    let mut frozen = checkpoint(&config);
    frozen.gold += 909;
    let payload = remote_payload(&frozen, &binding("dispatch"));
    let before = serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap();
    let error = replay_envelope(&config, &envelope(payload)).unwrap_err();
    assert!(error.contains("remote recovery receipt unavailable"));
    assert_eq!(
        serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap(),
        before
    );
}
