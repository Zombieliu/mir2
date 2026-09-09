//! Trade ownership is established by the recipient's reply, not by a name in
//! a personal session. These tests exercise actual shared gateway routing.
use super::*;

fn is_trade_packet(packet: &ServerPacket) -> bool {
    matches!(
        packet,
        ServerPacket::TradeRequest { .. }
            | ServerPacket::TradeAccept { .. }
            | ServerPacket::TradeGold { .. }
            | ServerPacket::TradeItem { .. }
            | ServerPacket::TradeCancel { .. }
            | ServerPacket::TradeConfirm
    )
}

#[test]
fn unsolicited_reply_and_edits_cannot_create_a_trade() {
    let (mut first, mut second) = started_shared_zone_sessions();
    for packet in [
        ClientPacket::TradeReply {
            accept_invite: true,
        },
        ClientPacket::TradeGold { amount: 10 },
        ClientPacket::TradeConfirm { locked: true },
    ] {
        assert!(!first.handle_packet(packet).iter().any(is_trade_packet));
    }
    assert!(first.world_snapshot().stage5_systems.trade.is_none());
    assert!(!second
        .handle_packet(ClientPacket::KeepAlive { time: 1 })
        .iter()
        .any(is_trade_packet));
}

#[test]
fn only_recipient_receives_invitation_and_decline_notifies_only_inviter() {
    let (mut first, mut second) = started_shared_zone_sessions();
    face_shared_trade_pair(&mut first, &mut second);
    assert!(!first
        .handle_packet(ClientPacket::TradeRequest)
        .iter()
        .any(is_trade_packet));
    assert!(second
        .handle_packet(ClientPacket::KeepAlive { time: 1 })
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeRequest { name } if name != "Blade")));
    assert!(!first
        .handle_packet(ClientPacket::TradeReply {
            accept_invite: true
        })
        .iter()
        .any(is_trade_packet));
    assert!(!second
        .handle_packet(ClientPacket::TradeReply {
            accept_invite: false
        })
        .iter()
        .any(is_trade_packet));
    assert!(first
        .handle_packet(ClientPacket::KeepAlive { time: 2 })
        .iter()
        .any(|p| matches!(p, ServerPacket::Chat { message, .. } if message.contains("Blade"))));
    assert!(first.world_snapshot().stage5_systems.trade.is_none());
    assert!(second.world_snapshot().stage5_systems.trade.is_none());
    assert!(!second
        .handle_packet(ClientPacket::TradeReply {
            accept_invite: true
        })
        .iter()
        .any(is_trade_packet));
}

#[test]
fn invitation_requires_mutual_facing() {
    let (mut first, mut second) = started_shared_zone_sessions();
    face_shared_trade_pair(&mut first, &mut second);
    second.handle_packet(ClientPacket::Turn {
        direction: MirDirection::Right,
    });
    first.handle_packet(ClientPacket::TradeRequest);
    assert!(!second
        .handle_packet(ClientPacket::KeepAlive { time: 1 })
        .iter()
        .any(is_trade_packet));
}

#[test]
fn unprepared_pair_cancel_closes_both_personal_states() {
    let (mut first, mut second) = started_shared_zone_sessions();
    open_shared_trade_pair(&mut first, &mut second);
    assert!(first
        .handle_packet(ClientPacket::TradeCancel)
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeCancel { unlock: false })));
    assert!(second
        .handle_packet(ClientPacket::KeepAlive { time: 1 })
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeCancel { unlock: false })));
    assert!(first.world_snapshot().stage5_systems.trade.is_none());
    assert!(second.world_snapshot().stage5_systems.trade.is_none());
}

#[test]
fn disconnected_inviter_cannot_be_accepted_and_new_invite_is_possible() {
    let (mut first, mut second) = started_shared_zone_sessions();
    face_shared_trade_pair(&mut first, &mut second);
    first.handle_packet(ClientPacket::TradeRequest);
    second.handle_packet(ClientPacket::KeepAlive { time: 1 });
    first.handle_packet(ClientPacket::LogOut);
    first.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    first.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(!second
        .handle_packet(ClientPacket::TradeReply {
            accept_invite: true
        })
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeAccept { .. })));
    assert!(second.world_snapshot().stage5_systems.trade.is_none());
    open_shared_trade_pair(&mut first, &mut second);
}

