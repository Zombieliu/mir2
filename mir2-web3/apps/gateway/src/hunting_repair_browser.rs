//! Strict browser carrier for the local authenticated repair owner. No action
//! is translated into purchase-v1 or the legacy repair packet stream.
use crate::GatewaySession;
use mir2_client_wire::{hunting_repair as wire, U64};
use mir2_protocol::ServerPacket;
use mir2_simulation::{
    HuntingRepairAction, HuntingRepairExecution, HuntingRepairRejection, HuntingRepairReply,
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
    let frame = wire::ServerFrame::new(request, reply).map_err(|error| error.to_string())?;
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
    match session.execute_hunting_repair_owner(true, owner_action(&request.action)) {
        Ok(execution) => frame_from_execution(request, execution),
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
            Ok(HuntingRepairExecution {
                reply: HuntingRepairReply::Pending { quote_id },
                packets: Vec::new(),
            })
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
        assert!(!encoded.contains("snapshot"));
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
}

#[cfg(test)]
#[path = "hunting_repair_postgres_tests.rs"]
mod postgres_tests;
