//! Deterministic clock boundaries use real held UserItem custody. Handoff's
//! source-row/actor binding remains a separate, stricter validation boundary.
use super::*;
use crate::config::ItemContainer;

fn sample(epoch: &str, now_ms: u64) -> RefineClockSample {
    RefineClockSample {
        epoch: epoch.into(),
        now_ms,
    }
}

fn held_item(unique_id: u64, slot: u8, refined: bool) -> ItemState {
    let template = mir2_game_data::crystal_item_by_name("Dagger").unwrap();
    let mut item = super::super::items::embedded_item_state_from_template(
        &template,
        ItemContainer::Bag1,
        slot,
    );
    item.unique_id = unique_id;
    item.durability_current = Some(7);
    item.durability_max = Some(23);
    let mut wire = try_user_item_from_item_state(&item).unwrap();
    if refined {
        wire.refine_added = 1;
        wire.refined_value = 1;
        wire.refine_success_chance = 37;
    }
    try_item_state_from_user_item(item, &wire).unwrap()
}

fn oven(remaining_ms: u64, epoch: Option<&str>, deadline: Option<u64>) -> Stage5RefineState {
    let target = held_item(990_001, 0, true);
    let material = held_item(990_002, 1, false);
    let mut state = Stage5RefineState {
        oven_item_state_json: Some(item_custody::encode(&target).unwrap()),
        current_item: Some(target.key.clone()),
        remaining_ms,
        clock_epoch: epoch.map(str::to_owned),
        collect_deadline_ms: deadline,
        refining: true,
        ready: remaining_ms == 0,
        ..Default::default()
    };
    state.slots.insert(3, material.key.clone());
    state
        .item_states
        .insert(3, item_custody::encode(&material).unwrap());
    validate(&state).unwrap();
    state
}

fn assert_custody_unchanged(before: &Stage5RefineState, after: &Stage5RefineState) {
    let mut expected = before.clone();
    expected.remaining_ms = after.remaining_ms;
    expected.clock_epoch = after.clock_epoch.clone();
    expected.collect_deadline_ms = after.collect_deadline_ms;
    expected.ready = after.ready;
    assert_eq!(
        &expected, after,
        "clock work cannot rewrite phase, items, slots or legacy pending custody"
    );
    let systems = crate::Stage5SystemsState {
        refine: after.clone(),
        ..Default::default()
    };
    assert_eq!(
        item_custody::reserved_ids(&systems).unwrap(),
        [990_001, 990_002].into_iter().collect()
    );
}

#[test]
fn same_epoch_restore_retains_exact_deadline_and_snapshot_ages_deterministically() {
    let before = oven(500, Some("server-a"), Some(1_000));
    let mut state = before.clone();
    restore_timer_at(&mut state, &sample("server-a", 900)).unwrap();
    assert_eq!(state.remaining_ms, 100);
    assert_eq!(state.collect_deadline_ms, Some(1_000));
    assert_eq!(state.clock_epoch.as_deref(), Some("server-a"));
    assert!(!state.ready);
    assert_custody_unchanged(&before, &state);

    let mut repeated = state.clone();
    snapshot_timer_at(&mut state, true, &sample("server-a", 950));
    snapshot_timer_at(&mut repeated, true, &sample("server-a", 950));
    assert_eq!(state, repeated);
    assert_eq!(state.remaining_ms, 50);
    assert_eq!(state.collect_deadline_ms, Some(1_000));
    snapshot_timer_at(&mut state, true, &sample("server-a", 1_001));
    assert_eq!(state.remaining_ms, 0);
    assert!(state.ready);
    assert_eq!(state.collect_deadline_ms, Some(1_000));
    assert_custody_unchanged(&before, &state);
}

#[test]
fn foreign_epoch_restores_saved_duration_and_ignores_foreign_deadline() {
    for foreign_deadline in [0, 1, u64::MAX] {
        let before = oven(500, Some("server-a"), Some(foreign_deadline));
        let mut state = before.clone();
        restore_timer_at(&mut state, &sample("server-b", 2_000)).unwrap();
        assert_eq!(
            state.remaining_ms, 500,
            "foreign process uptime is not elapsed oven time"
        );
        assert_eq!(state.collect_deadline_ms, Some(2_500));
        assert_eq!(state.clock_epoch.as_deref(), Some("server-b"));
        assert!(!state.ready);
        snapshot_timer_at(&mut state, true, &sample("server-b", 2_100));
        assert_eq!(state.remaining_ms, 400);
        assert_custody_unchanged(&before, &state);
    }
    let mut unanchored = oven(500, None, None);
    restore_timer_at(&mut unanchored, &sample("server-b", 2_000)).unwrap();
    assert_eq!(unanchored.collect_deadline_ms, Some(2_500));
}

#[test]
fn expiration_is_exact_at_zero_and_cannot_wrap_after_deadline() {
    for now in [1_000, 1_001, u64::MAX] {
        let before = oven(500, Some("server-a"), Some(1_000));
        let mut state = before.clone();
        restore_timer_at(&mut state, &sample("server-a", now)).unwrap();
        assert_eq!(state.remaining_ms, 0);
        assert_eq!(state.collect_deadline_ms, Some(now));
        assert!(state.ready);
        assert_custody_unchanged(&before, &state);
    }
}

#[test]
fn overflow_rejects_before_mutating_any_clock_or_custody() {
    let before = oven(u64::MAX, Some("old-server"), Some(0));
    let mut state = before.clone();
    assert_eq!(
        restore_timer_at(&mut state, &sample("new-server", 1)).unwrap_err(),
        "refine deadline overflow"
    );
    assert_eq!(state, before);
}

