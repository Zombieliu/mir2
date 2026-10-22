//! Pure custody fixtures only. Reader connections and command stamps are
//! synthetic test facts, not physical socket or Runtime Applied acceptance.
use super::*;
use mir2_client_bevy::inventory::ItemModel;
use serde_json::json;

fn id() -> String {
    "ab".repeat(32)
}
fn actor() -> MaterialActor {
    MaterialActor::new("material-fixture".into(), "material-player".into(), 1).unwrap()
}
fn owners() -> (NativeCommandStamp, NativeCommandStamp) {
    let (sender, _) = crate::gateway::command_channel(8);
    let fence = sender.ownership_fence().unwrap();
    (fence.test_world_ready(3, 0), fence.test_reconnect(3, 0))
}
fn connection_for(owner: NativeCommandStamp) -> MaterialConnection {
    MaterialConnection::for_test(owner, actor(), 1).unwrap()
}
fn inventory() -> InventoryModel {
    InventoryModel {
        capacity: 46,
        gold: 123,
        items: vec![ItemModel {
            unique_id: Some(u64::MAX),
            key: "material-fixture-root".into(),
            name: "Synthetic material".into(),
            quantity: 10,
            container: 0,
            slot: 8,
            durability_current: Some(40),
            durability_max: Some(50),
            description: "complete public item preimage".into(),
            ..Default::default()
        }],
        ..Default::default()
    }
}
fn quote() -> wire::Quote {
    wire::Quote {
        quote_id: id(),
        unique_id: U64::new(u64::MAX),
        item_index: 1001,
        input_count: U64::new(10),
        quantity: U64::new(3),
        unit_gold: U64::new(101),
        gold_equivalent: U64::new(303),
        fee_unbound_gold: U64::new(7),
        license_atoms_debit: U64::new(303),
        valuation_version: "fixture-prices-v1".into(),
        created_at_ms: 0,
        expires_at_ms: 30_000,
    }
}
fn receipt() -> wire::Receipt {
    wire::Receipt {
        quote_id: id(),
        input_unique_id: U64::new(u64::MAX),
        output_unique_id: U64::new(u64::MAX - 1),
        input_count: U64::new(10),
        quantity: U64::new(3),
        gold_equivalent: U64::new(303),
        fee_unbound_gold: U64::new(7),
        license_atoms_debit: U64::new(303),
        valuation_version: "fixture-prices-v1".into(),
        operation_key: "cd".repeat(32),
        request_digest: format!("sha256:{}", "ef".repeat(32)),
        business_day: 20_000,
        committed_at_ms: 2,
    }
}
fn raw(dispatch: &MaterialDispatch, reply: wire::Reply, snapshot: Option<Value>) -> String {
    wire::encode_server_frame(&wire::ServerFrame::new(&dispatch.request, reply, snapshot).unwrap())
        .unwrap()
}
fn committed_raw(dispatch: &MaterialDispatch, snapshot: Option<Value>) -> String {
    raw(
        dispatch,
        wire::Reply::Committed {
            receipt: receipt(),
            replayed: true,
            reload_required: false,
        },
        snapshot,
    )
}
struct Fixture {
    client: NativeHuntingMaterialClient,
    connection: MaterialConnection,
    tracker: MaterialSourceTracker,
    inventory: InventoryModel,
    source: MaterialSource,
}
fn ready() -> Fixture {
    let connection = connection_for(owners().0);
    let inventory = inventory();
    let tracker = MaterialSourceTracker::new();
    let candidate = tracker.stage_inventory(&connection, &inventory).unwrap();
    tracker.publish_inventory(candidate).unwrap();
    let source = tracker
        .source_for_actual(&connection, &inventory, u64::MAX, 3)
        .unwrap();
    let mut client = NativeHuntingMaterialClient::new();
    client.bind(connection.clone()).unwrap();
    Fixture {
        client,
        connection,
        tracker,
        inventory,
        source,
    }
}
fn quoted() -> Fixture {
    let mut f = ready();
    let dispatch = f.client.prepare_quote(&f.source, 0).unwrap();
    f.client.enter(&dispatch, 1).unwrap();
    f.client
        .receive(
            f.connection.owner(),
            &raw(&dispatch, wire::Reply::Quoted { quote: quote() }, None),
        )
        .unwrap();
    f
}
fn confirming() -> (Fixture, MaterialDispatch) {
    let mut f = quoted();
    let dispatch = f
        .client
        .prepare_confirm(f.connection.owner(), &id(), &f.source, 2)
        .unwrap();
    f.client.enter(&dispatch, 3).unwrap();
    (f, dispatch)
}

