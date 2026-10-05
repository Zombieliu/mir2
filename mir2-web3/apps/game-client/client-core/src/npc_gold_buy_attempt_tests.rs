use super::*;

type Ticket = (u64, u16);
fn opened(authority: &str) -> NpcGoldBuyAttemptSlot<Ticket> {
    let mut slot = NpcGoldBuyAttemptSlot::new();
    assert!(slot.observe_connection(NpcGoldBuyConnectionEpoch { run: 1, connection: 1 }));
    assert!(slot.observe_authority(authority));
    slot.set_available(true);
    slot
}
fn bound(slot: &mut NpcGoldBuyAttemptSlot<Ticket>, ticket: Ticket) -> NpcGoldBuyAttemptToken {
    let token = slot.reserve().expect("manual reserve");
    assert!(slot.bind(token, ticket));
    token
}

#[test]
fn only_complete_observed_authority_and_explicit_availability_allow_reserve() {
    let mut slot = NpcGoldBuyAttemptSlot::<Ticket>::new();
    assert_eq!(slot.authority_revision(), 0);
    assert!(!slot.can_reserve());
    slot.set_available(true);
    assert!(slot.reserve().is_none());
    assert!(!slot.observe_authority(""));
    assert!(slot.observe_connection(NpcGoldBuyConnectionEpoch { run: 1, connection: 1 }));
    assert!(slot.observe_authority("owner:1;service:1;catalog:full;inventory:full"));
    assert_eq!(slot.authority_revision(), 1);
    assert!(slot.reserve().is_none(), "observation does not implicitly reopen UI");
    slot.set_available(true);
    assert!(slot.can_reserve());
    let token = slot.reserve().unwrap();
    assert_eq!(token.value(), 1);
    assert_eq!(slot.flight().unwrap().ticket, None);
    assert_eq!(slot.flight().unwrap().phase, NpcGoldBuyAttemptPhase::Queued);
    assert_eq!(slot.flight().unwrap().authority_revision, 1);
}

#[test]
fn same_catalog_uid_with_different_quantities_cannot_create_two_flights() {
    let mut slot = opened("A");
    let token = bound(&mut slot, (0, 1));
    assert!(slot.reserve().is_none());
    assert!(!slot.bind(token, (0, 2)), "bound immutable ticket cannot be replaced");
    assert!(!slot.begin_entry(token, &(0, 2)));
    assert!(slot.begin_entry(token, &(0, 1)));
    assert!(slot.reserve().is_none());
    assert!(!slot.begin_entry(token, &(0, 1)), "entry is one-shot");
}

#[test]
fn reject_unpublished_is_queued_only_and_does_not_reuse_token() {
    let mut slot = opened("A");
    let queued = slot.reserve().unwrap();
    assert!(slot.reject_unpublished(queued));
    assert!(!slot.reject_unpublished(queued));
    assert_eq!(slot.history().back().unwrap().phase, NpcGoldBuyAttemptPhase::DefinitelyUnsent);
    let next = bound(&mut slot, (7, 2));
    assert!(next.value() > queued.value());
    assert!(!slot.reject_unpublished(next));
    assert_eq!(slot.flight().unwrap().phase, NpcGoldBuyAttemptPhase::Bound);
}

#[test]
fn bound_definitely_unsent_allows_a_new_manual_attempt_but_no_automatic_reserve() {
    let mut slot = opened("A");
    let first = bound(&mut slot, (7, 1));
    assert!(!slot.apply_receipt(first, &(7, 2), NpcGoldBuyAttemptOutcome::DefinitelyUnsent));
    assert!(!slot.apply_receipt(first, &(7, 1), NpcGoldBuyAttemptOutcome::Flushed));
    assert!(!slot.apply_receipt(first, &(7, 1), NpcGoldBuyAttemptOutcome::Unknown));
    assert!(slot.apply_receipt(first, &(7, 1), NpcGoldBuyAttemptOutcome::DefinitelyUnsent));
    assert!(slot.flight().is_none());
    assert!(slot.can_reserve());
    assert_eq!(slot.history().len(), 1);
    let next = slot.reserve().unwrap();
    assert!(next.value() > first.value());
}

