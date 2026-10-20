//! Pure, single-writer custody for native hunting repair. Preparation is not
//! transmission. Call `enter` inside the final physical owner's fence, directly
//! before start_send; only a sealed Runtime completion may call `applied`.
use std::collections::{BTreeMap, BTreeSet};

use crate::gateway::NativeCommandStamp;
use mir2_client_bevy::hunting_repair::{HuntingRepairMode, HuntingRepairModel, HuntingRepairPhase};
use mir2_client_wire::{hunting_repair as wire, U64};

type Result<T> = std::result::Result<T, &'static str>;
const CONTROL_TIMEOUT_MS: u64 = 10_000;
const MAX_ACTORS: usize = 32;
const MAX_CONFIRMED_QUOTES: usize = 4096;
const MAX_SELECTION_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RepairActor {
    pub realm: String,
    pub account_id: String,
    pub character_index: i32,
}

#[derive(Debug, Clone)]
pub struct RepairDispatch {
    pub owner: NativeCommandStamp,
    pub request: wire::ClientRequest,
    pub body: String,
}

#[derive(Debug)]
pub struct RepairReceived {
    pub frame: wire::ServerFrame,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AppliedCandidate {
    owner: NativeCommandStamp,
    request_id: u64,
    quote_id: String,
}

#[derive(Debug, Default)]
struct ActorState {
    quote: Option<wire::Quote>,
    quote_id: Option<String>,
    receipt: Option<wire::Receipt>,
    selection: Option<String>,
    confirmed: BTreeSet<String>,
    pending: bool,
    visible: bool,
    phase: HuntingRepairPhase,
    notice: Option<String>,
    reload_required: bool,
    applied_candidate: Option<AppliedCandidate>,
}

impl ActorState {
    fn reset_view(&mut self) {
        self.quote = None;
        self.quote_id = None;
        self.receipt = None;
        self.selection = None;
        self.visible = false;
        self.phase = HuntingRepairPhase::Idle;
        self.notice = None;
        self.reload_required = false;
        self.applied_candidate = None;
    }
}

#[derive(Debug)]
struct Control {
    owner: NativeCommandStamp,
    actor: RepairActor,
    request: wire::ClientRequest,
    body: String,
    selection: Option<String>,
    deadline: u64,
    entered: bool,
}

#[derive(Debug, Default)]
pub struct NativeHuntingRepairClient {
    binding: Option<(NativeCommandStamp, RepairActor)>,
    actors: BTreeMap<RepairActor, ActorState>,
    last_request_id: u64,
    control: Option<Control>,
}

impl NativeHuntingRepairClient {
    pub fn new() -> Self {
        Self::default()
    }

    /// The host has already verified realm mode, identity and full entry. A
    /// different physical owner can only Query a retained pending quote.
    pub fn bind(&mut self, owner: NativeCommandStamp, actor: RepairActor) -> Result<()> {
        if actor.realm.is_empty()
            || actor.realm.len() > 128
            || actor.realm.trim() != actor.realm
            || actor.account_id.is_empty()
            || actor.account_id.len() > 160
            || actor.account_id.trim() != actor.account_id
            || actor.character_index < 0
            || actor.realm.chars().any(char::is_control)
            || actor.account_id.chars().any(char::is_control)
        {
            self.withdraw();
            return Err("Invalid repair actor");
        }
        if self.binding.as_ref() == Some(&(owner, actor.clone())) {
            return Ok(());
        }
        if !self.actors.contains_key(&actor) && self.actors.len() >= MAX_ACTORS {
            self.withdraw();
            return Err("Repair actor custody capacity reached");
        }
        self.withdraw();
        let state = self.actors.entry(actor.clone()).or_default();
        if state.pending {
            state.selection = None;
            state.visible = false;
            state.reload_required = false;
            state.phase = HuntingRepairPhase::Pending;
            state.notice = Some("originalQueryRequired".into());
        } else {
            state.reset_view();
        }
        self.binding = Some((owner, actor));
        Ok(())
    }

    pub fn withdraw(&mut self) {
        self.control = None;
        if let Some((_, actor)) = self.binding.take() {
            if let Some(state) = self.actors.get_mut(&actor) {
                state.applied_candidate = None;
                state.visible = false;
                state.selection = None;
                if !state.pending {
                    state.reset_view();
                }
            }
        }
    }