fn rebind_same_actor(f: &mut Fixture) {
    f.client.withdraw();
    f.connection = connection_for(owners().1);
    f.client.bind(f.connection.clone()).unwrap();
}

#[test]
fn material_private_quote_tombstone_recovers_all_preentry_confirm_races_as_query_only() {
    // 0: UI Confirm never reached this host. 1: prepared but not entered.
    // 2: final send was definitely unsent. None may replay the old Confirm.
    for stage in 0..3 {
        let mut f = quoted();
        if stage > 0 {
            let confirm = f
                .client
                .prepare_confirm(f.connection.owner(), &id(), &f.source, 2)
                .unwrap();
            if stage == 2 {
                assert!(f.client.definitely_unsent(confirm.request.request_id.get()));
            }
        }
        rebind_same_actor(&mut f);
        assert!(!f.client.model().pending);
        assert!(f.client.model().quote_id.is_none());
        let query = f
            .client
            .prepare_query(f.connection.owner(), &id(), 4)
            .unwrap();
        assert!(matches!(&query.request.action,wire::Action::Query{quote_id} if quote_id==&id()));
        assert!(f.client.model().pending);
        assert!(f.client.model().receipt.is_none());
        assert!(f
            .client
            .prepare_confirm(f.connection.owner(), &id(), &f.source, 4)
            .is_err());
        f.client.enter(&query, 5).unwrap();
        f.client
            .receive(
                f.connection.owner(),
                &raw(
                    &query,
                    wire::Reply::Rejected {
                        quote_id: Some(id()),
                        code: wire::Rejection::NotSubmitted,
                    },
                    None,
                ),
            )
            .unwrap();
        assert!(!f.client.model().pending);
        assert_eq!(f.client.model().phase, MaterialPhase::Rejected);
        assert!(f
            .client
            .prepare_confirm(f.connection.owner(), &id(), &f.source, 6)
            .is_err());
        let tracker = MaterialSourceTracker::new();
        let staged = tracker
            .stage_inventory(&f.connection, &f.inventory)
            .unwrap();
        tracker.publish_inventory(staged).unwrap();
        let current = tracker
            .source_for_actual(&f.connection, &f.inventory, u64::MAX, 3)
            .unwrap();
        let fresh = f.client.prepare_quote(&current, 7).unwrap();
        assert!(
            matches!(fresh.request.action, wire::Action::Quote { .. }),
            "NotSubmitted permits an explicit fresh Quote only"
        );
    }
}

#[test]
fn material_nonpending_close_keeps_only_private_original_query_custody() {
    let mut f = quoted();
    assert!(f.client.close_view(f.connection.owner()));
    assert!(!f.client.model().pending);
    assert!(f.client.model().quote.is_none());
    assert!(f
        .client
        .prepare_confirm(f.connection.owner(), &id(), &f.source, 2)
        .is_err());
    let query = f
        .client
        .prepare_query(f.connection.owner(), &id(), 3)
        .unwrap();
    assert!(f.client.model().pending);
    assert_eq!(f.client.model().quote, Some(quote()));
    assert!(matches!(query.request.action, wire::Action::Query { .. }));
}

#[test]
fn material_private_tombstone_rebind_recovers_lost_not_submitted_delivery() {
    let mut f = quoted();
    let query = f
        .client
        .prepare_query(f.connection.owner(), &id(), 2)
        .unwrap();
    f.client.enter(&query, 3).unwrap();
    f.client
        .receive(
            f.connection.owner(),
            &raw(
                &query,
                wire::Reply::Rejected {
                    quote_id: Some(id()),
                    code: wire::Rejection::NotSubmitted,
                },
                None,
            ),
        )
        .unwrap();
    // Its terminal view could be pruned before UI delivery.
    rebind_same_actor(&mut f);
    assert!(f.client.model().quote_id.is_none());
    let query = f
        .client
        .prepare_query(f.connection.owner(), &id(), 4)
        .unwrap();
    f.client.enter(&query, 5).unwrap();
    f.client
        .receive(
            f.connection.owner(),
            &raw(
                &query,
                wire::Reply::Rejected {
                    quote_id: Some(id()),
                    code: wire::Rejection::NotSubmitted,
                },
                None,
            ),
        )
        .unwrap();
    assert!(!f.client.model().pending);
    assert_eq!(f.client.model().phase, MaterialPhase::Rejected);
    assert!(f.client.model().receipt.is_none());
}

