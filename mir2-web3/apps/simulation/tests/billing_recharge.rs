use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::billing::{
    RechargeOffer, RechargeOrder, RechargeOrderState, VerifiedRechargePayment,
};
use mir2_simulation::{
    AccountRecord, AccountStoreRepository, FileAccountStoreRepository, SimulationConfig,
    SimulationSession,
};

const NOW: u64 = 1_700_000_000_000;
fn offer(credits: u32) -> RechargeOffer {
    RechargeOffer {
        id: format!("credits-{credits}"),
        label: "Credits".into(),
        currency: "usd".into(),
        amount_minor: 500,
        credits,
    }
}
fn payment(session: &str, intent: &str) -> VerifiedRechargePayment {
    VerifiedRechargePayment {
        session_id: session.into(),
        payment_intent_id: intent.into(),
        amount_minor: 500,
        currency: "usd".into(),
        livemode: false,
    }
}
fn paid(config: &SimulationConfig, request: &str, suffix: &str, credits: u32) -> RechargeOrder {
    let index = config.default_character.index;
    let order = config
        .prepare_recharge_order("demo", index, request, &offer(credits), NOW, false)
        .unwrap();
    config
        .confirm_recharge_payment(
            "demo",
            &order.id,
            &payment(&format!("cs_{suffix}"), &format!("pi_{suffix}")),
            NOW + 1,
        )
        .unwrap()
}
fn login(config: &SimulationConfig) -> SimulationSession {
    let mut session = SimulationSession::new(config.clone());
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    let packets = session.handle_packet(ClientPacket::StartGame {
        character_index: config.default_character.index,
    });
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::StartGame { result: 4, .. })));
    session
}
fn path(label: &str) -> std::path::PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "mir2-billing-{label}-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("accounts.json")
}

#[test]
fn recharge_paid_pending_application_and_replays_grant_exactly_once() {
    let config = SimulationConfig::default();
    let index = config.default_character.index;
    let order = paid(&config, "request-once", "once", 100);
    assert_eq!(order.state, RechargeOrderState::PaidPending);
    assert_eq!(
        config.recharge_credit_status("demo", index).unwrap(),
        (0, 100)
    );
    let duplicate = config
        .prepare_recharge_order("demo", index, "request-once", &offer(100), NOW + 10, false)
        .unwrap();
    assert_eq!(duplicate, order);
    let applied = config
        .consume_inactive_pending_recharge_credits("demo", index, NOW + 20)
        .unwrap();
    assert_eq!((applied.credit, applied.granted), (100, 100));
    let replay = config
        .confirm_recharge_payment("demo", &order.id, &payment("cs_once", "pi_once"), NOW + 30)
        .unwrap();
    assert_eq!(replay.state, RechargeOrderState::Applied);
    assert_eq!(replay.applied.as_ref().unwrap().revision, applied.revision);
    let repeated = config
        .consume_inactive_pending_recharge_credits("demo", index, NOW + 40)
        .unwrap();
    assert_eq!(
        (repeated.credit, repeated.granted, repeated.revision),
        (100, 0, applied.revision)
    );
}

#[test]
fn recharge_payment_can_precede_checkout_bind_but_late_bind_cannot_change_it() {
    let config = SimulationConfig::default();
    let order = paid(&config, "request-reorder", "reorder", 100);
    assert_eq!(
        config
            .bind_recharge_checkout("demo", &order.id, "cs_reorder", NOW + 2)
            .unwrap(),
        order
    );
    assert_eq!(
        config
            .bind_recharge_checkout("demo", &order.id, "cs_other", NOW + 2)
            .unwrap_err(),
        "billingCheckoutBindingMismatch"
    );
}

#[test]
fn recharge_immutable_identity_amount_currency_mode_and_provider_tuple_reject() {
    let config = SimulationConfig::default();
    let index = config.default_character.index;
    let order = config
        .prepare_recharge_order("demo", index, "identity-request", &offer(100), NOW, false)
        .unwrap();
    config
        .bind_recharge_checkout("demo", &order.id, "cs_identity", NOW + 1)
        .unwrap();
    assert!(config
        .prepare_recharge_order("demo", index + 1, "not-owned", &offer(100), NOW, false)
        .is_err());
    assert_eq!(
        config
            .prepare_recharge_order("demo", index, "identity-request", &offer(101), NOW, false)
            .unwrap_err(),
        "billingRequestIdReused"
    );
    assert!(config
        .confirm_recharge_payment(
            "missing",
            &order.id,
            &payment("cs_identity", "pi_identity"),
            NOW + 2
        )
        .is_err());
    let exact = payment("cs_identity", "pi_identity");
    let mut mismatches = Vec::new();
    let mut p = exact.clone();
    p.amount_minor += 1;
    mismatches.push(p);
    let mut p = exact.clone();
    p.currency = "eur".into();
    mismatches.push(p);
    let mut p = exact.clone();
    p.livemode = true;
    mismatches.push(p);
    let mut p = exact.clone();
    p.session_id = "cs_wrong".into();
    mismatches.push(p);
    for mismatch in mismatches {
        assert_eq!(
            config
                .confirm_recharge_payment("demo", &order.id, &mismatch, NOW + 2)
                .unwrap_err(),
            "billingPaymentBindingMismatch"
        );
    }
    config
        .confirm_recharge_payment("demo", &order.id, &exact, NOW + 2)
        .unwrap();
    let mut changed_intent = exact;
    changed_intent.payment_intent_id = "pi_changed".into();
    assert_eq!(
        config
            .confirm_recharge_payment("demo", &order.id, &changed_intent, NOW + 3)
            .unwrap_err(),
        "billingPaymentBindingMismatch"
    );
    assert_eq!(
        config.recharge_credit_status("demo", index).unwrap(),
        (0, 100)
    );
}

