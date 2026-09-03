//! Crystal's S.TradeConfirm is a completed exchange, never a unilateral lock
//! or an internal escrow-preparation receipt (PlayerObject.cs / GameScene.cs).
use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::{SharedTradeOffer, SimulationConfig, SimulationSession};
use serde_json::{json, Value};

fn started(config: &SimulationConfig) -> SimulationSession {
    let mut session = SimulationSession::new(config.clone());
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: "demo".to_owned(),
            password: "demo".to_owned(),
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(session.active_identity().is_some());
    session
}

fn offer(session: &mut SimulationSession, gold: u32, with_item: bool) {
    session.trade_request("Trader");
    session.handle_packet(ClientPacket::TradeReply {
        accept_invite: true,
    });
    if gold > 0 {
        session.handle_packet(ClientPacket::TradeGold { amount: gold });
    }
    if with_item {
        let slot = session
            .world_snapshot()
            .inventory_items
            .iter()
            .find(|item| item.key == "red-potion")
            .expect("original demo item fixture")
            .slot;
        assert!(session
            .handle_packet(ClientPacket::DepositTradeItem {
                from: i32::from(slot),
                to: 3
            })
            .iter()
            .any(|packet| matches!(packet, ServerPacket::DepositTradeItem { success: true, .. })));
    }
}

fn incoming(own: &SharedTradeOffer, gold: u32) -> SharedTradeOffer {
    let nonce = if own.settlement_nonce == "00000000000000000000000000000002" {
        "00000000000000000000000000000003"
    } else {
        "00000000000000000000000000000002"
    };
    SharedTradeOffer {
        settlement_nonce: nonce.to_owned(),
        account_id: "trader-fixture".to_owned(),
        character_index: 1,
        character_name: "Trader".to_owned(),
        partner_name: own.character_name.clone(),
        gold,
        items: Vec::new(),
    }
}

fn completions(packets: &[ServerPacket]) -> usize {
    packets
        .iter()
        .filter(|packet| matches!(packet, ServerPacket::TradeConfirm))
        .count()
}

#[test]
fn personal_confirmation_is_only_a_lock_and_can_be_unlocked() {
    let mut session = started(&SimulationConfig::default());
    offer(&mut session, 25, true);
    let before = session.world_snapshot();
    assert!(session
        .handle_packet(ClientPacket::TradeConfirm { locked: true })
        .is_empty());
    let locked = session.world_snapshot();
    assert_eq!(locked.gold, before.gold);
    assert_eq!(locked.inventory_items, before.inventory_items);
    let trade = locked.stage5_systems.trade.unwrap();
    assert!(trade.locked && trade.accepted);
    assert!(!trade.completed && !trade.escrow_prepared);
    assert_eq!(
        session.handle_packet(ClientPacket::TradeConfirm { locked: false }),
        vec![ServerPacket::TradeCancel { unlock: true }]
    );
    let unlocked = session.world_snapshot();
    assert_eq!(unlocked.gold, before.gold);
    assert_eq!(unlocked.inventory_items, before.inventory_items);
    assert!(!unlocked.stage5_systems.trade.unwrap().locked);
}

#[test]
fn shared_preparation_is_not_completion_and_cannot_be_debited_twice() {
    let mut session = started(&SimulationConfig::default());
    offer(&mut session, 25, true);
    let before = session.world_snapshot();
    let (packets, own) = session.shared_trade_confirm();
    let own = own.expect("typed preparation returns the identified offer");
    assert_eq!(completions(&packets), 0);
    assert_eq!(packets, vec![ServerPacket::LoseGold { gold: 25 }]);
    let held = session.world_snapshot();
    let trade = held.stage5_systems.trade.as_ref().unwrap();
    assert!(trade.escrow_prepared && trade.locked && trade.accepted);
    assert!(!trade.completed);
    assert_eq!(held.gold, before.gold - 25);
    assert_eq!(held.inventory_items.len() + 1, before.inventory_items.len());
    assert_eq!(session.shared_trade_confirm(), (Vec::new(), None));
    for packet in [
        ClientPacket::TradeReply {
            accept_invite: false,
        },
        ClientPacket::TradeReply {
            accept_invite: true,
        },
        ClientPacket::TradeConfirm { locked: false },
        ClientPacket::TradeCancel,
    ] {
        assert!(session.handle_packet(packet).is_empty());
        let after = session.world_snapshot();
        assert_eq!(after.gold, held.gold);
        assert_eq!(after.inventory_items, held.inventory_items);
        assert_eq!(after.stage5_systems.trade, held.stage5_systems.trade);
    }
    let refund = session.rollback_shared_trade_offer(&own);
    assert_eq!(completions(&refund), 0);
    assert!(refund
        .iter()
        .any(|p| matches!(p, ServerPacket::TradeCancel { unlock: false })));
    assert_eq!(session.world_snapshot().gold, before.gold);
    assert_eq!(
        session.world_snapshot().inventory_items.len(),
        before.inventory_items.len()
    );
    assert!(session.rollback_shared_trade_offer(&own).is_empty());
    assert_eq!(session.world_snapshot().gold, before.gold);
}