#[test]
fn material_private_tombstone_rejects_foreign_id_actor_pending_and_outstanding_control() {
    let mut f = quoted();
    let prior = f.client.model();
    assert!(f
        .client
        .prepare_query(f.connection.owner(), &"cd".repeat(32), 2)
        .is_err());
    assert_eq!(f.client.model(), prior);
    let actor_a = f.connection.clone();
    let actor_b = MaterialActor::new("material-fixture".into(), "other-player".into(), 1).unwrap();
    let foreign = MaterialConnection::for_test(owners().1, actor_b, 1).unwrap();
    f.client.bind(foreign.clone()).unwrap();
    assert!(f.client.prepare_query(foreign.owner(), &id(), 3).is_err());
    f.client.bind(actor_a).unwrap();
    let quote_control = f.client.prepare_quote(&f.source, 4).unwrap();
    assert!(f
        .client
        .prepare_query(f.connection.owner(), &id(), 4)
        .is_err());
    f.client.enter(&quote_control, 5).unwrap();
    let mut newer = quote();
    newer.quote_id = "cd".repeat(32);
    f.client
        .receive(
            f.connection.owner(),
            &raw(
                &quote_control,
                wire::Reply::Quoted {
                    quote: newer.clone(),
                },
                None,
            ),
        )
        .unwrap();
    let confirm = f
        .client
        .prepare_confirm(f.connection.owner(), &newer.quote_id, &f.source, 6)
        .unwrap();
    f.client.enter(&confirm, 7).unwrap();
    let pending = f.client.model();
    assert!(f
        .client
        .prepare_query(f.connection.owner(), &id(), 8)
        .is_err());
    assert!(f
        .client
        .prepare_query(f.connection.owner(), &newer.quote_id, 8)
        .is_err());
    assert_eq!(
        f.client.model(),
        pending,
        "another Pending or control cannot be replaced"
    );
}

#[test]
fn material_source_requires_exact_published_candidate_and_same_value_refresh_preserves_revision() {
    let f = ready();
    assert_eq!(f.source.owner(), f.connection.owner());
    assert_eq!(
        (
            f.source.unique_id(),
            f.source.input_count(),
            f.source.quantity()
        ),
        (u64::MAX, 10, 3)
    );
    let old_candidate = f
        .tracker
        .stage_inventory(&f.connection, &f.inventory)
        .unwrap();
    assert!(f
        .tracker
        .source_for_actual(&f.connection, &f.inventory, u64::MAX, 3)
        .is_err());
    let latest = f
        .tracker
        .stage_inventory(&f.connection, &f.inventory)
        .unwrap();
    assert!(f.tracker.publish_inventory(old_candidate).is_err());
    assert!(f
        .tracker
        .source_for_actual(&f.connection, &f.inventory, u64::MAX, 3)
        .is_err());
    f.tracker.publish_inventory(latest).unwrap();
    let mut client = NativeHuntingMaterialClient::new();
    client.bind(f.connection.clone()).unwrap();
    let dispatch = client.prepare_quote(&f.source, 0).unwrap();
    assert!(client.definitely_unsent(dispatch.request.request_id.get()));
    // A real mutation event revokes even the same public bytes after recovery.
    f.tracker.invalidate();
    let restored = f
        .tracker
        .stage_inventory(&f.connection, &f.inventory)
        .unwrap();
    f.tracker.publish_inventory(restored).unwrap();
    assert!(client.prepare_quote(&f.source, 1).is_err());
    let current = f
        .tracker
        .source_for_actual(&f.connection, &f.inventory, u64::MAX, 3)
        .unwrap();
    client.prepare_quote(&current, 1).unwrap();
}