#[test]
fn recharge_provider_session_and_payment_cannot_fund_another_order_or_account() {
    let config = SimulationConfig::default();
    paid(&config, "first-provider", "unique", 100);
    let index = config.default_character.index;
    let second = config
        .prepare_recharge_order("demo", index, "second-provider", &offer(100), NOW, false)
        .unwrap();
    assert_eq!(
        config
            .bind_recharge_checkout("demo", &second.id, "cs_unique", NOW + 2)
            .unwrap_err(),
        "billingProviderBindingAlreadyUsed"
    );
    assert_eq!(
        config
            .confirm_recharge_payment(
                "demo",
                &second.id,
                &payment("cs_second", "pi_unique"),
                NOW + 2
            )
            .unwrap_err(),
        "billingProviderBindingAlreadyUsed"
    );
    config.account_store.lock().unwrap().accounts.insert(
        "other".into(),
        AccountRecord::new(config.default_character.clone()),
    );
    let other = config
        .prepare_recharge_order("other", index, "other-provider", &offer(100), NOW, false)
        .unwrap();
    assert_eq!(
        config
            .confirm_recharge_payment(
                "other",
                &other.id,
                &payment("cs_other", "pi_unique"),
                NOW + 2
            )
            .unwrap_err(),
        "billingProviderBindingAlreadyUsed"
    );
    assert_eq!(
        config.recharge_credit_status("other", index).unwrap(),
        (0, 0)
    );
}

#[test]
fn recharge_overflow_retains_complete_pending_orders_and_recovers_after_spending() {
    let config = SimulationConfig::default()
        .with_billing_monthly_card_credit_price(Some(3))
        .unwrap();
    let index = config.default_character.index;
    // Both payments reserved room before ordinary earnings raised the wallet.
    // The wallet increase is prepared state; the purchase and applications below
    // use the actual durable Source transactions.
    paid(&config, "overflow-first", "overflow_first", 3);
    paid(&config, "overflow-second", "overflow_second", 3);
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .saves
        .get_mut(&index)
        .unwrap()
        .credit = u32::MAX - 5;
    let before = config.recharge_orders("demo", index).unwrap();
    let first = config
        .consume_inactive_pending_recharge_credits("demo", index, NOW + 2)
        .unwrap();
    assert_eq!((first.credit, first.granted), (u32::MAX - 2, 3));
    assert_eq!(first.order_ids.len(), 1);
    let applied = config
        .recharge_order_for_payment("demo", &first.order_ids[0])
        .unwrap();
    let receipt = applied.applied.as_ref().unwrap();
    assert_eq!(
        (receipt.credits, receipt.credit_before, receipt.credit_after),
        (3, u32::MAX - 5, u32::MAX - 2)
    );
    assert_eq!(receipt.revision, first.revision);
    assert_eq!(
        config.recharge_credit_status("demo", index).unwrap(),
        (u32::MAX - 2, 3)
    );
    let waiting = serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap();
    let stalled = config
        .consume_inactive_pending_recharge_credits("demo", index, NOW + 3)
        .unwrap();
    assert_eq!(
        (stalled.credit, stalled.granted, stalled.revision),
        (first.credit, 0, first.revision)
    );
    assert!(stalled.order_ids.is_empty());
    assert_eq!(
        serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap(),
        waiting
    );
    config
        .buy_monthly_card_for_character("demo", index, "overflow-recovery-card", NOW + 4)
        .unwrap();
    assert_eq!(
        config.recharge_credit_status("demo", index).unwrap(),
        (u32::MAX - 5, 3)
    );
    let second = config
        .consume_inactive_pending_recharge_credits("demo", index, NOW + 5)
        .unwrap();
    assert_eq!((second.credit, second.granted), (u32::MAX - 2, 3));
    assert_eq!(second.order_ids.len(), 1);
    assert_ne!(second.order_ids, first.order_ids);
    let after = config.recharge_orders("demo", index).unwrap();
    assert_eq!(after.len(), before.len());
    assert!(after.iter().all(|order| {
        order.state == RechargeOrderState::Applied && order.applied.as_ref().unwrap().credits == 3
    }));
    assert_eq!(
        config.recharge_credit_status("demo", index).unwrap(),
        (u32::MAX - 2, 0)
    );
    assert_eq!(
        config
            .recharge_order_for_payment("demo", &applied.id)
            .unwrap(),
        applied
    );
}

