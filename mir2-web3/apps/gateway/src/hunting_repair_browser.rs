//! Strict browser carrier for the local authenticated repair owner. No action
//! is translated into purchase-v1 or the legacy repair packet stream.
use crate::GatewaySession;
use mir2_client_wire::{hunting_repair as wire, U64};
use mir2_protocol::ServerPacket;
use mir2_simulation::{
    HuntingRepairAction, HuntingRepairExecution, HuntingRepairOwnerTurn, HuntingRepairRejection,
    HuntingRepairReply,
};

pub struct BrowserHuntingRepairDispatch {
    pub frame: wire::ServerFrame,
    pub packets: Vec<ServerPacket>,
    /// Only a confirmed runtime retirement closes the original connection.
    pub close_session: bool,
}

fn owner_action(action: &wire::Action) -> HuntingRepairAction {
    match action {
        wire::Action::Quote { unique_id, special } => HuntingRepairAction::Quote {
            unique_id: unique_id.get(),
            special: *special,
        },
        wire::Action::Confirm { quote_id } => HuntingRepairAction::Confirm {
            quote_id: quote_id.clone(),
        },
        wire::Action::Query { quote_id } => HuntingRepairAction::Query {
            quote_id: quote_id.clone(),
        },
    }
}
fn rejection(value: HuntingRepairRejection) -> wire::Rejection {
    match value {
        HuntingRepairRejection::Disabled => wire::Rejection::Disabled,
        HuntingRepairRejection::Unauthenticated => wire::Rejection::Unauthenticated,
        HuntingRepairRejection::Unavailable => wire::Rejection::Unavailable,
        HuntingRepairRejection::InvalidRequest => wire::Rejection::InvalidRequest,
        HuntingRepairRejection::Unsupported => wire::Rejection::Unsupported,
        HuntingRepairRejection::Expired => wire::Rejection::Expired,
        HuntingRepairRejection::ItemChanged => wire::Rejection::ItemChanged,
        HuntingRepairRejection::NotSubmitted => wire::Rejection::NotSubmitted,
    }
}
fn response(
    request: &wire::ClientRequest,
    reply: wire::Reply,
    packets: Vec<ServerPacket>,
    close_session: bool,
) -> Result<BrowserHuntingRepairDispatch, String> {
    let frame = wire::ServerFrame::new(request, reply, None).map_err(|error| error.to_string())?;
    wire::encode_server_frame(&frame).map_err(|error| error.to_string())?;
    Ok(BrowserHuntingRepairDispatch {
        frame,
        packets,
        close_session,
    })
}
pub(crate) fn frame_from_execution(
    request: &wire::ClientRequest,
    execution: HuntingRepairExecution,
) -> Result<BrowserHuntingRepairDispatch, String> {
    let close_session = matches!(
        &execution.reply,
        HuntingRepairReply::Committed {
            reload_required: true,
            ..
        }
    );
    let reply = match execution.reply {
        HuntingRepairReply::Quoted { quote } => wire::Reply::Quoted {
            quote: wire::Quote {
                quote_id: quote.quote_id,
                unique_id: U64::new(quote.unique_id),
                special: quote.special,
                cost_bound_gold: U64::new(quote.cost_bound_gold),
                created_at_ms: quote.created_at_ms,
                expires_at_ms: quote.expires_at_ms,
            },
        },
        HuntingRepairReply::Pending { quote_id } => wire::Reply::Pending { quote_id },
        HuntingRepairReply::Committed {
            receipt,
            replayed,
            reload_required,
        } => wire::Reply::Committed {
            receipt: wire::Receipt {
                quote_id: receipt.quote_id,
                cost_bound_gold: U64::new(receipt.cost_bound_gold),
                free: receipt.free,
                operation_key: receipt.operation_key,
                request_digest: receipt.request_digest,
                business_day: receipt.business_day,
                committed_at_ms: receipt.committed_at_ms,
            },
            replayed,
            reload_required,
        },
        HuntingRepairReply::Rejected { quote_id, code } => wire::Reply::Rejected {
            // This is correlation with an existing request, never creation of a
            // quote or replacement operation for an unavailable owner.
            quote_id: quote_id.or_else(|| request.action.quote_id().map(str::to_owned)),
            code: rejection(code),
        },
    };
    response(request, reply, execution.packets, close_session)
}