#[test]
fn empty_preparation_still_returns_an_offer_and_finishes_only_on_delivery() {
    let mut session = started(&SimulationConfig::default());
    offer(&mut session, 0, false);
    let before = session.world_snapshot();
    let (packets, own) = session.shared_trade_confirm();
    assert!(packets.is_empty());
    let own = own.expect("an empty offer has a typed success independent of wire packets");
    let peer = incoming(&own, 0);
    assert_eq!(
        session.apply_shared_trade_delivery(&peer),
        vec![ServerPacket::TradeConfirm]
    );
    assert!(session.world_snapshot().stage5_systems.trade.is_none());
    assert_eq!(session.world_snapshot().gold, before.gold);
    assert_eq!(
        session.world_snapshot().inventory_items,
        before.inventory_items
    );
    assert!(session.apply_shared_trade_delivery(&peer).is_empty());
}

#[test]
fn invalid_offer_identity_or_slots_cannot_prepare_or_charge_gold() {
    for mutation in ["nonce", "unique-id", "slot", "balance"] {
        let mut session = started(&SimulationConfig::default());
        offer(&mut session, 25, true);
        let mut checkpoint = session.active_character_checkpoint().unwrap();
        let mut systems: Value =
            serde_json::from_str(checkpoint.stage5_systems_json.as_ref().unwrap()).unwrap();
        let trade = &mut systems["trade"];
        match mutation {
            "nonce" => trade["settlementNonce"] = json!("invalid"),
            "unique-id" => trade["offeredUniqueIds"]["3"] = json!(u64::MAX),
            "slot" => {
                let slot = trade["offeredSlots"]["3"].take();
                let uid = trade["offeredUniqueIds"]["3"].take();
                trade["offeredSlots"] = json!({"10": slot});
                trade["offeredUniqueIds"] = json!({"10": uid});
            }
            "balance" => trade["offeredGold"] = json!(session.world_snapshot().gold + 1),
            _ => unreachable!(),
        }
        checkpoint.stage5_systems_json = Some(systems.to_string());
        session
            .restore_active_character_checkpoint(&checkpoint)
            .unwrap();
        let before = session.world_snapshot();
        let (packets, own) = session.shared_trade_confirm();
        assert!(own.is_none(), "{mutation}");
        assert_eq!(completions(&packets), 0, "{mutation}");
        if mutation == "unique-id" {
            assert!(packets
                .iter()
                .any(|p| matches!(p, ServerPacket::TradeCancel { unlock: true })));
        }
        assert!(
            !packets
                .iter()
                .any(|p| matches!(p, ServerPacket::LoseGold { .. })),
            "{mutation}"
        );
        let after = session.world_snapshot();
        assert_eq!(after.gold, before.gold, "{mutation}");
        assert_eq!(after.inventory_items, before.inventory_items, "{mutation}");
        assert!(
            !after.stage5_systems.trade.unwrap().escrow_prepared,
            "{mutation}"
        );
    }
}