    fn current_actor(&self, owner: NativeCommandStamp) -> Result<RepairActor> {
        self.binding
            .as_ref()
            .filter(|(saved, _)| *saved == owner)
            .map(|(_, actor)| actor.clone())
            .ok_or("Stale repair owner")
    }

    fn prepare(
        &mut self,
        owner: NativeCommandStamp,
        actor: RepairActor,
        action: wire::Action,
        selection: Option<String>,
        now_ms: u64,
    ) -> Result<RepairDispatch> {
        if self.control.is_some() {
            return Err("Repair control already outstanding");
        }
        let deadline = now_ms
            .checked_add(CONTROL_TIMEOUT_MS)
            .ok_or("Invalid repair clock")?;
        let id = self
            .last_request_id
            .checked_add(1)
            .ok_or("Repair request IDs exhausted")?;
        let request =
            wire::ClientRequest::new(U64::new(id), action).map_err(|_| "Invalid repair request")?;
        let body =
            wire::encode_client_request(&request).map_err(|_| "Repair request encoding failed")?;
        self.last_request_id = id;
        self.control = Some(Control {
            owner,
            actor,
            request: request.clone(),
            body: body.clone(),
            selection,
            deadline,
            entered: false,
        });
        Ok(RepairDispatch {
            owner,
            request,
            body,
        })
    }

    pub fn prepare_quote(
        &mut self,
        owner: NativeCommandStamp,
        uid: u64,
        special: bool,
        selection: String,
        now_ms: u64,
    ) -> Result<RepairDispatch> {
        let actor = self.current_actor(owner)?;
        let state = self
            .actors
            .get(&actor)
            .ok_or("Repair actor custody missing")?;
        if state.pending
            || state.reload_required
            || self.control.is_some()
            || state.confirmed.len() >= MAX_CONFIRMED_QUOTES
            || selection.is_empty()
            || selection.len() > MAX_SELECTION_BYTES
        {
            return Err("Repair quote unavailable");
        }
        let dispatch = self.prepare(
            owner,
            actor.clone(),
            wire::Action::Quote {
                unique_id: U64::new(uid),
                special,
            },
            Some(selection.clone()),
            now_ms,
        )?;
        let state = self.actors.get_mut(&actor).expect("current repair actor");
        state.reset_view();
        state.selection = Some(selection);
        state.visible = true;
        state.phase = HuntingRepairPhase::Quoting;
        Ok(dispatch)
    }

    pub fn prepare_confirm(
        &mut self,
        owner: NativeCommandStamp,
        quote_id: &str,
        selection: &str,
        now_ms: u64,
    ) -> Result<RepairDispatch> {
        let actor = self.current_actor(owner)?;
        let state = self
            .actors
            .get(&actor)
            .ok_or("Repair actor custody missing")?;
        let quote = state
            .quote
            .as_ref()
            .ok_or("Original server quote required")?;
        if state.pending
            || state.reload_required
            || !state.visible
            || state.phase != HuntingRepairPhase::Quoted
            || quote.quote_id != quote_id
            || state.selection.as_deref() != Some(selection)
            || state.confirmed.contains(quote_id)
            || state.confirmed.len() >= MAX_CONFIRMED_QUOTES
            || now_ms >= quote.expires_at_ms as u64
        {
            return Err("Original repair confirmation unavailable");
        }
        self.prepare(
            owner,
            actor,
            wire::Action::Confirm {
                quote_id: quote_id.into(),
            },
            Some(selection.into()),
            now_ms,
        )
    }

    pub fn prepare_query(
        &mut self,
        owner: NativeCommandStamp,
        quote_id: &str,
        now_ms: u64,
    ) -> Result<RepairDispatch> {
        let actor = self.current_actor(owner)?;
        let state = self
            .actors
            .get(&actor)
            .ok_or("Repair actor custody missing")?;
        if state.reload_required || state.quote_id.as_deref() != Some(quote_id) {
            return Err("Original repair query required");
        }
        let dispatch = self.prepare(
            owner,
            actor.clone(),
            wire::Action::Query {
                quote_id: quote_id.into(),
            },
            None,
            now_ms,
        )?;
        // A later query supersedes any unapplied prior source. Its old queued
        // completion cannot release the current unresolved operation.
        self.actors
            .get_mut(&actor)
            .expect("current repair actor")
            .applied_candidate = None;
        Ok(dispatch)
    }

