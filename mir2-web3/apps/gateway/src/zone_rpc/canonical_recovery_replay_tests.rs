//! In-process canonical reset replay. No PostgreSQL publication is claimed.
use super::*;
use crate::routing::ZoneOwnerLeaseAuthority;

const PLAYER: &str = "canonical-recovery-player";

fn host() -> (ZoneHostServer, ZoneOwnerLease) {
    let authority = Arc::new(crate::InMemoryZoneOwnerLeaseAuthority::new());
    let lease = authority.owner_lease(&ZoneId::primary());
    let server = ZoneHostServer::with_identity_and_factory(
        "canonical-reset-host",
        8,
        GatewayConfig::default(),
        authority,
        None,
        ZoneRpcLimits::default(),
        Arc::new(SharedInProcessZoneRuntimeFactory::with_tick_cadences(
            Duration::from_secs(3600),
            BTreeMap::new(),
        )),
    );
    (server, lease)
}

fn envelope(lease: &ZoneOwnerLease, request: ZoneRpcRequest) -> ZoneRpcEnvelope {
    ZoneRpcEnvelope {
        protocol_version: ZONE_RPC_PROTOCOL_VERSION,
        session_id: PLAYER.into(),
        zone_id: lease.zone_id().as_str().into(),
        auth_token: None,
        request,
    }
}

fn execute(server: &ZoneHostServer, lease: &ZoneOwnerLease, command: WorldCommand) {
    let request = ZoneRpcRequest::Execute {
        owner_lease: lease.into(),
        mode: WireZoneOwnerCommandMode::Direct,
        command: WireWorldCommand::from_world(command).unwrap(),
    };
    server.handle_envelope(envelope(lease, request)).unwrap();
}

fn base(server: &ZoneHostServer, lease: &ZoneOwnerLease) -> ZoneBaseSnapshot {
    let ZoneRpcPayload::BaseSnapshot { snapshot } =
        server.export_base_snapshot(lease.zone_id()).unwrap()
    else {
        panic!("base snapshot")
    };
    snapshot
}

fn empty_base(lease: &ZoneOwnerLease, sequence: u64, digest: String) -> ZoneBaseSnapshot {
    ZoneBaseSnapshot::new(
        lease.zone_id().as_str().into(),
        zone_replication_build_id(),
        sequence,
        digest,
        SharedInProcessZoneRuntimeFactory::new()
            .checkpoint_bytes()
            .unwrap(),
        Vec::new(),
    )
    .unwrap()
}

fn reset(lease: &ZoneOwnerLease, snapshot: ZoneBaseSnapshot) -> WireHostJournalEntry {
    WireHostJournalEntry {
        sequence: snapshot.base_sequence,
        session_id: "__canonical_recovery__".into(),
        zone_id: lease.zone_id().as_str().into(),
        owner_lease: lease.into(),
        mode: WireZoneOwnerCommandMode::Direct,
        command: Some(WireWorldCommand::CanonicalZoneRecovery {
            snapshot: Box::new(snapshot),
        }),
        closed: false,
        zone_tick_ms: None,
    }
}

fn batch(entry: WireHostJournalEntry, head: &ZoneReplicationHead) -> ZoneMutationBatch {
    let digest = zone_replication_entry_digest(
        &parse_hex_digest(&head.latest_digest).unwrap(),
        head.next_sequence,
        &entry,
    )
    .unwrap();
    let digest = hex_lower_bytes(&digest);
    ZoneMutationBatch {
        version: ZONE_REPLICATION_HEAD_VERSION,
        zone_id: head.zone_id.clone(),
        build_id: zone_replication_build_id(),
        mutation_coverage: ZoneReplicationCoverage::CommandJournal,
        first_sequence: head.next_sequence,
        next_sequence: head.next_sequence + 1,
        previous_digest: head.latest_digest.clone(),
        latest_digest: digest.clone(),
        entries: vec![ZoneMutationEntry {
            sequence: head.next_sequence,
            digest,
            payload: serde_json::to_vec(&entry).unwrap(),
        }],
        has_more: false,
    }
}

fn session(server: &ZoneHostServer, lease: &ZoneOwnerLease) -> Arc<ZoneHostSession> {
    server
        .sessions
        .lock()
        .unwrap()
        .get(&(PLAYER.into(), lease.zone_id().as_str().into()))
        .unwrap()
        .clone()
}

