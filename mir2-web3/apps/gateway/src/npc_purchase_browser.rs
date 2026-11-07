//! Actual durable owner seam for the external browser/native purchase protocol.
//! Opt-in is a proposal. Accepted producer evidence comes from a successful
//! authenticated Begin and its same-turn complete owner snapshot pair.
use crate::{npc_purchase_owner_route::NpcPurchaseOwnerRouteExecution, GatewaySession};
use mir2_client_wire as wire;
use mir2_protocol::ServerPacket;
use mir2_simulation::npc_purchase_journal::{NpcPurchaseIntent, NpcPurchaseOperation};
use mir2_simulation::{
    NpcPurchaseCurrency, NpcPurchaseDurableError, NpcPurchaseOwnerAction, NpcPurchaseOwnerReply,
    NpcPurchaseProcessingOutcome, NpcPurchaseProducer, NpcPurchaseReceipt, NpcPurchaseRejection,
    NpcPurchaseRequest, NpcPurchaseSource,
};

#[derive(Debug, Default)]
pub struct BrowserNpcPurchaseConnection {
    opted_in: bool,
    accepted: Option<NpcPurchaseProducer>,
}
impl BrowserNpcPurchaseConnection {
    pub fn set_opted_in(&mut self, opted_in: bool) {
        self.opted_in = opted_in;
        if !opted_in {
            self.accepted = None;
        }
    }
    pub fn accepted(&self) -> Option<NpcPurchaseProducer> {
        self.accepted
    }
    pub fn withdraw(&mut self) {
        self.accepted = None;
    }
}

