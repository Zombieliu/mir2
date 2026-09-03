//! The real shared coordinator must not expose its prepare phase as Crystal's
//! completed-exchange packet. These tests use two sessions and actual routing.
use super::*;

fn inventory_totals(
    first: &GatewaySession,
    second: &GatewaySession,
) -> std::collections::BTreeMap<String, u64> {
    let mut totals = std::collections::BTreeMap::new();
    for item in first
        .world_snapshot()
        .inventory_items
        .into_iter()
        .chain(second.world_snapshot().inventory_items)
    {
        *totals.entry(item.key).or_insert(0) += u64::from(item.quantity);
    }
    totals
}

fn completion_count(packets: &[ServerPacket]) -> usize {
    packets
        .iter()
        .filter(|p| matches!(p, ServerPacket::TradeConfirm))
        .count()
}

fn open_pair(first: &mut GatewaySession, second: &mut GatewaySession) {
    first.handle_packet(ClientPacket::TradeRequest);
    second.handle_packet(ClientPacket::TradeRequest);
    for session in [first, second] {
        assert!(session
            .handle_packet(ClientPacket::TradeReply {
                accept_invite: true
            })
            .iter()
            .any(|p| matches!(p, ServerPacket::TradeAccept { .. })));
    }
}

fn prepare_first(first: &mut GatewaySession) -> Vec<ServerPacket> {
    first.handle_packet(ClientPacket::TradeGold { amount: 30 });
    let slot = inventory_slot_for_key(first, "red-potion");
    assert!(first
        .handle_packet(ClientPacket::DepositTradeItem { from: slot, to: 3 })
        .iter()
        .any(|p| matches!(p, ServerPacket::DepositTradeItem { success: true, .. })));
    let packets = first.handle_packet(ClientPacket::TradeConfirm { locked: true });
    assert_eq!(completion_count(&packets), 0);
    let trade = first.world_snapshot().stage5_systems.trade.unwrap();
    assert!(trade.escrow_prepared && trade.locked && !trade.completed);
    packets
}

#[test]
fn each_party_completes_once_only_after_its_committed_delivery() {
    let (mut first, mut second) = started_shared_zone_sessions();
    let first_gold = first.world_snapshot().gold;
    let total_gold = first_gold + second.world_snapshot().gold;
    let total_items = inventory_totals(&first, &second);
    open_pair(&mut first, &mut second);
    prepare_first(&mut first);
    assert_eq!(first.world_snapshot().gold, first_gold - 30);
    assert!(!has_inventory_key(&first, "red-potion"));
    assert_eq!(
        completion_count(&first.handle_packet(ClientPacket::KeepAlive { time: 501 })),
        0
    );
    assert_eq!(
        completion_count(&first.handle_packet(ClientPacket::TradeConfirm { locked: true })),
        0
    );
    assert_eq!(first.world_snapshot().gold, first_gold - 30);

    let second_packets = second.handle_packet(ClientPacket::TradeConfirm { locked: true });
    assert_eq!(completion_count(&second_packets), 1);
    let complete_at = second_packets
        .iter()
        .position(|p| matches!(p, ServerPacket::TradeConfirm))
        .unwrap();
    assert!(second_packets
        .iter()
        .enumerate()
        .filter(|(_, p)| matches!(
            p,
            ServerPacket::GainedGold { .. } | ServerPacket::GainedItem { .. }
        ))
        .all(|(i, _)| i < complete_at));
    assert!(has_inventory_key(&second, "red-potion"));
    assert!(second.world_snapshot().stage5_systems.trade.is_none());
    assert_eq!(
        completion_count(&first.handle_packet(ClientPacket::KeepAlive { time: 502 })),
        1
    );
    assert!(first.world_snapshot().stage5_systems.trade.is_none());
    assert_eq!(
        first.world_snapshot().gold + second.world_snapshot().gold,
        total_gold
    );
    assert_eq!(inventory_totals(&first, &second), total_items);
    for session in [&mut first, &mut second] {
        let repeated = session.handle_packet(ClientPacket::KeepAlive { time: 503 });
        assert_eq!(completion_count(&repeated), 0);
        assert!(!repeated.iter().any(|p| matches!(
            p,
            ServerPacket::GainedGold { .. } | ServerPacket::GainedItem { .. }
        )));
    }
}