#[test]
fn old_receipt_with_reused_ticket_cannot_release_new_token() {
    let mut slot = opened("A");
    let first = bound(&mut slot, (7, 1));
    assert!(slot.apply_receipt(first, &(7, 1), NpcGoldBuyAttemptOutcome::DefinitelyUnsent));
    let next = bound(&mut slot, (7, 1));
    let live = slot.flight().unwrap().clone();
    for outcome in [NpcGoldBuyAttemptOutcome::DefinitelyUnsent,
        NpcGoldBuyAttemptOutcome::Flushed, NpcGoldBuyAttemptOutcome::Unknown] {
        assert!(!slot.apply_receipt(first, &(7, 1), outcome));
        assert_eq!(slot.flight(), Some(&live));
    }
    assert!(slot.begin_entry(next, &(7, 1)));
}

#[test]
fn a_b_a_authority_revisions_retire_preentry_but_preserve_entered_tickets() {
    let mut slot = opened("A");
    let first = bound(&mut slot, (7, 1));
    assert!(slot.observe_authority("B"));
    assert_eq!(slot.history().back().unwrap().phase, NpcGoldBuyAttemptPhase::DefinitelyUnsent);
    let second = bound(&mut slot, (7, 1));
    assert!(slot.begin_entry(second, &(7, 1)));
    let barrier = slot.flight().unwrap().clone();
    assert!(slot.observe_authority("A"));
    assert_eq!(slot.authority_revision(), 3);
    assert_eq!(slot.flight(), Some(&barrier));
    assert!(slot.reserve().is_none());
    assert!(!slot.begin_entry(first, &(7, 1)));
    assert!(!slot.begin_entry(second, &(7, 1)));
    assert!(slot.observe_connection(NpcGoldBuyConnectionEpoch { run: 1, connection: 2 }));
    assert_eq!(slot.history().back().unwrap().phase, NpcGoldBuyAttemptPhase::Unknown);
    assert_eq!(slot.authority_revision(), 4);
    slot.set_available(true);
    assert!(slot.reserve().is_none(), "new connection needs a fresh complete model");
    assert!(slot.observe_authority("A"));
    slot.set_available(true);
    let third = bound(&mut slot, (7, 1));
    assert!(third.value() > second.value());
    assert!(slot.begin_entry(third, &(7, 1)));
    assert_eq!(slot.flight().unwrap().authority_revision, 5);
}

#[test]
fn close_reopen_cancels_queued_and_bound_only() {
    for bind_ticket in [false, true] {
        let mut slot = opened("A");
        let token = slot.reserve().unwrap();
        if bind_ticket { assert!(slot.bind(token, (7, 1))); }
        slot.set_available(false);
        assert!(slot.flight().is_none());
        assert_eq!(slot.history().back().unwrap().phase, NpcGoldBuyAttemptPhase::DefinitelyUnsent);
        assert!(!slot.begin_entry(token, &(7, 1)));
        assert!(slot.observe_authority("A"));
        assert_eq!(slot.authority_revision(), 1);
        slot.set_available(true);
        assert!(slot.reserve().unwrap().value() > token.value());
    }
}

#[test]
fn close_reopen_same_authority_never_releases_an_entered_barrier() {
    for outcome in [None, Some(NpcGoldBuyAttemptOutcome::Flushed), Some(NpcGoldBuyAttemptOutcome::Unknown)] {
        let mut slot = opened("A");
        let token = bound(&mut slot, (7, 1));
        assert!(slot.begin_entry(token, &(7, 1)));
        if let Some(outcome) = outcome { assert!(slot.apply_receipt(token, &(7, 1), outcome)); }
        let barrier = slot.flight().unwrap().clone();
        slot.set_available(false);
        assert!(slot.observe_authority("A"));
        slot.set_available(true);
        assert_eq!(slot.flight(), Some(&barrier));
        assert!(!slot.can_reserve());
        assert!(slot.reserve().is_none());
    }
}