#[test]
fn canonical_reset_is_server_only_and_rejects_session_dispatch() {
    let (server, lease) = host();
    let entry = reset(&lease, empty_base(&lease, 0, "0".repeat(64)));
    let command = entry.command.clone().unwrap();
    let decoded: WireWorldCommand =
        serde_json::from_slice(&serde_json::to_vec(&command).unwrap()).unwrap();
    assert_eq!(decoded.into_world().unwrap_err().code, "checkpoint_command");
    let error = server
        .handle_envelope(envelope(
            &lease,
            ZoneRpcRequest::Execute {
                owner_lease: (&lease).into(),
                mode: WireZoneOwnerCommandMode::Direct,
                command,
            },
        ))
        .unwrap_err();
    assert_eq!(error.code, "checkpoint_command");
    assert_eq!(server.runtime_execute_calls.load(Ordering::Acquire), 0);
    let hosted = session(&server, &lease);
    assert_eq!(
        entry
            .replay_session_mutation(&hosted, 0, true)
            .unwrap_err()
            .code,
        "checkpoint_command"
    );
    assert!(server.journal.lock().unwrap().entries.is_empty());
}

#[test]
fn reset_installs_canonical_source_and_base_before_next_normal_mutation() {
    let (source, lease) = host();
    for packet in [
        ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        },
        ClientPacket::StartGame { character_index: 0 },
    ] {
        execute(&source, &lease, WorldCommand::ClientPacket(packet));
    }
    let original = base(&source, &lease);
    let (standby, _) = host();
    standby
        .install_base_snapshot(lease.zone_id().as_str(), &original)
        .unwrap();
    let pre_head = source
        .journal
        .lock()
        .unwrap()
        .replication
        .head(lease.zone_id().as_str());
    let hosted = session(&source, &lease);
    let mut canonical = hosted
        .hosted
        .active_character_checkpoint()
        .unwrap()
        .unwrap();
    canonical.gold += 17;
    canonical.revision += 1;
    hosted
        .hosted
        .restore_active_character_checkpoint(&canonical)
        .unwrap();
    let recovered = base(&source, &lease);
    assert_ne!(recovered.snapshot_id, original.snapshot_id);
    assert_eq!(recovered.base_sequence, pre_head.next_sequence);
    let reset_entry = reset(&lease, recovered.clone());
    reset_entry
        .validate_canonical_zone_recovery(&recovered, &pre_head)
        .unwrap();
    source
        .journal
        .lock()
        .unwrap()
        .install_base(&recovered)
        .unwrap();
    source.append_journal(reset_entry).unwrap();
    execute(
        &source,
        &lease,
        WorldCommand::SetLanguage {
            language: "en-US".into(),
        },
    );
    let mutations = source
        .journal
        .lock()
        .unwrap()
        .export_batch(
            lease.zone_id().as_str(),
            pre_head.next_sequence,
            8,
            DEFAULT_ZONE_RPC_MAX_FRAME_BYTES,
        )
        .unwrap();
    standby
        .handle_envelope(envelope(
            &lease,
            ZoneRpcRequest::ApplyMutationBatch {
                batch: mutations.clone(),
            },
        ))
        .unwrap();
    assert_eq!(standby.session_count(), 1);
    assert!(!standby
        .sessions
        .lock()
        .unwrap()
        .keys()
        .any(|(id, _)| id == "__canonical_recovery__"));
    let installed = session(&standby, &lease)
        .hosted
        .active_character_checkpoint()
        .unwrap()
        .unwrap();
    assert_eq!(installed.gold, canonical.gold);
    assert_eq!(installed.revision, canonical.revision);
    let actual = standby
        .journal
        .lock()
        .unwrap()
        .replication
        .head(lease.zone_id().as_str());
    assert_eq!(
        actual.base_snapshot_id.as_deref(),
        Some(recovered.snapshot_id.as_str())
    );
    assert_eq!(actual.base_sequence, pre_head.next_sequence);
    assert_eq!(actual.next_sequence, pre_head.next_sequence + 2);
    assert_eq!(actual.latest_digest, mutations.latest_digest);
    assert_eq!(standby.runtime_execute_calls.load(Ordering::Acquire), 0);
}

#[test]
fn empty_reset_creates_no_session_and_advances_from_n_to_n_plus_one() {
    let (standby, lease) = host();
    let head = standby
        .journal
        .lock()
        .unwrap()
        .replication
        .head(lease.zone_id().as_str());
    let recovered = empty_base(&lease, head.next_sequence, head.latest_digest.clone());
    let mutation = batch(reset(&lease, recovered.clone()), &head);
    standby
        .handle_envelope(envelope(
            &lease,
            ZoneRpcRequest::ApplyMutationBatch { batch: mutation },
        ))
        .unwrap();
    let actual = standby
        .journal
        .lock()
        .unwrap()
        .replication
        .head(lease.zone_id().as_str());
    assert_eq!(actual.next_sequence, 1);
    assert_eq!(actual.base_snapshot_id, Some(recovered.snapshot_id));
    assert_eq!(standby.session_count(), 0);
    assert_eq!(standby.runtime_execute_calls.load(Ordering::Acquire), 0);
}

