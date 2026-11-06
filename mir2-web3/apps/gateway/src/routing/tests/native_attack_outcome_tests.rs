//! The receipt describes this command's authoritative launch, never a FIFO
//! guess, a prior animation, or a claim that positive damage occurred.
use super::*;
use crate::routing::{
    finish_native_attack_execution, InProcessZoneOwnerCommandClient, NativeAttackAdmission,
    NativeAttackError, SharedZoneLiveOutboundSender, ZoneOwnerCommandClient,
};
use mir2_simulation::{
    NativeAttackExecution, WorldCommandExecution, WorldCommandKind, WorldCommandOutcome,
};
use sha2::{Digest, Sha256};

fn fixture() -> (
    SharedInProcessZoneSessionRuntime,
    Arc<Mutex<SharedInProcessZoneState>>,
    u32,
) {
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut owner = shared_session_runtime(shared.clone());
    start_new_runtime(&mut owner, "typed-swing", "TypedSwing");
    let key = owner.current_presence_key().unwrap();
    let mut target = shared_monster_entity(260_925);
    target.ai = Some(2);
    target.hp = Some(10_000);
    target.max_hp = Some(10_000);
    target.x = 331;
    target.y = 270;
    {
        let mut state = shared.lock().unwrap();
        state.update_player_transform(&key, Point { x: 330, y: 270 }, MirDirection::Right);
        state.sync_map_layer(
            "0".into(),
            vec![target.clone()],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );
        state.take_pending_zone_packets(&key);
    }
    owner
        .inner
        .force_authoritative_player_transform(Point { x: 330, y: 270 }, MirDirection::Right);
    owner.sync_zone_snapshot();
    owner.validate_durable_npc_actor().unwrap();
    let prepared = owner
        .prepare_zone_native_player_attack(&WorldCommand::Attack {
            object_id: target.object_id,
        })
        .unwrap();
    assert_eq!(prepared.object_id, target.object_id);
    (owner, shared, target.object_id)
}

fn attack(
    owner: &mut SharedInProcessZoneSessionRuntime,
    target: u32,
) -> Result<NativeAttackExecution, NativeAttackError> {
    owner.execute_production_attack_requiring_typed_outcome(true, target)
}

#[test]
fn private_runtime_defaults_to_unsupported_without_executing_legacy_attack() {
    let mut runtime = InProcessWorldRuntime::new(GatewayConfig::default());
    assert_eq!(
        runtime.execute_production_attack_requiring_typed_outcome(true, 42),
        Err(NativeAttackError::Unsupported)
    );
    assert!(runtime.active_identity().is_none());
}

#[test]
fn authentication_and_actual_registered_actor_are_required_before_dispatch() {
    let (mut owner, shared, target) = fixture();
    let before = shared
        .lock()
        .unwrap()
        .zone_manager
        .checkpoint_bytes()
        .unwrap();
    assert!(matches!(
        owner.execute_production_attack_requiring_typed_outcome(false, target),
        Err(NativeAttackError::BeforeExecution(_))
    ));
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .zone_manager
            .checkpoint_bytes()
            .unwrap(),
        before
    );
    owner
        .movement_ingress
        .session_state
        .lock()
        .unwrap()
        .presence_key = None;
    assert!(matches!(
        attack(&mut owner, target),
        Err(NativeAttackError::BeforeExecution(_))
    ));
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .zone_manager
            .checkpoint_bytes()
            .unwrap(),
        before
    );
}

