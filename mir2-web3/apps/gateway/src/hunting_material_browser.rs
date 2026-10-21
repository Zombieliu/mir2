//! Strict authenticated material-owner adapter. Only its typed local owner turn
//! may execute a conversion; no generic command or legacy item fallback exists.
use crate::GatewaySession;
use mir2_client_wire::{hunting_material as wire, U64};
use mir2_protocol::ServerPacket;
use mir2_simulation::{
    HuntingMaterialAction, HuntingMaterialExecution, HuntingMaterialOwnerTurn,
    HuntingMaterialRejection, HuntingMaterialReply,
};

pub struct BrowserHuntingMaterialDispatch {
    pub frame: wire::ServerFrame,
    pub packets: Vec<ServerPacket>,
    /// Only a runtime-confirmed retirement closes the original session.
    pub close_session: bool,
}

fn owner_action(action: &wire::Action) -> HuntingMaterialAction {
    match action {
        wire::Action::Quote {
            unique_id,
            quantity,
        } => HuntingMaterialAction::Quote {
            unique_id: unique_id.get(),
            quantity: quantity.get(),
        },
        wire::Action::Confirm { quote_id } => HuntingMaterialAction::Confirm {
            quote_id: quote_id.clone(),
        },
        wire::Action::Query { quote_id } => HuntingMaterialAction::Query {
            quote_id: quote_id.clone(),
        },
    }
}
fn rejection(value: HuntingMaterialRejection) -> wire::Rejection {
    match value {
        HuntingMaterialRejection::Disabled => wire::Rejection::Disabled,
        HuntingMaterialRejection::Unauthenticated => wire::Rejection::Unauthenticated,
        HuntingMaterialRejection::Unavailable => wire::Rejection::Unavailable,
        HuntingMaterialRejection::InvalidRequest => wire::Rejection::InvalidRequest,
        HuntingMaterialRejection::Unsupported => wire::Rejection::Unsupported,
        HuntingMaterialRejection::Expired => wire::Rejection::Expired,
        HuntingMaterialRejection::ItemChanged => wire::Rejection::ItemChanged,
        HuntingMaterialRejection::NotSubmitted => wire::Rejection::NotSubmitted,
        HuntingMaterialRejection::Unconfigured => wire::Rejection::Unconfigured,
        HuntingMaterialRejection::InsufficientPermits => wire::Rejection::InsufficientPermits,
        HuntingMaterialRejection::InsufficientUnboundGold => {
            wire::Rejection::InsufficientUnboundGold
        }
    }
}
fn response(
    request: &wire::ClientRequest,
    reply: wire::Reply,
    packets: Vec<ServerPacket>,
    close_session: bool,
) -> Result<BrowserHuntingMaterialDispatch, String> {
    let frame = wire::ServerFrame::new(request, reply, None).map_err(|error| error.to_string())?;
    wire::encode_server_frame(&frame).map_err(|error| error.to_string())?;
    Ok(BrowserHuntingMaterialDispatch {
        frame,
        packets,
        close_session,
    })
}