    /// Consumes local transmission once. The host must immediately call its
    /// physical start_send without an await or external callback after success.
    pub fn enter(&mut self, dispatch: &RepairDispatch, now_ms: u64) -> Result<()> {
        let actor = self.current_actor(dispatch.owner)?;
        let control = self.control.as_ref().ok_or("Repair control missing")?;
        if control.entered
            || control.owner != dispatch.owner
            || control.actor != actor
            || control.request != dispatch.request
            || control.body != dispatch.body
            || now_ms >= control.deadline
        {
            return Err("Repair transmission custody mismatch");
        }
        let state = self
            .actors
            .get_mut(&actor)
            .ok_or("Repair actor custody missing")?;
        if state.reload_required {
            return Err("Repair owner requires reload");
        }
        match &control.request.action {
            wire::Action::Quote { .. } => {
                if state.pending || !state.visible || state.selection != control.selection {
                    return Err("Repair selection retired");
                }
            }
            wire::Action::Confirm { quote_id } => {
                let quote = state
                    .quote
                    .as_ref()
                    .ok_or("Original server quote required")?;
                if state.pending
                    || !state.visible
                    || state.phase != HuntingRepairPhase::Quoted
                    || state.selection != control.selection
                    || quote.quote_id != *quote_id
                    || now_ms >= quote.expires_at_ms as u64
                    || state.confirmed.contains(quote_id)
                    || state.confirmed.len() >= MAX_CONFIRMED_QUOTES
                {
                    return Err("Repair confirmation retired");
                }
                state.confirmed.insert(quote_id.clone());
                state.pending = true;
                state.phase = HuntingRepairPhase::Pending;
                state.notice = None;
            }
            wire::Action::Query { quote_id } => {
                if state.quote_id.as_deref() != Some(quote_id) {
                    return Err("Original repair query required");
                }
                state.pending = true;
                state.phase = HuntingRepairPhase::Pending;
                state.notice = None;
            }
        }
        self.control
            .as_mut()
            .expect("checked repair control")
            .entered = true;
        Ok(())
    }

    pub fn definitely_unsent(&mut self, request_id: u64) -> bool {
        if !self.control.as_ref().is_some_and(|control| {
            control.request.request_id.get() == request_id && !control.entered
        }) {
            return false;
        }
        let control = self.control.take().expect("checked unsent control");
        let state = self.actors.get_mut(&control.actor).expect("control actor");
        if matches!(control.request.action, wire::Action::Quote { .. }) {
            state.reset_view();
        }
        state.notice = Some("definitelyUnsent".into());
        true
    }