#[test]
fn material_sources_keep_tracker_and_reader_namespaces_and_full_inventory_preimages_separate() {
    let mut f = quoted();
    let other_tracker = MaterialSourceTracker::new();
    let candidate = other_tracker
        .stage_inventory(&f.connection, &f.inventory)
        .unwrap();
    assert!(f.tracker.publish_inventory(candidate).is_err());
    let candidate = other_tracker
        .stage_inventory(&f.connection, &f.inventory)
        .unwrap();
    other_tracker.publish_inventory(candidate).unwrap();
    let foreign = other_tracker
        .source_for_actual(&f.connection, &f.inventory, u64::MAX, 3)
        .unwrap();
    assert!(f
        .client
        .prepare_confirm(f.connection.owner(), &id(), &foreign, 2)
        .is_err());
    // Even an equal stamp/actor recreated by another reader is a new namespace.
    let other_connection = connection_for(f.connection.owner());
    assert!(f
        .tracker
        .source_for_actual(&other_connection, &f.inventory, u64::MAX, 3)
        .is_err());
    let mut changed = f.inventory.clone();
    for field in 0..4 {
        match field {
            0 => changed.items[0].description.push('!'),
            1 => changed.items[0].durability_current = Some(39),
            2 => changed.gold += 1,
            _ => changed.capacity = 54,
        }
        assert!(f
            .tracker
            .source_for_actual(&f.connection, &changed, u64::MAX, 3)
            .is_err());
        changed = f.inventory.clone();
    }
    f.client
        .prepare_confirm(f.connection.owner(), &id(), &f.source, 2)
        .unwrap();
}

#[test]
fn material_source_rejects_nonbag_zero_or_ambiguous_identity_quantity_and_noncanonical_capacity() {
    let f = ready();
    for (uid, quantity) in [
        (0, 1),
        (u64::MAX, 0),
        (u64::MAX, 11),
        (u64::MAX, u64::from(u32::MAX) + 1),
    ] {
        assert!(f
            .tracker
            .source_for_actual(&f.connection, &f.inventory, uid, quantity)
            .is_err());
    }
    for mutation in 0..8 {
        let mut image = f.inventory.clone();
        match mutation {
            0 => image.capacity = 47,
            1 => image.items[0].quantity = 0,
            2 => image.items[0].unique_id = Some(0),
            3 => image.items[0].unique_id = None,
            4 => {
                image.items[0].container = 1;
                image.items[0].slot = 0;
            }
            5 => image.items[0].slot = 40,
            6 => {
                let mut duplicate = image.items[0].clone();
                duplicate.slot = 9;
                image.items.push(duplicate);
            }
            _ => {
                let mut duplicate = image.items[0].clone();
                duplicate.unique_id = Some(123);
                image.items.push(duplicate);
            }
        }
        let tracker = MaterialSourceTracker::new();
        if let Ok(candidate) = tracker.stage_inventory(&f.connection, &image) {
            tracker.publish_inventory(candidate).unwrap();
            assert!(
                tracker
                    .source_for_actual(
                        &f.connection,
                        &image,
                        if mutation == 2 { 0 } else { u64::MAX },
                        3
                    )
                    .is_err(),
                "mutation {mutation}"
            );
        }
    }
    let mut invalid = f.inventory.clone();
    invalid.capacity = 47;
    assert!(f.tracker.stage_inventory(&f.connection, &invalid).is_err());
    let mut client = NativeHuntingMaterialClient::new();
    client.bind(f.connection.clone()).unwrap();
    assert!(
        client.prepare_quote(&f.source, 0).is_err(),
        "bad new inventory revokes prior readiness"
    );
}

#[test]
fn material_source_counters_exhaust_without_wrapping_or_reviving_old_capabilities() {
    for publication in [false, true] {
        let f = ready();
        {
            let mut state = f.tracker.state.lock().unwrap();
            if publication {
                state.publication = u64::MAX;
            } else {
                state.revision = u64::MAX;
            }
        }
        if publication {
            assert!(f
                .tracker
                .stage_inventory(&f.connection, &f.inventory)
                .is_err());
        } else {
            f.tracker.invalidate();
        }
        assert!(f
            .tracker
            .stage_inventory(&f.connection, &f.inventory)
            .is_err());
        assert!(f
            .tracker
            .source_for_actual(&f.connection, &f.inventory, u64::MAX, 3)
            .is_err());
        let mut client = NativeHuntingMaterialClient::new();
        client.bind(f.connection.clone()).unwrap();
        assert!(client.prepare_quote(&f.source, 0).is_err());
    }
}

