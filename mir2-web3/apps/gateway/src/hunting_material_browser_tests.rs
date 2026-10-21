//! Local typed-owner adapter checks with the real GatewaySession lease gate.
//! These fixtures do not run PostgreSQL, a socket transport or a shared World.
use super::*;
use crate::routing::{
    InMemoryZoneOwnerLeaseAuthority, SharedZoneOwnerLeaseAuthority, ZoneId, ZoneOwnerLeaseAuthority,
};
use mir2_simulation::{
    ActiveSessionIdentity, HuntingMaterialQuoteView, HuntingMaterialReceiptView,
    InProcessWorldRuntime, SimulationConfig, WorldCommand, WorldRuntime, WorldSnapshot,
};
use serde_json::json;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};

fn id() -> String {
    "ab".repeat(32)
}
fn query() -> wire::ClientRequest {
    wire::ClientRequest::new(U64::new(7), wire::Action::Query { quote_id: id() }).unwrap()
}
fn confirm() -> wire::ClientRequest {
    wire::ClientRequest::new(U64::new(8), wire::Action::Confirm { quote_id: id() }).unwrap()
}
fn quote_request() -> wire::ClientRequest {
    wire::ClientRequest::new(
        U64::new(u64::MAX),
        wire::Action::Quote {
            unique_id: U64::new(u64::MAX),
            quantity: U64::new(3),
        },
    )
    .unwrap()
}
fn receipt(quote_id: String) -> HuntingMaterialReceiptView {
    HuntingMaterialReceiptView {
        quote_id,
        input_unique_id: u64::MAX,
        output_unique_id: u64::MAX - 1,
        input_count: 10,
        quantity: 3,
        gold_equivalent: 303,
        fee_unbound_gold: 7,
        license_atoms_debit: 303,
        valuation_version: "browser-fixture-prices-v1".into(),
        operation_key: "cd".repeat(32),
        request_digest: format!("sha256:{}", "ef".repeat(32)),
        business_day: 20_000,
        committed_at_ms: 2000,
    }
}
fn committed_execution(quote_id: String, reload_required: bool) -> HuntingMaterialExecution {
    HuntingMaterialExecution {
        reply: HuntingMaterialReply::Committed {
            receipt: receipt(quote_id),
            replayed: true,
            reload_required,
        },
        packets: if reload_required {
            vec![ServerPacket::LogOutSuccess {
                characters: Vec::new(),
            }]
        } else {
            Vec::new()
        },
    }
}

