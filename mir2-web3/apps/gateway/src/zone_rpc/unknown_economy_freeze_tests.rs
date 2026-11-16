//! Unknown source commits are independent of expiring promotion readiness.
//! These are in-process protocol/admission checks, not durable restart recovery.
use super::*;

const ZONE: &str = "unknown-commit-zone";
const HEALTHY: &str = "healthy-commit-zone";

fn server() -> ZoneHostServer {
    ZoneHostServer::with_identity_and_factory(
        "unknown-freeze-test-host",
        8,
        GatewayConfig::default(),
        Arc::new(crate::InMemoryZoneOwnerLeaseAuthority::default()),
        Some("unknown-freeze-test-token".into()),
        ZoneRpcLimits::default(),
        Arc::new(SharedInProcessZoneRuntimeFactory::new()),
    )
}

fn envelope(zone: &str, request: ZoneRpcRequest) -> ZoneRpcEnvelope {
    ZoneRpcEnvelope {
        protocol_version: ZONE_RPC_PROTOCOL_VERSION,
        session_id: "unknown-freeze-session".into(),
        zone_id: zone.into(),
        auth_token: Some("unknown-freeze-test-token".into()),
        request,
    }
}

fn freeze(server: &ZoneHostServer) {
    assert_eq!(
        server
            .classify_zone_runtime_error(
                ZONE,
                "ZONE_ECONOMY_COMMIT_OUTCOME_UNKNOWN: original debit unresolved".into()
            )
            .code,
        "commit_outcome_unknown"
    );
}

fn assert_frozen(server: &ZoneHostServer) {
    assert!(server
        .unknown_commit_frozen_zones
        .lock()
        .unwrap()
        .contains(ZONE));
    assert!(!server
        .unknown_commit_frozen_zones
        .lock()
        .unwrap()
        .contains(HEALTHY));
}

#[test]
fn subsequent_real_envelope_prunes_readiness_without_clearing_unknown_commit() {
    let server = server();
    let zone = ZoneId::new(ZONE);
    let head = server.journal.lock().unwrap().replication.head(ZONE);
    let mut readiness = server
        .assess_promotion_readiness(&zone, &head, unix_now_ms(), 100)
        .unwrap();
    readiness.ready = true;
    readiness.readiness_id = Some("expired-readiness".into());
    readiness.expires_at_ms = Some(1);
    server.promotion_readiness.lock().unwrap().insert(
        "expired-readiness".into(),
        ZonePromotionReadinessRecord { readiness, head },
    );
    server
        .promotion_frozen_zones
        .lock()
        .unwrap()
        .insert(ZONE.into());
    freeze(&server);
    // ReplicationHead takes the actual envelope gate and calls the readiness
    // pruner. Its expired temporary barrier may vanish; Unknown must survive.
    assert!(matches!(
        server
            .handle_envelope(envelope(ZONE, ZoneRpcRequest::ReplicationHead))
            .unwrap(),
        ZoneRpcPayload::ReplicationHead { .. }
    ));
    assert!(server.promotion_readiness.lock().unwrap().is_empty());
    assert!(server.promotion_frozen_zones.lock().unwrap().is_empty());
    assert_frozen(&server);
    for request in [
        ZoneRpcRequest::OnConnect,
        ZoneRpcRequest::SaveActiveCharacter,
        ZoneRpcRequest::CloseSession {
            owner_lease: (&ZoneOwnerLease::in_process(&zone)).into(),
        },
    ] {
        assert_eq!(
            server
                .handle_envelope(envelope(ZONE, request))
                .unwrap_err()
                .code,
            "commit_outcome_unknown"
        );
        assert_frozen(&server);
    }
    assert_eq!(server.session_count(), 0);
    assert!(matches!(
        server
            .handle_envelope(envelope(HEALTHY, ZoneRpcRequest::OnConnect))
            .unwrap(),
        ZoneRpcPayload::Packets { .. }
    ));
    assert_eq!(server.session_count(), 1);
    assert_frozen(&server);
}