#[test]
fn no_oven_is_a_noop_and_stray_or_half_anchors_fail_without_changes() {
    let empty = Stage5RefineState::default();
    let mut state = empty.clone();
    restore_timer_at(&mut state, &sample("server-a", 1_000)).unwrap();
    snapshot_timer_at(&mut state, true, &sample("server-a", 1_000));
    snapshot_timer_at(&mut state, false, &sample("server-a", 1_000));
    assert_eq!(state, empty);

    for kind in 0..5 {
        let mut invalid = if kind < 3 {
            empty.clone()
        } else {
            oven(500, Some("server-a"), Some(1_000))
        };
        match kind {
            0 => invalid.remaining_ms = 1,
            1 => invalid.clock_epoch = Some("forged-anchor".into()),
            2 => invalid.collect_deadline_ms = Some(1_000),
            3 => invalid.clock_epoch = None,
            _ => invalid.collect_deadline_ms = None,
        }
        let before = invalid.clone();
        assert!(restore_timer_at(&mut invalid, &sample("server-a", 900)).is_err());
        assert_eq!(invalid, before);
    }
}

#[test]
fn phase_key_and_legacy_pending_validation_is_not_bypassed_by_clock_helpers() {
    for kind in 0..6 {
        let mut invalid = oven(500, Some("server-a"), Some(1_000));
        match kind {
            0 => invalid.refining = false,
            1 => invalid.current_item = Some("wrong-held-key".into()),
            2 => invalid.pending_unique_id = 42,
            3 => invalid.pending_stat = 1,
            4 => invalid.pending_chance = 1,
            _ => invalid.current_item = None,
        }
        let before = invalid.clone();
        assert!(restore_timer_at(&mut invalid, &sample("server-a", 900)).is_err());
        assert_eq!(invalid, before);
    }
}

#[test]
fn malformed_or_unrefined_held_items_are_rejected_before_clock_mutation() {
    for (added, value) in [(0, 1), (1, 4)] {
        let mut invalid = oven(500, Some("server-a"), Some(1_000));
        let item = held_item(990_001, 0, true);
        let mut wire = try_user_item_from_item_state(&item).unwrap();
        wire.refine_added = added;
        wire.refined_value = value;
        let invalid_target = try_item_state_from_user_item(item, &wire).unwrap();
        invalid.oven_item_state_json = item_custody::encode(&invalid_target);
        assert!(invalid.oven_item_state_json.is_some());
        let before = invalid.clone();
        assert_eq!(
            restore_timer_at(&mut invalid, &sample("server-a", 900)).unwrap_err(),
            "invalid refine oven target/phase"
        );
        assert_eq!(invalid, before);
    }

    let mut invalid = oven(500, Some("server-a"), Some(1_000));
    invalid.oven_item_state_json = Some("{malformed".into());
    let before = invalid.clone();
    assert!(restore_timer_at(&mut invalid, &sample("server-a", 900)).is_err());
    assert_eq!(invalid, before);
}

#[test]
fn same_epoch_stale_remaining_cannot_clear_the_authoritative_future_deadline() {
    // This tests clock semantics, not acceptance of a forged handoff input.
    // The handoff boundary must reject any changed source remaining/anchor.
    let before = oven(0, Some("server-a"), Some(1_000));
    let mut state = before.clone();
    restore_timer_at(&mut state, &sample("server-a", 900)).unwrap();
    assert_eq!(state.remaining_ms, 100);
    assert!(!state.ready);
    assert_eq!(state.collect_deadline_ms, Some(1_000));
    assert_custody_unchanged(&before, &state);
}

#[test]
fn public_snapshot_removes_only_server_anchors_and_keeps_real_remaining_custody() {
    let before = oven(500, Some("server-a"), Some(1_000));
    let mut state = before.clone();
    snapshot_timer_at(&mut state, false, &sample("server-a", 900));
    assert_eq!(state.remaining_ms, 100);
    assert_eq!(state.clock_epoch, None);
    assert_eq!(state.collect_deadline_ms, None);
    assert!(!state.ready);
    assert_custody_unchanged(&before, &state);
}

#[test]
fn production_wrapper_uses_a_real_single_capture_and_safe_sample_wire_shape() {
    let mut state = oven(500, Some("previous-process"), Some(0));
    let before = capture_clock();
    restore_timer(&mut state).unwrap();
    let after = capture_clock();
    assert_eq!(before.epoch, after.epoch);
    assert!(after.now_ms >= before.now_ms);
    assert_eq!(state.clock_epoch.as_deref(), Some(before.epoch.as_str()));
    assert_eq!(state.remaining_ms, 500);
    let installed_at = state.collect_deadline_ms.unwrap().checked_sub(500).unwrap();
    assert!(installed_at >= before.now_ms && installed_at <= after.now_ms);
    assert_eq!(
        serde_json::from_str::<RefineClockSample>(&serde_json::to_string(&after).unwrap()).unwrap(),
        after
    );
    for json in [
        r#"{"epoch":"server-a","nowMs":1,"extra":0}"#,
        r#"{"epoch":"server-a","nowMs":null}"#,
        r#"{"epoch":"server-a","nowMs":-1}"#,
        r#"{"epoch":"server-a","nowMs":"1"}"#,
    ] {
        assert!(serde_json::from_str::<RefineClockSample>(json).is_err());
    }
}