#[test]
fn gold_and_items_are_private_to_the_established_partner() {
    let registry = ZoneRegistry::in_process();
    let mut first = GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
    let mut second = GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
    let mut third = GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
    start_demo_character(&mut first);
    start_new_character(&mut second, "trade-second", "Blade");
    start_new_character(&mut third, "trade-third", "Watcher");
    let total_gold =
        first.world_snapshot().gold + second.world_snapshot().gold + third.world_snapshot().gold;
    open_shared_trade_pair(&mut first, &mut second);
    assert!(!third
        .handle_packet(ClientPacket::KeepAlive { time: 1 })
        .iter()
        .any(is_trade_packet));
    assert!(!first
        .handle_packet(ClientPacket::TradeGold { amount: 17 })
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeGold { .. })));
    assert!(second
        .handle_packet(ClientPacket::KeepAlive { time: 2 })
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeGold { amount: 17 })));
    let from = inventory_slot_for_key(&first, "red-potion");
    let packets = first.handle_packet(ClientPacket::DepositTradeItem { from, to: 3 });
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::DepositTradeItem { success: true, .. })));
    assert!(!packets
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeItem { .. })));
    assert!(second
        .handle_packet(ClientPacket::KeepAlive { time: 3 })
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeItem { .. })));
    assert!(!third
        .handle_packet(ClientPacket::KeepAlive { time: 4 })
        .iter()
        .any(is_trade_packet));
    first.handle_packet(ClientPacket::TradeConfirm { locked: true });
    assert!(
        first
            .world_snapshot()
            .stage5_systems
            .trade
            .as_ref()
            .unwrap()
            .escrow_prepared
    );
    third.handle_packet(ClientPacket::TradeRequest);
    third.handle_packet(ClientPacket::TradeReply {
        accept_invite: true,
    });
    third.handle_packet(ClientPacket::TradeConfirm { locked: true });
    third.handle_packet(ClientPacket::TradeCancel);
    assert!(third.world_snapshot().stage5_systems.trade.is_none());
    assert!(first.world_snapshot().stage5_systems.trade.is_some());
    assert!(second.world_snapshot().stage5_systems.trade.is_some());
    assert!(second
        .handle_packet(ClientPacket::TradeConfirm { locked: true })
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeConfirm)));
    first.handle_packet(ClientPacket::KeepAlive { time: 5 });
    assert_eq!(
        first.world_snapshot().gold + second.world_snapshot().gold + third.world_snapshot().gold,
        total_gold
    );
}

#[test]
fn editable_unlock_keeps_pair_and_private_edit_routing() {
    let (mut first, mut second) = started_shared_zone_sessions();
    open_shared_trade_pair(&mut first, &mut second);
    assert!(first
        .handle_packet(ClientPacket::TradeConfirm { locked: false })
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeCancel { unlock: true })));
    assert!(!second
        .handle_packet(ClientPacket::KeepAlive { time: 1 })
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeCancel { unlock: false })));
    first.handle_packet(ClientPacket::TradeGold { amount: 7 });
    assert!(second
        .handle_packet(ClientPacket::KeepAlive { time: 2 })
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeGold { amount: 7 })));
    first.handle_packet(ClientPacket::TradeConfirm { locked: true });
    assert!(second
        .handle_packet(ClientPacket::TradeConfirm { locked: true })
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeConfirm)));
}

