// Adapted from the bounded B2/P0 relay on historical baseline 4e6a9962fb57.
// That relay was uncommitted; original source SHA256:
// a67c2d75d1724a133bc83e1fc419570d906098a77c1fcebd71bf703e09d42502
// Canonical R16 keeps its existing shared_groups and owner_health authority.
//! Opt-in owner control transport. It has no simulation command or asset handle.

#[cfg(test)]
#[path = "companion_relay_supply_tests.rs"]
mod supply_tests;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[cfg(test)]
#[path = "companion_relay_life_tests.rs"]
mod life_tests;
use sha2::{Digest, Sha256};
use tokio::sync::{mpsc, oneshot};

const INPUT_CAPACITY: usize = 128;
pub const OUTPUT_CAPACITY: usize = 32;
const MAX_ADMISSIONS: usize = 2_048;
const MAX_PENDING: usize = 256;
const MAX_PER_OWNER: usize = 8;
const MAX_FINGERPRINTS: usize = 4_096;
const REQUEST_TTL_MS: u64 = 15_000;
const FINGERPRINT_TTL_MS: u64 = 10 * 60_000;
const MAX_RESULT_BYTES: usize = 56 * 1_024;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterBinding {
    pub account_id: String,
    pub character_index: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Enrollment {
    pub resident_id: String,
    pub archive_id: String,
    pub display_name: String,
    pub owner: CharacterBinding,
    pub actor: CharacterBinding,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompanionPolicy {
    pub schema_version: u32,
    pub realm_id: String,
    pub world_epoch: String,
    pub enrollments: Vec<Enrollment>,
}

impl CompanionPolicy {
    /// Trusted startup configuration only. Missing policy leaves existing game behavior intact.
    pub fn from_env() -> Result<Option<Self>, String> {
        let path = match env::var("MIR2_COMPANION_POLICY_PATH") {
            Ok(path) => path,
            Err(env::VarError::NotPresent) => return Ok(None),
            Err(_) => return Err("Invalid companion policy configuration.".into()),
        };
        let realm = env::var("MIR2_REALM_ID")
            .map_err(|_| "Companion policy requires an explicit realm ID.".to_string())?;
        if path.trim().is_empty() {
            return Err("Invalid companion policy configuration.".into());
        }
        let metadata =
            fs::metadata(&path).map_err(|_| "Companion policy is unavailable.".to_string())?;
        if metadata.len() > 256 * 1_024 {
            return Err("Invalid companion policy configuration.".into());
        }
        let bytes = fs::read(path).map_err(|_| "Companion policy is unavailable.".to_string())?;
        Self::parse(&bytes, &realm).map(Some)
    }

    pub fn parse(bytes: &[u8], explicit_realm_id: &str) -> Result<Self, String> {
        let invalid = || "Invalid companion policy configuration.".to_string();
        if bytes.len() > 256 * 1_024 || !valid_ref(explicit_realm_id) {
            return Err(invalid());
        }
        let policy: Self = serde_json::from_slice(bytes).map_err(|_| invalid())?;
        if policy.schema_version != 1
            || policy.realm_id != explicit_realm_id
            || !valid_ref(&policy.world_epoch)
            || policy.enrollments.len() > 256
        {
            return Err(invalid());
        }
        let mut residents = std::collections::BTreeSet::new();
        let mut archives = std::collections::BTreeSet::new();
        let mut actors = std::collections::BTreeSet::new();
        let mut owner_counts = BTreeMap::new();
        for enrollment in &policy.enrollments {
            let count = owner_counts.entry(&enrollment.owner).or_insert(0_usize);
            *count += 1;
            if !valid_ref(&enrollment.resident_id)
                || !valid_ref(&enrollment.archive_id)
                || enrollment.display_name.trim().is_empty()
                || enrollment.display_name.chars().count() > 64
                || enrollment.display_name.chars().any(char::is_control)
                || !valid_character(&enrollment.owner)
                || !valid_character(&enrollment.actor)
                || enrollment.owner == enrollment.actor
                || !residents.insert(&enrollment.resident_id)
                || !archives.insert(&enrollment.archive_id)
                || !actors.insert(&enrollment.actor)
                || *count > 64
            {
                return Err(invalid());
            }
        }
        Ok(policy)
    }

    pub fn actor_owner(&self, actor: &CharacterBinding) -> Option<&CharacterBinding> {
        self.enrollments
            .iter()
            .find(|item| &item.actor == actor)
            .map(|item| &item.owner)
    }
}

fn valid_character(value: &CharacterBinding) -> bool {
    !value.account_id.is_empty()
        && value.account_id.len() <= 160
        && value.account_id.trim() == value.account_id
        && !value.account_id.chars().any(char::is_control)
        && value.character_index >= 0
}

pub fn valid_ref(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
}

pub fn owner_ref(account_id: &str) -> String {
    format!("owner:{}", hex(&Sha256::digest(account_id.as_bytes())))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Operation {
    List,
    View,
    Instruct,
    ProposeTeaching,
    ConfirmTeaching,
    CancelTeaching,
    AuthorizeSupply,
    CancelSupply,
    AuthorizeLife,
    ResumeLife,
    CancelLife,
    RequestLifeSupply,
    Teach,
    RevokeFeedback,
    DeleteFeedback,
    DeleteEpisode,
    ClearGrowth,
    Recall,
    Resume,
    ExportBegin,
    ExportChunk,
    TrialEvent,
}

impl Operation {
    fn name(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::View => "view",
            Self::Instruct => "instruct",
            Self::ProposeTeaching => "proposeTeaching",
            Self::ConfirmTeaching => "confirmTeaching",
            Self::CancelTeaching => "cancelTeaching",
            Self::AuthorizeSupply => "authorizeSupply",
            Self::CancelSupply => "cancelSupply",
            Self::AuthorizeLife => "authorizeLife",
            Self::ResumeLife => "resumeLife",
            Self::CancelLife => "cancelLife",
            Self::RequestLifeSupply => "requestLifeSupply",
            Self::Teach => "teach",
            Self::RevokeFeedback => "revokeFeedback",
            Self::DeleteFeedback => "deleteFeedback",
            Self::DeleteEpisode => "deleteEpisode",
            Self::ClearGrowth => "clearGrowth",
            Self::Recall => "recall",
            Self::Resume => "resume",
            Self::ExportBegin => "exportBegin",
            Self::ExportChunk => "exportChunk",
            Self::TrialEvent => "trialEvent",
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OwnerRequest {
    pub request_id: String,
    pub resident_id: Option<String>,
    pub operation: Operation,
    pub payload: Value,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeRegister {
    pub request_id: String,
    pub resident_id: String,
    pub archive_id: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeReply {
    pub request_id: String,
    pub generation: u64,
    pub status: ReplyStatus,
    pub result: Value,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ReplyStatus {
    Ok,
    Denied,
    Offline,
    Unknown,
    Invalid,
    Conflict,
    Error,
}

// These requests can contain private memory. Never inherit a payload's Debug.
impl std::fmt::Debug for OwnerRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OwnerRequest { .. }")
    }
}
impl std::fmt::Debug for RuntimeRegister {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RuntimeRegister { .. }")
    }
}
impl std::fmt::Debug for RuntimeReply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RuntimeReply { .. }")
    }
}

impl OwnerRequest {
    pub fn validate(&self) -> bool {
        if !valid_ref(&self.request_id)
            || self.resident_id.as_deref().is_some_and(|id| !valid_ref(id))
            || (self.operation == Operation::List) != self.resident_id.is_none()
        {
            return false;
        }
        let Some(fields) = self.payload.as_object() else {
            return false;
        };
        let keys = |allowed: &[&str]| fields.keys().all(|key| allowed.contains(&key.as_str()));
        let reference = |name: &str| {
            fields
                .get(name)
                .and_then(Value::as_str)
                .is_some_and(valid_ref)
        };
        let version = || {
            fields
                .get("expectedPolicyVersion")
                .and_then(Value::as_u64)
                .is_some_and(|value| value > 0 && value <= MAX_SAFE_INTEGER)
        };
        match self.operation {
            Operation::List | Operation::Recall | Operation::Resume | Operation::ExportBegin | Operation::CancelSupply | Operation::CancelLife => {
                fields.is_empty()
            }
            Operation::View => {
                keys(&["cursor", "limit"])
                    && fields
                        .get("cursor")
                        .is_none_or(|cursor| cursor.as_str().is_some_and(valid_ref))
                    && fields
                        .get("limit")
                        .is_none_or(|limit| limit.as_u64().is_some_and(|n| (1..=50).contains(&n)))
            }
            Operation::Teach => keys(&["expectedPolicyVersion"]) && version(),
            Operation::AuthorizeLife => keys(&["expectedPolicyVersion", "budgetGold", "durationSeconds"])
                && version() && fields.get("budgetGold").and_then(Value::as_u64).is_some_and(|n| (1..=50).contains(&n))
                && fields.get("durationSeconds").and_then(Value::as_u64).is_some_and(|n| n == 1200 || n == 1800),
            Operation::ResumeLife => keys(&["expectedPolicyVersion", "lifePermitId"]) && version() && reference("lifePermitId"),
            Operation::RequestLifeSupply => keys(&["lifePermitId"]) && reference("lifePermitId"),
            Operation::Instruct | Operation::ProposeTeaching => {
                keys(&["text", "expectedPolicyVersion"])
                    && version()
                    && fields.get("text").and_then(Value::as_str).is_some_and(|text| {
                        !text.is_empty()
                            && text.trim() == text
                            && text.len() <= 6_000
                            && text.chars().count() <= 1_000
                            && !text.chars().any(char::is_control)
                    })
            }
            Operation::RevokeFeedback | Operation::DeleteFeedback => {
                keys(&["feedbackId", "expectedPolicyVersion"])
                    && version()
                    && reference("feedbackId")
            }
            Operation::ConfirmTeaching => {
                keys(&["proposalId", "expectedPolicyVersion"]) && version() && reference("proposalId")
            }
            Operation::CancelTeaching => {
                keys(&["proposalId"]) && reference("proposalId")
            }
            Operation::AuthorizeSupply => {
                keys(&["expectedPolicyVersion", "budgetGold", "quantity", "expiresInSeconds"])
                    && version()
                    && fields.get("budgetGold").and_then(Value::as_u64).is_some_and(|n| (1..=50).contains(&n))
                    && fields.get("quantity").and_then(Value::as_u64) == Some(1)
                    && fields.get("expiresInSeconds").and_then(Value::as_u64) == Some(300)
            }
            Operation::DeleteEpisode => {
                keys(&["episodeId", "expectedPolicyVersion"]) && version() && reference("episodeId")
            }
            Operation::ClearGrowth => {
                keys(&["confirmArchiveId", "expectedPolicyVersion"])
                    && version()
                    && reference("confirmArchiveId")
            }
            Operation::ExportChunk => {
                keys(&["exportId", "index"])
                    && reference("exportId")
                    && fields
                        .get("index")
                        .and_then(Value::as_u64)
                        .is_some_and(|n| n <= 128)
            }
            Operation::TrialEvent => {
                if !reference("visitId") {
                    return false;
                }
                let reason_text = || {
                    fields.get("reasonText").is_none_or(|value| {
                        value.as_str().is_some_and(|s| {
                            s.chars().count() <= 300 && !s.chars().any(char::is_control)
                        })
                    })
                };
                match fields.get("kind").and_then(Value::as_str) {
                    Some("visit") => {
                        keys(&["kind", "visitId", "entrySource", "reminderCount"])
                            && fields
                                .get("entrySource")
                                .and_then(Value::as_str)
                                .is_some_and(|s| {
                                    ["unprompted", "reminder", "scheduled", "unknown"].contains(&s)
                                })
                            && fields
                                .get("reminderCount")
                                .and_then(Value::as_u64)
                                .is_some_and(|n| n <= 100)
                    }
                    Some("choice") => {
                        keys(&["kind", "visitId", "reason", "reasonText"])
                            && fields
                                .get("reason")
                                .and_then(Value::as_str)
                                .is_some_and(|s| {
                                    [
                                        "shared-history",
                                        "stronger",
                                        "setup-ease",
                                        "curiosity",
                                        "other",
                                    ]
                                    .contains(&s)
                                })
                            && reason_text()
                    }
                    Some("teachingCheck") => {
                        keys(&["kind", "visitId", "understood", "reasonText"])
                            && fields.get("understood").is_some_and(Value::is_boolean)
                            && reason_text()
                    }
                    Some("purchaseIntent") => {
                        keys(&["kind", "visitId", "offerId"]) && reference("offerId")
                    }
                    Some("visitEnd") => keys(&["kind", "visitId"]),
                    _ => false,
                }
            }
        }
    }
}

/// Only the authenticated socket adapter constructs these; none is a wire type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Admission {
    pub id: String,
    pub character: CharacterBinding,
    pub identity_session_id: String,
    pub expires_at_ms: u64,
}

#[derive(Clone)]
pub struct RelayDelivery {
    pub recipient: Admission,
    /// Runtime commands must revalidate the requesting owner's authority too.
    pub authorizing_owner: Option<Admission>,
    pub event: Value,
}

#[derive(Clone)]
pub struct CompanionRelay {
    policy: Arc<CompanionPolicy>,
    sender: mpsc::Sender<Request>,
    supplies: Option<Arc<Mutex<crate::companion_supply::SupplyLedger>>>,
}

#[derive(Clone)]
pub struct SupplyAnchor {
    pub map_file_name: String,
    pub point: crate::companion_supply::SupplyPoint,
}

enum Command {
    Attach(Admission, mpsc::Sender<RelayDelivery>),
    Control(String, OwnerRequest),
    ControlAt(String, OwnerRequest, Option<SupplyAnchor>),
    Register(String, RuntimeRegister),
    Reply(String, RuntimeReply),
    Detach(String),
    Validate(RelayDelivery),
}
struct Request {
    command: Command,
    reply: oneshot::Sender<Result<bool, String>>,
}

impl CompanionRelay {
    pub fn from_env() -> Result<Option<Self>, String> {
        let Some(policy) = CompanionPolicy::from_env()? else { return Ok(None); };
        let invalid = || "Invalid companion supply configuration.".to_string();
        let path = env::var("MIR2_COMPANION_SUPPLY_JOURNAL_PATH").ok();
        let key = env::var("MIR2_COMPANION_SUPPLY_JOURNAL_MAC_KEY").ok();
        let supplies = match (path, key) {
            (None, None) => None,
            (Some(path), Some(key)) if !path.trim().is_empty() && key.len() == 64 => {
                let mut bytes = [0_u8;32];
                for (index, pair) in key.as_bytes().chunks_exact(2).enumerate() {
                    let value = std::str::from_utf8(pair).map_err(|_| invalid())?;
                    bytes[index] = u8::from_str_radix(value,16).map_err(|_| invalid())?;
                }
                Some(crate::companion_supply::SupplyLedger::open(path.into(), bytes,
                    policy.realm_id.clone(), policy.world_epoch.clone()).map_err(|_| invalid())?)
            }
            _ => return Err(invalid()),
        };
        Ok(Some(Self::with_supply(policy, supplies)))
    }

    pub fn new(policy: CompanionPolicy) -> Self {
        Self::with_supply(policy, None)
    }

    pub fn with_supply(policy: CompanionPolicy, supplies: Option<crate::companion_supply::SupplyLedger>) -> Self {
        let policy = Arc::new(policy);
        let supplies = supplies.map(|ledger| Arc::new(Mutex::new(ledger)));
        let (sender, mut receiver) = mpsc::channel::<Request>(INPUT_CAPACITY);
        let mut state = RelayState::new(Arc::clone(&policy));
        state.supplies = supplies.clone();
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(std::time::Duration::from_millis(250));
            timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    request = receiver.recv() => {
                        let Some(request) = request else { break; };
                        let result = state.handle(request.command, now_ms());
                        let _ = request.reply.send(result);
                    }
                    _ = timer.tick() => state.expire(now_ms()),
                }
            }
        });
        Self { policy, sender, supplies }
    }

    pub fn policy(&self) -> &CompanionPolicy {
        &self.policy
    }
    pub(crate) fn supply_ledger(&self) -> Option<&Arc<Mutex<crate::companion_supply::SupplyLedger>>> {
        self.supplies.as_ref()
    }
    pub async fn attach(
        &self,
        admission: Admission,
        sender: mpsc::Sender<RelayDelivery>,
    ) -> Result<(), String> {
        self.request(Command::Attach(admission, sender))
            .await
            .map(|_| ())
    }
    pub async fn control(&self, admission: &str, request: OwnerRequest) -> Result<(), String> {
        self.request(Command::Control(admission.into(), request))
            .await
            .map(|_| ())
    }
    pub async fn control_at(&self, admission: &str, request: OwnerRequest, anchor: Option<SupplyAnchor>) -> Result<(), String> {
        self.request(Command::ControlAt(admission.into(), request, anchor)).await.map(|_| ())
    }
    pub async fn register(&self, admission: &str, request: RuntimeRegister) -> Result<(), String> {
        self.request(Command::Register(admission.into(), request))
            .await
            .map(|_| ())
    }
    pub async fn reply(&self, admission: &str, request: RuntimeReply) -> Result<(), String> {
        self.request(Command::Reply(admission.into(), request))
            .await
            .map(|_| ())
    }
    pub async fn detach(&self, admission: &str) {
        let _ = self.request(Command::Detach(admission.into())).await;
    }
    pub async fn delivery_is_current(&self, delivery: &RelayDelivery) -> bool {
        self.request(Command::Validate(delivery.clone()))
            .await
            .unwrap_or(false)
    }
    async fn request(&self, command: Command) -> Result<bool, String> {
        let (reply, result) = oneshot::channel();
        self.sender
            .try_send(Request { command, reply })
            .map_err(|_| "Companion relay is unavailable.".to_string())?;
        tokio::time::timeout(std::time::Duration::from_secs(2), result)
            .await
            .map_err(|_| "Companion relay response is unknown.".to_string())?
            .map_err(|_| "Companion relay is unavailable.".to_string())?
    }
}