#[derive(Clone, Copy)]
enum Mode {
    Normal,
    CommittedQuery,
    Error,
    Panic,
}
struct Owner {
    inner: InProcessWorldRuntime,
    actions: Arc<Mutex<Vec<HuntingMaterialAction>>>,
    world_reads: Arc<AtomicUsize>,
    identity_panic: Arc<AtomicBool>,
    capability_panic: Arc<AtomicBool>,
    supported: bool,
    local: bool,
    mode: Mode,
}
impl WorldRuntime for Owner {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn on_connect(&self) -> Vec<ServerPacket> {
        Vec::new()
    }
    fn execute(&mut self, _: WorldCommand) -> Result<Vec<ServerPacket>, String> {
        panic!("material owner must never fall back to generic or legacy execution")
    }
    fn world_snapshot(&self) -> WorldSnapshot {
        self.world_reads.fetch_add(1, Ordering::SeqCst);
        let mut snapshot = self.inner.world_snapshot();
        snapshot.gold = 123;
        snapshot
    }
    fn active_identity(&self) -> Option<ActiveSessionIdentity> {
        assert!(
            !self.identity_panic.load(Ordering::SeqCst),
            "controlled material identity failure"
        );
        Some(ActiveSessionIdentity {
            account_id: "owner".into(),
            character_index: 1,
            character_name: "MaterialTester".into(),
        })
    }
    fn save_active_character(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn refresh_active_external_mail(&mut self) -> bool {
        false
    }
    fn requires_local_hunting_source_authority(&self) -> bool {
        self.local
    }
    fn supports_hunting_material_owner(&self) -> bool {
        assert!(
            !self.capability_panic.load(Ordering::SeqCst),
            "controlled material capability failure"
        );
        self.supported
    }
    fn execute_hunting_material_owner_turn(
        &mut self,
        authenticated: bool,
        action: HuntingMaterialAction,
    ) -> Result<HuntingMaterialOwnerTurn, String> {
        assert!(authenticated);
        self.actions.lock().unwrap().push(action.clone());
        match self.mode {
            Mode::Error => return Err("material owner acknowledgement unavailable".into()),
            Mode::Panic => panic!("controlled material owner execution failure"),
            _ => {}
        }
        let material = match action {
            HuntingMaterialAction::Quote {
                unique_id,
                quantity,
            } => HuntingMaterialExecution {
                reply: HuntingMaterialReply::Quoted {
                    quote: HuntingMaterialQuoteView {
                        quote_id: id(),
                        unique_id,
                        item_index: 1001,
                        input_count: 10,
                        quantity,
                        unit_gold: 101,
                        gold_equivalent: 101 * quantity,
                        fee_unbound_gold: (101 * quantity * 2 + 99) / 100,
                        license_atoms_debit: 101 * quantity,
                        valuation_version: "browser-fixture-prices-v1".into(),
                        created_at_ms: 1000,
                        expires_at_ms: 31_000,
                    },
                },
                packets: Vec::new(),
            },
            HuntingMaterialAction::Confirm { quote_id } => committed_execution(quote_id, false),
            HuntingMaterialAction::Query { quote_id } => {
                if matches!(self.mode, Mode::CommittedQuery) {
                    committed_execution(quote_id, false)
                } else {
                    HuntingMaterialExecution {
                        reply: HuntingMaterialReply::Pending { quote_id },
                        packets: Vec::new(),
                    }
                }
            }
        };
        let snapshot = matches!(
            &material.reply,
            HuntingMaterialReply::Committed {
                reload_required: false,
                ..
            }
        )
        .then(|| self.world_snapshot());
        Ok(HuntingMaterialOwnerTurn { material, snapshot })
    }
}
struct Fixture {
    session: GatewaySession,
    authority: Arc<InMemoryZoneOwnerLeaseAuthority>,
    actions: Arc<Mutex<Vec<HuntingMaterialAction>>>,
    world_reads: Arc<AtomicUsize>,
    identity_panic: Arc<AtomicBool>,
    capability_panic: Arc<AtomicBool>,
}
fn fixture(supported: bool, local: bool, with_authority: bool, mode: Mode) -> Fixture {
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let zone = ZoneId::primary();
    let lease = authority.owner_lease(&zone);
    let actions = Arc::new(Mutex::new(Vec::new()));
    let world_reads = Arc::new(AtomicUsize::new(0));
    let identity_panic = Arc::new(AtomicBool::new(false));
    let capability_panic = Arc::new(AtomicBool::new(false));
    let runtime = Box::new(Owner {
        inner: InProcessWorldRuntime::new(SimulationConfig::default()),
        actions: actions.clone(),
        world_reads: world_reads.clone(),
        supported,
        local,
        mode,
        identity_panic: identity_panic.clone(),
        capability_panic: capability_panic.clone(),
    });
    let gate = with_authority.then(|| authority.clone() as SharedZoneOwnerLeaseAuthority);
    Fixture {
        session: GatewaySession::with_routed_world_runtime_and_owner_authority(
            zone, lease, gate, runtime,
        ),
        authority,
        actions,
        world_reads,
        identity_panic,
        capability_panic,
    }
}

#[test]
fn material_browser_requires_verified_current_account_before_any_typed_execution() {
    let mut f = fixture(true, true, true, Mode::Normal);
    let request = query();
    for (account, verified) in [
        (Some("owner"), false),
        (Some("other"), true),
        (None, true),
        (Some(""), true),
        (Some(" owner"), true),
        (Some("owner "), true),
    ] {
        let dispatch =
            execute_browser_hunting_material(&mut f.session, account, verified, &request).unwrap();
        assert!(matches!(dispatch.frame.reply, wire::Reply::Rejected {
            quote_id: Some(ref actual), code: wire::Rejection::Unauthenticated,
        } if actual == &id()));
        assert!(!dispatch.close_session && dispatch.packets.is_empty());
    }
    assert!(f.actions.lock().unwrap().is_empty());
    assert_eq!(f.world_reads.load(Ordering::SeqCst), 0);
    let mut invalid = request;
    invalid.request_id = U64::new(0);
    assert!(
        execute_browser_hunting_material(&mut f.session, Some("owner"), true, &invalid).is_err()
    );
    assert!(f.actions.lock().unwrap().is_empty());
    // Enable faults only after the real GatewaySession captured its initial
    // identity. Adapter preflight failures must not escape or execute assets.
    f.identity_panic.store(true, Ordering::SeqCst);
    let dispatch =
        execute_browser_hunting_material(&mut f.session, Some("owner"), true, &query()).unwrap();
    assert_eq!(
        dispatch.frame.reply,
        wire::Reply::Unknown {
            quote_id: Some(id())
        }
    );
    assert!(!dispatch.close_session && dispatch.packets.is_empty());
    f.identity_panic.store(false, Ordering::SeqCst);
    f.capability_panic.store(true, Ordering::SeqCst);
    let dispatch =
        execute_browser_hunting_material(&mut f.session, Some("owner"), true, &query()).unwrap();
    assert_eq!(
        dispatch.frame.reply,
        wire::Reply::Unknown {
            quote_id: Some(id())
        }
    );
    assert!(!dispatch.close_session && dispatch.packets.is_empty());
    assert!(f.actions.lock().unwrap().is_empty());
    assert_eq!(f.world_reads.load(Ordering::SeqCst), 0);
}

#[test]
fn material_browser_refuses_unsupported_nonlocal_and_unguarded_runtimes() {
    for (supported, local, with_authority) in [
        (false, true, true),
        (true, false, true),
        (true, true, false),
    ] {
        let mut f = fixture(supported, local, with_authority, Mode::Normal);
        let dispatch =
            execute_browser_hunting_material(&mut f.session, Some("owner"), true, &query())
                .unwrap();
        assert!(matches!(dispatch.frame.reply, wire::Reply::Rejected {
            quote_id: Some(ref actual), code: wire::Rejection::Unsupported,
        } if actual == &id()));
        assert!(!dispatch.close_session && dispatch.frame.snapshot.is_none());
        assert!(f.actions.lock().unwrap().is_empty());
        assert_eq!(f.world_reads.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn material_browser_maps_typed_quote_confirm_query_without_truncating_source_identity() {
    let mut f = fixture(true, true, true, Mode::Normal);
    let quoted =
        execute_browser_hunting_material(&mut f.session, Some("owner"), true, &quote_request())
            .unwrap();
    let wire::Reply::Quoted { quote } = &quoted.frame.reply else {
        panic!("missing typed quote")
    };
    assert_eq!(quote.unique_id.get(), u64::MAX);
    assert_eq!(quote.input_count.get(), 10);
    assert_eq!(quote.quantity.get(), 3);
    assert_eq!(quote.gold_equivalent.get(), 303);
    assert_eq!(quote.fee_unbound_gold.get(), 7);
    assert_eq!(f.world_reads.load(Ordering::SeqCst), 0);
    let committed =
        execute_browser_hunting_material(&mut f.session, Some("owner"), true, &confirm()).unwrap();
    let wire::Reply::Committed {
        receipt,
        replayed,
        reload_required,
    } = &committed.frame.reply
    else {
        panic!("missing typed receipt")
    };
    assert!(*replayed && !*reload_required);
    assert_eq!(receipt.input_unique_id.get(), u64::MAX);
    assert_eq!(receipt.output_unique_id.get(), u64::MAX - 1);
    assert_eq!(receipt.input_count.get(), 10);
    receipt.validate_for_quote(quote).unwrap();
    assert_eq!(committed.frame.snapshot.as_ref().unwrap()["gold"], 123);
    assert!(!committed.close_session);
    let encoded = wire::encode_server_frame(&committed.frame).unwrap();
    assert_eq!(wire::parse_server_frame(&encoded).unwrap(), committed.frame);
    assert!(encoded.contains("\"inputUniqueId\":\"18446744073709551615\""));
    assert!(encoded.contains("\"outputUniqueId\":\"18446744073709551614\""));
    let pending =
        execute_browser_hunting_material(&mut f.session, Some("owner"), true, &query()).unwrap();
    assert!(
        matches!(pending.frame.reply, wire::Reply::Pending { quote_id: ref actual } if actual == &id())
    );
    let actions = f.actions.lock().unwrap();
    assert_eq!(actions.len(), 3);
    assert!(
        matches!(&actions[0], HuntingMaterialAction::Quote { unique_id, quantity } if *unique_id == u64::MAX && *quantity == 3)
    );
    assert!(
        matches!(&actions[1], HuntingMaterialAction::Confirm { quote_id } if quote_id == &id())
    );
    assert!(matches!(&actions[2], HuntingMaterialAction::Query { quote_id } if quote_id == &id()));
    assert_eq!(
        f.world_reads.load(Ordering::SeqCst),
        1,
        "only the committed owner turn reads its World"
    );
}

#[test]
fn material_browser_handoff_keeps_original_query_unknown_without_execution_or_close() {
    let mut f = fixture(true, true, true, Mode::Normal);
    let request = query();
    let first =
        execute_browser_hunting_material(&mut f.session, Some("owner"), true, &request).unwrap();
    assert!(matches!(first.frame.reply, wire::Reply::Pending { .. }));
    f.authority
        .handoff_zone_owner(&ZoneId::primary(), "next-material-owner");
    let dispatch =
        execute_browser_hunting_material(&mut f.session, Some("owner"), true, &request).unwrap();
    assert!(
        matches!(dispatch.frame.reply, wire::Reply::Unknown { quote_id: Some(ref actual) } if actual == &id())
    );
    assert!(!dispatch.close_session && dispatch.packets.is_empty());
    assert!(dispatch.frame.snapshot.is_none());
    assert_eq!(f.actions.lock().unwrap().len(), 1);
    assert_eq!(f.world_reads.load(Ordering::SeqCst), 0);
}

#[test]
fn material_browser_maps_every_typed_rejection_without_issuing_another_quote() {
    let request = query();
    for (code, expected) in [
        (
            HuntingMaterialRejection::Disabled,
            wire::Rejection::Disabled,
        ),
        (
            HuntingMaterialRejection::Unauthenticated,
            wire::Rejection::Unauthenticated,
        ),
        (
            HuntingMaterialRejection::Unavailable,
            wire::Rejection::Unavailable,
        ),
        (
            HuntingMaterialRejection::InvalidRequest,
            wire::Rejection::InvalidRequest,
        ),
        (
            HuntingMaterialRejection::Unsupported,
            wire::Rejection::Unsupported,
        ),
        (HuntingMaterialRejection::Expired, wire::Rejection::Expired),
        (
            HuntingMaterialRejection::ItemChanged,
            wire::Rejection::ItemChanged,
        ),
        (
            HuntingMaterialRejection::NotSubmitted,
            wire::Rejection::NotSubmitted,
        ),
        (
            HuntingMaterialRejection::Unconfigured,
            wire::Rejection::Unconfigured,
        ),
        (
            HuntingMaterialRejection::InsufficientPermits,
            wire::Rejection::InsufficientPermits,
        ),
        (
            HuntingMaterialRejection::InsufficientUnboundGold,
            wire::Rejection::InsufficientUnboundGold,
        ),
    ] {
        let dispatch = frame_from_execution(
            &request,
            HuntingMaterialExecution {
                reply: HuntingMaterialReply::Rejected {
                    quote_id: None,
                    code,
                },
                packets: Vec::new(),
            },
        )
        .unwrap();
        assert_eq!(
            dispatch.frame.reply,
            wire::Reply::Rejected {
                quote_id: Some(id()),
                code: expected
            }
        );
        assert!(!dispatch.close_session && dispatch.frame.snapshot.is_none());
        let unquoted = frame_from_execution(
            &quote_request(),
            HuntingMaterialExecution {
                reply: HuntingMaterialReply::Rejected {
                    quote_id: None,
                    code,
                },
                packets: Vec::new(),
            },
        )
        .unwrap();
        assert_eq!(
            unquoted.frame.reply,
            wire::Reply::Rejected {
                quote_id: None,
                code: expected
            }
        );
    }
}

#[test]
fn material_browser_captured_snapshot_is_immutable_and_oversize_retains_original_receipt() {
    let request = query();
    let mut snapshot = InProcessWorldRuntime::new(SimulationConfig::default()).world_snapshot();
    snapshot.gold = 123;
    let captured = frame_from_turn(
        &request,
        HuntingMaterialOwnerTurn {
            material: committed_execution(id(), false),
            snapshot: Some(snapshot.clone()),
        },
    )
    .unwrap();
    assert_eq!(captured.frame.snapshot.as_ref().unwrap()["gold"], 123);
    snapshot.gold = 999;
    assert_eq!(captured.frame.snapshot.as_ref().unwrap()["gold"], 123);
    snapshot.map_title = Some("x".repeat(wire::MAX_FRAME_BYTES));
    let limited = frame_from_turn(
        &request,
        HuntingMaterialOwnerTurn {
            material: committed_execution(id(), false),
            snapshot: Some(snapshot),
        },
    )
    .unwrap();
    assert_eq!(limited.frame.reply, captured.frame.reply);
    assert!(limited.frame.snapshot.is_none() && !limited.close_session);
    let encoded = wire::encode_server_frame(&limited.frame).unwrap();
    assert!(encoded.contains("\"snapshot\":null"));
    assert!(!encoded.contains("walletAfter") && !encoded.contains("checkpoint"));
    assert!(encoded.len() < 4096);
    wire::parse_server_frame(&encoded)
        .unwrap()
        .validate_for_request(&request)
        .unwrap();
}

#[test]
fn material_browser_reload_closes_only_confirmed_retirement_and_keeps_packets_without_snapshot() {
    let snapshot = InProcessWorldRuntime::new(SimulationConfig::default()).world_snapshot();
    let dispatch = frame_from_turn(
        &query(),
        HuntingMaterialOwnerTurn {
            material: committed_execution(id(), true),
            snapshot: Some(snapshot),
        },
    )
    .unwrap();
    assert!(dispatch.close_session);
    assert!(dispatch.frame.snapshot.is_none());
    assert!(matches!(
        dispatch.frame.reply,
        wire::Reply::Committed {
            reload_required: true,
            ..
        }
    ));
    assert!(
        matches!(dispatch.packets.as_slice(), [ServerPacket::LogOutSuccess { characters }] if characters.is_empty())
    );
    let raw = serde_json::to_value(&dispatch.frame).unwrap();
    assert_eq!(raw["snapshot"], json!(null));
    assert_eq!(raw["reply"]["receipt"]["operationKey"], "cd".repeat(32));
    let mut wrong = committed_execution("ef".repeat(32), false);
    assert!(frame_from_execution(&query(), wrong.clone()).is_err());
    if let HuntingMaterialReply::Committed { receipt, .. } = &mut wrong.reply {
        receipt.quote_id = id();
        receipt.output_unique_id = receipt.input_unique_id;
    }
    assert!(frame_from_execution(&query(), wrong).is_err());
}

#[test]
fn material_browser_execution_errors_and_panics_preserve_unknown_original_without_retry() {
    for mode in [Mode::Error, Mode::Panic] {
        let mut f = fixture(true, true, true, mode);
        for request in [confirm(), query(), quote_request()] {
            let before = f.actions.lock().unwrap().len();
            let dispatch =
                execute_browser_hunting_material(&mut f.session, Some("owner"), true, &request)
                    .unwrap();
            assert_eq!(
                dispatch.frame.reply,
                wire::Reply::Unknown {
                    quote_id: request.action.quote_id().map(str::to_owned),
                }
            );
            assert!(!dispatch.close_session && dispatch.packets.is_empty());
            assert!(dispatch.frame.snapshot.is_none());
            assert_eq!(
                f.actions.lock().unwrap().len(),
                before + 1,
                "no automatic second submission"
            );
        }
        assert_eq!(f.world_reads.load(Ordering::SeqCst), 0);
    }
    let dispatch = unknown(&confirm()).unwrap();
    assert_eq!(
        dispatch.frame.reply,
        wire::Reply::Unknown {
            quote_id: Some(id())
        }
    );
    assert!(!dispatch.close_session && dispatch.packets.is_empty());
}

struct CompletionGuard {
    authority: Arc<InMemoryZoneOwnerLeaseAuthority>,
    invoke: bool,
}
impl ZoneOwnerLeaseAuthority for CompletionGuard {
    fn owner_lease(&self, zone: &ZoneId) -> crate::routing::ZoneOwnerLease {
        self.authority.owner_lease(zone)
    }
    fn supports_durable_npc_purchase_lease(&self) -> bool {
        true
    }
    fn with_durable_npc_purchase_lease(
        &self,
        lease: &crate::routing::ZoneOwnerLease,
        operation: &mut (dyn FnMut() + Send),
    ) -> Result<(), String> {
        if self.invoke {
            self.authority
                .with_durable_npc_purchase_lease(lease, operation)?;
        }
        Err("controlled material guard completion/delivery failure".into())
    }
}

#[test]
fn material_browser_actual_guard_failure_retains_captured_owner_turn_without_outer_snapshot() {
    for invoke in [true, false] {
        let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
        let zone = ZoneId::primary();
        let lease = authority.owner_lease(&zone);
        let actions = Arc::new(Mutex::new(Vec::new()));
        let reads = Arc::new(AtomicUsize::new(0));
        let runtime = Box::new(Owner {
            inner: InProcessWorldRuntime::new(SimulationConfig::default()),
            actions: actions.clone(),
            world_reads: reads.clone(),
            supported: true,
            local: true,
            mode: Mode::CommittedQuery,
            identity_panic: Arc::new(AtomicBool::new(false)),
            capability_panic: Arc::new(AtomicBool::new(false)),
        });
        let gate = Arc::new(CompletionGuard { authority, invoke }) as SharedZoneOwnerLeaseAuthority;
        let mut session = GatewaySession::with_routed_world_runtime_and_owner_authority(
            zone,
            lease,
            Some(gate),
            runtime,
        );
        let before_reads = reads.load(Ordering::SeqCst);
        let dispatch =
            execute_browser_hunting_material(&mut session, Some("owner"), true, &query()).unwrap();
        if invoke {
            assert!(matches!(
                dispatch.frame.reply,
                wire::Reply::Committed {
                    reload_required: false,
                    ..
                }
            ));
            assert_eq!(dispatch.frame.snapshot.as_ref().unwrap()["gold"], 123);
        } else {
            assert_eq!(
                dispatch.frame.reply,
                wire::Reply::Unknown {
                    quote_id: Some(id())
                }
            );
            assert!(dispatch.frame.snapshot.is_none());
        }
        assert!(!dispatch.close_session);
        assert_eq!(actions.lock().unwrap().len(), usize::from(invoke));
        assert_eq!(
            reads.load(Ordering::SeqCst) - before_reads,
            usize::from(invoke),
            "no outer Session snapshot may replace a missing owner turn"
        );
    }
}