#[test]
fn material_connection_uses_independent_protocol_and_rebind_retains_only_original_pending_query() {
    let (sender, _) = crate::gateway::command_channel(8);
    let fence = sender.ownership_fence().unwrap();
    let current = fence.test_world_ready(3, 0);
    let verified = MaterialConnection::from_verified_reader(current, actor(), 1, &fence).unwrap();
    assert_eq!(verified.owner(), current);
    assert_eq!(verified.actor().realm(), "material-fixture");
    assert_eq!(verified.actor().account_id(), "material-player");
    assert_eq!(verified.actor().character_index(), 1);
    assert!(MaterialConnection::from_verified_reader(current, actor(), 0, &fence).is_err());
    let next = fence.test_reconnect(3, 0);
    assert!(MaterialConnection::from_verified_reader(current, actor(), 1, &fence).is_err());
    MaterialConnection::from_verified_reader(next, actor(), 1, &fence).unwrap();
    let (first, second) = owners();
    for version in [0, 2, u16::MAX] {
        assert!(MaterialConnection::for_test(first, actor(), version).is_err());
    }
    assert!(MaterialActor::new(" bad".into(), "player".into(), 0).is_err());
    assert!(MaterialActor::new("realm".into(), "".into(), 0).is_err());
    assert!(MaterialActor::new("realm".into(), "player".into(), -1).is_err());
    let (mut f, original) = confirming();
    let late = committed_raw(&original, Some(json!({"turn":"old-reader"})));
    let original_owner = f.connection.owner();
    f.client.withdraw();
    let resumed = MaterialConnection::for_test(second, actor(), 1).unwrap();
    f.client.bind(resumed.clone()).unwrap();
    assert!(f.client.model().pending);
    assert_eq!(f.client.model().quote_id.as_deref(), Some(id().as_str()));
    assert!(f.client.receive(original_owner, &late).is_err());
    assert!(f
        .client
        .prepare_confirm(second, &id(), &f.source, 4)
        .is_err());
    assert!(f.client.prepare_query(original_owner, &id(), 4).is_err());
    let query = f.client.prepare_query(second, &id(), 4).unwrap();
    f.client.enter(&query, 5).unwrap();
    f.client
        .receive(
            second,
            &raw(&query, wire::Reply::Pending { quote_id: id() }, None),
        )
        .unwrap();
    let other = MaterialActor::new("material-fixture".into(), "other-player".into(), 1).unwrap();
    f.client
        .bind(MaterialConnection::for_test(second, other, 1).unwrap())
        .unwrap();
    assert!(!f.client.model().pending && f.client.model().quote_id.is_none());
    assert!(f.client.prepare_query(second, &id(), 6).is_err());
    f.client.bind(resumed).unwrap();
    assert!(f.client.model().pending);
    f.client.prepare_query(second, &id(), 6).unwrap();
}

