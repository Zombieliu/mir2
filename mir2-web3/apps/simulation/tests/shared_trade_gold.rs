//! Immediate editable gold custody is independent from prepared item custody.
use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::{SharedTradeOffer, SimulationConfig, SimulationSession};
use serde_json::{json, Value};

fn started(config: &SimulationConfig) -> SimulationSession {
    let mut session = SimulationSession::new(config.clone());
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    session
}
fn open(session: &mut SimulationSession) {
    session.trade_request("Trader");
}
fn add(session: &mut SimulationSession, amount: u32) -> Vec<ServerPacket> {
    session.handle_packet(ClientPacket::TradeGold { amount })
}
fn incoming(own: &SharedTradeOffer) -> SharedTradeOffer {
    SharedTradeOffer {
        settlement_nonce: "abcdefabcdefabcdefabcdefabcdefab".into(),
        account_id: "other".into(),
        character_index: 1,
        character_name: "Trader".into(),
        partner_name: own.character_name.clone(),
        gold: 7,
        items: Vec::new(),
    }
}

#[test]
fn positive_deltas_immediately_debit_and_publish_cumulative_offer() {
    let mut s = started(&SimulationConfig::default());
    let before = s.world_snapshot().gold;
    open(&mut s);
    assert_eq!(
        add(&mut s, 10),
        vec![
            ServerPacket::LoseGold { gold: 10 },
            ServerPacket::TradeGold { amount: 10 }
        ]
    );
    assert_eq!(
        add(&mut s, 15),
        vec![
            ServerPacket::LoseGold { gold: 15 },
            ServerPacket::TradeGold { amount: 25 }
        ]
    );
    assert_eq!(s.world_snapshot().gold, before - 25);
    assert_eq!(s.shared_trade_unprepared_held_gold(), Some(25));
    let (packets, offer) = s.shared_trade_confirm();
    assert!(packets.is_empty());
    assert_eq!(offer.unwrap().gold, 25);
    assert_eq!(s.world_snapshot().gold, before - 25);
}

#[test]
fn invalid_edit_unlocks_without_changing_wallet_or_custody() {
    let mut s = started(&SimulationConfig::default());
    open(&mut s);
    add(&mut s, 10);
    for amount in [0, u32::MAX] {
        s.handle_packet(ClientPacket::TradeConfirm { locked: true });
        let before = s.world_snapshot();
        assert!(before.stage5_systems.trade.unwrap().locked);
        assert!(add(&mut s, amount).is_empty());
        let after = s.world_snapshot();
        assert_eq!(after.gold, before.gold);
        let trade = after.stage5_systems.trade.unwrap();
        assert!(!trade.locked && !trade.accepted);
        assert_eq!(trade.held_gold, Some(10));
        assert_eq!(trade.offered_gold, 10);
    }
}

#[test]
fn cancel_decline_and_highlevel_cancel_refund_once() {
    for action in 0..3 {
        let mut s = started(&SimulationConfig::default());
        let before = s.world_snapshot().gold;
        open(&mut s);
        add(&mut s, 25);
        let packets = match action {
            0 => s.handle_packet(ClientPacket::TradeCancel),
            1 => s.handle_packet(ClientPacket::TradeReply {
                accept_invite: false,
            }),
            _ => s.stage5_command("trade.cancel", Vec::new()),
        };
        assert_eq!(
            packets
                .iter()
                .filter(|p| matches!(p, ServerPacket::GainedGold { gold: 25 }))
                .count(),
            1
        );
        assert_eq!(s.world_snapshot().gold, before);
        assert!(s.world_snapshot().stage5_systems.trade.is_none());
        assert!(s.handle_packet(ClientPacket::TradeCancel).is_empty());
    }
}

#[test]
fn held_funds_cannot_be_overwritten_or_charged_by_highlevel_adapters() {
    let mut s = started(&SimulationConfig::default());
    open(&mut s);
    add(&mut s, 25);
    let before = s.world_snapshot();
    for (action, args) in [
        ("trade.start", vec!["Replacement".into()]),
        ("trade.offerGold", vec!["1".into(), "bichon".into()]),
        ("trade.accept", Vec::new()),
    ] {
        s.stage5_command(action, args);
        assert_eq!(s.world_snapshot().gold, before.gold);
        assert_eq!(
            s.world_snapshot().stage5_systems.trade,
            before.stage5_systems.trade
        );
    }
}