#[test]
fn launched_result_preserves_ordinary_owner_and_peer_animation_identity() {
    let (mut owner, shared, target) = fixture();
    let mut peer = shared_session_runtime(shared.clone());
    start_new_runtime(&mut peer, "typed-peer", "TypedPeer");
    let key = peer.current_presence_key().unwrap();
    shared.lock().unwrap().update_player_transform(
        &key,
        Point { x: 330, y: 271 },
        MirDirection::Up,
    );
    peer.inner
        .force_authoritative_player_transform(Point { x: 330, y: 271 }, MirDirection::Up);
    peer.sync_zone_snapshot();
    let local = owner.local_self_object_id().unwrap();
    let zone_actor = owner.current_zone_player_object_id().unwrap();
    let result = attack(&mut owner, target).unwrap();
    assert_eq!(result.admission, NativeAttackAdmission::Launched);
    assert_eq!(
        result.execution.outcome.command_kind,
        WorldCommandKind::Attack
    );
    assert_eq!(
        result.execution.outcome.packet_count,
        result.execution.packets.len()
    );
    assert!(result.execution.packets.iter().any(
        |packet| matches!(packet, ServerPacket::ObjectAttack { info } if info.object_id == local)
    ));
    let peer_packets = peer.execute(WorldCommand::Tick).unwrap();
    assert!(peer_packets.iter().any(|packet| matches!(packet, ServerPacket::ObjectAttack { info } if info.object_id == zone_actor)));
    assert!(peer_packets.iter().all(
        |packet| !matches!(packet, ServerPacket::ObjectAttack { info } if info.object_id == local)
    ));
}

#[test]
fn cooldown_and_out_of_range_are_not_launched() {
    let (mut owner, shared, target) = fixture();
    assert_eq!(
        attack(&mut owner, target).unwrap().admission,
        NativeAttackAdmission::Launched
    );
    let cooldown = attack(&mut owner, target).unwrap();
    assert_eq!(cooldown.admission, NativeAttackAdmission::NotLaunched);
    let key = owner.current_presence_key().unwrap();
    shared.lock().unwrap().update_player_transform(
        &key,
        Point { x: 320, y: 270 },
        MirDirection::Right,
    );
    owner
        .inner
        .force_authoritative_player_transform(Point { x: 320, y: 270 }, MirDirection::Right);
    let far = attack(&mut owner, target).unwrap();
    assert_eq!(far.admission, NativeAttackAdmission::NotLaunched);
    assert!(far
        .execution
        .packets
        .iter()
        .all(|packet| !matches!(packet, ServerPacket::ObjectAttack { .. })));
}

#[test]
fn missing_target_is_not_launched_and_still_returns_the_ordinary_correction() {
    let (mut owner, _, _) = fixture();
    let result = attack(&mut owner, 499_999).unwrap();
    assert_eq!(result.admission, NativeAttackAdmission::NotLaunched);
    assert!(result
        .execution
        .packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::UserLocation { .. })));
    assert!(result
        .execution
        .packets
        .iter()
        .all(|packet| !matches!(packet, ServerPacket::ObjectAttack { .. })));
}

#[test]
fn earlier_owner_animation_in_predrain_does_not_admit_this_rejected_attack() {
    let (mut owner, shared, target) = fixture();
    let accepted = attack(&mut owner, target).unwrap();
    let mut earlier = accepted
        .execution
        .packets
        .into_iter()
        .find(|packet| matches!(packet, ServerPacket::ObjectAttack { .. }))
        .unwrap();
    let zone_actor = owner.current_zone_player_object_id().unwrap();
    if let ServerPacket::ObjectAttack { info } = &mut earlier {
        info.object_id = zone_actor;
    }
    let key = owner.current_presence_key().unwrap();
    shared
        .lock()
        .unwrap()
        .queue_zone_packets(key, vec![earlier]);
    let rejected = attack(&mut owner, 499_999).unwrap();
    assert_eq!(rejected.admission, NativeAttackAdmission::NotLaunched);
    assert!(
        rejected
            .execution
            .packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectAttack { .. })),
        "fixture must actually expose a prior animation in the returned packets"
    );
}

#[test]
fn full_live_fifo_cannot_revoke_a_launched_attack() {
    let (mut owner, shared, target) = fixture();
    let key = owner.current_presence_key().unwrap();
    let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
    shared
        .lock()
        .unwrap()
        .register_live_zone_outbound(key, SharedZoneLiveOutboundSender::single(sender));
    assert!(owner.authoritative_zone_owner_correction().is_empty());
    let result = attack(&mut owner, target).unwrap();
    assert_eq!(result.admission, NativeAttackAdmission::Launched);
    assert!(
        receiver.try_recv().is_ok(),
        "the real live FIFO was occupied throughout dispatch"
    );
}