#[test]
fn material_confirm_enters_once_and_unsent_source_races_or_expiry_cannot_consume_it() {
    let mut f = quoted();
    let before = f.client.model();
    assert!(f
        .client
        .prepare_confirm(f.connection.owner(), &id(), &f.source, 30_000)
        .is_err());
    assert_eq!(f.client.model(), before);
    let first = f
        .client
        .prepare_confirm(f.connection.owner(), &id(), &f.source, 2)
        .unwrap();
    assert!(!f.client.model().pending);
    assert!(f.client.definitely_unsent(first.request.request_id.get()));
    let retry = f
        .client
        .prepare_confirm(f.connection.owner(), &id(), &f.source, 3)
        .unwrap();
    f.client.enter(&retry, 4).unwrap();
    assert!(f.client.model().pending);
    assert!(f.client.enter(&retry, 4).is_err());
    assert!(!f.client.definitely_unsent(retry.request.request_id.get()));
    assert!(f.client.close_view(f.connection.owner()));
    assert!(f.client.model().pending && !f.client.model().visible);
    assert!(f.client.prepare_quote(&f.source, 5).is_err());
    assert!(f
        .client
        .prepare_confirm(f.connection.owner(), &id(), &f.source, 5)
        .is_err());
    assert!(f
        .client
        .prepare_query(f.connection.owner(), &"cd".repeat(32), 5)
        .is_err());
    assert!(
        f.client
            .prepare_query(f.connection.owner(), &id(), 5)
            .is_err(),
        "closing does not cancel an entered control"
    );
    f.client
        .receive(
            f.connection.owner(),
            &raw(&retry, wire::Reply::Pending { quote_id: id() }, None),
        )
        .unwrap();
    f.client
        .prepare_query(f.connection.owner(), &id(), 5)
        .unwrap();
    let mut race = quoted();
    let dispatch = race
        .client
        .prepare_confirm(race.connection.owner(), &id(), &race.source, 2)
        .unwrap();
    race.tracker.invalidate();
    let candidate = race
        .tracker
        .stage_inventory(&race.connection, &race.inventory)
        .unwrap();
    race.tracker.publish_inventory(candidate).unwrap();
    assert!(race.client.enter(&dispatch, 3).is_err());
    assert!(!race.client.model().pending);
    assert!(race
        .client
        .definitely_unsent(dispatch.request.request_id.get()));
    let mut expired = quoted();
    let dispatch = expired
        .client
        .prepare_confirm(expired.connection.owner(), &id(), &expired.source, 29_999)
        .unwrap();
    assert!(expired.client.enter(&dispatch, 30_000).is_err());
    assert!(expired
        .client
        .definitely_unsent(dispatch.request.request_id.get()));
}

#[test]
fn material_raw_rejection_is_atomic_and_does_not_retire_original_quote_control() {
    let mut f = ready();
    let dispatch = f.client.prepare_quote(&f.source, 0).unwrap();
    f.client.enter(&dispatch, 1).unwrap();
    let valid = raw(&dispatch, wire::Reply::Quoted { quote: quote() }, None);
    let before = f.client.model();
    for (field, value) in [
        ("inputCount", json!("11")),
        ("uniqueId", json!("123")),
        ("quantity", json!("2")),
        ("goldEquivalent", json!("302")),
    ] {
        let mut changed: Value = serde_json::from_str(&valid).unwrap();
        changed["reply"]["quote"][field] = value;
        assert!(
            f.client
                .receive(f.connection.owner(), &changed.to_string())
                .is_err(),
            "{field}"
        );
        assert_eq!(f.client.model(), before);
    }
    let mut wrong_request: Value = serde_json::from_str(&valid).unwrap();
    wrong_request["requestId"] = json!("999");
    assert!(f
        .client
        .receive(f.connection.owner(), &wrong_request.to_string())
        .is_err());
    assert!(f.client.receive(owners().1, &valid).is_err());
    let duplicate = valid.replacen(
        "\"inputCount\":\"10\"",
        "\"inputCount\":\"10\",\"inputCo\\u0075nt\":\"10\"",
        1,
    );
    assert_ne!(duplicate, valid);
    assert!(f.client.receive(f.connection.owner(), &duplicate).is_err());
    assert_eq!(f.client.model(), before);
    f.client.receive(f.connection.owner(), &valid).unwrap();
    assert_eq!(f.client.model().phase, MaterialPhase::Quoted);
    assert_eq!(f.client.model().quote, Some(quote()));
}