#[test]
fn recharge_new_orders_reserve_unapplied_capacity_and_retry_at_capacity() {
    let config = SimulationConfig::default();
    let index = config.default_character.index;
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .saves
        .get_mut(&index)
        .unwrap()
        .credit = u32::MAX - 5;
    let first = config
        .prepare_recharge_order("demo", index, "reserve-first", &offer(3), NOW, false)
        .unwrap();
    config
        .prepare_recharge_order("demo", index, "reserve-second", &offer(2), NOW, false)
        .unwrap();
    for state in [
        RechargeOrderState::Prepared,
        RechargeOrderState::CheckoutBound,
        RechargeOrderState::PaidPending,
        RechargeOrderState::Applied,
    ] {
        match state {
            RechargeOrderState::CheckoutBound => {
                config
                    .bind_recharge_checkout("demo", &first.id, "cs_reserve", NOW + 1)
                    .unwrap();
            }
            RechargeOrderState::PaidPending => {
                config
                    .confirm_recharge_payment(
                        "demo",
                        &first.id,
                        &payment("cs_reserve", "pi_reserve"),
                        NOW + 2,
                    )
                    .unwrap();
            }
            RechargeOrderState::Applied => {
                assert_eq!(
                    config
                        .consume_inactive_pending_recharge_credits("demo", index, NOW + 3)
                        .unwrap()
                        .granted,
                    3
                );
            }
            _ => {}
        }
        let current = config
            .recharge_order_for_payment("demo", &first.id)
            .unwrap();
        assert_eq!(current.state, state);
        let before = serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap();
        assert_eq!(
            config
                .prepare_recharge_order(
                    "demo",
                    index,
                    "reserve-rejected",
                    &offer(1),
                    NOW + 4,
                    false
                )
                .unwrap_err(),
            "billingCreditCapacityExceeded"
        );
        assert_eq!(
            serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap(),
            before
        );
        assert_eq!(
            config
                .prepare_recharge_order("demo", index, "reserve-first", &offer(3), NOW + 5, false)
                .unwrap(),
            current
        );
    }
}

#[test]
fn recharge_skips_large_pending_payment_and_applies_smaller_complete_payment() {
    let config = SimulationConfig::default();
    let index = config.default_character.index;
    let large = paid(&config, "skip-large", "skip_large", 5);
    let small = paid(&config, "fit-small", "fit_small", 2);
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .saves
        .get_mut(&index)
        .unwrap()
        .credit = u32::MAX - 2;
    let applied = config
        .consume_inactive_pending_recharge_credits("demo", index, NOW + 2)
        .unwrap();
    assert_eq!((applied.credit, applied.granted), (u32::MAX, 2));
    assert_eq!(applied.order_ids, vec![small.id.clone()]);
    assert_eq!(
        config
            .recharge_order_for_payment("demo", &large.id)
            .unwrap(),
        large
    );
    let small = config
        .recharge_order_for_payment("demo", &small.id)
        .unwrap();
    assert_eq!(small.applied.as_ref().unwrap().credits, 2);
    assert_eq!(
        config.recharge_credit_status("demo", index).unwrap(),
        (u32::MAX, 5)
    );
    let before = serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap();
    let stalled = config
        .consume_inactive_pending_recharge_credits("demo", index, NOW + 3)
        .unwrap();
    assert_eq!(
        (stalled.credit, stalled.granted, stalled.revision),
        (u32::MAX, 0, applied.revision)
    );
    assert_eq!(
        serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap(),
        before
    );
}

#[test]
fn recharge_no_fitting_payment_never_publishes_unsaved_owner_and_spending_recovers() {
    let config = SimulationConfig::default()
        .with_billing_monthly_card_credit_price(Some(3))
        .unwrap();
    let index = config.default_character.index;
    let order = paid(&config, "active-capacity", "active_capacity", 3);
    let mut owner = login(&config);
    let mut checkpoint = owner.active_character_checkpoint().unwrap();
    // Prepared unsaved earnings, not a claim about natural gameplay earnings.
    checkpoint.credit = u32::MAX;
    checkpoint.gold = 321;
    owner
        .restore_active_character_checkpoint(&checkpoint)
        .unwrap();
    let durable_before = serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap();
    assert!(owner.consume_pending_recharge_credits().unwrap().is_empty());
    assert_eq!(
        serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap(),
        durable_before
    );
    assert_eq!(
        serde_json::to_value(owner.active_character_checkpoint().unwrap()).unwrap(),
        serde_json::to_value(&checkpoint).unwrap()
    );
    assert_eq!(
        config
            .recharge_order_for_payment("demo", &order.id)
            .unwrap(),
        order
    );
    let purchased = owner
        .buy_billing_monthly_card("active-capacity-recovery", NOW + 2)
        .unwrap();
    assert!(purchased
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoseCredit { credit: 3 })));
    assert_eq!(
        owner.active_character_checkpoint().unwrap().credit,
        u32::MAX - 3
    );
    assert_eq!(
        owner.poll_pending_recharge_credits(NOW + 5_000).unwrap(),
        vec![ServerPacket::GainedCredit { credit: 3 }]
    );
    let current = owner.active_character_checkpoint().unwrap();
    assert_eq!(
        (current.credit, current.gold, current.revision),
        (u32::MAX, 321, checkpoint.revision + 2)
    );
    assert_eq!(
        config.recharge_credit_status("demo", index).unwrap(),
        (u32::MAX, 0)
    );
    assert_eq!(
        config
            .recharge_order_for_payment("demo", &order.id)
            .unwrap()
            .applied
            .as_ref()
            .unwrap()
            .credits,
        3
    );
    assert!(owner
        .poll_pending_recharge_credits(NOW + 10_000)
        .unwrap()
        .is_empty());
}

