//! Immediate gold custody through the shared gateway, before preparation.
use super::*;

fn funded_gold_pair() -> (GatewaySession, GatewaySession) {
    let (mut a, mut b) = started_shared_zone_sessions();
    let total = a.world_snapshot().gold + b.world_snapshot().gold;
    a.handle_packet(ClientPacket::DropGold { amount: 50 });
    let drop = b
        .world_snapshot()
        .ground_drops
        .into_iter()
        .find(|drop| matches!(drop.loot, GroundDropLootSnapshot::Gold { amount: 50 }))
        .expect("peer sees legitimate funding drop");
    let map = a.world_snapshot().map_file_name.unwrap();
    b.transfer_map(&format!("crystal:{map}:{}:{}", drop.x, drop.y));
    assert_eq!(gained(&b.pick_up(drop.object_id)), vec![50]);
    assert!(b.world_snapshot().gold >= 50);
    assert_eq!(a.world_snapshot().gold + b.world_snapshot().gold, total);
    (a, b)
}

fn lost(packets: &[ServerPacket]) -> Vec<u32> {
    packets
        .iter()
        .filter_map(|p| match p {
            ServerPacket::LoseGold { gold } => Some(*gold),
            _ => None,
        })
        .collect()
}
fn gained(packets: &[ServerPacket]) -> Vec<u32> {
    packets
        .iter()
        .filter_map(|p| match p {
            ServerPacket::GainedGold { gold } => Some(*gold),
            _ => None,
        })
        .collect()
}
fn quoted(packets: &[ServerPacket]) -> Vec<u32> {
    packets
        .iter()
        .filter_map(|p| match p {
            ServerPacket::TradeGold { amount } => Some(*amount),
            _ => None,
        })
        .collect()
}

#[test]
fn repeated_gold_deltas_debit_immediately_and_only_partner_sees_cumulative_quote() {
    let (mut a, mut b) = funded_gold_pair();
    let initial = a.world_snapshot().gold;
    open_shared_trade_pair(&mut a, &mut b);
    for (delta, total) in [(7, 7), (11, 18)] {
        let owner = a.handle_packet(ClientPacket::TradeGold { amount: delta });
        assert_eq!(lost(&owner), vec![delta]);
        assert!(quoted(&owner).is_empty());
        assert_eq!(a.world_snapshot().gold, initial - total);
        assert_eq!(
            a.world_snapshot()
                .stage5_systems
                .trade
                .as_ref()
                .unwrap()
                .offered_gold,
            total
        );
        assert_eq!(
            quoted(&b.handle_packet(ClientPacket::KeepAlive {
                time: 100 + i64::from(total)
            })),
            vec![total]
        );
    }
}

#[test]
fn invalid_gold_deltas_do_not_debit_or_replace_the_held_quote() {
    let (mut a, mut b) = funded_gold_pair();
    open_shared_trade_pair(&mut a, &mut b);
    a.handle_packet(ClientPacket::TradeGold { amount: 7 });
    b.handle_packet(ClientPacket::KeepAlive { time: 1 });
    let wallet = a.world_snapshot().gold;
    for amount in [0, wallet.checked_add(1).unwrap()] {
        let packets = a.handle_packet(ClientPacket::TradeGold { amount });
        assert!(lost(&packets).is_empty());
        assert_eq!(a.world_snapshot().gold, wallet);
        assert_eq!(
            a.world_snapshot()
                .stage5_systems
                .trade
                .as_ref()
                .unwrap()
                .offered_gold,
            7
        );
        assert!(quoted(&b.handle_packet(ClientPacket::KeepAlive { time: 2 })).is_empty());
    }
}

#[test]
fn preparation_never_debits_gold_a_second_time_and_settlement_conserves_total() {
    let (mut a, mut b) = funded_gold_pair();
    let ga = a.world_snapshot().gold;
    let gb = b.world_snapshot().gold;
    open_shared_trade_pair(&mut a, &mut b);
    a.handle_packet(ClientPacket::TradeGold { amount: 7 });
    b.handle_packet(ClientPacket::TradeGold { amount: 11 });
    assert_eq!(a.world_snapshot().gold, ga - 7);
    assert_eq!(b.world_snapshot().gold, gb - 11);
    let first = a.handle_packet(ClientPacket::TradeConfirm { locked: true });
    assert!(lost(&first).is_empty());
    assert!(!first
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeConfirm)));
    let second = b.handle_packet(ClientPacket::TradeConfirm { locked: true });
    assert!(lost(&second).is_empty());
    assert!(second
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeConfirm)));
    a.handle_packet(ClientPacket::KeepAlive { time: 3 });
    assert_eq!(a.world_snapshot().gold, ga - 7 + 11);
    assert_eq!(b.world_snapshot().gold, gb - 11 + 7);
    assert_eq!(a.world_snapshot().gold + b.world_snapshot().gold, ga + gb);
}

