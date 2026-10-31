use super::*;
use crate::zone_rpc::{ZoneBaseSnapshotCompression, ZoneReplicationCoverage};
use flate2::{Compression, GzBuilder};
use std::io::Write;

pub(in crate::economy::player_drop) fn fixture_heads(
    zone: &str,
    host: &str,
) -> PlayerDropRecoveryHostHeads {
    let head = ZoneReplicationHead {
        version: ZONE_REPLICATION_HEAD_VERSION,
        zone_id: zone.into(),
        build_id: "recovery-proof-tests".into(),
        mutation_coverage: ZoneReplicationCoverage::CommandJournal,
        promotion_ready: false,
        base_snapshot_id: Some("previous-base".into()),
        base_sequence: 0,
        oldest_available_sequence: 0,
        entry_count: 0,
        next_sequence: 42,
        last_sequence: Some(41),
        latest_digest: "a".repeat(64),
    };
    let payload = serde_json::to_vec(&serde_json::json!({"version":head.version,"zoneId":zone,
        "zoneStateBytes":[1,2,3],"sessions":[]}))
    .unwrap();
    let mut encoder = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::new(6));
    encoder.write_all(&payload).unwrap();
    let mut base = ZoneBaseSnapshot {
        version: head.version,
        snapshot_id: String::new(),
        zone_id: zone.into(),
        build_id: head.build_id.clone(),
        mutation_coverage: head.mutation_coverage,
        apply_ready: true,
        base_sequence: head.next_sequence,
        latest_digest: head.latest_digest.clone(),
        created_at_ms: now_ms(),
        session_count: 0,
        compression: ZoneBaseSnapshotCompression::Gzip,
        uncompressed_bytes: payload.len(),
        payload: encoder.finish().unwrap(),
    };
    let bytes = serde_json::to_vec(&(
        base.version,
        &base.zone_id,
        &base.build_id,
        base.mutation_coverage,
        base.apply_ready,
        base.base_sequence,
        &base.latest_digest,
        base.created_at_ms,
        base.session_count,
        base.compression,
        base.uncompressed_bytes,
        &base.payload,
    ))
    .unwrap();
    base.snapshot_id = hash_bytes(b"obelisk.mir2.zone-base-snapshot.v5\0", &bytes);
    PlayerDropRecoveryHostHeads::new(host.into(), head, base).unwrap()
}

#[test]
fn recovery_base_checksum_and_complete_pre_head_are_bound() {
    let heads = fixture_heads("zone-1", "owner-b");
    let mut corrupt = heads.base_snapshot().unwrap();
    corrupt.payload[0] ^= 1;
    assert!(PlayerDropRecoveryHostHeads::new(
        "owner-b".into(),
        heads.pre_recovery_head().clone(),
        corrupt
    )
    .is_err());
    let mut head = heads.pre_recovery_head().clone();
    head.next_sequence += 1;
    assert!(PlayerDropRecoveryHostHeads::new(
        "owner-b".into(),
        head,
        heads.base_snapshot().unwrap()
    )
    .is_err());
    let mut changed = heads.pre_recovery_head().clone();
    changed.oldest_available_sequence += 1;
    let other =
        PlayerDropRecoveryHostHeads::new("owner-b".into(), changed, heads.base_snapshot().unwrap())
            .unwrap();
    assert_ne!(
        other.metadata.pre_recovery_head_digest,
        heads.metadata.pre_recovery_head_digest
    );
    assert_eq!(other.base_bytes, heads.base_bytes);
}

