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
    // The helper's Left turn starts Crystal's normal 350 ms turn cadence.
    // Test the opposite facing after it is eligible, not a queued turn intent.
    std::thread::sleep(std::time::Duration::from_millis(400));
    let turn_packets = second.handle_packet(ClientPacket::Turn {
        direction: MirDirection::Right,
    });
    let facing = second
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .expect("recipient must remain active")
        .direction;
    assert_eq!(
        facing,
        MirDirection::Right,
        "fixture turn must reach authoritative state: {turn_packets:?}"
    );
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
fn recipient_cancel_before_inviter_initializes_follows_queued_accept_and_offer() {
    let (mut recipient, mut inviter) = started_shared_zone_sessions();
    face_shared_trade_pair(&mut recipient, &mut inviter);
    let before = recipient.world_snapshot();
    let recipient_name = before
        .entities
        .iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .unwrap()
        .name
        .clone();

    inviter.handle_packet(ClientPacket::TradeRequest);
    assert!(recipient
        .handle_packet(ClientPacket::TradeReply {
            accept_invite: true
        })
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeAccept { .. })));
    let from = inventory_slot_for_key(&recipient, "red-potion");
    assert!(recipient
        .handle_packet(ClientPacket::DepositTradeItem { from, to: 0 })
        .iter()
        .any(|p| matches!(p, ServerPacket::DepositTradeItem { success: true, .. })));

    // The recipient can finish while the inviter has yet to drain its queue.
    assert!(inviter.world_snapshot().stage5_systems.trade.is_none());
    assert!(recipient
        .handle_packet(ClientPacket::TradeCancel)
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeCancel { unlock: false })));
    let observed = inviter
        .handle_packet(ClientPacket::KeepAlive { time: 1 })
        .into_iter()
        .filter(is_trade_packet)
        .collect::<Vec<_>>();
    assert!(
        matches!(observed.as_slice(), [
        ServerPacket::TradeAccept { name },
        ServerPacket::TradeItem { .. },
        ServerPacket::TradeCancel { unlock: false },
    ] if name == &recipient_name),
        "unexpected inviter trade packets: {observed:?}"
    );
    assert!(inviter.world_snapshot().stage5_systems.trade.is_none());
    assert!(!inviter
        .handle_packet(ClientPacket::KeepAlive { time: 2 })
        .iter()
        .any(is_trade_packet));
    let after = recipient.world_snapshot();
    assert!(after.stage5_systems.trade.is_none());
    assert_eq!(after.gold, before.gold);
    assert_eq!(after.inventory_items, before.inventory_items);
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
    // All three authenticated accounts belong to the same authoritative store.
    let config = GatewayConfig::default();
    let mut first = GatewaySession::new_with_zone_registry(config.clone(), &registry);
    let mut second = GatewaySession::new_with_zone_registry(config.clone(), &registry);
    let mut third = GatewaySession::new_with_zone_registry(config, &registry);
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
fn prepared_unlock_keeps_the_offer_editable_and_requires_fresh_confirmation() {
    let (mut first, mut second) = started_shared_zone_sessions();
    let gold = first.world_snapshot().gold;
    let partner_gold = second.world_snapshot().gold;
    let potion_slot = inventory_slot_for_key(&first, "red-potion");
    open_shared_trade_pair(&mut first, &mut second);
    first.handle_packet(ClientPacket::TradeGold { amount: 7 });
    assert!(first
        .handle_packet(ClientPacket::DepositTradeItem { from: potion_slot, to: 0 })
        .iter()
        .any(|p| matches!(p, ServerPacket::DepositTradeItem { success: true, .. })));
    first.handle_packet(ClientPacket::TradeConfirm { locked: true });
    let prepared = first.world_snapshot().stage5_systems.trade.unwrap();
    assert!(prepared.escrow_prepared && prepared.locked);
    assert_eq!(first.world_snapshot().gold, gold - 7);
    assert!(!has_inventory_key(&first, "red-potion"));

    // Crystal's explicit unlock is local to its sender.
    assert!(second
        .handle_packet(ClientPacket::TradeConfirm { locked: false })
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeCancel { unlock: true })));
    first.handle_packet(ClientPacket::KeepAlive { time: 1 });
    assert_eq!(first.world_snapshot().stage5_systems.trade.as_ref(), Some(&prepared));

    let packets = first.handle_packet(ClientPacket::TradeConfirm { locked: false });
    assert_eq!(packets.iter().filter(|p| matches!(p, ServerPacket::TradeCancel { unlock: true })).count(), 1);
    assert!(!packets.iter().any(|p| matches!(p,
        ServerPacket::TradeCancel { unlock: false } | ServerPacket::GainedGold { .. } | ServerPacket::GainedItem { .. })));
    let reopened = first.world_snapshot().stage5_systems.trade.unwrap();
    assert!(!reopened.escrow_prepared && !reopened.locked && !reopened.accepted);
    assert_eq!(reopened.settlement_nonce, prepared.settlement_nonce);
    assert_eq!(reopened.offered_unique_ids, prepared.offered_unique_ids);
    assert_eq!(reopened.offered_slots, prepared.offered_slots);
    assert_eq!(reopened.held_gold, Some(7));
    assert_eq!(first.world_snapshot().gold, gold - 7);
    assert_eq!(inventory_slot_for_key(&first, "red-potion"), potion_slot);

    first.handle_packet(ClientPacket::TradeConfirm { locked: false });
    assert_eq!(first.world_snapshot().stage5_systems.trade.as_ref(), Some(&reopened));
    assert!(first
        .handle_packet(ClientPacket::RetrieveTradeItem { from: 0, to: potion_slot })
        .iter()
        .any(|p| matches!(p, ServerPacket::RetrieveTradeItem { success: true, .. })));
    first.handle_packet(ClientPacket::DepositTradeItem { from: potion_slot, to: 1 });
    first.handle_packet(ClientPacket::TradeGold { amount: 5 });
    let peer_prepares = second.handle_packet(ClientPacket::TradeConfirm { locked: true });
    assert!(!peer_prepares.iter().any(|p| matches!(p,
        ServerPacket::TradeConfirm | ServerPacket::GainedItem { .. } | ServerPacket::GainedGold { .. })));
    assert!(second.world_snapshot().stage5_systems.trade.unwrap().escrow_prepared);
    assert_eq!(first.world_snapshot().gold, gold - 12);
    assert_eq!(second.world_snapshot().gold, partner_gold);

    first.handle_packet(ClientPacket::TradeConfirm { locked: true });
    let peer_delivers = second.handle_packet(ClientPacket::KeepAlive { time: 2 });
    assert_eq!(peer_delivers.iter().filter(|p| matches!(p, ServerPacket::GainedItem { item } if item.count == 5)).count(), 1);
    assert_eq!(peer_delivers.iter().filter(|p| matches!(p, ServerPacket::TradeConfirm)).count(), 1);
    assert_eq!(first.world_snapshot().gold, gold - 12);
    assert_eq!(second.world_snapshot().gold, partner_gold + 12);
    assert!(first.world_snapshot().stage5_systems.trade.is_none());
    assert!(second.world_snapshot().stage5_systems.trade.is_none());
}