#[test]
fn reset_rejects_semantically_tampered_entries_before_install_or_session_creation() {
    let (standby, lease) = host();
    let head = standby
        .journal
        .lock()
        .unwrap()
        .replication
        .head(lease.zone_id().as_str());
    let original = empty_base(&lease, head.next_sequence, head.latest_digest.clone());
    for kind in [
        "sequence", "zone", "digest", "session", "mode", "closed", "tick", "lease",
    ] {
        let mut changed = reset(&lease, original.clone());
        match kind {
            "sequence" => {
                changed.command = Some(WireWorldCommand::CanonicalZoneRecovery {
                    snapshot: Box::new(empty_base(&lease, 1, head.latest_digest.clone())),
                })
            }
            "zone" => {
                let other = ZoneOwnerLease::new(ZoneId::new("other-zone"), "other-owner", 1);
                changed.command = Some(WireWorldCommand::CanonicalZoneRecovery {
                    snapshot: Box::new(empty_base(&other, 0, head.latest_digest.clone())),
                });
            }
            "digest" => {
                changed.command = Some(WireWorldCommand::CanonicalZoneRecovery {
                    snapshot: Box::new(empty_base(&lease, 0, "1".repeat(64))),
                })
            }
            "session" => changed.session_id = PLAYER.into(),
            "mode" => {
                changed.mode = WireZoneOwnerCommandMode::ProductionPlayer {
                    authenticated: true,
                }
            }
            "closed" => changed.closed = true,
            "tick" => changed.zone_tick_ms = Some(100),
            "lease" => changed.owner_lease.zone_id = "other-zone".into(),
            _ => unreachable!(),
        }
        let error = standby
            .handle_envelope(envelope(
                &lease,
                ZoneRpcRequest::ApplyMutationBatch {
                    batch: batch(changed, &head),
                },
            ))
            .unwrap_err();
        assert_eq!(error.code, "canonical_recovery_reset_invalid", "{kind}");
        assert_eq!(standby.session_count(), 0, "{kind}");
        assert!(standby.journal.lock().unwrap().entries.is_empty(), "{kind}");
    }
}

#[test]
fn reset_requires_exact_base_anchor_and_digest_verified_batch() {
    let (server, lease) = host();
    let head = server
        .journal
        .lock()
        .unwrap()
        .replication
        .head(lease.zone_id().as_str());
    let recovered = empty_base(&lease, 0, head.latest_digest.clone());
    let entry = reset(&lease, recovered.clone());
    assert_eq!(
        server.append_journal(entry.clone()).unwrap_err().code,
        "canonical_recovery_base_unanchored"
    );
    assert!(server.journal.lock().unwrap().entries.is_empty());
    let mut other_base = recovered.clone();
    other_base.created_at_ms += 1;
    other_base.snapshot_id = zone_base_snapshot_checksum(&other_base).unwrap();
    server
        .journal
        .lock()
        .unwrap()
        .install_base(&other_base)
        .unwrap();
    assert_eq!(
        server.append_journal(entry.clone()).unwrap_err().code,
        "canonical_recovery_base_unanchored"
    );
    assert!(server.journal.lock().unwrap().entries.is_empty());
    let mut altered = batch(entry.clone(), &head);
    altered.entries[0].payload.push(b' ');
    // Whitespace does not alter canonical JSON; alter the envelope digest too.
    altered.entries[0].digest = "1".repeat(64);
    assert_eq!(
        server
            .handle_envelope(envelope(
                &lease,
                ZoneRpcRequest::ApplyMutationBatch { batch: altered }
            ))
            .unwrap_err()
            .code,
        "mutation_batch_invalid"
    );
    server
        .journal
        .lock()
        .unwrap()
        .install_base(&recovered)
        .unwrap();
    server.append_journal(entry).unwrap();
    assert_eq!(
        server
            .journal
            .lock()
            .unwrap()
            .export_batch(lease.zone_id().as_str(), 0, 8, 1)
            .unwrap_err()
            .code,
        "replication_cursor_compacted"
    );
    assert_eq!(
        server
            .journal
            .lock()
            .unwrap()
            .replication
            .head(lease.zone_id().as_str())
            .next_sequence,
        1
    );
}