#[test]
fn recharge_real_character_select_delete_rejects_every_unsettled_order_state() {
    for state in [
        RechargeOrderState::Prepared,
        RechargeOrderState::CheckoutBound,
        RechargeOrderState::PaidPending,
        RechargeOrderState::Review,
    ] {
        let config = SimulationConfig::default();
        let index = config.default_character.index;
        let order = config
            .prepare_recharge_order("demo", index, "delete-order", &offer(10), NOW, false)
            .unwrap();
        match state {
            RechargeOrderState::CheckoutBound => {
                config
                    .bind_recharge_checkout("demo", &order.id, "cs_delete", NOW + 1)
                    .unwrap();
            }
            RechargeOrderState::PaidPending => {
                config
                    .confirm_recharge_payment(
                        "demo",
                        &order.id,
                        &payment("cs_delete", "pi_delete"),
                        NOW + 1,
                    )
                    .unwrap();
            }
            RechargeOrderState::Review => {
                config
                    .mark_recharge_review("demo", &order.id, "refund", "evt_delete", NOW + 1)
                    .unwrap();
            }
            _ => {}
        }
        let mut selected = SimulationSession::new(config.clone());
        let before = serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap();
        assert!(selected
            .handle_packet(ClientPacket::DeleteCharacter {
                character_index: index
            })
            .is_empty());
        assert_eq!(
            serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap(),
            before
        );
        let login = selected.handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        });
        assert!(login
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
        let before = serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap();
        assert_eq!(
            selected.handle_packet(ClientPacket::DeleteCharacter {
                character_index: index
            }),
            vec![ServerPacket::DeleteCharacter { result: 1 }]
        );
        assert_eq!(
            serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap(),
            before
        );
        assert_eq!(
            config
                .recharge_order_for_payment("demo", &order.id)
                .unwrap()
                .state,
            state
        );
    }
}