struct Online {
    admission: Admission,
    sender: mpsc::Sender<RelayDelivery>,
}
struct Runtime {
    admission_id: String,
    generation: u64,
}
struct Pending {
    owner: Admission,
    runtime_id: String,
    generation: u64,
    request: OwnerRequest,
    deadline_at_ms: u64,
    supply_grant: Option<crate::companion_supply::SupplyGrant>,
    life_grant: Option<crate::companion_supply::LifeGrant>,
}
struct Fingerprint {
    digest: String,
    expires_at_ms: u64,
}
struct RelayState {
    policy: Arc<CompanionPolicy>,
    supplies: Option<Arc<Mutex<crate::companion_supply::SupplyLedger>>>,
    admissions: BTreeMap<String, Online>,
    runtimes: BTreeMap<String, Runtime>,
    generations: BTreeMap<String, u64>,
    pending: BTreeMap<String, Pending>,
    fingerprints: BTreeMap<String, Fingerprint>,
    activation_deliveries: BTreeMap<String, RelayDelivery>,
}

impl RelayState {
    fn new(policy: Arc<CompanionPolicy>) -> Self {
        Self {
            policy,
            supplies: None,
            admissions: BTreeMap::new(),
            runtimes: BTreeMap::new(),
            generations: BTreeMap::new(),
            pending: BTreeMap::new(),
            fingerprints: BTreeMap::new(),
            activation_deliveries: BTreeMap::new(),
        }
    }
    fn handle(&mut self, command: Command, now: u64) -> Result<bool, String> {
        self.expire(now);
        match command {
            Command::Attach(admission, sender) => {
                if admission.id.is_empty()
                    || !valid_character(&admission.character)
                    || admission.identity_session_id.is_empty()
                    || admission.expires_at_ms <= now
                    || sender.is_closed()
                    || self.admissions.len() >= MAX_ADMISSIONS
                    || self.admissions.contains_key(&admission.id)
                {
                    return Err("Companion admission is unavailable.".into());
                }
                self.admissions
                    .insert(admission.id.clone(), Online { admission, sender });
            }
            Command::Detach(id) => self.detach_at(&id, now),
            Command::Control(id, request) => self.control(&id, request, now)?,
            Command::ControlAt(id, request, anchor) => self.control_at(&id, request, anchor, now)?,
            Command::Register(id, request) => self.register(&id, request)?,
            Command::Reply(id, reply) => self.reply_at(&id, reply, now)?,
            Command::Validate(delivery) => return Ok(self.delivery_current(&delivery, now)),
        }
        Ok(true)
    }