#[test]
fn partner_quote_edits_invalidate_prepared_trade_before_the_peer_drains() {
    let (mut editor, mut peer) = started_shared_zone_sessions();
    let initial_editor_gold = editor.world_snapshot().gold;
    let initial_peer_gold = peer.world_snapshot().gold;
    let potion_slot = inventory_slot_for_key(&editor, "red-potion");
    open_shared_trade_pair(&mut editor, &mut peer);
    editor.handle_packet(ClientPacket::TradeGold { amount: 7 });
    editor.handle_packet(ClientPacket::DepositTradeItem {
        from: potion_slot,
        to: 0,
    });
    peer.handle_packet(ClientPacket::TradeConfirm { locked: true });
    let prepared_peer = peer.world_snapshot().stage5_systems.trade.unwrap();
    assert!(prepared_peer.escrow_prepared && prepared_peer.locked);

    let invalid = editor.handle_packet(ClientPacket::RetrieveTradeItem {
        from: 9,
        to: potion_slot,
    });
    assert!(invalid.iter().any(|packet| matches!(
        packet,
        ServerPacket::RetrieveTradeItem { success: false, .. }
    )));
    assert!(!peer
        .handle_packet(ClientPacket::KeepAlive { time: 1 })
        .iter()
        .any(|packet| { matches!(packet, ServerPacket::TradeCancel { .. }) }));
    assert_eq!(
        peer.world_snapshot().stage5_systems.trade.as_ref(),
        Some(&prepared_peer)
    );

    let retrieved = editor.handle_packet(ClientPacket::RetrieveTradeItem {
        from: 0,
        to: potion_slot,
    });
    assert!(retrieved.iter().any(|packet| matches!(
        packet,
        ServerPacket::RetrieveTradeItem { success: true, .. }
    )));
    assert!(peer.has_pending_shared_personal_work());
    let deposited = editor.handle_packet(ClientPacket::DepositTradeItem {
        from: potion_slot,
        to: 1,
    });
    assert!(deposited
        .iter()
        .any(|packet| matches!(packet, ServerPacket::DepositTradeItem { success: true, .. })));
    let confirm = editor.handle_packet(ClientPacket::TradeConfirm { locked: true });
    assert!(!confirm.iter().any(|packet| matches!(
        packet,
        ServerPacket::TradeConfirm
            | ServerPacket::GainedGold { .. }
            | ServerPacket::GainedItem { .. }
    )));
    let held_editor = editor.world_snapshot().stage5_systems.trade.unwrap();
    assert!(held_editor.escrow_prepared && held_editor.locked);
    // The editor's fresh confirmation arrived before the peer could drain.
    assert_eq!(
        peer.world_snapshot().stage5_systems.trade.as_ref(),
        Some(&prepared_peer)
    );
    let peer_update = peer.handle_packet(ClientPacket::KeepAlive { time: 2 });
    assert!(peer_update
        .iter()
        .any(|packet| matches!(packet, ServerPacket::TradeCancel { unlock: true })));
    let mut item_quotes = peer_update.iter().filter_map(|packet| match packet {
        ServerPacket::TradeItem { trade_items } => Some(trade_items),
        _ => None,
    });
    let items = item_quotes.next().expect("latest item quote");
    assert!(item_quotes.next().is_none());
    assert!(items[0].is_none());
    assert_eq!(items[1].as_ref().unwrap().count, 5);
    let reopened_peer = peer.world_snapshot().stage5_systems.trade.unwrap();
    assert_eq!(
        reopened_peer.settlement_nonce,
        prepared_peer.settlement_nonce
    );
    assert!(!reopened_peer.escrow_prepared && !reopened_peer.locked);

    // Even invalid gold attempts clear the other confirmation without a debit.
    let invalid_gold = peer.handle_packet(ClientPacket::TradeGold { amount: 0 });
    assert!(!invalid_gold.iter().any(|packet| matches!(
        packet,
        ServerPacket::LoseGold { .. } | ServerPacket::TradeGold { .. }
    )));
    let editor_update = editor.handle_packet(ClientPacket::KeepAlive { time: 3 });
    assert!(editor_update
        .iter()
        .any(|packet| matches!(packet, ServerPacket::TradeCancel { unlock: true })));
    let reopened_editor = editor.world_snapshot().stage5_systems.trade.unwrap();
    assert!(!reopened_editor.escrow_prepared && !reopened_editor.locked);
    assert_eq!(
        reopened_editor.offered_unique_ids,
        held_editor.offered_unique_ids
    );
    assert_eq!(reopened_editor.held_gold, Some(7));

    peer.handle_packet(ClientPacket::TradeConfirm { locked: true });
    assert!(editor
        .handle_packet(ClientPacket::TradeGold { amount: 5 })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoseGold { gold: 5 })));
    assert!(!editor
        .handle_packet(ClientPacket::TradeConfirm { locked: true })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::TradeConfirm)));
    let peer_update = peer.handle_packet(ClientPacket::KeepAlive { time: 4 });
    assert!(peer_update
        .iter()
        .any(|packet| matches!(packet, ServerPacket::TradeGold { amount: 12 })));
    let settled = peer.handle_packet(ClientPacket::TradeConfirm { locked: true });
    assert_eq!(
        settled
            .iter()
            .filter(|packet| matches!(packet, ServerPacket::TradeConfirm))
            .count(),
        1
    );
    assert_eq!(
        settled
            .iter()
            .filter(|packet| matches!(packet, ServerPacket::GainedItem { item } if item.count == 5))
            .count(),
        1
    );
    assert_eq!(
        settled
            .iter()
            .filter(|packet| matches!(packet, ServerPacket::GainedGold { gold: 12 }))
            .count(),
        1
    );
    editor.handle_packet(ClientPacket::KeepAlive { time: 5 });
    assert_eq!(editor.world_snapshot().gold, initial_editor_gold - 12);
    assert_eq!(peer.world_snapshot().gold, initial_peer_gold + 12);
    assert!(editor.world_snapshot().stage5_systems.trade.is_none());
    assert!(peer.world_snapshot().stage5_systems.trade.is_none());
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
