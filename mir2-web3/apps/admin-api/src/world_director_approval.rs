use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mir2_gateway::{
    CommonwareControlLog, DirectorPolicyState, DirectorPressureScores, DirectorProposal,
    DirectorProposalSource, FinalizedControlBlock, FinalizedDirectorInstallReceipt,
    FinalizedDirectorSubmission, Gate14Command, Gate14CommandEnvelope, NodeSigningIdentity,
    SignedDirectorCommand, WorldDirectorPolicy, WorldDirectorRuntimeStatus, WorldTelemetrySnapshot,
};
use postgres::{Client, NoTls};
use reqwest::blocking::Client as HttpClient;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const CHECKPOINT_VERSION: u32 = 1;
const DEFAULT_COMMAND_VALIDITY_MS: u64 = 90 * 60 * 1_000;
const MAX_AUDIT_RECORDS: usize = 10_000;
static ATOMIC_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectorApprovalStatus {
    PendingApproval,
    Finalizing,
    Executing,
    Completed,
    Rejected,
    Cancelled,
    Failed,
}

impl DirectorApprovalStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::PendingApproval => "pending_approval",
            Self::Finalizing => "finalizing",
            Self::Executing => "executing",
            Self::Completed => "completed",
            Self::Rejected => "rejected",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectorRiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorApprovalRecord {
    pub proposal_id: String,
    pub status: DirectorApprovalStatus,
    pub risk_level: DirectorRiskLevel,
    pub snapshot: WorldTelemetrySnapshot,
    pub pressure_scores: DirectorPressureScores,
    pub proposal: DirectorProposal,
    pub requested_by: String,
    pub requested_at_ms: u64,
    pub decided_by: Option<String>,
    pub decision_reason: Option<String>,
    pub decided_at_ms: Option<u64>,
    pub command_id: Option<String>,
    pub finalized_height: Option<u64>,
    pub finalized_digest: Option<String>,
    #[serde(default)]
    pub commonware_network_height: Option<u64>,
    #[serde(default)]
    pub commonware_network_state_root: Option<String>,
    #[serde(default)]
    pub commonware_network_command_digest: Option<String>,
    #[serde(default)]
    pub approval_audit_hash: Option<String>,
    pub zone_receipts: Vec<Value>,
    pub last_error: Option<String>,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorProposalPatch {
    pub duration_ms: Option<u64>,
    pub reward_budget: Option<u64>,
    pub target_zones: Option<BTreeSet<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorAuditRecord {
    pub audit_id: String,
    pub proposal_id: Option<String>,
    pub action: String,
    pub actor_id: String,
    pub from_status: Option<String>,
    pub to_status: Option<String>,
    pub reason: String,
    pub occurred_at_ms: u64,
    pub previous_hash: String,
    pub record_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorApprovalConfiguration {
    pub execution_configured: bool,
    pub persistence: String,
    pub director_public_key: Option<String>,
    pub committee_size: usize,
    pub zone_host_count: usize,
    pub automatic_generation_enabled: bool,
    pub generation_interval_seconds: u64,
    pub remote_commonware_configured: bool,
    pub remote_commonware_required: bool,
    pub proposal_generator: String,
    pub ai_configured: bool,
    pub ai_provider: Option<String>,
    pub ai_model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorApprovalDashboard {
    pub schema: String,
    pub generated_at_ms: u64,
    pub paused: bool,
    pub pause_reason: Option<String>,
    pub proposals: Vec<DirectorApprovalRecord>,
    pub audit: Vec<DirectorAuditRecord>,
    pub configuration: DirectorApprovalConfiguration,
    pub runtime_statuses: Vec<DirectorRuntimeTargetStatus>,
    pub pending_count: usize,
    pub active_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorRuntimeTargetStatus {
    pub endpoint: String,
    pub status: String,
    pub error: Option<String>,
    pub runtime: Option<WorldDirectorRuntimeStatus>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DurablePolicyState {
    reward_budget_spent_today: u64,
    active_event_ids: BTreeSet<String>,
    template_last_finished_at_ms: BTreeMap<String, u64>,
    budget_day: u64,
}

impl DurablePolicyState {
    fn policy_state(&self) -> DirectorPolicyState {
        DirectorPolicyState {
            reward_budget_spent_today: self.reward_budget_spent_today,
            active_event_ids: self.active_event_ids.clone(),
            template_last_finished_at_ms: self.template_last_finished_at_ms.clone(),
        }
    }

    fn roll_day(&mut self, now_ms: u64) {
        let day = now_ms / 86_400_000;
        if self.budget_day != day {
            self.budget_day = day;
            self.reward_budget_spent_today = 0;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DirectorApprovalCheckpoint {
    version: u32,
    #[serde(default)]
    revision: u64,
    paused: bool,
    pause_reason: Option<String>,
    proposals: BTreeMap<String, DirectorApprovalRecord>,
    #[serde(default = "genesis_audit_hash")]
    audit_base_hash: String,
    #[serde(default)]
    archived_audit_records: u64,
    audit: Vec<DirectorAuditRecord>,
    finalized: Vec<FinalizedControlBlock>,
    policy_state: DurablePolicyState,
}

impl Default for DirectorApprovalCheckpoint {
    fn default() -> Self {
        Self {
            version: CHECKPOINT_VERSION,
            revision: 0,
            paused: false,
            pause_reason: None,
            proposals: BTreeMap::new(),
            audit_base_hash: genesis_audit_hash(),
            archived_audit_records: 0,
            audit: Vec::new(),
            finalized: Vec::new(),
            policy_state: DurablePolicyState::default(),
        }
    }
}

#[derive(Clone)]
enum DirectorRepository {
    Postgres(String),
    File(PathBuf),
}

impl DirectorRepository {
    fn from_env() -> Self {
        env::var("ADMIN_DATABASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(Self::Postgres)
            .unwrap_or_else(|| {
                Self::File(PathBuf::from(
                    env::var("MIR2_WORLD_DIRECTOR_APPROVAL_FILE")
                        .unwrap_or_else(|_| ".mir2-data/world-director-approval.json".to_string()),
                ))
            })
    }

    fn label(&self) -> String {
        match self {
            Self::Postgres(_) => "postgres".to_string(),
            Self::File(path) => format!("atomic_json:{}", path.display()),
        }
    }

    fn load(&self) -> Result<DirectorApprovalCheckpoint, String> {
        let checkpoint = match self {
            Self::Postgres(database_url) => {
                let mut client = Client::connect(database_url, NoTls)
                    .map_err(|error| format!("world director postgres connect failed: {error}"))?;
                client
                    .query_opt(
                        "SELECT checkpoint_json, revision \
                         FROM world_director_control_state WHERE singleton = TRUE",
                        &[],
                    )
                    .map_err(|error| format!("world director checkpoint query failed: {error}"))?
                    .map(|row| {
                        let mut checkpoint = serde_json::from_value::<DirectorApprovalCheckpoint>(
                            row.get::<_, Value>("checkpoint_json"),
                        )?;
                        checkpoint.revision = row.get::<_, i64>("revision").max(0) as u64;
                        Ok::<_, serde_json::Error>(checkpoint)
                    })
                    .transpose()
                    .map_err(|error| format!("world director checkpoint decode failed: {error}"))?
                    .unwrap_or_default()
            }
            Self::File(path) if path.exists() => {
                let bytes = fs::read(path).map_err(|error| {
                    format!(
                        "failed to read world director approval checkpoint {}: {error}",
                        path.display()
                    )
                })?;
                serde_json::from_slice(&bytes)
                    .map_err(|error| format!("world director checkpoint decode failed: {error}"))?
            }
            Self::File(_) => DirectorApprovalCheckpoint::default(),
        };
        validate_checkpoint(&checkpoint)?;
        Ok(checkpoint)
    }

    fn save(&self, checkpoint: &mut DirectorApprovalCheckpoint, now_ms: u64) -> Result<(), String> {
        validate_checkpoint(checkpoint)?;
        let expected_revision = checkpoint.revision;
        let next_revision = expected_revision
            .checked_add(1)
            .ok_or_else(|| "world director checkpoint revision overflow".to_string())?;
        let mut next_checkpoint = checkpoint.clone();
        next_checkpoint.revision = next_revision;
        match self {
            Self::Postgres(database_url) => {
                let mut client = Client::connect(database_url, NoTls)
                    .map_err(|error| format!("world director postgres connect failed: {error}"))?;
                let mut transaction = client
                    .transaction()
                    .map_err(|error| format!("world director transaction begin failed: {error}"))?;
                let checkpoint_json = serde_json::to_value(&next_checkpoint)
                    .map_err(|error| format!("world director checkpoint encode failed: {error}"))?;
                let updated = transaction
                    .execute(
                        "UPDATE world_director_control_state SET \
                         checkpoint_json = $1, revision = $2, updated_at_ms = $3, \
                         updated_at = now() \
                         WHERE singleton = TRUE AND revision = $4",
                        &[
                            &checkpoint_json,
                            &(next_revision as i64),
                            &(now_ms as i64),
                            &(expected_revision as i64),
                        ],
                    )
                    .map_err(|error| format!("world director checkpoint update failed: {error}"))?;
                let inserted = if updated == 0 && expected_revision == 0 {
                    transaction
                        .execute(
                            "INSERT INTO world_director_control_state \
                             (singleton, checkpoint_json, revision, updated_at_ms) \
                             VALUES (TRUE, $1, $2, $3) ON CONFLICT (singleton) DO NOTHING",
                            &[&checkpoint_json, &(next_revision as i64), &(now_ms as i64)],
                        )
                        .map_err(|error| {
                            format!("world director checkpoint insert failed: {error}")
                        })?
                } else {
                    0
                };
                if updated == 0 && inserted == 0 {
                    return Err(
                        "world director checkpoint changed concurrently; retry the operation"
                            .to_string(),
                    );
                }
                for audit in &next_checkpoint.audit {
                    transaction
                        .execute(
                            "INSERT INTO world_director_audit \
                             (audit_id, proposal_id, action, actor_id, from_status, to_status, \
                              reason, occurred_at_ms, previous_hash, record_hash) \
                             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) \
                             ON CONFLICT (audit_id) DO NOTHING",
                            &[
                                &audit.audit_id,
                                &audit.proposal_id,
                                &audit.action,
                                &audit.actor_id,
                                &audit.from_status,
                                &audit.to_status,
                                &audit.reason,
                                &(audit.occurred_at_ms as i64),
                                &audit.previous_hash,
                                &audit.record_hash,
                            ],
                        )
                        .map_err(|error| format!("world director audit append failed: {error}"))?;
                }
                transaction.commit().map_err(|error| {
                    format!("world director transaction commit failed: {error}")
                })?;
                checkpoint.revision = next_revision;
                Ok(())
            }
            Self::File(path) => {
                atomic_json(path, &next_checkpoint)?;
                checkpoint.revision = next_revision;
                Ok(())
            }
        }
    }

    fn refresh_if_shared(&self, checkpoint: &mut DirectorApprovalCheckpoint) -> Result<(), String> {
        if matches!(self, Self::Postgres(_)) {
            *checkpoint = self.load()?;
        }
        Ok(())
    }
}

#[derive(Clone)]
enum DirectorDelivery {
    Disabled,
    Http {
        endpoints: Vec<String>,
        management_token: String,
    },
    #[cfg(test)]
    Test,
}

#[derive(Clone)]
enum DirectorProposalGenerator {
    RuleEngine,
    OpenAiResponses {
        endpoint: String,
        api_key: String,
        model: String,
        reasoning_effort: String,
        timeout_seconds: u64,
    },
}

impl DirectorProposalGenerator {
    fn from_env() -> Result<Self, String> {
        let mode = env::var("MIR2_WORLD_DIRECTOR_PROPOSAL_MODE")
            .unwrap_or_else(|_| "rule".to_string())
            .trim()
            .to_ascii_lowercase();
        match mode.as_str() {
            "rule" | "rules" | "rule_engine" => Ok(Self::RuleEngine),
            "openai" | "openai_responses" => {
                let api_key = env::var("MIR2_WORLD_DIRECTOR_AI_API_KEY")
                    .or_else(|_| env::var("OPENAI_API_KEY"))
                    .ok()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| {
                        "OpenAI proposal mode requires MIR2_WORLD_DIRECTOR_AI_API_KEY or OPENAI_API_KEY"
                            .to_string()
                    })?;
                let endpoint = env::var("MIR2_WORLD_DIRECTOR_AI_ENDPOINT")
                    .unwrap_or_else(|_| "https://api.openai.com/v1/responses".to_string())
                    .trim()
                    .to_string();
                if !endpoint.starts_with("https://") {
                    return Err("MIR2_WORLD_DIRECTOR_AI_ENDPOINT must use HTTPS".to_string());
                }
                let model = env::var("MIR2_WORLD_DIRECTOR_AI_MODEL")
                    .unwrap_or_else(|_| "gpt-5.6-terra".to_string())
                    .trim()
                    .to_string();
                if model.is_empty() {
                    return Err("MIR2_WORLD_DIRECTOR_AI_MODEL must not be empty".to_string());
                }
                let reasoning_effort = env::var("MIR2_WORLD_DIRECTOR_AI_REASONING_EFFORT")
                    .unwrap_or_else(|_| "low".to_string())
                    .trim()
                    .to_ascii_lowercase();
                if !matches!(
                    reasoning_effort.as_str(),
                    "none" | "low" | "medium" | "high" | "xhigh" | "max"
                ) {
                    return Err(
                        "MIR2_WORLD_DIRECTOR_AI_REASONING_EFFORT must be none, low, medium, high, xhigh, or max"
                            .to_string(),
                    );
                }
                let timeout_seconds = env::var("MIR2_WORLD_DIRECTOR_AI_TIMEOUT_SECONDS")
                    .ok()
                    .and_then(|value| value.trim().parse::<u64>().ok())
                    .unwrap_or(45)
                    .clamp(5, 120);
                Ok(Self::OpenAiResponses {
                    endpoint,
                    api_key,
                    model,
                    reasoning_effort,
                    timeout_seconds,
                })
            }
            _ => Err("MIR2_WORLD_DIRECTOR_PROPOSAL_MODE must be rule or openai".to_string()),
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::RuleEngine => "rule_engine",
            Self::OpenAiResponses { .. } => "openai_responses",
        }
    }

    fn ai_identity(&self) -> Option<(&'static str, &str)> {
        match self {
            Self::RuleEngine => None,
            Self::OpenAiResponses { model, .. } => Some(("openai", model)),
        }
    }
}

#[derive(Clone)]
struct DirectorApprovalConfig {
    repository: DirectorRepository,
    director: Option<NodeSigningIdentity>,
    validators: Vec<NodeSigningIdentity>,
    delivery: DirectorDelivery,
    automatic_generation_enabled: bool,
    generation_interval_seconds: u64,
    commonware_gateway_url: Option<String>,
    commonware_gateway_token: Option<String>,
    require_remote_commonware: bool,
    proposal_generator: DirectorProposalGenerator,
}

impl DirectorApprovalConfig {
    fn from_env() -> Result<Self, String> {
        let repository = DirectorRepository::from_env();
        let director = identity_from_pair(
            "MIR2_WORLD_DIRECTOR_SIGNING_KEY",
            "MIR2_WORLD_DIRECTOR_SIGNING_KEY_FILE",
        )?;
        let validators = validator_identities_from_env()?;
        if !validators.is_empty() && validators.len() < 4 {
            return Err("world director requires at least four validator identities".to_string());
        }
        let endpoints = env::var("MIR2_WORLD_DIRECTOR_ZONE_HOST_URLS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.trim_end_matches('/').to_string())
            .collect::<Vec<_>>();
        let management_token = env::var("MIR2_WORLD_DIRECTOR_MANAGEMENT_TOKEN")
            .unwrap_or_default()
            .trim()
            .to_string();
        if endpoints.is_empty() != management_token.is_empty() {
            return Err(
                "configure both MIR2_WORLD_DIRECTOR_ZONE_HOST_URLS and MIR2_WORLD_DIRECTOR_MANAGEMENT_TOKEN"
                    .to_string(),
            );
        }
        let delivery = if endpoints.is_empty() {
            DirectorDelivery::Disabled
        } else {
            DirectorDelivery::Http {
                endpoints,
                management_token,
            }
        };
        let commonware_gateway_url = env::var("MIR2_WORLD_DIRECTOR_COMMONWARE_GATEWAY_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(|value| value.trim_end_matches('/').to_string());
        let commonware_gateway_token = env::var("MIR2_WORLD_DIRECTOR_COMMONWARE_GATEWAY_TOKEN")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let require_remote_commonware = env::var("MIR2_WORLD_DIRECTOR_REQUIRE_REMOTE_COMMONWARE")
            .ok()
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes"
                )
            })
            .unwrap_or_else(deployment_is_production);
        if deployment_is_production()
            && commonware_gateway_url.is_some()
            && commonware_gateway_token.is_none()
        {
            return Err(
                "production World Director requires MIR2_WORLD_DIRECTOR_COMMONWARE_GATEWAY_TOKEN"
                    .to_string(),
            );
        }
        Ok(Self {
            repository,
            director,
            validators,
            delivery,
            automatic_generation_enabled: env_flag("MIR2_WORLD_DIRECTOR_AUTOMATIC_GENERATION"),
            generation_interval_seconds: env::var(
                "MIR2_WORLD_DIRECTOR_GENERATION_INTERVAL_SECONDS",
            )
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .unwrap_or(300)
            .clamp(30, 86_400),
            commonware_gateway_url,
            commonware_gateway_token,
            require_remote_commonware,
            proposal_generator: DirectorProposalGenerator::from_env()?,
        })
    }

    fn execution_configured(&self) -> bool {
        self.director.is_some()
            && self.validators.len() >= 4
            && !matches!(self.delivery, DirectorDelivery::Disabled)
            && (!self.require_remote_commonware || self.commonware_gateway_url.is_some())
    }

    fn zone_host_count(&self) -> usize {
        match &self.delivery {
            DirectorDelivery::Disabled => 0,
            DirectorDelivery::Http { endpoints, .. } => endpoints.len(),
            #[cfg(test)]
            DirectorDelivery::Test => 1,
        }
    }
}

struct DirectorApprovalInner {
    config: DirectorApprovalConfig,
    checkpoint: Mutex<DirectorApprovalCheckpoint>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AiDirectorDecision {
    should_propose: bool,
    template_id: String,
    target_zones: Vec<String>,
    duration_ms: u64,
    reward_budget: u64,
    rationale: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponseEnvelope {
    #[serde(default)]
    status: String,
    #[serde(default)]
    output: Vec<OpenAiOutputItem>,
    incomplete_details: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct OpenAiOutputItem {
    #[serde(default)]
    content: Vec<OpenAiContentItem>,
}

#[derive(Debug, Deserialize)]
struct OpenAiContentItem {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
    refusal: Option<String>,
}

#[derive(Clone)]
pub struct WorldDirectorApprovalService {
    inner: Arc<DirectorApprovalInner>,
}

impl WorldDirectorApprovalService {
    pub fn from_env() -> Result<Self, String> {
        Self::new(DirectorApprovalConfig::from_env()?)
    }

    fn new(config: DirectorApprovalConfig) -> Result<Self, String> {
        let checkpoint = config.repository.load()?;
        validate_finalized_chain(&checkpoint, &config.validators)?;
        Ok(Self {
            inner: Arc::new(DirectorApprovalInner {
                config,
                checkpoint: Mutex::new(checkpoint),
            }),
        })
    }

    pub fn automatic_generation_enabled(&self) -> bool {
        self.inner.config.automatic_generation_enabled
    }

    pub fn generation_interval(&self) -> Duration {
        Duration::from_secs(self.inner.config.generation_interval_seconds)
    }

    pub fn recover_inflight(&self, now_ms: u64) -> Result<usize, String> {
        let proposal_ids = {
            let mut checkpoint = self.lock_checkpoint()?;
            self.refresh_checkpoint(&mut checkpoint)?;
            checkpoint
                .proposals
                .values()
                .filter(|record| record.status == DirectorApprovalStatus::Finalizing)
                .map(|record| record.proposal_id.clone())
                .collect::<Vec<_>>()
        };
        let mut recovered = 0;
        for proposal_id in proposal_ids {
            self.recover_finalizing_proposal(&proposal_id, now_ms)?;
            recovered += 1;
        }
        Ok(recovered)
    }

    pub fn generate(
        &self,
        snapshot: WorldTelemetrySnapshot,
        requested_by: impl Into<String>,
        now_ms: u64,
    ) -> Result<Option<DirectorApprovalRecord>, String> {
        snapshot.validate()?;
        let scores = DirectorPressureScores::from_snapshot(&snapshot)?;
        let policy = WorldDirectorPolicy::mir2_default();
        let Some(proposal) = generate_director_proposal(
            &self.inner.config.proposal_generator,
            &policy,
            &snapshot,
            &scores,
            now_ms,
        )?
        else {
            return Ok(None);
        };
        let requested_by = requested_by.into();
        let mut checkpoint = self.lock_checkpoint()?;
        self.refresh_checkpoint(&mut checkpoint)?;
        refresh_lifecycle(&mut checkpoint, now_ms);
        checkpoint.policy_state.roll_day(now_ms);
        policy.approve(
            &proposal,
            &snapshot,
            &scores,
            &checkpoint.policy_state.policy_state(),
            now_ms,
        )?;
        if let Some(existing) = checkpoint.proposals.get(&proposal.proposal_id) {
            return Ok(Some(existing.clone()));
        }
        if let Some(existing) = checkpoint.proposals.values().find(|record| {
            record.proposal.template_id == proposal.template_id
                && matches!(
                    record.status,
                    DirectorApprovalStatus::PendingApproval
                        | DirectorApprovalStatus::Finalizing
                        | DirectorApprovalStatus::Executing
                )
        }) {
            return Ok(Some(existing.clone()));
        }
        let record = DirectorApprovalRecord {
            proposal_id: proposal.proposal_id.clone(),
            status: DirectorApprovalStatus::PendingApproval,
            risk_level: proposal_risk(&proposal),
            snapshot,
            pressure_scores: scores,
            proposal,
            requested_by,
            requested_at_ms: now_ms,
            decided_by: None,
            decision_reason: None,
            decided_at_ms: None,
            command_id: None,
            finalized_height: None,
            finalized_digest: None,
            commonware_network_height: None,
            commonware_network_state_root: None,
            commonware_network_command_digest: None,
            approval_audit_hash: None,
            zone_receipts: Vec::new(),
            last_error: None,
            updated_at_ms: now_ms,
        };
        checkpoint
            .proposals
            .insert(record.proposal_id.clone(), record.clone());
        append_audit(
            &mut checkpoint,
            Some(record.proposal_id.clone()),
            "proposal.generated",
            "world-director-engine",
            None,
            Some(record.status),
            &record.proposal.rationale,
            now_ms,
        )?;
        self.persist(&mut checkpoint, now_ms)?;
        Ok(Some(record))
    }

    pub fn approve(
        &self,
        proposal_id: &str,
        operator_id: &str,
        reason: &str,
        now_ms: u64,
    ) -> Result<DirectorApprovalRecord, String> {
        require_reason(reason)?;
        let (submission, delivery_target, command_id, approval_audit_hash) = {
            let mut checkpoint = self.lock_checkpoint()?;
            self.refresh_checkpoint(&mut checkpoint)?;
            refresh_lifecycle(&mut checkpoint, now_ms);
            checkpoint.policy_state.roll_day(now_ms);
            if checkpoint.paused {
                return Err(format!(
                    "world director is paused: {}",
                    checkpoint.pause_reason.as_deref().unwrap_or("no reason")
                ));
            }
            let existing = checkpoint
                .proposals
                .get(proposal_id)
                .cloned()
                .ok_or_else(|| format!("world director proposal not found: {proposal_id}"))?;
            if existing.status != DirectorApprovalStatus::PendingApproval {
                return Err(format!(
                    "proposal {proposal_id} is {}, expected pending_approval",
                    existing.status.as_str()
                ));
            }
            let director =
                self.inner.config.director.as_ref().ok_or_else(|| {
                    "world director signing identity is not configured".to_string()
                })?;
            if self.inner.config.validators.len() < 4 {
                return Err("world director validator committee is not configured".to_string());
            }
            if matches!(self.inner.config.delivery, DirectorDelivery::Disabled) {
                return Err("world director Zone Host delivery is not configured".to_string());
            }
            if self.inner.config.require_remote_commonware
                && self.inner.config.commonware_gateway_url.is_none()
            {
                return Err("remote Commonware gateway is required but not configured".to_string());
            }
            let policy = WorldDirectorPolicy::mir2_default();
            let plan = policy.approve(
                &existing.proposal,
                &existing.snapshot,
                &existing.pressure_scores,
                &checkpoint.policy_state.policy_state(),
                now_ms,
            )?;
            let command = SignedDirectorCommand::issue(
                &plan,
                &existing.snapshot,
                director,
                DEFAULT_COMMAND_VALIDITY_MS,
            )?;
            let committee = self
                .inner
                .config
                .validators
                .iter()
                .map(|validator| validator.public_key().to_string())
                .collect::<Vec<_>>();
            let control = CommonwareControlLog::new(committee.clone())?;
            for finalized in &checkpoint.finalized {
                control.import_finalized(finalized.clone())?;
            }
            let block = control.propose(&committee[0], vec![command.control_envelope()?])?;
            let mut finalized = None;
            for validator in &committee {
                if let Some(value) = control.vote(validator, &block.digest)? {
                    finalized = Some(value);
                    break;
                }
            }
            let finalized = finalized
                .ok_or_else(|| "world director command did not reach quorum".to_string())?;
            let submission = FinalizedDirectorSubmission::issue(
                finalized.clone(),
                &self.inner.config.validators,
            )?;
            checkpoint.finalized.push(finalized.clone());
            let record = checkpoint
                .proposals
                .get_mut(proposal_id)
                .expect("proposal was checked above");
            let from = record.status;
            record.status = DirectorApprovalStatus::Finalizing;
            record.decided_by = Some(operator_id.to_string());
            record.decision_reason = Some(reason.trim().to_string());
            record.decided_at_ms = Some(now_ms);
            record.command_id = Some(command.payload.command_id.clone());
            record.finalized_height = Some(finalized.block.height);
            record.finalized_digest = Some(finalized.block.digest.clone());
            record.updated_at_ms = now_ms;
            checkpoint.policy_state.reward_budget_spent_today = checkpoint
                .policy_state
                .reward_budget_spent_today
                .saturating_add(existing.proposal.reward_budget);
            checkpoint
                .policy_state
                .active_event_ids
                .insert(command.payload.command_id.clone());
            append_audit(
                &mut checkpoint,
                Some(proposal_id.to_string()),
                "proposal.approved_and_finalized",
                operator_id,
                Some(from),
                Some(DirectorApprovalStatus::Finalizing),
                reason,
                now_ms,
            )?;
            let approval_audit_hash = checkpoint
                .audit
                .last()
                .map(|audit| audit.record_hash.clone())
                .ok_or_else(|| "world director approval audit was not recorded".to_string())?;
            checkpoint
                .proposals
                .get_mut(proposal_id)
                .expect("proposal was checked above")
                .approval_audit_hash = Some(approval_audit_hash.clone());
            self.persist(&mut checkpoint, now_ms)?;
            (
                submission,
                self.inner.config.delivery.clone(),
                command.payload.command_id,
                approval_audit_hash,
            )
        };

        let remote_anchor = anchor_remote_commonware(
            &self.inner.config,
            proposal_id,
            &command_id,
            &approval_audit_hash,
            now_ms_now(),
        );
        let delivery = match remote_anchor {
            Ok(receipt) => {
                if let Some(receipt) = receipt {
                    self.record_remote_anchor(proposal_id, &receipt, now_ms_now())?;
                }
                deliver_submission(&delivery_target, &submission)
            }
            Err(error) => Err(error),
        };
        self.complete_delivery(proposal_id, delivery, now_ms_now())
    }

    pub fn retry_delivery(
        &self,
        proposal_id: &str,
        operator_id: &str,
        reason: &str,
        now_ms: u64,
    ) -> Result<DirectorApprovalRecord, String> {
        require_reason(reason)?;
        let (submission, command_id, approval_audit_hash, remote_already_finalized) = {
            let mut checkpoint = self.lock_checkpoint()?;
            self.refresh_checkpoint(&mut checkpoint)?;
            if checkpoint.paused {
                return Err(format!(
                    "world director is paused: {}",
                    checkpoint.pause_reason.as_deref().unwrap_or("no reason")
                ));
            }
            let record = checkpoint
                .proposals
                .get(proposal_id)
                .cloned()
                .ok_or_else(|| format!("world director proposal not found: {proposal_id}"))?;
            if record.status != DirectorApprovalStatus::Failed {
                return Err(format!(
                    "only failed Zone delivery may be retried; {proposal_id} is {}",
                    record.status.as_str()
                ));
            }
            let height = record
                .finalized_height
                .ok_or_else(|| "failed proposal has no finalized height".to_string())?;
            let finalized = checkpoint
                .finalized
                .iter()
                .find(|finalized| finalized.block.height == height)
                .cloned()
                .ok_or_else(|| format!("finalized block {height} is unavailable"))?;
            let command_id = record
                .command_id
                .clone()
                .ok_or_else(|| "failed proposal has no signed command id".to_string())?;
            let approval_audit_hash = record
                .approval_audit_hash
                .clone()
                .ok_or_else(|| "failed proposal has no approval audit hash".to_string())?;
            let submission =
                FinalizedDirectorSubmission::issue(finalized, &self.inner.config.validators)?;
            let mutable = checkpoint
                .proposals
                .get_mut(proposal_id)
                .expect("proposal was checked above");
            mutable.status = DirectorApprovalStatus::Finalizing;
            mutable.last_error = None;
            mutable.updated_at_ms = now_ms;
            append_audit(
                &mut checkpoint,
                Some(proposal_id.to_string()),
                "proposal.zone_delivery_retried",
                operator_id,
                Some(DirectorApprovalStatus::Failed),
                Some(DirectorApprovalStatus::Finalizing),
                reason,
                now_ms,
            )?;
            self.persist(&mut checkpoint, now_ms)?;
            (
                submission,
                command_id,
                approval_audit_hash,
                record.commonware_network_height.is_some(),
            )
        };
        let remote_anchor = if remote_already_finalized {
            Ok(None)
        } else {
            anchor_remote_commonware(
                &self.inner.config,
                proposal_id,
                &command_id,
                &approval_audit_hash,
                now_ms_now(),
            )
        };
        let delivery = match remote_anchor {
            Ok(receipt) => {
                if let Some(receipt) = receipt {
                    self.record_remote_anchor(proposal_id, &receipt, now_ms_now())?;
                }
                deliver_submission(&self.inner.config.delivery, &submission)
            }
            Err(error) => Err(error),
        };
        self.complete_delivery(proposal_id, delivery, now_ms_now())
    }

    fn recover_finalizing_proposal(
        &self,
        proposal_id: &str,
        now_ms: u64,
    ) -> Result<DirectorApprovalRecord, String> {
        let (submission, command_id, approval_audit_hash, remote_already_finalized) = {
            let mut checkpoint = self.lock_checkpoint()?;
            self.refresh_checkpoint(&mut checkpoint)?;
            let record = checkpoint
                .proposals
                .get(proposal_id)
                .cloned()
                .ok_or_else(|| format!("world director proposal not found: {proposal_id}"))?;
            if record.status != DirectorApprovalStatus::Finalizing {
                return Err(format!(
                    "only finalizing proposals may be recovered; {proposal_id} is {}",
                    record.status.as_str()
                ));
            }
            let height = record
                .finalized_height
                .ok_or_else(|| "finalizing proposal has no finalized height".to_string())?;
            let finalized = checkpoint
                .finalized
                .iter()
                .find(|finalized| finalized.block.height == height)
                .cloned()
                .ok_or_else(|| format!("finalized block {height} is unavailable"))?;
            let command_id = record
                .command_id
                .clone()
                .ok_or_else(|| "finalizing proposal has no signed command id".to_string())?;
            let approval_audit_hash = record
                .approval_audit_hash
                .clone()
                .ok_or_else(|| "finalizing proposal has no approval audit hash".to_string())?;
            let submission =
                FinalizedDirectorSubmission::issue(finalized, &self.inner.config.validators)?;
            append_audit(
                &mut checkpoint,
                Some(proposal_id.to_string()),
                "proposal.inflight_recovery_started",
                "world-director-recovery",
                Some(DirectorApprovalStatus::Finalizing),
                Some(DirectorApprovalStatus::Finalizing),
                "resuming idempotent Commonware anchor and Zone delivery after process restart",
                now_ms,
            )?;
            self.persist(&mut checkpoint, now_ms)?;
            (
                submission,
                command_id,
                approval_audit_hash,
                record.commonware_network_height.is_some(),
            )
        };
        let remote_anchor = if remote_already_finalized {
            Ok(None)
        } else {
            anchor_remote_commonware(
                &self.inner.config,
                proposal_id,
                &command_id,
                &approval_audit_hash,
                now_ms_now(),
            )
        };
        let delivery = match remote_anchor {
            Ok(receipt) => {
                if let Some(receipt) = receipt {
                    self.record_remote_anchor(proposal_id, &receipt, now_ms_now())?;
                }
                deliver_submission(&self.inner.config.delivery, &submission)
            }
            Err(error) => Err(error),
        };
        self.complete_delivery(proposal_id, delivery, now_ms_now())
    }

    pub fn edit(
        &self,
        proposal_id: &str,
        operator_id: &str,
        reason: &str,
        patch: DirectorProposalPatch,
        now_ms: u64,
    ) -> Result<DirectorApprovalRecord, String> {
        require_reason(reason)?;
        if patch.duration_ms.is_none()
            && patch.reward_budget.is_none()
            && patch.target_zones.is_none()
        {
            return Err("director proposal edit contains no changes".to_string());
        }
        let mut checkpoint = self.lock_checkpoint()?;
        self.refresh_checkpoint(&mut checkpoint)?;
        checkpoint.policy_state.roll_day(now_ms);
        let existing = checkpoint
            .proposals
            .get(proposal_id)
            .cloned()
            .ok_or_else(|| format!("world director proposal not found: {proposal_id}"))?;
        if existing.status != DirectorApprovalStatus::PendingApproval {
            return Err(format!(
                "only pending proposals may be edited; {proposal_id} is {}",
                existing.status.as_str()
            ));
        }
        let mut proposal = existing.proposal.clone();
        if let Some(duration_ms) = patch.duration_ms {
            proposal.duration_ms = duration_ms;
        }
        if let Some(reward_budget) = patch.reward_budget {
            proposal.reward_budget = reward_budget;
        }
        if let Some(target_zones) = patch.target_zones {
            proposal.target_zones = target_zones;
        }
        proposal.generation = proposal.generation.saturating_add(1);
        WorldDirectorPolicy::mir2_default().approve(
            &proposal,
            &existing.snapshot,
            &existing.pressure_scores,
            &checkpoint.policy_state.policy_state(),
            now_ms,
        )?;
        let record = checkpoint
            .proposals
            .get_mut(proposal_id)
            .expect("proposal was checked above");
        record.proposal = proposal;
        record.updated_at_ms = now_ms;
        record.last_error = None;
        let result = record.clone();
        append_audit(
            &mut checkpoint,
            Some(proposal_id.to_string()),
            "proposal.edited",
            operator_id,
            Some(DirectorApprovalStatus::PendingApproval),
            Some(DirectorApprovalStatus::PendingApproval),
            reason,
            now_ms,
        )?;
        self.persist(&mut checkpoint, now_ms)?;
        Ok(result)
    }

    pub fn reject(
        &self,
        proposal_id: &str,
        operator_id: &str,
        reason: &str,
        now_ms: u64,
    ) -> Result<DirectorApprovalRecord, String> {
        self.decide_without_execution(
            proposal_id,
            operator_id,
            reason,
            DirectorApprovalStatus::Rejected,
            "proposal.rejected",
            now_ms,
        )
    }

    pub fn cancel(
        &self,
        proposal_id: &str,
        operator_id: &str,
        reason: &str,
        now_ms: u64,
    ) -> Result<DirectorApprovalRecord, String> {
        self.decide_without_execution(
            proposal_id,
            operator_id,
            reason,
            DirectorApprovalStatus::Cancelled,
            "proposal.cancelled",
            now_ms,
        )
    }

    fn decide_without_execution(
        &self,
        proposal_id: &str,
        operator_id: &str,
        reason: &str,
        next: DirectorApprovalStatus,
        action: &str,
        now_ms: u64,
    ) -> Result<DirectorApprovalRecord, String> {
        require_reason(reason)?;
        let mut checkpoint = self.lock_checkpoint()?;
        self.refresh_checkpoint(&mut checkpoint)?;
        let record = checkpoint
            .proposals
            .get_mut(proposal_id)
            .ok_or_else(|| format!("world director proposal not found: {proposal_id}"))?;
        if record.status != DirectorApprovalStatus::PendingApproval {
            return Err(format!(
                "only pending proposals may be rejected or cancelled; {} is {}",
                proposal_id,
                record.status.as_str()
            ));
        }
        let from = record.status;
        record.status = next;
        record.decided_by = Some(operator_id.to_string());
        record.decision_reason = Some(reason.trim().to_string());
        record.decided_at_ms = Some(now_ms);
        record.updated_at_ms = now_ms;
        let result = record.clone();
        append_audit(
            &mut checkpoint,
            Some(proposal_id.to_string()),
            action,
            operator_id,
            Some(from),
            Some(next),
            reason,
            now_ms,
        )?;
        self.persist(&mut checkpoint, now_ms)?;
        Ok(result)
    }

    pub fn set_paused(
        &self,
        paused: bool,
        operator_id: &str,
        reason: &str,
        now_ms: u64,
    ) -> Result<(), String> {
        require_reason(reason)?;
        let mut checkpoint = self.lock_checkpoint()?;
        self.refresh_checkpoint(&mut checkpoint)?;
        if checkpoint.paused == paused {
            return Ok(());
        }
        checkpoint.paused = paused;
        checkpoint.pause_reason = paused.then(|| reason.trim().to_string());
        append_audit(
            &mut checkpoint,
            None,
            if paused {
                "control.paused"
            } else {
                "control.resumed"
            },
            operator_id,
            None,
            None,
            reason,
            now_ms,
        )?;
        self.persist(&mut checkpoint, now_ms)
    }

    pub fn dashboard(&self, now_ms: u64) -> Result<DirectorApprovalDashboard, String> {
        let (mut proposals, audit, paused, pause_reason) = {
            let mut checkpoint = self.lock_checkpoint()?;
            self.refresh_checkpoint(&mut checkpoint)?;
            let changed = refresh_lifecycle(&mut checkpoint, now_ms);
            if changed {
                self.persist(&mut checkpoint, now_ms)?;
            }
            (
                checkpoint.proposals.values().cloned().collect::<Vec<_>>(),
                checkpoint
                    .audit
                    .iter()
                    .rev()
                    .take(100)
                    .cloned()
                    .collect::<Vec<_>>(),
                checkpoint.paused,
                checkpoint.pause_reason.clone(),
            )
        };
        proposals.sort_by_key(|record| std::cmp::Reverse(record.updated_at_ms));
        let runtime_statuses = runtime_statuses(&self.inner.config.delivery);
        let pending_count = proposals
            .iter()
            .filter(|record| record.status == DirectorApprovalStatus::PendingApproval)
            .count();
        let active_count = proposals
            .iter()
            .filter(|record| {
                matches!(
                    record.status,
                    DirectorApprovalStatus::Finalizing | DirectorApprovalStatus::Executing
                )
            })
            .count();
        Ok(DirectorApprovalDashboard {
            schema: "obelisk.world-director.approval.v1".to_string(),
            generated_at_ms: now_ms,
            paused,
            pause_reason,
            proposals,
            audit,
            configuration: DirectorApprovalConfiguration {
                execution_configured: self.inner.config.execution_configured(),
                persistence: self.inner.config.repository.label(),
                director_public_key: self
                    .inner
                    .config
                    .director
                    .as_ref()
                    .map(|identity| identity.public_key().to_string()),
                committee_size: self.inner.config.validators.len(),
                zone_host_count: self.inner.config.zone_host_count(),
                automatic_generation_enabled: self.inner.config.automatic_generation_enabled,
                generation_interval_seconds: self.inner.config.generation_interval_seconds,
                remote_commonware_configured: self.inner.config.commonware_gateway_url.is_some(),
                remote_commonware_required: self.inner.config.require_remote_commonware,
                proposal_generator: self.inner.config.proposal_generator.label().to_string(),
                ai_configured: self.inner.config.proposal_generator.ai_identity().is_some(),
                ai_provider: self
                    .inner
                    .config
                    .proposal_generator
                    .ai_identity()
                    .map(|(provider, _)| provider.to_string()),
                ai_model: self
                    .inner
                    .config
                    .proposal_generator
                    .ai_identity()
                    .map(|(_, model)| model.to_string()),
            },
            runtime_statuses,
            pending_count,
            active_count,
        })
    }

    pub(crate) fn prometheus_metrics(&self, now_ms: u64) -> Result<String, String> {
        let (status_counts, paused, audit_records, last_audit_at_ms, remote_anchors, zone_receipts) = {
            let mut checkpoint = self.lock_checkpoint()?;
            self.refresh_checkpoint(&mut checkpoint)?;
            if refresh_lifecycle(&mut checkpoint, now_ms) {
                self.persist(&mut checkpoint, now_ms)?;
            }
            let mut status_counts = BTreeMap::new();
            for status in [
                DirectorApprovalStatus::PendingApproval,
                DirectorApprovalStatus::Finalizing,
                DirectorApprovalStatus::Executing,
                DirectorApprovalStatus::Completed,
                DirectorApprovalStatus::Rejected,
                DirectorApprovalStatus::Cancelled,
                DirectorApprovalStatus::Failed,
            ] {
                status_counts.insert(
                    status.as_str(),
                    checkpoint
                        .proposals
                        .values()
                        .filter(|record| record.status == status)
                        .count(),
                );
            }
            (
                status_counts,
                checkpoint.paused,
                checkpoint
                    .archived_audit_records
                    .saturating_add(checkpoint.audit.len() as u64),
                checkpoint
                    .audit
                    .last()
                    .map(|record| record.occurred_at_ms)
                    .unwrap_or(0),
                checkpoint
                    .proposals
                    .values()
                    .filter(|record| record.commonware_network_height.is_some())
                    .count(),
                checkpoint
                    .proposals
                    .values()
                    .map(|record| record.zone_receipts.len())
                    .sum::<usize>(),
            )
        };
        let runtime_statuses = runtime_statuses(&self.inner.config.delivery);
        let live_targets = runtime_statuses
            .iter()
            .filter(|target| target.status == "live")
            .count();
        let unavailable_targets = runtime_statuses.len().saturating_sub(live_targets);
        let mut output = format!(
            "# HELP mir2_world_director_execution_configured Whether signed World Director execution is fully configured.\n\
             # TYPE mir2_world_director_execution_configured gauge\n\
             mir2_world_director_execution_configured {}\n\
             # HELP mir2_world_director_paused Whether global World Director approval is paused.\n\
             # TYPE mir2_world_director_paused gauge\n\
             mir2_world_director_paused {}\n\
             # HELP mir2_world_director_proposals Stored World Director proposals by lifecycle status.\n\
             # TYPE mir2_world_director_proposals gauge\n",
            u8::from(self.inner.config.execution_configured()),
            u8::from(paused),
        );
        for (status, count) in status_counts {
            output.push_str(&format!(
                "mir2_world_director_proposals{{status=\"{status}\"}} {count}\n"
            ));
        }
        output.push_str(&format!(
            "# HELP mir2_world_director_audit_records Tamper-evident World Director audit records.\n\
             # TYPE mir2_world_director_audit_records gauge\n\
             mir2_world_director_audit_records {audit_records}\n\
             # HELP mir2_world_director_last_audit_timestamp_seconds Unix timestamp of the latest World Director audit record.\n\
             # TYPE mir2_world_director_last_audit_timestamp_seconds gauge\n\
             mir2_world_director_last_audit_timestamp_seconds {}\n\
             # HELP mir2_world_director_remote_anchors Proposals anchored by the remote Commonware validator network.\n\
             # TYPE mir2_world_director_remote_anchors gauge\n\
             mir2_world_director_remote_anchors {remote_anchors}\n\
             # HELP mir2_world_director_zone_receipts Successful Zone Host execution receipts.\n\
             # TYPE mir2_world_director_zone_receipts gauge\n\
             mir2_world_director_zone_receipts {zone_receipts}\n\
             # HELP mir2_world_director_zone_targets Configured Zone Host targets by observed health.\n\
             # TYPE mir2_world_director_zone_targets gauge\n\
             mir2_world_director_zone_targets{{status=\"live\"}} {live_targets}\n\
             mir2_world_director_zone_targets{{status=\"unavailable\"}} {unavailable_targets}\n",
            last_audit_at_ms / 1_000,
        ));
        Ok(output)
    }

    fn lock_checkpoint(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, DirectorApprovalCheckpoint>, String> {
        self.inner
            .checkpoint
            .lock()
            .map_err(|_| "world director approval mutex poisoned".to_string())
    }

    fn refresh_checkpoint(
        &self,
        checkpoint: &mut DirectorApprovalCheckpoint,
    ) -> Result<(), String> {
        self.inner.config.repository.refresh_if_shared(checkpoint)?;
        validate_finalized_chain(checkpoint, &self.inner.config.validators)
    }

    fn persist(
        &self,
        checkpoint: &mut DirectorApprovalCheckpoint,
        now_ms: u64,
    ) -> Result<(), String> {
        self.inner.config.repository.save(checkpoint, now_ms)
    }

    fn complete_delivery(
        &self,
        proposal_id: &str,
        delivery: Result<Vec<Value>, String>,
        now_ms: u64,
    ) -> Result<DirectorApprovalRecord, String> {
        let mut checkpoint = self.lock_checkpoint()?;
        self.refresh_checkpoint(&mut checkpoint)?;
        let record = checkpoint
            .proposals
            .get_mut(proposal_id)
            .ok_or_else(|| format!("world director proposal disappeared: {proposal_id}"))?;
        if delivery.is_err()
            && matches!(
                record.status,
                DirectorApprovalStatus::Executing | DirectorApprovalStatus::Completed
            )
        {
            return Ok(record.clone());
        }
        let from = record.status;
        match delivery {
            Ok(receipts) => {
                if !matches!(
                    record.status,
                    DirectorApprovalStatus::Finalizing | DirectorApprovalStatus::Failed
                ) {
                    return Err(format!(
                        "cannot complete Zone delivery while {proposal_id} is {}",
                        record.status.as_str()
                    ));
                }
                record.status = DirectorApprovalStatus::Executing;
                record.zone_receipts = receipts;
                record.last_error = None;
            }
            Err(error) => {
                record.status = DirectorApprovalStatus::Failed;
                record.last_error = Some(error);
            }
        }
        record.updated_at_ms = now_ms;
        let next = record.status;
        let error = record.last_error.clone();
        let result = record.clone();
        append_audit(
            &mut checkpoint,
            Some(proposal_id.to_string()),
            if next == DirectorApprovalStatus::Executing {
                "proposal.zone_delivery_succeeded"
            } else {
                "proposal.zone_delivery_failed"
            },
            "world-director-control-plane",
            Some(from),
            Some(next),
            error
                .as_deref()
                .unwrap_or("finalized command installed by Zone Host"),
            now_ms,
        )?;
        self.persist(&mut checkpoint, now_ms)?;
        Ok(result)
    }

    fn record_remote_anchor(
        &self,
        proposal_id: &str,
        receipt: &RemoteCommonwareReceipt,
        now_ms: u64,
    ) -> Result<(), String> {
        let mut checkpoint = self.lock_checkpoint()?;
        self.refresh_checkpoint(&mut checkpoint)?;
        let record = checkpoint
            .proposals
            .get_mut(proposal_id)
            .ok_or_else(|| format!("world director proposal disappeared: {proposal_id}"))?;
        if record.commonware_network_height == Some(receipt.finalized_height)
            && record.commonware_network_command_digest.as_deref()
                == Some(receipt.command_digest.as_str())
        {
            return Ok(());
        }
        record.commonware_network_height = Some(receipt.finalized_height);
        record.commonware_network_state_root = Some(receipt.state_root.clone());
        record.commonware_network_command_digest = Some(receipt.command_digest.clone());
        record.updated_at_ms = now_ms;
        append_audit(
            &mut checkpoint,
            Some(proposal_id.to_string()),
            "proposal.remote_commonware_finalized",
            "gate14-commonware",
            Some(DirectorApprovalStatus::Finalizing),
            Some(DirectorApprovalStatus::Finalizing),
            &format!(
                "remote Commonware height {} state {}",
                receipt.finalized_height, receipt.state_root
            ),
            now_ms,
        )?;
        self.persist(&mut checkpoint, now_ms)
    }
}

pub fn now_ms_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn generate_director_proposal(
    generator: &DirectorProposalGenerator,
    policy: &WorldDirectorPolicy,
    snapshot: &WorldTelemetrySnapshot,
    scores: &DirectorPressureScores,
    now_ms: u64,
) -> Result<Option<DirectorProposal>, String> {
    match generator {
        DirectorProposalGenerator::RuleEngine => Ok(DirectorProposal::bichon_wooma_rule(
            snapshot, scores, now_ms,
        )),
        DirectorProposalGenerator::OpenAiResponses {
            endpoint,
            api_key,
            model,
            reasoning_effort,
            timeout_seconds,
        } => {
            let request = policy.ai_request(snapshot, scores)?;
            let body = json!({
                "model": model,
                "store": false,
                "reasoning": {
                    "effort": reasoning_effort
                },
                "text": {
                    "verbosity": "low",
                    "format": {
                        "type": "json_schema",
                        "name": "mir2_world_director_decision",
                        "strict": true,
                        "schema": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": {
                                "shouldPropose": { "type": "boolean" },
                                "templateId": { "type": "string", "maxLength": 160 },
                                "targetZones": {
                                    "type": "array",
                                    "maxItems": 8,
                                    "items": { "type": "string", "maxLength": 160 }
                                },
                                "durationMs": { "type": "integer", "minimum": 0 },
                                "rewardBudget": { "type": "integer", "minimum": 0 },
                                "rationale": { "type": "string", "maxLength": 512 }
                            },
                            "required": [
                                "shouldPropose",
                                "templateId",
                                "targetZones",
                                "durationMs",
                                "rewardBudget",
                                "rationale"
                            ]
                        }
                    }
                },
                "instructions": concat!(
                    "你是 Mir2 世界事件提案器。只做分析和提案，不执行任何操作。",
                    "只能选择输入中列出的 templateId 与 targetZones，不能创造动作、脚本、物品、封禁或数据库写入。",
                    "只有当压力指标达到模板阈值且事件能改善玩家体验时 shouldPropose=true；否则返回 false。",
                    "false 时 templateId、targetZones、durationMs、rewardBudget、rationale 分别返回空字符串、空数组、0、0 和简短原因。",
                    "true 时 rationale 使用简洁中文，并严格遵守输入预算。所有提案仍需人工审批和服务器策略校验。"
                ),
                "input": serde_json::to_string(&request)
                    .map_err(|error| format!("AI director request encode failed: {error}"))?,
                "max_output_tokens": 1_200,
                "safety_identifier": format!(
                    "mir2-world-director-{}",
                    snapshot.region_id.replace(|character: char| !character.is_ascii_alphanumeric(), "-")
                )
            });
            let client = HttpClient::builder()
                .timeout(Duration::from_secs(*timeout_seconds))
                .build()
                .map_err(|error| format!("AI director HTTP client build failed: {error}"))?;
            let response = client
                .post(endpoint)
                .bearer_auth(api_key)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .map_err(|error| format!("OpenAI Responses request failed: {error}"))?;
            let status = response.status();
            let bytes = response
                .bytes()
                .map_err(|error| format!("OpenAI Responses body read failed: {error}"))?;
            if bytes.len() > 512 * 1024 {
                return Err("OpenAI Responses body exceeded 512 KiB".to_string());
            }
            if !status.is_success() {
                let preview = String::from_utf8_lossy(&bytes)
                    .replace(['\r', '\n'], " ")
                    .chars()
                    .take(512)
                    .collect::<String>();
                return Err(format!(
                    "OpenAI Responses returned HTTP {}: {}",
                    status.as_u16(),
                    preview
                ));
            }
            let decision = decode_openai_director_decision(&bytes)?;
            materialize_ai_director_proposal(decision, snapshot, model, now_ms)
        }
    }
}

fn decode_openai_director_decision(response: &[u8]) -> Result<AiDirectorDecision, String> {
    let envelope = serde_json::from_slice::<OpenAiResponseEnvelope>(response)
        .map_err(|error| format!("OpenAI Responses JSON decode failed: {error}"))?;
    if envelope.status == "incomplete" {
        return Err(format!(
            "OpenAI Responses generation was incomplete: {}",
            envelope
                .incomplete_details
                .map(|value| value.to_string())
                .unwrap_or_else(|| "no details".to_string())
        ));
    }
    let mut refusal = None;
    let output_text = envelope.output.iter().find_map(|output| {
        output.content.iter().find_map(|content| {
            if content.content_type == "refusal" {
                refusal = content.refusal.clone();
                None
            } else if content.content_type == "output_text" {
                content.text.clone()
            } else {
                None
            }
        })
    });
    if let Some(refusal) = refusal {
        return Err(format!(
            "OpenAI refused the World Director proposal: {refusal}"
        ));
    }
    let output_text =
        output_text.ok_or_else(|| "OpenAI Responses did not contain output_text".to_string())?;
    serde_json::from_str(&output_text)
        .map_err(|error| format!("OpenAI structured World Director output was invalid: {error}"))
}

fn materialize_ai_director_proposal(
    decision: AiDirectorDecision,
    snapshot: &WorldTelemetrySnapshot,
    model: &str,
    now_ms: u64,
) -> Result<Option<DirectorProposal>, String> {
    if !decision.should_propose {
        return Ok(None);
    }
    if decision.template_id.trim().is_empty()
        || decision.target_zones.is_empty()
        || decision.duration_ms == 0
        || decision.reward_budget == 0
        || decision.rationale.trim().is_empty()
    {
        return Err("OpenAI proposed an incomplete World Director event".to_string());
    }
    let normalized = serde_json::to_vec(&json!({
        "snapshotId": snapshot.snapshot_id,
        "model": model,
        "templateId": decision.template_id,
        "targetZones": decision.target_zones,
        "durationMs": decision.duration_ms,
        "rewardBudget": decision.reward_budget,
        "rationale": decision.rationale,
        "nowMs": now_ms
    }))
    .map_err(|error| format!("AI director decision hash encode failed: {error}"))?;
    let digest = Sha256::digest(normalized);
    let mut seed_bytes = [0_u8; 8];
    seed_bytes.copy_from_slice(&digest[..8]);
    let seed = u64::from_be_bytes(seed_bytes).max(1);
    let suffix = hex(&digest[..8]);
    Ok(Some(DirectorProposal {
        proposal_id: format!("proposal:{}:ai:{suffix}", snapshot.snapshot_id),
        snapshot_id: snapshot.snapshot_id.clone(),
        template_id: decision.template_id,
        source: DirectorProposalSource::Ai {
            provider: "openai".to_string(),
            model: model.to_string(),
        },
        target_zones: decision.target_zones.into_iter().collect(),
        duration_ms: decision.duration_ms,
        reward_budget: decision.reward_budget,
        seed,
        generation: 1,
        rationale: decision.rationale,
    }))
}

fn proposal_risk(proposal: &DirectorProposal) -> DirectorRiskLevel {
    if proposal.reward_budget > 200_000 || proposal.target_zones.len() > 4 {
        DirectorRiskLevel::High
    } else if proposal.reward_budget > 50_000 || proposal.target_zones.len() > 2 {
        DirectorRiskLevel::Medium
    } else {
        DirectorRiskLevel::Low
    }
}

fn require_reason(reason: &str) -> Result<(), String> {
    let len = reason.trim().chars().count();
    if !(8..=512).contains(&len) {
        return Err("director decision reason must contain 8..=512 characters".to_string());
    }
    Ok(())
}

fn refresh_lifecycle(checkpoint: &mut DirectorApprovalCheckpoint, now_ms: u64) -> bool {
    checkpoint.policy_state.roll_day(now_ms);
    let mut completed = Vec::new();
    for record in checkpoint.proposals.values_mut() {
        if record.status != DirectorApprovalStatus::Executing {
            continue;
        }
        let Some(decided_at_ms) = record.decided_at_ms else {
            continue;
        };
        if now_ms
            >= decided_at_ms
                .saturating_add(record.proposal.duration_ms)
                .saturating_add(60_000)
        {
            record.status = DirectorApprovalStatus::Completed;
            record.updated_at_ms = now_ms;
            if let Some(command_id) = record.command_id.clone() {
                checkpoint.policy_state.active_event_ids.remove(&command_id);
            }
            checkpoint
                .policy_state
                .template_last_finished_at_ms
                .insert(record.proposal.template_id.clone(), now_ms);
            completed.push(record.proposal_id.clone());
        }
    }
    for proposal_id in &completed {
        let _ = append_audit(
            checkpoint,
            Some(proposal_id.clone()),
            "proposal.completed",
            "world-director-lifecycle",
            Some(DirectorApprovalStatus::Executing),
            Some(DirectorApprovalStatus::Completed),
            "event duration elapsed and lifecycle closed",
            now_ms,
        );
    }
    !completed.is_empty()
}

#[allow(clippy::too_many_arguments)]
fn append_audit(
    checkpoint: &mut DirectorApprovalCheckpoint,
    proposal_id: Option<String>,
    action: &str,
    actor_id: &str,
    from_status: Option<DirectorApprovalStatus>,
    to_status: Option<DirectorApprovalStatus>,
    reason: &str,
    occurred_at_ms: u64,
) -> Result<(), String> {
    let previous_hash = checkpoint
        .audit
        .last()
        .map(|record| record.record_hash.clone())
        .unwrap_or_else(|| checkpoint.audit_base_hash.clone());
    let from_status = from_status.map(|status| status.as_str().to_string());
    let to_status = to_status.map(|status| status.as_str().to_string());
    let hash_input = serde_json::to_vec(&(
        &previous_hash,
        &proposal_id,
        action,
        actor_id,
        &from_status,
        &to_status,
        reason.trim(),
        occurred_at_ms,
    ))
    .map_err(|error| format!("director audit encode failed: {error}"))?;
    let record_hash = hex(Sha256::digest(hash_input).as_slice());
    let audit_id = format!("wda-{}", &record_hash[..24]);
    checkpoint.audit.push(DirectorAuditRecord {
        audit_id,
        proposal_id,
        action: action.to_string(),
        actor_id: actor_id.to_string(),
        from_status,
        to_status,
        reason: reason.trim().to_string(),
        occurred_at_ms,
        previous_hash,
        record_hash,
    });
    if checkpoint.audit.len() > MAX_AUDIT_RECORDS {
        let remove_count = checkpoint.audit.len() - MAX_AUDIT_RECORDS;
        let removed = checkpoint.audit.drain(..remove_count).collect::<Vec<_>>();
        if let Some(last) = removed.last() {
            checkpoint.audit_base_hash = last.record_hash.clone();
        }
        checkpoint.archived_audit_records = checkpoint
            .archived_audit_records
            .saturating_add(removed.len() as u64);
    }
    Ok(())
}

fn validate_checkpoint(checkpoint: &DirectorApprovalCheckpoint) -> Result<(), String> {
    if checkpoint.version != CHECKPOINT_VERSION {
        return Err(format!(
            "unsupported world director approval checkpoint version {}",
            checkpoint.version
        ));
    }
    if checkpoint.audit_base_hash.is_empty() {
        return Err("world director audit base hash is empty".to_string());
    }
    let mut previous_hash = checkpoint.audit_base_hash.clone();
    for audit in &checkpoint.audit {
        if audit.previous_hash != previous_hash {
            return Err(format!(
                "world director audit chain broke at {}",
                audit.audit_id
            ));
        }
        let hash_input = serde_json::to_vec(&(
            &audit.previous_hash,
            &audit.proposal_id,
            audit.action.as_str(),
            audit.actor_id.as_str(),
            &audit.from_status,
            &audit.to_status,
            audit.reason.as_str(),
            audit.occurred_at_ms,
        ))
        .map_err(|error| format!("director audit encode failed: {error}"))?;
        let expected = hex(Sha256::digest(hash_input).as_slice());
        if audit.record_hash != expected {
            return Err(format!(
                "world director audit hash mismatch at {}",
                audit.audit_id
            ));
        }
        previous_hash = audit.record_hash.clone();
    }
    Ok(())
}

fn genesis_audit_hash() -> String {
    "genesis".to_string()
}

fn validate_finalized_chain(
    checkpoint: &DirectorApprovalCheckpoint,
    validators: &[NodeSigningIdentity],
) -> Result<(), String> {
    if checkpoint.finalized.is_empty() || validators.is_empty() {
        return Ok(());
    }
    let committee = validators
        .iter()
        .map(|validator| validator.public_key().to_string())
        .collect::<Vec<_>>();
    let control = CommonwareControlLog::new(committee)?;
    for finalized in &checkpoint.finalized {
        control.import_finalized(finalized.clone())?;
    }
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteCommonwareStatus {
    finalized_height: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteCommonwareReceipt {
    accepted: bool,
    command_digest: String,
    finalized_height: u64,
    state_root: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteCommonwareAnchorLookup {
    finalized_height: u64,
    state_root: String,
    anchor: RemoteCommonwareAnchor,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteCommonwareAnchor {
    proposal_id: String,
    payload_digest: String,
    approval_audit_hash: String,
    finalized_height: u64,
    command_digest: String,
}

fn anchor_remote_commonware(
    config: &DirectorApprovalConfig,
    proposal_id: &str,
    command_id: &str,
    approval_audit_hash: &str,
    now_ms: u64,
) -> Result<Option<RemoteCommonwareReceipt>, String> {
    let Some(base_url) = config.commonware_gateway_url.as_deref() else {
        if config.require_remote_commonware {
            return Err("remote Commonware gateway is required but not configured".to_string());
        }
        return Ok(None);
    };
    let client = HttpClient::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|error| format!("remote Commonware HTTP client failed: {error}"))?;
    let mut last_error = None;
    for _ in 0..3 {
        match lookup_remote_commonware_anchor(
            &client,
            config,
            proposal_id,
            command_id,
            approval_audit_hash,
        ) {
            Ok(Some(receipt)) => return Ok(Some(receipt)),
            Ok(None) => {}
            Err(error) => {
                last_error = Some(error);
                continue;
            }
        }
        let mut status_request = client.get(format!("{base_url}/v1/status"));
        if let Some(token) = config.commonware_gateway_token.as_deref() {
            status_request = status_request.bearer_auth(token);
        }
        let status = status_request
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
            .and_then(reqwest::blocking::Response::json::<RemoteCommonwareStatus>)
            .map_err(|error| format!("remote Commonware status failed: {error}"))?;
        let envelope = Gate14CommandEnvelope {
            sequence: status.finalized_height.saturating_add(1),
            idempotency_key: format!("world-director:{command_id}"),
            submitted_at_ms: now_ms,
            command: Gate14Command::AnchorWorldDirector {
                command_id: command_id.to_string(),
                proposal_id: proposal_id.to_string(),
                payload_digest: command_id.to_string(),
                approval_audit_hash: approval_audit_hash.to_string(),
            },
        };
        envelope.validate()?;
        let expected_command_digest = envelope.digest()?;
        let mut submit_request = client
            .post(format!("{base_url}/v1/control/commands"))
            .json(&envelope);
        if let Some(token) = config.commonware_gateway_token.as_deref() {
            submit_request = submit_request.bearer_auth(token);
        }
        match submit_request.send() {
            Ok(response) if response.status().is_success() => {
                let receipt = response
                    .json::<RemoteCommonwareReceipt>()
                    .map_err(|error| format!("remote Commonware receipt decode failed: {error}"))?;
                if !receipt.accepted
                    || receipt.finalized_height != envelope.sequence
                    || receipt.command_digest != expected_command_digest
                    || !is_hex_digest(&receipt.state_root)
                {
                    return Err("remote Commonware returned inconsistent finality".to_string());
                }
                return Ok(Some(receipt));
            }
            Ok(response) => {
                last_error = Some(format!(
                    "remote Commonware rejected anchor with HTTP {}",
                    response.status()
                ));
            }
            Err(error) => {
                last_error = Some(format!("remote Commonware anchor failed: {error}"));
            }
        }
    }
    Err(last_error.unwrap_or_else(|| "remote Commonware anchor failed".to_string()))
}

fn lookup_remote_commonware_anchor(
    client: &HttpClient,
    config: &DirectorApprovalConfig,
    proposal_id: &str,
    command_id: &str,
    approval_audit_hash: &str,
) -> Result<Option<RemoteCommonwareReceipt>, String> {
    let base_url = config
        .commonware_gateway_url
        .as_deref()
        .ok_or_else(|| "remote Commonware gateway is not configured".to_string())?;
    let mut request = client.get(format!("{base_url}/v1/world-director/anchors/{command_id}"));
    if let Some(token) = config.commonware_gateway_token.as_deref() {
        request = request.bearer_auth(token);
    }
    let response = request
        .send()
        .map_err(|error| format!("remote Commonware anchor lookup failed: {error}"))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let lookup = response
        .error_for_status()
        .map_err(|error| format!("remote Commonware anchor lookup failed: {error}"))?
        .json::<RemoteCommonwareAnchorLookup>()
        .map_err(|error| format!("remote Commonware anchor lookup decode failed: {error}"))?;
    if lookup.anchor.proposal_id != proposal_id
        || lookup.anchor.payload_digest != command_id
        || lookup.anchor.approval_audit_hash != approval_audit_hash
        || lookup.anchor.finalized_height == 0
        || !is_hex_digest(&lookup.anchor.command_digest)
        || !is_hex_digest(&lookup.state_root)
        || lookup.finalized_height < lookup.anchor.finalized_height
    {
        return Err("remote Commonware anchor lookup returned inconsistent evidence".to_string());
    }
    Ok(Some(RemoteCommonwareReceipt {
        accepted: true,
        command_digest: lookup.anchor.command_digest,
        finalized_height: lookup.anchor.finalized_height,
        state_root: lookup.state_root,
    }))
}

fn is_hex_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn deliver_submission(
    delivery: &DirectorDelivery,
    submission: &FinalizedDirectorSubmission,
) -> Result<Vec<Value>, String> {
    match delivery {
        DirectorDelivery::Disabled => {
            Err("world director Zone Host delivery is not configured".to_string())
        }
        DirectorDelivery::Http {
            endpoints,
            management_token,
        } => {
            let client = HttpClient::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .map_err(|error| format!("world director HTTP client failed: {error}"))?;
            let mut receipts = Vec::new();
            for endpoint in endpoints {
                let response = client
                    .post(format!("{endpoint}/v1/world-director/finalized"))
                    .bearer_auth(management_token)
                    .json(submission)
                    .send()
                    .map_err(|error| format!("Zone Host {endpoint} delivery failed: {error}"))?
                    .error_for_status()
                    .map_err(|error| format!("Zone Host {endpoint} rejected finality: {error}"))?;
                let receipt =
                    response
                        .json::<FinalizedDirectorInstallReceipt>()
                        .map_err(|error| {
                            format!("Zone Host {endpoint} receipt decode failed: {error}")
                        })?;
                receipts.push(
                    serde_json::to_value(receipt)
                        .map_err(|error| format!("Zone Host receipt encode failed: {error}"))?,
                );
            }
            Ok(receipts)
        }
        #[cfg(test)]
        DirectorDelivery::Test => Ok(vec![serde_json::json!({
            "accepted": true,
            "finalizedHeight": submission.finalized.block.height,
        })]),
    }
}

fn runtime_statuses(delivery: &DirectorDelivery) -> Vec<DirectorRuntimeTargetStatus> {
    let DirectorDelivery::Http {
        endpoints,
        management_token,
    } = delivery
    else {
        return Vec::new();
    };
    let client = match HttpClient::builder()
        .timeout(Duration::from_secs(3))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return endpoints
                .iter()
                .map(|endpoint| DirectorRuntimeTargetStatus {
                    endpoint: endpoint.clone(),
                    status: "unavailable".to_string(),
                    error: Some(error.to_string()),
                    runtime: None,
                })
                .collect();
        }
    };
    endpoints
        .iter()
        .map(|endpoint| {
            match client
                .get(format!("{endpoint}/v1/world-director"))
                .bearer_auth(management_token)
                .send()
                .and_then(reqwest::blocking::Response::error_for_status)
                .and_then(reqwest::blocking::Response::json::<WorldDirectorRuntimeStatus>)
            {
                Ok(runtime) => DirectorRuntimeTargetStatus {
                    endpoint: endpoint.clone(),
                    status: "live".to_string(),
                    error: None,
                    runtime: Some(runtime),
                },
                Err(error) => DirectorRuntimeTargetStatus {
                    endpoint: endpoint.clone(),
                    status: "unavailable".to_string(),
                    error: Some(error.to_string()),
                    runtime: None,
                },
            }
        })
        .collect()
}

fn identity_from_pair(
    inline_name: &str,
    file_name: &str,
) -> Result<Option<NodeSigningIdentity>, String> {
    let inline = env::var(inline_name)
        .ok()
        .filter(|value| !value.trim().is_empty());
    let file = env::var(file_name)
        .ok()
        .filter(|value| !value.trim().is_empty());
    match (inline, file) {
        (Some(_), Some(_)) => Err(format!("configure {inline_name} or {file_name}, not both")),
        (Some(value), None) => NodeSigningIdentity::from_base64_seed(&value).map(Some),
        (None, Some(path)) => NodeSigningIdentity::from_file(path).map(Some),
        (None, None) => Ok(None),
    }
}

fn validator_identities_from_env() -> Result<Vec<NodeSigningIdentity>, String> {
    let inline = env::var("MIR2_WORLD_DIRECTOR_VALIDATOR_KEYS")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let files = env::var("MIR2_WORLD_DIRECTOR_VALIDATOR_KEY_FILES")
        .ok()
        .filter(|value| !value.trim().is_empty());
    match (inline, files) {
        (Some(_), Some(_)) => Err(
            "configure MIR2_WORLD_DIRECTOR_VALIDATOR_KEYS or MIR2_WORLD_DIRECTOR_VALIDATOR_KEY_FILES, not both"
                .to_string(),
        ),
        (Some(values), None) => values
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(NodeSigningIdentity::from_base64_seed)
            .collect(),
        (None, Some(values)) => values
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(NodeSigningIdentity::from_file)
            .collect(),
        (None, None) => Ok(Vec::new()),
    }
}

fn env_flag(name: &str) -> bool {
    env::var(name).ok().is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes"
        )
    })
}

fn deployment_is_production() -> bool {
    [
        "MIR2_RUNTIME_ENV",
        "MIR2_DEPLOYMENT_ENV",
        "MIR2_ENV",
        "VERCEL_ENV",
    ]
    .into_iter()
    .filter_map(|name| env::var(name).ok())
    .any(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "production" | "prod"
        )
    })
}

fn atomic_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create world director checkpoint directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("world director checkpoint encode failed: {error}"))?;
    let temporary = path.with_extension(format!(
        "tmp-{}-{}",
        std::process::id(),
        ATOMIC_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&temporary, [&bytes[..], b"\n"].concat()).map_err(|error| {
        format!(
            "failed to write world director checkpoint {}: {error}",
            temporary.display()
        )
    })?;
    fs::rename(&temporary, path).map_err(|error| {
        format!(
            "failed to publish world director checkpoint {}: {error}",
            path.display()
        )
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_gateway::{
        EconomyTelemetrySnapshot, GuildTelemetrySnapshot, MapTelemetrySnapshot,
        WORLD_DIRECTOR_SCHEMA,
    };
    use std::io::{Read, Write};
    use std::net::TcpListener;

    fn service() -> WorldDirectorApprovalService {
        service_with_commonware(None)
    }

    fn service_with_commonware(
        commonware_gateway_url: Option<String>,
    ) -> WorldDirectorApprovalService {
        WorldDirectorApprovalService::new(DirectorApprovalConfig {
            repository: DirectorRepository::File(std::env::temp_dir().join(format!(
                "mir2-world-director-approval-{}-{}.json",
                std::process::id(),
                now_ms_now()
            ))),
            director: Some(NodeSigningIdentity::from_seed([71; 32])),
            validators: vec![
                NodeSigningIdentity::from_seed([81; 32]),
                NodeSigningIdentity::from_seed([82; 32]),
                NodeSigningIdentity::from_seed([83; 32]),
                NodeSigningIdentity::from_seed([84; 32]),
            ],
            delivery: DirectorDelivery::Test,
            automatic_generation_enabled: true,
            generation_interval_seconds: 60,
            commonware_gateway_url,
            commonware_gateway_token: None,
            require_remote_commonware: false,
            proposal_generator: DirectorProposalGenerator::RuleEngine,
        })
        .unwrap()
    }

    fn snapshot(now_ms: u64) -> WorldTelemetrySnapshot {
        WorldTelemetrySnapshot {
            schema: WORLD_DIRECTOR_SCHEMA.to_string(),
            snapshot_id: format!("approval-test-{now_ms}"),
            game_id: "mir2".to_string(),
            region_id: "asia-hk".to_string(),
            observed_at_ms: now_ms,
            window_ms: 15 * 60 * 1_000,
            maps: vec![
                map("map:0", 80, 18, 20, 8_000, 8),
                map("map:D022", 20, 26, 0, 4_000, 4),
                map("map:D023", 12, 29, 0, 3_000, 6),
                map("map:D024", 8, 31, 0, 1_000, 12),
            ],
            economy: EconomyTelemetrySnapshot {
                gold_created: 2_000_000,
                gold_destroyed: 1_200_000,
                median_trade_price_index_bps: 11_200,
            },
            guilds: GuildTelemetrySnapshot {
                active_guilds: 9,
                largest_guild_population_bps: 2_500,
                largest_guild_boss_kill_share_bps: 5_800,
            },
        }
    }

    fn map(
        zone_id: &str,
        active_players: u32,
        median_level: u16,
        new_player_count: u32,
        monster_kills: u64,
        boss_kills: u64,
    ) -> MapTelemetrySnapshot {
        MapTelemetrySnapshot {
            zone_id: zone_id.to_string(),
            active_players,
            median_level,
            new_player_count,
            returning_player_count: 0,
            monster_kills,
            boss_kills,
            player_deaths: boss_kills.saturating_mul(2),
            completed_quests: active_players as u64 / 4,
        }
    }

    #[test]
    fn openai_structured_output_materializes_a_policy_bounded_proposal() {
        let now_ms = 1_800_000_000_000;
        let response = json!({
            "status": "completed",
            "output": [{
                "type": "message",
                "content": [{
                    "type": "output_text",
                    "text": serde_json::to_string(&json!({
                        "shouldPropose": true,
                        "templateId": "mir2.bichon-wooma-awakening.v1",
                        "targetZones": ["map:D022", "map:D024"],
                        "durationMs": 1_800_000,
                        "rewardBudget": 120_000,
                        "rationale": "沃玛区域疲劳度较高，建议开启受限事件并保留人工审批。"
                    })).unwrap()
                }]
            }]
        });
        let decision =
            decode_openai_director_decision(&serde_json::to_vec(&response).unwrap()).unwrap();
        let snapshot = snapshot(now_ms);
        let proposal =
            materialize_ai_director_proposal(decision, &snapshot, "gpt-5.6-terra", now_ms)
                .unwrap()
                .unwrap();
        assert!(matches!(
            proposal.source,
            DirectorProposalSource::Ai {
                ref provider,
                ref model
            } if provider == "openai" && model == "gpt-5.6-terra"
        ));
        let scores = DirectorPressureScores::from_snapshot(&snapshot).unwrap();
        WorldDirectorPolicy::mir2_default()
            .approve(
                &proposal,
                &snapshot,
                &scores,
                &DirectorPolicyState::default(),
                now_ms,
            )
            .unwrap();
    }

    #[test]
    fn openai_no_event_decision_returns_no_proposal() {
        let response = json!({
            "status": "completed",
            "output": [{
                "type": "message",
                "content": [{
                    "type": "output_text",
                    "text": serde_json::to_string(&json!({
                        "shouldPropose": false,
                        "templateId": "",
                        "targetZones": [],
                        "durationMs": 0,
                        "rewardBudget": 0,
                        "rationale": "当前压力不足，不生成事件。"
                    })).unwrap()
                }]
            }]
        });
        let decision =
            decode_openai_director_decision(&serde_json::to_vec(&response).unwrap()).unwrap();
        assert!(
            materialize_ai_director_proposal(
                decision,
                &snapshot(1_800_000_010_000),
                "gpt-5.6-terra",
                1_800_000_010_000,
            )
            .unwrap()
            .is_none()
        );
    }

    #[test]
    fn openai_refusal_is_reported_instead_of_falling_back_to_rules() {
        let response = json!({
            "status": "completed",
            "output": [{
                "type": "message",
                "content": [{
                    "type": "refusal",
                    "refusal": "request refused"
                }]
            }]
        });
        let error =
            decode_openai_director_decision(&serde_json::to_vec(&response).unwrap()).unwrap_err();
        assert!(error.contains("refused"));
    }

    #[test]
    fn openai_responses_adapter_posts_schema_and_returns_ai_proposal() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = vec![0_u8; 64 * 1024];
            let read = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..read]);
            assert!(request.starts_with("POST /v1/responses "));
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer test-api-key")
            );
            let body = request.split_once("\r\n\r\n").unwrap().1;
            let body = serde_json::from_str::<Value>(body).unwrap();
            assert_eq!(body["model"], "gpt-5.6-terra");
            assert_eq!(body["store"], false);
            assert_eq!(body["text"]["format"]["type"], "json_schema");
            assert_eq!(body["text"]["format"]["strict"], true);

            let decision = serde_json::to_string(&json!({
                "shouldPropose": true,
                "templateId": "mir2.bichon-wooma-awakening.v1",
                "targetZones": ["map:D022", "map:D024"],
                "durationMs": 1_800_000,
                "rewardBudget": 120_000,
                "rationale": "真实模型经安全闸生成的受限提案。"
            }))
            .unwrap();
            let response = json!({
                "status": "completed",
                "output": [{
                    "type": "message",
                    "content": [{
                        "type": "output_text",
                        "text": decision
                    }]
                }]
            })
            .to_string();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                response.len(),
                response
            )
            .unwrap();
        });

        let now_ms = 1_800_000_005_000;
        let snapshot = snapshot(now_ms);
        let scores = DirectorPressureScores::from_snapshot(&snapshot).unwrap();
        let proposal = generate_director_proposal(
            &DirectorProposalGenerator::OpenAiResponses {
                endpoint: format!("http://{address}/v1/responses"),
                api_key: "test-api-key".to_string(),
                model: "gpt-5.6-terra".to_string(),
                reasoning_effort: "low".to_string(),
                timeout_seconds: 5,
            },
            &WorldDirectorPolicy::mir2_default(),
            &snapshot,
            &scores,
            now_ms,
        )
        .unwrap()
        .unwrap();
        server.join().unwrap();
        assert!(matches!(proposal.source, DirectorProposalSource::Ai { .. }));
        WorldDirectorPolicy::mir2_default()
            .approve(
                &proposal,
                &snapshot,
                &scores,
                &DirectorPolicyState::default(),
                now_ms,
            )
            .unwrap();
    }

    #[test]
    fn proposal_requires_human_approval_then_reaches_zone_delivery() {
        let service = service();
        let now_ms = 1_800_000_000_000;
        let proposal = service
            .generate(snapshot(now_ms), "scheduler", now_ms)
            .unwrap()
            .unwrap();
        assert_eq!(proposal.status, DirectorApprovalStatus::PendingApproval);

        let approved = service
            .approve(
                &proposal.proposal_id,
                "ops-peer",
                "批准低风险沃玛事件用于人工审批验收",
                now_ms + 1,
            )
            .unwrap();
        assert_eq!(approved.status, DirectorApprovalStatus::Executing);
        assert_eq!(approved.finalized_height, Some(1));
        assert_eq!(approved.zone_receipts.len(), 1);

        let dashboard = service.dashboard(now_ms + 2).unwrap();
        assert_eq!(dashboard.pending_count, 0);
        assert_eq!(dashboard.active_count, 1);
        assert!(
            dashboard
                .audit
                .iter()
                .any(|record| record.action == "proposal.zone_delivery_succeeded")
        );
    }

    #[test]
    fn global_pause_blocks_approval_without_losing_the_proposal() {
        let service = service();
        let now_ms = 1_800_000_100_000;
        let proposal = service
            .generate(snapshot(now_ms), "scheduler", now_ms)
            .unwrap()
            .unwrap();
        service
            .set_paused(
                true,
                "incident-commander",
                "暂停导演审批以处理线上告警",
                now_ms + 1,
            )
            .unwrap();
        let error = service
            .approve(
                &proposal.proposal_id,
                "ops-peer",
                "尝试批准应被全局暂停阻止",
                now_ms + 2,
            )
            .unwrap_err();
        assert!(error.contains("paused"));
        assert_eq!(
            service.dashboard(now_ms + 3).unwrap().proposals[0].status,
            DirectorApprovalStatus::PendingApproval
        );
    }

    #[test]
    fn prometheus_metrics_report_control_and_execution_state() {
        let service = service();
        let now_ms = 1_800_000_150_000;
        let proposal = service
            .generate(snapshot(now_ms), "scheduler", now_ms)
            .unwrap()
            .unwrap();
        service
            .approve(
                &proposal.proposal_id,
                "ops-peer",
                "批准以验证导演运行指标完整回传",
                now_ms + 1,
            )
            .unwrap();
        let metrics = service.prometheus_metrics(now_ms + 2).unwrap();
        assert!(metrics.contains("mir2_world_director_proposals{status=\"executing\"} 1"));
        let audit_records = metrics
            .lines()
            .find_map(|line| {
                line.strip_prefix("mir2_world_director_audit_records ")
                    .and_then(|value| value.parse::<usize>().ok())
            })
            .unwrap();
        assert!(audit_records >= 2);
        assert!(metrics.contains("mir2_world_director_zone_receipts 1"));
        assert!(metrics.contains("mir2_world_director_paused 0"));
    }

    #[test]
    fn startup_recovery_resumes_a_finalized_command_idempotently() {
        let service = service();
        let now_ms = 1_800_000_175_000;
        let proposal = service
            .generate(snapshot(now_ms), "scheduler", now_ms)
            .unwrap()
            .unwrap();
        service
            .approve(
                &proposal.proposal_id,
                "ops-peer",
                "批准后模拟进程在保存 Zone 回执之前退出",
                now_ms + 1,
            )
            .unwrap();
        {
            let mut checkpoint = service.inner.checkpoint.lock().unwrap();
            let record = checkpoint.proposals.get_mut(&proposal.proposal_id).unwrap();
            record.status = DirectorApprovalStatus::Finalizing;
            record.zone_receipts.clear();
        }
        assert_eq!(service.recover_inflight(now_ms + 2).unwrap(), 1);
        let recovered = service
            .dashboard(now_ms + 3)
            .unwrap()
            .proposals
            .into_iter()
            .find(|record| record.proposal_id == proposal.proposal_id)
            .unwrap();
        assert_eq!(recovered.status, DirectorApprovalStatus::Executing);
        assert_eq!(recovered.zone_receipts.len(), 1);
    }

    #[test]
    fn stale_delivery_failure_cannot_regress_an_executing_proposal() {
        let service = service();
        let now_ms = 1_800_000_180_000;
        let proposal = service
            .generate(snapshot(now_ms), "scheduler", now_ms)
            .unwrap()
            .unwrap();
        let approved = service
            .approve(
                &proposal.proposal_id,
                "ops-peer",
                "批准后验证迟到的失败回执不能覆盖成功状态",
                now_ms + 1,
            )
            .unwrap();
        assert_eq!(approved.status, DirectorApprovalStatus::Executing);
        let after_stale_failure = service
            .complete_delivery(
                &proposal.proposal_id,
                Err("late failure from a competing recovery attempt".to_string()),
                now_ms + 2,
            )
            .unwrap();
        assert_eq!(
            after_stale_failure.status,
            DirectorApprovalStatus::Executing
        );
        assert!(after_stale_failure.last_error.is_none());
    }

    #[test]
    fn audit_checkpoint_compacts_with_a_verifiable_chain_anchor() {
        let mut checkpoint = DirectorApprovalCheckpoint::default();
        for index in 0..=MAX_AUDIT_RECORDS {
            append_audit(
                &mut checkpoint,
                None,
                &format!("test.audit.{index}"),
                "test",
                None,
                None,
                "retention test",
                1_800_000_190_000 + index as u64,
            )
            .unwrap();
        }
        assert_eq!(checkpoint.audit.len(), MAX_AUDIT_RECORDS);
        assert_eq!(checkpoint.archived_audit_records, 1);
        assert_eq!(
            checkpoint.audit.first().unwrap().previous_hash,
            checkpoint.audit_base_hash
        );
        validate_checkpoint(&checkpoint).unwrap();
    }

    #[test]
    fn approval_is_anchored_to_remote_commonware_before_zone_delivery() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for request_index in 0..3 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = vec![0_u8; 32 * 1024];
                let read = stream.read(&mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..read]);
                let (status, body) = if request_index == 0 {
                    assert!(request.starts_with("GET /v1/world-director/anchors/"));
                    ("404 Not Found", r#"{"error":"not found"}"#.to_string())
                } else if request_index == 1 {
                    assert!(request.starts_with("GET /v1/status "));
                    ("200 OK", r#"{"finalizedHeight":0}"#.to_string())
                } else {
                    assert!(request.starts_with("POST /v1/control/commands "));
                    assert!(request.contains("anchorWorldDirector"));
                    let envelope = serde_json::from_str::<Gate14CommandEnvelope>(
                        request.split_once("\r\n\r\n").unwrap().1,
                    )
                    .unwrap();
                    (
                        "200 OK",
                        format!(
                            r#"{{"accepted":true,"commandDigest":"{}","finalizedHeight":1,"stateRoot":"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"}}"#,
                            envelope.digest().unwrap()
                        ),
                    )
                };
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
            }
        });
        let service = service_with_commonware(Some(format!("http://{address}")));
        let now_ms = 1_800_000_200_000;
        let proposal = service
            .generate(snapshot(now_ms), "scheduler", now_ms)
            .unwrap()
            .unwrap();
        let approved = service
            .approve(
                &proposal.proposal_id,
                "ops-peer",
                "批准并验证远程 Commonware 锚定后再投递地图节点",
                now_ms + 1,
            )
            .unwrap();
        server.join().unwrap();
        assert_eq!(approved.status, DirectorApprovalStatus::Executing);
        assert_eq!(approved.commonware_network_height, Some(1));
        assert_eq!(
            approved.commonware_network_state_root.as_deref(),
            Some("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee")
        );
    }

    #[test]
    fn existing_remote_anchor_recovers_a_lost_submit_response_without_resubmission() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let service = service_with_commonware(Some(format!("http://{address}")));
        let now_ms = 1_800_000_250_000;
        let proposal = service
            .generate(snapshot(now_ms), "scheduler", now_ms)
            .unwrap()
            .unwrap();
        let proposal_id = proposal.proposal_id.clone();
        let service_for_server = service.clone();
        let proposal_for_server = proposal_id.clone();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = vec![0_u8; 32 * 1024];
            let read = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..read]);
            assert!(request.starts_with("GET /v1/world-director/anchors/"));
            let path = request.split_whitespace().nth(1).unwrap();
            let command_id = path.rsplit('/').next().unwrap();
            let approval_audit_hash = service_for_server
                .inner
                .checkpoint
                .lock()
                .unwrap()
                .proposals
                .get(&proposal_for_server)
                .unwrap()
                .approval_audit_hash
                .clone()
                .unwrap();
            let body = serde_json::json!({
                "gatewayId": "recovery-gateway",
                "finalizedHeight": 7,
                "stateRoot": "f".repeat(64),
                "anchor": {
                    "commandId": command_id,
                    "proposalId": proposal_for_server,
                    "payloadDigest": command_id,
                    "approvalAuditHash": approval_audit_hash,
                    "finalizedHeight": 7,
                    "commandDigest": "d".repeat(64)
                }
            })
            .to_string();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });
        let approved = service
            .approve(
                &proposal_id,
                "ops-peer",
                "恢复此前已经终局但提交回执丢失的远程锚点",
                now_ms + 1,
            )
            .unwrap();
        server.join().unwrap();
        assert_eq!(approved.status, DirectorApprovalStatus::Executing);
        assert_eq!(approved.commonware_network_height, Some(7));
        assert_eq!(
            approved.commonware_network_command_digest.as_deref(),
            Some("dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd")
        );
    }
}
