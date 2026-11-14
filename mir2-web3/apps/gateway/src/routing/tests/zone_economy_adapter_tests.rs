//! Dispatch evidence only. The probe never claims a PostgreSQL commit.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Observation {
    phase: &'static str,
    account: String,
    context: Option<SharedAccountInventoryExecutionContext>,
    retry_key: Option<String>,
}

#[derive(Debug)]
struct RuntimeTradeProbe(Arc<Mutex<Vec<Observation>>>);

impl SharedAccountInventoryService for RuntimeTradeProbe {
    fn commit(
        &self,
        runtime: &mut InProcessWorldRuntime,
        envelope: SharedAccountInventoryCommandEnvelope,
    ) -> SharedAccountInventoryTransactionReceipt {
        InProcessAccountInventoryService::new().commit(runtime, envelope)
    }

    fn settle_trade_fenced(
        &self,
        _: Option<&SharedAccountInventoryExecutionContext>,
        _: &SharedTradeOffer,
        _: &SharedTradeOffer,
    ) -> SharedTradeSettlementOutcome {
        panic!("shared trade must supply its authoritative runtime")
    }

    fn retry_trade_fenced(
        &self,
        _: Option<&SharedAccountInventoryExecutionContext>,
        _: &str,
        _: &SharedTradeOffer,
        _: &SharedTradeOffer,
    ) -> SharedTradeSettlementOutcome {
        panic!("recovery context cannot replace the authoritative runtime")
    }

    fn settle_trade_with_runtime(
        &self,
        runtime: &InProcessWorldRuntime,
        context: Option<&SharedAccountInventoryExecutionContext>,
        _: &SharedTradeOffer,
        _: &SharedTradeOffer,
    ) -> SharedTradeSettlementOutcome {
        self.0.lock().unwrap().push(Observation {
            phase: "initial",
            account: runtime.active_identity().unwrap().account_id,
            context: context.cloned(),
            retry_key: None,
        });
        SharedTradeSettlementOutcome::Deferred
    }

    fn retry_trade_with_runtime(
        &self,
        runtime: &InProcessWorldRuntime,
        context: Option<&SharedAccountInventoryExecutionContext>,
        key: &str,
        _: &SharedTradeOffer,
        _: &SharedTradeOffer,
    ) -> SharedTradeSettlementOutcome {
        self.0.lock().unwrap().push(Observation {
            phase: "retry",
            account: runtime.active_identity().unwrap().account_id,
            context: context.cloned(),
            retry_key: Some(key.into()),
        });
        SharedTradeSettlementOutcome::Deferred
    }
}

fn offer(account: &str) -> SharedTradeOffer {
    SharedTradeOffer {
        settlement_nonce: "00000000000000000000000000000017".into(),
        account_id: account.into(),
        character_index: 0,
        character_name: account.into(),
        partner_name: "partner".into(),
        gold: 1,
        items: Vec::new(),
    }
}

#[test]
fn zone_economy_trade_retry_keeps_original_context_and_passes_live_runtime_separately() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let service = Arc::new(RuntimeTradeProbe(observed.clone()));
    let state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime_with_account_inventory_service(state, service);
    start_new_runtime(&mut runtime, "economy-runtime-owner", "PermitOwner");
    let original = SharedAccountInventoryExecutionContext {
        zone_id: ZoneId::new("original-zone"),
        fencing_generation: 7,
        source_sequence: 12,
        created_at_ms: 100,
        external_commit_authorized: true,
    };
    let current = SharedAccountInventoryExecutionContext {
        zone_id: ZoneId::new("new-zone"),
        fencing_generation: 8,
        source_sequence: 13,
        ..original.clone()
    };
    runtime.set_economy_execution_context(Some(current.clone()));
    let first = offer("economy-runtime-owner");
    let second = offer("partner");
    assert_eq!(
        runtime.settle_shared_trade(&first, &second),
        SharedTradeSettlementOutcome::Deferred
    );
    assert_eq!(
        runtime.retry_shared_trade(&original, "original-key", &first, &second),
        SharedTradeSettlementOutcome::Deferred
    );
    assert_eq!(
        *observed.lock().unwrap(),
        vec![
            Observation {
                phase: "initial",
                account: "economy-runtime-owner".into(),
                context: Some(current),
                retry_key: None
            },
            Observation {
                phase: "retry",
                account: "economy-runtime-owner".into(),
                context: Some(original),
                retry_key: Some("original-key".into())
            },
        ]
    );
}