    fn emit(
        &mut self,
        recipient: &Admission,
        authorizing_owner: Option<Admission>,
        event: Value,
    ) -> bool {
        if event.to_string().len() > 63 * 1_024 {
            self.detach(&recipient.id);
            return false;
        }
        let success = self
            .admissions
            .get(&recipient.id)
            .filter(|online| online.admission == *recipient)
            .is_some_and(|online| {
                online
                    .sender
                    .try_send(RelayDelivery {
                        recipient: recipient.clone(),
                        authorizing_owner,
                        event,
                    })
                    .is_ok()
            });
        if !success {
            self.detach(&recipient.id);
        }
        success
    }

    fn result(
        &mut self,
        recipient: &Admission,
        request_id: &str,
        resident_id: Option<&str>,
        operation: &str,
        status: ReplyStatus,
        result: Value,
    ) {
        self.emit(
            recipient,
            None,
            result_event(request_id, resident_id, operation, status, result),
        );
    }

    fn register(&mut self, id: &str, request: RuntimeRegister) -> Result<(), String> {
        let admission = self
            .admissions
            .get(id)
            .ok_or("Companion admission is unavailable.")?
            .admission
            .clone();
        let enrollment = self
            .policy
            .enrollments
            .iter()
            .find(|item| {
                item.resident_id == request.resident_id
                    && item.archive_id == request.archive_id
                    && item.actor == admission.character
            })
            .cloned();
        if !valid_ref(&request.request_id)
            || !valid_ref(&request.resident_id)
            || !valid_ref(&request.archive_id)
        {
            self.result(
                &admission,
                &request.request_id,
                Some(&request.resident_id),
                "register",
                ReplyStatus::Invalid,
                json!({}),
            );
            return Ok(());
        }
        let Some(enrollment) = enrollment else {
            self.result(
                &admission,
                &request.request_id,
                Some(&request.resident_id),
                "register",
                ReplyStatus::Denied,
                json!({}),
            );
            return Ok(());
        };
        let generation = if let Some(runtime) = self.runtimes.get(&request.resident_id) {
            if runtime.admission_id != id {
                self.result(
                    &admission,
                    &request.request_id,
                    Some(&request.resident_id),
                    "register",
                    ReplyStatus::Conflict,
                    json!({}),
                );
                return Ok(());
            }
            runtime.generation
        } else {
            let next = self
                .generations
                .get(&request.resident_id)
                .copied()
                .unwrap_or(0)
                .checked_add(1)
                .filter(|n| *n <= MAX_SAFE_INTEGER)
                .ok_or("Companion runtime generations are exhausted.")?;
            self.generations.insert(request.resident_id.clone(), next);
            self.runtimes.insert(
                request.resident_id.clone(),
                Runtime {
                    admission_id: id.into(),
                    generation: next,
                },
            );
            next
        };
        self.result(&admission, &request.request_id, Some(&request.resident_id), "register", ReplyStatus::Ok, json!({
            "generation": generation, "ownerRef": owner_ref(&enrollment.owner.account_id), "archiveId": enrollment.archive_id,
            "worldEpoch": self.policy.world_epoch,
        }));
        Ok(())
    }

    fn control(&mut self, id: &str, request: OwnerRequest, now: u64) -> Result<(), String> {
        self.control_at(id, request, None, now)
    }