pub struct BrowserNpcPurchaseDispatch {
    pub frame: wire::ServerFrame,
    pub packets: Vec<ServerPacket>,
}
fn identity(bytes: [u8; 32]) -> Result<wire::Opaque32, String> {
    wire::Opaque32::from_bytes(bytes).map_err(|error| error.to_string())
}
fn currency(value: NpcPurchaseCurrency) -> wire::Currency {
    match value {
        NpcPurchaseCurrency::Gold => wire::Currency::Gold,
        NpcPurchaseCurrency::Pearls => wire::Currency::Pearls,
    }
}
fn source(value: NpcPurchaseSource) -> wire::Source {
    match value {
        NpcPurchaseSource::Trade => wire::Source::Trade,
        NpcPurchaseSource::BuyBack => wire::Source::BuyBack,
        NpcPurchaseSource::Used => wire::Source::Used,
    }
}
fn request(value: NpcPurchaseRequest) -> wire::PurchaseRequest {
    wire::PurchaseRequest {
        item_index: wire::U64::new(value.item_index),
        count: value.count,
        panel_type: value.panel_type,
    }
}
fn intent(value: NpcPurchaseIntent) -> Result<wire::Intent, String> {
    Ok(wire::Intent {
        request: request(value.request),
        currency: currency(value.currency),
        source: source(value.source),
        service_catalog_proof: identity(value.service_catalog_proof)?,
    })
}
fn operation(value: NpcPurchaseOperation) -> Result<wire::Operation, String> {
    Ok(wire::Operation {
        actor: identity(value.actor)?,
        request_scope: identity(value.request_scope)?,
        sequence: wire::U64::new(value.sequence),
        intent: intent(value.intent)?,
    })
}
fn producer(value: NpcPurchaseProducer) -> Result<wire::Producer, String> {
    Ok(wire::Producer {
        actor: identity(value.actor)?,
        producer_scope: identity(value.producer_scope)?,
        server_revision: wire::U64::new(value.server_revision),
    })
}
fn rejection(value: NpcPurchaseRejection) -> wire::Rejection {
    match value {
        NpcPurchaseRejection::InvalidRequest => wire::Rejection::InvalidRequest,
        NpcPurchaseRejection::PlayerDead => wire::Rejection::PlayerDead,
        NpcPurchaseRejection::ServiceUnavailable => wire::Rejection::ServiceUnavailable,
        NpcPurchaseRejection::UnsupportedService => wire::Rejection::UnsupportedService,
        NpcPurchaseRejection::UnknownGood => wire::Rejection::UnknownGood,
        NpcPurchaseRejection::InvalidQuantity => wire::Rejection::InvalidQuantity,
        NpcPurchaseRejection::InsufficientCurrency => wire::Rejection::InsufficientCurrency,
        NpcPurchaseRejection::ClockUnavailable => wire::Rejection::ClockUnavailable,
        NpcPurchaseRejection::InvalidDelivery => wire::Rejection::InvalidDelivery,
    }
}
pub(crate) fn receipt(value: &NpcPurchaseReceipt) -> Result<wire::Receipt, String> {
    value
        .validate_for_operation(value.entry.operation)
        .map_err(|error| error.to_string())?;
    let outcome = match value.entry.outcome.clone() {
        NpcPurchaseProcessingOutcome::Committed {
            request: raw,
            currency: c,
            source: s,
            charged,
            admitted_count,
            incoming_unique_id,
        } => wire::Outcome::Committed {
            request: request(raw),
            currency: currency(c),
            source: source(s),
            charged,
            admitted_count,
            incoming_unique_id: wire::U64::new(incoming_unique_id),
        },
        NpcPurchaseProcessingOutcome::Rejected {
            request: raw,
            reason,
        } => wire::Outcome::Rejected {
            request: request(raw),
            reason: rejection(reason),
        },
    };
    Ok(wire::Receipt {
        producer_scope: identity(value.producer_scope)?,
        entry: wire::ReceiptEntry {
            operation: operation(value.entry.operation)?,
            server_revision: wire::U64::new(value.entry.server_revision),
            outcome,
        },
    })
}
fn owner_request(value: wire::PurchaseRequest) -> NpcPurchaseRequest {
    NpcPurchaseRequest {
        item_index: value.item_index.get(),
        count: value.count,
        panel_type: value.panel_type,
    }
}
fn owner_operation(value: wire::Operation) -> NpcPurchaseOperation {
    NpcPurchaseOperation {
        actor: value.actor.get(),
        request_scope: value.request_scope.get(),
        sequence: value.sequence.get(),
        intent: NpcPurchaseIntent {
            request: owner_request(value.intent.request),
            currency: match value.intent.currency {
                wire::Currency::Gold => NpcPurchaseCurrency::Gold,
                wire::Currency::Pearls => NpcPurchaseCurrency::Pearls,
            },
            source: match value.intent.source {
                wire::Source::Trade => NpcPurchaseSource::Trade,
                wire::Source::BuyBack => NpcPurchaseSource::BuyBack,
                wire::Source::Used => NpcPurchaseSource::Used,
            },
            service_catalog_proof: value.intent.service_catalog_proof.get(),
        },
    }
}
fn owner_action(value: &wire::Action) -> NpcPurchaseOwnerAction {
    match value {
        wire::Action::Begin => NpcPurchaseOwnerAction::Begin,
        wire::Action::Quote { request } => NpcPurchaseOwnerAction::Quote {
            request: owner_request(request.clone()),
        },
        wire::Action::Query { operation } => NpcPurchaseOwnerAction::Query {
            operation: owner_operation(operation.clone()),
        },
        wire::Action::Purchase { operation } => NpcPurchaseOwnerAction::Purchase {
            operation: owner_operation(operation.clone()),
        },
    }
}
fn frame_failure(
    request: &wire::ClientRequest,
    state: wire::FailureState,
    known: Option<&NpcPurchaseReceipt>,
) -> Result<BrowserNpcPurchaseDispatch, String> {
    let frame = wire::ServerFrame::new(
        request.request_id,
        wire::ServerReply::Failure {
            state,
            receipt: known.map(receipt).transpose()?,
        },
        None,
        None,
    )
    .map_err(|error| error.to_string())?;
    frame
        .validate_for_request(request)
        .map_err(|error| error.to_string())?;
    Ok(BrowserNpcPurchaseDispatch {
        frame,
        packets: Vec::new(),
    })
}
/// Serialize only the supplied owner turn. This function never reads Session
/// or exposes the private journal/checkpoint attached to a route execution.
fn complete_frame_from_execution(
    request: &wire::ClientRequest,
    executed: &NpcPurchaseOwnerRouteExecution,
) -> Result<wire::ServerFrame, String> {
    let reply = match &executed.reply {
        NpcPurchaseOwnerReply::Producer { producer: value } => wire::ServerReply::Producer {
            producer: producer(*value)?,
        },
        NpcPurchaseOwnerReply::Quote { intent: value } => wire::ServerReply::Quote {
            intent: intent(*value)?,
        },
        NpcPurchaseOwnerReply::Recovery { receipt: value } => wire::ServerReply::Recovery {
            receipt: value.as_ref().map(receipt).transpose()?,
        },
        NpcPurchaseOwnerReply::Purchase {
            receipt: value,
            replayed,
        } => wire::ServerReply::Purchase {
            receipt: receipt(value)?,
            replayed: *replayed,
        },
    };
    let snapshot = executed
        .snapshot
        .as_ref()
        .map(|snapshot| {
            serde_json::to_value(snapshot.client_view()).map_err(|error| error.to_string())
        })
        .transpose()?;
    let authority = executed.authority.map(producer).transpose()?;
    let frame = wire::ServerFrame::new(request.request_id, reply, snapshot, authority)
        .map_err(|error| error.to_string())?;
    frame
        .validate_for_request(request)
        .map_err(|error| error.to_string())?;
    Ok(frame)
}
/// Preserve a genuine, correlated terminal receipt even if a later snapshot
/// projection or frame-size check fails. Never fabricate a receipt for Begin
/// or Quote, and never replace a failed mutation with ordinary gameplay.
pub(crate) fn frame_from_execution(
    request: &wire::ClientRequest,
    executed: &NpcPurchaseOwnerRouteExecution,
) -> Result<wire::ServerFrame, String> {
    let full = complete_frame_from_execution(request, executed).and_then(|frame| {
        wire::encode_server_frame(&frame).map_err(|error| error.to_string())?;
        Ok(frame)
    });
    match full {
        Ok(frame) => Ok(frame),
        Err(detail) => {
            let Some(known) = crate::npc_purchase_owner_route::terminal_receipt(&executed.reply)
            else {
                return Err(detail);
            };
            let Some(original) = owner_action(&request.action).operation() else {
                return Err(detail);
            };
            known
                .validate_for_operation(original)
                .map_err(|error| error.to_string())?;
            let fallback =
                frame_failure(request, wire::FailureState::PostCommit, Some(&known))?.frame;
            wire::encode_server_frame(&fallback).map_err(|error| error.to_string())?;
            Ok(fallback)
        }
    }
}
/// Called by the actual socket serial loop after current commercial identity
/// verification. It neither starts a session nor retries a mutation.
pub fn execute_browser_npc_purchase(
    session: &mut GatewaySession,
    state: &mut BrowserNpcPurchaseConnection,
    authenticated_account: Option<&str>,
    identity_verified: bool,
    external: &wire::ClientRequest,
) -> Result<BrowserNpcPurchaseDispatch, String> {
    execute_browser_npc_purchase_action(session, state, authenticated_account, identity_verified,
        external, owner_action(&external.action))
}