#[test]
fn durable_completion_is_after_saved_delivery_once_for_live_legacy_and_pretrade_states() {
    for state in ["live", "legacy", "pretrade"] {
        let config = SimulationConfig::default();
        let mut session = started(&config);
        let before = session.active_character_checkpoint().unwrap();
        let original_gold = session.world_snapshot().gold;
        offer(&mut session, 25, true);
        let own = session.shared_trade_confirm().1.unwrap();
        if state == "pretrade" {
            session
                .restore_active_character_checkpoint(&before)
                .unwrap();
        } else if state == "legacy" {
            let mut held = session.active_character_checkpoint().unwrap();
            let mut systems: Value =
                serde_json::from_str(held.stage5_systems_json.as_ref().unwrap()).unwrap();
            systems["trade"]
                .as_object_mut()
                .unwrap()
                .remove("escrowPrepared");
            systems["trade"]["completed"] = json!(true);
            held.stage5_systems_json = Some(systems.to_string());
            session.restore_active_character_checkpoint(&held).unwrap();
        }
        let peer = incoming(&own, 10);
        let event = "b".repeat(64);
        let packets = session
            .apply_shared_trade_settlement_projection(&event, &own, &peer)
            .unwrap();
        assert_eq!(completions(&packets), 1, "{state}");
        assert!(
            matches!(packets.last(), Some(ServerPacket::TradeConfirm)),
            "{state}"
        );
        assert_eq!(
            packets
                .iter()
                .any(|p| matches!(p, ServerPacket::LoseGold { gold: 25 })),
            state == "pretrade"
        );
        assert_eq!(session.world_snapshot().gold, original_gold - 15, "{state}");
        assert!(session.world_snapshot().stage5_systems.trade.is_none());
        assert!(session.has_shared_economy_projection_event(&event));
        assert!(session
            .apply_shared_trade_settlement_projection(&event, &own, &peer)
            .unwrap()
            .is_empty());
        let mut restored = started(&config);
        assert!(restored.has_shared_economy_projection_event(&event));
        assert_eq!(
            restored.world_snapshot().gold,
            original_gold - 15,
            "{state}"
        );
        assert!(restored
            .apply_shared_trade_settlement_projection(&event, &own, &peer)
            .unwrap()
            .is_empty());
    }
}

#[test]
fn delivery_and_rollback_are_owned_by_the_current_prepared_exchange() {
    let mut session = started(&SimulationConfig::default());
    offer(&mut session, 25, false);
    let own = session.shared_trade_confirm().1.unwrap();
    let held = session.world_snapshot();
    let mut wrong_refund = own.clone();
    wrong_refund.settlement_nonce = "f".repeat(32);
    assert!(session
        .rollback_shared_trade_offer(&wrong_refund)
        .is_empty());
    wrong_refund = own.clone();
    wrong_refund.gold += 1;
    assert!(session
        .rollback_shared_trade_offer(&wrong_refund)
        .is_empty());
    let mut wrong_delivery = incoming(&own, 10);
    wrong_delivery.partner_name = "SomeoneElse".to_owned();
    assert!(session
        .apply_shared_trade_delivery(&wrong_delivery)
        .is_empty());
    assert_eq!(session.world_snapshot().gold, held.gold);
    assert_eq!(
        session.world_snapshot().stage5_systems.trade,
        held.stage5_systems.trade
    );
    let peer = incoming(&own, 10);
    let packets = session.apply_shared_trade_delivery(&peer);
    assert_eq!(completions(&packets), 1);
    assert!(matches!(packets.last(), Some(ServerPacket::TradeConfirm)));
    assert_eq!(session.world_snapshot().gold, held.gold + 10);
    assert!(session.apply_shared_trade_delivery(&peer).is_empty());
    assert!(session.rollback_shared_trade_offer(&own).is_empty());
    assert_eq!(session.world_snapshot().gold, held.gold + 10);
}

#[cfg(feature = "test-support")]
#[test]
fn failed_save_retains_prepared_escrow_and_releases_completion_only_after_retry() {
    use mir2_simulation::AccountStoreTransactionFault;
    let config = SimulationConfig::default();
    let mut session = started(&config);
    offer(&mut session, 25, true);
    let own = session.shared_trade_confirm().1.unwrap();
    let held = session.world_snapshot();
    let peer = incoming(&own, 10);
    let event = "c".repeat(64);
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::Persist);
    assert!(session
        .apply_shared_trade_settlement_projection(&event, &own, &peer)
        .is_err());
    let failed = session.world_snapshot();
    assert_eq!(failed.gold, held.gold);
    assert_eq!(failed.inventory_items, held.inventory_items);
    assert_eq!(failed.stage5_systems.trade, held.stage5_systems.trade);
    assert!(!session.has_shared_economy_projection_event(&event));
    let packets = session
        .apply_shared_trade_settlement_projection(&event, &own, &peer)
        .unwrap();
    assert_eq!(completions(&packets), 1);
    assert!(matches!(packets.last(), Some(ServerPacket::TradeConfirm)));
    assert!(!packets
        .iter()
        .any(|p| matches!(p, ServerPacket::LoseGold { .. })));
    assert_eq!(session.world_snapshot().gold, held.gold + 10);
    assert!(session
        .apply_shared_trade_settlement_projection(&event, &own, &peer)
        .unwrap()
        .is_empty());
}