#[test]
fn zone_economy_player_trade_uses_runtime_dispatch_and_deferred_offer_rolls_back_once() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let service = Arc::new(RuntimeTradeProbe(observed.clone()));
    let factory =
        Arc::new(SharedInProcessZoneRuntimeFactory::with_account_inventory_service(service));
    let registry = ZoneRegistry::new(ZoneId::primary(), factory);
    let config = GatewayConfig::default();
    let mut first = GatewaySession::new_with_zone_registry(config.clone(), &registry);
    let mut second = GatewaySession::new_with_zone_registry(config, &registry);
    start_demo_character(&mut first);
    start_new_character(&mut second, "runtime-trade-partner", "PermitBob");
    let initial_gold = first.world_snapshot().gold;
    open_shared_trade_pair(&mut first, &mut second);
    first.handle_packet(ClientPacket::TradeGold { amount: 30 });
    first.handle_packet(ClientPacket::TradeConfirm { locked: true });
    second.handle_packet(ClientPacket::TradeConfirm { locked: true });
    assert_eq!(observed.lock().unwrap().len(), 1);
    assert_eq!(observed.lock().unwrap()[0].account, "runtime-trade-partner");
    first.handle_packet(ClientPacket::KeepAlive { time: 111 });
    assert_eq!(first.world_snapshot().gold, initial_gold);
    first.handle_packet(ClientPacket::KeepAlive { time: 112 });
    assert_eq!(first.world_snapshot().gold, initial_gold);
    assert_eq!(observed.lock().unwrap().len(), 1);
}

#[derive(Debug)]
struct CandidateDropUnknownProbe(Arc<Mutex<usize>>);

impl SharedAccountInventoryService for CandidateDropUnknownProbe {
    fn commit(
        &self,
        _: &mut InProcessWorldRuntime,
        _: SharedAccountInventoryCommandEnvelope,
    ) -> SharedAccountInventoryTransactionReceipt {
        panic!("unknown probe needs the original execution context")
    }
    fn commit_fenced(
        &self,
        runtime: &mut InProcessWorldRuntime,
        context: Option<&SharedAccountInventoryExecutionContext>,
        envelope: SharedAccountInventoryCommandEnvelope,
    ) -> SharedAccountInventoryCommitOutcome {
        *self.0.lock().unwrap() += 1;
        let idempotency_key = envelope.stable_idempotency_key();
        // Model the exact hazard: a private candidate exists, but its source
        // publication is not confirmed. No actual database commit is claimed.
        let receipt = InProcessAccountInventoryService::new().commit(runtime, envelope);
        assert!(receipt.committed);
        SharedAccountInventoryCommitOutcome::OutcomeUnknown {
            idempotency_key,
            execution_context: context.unwrap().clone(),
            receipt: SharedAccountInventoryTransactionReceipt {
                committed: false,
                packets: Vec::new(),
                ..receipt
            },
        }
    }
}

#[test]
fn zone_economy_unknown_debit_fences_candidate_before_shared_drop_publication_and_never_replays() {
    let calls = Arc::new(Mutex::new(0));
    let state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime_with_account_inventory_service(
        state.clone(),
        Arc::new(CandidateDropUnknownProbe(calls.clone())),
    );
    start_demo_runtime(&mut runtime);
    let original = runtime.inner.active_character_checkpoint().unwrap();
    let initial = runtime.inner.world_snapshot();
    assert!(initial.gold >= 1);
    let public_count = || {
        state
            .lock()
            .unwrap()
            .maps
            .values()
            .map(|map| map.ground_drops.len())
            .sum::<usize>()
    };
    let before_public = public_count();
    let context = SharedAccountInventoryExecutionContext {
        zone_id: ZoneId::new("test-shared-zone"),
        fencing_generation: 7,
        source_sequence: 55,
        created_at_ms: 100,
        external_commit_authorized: true,
    };
    runtime.set_economy_execution_context(Some(context.clone()));
    let error = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::DropGold {
            amount: 1,
        }))
        .unwrap_err();
    assert!(error.contains("COMMIT_OUTCOME_UNKNOWN"));
    assert_eq!(
        public_count(),
        before_public,
        "private candidate must not reach shared snapshot tail"
    );
    assert!(runtime.inner.world_snapshot().ground_drops.len() > initial.ground_drops.len());
    assert!(runtime.teardown_is_fenced());
    let pending = runtime.unresolved_economy_command.as_ref().unwrap();
    assert_eq!(pending.context.as_ref(), Some(&context));
    assert_eq!(
        pending.idempotency_key,
        pending.envelope.stable_idempotency_key()
    );
    assert!(matches!(
        pending.envelope.command,
        SharedAccountInventoryCommand::GoldDrop {
            amount: 1,
            request_id: 55
        }
    ));
    assert!(runtime
        .execute(WorldCommand::Tick)
        .unwrap_err()
        .contains("COMMIT_OUTCOME_UNKNOWN"));
    assert!(runtime.release_teardown_fence().is_err());
    assert!(runtime
        .prepare_teardown_checkpoint(&ZoneOwnerLease::new(
            ZoneId::new("test-shared-zone"),
            "owner",
            7
        ))
        .is_err());
    assert!(runtime
        .restore_active_character_checkpoint(&original)
        .is_err());
    assert!(runtime.save_active_character().is_err());
    assert_eq!(*calls.lock().unwrap(), 1);
    assert_eq!(public_count(), before_public);
}