#[test]
fn entered_flushed_and_unknown_cannot_be_downgraded_to_unsent() {
    for outcome in [None, Some(NpcGoldBuyAttemptOutcome::Flushed), Some(NpcGoldBuyAttemptOutcome::Unknown)] {
        let mut slot = opened("A");
        let token = bound(&mut slot, (7, 1));
        assert!(slot.begin_entry(token, &(7, 1)));
        if let Some(outcome) = outcome { assert!(slot.apply_receipt(token, &(7, 1), outcome)); }
        let barrier = slot.flight().unwrap().clone();
        assert!(!slot.apply_receipt(token, &(7, 1), NpcGoldBuyAttemptOutcome::DefinitelyUnsent));
        assert!(!slot.reject_unpublished(token));
        assert_eq!(slot.flight(), Some(&barrier));
    }
}

#[test]
fn exact_receipt_phase_transitions_and_duplicates_do_not_fabricate_ack() {
    for outcome in [NpcGoldBuyAttemptOutcome::Flushed, NpcGoldBuyAttemptOutcome::Unknown] {
        let mut slot = opened("A");
        let token = bound(&mut slot, (7, 1));
        assert!(slot.begin_entry(token, &(7, 1)));
        assert!(!slot.apply_receipt(token, &(7, 2), outcome));
        assert!(slot.apply_receipt(token, &(7, 1), outcome));
        let terminal = slot.flight().unwrap().clone();
        assert!(!slot.apply_receipt(token, &(7, 1), outcome));
        assert!(!slot.apply_receipt(token, &(7, 1), NpcGoldBuyAttemptOutcome::Flushed));
        assert!(!slot.apply_receipt(token, &(7, 1), NpcGoldBuyAttemptOutcome::Unknown));
        assert_eq!(slot.flight(), Some(&terminal));
        assert!(slot.reserve().is_none());
    }
}

#[test]
fn changed_full_authority_preserves_flushed_until_real_connection_change() {
    let mut slot = opened("A");
    let old = bound(&mut slot, (7, 1));
    assert!(slot.begin_entry(old, &(7, 1)));
    assert!(slot.apply_receipt(old, &(7, 1), NpcGoldBuyAttemptOutcome::Flushed));
    let barrier = slot.flight().unwrap().clone();
    assert!(slot.observe_authority("B"));
    assert_eq!(slot.flight(), Some(&barrier));
    assert_eq!(slot.flight().unwrap().phase, NpcGoldBuyAttemptPhase::Flushed);
    assert!(slot.reserve().is_none());
    assert!(slot.observe_connection(NpcGoldBuyConnectionEpoch { run: 1, connection: 2 }));
    assert!(slot.flight().is_none());
    assert_eq!(slot.history().back().unwrap().phase, NpcGoldBuyAttemptPhase::Unknown);
    assert!(!slot.can_reserve());
    assert!(slot.observe_authority("B")); slot.set_available(true);
    let next = bound(&mut slot, (7, 2));
    assert!(!slot.apply_receipt(old, &(7, 1), NpcGoldBuyAttemptOutcome::DefinitelyUnsent));
    assert!(!slot.apply_receipt(old, &(7, 1), NpcGoldBuyAttemptOutcome::Flushed));
    assert_eq!(slot.flight().unwrap().token, next);
}

#[test]
fn identical_full_authority_observation_does_not_advance_or_clear_barrier() {
    let authority = "owner:1;service:1;catalog:[full row];inventory:[full rows];gold:99";
    let mut slot = opened(authority);
    let token = bound(&mut slot, (7, 1));
    assert!(slot.begin_entry(token, &(7, 1)));
    for _ in 0..10 { assert!(slot.observe_authority(authority)); }
    assert_eq!(slot.authority_revision(), 1);
    assert_eq!(slot.flight().unwrap().token, token);
    assert!(slot.reserve().is_none());
}

#[test]
fn malformed_authority_needs_a_valid_observation_and_cannot_wash_entered_barrier() {
    let mut slot = opened("A");
    let queued = slot.reserve().unwrap();
    assert!(!slot.observe_authority(""));
    slot.set_available(true);
    assert!(slot.reserve().is_none(), "availability cannot manufacture a valid authority observation");
    assert!(slot.observe_authority("A"));
    let token = bound(&mut slot, (7, 1));
    assert!(token.value() > queued.value());
    assert!(slot.begin_entry(token, &(7, 1)));
    assert!(!slot.observe_authority(&"x".repeat(NPC_GOLD_BUY_AUTHORITY_MAX_BYTES + 1)));
    assert!(slot.observe_authority("A"));
    slot.set_available(true);
    assert_eq!(slot.flight().unwrap().token, token);
    assert_eq!(slot.authority_revision(), 1);
    assert!(slot.reserve().is_none());
}