/// Project only the owner turn supplied by the fenced route. A failed or
/// oversized current projection cannot erase its immutable committed receipt.
pub(crate) fn frame_from_turn(
    request: &wire::ClientRequest,
    turn: HuntingRepairOwnerTurn,
) -> Result<BrowserHuntingRepairDispatch, String> {
    let mut dispatch = frame_from_execution(request, turn.repair)?;
    if matches!(
        &dispatch.frame.reply,
        wire::Reply::Committed {
            reload_required: false,
            ..
        }
    ) {
        if let Some(snapshot) = turn.snapshot {
            let projected = crate::session::catch_gateway_panic(
                "hunting repair browser snapshot projection",
                || {
                    let value = serde_json::to_value(snapshot.client_view())
                        .map_err(|error| error.to_string())?;
                    let mut frame = dispatch.frame.clone();
                    frame.snapshot = Some(value);
                    wire::encode_server_frame(&frame).map_err(|error| error.to_string())?;
                    Ok::<_, String>(frame)
                },
            )
            .and_then(|result| result);
            if let Ok(frame) = projected {
                dispatch.frame = frame;
            }
        }
    }
    Ok(dispatch)
}
pub(crate) fn unknown(
    request: &wire::ClientRequest,
) -> Result<BrowserHuntingRepairDispatch, String> {
    response(
        request,
        wire::Reply::Unknown {
            quote_id: request.action.quote_id().map(str::to_owned),
        },
        Vec::new(),
        false,
    )
}
/// The actual socket loop supplies current account/cache proof. Query uses the
/// same authenticated owner gate even after repair policy is disabled.
pub fn execute_browser_hunting_repair(
    session: &mut GatewaySession,
    authenticated_account: Option<&str>,
    identity_verified: bool,
    request: &wire::ClientRequest,
) -> Result<BrowserHuntingRepairDispatch, String> {
    request.validate().map_err(|error| error.to_string())?;
    let identity = session.active_identity();
    let authenticated = identity_verified
        && authenticated_account
            .zip(identity.as_ref())
            .is_some_and(|(account, active)| {
                !account.is_empty() && account.trim() == account && active.account_id == account
            });
    if !authenticated {
        return response(
            request,
            wire::Reply::Rejected {
                quote_id: request.action.quote_id().map(str::to_owned),
                code: wire::Rejection::Unauthenticated,
            },
            Vec::new(),
            false,
        );
    }
    if !session.supports_hunting_repair_owner() {
        return response(
            request,
            wire::Reply::Rejected {
                quote_id: request.action.quote_id().map(str::to_owned),
                code: wire::Rejection::Unsupported,
            },
            Vec::new(),
            false,
        );
    }
    match session.execute_hunting_repair_owner_turn(true, owner_action(&request.action)) {
        Ok(turn) => frame_from_turn(request, turn),
        // Before/after execution cannot be distinguished after a panic or an
        // owner transport failure. Never turn missing evidence into rejection.
        Err(_) => unknown(request),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::{
        InMemoryZoneOwnerLeaseAuthority, SharedZoneOwnerLeaseAuthority, ZoneId,
        ZoneOwnerLeaseAuthority,
    };
    use mir2_simulation::{
        ActiveSessionIdentity, HuntingRepairReceiptView, InProcessWorldRuntime, SimulationConfig,
        WorldCommand, WorldRuntime, WorldSnapshot,
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    struct Owner {
        inner: InProcessWorldRuntime,
        calls: Arc<AtomicUsize>,
        supported: bool,
        unknown: bool,
        committed: bool,
        world_reads: Arc<AtomicUsize>,
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
        fn execute(&mut self, _command: WorldCommand) -> Result<Vec<ServerPacket>, String> {
            panic!("typed repair must not fall back to generic or legacy execution")
        }
        fn world_snapshot(&self) -> WorldSnapshot {
            self.world_reads.fetch_add(1, Ordering::SeqCst);
            self.inner.world_snapshot()
        }
        fn active_identity(&self) -> Option<ActiveSessionIdentity> {
            Some(ActiveSessionIdentity {
                account_id: "owner".into(),
                character_index: 1,
                character_name: "Tester".into(),
            })
        }
        fn save_active_character(&mut self) -> Result<(), String> {
            Ok(())
        }
        fn refresh_active_external_mail(&mut self) -> bool {
            false
        }
        fn supports_hunting_repair_owner(&self) -> bool {
            self.supported
        }
        fn execute_hunting_repair_owner(
            &mut self,
            authenticated: bool,
            action: HuntingRepairAction,
        ) -> Result<HuntingRepairExecution, String> {
            assert!(authenticated);
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.unknown {
                return Err("unknown source acknowledgement".into());
            }
            let HuntingRepairAction::Query { quote_id } = action else {
                panic!("expected original query");
            };
            if self.committed {
                return Ok(committed_repair(quote_id));
            }
            Ok(HuntingRepairExecution {
                reply: HuntingRepairReply::Pending { quote_id },
                packets: Vec::new(),
            })
        }
        fn execute_hunting_repair_owner_turn(
            &mut self,
            authenticated: bool,
            action: HuntingRepairAction,
        ) -> Result<HuntingRepairOwnerTurn, String> {
            let repair = self.execute_hunting_repair_owner(authenticated, action)?;
            let snapshot = self.committed.then(|| self.world_snapshot());
            Ok(HuntingRepairOwnerTurn { repair, snapshot })
        }
    }
    fn session(
        supported: bool,
        unknown: bool,
        with_authority: bool,
    ) -> (
        GatewaySession,
        Arc<AtomicUsize>,
        Arc<InMemoryZoneOwnerLeaseAuthority>,
    ) {
        let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
        let zone = ZoneId::primary();
        let lease = authority.owner_lease(&zone);
        let calls = Arc::new(AtomicUsize::new(0));
        let runtime = Box::new(Owner {
            inner: InProcessWorldRuntime::new(SimulationConfig::default()),
            calls: calls.clone(),
            supported,
            unknown,
            committed: false,
            world_reads: Arc::new(AtomicUsize::new(0)),
        });
        let gate = with_authority.then(|| authority.clone() as SharedZoneOwnerLeaseAuthority);
        (
            GatewaySession::with_routed_world_runtime_and_owner_authority(
                zone, lease, gate, runtime,
            ),
            calls,
            authority,
        )
    }
    fn query() -> wire::ClientRequest {
        wire::ClientRequest::new(
            U64::new(7),
            wire::Action::Query {
                quote_id: "ab".repeat(32),
            },
        )
        .unwrap()
    }
    fn committed_repair(quote_id: String) -> HuntingRepairExecution {
        HuntingRepairExecution {
            reply: HuntingRepairReply::Committed {
                receipt: HuntingRepairReceiptView {
                    quote_id,
                    cost_bound_gold: 0,
                    free: true,
                    operation_key: "original-op".into(),
                    request_digest: "original-digest".into(),
                    business_day: 1,
                    committed_at_ms: 1,
                },
                replayed: true,
                reload_required: false,
            },
            packets: Vec::new(),
        }
    }
    #[test]
    fn real_gateway_boundary_requires_current_account_and_local_owner_guard() {
        let (mut session, calls, authority) = session(true, false, true);
        let request = query();
        for (account, verified) in [(Some("owner"), false), (Some("other"), true), (None, true)] {
            let dispatch =
                execute_browser_hunting_repair(&mut session, account, verified, &request).unwrap();
            assert!(matches!(
                dispatch.frame.reply,
                wire::Reply::Rejected {
                    code: wire::Rejection::Unauthenticated,
                    ..
                }
            ));
        }
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        let dispatch =
            execute_browser_hunting_repair(&mut session, Some("owner"), true, &request).unwrap();
        assert!(matches!(dispatch.frame.reply, wire::Reply::Pending { .. }));
        assert!(!dispatch.close_session);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        authority.handoff_zone_owner(&ZoneId::primary(), "next-owner");
        let dispatch =
            execute_browser_hunting_repair(&mut session, Some("owner"), true, &request).unwrap();
        assert!(matches!(dispatch.frame.reply, wire::Reply::Unknown { .. }));
        assert!(!dispatch.close_session);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn unsupported_paths_never_execute_and_unknown_preserves_same_session_for_query() {
        for (supported, with_authority) in [(false, true), (true, false)] {
            let (mut session, calls, _) = session(supported, false, with_authority);
            let dispatch =
                execute_browser_hunting_repair(&mut session, Some("owner"), true, &query())
                    .unwrap();
            assert!(matches!(
                dispatch.frame.reply,
                wire::Reply::Rejected {
                    code: wire::Rejection::Unsupported,
                    ..
                }
            ));
            assert_eq!(calls.load(Ordering::SeqCst), 0);
        }
        let (mut session, calls, _) = session(true, true, true);
        for _ in 0..2 {
            let dispatch =
                execute_browser_hunting_repair(&mut session, Some("owner"), true, &query())
                    .unwrap();
            assert!(matches!(dispatch.frame.reply, wire::Reply::Unknown { .. }));
            assert!(!dispatch.close_session);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn immutable_free_receipt_has_no_historical_wallet_projection() {
        let id = "ab".repeat(32);
        let request = wire::ClientRequest::new(
            U64::new(1),
            wire::Action::Query {
                quote_id: id.clone(),
            },
        )
        .unwrap();
        let dispatch = frame_from_execution(
            &request,
            HuntingRepairExecution {
                reply: HuntingRepairReply::Committed {
                    receipt: HuntingRepairReceiptView {
                        quote_id: id,
                        cost_bound_gold: 0,
                        free: true,
                        operation_key: "op".into(),
                        request_digest: "digest".into(),
                        business_day: 1,
                        committed_at_ms: 1,
                    },
                    replayed: true,
                    reload_required: true,
                },
                packets: Vec::new(),
            },
        )
        .unwrap();
        assert!(dispatch.close_session);
        let encoded = wire::encode_server_frame(&dispatch.frame).unwrap();
        assert!(!encoded.contains("wallet"));
        assert!(dispatch.frame.snapshot.is_none());
        assert!(encoded.contains("\"snapshot\":null"));
    }
    #[test]
    fn unknown_retains_original_query_without_closing_or_packets() {
        let request = wire::ClientRequest::new(
            U64::new(3),
            wire::Action::Confirm {
                quote_id: "cd".repeat(32),
            },
        )
        .unwrap();
        let dispatch = unknown(&request).unwrap();
        assert!(!dispatch.close_session && dispatch.packets.is_empty());
        assert!(
            matches!(dispatch.frame.reply, wire::Reply::Unknown { quote_id:Some(id) } if id=="cd".repeat(32))
        );
    }

    #[test]
    fn owner_turn_snapshot_is_immutable_and_oversize_keeps_original_commit() {
        let request = query();
        let mut snapshot = InProcessWorldRuntime::new(SimulationConfig::default()).world_snapshot();
        snapshot.gold = 123;
        let captured = frame_from_turn(
            &request,
            HuntingRepairOwnerTurn {
                repair: committed_repair("ab".repeat(32)),
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
            HuntingRepairOwnerTurn {
                repair: committed_repair("ab".repeat(32)),
                snapshot: Some(snapshot),
            },
        )
        .unwrap();
        assert_eq!(limited.frame.reply, captured.frame.reply);
        assert!(limited.frame.snapshot.is_none() && !limited.close_session);
        let encoded = wire::encode_server_frame(&limited.frame).unwrap();
        assert!(encoded.len() < 4096);
        wire::parse_server_frame(&encoded)
            .unwrap()
            .validate_for_request(&request)
            .unwrap();
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
            Err("controlled guard completion/delivery failure".into())
        }
    }

    #[test]
    fn actual_owner_turn_retains_captured_commit_after_guard_release_failure_without_reread() {
        for invoke in [true, false] {
            let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
            let zone = ZoneId::primary();
            let lease = authority.owner_lease(&zone);
            let calls = Arc::new(AtomicUsize::new(0));
            let reads = Arc::new(AtomicUsize::new(0));
            let runtime = Box::new(Owner {
                inner: InProcessWorldRuntime::new(SimulationConfig::default()),
                calls: calls.clone(),
                supported: true,
                unknown: false,
                committed: true,
                world_reads: reads.clone(),
            });
            let gate =
                Arc::new(CompletionGuard { authority, invoke }) as SharedZoneOwnerLeaseAuthority;
            let mut session = GatewaySession::with_routed_world_runtime_and_owner_authority(
                zone,
                lease,
                Some(gate),
                runtime,
            );
            let before_reads = reads.load(Ordering::SeqCst);
            let dispatch =
                execute_browser_hunting_repair(&mut session, Some("owner"), true, &query())
                    .unwrap();
            if invoke {
                assert!(matches!(
                    &dispatch.frame.reply,
                    wire::Reply::Committed {
                        reload_required: false,
                        ..
                    }
                ));
                assert!(dispatch.frame.snapshot.is_some());
            } else {
                assert!(matches!(&dispatch.frame.reply, wire::Reply::Unknown { .. }));
                assert!(dispatch.frame.snapshot.is_none());
            }
            assert_eq!(calls.load(Ordering::SeqCst), usize::from(invoke));
            assert_eq!(
                reads.load(Ordering::SeqCst) - before_reads,
                usize::from(invoke),
                "no outer Session snapshot may repair a lost turn"
            );
        }
    }
}

#[cfg(test)]
#[path = "hunting_repair_postgres_tests.rs"]
mod postgres_tests;