#[test]
fn cancellation_refunds_waiting_offer_once_without_any_completion() {
    let (mut first, mut second) = started_shared_zone_sessions();
    let gold = first.world_snapshot().gold;
    let total_items = inventory_totals(&first, &second);
    open_pair(&mut first, &mut second);
    prepare_first(&mut first);
    assert_eq!(
        completion_count(&second.handle_packet(ClientPacket::TradeCancel)),
        0
    );
    let refund = first.handle_packet(ClientPacket::KeepAlive { time: 504 });
    assert_eq!(completion_count(&refund), 0);
    assert!(refund
        .iter()
        .any(|p| matches!(p, ServerPacket::GainedGold { gold: 30 })));
    assert!(refund
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeCancel { unlock: false })));
    assert_eq!(first.world_snapshot().gold, gold);
    assert!(has_inventory_key(&first, "red-potion"));
    assert_eq!(inventory_totals(&first, &second), total_items);
    let repeated = first.handle_packet(ClientPacket::KeepAlive { time: 505 });
    assert_eq!(completion_count(&repeated), 0);
    assert!(!repeated.iter().any(|p| matches!(
        p,
        ServerPacket::GainedGold { .. } | ServerPacket::GainedItem { .. }
    )));
    assert_eq!(first.world_snapshot().gold, gold);
}

#[test]
fn receiver_capacity_rejection_refunds_both_sides_without_completion() {
    let (mut first, mut second) = started_shared_zone_sessions();
    fill_gateway_bag(&mut second);
    let first_gold = first.world_snapshot().gold;
    let second_gold = second.world_snapshot().gold;
    let total_items = inventory_totals(&first, &second);
    open_pair(&mut first, &mut second);
    prepare_first(&mut first);
    let rejected = second.handle_packet(ClientPacket::TradeConfirm { locked: true });
    assert_eq!(completion_count(&rejected), 0);
    assert!(rejected
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeCancel { unlock: false })));
    let refund = first.handle_packet(ClientPacket::KeepAlive { time: 506 });
    assert_eq!(completion_count(&refund), 0);
    assert_eq!(first.world_snapshot().gold, first_gold);
    assert_eq!(second.world_snapshot().gold, second_gold);
    assert!(has_inventory_key(&first, "red-potion"));
    assert_eq!(inventory_totals(&first, &second), total_items);
}