#[test]
fn recovery_receipt_keeps_exact_base_bytes_and_rejects_receipt_or_base_tampering() {
    let heads = fixture_heads("zone-1", "owner-b");
    let data = ReceiptData {
        version: 1,
        source_namespace: "public".into(),
        zone_id: "zone-1".into(),
        recovery_id: "recovery-1".into(),
        owner_id: "owner-b".into(),
        owner_epoch: 8,
        source_digest: "b".repeat(64),
        source_accounts: vec![],
        source_characters: vec![],
        operation_set_digest: "c".repeat(64),
        operation_count: 0,
        admission_set_digest: None,
        admission_count: None,
        max_operations: 256,
        state_counts: BTreeMap::new(),
        observation_sequence: 42,
        host: heads.metadata.clone(),
        observed_at_ms: 1000,
        valid_until_ms: 5000,
        requested_lifetime_ms: 4000,
    };
    let mut receipt = PlayerDropRecoveryReceipt {
        receipt_digest: hash_value(b"mir2.player-drop-recovery-receipt.v1\0", &data).unwrap(),
        data,
        base_bytes: heads.base_bytes.clone(),
        connection_binding_digest: connection_binding_digest("fixture"),
    };
    assert_eq!(
        receipt.host_heads().unwrap().base_snapshot_bytes(),
        heads.base_snapshot_bytes()
    );
    receipt.data.host.post_world_digest = "d".repeat(64);
    assert!(receipt.host_heads().is_err());
    receipt.data.host = heads.metadata;
    receipt.base_bytes.push(b' ');
    assert!(
        receipt.host_heads().is_err(),
        "even valid JSON whitespace changes the exact retained bytes"
    );
}

#[test]
fn recovery_empty_set_is_not_a_cross_zone_or_cross_namespace_proof() {
    let one = make_set("public".into(), "fixture", "zone-1", vec![], vec![], 256).unwrap();
    let zone = make_set("public".into(), "fixture", "zone-2", vec![], vec![], 256).unwrap();
    let namespace = make_set("source_b".into(), "fixture", "zone-1", vec![], vec![], 256).unwrap();
    assert_ne!(one.operation_set_digest, zone.operation_set_digest);
    assert_ne!(one.operation_set_digest, namespace.operation_set_digest);
    assert_ne!(one.admission_set_digest, zone.admission_set_digest);
    assert_ne!(one.admission_set_digest, namespace.admission_set_digest);
    assert_eq!(one.admission_count(), 0);
    assert!(one.required_characters().is_empty());
}

fn fixture_operation_record(status: PlayerDropOperationStatus) -> PlayerDropOperationRecord {
    PlayerDropOperationRecord {
        source_namespace: "public".into(),
        connection_binding_digest: connection_binding_digest("fixture"),
        operation: super::super::tests::fixture(),
        status,
        receipt: None,
        terminal_event_id: None,
        terminal_recipient: None,
        observation: None,
        abort_audit: None,
    }
}

#[test]
fn recovery_linked_admission_requires_exact_operation_and_preserves_legacy_operations() {
    let record = fixture_operation_record(PlayerDropOperationStatus::Prepared);
    let binding = record.operation().binding();
    let digest = record.operation().operation_digest();
    assert!(linked_recovery_operation(
        binding,
        PlayerDropAdmissionStatus::Linked,
        Some(digest),
        None
    )
    .is_err());
    assert!(linked_recovery_operation(
        binding,
        PlayerDropAdmissionStatus::Linked,
        Some(&"0".repeat(64)),
        Some(&record)
    )
    .is_err());
    assert!(linked_recovery_operation(
        binding,
        PlayerDropAdmissionStatus::Linked,
        Some(digest),
        Some(&record)
    )
    .unwrap()
    .is_some());
    assert!(linked_recovery_operation(
        binding,
        PlayerDropAdmissionStatus::Admitted,
        None,
        Some(&record)
    )
    .is_err());
    assert!(linked_recovery_operation(
        binding,
        PlayerDropAdmissionStatus::Aborted,
        None,
        Some(&record)
    )
    .is_err());
    let legacy = make_set(
        "public".into(),
        "fixture",
        binding.zone_id(),
        vec![record.clone()],
        vec![],
        256,
    )
    .unwrap();
    assert_eq!(legacy.records().len(), 1);
    assert!(
        legacy.admissions().is_empty(),
        "old operations do not manufacture admissions"
    );
}