#[test]
fn material_committed_stays_pending_and_application_candidates_are_query_revocable_not_applied() {
    let (mut f, dispatch) = confirming();
    let snapshot =
        json!({"completeTurn":"fixture-only","inventory":[{"uniqueId":(u64::MAX-1).to_string()}]});
    let received = f
        .client
        .receive(
            f.connection.owner(),
            &committed_raw(&dispatch, Some(snapshot.clone())),
        )
        .unwrap();
    let candidate = received.application_candidate.unwrap();
    assert_eq!(candidate.owner(), f.connection.owner());
    assert_eq!(candidate.request_id(), dispatch.request.request_id.get());
    assert_eq!(candidate.quote_id(), id());
    assert_eq!(candidate.snapshot(), &snapshot);
    assert!(f.client.application_is_current(&candidate));
    let known = f.client.model();
    assert_eq!(known.phase, MaterialPhase::Pending);
    assert!(known.pending && known.receipt == Some(receipt()));
    assert_ne!(known.phase, MaterialPhase::Complete);
    f.client.close_view(f.connection.owner());
    assert!(f.client.model().pending && f.client.application_is_current(&candidate));
    let query = f
        .client
        .prepare_query(f.connection.owner(), &id(), 5)
        .unwrap();
    assert!(!f.client.application_is_current(&candidate));
    f.client.enter(&query, 6).unwrap();
    let replacement = f
        .client
        .receive(
            f.connection.owner(),
            &committed_raw(&query, Some(json!({"completeTurn":"next"}))),
        )
        .unwrap()
        .application_candidate
        .unwrap();
    assert!(f.client.application_is_current(&replacement));
    assert!(!f.client.application_is_current(&candidate));
    assert_eq!(f.client.model().phase, MaterialPhase::Pending);
    f.client.withdraw();
    assert!(!f.client.application_is_current(&replacement));
    f.client.bind(f.connection.clone()).unwrap();
    assert!(!f.client.application_is_current(&replacement));
    assert!(f.client.model().pending && f.client.model().receipt == Some(receipt()));
    assert!(f.client.prepare_quote(&f.source, 7).is_err());
    let query = f
        .client
        .prepare_query(f.connection.owner(), &id(), 7)
        .unwrap();
    f.client.enter(&query, 8).unwrap();
    let received = f
        .client
        .receive(
            f.connection.owner(),
            &raw(
                &query,
                wire::Reply::Committed {
                    receipt: receipt(),
                    replayed: true,
                    reload_required: true,
                },
                None,
            ),
        )
        .unwrap();
    assert!(received.application_candidate.is_none());
    assert!(f.client.model().pending && f.client.model().reload_required);
    assert!(f
        .client
        .prepare_query(f.connection.owner(), &id(), 9)
        .is_err());
    let next = connection_for(owners().1);
    let next_owner = next.owner();
    f.client.bind(next).unwrap();
    assert!(f.client.model().pending && !f.client.model().reload_required);
    f.client.prepare_query(next_owner, &id(), 9).unwrap();
}

#[test]
fn material_financial_or_source_receipt_conflicts_are_atomic_and_keep_original_confirmation() {
    let (mut f, dispatch) = confirming();
    let before = f.client.model();
    for (field, value) in [
        ("inputUniqueId", json!("123")),
        ("inputCount", json!("11")),
        ("quantity", json!("1")),
        ("goldEquivalent", json!("606")),
        ("feeUnboundGold", json!("8")),
        ("valuationVersion", json!("other-price-version")),
    ] {
        let mut changed = receipt();
        let mut data = serde_json::to_value(&changed).unwrap();
        data[field] = value;
        if field == "goldEquivalent" {
            data["licenseAtomsDebit"] = json!("606");
        }
        changed = serde_json::from_value(data).unwrap();
        changed.validate().unwrap();
        let message = raw(
            &dispatch,
            wire::Reply::Committed {
                receipt: changed,
                replayed: false,
                reload_required: false,
            },
            None,
        );
        assert!(
            f.client.receive(f.connection.owner(), &message).is_err(),
            "{field}"
        );
        assert_eq!(f.client.model(), before);
    }
    let received = f
        .client
        .receive(f.connection.owner(), &committed_raw(&dispatch, None))
        .unwrap();
    assert!(received.application_candidate.is_none());
    assert_eq!(f.client.model().receipt, Some(receipt()));
    assert!(f.client.model().pending);
    let query = f
        .client
        .prepare_query(f.connection.owner(), &id(), 4)
        .unwrap();
    f.client.enter(&query, 5).unwrap();
    let known = f.client.model();
    let mut changed = receipt();
    changed.operation_key = "ef".repeat(32);
    assert!(f
        .client
        .receive(
            f.connection.owner(),
            &raw(
                &query,
                wire::Reply::Committed {
                    receipt: changed,
                    replayed: true,
                    reload_required: false
                },
                None
            )
        )
        .is_err());
    assert_eq!(f.client.model(), known);
    f.client
        .receive(f.connection.owner(), &committed_raw(&query, None))
        .unwrap();
    assert_eq!(f.client.model().receipt, Some(receipt()));
    let query = f
        .client
        .prepare_query(f.connection.owner(), &id(), 6)
        .unwrap();
    f.client.enter(&query, 7).unwrap();
    let known = f.client.model();
    assert!(f
        .client
        .receive(
            f.connection.owner(),
            &raw(
                &query,
                wire::Reply::Rejected {
                    quote_id: Some(id()),
                    code: wire::Rejection::NotSubmitted,
                },
                None
            )
        )
        .is_err());
    assert_eq!(
        f.client.model(),
        known,
        "known receipt cannot be withdrawn by NotSubmitted"
    );
    f.client
        .receive(
            f.connection.owner(),
            &raw(&query, wire::Reply::Pending { quote_id: id() }, None),
        )
        .unwrap();
    assert!(f.client.model().pending && f.client.model().receipt == Some(receipt()));
}