#[test]
fn prepared_unlock_cancels_and_refunds_both_sides_without_stale_open_pair() {
    let (mut first, mut second) = started_shared_zone_sessions();
    let gold = first.world_snapshot().gold;
    open_shared_trade_pair(&mut first, &mut second);
    first.handle_packet(ClientPacket::TradeGold { amount: 7 });
    first.handle_packet(ClientPacket::TradeConfirm { locked: true });
    assert_eq!(first.world_snapshot().gold, gold - 7);
    assert!(second
        .handle_packet(ClientPacket::TradeConfirm { locked: false })
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeCancel { unlock: false })));
    first.handle_packet(ClientPacket::KeepAlive { time: 1 });
    assert_eq!(first.world_snapshot().gold, gold);
    assert!(first.world_snapshot().stage5_systems.trade.is_none());
    assert!(second.world_snapshot().stage5_systems.trade.is_none());
}

#[test]
fn disabled_trade_preference_rejects_invites() {
    let (mut first, mut second) = started_shared_zone_sessions();
    face_shared_trade_pair(&mut first, &mut second);
    second.handle_packet(ClientPacket::Chat {
        message: "@ALLOWTRADE".to_string(),
        linked_items: Vec::new(),
    });
    first.handle_packet(ClientPacket::TradeRequest);
    assert!(!second
        .handle_packet(ClientPacket::KeepAlive { time: 1 })
        .iter()
        .any(is_trade_packet));
}

#[test]
fn teardown_fenced_recipient_rejects_request_and_inviter_rejects_acceptance() {
    let state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut first = shared_session_runtime(state.clone());
    let mut second = shared_session_runtime(state.clone());
    start_demo_runtime(&mut first);
    start_new_runtime(&mut second, "fenced-trade-peer", "Blade");
    let first_key = first.current_presence_key().unwrap();
    let second_key = second.current_presence_key().unwrap();
    {
        let mut shared = state.lock().unwrap();
        let here = shared.players[&first_key].entity.clone();
        shared.players.get_mut(&first_key).unwrap().entity.direction = MirDirection::Right;
        let peer = shared.players.get_mut(&second_key).unwrap();
        peer.entity.x = here.x + 1;
        peer.entity.y = here.y;
        peer.entity.direction = MirDirection::Left;
        shared.teardown_fences.insert(second_key.clone());
    }
    first.execute_shared_trade_request();
    assert!(state.lock().unwrap().trade_links.invitations.is_empty());
    state.lock().unwrap().teardown_fences.remove(&second_key);
    first.execute_shared_trade_request();
    assert_eq!(state.lock().unwrap().trade_links.invitations.len(), 1);
    state.lock().unwrap().teardown_fences.insert(first_key);
    assert!(second.execute_shared_trade_reply(true).is_empty());
    assert!(state.lock().unwrap().trade_links.pairs.is_empty());
}

#[test]
fn world_checkpoint_drops_all_online_trade_links() {
    let state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut first = shared_session_runtime(state.clone());
    let mut second = shared_session_runtime(state.clone());
    start_demo_runtime(&mut first);
    start_new_runtime(&mut second, "checkpoint-trade-peer", "Blade");
    let first_key = first.current_presence_key().unwrap();
    let second_key = second.current_presence_key().unwrap();
    let mut shared = state.lock().unwrap();
    let peer = super::super::SharedTradePeer {
        key: second_key.clone(),
        owner_object_id: shared.players[&first_key].zone_object_id,
        peer_object_id: shared.players[&second_key].zone_object_id,
    };
    shared
        .trade_links
        .invitations
        .push((first_key.clone(), peer.clone()));
    shared.trade_links.pairs.push((first_key.clone(), peer));
    shared
        .trade_links
        .initializations
        .push((first_key.clone(), "Blade".into()));
    shared.trade_links.cancellations.push(first_key.clone());
    shared
        .trade_links
        .refusals
        .push((first_key, "Blade".into()));
    let checkpoint = shared.world_checkpoint().unwrap();
    assert!(checkpoint.trade_links.invitations.is_empty());
    assert!(checkpoint.trade_links.pairs.is_empty());
    assert!(checkpoint.trade_links.initializations.is_empty());
    assert!(checkpoint.trade_links.cancellations.is_empty());
    assert!(checkpoint.trade_links.refusals.is_empty());
}