fn execute_browser_npc_purchase_action(
    session: &mut GatewaySession,
    state: &mut BrowserNpcPurchaseConnection,
    authenticated_account: Option<&str>,
    identity_verified: bool,
    external: &wire::ClientRequest,
    action: NpcPurchaseOwnerAction,
) -> Result<BrowserNpcPurchaseDispatch, String> {
    external.validate().map_err(|error| error.to_string())?;
    let active = session.active_identity();
    let qualified = identity_verified
        && state.opted_in
        && authenticated_account
            .zip(active.as_ref())
            .is_some_and(|(account, active)| {
                !account.is_empty() && account.trim() == account && active.account_id == account
            })
        && session.supports_durable_npc_purchase_owner();
    if !qualified {
        state.withdraw();
        return frame_failure(external, wire::FailureState::BeforeExecution, None);
    }
    match &external.action {
        wire::Action::Begin => state.withdraw(),
        wire::Action::Quote { .. } | wire::Action::Query { .. } | wire::Action::Purchase { .. } => {
            let Some(accepted) = state.accepted else {
                return frame_failure(external, wire::FailureState::BeforeExecution, None);
            };
            let matches_binding = match &external.action {
                wire::Action::Query { operation } => operation.actor.get() == accepted.actor,
                wire::Action::Purchase { operation } => {
                    operation.actor.get() == accepted.actor
                        && operation.request_scope.get() == accepted.producer_scope
                }
                _ => true,
            };
            if !matches_binding {
                return frame_failure(external, wire::FailureState::BeforeExecution, None);
            }
        }
    }
    match session.execute_npc_purchase_owner(true, action) {
        Ok(executed) => {
            let frame = frame_from_execution(external, &executed)?;
            if let NpcPurchaseOwnerReply::Producer { producer } = executed.reply {
                if executed.authority == Some(producer) && executed.snapshot.is_some() {
                    state.accepted = Some(producer);
                }
            }
            Ok(BrowserNpcPurchaseDispatch {
                frame,
                packets: executed.execution.packets,
            })
        }
        Err(NpcPurchaseDurableError::BeforeExecution { .. }) => {
            frame_failure(external, wire::FailureState::BeforeExecution, None)
        }
        Err(NpcPurchaseDurableError::Unknown { .. }) => {
            frame_failure(external, wire::FailureState::Unknown, None)
        }
        Err(NpcPurchaseDurableError::PostCommit { receipt: known, .. }) => {
            let Some(operation) = action.operation() else {
                return Err("unrelated post-commit receipt".into());
            };
            known
                .validate_for_operation(operation)
                .map_err(|error| error.to_string())?;
            frame_failure(external, wire::FailureState::PostCommit, Some(&known))
        }
    }
}