#[test]
fn unprepared_self_or_peer_cancel_refunds_each_owner_once() {
    for cancel_owner in [true, false] {
        let (mut a, mut b) = funded_gold_pair();
        let ga = a.world_snapshot().gold;
        let gb = b.world_snapshot().gold;
        open_shared_trade_pair(&mut a, &mut b);
        a.handle_packet(ClientPacket::TradeGold { amount: 7 });
        b.handle_packet(ClientPacket::TradeGold { amount: 11 });
        let (cancelled, peer) = if cancel_owner {
            (&mut a, &mut b)
        } else {
            (&mut b, &mut a)
        };
        let expected = if cancel_owner { 7 } else { 11 };
        let own = cancelled.handle_packet(ClientPacket::TradeCancel);
        assert_eq!(gained(&own), vec![expected]);
        assert_eq!(
            gained(&peer.handle_packet(ClientPacket::KeepAlive { time: 4 })),
            vec![18 - expected]
        );
        for session in [&mut a, &mut b] {
            assert!(session.world_snapshot().stage5_systems.trade.is_none());
            assert!(gained(&session.handle_packet(ClientPacket::TradeCancel)).is_empty());
            assert!(gained(&session.handle_packet(ClientPacket::KeepAlive { time: 5 })).is_empty());
        }
        assert_eq!(a.world_snapshot().gold, ga);
        assert_eq!(b.world_snapshot().gold, gb);
    }
}

#[test]
fn unprepared_logout_or_disconnect_refunds_before_save_and_peer_refunds_once() {
    for leave in [ClientPacket::LogOut, ClientPacket::Disconnect] {
        let (mut a, mut b) = funded_gold_pair();
        let ga = a.world_snapshot().gold;
        let gb = b.world_snapshot().gold;
        open_shared_trade_pair(&mut a, &mut b);
        a.handle_packet(ClientPacket::TradeGold { amount: 7 });
        b.handle_packet(ClientPacket::TradeGold { amount: 11 });
        a.handle_packet(leave);
        assert_eq!(
            gained(&b.handle_packet(ClientPacket::KeepAlive { time: 6 })),
            vec![11]
        );
        assert_eq!(b.world_snapshot().gold, gb);
        assert!(gained(&b.handle_packet(ClientPacket::KeepAlive { time: 7 })).is_empty());
        start_demo_character(&mut a);
        assert_eq!(a.world_snapshot().gold, ga);
        assert!(a.world_snapshot().stage5_systems.trade.is_none());
    }
}

#[test]
fn gold_edits_are_rejected_when_either_participant_has_prepared() {
    let (mut a, mut b) = funded_gold_pair();
    open_shared_trade_pair(&mut a, &mut b);
    a.handle_packet(ClientPacket::TradeGold { amount: 7 });
    b.handle_packet(ClientPacket::TradeGold { amount: 11 });
    a.handle_packet(ClientPacket::TradeConfirm { locked: true });
    for session in [&mut a, &mut b] {
        let before = session.world_snapshot();
        let packets = session.handle_packet(ClientPacket::TradeGold { amount: 1 });
        assert!(lost(&packets).is_empty());
        assert_eq!(session.world_snapshot().gold, before.gold);
        assert_eq!(
            session
                .world_snapshot()
                .stage5_systems
                .trade
                .as_ref()
                .unwrap()
                .offered_gold,
            before.stage5_systems.trade.as_ref().unwrap().offered_gold
        );
    }
    assert!(
        a.world_snapshot()
            .stage5_systems
            .trade
            .as_ref()
            .unwrap()
            .escrow_prepared
    );
    assert!(b
        .handle_packet(ClientPacket::TradeConfirm { locked: true })
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeConfirm)));
}

fn open_runtime_pair(
    a: &mut SharedInProcessZoneSessionRuntime,
    b: &mut SharedInProcessZoneSessionRuntime,
) {
    let ka = a.current_presence_key().unwrap();
    let kb = b.current_presence_key().unwrap();
    {
        let mut state = a.zone_state.lock().unwrap();
        let here = state.players[&ka].entity.clone();
        state.players.get_mut(&ka).unwrap().entity.direction = MirDirection::Right;
        let peer = state.players.get_mut(&kb).unwrap();
        peer.entity.x = here.x + 1;
        peer.entity.y = here.y;
        peer.entity.direction = MirDirection::Left;
    }
    a.execute_shared_trade_request();
    assert!(b
        .execute_shared_trade_reply(true)
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeAccept { .. })));
    a.apply_shared_trade_link_events();
}

