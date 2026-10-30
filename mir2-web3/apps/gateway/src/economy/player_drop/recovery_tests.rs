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
    assert!(
        PlayerDropRecoveryHostHeads::new(
            "owner-b".into(),
            heads.pre_recovery_head().clone(),
            corrupt
        )
        .is_err()
    );
    let mut head = heads.pre_recovery_head().clone();
    head.next_sequence += 1;
    assert!(
        PlayerDropRecoveryHostHeads::new("owner-b".into(), head, heads.base_snapshot().unwrap())
            .is_err()
    );
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
    let one = make_set("public".into(), "fixture", "zone-1", vec![], 256).unwrap();
    let zone = make_set("public".into(), "fixture", "zone-2", vec![], 256).unwrap();
    let namespace = make_set("source_b".into(), "fixture", "zone-1", vec![], 256).unwrap();
    assert_ne!(one.operation_set_digest, zone.operation_set_digest);
    assert_ne!(one.operation_set_digest, namespace.operation_set_digest);
    assert!(one.required_characters().is_empty());
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