/// Companion purchases retain the existing external protocol. A server-owned
/// permit, never a request field, selects the internal bounded transaction.
/// The single-writer ledger guard covers reserve, economic execution and result
/// publication into the ledger; owner revocation cannot race a new expenditure.
pub(crate) fn execute_companion_npc_purchase(
    session: &mut GatewaySession,
    state: &mut BrowserNpcPurchaseConnection,
    authenticated_account: Option<&str>,
    identity_verified: bool,
    external: &wire::ClientRequest,
    relay: Option<&crate::companion_relay::CompanionRelay>,
    admission: Option<&crate::companion_relay::Admission>,
    now_ms: u64,
) -> Result<BrowserNpcPurchaseDispatch, String> {
    external.validate().map_err(|_| "Invalid bounded NPC purchase request.".to_string())?;
    let active = session.active_identity();
    let binding = active.as_ref().map(|active| crate::companion_relay::CharacterBinding {
        account_id: active.account_id.clone(), character_index: active.character_index,
    });
    let enrolled = binding.as_ref().zip(relay).is_some_and(|(actor, relay)| relay.policy().actor_owner(actor).is_some());
    let qualified = identity_verified && state.opted_in
        && authenticated_account.zip(active.as_ref()).is_some_and(|(account, active)| account == active.account_id && !account.is_empty())
        && session.supports_durable_npc_purchase_owner();
    let producer_matches = state.accepted.is_some_and(|accepted| match &external.action {
        wire::Action::Purchase { operation } => operation.actor.get() == accepted.actor && operation.request_scope.get() == accepted.producer_scope,
        wire::Action::Query { operation } => operation.actor.get() == accepted.actor,
        _ => true,
    });
    if !enrolled || !qualified || !producer_matches || matches!(external.action, wire::Action::Begin) {
        return execute_browser_npc_purchase(session, state, authenticated_account, identity_verified, external);
    }
    let binding = binding.expect("enrolled canonical actor");
    let Some(ledger) = relay.and_then(|relay| relay.supply_ledger()) else {
        return frame_failure(external, wire::FailureState::BeforeExecution, None);
    };
    let Ok(mut ledger) = ledger.lock() else {
        return frame_failure(external, wire::FailureState::BeforeExecution, None);
    };
    match &external.action {
        wire::Action::Begin => unreachable!("handled above"),
        wire::Action::Quote { .. } => {
            let permitted = admission.filter(|admission| admission.character == binding && admission.expires_at_ms > now_ms)
                .is_some_and(|admission| ledger.active_grant(&binding, &admission.id, now_ms).is_ok_and(|grant| grant.is_some()));
            if !permitted { return frame_failure(external, wire::FailureState::BeforeExecution, None); }
            execute_browser_npc_purchase(session, state, authenticated_account, identity_verified, external)
        }
        wire::Action::Query { operation } => {
            if ledger.query_allowed(&binding, owner_operation(operation.clone())).is_err() {
                return frame_failure(external, wire::FailureState::BeforeExecution, None);
            }
            let dispatch = execute_browser_npc_purchase(session, state, authenticated_account, identity_verified, external)?;
            persist_supply_reply(&mut ledger, &binding, &dispatch.frame.reply);
            Ok(dispatch)
        }
        wire::Action::Purchase { operation } => {
            let Some(admission) = admission.filter(|admission| admission.character == binding && admission.expires_at_ms > now_ms) else {
                return frame_failure(external, wire::FailureState::BeforeExecution, None);
            };
            let original = owner_operation(operation.clone());
            let reservation = match ledger.reserve(&binding, &admission.id, original, now_ms) {
                Ok(reservation) => reservation,
                Err(_) => return frame_failure(external, wire::FailureState::BeforeExecution, None),
            };
            let dispatch = if reservation.already_reserved {
                // Crash before execution and loss after commit are indistinguishable.
                // Neither authorizes another first execution of this reservation.
                let mut recovered = match session.execute_npc_purchase_owner(true, NpcPurchaseOwnerAction::Query { operation: original }) {
                    Ok(recovered) => recovered,
                    Err(_) => return frame_failure(external, wire::FailureState::Unknown, None),
                };
                match recovered.reply {
                    NpcPurchaseOwnerReply::Recovery { receipt: Some(ref known) } => {
                        known.validate_for_operation(original).map_err(|_| "Invalid original supply receipt.".to_string())?;
                        recovered.reply = NpcPurchaseOwnerReply::Purchase { receipt: known.clone(), replayed: true };
                    }
                    _ => return frame_failure(external, wire::FailureState::Unknown, None),
                }
                BrowserNpcPurchaseDispatch { frame: frame_from_execution(external, &recovered)?, packets: Vec::new() }
            } else {
                let limit = mir2_simulation::NpcPurchaseGoldLimit {
                    max_gold: reservation.grant.max_gold, max_count: 1, item_template_index: 658, npc_index: 9,
                };
                execute_browser_npc_purchase_action(session, state, authenticated_account, identity_verified,
                    external, NpcPurchaseOwnerAction::PurchaseBounded { operation: original, limit })?
            };
            persist_supply_reply(&mut ledger, &binding, &dispatch.frame.reply);
            Ok(dispatch)
        }
    }
}