    pub fn receive(&mut self, owner: NativeCommandStamp, raw: &str) -> Result<RepairReceived> {
        let actor = self.current_actor(owner)?;
        let frame = wire::parse_server_frame(raw).map_err(|_| "Invalid repair server frame")?;
        let control = self
            .control
            .as_ref()
            .ok_or("Repair response custody missing")?;
        if !control.entered || control.owner != owner || control.actor != actor {
            return Err("Stale repair response owner");
        }
        frame
            .validate_for_request(&control.request)
            .map_err(|_| "Repair response correlation mismatch")?;
        let state = self
            .actors
            .get_mut(&actor)
            .ok_or("Repair actor custody missing")?;
        match (&control.request.action, &frame.reply) {
            (wire::Action::Quote { .. }, wire::Reply::Quoted { quote }) => {
                if state.selection != control.selection
                    || !state.visible
                    || state.confirmed.contains(&quote.quote_id)
                {
                    return Err("Repair quote selection retired");
                }
                state.quote = Some(quote.clone());
                state.quote_id = Some(quote.quote_id.clone());
                state.phase = HuntingRepairPhase::Quoted;
                state.notice = None;
            }
            (wire::Action::Quote { .. }, wire::Reply::Rejected { code, .. }) => {
                state.phase = HuntingRepairPhase::Rejected;
                state.notice = Some(rejection(*code).into());
            }
            (wire::Action::Quote { .. }, wire::Reply::Unknown { .. }) => {
                state.phase = HuntingRepairPhase::Idle;
                state.notice = Some("quoteUnknown".into());
            }
            (
                _,
                wire::Reply::Committed {
                    receipt,
                    reload_required,
                    ..
                },
            ) => {
                if state
                    .quote
                    .as_ref()
                    .is_some_and(|quote| quote.cost_bound_gold != receipt.cost_bound_gold)
                    || state.receipt.as_ref().is_some_and(|known| known != receipt)
                    || state.quote_id.as_deref() != Some(receipt.quote_id.as_str())
                {
                    return Err("Immutable repair receipt mismatch");
                }
                state.receipt = Some(receipt.clone());
                state.pending = true;
                state.phase = HuntingRepairPhase::Pending;
                state.reload_required = *reload_required;
                state.notice = Some(
                    if *reload_required {
                        "reloadRequired"
                    } else {
                        "snapshotRequired"
                    }
                    .into(),
                );
                state.applied_candidate = if !*reload_required && frame.snapshot.is_some() {
                    Some(AppliedCandidate {
                        owner,
                        request_id: frame.request_id.get(),
                        quote_id: receipt.quote_id.clone(),
                    })
                } else {
                    None
                };
            }
            (
                wire::Action::Query { .. },
                wire::Reply::Rejected {
                    code: wire::Rejection::NotSubmitted,
                    ..
                },
            ) if state.receipt.is_none() => {
                state.pending = false;
                state.phase = HuntingRepairPhase::Rejected;
                state.notice = Some("notSubmitted".into());
                state.applied_candidate = None;
            }
            (_, wire::Reply::Rejected { code, .. }) => {
                state.pending = true;
                state.phase = HuntingRepairPhase::Pending;
                state.notice = Some(rejection(*code).into());
                state.applied_candidate = None;
            }
            (_, wire::Reply::Pending { .. } | wire::Reply::Unknown { .. }) => {
                state.pending = true;
                state.phase = HuntingRepairPhase::Pending;
                state.notice = Some(
                    if matches!(&frame.reply, wire::Reply::Pending { .. }) {
                        "pending"
                    } else {
                        "unknown"
                    }
                    .into(),
                );
                state.applied_candidate = None;
            }
            _ => return Err("Unexpected repair response"),
        }
        self.control = None;
        Ok(RepairReceived { frame })
    }

    /// The raw frame, its receipt or admission to Runtime is insufficient.
    /// Only the host's sealed exact full-model completion may call this method.
    pub fn applied(&mut self, owner: NativeCommandStamp, request_id: u64, quote_id: &str) -> bool {
        let Ok(actor) = self.current_actor(owner) else {
            return false;
        };
        let Some(state) = self.actors.get_mut(&actor) else {
            return false;
        };
        let expected = AppliedCandidate {
            owner,
            request_id,
            quote_id: quote_id.into(),
        };
        if !state.pending
            || state.reload_required
            || state.applied_candidate.as_ref() != Some(&expected)
            || state
                .receipt
                .as_ref()
                .is_none_or(|receipt| receipt.quote_id != quote_id)
        {
            return false;
        }
        state.applied_candidate = None;
        state.pending = false;
        state.phase = HuntingRepairPhase::Committed;
        state.notice = None;
        true
    }

    pub fn check_timeout(&mut self, now_ms: u64) -> bool {
        if !self
            .control
            .as_ref()
            .is_some_and(|control| now_ms >= control.deadline)
        {
            return false;
        }
        let control = self.control.take().expect("expired repair control");
        let state = self.actors.get_mut(&control.actor).expect("control actor");
        state.phase = if state.pending {
            HuntingRepairPhase::Pending
        } else if state.quote.is_some() {
            HuntingRepairPhase::Quoted
        } else {
            HuntingRepairPhase::Idle
        };
        state.notice = Some("timeout".into());
        true
    }

    pub fn close_view(&mut self, owner: NativeCommandStamp) -> bool {
        let Ok(actor) = self.current_actor(owner) else {
            return false;
        };
        let state = self.actors.get_mut(&actor).expect("current repair actor");
        state.visible = false;
        if !state.pending {
            state.reset_view();
            self.control = None;
        }
        true
    }