#[test]
fn pending_old_pair_cancellation_blocks_a_new_invitation_until_drained() {
    let state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut first = shared_session_runtime(state.clone());
    let mut second = shared_session_runtime(state.clone());
    let mut third = shared_session_runtime(state.clone());
    start_demo_runtime(&mut first);
    start_new_runtime(&mut second, "pending-peer", "Blade");
    start_new_runtime(&mut third, "pending-third", "Watcher");
    let a = first.current_presence_key().unwrap();
    let b = second.current_presence_key().unwrap();
    let c = third.current_presence_key().unwrap();
    {
        let mut shared = state.lock().unwrap();
        let here = shared.players[&a].entity.clone();
        shared.players.get_mut(&a).unwrap().entity.direction = MirDirection::Right;
        let peer = shared.players.get_mut(&b).unwrap();
        peer.entity.x = here.x + 1;
        peer.entity.y = here.y;
        peer.entity.direction = MirDirection::Left;
        let third = shared.players.get_mut(&c).unwrap();
        third.entity.x = here.x + 2;
        third.entity.y = here.y;
        third.entity.direction = MirDirection::Left;
    }
    first.execute_shared_trade_request();
    second.execute_shared_trade_reply(true);
    first.apply_shared_trade_link_events();
    first.cancel_pending_shared_trade_offers();
    state
        .lock()
        .unwrap()
        .players
        .get_mut(&b)
        .unwrap()
        .entity
        .direction = MirDirection::Right;
    third.execute_shared_trade_request();
    assert!(state.lock().unwrap().trade_links.invitations.is_empty());
    assert!(second
        .apply_shared_trade_link_events()
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeCancel { unlock: false })));
    third.execute_shared_trade_request();
    assert_eq!(state.lock().unwrap().trade_links.invitations.len(), 1);
}

#[test]
fn bootstrap_failure_closes_pair_and_refunds_the_waiting_peer() {
    #[derive(Debug)]
    struct RejectBootstrap;
    impl super::super::SharedAccountInventoryService for RejectBootstrap {
        fn commit(
            &self,
            _: &mut InProcessWorldRuntime,
            _: super::super::SharedAccountInventoryCommandEnvelope,
        ) -> mir2_simulation::SharedAccountInventoryTransactionReceipt {
            panic!("failed bootstrap must not commit assets");
        }
        fn bootstrap_fenced(
            &self,
            _: &InProcessWorldRuntime,
            _: Option<&super::super::SharedAccountInventoryExecutionContext>,
        ) -> bool {
            false
        }
    }
    let state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut first = shared_session_runtime(state.clone());
    let mut second = shared_session_runtime(state.clone());
    start_demo_runtime(&mut first);
    start_new_runtime(&mut second, "bootstrap-peer", "Blade");
    let a = first.current_presence_key().unwrap();
    let b = second.current_presence_key().unwrap();
    {
        let mut shared = state.lock().unwrap();
        let here = shared.players[&a].entity.clone();
        shared.players.get_mut(&a).unwrap().entity.direction = MirDirection::Right;
        let peer = shared.players.get_mut(&b).unwrap();
        peer.entity.x = here.x + 1;
        peer.entity.y = here.y;
        peer.entity.direction = MirDirection::Left;
    }
    first.execute_shared_trade_request();
    second.execute_shared_trade_reply(true);
    first.apply_shared_trade_link_events();
    let gold = first.inner.world_snapshot().gold;
    first
        .inner
        .execute(WorldCommand::ClientPacket(ClientPacket::TradeGold {
            amount: 7,
        }))
        .unwrap();
    first.execute_shared_trade_confirm(true);
    assert_eq!(first.inner.world_snapshot().gold, gold - 7);
    second.account_inventory_service = Arc::new(RejectBootstrap);
    assert!(second
        .execute_shared_trade_confirm(true)
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeCancel { unlock: false })));
    first.apply_pending_shared_trade_packets();
    assert_eq!(first.inner.world_snapshot().gold, gold);
    assert!(!first.inner.has_active_shared_trade_state());
    assert!(!second.inner.has_active_shared_trade_state());
    assert!(state.lock().unwrap().trade_links.pairs.is_empty());
}
