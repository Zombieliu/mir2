//! Source TradeDialogs/GameScene packet semantics, without a GUI or server.
use super::*;
use serde_json::json;

fn open() -> SocialModel {
    let mut model = SocialModel::default();
    assert!(model.apply_packet("TradeAccept", &json!({"name":"Guest"})));
    model.trade.partner_gold = 70;
    model.trade.my_gold = 30;
    model.trade.my_offer_nonce = Some("exchange-a".into());
    model.trade.my_confirmed = true;
    model.trade.partner_items = vec![
        None,
        Some(TradeItemModel {
            unique_id: Some(22),
            count: 2,
            ..Default::default()
        }),
    ];
    model.trade.my_items = vec![Some(TradeItemModel {
        unique_id: Some(11),
        count: 1,
        ..Default::default()
    })];
    model
}

fn packet(model: &mut SocialModel, name: &str, payload: Value) {
    let mut incoming = model.clone();
    assert!(incoming.apply_packet(name, &payload));
    model.apply_authoritative(incoming);
}

#[test]
fn trade_wire_sparse_slots_are_not_compacted_and_zero_count_is_not_invented() {
    let mut model = open();
    packet(
        &mut model,
        "TradeItem",
        json!({"tradeItems":[null,{"unique_id":55,"item_index":27,"count":0},null,null,{"uniqueId":56,"itemIndex":28,"count":5}]}),
    );
    assert_eq!(model.trade.partner_items.len(), 5);
    assert!(model.trade.partner_items[0].is_none());
    assert!(model.trade.partner_items[2].is_none());
    assert!(model.trade.partner_items[3].is_none());
    assert_eq!(
        model.trade.partner_items[1].as_ref().unwrap().unique_id,
        Some(55)
    );
    assert_eq!(model.trade.partner_items[1].as_ref().unwrap().count, 0);
    assert_eq!(
        model.trade.partner_items[4].as_ref().unwrap().item_index,
        Some(28)
    );
    assert!(!model.trade.my_confirmed);
    assert_eq!(model.trade.my_gold, 30);
    assert_eq!(
        model.trade.my_items[0].as_ref().unwrap().unique_id,
        Some(11)
    );
    assert_eq!(
        serde_json::to_value(&model.trade).unwrap()["partnerItems"][0],
        Value::Null
    );
}

#[test]
fn trade_malformed_packets_are_transactional_and_do_not_advance_revision() {
    let model = open();
    for (name, payload) in [
        ("TradeItem", json!({"tradeItems":[null,42]})),
        ("TradeItem", json!({"tradeItems":[{}]})),
        (
            "TradeItem",
            json!({"tradeItems":[{"uniqueId":1,"count":65536}]}),
        ),
        (
            "TradeItem",
            json!({"tradeItems":[{"uniqueId":"bad","count":1}]}),
        ),
        ("TradeItem", json!({"partnerItems":[false],"tradeItems":[]})),
        ("TradeItem", json!({"tradeItems":vec![Value::Null;11]})),
        ("TradeGold", json!({"amount":4_294_967_296u64})),
        ("TradeGold", json!({"amount":-1})),
        ("TradeCancel", json!({})),
        ("TradeCancel", json!({"unlock":1})),
        (
            "DepositTradeItem",
            json!({"from":2,"to":1,"success":"false"}),
        ),
        ("RetrieveTradeItem", json!({"from":2,"success":true})),
    ] {
        let mut current = model.clone();
        assert!(!current.apply_packet(name, &payload), "{name} {payload}");
        assert_eq!(current, model);
    }
}

#[test]
fn trade_equal_guest_offer_packets_still_release_local_lock_but_not_own_gold_request() {
    for (name, payload) in [
        ("TradeGold", json!({"amount":70})),
        (
            "TradeItem",
            json!({"tradeItems":[null,{"uniqueId":22,"count":2}]}),
        ),
    ] {
        let mut model = open();
        model.begin_pending(SocialPendingOperation::TradeGold { amount: 70 });
        let revision = model.trade.event_revision;
        packet(&mut model, name, payload.clone());
        assert_eq!(model.trade.event_revision, revision + 1);
        assert!(!model.trade.my_confirmed);
        model.trade.my_confirmed = true;
        packet(&mut model, name, payload);
        assert_eq!(model.trade.event_revision, revision + 2);
        assert!(!model.trade.my_confirmed);
        assert_eq!(
            model.pending,
            vec![SocialPendingOperation::TradeGold { amount: 70 }]
        );
        assert_eq!(model.trade.my_gold, 30);
    }
}

#[test]
fn trade_unlock_is_not_cancel_and_keeps_offers_partner_and_unrelated_pending() {
    let mut model = open();
    let before = model.clone();
    model.begin_pending(SocialPendingOperation::TradeGold { amount: 10 });
    model.begin_pending(SocialPendingOperation::TradeCancel);
    model.begin_pending(SocialPendingOperation::TradeConfirm { locked: true });
    model.begin_pending(SocialPendingOperation::GuildInfo);
    packet(&mut model, "TradeCancel", json!({"unlock":true}));
    assert_eq!(model.trade.state, "open");
    assert_eq!(model.trade.partner, before.trade.partner);
    assert_eq!(model.trade.my_items, before.trade.my_items);
    assert_eq!(model.trade.partner_items, before.trade.partner_items);
    assert_eq!((model.trade.my_gold, model.trade.partner_gold), (30, 70));
    assert!(!model.trade.my_confirmed);
    assert_eq!(
        model.pending,
        vec![
            SocialPendingOperation::TradeGold { amount: 10 },
            SocialPendingOperation::TradeCancel,
            SocialPendingOperation::GuildInfo
        ]
    );
}