#[derive(Debug)]
struct GoldBootstrapProbe {
    balances: Arc<Mutex<Vec<u32>>>,
    allow: bool,
}
impl SharedAccountInventoryService for GoldBootstrapProbe {
    fn commit(
        &self,
        _: &mut InProcessWorldRuntime,
        _: SharedAccountInventoryCommandEnvelope,
    ) -> SharedAccountInventoryTransactionReceipt {
        panic!("gold quote must not settle assets")
    }
    fn bootstrap_fenced(
        &self,
        runtime: &InProcessWorldRuntime,
        _: Option<&SharedAccountInventoryExecutionContext>,
    ) -> bool {
        self.balances
            .lock()
            .unwrap()
            .push(runtime.world_snapshot().gold);
        self.allow
    }
}

#[test]
fn first_gold_debit_bootstraps_the_predebit_balance_and_failure_debits_nothing() {
    for allow in [true, false] {
        let state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let balances = Arc::new(Mutex::new(Vec::new()));
        let service = Arc::new(GoldBootstrapProbe {
            balances: balances.clone(),
            allow,
        });
        let mut a = shared_session_runtime_with_account_inventory_service(state.clone(), service);
        let mut b = shared_session_runtime(state);
        start_demo_runtime(&mut a);
        start_new_runtime(&mut b, "gold-bootstrap-peer", "GoldPeer");
        open_runtime_pair(&mut a, &mut b);
        let gold = a.inner.world_snapshot().gold;
        let packets = a
            .execute(WorldCommand::ClientPacket(ClientPacket::TradeGold {
                amount: 7,
            }))
            .unwrap();
        assert_eq!(balances.lock().unwrap().first().copied(), Some(gold));
        assert_eq!(
            a.inner.world_snapshot().gold,
            if allow { gold - 7 } else { gold }
        );
        assert_eq!(lost(&packets), if allow { vec![7] } else { vec![] });
    }
}

#[test]
fn saved_unprepared_gold_in_a_fresh_zone_refunds_once_but_pending_projection_holds_it() {
    let state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut source = shared_session_runtime(state.clone());
    let mut peer = shared_session_runtime(state);
    start_demo_runtime(&mut source);
    start_new_runtime(&mut peer, "gold-restart-peer", "GoldPeer");
    open_runtime_pair(&mut source, &mut peer);
    let gold = source.inner.world_snapshot().gold;
    source
        .execute(WorldCommand::ClientPacket(ClientPacket::TradeGold {
            amount: 7,
        }))
        .unwrap();
    let saved = source.inner.active_character_checkpoint().unwrap();
    assert_eq!(saved.gold, gold - 7);
    for initially_pending in [false, true] {
        let pending = Arc::new(Mutex::new(initially_pending));
        let service = Arc::new(TradeProjectionReconciliationProbe {
            calls: Arc::new(Mutex::new(0)),
            pending: pending.clone(),
        });
        let mut restored = shared_session_runtime_with_account_inventory_service(
            Arc::new(Mutex::new(SharedInProcessZoneState::new())),
            service,
        );
        start_demo_runtime(&mut restored);
        restored
            .inner
            .restore_active_character_checkpoint(&saved)
            .unwrap();
        let first = restored.apply_pending_shared_trade_packets();
        if initially_pending {
            assert!(gained(&first).is_empty());
            assert_eq!(restored.inner.world_snapshot().gold, gold - 7);
            assert!(restored.inner.has_active_shared_trade_state());
            *pending.lock().unwrap() = false;
            assert_eq!(
                gained(&restored.apply_pending_shared_trade_packets()),
                vec![7]
            );
        } else {
            assert_eq!(gained(&first), vec![7]);
        }
        assert_eq!(restored.inner.world_snapshot().gold, gold);
        assert!(!restored.inner.has_active_shared_trade_state());
        let refunded = restored.inner.active_character_checkpoint().unwrap();
        restored
            .inner
            .restore_active_character_checkpoint(&refunded)
            .unwrap();
        assert!(gained(&restored.apply_pending_shared_trade_packets()).is_empty());
        assert_eq!(restored.inner.world_snapshot().gold, gold);
    }
}