#[test]
fn readiness_assessment_resume_and_promotion_cannot_admit_an_unknown_zone() {
    let server = server();
    let zone = ZoneId::new(ZONE);
    let lease = ZoneOwnerLease::in_process(&zone);
    let head = server.journal.lock().unwrap().replication.head(ZONE);
    freeze(&server);
    server.quiesced_zones.lock().unwrap().insert(ZONE.into());
    server.prune_expired_promotion_readiness(u64::MAX).unwrap();
    assert_eq!(
        server
            .assess_promotion_readiness_locked(&zone, &head, unix_now_ms(), 100)
            .unwrap_err()
            .code,
        "commit_outcome_unknown"
    );
    assert_eq!(
        server
            .promote_replica_locked(&zone, "any-readiness", &lease)
            .unwrap_err()
            .code,
        "commit_outcome_unknown"
    );
    assert_eq!(
        server
            .resume_after_quiesce_locked(&zone, &lease)
            .unwrap_err()
            .code,
        "commit_outcome_unknown"
    );
    assert!(server.quiesced_zones.lock().unwrap().contains(ZONE));
    for request in [
        ZoneRpcRequest::AssessPromotionReadiness {
            active_head: head,
            source_observed_at_ms: unix_now_ms(),
            max_lag_ms: 100,
        },
        ZoneRpcRequest::PromoteReplica {
            readiness_id: "any-readiness".into(),
            owner_lease: (&lease).into(),
        },
        ZoneRpcRequest::ResumeAfterQuiesce {
            owner_lease: (&lease).into(),
        },
    ] {
        assert_eq!(
            server
                .handle_envelope(envelope(ZONE, request))
                .unwrap_err()
                .code,
            "commit_outcome_unknown"
        );
    }
    let healthy = ZoneId::new(HEALTHY);
    let healthy_head = server.journal.lock().unwrap().replication.head(HEALTHY);
    server
        .assess_promotion_readiness(&healthy, &healthy_head, unix_now_ms(), 100)
        .unwrap();
    assert_frozen(&server);
}

#[test]
fn checkpoint_and_replica_replacement_cannot_export_or_erase_unknown_authority() {
    let server = server();
    let zone = ZoneId::new(ZONE);
    let head = server.journal.lock().unwrap().replication.head(ZONE);
    let ZoneRpcPayload::BaseSnapshot { snapshot } = server.export_base_snapshot(&zone).unwrap()
    else {
        panic!("base snapshot expected")
    };
    let batch = ZoneMutationBatch {
        version: ZONE_REPLICATION_HEAD_VERSION,
        zone_id: ZONE.into(),
        build_id: head.build_id,
        mutation_coverage: ZoneReplicationCoverage::CommandJournal,
        first_sequence: head.next_sequence,
        next_sequence: head.next_sequence,
        previous_digest: head.latest_digest.clone(),
        latest_digest: head.latest_digest,
        entries: Vec::new(),
        has_more: false,
    };
    freeze(&server);
    assert_eq!(
        server
            .install_base_snapshot(ZONE, &snapshot)
            .unwrap_err()
            .code,
        "commit_outcome_unknown"
    );
    assert_eq!(
        server.apply_mutation_batch(ZONE, &batch).unwrap_err().code,
        "commit_outcome_unknown"
    );
    assert_eq!(
        server
            .compact_mutation_journal(ZONE, &snapshot)
            .unwrap_err()
            .code,
        "commit_outcome_unknown"
    );
    assert_eq!(
        server
            .install_host_checkpoint(b"invalid-checkpoint")
            .unwrap_err()
            .code,
        "commit_outcome_unknown"
    );
    assert_eq!(
        server.export_base_snapshot(&zone).unwrap_err().code,
        "commit_outcome_unknown"
    );
    assert_eq!(
        server.export_host_checkpoint().unwrap_err().code,
        "commit_outcome_unknown"
    );
    // Global host replacement is also refused when its envelope names an
    // otherwise healthy Zone, because it would replace the affected image.
    assert_eq!(
        server
            .handle_envelope(envelope(
                HEALTHY,
                ZoneRpcRequest::InstallHostCheckpoint { bytes: Vec::new() }
            ))
            .unwrap_err()
            .code,
        "commit_outcome_unknown"
    );
    assert!(server.export_base_snapshot(&ZoneId::new(HEALTHY)).is_ok());
    assert_frozen(&server);
}

#[test]
fn health_and_receipt_query_remain_readonly_before_unknown_mutation_admission() {
    let server = server();
    freeze(&server);
    assert!(matches!(
        server
            .handle_envelope(envelope(ZONE, ZoneRpcRequest::Health))
            .unwrap(),
        ZoneRpcPayload::Health { .. }
    ));
    let binding = WireZoneSaveBinding {
        operation_id: "original-operation".into(),
        rpc_session_id: "unknown-freeze-session".into(),
        zone_id: ZONE.into(),
        owner_id: "old-owner".into(),
        fencing_token: 7,
    };
    // This non-PG fixture reaches capability validation before world/lease
    // admission. Actual old-epoch receipt rows are tested by the PG modules.
    assert_eq!(
        server
            .handle_envelope(envelope(
                ZONE,
                ZoneRpcRequest::QueryZoneSaveOperation { binding }
            ))
            .unwrap_err()
            .code,
        "unsupported_capability"
    );
    assert_eq!(server.session_count(), 0);
    assert_frozen(&server);
}
