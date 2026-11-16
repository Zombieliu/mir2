//! Private relay state-machine boundaries. No listener or production credential.
//! Register as a direct child of `companion_relay`, beside its existing tests.
use super::*;
use crate::companion_supply::{SupplyError, SupplyGrant, SupplyLedger, SupplyPoint};
use mir2_simulation::npc_purchase_journal::{NpcPurchaseIntent, NpcPurchaseOperation};
use mir2_simulation::{
    NpcPurchaseCurrency, NpcPurchaseProcessingOutcome, NpcPurchaseRejection, NpcPurchaseRequest,
    NpcPurchaseSource,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);
const PUBLIC_FIXTURE_KEY: [u8; 32] = [0x6c; 32];

struct OwnedFiles(PathBuf);
impl Drop for OwnedFiles {
    fn drop(&mut self) {
        // Only direct ordinary files inside this exact fresh directory; no
        // recursion, path following, repository cleanup or external state.
        if let Ok(entries) = fs::read_dir(&self.0) {
            for entry in entries.flatten() {
                if entry.path().parent() == Some(self.0.as_path())
                    && entry.file_type().is_ok_and(|kind| kind.is_file())
                {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
        let _ = fs::remove_dir(&self.0);
    }
}

fn character(account: &str, index: i32) -> CharacterBinding {
    CharacterBinding {
        account_id: account.into(),
        character_index: index,
    }
}
fn policy() -> CompanionPolicy {
    CompanionPolicy {
        schema_version: 1,
        realm_id: "relay-supply-test".into(),
        world_epoch: "relay-supply-epoch".into(),
        enrollments: vec![Enrollment {
            resident_id: "alo".into(),
            archive_id: "alo-archive".into(),
            display_name: "Alo".into(),
            owner: character("human", 1),
            actor: character("actor", 2),
        }],
    }
}
fn admission(id: &str, account: &str, index: i32) -> Admission {
    Admission {
        id: id.into(),
        character: character(account, index),
        identity_session_id: format!("identity-{id}"),
        expires_at_ms: 1_000_000,
    }
}
fn authorize_request(id: &str) -> OwnerRequest {
    OwnerRequest {
        request_id: id.into(),
        resident_id: Some("alo".into()),
        operation: Operation::AuthorizeSupply,
        payload: json!({"expectedPolicyVersion":1,"budgetGold":50,"quantity":1,"expiresInSeconds":300}),
    }
}
fn cancellation(id: &str, operation: Operation) -> OwnerRequest {
    OwnerRequest {
        request_id: id.into(),
        resident_id: Some("alo".into()),
        operation,
        payload: json!({}),
    }
}
fn anchor(x: i32, y: i32) -> Option<SupplyAnchor> {
    Some(SupplyAnchor {
        map_file_name: "0".into(),
        point: SupplyPoint { x, y },
    })
}
fn attach(state: &mut RelayState, value: Admission) -> mpsc::Receiver<RelayDelivery> {
    let (sender, receiver) = mpsc::channel(OUTPUT_CAPACITY);
    state.handle(Command::Attach(value, sender), 100).unwrap();
    receiver
}
fn register(state: &mut RelayState, id: &str, receiver: &mut mpsc::Receiver<RelayDelivery>) {
    state
        .handle(
            Command::Register(
                id.into(),
                RuntimeRegister {
                    request_id: "runtime-register".into(),
                    resident_id: "alo".into(),
                    archive_id: "alo-archive".into(),
                },
            ),
            100,
        )
        .unwrap();
    assert_eq!(
        receiver.try_recv().unwrap().event["payload"]["status"],
        "ok"
    );
}
struct Fixture {
    state: RelayState,
    owner: mpsc::Receiver<RelayDelivery>,
    actor: mpsc::Receiver<RelayDelivery>,
    ledger: Arc<Mutex<SupplyLedger>>,
    path: PathBuf,
    _files: OwnedFiles,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().canonicalize().unwrap();
        let suffix = DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = root.join(format!(
            "mir2-relay-supply-{}-{nanos}-{suffix}",
            std::process::id()
        ));
        assert!(directory.starts_with(&root));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("supply.json");
        let configured = policy();
        let ledger = Arc::new(Mutex::new(
            SupplyLedger::open(
                path.clone(),
                PUBLIC_FIXTURE_KEY,
                configured.realm_id.clone(),
                configured.world_epoch.clone(),
            )
            .unwrap(),
        ));
        let mut state = RelayState::new(Arc::new(configured));
        state.supplies = Some(ledger.clone());
        let owner = attach(&mut state, admission("owner-admission", "human", 1));
        let mut actor = attach(&mut state, admission("actor-admission", "actor", 2));
        register(&mut state, "actor-admission", &mut actor);
        Self {
            state,
            owner,
            actor,
            ledger,
            path,
            _files: OwnedFiles(directory),
        }
    }
    fn control(&mut self, request: OwnerRequest, at: Option<SupplyAnchor>, now: u64) {
        self.state
            .handle(
                Command::ControlAt("owner-admission".into(), request, at),
                now,
            )
            .unwrap();
    }
    fn grant(&mut self, request_id: &str, now: u64) -> (SupplyGrant, RelayDelivery) {
        self.control(authorize_request(request_id), anchor(300, 620), now);
        let delivery = self.actor.try_recv().unwrap();
        let grant =
            serde_json::from_value(delivery.event["payload"]["payload"]["grant"].clone()).unwrap();
        (grant, delivery)
    }
    fn finish(&mut self, delivery: &RelayDelivery, now: u64) {
        let reply = self.reply(
            delivery,
            ReplyStatus::Ok,
            Self::accepted_view(delivery),
            now,
        );
        assert_eq!(reply["status"], "ok");
    }
    fn accepted_view(delivery: &RelayDelivery) -> Value {
        json!({
            "profile":{"policyVersion":delivery.event["payload"]["payload"]["expectedPolicyVersion"]},
            "runtime":{"supplyTask":{
                "status":"authorized",
                "permitId":delivery.event["payload"]["payload"]["grant"]["permitId"]
            }}
        })
    }
    fn reply(
        &mut self,
        delivery: &RelayDelivery,
        status: ReplyStatus,
        result: Value,
        now: u64,
    ) -> Value {
        self.state
            .handle(
                Command::Reply(
                    "actor-admission".into(),
                    RuntimeReply {
                        request_id: delivery.event["payload"]["requestId"]
                            .as_str()
                            .unwrap()
                            .into(),
                        generation: delivery.event["payload"]["generation"].as_u64().unwrap(),
                        status,
                        result,
                    },
                ),
                now,
            )
            .unwrap();
        self.owner.try_recv().unwrap().event["payload"].clone()
    }
    fn activate(&mut self, request_id: &str, now: u64) -> SupplyGrant {
        let (grant, delivery) = self.grant(request_id, now);
        self.finish(&delivery, now + 1);
        assert_eq!(self.active(now + 1).unwrap(), Some(grant.clone()));
        grant
    }
    fn reset_relay_state(&mut self) {
        // Exercise loss of the relay's volatile fingerprint cache while keeping
        // the independent trusted ledger intact. This is not a process-restart
        // claim; the ledger's own reopen tests establish persistence separately.
        let mut state = RelayState::new(Arc::new(policy()));
        state.supplies = Some(self.ledger.clone());
        let owner = attach(&mut state, admission("owner-admission", "human", 1));
        let mut actor = attach(&mut state, admission("actor-admission", "actor", 2));
        register(&mut state, "actor-admission", &mut actor);
        self.state = state;
        self.owner = owner;
        self.actor = actor;
    }
    fn active(&self, now: u64) -> Result<Option<SupplyGrant>, SupplyError> {
        self.ledger
            .lock()
            .unwrap()
            .active_grant(&character("actor", 2), "actor-admission", now)
    }
}

#[test]
fn companion_supply_relay_refuses_owner_grant_anchor_and_hard_limit_injection() {
    let mut fixture = Fixture::new();
    let valid = authorize_request("authorize");
    for (index, (key, value)) in [
        ("grant", json!({"permitId":"client-minted","maxGold":500})),
        ("returnAnchor", json!({"x":288,"y":608})),
        ("ownerRef", json!("forged-owner")),
        ("npcIndex", json!(9)),
        ("maxGold", json!(500)),
    ]
    .into_iter()
    .enumerate()
    {
        let mut request = valid.clone();
        request.request_id = format!("injected-{index}");
        request.payload[key] = value;
        fixture.control(request, anchor(300, 620), 101 + index as u64);
        assert_eq!(
            fixture.owner.try_recv().unwrap().event["payload"]["status"],
            "invalid"
        );
        assert!(fixture.actor.try_recv().is_err());
    }
    assert!(fixture
        .ledger
        .lock()
        .unwrap()
        .grant_for_actor(&character("actor", 2))
        .unwrap()
        .is_none());
    assert!(fixture.state.pending.is_empty() && fixture.state.fingerprints.is_empty());
}

#[test]
fn companion_supply_relay_requires_a_trusted_bichon_anchor_before_creating_a_grant() {
    let mut fixture = Fixture::new();
    let wrong_map = Some(SupplyAnchor {
        map_file_name: "0141".into(),
        point: SupplyPoint { x: 300, y: 620 },
    });
    for (index, supplied) in [None, wrong_map, anchor(311, 620)].into_iter().enumerate() {
        fixture.control(
            authorize_request(&format!("anchor-{index}")),
            supplied,
            101 + index as u64,
        );
        assert_eq!(
            fixture.owner.try_recv().unwrap().event["payload"]["status"],
            "denied"
        );
        assert!(fixture.actor.try_recv().is_err());
        assert!(fixture
            .ledger
            .lock()
            .unwrap()
            .grant_for_actor(&character("actor", 2))
            .unwrap()
            .is_none());
    }
}

#[test]
fn companion_supply_relay_injects_only_the_server_grant_and_duplicate_cannot_refresh_it() {
    let mut fixture = Fixture::new();
    let (grant, first) = fixture.grant("same-request", 101);
    assert_eq!(grant.return_anchor, SupplyPoint { x: 300, y: 620 });
    assert_eq!(grant.npc_position, SupplyPoint { x: 288, y: 608 });
    assert_eq!(
        (
            grant.npc_index,
            grant.item_index,
            grant.quantity,
            grant.max_gold
        ),
        (9, 658, 1, 50)
    );
    assert_eq!(grant.expires_at_ms, 300_101);
    assert_eq!(first.event["payload"]["ownerRef"], owner_ref("human"));
    assert_eq!(
        first.event["payload"]["payload"].as_object().unwrap().len(),
        5
    );
    assert!(fixture.active(101).unwrap().is_none(), "A prepared draft is not a spending grant before the runtime confirms the exact version and permit.");
    fixture.finish(&first, 102);
    let before = fs::read(&fixture.path).unwrap();
    fixture.control(authorize_request("same-request"), anchor(280, 610), 2000);
    let duplicate = fixture.actor.try_recv().unwrap();
    let duplicate_grant: SupplyGrant =
        serde_json::from_value(duplicate.event["payload"]["payload"]["grant"].clone()).unwrap();
    assert_eq!(duplicate_grant, grant);
    assert_eq!(fs::read(&fixture.path).unwrap(), before);
    assert_eq!(fixture.active(2001).unwrap(), Some(grant));
}

#[test]
fn companion_supply_relay_denies_other_account_character_and_actor_owners_without_side_effects() {
    let mut fixture = Fixture::new();
    for (index, account, character_index) in [(0, "outsider", 1), (1, "human", 2), (2, "actor", 2)]
    {
        let id = format!("unqualified-{index}");
        let mut receiver = attach(&mut fixture.state, admission(&id, account, character_index));
        fixture
            .state
            .handle(
                Command::ControlAt(
                    id,
                    authorize_request(&format!("wrong-owner-{index}")),
                    anchor(300, 620),
                ),
                101,
            )
            .unwrap();
        assert_eq!(
            receiver.try_recv().unwrap().event["payload"]["status"],
            "denied"
        );
        assert!(fixture.actor.try_recv().is_err());
    }
    assert!(fixture
        .ledger
        .lock()
        .unwrap()
        .grant_for_actor(&character("actor", 2))
        .unwrap()
        .is_none());
}

#[test]
fn companion_supply_relay_actor_readmission_does_not_inherit_the_old_spending_admission() {
    let mut fixture = Fixture::new();
    let (grant, delivery) = fixture.grant("first-admission", 101);
    fixture.finish(&delivery, 102);
    fixture
        .state
        .handle(Command::Detach("actor-admission".into()), 103)
        .unwrap();
    let mut replacement = attach(
        &mut fixture.state,
        admission("actor-replacement", "actor", 2),
    );
    register(&mut fixture.state, "actor-replacement", &mut replacement);
    assert_eq!(
        fixture
            .ledger
            .lock()
            .unwrap()
            .active_grant(&character("actor", 2), "actor-replacement", 104)
            .err(),
        Some(SupplyError::AdmissionChanged)
    );
    fixture.control(authorize_request("first-admission"), anchor(280, 610), 105);
    let repeated = replacement.try_recv().unwrap();
    let repeated_grant: SupplyGrant =
        serde_json::from_value(repeated.event["payload"]["payload"]["grant"].clone()).unwrap();
    assert_eq!(repeated_grant, grant);
    assert_eq!(
        fixture
            .ledger
            .lock()
            .unwrap()
            .active_grant(&character("actor", 2), "actor-replacement", 106)
            .err(),
        Some(SupplyError::AdmissionChanged)
    );
}

#[test]
fn companion_supply_relay_cancel_and_recall_revoke_even_when_the_actor_is_offline() {
    for operation in [Operation::CancelSupply, Operation::Recall] {
        let mut fixture = Fixture::new();
        let (_, delivery) = fixture.grant("offline-grant", 101);
        fixture.finish(&delivery, 102);
        fixture
            .state
            .handle(Command::Detach("actor-admission".into()), 103)
            .unwrap();
        fixture.control(cancellation("offline-revoke", operation), None, 104);
        assert_eq!(fixture.active(105).err(), Some(SupplyError::Revoked));
        assert_eq!(
            fixture.owner.try_recv().unwrap().event["payload"]["status"],
            "offline"
        );
        assert!(fixture.state.pending.is_empty());
    }
}

#[test]
fn companion_supply_relay_conflicting_request_id_cannot_revoke_online_pending_or_offline_grants() {
    for operation in [Operation::CancelSupply, Operation::Recall] {
        for offline in [false, true] {
            let mut fixture = Fixture::new();
            let grant = fixture.activate("already-active-a", 101);
            let (_draft, delivery) = fixture.grant("shared-request-id", 103);
            assert_eq!(fixture.active(103).unwrap(), Some(grant.clone()));
            if offline {
                fixture
                    .state
                    .handle(Command::Detach("actor-admission".into()), 104)
                    .unwrap();
                assert_eq!(
                    fixture.owner.try_recv().unwrap().event["payload"]["status"],
                    "unknown"
                );
            }
            let before = fs::read(&fixture.path).unwrap();
            fixture.control(cancellation("shared-request-id", operation), None, 105);
            assert_eq!(
                fixture.owner.try_recv().unwrap().event["payload"]["status"],
                "conflict"
            );
            assert_eq!(fixture.active(106).unwrap(), Some(grant));
            assert_eq!(
                fs::read(&fixture.path).unwrap(),
                before,
                "A conflicting wire request cannot mutate the private grant journal."
            );
            assert!(fixture.actor.try_recv().is_err());
            assert!(!offline || !fixture.state.delivery_current(&delivery, 106));
        }
    }
}

#[test]
fn companion_supply_relay_changed_authorization_payload_cannot_replace_or_extend_a_grant() {
    let mut fixture = Fixture::new();
    let (grant, delivery) = fixture.grant("conflicting-authorization", 101);
    fixture.finish(&delivery, 102);
    let before = fs::read(&fixture.path).unwrap();
    for (index, payload) in [
        json!({"expectedPolicyVersion":1,"budgetGold":39,"quantity":1,"expiresInSeconds":300}),
        json!({"expectedPolicyVersion":2,"budgetGold":50,"quantity":1,"expiresInSeconds":300}),
    ]
    .into_iter()
    .enumerate()
    {
        let mut request = authorize_request("conflicting-authorization");
        request.payload = payload;
        fixture.control(request, anchor(280, 610), 103 + index as u64);
        assert_eq!(
            fixture.owner.try_recv().unwrap().event["payload"]["status"],
            "conflict"
        );
        assert_eq!(fixture.active(105).unwrap(), Some(grant.clone()));
        assert_eq!(fs::read(&fixture.path).unwrap(), before);
        assert!(fixture.actor.try_recv().is_err());
    }
}

#[test]
fn companion_supply_relay_conflict_error_and_unknown_runtime_replies_abandon_but_preserve_active_a()
{
    for status in [
        ReplyStatus::Conflict,
        ReplyStatus::Error,
        ReplyStatus::Unknown,
    ] {
        let mut fixture = Fixture::new();
        let original = fixture.activate("active-a", 101);
        let (draft, delivery) = fixture.grant("candidate-b", 103);
        assert_ne!(draft.permit_id, original.permit_id);
        assert_eq!(fixture.active(103).unwrap(), Some(original.clone()));
        let reply = fixture.reply(
            &delivery,
            status,
            json!({"profile":{"policyVersion":2}}),
            104,
        );
        assert_eq!(reply["status"], serde_json::to_value(status).unwrap());
        assert_eq!(fixture.active(105).unwrap(), Some(original));
        assert!(fixture.state.pending.is_empty());
        // Abandoning a failed draft must not seal unrelated future permission.
        let replacement = fixture.activate("fresh-c", 106);
        assert_ne!(replacement.permit_id, draft.permit_id);
    }
}

#[test]
fn companion_supply_relay_runtime_ok_requires_the_exact_policy_version_permit_and_authorized_state()
{
    for changed in ["policyVersion", "permitId", "status", "missingOwnerView"] {
        let mut fixture = Fixture::new();
        let original = fixture.activate("active-a", 101);
        let (_draft, delivery) = fixture.grant("candidate-b", 103);
        let mut result = Fixture::accepted_view(&delivery);
        match changed {
            "policyVersion" => result["profile"]["policyVersion"] = json!(2),
            "permitId" => result["runtime"]["supplyTask"]["permitId"] = json!("another-permit"),
            "status" => result["runtime"]["supplyTask"]["status"] = json!("running"),
            "missingOwnerView" => result = json!({}),
            _ => unreachable!(),
        }
        assert_eq!(
            fixture.reply(&delivery, ReplyStatus::Ok, result, 104)["status"],
            "error"
        );
        assert_eq!(fixture.active(105).unwrap(), Some(original));
        assert!(fixture.state.pending.is_empty());
    }
}

#[test]
fn companion_supply_relay_timeout_and_late_runtime_ok_cannot_replace_active_a() {
    let mut fixture = Fixture::new();
    let original = fixture.activate("active-a", 101);
    let (_draft, delivery) = fixture.grant("timed-out-b", 103);
    assert_eq!(fixture.active(104).unwrap(), Some(original.clone()));
    fixture.state.expire(20_000);
    assert_eq!(
        fixture.owner.try_recv().unwrap().event["payload"]["status"],
        "unknown"
    );
    assert_eq!(fixture.active(20_001).unwrap(), Some(original.clone()));
    assert!(!fixture.state.delivery_current(&delivery, 20_001));
    fixture
        .state
        .handle(
            Command::Reply(
                "actor-admission".into(),
                RuntimeReply {
                    request_id: delivery.event["payload"]["requestId"]
                        .as_str()
                        .unwrap()
                        .into(),
                    generation: delivery.event["payload"]["generation"].as_u64().unwrap(),
                    status: ReplyStatus::Ok,
                    result: Fixture::accepted_view(&delivery),
                },
            ),
            20_002,
        )
        .unwrap();
    assert!(fixture.owner.try_recv().is_err());
    assert_eq!(fixture.active(20_003).unwrap(), Some(original));
}

#[test]
fn companion_supply_relay_offline_before_or_after_dispatch_preserves_active_a() {
    for offline_before_dispatch in [true, false] {
        let mut fixture = Fixture::new();
        let original = fixture.activate("active-a", 101);
        if offline_before_dispatch {
            fixture
                .state
                .handle(Command::Detach("actor-admission".into()), 103)
                .unwrap();
            fixture.control(authorize_request("offline-b"), anchor(280, 610), 104);
            assert_eq!(
                fixture.owner.try_recv().unwrap().event["payload"]["status"],
                "offline"
            );
        } else {
            let (_draft, delivery) = fixture.grant("offline-b", 103);
            fixture
                .state
                .handle(Command::Detach("actor-admission".into()), 104)
                .unwrap();
            assert_eq!(
                fixture.owner.try_recv().unwrap().event["payload"]["status"],
                "unknown"
            );
            assert!(!fixture.state.delivery_current(&delivery, 105));
        }
        assert_eq!(fixture.active(106).unwrap(), Some(original));
        assert!(fixture.state.pending.is_empty());
    }
}

#[test]
fn companion_supply_relay_owner_disconnect_abandons_pending_b_without_replacing_active_a() {
    let mut fixture = Fixture::new();
    let original = fixture.activate("active-a", 101);
    let (_draft, delivery) = fixture.grant("pending-b", 103);
    fixture
        .state
        .handle(Command::Detach("owner-admission".into()), 104)
        .unwrap();
    assert!(fixture.owner.try_recv().is_err());
    assert!(fixture.state.pending.is_empty());
    assert!(!fixture.state.delivery_current(&delivery, 105));
    assert_eq!(fixture.active(105).unwrap(), Some(original));
}

#[test]
fn companion_supply_relay_disconnected_draft_cannot_activate_after_owner_reconnect_and_id_retry() {
    let mut fixture = Fixture::new();
    let original = fixture.activate("active-a", 101);
    let (draft, delivery) = fixture.grant("disconnected-b", 103);
    fixture
        .state
        .handle(Command::Detach("owner-admission".into()), 104)
        .unwrap();
    assert!(fixture.state.pending.is_empty());
    assert!(!fixture.state.delivery_current(&delivery, 105));

    // Reconnect the same authenticated character through a new synthetic
    // admission. Durable owner request IDs are scoped to the character, not
    // this transient connection, so this retries the exact original draft.
    let (sender, receiver) = mpsc::channel(OUTPUT_CAPACITY);
    fixture
        .state
        .handle(
            Command::Attach(admission("owner-reconnected", "human", 1), sender),
            105,
        )
        .unwrap();
    fixture.owner = receiver;
    fixture
        .state
        .handle(
            Command::Reply(
                "actor-admission".into(),
                RuntimeReply {
                    request_id: delivery.event["payload"]["requestId"]
                        .as_str()
                        .unwrap()
                        .into(),
                    generation: delivery.event["payload"]["generation"].as_u64().unwrap(),
                    status: ReplyStatus::Ok,
                    result: Fixture::accepted_view(&delivery),
                },
            ),
            106,
        )
        .unwrap();
    assert!(
        fixture.owner.try_recv().is_err(),
        "A late old ACK has no pending request."
    );
    let sealed = fs::read(&fixture.path).unwrap();
    fixture
        .state
        .handle(
            Command::ControlAt(
                "owner-reconnected".into(),
                authorize_request("disconnected-b"),
                anchor(280, 610),
            ),
            107,
        )
        .unwrap();
    let repeated = fixture.actor.try_recv().unwrap();
    let repeated_grant: SupplyGrant =
        serde_json::from_value(repeated.event["payload"]["payload"]["grant"].clone()).unwrap();
    assert_eq!(
        repeated_grant, draft,
        "Retry must not mint or refresh a grant."
    );
    assert_eq!(
        fixture.reply(
            &repeated,
            ReplyStatus::Ok,
            Fixture::accepted_view(&repeated),
            108
        )["status"],
        "error",
        "A reconnected owner's retry cannot turn the disconnected draft into spending authority."
    );
    assert_eq!(fixture.active(109).unwrap(), Some(original));
    assert_eq!(fs::read(&fixture.path).unwrap(), sealed);
    assert_eq!(
        fixture
            .ledger
            .lock()
            .unwrap()
            .activate(
                &namespaced_request_id(&policy(), &character("human", 1), "disconnected-b"),
                &character("actor", 2),
                "actor-admission",
                &draft.permit_id,
                109,
            )
            .err(),
        Some(SupplyError::Revoked),
        "The disconnect must durably revoke the draft, rather than only remove relay pending state."
    );
}

#[test]
fn companion_supply_relay_activation_error_seals_draft_after_original_outcome_is_reconciled() {
    let mut fixture = Fixture::new();
    let original = fixture.activate("active-a", 101);
    let (draft, delivery) = fixture.grant("blocked-b", 103);

    // A public synthetic operation models A reserving its one ordinary
    // purchase while B's runtime acknowledgment is in flight. This exercises
    // the real ledger's Unresolved guard; no game purchase or outcome is run.
    let operation = NpcPurchaseOperation {
        actor: [11; 32],
        request_scope: [22; 32],
        sequence: 1,
        intent: NpcPurchaseIntent {
            request: NpcPurchaseRequest {
                item_index: 658,
                count: 1,
                panel_type: 0,
            },
            currency: NpcPurchaseCurrency::Gold,
            source: NpcPurchaseSource::Trade,
            service_catalog_proof: [33; 32],
        },
    };
    let request_key = namespaced_request_id(&policy(), &character("human", 1), "blocked-b");
    {
        let mut ledger = fixture.ledger.lock().unwrap();
        let reservation = ledger
            .reserve(&character("actor", 2), "actor-admission", operation, 104)
            .unwrap();
        assert_eq!(reservation.grant, original);
        assert!(!reservation.already_reserved);
        assert_eq!(
            ledger
                .activate(
                    &request_key,
                    &character("actor", 2),
                    "actor-admission",
                    &draft.permit_id,
                    104
                )
                .err(),
            Some(SupplyError::Unresolved)
        );
    }
    assert_eq!(
        fixture.reply(
            &delivery,
            ReplyStatus::Ok,
            Fixture::accepted_view(&delivery),
            105
        )["status"],
        "error"
    );
    assert!(fixture.state.pending.is_empty());
    assert_eq!(fixture.active(105).unwrap(), Some(original.clone()));

    // Make the transient guard disappear using an explicitly synthetic
    // terminal rejection, not a fabricated successful gameplay receipt.
    fixture
        .ledger
        .lock()
        .unwrap()
        .resolve(
            &character("actor", 2),
            operation,
            &NpcPurchaseProcessingOutcome::Rejected {
                request: operation.intent.request,
                reason: NpcPurchaseRejection::InvalidDelivery,
            },
        )
        .unwrap();
    let sealed = fs::read(&fixture.path).unwrap();
    let (repeated_grant, repeated) = fixture.grant("blocked-b", 107);
    assert_eq!(repeated_grant, draft);
    assert_eq!(
        fixture.reply(
            &repeated,
            ReplyStatus::Ok,
            Fixture::accepted_view(&repeated),
            108
        )["status"],
        "error",
        "A former activation error cannot become successful after its transient guard clears."
    );
    assert_eq!(fixture.active(109).unwrap(), Some(original));
    assert_eq!(fs::read(&fixture.path).unwrap(), sealed);
    assert_eq!(
        fixture
            .ledger
            .lock()
            .unwrap()
            .activate(
                &request_key,
                &character("actor", 2),
                "actor-admission",
                &draft.permit_id,
                109,
            )
            .err(),
        Some(SupplyError::Revoked)
    );
    let fresh = fixture.activate("fresh-c", 110);
    assert_ne!(
        fresh.permit_id, draft.permit_id,
        "Only a new owner decision grants a new budget."
    );
}

#[test]
fn companion_supply_relay_only_matching_runtime_ok_activates_b_and_replaces_a() {
    let mut fixture = Fixture::new();
    let original = fixture.activate("active-a", 101);
    fixture.control(authorize_request("candidate-b"), anchor(280, 610), 103);
    let delivery = fixture.actor.try_recv().unwrap();
    let replacement: SupplyGrant =
        serde_json::from_value(delivery.event["payload"]["payload"]["grant"].clone()).unwrap();
    assert_ne!(replacement.permit_id, original.permit_id);
    assert_eq!(replacement.return_anchor, SupplyPoint { x: 280, y: 610 });
    assert_eq!(fixture.active(103).unwrap(), Some(original));
    fixture.finish(&delivery, 104);
    assert_eq!(fixture.active(105).unwrap(), Some(replacement));
    assert!(fixture.state.pending.is_empty());
}

#[test]
fn companion_supply_relay_retrying_an_old_cancel_does_not_revoke_a_newly_activated_b() {
    for completed_cancel in [false, true] {
        let mut fixture = Fixture::new();
        fixture.activate("active-a", 101);
        fixture.control(cancellation("cancel-a", Operation::CancelSupply), None, 103);
        let cancel_delivery = fixture.actor.try_recv().unwrap();
        assert_eq!(fixture.active(103).err(), Some(SupplyError::Revoked));
        if completed_cancel {
            assert_eq!(
                fixture.reply(
                    &cancel_delivery,
                    ReplyStatus::Ok,
                    json!({"runtime":{"supplyTask":{"status":"cancelled"}}}),
                    104
                )["status"],
                "ok"
            );
        }
        let replacement = fixture.activate("active-b", 105);
        let before = fs::read(&fixture.path).unwrap();
        fixture.control(cancellation("cancel-a", Operation::CancelSupply), None, 107);
        assert_eq!(fixture.active(108).unwrap(), Some(replacement));
        assert_eq!(
            fs::read(&fixture.path).unwrap(),
            before,
            "A cancellation retry must retain its first target, never revoke a later grant."
        );
    }
}

#[test]
fn companion_supply_relay_offline_cancel_retry_keeps_its_original_target_after_actor_returns() {
    let mut fixture = Fixture::new();
    fixture.activate("active-a", 101);
    fixture
        .state
        .handle(Command::Detach("actor-admission".into()), 103)
        .unwrap();
    fixture.control(
        cancellation("offline-cancel-a", Operation::CancelSupply),
        None,
        104,
    );
    assert_eq!(
        fixture.owner.try_recv().unwrap().event["payload"]["status"],
        "offline"
    );
    assert_eq!(fixture.active(104).err(), Some(SupplyError::Revoked));
    // Reattach only a public synthetic admission; no production identity changes.
    let mut actor = attach(&mut fixture.state, admission("actor-admission", "actor", 2));
    register(&mut fixture.state, "actor-admission", &mut actor);
    fixture.actor = actor;
    let replacement = fixture.activate("active-b", 105);
    let before = fs::read(&fixture.path).unwrap();
    fixture.control(
        cancellation("offline-cancel-a", Operation::CancelSupply),
        None,
        107,
    );
    assert_eq!(fixture.active(108).unwrap(), Some(replacement));
    assert_eq!(fs::read(&fixture.path).unwrap(), before);
}

#[test]
fn companion_supply_relay_cancel_retry_after_volatile_state_reset_cannot_revoke_active_b() {
    let mut fixture = Fixture::new();
    fixture.activate("active-a", 101);
    fixture.control(cancellation("cancel-a", Operation::CancelSupply), None, 103);
    let cancel_delivery = fixture.actor.try_recv().unwrap();
    fixture.reply(&cancel_delivery, ReplyStatus::Ok, json!({}), 104);
    let replacement = fixture.activate("active-b", 105);
    fixture.reset_relay_state();
    let before = fs::read(&fixture.path).unwrap();
    fixture.control(cancellation("cancel-a", Operation::CancelSupply), None, 107);
    assert_eq!(fixture.active(108).unwrap(), Some(replacement));
    assert_eq!(fs::read(&fixture.path).unwrap(), before);
    assert!(
        fixture.actor.try_recv().is_err(),
        "A durable cancellation retry must not cancel the runner's new task either."
    );
}

#[test]
fn companion_supply_relay_cross_operation_id_reuse_after_state_reset_has_no_new_effect() {
    for originally_cancel in [false, true] {
        let mut fixture = Fixture::new();
        fixture.activate("shared-operation-id", 101);
        if originally_cancel {
            fixture.control(
                cancellation("original-cancel-id", Operation::CancelSupply),
                None,
                103,
            );
            let cancelled = fixture.actor.try_recv().unwrap();
            fixture.reply(&cancelled, ReplyStatus::Ok, json!({}), 104);
        }
        let replacement = fixture.activate("active-b", 105);
        fixture.reset_relay_state();
        let before = fs::read(&fixture.path).unwrap();
        if originally_cancel {
            fixture.control(
                authorize_request("original-cancel-id"),
                anchor(280, 610),
                107,
            );
        } else {
            fixture.control(
                cancellation("shared-operation-id", Operation::CancelSupply),
                None,
                107,
            );
        }
        assert_ne!(
            fixture.owner.try_recv().unwrap().event["payload"]["status"],
            "ok"
        );
        assert_eq!(fixture.active(108).unwrap(), Some(replacement));
        assert_eq!(fs::read(&fixture.path).unwrap(), before);
        assert!(fixture.actor.try_recv().is_err());
    }
}