#[test]
fn prepared_refund_overflow_retains_final_offer_until_capacity_allows_one_refund() {
    let state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut a = shared_session_runtime(state.clone());
    let mut b = shared_session_runtime(state.clone());
    start_demo_runtime(&mut a);
    start_new_runtime(&mut b, "gold-cap-peer", "GoldPeer");
    open_runtime_pair(&mut a, &mut b);
    a.execute(WorldCommand::ClientPacket(ClientPacket::TradeGold {
        amount: 7,
    }))
    .unwrap();
    a.execute_shared_trade_confirm(true);
    let key = a.current_presence_key().unwrap();
    let mut held = a.inner.active_character_checkpoint().unwrap();
    held.gold = u32::MAX;
    a.inner.restore_active_character_checkpoint(&held).unwrap();
    let failed = a.cancel_pending_shared_trade_offers();
    assert!(gained(&failed).is_empty());
    assert_eq!(a.inner.world_snapshot().gold, u32::MAX);
    assert!(a.inner.has_active_shared_trade_state());
    assert_eq!(
        state
            .lock()
            .unwrap()
            .pending_trade_rollbacks
            .get(&key)
            .unwrap()
            .len(),
        1
    );
    assert!(gained(&a.apply_pending_shared_trade_packets()).is_empty());
    assert_eq!(
        state
            .lock()
            .unwrap()
            .pending_trade_rollbacks
            .get(&key)
            .unwrap()
            .len(),
        1
    );

    // Model spending unrelated gold while custody is still held, without
    // changing the saved trade nonce or its already-debited preparation.
    let mut room = a.inner.active_character_checkpoint().unwrap();
    room.gold = u32::MAX - 7;
    a.inner.restore_active_character_checkpoint(&room).unwrap();
    assert_eq!(gained(&a.apply_pending_shared_trade_packets()), vec![7]);
    assert_eq!(a.inner.world_snapshot().gold, u32::MAX);
    assert!(!a.inner.has_active_shared_trade_state());
    assert!(state
        .lock()
        .unwrap()
        .pending_trade_rollbacks
        .get(&key)
        .is_none_or(Vec::is_empty));
    assert!(gained(&a.apply_pending_shared_trade_packets()).is_empty());
    assert_eq!(a.inner.world_snapshot().gold, u32::MAX);
}

#[test]
fn peer_preparation_rejects_item_retrieval_and_deposit_without_changing_accepted_offer() {
    let (mut a, mut b) = funded_gold_pair();
    let ga = a.world_snapshot().gold;
    let gb = b.world_snapshot().gold;
    open_shared_trade_pair(&mut a, &mut b);
    let red_slot = inventory_slot_for_key(&a, "red-potion");
    let original = a
        .world_snapshot()
        .inventory_items
        .into_iter()
        .find(|item| item.key == "red-potion")
        .unwrap();
    let offered = a.handle_packet(ClientPacket::DepositTradeItem {
        from: red_slot,
        to: 3,
    });
    assert!(offered
        .iter()
        .any(|p| matches!(p, ServerPacket::DepositTradeItem { success: true, .. })));
    b.handle_packet(ClientPacket::TradeGold { amount: 11 });
    b.handle_packet(ClientPacket::TradeConfirm { locked: true });
    // Drain the legitimate earlier quote before attributing packets to the
    // attempted edit. Every owner command also drains queued peer packets.
    assert_eq!(
        quoted(&a.handle_packet(ClientPacket::KeepAlive { time: 29 })),
        vec![11]
    );
    assert!(
        b.world_snapshot()
            .stage5_systems
            .trade
            .as_ref()
            .unwrap()
            .escrow_prepared
    );
    let before = a.world_snapshot().stage5_systems.trade.unwrap();

    let retrieve = a.handle_packet(ClientPacket::RetrieveTradeItem {
        from: 3,
        to: red_slot,
    });
    assert!(retrieve.iter().any(|p| matches!(p,
        ServerPacket::RetrieveTradeItem { from: 3, to, success: false } if *to == red_slot)));
    assert!(quoted(&retrieve).is_empty());
    assert!(!retrieve
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeItem { .. })));
    let deposit = a.handle_packet(ClientPacket::DepositTradeItem {
        from: red_slot,
        to: 4,
    });
    assert!(deposit.iter().any(|p| matches!(p,
        ServerPacket::DepositTradeItem { from, to: 4, success: false } if *from == red_slot)));
    assert!(!deposit
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeItem { .. })));
    let after = a.world_snapshot().stage5_systems.trade.unwrap();
    assert_eq!(after.offered_slots, before.offered_slots);
    assert_eq!(after.offered_unique_ids, before.offered_unique_ids);
    assert_eq!(after.offered_items, before.offered_items);
    assert!(
        b.world_snapshot()
            .stage5_systems
            .trade
            .as_ref()
            .unwrap()
            .escrow_prepared
    );

    assert!(a
        .handle_packet(ClientPacket::TradeConfirm { locked: true })
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeConfirm)));
    b.handle_packet(ClientPacket::KeepAlive { time: 30 });
    assert_eq!(a.world_snapshot().gold, ga + 11);
    assert_eq!(b.world_snapshot().gold, gb - 11);
    assert!(!a
        .world_snapshot()
        .inventory_items
        .iter()
        .any(|item| item.key == "red-potion"));
    assert!(b
        .world_snapshot()
        .inventory_items
        .iter()
        .any(|item| item.key == original.key && item.quantity == original.quantity));
    assert!(a.world_snapshot().stage5_systems.trade.is_none());
    assert!(b.world_snapshot().stage5_systems.trade.is_none());
}