#[test]
fn authority_bound_is_utf8_bytes_and_exact_limit_is_accepted() {
    let mut slot = opened("A");
    assert!(!slot.observe_authority(&"界".repeat(NPC_GOLD_BUY_AUTHORITY_MAX_BYTES / 3 + 1)));
    assert!(slot.observe_authority(&"x".repeat(NPC_GOLD_BUY_AUTHORITY_MAX_BYTES)));
    assert_eq!(slot.authority_revision(), 2);
}

#[test]
fn history_is_bounded_to_last_32_and_evicted_receipts_cannot_touch_new_flight() {
    let mut slot = opened("A");
    let mut first = None;
    for i in 1..=40 {
        let token = bound(&mut slot, (7, i));
        if first.is_none() { first = Some(token); }
        assert!(slot.apply_receipt(token, &(7, i), NpcGoldBuyAttemptOutcome::DefinitelyUnsent));
    }
    assert_eq!(slot.history().len(), 32);
    assert_eq!(slot.history().front().unwrap().token.value(), 9);
    assert_eq!(slot.history().back().unwrap().token.value(), 40);
    let next = bound(&mut slot, (7, 1));
    assert!(!slot.apply_receipt(first.unwrap(), &(7, 1), NpcGoldBuyAttemptOutcome::DefinitelyUnsent));
    assert_eq!(slot.flight().unwrap().token, next);
}

#[test]
fn token_allocator_checked_exhaustion_never_wraps_or_resets() {
    let mut slot = opened("A");
    slot.last_token = u64::MAX - 1;
    let last = bound(&mut slot, (7, 1));
    assert_eq!(last.value(), u64::MAX);
    assert!(slot.apply_receipt(last, &(7, 1), NpcGoldBuyAttemptOutcome::DefinitelyUnsent));
    assert!(!slot.can_reserve());
    assert!(slot.reserve().is_none());
    slot.set_available(true);
    assert!(!slot.observe_authority("B"));
    assert_eq!(slot.authority_revision(), 1);
    assert!(slot.reserve().is_none());
    assert_eq!(slot.last_token, u64::MAX);
}

#[test]
fn revision_exhaustion_cancels_preentry_but_preserves_entered_barrier() {
    for entered in [false, true] {
        let mut slot = opened("A");
        slot.authority_revision = u64::MAX;
        let token = bound(&mut slot, (7, 1));
        if entered { assert!(slot.begin_entry(token, &(7, 1))); }
        assert!(!slot.observe_authority("B"));
        assert_eq!(slot.authority_revision(), u64::MAX);
        slot.set_available(true);
        assert!(!slot.observe_authority("A"));
        assert!(slot.reserve().is_none());
        if entered {
            assert_eq!(slot.flight().unwrap().phase, NpcGoldBuyAttemptPhase::Entered);
            assert!(slot.apply_receipt(token, &(7, 1), NpcGoldBuyAttemptOutcome::Unknown));
        } else {
            assert!(slot.flight().is_none());
            assert_eq!(slot.history().back().unwrap().phase, NpcGoldBuyAttemptPhase::DefinitelyUnsent);
        }
    }
}