    fn control_at(&mut self, id: &str, request: OwnerRequest, anchor: Option<SupplyAnchor>, now: u64) -> Result<(), String> {
        let owner = self
            .admissions
            .get(id)
            .ok_or("Companion admission is unavailable.")?
            .admission
            .clone();
        let reject = |state: &mut Self, status| {
            state.result(
                &owner,
                &request.request_id,
                request.resident_id.as_deref(),
                request.operation.name(),
                status,
                json!({}),
            )
        };
        if !request.validate() {
            reject(self, ReplyStatus::Invalid);
            return Ok(());
        }
        if request.operation == Operation::List {
            let companions = self.policy.enrollments.iter().filter(|item| item.owner == owner.character).map(|item| json!({
                "residentId": item.resident_id, "archiveId": item.archive_id, "displayName": item.display_name,
                "online": self.runtimes.contains_key(&item.resident_id),
            })).collect::<Vec<_>>();
            self.result(
                &owner,
                &request.request_id,
                None,
                "list",
                ReplyStatus::Ok,
                json!({ "companions": companions, "worldEpoch": self.policy.world_epoch }),
            );
            return Ok(());
        }
        if request.operation == Operation::RequestLifeSupply {
            return self.claim_life_supply(&owner, &request, anchor, now);
        }
        let resident = request.resident_id.as_deref().expect("validated resident");
        let Some(enrollment) = self
            .policy
            .enrollments
            .iter()
            .find(|item| item.resident_id == resident && item.owner == owner.character)
            .cloned()
        else {
            reject(self, ReplyStatus::Denied);
            return Ok(());
        };
        if request.operation == Operation::ClearGrowth
            && request.payload["confirmArchiveId"] != enrollment.archive_id
        {
            reject(self, ReplyStatus::Invalid);
            return Ok(());
        }
        let request_id = namespaced_request_id(&self.policy, &owner.character, &request.request_id);
        let digest = hex(&Sha256::digest(
            serde_json::to_vec(&(resident, request.operation, &request.payload))
                .expect("JSON request"),
        ));
        if self
            .fingerprints
            .get(&request_id)
            .is_some_and(|entry| entry.digest != digest)
        {
            reject(self, ReplyStatus::Conflict);
            return Ok(());
        }
        // A conflicting replay has no side effects. A valid revocation still
        // precedes runtime-liveness checks, including when the actor is offline.
        if matches!(request.operation, Operation::CancelLife | Operation::Recall) {
            if let Some(ledger) = self.supplies.clone() {
                let mut ledger = ledger.lock().map_err(|_| "Companion life authority unavailable.".to_string())?;
                let life_key = if request.operation == Operation::Recall { format!("{request_id}:life") } else { request_id.clone() };
                match ledger.life_revocation_seen(&life_key, &owner.character, resident) {
                    Ok(true) => { reject(self, ReplyStatus::Unknown); return Ok(()); }
                    Err(_) => { reject(self, ReplyStatus::Conflict); return Ok(()); }
                    Ok(false) => {}
                }
                if ledger.revoke_life_request(&life_key, &owner.character, resident, now).is_err() {
                    reject(self, ReplyStatus::Error); return Ok(());
                }
            }
        }
        if matches!(request.operation, Operation::CancelSupply | Operation::Recall) {
            if let Some(ledger) = self.supplies.clone() {
                let mut ledger = ledger.lock().map_err(|_| "Companion supply authority unavailable.".to_string())?;
                match ledger.revocation_seen(&request_id, &owner.character, resident) {
                    Ok(true) => { reject(self, ReplyStatus::Unknown); return Ok(()); }
                    Err(_) => { reject(self, ReplyStatus::Conflict); return Ok(()); }
                    Ok(false) => {}
                }
                let revoked = ledger.revoke_request(&request_id, &owner.character, resident, now);
                if revoked.is_err() { reject(self, ReplyStatus::Error); return Ok(()); }
            }
        }
        if let Some(pending) = self.pending.get(&request_id) {
            if pending.owner.id == owner.id {
                return Ok(());
            }
            reject(self, ReplyStatus::Unknown);
            return Ok(());
        }
        if self.pending.len() >= MAX_PENDING
            || self
                .pending
                .values()
                .filter(|item| item.owner.character == owner.character)
                .count()
                >= MAX_PER_OWNER
        {
            reject(self, ReplyStatus::Error);
            return Ok(());
        }
        let Some(runtime) = self.runtimes.get(resident) else {
            reject(self, ReplyStatus::Offline);
            return Ok(());
        };
        let runtime_id = runtime.admission_id.clone();
        let generation = runtime.generation;
        let Some(actor) = self
            .admissions
            .get(&runtime_id)
            .map(|online| online.admission.clone())
        else {
            reject(self, ReplyStatus::Offline);
            return Ok(());
        };
        if self.fingerprints.len() >= MAX_FINGERPRINTS
            && !self.fingerprints.contains_key(&request_id)
        {
            reject(self, ReplyStatus::Error);
            return Ok(());
        }
        let deadline = now
            .saturating_add(REQUEST_TTL_MS)
            .min(owner.expires_at_ms)
            .min(actor.expires_at_ms);
        let mut runtime_payload = request.payload.clone();
        let mut supply_grant = None;
        let mut life_grant = None;
        if matches!(request.operation, Operation::AuthorizeLife | Operation::ResumeLife) {
            let Some(ledger) = self.supplies.clone() else { reject(self, ReplyStatus::Denied); return Ok(()); };
            let mut ledger = ledger.lock().map_err(|_| "Companion life authority unavailable.".to_string())?;
            let prepared = if request.operation == Operation::AuthorizeLife {
                ledger.prepare_life(&request_id, &enrollment, &actor.id, now,
                    request.payload["budgetGold"].as_u64().expect("validated life budget") as u32,
                    request.payload["durationSeconds"].as_u64().expect("validated life duration") as u32)
            } else {
                ledger.resume_life_prepare(&request_id, &enrollment, &actor.id,
                    request.payload["lifePermitId"].as_str().expect("validated life reference"), now)
            };
            let grant = match prepared { Ok(grant) => grant, Err(_) => { reject(self, ReplyStatus::Denied); return Ok(()); } };
            runtime_payload["grant"] = serde_json::to_value(&grant).expect("public bounded life grant");
            life_grant = Some(grant);
        }
        if request.operation == Operation::AuthorizeSupply {
            let Some(anchor) = anchor.filter(|anchor| anchor.map_file_name == "0") else {
                reject(self, ReplyStatus::Denied); return Ok(());
            };
            let Some(ledger) = self.supplies.clone() else {
                reject(self, ReplyStatus::Denied); return Ok(());
            };
            let grant = ledger.lock().map_err(|_| "Companion supply authority unavailable.".to_string())?
                .prepare(&request_id, &enrollment, &actor.id, anchor.point, now,
                    request.payload["budgetGold"].as_u64().expect("validated budget") as u32, 300);
            let grant = match grant { Ok(grant) => grant, Err(_) => { reject(self, ReplyStatus::Denied); return Ok(()); } };
            runtime_payload["grant"] = serde_json::to_value(&grant).expect("public bounded supply grant");
            supply_grant = Some(grant);
        }
        let event = json!({ "type": "companionRequest", "payload": {
            "requestId": request_id, "residentId": resident, "archiveId": enrollment.archive_id, "worldEpoch": self.policy.world_epoch,
            "generation": generation, "ownerRef": owner_ref(&owner.character.account_id), "operation": request.operation,
            "payload": runtime_payload, "deadlineAtMs": deadline,
        }});
        self.fingerprints.insert(
            request_id.clone(),
            Fingerprint {
                digest,
                expires_at_ms: now.saturating_add(FINGERPRINT_TTL_MS),
            },
        );
        self.pending.insert(
            request_id.clone(),
            Pending {
                owner: owner.clone(),
                runtime_id,
                generation,
                request,
                deadline_at_ms: deadline,
                supply_grant,
                life_grant,
            },
        );
        // Admission is recorded before delivery. A full/closed output therefore
        // follows the same fail-closed unknown path as a lost runtime connection.
        self.emit(&actor, Some(owner), event);
        Ok(())
    }