#[test]
fn material_unknown_timeout_and_tampered_unsent_dispatch_never_authorize_reconfirmation() {
    let (mut f, dispatch) = confirming();
    f.client
        .receive(
            f.connection.owner(),
            &raw(
                &dispatch,
                wire::Reply::Unknown {
                    quote_id: Some(id()),
                },
                None,
            ),
        )
        .unwrap();
    assert!(f.client.model().pending);
    f.client.close_view(f.connection.owner());
    let query = f
        .client
        .prepare_query(f.connection.owner(), &id(), 4)
        .unwrap();
    f.client.enter(&query, 5).unwrap();
    let late = committed_raw(&query, None);
    assert!(f.client.check_timeout(50_000));
    assert!(f.client.receive(f.connection.owner(), &late).is_err());
    assert!(f.client.model().pending && f.client.model().quote_id == Some(id()));
    assert!(f.client.prepare_quote(&f.source, 50_001).is_err());
    assert!(f
        .client
        .prepare_confirm(f.connection.owner(), &id(), &f.source, 50_001)
        .is_err());
    let retry = f
        .client
        .prepare_query(f.connection.owner(), &id(), 50_001)
        .unwrap();
    assert!(
        matches!(retry.request.action, wire::Action::Query { quote_id: ref actual } if actual == &id())
    );
    assert_ne!(retry.request.request_id, query.request.request_id);
    f.client.enter(&retry, 50_002).unwrap();
    f.client
        .receive(
            f.connection.owner(),
            &raw(
                &retry,
                wire::Reply::Rejected {
                    quote_id: Some(id()),
                    code: wire::Rejection::NotSubmitted,
                },
                None,
            ),
        )
        .unwrap();
    let not_submitted = f.client.model();
    assert!(!not_submitted.pending && not_submitted.receipt.is_none());
    assert_eq!(not_submitted.phase, MaterialPhase::Rejected);
    assert!(
        f.client.control.is_none(),
        "original query never auto-confirms"
    );
    // Only explicit new user intent may prepare a replacement quote after a
    // definitive original no-commit result. No debit, refund or send occurs.
    let fresh = f.client.prepare_quote(&f.source, 50_003).unwrap();
    assert!(matches!(fresh.request.action, wire::Action::Quote { .. }));
    assert!(!f.client.model().pending);
    let mut unsent = quoted();
    let mut prepared = unsent
        .client
        .prepare_confirm(unsent.connection.owner(), &id(), &unsent.source, 2)
        .unwrap();
    prepared.body.push(' ');
    assert!(unsent.client.enter(&prepared, 3).is_err());
    assert!(!unsent.client.model().pending);
    assert!(unsent
        .client
        .definitely_unsent(prepared.request.request_id.get()));
    unsent
        .client
        .prepare_confirm(unsent.connection.owner(), &id(), &unsent.source, 4)
        .unwrap();
    let mut clock = ready();
    let before = clock.client.model();
    assert!(clock.client.prepare_quote(&clock.source, u64::MAX).is_err());
    assert_eq!(clock.client.model(), before);
}