#[test]
fn unknown_outcome_holds_both_offers_until_definitive_commit_or_rejection() {
    for commits in [true, false] {
        let unresolved = Arc::new(Mutex::new(true));
        let calls = Arc::new(Mutex::new(0));
        let service: SharedAccountInventoryServiceHandle = if commits {
            Arc::new(UnknownThenCommittedTradeSettlementService {
                unresolved: Arc::clone(&unresolved),
                calls: Arc::clone(&calls),
            })
        } else {
            Arc::new(UnknownThenRejectedTradeSettlementService {
                unresolved: Arc::clone(&unresolved),
                calls: Arc::clone(&calls),
            })
        };
        let factory =
            Arc::new(SharedInProcessZoneRuntimeFactory::with_account_inventory_service(service));
        let registry = ZoneRegistry::new(
            ZoneId::primary(),
            Arc::clone(&factory) as SharedZoneRuntimeFactory,
        );
        let config = GatewayConfig::default();
        let mut first = GatewaySession::new_with_zone_registry(config.clone(), &registry);
        let mut second = GatewaySession::new_with_zone_registry(config, &registry);
        start_demo_character(&mut first);
        start_new_character(&mut second, "trade-completion-peer", "PhasePeer");
        let first_gold = first.world_snapshot().gold;
        let second_gold = second.world_snapshot().gold;
        let total_items = inventory_totals(&first, &second);
        open_pair(&mut first, &mut second);
        prepare_first(&mut first);
        let unknown = second
            .execute_production_player_command(
                true,
                WorldCommand::ClientPacket(ClientPacket::TradeConfirm { locked: true }),
            )
            .unwrap()
            .packets;
        assert_eq!(completion_count(&unknown), 0);
        assert!(!unknown.iter().any(|p| matches!(
            p,
            ServerPacket::GainedGold { .. } | ServerPacket::GainedItem { .. }
        )));
        for session in [&mut first, &mut second] {
            let trade = session.world_snapshot().stage5_systems.trade.unwrap();
            assert!(trade.escrow_prepared && !trade.completed);
            let cancel = session.handle_packet(ClientPacket::TradeCancel);
            assert_eq!(completion_count(&cancel), 0);
            assert!(!cancel.iter().any(|p| matches!(
                p,
                ServerPacket::TradeCancel { .. } | ServerPacket::GainedGold { .. }
            )));
            assert!(session.world_snapshot().stage5_systems.trade.is_some());
        }
        assert_eq!(first.world_snapshot().gold, first_gold - 30);
        assert_eq!(second.world_snapshot().gold, second_gold);
        *unresolved.lock().unwrap() = false;
        let resolved_first = first
            .execute_production_player_command(
                true,
                WorldCommand::ClientPacket(ClientPacket::KeepAlive { time: 507 }),
            )
            .unwrap()
            .packets;
        let resolved_second = second
            .execute_production_player_command(
                true,
                WorldCommand::ClientPacket(ClientPacket::KeepAlive { time: 508 }),
            )
            .unwrap()
            .packets;
        assert_eq!(completion_count(&resolved_first), usize::from(commits));
        assert_eq!(completion_count(&resolved_second), usize::from(commits));
        assert_eq!(
            first.world_snapshot().gold + second.world_snapshot().gold,
            first_gold + second_gold
        );
        assert_eq!(has_inventory_key(&first, "red-potion"), !commits);
        assert_eq!(has_inventory_key(&second, "red-potion"), commits);
        assert_eq!(inventory_totals(&first, &second), total_items);
        for session in [&mut first, &mut second] {
            let repeat = session.handle_packet(ClientPacket::KeepAlive { time: 509 });
            assert_eq!(completion_count(&repeat), 0);
            assert!(!repeat.iter().any(|p| matches!(
                p,
                ServerPacket::GainedGold { .. } | ServerPacket::GainedItem { .. }
            )));
        }
        let resources = factory.resources_for_zone(&ZoneId::primary());
        let state = resources.zone_state.lock().unwrap();
        assert!(state.unresolved_trade_settlements.is_empty());
        assert!(state.pending_trade_deliveries.is_empty());
        assert!(state.pending_trade_rollbacks.is_empty());
    }
}

#[test]
fn unfenced_deferred_settlement_never_announces_completion() {
    let calls = Arc::new(Mutex::new(0));
    let service = Arc::new(UnknownThenRejectedTradeSettlementService {
        unresolved: Arc::new(Mutex::new(true)),
        calls: Arc::clone(&calls),
    }) as SharedAccountInventoryServiceHandle;
    let registry = ZoneRegistry::new(
        ZoneId::primary(),
        Arc::new(SharedInProcessZoneRuntimeFactory::with_account_inventory_service(service))
            as SharedZoneRuntimeFactory,
    );
    let config = GatewayConfig::default();
    let mut first = GatewaySession::new_with_zone_registry(config.clone(), &registry);
    let mut second = GatewaySession::new_with_zone_registry(config, &registry);
    start_demo_character(&mut first);
    start_new_character(&mut second, "trade-deferred-phase", "DeferPeer");
    let first_gold = first.world_snapshot().gold;
    let total_items = inventory_totals(&first, &second);
    open_pair(&mut first, &mut second);
    prepare_first(&mut first);
    assert_eq!(
        completion_count(&second.handle_packet(ClientPacket::TradeConfirm { locked: true })),
        0
    );
    assert_eq!(
        completion_count(&first.handle_packet(ClientPacket::KeepAlive { time: 510 })),
        0
    );
    assert_eq!(first.world_snapshot().gold, first_gold);
    assert!(has_inventory_key(&first, "red-potion"));
    assert_eq!(inventory_totals(&first, &second), total_items);
    assert_eq!(*calls.lock().unwrap(), 1);
}