#[test]
fn prepared_edits_and_orphan_refunds_cannot_release_coordinator_custody() {
    let mut s = started(&SimulationConfig::default());
    open(&mut s);
    add(&mut s, 25);
    let own = s.shared_trade_confirm().1.unwrap();
    let before = s.world_snapshot();
    assert!(add(&mut s, 1).is_empty());
    assert!(s.recover_unprepared_trade_gold().is_empty());
    assert!(s.handle_packet(ClientPacket::TradeCancel).is_empty());
    assert!(s.shared_trade_confirm().1.is_none());
    assert_eq!(s.world_snapshot().gold, before.gold);
    assert_eq!(
        s.world_snapshot().stage5_systems.trade,
        before.stage5_systems.trade
    );
    assert!(s
        .rollback_shared_trade_offer(&own)
        .iter()
        .any(|p| matches!(p, ServerPacket::GainedGold { gold: 25 })));
    assert!(s.rollback_shared_trade_offer(&own).is_empty());
}

#[test]
fn legacy_unprepared_offer_only_charges_outstanding_gold_at_preparation() {
    let mut s = started(&SimulationConfig::default());
    open(&mut s);
    let mut checkpoint = s.active_character_checkpoint().unwrap();
    let original = checkpoint.gold;
    let mut state: Value =
        serde_json::from_str(checkpoint.stage5_systems_json.as_ref().unwrap()).unwrap();
    state["trade"].as_object_mut().unwrap().remove("heldGold");
    state["trade"]["offeredGold"] = json!(20);
    checkpoint.stage5_systems_json = Some(state.to_string());
    s.restore_active_character_checkpoint(&checkpoint).unwrap();
    add(&mut s, 5);
    assert_eq!(s.shared_trade_unprepared_held_gold(), Some(5));
    let (packets, own) = s.shared_trade_confirm();
    assert_eq!(own.unwrap().gold, 25);
    assert_eq!(packets, vec![ServerPacket::LoseGold { gold: 20 }]);
    assert_eq!(s.world_snapshot().gold, original - 25);
}

#[test]
fn saved_full_wallet_hold_is_not_reseeded_or_automatically_refunded() {
    let config = SimulationConfig::default();
    let mut s = started(&config);
    let gold = s.world_snapshot().gold;
    open(&mut s);
    add(&mut s, gold);
    s.save_active_character().unwrap();
    let mut restored = started(&config);
    assert_eq!(restored.world_snapshot().gold, 0);
    assert_eq!(restored.shared_trade_unprepared_held_gold(), Some(gold));
    assert!(restored
        .recover_unprepared_trade_gold()
        .iter()
        .any(|p| matches!(p, ServerPacket::GainedGold { gold: amount } if *amount == gold)));
    assert_eq!(restored.world_snapshot().gold, gold);
    assert!(restored.recover_unprepared_trade_gold().is_empty());
    assert_eq!(started(&config).world_snapshot().gold, gold);
}

#[test]
fn cancellation_and_prepared_refund_fail_closed_over_wallet_cap() {
    for prepared in [false, true] {
        let mut s = started(&SimulationConfig::default());
        open(&mut s);
        add(&mut s, 25);
        let own = prepared.then(|| s.shared_trade_confirm().1.unwrap());
        let mut checkpoint = s.active_character_checkpoint().unwrap();
        checkpoint.gold = u32::MAX;
        s.restore_active_character_checkpoint(&checkpoint).unwrap();
        let before = s.world_snapshot().stage5_systems.trade;
        let packets = match own {
            Some(offer) => s.rollback_shared_trade_offer(&offer),
            None => s.handle_packet(ClientPacket::TradeCancel),
        };
        assert!(packets.is_empty());
        assert_eq!(s.world_snapshot().gold, u32::MAX);
        assert_eq!(s.world_snapshot().stage5_systems.trade, before);
    }
}

#[test]
fn older_editable_checkpoint_projects_only_missing_gold_and_retains_item_debit_independence() {
    let config = SimulationConfig::default();
    let mut s = started(&config);
    let original = s.world_snapshot().gold;
    open(&mut s);
    add(&mut s, 10);
    let slot = s
        .world_snapshot()
        .inventory_items
        .iter()
        .find(|item| item.key == "red-potion")
        .unwrap()
        .slot;
    s.handle_packet(ClientPacket::DepositTradeItem {
        from: i32::from(slot),
        to: 0,
    });
    let earlier = s.active_character_checkpoint().unwrap();
    add(&mut s, 15);
    let own = s.shared_trade_confirm().1.unwrap();
    s.restore_active_character_checkpoint(&earlier).unwrap();
    let peer = incoming(&own);
    let event = "d".repeat(64);
    let packets = s
        .apply_shared_trade_settlement_projection(&event, &own, &peer)
        .unwrap();
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::LoseGold { gold: 15 })));
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::DeleteItem { .. })));
    assert_eq!(s.world_snapshot().gold, original - 25 + 7);
    assert!(s
        .apply_shared_trade_settlement_projection(&event, &own, &peer)
        .unwrap()
        .is_empty());
}