#[test]
fn strict_dispatch_preserves_authority_errors_without_fabricating_a_decision() {
    let (mut owner, _, _) = fixture();
    let config = owner.inner.shared_mentor_config().unwrap();
    let store = config.account_store.clone();
    assert!(std::thread::spawn(move || {
        let _guard = store.lock().unwrap();
        panic!("fixture source failure");
    })
    .join()
    .is_err());
    let session = owner.current_zone_session_id().unwrap();
    let mut observed = false;
    let mut observe = |_: &[ZoneOutbound]| {
        observed = true;
    };
    assert!(owner
        .try_dispatch_zone_player_command_observing_outbounds(
            ZoneCommand::CancelPendingMovement {
                session_id: session
            },
            Some(&mut observe)
        )
        .is_err());
    assert!(
        !observed,
        "failed source admission must not publish empty outbounds as a business decision"
    );
}

#[test]
fn teardown_fence_never_becomes_not_launched() {
    let (mut owner, shared, target) = fixture();
    let key = owner.current_presence_key().unwrap();
    shared.lock().unwrap().teardown_fences.insert(key);
    assert!(matches!(
        attack(&mut owner, target),
        Err(NativeAttackError::BeforeExecution(_))
    ));
    let session = owner.current_zone_session_id().unwrap();
    assert!(owner
        .try_dispatch_zone_player_command_observing_outbounds(
            ZoneCommand::CancelPendingMovement {
                session_id: session
            },
            None
        )
        .is_err());
}

fn exhaust_manager_admission(shared: &Arc<Mutex<SharedInProcessZoneState>>, session: &SessionId) {
    // Construct and verify the saturated fixture without holding the shared
    // mutex. A malformed fixture must fail normally, never poison owner Drop.
    let original = { shared.lock().unwrap().zone_manager.checkpoint_bytes() }.unwrap();
    let mut manager = ZoneManager::restore_checkpoint(&original).unwrap();
    manager
        .bind_logical_world(
            mir2_simulation::LogicalWorldId::legacy_execution_domain("typed-overflow").unwrap(),
        )
        .unwrap();
    manager.handle(ZoneCommand::CancelPendingMovement {
        session_id: session.clone(),
    });
    let count = manager.admitted_action_count();
    let bytes = manager.checkpoint_bytes().unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let root = value["state_root"].as_str().unwrap();
    // Keep the checkpoint serializer's exact field order, including nested
    // authority structs. This is a valid saturated fixture, not bad-root input.
    let mut text = String::from_utf8(bytes).unwrap();
    let root_field = format!("\"state_root\":\"{root}\"");
    for name in ["admission_sequence", "source_sequence"] {
        let token = format!("\"{name}\":{count}");
        assert_eq!(text.matches(&token).count(), 1);
        text = text.replace(&token, &format!("\"{name}\":{}", u64::MAX));
    }
    let mut hash = Sha256::new();
    hash.update(b"obelisk.mir2.zone-manager.v4\0");
    // ZoneManagerCheckpoint::canonical_root clears state_root, whose
    // skip_serializing_if then omits the field entirely (not an empty value).
    let omitted_root_field = format!(",{root_field}");
    assert_eq!(text.matches(&omitted_root_field).count(), 1);
    let canonical = text.replace(&omitted_root_field, "");
    hash.update(canonical.as_bytes());
    let root = hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    text = text.replace(&root_field, &format!("\"state_root\":\"{root}\""));
    let exhausted = ZoneManager::restore_checkpoint(text.as_bytes())
        .expect("saturated manager fixture must satisfy the production checkpoint commitment");
    assert_eq!(exhausted.admitted_action_count(), u64::MAX);
    assert!(exhausted.ensure_admission_capacity(1).is_err());
    shared.lock().unwrap().zone_manager = exhausted;
}

#[test]
fn manager_admission_overflow_is_an_error_not_a_missing_animation_rejection() {
    let (mut owner, shared, target) = fixture();
    let session = owner.current_zone_session_id().unwrap();
    exhaust_manager_admission(&shared, &session);
    let result = attack(&mut owner, target);
    assert!(
        matches!(result, Err(NativeAttackError::Unknown(_))),
        "{result:?}"
    );
}

