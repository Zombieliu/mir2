//! Journal ordering survives verified replay without granting durable authority.
use super::*;

#[derive(Debug, Default)]
struct OrderedReceiptProbe {
    inner: InProcessAccountInventoryService,
    calls: Mutex<Vec<(Option<SharedAccountInventoryExecutionContext>, u64)>>,
}

impl SharedAccountInventoryService for OrderedReceiptProbe {
    fn commit(
        &self,
        runtime: &mut InProcessWorldRuntime,
        envelope: SharedAccountInventoryCommandEnvelope,
    ) -> SharedAccountInventoryTransactionReceipt {
        self.inner.commit(runtime, envelope)
    }

    fn commit_in_zone(
        &self,
        runtime: &mut InProcessWorldRuntime,
        zone_id: &ZoneId,
        context: Option<&SharedAccountInventoryExecutionContext>,
        envelope: SharedAccountInventoryCommandEnvelope,
    ) -> SharedAccountInventoryCommitOutcome {
        if let SharedAccountInventoryCommand::GoldDrop { request_id, .. } = &envelope.command {
            self.calls
                .lock()
                .unwrap()
                .push((context.cloned(), *request_id));
        }
        self.inner
            .commit_in_zone(runtime, zone_id, context, envelope)
    }
}

fn hosted_fixture() -> (
    HostedZoneOwnerCommandClient,
    Arc<Mutex<SharedInProcessZoneState>>,
    Arc<OrderedReceiptProbe>,
) {
    let state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let service = Arc::new(OrderedReceiptProbe::default());
    let mut runtime = shared_session_runtime_with_account_inventory_service(
        state.clone(),
        service.clone() as SharedAccountInventoryServiceHandle,
    );
    start_demo_runtime(&mut runtime);
    (
        HostedZoneOwnerCommandClient::new(Box::new(runtime)),
        state,
        service,
    )
}

fn checkpointed_sequence(state: &Arc<Mutex<SharedInProcessZoneState>>) -> u64 {
    state
        .lock()
        .unwrap()
        .checkpoint()
        .unwrap()
        .next_economy_source_sequence
}

#[test]
fn hosted_verified_economy_replay_preserves_ordering_and_next_direct_request() {
    let (active, active_state, active_service) = hosted_fixture();
    let (replica, replica_state, replica_service) = hosted_fixture();
    let initial_gold = active.world_snapshot().unwrap().gold;
    assert_eq!(replica.world_snapshot().unwrap().gold, initial_gold);
    assert!(initial_gold >= 30);
    let initial_sequence = checkpointed_sequence(&active_state);
    assert_eq!(checkpointed_sequence(&replica_state), initial_sequence);
    let zone = ZoneId::new("test-shared-zone");
    let lease = ZoneOwnerLease::in_process(&zone);
    let ordered_drop = || {
        ZoneOwnerCommandRequest::production_player(
            lease.clone(),
            true,
            WorldCommand::ClientPacket(ClientPacket::DropGold { amount: 10 }),
        )
        .with_source_sequence(41)
    };
    let a = active.execute_request(ordered_drop()).unwrap();
    let b = replica.execute_replay_request(ordered_drop()).unwrap();
    for execution in [&a, &b] {
        assert!(execution
            .packets
            .iter()
            .any(|p| matches!(p, ServerPacket::LoseGold { gold: 10 })));
    }
    assert_eq!(active.world_snapshot().unwrap().gold, initial_gold - 10);
    assert_eq!(replica.world_snapshot().unwrap().gold, initial_gold - 10);
    assert_eq!(checkpointed_sequence(&active_state), initial_sequence);
    assert_eq!(
        checkpointed_sequence(&replica_state),
        initial_sequence,
        "replay must not consume a fallback counter for a journal-ordered command"
    );

    for (service, authorized) in [(&active_service, true), (&replica_service, false)] {
        let calls = service.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, 41);
        let context = calls[0]
            .0
            .as_ref()
            .expect("verified journal sequence survives replay");
        assert_eq!(context.zone_id, zone);
        assert_eq!(context.source_sequence, 41);
        assert_eq!(context.external_commit_authorized, authorized);
    }

    // A later trusted local command has no journal sequence. Both sides must
    // allocate the same checkpointed fallback ID and actually debit once.
    let direct_drop = || {
        ZoneOwnerCommandRequest::direct(
            lease.clone(),
            WorldCommand::ClientPacket(ClientPacket::DropGold { amount: 11 }),
        )
    };
    active.execute_request(direct_drop()).unwrap();
    replica.execute_request(direct_drop()).unwrap();
    for (client, state, service) in [
        (&active, &active_state, &active_service),
        (&replica, &replica_state, &replica_service),
    ] {
        assert_eq!(client.world_snapshot().unwrap().gold, initial_gold - 21);
        assert_eq!(checkpointed_sequence(state), initial_sequence + 1);
        let calls = service.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[1].1, initial_sequence + 1);
        assert!(
            calls[1].0.is_none(),
            "unordered Direct work gains no execution authority"
        );
    }
}
