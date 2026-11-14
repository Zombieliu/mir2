//! Relay + real durable life journal, with public synthetic local identities.
//! No socket listener, game process, model or configured private key.
use super::*;
use crate::companion_supply::{
    LifeGrant, LifeScopeStatus, SupplyError, SupplyGrant, SupplyLedger, SupplyPoint,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
const PUBLIC_KEY: [u8; 32] = [0x72; 32];
struct OwnedDirectory(PathBuf);
impl Drop for OwnedDirectory {
    fn drop(&mut self) {
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
        realm_id: "relay-life:test".into(),
        world_epoch: "relay-life:epoch".into(),
        enrollments: vec![Enrollment {
            resident_id: "alo".into(),
            archive_id: "archive:alo".into(),
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
        identity_session_id: format!("identity:{id}"),
        expires_at_ms: 10_000_000,
    }
}
fn at(x: i32, y: i32) -> Option<SupplyAnchor> {
    Some(SupplyAnchor {
        map_file_name: "0".into(),
        point: SupplyPoint { x, y },
    })
}
fn request(id: &str, operation: Operation, payload: Value) -> OwnerRequest {
    OwnerRequest {
        request_id: id.into(),
        resident_id: Some("alo".into()),
        operation,
        payload,
    }
}
fn authorize(id: &str) -> OwnerRequest {
    request(
        id,
        Operation::AuthorizeLife,
        json!({"expectedPolicyVersion":1,"budgetGold":50,"durationSeconds":1200}),
    )
}
fn claim(id: &str, grant: &LifeGrant) -> OwnerRequest {
    request(
        id,
        Operation::RequestLifeSupply,
        json!({"lifePermitId":grant.life_permit_id}),
    )
}
fn attach(state: &mut RelayState, value: Admission, now: u64) -> mpsc::Receiver<RelayDelivery> {
    let (sender, receiver) = mpsc::channel(OUTPUT_CAPACITY);
    state.handle(Command::Attach(value, sender), now).unwrap();
    receiver
}
fn register(
    state: &mut RelayState,
    id: &str,
    receiver: &mut mpsc::Receiver<RelayDelivery>,
    now: u64,
) {
    state
        .handle(
            Command::Register(
                id.into(),
                RuntimeRegister {
                    request_id: "register:fixture".into(),
                    resident_id: "alo".into(),
                    archive_id: "archive:alo".into(),
                },
            ),
            now,
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
    owner_id: String,
    actor_id: String,
    path: PathBuf,
    _files: OwnedDirectory,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().canonicalize().unwrap();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = root.join(format!(
            "mir2-relay-life-{}-{nanos}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(directory.starts_with(&root));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("supply.json");
        let configured = policy();
        let ledger = Arc::new(Mutex::new(
            SupplyLedger::open(
                path.clone(),
                PUBLIC_KEY,
                configured.realm_id.clone(),
                configured.world_epoch.clone(),
            )
            .unwrap(),
        ));
        let mut state = RelayState::new(Arc::new(configured));
        state.supplies = Some(ledger.clone());
        let owner = attach(&mut state, admission("owner:a", "human", 1), 100);
        let mut actor = attach(&mut state, admission("actor:a", "actor", 2), 100);
        register(&mut state, "actor:a", &mut actor, 100);
        Self {
            state,
            owner,
            actor,
            ledger,
            owner_id: "owner:a".into(),
            actor_id: "actor:a".into(),
            path,
            _files: OwnedDirectory(directory),
        }
    }
    fn owner_control(&mut self, value: OwnerRequest, now: u64) {
        self.state
            .handle(Command::ControlAt(self.owner_id.clone(), value, None), now)
            .unwrap();
    }
    fn actor_control(&mut self, value: OwnerRequest, position: Option<SupplyAnchor>, now: u64) {
        self.state
            .handle(
                Command::ControlAt(self.actor_id.clone(), value, position),
                now,
            )
            .unwrap();
    }
    fn prepare(&mut self, id: &str, now: u64) -> (LifeGrant, RelayDelivery) {
        self.owner_control(authorize(id), now);
        let delivery = self.actor.try_recv().unwrap();
        let grant =
            serde_json::from_value(delivery.event["payload"]["payload"]["grant"].clone()).unwrap();
        (grant, delivery)
    }
    fn accepted(delivery: &RelayDelivery) -> Value {
        json!({"profile":{"policyVersion":delivery.event["payload"]["payload"]["expectedPolicyVersion"]},
            "runtime":{"lifeTask":{"status":"running","lifePermitId":delivery.event["payload"]["payload"]["grant"]["lifePermitId"]}}})
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
                    self.actor_id.clone(),
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
    fn activate(&mut self, id: &str, now: u64) -> (LifeGrant, RelayDelivery) {
        let (grant, delivery) = self.prepare(id, now);
        assert_eq!(
            self.reply(
                &delivery,
                ReplyStatus::Ok,
                Self::accepted(&delivery),
                now + 1
            )["status"],
            "ok"
        );
        let activation = self.actor.try_recv().unwrap();
        assert_eq!(activation.event["payload"]["operation"], "lifeActivated");
        assert!(self.state.delivery_current(&activation, now + 1));
        (grant, activation)
    }
    fn active(&self, now: u64) -> Result<Option<LifeGrant>, SupplyError> {
        self.ledger
            .lock()
            .unwrap()
            .active_life(&character("actor", 2), &self.actor_id, now)
    }
    fn claim_result(
        &mut self,
        id: &str,
        grant: &LifeGrant,
        position: Option<SupplyAnchor>,
        now: u64,
    ) -> Value {
        self.actor_control(claim(id, grant), position, now);
        self.actor.try_recv().unwrap().event["payload"].clone()
    }
    fn reset_volatile_relay(&mut self, now: u64) {
        // This resets the relay cache only, not the journal process/boot.
        let mut state = RelayState::new(Arc::new(policy()));
        state.supplies = Some(self.ledger.clone());
        let owner = attach(&mut state, admission(&self.owner_id, "human", 1), now);
        let mut actor = attach(&mut state, admission(&self.actor_id, "actor", 2), now);
        register(&mut state, &self.actor_id, &mut actor, now);
        self.state = state;
        self.owner = owner;
        self.actor = actor;
    }
}

#[test]
fn companion_relay_life_prepare_requires_matching_ack_before_scope_child_or_private_activation() {
    let mut fixture = Fixture::new();
    let (grant, delivery) = fixture.prepare("authorize:a", 101);
    assert!(fixture.active(102).unwrap().is_none());
    assert!(!fixture
        .ledger
        .lock()
        .unwrap()
        .projection_handle()
        .read()
        .unwrap()
        .actors
        .contains_key(&character("actor", 2)));
    assert_eq!(
        fixture.claim_result("child:before-ack", &grant, at(288, 616), 102)["status"],
        "denied"
    );
    assert!(fixture.actor.try_recv().is_err());
    assert_eq!(
        fixture.reply(
            &delivery,
            ReplyStatus::Ok,
            Fixture::accepted(&delivery),
            103
        )["status"],
        "ok"
    );
    let activation = fixture.actor.try_recv().unwrap();
    assert_eq!(
        activation.event["payload"]["payload"],
        json!({"lifePermitId":grant.life_permit_id})
    );
    assert!(fixture.state.delivery_current(&activation, 104));
    assert_eq!(fixture.active(104).unwrap(), Some(grant));
    assert!(fixture.state.pending.is_empty());
}

#[test]
fn companion_relay_life_private_activation_is_exact_issued_and_fails_after_revocation() {
    let mut fixture = Fixture::new();
    let (_grant, activation) = fixture.activate("authorize:a", 101);
    for changed in [
        "lifePermitId",
        "deadline",
        "generation",
        "ownerRef",
        "type",
        "requestId",
        "missingOwner",
    ] {
        let mut forged = activation.clone();
        match changed {
            "lifePermitId" => {
                forged.event["payload"]["payload"]["lifePermitId"] = json!("life:forged")
            }
            "deadline" => forged.event["payload"]["deadlineAtMs"] = json!(900_000),
            "generation" => forged.event["payload"]["generation"] = json!(99),
            "ownerRef" => forged.event["payload"]["ownerRef"] = json!("forged"),
            "type" => forged.event["type"] = json!("forged"),
            "requestId" => forged.event["payload"]["requestId"] = json!("activation:forged"),
            "missingOwner" => forged.authorizing_owner = None,
            _ => unreachable!(),
        }
        assert!(
            !fixture.state.delivery_current(&forged, 103),
            "Changed {changed} must not become an activation capability."
        );
    }
    let deadline = activation.event["payload"]["deadlineAtMs"]
        .as_u64()
        .unwrap();
    assert!(!fixture.state.delivery_current(&activation, deadline));
    fixture.owner_control(request("cancel:a", Operation::CancelLife, json!({})), 104);
    assert!(!fixture.state.delivery_current(&activation, 105));
    assert_eq!(fixture.active(105).err(), Some(SupplyError::Revoked));
}

#[test]
fn companion_relay_life_activation_cannot_be_forged_by_owner_payload_or_owner_reply() {
    let mut fixture = Fixture::new();
    assert!(serde_json::from_value::<OwnerRequest>(json!({"requestId":"forged","residentId":"alo","operation":"lifeActivated","payload":{"lifePermitId":"life:forged"}})).is_err());
    for (index, field) in ["grant", "home", "bounds", "routeId"]
        .into_iter()
        .enumerate()
    {
        let mut invalid = authorize(&format!("owner:injection:{index}"));
        invalid.payload[field] = json!({"lifePermitId":"life:forged"});
        fixture.owner_control(invalid, 101 + index as u64);
        assert_eq!(
            fixture.owner.try_recv().unwrap().event["payload"]["status"],
            "invalid"
        );
        assert!(fixture.actor.try_recv().is_err());
    }
    let (_grant, delivery) = fixture.prepare("authorize:valid", 110);
    let wrong_sender = fixture.state.handle(
        Command::Reply(
            fixture.owner_id.clone(),
            RuntimeReply {
                request_id: delivery.event["payload"]["requestId"]
                    .as_str()
                    .unwrap()
                    .into(),
                generation: delivery.event["payload"]["generation"].as_u64().unwrap(),
                status: ReplyStatus::Ok,
                result: Fixture::accepted(&delivery),
            },
        ),
        111,
    );
    assert!(wrong_sender.is_err());
    assert!(fixture.active(111).unwrap().is_none());
    assert!(fixture.actor.try_recv().is_err());
    assert_eq!(
        fixture.reply(
            &delivery,
            ReplyStatus::Ok,
            Fixture::accepted(&delivery),
            112
        )["status"],
        "ok"
    );
    assert!(fixture
        .state
        .delivery_current(&fixture.actor.try_recv().unwrap(), 112));
}

#[test]
fn companion_relay_life_actor_claim_is_home_bound_owner_denied_and_repeated_request_has_one_child()
{
    let mut fixture = Fixture::new();
    let (grant, _) = fixture.activate("authorize:a", 101);
    let before = fs::read(&fixture.path).unwrap();
    fixture
        .state
        .handle(
            Command::ControlAt(
                fixture.owner_id.clone(),
                claim("owner:claim", &grant),
                at(288, 616),
            ),
            103,
        )
        .unwrap();
    assert_eq!(
        fixture.owner.try_recv().unwrap().event["payload"]["status"],
        "denied"
    );
    let mut wrong_map = at(288, 616).unwrap();
    wrong_map.map_file_name = "0141".into();
    for (index, position) in [None, at(290, 616), Some(wrong_map)]
        .into_iter()
        .enumerate()
    {
        assert_eq!(
            fixture.claim_result(
                &format!("child:not-home:{index}"),
                &grant,
                position,
                104 + index as u64
            )["status"],
            "denied"
        );
    }
    assert!(fs::read(&fixture.path).unwrap() == before);
    let result = fixture.claim_result("child:first", &grant, at(289, 616), 110);
    assert_eq!(result["status"], "ok");
    let child: SupplyGrant = serde_json::from_value(result["result"]["grant"].clone()).unwrap();
    assert_eq!(
        (child.max_gold, child.quantity, child.item_index),
        (50, 1, 658)
    );
    assert_eq!(child.expires_at_ms, 300_110);
    let sealed = fs::read(&fixture.path).unwrap();
    let duplicate = fixture.claim_result("child:different-request-id", &grant, at(288, 616), 111);
    assert_eq!(duplicate["status"], "ok");
    assert_eq!(
        serde_json::from_value::<SupplyGrant>(duplicate["result"]["grant"].clone()).unwrap(),
        child
    );
    assert!(
        fs::read(&fixture.path).unwrap() == sealed,
        "A caller-chosen retry ID cannot mint another budget."
    );
}

#[test]
fn companion_relay_life_cancel_replay_cannot_revoke_or_dispatch_cancel_to_new_life_after_cache_reset(
) {
    for reset in [false, true] {
        let mut fixture = Fixture::new();
        let (first, activation) = fixture.activate("authorize:a", 101);
        fixture.claim_result("child:a", &first, at(288, 616), 103);
        fixture.owner_control(request("cancel:a", Operation::CancelLife, json!({})), 104);
        let cancel = fixture.actor.try_recv().unwrap();
        assert!(!fixture.state.delivery_current(&activation, 104));
        assert_eq!(
            fixture.reply(
                &cancel,
                ReplyStatus::Ok,
                json!({"runtime":{"lifeTask":{"status":"cancelled"}}}),
                105
            )["status"],
            "ok"
        );
        let (second, _) = fixture.activate("authorize:b", 106);
        if reset {
            fixture.reset_volatile_relay(108);
        }
        let before = fs::read(&fixture.path).unwrap();
        fixture.owner_control(request("cancel:a", Operation::CancelLife, json!({})), 109);
        assert_eq!(
            fixture.owner.try_recv().unwrap().event["payload"]["status"],
            "unknown"
        );
        assert!(
            fixture.actor.try_recv().is_err(),
            "A durable cancel replay must not cancel the new Node life either."
        );
        assert_eq!(fixture.active(110).unwrap(), Some(second));
        assert!(fs::read(&fixture.path).unwrap() == before);
    }
}

#[test]
fn companion_relay_life_owner_detach_preserves_committed_life_and_already_issued_activation() {
    let mut fixture = Fixture::new();
    let (grant, activation) = fixture.activate("authorize:a", 101);
    fixture
        .state
        .handle(Command::Detach(fixture.owner_id.clone()), 103)
        .unwrap();
    assert_eq!(fixture.active(104).unwrap(), Some(grant));
    assert!(fixture.state.delivery_current(&activation, 104), "An already committed life must start even if its owner disconnects before the actor writer flushes activation.");
}

#[test]
fn companion_relay_life_actor_detach_requires_resume_and_old_unattempted_child_stays_recovery_only()
{
    let mut fixture = Fixture::new();
    let (grant, activation) = fixture.activate("authorize:a", 101);
    let result = fixture.claim_result("child:a", &grant, at(288, 616), 103);
    assert_eq!(result["status"], "ok");
    let first_child: SupplyGrant =
        serde_json::from_value(result["result"]["grant"].clone()).unwrap();
    fixture
        .state
        .handle(Command::Detach(fixture.actor_id.clone()), 104)
        .unwrap();
    assert!(!fixture.state.delivery_current(&activation, 105));
    assert_eq!(
        fixture.active(105).err(),
        Some(SupplyError::AdmissionChanged)
    );
    assert_eq!(
        fixture
            .ledger
            .lock()
            .unwrap()
            .projection_handle()
            .read()
            .unwrap()
            .actors[&character("actor", 2)]
            .status,
        LifeScopeStatus::Paused
    );
    fixture.actor_id = "actor:replacement".into();
    fixture.actor = attach(
        &mut fixture.state,
        admission(&fixture.actor_id, "actor", 2),
        106,
    );
    register(
        &mut fixture.state,
        &fixture.actor_id,
        &mut fixture.actor,
        106,
    );
    assert_eq!(
        fixture.claim_result("child:before-resume", &grant, at(288, 616), 107)["status"],
        "denied"
    );
    fixture.owner_control(
        request(
            "resume:a",
            Operation::ResumeLife,
            json!({"expectedPolicyVersion":1,"lifePermitId":grant.life_permit_id}),
        ),
        108,
    );
    let resume = fixture.actor.try_recv().unwrap();
    assert_eq!(
        fixture.reply(&resume, ReplyStatus::Ok, Fixture::accepted(&resume), 109)["status"],
        "ok"
    );
    let resumed = fixture.actor.try_recv().unwrap();
    assert!(fixture.state.delivery_current(&resumed, 110));
    assert_eq!(fixture.active(110).unwrap(), Some(grant.clone()));
    assert_eq!(
        fixture.claim_result("child:after-resume", &grant, at(288, 616), 111)["status"],
        "denied"
    );
    assert_eq!(
        fixture
            .ledger
            .lock()
            .unwrap()
            .grant_for_actor(&character("actor", 2))
            .unwrap(),
        Some(first_child)
    );
}

#[test]
fn companion_relay_life_timeout_and_owner_disconnect_seal_initial_draft_against_same_id_retry() {
    for disconnect in [false, true] {
        let mut fixture = Fixture::new();
        let (draft, delivery) = fixture.prepare("authorize:pending", 101);
        let now = if disconnect {
            fixture
                .state
                .handle(Command::Detach(fixture.owner_id.clone()), 102)
                .unwrap();
            fixture.owner_id = "owner:replacement".into();
            fixture.owner = attach(
                &mut fixture.state,
                admission(&fixture.owner_id, "human", 1),
                103,
            );
            104
        } else {
            fixture.state.expire(20_000);
            assert_eq!(
                fixture.owner.try_recv().unwrap().event["payload"]["status"],
                "unknown"
            );
            20_001
        };
        assert!(!fixture.state.delivery_current(&delivery, now));
        assert!(!fixture
            .ledger
            .lock()
            .unwrap()
            .projection_handle()
            .read()
            .unwrap()
            .actors
            .contains_key(&character("actor", 2)));
        let (same, retried) = fixture.prepare("authorize:pending", now);
        assert_eq!(same, draft);
        assert_eq!(
            fixture.reply(
                &retried,
                ReplyStatus::Ok,
                Fixture::accepted(&retried),
                now + 1
            )["status"],
            "error"
        );
        assert!(fixture.actor.try_recv().is_err());
        assert!(fixture.active(now + 2).unwrap().is_none());
    }
}