    fn claim_life_supply(&mut self, actor: &Admission, request: &OwnerRequest, anchor: Option<SupplyAnchor>, now: u64) -> Result<(), String> {
        let resident = request.resident_id.as_deref().expect("validated resident");
        let bound = actor.expires_at_ms > now && self.policy.enrollments.iter().any(|item| item.resident_id == resident && item.actor == actor.character)
            && self.runtimes.get(resident).is_some_and(|runtime| runtime.admission_id == actor.id);
        let at_home = anchor.is_some_and(|p| p.map_file_name == "0" && (p.point.x - 288).abs().max((p.point.y - 616).abs()) <= 1);
        let granted = if bound && at_home {
            self.supplies.as_ref().and_then(|ledger| ledger.lock().ok()).and_then(|mut ledger|
                ledger.claim_life_supply(&actor.character, &actor.id, request.payload["lifePermitId"].as_str().expect("validated life reference"), now).ok())
        } else { None };
        let (status, result) = match granted {
            Some(grant) => (ReplyStatus::Ok, json!({"grant":grant,"lifePermitId":request.payload["lifePermitId"]})),
            None => (ReplyStatus::Denied, json!({})),
        };
        self.result(actor, &request.request_id, Some(resident), request.operation.name(), status, result);
        Ok(())
    }

    fn reply(&mut self, id: &str, reply: RuntimeReply) -> Result<(), String> {
        self.reply_at(id, reply, now_ms())
    }

    fn reply_at(&mut self, id: &str, mut reply: RuntimeReply, now: u64) -> Result<(), String> {
        if !valid_ref(&reply.request_id)
            || reply.generation == 0
            || reply.generation > MAX_SAFE_INTEGER
            || serde_json::to_vec(&reply.result)
                .map_or(true, |bytes| bytes.len() > MAX_RESULT_BYTES)
        {
            return Err("Invalid companion runtime result.".into());
        }
        let Some(pending) = self.pending.get(&reply.request_id) else {
            return Ok(());
        };
        if pending.runtime_id != id || pending.generation != reply.generation {
            return Err("Companion result is not authorized.".into());
        }
        if reply.status == ReplyStatus::Ok && !valid_export_result(&pending.request, &reply.result)
        {
            return Err("Invalid companion export result.".into());
        }
        let pending = self
            .pending
            .remove(&reply.request_id)
            .expect("pending checked");
        if let Some(grant) = &pending.life_grant {
            let enrollment = self.policy.enrollments.iter().find(|item| Some(item.resident_id.as_str()) == pending.request.resident_id.as_deref())
                .ok_or("Companion life enrollment unavailable.")?;
            let key = namespaced_request_id(&self.policy, &pending.owner.character, &pending.request.request_id);
            let accepted = reply.status == ReplyStatus::Ok && pending.deadline_at_ms > now
                && reply.result["profile"]["policyVersion"] == pending.request.payload["expectedPolicyVersion"]
                && reply.result["runtime"]["lifeTask"]["lifePermitId"].as_str() == Some(grant.life_permit_id.as_str())
                && reply.result["runtime"]["lifeTask"]["status"] == "running";
            let activated = self.supplies.as_ref().and_then(|ledger| ledger.lock().ok()).is_some_and(|mut ledger| {
                let outcome = if accepted {
                    if pending.request.operation == Operation::ResumeLife {
                        ledger.resume_life_activate(&key, &enrollment.actor, &pending.runtime_id, &grant.life_permit_id, now)
                    } else { ledger.activate_life(&key, &enrollment.actor, &pending.runtime_id, &grant.life_permit_id, now) }
                } else { Err(crate::companion_supply::SupplyError::Invalid) };
                if outcome.is_ok() { true } else { let _ = ledger.abandon_prepared_life(&key, &enrollment.actor, &grant.life_permit_id, now); false }
            });
            if reply.status == ReplyStatus::Ok && !activated { reply.status=ReplyStatus::Error; reply.result=json!({}); }
            if activated {
                // This private message is emitted only after the ledger commit.
                // It is deliberately absent from the owner Operation enum.
                let actor = self.admissions.get(&pending.runtime_id).map(|online| online.admission.clone());
                if let Some(actor) = actor {
                    let event = json!({"type":"companionRequest","payload":{
                        "requestId":format!("{key}:activation"), "residentId":pending.request.resident_id,
                        "archiveId":enrollment.archive_id,"worldEpoch":self.policy.world_epoch,
                        "generation":pending.generation,"ownerRef":owner_ref(&pending.owner.character.account_id),
                        "operation":"lifeActivated","payload":{"lifePermitId":grant.life_permit_id},
                        "deadlineAtMs":pending.deadline_at_ms
                    }});
                    let activation_id = format!("{key}:activation");
                    if self.activation_deliveries.len() >= MAX_PENDING { reply.status=ReplyStatus::Unknown; reply.result=json!({}); }
                    else {
                        self.activation_deliveries.insert(activation_id, RelayDelivery { recipient:actor.clone(), authorizing_owner:Some(pending.owner.clone()), event:event.clone() });
                        if !self.emit(&actor, Some(pending.owner.clone()), event) { reply.status=ReplyStatus::Unknown; reply.result=json!({}); }
                    }
                } else { reply.status=ReplyStatus::Unknown; reply.result=json!({}); }
            }
        }
        if let Some(grant) = &pending.supply_grant {
            let enrollment = self.policy.enrollments.iter().find(|item| Some(item.resident_id.as_str()) == pending.request.resident_id.as_deref())
                .ok_or("Companion supply enrollment unavailable.")?;
            let key = namespaced_request_id(&self.policy, &pending.owner.character, &pending.request.request_id);
            let accepted = reply.status == ReplyStatus::Ok
                && reply.result["profile"]["policyVersion"] == pending.request.payload["expectedPolicyVersion"]
                && reply.result["runtime"]["supplyTask"]["permitId"].as_str() == Some(grant.permit_id.as_str())
                && reply.result["runtime"]["supplyTask"]["status"] == "authorized"
                && pending.deadline_at_ms > now;
            let activated = self.supplies.as_ref().and_then(|ledger| ledger.lock().ok())
                .is_some_and(|mut ledger| {
                    if accepted {
                        match ledger.activate(&key, &enrollment.actor, &pending.runtime_id, &grant.permit_id, now) {
                            Ok(_) => true,
                            Err(_) => {
                                let _ = ledger.abandon_prepared(&key, &enrollment.actor, &grant.permit_id, now);
                                false
                            }
                        }
                    } else {
                        let _ = ledger.abandon_prepared(&key, &enrollment.actor, &grant.permit_id, now);
                        false
                    }
                });
            if reply.status == ReplyStatus::Ok && !activated { reply.status = ReplyStatus::Error; reply.result = json!({}); }
        }
        let result = if matches!(reply.status, ReplyStatus::Ok | ReplyStatus::Conflict) {
            reply.result
        } else {
            json!({})
        };
        self.result(
            &pending.owner,
            &pending.request.request_id,
            pending.request.resident_id.as_deref(),
            pending.request.operation.name(),
            reply.status,
            result,
        );
        Ok(())
    }

    fn abandon_supply(&self, pending: &Pending, now: u64) {
        if let Some(grant) = &pending.life_grant {
            if let Some(enrollment) = self.policy.enrollments.iter().find(|item| Some(item.resident_id.as_str()) == pending.request.resident_id.as_deref()) {
                if let Some(mut ledger) = self.supplies.as_ref().and_then(|ledger| ledger.lock().ok()) {
                    let key = namespaced_request_id(&self.policy, &pending.owner.character, &pending.request.request_id);
                    let _ = ledger.abandon_prepared_life(&key, &enrollment.actor, &grant.life_permit_id, now);
                }
            }
        }
        let Some(grant) = &pending.supply_grant else { return; };
        let Some(enrollment) = self.policy.enrollments.iter().find(|item| Some(item.resident_id.as_str()) == pending.request.resident_id.as_deref()) else { return; };
        if let Some(ledger) = self.supplies.as_ref().and_then(|ledger| ledger.lock().ok()) {
            let mut ledger = ledger;
            let key = namespaced_request_id(&self.policy, &pending.owner.character, &pending.request.request_id);
            let _ = ledger.abandon_prepared(&key, &enrollment.actor, &grant.permit_id, now);
        }
    }

