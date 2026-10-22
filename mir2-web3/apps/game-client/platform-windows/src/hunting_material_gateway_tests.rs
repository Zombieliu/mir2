//! Host boundary tests use a synthetic physical fence and sink. Real World
//! application is exercised below; these are not socket or GUI acceptance.
use super::*;
use futures_util::Sink;
use mir2_client_wire::U64;
use std::task::{Context, Poll};

#[derive(Default)]
struct SinkState {
    frames: Vec<String>,
    ready_hook: Option<Box<dyn FnOnce() + Send>>,
    start_error: bool,
}
#[derive(Clone, Default)]
struct TestSink(Arc<Mutex<SinkState>>);
impl Sink<Message> for TestSink {
    type Error = &'static str;
    fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let hook = self.0.lock().unwrap().ready_hook.take();
        if let Some(hook) = hook {
            hook();
        }
        Poll::Ready(Ok(()))
    }
    fn start_send(self: Pin<&mut Self>, frame: Message) -> Result<(), Self::Error> {
        let mut s = self.0.lock().unwrap();
        s.frames.push(frame.to_text().unwrap().into());
        if s.start_error {
            Err("synthetic unknown send")
        } else {
            Ok(())
        }
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}
const INPUT: u64 = 9_100_001;
const OUTPUT: u64 = u64::MAX - 1;
fn id() -> String {
    "ab".repeat(32)
}
fn item(uid: u64, slot: u8, count: u16) -> Value {
    let template = mir2_game_data::crystal_item_by_name("(HP)DrugSmall").unwrap();
    let real = mir2_game_data::crystal_real_item_for_player(&template, 8, MirClass::Warrior);
    let tooltip = CrystalItemTooltipSourceModel {
        info: serde_json::from_value(serde_json::to_value(&template).unwrap()).unwrap(),
        real_info: Some(serde_json::from_value(serde_json::to_value(real).unwrap()).unwrap()),
        user_item: Some(CrystalUserItemModel {
            unique_id: uid,
            item_index: template.item_index,
            count,
            ..Default::default()
        }),
        socket_infos: vec![],
        real_socket_infos: vec![],
    };
    json!({"uniqueId":uid,"key":format!("crystal-item-{}",template.item_index),"name":template.name,
        "slot":slot,"container":"bag1","quantity":count,"icon":template.image,
        "description":"Synthetic complete native material fixture","durabilityCurrent":null,"durabilityMax":null,
        "sellValue":0,"grade":"common","addedAttack":0,"addedDefence":0,"tooltipSource":tooltip})
}
fn owner(converted: bool) -> Value {
    let mut owner = crate::npc_purchase_projection::tests::owner();
    owner["mapIndex"] = json!(0);
    owner["inventoryItems"] = if converted {
        json!([item(INPUT, 0, 7), item(OUTPUT, 1, 3)])
    } else {
        json!([item(INPUT, 0, 10)])
    };
    owner
}
fn inventory(converted: bool) -> InventoryModel {
    serde_json::from_str(
        &crate::npc_purchase_projection::project_owner(&owner(converted))
            .unwrap()
            .inventory_json,
    )
    .unwrap()
}
fn quote() -> wire::Quote {
    let now = gateway_unix_ms() as i64;
    wire::Quote {
        quote_id: id(),
        unique_id: U64::new(INPUT),
        item_index: mir2_game_data::crystal_item_by_name("(HP)DrugSmall")
            .unwrap()
            .item_index,
        input_count: U64::new(10),
        quantity: U64::new(3),
        unit_gold: U64::new(125),
        gold_equivalent: U64::new(375),
        fee_unbound_gold: U64::new(8),
        license_atoms_debit: U64::new(375),
        valuation_version: "synthetic-fixed-v1".into(),
        created_at_ms: now,
        expires_at_ms: now + 30_000,
    }
}
fn receipt(q: &wire::Quote) -> wire::Receipt {
    wire::Receipt {
        quote_id: q.quote_id.clone(),
        input_unique_id: q.unique_id,
        output_unique_id: U64::new(OUTPUT),
        input_count: q.input_count,
        quantity: q.quantity,
        gold_equivalent: q.gold_equivalent,
        fee_unbound_gold: q.fee_unbound_gold,
        license_atoms_debit: q.license_atoms_debit,
        valuation_version: q.valuation_version.clone(),
        operation_key: "cd".repeat(32),
        request_digest: format!("sha256:{}", "ef".repeat(32)),
        business_day: 1,
        committed_at_ms: gateway_unix_ms() as i64,
    }
}
fn frame(d: &MaterialDispatch, reply: wire::Reply, snapshot: Option<Value>) -> String {
    wire::encode_server_frame(&wire::ServerFrame::new(&d.request, reply, snapshot).unwrap())
        .unwrap()
}
struct Fixture {
    gateway: NativeHuntingMaterialGateway,
    fence: NativeCommandFence,
    stamp: NativeCommandStamp,
    purchases: npc_purchase_gateway::NativeNpcPurchaseGateway,
    inventory: InventoryModel,
}
fn entry() -> Fixture {
    let (sender, _) = command_channel(8);
    let fence = sender.ownership_fence().unwrap();
    let stamp = fence.test_world_ready(3, 0);
    let mut gateway = NativeHuntingMaterialGateway::new();
    gateway.observe_connection(&json!({"type":"realmInfo","payload":{"schema":"mir2-realm-handshake/1",
        "realmId":"synthetic-material","huntingMaterialOwnerProtocolVersion":1,"npcRepairMode":"legacy"}}).to_string()).unwrap();
    gateway.observe_connection(&json!({"type":"identitySession","token":format!("e30.{}","A".repeat(43)),"session":{
        "accountId":"alice","sessionId":"synthetic-session","expiresAtMs":gateway_unix_ms()+600_000,"revokedAtMs":null}}).to_string()).unwrap();
    let mut purchases = npc_purchase_gateway::NativeNpcPurchaseGateway::new().unwrap();
    gateway
        .tick(
            &GatewaySessionContext {
                account_id: Some("ALICE".into()),
                character_index: Some(7),
                ..Default::default()
            },
            &fence,
            Some(stamp),
            ConnectionPhase::Normal,
            true,
            &mut purchases,
        )
        .unwrap();
    Fixture {
        gateway,
        fence,
        stamp,
        purchases,
        inventory: inventory(false),
    }
}
fn ready() -> Fixture {
    let f = entry();
    let value = serde_json::to_value(&f.inventory).unwrap();
    let ticket = f
        .fence
        .stage_hunting_material_inventory(&value, true)
        .unwrap();
    f.fence.publish_hunting_material_inventory(ticket, f.stamp);
    f
}
fn owned(f: &Fixture, action: HuntingMaterialCommand) -> Box<OwnedGatewayCommand> {
    let GatewayCommand::Owned(owned) = f
        .fence
        .prepare(f.stamp, GatewayCommand::HuntingMaterial(action))
        .unwrap()
    else {
        unreachable!()
    };
    owned
}
fn seed_quote(f: &mut Fixture) {
    let source = f
        .fence
        .hunting_material_source(&f.inventory, INPUT, 3)
        .unwrap();
    let dispatch = f
        .gateway
        .client
        .prepare_quote(&source, gateway_unix_ms())
        .unwrap();
    f.gateway
        .client
        .enter(&dispatch, gateway_unix_ms())
        .unwrap();
    f.gateway
        .receive(
            &frame(&dispatch, wire::Reply::Quoted { quote: quote() }, None),
            &f.fence,
        )
        .unwrap();
}
fn confirm(f: &mut Fixture) -> (Box<OwnedGatewayCommand>, MaterialDispatch) {
    let source = f.fence.hunting_material_source(&f.inventory, INPUT, 3);
    let owned = owned(
        f,
        HuntingMaterialCommand::Confirm {
            quote_id: id(),
            source,
        },
    );
    let dispatch = f.gateway.prepare(&owned, &mut f.purchases).unwrap();
    (owned, dispatch)
}