#[test]
fn trade_completion_and_terminal_cancel_reset_both_sides_and_only_trade_pending() {
    for (name, payload) in [
        ("TradeConfirm", json!({})),
        ("TradeCancel", json!({"unlock":false})),
    ] {
        let mut model = open();
        let revision = model.trade.event_revision;
        for request in [
            SocialPendingOperation::TradeRequest,
            SocialPendingOperation::TradeReply,
            SocialPendingOperation::TradeGold { amount: 9 },
            SocialPendingOperation::TradeDeposit { from: 1, to: 2 },
            SocialPendingOperation::TradeRetrieve { from: 3, to: 4 },
            SocialPendingOperation::TradeConfirm { locked: true },
            SocialPendingOperation::TradeCancel,
            SocialPendingOperation::GuildInfo,
        ] {
            assert!(model.begin_pending(request));
        }
        packet(&mut model, name, payload);
        assert_eq!(
            model.trade,
            TradeModel {
                event_revision: revision + 1,
                ..Default::default()
            }
        );
        assert_eq!(model.pending, vec![SocialPendingOperation::GuildInfo]);
    }
}

#[test]
fn trade_new_owner_clears_previous_offer_but_duplicate_accept_retains_current_offer() {
    let mut model = open();
    let before = model.trade.clone();
    packet(&mut model, "TradeAccept", json!({"name":"Guest"}));
    assert_eq!(
        model.trade,
        TradeModel {
            event_revision: before.event_revision + 1,
            ..before
        }
    );
    packet(&mut model, "TradeAccept", json!({"name":"Other"}));
    assert_eq!(model.trade.my_gold, 0);
    assert!(model.trade.my_items.is_empty());
    assert!(model.trade.partner_items.is_empty());
    assert!(model.trade.my_offer_nonce.is_none());
    assert_eq!(model.trade.partner.as_deref(), Some("Other"));
}

#[test]
fn trade_matching_failure_is_a_cell_unlock_not_an_item_success() {
    for (name, request) in [
        (
            "DepositTradeItem",
            SocialPendingOperation::TradeDeposit { from: 2, to: 4 },
        ),
        (
            "RetrieveTradeItem",
            SocialPendingOperation::TradeRetrieve { from: 2, to: 4 },
        ),
    ] {
        for success in [false, true] {
            let mut model = open();
            let own = model.trade.my_items.clone();
            model.begin_pending(request.clone());
            packet(&mut model, name, json!({"from":2,"to":5,"success":success}));
            assert_eq!(model.pending, vec![request.clone()]);
            packet(&mut model, name, json!({"from":2,"to":4,"success":success}));
            assert!(model.pending.is_empty());
            assert!(!model.trade.my_confirmed);
            assert_eq!(model.last_event.as_ref().unwrap().success, Some(success));
            assert_eq!(
                model.trade.my_items, own,
                "a receipt alone does not invent exact item metadata"
            );
        }
    }
}

#[test]
fn trade_own_gold_request_requires_exact_same_exchange_snapshot_delta() {
    for (event, partner, nonce, gold, releases) in [
        ("NativeOwnTradeSnapshot", "Guest", "exchange-a", 40, true),
        ("NativeOwnTradeSnapshot", "Guest", "exchange-a", 30, false),
        ("NativeOwnTradeSnapshot", "Guest", "exchange-a", 10, false),
        ("NativeOwnTradeSnapshot", "Other", "exchange-a", 40, false),
        ("NativeOwnTradeSnapshot", "Guest", "exchange-b", 40, false),
        ("TradeGold", "Guest", "exchange-a", 40, false),
    ] {
        let mut model = open();
        model.begin_pending(SocialPendingOperation::TradeGold { amount: 10 });
        let mut incoming = model.clone();
        incoming.trade.my_gold = gold;
        incoming.trade.partner = Some(partner.into());
        incoming.trade.my_offer_nonce = Some(nonce.into());
        incoming.last_event = Some(SocialAuthoritativeEvent {
            packet: event.into(),
            success: None,
            subject: Some(partner.into()),
            from: None,
            to: None,
            change_type: None,
            rank_index: None,
            amount: Some(gold),
        });
        model.apply_authoritative(incoming);
        assert_eq!(
            model.pending.is_empty(),
            releases,
            "{event} {partner} {nonce} {gold}"
        );
    }
}

#[test]
fn trade_same_partner_new_exchange_cannot_acknowledge_an_old_own_offer() {
    let mut model = open();
    model.begin_pending(SocialPendingOperation::TradeGold { amount: 10 });
    let previous = model.trade.open_revision;
    let mut incoming = model.clone();
    assert!(incoming.apply_packet("TradeCancel", &json!({"unlock":false})));
    assert!(incoming.apply_packet("TradeAccept", &json!({"name":"Guest"})));
    assert!(incoming.trade.open_revision > previous);
    model.apply_authoritative(incoming);
    assert!(
        model.pending.is_empty(),
        "old exchange work is superseded, not a success receipt"
    );
    assert_eq!(model.trade.my_gold, 0);
    assert!(model.trade.my_offer_nonce.is_none());
}