    fn delivery_current(&self, delivery: &RelayDelivery, now: u64) -> bool {
        let active = |admission: &Admission| {
            admission.expires_at_ms > now
                && self.admissions.get(&admission.id).is_some_and(|online| {
                    online.admission == *admission && !online.sender.is_closed()
                })
        };
        if !active(&delivery.recipient) {
            return false;
        }
        if delivery.event["payload"]["operation"] == "lifeActivated" && delivery.authorizing_owner.is_none() {
            return false;
        }
        if let Some(owner) = &delivery.authorizing_owner {
            let Some(id) = delivery.event["payload"]["requestId"].as_str() else {
                return false;
            };
            if delivery.event["payload"]["operation"] == "lifeActivated" {
                let payload = &delivery.event["payload"];
                let exact = self.activation_deliveries.get(id).is_some_and(|issued| issued.recipient == delivery.recipient && issued.authorizing_owner.as_ref() == Some(owner) && issued.event == delivery.event);
                let current_runtime = payload["residentId"].as_str().and_then(|resident| self.runtimes.get(resident)).is_some_and(|runtime| runtime.admission_id == delivery.recipient.id && Some(runtime.generation) == payload["generation"].as_u64());
                let scoped = self.supplies.as_ref().and_then(|ledger| ledger.lock().ok()).is_some_and(|ledger| {
                    let handle = ledger.projection_handle();
                    let Ok(projection) = handle.read() else { return false; };
                    !projection.poisoned && projection.actors.get(&delivery.recipient.character).is_some_and(|scope| scope.status == crate::companion_supply::LifeScopeStatus::Active && scope.expires_at_ms > now && Some(scope.life_permit_id.as_str()) == payload["payload"]["lifePermitId"].as_str())
                });
                return exact && current_runtime && scoped && payload["deadlineAtMs"].as_u64().is_some_and(|deadline| deadline > now);
            }
            if !active(owner) {
                return false;
            }
            return self.pending.get(id).is_some_and(|pending| {
                pending.owner == *owner
                    && pending.runtime_id == delivery.recipient.id
                    && delivery.event["payload"]["generation"].as_u64() == Some(pending.generation)
                    && delivery.event["payload"]["deadlineAtMs"].as_u64()
                        == Some(pending.deadline_at_ms)
                    && pending.deadline_at_ms > now
            });
        }
        true
    }

    fn detach(&mut self, id: &str) {
        self.detach_at(id, now_ms());
    }

    fn detach_at(&mut self, id: &str, now: u64) {
        // Committed offline-life activation no longer depends on the owner
        // connection. The actor admission, durable scope and deadline still do.
        self.activation_deliveries.retain(|_, delivery| delivery.recipient.id != id);
        if let Some(actor) = self.admissions.get(id).map(|online| online.admission.clone()) {
            if let Some(mut ledger) = self.supplies.as_ref().and_then(|ledger| ledger.lock().ok()) {
                let _ = ledger.pause_life(&actor.character, &actor.id, now);
            }
        }
        self.admissions.remove(id);
        self.runtimes
            .retain(|_, runtime| runtime.admission_id != id);
        let affected = self
            .pending
            .iter()
            .filter(|(_, pending)| pending.owner.id == id || pending.runtime_id == id)
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        for request_id in affected {
            if let Some(pending) = self.pending.remove(&request_id) {
                self.abandon_supply(&pending, now);
                if pending.owner.id != id {
                    self.result(
                        &pending.owner,
                        &pending.request.request_id,
                        pending.request.resident_id.as_deref(),
                        pending.request.operation.name(),
                        ReplyStatus::Unknown,
                        json!({}),
                    );
                }
            }
        }
    }

    fn expire(&mut self, now: u64) {
        self.activation_deliveries.retain(|_, delivery| delivery.event["payload"]["deadlineAtMs"].as_u64().is_some_and(|deadline| deadline > now));
        let dead = self
            .admissions
            .iter()
            .filter(|(_, online)| {
                online.admission.expires_at_ms <= now || online.sender.is_closed()
            })
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        for id in dead {
            self.detach_at(&id, now);
        }
        let expired = self
            .pending
            .iter()
            .filter(|(_, pending)| pending.deadline_at_ms <= now)
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        for id in expired {
            if let Some(pending) = self.pending.remove(&id) {
                self.abandon_supply(&pending, now);
                self.result(
                    &pending.owner,
                    &pending.request.request_id,
                    pending.request.resident_id.as_deref(),
                    pending.request.operation.name(),
                    ReplyStatus::Unknown,
                    json!({}),
                );
            }
        }
        self.fingerprints
            .retain(|_, entry| entry.expires_at_ms > now);
    }
}

pub fn result_event(
    request_id: &str,
    resident_id: Option<&str>,
    operation: &str,
    status: ReplyStatus,
    result: Value,
) -> Value {
    let request_id = if valid_ref(request_id) {
        request_id
    } else {
        "invalid-request"
    };
    let resident_id = resident_id.filter(|id| valid_ref(id));
    json!({ "type": "companionResult", "payload": { "requestId": request_id, "residentId": resident_id, "operation": operation, "status": status, "result": result } })
}

fn namespaced_request_id(
    policy: &CompanionPolicy,
    owner: &CharacterBinding,
    client_id: &str,
) -> String {
    let bytes = serde_json::to_vec(&(
        "mir2-companion-request-v1",
        &policy.realm_id,
        &policy.world_epoch,
        owner,
        client_id,
    ))
    .expect("JSON identity");
    format!("cr:{}", hex(&Sha256::digest(bytes)))
}