#[cfg(test)]
#[path = "companion_supply_route_tests.rs"]
mod companion_supply_route_tests;

fn persist_supply_reply(
    ledger: &mut crate::companion_supply::SupplyLedger,
    actor: &crate::companion_relay::CharacterBinding,
    reply: &wire::ServerReply,
) {
    let known = match reply {
        wire::ServerReply::Purchase { receipt, .. } => Some(receipt),
        wire::ServerReply::Recovery { receipt } | wire::ServerReply::Failure { receipt, .. } => receipt.as_ref(),
        _ => None,
    };
    let Some(known) = known else { return; };
    let outcome = match known.entry.outcome.clone() {
        wire::Outcome::Committed { request, currency, source, charged, admitted_count, incoming_unique_id } => NpcPurchaseProcessingOutcome::Committed {
            request: owner_request(request),
            currency: match currency { wire::Currency::Gold => NpcPurchaseCurrency::Gold, wire::Currency::Pearls => NpcPurchaseCurrency::Pearls },
            source: match source { wire::Source::Trade => NpcPurchaseSource::Trade, wire::Source::BuyBack => NpcPurchaseSource::BuyBack, wire::Source::Used => NpcPurchaseSource::Used },
            charged, admitted_count, incoming_unique_id: incoming_unique_id.get(),
        },
        wire::Outcome::Rejected { request, reason } => NpcPurchaseProcessingOutcome::Rejected {
            request: owner_request(request), reason: match reason {
                wire::Rejection::InvalidRequest => NpcPurchaseRejection::InvalidRequest,
                wire::Rejection::PlayerDead => NpcPurchaseRejection::PlayerDead,
                wire::Rejection::ServiceUnavailable => NpcPurchaseRejection::ServiceUnavailable,
                wire::Rejection::UnsupportedService => NpcPurchaseRejection::UnsupportedService,
                wire::Rejection::UnknownGood => NpcPurchaseRejection::UnknownGood,
                wire::Rejection::InvalidQuantity => NpcPurchaseRejection::InvalidQuantity,
                wire::Rejection::InsufficientCurrency => NpcPurchaseRejection::InsufficientCurrency,
                wire::Rejection::ClockUnavailable => NpcPurchaseRejection::ClockUnavailable,
                wire::Rejection::InvalidDelivery => NpcPurchaseRejection::InvalidDelivery,
            },
        },
    };
    // An uncertain auxiliary publication seals further execution. The real
    // game receipt is still returned truthfully and can be reconciled by Query.
    let _ = ledger.resolve(actor, owner_operation(known.entry.operation.clone()), &outcome);
}