#[test]
fn connection_credential_is_required_and_same_epoch_is_idempotent() {
    let mut slot = NpcGoldBuyAttemptSlot::<Ticket>::new();
    assert!(slot.observe_authority("A")); slot.set_available(true);
    assert!(!slot.can_reserve()); assert!(slot.reserve().is_none());
    assert!(!slot.observe_connection(NpcGoldBuyConnectionEpoch { run: 0, connection: 1 }));
    assert!(!slot.observe_connection(NpcGoldBuyConnectionEpoch { run: 1, connection: 0 }));
    let epoch = NpcGoldBuyConnectionEpoch { run: 1, connection: 1 };
    assert!(slot.observe_connection(epoch));
    assert!(!slot.can_reserve()); assert!(slot.reserve().is_none());
    assert!(slot.observe_authority("A")); slot.set_available(true);
    let token = bound(&mut slot, (7, 1)); let before=slot.flight().unwrap().clone();
    let revision=slot.authority_revision();
    assert!(slot.observe_connection(epoch));
    assert_eq!(slot.flight(),Some(&before)); assert_eq!(slot.authority_revision(),revision);
    assert!(slot.begin_entry(token,&(7,1)));
}
#[test]
fn same_connection_all_unresolved_phases_survive_full_model_owner_and_ui_changes() {
    for outcome in [None,Some(NpcGoldBuyAttemptOutcome::Flushed),Some(NpcGoldBuyAttemptOutcome::Unknown)] {
        let mut slot=opened("A");let token=bound(&mut slot,(7,1));assert!(slot.begin_entry(token,&(7,1)));
        if let Some(outcome)=outcome{assert!(slot.apply_receipt(token,&(7,1),outcome));}
        let barrier=slot.flight().unwrap().clone();
        for authority in ["catalog B","wallet B","other character","A"] {
            slot.set_available(false);assert!(slot.observe_authority(authority));slot.set_available(true);
            assert!(slot.observe_connection(NpcGoldBuyConnectionEpoch{run:1,connection:1}));
            assert_eq!(slot.flight(),Some(&barrier));assert!(!slot.can_reserve());assert!(slot.reserve().is_none());
        }
        assert!(slot.history().is_empty());
    }
}
#[test]
fn initial_epoch_cannot_attribute_or_retire_legacy_unresolved_work() {
    for phase in [NpcGoldBuyAttemptPhase::Entered,NpcGoldBuyAttemptPhase::Flushed,NpcGoldBuyAttemptPhase::Unknown] {
        let mut slot=NpcGoldBuyAttemptSlot::<Ticket>::new();
        slot.flight=Some(NpcGoldBuyAttempt{token:NpcGoldBuyAttemptToken(9),authority_revision:4,ticket:Some((7,1)),phase});
        slot.authority_revision=4;slot.last_token=9;let before=slot.flight().unwrap().clone();
        assert!(!slot.observe_connection(NpcGoldBuyConnectionEpoch{run:1,connection:1}));
        assert!(!slot.observe_connection(NpcGoldBuyConnectionEpoch{run:2,connection:1}));
        assert_eq!(slot.connection_epoch(),None);assert_eq!(slot.flight(),Some(&before));
        assert_eq!(slot.authority_revision(),4);assert_eq!(slot.last_token,9);assert!(slot.history().is_empty());
        slot.set_available(true);assert!(slot.reserve().is_none());
    }
}
#[test]
fn old_pair_old_receipt_and_pair_overflow_cannot_clear_current_barrier() {
    let mut slot=opened("A");let old=bound(&mut slot,(7,1));assert!(slot.begin_entry(old,&(7,1)));
    assert!(slot.observe_connection(NpcGoldBuyConnectionEpoch{run:2,connection:1}));
    assert!(slot.observe_authority("A"));slot.set_available(true);
    let fresh=bound(&mut slot,(7,1));let before=slot.flight().unwrap().clone();
    for epoch in [NpcGoldBuyConnectionEpoch{run:1,connection:u64::MAX},NpcGoldBuyConnectionEpoch{run:2,connection:0}] {
        assert!(!slot.observe_connection(epoch));assert_eq!(slot.flight(),Some(&before));
    }
    for outcome in [NpcGoldBuyAttemptOutcome::DefinitelyUnsent,NpcGoldBuyAttemptOutcome::Flushed,NpcGoldBuyAttemptOutcome::Unknown] {
        assert!(!slot.apply_receipt(old,&(7,1),outcome));assert_eq!(slot.flight(),Some(&before));
    }
    assert!(slot.begin_entry(fresh,&(7,1)));let barrier=slot.flight().unwrap().clone();
    slot.authority_revision=u64::MAX;
    assert!(!slot.observe_connection(NpcGoldBuyConnectionEpoch{run:2,connection:2}));
    assert_eq!(slot.connection_epoch(),Some(NpcGoldBuyConnectionEpoch{run:2,connection:1}));
    assert_eq!(slot.flight(),Some(&barrier));assert!(slot.reserve().is_none());
    assert!(!slot.observe_connection(NpcGoldBuyConnectionEpoch{run:3,connection:1}));
    assert_eq!(slot.last_token,fresh.value());
}