#[test]
fn recovery_admission_source_retirement_requires_terminal_resolution() {
    assert!(!admission_source_may_retire(
        PlayerDropAdmissionStatus::Admitted,
        None
    ));
    assert!(admission_source_may_retire(
        PlayerDropAdmissionStatus::Aborted,
        None
    ));
    for status in [
        PlayerDropOperationStatus::Prepared,
        PlayerDropOperationStatus::Committed,
    ] {
        assert!(!admission_source_may_retire(
            PlayerDropAdmissionStatus::Linked,
            Some(status)
        ));
    }
    for status in [
        PlayerDropOperationStatus::Consumed,
        PlayerDropOperationStatus::Expired,
        PlayerDropOperationStatus::Aborted,
    ] {
        assert!(admission_source_may_retire(
            PlayerDropAdmissionStatus::Linked,
            Some(status)
        ));
    }
    assert!(!admission_source_may_retire(
        PlayerDropAdmissionStatus::Linked,
        None
    ));
}

#[test]
fn recovery_admission_abort_requires_exact_owner_rejection_or_newer_successor_audit() {
    let record = fixture_operation_record(PlayerDropOperationStatus::Prepared);
    let binding = record.operation().binding();
    assert!(admission_abort_authority_matches(
        binding,
        "ownerRejected",
        binding.owner_id(),
        7,
        "owner-b",
        8
    ));
    assert!(!admission_abort_authority_matches(
        binding,
        "ownerRejected",
        "owner-b",
        7,
        "owner-b",
        8
    ));
    assert!(!admission_abort_authority_matches(
        binding,
        "ownerRejected",
        binding.owner_id(),
        8,
        "owner-b",
        8
    ));
    assert!(admission_abort_authority_matches(
        binding,
        "successorAborted",
        "owner-b",
        8,
        "owner-b",
        8
    ));
    assert!(!admission_abort_authority_matches(
        binding,
        "successorAborted",
        binding.owner_id(),
        7,
        "owner-b",
        8
    ));
    assert!(!admission_abort_authority_matches(
        binding,
        "successorAborted",
        "owner-c",
        8,
        "owner-b",
        8
    ));
    assert!(!admission_abort_authority_matches(
        binding,
        "successorAborted",
        "owner-c",
        9,
        "owner-b",
        8
    ));
    assert!(!admission_abort_authority_matches(
        binding, "unknown", "owner-b", 8, "owner-b", 8
    ));
}

#[test]
fn recovery_v1_receipt_keeps_legacy_bytes_and_v2_requires_complete_admission_evidence() {
    let heads = fixture_heads("zone-1", "owner-b");
    let original = serde_json::json!({
        "version":1,"sourceNamespace":"public","zoneId":"zone-1","recoveryId":"historical",
        "ownerId":"owner-b","ownerEpoch":8,"sourceDigest":"b".repeat(64),
        "sourceAccounts":[],"sourceCharacters":[],"operationSetDigest":"c".repeat(64),
        "operationCount":0,"maxOperations":256,"stateCounts":{},"observationSequence":42,
        "host":heads.metadata,"observedAtMs":1000,"validUntilMs":5000,"requestedLifetimeMs":4000
    });
    let mut data: ReceiptData = serde_json::from_value(original.clone()).unwrap();
    assert_eq!(serde_json::to_value(&data).unwrap(), original);
    assert_eq!(
        receipt_data_digest(&data).unwrap(),
        hash_value(b"mir2.player-drop-recovery-receipt.v1\0", &original).unwrap()
    );
    data.version = 2;
    assert!(receipt_data_digest(&data).is_err());
    data.admission_set_digest = Some("d".repeat(64));
    assert!(receipt_data_digest(&data).is_err());
    data.admission_count = Some(0);
    let certified = receipt_data_digest(&data).unwrap();
    data.admission_count = Some(1);
    assert_ne!(receipt_data_digest(&data).unwrap(), certified);
    data.version = 1;
    assert!(
        receipt_data_digest(&data).is_err(),
        "historical schema cannot carry new evidence"
    );
}

#[test]
fn recovery_roster_absence_only_retires_terminal_source_actors() {
    for terminal in [
        PlayerDropOperationStatus::Consumed,
        PlayerDropOperationStatus::Expired,
        PlayerDropOperationStatus::Aborted,
    ] {
        assert!(terminal_source_may_retire(terminal));
    }
    for unresolved in [
        PlayerDropOperationStatus::Prepared,
        PlayerDropOperationStatus::Committed,
    ] {
        assert!(!terminal_source_may_retire(unresolved));
    }
}