#[test]
fn malformed_explicit_custody_is_rejected_on_restore() {
    let mut s = started(&SimulationConfig::default());
    open(&mut s);
    add(&mut s, 25);
    let mut checkpoint = s.active_character_checkpoint().unwrap();
    let mut state: Value =
        serde_json::from_str(checkpoint.stage5_systems_json.as_ref().unwrap()).unwrap();
    state["trade"]["heldGold"] = json!(26);
    checkpoint.stage5_systems_json = Some(state.to_string());
    assert!(s.restore_active_character_checkpoint(&checkpoint).is_err());
    assert_eq!(s.shared_trade_unprepared_held_gold(), Some(25));
}

#[cfg(feature = "test-support")]
#[test]
fn orphan_refund_save_failure_restores_custody_then_retries_once() {
    use mir2_simulation::AccountStoreTransactionFault;
    let config = SimulationConfig::default();
    let mut s = started(&config);
    let original = s.world_snapshot().gold;
    open(&mut s);
    add(&mut s, 25);
    s.save_active_character().unwrap();
    let before = s.world_snapshot();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::Persist);
    assert!(s.recover_unprepared_trade_gold().is_empty());
    assert_eq!(s.world_snapshot().gold, before.gold);
    assert_eq!(
        s.world_snapshot().stage5_systems.trade,
        before.stage5_systems.trade
    );
    assert!(s
        .recover_unprepared_trade_gold()
        .iter()
        .any(|p| matches!(p, ServerPacket::GainedGold { gold: 25 })));
    assert_eq!(s.world_snapshot().gold, original);
    assert!(s.recover_unprepared_trade_gold().is_empty());
    assert_eq!(started(&config).world_snapshot().gold, original);
}

#[test]
fn mixed_prepared_rollback_full_bag_retains_custody_until_retry() {
    let mut s = started(&SimulationConfig::default());
    let original = s.world_snapshot().gold;
    open(&mut s);
    add(&mut s, 25);
    let slot = s
        .world_snapshot()
        .inventory_items
        .iter()
        .find(|item| item.key == "red-potion")
        .unwrap()
        .slot;
    s.handle_packet(ClientPacket::DepositTradeItem {
        from: i32::from(slot),
        to: 0,
    });
    let own = s.shared_trade_confirm().1.unwrap();
    assert_eq!(own.items.len(), 1);
    for _ in 0..100 {
        s.stage5_command("qa.giveItem", vec!["wooden-sword".into(), "1".into()]);
    }
    let before = s.world_snapshot();
    let failed = s.rollback_shared_trade_offer(&own);
    assert!(!failed.is_empty());
    assert!(!failed.iter().any(|p| matches!(
        p,
        ServerPacket::TradeCancel { .. }
            | ServerPacket::TradeConfirm
            | ServerPacket::GainedItem { .. }
            | ServerPacket::GainedGold { .. }
    )));
    assert_eq!(s.world_snapshot().gold, before.gold);
    assert_eq!(
        s.world_snapshot().stage5_systems.trade,
        before.stage5_systems.trade
    );
    assert!(s.shared_trade_offer_matches_active_escrow(&own, true));
    let mut checkpoint = s.active_character_checkpoint().unwrap();
    checkpoint.inventory_items_json.pop().unwrap();
    s.restore_active_character_checkpoint(&checkpoint).unwrap();
    let returned = s.rollback_shared_trade_offer(&own);
    assert!(returned
        .iter()
        .any(|p| matches!(p, ServerPacket::GainedGold { gold: 25 })));
    assert!(returned
        .iter()
        .any(|p| matches!(p, ServerPacket::GainedItem { .. })));
    assert_eq!(s.world_snapshot().gold, original);
    assert!(!s.shared_trade_offer_matches_active_escrow(&own, true));
    assert!(s.rollback_shared_trade_offer(&own).is_empty());
}

#[test]
fn cumulative_offer_overflow_with_available_wallet_unlocks_without_debit() {
    let mut s = started(&SimulationConfig::default());
    open(&mut s);
    let mut checkpoint = s.active_character_checkpoint().unwrap();
    checkpoint.gold = 1;
    let mut state: Value =
        serde_json::from_str(checkpoint.stage5_systems_json.as_ref().unwrap()).unwrap();
    state["trade"]["offeredGold"] = json!(u32::MAX);
    state["trade"]["heldGold"] = json!(u32::MAX);
    state["trade"]["locked"] = json!(true);
    state["trade"]["accepted"] = json!(true);
    checkpoint.stage5_systems_json = Some(state.to_string());
    s.restore_active_character_checkpoint(&checkpoint).unwrap();
    assert!(add(&mut s, 1).is_empty());
    assert_eq!(s.world_snapshot().gold, 1);
    let trade = s.world_snapshot().stage5_systems.trade.unwrap();
    assert_eq!(trade.offered_gold, u32::MAX);
    assert_eq!(trade.held_gold, Some(u32::MAX));
    assert!(!trade.locked && !trade.accepted);
}