fn valid_export_result(request: &OwnerRequest, result: &Value) -> bool {
    if !matches!(
        request.operation,
        Operation::ExportBegin | Operation::ExportChunk
    ) {
        return true;
    }
    let Some(fields) = result.as_object() else {
        return false;
    };
    if !fields
        .get("exportId")
        .and_then(Value::as_str)
        .is_some_and(valid_ref)
    {
        return false;
    }
    let Some(count) = fields
        .get("chunkCount")
        .and_then(Value::as_u64)
        .filter(|n| (1..=128).contains(n))
    else {
        return false;
    };
    match request.operation {
        Operation::ExportBegin => {
            fields.len() == 4
                && fields.keys().all(|key| {
                    ["exportId", "totalBytes", "sha256", "chunkCount"].contains(&key.as_str())
                })
                && fields
                    .get("totalBytes")
                    .and_then(Value::as_u64)
                    .is_some_and(|n| {
                        n > 0 && n <= 2 * 1_024 * 1_024 && n.div_ceil(16 * 1_024) == count
                    })
                && fields
                    .get("sha256")
                    .and_then(Value::as_str)
                    .is_some_and(|s| {
                        s.len() == 64
                            && s.bytes()
                                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                    })
        }
        Operation::ExportChunk => {
            fields.len() == 4
                && fields.keys().all(|key| {
                    ["exportId", "index", "dataBase64", "chunkCount"].contains(&key.as_str())
                })
                && result["exportId"] == request.payload["exportId"]
                && result["index"] == request.payload["index"]
                && fields
                    .get("index")
                    .and_then(Value::as_u64)
                    .is_some_and(|n| n < count)
                && fields
                    .get("dataBase64")
                    .and_then(Value::as_str)
                    .is_some_and(|s| {
                        s.len() <= 21_848
                            && base64::engine::general_purpose::STANDARD
                                .decode(s)
                                .is_ok_and(|bytes| !bytes.is_empty() && bytes.len() <= 16 * 1_024)
                    })
        }
        _ => unreachable!(),
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn character(account: &str, index: i32) -> CharacterBinding {
        CharacterBinding {
            account_id: account.into(),
            character_index: index,
        }
    }
    fn policy() -> CompanionPolicy {
        CompanionPolicy::parse(br#"{"schemaVersion":1,"realmId":"trial","worldEpoch":"epoch-1","enrollments":[{"residentId":"alo","archiveId":"archive-1","displayName":"Alo","owner":{"accountId":"human","characterIndex":1},"actor":{"accountId":"actor","characterIndex":2}}]}"#, "trial").unwrap()
    }
    fn admission(id: &str, account: &str, index: i32) -> Admission {
        Admission {
            id: id.into(),
            character: character(account, index),
            identity_session_id: format!("identity-{id}"),
            expires_at_ms: 1_000_000,
        }
    }
    fn attach(state: &mut RelayState, value: Admission) -> mpsc::Receiver<RelayDelivery> {
        let (tx, rx) = mpsc::channel(OUTPUT_CAPACITY);
        state.handle(Command::Attach(value, tx), 100).unwrap();
        rx
    }
    fn request(id: &str) -> OwnerRequest {
        OwnerRequest {
            request_id: id.into(),
            resident_id: Some("alo".into()),
            operation: Operation::Teach,
            payload: json!({"expectedPolicyVersion":1}),
        }
    }
    fn register(state: &mut RelayState, id: &str) {
        state
            .handle(
                Command::Register(
                    id.into(),
                    RuntimeRegister {
                        request_id: "register-1".into(),
                        resident_id: "alo".into(),
                        archive_id: "archive-1".into(),
                    },
                ),
                100,
            )
            .unwrap();
    }
    fn fixture() -> (
        RelayState,
        mpsc::Receiver<RelayDelivery>,
        mpsc::Receiver<RelayDelivery>,
    ) {
        let mut state = RelayState::new(Arc::new(policy()));
        let owner = attach(&mut state, admission("owner", "human", 1));
        let mut actor = attach(&mut state, admission("actor", "actor", 2));
        register(&mut state, "actor");
        assert_eq!(actor.try_recv().unwrap().event["payload"]["status"], "ok");
        (state, owner, actor)
    }

    #[test]
    fn configured_policy_is_strict_exact_and_never_uses_realm_fallback() {
        let mut value = serde_json::to_value(policy()).unwrap();
        assert!(CompanionPolicy::parse(&serde_json::to_vec(&value).unwrap(), "other").is_err());
        assert!(CompanionPolicy::parse(&serde_json::to_vec(&value).unwrap(), "").is_err());
        value["enrollments"][0]["actor"]["role"] = json!("ai");
        assert!(CompanionPolicy::parse(&serde_json::to_vec(&value).unwrap(), "trial").is_err());
        let mut value = serde_json::to_value(policy()).unwrap();
        let duplicate = value["enrollments"][0].clone();
        value["enrollments"].as_array_mut().unwrap().push(duplicate);
        assert!(CompanionPolicy::parse(&serde_json::to_vec(&value).unwrap(), "trial").is_err());
        assert!(!valid_ref("账号"));
        assert!(!valid_ref(&"a".repeat(101)));
        assert_eq!(
            owner_ref("human"),
            format!("owner:{}", hex(&Sha256::digest(b"human")))
        );
    }

    #[test]
    fn natural_instructions_are_private_versioned_and_bounded_without_extra_authority() {
        let mut input = request("instruct-1");
        input.operation = Operation::Instruct;
        input.payload = json!({"text":"在附近打两只怪，再回来找我，途中别捡东西。", "expectedPolicyVersion":1});
        assert!(input.validate());
        assert_eq!(input.operation.name(), "instruct");
        assert!(!format!("{input:?}").contains("两只"));
        assert_eq!(serde_json::from_value::<Operation>(json!("instruct")).unwrap(), Operation::Instruct);
        for payload in [
            json!({"text":"", "expectedPolicyVersion":1}),
            json!({"text":" 跟着我", "expectedPolicyVersion":1}),
            json!({"text":"跟着我\n打怪", "expectedPolicyVersion":1}),
            json!({"text":"怪".repeat(1001), "expectedPolicyVersion":1}),
            json!({"text":"跟着我", "expectedPolicyVersion":0}),
            json!({"text":"跟着我", "expectedPolicyVersion":1,"ownerRef":"forged"}),
            json!({"text":"跟着我", "expectedPolicyVersion":1,"apiKey":"forbidden"}),
        ] { input.payload = payload; assert!(!input.validate()); }
    }

    #[test]
    fn owner_payloads_and_trial_intents_are_strict_and_bounded() {
        assert!(request("teach-1").validate());
        let mut input = request("teach-1");
        input.payload["owner"] = json!("human");
        assert!(!input.validate());
        let mut input = request("trial-1");
        input.operation = Operation::TrialEvent;
        for payload in [
            json!({"kind":"visit","visitId":"visit-1","entrySource":"unknown","reminderCount":0}),
            json!({"kind":"visitEnd","visitId":"visit-1"}),
            json!({"kind":"choice","visitId":"visit-1","reason":"shared-history"}),
            json!({"kind":"teachingCheck","visitId":"visit-1","understood":true}),
            json!({"kind":"purchaseIntent","visitId":"visit-1","offerId":"offer-1"}),
        ] {
            input.payload = payload;
            assert!(input.validate());
        }
        input.payload = json!({"kind":"payment","visitId":"visit-1","amount":1});
        assert!(!input.validate());
        input.payload = json!({"kind":"visitEnd","visitId":"visit-1","confirmed":true});
        assert!(!input.validate());
        input.payload = json!({"kind":"choice","visitId":"visit-1","reason":"other","reasonText":"x".repeat(301)});
        assert!(!input.validate());
    }

    #[test]
    fn exact_enrollment_and_original_owner_admission_authorize_round_trip() {
        let (mut state, mut owner, mut actor) = fixture();
        let mut outsider = attach(&mut state, admission("outsider", "other", 1));
        state
            .handle(Command::Control("outsider".into(), request("same-id")), 101)
            .unwrap();
        assert_eq!(
            outsider.try_recv().unwrap().event["payload"]["status"],
            "denied"
        );
        assert!(actor.try_recv().is_err());
        state
            .handle(Command::Control("owner".into(), request("same-id")), 102)
            .unwrap();
        let command = actor.try_recv().unwrap();
        assert!(state.delivery_current(&command, 103));
        assert_eq!(command.event["payload"]["ownerRef"], owner_ref("human"));
        let runtime_id = command.event["payload"]["requestId"]
            .as_str()
            .unwrap()
            .to_string();
        assert_ne!(runtime_id, "same-id");
        assert!(valid_ref(&runtime_id));
        let reply = RuntimeReply {
            request_id: runtime_id,
            generation: 1,
            status: ReplyStatus::Ok,
            result: json!({"policyVersion":2}),
        };
        assert!(state
            .handle(Command::Reply("outsider".into(), reply.clone()), 103)
            .is_err());
        state
            .handle(Command::Reply("actor".into(), reply), 104)
            .unwrap();
        let result = owner.try_recv().unwrap();
        assert_eq!(result.event["payload"]["requestId"], "same-id");
        assert_eq!(result.event["payload"]["result"]["policyVersion"], 2);
        state.handle(Command::Detach("owner".into()), 105).unwrap();
        assert!(!state.delivery_current(&result, 106));
    }

    #[test]
    fn runtime_generation_disconnect_and_explicit_retry_keep_stable_request_namespace() {
        let (mut state, mut owner, mut actor) = fixture();
        state
            .handle(Command::Control("owner".into(), request("retry")), 101)
            .unwrap();
        let first = actor.try_recv().unwrap();
        let runtime_id = first.event["payload"]["requestId"]
            .as_str()
            .unwrap()
            .to_string();
        state.handle(Command::Detach("actor".into()), 102).unwrap();
        assert_eq!(
            owner.try_recv().unwrap().event["payload"]["status"],
            "unknown"
        );
        assert!(!state.delivery_current(&first, 103));
        let mut replacement = attach(&mut state, admission("actor-new", "actor", 2));
        register(&mut state, "actor-new");
        assert_eq!(
            replacement.try_recv().unwrap().event["payload"]["result"]["generation"],
            2
        );
        state
            .handle(Command::Control("owner".into(), request("retry")), 104)
            .unwrap();
        let second = replacement.try_recv().unwrap();
        assert_eq!(second.event["payload"]["requestId"], runtime_id);
        assert!(state
            .handle(
                Command::Reply(
                    "actor".into(),
                    RuntimeReply {
                        request_id: runtime_id.clone(),
                        generation: 1,
                        status: ReplyStatus::Ok,
                        result: json!({})
                    }
                ),
                105
            )
            .is_err());
        let mut conflicting = request("retry");
        conflicting.payload = json!({"expectedPolicyVersion":2});
        state
            .handle(Command::Control("owner".into(), conflicting), 106)
            .unwrap();
        assert_eq!(
            owner.try_recv().unwrap().event["payload"]["status"],
            "conflict"
        );
        state.expire(20_000);
        assert_eq!(
            owner.try_recv().unwrap().event["payload"]["status"],
            "unknown"
        );
    }

    #[test]
    fn duplicate_runtime_and_owner_change_do_not_transfer_admissions() {
        let (mut state, mut owner, mut actor) = fixture();
        let mut duplicate = attach(&mut state, admission("duplicate", "actor", 2));
        register(&mut state, "duplicate");
        assert_eq!(
            duplicate.try_recv().unwrap().event["payload"]["status"],
            "conflict"
        );
        state
            .handle(Command::Control("owner".into(), request("old")), 101)
            .unwrap();
        let old = actor.try_recv().unwrap();
        state.handle(Command::Detach("owner".into()), 102).unwrap();
        let _new_owner = attach(&mut state, admission("owner-new", "human", 1));
        assert!(!state.delivery_current(&old, 103));
        let id = old.event["payload"]["requestId"].as_str().unwrap();
        state
            .handle(
                Command::Reply(
                    "actor".into(),
                    RuntimeReply {
                        request_id: id.into(),
                        generation: 1,
                        status: ReplyStatus::Ok,
                        result: json!({"private":"old"}),
                    },
                ),
                104,
            )
            .unwrap();
        assert!(owner.try_recv().is_err());
        assert!(state.pending.is_empty());
    }

    #[test]
    fn pending_limits_and_output_failure_fail_closed_without_unbounded_buffers() {
        let (mut state, mut owner, actor) = fixture();
        for index in 0..8 {
            state
                .handle(
                    Command::Control("owner".into(), request(&format!("op-{index}"))),
                    101,
                )
                .unwrap();
        }
        state
            .handle(Command::Control("owner".into(), request("ninth")), 102)
            .unwrap();
        assert_eq!(
            owner.try_recv().unwrap().event["payload"]["status"],
            "error"
        );
        assert_eq!(state.pending.len(), 8);
        drop(actor);
        state.expire(103);
        assert!(!state.runtimes.contains_key("alo"));
        assert!(state.pending.is_empty());
        for _ in 0..8 {
            assert_eq!(
                owner.try_recv().unwrap().event["payload"]["status"],
                "unknown"
            );
        }
        let mut list = request("list");
        list.operation = Operation::List;
        list.resident_id = None;
        list.payload = json!({});
        state
            .handle(Command::Control("owner".into(), list), 104)
            .unwrap();
        assert_eq!(
            owner.try_recv().unwrap().event["payload"]["result"]["companions"][0]["online"],
            false
        );
    }

    #[test]
    fn export_results_cannot_exceed_frame_or_chunk_contract() {
        let mut input = request("export");
        input.operation = Operation::ExportBegin;
        input.payload = json!({});
        let mut output =
            json!({"exportId":"export-1","totalBytes":1,"sha256":"a".repeat(64),"chunkCount":1});
        assert!(valid_export_result(&input, &output));
        output["totalBytes"] = json!(2 * 1_024 * 1_024 + 1);
        assert!(!valid_export_result(&input, &output));
        input.operation = Operation::ExportChunk;
        input.payload = json!({"exportId":"export-1","index":0});
        let output = json!({"exportId":"export-1","index":0,"dataBase64":"eA==","chunkCount":1});
        assert!(valid_export_result(&input, &output));
        let mut foreign = output.clone();
        foreign["exportId"] = json!("foreign");
        assert!(!valid_export_result(&input, &foreign));
        let mut huge = output;
        huge["dataBase64"] =
            json!(base64::engine::general_purpose::STANDARD.encode(vec![0; 16 * 1_024 + 1]));
        assert!(!valid_export_result(&input, &huge));
    }

    #[test]
    fn explicit_timeout_retry_does_not_revive_old_queued_delivery() {
        let (mut state, mut owner, mut actor) = fixture();
        state
            .handle(
                Command::Control("owner".into(), request("timeout-retry")),
                101,
            )
            .unwrap();
        let old = actor.try_recv().unwrap();
        state.expire(20_000);
        assert_eq!(
            owner.try_recv().unwrap().event["payload"]["status"],
            "unknown"
        );
        state
            .handle(
                Command::Control("owner".into(), request("timeout-retry")),
                20_001,
            )
            .unwrap();
        let current = actor.try_recv().unwrap();
        assert_eq!(
            old.event["payload"]["requestId"],
            current.event["payload"]["requestId"]
        );
        assert!(!state.delivery_current(&old, 20_002));
        assert!(state.delivery_current(&current, 20_002));
    }

    #[test]
    fn runtime_unknown_is_forwarded_without_error_text_or_automatic_replay() {
        let (mut state, mut owner, mut actor) = fixture();
        state
            .handle(
                Command::Control("owner".into(), request("committed-unknown")),
                101,
            )
            .unwrap();
        let command = actor.try_recv().unwrap();
        state
            .handle(
                Command::Reply(
                    "actor".into(),
                    RuntimeReply {
                        request_id: command.event["payload"]["requestId"]
                            .as_str()
                            .unwrap()
                            .into(),
                        generation: 1,
                        status: ReplyStatus::Unknown,
                        result: json!({"message":"private internal error"}),
                    },
                ),
                102,
            )
            .unwrap();
        let result = owner.try_recv().unwrap();
        assert_eq!(result.event["payload"]["status"], "unknown");
        assert_eq!(result.event["payload"]["result"], json!({}));
        assert!(state.pending.is_empty());
        assert!(actor.try_recv().is_err());
    }

    #[test]
    fn full_runtime_output_revokes_registration_and_resolves_pending_unknown() {
        let (mut state, mut owner, _actor) = fixture();
        for _ in 0..OUTPUT_CAPACITY {
            register(&mut state, "actor");
        }
        assert!(state.runtimes.contains_key("alo"));
        state
            .handle(Command::Control("owner".into(), request("full")), 101)
            .unwrap();
        assert!(!state.runtimes.contains_key("alo"));
        assert!(!state.admissions.contains_key("actor"));
        assert!(state.pending.is_empty());
        assert_eq!(
            owner.try_recv().unwrap().event["payload"]["status"],
            "unknown"
        );
    }

    #[test]
    fn registration_never_accepts_owner_role_or_foreign_archive_declarations() {
        let mut state = RelayState::new(Arc::new(policy()));
        let mut owner = attach(&mut state, admission("owner", "human", 1));
        let mut actor = attach(&mut state, admission("actor", "actor", 2));
        register(&mut state, "owner");
        assert_eq!(
            owner.try_recv().unwrap().event["payload"]["status"],
            "denied"
        );
        state
            .handle(
                Command::Register(
                    "actor".into(),
                    RuntimeRegister {
                        request_id: "reg".into(),
                        resident_id: "alo".into(),
                        archive_id: "foreign".into(),
                    },
                ),
                101,
            )
            .unwrap();
        assert_eq!(
            actor.try_recv().unwrap().event["payload"]["status"],
            "denied"
        );
        assert!(state.runtimes.is_empty());
    }
}