#[test]
fn native_material_host_marker_preserves_claimed_namespace() {
    for raw in [
        r#"{"type":"huntingMaterialOwner"}"#,
        r#"{"ty\u0070e":"huntingMaterialOwner"}"#,
        r#"{"type":"worldSnapshot","type":"huntingMaterialOwner"}"#,
    ] {
        assert!(claims_owner_frame(raw));
    }
    assert!(!claims_owner_frame(
        r#"{"type":"packet","payload":{"type":"huntingMaterialOwner"}}"#
    ));
}
#[test]
fn native_material_host_first_entry_needs_next_exact_successful_full_producer() {
    let f = entry();
    assert!(f
        .fence
        .hunting_material_source(&f.inventory, INPUT, 3)
        .is_none());
    let value = serde_json::to_value(&f.inventory).unwrap();
    let stale = f
        .fence
        .stage_hunting_material_inventory(&value, true)
        .unwrap();
    assert!(f
        .fence
        .hunting_material_source(&f.inventory, INPUT, 3)
        .is_none());
    let latest = f
        .fence
        .stage_hunting_material_inventory(&value, true)
        .unwrap();
    f.fence.publish_hunting_material_inventory(stale, f.stamp);
    assert!(f
        .fence
        .hunting_material_source(&f.inventory, INPUT, 3)
        .is_none());
    f.fence.publish_hunting_material_inventory(latest, f.stamp);
    assert!(f
        .fence
        .hunting_material_source(&f.inventory, INPUT, 3)
        .is_some());
    assert!(f
        .fence
        .stage_hunting_material_inventory(&value, false)
        .is_none());
    assert!(f
        .fence
        .hunting_material_source(&f.inventory, INPUT, 3)
        .is_none());
}
#[tokio::test]
async fn native_material_host_final_ready_rechecks_source_and_other_lane() {
    for other_lane in [false, true] {
        let mut f = ready();
        seed_quote(&mut f);
        let (owned, dispatch) = confirm(&mut f);
        assert!(f.fence.0.lock().unwrap().hunting_material_busy);
        let fence = f.fence.clone();
        let mut sink = TestSink::default();
        sink.0.lock().unwrap().ready_hook = Some(Box::new(move || {
            let mut state = fence.0.lock().unwrap();
            if other_lane {
                state.hunting_repair_busy = true;
            } else {
                state.hunting_material_source.invalidate();
            }
        }));
        assert!(matches!(
            f.gateway.send(&mut sink, &owned, dispatch).await,
            NativeSinkCommit::DefinitelyUnsent
        ));
        assert!(sink.0.lock().unwrap().frames.is_empty());
        assert!(!f.gateway.client.model().pending);
        assert!(!f.fence.0.lock().unwrap().hunting_material_busy);
    }
}
#[tokio::test]
async fn native_material_host_pending_close_and_unknown_send_only_allow_original_query() {
    let mut f = ready();
    seed_quote(&mut f);
    let (confirm, dispatch) = confirm(&mut f);
    let mut sink = TestSink::default();
    sink.0.lock().unwrap().start_error = true;
    assert!(matches!(
        f.gateway.send(&mut sink, &confirm, dispatch).await,
        NativeSinkCommit::Unknown(_)
    ));
    let close = owned(&f, HuntingMaterialCommand::Close);
    assert!(f.gateway.prepare(&close, &mut f.purchases).is_none());
    f.fence.retire(&close);
    assert!(f.gateway.blocks_purchase());
    assert!(f.fence.0.lock().unwrap().hunting_material_busy);
    assert!(f.gateway.client.check_timeout(gateway_unix_ms() + 10_001));
    let source = f.fence.hunting_material_source(&f.inventory, INPUT, 3);
    let again = owned(
        &f,
        HuntingMaterialCommand::Confirm {
            quote_id: id(),
            source,
        },
    );
    assert!(f.gateway.prepare(&again, &mut f.purchases).is_none());
    f.fence.retire(&again);
    let query = owned(&f, HuntingMaterialCommand::Query { quote_id: id() });
    let dispatch = f.gateway.prepare(&query, &mut f.purchases).unwrap();
    sink.0.lock().unwrap().start_error = false;
    assert!(matches!(
        f.gateway.send(&mut sink, &query, dispatch).await,
        NativeSinkCommit::Flushed
    ));
    let frames = &sink.0.lock().unwrap().frames;
    assert_eq!(frames.len(), 2);
    assert!(matches!(
        wire::parse_client_request(&frames[0]).unwrap().action,
        wire::Action::Confirm { .. }
    ));
    assert!(matches!(
        wire::parse_client_request(&frames[1]).unwrap().action,
        wire::Action::Query { .. }
    ));
}