    pub fn model(&self) -> HuntingRepairModel {
        let Some(state) = self
            .binding
            .as_ref()
            .and_then(|(_, actor)| self.actors.get(actor))
        else {
            return HuntingRepairModel::default();
        };
        HuntingRepairModel {
            mode: HuntingRepairMode::HuntingV1,
            phase: state.phase,
            quote_id: state.quote_id.clone(),
            unique_id: state.quote.as_ref().map(|quote| quote.unique_id.get()),
            special: state.quote.as_ref().is_some_and(|quote| quote.special),
            cost_bound_gold: state
                .receipt
                .as_ref()
                .map(|receipt| receipt.cost_bound_gold.get())
                .or_else(|| {
                    state
                        .quote
                        .as_ref()
                        .map(|quote| quote.cost_bound_gold.get())
                }),
            expires_at_ms: state.quote.as_ref().map(|quote| quote.expires_at_ms),
            reload_required: state.reload_required,
            notice: state.notice.clone(),
            ..Default::default()
        }
    }
}

fn rejection(code: wire::Rejection) -> &'static str {
    match code {
        wire::Rejection::Disabled => "disabled",
        wire::Rejection::Unauthenticated => "unauthenticated",
        wire::Rejection::Unavailable => "unavailable",
        wire::Rejection::InvalidRequest => "invalidRequest",
        wire::Rejection::Unsupported => "unsupported",
        wire::Rejection::Expired => "expired",
        wire::Rejection::ItemChanged => "itemChanged",
        wire::Rejection::NotSubmitted => "notSubmitted",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn actor() -> RepairActor {
        RepairActor {
            realm: "synthetic-realm".into(),
            account_id: "synthetic-player".into(),
            character_index: 1,
        }
    }
    fn owners() -> (NativeCommandStamp, NativeCommandStamp) {
        let (sender, _) = crate::gateway::command_channel(8);
        let fence = sender.ownership_fence().unwrap();
        (fence.test_world_ready(3, 0), fence.test_reconnect(3, 0))
    }
    fn id() -> String {
        "ab".repeat(32)
    }
    fn receipt() -> wire::Receipt {
        wire::Receipt {
            quote_id: id(),
            cost_bound_gold: U64::new(10),
            free: false,
            operation_key: "original-operation".into(),
            request_digest: "original-digest".into(),
            business_day: 1,
            committed_at_ms: 20,
        }
    }
    fn raw(
        dispatch: &RepairDispatch,
        reply: wire::Reply,
        snapshot: Option<serde_json::Value>,
    ) -> String {
        wire::encode_server_frame(
            &wire::ServerFrame::new(&dispatch.request, reply, snapshot).unwrap(),
        )
        .unwrap()
    }
    fn quoted() -> (NativeHuntingRepairClient, NativeCommandStamp) {
        let owner = owners().0;
        let mut client = NativeHuntingRepairClient::new();
        client.bind(owner, actor()).unwrap();
        let dispatch = client
            .prepare_quote(owner, u64::MAX, true, "original-selection".into(), 0)
            .unwrap();
        assert_eq!(dispatch.request.request_id.get(), 1);
        client.enter(&dispatch, 1).unwrap();
        let quote = wire::Quote {
            quote_id: id(),
            unique_id: U64::new(u64::MAX),
            special: true,
            cost_bound_gold: U64::new(10),
            created_at_ms: 0,
            expires_at_ms: 30_000,
        };
        client
            .receive(owner, &raw(&dispatch, wire::Reply::Quoted { quote }, None))
            .unwrap();
        (client, owner)
    }
    fn confirming() -> (
        NativeHuntingRepairClient,
        NativeCommandStamp,
        RepairDispatch,
    ) {
        let (mut client, owner) = quoted();
        let dispatch = client
            .prepare_confirm(owner, &id(), "original-selection", 2)
            .unwrap();
        client.enter(&dispatch, 3).unwrap();
        (client, owner, dispatch)
    }

    #[test]
    fn quoted_cost_is_readonly_and_confirm_enters_only_once_at_physical_boundary() {
        let (mut client, owner) = quoted();
        let model = client.model();
        assert_eq!(model.phase, HuntingRepairPhase::Quoted);
        assert_eq!(
            (model.unique_id, model.cost_bound_gold, model.bound_gold),
            (Some(u64::MAX), Some(10), None)
        );
        assert!(client
            .prepare_confirm(owner, &id(), "changed-selection", 2)
            .is_err());
        let dispatch = client
            .prepare_confirm(owner, &id(), "original-selection", 2)
            .unwrap();
        assert_eq!(client.model().phase, HuntingRepairPhase::Quoted);
        assert!(client
            .prepare_confirm(owner, &id(), "original-selection", 2)
            .is_err());
        client.enter(&dispatch, 3).unwrap();
        assert_eq!(client.model().phase, HuntingRepairPhase::Pending);
        assert!(client.enter(&dispatch, 3).is_err());
        assert!(!client.definitely_unsent(dispatch.request.request_id.get()));
    }

    #[test]
    fn definitely_unsent_and_expiry_before_entry_do_not_consume_confirmation() {
        let (mut client, owner) = quoted();
        let dispatch = client
            .prepare_confirm(owner, &id(), "original-selection", 2)
            .unwrap();
        assert!(client.definitely_unsent(dispatch.request.request_id.get()));
        let late = client
            .prepare_confirm(owner, &id(), "original-selection", 29_999)
            .unwrap();
        assert!(client.enter(&late, 30_000).is_err());
        assert!(client.definitely_unsent(late.request.request_id.get()));
        assert!(client
            .prepare_confirm(owner, &id(), "original-selection", 30_000)
            .is_err());
        let retry = client
            .prepare_confirm(owner, &id(), "original-selection", 10)
            .unwrap();
        assert!(client.enter(&retry, 11).is_ok());
    }

    #[test]
    fn raw_strictness_and_correlation_fail_without_consuming_original_control() {
        let (mut client, owner, dispatch) = confirming();
        let valid = raw(&dispatch, wire::Reply::Pending { quote_id: id() }, None);
        for invalid in [
            valid.replace(
                "\"protocolVersion\":1",
                "\"protocolVersion\":1,\"protocolVersion\":1",
            ),
            valid.replace(
                "\"snapshot\":null",
                "\"snapshot\":null,\"snap\\u0073hot\":null",
            ),
            valid.replace("\"requestId\":\"2\"", "\"requestId\":\"999\""),
            valid.replace("huntingRepairOwner", "npcPurchaseOwner"),
            valid.replace(",\"snapshot\":null", ""),
        ] {
            assert!(client.receive(owner, &invalid).is_err());
        }
        assert!(client.receive(owner, &valid).is_ok());
        assert!(client.receive(owner, &valid).is_err());
    }

    #[test]
    fn known_receipt_and_null_snapshot_remain_pending_original_query() {
        let (mut client, owner, dispatch) = confirming();
        client
            .receive(
                owner,
                &raw(
                    &dispatch,
                    wire::Reply::Committed {
                        receipt: receipt(),
                        replayed: false,
                        reload_required: false,
                    },
                    None,
                ),
            )
            .unwrap();
        assert!(!client.applied(owner, dispatch.request.request_id.get(), &id()));
        assert_eq!(client.model().phase, HuntingRepairPhase::Pending);
        assert!(client
            .prepare_quote(owner, 1, false, "replacement".into(), 4)
            .is_err());
        assert!(client
            .prepare_confirm(owner, &id(), "original-selection", 4)
            .is_err());
        assert!(client.close_view(owner));
        let query = client.prepare_query(owner, &id(), 4).unwrap();
        client.enter(&query, 5).unwrap();
        client
            .receive(
                owner,
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
        assert_eq!(
            client.model().phase,
            HuntingRepairPhase::Pending,
            "known immutable receipt beats later notSubmitted"
        );
    }

    #[test]
    fn complete_source_needs_exact_sealed_apply_and_rejects_stale_completion() {
        let (mut client, owner, dispatch) = confirming();
        client
            .receive(
                owner,
                &raw(
                    &dispatch,
                    wire::Reply::Committed {
                        receipt: receipt(),
                        replayed: false,
                        reload_required: false,
                    },
                    Some(json!({"synthetic":"owner"})),
                ),
            )
            .unwrap();
        assert_eq!(client.model().phase, HuntingRepairPhase::Pending);
        assert!(!client.applied(owner, dispatch.request.request_id.get() + 1, &id()));
        let query = client.prepare_query(owner, &id(), 4).unwrap();
        assert!(!client.applied(owner, dispatch.request.request_id.get(), &id()));
        client.enter(&query, 5).unwrap();
        client
            .receive(
                owner,
                &raw(
                    &query,
                    wire::Reply::Committed {
                        receipt: receipt(),
                        replayed: true,
                        reload_required: false,
                    },
                    Some(json!({"synthetic":"owner"})),
                ),
            )
            .unwrap();
        assert!(client.applied(owner, query.request.request_id.get(), &id()));
        assert!(!client.applied(owner, query.request.request_id.get(), &id()));
        assert_eq!(client.model().phase, HuntingRepairPhase::Committed);
    }

    #[test]
    fn malformed_projection_is_not_an_apply_witness() {
        let (mut client, owner, dispatch) = confirming();
        let received = client
            .receive(
                owner,
                &raw(
                    &dispatch,
                    wire::Reply::Committed {
                        receipt: receipt(),
                        replayed: false,
                        reload_required: false,
                    },
                    Some(json!(false)),
                ),
            )
            .unwrap();
        assert_eq!(
            received.frame.snapshot,
            Some(json!(false)),
            "DTO only; host projection must refuse this source"
        );
        assert_eq!(client.model().phase, HuntingRepairPhase::Pending);
        assert!(client.prepare_query(owner, &id(), 4).is_ok());
    }

    #[test]
    fn timeout_after_enter_allows_only_query_and_late_raw_cannot_settle() {
        let (mut client, owner, dispatch) = confirming();
        assert!(!client.check_timeout(10_001));
        assert!(client.check_timeout(10_002));
        assert_eq!(client.model().phase, HuntingRepairPhase::Pending);
        assert!(client
            .receive(
                owner,
                &raw(&dispatch, wire::Reply::Pending { quote_id: id() }, None)
            )
            .is_err());
        assert!(client
            .prepare_confirm(owner, &id(), "original-selection", 10_003)
            .is_err());
        assert!(client.prepare_query(owner, &id(), 10_003).is_ok());
    }

    #[test]
    fn same_actor_rebind_keeps_original_pending_but_retires_old_raw_and_apply() {
        let (mut client, owner, dispatch) = confirming();
        let newer = owners().1;
        client.bind(newer, actor()).unwrap();
        assert_eq!(client.model().phase, HuntingRepairPhase::Pending);
        assert!(client
            .receive(
                owner,
                &raw(&dispatch, wire::Reply::Pending { quote_id: id() }, None)
            )
            .is_err());
        assert!(!client.applied(owner, dispatch.request.request_id.get(), &id()));
        assert!(client.prepare_query(newer, &"cd".repeat(32), 4).is_err());
        let query = client.prepare_query(newer, &id(), 4).unwrap();
        assert!(query.request.request_id.get() > dispatch.request.request_id.get());
        client.enter(&query, 5).unwrap();
    }

    #[test]
    fn other_actor_never_inherits_pending_and_returning_actor_can_query_it() {
        let (mut client, _, _) = confirming();
        let newer = owners().1;
        let mut other = actor();
        other.account_id = "other-synthetic-player".into();
        client.bind(newer, other).unwrap();
        assert_eq!(client.model().phase, HuntingRepairPhase::Idle);
        assert!(client.prepare_query(newer, &id(), 4).is_err());
        client.bind(newer, actor()).unwrap();
        assert_eq!(client.model().phase, HuntingRepairPhase::Pending);
        assert!(client.prepare_query(newer, &id(), 4).is_ok());
    }

    #[test]
    fn reload_requires_new_binding_and_preserves_receipt_for_query() {
        let (mut client, owner, dispatch) = confirming();
        client
            .receive(
                owner,
                &raw(
                    &dispatch,
                    wire::Reply::Committed {
                        receipt: receipt(),
                        replayed: false,
                        reload_required: true,
                    },
                    None,
                ),
            )
            .unwrap();
        assert!(client.model().reload_required);
        assert!(client.prepare_query(owner, &id(), 4).is_err());
        assert!(!client.applied(owner, dispatch.request.request_id.get(), &id()));
        let newer = owners().1;
        client.bind(newer, actor()).unwrap();
        assert!(!client.model().reload_required);
        assert!(client.prepare_query(newer, &id(), 4).is_ok());
    }

    #[test]
    fn not_submitted_query_without_known_receipt_releases_pending_but_never_reconfirms() {
        let (mut client, owner, _) = confirming();
        client.check_timeout(10_002);
        let query = client.prepare_query(owner, &id(), 10_003).unwrap();
        client.enter(&query, 10_004).unwrap();
        client
            .receive(
                owner,
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
        assert_eq!(client.model().phase, HuntingRepairPhase::Rejected);
        assert!(client
            .prepare_confirm(owner, &id(), "original-selection", 10_005)
            .is_err());
        assert!(client
            .prepare_quote(owner, 5, false, "fresh-selection".into(), 10_005)
            .is_ok());
    }

    #[test]
    fn prepared_body_tamper_close_and_invalid_clock_are_definitely_unsent() {
        let (mut client, owner) = quoted();
        let mut dispatch = client
            .prepare_confirm(owner, &id(), "original-selection", 2)
            .unwrap();
        dispatch.body.push(' ');
        assert!(client.enter(&dispatch, 3).is_err());
        assert!(client.close_view(owner));
        assert!(client.enter(&dispatch, 3).is_err());
        assert_eq!(client.model().phase, HuntingRepairPhase::Idle);
        assert!(client
            .prepare_quote(owner, 1, false, "selection".into(), u64::MAX)
            .is_err());
        client.withdraw();
        assert_eq!(client.model().mode, HuntingRepairMode::Unknown);
        assert!(client
            .prepare_quote(owner, 1, false, "selection".into(), 4)
            .is_err());
    }

    #[test]
    fn prepared_control_has_no_receipt_authority_and_same_binding_is_idempotent() {
        let (mut client, owner) = quoted();
        let dispatch = client
            .prepare_confirm(owner, &id(), "original-selection", 2)
            .unwrap();
        let response = raw(&dispatch, wire::Reply::Pending { quote_id: id() }, None);
        assert!(client.receive(owner, &response).is_err());
        client.bind(owner, actor()).unwrap();
        assert!(client.enter(&dispatch, 3).is_ok());
        assert!(client.receive(owner, &response).is_ok());
        let mut invalid = actor();
        invalid.realm.clear();
        assert!(client.bind(owner, invalid).is_err());
        assert_eq!(client.model().mode, HuntingRepairMode::Unknown);
        client.bind(owner, actor()).unwrap();
        assert_eq!(client.model().phase, HuntingRepairPhase::Pending);
    }

    #[test]
    fn immutable_receipt_conflict_does_not_consume_original_query() {
        let (mut client, owner, dispatch) = confirming();
        client
            .receive(
                owner,
                &raw(
                    &dispatch,
                    wire::Reply::Committed {
                        receipt: receipt(),
                        replayed: false,
                        reload_required: false,
                    },
                    None,
                ),
            )
            .unwrap();
        let query = client.prepare_query(owner, &id(), 4).unwrap();
        client.enter(&query, 5).unwrap();
        let mut conflicting = receipt();
        conflicting.operation_key = "different-operation".into();
        assert!(client
            .receive(
                owner,
                &raw(
                    &query,
                    wire::Reply::Committed {
                        receipt: conflicting,
                        replayed: true,
                        reload_required: false
                    },
                    None
                )
            )
            .is_err());
        assert!(client
            .receive(
                owner,
                &raw(
                    &query,
                    wire::Reply::Committed {
                        receipt: receipt(),
                        replayed: true,
                        reload_required: false
                    },
                    None
                )
            )
            .is_ok());
        assert_eq!(client.model().phase, HuntingRepairPhase::Pending);
    }

    #[test]
    fn exhausted_request_ids_and_actor_capacity_fail_closed() {
        let (mut client, owner) = quoted();
        client.last_request_id = u64::MAX;
        assert!(client
            .prepare_confirm(owner, &id(), "original-selection", 2)
            .is_err());
        assert!(client.control.is_none());
        for index in 1..MAX_ACTORS {
            let mut other = actor();
            other.character_index = i32::try_from(index + 1).unwrap();
            client.bind(owner, other).unwrap();
        }
        let mut extra = actor();
        extra.character_index = 1000;
        assert!(client.bind(owner, extra).is_err());
        assert_eq!(client.model().mode, HuntingRepairMode::Unknown);
    }
}