pub(crate) fn frame_from_execution(
    request: &wire::ClientRequest,
    execution: HuntingMaterialExecution,
) -> Result<BrowserHuntingMaterialDispatch, String> {
    let close_session = matches!(
        &execution.reply,
        HuntingMaterialReply::Committed {
            reload_required: true,
            ..
        }
    );
    let reply = match execution.reply {
        HuntingMaterialReply::Quoted { quote } => wire::Reply::Quoted {
            quote: wire::Quote {
                quote_id: quote.quote_id,
                unique_id: U64::new(quote.unique_id),
                item_index: quote.item_index,
                input_count: U64::new(quote.input_count),
                quantity: U64::new(quote.quantity),
                unit_gold: U64::new(quote.unit_gold),
                gold_equivalent: U64::new(quote.gold_equivalent),
                fee_unbound_gold: U64::new(quote.fee_unbound_gold),
                license_atoms_debit: U64::new(quote.license_atoms_debit),
                valuation_version: quote.valuation_version,
                created_at_ms: quote.created_at_ms,
                expires_at_ms: quote.expires_at_ms,
            },
        },
        HuntingMaterialReply::Pending { quote_id } => wire::Reply::Pending { quote_id },
        HuntingMaterialReply::Committed {
            receipt,
            replayed,
            reload_required,
        } => wire::Reply::Committed {
            receipt: wire::Receipt {
                quote_id: receipt.quote_id,
                input_unique_id: U64::new(receipt.input_unique_id),
                output_unique_id: U64::new(receipt.output_unique_id),
                input_count: U64::new(receipt.input_count),
                quantity: U64::new(receipt.quantity),
                gold_equivalent: U64::new(receipt.gold_equivalent),
                fee_unbound_gold: U64::new(receipt.fee_unbound_gold),
                license_atoms_debit: U64::new(receipt.license_atoms_debit),
                valuation_version: receipt.valuation_version,
                operation_key: receipt.operation_key,
                request_digest: receipt.request_digest,
                business_day: receipt.business_day,
                committed_at_ms: receipt.committed_at_ms,
            },
            replayed,
            reload_required,
        },
        HuntingMaterialReply::Rejected { quote_id, code } => wire::Reply::Rejected {
            // Reuse only the request's original correlation. This is not an
            // owner proof or a new quote after an unavailable transaction.
            quote_id: quote_id.or_else(|| request.action.quote_id().map(str::to_owned)),
            code: rejection(code),
        },
    };
    response(request, reply, execution.packets, close_session)
}

/// Consume only the result/projection bundle captured within the actual owner
/// turn. Missing, panicked or oversized projection keeps the immutable receipt;
/// an unrelated outer-session snapshot must never repair it.
pub(crate) fn frame_from_turn(
    request: &wire::ClientRequest,
    turn: HuntingMaterialOwnerTurn,
) -> Result<BrowserHuntingMaterialDispatch, String> {
    let mut dispatch = frame_from_execution(request, turn.material)?;
    if matches!(
        &dispatch.frame.reply,
        wire::Reply::Committed {
            reload_required: false,
            ..
        }
    ) {
        if let Some(snapshot) = turn.snapshot {
            let projected = crate::session::catch_gateway_panic(
                "hunting material browser snapshot projection",
                || {
                    // This established client view removes exact ground item
                    // custody and private social/operation identities. Snapshot
                    // numeric transport sanitization remains the socket layer's
                    // shared owner-presentation responsibility.
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
) -> Result<BrowserHuntingMaterialDispatch, String> {
    response(
        request,
        wire::Reply::Unknown {
            quote_id: request.action.quote_id().map(str::to_owned),
        },
        Vec::new(),
        false,
    )
}

/// Account proof comes from the verified socket identity. The runtime must also
/// expose the dedicated local-owner capability under its current real lease;
/// neither raw RPC owners nor a generic item command can provide that proof.
/// Historical Query keeps this same account gate after admission is disabled.
pub fn execute_browser_hunting_material(
    session: &mut GatewaySession,
    authenticated_account: Option<&str>,
    identity_verified: bool,
    request: &wire::ClientRequest,
) -> Result<BrowserHuntingMaterialDispatch, String> {
    request.validate().map_err(|error| error.to_string())?;
    let identity =
        match crate::session::catch_gateway_panic("hunting material browser owner identity", || {
            session.active_identity()
        }) {
            Ok(identity) => identity,
            Err(_) => return unknown(request),
        };
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
    // supports_hunting_material_owner includes the runtime's local-source
    // requirement and actual owner-lease guard, not only its method name.
    let supported = match crate::session::catch_gateway_panic(
        "hunting material browser owner capability",
        || session.supports_hunting_material_owner(),
    ) {
        Ok(supported) => supported,
        Err(_) => return unknown(request),
    };
    if !supported {
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
    match crate::session::catch_gateway_panic("hunting material browser owner turn", || {
        session.execute_hunting_material_owner_turn(true, owner_action(&request.action))
    })
    .and_then(|result| result)
    {
        Ok(turn) => frame_from_turn(request, turn),
        // Execution/delivery uncertainty cannot become an ordinary rejection
        // or a replacement Confirm. Preserve only the original Query identity.
        Err(_) => unknown(request),
    }
}

#[cfg(test)]
#[path = "hunting_material_browser_tests.rs"]
mod tests;