#[tokio::test]
async fn native_material_host_original_query_preserves_true_other_lane_pending() {
    let mut f = ready();
    seed_quote(&mut f);
    let (confirm, dispatch) = confirm(&mut f);
    let mut sink = TestSink::default();
    assert!(matches!(
        f.gateway.send(&mut sink, &confirm, dispatch).await,
        NativeSinkCommit::Flushed
    ));
    // A new entry withdraws transient lane suspension, but preserves the
    // permanent actor's entered Confirm and original recovery id.
    f.gateway.reset_entry();
    f.gateway
        .tick(
            &GatewaySessionContext {
                account_id: Some("alice".into()),
                character_index: Some(7),
                ..Default::default()
            },
            &f.fence,
            Some(f.stamp),
            ConnectionPhase::Normal,
            true,
            &mut f.purchases,
        )
        .unwrap();
    assert!(f.gateway.client.model().pending);
    assert!(!f.gateway.purchase_suspended);
    f.purchases = npc_purchase_gateway::pending_purchase_fixture();
    assert!(!f.purchases.suspend_for_repair());
    // Repair busy is also irrelevant to this original read-only material query.
    f.fence.0.lock().unwrap().hunting_repair_busy = true;
    let query = owned(&f, HuntingMaterialCommand::Query { quote_id: id() });
    let dispatch = f
        .gateway
        .prepare(&query, &mut f.purchases)
        .expect("original Query bypasses new charge arbitration");
    assert!(!f.gateway.purchase_suspended);
    assert!(matches!(
        f.gateway.send(&mut sink, &query, dispatch).await,
        NativeSinkCommit::Flushed
    ));
    assert!(!f.purchases.suspend_for_repair());
    assert!(f.gateway.client.model().pending);
    assert!(f.fence.0.lock().unwrap().hunting_repair_busy);
    let frames = &sink.0.lock().unwrap().frames;
    assert_eq!(frames.len(), 2);
    assert!(matches!(
        wire::parse_client_request(&frames[0]).unwrap().action,
        wire::Action::Confirm { .. }
    ));
    assert!(matches!(
        wire::parse_client_request(&frames[1]).unwrap().action,
        wire::Action::Query { .. }
    ));
}
#[tokio::test]
async fn native_material_host_complete_requires_same_current_private_world_applied() {
    for retire in [false, true] {
        let mut f = ready();
        seed_quote(&mut f);
        let (owned, dispatch) = confirm(&mut f);
        assert!(matches!(
            f.gateway
                .send(&mut TestSink::default(), &owned, dispatch.clone())
                .await,
            NativeSinkCommit::Flushed
        ));
        let q = f.gateway.client.model().quote.unwrap();
        let mut bundles = Vec::new();
        f.gateway
            .receive_with(
                &frame(
                    &dispatch,
                    wire::Reply::Committed {
                        receipt: receipt(&q),
                        replayed: false,
                        reload_required: false,
                    },
                    Some(owner(true)),
                ),
                &f.fence,
                |bundle| {
                    bundles.push(bundle);
                    true
                },
            )
            .unwrap();
        assert_eq!(bundles.len(), 1);
        assert!(f.gateway.blocks_purchase());
        f.gateway.drain_applied(&f.fence).unwrap();
        assert!(f.gateway.blocks_purchase());
        let mut world = bevy::prelude::World::new();
        assert!(
            mir2_bevy_runtime::hunting_material_economy::apply_native_hunting_material_bundle(
                &mut world,
                bundles.pop().unwrap()
            )
            .is_ok()
        );
        let gate = f.gateway.gate.clone().unwrap();
        let applied = gate.try_recv_applied().unwrap();
        if retire {
            gate.retire();
        }
        assert_eq!(
            f.gateway
                .complete_applied(&f.fence, &gate, &applied)
                .unwrap(),
            !retire
        );
        assert_eq!(f.gateway.client.model().pending, retire);
        assert_eq!(f.gateway.blocks_purchase(), retire);
        if !retire {
            assert_eq!(f.gateway.client.model().phase, MaterialPhase::Complete);
            assert_eq!(world.resource::<InventoryModel>().items.len(), 2);
        }
    }
}
#[test]
fn native_material_host_logout_keeps_identity_but_new_login_and_owner_revoke_retire_gates() {
    let mut f = ready();
    let gate = f.gateway.gate.clone().unwrap();
    f.fence.revoke_owner();
    assert!(gate.epoch().is_none());
    f.gateway
        .before_ordinary_frame(r#"{"type":"packet","packet":"LogOutSuccess"}"#);
    assert!(f.gateway.connection.identity(gateway_unix_ms()).is_some());
    assert!(f.gateway.binding.is_none());
    f.gateway.clear_identity_and_leave();
    assert!(f.gateway.connection.identity(gateway_unix_ms()).is_none());
}

async fn popped_applied() -> (
    Fixture,
    NativeHuntingMaterialGate,
    NativeHuntingMaterialApplied,
    MaterialApplicationCandidate,
    NativeNpcEconomyProjection,
) {
    let mut f = ready();
    seed_quote(&mut f);
    let (owned, dispatch) = confirm(&mut f);
    assert!(matches!(
        f.gateway
            .send(&mut TestSink::default(), &owned, dispatch.clone())
            .await,
        NativeSinkCommit::Flushed
    ));
    let quote = f.gateway.client.model().quote.unwrap();
    let mut bundles = Vec::new();
    f.gateway
        .receive_with(
            &frame(
                &dispatch,
                wire::Reply::Committed {
                    receipt: receipt(&quote),
                    replayed: false,
                    reload_required: false,
                },
                Some(owner(true)),
            ),
            &f.fence,
            |bundle| {
                bundles.push(bundle);
                true
            },
        )
        .unwrap();
    let source = f.gateway.sources.front().unwrap();
    let candidate = source.candidate.clone();
    let projection = source.projection.clone();
    let mut world = bevy::prelude::World::new();
    assert!(
        mir2_bevy_runtime::hunting_material_economy::apply_native_hunting_material_bundle(
            &mut world,
            bundles.pop().unwrap()
        )
        .is_ok()
    );
    let gate = f.gateway.gate.clone().unwrap();
    let applied = gate.try_recv_applied().unwrap();
    (f, gate, applied, candidate, projection)
}

#[tokio::test]
async fn native_material_host_client_api_popped_then_retired_applied_cannot_complete() {
    let (mut f, gate, applied, candidate, _) = popped_applied().await;
    assert!(f.gateway.client.application_is_current(&candidate));
    gate.retire();
    let fence = f.fence.clone();
    let _physical = fence.0.lock().unwrap();
    assert!(!f.gateway.client.applied(&candidate, &gate, &applied));
    assert!(f.gateway.client.model().pending);
    assert_ne!(f.gateway.client.model().phase, MaterialPhase::Complete);
}

#[tokio::test]
async fn native_material_host_client_api_new_prepare_retires_popped_applied_without_query() {
    let (mut f, gate, applied, candidate, projection) = popped_applied().await;
    assert!(f.gateway.client.application_is_current(&candidate));
    // The old Windows candidate is deliberately still current. A newer Runtime
    // preparation alone must independently invalidate the popped proof.
    let _new_bundle = gate
        .prepare(
            applied.request_id() + 1,
            applied.quote(),
            applied.receipt(),
            projection,
        )
        .unwrap();
    let fence = f.fence.clone();
    let _physical = fence.0.lock().unwrap();
    assert!(!f.gateway.client.applied(&candidate, &gate, &applied));
    assert!(f.gateway.client.application_is_current(&candidate));
    assert!(f.gateway.client.model().pending);
    assert_ne!(f.gateway.client.model().phase, MaterialPhase::Complete);
}

#[tokio::test]
async fn native_material_host_equal_forced_delivery_retries_after_actual_queue_saturation() {
    // Install the actual shared queue and production reset consumer, without a
    // renderer. This tests admission/retry, not complete GUI model ingestion.
    let _queue_guard = crate::gateway::native_queue_test_guard();
    let mut app = bevy::prelude::App::new();
    app.add_plugins(mir2_bevy_runtime::Mir2NativeSessionBoundaryPlugin);
    let mut f = ready();
    seed_quote(&mut f);
    let (owned, dispatch) = confirm(&mut f);
    assert!(matches!(
        f.gateway
            .send(&mut TestSink::default(), &owned, dispatch.clone())
            .await,
        NativeSinkCommit::Flushed
    ));
    f.gateway
        .receive(
            &frame(
                &dispatch,
                wire::Reply::Unknown {
                    quote_id: Some(id()),
                },
                None,
            ),
            &f.fence,
        )
        .unwrap();
    let expected = f.gateway.model();
    assert_eq!(f.gateway.last_model.as_ref(), Some(&expected));
    assert!(f.gateway.client.model().pending);
    assert!(!f.gateway.publish_pending);
    assert!(mir2_bevy_runtime::native_ingest::push_native_data_reset());
    app.update();
    let filler = serde_json::to_string(&HuntingMaterialModel::default()).unwrap();
    let fill = || {
        let mut admitted = 0;
        while mir2_bevy_runtime::native_ingest::push_native_hunting_material_model(filler.clone()) {
            admitted += 1;
            assert!(admitted <= 1024, "actual native queue must remain bounded");
        }
        admitted
    };
    let capacity = fill();
    assert!(capacity > 0);
    // The current Pending/unknown value equals its last successful delivery.
    // A forced acknowledgement still has to survive failed admission.
    f.gateway.publish(true);
    assert!(f.gateway.publish_pending);
    f.gateway.publish(false);
    assert!(f.gateway.publish_pending);
    assert_eq!(f.gateway.last_model.as_ref(), Some(&expected));
    // A real reset dominates the saturated queue and the real consumer drains
    // that barrier. No success boolean or queue-capacity stub is substituted.
    assert!(mir2_bevy_runtime::native_ingest::push_native_data_reset());
    app.update();
    f.gateway.publish(false);
    assert!(!f.gateway.publish_pending);
    assert_eq!(f.gateway.last_model.as_ref(), Some(&expected));
    f.gateway.publish(false);
    assert_eq!(
        fill() + 1,
        capacity,
        "one equal real delivery is retried, without per-frame duplicates"
    );
    assert!(f.gateway.client.model().pending);
}

fn rebind_material_actor(f: &mut Fixture) {
    f.gateway.open_connection();
    f.stamp = f.fence.test_reconnect(3, 0);
    f.gateway.observe_connection(&json!({"type":"realmInfo","payload":{"schema":"mir2-realm-handshake/1",
        "realmId":"synthetic-material","huntingMaterialOwnerProtocolVersion":1,"npcRepairMode":"legacy"}}).to_string()).unwrap();
    f.gateway.observe_connection(&json!({"type":"identitySession","token":format!("e30.{}","A".repeat(43)),"session":{
        "accountId":"alice","sessionId":"synthetic-rebound-session","expiresAtMs":gateway_unix_ms()+600_000,"revokedAtMs":null}}).to_string()).unwrap();
    f.gateway
        .tick(
            &GatewaySessionContext {
                account_id: Some("alice".into()),
                character_index: Some(7),
                ..Default::default()
            },
            &f.fence,
            Some(f.stamp),
            ConnectionPhase::Normal,
            true,
            &mut f.purchases,
        )
        .unwrap();
}

#[tokio::test]
async fn native_material_host_tombstone_recovers_never_entered_confirm_as_query_only() {
    for stage in 0..4 {
        let mut f = ready();
        seed_quote(&mut f);
        let mut sink = TestSink::default();
        match stage {
            0 => {} // UI Confirm was discarded by provenance reset before host.
            1 => {
                let (owned, _prepared) = confirm(&mut f);
                f.fence.retire(&owned);
            }
            2 => {
                let (owned, dispatch) = confirm(&mut f);
                let fence = f.fence.clone();
                sink.0.lock().unwrap().ready_hook = Some(Box::new(move || {
                    fence.0.lock().unwrap().hunting_material_source.invalidate()
                }));
                assert!(matches!(
                    f.gateway.send(&mut sink, &owned, dispatch).await,
                    NativeSinkCommit::DefinitelyUnsent
                ));
            }
            3 => {
                let close = owned(&f, HuntingMaterialCommand::Close);
                assert!(f.gateway.prepare(&close, &mut f.purchases).is_none());
                f.fence.retire(&close);
            }
            _ => unreachable!(),
        }
        assert!(sink.0.lock().unwrap().frames.is_empty());
        rebind_material_actor(&mut f);
        assert!(!f.gateway.client.model().pending);
        assert!(f.gateway.client.model().quote_id.is_none());
        let query = owned(&f, HuntingMaterialCommand::Query { quote_id: id() });
        let dispatch = f
            .gateway
            .prepare(&query, &mut f.purchases)
            .expect("private original tombstone");
        assert!(matches!(
            f.gateway.send(&mut sink, &query, dispatch.clone()).await,
            NativeSinkCommit::Flushed
        ));
        f.gateway
            .receive(
                &frame(
                    &dispatch,
                    wire::Reply::Rejected {
                        quote_id: Some(id()),
                        code: wire::Rejection::NotSubmitted,
                    },
                    None,
                ),
                &f.fence,
            )
            .unwrap();
        assert!(!f.gateway.client.model().pending);
        assert_eq!(f.gateway.client.model().phase, MaterialPhase::Rejected);
        let invalid = owned(
            &f,
            HuntingMaterialCommand::Confirm {
                quote_id: id(),
                source: None,
            },
        );
        assert!(f.gateway.prepare(&invalid, &mut f.purchases).is_none());
        f.fence.retire(&invalid);
        let frames = &sink.0.lock().unwrap().frames;
        assert_eq!(frames.len(), 1);
        assert!(matches!(
            wire::parse_client_request(&frames[0]).unwrap().action,
            wire::Action::Query { .. }
        ));
    }
}

#[tokio::test]
async fn native_material_host_tombstone_recovers_lost_terminal_views_with_immutable_receipt() {
    for completed in [false, true] {
        let mut f = ready();
        seed_quote(&mut f);
        let mut sink = TestSink::default();
        let mut world = bevy::prelude::World::new();
        if completed {
            let (owned, dispatch) = confirm(&mut f);
            assert!(matches!(
                f.gateway.send(&mut sink, &owned, dispatch.clone()).await,
                NativeSinkCommit::Flushed
            ));
            let q = f.gateway.client.model().quote.unwrap();
            let mut bundles = Vec::new();
            f.gateway
                .receive_with(
                    &frame(
                        &dispatch,
                        wire::Reply::Committed {
                            receipt: receipt(&q),
                            replayed: false,
                            reload_required: false,
                        },
                        Some(owner(true)),
                    ),
                    &f.fence,
                    |bundle| {
                        bundles.push(bundle);
                        true
                    },
                )
                .unwrap();
            assert!(
                mir2_bevy_runtime::hunting_material_economy::apply_native_hunting_material_bundle(
                    &mut world,
                    bundles.pop().unwrap()
                )
                .is_ok()
            );
            f.gateway.drain_applied(&f.fence).unwrap();
            assert_eq!(f.gateway.client.model().phase, MaterialPhase::Complete);
        } else {
            let query = owned(&f, HuntingMaterialCommand::Query { quote_id: id() });
            let dispatch = f.gateway.prepare(&query, &mut f.purchases).unwrap();
            assert!(matches!(
                f.gateway.send(&mut sink, &query, dispatch.clone()).await,
                NativeSinkCommit::Flushed
            ));
            f.gateway
                .receive(
                    &frame(
                        &dispatch,
                        wire::Reply::Rejected {
                            quote_id: Some(id()),
                            code: wire::Rejection::NotSubmitted,
                        },
                        None,
                    ),
                    &f.fence,
                )
                .unwrap();
            assert_eq!(f.gateway.client.model().phase, MaterialPhase::Rejected);
        }
        assert!(!f.gateway.client.model().pending);
        // The host terminal view was never consumed before the reset.
        rebind_material_actor(&mut f);
        assert!(f.gateway.client.model().quote_id.is_none());
        let query = owned(&f, HuntingMaterialCommand::Query { quote_id: id() });
        let dispatch = f.gateway.prepare(&query, &mut f.purchases).unwrap();
        assert_eq!(f.gateway.client.model().receipt.is_some(), completed);
        assert!(matches!(
            f.gateway.send(&mut sink, &query, dispatch.clone()).await,
            NativeSinkCommit::Flushed
        ));
        let rejected = frame(
            &dispatch,
            wire::Reply::Rejected {
                quote_id: Some(id()),
                code: wire::Rejection::NotSubmitted,
            },
            None,
        );
        if completed {
            assert!(f.gateway.receive(&rejected, &f.fence).is_err());
            assert!(f.gateway.client.model().receipt.is_some());
            let exact = f.gateway.client.model().receipt.unwrap();
            let mut bundles = Vec::new();
            f.gateway
                .receive_with(
                    &frame(
                        &dispatch,
                        wire::Reply::Committed {
                            receipt: exact,
                            replayed: true,
                            reload_required: false,
                        },
                        Some(owner(true)),
                    ),
                    &f.fence,
                    |bundle| {
                        bundles.push(bundle);
                        true
                    },
                )
                .unwrap();
            assert!(
                mir2_bevy_runtime::hunting_material_economy::apply_native_hunting_material_bundle(
                    &mut world,
                    bundles.pop().unwrap()
                )
                .is_ok()
            );
            f.gateway.drain_applied(&f.fence).unwrap();
            assert_eq!(f.gateway.client.model().phase, MaterialPhase::Complete);
        } else {
            f.gateway.receive(&rejected, &f.fence).unwrap();
            assert!(!f.gateway.client.model().pending);
        }
        let frames = &sink.0.lock().unwrap().frames;
        assert_eq!(frames.len(), 2);
        let confirm_count = frames
            .iter()
            .filter(|raw| {
                matches!(
                    wire::parse_client_request(raw).unwrap().action,
                    wire::Action::Confirm { .. }
                )
            })
            .count();
        assert_eq!(
            confirm_count,
            usize::from(completed),
            "recovery never reissues Confirm"
        );
        assert!(matches!(
            wire::parse_client_request(&frames[1]).unwrap().action,
            wire::Action::Query { .. }
        ));
    }
}