#[test]
fn recharge_delete_protects_cash_wallet_and_unused_card_then_preserves_archived_receipts() {
    let file = path("archived-order");
    let config = SimulationConfig::default()
        .with_account_store_path(&file)
        .with_billing_monthly_card_credit_price(Some(10))
        .unwrap();
    let index = config.default_character.index;
    let order = paid(&config, "delete-cash", "delete_cash", 10);
    config
        .consume_inactive_pending_recharge_credits("demo", index, NOW + 2)
        .unwrap();
    let applied = config
        .recharge_order_for_payment("demo", &order.id)
        .unwrap();
    let mut selected = SimulationSession::new(config.clone());
    let login = selected.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    assert!(login
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    assert_eq!(
        selected.handle_packet(ClientPacket::DeleteCharacter {
            character_index: index
        }),
        vec![ServerPacket::DeleteCharacter { result: 1 }]
    );
    assert_eq!(
        config.recharge_credit_status("demo", index).unwrap(),
        (10, 0)
    );
    let card = config
        .buy_monthly_card_for_character("demo", index, "delete-unused-card", NOW + 3)
        .unwrap();
    assert_eq!(
        config.recharge_credit_status("demo", index).unwrap(),
        (0, 0)
    );
    let before = serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap();
    assert_eq!(
        selected.handle_packet(ClientPacket::DeleteCharacter {
            character_index: index
        }),
        vec![ServerPacket::DeleteCharacter { result: 1 }]
    );
    assert_eq!(
        serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap(),
        before
    );
    let activated = config
        .activate_monthly_card_for_character("demo", index, card.unique_id, NOW + 4)
        .unwrap();
    assert_eq!(
        selected.handle_packet(ClientPacket::DeleteCharacter {
            character_index: index
        }),
        vec![ServerPacket::DeleteCharacterSuccess {
            character_index: index
        }]
    );
    assert!(config.recharge_orders("demo", index).is_err());
    assert_eq!(
        config
            .recharge_order_for_payment("demo", &order.id)
            .unwrap(),
        applied
    );
    assert_eq!(
        config
            .confirm_recharge_payment(
                "demo",
                &order.id,
                &payment("cs_delete_cash", "pi_delete_cash"),
                NOW + 5
            )
            .unwrap(),
        applied
    );
    config
        .mark_recharge_review(
            "demo",
            &order.id,
            "charge.refunded",
            "evt_archive_refund",
            NOW + 6,
        )
        .unwrap();
    let reviewed = config
        .recharge_order_for_payment("demo", &order.id)
        .unwrap();
    assert_eq!(reviewed.state, RechargeOrderState::Review);
    assert_eq!(reviewed.applied, applied.applied);
    assert_eq!(reviewed.review_events.len(), 1);
    assert_eq!(
        config
            .recharge_order_for_payment("other", &order.id)
            .unwrap_err(),
        "billingOrderMissing"
    );
    let card_ledger = config.account_store.lock().unwrap().accounts["demo"]
        .monthly_card
        .clone()
        .unwrap();
    assert_eq!(
        card_ledger.expires_at_ms,
        activated.status.expires_at_ms.unwrap()
    );
    assert!(card_ledger
        .item_receipts
        .values()
        .all(|unit| unit.redeemed_at_ms.is_some()));
    drop(selected);
    drop(config);
    let reopened = SimulationConfig::default().with_account_store_path(&file);
    assert_eq!(
        reopened
            .recharge_order_for_payment("demo", &order.id)
            .unwrap(),
        reviewed
    );
    let store = reopened.account_store.lock().unwrap();
    assert!(!store.accounts["demo"].saves.contains_key(&index));
    assert!(!store.accounts["demo"]
        .characters
        .iter()
        .any(|character| character.index == index));
    assert_eq!(
        store.accounts["demo"].monthly_card.as_ref().unwrap(),
        &card_ledger
    );
}

#[test]
fn recharge_archived_unapplied_order_records_verified_payment_and_review_without_roster() {
    let config = SimulationConfig::default();
    let index = config.default_character.index;
    let order = config
        .prepare_recharge_order(
            "demo",
            index,
            "legacy-archived-order",
            &offer(10),
            NOW,
            false,
        )
        .unwrap();
    let bound = config
        .bind_recharge_checkout("demo", &order.id, "cs_archived", NOW + 1)
        .unwrap();
    // Prepared historical archive from before the deletion guard. New ordinary
    // DeleteCharacter packets cannot produce this unpaid, missing-roster state.
    {
        let mut store = config.account_store.lock().unwrap();
        let account = store.accounts.get_mut("demo").unwrap();
        account
            .characters
            .retain(|character| character.index != index);
        account.saves.remove(&index);
    }
    assert!(config.recharge_orders("demo", index).is_err());
    assert_eq!(
        config
            .recharge_order_for_payment("demo", &order.id)
            .unwrap(),
        bound
    );
    let confirmed = config
        .confirm_recharge_payment(
            "demo",
            &order.id,
            &payment("cs_archived", "pi_archived"),
            NOW + 2,
        )
        .unwrap();
    assert_eq!(confirmed.state, RechargeOrderState::PaidPending);
    assert_eq!(confirmed.character_index, index);
    assert_eq!(
        config
            .recharge_order_for_payment("demo", &order.id)
            .unwrap(),
        confirmed
    );
    assert!(config
        .consume_inactive_pending_recharge_credits("demo", index, NOW + 3)
        .is_err());
    config
        .mark_recharge_review(
            "demo",
            &order.id,
            "charge.refunded",
            "evt_archived",
            NOW + 4,
        )
        .unwrap();
    let reviewed = config
        .recharge_order_for_payment("demo", &order.id)
        .unwrap();
    assert_eq!(reviewed.state, RechargeOrderState::Review);
    assert_eq!(reviewed.payment_intent_id, confirmed.payment_intent_id);
    assert_eq!(reviewed.checkout_session_id, confirmed.checkout_session_id);
    assert_eq!(reviewed.paid_at_ms, confirmed.paid_at_ms);
    assert!(reviewed.applied.is_none());
    assert_eq!(reviewed.review_events.len(), 1);
    let store = config.account_store.lock().unwrap();
    assert!(!store.accounts["demo"].saves.contains_key(&index));
    assert!(!store.accounts["demo"]
        .characters
        .iter()
        .any(|character| character.index == index));
}

#[test]
fn recharge_character_deletion_keeps_unfunded_credits_compatible() {
    let config = SimulationConfig::default();
    let index = config.default_character.index;
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .saves
        .get_mut(&index)
        .unwrap()
        .credit = 77;
    let mut selected = SimulationSession::new(config.clone());
    selected.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    assert_eq!(
        selected.handle_packet(ClientPacket::DeleteCharacter {
            character_index: index
        }),
        vec![ServerPacket::DeleteCharacterSuccess {
            character_index: index
        }]
    );
    let store = config.account_store.lock().unwrap();
    assert!(store.accounts["demo"].billing.is_none());
    assert!(!store.accounts["demo"].saves.contains_key(&index));
}

#[test]
fn recharge_review_before_payment_is_terminal_and_review_after_spend_never_rewinds() {
    let config = SimulationConfig::default();
    let index = config.default_character.index;
    let before = config
        .prepare_recharge_order("demo", index, "review-before", &offer(100), NOW, false)
        .unwrap();
    config
        .mark_recharge_review("demo", &before.id, "refund", "evt_before", NOW + 1)
        .unwrap();
    let delayed = config
        .confirm_recharge_payment(
            "demo",
            &before.id,
            &payment("cs_before", "pi_before"),
            NOW + 2,
        )
        .unwrap();
    assert_eq!(delayed.state, RechargeOrderState::Review);
    assert_eq!(
        config
            .consume_inactive_pending_recharge_credits("demo", index, NOW + 3)
            .unwrap()
            .granted,
        0
    );
    let after = paid(&config, "review-after", "after", 100);
    let applied = config
        .consume_inactive_pending_recharge_credits("demo", index, NOW + 3)
        .unwrap();
    // A prepared spent-balance fixture proves review does not restore a snapshot.
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .saves
        .get_mut(&index)
        .unwrap()
        .credit = 10;
    config
        .mark_recharge_review("demo", &after.id, "dispute", "evt_after", NOW + 4)
        .unwrap();
    let original = config.recharge_orders("demo", index).unwrap();
    config
        .mark_recharge_review("demo", &after.id, "dispute", "evt_after", NOW + 5)
        .unwrap();
    assert_eq!(config.recharge_orders("demo", index).unwrap(), original);
    assert_eq!(
        config.recharge_credit_status("demo", index).unwrap(),
        (10, 0)
    );
    let receipt = original
        .iter()
        .find(|o| o.id == after.id)
        .unwrap()
        .applied
        .as_ref()
        .unwrap();
    assert_eq!(
        (receipt.revision, receipt.credit_after),
        (applied.revision, 100)
    );
}

#[test]
fn recharge_owner_consumes_current_checkpoint_and_preserves_unsaved_personal_state() {
    let config = SimulationConfig::default();
    let mut owner = login(&config);
    let mut unsaved = owner.active_character_checkpoint().unwrap();
    unsaved.gold = 321;
    owner.restore_active_character_checkpoint(&unsaved).unwrap();
    let order = paid(&config, "owner-checkpoint", "owner", 100);
    let packets = owner.consume_pending_recharge_credits().unwrap();
    assert_eq!(packets, vec![ServerPacket::GainedCredit { credit: 100 }]);
    let current = owner.active_character_checkpoint().unwrap();
    assert_eq!((current.credit, current.gold), (100, 321));
    assert_eq!(current.revision, unsaved.revision + 1);
    assert!(owner.consume_pending_recharge_credits().unwrap().is_empty());
    owner.save_active_character().unwrap();
    let store = config.account_store.lock().unwrap();
    let durable = &store.accounts["demo"].saves[&config.default_character.index];
    assert_eq!((durable.credit, durable.gold), (100, 321));
    assert_eq!(
        store.accounts["demo"].billing.as_ref().unwrap().orders[&order.id].state,
        RechargeOrderState::Applied
    );
}

#[test]
fn recharge_stale_online_checkpoint_cannot_overwrite_newer_save_or_apply_payment() {
    let config = SimulationConfig::default();
    let mut stale = login(&config);
    let fresh = login(&config);
    fresh.save_active_character().unwrap();
    paid(&config, "stale-checkpoint", "stale", 100);
    assert_eq!(
        stale.consume_pending_recharge_credits().unwrap_err(),
        "billingOwnerCheckpointStale"
    );
    assert!(stale.save_active_character().is_err());
    assert_eq!(
        config
            .recharge_credit_status("demo", config.default_character.index)
            .unwrap(),
        (0, 100)
    );
}

#[test]
fn recharge_tick_poll_waits_five_seconds_then_applies_the_payment_once() {
    let config = SimulationConfig::default();
    let index = config.default_character.index;
    let order = config
        .prepare_recharge_order("demo", index, "tick-window", &offer(100), NOW, false)
        .unwrap();
    let mut owner = login(&config);
    let before = owner.active_character_checkpoint().unwrap();
    assert!(owner.poll_pending_recharge_credits(NOW).unwrap().is_empty());
    config
        .confirm_recharge_payment(
            "demo",
            &order.id,
            &payment("cs_tickwindow", "pi_tickwindow"),
            NOW + 1,
        )
        .unwrap();
    assert!(owner
        .poll_pending_recharge_credits(NOW + 4_999)
        .unwrap()
        .is_empty());
    let waiting = owner.active_character_checkpoint().unwrap();
    assert_eq!((waiting.credit, waiting.revision), (0, before.revision));
    assert_eq!(
        config.recharge_credit_status("demo", index).unwrap(),
        (0, 100)
    );
    assert_eq!(
        owner.poll_pending_recharge_credits(NOW + 5_000).unwrap(),
        vec![ServerPacket::GainedCredit { credit: 100 }]
    );
    config
        .confirm_recharge_payment(
            "demo",
            &order.id,
            &payment("cs_tickwindow", "pi_tickwindow"),
            NOW + 6_000,
        )
        .unwrap();
    assert!(owner
        .poll_pending_recharge_credits(NOW + 9_999)
        .unwrap()
        .is_empty());
    assert!(owner
        .poll_pending_recharge_credits(NOW + 10_000)
        .unwrap()
        .is_empty());
    let current = owner.active_character_checkpoint().unwrap();
    assert_eq!(
        (current.credit, current.revision),
        (100, before.revision + 1)
    );
    let orders = config.recharge_orders("demo", index).unwrap();
    assert_eq!(orders[0].state, RechargeOrderState::Applied);
    assert_eq!(
        orders[0].applied.as_ref().unwrap().revision,
        current.revision
    );
}

#[test]
fn recharge_explicit_refresh_and_purchase_preflight_bypass_the_tick_window() {
    let config = SimulationConfig::default();
    let index = config.default_character.index;
    config
        .prepare_recharge_order("demo", index, "refresh-now", &offer(10), NOW, false)
        .unwrap();
    let mut owner = login(&config);
    let initial_revision = owner.active_character_checkpoint().unwrap().revision;
    assert!(owner.poll_pending_recharge_credits(NOW).unwrap().is_empty());
    paid(&config, "refresh-now", "refreshnow", 10);
    assert!(owner
        .poll_pending_recharge_credits(NOW + 1)
        .unwrap()
        .is_empty());
    assert_eq!(
        owner.consume_pending_recharge_credits().unwrap(),
        vec![ServerPacket::GainedCredit { credit: 10 }]
    );
    paid(&config, "purchase-now", "purchasenow", 25);
    assert_eq!(
        owner.refresh_recharge_credit_preflight().unwrap(),
        vec![ServerPacket::GainedCredit { credit: 25 }]
    );
    assert!(owner.consume_pending_recharge_credits().unwrap().is_empty());
    assert!(owner
        .poll_pending_recharge_credits(NOW + 2)
        .unwrap()
        .is_empty());
    assert!(owner
        .poll_pending_recharge_credits(NOW + 5_000)
        .unwrap()
        .is_empty());
    let current = owner.active_character_checkpoint().unwrap();
    assert_eq!(
        (current.credit, current.revision),
        (35, initial_revision + 2)
    );
    assert_eq!(
        config.recharge_credit_status("demo", index).unwrap(),
        (35, 0)
    );
    assert!(config
        .recharge_orders("demo", index)
        .unwrap()
        .iter()
        .all(|order| order.state == RechargeOrderState::Applied));
}

#[test]
fn recharge_tick_skips_empty_ledgers_and_never_uses_an_unauthenticated_demo_owner() {
    let config = SimulationConfig::default();
    let index = config.default_character.index;
    let mut owner = login(&config);
    assert!(owner.poll_pending_recharge_credits(NOW).unwrap().is_empty());
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .billing = Some(mir2_simulation::billing::BillingLedger::default());
    assert!(owner
        .poll_pending_recharge_credits(NOW + 1)
        .unwrap()
        .is_empty());
    // Empty ledgers must not arm a five-second delay for the first real order.
    paid(&config, "first-real-order", "firstreal", 7);
    let mut unauthenticated = SimulationSession::new(config.clone());
    assert!(unauthenticated
        .poll_pending_recharge_credits(NOW + 2)
        .unwrap()
        .is_empty());
    assert!(unauthenticated
        .consume_pending_recharge_credits()
        .unwrap()
        .is_empty());
    assert!(unauthenticated.billing_authenticated_config().is_err());
    assert_eq!(
        config.recharge_credit_status("demo", index).unwrap(),
        (0, 7)
    );
    assert_eq!(
        owner.poll_pending_recharge_credits(NOW + 2).unwrap(),
        vec![ServerPacket::GainedCredit { credit: 7 }]
    );
    assert_eq!(
        config.recharge_credit_status("demo", index).unwrap(),
        (7, 0)
    );
}

#[test]
fn recharge_enabled_tick_polls_without_cached_ledger_and_throttles_failures() {
    let mut config = SimulationConfig::default()
        .with_billing_monthly_card_credit_price(Some(10))
        .unwrap();
    let index = config.default_character.index;
    let mut owner = login(&config);
    assert!(config.account_store.lock().unwrap().accounts["demo"]
        .billing
        .is_none());
    // An unavailable Source repository proves the first enabled Tick actually
    // attempts refresh despite a cached absent ledger, with no live DB needed.
    config.account_store_database_mode = mir2_simulation::AccountStoreDatabaseMode::SourceOfTruth;
    config.account_store_database_url = None;
    owner.rebind_account_store(&config);
    assert_eq!(
        owner.poll_pending_recharge_credits(NOW).unwrap_err(),
        "postgres account source is unavailable"
    );
    assert!(owner
        .poll_pending_recharge_credits(NOW + 4_999)
        .unwrap()
        .is_empty());
    // Explicit refresh is never suppressed by the failed Tick's interval.
    assert_eq!(
        owner.consume_pending_recharge_credits().unwrap_err(),
        "postgres account source is unavailable"
    );
    config.account_store_database_mode = mir2_simulation::AccountStoreDatabaseMode::Mirror;
    owner.rebind_account_store(&config);
    paid(&config, "enabled-first-order", "enabledfirst", 13);
    assert_eq!(
        owner.poll_pending_recharge_credits(NOW + 5_000).unwrap(),
        vec![ServerPacket::GainedCredit { credit: 13 }]
    );
    assert_eq!(
        config.recharge_credit_status("demo", index).unwrap(),
        (13, 0)
    );
    assert!(owner
        .poll_pending_recharge_credits(NOW + 10_000)
        .unwrap()
        .is_empty());
}

#[test]
fn recharge_file_reload_retains_pending_and_applied_receipts_without_regrant() {
    let path = path("reload");
    let config = SimulationConfig::default().with_account_store_path(&path);
    let index = config.default_character.index;
    let order = paid(&config, "file-reload", "reload", 100);
    let durable = FileAccountStoreRepository::new(&path)
        .load(config.default_character.clone())
        .unwrap();
    assert_eq!(durable.schema_version, 8);
    assert_eq!(
        durable.accounts["demo"].billing.as_ref().unwrap().orders[&order.id].state,
        RechargeOrderState::PaidPending
    );
    drop(config);
    let reopened = SimulationConfig::default().with_account_store_path(&path);
    let applied = reopened
        .consume_inactive_pending_recharge_credits("demo", index, NOW + 2)
        .unwrap();
    drop(reopened);
    let reopened = SimulationConfig::default().with_account_store_path(&path);
    assert_eq!(
        reopened
            .consume_inactive_pending_recharge_credits("demo", index, NOW + 3)
            .unwrap()
            .granted,
        0
    );
    let again = reopened
        .confirm_recharge_payment(
            "demo",
            &order.id,
            &payment("cs_reload", "pi_reload"),
            NOW + 4,
        )
        .unwrap();
    assert_eq!(again.applied.unwrap().revision, applied.revision);
}

#[test]
fn recharge_legacy_schema_upgrades_empty_but_cannot_promote_injected_paid_authority() {
    let config = SimulationConfig::default();
    let path = path("schema");
    let mut legacy = serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap();
    legacy["schemaVersion"] = 7.into();
    std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
    assert_eq!(
        FileAccountStoreRepository::new(&path)
            .load(config.default_character.clone())
            .unwrap()
            .schema_version,
        8
    );
    paid(&config, "injected-schema", "injected", 100);
    let mut injected = serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap();
    injected["schemaVersion"] = 7.into();
    std::fs::write(&path, serde_json::to_vec(&injected).unwrap()).unwrap();
    assert_eq!(
        FileAccountStoreRepository::new(&path)
            .load(config.default_character.clone())
            .unwrap_err(),
        "billingSchemaInvalid"
    );
    injected["schemaVersion"] = 8.into();
    let orders = injected["accounts"]["demo"]["billing"]["orders"]
        .as_object_mut()
        .unwrap();
    orders.values_mut().next().unwrap()["paymentIntentId"] = serde_json::Value::Null;
    std::fs::write(&path, serde_json::to_vec(&injected).unwrap()).unwrap();
    assert!(FileAccountStoreRepository::new(&path)
        .load(config.default_character.clone())
        .is_err());
}

#[test]
fn recharge_schema_seven_cannot_promote_real_monthly_card_item_receipts() {
    let config = SimulationConfig::default()
        .with_billing_monthly_card_credit_price(Some(10))
        .unwrap();
    let index = config.default_character.index;
    let path = path("monthly-item-schema");
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .monthly_card = Some(mir2_simulation::monthly_card::MonthlyCardLedger::default());
    let mut legacy = serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap();
    legacy["schemaVersion"] = 7.into();
    std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
    assert_eq!(
        FileAccountStoreRepository::new(&path)
            .load(config.default_character.clone())
            .unwrap()
            .schema_version,
        8
    );
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .saves
        .get_mut(&index)
        .unwrap()
        .credit = 10;
    config
        .buy_monthly_card_for_character("demo", index, "schema-seven-card", NOW)
        .unwrap();
    let store = config.account_store.lock().unwrap();
    assert!(store.accounts["demo"].billing.is_none());
    assert_eq!(
        store.accounts["demo"]
            .monthly_card
            .as_ref()
            .unwrap()
            .item_receipts
            .len(),
        1
    );
    let mut injected = serde_json::to_value(&*store).unwrap();
    drop(store);
    injected["schemaVersion"] = 7.into();
    std::fs::write(&path, serde_json::to_vec(&injected).unwrap()).unwrap();
    assert_eq!(
        FileAccountStoreRepository::new(&path)
            .load(config.default_character.clone())
            .unwrap_err(),
        "billingSchemaInvalid"
    );
    injected["schemaVersion"] = 8.into();
    std::fs::write(&path, serde_json::to_vec(&injected).unwrap()).unwrap();
    let loaded = FileAccountStoreRepository::new(&path)
        .load(config.default_character.clone())
        .unwrap();
    assert_eq!(
        loaded.accounts["demo"]
            .monthly_card
            .as_ref()
            .unwrap()
            .item_receipts
            .len(),
        1
    );
}

#[test]
fn recharge_paid_application_receipt_cannot_be_malformed_or_rebound_on_reload() {
    let config = SimulationConfig::default();
    let index = config.default_character.index;
    let order = paid(&config, "receipt-binding", "receipt", 100);
    config
        .consume_inactive_pending_recharge_credits("demo", index, NOW + 2)
        .unwrap();
    let path = path("receipt");
    let mut value = serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap();
    value["accounts"]["demo"]["billing"]["orders"][&order.id]["applied"]["creditAfter"] =
        999.into();
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(
        FileAccountStoreRepository::new(&path)
            .load(config.default_character.clone())
            .unwrap_err(),
        "billingApplicationReceiptInvalid"
    );
}

#[test]
fn recharge_monthly_card_credit_price_is_explicit_positive_and_default_disabled() {
    assert_eq!(
        SimulationConfig::default().billing_monthly_card_credit_price,
        None
    );
    assert!(SimulationConfig::default()
        .with_billing_monthly_card_credit_price(Some(0))
        .is_err());
    assert_eq!(
        SimulationConfig::default()
            .with_billing_monthly_card_credit_price(Some(300))
            .unwrap()
            .billing_monthly_card_credit_price,
        Some(300)
    );
}

#[cfg(feature = "test-support")]
#[test]
fn recharge_before_persist_failure_rolls_back_confirm_and_owner_application() {
    use mir2_simulation::AccountStoreTransactionFault;
    let config = SimulationConfig::default();
    let mut owner = login(&config);
    let index = config.default_character.index;
    let order = config
        .prepare_recharge_order("demo", index, "rollback-prepare", &offer(100), NOW, false)
        .unwrap();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(config
        .confirm_recharge_payment(
            "demo",
            &order.id,
            &payment("cs_rollback", "pi_rollback"),
            NOW + 1
        )
        .is_err());
    assert_eq!(
        config.recharge_orders("demo", index).unwrap()[0].state,
        RechargeOrderState::Prepared
    );
    config
        .confirm_recharge_payment(
            "demo",
            &order.id,
            &payment("cs_rollback", "pi_rollback"),
            NOW + 1,
        )
        .unwrap();
    let before = owner.active_character_checkpoint().unwrap();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(owner.consume_pending_recharge_credits().is_err());
    assert_eq!(
        serde_json::to_value(owner.active_character_checkpoint().unwrap()).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    assert_eq!(
        config.recharge_credit_status("demo", index).unwrap(),
        (0, 100)
    );
    assert_eq!(
        owner.consume_pending_recharge_credits().unwrap(),
        vec![ServerPacket::GainedCredit { credit: 100 }]
    );
}

#[cfg(feature = "test-support")]
#[test]
fn recharge_unknown_file_publication_freezes_without_claiming_payment_rollback() {
    use mir2_simulation::AccountStoreTransactionFault;
    let path = path("unknown");
    let config = SimulationConfig::default().with_account_store_path(&path);
    let order = config
        .prepare_recharge_order(
            "demo",
            config.default_character.index,
            "unknown-publication",
            &offer(100),
            NOW,
            false,
        )
        .unwrap();
    config.inject_account_store_transaction_fault(
        AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync,
    );
    let error = config
        .confirm_recharge_payment(
            "demo",
            &order.id,
            &payment("cs_unknown", "pi_unknown"),
            NOW + 1,
        )
        .unwrap_err();
    assert!(
        error.contains("ACCOUNT_STORE_COMMIT_OUTCOME_UNKNOWN"),
        "{error}"
    );
    assert!(config
        .prepare_recharge_order(
            "demo",
            config.default_character.index,
            "blocked-after-unknown",
            &offer(100),
            NOW + 2,
            false
        )
        .unwrap_err()
        .contains("frozen"));
    // The actual replaced file may already contain the confirmed payment.
    let file = FileAccountStoreRepository::new(&path)
        .load(config.default_character.clone())
        .unwrap();
    assert_eq!(
        file.accounts["demo"].billing.as_ref().unwrap().orders[&order.id].state,
        RechargeOrderState::PaidPending
    );
}