#[test]
fn post_dispatch_failure_and_missing_observation_remain_unknown() {
    assert!(matches!(
        finish_native_attack_execution(
            Err("reconciliation failed after launch".into()),
            Some(NativeAttackAdmission::Launched)
        ),
        Err(NativeAttackError::Unknown(_))
    ));
    let execution = WorldCommandExecution {
        packets: Vec::new(),
        outcome: WorldCommandOutcome {
            command_kind: WorldCommandKind::Attack,
            packet_count: 0,
            snapshot_tick: 0,
            active_identity: None,
        },
        game_shop_purchase_outcome: None,
    };
    assert!(matches!(
        finish_native_attack_execution(Ok(execution), None),
        Err(NativeAttackError::Unknown(_))
    ));
}

#[test]
fn owner_domain_mismatch_is_rejected_before_the_shared_pipeline() {
    let (owner, shared, target) = fixture();
    let before = shared
        .lock()
        .unwrap()
        .zone_manager
        .checkpoint_bytes()
        .unwrap();
    let mut runtime: ZoneRuntimeHandle = Box::new(owner);
    let client = InProcessZoneOwnerCommandClient::default();
    let lease = ZoneOwnerLease::in_process(&ZoneId::new("wrong-domain"));
    let result = client.execute_production_attack_requiring_typed_outcome(
        &mut runtime,
        ZoneOwnerCommandRequest::production_player(
            lease,
            true,
            WorldCommand::Attack { object_id: target },
        ),
    );
    assert!(matches!(result, Err(NativeAttackError::BeforeExecution(_))));
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .zone_manager
            .checkpoint_bytes()
            .unwrap(),
        before
    );
}

#[test]
fn stale_owner_lease_cannot_enter_the_typed_runtime() {
    let (owner, shared, target) = fixture();
    let before = shared
        .lock()
        .unwrap()
        .zone_manager
        .checkpoint_bytes()
        .unwrap();
    let mut runtime: ZoneRuntimeHandle = Box::new(owner);
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let zone = ZoneId::new("test-shared-zone");
    let stale = authority.owner_lease_at(&zone, 0);
    let _current = authority.handoff_zone_owner(&zone, "replacement-owner");
    let client = InProcessZoneOwnerCommandClient::with_owner_lease_authority(authority);
    assert!(matches!(
        client.execute_production_attack_requiring_typed_outcome(
            &mut runtime,
            ZoneOwnerCommandRequest::production_player(
                stale,
                true,
                WorldCommand::Attack { object_id: target }
            ),
        ),
        Err(NativeAttackError::BeforeExecution(_))
    ));
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .zone_manager
            .checkpoint_bytes()
            .unwrap(),
        before
    );
}

#[test]
fn session_bridge_requires_authentication_and_uses_the_shared_typed_route() {
    let registry = ZoneRegistry::in_process();
    let mut session = GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
    assert!(matches!(
        session.execute_production_attack_requiring_typed_outcome(499_999, true),
        Err(NativeAttackError::BeforeExecution(_))
    ));
    start_new_character(&mut session, "typed-bridge", "TypedBridge");
    assert!(matches!(
        session.execute_production_attack_requiring_typed_outcome(499_999, false),
        Err(NativeAttackError::BeforeExecution(_))
    ));
    let result = session
        .execute_production_attack_requiring_typed_outcome(499_999, true)
        .unwrap();
    assert_eq!(result.admission, NativeAttackAdmission::NotLaunched);
    assert!(result
        .execution
        .packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::UserLocation { .. })));
}

#[test]
fn a_launched_swing_does_not_require_positive_damage() {
    let (mut owner, _, target) = fixture();
    let mut prepared = owner
        .prepare_zone_native_player_attack(&WorldCommand::Attack { object_id: target })
        .unwrap();
    prepared.damage = 0;
    let mut admission = None;
    let packets = owner
        .execute_zone_native_player_attack_observing(prepared, Some(&mut admission))
        .unwrap();
    assert_eq!(admission, Some(NativeAttackAdmission::Launched));
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::ObjectAttack { .. })));
    assert!(packets
        .iter()
        .all(|packet| !matches!(packet, ServerPacket::DamageIndicator { .. })));
}
